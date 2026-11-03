//! `SensitiveText`: words someone said or will hear, which are personal data.

/// Text that is personal data: a transcript, a prompt, a reply. `Debug` shows only its
/// length, so it can't leak into logs by accident. Serializes as a plain string.
#[derive(Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct SensitiveText(String);

impl SensitiveText {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// The words. Call this only where they are needed: a request body, a prompt, the
    /// caller's own screen.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Debug for SensitiveText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SensitiveText([REDACTED; {} bytes])", self.0.len())
    }
}

impl From<String> for SensitiveText {
    fn from(text: String) -> Self {
        Self(text)
    }
}

impl From<&str> for SensitiveText {
    fn from(text: &str) -> Self {
        Self(text.to_owned())
    }
}
