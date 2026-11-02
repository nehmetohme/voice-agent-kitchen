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
mod draft;
mod hash;
mod ids;
mod jurisdiction;
mod progress;
mod rule_pack;
mod settings;
mod use_case;

pub use actor::{Actor, ActorError};
pub use agent::{Agent, AgentError, DraftChange};
pub use agent_name::{AgentName, AgentNameError};
pub use audit::{AuditAction, AuditEvent};
pub use catalog::{Language, LanguageError, VoiceId, VoiceIdError};
pub use draft::{AgentDraft, DraftField, DraftPatch, Persona};
pub use hash::{ContentHash, ContentHashError};
pub use ids::{AgentId, AuditEventId, IdError};
pub use jurisdiction::{Jurisdiction, JurisdictionError};
pub use progress::{ProgressFacts, SetupProgress, SetupStep, StepProgress};
pub use rule_pack::{RulePackRef, RulePackRefError};
pub use settings::{
    AgentSettings, ModelProfile, ModelProfileError, TurnDetection, TurnDetectionError,
};
pub use use_case::{UseCase, UseCaseError};
