# vak-repositories: persistence ports

## Responsibility

This crate defines the persistence ports:

- one trait per aggregate: `AgentRepo`, `PlanRepo`, `VersionRepo`, `TestRunRepo`, `SessionRepo`, `AuditRepo`
- the `JobQueue` port, which hides apalis-postgres from services
- `RepoError`, plus paging and filter types

It contains **traits and port types only**. The Postgres implementation lives in `vak-repositories-pg`, and the in-memory fakes live in `vak-testkit`.

## Dependencies

- **May depend on:** `vak-domain`.
- **External crates:** `thiserror`, plus the async-trait helper used by `vak-providers`, if any.
- **Must not depend on:** sqlx or any database driver, services, api, providers, pipeline, adapters.

## Conventions

- **Domain types only:** ports take and return `vak-domain` types (`Agent`, `AgentId`, `PublishedVersion`), never rows, DTOs or sqlx types.
- **Errors:** `RepoError { NotFound, Conflict, Unavailable(#[source] BoxError) }`. Implementations map driver errors into it; the driver error is a source, never a variant.
- **Concurrency:** use optimistic concurrency through a version or etag argument on updates. A stale write returns `Conflict`.
- **Transactions:** a use case that must be atomic gets one port method. Don't leak transaction handles into services.
- **Ports are `pub`;** everything else is `pub(crate)`.

## Testing

- **Conformance suite:** each port has one in `vak-testkit`, for example `testkit::repositories::agent_repo_conformance`. It runs against both the in-memory fake and `vak-repositories-pg` (with `#[sqlx::test]`), so the fake can't drift from Postgres.
- **Tests here** cover only pure helpers, such as paging cursors and `RepoError` classification.

## Coverage floor

No per-crate floor; this crate counts toward the global 80%.

## Example

This shows the pattern:

```rust
pub trait AgentRepo: Send + Sync {
    fn get(&self, id: AgentId) -> impl Future<Output = Result<Agent, RepoError>> + Send;

    /// Fails with `RepoError::Conflict` when `expected` is not the stored revision.
    fn update(
        &self,
        agent: &Agent,
        expected: Revision,
    ) -> impl Future<Output = Result<Revision, RepoError>> + Send;
}
```
