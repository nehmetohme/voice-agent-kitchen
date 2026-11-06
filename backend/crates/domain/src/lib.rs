//! Pure domain model: entities, value objects, IDs, the plan graph and its
//! validation, rule packs, decision types and lifecycle state machines.
//!
//! This crate has no I/O, no async and no framework dependencies. Every rule is a
//! pure function with unit and property tests. The only external crates allowed
//! are listed under `[crates.vak-domain]` in `backend/layers.toml`.
//!
//! Creating an agent needs only a name, a use case and a voice; every other field
//! of the draft has a default:
//!
//! ```
//! use vak_domain::{AgentDraft, AgentName, SetupProgress, UseCase, VoiceId};
//!
//! let draft = AgentDraft::new(
//!     AgentName::parse("Front desk")?,
//!     UseCase::parse("support")?,
//!     VoiceId::parse("af_heart")?,
//! );
//! assert_eq!(draft.persona().language().as_str(), "en-US");
//! assert_eq!(draft.rule_pack().id(), "us-support");
//! assert_eq!(SetupProgress::derive(&draft, &Default::default()).percent(), 25);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod actor;
mod agent;
mod agent_name;
mod audit;
pub mod catalog;
mod decision;
mod draft;
mod draft_plan;
mod hash;
mod ids;
mod jurisdiction;
mod opt_out;
mod plan;
mod progress;
mod purpose;
mod rule_pack;
mod scenario;
mod session;
mod settings;
mod test_run;
mod timeline;
mod use_case;
mod voice;

pub use actor::{Actor, ActorError};
pub use agent::{Agent, AgentError, DraftChange};
pub use agent_name::{AgentName, AgentNameError};
pub use audit::{AuditAction, AuditEvent, PurposeAuditAction, PurposeAuditEvent};
pub use catalog::{Language, LanguageError, VoiceId, VoiceIdError};
pub use decision::{Answer, ChoiceOption, DecisionError, Question, QuestionId, QuestionKind};
pub use draft::{AgentDraft, DraftField, DraftPatch, Persona};
pub use draft_plan::DraftPlan;
pub use hash::{ContentHash, ContentHashError};
pub use ids::{AgentId, AuditEventId, IdError, PurposeId, TestRunId};
pub use jurisdiction::{Jurisdiction, JurisdictionError};
pub use opt_out::{
    DEFAULT_OPT_OUT_CHECK_QUESTION, DEFAULT_OPT_OUT_CONFIRMATION, DEFAULT_OPT_OUT_PHRASES,
    OptOutPhrases,
};
pub use plan::{EdgeId, NodeId, NodeKind, Plan, PlanEdge, PlanError, PlanNode, PlanShapeError};
pub use progress::{ProgressFacts, SetupProgress, SetupStep, StepProgress};
pub use purpose::{
    BuiltinTool, ComplianceAdditions, ComplianceAdditionsSpec, ComplianceError,
    EffectiveCompliance, EscalationAction, EscalationRule, GuardrailError, Guardrails,
    GuardrailsSpec, HttpMethod, HttpTool, HttpToolSpec, ParamKind, PersonalData, PlanOnUpdate,
    Purpose, PurposeChange, PurposeChoice, PurposeDefinition, PurposeError, PurposeName,
    PurposeNameError, PurposeProblem, PurposeRef, PurposeSpec, PurposeTools, PurposeUpdate,
    PurposeVersion, SecretRef, ToolError, ToolHosts, ToolInputs, ToolName, ToolParam, ToolUrl,
    UrlError,
};
pub use rule_pack::{
    LockedText, Retention, RulePack, RulePackCatalog, RulePackError, RulePackRef, RulePackRefError,
    RulePackSpec,
};
pub use scenario::{RubricItem, Scenario, ScenarioError, ScenarioId, ScenarioSet, ScenarioSpec};
pub use session::{OptOutLayer, SessionEndReason, Speaker};
pub use settings::{
    AgentSettings, ModelProfile, ModelProfileError, TurnDetection, TurnDetectionError,
};
pub use test_run::{
    RubricCheck, RunModels, RunProgress, ScenarioResult, ScenarioStatus, TestRun, TestRunParts,
    TestRunStatus, TranscriptLine, Verdict,
};
pub use timeline::{StageLatencies, TurnTimeline};
pub use use_case::{UseCase, UseCaseError};
pub use voice::SessionScript;
