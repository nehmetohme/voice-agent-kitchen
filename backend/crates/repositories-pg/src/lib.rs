//! Postgres implementations of the `vak-repositories` ports, built on sqlx,
//! with forward-only migrations in `migrations/` and the committed `.sqlx/`
//! offline query cache.
//!
//! This is the only crate allowed to depend on sqlx (enforced by `deny.toml`).
//! sqlx types never cross the port boundary.

mod agents;
mod database;
mod purpose_records;
mod purposes;
mod records;
mod scenario_records;
mod test_runs;

pub use agents::PgAgentRepo;
pub use database::{Database, DatabaseConfig, DatabaseError};
pub use purposes::PgPurposeRepo;
pub use test_runs::PgTestRunRepo;
