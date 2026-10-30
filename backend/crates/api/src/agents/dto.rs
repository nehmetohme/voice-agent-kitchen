//! The wire format of `/api/v1/agents` and its mapping to and from domain types.
//! Requests are parsed liberally (any case, surrounding spaces); responses are strict.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use vak_domain::{
    Agent, AgentNameError, ModelProfile, SetupProgress, SetupStep, TurnDetection, UseCase,
};
use vak_services::AgentSummary;

/// What the agent is for. It picks the starting template and the default rule pack.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = UseCase)]
pub(crate) enum UseCaseDto {
    Support,
    Service,
    Booking,
    Survey,
    Telemarketing,
    Custom,
}

impl From<UseCase> for UseCaseDto {
    fn from(use_case: UseCase) -> Self {
        match use_case {
            UseCase::Support => Self::Support,
            UseCase::Service => Self::Service,
            UseCase::Booking => Self::Booking,
            UseCase::Survey => Self::Survey,
            UseCase::Telemarketing => Self::Telemarketing,
            UseCase::Custom => Self::Custom,
        }
    }
}

/// Which LLM profile answers: `balanced` (the main voice model) or `fast` (quicker first
/// words, simpler answers).
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = ModelProfile)]
pub(crate) enum ModelProfileDto {
    Balanced,
    Fast,
}

impl From<ModelProfile> for ModelProfileDto {
    fn from(profile: ModelProfile) -> Self {
        match profile {
            ModelProfile::Balanced => Self::Balanced,
            ModelProfile::Fast => Self::Fast,
        }
    }
}

/// When the caller has finished speaking, and whether they may interrupt the agent.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = TurnDetection)]
pub(crate) struct TurnDetectionDto {
    /// Milliseconds of caller silence that end a turn, 200 to 2000.
    #[schema(minimum = 200, maximum = 2000, example = 500)]
    pub(crate) silence_ms: u32,
    /// Whether the caller can interrupt the agent while it speaks.
    pub(crate) barge_in: bool,
}

impl From<TurnDetection> for TurnDetectionDto {
    fn from(turn: TurnDetection) -> Self {
        Self {
            silence_ms: turn.silence_ms(),
            barge_in: turn.barge_in(),
        }
    }
}

/// The compliance rule pack the agent pins: `<jurisdiction>-<use case>` and its version.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = RulePack)]
pub(crate) struct RulePackDto {
    #[schema(example = "us-support")]
    id: String,
    #[schema(minimum = 1, example = 1)]
    version: u32,
}

/// A voice agent and its current draft.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = Agent)]
pub(crate) struct AgentDto {
    #[schema(format = "uuid")]
    id: String,
    #[schema(example = "Front desk")]
    name: String,
    use_case: UseCaseDto,
    /// A voice ID from `GET /api/v1/catalog/voices`.
    #[schema(example = "af_heart")]
    voice: String,
    /// A language code from `GET /api/v1/catalog/languages`.
    #[schema(example = "en-US")]
    language: String,
    /// Region code, e.g. `US`, `US-CA` or `UK`.
    #[schema(example = "US")]
    jurisdiction: String,
    rule_pack: RulePackDto,
    model: ModelProfileDto,
    turn_detection: TurnDetectionDto,
    /// SHA-256 of the draft: what a test run tested and what a version froze.
    #[schema(pattern = "^sha256:[0-9a-f]{64}$")]
    draft_hash: String,
    #[schema(format = DateTime)]
    created_at: String,
    #[schema(format = DateTime)]
    updated_at: String,
    /// When the agent was archived; `null` while it is active.
    #[schema(format = DateTime, required = true)]
    archived_at: Option<String>,
}

impl From<&Agent> for AgentDto {
    fn from(agent: &Agent) -> Self {
        let draft = agent.draft();
        let persona = draft.persona();
        let settings = draft.settings();
        Self {
            id: agent.id().to_string(),
            name: persona.name().to_string(),
            use_case: persona.use_case().into(),
            voice: persona.voice().to_string(),
            language: persona.language().to_string(),
            jurisdiction: draft.jurisdiction().to_string(),
            rule_pack: RulePackDto {
                id: draft.rule_pack().id().to_owned(),
                version: draft.rule_pack().version(),
            },
            model: settings.model().into(),
            turn_detection: settings.turn_detection().into(),
            draft_hash: draft.content_hash().to_string(),
            created_at: agent.created_at().to_string(),
            updated_at: agent.updated_at().to_string(),
            archived_at: agent.archived_at().map(|at| at.to_string()),
        }
    }
}

/// An agent as lists show it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = AgentSummary)]
pub(crate) struct AgentSummaryDto {
    #[schema(format = "uuid")]
    id: String,
    name: String,
    use_case: UseCaseDto,
    voice: String,
    #[schema(format = DateTime)]
    updated_at: String,
    /// Setup progress: 0, 25, 50, 75 or 100.
    #[schema(minimum = 0, maximum = 100)]
    progress_percent: u8,
}

impl From<&AgentSummary> for AgentSummaryDto {
    fn from(summary: &AgentSummary) -> Self {
        let persona = summary.agent.draft().persona();
        Self {
            id: summary.agent.id().to_string(),
            name: persona.name().to_string(),
            use_case: persona.use_case().into(),
            voice: persona.voice().to_string(),
            updated_at: summary.agent.updated_at().to_string(),
            progress_percent: summary.progress.percent(),
        }
    }
}

/// Active agents, most recently updated first (at most 200).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = AgentList)]
pub(crate) struct AgentListDto {
    pub(crate) items: Vec<AgentSummaryDto>,
}

/// A step of the setup flow.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = SetupStep)]
pub(crate) enum SetupStepDto {
    Provision,
    Plan,
    Test,
    Publish,
}

impl From<SetupStep> for SetupStepDto {
    fn from(step: SetupStep) -> Self {
        match step {
            SetupStep::Provision => Self::Provision,
            SetupStep::Plan => Self::Plan,
            SetupStep::Test => Self::Test,
            SetupStep::Publish => Self::Publish,
        }
    }
}

/// One setup step and whether it is done.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = SetupStepStatus)]
pub(crate) struct SetupStepStatusDto {
    step: SetupStepDto,
    done: bool,
}

/// Setup progress: four steps, 25 % each, derived from the agent's state.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = SetupProgress)]
pub(crate) struct SetupProgressDto {
    /// 0, 25, 50, 75 or 100.
    #[schema(minimum = 0, maximum = 100)]
    percent: u8,
    /// Provision, Plan, Test and Publish, in order.
    steps: Vec<SetupStepStatusDto>,
    /// The first step not done yet; `null` when setup is complete.
    #[schema(required = true)]
    next_step: Option<SetupStepDto>,
}

impl From<&SetupProgress> for SetupProgressDto {
    fn from(progress: &SetupProgress) -> Self {
        Self {
            percent: progress.percent(),
            steps: progress
                .steps()
                .iter()
                .map(|step| SetupStepStatusDto {
                    step: step.step().into(),
                    done: step.done(),
                })
                .collect(),
            next_step: progress.next_step().map(Into::into),
        }
    }
}

/// The message for an invalid name.
pub(crate) fn name_message(error: AgentNameError) -> &'static str {
    match error {
        AgentNameError::Empty => "Enter a name.",
        AgentNameError::TooLong { .. } => "Use 80 characters or fewer.",
        AgentNameError::ControlCharacter => "Remove line breaks and control characters.",
    }
}
