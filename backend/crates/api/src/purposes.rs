//! `/api/v1/purposes`: list, duplicate, read and autosave a purpose's draft and starting flow,
//! and save a version. Each handler parses its input, calls one service function and maps the
//! result.

pub(crate) mod dto;
mod requests;

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use vak_domain::{Actor, Plan};
use vak_services::DuplicatePurpose;

use self::dto::{PurposeDto, PurposeListDto, PurposeSummaryDto};
pub(crate) use self::requests::{CreatePurposeRequest, UpdatePurposeRequest, purpose_id};
use crate::json::JsonBody;
use crate::plans::{AgentPlanDto, PlanRequest};
use crate::problem::{ApiError, Problem};
use crate::state::AppState;

/// The largest purpose body: a whole flow fits, like a plan's.
pub(crate) const BODY_LIMIT: usize = crate::plans::BODY_LIMIT;

/// List the built-in purposes, then the workspace's own, with their versions and usage.
#[utoipa::path(
    get,
    path = "/purposes",
    operation_id = "listPurposes",
    tag = "purposes",
    responses(
        (status = OK, description = "Built-in purposes in use-case order, then the workspace's own, newest first", body = PurposeListDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn list(State(state): State<AppState>) -> Result<Json<PurposeListDto>, ApiError> {
    let summaries = state.purposes.list().await?;
    Ok(Json(PurposeListDto {
        items: summaries.iter().map(PurposeSummaryDto::from).collect(),
    }))
}

/// Make a purpose of the workspace's as a copy of another one: a draft without a version.
#[utoipa::path(
    post,
    path = "/purposes",
    operation_id = "createPurpose",
    tag = "purposes",
    request_body = CreatePurposeRequest,
    responses(
        (status = CREATED, description = "The new purpose", body = PurposeDto,
            headers(("Location" = String, description = "The new purpose's URL"))),
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
    JsonBody(body): JsonBody<CreatePurposeRequest>,
) -> Result<Response, ApiError> {
    let command = DuplicatePurpose::try_from(body)?;
    let view = state.purposes.duplicate(&actor, command).await?;
    let location = format!("/api/v1/purposes/{}", view.purpose.id());
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(PurposeDto::from(&view)),
    )
        .into_response())
}

/// One purpose: its draft without the flow, its problems and its compliance baselines.
#[utoipa::path(
    get,
    path = "/purposes/{id}",
    operation_id = "getPurpose",
    tag = "purposes",
    params(("id" = String, Path, format = "uuid", description = "The purpose's ID")),
    responses(
        (status = OK, description = "The purpose", body = PurposeDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such purpose", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PurposeDto>, ApiError> {
    let view = state.purposes.get(purpose_id(&id)?).await?;
    Ok(Json(PurposeDto::from(&view)))
}

/// Change the draft (autosave). Send only the sections that changed; each replaces that
/// section whole. A change that changes nothing is stored and audited nowhere.
#[utoipa::path(
    patch,
    path = "/purposes/{id}",
    operation_id = "updatePurpose",
    tag = "purposes",
    params(("id" = String, Path, format = "uuid", description = "The purpose's ID")),
    request_body = UpdatePurposeRequest,
    responses(
        (status = OK, description = "The purpose after the change, with its problems", body = PurposeDto),
        (status = BAD_REQUEST, description = "The body is not JSON", body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such purpose", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "A built-in purpose can't change, or it kept changing while saving", body = Problem, content_type = "application/problem+json"),
        (status = UNPROCESSABLE_ENTITY, description = "Invalid fields, each named by its path in `errors`", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn update(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(body): JsonBody<UpdatePurposeRequest>,
) -> Result<Json<PurposeDto>, ApiError> {
    let id = purpose_id(&id)?;
    let patch = body.into_patch()?;
    let view = state.purposes.update_draft(&actor, id, &patch).await?;
    Ok(Json(PurposeDto::from(&view)))
}

/// The starting flow, in the agent plan's shape, beside its class's locked texts in the
/// default jurisdiction. It is checked by the graph rules; agents add the locked texts.
#[utoipa::path(
    get,
    path = "/purposes/{id}/flow",
    operation_id = "getPurposeFlow",
    tag = "purposes",
    params(("id" = String, Path, format = "uuid", description = "The purpose's ID")),
    responses(
        (status = OK, description = "The starting flow and its problems", body = AgentPlanDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such purpose", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn flow(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AgentPlanDto>, ApiError> {
    let shown = state.purposes.flow(purpose_id(&id)?).await?;
    Ok(Json(AgentPlanDto::from(&shown)))
}

/// Replace the draft's starting flow (autosave). A flow that breaks a graph rule is saved too
/// and answers with its problems; a malformed one is refused and nothing is stored.
#[utoipa::path(
    put,
    path = "/purposes/{id}/flow",
    operation_id = "savePurposeFlow",
    tag = "purposes",
    params(("id" = String, Path, format = "uuid", description = "The purpose's ID")),
    request_body = PlanRequest,
    responses(
        (status = OK, description = "The saved flow and its problems", body = AgentPlanDto),
        (status = BAD_REQUEST, description = "The body is not JSON", body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such purpose", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "A built-in purpose can't change, or it kept changing while saving", body = Problem, content_type = "application/problem+json"),
        (status = UNPROCESSABLE_ENTITY, description = "Malformed steps or paths, each named in `errors`", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn save_flow(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(body): JsonBody<PlanRequest>,
) -> Result<Json<AgentPlanDto>, ApiError> {
    let id = purpose_id(&id)?;
    let flow = Plan::try_from(body)?;
    let saved = state.purposes.save_flow(&actor, id, flow).await?;
    Ok(Json(AgentPlanDto::from(&saved)))
}

/// Freeze the draft as the next version. Agents that pin an older one see an update.
#[utoipa::path(
    post,
    path = "/purposes/{id}/versions",
    operation_id = "savePurposeVersion",
    tag = "purposes",
    params(("id" = String, Path, format = "uuid", description = "The purpose's ID")),
    responses(
        (status = CREATED, description = "The purpose with its new latest version", body = PurposeDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such purpose", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "A built-in purpose, nothing changed since the latest version, or it kept changing", body = Problem, content_type = "application/problem+json"),
        (status = UNPROCESSABLE_ENTITY, description = "The draft has problems, each named in `errors`", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn save_version(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let view = state
        .purposes
        .save_version(&actor, purpose_id(&id)?)
        .await?;
    Ok((StatusCode::CREATED, Json(PurposeDto::from(&view))).into_response())
}
