//! The voice session's failure and limit paths, beside the `voice-core` contracts: each model
//! failing or running out of time, the session's time limit, audio that isn't PCM16, and the
//! latency the session reports. Same simulated session as the contracts
//! (`contract_voice_core/mod.rs`, used as is).
#![expect(
    clippy::expect_used,
    reason = "test helpers: a failed expectation is a test failure"
)]

mod contract_voice_core;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use contract_voice_core::*;
use serde_json::json;
use vak_domain::{OptOutLayer, SessionEndReason, StageLatencies};
use vak_pipeline::SessionObserver;
use vak_testkit::media::Sent;
use vak_testkit::providers::{LlmStep, ScriptedTts};

/// The `code` of every `error` event, in order.
fn errors(log: &[Sent]) -> Vec<String> {
    events(log)
        .into_iter()
        .filter(|(_, event)| event["type"] == "error")
        .map(|(_, event)| event["code"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// Whether the agent said `text` in a turn of its own.
fn agent_said(log: &[Sent], text: &str) -> bool {
    transcripts_of(log, "agent")
        .iter()
        .any(|(_, said, _)| said == text)
}

/// One caller turn after the disclosure: 500 ms of speech, then silence until `done`.
async fn caller_turn(call: &Call, what: &str, done: impl Fn(&[Sent]) -> bool) {
    call.client.speak(ms(500)).await;
    call.wait_for(what, done).await;
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn without_its_disclosure_the_session_ends_before_saying_anything() {
    let fakes = Fakes::with_tts(ScriptedTts::new().with_failure_for(DISCLOSURE));
    let call = start(&fakes, config());

    call.client.send(json!({"type": "session.start"})).await;
    let ended = call.finished().await;

    assert_eq!(ended.outcome.end_reason(), SessionEndReason::Error);
    assert_eq!(errors(&ended.log), ["provider_unavailable"]);
    assert_eq!(
        last_event(&ended.log),
        json!({"type": "session.ended", "reason": "error"})
    );
    assert!(
        audio(&ended.log).is_empty(),
        "no audio without the disclosure"
    );
    assert_matches_media_schema(&ended.log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_failed_transcription_asks_again_and_the_call_goes_on() {
    let fakes = Fakes::new();
    let call = start(&fakes, config());
    call.begin().await;

    // Nothing scripted: the STT fails.
    caller_turn(&call, "the error", |log| has_type(log, "error")).await;
    fakes.stt.push_reply("Hello?", ms(50));
    fakes.llm.push_reply([LlmStep::text("Hi there.", ms(20))]);
    caller_turn(&call, "the reply", |log| agent_said(log, "Hi there.")).await;
    let ended = call.hang_up().await;

    assert_eq!(errors(&ended.log), ["provider_unavailable"]);
    assert_eq!(ended.outcome.end_reason(), SessionEndReason::ClientEnded);
    assert_matches_media_schema(&ended.log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_failed_or_slow_language_model_leaves_the_turn_unanswered() {
    let fakes = Fakes::new();
    let deadlines = vak_pipeline::StageDeadlines {
        llm_first_token: ms(1_000),
        ..Default::default()
    };
    let call = start(&fakes, config().with_deadlines(deadlines));
    call.begin().await;

    // Nothing scripted for the LLM: it fails.
    fakes.stt.push_reply("Are you open?", ms(50));
    caller_turn(&call, "the first error", |log| errors(log).len() == 1).await;
    // A first token after the deadline: the reply times out.
    fakes.stt.push_reply("Hello?", ms(50));
    fakes
        .llm
        .push_reply([LlmStep::text("Too late.", ms(5_000))]);
    caller_turn(&call, "the second error", |log| errors(log).len() == 2).await;
    let ended = call.hang_up().await;

    assert_eq!(
        errors(&ended.log),
        ["provider_unavailable", "provider_unavailable"]
    );
    assert!(!agent_said(&ended.log, "Too late."));
    assert_eq!(
        transcripts_of(&ended.log, "agent").len(),
        1,
        "only the disclosure"
    );
    assert_matches_media_schema(&ended.log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_clause_that_fails_to_synthesize_is_skipped_and_the_reply_ends() {
    let fakes = Fakes::with_tts(ScriptedTts::new().with_failure_for("Then the rest."));
    let call = start(&fakes, config());
    call.begin().await;

    fakes.stt.push_reply("Can I book?", ms(50));
    fakes
        .llm
        .push_reply([LlmStep::text("Sure, happy to help. Then the rest.", ms(20))]);
    caller_turn(&call, "the reply to end", |log| {
        has(log, "agent.audio.end", 2)
    })
    .await;
    let ended = call.hang_up().await;

    assert_eq!(errors(&ended.log), ["provider_unavailable"]);
    let start = index_of(&ended.log, "agent.audio.start", 2).expect("the reply starts");
    let end = index_of(&ended.log, "agent.audio.end", 2).expect("the reply ends");
    assert!(
        samples_between(&ended.log, start, end) > 0,
        "the first clause plays"
    );
    assert_matches_media_schema(&ended.log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_opt_out_ends_the_call_even_when_the_confirmation_cannot_be_spoken() {
    let fakes = Fakes::with_tts(ScriptedTts::new().with_failure_for(CONFIRMATION));
    fakes.stt.push_reply("Stop calling me.", ms(50));
    let call = start(&fakes, config());
    call.begin().await;

    call.client.speak(ms(500)).await;
    call.wait_for("the session to end", |log| has_type(log, "session.ended"))
        .await;
    let ended = call.finished().await;

    assert_eq!(
        ended.outcome.end_reason(),
        SessionEndReason::OptOut(OptOutLayer::Phrase)
    );
    assert_eq!(
        last_event(&ended.log),
        json!({"type": "session.ended", "reason": "opt_out", "opt_out_layer": "phrase"})
    );
    assert!(!agent_said(&ended.log, CONFIRMATION));
    assert_matches_media_schema(&ended.log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_session_ends_at_its_time_limit() {
    let fakes = Fakes::new();
    let call = start(&fakes, config().with_max_duration(Duration::from_secs(5)));
    call.begin().await;

    call.wait_for("the time limit", |log| has_type(log, "session.ended"))
        .await;
    let ended = call.finished().await;

    assert_eq!(ended.outcome.end_reason(), SessionEndReason::Timeout);
    assert_eq!(
        last_event(&ended.log),
        json!({"type": "session.ended", "reason": "timeout"})
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn audio_that_is_not_pcm16_is_answered_with_an_error() {
    let fakes = Fakes::new();
    let call = start(&fakes, config());

    // Before session.start nothing is answered.
    call.client.send_bytes(&[0, 0, 0]);
    call.begin().await;
    call.client.send_bytes(&[0; 640]);
    call.client.send_bytes(&[0, 0, 0]);
    call.wait_for("the error", |log| has_type(log, "error"))
        .await;
    let ended = call.hang_up().await;

    assert_eq!(errors(&ended.log), ["unsupported_audio"]);
    assert_eq!(ended.outcome.end_reason(), SessionEndReason::ClientEnded);
    assert_matches_media_schema(&ended.log);
}

/// Records what the session reports.
#[derive(Default)]
struct Recorder {
    replies: Mutex<Vec<StageLatencies>>,
    decisions: Mutex<Vec<Duration>>,
}

impl SessionObserver for Recorder {
    fn reply_latencies(&self, latencies: &StageLatencies) {
        self.replies.lock().expect("unpoisoned").push(*latencies);
    }

    fn decision_latency(&self, latency: Duration) {
        self.decisions.lock().expect("unpoisoned").push(latency);
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn every_reply_and_opt_out_decision_reports_its_latency() {
    let fakes = Fakes::new();
    let recorder = Arc::new(Recorder::default());
    let call = start(&fakes, config().with_observer(recorder.clone()));
    call.begin().await;

    fakes.stt.push_reply("Can I book a table?", ms(50));
    fakes.llm.push_reply([LlmStep::text("Sure.", ms(100))]);
    caller_turn(&call, "the reply", |log| agent_said(log, "Sure.")).await;
    call.hang_up().await;

    let replies = recorder.replies.lock().expect("unpoisoned").clone();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].stt(), Some(ms(50)));
    let decisions = recorder.decisions.lock().expect("unpoisoned").clone();
    assert_eq!(decisions.len(), 1, "one decision per caller turn");
}
