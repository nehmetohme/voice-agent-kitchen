//! Persistence ports: one trait per aggregate (`AgentRepo`, `PlanRepo`,
//! `VersionRepo`, `TestRunRepo`, `SessionRepo`, `AuditRepo`) and the `JobQueue`
//! port, plus `RepoError`.
//!
//! Traits and port types only. The Postgres implementation lives in
//! `vak-repositories-pg`; in-memory fakes live in `vak-testkit`.
