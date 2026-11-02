//! Voice sessions over HTTP and the media WebSocket: tickets need an active agent and
//! configured models, the WebSocket needs the subprotocol, its ticket, and its operator, and a
//! real socket carries the `vak.media.v1` conversation end to end (scripted engines).
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "helpers outside #[test] functions fail the test on a malformed response"
)]

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use futures_util::{SinkExt as _, StreamExt as _};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tower::ServiceExt as _;
use vak_services::{AgentService, Engines, VoiceService};
use vak_testkit::providers::{LlmStep, ScriptedDecisions, ScriptedLlm, ScriptedStt, ScriptedTts};
use vak_testkit::repositories::InMemoryAgentRepo;

const USER: &str = "42";
const EMAIL: &str = "ada@example.com";

struct Api {
    app: Router,
    stt: Arc<ScriptedStt>,
    llm: Arc<ScriptedLlm>,
}

fn api(with_models: bool) -> Api {
    let agents = AgentService::new(Arc::new(InMemoryAgentRepo::default()));
    let stt = Arc::new(ScriptedStt::new());
    let llm = Arc::new(ScriptedLlm::new());
    let engines = with_models.then(|| {
        Engines::new(
            stt.clone(),
            llm.clone(),
            Arc::new(ScriptedTts::new().with_delay(Duration::from_millis(5))),
            Arc::new(ScriptedDecisions::new()),
        )
    });
    let voice = VoiceService::new(agents.clone(), engines);
    let app = vak_api::app(vak_api::AppState::new(agents).with_voice(voice));
    Api { app, stt, llm }
}

async fn call(app: &Router, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("x-forwarded-user", USER)
        .header("x-forwarded-email", EMAIL);
    let request = match body {
        Some(body) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => builder.body(Body::empty()),
    }
    .expect("a request");
    let response = app.clone().oneshot(request).await.expect("answers");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("a body")
        .to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

async fn create_agent(app: &Router) -> String {
    let (status, agent) = call(
        app,
        Method::POST,
        "/api/v1/agents",
        Some(json!({"name": "Ava", "useCase": "booking", "voice": "af_heart"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    agent["id"].as_str().expect("an id").to_owned()
}

#[tokio::test]
async fn a_session_needs_an_active_agent_and_configured_models() {
    let api = api(true);
    let agent = create_agent(&api.app).await;

    let (status, session) = call(
        &api.app,
        Method::POST,
        &format!("/api/v1/agents/{agent}/sessions"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = session["id"].as_str().expect("an id");
    assert_eq!(session["mediaUrl"], format!("/api/v1/sessions/{id}/media"));

    for unknown in ["not-an-id", "01920000-0000-7000-8000-00000000beef"] {
        let (status, _) = call(
            &api.app,
            Method::POST,
            &format!("/api/v1/agents/{unknown}/sessions"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    let (status, _) = call(
        &api.app,
        Method::DELETE,
        &format!("/api/v1/agents/{agent}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, problem) = call(
        &api.app,
        Method::POST,
        &format!("/api/v1/agents/{agent}/sessions"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["status"], 409);

    let without_models = self::api(false);
    let agent = create_agent(&without_models.app).await;
    let (status, problem) = call(
        &without_models.app,
        Method::POST,
        &format!("/api/v1/agents/{agent}/sessions"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        problem["detail"],
        "Voice sessions aren't set up on this server."
    );
}

#[tokio::test]
async fn the_media_route_wants_a_websocket_and_the_latency_route_reports_every_stage() {
    let api = api(true);

    let (status, problem) = call(&api.app, Method::GET, "/api/v1/sessions/abc/media", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(problem["detail"], "Open this address as a WebSocket.");

    let (status, latency) = call(&api.app, Method::GET, "/api/v1/voice/latency", None).await;
    assert_eq!(status, StatusCode::OK);
    let stages: Vec<&str> = latency["stages"]
        .as_array()
        .expect("stages")
        .iter()
        .map(|stage| stage["stage"].as_str().expect("a name"))
        .collect();
    assert_eq!(
        stages,
        [
            "stt",
            "decision",
            "llm_ttft",
            "tts_first_audio",
            "mouth_to_ear"
        ]
    );
    assert_eq!(latency["stages"][3]["count"], 0);
    assert_eq!(latency["stages"][3]["p50Ms"], Value::Null);
    assert_eq!(latency["stages"][3]["buckets"][16]["leMs"], Value::Null);
}

/// Serves `app` on a free local port.
async fn serve(app: Router) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
    let addr = listener.local_addr().expect("an address");
    tokio::spawn(async move { axum::serve(listener, app).await });
    addr
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(
    addr: std::net::SocketAddr,
    path: &str,
    protocol: Option<&str>,
    user: &str,
) -> Result<Socket, tokio_tungstenite::tungstenite::Error> {
    connect_from(addr, path, protocol, user, None).await
}

/// Like `connect`, as a browser on the page at `origin` would.
async fn connect_from(
    addr: std::net::SocketAddr,
    path: &str,
    protocol: Option<&str>,
    user: &str,
    origin: Option<&str>,
) -> Result<Socket, tokio_tungstenite::tungstenite::Error> {
    let mut request = format!("ws://{addr}{path}")
        .into_client_request()
        .expect("a request");
    let headers = request.headers_mut();
    if let Some(origin) = origin {
        headers.insert(header::ORIGIN, origin.parse().expect("a header"));
    }
    headers.insert("x-forwarded-user", user.parse().expect("a header"));
    headers.insert("x-forwarded-email", EMAIL.parse().expect("a header"));
    if let Some(protocol) = protocol {
        headers.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            protocol.parse().expect("a header"),
        );
    }
    tokio_tungstenite::connect_async(request)
        .await
        .map(|(socket, _)| socket)
}

/// The next text event, skipping audio; panics after 10 s.
async fn next_event(socket: &mut Socket) -> Value {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(10), socket.next())
            .await
            .expect("an event within 10 s")
            .expect("the socket is open")
            .expect("a message");
        if let Message::Text(text) = message {
            return serde_json::from_str(text.as_str()).expect("JSON");
        }
    }
}

async fn until(socket: &mut Socket, kind: &str) -> Value {
    for _ in 0..1_000 {
        let event = next_event(socket).await;
        if event["type"] == kind {
            return event;
        }
    }
    panic!("no {kind}");
}

#[tokio::test]
async fn a_conversation_runs_over_the_media_websocket() {
    let api = api(true);
    api.stt
        .push_reply("Can I book a table?", Duration::from_millis(5));
    api.llm
        .push_reply([LlmStep::text("Sure, for when?", Duration::from_millis(5))]);
    let agent = create_agent(&api.app).await;
    let (_, session) = call(
        &api.app,
        Method::POST,
        &format!("/api/v1/agents/{agent}/sessions"),
        None,
    )
    .await;
    let path = session["mediaUrl"].as_str().expect("a URL").to_owned();
    let addr = serve(api.app.clone()).await;

    assert!(
        connect(addr, &path, None, USER).await.is_err(),
        "the subprotocol is required"
    );
    assert!(
        connect(addr, &path, Some("vak.media.v1"), "someone-else")
            .await
            .is_err(),
        "only the operator who started the session may open it"
    );
    let refused = connect_from(
        addr,
        &path,
        Some("vak.media.v1"),
        USER,
        Some("https://evil.example"),
    )
    .await;
    assert!(
        matches!(
            refused,
            Err(tokio_tungstenite::tungstenite::Error::Http(ref response))
                if response.status() == StatusCode::FORBIDDEN
        ),
        "a page on another site may not open the call: {refused:?}"
    );
    // The app's own page: `Origin` matches the host the handshake went to.
    let mut socket = connect_from(
        addr,
        &path,
        Some("vak.media.v1"),
        USER,
        Some(&format!("http://{addr}")),
    )
    .await
    .expect("the media socket opens");
    assert!(
        connect(addr, &path, Some("vak.media.v1"), USER)
            .await
            .is_err(),
        "a session opens once"
    );

    socket
        .send(Message::text(r#"{"type":"session.start"}"#))
        .await
        .expect("sent");
    assert_eq!(next_event(&mut socket).await["type"], "session.started");
    until(&mut socket, "agent.audio.end").await;

    // 600 ms of speech, then silence, as binary 16 kHz PCM frames, one per 20 ms.
    socket
        .send(Message::text("not a client event"))
        .await
        .expect("sent");
    assert_eq!(until(&mut socket, "error").await["code"], "bad_message");
    let speech: Vec<u8> = (0..320_i16)
        .flat_map(|i| (if (i / 16) % 2 == 0 { 8_000_i16 } else { -8_000 }).to_le_bytes())
        .collect();
    for frame in 0..60 {
        let samples = if frame < 30 {
            speech.clone()
        } else {
            vec![0; 640]
        };
        socket.send(Message::binary(samples)).await.expect("sent");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let transcript = until(&mut socket, "transcript.final").await;
    assert_eq!(transcript["speaker"], "caller");
    assert_eq!(transcript["text"], "Can I book a table?");
    let reply = until(&mut socket, "transcript.final").await;
    assert_eq!(reply["text"], "Sure, for when?");

    socket
        .send(Message::text(r#"{"type":"session.end"}"#))
        .await
        .expect("sent");
    assert_eq!(
        until(&mut socket, "session.ended").await,
        json!({"type": "session.ended", "reason": "client_ended"})
    );
}
