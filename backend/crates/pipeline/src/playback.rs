//! The agent's audio for the turn being spoken: its clauses' samples in order, cut into
//! 20 ms frames as the pacer sends them, and how much of each was sent, so a barge-in knows
//! which words the caller actually heard.

use std::collections::VecDeque;

use vak_audio::AudioFrame;
use vak_providers::media::OUTPUT_SAMPLE_RATE;

/// Samples in one 20 ms frame of agent audio.
const FRAME_SAMPLES: usize = 480;

/// What kind of agent turn is playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TurnKind {
    /// The locked disclosure: the caller can't interrupt it.
    Disclosure,
    /// An LLM reply: the caller may barge in.
    Reply,
    /// The locked opt-out confirmation: the caller can't interrupt it.
    Confirmation,
}

struct Clause {
    text: String,
    samples: Vec<i16>,
    sent: usize,
}

impl Clause {
    fn frames(&self) -> usize {
        self.samples.len().div_ceil(FRAME_SAMPLES)
    }

    fn sent_frames(&self) -> usize {
        self.sent.div_ceil(FRAME_SAMPLES)
    }
}

/// One agent turn's audio.
pub(crate) struct Playing {
    pub(crate) turn_id: u32,
    pub(crate) kind: TurnKind,
    clauses: VecDeque<Clause>,
    played: Vec<Clause>,
    /// No more clauses will come.
    complete: bool,
}

impl Playing {
    pub(crate) fn new(turn_id: u32, kind: TurnKind) -> Self {
        Self {
            turn_id,
            kind,
            clauses: VecDeque::new(),
            played: Vec::new(),
            complete: false,
        }
    }

    /// Queues a clause's audio (24 kHz) after the ones already queued.
    pub(crate) fn push(&mut self, text: String, samples: Vec<i16>) {
        self.clauses.push_back(Clause {
            text,
            samples,
            sent: 0,
        });
    }

    /// No more clauses will come for this turn.
    pub(crate) fn complete(&mut self) {
        self.complete = true;
    }

    /// The next frame to send, or `None` when the queued audio is used up.
    pub(crate) fn next_frame(&mut self) -> Option<AudioFrame> {
        loop {
            let clause = self.clauses.front_mut()?;
            if clause.sent < clause.samples.len() {
                // 24 kHz is a supported rate, so this can't fail; if it ever did, the turn
                // would end early, so it says so.
                let frame =
                    match AudioFrame::padded(OUTPUT_SAMPLE_RATE, &clause.samples[clause.sent..]) {
                        Ok(frame) => frame,
                        Err(error) => {
                            tracing::warn!(%error, "no agent audio frame; the turn ends early");
                            return None;
                        }
                    };
                clause.sent = (clause.sent + FRAME_SAMPLES).min(clause.samples.len());
                return Some(frame);
            }
            if let Some(done) = self.clauses.pop_front() {
                self.played.push(done);
            }
        }
    }

    /// Every frame was sent and no more clauses will come.
    pub(crate) fn finished(&self) -> bool {
        self.complete
            && self
                .clauses
                .iter()
                .all(|clause| clause.sent >= clause.samples.len())
    }

    /// Everything this turn says, clause by clause.
    pub(crate) fn text(&self) -> String {
        self.played
            .iter()
            .chain(&self.clauses)
            .map(|clause| clause.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The whole words sent so far: every clause sent in full, and of a clause cut short as
    /// many of its words as the share of its audio that went out.
    pub(crate) fn spoken(&self) -> String {
        let mut words: Vec<&str> = Vec::new();
        for clause in self.played.iter().chain(&self.clauses) {
            let all: Vec<&str> = clause.text.split_whitespace().collect();
            if clause.sent >= clause.samples.len() {
                words.extend(all);
                continue;
            }
            let heard = all.len() * clause.sent_frames() / clause.frames().max(1);
            words.extend(all.into_iter().take(heard));
            break;
        }
        words.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_come_out_in_order_and_the_last_one_is_padded() {
        let mut playing = Playing::new(2, TurnKind::Reply);
        assert!(playing.next_frame().is_none());
        playing.push("One.".to_owned(), vec![1; 960]);
        playing.push("Two.".to_owned(), vec![2; 500]);
        assert!(!playing.finished());
        let frames: Vec<i16> = std::iter::from_fn(|| playing.next_frame())
            .map(|frame| frame.samples()[0])
            .collect();
        assert_eq!(frames, [1, 1, 2, 2]);
        assert!(!playing.finished(), "more clauses may come");
        playing.complete();
        assert!(playing.finished());
        assert_eq!(playing.text(), "One. Two.");
        assert_eq!(playing.spoken(), "One. Two.");
    }

    #[test]
    fn an_interrupted_turn_was_heard_up_to_whole_words() {
        let mut playing = Playing::new(4, TurnKind::Reply);
        playing.push(
            "Let me tell you about our menu.".to_owned(),
            vec![0; 480 * 10],
        );
        assert_eq!(playing.spoken(), "");
        for _ in 0..4 {
            playing.next_frame();
        }
        // 4 of 10 frames of 7 words: 2 words.
        assert_eq!(playing.spoken(), "Let me");
        assert_eq!(playing.text(), "Let me tell you about our menu.");
        assert_eq!(playing.turn_id, 4);
        assert_eq!(playing.kind, TurnKind::Reply);
    }
}
