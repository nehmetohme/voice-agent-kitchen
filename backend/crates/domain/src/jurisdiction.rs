//! Where an agent's calls happen, which selects the rule-pack variant.

use std::fmt;

/// A region code: two letters, optionally a subdivision of one to three letters or digits,
/// e.g. `US`, `US-CA`, `EU-FR`, `UK`. Stored upper case. Defaults to `US`.
///
/// Only the format is checked: which jurisdictions have a rule pack is decided when rule
/// packs arrive (M3).
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
        let code = raw.trim().replace('_', "-").to_ascii_uppercase();
        let region = |part: &str| part.len() == 2 && part.bytes().all(|b| b.is_ascii_uppercase());
        let subdivision = |part: &str| {
            (1..=3).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_alphanumeric())
        };
        let valid = match code.split_once('-') {
            None => region(&code),
            Some((head, tail)) => region(head) && subdivision(tail),
        };
        if valid {
            Ok(Self(code))
        } else {
            Err(JurisdictionError::Invalid)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Jurisdiction {
    fn default() -> Self {
        Self("US".to_owned())
    }
}

impl fmt::Display for Jurisdiction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn accepts_regions_and_subdivisions() {
        for code in ["US", "UK", "us-ny", "EU-FR", "gb_sct", "FR-75"] {
            assert!(Jurisdiction::parse(code).is_ok(), "{code}");
        }
        for code in [
            "U", "USA", "US-", "-CA", "US-CALI", "U1", "US CA", "US-C-A", "ÜS",
        ] {
            assert_eq!(
                Jurisdiction::parse(code),
                Err(JurisdictionError::Invalid),
                "{code}"
            );
        }
        assert_eq!(Jurisdiction::default().to_string(), "US");
    }

    proptest! {
        #[test]
        fn parsing_is_idempotent(raw in "[a-zA-Z0-9_ -]{0,8}") {
            if let Ok(code) = Jurisdiction::parse(&raw) {
                prop_assert_eq!(Jurisdiction::parse(code.as_str()), Ok(code.clone()));
            }
        }
    }
}
