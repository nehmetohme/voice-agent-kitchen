//! `PurposeRepo` on Postgres. Every mutation is one transaction: the operator row, the
//! purpose (and a new version) and the audit event commit together or not at all. Versions
//! are only ever inserted; a trigger rejects any change to one.

use jiff_sqlx::ToSqlx as _;
use serde::Serialize;
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;
use vak_domain::{
    Purpose, PurposeAuditAction, PurposeAuditEvent, PurposeId, PurposeName, PurposeRef,
    PurposeVersion, UseCase,
};
use vak_repositories::{
    BoxFuture, PurposeListing, PurposeQuery, PurposeRepo, RepoError, Revision, Versioned,
};

use crate::agents::{DEFAULT_WORKSPACE, map_error, to_i64, upsert_operator};
use crate::purpose_records::DefinitionRecord;
use crate::records::corrupt;

/// `audit_events.details` of a purpose event.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PurposeAuditDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hash: Option<String>,
}

impl PurposeAuditDetails {
    fn of(action: &PurposeAuditAction) -> Json<Self> {
        Json(match action {
            PurposeAuditAction::Created { source, version } => Self {
                source: Some(source.to_string()),
                version: *version,
                hash: None,
            },
            PurposeAuditAction::DraftUpdated { draft_hash } => Self {
                source: None,
                version: None,
                hash: Some(draft_hash.as_str().to_owned()),
            },
            PurposeAuditAction::VersionSaved {
                version,
                content_hash,
            } => Self {
                source: None,
                version: Some(*version),
                hash: Some(content_hash.as_str().to_owned()),
            },
        })
    }
}

/// [`PurposeRepo`] on a Postgres pool.
#[derive(Debug, Clone)]
pub struct PgPurposeRepo {
    pool: PgPool,
}

/// One `purposes` row with its latest version, if any.
struct PurposeRow {
    id: Uuid,
    draft: Json<DefinitionRecord>,
    draft_hash: String,
    revision: i64,
    created_at: jiff_sqlx::Timestamp,
    updated_at: jiff_sqlx::Timestamp,
    latest_version: i32,
    latest: Option<Json<DefinitionRecord>>,
    latest_hash: Option<String>,
    latest_at: Option<jiff_sqlx::Timestamp>,
}

fn version_number(raw: i32) -> Result<u32, RepoError> {
    u32::try_from(raw).map_err(corrupt("purpose.version"))
}

impl TryFrom<PurposeRow> for Versioned<Purpose> {
    type Error = RepoError;

    fn try_from(row: PurposeRow) -> Result<Self, RepoError> {
        let id = PurposeId::from_uuid(row.id).map_err(corrupt("purpose.id"))?;
        let Json(draft) = row.draft;
        let draft = draft.into_definition(&row.draft_hash)?;
        let latest = match (row.latest, row.latest_hash, row.latest_at) {
            (Some(Json(definition)), Some(hash), Some(at)) => Some(PurposeVersion::restore(
                PurposeRef::new(id, version_number(row.latest_version)?)
                    .map_err(corrupt("purpose.latest_version"))?,
                definition.into_definition(&hash)?,
                at.to_jiff(),
            )),
            (None, None, None) if row.latest_version == 0 => None,
            _ => return Err(corrupt("purpose.latest_version")(())),
        };
        Ok(Self {
            value: Purpose::restore(
                id,
                draft,
                latest,
                row.created_at.to_jiff(),
                row.updated_at.to_jiff(),
            ),
            revision: Revision::new(u64::try_from(row.revision).map_err(corrupt("revision"))?),
        })
    }
}

impl PgPurposeRepo {
    /// The repository on `pool`. Production code gets one from `Database::purposes`.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn create_purpose(
        &self,
        purpose: &Purpose,
        event: &PurposeAuditEvent,
    ) -> Result<Revision, RepoError> {
        let mut tx = self.pool.begin().await.map_err(map_error)?;
        let operator = upsert_operator(&mut tx, event.actor()).await?;
        let draft = purpose.draft();
        let draft_hash = draft.content_hash();
        sqlx::query!(
            "INSERT INTO purposes (id, workspace_id, name, draft, draft_hash, latest_version,
                                   revision, created_at, created_by, updated_at)
             VALUES ($1, $2, $3, $4, $5, 0, 1, $6, $7, $8)",
            purpose.id().as_uuid(),
            DEFAULT_WORKSPACE,
            draft.name().as_str(),
            DefinitionRecord::of(draft) as _,
            draft_hash.as_str(),
            purpose.created_at().to_sqlx() as _,
            operator,
            purpose.updated_at().to_sqlx() as _,
        )
        .execute(&mut *tx)
        .await
        .map_err(map_error)?;
        insert_event(&mut tx, operator, event).await?;
        tx.commit().await.map_err(map_error)?;
        Ok(Revision::FIRST)
    }

    async fn get_purpose(&self, id: PurposeId) -> Result<Versioned<Purpose>, RepoError> {
        let row = sqlx::query_as!(
            PurposeRow,
            r#"SELECT p.id, p.draft AS "draft: _", p.draft_hash, p.revision,
                      p.created_at AS "created_at: jiff_sqlx::Timestamp",
                      p.updated_at AS "updated_at: jiff_sqlx::Timestamp",
                      p.latest_version,
                      v.definition AS "latest?: _", v.content_hash AS "latest_hash?",
                      v.created_at AS "latest_at?: jiff_sqlx::Timestamp"
               FROM purposes p
               LEFT JOIN purpose_versions v
                      ON v.purpose_id = p.id AND v.version = p.latest_version
               WHERE p.id = $1 AND p.workspace_id = $2"#,
            id.as_uuid(),
            DEFAULT_WORKSPACE,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_error)?
        .ok_or(RepoError::NotFound)?;
        row.try_into()
    }

    async fn list_purposes(&self, query: PurposeQuery) -> Result<Vec<PurposeListing>, RepoError> {
        let rows = sqlx::query!(
            r#"SELECT p.id, p.name, p.draft ->> 'description' AS "description!",
                      p.draft ->> 'compliance' AS "compliance!", p.latest_version,
                      (v.content_hash IS DISTINCT FROM p.draft_hash) AS "unsaved!"
               FROM purposes p
               LEFT JOIN purpose_versions v
                      ON v.purpose_id = p.id AND v.version = p.latest_version
               WHERE p.workspace_id = $1
               ORDER BY p.updated_at DESC, p.id DESC
               LIMIT $2"#,
            DEFAULT_WORKSPACE,
            i64::from(query.limit),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(PurposeListing {
                    id: PurposeId::from_uuid(row.id).map_err(corrupt("purpose.id"))?,
                    name: PurposeName::parse(&row.name).map_err(corrupt("purpose.name"))?,
                    description: row.description,
                    compliance: UseCase::parse(&row.compliance)
                        .map_err(corrupt("purpose.compliance"))?,
                    latest_version: match version_number(row.latest_version)? {
                        0 => None,
                        number => Some(number),
                    },
                    unsaved_changes: row.unsaved,
                })
            })
            .collect()
    }

    async fn update_purpose(
        &self,
        purpose: &Purpose,
        expected: Revision,
        event: &PurposeAuditEvent,
    ) -> Result<Revision, RepoError> {
        let mut tx = self.pool.begin().await.map_err(map_error)?;
        let operator = upsert_operator(&mut tx, event.actor()).await?;
        let draft = purpose.draft();
        let draft_hash = draft.content_hash();
        let revision = sqlx::query_scalar!(
            "UPDATE purposes
             SET name = $3, draft = $4, draft_hash = $5, revision = revision + 1, updated_at = $6
             WHERE id = $1 AND workspace_id = $7 AND revision = $2
             RETURNING revision",
            purpose.id().as_uuid(),
            to_i64(expected)?,
            draft.name().as_str(),
            DefinitionRecord::of(draft) as _,
            draft_hash.as_str(),
            purpose.updated_at().to_sqlx() as _,
            DEFAULT_WORKSPACE,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_error)?;
        let Some(revision) = revision else {
            return Err(missing_or_stale(&mut tx, purpose.id()).await);
        };
        insert_event(&mut tx, operator, event).await?;
        tx.commit().await.map_err(map_error)?;
        revision_of(revision)
    }

    async fn save_purpose_version(
        &self,
        purpose: &Purpose,
        expected: Revision,
        version: &PurposeVersion,
        event: &PurposeAuditEvent,
    ) -> Result<Revision, RepoError> {
        let mut tx = self.pool.begin().await.map_err(map_error)?;
        let operator = upsert_operator(&mut tx, event.actor()).await?;
        let number =
            i32::try_from(version.reference().version()).map_err(RepoError::unavailable)?;
        let revision = sqlx::query_scalar!(
            "UPDATE purposes
             SET latest_version = $3, revision = revision + 1, updated_at = $4
             WHERE id = $1 AND workspace_id = $5 AND revision = $2
             RETURNING revision",
            purpose.id().as_uuid(),
            to_i64(expected)?,
            number,
            purpose.updated_at().to_sqlx() as _,
            DEFAULT_WORKSPACE,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_error)?;
        let Some(revision) = revision else {
            return Err(missing_or_stale(&mut tx, purpose.id()).await);
        };
        let definition = version.definition();
        let content_hash = definition.content_hash();
        sqlx::query!(
            "INSERT INTO purpose_versions (purpose_id, version, definition, content_hash,
                                           created_at, created_by)
             VALUES ($1, $2, $3, $4, $5, $6)",
            purpose.id().as_uuid(),
            number,
            DefinitionRecord::of(definition) as _,
            content_hash.as_str(),
            version.created_at().to_sqlx() as _,
            operator,
        )
        .execute(&mut *tx)
        .await
        .map_err(map_error)?;
        insert_event(&mut tx, operator, event).await?;
        tx.commit().await.map_err(map_error)?;
        revision_of(revision)
    }

    async fn get_version(&self, reference: PurposeRef) -> Result<PurposeVersion, RepoError> {
        let number = i32::try_from(reference.version()).map_err(RepoError::unavailable)?;
        let row = sqlx::query!(
            r#"SELECT v.definition AS "definition: Json<DefinitionRecord>", v.content_hash,
                      v.created_at AS "created_at: jiff_sqlx::Timestamp"
               FROM purpose_versions v JOIN purposes p ON p.id = v.purpose_id
               WHERE v.purpose_id = $1 AND v.version = $2 AND p.workspace_id = $3"#,
            reference.id().as_uuid(),
            number,
            DEFAULT_WORKSPACE,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_error)?
        .ok_or(RepoError::NotFound)?;
        let Json(definition) = row.definition;
        Ok(PurposeVersion::restore(
            reference,
            definition.into_definition(&row.content_hash)?,
            row.created_at.to_jiff(),
        ))
    }
}

fn revision_of(raw: i64) -> Result<Revision, RepoError> {
    u64::try_from(raw)
        .map(Revision::new)
        .map_err(RepoError::unavailable)
}

/// Why a conditional update matched no row: the purpose is gone, or its revision moved on.
async fn missing_or_stale(tx: &mut Transaction<'_, Postgres>, id: PurposeId) -> RepoError {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM purposes WHERE id = $1 AND workspace_id = $2)
           AS "exists!""#,
        id.as_uuid(),
        DEFAULT_WORKSPACE,
    )
    .fetch_one(&mut **tx)
    .await;
    match exists {
        Ok(true) => RepoError::Conflict,
        Ok(false) => RepoError::NotFound,
        Err(error) => map_error(error),
    }
}

async fn insert_event(
    tx: &mut Transaction<'_, Postgres>,
    operator: Uuid,
    event: &PurposeAuditEvent,
) -> Result<(), RepoError> {
    sqlx::query!(
        "INSERT INTO audit_events (id, workspace_id, at, actor_id, actor, action, entity,
                                   entity_id, details)
         VALUES ($1, $2, $3, $4, $5, $6, 'purpose', $7, $8)",
        event.id().as_uuid(),
        DEFAULT_WORKSPACE,
        event.at().to_sqlx() as _,
        operator,
        event.actor().email(),
        event.action().name(),
        event.purpose_id().as_uuid(),
        PurposeAuditDetails::of(event.action()) as _,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_error)?;
    Ok(())
}

impl PurposeRepo for PgPurposeRepo {
    fn create<'a>(
        &'a self,
        purpose: &'a Purpose,
        event: &'a PurposeAuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(self.create_purpose(purpose, event))
    }

    fn get(&self, id: PurposeId) -> BoxFuture<'_, Result<Versioned<Purpose>, RepoError>> {
        Box::pin(self.get_purpose(id))
    }

    fn list(&self, query: PurposeQuery) -> BoxFuture<'_, Result<Vec<PurposeListing>, RepoError>> {
        Box::pin(self.list_purposes(query))
    }

    fn update_draft<'a>(
        &'a self,
        purpose: &'a Purpose,
        expected: Revision,
        event: &'a PurposeAuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(self.update_purpose(purpose, expected, event))
    }

    fn save_version<'a>(
        &'a self,
        purpose: &'a Purpose,
        expected: Revision,
        version: &'a PurposeVersion,
        event: &'a PurposeAuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(self.save_purpose_version(purpose, expected, version, event))
    }

    fn version(&self, reference: PurposeRef) -> BoxFuture<'_, Result<PurposeVersion, RepoError>> {
        Box::pin(self.get_version(reference))
    }
}
