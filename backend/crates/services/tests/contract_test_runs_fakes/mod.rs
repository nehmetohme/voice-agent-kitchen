//! Shared setup of the `test-runs` v1 services contract (`contract_test_runs.rs`): one fake
//! LLM that plays both sides of a text conversation, a fake decision model, and the services
//! wired to in-memory repositories.
//!
//! - `FakeLlm` tells the simulated caller from the agent by the tools a request offers: only
//!   the caller gets `end_call`. A caller request picks its script by a phrase of the
//!   scenario's goal in its instructions and plays the move for its turn (its own lines so
//!   far: the `assistant` messages). An agent request answers the last caller line from a
//!   table, or with a fixed reply. Every request is recorded, and the number of streams open
//!   at once is tracked.
//! - `FakeJudge` answers `opt_out` from a table keyed by a phrase of the last caller turn and
//!   every other yes/no question from a table keyed by a phrase of its instructions.
//! - Both answer after a short pause on tokio's clock, so paused-time tests run them in
//!   virtual time and overlapping calls really overlap.
//!
//! Part of the contract: never edit it once it is tagged `contract/test-runs/v1`.
#![allow(dead_code, reason = "each test uses part of the helpers")]
#![expect(
    clippy::expect_used,
    reason = "test helpers: a failed expectation is a test failure"
)]

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::sync::Notify;
use vak_domain::{
    Actor, AgentId, AgentName, Answer, QuestionKind, RubricItem, Scenario, ScenarioId, ScenarioSet,
    ScenarioSpec, UseCase, VoiceId,
};
use vak_providers::{
    BoxFuture, ChatMessage, DecisionEngine, DecisionRequest, Decisions, LlmEngine, LlmEvent,
    LlmRequest, LlmStream, ProviderError, Role, SensitiveText, ToolCall,
};
use vak_services::{AgentService, CreateAgent, TestEngines, TestRunService, TestRunSettings};
use vak_testkit::repositories::{InMemoryAgentRepo, InMemoryTestRunRepo};

pub(crate) const LLM_MODEL: &str = "fake-llm";
pub(crate) const DECISION_MODEL: &str = "fake-decisions";
pub(crate) const CALLER_TOOL: &str = "end_call";
pub(crate) const OPT_OUT_TOOL: &str = "mark_do_not_call";
pub(crate) const OPT_OUT_QUESTION: &str = "opt_out";
/// What the agent says when the table has nothing for the caller's line.
pub(crate) const AGENT_REPLY: &str = "Happy to help with that.";
/// How long each fake takes to answer, on tokio's clock.
pub(crate) const PAUSE: Duration = Duration::from_millis(10);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn actor() -> Actor {
    Actor::new("7364019283746501", "ada@example.com").expect("valid actor")
}

// ---- Scenarios ---------------------------------------------------------------------------

pub(crate) fn item(question: &str, expected: bool) -> RubricItem {
    RubricItem::new(question, expected).expect("valid rubric item")
}

/// A scenario whose goal is `goal` (the fake caller finds its script by it).
pub(crate) fn scenario(id: &str, goal: &str, required: bool, rubric: Vec<RubricItem>) -> Scenario {
    Scenario::new(ScenarioSpec {
        id: ScenarioId::parse(id).expect("valid ID"),
        name: format!("Scenario {id}"),
        goal: goal.to_owned(),
        persona: "Polite, answers briefly.".to_owned(),
        required,
        max_turns: 6,
        rubric,
    })
    .expect("valid scenario")
}

pub(crate) fn set(scenarios: Vec<Scenario>) -> ScenarioSet {
    ScenarioSet::new(scenarios).expect("valid set")
}

// ---- The fake LLM ------------------------------------------------------------------------

/// What one side says in one turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Move {
    /// Text, then the end of the reply.
    Say(&'static str),
    /// The caller's last words, then the `end_call` tool.
    HangUp(&'static str),
    /// A call of the named tool and nothing else.
    Tool(&'static str),
    /// Never answers.
    Stall,
    /// Fails at once.
    Fail,
}

/// Who a request is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Caller,
    Agent,
}

/// One recorded request.
#[derive(Debug, Clone)]
pub(crate) struct LlmCall {
    pub(crate) side: Side,
    /// `(role, content)` of every message, in order.
    pub(crate) messages: Vec<(Role, String)>,
    pub(crate) tools: Vec<String>,
    pub(crate) greedy_seed: Option<u64>,
    pub(crate) deadline: Duration,
    pub(crate) max_tokens: Option<u32>,
}

#[derive(Debug, Default)]
struct LlmState {
    calls: Vec<LlmCall>,
    open: usize,
    max_open: usize,
    dropped_unfinished: usize,
}

/// Plays the caller and the agent. See the module docs.
#[derive(Debug, Default)]
pub(crate) struct FakeLlm {
    callers: Vec<(&'static str, Vec<Move>)>,
    agent: Vec<(&'static str, Move)>,
    state: Arc<Mutex<LlmState>>,
    opened: Arc<Notify>,
}

impl FakeLlm {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The caller of the scenario whose goal contains `goal_phrase` plays `moves`, one per
    /// turn; past the last one it hangs up.
    pub(crate) fn caller(mut self, goal_phrase: &'static str, moves: Vec<Move>) -> Self {
        self.callers.push((goal_phrase, moves));
        self
    }

    /// The agent answers a caller line containing `phrase` with `reply`.
    pub(crate) fn agent(mut self, phrase: &'static str, reply: Move) -> Self {
        self.agent.push((phrase, reply));
        self
    }

    pub(crate) fn calls(&self) -> Vec<LlmCall> {
        lock(&self.state).calls.clone()
    }

    /// The most streams that were open at once.
    pub(crate) fn max_open(&self) -> usize {
        lock(&self.state).max_open
    }

    /// Streams dropped before they finished: requests the caller cancelled.
    pub(crate) fn dropped_unfinished(&self) -> usize {
        lock(&self.state).dropped_unfinished
    }

    /// Resolves the next time a request comes in.
    pub(crate) fn opened(&self) -> Arc<Notify> {
        Arc::clone(&self.opened)
    }

    fn play(&self, request: &LlmRequest) -> (Side, Move) {
        let offers = |name: &str| request.tools().iter().any(|tool| tool.name() == name);
        let messages = request.messages();
        if offers(CALLER_TOOL) {
            let instructions = messages
                .first()
                .map(ChatMessage::content)
                .unwrap_or_default();
            let turn = messages
                .iter()
                .filter(|m| m.role() == Role::Assistant)
                .count();
            let script = self
                .callers
                .iter()
                .find(|(phrase, _)| instructions.contains(phrase))
                .map(|(_, moves)| moves.clone())
                .unwrap_or_default();
            let next = script
                .get(turn)
                .cloned()
                .unwrap_or(Move::HangUp("Goodbye."));
            return (Side::Caller, next);
        }
        let last_caller = messages
            .iter()
            .rev()
            .find(|m| m.role() == Role::User)
            .map(ChatMessage::content)
            .unwrap_or_default();
        let reply = self
            .agent
            .iter()
            .find(|(phrase, _)| last_caller.contains(phrase))
            .map_or(Move::Say(AGENT_REPLY), |(_, reply)| reply.clone());
        (Side::Agent, reply)
    }
}

impl LlmEngine for FakeLlm {
    fn stream(
        &self,
        request: LlmRequest,
    ) -> BoxFuture<'_, Result<Box<dyn LlmStream>, ProviderError>> {
        let (side, next) = self.play(&request);
        {
            let mut state = lock(&self.state);
            state.calls.push(LlmCall {
                side,
                messages: request
                    .messages()
                    .iter()
                    .map(|m| (m.role(), m.content().to_owned()))
                    .collect(),
                tools: request
                    .tools()
                    .iter()
                    .map(|t| t.name().to_owned())
                    .collect(),
                greedy_seed: request.greedy_seed(),
                deadline: request.deadline(),
                max_tokens: request.max_tokens(),
            });
        }
        self.opened.notify_waiters();
        let state = Arc::clone(&self.state);
        Box::pin(async move {
            if next == Move::Fail {
                return Err(ProviderError::Unavailable);
            }
            let events = match next {
                Move::Say(text) => vec![LlmEvent::Text(SensitiveText::new(text))],
                Move::HangUp(text) => vec![
                    LlmEvent::Text(SensitiveText::new(text)),
                    LlmEvent::ToolCall(ToolCall::new("call_1", CALLER_TOOL, "{}")),
                ],
                Move::Tool(name) => vec![LlmEvent::ToolCall(ToolCall::new("call_1", name, "{}"))],
                Move::Stall | Move::Fail => Vec::new(),
            };
            {
                let mut state = lock(&state);
                state.open += 1;
                state.max_open = state.max_open.max(state.open);
            }
            Ok(Box::new(FakeStream {
                state,
                events: events.into(),
                stall: next == Move::Stall,
                done: false,
            }) as Box<dyn LlmStream>)
        })
    }
}

struct FakeStream {
    state: Arc<Mutex<LlmState>>,
    events: std::collections::VecDeque<LlmEvent>,
    stall: bool,
    done: bool,
}

impl LlmStream for FakeStream {
    fn next_event(&mut self) -> BoxFuture<'_, Option<Result<LlmEvent, ProviderError>>> {
        Box::pin(async move {
            if self.done {
                return None;
            }
            if self.stall {
                std::future::pending::<()>().await;
            }
            tokio::time::sleep(PAUSE).await;
            if let Some(event) = self.events.pop_front() {
                Some(Ok(event))
            } else {
                self.done = true;
                Some(Ok(LlmEvent::Done))
            }
        })
    }
}

impl Drop for FakeStream {
    fn drop(&mut self) {
        let mut state = lock(&self.state);
        state.open = state.open.saturating_sub(1);
        if !self.done {
            state.dropped_unfinished += 1;
        }
    }
}

// ---- The fake decision model ------------------------------------------------------------

/// One recorded decision request.
#[derive(Debug, Clone)]
pub(crate) struct DecisionCall {
    /// `(id, instructions)` of every question.
    pub(crate) questions: Vec<(String, String)>,
    /// `(speaker, text)` of every turn; speakers are `caller` and `agent`.
    pub(crate) turns: Vec<(String, String)>,
    pub(crate) deadline: Duration,
}

/// Answers yes/no questions from tables. See the module docs.
#[derive(Debug)]
pub(crate) struct FakeJudge {
    opt_out: Vec<(&'static str, f64)>,
    rubric: Vec<(&'static str, f64)>,
    rubric_default: Mutex<f64>,
    fail_rubric: bool,
    calls: Mutex<Vec<DecisionCall>>,
}

impl FakeJudge {
    /// Nobody opts out, and every rubric question is answered yes (0.9).
    pub(crate) fn new() -> Self {
        Self {
            opt_out: Vec::new(),
            rubric: Vec::new(),
            rubric_default: Mutex::new(0.9),
            fail_rubric: false,
            calls: Mutex::new(Vec::new()),
        }
    }

    /// `opt_out` is `p` when the last caller turn contains `phrase`.
    pub(crate) fn opt_out(mut self, phrase: &'static str, p: f64) -> Self {
        self.opt_out.push((phrase, p));
        self
    }

    /// A rubric question containing `phrase` is answered `p`.
    pub(crate) fn rubric(mut self, phrase: &'static str, p: f64) -> Self {
        self.rubric.push((phrase, p));
        self
    }

    /// From now on, rubric questions without an entry in the table are answered `p`.
    pub(crate) fn answer_rubric_with(&self, p: f64) {
        *lock(&self.rubric_default) = p;
    }

    /// Requests without `opt_out` (the rubric's) fail.
    pub(crate) fn failing_rubric(mut self) -> Self {
        self.fail_rubric = true;
        self
    }

    pub(crate) fn calls(&self) -> Vec<DecisionCall> {
        lock(&self.calls).clone()
    }

    /// The requests that score a rubric: those without the `opt_out` question.
    pub(crate) fn rubric_calls(&self) -> Vec<DecisionCall> {
        self.calls()
            .into_iter()
            .filter(|call| call.questions.iter().all(|(id, _)| id != OPT_OUT_QUESTION))
            .collect()
    }

    fn answer(&self, request: &DecisionRequest) -> Result<Decisions, ProviderError> {
        let last_caller = request
            .turns()
            .iter()
            .rev()
            .find(|(speaker, _)| speaker.as_str() == "caller")
            .map(|(_, text)| text.as_str())
            .unwrap_or_default();
        let scores_rubric = request
            .questions()
            .iter()
            .all(|(id, _)| id.as_str() != OPT_OUT_QUESTION);
        if scores_rubric && self.fail_rubric {
            return Err(ProviderError::Unavailable);
        }
        let answers = request
            .questions()
            .iter()
            .map(|(id, question)| {
                assert_eq!(
                    question.kind(),
                    &QuestionKind::YesNo,
                    "only yes/no questions"
                );
                let probability = if id.as_str() == OPT_OUT_QUESTION {
                    self.opt_out
                        .iter()
                        .find(|(phrase, _)| last_caller.contains(phrase))
                        .map_or(0.0, |(_, p)| *p)
                } else {
                    self.rubric
                        .iter()
                        .find(|(phrase, _)| question.instructions().contains(phrase))
                        .map_or(*lock(&self.rubric_default), |(_, p)| *p)
                };
                (id.clone(), Answer::YesNo { probability })
            })
            .collect();
        Ok(Decisions::new(DECISION_MODEL.to_owned(), answers))
    }
}

impl DecisionEngine for FakeJudge {
    fn decide<'a>(
        &'a self,
        request: &'a DecisionRequest,
    ) -> BoxFuture<'a, Result<Decisions, ProviderError>> {
        lock(&self.calls).push(DecisionCall {
            questions: request
                .questions()
                .iter()
                .map(|(id, q)| (id.as_str().to_owned(), q.instructions().to_owned()))
                .collect(),
            turns: request
                .turns()
                .iter()
                .map(|(speaker, text)| (speaker.as_str().to_owned(), text.as_str().to_owned()))
                .collect(),
            deadline: request.deadline(),
        });
        Box::pin(async move {
            tokio::time::sleep(PAUSE).await;
            self.answer(request)
        })
    }
}

// ---- The services ------------------------------------------------------------------------

/// The services over in-memory repositories and the fakes.
pub(crate) struct World {
    pub(crate) agents: AgentService,
    pub(crate) runs: TestRunService,
    pub(crate) runs_repo: Arc<InMemoryTestRunRepo>,
    pub(crate) llm: Arc<FakeLlm>,
    pub(crate) judge: Arc<FakeJudge>,
}

impl World {
    pub(crate) fn new(llm: FakeLlm, judge: FakeJudge) -> Self {
        Self::with_settings(llm, judge, TestRunSettings::default())
    }

    pub(crate) fn with_settings(llm: FakeLlm, judge: FakeJudge, settings: TestRunSettings) -> Self {
        let runs_repo = Arc::new(InMemoryTestRunRepo::default());
        let agents = AgentService::new(Arc::new(InMemoryAgentRepo::default()))
            .with_test_runs(runs_repo.clone());
        let llm = Arc::new(llm);
        let judge = Arc::new(judge);
        let engines = TestEngines::new(llm.clone(), LLM_MODEL, judge.clone(), DECISION_MODEL);
        let runs = TestRunService::new(agents.clone(), runs_repo.clone(), Some(engines))
            .with_settings(settings);
        Self {
            agents,
            runs,
            runs_repo,
            llm,
            judge,
        }
    }

    /// A new agent named "Front desk" for `use_case`.
    pub(crate) async fn agent(&self, use_case: UseCase) -> AgentId {
        let command = CreateAgent {
            name: AgentName::parse("Front desk").expect("valid"),
            use_case,
            voice: VoiceId::parse("af_heart").expect("valid"),
            language: None,
            jurisdiction: None,
            model: None,
            turn_detection: None,
        };
        self.agents
            .create(&actor(), command)
            .await
            .expect("created")
            .id()
    }

    /// A new agent with `scenarios` saved.
    pub(crate) async fn agent_with(&self, scenarios: ScenarioSet) -> AgentId {
        let id = self.agent(UseCase::Booking).await;
        self.agents
            .save_scenarios(&actor(), id, scenarios)
            .await
            .expect("saved");
        id
    }
}
