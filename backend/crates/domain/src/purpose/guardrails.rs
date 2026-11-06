//! A purpose's guardrails: what the agent may talk about, what it must never say or do, when
//! to escalate, and its limits. M3b compiles the text ones into the LLM instructions of test
//! calls; M4b checks them with the decision engine.

/// What to do when an escalation rule's question is answered yes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EscalationAction {
    /// Hand the call to a person.
    Transfer,
    /// End the call after saying `message` word for word.
    EndCall { message: String },
    /// Flag the call for review and carry on.
    Flag,
}

impl EscalationAction {
    /// The wire name: `transfer`, `end_call` or `flag`.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Transfer => "transfer",
            Self::EndCall { .. } => "end_call",
            Self::Flag => "flag",
        }
    }
}

/// When to escalate (a yes/no question for the decision engine) and what to do then.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EscalationRule {
    when: String,
    action: EscalationAction,
}

impl EscalationRule {
    /// Longest question, in characters.
    pub const MAX_QUESTION: usize = 200;
    /// Longest end-of-call message, in characters.
    pub const MAX_MESSAGE: usize = 500;

    /// `when` is a yes/no question, 1–200 characters; an end-of-call message is 1–500. Both
    /// are trimmed.
    pub fn new(when: &str, action: EscalationAction) -> Result<Self, GuardrailError> {
        Ok(Self {
            when: when.trim().to_owned(),
            action,
        })
    }

    #[must_use]
    pub fn when(&self) -> &str {
        &self.when
    }

    #[must_use]
    pub fn action(&self) -> &EscalationAction {
        &self.action
    }
}

/// Personal data the agent must never ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PersonalData {
    /// Payment card numbers.
    CardNumber,
    /// Social security and other national ID numbers.
    NationalId,
    /// Passwords and PINs.
    Password,
    /// Bank account numbers.
    BankAccount,
    /// Health and medical details.
    Health,
}

impl PersonalData {
    pub const ALL: [Self; 5] = [
        Self::CardNumber,
        Self::NationalId,
        Self::Password,
        Self::BankAccount,
        Self::Health,
    ];

    /// The wire name, e.g. `card_number`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CardNumber => "card_number",
            Self::NationalId => "national_id",
            Self::Password => "password",
            Self::BankAccount => "bank_account",
            Self::Health => "health",
        }
    }

    /// In a sentence: "payment card numbers".
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::CardNumber => "payment card numbers",
            Self::NationalId => "social security or other national ID numbers",
            Self::Password => "passwords or PINs",
            Self::BankAccount => "bank account numbers",
            Self::Health => "health or medical details",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, GuardrailError> {
        let raw = raw.trim();
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str().eq_ignore_ascii_case(raw))
            .ok_or(GuardrailError::UnknownPersonalData)
    }
}

/// Why guardrails are refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuardrailError {
    #[error("an item can't be empty")]
    EmptyItem,
    #[error("an item is longer than {max} characters")]
    ItemTooLong { max: usize },
    #[error("a list holds at most {max} items")]
    TooManyItems { max: usize },
    #[error("a purpose has at most {max} escalation rules")]
    TooManyEscalations { max: usize },
    #[error("an escalation needs a yes/no question of 1-200 characters")]
    InvalidQuestion,
    #[error("the end-of-call message must be 1-500 characters")]
    InvalidMessage,
    #[error("the call length must be {min} to {max} minutes")]
    CallLengthOutOfRange { min: u32, max: u32 },
    #[error("unknown kind of personal data")]
    UnknownPersonalData,
}

/// The parts of [`Guardrails`], before [`Guardrails::new`] checks them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardrailsSpec {
    /// The scope; empty means no limit.
    pub allowed_topics: Vec<String>,
    /// What the agent must never say or do.
    pub never: Vec<String>,
    pub escalations: Vec<EscalationRule>,
    /// 1 to 30.
    pub max_call_minutes: u32,
    pub never_ask: Vec<PersonalData>,
}

/// Checked guardrails.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Guardrails {
    allowed_topics: Vec<String>,
    never: Vec<String>,
    escalations: Vec<EscalationRule>,
    max_call_minutes: u32,
    never_ask: Vec<PersonalData>,
}

impl Default for Guardrails {
    /// No limits beyond the platform's: any topic, nothing added, 30-minute calls.
    fn default() -> Self {
        Self {
            allowed_topics: Vec::new(),
            never: Vec::new(),
            escalations: Vec::new(),
            max_call_minutes: Self::MAX_CALL_MINUTES,
            never_ask: Vec::new(),
        }
    }
}

impl Guardrails {
    /// Most topics, and most never-rules.
    pub const MAX_ITEMS: usize = 20;
    /// Longest topic or never-rule, in characters.
    pub const MAX_ITEM: usize = 200;
    /// Most escalation rules: each is a decision question, and none may block a reply.
    pub const MAX_ESCALATIONS: usize = 5;
    /// Shortest call limit.
    pub const MIN_CALL_MINUTES: u32 = 1;
    /// Longest call limit: the platform's cap.
    pub const MAX_CALL_MINUTES: u32 = 30;

    /// Topics and never-rules are trimmed, 1–200 characters, at most 20 each; repeats
    /// (ignoring case) are dropped. At most 5 escalation rules; a call limit of 1–30 minutes;
    /// each kind of personal data once.
    pub fn new(spec: GuardrailsSpec) -> Result<Self, GuardrailError> {
        Ok(Self {
            allowed_topics: spec.allowed_topics,
            never: spec.never,
            escalations: spec.escalations,
            max_call_minutes: spec.max_call_minutes,
            never_ask: spec.never_ask,
        })
    }

    #[must_use]
    pub fn allowed_topics(&self) -> &[String] {
        &self.allowed_topics
    }

    #[must_use]
    pub fn never(&self) -> &[String] {
        &self.never
    }

    #[must_use]
    pub fn escalations(&self) -> &[EscalationRule] {
        &self.escalations
    }

    #[must_use]
    pub fn max_call_minutes(&self) -> u32 {
        self.max_call_minutes
    }

    /// In [`PersonalData::ALL`] order.
    #[must_use]
    pub fn never_ask(&self) -> &[PersonalData] {
        &self.never_ask
    }
}
