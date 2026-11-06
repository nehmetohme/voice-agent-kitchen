//! The OpenAPI document, built from the same `OpenApiRouter`s the server serves.

use axum::extract::DefaultBodyLimit;
use serde_json::{Map, Value};
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::state::AppState;
use crate::{agents, catalog, health, latency, plans, purposes, scenarios, sessions, test_runs};

/// Document-level metadata. Paths and schemas come from the routes themselves.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Voice Agent Kitchen API",
        description = "Provision, plan, test and publish real-time voice agents. Every \
            `/api/v1` route needs the operator identity Databricks Apps forwards; errors are \
            RFC 9457 `application/problem+json`."
    ),
    tags(
        (name = "health", description = "Process liveness"),
        (name = "agents", description = "Create, list, edit (autosave) and archive agents"),
        (name = "plans", description = "An agent's plan: steps and paths, checked against its rule pack"),
        (name = "purposes", description = "What agents are for and how they may work: versioned definitions with a starting flow, tools, guardrails and tighten-only compliance additions"),
        (name = "catalog", description = "The voices, languages and tools agents can use"),
        (name = "sessions", description = "Live test calls: a session and its media WebSocket \
            (subprotocol `vak.media.v1`, `contracts/media/`)"),
        (name = "voice", description = "The voice path's latency"),
        (name = "testing", description = "Test scenarios, and test runs that play them against \
            simulated callers and score them by their rubrics"),
    ),
    modifiers(&ForwardedIdentity)
)]
struct ApiDoc;

/// The `databricksApps` security scheme: the identity headers the Databricks Apps proxy
/// sets after single sign-on.
struct ForwardedIdentity;

impl Modify for ForwardedIdentity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "databricksApps",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                "X-Forwarded-Email",
                "Set by the Databricks Apps proxy after sign-on, together with \
                 X-Forwarded-User. Clients never send them themselves.",
            ))),
        );
    }
}

/// The unauthenticated routes: `GET /health`.
pub(crate) fn public<S: Clone + Send + Sync + 'static>() -> OpenApiRouter<S> {
    OpenApiRouter::new().routes(routes!(health::get))
}

/// The `/api/v1` routes, before identity and state are applied.
pub(crate) fn v1() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(agents::create, agents::list))
        .routes(routes!(agents::get, agents::update, agents::archive))
        .routes(routes!(agents::progress))
        .routes(routes!(agents::purpose))
        .routes(routes!(agents::update_purpose))
        .merge(
            OpenApiRouter::new()
                .routes(routes!(plans::get, plans::save))
                .routes(routes!(plans::validate))
                // Plans are larger than the other bodies; this limit replaces the default.
                .layer(DefaultBodyLimit::max(plans::BODY_LIMIT)),
        )
        .merge(
            OpenApiRouter::new()
                .routes(routes!(purposes::list, purposes::create))
                .routes(routes!(purposes::get, purposes::update))
                .routes(routes!(purposes::flow, purposes::save_flow))
                .routes(routes!(purposes::save_version))
                // A purpose's flow is as large as a plan.
                .layer(DefaultBodyLimit::max(purposes::BODY_LIMIT)),
        )
        .routes(routes!(catalog::voices))
        .routes(routes!(catalog::languages))
        .routes(routes!(catalog::tools))
        .routes(routes!(sessions::start))
        .routes(routes!(latency::get))
        .routes(routes!(scenarios::get, scenarios::save))
        .routes(routes!(test_runs::start))
        .routes(routes!(test_runs::get))
        .routes(routes!(test_runs::cancel))
        .routes(routes!(test_runs::list))
}

/// The `/api/v1` routes that outlive a request (the media WebSocket): identity, but no
/// request deadline and no body limit.
pub(crate) fn v1_media() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(sessions::media))
}

/// Every route, each registered together with its OpenAPI description.
fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .merge(public())
        .nest(crate::API_PREFIX, v1().merge(v1_media()))
}

/// The OpenAPI 3.1 document as pretty JSON with every object's keys sorted and a trailing
/// newline, so the same code always renders the same bytes (`contracts/openapi.json`).
pub fn openapi_json() -> Result<String, serde_json::Error> {
    let (_, document) = routes().split_for_parts();
    let document = sorted(serde_json::to_value(document)?);
    let mut json = serde_json::to_string_pretty(&document)?;
    json.push('\n');
    Ok(json)
}

/// Sorts object keys at every level, whatever map type `serde_json` was built with.
fn sorted(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key, sorted(value)))
                    .collect::<Map<String, Value>>(),
            )
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn sorts_keys_at_every_level_and_keeps_array_order() {
        let value = json!({ "b": [{ "z": 1, "a": 2 }, 3], "a": { "y": null, "x": true } });

        let rendered = serde_json::to_string(&sorted(value)).expect("serializes");

        assert_eq!(
            rendered,
            r#"{"a":{"x":true,"y":null},"b":[{"a":2,"z":1},3]}"#
        );
    }
}
