//! What a purpose says: its name, compliance class, persona, starting flow, tools,
//! guardrails and compliance additions. A draft and every version hold one.

use super::canonical;
use super::{
    ComplianceAdditions, Guardrails, PurposeError, PurposeProblem, PurposeTools, ToolHosts,
};
use crate::{ContentHash, LockedText, Plan, UseCase};

/// A purpose's display name: 1–80 characters, trimmed, no control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PurposeName(String);

/// Why a name is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PurposeNameError {
    #[error("a purpose needs a name")]
    Empty,
    #[error("a purpose name is at most {max} characters")]
    TooLong { max: usize },
    #[error("a purpose name can't contain control characters")]
    ControlCharacter,
}

impl PurposeName {
    /// Longest name, in characters.
    pub const MAX: usize = 80;

    pub fn parse(raw: &str) -> Result<Self, PurposeNameError> {
        let name = raw.trim();
        match name.chars().count() {
            0 => Err(PurposeNameError::Empty),
            n if n > Self::MAX => Err(PurposeNameError::TooLong { max: Self::MAX }),
            _ if name.chars().any(char::is_control) => Err(PurposeNameError::ControlCharacter),
            _ => Ok(Self(name.to_owned())),
        }
    }

    /// A name known to be valid (the system purposes'); a unit test checks each one.
    pub(super) fn fixed(name: &str) -> Self {
        Self(name.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The parts of a purpose definition, before [`PurposeDefinition::new`] checks them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurposeSpec {
    pub name: PurposeName,
    /// Up to 500 characters.
    pub description: String,
    /// Which rule-pack family applies; with the agent's jurisdiction it picks the pack.
    pub compliance: UseCase,
    /// The default persona prompt, up to 2,000 characters.
    pub persona: String,
    /// The starting flow, its start steps without the locked texts.
    pub flow: Plan,
    pub tools: PurposeTools,
    pub guardrails: Guardrails,
    pub additions: ComplianceAdditions,
}

/// Changes to a definition: each part set replaces that part whole; `None` keeps it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PurposePatch {
    pub name: Option<PurposeName>,
    pub description: Option<String>,
    pub compliance: Option<UseCase>,
    pub persona: Option<String>,
    pub flow: Option<Plan>,
    pub tools: Option<PurposeTools>,
    pub guardrails: Option<Guardrails>,
    pub additions: Option<ComplianceAdditions>,
}

/// A checked purpose definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PurposeDefinition {
    name: PurposeName,
    description: String,
    compliance: UseCase,
    persona: String,
    flow: Plan,
    tools: PurposeTools,
    guardrails: Guardrails,
    additions: ComplianceAdditions,
}

impl PurposeDefinition {
    /// Longest description, in characters.
    pub const MAX_DESCRIPTION: usize = 500;
    /// Longest persona prompt, in characters.
    pub const MAX_PERSONA: usize = 2_000;

    /// Trims the description and persona (line endings normalised to `\n`) and checks their
    /// lengths. The parts are checked by their own constructors.
    pub fn new(spec: PurposeSpec) -> Result<Self, PurposeError> {
        let description = text(&spec.description);
        if description.chars().count() > Self::MAX_DESCRIPTION {
            return Err(PurposeError::DescriptionTooLong {
                max: Self::MAX_DESCRIPTION,
            });
        }
        let persona = text(&spec.persona);
        if persona.chars().count() > Self::MAX_PERSONA {
            return Err(PurposeError::PersonaTooLong {
                max: Self::MAX_PERSONA,
            });
        }
        Ok(Self {
            description,
            persona,
            ..Self::from_parts(spec)
        })
    }

    /// A definition known to be valid (the system purposes'); a unit test checks each one.
    pub(super) fn from_parts(spec: PurposeSpec) -> Self {
        Self {
            name: spec.name,
            description: spec.description,
            compliance: spec.compliance,
            persona: spec.persona,
            flow: spec.flow,
            tools: spec.tools,
            guardrails: spec.guardrails,
            additions: spec.additions,
        }
    }

    /// This definition's parts, to change some and check them again.
    #[must_use]
    pub fn to_spec(&self) -> PurposeSpec {
        PurposeSpec {
            name: self.name.clone(),
            description: self.description.clone(),
            compliance: self.compliance,
            persona: self.persona.clone(),
            flow: self.flow.clone(),
            tools: self.tools.clone(),
            guardrails: self.guardrails.clone(),
            additions: self.additions.clone(),
        }
    }

    /// This definition with `patch` applied, checked again.
    pub fn apply(&self, patch: &PurposePatch) -> Result<Self, PurposeError> {
        let spec = self.to_spec();
        Self::new(PurposeSpec {
            name: patch.name.clone().unwrap_or(spec.name),
            description: patch.description.clone().unwrap_or(spec.description),
            compliance: patch.compliance.unwrap_or(spec.compliance),
            persona: patch.persona.clone().unwrap_or(spec.persona),
            flow: patch.flow.clone().unwrap_or(spec.flow),
            tools: patch.tools.clone().unwrap_or(spec.tools),
            guardrails: patch.guardrails.clone().unwrap_or(spec.guardrails),
            additions: patch.additions.clone().unwrap_or(spec.additions),
        })
    }

    /// The same definition under another name.
    #[must_use]
    pub fn renamed(&self, name: PurposeName) -> Self {
        Self {
            name,
            ..self.clone()
        }
    }

    /// The starting flow as an agent gets it: each start step opens with `locked`, in order.
    #[must_use]
    pub fn opened_flow(&self, locked: &[LockedText]) -> Plan {
        self.flow.opened_with(locked)
    }

    /// What keeps this definition from being saved as a version, in a stable order: the
    /// flow's graph problems, then each HTTP tool whose host `hosts` doesn't allow.
    #[must_use]
    pub fn problems(&self, hosts: &ToolHosts) -> Vec<PurposeProblem> {
        let flow = self.flow.validate_graph().err().unwrap_or_default();
        let hosts = self
            .tools
            .http()
            .iter()
            .filter(|tool| !hosts.allows(tool.url().host()))
            .map(|tool| PurposeProblem::HostNotAllowed {
                tool: tool.name().clone(),
                host: tool.url().host().to_owned(),
            });
        flow.into_iter()
            .map(PurposeProblem::Flow)
            .chain(hosts)
            .collect()
    }

    /// SHA-256 over a canonical, versioned encoding (`vak.purpose.v1`): equal definitions
    /// have equal hashes.
    #[must_use]
    pub fn content_hash(&self) -> ContentHash {
        ContentHash::of(&canonical::bytes(self))
    }

    #[must_use]
    pub fn name(&self) -> &PurposeName {
        &self.name
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn compliance(&self) -> UseCase {
        self.compliance
    }

    #[must_use]
    pub fn persona(&self) -> &str {
        &self.persona
    }

    /// The starting flow, its start steps without the locked texts.
    #[must_use]
    pub fn flow(&self) -> &Plan {
        &self.flow
    }

    #[must_use]
    pub fn tools(&self) -> &PurposeTools {
        &self.tools
    }

    #[must_use]
    pub fn guardrails(&self) -> &Guardrails {
        &self.guardrails
    }

    #[must_use]
    pub fn additions(&self) -> &ComplianceAdditions {
        &self.additions
    }
}

/// Trimmed, with line endings normalised to `\n`.
fn text(raw: &str) -> String {
    raw.trim().replace("\r\n", "\n").replace('\r', "\n")
}
