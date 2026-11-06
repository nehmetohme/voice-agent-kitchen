//! The confirm band's edge paths, beside the `opt-out-confirm` contract: a blank check
//! question, an answer whose decision times out, a later in-band turn, an opt-out while the
//! check question plays, what happens to the dropped reply's LLM stream, and a configured
//! band floor. Same simulated session as the contracts (`contract_voice_core/mod.rs`, used as
//! is).
mod contract_voice_core;

use std::time::Duration;

use contract_voice_core::*;
use serde_json::json;
use vak_domain::{OptOutLayer, SessionEndReason};
use vak_pipeline::{OptOutPolicy, SessionConfig};
use vak_testkit::media::Sent;
use vak_testkit::providers::{DecisionReply, LlmStep, ScriptedTts};

const CHECK: &str = "Just to confirm, do you want us to stop contacting you?";

fn policy() -> OptOutPolicy {
    OptOutPolicy::new(OPT_OUT_PHRASES, CONFIRMATION)
        .with_check_question(CHECK)
        .with_decision_wait(DECISION_WAIT)
        .with_decision_deadline(DECISION_DEADLINE)
}

fn session(policy: OptOutPolicy) -> SessionConfig {
    config().with_opt_out(policy)
}

fn fakes() -> Fakes {
    Fakes::with_tts(ScriptedTts::new().with_audio_for(CHECK, Duration::from_secs(2)))
}

/// One caller turn: `text` (500 ms of speech, transcribed in 50 ms), scored `probability`
/// after 100 ms, with `reply` as the LLM's answer (after 50 ms).
fn script_turn(fakes: &Fakes, text: &str, probability: f64, reply: &str) {
    fakes.stt.push_reply(text, ms(50));
    fakes
        .decisions
        .push(DecisionReply::yes_no("opt_out", probability).after(ms(100)));
    fakes.llm.push_reply([LlmStep::text(reply, ms(50))]);
}

fn agent_said(log: &[Sent]) -> Vec<String> {
    transcripts_of(log, "agent")
        .into_iter()
        .map(|(_, text, _)| text)
        .collect()
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_blank_check_question_turns_the_band_off() {
    let fakes = fakes();
    script_turn(&fakes, "It's a one.", 0.5, "Thanks.");
    let call = start(&fakes, session(policy().with_check_question("  ")));
    call.begin().await;
    call.client.speak(ms(500)).await;
    call.wait_for("the session to end", |log| has_type(log, "session.ended"))
        .await;
    let ended = call.finished().await;

    assert_eq!(
        ended.outcome.end_reason(),
        SessionEndReason::OptOut(OptOutLayer::Decision),
        "without a check question, unsure counts as an opt-out"
    );
    assert_eq!(agent_said(&ended.log), [DISCLOSURE, CONFIRMATION]);
    assert_matches_media_schema(&ended.log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_band_floor_is_configurable() {
    let fakes = fakes();
    script_turn(&fakes, "It's a one.", 0.5, "Thanks.");
    let call = start(&fakes, session(policy().with_confirm_threshold(0.6)));
    call.begin().await;
    call.client.speak(ms(500)).await;
    call.wait_for("the reply", |log| has(log, "agent.audio.end", 2))
        .await;
    let ended = call.hang_up().await;

    assert_eq!(agent_said(&ended.log), [DISCLOSURE, "Thanks."]);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_answer_whose_decision_times_out_settles_the_check_and_a_later_turn_may_ask_again() {
    let fakes = fakes();
    script_turn(&fakes, "It's a one.", 0.343, "Sorry to hear that.");
    let call = start(&fakes, session(policy()));
    call.begin().await;
    call.client.speak(ms(500)).await;
    call.wait_for("the check question", |log| has(log, "agent.audio.end", 2))
        .await;

    // The answer's decision times out: the call goes on, and the check is settled.
    fakes.stt.push_reply("Hmm.", ms(50));
    fakes.decisions.push(DecisionReply::timeout());
    fakes
        .llm
        .push_reply([LlmStep::text("Shall we go on?", ms(50))]);
    call.client.speak(ms(500)).await;
    call.wait_for("the reply to the answer", |log| {
        has(log, "agent.audio.end", 4)
    })
    .await;

    // A later turn in the band asks again.
    script_turn(&fakes, "Two.", 0.4, "Got it.");
    call.client.speak(ms(500)).await;
    call.wait_for("the second check question", |log| {
        has(log, "agent.audio.end", 6)
    })
    .await;
    let ended = call.hang_up().await;

    assert_eq!(
        agent_said(&ended.log),
        [DISCLOSURE, CHECK, "Shall we go on?", CHECK]
    );
    let statuses: Vec<String> = events(&ended.log)
        .into_iter()
        .filter(|(_, event)| event["type"] == "decision")
        .map(|(_, event)| event["status"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(statuses, ["answered", "timeout", "answered"]);
    assert_eq!(ended.outcome.end_reason(), SessionEndReason::ClientEnded);
    assert_matches_media_schema(&ended.log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_opt_out_while_the_check_question_plays_lets_it_finish_then_confirms() {
    let fakes = fakes();
    script_turn(&fakes, "It's a one.", 0.343, "Sorry to hear that.");
    let call = start(&fakes, session(policy()));
    call.begin().await;
    call.client.speak(ms(500)).await;
    call.wait_for("the check question to start", |log| {
        has(log, "agent.audio.start", 2)
    })
    .await;
    fakes.stt.push_reply("Stop calling me.", ms(50));
    // Over the check question: the phrase list catches it before it ends.
    call.client.speak(ms(400)).await;
    call.wait_for("the session to end", |log| has_type(log, "session.ended"))
        .await;
    let ended = call.finished().await;
    let log = &ended.log;

    assert_eq!(
        ended.outcome.end_reason(),
        SessionEndReason::OptOut(OptOutLayer::Phrase)
    );
    assert_eq!(agent_said(log), [DISCLOSURE, CHECK, CONFIRMATION]);
    let check_end = at(log, "agent.audio.end", 2).expect("the check question ends");
    let confirmation = at(log, "agent.audio.start", 4).expect("the confirmation starts");
    assert!(
        confirmation >= check_end,
        "the confirmation waits for the check question"
    );
    assert_eq!(
        last_event(log),
        json!({"type": "session.ended", "reason": "opt_out", "opt_out_layer": "phrase"})
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_dropped_reply_streams_to_its_end_unspoken() {
    let fakes = fakes();
    fakes.stt.push_reply("It's a one.", ms(50));
    fakes
        .decisions
        .push(DecisionReply::yes_no("opt_out", 0.343).after(ms(100)));
    // The reply's first clause is ready before the score, the rest streams after it.
    fakes.llm.push_reply([
        LlmStep::text("Sorry to hear that. ", ms(50)),
        LlmStep::text("What went wrong?", ms(200)),
    ]);
    let call = start(&fakes, session(policy()));
    call.begin().await;
    call.client.speak(ms(500)).await;
    call.wait_for("the check question", |log| has(log, "agent.audio.end", 2))
        .await;
    let ended = call.hang_up().await;

    assert_eq!(agent_said(&ended.log), [DISCLOSURE, CHECK]);
    let llm = fakes.llm.calls();
    let dropped = llm.first().expect("the LLM was asked");
    assert!(
        dropped.completed,
        "the dropped reply's stream runs to its end"
    );
    assert!(dropped.cancelled_at.is_none());
    assert!(
        fakes
            .tts
            .calls()
            .iter()
            .all(|call| call.text != "What went wrong?"),
        "nothing of the dropped reply is synthesized after it is dropped"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_dropped_reply_stops_listening_once_the_caller_answers() {
    let fakes = fakes();
    fakes.stt.push_reply("It's a one.", ms(50));
    fakes
        .decisions
        .push(DecisionReply::yes_no("opt_out", 0.343).after(ms(100)));
    // A slow stream, still running when the caller answers the check question.
    fakes.llm.push_reply([
        LlmStep::text("Sorry to hear that. ", ms(50)),
        LlmStep::text("What went wrong?", Duration::from_secs(10)),
    ]);
    let call = start(&fakes, session(policy()));
    call.begin().await;
    call.client.speak(ms(500)).await;
    call.wait_for("the check question", |log| has(log, "agent.audio.end", 2))
        .await;
    fakes.stt.push_reply("No.", ms(50));
    fakes.decisions.push(DecisionReply::yes_no("opt_out", 0.55));
    fakes.llm.push_reply([LlmStep::text("Okay.", ms(50))]);
    call.client.speak(ms(500)).await;
    call.wait_for("the reply to the answer", |log| {
        has(log, "agent.audio.end", 4)
    })
    .await;
    let ended = call.hang_up().await;

    assert_eq!(agent_said(&ended.log), [DISCLOSURE, CHECK, "Okay."]);
    let llm = fakes.llm.calls();
    let dropped = llm.first().expect("the LLM was asked");
    let answering = llm.get(1).expect("the LLM answers the answer");
    let cancelled = dropped
        .cancelled_at
        .expect("the dropped stream is cancelled");
    assert!(
        cancelled <= answering.started_at,
        "the dropped stream stops before the next request starts"
    );
}
