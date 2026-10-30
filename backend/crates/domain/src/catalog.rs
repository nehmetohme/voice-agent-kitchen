//! The voices and languages an agent can use. A static list for now: the Kokoro voices the
//! TTS endpoints serve, and the languages the STT and TTS models cover.

use std::fmt;

/// A voice the TTS model can speak with.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct VoiceInfo {
    /// The TTS model's voice ID, e.g. `af_heart`.
    pub id: &'static str,
    /// Friendly name shown to operators.
    pub name: &'static str,
    /// Accent and gender, e.g. `American English, female`.
    pub description: &'static str,
    /// The language tag the voice speaks (one of [`LANGUAGES`]).
    pub language: &'static str,
}

/// A language an agent can speak and understand.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct LanguageInfo {
    /// BCP 47 tag, e.g. `en-US`.
    pub code: &'static str,
    /// Friendly name shown to operators.
    pub name: &'static str,
}

/// Every voice, recommended first. Seven at most, so a plain picker stays short (Hick).
pub const VOICES: &[VoiceInfo] = &[
    VoiceInfo {
        id: "af_heart",
        name: "Heart",
        description: "American English, female",
        language: "en-US",
    },
    VoiceInfo {
        id: "af_bella",
        name: "Bella",
        description: "American English, female",
        language: "en-US",
    },
    VoiceInfo {
        id: "am_michael",
        name: "Michael",
        description: "American English, male",
        language: "en-US",
    },
    VoiceInfo {
        id: "am_fenrir",
        name: "Fenrir",
        description: "American English, male",
        language: "en-US",
    },
    VoiceInfo {
        id: "bf_emma",
        name: "Emma",
        description: "British English, female",
        language: "en-GB",
    },
    VoiceInfo {
        id: "bm_george",
        name: "George",
        description: "British English, male",
        language: "en-GB",
    },
];

/// Every language, the default first.
pub const LANGUAGES: &[LanguageInfo] = &[
    LanguageInfo {
        code: "en-US",
        name: "English (United States)",
    },
    LanguageInfo {
        code: "en-GB",
        name: "English (United Kingdom)",
    },
];

/// A voice from [`VOICES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoiceId(&'static VoiceInfo);

/// The input names no voice in [`VOICES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum VoiceIdError {
    #[error("unknown voice")]
    Unknown,
}

impl VoiceId {
    /// Accepts a voice ID in any case, with surrounding whitespace.
    pub fn parse(raw: &str) -> Result<Self, VoiceIdError> {
        // CONTRACT-STUB
        let _ = raw;
        Err(VoiceIdError::Unknown)
    }

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        self.0.id
    }

    #[must_use]
    pub fn info(&self) -> &'static VoiceInfo {
        self.0
    }
}

impl fmt::Display for VoiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A language from [`LANGUAGES`]. Defaults to `en-US`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Language(&'static LanguageInfo);

/// The input names no language in [`LANGUAGES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LanguageError {
    #[error("unsupported language; expected one of en-US, en-GB")]
    Unsupported,
}

impl Language {
    /// Accepts a tag in any case, with `_` or `-` (Postel): `en_us`, `EN-us` and `en-US`
    /// are the same language.
    pub fn parse(raw: &str) -> Result<Self, LanguageError> {
        // CONTRACT-STUB
        let _ = raw;
        Err(LanguageError::Unsupported)
    }

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        self.0.code
    }

    #[must_use]
    pub fn info(&self) -> &'static LanguageInfo {
        self.0
    }
}

impl Default for Language {
    fn default() -> Self {
        // CONTRACT-STUB
        Self(&LANGUAGES[1])
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
