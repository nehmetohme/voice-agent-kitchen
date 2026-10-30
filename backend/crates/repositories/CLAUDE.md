# vak-repositories: persistence ports

## Responsibility

This crate defines the persistence ports:

- one trait per aggregate: `AgentRepo` today; `PlanRepo`, `VersionRepo`, `TestRunRepo`, `SessionRepo` and `AuditRepo` arrive with their milestones
- the `JobQueue` port, which hides apalis-postgres from services
- `RepoError`, `Revision`, `Versioned<T>`, plus paging and filter types (`AgentQuery`)

It contains **traits and port types only**. The Postgres implementation lives in `vak-repositories-pg`, and the in-memory fakes live in `vak-testkit`.

`AgentRepo` stores an agent with its draft. Each mutation (`create`, `update`) also takes the `AuditEvent` that records it, and stores both in one transaction.

## Dependencies

- **May depend on:** `vak-domain`.
- **External crates:** `thiserror`. No async-trait helper: ports return `BoxFuture` (below).
- **Must not depend on:** sqlx or any database driver, services, api, providers, pipeline, adapters.

## Conventions

- **Domain types only:** ports take and return `vak-domain` types (`Agent`, `AgentId`, `AuditEvent`), never rows, DTOs or sqlx types.
- **Dyn-compatible ports:** methods return `BoxFuture<'_, Result<T, RepoError>>`, so services hold `Arc<dyn AgentRepo>` and `vak-server` picks the implementation. One allocation per call is noise next to a database round trip.
- **Errors:** `RepoError { NotFound, Conflict, Unavailable(#[source] BoxError) }`. Implementations map driver errors into it; the driver error is a source, never a variant. A stored row the domain rejects is `Unavailable`.
- **Concurrency:** optimistic, through `Revision` (starts at 1, +1 per update). `get` returns `Versioned<T>`; `update` takes the expected revision, and a stale write returns `Conflict`.
- **Transactions:** a use case that must be atomic gets one port method, and the audit event travels with the change. Don't leak transaction handles into services.
- **Ports are `pub`;** everything else is `pub(crate)`.

## Testing

- **Conformance suite:** each port has one in `vak-testkit`, e.g. `vak_testkit::repositories::agent_repo_conformance`. It runs against both the in-memory fake and `vak-repositories-pg` (with `#[sqlx::test]`), so the fake can't drift from Postgres.
- **Tests here** cover only pure helpers, such as `Revision` and `RepoError` messages.

## Coverage floor

No per-crate floor; this crate counts toward the global 80%.

## Example

```rust
pub trait AgentRepo: Send + Sync {
    fn get(&self, id: AgentId) -> BoxFuture<'_, Result<Versioned<Agent>, RepoError>>;

    /// Fails with `RepoError::Conflict` when `expected` is not the stored revision.
    fn update<'a>(
        &'a self,
        agent: &'a Agent,
        expected: Revision,
        event: &'a AuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>>;
}
```
