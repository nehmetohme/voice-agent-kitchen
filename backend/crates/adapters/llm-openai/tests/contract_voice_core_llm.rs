//! Contract `voice-core` v1 (LLM adapter): `vak-adapter-llm-openai` streams OpenAI-compatible
//! chat completions and passes the shared `LlmEngine` suites
//! (`vak_testkit::conformance::{llm_conformance, llm_tool_conformance}`), against a wiremock
//! stand-in that replays streams recorded from the local LLM (`backend/fixtures/llm/`), and
//! against the LAN LLM in the ignored live test.
//!
//! What it pins:
//! - It passes both suites against the recorded streams.
//! - The request: `POST {base_url}/chat/completions` with `model`, `stream: true`, the
//!   messages as `{role, content}`, `max_tokens`, the tools in function format, the configured
//!   extra body merged in (thinking off), and `Authorization: Bearer <key>` only when a key is
//!   configured. An extra body that is not a JSON object is refused when the adapter is built.
//! - The reply streams: every content delta becomes its own `Text` event, in order, then one
//!   `Done`; a tool call split over several deltas becomes one complete `ToolCall`.
//! - Errors: 401 and 403 `Auth`, 429 `RateLimited` with its `Retry-After`, 500, 502 and 503
//!   `Unavailable` (retryable), 400 and 404 `Protocol`; a malformed event is `Protocol`.
//! - No first event before the request's deadline is `Timeout`; a stream that then stalls
//!   past the idle timeout is `Timeout`; dropping the stream closes the connection, which
//!   frees a one-slot server; an unreachable server is `Unavailable`.
//! - Live (`#[ignore]`, LAN LLM): both suites pass against `VAK_LLM_BASE_URL`.
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

use secrecy::SecretString;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use vak_adapter_llm_openai::{OpenAiLlm, OpenAiLlmConfig};
use vak_providers::{LlmEngine, LlmEvent, LlmRequest, LlmStream, ProviderError, Role, ToolSpec};
use vak_testkit::conformance::{llm_conformance, llm_tool_conformance};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const COMPLETIONS: &str = "/v1/chat/completions";
const MODEL: &str = "test-model";
const DEADLINE: Duration = Duration::from_secs(5);

fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../../../fixtures/llm/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"))
}

/// The non-empty content deltas of a recorded stream, in order.
fn content_deltas(stream: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(stream)
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter(|data| *data != "[DONE]")
        .filter_map(|data| {
            let chunk: Value = serde_json::from_str(data).expect("a recorded chunk is JSON");
            chunk["choices"][0]["delta"]["content"]
                .as_str()
                .filter(|content| !content.is_empty())
                .map(str::to_owned)
        })
        .collect()
}

fn sse(body: Vec<u8>) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_bytes(body)
}

/// A wiremock chat-completions server that replays the recorded tool call when the request
/// offers tools and the recorded text reply otherwise. It records every body it received.
struct Server {
    mock: MockServer,
    bodies: Arc<Mutex<Vec<Value>>>,
}

impl Server {
    async fn start() -> Self {
        let mock = MockServer::start().await;
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let seen = bodies.clone();
        let text = fixture("text_reply.sse");
        let tool = fixture("tool_call.sse");
        Mock::given(method("POST"))
            .and(path(COMPLETIONS))
            .respond_with(move |request: &Request| {
                let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
                seen.lock().expect("unpoisoned").push(body.clone());
                let offers_tools = body["tools"]
                    .as_array()
                    .is_some_and(|tools| !tools.is_empty());
                sse(if offers_tools {
                    tool.clone()
                } else {
                    text.clone()
                })
            })
            .mount(&mock)
            .await;
        Self { mock, bodies }
    }

    fn base_url(&self) -> String {
        format!("{}/v1", self.mock.uri())
    }

    fn engine(&self) -> OpenAiLlm {
        OpenAiLlm::new(
            OpenAiLlmConfig::new(self.base_url(), MODEL)
                .with_extra_body(json!({"chat_template_kwargs": {"enable_thinking": false}})),
        )
        .expect("the adapter builds")
    }

    fn bodies(&self) -> Vec<Value> {
        self.bodies.lock().expect("unpoisoned").clone()
    }
}

fn question() -> LlmRequest {
    LlmRequest::new(DEADLINE)
        .with_message(Role::System, "You are a phone agent for a restaurant.")
        .with_message(Role::User, "Hi, are you open tonight?")
        .with_max_tokens(40)
}

async fn all_events(engine: &OpenAiLlm, request: LlmRequest) -> Vec<LlmEvent> {
    let mut stream = engine.stream(request).await.expect("the stream starts");
    let mut events = Vec::new();
    while let Some(event) = stream.next_event().await {
        events.push(event.expect("the stream does not fail"));
    }
    events
}

/// The error a request ends in, whether the request itself or its first event fails.
async fn first_error(engine: &OpenAiLlm, request: LlmRequest) -> ProviderError {
    match engine.stream(request).await {
        Err(error) => error,
        Ok(mut stream) => loop {
            match stream.next_event().await {
                Some(Err(error)) => break error,
                Some(Ok(_)) => {}
                None => panic!("the stream ended without the expected error"),
            }
        },
    }
}

#[tokio::test]
async fn conforms_to_the_llm_port_with_recorded_streams() {
    let server = Server::start().await;
    let engine = server.engine();

    llm_conformance(&engine).await;
    llm_tool_conformance(&engine).await;

    let bodies = server.bodies();
    assert!(!bodies.is_empty(), "the suites reached the server");
    for body in &bodies {
        assert_eq!(body["model"], MODEL);
        assert_eq!(body["stream"], true);
        assert_eq!(body["chat_template_kwargs"]["enable_thinking"], false);
        for message in body["messages"].as_array().expect("messages") {
            assert!(message["role"].is_string() && message["content"].is_string());
        }
    }
}

#[tokio::test]
async fn the_request_is_a_streaming_chat_with_tools_and_the_extra_body() {
    let server = Server::start().await;
    let engine = OpenAiLlm::new(
        OpenAiLlmConfig::new(server.base_url(), MODEL)
            .with_bearer_token(SecretString::from("test-key"))
            .with_extra_body(json!({"chat_template_kwargs": {"enable_thinking": false}})),
    )
    .expect("the adapter builds");
    let request = question().with_tool(
        ToolSpec::new("mark_do_not_call", "End the call and never call again.")
            .with_parameters_json(r#"{"type":"object","properties":{"reason":{"type":"string"}}}"#),
    );

    all_events(&engine, request).await;

    let requests = server.mock.received_requests().await.expect("recording");
    let sent = requests.first().expect("a request");
    assert_eq!(sent.url.path(), COMPLETIONS);
    assert_eq!(
        sent.headers
            .get("authorization")
            .map(|value| value.to_str().expect("ascii")),
        Some("Bearer test-key")
    );
    let body: Value = serde_json::from_slice(&sent.body).expect("a JSON body");
    assert_eq!(body["model"], MODEL);
    assert_eq!(body["stream"], true);
    assert_eq!(body["max_tokens"], 40);
    assert_eq!(
        body["messages"],
        json!([
            {"role": "system", "content": "You are a phone agent for a restaurant."},
            {"role": "user", "content": "Hi, are you open tonight?"}
        ])
    );
    assert_eq!(
        body["tools"],
        json!([{
            "type": "function",
            "function": {
                "name": "mark_do_not_call",
                "description": "End the call and never call again.",
                "parameters": {"type": "object", "properties": {"reason": {"type": "string"}}}
            }
        }])
    );
    assert_eq!(
        body["chat_template_kwargs"],
        json!({"enable_thinking": false})
    );
}

#[tokio::test]
async fn no_authorization_header_without_a_key_and_no_extra_body_that_is_not_an_object() {
    let server = Server::start().await;
    let engine =
        OpenAiLlm::new(OpenAiLlmConfig::new(server.base_url(), MODEL)).expect("the adapter builds");

    all_events(&engine, question()).await;

    let requests = server.mock.received_requests().await.expect("recording");
    assert!(requests[0].headers.get("authorization").is_none());
    assert!(
        OpenAiLlm::new(OpenAiLlmConfig::new(server.base_url(), MODEL).with_extra_body(json!([1])))
            .is_err(),
        "an extra body that is not an object is refused"
    );
}

#[tokio::test]
async fn every_content_delta_streams_as_its_own_text_event() {
    let server = Server::start().await;
    let deltas = content_deltas(&fixture("text_reply.sse"));

    let events = all_events(&server.engine(), question()).await;

    let texts: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            LlmEvent::Text(text) => Some(text.as_str().to_owned()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, deltas, "one Text event per delta, in order");
    assert_eq!(
        texts.concat(),
        "Yes, we are open tonight until 10 PM. Would you like to make a reservation?"
    );
    assert!(matches!(events.last(), Some(LlmEvent::Done)));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, LlmEvent::Done))
            .count(),
        1
    );
}

#[tokio::test]
async fn a_tool_call_split_over_deltas_arrives_complete() {
    let server = Server::start().await;
    let request = question().with_tool(ToolSpec::new(
        "mark_do_not_call",
        "Record that the caller asked not to be contacted again, and end the call.",
    ));

    let events = all_events(&server.engine(), request).await;

    let calls: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            LlmEvent::ToolCall(call) => Some(call),
            _ => None,
        })
        .collect();
    assert_eq!(calls.len(), 1, "one tool call");
    assert_eq!(calls[0].id(), "CSmArN5Z41GcseGT2vT3sA5sKQYgiOw2");
    assert_eq!(calls[0].name(), "mark_do_not_call");
    assert_eq!(calls[0].arguments(), "{}");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, LlmEvent::Text(_))),
        "a null content is no text"
    );
    assert!(matches!(events.last(), Some(LlmEvent::Done)));
}

/// `(status, the expected error, whether it is retryable)`.
type ErrorCase = (u16, fn(&ProviderError) -> bool, bool);

#[tokio::test]
async fn http_errors_map_to_provider_errors() {
    let cases: [ErrorCase; 8] = [
        (400, |e| matches!(e, ProviderError::Protocol(_)), false),
        (401, |e| matches!(e, ProviderError::Auth), false),
        (403, |e| matches!(e, ProviderError::Auth), false),
        (404, |e| matches!(e, ProviderError::Protocol(_)), false),
        (
            429,
            |e| {
                matches!(
                    e,
                    ProviderError::RateLimited {
                        retry_after: Some(after)
                    } if *after == Duration::from_secs(2)
                )
            },
            true,
        ),
        (500, |e| matches!(e, ProviderError::Unavailable), true),
        (502, |e| matches!(e, ProviderError::Unavailable), true),
        (503, |e| matches!(e, ProviderError::Unavailable), true),
    ];
    for (status, expected, retryable) in cases {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(COMPLETIONS))
            .respond_with(
                ResponseTemplate::new(status)
                    .insert_header("retry-after", "2")
                    .set_body_json(json!({"error": {"message": "failed"}})),
            )
            .mount(&mock)
            .await;
        let engine = OpenAiLlm::new(OpenAiLlmConfig::new(format!("{}/v1", mock.uri()), MODEL))
            .expect("the adapter builds");

        let error = first_error(&engine, question()).await;

        assert!(expected(&error), "{status} became {error:?}");
        assert_eq!(error.is_retryable(), retryable, "{status}: {error:?}");
    }
}

#[tokio::test]
async fn a_malformed_event_is_a_protocol_error() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(COMPLETIONS))
        .respond_with(sse(b"data: {\"choices\": [ not json\n\n".to_vec()))
        .mount(&mock)
        .await;
    let engine = OpenAiLlm::new(OpenAiLlmConfig::new(format!("{}/v1", mock.uri()), MODEL))
        .expect("the adapter builds");

    let error = first_error(&engine, question()).await;

    assert!(matches!(error, ProviderError::Protocol(_)), "{error:?}");
}

#[tokio::test]
async fn no_first_event_before_the_deadline_is_a_timeout() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(COMPLETIONS))
        .respond_with(sse(fixture("text_reply.sse")).set_delay(Duration::from_secs(5)))
        .mount(&mock)
        .await;
    let engine = OpenAiLlm::new(OpenAiLlmConfig::new(format!("{}/v1", mock.uri()), MODEL))
        .expect("the adapter builds");
    let request = LlmRequest::new(Duration::from_millis(300)).with_message(Role::User, "Hi");

    let started = Instant::now();
    let error = first_error(&engine, request).await;

    assert_eq!(error, ProviderError::Timeout);
    assert!(error.is_retryable());
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "gave up after {:?}",
        started.elapsed()
    );
}

/// A one-connection HTTP server that answers with an event stream holding one text delta,
/// then sends nothing more and keeps the connection open. `closed` fires when the client
/// hangs up.
struct StallingServer {
    base_url: String,
    closed: oneshot::Receiver<()>,
}

async fn stalling_server() -> StallingServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a free port");
    let address = listener.local_addr().expect("an address");
    let (closed_tx, closed) = oneshot::channel();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("a connection");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        // The request head, then its body (Content-Length bytes).
        loop {
            let read = socket.read(&mut buffer).await.expect("readable");
            assert!(read > 0, "the client hung up before sending a request");
            request.extend_from_slice(&buffer[..read]);
            let text = String::from_utf8_lossy(&request).to_string();
            if let Some(head_end) = text.find("\r\n\r\n") {
                let length = text[..head_end]
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if request.len() >= head_end + 4 + length {
                    break;
                }
            }
        }
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\nconnection: close\r\n\r\n\
                  data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
            )
            .await
            .expect("writable");
        socket.flush().await.expect("flushed");
        // Hold the connection open until the client closes it.
        while matches!(socket.read(&mut buffer).await, Ok(read) if read > 0) {}
        let _ = closed_tx.send(());
    });
    StallingServer {
        base_url: format!("http://{address}/v1"),
        closed,
    }
}

async fn first_text(stream: &mut Box<dyn LlmStream>) {
    match stream.next_event().await {
        Some(Ok(LlmEvent::Text(text))) => assert_eq!(text.as_str(), "Hello"),
        other => panic!("expected the first text delta, got {other:?}"),
    }
}

#[tokio::test]
async fn a_stream_that_stalls_past_the_idle_timeout_is_a_timeout() {
    let server = stalling_server().await;
    let engine = OpenAiLlm::new(
        OpenAiLlmConfig::new(server.base_url, MODEL).with_idle_timeout(Duration::from_millis(300)),
    )
    .expect("the adapter builds");
    let mut stream = engine.stream(question()).await.expect("the stream starts");
    first_text(&mut stream).await;

    let started = Instant::now();
    let next = stream.next_event().await;

    assert!(
        matches!(next, Some(Err(ProviderError::Timeout))),
        "a stalled stream should time out, got {next:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "gave up after {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn dropping_the_stream_closes_the_connection() {
    let server = stalling_server().await;
    let engine =
        OpenAiLlm::new(OpenAiLlmConfig::new(server.base_url, MODEL)).expect("the adapter builds");
    let mut stream = engine.stream(question()).await.expect("the stream starts");
    first_text(&mut stream).await;

    drop(stream);

    tokio::time::timeout(Duration::from_secs(2), server.closed)
        .await
        .expect("the server sees the connection close within 2 s")
        .expect("the server reports it");
}

#[tokio::test]
async fn an_unreachable_server_is_unavailable() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
    let base_url = format!("http://{}/v1", listener.local_addr().expect("an address"));
    drop(listener);
    let engine = OpenAiLlm::new(OpenAiLlmConfig::new(base_url, MODEL)).expect("the adapter builds");

    let error = first_error(&engine, question()).await;

    assert_eq!(error, ProviderError::Unavailable);
    assert!(error.is_retryable());
}

#[tokio::test]
#[ignore = "live: needs the LAN LLM (VAK_LLM_BASE_URL in .env); run with just adapters-live"]
async fn live_conforms_against_the_local_llm() {
    let base_url = std::env::var("VAK_LLM_BASE_URL").expect("VAK_LLM_BASE_URL is set");
    let model = std::env::var("VAK_LLM_MODEL").expect("VAK_LLM_MODEL is set");
    let mut config = OpenAiLlmConfig::new(base_url, model);
    if let Ok(key) = std::env::var("VAK_LLM_API_KEY")
        && !key.is_empty()
    {
        config = config.with_bearer_token(SecretString::from(key));
    }
    if let Ok(extra) = std::env::var("VAK_LLM_EXTRA_BODY")
        && !extra.is_empty()
    {
        let extra: Value = serde_json::from_str(&extra).expect("VAK_LLM_EXTRA_BODY is JSON");
        config = config.with_extra_body(extra);
    }
    let engine = OpenAiLlm::new(config).expect("the adapter builds");

    llm_conformance(&engine).await;
    llm_tool_conformance(&engine).await;
}
