# LLM fixtures

Server-sent event streams recorded from the local LLM (KAT-Coder-V2.5-Dev on llama-server, thinking off), replayed by the `vak-adapter-llm-openai` tests through wiremock. Each file is the exact `/v1/chat/completions` response body with `"stream": true`; nothing was edited.

| File | Request | Response |
|---|---|---|
| `text_reply.sse` | system: restaurant phone agent, `mark_do_not_call` tool offered; user: "Hi, are you open tonight?" | 20 content deltas: "Yes, we are open tonight until 10 PM. Would you like to make a reservation?", `finish_reason: stop`, `[DONE]` |
| `tool_call.sse` | the same, user: "Take me off your list, and don't ever phone me again." | one `mark_do_not_call` tool call, its arguments `{}` split over two deltas, `finish_reason: tool_calls`, `[DONE]` |

The `timings` object in the last chunk is llama-server's own (prompt and generation speed); other servers leave it out.

## Rules

- **Recorded, not written by hand,** from our own local models with synthetic prompts. No customer data.
- They are contract inputs: the tag `contract/voice-core/v1` pins them. To re-record one, add a new contract version.
