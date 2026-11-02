//! `ScriptedLlm`: streamed replies from a script.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::time::Instant;
use vak_providers::{
    BoxFuture, LlmEngine, LlmEvent, LlmRequest, LlmStream, ProviderError, SensitiveText, ToolCall,
};

use super::{lock, within};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Text(String),
    ToolCall { name: String, arguments: String },
}

/// One step of a scripted reply, emitted `after` the previous step (the first one after
/// the request).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmStep {
    step: Step,
    after: Duration,
}

impl LlmStep {
    /// A piece of reply text.
    #[must_use]
    pub fn text(text: &str, after: Duration) -> Self {
        Self {
            step: Step::Text(text.to_owned()),
            after,
        }
    }

    /// A call of tool `name` with `arguments` (JSON text).
    #[must_use]
    pub fn tool_call(name: &str, arguments: &str, after: Duration) -> Self {
        Self {
            step: Step::ToolCall {
                name: name.to_owned(),
                arguments: arguments.to_owned(),
            },
            after,
        }
    }
}

/// One recorded `stream` call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LlmCall {
    /// `(role, content)` of every message, in order; roles are `system`, `user` and
    /// `assistant`.
    pub messages: Vec<(String, String)>,
    /// The names of the offered tools.
    pub tools: Vec<String>,
    pub started_at: Instant,
    /// When the caller dropped the stream before the reply was complete.
    pub cancelled_at: Option<Instant>,
    /// Whether every scripted step and the final `Done` were delivered.
    pub completed: bool,
}

#[derive(Debug, Default)]
struct State {
    replies: VecDeque<Vec<LlmStep>>,
    calls: Vec<LlmCall>,
}

/// An [`LlmEngine`] that streams each request's reply from the next script: its steps at
/// their delays, then `LlmEvent::Done`. Dropping the stream early records the
/// cancellation. A request with nothing left in the script fails with
/// `ProviderError::Unavailable`.
#[derive(Debug, Default)]
pub struct ScriptedLlm {
    state: Arc<Mutex<State>>,
}

impl ScriptedLlm {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The next request is answered with `steps`, then `Done`.
    pub fn push_reply(&self, steps: impl IntoIterator<Item = LlmStep>) {
        lock(&self.state)
            .replies
            .push_back(steps.into_iter().collect());
    }

    /// Every call so far, oldest first.
    #[must_use]
    pub fn calls(&self) -> Vec<LlmCall> {
        lock(&self.state).calls.clone()
    }
}

impl LlmEngine for ScriptedLlm {
    fn stream(
        &self,
        request: LlmRequest,
    ) -> BoxFuture<'_, Result<Box<dyn LlmStream>, ProviderError>> {
        let reply = {
            let mut state = lock(&self.state);
            state.calls.push(LlmCall {
                messages: request
                    .messages()
                    .iter()
                    .map(|message| {
                        (
                            message.role().as_str().to_owned(),
                            message.content().to_owned(),
                        )
                    })
                    .collect(),
                tools: request
                    .tools()
                    .iter()
                    .map(|tool| tool.name().to_owned())
                    .collect(),
                started_at: Instant::now(),
                cancelled_at: None,
                completed: false,
            });
            let call = state.calls.len() - 1;
            state.replies.pop_front().map(|steps| ScriptedStream {
                state: Arc::clone(&self.state),
                call,
                steps: steps.into(),
                deadline: request.deadline(),
                started: false,
                finished: false,
                tool_calls: 0,
            })
        };
        Box::pin(async move {
            let stream = reply.ok_or(ProviderError::Unavailable)?;
            Ok(Box::new(stream) as Box<dyn LlmStream>)
        })
    }
}

struct ScriptedStream {
    state: Arc<Mutex<State>>,
    call: usize,
    steps: VecDeque<LlmStep>,
    deadline: Duration,
    started: bool,
    finished: bool,
    tool_calls: u32,
}

impl LlmStream for ScriptedStream {
    fn next_event(&mut self) -> BoxFuture<'_, Option<Result<LlmEvent, ProviderError>>> {
        Box::pin(async move {
            if self.finished {
                return None;
            }
            let Some(step) = self.steps.pop_front() else {
                self.finished = true;
                if let Some(call) = lock(&self.state).calls.get_mut(self.call) {
                    call.completed = true;
                }
                return Some(Ok(LlmEvent::Done));
            };
            if self.started {
                tokio::time::sleep(step.after).await;
            } else if let Err(error) = within(step.after, self.deadline).await {
                self.finished = true;
                return Some(Err(error));
            }
            self.started = true;
            Some(Ok(match step.step {
                Step::Text(text) => LlmEvent::Text(SensitiveText::new(text)),
                Step::ToolCall { name, arguments } => {
                    self.tool_calls += 1;
                    LlmEvent::ToolCall(ToolCall::new(
                        format!("call_{}", self.tool_calls),
                        name,
                        arguments,
                    ))
                }
            }))
        })
    }
}

impl Drop for ScriptedStream {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        if let Some(call) = lock(&self.state).calls.get_mut(self.call) {
            call.cancelled_at.get_or_insert_with(Instant::now);
        }
    }
}
