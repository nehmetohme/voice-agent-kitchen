//! Purposes: what an agent is for and how it may work. A purpose is a workspace-wide
//! definition (a compliance class, a persona, a starting flow, tools, guardrails and
//! compliance additions) with one editable draft and numbered, immutable versions. Agents pin
//! a version. The six use cases are the built-in system purposes, at version 1.

mod canonical;
mod compliance;
mod definition;
mod guardrails;
mod system;
mod tools;
mod update;

use jiff::Timestamp;

pub use compliance::{
    ComplianceAdditions, ComplianceAdditionsSpec, ComplianceError, EffectiveCompliance,
};
pub use definition::{PurposeDefinition, PurposeName, PurposeNameError, PurposePatch, PurposeSpec};
pub use guardrails::{
    EscalationAction, EscalationRule, GuardrailError, Guardrails, GuardrailsSpec, PersonalData,
};
pub use tools::{
    BuiltinTool, HttpMethod, HttpTool, HttpToolSpec, ParamKind, PurposeTools, SecretRef, ToolError,
    ToolHosts, ToolInputs, ToolName, ToolParam, ToolUrl, UrlError,
};
pub use update::{PlanOnUpdate, PurposeChange, PurposeUpdate};

use crate::{PlanError, PurposeId, UseCase};

/// A pinned purpose version: `(id, version)`, the version 1 or more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PurposeRef {
    id: PurposeId,
    version: u32,
}

impl PurposeRef {
    /// A reference read back from storage or a request.
    pub fn new(id: PurposeId, version: u32) -> Result<Self, PurposeError> {
        if version == 0 {
            return Err(PurposeError::InvalidVersion);
        }
        Ok(Self { id, version })
    }

    /// The use case's system purpose, version 1: what every agent pinned before purposes.
    #[must_use]
    pub fn system(use_case: UseCase) -> Self {
        Self {
            id: PurposeId::system(use_case),
            version: 1,
        }
    }

    #[must_use]
    pub fn id(&self) -> PurposeId {
        self.id
    }

    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Whether this names a built-in system purpose.
    #[must_use]
    pub fn is_system(&self) -> bool {
        self.id.system_use_case().is_some()
    }
}

/// A purpose version chosen for an agent: its reference and its compliance class, which
/// becomes the agent's use case. Built from the version, so the two always agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PurposeChoice {
    reference: PurposeRef,
    compliance: UseCase,
}

impl PurposeChoice {
    /// The use case's system purpose, version 1.
    #[must_use]
    pub fn system(use_case: UseCase) -> Self {
        Self {
            reference: PurposeRef::system(use_case),
            compliance: use_case,
        }
    }

    #[must_use]
    pub fn reference(&self) -> PurposeRef {
        self.reference
    }

    #[must_use]
    pub fn compliance(&self) -> UseCase {
        self.compliance
    }
}

/// Something that keeps a draft from being saved as a version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PurposeProblem {
    /// The starting flow breaks a graph rule.
    Flow(PlanError),
    /// An HTTP tool calls a host that isn't on the workspace's allow-list.
    HostNotAllowed { tool: ToolName, host: String },
}

/// A rule a purpose change breaks.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PurposeError {
    #[error("a built-in purpose can't change; duplicate it to customize")]
    ReadOnly,
    #[error("nothing changed since version {version}")]
    Unchanged { version: u32 },
    #[error("the draft has {} problem(s) to fix before it can be saved", problems.len())]
    Invalid { problems: Vec<PurposeProblem> },
    #[error("a purpose version is 1 or more")]
    InvalidVersion,
    #[error("the description is longer than {max} characters")]
    DescriptionTooLong { max: usize },
    #[error("the persona is longer than {max} characters")]
    PersonaTooLong { max: usize },
    #[error("the agent already pins this version")]
    SameVersion,
    #[error("that version belongs to another purpose")]
    OtherPurpose,
}

/// A saved version: immutable once made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurposeVersion {
    reference: PurposeRef,
    definition: PurposeDefinition,
    created_at: Timestamp,
}

impl PurposeVersion {
    /// The use case's system purpose, version 1.
    #[must_use]
    pub fn system(use_case: UseCase) -> Self {
        Self {
            reference: PurposeRef::system(use_case),
            definition: system::definition(use_case),
            created_at: Timestamp::UNIX_EPOCH,
        }
    }

    /// A version read back from storage, exactly as it was saved.
    #[must_use]
    pub fn restore(
        reference: PurposeRef,
        definition: PurposeDefinition,
        created_at: Timestamp,
    ) -> Self {
        Self {
            reference,
            definition,
            created_at,
        }
    }

    #[must_use]
    pub fn reference(&self) -> PurposeRef {
        self.reference
    }

    #[must_use]
    pub fn definition(&self) -> &PurposeDefinition {
        &self.definition
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// This version as an agent's choice.
    #[must_use]
    pub fn choice(&self) -> PurposeChoice {
        PurposeChoice {
            reference: self.reference,
            compliance: self.definition.compliance(),
        }
    }
}

/// A purpose: its draft and its latest saved version. System purposes are read-only; the
/// workspace's own purposes start as a draft copy of another one, without a version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Purpose {
    id: PurposeId,
    draft: PurposeDefinition,
    latest: Option<PurposeVersion>,
    created_at: Timestamp,
    updated_at: Timestamp,
}

impl Purpose {
    /// The use case's built-in purpose: version 1, and a draft equal to it.
    #[must_use]
    pub fn system(use_case: UseCase) -> Self {
        let version = PurposeVersion::system(use_case);
        Self {
            id: version.reference.id,
            draft: version.definition.clone(),
            latest: Some(version),
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: Timestamp::UNIX_EPOCH,
        }
    }

    /// A new purpose of the workspace's, made at `now` from a copy of `source` under `name`.
    /// It has no version until the draft is saved as one.
    #[must_use]
    pub fn duplicate(
        id: PurposeId,
        source: &PurposeDefinition,
        name: PurposeName,
        now: Timestamp,
    ) -> Self {
        Self {
            id,
            draft: source.renamed(name),
            latest: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// A purpose read back from storage, exactly as it was saved.
    #[must_use]
    pub fn restore(
        id: PurposeId,
        draft: PurposeDefinition,
        latest: Option<PurposeVersion>,
        created_at: Timestamp,
        updated_at: Timestamp,
    ) -> Self {
        Self {
            id,
            draft,
            latest,
            created_at,
            updated_at,
        }
    }

    /// The purpose with `draft` as its draft at `now`, or `None` when nothing changes. A
    /// system purpose can't change.
    pub fn update_draft(
        &self,
        draft: PurposeDefinition,
        now: Timestamp,
    ) -> Result<Option<Self>, PurposeError> {
        if self.is_system() {
            return Err(PurposeError::ReadOnly);
        }
        if draft == self.draft {
            return Ok(None);
        }
        Ok(Some(Self {
            draft,
            updated_at: now.max(self.updated_at),
            ..self.clone()
        }))
    }

    /// Saves the draft as the next version, at `now`. Needs a change since the latest
    /// version and a draft without problems (`hosts` is the tool host allow-list).
    pub fn save_version(
        &self,
        hosts: &ToolHosts,
        now: Timestamp,
    ) -> Result<(Self, PurposeVersion), PurposeError> {
        if self.is_system() {
            return Err(PurposeError::ReadOnly);
        }
        if let Some(version) = self
            .latest_version()
            .filter(|_| !self.has_unsaved_changes())
        {
            return Err(PurposeError::Unchanged { version });
        }
        let problems = self.problems(hosts);
        if !problems.is_empty() {
            return Err(PurposeError::Invalid { problems });
        }
        let number = self.latest_version().unwrap_or(0).saturating_add(1);
        let version = PurposeVersion {
            reference: PurposeRef::new(self.id, number)?,
            definition: self.draft.clone(),
            created_at: now,
        };
        let purpose = Self {
            latest: Some(version.clone()),
            updated_at: now.max(self.updated_at),
            ..self.clone()
        };
        Ok((purpose, version))
    }

    /// What keeps the draft from being saved as a version.
    #[must_use]
    pub fn problems(&self, hosts: &ToolHosts) -> Vec<PurposeProblem> {
        self.draft.problems(hosts)
    }

    #[must_use]
    pub fn id(&self) -> PurposeId {
        self.id
    }

    #[must_use]
    pub fn draft(&self) -> &PurposeDefinition {
        &self.draft
    }

    /// The latest saved version, if any.
    #[must_use]
    pub fn latest(&self) -> Option<&PurposeVersion> {
        self.latest.as_ref()
    }

    /// The latest version's number, if any.
    #[must_use]
    pub fn latest_version(&self) -> Option<u32> {
        self.latest
            .as_ref()
            .map(|version| version.reference.version)
    }

    /// Whether the draft differs from the latest version (always, before the first).
    #[must_use]
    pub fn has_unsaved_changes(&self) -> bool {
        self.latest
            .as_ref()
            .is_none_or(|latest| latest.definition != self.draft)
    }

    #[must_use]
    pub fn is_system(&self) -> bool {
        self.id.system_use_case().is_some()
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    #[must_use]
    pub fn updated_at(&self) -> Timestamp {
        self.updated_at
    }
}

impl PurposeId {
    /// The fixed ID of the use case's system purpose:
    /// `019a0000-0000-7000-8000-00000000000N`, N = 1–6 in [`UseCase::ALL`] order.
    #[must_use]
    pub fn system(use_case: UseCase) -> Self {
        system::id(use_case)
    }

    /// The use case whose system purpose this is, if it is one.
    #[must_use]
    pub fn system_use_case(&self) -> Option<UseCase> {
        system::use_case_of(*self)
    }
}
