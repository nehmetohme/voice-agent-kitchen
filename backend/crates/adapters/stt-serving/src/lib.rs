//! `SttEngine` adapter that calls a speech-to-text model over the Databricks
//! Model Serving invocations contract (`contracts/serving/stt.json`). The local
//! model shim speaks the same contract, so dev and prod share this code.
//!
//! ```text
//! POST {base_url}/serving-endpoints/{endpoint}/invocations
//! {"dataframe_records":[{"audio_b64":"…","encoding":"pcm_s16le","sample_rate":16000,"language":"en"}]}
//! → {"predictions":[{"text":"…"}]}
//! ```

use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use reqwest::StatusCode;
use reqwest::header::{AUTHORIZATION, HeaderValue, RETRY_AFTER};
use secrecy::{ExposeSecret as _, SecretString};
use serde_json::{Value, json};
use vak_providers::{BoxFuture, ProviderError, SttEngine, SttRequest, Transcript};

/// Where the STT endpoint is and how to authenticate.
#[derive(Debug, Clone)]
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
    #[error("the base URL is not an http URL (https arrives with the Databricks deployment)")]
    BaseUrl,
    #[error("the bearer token is not a valid header value")]
    Token,
    #[error("the HTTP client could not be built")]
    Client,
}

/// [`SttEngine`] over a Databricks serving endpoint (or the local model shim).
#[derive(Debug)]
pub struct ServingStt {
    client: reqwest::Client,
    url: String,
    auth: Option<HeaderValue>,
}

impl ServingStt {
    pub fn new(config: ServingSttConfig) -> Result<Self, BuildError> {
        let base = config.base_url.trim_end_matches('/');
        if !base.starts_with("http://") {
            return Err(BuildError::BaseUrl);
        }
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
            url: format!(
                "{base}/serving-endpoints/{}/invocations",
                config.endpoint.trim_matches('/')
            ),
            auth,
        })
    }

    async fn call(&self, body: Value) -> Result<Transcript, ProviderError> {
        let mut request = self.client.post(&self.url).json(&body);
        if let Some(auth) = &self.auth {
            request = request.header(AUTHORIZATION, auth.clone());
        }
        let response = request
            .send()
            .await
            .map_err(|error| transport_error(&error))?;
        let status = response.status();
        if !status.is_success() {
            return Err(status_error(status, response.headers().get(RETRY_AFTER)));
        }
        let answer: Value = response
            .json()
            .await
            .map_err(|error| transport_error(&error))?;
        match answer["predictions"].as_array().map(Vec::as_slice) {
            Some([prediction]) => prediction["text"]
                .as_str()
                .map(|text| Transcript::new(text.trim()))
                .ok_or_else(|| ProviderError::Protocol("a prediction without text".to_owned())),
            _ => Err(ProviderError::Protocol(
                "expected one prediction per utterance".to_owned(),
            )),
        }
    }
}

impl SttEngine for ServingStt {
    fn transcribe<'a>(
        &'a self,
        request: SttRequest<'a>,
    ) -> BoxFuture<'a, Result<Transcript, ProviderError>> {
        Box::pin(async move {
            let started = Instant::now();
            let pcm: Vec<u8> = request
                .audio()
                .iter()
                .flat_map(|sample| sample.to_le_bytes())
                .collect();
            let mut record = json!({
                "audio_b64": BASE64.encode(pcm),
                "encoding": "pcm_s16le",
                "sample_rate": 16_000,
            });
            if let Some(language) = request.language() {
                record["language"] = json!(language);
            }
            let body = json!({ "dataframe_records": [record] });
            let result = tokio::time::timeout(request.deadline(), self.call(body))
                .await
                .unwrap_or(Err(ProviderError::Timeout));
            tracing::debug!(
                samples = request.audio().len(),
                elapsed_ms = started.elapsed().as_millis(),
                ok = result.is_ok(),
                "stt invocation"
            );
            result
        })
    }
}

/// A failure to reach the endpoint or to read its answer.
fn transport_error(error: &reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout
    } else if error.is_decode() {
        ProviderError::Protocol("the answer is not JSON".to_owned())
    } else {
        ProviderError::Unavailable
    }
}

/// The Databricks serving error statuses (`contracts/serving/README.md`).
fn status_error(status: StatusCode, retry_after: Option<&HeaderValue>) -> ProviderError {
    match status.as_u16() {
        401 | 403 => ProviderError::Auth,
        429 => ProviderError::RateLimited {
            retry_after: retry_after
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse().ok())
                .map(Duration::from_secs),
        },
        503 | 502 => ProviderError::Unavailable,
        504 => ProviderError::Timeout,
        // A model that failed (500) or a request the endpoint refused: retrying won't help.
        _ => ProviderError::Protocol(format!("the endpoint answered {status}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_http_urls_build() {
        assert!(ServingStt::new(ServingSttConfig::new("http://127.0.0.1:8001/", "stt")).is_ok());
        assert!(matches!(
            ServingStt::new(ServingSttConfig::new(
                "https://example.cloud.databricks.com",
                "stt"
            )),
            Err(BuildError::BaseUrl)
        ));
        assert!(matches!(
            ServingStt::new(
                ServingSttConfig::new("http://127.0.0.1:8001", "stt")
                    .with_bearer_token(SecretString::from("bad\ntoken"))
            ),
            Err(BuildError::Token)
        ));
    }

    #[test]
    fn statuses_map_to_provider_errors() {
        assert_eq!(
            status_error(StatusCode::BAD_GATEWAY, None),
            ProviderError::Unavailable
        );
        let after = HeaderValue::from_static("3");
        assert_eq!(
            status_error(StatusCode::TOO_MANY_REQUESTS, Some(&after)),
            ProviderError::RateLimited {
                retry_after: Some(Duration::from_secs(3))
            }
        );
    }
}
