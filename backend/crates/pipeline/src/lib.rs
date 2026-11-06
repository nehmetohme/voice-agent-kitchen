//! Real-time voice pipeline: one actor task per session, bounded channels
//! between stages (each with a declared overflow policy), turn-taking, barge-in
//! via cancellation, plan execution and per-stage latency budgets.
//!
//! It talks to models and transports only through `vak-providers` ports and is
//! tested deterministically with paused time and scripted fakes.
//!
//! [`run_session`] runs one conversation over a [`vak_providers::media::MediaTransport`]:
//! it speaks the locked disclosure first, then answers each caller turn
//! (VAD → end of turn → STT → opt-out checks and LLM → clause chunking → TTS → paced
//! audio), lets the caller barge in, and ends on the client's `session.end` or an
//! opt-out from any of its three layers.
//!
//! [`run_text_session`] plays the same reply logic in text, against a simulated caller, for
//! test runs: no audio, the same router, opt-out layers and confirm band.

mod chunker;
mod config;
mod detector;
mod outcome;
mod playback;
mod routing;
mod session;
mod stages;
mod text;

pub use config::{
    Engines, OptOutPolicy, SessionConfig, SessionObserver, StageDeadlines, TurnPolicy,
};
pub use outcome::{SessionOutcome, TurnRecord};
pub use session::run_session;
pub use text::{
    CALLER_TOOL, CallerTurn, LlmCaller, SimulatedCaller, TextDeadlines, TextEngines, TextOutcome,
    TextSessionConfig, run_text_session,
};
