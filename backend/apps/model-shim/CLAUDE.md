# vak-model-shim: local stand-in for Databricks STT/TTS serving (dev-only)

## Responsibility

This crate builds a small dev-only HTTP server on `:8001` that speaks the Databricks Model Serving invocation shapes:

- `POST /serving-endpoints/stt/invocations` (`contracts/serving/stt.json`)
- `POST /serving-endpoints/tts/invocations` (`contracts/serving/tts.json`)
- `GET /health` (readiness for process-compose; not a Databricks route)

It translates each request to the local mlx-audio server on `:8000` (Parakeet for STT, Kokoro for TTS) and translates the response back. `vak-adapter-stt-serving` and `vak-adapter-tts-serving` therefore run unchanged in development; only the base URL differs.

| Shim | mlx-audio 0.6.3 |
|---|---|
| STT record (base64 PCM s16le, 16 kHz) | multipart `POST /v1/audio/transcriptions`: `file` = the PCM in a WAV header, `model`, `response_format=json`, `language` if given |
| TTS record (`text`, `voice`, `speed`) | JSON `POST /v1/audio/speech`: `model`, `input`, `voice`, `speed`, `lang_code` = the voice's first letter, `response_format=wav` |
| prediction | STT: `text`, trimmed. TTS: the WAV's samples as base64; anything but 24 kHz mono 16-bit PCM is a 500, never relabelled |

**Configuration** (environment, `src/config.rs`): `VAK_SHIM_ADDR` (`127.0.0.1:8001`; loopback only, since the shim has no auth), `VAK_SHIM_MLX_AUDIO_URL` (`http://127.0.0.1:8000`), `VAK_SHIM_STT_MODEL`, `VAK_SHIM_TTS_MODEL` (a Hugging Face repo or a local folder; `deploy/local/process-compose.yml` passes the folders under `VAK_MODELS_DIR`), `VAK_SHIM_TIMEOUT_MS` (30000; the first call also loads the model) and `VAK_LOG` (`info`).

| Module | Does |
|---|---|
| `config` | settings from a lookup function, validated |
| `request` | parses and validates bodies against the schemas; messages name the field, never its value |
| `mlx_audio` | the reqwest client: deadline on every call, failures mapped to errors |
| `routes` | axum router, 16 MiB body limit, one log line per invocation |
| `error` | `ServingError`: status plus the Databricks `{error_code, message}` body |
| `wav` | writes a 44-byte PCM header; reads `fmt ` and `data` chunks |

## Dependencies

- **May depend on:** `vak-config`, `vak-telemetry`. Nothing else in the workspace: it shares wire contracts, not product code. It uses `vak-telemetry` only.
- **External crates:** `axum` (server side) and `reqwest` without TLS (plain HTTP to mlx-audio on this machine). `deny.toml` lists this crate as a wrapper for both. Also `anyhow`, `base64`, `tokio`, `serde`, `serde_json`, `tracing`. Dev: `wiremock`, `jsonschema`.
- **Must not depend on:** domain, providers, adapters, services, api, pipeline, repositories. It never ships to production and is never deployed to Databricks.

## Conventions

- **Match the contracts byte-for-byte:** field names, encodings, status codes. If mlx-audio lacks something, document the gap; don't change the contract.
- **Timeouts:** every forwarded call has one, and an mlx-audio failure maps to the same error shape Databricks returns: a timeout is 504 `DEADLINE_EXCEEDED`, a refused connection 503 `TEMPORARILY_UNAVAILABLE`, an error status or unusable answer 500 `INTERNAL_ERROR`.
- **Logging:** never log audio or text payloads, or mlx-audio error bodies (they may quote the input). Log the endpoint, status and latency.
- **Errors:** `anyhow` is fine here; it's an app. No `unwrap`/`expect`/`panic!`.

## Testing

- **Contract tests** (`tests/contract_model_shim.rs`, feature `model-shim` in `contracts/contracts.toml`): black-box. Each test starts the real binary against a wiremock mlx-audio and validates every request and response against the JSON Schemas. The signed tag pins the test file and both schemas.
- **Unit tests** per module: config parsing, field validation, WAV reading and writing, error codes, and the router's body limit (413), unknown routes (404) and `/health`.
- **Against real models:** `just dev-smoke` sends a fixture WAV and a sentence through the running shim.

## Coverage floor

`apps/` is excluded from coverage.

## Example

```sh
just dev   # starts mlx-audio :8000, then vak-model-shim :8001
curl -s localhost:8001/serving-endpoints/tts/invocations -H 'content-type: application/json' \
  -d '{"dataframe_records":[{"text":"Hello.","encoding":"pcm_s16le","sample_rate":24000}]}' \
  | jq -r '.predictions[0].audio_b64' | base64 -d > hello.pcm   # 24 kHz s16le mono
```
