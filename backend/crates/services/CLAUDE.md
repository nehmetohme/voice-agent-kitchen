# vak-services: use cases

## Responsibility

This crate holds the application use cases:

- agents
- plans
- versions and publishing (test-gated, immutable, with rollback)
- test runs
- sessions
- audit

It orchestrates repositories, providers and the pipeline, and re-exports the few port types `vak-api` needs, such as `MediaTransport` for WebSocket bridging.

## Dependencies

- **May depend on:** `vak-repositories`, `vak-providers`, `vak-pipeline`, `vak-domain`. Dev-only: `vak-testkit`.
- **External crates:** `thiserror`, `tracing`, `tokio` (only what orchestration needs).
- **Must not depend on:** repositories-pg, any adapter, api, axum, sqlx, reqwest.

## Conventions

- **One public function per use case,** such as `publish_version(&self, cmd: PublishVersion) -> Result<PublishedVersion, PublishError>`. Inputs are command structs of domain types, not DTOs.
- **Business rules live in `vak-domain`.** Services load, call domain rules, persist and emit audit events. If you're writing an `if` about business policy here, move it to the domain.
- **The outside world is reached only through ports.** Use generics over port traits, and `dyn` only where the provider is chosen at runtime.
- **Errors:** a `ServiceError` per use case (`thiserror`), built from domain, `RepoError` and `ProviderError`. Don't stringify errors.
- **Timeouts:** every port call that can block on I/O has a deadline. Transactions are port methods, not service code.
- **Audit:** every state change (publish, rollback, plan edit) writes an audit event through `AuditRepo`.
- **No HTTP concepts** such as status codes or headers. The API maps errors.
- **Visibility:** `pub(crate)` is the default; use-case functions and their commands and errors are `pub`.

## Testing

- **Unit tests** use hand-written fakes from `vak-testkit`: in-memory repos and scripted providers.
- **mockall** only for "must not call X" interaction checks.
- **Coverage per use case:** the happy path, each error variant, and audit emission.
- **Mutation testing** (`cargo mutants --in-diff`) runs nightly on this crate as a report.

## Coverage floor

**90%** line coverage (`coverage.toml`).

## Example

This shows the pattern:

```rust
pub async fn rename_agent<R: AgentRepo, A: AuditRepo>(
    agents: &R,
    audit: &A,
    cmd: RenameAgent,
) -> Result<Agent, RenameAgentError> {
    let mut agent = agents.get(cmd.id).await?;
    agent.rename(AgentName::parse(&cmd.name)?); // rule lives in vak-domain
    agents.update(&agent, cmd.expected_revision).await?;
    audit.record(AuditEvent::agent_renamed(&agent, cmd.actor)).await?;
    Ok(agent)
}
```
