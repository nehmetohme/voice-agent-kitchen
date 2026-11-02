//! `LlmEngine` adapter for OpenAI-compatible streaming chat completions with
//! tools. One code path serves Databricks Foundation Model APIs in production and
//! the local llama-server in development; only base URL and auth differ.
//!
//! ```text
//! POST {base_url}/chat/completions   {"model":…,"stream":true,"messages":[…],"tools":[…], …extra body}
//! → text/event-stream: data: {"choices":[{"delta":{"content":"…"}}]} … data: [DONE]
//! ```

use std::time::Duration;

use secrecy::SecretString;
use vak_providers::{BoxFuture, LlmEngine, LlmRequest, LlmStream, ProviderError};

/// Where the chat-completions API is, which model to ask, and how.
#[derive(Debug, Clone)]
#[expect(dead_code, reason = "CONTRACT-STUB: the engine reads them")]
pub struct OpenAiLlmConfig {
    base_url: String,
    model: String,
    token: Option<SecretString>,
    extra_body: serde_json::Value,
    idle_timeout: Duration,
    connect_timeout: Duration,
}

impl OpenAiLlmConfig {
    /// `base_url` ends before `/chat/completions` (e.g. `http://host:8080/v1`). No
    /// authentication and no extra body; a stream that sends nothing for 5 s times out,
    /// and connections time out after 2 s.
    #[must_use]
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            token: None,
            extra_body: serde_json::Value::Object(serde_json::Map::new()),
            idle_timeout: Duration::from_secs(5),
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

    /// A JSON object merged into every request body, e.g.
    /// `{"chat_template_kwargs":{"enable_thinking":false}}`. Anything but an object is a
    /// [`BuildError`].
    #[must_use]
    pub fn with_extra_body(self, extra_body: serde_json::Value) -> Self {
        Self { extra_body, ..self }
    }

    /// How long a started stream may stay silent before it fails with
    /// [`ProviderError::Timeout`].
    #[must_use]
    pub fn with_idle_timeout(self, idle_timeout: Duration) -> Self {
        Self {
            idle_timeout,
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
    #[error("the extra body is not a JSON object")]
    ExtraBody,
    #[error("the HTTP client could not be built")]
    Client,
}

/// [`LlmEngine`] over an OpenAI-compatible chat-completions API.
#[derive(Debug)]
pub struct OpenAiLlm {
    // CONTRACT-STUB
    _config: OpenAiLlmConfig,
}

impl OpenAiLlm {
    pub fn new(config: OpenAiLlmConfig) -> Result<Self, BuildError> {
        // CONTRACT-STUB
        Ok(Self { _config: config })
    }
}

impl LlmEngine for OpenAiLlm {
    fn stream(
        &self,
        request: LlmRequest,
    ) -> BoxFuture<'_, Result<Box<dyn LlmStream>, ProviderError>> {
        // CONTRACT-STUB
        drop(request);
        Box::pin(async { Err(ProviderError::Unavailable) })
    }
}
