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

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use vak_providers::ProviderError;

pub use decisions::{DecisionCall, DecisionReply, ScriptedDecisions};
pub use llm::{LlmCall, LlmStep, ScriptedLlm};
pub use stt::{ScriptedStt, SttCall};
pub use tts::{ScriptedTts, TtsCall};

/// A test that panicked while holding a fake's lock leaves its state usable for inspection.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn get_mut<T>(mutex: &mut Mutex<T>) -> &mut T {
    mutex.get_mut().unwrap_or_else(PoisonError::into_inner)
}

/// Waits `delay`, or fails with `Timeout` at `deadline` when the delay reaches it.
async fn within(delay: Duration, deadline: Duration) -> Result<(), ProviderError> {
    if delay >= deadline {
        tokio::time::sleep(deadline).await;
        Err(ProviderError::Timeout)
    } else {
        tokio::time::sleep(delay).await;
        Ok(())
    }
}
