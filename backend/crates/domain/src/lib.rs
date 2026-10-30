//! Pure domain model: entities, value objects, IDs, the plan graph and its
//! validation, rule packs, decision types and lifecycle state machines.
//!
//! This crate has no I/O, no async and no framework dependencies. Every rule is a
//! pure function with unit and property tests. The only external crates allowed
//! are listed under `[crates.vak-domain]` in `backend/layers.toml`.
