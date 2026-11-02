//! `ScriptedStt`: transcripts from a script.

use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;
use vak_providers::{BoxFuture, ProviderError, SttEngine, SttRequest, Transcript};

/// One recorded `transcribe` call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SttCall {
    /// How many 16 kHz samples the utterance had.
    pub samples: usize,
    pub started_at: Instant,
}

/// An [`SttEngine`] that answers each call with the next scripted transcript, `after` the
/// scripted delay. A call with nothing left in the script fails with
/// `ProviderError::Unavailable`.
#[derive(Debug, Default)]
pub struct ScriptedStt {
    // CONTRACT-STUB
    _state: Mutex<()>,
}

impl ScriptedStt {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The next call answers `text` after `after`.
    pub fn push_reply(&self, text: &str, after: Duration) {
        // CONTRACT-STUB
        let _ = (text, after);
    }

    /// Every call so far, oldest first.
    #[must_use]
    pub fn calls(&self) -> Vec<SttCall> {
        // CONTRACT-STUB
        Vec::new()
    }
}

impl SttEngine for ScriptedStt {
    fn transcribe<'a>(
        &'a self,
        request: SttRequest<'a>,
    ) -> BoxFuture<'a, Result<Transcript, ProviderError>> {
        // CONTRACT-STUB
        let _ = request;
        Box::pin(async { Err(ProviderError::Unavailable) })
    }
}
