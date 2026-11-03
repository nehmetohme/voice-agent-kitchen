//! Turn detection on the caller's audio: VAD on every 20 ms frame, the utterance being
//! spoken (with a little audio from before its first speech frame), and how long the
//! current run of speech has lasted, for barge-in.

use std::collections::VecDeque;
use std::time::Duration;

use tokio::time::Instant;
use vak_audio::{AudioFrame, Vad, VadEvent};

const FRAME: Duration = Duration::from_millis(20);
/// Audio kept from before the first speech frame, so STT hears the first syllable whole.
const PREROLL_FRAMES: usize = 10;
/// A pause shorter than this does not break a run of speech.
const RUN_GAP: Duration = Duration::from_millis(100);
/// Fewer speech frames than this is a click, not an utterance.
const MIN_SPEECH_FRAMES: u32 = 3;
/// Silence kept after the last speech frame of an utterance.
const TRAILING_SILENCE: Duration = Duration::from_millis(200);
/// The longest utterance STT gets: 30 s at 16 kHz.
const MAX_UTTERANCE_SAMPLES: usize = 480_000;

/// A finished utterance: what the caller said, to transcribe.
#[derive(Debug)]
pub(crate) struct Utterance {
    pub(crate) samples: Vec<i16>,
    /// When the last speech frame arrived.
    pub(crate) speech_end: Instant,
}

struct Speaking {
    samples: Vec<i16>,
    speech_frames: u32,
    last_speech_at: Instant,
    /// Samples up to and including the last speech frame.
    samples_at_last_speech: usize,
    run_started_at: Instant,
}

pub(crate) struct TurnDetector {
    vad: Box<dyn Vad>,
    preroll: VecDeque<AudioFrame>,
    speaking: Option<Speaking>,
    end_of_turn_silence: Duration,
}

impl TurnDetector {
    pub(crate) fn new(vad: Box<dyn Vad>, end_of_turn_silence: Duration) -> Self {
        Self {
            vad,
            preroll: VecDeque::with_capacity(PREROLL_FRAMES + 1),
            speaking: None,
            end_of_turn_silence,
        }
    }

    /// Classifies a frame that arrived at `now` (silence when `muted`). Returns how long the
    /// caller has now been speaking without a real pause, or `None` when this frame is not
    /// speech.
    pub(crate) fn push(
        &mut self,
        frame: &AudioFrame,
        now: Instant,
        muted: bool,
    ) -> Option<Duration> {
        let speech = !muted && self.vad.push(frame) == VadEvent::Speech;
        match (&mut self.speaking, speech) {
            (None, false) => {
                if self.preroll.len() == PREROLL_FRAMES {
                    self.preroll.pop_front();
                }
                self.preroll.push_back(frame.clone());
                None
            }
            (None, true) => {
                let mut samples = Vec::with_capacity(16_000);
                for earlier in self.preroll.drain(..) {
                    samples.extend_from_slice(earlier.samples());
                }
                samples.extend_from_slice(frame.samples());
                self.speaking = Some(Speaking {
                    samples_at_last_speech: samples.len(),
                    samples,
                    speech_frames: 1,
                    last_speech_at: now,
                    run_started_at: now,
                });
                Some(FRAME)
            }
            (Some(speaking), speech) => {
                if speaking.samples.len() < MAX_UTTERANCE_SAMPLES {
                    speaking.samples.extend_from_slice(frame.samples());
                }
                if !speech {
                    return None;
                }
                if now.saturating_duration_since(speaking.last_speech_at) > RUN_GAP {
                    speaking.run_started_at = now;
                }
                speaking.speech_frames += 1;
                speaking.last_speech_at = now;
                speaking.samples_at_last_speech = speaking.samples.len();
                Some(now.saturating_duration_since(speaking.run_started_at) + FRAME)
            }
        }
    }

    /// When the current utterance ends if the caller stays silent.
    pub(crate) fn end_of_turn(&self) -> Option<Instant> {
        self.speaking.as_ref().map(|speaking| {
            if speaking.samples.len() >= MAX_UTTERANCE_SAMPLES {
                speaking.last_speech_at
            } else {
                speaking.last_speech_at + self.end_of_turn_silence
            }
        })
    }

    /// Whether the caller is in the middle of an utterance.
    pub(crate) fn speaking(&self) -> bool {
        self.speaking.is_some()
    }

    /// Ends the current utterance. `None` when there was none, or only a click.
    pub(crate) fn take(&mut self) -> Option<Utterance> {
        let speaking = self.speaking.take()?;
        if speaking.speech_frames < MIN_SPEECH_FRAMES {
            return None;
        }
        // Keep a little silence after the last speech frame, no more.
        let trailing = usize::try_from(TRAILING_SILENCE.as_millis() * 16).unwrap_or(0);
        let keep = speaking
            .samples_at_last_speech
            .saturating_add(trailing)
            .min(speaking.samples.len());
        let mut samples = speaking.samples;
        samples.truncate(keep);
        Some(Utterance {
            samples,
            speech_end: speaking.last_speech_at,
        })
    }

    /// Forgets the current utterance and the preroll.
    pub(crate) fn reset(&mut self) {
        self.speaking = None;
        self.preroll.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Loud;
    impl Vad for Loud {
        fn push(&mut self, frame: &AudioFrame) -> VadEvent {
            if frame.samples().iter().any(|s| s.unsigned_abs() >= 1_000) {
                VadEvent::Speech
            } else {
                VadEvent::Silence
            }
        }
    }

    fn frame(level: i16) -> AudioFrame {
        AudioFrame::new(16_000, &[level; 320]).unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn an_utterance_runs_from_the_first_speech_to_the_silence_after_it() {
        let mut detector = TurnDetector::new(Box::new(Loud), Duration::from_millis(500));
        let start = Instant::now();
        let at = |frames: u32| start + FRAME * frames;
        for i in 0..12 {
            assert_eq!(detector.push(&frame(0), at(i), false), None);
        }
        assert!(!detector.speaking() && detector.end_of_turn().is_none());
        assert_eq!(detector.push(&frame(5_000), at(12), false), Some(FRAME));
        assert_eq!(detector.push(&frame(5_000), at(13), false), Some(FRAME * 2));
        assert_eq!(detector.push(&frame(5_000), at(14), false), Some(FRAME * 3));
        // Muted audio is silence.
        assert_eq!(detector.push(&frame(5_000), at(15), true), None);
        assert!(detector.speaking());
        assert_eq!(
            detector.end_of_turn(),
            Some(at(14) + Duration::from_millis(500))
        );
        for i in 16..40 {
            detector.push(&frame(0), at(i), false);
        }
        let utterance = detector.take().expect("an utterance");
        assert_eq!(utterance.speech_end, at(14));
        // Ten frames of preroll, three of speech, ten (200 ms) of trailing silence.
        assert_eq!(utterance.samples.len(), 23 * 320);
        assert!(detector.take().is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn a_long_pause_starts_a_new_run_and_a_click_is_no_utterance() {
        let mut detector = TurnDetector::new(Box::new(Loud), Duration::from_millis(500));
        let start = Instant::now();
        let at = |frames: u32| start + FRAME * frames;
        detector.push(&frame(5_000), at(0), false);
        detector.push(&frame(5_000), at(1), false);
        for i in 2..10 {
            detector.push(&frame(0), at(i), false);
        }
        assert_eq!(detector.push(&frame(5_000), at(10), false), Some(FRAME));
        detector.reset();
        assert!(!detector.speaking());

        detector.push(&frame(5_000), at(20), false);
        detector.push(&frame(0), at(21), false);
        assert!(detector.take().is_none(), "one speech frame is a click");
    }
}
