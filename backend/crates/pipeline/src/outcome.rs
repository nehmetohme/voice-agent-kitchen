//! What a finished session reports: why it ended and every turn's timings.

use vak_domain::{SessionEndReason, Speaker, StageLatencies};

/// One turn of a finished session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnRecord {
    pub(crate) turn_id: u32,
    pub(crate) speaker: Speaker,
    pub(crate) latencies: Option<StageLatencies>,
}

impl TurnRecord {
    #[must_use]
    pub const fn turn_id(&self) -> u32 {
        self.turn_id
    }

    #[must_use]
    pub const fn speaker(&self) -> Speaker {
        self.speaker
    }

    /// The stage timings of an agent reply; `None` for caller turns and the disclosure.
    #[must_use]
    pub const fn latencies(&self) -> Option<StageLatencies> {
        self.latencies
    }
}

/// How a session ended, and its turns in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionOutcome {
    pub(crate) end_reason: SessionEndReason,
    pub(crate) turns: Vec<TurnRecord>,
}

impl SessionOutcome {
    #[must_use]
    pub const fn end_reason(&self) -> SessionEndReason {
        self.end_reason
    }

    #[must_use]
    pub fn turns(&self) -> &[TurnRecord] {
        &self.turns
    }
}
