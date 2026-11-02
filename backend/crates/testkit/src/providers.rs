//! Scripted fakes of the provider ports. Each one answers from a script, after virtual
//! delays on tokio's clock (so paused-time tests run them instantly and deterministically),
//! and records every call for assertions.
//!
//! Like the real adapters, every fake honours its request's deadline: a scripted delay at
//! or past the deadline fails with `ProviderError::Timeout` when the deadline passes.

mod decisions;
mod llm;
mod stt;
mod tts;

pub use decisions::{DecisionCall, DecisionReply, ScriptedDecisions};
pub use llm::{LlmCall, LlmStep, ScriptedLlm};
pub use stt::{ScriptedStt, SttCall};
pub use tts::{ScriptedTts, TtsCall};
