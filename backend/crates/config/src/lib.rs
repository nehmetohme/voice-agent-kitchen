//! Typed application settings with defaults and overrides.
//!
//! This crate never reads the process environment itself: the composition root
//! passes a lookup function, which keeps loading pure and testable.
//!
//! ```
//! let settings = vak_config::Settings::from_lookup(|name| match name {
//!     "VAK_HTTP_ADDR" => Some("0.0.0.0:8080".to_owned()),
//!     _ => None,
//! })?;
//! assert_eq!(settings.http.addr.port(), 8080);
//! assert_eq!(settings.log.filter, "info");
//! # Ok::<(), vak_config::ConfigError>(())
//! ```

use std::net::{AddrParseError, IpAddr, Ipv4Addr, SocketAddr};

/// Overrides [`HttpSettings::addr`], as `IP:port` (e.g. `0.0.0.0:8000`).
pub const HTTP_ADDR_VAR: &str = "VAK_HTTP_ADDR";
/// Overrides [`LogSettings::filter`], in `tracing` `EnvFilter` syntax (e.g. `info,vak_api=debug`).
pub const LOG_VAR: &str = "VAK_LOG";

/// All settings, validated.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub http: HttpSettings,
    pub log: LogSettings,
}

/// The HTTP listener.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpSettings {
    /// Address to bind. Default `127.0.0.1:3000`.
    pub addr: SocketAddr,
}

/// Log output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogSettings {
    /// Which spans and events to record. Default `info`.
    pub filter: String,
}

impl Default for HttpSettings {
    fn default() -> Self {
        Self {
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000),
        }
    }
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            filter: "info".to_owned(),
        }
    }
}

/// A setting that could not be loaded. Never contains the offending value, which may be
/// a secret for other settings.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{var} is not a socket address; expected IP:port, e.g. 127.0.0.1:3000")]
    InvalidAddr {
        var: &'static str,
        #[source]
        source: AddrParseError,
    },
}

impl Settings {
    /// Defaults, overridden by the variables `lookup` returns. Values are trimmed; empty
    /// values count as unset.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let value = |name: &str| {
            lookup(name)
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        let mut settings = Self::default();

        if let Some(addr) = value(HTTP_ADDR_VAR) {
            settings.http.addr = addr.parse().map_err(|source| ConfigError::InvalidAddr {
                var: HTTP_ADDR_VAR,
                source,
            })?;
        }
        if let Some(filter) = value(LOG_VAR) {
            settings.log.filter = filter;
        }
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn defaults_listen_on_localhost_3000_and_log_info() {
        let settings = Settings::from_lookup(lookup(&[])).expect("defaults are valid");

        assert_eq!(settings, Settings::default());
        assert_eq!(settings.http.addr.to_string(), "127.0.0.1:3000");
        assert_eq!(settings.log.filter, "info");
    }

    #[test]
    fn http_addr_is_overridden_by_vak_http_addr() {
        let settings =
            Settings::from_lookup(lookup(&[("VAK_HTTP_ADDR", "0.0.0.0:8000")])).expect("valid");

        assert_eq!(settings.http.addr.to_string(), "0.0.0.0:8000");
    }

    #[test]
    fn ipv6_addresses_are_accepted() {
        let settings =
            Settings::from_lookup(lookup(&[("VAK_HTTP_ADDR", "[::1]:3001")])).expect("valid");

        assert_eq!(settings.http.addr.to_string(), "[::1]:3001");
    }

    #[test]
    fn values_are_trimmed_and_empty_values_are_unset() {
        let settings = Settings::from_lookup(lookup(&[
            ("VAK_HTTP_ADDR", "  127.0.0.1:4000\n"),
            ("VAK_LOG", "   "),
        ]))
        .expect("valid");

        assert_eq!(settings.http.addr.port(), 4000);
        assert_eq!(settings.log.filter, "info");
    }

    #[test]
    fn log_filter_is_overridden_by_vak_log() {
        let settings =
            Settings::from_lookup(lookup(&[("VAK_LOG", "debug,hyper=warn")])).expect("valid");

        assert_eq!(settings.log.filter, "debug,hyper=warn");
    }

    #[test]
    fn invalid_addr_names_the_variable_but_not_the_value() {
        let error = Settings::from_lookup(lookup(&[("VAK_HTTP_ADDR", "localhost:secret")]))
            .expect_err("a host name is not an IP address");

        let message = error.to_string();
        assert!(message.contains("VAK_HTTP_ADDR"), "{message}");
        assert!(!message.contains("secret"), "{message}");
        assert!(std::error::Error::source(&error).is_some());
    }
}
