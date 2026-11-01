//! A deadline for every `/api/v1` request: a handler that runs past it is dropped and the
//! caller gets a 503 problem instead of a hanging connection.
//!
//! Long-lived routes must not run under it. When the media WebSocket arrives (M4), mount it
//! outside this layer (or exempt its path), or every session would be cut at the deadline.

use std::time::Duration;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse as _, Response};

use crate::problem::ApiError;

/// Middleware: answers [`ApiError::Timeout`] when the rest of the stack takes longer than
/// `limit`. Dropping the handler's future cancels its work, including a pending database call.
pub(crate) async fn within_deadline(
    State(limit): State<Duration>,
    request: Request,
    next: Next,
) -> Response {
    tokio::time::timeout(limit, next.run(request))
        .await
        .unwrap_or_else(|_elapsed| ApiError::Timeout.into_response())
}
