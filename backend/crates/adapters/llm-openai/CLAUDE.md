# vak-adapter-llm-openai

Read `../CLAUDE.md` first; the shared adapter rules apply here.

## Responsibility

This crate implements `LlmEngine` (from `vak-providers`) over the OpenAI-compatible `/chat/completions` API:

- streaming over SSE
- tool calls
- cancellation on barge-in

## Targets

| Environment | Endpoint | Auth |
|---|---|---|
| Production | Databricks Foundation Model APIs / Model Serving `llm` endpoint | OAuth M2M token from `vak-databricks-auth` |
| Local dev | llama-server `:8080` (profile "full": Qwen3.6-35B-A3B; "light": Nemotron 3.5 Bolt E4B) | none |

The two targets share one code path; base URL and auth come from config.

## Dependencies

- **May depend on:** `vak-providers`, `vak-domain`, `vak-audio`, `vak-databricks-auth`. Dev-only: `vak-testkit`.
- **Must not depend on:** other adapters, repositories(-pg), services, pipeline, api.

## Notes

- **Time to first token** is the critical latency. Emit tokens as they arrive; never buffer the full response.
- **Malformed input:** map SSE parse errors to `ProviderError::Protocol`. A stalled stream becomes `ProviderError::Timeout`.

## Coverage floor

**75%**.
