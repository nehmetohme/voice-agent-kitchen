//! Voice activity detection: per-frame speech or silence. Turn-taking (end of turn,
//! barge-in) is built on top of it in `vak-pipeline`.

use crate::AudioFrame;

/// What a [`Vad`] heard in one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadEvent {
    Speech,
    Silence,
}

/// Classifies 20 ms frames as speech or silence. Implementations keep whatever state they
/// need between frames and must take microseconds per frame: they run inline on the
/// session's task.
pub trait Vad: Send {
    fn push(&mut self, frame: &AudioFrame) -> VadEvent;
}

/// An energy detector with an adaptive noise floor: a frame is speech when its RMS is
/// above both a fixed minimum and a multiple of the background level, which it learns from
/// the frames it calls silence. Cheap (one pass over 320 samples) and good enough for a
/// browser microphone with echo cancellation; a model VAD (Earshot, Silero) can replace it
/// behind the same trait.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnergyVad {
    min_rms: f32,
    ratio: f32,
    noise_floor: f32,
}

impl EnergyVad {
    /// Speech must be louder than `min_rms` and `ratio` times the learned noise floor.
    #[must_use]
    pub const fn new(min_rms: f32, ratio: f32) -> Self {
        Self {
            min_rms,
            ratio,
            noise_floor: 0.0,
        }
    }

    /// The background level learned so far.
    #[must_use]
    pub const fn noise_floor(&self) -> f32 {
        self.noise_floor
    }
}

impl Default for EnergyVad {
    /// RMS 500 (about -36 dBFS) and three times the noise floor.
    fn default() -> Self {
        Self::new(500.0, 3.0)
    }
}

impl Vad for EnergyVad {
    fn push(&mut self, frame: &AudioFrame) -> VadEvent {
        let rms = frame.rms();
        if rms >= self.min_rms && rms >= self.noise_floor * self.ratio {
            VadEvent::Speech
        } else {
            // A slow average of the background, about one second to settle.
            self.noise_floor = 0.95f32.mul_add(self.noise_floor, 0.05 * rms);
            VadEvent::Silence
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(level: i16) -> AudioFrame {
        let samples: Vec<i16> = (0..320)
            .map(|i| if i % 2 == 0 { level } else { -level })
            .collect();
        AudioFrame::new(16_000, &samples).unwrap()
    }

    #[test]
    fn loud_frames_are_speech_and_quiet_ones_silence() {
        let mut vad = EnergyVad::default();
        assert_eq!(vad.push(&frame(0)), VadEvent::Silence);
        assert_eq!(vad.push(&frame(200)), VadEvent::Silence);
        assert_eq!(vad.push(&frame(4_000)), VadEvent::Speech);
        assert_eq!(vad.push(&frame(8_000)), VadEvent::Speech);
    }

    #[test]
    fn a_noisy_background_raises_the_bar() {
        let mut vad = EnergyVad::default();
        for _ in 0..200 {
            assert_eq!(vad.push(&frame(450)), VadEvent::Silence);
        }
        assert!(vad.noise_floor() > 400.0, "{}", vad.noise_floor());
        // Above the fixed minimum but not three times the background.
        assert_eq!(vad.push(&frame(900)), VadEvent::Silence);
        assert_eq!(vad.push(&frame(3_000)), VadEvent::Speech);
    }
}
