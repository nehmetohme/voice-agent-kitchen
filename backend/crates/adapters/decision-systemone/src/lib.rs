//! `DecisionEngine` adapter for the System One API: typed, fast decisions
//! (choice, yes/no, score) from `openjev-qwen35-4b` behind Databricks Unity
//! Gateway in production and the local llama-server in development.
//!
//! ```text
//! POST {base_url}{path}   path: /v1/systemone (llama-server), /ai-gateway/typesafe/v1/systemone (Databricks)
//! {"model":…,"state":{"turns":[{"speaker":"caller","text":"…"}],"plan_node":…},
//!  "questions":{"opt_out":{"type":"noul","instructions":"…"},
//!               "intent":{"type":"choice","instructions":"…","criteria":{"book":"…","other":null}}}}
//! → {"model":…,"answers":{"opt_out":{"type":"noul","noul":0.87},
//!                         "intent":{"type":"choice","choice":"book","confidence":0.9,"probabilities":{…}}}}
//! ```

use std::time::Duration;

use secrecy::SecretString;
use vak_providers::{BoxFuture, DecisionEngine, DecisionRequest, Decisions, ProviderError};

/// Where the System One API is, which model to ask, and how.
#[derive(Debug, Clone)]
#[expect(dead_code, reason = "CONTRACT-STUB: the engine reads them")]
pub struct SystemOneConfig {
    base_url: String,
    path: String,
    model: String,
    token: Option<SecretString>,
    connect_timeout: Duration,
}

impl SystemOneConfig {
    /// llama-server's path (`/v1/systemone`) under `base_url`, no authentication;
    /// connections time out after 2 s.
    #[must_use]
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            path: "/v1/systemone".to_owned(),
            model: model.into(),
            token: None,
            connect_timeout: Duration::from_secs(2),
        }
    }

    /// Another path under the base URL, e.g. Databricks'
    /// `/ai-gateway/typesafe/v1/systemone`.
    #[must_use]
    pub fn with_path(self, path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            ..self
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

/// [`DecisionEngine`] over the System One API.
#[derive(Debug)]
pub struct SystemOneDecisions {
    // CONTRACT-STUB
    _config: SystemOneConfig,
}

impl SystemOneDecisions {
    pub fn new(config: SystemOneConfig) -> Result<Self, BuildError> {
        // CONTRACT-STUB
        Ok(Self { _config: config })
    }
}

impl DecisionEngine for SystemOneDecisions {
    fn decide<'a>(
        &'a self,
        request: &'a DecisionRequest,
    ) -> BoxFuture<'a, Result<Decisions, ProviderError>> {
        // CONTRACT-STUB
        let _ = request;
        Box::pin(async { Err(ProviderError::Unavailable) })
    }
}
