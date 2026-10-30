//! HTTP and WebSocket API: axum routers, DTOs, request validation, RFC 9457
//! problem+json errors and the OpenAPI document.
//!
//! Handlers are thin: parse, validate, call one service, map the result.
//!
//! Routes are registered through utoipa-axum's `OpenApiRouter`, so a route and its OpenAPI
//! description can't drift apart. [`app`] builds the application router that the
//! composition root (`vak-server`) serves. [`openapi_json`] renders the same routes as the
//! OpenAPI 3.1 document committed at `contracts/openapi.json`.

mod agents;
mod catalog;
mod health;
mod identity;
mod json;
mod openapi;
mod problem;
mod state;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware;

pub use openapi::openapi_json;
pub use state::AppState;

use crate::problem::ApiError;

/// Where the versioned API lives.
const API_PREFIX: &str = "/api/v1";
/// The largest request body any route accepts. Agent drafts are a few hundred bytes.
const BODY_LIMIT: usize = 64 * 1024;

/// The unauthenticated routes.
///
/// | Route | Handler |
/// |---|---|
/// | `GET /health` | liveness: `{"status":"ok","version":"<vak-api version>"}` |
pub fn router() -> Router {
    openapi::public().into()
}

/// The whole application: [`router`] plus the `/api/v1` routes. Every `/api/v1` request,
/// including unknown paths, needs the forwarded identity (or the dev operator) and answers
/// errors as `application/problem+json`.
///
/// | Route | Handler |
/// |---|---|
/// | `POST /api/v1/agents` | create (only `name`, `useCase` and `voice` required) |
/// | `GET /api/v1/agents` | active agents, newest first, with setup progress |
/// | `GET /api/v1/agents/{id}` | one agent |
/// | `PATCH /api/v1/agents/{id}` | change the draft (autosave) |
/// | `DELETE /api/v1/agents/{id}` | archive |
/// | `GET /api/v1/agents/{id}/progress` | setup progress |
/// | `GET /api/v1/catalog/voices`, `/languages` | what an agent can use |
pub fn app(state: AppState) -> Router {
    let (v1, _) = openapi::v1().split_for_parts();
    let v1 = v1
        .fallback(not_found)
        .layer(middleware::from_fn_with_state(
            state.identity.clone(),
            identity::require_actor,
        ))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .with_state(state);
    router().nest(API_PREFIX, v1)
}

async fn not_found() -> ApiError {
    ApiError::NotFound
}
