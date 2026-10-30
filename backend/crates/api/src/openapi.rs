//! The OpenAPI document, built from the same `OpenApiRouter` the server serves.

use serde_json::{Map, Value};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::health;

/// Document-level metadata. Paths and schemas come from the routes themselves.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Voice Agent Kitchen API",
        description = "Provision, plan, test and publish real-time voice agents."
    ),
    tags((name = "health", description = "Process liveness"))
)]
struct ApiDoc;

/// Every route, each registered together with its OpenAPI description.
pub(crate) fn routes() -> OpenApiRouter {
    OpenApiRouter::with_openapi(ApiDoc::openapi()).routes(routes!(health::get))
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
