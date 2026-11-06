//! Contract `test-runs` v1 (domain): scenarios, rubric scoring, a run's verdict and lifecycle,
//! and the Test step of setup progress.
//!
//! - A scenario is what a simulated caller plays: an ID, a name, the caller's goal and persona,
//!   a required flag, a turn cap (1 to 20 caller turns) and 1 to 8 rubric items, each a yes/no
//!   question with the answer it expects. A set holds 1 to 8 scenarios with unique IDs, and at
//!   least one of them is required. Every rule is checked when the value is built.
//! - Scenarios belong to the agent's draft. Until the operator saves some, the draft is tested
//!   with its use case's default set (`ScenarioSet::defaults_for`, the one seam through which
//!   purposes will supply sets). Every default set is valid and has a required scenario whose
//!   caller opts out mid-call; the survey's is: completes the survey, opts out mid-call, asks
//!   to be called back later. Saved scenarios are part of the draft and its content hash; a
//!   draft without them keeps its hash.
//! - A scenario's version is a content hash of everything that defines it.
//! - A rubric check passes when the decision model's probability of yes is on the expected side
//!   of 0.5 (0.5 itself counts as yes). No answer, or a probability outside 0 to 1, fails it.
//! - A scenario passes when its conversation ended without an error and every check passed. A
//!   conversation that failed, or a check that got no answer, is an error, not a fail.
//! - A run passes only if every required scenario passes; a failed optional scenario doesn't
//!   fail it. A run has a verdict only once it finished.
//! - A run records the draft hash and every scenario's version when it is queued, and again
//!   from the draft it actually tests when it starts. It moves queued → running → finished, or
//!   to cancelled (by the operator) or interrupted (the server stopped); a cancelled or
//!   interrupted run skips its open scenarios and has no verdict. Its progress counts the
//!   scenarios that are done.
//! - The Test step (75 % with a valid plan) is done when the latest finished run on the
//!   draft's current hash passed. A passing run on an earlier draft doesn't count, so an edit
//!   makes it stale; a later failing run on the same draft undoes it.
//! - The same draft and the same answers always give the same verdict.
//!
//! Contract file: never edit it once it is tagged `contract/test-runs/v1`. To change the
//! contract, add a new version.
#![expect(
    clippy::expect_used,
    reason = "fixture helpers outside #[test] functions fail the test on invalid input"
)]

use std::collections::HashSet;
use std::time::Duration;

use jiff::Timestamp;
use vak_domain::{
    AgentDraft, AgentId, AgentName, ContentHash, DraftField, DraftPatch, OptOutLayer,
    ProgressFacts, RubricCheck, RubricItem, RunModels, Scenario, ScenarioError, ScenarioId,
    ScenarioResult, ScenarioSet, ScenarioSpec, ScenarioStatus, SessionEndReason, SetupProgress,
    SetupStep, Speaker, TestRun, TestRunId, TestRunStatus, TranscriptLine, UseCase, Verdict,
    VoiceId,
};

fn front_desk(use_case: UseCase) -> AgentDraft {
    AgentDraft::new(
        AgentName::parse("Front desk").expect("valid name"),
        use_case,
        VoiceId::parse("af_heart").expect("valid voice"),
    )
}

fn scenario_id(raw: &str) -> ScenarioId {
    ScenarioId::parse(raw).expect("valid scenario ID")
}

fn item(question: &str, expected: bool) -> RubricItem {
    RubricItem::new(question, expected).expect("valid rubric item")
}

fn spec(id: &str, required: bool) -> ScenarioSpec {
    ScenarioSpec {
        id: scenario_id(id),
        name: format!("Scenario {id}"),
        goal: "Book a table for two tomorrow at seven.".to_owned(),
        persona: "A regular customer in a hurry.".to_owned(),
        required,
        max_turns: 6,
        rubric: vec![item("Did the agent confirm the booking?", true)],
    }
}

fn scenario(id: &str, required: bool) -> Scenario {
    Scenario::new(spec(id, required)).expect("valid scenario")
}

fn set(scenarios: Vec<Scenario>) -> ScenarioSet {
    ScenarioSet::new(scenarios).expect("valid scenario set")
}

fn with_scenarios(draft: &AgentDraft, scenarios: ScenarioSet) -> AgentDraft {
    draft.apply(&DraftPatch {
        scenarios: Some(scenarios),
        ..DraftPatch::default()
    })
}

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_second(1_793_600_000 + seconds).expect("valid time")
}

fn agent_id() -> AgentId {
    AgentId::parse("01920000-0000-7000-8000-000000000001").expect("v7")
}

fn run_id(n: u8) -> TestRunId {
    TestRunId::parse(&format!("01920000-0000-7000-8000-0000000000{n:02x}")).expect("v7")
}

fn models() -> RunModels {
    RunModels::new("fake-llm", "fake-decisions", 7)
}

fn transcript() -> Vec<TranscriptLine> {
    vec![
        TranscriptLine::new(Speaker::Agent, "Hi, this is Front desk."),
        TranscriptLine::new(Speaker::Caller, "A table for two, please."),
        TranscriptLine::new(Speaker::Agent, "Booked for tomorrow at seven."),
    ]
}

/// The scenario's result with every rubric item answered `p`.
fn answered(result: &ScenarioResult, end: SessionEndReason, p: Option<f64>) -> ScenarioResult {
    let checks = result
        .scenario()
        .rubric()
        .iter()
        .map(|rubric| RubricCheck::score(rubric, p))
        .collect();
    result
        .running()
        .finished(end, transcript(), checks, Duration::from_secs(3))
}

/// A run of `draft` started and every scenario finished with `p` for every check, except the
/// scenarios named in `failing`, answered 0.
fn finished_run(id: TestRunId, draft: &AgentDraft, failing: &[&str]) -> TestRun {
    let queued = TestRun::queue(id, agent_id(), draft, at(0));
    let mut run = queued
        .start(draft, models(), at(1))
        .expect("a queued run starts");
    for result in run.scenarios().to_vec() {
        let p = if failing.contains(&result.scenario().id().as_str()) {
            0.0
        } else {
            1.0
        };
        let done = answered(&result, SessionEndReason::ClientEnded, Some(p));
        run = run.record(done).expect("a running run records results");
    }
    run.finish(at(9)).expect("a running run finishes")
}

// ---- Scenarios ---------------------------------------------------------------------------

#[test]
fn a_scenario_keeps_what_defines_it() {
    let built = scenario("books-a-table", true);

    assert_eq!(built.id().as_str(), "books-a-table");
    assert_eq!(built.name(), "Scenario books-a-table");
    assert_eq!(built.goal(), "Book a table for two tomorrow at seven.");
    assert_eq!(built.persona(), "A regular customer in a hurry.");
    assert!(built.required());
    assert_eq!(built.max_turns(), 6);
    assert_eq!(built.rubric().len(), 1);
    assert_eq!(
        built.rubric()[0].question(),
        "Did the agent confirm the booking?"
    );
    assert!(built.rubric()[0].expected());
    // The simulated caller plays the goal and the persona, word for word.
    let instructions = built.caller_instructions();
    assert!(instructions.contains("Book a table for two tomorrow at seven."));
    assert!(instructions.contains("A regular customer in a hurry."));
}

#[test]
fn scenario_ids_are_short_lowercase_slugs() {
    for good in [
        "a",
        "opts-out-mid-call",
        "case2",
        &"x".repeat(ScenarioId::MAX_LEN),
    ] {
        assert_eq!(
            ScenarioId::parse(good).map(|id| id.as_str().to_owned()),
            Ok(good.to_owned())
        );
    }
    for bad in [
        "",
        "2nd",
        "-a",
        "Has-Caps",
        "has space",
        "under_score",
        &"x".repeat(41),
    ] {
        assert_eq!(
            ScenarioId::parse(bad),
            Err(ScenarioError::InvalidId),
            "{bad:?}"
        );
    }
    assert_eq!(ScenarioId::MAX_LEN, 40);
}

#[test]
fn every_scenario_rule_is_checked_when_it_is_built() {
    let cases: Vec<(ScenarioSpec, ScenarioError)> = vec![
        (
            ScenarioSpec {
                name: "  ".to_owned(),
                ..spec("a", true)
            },
            ScenarioError::Name,
        ),
        (
            ScenarioSpec {
                name: "n".repeat(Scenario::MAX_NAME + 1),
                ..spec("a", true)
            },
            ScenarioError::Name,
        ),
        (
            ScenarioSpec {
                goal: String::new(),
                ..spec("a", true)
            },
            ScenarioError::Goal,
        ),
        (
            ScenarioSpec {
                goal: "g".repeat(Scenario::MAX_GOAL + 1),
                ..spec("a", true)
            },
            ScenarioError::Goal,
        ),
        (
            ScenarioSpec {
                persona: "p".repeat(Scenario::MAX_PERSONA + 1),
                ..spec("a", true)
            },
            ScenarioError::Persona,
        ),
        (
            ScenarioSpec {
                max_turns: 0,
                ..spec("a", true)
            },
            ScenarioError::MaxTurns,
        ),
        (
            ScenarioSpec {
                max_turns: Scenario::MAX_TURNS + 1,
                ..spec("a", true)
            },
            ScenarioError::MaxTurns,
        ),
        (
            ScenarioSpec {
                rubric: Vec::new(),
                ..spec("a", true)
            },
            ScenarioError::RubricCount,
        ),
        (
            ScenarioSpec {
                rubric: vec![item("Did it work?", true); Scenario::MAX_RUBRIC + 1],
                ..spec("a", true)
            },
            ScenarioError::RubricCount,
        ),
    ];
    for (bad, error) in cases {
        assert_eq!(Scenario::new(bad), Err(error));
    }
    assert_eq!(RubricItem::new(" ", true), Err(ScenarioError::Question));
    assert_eq!(
        RubricItem::new(&"q".repeat(RubricItem::MAX_QUESTION + 1), false),
        Err(ScenarioError::Question)
    );
    // Text is trimmed; an empty persona is allowed.
    let trimmed = Scenario::new(ScenarioSpec {
        name: "  Books a table  ".to_owned(),
        persona: String::new(),
        ..spec("a", false)
    })
    .expect("valid");
    assert_eq!(trimmed.name(), "Books a table");
    assert_eq!(trimmed.persona(), "");
    assert_eq!(item("  Did it work?  ", true).question(), "Did it work?");
    assert_eq!(
        (
            Scenario::MAX_NAME,
            Scenario::MAX_GOAL,
            Scenario::MAX_PERSONA,
            Scenario::MAX_TURNS,
            Scenario::MAX_RUBRIC,
            RubricItem::MAX_QUESTION,
        ),
        (80, 1_000, 1_000, 20, 8, 300)
    );
}

#[test]
fn a_set_has_unique_ids_and_a_required_scenario() {
    assert_eq!(ScenarioSet::new(Vec::new()), Err(ScenarioError::Count));
    assert_eq!(
        ScenarioSet::new(
            (0..=ScenarioSet::MAX_SCENARIOS)
                .map(|i| scenario(&format!("s{i}"), true))
                .collect()
        ),
        Err(ScenarioError::Count)
    );
    assert_eq!(
        ScenarioSet::new(vec![scenario("same", true), scenario("same", false)]),
        Err(ScenarioError::DuplicateId(scenario_id("same")))
    );
    assert_eq!(
        ScenarioSet::new(vec![scenario("a", false), scenario("b", false)]),
        Err(ScenarioError::NoneRequired)
    );
    let ok = set(vec![scenario("a", true), scenario("b", false)]);
    let ids: Vec<&str> = ok.scenarios().iter().map(|s| s.id().as_str()).collect();
    assert_eq!(ids, ["a", "b"], "a set keeps its order");
    assert_eq!(ScenarioSet::MAX_SCENARIOS, 8);
}

#[test]
fn every_use_case_has_a_valid_default_set_with_a_required_opt_out() {
    for use_case in UseCase::ALL {
        let defaults = ScenarioSet::defaults_for(use_case);
        let rebuilt = ScenarioSet::new(defaults.scenarios().to_vec());
        assert_eq!(
            rebuilt.as_ref(),
            Ok(&defaults),
            "{use_case}: the defaults are a valid set"
        );
        let opt_out = defaults
            .scenarios()
            .iter()
            .find(|s| s.id().as_str() == "opts-out-mid-call");
        assert!(
            opt_out.is_some_and(Scenario::required),
            "{use_case}: a required scenario whose caller opts out"
        );
        assert_eq!(
            ScenarioSet::defaults_for(use_case),
            defaults,
            "{use_case}: the defaults are stable"
        );
    }
    let survey = ScenarioSet::defaults_for(UseCase::Survey);
    let survey: Vec<&str> = survey.scenarios().iter().map(|s| s.id().as_str()).collect();
    for id in ["completes-survey", "opts-out-mid-call", "asks-to-call-back"] {
        assert!(survey.contains(&id), "the survey defaults have {id}");
    }
}

#[test]
fn a_scenarios_version_changes_with_anything_that_defines_it() {
    let base = scenario("a", true);
    assert_eq!(base.version(), scenario("a", true).version());
    assert!(base.version().as_str().starts_with("sha256:"));
    let variants = [
        ScenarioSpec {
            id: scenario_id("b"),
            ..spec("a", true)
        },
        ScenarioSpec {
            name: "Other".to_owned(),
            ..spec("a", true)
        },
        ScenarioSpec {
            goal: "Cancel a booking.".to_owned(),
            ..spec("a", true)
        },
        ScenarioSpec {
            persona: "Calm.".to_owned(),
            ..spec("a", true)
        },
        spec("a", false),
        ScenarioSpec {
            max_turns: 7,
            ..spec("a", true)
        },
        ScenarioSpec {
            rubric: vec![item("Did the agent confirm the booking?", false)],
            ..spec("a", true)
        },
        ScenarioSpec {
            rubric: vec![item("Did the agent say goodbye?", true)],
            ..spec("a", true)
        },
    ];
    let mut versions = HashSet::from([base.version()]);
    for variant in variants {
        let version = Scenario::new(variant).expect("valid").version();
        assert!(versions.insert(version), "every change gives a new version");
    }
}

// ---- Scenarios in the draft --------------------------------------------------------------

#[test]
fn the_default_set_applies_until_scenarios_are_saved() {
    let draft = front_desk(UseCase::Survey);
    assert_eq!(
        draft.scenarios(),
        None,
        "a new draft has no saved scenarios"
    );
    assert_eq!(
        draft.scenarios_or_defaults(),
        ScenarioSet::defaults_for(UseCase::Survey)
    );
    assert_eq!(
        draft.apply(&DraftPatch::default()).content_hash(),
        draft.content_hash(),
        "a draft without saved scenarios keeps its hash"
    );

    let own = set(vec![scenario("books-a-table", true)]);
    let saved = with_scenarios(&draft, own.clone());
    assert_eq!(saved.scenarios(), Some(&own));
    assert_eq!(saved.scenarios_or_defaults(), own);
    assert_eq!(draft.changed_fields(&saved), [DraftField::Scenarios]);
    assert_eq!(DraftField::Scenarios.as_str(), "scenarios");
    assert_ne!(saved.content_hash(), draft.content_hash());
    assert_eq!(
        with_scenarios(&saved, own.clone()),
        saved,
        "saving the same set changes nothing"
    );

    let edited = with_scenarios(
        &saved,
        set(vec![
            scenario("books-a-table", false),
            scenario("other", true),
        ]),
    );
    assert_ne!(edited.content_hash(), saved.content_hash());
    // The operator's scenarios stay when the use case changes.
    let moved = saved.apply(&DraftPatch {
        use_case: Some(UseCase::Booking),
        ..DraftPatch::default()
    });
    assert_eq!(moved.scenarios(), Some(&own));
    assert_eq!(
        front_desk(UseCase::Booking).scenarios_or_defaults(),
        ScenarioSet::defaults_for(UseCase::Booking)
    );
}

// ---- Rubric checks and scenario results ----------------------------------------------------

#[test]
fn a_check_passes_on_the_expected_side_of_one_half() {
    let yes = item("Did the agent confirm the booking?", true);
    let no = item("Did the agent claim to be human?", false);
    for (rubric, p, passed) in [
        (&yes, 0.9, true),
        (&yes, 0.5, true),
        (&yes, 0.49, false),
        (&no, 0.1, true),
        (&no, 0.49, true),
        (&no, 0.5, false),
        (&no, 1.0, false),
    ] {
        let check = RubricCheck::score(rubric, Some(p));
        assert_eq!(check.passed(), passed, "{} at {p}", rubric.question());
        assert_eq!(check.probability(), Some(p));
        assert_eq!(check.question(), rubric.question());
        assert_eq!(check.expected(), rubric.expected());
    }
    for unanswered in [None, Some(-0.1), Some(1.5), Some(f64::NAN)] {
        let check = RubricCheck::score(&yes, unanswered);
        assert!(!check.passed());
        assert_eq!(check.probability(), None, "{unanswered:?} is no answer");
    }
}

#[test]
fn a_scenario_passes_only_without_errors_and_with_every_check_passed() {
    let pending = ScenarioResult::pending(&scenario("a", true));
    assert_eq!(pending.status(), ScenarioStatus::Pending);
    assert_eq!(pending.running().status(), ScenarioStatus::Running);
    assert_eq!(pending.skipped().status(), ScenarioStatus::Skipped);

    let passed = answered(&pending, SessionEndReason::ClientEnded, Some(0.9));
    assert_eq!(passed.status(), ScenarioStatus::Passed);
    assert_eq!(passed.end_reason(), Some(SessionEndReason::ClientEnded));
    assert_eq!(passed.transcript(), transcript());
    assert_eq!(passed.checks().len(), 1);
    assert_eq!(passed.duration(), Some(Duration::from_secs(3)));

    let opted_out = answered(
        &pending,
        SessionEndReason::OptOut(OptOutLayer::Phrase),
        Some(0.9),
    );
    assert_eq!(
        opted_out.status(),
        ScenarioStatus::Passed,
        "an opt-out is an ending, not an error"
    );
    assert_eq!(
        answered(&pending, SessionEndReason::Timeout, Some(0.2)).status(),
        ScenarioStatus::Failed
    );
    assert_eq!(
        answered(&pending, SessionEndReason::Error, Some(0.9)).status(),
        ScenarioStatus::Error,
        "a conversation that failed is an error"
    );
    assert_eq!(
        answered(&pending, SessionEndReason::ClientEnded, None).status(),
        ScenarioStatus::Error,
        "a check without an answer is an error"
    );
}

// ---- The verdict and the run's lifecycle ----------------------------------------------------

#[test]
fn a_run_passes_only_if_every_required_scenario_passes() {
    let draft = with_scenarios(
        &front_desk(UseCase::Booking),
        set(vec![
            scenario("first", true),
            scenario("second", true),
            scenario("optional", false),
        ]),
    );

    let all = finished_run(run_id(1), &draft, &[]);
    assert_eq!(all.status(), TestRunStatus::Finished);
    assert_eq!(all.verdict(), Some(Verdict::Passed));

    let optional = finished_run(run_id(2), &draft, &["optional"]);
    assert_eq!(
        optional.verdict(),
        Some(Verdict::Passed),
        "an optional failure doesn't fail it"
    );
    let statuses: Vec<ScenarioStatus> = optional
        .scenarios()
        .iter()
        .map(ScenarioResult::status)
        .collect();
    assert_eq!(
        statuses,
        [
            ScenarioStatus::Passed,
            ScenarioStatus::Passed,
            ScenarioStatus::Failed
        ]
    );

    let required = finished_run(run_id(3), &draft, &["second"]);
    assert_eq!(required.verdict(), Some(Verdict::Failed));
    assert_eq!(Verdict::of(required.scenarios()), Verdict::Failed);
    assert_eq!(Verdict::of(optional.scenarios()), Verdict::Passed);

    // A required scenario that errored fails the run too.
    let queued = TestRun::queue(run_id(4), agent_id(), &draft, at(0));
    let mut run = queued.start(&draft, models(), at(1)).expect("starts");
    for result in run.scenarios().to_vec() {
        let end = if result.scenario().id().as_str() == "first" {
            SessionEndReason::Error
        } else {
            SessionEndReason::ClientEnded
        };
        run = run
            .record(answered(&result, end, Some(1.0)))
            .expect("records");
    }
    assert_eq!(
        run.finish(at(9)).and_then(|run| run.verdict()),
        Some(Verdict::Failed)
    );
}

#[test]
fn a_run_records_what_it_tests() {
    let draft = front_desk(UseCase::Survey);
    let queued = TestRun::queue(run_id(1), agent_id(), &draft, at(0));

    assert_eq!(queued.id(), run_id(1));
    assert_eq!(queued.agent_id(), agent_id());
    assert_eq!(queued.status(), TestRunStatus::Queued);
    assert_eq!(queued.draft_hash(), &draft.content_hash());
    assert_eq!(queued.verdict(), None);
    assert_eq!(queued.models(), None);
    assert_eq!(queued.created_at(), at(0));
    assert_eq!(queued.started_at(), None);
    let versions: Vec<ContentHash> = queued
        .scenarios()
        .iter()
        .map(|r| r.scenario().version())
        .collect();
    let expected: Vec<ContentHash> = draft
        .scenarios_or_defaults()
        .scenarios()
        .iter()
        .map(Scenario::version)
        .collect();
    assert_eq!(
        versions, expected,
        "every scenario of the draft, in order, with its version"
    );
    assert!(
        queued
            .scenarios()
            .iter()
            .all(|r| r.status() == ScenarioStatus::Pending)
    );
    assert_eq!(
        TestRun::queue(run_id(1), agent_id(), &draft, at(0)),
        queued,
        "the same draft always gives the same run"
    );

    // It tests the draft as it is when it starts.
    let edited = with_scenarios(&draft, set(vec![scenario("only", true)]));
    let running = queued
        .start(&edited, models(), at(5))
        .expect("a queued run starts");
    assert_eq!(running.status(), TestRunStatus::Running);
    assert_eq!(running.draft_hash(), &edited.content_hash());
    assert_eq!(running.scenarios().len(), 1);
    assert_eq!(running.models(), Some(&models()));
    assert_eq!(running.models().map(RunModels::llm), Some("fake-llm"));
    assert_eq!(
        running.models().map(RunModels::decisions),
        Some("fake-decisions")
    );
    assert_eq!(running.models().map(RunModels::seed), Some(7));
    assert_eq!(running.started_at(), Some(at(5)));
    assert_eq!(
        running.start(&edited, models(), at(6)),
        None,
        "a run starts once"
    );
}

#[test]
fn progress_counts_done_scenarios_and_a_run_finishes_once() {
    let draft = with_scenarios(
        &front_desk(UseCase::Booking),
        set(vec![
            scenario("a", true),
            scenario("b", true),
            scenario("c", false),
        ]),
    );
    let queued = TestRun::queue(run_id(1), agent_id(), &draft, at(0));
    assert_eq!(
        (queued.progress().done(), queued.progress().total()),
        (0, 3)
    );
    assert_eq!(queued.progress().percent(), 0);
    assert_eq!(
        queued.record(queued.scenarios()[0].running()),
        None,
        "only a running run records"
    );
    assert_eq!(queued.finish(at(1)), None, "only a running run finishes");

    let mut run = queued.start(&draft, models(), at(1)).expect("starts");
    run = run.record(run.scenarios()[0].running()).expect("records");
    assert_eq!(run.progress().done(), 0, "a running scenario isn't done");
    let first = answered(
        &run.scenarios()[0],
        SessionEndReason::ClientEnded,
        Some(1.0),
    );
    run = run.record(first).expect("records");
    assert_eq!((run.progress().done(), run.progress().percent()), (1, 33));
    let unknown = ScenarioResult::pending(&scenario("nope", true));
    assert_eq!(run.record(unknown), None, "a result must belong to the run");

    let rest: Vec<ScenarioResult> = run.scenarios()[1..].to_vec();
    for result in rest {
        run = run
            .record(answered(&result, SessionEndReason::ClientEnded, Some(1.0)))
            .expect("records");
    }
    assert_eq!(run.progress().percent(), 100);
    let finished = run.finish(at(9)).expect("finishes");
    assert_eq!(finished.finished_at(), Some(at(9)));
    assert_eq!(finished.finish(at(10)), None);
    assert_eq!(
        finished.cancel(at(10)),
        None,
        "a finished run can't be cancelled"
    );
    assert_eq!(finished.interrupt(at(10)), None);
    assert!(!finished.is_active());
}

#[test]
fn a_cancelled_or_interrupted_run_skips_open_scenarios_and_has_no_verdict() {
    let draft = with_scenarios(
        &front_desk(UseCase::Booking),
        set(vec![scenario("a", true), scenario("b", true)]),
    );
    let queued = TestRun::queue(run_id(1), agent_id(), &draft, at(0));
    assert!(queued.is_active());
    let cancelled = queued.cancel(at(1)).expect("a queued run can be cancelled");
    assert_eq!(cancelled.status(), TestRunStatus::Cancelled);
    assert_eq!(cancelled.verdict(), None);
    assert_eq!(cancelled.finished_at(), Some(at(1)));
    assert!(
        cancelled
            .scenarios()
            .iter()
            .all(|r| r.status() == ScenarioStatus::Skipped)
    );
    assert_eq!(
        cancelled.cancel(at(2)),
        None,
        "cancelling twice changes nothing"
    );
    assert_eq!(cancelled.start(&draft, models(), at(2)), None);
    assert!(!cancelled.is_active());

    let mut running = queued.start(&draft, models(), at(1)).expect("starts");
    let first = answered(
        &running.scenarios()[0],
        SessionEndReason::ClientEnded,
        Some(1.0),
    );
    running = running.record(first).expect("records");
    running = running
        .record(running.scenarios()[1].running())
        .expect("records");
    for stopped in [
        running
            .cancel(at(3))
            .expect("a running run can be cancelled"),
        running
            .interrupt(at(3))
            .expect("a running run can be interrupted"),
    ] {
        let statuses: Vec<ScenarioStatus> = stopped
            .scenarios()
            .iter()
            .map(ScenarioResult::status)
            .collect();
        assert_eq!(statuses, [ScenarioStatus::Passed, ScenarioStatus::Skipped]);
        assert_eq!(stopped.verdict(), None);
        assert_eq!(stopped.progress().percent(), 100);
    }
    assert_eq!(
        running.interrupt(at(3)).map(|run| run.status()),
        Some(TestRunStatus::Interrupted)
    );
}

#[test]
fn statuses_and_verdicts_have_wire_names() {
    let names: Vec<&str> = [
        TestRunStatus::Queued,
        TestRunStatus::Running,
        TestRunStatus::Finished,
        TestRunStatus::Cancelled,
        TestRunStatus::Interrupted,
    ]
    .iter()
    .map(|status| status.as_str())
    .collect();
    assert_eq!(
        names,
        ["queued", "running", "finished", "cancelled", "interrupted"]
    );
    let names: Vec<&str> = [
        ScenarioStatus::Pending,
        ScenarioStatus::Running,
        ScenarioStatus::Passed,
        ScenarioStatus::Failed,
        ScenarioStatus::Error,
        ScenarioStatus::Skipped,
    ]
    .iter()
    .map(|status| status.as_str())
    .collect();
    assert_eq!(
        names,
        ["pending", "running", "passed", "failed", "error", "skipped"]
    );
    assert_eq!(
        (Verdict::Passed.as_str(), Verdict::Failed.as_str()),
        ("passed", "failed")
    );
}

// ---- The Test step ---------------------------------------------------------------------------

fn percent(draft: &AgentDraft, facts: &ProgressFacts) -> (u8, Option<SetupStep>) {
    let progress = SetupProgress::derive(draft, facts);
    (progress.percent(), progress.next_step())
}

#[test]
fn a_passing_run_on_the_current_draft_completes_the_test_step() {
    let draft = front_desk(UseCase::Survey);
    let plan_valid = ProgressFacts {
        plan_valid: true,
        ..ProgressFacts::default()
    };
    let tested = finished_run(run_id(1), &draft, &[]);
    assert_eq!(tested.verdict(), Some(Verdict::Passed));

    let passed = plan_valid
        .clone()
        .with_test_verdict(tested.draft_hash(), tested.verdict());
    assert_eq!(percent(&draft, &passed), (75, Some(SetupStep::Publish)));

    let failed = plan_valid
        .clone()
        .with_test_verdict(&draft.content_hash(), Some(Verdict::Failed));
    assert_eq!(
        percent(&draft, &failed),
        (50, Some(SetupStep::Test)),
        "the latest run on this draft failed"
    );
    assert_eq!(
        percent(
            &draft,
            &plan_valid
                .clone()
                .with_test_verdict(&draft.content_hash(), None)
        ),
        (50, Some(SetupStep::Test)),
        "no finished run on this draft"
    );

    // An edit makes the passing run stale: it tested an earlier draft.
    let edited = draft.apply(&DraftPatch {
        name: Some(AgentName::parse("After hours").expect("valid")),
        ..DraftPatch::default()
    });
    assert_eq!(percent(&edited, &passed), (50, Some(SetupStep::Test)));
    let rescoped = with_scenarios(&draft, set(vec![scenario("only", true)]));
    assert_eq!(
        percent(&rescoped, &passed),
        (50, Some(SetupStep::Test)),
        "changing the scenarios makes it stale too"
    );
}

#[test]
fn the_same_draft_and_answers_give_the_same_verdict() {
    let draft = front_desk(UseCase::Survey);
    let first = finished_run(run_id(1), &draft, &["asks-to-call-back"]);
    let again = finished_run(run_id(2), &draft, &["asks-to-call-back"]);

    assert_eq!(first.draft_hash(), again.draft_hash());
    assert_eq!(first.verdict(), again.verdict());
    assert_eq!(first.scenarios(), again.scenarios());
}
