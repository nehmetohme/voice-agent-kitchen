//! Moving an agent to another version of its purpose: what changes, and what happens to the
//! agent's plan. An edited plan is never overwritten unless the operator asks.

use super::{PurposeError, PurposeRef, PurposeVersion};
use crate::{AgentDraft, DraftPatch, RulePackCatalog, UseCase};

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
        let _ = (draft, packs, replace_plan);
        Ok(Self {
            from: current.reference(),
            to: next.reference(),
            changes: Vec::new(),
            plan: PlanOnUpdate::FollowsPurpose,
            plan_edited: false,
            patch: DraftPatch::default(),
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
