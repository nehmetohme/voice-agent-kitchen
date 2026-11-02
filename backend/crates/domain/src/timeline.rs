//! Per-turn stage timings: the source of every voice latency metric.

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
