# vak-providers: model and transport ports

## Responsibility

This crate defines the traits through which the pipeline and services reach models and media transports:

- `SttEngine`: one finished utterance in (`SttRequest`: 16 kHz mono PCM16, a language hint, a deadline), its `Transcript` out. Streaming STT is later.
- `LlmEngine`: streamed chat with tools. `stream(LlmRequest)` opens a `Box<dyn LlmStream>`, whose `next_event` yields `LlmEvent::Text`, `LlmEvent::ToolCall` (once its arguments are complete) and finally `LlmEvent::Done`. The request's deadline is the time to first token. `LlmRequest::with_greedy_sampling(seed)` pins sampling (temperature 0 and a seed) where the model honours it, for reproducible test runs; without it the model's defaults apply.
- `TtsEngine`: one clause in (`TtsRequest`: text, catalog voice ID, speed, deadline), its `Speech` out: mono PCM16 at its sample rate (24 kHz from the serving contract).
- `DecisionEngine`: typed System One decisions. A `DecisionRequest` holds the recent turns, the plan node and `(QuestionId, Question)` pairs from `vak-domain`; `Decisions` holds the model ID and one `Answer` per question. Yes/no and choice questions exist today.
- `media`: the `MediaTransport` port and the `vak.media.v1` wire types (below). It serves the browser WebSocket today; telephony and WebRTC are later.
- `TelephonyCarrier`: later.

It also holds `ProviderError`, `SensitiveText` and `BoxFuture`. It contains **traits and port types only**: implementations live in `crates/adapters/*`, the WebSocket bridge in `vak-api`, and fakes in `vak-testkit`.

### The media module

- **Constants:** `PROTOCOL` (`vak.media.v1`), `INPUT_SAMPLE_RATE` (16 kHz, client audio), `OUTPUT_SAMPLE_RATE` (24 kHz, Kokoro's native rate, so nothing resamples) and `FRAME_MS` (20).
- **Events:** `ClientEvent` (`session.start`, `client.mute`, `session.end`) and `ServerEvent` (`session.started`, `transcript.partial`, `transcript.final`, `agent.audio.start`, `agent.audio.end`, `agent.interrupted`, `plan.node`, `decision`, `latency`, `error`, `session.ended`). Both are serde-tagged on `type` with the wire names, and `CLIENT_EVENT_TYPES` and `SERVER_EVENT_TYPES` list them. `ClientEvent::from_json` parses a text frame; `ServerEvent::to_json` writes one. `session_started` and `session_ended` build those events from the protocol's formats and a domain `SessionEndReason`.
- **Channels:** `media_channel()` returns a connected `(MediaTransport, MediaPeer)`. The pipeline holds the transport (`recv` gives an `Inbound`, `send` takes an `Outbound`). The peer side is the API's WebSocket bridge or testkit's fake client, and it talks through a cloneable `PeerSender`. `Inbound` is `Audio(AudioFrame)` (20 ms at 16 kHz), `Event`, `Invalid` (a text frame that isn't a client event) or `UnsupportedAudio` (an odd byte count). `Outbound` is `Event` or `Audio(AudioFrame)`. A gone end is `Closed`. Frames are fixed arrays, so audio never allocates on its way: `InputFramer` (one per socket) cuts the client's binary messages, any even length, into `INPUT_FRAME_SAMPLES` = 320-sample frames and keeps a partial frame for the next message, and `AudioFrame::copy_le_bytes` writes agent frames into the bridge's buffer.

| Edge | Capacity | When full |
|---|---|---|
| peer → pipeline, audio (`push_audio`, `InputFramer::push`) | `INBOUND_CAPACITY` = 64 items, about 1.3 s | Never waits: the frame is dropped (`PushAudio::Dropped`), because late audio is worthless and the socket reader must not stall. An `UnsupportedAudio` marker is dropped the same way |
| peer → pipeline, events (`push_text`, `push_event`) | the same queue | The peer waits; events are rare |
| pipeline → peer (`MediaTransport::send`) | `OUTBOUND_CAPACITY` = 256 items | The pipeline waits; the bridge only writes to the socket |

## Dependencies

- **May depend on:** `vak-domain`, `vak-audio`.
- **External crates:** small type crates only: `thiserror`, `serde`, `serde_json` (media events to and from JSON text frames: one encoding for the bridge and the fakes) and `tokio` with the `sync` feature only (the channel types; no runtime, no I/O). Dev-only: `jsonschema`, and tokio's `macros` and `rt`. Add each to `[workspace.dependencies]` when you first use it.
- **Must not depend on:** reqwest, tungstenite, sqlx, axum, any adapter, repositories, pipeline, services or api.

## Conventions

- **Dynamic dispatch:** every port method returns `BoxFuture<'_, …>` (a pinned, boxed `Send` future), so the ports are dyn-compatible. The composition root picks an implementation per deployment and hands it over as `Arc<dyn …>`. The box costs one allocation per call, and calls happen per utterance or clause, never per audio frame. No macro crate (`async-trait`, `dynosaur`) is needed. The crate doc (`src/lib.rs`) records this choice.
- **Streaming ports:** return a handle, such as `Box<dyn LlmStream>`. Handles are dyn-friendly, cancel on drop and are easy to fake.
- **Errors:** `ProviderError { Timeout, RateLimited { retry_after }, Auth, Protocol(String), Unavailable }` with `fn is_retryable(&self) -> bool`. `Timeout`, `RateLimited` and `Unavailable` are retryable; `Auth` and `Protocol` are not. Vendor types never appear in a signature, and a `Protocol` message never quotes a transcript, prompt or reply.
- **Deadlines:** every request carries its deadline, and the implementation fails with `Timeout` when it passes. A decision timeout is never an answer.
- **Cancellation:** every call is cancel-safe on drop. Dropping the future, or the `LlmStream`, aborts the request, because barge-in cancels LLM and TTS work mid-stream. No `CancellationToken` appears in a signature.
- **Payloads:** transcripts, prompts, replies and tool arguments travel as `SensitiveText`. Its `Debug` shows only the byte count, and it serializes as a plain string. The `Debug` of `SttRequest`, `TtsRequest` and `Speech` hides text and samples too.
- **Visibility:** `pub(crate)` for helpers; the ports themselves are `pub`.

## Testing

- **Conformance suites:** every port has a shared suite in `vak_testkit::conformance`: `stt_conformance`, `tts_conformance`, `llm_conformance` with `llm_tool_conformance`, and `decision_conformance`. Every implementation (adapters and fakes) must pass it.
- **Tests here** cover only pure helpers on the port types: error classification, `Speech::duration`, `Debug` redaction, the media channel's overflow, and the events' wire shapes.
- **Contract:** `tests/contract_voice_core_media.rs` (`voice-core` v1) checks the media types against `contracts/media/vak.media.v1.json` and `examples.json`. Every valid client example parses, every invalid one is refused, every server example serializes back to the identical JSON, and the type lists and constants match the schema.

## Coverage floor

No per-crate floor; this crate counts toward the global 80%.

## Example

The `SttEngine` port (`src/stt.rs`):

```rust
/// Speech to text, one utterance per call.
pub trait SttEngine: Send + Sync {
    fn transcribe<'a>(
        &'a self,
        request: SttRequest<'a>,
    ) -> BoxFuture<'a, Result<Transcript, ProviderError>>;
}
```
