//! Per-turn stage timings: the source of every voice latency metric.
//!
//! The pipeline stamps each stage of a reply on its [`TurnTimeline`], as offsets from the
//! start of the session (the domain reads no clock), and derives the [`StageLatencies`] from
//! it once the reply's first audio is out.

use std::time::Duration;

/// How long each stage of one agent reply took. A stage that did not run in the turn is
/// `None`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StageLatencies {
    stt: Option<Duration>,
    llm_ttft: Option<Duration>,
    tts_first_audio: Option<Duration>,
    mouth_to_ear: Option<Duration>,
}

impl StageLatencies {
    #[must_use]
    pub const fn new(
        stt: Option<Duration>,
        llm_ttft: Option<Duration>,
        tts_first_audio: Option<Duration>,
        mouth_to_ear: Option<Duration>,
    ) -> Self {
        Self {
            stt,
            llm_ttft,
            tts_first_audio,
            mouth_to_ear,
        }
    }

    /// Speech-to-text: from the request to the final transcript.
    #[must_use]
    pub const fn stt(&self) -> Option<Duration> {
        self.stt
    }

    /// LLM time to first token: from the request to the first text.
    #[must_use]
    pub const fn llm_ttft(&self) -> Option<Duration> {
        self.llm_ttft
    }

    /// Text-to-speech time to first audio: from the first clause's request to its audio.
    #[must_use]
    pub const fn tts_first_audio(&self) -> Option<Duration> {
        self.tts_first_audio
    }

    /// Mouth-to-ear: from the end of the caller's speech to the reply's first audio frame.
    #[must_use]
    pub const fn mouth_to_ear(&self) -> Option<Duration> {
        self.mouth_to_ear
    }
}

/// When each stage of one reply started and finished, as offsets from the session start.
/// Stamp each moment once, as it happens; unstamped stages count as not run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TurnTimeline {
    /// The caller's last speech frame of the turn being answered.
    pub speech_end: Option<Duration>,
    pub stt_start: Option<Duration>,
    pub stt_done: Option<Duration>,
    pub llm_start: Option<Duration>,
    pub llm_first_token: Option<Duration>,
    pub tts_start: Option<Duration>,
    pub tts_first_audio: Option<Duration>,
    /// The reply's first audio frame went out.
    pub first_audio_out: Option<Duration>,
}

impl TurnTimeline {
    /// Each stage's duration, where both of its ends are stamped and in order.
    #[must_use]
    pub fn latencies(&self) -> StageLatencies {
        let span = |from: Option<Duration>, to: Option<Duration>| to?.checked_sub(from?);
        StageLatencies {
            stt: span(self.stt_start, self.stt_done),
            llm_ttft: span(self.llm_start, self.llm_first_token),
            tts_first_audio: span(self.tts_start, self.tts_first_audio),
            mouth_to_ear: span(self.speech_end, self.first_audio_out),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stamp `millis` into the session.
    const fn at(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    #[test]
    fn latencies_are_the_spans_between_stamps() {
        let timeline = TurnTimeline {
            speech_end: Some(at(1_000)),
            stt_start: Some(at(1_500)),
            stt_done: Some(at(1_620)),
            llm_start: Some(at(1_620)),
            llm_first_token: Some(at(1_920)),
            tts_start: Some(at(1_920)),
            tts_first_audio: Some(at(2_070)),
            first_audio_out: Some(at(2_070)),
        };
        assert_eq!(
            timeline.latencies(),
            StageLatencies::new(Some(at(120)), Some(at(300)), Some(at(150)), Some(at(1_070)))
        );
        let latencies = timeline.latencies();
        assert_eq!(latencies.stt(), Some(at(120)));
        assert_eq!(latencies.llm_ttft(), Some(at(300)));
        assert_eq!(latencies.tts_first_audio(), Some(at(150)));
        assert_eq!(latencies.mouth_to_ear(), Some(at(1_070)));
    }

    #[test]
    fn a_stage_without_both_stamps_or_out_of_order_did_not_run() {
        let timeline = TurnTimeline {
            stt_start: Some(at(10)),
            llm_start: Some(at(50)),
            llm_first_token: Some(at(40)),
            ..TurnTimeline::default()
        };
        assert_eq!(timeline.latencies(), StageLatencies::default());
    }
}
