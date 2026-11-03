//! The `SttEngine` contract.

use std::time::Duration;

use vak_providers::{SttEngine, SttRequest};

use crate::fixtures;

const DEADLINE: Duration = Duration::from_secs(20);

/// Checks the `SttEngine` contract:
///
/// 1. A recorded utterance (`utterance_book_table_16k.wav`, 16 kHz mono PCM16) comes back
///    as its words: the transcript contains "book a table", ignoring case and punctuation.
/// 2. A transcript's `Debug` output never shows the words.
/// 3. One second of digital silence is not an error; its transcript may be empty.
///
/// # Panics
/// When `engine` breaks a rule.
#[expect(clippy::panic, reason = "a broken rule fails the calling test")]
pub async fn stt_conformance(engine: &dyn SttEngine) {
    let utterance = fixtures::utterance_book_table();
    let transcript = engine
        .transcribe(SttRequest::new(utterance.samples(), DEADLINE).with_language("en"))
        .await
        .unwrap_or_else(|error| panic!("STT rule 1: the fixture utterance failed: {error}"));
    let heard = super::words(transcript.text());
    assert!(
        heard.contains("book a table"),
        "STT rule 1: the transcript of {:?} should contain \"book a table\", got {heard:?}",
        utterance.transcript()
    );

    let debug = format!("{transcript:?}");
    assert!(
        !debug.contains("table"),
        "STT rule 2: Debug of a transcript shows its words: {debug}"
    );

    let silence = vec![0_i16; 16_000];
    if let Err(error) = engine.transcribe(SttRequest::new(&silence, DEADLINE)).await {
        panic!("STT rule 3: one second of silence failed: {error}");
    }
}
