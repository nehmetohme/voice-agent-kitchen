# contracts/media/

`vak.media.v1`: the WebSocket protocol between the browser and the voice pipeline, at `GET /api/v1/sessions/{id}/media` with subprotocol `vak.media.v1`.

| File | What |
|---|---|
| `vak.media.v1.json` | JSON Schema (draft 2020-12) of the text events: `$defs/client_event` (browser to server) and `$defs/server_event` (server to browser) |
| `examples.json` | Valid and invalid examples of both, each invalid one with the reason it is invalid |

Both files are pinned by the tag `contract/voice-core/v1` (`../contracts.toml`). The server checks them in `backend/crates/providers/tests/contract_voice_core_media.rs` (its `vak_providers::media` types), the browser in `frontend/src/shared/voice/media-protocol.contract.test.ts` (its parser), and the pipeline contracts validate every event a session sends against the schema. To change the protocol, bump `voice-core` first.

## Frames

| Direction | Binary frames | Text frames |
|---|---|---|
| browser → server | PCM s16le mono **16 kHz**, 20 ms frames (640 bytes); any even length is accepted | `session.start`, `client.mute`, `session.end` |
| server → browser | PCM s16le mono **24 kHz** (Kokoro's native rate, so the server never resamples), 20 ms frames (960 bytes) | `session.started`, `transcript.partial`, `transcript.final`, `agent.audio.start`, `agent.audio.end`, `agent.interrupted`, `plan.node`, `decision`, `latency`, `error`, `session.ended` |

## A session

1. The browser opens the socket and, once its microphone is open and it can play audio, sends `session.start`. The server sends nothing before it.
2. The server answers `session.started` with the session ID and both audio formats, then speaks the locked disclosure as turn 0: `agent.audio.start {turn_id: 0}`, its audio frames, `agent.audio.end`, `transcript.final`.
3. Turns count up from 0 in the order they are committed, whoever speaks. A caller turn ends after the agent's end-of-turn silence; its `transcript.final` arrives in speech order. A reply is `agent.audio.start`, its frames, then exactly one of `agent.audio.end` (fully sent) or `agent.interrupted` (barge-in), then its `transcript.final` with what was actually said, and a `latency` event with its stage timings.
4. Binary frames from the server belong to the turn of the latest `agent.audio.start`. On `agent.interrupted` the browser stops playback at once and drops queued audio and every frame until the next `agent.audio.start`.
5. `decision` reports the decision model's answer about a caller turn. A timeout or error is reported as such, with no probability: it is never a "no".
6. `session.ended` is the last event; the server then closes the socket. An opt-out names the layer that caught it: `phrase`, `decision` or `llm_tool`.

Text never echoes audio, and `error` messages never quote what the caller said. Transcripts are personal data: the browser shows them to the operator and logs none of them.
