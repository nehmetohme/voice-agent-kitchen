# backend/: Rust workspace

## Responsibility

This folder holds the whole Voice Agent Kitchen server: a layered Cargo workspace (edition 2024, resolver 3). The toolchain is pinned to 1.99.1 in `rust-toolchain.toml`. Read the `CLAUDE.md` of a crate before you edit it.

## Layers

```
            apps/server (composition root: the only place that wires implementations)
                 │
   api ──► services ──► pipeline ──► providers (ports) ◄── adapters/* (implementations)
                 │                         ▲
                 └──► repositories (ports) ◄── repositories-pg (sqlx)
                                    │
                               domain (pure: types, rules, state machines; no I/O)
```

`layers.toml` lists the allowed edges, and `cargo xtask check-layers` enforces them. The table below is a summary:

| Crate | May depend on | Must never depend on |
|---|---|---|
| vak-domain | none; external: serde, thiserror, uuid, jiff | tokio, sqlx, axum, reqwest, any adapter |
| vak-audio | none | tokio net, sqlx, axum |
| vak-providers | domain, audio | reqwest, tungstenite, sqlx |
| vak-repositories | domain | sqlx, services, api |
| vak-repositories-pg | repositories, domain, databricks-auth | services, api, providers |
| vak-adapter-* | providers, domain, audio, databricks-auth | repositories, services, api, other adapters |
| vak-databricks-auth, vak-config, vak-telemetry | none | every other workspace crate |
| vak-pipeline | providers, domain, audio | repositories, api, adapters, axum, sqlx |
| vak-services | repositories, providers, pipeline, domain | repositories-pg, adapters, api, axum, sqlx |
| vak-api | services, domain | repositories(-pg), providers, pipeline, adapters, sqlx |
| vak-server | everything | — |
| vak-model-shim | config, telemetry | every product crate |
| vak-testkit, vak-contract-tests | domain, audio, repositories, providers (+ api for contract-tests) | being a normal or build dependency of anything |

- Every crate except domain, audio, providers, repositories, databricks-auth, config, telemetry, testkit, model-shim and xtask may take `vak-testkit` as a **dev-dependency**.
- `deny.toml` `wrappers` limit who may use sqlx (repositories-pg), axum (api, server, model-shim) and reqwest (adapters, databricks-auth, model-shim). openssl is banned; use rustls.
- `layers.toml` is owner-reviewed. Don't edit it. If you need a new edge, stop and ask.

## Conventions

- **Errors:** use `thiserror` in libraries, and `anyhow` only in `apps/`, `xtask/` and tests.
- **No panics:** never use `unwrap`, `expect`, `panic!` or `todo!` outside tests; clippy denies them.
- **Narrow APIs:** default to `pub(crate)`. `unreachable_pub` is on.
- **Timeouts:** every external call (HTTP, database, model) has one.
- **Secrets:** every credential is a `secrecy::SecretString`.
- **PII:** never log transcripts, audio or request bodies above `debug`.
- **Environment:** only `vak-server` reads it. Inner crates receive typed config.
- **Lints:** they come from `[workspace.lints]` in `Cargo.toml`, and every crate sets `[lints] workspace = true`. Cargo can't override them per package, so relax a lint only with a crate- or item-level `#[expect(lint, reason = "...")]`.
- **Dependency versions:** declare them once in `[workspace.dependencies]`, and add each entry in the same change as the first crate that uses it. cargo-shear and cargo-deny reject unused entries.
- **Internal edges:** add them only when code uses them. That's why most crates have no dependencies yet.

| Layer | Error convention |
|---|---|
| domain | `thiserror` enums of rule violations; no I/O error types |
| repositories | `RepoError { NotFound, Conflict, Unavailable(#[source] BoxError) }`; sqlx types never cross the port |
| providers/adapters | `ProviderError { Timeout, RateLimited, Auth, Protocol, Unavailable }` with a retryability flag |
| services | `ServiceError` per use case, built from domain, repo and provider errors |
| api | `ApiError: IntoResponse` producing RFC 9457 `application/problem+json`; logs the source chain once |
| apps, xtask, tests | `anyhow` |

## Commands

Run these from the repo root:

| Recipe | Runs |
|---|---|
| `just backend-verify` | fmt --check, clippy `--all-targets --all-features -D warnings`, check-layers, nextest under llvm-cov + coverage-gate, doctests, `cargo deny check`, `cargo audit`, `cargo shear` |
| `just backend-verify-fast` | fmt --check, clippy, check-layers |
| `just backend-test` | nextest + doctests |
| `just backend-fmt` | `cargo fmt --all`, `taplo fmt` |
| `just backend-coverage` | instrumented nextest run, then `cargo xtask coverage-gate` |

## Testing

- **Write contract tests first.** They match `**/tests/contract_*.rs`. Once a file is tagged `contract/<feature>/vN`, never edit it; bump the version instead.
- **Unit tests** cover every layer, using fakes from `vak-testkit` for the layer below.
- **nextest profiles** (`.config/nextest.toml`): `default` locally, `ci` in CI (JUnit, no retries). Audio fixtures: `fixtures/audio/README.md`.

## Coverage floors

The floors are in `coverage.toml` and gated by `cargo xtask coverage-gate`. They are line coverage, and they only go up.

- domain 95, services 90, pipeline 90, api 85, repositories-pg 85, adapters 75
- global 80 across all instrumented library code
- `apps/`, `xtask/`, `crates/testkit`, `crates/contract-tests` and `tests/` files are excluded
- crates with zero instrumented lines pass

## Adding a crate

1. Add the folder to `members` in `Cargo.toml`. The package is named `vak-*`, inherits `[workspace.package]` and sets `[lints] workspace = true`.
2. Ask the owner for a `[crates.vak-*]` entry in `layers.toml`; check-layers fails without one.
3. Write a crate-level `//!` doc comment and a `CLAUDE.md` in this format.
4. If the crate is a gated layer, add its floor to `coverage.toml`.
5. If it may use sqlx, axum or reqwest, the owner adds it to the matching `wrappers` in `deny.toml`.

## Example

`just backend-verify-fast` before every commit; `cd backend && cargo xtask check-layers` after touching any `Cargo.toml`.
