//! Contract `test-runs` v1 (HTTP): scenarios and test runs under `/api/v1`.
//!
//! - `GET /agents/{id}/scenarios` answers the agent's saved scenarios, or its use case's
//!   defaults with `saved: false`. `PUT` replaces them (`{scenarios: [...]}`), each with an
//!   `id`, `name`, `goal`, `persona`, `required`, `maxTurns` and `rubric` of `{question,
//!   expected: "yes" | "no"}`; a malformed set is a 422 naming each field (`scenarios[0].name`,
//!   `scenarios[1].rubric[0].question`, `scenarios` for the set's own rules) and nothing is
//!   stored. A change writes one `agent.draft_updated` audit event for `scenarios`.
//! - `POST /test-runs` (`{agentId}`) creates a job: 202 with `Location: /api/v1/test-runs/{id}`
//!   and the queued run. `GET /test-runs/{id}` reports its `status` (`queued`, `running`,
//!   `finished`, `cancelled`, `interrupted`), `progress` (`done`, `total`, `percent`),
//!   `verdict` (`passed`, `failed`, or `null` until finished), the `draftHash` it tested, the
//!   `models` it used (`llm`, `decisions`, `seed`), and per scenario its definition and
//!   `version`, `status`, `endReason` (`caller_hung_up`, `opt_out`, `turn_cap`, `error`),
//!   `optOutLayer`, `durationMs`, `transcript` (`speaker`, `text`) and rubric `checks`
//!   (`question`, `expected`, `probability`, `passed`).
//! - `POST /test-runs/{id}/cancel` cancels a queued or running run (again: unchanged); a
//!   finished run is a 409.
//! - `GET /agents/{id}/test-runs` lists the agent's runs, newest first, with the agent's current
//!   `draftHash` and `current` on each run that tested it.
//! - A passing run on the current draft (with a valid plan) makes setup progress 75 %; an edit
//!   makes the run stale (`current: false`) and progress 50 %.
//! - Errors: 401 without an identity; 404 for an unknown or malformed run or agent; 409 for an
//!   archived agent or one with a run in progress; 422 for a missing or malformed `agentId`;
//!   503 when the server has no models for test runs. All are `application/problem+json`.
//!
//! Contract file: never edit it once it is tagged `contract/test-runs/v1`. To change the
//! contract, add a new version.
#![expect(
    clippy::expect_used,
    clippy::too_many_lines,
    reason = "helpers outside #[test] functions fail the test on a malformed response; the job \
              test follows one run from start to stale"
)]

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use tower::ServiceExt as _;
use vak_domain::DraftField;
use vak_services::{AgentService, TestEngines, TestRunService};
use vak_testkit::providers::{LlmStep, ScriptedDecisions, ScriptedLlm};
use vak_testkit::repositories::{InMemoryAgentRepo, InMemoryTestRunRepo};

const USER: &str = "7364019283746501";
const EMAIL: &str = "ada@example.com";
const UNKNOWN_ID: &str = "01920000-0000-7000-8000-00000000beef";
const QUICK: Duration = Duration::from_millis(10);

struct Reply {
    status: StatusCode,
    content_type: Option<String>,
    location: Option<String>,
    body: Value,
}

struct Api {
    app: Router,
    agents_repo: Arc<InMemoryAgentRepo>,
    runs: TestRunService,
    llm: Arc<ScriptedLlm>,
}

fn request(method: Method, uri: &str, signed_in: bool, json: Option<String>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if signed_in {
        builder = builder
            .header("x-forwarded-user", USER)
            .header("x-forwarded-email", EMAIL);
    }
    match json {
        Some(json) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json)),
        None => builder.body(Body::empty()),
    }
    .expect("valid request")
}

impl Api {
    /// The API with test runs on, over in-memory storage and scripted models.
    fn new() -> Self {
        Self::build(true)
    }

    /// `models`: whether the test runs have models to talk to.
    fn build(models: bool) -> Self {
        let agents_repo = Arc::new(InMemoryAgentRepo::default());
        let runs_repo = Arc::new(InMemoryTestRunRepo::default());
        let agents = AgentService::new(agents_repo.clone()).with_test_runs(runs_repo.clone());
        let llm = Arc::new(ScriptedLlm::new());
        let engines = models.then(|| {
            TestEngines::new(
                llm.clone(),
                "fake-llm",
                Arc::new(ScriptedDecisions::new()),
                "fake-decisions",
            )
        });
        let runs = TestRunService::new(agents.clone(), runs_repo, engines);
        let app = vak_api::app(vak_api::AppState::new(agents).with_test_runs(runs.clone()));
        Self {
            app,
            agents_repo,
            runs,
            llm,
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

    async fn send(&self, method: Method, uri: &str, body: Option<&Value>) -> Reply {
        self.call(request(method, uri, true, body.map(Value::to_string)))
            .await
    }

    async fn agent(&self, use_case: &str) -> String {
        let body = json!({ "name": "Front desk", "useCase": use_case, "voice": "af_heart" });
        let reply = self.send(Method::POST, "/api/v1/agents", Some(&body)).await;
        assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
        reply.body["id"].as_str().expect("an id").to_owned()
    }

    /// Saves the use case's template as the plan, so the Plan step is done.
    async fn valid_plan(&self, agent: &str) {
        let uri = format!("/api/v1/agents/{agent}/plan");
        let plan = self.send(Method::GET, &uri, None).await.body;
        let body = json!({ "nodes": plan["nodes"], "edges": plan["edges"] });
        let saved = self.send(Method::PUT, &uri, Some(&body)).await;
        assert_eq!(
            saved.body["validation"]["valid"],
            json!(true),
            "{}",
            saved.body
        );
    }

    async fn put_scenarios(&self, agent: &str, scenarios: &Value) -> Reply {
        let uri = format!("/api/v1/agents/{agent}/scenarios");
        self.send(Method::PUT, &uri, Some(&json!({ "scenarios": scenarios })))
            .await
    }

    async fn start(&self, agent: &str) -> Reply {
        let body = json!({ "agentId": agent });
        self.send(Method::POST, "/api/v1/test-runs", Some(&body))
            .await
    }

    async fn run(&self, id: &str) -> Reply {
        self.send(Method::GET, &format!("/api/v1/test-runs/{id}"), None)
            .await
    }

    async fn percent(&self, agent: &str) -> Value {
        let uri = format!("/api/v1/agents/{agent}/progress");
        self.send(Method::GET, &uri, None).await.body["percent"].clone()
    }

    /// Every simulated caller of the next run hangs up on its first turn.
    fn callers_hang_up(&self, scenarios: usize) {
        for _ in 0..scenarios {
            self.llm.push_reply([
                LlmStep::text("Thanks, goodbye.", QUICK),
                LlmStep::tool_call("end_call", "{}", QUICK),
            ]);
        }
    }

    fn audit_fields(&self) -> Vec<Vec<DraftField>> {
        self.agents_repo
            .audit_events()
            .iter()
            .filter_map(|event| match event.action() {
                vak_domain::AuditAction::AgentDraftUpdated { fields, .. } => Some(fields.clone()),
                _ => None,
            })
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

fn fields(reply: &Reply) -> Vec<String> {
    reply.body["errors"]
        .as_array()
        .expect("errors")
        .iter()
        .map(|error| error["field"].as_str().expect("a field").to_owned())
        .collect()
}

/// Two scenarios the scripted models settle: the decision model answers every question with
/// 0, so "keeps it short" (expects no) passes and "books a table" (expects yes) fails.
fn two_scenarios() -> Value {
    json!([
        {
            "id": "keeps-it-short",
            "name": "  Keeps it short  ",
            "goal": "Say hello and hang up.",
            "persona": "In a hurry.",
            "required": true,
            "maxTurns": 4,
            "rubric": [{ "question": "Did the agent pressure the caller?", "expected": "no" }]
        },
        {
            "id": "books-a-table",
            "name": "Books a table",
            "goal": "Book a table for two.",
            "persona": "",
            "required": false,
            "maxTurns": 8,
            "rubric": [{ "question": "Did the agent book a table?", "expected": "yes" }]
        }
    ])
}

#[tokio::test]
async fn scenarios_are_the_defaults_until_saved() {
    let api = Api::new();
    let agent = api.agent("survey").await;
    let uri = format!("/api/v1/agents/{agent}/scenarios");

    let defaults = api.send(Method::GET, &uri, None).await;
    assert_eq!(defaults.status, StatusCode::OK, "{}", defaults.body);
    assert_eq!(defaults.body["saved"], json!(false));
    let listed = defaults.body["scenarios"].as_array().expect("scenarios");
    let ids: Vec<&str> = listed
        .iter()
        .map(|s| s["id"].as_str().expect("id"))
        .collect();
    for id in ["completes-survey", "opts-out-mid-call", "asks-to-call-back"] {
        assert!(ids.contains(&id), "{id} in {ids:?}");
    }
    for scenario in listed {
        for key in ["name", "goal", "persona"] {
            assert!(scenario[key].is_string(), "{scenario}");
        }
        assert!(scenario["required"].is_boolean());
        assert!(
            scenario["maxTurns"]
                .as_u64()
                .is_some_and(|n| (1..=20).contains(&n))
        );
        let rubric = scenario["rubric"].as_array().expect("rubric");
        assert_ne!(rubric.len(), 0, "{scenario}");
        for item in rubric {
            assert!(item["question"].is_string());
            assert!(item["expected"] == json!("yes") || item["expected"] == json!("no"));
        }
    }

    let saved = api.put_scenarios(&agent, &two_scenarios()).await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    assert_eq!(saved.body["saved"], json!(true));
    assert_eq!(
        saved.body["scenarios"][0]["name"],
        json!("Keeps it short"),
        "trimmed"
    );
    assert_eq!(
        saved.body["scenarios"][1]["rubric"][0]["expected"],
        json!("yes")
    );
    assert_eq!(api.send(Method::GET, &uri, None).await.body, saved.body);
    assert_eq!(api.audit_fields(), [vec![DraftField::Scenarios]]);
    let unchanged = api.put_scenarios(&agent, &two_scenarios()).await;
    assert_eq!(unchanged.status, StatusCode::OK);
    assert_eq!(
        api.audit_fields().len(),
        1,
        "saving the same set stores nothing"
    );
}

#[tokio::test]
async fn malformed_scenarios_name_each_field() {
    let api = Api::new();
    let agent = api.agent("booking").await;
    let base = two_scenarios();
    let with = |edit: &dyn Fn(&mut Value)| {
        let mut body = base.clone();
        edit(&mut body);
        body
    };

    let cases: Vec<(Value, Vec<&str>)> = vec![
        (
            with(&|b| b[0]["name"] = json!(" ")),
            vec!["scenarios[0].name"],
        ),
        (
            with(&|b| b[0]["id"] = json!("Not An Id")),
            vec!["scenarios[0].id"],
        ),
        (
            with(&|b| b[1]["goal"] = json!("")),
            vec!["scenarios[1].goal"],
        ),
        (
            with(&|b| b[0]["maxTurns"] = json!(0)),
            vec!["scenarios[0].maxTurns"],
        ),
        (
            with(&|b| b[0]["rubric"] = json!([])),
            vec!["scenarios[0].rubric"],
        ),
        (
            with(&|b| b[1]["rubric"][0]["question"] = json!("")),
            vec!["scenarios[1].rubric[0].question"],
        ),
        (
            with(&|b| b[1]["rubric"][0]["expected"] = json!("maybe")),
            vec!["scenarios[1].rubric[0].expected"],
        ),
        (
            with(&|b| b[1]["id"] = json!("keeps-it-short")),
            vec!["scenarios[1].id"],
        ),
        (
            with(&|b| b[0]["required"] = json!(false)),
            vec!["scenarios"],
        ),
        (json!([]), vec!["scenarios"]),
        (
            with(&|b| {
                b[0]["name"] = json!("");
                b[1]["rubric"][0]["question"] = json!("");
            }),
            vec!["scenarios[0].name", "scenarios[1].rubric[0].question"],
        ),
    ];
    for (body, expected) in cases {
        let reply = api.put_scenarios(&agent, &body).await;
        assert_problem(&reply, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(fields(&reply), expected, "{body}");
    }
    let uri = format!("/api/v1/agents/{agent}/scenarios");
    assert_eq!(
        api.send(Method::GET, &uri, None).await.body["saved"],
        json!(false),
        "nothing was stored"
    );
    assert_eq!(api.audit_fields(), Vec::<Vec<DraftField>>::new());
}

#[tokio::test]
async fn a_test_run_is_a_job_that_reports_progress_and_its_verdict() {
    let api = Api::new();
    let agent = api.agent("booking").await;
    api.valid_plan(&agent).await;
    assert_eq!(
        api.put_scenarios(&agent, &two_scenarios()).await.status,
        StatusCode::OK
    );
    assert_eq!(api.percent(&agent).await, json!(50));

    let started = api.start(&agent).await;
    assert_eq!(started.status, StatusCode::ACCEPTED, "{}", started.body);
    let run = &started.body;
    let id = run["id"].as_str().expect("an id").to_owned();
    assert_eq!(
        started.location.as_deref(),
        Some(format!("/api/v1/test-runs/{id}").as_str())
    );
    assert_eq!(run["agentId"], json!(agent));
    assert_eq!(run["status"], json!("queued"));
    assert_eq!(run["verdict"], Value::Null);
    assert_eq!(
        run["progress"],
        json!({ "done": 0, "total": 2, "percent": 0 })
    );
    assert_eq!(run["models"], Value::Null);
    assert!(
        run["draftHash"]
            .as_str()
            .is_some_and(|h| h.starts_with("sha256:"))
    );
    assert!(run["createdAt"].is_string());
    assert_eq!(run["startedAt"], Value::Null);
    assert_eq!(run["finishedAt"], Value::Null);
    let first = &run["scenarios"][0];
    assert_eq!(first["id"], json!("keeps-it-short"));
    assert_eq!(first["name"], json!("Keeps it short"));
    assert_eq!(first["goal"], json!("Say hello and hang up."));
    assert_eq!(first["required"], json!(true));
    assert_eq!(first["maxTurns"], json!(4));
    assert!(
        first["version"]
            .as_str()
            .is_some_and(|v| v.starts_with("sha256:"))
    );
    assert_eq!(first["status"], json!("pending"));
    assert_eq!(first["endReason"], Value::Null);
    assert_eq!(first["transcript"], json!([]));
    assert_eq!(first["checks"], json!([]));
    assert_eq!(
        api.run(&id).await.body,
        started.body,
        "GET reports the same job"
    );

    api.callers_hang_up(2);
    assert_eq!(api.runs.worker().drain().await, 1);

    let done = api.run(&id).await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    let run = &done.body;
    assert_eq!(run["status"], json!("finished"));
    assert_eq!(
        run["verdict"],
        json!("passed"),
        "only the optional scenario failed"
    );
    assert_eq!(
        run["progress"],
        json!({ "done": 2, "total": 2, "percent": 100 })
    );
    assert_eq!(
        run["models"],
        json!({ "llm": "fake-llm", "decisions": "fake-decisions", "seed": 1 })
    );
    assert!(run["startedAt"].is_string() && run["finishedAt"].is_string());
    let first = &run["scenarios"][0];
    assert_eq!(first["status"], json!("passed"));
    assert_eq!(first["endReason"], json!("caller_hung_up"));
    assert_eq!(first["optOutLayer"], Value::Null);
    assert!(first["durationMs"].as_u64().is_some());
    assert_eq!(
        first["transcript"],
        json!([
            {
                "speaker": "agent",
                "text": "Hi, this is Front desk, an automated AI assistant. This call may be recorded."
            },
            { "speaker": "caller", "text": "Thanks, goodbye." }
        ])
    );
    assert_eq!(
        first["checks"],
        json!([{
            "question": "Did the agent pressure the caller?",
            "expected": "no",
            "probability": 0.0,
            "passed": true
        }])
    );
    let second = &run["scenarios"][1];
    assert_eq!(second["status"], json!("failed"));
    assert_eq!(second["checks"][0]["passed"], json!(false));

    assert_eq!(api.percent(&agent).await, json!(75));
    let listed = api
        .send(
            Method::GET,
            &format!("/api/v1/agents/{agent}/test-runs"),
            None,
        )
        .await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.body);
    assert_eq!(listed.body["draftHash"], run["draftHash"]);
    assert_eq!(listed.body["items"][0]["id"], json!(id));
    assert_eq!(listed.body["items"][0]["status"], json!("finished"));
    assert_eq!(listed.body["items"][0]["verdict"], json!("passed"));
    assert_eq!(listed.body["items"][0]["current"], json!(true));
    assert_eq!(listed.body["items"][0]["progress"]["percent"], json!(100));

    // An edit makes the run stale.
    let rename = json!({ "name": "After hours" });
    let patched = api
        .send(
            Method::PATCH,
            &format!("/api/v1/agents/{agent}"),
            Some(&rename),
        )
        .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);
    assert_eq!(api.percent(&agent).await, json!(50));
    let listed = api
        .send(
            Method::GET,
            &format!("/api/v1/agents/{agent}/test-runs"),
            None,
        )
        .await;
    assert_eq!(listed.body["items"][0]["current"], json!(false));
    assert_ne!(listed.body["draftHash"], run["draftHash"]);
}

#[tokio::test]
async fn a_queued_run_can_be_cancelled_but_a_finished_one_cannot() {
    let api = Api::new();
    let agent = api.agent("booking").await;
    assert_eq!(
        api.put_scenarios(&agent, &two_scenarios()).await.status,
        StatusCode::OK
    );
    let id = api.start(&agent).await.body["id"]
        .as_str()
        .expect("id")
        .to_owned();
    let cancel = format!("/api/v1/test-runs/{id}/cancel");

    let cancelled = api.send(Method::POST, &cancel, None).await;
    assert_eq!(cancelled.status, StatusCode::OK, "{}", cancelled.body);
    assert_eq!(cancelled.body["status"], json!("cancelled"));
    assert_eq!(cancelled.body["verdict"], Value::Null);
    assert_eq!(cancelled.body["scenarios"][0]["status"], json!("skipped"));
    assert_eq!(
        api.send(Method::POST, &cancel, None).await.body["status"],
        json!("cancelled")
    );
    assert_eq!(api.runs.worker().drain().await, 0);

    let again = api.start(&agent).await;
    assert_eq!(
        again.status,
        StatusCode::ACCEPTED,
        "a new run once none is active"
    );
    let busy = api.start(&agent).await;
    assert_problem(&busy, StatusCode::CONFLICT);
    api.callers_hang_up(2);
    assert_eq!(api.runs.worker().drain().await, 1);
    let finished = again.body["id"].as_str().expect("id");
    let refused = api
        .send(
            Method::POST,
            &format!("/api/v1/test-runs/{finished}/cancel"),
            None,
        )
        .await;
    assert_problem(&refused, StatusCode::CONFLICT);
}

#[tokio::test]
async fn errors_are_problems() {
    let api = Api::new();
    let agent = api.agent("support").await;

    let missing = api
        .send(Method::POST, "/api/v1/test-runs", Some(&json!({})))
        .await;
    assert_problem(&missing, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(fields(&missing), ["agentId"]);
    let malformed = api
        .send(
            Method::POST,
            "/api/v1/test-runs",
            Some(&json!({ "agentId": "not-a-uuid" })),
        )
        .await;
    assert_problem(&malformed, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(fields(&malformed), ["agentId"]);
    assert_problem(&api.start(UNKNOWN_ID).await, StatusCode::NOT_FOUND);
    assert_problem(&api.run(UNKNOWN_ID).await, StatusCode::NOT_FOUND);
    assert_problem(&api.run("not-a-uuid").await, StatusCode::NOT_FOUND);
    let cancel_unknown = format!("/api/v1/test-runs/{UNKNOWN_ID}/cancel");
    assert_problem(
        &api.send(Method::POST, &cancel_unknown, None).await,
        StatusCode::NOT_FOUND,
    );
    for uri in [
        format!("/api/v1/agents/{UNKNOWN_ID}/scenarios"),
        format!("/api/v1/agents/{UNKNOWN_ID}/test-runs"),
    ] {
        assert_problem(
            &api.send(Method::GET, &uri, None).await,
            StatusCode::NOT_FOUND,
        );
    }

    let archived = api.agent("support").await;
    let deleted = api
        .send(Method::DELETE, &format!("/api/v1/agents/{archived}"), None)
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    assert_problem(&api.start(&archived).await, StatusCode::CONFLICT);
    assert_problem(
        &api.put_scenarios(&archived, &two_scenarios()).await,
        StatusCode::CONFLICT,
    );

    for (method, uri) in [
        (Method::POST, "/api/v1/test-runs".to_owned()),
        (Method::GET, format!("/api/v1/test-runs/{UNKNOWN_ID}")),
        (
            Method::POST,
            format!("/api/v1/test-runs/{UNKNOWN_ID}/cancel"),
        ),
        (Method::GET, format!("/api/v1/agents/{agent}/scenarios")),
        (Method::PUT, format!("/api/v1/agents/{agent}/scenarios")),
        (Method::GET, format!("/api/v1/agents/{agent}/test-runs")),
    ] {
        let reply = api.call(request(method, &uri, false, None)).await;
        assert_problem(&reply, StatusCode::UNAUTHORIZED);
    }

    // Without models, a run can't start.
    let offline = Api::build(false);
    let agent = offline.agent("support").await;
    assert_problem(
        &offline.start(&agent).await,
        StatusCode::SERVICE_UNAVAILABLE,
    );
}
