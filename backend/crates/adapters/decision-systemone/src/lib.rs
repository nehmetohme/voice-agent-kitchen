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
//!
//! A timeout is a `ProviderError::Timeout`, never a "no": the pipeline must not read a slow
//! decision model as a caller who didn't opt out.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use reqwest::StatusCode;
use reqwest::header::{AUTHORIZATION, HeaderValue, RETRY_AFTER};
use secrecy::{ExposeSecret as _, SecretString};
use serde_json::{Map, Value, json};
use vak_domain::{Answer, Question, QuestionKind};
use vak_providers::{BoxFuture, DecisionEngine, DecisionRequest, Decisions, ProviderError};

/// Where the System One API is, which model to ask, and how.
#[derive(Debug, Clone)]
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
    #[error("the base URL is not an http URL (https arrives with the Databricks deployment)")]
    BaseUrl,
    #[error("the bearer token is not a valid header value")]
    Token,
    #[error("the HTTP client could not be built")]
    Client,
}

/// [`DecisionEngine`] over the System One API.
#[derive(Debug)]
pub struct SystemOneDecisions {
    client: reqwest::Client,
    url: String,
    model: String,
    auth: Option<HeaderValue>,
}

impl SystemOneDecisions {
    pub fn new(config: SystemOneConfig) -> Result<Self, BuildError> {
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
            url: format!("{base}/{}", config.path.trim_start_matches('/')),
            model: config.model,
            auth,
        })
    }

    async fn call(&self, request: &DecisionRequest) -> Result<Decisions, ProviderError> {
        let body = self.body(request)?;
        let mut http = self.client.post(&self.url).json(&body);
        if let Some(auth) = &self.auth {
            http = http.header(AUTHORIZATION, auth.clone());
        }
        let response = http.send().await.map_err(|error| transport_error(&error))?;
        let status = response.status();
        if !status.is_success() {
            return Err(status_error(status, response.headers().get(RETRY_AFTER)));
        }
        let answer: Value = response
            .json()
            .await
            .map_err(|error| transport_error(&error))?;
        let model = answer["model"].as_str().unwrap_or(&self.model).to_owned();
        let answers = request
            .questions()
            .iter()
            .map(|(id, question)| {
                let found = &answer["answers"][id.as_str()];
                let parsed = parse_answer(found, question)
                    .ok_or_else(|| protocol("an answer is missing or does not fit its question"))?;
                Ok((id.clone(), parsed))
            })
            .collect::<Result<Vec<_>, ProviderError>>()?;
        Ok(Decisions::new(model, answers))
    }

    /// The request body; requests the API would refuse fail here, before any traffic.
    fn body(&self, request: &DecisionRequest) -> Result<Value, ProviderError> {
        if request.questions().is_empty() {
            return Err(protocol("a decision request needs at least one question"));
        }
        let ids: BTreeSet<&str> = request
            .questions()
            .iter()
            .map(|(id, _)| id.as_str())
            .collect();
        if ids.len() != request.questions().len() {
            return Err(protocol("question IDs must be unique"));
        }
        let turns: Vec<Value> = request
            .turns()
            .iter()
            .map(|(speaker, text)| json!({"speaker": speaker.as_str(), "text": text.as_str()}))
            .collect();
        let mut state = Map::new();
        state.insert("turns".to_owned(), Value::Array(turns));
        if let Some(node) = request.plan_node() {
            state.insert("plan_node".to_owned(), json!(node));
        }
        let questions: Map<String, Value> = request
            .questions()
            .iter()
            .map(|(id, question)| (id.as_str().to_owned(), question_json(question)))
            .collect();
        Ok(json!({"model": self.model, "state": state, "questions": questions}))
    }
}

impl DecisionEngine for SystemOneDecisions {
    fn decide<'a>(
        &'a self,
        request: &'a DecisionRequest,
    ) -> BoxFuture<'a, Result<Decisions, ProviderError>> {
        Box::pin(async move {
            let started = Instant::now();
            let result = tokio::time::timeout(request.deadline(), self.call(request))
                .await
                .unwrap_or(Err(ProviderError::Timeout));
            tracing::debug!(
                questions = request.questions().len(),
                elapsed_ms = started.elapsed().as_millis(),
                ok = result.is_ok(),
                "systemone decision"
            );
            result
        })
    }
}

fn question_json(question: &Question) -> Value {
    match question.kind() {
        QuestionKind::Choice(options) => {
            let criteria: Map<String, Value> = options
                .iter()
                .map(|option| {
                    let description = option.description().map_or(Value::Null, |d| json!(d));
                    (option.label().to_owned(), description)
                })
                .collect();
            json!({"type": "choice", "instructions": question.instructions(), "criteria": criteria})
        }
        _ => json!({"type": "noul", "instructions": question.instructions()}),
    }
}

/// An answer of the type its question asked for, in range; `None` otherwise.
fn parse_answer(found: &Value, question: &Question) -> Option<Answer> {
    let answer = match found["type"].as_str()? {
        "noul" => Answer::YesNo {
            // `ai_decide` calls it `probability`.
            probability: found["noul"]
                .as_f64()
                .or_else(|| found["probability"].as_f64())?,
        },
        "choice" => {
            let probabilities: Vec<(String, f64)> = found["probabilities"]
                .as_object()?
                .iter()
                .map(|(label, p)| Some((label.clone(), p.as_f64()?)))
                .collect::<Option<_>>()?;
            let label = found["choice"].as_str()?.to_owned();
            let confidence = found["confidence"].as_f64().or_else(|| {
                probabilities
                    .iter()
                    .find(|(option, _)| *option == label)
                    .map(|(_, p)| *p)
            })?;
            Answer::Choice {
                label,
                confidence,
                probabilities,
            }
        }
        _ => return None,
    };
    answer.fits(question).then_some(answer)
}

fn protocol(why: &str) -> ProviderError {
    ProviderError::Protocol(why.to_owned())
}

/// A failure to reach the server or to read its answer.
fn transport_error(error: &reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout
    } else if error.is_decode() {
        protocol("the answer is not JSON")
    } else {
        ProviderError::Unavailable
    }
}

fn status_error(status: StatusCode, retry_after: Option<&HeaderValue>) -> ProviderError {
    match status.as_u16() {
        401 | 403 => ProviderError::Auth,
        // 529: the System One API is overloaded.
        429 | 529 => ProviderError::RateLimited {
            retry_after: retry_after
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse().ok())
                .map(Duration::from_secs),
        },
        504 => ProviderError::Timeout,
        500..=599 => ProviderError::Unavailable,
        _ => ProviderError::Protocol(format!("the server answered {status}")),
    }
}

#[cfg(test)]
mod tests {
    use vak_domain::ChoiceOption;

    use super::*;

    #[test]
    fn only_plain_http_urls_build() {
        assert!(
            SystemOneDecisions::new(SystemOneConfig::new("http://127.0.0.1:8081", "m")).is_ok()
        );
        assert!(matches!(
            SystemOneDecisions::new(SystemOneConfig::new("https://host", "m")),
            Err(BuildError::BaseUrl)
        ));
        assert!(matches!(
            SystemOneDecisions::new(
                SystemOneConfig::new("http://127.0.0.1:8081", "m")
                    .with_bearer_token(SecretString::from("bad\ntoken"))
                    .with_connect_timeout(Duration::from_secs(1))
            ),
            Err(BuildError::Token)
        ));
    }

    #[test]
    fn a_choice_without_confidence_takes_its_pick_s_probability() {
        let question =
            Question::choice("Which?", [ChoiceOption::new("a"), ChoiceOption::new("b")]).unwrap();
        let answer = parse_answer(
            &json!({"type": "choice", "choice": "b", "probabilities": {"a": 0.3, "b": 0.7}}),
            &question,
        );
        assert!(
            matches!(answer, Some(Answer::Choice { confidence, .. }) if (confidence - 0.7).abs() < 1e-9)
        );
        assert!(
            parse_answer(
                &json!({"type": "choice", "choice": "b", "probabilities": {"a": "x"}}),
                &question
            )
            .is_none()
        );
    }

    #[test]
    fn statuses_map_to_provider_errors() {
        assert_eq!(
            status_error(StatusCode::GATEWAY_TIMEOUT, None),
            ProviderError::Timeout
        );
        assert_eq!(
            status_error(StatusCode::INTERNAL_SERVER_ERROR, None),
            ProviderError::Unavailable
        );
        assert_eq!(
            status_error(
                StatusCode::TOO_MANY_REQUESTS,
                Some(&HeaderValue::from_static("1"))
            ),
            ProviderError::RateLimited {
                retry_after: Some(Duration::from_secs(1))
            }
        );
    }
}
