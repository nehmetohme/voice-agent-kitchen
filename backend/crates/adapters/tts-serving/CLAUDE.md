# vak-adapter-tts-serving

Read `../CLAUDE.md` first; the shared adapter rules apply here.

## Responsibility

This crate implements `TtsEngine` (from `vak-providers`) as `ServingTts`. It POSTs one clause to `{base_url}/serving-endpoints/{endpoint}/invocations`: one `dataframe_records` entry with `text`, `voice`, `speed`, `encoding: pcm_s16le` and `sample_rate: 24000`. It returns the clause's audio as one `Speech`: mono PCM16 at 24 kHz, Kokoro's native rate, which the pipeline cuts into 20 ms frames and plays as is. Request and response shapes follow `contracts/serving/tts.json`.

- **Config:** `ServingTtsConfig::new(base_url, endpoint)`, where `endpoint` is the serving endpoint's name (`VAK_TTS_ENDPOINT`, default `tts`), plus `with_bearer_token` and `with_connect_timeout`. An `https://` base URL is refused with `BuildError::BaseUrl` until M7.
- **Refused before any request:** blank text, and a clause over 1000 characters (`Protocol`).
- **The answer must be 24 kHz `pcm_s16le`** with one prediction of whole 16-bit samples in valid base64. Anything else is `Protocol`: audio is never relabelled.
- **Errors** (as for STT): 401 and 403 are `Auth`, 429 is `RateLimited`, 502 and 503 are `Unavailable`, and 504 is `Timeout`. Anything else, 500 included, is `Protocol`.

## Targets

| Environment | Endpoint | Auth |
|---|---|---|
| Production | Databricks Model Serving `tts` endpoint (Kokoro-82M, MLflow pyfunc, warm A10G) | OAuth M2M token from `vak-databricks-auth` (M7) |
| Local dev | `vak-model-shim` `:8001`, which forwards to mlx-audio `:8000` (Kokoro) | none |

The two targets share one code path; base URL and auth come from config.

## Dependencies

- **May depend on:** `vak-providers`, `vak-domain`, `vak-audio`, `vak-databricks-auth`. Dev-only: `vak-testkit`.
- **Uses today:** `vak-providers`, plus `reqwest` (`json`), `base64`, `serde_json`, `secrecy`, `thiserror`, `tracing` and `tokio` (`time`). Dev-only: `wiremock`, `jsonschema`. Framing into `AudioFrame`s happens in the pipeline, not here.
- **Must not depend on:** other adapters, repositories(-pg), services, pipeline, api.

## Notes

- **Time to first byte** matters. Request the native output rate the pipeline needs, which avoids resampling.
- **Barge-in** cancels synthesis mid-clause. Dropping the future aborts the request.
- **Text is PII.** Never log it. The debug log line records only the character count, the elapsed time and the outcome.
- **Tests:** `tests/contract_voice_core_tts.rs` (`voice-core` v1) runs `tts_conformance` against a wiremock endpoint that answers only schema-valid requests. Its ignored live test runs the suite against `VAK_TTS_BASE_URL` (`just adapters-live`, with `just dev` running).

## Coverage floor

**75%**.
