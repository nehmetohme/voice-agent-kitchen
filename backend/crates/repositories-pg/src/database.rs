//! The connection pool and migrations.

use std::str::FromStr as _;
use std::time::Duration;

use secrecy::{ExposeSecret as _, SecretString};
use sqlx::PgPool;
use sqlx::migrate::{MigrateError, Migrator};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

use crate::PgAgentRepo;

/// The migrations in `migrations/`, embedded at compile time.
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// How to reach Postgres.
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// `postgres://` URL. It may carry a password, so it is a secret.
    pub url: SecretString,
    /// Pool size.
    pub max_connections: u32,
    /// How long a caller waits for a free connection.
    pub acquire_timeout: Duration,
    /// Postgres cancels any statement that runs longer.
    pub statement_timeout: Duration,
}

impl DatabaseConfig {
    /// `url` with defaults that suit one app instance: 10 connections, 5 s to acquire one,
    /// 5 s per statement.
    #[must_use]
    pub fn new(url: SecretString) -> Self {
        Self {
            url,
            max_connections: 10,
            acquire_timeout: Duration::from_secs(5),
            statement_timeout: Duration::from_secs(5),
        }
    }
}

/// The database could not be set up. Messages never contain the URL.
#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("the database URL is not a valid postgres:// URL")]
    InvalidUrl(#[source] sqlx::Error),
    #[error("could not run the database migrations")]
    Migrate(#[source] MigrateError),
}

/// The Postgres pool, and the repositories built on it.
#[derive(Debug, Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// A pool that connects on first use, so the server starts (and `/health` answers) while
    /// the database is still coming up.
    pub fn connect_lazy(config: &DatabaseConfig) -> Result<Self, DatabaseError> {
        let statement_timeout = format!("{}ms", config.statement_timeout.as_millis());
        let options = PgConnectOptions::from_str(config.url.expose_secret())
            .map_err(DatabaseError::InvalidUrl)?
            .application_name("vak-server")
            .options([("statement_timeout", statement_timeout.as_str())]);
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(config.acquire_timeout)
            .connect_lazy_with(options);
        Ok(Self { pool })
    }

    /// Applies every migration not applied yet. Forward-only; safe to run on every deploy.
    pub async fn migrate(&self) -> Result<(), DatabaseError> {
        MIGRATOR
            .run(&self.pool)
            .await
            .map_err(DatabaseError::Migrate)
    }

    /// The `AgentRepo` on this pool.
    #[must_use]
    pub fn agents(&self) -> PgAgentRepo {
        PgAgentRepo::new(self.pool.clone())
    }
}
