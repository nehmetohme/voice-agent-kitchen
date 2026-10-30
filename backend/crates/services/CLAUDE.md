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

Today (M2):

- `AgentService::new(Arc<dyn AgentRepo>)`: `create` (defaults from the domain), `list` (active agents with their `SetupProgress`, newest first, at most `LIST_LIMIT` = 200), `get`, `progress`, `update_draft` (autosave; a no-op patch stores and audits nothing; retries up to 3 times on a stale revision) and `archive` (idempotent). Errors: `CreateAgentError`, `ListAgentsError`, `FindAgentError`, `UpdateDraftError`.
- `catalog::voices()` and `catalog::languages()`: the domain's static catalog.

## Dependencies

- **May depend on:** `vak-repositories`, `vak-providers`, `vak-pipeline`, `vak-domain`. Dev-only: `vak-testkit`.
- **External crates:** `thiserror`, `tracing`, `tokio` (only what orchestration needs).
- **Must not depend on:** repositories-pg, any adapter, api, axum, sqlx, reqwest.

## Conventions

- **One public function per use case,** such as `publish_version(&self, cmd: PublishVersion) -> Result<PublishedVersion, PublishError>`. Inputs are command structs of domain types, not DTOs.
- **Business rules live in `vak-domain`.** Services load, call domain rules, persist and emit audit events. If you're writing an `if` about business policy here, move it to the domain.
- **The outside world is reached only through ports.** Repository ports are dyn-compatible and held as `Arc<dyn …>`, so `vak-api` stays free of generics and `vak-server` picks the implementation. Use generics over provider ports on hot paths (the pipeline).
- **IDs and time:** services generate UUID v7 IDs and read the clock (`now()` rounds to microseconds, Postgres precision); the domain never does.
- **Errors:** a `ServiceError` per use case (`thiserror`), built from domain, `RepoError` and `ProviderError`. Don't stringify errors.
- **Timeouts:** every port call runs under a deadline (`AgentService` default 5 s, `with_deadline` to change it). Transactions are port methods, not service code.
- **Audit:** every state change writes an audit event. Build it with the domain's `AuditEvent` and pass it to the same repository call that stores the change, so both commit in one transaction.
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

The shape of `AgentService::archive` (`src/agents.rs`):

```rust
pub async fn archive(&self, actor: &Actor, id: AgentId) -> Result<(), FindAgentError> {
    let stored = self.within(self.repo.get(id)).await?;
    let Some(archived) = stored.value.archive(now()) else {
        return Ok(()); // the rule lives in vak-domain: archiving twice changes nothing
    };
    let event = audit_event(actor, &archived, AuditAction::AgentArchived)?;
    // One port call: the change and its audit event commit together.
    self.save(&archived, stored.revision, &event).await?;
    Ok(())
}
```
