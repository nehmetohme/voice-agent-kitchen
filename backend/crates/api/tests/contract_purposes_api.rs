//! Contract `purposes` v1 (HTTP): purposes, their versions, and agents that pin them.
//!
//! - `GET /purposes` lists the six built-in purposes first, in use-case order, at version 1,
//!   then the workspace's own; each with `latestVersion`, `unsavedChanges` and `usedBy`
//!   (active agents).
//! - `POST /purposes` duplicates a purpose (its latest version, or the one asked for) into a
//!   new draft without a version, named "Copy of …" unless named.
//! - `GET /purposes/{id}` answers the draft (without the flow), what keeps it from being
//!   saved (`problems`), and per shipped jurisdiction the pack's baseline with the effective
//!   policy. `PATCH` autosaves whole sections, validated field by field (422 with paths such
//!   as `tools.http[0].url`); a no-op changes and audits nothing. Built-in purposes are
//!   read-only (409).
//! - `GET`/`PUT /purposes/{id}/flow` read and replace the starting flow in the agent plan's
//!   shape; it is checked by the graph rules and opens with no locked text of its own.
//! - `POST /purposes/{id}/versions` freezes the draft as the next version: 409 when nothing
//!   changed, 422 naming each problem.
//! - `GET /catalog/tools` lists the built-in tools and the tool host allow-list.
//! - `POST /agents` with only `useCase` pins the system purpose v1; with `purposeId`, that
//!   purpose's latest version (`useCase` must be its class). `PATCH /agents/{id}` switches
//!   purpose. `GET /agents/{id}/purpose` reports a newer version as an `update` with what
//!   changes and what happens to the plan; `POST …/purpose/update` moves the agent and keeps an
//!   edited plan unless `replacePlan`.
//! - Every route needs the forwarded identity; unknown IDs are 404; every mutation is audited.
//!
//! Contract file: never edit it once it is tagged `contract/purposes/v1`. To change the
//! contract, add a new version.
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
use vak_domain::{AuditAction, DraftField, ToolHosts};
use vak_testkit::repositories::{InMemoryAgentRepo, InMemoryPurposeRepo};

const USER: &str = "7364019283746501";
const EMAIL: &str = "ada@example.com";
/// A UUID v7 that no test creates.
const UNKNOWN_ID: &str = "01920000-0000-7000-8000-00000000beef";
const SUPPORT: &str = "019a0000-0000-7000-8000-000000000001";
const BOOKING: &str = "019a0000-0000-7000-8000-000000000003";
const SYSTEM: [(&str, &str, &str); 6] = [
    (SUPPORT, "Customer support", "support"),
    (
        "019a0000-0000-7000-8000-000000000002",
        "Customer service",
        "service",
    ),
    (BOOKING, "Appointment booking", "booking"),
    ("019a0000-0000-7000-8000-000000000004", "Survey", "survey"),
    (
        "019a0000-0000-7000-8000-000000000005",
        "Outbound sales",
        "telemarketing",
    ),
    (
        "019a0000-0000-7000-8000-000000000006",
        "Something else",
        "custom",
    ),
];
const ALLOWED_HOST: &str = "calendar.example.com";

struct Reply {
    status: StatusCode,
    content_type: Option<String>,
    location: Option<String>,
    body: Value,
}

struct Api {
    app: Router,
    agents: Arc<InMemoryAgentRepo>,
    purposes: Arc<InMemoryPurposeRepo>,
}

#[derive(Debug, Clone, Copy)]
enum Identity {
    Operator,
    Anonymous,
}

fn request(method: Method, uri: &str, identity: Identity, json: Option<String>) -> Request<Body> {
    let builder = Request::builder().method(method).uri(uri);
    let builder = match identity {
        Identity::Operator => builder
            .header("x-forwarded-user", USER)
            .header("x-forwarded-email", EMAIL),
        Identity::Anonymous => builder,
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
        let agents = Arc::new(InMemoryAgentRepo::default());
        let purposes = Arc::new(InMemoryPurposeRepo::default());
        let service = vak_services::AgentService::new(agents.clone())
            .with_purposes(purposes.clone())
            .with_tool_hosts(ToolHosts::new([ALLOWED_HOST, "*.tools.test"]).expect("valid"));
        let app = vak_api::app(vak_api::AppState::new(service));
        Self {
            app,
            agents,
            purposes,
        }
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

    async fn send(&self, method: Method, uri: &str, body: Option<Value>) -> Reply {
        let body = body.map(|json| json.to_string());
        self.call(request(method, uri, Identity::Operator, body))
            .await
    }

    async fn get(&self, uri: &str) -> Value {
        let reply = self.send(Method::GET, uri, None).await;
        assert_eq!(reply.status, StatusCode::OK, "GET {uri}: {}", reply.body);
        reply.body
    }

    /// A duplicate of `source` named `name`; answers its ID.
    async fn duplicate(&self, source: &str, name: &str) -> String {
        let reply = self
            .send(
                Method::POST,
                "/api/v1/purposes",
                Some(json!({ "duplicateOf": source, "name": name })),
            )
            .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
        reply.body["id"].as_str().expect("an id").to_owned()
    }

    async fn patch_purpose(&self, id: &str, body: Value) -> Reply {
        self.send(Method::PATCH, &format!("/api/v1/purposes/{id}"), Some(body))
            .await
    }

    async fn save_version(&self, id: &str) -> Reply {
        self.send(
            Method::POST,
            &format!("/api/v1/purposes/{id}/versions"),
            None,
        )
        .await
    }

    /// A new agent from `body`; answers its JSON.
    async fn create_agent(&self, body: Value) -> Value {
        let reply = self.send(Method::POST, "/api/v1/agents", Some(body)).await;
        assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
        reply.body
    }

    fn purpose_actions(&self) -> Vec<&'static str> {
        self.purposes
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
}

/// The `field`s of a 422 problem.
fn fields(reply: &Reply) -> Vec<String> {
    assert_problem(reply, StatusCode::UNPROCESSABLE_ENTITY);
    reply.body["errors"]
        .as_array()
        .expect("errors")
        .iter()
        .map(|error| error["field"].as_str().expect("field").to_owned())
        .collect()
}

fn items(body: &Value) -> &Vec<Value> {
    body["items"].as_array().expect("items")
}

/// The listed purpose with `id`.
fn listed<'a>(list: &'a Value, id: &str) -> &'a Value {
    items(list)
        .iter()
        .find(|item| item["id"] == id)
        .expect("the purpose is listed")
}

fn booking_tool(url: &str) -> Value {
    json!({
        "name": "book_slot",
        "description": "Book a free slot in the clinic's calendar.",
        "inputs": {
            "type": "object",
            "properties": {
                "date": { "type": "string", "description": "The day, as YYYY-MM-DD" },
                "people": { "type": "integer", "description": "How many come" },
            },
            "required": ["date"],
        },
        "method": "POST",
        "url": url,
        "timeoutMs": 4000,
        "secret": "CALENDAR_API_KEY",
        "confirm": true,
    })
}

fn front_desk() -> Value {
    json!({ "name": "Front desk", "useCase": "support", "voice": "af_heart" })
}

// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn the_six_built_in_purposes_are_listed_first_at_version_1() {
    let api = Api::new();

    let list = api.get("/api/v1/purposes").await;

    let shown: Vec<(&str, &str, &str)> = items(&list)
        .iter()
        .map(|item| {
            (
                item["id"].as_str().expect("id"),
                item["name"].as_str().expect("name"),
                item["complianceClass"].as_str().expect("class"),
            )
        })
        .collect();
    assert_eq!(shown, SYSTEM);
    for item in items(&list) {
        assert_eq!(item["system"], true, "{item}");
        assert_eq!(item["latestVersion"], 1, "{item}");
        assert_eq!(item["unsavedChanges"], false, "{item}");
        assert_eq!(item["usedBy"], 0, "{item}");
        assert!(item["description"].is_string(), "{item}");
    }
}

#[tokio::test]
async fn an_agent_created_with_only_a_use_case_pins_its_system_purpose() {
    let api = Api::new();

    let agent = api.create_agent(front_desk()).await;

    assert_eq!(
        agent["purpose"],
        json!({ "id": SUPPORT, "version": 1, "system": true })
    );
    assert_eq!(agent["useCase"], "support");
    let id = agent["id"].as_str().expect("id");
    let pinned = api.get(&format!("/api/v1/agents/{id}/purpose")).await;
    assert_eq!(
        pinned,
        json!({
            "id": SUPPORT, "version": 1, "name": "Customer support", "system": true,
            "latestVersion": 1, "update": null,
        })
    );
    let list = api.get("/api/v1/agents").await;
    assert_eq!(
        items(&list)[0]["purpose"],
        json!({ "id": SUPPORT, "version": 1, "system": true })
    );
    let purposes = api.get("/api/v1/purposes").await;
    assert_eq!(listed(&purposes, SUPPORT)["usedBy"], 1);
    assert_eq!(listed(&purposes, BOOKING)["usedBy"], 0);
    api.send(Method::DELETE, &format!("/api/v1/agents/{id}"), None)
        .await;
    let purposes = api.get("/api/v1/purposes").await;
    assert_eq!(
        listed(&purposes, SUPPORT)["usedBy"],
        0,
        "archived agents don't count"
    );
}

#[tokio::test]
async fn a_built_in_purpose_shows_its_pack_baseline_and_is_read_only() {
    let api = Api::new();

    let support = api.get(&format!("/api/v1/purposes/{SUPPORT}")).await;

    assert_eq!(support["id"], SUPPORT);
    assert_eq!(support["system"], true);
    assert_eq!(support["latestVersion"], 1);
    assert_eq!(support["unsavedChanges"], false);
    assert_eq!(support["problems"], json!([]));
    let draft = &support["draft"];
    assert_eq!(draft["name"], "Customer support");
    assert_eq!(draft["complianceClass"], "support");
    assert_eq!(
        draft["persona"],
        "Your job: answer customers' questions and help solve their problems."
    );
    assert_eq!(
        draft["tools"],
        json!({ "builtins": ["mark_do_not_call"], "http": [] })
    );
    assert_eq!(
        draft["guardrails"],
        json!({
            "allowedTopics": [], "never": [], "escalations": [], "maxCallMinutes": 30,
            "neverAsk": [],
        })
    );
    assert_eq!(
        draft["compliance"],
        json!({
            "optOutPhrases": [], "forbiddenClaims": [], "recordingsDays": null,
            "transcriptsDays": null,
        })
    );
    let baselines = support["baselines"].as_array().expect("baselines");
    let jurisdictions: Vec<&str> = baselines
        .iter()
        .map(|baseline| baseline["jurisdiction"].as_str().expect("jurisdiction"))
        .collect();
    assert_eq!(jurisdictions, ["US", "US-CA"]);
    let us = &baselines[0];
    assert_eq!(us["rulePack"], json!({ "id": "us-support", "version": 1 }));
    assert_eq!(us["lockedTexts"][0]["key"], "ai_disclosure");
    assert!(us["optOutConfirmation"].is_string());
    assert_eq!(us["retention"]["auditDays"], 1826);
    assert_eq!(us["effective"]["optOutPhrases"], us["optOutPhrases"]);
    assert_eq!(us["effective"]["forbiddenClaims"], us["forbiddenClaims"]);
    assert_eq!(us["effective"]["retention"], us["retention"]);

    let flow = api.get(&format!("/api/v1/purposes/{SUPPORT}/flow")).await;
    assert_eq!(flow["saved"], true);
    assert_eq!(
        flow["rulePack"],
        json!({ "id": "us-support", "version": 1 })
    );
    assert_eq!(flow["lockedTexts"], us["lockedTexts"]);
    assert_eq!(flow["validation"], json!({ "valid": true, "errors": [] }));
    let start = &flow["nodes"][0];
    assert_eq!(start["kind"], "start");
    assert_eq!(
        start["text"], "Thanks for calling. How can I help you today?",
        "the flow carries no locked text of its own"
    );

    let edit = api
        .patch_purpose(SUPPORT, json!({ "description": "Mine now" }))
        .await;
    assert_problem(&edit, StatusCode::CONFLICT);
    let flow_edit = api
        .send(
            Method::PUT,
            &format!("/api/v1/purposes/{SUPPORT}/flow"),
            Some(json!({ "nodes": flow["nodes"], "edges": flow["edges"] })),
        )
        .await;
    assert_problem(&flow_edit, StatusCode::CONFLICT);
    assert_problem(&api.save_version(SUPPORT).await, StatusCode::CONFLICT);
    assert_eq!(api.purpose_actions(), [] as [&str; 0]);
}

#[tokio::test]
async fn duplicating_makes_a_draft_without_a_version() {
    let api = Api::new();

    let reply = api
        .send(
            Method::POST,
            "/api/v1/purposes",
            Some(json!({ "duplicateOf": BOOKING, "name": "  Clinic bookings " })),
        )
        .await;

    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    let id = reply.body["id"].as_str().expect("id").to_owned();
    assert_eq!(id.chars().nth(14), Some('7'), "UUID v7: {id}");
    assert_eq!(
        reply.location.as_deref(),
        Some(format!("/api/v1/purposes/{id}").as_str())
    );
    let copy = &reply.body;
    assert_eq!(copy["system"], false);
    assert_eq!(copy["latestVersion"], Value::Null);
    assert_eq!(copy["unsavedChanges"], true);
    assert_eq!(copy["usedBy"], 0);
    let original = api.get(&format!("/api/v1/purposes/{BOOKING}")).await;
    let mut expected = original["draft"].clone();
    expected["name"] = json!("Clinic bookings");
    assert_eq!(copy["draft"], expected);
    assert_eq!(api.get(&format!("/api/v1/purposes/{id}")).await, *copy);
    assert_eq!(api.purpose_actions(), ["purpose.created"]);

    let unnamed = api
        .send(
            Method::POST,
            "/api/v1/purposes",
            Some(json!({ "duplicateOf": SUPPORT })),
        )
        .await;
    assert_eq!(unnamed.status, StatusCode::CREATED, "{}", unnamed.body);
    assert_eq!(unnamed.body["draft"]["name"], "Copy of Customer support");

    let list = api.get("/api/v1/purposes").await;
    let ids: Vec<&str> = items(&list)
        .iter()
        .map(|item| item["id"].as_str().expect("id"))
        .collect();
    assert_eq!(ids.len(), 8);
    assert_eq!(
        ids.get(6..),
        Some([unnamed.body["id"].as_str().expect("id"), id.as_str()].as_slice()),
        "the workspace's own after the built-ins, newest first"
    );
    assert_eq!(listed(&list, &id)["latestVersion"], Value::Null);
    assert_eq!(listed(&list, &id)["unsavedChanges"], true);

    let no_version = api
        .send(
            Method::POST,
            "/api/v1/agents",
            Some(json!({
                "name": "Clinic desk", "useCase": "booking", "voice": "af_heart", "purposeId": id,
            })),
        )
        .await;
    assert_eq!(fields(&no_version), ["purposeId"]);
    for bad in [
        json!({ "duplicateOf": UNKNOWN_ID }),
        json!({ "duplicateOf": "not-a-uuid" }),
        json!({}),
    ] {
        let refused = api.send(Method::POST, "/api/v1/purposes", Some(bad)).await;
        assert_eq!(fields(&refused), ["duplicateOf"]);
    }
    let unnamed_version = api
        .send(
            Method::POST,
            "/api/v1/purposes",
            Some(json!({ "duplicateOf": SUPPORT, "version": 9 })),
        )
        .await;
    assert_eq!(fields(&unnamed_version), ["version"]);
    let blank = api
        .send(
            Method::POST,
            "/api/v1/purposes",
            Some(json!({ "duplicateOf": SUPPORT, "name": " " })),
        )
        .await;
    assert_eq!(fields(&blank), ["name"]);
}

#[tokio::test]
async fn tools_autosave_and_a_host_off_the_allow_list_is_a_problem() {
    let api = Api::new();
    let id = api.duplicate(BOOKING, "Clinic bookings").await;

    let tools = api
        .patch_purpose(
            &id,
            json!({ "tools": {
                "builtins": ["transfer_to_human"],
                "http": [booking_tool("https://calendar.example.com/v1/slots")],
            } }),
        )
        .await;

    assert_eq!(tools.status, StatusCode::OK, "{}", tools.body);
    let saved = &tools.body["draft"]["tools"];
    assert_eq!(
        saved["builtins"],
        json!(["transfer_to_human", "mark_do_not_call"])
    );
    assert_eq!(saved["http"][0]["name"], "book_slot");
    assert_eq!(saved["http"][0]["timeoutMs"], 4000);
    assert_eq!(saved["http"][0]["secret"], "CALENDAR_API_KEY");
    assert_eq!(saved["http"][0]["confirm"], true);
    assert_eq!(
        saved["http"][0]["inputs"]["properties"]["date"],
        json!({ "type": "string", "description": "The day, as YYYY-MM-DD" })
    );
    assert_eq!(saved["http"][0]["inputs"]["required"], json!(["date"]));
    assert_eq!(tools.body["problems"], json!([]));

    let elsewhere = api
        .patch_purpose(
            &id,
            json!({ "tools": { "builtins": [], "http": [booking_tool("https://evil.test/x")] } }),
        )
        .await;
    assert_eq!(elsewhere.status, StatusCode::OK, "{}", elsewhere.body);
    assert_eq!(
        elsewhere.body["draft"]["tools"]["builtins"],
        json!(["mark_do_not_call"]),
        "the opt-out tool is always on"
    );
    let problems = elsewhere.body["problems"].as_array().expect("problems");
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0]["code"], "host_not_allowed");
    assert_eq!(problems[0]["field"], "tools.http[0].url");
    assert!(
        problems[0]["message"]
            .as_str()
            .expect("message")
            .contains("evil.test")
    );
    let wildcard = api
        .patch_purpose(
            &id,
            json!({ "tools": { "builtins": [], "http": [booking_tool("https://eu.tools.test/x")] } }),
        )
        .await;
    assert_eq!(wildcard.body["problems"], json!([]));

    let mut bad_name = booking_tool("https://calendar.example.com/x");
    bad_name["name"] = json!("Book slot!");
    let mut bad_url = booking_tool("http://calendar.example.com/x");
    bad_url["name"] = json!("plain_http");
    let mut slow = booking_tool("https://calendar.example.com/x");
    slow["name"] = json!("slow");
    slow["timeoutMs"] = json!(20_000);
    let refused = api
        .patch_purpose(
            &id,
            json!({ "tools": { "builtins": ["send_fax"], "http": [bad_name, bad_url, slow] } }),
        )
        .await;
    assert_eq!(
        fields(&refused),
        [
            "tools.builtins[0]",
            "tools.http[0].name",
            "tools.http[1].url",
            "tools.http[2].timeoutMs"
        ]
    );
    assert_eq!(
        api.purpose_actions(),
        [
            "purpose.created",
            "purpose.draft_updated",
            "purpose.draft_updated",
            "purpose.draft_updated"
        ]
    );
}

#[tokio::test]
async fn guardrails_autosave_and_are_checked_field_by_field() {
    let api = Api::new();
    let id = api.duplicate(BOOKING, "Clinic bookings").await;

    let guardrails = api
        .patch_purpose(
            &id,
            json!({ "guardrails": {
                "allowedTopics": ["Booking appointments"],
                "never": ["Give medical advice"],
                "escalations": [
                    { "when": "Does the caller ask for a human?", "action": "transfer" },
                    { "when": "Is the caller abusive?", "action": "end_call",
                      "message": "I'm ending this call now. Goodbye." },
                ],
                "maxCallMinutes": 10,
                "neverAsk": ["card_number", "national_id"],
            } }),
        )
        .await;
    assert_eq!(guardrails.status, StatusCode::OK, "{}", guardrails.body);
    assert_eq!(
        guardrails.body["draft"]["guardrails"]["escalations"],
        json!([
            { "when": "Does the caller ask for a human?", "action": "transfer", "message": null },
            { "when": "Is the caller abusive?", "action": "end_call",
              "message": "I'm ending this call now. Goodbye." },
        ])
    );
    let bad_guardrails = api
        .patch_purpose(
            &id,
            json!({ "guardrails": {
                "allowedTopics": [], "never": [],
                "escalations": [{ "when": " ", "action": "transfer" }],
                "maxCallMinutes": 45, "neverAsk": ["shoe_size"],
            } }),
        )
        .await;
    assert_eq!(
        fields(&bad_guardrails),
        [
            "guardrails.escalations[0].when",
            "guardrails.maxCallMinutes",
            "guardrails.neverAsk[0]"
        ]
    );

    assert_eq!(
        api.purpose_actions(),
        ["purpose.created", "purpose.draft_updated"]
    );
}

#[tokio::test]
async fn compliance_additions_and_the_overview_autosave_and_tighten_the_pack() {
    let api = Api::new();
    let id = api.duplicate(BOOKING, "Clinic bookings").await;

    let compliance = api
        .patch_purpose(
            &id,
            json!({
                "compliance": {
                    "optOutPhrases": ["Leave me alone"], "forbiddenClaims": ["Promising a refund."],
                    "recordingsDays": 30, "transcriptsDays": null,
                },
                "description": "Clinic appointments.",
                "persona": "Your job: book clinic appointments.",
            }),
        )
        .await;
    assert_eq!(compliance.status, StatusCode::OK, "{}", compliance.body);
    let draft = &compliance.body["draft"];
    assert_eq!(
        draft["compliance"]["optOutPhrases"],
        json!(["leave me alone"])
    );
    assert_eq!(draft["description"], "Clinic appointments.");
    let us = &compliance.body["baselines"][0];
    assert_eq!(us["effective"]["retention"]["recordingsDays"], 30);
    assert_eq!(
        us["effective"]["retention"]["auditDays"],
        us["retention"]["auditDays"]
    );
    assert_eq!(
        us["effective"]["optOutPhrases"]
            .as_array()
            .expect("phrases")
            .last(),
        Some(&json!("leave me alone"))
    );
    let audit_floor = api
        .patch_purpose(
            &id,
            json!({ "compliance": {
                "optOutPhrases": [], "forbiddenClaims": [], "recordingsDays": 0,
                "transcriptsDays": null, "auditDays": 1,
            } }),
        )
        .await;
    assert_problem(&audit_floor, StatusCode::UNPROCESSABLE_ENTITY);

    let before = api.purpose_actions().len();
    let same = api
        .patch_purpose(&id, json!({ "description": "Clinic appointments." }))
        .await;
    assert_eq!(same.status, StatusCode::OK);
    assert_eq!(
        api.purpose_actions().len(),
        before,
        "a no-op is not audited"
    );
    let renamed = api
        .patch_purpose(&id, json!({ "name": "", "complianceClass": "sales" }))
        .await;
    assert_eq!(fields(&renamed), ["name", "complianceClass"]);
    assert_eq!(
        api.purpose_actions(),
        ["purpose.created", "purpose.draft_updated"]
    );
    let missing = api
        .patch_purpose(UNKNOWN_ID, json!({ "description": "Nobody" }))
        .await;
    assert_problem(&missing, StatusCode::NOT_FOUND);
}

/// A flow with one step that leads nowhere.
fn flow_with_dead_end(flow: &Value) -> Value {
    let mut nodes = flow["nodes"].as_array().expect("nodes").clone();
    nodes.push(json!({ "id": "dead-end", "kind": "say", "label": "Dead end", "text": "Hm." }));
    let mut edges = flow["edges"].as_array().expect("edges").clone();
    edges.push(
        json!({ "id": "to-dead-end", "from": "start", "to": "dead-end", "condition": "Never" }),
    );
    json!({ "nodes": nodes, "edges": edges })
}

#[tokio::test]
async fn the_flow_is_edited_like_a_plan_and_must_be_valid_to_save_a_version() {
    let api = Api::new();
    let id = api.duplicate(BOOKING, "Clinic bookings").await;
    let uri = format!("/api/v1/purposes/{id}/flow");
    let flow = api.get(&uri).await;

    let broken = api
        .send(Method::PUT, &uri, Some(flow_with_dead_end(&flow)))
        .await;

    assert_eq!(broken.status, StatusCode::OK, "{}", broken.body);
    assert_eq!(broken.body["validation"]["valid"], false);
    let errors = broken.body["validation"]["errors"]
        .as_array()
        .expect("errors");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0]["code"], "missing_exit");
    assert_eq!(errors[0]["nodeId"], "dead-end");
    let purpose = api.get(&format!("/api/v1/purposes/{id}")).await;
    assert_eq!(purpose["problems"][0]["code"], "missing_exit");
    assert_eq!(purpose["problems"][0]["field"], "flow");
    assert_eq!(purpose["problems"][0]["nodeId"], "dead-end");

    let refused = api.save_version(&id).await;
    assert_eq!(fields(&refused), ["flow"]);
    let malformed = api
        .send(
            Method::PUT,
            &uri,
            Some(json!({ "nodes": [{ "id": "x" }], "edges": [] })),
        )
        .await;
    assert_eq!(fields(&malformed), ["nodes[0].kind", "nodes[0].label"]);

    let fixed = api
        .send(
            Method::PUT,
            &uri,
            Some(json!({ "nodes": flow["nodes"], "edges": flow["edges"] })),
        )
        .await;
    assert_eq!(fixed.body["validation"]["valid"], true);
    let saved = api.save_version(&id).await;
    assert_eq!(saved.status, StatusCode::CREATED, "{}", saved.body);
    assert_eq!(saved.body["latestVersion"], 1);
    assert_eq!(saved.body["unsavedChanges"], false);
    let again = api.save_version(&id).await;
    assert_problem(&again, StatusCode::CONFLICT);
    assert_eq!(
        api.purpose_actions(),
        [
            "purpose.created",
            "purpose.draft_updated",
            "purpose.draft_updated",
            "purpose.version_saved"
        ]
    );
    let missing = api
        .send(
            Method::GET,
            &format!("/api/v1/purposes/{UNKNOWN_ID}/flow"),
            None,
        )
        .await;
    assert_problem(&missing, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn versions_are_numbered_and_a_saved_one_never_changes() {
    let api = Api::new();
    let id = api.duplicate(BOOKING, "Clinic bookings").await;

    let first = api.save_version(&id).await;
    api.patch_purpose(&id, json!({ "description": "Version two." }))
        .await;
    let listed_draft = api.get("/api/v1/purposes").await;
    assert_eq!(listed(&listed_draft, &id)["unsavedChanges"], true);
    let second = api.save_version(&id).await;

    assert_eq!(first.body["latestVersion"], 1);
    assert_eq!(second.status, StatusCode::CREATED, "{}", second.body);
    assert_eq!(second.body["latestVersion"], 2);
    let versions = api.purposes.versions();
    let numbers: Vec<u32> = versions
        .iter()
        .map(|version| version.reference().version())
        .collect();
    assert_eq!(numbers, [1, 2]);
    assert_eq!(versions[0].definition().description(), "");
    assert_eq!(versions[1].definition().description(), "Version two.");
    let list = api.get("/api/v1/purposes").await;
    assert_eq!(listed(&list, &id)["latestVersion"], 2);
    assert_eq!(listed(&list, &id)["unsavedChanges"], false);
}

#[tokio::test]
async fn an_agent_created_from_a_purpose_pins_its_latest_version_and_shows_its_flow() {
    let api = Api::new();
    let id = api.duplicate(BOOKING, "Clinic bookings").await;
    let uri = format!("/api/v1/purposes/{id}/flow");
    let mut flow = api.get(&uri).await;
    flow["nodes"][0]["text"] = json!("Welcome to the clinic.");
    api.send(
        Method::PUT,
        &uri,
        Some(json!({ "nodes": flow["nodes"], "edges": flow["edges"] })),
    )
    .await;
    api.save_version(&id).await;

    let agent = api
        .create_agent(json!({
            "name": "Clinic desk", "useCase": "booking", "voice": "af_heart", "purposeId": id,
        }))
        .await;

    assert_eq!(
        agent["purpose"],
        json!({ "id": id, "version": 1, "system": false })
    );
    assert_eq!(agent["useCase"], "booking");
    assert_eq!(
        agent["rulePack"],
        json!({ "id": "us-booking", "version": 1 })
    );
    let agent_id = agent["id"].as_str().expect("id");
    let plan = api.get(&format!("/api/v1/agents/{agent_id}/plan")).await;
    assert_eq!(plan["saved"], false);
    let locked: Vec<&str> = plan["lockedTexts"]
        .as_array()
        .expect("locked texts")
        .iter()
        .map(|text| text["text"].as_str().expect("text"))
        .collect();
    assert_eq!(
        plan["nodes"][0]["text"],
        format!("{} Welcome to the clinic.", locked.join(" "))
    );
    assert_eq!(plan["validation"]["valid"], true);
    let progress = api
        .get(&format!("/api/v1/agents/{agent_id}/progress"))
        .await;
    assert_eq!(progress["percent"], 25);
    let list = api.get("/api/v1/purposes").await;
    assert_eq!(listed(&list, &id)["usedBy"], 1);

    let mismatch = api
        .send(
            Method::POST,
            "/api/v1/agents",
            Some(json!({
                "name": "Clinic desk", "useCase": "survey", "voice": "af_heart", "purposeId": id,
            })),
        )
        .await;
    assert_eq!(fields(&mismatch), ["useCase"]);
    for unknown in [UNKNOWN_ID, "not-a-uuid"] {
        let refused = api
            .send(
                Method::POST,
                "/api/v1/agents",
                Some(json!({
                    "name": "Clinic desk", "useCase": "booking", "voice": "af_heart",
                    "purposeId": unknown,
                })),
            )
            .await;
        assert_eq!(fields(&refused), ["purposeId"]);
    }
}

#[tokio::test]
async fn a_newer_version_is_an_update_that_says_what_changes() {
    let api = Api::new();
    let id = api.duplicate(BOOKING, "Clinic bookings").await;
    api.save_version(&id).await;
    let agent = api
        .create_agent(json!({
            "name": "Clinic desk", "useCase": "booking", "voice": "af_heart", "purposeId": id,
        }))
        .await;
    let agent_id = agent["id"].as_str().expect("id");
    let pinned_uri = format!("/api/v1/agents/{agent_id}/purpose");

    api.patch_purpose(
        &id,
        json!({
            "description": "Clinic appointments.",
            "guardrails": {
                "allowedTopics": ["Appointments"], "never": [], "escalations": [],
                "maxCallMinutes": 10, "neverAsk": [],
            },
        }),
    )
    .await;
    let unsaved = api.get(&pinned_uri).await;
    assert_eq!(unsaved["update"], Value::Null, "a draft is not an update");
    api.save_version(&id).await;

    let pinned = api.get(&pinned_uri).await;

    assert_eq!(pinned["id"], id.as_str());
    assert_eq!(pinned["version"], 1);
    assert_eq!(pinned["name"], "Clinic bookings");
    assert_eq!(pinned["system"], false);
    assert_eq!(pinned["latestVersion"], 2);
    let update = &pinned["update"];
    assert_eq!(update["version"], 2);
    let areas: Vec<&str> = update["changes"]
        .as_array()
        .expect("changes")
        .iter()
        .map(|change| change["area"].as_str().expect("area"))
        .collect();
    assert_eq!(areas, ["description", "guardrails"]);
    for change in update["changes"].as_array().expect("changes") {
        assert!(
            change["summary"]
                .as_str()
                .is_some_and(|text| !text.is_empty())
        );
    }
    assert_eq!(update["plan"], "follows");
    assert_eq!(update["planEdited"], false);

    let updated = api
        .send(
            Method::POST,
            &format!("{pinned_uri}/update"),
            Some(json!({ "version": 2 })),
        )
        .await;

    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    assert_eq!(updated.body["version"], 2);
    assert_eq!(updated.body["update"], Value::Null);
    let reloaded = api.get(&format!("/api/v1/agents/{agent_id}")).await;
    assert_eq!(
        reloaded["purpose"],
        json!({ "id": id, "version": 2, "system": false })
    );
    let events = api.agents.audit_events();
    let last = events.last().map(|event| event.action().clone());
    assert!(
        matches!(
            last,
            Some(AuditAction::AgentDraftUpdated { ref fields, .. }) if fields == &[DraftField::Purpose]
        ),
        "{last:?}"
    );
}

#[tokio::test]
async fn an_update_needs_another_saved_version_and_an_active_agent() {
    let api = Api::new();
    let id = api.duplicate(BOOKING, "Clinic bookings").await;
    api.save_version(&id).await;
    let agent = api
        .create_agent(json!({
            "name": "Clinic desk", "useCase": "booking", "voice": "af_heart", "purposeId": id,
        }))
        .await;
    let agent_id = agent["id"].as_str().expect("id");
    let pinned_uri = format!("/api/v1/agents/{agent_id}/purpose");
    api.patch_purpose(&id, json!({ "description": "Version two." }))
        .await;
    api.save_version(&id).await;
    api.send(
        Method::POST,
        &format!("{pinned_uri}/update"),
        Some(json!({ "version": 2 })),
    )
    .await;

    let same = api
        .send(
            Method::POST,
            &format!("{pinned_uri}/update"),
            Some(json!({ "version": 2 })),
        )
        .await;
    assert_eq!(fields(&same), ["version"]);
    let unknown = api
        .send(
            Method::POST,
            &format!("{pinned_uri}/update"),
            Some(json!({ "version": 7 })),
        )
        .await;
    assert_eq!(fields(&unknown), ["version"]);
    let missing = api
        .send(
            Method::GET,
            &format!("/api/v1/agents/{UNKNOWN_ID}/purpose"),
            None,
        )
        .await;
    assert_problem(&missing, StatusCode::NOT_FOUND);
    api.send(Method::DELETE, &format!("/api/v1/agents/{agent_id}"), None)
        .await;
    let archived = api
        .send(
            Method::POST,
            &format!("{pinned_uri}/update"),
            Some(json!({ "version": 1 })),
        )
        .await;
    assert_problem(&archived, StatusCode::CONFLICT);
}

#[tokio::test]
async fn an_edited_plan_is_kept_on_update_unless_it_is_replaced() {
    let api = Api::new();
    let id = api.duplicate(SUPPORT, "Help desk").await;
    api.save_version(&id).await;
    let agent = api
        .create_agent(json!({
            "name": "Help desk", "useCase": "support", "voice": "af_heart", "purposeId": id,
        }))
        .await;
    let agent_id = agent["id"].as_str().expect("id");
    let plan_uri = format!("/api/v1/agents/{agent_id}/plan");
    let pinned_uri = format!("/api/v1/agents/{agent_id}/purpose");
    let flow_uri = format!("/api/v1/purposes/{id}/flow");

    // Saved unchanged ("Use this plan"): the plan still follows the purpose.
    let template = api.get(&plan_uri).await;
    api.send(
        Method::PUT,
        &plan_uri,
        Some(json!({ "nodes": template["nodes"], "edges": template["edges"] })),
    )
    .await;
    let mut flow = api.get(&flow_uri).await;
    flow["nodes"][0]["text"] = json!("Version two greets you.");
    api.send(
        Method::PUT,
        &flow_uri,
        Some(json!({ "nodes": flow["nodes"], "edges": flow["edges"] })),
    )
    .await;
    api.save_version(&id).await;
    let preview = api.get(&pinned_uri).await;
    assert_eq!(preview["update"]["plan"], "replaced");
    assert_eq!(preview["update"]["planEdited"], false);
    api.send(
        Method::POST,
        &format!("{pinned_uri}/update"),
        Some(json!({ "version": 2 })),
    )
    .await;
    let followed = api.get(&plan_uri).await;
    assert_eq!(followed["saved"], true);
    assert!(
        followed["nodes"][0]["text"]
            .as_str()
            .is_some_and(|text| text.ends_with("Version two greets you."))
    );

    // Edited: kept unless replaced.
    let mut edited = followed.clone();
    edited["nodes"][1]["label"] = json!("My own step");
    api.send(
        Method::PUT,
        &plan_uri,
        Some(json!({ "nodes": edited["nodes"], "edges": edited["edges"] })),
    )
    .await;
    flow["nodes"][0]["text"] = json!("Version three greets you.");
    api.send(
        Method::PUT,
        &flow_uri,
        Some(json!({ "nodes": flow["nodes"], "edges": flow["edges"] })),
    )
    .await;
    api.save_version(&id).await;
    let preview = api.get(&pinned_uri).await;
    assert_eq!(preview["update"]["plan"], "kept");
    assert_eq!(preview["update"]["planEdited"], true);
    api.send(
        Method::POST,
        &format!("{pinned_uri}/update"),
        Some(json!({ "version": 3 })),
    )
    .await;
    let kept = api.get(&plan_uri).await;
    assert_eq!(kept["nodes"], edited["nodes"]);
    assert_eq!(api.get(&pinned_uri).await["version"], 3);

    flow["nodes"][0]["text"] = json!("Version four greets you.");
    api.send(
        Method::PUT,
        &flow_uri,
        Some(json!({ "nodes": flow["nodes"], "edges": flow["edges"] })),
    )
    .await;
    api.save_version(&id).await;
    let replaced = api
        .send(
            Method::POST,
            &format!("{pinned_uri}/update"),
            Some(json!({ "version": 4, "replacePlan": true })),
        )
        .await;
    assert_eq!(replaced.status, StatusCode::OK, "{}", replaced.body);
    let plan = api.get(&plan_uri).await;
    assert_eq!(plan["nodes"][1]["label"], template["nodes"][1]["label"]);
    assert!(
        plan["nodes"][0]["text"]
            .as_str()
            .is_some_and(|text| text.ends_with("Version four greets you."))
    );
}

#[tokio::test]
async fn patching_an_agent_switches_its_purpose() {
    let api = Api::new();
    let id = api.duplicate(BOOKING, "Clinic bookings").await;
    api.save_version(&id).await;
    let agent = api.create_agent(front_desk()).await;
    let uri = format!("/api/v1/agents/{}", agent["id"].as_str().expect("id"));

    let switched = api
        .send(Method::PATCH, &uri, Some(json!({ "purposeId": id })))
        .await;

    assert_eq!(switched.status, StatusCode::OK, "{}", switched.body);
    assert_eq!(
        switched.body["purpose"],
        json!({ "id": id, "version": 1, "system": false })
    );
    assert_eq!(switched.body["useCase"], "booking");
    assert_eq!(switched.body["rulePack"]["id"], "us-booking");
    let back = api
        .send(Method::PATCH, &uri, Some(json!({ "useCase": "survey" })))
        .await;
    assert_eq!(
        back.body["purpose"],
        json!({ "id": "019a0000-0000-7000-8000-000000000004", "version": 1, "system": true })
    );
    let unknown = api
        .send(
            Method::PATCH,
            &uri,
            Some(json!({ "purposeId": UNKNOWN_ID })),
        )
        .await;
    assert_eq!(fields(&unknown), ["purposeId"]);
}

#[tokio::test]
async fn the_tool_catalog_lists_the_built_ins_and_the_allowed_hosts() {
    let api = Api::new();

    let catalog = api.get("/api/v1/catalog/tools").await;

    let builtins = catalog["builtins"].as_array().expect("builtins");
    let names: Vec<&str> = builtins
        .iter()
        .map(|tool| tool["name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        [
            "transfer_to_human",
            "end_call",
            "record_answer",
            "schedule_callback",
            "mark_do_not_call"
        ]
    );
    for tool in builtins {
        assert!(
            tool["label"].is_string() && tool["description"].is_string(),
            "{tool}"
        );
        assert_eq!(tool["locked"], tool["name"] == "mark_do_not_call", "{tool}");
    }
    assert_eq!(
        catalog["allowedHosts"],
        json!([ALLOWED_HOST, "*.tools.test"])
    );
}

#[tokio::test]
async fn every_purpose_route_needs_the_forwarded_identity() {
    let api = Api::new();
    let purpose = format!("/api/v1/purposes/{SUPPORT}");
    let agent = format!("/api/v1/agents/{UNKNOWN_ID}");
    let routes = [
        (Method::GET, "/api/v1/purposes".to_owned()),
        (Method::POST, "/api/v1/purposes".to_owned()),
        (Method::GET, purpose.clone()),
        (Method::PATCH, purpose.clone()),
        (Method::GET, format!("{purpose}/flow")),
        (Method::PUT, format!("{purpose}/flow")),
        (Method::POST, format!("{purpose}/versions")),
        (Method::GET, "/api/v1/catalog/tools".to_owned()),
        (Method::GET, format!("{agent}/purpose")),
        (Method::POST, format!("{agent}/purpose/update")),
    ];

    for (method, uri) in routes {
        let reply = api
            .call(request(method.clone(), &uri, Identity::Anonymous, None))
            .await;
        assert_problem(&reply, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(api.purpose_actions(), [] as [&str; 0]);
}

/// Follows a local `$ref` if `schema` is one.
fn resolve<'a>(document: &'a Value, schema: &'a Value) -> &'a Value {
    match schema["$ref"].as_str() {
        Some(reference) => document
            .pointer(reference.trim_start_matches('#'))
            .expect("the $ref resolves"),
        None => schema,
    }
}

#[test]
fn the_openapi_documents_every_purpose_route_and_keeps_the_create_agent_body() {
    let document: Value = serde_json::from_str(&vak_api::openapi_json().expect("renders"))
        .expect("the document is JSON");

    let operations = [
        ("/api/v1/purposes", "get", "listPurposes"),
        ("/api/v1/purposes", "post", "createPurpose"),
        ("/api/v1/purposes/{id}", "get", "getPurpose"),
        ("/api/v1/purposes/{id}", "patch", "updatePurpose"),
        ("/api/v1/purposes/{id}/flow", "get", "getPurposeFlow"),
        ("/api/v1/purposes/{id}/flow", "put", "savePurposeFlow"),
        (
            "/api/v1/purposes/{id}/versions",
            "post",
            "savePurposeVersion",
        ),
        ("/api/v1/catalog/tools", "get", "listTools"),
        ("/api/v1/agents/{id}/purpose", "get", "getAgentPurpose"),
        (
            "/api/v1/agents/{id}/purpose/update",
            "post",
            "updateAgentPurpose",
        ),
    ];
    for (path, method, id) in operations {
        let operation = &document["paths"][path][method];
        assert_eq!(operation["operationId"], id, "{method} {path}");
        assert!(
            operation["responses"]["401"].is_object(),
            "{method} {path} documents 401"
        );
    }

    let create = resolve(
        &document,
        &document["paths"]["/api/v1/agents"]["post"]["requestBody"]["content"]["application/json"]
            ["schema"],
    );
    let required: BTreeSet<&str> = create["required"]
        .as_array()
        .expect("required")
        .iter()
        .map(|name| name.as_str().expect("name"))
        .collect();
    assert_eq!(required, BTreeSet::from(["name", "useCase", "voice"]));
    let purpose_id = &create["properties"]["purposeId"];
    assert!(purpose_id.get("default").is_some(), "{purpose_id}");
    let update = resolve(
        &document,
        &document["paths"]["/api/v1/agents/{id}"]["patch"]["requestBody"]["content"]["application/json"]
            ["schema"],
    );
    assert!(update["properties"]["purposeId"].is_object());
}
