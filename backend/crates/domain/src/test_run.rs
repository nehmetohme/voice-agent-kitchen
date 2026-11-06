//! Test runs: an agent's scenarios played by simulated callers, scored by their rubrics, with
//! one verdict. A run is a job: queued, then running, then finished, or cancelled by the
//! operator, or interrupted when the server stops.
//!
//! The rules live here; services drive a run through them and store every step.

use std::time::Duration;

use jiff::Timestamp;

use crate::{
    AgentDraft, AgentId, ContentHash, RubricItem, Scenario, SessionEndReason, Speaker, TestRunId,
};

/// The probability of yes from which an answer counts as yes.
pub(crate) const RUBRIC_YES_FROM: f64 = 0.5;

/// Where a run is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestRunStatus {
    /// Waiting for the worker.
    Queued,
    /// The worker is playing its scenarios.
    Running,
    /// Every scenario is done; the run has a verdict.
    Finished,
    /// The operator cancelled it.
    Cancelled,
    /// The server stopped while it ran.
    Interrupted,
}

impl TestRunStatus {
    pub const ALL: [Self; 5] = [
        Self::Queued,
        Self::Running,
        Self::Finished,
        Self::Cancelled,
        Self::Interrupted,
    ];

    /// The wire and storage name, e.g. `queued`.
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

    /// The status with this storage name.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.as_str() == raw)
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

    /// The verdict with this storage name.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        [Self::Passed, Self::Failed]
            .into_iter()
            .find(|verdict| verdict.as_str() == raw)
    }

    /// Passed only if every required scenario passed; optional ones don't count.
    #[must_use]
    pub fn of(results: &[ScenarioResult]) -> Self {
        let required_passed = results
            .iter()
            .filter(|result| result.scenario.required())
            .all(|result| result.status == ScenarioStatus::Passed);
        if required_passed {
            Self::Passed
        } else {
            Self::Failed
        }
    }
}

/// Where one scenario of a run is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScenarioStatus {
    Pending,
    Running,
    /// Its conversation ended without an error and every check passed.
    Passed,
    /// A check failed.
    Failed,
    /// Its conversation failed, or a check got no answer: it couldn't be judged.
    Error,
    /// The run stopped before it was done.
    Skipped,
}

impl ScenarioStatus {
    pub const ALL: [Self; 6] = [
        Self::Pending,
        Self::Running,
        Self::Passed,
        Self::Failed,
        Self::Error,
        Self::Skipped,
    ];

    /// The wire and storage name, e.g. `passed`.
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

    /// The status with this storage name.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.as_str() == raw)
    }

    /// Whether the scenario is done: passed, failed, errored or skipped.
    #[must_use]
    pub const fn is_done(self) -> bool {
        !matches!(self, Self::Pending | Self::Running)
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

/// A rubric item and the decision model's answer to it.
#[derive(Debug, Clone, PartialEq)]
pub struct RubricCheck {
    question: String,
    expected: bool,
    probability: Option<f64>,
    passed: bool,
}

impl RubricCheck {
    /// Scores `item` with the probability of yes: it passes when the probability is on the
    /// expected side of [`RUBRIC_YES_FROM`] (which counts as yes). No answer, or one outside
    /// 0 to 1, is no answer, and fails.
    #[must_use]
    pub fn score(item: &RubricItem, probability: Option<f64>) -> Self {
        let probability = probability.filter(|p| (0.0..=1.0).contains(p));
        let passed = probability.is_some_and(|p| (p >= RUBRIC_YES_FROM) == item.expected());
        Self {
            question: item.question().to_owned(),
            expected: item.expected(),
            probability,
            passed,
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

    /// The probability of yes, or `None` when the decision model gave no usable answer.
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
    /// A scenario not played yet.
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

    /// A result read back from storage, exactly as it was saved.
    #[must_use]
    pub fn restore(
        scenario: Scenario,
        status: ScenarioStatus,
        end_reason: Option<SessionEndReason>,
        transcript: Vec<TranscriptLine>,
        checks: Vec<RubricCheck>,
        duration: Option<Duration>,
    ) -> Self {
        Self {
            scenario,
            status,
            end_reason,
            transcript,
            checks,
            duration,
        }
    }

    /// The scenario being played.
    #[must_use]
    pub fn running(&self) -> Self {
        Self {
            status: ScenarioStatus::Running,
            ..self.clone()
        }
    }

    /// The scenario played to `end`, with its transcript and checks. It passes when the
    /// conversation didn't fail and every check passed; a failed conversation, no checks or
    /// a check without an answer make it an error, since it couldn't be judged.
    #[must_use]
    pub fn finished(
        &self,
        end: SessionEndReason,
        transcript: Vec<TranscriptLine>,
        checks: Vec<RubricCheck>,
        duration: Duration,
    ) -> Self {
        let unjudged = end == SessionEndReason::Error
            || checks.is_empty()
            || checks.iter().any(|check| check.probability.is_none());
        let status = if unjudged {
            ScenarioStatus::Error
        } else if checks.iter().all(RubricCheck::passed) {
            ScenarioStatus::Passed
        } else {
            ScenarioStatus::Failed
        };
        Self {
            scenario: self.scenario.clone(),
            status,
            end_reason: Some(end),
            transcript,
            checks,
            // Whole milliseconds: what is shown and stored.
            duration: Some(Duration::from_millis(
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
            )),
        }
    }

    /// The scenario left out: its run stopped before it was played.
    #[must_use]
    pub fn skipped(&self) -> Self {
        Self {
            status: ScenarioStatus::Skipped,
            ..self.clone()
        }
    }

    #[must_use]
    pub fn scenario(&self) -> &Scenario {
        &self.scenario
    }

    #[must_use]
    pub fn status(&self) -> ScenarioStatus {
        self.status
    }

    /// How its conversation ended: the caller hung up (`ClientEnded`), an opt-out, the turn
    /// cap (`Timeout`) or a model error. `None` until played.
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

    /// How long it took to play and score, in whole milliseconds.
    #[must_use]
    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }

    /// Open scenarios become skipped; done ones stay as they are.
    fn stopped(&self) -> Self {
        if self.status.is_done() {
            self.clone()
        } else {
            self.skipped()
        }
    }
}

/// The models a run used, and the seed of its greedy sampling.
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

    /// The LLM that played the agent and the callers.
    #[must_use]
    pub fn llm(&self) -> &str {
        &self.llm
    }

    /// The decision model that answered the opt-out questions and scored the rubrics.
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

    /// 0 to 100, rounded down.
    #[must_use]
    pub fn percent(&self) -> u8 {
        let percent = self.done.saturating_mul(100).checked_div(self.total);
        percent
            .and_then(|percent| u8::try_from(percent).ok())
            .unwrap_or(0)
    }
}

/// What a stored run is made of, for [`TestRun::restore`].
#[derive(Debug, Clone, PartialEq)]
pub struct TestRunParts {
    pub id: TestRunId,
    pub agent_id: AgentId,
    pub draft_hash: ContentHash,
    pub status: TestRunStatus,
    pub models: Option<RunModels>,
    pub scenarios: Vec<ScenarioResult>,
    pub created_at: Timestamp,
    pub started_at: Option<Timestamp>,
    pub finished_at: Option<Timestamp>,
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

/// The draft's scenarios, not played yet.
fn pending(draft: &AgentDraft) -> Vec<ScenarioResult> {
    draft
        .scenarios_or_defaults()
        .scenarios()
        .iter()
        .map(ScenarioResult::pending)
        .collect()
}

impl TestRun {
    /// A run of `draft`, queued at `now`: it records the draft hash and every scenario (with
    /// its version) the draft is tested with.
    #[must_use]
    pub fn queue(id: TestRunId, agent_id: AgentId, draft: &AgentDraft, now: Timestamp) -> Self {
        Self {
            id,
            agent_id,
            draft_hash: draft.content_hash(),
            status: TestRunStatus::Queued,
            models: None,
            scenarios: pending(draft),
            created_at: now,
            started_at: None,
            finished_at: None,
        }
    }

    /// A run read back from storage, exactly as it was saved.
    #[must_use]
    pub fn restore(parts: TestRunParts) -> Self {
        Self {
            id: parts.id,
            agent_id: parts.agent_id,
            draft_hash: parts.draft_hash,
            status: parts.status,
            models: parts.models,
            scenarios: parts.scenarios,
            created_at: parts.created_at,
            started_at: parts.started_at,
            finished_at: parts.finished_at,
        }
    }

    /// The queued run started at `now` on `draft` as it is now (edits made while it waited
    /// count), with `models`. `None` unless it is queued.
    #[must_use]
    pub fn start(&self, draft: &AgentDraft, models: RunModels, now: Timestamp) -> Option<Self> {
        (self.status == TestRunStatus::Queued).then(|| Self {
            draft_hash: draft.content_hash(),
            status: TestRunStatus::Running,
            models: Some(models),
            scenarios: pending(draft),
            started_at: Some(now),
            ..self.clone()
        })
    }

    /// The running run with `result` in place of its scenario's entry. `None` unless it is
    /// running and has that scenario.
    #[must_use]
    pub fn record(&self, result: ScenarioResult) -> Option<Self> {
        if self.status != TestRunStatus::Running {
            return None;
        }
        let index = self
            .scenarios
            .iter()
            .position(|entry| entry.scenario.id() == result.scenario.id())?;
        let mut scenarios = self.scenarios.clone();
        *scenarios.get_mut(index)? = result;
        Some(Self {
            scenarios,
            ..self.clone()
        })
    }

    /// The running run finished at `now`. A scenario still open is an error: the run ended
    /// without judging it. `None` unless it is running.
    #[must_use]
    pub fn finish(&self, now: Timestamp) -> Option<Self> {
        (self.status == TestRunStatus::Running).then(|| Self {
            status: TestRunStatus::Finished,
            scenarios: self
                .scenarios
                .iter()
                .map(|result| {
                    if result.status.is_done() {
                        result.clone()
                    } else {
                        Self::unjudged(result)
                    }
                })
                .collect(),
            finished_at: Some(now),
            ..self.clone()
        })
    }

    fn unjudged(result: &ScenarioResult) -> ScenarioResult {
        ScenarioResult {
            status: ScenarioStatus::Error,
            ..result.clone()
        }
    }

    /// The run cancelled by the operator at `now`; open scenarios are skipped. `None` unless
    /// it is queued or running.
    #[must_use]
    pub fn cancel(&self, now: Timestamp) -> Option<Self> {
        self.stop(TestRunStatus::Cancelled, now)
    }

    /// The run interrupted at `now`, because the server stopped while it ran; open scenarios
    /// are skipped. `None` unless it is queued or running.
    #[must_use]
    pub fn interrupt(&self, now: Timestamp) -> Option<Self> {
        self.stop(TestRunStatus::Interrupted, now)
    }

    fn stop(&self, status: TestRunStatus, now: Timestamp) -> Option<Self> {
        self.is_active().then(|| Self {
            status,
            scenarios: self.scenarios.iter().map(ScenarioResult::stopped).collect(),
            finished_at: Some(now),
            ..self.clone()
        })
    }

    #[must_use]
    pub fn id(&self) -> TestRunId {
        self.id
    }

    #[must_use]
    pub fn agent_id(&self) -> AgentId {
        self.agent_id
    }

    /// The hash of the draft the run tests.
    #[must_use]
    pub fn draft_hash(&self) -> &ContentHash {
        &self.draft_hash
    }

    #[must_use]
    pub fn status(&self) -> TestRunStatus {
        self.status
    }

    /// The verdict, once the run finished.
    #[must_use]
    pub fn verdict(&self) -> Option<Verdict> {
        (self.status == TestRunStatus::Finished).then(|| Verdict::of(&self.scenarios))
    }

    /// The models it used, once it started.
    #[must_use]
    pub fn models(&self) -> Option<&RunModels> {
        self.models.as_ref()
    }

    /// Every scenario, in the set's order.
    #[must_use]
    pub fn scenarios(&self) -> &[ScenarioResult] {
        &self.scenarios
    }

    #[must_use]
    pub fn progress(&self) -> RunProgress {
        RunProgress {
            done: self
                .scenarios
                .iter()
                .filter(|result| result.status.is_done())
                .count(),
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

    /// When it finished, was cancelled or was interrupted.
    #[must_use]
    pub fn finished_at(&self) -> Option<Timestamp> {
        self.finished_at
    }

    /// Queued or running.
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(self.status, TestRunStatus::Queued | TestRunStatus::Running)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentName, UseCase, VoiceId};

    fn draft() -> AgentDraft {
        AgentDraft::new(
            AgentName::parse("Desk").expect("valid"),
            UseCase::Booking,
            VoiceId::parse("af_heart").expect("valid"),
        )
    }

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_second(seconds).expect("valid")
    }

    fn run() -> TestRun {
        let id = TestRunId::parse("01920000-0000-7000-8000-000000000001").expect("v7");
        let agent = AgentId::parse("01920000-0000-7000-8000-000000000002").expect("v7");
        TestRun::queue(id, agent, &draft(), at(0))
    }

    #[test]
    fn names_parse_back() {
        for status in TestRunStatus::ALL {
            assert_eq!(TestRunStatus::parse(status.as_str()), Some(status));
        }
        for status in ScenarioStatus::ALL {
            assert_eq!(ScenarioStatus::parse(status.as_str()), Some(status));
        }
        for verdict in [Verdict::Passed, Verdict::Failed] {
            assert_eq!(Verdict::parse(verdict.as_str()), Some(verdict));
        }
        assert_eq!(TestRunStatus::parse("done"), None);
        assert_eq!(ScenarioStatus::parse(""), None);
        assert_eq!(Verdict::parse("ok"), None);
    }

    #[test]
    fn finishing_with_open_scenarios_marks_them_errors() {
        let running = run()
            .start(&draft(), RunModels::new("l", "d", 1), at(1))
            .expect("starts");
        let finished = running.finish(at(2)).expect("finishes");
        assert!(
            finished
                .scenarios()
                .iter()
                .all(|r| r.status() == ScenarioStatus::Error)
        );
        assert_eq!(finished.verdict(), Some(Verdict::Failed));
    }

    #[test]
    fn a_restored_run_is_the_run_that_was_stored() {
        let original = run();
        let restored = TestRun::restore(TestRunParts {
            id: original.id(),
            agent_id: original.agent_id(),
            draft_hash: original.draft_hash().clone(),
            status: original.status(),
            models: original.models().cloned(),
            scenarios: original
                .scenarios()
                .iter()
                .map(|r| {
                    ScenarioResult::restore(
                        r.scenario().clone(),
                        r.status(),
                        r.end_reason(),
                        r.transcript().to_vec(),
                        r.checks().to_vec(),
                        r.duration(),
                    )
                })
                .collect(),
            created_at: original.created_at(),
            started_at: original.started_at(),
            finished_at: original.finished_at(),
        });
        assert_eq!(restored, original);
        assert_eq!(
            RunProgress { done: 0, total: 0 }.percent(),
            0,
            "an empty run has no progress"
        );
    }
}
