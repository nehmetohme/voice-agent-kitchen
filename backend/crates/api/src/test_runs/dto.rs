//! The wire format of `/api/v1/test-runs` and `/api/v1/agents/{id}/test-runs`.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use vak_domain::{
    OptOutLayer, RubricCheck, RunModels, ScenarioResult, ScenarioStatus, SessionEndReason, Speaker,
    TestRun, TestRunStatus, Verdict,
};
use vak_services::AgentTestRuns;

use crate::scenarios::ExpectedDto;

/// Where a run is in its life.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = TestRunStatus)]
pub(crate) enum TestRunStatusDto {
    /// Waiting for the worker.
    Queued,
    /// Playing its scenarios.
    Running,
    /// Every scenario is done, and the run has a verdict.
    Finished,
    /// Cancelled by an operator.
    Cancelled,
    /// The server stopped while it ran.
    Interrupted,
}

impl From<TestRunStatus> for TestRunStatusDto {
    fn from(status: TestRunStatus) -> Self {
        match status {
            TestRunStatus::Queued => Self::Queued,
            TestRunStatus::Running => Self::Running,
            TestRunStatus::Finished => Self::Finished,
            TestRunStatus::Cancelled => Self::Cancelled,
            TestRunStatus::Interrupted => Self::Interrupted,
        }
    }
}

/// A finished run's verdict: passed only if every required scenario passed.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = Verdict)]
pub(crate) enum VerdictDto {
    Passed,
    Failed,
}

impl From<Verdict> for VerdictDto {
    fn from(verdict: Verdict) -> Self {
        match verdict {
            Verdict::Passed => Self::Passed,
            Verdict::Failed => Self::Failed,
        }
    }
}

/// Where one scenario of a run is.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = ScenarioStatus)]
pub(crate) enum ScenarioStatusDto {
    Pending,
    Running,
    /// Its conversation ended without an error and every check passed.
    Passed,
    /// A check failed.
    Failed,
    /// A model failed, or a check got no answer: it couldn't be judged.
    Error,
    /// The run stopped before it was played.
    Skipped,
}

impl From<ScenarioStatus> for ScenarioStatusDto {
    fn from(status: ScenarioStatus) -> Self {
        match status {
            ScenarioStatus::Pending => Self::Pending,
            ScenarioStatus::Running => Self::Running,
            ScenarioStatus::Passed => Self::Passed,
            ScenarioStatus::Failed => Self::Failed,
            ScenarioStatus::Error => Self::Error,
            ScenarioStatus::Skipped => Self::Skipped,
        }
    }
}

/// How a scenario's conversation ended.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = ScenarioEndReason)]
pub(crate) enum EndReasonDto {
    /// The simulated caller hung up.
    CallerHungUp,
    /// The caller opted out; `optOutLayer` says which layer caught it.
    OptOut,
    /// The scenario's turn cap was reached.
    TurnCap,
    /// A model failed.
    Error,
}

/// Which opt-out layer ended the conversation.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = ScenarioOptOutLayer)]
pub(crate) enum OptOutLayerDto {
    Phrase,
    Decision,
    LlmTool,
}

fn end_reason(end: SessionEndReason) -> (EndReasonDto, Option<OptOutLayerDto>) {
    match end {
        SessionEndReason::ClientEnded => (EndReasonDto::CallerHungUp, None),
        SessionEndReason::OptOut(layer) => (
            EndReasonDto::OptOut,
            Some(match layer {
                OptOutLayer::Phrase => OptOutLayerDto::Phrase,
                OptOutLayer::Decision => OptOutLayerDto::Decision,
                OptOutLayer::LlmTool => OptOutLayerDto::LlmTool,
            }),
        ),
        SessionEndReason::Timeout => (EndReasonDto::TurnCap, None),
        // Text conversations don't escalate or follow plan nodes yet.
        SessionEndReason::Error | SessionEndReason::Escalation | SessionEndReason::EndNode => {
            (EndReasonDto::Error, None)
        }
    }
}

/// Who spoke a line: the simulated caller or the agent.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = TranscriptSpeaker)]
pub(crate) enum SpeakerDto {
    Caller,
    Agent,
}

/// One line of a scenario's conversation.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = TranscriptLine)]
pub(crate) struct TranscriptLineDto {
    speaker: SpeakerDto,
    text: String,
}

/// A rubric question and the decision model's answer.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = RubricCheck)]
pub(crate) struct RubricCheckDto {
    question: String,
    expected: ExpectedDto,
    /// The probability of yes, 0 to 1; `null` when the decision model gave no answer.
    probability: Option<f64>,
    /// Whether the answer is on the expected side of 0.5 (which counts as yes).
    passed: bool,
}

impl From<&RubricCheck> for RubricCheckDto {
    fn from(check: &RubricCheck) -> Self {
        Self {
            question: check.question().to_owned(),
            expected: ExpectedDto::of(check.expected()),
            probability: check.probability(),
            passed: check.passed(),
        }
    }
}

/// One scenario of a run: its definition as tested, and how it went.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = ScenarioResult)]
pub(crate) struct ScenarioResultDto {
    id: String,
    name: String,
    goal: String,
    persona: String,
    required: bool,
    max_turns: u8,
    /// The scenario's version: a hash of everything that defines it.
    version: String,
    status: ScenarioStatusDto,
    /// `null` until it was played.
    end_reason: Option<EndReasonDto>,
    opt_out_layer: Option<OptOutLayerDto>,
    /// How long it took to play and score.
    duration_ms: Option<u64>,
    transcript: Vec<TranscriptLineDto>,
    checks: Vec<RubricCheckDto>,
}

impl From<&ScenarioResult> for ScenarioResultDto {
    fn from(result: &ScenarioResult) -> Self {
        let scenario = result.scenario();
        let (end_reason, opt_out_layer) = result
            .end_reason()
            .map(end_reason)
            .map_or((None, None), |(end, layer)| (Some(end), layer));
        Self {
            id: scenario.id().to_string(),
            name: scenario.name().to_owned(),
            goal: scenario.goal().to_owned(),
            persona: scenario.persona().to_owned(),
            required: scenario.required(),
            max_turns: scenario.max_turns(),
            version: scenario.version().to_string(),
            status: result.status().into(),
            end_reason,
            opt_out_layer,
            duration_ms: result
                .duration()
                .and_then(|duration| u64::try_from(duration.as_millis()).ok()),
            transcript: result
                .transcript()
                .iter()
                .map(|line| TranscriptLineDto {
                    speaker: match line.speaker() {
                        Speaker::Caller => SpeakerDto::Caller,
                        Speaker::Agent => SpeakerDto::Agent,
                    },
                    text: line.text().to_owned(),
                })
                .collect(),
            checks: result.checks().iter().map(RubricCheckDto::from).collect(),
        }
    }
}

/// How many of a run's scenarios are done.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = TestRunProgress)]
pub(crate) struct ProgressDto {
    done: usize,
    total: usize,
    /// 0 to 100, rounded down.
    #[schema(minimum = 0, maximum = 100)]
    percent: u8,
}

impl ProgressDto {
    fn of(run: &TestRun) -> Self {
        let progress = run.progress();
        Self {
            done: progress.done(),
            total: progress.total(),
            percent: progress.percent(),
        }
    }
}

/// The models a run used, and the seed of its greedy sampling (temperature 0).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = TestRunModels)]
pub(crate) struct ModelsDto {
    /// The LLM that played the agent and the callers.
    llm: String,
    /// The decision model that answered the opt-out questions and scored the rubrics.
    decisions: String,
    seed: u64,
}

impl From<&RunModels> for ModelsDto {
    fn from(models: &RunModels) -> Self {
        Self {
            llm: models.llm().to_owned(),
            decisions: models.decisions().to_owned(),
            seed: models.seed(),
        }
    }
}

/// A test run: a job that plays the agent's scenarios against simulated callers.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = TestRun)]
pub(crate) struct TestRunDto {
    #[schema(format = "uuid")]
    id: String,
    #[schema(format = "uuid")]
    agent_id: String,
    status: TestRunStatusDto,
    /// `null` until the run finished.
    verdict: Option<VerdictDto>,
    /// The draft the run tests: the agent's when queued, then the one it started on.
    draft_hash: String,
    progress: ProgressDto,
    /// `null` until the run started.
    models: Option<ModelsDto>,
    created_at: String,
    started_at: Option<String>,
    /// When it finished, was cancelled or was interrupted.
    finished_at: Option<String>,
    /// Every scenario, in the set's order.
    scenarios: Vec<ScenarioResultDto>,
}

impl From<&TestRun> for TestRunDto {
    fn from(run: &TestRun) -> Self {
        Self {
            id: run.id().to_string(),
            agent_id: run.agent_id().to_string(),
            status: run.status().into(),
            verdict: run.verdict().map(VerdictDto::from),
            draft_hash: run.draft_hash().to_string(),
            progress: ProgressDto::of(run),
            models: run.models().map(ModelsDto::from),
            created_at: run.created_at().to_string(),
            started_at: run.started_at().map(|at| at.to_string()),
            finished_at: run.finished_at().map(|at| at.to_string()),
            scenarios: run
                .scenarios()
                .iter()
                .map(ScenarioResultDto::from)
                .collect(),
        }
    }
}

/// A run as an agent's list shows it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = TestRunSummary)]
pub(crate) struct TestRunSummaryDto {
    #[schema(format = "uuid")]
    id: String,
    status: TestRunStatusDto,
    verdict: Option<VerdictDto>,
    draft_hash: String,
    /// Whether the run tested the agent's current draft; `false` means it is stale.
    current: bool,
    progress: ProgressDto,
    created_at: String,
    finished_at: Option<String>,
}

/// An agent's test runs, newest first (at most 20), with its current draft hash.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = AgentTestRuns)]
pub(crate) struct AgentTestRunsDto {
    draft_hash: String,
    items: Vec<TestRunSummaryDto>,
}

impl From<&AgentTestRuns> for AgentTestRunsDto {
    fn from(list: &AgentTestRuns) -> Self {
        Self {
            draft_hash: list.draft_hash.to_string(),
            items: list
                .runs
                .iter()
                .map(|run| TestRunSummaryDto {
                    id: run.id().to_string(),
                    status: run.status().into(),
                    verdict: run.verdict().map(VerdictDto::from),
                    draft_hash: run.draft_hash().to_string(),
                    current: run.draft_hash() == &list.draft_hash,
                    progress: ProgressDto::of(run),
                    created_at: run.created_at().to_string(),
                    finished_at: run.finished_at().map(|at| at.to_string()),
                })
                .collect(),
        }
    }
}

/// Start a test run of an agent.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = StartTestRunInput)]
pub(crate) struct StartTestRunRequest {
    /// The agent to test, with its current draft and scenarios.
    #[schema(value_type = String, format = "uuid", required = true)]
    pub(crate) agent_id: Option<String>,
}
