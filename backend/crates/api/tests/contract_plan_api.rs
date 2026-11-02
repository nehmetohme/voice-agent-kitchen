//! Contract `plan` v1 (HTTP): the agent's plan under `/api/v1/agents/{id}/plan`.
//!
//! - `GET` answers the saved plan, or the use case's template with `saved: false` until a plan
//!   is saved. With it come the rule pack it is checked against, the locked texts its start
//!   step must open with, the pack's opt-out confirmation (what the agent says, word for word,
//!   when the caller opts out), and the validation result.
//! - `PUT` replaces the plan and answers like `GET`. A plan that breaks a graph rule is saved
//!   anyway and comes back with located errors (`code`, `message`, and `nodeId` or `edgeId`,
//!   or neither for the whole plan). A malformed plan is a 422 problem naming each field, and
//!   nothing is stored. Each change writes one `agent.draft_updated` audit event for `plan`.
//! - `POST …/plan/validate` checks a plan against the agent's rule pack without saving it.
//! - Setup progress: a saved plan that validates completes Plan, so the agent is 50 % set up;
//!   a saved plan that doesn't, or one whose rule pack is unknown, leaves it at 25 %.
//! - Every route needs the forwarded identity; unknown agents are 404; archived agents'
//!   plans can be read and checked but not changed (409).
//!
//! Contract file: never edit it once it is tagged `contract/plan/v1`. To change the contract,
//! add a new version.
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
use vak_domain::{AuditAction, DraftField};
use vak_testkit::repositories::InMemoryAgentRepo;

const USER: &str = "7364019283746501";
const EMAIL: &str = "ada@example.com";
/// A UUID v7 that no test creates.
const UNKNOWN_ID: &str = "01920000-0000-7000-8000-00000000beef";
const USE_CASES: [&str; 6] = [
    "support",
    "service",
    "booking",
    "survey",
    "telemarketing",
    "custom",
];
const KINDS: [&str; 7] = ["start", "say", "ask", "collect", "tool", "transfer", "end"];

struct Reply {
    status: StatusCode,
    content_type: Option<String>,
    body: Value,
}

struct Api {
    app: Router,
    repo: Arc<InMemoryAgentRepo>,
}

/// The identity headers a request carries.
#[derive(Debug, Clone, Copy)]
enum Identity {
    Operator,
    Anonymous,
    EmailOnly,
    BadEmail,
}

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
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
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
            body,
        }
    }

    async fn send(&self, method: Method, uri: &str, body: Option<&Value>) -> Reply {
        let body = body.map(Value::to_string);
        self.call(request(method, uri, Identity::Operator, body))
            .await
    }

    /// A new agent from `body`; answers its ID.
    async fn create(&self, body: Value) -> String {
        let reply = self.send(Method::POST, "/api/v1/agents", Some(&body)).await;
        assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
        reply.body["id"].as_str().expect("an id").to_owned()
    }

    async fn create_for(&self, use_case: &str) -> String {
        self.create(json!({ "name": "Planner", "useCase": use_case, "voice": "af_heart" }))
            .await
    }

    async fn plan(&self, id: &str) -> Reply {
        self.send(Method::GET, &format!("/api/v1/agents/{id}/plan"), None)
            .await
    }

    async fn put_plan(&self, id: &str, plan: &Value) -> Reply {
        self.send(
            Method::PUT,
            &format!("/api/v1/agents/{id}/plan"),
            Some(plan),
        )
        .await
    }

    async fn validate(&self, id: &str, plan: &Value) -> Reply {
        let uri = format!("/api/v1/agents/{id}/plan/validate");
        self.send(Method::POST, &uri, Some(plan)).await
    }

    async fn percent(&self, id: &str) -> Value {
        let uri = format!("/api/v1/agents/{id}/progress");
        self.send(Method::GET, &uri, None).await.body["percent"].clone()
    }

    fn audit_actions(&self) -> Vec<&'static str> {
        self.repo
            .audit_events()
            .iter()
            .map(|event| event.action().name())
            .collect()
    }
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

fn step(id: &str, kind: &str, label: &str, text: &str) -> Value {
    json!({ "id": id, "kind": kind, "label": label, "text": text })
}

fn path(id: &str, from: &str, to: &str, condition: &str) -> Value {
    json!({ "id": id, "from": from, "to": to, "condition": condition })
}

/// The steps and paths of a plan reply, as a `PUT` body.
fn body_of(plan: &Value) -> Value {
    json!({ "nodes": plan["nodes"], "edges": plan["edges"] })
}

fn texts(list: &Value) -> Vec<String> {
    list.as_array()
        .expect("lockedTexts")
        .iter()
        .map(|locked| locked["text"].as_str().expect("text").to_owned())
        .collect()
}

/// The agent's locked texts, then `rest`: an opening that keeps the rules.
fn opening(plan: &Value, rest: &str) -> String {
    let mut parts = texts(&plan["lockedTexts"]);
    parts.push(rest.to_owned());
    parts.join(" ")
}

/// Whether `text` opens with every locked text, in order, with whitespace between.
fn opens_with(text: &str, locked: &[String]) -> bool {
    let mut rest = text.trim_start();
    for locked in locked {
        match rest.strip_prefix(locked.as_str()) {
            Some(after) => rest = after.trim_start(),
            None => return false,
        }
    }
    true
}

/// `(code, nodeId, edgeId)` of each error in `validation`; every message is plain text.
fn located(validation: &Value) -> BTreeSet<(String, Option<String>, Option<String>)> {
    let errors = validation["errors"].as_array().expect("errors");
    let set: BTreeSet<_> = errors
        .iter()
        .map(|error| {
            let message = error["message"].as_str().expect("a message");
            assert!(!message.trim().is_empty(), "{error}");
            let id = |key: &str| error[key].as_str().map(str::to_owned);
            let code = error["code"].as_str().expect("a code").to_owned();
            (code, id("nodeId"), id("edgeId"))
        })
        .collect();
    assert_eq!(set.len(), errors.len(), "no duplicate errors: {validation}");
    set
}

fn on_node(code: &str, node: &str) -> (String, Option<String>, Option<String>) {
    (code.to_owned(), Some(node.to_owned()), None)
}

fn on_edge(code: &str, edge: &str) -> (String, Option<String>, Option<String>) {
    (code.to_owned(), None, Some(edge.to_owned()))
}

fn on_plan(code: &str) -> (String, Option<String>, Option<String>) {
    (code.to_owned(), None, None)
}

/// Checks the shape of a plan reply and answers its start step.
fn start_of(plan: &Value) -> Value {
    for node in plan["nodes"].as_array().expect("nodes") {
        assert!(
            node["id"].is_string() && node["label"].is_string(),
            "{node}"
        );
        assert!(node["text"].is_string(), "{node}");
        let kind = node["kind"].as_str().expect("kind");
        assert!(KINDS.contains(&kind), "{node}");
    }
    for edge in plan["edges"].as_array().expect("edges") {
        assert!(
            edge["id"].is_string() && edge["condition"].is_string(),
            "{edge}"
        );
        assert!(edge["from"].is_string() && edge["to"].is_string(), "{edge}");
    }
    let starts: Vec<&Value> = plan["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .filter(|node| node["kind"] == "start")
        .collect();
    assert_eq!(starts.len(), 1, "exactly one start step: {plan}");
    starts[0].clone()
}

#[tokio::test]
async fn every_use_case_starts_from_a_valid_template_that_is_not_saved_yet() {
    let api = Api::new();

    for use_case in USE_CASES {
        let id = api.create_for(use_case).await;

        let reply = api.plan(&id).await;

        assert_eq!(reply.status, StatusCode::OK, "{use_case}: {}", reply.body);
        assert_eq!(reply.content_type.as_deref(), Some("application/json"));
        let plan = &reply.body;
        assert_eq!(plan["saved"], false, "{use_case}");
        assert_eq!(
            plan["rulePack"],
            json!({ "id": format!("us-{use_case}"), "version": 1 })
        );
        let keys: Vec<&str> = plan["lockedTexts"]
            .as_array()
            .expect("lockedTexts")
            .iter()
            .map(|locked| locked["key"].as_str().expect("key"))
            .collect();
        assert_eq!(
            keys.get(..2),
            Some(["ai_disclosure", "recording_notice"].as_slice())
        );
        let confirmation = plan["optOutConfirmation"]
            .as_str()
            .expect("optOutConfirmation");
        assert!(!confirmation.trim().is_empty(), "{use_case}");
        assert!(
            !texts(&plan["lockedTexts"]).contains(&confirmation.to_owned()),
            "{use_case}: the confirmation is not part of the opening"
        );
        let start = start_of(plan);
        let opening = start["text"].as_str().expect("text");
        assert!(
            opens_with(opening, &texts(&plan["lockedTexts"])),
            "{use_case}: {opening}"
        );
        assert_eq!(
            plan["validation"],
            json!({ "valid": true, "errors": [] }),
            "{use_case}"
        );
        assert_eq!(
            api.percent(&id).await,
            25,
            "reading the template saves nothing"
        );
    }
    assert_eq!(api.audit_actions(), ["agent.created"; 6]);
}

#[tokio::test]
async fn saving_a_valid_plan_completes_the_plan_step_and_is_audited_once() {
    let api = Api::new();
    let id = api.create_for("booking").await;
    let created_hash = api
        .send(Method::GET, &format!("/api/v1/agents/{id}"), None)
        .await
        .body["draftHash"]
        .clone();
    let template = api.plan(&id).await.body;

    let reply = api.put_plan(&id, &body_of(&template)).await;

    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.body["saved"], true);
    assert_eq!(reply.body["nodes"], template["nodes"]);
    assert_eq!(reply.body["edges"], template["edges"]);
    assert_eq!(
        reply.body["validation"],
        json!({ "valid": true, "errors": [] })
    );
    assert_eq!(
        api.plan(&id).await.body,
        reply.body,
        "a reload shows what was saved"
    );
    let progress = api
        .send(Method::GET, &format!("/api/v1/agents/{id}/progress"), None)
        .await;
    assert_eq!(progress.body["percent"], 50);
    assert_eq!(
        progress.body["steps"][1],
        json!({ "step": "plan", "done": true })
    );
    assert_eq!(progress.body["nextStep"], "test");
    let list = api.send(Method::GET, "/api/v1/agents", None).await;
    assert_eq!(list.body["items"][0]["progressPercent"], 50);
    let agent = api
        .send(Method::GET, &format!("/api/v1/agents/{id}"), None)
        .await;
    assert_ne!(
        agent.body["draftHash"], created_hash,
        "the plan is part of the draft"
    );

    assert_eq!(
        api.audit_actions(),
        ["agent.created", "agent.draft_updated"]
    );
    let events = api.repo.audit_events();
    let latest_event = events.last().map(vak_domain::AuditEvent::action);
    let Some(AuditAction::AgentDraftUpdated { fields, .. }) = latest_event else {
        panic!("a draft update event: {events:?}");
    };
    assert_eq!(fields, &[DraftField::Plan]);
    let again = api.put_plan(&id, &body_of(&template)).await;
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(again.body, reply.body);
    assert_eq!(
        api.audit_actions().len(),
        2,
        "saving the same plan writes no event"
    );
}

#[tokio::test]
async fn a_plan_that_breaks_the_rules_is_saved_with_each_error_located() {
    let api = Api::new();
    let id = api.create_for("support").await;
    let opening = opening(&api.plan(&id).await.body, "How can I help?");
    let plan = json!({
        "nodes": [
            step("start", "start", "Greeting", &opening),
            step("ask", "ask", "Ask what they need", "What can I help with?"),
            step("orphan", "end", "Orphan", "Goodbye."),
        ],
        "edges": [
            path("to-ask", "start", "ask", "   "),
            path("to-ghost", "start", "ghost", "The caller asks for nobody"),
        ],
    });

    let reply = api.put_plan(&id, &plan).await;

    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.body["saved"], true);
    assert_eq!(reply.body["validation"]["valid"], false);
    assert_eq!(
        located(&reply.body["validation"]),
        BTreeSet::from([
            on_node("missing_exit", "ask"),
            on_node("unreachable_node", "orphan"),
            on_edge("empty_condition", "to-ask"),
            on_edge("dangling_edge", "to-ghost"),
        ])
    );
    assert_eq!(
        reply.body["edges"][0]["condition"], "",
        "conditions are trimmed"
    );
    assert_eq!(api.plan(&id).await.body, reply.body);
    assert_eq!(
        api.percent(&id).await,
        25,
        "an invalid plan leaves Plan open"
    );
    assert_eq!(
        api.audit_actions(),
        ["agent.created", "agent.draft_updated"]
    );
}

#[tokio::test]
async fn validate_checks_a_plan_without_saving_it() {
    let api = Api::new();
    let id = api.create_for("survey").await;
    let template = api.plan(&id).await.body;
    let opening = opening(&template, "Shall we start?");
    let goodbye = step("bye", "end", "Goodbye", "Thanks, goodbye.");
    let check = |nodes: Value, edges: Value| json!({ "nodes": nodes, "edges": edges });

    let valid = api.validate(&id, &body_of(&template)).await;
    let no_start = api.validate(&id, &check(json!([goodbye]), json!([]))).await;
    let two_starts = check(
        json!([
            step("a", "start", "First", &opening),
            step("b", "start", "Second", &opening),
            goodbye
        ]),
        json!([
            path("p1", "a", "bye", "Always"),
            path("p2", "b", "bye", "Always")
        ]),
    );
    let two_starts = api.validate(&id, &two_starts).await;
    let unlocked = check(
        json!([step("start", "start", "Greeting", "Hi there!"), goodbye]),
        json!([path("p1", "start", "bye", "Always")]),
    );
    let unlocked = api.validate(&id, &unlocked).await;

    assert_eq!(valid.status, StatusCode::OK, "{}", valid.body);
    assert_eq!(valid.content_type.as_deref(), Some("application/json"));
    assert_eq!(valid.body, json!({ "valid": true, "errors": [] }));
    assert_eq!(no_start.body["valid"], false);
    assert_eq!(
        located(&no_start.body),
        BTreeSet::from([on_plan("missing_start")])
    );
    assert_eq!(
        located(&two_starts.body),
        BTreeSet::from([
            on_node("multiple_starts", "a"),
            on_node("multiple_starts", "b")
        ])
    );
    let errors = unlocked.body["errors"].as_array().expect("errors");
    assert_eq!(
        errors.len(),
        texts(&template["lockedTexts"]).len(),
        "{}",
        unlocked.body
    );
    for error in errors {
        assert_eq!(
            (&error["code"], &error["nodeId"]),
            (&json!("missing_locked_text"), &json!("start"))
        );
    }
    assert_eq!(
        api.plan(&id).await.body,
        template,
        "validating saves nothing"
    );
    assert_eq!(api.audit_actions(), ["agent.created"]);
}

/// The 422 problem's field names, after checking the problem's shape.
fn fields(reply: &Reply) -> BTreeSet<String> {
    assert_problem(reply, StatusCode::UNPROCESSABLE_ENTITY);
    reply.body["errors"]
        .as_array()
        .expect("errors")
        .iter()
        .map(|error| {
            assert!(error["message"].is_string(), "{error}");
            error["field"].as_str().expect("field").to_owned()
        })
        .collect()
}

#[tokio::test]
async fn a_malformed_plan_is_a_422_problem_that_names_each_field_and_stores_nothing() {
    let api = Api::new();
    let id = api.create_for("service").await;
    let template = api.plan(&id).await.body;
    let one = |node: Value| json!({ "nodes": [node], "edges": [] });
    let too_many: Vec<Value> = (0..51)
        .map(|i| step(&format!("s{i}"), "say", "Say", ""))
        .collect();
    let cases = [
        (one(step("a", "dance", "Dance", "")), "nodes[0].kind"),
        (one(step("a", "say", "  ", "")), "nodes[0].label"),
        (one(step("a b", "say", "Say", "")), "nodes[0].id"),
        (
            json!({ "nodes": [step("a", "say", "A", ""), step("a", "end", "B", "")], "edges": [] }),
            "nodes[1].id",
        ),
        (
            json!({ "nodes": [step("a", "say", "A", "")], "edges": [path("p", "a", "not an id", "x")] }),
            "edges[0].to",
        ),
        (json!({ "nodes": [step("a", "say", "A", "")] }), "edges"),
        (json!({ "nodes": too_many, "edges": [] }), "nodes"),
    ];

    for (plan, field) in cases {
        let put = api.put_plan(&id, &plan).await;
        assert_eq!(
            fields(&put),
            BTreeSet::from([field.to_owned()]),
            "PUT {plan}"
        );
        let validate = api.validate(&id, &plan).await;
        assert_eq!(
            fields(&validate),
            BTreeSet::from([field.to_owned()]),
            "validate {plan}"
        );
    }
    let malformed = api
        .call(request(
            Method::PUT,
            &format!("/api/v1/agents/{id}/plan"),
            Identity::Operator,
            Some("{ not json".to_owned()),
        ))
        .await;
    assert_problem(&malformed, StatusCode::BAD_REQUEST);
    assert_eq!(api.plan(&id).await.body, template, "nothing was stored");
    assert_eq!(api.audit_actions(), ["agent.created"]);
}

#[tokio::test]
async fn the_plan_is_checked_against_the_agents_current_rule_pack() {
    let api = Api::new();
    let id = api
        .create(json!({ "name": "Bookings", "useCase": "booking", "voice": "af_heart", "jurisdiction": "us-ca" }))
        .await;
    let template = api.plan(&id).await.body;
    assert_eq!(
        template["rulePack"],
        json!({ "id": "us-ca-booking", "version": 1 })
    );
    assert_eq!(template["validation"]["valid"], true);
    api.put_plan(&id, &body_of(&template)).await;
    assert_eq!(api.percent(&id).await, 50);
    let agent = format!("/api/v1/agents/{id}");

    // No rule pack exists for ZZ: the saved plan stays, but nothing can vouch for it.
    let patched = api
        .send(
            Method::PATCH,
            &agent,
            Some(&json!({ "jurisdiction": "ZZ" })),
        )
        .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);
    let unpacked = api.plan(&id).await.body;
    assert_eq!(unpacked["saved"], true);
    assert_eq!(unpacked["nodes"], template["nodes"]);
    assert_eq!(
        unpacked["rulePack"],
        json!({ "id": "zz-booking", "version": 1 })
    );
    assert_eq!(unpacked["lockedTexts"], json!([]));
    assert_eq!(unpacked["optOutConfirmation"], Value::Null);
    assert_eq!(unpacked["validation"]["valid"], false);
    assert_eq!(
        located(&unpacked["validation"]),
        BTreeSet::from([on_plan("rule_pack_unavailable")])
    );
    assert_eq!(api.percent(&id).await, 25);

    api.send(
        Method::PATCH,
        &agent,
        Some(&json!({ "jurisdiction": "US-CA" })),
    )
    .await;
    assert_eq!(api.plan(&id).await.body["validation"]["valid"], true);
    assert_eq!(api.percent(&id).await, 50);
}

#[tokio::test]
async fn an_archived_agents_plan_can_be_read_and_checked_but_not_changed() {
    let api = Api::new();
    let id = api.create_for("support").await;
    let template = api.plan(&id).await.body;
    api.send(Method::DELETE, &format!("/api/v1/agents/{id}"), None)
        .await;

    let read = api.plan(&id).await;
    let checked = api.validate(&id, &body_of(&template)).await;
    let changed = api.put_plan(&id, &body_of(&template)).await;

    assert_eq!(read.status, StatusCode::OK);
    assert_eq!(read.body, template);
    assert_eq!(checked.status, StatusCode::OK);
    assert_eq!(checked.body["valid"], true);
    assert_problem(&changed, StatusCode::CONFLICT);
    assert_eq!(api.audit_actions(), ["agent.created", "agent.archived"]);
}

#[tokio::test]
async fn every_plan_route_needs_the_forwarded_identity_and_unknown_agents_are_404s() {
    let api = Api::new();
    let id = api.create_for("custom").await;
    let signed_in = api.plan(&id).await;
    assert_eq!(signed_in.status, StatusCode::OK, "{}", signed_in.body);
    let plan = body_of(&signed_in.body);
    let routes = |agent: &str| {
        [
            (Method::GET, format!("/api/v1/agents/{agent}/plan")),
            (Method::PUT, format!("/api/v1/agents/{agent}/plan")),
            (
                Method::POST,
                format!("/api/v1/agents/{agent}/plan/validate"),
            ),
        ]
    };

    for (method, uri) in routes(&id) {
        for identity in [Identity::Anonymous, Identity::EmailOnly, Identity::BadEmail] {
            let body = Some(plan.to_string());
            let reply = api
                .call(request(method.clone(), &uri, identity, body))
                .await;
            assert_problem(&reply, StatusCode::UNAUTHORIZED);
        }
    }
    for unknown in [UNKNOWN_ID, "not-a-uuid"] {
        for (method, uri) in routes(unknown) {
            let reply = api.send(method.clone(), &uri, Some(&plan)).await;
            assert_problem(&reply, StatusCode::NOT_FOUND);
        }
    }
    assert_eq!(api.audit_actions(), ["agent.created"]);
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
fn the_openapi_document_describes_the_plan_routes() {
    let document: Value = serde_json::from_str(&vak_api::openapi_json().expect("renders"))
        .expect("the document is JSON");

    let operations = [
        ("/api/v1/agents/{id}/plan", "get"),
        ("/api/v1/agents/{id}/plan", "put"),
        ("/api/v1/agents/{id}/plan/validate", "post"),
    ];
    for (path, method) in operations {
        let operation = &document["paths"][path][method];
        assert!(
            operation["operationId"].is_string(),
            "{method} {path} is documented"
        );
        for status in ["200", "401", "404"] {
            assert!(
                operation["responses"][status].is_object(),
                "{method} {path} documents {status}"
            );
        }
    }
    let put = &document["paths"]["/api/v1/agents/{id}/plan"]["put"];
    assert!(put["responses"]["409"].is_object() && put["responses"]["422"].is_object());
    let body = resolve(
        &document,
        &put["requestBody"]["content"]["application/json"]["schema"],
    );
    let required: BTreeSet<&str> = body["required"]
        .as_array()
        .expect("required list")
        .iter()
        .map(|name| name.as_str().expect("name"))
        .collect();
    assert_eq!(required, BTreeSet::from(["edges", "nodes"]));
    let node = resolve(&document, &body["properties"]["nodes"]["items"]);
    let kinds: BTreeSet<&str> = resolve(&document, &node["properties"]["kind"])["enum"]
        .as_array()
        .expect("kind is an enum")
        .iter()
        .map(|kind| kind.as_str().expect("kind"))
        .collect();
    assert_eq!(kinds, BTreeSet::from(KINDS));
}
