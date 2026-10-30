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
    /// Longest pack ID.
    const MAX_ID: usize = 64;

    /// The default pack for an agent: `<jurisdiction>-<use case>`, lower case, version 1.
    #[must_use]
    pub fn for_agent(use_case: UseCase, jurisdiction: &Jurisdiction) -> Self {
        Self {
            id: format!(
                "{}-{}",
                jurisdiction.as_str().to_ascii_lowercase(),
                use_case.as_str()
            ),
            version: 1,
        }
    }

    /// A reference read back from storage.
    pub fn new(id: &str, version: u32) -> Result<Self, RulePackRefError> {
        let valid_id = (1..=Self::MAX_ID).contains(&id.len())
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !valid_id {
            return Err(RulePackRefError::InvalidId);
        }
        if version == 0 {
            return Err(RulePackRefError::InvalidVersion);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_pack_is_a_valid_reference() {
        let jurisdiction = Jurisdiction::parse("EU-FR").expect("valid");
        for use_case in UseCase::ALL {
            let pack = RulePackRef::for_agent(use_case, &jurisdiction);
            assert_eq!(RulePackRef::new(pack.id(), pack.version()), Ok(pack));
        }
    }

    #[test]
    fn stored_references_are_checked() {
        assert_eq!(RulePackRef::new("", 1), Err(RulePackRefError::InvalidId));
        assert_eq!(
            RulePackRef::new("US-support", 1),
            Err(RulePackRefError::InvalidId)
        );
        assert_eq!(
            RulePackRef::new(&"a".repeat(65), 1),
            Err(RulePackRefError::InvalidId)
        );
        assert_eq!(
            RulePackRef::new("us-support", 0),
            Err(RulePackRefError::InvalidVersion)
        );
        assert_eq!(
            RulePackRef::new("us-support", 3).map(|pack| pack.version()),
            Ok(3)
        );
    }
}
