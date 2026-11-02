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

/// The default language.
const EN_US: LanguageInfo = LanguageInfo {
    code: "en-US",
    name: "English (United States)",
};

/// Every language, the default first.
pub const LANGUAGES: &[LanguageInfo] = &[
    EN_US,
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
        let raw = raw.trim();
        VOICES
            .iter()
            .find(|voice| voice.id.eq_ignore_ascii_case(raw))
            .map(Self)
            .ok_or(VoiceIdError::Unknown)
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
        let tag = raw.trim().replace('_', "-");
        LANGUAGES
            .iter()
            .find(|language| language.code.eq_ignore_ascii_case(&tag))
            .map(Self)
            .ok_or(LanguageError::Unsupported)
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
        Self(&EN_US)
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn ids_and_codes_are_unique() {
        let voices: HashSet<&str> = VOICES.iter().map(|voice| voice.id).collect();
        let languages: HashSet<&str> = LANGUAGES.iter().map(|language| language.code).collect();

        assert_eq!(voices.len(), VOICES.len());
        assert_eq!(languages.len(), LANGUAGES.len());
    }

    #[test]
    fn parsed_values_expose_their_catalog_entry() {
        let voice = VoiceId::parse("bm_george").expect("listed");
        let language = Language::parse("EN-gb").expect("listed");

        assert_eq!(voice.info().name, "George");
        assert_eq!(voice.to_string(), "bm_george");
        assert_eq!(language.info().name, "English (United Kingdom)");
        assert_eq!(language.to_string(), "en-GB");
        assert_eq!(Language::default().info(), &EN_US);
    }
}
