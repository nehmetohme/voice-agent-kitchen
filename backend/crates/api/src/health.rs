//! `GET /health`: liveness. It reports that the process serves HTTP and checks no
//! dependencies, so the platform never restarts a healthy process over a slow database.

use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

/// Response body of `GET /health`.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct Health {
    /// Always `ok` while the process serves HTTP.
    status: HealthStatus,
    /// The server's version (the `vak-api` package version), e.g. `0.1.0`.
    version: &'static str,
}

/// Liveness state. Only `ok` exists: a process that can't serve doesn't answer at all.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum HealthStatus {
    Ok,
}

/// Liveness: answers while the process serves HTTP. Unauthenticated, checks no dependencies.
#[utoipa::path(
    get,
    path = "/health",
    operation_id = "getHealth",
    tag = "health",
    responses((status = OK, description = "The process is serving HTTP", body = Health))
)]
pub(crate) async fn get() -> Json<Health> {
    Json(Health {
        status: HealthStatus::Ok,
        version: env!("CARGO_PKG_VERSION"),
    })
}
