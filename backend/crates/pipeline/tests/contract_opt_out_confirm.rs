//! Contract `opt-out-confirm` v1: confirm when unsure. The decision model's `opt_out` score
//! no longer ends a call on its own unless it is sure: a score in the confirm band makes the
//! agent ask a fixed check question, and the caller's answer decides. Same simulated session
//! as the `voice-core` contracts (`contract_voice_core/mod.rs`, used as is), but with the
//! platform's default opt-out thresholds and the check question `CHECK`.
//!
//! What it pins:
//! - **The bands** (the defaults of `OptOutPolicy::new`): a score below 0.3 does nothing; from
//!   0.3 up to (not including) the end threshold 0.8 the agent asks the check question; at
//!   0.8 or more the session ends at once with the locked confirmation, as before.
//! - **The check question replaces the reply.** It is the next agent turn, spoken word for
//!   word and in full, and the caller can't interrupt it. The LLM's reply to the in-band turn
//!   is never spoken. The check question reports a `latency` event like a reply, with
//!   `llm_ttft_ms: null` (the LLM didn't produce it).
//! - **A late in-band answer** (after the 300 ms wait, while the reply plays) cuts the reply
//!   (`agent.interrupted`, then its `transcript.final` with `interrupted: true`) and the check
//!   question follows at once.
//! - **The answer decides.** The caller's next turn is judged with the check question in the
//!   decision model's context: a score at or above 0.8 ends the session with the locked
//!   confirmation and `session.ended` reason `opt_out`, layer `decision`. Anything below
//!   (a no, a question, a score that would be in the band) carries on: the LLM answers, having
//!   seen the check question, and the agent never asks twice in a row. Speech over the check
//!   question is that answer.
//! - **Recall never drops.** The LLM's `mark_do_not_call` tool still ends the call at once
//!   while a check is pending, and a check question that can't be synthesized ends the call
//!   as an opt-out, as an in-band score did before.
//! - Every event the session sends matches the media schema.
//!
//! Contract file: never edit it once it is tagged `contract/opt-out-confirm/v1`. To change
//! the contract, add a new version.
#![expect(
    clippy::expect_used,
    reason = "test helpers: a failed expectation is a test failure"
)]

mod contract_voice_core;

use std::time::Duration;

use contract_voice_core::*;
use serde_json::{Value, json};
use vak_domain::{OptOutLayer, SessionEndReason, Speaker};
use vak_pipeline::{OptOutPolicy, SessionConfig};
use vak_testkit::media::Sent;
use vak_testkit::providers::{DecisionReply, LlmStep, ScriptedTts};

/// The check question this contract configures.
const CHECK: &str = "Just to confirm, do you want us to stop contacting you?";
/// How long the scripted TTS speaks the check question.
const CHECK_AUDIO: Duration = Duration::from_secs(2);
/// The answer that ended the owner's first live test call as a false opt-out.
const SURVEY_ANSWER: &str = "It's a one.";
/// The LLM's reply to `SURVEY_ANSWER`.
const REPLY: &str = "Thanks for telling us. What went wrong?";
/// The LLM's reply to the caller's answer to the check question.
const NEXT: &str = "Okay, let's go on.";
const STT_AFTER: Duration = ms(50);
const DECISION_AFTER: Duration = ms(100);

/// The contract session (`contract_voice_core::config()`) with the default opt-out
/// thresholds and `CHECK` as the check question.
fn confirm_config() -> SessionConfig {
    config().with_opt_out(
        OptOutPolicy::new(OPT_OUT_PHRASES, CONFIRMATION)
            .with_check_question(CHECK)
            .with_decision_wait(DECISION_WAIT)
            .with_decision_deadline(DECISION_DEADLINE),
    )
}

/// The contract engines; the TTS speaks the check question in 2 s.
fn fakes_with(tts: ScriptedTts) -> Fakes {
    Fakes::with_tts(tts.with_audio_for(CHECK, CHECK_AUDIO))
}

fn fakes() -> Fakes {
    fakes_with(ScriptedTts::new())
}

/// After the disclosure the caller says `SURVEY_ANSWER` (500 ms of speech, transcribed in
/// 50 ms). The decision model scores it `probability` after 100 ms; the LLM's reply `REPLY`
/// streams after 50 ms, so it is ready before the score.
async fn survey_answer_scored(fakes: &Fakes, probability: f64) -> Call {
    fakes.stt.push_reply(SURVEY_ANSWER, STT_AFTER);
    fakes
        .decisions
        .push(DecisionReply::yes_no("opt_out", probability).after(DECISION_AFTER));
    fakes.llm.push_reply([LlmStep::text(REPLY, ms(50))]);
    let call = start(fakes, confirm_config());
    call.begin().await;
    call.client.speak(ms(500)).await;
    call
}

/// `survey_answer_scored` with a score in the band, until the check question has been asked.
async fn check_asked(fakes: &Fakes) -> Call {
    let call = survey_answer_scored(fakes, 0.343).await;
    call.wait_for("the check question", |log| has(log, "agent.audio.end", 2))
        .await;
    call
}

/// The caller answers `answer` (500 ms of speech), scored `probability` at once; the LLM
/// would reply `reply`.
async fn caller_answers(call: &Call, fakes: &Fakes, answer: &str, probability: f64, reply: &str) {
    fakes.stt.push_reply(answer, STT_AFTER);
    fakes
        .decisions
        .push(DecisionReply::yes_no("opt_out", probability));
    fakes.llm.push_reply([LlmStep::text(reply, ms(50))]);
    call.client.speak(ms(500)).await;
}

async fn until_ended(call: Call) -> Ended {
    call.wait_for("the session to end", |log| has_type(log, "session.ended"))
        .await;
    call.finished().await
}

fn said(turns: &[(u32, &str)]) -> Vec<(u32, String, bool)> {
    turns
        .iter()
        .map(|(turn, text)| (*turn, (*text).to_owned(), false))
        .collect()
}

/// The audio samples the browser received for agent turn `turn`.
fn turn_samples(log: &[Sent], turn: u32) -> usize {
    let start = index_of(log, "agent.audio.start", turn).expect("the turn starts");
    let end = index_of(log, "agent.audio.end", turn)
        .or_else(|| index_of(log, "agent.interrupted", turn))
        .expect("the turn ends");
    samples_between(log, start, end)
}

/// The session ended on an opt-out caught by `layer`, after the full locked confirmation.
fn assert_opted_out(ended: &Ended, wire_layer: &str, layer: OptOutLayer) {
    let log = &ended.log;
    assert_eq!(
        last_event(log),
        json!({"type": "session.ended", "reason": "opt_out", "opt_out_layer": wire_layer})
    );
    assert_eq!(ended.outcome.end_reason(), SessionEndReason::OptOut(layer));
    let agent = transcripts_of(log, "agent");
    let (turn, text, interrupted) = agent.last().expect("the agent spoke").clone();
    assert_eq!(
        text, CONFIRMATION,
        "the agent's last words are the confirmation"
    );
    assert!(!interrupted);
    assert_eq!(
        turn_samples(log, turn),
        3 * OUTPUT_SAMPLES_PER_SECOND / 2,
        "all 1.5 s of the confirmation are sent"
    );
}

fn assert_carried_on(ended: &Ended) {
    assert_eq!(ended.outcome.end_reason(), SessionEndReason::ClientEnded);
    assert_eq!(
        last_event(&ended.log),
        json!({"type": "session.ended", "reason": "client_ended"})
    );
}

/// The `decision` events about the opt-out question.
fn opt_out_decisions(log: &[Sent]) -> Vec<Value> {
    events(log)
        .into_iter()
        .filter(|(_, event)| event["type"] == "decision" && event["question"] == "opt_out")
        .map(|(_, event)| event.clone())
        .collect()
}

/// How often the check question was synthesized.
fn checks_synthesized(fakes: &Fakes) -> usize {
    fakes
        .tts
        .calls()
        .iter()
        .filter(|call| call.text == CHECK)
        .count()
}

/// The last two turns the decision model saw in its `call`-th request.
fn last_decision_turns(fakes: &Fakes, call: usize) -> Vec<(String, String)> {
    let calls = fakes.decisions.calls();
    let turns = &calls.get(call).expect("the decision model was asked").turns;
    turns[turns.len().saturating_sub(2)..].to_vec()
}

fn pair(speaker: &str, text: &str) -> (String, String) {
    (speaker.to_owned(), text.to_owned())
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_score_in_the_confirm_band_asks_the_check_question_instead_of_ending_the_call() {
    let fakes = fakes();
    let call = check_asked(&fakes).await;
    let ended = call.hang_up().await;
    let log = &ended.log;

    assert_carried_on(&ended);
    assert_eq!(
        transcripts_of(log, "agent"),
        said(&[(0, DISCLOSURE), (2, CHECK)]),
        "the agent asks the check question, and its reply is never spoken"
    );
    assert_eq!(
        turn_samples(log, 2),
        2 * OUTPUT_SAMPLES_PER_SECOND,
        "the check question is spoken in full"
    );
    assert_eq!(checks_synthesized(&fakes), 1);

    let decisions = opt_out_decisions(log);
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0]["turn_id"], 1);
    assert_eq!(decisions[0]["status"], "answered");
    let probability = decisions[0]["probability"].as_f64().expect("a probability");
    assert!(
        (probability - 0.343).abs() < 1e-6,
        "probability {probability}"
    );

    let latency: Vec<Value> = events(log)
        .into_iter()
        .filter(|(_, event)| event["type"] == "latency")
        .map(|(_, event)| event.clone())
        .collect();
    assert_eq!(latency.len(), 1, "the check question reports its timings");
    let event = &latency[0];
    assert_eq!(event["turn_id"], 2);
    assert_eq!(event["stt_ms"], json!(50));
    assert_eq!(
        event["llm_ttft_ms"],
        Value::Null,
        "the check question doesn't come from the LLM"
    );
    assert_eq!(event["tts_first_audio_ms"], json!(0));
    let mouth_to_ear = event["mouth_to_ear_ms"].as_u64().expect("mouth-to-ear");
    let budget =
        u64::try_from((END_OF_TURN + STT_AFTER + DECISION_AFTER).as_millis()).expect("small");
    let frame = u64::try_from(FRAME.as_millis()).expect("small");
    assert!(
        mouth_to_ear.abs_diff(budget) <= frame,
        "mouth-to-ear {mouth_to_ear} ms, expected {budget} ms ± one frame"
    );
    let record = ended
        .outcome
        .turns()
        .iter()
        .find(|record| record.turn_id() == 2)
        .expect("the check question is a turn of the outcome");
    assert_eq!(record.speaker(), Speaker::Agent);
    let latencies = record.latencies().expect("its timings are recorded");
    assert_eq!(latencies.stt(), Some(STT_AFTER));
    assert_eq!(latencies.llm_ttft(), None);
    assert_matches_media_schema(log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_clear_yes_to_the_check_question_ends_the_call_with_the_locked_confirmation() {
    let fakes = fakes();
    let call = check_asked(&fakes).await;
    caller_answers(&call, &fakes, "Yes.", 0.944, "Okay, thanks for your time!").await;
    let ended = until_ended(call).await;

    assert_opted_out(&ended, "decision", OptOutLayer::Decision);
    assert_eq!(
        transcripts_of(&ended.log, "agent"),
        said(&[(0, DISCLOSURE), (2, CHECK), (4, CONFIRMATION)]),
        "the LLM's reply to the yes is never spoken"
    );
    assert_eq!(
        last_decision_turns(&fakes, 1),
        [pair("agent", CHECK), pair("caller", "Yes.")],
        "the decision model judges the answer with the check question in its context"
    );
    assert_matches_media_schema(&ended.log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn anything_short_of_a_clear_yes_carries_on_without_asking_again() {
    // Both answers score in the confirm band once the check question is in the context.
    for (answer, probability) in [("No, go on.", 0.457), ("What?", 0.703)] {
        let fakes = fakes();
        let call = check_asked(&fakes).await;
        caller_answers(&call, &fakes, answer, probability, NEXT).await;
        call.wait_for("the reply to the answer", |log| {
            has(log, "agent.audio.end", 4)
        })
        .await;
        let ended = call.hang_up().await;

        assert_carried_on(&ended);
        assert_eq!(
            transcripts_of(&ended.log, "agent"),
            said(&[(0, DISCLOSURE), (2, CHECK), (4, NEXT)]),
            "{answer}: the LLM answers, and the check question is not asked again"
        );
        assert_eq!(checks_synthesized(&fakes), 1, "{answer}");
        let llm = fakes.llm.calls();
        let answering = llm.get(1).expect("the LLM answers the answer");
        assert_system_prompt(answering);
        assert_eq!(
            history(answering),
            [
                turn("assistant", DISCLOSURE),
                turn("user", SURVEY_ANSWER),
                turn("assistant", CHECK),
                turn("user", answer),
            ],
            "{answer}: the LLM sees the check question in place of its dropped reply"
        );
        assert_matches_media_schema(&ended.log);
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_confirm_band_runs_from_0_3_up_to_the_0_8_end_threshold() {
    for (probability, expected) in [
        (0.29, REPLY),
        (0.3, CHECK),
        (0.79, CHECK),
        (0.8, CONFIRMATION),
    ] {
        let fakes = fakes();
        let call = survey_answer_scored(&fakes, probability).await;
        let ended = if expected == CONFIRMATION {
            until_ended(call).await
        } else {
            call.wait_for("the agent's next turn", |log| {
                has(log, "agent.audio.end", 2)
            })
            .await;
            call.hang_up().await
        };

        assert_eq!(
            transcripts_of(&ended.log, "agent"),
            said(&[(0, DISCLOSURE), (2, expected)]),
            "p = {probability}"
        );
        if expected == CONFIRMATION {
            assert_opted_out(&ended, "decision", OptOutLayer::Decision);
        } else {
            assert_carried_on(&ended);
        }
        assert_matches_media_schema(&ended.log);
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_late_in_band_score_cuts_the_playing_reply_and_asks_at_once() {
    const LONG: &str = "I am so sorry to hear that our service let you down.";
    let fakes = fakes_with(ScriptedTts::new().with_audio_for(LONG, Duration::from_secs(4)));
    fakes.stt.push_reply(SURVEY_ANSWER, STT_AFTER);
    // After the 300 ms wait, before the 2 s deadline: the reply is already playing.
    fakes
        .decisions
        .push(DecisionReply::yes_no("opt_out", 0.5).after(ms(1_000)));
    fakes.llm.push_reply([LlmStep::text(LONG, ms(50))]);
    let call = start(&fakes, confirm_config());
    call.begin().await;
    call.client.speak(ms(500)).await;
    call.wait_for("the check question", |log| has(log, "agent.audio.end", 3))
        .await;
    let ended = call.hang_up().await;
    let log = &ended.log;

    assert_carried_on(&ended);
    assert_eq!(turns_of(log, "agent.audio.start"), [0, 2, 3]);
    let decided = events(log)
        .into_iter()
        .find(|(_, event)| event["type"] == "decision")
        .map(|(at, _)| at)
        .expect("a decision event");
    let cut = at(log, "agent.interrupted", 2).expect("the reply is cut");
    let asked = at(log, "agent.audio.start", 3).expect("the check question starts");
    assert!(cut >= decided, "the reply is cut only once the score is in");
    assert!(
        asked <= cut + FRAME,
        "the check question follows the cut at once"
    );
    let cut_at = index_of(log, "agent.interrupted", 2).expect("the cut");
    let asked_at = index_of(log, "agent.audio.start", 3).expect("the check question");
    assert_eq!(
        samples_between(log, cut_at, asked_at),
        0,
        "no audio of the reply after the cut"
    );

    let agent = transcripts_of(log, "agent");
    assert_eq!(agent.len(), 3);
    let (turn, spoken, interrupted) = &agent[1];
    assert_eq!(*turn, 2);
    assert!(
        *interrupted,
        "the cut reply's transcript says it was interrupted"
    );
    assert!(
        !spoken.is_empty() && LONG.starts_with(spoken.as_str()),
        "the cut reply's transcript holds the words spoken: {spoken:?}"
    );
    assert_eq!(agent[2], (3, CHECK.to_owned(), false));
    assert_eq!(turn_samples(log, 3), 2 * OUTPUT_SAMPLES_PER_SECOND);
    assert_matches_media_schema(log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_llm_tool_still_ends_the_call_at_once_while_a_check_is_pending() {
    // Every synthesis takes 100 ms, so the check question is still being synthesized when
    // the LLM calls its tool.
    let fakes = fakes_with(ScriptedTts::new().with_delay(ms(100)));
    fakes.stt.push_reply(
        "Honestly, I don't want to hear from you people again.",
        STT_AFTER,
    );
    fakes
        .decisions
        .push(DecisionReply::yes_no("opt_out", 0.5).after(ms(20)));
    fakes
        .llm
        .push_reply([LlmStep::tool_call(OPT_OUT_TOOL, "{}", ms(60))]);
    let call = start(&fakes, confirm_config());
    call.begin().await;
    call.client.speak(ms(500)).await;
    let ended = until_ended(call).await;

    assert_opted_out(&ended, "llm_tool", OptOutLayer::LlmTool);
    assert_eq!(
        transcripts_of(&ended.log, "agent"),
        said(&[(0, DISCLOSURE), (2, CONFIRMATION)]),
        "the check question is never spoken once the caller has opted out"
    );
    assert_matches_media_schema(&ended.log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_check_question_that_cannot_be_spoken_ends_the_call_as_an_opt_out() {
    let fakes = fakes_with(ScriptedTts::new().with_failure_for(CHECK));
    let call = survey_answer_scored(&fakes, 0.5).await;
    let ended = until_ended(call).await;

    assert_opted_out(&ended, "decision", OptOutLayer::Decision);
    assert_eq!(
        transcripts_of(&ended.log, "agent"),
        said(&[(0, DISCLOSURE), (2, CONFIRMATION)])
    );
    assert_matches_media_schema(&ended.log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_check_question_cannot_be_interrupted_and_speech_over_it_answers_it() {
    let fakes = fakes();
    let call = survey_answer_scored(&fakes, 0.343).await;
    call.wait_for("the check question to start", |log| {
        has(log, "agent.audio.start", 2)
    })
    .await;
    fakes.stt.push_reply("Yes, stop.", STT_AFTER);
    fakes
        .decisions
        .push(DecisionReply::yes_no("opt_out", 0.937));
    fakes.llm.push_reply([LlmStep::text("Okay.", ms(50))]);
    call.client.silence(ms(300)).await;
    // Over the check question, well past the 200 ms barge-in threshold.
    call.client.speak(ms(600)).await;
    let ended = until_ended(call).await;
    let log = &ended.log;

    assert!(
        !has_type(log, "agent.interrupted"),
        "the caller interrupted the check question"
    );
    assert_eq!(
        turn_samples(log, 2),
        2 * OUTPUT_SAMPLES_PER_SECOND,
        "the check question is spoken in full"
    );
    assert_opted_out(&ended, "decision", OptOutLayer::Decision);
    assert_eq!(
        transcripts_of(log, "agent"),
        said(&[(0, DISCLOSURE), (2, CHECK), (4, CONFIRMATION)])
    );
    assert_eq!(
        transcripts_of(log, "caller"),
        said(&[(1, SURVEY_ANSWER), (3, "Yes, stop.")])
    );
    assert_eq!(
        last_decision_turns(&fakes, 1),
        [pair("agent", CHECK), pair("caller", "Yes, stop.")]
    );
    assert_matches_media_schema(log);
}
