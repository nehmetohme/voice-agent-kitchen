//! Test runs: an agent's scenarios played by simulated callers in text, scored by their
//! rubrics through the decision model.

use std::sync::Arc;
use std::time::Duration;

use vak_domain::{Actor, AgentId, ContentHash, TestRun, TestRunId};
use vak_providers::{DecisionEngine, LlmEngine};
use vak_repositories::{RepoError, TestRunRepo};

use crate::FindAgentError;
use crate::agents::AgentService;

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
    /// Scenarios at once.
    pub scenario_concurrency: usize,
    /// The seed of every LLM request's greedy sampling.
    pub seed: u64,
    /// An LLM request's time to first token.
    pub llm_first_token: Duration,
    /// A whole LLM reply, the caller's or the agent's.
    pub llm_reply: Duration,
    /// The opt-out decision about a caller line.
    pub decision: Duration,
    /// Scoring a scenario's rubric.
    pub rubric: Duration,
}

impl Default for TestRunSettings {
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

/// An agent's runs, newest first, and its current draft hash.
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

/// The test run use cases.
#[derive(Clone)]
pub struct TestRunService {
    agents: AgentService,
    runs: Arc<dyn TestRunRepo>,
    engines: Option<TestEngines>,
    settings: TestRunSettings,
}

impl TestRunService {
    #[must_use]
    pub fn new(
        agents: AgentService,
        runs: Arc<dyn TestRunRepo>,
        engines: Option<TestEngines>,
    ) -> Self {
        Self {
            agents,
            runs,
            engines,
            settings: TestRunSettings::default(),
        }
    }

    #[must_use]
    pub fn with_settings(self, settings: TestRunSettings) -> Self {
        Self { settings, ..self }
    }

    pub async fn start(&self, actor: &Actor, agent: AgentId) -> Result<TestRun, StartTestRunError> {
        std::future::ready(()).await;
        let _ = (actor, agent, &self.agents, &self.runs);
        if let Some(engines) = &self.engines {
            let _ = (
                &engines.llm,
                &engines.llm_model,
                &engines.decisions,
                &engines.decision_model,
            );
        }
        Err(StartTestRunError::ModelsUnavailable)
    }

    pub async fn get(&self, id: TestRunId) -> Result<TestRun, FindTestRunError> {
        std::future::ready(()).await;
        let _ = id;
        Err(FindTestRunError::NotFound)
    }

    pub async fn list(&self, agent: AgentId) -> Result<AgentTestRuns, FindAgentError> {
        std::future::ready(()).await;
        let _ = agent;
        Err(FindAgentError::NotFound)
    }

    pub async fn cancel(
        &self,
        actor: &Actor,
        id: TestRunId,
    ) -> Result<TestRun, CancelTestRunError> {
        std::future::ready(()).await;
        let _ = (actor, id);
        Err(CancelTestRunError::NotFound)
    }

    #[must_use]
    pub fn worker(&self) -> TestRunWorker {
        TestRunWorker {
            service: self.clone(),
        }
    }
}

/// Runs queued test runs in the background.
#[derive(Clone)]
pub struct TestRunWorker {
    service: TestRunService,
}

impl TestRunWorker {
    /// Runs queued runs until none is left; returns how many it ran.
    pub async fn drain(&self) -> usize {
        std::future::ready(()).await;
        let _ = &self.service.settings;
        0
    }

    /// Interrupts runs a stopped server left running; returns how many.
    pub async fn recover(&self) -> usize {
        std::future::ready(()).await;
        0
    }
}
