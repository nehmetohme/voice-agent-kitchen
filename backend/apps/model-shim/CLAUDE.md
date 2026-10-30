# vak-model-shim: local stand-in for Databricks STT/TTS serving (dev-only)

## Responsibility

This crate builds a small dev-only HTTP server on `:8001` that speaks the Databricks Model Serving invocation shapes:

- `POST /serving-endpoints/stt/invocations` (`contracts/serving/stt.json`)
- `POST /serving-endpoints/tts/invocations` (`contracts/serving/tts.json`)

It translates each request to the local mlx-audio server on `:8000` (Parakeet for STT, Kokoro for TTS) and translates the response back. `vak-adapter-stt-serving` and `vak-adapter-tts-serving` therefore run unchanged in development; only the base URL differs.

**Status:** a placeholder `fn main() {}`. The implementation lands with the local dev stack (T0.8).

## Dependencies

- **May depend on:** `vak-config`, `vak-telemetry`. Nothing else in the workspace: it shares wire contracts, not product code.
- **External crates:** `axum` (server side) and `reqwest` with rustls (forwarding to mlx-audio). `deny.toml` lists this crate as a wrapper for both. Also `anyhow`, `clap`, `tokio`, `serde`, `serde_json`.
- **Must not depend on:** domain, providers, adapters, services, api, pipeline, repositories. It never ships to production and is never deployed to Databricks.

## Conventions

- **Match the contracts byte-for-byte:** field names, encodings, status codes. If mlx-audio lacks something, document the gap; don't change the contract.
- **Timeouts:** every forwarded call has one, and an mlx-audio failure maps to the same error shape Databricks returns.
- **Logging:** never log audio or text payloads. Log the endpoint, status and latency.
- **Errors:** `anyhow` is fine here; it's an app. No `unwrap`/`expect`/`panic!`.

## Testing

- **Request/response translation:** unit tests against the contract JSON schemas, with mlx-audio mocked by wiremock.
- **The real check:** the serving adapters' contract tests run against this shim locally.

## Coverage floor

`apps/` is excluded from coverage.

## Example

This shows how it will run after T0.8:

```sh
just dev   # starts mlx-audio :8000, then vak-model-shim :8001
curl -s -X POST localhost:8001/serving-endpoints/tts/invocations \
  -H 'content-type: application/json' -d '{"inputs":{"text":"Hello"}}' -o hello.wav
```
