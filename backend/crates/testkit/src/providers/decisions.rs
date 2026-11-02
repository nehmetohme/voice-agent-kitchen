//! `ScriptedDecisions`: decision answers from a script.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;
use vak_domain::{Answer, ChoiceOption, QuestionKind};
use vak_providers::{BoxFuture, DecisionEngine, DecisionRequest, Decisions, ProviderError};

use super::{lock, within};

#[derive(Debug, Clone, PartialEq)]
enum Outcome {
    /// Yes/no probabilities and choice labels by question ID; anything else: 0 and the
    /// first option.
    Answers {
        yes_no: Vec<(String, f64)>,
        choices: Vec<(String, String)>,
    },
    Timeout,
    Fail(ProviderError),
}

/// How one decision call goes.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionReply {
    outcome: Outcome,
    after: Duration,
}

impl DecisionReply {
    /// Answers yes/no question `question` with `probability`, at once. Every other yes/no
    /// question gets 0, every choice its first option.
    #[must_use]
    pub fn yes_no(question: &str, probability: f64) -> Self {
        Self {
            outcome: Outcome::Answers {
                yes_no: vec![(question.to_owned(), probability)],
                choices: Vec::new(),
            },
            after: Duration::ZERO,
        }
    }

    /// The same reply, also picking `label` for choice question `question`.
    #[must_use]
    pub fn with_choice(mut self, question: &str, label: &str) -> Self {
        if let Outcome::Answers { choices, .. } = &mut self.outcome {
            choices.push((question.to_owned(), label.to_owned()));
        }
        self
    }

    /// Never answers: the call fails with `ProviderError::Timeout` when its deadline passes.
    #[must_use]
    pub fn timeout() -> Self {
        Self {
            outcome: Outcome::Timeout,
            after: Duration::ZERO,
        }
    }

    /// Fails with `error` at once.
    #[must_use]
    pub fn fail(error: ProviderError) -> Self {
        Self {
            outcome: Outcome::Fail(error),
            after: Duration::ZERO,
        }
    }

    /// The same reply, `delay` later.
    #[must_use]
    pub fn after(self, delay: Duration) -> Self {
        Self {
            after: delay,
            ..self
        }
    }
}

/// One recorded `decide` call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DecisionCall {
    /// The question IDs, in request order.
    pub questions: Vec<String>,
    /// `(speaker, text)` of every turn in the request's state; speakers are `caller` and
    /// `agent`.
    pub turns: Vec<(String, String)>,
    pub started_at: Instant,
}

#[derive(Debug, Default)]
struct State {
    replies: VecDeque<DecisionReply>,
    calls: Vec<DecisionCall>,
}

/// A [`DecisionEngine`] that answers each call with the next scripted reply. A call with
/// nothing left in the script answers at once: 0 for every yes/no question, the first
/// option of every choice.
#[derive(Debug, Default)]
pub struct ScriptedDecisions {
    state: Mutex<State>,
}

impl ScriptedDecisions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The next call goes as `reply` says.
    pub fn push(&self, reply: DecisionReply) {
        lock(&self.state).replies.push_back(reply);
    }

    /// Every call so far, oldest first.
    #[must_use]
    pub fn calls(&self) -> Vec<DecisionCall> {
        lock(&self.state).calls.clone()
    }
}

impl DecisionEngine for ScriptedDecisions {
    fn decide<'a>(
        &'a self,
        request: &'a DecisionRequest,
    ) -> BoxFuture<'a, Result<Decisions, ProviderError>> {
        let reply = {
            let mut state = lock(&self.state);
            state.calls.push(DecisionCall {
                questions: request
                    .questions()
                    .iter()
                    .map(|(id, _)| id.as_str().to_owned())
                    .collect(),
                turns: request
                    .turns()
                    .iter()
                    .map(|(speaker, text)| (speaker.as_str().to_owned(), text.as_str().to_owned()))
                    .collect(),
                started_at: Instant::now(),
            });
            state.replies.pop_front().unwrap_or(DecisionReply {
                outcome: Outcome::Answers {
                    yes_no: Vec::new(),
                    choices: Vec::new(),
                },
                after: Duration::ZERO,
            })
        };
        Box::pin(async move {
            match reply.outcome {
                Outcome::Timeout => within(request.deadline(), request.deadline())
                    .await
                    .and(Err(ProviderError::Timeout)),
                Outcome::Fail(error) => {
                    within(reply.after, request.deadline()).await?;
                    Err(error)
                }
                Outcome::Answers { yes_no, choices } => {
                    within(reply.after, request.deadline()).await?;
                    Ok(answer(request, &yes_no, &choices))
                }
            }
        })
    }
}

fn answer(
    request: &DecisionRequest,
    yes_no: &[(String, f64)],
    choices: &[(String, String)],
) -> Decisions {
    let scripted = |list: &[(String, f64)], id: &str| {
        list.iter()
            .find(|(question, _)| question == id)
            .map(|(_, value)| *value)
    };
    let answers = request
        .questions()
        .iter()
        .map(|(id, question)| {
            let answer = match question.kind() {
                QuestionKind::Choice(options) => {
                    let label = choices
                        .iter()
                        .find(|(question, _)| question == id.as_str())
                        .map(|(_, label)| label.clone())
                        .or_else(|| options.first().map(|option| option.label().to_owned()))
                        .unwrap_or_default();
                    let probabilities = options
                        .iter()
                        .map(ChoiceOption::label)
                        .map(|option| {
                            let p = if option == label { 1.0 } else { 0.0 };
                            (option.to_owned(), p)
                        })
                        .collect();
                    Answer::Choice {
                        label,
                        confidence: 1.0,
                        probabilities,
                    }
                }
                _ => Answer::YesNo {
                    probability: scripted(yes_no, id.as_str()).unwrap_or(0.0),
                },
            };
            (id.clone(), answer)
        })
        .collect();
    Decisions::new("scripted".to_owned(), answers)
}
