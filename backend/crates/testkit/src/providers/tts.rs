//! `ScriptedTts`: audio of a scripted length for any text.

use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;
use vak_providers::{BoxFuture, ProviderError, Speech, TtsEngine, TtsRequest};

use super::{get_mut, lock, within};

/// Audio per character of text when no length is scripted.
const PER_CHARACTER: Duration = Duration::from_millis(60);
/// Samples per millisecond at 24 kHz.
const SAMPLES_PER_MS: usize = 24;

/// One recorded `synthesize` call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TtsCall {
    pub text: String,
    pub voice: String,
    pub started_at: Instant,
}

#[derive(Debug, Default)]
struct State {
    delay: Duration,
    lengths: Vec<(String, Duration)>,
    failing: Vec<String>,
    calls: Vec<TtsCall>,
}

/// A [`TtsEngine`] that answers every request, after its first-audio delay (default none),
/// with audible 24 kHz audio: as long as [`ScriptedTts::with_audio_for`] says for that
/// text, otherwise 60 ms per character. Texts are compared after trimming whitespace.
/// Blank text fails with `ProviderError::Protocol`, like the serving adapter, and a text
/// set by [`ScriptedTts::with_failure_for`] with `ProviderError::Unavailable`.
#[derive(Debug, Default)]
pub struct ScriptedTts {
    state: Mutex<State>,
}

impl ScriptedTts {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every request waits `delay` before its audio arrives.
    #[must_use]
    pub fn with_delay(mut self, delay: Duration) -> Self {
        get_mut(&mut self.state).delay = delay;
        self
    }

    /// Every request for `text` fails with `ProviderError::Unavailable`, after the delay.
    #[must_use]
    pub fn with_failure_for(mut self, text: &str) -> Self {
        get_mut(&mut self.state)
            .failing
            .push(text.trim().to_owned());
        self
    }

    /// `text` is spoken in exactly `duration`.
    #[must_use]
    pub fn with_audio_for(mut self, text: &str, duration: Duration) -> Self {
        get_mut(&mut self.state)
            .lengths
            .push((text.trim().to_owned(), duration));
        self
    }

    /// Every call so far, oldest first.
    #[must_use]
    pub fn calls(&self) -> Vec<TtsCall> {
        lock(&self.state).calls.clone()
    }
}

impl TtsEngine for ScriptedTts {
    fn synthesize<'a>(
        &'a self,
        request: TtsRequest<'a>,
    ) -> BoxFuture<'a, Result<Speech, ProviderError>> {
        let (delay, length, fails) = {
            let mut state = lock(&self.state);
            state.calls.push(TtsCall {
                text: request.text().to_owned(),
                voice: request.voice().to_owned(),
                started_at: Instant::now(),
            });
            let text = request.text().trim();
            let length = state
                .lengths
                .iter()
                .find(|(scripted, _)| scripted == text)
                .map_or_else(
                    || {
                        let chars = u32::try_from(text.chars().count()).unwrap_or(u32::MAX);
                        PER_CHARACTER.saturating_mul(chars)
                    },
                    |(_, length)| *length,
                );
            let fails = state.failing.iter().any(|failing| failing == text);
            (state.delay, length, fails)
        };
        Box::pin(async move {
            if request.text().trim().is_empty() {
                return Err(ProviderError::Protocol("blank text".to_owned()));
            }
            within(delay, request.deadline()).await?;
            if fails {
                return Err(ProviderError::Unavailable);
            }
            let samples = usize::try_from(length.as_millis())
                .unwrap_or(usize::MAX)
                .saturating_mul(SAMPLES_PER_MS);
            Ok(Speech::new(24_000, tone(samples)))
        })
    }
}

/// A 500 Hz square wave, loud enough to pass for speech.
fn tone(samples: usize) -> Vec<i16> {
    (0..samples)
        .map(|i| if (i / 24) % 2 == 0 { 3_000 } else { -3_000 })
        .collect()
}
