//! `LlmEngine` adapter for OpenAI-compatible streaming chat completions with
//! tools. One code path serves Databricks Foundation Model APIs in production and
//! the local llama-server in development; only base URL and auth differ.
//!
//! ```text
//! POST {base_url}/chat/completions   {"model":…,"stream":true,"messages":[…],"tools":[…], …extra body}
//! → text/event-stream: data: {"choices":[{"delta":{"content":"…"}}]} … data: [DONE]
//! ```
//!
//! Every content delta becomes a `Text` event the moment it arrives; tool-call fragments
//! are put together and come out as one `ToolCall` when the model finishes. The request has
//! until its deadline for the first content; after that, a stream silent for the idle
//! timeout fails with `Timeout`. Dropping the stream drops the connection, which tells the
//! server to stop generating.

mod sse;

use std::time::Duration;

use reqwest::StatusCode;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderValue, RETRY_AFTER};
use secrecy::{ExposeSecret as _, SecretString};
use serde_json::{Map, Value, json};
use tokio::time::Instant;
use vak_providers::{BoxFuture, LlmEngine, LlmRequest, LlmStream, ProviderError};

use crate::sse::ChatStream;

/// Where the chat-completions API is, which model to ask, and how.
#[derive(Debug, Clone)]
pub struct OpenAiLlmConfig {
    base_url: String,
    model: String,
    token: Option<SecretString>,
    extra_body: Value,
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
            extra_body: Value::Object(Map::new()),
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
    pub fn with_extra_body(self, extra_body: Value) -> Self {
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
    #[error("the base URL is not an http URL (https arrives with the Databricks deployment)")]
    BaseUrl,
    #[error("the extra body is not a JSON object")]
    ExtraBody,
    #[error("the bearer token is not a valid header value")]
    Token,
    #[error("the HTTP client could not be built")]
    Client,
}

/// [`LlmEngine`] over an OpenAI-compatible chat-completions API.
#[derive(Debug)]
pub struct OpenAiLlm {
    client: reqwest::Client,
    url: String,
    model: String,
    auth: Option<HeaderValue>,
    extra_body: Map<String, Value>,
    idle_timeout: Duration,
}

impl OpenAiLlm {
    pub fn new(config: OpenAiLlmConfig) -> Result<Self, BuildError> {
        let base = config.base_url.trim_end_matches('/');
        if !base.starts_with("http://") {
            return Err(BuildError::BaseUrl);
        }
        let Value::Object(extra_body) = config.extra_body else {
            return Err(BuildError::ExtraBody);
        };
        let auth = config
            .token
            .map(|token| {
                let mut value = HeaderValue::from_str(&format!("Bearer {}", token.expose_secret()))
                    .map_err(|_| BuildError::Token)?;
                value.set_sensitive(true);
                Ok(value)
            })
            .transpose()?;
        let client = reqwest::Client::builder()
            .connect_timeout(config.connect_timeout)
            .build()
            .map_err(|_| BuildError::Client)?;
        Ok(Self {
            client,
            url: format!("{base}/chat/completions"),
            model: config.model,
            auth,
            extra_body,
            idle_timeout: config.idle_timeout,
        })
    }

    /// The request body: the extra body, then the request's own fields over it.
    fn body(&self, request: &LlmRequest) -> Result<Value, ProviderError> {
        let mut body = self.extra_body.clone();
        body.insert("model".to_owned(), json!(self.model));
        body.insert("stream".to_owned(), json!(true));
        let messages: Vec<Value> = request
            .messages()
            .iter()
            .map(|message| json!({"role": message.role().as_str(), "content": message.content()}))
            .collect();
        body.insert("messages".to_owned(), Value::Array(messages));
        if let Some(max_tokens) = request.max_tokens() {
            body.insert("max_tokens".to_owned(), json!(max_tokens));
        }
        if !request.tools().is_empty() {
            let tools = request
                .tools()
                .iter()
                .map(|tool| {
                    let parameters: Value = serde_json::from_str(tool.parameters_json())
                        .map_err(|_| protocol("a tool's parameters are not JSON"))?;
                    Ok(json!({"type": "function", "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": parameters,
                    }}))
                })
                .collect::<Result<Vec<Value>, ProviderError>>()?;
            body.insert("tools".to_owned(), Value::Array(tools));
        }
        Ok(Value::Object(body))
    }

    async fn open(&self, request: LlmRequest) -> Result<Box<dyn LlmStream>, ProviderError> {
        let first_content_by = Instant::now() + request.deadline();
        let body = self.body(&request)?;
        let mut http = self
            .client
            .post(&self.url)
            .header(ACCEPT, "text/event-stream")
            .json(&body);
        if let Some(auth) = &self.auth {
            http = http.header(AUTHORIZATION, auth.clone());
        }
        let response = tokio::time::timeout_at(first_content_by, http.send())
            .await
            .map_err(|_| ProviderError::Timeout)?
            .map_err(|error| transport_error(&error))?;
        let status = response.status();
        if !status.is_success() {
            tracing::debug!(%status, "chat completion refused");
            return Err(status_error(status, response.headers().get(RETRY_AFTER)));
        }
        Ok(Box::new(ChatStream::new(
            response,
            first_content_by,
            self.idle_timeout,
        )))
    }
}

impl LlmEngine for OpenAiLlm {
    fn stream(
        &self,
        request: LlmRequest,
    ) -> BoxFuture<'_, Result<Box<dyn LlmStream>, ProviderError>> {
        Box::pin(self.open(request))
    }
}

fn protocol(why: &str) -> ProviderError {
    ProviderError::Protocol(why.to_owned())
}

/// A failure to reach the server or to read its answer.
fn transport_error(error: &reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout
    } else {
        ProviderError::Unavailable
    }
}

fn status_error(status: StatusCode, retry_after: Option<&HeaderValue>) -> ProviderError {
    match status.as_u16() {
        401 | 403 => ProviderError::Auth,
        429 => ProviderError::RateLimited {
            retry_after: retry_after
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse().ok())
                .map(Duration::from_secs),
        },
        500..=599 => ProviderError::Unavailable,
        _ => ProviderError::Protocol(format!("the server answered {status}")),
    }
}

#[cfg(test)]
mod tests {
    use vak_providers::{Role, ToolSpec};

    use super::*;

    #[test]
    fn only_plain_http_urls_and_object_extra_bodies_build() {
        assert!(OpenAiLlm::new(OpenAiLlmConfig::new("http://127.0.0.1:8080/v1/", "m")).is_ok());
        assert!(matches!(
            OpenAiLlm::new(OpenAiLlmConfig::new("https://host/serving-endpoints", "m")),
            Err(BuildError::BaseUrl)
        ));
        assert!(matches!(
            OpenAiLlm::new(
                OpenAiLlmConfig::new("http://127.0.0.1:8080/v1", "m")
                    .with_bearer_token(SecretString::from("bad\nkey"))
                    .with_connect_timeout(Duration::from_secs(1))
            ),
            Err(BuildError::Token)
        ));
    }

    #[test]
    fn the_request_fields_win_over_the_extra_body() {
        let llm = OpenAiLlm::new(
            OpenAiLlmConfig::new("http://127.0.0.1:8080/v1", "real")
                .with_extra_body(json!({"model": "extra", "top_k": 20})),
        )
        .unwrap();
        let body = llm
            .body(&LlmRequest::new(Duration::from_secs(1)).with_message(Role::User, "Hi"))
            .unwrap();
        assert_eq!(body["model"], "real");
        assert_eq!(body["top_k"], 20);
        assert!(body.get("tools").is_none() && body.get("max_tokens").is_none());
        let broken = LlmRequest::new(Duration::from_secs(1))
            .with_tool(ToolSpec::new("t", "d").with_parameters_json("{not json"));
        assert!(matches!(llm.body(&broken), Err(ProviderError::Protocol(_))));
    }
}
