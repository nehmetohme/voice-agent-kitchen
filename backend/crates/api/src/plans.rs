//! `/api/v1/agents/{id}/plan`: read the plan (or the template), save it (autosave), and check
//! a plan without saving it. Each handler parses its input, calls one service function and
//! maps the result.

mod dto;
mod requests;

use axum::extract::{Path, State};
use axum::{Extension, Json};
use vak_domain::{Actor, Plan};

use self::dto::{AgentPlanDto, ValidationDto};
pub(crate) use self::requests::PlanRequest;
use crate::agents::agent_id;
use crate::json::JsonBody;
use crate::problem::{ApiError, Problem};
use crate::state::AppState;

/// The largest plan body: 50 steps of 2,000 characters fit, with room for multi-byte text.
pub(crate) const BODY_LIMIT: usize = 512 * 1024;

/// The agent's plan: the saved one, or its use case's template until one is saved, with the
/// rule pack it is checked against and its problems.
#[utoipa::path(
    get,
    path = "/agents/{id}/plan",
    operation_id = "getAgentPlan",
    tag = "plans",
    params(("id" = String, Path, format = "uuid", description = "The agent's ID")),
    responses(
        (status = OK, description = "The plan, checked against the agent's rule pack", body = AgentPlanDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such agent", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AgentPlanDto>, ApiError> {
    let shown = state.agents.plan(agent_id(&id)?).await?;
    Ok(Json(AgentPlanDto::from(&shown)))
}

/// Replace the agent's plan (autosave). A plan that breaks a graph rule is saved too and
/// answers with its problems; a malformed one is refused and nothing is stored.
#[utoipa::path(
    put,
    path = "/agents/{id}/plan",
    operation_id = "saveAgentPlan",
    tag = "plans",
    params(("id" = String, Path, format = "uuid", description = "The agent's ID")),
    request_body = PlanRequest,
    responses(
        (status = OK, description = "The saved plan and its problems", body = AgentPlanDto),
        (status = BAD_REQUEST, description = "The body is not JSON", body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such agent", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The agent is archived, or kept changing while saving", body = Problem, content_type = "application/problem+json"),
        (status = UNPROCESSABLE_ENTITY, description = "Malformed steps or paths, each named in `errors`", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn save(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(body): JsonBody<PlanRequest>,
) -> Result<Json<AgentPlanDto>, ApiError> {
    let id = agent_id(&id)?;
    let plan = Plan::try_from(body)?;
    let saved = state.agents.save_plan(&actor, id, plan).await?;
    Ok(Json(AgentPlanDto::from(&saved)))
}

/// Check a plan against the agent's rule pack without saving it.
#[utoipa::path(
    post,
    path = "/agents/{id}/plan/validate",
    operation_id = "validateAgentPlan",
    tag = "plans",
    params(("id" = String, Path, format = "uuid", description = "The agent's ID")),
    request_body = PlanRequest,
    responses(
        (status = OK, description = "Whether the plan is valid, and its problems", body = ValidationDto),
        (status = BAD_REQUEST, description = "The body is not JSON", body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such agent", body = Problem, content_type = "application/problem+json"),
        (status = UNPROCESSABLE_ENTITY, description = "Malformed steps or paths, each named in `errors`", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn validate(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonBody(body): JsonBody<PlanRequest>,
) -> Result<Json<ValidationDto>, ApiError> {
    let id = agent_id(&id)?;
    let plan = Plan::try_from(body)?;
    let validation = state.agents.validate_plan(id, &plan).await?;
    Ok(Json(ValidationDto::from(&validation)))
}
