//! Audio test support: a deterministic voice activity detector.

use vak_audio::{AudioFrame, Vad, VadEvent};

/// Speech is any frame whose loudest sample reaches `threshold` (default 1000); everything
/// else is silence. Deterministic, so pipeline simulations don't depend on a real VAD
/// model's judgement of synthetic audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelVad {
    threshold: u16,
}

impl Default for LevelVad {
    fn default() -> Self {
        Self { threshold: 1_000 }
    }
}

impl LevelVad {
    #[must_use]
    pub const fn new(threshold: u16) -> Self {
        Self { threshold }
    }
}

impl Vad for LevelVad {
    fn push(&mut self, frame: &AudioFrame) -> VadEvent {
        // CONTRACT-STUB
        let _ = (frame, self.threshold);
        VadEvent::Silence
    }
}
