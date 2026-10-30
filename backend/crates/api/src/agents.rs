//! `/api/v1/agents`: create, list, get, update (autosave), archive, progress. Each handler
//! parses its input, calls one service function and maps the result.

mod dto;
mod requests;

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use vak_domain::{Actor, AgentId, DraftPatch};
use vak_services::CreateAgent;

use self::dto::{AgentDto, AgentListDto, AgentSummaryDto, SetupProgressDto};
pub(crate) use self::requests::{CreateAgentRequest, UpdateAgentRequest};
use crate::json::JsonBody;
use crate::problem::{ApiError, Problem};
use crate::state::AppState;

/// An ID that can't name an agent is the same as one that names no agent.
fn agent_id(raw: &str) -> Result<AgentId, ApiError> {
    AgentId::parse(raw).map_err(|_| ApiError::NotFound)
}

/// Create an agent with only a name, a purpose and a voice; every other field gets a default.
#[utoipa::path(
    post,
    path = "/agents",
    operation_id = "createAgent",
    tag = "agents",
    request_body = CreateAgentRequest,
    responses(
        (status = CREATED, description = "The agent, with every default filled in", body = AgentDto,
            headers(("Location" = String, description = "The new agent's URL"))),
        (status = BAD_REQUEST, description = "The body is not JSON", body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = UNPROCESSABLE_ENTITY, description = "Invalid fields, each named in `errors`", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn create(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    JsonBody(body): JsonBody<CreateAgentRequest>,
) -> Result<Response, ApiError> {
    let command = CreateAgent::try_from(body)?;
    let agent = state.agents.create(&actor, command).await?;
    let location = format!("/api/v1/agents/{}", agent.id());
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(AgentDto::from(&agent)),
    )
        .into_response())
}

/// List active agents, most recently updated first, with their setup progress.
#[utoipa::path(
    get,
    path = "/agents",
    operation_id = "listAgents",
    tag = "agents",
    responses(
        (status = OK, description = "Active agents, newest first (at most 200)", body = AgentListDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn list(State(state): State<AppState>) -> Result<Json<AgentListDto>, ApiError> {
    let summaries = state.agents.list().await?;
    Ok(Json(AgentListDto {
        items: summaries.iter().map(AgentSummaryDto::from).collect(),
    }))
}

/// Get one agent, archived or not.
#[utoipa::path(
    get,
    path = "/agents/{id}",
    operation_id = "getAgent",
    tag = "agents",
    params(("id" = String, Path, format = "uuid", description = "The agent's ID")),
    responses(
        (status = OK, description = "The agent", body = AgentDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such agent", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AgentDto>, ApiError> {
    let agent = state.agents.get(agent_id(&id)?).await?;
    Ok(Json(AgentDto::from(&agent)))
}

/// Change the agent's draft (autosave). Send only the fields that changed; a change that
/// changes nothing is stored nowhere and audited nowhere.
#[utoipa::path(
    patch,
    path = "/agents/{id}",
    operation_id = "updateAgent",
    tag = "agents",
    params(("id" = String, Path, format = "uuid", description = "The agent's ID")),
    request_body = UpdateAgentRequest,
    responses(
        (status = OK, description = "The agent after the change", body = AgentDto),
        (status = BAD_REQUEST, description = "The body is not JSON", body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such agent", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The agent is archived, or kept changing while saving", body = Problem, content_type = "application/problem+json"),
        (status = UNPROCESSABLE_ENTITY, description = "Invalid fields, each named in `errors`", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn update(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(body): JsonBody<UpdateAgentRequest>,
) -> Result<Json<AgentDto>, ApiError> {
    let id = agent_id(&id)?;
    let patch = DraftPatch::try_from(body)?;
    let agent = state.agents.update_draft(&actor, id, &patch).await?;
    Ok(Json(AgentDto::from(&agent)))
}

/// Archive the agent: lists hide it and its draft can't change. Archiving twice is fine.
#[utoipa::path(
    delete,
    path = "/agents/{id}",
    operation_id = "archiveAgent",
    tag = "agents",
    params(("id" = String, Path, format = "uuid", description = "The agent's ID")),
    responses(
        (status = NO_CONTENT, description = "The agent is archived"),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such agent", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The agent kept changing while archiving", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn archive(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.agents.archive(&actor, agent_id(&id)?).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The agent's setup progress: Provision, Plan, Test and Publish, 25 % each.
#[utoipa::path(
    get,
    path = "/agents/{id}/progress",
    operation_id = "getAgentProgress",
    tag = "agents",
    params(("id" = String, Path, format = "uuid", description = "The agent's ID")),
    responses(
        (status = OK, description = "Setup progress, derived from the agent's state", body = SetupProgressDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such agent", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn progress(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SetupProgressDto>, ApiError> {
    let progress = state.agents.progress(agent_id(&id)?).await?;
    Ok(Json(SetupProgressDto::from(&progress)))
}
