---
paths:
  - "backend/**/*.rs"
  - "backend/**/Cargo.toml"
---

# Backend layers

Dependencies point down only. `backend/layers.toml` is the source; `cargo xtask check-layers` and cargo-deny enforce it.

```
api → services → pipeline → providers (ports) ← adapters/*
         └────→ repositories (ports) ← repositories-pg
everything → domain          apps/server wires implementations to ports
```

- `domain`: pure types and rules. No I/O, async, tokio, sqlx, axum or reqwest.
- `providers`, `repositories`: traits and port types only.
- Adapters: one port each. Map vendor types and errors at the boundary. Never depend on `services`, `api`, `pipeline` or another adapter.
- `api`: parse, validate, call one service, map errors to problem+json. Every route has auth and is in the OpenAPI document.
- `services`: one public function per use case. Rules come from `domain`; the outside is reached through ports only.
- `pipeline`: no blocking calls in async code, bounded channels with a declared overflow policy, a latency histogram per stage.
- sqlx only in `repositories-pg`, axum only in `api` and apps, reqwest only in adapters and `databricks-auth`.
- `thiserror` in libraries, `anyhow` only in apps. No `unwrap` or `expect` outside tests. Every external call has a timeout.
- Never edit `backend/layers.toml` or `backend/**/.sqlx/**`. Read the crate's `CLAUDE.md` before you edit it.
