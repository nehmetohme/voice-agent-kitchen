//! `AgentRepo` on Postgres. Every mutation is one transaction: the operator row, the agent,
//! its draft and the audit event commit together or not at all.

use jiff_sqlx::ToSqlx as _;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;
use vak_domain::{Actor, Agent, AgentId, AuditEvent};
use vak_repositories::{AgentQuery, AgentRepo, BoxFuture, RepoError, Revision, Versioned};

use crate::records::{AgentRow, AuditDetails, PersonaRecord, SettingsRecord};

/// The single MVP workspace, seeded by the first migration.
const DEFAULT_WORKSPACE: Uuid = Uuid::from_u128(0x01923a00_0000_7000_8000_000000000001);

/// Maps a driver error to the port's error: unique violation → `Conflict`, no row →
/// `NotFound`, anything else → `Unavailable` with the driver error as its source.
fn map_error(error: sqlx::Error) -> RepoError {
    match &error {
        sqlx::Error::RowNotFound => RepoError::NotFound,
        sqlx::Error::Database(database) if database.is_unique_violation() => RepoError::Conflict,
        _ => RepoError::unavailable(error),
    }
}

fn to_i64(revision: Revision) -> Result<i64, RepoError> {
    i64::try_from(revision.get()).map_err(RepoError::unavailable)
}

/// [`AgentRepo`] on a Postgres pool.
#[derive(Debug, Clone)]
pub struct PgAgentRepo {
    pool: PgPool,
}

impl PgAgentRepo {
    /// The repository on `pool`. Production code gets one from `Database::agents`.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn create_agent(&self, agent: &Agent, event: &AuditEvent) -> Result<Revision, RepoError> {
        let mut tx = self.pool.begin().await.map_err(map_error)?;
        let operator = upsert_operator(&mut tx, event.actor()).await?;
        let draft = agent.draft();
        let content_hash = draft.content_hash();
        sqlx::query!(
            "INSERT INTO agents (id, workspace_id, name, use_case, jurisdiction, revision,
                                 created_at, created_by, updated_at, archived_at)
             VALUES ($1, $2, $3, $4, $5, 1, $6, $7, $8, $9)",
            agent.id().as_uuid(),
            DEFAULT_WORKSPACE,
            draft.persona().name().as_str(),
            draft.persona().use_case().as_str(),
            draft.jurisdiction().as_str(),
            agent.created_at().to_sqlx() as _,
            operator,
            agent.updated_at().to_sqlx() as _,
            agent.archived_at().map(jiff_sqlx::Timestamp::from) as _,
        )
        .execute(&mut *tx)
        .await
        .map_err(map_error)?;
        sqlx::query!(
            "INSERT INTO agent_drafts (agent_id, persona, settings, rule_pack_id,
                                       rule_pack_version, content_hash, updated_at, updated_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            agent.id().as_uuid(),
            PersonaRecord::of(draft) as _,
            SettingsRecord::of(draft) as _,
            draft.rule_pack().id(),
            i32::try_from(draft.rule_pack().version()).map_err(RepoError::unavailable)?,
            content_hash.as_str(),
            agent.updated_at().to_sqlx() as _,
            operator,
        )
        .execute(&mut *tx)
        .await
        .map_err(map_error)?;
        insert_event(&mut tx, operator, event).await?;
        tx.commit().await.map_err(map_error)?;
        Ok(Revision::FIRST)
    }

    async fn get_agent(&self, id: AgentId) -> Result<Versioned<Agent>, RepoError> {
        let row = sqlx::query_as!(
            AgentRow,
            r#"SELECT a.id, a.name, a.use_case, a.jurisdiction, a.revision,
                      a.created_at AS "created_at: jiff_sqlx::Timestamp",
                      a.updated_at AS "updated_at: jiff_sqlx::Timestamp",
                      a.archived_at AS "archived_at: jiff_sqlx::Timestamp",
                      d.persona AS "persona: _", d.settings AS "settings: _",
                      d.rule_pack_id, d.rule_pack_version, d.content_hash
               FROM agents a JOIN agent_drafts d ON d.agent_id = a.id
               WHERE a.id = $1 AND a.workspace_id = $2"#,
            id.as_uuid(),
            DEFAULT_WORKSPACE,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_error)?
        .ok_or(RepoError::NotFound)?;
        row.try_into()
    }

    async fn list_agents(&self, query: AgentQuery) -> Result<Vec<Agent>, RepoError> {
        let rows = sqlx::query_as!(
            AgentRow,
            r#"SELECT a.id, a.name, a.use_case, a.jurisdiction, a.revision,
                      a.created_at AS "created_at: jiff_sqlx::Timestamp",
                      a.updated_at AS "updated_at: jiff_sqlx::Timestamp",
                      a.archived_at AS "archived_at: jiff_sqlx::Timestamp",
                      d.persona AS "persona: _", d.settings AS "settings: _",
                      d.rule_pack_id, d.rule_pack_version, d.content_hash
               FROM agents a JOIN agent_drafts d ON d.agent_id = a.id
               WHERE a.workspace_id = $1 AND a.archived_at IS NULL
               ORDER BY a.updated_at DESC, a.id DESC
               LIMIT $2"#,
            DEFAULT_WORKSPACE,
            i64::from(query.limit),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_error)?;
        rows.into_iter()
            .map(|row| Versioned::<Agent>::try_from(row).map(|stored| stored.value))
            .collect()
    }

    async fn update_agent(
        &self,
        agent: &Agent,
        expected: Revision,
        event: &AuditEvent,
    ) -> Result<Revision, RepoError> {
        let mut tx = self.pool.begin().await.map_err(map_error)?;
        let operator = upsert_operator(&mut tx, event.actor()).await?;
        let draft = agent.draft();
        let content_hash = draft.content_hash();
        let revision = sqlx::query_scalar!(
            "UPDATE agents
             SET name = $3, use_case = $4, jurisdiction = $5, revision = revision + 1,
                 updated_at = $6, archived_at = $7
             WHERE id = $1 AND workspace_id = $8 AND revision = $2
             RETURNING revision",
            agent.id().as_uuid(),
            to_i64(expected)?,
            draft.persona().name().as_str(),
            draft.persona().use_case().as_str(),
            draft.jurisdiction().as_str(),
            agent.updated_at().to_sqlx() as _,
            agent.archived_at().map(jiff_sqlx::Timestamp::from) as _,
            DEFAULT_WORKSPACE,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_error)?;
        let Some(revision) = revision else {
            // Asked on the transaction's own connection: a second pool acquire while this one
            // is held could exhaust the pool under concurrent stale writes. Dropping `tx`
            // afterwards rolls back the operator upsert too.
            return Err(missing_or_stale(&mut tx, agent.id()).await);
        };
        sqlx::query!(
            "UPDATE agent_drafts
             SET persona = $2, settings = $3, rule_pack_id = $4, rule_pack_version = $5,
                 content_hash = $6, updated_at = $7, updated_by = $8
             WHERE agent_id = $1",
            agent.id().as_uuid(),
            PersonaRecord::of(draft) as _,
            SettingsRecord::of(draft) as _,
            draft.rule_pack().id(),
            i32::try_from(draft.rule_pack().version()).map_err(RepoError::unavailable)?,
            content_hash.as_str(),
            agent.updated_at().to_sqlx() as _,
            operator,
        )
        .execute(&mut *tx)
        .await
        .map_err(map_error)?;
        insert_event(&mut tx, operator, event).await?;
        tx.commit().await.map_err(map_error)?;
        u64::try_from(revision)
            .map(Revision::new)
            .map_err(RepoError::unavailable)
    }
}

/// Why a conditional update matched no row: the agent is gone, or its revision moved on.
async fn missing_or_stale(tx: &mut Transaction<'_, Postgres>, id: AgentId) -> RepoError {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM agents WHERE id = $1 AND workspace_id = $2)
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

/// Creates the operator on first sight and returns their ID. The row is rewritten only when
/// the email changed or `last_seen_at` is a minute old, so autosaves don't write it each time.
async fn upsert_operator(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
) -> Result<Uuid, RepoError> {
    sqlx::query_scalar!(
        r#"WITH upserted AS (
               INSERT INTO operators (workspace_id, external_id, email)
               VALUES ($1, $2, $3)
               ON CONFLICT (workspace_id, external_id)
               DO UPDATE SET email = EXCLUDED.email, last_seen_at = now()
               WHERE operators.email IS DISTINCT FROM EXCLUDED.email
                  OR operators.last_seen_at < now() - interval '1 minute'
               RETURNING id
           )
           SELECT id AS "id!" FROM upserted
           UNION ALL
           SELECT id FROM operators WHERE workspace_id = $1 AND external_id = $2
           LIMIT 1"#,
        DEFAULT_WORKSPACE,
        actor.user_id(),
        actor.email(),
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(map_error)
}

async fn insert_event(
    tx: &mut Transaction<'_, Postgres>,
    operator: Uuid,
    event: &AuditEvent,
) -> Result<(), RepoError> {
    sqlx::query!(
        "INSERT INTO audit_events (id, workspace_id, at, actor_id, actor, action, entity,
                                   entity_id, details)
         VALUES ($1, $2, $3, $4, $5, $6, 'agent', $7, $8)",
        event.id().as_uuid(),
        DEFAULT_WORKSPACE,
        event.at().to_sqlx() as _,
        operator,
        event.actor().email(),
        event.action().name(),
        event.agent_id().as_uuid(),
        AuditDetails::of(event.action()) as _,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_error)?;
    Ok(())
}

impl AgentRepo for PgAgentRepo {
    fn create<'a>(
        &'a self,
        agent: &'a Agent,
        event: &'a AuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(self.create_agent(agent, event))
    }

    fn get(&self, id: AgentId) -> BoxFuture<'_, Result<Versioned<Agent>, RepoError>> {
        Box::pin(self.get_agent(id))
    }

    fn list(&self, query: AgentQuery) -> BoxFuture<'_, Result<Vec<Agent>, RepoError>> {
        Box::pin(self.list_agents(query))
    }

    fn update<'a>(
        &'a self,
        agent: &'a Agent,
        expected: Revision,
        event: &'a AuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(self.update_agent(agent, expected, event))
    }
}
