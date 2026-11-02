//! `DecisionEngine`: typed answers (System One) about the conversation so far.

use std::time::Duration;

use vak_domain::{Answer, Question, QuestionId, Speaker};

use crate::{BoxFuture, ProviderError, SensitiveText};

/// Questions about one state of the conversation, asked in one round trip.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionRequest {
    turns: Vec<(Speaker, SensitiveText)>,
    plan_node: Option<String>,
    questions: Vec<(QuestionId, Question)>,
    deadline: Duration,
}

impl DecisionRequest {
    /// No turns and no questions yet. The engine fails with [`ProviderError::Timeout`] when
    /// it has no answers within `deadline`: a timeout is never an answer.
    #[must_use]
    pub const fn new(deadline: Duration) -> Self {
        Self {
            turns: Vec::new(),
            plan_node: None,
            questions: Vec::new(),
            deadline,
        }
    }

    /// Adds a turn of the conversation, oldest first.
    #[must_use]
    pub fn with_turn(mut self, speaker: Speaker, text: impl Into<String>) -> Self {
        self.turns.push((speaker, SensitiveText::new(text)));
        self
    }

    /// The plan node the conversation is at.
    #[must_use]
    pub fn with_plan_node(self, node: impl Into<String>) -> Self {
        Self {
            plan_node: Some(node.into()),
            ..self
        }
    }

    #[must_use]
    pub fn with_question(mut self, id: QuestionId, question: Question) -> Self {
        self.questions.push((id, question));
        self
    }

    #[must_use]
    pub fn turns(&self) -> &[(Speaker, SensitiveText)] {
        &self.turns
    }

    #[must_use]
    pub fn plan_node(&self) -> Option<&str> {
        self.plan_node.as_deref()
    }

    #[must_use]
    pub fn questions(&self) -> &[(QuestionId, Question)] {
        &self.questions
    }

    #[must_use]
    pub const fn deadline(&self) -> Duration {
        self.deadline
    }
}

/// The answers to every question of a request, from the model that gave them.
#[derive(Debug, Clone, PartialEq)]
pub struct Decisions {
    model: String,
    answers: Vec<(QuestionId, Answer)>,
}

impl Decisions {
    #[must_use]
    pub const fn new(model: String, answers: Vec<(QuestionId, Answer)>) -> Self {
        Self { model, answers }
    }

    /// The model ID the backend reports, which may differ from the one requested.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// The answer to question `id`.
    #[must_use]
    pub fn answer(&self, id: &str) -> Option<&Answer> {
        self.answers
            .iter()
            .find(|(question, _)| question.as_str() == id)
            .map(|(_, answer)| answer)
    }

    #[must_use]
    pub fn answers(&self) -> &[(QuestionId, Answer)] {
        &self.answers
    }
}

/// Typed decisions: one request, every question answered in parallel.
pub trait DecisionEngine: Send + Sync {
    fn decide<'a>(
        &'a self,
        request: &'a DecisionRequest,
    ) -> BoxFuture<'a, Result<Decisions, ProviderError>>;
}
