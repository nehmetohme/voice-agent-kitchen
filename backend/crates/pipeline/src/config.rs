//! What a session needs: its engines and its configuration.

use std::sync::Arc;
use std::time::Duration;

use vak_audio::{EnergyVad, Vad};
use vak_domain::StageLatencies;
use vak_providers::{DecisionEngine, LlmEngine, SttEngine, TtsEngine};

/// The model engines a session talks to. Cheap to clone; shared by every session.
#[derive(Clone)]
pub struct Engines {
    pub(crate) stt: Arc<dyn SttEngine>,
    pub(crate) llm: Arc<dyn LlmEngine>,
    pub(crate) tts: Arc<dyn TtsEngine>,
    pub(crate) decisions: Arc<dyn DecisionEngine>,
}

impl Engines {
    #[must_use]
    pub fn new(
        stt: Arc<dyn SttEngine>,
        llm: Arc<dyn LlmEngine>,
        tts: Arc<dyn TtsEngine>,
        decisions: Arc<dyn DecisionEngine>,
    ) -> Self {
        Self {
            stt,
            llm,
            tts,
            decisions,
        }
    }
}

/// When a caller's turn ends and when they may interrupt the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnPolicy {
    pub(crate) end_of_turn_silence: Duration,
    pub(crate) barge_in: Option<Duration>,
}

impl TurnPolicy {
    /// A caller's turn ends after `end_of_turn_silence` without speech. Barge-in is on: the
    /// caller interrupts the agent's reply by speaking for 300 ms.
    #[must_use]
    pub const fn new(end_of_turn_silence: Duration) -> Self {
        Self {
            end_of_turn_silence,
            barge_in: Some(Duration::from_millis(300)),
        }
    }

    /// The caller interrupts the agent after speaking over it for `min_speech`.
    #[must_use]
    pub const fn with_barge_in_min_speech(self, min_speech: Duration) -> Self {
        Self {
            barge_in: Some(min_speech),
            ..self
        }
    }

    /// The agent always finishes what it says.
    #[must_use]
    pub const fn without_barge_in(self) -> Self {
        Self {
            barge_in: None,
            ..self
        }
    }
}

/// How the session honours a caller's request not to be contacted again. Three layers,
/// each enough on its own: the phrase list, the decision model's `opt_out` question, and
/// the LLM's `mark_do_not_call` tool. A decision that times out or fails is never read as
/// a no.
#[derive(Debug, Clone, PartialEq)]
pub struct OptOutPolicy {
    pub(crate) phrases: Vec<String>,
    pub(crate) confirmation: String,
    pub(crate) decision_threshold: f64,
    pub(crate) decision_wait: Duration,
    pub(crate) decision_deadline: Duration,
    pub(crate) check_question: String,
}

impl OptOutPolicy {
    /// `phrases` end the session when the caller says one (any case, any punctuation);
    /// `confirmation` is the locked line the agent says before it ends the session.
    /// The decision layer opts out at a probability of 0.3 or more (recall first), holds
    /// the agent's reply for up to 300 ms for its answer and gives up after 2 s.
    #[must_use]
    pub fn new(
        phrases: impl IntoIterator<Item = impl Into<String>>,
        confirmation: impl Into<String>,
    ) -> Self {
        Self {
            phrases: phrases.into_iter().map(Into::into).collect(),
            confirmation: confirmation.into(),
            decision_threshold: 0.3,
            decision_wait: Duration::from_millis(300),
            decision_deadline: Duration::from_secs(2),
            check_question: String::new(),
        }
    }

    /// The check question the agent asks when the decision model is unsure.
    #[must_use]
    pub fn with_check_question(self, question: impl Into<String>) -> Self {
        Self {
            check_question: question.into(),
            ..self
        }
    }

    /// The decision model opts out at this probability of yes or more.
    #[must_use]
    pub fn with_decision_threshold(self, threshold: f64) -> Self {
        Self {
            decision_threshold: threshold,
            ..self
        }
    }

    /// How long the agent's reply waits for the opt-out answer before it plays anyway.
    /// An answer that comes later still ends the session.
    #[must_use]
    pub fn with_decision_wait(self, wait: Duration) -> Self {
        Self {
            decision_wait: wait,
            ..self
        }
    }

    /// How long the decision model may take before the call counts as timed out.
    #[must_use]
    pub fn with_decision_deadline(self, deadline: Duration) -> Self {
        Self {
            decision_deadline: deadline,
            ..self
        }
    }
}

/// How long each model call may take before it counts as failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageDeadlines {
    /// STT: from the end of the caller's turn to its transcript.
    pub stt: Duration,
    /// LLM: from the request to the first token.
    pub llm_first_token: Duration,
    /// TTS: from a clause's request to its audio.
    pub tts: Duration,
}

impl Default for StageDeadlines {
    /// Generous enough for a local model's first call; production tightens them.
    fn default() -> Self {
        Self {
            stt: Duration::from_secs(10),
            llm_first_token: Duration::from_secs(10),
            tts: Duration::from_secs(10),
        }
    }
}

/// Told about every reply's stage timings as soon as its first audio is out: the place
/// services record latency histograms.
pub trait SessionObserver: Send + Sync {
    fn reply_latencies(&self, latencies: &StageLatencies);

    /// How long an opt-out decision took, from the caller's transcript to its answer,
    /// timeout or failure. It runs beside the LLM; while it is out, the reply waits (at most
    /// the policy's decision wait), which `mouth_to_ear` includes.
    fn decision_latency(&self, latency: Duration) {
        let _ = latency;
    }
}

/// One session's configuration.
pub struct SessionConfig {
    pub(crate) session_id: String,
    pub(crate) disclosure: String,
    pub(crate) system_prompt: String,
    pub(crate) voice: String,
    pub(crate) turn_policy: TurnPolicy,
    pub(crate) opt_out: OptOutPolicy,
    pub(crate) vad: Option<Box<dyn Vad>>,
    pub(crate) deadlines: StageDeadlines,
    pub(crate) max_duration: Duration,
    pub(crate) observer: Option<Arc<dyn SessionObserver>>,
}

impl SessionConfig {
    /// `session_id` names the session on the wire. `disclosure` is the locked opening line
    /// (AI disclosure, recording notice): the agent speaks it word for word before anything
    /// else, and the caller can't interrupt it.
    #[must_use]
    pub fn new(session_id: impl Into<String>, disclosure: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            disclosure: disclosure.into(),
            system_prompt: String::new(),
            voice: "af_heart".to_owned(),
            turn_policy: TurnPolicy::new(Duration::from_millis(500)),
            opt_out: OptOutPolicy::new(
                vak_domain::DEFAULT_OPT_OUT_PHRASES,
                vak_domain::DEFAULT_OPT_OUT_CONFIRMATION,
            ),
            vad: None,
            deadlines: StageDeadlines::default(),
            max_duration: Duration::from_mins(30),
            observer: None,
        }
    }

    /// The agent's instructions for the LLM.
    #[must_use]
    pub fn with_system_prompt(self, system_prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: system_prompt.into(),
            ..self
        }
    }

    /// The TTS voice, a catalog voice ID (default `af_heart`).
    #[must_use]
    pub fn with_voice(self, voice: impl Into<String>) -> Self {
        Self {
            voice: voice.into(),
            ..self
        }
    }

    #[must_use]
    pub fn with_turn_policy(self, turn_policy: TurnPolicy) -> Self {
        Self {
            turn_policy,
            ..self
        }
    }

    #[must_use]
    pub fn with_opt_out(self, opt_out: OptOutPolicy) -> Self {
        Self { opt_out, ..self }
    }

    /// The voice activity detector for this session's microphone audio (default:
    /// [`EnergyVad`]).
    #[must_use]
    pub fn with_vad(self, vad: Box<dyn Vad>) -> Self {
        Self {
            vad: Some(vad),
            ..self
        }
    }

    #[must_use]
    pub fn with_deadlines(self, deadlines: StageDeadlines) -> Self {
        Self { deadlines, ..self }
    }

    /// The session ends with reason `timeout` after this long (default 30 minutes).
    #[must_use]
    pub fn with_max_duration(self, max_duration: Duration) -> Self {
        Self {
            max_duration,
            ..self
        }
    }

    #[must_use]
    pub fn with_observer(self, observer: Arc<dyn SessionObserver>) -> Self {
        Self {
            observer: Some(observer),
            ..self
        }
    }

    pub(crate) fn take_vad(&mut self) -> Box<dyn Vad> {
        self.vad
            .take()
            .unwrap_or_else(|| Box::new(EnergyVad::default()))
    }
}
