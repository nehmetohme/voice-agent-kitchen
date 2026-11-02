//! Contract `voice-core` v1 (TTS adapter): `vak-adapter-tts-serving` speaks the Databricks
//! serving contract `contracts/serving/tts.json` and passes the shared `TtsEngine` suite
//! (`vak_testkit::conformance::tts_conformance`), against a wiremock stand-in here and
//! against the local model shim in the ignored live test.
//!
//! What it pins:
//! - It passes the conformance suite against an endpoint that answers only schema-valid
//!   requests: one record per clause with its text and voice, `pcm_s16le` at 24 kHz, at
//!   `POST {base_url}/serving-endpoints/{endpoint}/invocations`; the speed goes along.
//! - The audio comes back as exactly the samples the endpoint sent (base64 of s16le bytes).
//! - Text the contract forbids (blank, over 1000 characters) is refused before any request.
//! - `Authorization: Bearer <token>` goes out only when a token is configured.
//! - Serving errors map to `ProviderError` as for STT: 400 and 404 `Protocol`, 401 and 403
//!   `Auth`, 429 `RateLimited`, 503 `Unavailable`, 504 `Timeout` (those three retryable), 500
//!   not retryable. An answer in another format (not 24 kHz, an odd byte count, bad base64,
//!   no prediction) is `Protocol`: audio is never relabelled.
//! - A slow endpoint fails with `Timeout` at the request's deadline; an unreachable one with
//!   `Unavailable`.
//! - Live (`#[ignore]`, `just dev`): the suite passes against `VAK_TTS_BASE_URL`.
//!
//! Contract file: never edit it once it is tagged `contract/voice-core/v1`. To change the
//! contract, add a new version.
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers: a failed expectation is a test failure"
)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use secrecy::SecretString;
use serde_json::{Value, json};
use vak_adapter_tts_serving::{ServingTts, ServingTtsConfig};
use vak_providers::{ProviderError, Speech, TtsEngine, TtsRequest};
use vak_testkit::conformance::tts_conformance;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const INVOCATIONS: &str = "/serving-endpoints/tts/invocations";
const DEADLINE: Duration = Duration::from_secs(5);
const VOICE: &str = "af_heart";

/// A validator for `contracts/serving/tts.json`'s `$defs/<def>`.
fn schema(def: &str) -> jsonschema::Validator {
    let path = format!(
        "{}/../../../../contracts/serving/tts.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"));
    let mut root: Value = serde_json::from_str(&text).expect("the schema is JSON");
    root["$ref"] = json!(format!("#/$defs/{def}"));
    jsonschema::validator_for(&root).unwrap_or_else(|e| panic!("tts.json compiles: {e}"))
}

fn pcm_b64(samples: &[i16]) -> String {
    let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    BASE64.encode(bytes)
}

/// A wiremock TTS endpoint. It refuses (400) any body that breaks the request schema, and
/// speaks each record as a loud tone of 50 ms per character of its text, at 24 kHz. It
/// records every body it received.
struct Endpoint {
    server: MockServer,
    bodies: Arc<Mutex<Vec<Value>>>,
}

impl Endpoint {
    async fn start() -> Self {
        let server = MockServer::start().await;
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let seen = bodies.clone();
        let request_schema = schema("request");
        let response_schema = schema("response");
        Mock::given(method("POST"))
            .and(path(INVOCATIONS))
            .respond_with(move |request: &Request| {
                let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
                seen.lock().expect("unpoisoned").push(body.clone());
                if !request_schema.is_valid(&body) {
                    return ResponseTemplate::new(400).set_body_json(
                        json!({"error_code": "BAD_REQUEST", "message": "the body breaks the schema"}),
                    );
                }
                let predictions: Vec<Value> = body["dataframe_records"]
                    .as_array()
                    .expect("records")
                    .iter()
                    .map(|record| {
                        let chars = record["text"].as_str().expect("text").chars().count();
                        // 50 ms per character: 1,200 samples at 24 kHz.
                        let samples: Vec<i16> = (0..chars * 1_200)
                            .map(|i| if (i / 24) % 2 == 0 { 6_000 } else { -6_000 })
                            .collect();
                        json!({"audio_b64": pcm_b64(&samples), "encoding": "pcm_s16le", "sample_rate": 24000})
                    })
                    .collect();
                let response = json!({ "predictions": predictions });
                assert!(response_schema.is_valid(&response), "the fake answers by the schema");
                ResponseTemplate::new(200).set_body_json(response)
            })
            .mount(&server)
            .await;
        Self { server, bodies }
    }

    fn engine(&self) -> ServingTts {
        ServingTts::new(ServingTtsConfig::new(self.server.uri(), "tts"))
            .expect("the adapter builds")
    }

    fn bodies(&self) -> Vec<Value> {
        self.bodies.lock().expect("unpoisoned").clone()
    }
}

/// An endpoint that answers every request with `status` and `body`.
async fn answering(status: u16, body: Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(INVOCATIONS))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(&server)
        .await;
    server
}

async fn synthesize(
    base_url: String,
    text: &str,
    deadline: Duration,
) -> Result<Speech, ProviderError> {
    let engine =
        ServingTts::new(ServingTtsConfig::new(base_url, "tts")).expect("the adapter builds");
    engine
        .synthesize(TtsRequest::new(text, VOICE, deadline))
        .await
}

#[tokio::test]
async fn conforms_to_the_tts_port_over_the_serving_contract() {
    let endpoint = Endpoint::start().await;

    tts_conformance(&endpoint.engine()).await;

    let bodies = endpoint.bodies();
    assert!(!bodies.is_empty(), "the suite reached the endpoint");
    let request_schema = schema("request");
    for body in &bodies {
        assert!(
            request_schema.is_valid(body),
            "a request breaks tts.json: {body}"
        );
        let records = body["dataframe_records"].as_array().expect("records");
        assert_eq!(records.len(), 1, "one clause per request");
        assert_eq!(records[0]["encoding"], "pcm_s16le");
        assert_eq!(records[0]["sample_rate"], 24000);
        assert_eq!(records[0]["voice"], VOICE);
    }
}

#[tokio::test]
async fn the_audio_comes_back_as_exactly_the_samples_sent() {
    let samples = [0_i16, 1, -1, 1_000, -1_000, i16::MAX, i16::MIN, 12_345];
    let server = answering(
        200,
        json!({"predictions": [{"audio_b64": pcm_b64(&samples), "encoding": "pcm_s16le", "sample_rate": 24000}]}),
    )
    .await;

    let speech = synthesize(server.uri(), "Hello there.", DEADLINE)
        .await
        .expect("the clause is spoken");

    assert_eq!(speech.sample_rate(), 24_000);
    assert_eq!(speech.samples(), samples);
}

#[tokio::test]
async fn text_voice_and_speed_go_in_one_record() {
    let endpoint = Endpoint::start().await;

    endpoint
        .engine()
        .synthesize(TtsRequest::new("Good evening.", "bm_george", DEADLINE).with_speed(1.25))
        .await
        .expect("the clause is spoken");

    let body = endpoint.bodies().pop().expect("a request");
    let record = &body["dataframe_records"][0];
    assert_eq!(record["text"], "Good evening.");
    assert_eq!(record["voice"], "bm_george");
    assert_eq!(record["speed"], 1.25);
}

#[tokio::test]
async fn text_the_contract_forbids_is_refused_before_any_request() {
    let endpoint = Endpoint::start().await;
    let engine = endpoint.engine();
    let too_long = "a".repeat(1_001);

    for text in [" \n ", too_long.as_str()] {
        let error = engine
            .synthesize(TtsRequest::new(text, VOICE, DEADLINE))
            .await
            .expect_err("forbidden text fails");
        assert!(
            !error.is_retryable(),
            "{} characters: {error:?}",
            text.len()
        );
    }
    assert!(endpoint.bodies().is_empty(), "nothing reached the endpoint");
}

#[tokio::test]
async fn a_bearer_token_is_sent_only_when_configured() {
    let endpoint = Endpoint::start().await;
    let with_token = ServingTts::new(
        ServingTtsConfig::new(endpoint.server.uri(), "tts")
            .with_bearer_token(SecretString::from("test-token")),
    )
    .expect("the adapter builds");

    with_token
        .synthesize(TtsRequest::new("Hello.", VOICE, DEADLINE))
        .await
        .expect("spoken");
    endpoint
        .engine()
        .synthesize(TtsRequest::new("Hello.", VOICE, DEADLINE))
        .await
        .expect("spoken");

    let requests = endpoint
        .server
        .received_requests()
        .await
        .expect("recording");
    let auth: Vec<Option<String>> = requests
        .iter()
        .map(|request| {
            request
                .headers
                .get("authorization")
                .map(|value| value.to_str().expect("ascii").to_owned())
        })
        .collect();
    assert_eq!(auth, [Some("Bearer test-token".to_owned()), None]);
}

/// `(status, error_code, the expected error, whether it is retryable)`.
type ErrorCase = (u16, &'static str, fn(&ProviderError) -> bool, bool);

#[tokio::test]
async fn serving_errors_map_to_provider_errors() {
    let cases: [ErrorCase; 8] = [
        (
            400,
            "BAD_REQUEST",
            |e| matches!(e, ProviderError::Protocol(_)),
            false,
        ),
        (
            401,
            "BAD_REQUEST",
            |e| matches!(e, ProviderError::Auth),
            false,
        ),
        (
            403,
            "BAD_REQUEST",
            |e| matches!(e, ProviderError::Auth),
            false,
        ),
        (
            404,
            "RESOURCE_DOES_NOT_EXIST",
            |e| matches!(e, ProviderError::Protocol(_)),
            false,
        ),
        (
            429,
            "REQUEST_LIMIT_EXCEEDED",
            |e| matches!(e, ProviderError::RateLimited { .. }),
            true,
        ),
        (500, "INTERNAL_ERROR", |_| true, false),
        (
            503,
            "TEMPORARILY_UNAVAILABLE",
            |e| matches!(e, ProviderError::Unavailable),
            true,
        ),
        (
            504,
            "DEADLINE_EXCEEDED",
            |e| matches!(e, ProviderError::Timeout),
            true,
        ),
    ];
    for (status, code, expected, retryable) in cases {
        let server = answering(status, json!({"error_code": code, "message": "failed"})).await;
        let error = synthesize(server.uri(), "Hello.", DEADLINE)
            .await
            .expect_err("an error status fails the call");
        assert!(expected(&error), "{status} {code} became {error:?}");
        assert_eq!(
            error.is_retryable(),
            retryable,
            "{status} {code}: {error:?}"
        );
    }
}

#[tokio::test]
async fn audio_in_another_format_is_a_protocol_error_never_relabelled() {
    let tone = pcm_b64(&[1_000, -1_000, 1_000, -1_000]);
    let cases = [
        (
            "22.05 kHz audio",
            json!({"predictions": [{"audio_b64": tone, "encoding": "pcm_s16le", "sample_rate": 22050}]}),
        ),
        (
            "an odd byte count",
            json!({"predictions": [{"audio_b64": BASE64.encode([1_u8, 2, 3]), "encoding": "pcm_s16le", "sample_rate": 24000}]}),
        ),
        (
            "bad base64",
            json!({"predictions": [{"audio_b64": "@@@@", "encoding": "pcm_s16le", "sample_rate": 24000}]}),
        ),
        ("no prediction", json!({"predictions": []})),
        (
            "no audio",
            json!({"predictions": [{"encoding": "pcm_s16le", "sample_rate": 24000}]}),
        ),
    ];
    for (what, body) in cases {
        let server = answering(200, body).await;
        let error = synthesize(server.uri(), "Hello.", DEADLINE)
            .await
            .expect_err("a broken answer fails the call");
        assert!(
            matches!(error, ProviderError::Protocol(_)),
            "{what} became {error:?}"
        );
        assert!(!error.is_retryable(), "{what}");
    }
}

#[tokio::test]
async fn a_slow_endpoint_times_out_at_the_deadline() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(INVOCATIONS))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"predictions": [{"audio_b64": pcm_b64(&[1, 2]), "encoding": "pcm_s16le", "sample_rate": 24000}]}))
                .set_delay(Duration::from_secs(5)),
        )
        .mount(&server)
        .await;

    let started = Instant::now();
    let error = synthesize(server.uri(), "Hello.", Duration::from_millis(300))
        .await
        .expect_err("a late answer fails the call");

    assert_eq!(error, ProviderError::Timeout);
    assert!(error.is_retryable());
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "the call gave up after {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn an_unreachable_endpoint_is_unavailable() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
    let base_url = format!("http://{}", listener.local_addr().expect("an address"));
    drop(listener);

    let error = synthesize(base_url, "Hello.", DEADLINE)
        .await
        .expect_err("nothing listens there");

    assert_eq!(error, ProviderError::Unavailable);
    assert!(error.is_retryable());
}

#[tokio::test]
#[ignore = "live: needs the local stack (just dev); run with just adapters-live"]
async fn live_conforms_against_the_local_model_shim() {
    let base_url = std::env::var("VAK_TTS_BASE_URL").expect("VAK_TTS_BASE_URL is set");
    let endpoint = std::env::var("VAK_TTS_ENDPOINT").unwrap_or_else(|_| "tts".to_owned());
    let engine =
        ServingTts::new(ServingTtsConfig::new(base_url, endpoint)).expect("the adapter builds");

    tts_conformance(&engine).await;
}
