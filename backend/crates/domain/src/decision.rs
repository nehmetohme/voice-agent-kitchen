//! Typed decisions (System One): questions whose answer type is fixed by the question, and
//! the answers the decision model gives. The model never writes text; code reads its
//! probabilities and applies thresholds.

use std::collections::BTreeSet;

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
    /// The longest question ID.
    pub const MAX_LEN: usize = 64;

    pub fn parse(raw: &str) -> Result<Self, DecisionError> {
        let mut chars = raw.chars();
        let starts_with_letter = chars.next().is_some_and(|c| c.is_ascii_lowercase());
        let rest_ok = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if starts_with_letter && rest_ok && raw.len() <= Self::MAX_LEN {
            Ok(Self(raw.to_owned()))
        } else {
            Err(DecisionError::InvalidQuestionId)
        }
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
    /// The longest label.
    pub const MAX_LABEL_LEN: usize = 128;

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
    /// The most options a choice may have.
    pub const MAX_OPTIONS: usize = 255;

    /// A yes/no question; the answer is the probability of yes.
    #[must_use]
    pub fn yes_no(instructions: impl Into<String>) -> Self {
        Self {
            instructions: instructions.into(),
            kind: QuestionKind::YesNo,
        }
    }

    /// A choice among `options`: 1 to 255 unique labels of 1 to 128 characters.
    pub fn choice(
        instructions: impl Into<String>,
        options: impl IntoIterator<Item = ChoiceOption>,
    ) -> Result<Self, DecisionError> {
        let options: Vec<ChoiceOption> = options.into_iter().collect();
        if options.is_empty() || options.len() > Self::MAX_OPTIONS {
            return Err(DecisionError::OptionCount);
        }
        let mut labels = BTreeSet::new();
        for option in &options {
            let len = option.label.chars().count();
            if len == 0 || len > ChoiceOption::MAX_LABEL_LEN {
                return Err(DecisionError::InvalidLabel);
            }
            if !labels.insert(option.label.as_str()) {
                return Err(DecisionError::DuplicateLabel);
            }
        }
        Ok(Self {
            instructions: instructions.into(),
            kind: QuestionKind::Choice(options),
        })
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

impl Answer {
    /// Whether this answer fits `question`: the same type, probabilities from 0 to 1, and for
    /// a choice a known label with exactly one probability per option.
    #[must_use]
    pub fn fits(&self, question: &Question) -> bool {
        let unit = |p: f64| (0.0..=1.0).contains(&p);
        match (self, question.kind()) {
            (Self::YesNo { probability }, QuestionKind::YesNo) => unit(*probability),
            (
                Self::Choice {
                    label,
                    confidence,
                    probabilities,
                },
                QuestionKind::Choice(options),
            ) => {
                let known: BTreeSet<&str> = options.iter().map(ChoiceOption::label).collect();
                let answered: BTreeSet<&str> = probabilities
                    .iter()
                    .map(|(label, _)| label.as_str())
                    .collect();
                known.contains(label.as_str())
                    && unit(*confidence)
                    && answered == known
                    && answered.len() == probabilities.len()
                    && probabilities.iter().all(|(_, p)| unit(*p))
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_ids_are_snake_case() {
        for valid in ["opt_out", "a", "branch2", "x_1_y"] {
            assert_eq!(QuestionId::parse(valid).unwrap().as_str(), valid);
        }
        let too_long = "a".repeat(QuestionId::MAX_LEN + 1);
        for invalid in [
            "",
            "Opt",
            "1a",
            "_a",
            "opt out",
            "opt-out",
            too_long.as_str(),
        ] {
            assert_eq!(
                QuestionId::parse(invalid),
                Err(DecisionError::InvalidQuestionId),
                "{invalid:?}"
            );
        }
    }

    #[test]
    fn a_choice_needs_unique_labels_of_sane_length() {
        let question = Question::choice(
            "Which?",
            [
                ChoiceOption::new("a").with_description("first"),
                ChoiceOption::new("b"),
            ],
        )
        .unwrap();
        assert_eq!(question.instructions(), "Which?");
        let QuestionKind::Choice(options) = question.kind() else {
            panic!("a choice")
        };
        assert_eq!(options[0].label(), "a");
        assert_eq!(options[0].description(), Some("first"));
        assert_eq!(options[1].description(), None);

        assert_eq!(Question::choice("x", []), Err(DecisionError::OptionCount));
        let many = (0..256).map(|i| ChoiceOption::new(format!("o{i}")));
        assert_eq!(Question::choice("x", many), Err(DecisionError::OptionCount));
        assert_eq!(
            Question::choice("x", [ChoiceOption::new("")]),
            Err(DecisionError::InvalidLabel)
        );
        assert_eq!(
            Question::choice("x", [ChoiceOption::new("l".repeat(129))]),
            Err(DecisionError::InvalidLabel)
        );
        assert_eq!(
            Question::choice("x", [ChoiceOption::new("a"), ChoiceOption::new("a")]),
            Err(DecisionError::DuplicateLabel)
        );
    }

    #[test]
    fn answers_fit_only_their_question() {
        let yes_no = Question::yes_no("Stop?");
        assert_eq!(yes_no.kind(), &QuestionKind::YesNo);
        let choice =
            Question::choice("Which?", [ChoiceOption::new("a"), ChoiceOption::new("b")]).unwrap();
        let probability = |p| Answer::YesNo { probability: p };
        assert!(probability(0.0).fits(&yes_no));
        assert!(probability(1.0).fits(&yes_no));
        assert!(!probability(1.5).fits(&yes_no));
        assert!(!probability(f64::NAN).fits(&yes_no));
        assert!(!probability(0.5).fits(&choice));

        let pick = |label: &str, probabilities: &[(&str, f64)]| Answer::Choice {
            label: label.to_owned(),
            confidence: 0.8,
            probabilities: probabilities
                .iter()
                .map(|(l, p)| ((*l).to_owned(), *p))
                .collect(),
        };
        assert!(pick("a", &[("a", 0.9), ("b", 0.1)]).fits(&choice));
        assert!(!pick("c", &[("a", 0.9), ("b", 0.1)]).fits(&choice));
        assert!(!pick("a", &[("a", 0.9)]).fits(&choice));
        assert!(!pick("a", &[("a", 0.9), ("b", 0.1), ("a", 0.0)]).fits(&choice));
        assert!(!pick("a", &[("a", 1.9), ("b", 0.1)]).fits(&choice));
        assert!(!pick("a", &[("a", 0.9), ("b", 0.1)]).fits(&yes_no));
    }
}
