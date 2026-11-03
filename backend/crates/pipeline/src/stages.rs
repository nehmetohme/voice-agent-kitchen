//! Model calls run as child tasks of the session actor. Each one reports back over the
//! actor's stage channel and stops as soon as its token is cancelled: dropping the engine's
//! future (or LLM stream) aborts the request, which is how barge-in and opt-out cancel work in
//! flight.
//!
//! Edge policy: tasks → actor is bounded and tasks wait on a full channel. The actor drains it
//! continuously, and a task sends at most a few events per stage (one per LLM delta).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use vak_providers::{
    DecisionEngine, DecisionRequest, Decisions, LlmEngine, LlmEvent, LlmRequest, ProviderError,
    Speech, SttEngine, SttRequest, Transcript, TtsEngine, TtsRequest,
};

/// Room for a burst of LLM deltas between two turns of the actor's loop.
pub(crate) const STAGE_CAPACITY: usize = 64;

/// What a TTS call is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Synthesis {
    Disclosure,
    Confirmation,
    Clause { reply: u64, index: usize },
}

/// A stage's report to the actor.
#[derive(Debug)]
pub(crate) enum StageEvent {
    Stt {
        utterance: u64,
        started: Instant,
        result: Result<Transcript, ProviderError>,
    },
    Decision {
        turn_id: u32,
        started: Instant,
        result: Result<Decisions, ProviderError>,
    },
    Llm {
        reply: u64,
        event: Result<LlmEvent, ProviderError>,
    },
    Tts {
        synthesis: Synthesis,
        started: Instant,
        result: Result<Speech, ProviderError>,
    },
}

/// Spawns the session's model calls.
#[derive(Clone)]
pub(crate) struct Stages {
    pub(crate) events: mpsc::Sender<StageEvent>,
}

impl Stages {
    pub(crate) fn transcribe(
        &self,
        engine: Arc<dyn SttEngine>,
        utterance: u64,
        audio: Vec<i16>,
        deadline: Duration,
        cancel: CancellationToken,
    ) {
        let events = self.events.clone();
        tokio::spawn(async move {
            let started = Instant::now();
            let call = engine.transcribe(SttRequest::new(&audio, deadline).with_language("en"));
            let result = tokio::select! {
                biased;
                () = cancel.cancelled() => return,
                result = within(deadline, call) => result,
            };
            // A send fails only once the session actor is gone: the session has ended.
            let _ = events
                .send(StageEvent::Stt {
                    utterance,
                    started,
                    result,
                })
                .await;
        });
    }

    pub(crate) fn decide(
        &self,
        engine: Arc<dyn DecisionEngine>,
        turn_id: u32,
        request: DecisionRequest,
        cancel: CancellationToken,
    ) {
        let events = self.events.clone();
        tokio::spawn(async move {
            let started = Instant::now();
            let call = engine.decide(&request);
            let result = tokio::select! {
                biased;
                () = cancel.cancelled() => return,
                result = within(request.deadline(), call) => result,
            };
            // A send fails only once the session actor is gone: the session has ended.
            let _ = events
                .send(StageEvent::Decision {
                    turn_id,
                    started,
                    result,
                })
                .await;
        });
    }

    /// Streams a reply: one `Llm` event per stream event, until `Done` or an error.
    pub(crate) fn stream(
        &self,
        engine: Arc<dyn LlmEngine>,
        reply: u64,
        request: LlmRequest,
        cancel: CancellationToken,
    ) {
        let events = self.events.clone();
        tokio::spawn(async move {
            let deadline = request.deadline();
            let opened = tokio::select! {
                biased;
                () = cancel.cancelled() => return,
                opened = within(deadline, engine.stream(request)) => opened,
            };
            let mut stream = match opened {
                Ok(stream) => stream,
                Err(error) => {
                    // Fails only once the session has ended.
                    let _ = events
                        .send(StageEvent::Llm {
                            reply,
                            event: Err(error),
                        })
                        .await;
                    return;
                }
            };
            loop {
                let next = tokio::select! {
                    biased;
                    () = cancel.cancelled() => return,
                    next = stream.next_event() => next,
                };
                let Some(event) = next else { return };
                let last = !matches!(event, Ok(LlmEvent::Text(_) | LlmEvent::ToolCall(_)));
                if events.send(StageEvent::Llm { reply, event }).await.is_err() || last {
                    return;
                }
            }
        });
    }

    pub(crate) fn synthesize(
        &self,
        engine: Arc<dyn TtsEngine>,
        synthesis: Synthesis,
        text: String,
        voice: String,
        deadline: Duration,
        cancel: CancellationToken,
    ) {
        let events = self.events.clone();
        tokio::spawn(async move {
            let started = Instant::now();
            let call = engine.synthesize(TtsRequest::new(&text, &voice, deadline));
            let result = tokio::select! {
                biased;
                () = cancel.cancelled() => return,
                result = within(deadline, call) => result,
            };
            // A send fails only once the session actor is gone: the session has ended.
            let _ = events
                .send(StageEvent::Tts {
                    synthesis,
                    started,
                    result,
                })
                .await;
        });
    }
}

/// The engine enforces its request's deadline; this is the backstop for one that doesn't.
async fn within<T>(
    deadline: Duration,
    call: impl Future<Output = Result<T, ProviderError>>,
) -> Result<T, ProviderError> {
    tokio::time::timeout(deadline, call)
        .await
        .unwrap_or(Err(ProviderError::Timeout))
}
