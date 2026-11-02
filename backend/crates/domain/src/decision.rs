//! Typed decisions (System One): questions whose answer type is fixed by the question, and
//! the answers the decision model gives. The model never writes text; code reads its
//! probabilities and applies thresholds.

/// The key of a question in a decision request, e.g. `opt_out`: lowercase letters, digits
/// and underscores, 1 to 64 characters, starting with a letter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QuestionId(String);

/// A question or its ID breaks a rule of the decision API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DecisionError {
    #[error("a question ID is lowercase letters, digits and underscores, starting with a letter")]
    InvalidQuestionId,
    #[error("a choice has 1 to 255 options")]
    OptionCount,
    #[error("a choice label is 1 to 128 characters")]
    InvalidLabel,
    #[error("the labels of a choice are unique")]
    DuplicateLabel,
}

impl QuestionId {
    pub fn parse(raw: &str) -> Result<Self, DecisionError> {
        // CONTRACT-STUB
        let _ = raw;
        Err(DecisionError::InvalidQuestionId)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One option of a choice question: a label, and optionally what it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceOption {
    label: String,
    description: Option<String>,
}

impl ChoiceOption {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            description: None,
        }
    }

    #[must_use]
    pub fn with_description(self, description: impl Into<String>) -> Self {
        Self {
            description: Some(description.into()),
            ..self
        }
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// What kind of answer a question gets.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum QuestionKind {
    /// The probability of yes (System One `noul`).
    YesNo,
    /// One of the options, with a probability for each (System One `choice`).
    Choice(Vec<ChoiceOption>),
}

/// A question for the decision model: plain-language instructions and an answer type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    instructions: String,
    kind: QuestionKind,
}

impl Question {
    /// A yes/no question; the answer is the probability of yes.
    #[must_use]
    pub fn yes_no(instructions: impl Into<String>) -> Self {
        Self {
            instructions: instructions.into(),
            kind: QuestionKind::YesNo,
        }
    }

    /// A choice among `options`: 1 to 255 unique labels.
    pub fn choice(
        instructions: impl Into<String>,
        options: impl IntoIterator<Item = ChoiceOption>,
    ) -> Result<Self, DecisionError> {
        // CONTRACT-STUB
        let _ = (instructions.into(), options.into_iter().count());
        Err(DecisionError::OptionCount)
    }

    #[must_use]
    pub fn instructions(&self) -> &str {
        &self.instructions
    }

    #[must_use]
    pub fn kind(&self) -> &QuestionKind {
        &self.kind
    }
}

/// The decision model's answer to one question.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Answer {
    /// The probability of yes, from 0 to 1.
    YesNo { probability: f64 },
    /// The chosen label, its confidence, and the probability of every option.
    Choice {
        label: String,
        confidence: f64,
        probabilities: Vec<(String, f64)>,
    },
}
