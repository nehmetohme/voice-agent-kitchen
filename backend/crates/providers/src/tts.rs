//! `TtsEngine`: one clause of text in, its audio out.

use std::time::Duration;

use crate::{BoxFuture, ProviderError};

/// One clause to speak.
#[derive(Clone, Copy)]
pub struct TtsRequest<'a> {
    text: &'a str,
    voice: &'a str,
    speed: f32,
    deadline: Duration,
}

impl<'a> TtsRequest<'a> {
    /// `voice` is a catalog voice ID such as `af_heart`; the engine fails with
    /// [`ProviderError::Timeout`] if it has no audio within `deadline`.
    #[must_use]
    pub const fn new(text: &'a str, voice: &'a str, deadline: Duration) -> Self {
        Self {
            text,
            voice,
            speed: 1.0,
            deadline,
        }
    }

    /// Speaking rate, 0.5 to 2.0; 1.0 is normal.
    #[must_use]
    pub const fn with_speed(self, speed: f32) -> Self {
        Self { speed, ..self }
    }

    #[must_use]
    pub const fn text(&self) -> &'a str {
        self.text
    }

    #[must_use]
    pub const fn voice(&self) -> &'a str {
        self.voice
    }

    #[must_use]
    pub const fn speed(&self) -> f32 {
        self.speed
    }

    #[must_use]
    pub const fn deadline(&self) -> Duration {
        self.deadline
    }
}

impl std::fmt::Debug for TtsRequest<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TtsRequest")
            .field(
                "text",
                &format_args!("[REDACTED; {} bytes]", self.text.len()),
            )
            .field("voice", &self.voice)
            .field("speed", &self.speed)
            .field("deadline", &self.deadline)
            .finish()
    }
}

/// Synthesized speech: mono PCM16 at `sample_rate` (24 kHz from the serving contract).
#[derive(Clone, PartialEq, Eq)]
pub struct Speech {
    sample_rate: u32,
    samples: Vec<i16>,
}

impl Speech {
    #[must_use]
    pub const fn new(sample_rate: u32, samples: Vec<i16>) -> Self {
        Self {
            sample_rate,
            samples,
        }
    }

    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    #[must_use]
    pub fn samples(&self) -> &[i16] {
        &self.samples
    }

    #[must_use]
    pub fn into_samples(self) -> Vec<i16> {
        self.samples
    }

    /// How long the audio plays.
    #[must_use]
    pub fn duration(&self) -> Duration {
        // CONTRACT-STUB
        Duration::ZERO
    }
}

impl std::fmt::Debug for Speech {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Speech")
            .field("sample_rate", &self.sample_rate)
            .field("samples", &self.samples.len())
            .finish()
    }
}

/// Text to speech, one clause per call.
pub trait TtsEngine: Send + Sync {
    fn synthesize<'a>(
        &'a self,
        request: TtsRequest<'a>,
    ) -> BoxFuture<'a, Result<Speech, ProviderError>>;
}
