//! The `TestRunRepo` conformance suite. Every implementation, the in-memory fake and Postgres,
//! must pass it, so the fake services are tested against can't drift from production.

use std::time::Duration;

use jiff::{SignedDuration, Timestamp};
use uuid::Uuid;
use vak_domain::{
    Actor, Agent, AgentDraft, AgentId, AgentName, AuditAction, AuditEvent, AuditEventId,
    OptOutLayer, RubricCheck, RunModels, SessionEndReason, Speaker, TestRun, TestRunId,
    TestRunStatus, TranscriptLine, UseCase, Verdict, VoiceId,
};
use vak_repositories::{AgentRepo, RepoError, Revision, TestRunRepo};

/// Builds agents, runs and events with fixed, microsecond-precise timestamps.
struct Fixture {
    actor: Actor,
    start: Timestamp,
}

impl Fixture {
    #[expect(
        clippy::expect_used,
        reason = "fixed fixture values; a failure is a test bug"
    )]
    fn new() -> Self {
        Self {
            actor: Actor::new("4815162342", "ada@example.com").expect("valid actor"),
            start: Timestamp::from_microsecond(1_793_100_000_654_321).expect("valid time"),
        }
    }

    fn at(&self, minutes: i64) -> Timestamp {
        self.start + SignedDuration::from_mins(minutes)
    }

    fn event(&self, agent: AgentId, at: Timestamp, action: AuditAction) -> AuditEvent {
        AuditEvent::new(id(), at, self.actor.clone(), agent, action)
    }

    /// A stored survey agent (its default scenarios have rubrics to fill).
    #[expect(
        clippy::expect_used,
        reason = "a conformance suite fails the calling test"
    )]
    async fn agent<A: AgentRepo + ?Sized>(&self, agents: &A) -> Agent {
        let draft = AgentDraft::new(
            AgentName::parse("Survey desk").expect("valid name"),
            UseCase::Survey,
            VoiceId::parse("af_heart").expect("valid voice"),
        );
        let agent = Agent::create(id(), draft, self.at(0));
        let event = self.event(
            agent.id(),
            agent.created_at(),
            AuditAction::AgentCreated {
                draft_hash: agent.draft().content_hash(),
            },
        );
        agents.create(&agent, &event).await.expect("create agent");
        agent
    }

    /// A queued run of `agent`, stored, queued `minutes` after the start.
    #[expect(
        clippy::expect_used,
        reason = "a conformance suite fails the calling test"
    )]
    async fn queued<R: TestRunRepo + ?Sized>(
        &self,
        runs: &R,
        agent: &Agent,
        minutes: i64,
    ) -> TestRun {
        let run = TestRun::queue(id(), agent.id(), agent.draft(), self.at(minutes));
        let event = self.event(
            agent.id(),
            run.created_at(),
            AuditAction::TestRunStarted {
                run: run.id(),
                draft_hash: run.draft_hash().clone(),
            },
        );
        let revision = runs.create(&run, &event).await.expect("create run");
        assert_eq!(revision, Revision::FIRST, "a new run is at revision 1");
        run
    }
}

fn id<T: From<IdOf>>() -> T {
    T::from(IdOf(Uuid::now_v7()))
}

/// A fresh UUID v7, for any of the typed IDs.
struct IdOf(Uuid);

impl From<IdOf> for AgentId {
    #[expect(clippy::expect_used, reason = "UUID v7 always converts")]
    fn from(raw: IdOf) -> Self {
        Self::from_uuid(raw.0).expect("v7")
    }
}

impl From<IdOf> for TestRunId {
    #[expect(clippy::expect_used, reason = "UUID v7 always converts")]
    fn from(raw: IdOf) -> Self {
        Self::from_uuid(raw.0).expect("v7")
    }
}

impl From<IdOf> for AuditEventId {
    #[expect(clippy::expect_used, reason = "UUID v7 always converts")]
    fn from(raw: IdOf) -> Self {
        Self::from_uuid(raw.0).expect("v7")
    }
}

/// Every scenario of `run` played: transcripts, every kind of ending, answered and unanswered
/// checks; then finished `minutes` after the start.
#[expect(
    clippy::expect_used,
    reason = "a conformance suite fails the calling test"
)]
fn played(run: &TestRun, fx: &Fixture, minutes: i64, p: Option<f64>) -> TestRun {
    let endings = [
        SessionEndReason::ClientEnded,
        SessionEndReason::OptOut(OptOutLayer::Phrase),
        SessionEndReason::OptOut(OptOutLayer::Decision),
        SessionEndReason::OptOut(OptOutLayer::LlmTool),
        SessionEndReason::Timeout,
        SessionEndReason::Error,
    ];
    let mut run = run.clone();
    for (i, result) in run.scenarios().to_vec().into_iter().enumerate() {
        let transcript = vec![
            TranscriptLine::new(Speaker::Agent, "Hi, this is Survey desk."),
            TranscriptLine::new(
                Speaker::Caller,
                format!("Answer {i}, with ünïcode and \"quotes\"."),
            ),
        ];
        let checks = result
            .scenario()
            .rubric()
            .iter()
            .map(|item| RubricCheck::score(item, p))
            .collect();
        let end = endings
            .get(i % endings.len())
            .copied()
            .unwrap_or(SessionEndReason::Timeout);
        let done = result
            .running()
            .finished(end, transcript, checks, Duration::from_millis(1_234));
        run = run.record(done).expect("a running run records");
    }
    run.finish(fx.at(minutes)).expect("a running run finishes")
}

/// Runs every port rule against `runs`, which must start empty; `agents` stores the agents the
/// runs belong to.
///
/// # Panics
///
/// When `runs` breaks a rule of the `TestRunRepo` port; the message names the rule.
pub async fn test_run_repo_conformance<A, R>(agents: &A, runs: &R)
where
    A: AgentRepo + ?Sized,
    R: TestRunRepo + ?Sized,
{
    let fx = Fixture::new();
    reads_back_what_create_stored(agents, runs, &fx).await;
    an_agent_has_one_active_run(agents, runs, &fx).await;
    updates_need_the_stored_revision_and_keep_every_result(agents, runs, &fx).await;
    lists_newest_first_and_queues_oldest_first(agents, runs, &fx).await;
    latest_verdicts_are_per_agent_and_draft(agents, runs, &fx).await;
}

#[expect(
    clippy::expect_used,
    reason = "a conformance suite fails the calling test"
)]
async fn reads_back_what_create_stored<A, R>(agents: &A, runs: &R, fx: &Fixture)
where
    A: AgentRepo + ?Sized,
    R: TestRunRepo + ?Sized,
{
    let agent = fx.agent(agents).await;
    let run = fx.queued(runs, &agent, 1).await;
    let stored = runs.get(run.id()).await.expect("get");
    assert_eq!(
        stored.value, run,
        "get reads back exactly what create stored"
    );
    assert_eq!(stored.revision, Revision::FIRST);
    assert!(
        matches!(runs.get(id()).await, Err(RepoError::NotFound)),
        "an unknown run is NotFound"
    );
    let cancelled = run.cancel(fx.at(2)).expect("cancels");
    runs.update(&cancelled, Revision::FIRST, None)
        .await
        .expect("frees the agent's active slot");
}

#[expect(
    clippy::expect_used,
    reason = "a conformance suite fails the calling test"
)]
async fn an_agent_has_one_active_run<A, R>(agents: &A, runs: &R, fx: &Fixture)
where
    A: AgentRepo + ?Sized,
    R: TestRunRepo + ?Sized,
{
    let agent = fx.agent(agents).await;
    let other = fx.agent(agents).await;
    let first = fx.queued(runs, &agent, 10).await;
    let second = TestRun::queue(id(), agent.id(), agent.draft(), fx.at(11));
    let event = fx.event(
        agent.id(),
        fx.at(11),
        AuditAction::TestRunStarted {
            run: second.id(),
            draft_hash: second.draft_hash().clone(),
        },
    );
    assert!(
        matches!(runs.create(&second, &event).await, Err(RepoError::Conflict)),
        "a second active run of the same agent is a Conflict"
    );
    fx.queued(runs, &other, 11).await;
    // Once the first is done, the agent can run again.
    let interrupted = first.interrupt(fx.at(12)).expect("interrupts");
    runs.update(&interrupted, Revision::FIRST, None)
        .await
        .expect("update");
    fx.queued(runs, &agent, 13).await;
}

#[expect(
    clippy::expect_used,
    reason = "a conformance suite fails the calling test"
)]
async fn updates_need_the_stored_revision_and_keep_every_result<A, R>(
    agents: &A,
    runs: &R,
    fx: &Fixture,
) where
    A: AgentRepo + ?Sized,
    R: TestRunRepo + ?Sized,
{
    let agent = fx.agent(agents).await;
    let run = fx.queued(runs, &agent, 20).await;
    let running = run
        .start(
            agent.draft(),
            RunModels::new("KAT-Coder", "Kev-4B", 7),
            fx.at(21),
        )
        .expect("starts");
    let revision = runs
        .update(&running, Revision::FIRST, None)
        .await
        .expect("update");
    assert_eq!(
        revision,
        Revision::new(2),
        "an update moves the revision on"
    );
    assert!(
        matches!(
            runs.update(&running, Revision::FIRST, None).await,
            Err(RepoError::Conflict)
        ),
        "a stale revision is a Conflict"
    );
    let unknown = TestRun::queue(id(), agent.id(), agent.draft(), fx.at(21));
    assert!(
        matches!(
            runs.update(&unknown, Revision::FIRST, None).await,
            Err(RepoError::NotFound)
        ),
        "an unknown run is NotFound"
    );
    assert_eq!(
        runs.running()
            .await
            .expect("running")
            .first()
            .map(|stored| stored.value.id()),
        Some(running.id()),
        "running lists the running run"
    );

    for (minutes, p) in [(22, Some(0.75)), (23, None)] {
        let stored = runs.get(running.id()).await.expect("get");
        let finished = played(&running, fx, minutes, p);
        let finished = if p.is_some() {
            finished
        } else {
            // A cancel by the operator, with its audit event, is an update too.
            let cancelled = running.cancel(fx.at(minutes)).expect("cancels");
            let event = fx.event(
                agent.id(),
                fx.at(minutes),
                AuditAction::TestRunCancelled { run: running.id() },
            );
            runs.update(&cancelled, stored.revision, Some(&event))
                .await
                .expect("cancel");
            cancelled
        };
        if p.is_some() {
            runs.update(&finished, stored.revision, None)
                .await
                .expect("update");
        }
        let read = runs.get(running.id()).await.expect("get").value;
        assert_eq!(read, finished, "every result reads back exactly");
    }
    assert_eq!(
        runs.running().await.expect("running").len(),
        0,
        "a cancelled run isn't running"
    );
}

#[expect(
    clippy::expect_used,
    reason = "a conformance suite fails the calling test"
)]
async fn lists_newest_first_and_queues_oldest_first<A, R>(agents: &A, runs: &R, fx: &Fixture)
where
    A: AgentRepo + ?Sized,
    R: TestRunRepo + ?Sized,
{
    let agent = fx.agent(agents).await;
    let mut ids = Vec::new();
    for minutes in [30, 31, 32] {
        let run = fx.queued(runs, &agent, minutes).await;
        ids.push(run.id());
        if minutes < 32 {
            let cancelled = run.cancel(fx.at(minutes)).expect("cancels");
            runs.update(&cancelled, Revision::FIRST, None)
                .await
                .expect("update");
        }
    }
    let listed: Vec<TestRunId> = runs
        .list(agent.id(), 10)
        .await
        .expect("list")
        .iter()
        .map(TestRun::id)
        .collect();
    ids.reverse();
    assert_eq!(listed, ids, "an agent's runs, newest first");
    let limited = runs.list(agent.id(), 2).await.expect("list");
    assert_eq!(limited.len(), 2, "at most the limit");
    assert!(
        runs.list(id(), 10).await.expect("list").is_empty(),
        "no runs for an agent without any"
    );
    let oldest = runs
        .next_queued()
        .await
        .expect("next queued")
        .expect("a run is queued");
    assert_eq!(oldest.value.status(), TestRunStatus::Queued);
    assert!(
        oldest.value.created_at() <= fx.at(32),
        "the oldest queued run comes first"
    );
}

#[expect(
    clippy::expect_used,
    reason = "a conformance suite fails the calling test"
)]
async fn latest_verdicts_are_per_agent_and_draft<A, R>(agents: &A, runs: &R, fx: &Fixture)
where
    A: AgentRepo + ?Sized,
    R: TestRunRepo + ?Sized,
{
    // Drain whatever is still queued, so this rule starts from a known state.
    while let Some(stored) = runs.next_queued().await.expect("next queued") {
        let cancelled = stored.value.cancel(fx.at(40)).expect("cancels");
        runs.update(&cancelled, stored.revision, None)
            .await
            .expect("update");
    }
    let agent = fx.agent(agents).await;
    let other = fx.agent(agents).await;
    let models = || RunModels::new("KAT-Coder", "Kev-4B", 1);
    for (minutes, p) in [(41, Some(1.0)), (43, Some(0.0))] {
        let run = fx.queued(runs, &agent, minutes).await;
        let running = run
            .start(agent.draft(), models(), fx.at(minutes))
            .expect("starts");
        let finished = played(&running, fx, minutes + 1, p);
        runs.update(&finished, Revision::FIRST, None)
            .await
            .expect("update");
    }
    let run = fx.queued(runs, &other, 45).await;
    let running = run
        .start(other.draft(), models(), fx.at(45))
        .expect("starts");
    let passed = played(&running, fx, 46, Some(1.0));
    assert_eq!(passed.verdict(), Some(Verdict::Passed));
    runs.update(&passed, Revision::FIRST, None)
        .await
        .expect("update");
    // A newer run that never finished doesn't count.
    fx.queued(runs, &other, 47).await;

    let hash = agent.draft().content_hash();
    let mut verdicts = runs
        .latest_verdicts(&[
            (agent.id(), hash.clone()),
            (other.id(), other.draft().content_hash()),
            (id(), hash.clone()),
        ])
        .await
        .expect("latest verdicts");
    verdicts.sort_by_key(|(agent, _)| *agent);
    let mut expected = vec![(agent.id(), Verdict::Failed), (other.id(), Verdict::Passed)];
    expected.sort_by_key(|(agent, _)| *agent);
    assert_eq!(
        verdicts, expected,
        "the latest finished run per agent and draft hash"
    );
    let stale =
        vak_domain::ContentHash::parse(&format!("sha256:{}", "0".repeat(64))).expect("valid hash");
    assert!(
        runs.latest_verdicts(&[(agent.id(), stale)])
            .await
            .expect("latest verdicts")
            .is_empty(),
        "no verdict for a draft no run tested"
    );
    assert!(
        runs.latest_verdicts(&[])
            .await
            .expect("latest verdicts")
            .is_empty(),
        "nothing asked, nothing answered"
    );
}
