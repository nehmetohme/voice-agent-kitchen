# vak-contract-tests: HTTP tests against the OpenAPI contract (dev-only)

## Responsibility

This crate boots the API with `vak-testkit` fakes behind the services and checks that responses match the committed `contracts/openapi.json`:

- every documented route answers
- every response body validates against its schema (`jsonschema` crate)
- every error is `application/problem+json`

The tests live in `tests/`. `src/lib.rs` holds only shared helpers, such as loading the spec and a schema validator.

## Dependencies

- **May depend on:** `vak-domain`, `vak-audio`, `vak-repositories`, `vak-providers`, `vak-api`. Dev-only: `vak-testkit`.
- **External crates:** `jsonschema`, `serde_json`, `tokio`, `tower` (`oneshot`), `http-body-util`.
- **Must not depend on:** repositories-pg, adapters, pipeline, server, sqlx, reqwest.
- **Dev-only:** no crate may list this one under `[dependencies]` or `[build-dependencies]`; `cargo xtask check-layers` fails if one does.

## Conventions

- **Contract files** match `**/tests/contract_*.rs` and are pinned by signed `contract/<feature>/vN` tags.
  - **Never edit a tagged file.** To change a contract, add a new version and get it signed.
  - Register every contract file in `contracts/contracts.toml`.
- **The spec is the source of truth.** Never edit `contracts/openapi.json` by hand; regenerate it with `just openapi`.
- **No network:** drive the router in-process with `tower::ServiceExt::oneshot`, no sockets.
- **Fixtures:** request bodies come from testkit builders, never from real customer data.
- **Fuzzing** (Schemathesis) runs against the same spec in CI, separately from these tests.

## Testing

This crate *is* tests. Each contract test asserts the status, content type and full body shape, not just "200 OK".

## Coverage floor

Excluded from coverage. Its tests count toward the coverage of the crates they exercise.

## Example

This shows the pattern:

```rust
#[tokio::test]
async fn contract_agents_list_matches_schema() {
    let spec = vak_contract_tests::spec();
    let response = app_with_fakes().oneshot(get("/api/agents")).await.unwrap();
    spec.assert_response("listAgents", &response).await;
}
```
