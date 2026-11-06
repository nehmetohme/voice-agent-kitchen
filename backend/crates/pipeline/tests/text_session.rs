//! Text-mode conversations (`run_text_session`), not a contract: the endings, the opt-out layers
//! and the confirm band in text, deadlines, and the simulated caller's requests, with testkit's
//! scripted engines and a caller that reads its lines from a list. The `test-runs` v1 contract
//! covers the same through test runs (`vak-services`).

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use vak_domain::{
    DEFAULT_OPT_OUT_CHECK_QUESTION, DEFAULT_OPT_OUT_CONFIRMATION, OptOutLayer, SessionEndReason,
    Speaker, TranscriptLine,
};
use vak_pipeline::{
    CALLER_TOOL, CallerTurn, LlmCaller, OptOutPolicy, SimulatedCaller, TextDeadlines, TextEngines,
    TextSessionConfig, run_text_session,
};
use vak_providers::{BoxFuture, ProviderError};
use vak_testkit::providers::{DecisionReply, LlmStep, ScriptedDecisions, ScriptedLlm};

const DISCLOSURE: &str = "Hi, this is Ava, an automated AI assistant.";
const QUICK: Duration = Duration::from_millis(10);
const DEADLINES: TextDeadlines = TextDeadlines {
    first_token: Duration::from_secs(2),
    reply: Duration::from_secs(5),
};

/// A caller that says its lines in order, then hangs up; `Err` lines fail the turn.
struct Script(VecDeque<Result<CallerTurn, ProviderError>>);

impl Script {
    fn says(lines: &[&str]) -> Self {
        Self(
            lines
                .iter()
                .map(|line| Ok(CallerTurn::Says((*line).to_owned())))
                .collect(),
        )
    }
}

impl SimulatedCaller for Script {
    fn next_turn<'a>(
        &'a mut self,
        _transcript: &'a [TranscriptLine],
    ) -> BoxFuture<'a, Result<CallerTurn, ProviderError>> {
        let next = self
            .0
            .pop_front()
            .unwrap_or(Ok(CallerTurn::HangsUp { last_words: None }));
        Box::pin(async move { next })
    }
}

fn config() -> TextSessionConfig {
    TextSessionConfig::new(DISCLOSURE, "You are Ava.")
        .with_deadlines(DEADLINES)
        .with_max_caller_turns(4)
        .with_seed(9)
}

fn engines(llm: &Arc<ScriptedLlm>, decisions: &Arc<ScriptedDecisions>) -> TextEngines {
    TextEngines::new(llm.clone(), decisions.clone())
}

fn reply(llm: &ScriptedLlm, text: &str) {
    llm.push_reply([LlmStep::text(text, QUICK)]);
}

fn lines(transcript: &[TranscriptLine]) -> Vec<(Speaker, &str)> {
    transcript.iter().map(|l| (l.speaker(), l.text())).collect()
}

#[tokio::test(start_paused = true)]
async fn the_agent_answers_each_line_until_the_turn_cap() {
    let llm = Arc::new(ScriptedLlm::new());
    let decisions = Arc::new(ScriptedDecisions::new());
    for text in ["One.", "Two.", "Three.", "Four."] {
        reply(&llm, text);
    }
    let mut caller = Script::says(&["a", "b", "c", "d", "e"]);

    let outcome = run_text_session(&config(), &engines(&llm, &decisions), &mut caller).await;

    assert_eq!(outcome.end_reason(), SessionEndReason::Timeout);
    assert_eq!(
        outcome.transcript().len(),
        9,
        "the disclosure and four exchanges"
    );
    let calls = llm.calls();
    assert_eq!(calls.len(), 4);
    assert_eq!(calls[0].messages[0].0, "system");
    assert_eq!(
        calls[0].messages[1..],
        [
            ("assistant".to_owned(), DISCLOSURE.to_owned()),
            ("user".to_owned(), "a".to_owned())
        ]
    );
    assert_eq!(
        decisions.calls().len(),
        4,
        "one opt-out question per caller line"
    );
}

#[tokio::test(start_paused = true)]
async fn a_hang_up_records_the_last_words_and_a_blank_line_ends_too() {
    let llm = Arc::new(ScriptedLlm::new());
    let decisions = Arc::new(ScriptedDecisions::new());
    let mut caller = Script(VecDeque::from([Ok(CallerTurn::HangsUp {
        last_words: Some("  Bye.  ".to_owned()),
    })]));
    let outcome = run_text_session(&config(), &engines(&llm, &decisions), &mut caller).await;
    assert_eq!(outcome.end_reason(), SessionEndReason::ClientEnded);
    assert_eq!(
        lines(outcome.transcript()),
        [(Speaker::Agent, DISCLOSURE), (Speaker::Caller, "Bye.")]
    );
    assert!(llm.calls().is_empty(), "last words aren't answered");

    let mut blank = Script::says(&["   "]);
    let outcome = run_text_session(&config(), &engines(&llm, &decisions), &mut blank).await;
    assert_eq!(outcome.end_reason(), SessionEndReason::ClientEnded);
    assert_eq!(outcome.transcript().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn model_failures_end_the_conversation_with_an_error() {
    let llm = Arc::new(ScriptedLlm::new());
    let decisions = Arc::new(ScriptedDecisions::new());
    // No reply scripted: the agent's LLM fails.
    let mut caller = Script::says(&["Hello?"]);
    let outcome = run_text_session(&config(), &engines(&llm, &decisions), &mut caller).await;
    assert_eq!(outcome.end_reason(), SessionEndReason::Error);

    let mut failing = Script(VecDeque::from([Err(ProviderError::Unavailable)]));
    let outcome = run_text_session(&config(), &engines(&llm, &decisions), &mut failing).await;
    assert_eq!(outcome.end_reason(), SessionEndReason::Error);
    assert_eq!(outcome.into_transcript().len(), 1, "only the disclosure");

    let slow = Arc::new(ScriptedLlm::new());
    slow.push_reply([LlmStep::text("Late.", Duration::from_secs(60))]);
    let mut caller = Script::says(&["Hello?"]);
    let outcome = run_text_session(&config(), &engines(&slow, &decisions), &mut caller).await;
    assert_eq!(
        outcome.end_reason(),
        SessionEndReason::Error,
        "past its deadline"
    );
}

#[tokio::test(start_paused = true)]
async fn every_opt_out_layer_ends_with_the_confirmation() {
    let decisions = Arc::new(ScriptedDecisions::new());
    let llm = Arc::new(ScriptedLlm::new());
    let mut phrase = Script::says(&["Please stop calling me."]);
    let outcome = run_text_session(&config(), &engines(&llm, &decisions), &mut phrase).await;
    assert_eq!(
        outcome.end_reason(),
        SessionEndReason::OptOut(OptOutLayer::Phrase)
    );
    assert_eq!(
        outcome.transcript().last().map(TranscriptLine::text),
        Some(DEFAULT_OPT_OUT_CONFIRMATION)
    );

    reply(&llm, "Sure.");
    decisions.push(DecisionReply::yes_no("opt_out", 0.9));
    let mut decided = Script::says(&["I'm done with you."]);
    let outcome = run_text_session(&config(), &engines(&llm, &decisions), &mut decided).await;
    assert_eq!(
        outcome.end_reason(),
        SessionEndReason::OptOut(OptOutLayer::Decision)
    );

    llm.push_reply([LlmStep::tool_call("mark_do_not_call", "{}", QUICK)]);
    let mut tool = Script::says(&["Put me on the no-call thing."]);
    let outcome = run_text_session(&config(), &engines(&llm, &decisions), &mut tool).await;
    assert_eq!(
        outcome.end_reason(),
        SessionEndReason::OptOut(OptOutLayer::LlmTool)
    );

    // A blank confirmation still ends the conversation, without a line.
    let quiet = config().with_opt_out(OptOutPolicy::new(["stop calling"], " "));
    let mut phrase = Script::says(&["stop calling"]);
    let outcome = run_text_session(&quiet, &engines(&llm, &decisions), &mut phrase).await;
    assert_eq!(outcome.transcript().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn the_confirm_band_asks_once_and_a_failed_decision_is_no_answer() {
    let llm = Arc::new(ScriptedLlm::new());
    let decisions = Arc::new(ScriptedDecisions::new());
    reply(&llm, "Dropped.");
    reply(&llm, "Then let's go on.");
    decisions.push(DecisionReply::yes_no("opt_out", 0.5));
    decisions.push(DecisionReply::yes_no("opt_out", 0.5));
    decisions.push(DecisionReply::fail(ProviderError::Unavailable));
    reply(&llm, "Fine.");
    let mut caller = Script::says(&["Leave me alone.", "Maybe.", "Hm."]);

    let outcome = run_text_session(&config(), &engines(&llm, &decisions), &mut caller).await;

    assert_eq!(
        lines(outcome.transcript())[1..],
        [
            (Speaker::Caller, "Leave me alone."),
            (Speaker::Agent, DEFAULT_OPT_OUT_CHECK_QUESTION),
            (Speaker::Caller, "Maybe."),
            (Speaker::Agent, "Then let's go on."),
            (Speaker::Caller, "Hm."),
            (Speaker::Agent, "Fine."),
        ],
        "the answer carries on without a second check; a failed decision is no answer"
    );
    assert_eq!(outcome.end_reason(), SessionEndReason::ClientEnded);

    // Without a check question, unsure counts as an opt-out.
    let strict = config().with_opt_out(
        OptOutPolicy::new(["stop calling"], DEFAULT_OPT_OUT_CONFIRMATION).with_check_question(""),
    );
    reply(&llm, "Dropped.");
    decisions.push(DecisionReply::yes_no("opt_out", 0.5));
    let mut caller = Script::says(&["Leave me alone."]);
    let outcome = run_text_session(&strict, &engines(&llm, &decisions), &mut caller).await;
    assert_eq!(
        outcome.end_reason(),
        SessionEndReason::OptOut(OptOutLayer::Decision)
    );
}

#[tokio::test(start_paused = true)]
async fn an_empty_reply_says_nothing_and_the_conversation_goes_on() {
    let llm = Arc::new(ScriptedLlm::new());
    let decisions = Arc::new(ScriptedDecisions::new());
    llm.push_reply([]);
    let mut caller = Script::says(&["Hello?"]);
    let outcome = run_text_session(&config(), &engines(&llm, &decisions), &mut caller).await;
    assert_eq!(
        lines(outcome.transcript()),
        [(Speaker::Agent, DISCLOSURE), (Speaker::Caller, "Hello?")]
    );
}

#[tokio::test(start_paused = true)]
async fn the_llm_caller_plays_its_instructions_and_hangs_up_with_its_tool() {
    let llm = Arc::new(ScriptedLlm::new());
    llm.push_reply([LlmStep::text("A table for two.", QUICK)]);
    llm.push_reply([
        LlmStep::text("Thanks, bye.", QUICK),
        LlmStep::tool_call(CALLER_TOOL, "{}", QUICK),
    ]);
    llm.push_reply([]);
    let mut caller = LlmCaller::new(llm.clone(), "Book a table.", DEADLINES, 3);
    let transcript = vec![
        TranscriptLine::new(Speaker::Agent, DISCLOSURE),
        TranscriptLine::new(Speaker::Caller, "Hi."),
        TranscriptLine::new(Speaker::Agent, "How can I help?"),
    ];

    let first = caller.next_turn(&transcript).await.expect("a turn");
    let second = caller.next_turn(&transcript).await.expect("a turn");
    let third = caller.next_turn(&transcript).await.expect("a turn");

    assert_eq!(first, CallerTurn::Says("A table for two.".to_owned()));
    assert_eq!(
        second,
        CallerTurn::HangsUp {
            last_words: Some("Thanks, bye.".to_owned())
        }
    );
    assert_eq!(
        third,
        CallerTurn::HangsUp { last_words: None },
        "nothing to say hangs up"
    );
    let call = &llm.calls()[0];
    assert_eq!(
        call.messages,
        [
            ("system".to_owned(), "Book a table.".to_owned()),
            ("user".to_owned(), DISCLOSURE.to_owned()),
            ("assistant".to_owned(), "Hi.".to_owned()),
            ("user".to_owned(), "How can I help?".to_owned()),
        ]
    );
    assert_eq!(call.tools, [CALLER_TOOL]);
}
