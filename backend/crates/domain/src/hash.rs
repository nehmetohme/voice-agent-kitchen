//! Content hashes: `sha256:` plus 64 lowercase hex digits. A draft's hash names exactly what
//! was tested and what was published.

use std::fmt;

use sha2::{Digest as _, Sha256};

/// A SHA-256 content hash, `sha256:<64 lowercase hex digits>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentHash(String);

/// The text is not a content hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ContentHashError {
    #[error("not a content hash; expected sha256: and 64 lowercase hex digits")]
    Malformed,
}

const PREFIX: &str = "sha256:";
const HEX: &[u8; 16] = b"0123456789abcdef";

impl ContentHash {
    /// The hash of `bytes`.
    pub(crate) fn of(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut text = String::with_capacity(PREFIX.len() + 2 * digest.len());
        text.push_str(PREFIX);
        for byte in &digest {
            text.push(char::from(HEX[usize::from(byte >> 4)]));
            text.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        Self(text)
    }

    /// A hash read back from storage.
    pub fn parse(raw: &str) -> Result<Self, ContentHashError> {
        let hex = raw
            .strip_prefix(PREFIX)
            .ok_or(ContentHashError::Malformed)?;
        let lower_hex = |c: char| c.is_ascii_digit() || ('a'..='f').contains(&c);
        if hex.len() == 64 && hex.chars().all(lower_hex) {
            Ok(Self(raw.to_owned()))
        } else {
            Err(ContentHashError::Malformed)
        }
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

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn is_sha256_in_lowercase_hex() {
        // FIPS 180-4 example: SHA-256("abc").
        assert_eq!(
            ContentHash::of(b"abc").as_str(),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn parse_accepts_only_the_canonical_form() {
        let hash = ContentHash::of(b"draft");

        assert_eq!(ContentHash::parse(hash.as_str()), Ok(hash.clone()));
        assert_eq!(hash.to_string(), hash.as_str());
        let upper = hash.as_str().to_uppercase().replace("SHA256:", PREFIX);
        for bad in [
            "",
            "sha256:",
            "md5:abc",
            upper.as_str(),
            &hash.as_str()[1..],
        ] {
            assert_eq!(
                ContentHash::parse(bad),
                Err(ContentHashError::Malformed),
                "{bad}"
            );
        }
    }

    proptest! {
        #[test]
        fn every_hash_parses_back(message in proptest::collection::vec(any::<u8>(), 0..300)) {
            let hash = ContentHash::of(&message);
            prop_assert_eq!(ContentHash::parse(hash.as_str()), Ok(hash.clone()));
            prop_assert_eq!(hash.as_str().len(), 71);
        }
    }
}
