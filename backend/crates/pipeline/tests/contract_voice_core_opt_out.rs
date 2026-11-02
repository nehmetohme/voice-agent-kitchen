//! Contract `voice-core` v1 (opt-out): a caller's request not to be contacted again ends the
//! session, whichever of the three layers catches it, and a slow or failing decision model
//! never suppresses it. Same simulated session as `contract_voice_core_pipeline.rs`
//! (`contract_voice_core/mod.rs`): phrases "stop calling" and "take me off your list", the
//! decision model's yes/no `opt_out` question with threshold 0.5, a 300 ms wait for its answer
//! and a 2 s deadline, and the LLM tool `mark_do_not_call`.
//!
//! What it pins:
//! - **Each layer alone ends the session**: the phrase list (any case and punctuation), the
//!   decision model (`opt_out` at or above the threshold), and the LLM calling
//!   `mark_do_not_call`, which every LLM request offers.
//! - **How it ends:** the agent says the locked confirmation in full as the next agent turn,
//!   then `session.ended` with `reason: opt_out` and the layer that caught it is the last
//!   event, the session finishes without the caller hanging up, and its outcome says
//!   `OptOut(layer)`. The LLM's reply to that turn is never spoken: no pitch after an opt-out.
//! - **A decision timeout never suppresses an opt-out:** with the decision model timing out
//!   or failing, the phrase list and the LLM tool still end the session; an answer that comes
//!   after the 300 ms wait (but before the deadline) still ends it; and a timeout or failure
//!   is reported as `status: timeout` or `error`, without a probability, never as a no.
//! - Every event the session sends matches the media schema.
//!
//! Contract file: never edit it once it is tagged `contract/voice-core/v1`. To change the
//! contract, add a new version.
#![expect(
    clippy::expect_used,
    reason = "test helpers: a failed expectation is a test failure"
)]

mod contract_voice_core;

use contract_voice_core::*;
use serde_json::{Value, json};
use vak_domain::{OptOutLayer, SessionEndReason};
use vak_providers::ProviderError;
use vak_testkit::providers::{DecisionReply, LlmStep};

/// The caller says `text` (500 ms of speech, transcribed in 50 ms) after the disclosure, and
/// the session runs until it ends on its own.
async fn caller_says(fakes: &Fakes, text: &str) -> Ended {
    fakes.stt.push_reply(text, ms(50));
    let call = start(fakes, config());
    call.begin().await;
    call.client.speak(ms(500)).await;
    call.wait_for("the session to end", |log| has_type(log, "session.ended"))
        .await;
    call.finished().await
}

/// The session ended by an opt-out that `layer` caught, after the full confirmation.
fn assert_opted_out(ended: &Ended, wire_layer: &str, layer: OptOutLayer) {
    let log = &ended.log;
    assert_eq!(
        last_event(log),
        json!({"type": "session.ended", "reason": "opt_out", "opt_out_layer": wire_layer}),
        "the session ends with an opt-out caught by the {wire_layer} layer"
    );
    assert_eq!(ended.outcome.end_reason(), SessionEndReason::OptOut(layer));
    let agent = transcripts_of(log, "agent");
    let (turn, text, interrupted) = agent.last().expect("the agent spoke").clone();
    assert_eq!(
        text, CONFIRMATION,
        "the agent's last words are the locked confirmation"
    );
    assert!(!interrupted, "the confirmation is not cut short");
    let end = index_of(log, "agent.audio.end", turn).expect("the confirmation is spoken in full");
    let start = index_of(log, "agent.audio.start", turn).expect("the confirmation starts");
    assert_eq!(
        samples_between(log, start, end),
        3 * OUTPUT_SAMPLES_PER_SECOND / 2,
        "all 1.5 s of the confirmation are sent"
    );
    assert_matches_media_schema(log);
}

/// The agent said nothing but the disclosure and the confirmation.
fn assert_no_reply_spoken(ended: &Ended) {
    assert_eq!(
        transcripts_of(&ended.log, "agent"),
        [
            (0, DISCLOSURE.to_owned(), false),
            (2, CONFIRMATION.to_owned(), false)
        ],
        "the LLM's reply to an opt-out is never spoken"
    );
    assert_eq!(turns_of(&ended.log, "agent.audio.start"), [0, 2]);
}

/// The `decision` events about the opt-out question.
fn opt_out_decisions(ended: &Ended) -> Vec<Value> {
    events(&ended.log)
        .into_iter()
        .filter(|(_, event)| event["type"] == "decision" && event["question"] == "opt_out")
        .map(|(_, event)| event.clone())
        .collect()
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_phrase_list_alone_ends_the_session() {
    let fakes = Fakes::new();
    fakes.decisions.push(DecisionReply::yes_no("opt_out", 0.0));
    fakes.llm.push_reply([LlmStep::text(
        "Of course! Before you go, have you tried our tasting menu?",
        ms(100),
    )]);

    let ended = caller_says(&fakes, "Please, STOP calling me!").await;

    assert_opted_out(&ended, "phrase", OptOutLayer::Phrase);
    assert_no_reply_spoken(&ended);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_decision_model_alone_ends_the_session() {
    const CALLER: &str = "I really don't want to hear from you people ever again.";
    let fakes = Fakes::new();
    fakes
        .decisions
        .push(DecisionReply::yes_no("opt_out", 0.9).after(ms(100)));
    // The reply is ready before the decision; it must still not be spoken.
    fakes.llm.push_reply([LlmStep::text(
        "I'm sorry to hear that, can I offer you a discount?",
        ms(50),
    )]);

    let ended = caller_says(&fakes, CALLER).await;

    assert_opted_out(&ended, "decision", OptOutLayer::Decision);
    assert_no_reply_spoken(&ended);
    let calls = fakes.decisions.calls();
    let call = calls.first().expect("the decision model was asked");
    assert!(call.questions.iter().any(|question| question == "opt_out"));
    assert!(
        call.turns
            .iter()
            .any(|(speaker, text)| speaker == "caller" && text == CALLER),
        "the decision model sees what the caller said"
    );
    let decisions = opt_out_decisions(&ended);
    let decision = decisions.first().expect("a decision event");
    assert_eq!(decision["turn_id"], 1);
    assert_eq!(decision["status"], "answered");
    let probability = decision["probability"].as_f64().expect("a probability");
    assert!(
        (probability - 0.9).abs() < 1e-6,
        "probability {probability}"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_llm_tool_alone_ends_the_session() {
    let fakes = Fakes::new();
    fakes.decisions.push(DecisionReply::yes_no("opt_out", 0.0));
    fakes
        .llm
        .push_reply([LlmStep::tool_call(OPT_OUT_TOOL, "{}", ms(200))]);

    let ended = caller_says(&fakes, "Hmm, I'm not sure this is for me.").await;

    assert_opted_out(&ended, "llm_tool", OptOutLayer::LlmTool);
    assert_no_reply_spoken(&ended);
    let llm = fakes.llm.calls();
    assert!(
        llm.first()
            .expect("the LLM was asked")
            .tools
            .iter()
            .any(|tool| tool == OPT_OUT_TOOL),
        "the LLM is offered the opt-out tool"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_decision_timeout_does_not_suppress_the_phrase_list() {
    let fakes = Fakes::new();
    fakes.decisions.push(DecisionReply::timeout());
    fakes
        .llm
        .push_reply([LlmStep::text("Sure, what else can I do for you?", ms(100))]);

    let ended = caller_says(&fakes, "Take me off your list, please.").await;

    assert_opted_out(&ended, "phrase", OptOutLayer::Phrase);
    assert_no_reply_spoken(&ended);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_decision_timeout_does_not_suppress_the_llm_tool() {
    let fakes = Fakes::new();
    fakes.decisions.push(DecisionReply::timeout());
    fakes
        .llm
        .push_reply([LlmStep::tool_call(OPT_OUT_TOOL, "{}", ms(200))]);

    let ended = caller_says(&fakes, "I'd rather not continue this conversation.").await;

    assert_opted_out(&ended, "llm_tool", OptOutLayer::LlmTool);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_failed_decision_does_not_suppress_the_phrase_list() {
    let fakes = Fakes::new();
    fakes
        .decisions
        .push(DecisionReply::fail(ProviderError::Unavailable));
    fakes
        .llm
        .push_reply([LlmStep::text("Sure, what else can I do for you?", ms(100))]);

    let ended = caller_says(&fakes, "Stop calling this number.").await;

    assert_opted_out(&ended, "phrase", OptOutLayer::Phrase);
    assert_no_reply_spoken(&ended);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_late_decision_still_ends_the_session() {
    let fakes = Fakes::new();
    // After the 300 ms wait, before the 2 s deadline: the agent may already be replying.
    fakes
        .decisions
        .push(DecisionReply::yes_no("opt_out", 0.9).after(ms(800)));
    fakes.llm.push_reply([LlmStep::text(
        "No problem, let me tell you about our specials tonight.",
        ms(50),
    )]);

    let ended = caller_says(&fakes, "I've heard enough of this, honestly.").await;

    assert_opted_out(&ended, "decision", OptOutLayer::Decision);
}

/// The caller asks a question the decision model can't answer in time (`reply`); the
/// conversation goes on, and the caller hangs up once the answer and the decision event are in.
async fn conversation_with_a_broken_decision_model(reply: DecisionReply) -> Ended {
    const ANSWER: &str = "We open at noon tomorrow.";
    let fakes = Fakes::new();
    fakes.decisions.push(reply);
    fakes
        .stt
        .push_reply("What time do you open tomorrow?", ms(50));
    fakes.llm.push_reply([LlmStep::text(ANSWER, ms(100))]);
    let call = start(&fakes, config());
    call.begin().await;
    call.client.speak(ms(500)).await;
    call.wait_for("the answer and the opt-out decision", |log| {
        has(log, "agent.audio.end", 2)
            && events(log)
                .iter()
                .any(|(_, event)| event["type"] == "decision" && event["question"] == "opt_out")
    })
    .await;
    let ended = call.hang_up().await;
    assert!(
        transcripts_of(&ended.log, "agent").contains(&(2, ANSWER.to_owned(), false)),
        "the agent still answers"
    );
    assert_eq!(
        last_event(&ended.log),
        json!({"type": "session.ended", "reason": "client_ended"})
    );
    assert_eq!(ended.outcome.end_reason(), SessionEndReason::ClientEnded);
    assert_matches_media_schema(&ended.log);
    ended
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_decision_timeout_is_reported_as_a_timeout_not_a_no() {
    let ended = conversation_with_a_broken_decision_model(DecisionReply::timeout()).await;

    let decisions = opt_out_decisions(&ended);
    let decision = decisions.first().expect("a decision event");
    assert_eq!(decision["turn_id"], 1);
    assert_eq!(decision["status"], "timeout");
    assert!(decision.get("probability").is_none() && decision.get("choice").is_none());
    let latency = decision["latency_ms"].as_u64().expect("a latency");
    let deadline = u64::try_from(DECISION_DEADLINE.as_millis()).expect("small");
    let frame = u64::try_from(FRAME.as_millis()).expect("small");
    assert!(
        latency + frame >= deadline,
        "the decision timed out after {latency} ms, before its {deadline} ms deadline"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_failed_decision_is_reported_as_an_error_not_a_no() {
    let ended =
        conversation_with_a_broken_decision_model(DecisionReply::fail(ProviderError::Unavailable))
            .await;

    let decisions = opt_out_decisions(&ended);
    let decision = decisions.first().expect("a decision event");
    assert_eq!(decision["turn_id"], 1);
    assert_eq!(decision["status"], "error");
    assert!(decision.get("probability").is_none() && decision.get("choice").is_none());
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_opt_out_question_is_asked_about_every_caller_turn() {
    let fakes = Fakes::new();
    fakes.stt.push_reply("Hi, I'd like a table.", ms(50));
    fakes.stt.push_reply("For two, please.", ms(50));
    fakes
        .llm
        .push_reply([LlmStep::text("Sure, for how many people?", ms(100))]);
    fakes
        .llm
        .push_reply([LlmStep::text("Great, a table for two.", ms(100))]);
    let call = start(&fakes, config());
    call.begin().await;
    call.client.speak(ms(400)).await;
    call.wait_for("the first reply", |log| has(log, "agent.audio.end", 2))
        .await;
    call.client.speak(ms(400)).await;
    call.wait_for("the second reply", |log| has(log, "agent.audio.end", 4))
        .await;
    let ended = call.hang_up().await;

    let calls = fakes.decisions.calls();
    assert_eq!(calls.len(), 2, "one decision call per caller turn");
    for call in &calls {
        assert!(call.questions.iter().any(|question| question == "opt_out"));
    }
    assert!(
        calls[1]
            .turns
            .iter()
            .any(|(speaker, text)| speaker == "caller" && text == "For two, please."),
        "the second call sees the second caller turn"
    );
    let answered: Vec<Value> = opt_out_decisions(&ended);
    assert_eq!(
        answered.iter().map(turn_id).collect::<Vec<_>>(),
        [1, 3],
        "one opt-out decision event per caller turn"
    );
    for decision in &answered {
        assert_eq!(decision["status"], "answered");
        assert_eq!(decision["probability"], json!(0.0));
    }
    assert_eq!(ended.outcome.end_reason(), SessionEndReason::ClientEnded);
    assert_matches_media_schema(&ended.log);
}
