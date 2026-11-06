//! Test run edge cases beyond the `test-runs` v1 contract: storage failures, runs that can't run
//! when the worker claims them, and the background loop.

use std::sync::Arc;
use std::time::Duration;

use vak_domain::{AgentName, TestRunStatus, UseCase, VoiceId};
use vak_testkit::providers::{LlmStep, ScriptedDecisions, ScriptedLlm};
use vak_testkit::repositories::{InMemoryAgentRepo, InMemoryTestRunRepo};

use super::*;
use crate::CreateAgent;

const QUICK: Duration = Duration::from_millis(10);

fn actor() -> Actor {
    Actor::new("7364019283746501", "ada@example.com").expect("valid")
}

struct Fixture {
    agents: AgentService,
    agents_repo: Arc<InMemoryAgentRepo>,
    runs_repo: Arc<InMemoryTestRunRepo>,
    llm: Arc<ScriptedLlm>,
    service: TestRunService,
}

fn fixture() -> Fixture {
    let agents_repo = Arc::new(InMemoryAgentRepo::default());
    let runs_repo = Arc::new(InMemoryTestRunRepo::default());
    let agents = AgentService::new(agents_repo.clone()).with_test_runs(runs_repo.clone());
    let llm = Arc::new(ScriptedLlm::new());
    let engines = TestEngines::new(
        llm.clone(),
        "fake-llm",
        Arc::new(ScriptedDecisions::new()),
        "fake-decisions",
    );
    let service = TestRunService::new(agents.clone(), runs_repo.clone(), Some(engines));
    Fixture {
        agents,
        agents_repo,
        runs_repo,
        llm,
        service,
    }
}

impl Fixture {
    async fn agent(&self) -> AgentId {
        let command = CreateAgent {
            name: AgentName::parse("Front desk").expect("valid"),
            use_case: UseCase::Booking,
            voice: VoiceId::parse("af_heart").expect("valid"),
            language: None,
            jurisdiction: None,
            model: None,
            turn_detection: None,
        };
        self.agents
            .create(&actor(), command)
            .await
            .expect("created")
            .id()
    }

    /// Every caller of the next run hangs up on its first turn.
    fn callers_hang_up(&self, scenarios: usize) {
        for _ in 0..scenarios {
            self.llm.push_reply([
                LlmStep::text("Bye.", QUICK),
                LlmStep::tool_call("end_call", "{}", QUICK),
            ]);
        }
    }
}

#[tokio::test(start_paused = true)]
async fn storage_failures_are_unavailable() {
    let fx = fixture();
    let agent = fx.agent().await;
    fx.runs_repo.set_unavailable(true);
    assert!(matches!(
        fx.service.start(&actor(), agent).await,
        Err(StartTestRunError::Unavailable(_))
    ));
    let id = TestRunId::from_uuid(Uuid::now_v7()).expect("v7");
    assert!(matches!(
        fx.service.get(id).await,
        Err(FindTestRunError::Unavailable(_))
    ));
    assert!(matches!(
        fx.service.cancel(&actor(), id).await,
        Err(CancelTestRunError::Unavailable(_))
    ));
    assert!(matches!(
        fx.service.list(agent).await,
        Err(FindAgentError::Unavailable(_))
    ));
    assert!(matches!(
        fx.agents.progress(agent).await,
        Err(FindAgentError::Unavailable(_))
    ));
    assert_eq!(fx.service.worker().drain().await, 0, "the worker stops");
    assert_eq!(fx.service.worker().recover().await, 0);

    fx.runs_repo.set_unavailable(false);
    fx.agents_repo.set_unavailable(true);
    assert!(matches!(
        fx.service.start(&actor(), agent).await,
        Err(StartTestRunError::Unavailable(_))
    ));
    let unknown = AgentId::from_uuid(Uuid::now_v7()).expect("v7");
    fx.agents_repo.set_unavailable(false);
    assert!(matches!(
        fx.service.list(unknown).await,
        Err(FindAgentError::NotFound)
    ));
}

#[tokio::test(start_paused = true)]
async fn a_run_that_cannot_run_when_claimed_is_interrupted() {
    let fx = fixture();
    let agent = fx.agent().await;
    let queued = fx.service.start(&actor(), agent).await.expect("queued");
    fx.agents.archive(&actor(), agent).await.expect("archived");

    assert_eq!(fx.service.worker().drain().await, 0);
    let run = fx.service.get(queued.id()).await.expect("found");
    assert_eq!(
        run.status(),
        TestRunStatus::Interrupted,
        "its agent was archived"
    );

    // A server without models interrupts what was queued before.
    let other = fx.agent().await;
    let queued = fx.service.start(&actor(), other).await.expect("queued");
    let offline = TestRunService::new(fx.agents.clone(), fx.runs_repo.clone(), None);
    assert_eq!(offline.worker().drain().await, 0);
    let run = offline.get(queued.id()).await.expect("found");
    assert_eq!(run.status(), TestRunStatus::Interrupted);
}

#[tokio::test(start_paused = true)]
async fn the_worker_runs_what_is_queued_until_shutdown() {
    let fx = fixture();
    let agent = fx.agent().await;
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let worker = tokio::spawn(fx.service.worker().run(async {
        let _ = stopped.await;
    }));
    fx.callers_hang_up(3);
    let queued = fx.service.start(&actor(), agent).await.expect("queued");

    let mut status = TestRunStatus::Queued;
    for _ in 0..100 {
        status = fx.service.get(queued.id()).await.expect("found").status();
        if status == TestRunStatus::Finished {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        status,
        TestRunStatus::Finished,
        "the worker woke up and ran it"
    );

    stop.send(()).expect("the worker is running");
    worker.await.expect("the worker stops");
}
