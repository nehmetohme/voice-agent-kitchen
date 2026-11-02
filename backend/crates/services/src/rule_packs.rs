//! The rule packs the product ships: `contracts/rule-packs/<id>.v<version>.toml`, embedded at
//! build time and checked by the domain when they load.

use vak_domain::{RulePack, RulePackCatalog, RulePackError};

/// Why a shipped rule pack didn't load.
#[derive(Debug, thiserror::Error)]
pub enum RulePackLoadError {
    #[error("{file}: not a rule pack file")]
    Malformed { file: String },
    #[error("{file}: {source}")]
    Invalid {
        file: String,
        #[source]
        source: RulePackError,
    },
}

/// Parses one rule pack file. Its name must be `<id>.v<version>.toml`, matching its content.
pub fn parse(file_name: &str, text: &str) -> Result<RulePack, RulePackLoadError> {
    // CONTRACT-STUB
    drop(text.to_owned());
    Err(RulePackLoadError::Malformed {
        file: file_name.to_owned(),
    })
}

/// Every shipped rule pack.
pub fn shipped() -> Result<RulePackCatalog, RulePackLoadError> {
    // CONTRACT-STUB
    Err(RulePackLoadError::Malformed {
        file: String::new(),
    })
}
