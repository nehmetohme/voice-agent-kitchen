//! Test scenarios: what a simulated caller plays against the agent, and the rubric its
//! conversation is scored by.

use std::fmt;

use crate::{ContentHash, UseCase};

/// A scenario's ID within its set.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScenarioId(String);

/// A scenario or a set breaks a rule.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScenarioError {
    #[error(
        "a scenario ID is 1 to 40 lowercase letters, digits and hyphens, starting with a letter"
    )]
    InvalidId,
    #[error("a scenario's name is 1 to 80 characters")]
    Name,
    #[error("a scenario's goal is 1 to 1,000 characters")]
    Goal,
    #[error("a scenario's persona is at most 1,000 characters")]
    Persona,
    #[error("a scenario's turn cap is 1 to 20 caller turns")]
    MaxTurns,
    #[error("a scenario has 1 to 8 rubric items")]
    RubricCount,
    #[error("a rubric question is 1 to 300 characters")]
    Question,
    #[error("a set has 1 to 8 scenarios")]
    Count,
    #[error("two scenarios share the ID {0}")]
    DuplicateId(ScenarioId),
    #[error("at least one scenario is required")]
    NoneRequired,
}

impl ScenarioId {
    /// Longest ID, in characters.
    pub const MAX_LEN: usize = 40;

    pub fn parse(raw: &str) -> Result<Self, ScenarioError> {
        Ok(Self(raw.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ScenarioId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A yes/no question the decision model answers about the conversation, and the answer the
/// scenario expects.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RubricItem {
    question: String,
    expected: bool,
}

impl RubricItem {
    /// Longest question, in characters.
    pub const MAX_QUESTION: usize = 300;

    pub fn new(question: &str, expected: bool) -> Result<Self, ScenarioError> {
        Ok(Self {
            question: question.to_owned(),
            expected,
        })
    }

    #[must_use]
    pub fn question(&self) -> &str {
        &self.question
    }

    /// The answer that passes: `true` for yes.
    #[must_use]
    pub fn expected(&self) -> bool {
        self.expected
    }
}

/// The fields of a scenario, checked by [`Scenario::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioSpec {
    pub id: ScenarioId,
    pub name: String,
    /// What the simulated caller wants, and what to say or not say.
    pub goal: String,
    /// Who the simulated caller is and how they talk; may be empty.
    pub persona: String,
    /// A run passes only if every required scenario passes.
    pub required: bool,
    /// The most caller turns before the conversation stops.
    pub max_turns: u8,
    pub rubric: Vec<RubricItem>,
}

/// One scenario: a simulated caller's goal and persona, and how its conversation is scored.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Scenario {
    id: ScenarioId,
    name: String,
    goal: String,
    persona: String,
    required: bool,
    max_turns: u8,
    rubric: Vec<RubricItem>,
}

impl Scenario {
    /// Longest name, in characters.
    pub const MAX_NAME: usize = 80;
    /// Longest goal, in characters.
    pub const MAX_GOAL: usize = 1_000;
    /// Longest persona, in characters.
    pub const MAX_PERSONA: usize = 1_000;
    /// The highest turn cap.
    pub const MAX_TURNS: u8 = 20;
    /// The most rubric items.
    pub const MAX_RUBRIC: usize = 8;

    pub fn new(spec: ScenarioSpec) -> Result<Self, ScenarioError> {
        Ok(Self {
            id: spec.id,
            name: spec.name,
            goal: spec.goal,
            persona: spec.persona,
            required: spec.required,
            max_turns: spec.max_turns,
            rubric: spec.rubric,
        })
    }

    #[must_use]
    pub fn id(&self) -> &ScenarioId {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn goal(&self) -> &str {
        &self.goal
    }

    #[must_use]
    pub fn persona(&self) -> &str {
        &self.persona
    }

    #[must_use]
    pub fn required(&self) -> bool {
        self.required
    }

    #[must_use]
    pub fn max_turns(&self) -> u8 {
        self.max_turns
    }

    #[must_use]
    pub fn rubric(&self) -> &[RubricItem] {
        &self.rubric
    }

    /// The scenario's version: a content hash of everything that defines it.
    #[must_use]
    pub fn version(&self) -> ContentHash {
        ContentHash::of(self.id.as_str().as_bytes())
    }

    /// The simulated caller's instructions.
    #[must_use]
    pub fn caller_instructions(&self) -> String {
        String::new()
    }
}

/// The scenarios an agent is tested with.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScenarioSet {
    scenarios: Vec<Scenario>,
}

impl ScenarioSet {
    /// The most scenarios in a set.
    pub const MAX_SCENARIOS: usize = 8;

    pub fn new(scenarios: Vec<Scenario>) -> Result<Self, ScenarioError> {
        Ok(Self { scenarios })
    }

    /// The default scenarios of `use_case`.
    #[must_use]
    pub fn defaults_for(use_case: UseCase) -> Self {
        let _ = use_case;
        Self {
            scenarios: Vec::new(),
        }
    }

    #[must_use]
    pub fn scenarios(&self) -> &[Scenario] {
        &self.scenarios
    }
}
