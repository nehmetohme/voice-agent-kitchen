//! The agent's draft: its current, editable configuration.

use crate::{
    AgentName, AgentSettings, ContentHash, Jurisdiction, Language, ModelProfile, RulePackRef,
    TurnDetection, UseCase, VoiceId,
};

/// Who the agent is: name, purpose, voice and language.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Persona {
    name: AgentName,
    use_case: UseCase,
    voice: VoiceId,
    language: Language,
}

impl Persona {
    #[must_use]
    pub fn new(name: AgentName, use_case: UseCase, voice: VoiceId, language: Language) -> Self {
        Self {
            name,
            use_case,
            voice,
            language,
        }
    }

    #[must_use]
    pub fn name(&self) -> &AgentName {
        &self.name
    }

    #[must_use]
    pub fn use_case(&self) -> UseCase {
        self.use_case
    }

    #[must_use]
    pub fn voice(&self) -> VoiceId {
        self.voice
    }

    #[must_use]
    pub fn language(&self) -> Language {
        self.language
    }
}

/// The agent's editable configuration. Creating one needs only a name, a use case and a
/// voice; everything else starts from a default (Tesler). The rule pack always follows the
/// use case and jurisdiction.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentDraft {
    persona: Persona,
    jurisdiction: Jurisdiction,
    rule_pack: RulePackRef,
    settings: AgentSettings,
}

/// Changes to a draft. `None` leaves a field as it is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DraftPatch {
    pub name: Option<AgentName>,
    pub use_case: Option<UseCase>,
    pub voice: Option<VoiceId>,
    pub language: Option<Language>,
    pub jurisdiction: Option<Jurisdiction>,
    pub model: Option<ModelProfile>,
    pub turn_detection: Option<TurnDetection>,
}

/// A part of the draft, as named in audit events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DraftField {
    Name,
    UseCase,
    Voice,
    Language,
    Jurisdiction,
    RulePack,
    Model,
    TurnDetection,
}

impl DraftField {
    /// The storage name, e.g. `use_case`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::UseCase => "use_case",
            Self::Voice => "voice",
            Self::Language => "language",
            Self::Jurisdiction => "jurisdiction",
            Self::RulePack => "rule_pack",
            Self::Model => "model",
            Self::TurnDetection => "turn_detection",
        }
    }
}

impl AgentDraft {
    /// A new draft from the three required fields; every other field takes its default.
    #[must_use]
    pub fn new(name: AgentName, use_case: UseCase, voice: VoiceId) -> Self {
        // CONTRACT-STUB
        let jurisdiction = Jurisdiction::default();
        Self {
            rule_pack: RulePackRef::for_agent(use_case, &jurisdiction),
            persona: Persona::new(name, use_case, voice, Language::default()),
            jurisdiction,
            settings: AgentSettings::default(),
        }
    }

    /// A draft read back from storage, exactly as it was saved.
    #[must_use]
    pub fn restore(
        persona: Persona,
        jurisdiction: Jurisdiction,
        rule_pack: RulePackRef,
        settings: AgentSettings,
    ) -> Self {
        Self {
            persona,
            jurisdiction,
            rule_pack,
            settings,
        }
    }

    /// This draft with `patch` applied.
    #[must_use]
    pub fn apply(&self, patch: &DraftPatch) -> Self {
        // CONTRACT-STUB
        let _ = patch;
        self.clone()
    }

    /// The fields that differ between `self` and `other`, in [`DraftField`] order.
    #[must_use]
    pub fn changed_fields(&self, other: &Self) -> Vec<DraftField> {
        // CONTRACT-STUB
        let _ = other;
        Vec::new()
    }

    /// SHA-256 over a canonical encoding of every field. Equal drafts have equal hashes.
    #[must_use]
    pub fn content_hash(&self) -> ContentHash {
        // CONTRACT-STUB
        ContentHash::of(&[])
    }

    #[must_use]
    pub fn persona(&self) -> &Persona {
        &self.persona
    }

    #[must_use]
    pub fn jurisdiction(&self) -> &Jurisdiction {
        &self.jurisdiction
    }

    #[must_use]
    pub fn rule_pack(&self) -> &RulePackRef {
        &self.rule_pack
    }

    #[must_use]
    pub fn settings(&self) -> &AgentSettings {
        &self.settings
    }
}
