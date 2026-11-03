//! `ScriptedStt`: transcripts from a script.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;
use vak_providers::{BoxFuture, ProviderError, SttEngine, SttRequest, Transcript};

use super::{lock, within};

/// One recorded `transcribe` call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SttCall {
    /// How many 16 kHz samples the utterance had.
    pub samples: usize,
    pub started_at: Instant,
}

#[derive(Debug, Default)]
struct State {
    replies: VecDeque<(String, Duration)>,
    calls: Vec<SttCall>,
}

/// An [`SttEngine`] that answers each call with the next scripted transcript, `after` the
/// scripted delay. A call with nothing left in the script fails with
/// `ProviderError::Unavailable`.
#[derive(Debug, Default)]
pub struct ScriptedStt {
    state: Mutex<State>,
}

impl ScriptedStt {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The next call answers `text` after `after`.
    pub fn push_reply(&self, text: &str, after: Duration) {
        lock(&self.state)
            .replies
            .push_back((text.to_owned(), after));
    }

    /// Every call so far, oldest first.
    #[must_use]
    pub fn calls(&self) -> Vec<SttCall> {
        lock(&self.state).calls.clone()
    }
}

impl SttEngine for ScriptedStt {
    fn transcribe<'a>(
        &'a self,
        request: SttRequest<'a>,
    ) -> BoxFuture<'a, Result<Transcript, ProviderError>> {
        let reply = {
            let mut state = lock(&self.state);
            state.calls.push(SttCall {
                samples: request.audio().len(),
                started_at: Instant::now(),
            });
            state.replies.pop_front()
        };
        Box::pin(async move {
            let (text, after) = reply.ok_or(ProviderError::Unavailable)?;
            within(after, request.deadline()).await?;
            Ok(Transcript::new(text))
        })
    }
}
