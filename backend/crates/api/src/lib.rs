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
mod state;

use axum::Router;

pub use openapi::openapi_json;
pub use state::AppState;

/// The application router.
///
/// | Route | Handler |
/// |---|---|
/// | `GET /health` | liveness: `{"status":"ok","version":"<vak-api version>"}` |
pub fn router() -> Router {
    openapi::routes().into()
}

/// The whole application: [`router`] plus the `/api/v1` routes, which need a signed-in
/// operator.
pub fn app(state: AppState) -> Router {
    // CONTRACT-STUB
    drop(state);
    router()
}
