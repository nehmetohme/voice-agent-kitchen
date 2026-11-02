//! `ScriptedLlm`: streamed replies from a script.

use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;
use vak_providers::{BoxFuture, LlmEngine, LlmRequest, LlmStream, ProviderError};

/// One step of a scripted reply, emitted `after` the previous step (the first one after
/// the request).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmStep {
    // CONTRACT-STUB
    _after: Duration,
}

impl LlmStep {
    /// A piece of reply text.
    #[must_use]
    pub fn text(text: &str, after: Duration) -> Self {
        // CONTRACT-STUB
        let _ = text;
        Self { _after: after }
    }

    /// A call of tool `name` with `arguments` (JSON text).
    #[must_use]
    pub fn tool_call(name: &str, arguments: &str, after: Duration) -> Self {
        // CONTRACT-STUB
        let _ = (name, arguments);
        Self { _after: after }
    }
}

/// One recorded `stream` call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LlmCall {
    /// `(role, content)` of every message, in order; roles are `system`, `user` and
    /// `assistant`.
    pub messages: Vec<(String, String)>,
    /// The names of the offered tools.
    pub tools: Vec<String>,
    pub started_at: Instant,
    /// When the caller dropped the stream before the reply was complete.
    pub cancelled_at: Option<Instant>,
    /// Whether every scripted step and the final `Done` were delivered.
    pub completed: bool,
}

/// An [`LlmEngine`] that streams each request's reply from the next script: its steps at
/// their delays, then `LlmEvent::Done`. Dropping the stream early records the
/// cancellation. A request with nothing left in the script fails with
/// `ProviderError::Unavailable`.
#[derive(Debug, Default)]
pub struct ScriptedLlm {
    // CONTRACT-STUB
    _state: Mutex<()>,
}

impl ScriptedLlm {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The next request is answered with `steps`, then `Done`.
    pub fn push_reply(&self, steps: impl IntoIterator<Item = LlmStep>) {
        // CONTRACT-STUB
        let _ = steps.into_iter().count();
    }

    /// Every call so far, oldest first.
    #[must_use]
    pub fn calls(&self) -> Vec<LlmCall> {
        // CONTRACT-STUB
        Vec::new()
    }
}

impl LlmEngine for ScriptedLlm {
    fn stream(
        &self,
        request: LlmRequest,
    ) -> BoxFuture<'_, Result<Box<dyn LlmStream>, ProviderError>> {
        // CONTRACT-STUB
        drop(request);
        Box::pin(async { Err(ProviderError::Unavailable) })
    }
}
