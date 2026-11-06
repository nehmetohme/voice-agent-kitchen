//! Moving an agent to another version of its purpose: what changes, and what happens to the
//! agent's plan. An edited plan is never overwritten unless the operator asks.

use super::{PurposeDefinition, PurposeError, PurposeRef, PurposeVersion};
use crate::{AgentDraft, DraftPatch, RulePackCatalog, RulePackRef, UseCase};

/// A part of a purpose that differs between two versions, in a fixed order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PurposeChange {
    Name,
    Description,
    /// The rule-pack family changed, so the agent's rule pack moves too.
    ComplianceClass {
        from: UseCase,
        to: UseCase,
    },
    Persona,
    Flow,
    Tools,
    Guardrails,
    ComplianceAdditions,
}

impl PurposeChange {
    /// The wire name of the area: `name`, `description`, `compliance_class`, `persona`,
    /// `flow`, `tools`, `guardrails` or `compliance`.
    #[must_use]
    pub const fn area(&self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Description => "description",
            Self::ComplianceClass { .. } => "compliance_class",
            Self::Persona => "persona",
            Self::Flow => "flow",
            Self::Tools => "tools",
            Self::Guardrails => "guardrails",
            Self::ComplianceAdditions => "compliance",
        }
    }
}

/// What an update does to the agent's plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlanOnUpdate {
    /// No plan is saved: the agent shows the new version's starting flow.
    FollowsPurpose,
    /// The saved plan was still the old version's flow: it becomes the new one.
    Replaced,
    /// The saved plan has the operator's edits: it stays as it is.
    KeptEdits,
    /// The saved plan had edits, and the operator chose to replace it with the new flow.
    ReplacedEdits,
}

impl PlanOnUpdate {
    /// The wire name: `follows`, `replaced`, `kept` or `replaced_edits`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FollowsPurpose => "follows",
            Self::Replaced => "replaced",
            Self::KeptEdits => "kept",
            Self::ReplacedEdits => "replaced_edits",
        }
    }
}

/// An agent's move from its pinned version to another version of the same purpose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurposeUpdate {
    from: PurposeRef,
    to: PurposeRef,
    changes: Vec<PurposeChange>,
    plan: PlanOnUpdate,
    plan_edited: bool,
    patch: DraftPatch,
}

impl PurposeUpdate {
    /// Moving `draft`, which pins `current`, to `next`. `packs` resolves the locked texts the
    /// agent's flow opens with, before and after. With `replace_plan`, an edited saved plan
    /// is replaced by the new flow; otherwise it is kept.
    pub fn new(
        draft: &AgentDraft,
        current: &PurposeVersion,
        next: &PurposeVersion,
        packs: &RulePackCatalog,
        replace_plan: bool,
    ) -> Result<Self, PurposeError> {
        let (from, to) = (current.reference(), next.reference());
        if from.id() != to.id() {
            return Err(PurposeError::OtherPurpose);
        }
        if from == to {
            return Err(PurposeError::SameVersion);
        }
        let locked_of = |pack: &RulePackRef| {
            packs
                .get(pack)
                .map(|pack| pack.locked_texts().to_vec())
                .unwrap_or_default()
        };
        let next_pack =
            RulePackRef::for_agent(next.definition().compliance(), draft.jurisdiction());
        let next_flow = next.definition().opened_flow(&locked_of(&next_pack));
        let (plan, plan_edited, new_plan) = match draft.plan() {
            None => (PlanOnUpdate::FollowsPurpose, false, None),
            Some(saved)
                if *saved
                    == current
                        .definition()
                        .opened_flow(&locked_of(draft.rule_pack())) =>
            {
                (PlanOnUpdate::Replaced, false, Some(next_flow))
            }
            Some(_) if replace_plan => (PlanOnUpdate::ReplacedEdits, true, Some(next_flow)),
            Some(_) => (PlanOnUpdate::KeptEdits, true, None),
        };
        Ok(Self {
            from,
            to,
            changes: changes(current.definition(), next.definition()),
            plan,
            plan_edited,
            patch: DraftPatch {
                purpose: Some(next.choice()),
                plan: new_plan,
                ..DraftPatch::default()
            },
        })
    }

    #[must_use]
    pub fn from(&self) -> PurposeRef {
        self.from
    }

    #[must_use]
    pub fn to(&self) -> PurposeRef {
        self.to
    }

    /// What differs between the two versions, in [`PurposeChange`] order.
    #[must_use]
    pub fn changes(&self) -> &[PurposeChange] {
        &self.changes
    }

    #[must_use]
    pub fn plan(&self) -> PlanOnUpdate {
        self.plan
    }

    /// Whether the agent's saved plan differs from the flow its current version gave it.
    #[must_use]
    pub fn plan_edited(&self) -> bool {
        self.plan_edited
    }

    /// The draft change that carries the update out.
    #[must_use]
    pub fn patch(&self) -> &DraftPatch {
        &self.patch
    }
}

/// What differs between two definitions, in [`PurposeChange`] order.
fn changes(old: &PurposeDefinition, new: &PurposeDefinition) -> Vec<PurposeChange> {
    [
        (old.name() != new.name()).then_some(PurposeChange::Name),
        (old.description() != new.description()).then_some(PurposeChange::Description),
        (old.compliance() != new.compliance()).then_some(PurposeChange::ComplianceClass {
            from: old.compliance(),
            to: new.compliance(),
        }),
        (old.persona() != new.persona()).then_some(PurposeChange::Persona),
        (old.flow() != new.flow()).then_some(PurposeChange::Flow),
        (old.tools() != new.tools()).then_some(PurposeChange::Tools),
        (old.guardrails() != new.guardrails()).then_some(PurposeChange::Guardrails),
        (old.additions() != new.additions()).then_some(PurposeChange::ComplianceAdditions),
    ]
    .into_iter()
    .flatten()
    .collect()
}
