# crates/adapters/: provider implementations

## Responsibility

This folder has one crate per external system, and each crate implements **exactly one** `vak-providers` port:

| Crate | Port | Talks to |
|---|---|---|
| `vak-adapter-llm-openai` | `LlmEngine` | OpenAI-compatible chat: Databricks FMAPI, local llama-server |
| `vak-adapter-stt-serving` | `SttEngine` | Databricks serving invocations contract for STT, local model shim |
| `vak-adapter-tts-serving` | `TtsEngine` | Databricks serving invocations contract for TTS, local model shim |
| `vak-adapter-decision-systemone` | `DecisionEngine` | System One API: Unity Gateway, local llama-server |

Switching between local and Databricks changes **only base URL and auth**, set through configuration. Never add an outside model vendor.

## Dependencies

- **May depend on:** `vak-providers`, `vak-domain`, `vak-audio`, `vak-databricks-auth`. Dev-only: `vak-testkit`.
- **External crates:** `reqwest` (rustls only; adapters are `deny.toml` `wrappers`), `tokio-tungstenite` (aligned with axum's version), `serde`, `serde_json`, `secrecy`, `thiserror`, `tracing`.
- **Must not depend on:** repositories, repositories-pg, services, pipeline, api, any other adapter, sqlx, axum, openssl.

## Conventions

- **Errors:** map every vendor error into `ProviderError` (`Timeout`, `RateLimited`, `Auth`, `Protocol`, `Unavailable`) with the right retryability. Vendor types never leave the crate.
- **Timeouts:** every request has one: a connect timeout plus a per-call deadline taken from the port's options. No unbounded streams; a stalled stream becomes `Timeout`.
- **Cancellation:** honour it. Dropping the future or cancelling the token aborts the HTTP request or stream.
- **Auth:** tokens come from `vak-databricks-auth` or config as `SecretString`. Call `expose_secret()` only where the header is built, and mark the header with `HeaderValue::set_sensitive(true)`.
- **PII:** never log request or response bodies (prompts, transcripts, audio). Log only the endpoint name, status, latency and sizes.
- **Wire types:** keep request/response DTOs private (`pub(crate)`) and match `contracts/serving/*.json` where a contract exists.
- **Clients:** construct one `reqwest::Client` per adapter instance and reuse it. Don't build a client per call.

## Testing

- **wiremock** 0.6 for REST. Assert request shape (insta snapshots of bodies), auth header presence, timeout mapping and error mapping.
- **Streaming:** WebSocket and SSE streams use the testkit fake server, which replays recorded sessions.
- **Conformance:** each adapter passes its port's conformance suite from `vak-testkit`.
- **Live smoke tests:** these hit the local server or Databricks. Mark them `#[ignore]` so they never run in `just verify`.

## Coverage floor

**75%** line coverage per adapter crate (`coverage.toml`).

## Example

This shows the pattern:

```rust
fn map_status(status: reqwest::StatusCode) -> ProviderError {
    match status.as_u16() {
        401 | 403 => ProviderError::Auth,
        429 => ProviderError::RateLimited { retry_after: None },
        500..=599 => ProviderError::Unavailable,
        _ => ProviderError::Protocol(format!("unexpected status {status}")),
    }
}
```
