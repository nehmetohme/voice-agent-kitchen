//! The rule packs the product ships: `contracts/rule-packs/<id>.v<version>.toml`, embedded at
//! build time and checked by the domain when they load. A published file never changes; a
//! new version is a new file, added to [`FILES`].

use jiff::civil::Date;
use serde::Deserialize;
use vak_domain::{
    Jurisdiction, LockedText, Retention, RulePack, RulePackCatalog, RulePackError, RulePackSpec,
    UseCase,
};

/// `(name, content)` of a file in `contracts/rule-packs/`.
macro_rules! pack {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!("../../../../contracts/rule-packs/", $name)),
        )
    };
}

/// Every shipped pack file: `(file name, content)`. The `plan` contract checks that this
/// list holds exactly the files in `contracts/rule-packs/`.
const FILES: &[(&str, &str)] = &[
    pack!("us-booking.v1.toml"),
    pack!("us-ca-booking.v1.toml"),
    pack!("us-ca-custom.v1.toml"),
    pack!("us-ca-service.v1.toml"),
    pack!("us-ca-support.v1.toml"),
    pack!("us-ca-survey.v1.toml"),
    pack!("us-ca-telemarketing.v1.toml"),
    pack!("us-custom.v1.toml"),
    pack!("us-service.v1.toml"),
    pack!("us-support.v1.toml"),
    pack!("us-survey.v1.toml"),
    pack!("us-telemarketing.v1.toml"),
];

/// Why a shipped rule pack didn't load.
#[derive(Debug, thiserror::Error)]
pub enum RulePackLoadError {
    #[error("{file}: not a rule pack file")]
    Malformed {
        file: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("{file}: `{field}` is not valid")]
    Field { file: String, field: &'static str },
    #[error("{file}: the file name must be <id>.v<version>.toml, from its content")]
    FileName { file: String },
    #[error("{file}: {source}")]
    Invalid {
        file: String,
        #[source]
        source: RulePackError,
    },
}

/// One pack file, as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackFile {
    id: String,
    version: u32,
    jurisdiction: String,
    use_case: String,
    effective: toml::value::Datetime,
    opt_out_phrases: Vec<String>,
    opt_out_confirmation: String,
    forbidden_claims: Vec<String>,
    retention: RetentionFile,
    locked: Vec<LockedFile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetentionFile {
    #[serde(rename = "recordings_days")]
    recordings: u32,
    #[serde(rename = "transcripts_days")]
    transcripts: u32,
    #[serde(rename = "audit_days")]
    audit: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LockedFile {
    key: String,
    text: String,
}

/// A TOML local date (`2026-11-01`): no time, no offset.
fn local_date(value: toml::value::Datetime) -> Option<Date> {
    let date = value
        .date
        .filter(|_| value.time.is_none() && value.offset.is_none())?;
    Date::new(
        i16::try_from(date.year).ok()?,
        i8::try_from(date.month).ok()?,
        i8::try_from(date.day).ok()?,
    )
    .ok()
}

/// Parses one rule pack file. Its name must be `<id>.v<version>.toml`, matching its content.
pub fn parse(file_name: &str, text: &str) -> Result<RulePack, RulePackLoadError> {
    let file = || file_name.to_owned();
    let field = |field| RulePackLoadError::Field {
        file: file(),
        field,
    };
    let raw: PackFile = toml::from_str(text).map_err(|source| RulePackLoadError::Malformed {
        file: file(),
        source,
    })?;
    if file_name != format!("{}.v{}.toml", raw.id, raw.version) {
        return Err(RulePackLoadError::FileName { file: file() });
    }
    let invalid = |source| RulePackLoadError::Invalid {
        file: file(),
        source,
    };
    let locked = raw
        .locked
        .iter()
        .map(|locked| LockedText::new(&locked.key, &locked.text))
        .collect::<Result<Vec<_>, _>>()
        .map_err(invalid)?;
    let retention = Retention::new(
        raw.retention.recordings,
        raw.retention.transcripts,
        raw.retention.audit,
    )
    .map_err(invalid)?;
    let spec = RulePackSpec {
        jurisdiction: Jurisdiction::parse(&raw.jurisdiction).map_err(|_| field("jurisdiction"))?,
        use_case: UseCase::parse(&raw.use_case).map_err(|_| field("use_case"))?,
        effective: local_date(raw.effective).ok_or_else(|| field("effective"))?,
        id: raw.id,
        version: raw.version,
        locked,
        opt_out_phrases: raw.opt_out_phrases,
        opt_out_confirmation: raw.opt_out_confirmation,
        forbidden_claims: raw.forbidden_claims,
        retention,
    };
    RulePack::new(spec).map_err(invalid)
}

/// Every shipped rule pack. `vak-server` loads them at startup and refuses to start when one
/// doesn't load.
pub fn shipped() -> Result<RulePackCatalog, RulePackLoadError> {
    let packs = FILES
        .iter()
        .map(|(name, text)| parse(name, text))
        .collect::<Result<Vec<_>, _>>()?;
    RulePackCatalog::new(packs).map_err(|source| RulePackLoadError::Invalid {
        file: "contracts/rule-packs".to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn us_support() -> &'static str {
        FILES
            .iter()
            .find(|(name, _)| *name == "us-support.v1.toml")
            .map(|(_, text)| *text)
            .expect("us-support ships")
    }

    #[test]
    fn each_kind_of_problem_names_the_file() {
        let text = us_support();
        let cases = [
            (
                parse("us-support.v1.toml", "id = "),
                "us-support.v1.toml: not a rule pack file",
            ),
            (
                parse("other.v1.toml", text),
                "other.v1.toml: the file name must be <id>.v<version>.toml, from its content",
            ),
            (
                parse(
                    "us-support.v1.toml",
                    &text.replace("\"US\"", "\"not a place\""),
                ),
                "us-support.v1.toml: `jurisdiction` is not valid",
            ),
            (
                parse(
                    "us-support.v1.toml",
                    &text.replace("\"support\"", "\"sales\""),
                ),
                "us-support.v1.toml: `use_case` is not valid",
            ),
            (
                parse(
                    "us-support.v1.toml",
                    &text.replace("2026-11-01", "2026-11-01T10:00:00Z"),
                ),
                "us-support.v1.toml: `effective` is not valid",
            ),
            (
                parse(
                    "us-support.v1.toml",
                    &text.replace("audit_days = 1826", "audit_days = 0"),
                ),
                "us-support.v1.toml: every retention period is at least one day",
            ),
        ];
        for (result, message) in cases {
            assert_eq!(
                result.map_err(|error| error.to_string()),
                Err(message.to_owned())
            );
        }
    }

    #[test]
    fn the_effective_date_is_a_local_date() {
        let date = |raw: &str| {
            raw.parse::<toml::value::Datetime>()
                .ok()
                .and_then(local_date)
        };

        assert_eq!(date("2026-11-01"), Date::new(2026, 11, 1).ok());
        assert_eq!(date("2026-11-01T10:00:00"), None);
        assert_eq!(date("10:00:00"), None);
    }
}
