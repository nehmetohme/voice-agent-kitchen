//! Compliance is tighten-only: a purpose can add opt-out phrases and forbidden claims and
//! shorten recording and transcript retention, never weaken its rule pack.

use crate::{LockedText, Retention, RulePack, RulePackRef};

/// Why compliance additions are refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ComplianceError {
    #[error("a phrase or claim can't be empty")]
    EmptyItem,
    #[error("an opt-out phrase is at most {max} characters, a claim at most 200")]
    ItemTooLong { max: usize },
    #[error("a list holds at most {max} items")]
    TooManyItems { max: usize },
    #[error("a retention cap is 1 to 36500 days")]
    InvalidRetention,
}

/// The parts of [`ComplianceAdditions`], before [`ComplianceAdditions::new`] checks them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComplianceAdditionsSpec {
    pub opt_out_phrases: Vec<String>,
    pub forbidden_claims: Vec<String>,
    /// Keep recordings at most this many days; `None` keeps the pack's period.
    pub recordings_days: Option<u32>,
    /// Keep transcripts at most this many days; `None` keeps the pack's period.
    pub transcripts_days: Option<u32>,
}

/// What a purpose adds to its rule pack. There is no field for the audit period: a purpose
/// can't shorten it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ComplianceAdditions {
    opt_out_phrases: Vec<String>,
    forbidden_claims: Vec<String>,
    recordings_days: Option<u32>,
    transcripts_days: Option<u32>,
}

impl ComplianceAdditions {
    /// Most extra phrases, and most extra claims.
    pub const MAX_ITEMS: usize = 20;
    /// Longest opt-out phrase.
    pub const MAX_PHRASE: usize = 100;
    /// Longest claim.
    pub const MAX_CLAIM: usize = 200;
    /// Longest retention cap (100 years).
    pub const MAX_DAYS: u32 = 36_500;

    /// Phrases are trimmed and lower-cased, claims trimmed; repeats are dropped. Caps are
    /// 1 to 36,500 days.
    pub fn new(spec: ComplianceAdditionsSpec) -> Result<Self, ComplianceError> {
        Ok(Self {
            opt_out_phrases: spec.opt_out_phrases,
            forbidden_claims: spec.forbidden_claims,
            recordings_days: spec.recordings_days,
            transcripts_days: spec.transcripts_days,
        })
    }

    #[must_use]
    pub fn opt_out_phrases(&self) -> &[String] {
        &self.opt_out_phrases
    }

    #[must_use]
    pub fn forbidden_claims(&self) -> &[String] {
        &self.forbidden_claims
    }

    #[must_use]
    pub fn recordings_days(&self) -> Option<u32> {
        self.recordings_days
    }

    #[must_use]
    pub fn transcripts_days(&self) -> Option<u32> {
        self.transcripts_days
    }
}

/// The policy that applies to a call: the pack, tightened by the purpose's additions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveCompliance {
    rule_pack: RulePackRef,
    locked: Vec<LockedText>,
    opt_out_phrases: Vec<String>,
    opt_out_confirmation: String,
    forbidden_claims: Vec<String>,
    retention: Retention,
}

impl EffectiveCompliance {
    /// `pack` with `additions`: the pack's locked texts, confirmation and audit period as
    /// they are; its phrases and claims, then the extra ones; recordings and transcripts kept
    /// for the shorter of the pack's period and the cap.
    #[must_use]
    pub fn of(pack: &RulePack, additions: &ComplianceAdditions) -> Self {
        let _ = additions;
        Self {
            rule_pack: pack.reference().clone(),
            locked: Vec::new(),
            opt_out_phrases: Vec::new(),
            opt_out_confirmation: String::new(),
            forbidden_claims: Vec::new(),
            retention: pack.retention(),
        }
    }

    #[must_use]
    pub fn rule_pack(&self) -> &RulePackRef {
        &self.rule_pack
    }

    #[must_use]
    pub fn locked_texts(&self) -> &[LockedText] {
        &self.locked
    }

    #[must_use]
    pub fn opt_out_phrases(&self) -> &[String] {
        &self.opt_out_phrases
    }

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
