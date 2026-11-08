//! The agent's draft: its current, editable configuration.

use crate::{
    AgentName, AgentSettings, ContentHash, Jurisdiction, Language, ModelProfile, Plan,
    PurposeChoice, PurposeRef, RulePackRef, ScenarioSet, TurnDetection, UseCase, VoiceId,
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
/// use case and jurisdiction. The plan is `None` until the operator saves one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentDraft {
    persona: Persona,
    jurisdiction: Jurisdiction,
    rule_pack: RulePackRef,
    settings: AgentSettings,
    plan: Option<Plan>,
    scenarios: Option<ScenarioSet>,
    purpose: PurposeRef,
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
    /// Replaces the saved plan.
    pub plan: Option<Plan>,
    /// Replaces the saved test scenarios.
    pub scenarios: Option<ScenarioSet>,
    /// Pins another purpose version; its compliance class becomes the use case. Without it,
    /// a new use case pins that use case's system purpose.
    pub purpose: Option<PurposeChoice>,
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
    Plan,
    Scenarios,
    Purpose,
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
            Self::Plan => "plan",
            Self::Scenarios => "scenarios",
            Self::Purpose => "purpose",
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
            plan: None,
            scenarios: None,
            purpose: PurposeRef::system(use_case),
        }
    }

    /// A draft read back from storage, exactly as it was saved.
    #[must_use]
    pub fn restore(
        persona: Persona,
        jurisdiction: Jurisdiction,
        rule_pack: RulePackRef,
        settings: AgentSettings,
        plan: Option<Plan>,
    ) -> Self {
        Self {
            purpose: PurposeRef::system(persona.use_case),
            persona,
            jurisdiction,
            rule_pack,
            settings,
            plan,
            scenarios: None,
        }
    }

    /// This draft as read back from storage, with its saved test scenarios (`None` until the
    /// operator saves some). Pairs with [`AgentDraft::restore`].
    #[must_use]
    pub fn with_restored_scenarios(self, scenarios: Option<ScenarioSet>) -> Self {
        Self { scenarios, ..self }
    }

    /// This restored draft pinning `purpose`, as stored. Drafts restored without it pin their
    /// use case's system purpose, version 1.
    #[must_use]
    pub fn with_purpose(self, purpose: PurposeRef) -> Self {
        Self { purpose, ..self }
    }

    /// This draft with `patch` applied. When the use case or the jurisdiction changes, the
    /// rule pack moves to the default pack for the new pair; a saved plan stays, so the
    /// operator's work is kept and checked against the new pack.
    #[must_use]
    pub fn apply(&self, patch: &DraftPatch) -> Self {
        let (use_case, purpose) = match (patch.purpose, patch.use_case) {
            (Some(choice), _) => (choice.compliance(), choice.reference()),
            (None, Some(use_case)) if use_case != self.persona.use_case => {
                (use_case, PurposeRef::system(use_case))
            }
            (None, _) => (self.persona.use_case, self.purpose),
        };
        let persona = Persona {
            name: patch
                .name
                .clone()
                .unwrap_or_else(|| self.persona.name.clone()),
            use_case,
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
            plan: patch.plan.clone().or_else(|| self.plan.clone()),
            scenarios: patch.scenarios.clone().or_else(|| self.scenarios.clone()),
            purpose,
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
            (DraftField::Plan, self.plan != other.plan),
            (DraftField::Scenarios, self.scenarios != other.scenarios),
            (DraftField::Purpose, self.purpose != other.purpose),
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
    /// changes every hash, so it carries a version. A saved plan adds its lines at the end, and
    /// saved scenarios theirs after it; a draft without them encodes exactly as before they
    /// existed, so its hash is unchanged. The pinned purpose adds its lines last, unless it is
    /// the use case's system purpose version 1, which every draft pinned before purposes existed.
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
        let mut line = |key: &str, value: &str| {
            bytes.extend_from_slice(format!("{key}={}:{value}\n", value.len()).as_bytes());
        };
        for (key, value) in fields {
            line(key, &value);
        }
        if let Some(plan) = &self.plan {
            line("plan.nodes", &plan.nodes().len().to_string());
            for (i, node) in plan.nodes().iter().enumerate() {
                line(&format!("plan.node.{i}.id"), node.id().as_str());
                line(&format!("plan.node.{i}.kind"), node.kind().as_str());
                line(&format!("plan.node.{i}.label"), node.label());
                line(&format!("plan.node.{i}.text"), node.text());
            }
            line("plan.edges", &plan.edges().len().to_string());
            for (i, edge) in plan.edges().iter().enumerate() {
                line(&format!("plan.edge.{i}.id"), edge.id().as_str());
                line(&format!("plan.edge.{i}.from"), edge.from().as_str());
                line(&format!("plan.edge.{i}.to"), edge.to().as_str());
                line(&format!("plan.edge.{i}.condition"), edge.condition());
            }
        }
        if let Some(scenarios) = &self.scenarios {
            // Each scenario by its version: a hash of everything that defines it.
            line("scenarios", &scenarios.scenarios().len().to_string());
            for (i, scenario) in scenarios.scenarios().iter().enumerate() {
                line(
                    &format!("scenario.{i}.version"),
                    scenario.version().as_str(),
                );
            }
        }
        if self.purpose != PurposeRef::system(self.persona.use_case) {
            line("purpose.id", &self.purpose.id().to_string());
            line("purpose.version", &self.purpose.version().to_string());
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

    /// The purpose version the agent pins.
    #[must_use]
    pub fn purpose(&self) -> &PurposeRef {
        &self.purpose
    }

    /// The saved plan, if the operator has saved one.
    #[must_use]
    pub fn plan(&self) -> Option<&Plan> {
        self.plan.as_ref()
    }

    /// The saved test scenarios, if the operator has saved some.
    #[must_use]
    pub fn scenarios(&self) -> Option<&ScenarioSet> {
        self.scenarios.as_ref()
    }

    /// The scenarios the agent is tested with: the saved ones, or its use case's defaults.
    #[must_use]
    pub fn scenarios_or_defaults(&self) -> ScenarioSet {
        self.scenarios
            .clone()
            .unwrap_or_else(|| ScenarioSet::defaults_for(self.persona.use_case))
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
    fn a_saved_plan_adds_its_lines_after_the_draft_fields() {
        // Drafts without a plan keep the encoding above, so their stored hashes stay valid.
        let start = crate::PlanNode::new(
            crate::NodeId::parse("s").expect("valid"),
            crate::NodeKind::Start,
            "Hi",
            "Hello.",
        )
        .expect("valid");
        let path = crate::PlanEdge::new(
            crate::EdgeId::parse("p").expect("valid"),
            crate::NodeId::parse("s").expect("valid"),
            crate::NodeId::parse("s").expect("valid"),
            "Again",
        )
        .expect("valid");
        let plan = Plan::new(vec![start], vec![path]).expect("well-formed");
        let draft = front_desk().apply(&DraftPatch {
            plan: Some(plan),
            ..DraftPatch::default()
        });

        let text = String::from_utf8(draft.canonical_bytes()).expect("UTF-8");

        assert!(text.ends_with(
            "turn.barge_in=4:true\nplan.nodes=1:1\nplan.node.0.id=1:s\n\
             plan.node.0.kind=5:start\nplan.node.0.label=2:Hi\nplan.node.0.text=6:Hello.\n\
             plan.edges=1:1\nplan.edge.0.id=1:p\nplan.edge.0.from=1:s\nplan.edge.0.to=1:s\n\
             plan.edge.0.condition=5:Again\n"
        ));
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
                DraftField::Model,
                DraftField::Purpose
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
            None,
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
                    plan: None,
                    scenarios: None,
                    purpose: None,
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
