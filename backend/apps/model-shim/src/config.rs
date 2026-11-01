//! Settings from the environment, loaded through a lookup function so tests stay pure.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use anyhow::{Context as _, bail};

/// Listen address, `IP:port`, loopback only (the shim has no auth). Default `127.0.0.1:8001`.
pub(crate) const ADDR_VAR: &str = "VAK_SHIM_ADDR";
/// mlx-audio base URL (`http://` only). Default `http://127.0.0.1:8000`.
pub(crate) const MLX_AUDIO_URL_VAR: &str = "VAK_SHIM_MLX_AUDIO_URL";
/// The `model` sent to mlx-audio for STT: a Hugging Face repo or a local folder.
pub(crate) const STT_MODEL_VAR: &str = "VAK_SHIM_STT_MODEL";
/// The `model` sent to mlx-audio for TTS: a Hugging Face repo or a local folder.
pub(crate) const TTS_MODEL_VAR: &str = "VAK_SHIM_TTS_MODEL";
/// Deadline of each forwarded call, in milliseconds (1 to 600000). Default 30000: the
/// first call also loads the model.
pub(crate) const TIMEOUT_MS_VAR: &str = "VAK_SHIM_TIMEOUT_MS";
/// Log filter in `tracing` `EnvFilter` syntax. Default `info`.
pub(crate) const LOG_VAR: &str = "VAK_LOG";

const MAX_TIMEOUT_MS: u64 = 600_000;

/// All settings, validated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Config {
    pub(crate) addr: SocketAddr,
    /// Without a trailing slash.
    pub(crate) mlx_audio_url: String,
    pub(crate) stt_model: String,
    pub(crate) tts_model: String,
    pub(crate) timeout: Duration,
    pub(crate) log_filter: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8001),
            mlx_audio_url: "http://127.0.0.1:8000".to_owned(),
            stt_model: "mlx-community/parakeet-tdt-0.6b-v2".to_owned(),
            tts_model: "mlx-community/Kokoro-82M-bf16".to_owned(),
            timeout: Duration::from_secs(30),
            log_filter: "info".to_owned(),
        }
    }
}

impl Config {
    /// Defaults, overridden by the variables `lookup` returns. Values are trimmed; empty
    /// values count as unset.
    pub(crate) fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let value = |name: &str| {
            lookup(name)
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        let mut config = Self::default();

        if let Some(addr) = value(ADDR_VAR) {
            config.addr = addr
                .parse()
                .with_context(|| format!("{ADDR_VAR} is not IP:port, e.g. 127.0.0.1:8001"))?;
            if !config.addr.ip().is_loopback() {
                bail!(
                    "{ADDR_VAR} must be a loopback address (127.0.0.1 or [::1]): the shim has \
                     no auth, so it never listens on the network"
                );
            }
        }
        if let Some(url) = value(MLX_AUDIO_URL_VAR) {
            let url = url.trim_end_matches('/');
            let host = url.strip_prefix("http://").unwrap_or_default();
            if host.is_empty() || host.contains(['/', '?', '#']) {
                bail!("{MLX_AUDIO_URL_VAR} must be http://host:port, e.g. http://127.0.0.1:8000");
            }
            url.clone_into(&mut config.mlx_audio_url);
        }
        if let Some(model) = value(STT_MODEL_VAR) {
            config.stt_model = model;
        }
        if let Some(model) = value(TTS_MODEL_VAR) {
            config.tts_model = model;
        }
        if let Some(ms) = value(TIMEOUT_MS_VAR) {
            let ms: u64 = ms
                .parse()
                .ok()
                .filter(|ms| (1..=MAX_TIMEOUT_MS).contains(ms))
                .with_context(|| {
                    format!("{TIMEOUT_MS_VAR} must be milliseconds from 1 to {MAX_TIMEOUT_MS}")
                })?;
            config.timeout = Duration::from_millis(ms);
        }
        if let Some(filter) = value(LOG_VAR) {
            config.log_filter = filter;
        }
        Ok(config)
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
    fn defaults_match_the_local_stack() {
        let config = Config::from_lookup(lookup(&[])).expect("defaults are valid");

        assert_eq!(config, Config::default());
        assert_eq!(config.addr.to_string(), "127.0.0.1:8001");
        assert_eq!(config.mlx_audio_url, "http://127.0.0.1:8000");
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn every_setting_is_overridden_and_trimmed() {
        let config = Config::from_lookup(lookup(&[
            (ADDR_VAR, " 127.0.0.1:9001 "),
            (MLX_AUDIO_URL_VAR, "http://10.0.0.5:8000/"),
            (STT_MODEL_VAR, "/models/parakeet"),
            (TTS_MODEL_VAR, "/models/kokoro"),
            (TIMEOUT_MS_VAR, "250"),
            (LOG_VAR, "debug"),
        ]))
        .expect("valid");

        assert_eq!(config.addr.to_string(), "127.0.0.1:9001");
        assert_eq!(config.mlx_audio_url, "http://10.0.0.5:8000");
        assert_eq!(config.stt_model, "/models/parakeet");
        assert_eq!(config.tts_model, "/models/kokoro");
        assert_eq!(config.timeout, Duration::from_millis(250));
        assert_eq!(config.log_filter, "debug");
    }

    #[test]
    fn empty_values_count_as_unset() {
        let config = Config::from_lookup(lookup(&[(STT_MODEL_VAR, "  "), (TIMEOUT_MS_VAR, "")]))
            .expect("valid");

        assert_eq!(config, Config::default());
    }

    #[test]
    fn invalid_values_name_the_variable() {
        for (var, value) in [
            (ADDR_VAR, "localhost"),
            (ADDR_VAR, "0.0.0.0:8001"),
            (ADDR_VAR, "192.168.1.5:8001"),
            (MLX_AUDIO_URL_VAR, "https://127.0.0.1:8000"),
            (MLX_AUDIO_URL_VAR, "http://"),
            (MLX_AUDIO_URL_VAR, "http://127.0.0.1:8000/v1"),
            (TIMEOUT_MS_VAR, "0"),
            (TIMEOUT_MS_VAR, "600001"),
            (TIMEOUT_MS_VAR, "soon"),
        ] {
            let error = Config::from_lookup(lookup(&[(var, value)])).expect_err(value);

            assert!(error.to_string().contains(var), "{var}={value}: {error}");
        }
    }
}
