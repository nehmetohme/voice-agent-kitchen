//! RFC 9457 problem details: every error answers `application/problem+json`.

use std::error::Error;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;
use vak_domain::{PurposeError, PurposeProblem};
use vak_services::{
    ArchiveAgentError, CancelTestRunError, CreateAgentError, DuplicatePurposeError, FindAgentError,
    FindPurposeError, FindTestRunError, ListAgentsError, OpenMediaError, SavePurposeVersionError,
    StartSessionError, StartTestRunError, UpdateAgentPurposeError, UpdateDraftError,
    UpdatePurposeError,
};

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
    /// The field's path in the request body, e.g. `useCase` or `nodes[2].label`.
    pub(crate) field: String,
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
    /// The request is not what the route takes (e.g. not a WebSocket upgrade).
    BadRequest(&'static str),
    /// The caller may not do this from where it asked (e.g. a page on another site).
    Forbidden(&'static str),
    /// The body is not `application/json`.
    UnsupportedMediaType,
    /// No such resource (or an ID that can't name one).
    NotFound,
    /// The body is JSON but breaks the schema or a domain rule.
    Invalid(Vec<FieldError>),
    /// The resource's state forbids the change.
    Conflict(&'static str),
    /// The request ran past its deadline.
    Timeout,
    /// The feature can't serve the request now; nothing failed that needs logging.
    Refused(&'static str),
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
            Self::BadRequest(detail) => (StatusCode::BAD_REQUEST, (*detail).to_owned()),
            Self::Forbidden(detail) => (StatusCode::FORBIDDEN, (*detail).to_owned()),
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
            Self::Timeout => (
                StatusCode::SERVICE_UNAVAILABLE,
                "The request took too long. Try again shortly.".to_owned(),
            ),
            Self::Refused(detail) => (StatusCode::SERVICE_UNAVAILABLE, (*detail).to_owned()),
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

/// A single invalid field.
fn invalid(field: &str, message: &'static str) -> ApiError {
    ApiError::Invalid(vec![FieldError {
        field: field.to_owned(),
        message,
    }])
}

const NO_PURPOSE: &str = "Choose a purpose from the list.";
const NO_VERSION: &str = "This purpose has no saved version yet. Save one on its page first.";
const READ_ONLY: &str = "Built-in purposes can't change. Duplicate it to customize.";

impl From<CreateAgentError> for ApiError {
    fn from(error: CreateAgentError) -> Self {
        match error {
            CreateAgentError::PurposeNotFound => invalid("purposeId", NO_PURPOSE),
            CreateAgentError::PurposeHasNoVersion => invalid("purposeId", NO_VERSION),
            CreateAgentError::UseCaseMismatch { .. } => {
                invalid("useCase", "Use the chosen purpose's compliance class.")
            }
            unavailable @ CreateAgentError::Unavailable(_) => {
                Self::Unavailable(Box::new(unavailable))
            }
        }
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

impl From<ArchiveAgentError> for ApiError {
    fn from(error: ArchiveAgentError) -> Self {
        match error {
            ArchiveAgentError::NotFound => Self::NotFound,
            ArchiveAgentError::Conflict => {
                Self::Conflict("The agent changed while archiving. Try again.")
            }
            unavailable @ ArchiveAgentError::Unavailable(_) => {
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
            UpdateDraftError::PurposeNotFound => invalid("purposeId", NO_PURPOSE),
            UpdateDraftError::PurposeHasNoVersion => invalid("purposeId", NO_VERSION),
            unavailable @ UpdateDraftError::Unavailable(_) => {
                Self::Unavailable(Box::new(unavailable))
            }
        }
    }
}

impl From<StartSessionError> for ApiError {
    fn from(error: StartSessionError) -> Self {
        match error {
            StartSessionError::NotFound => Self::NotFound,
            StartSessionError::Archived => {
                Self::Conflict("This agent is archived, so it can't take calls.")
            }
            StartSessionError::VoiceUnavailable => {
                Self::Refused("Voice sessions aren't set up on this server.")
            }
            StartSessionError::Busy => {
                Self::Refused("Too many test calls are running. Try again shortly.")
            }
            unavailable @ StartSessionError::Unavailable(_) => {
                Self::Unavailable(Box::new(unavailable))
            }
        }
    }
}

impl From<OpenMediaError> for ApiError {
    fn from(error: OpenMediaError) -> Self {
        match error {
            OpenMediaError::NotFound => Self::NotFound,
            OpenMediaError::Busy => {
                Self::Refused("Too many test calls are running. Try again shortly.")
            }
        }
    }
}

impl From<StartTestRunError> for ApiError {
    fn from(error: StartTestRunError) -> Self {
        match error {
            StartTestRunError::NotFound => Self::NotFound,
            StartTestRunError::Archived => {
                Self::Conflict("This agent is archived, so it can't be tested.")
            }
            StartTestRunError::AlreadyRunning => Self::Conflict(
                "A test run of this agent is already queued or running. Wait for it, or cancel it.",
            ),
            StartTestRunError::ModelsUnavailable => {
                Self::Refused("Test runs aren't set up on this server: it has no models for them.")
            }
            unavailable @ StartTestRunError::Unavailable(_) => {
                Self::Unavailable(Box::new(unavailable))
            }
        }
    }
}

impl From<FindPurposeError> for ApiError {
    fn from(error: FindPurposeError) -> Self {
        match error {
            FindPurposeError::NotFound => Self::NotFound,
            unavailable @ FindPurposeError::Unavailable(_) => {
                Self::Unavailable(Box::new(unavailable))
            }
        }
    }
}

impl From<DuplicatePurposeError> for ApiError {
    fn from(error: DuplicatePurposeError) -> Self {
        match error {
            DuplicatePurposeError::SourceNotFound => {
                invalid("duplicateOf", "Choose a purpose to copy.")
            }
            DuplicatePurposeError::VersionNotFound => {
                invalid("version", "Choose a version the purpose has.")
            }
            unavailable @ DuplicatePurposeError::Unavailable(_) => {
                Self::Unavailable(Box::new(unavailable))
            }
        }
    }
}

impl From<UpdatePurposeError> for ApiError {
    fn from(error: UpdatePurposeError) -> Self {
        match error {
            UpdatePurposeError::NotFound => Self::NotFound,
            UpdatePurposeError::ReadOnly => Self::Conflict(READ_ONLY),
            UpdatePurposeError::Invalid(PurposeError::PersonaTooLong { .. }) => {
                invalid("persona", "Use at most 2,000 characters.")
            }
            UpdatePurposeError::Invalid(_) => invalid("description", "Use at most 500 characters."),
            UpdatePurposeError::Conflict => {
                Self::Conflict("The purpose changed while saving. Reload it and try again.")
            }
            unavailable @ UpdatePurposeError::Unavailable(_) => {
                Self::Unavailable(Box::new(unavailable))
            }
        }
    }
}

impl From<SavePurposeVersionError> for ApiError {
    fn from(error: SavePurposeVersionError) -> Self {
        match error {
            SavePurposeVersionError::NotFound => Self::NotFound,
            SavePurposeVersionError::ReadOnly => Self::Conflict(READ_ONLY),
            SavePurposeVersionError::Unchanged { .. } => Self::Conflict(
                "Nothing changed since the latest version, so there is nothing to save.",
            ),
            SavePurposeVersionError::Invalid { problems } => {
                Self::Invalid(problem_fields(&problems))
            }
            SavePurposeVersionError::Conflict => {
                Self::Conflict("The purpose changed while saving. Reload it and try again.")
            }
            unavailable @ SavePurposeVersionError::Unavailable(_) => {
                Self::Unavailable(Box::new(unavailable))
            }
        }
    }
}

impl From<FindTestRunError> for ApiError {
    fn from(error: FindTestRunError) -> Self {
        match error {
            FindTestRunError::NotFound => Self::NotFound,
            unavailable @ FindTestRunError::Unavailable(_) => {
                Self::Unavailable(Box::new(unavailable))
            }
        }
    }
}

impl From<CancelTestRunError> for ApiError {
    fn from(error: CancelTestRunError) -> Self {
        match error {
            CancelTestRunError::NotFound => Self::NotFound,
            CancelTestRunError::Finished => {
                Self::Conflict("This test run is over, so it can't be cancelled.")
            }
            CancelTestRunError::Conflict => {
                Self::Conflict("The test run kept changing while cancelling. Try again.")
            }
            unavailable @ CancelTestRunError::Unavailable(_) => {
                Self::Unavailable(Box::new(unavailable))
            }
        }
    }
}

/// One field error per section with problems: `flow` or `tools`.
fn problem_fields(problems: &[PurposeProblem]) -> Vec<FieldError> {
    let mut errors: Vec<FieldError> = Vec::new();
    for problem in problems {
        let (field, message) = match problem {
            PurposeProblem::Flow(_) => (
                "flow".to_owned(),
                "Fix the starting flow's problems first; each is shown on its step.",
            ),
            PurposeProblem::HostNotAllowed { .. } => (
                "tools".to_owned(),
                "A tool calls a host that isn't on the tool host allow-list.",
            ),
        };
        if !errors.iter().any(|error| error.field == field) {
            errors.push(FieldError { field, message });
        }
    }
    errors
}

impl From<UpdateAgentPurposeError> for ApiError {
    fn from(error: UpdateAgentPurposeError) -> Self {
        match error {
            UpdateAgentPurposeError::NotFound => Self::NotFound,
            UpdateAgentPurposeError::Archived => {
                Self::Conflict("This agent is archived, so it can't be edited.")
            }
            UpdateAgentPurposeError::VersionNotFound => {
                invalid("version", "Choose a version the purpose has.")
            }
            UpdateAgentPurposeError::SameVersion => {
                invalid("version", "The agent already uses this version.")
            }
            UpdateAgentPurposeError::Conflict => {
                Self::Conflict("The agent changed while saving. Reload it and try again.")
            }
            unavailable @ UpdateAgentPurposeError::Unavailable(_) => {
                Self::Unavailable(Box::new(unavailable))
            }
        }
    }
}
