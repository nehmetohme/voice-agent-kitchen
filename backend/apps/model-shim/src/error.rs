//! Databricks-shaped errors: a status and `{"error_code": "...", "message": "..."}`
//! (`$defs/error` in contracts/serving/*.json). Messages never contain text or audio.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// The `error_code`s the shim answers with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ErrorCode {
    BadRequest,
    ResourceDoesNotExist,
    InternalError,
    TemporarilyUnavailable,
    DeadlineExceeded,
}

/// A failed invocation.
#[derive(Debug)]
pub(crate) struct ServingError {
    status: StatusCode,
    code: ErrorCode,
    message: String,
}

impl ServingError {
    fn new(status: StatusCode, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    /// 400: the request is not valid for the contract.
    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, ErrorCode::BadRequest, message)
    }

    /// 413: the body is over the size limit.
    pub(crate) fn too_large(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::BadRequest,
            message,
        )
    }

    /// 404: no such serving endpoint or route.
    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            ErrorCode::ResourceDoesNotExist,
            message,
        )
    }

    /// 500: the model failed or answered something unusable.
    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::InternalError,
            message,
        )
    }

    /// 503: the model server can't be reached.
    pub(crate) fn unavailable(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::TemporarilyUnavailable,
            message,
        )
    }

    /// 504: the model server didn't answer in time.
    pub(crate) fn deadline_exceeded(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::GATEWAY_TIMEOUT,
            ErrorCode::DeadlineExceeded,
            message,
        )
    }

    pub(crate) fn status(&self) -> StatusCode {
        self.status
    }

    pub(crate) fn code(&self) -> ErrorCode {
        self.code
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error_code: ErrorCode,
    message: String,
}

impl IntoResponse for ServingError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error_code: self.code,
            message: self.message,
        };
        (self.status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_serialize_as_databricks_error_codes() {
        let codes = [
            (ErrorCode::BadRequest, "BAD_REQUEST"),
            (ErrorCode::ResourceDoesNotExist, "RESOURCE_DOES_NOT_EXIST"),
            (ErrorCode::InternalError, "INTERNAL_ERROR"),
            (ErrorCode::TemporarilyUnavailable, "TEMPORARILY_UNAVAILABLE"),
            (ErrorCode::DeadlineExceeded, "DEADLINE_EXCEEDED"),
        ];
        for (code, name) in codes {
            assert_eq!(serde_json::to_value(code).expect("serializes"), name);
        }
    }

    #[test]
    fn each_constructor_pairs_a_status_with_a_code() {
        let cases = [
            (ServingError::bad_request("x"), 400, ErrorCode::BadRequest),
            (ServingError::too_large("x"), 413, ErrorCode::BadRequest),
            (
                ServingError::not_found("x"),
                404,
                ErrorCode::ResourceDoesNotExist,
            ),
            (ServingError::internal("x"), 500, ErrorCode::InternalError),
            (
                ServingError::unavailable("x"),
                503,
                ErrorCode::TemporarilyUnavailable,
            ),
            (
                ServingError::deadline_exceeded("x"),
                504,
                ErrorCode::DeadlineExceeded,
            ),
        ];
        for (error, status, code) in cases {
            assert_eq!((error.status().as_u16(), error.code()), (status, code));
        }
    }
}
