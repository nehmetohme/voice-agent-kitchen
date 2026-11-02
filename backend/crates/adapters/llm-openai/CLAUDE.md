# vak-adapter-llm-openai

Read `../CLAUDE.md` first; the shared adapter rules apply here.

## Responsibility

This crate implements `LlmEngine` (from `vak-providers`) as `OpenAiLlm`, over the OpenAI-compatible `POST {base_url}/chat/completions` API:

- **Streaming over SSE** (`src/sse.rs`): every content delta becomes a `Text` event the moment it arrives. The stream ends with one `Done`, at `data: [DONE]` or when the stream ends.
- **Tool calls:** the request offers the port's tools in function format. Tool-call fragments are put together and come out as one complete `ToolCall` when the model finishes.
- **Cancellation on barge-in:** dropping the stream drops the connection, which tells the server to stop generating and frees a one-slot server for the next request.

The request body carries `model`, `stream: true`, the messages as `{role, content}`, `max_tokens` when set, and the tools.

- **Config:** `OpenAiLlmConfig::new(base_url, model)`, where `base_url` ends before `/chat/completions`, e.g. `http://host:8080/v1`.
  - `with_bearer_token` (`VAK_LLM_API_KEY`).
  - `with_extra_body`: a JSON object merged into every request body, under the request's own fields (`VAK_LLM_EXTRA_BODY`, e.g. `{"chat_template_kwargs":{"enable_thinking":false}}`). Anything but an object is `BuildError::ExtraBody`.
  - `with_idle_timeout` (default 5 s) and `with_connect_timeout` (default 2 s).
  - An `https://` base URL is refused with `BuildError::BaseUrl` until M7.
- **Errors:** 401 and 403 are `Auth`, 429 is `RateLimited` with its `Retry-After`, any 5xx is `Unavailable`, and anything else is `Protocol`. So is a malformed event, or tool parameters that aren't JSON.

## Targets

| Environment | Endpoint | Auth |
|---|---|---|
| Production | Databricks Foundation Model APIs / Model Serving `llm` endpoint | OAuth M2M token from `vak-databricks-auth` (M7) |
| Local dev | llama-server on a LAN GPU box (KAT-Coder, one slot) at `VAK_LLM_BASE_URL`, with thinking off through `VAK_LLM_EXTRA_BODY` | none, or `VAK_LLM_API_KEY` |

The two targets share one code path; base URL and auth come from config.

## Dependencies

- **May depend on:** `vak-providers`, `vak-domain`, `vak-audio`, `vak-databricks-auth`. Dev-only: `vak-testkit`.
- **Uses today:** `vak-providers`, plus `reqwest` (`json`), `serde_json`, `secrecy`, `thiserror`, `tracing` and `tokio` (`time`). Dev-only: `wiremock`, and tokio's `net` and `io-util` for the stalling test server.
- **Must not depend on:** other adapters, repositories(-pg), services, pipeline, api.

## Notes

- **Time to first token** is the critical latency. Emit tokens as they arrive; never buffer the full response.
- **Deadlines:** the request's deadline covers the time to the first content. After that, a stream silent for the idle timeout fails with `ProviderError::Timeout`.
- **Malformed input:** map SSE parse errors to `ProviderError::Protocol`.
- **Prompts and replies are PII.** Never log them; text and tool arguments travel as `SensitiveText`.
- **Tests:** `tests/contract_voice_core_llm.rs` (`voice-core` v1) runs `llm_conformance` and `llm_tool_conformance` against wiremock replaying streams recorded from the local LLM (`backend/fixtures/llm/text_reply.sse`, `tool_call.sse`). Its ignored live test runs both suites against `VAK_LLM_BASE_URL` (`just adapters-live`, with the LAN LLM in `.env`).

## Coverage floor

**75%**.
