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

    /// This draft with `patch` applied. When the use case or the jurisdiction changes, the
    /// rule pack moves to the default pack for the new pair.
    #[must_use]
    pub fn apply(&self, patch: &DraftPatch) -> Self {
        let persona = Persona {
            name: patch
                .name
                .clone()
                .unwrap_or_else(|| self.persona.name.clone()),
            use_case: patch.use_case.unwrap_or(self.persona.use_case),
            voice: patch.voice.unwrap_or(self.persona.voice),
            language: patch.language.unwrap_or(self.persona.language),
        };
        let jurisdiction = patch
            .jurisdiction
            .clone()
            .unwrap_or_else(|| self.jurisdiction.clone());
        let rule_pack =
            if persona.use_case == self.persona.use_case && jurisdiction == self.jurisdiction {
                self.rule_pack.clone()
            } else {
                RulePackRef::for_agent(persona.use_case, &jurisdiction)
            };
        let settings = AgentSettings::new(
            patch.model.unwrap_or(self.settings.model()),
            patch
                .turn_detection
                .unwrap_or(self.settings.turn_detection()),
        );
        Self {
            persona,
            jurisdiction,
            rule_pack,
            settings,
        }
    }

    /// The fields that differ between `self` and `other`, in [`DraftField`] order.
    #[must_use]
    pub fn changed_fields(&self, other: &Self) -> Vec<DraftField> {
        let (a, b) = (&self.persona, &other.persona);
        [
            (DraftField::Name, a.name != b.name),
            (DraftField::UseCase, a.use_case != b.use_case),
            (DraftField::Voice, a.voice != b.voice),
            (DraftField::Language, a.language != b.language),
            (
                DraftField::Jurisdiction,
                self.jurisdiction != other.jurisdiction,
            ),
            (DraftField::RulePack, self.rule_pack != other.rule_pack),
            (
                DraftField::Model,
                self.settings.model() != other.settings.model(),
            ),
            (
                DraftField::TurnDetection,
                self.settings.turn_detection() != other.settings.turn_detection(),
            ),
        ]
        .into_iter()
        .filter_map(|(field, changed)| changed.then_some(field))
        .collect()
    }

    /// SHA-256 over a canonical encoding of every field. Equal drafts have equal hashes.
    #[must_use]
    pub fn content_hash(&self) -> ContentHash {
        ContentHash::of(&self.canonical_bytes())
    }

    /// A versioned, length-prefixed `key=<length>:<value>` line per field, in a fixed order.
    /// Length prefixes keep any value from forging another field. Changing this encoding
    /// changes every hash, so it carries a version.
    fn canonical_bytes(&self) -> Vec<u8> {
        let turn = self.settings.turn_detection();
        let fields: [(&str, String); 10] = [
            ("name", self.persona.name.as_str().to_owned()),
            ("use_case", self.persona.use_case.as_str().to_owned()),
            ("voice", self.persona.voice.as_str().to_owned()),
            ("language", self.persona.language.as_str().to_owned()),
            ("jurisdiction", self.jurisdiction.as_str().to_owned()),
            ("rule_pack.id", self.rule_pack.id().to_owned()),
            ("rule_pack.version", self.rule_pack.version().to_string()),
            ("model", self.settings.model().as_str().to_owned()),
            ("turn.silence_ms", turn.silence_ms().to_string()),
            ("turn.barge_in", turn.barge_in().to_string()),
        ];
        let mut bytes = b"vak.agent-draft.v1\n".to_vec();
        for (key, value) in fields {
            bytes.extend_from_slice(format!("{key}={}:{value}\n", value.len()).as_bytes());
        }
        bytes
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

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::catalog::{LANGUAGES, VOICES};

    fn front_desk() -> AgentDraft {
        AgentDraft::new(
            AgentName::parse("Front desk").expect("valid"),
            UseCase::Support,
            VoiceId::parse("af_heart").expect("valid"),
        )
    }

    #[test]
    fn the_encoding_is_pinned() {
        // Changing the canonical encoding changes every stored hash: bump its version tag.
        assert_eq!(
            String::from_utf8(front_desk().canonical_bytes()).expect("UTF-8"),
            "vak.agent-draft.v1\nname=10:Front desk\nuse_case=7:support\nvoice=8:af_heart\n\
             language=5:en-US\njurisdiction=2:US\nrule_pack.id=10:us-support\n\
             rule_pack.version=1:1\nmodel=8:balanced\nturn.silence_ms=3:500\n\
             turn.barge_in=4:true\n"
        );
    }

    #[test]
    fn changed_fields_lists_what_a_patch_changed_including_the_rule_pack() {
        let original = front_desk();
        let patch = DraftPatch {
            use_case: Some(UseCase::Survey),
            voice: Some(VoiceId::parse("bf_emma").expect("valid")),
            model: Some(ModelProfile::Fast),
            ..DraftPatch::default()
        };

        let patched = original.apply(&patch);

        assert_eq!(
            original.changed_fields(&patched),
            [
                DraftField::UseCase,
                DraftField::Voice,
                DraftField::RulePack,
                DraftField::Model
            ]
        );
        assert_eq!(original.changed_fields(&original), []);
        assert_eq!(DraftField::TurnDetection.as_str(), "turn_detection");
    }

    #[test]
    fn restore_keeps_a_pinned_rule_pack() {
        let pinned = RulePackRef::new("us-support", 4).expect("valid");
        let draft = front_desk();

        let restored = AgentDraft::restore(
            draft.persona().clone(),
            draft.jurisdiction().clone(),
            pinned.clone(),
            *draft.settings(),
        );
        let renamed = restored.apply(&DraftPatch {
            name: Some(AgentName::parse("Desk").expect("valid")),
            ..DraftPatch::default()
        });

        assert_eq!(renamed.rule_pack(), &pinned);
    }

    fn patches() -> impl Strategy<Value = DraftPatch> {
        let voices: Vec<&str> = VOICES.iter().map(|voice| voice.id).collect();
        let languages: Vec<&str> = LANGUAGES.iter().map(|language| language.code).collect();
        (
            proptest::option::of("[A-Za-z ]{1,20}"),
            proptest::option::of(proptest::sample::select(UseCase::ALL.to_vec())),
            proptest::option::of(proptest::sample::select(voices)),
            proptest::option::of(proptest::sample::select(languages)),
            proptest::option::of(proptest::sample::select(vec!["US", "UK", "US-CA"])),
            proptest::option::of(proptest::sample::select(ModelProfile::ALL.to_vec())),
            proptest::option::of((200_u32..=2_000, any::<bool>())),
        )
            .prop_map(
                |(name, use_case, voice, language, jurisdiction, model, turn)| DraftPatch {
                    name: name.and_then(|name: String| AgentName::parse(&name).ok()),
                    use_case,
                    voice: voice.and_then(|id| VoiceId::parse(id).ok()),
                    language: language.and_then(|code| Language::parse(code).ok()),
                    jurisdiction: jurisdiction.and_then(|code| Jurisdiction::parse(code).ok()),
                    model,
                    turn_detection: turn
                        .and_then(|(ms, barge_in)| TurnDetection::new(ms, barge_in).ok()),
                },
            )
    }

    proptest! {
        #[test]
        fn the_hash_changes_exactly_when_a_field_changes(patch in patches()) {
            let original = front_desk();
            let patched = original.apply(&patch);
            let changed = !original.changed_fields(&patched).is_empty();
            prop_assert_eq!(changed, original.content_hash() != patched.content_hash());
            prop_assert_eq!(changed, original != patched);
        }

        #[test]
        fn the_rule_pack_always_matches_the_use_case_and_jurisdiction(patch in patches()) {
            let patched = front_desk().apply(&patch);
            prop_assert_eq!(
                patched.rule_pack(),
                &RulePackRef::for_agent(patched.persona().use_case(), patched.jurisdiction())
            );
        }

        #[test]
        fn applying_a_patch_twice_changes_nothing_more(patch in patches()) {
            let once = front_desk().apply(&patch);
            prop_assert_eq!(once.apply(&patch), once);
        }
    }
}
