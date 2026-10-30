//! Dev-only HTTP contract tests: boot the API with testkit fakes and check
//! every response against the committed `contracts/openapi.json`.
//!
//! The tests live in `tests/`. No crate may depend on this one as a normal or
//! build dependency.
