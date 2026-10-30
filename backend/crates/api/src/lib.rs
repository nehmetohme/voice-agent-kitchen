//! HTTP and WebSocket API: axum routers, DTOs, request validation, RFC 9457
//! problem+json errors and the OpenAPI document.
//!
//! Handlers are thin: parse, validate, call one service, map the result.
//!
//! [`router`] builds the whole application router; the composition root (`vak-server`)
//! serves it.

mod health;

use axum::Router;
use axum::routing::get;

/// The application router.
///
/// | Route | Handler |
/// |---|---|
/// | `GET /health` | liveness: `{"status":"ok","version":"<vak-api version>"}` |
pub fn router() -> Router {
    Router::new().route("/health", get(health::get))
}
