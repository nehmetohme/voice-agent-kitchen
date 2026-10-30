//! Content hashes: `sha256:` plus 64 lowercase hex digits. A draft's hash names exactly what
//! was tested and what was published.

use std::fmt;

/// A SHA-256 content hash, `sha256:<64 lowercase hex digits>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentHash(String);

/// The text is not a content hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ContentHashError {
    #[error("not a content hash; expected sha256: and 64 lowercase hex digits")]
    Malformed,
}

impl ContentHash {
    /// The hash of `bytes`.
    pub(crate) fn of(bytes: &[u8]) -> Self {
        // CONTRACT-STUB
        let _ = bytes;
        Self(String::new())
    }

    /// A hash read back from storage.
    pub fn parse(raw: &str) -> Result<Self, ContentHashError> {
        // CONTRACT-STUB
        Ok(Self(raw.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
