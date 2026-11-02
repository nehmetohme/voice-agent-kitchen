//! `ScriptedDecisions`: decision answers from a script.

use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;
use vak_providers::{BoxFuture, DecisionEngine, DecisionRequest, Decisions, ProviderError};

/// How one decision call goes.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionReply {
    // CONTRACT-STUB
    _after: Duration,
}

impl DecisionReply {
    /// Answers yes/no question `question` with `probability`, at once. Every other yes/no
    /// question gets 0, every choice its first option.
    #[must_use]
    pub fn yes_no(question: &str, probability: f64) -> Self {
        // CONTRACT-STUB
        let _ = (question, probability);
        Self {
            _after: Duration::ZERO,
        }
    }

    /// Never answers: the call fails with `ProviderError::Timeout` when its deadline passes.
    #[must_use]
    pub fn timeout() -> Self {
        // CONTRACT-STUB
        Self {
            _after: Duration::MAX,
        }
    }

    /// Fails with `error` at once.
    #[must_use]
    pub fn fail(error: ProviderError) -> Self {
        // CONTRACT-STUB
        drop(error);
        Self {
            _after: Duration::ZERO,
        }
    }

    /// The same reply, `delay` later.
    #[must_use]
    pub fn after(self, delay: Duration) -> Self {
        // CONTRACT-STUB
        Self { _after: delay }
    }
}

/// One recorded `decide` call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DecisionCall {
    /// The question IDs, in request order.
    pub questions: Vec<String>,
    /// `(speaker, text)` of every turn in the request's state; speakers are `caller` and
    /// `agent`.
    pub turns: Vec<(String, String)>,
    pub started_at: Instant,
}

/// A [`DecisionEngine`] that answers each call with the next scripted reply. A call with
/// nothing left in the script answers at once: 0 for every yes/no question, the first
/// option of every choice.
#[derive(Debug, Default)]
pub struct ScriptedDecisions {
    // CONTRACT-STUB
    _state: Mutex<()>,
}

impl ScriptedDecisions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The next call goes as `reply` says.
    #[expect(clippy::needless_pass_by_value, reason = "contract stub")]
    pub fn push(&self, reply: DecisionReply) {
        // CONTRACT-STUB
        let _ = reply;
    }

    /// Every call so far, oldest first.
    #[must_use]
    pub fn calls(&self) -> Vec<DecisionCall> {
        // CONTRACT-STUB
        Vec::new()
    }
}

impl DecisionEngine for ScriptedDecisions {
    fn decide<'a>(
        &'a self,
        request: &'a DecisionRequest,
    ) -> BoxFuture<'a, Result<Decisions, ProviderError>> {
        // CONTRACT-STUB
        let _ = request;
        Box::pin(async { Err(ProviderError::Unavailable) })
    }
}
