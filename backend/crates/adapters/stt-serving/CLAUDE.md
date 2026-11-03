# vak-adapter-stt-serving

Read `../CLAUDE.md` first; the shared adapter rules apply here.

## Responsibility

This crate implements `SttEngine` (from `vak-providers`) as `ServingStt`. It POSTs one utterance to `{base_url}/serving-endpoints/{endpoint}/invocations`: one `dataframe_records` entry with `audio_b64` (base64 of the samples' little-endian bytes), `encoding: pcm_s16le`, `sample_rate: 16000` and the language hint when given. It reads `{"predictions":[{"text":…}]}`, expecting exactly one prediction, and trims the text. Request and response shapes follow `contracts/serving/stt.json`.

- **Config:** `ServingSttConfig::new(base_url, endpoint)`, where `endpoint` is the serving endpoint's name (`VAK_STT_ENDPOINT`, default `stt`), plus `with_bearer_token` and `with_connect_timeout`. An `https://` base URL is refused with `BuildError::BaseUrl` until M7.
- **Errors** (the serving statuses in `contracts/serving/README.md`): 401 and 403 are `Auth`, 429 is `RateLimited`, 502 and 503 are `Unavailable`, and 504 is `Timeout`. Anything else, 500 included, is `Protocol` (not retryable), and so is an answer that breaks the contract.

## Targets

| Environment | Endpoint | Auth |
|---|---|---|
| Production | Databricks Model Serving `stt` endpoint (Parakeet TDT 0.6B v2, MLflow pyfunc, warm A10G) | OAuth M2M token from `vak-databricks-auth` (M7) |
| Local dev | `vak-model-shim` `:8001`, which forwards to mlx-audio `:8000` (Parakeet) | none |

The two targets share one code path; base URL and auth come from config.

## Dependencies

- **May depend on:** `vak-providers`, `vak-domain`, `vak-audio`, `vak-databricks-auth`. Dev-only: `vak-testkit`.
- **Uses today:** `vak-providers`, plus `reqwest` (`json`), `base64`, `serde_json`, `secrecy`, `thiserror`, `tracing` and `tokio` (`time`). Dev-only: `wiremock`, `jsonschema`.
- **Must not depend on:** other adapters, repositories(-pg), services, pipeline, api.

## Notes

- **Audio:** send it in the encoding and sample rate the contract declares: the port's 16 kHz mono PCM16, encoded here as little-endian bytes. Don't resample here.
- **Transcripts:** they are PII. Never log them; return them in a `Transcript`, which holds the text as a `SensitiveText`. The debug log line records only the sample count, the elapsed time and the outcome.
- **Tests:** `tests/contract_voice_core_stt.rs` (`voice-core` v1) runs `stt_conformance` against a wiremock endpoint that answers only schema-valid requests. Its ignored live test runs the suite against `VAK_STT_BASE_URL` (`just adapters-live`, with `just dev` running).

## Coverage floor

**75%**.
