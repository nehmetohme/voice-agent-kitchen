# vak-testkit: shared test support (dev-only)

## Responsibility

This crate holds test support shared across the workspace:

- **Fakes:** in-memory fakes for every repository port, and scripted fakes for every provider port (STT, LLM, TTS, decisions, media transport). The scripted fakes answer after virtual delays.
- **Conformance suites:** one per port. Every implementation, real or fake, must pass it.
- **Audio:** fixture loading and a deterministic VAD.
- **Later:** builders for domain objects with sensible defaults, and a fake WebSocket server that replays recorded provider sessions for streaming adapters.

Today:

| Module | Holds |
|---|---|
| `repositories` | `InMemoryAgentRepo`, with `audit_events()`, `set_unavailable(bool)`, `set_stalled(bool)` and `fail_next_updates_with_conflict(n)` for error paths. `agent_repo_conformance(repo, audit_trail)` (including that a saved plan and saved scenarios are stored, read back and replaced), which the fake passes here and `PgAgentRepo` passes in `vak-repositories-pg`. `InMemoryTestRunRepo` (`audit_events()`, `set_unavailable(bool)`) and `test_run_repo_conformance(agents, runs)` (round trips of every result, one active run per agent, revisions, order, the queue, `latest_verdicts`), passed by the fake here and `PgTestRunRepo` |
| `conformance` | The provider suites: `stt_conformance`, `tts_conformance`, `llm_conformance`, `llm_tool_conformance` and `decision_conformance`, each over `&dyn` port. Part of contract `voice-core` v1, pinned by the tag `contract/voice-core/v1`: never edit `conformance.rs` or `conformance/` |
| `providers` | `ScriptedStt` (`push_reply(text, after)`), `ScriptedLlm` (`push_reply` of `LlmStep::text` and `LlmStep::tool_call`; records a dropped stream as `cancelled_at`), `ScriptedTts` (`with_delay`, `with_audio_for(text, duration)`, otherwise 60 ms of 24 kHz audio per character; `with_failure_for(text)` fails that text with `Unavailable`), and `ScriptedDecisions` (`push` of a `DecisionReply`: `yes_no(question, p)`, `.with_choice(question, label)`, `timeout()`, `fail(error)`, `.after(delay)`) |
| `media` | `fake_transport() -> (MediaTransport, FakeClient)`. The fake browser `send`s client events, `send_bytes` raw binary frames (cut by an `InputFramer`, as the bridge does), and plays `speak(d)`, `silence(d)` or `silence_until(what, done)` as one 20 ms frame of 16 kHz audio every 20 ms of tokio time. `log()` returns every server message as `Sent::Event { at, event }` or `Sent::Audio { at, samples, sample_rate }` |
| `audio` | `LevelVad`: speech is a frame whose loudest sample reaches the threshold (default 1000) |
| `fixtures` | `utterance_book_table()`: `backend/fixtures/audio/utterance_book_table_16k.wav`, a canonical 16 kHz mono PCM16 WAV parsed here, and its `.txt` transcript |

Every scripted fake records its calls (`calls()`) and honours its request's deadline: a scripted delay at or past the deadline fails with `ProviderError::Timeout` when the deadline passes. With nothing left in the script, STT and LLM fail with `Unavailable`, and decisions answer at once: 0 for every yes/no question and the first option of every choice. Blank TTS text fails with `Protocol`, like the serving adapter.

Postgres for tests comes from `backend/tools/test-db.sh` through a nextest setup script, not from a testcontainers helper: nextest runs each test in its own process, and one shared container per worktree is faster than one per test.

## Dependencies

- **May depend on:** `vak-domain`, `vak-audio`, `vak-repositories`, `vak-providers`.
- **External crates:** `tokio` (`rt`, `sync`, `time`: the fakes wait on tokio's clock, so paused-time tests run them in virtual time), `serde_json` (the fake client's events), and `jiff` and `uuid` (v7) to build domain fixtures. Add test crates such as `proptest` when first used.
- **Must not depend on:** repositories-pg, adapters, pipeline, services, api, server.
- **Dev-only:** this crate may only ever be a `[dev-dependencies]` entry. `cargo xtask check-layers` fails if any crate lists it under `[dependencies]` or `[build-dependencies]`. Only crates whose `layers.toml` entry has it in `dev-deps` may use it.

## Conventions

- **Fakes are real implementations,** not mocks: in-memory state, deterministic, no sleeping on wall-clock time. Provider fakes sleep on tokio's clock only.
- **Conformance suites** are generic functions over the port: `agent_repo_conformance(&repo, audit_trail)` over any `AgentRepo`, and `stt_conformance(&dyn SttEngine)` and the others for the provider ports. Every rule in a suite states the port contract it checks, and a suite fails at the first broken rule and names it.
- **When a port changes,** update its fake and its conformance suite in the same change. A pinned suite changes only with a new contract version.
- **Builders** (when they come) default every field and expose `with_*` setters: `AgentBuilder::new().with_name("Support").build()`.
- **`unwrap`/`expect`:** allowed only inside `#[cfg(test)]` code. Public helpers return `Result` or document the panic in a `# Panics` section (and carry `#[expect(clippy::panic, reason = "...")]`).

## Testing

- Each fake runs its own conformance suite here (`tests/fakes_conform.rs` for the provider fakes, a unit test in `src/repositories.rs` for `InMemoryAgentRepo`), so the suites are proven against at least one implementation and the fakes behave like the real adapters.

## Coverage floor

Excluded from coverage (`--ignore-filename-regex`). Its code is measured through the crates that use it.

## Example

From `tests/fakes_conform.rs`:

```rust
#[tokio::test(start_paused = true)]
async fn scripted_stt_conforms() {
    let stt = ScriptedStt::new();
    stt.push_reply("Hi, I would like to book a table for two tomorrow at seven.", QUICK);
    stt.push_reply("", QUICK);

    stt_conformance(&stt).await;

    assert_eq!(stt.calls().len(), 2);
}
```
