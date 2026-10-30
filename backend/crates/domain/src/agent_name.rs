//! The agent's display name.

use std::fmt;

/// Display name of an agent: 1–80 characters after trimming, no control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentName(String);

/// Why a name is not a valid [`AgentName`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AgentNameError {
    #[error("agent name is empty")]
    Empty,
    #[error("agent name is longer than {max} characters")]
    TooLong { max: usize },
    #[error("agent name contains a control character")]
    ControlCharacter,
}

impl AgentName {
    /// Longest name, in characters.
    pub const MAX: usize = 80;

    /// Trims surrounding whitespace, then checks the length and the characters.
    pub fn parse(raw: &str) -> Result<Self, AgentNameError> {
        let name = raw.trim();
        match name.chars().count() {
            0 => Err(AgentNameError::Empty),
            n if n > Self::MAX => Err(AgentNameError::TooLong { max: Self::MAX }),
            _ if name.chars().any(char::is_control) => Err(AgentNameError::ControlCharacter),
            _ => Ok(Self(name.to_owned())),
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn counts_characters_not_bytes() {
        let name = "é".repeat(AgentName::MAX);

        assert_eq!(AgentName::parse(&name).map(|n| n.to_string()), Ok(name));
    }

    #[test]
    fn errors_explain_the_rule() {
        assert_eq!(AgentNameError::Empty.to_string(), "agent name is empty");
        assert_eq!(
            AgentNameError::TooLong { max: 80 }.to_string(),
            "agent name is longer than 80 characters"
        );
    }

    proptest! {
        #[test]
        fn parsing_is_idempotent(raw in "\\PC{0,100}") {
            if let Ok(name) = AgentName::parse(&raw) {
                prop_assert_eq!(AgentName::parse(name.as_str()), Ok(name.clone()));
                prop_assert!(name.as_str().chars().count() <= AgentName::MAX);
                prop_assert_eq!(name.as_str().trim(), name.as_str());
            }
        }
    }
}
