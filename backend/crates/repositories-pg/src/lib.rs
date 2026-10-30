//! Postgres implementations of the `vak-repositories` ports, built on sqlx,
//! with forward-only migrations in `migrations/` and the committed `.sqlx/`
//! offline query cache.
//!
//! This is the only crate allowed to depend on sqlx (enforced by `deny.toml`).
//! sqlx types never cross the port boundary.
