# vak-providers: model and transport ports

## Responsibility

This crate defines the traits through which the pipeline and services reach models and media transports:

- `LlmEngine`: streamed chat with tools
- `SttEngine`: per-utterance or streaming transcription
- `TtsEngine`: per-clause synthesis
- `DecisionEngine`: typed System One decisions (choice, yes/no, score)
- `MediaTransport`: the browser WebSocket today, telephony later
- `TelephonyCarrier`: later

It also holds their request, stream-event and error types, including `ProviderError`. It contains **traits and port types only**: implementations live in `crates/adapters/*`, and fakes live in `vak-testkit`.

## Dependencies

- **May depend on:** `vak-domain`, `vak-audio`.
- **External crates:** small type crates only, such as `thiserror`, `futures-core` and `tokio-util` (for `CancellationToken` in signatures). Add each to `[workspace.dependencies]` when you first use it.
- **Must not depend on:** reqwest, tungstenite, sqlx, axum, any adapter, repositories, pipeline, services or api.

## Conventions

- **Static dispatch:** prefer native `async fn` in traits, used with generics.
- **Dynamic dispatch:** where `dyn` is needed (the provider is chosen per agent at runtime), use `dynosaur` or `async-trait`. Document the choice once in this crate.
- **Streaming ports:** return channel-based session handles, such as `SttSession { audio_tx, events_rx }`. Handles are dyn-friendly, cancellable and easy to fake.
- **Errors:** `ProviderError { Timeout, RateLimited, Auth, Protocol, Unavailable }` with `fn is_retryable(&self) -> bool`. Vendor types never appear in a signature.
- **Cancellation:** every call takes a `CancellationToken` or is cancel-safe on drop, because barge-in cancels LLM and TTS work mid-stream.
- **Payloads:** wrap transcripts and prompts in redacting newtypes, so `Debug` never prints PII.
- **Visibility:** `pub(crate)` for helpers; the ports themselves are `pub`.

## Testing

- **Conformance suites:** every port has a shared suite in `vak-testkit`, for example `testkit::providers::stt_conformance`. Every implementation (adapters and fakes) must pass it.
- **Tests here** cover only pure helpers on the port types, such as error classification and event ordering.

## Coverage floor

No per-crate floor; this crate counts toward the global 80%.

## Example

This shows the pattern:

```rust
pub trait SttEngine: Send + Sync {
    /// Transcribes one finished utterance. Implementations enforce `opts.timeout`.
    fn transcribe(
        &self,
        audio: &[AudioFrame],
        opts: &SttOptions,
    ) -> impl Future<Output = Result<Transcript, ProviderError>> + Send;
}
```
