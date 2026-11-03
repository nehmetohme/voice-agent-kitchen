//! `SttEngine`: one finished utterance in, its transcript out.

use std::time::Duration;

use crate::{BoxFuture, ProviderError, SensitiveText};

/// One utterance to transcribe: mono PCM16 at 16 kHz ([`crate::media::INPUT_SAMPLE_RATE`]).
#[derive(Clone, Copy)]
pub struct SttRequest<'a> {
    audio: &'a [i16],
    language: Option<&'a str>,
    deadline: Duration,
}

impl<'a> SttRequest<'a> {
    /// `audio` is 16 kHz mono PCM16; the engine fails with [`ProviderError::Timeout`] if it
    /// has no transcript within `deadline`.
    #[must_use]
    pub const fn new(audio: &'a [i16], deadline: Duration) -> Self {
        Self {
            audio,
            language: None,
            deadline,
        }
    }

    /// An ISO 639-1 language hint, e.g. `en`.
    #[must_use]
    pub const fn with_language(self, language: &'a str) -> Self {
        Self {
            language: Some(language),
            ..self
        }
    }

    #[must_use]
    pub const fn audio(&self) -> &'a [i16] {
        self.audio
    }

    #[must_use]
    pub const fn language(&self) -> Option<&'a str> {
        self.language
    }

    #[must_use]
    pub const fn deadline(&self) -> Duration {
        self.deadline
    }
}

impl std::fmt::Debug for SttRequest<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SttRequest")
            .field("samples", &self.audio.len())
            .field("language", &self.language)
            .field("deadline", &self.deadline)
            .finish()
    }
}

/// What the caller said, trimmed; empty when no speech was recognised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    text: SensitiveText,
}

impl Transcript {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: SensitiveText::new(text),
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        self.text.as_str()
    }
}

/// Speech to text, one utterance per call.
pub trait SttEngine: Send + Sync {
    fn transcribe<'a>(
        &'a self,
        request: SttRequest<'a>,
    ) -> BoxFuture<'a, Result<Transcript, ProviderError>>;
}
