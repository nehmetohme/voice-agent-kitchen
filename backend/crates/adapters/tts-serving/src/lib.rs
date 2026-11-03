//! `TtsEngine` adapter that calls a text-to-speech model over the Databricks
//! Model Serving invocations contract (`contracts/serving/tts.json`). The local
//! model shim speaks the same contract, so dev and prod share this code.
//!
//! ```text
//! POST {base_url}/serving-endpoints/{endpoint}/invocations
//! {"dataframe_records":[{"text":"…","voice":"af_heart","speed":1.0,"encoding":"pcm_s16le","sample_rate":24000}]}
//! → {"predictions":[{"audio_b64":"…","encoding":"pcm_s16le","sample_rate":24000}]}
//! ```

use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use reqwest::StatusCode;
use reqwest::header::{AUTHORIZATION, HeaderValue, RETRY_AFTER};
use secrecy::{ExposeSecret as _, SecretString};
use serde_json::{Value, json};
use vak_providers::{BoxFuture, ProviderError, Speech, TtsEngine, TtsRequest};

/// The rate the contract returns: Kokoro's native 24 kHz, which the pipeline plays as is.
const SAMPLE_RATE: u32 = 24_000;
/// The longest clause the contract accepts.
const MAX_TEXT_CHARS: usize = 1_000;

/// Where the TTS endpoint is and how to authenticate.
#[derive(Debug, Clone)]
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
    #[error("the base URL is not an http URL (https arrives with the Databricks deployment)")]
    BaseUrl,
    #[error("the bearer token is not a valid header value")]
    Token,
    #[error("the HTTP client could not be built")]
    Client,
}

/// [`TtsEngine`] over a Databricks serving endpoint (or the local model shim).
#[derive(Debug)]
pub struct ServingTts {
    client: reqwest::Client,
    url: String,
    auth: Option<HeaderValue>,
}

impl ServingTts {
    pub fn new(config: ServingTtsConfig) -> Result<Self, BuildError> {
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

    async fn call(&self, body: Value) -> Result<Speech, ProviderError> {
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
        let [prediction] = answer["predictions"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
        else {
            return Err(protocol("expected one prediction per clause"));
        };
        if prediction["encoding"] != "pcm_s16le" || prediction["sample_rate"] != SAMPLE_RATE {
            return Err(protocol("the audio is not 24 kHz PCM s16le"));
        }
        let bytes = prediction["audio_b64"]
            .as_str()
            .and_then(|audio| BASE64.decode(audio).ok())
            .ok_or_else(|| protocol("the audio is not base64"))?;
        let (pairs, rest) = bytes.as_chunks::<2>();
        if pairs.is_empty() || !rest.is_empty() {
            return Err(protocol("the audio is not whole 16-bit samples"));
        }
        let samples = pairs.iter().map(|pair| i16::from_le_bytes(*pair)).collect();
        Ok(Speech::new(SAMPLE_RATE, samples))
    }
}

impl TtsEngine for ServingTts {
    fn synthesize<'a>(
        &'a self,
        request: TtsRequest<'a>,
    ) -> BoxFuture<'a, Result<Speech, ProviderError>> {
        Box::pin(async move {
            let text = request.text().trim();
            if text.is_empty() {
                return Err(protocol("blank text"));
            }
            if text.chars().count() > MAX_TEXT_CHARS {
                return Err(protocol("the clause is longer than 1000 characters"));
            }
            let started = Instant::now();
            let body = json!({ "dataframe_records": [{
                "text": text,
                "voice": request.voice(),
                "speed": request.speed(),
                "encoding": "pcm_s16le",
                "sample_rate": SAMPLE_RATE,
            }] });
            let result = tokio::time::timeout(request.deadline(), self.call(body))
                .await
                .unwrap_or(Err(ProviderError::Timeout));
            tracing::debug!(
                chars = text.chars().count(),
                elapsed_ms = started.elapsed().as_millis(),
                ok = result.is_ok(),
                "tts invocation"
            );
            result
        })
    }
}

fn protocol(why: &str) -> ProviderError {
    ProviderError::Protocol(why.to_owned())
}

/// A failure to reach the endpoint or to read its answer.
fn transport_error(error: &reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout
    } else if error.is_decode() {
        protocol("the answer is not JSON")
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
        assert!(ServingTts::new(ServingTtsConfig::new("http://127.0.0.1:8001", "tts")).is_ok());
        assert!(matches!(
            ServingTts::new(ServingTtsConfig::new(
                "https://example.cloud.databricks.com",
                "tts"
            )),
            Err(BuildError::BaseUrl)
        ));
        assert!(matches!(
            ServingTts::new(
                ServingTtsConfig::new("http://127.0.0.1:8001", "tts")
                    .with_bearer_token(SecretString::from("bad\ntoken"))
                    .with_connect_timeout(Duration::from_secs(1))
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
        assert_eq!(
            status_error(StatusCode::TOO_MANY_REQUESTS, None),
            ProviderError::RateLimited { retry_after: None }
        );
    }
}
