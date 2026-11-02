//! Contract `voice-core` v1 (pipeline): one voice session in simulated time, against
//! scripted engines and a fake browser from `vak-testkit`.
//!
//! Every test runs on a paused tokio clock. The fake browser sends one 20 ms frame of
//! 16 kHz audio every 20 ms, speech or silence, as a real microphone does; the engines answer
//! after scripted delays; every server message is stamped with the virtual time it arrived.
//! The session (`contract_voice_core/mod.rs`) detects speech with `LevelVad`, ends a caller
//! turn after 500 ms of silence and lets the caller barge in after 200 ms of speech.
//!
//! What it pins:
//! - **Disclosure first.** Nothing is sent before `session.start`. Then `session.started`
//!   announces the session and its audio formats, and the locked disclosure is turn 0:
//!   synthesized word for word in one request, spoken before any reply, never interrupted
//!   (even when the caller talks over it), and the first assistant message the LLM sees.
//!   Agent audio is 24 kHz, 20 ms frames, only inside an agent turn.
//! - **Barge-in.** When the caller speaks over a reply for 200 ms, `agent.interrupted` arrives
//!   within one frame of that threshold, no agent audio arrives more than one frame after it,
//!   the in-flight LLM request is cancelled (within the same frame), the reply's transcript and
//!   its history entry stop at whole words spoken so far, and the interrupting speech is
//!   answered as the next turn.
//! - **Turn order.** Caller and agent turns alternate with turn IDs that count up; the LLM
//!   sees the whole conversation in order; caller transcripts keep speech order even when STT
//!   answers out of order, and two caller turns in a row get one reply that saw both.
//! - **Stage timings.** Every reply reports STT, LLM time to first token, TTS time to first
//!   audio and mouth-to-ear, in a `latency` event and in the session outcome, matching the
//!   scripted delays and the audio the browser actually received.
//! - Every event the session sends matches the media schema.
//!
//! Contract file: never edit it once it is tagged `contract/voice-core/v1`. To change the
//! contract, add a new version.
#![expect(
    clippy::expect_used,
    reason = "test helpers: a failed expectation is a test failure"
)]

mod contract_voice_core;

use std::time::Duration;

use contract_voice_core::*;
use serde_json::json;
use tokio::time::Instant;
use vak_domain::{SessionEndReason, Speaker};
use vak_testkit::media::Sent;
use vak_testkit::providers::{LlmStep, ScriptedTts};

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn nothing_is_sent_before_session_start_then_the_session_announces_itself() {
    let fakes = Fakes::new();
    let call = start(&fakes, config());

    call.client.silence(Duration::from_secs(1)).await;
    assert!(
        call.log().is_empty(),
        "the server sent something before session.start"
    );

    call.begin().await;
    let ended = call.hang_up().await;
    let log = &ended.log;
    let events = events(log);
    assert_eq!(
        *events.first().expect("an event").1,
        json!({
            "type": "session.started",
            "session_id": SESSION_ID,
            "audio": {
                "input": {"encoding": "pcm_s16le", "sample_rate": 16000, "frame_ms": 20},
                "output": {"encoding": "pcm_s16le", "sample_rate": 24000, "frame_ms": 20}
            }
        })
    );
    assert_eq!(
        *events.get(1).expect("a second event").1,
        json!({"type": "agent.audio.start", "turn_id": 0}),
        "the disclosure comes right after session.started"
    );

    let first_turn = index_of(log, "agent.audio.start", 0).expect("the disclosure starts");
    for (position, sent) in log.iter().enumerate() {
        if let Sent::Audio {
            samples,
            sample_rate,
            ..
        } = sent
        {
            assert!(position > first_turn, "audio before the first agent turn");
            assert_eq!(*sample_rate, 24_000, "agent audio is 24 kHz");
            assert_eq!(*samples, 480, "agent audio comes in 20 ms frames");
        }
    }
    assert_eq!(
        last_event(log),
        json!({"type": "session.ended", "reason": "client_ended"})
    );
    assert_eq!(ended.outcome.end_reason(), SessionEndReason::ClientEnded);
    assert_matches_media_schema(log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_locked_disclosure_is_spoken_first_word_for_word_and_uninterrupted() {
    const QUESTION: &str = "Hello? Is anyone there?";
    const REPLY: &str = "Yes, hi! How can I help you today?";
    let fakes = Fakes::new();
    fakes.stt.push_reply(QUESTION, ms(120));
    fakes.llm.push_reply([LlmStep::text(REPLY, ms(200))]);
    let call = start(&fakes, config());

    call.client.send(json!({"type": "session.start"})).await;
    // The caller talks over the disclosure straight away.
    call.client.speak(ms(300)).await;
    call.wait_for("the reply to the caller", |log| {
        has(log, "agent.audio.end", 2)
    })
    .await;
    let ended = call.hang_up().await;
    let log = &ended.log;

    let tts = fakes.tts.calls();
    let first = tts.first().expect("the TTS was called");
    assert_eq!(
        first.text, DISCLOSURE,
        "the disclosure is synthesized unchanged"
    );
    assert_eq!(first.voice, VOICE);

    assert_eq!(turns_of(log, "agent.audio.start"), [0, 2]);
    assert!(
        !has(log, "agent.interrupted", 0),
        "the caller interrupted the disclosure"
    );
    let start0 = index_of(log, "agent.audio.start", 0).expect("the disclosure starts");
    let end0 = index_of(log, "agent.audio.end", 0).expect("the disclosure ends");
    assert_eq!(
        samples_between(log, start0, end0),
        2 * OUTPUT_SAMPLES_PER_SECOND,
        "all 2 s of the disclosure are sent"
    );
    let start2 = index_of(log, "agent.audio.start", 2).expect("the reply starts");
    assert!(start2 > end0, "the reply starts before the disclosure ends");

    assert_eq!(
        transcripts_of(log, "agent"),
        [
            (0, DISCLOSURE.to_owned(), false),
            (2, REPLY.to_owned(), false)
        ]
    );
    assert_eq!(
        transcripts_of(log, "caller"),
        [(1, QUESTION.to_owned(), false)]
    );

    let llm = fakes.llm.calls();
    let first_call = llm.first().expect("the LLM was called");
    assert_system_prompt(first_call);
    assert_eq!(
        history(first_call),
        [turn("assistant", DISCLOSURE), turn("user", QUESTION)],
        "the LLM knows the disclosure was said"
    );
    assert_matches_media_schema(log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn barge_in_stops_agent_audio_within_one_frame_and_cancels_the_llm() {
    const CLAUSE: &str = "Let me tell you about our opening hours and our menu.";
    const INTERRUPTION: &str = "Wait, are you open on Sunday?";
    const ANSWER: &str = "Yes, we are open on Sunday from noon.";
    let fakes = Fakes::with_tts(ScriptedTts::new().with_audio_for(CLAUSE, Duration::from_secs(3)));
    fakes.stt.push_reply("Hello.", ms(50));
    fakes.stt.push_reply(INTERRUPTION, ms(50));
    // The first clause arrives quickly; the rest of the reply is still streaming when the
    // caller interrupts.
    fakes.llm.push_reply([
        LlmStep::text(&format!("{CLAUSE} "), ms(100)),
        LlmStep::text(
            "We also have a terrace by the river.",
            Duration::from_secs(5),
        ),
    ]);
    fakes.llm.push_reply([LlmStep::text(ANSWER, ms(100))]);
    let call = start(&fakes, config());

    call.begin().await;
    call.client.speak(ms(400)).await;
    call.wait_for("the reply to start", |log| has(log, "agent.audio.start", 2))
        .await;
    call.client.silence(ms(500)).await;
    let onset = Instant::now();
    call.client.speak(ms(600)).await;
    call.wait_for("the answer to the interruption", |log| {
        has(log, "agent.audio.end", 4)
    })
    .await;
    let ended = call.hang_up().await;
    let log = &ended.log;

    assert_eq!(
        turns_of(log, "agent.interrupted"),
        [2],
        "one barge-in, on turn 2"
    );
    let interrupted = at(log, "agent.interrupted", 2).expect("turn 2 is interrupted");
    let threshold = onset + BARGE_IN_MIN_SPEECH;
    assert!(
        interrupted + FRAME >= threshold && interrupted <= threshold + FRAME,
        "agent.interrupted should arrive within a frame of 200 ms of speech, came {:?} after onset",
        interrupted - onset
    );

    let from = index_of(log, "agent.interrupted", 2).expect("turn 2 is interrupted");
    let to = index_of(log, "agent.audio.start", 4).expect("the answer starts");
    for (sent_at, ..) in audio(log.get(from..to).expect("a range")) {
        assert!(
            sent_at <= interrupted + FRAME,
            "agent audio {:?} after agent.interrupted",
            sent_at - interrupted
        );
    }
    assert!(
        !has(log, "agent.audio.end", 2),
        "an interrupted turn ends with agent.interrupted only"
    );

    let llm = fakes.llm.calls();
    assert_eq!(llm.len(), 2, "one LLM call per reply");
    let cancelled = llm[0]
        .cancelled_at
        .expect("the in-flight LLM request is cancelled");
    assert!(
        !llm[0].completed,
        "the interrupted reply did not stream to the end"
    );
    assert!(
        cancelled <= interrupted + FRAME,
        "the LLM is cancelled {:?} after agent.interrupted",
        cancelled.saturating_duration_since(interrupted)
    );

    let agent = transcripts_of(log, "agent");
    let (_, spoken, cut) = agent
        .iter()
        .find(|(turn, ..)| *turn == 2)
        .expect("a transcript of turn 2")
        .clone();
    assert!(cut, "turn 2's transcript is marked interrupted");
    assert!(!spoken.is_empty(), "some words of turn 2 were spoken");
    assert!(
        CLAUSE.starts_with(&spoken) && CLAUSE[spoken.len()..].starts_with(' '),
        "turn 2's transcript {spoken:?} should be the whole words of {CLAUSE:?} spoken before the barge-in"
    );
    assert_eq!(
        history(&llm[1]),
        [
            turn("assistant", DISCLOSURE),
            turn("user", "Hello."),
            turn("assistant", &spoken),
            turn("user", INTERRUPTION),
        ],
        "the next request knows only what was actually said"
    );

    assert_eq!(
        transcripts_of(log, "caller"),
        [
            (1, "Hello.".to_owned(), false),
            (3, INTERRUPTION.to_owned(), false)
        ]
    );
    assert_eq!(turns_of(log, "agent.audio.start"), [0, 2, 4]);
    assert!(agent.contains(&(4, ANSWER.to_owned(), false)));
    assert_matches_media_schema(log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn caller_and_agent_turns_alternate_and_the_llm_sees_the_whole_conversation() {
    let caller = [
        ("I'd like to book a table.", 300, 400),
        ("For four people, please.", 50, 500),
        ("Seven tonight.", 150, 300),
    ];
    let agent = [
        ("Sure, for how many people?", 200),
        ("And what time works for you?", 100),
        ("Done, a table for four at seven tonight.", 400),
    ];
    let fakes = Fakes::new();
    for (text, stt_ms, _) in caller {
        fakes.stt.push_reply(text, ms(stt_ms));
    }
    for (text, first_token_ms) in agent {
        fakes
            .llm
            .push_reply([LlmStep::text(text, ms(first_token_ms))]);
    }
    let call = start(&fakes, config());

    call.begin().await;
    for (exchange, (_, _, speech_ms)) in caller.iter().enumerate() {
        call.client.speak(ms(*speech_ms)).await;
        let reply = u32::try_from(2 * exchange + 2).expect("a small turn");
        call.wait_for("the reply", |log| has(log, "agent.audio.end", reply))
            .await;
    }
    let ended = call.hang_up().await;
    let log = &ended.log;

    let expected = [
        ("agent", 0, DISCLOSURE),
        ("caller", 1, caller[0].0),
        ("agent", 2, agent[0].0),
        ("caller", 3, caller[1].0),
        ("agent", 4, agent[1].0),
        ("caller", 5, caller[2].0),
        ("agent", 6, agent[2].0),
    ]
    .map(|(who, turn, text)| (who.to_owned(), turn, text.to_owned(), false));
    assert_eq!(transcripts(log), expected);

    assert_eq!(turns_of(log, "agent.audio.start"), [0, 2, 4, 6]);
    for (turn, next) in [(0, Some(2)), (2, Some(4)), (4, Some(6)), (6, None)] {
        let start = index_of(log, "agent.audio.start", turn).expect("the turn starts");
        let end = index_of(log, "agent.audio.end", turn).expect("the turn ends");
        assert!(start < end, "turn {turn} ends before it starts");
        if let Some(next) = next {
            let next = index_of(log, "agent.audio.start", next).expect("the next turn starts");
            assert!(end < next, "turn {turn} overlaps the next agent turn");
        }
    }

    let llm = fakes.llm.calls();
    assert_eq!(llm.len(), 3, "one LLM call per caller turn");
    let mut conversation = vec![turn("assistant", DISCLOSURE)];
    for (call, ((said, ..), (answered, _))) in llm.iter().zip(caller.iter().zip(agent)) {
        assert_system_prompt(call);
        assert!(call.completed && call.cancelled_at.is_none());
        conversation.push(turn("user", said));
        assert_eq!(
            history(call),
            conversation,
            "the history is complete and in order"
        );
        conversation.push(turn("assistant", answered));
    }

    assert_eq!(
        last_event(log),
        json!({"type": "session.ended", "reason": "client_ended"})
    );
    assert_eq!(ended.outcome.end_reason(), SessionEndReason::ClientEnded);
    let turns: Vec<(u32, Speaker)> = ended
        .outcome
        .turns()
        .iter()
        .map(|record| (record.turn_id(), record.speaker()))
        .collect();
    assert_eq!(
        turns,
        [
            (0, Speaker::Agent),
            (1, Speaker::Caller),
            (2, Speaker::Agent),
            (3, Speaker::Caller),
            (4, Speaker::Agent),
            (5, Speaker::Caller),
            (6, Speaker::Agent),
        ]
    );
    assert_matches_media_schema(log);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn transcripts_keep_speech_order_when_stt_answers_out_of_order() {
    const FIRST: &str = "I'd like a table for tonight.";
    const SECOND: &str = "Actually, make that tomorrow.";
    const REPLY: &str = "Sure, tomorrow it is. For how many people?";
    let fakes = Fakes::new();
    // The first utterance takes far longer to transcribe than the second.
    fakes.stt.push_reply(FIRST, Duration::from_secs(2));
    fakes.stt.push_reply(SECOND, ms(50));
    fakes.llm.push_reply([LlmStep::text(REPLY, ms(100))]);
    let call = start(&fakes, config());

    call.begin().await;
    call.client.speak(ms(400)).await;
    // Long enough to end turn 1, too short for its transcript to be back.
    call.client.silence(ms(700)).await;
    call.client.speak(ms(400)).await;
    call.wait_for("the reply", |log| has(log, "agent.audio.end", 3))
        .await;
    let ended = call.hang_up().await;
    let log = &ended.log;

    let stt = fakes.stt.calls();
    assert_eq!(stt.len(), 2);
    assert!(stt[0].started_at < stt[1].started_at);
    assert_eq!(
        transcripts_of(log, "caller"),
        [(1, FIRST.to_owned(), false), (2, SECOND.to_owned(), false)],
        "caller transcripts keep speech order"
    );
    assert_eq!(
        turns_of(log, "agent.audio.start"),
        [0, 3],
        "one reply to both caller turns"
    );
    assert!(!has_type(log, "agent.interrupted"));

    let llm = fakes.llm.calls();
    let spoken = llm.last().expect("the LLM was called");
    assert!(spoken.completed, "the spoken reply streamed to the end");
    let users = |call: &vak_testkit::providers::LlmCall| -> String {
        let text: Vec<String> = history(call)
            .into_iter()
            .filter(|(role, _)| role == "user")
            .map(|(_, content)| content)
            .collect();
        text.join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert_eq!(
        users(spoken),
        format!("{FIRST} {SECOND}"),
        "the reply saw both caller turns, in speech order"
    );
    for call in &llm {
        let said = users(call);
        if let Some(second) = said.find(SECOND) {
            assert!(
                said.find(FIRST).is_some_and(|first| first < second),
                "an LLM call saw the second turn without the first before it"
            );
        }
    }
    assert!(transcripts_of(log, "agent").contains(&(3, REPLY.to_owned(), false)));
    assert_matches_media_schema(log);
}

/// TTS time to first audio in the timings test.
const TTS_FIRST_AUDIO_MS: u64 = 150;

/// One exchange of the timings test: what the scripts take, and when the caller stopped.
struct Expected {
    stt_ms: u64,
    ttft_ms: u64,
    speech_end: Instant,
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn every_reply_records_its_stage_timings() {
    // (caller speech, STT, LLM first token, reply)
    let exchanges = [
        (600, 120, 300, "Sure, I can help with that."),
        (400, 80, 250, "Great, see you at eight."),
    ];
    let fakes = Fakes::with_tts(ScriptedTts::new().with_delay(ms(TTS_FIRST_AUDIO_MS)));
    fakes
        .stt
        .push_reply("Can I book a table for tomorrow?", ms(exchanges[0].1));
    fakes
        .stt
        .push_reply("Two people, at eight.", ms(exchanges[1].1));
    fakes.llm.push_reply([
        LlmStep::text(exchanges[0].3, ms(exchanges[0].2)),
        LlmStep::text(" How many people?", Duration::ZERO),
    ]);
    fakes
        .llm
        .push_reply([LlmStep::text(exchanges[1].3, ms(exchanges[1].2))]);
    let call = start(&fakes, config());

    call.begin().await;
    let mut expected = Vec::new();
    for (exchange, (speech_ms, stt_ms, ttft_ms, _)) in exchanges.iter().enumerate() {
        let onset = Instant::now();
        call.client.speak(ms(*speech_ms)).await;
        expected.push(Expected {
            stt_ms: *stt_ms,
            ttft_ms: *ttft_ms,
            speech_end: onset + ms(*speech_ms),
        });
        let reply = u32::try_from(2 * exchange + 2).expect("a small turn");
        call.wait_for("the reply", |log| has(log, "agent.audio.end", reply))
            .await;
    }
    let ended = call.hang_up().await;
    let log = &ended.log;

    let latencies: Vec<_> = events(log)
        .into_iter()
        .filter(|(_, event)| event["type"] == "latency")
        .map(|(_, event)| event.clone())
        .collect();
    assert_eq!(
        latencies.iter().map(turn_id).collect::<Vec<_>>(),
        [2, 4],
        "one latency event per reply"
    );
    for (expected, event) in expected.iter().zip(&latencies) {
        assert_reply_timings(&ended, event, expected);
    }
    for record in ended.outcome.turns() {
        if record.speaker() == Speaker::Caller {
            assert!(
                record.latencies().is_none(),
                "caller turns have no reply latencies"
            );
        }
    }
    assert_matches_media_schema(log);
}

/// The `latency` event of one reply matches the scripted delays, the audio the browser
/// received, and the session outcome.
fn assert_reply_timings(ended: &Ended, event: &serde_json::Value, expected: &Expected) {
    let log = &ended.log;
    let turn = turn_id(event);
    let frame_ms = u64::try_from(FRAME.as_millis()).expect("small");
    let end_of_turn_ms = u64::try_from(END_OF_TURN.as_millis()).expect("small");
    assert_eq!(event["stt_ms"], json!(expected.stt_ms), "turn {turn}: STT");
    assert_eq!(
        event["llm_ttft_ms"],
        json!(expected.ttft_ms),
        "turn {turn}: LLM first token"
    );
    assert_eq!(
        event["tts_first_audio_ms"],
        json!(TTS_FIRST_AUDIO_MS),
        "turn {turn}: TTS first audio"
    );
    let mouth_to_ear = event["mouth_to_ear_ms"]
        .as_u64()
        .expect("mouth-to-ear is measured");
    let budget = end_of_turn_ms + expected.stt_ms + expected.ttft_ms + TTS_FIRST_AUDIO_MS;
    assert!(
        mouth_to_ear.abs_diff(budget) <= frame_ms,
        "turn {turn}: mouth-to-ear {mouth_to_ear} ms, expected {budget} ms ± one frame"
    );

    // The reported value is what the browser experienced.
    let start = index_of(log, "agent.audio.start", turn).expect("the reply starts");
    let first_audio = log[start..]
        .iter()
        .find_map(|sent| match sent {
            Sent::Audio { at, .. } => Some(*at),
            _ => None,
        })
        .expect("the reply has audio");
    let observed = u64::try_from((first_audio - expected.speech_end).as_millis()).expect("small");
    assert!(
        mouth_to_ear.abs_diff(observed) <= frame_ms,
        "turn {turn}: reported {mouth_to_ear} ms, but the first audio came {observed} ms after the caller stopped"
    );

    let record = ended
        .outcome
        .turns()
        .iter()
        .find(|record| record.turn_id() == turn)
        .expect("the outcome records the reply");
    let recorded = record
        .latencies()
        .expect("the reply's latencies are recorded");
    assert_eq!(recorded.stt(), Some(ms(expected.stt_ms)));
    assert_eq!(recorded.llm_ttft(), Some(ms(expected.ttft_ms)));
    assert_eq!(recorded.tts_first_audio(), Some(ms(TTS_FIRST_AUDIO_MS)));
    assert_eq!(recorded.mouth_to_ear(), Some(ms(mouth_to_ear)));
}
