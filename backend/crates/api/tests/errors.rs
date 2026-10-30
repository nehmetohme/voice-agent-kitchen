//! Error paths of `/api/v1` that the contract doesn't pin: storage failures are 503 problems
//! whose body never carries the internal error, write conflicts are 409, a body without
//! `application/json` is 415, and unknown paths still need an identity.
#![expect(
    clippy::expect_used,
    reason = "helpers outside #[test] functions fail the test on a malformed response"
)]

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use tower::ServiceExt as _;
use vak_testkit::repositories::InMemoryAgentRepo;

/// What the in-memory repo puts in its errors; it must never reach a response.
const INTERNAL_ERROR: &str = "in-memory repo switched off";

fn app() -> (Router, Arc<InMemoryAgentRepo>) {
    let repo = Arc::new(InMemoryAgentRepo::default());
    let agents = vak_services::AgentService::new(repo.clone());
    (vak_api::app(vak_api::AppState::new(agents)), repo)
}

async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<&str>,
) -> (StatusCode, String, Value) {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("x-forwarded-user", "42")
        .header("x-forwarded-email", "ada@example.com");
    let request = match body {
        Some(body) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned())),
        None => builder.body(Body::empty()),
    }
    .expect("valid request");
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("the router never fails");
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("readable")
        .to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, content_type, body)
}

async fn create(app: &Router) -> String {
    let (_, _, body) = send(
        app,
        Method::POST,
        "/api/v1/agents",
        Some(r#"{"name":"Front desk","useCase":"support","voice":"af_heart"}"#),
    )
    .await;
    body["id"].as_str().expect("an id").to_owned()
}

#[tokio::test]
async fn storage_failures_are_503_problems_without_internal_details() {
    let (app, repo) = app();
    let id = create(&app).await;
    repo.set_unavailable(true);
    let agent = format!("/api/v1/agents/{id}");
    let calls = [
        (
            Method::POST,
            "/api/v1/agents".to_owned(),
            Some(r#"{"name":"A","useCase":"survey","voice":"af_heart"}"#),
        ),
        (Method::GET, "/api/v1/agents".to_owned(), None),
        (Method::GET, agent.clone(), None),
        (Method::PATCH, agent.clone(), Some(r#"{"name":"B"}"#)),
        (Method::DELETE, agent.clone(), None),
        (Method::GET, format!("{agent}/progress"), None),
    ];

    for (method, uri, body) in calls {
        let (status, content_type, problem) = send(&app, method.clone(), &uri, body).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{method} {uri}");
        assert_eq!(content_type, "application/problem+json");
        assert_eq!(problem["status"], 503);
        assert_eq!(problem["title"], "Service Unavailable");
        assert!(!problem.to_string().contains(INTERNAL_ERROR), "{problem}");
    }
}

#[tokio::test]
async fn writes_that_keep_conflicting_are_409_problems() {
    let (app, repo) = app();
    let id = create(&app).await;
    let agent = format!("/api/v1/agents/{id}");

    repo.fail_next_updates_with_conflict(3);
    let (patch, _, patch_problem) = send(
        &app,
        Method::PATCH,
        &agent,
        Some(r#"{"name":"After hours"}"#),
    )
    .await;
    repo.fail_next_updates_with_conflict(3);
    let (archive, _, archive_problem) = send(&app, Method::DELETE, &agent, None).await;

    assert_eq!(patch, StatusCode::CONFLICT);
    assert_eq!(patch_problem["status"], 409);
    assert_eq!(archive, StatusCode::CONFLICT);
    assert_eq!(
        archive_problem["detail"],
        "The agent changed while archiving. Try again."
    );
}

#[tokio::test]
async fn a_body_that_is_not_json_is_a_415_problem() {
    let (app, _) = app();
    let request = Request::post("/api/v1/agents")
        .header("x-forwarded-user", "42")
        .header("x-forwarded-email", "ada@example.com")
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from("name=Front desk"))
        .expect("valid request");

    let response = app.oneshot(request).await.expect("the router never fails");

    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn a_body_of_the_wrong_shape_is_a_422_problem() {
    let (app, _) = app();

    let (status, content_type, problem) =
        send(&app, Method::POST, "/api/v1/agents", Some(r#"{"name":42}"#)).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(content_type, "application/problem+json");
    assert_eq!(
        problem["detail"],
        "The request body doesn't match the schema."
    );
}

#[tokio::test]
async fn unknown_api_paths_need_an_identity_then_answer_404_problems() {
    let (app, _) = app();
    let anonymous = Request::get("/api/v1/nowhere")
        .body(Body::empty())
        .expect("valid request");

    let unauthenticated = app.clone().oneshot(anonymous).await.expect("never fails");
    let (status, content_type, problem) = send(&app, Method::GET, "/api/v1/nowhere", None).await;

    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(content_type, "application/problem+json");
    assert_eq!(
        problem,
        json!({
            "type": "about:blank",
            "title": "Not Found",
            "status": 404,
            "detail": "Nothing exists at this address.",
        })
    );
}
