//! Conformance suites: the shared contract of each provider port. Every implementation
//! passes its port's suite: the scripted fakes in this crate, and each adapter both against
//! a wiremock stand-in and against the local model servers (`just dev`).
//!
//! Part of contract `voice-core` v1: this file and `conformance/` are pinned by the signed
//! tag `contract/voice-core/v1`. Never edit them once tagged; add a new version instead.
//!
//! A suite fails the calling test at the first broken rule and says which rule. Deadlines
//! are generous because a local model's first call may still be warming up; the voice path
//! uses far tighter ones.

mod decisions;
mod llm;
mod stt;
mod tts;

pub use decisions::decision_conformance;
pub use llm::{llm_conformance, llm_tool_conformance};
pub use stt::stt_conformance;
pub use tts::tts_conformance;

/// Lowercase words separated by single spaces: what a transcript says, without its
/// punctuation and case.
fn words(text: &str) -> String {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
