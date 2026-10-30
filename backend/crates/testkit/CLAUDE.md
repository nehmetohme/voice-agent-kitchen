# vak-testkit: shared test support (dev-only)

## Responsibility

This crate holds test support shared across the workspace:

- **Fakes:** in-memory fakes for every repository port, and scripted fakes for every provider port (STT, LLM, TTS, decisions, media transport). The scripted fakes emit events after virtual delays.
- **Conformance suites:** one per port. Every implementation, real or fake, must pass it.
- **Builders** for domain objects with sensible defaults.
- **Audio fixtures:** loading WAVs from `backend/fixtures/audio/` (with `hound`) as 20 ms `AudioFrame`s.
- **Postgres:** the test container (testcontainers `postgres` module) for `vak-repositories-pg`.
- **Fake WebSocket server:** it replays recorded provider sessions for adapter tests.

## Dependencies

- **May depend on:** `vak-domain`, `vak-audio`, `vak-repositories`, `vak-providers`.
- **External crates:** test crates: `tokio` (test-util), `hound`, `testcontainers`, `testcontainers-modules`, `proptest`, `wiremock`.
- **Must not depend on:** repositories-pg, adapters, pipeline, services, api, server.
- **Dev-only:** this crate may only ever be a `[dev-dependencies]` entry. `cargo xtask check-layers` fails if any crate lists it under `[dependencies]` or `[build-dependencies]`. Only crates whose `layers.toml` entry has it in `dev-deps` may use it.

## Conventions

- **Fakes are real implementations,** not mocks: in-memory state, deterministic, no sleeping on wall-clock time.
- **Conformance suites** are generic functions over the port trait, such as `pub async fn agent_repo_conformance<R: AgentRepo>(make: impl Fn() -> R)`. Every test in a suite states the port contract it checks.
- **When a port changes,** update its fake and its conformance suite in the same change.
- **Builders** default every field and expose `with_*` setters: `AgentBuilder::new().with_name("Support").build()`.
- **`unwrap`/`expect`:** allowed only inside `#[cfg(test)]` code. Public helpers return `Result` or document the panic in a `# Panics` section (and carry `#[expect(clippy::panic, reason = "...")]`).

## Testing

- Each fake runs its own conformance suite here, so suites are proven against at least one implementation.

## Coverage floor

Excluded from coverage (`--ignore-filename-regex`). Its code is measured through the crates that use it.

## Example

This shows the pattern:

```rust
// in vak-repositories-pg/tests/agent_repo.rs
#[sqlx::test]
async fn pg_agent_repo_conforms(pool: PgPool) {
    vak_testkit::repositories::agent_repo_conformance(|| PgAgentRepo::new(pool.clone())).await;
}
```
