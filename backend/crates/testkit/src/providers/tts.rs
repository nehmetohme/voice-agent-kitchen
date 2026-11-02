//! `ScriptedTts`: audio of a scripted length for any text.

use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;
use vak_providers::{BoxFuture, ProviderError, Speech, TtsEngine, TtsRequest};

/// One recorded `synthesize` call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TtsCall {
    pub text: String,
    pub voice: String,
    pub started_at: Instant,
}

/// A [`TtsEngine`] that answers every request, after its first-audio delay (default none),
/// with audible 24 kHz audio: as long as [`ScriptedTts::with_audio_for`] says for that
/// text, otherwise 60 ms per character. Texts are compared after trimming whitespace.
/// Blank text fails with `ProviderError::Protocol`, like the serving adapter.
#[derive(Debug, Default)]
pub struct ScriptedTts {
    // CONTRACT-STUB
    _state: Mutex<()>,
}

impl ScriptedTts {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every request waits `delay` before its audio arrives.
    #[must_use]
    pub fn with_delay(self, delay: Duration) -> Self {
        // CONTRACT-STUB
        let _ = delay;
        self
    }

    /// `text` is spoken in exactly `duration`.
    #[must_use]
    pub fn with_audio_for(self, text: &str, duration: Duration) -> Self {
        // CONTRACT-STUB
        let _ = (text, duration);
        self
    }

    /// Every call so far, oldest first.
    #[must_use]
    pub fn calls(&self) -> Vec<TtsCall> {
        // CONTRACT-STUB
        Vec::new()
    }
}

impl TtsEngine for ScriptedTts {
    fn synthesize<'a>(
        &'a self,
        request: TtsRequest<'a>,
    ) -> BoxFuture<'a, Result<Speech, ProviderError>> {
        // CONTRACT-STUB
        let _ = request;
        Box::pin(async { Err(ProviderError::Unavailable) })
    }
}
