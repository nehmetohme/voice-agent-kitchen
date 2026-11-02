//! Contract `voice-core` v1 (decision adapter): `vak-adapter-decision-systemone` speaks the
//! System One API and passes the shared `DecisionEngine` suite
//! (`vak_testkit::conformance::decision_conformance`), against a wiremock stand-in here and
//! against the local llama-server (Kev-4B) in the ignored live test.
//!
//! What it pins:
//! - It passes the conformance suite against an endpoint that answers only well-formed
//!   requests.
//! - The request: `POST {base_url}/v1/systemone` (another path when configured, e.g.
//!   Databricks' `/ai-gateway/typesafe/v1/systemone`) with `model`, a `state` of the turns as
//!   `{speaker, text}` plus `plan_node` when set, and `questions` keyed by ID: `noul` for yes/no,
//!   `choice` with `criteria` (label to description or null). `Authorization: Bearer <token>`
//!   only when configured.
//! - Answers: `noul` (or `ai_decide`'s `probability`) becomes `Answer::YesNo`; `choice` becomes
//!   `Answer::Choice` with its confidence and every option's probability. An answer that is
//!   missing, of another type, names an unknown label or is out of range is `Protocol`.
//! - Requests the API would refuse (no questions, a repeated ID) fail before any request; so
//!   do invalid questions and IDs, at construction.
//! - Errors: 400 and 422 `Protocol`, 401 and 403 `Auth`, 429 and 529 `RateLimited`, 503
//!   `Unavailable` (the last three retryable). No answer before the deadline is `Timeout`, never
//!   a no; an unreachable server is `Unavailable`.
//! - Live (`#[ignore]`, `just dev`): the suite passes against `VAK_DECISION_BASE_URL`.
//!
//! Contract file: never edit it once it is tagged `contract/voice-core/v1`. To change the
//! contract, add a new version.
#![expect(
    clippy::expect_used,
    reason = "test helpers: a failed expectation is a test failure"
)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use secrecy::SecretString;
use serde_json::{Map, Value, json};
use vak_adapter_decision_systemone::{SystemOneConfig, SystemOneDecisions};
use vak_domain::{Answer, ChoiceOption, Question, QuestionId, Speaker};
use vak_providers::{DecisionEngine, DecisionRequest, ProviderError};
use vak_testkit::conformance::decision_conformance;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const SYSTEM_ONE: &str = "/v1/systemone";
const MODEL: &str = "test-decider";
const DEADLINE: Duration = Duration::from_secs(5);

fn id(raw: &str) -> QuestionId {
    QuestionId::parse(raw).expect("a valid question ID")
}

fn intent() -> Question {
    Question::choice(
        "What does the caller want?",
        [
            ChoiceOption::new("book").with_description("book a table"),
            ChoiceOption::new("other"),
        ],
    )
    .expect("a valid choice")
}

fn opt_out() -> Question {
    Question::yes_no("Is the caller asking not to be contacted again?")
}

fn request() -> DecisionRequest {
    DecisionRequest::new(DEADLINE)
        .with_turn(Speaker::Agent, "Would you like to book a table?")
        .with_turn(Speaker::Caller, "Yes, for two.")
        .with_question(id("opt_out"), opt_out())
        .with_question(id("intent"), intent())
}

/// Why `body` is not a well-formed System One request, if it isn't.
fn malformed(body: &Value) -> Option<String> {
    if body["model"] != MODEL {
        return Some(format!("model {}", body["model"]));
    }
    let Some(turns) = body["state"]["turns"].as_array() else {
        return Some("no state.turns".to_owned());
    };
    for turn in turns {
        let speaker_ok = turn["speaker"] == "agent" || turn["speaker"] == "caller";
        if !speaker_ok || !turn["text"].is_string() {
            return Some(format!("turn {turn}"));
        }
    }
    let Some(questions) = body["questions"].as_object() else {
        return Some("no questions object".to_owned());
    };
    if questions.is_empty() {
        return Some("no questions".to_owned());
    }
    for (name, question) in questions {
        if !question["instructions"].is_string() {
            return Some(format!("{name} has no instructions"));
        }
        match question["type"].as_str() {
            Some("noul") => {}
            Some("choice") => {
                let Some(criteria) = question["criteria"].as_object() else {
                    return Some(format!("{name} has no criteria"));
                };
                if criteria.is_empty() || !criteria.values().all(|d| d.is_string() || d.is_null()) {
                    return Some(format!("{name} criteria"));
                }
            }
            other => return Some(format!("{name} type {other:?}")),
        }
    }
    None
}

/// Answers a well-formed request: `opt_out`-like yes/no questions get 0.95 when a caller turn
/// says "stop calling" and 0.02 otherwise; a choice picks `opt_out` then, else `book`, else
/// its first label, with 0.85 for the pick and the rest shared out.
fn answer(body: &Value) -> Value {
    let stop = body["state"]["turns"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|turn| {
            turn["speaker"] == "caller"
                && turn["text"]
                    .as_str()
                    .is_some_and(|text| text.to_lowercase().contains("stop calling"))
        });
    let mut answers = Map::new();
    for (name, question) in body["questions"].as_object().into_iter().flatten() {
        let value = if question["type"] == "noul" {
            json!({"type": "noul", "noul": if stop { 0.95 } else { 0.02 }})
        } else {
            let labels: Vec<&String> = question["criteria"]
                .as_object()
                .expect("criteria")
                .keys()
                .collect();
            let wanted = if stop { "opt_out" } else { "book" };
            let pick = labels
                .iter()
                .find(|label| label.as_str() == wanted)
                .or_else(|| labels.first())
                .expect("a label")
                .as_str()
                .to_owned();
            let rest = labels.len().saturating_sub(1).max(1);
            let share = 0.15 / f64::from(u32::try_from(rest).expect("few labels"));
            let probabilities: Map<String, Value> = labels
                .iter()
                .map(|label| {
                    let p = if label.as_str() == pick { 0.85 } else { share };
                    ((*label).clone(), json!(p))
                })
                .collect();
            json!({"type": "choice", "choice": pick, "confidence": 0.8, "probabilities": probabilities})
        };
        answers.insert(name.clone(), value);
    }
    json!({"model": MODEL, "answers": answers, "usage": {"input_tokens": 100, "output_tokens": 0}})
}

/// A wiremock System One endpoint at `route`: it refuses (400) malformed requests, answers
/// the rest with [`answer`], and records every body.
struct Endpoint {
    server: MockServer,
    bodies: Arc<Mutex<Vec<Value>>>,
}

impl Endpoint {
    async fn start(route: &str) -> Self {
        let server = MockServer::start().await;
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let seen = bodies.clone();
        Mock::given(method("POST"))
            .and(path(route))
            .respond_with(move |request: &Request| {
                let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
                seen.lock().expect("unpoisoned").push(body.clone());
                match malformed(&body) {
                    Some(why) => ResponseTemplate::new(400).set_body_json(
                        json!({"error": {"code": 400, "message": why, "type": "invalid_request_error"}}),
                    ),
                    None => ResponseTemplate::new(200).set_body_json(answer(&body)),
                }
            })
            .mount(&server)
            .await;
        Self { server, bodies }
    }

    fn engine(&self) -> SystemOneDecisions {
        SystemOneDecisions::new(SystemOneConfig::new(self.server.uri(), MODEL))
            .expect("the adapter builds")
    }

    fn bodies(&self) -> Vec<Value> {
        self.bodies.lock().expect("unpoisoned").clone()
    }
}

/// An engine for an endpoint that answers every request with `status` and `body`. Keep the
/// server: dropping it stops the endpoint.
async fn answering(status: u16, body: Value) -> (MockServer, SystemOneDecisions) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(SYSTEM_ONE))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(&server)
        .await;
    let engine = SystemOneDecisions::new(SystemOneConfig::new(server.uri(), MODEL))
        .expect("the adapter builds");
    (server, engine)
}

#[tokio::test]
async fn conforms_to_the_decision_port_over_system_one() {
    let endpoint = Endpoint::start(SYSTEM_ONE).await;

    decision_conformance(&endpoint.engine()).await;

    let bodies = endpoint.bodies();
    assert!(!bodies.is_empty(), "the suite reached the endpoint");
    for body in &bodies {
        assert_eq!(malformed(body), None, "a malformed request: {body}");
    }
}

#[tokio::test]
async fn the_request_carries_the_state_and_typed_questions() {
    let endpoint = Endpoint::start(SYSTEM_ONE).await;
    let engine = endpoint.engine();

    engine
        .decide(&request().with_plan_node("take_booking"))
        .await
        .expect("decided");
    engine.decide(&request()).await.expect("decided");

    let bodies = endpoint.bodies();
    assert_eq!(bodies[0]["model"], MODEL);
    assert_eq!(
        bodies[0]["state"],
        json!({
            "turns": [
                {"speaker": "agent", "text": "Would you like to book a table?"},
                {"speaker": "caller", "text": "Yes, for two."}
            ],
            "plan_node": "take_booking"
        })
    );
    assert_eq!(
        bodies[0]["questions"],
        json!({
            "opt_out": {"type": "noul", "instructions": "Is the caller asking not to be contacted again?"},
            "intent": {
                "type": "choice",
                "instructions": "What does the caller want?",
                "criteria": {"book": "book a table", "other": null}
            }
        })
    );
    assert!(
        bodies[1]["state"].get("plan_node").is_none(),
        "no plan node unless one is set"
    );
    let requests = endpoint
        .server
        .received_requests()
        .await
        .expect("recording");
    assert!(requests[0].headers.get("authorization").is_none());
}

#[tokio::test]
async fn another_path_and_a_bearer_token_when_configured() {
    let route = "/ai-gateway/typesafe/v1/systemone";
    let endpoint = Endpoint::start(route).await;
    let engine = SystemOneDecisions::new(
        SystemOneConfig::new(endpoint.server.uri(), MODEL)
            .with_path(route)
            .with_bearer_token(SecretString::from("test-token")),
    )
    .expect("the adapter builds");

    engine.decide(&request()).await.expect("decided");

    let requests = endpoint
        .server
        .received_requests()
        .await
        .expect("recording");
    let sent = requests.first().expect("a request");
    assert_eq!(sent.url.path(), route);
    assert_eq!(
        sent.headers
            .get("authorization")
            .map(|value| value.to_str().expect("ascii")),
        Some("Bearer test-token")
    );
}

#[tokio::test]
async fn answers_become_typed_answers() {
    let (_server, engine) = answering(
        200,
        json!({
            "model": "system.ai.openjev-qwen35-4b",
            "answers": {
                "opt_out": {"type": "noul", "probability": 0.7},
                "intent": {"type": "choice", "choice": "other", "confidence": 0.6,
                           "probabilities": {"book": 0.25, "other": 0.75}}
            }
        }),
    )
    .await;

    let decisions = engine.decide(&request()).await.expect("decided");

    assert_eq!(decisions.model(), "system.ai.openjev-qwen35-4b");
    match decisions.answer("opt_out") {
        Some(Answer::YesNo { probability, .. }) => assert!((probability - 0.7).abs() < 1e-9),
        other => panic!("ai_decide's probability should read as yes/no, got {other:?}"),
    }
    match decisions.answer("intent") {
        Some(Answer::Choice {
            label,
            confidence,
            probabilities,
            ..
        }) => {
            assert_eq!(label, "other");
            assert!((confidence - 0.6).abs() < 1e-9);
            let mut probabilities = probabilities.clone();
            probabilities.sort_by(|a, b| a.0.cmp(&b.0));
            assert_eq!(
                probabilities,
                [("book".to_owned(), 0.25), ("other".to_owned(), 0.75)]
            );
        }
        other => panic!("intent should be a choice, got {other:?}"),
    }
}

#[tokio::test]
async fn an_answer_that_does_not_fit_the_question_is_a_protocol_error() {
    let choice = json!({"type": "choice", "choice": "book", "confidence": 0.9,
                        "probabilities": {"book": 0.9, "other": 0.1}});
    let noul = json!({"type": "noul", "noul": 0.1});
    let cases = [
        (
            "a missing answer",
            json!({"model": MODEL, "answers": {"intent": choice}}),
        ),
        (
            "an unknown answer type",
            json!({"model": MODEL, "answers": {"opt_out": {"type": "box", "box": [0, 0, 1, 1]}, "intent": choice}}),
        ),
        (
            "an unknown label",
            json!({"model": MODEL, "answers": {"opt_out": noul, "intent": {"type": "choice", "choice": "dance", "confidence": 0.9, "probabilities": {"dance": 1.0}}}}),
        ),
        (
            "a probability above 1",
            json!({"model": MODEL, "answers": {"opt_out": {"type": "noul", "noul": 1.5}, "intent": choice}}),
        ),
        (
            "a yes/no answer to a choice",
            json!({"model": MODEL, "answers": {"opt_out": noul, "intent": noul}}),
        ),
        ("no answers at all", json!({"model": MODEL})),
    ];
    for (what, body) in cases {
        let (_server, engine) = answering(200, body).await;
        let error = engine
            .decide(&request())
            .await
            .expect_err("an unfitting answer fails the call");
        assert!(
            matches!(error, ProviderError::Protocol(_)),
            "{what} became {error:?}"
        );
        assert!(!error.is_retryable(), "{what}");
    }
}

#[tokio::test]
async fn requests_the_api_would_refuse_fail_before_sending() {
    let endpoint = Endpoint::start(SYSTEM_ONE).await;
    let engine = endpoint.engine();
    let no_questions = DecisionRequest::new(DEADLINE).with_turn(Speaker::Caller, "Hello.");
    let repeated = request().with_question(id("opt_out"), opt_out());

    for (what, request) in [("no questions", no_questions), ("a repeated ID", repeated)] {
        let error = engine.decide(&request).await.expect_err("refused");
        assert!(!error.is_retryable(), "{what}: {error:?}");
    }
    assert!(endpoint.bodies().is_empty(), "nothing reached the endpoint");

    assert!(
        Question::choice("Which?", []).is_err(),
        "a choice needs options"
    );
    assert!(
        Question::choice("Which?", [ChoiceOption::new("a"), ChoiceOption::new("a")]).is_err(),
        "choice labels are unique"
    );
    assert!(QuestionId::parse("Opt Out").is_err());
    assert!(QuestionId::parse("").is_err());
    assert_eq!(id("opt_out").as_str(), "opt_out");
}

/// `(status, the expected error, whether it is retryable)`.
type ErrorCase = (u16, fn(&ProviderError) -> bool, bool);

#[tokio::test]
async fn http_errors_map_to_provider_errors() {
    let cases: [ErrorCase; 7] = [
        (400, |e| matches!(e, ProviderError::Protocol(_)), false),
        (401, |e| matches!(e, ProviderError::Auth), false),
        (403, |e| matches!(e, ProviderError::Auth), false),
        (422, |e| matches!(e, ProviderError::Protocol(_)), false),
        (
            429,
            |e| matches!(e, ProviderError::RateLimited { .. }),
            true,
        ),
        (
            529,
            |e| matches!(e, ProviderError::RateLimited { .. }),
            true,
        ),
        (503, |e| matches!(e, ProviderError::Unavailable), true),
    ];
    for (status, expected, retryable) in cases {
        let (_server, engine) = answering(
            status,
            json!({"error": {"code": status, "message": "failed"}}),
        )
        .await;
        let error = engine
            .decide(&request())
            .await
            .expect_err("an error status fails");
        assert!(expected(&error), "{status} became {error:?}");
        assert_eq!(error.is_retryable(), retryable, "{status}: {error:?}");
    }
}

#[tokio::test]
async fn no_answer_before_the_deadline_is_a_timeout_never_a_no() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(SYSTEM_ONE))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(answer(&json!({"questions": {"opt_out": {"type": "noul"}}})))
                .set_delay(Duration::from_secs(5)),
        )
        .mount(&server)
        .await;
    let engine = SystemOneDecisions::new(SystemOneConfig::new(server.uri(), MODEL))
        .expect("the adapter builds");
    let request = DecisionRequest::new(Duration::from_millis(300))
        .with_turn(Speaker::Caller, "Please stop calling me.")
        .with_question(id("opt_out"), opt_out());

    let started = Instant::now();
    let result = engine.decide(&request).await;

    assert_eq!(
        result.expect_err("a late answer fails"),
        ProviderError::Timeout
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "gave up after {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn an_unreachable_server_is_unavailable() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
    let base_url = format!("http://{}", listener.local_addr().expect("an address"));
    drop(listener);
    let engine =
        SystemOneDecisions::new(SystemOneConfig::new(base_url, MODEL)).expect("the adapter builds");

    let error = engine
        .decide(&request())
        .await
        .expect_err("nothing listens there");

    assert_eq!(error, ProviderError::Unavailable);
    assert!(error.is_retryable());
}

#[tokio::test]
#[ignore = "live: needs the local stack (just dev); run with just adapters-live"]
async fn live_conforms_against_the_local_decision_server() {
    let base_url = std::env::var("VAK_DECISION_BASE_URL").expect("VAK_DECISION_BASE_URL is set");
    let model = std::env::var("VAK_DECISION_MODEL").expect("VAK_DECISION_MODEL is set");
    let mut config = SystemOneConfig::new(base_url, model);
    if let Ok(route) = std::env::var("VAK_DECISION_PATH")
        && !route.is_empty()
    {
        config = config.with_path(route);
    }
    let engine = SystemOneDecisions::new(config).expect("the adapter builds");

    decision_conformance(&engine).await;
}
