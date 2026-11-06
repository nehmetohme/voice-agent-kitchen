//! `PgTestRunRepo` against real Postgres: the `vak-testkit` conformance suite (the one the
//! in-memory fake passes), the audit events of an operator's changes, and rows the domain
//! rejects. `#[sqlx::test]` gives each test a fresh database with the migrations applied.
#![expect(
    clippy::expect_used,
    reason = "helpers outside #[test] functions fail the test on a database error"
)]

use sqlx::PgPool;
use uuid::Uuid;
use vak_domain::{
    Actor, Agent, AgentDraft, AgentId, AgentName, AuditAction, AuditEvent, AuditEventId, TestRun,
    TestRunId, UseCase, VoiceId,
};
use vak_repositories::{AgentRepo, RepoError, Revision, TestRunRepo};
use vak_repositories_pg::{PgAgentRepo, PgTestRunRepo};
use vak_testkit::repositories::test_run_repo_conformance;

fn actor() -> Actor {
    Actor::new("4815162342", "ada@example.com").expect("valid")
}

fn event(agent: AgentId, action: AuditAction) -> AuditEvent {
    let at = jiff::Timestamp::from_microsecond(1_793_000_000_000_000).expect("valid");
    let id = AuditEventId::from_uuid(Uuid::now_v7()).expect("v7");
    AuditEvent::new(id, at, actor(), agent, action)
}

async fn agent(pool: &PgPool) -> Agent {
    let draft = AgentDraft::new(
        AgentName::parse("Survey desk").expect("valid"),
        UseCase::Survey,
        VoiceId::parse("af_heart").expect("valid"),
    );
    let at = jiff::Timestamp::from_microsecond(1_793_000_000_000_000).expect("valid");
    let agent = Agent::create(AgentId::from_uuid(Uuid::now_v7()).expect("v7"), draft, at);
    let created = event(
        agent.id(),
        AuditAction::AgentCreated {
            draft_hash: agent.draft().content_hash(),
        },
    );
    PgAgentRepo::new(pool.clone())
        .create(&agent, &created)
        .await
        .expect("created");
    agent
}

async fn queued(pool: &PgPool, agent: &Agent) -> TestRun {
    let id = TestRunId::from_uuid(Uuid::now_v7()).expect("v7");
    let run = TestRun::queue(id, agent.id(), agent.draft(), agent.created_at());
    let started = event(
        agent.id(),
        AuditAction::TestRunStarted {
            run: id,
            draft_hash: run.draft_hash().clone(),
        },
    );
    PgTestRunRepo::new(pool.clone())
        .create(&run, &started)
        .await
        .expect("created");
    run
}

#[sqlx::test]
async fn pg_test_run_repo_conforms(pool: PgPool) {
    test_run_repo_conformance(&PgAgentRepo::new(pool.clone()), &PgTestRunRepo::new(pool)).await;
}

#[sqlx::test]
async fn starting_and_cancelling_are_audited_with_the_run(pool: PgPool) {
    let agent = agent(&pool).await;
    let run = queued(&pool, &agent).await;
    let cancelled = run.cancel(agent.created_at()).expect("cancels");
    PgTestRunRepo::new(pool.clone())
        .update(
            &cancelled,
            Revision::FIRST,
            Some(&event(
                agent.id(),
                AuditAction::TestRunCancelled { run: run.id() },
            )),
        )
        .await
        .expect("cancelled");

    let rows: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT action, entity, details ->> 'testRunId', details ->> 'draftHash'
         FROM audit_events WHERE entity_id = $1 ORDER BY at, id",
    )
    .bind(agent.id().as_uuid())
    .fetch_all(&pool)
    .await
    .expect("readable");
    let run_id = Some(run.id().to_string());
    let hash = Some(run.draft_hash().as_str().to_owned());
    assert_eq!(
        rows,
        [
            (
                "agent.created".to_owned(),
                "agent".to_owned(),
                None,
                hash.clone()
            ),
            (
                "test_run.started".to_owned(),
                "agent".to_owned(),
                run_id.clone(),
                hash
            ),
            (
                "test_run.cancelled".to_owned(),
                "agent".to_owned(),
                run_id,
                None
            ),
        ]
    );
}

#[sqlx::test]
async fn a_row_the_domain_rejects_is_unavailable_not_a_run(pool: PgPool) {
    let repo = PgTestRunRepo::new(pool.clone());
    let scenario = queued(&pool, &agent(&pool).await).await;
    let verdict = queued(&pool, &agent(&pool).await).await;
    sqlx::query(
        "UPDATE test_runs SET scenarios = jsonb_set(scenarios, '{0,scenario,maxTurns}', '0')
         WHERE id = $1",
    )
    .bind(scenario.id().as_uuid())
    .execute(&pool)
    .await
    .expect("updated");
    // A finished status without a verdict breaks a CHECK; a verdict the results don't give is
    // caught on read.
    sqlx::query(
        "UPDATE test_runs SET status = 'finished', verdict = 'passed',
                              scenarios = jsonb_set(scenarios, '{0,status}', '\"failed\"')
         WHERE id = $1",
    )
    .bind(verdict.id().as_uuid())
    .execute(&pool)
    .await
    .expect("updated");

    for run in [&scenario, &verdict] {
        let error = repo.get(run.id()).await.expect_err("corrupt");
        assert!(matches!(error, RepoError::Unavailable(_)), "{error:?}");
    }
}

#[sqlx::test]
async fn the_database_keeps_one_active_run_per_agent(pool: PgPool) {
    let agent = agent(&pool).await;
    queued(&pool, &agent).await;
    let second = sqlx::query(
        "INSERT INTO test_runs (id, workspace_id, agent_id, created_by, draft_hash, status,
                                scenarios, created_at)
         SELECT $1, workspace_id, agent_id, created_by, draft_hash, 'running', '[]', created_at
         FROM test_runs LIMIT 1",
    )
    .bind(Uuid::now_v7())
    .execute(&pool)
    .await
    .expect_err("a second active run");
    assert!(
        second
            .as_database_error()
            .is_some_and(sqlx::error::DatabaseError::is_unique_violation),
        "{second}"
    );
}
