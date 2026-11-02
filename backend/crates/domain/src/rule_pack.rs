//! Compliance rule packs: versioned data per use case and jurisdiction. A draft pins one
//! (`RulePackRef`); the catalog resolves the pinned version; plans are checked against it.

use jiff::civil::Date;

use crate::{Jurisdiction, Plan, PlanError, UseCase};

/// A pinned rule pack: `id` names the use case and jurisdiction variant (`us-support`),
/// `version` is the pack version (1 or more).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RulePackRef {
    id: String,
    version: u32,
}

/// The stored reference is not well formed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RulePackRefError {
    #[error("rule pack id must be 1-64 lowercase letters, digits or '-'")]
    InvalidId,
    #[error("rule pack version must be 1 or more")]
    InvalidVersion,
}

impl RulePackRef {
    /// Longest pack ID.
    const MAX_ID: usize = 64;

    /// The default pack for an agent: `<jurisdiction>-<use case>`, lower case, version 1.
    #[must_use]
    pub fn for_agent(use_case: UseCase, jurisdiction: &Jurisdiction) -> Self {
        Self {
            id: format!(
                "{}-{}",
                jurisdiction.as_str().to_ascii_lowercase(),
                use_case.as_str()
            ),
            version: 1,
        }
    }

    /// A reference read back from storage.
    pub fn new(id: &str, version: u32) -> Result<Self, RulePackRefError> {
        let valid_id = (1..=Self::MAX_ID).contains(&id.len())
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !valid_id {
            return Err(RulePackRefError::InvalidId);
        }
        if version == 0 {
            return Err(RulePackRefError::InvalidVersion);
        }
        Ok(Self {
            id: id.to_owned(),
            version,
        })
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }
}

/// Text the agent must say word for word, such as the AI disclosure. Keys are 1–32
/// lowercase letters, digits or `_`; the text is trimmed, not empty, and at most
/// [`LockedText::MAX`] characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LockedText {
    key: String,
    text: String,
}

impl LockedText {
    /// Says the caller is talking to an AI. Every pack has it.
    pub const AI_DISCLOSURE: &'static str = "ai_disclosure";
    /// Says the call is recorded and transcribed. Every pack has it.
    pub const RECORDING_NOTICE: &'static str = "recording_notice";
    /// Longest locked text, in characters. The opt-out confirmation has the same limit.
    pub const MAX: usize = 500;

    /// Longest key, in characters.
    const MAX_KEY: usize = 32;

    pub fn new(key: &str, text: &str) -> Result<Self, RulePackError> {
        let valid_key = (1..=Self::MAX_KEY).contains(&key.len())
            && key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        let text = spoken_text(text).ok_or(RulePackError::InvalidLockedText)?;
        if !valid_key {
            return Err(RulePackError::InvalidLockedText);
        }
        Ok(Self {
            key: key.to_owned(),
            text,
        })
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// How long call data is kept, in days. Each period is at least one day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Retention {
    recordings: u32,
    transcripts: u32,
    audit: u32,
}

impl Retention {
    pub fn new(
        recordings_days: u32,
        transcripts_days: u32,
        audit_days: u32,
    ) -> Result<Self, RulePackError> {
        if recordings_days == 0 || transcripts_days == 0 || audit_days == 0 {
            return Err(RulePackError::InvalidRetention);
        }
        Ok(Self {
            recordings: recordings_days,
            transcripts: transcripts_days,
            audit: audit_days,
        })
    }

    #[must_use]
    pub fn recordings_days(&self) -> u32 {
        self.recordings
    }

    #[must_use]
    pub fn transcripts_days(&self) -> u32 {
        self.transcripts
    }

    #[must_use]
    pub fn audit_days(&self) -> u32 {
        self.audit
    }
}

/// The parts of a rule pack, before [`RulePack::new`] checks them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulePackSpec {
    /// `<jurisdiction>-<use case>`, lower case, e.g. `us-ca-booking`.
    pub id: String,
    /// 1 or more. A published version never changes; a change is a new version.
    pub version: u32,
    pub jurisdiction: Jurisdiction,
    pub use_case: UseCase,
    /// The day the pack takes effect.
    pub effective: Date,
    /// What the start step opens with, in order.
    pub locked: Vec<LockedText>,
    /// Phrases that end the call when the caller says them.
    pub opt_out_phrases: Vec<String>,
    /// Said word for word when the caller opts out, before the call ends. Not part of the
    /// opening.
    pub opt_out_confirmation: String,
    /// What the agent must never claim, in plain words.
    pub forbidden_claims: Vec<String>,
    pub retention: Retention,
}

/// A checked rule pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulePack {
    reference: RulePackRef,
    jurisdiction: Jurisdiction,
    use_case: UseCase,
    effective: Date,
    locked: Vec<LockedText>,
    opt_out_phrases: Vec<String>,
    opt_out_confirmation: String,
    forbidden_claims: Vec<String>,
    retention: Retention,
}

/// A rule a rule pack, or a catalog of them, breaks.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RulePackError {
    #[error("the pack id must be {expected}, from its jurisdiction and use case")]
    IdMismatch { expected: String },
    #[error("a rule pack version must be 1 or more")]
    InvalidVersion,
    #[error("a locked text needs a key of lowercase letters, digits or '_', and some text")]
    InvalidLockedText,
    #[error("the pack has no {key} text")]
    MissingLockedText { key: String },
    #[error("the pack has two {key} texts")]
    DuplicateLockedText { key: String },
    #[error("the pack lists no opt-out phrases")]
    NoOptOutPhrases,
    #[error("the opt-out confirmation must be 1-500 characters")]
    InvalidOptOutConfirmation,
    #[error("phrases and claims can't be empty")]
    EmptyPhrase,
    #[error("every retention period is at least one day")]
    InvalidRetention,
    #[error("rule pack {id} v{version} is defined twice")]
    Duplicate { id: String, version: u32 },
}

/// Text the agent says word for word: trimmed, 1 to [`LockedText::MAX`] characters.
fn spoken_text(raw: &str) -> Option<String> {
    let text = raw.trim();
    (1..=LockedText::MAX)
        .contains(&text.chars().count())
        .then(|| text.to_owned())
}

/// Trimmed, non-empty phrases; `lower` also lower-cases them.
fn phrases(raw: Vec<String>, lower: bool) -> Result<Vec<String>, RulePackError> {
    raw.into_iter()
        .map(|phrase| {
            let phrase = phrase.trim();
            match (phrase.is_empty(), lower) {
                (true, _) => Err(RulePackError::EmptyPhrase),
                (false, true) => Ok(phrase.to_lowercase()),
                (false, false) => Ok(phrase.to_owned()),
            }
        })
        .collect()
}

/// The locked texts, if their keys are unique and include the two every pack needs.
fn checked_locked(locked: Vec<LockedText>) -> Result<Vec<LockedText>, RulePackError> {
    let mut keys = std::collections::BTreeSet::new();
    if let Some(twice) = locked.iter().find(|text| !keys.insert(text.key())) {
        return Err(RulePackError::DuplicateLockedText {
            key: twice.key().to_owned(),
        });
    }
    for required in [LockedText::AI_DISCLOSURE, LockedText::RECORDING_NOTICE] {
        if !keys.contains(required) {
            return Err(RulePackError::MissingLockedText {
                key: required.to_owned(),
            });
        }
    }
    Ok(locked)
}

impl RulePack {
    /// Checks `spec`: the ID follows the jurisdiction and use case, the version is 1 or more,
    /// the AI disclosure and the recording notice are there (each key once), there is at
    /// least one opt-out phrase (stored trimmed and lower case), and the opt-out
    /// confirmation is a spoken text like a locked one.
    pub fn new(spec: RulePackSpec) -> Result<Self, RulePackError> {
        let expected = RulePackRef::for_agent(spec.use_case, &spec.jurisdiction);
        if spec.id != expected.id() {
            return Err(RulePackError::IdMismatch {
                expected: expected.id().to_owned(),
            });
        }
        let reference = RulePackRef::new(&spec.id, spec.version).map_err(|error| match error {
            RulePackRefError::InvalidVersion => RulePackError::InvalidVersion,
            RulePackRefError::InvalidId => RulePackError::IdMismatch {
                expected: expected.id().to_owned(),
            },
        })?;
        let locked = checked_locked(spec.locked)?;
        if spec.opt_out_phrases.is_empty() {
            return Err(RulePackError::NoOptOutPhrases);
        }
        let opt_out_phrases = phrases(spec.opt_out_phrases, true)?;
        let opt_out_confirmation = spoken_text(&spec.opt_out_confirmation)
            .ok_or(RulePackError::InvalidOptOutConfirmation)?;
        Ok(Self {
            reference,
            jurisdiction: spec.jurisdiction,
            use_case: spec.use_case,
            effective: spec.effective,
            locked,
            opt_out_phrases,
            opt_out_confirmation,
            forbidden_claims: phrases(spec.forbidden_claims, false)?,
            retention: spec.retention,
        })
    }

    /// The pack's ID and version.
    #[must_use]
    pub fn reference(&self) -> &RulePackRef {
        &self.reference
    }

    #[must_use]
    pub fn jurisdiction(&self) -> &Jurisdiction {
        &self.jurisdiction
    }

    #[must_use]
    pub fn use_case(&self) -> UseCase {
        self.use_case
    }

    #[must_use]
    pub fn effective(&self) -> Date {
        self.effective
    }

    /// What the start step opens with, in order.
    #[must_use]
    pub fn locked_texts(&self) -> &[LockedText] {
        &self.locked
    }

    /// Trimmed and lower case.
    #[must_use]
    pub fn opt_out_phrases(&self) -> &[String] {
        &self.opt_out_phrases
    }

    /// What the agent says, word for word, when the caller opts out. Trimmed.
    #[must_use]
    pub fn opt_out_confirmation(&self) -> &str {
        &self.opt_out_confirmation
    }

    #[must_use]
    pub fn forbidden_claims(&self) -> &[String] {
        &self.forbidden_claims
    }

    #[must_use]
    pub fn retention(&self) -> Retention {
        self.retention
    }
}

/// Every known rule pack, each (id, version) once. Agents pin a version, and the pinned
/// version is the one that applies, even when a later one exists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RulePackCatalog {
    packs: Vec<RulePack>,
}

impl RulePackCatalog {
    /// Fails when two packs share an ID and version, even with the same content: a
    /// published version is defined once.
    pub fn new(mut packs: Vec<RulePack>) -> Result<Self, RulePackError> {
        packs.sort_by(|a, b| key_of(a.reference()).cmp(&key_of(b.reference())));
        if let Some(pair) = packs.windows(2).find(|pair| {
            pair.first().map(RulePack::reference) == pair.get(1).map(RulePack::reference)
        }) && let Some(twice) = pair.first()
        {
            return Err(RulePackError::Duplicate {
                id: twice.reference().id().to_owned(),
                version: twice.reference().version(),
            });
        }
        Ok(Self { packs })
    }

    /// Exactly the pinned version, if the catalog has it.
    #[must_use]
    pub fn get(&self, pack: &RulePackRef) -> Option<&RulePack> {
        self.packs
            .binary_search_by(|stored| key_of(stored.reference()).cmp(&key_of(pack)))
            .ok()
            .and_then(|index| self.packs.get(index))
    }

    /// The highest version of `id`.
    #[must_use]
    pub fn latest(&self, id: &str) -> Option<&RulePack> {
        self.packs
            .iter()
            .filter(|pack| pack.reference().id() == id)
            .max_by_key(|pack| pack.reference().version())
    }

    /// Every pack, by id and then version.
    pub fn iter(&self) -> impl Iterator<Item = &RulePack> {
        self.packs.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.packs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.packs.is_empty()
    }

    /// Validates `plan` against the pack `pack` names. A pack the catalog doesn't have makes
    /// the plan invalid: there is nothing to check its locked texts against.
    pub fn validate(&self, plan: &Plan, pack: &RulePackRef) -> Result<(), Vec<PlanError>> {
        match self.get(pack) {
            Some(rules) => plan.validate(rules),
            None => Err(vec![PlanError::RulePackUnavailable { pack: pack.clone() }]),
        }
    }
}

/// How the catalog orders packs: by ID, then version.
fn key_of(pack: &RulePackRef) -> (&str, u32) {
    (pack.id(), pack.version())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_pack_is_a_valid_reference() {
        let jurisdiction = Jurisdiction::parse("EU-FR").expect("valid");
        for use_case in UseCase::ALL {
            let pack = RulePackRef::for_agent(use_case, &jurisdiction);
            assert_eq!(RulePackRef::new(pack.id(), pack.version()), Ok(pack));
        }
    }

    #[test]
    fn stored_references_are_checked() {
        assert_eq!(RulePackRef::new("", 1), Err(RulePackRefError::InvalidId));
        assert_eq!(
            RulePackRef::new("US-support", 1),
            Err(RulePackRefError::InvalidId)
        );
        assert_eq!(
            RulePackRef::new(&"a".repeat(65), 1),
            Err(RulePackRefError::InvalidId)
        );
        assert_eq!(
            RulePackRef::new("us-support", 0),
            Err(RulePackRefError::InvalidVersion)
        );
        assert_eq!(
            RulePackRef::new("us-support", 3).map(|pack| pack.version()),
            Ok(3)
        );
    }
}
