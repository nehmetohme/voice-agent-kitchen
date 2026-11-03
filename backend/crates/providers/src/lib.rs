//! Provider ports: the traits the pipeline and services use to reach models
//! and media transports (`LlmEngine`, `SttEngine`, `TtsEngine`, `DecisionEngine`,
//! `MediaTransport`), plus their stream event and error types.
//!
//! Traits and port types only. Implementations live in `crates/adapters/*`;
//! fakes live in `vak-testkit`.
//!
//! **Dispatch.** Every port method returns a [`BoxFuture`], so the ports are
//! dyn-compatible: the composition root picks an implementation per deployment (local or
//! Databricks) and hands it over as `Arc<dyn …>`. The box costs one allocation per call,
//! and calls happen per utterance or clause, never per audio frame. No macro crate
//! (`async-trait`, `dynosaur`) is needed for that.
//!
//! **Cancellation.** Every call is cancel-safe on drop: dropping the future, or the
//! [`LlmStream`], aborts the request. Barge-in relies on it.
//!
//! **Personal data.** Transcripts, prompts and replies travel as [`SensitiveText`], whose
//! `Debug` never shows the words.

mod decision;
mod error;
mod llm;
pub mod media;
mod stt;
mod text;
mod tts;

use std::future::Future;
use std::pin::Pin;

pub use decision::{DecisionEngine, DecisionRequest, Decisions};
pub use error::ProviderError;
pub use llm::{ChatMessage, LlmEngine, LlmEvent, LlmRequest, LlmStream, Role, ToolCall, ToolSpec};
pub use stt::{SttEngine, SttRequest, Transcript};
pub use text::SensitiveText;
pub use tts::{Speech, TtsEngine, TtsRequest};

/// A boxed, sendable future: what every port method returns.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
