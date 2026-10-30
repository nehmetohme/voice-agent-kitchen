//! Runtime settings of an agent: which model profile answers and how turns are detected.
//! Every setting has a default, so creating an agent never asks for them (Tesler).

/// Which LLM profile answers. The deployment maps a profile to a serving endpoint, so the
/// agent never names a vendor or a model file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ModelProfile {
    /// The main voice model: the best answers within the latency budget. The default.
    #[default]
    Balanced,
    /// A smaller model: faster first words, simpler answers.
    Fast,
}

/// The input is not a [`ModelProfile`] name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ModelProfileError {
    #[error("unknown model profile; expected balanced or fast")]
    Unknown,
}

impl ModelProfile {
    /// Every profile, the default first.
    pub const ALL: [Self; 2] = [Self::Balanced, Self::Fast];

    /// The wire and storage name, e.g. `balanced`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Fast => "fast",
        }
    }

    /// Accepts a name in any case, with surrounding whitespace.
    pub fn parse(raw: &str) -> Result<Self, ModelProfileError> {
        // CONTRACT-STUB
        let _ = raw;
        Err(ModelProfileError::Unknown)
    }
}

/// When the caller has finished speaking, and whether they may interrupt the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TurnDetection {
    silence_ms: u32,
    barge_in: bool,
}

/// The turn-detection values are out of range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TurnDetectionError {
    #[error("end-of-turn silence must be between {min} and {max} ms")]
    SilenceOutOfRange { min: u32, max: u32 },
}

impl TurnDetection {
    /// Shortest end-of-turn silence: below it the agent talks over people who pause.
    pub const MIN_SILENCE_MS: u32 = 200;
    /// Longest end-of-turn silence: above it the agent feels slow.
    pub const MAX_SILENCE_MS: u32 = 2_000;
    /// Default end-of-turn silence.
    pub const DEFAULT_SILENCE_MS: u32 = 500;

    /// `silence_ms`: how long the caller is silent before the agent answers.
    /// `barge_in`: whether the caller can interrupt the agent.
    pub fn new(silence_ms: u32, barge_in: bool) -> Result<Self, TurnDetectionError> {
        // CONTRACT-STUB
        Ok(Self {
            silence_ms,
            barge_in,
        })
    }

    #[must_use]
    pub fn silence_ms(&self) -> u32 {
        self.silence_ms
    }

    #[must_use]
    pub fn barge_in(&self) -> bool {
        self.barge_in
    }
}

impl Default for TurnDetection {
    /// 500 ms of silence ends a turn, and callers can interrupt.
    fn default() -> Self {
        // CONTRACT-STUB
        Self {
            silence_ms: 0,
            barge_in: false,
        }
    }
}

/// An agent's runtime settings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct AgentSettings {
    model: ModelProfile,
    turn_detection: TurnDetection,
}

impl AgentSettings {
    #[must_use]
    pub fn new(model: ModelProfile, turn_detection: TurnDetection) -> Self {
        Self {
            model,
            turn_detection,
        }
    }

    #[must_use]
    pub fn model(&self) -> ModelProfile {
        self.model
    }

    #[must_use]
    pub fn turn_detection(&self) -> TurnDetection {
        self.turn_detection
    }
}
