//! `PgPurposeRepo` against real Postgres, and the purposes migration on an existing database.
//! `#[sqlx::test]` gives each test a fresh database; `DATABASE_URL` comes from the nextest
//! setup script (`backend/tools/test-db.sh`).
#![expect(
    clippy::expect_used,
    reason = "helpers outside #[test] functions fail the test on a database error"
)]

use std::path::PathBuf;

use sqlx::PgPool;
use sqlx::migrate::Migrator;
use uuid::Uuid;
use vak_domain::{
    Actor, AgentDraft, AgentId, AgentName, AuditEventId, Purpose, PurposeAuditAction,
    PurposeAuditEvent, PurposeId, PurposeName, PurposeRef, PurposeVersion, ToolHosts, UseCase,
    VoiceId,
};
use vak_repositories::{AgentRepo, PurposeRepo, Revision};
use vak_repositories_pg::{PgAgentRepo, PgPurposeRepo};
use vak_testkit::repositories::purpose_repo_conformance;

/// The migration that adds purposes; the ones before it make the M5 schema.
const PURPOSES_MIGRATION: i64 = 20_261_108_120_000;

#[sqlx::test]
async fn pg_purpose_repo_conforms(pool: PgPool) {
    purpose_repo_conformance(&PgPurposeRepo::new(pool)).await;
}

fn event(purpose: &Purpose, action: PurposeAuditAction) -> PurposeAuditEvent {
    PurposeAuditEvent::new(
        AuditEventId::from_uuid(Uuid::now_v7()).expect("v7"),
        purpose.updated_at(),
        Actor::new("4815162342", "ada@example.com").expect("valid"),
        purpose.id(),
        action,
    )
}

/// A purpose with version 1 stored.
async fn versioned(repo: &PgPurposeRepo) -> (Purpose, PurposeVersion) {
    let at = jiff::Timestamp::from_microsecond(1_793_000_000_000_000).expect("valid");
    let purpose = Purpose::duplicate(
        PurposeId::from_uuid(Uuid::now_v7()).expect("v7"),
        PurposeVersion::system(UseCase::Support).definition(),
        PurposeName::parse("Help desk").expect("valid"),
        at,
    );
    let created = event(
        &purpose,
        PurposeAuditAction::Created {
            source: PurposeId::system(UseCase::Support),
            version: Some(1),
        },
    );
    repo.create(&purpose, &created).await.expect("created");
    let (saved, version) = purpose
        .save_version(&ToolHosts::default(), at)
        .expect("version 1");
    let saved_event = event(
        &saved,
        PurposeAuditAction::VersionSaved {
            version: 1,
            content_hash: version.definition().content_hash(),
        },
    );
    repo.save_version(&saved, Revision::FIRST, &version, &saved_event)
        .await
        .expect("saved");
    (saved, version)
}

#[sqlx::test]
async fn a_saved_version_can_never_change_or_go(pool: PgPool) {
    let repo = PgPurposeRepo::new(pool.clone());
    let (purpose, _) = versioned(&repo).await;

    for statement in [
        "UPDATE purpose_versions SET content_hash = content_hash WHERE purpose_id = $1",
        "DELETE FROM purpose_versions WHERE purpose_id = $1",
    ] {
        let error = sqlx::query(statement)
            .bind(purpose.id().as_uuid())
            .execute(&pool)
            .await
            .expect_err("purpose_versions is append-only");
        let code = error
            .as_database_error()
            .and_then(sqlx::error::DatabaseError::code)
            .map(std::borrow::Cow::into_owned);
        assert_eq!(code.as_deref(), Some("42501"), "{statement}");
    }
    let truncate = sqlx::query("TRUNCATE purpose_versions CASCADE")
        .execute(&pool)
        .await;
    assert!(truncate.is_err(), "TRUNCATE is refused too");
}

#[sqlx::test]
async fn purpose_events_are_audited_with_their_details(pool: PgPool) {
    let repo = PgPurposeRepo::new(pool.clone());
    let (purpose, _) = versioned(&repo).await;

    let rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT action, details ->> 'source', details ->> 'version' FROM audit_events
         WHERE entity = 'purpose' AND entity_id = $1 ORDER BY at, id",
    )
    .bind(purpose.id().as_uuid())
    .fetch_all(&pool)
    .await
    .expect("audit_events is readable");

    let source = PurposeId::system(UseCase::Support).to_string();
    assert_eq!(
        rows,
        [
            (
                "purpose.created".to_owned(),
                Some(source),
                Some("1".to_owned())
            ),
            (
                "purpose.version_saved".to_owned(),
                None,
                Some("1".to_owned())
            ),
        ]
    );
}

#[sqlx::test]
async fn a_draft_that_breaks_a_rule_is_refused_on_read(pool: PgPool) {
    let repo = PgPurposeRepo::new(pool.clone());
    let (purpose, _) = versioned(&repo).await;

    sqlx::query("UPDATE purposes SET draft = jsonb_set(draft, '{name}', '\"\"') WHERE id = $1")
        .bind(purpose.id().as_uuid())
        .execute(&pool)
        .await
        .expect("updated outside the repository");

    assert!(
        repo.get(purpose.id()).await.is_err(),
        "the domain refuses it"
    );
    let reference = PurposeRef::new(purpose.id(), 1).expect("valid");
    assert!(
        repo.version(reference).await.is_ok(),
        "the version is untouched"
    );
}

/// The migrations before purposes, applied from a copy of their files.
async fn migrate_before_purposes(pool: &PgPool) {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let target = std::env::temp_dir().join(format!("vak-migrations-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&target).expect("temp dir");
    for entry in std::fs::read_dir(&source).expect("migrations dir") {
        let path = entry.expect("entry").path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let version: i64 = name
            .split('_')
            .next()
            .and_then(|digits| digits.parse().ok())
            .unwrap_or(i64::MAX);
        if version < PURPOSES_MIGRATION {
            std::fs::copy(&path, target.join(name)).expect("copy");
        }
    }
    Migrator::new(target.as_path())
        .await
        .expect("migrations load")
        .run(pool)
        .await
        .expect("the M5 schema");
    std::fs::remove_dir_all(&target).expect("clean up");
}

#[sqlx::test(migrations = false)]
async fn existing_agents_move_to_their_use_cases_system_purpose_unchanged(pool: PgPool) {
    migrate_before_purposes(&pool).await;
    let draft = AgentDraft::new(
        AgentName::parse("Front desk").expect("valid"),
        UseCase::Booking,
        VoiceId::parse("af_heart").expect("valid"),
    );
    let id = AgentId::from_uuid(Uuid::now_v7()).expect("v7");
    let operator: Uuid = sqlx::query_scalar(
        "INSERT INTO operators (workspace_id, external_id, email)
         VALUES ('01923a00-0000-7000-8000-000000000001', '4815162342', 'ada@example.com')
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("operator");
    sqlx::query(
        "INSERT INTO agents (id, workspace_id, name, use_case, jurisdiction, created_at,
                             created_by, updated_at)
         VALUES ($1, '01923a00-0000-7000-8000-000000000001', 'Front desk', 'booking', 'US',
                 now(), $2, now())",
    )
    .bind(id.as_uuid())
    .bind(operator)
    .execute(&pool)
    .await
    .expect("agent");
    sqlx::query(
        "INSERT INTO agent_drafts (agent_id, persona, settings, rule_pack_id, rule_pack_version,
                                   content_hash, updated_at, updated_by)
         VALUES ($1, '{\"voice\":\"af_heart\",\"language\":\"en-US\"}',
                 '{\"model\":\"balanced\",\"turnDetection\":{\"silenceMs\":500,\"bargeIn\":true}}',
                 'us-booking', 1, $2, now(), $3)",
    )
    .bind(id.as_uuid())
    .bind(draft.content_hash().as_str())
    .bind(operator)
    .execute(&pool)
    .await
    .expect("draft");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("the purposes migration applies");

    let agent = PgAgentRepo::new(pool)
        .get(id)
        .await
        .expect("the agent reads back");
    assert_eq!(
        agent.value.draft().purpose(),
        &PurposeRef::system(UseCase::Booking),
        "pinned to its use case's system purpose, version 1"
    );
    assert_eq!(
        agent.value.draft().content_hash(),
        draft.content_hash(),
        "with the hash it had"
    );
}
