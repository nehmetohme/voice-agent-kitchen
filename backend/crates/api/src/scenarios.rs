//! `/api/v1/agents/{id}/scenarios`: read the agent's test scenarios (or its use case's
//! defaults) and replace them. Each handler parses its input, calls one service function and
//! maps the result.

mod dto;

use axum::extract::{Path, State};
use axum::{Extension, Json};
use vak_domain::{Actor, ScenarioSet};

pub(crate) use self::dto::ExpectedDto;
use self::dto::{AgentScenariosDto, ScenariosRequest};
use crate::agents::agent_id;
use crate::json::JsonBody;
use crate::problem::{ApiError, Problem};
use crate::state::AppState;

/// The agent's test scenarios: the saved ones, or its use case's defaults until some are
/// saved.
#[utoipa::path(
    get,
    path = "/agents/{id}/scenarios",
    operation_id = "getAgentScenarios",
    tag = "testing",
    params(("id" = String, Path, format = "uuid", description = "The agent's ID")),
    responses(
        (status = OK, description = "The scenarios the agent is tested with", body = AgentScenariosDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such agent", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AgentScenariosDto>, ApiError> {
    let shown = state.agents.scenarios(agent_id(&id)?).await?;
    Ok(Json(AgentScenariosDto::from(&shown)))
}

/// Replace the agent's test scenarios. A malformed set is refused and nothing is stored.
#[utoipa::path(
    put,
    path = "/agents/{id}/scenarios",
    operation_id = "saveAgentScenarios",
    tag = "testing",
    params(("id" = String, Path, format = "uuid", description = "The agent's ID")),
    request_body = ScenariosRequest,
    responses(
        (status = OK, description = "The saved scenarios", body = AgentScenariosDto),
        (status = BAD_REQUEST, description = "The body is not JSON", body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such agent", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The agent is archived, or kept changing while saving", body = Problem, content_type = "application/problem+json"),
        (status = UNPROCESSABLE_ENTITY, description = "Malformed scenarios, each field named in `errors`", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn save(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(body): JsonBody<ScenariosRequest>,
) -> Result<Json<AgentScenariosDto>, ApiError> {
    let id = agent_id(&id)?;
    let scenarios = ScenarioSet::try_from(body)?;
    let saved = state.agents.save_scenarios(&actor, id, scenarios).await?;
    Ok(Json(AgentScenariosDto::from(&saved)))
}
