//! The session actor: one task per conversation. It owns the turn state and `select!`s over
//! the client's input, its child tasks' reports, the 20 ms pacing tick and its timers (end of
//! turn, the opt-out decision's wait, the session limit). Model calls run in child tasks
//! (`stages`), so nothing here blocks between `.await`s.
//!
//! A turn's life:
//! 1. Caller audio is cut into 20 ms frames and run through the VAD (`detector`). An
//!    utterance ends after the end-of-turn silence and goes to STT.
//! 2. Transcripts come back possibly out of order and are released in speech order. Each one
//!    is checked against the opt-out phrases at once and sent to the decision model (`opt_out`
//!    question) in parallel with the LLM.
//! 3. The LLM's text is cut into clauses; each clause is synthesized (two ahead of playback)
//!    and paced out at one frame per 20 ms, after a three-frame head start.
//! 4. The reply waits for the opt-out decision up to `decision_wait`; a later "yes" still ends
//!    the session. Caller speech over a reply for the barge-in threshold interrupts it.
//! 5. A decision in the confirm band replaces the reply with the check question (`Check`):
//!    the reply is dropped, or cut if it already plays, while its LLM stream keeps running
//!    for the opt-out tool. The caller's next turn answers the question.

use std::collections::VecDeque;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::{Instant, Interval, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use vak_audio::AudioFrame;
use vak_domain::{Answer, OptOutLayer, OptOutPhrases, SessionEndReason, Speaker, TurnTimeline};
use vak_providers::media::{
    ClientEvent, DecisionStatus, ErrorCode, INPUT_SAMPLE_RATE, Inbound, MediaTransport, Outbound,
    ServerEvent,
};
use vak_providers::{Decisions, LlmEvent, ProviderError, SensitiveText, Speech, Transcript};

use crate::chunker::ClauseChunker;
use crate::detector::{TurnDetector, Utterance};
use crate::outcome::TurnRecord;
use crate::playback::{Playing, TurnKind};
use crate::routing::{Line, OPT_OUT_QUESTION, OPT_OUT_TOOL, Router};
use crate::stages::{STAGE_CAPACITY, StageEvent, Stages, Synthesis};
use crate::{Engines, SessionConfig, SessionOutcome};

const FRAME: Duration = Duration::from_millis(20);
/// Frames sent at once when an agent turn starts, so the browser's playback never starves.
const HEAD_START_FRAMES: usize = 3;
/// Clauses synthesized ahead of the one playing: enough to hide TTS latency, few enough that
/// a barge-in wastes little.
const TTS_LOOKAHEAD: usize = 2;
/// Speech that cancels a reply still being prepared, when barge-in is off.
const DEFAULT_CANCEL_SPEECH: Duration = Duration::from_millis(300);

/// Runs one voice session until the client ends it, an opt-out ends it, or the transport
/// closes, and reports how it went.
pub async fn run_session(
    mut config: SessionConfig,
    engines: Engines,
    transport: MediaTransport,
) -> SessionOutcome {
    let Some(router) = Router::new(&config.system_prompt) else {
        return SessionOutcome {
            end_reason: SessionEndReason::Error,
            turns: Vec::new(),
        };
    };
    let (inbound, outbound) = transport.into_parts();
    let (events, stage_rx) = mpsc::channel(STAGE_CAPACITY);
    let detector = TurnDetector::new(config.take_vad(), config.turn_policy.end_of_turn_silence);
    let phrases = OptOutPhrases::new(&config.opt_out.phrases);
    tracing::info!(session_id = %config.session_id, "voice session open");
    let session = Session {
        router,
        phrases,
        engines,
        inbound,
        outbound,
        stage_rx,
        stages: Stages { events },
        cancel: CancellationToken::new(),
        origin: Instant::now(),
        started: false,
        muted: false,
        detector,
        next_turn: 0,
        history: Vec::new(),
        pending_stt: VecDeque::new(),
        next_utterance: 0,
        unanswered: Vec::new(),
        decisions: Vec::new(),
        reply: None,
        listening: None,
        next_reply: 0,
        playing: None,
        tick: pacing_tick(),
        check: None,
        opt_out: None,
        ended: None,
        turns: Vec::new(),
        config,
    };
    session.run().await
}

/// A committed caller utterance waiting for its transcript.
struct PendingUtterance {
    id: u64,
    speech_end: Instant,
    result: Option<(Instant, Instant, Result<Transcript, ProviderError>)>,
}

/// A transcribed caller turn that no reply has answered yet.
#[derive(Debug, Clone, Copy)]
struct CallerTurn {
    speech_end: Instant,
    stt_started: Instant,
    stt_done: Instant,
}

struct PendingDecision {
    turn_id: u32,
    started: Instant,
    cancel: CancellationToken,
}

enum ClauseState {
    Waiting,
    Synthesizing,
    Ready(Vec<i16>),
    /// Handed to playback, or failed: nothing more to do.
    Done,
}

struct Clause {
    text: String,
    state: ClauseState,
}

/// The LLM reply being prepared or spoken.
struct Reply {
    id: u64,
    /// The whole reply: its LLM stream and its clauses' syntheses.
    cancel: CancellationToken,
    /// Its clauses' syntheses only (a child of `cancel`): dropping the reply for the check
    /// question stops them and keeps the LLM stream.
    tts: CancellationToken,
    answers: Vec<CallerTurn>,
    started: Instant,
    first_token: Option<Instant>,
    first_audio: Option<(Instant, Instant)>,
    chunker: ClauseChunker,
    clauses: Vec<Clause>,
    /// Clauses before this index are with playback (or failed).
    handed: usize,
    llm_done: bool,
    turn_id: Option<u32>,
}

struct OptOut {
    layer: OptOutLayer,
    audio: Option<Vec<i16>>,
}

/// The opt-out check question, asked when the decision model is unsure.
struct Check {
    cancel: CancellationToken,
    state: CheckState,
}

enum CheckState {
    Synthesizing,
    /// Waiting for its turn; `tts` is when its synthesis started and finished.
    Ready {
        samples: Vec<i16>,
        tts: (Instant, Instant),
    },
    /// Spoken, or being spoken. `answer` is the first caller turn after it, whose decision
    /// settles the check.
    Asked {
        answer: Option<u32>,
    },
}

/// A reply dropped for the check question whose LLM stream still runs, only so that a
/// `mark_do_not_call` tool call can still end the session. Its text is never spoken.
struct Listening {
    reply: u64,
    cancel: CancellationToken,
}

struct Session {
    config: SessionConfig,
    engines: Engines,
    router: Router,
    phrases: OptOutPhrases,
    inbound: mpsc::Receiver<Inbound>,
    outbound: mpsc::Sender<Outbound>,
    stage_rx: mpsc::Receiver<StageEvent>,
    stages: Stages,
    cancel: CancellationToken,
    origin: Instant,
    started: bool,
    muted: bool,
    detector: TurnDetector,
    next_turn: u32,
    history: Vec<Line>,
    pending_stt: VecDeque<PendingUtterance>,
    next_utterance: u64,
    unanswered: Vec<CallerTurn>,
    decisions: Vec<PendingDecision>,
    reply: Option<Reply>,
    listening: Option<Listening>,
    next_reply: u64,
    playing: Option<Playing>,
    tick: Interval,
    check: Option<Check>,
    opt_out: Option<OptOut>,
    ended: Option<SessionEndReason>,
    turns: Vec<TurnRecord>,
}

fn pacing_tick() -> Interval {
    let mut tick = tokio::time::interval_at(Instant::now() + FRAME, FRAME);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    tick
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

impl Session {
    async fn run(mut self) -> SessionOutcome {
        let limit = self.origin + self.config.max_duration;
        while self.ended.is_none() {
            let end_of_turn = self.detector.end_of_turn();
            let gate = self.gate_deadline();
            tokio::select! {
                biased;
                item = self.inbound.recv() => match item {
                    Some(item) => self.on_inbound(item).await,
                    None => self.finish(SessionEndReason::ClientEnded).await,
                },
                Some(event) = self.stage_rx.recv() => self.on_stage(event).await,
                _ = self.tick.tick(), if self.playing.is_some() => self.on_tick().await,
                () = sleep_until(end_of_turn), if end_of_turn.is_some() => self.end_utterance(),
                () = sleep_until(gate), if gate.is_some() => self.try_play().await,
                () = tokio::time::sleep_until(limit) => self.finish(SessionEndReason::Timeout).await,
            }
        }
        self.cancel.cancel();
        let end_reason = self.ended.unwrap_or(SessionEndReason::Error);
        tracing::info!(session_id = %self.config.session_id, reason = ?end_reason, "voice session ended");
        SessionOutcome {
            end_reason,
            turns: self.turns,
        }
    }

    // ---- Output ------------------------------------------------------------------------

    async fn send(&mut self, event: ServerEvent) {
        if self.outbound.send(Outbound::Event(event)).await.is_err() {
            self.ended.get_or_insert(SessionEndReason::ClientEnded);
        }
    }

    async fn send_frame(&mut self, frame: AudioFrame) {
        if self.outbound.send(Outbound::Audio(frame)).await.is_err() {
            self.ended.get_or_insert(SessionEndReason::ClientEnded);
        }
    }

    async fn send_error(&mut self, code: ErrorCode, message: &str) {
        self.send(ServerEvent::Error {
            code,
            message: message.to_owned(),
        })
        .await;
    }

    async fn finish(&mut self, reason: SessionEndReason) {
        if self.ended.is_some() {
            return;
        }
        self.send(ServerEvent::session_ended(reason)).await;
        self.ended = Some(reason);
    }

    fn allocate_turn(&mut self) -> u32 {
        let turn = self.next_turn;
        self.next_turn += 1;
        turn
    }

    fn ending(&self) -> bool {
        self.opt_out.is_some() || self.ended.is_some()
    }

    // ---- Client input ------------------------------------------------------------------

    async fn on_inbound(&mut self, item: Inbound) {
        match item {
            Inbound::Event(ClientEvent::SessionStart { .. }) => self.start().await,
            Inbound::Event(ClientEvent::Mute { muted }) => self.muted = muted,
            Inbound::Event(ClientEvent::SessionEnd) => {
                self.finish(SessionEndReason::ClientEnded).await;
            }
            // The transport only carries 16 kHz frames (`InputFramer`).
            Inbound::Audio(frame)
                if self.started && !self.ending() && frame.sample_rate() == INPUT_SAMPLE_RATE =>
            {
                self.on_frame(&frame).await;
            }
            // Nothing is sent before session.start.
            Inbound::Invalid if self.started => {
                self.send_error(
                    ErrorCode::BadMessage,
                    "the text frame is not a vak.media.v1 client event",
                )
                .await;
            }
            Inbound::UnsupportedAudio if self.started => {
                self.send_error(
                    ErrorCode::UnsupportedAudio,
                    "binary frames carry 16 kHz PCM s16le: an even number of bytes",
                )
                .await;
            }
            _ => {}
        }
    }

    async fn start(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        self.send(ServerEvent::session_started(self.config.session_id.clone()))
            .await;
        let disclosure = self.config.disclosure.trim().to_owned();
        if disclosure.is_empty() {
            return;
        }
        let turn_id = self.allocate_turn();
        self.turns.push(TurnRecord {
            turn_id,
            speaker: Speaker::Agent,
            latencies: None,
        });
        self.history.push(Line {
            speaker: Speaker::Agent,
            text: disclosure.clone(),
        });
        self.stages.synthesize(
            self.engines.tts.clone(),
            Synthesis::Disclosure,
            disclosure,
            self.config.voice.clone(),
            self.config.deadlines.tts,
            self.cancel.child_token(),
        );
    }

    async fn on_frame(&mut self, frame: &AudioFrame) {
        let now = Instant::now();
        if self.detector.end_of_turn().is_some_and(|at| now >= at) {
            self.end_utterance();
        }
        if let Some(speech) = self.detector.push(frame, now, self.muted) {
            self.on_speech(speech).await;
        }
    }

    /// The caller has been speaking for `speech` without a real pause.
    async fn on_speech(&mut self, speech: Duration) {
        let barge_in = self.config.turn_policy.barge_in;
        let replying = self
            .playing
            .as_ref()
            .is_some_and(|playing| playing.kind == TurnKind::Reply);
        if replying {
            if barge_in.is_some_and(|min| speech >= min) {
                self.interrupt().await;
            }
        } else if self.reply.is_some() && speech >= barge_in.unwrap_or(DEFAULT_CANCEL_SPEECH) {
            // The caller has more to say: answer everything once they finish.
            self.cancel_reply();
        }
    }

    fn end_utterance(&mut self) {
        let Some(Utterance {
            samples,
            speech_end,
        }) = self.detector.take()
        else {
            return;
        };
        let id = self.next_utterance;
        self.next_utterance += 1;
        self.stages.transcribe(
            self.engines.stt.clone(),
            id,
            samples,
            self.config.deadlines.stt,
            self.cancel.child_token(),
        );
        self.pending_stt.push_back(PendingUtterance {
            id,
            speech_end,
            result: None,
        });
    }

    // ---- Stage reports -----------------------------------------------------------------

    async fn on_stage(&mut self, event: StageEvent) {
        match event {
            StageEvent::Stt {
                utterance,
                started,
                result,
            } => {
                if let Some(pending) = self.pending_stt.iter_mut().find(|p| p.id == utterance) {
                    pending.result = Some((started, Instant::now(), result));
                }
                self.release_transcripts().await;
                self.maybe_start_reply();
            }
            StageEvent::Decision {
                turn_id,
                started,
                result,
            } => self.on_decision(turn_id, started, result).await,
            StageEvent::Llm { reply, event } => {
                if self
                    .reply
                    .as_ref()
                    .is_some_and(|current| current.id == reply)
                {
                    self.on_llm(event).await;
                } else if self
                    .listening
                    .as_ref()
                    .is_some_and(|listening| listening.reply == reply)
                {
                    self.on_dropped_llm(event).await;
                }
            }
            StageEvent::Tts {
                synthesis,
                started,
                result,
            } => self.on_tts(synthesis, started, result).await,
        }
    }

    /// Releases transcripts in speech order: each waits for every earlier utterance's.
    async fn release_transcripts(&mut self) {
        while self
            .pending_stt
            .front()
            .is_some_and(|pending| pending.result.is_some())
        {
            let Some(PendingUtterance {
                speech_end,
                result: Some((stt_started, stt_done, result)),
                ..
            }) = self.pending_stt.pop_front()
            else {
                break;
            };
            match result {
                Ok(transcript) if !transcript.text().trim().is_empty() => {
                    let turn = CallerTurn {
                        speech_end,
                        stt_started,
                        stt_done,
                    };
                    self.caller_turn(transcript.text().trim(), turn).await;
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(%error, "speech recognition failed");
                    self.send_error(
                        ErrorCode::ProviderUnavailable,
                        "speech recognition failed; please say that again",
                    )
                    .await;
                }
            }
        }
    }

    async fn caller_turn(&mut self, text: &str, turn: CallerTurn) {
        if self.ending() {
            return;
        }
        let turn_id = self.allocate_turn();
        self.turns.push(TurnRecord {
            turn_id,
            speaker: Speaker::Caller,
            latencies: None,
        });
        self.send(ServerEvent::TranscriptFinal {
            turn_id,
            speaker: Speaker::Caller.into(),
            text: SensitiveText::new(text),
            interrupted: false,
        })
        .await;
        self.history.push(Line {
            speaker: Speaker::Caller,
            text: text.to_owned(),
        });
        self.unanswered.push(turn);
        if let Some(Check {
            state: CheckState::Asked { answer },
            ..
        }) = self.check.as_mut()
        {
            answer.get_or_insert(turn_id);
        }
        if self.phrases.matches(text) {
            self.opt_out(OptOutLayer::Phrase).await;
            return;
        }
        let cancel = self.cancel.child_token();
        let request = self
            .router
            .decision_request(&self.history, self.config.opt_out.decision_deadline);
        self.stages.decide(
            self.engines.decisions.clone(),
            turn_id,
            request,
            cancel.clone(),
        );
        self.decisions.push(PendingDecision {
            turn_id,
            started: Instant::now(),
            cancel,
        });
    }

    async fn on_decision(
        &mut self,
        turn_id: u32,
        started: Instant,
        result: Result<Decisions, ProviderError>,
    ) {
        let Some(position) = self.decisions.iter().position(|d| d.turn_id == turn_id) else {
            return;
        };
        self.decisions.remove(position);
        let elapsed = Instant::now().saturating_duration_since(started);
        if let Some(observer) = &self.config.observer {
            observer.decision_latency(elapsed);
        }
        let latency_ms = millis(elapsed);
        let event = |status, probability| ServerEvent::Decision {
            turn_id,
            question: OPT_OUT_QUESTION.to_owned(),
            status,
            latency_ms,
            probability,
            choice: None,
        };
        let answer = match &result {
            Ok(decisions) => match decisions.answer(OPT_OUT_QUESTION) {
                Some(Answer::YesNo { probability }) if (0.0..=1.0).contains(probability) => {
                    Some(*probability)
                }
                _ => None,
            },
            Err(_) => None,
        };
        // The answer to the check question settles it, whatever it is; it never asks again.
        let answers_check = self.check.as_ref().is_some_and(|check| {
            matches!(check.state, CheckState::Asked { answer: Some(answer) } if answer == turn_id)
        });
        if answers_check {
            self.check = None;
        }
        match (answer, result) {
            (Some(probability), _) => {
                self.send(event(DecisionStatus::Answered, Some(probability)))
                    .await;
                let policy = &self.config.opt_out;
                if probability >= policy.decision_threshold {
                    self.opt_out(OptOutLayer::Decision).await;
                    return;
                }
                if !answers_check && probability >= policy.confirm_threshold {
                    self.ask_check().await;
                }
            }
            (None, Err(ProviderError::Timeout)) => {
                tracing::warn!(turn_id, "the opt-out decision timed out");
                self.send(event(DecisionStatus::Timeout, None)).await;
            }
            (None, _) => {
                tracing::warn!(turn_id, "the opt-out decision failed");
                self.send(event(DecisionStatus::Error, None)).await;
            }
        }
        self.try_play().await;
    }

    /// The decision model is unsure: the check question replaces the reply. A reply that
    /// hasn't started is dropped now; one that plays is cut once the question is synthesized
    /// (`try_play`), so the caller hears no gap.
    async fn ask_check(&mut self) {
        if self.ending() || self.check.is_some() {
            return;
        }
        let question = self.config.opt_out.check_question.trim();
        if question.is_empty() {
            // No check question: unsure counts as an opt-out.
            self.opt_out(OptOutLayer::Decision).await;
            return;
        }
        let question = question.to_owned();
        tracing::info!("the opt-out decision is unsure; asking the caller");
        if self
            .reply
            .as_ref()
            .is_some_and(|reply| reply.turn_id.is_none())
        {
            self.drop_reply();
        }
        let cancel = self.cancel.child_token();
        self.stages.synthesize(
            self.engines.tts.clone(),
            Synthesis::Check,
            question,
            self.config.voice.clone(),
            self.config.deadlines.tts,
            cancel.clone(),
        );
        self.check = Some(Check {
            cancel,
            state: CheckState::Synthesizing,
        });
    }

    /// Whether the check question is still to be asked: no reply may start before it.
    fn check_pending(&self) -> bool {
        self.check
            .as_ref()
            .is_some_and(|check| !matches!(check.state, CheckState::Asked { .. }))
    }

    // ---- The reply ---------------------------------------------------------------------

    fn maybe_start_reply(&mut self) {
        if !self.started
            || self.ending()
            || self.unanswered.is_empty()
            || !self.pending_stt.is_empty()
            || self.detector.speaking()
            || self.reply.is_some()
            || self.check_pending()
        {
            return;
        }
        // A dropped reply's tool call no longer counts once the caller has moved on; on a
        // one-slot LLM it would also hold up this request.
        self.stop_listening();
        let id = self.next_reply;
        self.next_reply += 1;
        let cancel = self.cancel.child_token();
        let request = self
            .router
            .llm_request(&self.history, self.config.deadlines.llm_first_token);
        self.stages
            .stream(self.engines.llm.clone(), id, request, cancel.clone());
        self.reply = Some(Reply {
            id,
            tts: cancel.child_token(),
            cancel,
            answers: self.unanswered.clone(),
            started: Instant::now(),
            first_token: None,
            first_audio: None,
            chunker: ClauseChunker::default(),
            clauses: Vec::new(),
            handed: 0,
            llm_done: false,
            turn_id: None,
        });
    }

    fn cancel_reply(&mut self) {
        if let Some(reply) = self.reply.take() {
            reply.cancel.cancel();
        }
    }

    /// Drops the reply unspoken for the check question. Its syntheses stop; its LLM stream,
    /// if still running, goes on as `listening`.
    fn drop_reply(&mut self) {
        let Some(reply) = self.reply.take() else {
            return;
        };
        reply.tts.cancel();
        self.stop_listening();
        if reply.llm_done {
            reply.cancel.cancel();
        } else {
            self.listening = Some(Listening {
                reply: reply.id,
                cancel: reply.cancel,
            });
        }
    }

    fn stop_listening(&mut self) {
        if let Some(listening) = self.listening.take() {
            listening.cancel.cancel();
        }
    }

    /// The LLM stream of a dropped reply: only its opt-out tool counts.
    async fn on_dropped_llm(&mut self, event: Result<LlmEvent, ProviderError>) {
        match event {
            Ok(LlmEvent::ToolCall(call)) if call.name() == OPT_OUT_TOOL => {
                self.opt_out(OptOutLayer::LlmTool).await;
            }
            Ok(LlmEvent::Text(_) | LlmEvent::ToolCall(_)) => {}
            // Done, or the stream failed: nothing more can come.
            _ => self.stop_listening(),
        }
    }

    async fn on_llm(&mut self, event: Result<LlmEvent, ProviderError>) {
        let Some(reply) = self.reply.as_mut() else {
            return;
        };
        match event {
            Ok(LlmEvent::Text(text)) => {
                reply.first_token.get_or_insert_with(Instant::now);
                let clauses = reply.chunker.push(text.as_str());
                reply.clauses.extend(clauses.into_iter().map(|text| Clause {
                    text,
                    state: ClauseState::Waiting,
                }));
            }
            Ok(LlmEvent::ToolCall(call)) => {
                reply.first_token.get_or_insert_with(Instant::now);
                if call.name() == OPT_OUT_TOOL {
                    self.opt_out(OptOutLayer::LlmTool).await;
                    return;
                }
            }
            Ok(LlmEvent::Done) => {
                reply.llm_done = true;
                if let Some(text) = reply.chunker.finish() {
                    reply.clauses.push(Clause {
                        text,
                        state: ClauseState::Waiting,
                    });
                }
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "the language model failed");
                reply.llm_done = true;
                let silent = reply.turn_id.is_none();
                self.send_error(
                    ErrorCode::ProviderUnavailable,
                    "the language model is unavailable; please try again",
                )
                .await;
                if silent {
                    self.drop_silent_reply();
                    return;
                }
            }
        }
        self.advance_reply().await;
    }

    /// A reply that will never say anything: its caller turns count as answered, so the
    /// session doesn't retry them in a loop.
    fn drop_silent_reply(&mut self) {
        self.cancel_reply();
        self.unanswered.clear();
    }

    /// Starts the next clauses' TTS, hands ready clauses to playback, and starts or ends the
    /// reply's turn as its state allows.
    async fn advance_reply(&mut self) {
        let Some(reply) = self.reply.as_mut() else {
            return;
        };
        let in_flight = reply.clauses[reply.handed..]
            .iter()
            .filter(|clause| !matches!(clause.state, ClauseState::Waiting))
            .count();
        let mut room = TTS_LOOKAHEAD.saturating_sub(in_flight);
        for (index, clause) in reply.clauses.iter_mut().enumerate().skip(reply.handed) {
            if room == 0 {
                break;
            }
            if matches!(clause.state, ClauseState::Waiting) {
                clause.state = ClauseState::Synthesizing;
                room -= 1;
                self.stages.synthesize(
                    self.engines.tts.clone(),
                    Synthesis::Clause {
                        reply: reply.id,
                        index,
                    },
                    clause.text.clone(),
                    self.config.voice.clone(),
                    self.config.deadlines.tts,
                    reply.tts.child_token(),
                );
            }
        }
        if reply.llm_done && reply.clauses.is_empty() {
            tracing::warn!("the language model replied with nothing to say");
            self.drop_silent_reply();
            return;
        }
        if reply.turn_id.is_some() {
            self.hand_to_playback();
        } else {
            self.try_play().await;
        }
    }

    /// Moves the reply's ready clauses, in order, into the playing turn.
    fn hand_to_playback(&mut self) {
        let (Some(reply), Some(playing)) = (self.reply.as_mut(), self.playing.as_mut()) else {
            return;
        };
        if reply.turn_id != Some(playing.turn_id) {
            return;
        }
        while let Some(clause) = reply.clauses.get_mut(reply.handed) {
            match std::mem::replace(&mut clause.state, ClauseState::Done) {
                ClauseState::Ready(samples) => playing.push(clause.text.clone(), samples),
                ClauseState::Done => {}
                waiting => {
                    clause.state = waiting;
                    break;
                }
            }
            reply.handed += 1;
        }
        if reply.llm_done && reply.handed == reply.clauses.len() {
            playing.complete();
        }
    }

    /// When the reply may start playing although the opt-out decision is still out.
    fn gate_deadline(&self) -> Option<Instant> {
        let reply = self.reply.as_ref()?;
        if reply.turn_id.is_some() || self.playing.is_some() || self.ending() {
            return None;
        }
        let ready = reply
            .clauses
            .first()
            .is_some_and(|clause| matches!(clause.state, ClauseState::Ready(_)));
        let earliest = self.decisions.iter().map(|d| d.started).min()?;
        ready.then_some(earliest + self.config.opt_out.decision_wait)
    }

    /// Starts the next agent turn, if one is ready and nothing is playing.
    async fn try_play(&mut self) {
        if self.ended.is_some() {
            return;
        }
        if let Some(opt_out) = self.opt_out.as_mut() {
            if self.playing.is_none()
                && let Some(samples) = opt_out.audio.take()
            {
                let text = self.config.opt_out.confirmation.clone();
                self.start_turn(TurnKind::Confirmation, text, samples).await;
            }
            return;
        }
        let check_ready = self
            .check
            .as_ref()
            .is_some_and(|check| matches!(check.state, CheckState::Ready { .. }));
        if check_ready {
            self.cut_reply().await;
            if self.playing.is_none() {
                self.start_check().await;
            }
            return;
        }
        if self.playing.is_some() || self.check_pending() {
            return;
        }
        let now = Instant::now();
        let wait = self.config.opt_out.decision_wait;
        let gated = self
            .decisions
            .iter()
            .any(|decision| now < decision.started + wait);
        let Some(reply) = self.reply.as_mut() else {
            return;
        };
        if reply.turn_id.is_some() || gated {
            return;
        }
        let Some(first) = reply.clauses.first_mut() else {
            return;
        };
        let ClauseState::Ready(samples) = std::mem::replace(&mut first.state, ClauseState::Done)
        else {
            return;
        };
        let text = first.text.clone();
        reply.handed = 1;
        self.start_turn(TurnKind::Reply, text, samples).await;
        self.hand_to_playback();
    }

    async fn start_turn(&mut self, kind: TurnKind, text: String, samples: Vec<i16>) -> u32 {
        let turn_id = if kind == TurnKind::Disclosure {
            0
        } else {
            self.allocate_turn()
        };
        self.send(ServerEvent::AgentAudioStart { turn_id }).await;
        let mut playing = Playing::new(turn_id, kind);
        playing.push(text, samples);
        if kind != TurnKind::Reply {
            playing.complete();
        }
        for _ in 0..HEAD_START_FRAMES {
            if let Some(frame) = playing.next_frame() {
                self.send_frame(frame).await;
            }
        }
        self.playing = Some(playing);
        self.tick = pacing_tick();
        if kind == TurnKind::Reply {
            self.reply_started(turn_id).await;
        } else if kind == TurnKind::Confirmation {
            self.turns.push(TurnRecord {
                turn_id,
                speaker: Speaker::Agent,
                latencies: None,
            });
        }
        turn_id
    }

    /// Asks the synthesized check question as the next agent turn. It answers the caller's
    /// turns so far, so it reports their timings like a reply, without an LLM stage.
    async fn start_check(&mut self) {
        let Some(Check {
            cancel,
            state: CheckState::Ready { samples, tts },
        }) = self
            .check
            .take_if(|check| matches!(check.state, CheckState::Ready { .. }))
        else {
            return;
        };
        let text = self.config.opt_out.check_question.trim().to_owned();
        // Locked and uninterruptible, so it is in the history as soon as it starts: speech
        // over it is the answer, judged with the question in context.
        self.history.push(Line {
            speaker: Speaker::Agent,
            text: text.clone(),
        });
        self.check = Some(Check {
            cancel,
            state: CheckState::Asked { answer: None },
        });
        let turn_id = self.start_turn(TurnKind::Check, text, samples).await;
        let answered = self.unanswered.last().copied();
        self.unanswered.clear();
        let offset = |at: Instant| Some(at.saturating_duration_since(self.origin));
        let timeline = TurnTimeline {
            speech_end: answered.and_then(|turn| offset(turn.speech_end)),
            stt_start: answered.and_then(|turn| offset(turn.stt_started)),
            stt_done: answered.and_then(|turn| offset(turn.stt_done)),
            llm_start: None,
            llm_first_token: None,
            tts_start: offset(tts.0),
            tts_first_audio: offset(tts.1),
            first_audio_out: offset(Instant::now()),
        };
        self.record_agent_turn(turn_id, timeline).await;
    }

    /// The reply's first audio is out: its caller turns are answered and its timings known.
    async fn reply_started(&mut self, turn_id: u32) {
        let Some(reply) = self.reply.as_mut() else {
            return;
        };
        reply.turn_id = Some(turn_id);
        self.unanswered.clear();
        let offset = |at: Instant| Some(at.saturating_duration_since(self.origin));
        let answered = reply.answers.last();
        let timeline = TurnTimeline {
            speech_end: answered.and_then(|turn| offset(turn.speech_end)),
            stt_start: answered.and_then(|turn| offset(turn.stt_started)),
            stt_done: answered.and_then(|turn| offset(turn.stt_done)),
            llm_start: offset(reply.started),
            llm_first_token: reply.first_token.and_then(offset),
            tts_start: reply.first_audio.and_then(|(start, _)| offset(start)),
            tts_first_audio: reply.first_audio.and_then(|(_, done)| offset(done)),
            first_audio_out: offset(Instant::now()),
        };
        self.record_agent_turn(turn_id, timeline).await;
    }

    /// Records an answering agent turn's timings: in the outcome, for the observer and in a
    /// `latency` event.
    async fn record_agent_turn(&mut self, turn_id: u32, timeline: TurnTimeline) {
        let latencies = timeline.latencies();
        self.turns.push(TurnRecord {
            turn_id,
            speaker: Speaker::Agent,
            latencies: Some(latencies),
        });
        if let Some(observer) = &self.config.observer {
            observer.reply_latencies(&latencies);
        }
        self.send(ServerEvent::Latency {
            turn_id,
            stt_ms: latencies.stt().map(millis),
            llm_ttft_ms: latencies.llm_ttft().map(millis),
            tts_first_audio_ms: latencies.tts_first_audio().map(millis),
            mouth_to_ear_ms: latencies.mouth_to_ear().map(millis),
        })
        .await;
    }

    async fn on_tts(
        &mut self,
        synthesis: Synthesis,
        started: Instant,
        result: Result<Speech, ProviderError>,
    ) {
        let samples = match result {
            Ok(speech) => Some(speech.into_samples()),
            Err(error) => {
                tracing::warn!(%error, "speech synthesis failed");
                None
            }
        };
        match synthesis {
            Synthesis::Disclosure => {
                if let Some(samples) = samples {
                    let text = self.config.disclosure.trim().to_owned();
                    self.start_turn(TurnKind::Disclosure, text, samples).await;
                } else {
                    // The disclosure must come first: without it, the session can't go on.
                    self.send_error(
                        ErrorCode::ProviderUnavailable,
                        "speech synthesis is unavailable",
                    )
                    .await;
                    self.finish(SessionEndReason::Error).await;
                }
            }
            Synthesis::Check => {
                let Some(check) = self
                    .check
                    .as_mut()
                    .filter(|check| matches!(check.state, CheckState::Synthesizing))
                else {
                    return;
                };
                if let Some(samples) = samples {
                    check.state = CheckState::Ready {
                        samples,
                        tts: (started, Instant::now()),
                    };
                    self.try_play().await;
                } else {
                    // The caller can't be asked: unsure counts as an opt-out.
                    self.check = None;
                    self.send_error(
                        ErrorCode::ProviderUnavailable,
                        "speech synthesis failed for the opt-out check",
                    )
                    .await;
                    self.opt_out(OptOutLayer::Decision).await;
                }
            }
            Synthesis::Confirmation => match (samples, self.opt_out.as_mut()) {
                (Some(samples), Some(opt_out)) => {
                    opt_out.audio = Some(samples);
                    self.try_play().await;
                }
                (_, opt_out) => {
                    let layer = opt_out.map_or(OptOutLayer::Phrase, |opt_out| opt_out.layer);
                    self.finish(SessionEndReason::OptOut(layer)).await;
                }
            },
            Synthesis::Clause { reply, index } => {
                let Some(current) = self.reply.as_mut().filter(|current| current.id == reply)
                else {
                    return;
                };
                if index == 0 {
                    current.first_audio = Some((started, Instant::now()));
                }
                let failed = samples.is_none();
                if let Some(clause) = current.clauses.get_mut(index) {
                    clause.state = samples.map_or(ClauseState::Done, ClauseState::Ready);
                }
                if failed {
                    self.send_error(
                        ErrorCode::ProviderUnavailable,
                        "speech synthesis failed for part of the reply",
                    )
                    .await;
                }
                self.advance_reply().await;
            }
        }
    }

    // ---- Playback ----------------------------------------------------------------------

    async fn on_tick(&mut self) {
        let Some(playing) = self.playing.as_mut() else {
            return;
        };
        if let Some(frame) = playing.next_frame() {
            self.send_frame(frame).await;
        } else if playing.finished() {
            self.end_turn().await;
        }
    }

    async fn end_turn(&mut self) {
        let Some(playing) = self.playing.take() else {
            return;
        };
        let turn_id = playing.turn_id;
        let text = playing.text();
        self.send(ServerEvent::AgentAudioEnd { turn_id }).await;
        if !text.is_empty() {
            self.send(ServerEvent::TranscriptFinal {
                turn_id,
                speaker: Speaker::Agent.into(),
                text: SensitiveText::new(text.clone()),
                interrupted: false,
            })
            .await;
        }
        match playing.kind {
            TurnKind::Disclosure | TurnKind::Check => {}
            TurnKind::Reply => {
                self.history.push(Line {
                    speaker: Speaker::Agent,
                    text,
                });
                self.reply = None;
            }
            TurnKind::Confirmation => {
                let layer = self
                    .opt_out
                    .as_ref()
                    .map_or(OptOutLayer::Phrase, |opt_out| opt_out.layer);
                self.finish(SessionEndReason::OptOut(layer)).await;
                return;
            }
        }
        self.maybe_start_reply();
        self.try_play().await;
    }

    /// Barge-in: stop the reply now, keep what the caller heard.
    async fn interrupt(&mut self) {
        let Some(playing) = self
            .playing
            .take_if(|playing| playing.kind == TurnKind::Reply)
        else {
            return;
        };
        self.cancel_reply();
        self.stop_playing(playing).await;
    }

    /// The check question is ready: a reply that plays stops for it, the same way as on
    /// barge-in, but its LLM stream goes on listening for the opt-out tool.
    async fn cut_reply(&mut self) {
        let Some(playing) = self
            .playing
            .take_if(|playing| playing.kind == TurnKind::Reply)
        else {
            return;
        };
        self.drop_reply();
        self.stop_playing(playing).await;
    }

    /// A reply stops mid-turn: `agent.interrupted`, then the words the caller heard, which
    /// is all the history keeps of it.
    async fn stop_playing(&mut self, playing: Playing) {
        let turn_id = playing.turn_id;
        self.send(ServerEvent::AgentInterrupted { turn_id }).await;
        let spoken = playing.spoken();
        if !spoken.is_empty() {
            self.send(ServerEvent::TranscriptFinal {
                turn_id,
                speaker: Speaker::Agent.into(),
                text: SensitiveText::new(spoken.clone()),
                interrupted: true,
            })
            .await;
            self.history.push(Line {
                speaker: Speaker::Agent,
                text: spoken,
            });
        }
    }

    // ---- Opt-out -----------------------------------------------------------------------

    /// One of the three layers caught an opt-out: stop everything, confirm, end.
    async fn opt_out(&mut self, layer: OptOutLayer) {
        if self.ending() {
            return;
        }
        tracing::info!(?layer, "the caller opted out");
        for decision in self.decisions.drain(..) {
            decision.cancel.cancel();
        }
        // A check question being spoken finishes (it is locked); one not yet spoken never is.
        if let Some(check) = self.check.take() {
            check.cancel.cancel();
        }
        self.stop_listening();
        self.interrupt().await;
        self.cancel_reply();
        self.unanswered.clear();
        self.detector.reset();
        self.opt_out = Some(OptOut { layer, audio: None });
        self.stages.synthesize(
            self.engines.tts.clone(),
            Synthesis::Confirmation,
            self.config.opt_out.confirmation.clone(),
            self.config.voice.clone(),
            self.config.deadlines.tts,
            self.cancel.child_token(),
        );
    }
}
