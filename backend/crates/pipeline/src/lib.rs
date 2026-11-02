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

mod chunker;
mod config;
mod detector;
mod outcome;
mod playback;
mod routing;
mod session;
mod stages;

pub use config::{
    Engines, OptOutPolicy, SessionConfig, SessionObserver, StageDeadlines, TurnPolicy,
};
pub use outcome::{SessionOutcome, TurnRecord};
pub use session::run_session;
