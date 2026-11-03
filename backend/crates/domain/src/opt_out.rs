//! The deterministic opt-out layer: a phrase list matched against what the caller said.
//! The decision model and the LLM tool are the other two layers (`vak-pipeline`); any one of
//! the three ends the session.

/// Opt-out phrases, until rule packs (M3) supply them per jurisdiction and use case.
pub const DEFAULT_OPT_OUT_PHRASES: [&str; 9] = [
    "stop calling",
    "don't call",
    "do not call",
    "never call",
    "take me off",
    "remove me from",
    "unsubscribe",
    "opt out",
    "do not contact",
];

/// What the agent says before it ends a session on an opt-out, until rule packs supply it.
pub const DEFAULT_OPT_OUT_CONFIRMATION: &str = "Understood. You won't be contacted again. Goodbye.";

/// Phrases that end a session when the caller says one. Matching ignores case and
/// punctuation and respects word boundaries: "stop calling" matches "Please, STOP calling!"
/// but not "nonstop callings".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OptOutPhrases {
    phrases: Vec<String>,
}

impl OptOutPhrases {
    /// Blank phrases are dropped.
    #[must_use]
    pub fn new(phrases: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        Self {
            phrases: phrases
                .into_iter()
                .map(|phrase| normalize(phrase.as_ref()))
                .filter(|phrase| !phrase.is_empty())
                .collect(),
        }
    }

    /// Whether `text` contains one of the phrases.
    #[must_use]
    pub fn matches(&self, text: &str) -> bool {
        let text = format!(" {} ", normalize(text));
        self.phrases
            .iter()
            .any(|phrase| text.contains(&format!(" {phrase} ")))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.phrases.is_empty()
    }
}

/// Lowercase words separated by single spaces. Apostrophes are dropped inside words
/// ("don't" becomes "dont"), every other non-alphanumeric character separates words.
fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| *c != '\'' && *c != '\u{2019}')
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrases_match_whole_words_in_any_case_and_punctuation() {
        let phrases = OptOutPhrases::new(["stop calling", "take me off your list"]);
        assert!(phrases.matches("Please, STOP calling me!"));
        assert!(phrases.matches("stop calling"));
        assert!(phrases.matches("Take me off your list, please."));
        assert!(phrases.matches("Stop   calling\nthis number."));
        assert!(!phrases.matches("nonstop callings"));
        assert!(!phrases.matches("I want to book a table."));
        assert!(!phrases.matches(""));
    }

    #[test]
    fn apostrophes_do_not_split_words() {
        let phrases = OptOutPhrases::new(["don't call"]);
        assert!(phrases.matches("Don't call me again."));
        assert!(phrases.matches("Don\u{2019}t call me again."));
        assert!(phrases.matches("dont call"));
        assert!(!phrases.matches("don t call"));
    }

    #[test]
    fn blank_phrases_never_match() {
        let phrases = OptOutPhrases::new(["", "  ", "!!"]);
        assert!(phrases.is_empty());
        assert!(!phrases.matches("anything at all"));
        assert!(OptOutPhrases::default().is_empty());
    }

    #[test]
    fn the_defaults_catch_common_requests() {
        let phrases = OptOutPhrases::new(DEFAULT_OPT_OUT_PHRASES);
        for request in [
            "Please stop calling me.",
            "Don't call this number again.",
            "Take me off your list.",
            "Remove me from your list.",
            "I want to opt out.",
        ] {
            assert!(phrases.matches(request), "{request}");
        }
        assert!(!phrases.matches("Can I book a table for two?"));
        assert!(DEFAULT_OPT_OUT_CONFIRMATION.ends_with("Goodbye."));
    }
}
