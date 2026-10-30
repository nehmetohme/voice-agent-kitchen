# vak-adapter-tts-serving

Read `../CLAUDE.md` first; the shared adapter rules apply here.

## Responsibility

This crate implements `TtsEngine` (from `vak-providers`) by POSTing one clause of text to `/serving-endpoints/tts/invocations` and returning audio frames. Request and response shapes follow `contracts/serving/tts.json`.

## Targets

| Environment | Endpoint | Auth |
|---|---|---|
| Production | Databricks Model Serving `tts` endpoint (Kokoro-82M, MLflow pyfunc, warm A10G) | OAuth M2M token from `vak-databricks-auth` |
| Local dev | `vak-model-shim` `:8001`, which forwards to mlx-audio `:8000` (Kokoro) | none |

The two targets share one code path; base URL and auth come from config.

## Dependencies

- **May depend on:** `vak-providers`, `vak-domain`, `vak-audio` (decode into `AudioFrame`), `vak-databricks-auth`. Dev-only: `vak-testkit`.
- **Must not depend on:** other adapters, repositories(-pg), services, pipeline, api.

## Notes

- **Time to first byte** matters. Request the native output rate the pipeline needs, which avoids resampling.
- **Barge-in** cancels synthesis mid-clause. Dropping the future must abort the request.
- **Text is PII.** Never log it.

## Coverage floor

**75%**.
