//! Test runs: an agent's scenarios played by simulated callers in text, scored by their
//! rubrics through the decision model, with one verdict.
//!
//! A run is a job. [`TestRunService::start`] stores it queued, and the [`TestRunWorker`] that
//! `vak-server` runs in the background claims queued runs one at a time, plays their scenarios
//! (one at a time by default: a local LLM serves one request at a time) and stores every step,
//! so `get` reports progress while it runs. [`TestRunService::cancel`] stops a run at once: the
//! worker drops the model calls in flight.
//!
//! Each scenario is a text conversation (`vak_pipeline::run_text_session`) between the agent's
//! reply logic and an LLM playing the scenario's caller, then one decision request with every
//! rubric question over the transcript. Every LLM request samples greedily with the settings'
//! seed, and the run records the models and every scenario's version, so a run on the same
//! draft with deterministic models always gives the same verdict.
//!
//! Transcripts are stored with the run, never logged.

mod worker;

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use vak_domain::{
    Actor, AgentId, AuditAction, AuditEvent, AuditEventId, ContentHash, TestRun, TestRunId,
    TestRunStatus,
};
use vak_providers::{DecisionEngine, LlmEngine};
use vak_repositories::{RepoError, TestRunRepo};

pub use self::worker::TestRunWorker;
use crate::FindAgentError;
use crate::agents::{AgentService, now};

/// The most runs [`TestRunService::list`] returns.
pub const RUNS_LISTED: u16 = 20;
/// How long one repository call may take.
const REPO_DEADLINE: Duration = Duration::from_secs(5);
/// How often a cancel is retried when the worker saved in between.
const CANCEL_ATTEMPTS: u32 = 3;

/// The models a test run talks to, with the IDs it records.
#[derive(Clone)]
pub struct TestEngines {
    llm: Arc<dyn LlmEngine>,
    llm_model: String,
    decisions: Arc<dyn DecisionEngine>,
    decision_model: String,
}

impl TestEngines {
    /// `llm` plays both the agent and the simulated caller; `decisions` answers the opt-out
    /// questions and scores the rubrics. The model names are recorded on every run.
    #[must_use]
    pub fn new(
        llm: Arc<dyn LlmEngine>,
        llm_model: impl Into<String>,
        decisions: Arc<dyn DecisionEngine>,
        decision_model: impl Into<String>,
    ) -> Self {
        Self {
            llm,
            llm_model: llm_model.into(),
            decisions,
            decision_model: decision_model.into(),
        }
    }
}

/// How test runs are run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestRunSettings {
    /// Scenarios at once (default 1: a local LLM serves one request at a time, and the
    /// simulated caller and the agent share it).
    pub scenario_concurrency: usize,
    /// The seed of every LLM request's greedy sampling (temperature 0).
    pub seed: u64,
    /// An LLM request's time to first token, the agent's or the caller's.
    pub llm_first_token: Duration,
    /// A whole LLM reply.
    pub llm_reply: Duration,
    /// The opt-out decision about a caller line.
    pub decision: Duration,
    /// Scoring a scenario's rubric over its transcript.
    pub rubric: Duration,
}

impl Default for TestRunSettings {
    /// Generous for a local model's first call: a run isn't real time.
    fn default() -> Self {
        Self {
            scenario_concurrency: 1,
            seed: 1,
            llm_first_token: Duration::from_secs(20),
            llm_reply: Duration::from_secs(30),
            decision: Duration::from_secs(5),
            rubric: Duration::from_secs(20),
        }
    }
}

/// An agent's runs, newest first, and the hash of its current draft: a run on another hash
/// is stale.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentTestRuns {
    pub draft_hash: ContentHash,
    pub runs: Vec<TestRun>,
}

/// Why [`TestRunService::start`] failed.
#[derive(Debug, thiserror::Error)]
pub enum StartTestRunError {
    #[error("no such agent")]
    NotFound,
    #[error("the agent is archived")]
    Archived,
    #[error("test runs need the models, which are not configured on this server")]
    ModelsUnavailable,
    #[error("a test run of this agent is already queued or running")]
    AlreadyRunning,
    #[error("storage is unavailable")]
    Unavailable(#[source] RepoError),
}

/// Why [`TestRunService::get`] failed.
#[derive(Debug, thiserror::Error)]
pub enum FindTestRunError {
    #[error("no such test run")]
    NotFound,
    #[error("storage is unavailable")]
    Unavailable(#[source] RepoError),
}

/// Why [`TestRunService::cancel`] failed.
#[derive(Debug, thiserror::Error)]
pub enum CancelTestRunError {
    #[error("no such test run")]
    NotFound,
    #[error("the test run already finished")]
    Finished,
    #[error("the test run kept changing while it was being cancelled")]
    Conflict,
    #[error("storage is unavailable")]
    Unavailable(#[source] RepoError),
}

impl From<RepoError> for FindTestRunError {
    fn from(error: RepoError) -> Self {
        match error {
            RepoError::NotFound => Self::NotFound,
            other => Self::Unavailable(other),
        }
    }
}

impl From<FindTestRunError> for CancelTestRunError {
    fn from(error: FindTestRunError) -> Self {
        match error {
            FindTestRunError::NotFound => Self::NotFound,
            FindTestRunError::Unavailable(source) => Self::Unavailable(source),
        }
    }
}

struct Inner {
    agents: AgentService,
    runs: Arc<dyn TestRunRepo>,
    engines: Option<TestEngines>,
    settings: TestRunSettings,
    /// The worker's cancel signal per running run, in this process.
    cancels: Mutex<HashMap<TestRunId, CancellationToken>>,
    /// Wakes the worker when a run is queued.
    queued: Notify,
}

/// The test run use cases. Cheap to clone; the clones share the worker's signals.
#[derive(Clone)]
pub struct TestRunService {
    inner: Arc<Inner>,
}

impl TestRunService {
    /// `engines` is `None` when this server has no models for test runs: starting one is then
    /// refused with [`StartTestRunError::ModelsUnavailable`].
    #[must_use]
    pub fn new(
        agents: AgentService,
        runs: Arc<dyn TestRunRepo>,
        engines: Option<TestEngines>,
    ) -> Self {
        Self::build(agents, runs, engines, TestRunSettings::default())
    }

    /// The same service with these settings.
    #[must_use]
    pub fn with_settings(self, settings: TestRunSettings) -> Self {
        Self::build(
            self.inner.agents.clone(),
            Arc::clone(&self.inner.runs),
            self.inner.engines.clone(),
            settings,
        )
    }

    fn build(
        agents: AgentService,
        runs: Arc<dyn TestRunRepo>,
        engines: Option<TestEngines>,
        settings: TestRunSettings,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                agents,
                runs,
                engines,
                settings,
                cancels: Mutex::new(HashMap::new()),
                queued: Notify::new(),
            }),
        }
    }

    /// Queues a run of the agent's current draft and its scenarios, started by `actor`
    /// (audited as `test_run.started`). An agent has one run queued or running at a time.
    pub async fn start(&self, actor: &Actor, agent: AgentId) -> Result<TestRun, StartTestRunError> {
        if self.inner.engines.is_none() {
            return Err(StartTestRunError::ModelsUnavailable);
        }
        let agent = self
            .inner
            .agents
            .get(agent)
            .await
            .map_err(|error| match error {
                FindAgentError::NotFound => StartTestRunError::NotFound,
                FindAgentError::Unavailable(source) => StartTestRunError::Unavailable(source),
            })?;
        if agent.is_archived() {
            return Err(StartTestRunError::Archived);
        }
        let id = TestRunId::from_uuid(Uuid::now_v7())
            .map_err(|error| StartTestRunError::Unavailable(RepoError::unavailable(error)))?;
        let at = now();
        let run = TestRun::queue(id, agent.id(), agent.draft(), at);
        let action = AuditAction::TestRunStarted {
            run: id,
            draft_hash: run.draft_hash().clone(),
        };
        let event = audit_event(actor, &run, action, at).map_err(StartTestRunError::Unavailable)?;
        within(self.inner.runs.create(&run, &event))
            .await
            .map_err(|error| match error {
                RepoError::Conflict => StartTestRunError::AlreadyRunning,
                other => StartTestRunError::Unavailable(other),
            })?;
        tracing::info!(test_run = %id, scenarios = run.scenarios().len(), "test run queued");
        self.inner.queued.notify_one();
        Ok(run)
    }

    /// The run, with its progress and, once finished, its verdict.
    pub async fn get(&self, id: TestRunId) -> Result<TestRun, FindTestRunError> {
        Ok(within(self.inner.runs.get(id)).await?.value)
    }

    /// The agent's latest runs, newest first (at most [`RUNS_LISTED`]), with its current draft
    /// hash.
    pub async fn list(&self, agent: AgentId) -> Result<AgentTestRuns, FindAgentError> {
        let agent = self.inner.agents.get(agent).await?;
        let runs = within(self.inner.runs.list(agent.id(), RUNS_LISTED))
            .await
            .map_err(FindAgentError::Unavailable)?;
        Ok(AgentTestRuns {
            draft_hash: agent.draft().content_hash(),
            runs,
        })
    }

    /// Cancels a queued or running run (audited as `test_run.cancelled`); the worker stops
    /// it at once. Cancelling a cancelled run changes nothing.
    pub async fn cancel(
        &self,
        actor: &Actor,
        id: TestRunId,
    ) -> Result<TestRun, CancelTestRunError> {
        for _ in 0..CANCEL_ATTEMPTS {
            let stored = within(self.inner.runs.get(id))
                .await
                .map_err(FindTestRunError::from)?;
            if stored.value.status() == TestRunStatus::Cancelled {
                return Ok(stored.value);
            }
            let at = now();
            let Some(cancelled) = stored.value.cancel(at) else {
                return Err(CancelTestRunError::Finished);
            };
            let action = AuditAction::TestRunCancelled { run: id };
            let event = audit_event(actor, &cancelled, action, at)
                .map_err(CancelTestRunError::Unavailable)?;
            match within(
                self.inner
                    .runs
                    .update(&cancelled, stored.revision, Some(&event)),
            )
            .await
            {
                Err(RepoError::Conflict) => {}
                Err(error) => return Err(FindTestRunError::from(error).into()),
                Ok(_) => {
                    if let Some(token) = self.cancels().get(&id) {
                        token.cancel();
                    }
                    tracing::info!(test_run = %id, "test run cancelled");
                    return Ok(cancelled);
                }
            }
        }
        Err(CancelTestRunError::Conflict)
    }

    /// The worker that runs this service's queued runs.
    #[must_use]
    pub fn worker(&self) -> TestRunWorker {
        TestRunWorker::new(self.clone())
    }

    fn cancels(&self) -> MutexGuard<'_, HashMap<TestRunId, CancellationToken>> {
        self.inner
            .cancels
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

fn audit_event(
    actor: &Actor,
    run: &TestRun,
    action: AuditAction,
    at: jiff::Timestamp,
) -> Result<AuditEvent, RepoError> {
    let id = AuditEventId::from_uuid(Uuid::now_v7()).map_err(RepoError::unavailable)?;
    Ok(AuditEvent::new(
        id,
        at,
        actor.clone(),
        run.agent_id(),
        action,
    ))
}

/// Runs a repository call under its deadline.
async fn within<T>(call: impl Future<Output = Result<T, RepoError>>) -> Result<T, RepoError> {
    tokio::time::timeout(REPO_DEADLINE, call)
        .await
        .unwrap_or_else(|elapsed| Err(RepoError::unavailable(elapsed)))
}
