//! Catalog use cases: what operators can choose from when they set up an agent.

use vak_domain::catalog::{LANGUAGES, LanguageInfo, VOICES, VoiceInfo};

/// Every voice an agent can use, recommended first.
#[must_use]
pub fn voices() -> &'static [VoiceInfo] {
    VOICES
}

/// Every language an agent can speak, the default first.
#[must_use]
pub fn languages() -> &'static [LanguageInfo] {
    LANGUAGES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_the_domain_catalog() {
        assert_eq!(voices().first().map(|voice| voice.id), Some("af_heart"));
        assert_eq!(
            languages().first().map(|language| language.code),
            Some("en-US")
        );
    }
}
