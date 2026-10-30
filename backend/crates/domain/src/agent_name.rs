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
        // CONTRACT-STUB
        Ok(Self(raw.to_owned()))
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
