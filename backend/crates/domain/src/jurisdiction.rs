//! Where an agent's calls happen, which selects the rule-pack variant.

use std::fmt;

/// A region code: two letters, optionally a subdivision of one to three letters or digits,
/// e.g. `US`, `US-CA`, `EU-FR`, `UK`. Stored upper case. Defaults to `US`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Jurisdiction(String);

/// The input is not a region code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum JurisdictionError {
    #[error("not a jurisdiction code; expected e.g. US, US-CA or UK")]
    Invalid,
}

impl Jurisdiction {
    /// Accepts any case and `_` or `-` as the separator (Postel): `us_ca` is `US-CA`.
    pub fn parse(raw: &str) -> Result<Self, JurisdictionError> {
        // CONTRACT-STUB
        let _ = raw;
        Err(JurisdictionError::Invalid)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Jurisdiction {
    fn default() -> Self {
        // CONTRACT-STUB
        Self("XX".to_owned())
    }
}

impl fmt::Display for Jurisdiction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
