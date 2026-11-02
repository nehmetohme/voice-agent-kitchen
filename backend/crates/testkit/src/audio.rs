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
        let peak = frame
            .samples()
            .iter()
            .map(|sample| sample.unsigned_abs())
            .max()
            .unwrap_or(0);
        if peak >= self.threshold {
            VadEvent::Speech
        } else {
            VadEvent::Silence
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_loudest_sample_decides() {
        let mut vad = LevelVad::default();
        let frame = |level: i16| AudioFrame::new(16_000, &[level; 320]).unwrap();
        assert_eq!(vad.push(&frame(0)), VadEvent::Silence);
        assert_eq!(vad.push(&frame(999)), VadEvent::Silence);
        assert_eq!(vad.push(&frame(-1_000)), VadEvent::Speech);
        assert_eq!(LevelVad::new(10).push(&frame(10)), VadEvent::Speech);
    }
}
