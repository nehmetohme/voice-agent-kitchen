//! Databricks authentication: an OAuth machine-to-machine token cache with
//! proactive refresh, and short-lived Lakebase Postgres credentials.
//!
//! Shared by the adapters and `vak-repositories-pg`. Tokens are always
//! `secrecy::SecretString` and are never logged.
