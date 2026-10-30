//! Contract `agent-provisioning` v1 (HTTP): the `/api/v1` agent and catalog endpoints.
//!
//! - `POST /agents` needs only `name`, `useCase` and `voice` (≤ 3 required fields); every
//!   optional property has a default in the OpenAPI schema, and the server fills it in.
//! - `GET /agents`, `GET /agents/{id}`, `PATCH /agents/{id}` (autosave),
//!   `DELETE /agents/{id}` (archive) and `GET /agents/{id}/progress` (25 % after creation).
//! - `GET /catalog/voices` and `GET /catalog/languages`.
//! - Every `/api/v1` route answers 401 `application/problem+json` without the identity
//!   Databricks Apps forwards (`X-Forwarded-User`, `X-Forwarded-Email`); `/health` stays public.
//! - Every mutation writes one audit event naming the signed-in operator.
//!
//! Contract file: never edit it once it is tagged `contract/agent-provisioning/v1`. To change
//! the contract, add a new version.
#![expect(
    clippy::expect_used,
    reason = "helpers outside #[test] functions fail the test on a malformed response"
)]

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use tower::ServiceExt as _;
use vak_testkit::repositories::InMemoryAgentRepo;

const USER: &str = "7364019283746501";
const EMAIL: &str = "ada@example.com";
/// A UUID v7 that no test creates.
const UNKNOWN_ID: &str = "01920000-0000-7000-8000-00000000beef";

struct Reply {
    status: StatusCode,
    content_type: Option<String>,
    location: Option<String>,
    body: Value,
}

struct Api {
    app: Router,
    repo: Arc<InMemoryAgentRepo>,
}

impl Api {
    fn new() -> Self {
        let repo = Arc::new(InMemoryAgentRepo::default());
        let agents = vak_services::AgentService::new(repo.clone());
        let app = vak_api::app(vak_api::AppState::new(agents));
        Self { app, repo }
    }

    async fn call(&self, request: Request<Body>) -> Reply {
        let response = self
            .app
            .clone()
            .oneshot(request)
            .await
            .expect("the router never fails");
        let status = response.status();
        let text = |name| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        let content_type = text(header::CONTENT_TYPE);
        let location = text(header::LOCATION);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body is readable")
            .to_bytes();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("body is JSON")
        };
        Reply {
            status,
            content_type,
            location,
            body,
        }
    }

    /// A request from the signed-in operator, as Databricks Apps forwards it.
    async fn send(&self, method: Method, uri: &str, body: Option<Value>) -> Reply {
        let body = body.map(|json| json.to_string());
        self.call(request(method, uri, Identity::Operator, body))
            .await
    }

    /// A request without a body, carrying `identity`.
    async fn send_as(&self, identity: Identity, method: Method, uri: &str) -> Reply {
        self.call(request(method, uri, identity, None)).await
    }

    async fn create(&self, body: Value) -> Reply {
        self.send(Method::POST, "/api/v1/agents", Some(body)).await
    }

    fn audit_actions(&self) -> Vec<&'static str> {
        self.repo
            .audit_events()
            .iter()
            .map(|event| event.action().name())
            .collect()
    }
}

/// The identity headers a request carries.
#[derive(Debug, Clone, Copy)]
enum Identity {
    /// `X-Forwarded-User` and `X-Forwarded-Email`, as Databricks Apps sends them.
    Operator,
    /// No identity headers.
    Anonymous,
    /// Only `X-Forwarded-Email`.
    EmailOnly,
    /// Both headers, but the email is not an address.
    BadEmail,
}

/// A request with `identity`, and `json` as an `application/json` body if given.
fn request(method: Method, uri: &str, identity: Identity, json: Option<String>) -> Request<Body> {
    let builder = Request::builder().method(method).uri(uri);
    let builder = match identity {
        Identity::Operator => builder
            .header("x-forwarded-user", USER)
            .header("x-forwarded-email", EMAIL),
        Identity::Anonymous => builder,
        Identity::EmailOnly => builder.header("x-forwarded-email", EMAIL),
        Identity::BadEmail => builder
            .header("x-forwarded-user", USER)
            .header("x-forwarded-email", "not an email"),
    };
    match json {
        Some(json) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json)),
        None => builder.body(Body::empty()),
    }
    .expect("valid request")
}

fn front_desk() -> Value {
    json!({ "name": "Front desk", "useCase": "support", "voice": "af_heart" })
}

fn assert_problem(reply: &Reply, status: StatusCode) {
    assert_eq!(reply.status, status, "{}", reply.body);
    assert_eq!(
        reply.content_type.as_deref(),
        Some("application/problem+json"),
        "{}",
        reply.body
    );
    assert_eq!(reply.body["status"], json!(status.as_u16()));
    assert!(reply.body["title"].is_string(), "{}", reply.body);
}

fn id_of(reply: &Reply) -> String {
    reply.body["id"].as_str().expect("an id").to_owned()
}

#[tokio::test]
async fn create_needs_only_a_name_a_use_case_and_a_voice_and_fills_in_the_defaults() {
    let api = Api::new();

    let reply = api.create(front_desk()).await;

    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    assert_eq!(reply.content_type.as_deref(), Some("application/json"));
    let id = id_of(&reply);
    assert_eq!(id.len(), 36);
    assert_eq!(id.chars().nth(14), Some('7'), "UUID v7: {id}");
    assert_eq!(
        reply.location.as_deref(),
        Some(format!("/api/v1/agents/{id}").as_str())
    );
    let body = &reply.body;
    assert_eq!(body["name"], "Front desk");
    assert_eq!(body["useCase"], "support");
    assert_eq!(body["voice"], "af_heart");
    assert_eq!(body["language"], "en-US");
    assert_eq!(body["jurisdiction"], "US");
    assert_eq!(
        body["rulePack"],
        json!({ "id": "us-support", "version": 1 })
    );
    assert_eq!(body["model"], "balanced");
    assert_eq!(
        body["turnDetection"],
        json!({ "silenceMs": 500, "bargeIn": true })
    );
    let hash = body["draftHash"].as_str().expect("draftHash");
    assert!(hash.starts_with("sha256:") && hash.len() == 71, "{hash}");
    assert!(
        body["createdAt"]
            .as_str()
            .expect("createdAt")
            .ends_with('Z')
    );
    assert_eq!(body["createdAt"], body["updatedAt"]);
    assert_eq!(body["archivedAt"], Value::Null);
}

#[tokio::test]
async fn create_writes_one_audit_event_naming_the_signed_in_operator() {
    let api = Api::new();

    let id = id_of(&api.create(front_desk()).await);

    let events = api.repo.audit_events();
    assert_eq!(events.len(), 1);
    let event = events.first().expect("one event");
    assert_eq!(event.action().name(), "agent.created");
    assert_eq!(event.actor().user_id(), USER);
    assert_eq!(event.actor().email(), EMAIL);
    assert_eq!(event.agent_id().to_string(), id);
}

#[tokio::test]
async fn create_accepts_every_optional_field() {
    let api = Api::new();

    let reply = api
        .create(json!({
            "name": "  Bookings  ",
            "useCase": "Booking",
            "voice": "bf_emma",
            "language": "en-GB",
            "jurisdiction": "us-ca",
            "model": "fast",
            "turnDetection": { "silenceMs": 800, "bargeIn": false },
        }))
        .await;

    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    let body = &reply.body;
    assert_eq!(body["name"], "Bookings");
    assert_eq!(body["useCase"], "booking");
    assert_eq!(body["language"], "en-GB");
    assert_eq!(body["jurisdiction"], "US-CA");
    assert_eq!(
        body["rulePack"],
        json!({ "id": "us-ca-booking", "version": 1 })
    );
    assert_eq!(body["model"], "fast");
    assert_eq!(
        body["turnDetection"],
        json!({ "silenceMs": 800, "bargeIn": false })
    );
}

#[tokio::test]
async fn invalid_input_is_a_422_problem_that_names_each_field_and_stores_nothing() {
    let api = Api::new();

    let reply = api
        .create(json!({ "name": " ", "useCase": "sales", "voice": "robot_9000" }))
        .await;

    assert_problem(&reply, StatusCode::UNPROCESSABLE_ENTITY);
    let fields: BTreeSet<&str> = reply.body["errors"]
        .as_array()
        .expect("errors")
        .iter()
        .map(|error| error["field"].as_str().expect("field"))
        .collect();
    assert_eq!(fields, BTreeSet::from(["name", "useCase", "voice"]));
    assert_eq!(api.audit_actions(), [] as [&str; 0]);

    let missing = api
        .create(json!({ "name": "Front desk", "useCase": "support" }))
        .await;
    assert_problem(&missing, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(missing.body["errors"][0]["field"], "voice");

    let malformed = api
        .call(request(
            Method::POST,
            "/api/v1/agents",
            Identity::Operator,
            Some("{ not json".to_owned()),
        ))
        .await;
    assert_problem(&malformed, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_shows_active_agents_most_recently_updated_first() {
    let api = Api::new();
    let first = id_of(&api.create(front_desk()).await);
    let second = id_of(
        &api.create(json!({ "name": "Survey line", "useCase": "survey", "voice": "am_michael" }))
            .await,
    );

    let reply = api.send(Method::GET, "/api/v1/agents", None).await;

    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    let items = reply.body["items"].as_array().expect("items");
    let ids: Vec<&str> = items
        .iter()
        .map(|item| item["id"].as_str().expect("id"))
        .collect();
    assert_eq!(ids, [second.as_str(), first.as_str()]);
    let survey = items.first().expect("an item");
    assert_eq!(survey["name"], "Survey line");
    assert_eq!(survey["useCase"], "survey");
    assert_eq!(survey["voice"], "am_michael");
    assert_eq!(survey["progressPercent"], 25);
    assert!(survey["updatedAt"].is_string());

    api.send(
        Method::PATCH,
        &format!("/api/v1/agents/{first}"),
        Some(json!({ "name": "Front desk (edited)" })),
    )
    .await;
    let reordered = api.send(Method::GET, "/api/v1/agents", None).await;
    assert_eq!(reordered.body["items"][0]["id"], first.as_str());
}

#[tokio::test]
async fn get_returns_the_agent_and_unknown_ids_are_404_problems() {
    let api = Api::new();
    let created = api.create(front_desk()).await;
    let id = id_of(&created);

    let reply = api
        .send(Method::GET, &format!("/api/v1/agents/{id}"), None)
        .await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body, created.body);
    for unknown in [UNKNOWN_ID, "not-a-uuid"] {
        let missing = api
            .send(Method::GET, &format!("/api/v1/agents/{unknown}"), None)
            .await;
        assert_problem(&missing, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn a_new_agent_is_25_percent_set_up_with_plan_next() {
    let api = Api::new();
    let id = id_of(&api.create(front_desk()).await);

    let reply = api
        .send(Method::GET, &format!("/api/v1/agents/{id}/progress"), None)
        .await;

    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(
        reply.body,
        json!({
            "percent": 25,
            "steps": [
                { "step": "provision", "done": true },
                { "step": "plan", "done": false },
                { "step": "test", "done": false },
                { "step": "publish", "done": false },
            ],
            "nextStep": "plan",
        })
    );
    let missing = api
        .send(
            Method::GET,
            &format!("/api/v1/agents/{UNKNOWN_ID}/progress"),
            None,
        )
        .await;
    assert_problem(&missing, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn patch_autosaves_the_changed_fields_and_audits_each_change() {
    let api = Api::new();
    let created = api.create(front_desk()).await;
    let id = id_of(&created);
    let uri = format!("/api/v1/agents/{id}");

    let reply = api
        .send(
            Method::PATCH,
            &uri,
            Some(json!({ "name": "After hours", "model": "fast" })),
        )
        .await;

    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.body["name"], "After hours");
    assert_eq!(reply.body["model"], "fast");
    assert_eq!(reply.body["voice"], "af_heart");
    assert_ne!(reply.body["draftHash"], created.body["draftHash"]);
    let reloaded = api.send(Method::GET, &uri, None).await;
    assert_eq!(reloaded.body, reply.body);
    assert_eq!(
        api.audit_actions(),
        ["agent.created", "agent.draft_updated"]
    );

    let unchanged = api
        .send(Method::PATCH, &uri, Some(json!({ "name": "After hours" })))
        .await;
    assert_eq!(unchanged.status, StatusCode::OK);
    assert_eq!(unchanged.body, reply.body);
    assert_eq!(
        api.audit_actions().len(),
        2,
        "a no-op patch writes no event"
    );

    let invalid = api
        .send(Method::PATCH, &uri, Some(json!({ "voice": "robot_9000" })))
        .await;
    assert_problem(&invalid, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(invalid.body["errors"][0]["field"], "voice");

    let missing = api
        .send(
            Method::PATCH,
            &format!("/api/v1/agents/{UNKNOWN_ID}"),
            Some(json!({ "name": "Nobody" })),
        )
        .await;
    assert_problem(&missing, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_archives_the_agent_once() {
    let api = Api::new();
    let id = id_of(&api.create(front_desk()).await);
    let uri = format!("/api/v1/agents/{id}");

    let reply = api.send(Method::DELETE, &uri, None).await;

    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    assert_eq!(reply.body, Value::Null);
    let list = api.send(Method::GET, "/api/v1/agents", None).await;
    assert_eq!(list.body["items"], json!([]));
    let archived = api.send(Method::GET, &uri, None).await;
    assert_eq!(archived.status, StatusCode::OK);
    assert!(archived.body["archivedAt"].is_string());
    let edit = api
        .send(Method::PATCH, &uri, Some(json!({ "name": "Too late" })))
        .await;
    assert_problem(&edit, StatusCode::CONFLICT);
    let again = api.send(Method::DELETE, &uri, None).await;
    assert_eq!(again.status, StatusCode::NO_CONTENT);
    assert_eq!(api.audit_actions(), ["agent.created", "agent.archived"]);
    let missing = api
        .send(
            Method::DELETE,
            &format!("/api/v1/agents/{UNKNOWN_ID}"),
            None,
        )
        .await;
    assert_problem(&missing, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn every_api_route_needs_the_forwarded_identity_and_health_does_not() {
    let api = Api::new();
    let agent = format!("/api/v1/agents/{UNKNOWN_ID}");
    let routes = [
        (Method::POST, "/api/v1/agents".to_owned()),
        (Method::GET, "/api/v1/agents".to_owned()),
        (Method::GET, agent.clone()),
        (Method::PATCH, agent.clone()),
        (Method::DELETE, agent.clone()),
        (Method::GET, format!("{agent}/progress")),
        (Method::GET, "/api/v1/catalog/voices".to_owned()),
        (Method::GET, "/api/v1/catalog/languages".to_owned()),
    ];

    for (method, uri) in routes {
        for identity in [Identity::Anonymous, Identity::EmailOnly, Identity::BadEmail] {
            let reply = api.send_as(identity, method.clone(), &uri).await;
            assert_problem(&reply, StatusCode::UNAUTHORIZED);
        }
    }
    assert_eq!(api.audit_actions(), [] as [&str; 0]);
    let health = api
        .send_as(Identity::Anonymous, Method::GET, "/health")
        .await;
    assert_eq!(health.status, StatusCode::OK);
}

#[tokio::test]
async fn the_catalog_lists_voices_and_languages() {
    let api = Api::new();

    let voices = api.send(Method::GET, "/api/v1/catalog/voices", None).await;
    let languages = api
        .send(Method::GET, "/api/v1/catalog/languages", None)
        .await;

    assert_eq!(voices.status, StatusCode::OK);
    let voices = voices.body["items"].as_array().expect("voice items");
    assert!(!voices.is_empty() && voices.len() <= 7, "{voices:?}");
    let heart = voices
        .iter()
        .find(|voice| voice["id"] == "af_heart")
        .expect("the default voice is listed");
    assert!(heart["name"].is_string() && heart["description"].is_string());
    assert_eq!(heart["language"], "en-US");
    assert_eq!(languages.status, StatusCode::OK);
    let codes: Vec<&str> = languages.body["items"]
        .as_array()
        .expect("language items")
        .iter()
        .map(|language| language["code"].as_str().expect("code"))
        .collect();
    assert_eq!(codes.first(), Some(&"en-US"));
}

/// Follows a local `$ref` (`#/components/schemas/<name>`) if `schema` is one.
fn resolve<'a>(document: &'a Value, schema: &'a Value) -> &'a Value {
    match schema["$ref"].as_str() {
        Some(reference) => document
            .pointer(reference.trim_start_matches('#'))
            .expect("the $ref resolves"),
        None => schema,
    }
}

#[test]
fn the_openapi_create_schema_has_at_most_three_required_fields_and_defaults_for_the_rest() {
    let document: Value = serde_json::from_str(&vak_api::openapi_json().expect("renders"))
        .expect("the document is JSON");

    let operation = &document["paths"]["/api/v1/agents"]["post"];
    let schema = resolve(
        &document,
        &operation["requestBody"]["content"]["application/json"]["schema"],
    );
    let required: BTreeSet<&str> = schema["required"]
        .as_array()
        .expect("required list")
        .iter()
        .map(|name| name.as_str().expect("name"))
        .collect();
    assert_eq!(required, BTreeSet::from(["name", "useCase", "voice"]));
    let properties = schema["properties"].as_object().expect("properties");
    for (name, property) in properties {
        if !required.contains(name.as_str()) {
            assert!(
                property.get("default").is_some(),
                "optional property {name} has no default: {property}"
            );
        }
    }
    for name in ["language", "jurisdiction", "model", "turnDetection"] {
        assert!(properties.contains_key(name), "{name} is documented");
    }

    let operations = [
        ("/api/v1/agents", "post"),
        ("/api/v1/agents", "get"),
        ("/api/v1/agents/{id}", "get"),
        ("/api/v1/agents/{id}", "patch"),
        ("/api/v1/agents/{id}", "delete"),
        ("/api/v1/agents/{id}/progress", "get"),
        ("/api/v1/catalog/voices", "get"),
        ("/api/v1/catalog/languages", "get"),
    ];
    for (path, method) in operations {
        let operation = &document["paths"][path][method];
        assert!(
            operation["operationId"].is_string(),
            "{method} {path} is documented"
        );
        assert!(
            operation["responses"]["401"].is_object(),
            "{method} {path} documents 401"
        );
    }
}
