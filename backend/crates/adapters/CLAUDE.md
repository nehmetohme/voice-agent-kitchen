# crates/adapters/: provider implementations

## Responsibility

This folder has one crate per external system, and each crate implements **exactly one** `vak-providers` port:

| Crate | Port | Engine and config | Talks to |
|---|---|---|---|
| `vak-adapter-llm-openai` | `LlmEngine` | `OpenAiLlm`, `OpenAiLlmConfig` | OpenAI-compatible chat: Databricks FMAPI, local llama-server |
| `vak-adapter-stt-serving` | `SttEngine` | `ServingStt`, `ServingSttConfig` | Databricks serving invocations contract for STT, local model shim |
| `vak-adapter-tts-serving` | `TtsEngine` | `ServingTts`, `ServingTtsConfig` | Databricks serving invocations contract for TTS, local model shim |
| `vak-adapter-decision-systemone` | `DecisionEngine` | `SystemOneDecisions`, `SystemOneConfig` | System One API: Unity Gateway, local llama-server |

Switching between local and Databricks changes **only base URL, model or endpoint, and auth**, set through configuration (`vak-server` builds all four from `vak_config::ModelSettings`). Never add an outside model vendor.

Shared shape today (M4a):

- **Build:** `Engine::new(config) -> Result<Self, BuildError>`. Every config takes `with_bearer_token(SecretString)` and `with_connect_timeout(Duration)` (default 2 s). `BuildError` is `BaseUrl`, `Token` (not a valid header value) or `Client`, and the LLM adds `ExtraBody`.
- **Plain HTTP only, for now:** a base URL that doesn't start with `http://` (an `https://` one included) is refused with `BuildError::BaseUrl`. TLS (rustls) comes with Databricks auth in M7.

## Dependencies

- **May depend on:** `vak-providers`, `vak-domain`, `vak-audio`, `vak-databricks-auth`. Dev-only: `vak-testkit`.
- **External crates:** `reqwest` with the `json` feature (adapters are `deny.toml` `wrappers`), `serde_json`, `secrecy`, `thiserror`, `tracing` and `tokio` (`time`); the STT and TTS adapters add `base64`. Dev-only: `wiremock`, `jsonschema` (STT and TTS) and tokio. Later: rustls for TLS (M7), and `tokio-tungstenite` (aligned with axum's version) for streaming WebSocket providers.
- **Must not depend on:** repositories, repositories-pg, services, pipeline, api, any other adapter, sqlx, axum, openssl.

## Conventions

- **Errors:** map every vendor error into `ProviderError` (`Timeout`, `RateLimited` with `Retry-After`, `Auth`, `Protocol`, `Unavailable`) with the right retryability. Vendor types never leave the crate. An unreachable server is `Unavailable`, and an answer that breaks the contract is `Protocol`.
- **Timeouts:** every call has two: the connect timeout, plus the request's deadline over the whole call (`tokio::time::timeout`), which fails with `Timeout`. For the LLM, the deadline covers the time to first content, and then a stream silent for the idle timeout (default 5 s) fails with `Timeout`. No unbounded streams.
- **Cancellation:** honour it. Dropping the future, or the LLM stream, aborts the HTTP request.
- **Auth:** tokens come from config as `SecretString` (later from `vak-databricks-auth`). Call `expose_secret()` only where the header is built, and mark the header with `HeaderValue::set_sensitive(true)`. `Authorization` goes out only when a token is configured.
- **PII:** never log request or response bodies (prompts, transcripts, audio). Log only at `debug`, and only sizes, latency and the outcome.
- **Wire types:** keep request and response shapes private (built as `serde_json::Value`) and match `contracts/serving/*.json` where a contract exists.
- **Clients:** construct one `reqwest::Client` per adapter instance and reuse it. Don't build a client per call.

## Testing

- **Contract test per adapter** (`voice-core` v1): `tests/contract_voice_core_stt.rs`, `contract_voice_core_tts.rs`, `contract_voice_core_llm.rs` and `contract_voice_core_decisions.rs`. Never edit them once the tag `contract/voice-core/v1` pins them.
- **Conformance:** each adapter passes its port's suite from `vak_testkit::conformance` (`stt_conformance`, `tts_conformance`, `llm_conformance` with `llm_tool_conformance`, `decision_conformance`) against a wiremock 0.6 stand-in.
- **wiremock** also checks the request shape (the STT and TTS bodies validate against `contracts/serving/*.json`), the auth header, and the mapping of timeouts and error statuses.
- **Streaming:** the LLM tests replay SSE streams recorded from the local LLM (`backend/fixtures/llm/`) through wiremock. A small TCP server in the test stalls a stream on purpose.
- **Live tests** run the same suites against the real models. They are marked `#[ignore]`, so they never run in `just verify`. `just adapters-live` runs them one at a time with `deploy/local/env.sh` loaded: it needs `just dev` (speech and decisions) and the LAN LLM in `.env`. Pass `-p <crate>` to run one adapter.

## Coverage floor

**75%** line coverage per adapter crate (`coverage.toml`).

## Example

How `vak-adapter-llm-openai` maps HTTP statuses:

```rust
fn status_error(status: StatusCode, retry_after: Option<&HeaderValue>) -> ProviderError {
    match status.as_u16() {
        401 | 403 => ProviderError::Auth,
        429 => ProviderError::RateLimited {
            retry_after: retry_after
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse().ok())
                .map(Duration::from_secs),
        },
        500..=599 => ProviderError::Unavailable,
        _ => ProviderError::Protocol(format!("the server answered {status}")),
    }
}
```
