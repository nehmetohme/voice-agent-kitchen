//! The OpenAPI document is generated from the code (utoipa), documents every route, and
//! renders deterministically so `contracts/openapi.json` can be drift-checked.
#![expect(
    clippy::expect_used,
    reason = "helpers outside #[test] functions fail the test on a malformed document"
)]

use serde_json::{Value, json};

fn rendered() -> String {
    vak_api::openapi_json().expect("the document serializes")
}

fn document() -> Value {
    serde_json::from_str(&rendered()).expect("the document is JSON")
}

/// Follows a local `$ref` (`#/components/schemas/<name>`) if `schema` is one.
fn resolve<'a>(document: &'a Value, schema: &'a Value) -> &'a Value {
    match schema["$ref"].as_str() {
        Some(reference) => {
            let pointer = reference.trim_start_matches('#');
            document.pointer(pointer).expect("the $ref resolves")
        }
        None => schema,
    }
}

#[test]
fn is_openapi_3_1() {
    let document = document();

    let version = document["openapi"].as_str().expect("openapi version");
    assert!(version.starts_with("3.1."), "{version}");
    assert_eq!(document["info"]["title"], "Voice Agent Kitchen API");
    assert_eq!(document["info"]["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn documents_get_health_and_its_response_schema() {
    let document = document();

    let operation = &document["paths"]["/health"]["get"];
    assert_eq!(operation["operationId"], "getHealth");
    assert_eq!(operation["tags"], json!(["health"]));
    let schema = &operation["responses"]["200"]["content"]["application/json"]["schema"];
    assert_eq!(schema["$ref"], "#/components/schemas/Health");
    let health = resolve(&document, schema);
    assert_eq!(health["type"], "object");
    assert_eq!(health["required"], json!(["status", "version"]));
    let status = resolve(&document, &health["properties"]["status"]);
    assert_eq!(status["type"], "string");
    assert_eq!(status["enum"], json!(["ok"]));
    assert_eq!(health["properties"]["version"]["type"], "string");
}

#[test]
fn documents_exactly_the_routes_the_router_serves() {
    let document = document();

    let paths: Vec<&str> = document["paths"]
        .as_object()
        .expect("paths object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(paths, ["/health"]);
}

#[test]
fn renders_deterministically_with_sorted_keys_and_a_trailing_newline() {
    let first = rendered();

    assert_eq!(first, rendered());
    assert!(first.ends_with("}\n") && !first.ends_with("\n\n"));
    // serde_json's default map is a BTreeMap, so this re-rendering has every key sorted.
    let parsed: Value = serde_json::from_str(&first).expect("JSON");
    let sorted = serde_json::to_string_pretty(&parsed).expect("serializes") + "\n";
    assert_eq!(first, sorted);
}

#[test]
fn the_committed_contract_is_current() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../contracts/openapi.json");
    let committed = std::fs::read_to_string(path).expect("contracts/openapi.json exists");

    assert!(
        committed == rendered(),
        "contracts/openapi.json is out of date: run `just openapi` and commit the result"
    );
}
