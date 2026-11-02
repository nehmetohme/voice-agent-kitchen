//! What the Plan step shows for a draft: the saved plan, or the use case's template until one
//! is saved, checked against the draft's pinned rule pack.

use crate::{AgentDraft, LockedText, Plan, PlanError, RulePackCatalog, RulePackRef};

/// A draft's plan, with the rules it is checked against and the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftPlan {
    plan: Plan,
    saved: bool,
    rule_pack: RulePackRef,
    locked_texts: Vec<LockedText>,
    opt_out_confirmation: Option<String>,
    validation: Result<(), Vec<PlanError>>,
}

impl DraftPlan {
    /// The draft's saved plan, or its use case's template (opening with the pinned pack's
    /// locked texts) when none is saved, validated against the pinned pack. When `packs` has
    /// no such pack there are no locked texts or confirmation, and the plan is invalid.
    #[must_use]
    pub fn of(draft: &AgentDraft, packs: &RulePackCatalog) -> Self {
        let pack = packs.get(draft.rule_pack());
        let locked_texts = pack
            .map(|pack| pack.locked_texts().to_vec())
            .unwrap_or_default();
        let (plan, saved) = match draft.plan() {
            Some(plan) => (plan.clone(), true),
            None => (
                Plan::template(draft.persona().use_case(), &locked_texts),
                false,
            ),
        };
        let validation = packs.validate(&plan, draft.rule_pack());
        Self {
            plan,
            saved,
            rule_pack: draft.rule_pack().clone(),
            locked_texts,
            opt_out_confirmation: pack.map(|pack| pack.opt_out_confirmation().to_owned()),
            validation,
        }
    }

    #[must_use]
    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    /// Whether the operator has saved a plan; `false` means this is the template.
    #[must_use]
    pub fn saved(&self) -> bool {
        self.saved
    }

    #[must_use]
    pub fn rule_pack(&self) -> &RulePackRef {
        &self.rule_pack
    }

    /// What the start step must open with, in order; empty when the pack is unknown.
    #[must_use]
    pub fn locked_texts(&self) -> &[LockedText] {
        &self.locked_texts
    }

    /// What the agent says when the caller opts out; `None` when the pack is unknown.
    #[must_use]
    pub fn opt_out_confirmation(&self) -> Option<&str> {
        self.opt_out_confirmation.as_deref()
    }

    /// Every rule the plan breaks, or `Ok`.
    #[must_use]
    pub fn validation(&self) -> &Result<(), Vec<PlanError>> {
        &self.validation
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;
    use crate::{
        AgentName, DraftPatch, Jurisdiction, Retention, RulePack, RulePackSpec, UseCase, VoiceId,
    };

    fn catalog() -> RulePackCatalog {
        let spec = RulePackSpec {
            id: "us-booking".to_owned(),
            version: 1,
            jurisdiction: Jurisdiction::default(),
            use_case: UseCase::Booking,
            effective: date(2026, 11, 1),
            locked: vec![
                LockedText::new(LockedText::AI_DISCLOSURE, "This is an AI.").expect("valid"),
                LockedText::new(LockedText::RECORDING_NOTICE, "It's recorded.").expect("valid"),
            ],
            opt_out_phrases: vec!["stop".to_owned()],
            opt_out_confirmation: "Goodbye.".to_owned(),
            forbidden_claims: vec![],
            retention: Retention::new(1, 1, 1).expect("valid"),
        };
        RulePackCatalog::new(vec![RulePack::new(spec).expect("valid")]).expect("one pack")
    }

    fn draft() -> AgentDraft {
        AgentDraft::new(
            AgentName::parse("Bookings").expect("valid"),
            UseCase::Booking,
            VoiceId::parse("af_heart").expect("valid"),
        )
    }

    #[test]
    fn an_unsaved_draft_shows_its_template_with_the_pinned_packs_texts() {
        let shown = DraftPlan::of(&draft(), &catalog());

        assert!(!shown.saved());
        assert_eq!(
            shown.plan(),
            &Plan::template(UseCase::Booking, shown.locked_texts())
        );
        assert_eq!(shown.locked_texts().len(), 2);
        assert_eq!(shown.opt_out_confirmation(), Some("Goodbye."));
        assert_eq!(shown.rule_pack().id(), "us-booking");
        assert_eq!(shown.validation(), &Ok(()));
    }

    #[test]
    fn a_saved_plan_is_shown_as_saved_even_without_a_pack() {
        let saved = draft().apply(&DraftPatch {
            plan: Some(Plan::default()),
            jurisdiction: Some(Jurisdiction::parse("ZZ").expect("valid")),
            ..DraftPatch::default()
        });

        let shown = DraftPlan::of(&saved, &catalog());

        assert!(shown.saved());
        assert_eq!(shown.plan(), &Plan::default());
        assert_eq!(shown.locked_texts(), []);
        assert_eq!(shown.opt_out_confirmation(), None);
        assert_eq!(
            shown.validation(),
            &Err(vec![PlanError::RulePackUnavailable {
                pack: saved.rule_pack().clone()
            }])
        );
    }
}
