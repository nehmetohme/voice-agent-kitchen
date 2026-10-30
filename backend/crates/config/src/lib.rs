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
//! assert_eq!(settings.profile, vak_config::Profile::Local);
//! # Ok::<(), vak_config::ConfigError>(())
//! ```

use std::net::{AddrParseError, IpAddr, Ipv4Addr, SocketAddr};

use secrecy::SecretString;

/// Overrides [`HttpSettings::addr`], as `IP:port` (e.g. `0.0.0.0:8000`).
pub const HTTP_ADDR_VAR: &str = "VAK_HTTP_ADDR";
/// Overrides [`LogSettings::filter`], in `tracing` `EnvFilter` syntax (e.g. `info,vak_api=debug`).
pub const LOG_VAR: &str = "VAK_LOG";
/// Sets [`Settings::profile`]: `local` (default) or `databricks`.
pub const PROFILE_VAR: &str = "VAK_PROFILE";
/// Sets [`DatabaseSettings::url`]: a `postgres://` URL. The name sqlx and psql use.
pub const DATABASE_URL_VAR: &str = "DATABASE_URL";
/// Sets [`AuthSettings::dev_operator_email`]. Allowed only in the `local` profile.
pub const DEV_OPERATOR_VAR: &str = "VAK_DEV_OPERATOR_EMAIL";

/// All settings, validated.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub profile: Profile,
    pub http: HttpSettings,
    pub log: LogSettings,
    pub database: DatabaseSettings,
    pub auth: AuthSettings,
}

/// Where the server runs. Switching profiles changes configuration only, never code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Profile {
    /// A developer's machine: local models and Postgres. The default.
    #[default]
    Local,
    /// Databricks Apps: Model Serving, Lakebase and single sign-on.
    Databricks,
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

/// Postgres (local) or Lakebase (Databricks).
#[derive(Clone, Debug, Default)]
pub struct DatabaseSettings {
    /// `postgres://` URL from [`DATABASE_URL_VAR`]. It may carry a password, so it is a
    /// secret: `Debug` never prints it. `serve` and `migrate` need it.
    pub url: Option<SecretString>,
}

/// How callers are identified.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuthSettings {
    /// Local development only: requests without the Databricks identity headers act as this
    /// operator. `None` (the default) means every API request needs the headers. Setting it
    /// in any profile but `local` is a configuration error.
    pub dev_operator_email: Option<String>,
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
    #[error("{var} must be local or databricks")]
    InvalidProfile { var: &'static str },
    #[error("{var} is allowed only in the local profile; unset it")]
    DevOperatorOutsideLocal { var: &'static str },
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

        if let Some(profile) = value(PROFILE_VAR) {
            settings.profile = match profile.to_ascii_lowercase().as_str() {
                "local" => Profile::Local,
                "databricks" => Profile::Databricks,
                _ => return Err(ConfigError::InvalidProfile { var: PROFILE_VAR }),
            };
        }
        if let Some(addr) = value(HTTP_ADDR_VAR) {
            settings.http.addr = addr.parse().map_err(|source| ConfigError::InvalidAddr {
                var: HTTP_ADDR_VAR,
                source,
            })?;
        }
        if let Some(filter) = value(LOG_VAR) {
            settings.log.filter = filter;
        }
        settings.database.url = value(DATABASE_URL_VAR).map(SecretString::from);
        if let Some(email) = value(DEV_OPERATOR_VAR) {
            if settings.profile != Profile::Local {
                return Err(ConfigError::DevOperatorOutsideLocal {
                    var: DEV_OPERATOR_VAR,
                });
            }
            settings.auth.dev_operator_email = Some(email);
        }
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use secrecy::ExposeSecret as _;

    use super::*;

    fn lookup<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn defaults_are_local_on_localhost_3000_with_no_database_or_dev_operator() {
        let settings = Settings::from_lookup(lookup(&[])).expect("defaults are valid");

        assert_eq!(settings.profile, Profile::Local);
        assert_eq!(settings.http, HttpSettings::default());
        assert_eq!(settings.http.addr.to_string(), "127.0.0.1:3000");
        assert_eq!(settings.log.filter, "info");
        assert!(settings.database.url.is_none());
        assert_eq!(settings.auth, AuthSettings::default());
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
            ("DATABASE_URL", " "),
            ("VAK_DEV_OPERATOR_EMAIL", ""),
        ]))
        .expect("valid");

        assert_eq!(settings.http.addr.port(), 4000);
        assert_eq!(settings.log.filter, "info");
        assert!(settings.database.url.is_none());
        assert_eq!(settings.auth.dev_operator_email, None);
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

    #[test]
    fn the_profile_is_parsed_in_any_case_and_rejected_otherwise() {
        let databricks =
            Settings::from_lookup(lookup(&[("VAK_PROFILE", "Databricks")])).expect("valid");
        let error = Settings::from_lookup(lookup(&[("VAK_PROFILE", "prod")])).expect_err("bad");

        assert_eq!(databricks.profile, Profile::Databricks);
        assert_eq!(error.to_string(), "VAK_PROFILE must be local or databricks");
    }

    #[test]
    fn the_database_url_is_a_secret_debug_never_prints() {
        let settings = Settings::from_lookup(lookup(&[(
            "DATABASE_URL",
            "postgres://vak:hunter2@db:5432/vak",
        )]))
        .expect("valid");

        let url = settings.database.url.as_ref().expect("set");
        assert_eq!(url.expose_secret(), "postgres://vak:hunter2@db:5432/vak");
        assert!(!format!("{settings:?}").contains("hunter2"));
    }

    #[test]
    fn a_dev_operator_is_allowed_only_in_the_local_profile() {
        let local = Settings::from_lookup(lookup(&[("VAK_DEV_OPERATOR_EMAIL", "dev@localhost")]))
            .expect("local is the default");
        let error = Settings::from_lookup(lookup(&[
            ("VAK_PROFILE", "databricks"),
            ("VAK_DEV_OPERATOR_EMAIL", "dev@localhost"),
        ]))
        .expect_err("never on Databricks");

        assert_eq!(
            local.auth.dev_operator_email.as_deref(),
            Some("dev@localhost")
        );
        let message = error.to_string();
        assert!(message.contains("VAK_DEV_OPERATOR_EMAIL"), "{message}");
        assert!(!message.contains("dev@localhost"), "{message}");
    }
}
