# vak-repositories-pg: Postgres implementations

## Responsibility

This crate implements the `vak-repositories` ports on Postgres 18 with sqlx 0.9. It owns:

- `migrations/`: forward-only `<timestamp>_<name>.sql` files, embedded with `sqlx::migrate!()` and run by `vak-server migrate`
- `.sqlx/`: the committed offline query cache, generated and **never hand-edited**
- connection pool setup and Lakebase credential refresh, through `vak-databricks-auth`
- the `JobQueue` implementation (apalis-postgres)

## Dependencies

- **May depend on:** `vak-repositories`, `vak-domain`, `vak-databricks-auth`. Dev-only: `vak-testkit`.
- **External crates:** `sqlx` (this is the only crate allowed; `deny.toml` `wrappers`), `apalis-postgres`, `thiserror`, `secrecy`.
- **Must not depend on:** services, api, providers, pipeline, adapters, axum, reqwest.

## Conventions

- **SQL:** raw SQL with compile-time checked macros (`query!`, `query_as!`).
- **Dynamic SQL:** it needs `AssertSqlSafe` and a comment explaining why. Never use `format!`.
- **Port boundary:** map every sqlx error into `RepoError` (unique violation → `Conflict`, row not found → `NotFound`, anything else → `Unavailable`). sqlx types never cross the port.
- **IDs and snapshots:** UUIDv7 IDs come from the domain, with `uuidv7()` as the column default. Published versions are immutable JSONB snapshots.
- **Postgres features:**
  - job claims: `FOR UPDATE SKIP LOCKED`
  - per-campaign limits: advisory locks
  - wake-ups: `LISTEN/NOTIFY`
- **Timeouts:** every query runs under a statement timeout or pool acquire timeout. No unbounded waits.
- **Credentials:** they are `SecretString`. The Lakebase OAuth token is the password and is refreshed hourly. Never log connection strings.
- **Offline builds:** builds use `SQLX_OFFLINE=true`. After changing a query or migration, run `just db-prepare` (the recipe arrives with the first migration) and commit `.sqlx/`. CI runs `cargo sqlx prepare --workspace --check`.

## Testing

- `#[sqlx::test]` gives each test a fresh database with migrations applied. Postgres comes from testcontainers locally and from a service container in CI.
- Every repository passes the matching conformance suite from `vak-testkit`, the same suite the in-memory fake passes.
- Migrations get a test that applies them to an empty database.

## Coverage floor

**85%** line coverage (`coverage.toml`).

## Example

This shows the pattern:

```rust
pub(crate) async fn get(pool: &PgPool, id: AgentId) -> Result<Agent, RepoError> {
    let row = sqlx::query_as!(AgentRow, "SELECT id, name, revision FROM agents WHERE id = $1", id.as_uuid())
        .fetch_optional(pool)
        .await
        .map_err(RepoError::unavailable)?
        .ok_or(RepoError::NotFound)?;
    row.try_into()
}
```
