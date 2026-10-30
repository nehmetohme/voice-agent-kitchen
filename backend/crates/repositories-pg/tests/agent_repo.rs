//! `PgAgentRepo` against real Postgres. `#[sqlx::test]` gives each test a fresh database with
//! the migrations applied; `DATABASE_URL` comes from the nextest setup script
//! (`backend/tools/test-db.sh`, started on demand in Docker).
#![expect(
    clippy::expect_used,
    reason = "helpers outside #[test] functions fail the test on a database error"
)]

use secrecy::SecretString;
use sqlx::PgPool;
use sqlx::postgres::PgConnectOptions;
use uuid::Uuid;
use vak_domain::{
    Actor, Agent, AgentDraft, AgentId, AgentName, AuditAction, AuditEvent, AuditEventId, UseCase,
    VoiceId,
};
use vak_repositories::{AgentQuery, AgentRepo, RepoError};
use vak_repositories_pg::{Database, DatabaseConfig, DatabaseError, PgAgentRepo};
use vak_testkit::repositories::{AuditRow, agent_repo_conformance};

async fn audit_trail(pool: &PgPool, id: AgentId) -> Vec<AuditRow> {
    let rows: Vec<(String, String, String, Uuid)> = sqlx::query_as(
        "SELECT e.action, o.external_id, e.actor, e.entity_id
         FROM audit_events e JOIN operators o ON o.id = e.actor_id
         WHERE e.entity = 'agent' AND e.entity_id = $1
         ORDER BY e.at, e.id",
    )
    .bind(id.as_uuid())
    .fetch_all(pool)
    .await
    .expect("audit_events is readable");
    rows.into_iter()
        .map(|(action, actor_user_id, actor_email, agent_id)| AuditRow {
            action,
            actor_user_id,
            actor_email,
            agent_id: AgentId::from_uuid(agent_id).expect("v7"),
        })
        .collect()
}

/// A created agent and its stored `agent.created` event, by Ada.
async fn created(repo: &PgAgentRepo) -> Agent {
    created_by(repo, "ada@example.com").await
}

/// A created agent, by the operator `4815162342` signed in as `email`.
async fn created_by(repo: &PgAgentRepo, email: &str) -> Agent {
    let draft = AgentDraft::new(
        AgentName::parse("Front desk").expect("valid"),
        UseCase::Support,
        VoiceId::parse("af_heart").expect("valid"),
    );
    let at = jiff::Timestamp::from_microsecond(1_793_000_000_000_000).expect("valid");
    let agent = Agent::create(AgentId::from_uuid(Uuid::now_v7()).expect("v7"), draft, at);
    let event = AuditEvent::new(
        AuditEventId::from_uuid(Uuid::now_v7()).expect("v7"),
        at,
        Actor::new("4815162342", email).expect("valid"),
        agent.id(),
        AuditAction::AgentCreated {
            draft_hash: agent.draft().content_hash(),
        },
    );
    repo.create(&agent, &event).await.expect("created");
    agent
}

#[sqlx::test]
async fn pg_agent_repo_conforms(pool: PgPool) {
    let repo = PgAgentRepo::new(pool.clone());

    agent_repo_conformance(&repo, |id| {
        let pool = pool.clone();
        async move { audit_trail(&pool, id).await }
    })
    .await;
}

#[sqlx::test]
async fn audit_events_are_append_only(pool: PgPool) {
    let repo = PgAgentRepo::new(pool.clone());
    let agent = created(&repo).await;

    for statement in [
        "UPDATE audit_events SET action = 'agent.rewritten'",
        "DELETE FROM audit_events",
        "TRUNCATE audit_events",
    ] {
        let error = sqlx::query(statement)
            .execute(&pool)
            .await
            .expect_err(statement);
        let code = error
            .as_database_error()
            .and_then(|error| error.code().map(std::borrow::Cow::into_owned));
        assert_eq!(code.as_deref(), Some("42501"), "{statement}: {error}");
    }
    assert_eq!(audit_trail(&pool, agent.id()).await.len(), 1);
}

#[sqlx::test]
async fn the_operator_is_recorded_once_and_keeps_their_latest_email(pool: PgPool) {
    let repo = PgAgentRepo::new(pool.clone());
    let operators = || async {
        sqlx::query_as::<_, (String, String)>("SELECT external_id, email FROM operators")
            .fetch_all(&pool)
            .await
            .expect("readable")
    };
    created(&repo).await;
    created(&repo).await;

    assert_eq!(
        operators().await,
        [("4815162342".to_owned(), "ada@example.com".to_owned())]
    );

    let renamed = created_by(&repo, "ada.lovelace@example.com").await;

    assert_eq!(
        operators().await,
        [(
            "4815162342".to_owned(),
            "ada.lovelace@example.com".to_owned()
        )]
    );
    assert_eq!(audit_trail(&pool, renamed.id()).await.len(), 1);
}

#[sqlx::test]
async fn a_row_the_domain_rejects_is_unavailable_not_an_agent(pool: PgPool) {
    let repo = PgAgentRepo::new(pool.clone());
    let voice = created(&repo).await;
    let hash = created(&repo).await;
    sqlx::query("UPDATE agent_drafts SET persona = '{\"voice\":\"robot\",\"language\":\"en-US\"}' WHERE agent_id = $1")
        .bind(voice.id().as_uuid())
        .execute(&pool)
        .await
        .expect("updated");
    sqlx::query("UPDATE agents SET name = 'Edited behind our back' WHERE id = $1")
        .bind(hash.id().as_uuid())
        .execute(&pool)
        .await
        .expect("updated");

    for agent in [&voice, &hash] {
        let error = repo.get(agent.id()).await.expect_err("corrupt");
        assert!(matches!(error, RepoError::Unavailable(_)), "{error:?}");
    }
    assert!(matches!(
        repo.list(AgentQuery { limit: 10 }).await,
        Err(RepoError::Unavailable(_))
    ));
}

fn url_of(options: &PgConnectOptions) -> SecretString {
    format!(
        "postgres://{}@{}:{}/{}",
        options.get_username(),
        options.get_host(),
        options.get_port(),
        options.get_database().unwrap_or("postgres"),
    )
    .into()
}

#[sqlx::test(migrations = false)]
async fn migrations_apply_to_an_empty_database_and_rerun_safely(
    _pool: sqlx::postgres::PgPoolOptions,
    options: PgConnectOptions,
) {
    let database = Database::connect_lazy(&DatabaseConfig::new(url_of(&options))).expect("valid");

    database
        .migrate()
        .await
        .expect("first run applies every migration");
    database
        .migrate()
        .await
        .expect("a second run changes nothing");

    let agents = database
        .agents()
        .list(AgentQuery { limit: 10 })
        .await
        .expect("the schema serves the repository");
    assert_eq!(agents, []);
}

#[test]
fn a_malformed_url_is_rejected_without_echoing_it() {
    let secret = "postgres://user:hunter2@[not-a-host";

    let error = Database::connect_lazy(&DatabaseConfig::new(secret.into())).expect_err("malformed");

    assert!(matches!(error, DatabaseError::InvalidUrl(_)));
    assert!(!error.to_string().contains("hunter2"), "{error}");
}
