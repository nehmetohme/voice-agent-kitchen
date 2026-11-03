//! Voice sessions: `POST /agents/{id}/sessions` prepares a live test call, and
//! `GET /sessions/{id}/media` is its WebSocket (subprotocol `vak.media.v1`,
//! `contracts/media/`), which [`bridge`] connects to the pipeline.

mod bridge;

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Serialize;
use utoipa::ToSchema;
use vak_domain::{Actor, AgentId};
use vak_services::media::PROTOCOL;

use crate::problem::{ApiError, Problem};
use crate::state::AppState;

/// The largest WebSocket message a client may send: well above one 20 ms audio frame
/// (640 bytes) or a client event.
const MAX_MESSAGE_BYTES: usize = 64 * 1024;

/// A prepared test call: open its media WebSocket within a minute.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = VoiceSession)]
pub(crate) struct VoiceSessionDto {
    /// The session's ID.
    #[schema(format = "uuid")]
    id: String,
    /// The media WebSocket, relative to the server: subprotocol `vak.media.v1`, usable once,
    /// by the operator who started the session.
    #[schema(example = "/api/v1/sessions/0192f4c1-7c1e-7d34-9a51-3c2b1d0e9f87/media")]
    media_url: String,
}

/// Prepare a live test call with the agent.
#[utoipa::path(
    post,
    path = "/agents/{id}/sessions",
    operation_id = "startSession",
    tag = "sessions",
    params(("id" = String, Path, format = "uuid", description = "The agent's ID")),
    responses(
        (status = CREATED, description = "The session, waiting for its media WebSocket", body = VoiceSessionDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such agent", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The agent is archived", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Voice isn't set up, too many calls are running, or storage is unavailable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn start(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let agent = AgentId::parse(&id).map_err(|_| ApiError::NotFound)?;
    let ticket = state.voice.start_session(&actor, agent).await?;
    let session = VoiceSessionDto {
        media_url: format!("{}{}", crate::API_PREFIX, ticket.media_path),
        id: ticket.session_id,
    };
    Ok((StatusCode::CREATED, Json(session)).into_response())
}

/// The session's media WebSocket. Offer the `vak.media.v1` subprotocol; the text and binary
/// frames are described in `contracts/media/`.
#[utoipa::path(
    get,
    path = "/sessions/{id}/media",
    operation_id = "openSessionMedia",
    tag = "sessions",
    params(("id" = String, Path, format = "uuid", description = "The session's ID")),
    responses(
        (status = SWITCHING_PROTOCOLS, description = "The media WebSocket, subprotocol vak.media.v1"),
        (status = BAD_REQUEST, description = "Not a WebSocket upgrade that offers vak.media.v1", body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The handshake comes from a page on another site", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such session for this operator, or it expired or was used", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Too many calls are running", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn media(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    headers: HeaderMap,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Result<Response, ApiError> {
    let upgrade = upgrade.map_err(|_| ApiError::BadRequest("Open this address as a WebSocket."))?;
    if !same_origin(&headers) {
        return Err(ApiError::Forbidden(
            "Open test calls from this app's own pages.",
        ));
    }
    if !offers_protocol(&headers) {
        return Err(ApiError::BadRequest(
            "Offer the vak.media.v1 WebSocket subprotocol.",
        ));
    }
    let session = state.voice.open_media(&actor, &id)?;
    Ok(upgrade
        .protocols([PROTOCOL])
        .max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| bridge::run(socket, session)))
}

/// Whether a WebSocket handshake comes from this app's own pages.
///
/// A browser sends `Origin` on every WebSocket handshake and attaches the operator's session
/// cookie, so without this a page on another site could open a call as them (cross-site
/// WebSocket hijacking). Page scripts can't set `Origin`, `Host` or `X-Forwarded-Host` on a
/// handshake. A request without `Origin` is not a browser and has no ambient credentials, so
/// it passes, as with gorilla/websocket's default check. Behind the Databricks Apps proxy the
/// public host is `X-Forwarded-Host` (the first, if several); otherwise `Host`.
fn same_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get(header::HOST))
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim);
    // `Origin` is `scheme://host[:port]`, or `null` from an opaque origin.
    let authority = origin
        .to_str()
        .ok()
        .and_then(|origin| origin.split_once("://"))
        .map(|(_, authority)| authority);
    matches!((authority, host), (Some(authority), Some(host)) if authority.eq_ignore_ascii_case(host))
}

/// Whether the client offered `vak.media.v1` in `Sec-WebSocket-Protocol`.
fn offers_protocol(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|offered| offered.trim() == PROTOCOL)
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        pairs
            .iter()
            .map(|(name, value)| {
                (
                    header::HeaderName::from_static(name),
                    HeaderValue::from_static(value),
                )
            })
            .collect()
    }

    #[test]
    fn only_the_apps_own_pages_may_open_a_call() {
        let local = [("host", "127.0.0.1:5173")];
        assert!(same_origin(&headers(&local)), "no Origin: not a browser");
        assert!(same_origin(&headers(&[
            ("host", "127.0.0.1:5173"),
            ("origin", "http://127.0.0.1:5173")
        ])));
        assert!(same_origin(&headers(&[
            ("host", "10.0.0.7:8000"),
            ("x-forwarded-host", "vak.cloud.databricksapps.com, proxy"),
            ("origin", "https://VAK.cloud.databricksapps.com")
        ])));

        for origin in [
            "https://evil.example",
            "http://127.0.0.1:5174",
            "null",
            "garbage",
        ] {
            let mut cross = headers(&local);
            cross.insert(header::ORIGIN, HeaderValue::from_static(origin));
            assert!(!same_origin(&cross), "{origin}");
        }
        assert!(
            !same_origin(&headers(&[("origin", "http://127.0.0.1:5173")])),
            "no host to compare with"
        );
    }

    #[test]
    fn the_subprotocol_must_be_offered() {
        let mut headers = HeaderMap::new();
        assert!(!offers_protocol(&headers));
        headers.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("chat, vak.media.v1"),
        );
        assert!(offers_protocol(&headers));
        headers.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("vak.media.v2"),
        );
        assert!(!offers_protocol(&headers));
    }
}
