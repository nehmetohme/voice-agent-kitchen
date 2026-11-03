//! Contract `voice-core` v1 (STT adapter): `vak-adapter-stt-serving` speaks the Databricks
//! serving contract `contracts/serving/stt.json` and passes the shared `SttEngine` suite
//! (`vak_testkit::conformance::stt_conformance`), against a wiremock stand-in here and
//! against the local model shim in the ignored live test.
//!
//! What it pins:
//! - It passes the conformance suite against an endpoint that answers only schema-valid
//!   requests: one record per utterance, `pcm_s16le` at 16 kHz, the language hint when given,
//!   the audio as base64 of the samples' little-endian bytes, at
//!   `POST {base_url}/serving-endpoints/{endpoint}/invocations`.
//! - `Authorization: Bearer <token>` goes out only when a token is configured.
//! - Serving errors map to `ProviderError`: 400 and 404 `Protocol`, 401 and 403 `Auth`, 429
//!   `RateLimited`, 503 `Unavailable` and 504 `Timeout` (those three retryable), 500 not
//!   retryable; an answer that breaks the schema is `Protocol`.
//! - A slow endpoint fails with `Timeout` at the request's deadline; an unreachable one with
//!   `Unavailable`.
//! - Live (`#[ignore]`, `just dev`): the suite passes against `VAK_STT_BASE_URL`.
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
use vak_adapter_stt_serving::{ServingStt, ServingSttConfig};
use vak_providers::{ProviderError, SttEngine, SttRequest};
use vak_testkit::conformance::stt_conformance;
use vak_testkit::fixtures;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const INVOCATIONS: &str = "/serving-endpoints/stt/invocations";
const DEADLINE: Duration = Duration::from_secs(5);

/// A validator for `contracts/serving/stt.json`'s `$defs/<def>`.
fn schema(def: &str) -> jsonschema::Validator {
    let path = format!(
        "{}/../../../../contracts/serving/stt.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"));
    let mut root: Value = serde_json::from_str(&text).expect("the schema is JSON");
    root["$ref"] = json!(format!("#/$defs/{def}"));
    jsonschema::validator_for(&root).unwrap_or_else(|e| panic!("stt.json compiles: {e}"))
}

/// A wiremock STT endpoint. It refuses (400) any body that breaks the request schema, and
/// answers each record with the fixture's words when its audio has sound and with "" when it
/// is silent. It records every body it received.
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
        let words = fixtures::utterance_book_table().transcript().to_owned();
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
                        let audio = BASE64
                            .decode(record["audio_b64"].as_str().expect("audio"))
                            .expect("base64");
                        let sound = audio.iter().any(|byte| *byte != 0);
                        json!({"text": if sound { words.as_str() } else { "" }})
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

    fn engine(&self) -> ServingStt {
        ServingStt::new(ServingSttConfig::new(self.server.uri(), "stt"))
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

async fn transcribe(base_url: String, deadline: Duration) -> Result<String, ProviderError> {
    let engine =
        ServingStt::new(ServingSttConfig::new(base_url, "stt")).expect("the adapter builds");
    let audio = fixtures::utterance_book_table();
    engine
        .transcribe(SttRequest::new(audio.samples(), deadline))
        .await
        .map(|transcript| transcript.text().to_owned())
}

#[tokio::test]
async fn conforms_to_the_stt_port_over_the_serving_contract() {
    let endpoint = Endpoint::start().await;

    stt_conformance(&endpoint.engine()).await;

    let bodies = endpoint.bodies();
    assert!(!bodies.is_empty(), "the suite reached the endpoint");
    let request_schema = schema("request");
    for body in &bodies {
        assert!(
            request_schema.is_valid(body),
            "a request breaks stt.json: {body:.200}"
        );
        let records = body["dataframe_records"].as_array().expect("records");
        assert_eq!(records.len(), 1, "one utterance per request");
        assert_eq!(records[0]["encoding"], "pcm_s16le");
        assert_eq!(records[0]["sample_rate"], 16000);
    }
    assert!(
        bodies
            .iter()
            .any(|body| body["dataframe_records"][0]["language"] == "en"),
        "the language hint is sent when given"
    );
}

#[tokio::test]
async fn the_utterance_travels_as_base64_little_endian_pcm() {
    let endpoint = Endpoint::start().await;
    let audio = fixtures::utterance_book_table();

    endpoint
        .engine()
        .transcribe(SttRequest::new(audio.samples(), DEADLINE))
        .await
        .expect("the utterance transcribes");

    let body = endpoint.bodies().pop().expect("a request");
    let sent = BASE64
        .decode(
            body["dataframe_records"][0]["audio_b64"]
                .as_str()
                .expect("audio"),
        )
        .expect("standard base64");
    let expected: Vec<u8> = audio
        .samples()
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    assert_eq!(sent, expected, "audio_b64 is the samples, s16le, no header");
    assert!(
        body["dataframe_records"][0].get("language").is_none(),
        "no language hint unless one is given"
    );
}

#[tokio::test]
async fn a_bearer_token_is_sent_only_when_configured() {
    let endpoint = Endpoint::start().await;
    let audio = fixtures::utterance_book_table();
    let with_token = ServingStt::new(
        ServingSttConfig::new(endpoint.server.uri(), "stt")
            .with_bearer_token(SecretString::from("test-token")),
    )
    .expect("the adapter builds");

    with_token
        .transcribe(SttRequest::new(audio.samples(), DEADLINE))
        .await
        .expect("transcribes");
    endpoint
        .engine()
        .transcribe(SttRequest::new(audio.samples(), DEADLINE))
        .await
        .expect("transcribes");

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
        let error = transcribe(server.uri(), DEADLINE)
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
async fn an_answer_that_breaks_the_contract_is_a_protocol_error() {
    for body in [
        json!({"predictions": []}),
        json!({"predictions": [{"words": "hi"}]}),
        json!({"answer": "hi"}),
    ] {
        let server = answering(200, body.clone()).await;
        let error = transcribe(server.uri(), DEADLINE)
            .await
            .expect_err("a broken answer fails the call");
        assert!(
            matches!(error, ProviderError::Protocol(_)),
            "{body} became {error:?}"
        );
        assert!(!error.is_retryable());
    }
}

#[tokio::test]
async fn a_slow_endpoint_times_out_at_the_deadline() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(INVOCATIONS))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"predictions": [{"text": "late"}]}))
                .set_delay(Duration::from_secs(5)),
        )
        .mount(&server)
        .await;

    let started = Instant::now();
    let error = transcribe(server.uri(), Duration::from_millis(300))
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

    let error = transcribe(base_url, DEADLINE)
        .await
        .expect_err("nothing listens there");

    assert_eq!(error, ProviderError::Unavailable);
    assert!(error.is_retryable());
}

#[tokio::test]
#[ignore = "live: needs the local stack (just dev); run with just adapters-live"]
async fn live_conforms_against_the_local_model_shim() {
    let base_url = std::env::var("VAK_STT_BASE_URL").expect("VAK_STT_BASE_URL is set");
    let endpoint = std::env::var("VAK_STT_ENDPOINT").unwrap_or_else(|_| "stt".to_owned());
    let engine =
        ServingStt::new(ServingSttConfig::new(base_url, endpoint)).expect("the adapter builds");

    stt_conformance(&engine).await;
}
