//! Contract `voice-core` v1 (media protocol, server side): the `vak.media.v1` event types in
//! `vak_providers::media` agree with the JSON Schema `contracts/media/vak.media.v1.json` and
//! the shared examples `contracts/media/examples.json`. The browser checks the same files in
//! `frontend/src/shared/voice/media-protocol.contract.test.ts`.
//!
//! What it pins:
//! - The schema is valid JSON Schema (draft 2020-12), and the examples agree with it: every
//!   valid example validates, every invalid one does not.
//! - The server accepts what clients may send and nothing else: every valid client example
//!   parses as a `ClientEvent` (into the expected variant), every invalid one is refused.
//! - The server sends exactly the schema's shapes: every valid server example parses as a
//!   `ServerEvent` and serializes back to the identical JSON.
//! - The examples use every event type the schema defines, and the server's type lists
//!   (`CLIENT_EVENT_TYPES`, `SERVER_EVENT_TYPES`) name exactly those types.
//! - The protocol constants (subprotocol, sample rates, frame length) match the schema.
//!
//! Contract file: never edit it once it is tagged `contract/voice-core/v1`. To change the
//! contract, add a new version.
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers: a failed expectation is a test failure"
)]

use std::collections::BTreeSet;

use serde_json::{Value, json};
use vak_providers::media::{
    CLIENT_EVENT_TYPES, ClientEvent, FRAME_MS, INPUT_SAMPLE_RATE, OUTPUT_SAMPLE_RATE, PROTOCOL,
    SERVER_EVENT_TYPES, ServerEvent,
};

fn contract_file(name: &str) -> Value {
    let path = format!(
        "{}/../../../contracts/media/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path} is not JSON: {e}"))
}

fn schema() -> Value {
    contract_file("vak.media.v1.json")
}

fn examples() -> Value {
    contract_file("examples.json")
}

/// A validator for the schema's `$defs/<def>`.
fn validator(def: &str) -> jsonschema::Validator {
    let mut root = schema();
    root["$ref"] = json!(format!("#/$defs/{def}"));
    jsonschema::validator_for(&root).unwrap_or_else(|e| panic!("$defs/{def} compiles: {e}"))
}

fn valid(side: &str) -> Vec<Value> {
    examples()[side]["valid"]
        .as_array()
        .unwrap_or_else(|| panic!("examples.json has {side}.valid"))
        .clone()
}

/// `(why, event)` of every invalid example of `side`.
fn invalid(side: &str) -> Vec<(String, Value)> {
    examples()[side]["invalid"]
        .as_array()
        .unwrap_or_else(|| panic!("examples.json has {side}.invalid"))
        .iter()
        .map(|example| {
            let why = example["why"]
                .as_str()
                .expect("every invalid example says why");
            (why.to_owned(), example["event"].clone())
        })
        .collect()
}

/// The `type` const of every event in the schema's `$defs/<union>` (`client_event` or
/// `server_event`).
fn schema_types(union: &str) -> BTreeSet<String> {
    let schema = schema();
    schema["$defs"][union]["oneOf"]
        .as_array()
        .unwrap_or_else(|| panic!("$defs/{union} is a oneOf"))
        .iter()
        .map(|branch| {
            let reference = branch["$ref"].as_str().expect("each branch is a $ref");
            let def = reference
                .strip_prefix("#/$defs/")
                .expect("a local reference");
            schema["$defs"][def]["properties"]["type"]["const"]
                .as_str()
                .unwrap_or_else(|| panic!("$defs/{def} has a type const"))
                .to_owned()
        })
        .collect()
}

fn example_types(side: &str) -> BTreeSet<String> {
    valid(side)
        .iter()
        .map(|event| event["type"].as_str().expect("a type").to_owned())
        .collect()
}

#[test]
fn the_schema_is_valid_draft_2020_12() {
    let schema = schema();
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert!(
        jsonschema::meta::is_valid(&schema),
        "vak.media.v1.json is not a valid JSON Schema"
    );
    validator("client_event");
    validator("server_event");
}

#[test]
fn the_examples_agree_with_the_schema() {
    for side in ["client", "server"] {
        let validator = validator(&format!("{side}_event"));
        for event in valid(side) {
            let errors: Vec<String> = validator
                .iter_errors(&event)
                .map(|e| e.to_string())
                .collect();
            assert!(
                errors.is_empty(),
                "valid {side} example {event} is rejected: {errors:?}"
            );
        }
        for (why, event) in invalid(side) {
            assert!(
                !validator.is_valid(&event),
                "invalid {side} example ({why}) is accepted: {event}"
            );
        }
    }
}

#[test]
fn the_server_parses_every_valid_client_event() {
    for event in valid("client") {
        if let Err(error) = serde_json::from_value::<ClientEvent>(event.clone()) {
            panic!("the server refuses valid client event {event}: {error}");
        }
    }
}

#[test]
fn client_events_parse_into_their_variants() {
    let parse = |event: Value| {
        serde_json::from_value::<ClientEvent>(event.clone())
            .unwrap_or_else(|e| panic!("{event} parses: {e}"))
    };
    assert!(matches!(
        parse(json!({"type": "session.start"})),
        ClientEvent::SessionStart { .. }
    ));
    assert!(matches!(
        parse(
            json!({"type": "session.start", "audio": {"encoding": "pcm_s16le", "sample_rate": 16000}})
        ),
        ClientEvent::SessionStart { audio: Some(_), .. }
    ));
    assert!(matches!(
        parse(json!({"type": "client.mute", "muted": true})),
        ClientEvent::Mute { muted: true, .. }
    ));
    assert!(matches!(
        parse(json!({"type": "client.mute", "muted": false})),
        ClientEvent::Mute { muted: false, .. }
    ));
    assert!(matches!(
        parse(json!({"type": "session.end"})),
        ClientEvent::SessionEnd
    ));
}

#[test]
fn the_server_refuses_every_invalid_client_event() {
    for (why, event) in invalid("client") {
        assert!(
            serde_json::from_value::<ClientEvent>(event.clone()).is_err(),
            "the server accepts an invalid client event ({why}): {event}"
        );
    }
}

#[test]
fn server_events_serialize_exactly_as_the_schema_says() {
    let validator = validator("server_event");
    for event in valid("server") {
        let parsed: ServerEvent = serde_json::from_value(event.clone())
            .unwrap_or_else(|e| panic!("{event} is not a ServerEvent: {e}"));
        let wire = serde_json::to_value(&parsed).expect("a ServerEvent serializes");
        assert_eq!(wire, event, "{event} does not serialize back unchanged");
        assert!(validator.is_valid(&wire), "{wire} breaks the schema");
    }
}

#[test]
fn the_examples_use_every_event_type_and_the_server_knows_them_all() {
    for (side, union, server_list) in [
        ("client", "client_event", CLIENT_EVENT_TYPES),
        ("server", "server_event", SERVER_EVENT_TYPES),
    ] {
        let in_schema = schema_types(union);
        assert_eq!(
            example_types(side),
            in_schema,
            "the valid {side} examples should use every {side} event type"
        );
        let known: BTreeSet<String> = server_list.iter().map(|t| (*t).to_owned()).collect();
        assert_eq!(
            known, in_schema,
            "vak_providers::media's {side} event types differ from the schema"
        );
    }
}

#[test]
fn the_protocol_constants_match_the_schema() {
    let schema = schema();
    let defs = &schema["$defs"];
    assert_eq!(PROTOCOL, "vak.media.v1");
    assert!(
        schema["title"]
            .as_str()
            .is_some_and(|title| title.starts_with(PROTOCOL)),
        "the schema's title names the protocol"
    );
    assert_eq!(
        defs["input_audio"]["properties"]["sample_rate"]["const"],
        json!(INPUT_SAMPLE_RATE)
    );
    let audio = &defs["session_started"]["properties"]["audio"]["properties"];
    assert_eq!(
        audio["input"]["properties"]["sample_rate"]["const"],
        json!(INPUT_SAMPLE_RATE)
    );
    assert_eq!(
        audio["output"]["properties"]["sample_rate"]["const"],
        json!(OUTPUT_SAMPLE_RATE)
    );
    assert_eq!(
        audio["input"]["properties"]["frame_ms"]["const"],
        json!(FRAME_MS)
    );
    assert_eq!(
        audio["output"]["properties"]["frame_ms"]["const"],
        json!(FRAME_MS)
    );
    assert_eq!(INPUT_SAMPLE_RATE, 16_000);
    assert_eq!(OUTPUT_SAMPLE_RATE, 24_000);
    assert_eq!(FRAME_MS, 20);
}
