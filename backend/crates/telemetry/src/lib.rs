//! Telemetry setup: the tracing subscriber today; OpenTelemetry export,
//! metric names and histogram buckets later.
//!
//! Only the composition root calls into this crate.

use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::ParseError;
use tracing_subscriber::util::{SubscriberInitExt as _, TryInitError};

/// Telemetry could not be set up.
#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("invalid log filter")]
    InvalidFilter(#[source] ParseError),
    #[error("a global tracing subscriber is already installed")]
    AlreadyInitialized(#[source] TryInitError),
}

/// Installs the global tracing subscriber: human-readable output on stderr, filtered by
/// `filter` in `EnvFilter` syntax (e.g. `info,vak_api=debug`). Call it once, at startup.
pub fn init(filter: &str) -> Result<(), TelemetryError> {
    let filter = EnvFilter::try_new(filter).map_err(TelemetryError::InvalidFilter)?;
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .finish()
        .try_init()
        .map_err(TelemetryError::AlreadyInitialized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_filter_is_rejected_before_anything_is_installed() {
        let error = init("vak_api=loud").expect_err("`loud` is not a level");

        assert!(matches!(error, TelemetryError::InvalidFilter(_)));
        assert!(std::error::Error::source(&error).is_some());
    }

    // The only test that installs the global subscriber, so it holds under `cargo test` too.
    #[test]
    fn installs_once_then_reports_already_initialized() {
        init("info,vak=debug").expect("first install succeeds");

        let error = init("info").expect_err("second install fails");

        assert!(matches!(error, TelemetryError::AlreadyInitialized(_)));
    }
}
