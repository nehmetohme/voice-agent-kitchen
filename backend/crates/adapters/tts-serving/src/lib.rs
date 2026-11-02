//! `TtsEngine` adapter that calls a text-to-speech model over the Databricks
//! Model Serving invocations contract (`contracts/serving/tts.json`). The local
//! model shim speaks the same contract, so dev and prod share this code.
//!
//! ```text
//! POST {base_url}/serving-endpoints/{endpoint}/invocations
//! {"dataframe_records":[{"text":"…","voice":"af_heart","speed":1.0,"encoding":"pcm_s16le","sample_rate":24000}]}
//! → {"predictions":[{"audio_b64":"…","encoding":"pcm_s16le","sample_rate":24000}]}
//! ```

use std::time::Duration;

use secrecy::SecretString;
use vak_providers::{BoxFuture, ProviderError, Speech, TtsEngine, TtsRequest};

/// Where the TTS endpoint is and how to authenticate.
#[derive(Debug, Clone)]
#[expect(dead_code, reason = "CONTRACT-STUB: the engine reads them")]
pub struct ServingTtsConfig {
    base_url: String,
    endpoint: String,
    token: Option<SecretString>,
    connect_timeout: Duration,
}

impl ServingTtsConfig {
    /// `base_url` is the workspace (or local shim) root, `endpoint` the serving endpoint's
    /// name, e.g. `tts`. No authentication; connections time out after 2 s.
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

/// [`TtsEngine`] over a Databricks serving endpoint (or the local model shim).
#[derive(Debug)]
pub struct ServingTts {
    // CONTRACT-STUB
    _config: ServingTtsConfig,
}

impl ServingTts {
    pub fn new(config: ServingTtsConfig) -> Result<Self, BuildError> {
        // CONTRACT-STUB
        Ok(Self { _config: config })
    }
}

impl TtsEngine for ServingTts {
    fn synthesize<'a>(
        &'a self,
        request: TtsRequest<'a>,
    ) -> BoxFuture<'a, Result<Speech, ProviderError>> {
        // CONTRACT-STUB
        let _ = request;
        Box::pin(async { Err(ProviderError::Unavailable) })
    }
}
