//! Text-mode conversations: the agent's reply logic without audio, for test runs.
//!
//! [`run_text_session`] plays one conversation between the agent and a [`SimulatedCaller`]
//! line by line. The agent side is the voice session's: the locked disclosure first, the same
//! [`Router`] (instructions, history rules, tools and decision questions), and the same three
//! opt-out layers with the confirm band ([`OptOutPolicy`]). There is no STT, TTS, VAD, pacing
//! or barge-in, so a conversation is as fast as its model calls and reproducible with
//! deterministic models.
//!
//! Per caller line, as on a call: the opt-out phrases first (a match ends the conversation
//! before any model call); then the decision model's `opt_out` question and the LLM's reply,
//! in parallel. The decision is always awaited (up to its deadline): in text there is no
//! reply to hold back, and waiting gives the result a late answer would have had on a call. At
//! or above the decision threshold the agent says the locked confirmation and the conversation
//! ends (`decision`); a `mark_do_not_call` call ends it the same way (`llm_tool`); in the
//! confirm band the check question replaces the reply, and the next caller line answers it
//! with the decision threshold, never asking twice in a row. A decision that times out or
//! fails is never a no: the conversation goes on.
//!
//! It ends when the caller hangs up (`ClientEnded`; its last words are recorded but not
//! answered), an opt-out ends it, the caller's turn cap is reached (`Timeout`), or a model
//! fails (`Error`). Every model call has a deadline. Audio-mode simulation (TTS for the caller,
//! STT for the agent) is a later option.

use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;
use vak_domain::{Answer, OptOutLayer, OptOutPhrases, SessionEndReason, Speaker, TranscriptLine};
use vak_providers::{
    BoxFuture, DecisionEngine, DecisionRequest, LlmEngine, LlmEvent, LlmRequest, ProviderError,
    Role, ToolSpec,
};

use crate::OptOutPolicy;
use crate::routing::{Line, OPT_OUT_QUESTION, OPT_OUT_TOOL, Router};

/// The simulated caller's tool for hanging up.
pub const CALLER_TOOL: &str = "end_call";
const CALLER_TOOL_DESCRIPTION: &str = "Hang up: the call is over for you. Say your goodbye in \
     the same reply.";
/// Enough for two short spoken sentences.
const MAX_CALLER_TOKENS: u32 = 120;

/// The models a text conversation talks to. Cheap to clone.
#[derive(Clone)]
pub struct TextEngines {
    llm: Arc<dyn LlmEngine>,
    decisions: Arc<dyn DecisionEngine>,
}

impl TextEngines {
    #[must_use]
    pub fn new(llm: Arc<dyn LlmEngine>, decisions: Arc<dyn DecisionEngine>) -> Self {
        Self { llm, decisions }
    }
}

/// How long the LLM may take, the agent's replies and the caller's lines alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextDeadlines {
    /// From the request to its first event.
    pub first_token: Duration,
    /// From the request to the end of the reply.
    pub reply: Duration,
}

/// One text conversation's configuration.
#[derive(Debug, Clone)]
pub struct TextSessionConfig {
    disclosure: String,
    system_prompt: String,
    opt_out: OptOutPolicy,
    deadlines: TextDeadlines,
    max_caller_turns: u8,
    seed: u64,
}

impl TextSessionConfig {
    /// `disclosure` is the agent's locked first line and `system_prompt` its instructions.
    /// By default: the platform's opt-out phrases and confirmation, 20 s to the first token
    /// and 30 s per reply, 8 caller turns, seed 1.
    #[must_use]
    pub fn new(disclosure: impl Into<String>, system_prompt: impl Into<String>) -> Self {
        Self {
            disclosure: disclosure.into(),
            system_prompt: system_prompt.into(),
            opt_out: OptOutPolicy::new(
                vak_domain::DEFAULT_OPT_OUT_PHRASES,
                vak_domain::DEFAULT_OPT_OUT_CONFIRMATION,
            ),
            deadlines: TextDeadlines {
                first_token: Duration::from_secs(20),
                reply: Duration::from_secs(30),
            },
            max_caller_turns: 8,
            seed: 1,
        }
    }

    /// The opt-out layers; its decision deadline bounds each `opt_out` question (the decision
    /// wait doesn't apply: text has no reply to hold back).
    #[must_use]
    pub fn with_opt_out(self, opt_out: OptOutPolicy) -> Self {
        Self { opt_out, ..self }
    }

    #[must_use]
    pub fn with_deadlines(self, deadlines: TextDeadlines) -> Self {
        Self { deadlines, ..self }
    }

    /// The conversation stops after this many caller lines (`Timeout`).
    #[must_use]
    pub fn with_max_caller_turns(self, max_caller_turns: u8) -> Self {
        Self {
            max_caller_turns,
            ..self
        }
    }

    /// Every agent reply samples greedily with this seed.
    #[must_use]
    pub fn with_seed(self, seed: u64) -> Self {
        Self { seed, ..self }
    }
}

/// What the simulated caller does on its turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallerTurn {
    /// One line to the agent.
    Says(String),
    /// The caller hangs up, maybe with some last words.
    HangsUp { last_words: Option<String> },
}

/// The other side of a text conversation: given the conversation so far, its next turn.
pub trait SimulatedCaller: Send {
    fn next_turn<'a>(
        &'a mut self,
        transcript: &'a [TranscriptLine],
    ) -> BoxFuture<'a, Result<CallerTurn, ProviderError>>;
}

/// A simulated caller played by an LLM: its instructions as the system message, the agent's
/// lines as the other party's and its own as its replies, and the `end_call` tool to hang up.
/// It samples greedily with the conversation's seed.
pub struct LlmCaller {
    llm: Arc<dyn LlmEngine>,
    instructions: String,
    deadlines: TextDeadlines,
    seed: u64,
}

impl LlmCaller {
    #[must_use]
    pub fn new(
        llm: Arc<dyn LlmEngine>,
        instructions: impl Into<String>,
        deadlines: TextDeadlines,
        seed: u64,
    ) -> Self {
        Self {
            llm,
            instructions: instructions.into(),
            deadlines,
            seed,
        }
    }

    fn request(&self, transcript: &[TranscriptLine]) -> LlmRequest {
        transcript
            .iter()
            .fold(
                LlmRequest::new(self.deadlines.first_token)
                    .with_message(Role::System, self.instructions.as_str()),
                |request, line| {
                    let role = match line.speaker() {
                        Speaker::Agent => Role::User,
                        Speaker::Caller => Role::Assistant,
                    };
                    request.with_message(role, line.text())
                },
            )
            .with_tool(ToolSpec::new(CALLER_TOOL, CALLER_TOOL_DESCRIPTION))
            .with_max_tokens(MAX_CALLER_TOKENS)
            .with_greedy_sampling(self.seed)
    }
}

impl SimulatedCaller for LlmCaller {
    fn next_turn<'a>(
        &'a mut self,
        transcript: &'a [TranscriptLine],
    ) -> BoxFuture<'a, Result<CallerTurn, ProviderError>> {
        Box::pin(async move {
            let request = self.request(transcript);
            let reply = collect(self.llm.as_ref(), request, self.deadlines.reply).await?;
            let words = Some(reply.text.trim().to_owned()).filter(|words| !words.is_empty());
            Ok(match words {
                Some(words) if !reply.called(CALLER_TOOL) => CallerTurn::Says(words),
                last_words => CallerTurn::HangsUp { last_words },
            })
        })
    }
}

/// How a text conversation went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextOutcome {
    end_reason: SessionEndReason,
    transcript: Vec<TranscriptLine>,
}

impl TextOutcome {
    /// `ClientEnded` (the caller hung up), `OptOut`, `Timeout` (the turn cap) or `Error`.
    #[must_use]
    pub const fn end_reason(&self) -> SessionEndReason {
        self.end_reason
    }

    #[must_use]
    pub fn transcript(&self) -> &[TranscriptLine] {
        &self.transcript
    }

    #[must_use]
    pub fn into_transcript(self) -> Vec<TranscriptLine> {
        self.transcript
    }
}

/// An LLM reply, read to its end.
#[derive(Debug, Default)]
struct Reply {
    text: String,
    tools: Vec<String>,
}

impl Reply {
    fn called(&self, tool: &str) -> bool {
        self.tools.iter().any(|name| name == tool)
    }
}

/// Streams `request` to its end: the first event within the request's deadline, the whole
/// reply within `total`. Dropping the future drops the stream, which aborts the request.
async fn collect(
    llm: &dyn LlmEngine,
    request: LlmRequest,
    total: Duration,
) -> Result<Reply, ProviderError> {
    let first_by = Instant::now() + request.deadline();
    let read = async {
        let mut stream = tokio::time::timeout_at(first_by, llm.stream(request))
            .await
            .map_err(|_| ProviderError::Timeout)??;
        let mut reply = Reply::default();
        let mut started = false;
        loop {
            let next = if started {
                stream.next_event().await
            } else {
                tokio::time::timeout_at(first_by, stream.next_event())
                    .await
                    .map_err(|_| ProviderError::Timeout)?
            };
            started = true;
            match next {
                Some(Ok(LlmEvent::Text(text))) => reply.text.push_str(text.as_str()),
                Some(Ok(LlmEvent::ToolCall(call))) => reply.tools.push(call.name().to_owned()),
                Some(Err(error)) => return Err(error),
                // Done, an event this loop doesn't use, or the end of the stream.
                Some(Ok(LlmEvent::Done)) | None => return Ok(reply),
                Some(Ok(_)) => {}
            }
        }
    };
    tokio::time::timeout(total, read)
        .await
        .unwrap_or(Err(ProviderError::Timeout))
}

/// The probability of yes to `opt_out`, or `None` when the decision timed out, failed or gave
/// no usable answer: never a no, and never a yes.
async fn opt_out_probability(
    engine: &dyn DecisionEngine,
    request: &DecisionRequest,
) -> Option<f64> {
    let answer = tokio::time::timeout(request.deadline(), engine.decide(request))
        .await
        .unwrap_or(Err(ProviderError::Timeout));
    match answer {
        Ok(decisions) => match decisions.answer(OPT_OUT_QUESTION) {
            Some(Answer::YesNo { probability }) if (0.0..=1.0).contains(probability) => {
                Some(*probability)
            }
            _ => None,
        },
        Err(error) => {
            tracing::warn!(%error, "a test conversation's opt-out decision failed");
            None
        }
    }
}

/// What the agent does with one caller line.
enum Step {
    /// Answer with this text (blank: say nothing).
    Answer(String),
    /// Ask the opt-out check question instead of answering.
    Check,
    /// End the conversation.
    End(SessionEndReason),
}

struct Conversation<'a> {
    config: &'a TextSessionConfig,
    engines: &'a TextEngines,
    router: Router,
    phrases: OptOutPhrases,
    transcript: Vec<TranscriptLine>,
    /// The agent's last line was the check question: this caller line answers it.
    check_asked: bool,
}

impl Conversation<'_> {
    fn say(&mut self, speaker: Speaker, text: &str) {
        self.transcript.push(TranscriptLine::new(speaker, text));
    }

    fn history(&self) -> Vec<Line> {
        self.transcript
            .iter()
            .map(|line| Line {
                speaker: line.speaker(),
                text: line.text().to_owned(),
            })
            .collect()
    }

    /// The agent's answer to the caller line just added.
    async fn answer(&mut self) -> Step {
        let caller_line = self.transcript.last().map_or("", TranscriptLine::text);
        if self.phrases.matches(caller_line) {
            return Step::End(SessionEndReason::OptOut(OptOutLayer::Phrase));
        }
        let policy = &self.config.opt_out;
        let history = self.history();
        let decision = self
            .router
            .decision_request(&history, policy.decision_deadline);
        let request = self
            .router
            .llm_request(&history, self.config.deadlines.first_token)
            .with_greedy_sampling(self.config.seed);
        let (probability, reply) = tokio::join!(
            opt_out_probability(self.engines.decisions.as_ref(), &decision),
            collect(
                self.engines.llm.as_ref(),
                request,
                self.config.deadlines.reply
            ),
        );
        let answers_check = std::mem::take(&mut self.check_asked);
        let in_band = !answers_check && probability.is_some_and(|p| p >= policy.confirm_threshold);
        if probability.is_some_and(|p| p >= policy.decision_threshold) {
            return Step::End(SessionEndReason::OptOut(OptOutLayer::Decision));
        }
        match reply {
            Ok(reply) if reply.called(OPT_OUT_TOOL) => {
                Step::End(SessionEndReason::OptOut(OptOutLayer::LlmTool))
            }
            Ok(_) | Err(_) if in_band => {
                if policy.check_question.is_empty() {
                    Step::End(SessionEndReason::OptOut(OptOutLayer::Decision))
                } else {
                    Step::Check
                }
            }
            Ok(reply) => Step::Answer(reply.text.trim().to_owned()),
            Err(error) => {
                tracing::warn!(%error, "a test conversation's agent reply failed");
                Step::End(SessionEndReason::Error)
            }
        }
    }

    fn end(self, end_reason: SessionEndReason) -> TextOutcome {
        TextOutcome {
            end_reason,
            transcript: self.transcript,
        }
    }
}

/// Plays one conversation between the agent and `caller` until it ends. See the module docs.
pub async fn run_text_session(
    config: &TextSessionConfig,
    engines: &TextEngines,
    caller: &mut dyn SimulatedCaller,
) -> TextOutcome {
    let Some(router) = Router::new(&config.system_prompt) else {
        return TextOutcome {
            end_reason: SessionEndReason::Error,
            transcript: Vec::new(),
        };
    };
    let mut conversation = Conversation {
        config,
        engines,
        router,
        phrases: OptOutPhrases::new(&config.opt_out.phrases),
        transcript: Vec::new(),
        check_asked: false,
    };
    let disclosure = config.disclosure.trim();
    if !disclosure.is_empty() {
        conversation.say(Speaker::Agent, disclosure);
    }
    for _ in 0..config.max_caller_turns {
        let turn = caller.next_turn(&conversation.transcript).await;
        let line = match turn {
            Ok(CallerTurn::Says(line)) if !line.trim().is_empty() => line,
            Ok(CallerTurn::Says(_)) => return conversation.end(SessionEndReason::ClientEnded),
            Ok(CallerTurn::HangsUp { last_words }) => {
                if let Some(words) = last_words.filter(|words| !words.trim().is_empty()) {
                    conversation.say(Speaker::Caller, words.trim());
                }
                return conversation.end(SessionEndReason::ClientEnded);
            }
            Err(error) => {
                tracing::warn!(%error, "a test conversation's simulated caller failed");
                return conversation.end(SessionEndReason::Error);
            }
        };
        conversation.say(Speaker::Caller, line.trim());
        match conversation.answer().await {
            Step::Answer(text) if text.is_empty() => {}
            Step::Answer(text) => conversation.say(Speaker::Agent, &text),
            Step::Check => {
                let question = config.opt_out.check_question.clone();
                conversation.say(Speaker::Agent, &question);
                conversation.check_asked = true;
            }
            Step::End(SessionEndReason::OptOut(layer)) => {
                let confirmation = config.opt_out.confirmation.trim().to_owned();
                if !confirmation.is_empty() {
                    conversation.say(Speaker::Agent, &confirmation);
                }
                return conversation.end(SessionEndReason::OptOut(layer));
            }
            Step::End(reason) => return conversation.end(reason),
        }
    }
    conversation.end(SessionEndReason::Timeout)
}
