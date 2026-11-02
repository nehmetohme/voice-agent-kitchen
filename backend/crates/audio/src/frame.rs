//! `AudioFrame`: 20 ms of mono PCM16 in a fixed-size buffer, so frames move through
//! channels without a heap allocation each.

/// Every frame holds this much audio.
pub const FRAME_MS: u32 = 20;

/// Samples in a frame at the highest supported rate (48 kHz).
pub const MAX_FRAME_SAMPLES: usize = 960;

/// 20 ms of mono signed 16-bit PCM at one sample rate (320 samples at 16 kHz, 480 at
/// 24 kHz).
#[derive(Clone, PartialEq, Eq)]
pub struct AudioFrame {
    samples: [i16; MAX_FRAME_SAMPLES],
    len: usize,
    sample_rate: u32,
}

/// The samples don't make one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    #[error("unsupported sample rate {0} Hz")]
    UnsupportedRate(u32),
    #[error("a 20 ms frame at this rate holds {expected} samples, got {got}")]
    WrongLength { expected: usize, got: usize },
}

impl AudioFrame {
    /// A frame of `samples` at `sample_rate`, which must be exactly 20 ms long.
    pub fn new(sample_rate: u32, samples: &[i16]) -> Result<Self, FrameError> {
        // CONTRACT-STUB
        let _ = samples;
        Err(FrameError::UnsupportedRate(sample_rate))
    }

    /// Samples in one frame at `sample_rate`, or `None` for an unsupported rate.
    #[must_use]
    pub const fn samples_per_frame(sample_rate: u32) -> Option<usize> {
        // CONTRACT-STUB
        let _ = sample_rate;
        None
    }

    #[must_use]
    pub fn samples(&self) -> &[i16] {
        self.samples.get(..self.len).unwrap_or_default()
    }

    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

impl std::fmt::Debug for AudioFrame {
    // Audio is personal data: show its shape, never its samples.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioFrame")
            .field("sample_rate", &self.sample_rate)
            .field("samples", &self.len)
            .finish()
    }
}
