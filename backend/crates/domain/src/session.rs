//! A voice session: who speaks a turn, and why a session ended.

/// Who spoke a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Speaker {
    /// The person talking to the agent.
    Caller,
    /// The voice agent.
    Agent,
}

impl Speaker {
    /// The wire name: `caller` or `agent`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Caller => "caller",
            Self::Agent => "agent",
        }
    }
}

/// Which of the three opt-out layers caught the caller's request not to be contacted.
/// Any one of them alone ends the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptOutLayer {
    /// The deterministic phrase list matched the caller's words.
    Phrase,
    /// The decision model's opt-out question reached its threshold.
    Decision,
    /// The LLM called the opt-out tool.
    LlmTool,
}

/// Why a session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SessionEndReason {
    /// The caller hung up or the connection closed.
    ClientEnded,
    /// The caller asked not to be contacted again.
    OptOut(OptOutLayer),
    /// The session was handed to a human (later milestones).
    Escalation,
    /// The plan reached an end node (M4b).
    EndNode,
    /// The session ran past its time limit.
    Timeout,
    /// The session could not go on.
    Error,
}
