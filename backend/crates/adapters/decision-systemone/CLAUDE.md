# vak-adapter-decision-systemone

Read `../CLAUDE.md` first; the shared adapter rules apply here.

## Responsibility

This crate implements `DecisionEngine` (from `vak-providers`) over the System One API. The API returns typed, fast decisions instead of generated text:

- `choice` over a plan node's outgoing edges
- yes/no checks: opt-out, escalation
- scores and outcomes

The model is `openjev-qwen35-4b`. Platform questions live in `contracts/decisions/*.toml`.

## Targets

| Environment | Endpoint | Auth |
|---|---|---|
| Production | Databricks Unity Gateway, System One API | OAuth M2M token from `vak-databricks-auth` |
| Local dev | llama-server `:8081`, `/v1/systemone` (Kev-4B) | none |

The two targets share one code path; base URL and auth come from config.

## Dependencies

- **May depend on:** `vak-providers`, `vak-domain` (decision types), `vak-audio`, `vak-databricks-auth`. Dev-only: `vak-testkit`.
- **Must not depend on:** other adapters, repositories(-pg), services, pipeline, api.

## Notes

- **Decisions are in the voice path.** Use tight per-call deadlines.
- **Return confidence with every answer.** The pipeline applies thresholds and fallbacks. A timeout is a `ProviderError::Timeout`, never a "no" answer, because a timeout must never suppress an opt-out.
- **Transcripts in the request are PII.** Never log them.

## Coverage floor

**75%**.
