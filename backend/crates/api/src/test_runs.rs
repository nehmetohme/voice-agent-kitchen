//! Test runs: `POST /test-runs` queues a job that plays an agent's scenarios against simulated
//! callers, `GET /test-runs/{id}` reports its progress and verdict, `POST /test-runs/{id}/cancel`
//! stops it, and `GET /agents/{id}/test-runs` lists an agent's runs. Each handler parses its
//! input, calls one service function and maps the result.

mod dto;

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use vak_domain::{Actor, AgentId, TestRunId};
use vak_services::TestRunService;

use self::dto::{AgentTestRunsDto, StartTestRunRequest, TestRunDto};
use crate::agents::agent_id;
use crate::json::JsonBody;
use crate::problem::{ApiError, FieldError, Problem};
use crate::state::AppState;

/// The test runs this server serves; without models it serves none.
fn service(state: &AppState) -> Result<&TestRunService, ApiError> {
    state
        .test_runs
        .as_ref()
        .ok_or(ApiError::Refused("Test runs aren't set up on this server."))
}

/// An ID that can't name a run names none.
fn run_id(raw: &str) -> Result<TestRunId, ApiError> {
    TestRunId::parse(raw).map_err(|_| ApiError::NotFound)
}

/// Queue a test run of the agent's current draft: every scenario is played by a simulated
/// caller and scored by its rubric, in the background. Poll the run for its progress.
#[utoipa::path(
    post,
    path = "/test-runs",
    operation_id = "startTestRun",
    tag = "testing",
    request_body = StartTestRunRequest,
    responses(
        (status = ACCEPTED, description = "The queued run", body = TestRunDto,
            headers(("Location" = String, description = "The run's URL"))),
        (status = BAD_REQUEST, description = "The body is not JSON", body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such agent", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The agent is archived, or a run of it is already queued or running", body = Problem, content_type = "application/problem+json"),
        (status = UNPROCESSABLE_ENTITY, description = "A missing or malformed `agentId`", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "The server has no models for test runs, or storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn start(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    JsonBody(body): JsonBody<StartTestRunRequest>,
) -> Result<Response, ApiError> {
    let invalid = |message| {
        ApiError::Invalid(vec![FieldError {
            field: "agentId".to_owned(),
            message,
        }])
    };
    let raw = body
        .agent_id
        .ok_or_else(|| invalid("This field is required."))?;
    let agent = AgentId::parse(&raw).map_err(|_| invalid("Use the agent's ID, a UUID."))?;
    let run = service(&state)?.start(&actor, agent).await?;
    let location = format!("{}/test-runs/{}", crate::API_PREFIX, run.id());
    Ok((
        StatusCode::ACCEPTED,
        [(header::LOCATION, location)],
        Json(TestRunDto::from(&run)),
    )
        .into_response())
}

/// The run: its status and progress, its verdict once finished, and every scenario's
/// transcript and rubric checks so far.
#[utoipa::path(
    get,
    path = "/test-runs/{id}",
    operation_id = "getTestRun",
    tag = "testing",
    params(("id" = String, Path, format = "uuid", description = "The run's ID")),
    responses(
        (status = OK, description = "The run", body = TestRunDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such run", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Test runs aren't set up, or storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TestRunDto>, ApiError> {
    let run = service(&state)?.get(run_id(&id)?).await?;
    Ok(Json(TestRunDto::from(&run)))
}

/// Cancel a queued or running run; what is left is skipped. Cancelling a cancelled run
/// changes nothing.
#[utoipa::path(
    post,
    path = "/test-runs/{id}/cancel",
    operation_id = "cancelTestRun",
    tag = "testing",
    params(("id" = String, Path, format = "uuid", description = "The run's ID")),
    responses(
        (status = OK, description = "The cancelled run", body = TestRunDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such run", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The run already finished", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Test runs aren't set up, or storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn cancel(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
) -> Result<Json<TestRunDto>, ApiError> {
    let run = service(&state)?.cancel(&actor, run_id(&id)?).await?;
    Ok(Json(TestRunDto::from(&run)))
}

/// The agent's test runs, newest first, each marked `current` when it tested the agent's
/// current draft.
#[utoipa::path(
    get,
    path = "/agents/{id}/test-runs",
    operation_id = "listAgentTestRuns",
    tag = "testing",
    params(("id" = String, Path, format = "uuid", description = "The agent's ID")),
    responses(
        (status = OK, description = "The agent's runs", body = AgentTestRunsDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such agent", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Test runs aren't set up, or storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn list(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AgentTestRunsDto>, ApiError> {
    let id = agent_id(&id)?;
    let runs = service(&state)?.list(id).await?;
    Ok(Json(AgentTestRunsDto::from(&runs)))
}
