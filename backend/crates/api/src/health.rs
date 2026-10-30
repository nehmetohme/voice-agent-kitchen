//! `GET /health`: liveness. It reports that the process serves HTTP and checks no
//! dependencies, so the platform never restarts a healthy process over a slow database.

use axum::Json;
use serde::Serialize;

/// Response body of `GET /health`.
#[derive(Debug, Serialize)]
pub(crate) struct Health {
    status: &'static str,
    version: &'static str,
}

pub(crate) async fn get() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}
