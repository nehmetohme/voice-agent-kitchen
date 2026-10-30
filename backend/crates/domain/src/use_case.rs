//! What an agent is for. The use case picks the starting template and the default rule pack.

use std::fmt;

/// The agent's purpose. Six options, so a picker stays within Hick's and Miller's limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UseCase {
    /// Answers questions and solves problems for existing customers.
    Support,
    /// Handles account and order requests (changes, status, billing).
    Service,
    /// Books, moves and cancels appointments.
    Booking,
    /// Asks a fixed set of questions and records the answers.
    Survey,
    /// Outbound sales calls. Carries the strictest rule pack.
    Telemarketing,
    /// Anything else; starts from a blank plan.
    Custom,
}

/// The input is not one of the [`UseCase`] names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UseCaseError {
    #[error(
        "unknown use case; expected support, service, booking, survey, telemarketing or custom"
    )]
    Unknown,
}

impl UseCase {
    /// Every use case, in the order pickers show them.
    pub const ALL: [Self; 6] = [
        Self::Support,
        Self::Service,
        Self::Booking,
        Self::Survey,
        Self::Telemarketing,
        Self::Custom,
    ];

    /// The wire and storage name, e.g. `support`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Support => "support",
            Self::Service => "service",
            Self::Booking => "booking",
            Self::Survey => "survey",
            Self::Telemarketing => "telemarketing",
            Self::Custom => "custom",
        }
    }

    /// Accepts a name in any case, with surrounding whitespace (Postel).
    pub fn parse(raw: &str) -> Result<Self, UseCaseError> {
        let raw = raw.trim();
        Self::ALL
            .into_iter()
            .find(|use_case| use_case.as_str().eq_ignore_ascii_case(raw))
            .ok_or(UseCaseError::Unknown)
    }
}

impl fmt::Display for UseCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_round_trips() {
        for use_case in UseCase::ALL {
            assert_eq!(UseCase::parse(use_case.as_str()), Ok(use_case));
            assert_eq!(UseCase::parse(&use_case.to_string()), Ok(use_case));
        }
    }
}
