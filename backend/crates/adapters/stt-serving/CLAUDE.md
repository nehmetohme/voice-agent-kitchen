# vak-adapter-stt-serving

Read `../CLAUDE.md` first; the shared adapter rules apply here.

## Responsibility

This crate implements `SttEngine` (from `vak-providers`) by POSTing one utterance to `/serving-endpoints/stt/invocations`. Request and response shapes follow `contracts/serving/stt.json`.

## Targets

| Environment | Endpoint | Auth |
|---|---|---|
| Production | Databricks Model Serving `stt` endpoint (Parakeet TDT 0.6B v2, MLflow pyfunc, warm A10G) | OAuth M2M token from `vak-databricks-auth` |
| Local dev | `vak-model-shim` `:8001`, which forwards to mlx-audio `:8000` (Parakeet) | none |

The two targets share one code path; base URL and auth come from config.

## Dependencies

- **May depend on:** `vak-providers`, `vak-domain`, `vak-audio` (frame encoding), `vak-databricks-auth`. Dev-only: `vak-testkit`.
- **Must not depend on:** other adapters, repositories(-pg), services, pipeline, api.

## Notes

- **Audio:** send it in the encoding and sample rate the contract declares, encoded with `vak-audio`. Don't resample here.
- **Transcripts:** they are PII. Never log them; return them in the redacting newtype from `vak-providers`.

## Coverage floor

**75%**.
