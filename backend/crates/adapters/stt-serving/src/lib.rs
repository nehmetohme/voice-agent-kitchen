//! `SttEngine` adapter that calls a speech-to-text model over the Databricks
//! Model Serving invocations contract (`contracts/serving/stt.json`). The local
//! model shim speaks the same contract, so dev and prod share this code.
//!
//! ```text
//! POST {base_url}/serving-endpoints/{endpoint}/invocations
//! {"dataframe_records":[{"audio_b64":"…","encoding":"pcm_s16le","sample_rate":16000,"language":"en"}]}
//! → {"predictions":[{"text":"…"}]}
//! ```

use std::time::Duration;

use secrecy::SecretString;
use vak_providers::{BoxFuture, ProviderError, SttEngine, SttRequest, Transcript};

/// Where the STT endpoint is and how to authenticate.
#[derive(Debug, Clone)]
#[expect(dead_code, reason = "CONTRACT-STUB: the engine reads them")]
pub struct ServingSttConfig {
    base_url: String,
    endpoint: String,
    token: Option<SecretString>,
    connect_timeout: Duration,
}

impl ServingSttConfig {
    /// `base_url` is the workspace (or local shim) root, `endpoint` the serving endpoint's
    /// name, e.g. `stt`. No authentication; connections time out after 2 s.
    #[must_use]
    pub fn new(base_url: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            endpoint: endpoint.into(),
            token: None,
            connect_timeout: Duration::from_secs(2),
        }
    }

    /// Sends `Authorization: Bearer <token>` with every request.
    #[must_use]
    pub fn with_bearer_token(self, token: SecretString) -> Self {
        Self {
            token: Some(token),
            ..self
        }
    }

    #[must_use]
    pub fn with_connect_timeout(self, connect_timeout: Duration) -> Self {
        Self {
            connect_timeout,
            ..self
        }
    }
}

/// The adapter could not be set up.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("the base URL is not an http(s) URL")]
    BaseUrl,
    #[error("the HTTP client could not be built")]
    Client,
}

/// [`SttEngine`] over a Databricks serving endpoint (or the local model shim).
#[derive(Debug)]
pub struct ServingStt {
    // CONTRACT-STUB
    _config: ServingSttConfig,
}

impl ServingStt {
    pub fn new(config: ServingSttConfig) -> Result<Self, BuildError> {
        // CONTRACT-STUB
        Ok(Self { _config: config })
    }
}

impl SttEngine for ServingStt {
    fn transcribe<'a>(
        &'a self,
        request: SttRequest<'a>,
    ) -> BoxFuture<'a, Result<Transcript, ProviderError>> {
        // CONTRACT-STUB
        let _ = request;
        Box::pin(async { Err(ProviderError::Unavailable) })
    }
}
