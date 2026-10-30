//! HTTP and WebSocket API: axum routers, DTOs, request validation, RFC 9457
//! problem+json errors and the OpenAPI document.
//!
//! Handlers are thin: parse, validate, call one service, map the result.
//!
//! Routes are registered through utoipa-axum's `OpenApiRouter`, so a route and its OpenAPI
//! description can't drift apart. [`router`] builds the application router that the
//! composition root (`vak-server`) serves. [`openapi_json`] renders the same routes as the
//! OpenAPI 3.1 document committed at `contracts/openapi.json`.

mod health;
mod openapi;

use axum::Router;

pub use openapi::openapi_json;

/// The application router.
///
/// | Route | Handler |
/// |---|---|
/// | `GET /health` | liveness: `{"status":"ok","version":"<vak-api version>"}` |
pub fn router() -> Router {
    openapi::routes().into()
}
