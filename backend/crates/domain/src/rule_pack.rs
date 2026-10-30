//! The compliance rule pack an agent pins. The packs themselves arrive in M3; until then a
//! draft records which pack and version it will use.

use crate::{Jurisdiction, UseCase};

/// A pinned rule pack: `id` names the use case and jurisdiction variant (`us-support`),
/// `version` is the pack version (1 or more).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RulePackRef {
    id: String,
    version: u32,
}

/// The stored reference is not well formed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RulePackRefError {
    #[error("rule pack id must be 1-64 lowercase letters, digits or '-'")]
    InvalidId,
    #[error("rule pack version must be 1 or more")]
    InvalidVersion,
}

impl RulePackRef {
    /// The default pack for an agent: `<jurisdiction>-<use case>`, lower case, version 1.
    #[must_use]
    pub fn for_agent(use_case: UseCase, jurisdiction: &Jurisdiction) -> Self {
        // CONTRACT-STUB
        let _ = (use_case, jurisdiction);
        Self {
            id: String::new(),
            version: 0,
        }
    }

    /// A reference read back from storage.
    pub fn new(id: &str, version: u32) -> Result<Self, RulePackRefError> {
        // CONTRACT-STUB
        Ok(Self {
            id: id.to_owned(),
            version,
        })
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }
}
