//! Test scenarios: what a simulated caller plays against the agent, and the rubric its
//! conversation is scored by. A set of them belongs to the agent's draft; until the operator
//! saves one, the use case's defaults apply ([`ScenarioSet::defaults_for`]).

mod defaults;

use std::collections::BTreeSet;
use std::fmt;

use crate::{ContentHash, UseCase};

/// A scenario's ID within its set: 1 to 40 lowercase ASCII letters, digits and hyphens,
/// starting with a letter.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScenarioId(String);

/// A scenario or a set breaks a rule. Each variant names the field it is about, so the API can
/// locate it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScenarioError {
    #[error(
        "a scenario ID is 1 to 40 lowercase letters, digits and hyphens, starting with a letter"
    )]
    InvalidId,
    #[error("a scenario's name is 1 to 80 characters, without control characters")]
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

    /// Accepts the ID exactly as given: IDs are machine names, so nothing is trimmed.
    pub fn parse(raw: &str) -> Result<Self, ScenarioError> {
        let starts_with_letter = raw.bytes().next().is_some_and(|b| b.is_ascii_lowercase());
        let allowed = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-';
        if starts_with_letter && raw.len() <= Self::MAX_LEN && raw.bytes().all(allowed) {
            Ok(Self(raw.to_owned()))
        } else {
            Err(ScenarioError::InvalidId)
        }
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

/// Trims `raw` and checks its length in characters.
fn text(raw: &str, min: usize, max: usize, error: ScenarioError) -> Result<String, ScenarioError> {
    let trimmed = raw.trim();
    if (min..=max).contains(&trimmed.chars().count()) {
        Ok(trimmed.to_owned())
    } else {
        Err(error)
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

    /// The question is trimmed.
    pub fn new(question: &str, expected: bool) -> Result<Self, ScenarioError> {
        Ok(Self {
            question: text(question, 1, Self::MAX_QUESTION, ScenarioError::Question)?,
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

    /// Checks every field; text is trimmed.
    pub fn new(spec: ScenarioSpec) -> Result<Self, ScenarioError> {
        let name = text(&spec.name, 1, Self::MAX_NAME, ScenarioError::Name)?;
        if name.chars().any(char::is_control) {
            return Err(ScenarioError::Name);
        }
        let goal = text(&spec.goal, 1, Self::MAX_GOAL, ScenarioError::Goal)?;
        let persona = text(&spec.persona, 0, Self::MAX_PERSONA, ScenarioError::Persona)?;
        if !(1..=Self::MAX_TURNS).contains(&spec.max_turns) {
            return Err(ScenarioError::MaxTurns);
        }
        if !(1..=Self::MAX_RUBRIC).contains(&spec.rubric.len()) {
            return Err(ScenarioError::RubricCount);
        }
        Ok(Self {
            id: spec.id,
            name,
            goal,
            persona,
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

    /// The scenario's version: SHA-256 over a versioned, length-prefixed encoding of every
    /// field, so any change gives a new version and equal scenarios share one.
    #[must_use]
    pub fn version(&self) -> ContentHash {
        let mut bytes = b"vak.scenario.v1\n".to_vec();
        let mut line = |key: &str, value: &str| {
            bytes.extend_from_slice(format!("{key}={}:{value}\n", value.len()).as_bytes());
        };
        line("id", self.id.as_str());
        line("name", &self.name);
        line("goal", &self.goal);
        line("persona", &self.persona);
        line("required", &self.required.to_string());
        line("max_turns", &self.max_turns.to_string());
        line("rubric", &self.rubric.len().to_string());
        for (i, item) in self.rubric.iter().enumerate() {
            line(&format!("rubric.{i}.question"), &item.question);
            line(&format!("rubric.{i}.expected"), &item.expected.to_string());
        }
        ContentHash::of(&bytes)
    }

    /// The simulated caller's instructions: who to be, what to want, and how to end the call
    /// (its `end_call` tool). The goal and persona appear word for word.
    #[must_use]
    pub fn caller_instructions(&self) -> String {
        let persona = if self.persona.is_empty() {
            "an ordinary caller"
        } else {
            &self.persona
        };
        format!(
            "You are role-playing a person on a phone call with an automated voice agent, to \
             rehearse the agent before it goes live.\n\n\
             Who you are: {persona}\n\
             What you want: {goal}\n\n\
             How to play the part:\n\
             - Speak as this person, in the first person, in one or two short spoken sentences \
             per turn, with no lists, markdown or stage directions.\n\
             - Pursue what you want, and answer the agent's questions the way this person \
             would.\n\
             - Never say you are an AI, a model or a rehearsal, and never write the agent's \
             lines.\n\
             - When you have what you want, the agent has ended the conversation, or there is \
             nothing left to say, say a short goodbye and call the end_call tool.",
            goal = self.goal,
        )
    }
}

/// The scenarios an agent is tested with: 1 to 8, unique IDs, at least one required.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScenarioSet {
    scenarios: Vec<Scenario>,
}

impl ScenarioSet {
    /// The most scenarios in a set.
    pub const MAX_SCENARIOS: usize = 8;

    /// Checks the set's own rules; each scenario was checked when it was built.
    pub fn new(scenarios: Vec<Scenario>) -> Result<Self, ScenarioError> {
        if !(1..=Self::MAX_SCENARIOS).contains(&scenarios.len()) {
            return Err(ScenarioError::Count);
        }
        let mut ids = BTreeSet::new();
        for scenario in &scenarios {
            if !ids.insert(scenario.id()) {
                return Err(ScenarioError::DuplicateId(scenario.id().clone()));
            }
        }
        if !scenarios.iter().any(Scenario::required) {
            return Err(ScenarioError::NoneRequired);
        }
        Ok(Self { scenarios })
    }

    /// The default scenarios of `use_case`: what a draft is tested with until the operator
    /// saves its own. Every set has a required scenario whose caller opts out mid-call.
    ///
    /// **The seam for purposes.** Versioned purposes will supply their own scenario sets; this
    /// is the one place that picks a set for an agent, so it is where they plug in.
    #[must_use]
    pub fn defaults_for(use_case: UseCase) -> Self {
        Self {
            scenarios: defaults::for_use_case(use_case),
        }
    }

    #[must_use]
    pub fn scenarios(&self) -> &[Scenario] {
        &self.scenarios
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> ScenarioSpec {
        ScenarioSpec {
            id: ScenarioId::parse("a").expect("valid"),
            name: "Name".to_owned(),
            goal: "Goal.".to_owned(),
            persona: String::new(),
            required: true,
            max_turns: 3,
            rubric: vec![RubricItem::new("Did it work?", true).expect("valid")],
        }
    }

    #[test]
    fn names_have_no_control_characters() {
        let bad = ScenarioSpec {
            name: "Line\nbreak".to_owned(),
            ..spec()
        };
        assert_eq!(Scenario::new(bad), Err(ScenarioError::Name));
    }

    #[test]
    fn a_caller_without_a_persona_is_an_ordinary_caller() {
        let scenario = Scenario::new(spec()).expect("valid");
        let instructions = scenario.caller_instructions();
        assert!(instructions.contains("Who you are: an ordinary caller"));
        assert!(instructions.contains("What you want: Goal."));
        assert!(instructions.contains("end_call"));
        assert_eq!(scenario.id().to_string(), "a");
    }

    #[test]
    fn the_version_encoding_is_pinned() {
        // Changing the encoding changes every recorded scenario version: bump its tag.
        let version = Scenario::new(spec()).expect("valid").version();
        assert_eq!(
            version.as_str(),
            ContentHash::of(
                b"vak.scenario.v1\nid=1:a\nname=4:Name\ngoal=5:Goal.\npersona=0:\n\
                  required=4:true\nmax_turns=1:3\nrubric=1:1\nrubric.0.question=12:Did it work?\n\
                  rubric.0.expected=4:true\n"
            )
            .as_str()
        );
    }
}
