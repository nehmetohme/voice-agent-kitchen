//! The `TtsEngine` contract.

use std::time::Duration;

use vak_providers::{Speech, TtsEngine, TtsRequest};

const DEADLINE: Duration = Duration::from_secs(20);
const VOICE: &str = "af_heart";
const SHORT: &str = "Hello there.";
const LONG: &str =
    "Thanks for calling Bistro Verde. Your table for two is booked for seven tonight.";

/// Checks the `TtsEngine` contract:
///
/// 1. A short and a long clause each come back as 24 kHz audio between 0.2 s and 15 s long
///    that is not silent (its loudest sample reaches 300).
/// 2. The longer clause gives the longer audio: the text really drives the speech.
/// 3. Blank text is refused with an error that is not retryable.
///
/// # Panics
/// When `engine` breaks a rule.
#[expect(clippy::panic, reason = "a broken rule fails the calling test")]
pub async fn tts_conformance(engine: &dyn TtsEngine) {
    let short = synthesize(engine, SHORT).await;
    let long = synthesize(engine, LONG).await;
    for (name, speech) in [("short", &short), ("long", &long)] {
        assert_eq!(
            speech.sample_rate(),
            24_000,
            "TTS rule 1: the {name} clause is not 24 kHz"
        );
        let duration = speech.duration();
        assert!(
            (Duration::from_millis(200)..=Duration::from_secs(15)).contains(&duration),
            "TTS rule 1: the {name} clause lasts {duration:?}"
        );
        let peak = speech
            .samples()
            .iter()
            .map(|sample| sample.unsigned_abs())
            .max()
            .unwrap_or(0);
        assert!(
            peak >= 300,
            "TTS rule 1: the {name} clause is silent (peak {peak})"
        );
    }
    assert!(
        long.duration() > short.duration(),
        "TTS rule 2: {LONG:?} ({:?}) should last longer than {SHORT:?} ({:?})",
        long.duration(),
        short.duration()
    );

    match engine
        .synthesize(TtsRequest::new(" \n ", VOICE, DEADLINE))
        .await
    {
        Ok(speech) => panic!("TTS rule 3: blank text gave {speech:?}"),
        Err(error) => assert!(
            !error.is_retryable(),
            "TTS rule 3: blank text gave a retryable error: {error}"
        ),
    }
}

#[expect(clippy::panic, reason = "a broken rule fails the calling test")]
async fn synthesize(engine: &dyn TtsEngine, text: &str) -> Speech {
    engine
        .synthesize(TtsRequest::new(text, VOICE, DEADLINE))
        .await
        .unwrap_or_else(|error| panic!("TTS rule 1: {text:?} failed: {error}"))
}
