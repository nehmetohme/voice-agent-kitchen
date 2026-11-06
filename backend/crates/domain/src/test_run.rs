//! Test runs: an agent's scenarios played by simulated callers, scored by their rubrics, with
//! one verdict.

use std::time::Duration;

use jiff::Timestamp;

use crate::{
    AgentDraft, AgentId, ContentHash, RubricItem, Scenario, SessionEndReason, Speaker, TestRunId,
};

/// Where a run is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestRunStatus {
    Queued,
    Running,
    Finished,
    Cancelled,
    Interrupted,
}

impl TestRunStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Finished => "finished",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

/// A finished run's outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Verdict {
    Passed,
    Failed,
}

impl Verdict {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }

    /// The verdict over these results.
    #[must_use]
    pub fn of(results: &[ScenarioResult]) -> Self {
        let _ = results;
        Self::Failed
    }
}

/// Where one scenario of a run is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScenarioStatus {
    Pending,
    Running,
    Passed,
    Failed,
    Error,
    Skipped,
}

impl ScenarioStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Error => "error",
            Self::Skipped => "skipped",
        }
    }
}

/// One line of a scenario's conversation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TranscriptLine {
    speaker: Speaker,
    text: String,
}

impl TranscriptLine {
    #[must_use]
    pub fn new(speaker: Speaker, text: impl Into<String>) -> Self {
        Self {
            speaker,
            text: text.into(),
        }
    }

    #[must_use]
    pub fn speaker(&self) -> Speaker {
        self.speaker
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// A rubric item and the decision model's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct RubricCheck {
    question: String,
    expected: bool,
    probability: Option<f64>,
    passed: bool,
}

impl RubricCheck {
    /// Scores `item` with the probability of yes.
    #[must_use]
    pub fn score(item: &RubricItem, probability: Option<f64>) -> Self {
        Self {
            question: item.question().to_owned(),
            expected: item.expected(),
            probability,
            passed: false,
        }
    }

    #[must_use]
    pub fn question(&self) -> &str {
        &self.question
    }

    #[must_use]
    pub fn expected(&self) -> bool {
        self.expected
    }

    #[must_use]
    pub fn probability(&self) -> Option<f64> {
        self.probability
    }

    #[must_use]
    pub fn passed(&self) -> bool {
        self.passed
    }
}

/// One scenario of a run, and how it went.
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioResult {
    scenario: Scenario,
    status: ScenarioStatus,
    end_reason: Option<SessionEndReason>,
    transcript: Vec<TranscriptLine>,
    checks: Vec<RubricCheck>,
    duration: Option<Duration>,
}

impl ScenarioResult {
    #[must_use]
    pub fn pending(scenario: &Scenario) -> Self {
        Self {
            scenario: scenario.clone(),
            status: ScenarioStatus::Pending,
            end_reason: None,
            transcript: Vec::new(),
            checks: Vec::new(),
            duration: None,
        }
    }

    #[must_use]
    pub fn running(&self) -> Self {
        self.clone()
    }

    #[must_use]
    pub fn finished(
        &self,
        end: SessionEndReason,
        transcript: Vec<TranscriptLine>,
        checks: Vec<RubricCheck>,
        duration: Duration,
    ) -> Self {
        let _ = (end, transcript, checks, duration);
        self.clone()
    }

    #[must_use]
    pub fn skipped(&self) -> Self {
        self.clone()
    }

    #[must_use]
    pub fn scenario(&self) -> &Scenario {
        &self.scenario
    }

    #[must_use]
    pub fn status(&self) -> ScenarioStatus {
        self.status
    }

    #[must_use]
    pub fn end_reason(&self) -> Option<SessionEndReason> {
        self.end_reason
    }

    #[must_use]
    pub fn transcript(&self) -> &[TranscriptLine] {
        &self.transcript
    }

    #[must_use]
    pub fn checks(&self) -> &[RubricCheck] {
        &self.checks
    }

    #[must_use]
    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }
}

/// The models a run used, and its sampling seed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RunModels {
    llm: String,
    decisions: String,
    seed: u64,
}

impl RunModels {
    #[must_use]
    pub fn new(llm: impl Into<String>, decisions: impl Into<String>, seed: u64) -> Self {
        Self {
            llm: llm.into(),
            decisions: decisions.into(),
            seed,
        }
    }

    #[must_use]
    pub fn llm(&self) -> &str {
        &self.llm
    }

    #[must_use]
    pub fn decisions(&self) -> &str {
        &self.decisions
    }

    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }
}

/// How many of a run's scenarios are done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunProgress {
    done: usize,
    total: usize,
}

impl RunProgress {
    #[must_use]
    pub fn done(&self) -> usize {
        self.done
    }

    #[must_use]
    pub fn total(&self) -> usize {
        self.total
    }

    #[must_use]
    pub fn percent(&self) -> u8 {
        0
    }
}

/// One test run of an agent.
#[derive(Debug, Clone, PartialEq)]
pub struct TestRun {
    id: TestRunId,
    agent_id: AgentId,
    draft_hash: ContentHash,
    status: TestRunStatus,
    models: Option<RunModels>,
    scenarios: Vec<ScenarioResult>,
    created_at: Timestamp,
    started_at: Option<Timestamp>,
    finished_at: Option<Timestamp>,
}

impl TestRun {
    /// A run of `draft`, queued at `now`.
    #[must_use]
    pub fn queue(id: TestRunId, agent_id: AgentId, draft: &AgentDraft, now: Timestamp) -> Self {
        Self {
            id,
            agent_id,
            draft_hash: draft.content_hash(),
            status: TestRunStatus::Queued,
            models: None,
            scenarios: Vec::new(),
            created_at: now,
            started_at: None,
            finished_at: None,
        }
    }

    #[must_use]
    pub fn start(&self, draft: &AgentDraft, models: RunModels, now: Timestamp) -> Option<Self> {
        let _ = (draft, models, now);
        None
    }

    #[must_use]
    pub fn record(&self, result: ScenarioResult) -> Option<Self> {
        drop(result);
        None
    }

    #[must_use]
    pub fn finish(&self, now: Timestamp) -> Option<Self> {
        let _ = now;
        None
    }

    #[must_use]
    pub fn cancel(&self, now: Timestamp) -> Option<Self> {
        let _ = now;
        None
    }

    #[must_use]
    pub fn interrupt(&self, now: Timestamp) -> Option<Self> {
        let _ = now;
        None
    }

    #[must_use]
    pub fn id(&self) -> TestRunId {
        self.id
    }

    #[must_use]
    pub fn agent_id(&self) -> AgentId {
        self.agent_id
    }

    #[must_use]
    pub fn draft_hash(&self) -> &ContentHash {
        &self.draft_hash
    }

    #[must_use]
    pub fn status(&self) -> TestRunStatus {
        self.status
    }

    #[must_use]
    pub fn verdict(&self) -> Option<Verdict> {
        None
    }

    #[must_use]
    pub fn models(&self) -> Option<&RunModels> {
        self.models.as_ref()
    }

    #[must_use]
    pub fn scenarios(&self) -> &[ScenarioResult] {
        &self.scenarios
    }

    #[must_use]
    pub fn progress(&self) -> RunProgress {
        RunProgress {
            done: 0,
            total: self.scenarios.len(),
        }
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    #[must_use]
    pub fn started_at(&self) -> Option<Timestamp> {
        self.started_at
    }

    #[must_use]
    pub fn finished_at(&self) -> Option<Timestamp> {
        self.finished_at
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(self.status, TestRunStatus::Queued | TestRunStatus::Running)
    }
}
