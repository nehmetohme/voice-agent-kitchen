//! RFC 9457 problem details: every error answers `application/problem+json`.

use std::error::Error;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;
use vak_services::{CreateAgentError, FindAgentError, ListAgentsError, UpdateDraftError};

/// The media type of every error response.
pub(crate) const PROBLEM_JSON: &str = "application/problem+json";

/// An RFC 9457 problem. `type` is always `about:blank`, so `title` is the HTTP status phrase;
/// `detail` says what went wrong and how to fix it.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct Problem {
    /// Always `about:blank`.
    #[serde(rename = "type")]
    kind: &'static str,
    /// The HTTP status phrase, e.g. `Unprocessable Content`.
    title: &'static str,
    /// The HTTP status code.
    status: u16,
    /// What went wrong, for people.
    detail: String,
    /// One entry per invalid field (validation errors only).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    errors: Vec<FieldError>,
}

/// An invalid request field and how to fix it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct FieldError {
    /// The field's name in the request body, e.g. `useCase`.
    pub(crate) field: &'static str,
    /// What to change, e.g. `Enter a name.`
    pub(crate) message: &'static str,
}

/// Every way a request can fail. Messages never carry internal error text or request data.
#[derive(Debug)]
pub(crate) enum ApiError {
    /// No identity, or a malformed one.
    Unauthenticated,
    /// The body is not JSON.
    MalformedJson,
    /// The body is not `application/json`.
    UnsupportedMediaType,
    /// No such resource (or an ID that can't name one).
    NotFound,
    /// The body is JSON but breaks the schema or a domain rule.
    Invalid(Vec<FieldError>),
    /// The resource's state forbids the change.
    Conflict(&'static str),
    /// A dependency failed; the source is logged here, once, and never sent.
    Unavailable(Box<dyn Error + Send + Sync>),
}

impl ApiError {
    fn status_and_detail(&self) -> (StatusCode, String) {
        match self {
            Self::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "Sign in through Databricks to use the API.".to_owned(),
            ),
            Self::MalformedJson => (
                StatusCode::BAD_REQUEST,
                "The request body is not valid JSON.".to_owned(),
            ),
            Self::UnsupportedMediaType => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Send the request body as application/json.".to_owned(),
            ),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "Nothing exists at this address.".to_owned(),
            ),
            Self::Invalid(errors) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                match errors.len() {
                    0 => "The request body doesn't match the schema.".to_owned(),
                    1 => "1 field needs a change.".to_owned(),
                    n => format!("{n} fields need a change."),
                },
            ),
            Self::Conflict(detail) => (StatusCode::CONFLICT, (*detail).to_owned()),
            Self::Unavailable(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "The service is temporarily unavailable. Try again shortly.".to_owned(),
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, detail) = self.status_and_detail();
        let errors = match self {
            Self::Invalid(errors) => errors,
            Self::Unavailable(source) => {
                tracing::error!(error = %chain(source.as_ref()), "request failed");
                Vec::new()
            }
            _ => Vec::new(),
        };
        let problem = Problem {
            kind: "about:blank",
            title: status.canonical_reason().unwrap_or("Error"),
            status: status.as_u16(),
            detail,
            errors,
        };
        let mut response = (status, Json(problem)).into_response();
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(PROBLEM_JSON));
        response
    }
}

/// An error and its sources, outermost first: `storage unavailable: connection refused`.
fn chain(error: &(dyn Error + 'static)) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        match rejection {
            JsonRejection::JsonDataError(_) => Self::Invalid(Vec::new()),
            JsonRejection::MissingJsonContentType(_) => Self::UnsupportedMediaType,
            _ => Self::MalformedJson,
        }
    }
}

impl From<CreateAgentError> for ApiError {
    fn from(error: CreateAgentError) -> Self {
        Self::Unavailable(Box::new(error))
    }
}

impl From<ListAgentsError> for ApiError {
    fn from(error: ListAgentsError) -> Self {
        Self::Unavailable(Box::new(error))
    }
}

impl From<FindAgentError> for ApiError {
    fn from(error: FindAgentError) -> Self {
        match error {
            FindAgentError::NotFound => Self::NotFound,
            unavailable @ FindAgentError::Unavailable(_) => {
                Self::Unavailable(Box::new(unavailable))
            }
        }
    }
}

impl From<UpdateDraftError> for ApiError {
    fn from(error: UpdateDraftError) -> Self {
        match error {
            UpdateDraftError::NotFound => Self::NotFound,
            UpdateDraftError::Archived => {
                Self::Conflict("This agent is archived, so it can't be edited.")
            }
            UpdateDraftError::Conflict => {
                Self::Conflict("The agent changed while saving. Reload it and try again.")
            }
            unavailable @ UpdateDraftError::Unavailable(_) => {
                Self::Unavailable(Box::new(unavailable))
            }
        }
    }
}
