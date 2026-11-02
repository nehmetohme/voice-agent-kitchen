//! Shared setup of the `voice-core` v1 pipeline contracts (`contract_voice_core_pipeline.rs`
//! and `contract_voice_core_opt_out.rs`): the scripted engines, the session configuration,
//! the fake browser and readers for what it received.
//!
//! Part of the contract: never edit it once it is tagged `contract/voice-core/v1`.
#![allow(dead_code, reason = "each contract file uses part of the helpers")]
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers: a failed expectation is a test failure"
)]

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use vak_pipeline::{Engines, OptOutPolicy, SessionConfig, SessionOutcome, TurnPolicy, run_session};
use vak_testkit::audio::LevelVad;
use vak_testkit::media::{FakeClient, Sent, fake_transport};
use vak_testkit::providers::{LlmCall, ScriptedDecisions, ScriptedLlm, ScriptedStt, ScriptedTts};

pub(crate) const SESSION_ID: &str = "test-session";
pub(crate) const DISCLOSURE: &str =
    "Hi, this is Ava, an automated assistant for Bistro Verde. This call may be recorded.";
pub(crate) const SYSTEM_PROMPT: &str =
    "You are Ava, the phone assistant of Bistro Verde. Keep every answer short.";
pub(crate) const VOICE: &str = "af_heart";
pub(crate) const CONFIRMATION: &str = "Understood. We won't contact you again. Goodbye.";
pub(crate) const OPT_OUT_PHRASES: [&str; 2] = ["stop calling", "take me off your list"];
pub(crate) const OPT_OUT_TOOL: &str = "mark_do_not_call";

/// One audio frame, in either direction.
pub(crate) const FRAME: Duration = Duration::from_millis(20);
pub(crate) const END_OF_TURN: Duration = Duration::from_millis(500);
pub(crate) const BARGE_IN_MIN_SPEECH: Duration = Duration::from_millis(200);
pub(crate) const DECISION_THRESHOLD: f64 = 0.5;
pub(crate) const DECISION_WAIT: Duration = Duration::from_millis(300);
pub(crate) const DECISION_DEADLINE: Duration = Duration::from_secs(2);
/// How long the scripted TTS speaks the disclosure and the opt-out confirmation.
pub(crate) const DISCLOSURE_AUDIO: Duration = Duration::from_secs(2);
pub(crate) const CONFIRMATION_AUDIO: Duration = Duration::from_millis(1_500);
/// Server audio: 24 kHz.
pub(crate) const OUTPUT_SAMPLES_PER_SECOND: usize = 24_000;

pub(crate) const fn ms(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

/// The scripted engines of one session. Scripts start empty: unscripted decisions answer
/// "no" at once; the TTS speaks every text (the disclosure in 2 s, the confirmation in
/// 1.5 s, anything else at 60 ms per character).
pub(crate) struct Fakes {
    pub(crate) stt: Arc<ScriptedStt>,
    pub(crate) llm: Arc<ScriptedLlm>,
    pub(crate) tts: Arc<ScriptedTts>,
    pub(crate) decisions: Arc<ScriptedDecisions>,
}

impl Fakes {
    pub(crate) fn new() -> Self {
        Self::with_tts(ScriptedTts::new())
    }

    /// The same, with this TTS (the disclosure and confirmation lengths are added to it).
    pub(crate) fn with_tts(tts: ScriptedTts) -> Self {
        Self {
            stt: Arc::new(ScriptedStt::new()),
            llm: Arc::new(ScriptedLlm::new()),
            tts: Arc::new(
                tts.with_audio_for(DISCLOSURE, DISCLOSURE_AUDIO)
                    .with_audio_for(CONFIRMATION, CONFIRMATION_AUDIO),
            ),
            decisions: Arc::new(ScriptedDecisions::new()),
        }
    }

    pub(crate) fn engines(&self) -> Engines {
        Engines::new(
            self.stt.clone(),
            self.llm.clone(),
            self.tts.clone(),
            self.decisions.clone(),
        )
    }
}

/// The session every contract test runs.
pub(crate) fn config() -> SessionConfig {
    SessionConfig::new(SESSION_ID, DISCLOSURE)
        .with_system_prompt(SYSTEM_PROMPT)
        .with_voice(VOICE)
        .with_turn_policy(
            TurnPolicy::new(END_OF_TURN).with_barge_in_min_speech(BARGE_IN_MIN_SPEECH),
        )
        .with_opt_out(
            OptOutPolicy::new(OPT_OUT_PHRASES, CONFIRMATION)
                .with_decision_threshold(DECISION_THRESHOLD)
                .with_decision_wait(DECISION_WAIT)
                .with_decision_deadline(DECISION_DEADLINE),
        )
        .with_vad(Box::new(LevelVad::default()))
}

/// A running session and the fake browser on its other end.
pub(crate) struct Call {
    pub(crate) client: FakeClient,
    session: JoinHandle<SessionOutcome>,
}

/// A finished session: its outcome and everything the browser received.
pub(crate) struct Ended {
    pub(crate) outcome: SessionOutcome,
    pub(crate) log: Vec<Sent>,
}

pub(crate) fn start(fakes: &Fakes, config: SessionConfig) -> Call {
    let (transport, client) = fake_transport();
    let session = tokio::spawn(run_session(config, fakes.engines(), transport));
    Call { client, session }
}

impl Call {
    /// Sends `session.start` and keeps the microphone open (silent) until the disclosure has
    /// been spoken.
    pub(crate) async fn begin(&self) {
        self.client.send(json!({"type": "session.start"})).await;
        self.wait_for("the disclosure to end", |log| {
            has(log, "agent.audio.end", 0)
        })
        .await;
    }

    /// Silence until `done` holds.
    pub(crate) async fn wait_for(&self, what: &str, done: impl Fn(&[Sent]) -> bool) {
        self.client.silence_until(what, done).await;
    }

    pub(crate) fn log(&self) -> Vec<Sent> {
        self.client.log()
    }

    /// The caller hangs up: `session.end`, then the session must finish.
    pub(crate) async fn hang_up(self) -> Ended {
        self.client.send(json!({"type": "session.end"})).await;
        self.finished().await
    }

    /// Waits for the session to finish on its own.
    pub(crate) async fn finished(self) -> Ended {
        let outcome = tokio::time::timeout(Duration::from_secs(30), self.session)
            .await
            .expect("the session finishes")
            .expect("the session task does not panic");
        Ended {
            outcome,
            log: self.client.log(),
        }
    }
}

// ---- Reading the log -----------------------------------------------------------------------

/// Every text event, in arrival order.
pub(crate) fn events(log: &[Sent]) -> Vec<(Instant, &Value)> {
    log.iter()
        .filter_map(|sent| match sent {
            Sent::Event { at, event, .. } => Some((*at, event)),
            _ => None,
        })
        .collect()
}

fn is(event: &Value, kind: &str, turn: u32) -> bool {
    event["type"] == kind && event["turn_id"] == json!(turn)
}

/// The log position of the first `kind` event of `turn`.
pub(crate) fn index_of(log: &[Sent], kind: &str, turn: u32) -> Option<usize> {
    log.iter()
        .position(|sent| matches!(sent, Sent::Event { event, .. } if is(event, kind, turn)))
}

pub(crate) fn has(log: &[Sent], kind: &str, turn: u32) -> bool {
    index_of(log, kind, turn).is_some()
}

pub(crate) fn has_type(log: &[Sent], kind: &str) -> bool {
    events(log).iter().any(|(_, event)| event["type"] == kind)
}

/// When the first `kind` event of `turn` arrived.
pub(crate) fn at(log: &[Sent], kind: &str, turn: u32) -> Option<Instant> {
    events(log)
        .into_iter()
        .find(|(_, event)| is(event, kind, turn))
        .map(|(at, _)| at)
}

/// The `turn_id` of every `kind` event, in order.
pub(crate) fn turns_of(log: &[Sent], kind: &str) -> Vec<u32> {
    events(log)
        .iter()
        .filter(|(_, event)| event["type"] == kind)
        .map(|(_, event)| turn_id(event))
        .collect()
}

pub(crate) fn turn_id(event: &Value) -> u32 {
    event["turn_id"]
        .as_u64()
        .and_then(|turn| u32::try_from(turn).ok())
        .unwrap_or_else(|| panic!("{event} has a turn_id"))
}

/// `(speaker, turn, text, interrupted)` of every `transcript.final`, in order.
pub(crate) fn transcripts(log: &[Sent]) -> Vec<(String, u32, String, bool)> {
    events(log)
        .iter()
        .filter(|(_, event)| event["type"] == "transcript.final")
        .map(|(_, event)| {
            (
                event["speaker"].as_str().expect("a speaker").to_owned(),
                turn_id(event),
                event["text"].as_str().expect("a text").to_owned(),
                event["interrupted"].as_bool().expect("interrupted"),
            )
        })
        .collect()
}

/// `(turn, text, interrupted)` of the transcripts of `speaker` (`caller` or `agent`).
pub(crate) fn transcripts_of(log: &[Sent], speaker: &str) -> Vec<(u32, String, bool)> {
    transcripts(log)
        .into_iter()
        .filter(|(who, ..)| who == speaker)
        .map(|(_, turn, text, interrupted)| (turn, text, interrupted))
        .collect()
}

/// `(at, samples, sample_rate)` of every audio frame, in order.
pub(crate) fn audio(log: &[Sent]) -> Vec<(Instant, usize, u32)> {
    log.iter()
        .filter_map(|sent| match sent {
            Sent::Audio {
                at,
                samples,
                sample_rate,
                ..
            } => Some((*at, *samples, *sample_rate)),
            _ => None,
        })
        .collect()
}

/// The audio samples between two log positions.
pub(crate) fn samples_between(log: &[Sent], from: usize, to: usize) -> usize {
    audio(log.get(from..to).expect("a range of the log"))
        .iter()
        .map(|(_, samples, _)| samples)
        .sum()
}

pub(crate) fn last_event(log: &[Sent]) -> Value {
    events(log)
        .last()
        .map(|(_, event)| (*event).clone())
        .expect("the server sent an event")
}

/// The conversation an LLM call saw, without system messages: `(role, content)`.
pub(crate) fn history(call: &LlmCall) -> Vec<(String, String)> {
    call.messages
        .iter()
        .filter(|(role, _)| role != "system")
        .cloned()
        .collect()
}

pub(crate) fn turn(role: &str, content: &str) -> (String, String) {
    (role.to_owned(), content.to_owned())
}

/// Every LLM call starts with a system message carrying the agent's instructions.
pub(crate) fn assert_system_prompt(call: &LlmCall) {
    let (role, content) = call.messages.first().expect("the call has messages");
    assert_eq!(role, "system", "the first message is the system prompt");
    assert!(
        content.contains(SYSTEM_PROMPT),
        "the system prompt carries the agent's instructions"
    );
}

/// Every event the session sent matches `contracts/media/vak.media.v1.json`.
pub(crate) fn assert_matches_media_schema(log: &[Sent]) {
    let path = format!(
        "{}/../../../contracts/media/vak.media.v1.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"));
    let mut schema: Value = serde_json::from_str(&text).expect("the schema is JSON");
    schema["$ref"] = json!("#/$defs/server_event");
    let validator = jsonschema::validator_for(&schema).expect("the schema compiles");
    for (_, event) in events(log) {
        let errors: Vec<String> = validator
            .iter_errors(event)
            .map(|e| e.to_string())
            .collect();
        assert!(
            errors.is_empty(),
            "{event} breaks the media schema: {errors:?}"
        );
    }
}
