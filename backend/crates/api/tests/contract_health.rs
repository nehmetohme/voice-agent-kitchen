//! Contract `health` v1: `GET /health` answers 200 with
//! `{"status":"ok","version":"<vak-api package version>"}`.
//!
//! Contract file: never edit it once it is tagged `contract/health/v1`. To change the
//! contract, add a new version.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use tower::ServiceExt as _;

#[tokio::test]
async fn health_returns_200_with_status_ok_and_the_api_version() {
    let request = Request::get("/health")
        .body(Body::empty())
        .expect("valid request");

    let response = vak_api::router()
        .oneshot(request)
        .await
        .expect("the router never fails");

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    assert_eq!(content_type, Some("application/json"));
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body is readable")
        .to_bytes();
    let body: Value = serde_json::from_slice(&body).expect("body is JSON");
    assert_eq!(
        body,
        json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") })
    );
}
