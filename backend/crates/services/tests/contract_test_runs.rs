//! Contract `test-runs` v1 (services): text-mode test runs with simulated callers, in
//! simulated time with deterministic fakes (`contract_test_runs_fakes/`).
//!
//! - `TestRunService::start` queues a run of the agent's scenarios and writes a
//!   `test_run.started` audit event; the worker (`TestRunWorker::drain`) runs queued runs.
//! - A simulated caller (an LLM playing the scenario, offered the `end_call` tool) talks to
//!   the agent's real reply logic in text: the agent speaks its locked disclosure first; each
//!   caller line goes through the opt-out phrases, the decision model's `opt_out` question
//!   (with the confirm band: the check question replaces the reply, the next caller line
//!   answers it) and the LLM's `mark_do_not_call` tool, as on a call. The conversation ends
//!   when the caller hangs up, an opt-out ends it with the locked confirmation, it reaches the
//!   scenario's turn cap, or a model fails.
//! - The rubric is scored by the decision model: one request per scenario with every rubric
//!   question over the whole transcript. A run passes only if every required scenario passes.
//! - Reproducible: every LLM request samples greedily with the settings' seed, and the run
//!   records the draft hash, the model IDs and every scenario's version. The same draft and
//!   the same fakes give the same transcripts, checks and verdict.
//! - Scenarios run one at a time by default (a local LLM has one slot), more when configured.
//!   Every model call has a deadline: a model that never answers fails its scenario with an
//!   error and the run still finishes.
//! - Cancelling stops a run at once, drops the model call in flight and skips what is left
//!   (`test_run.cancelled`); a run left running by a stopped server is interrupted on recovery.
//! - Setup progress: 75 % once the latest finished run on the current draft passed (with a
//!   valid plan); an edit makes it stale, and a later failing run on the same draft undoes it.
//!
//! Contract file: never edit it once it is tagged `contract/test-runs/v1`. To change the
//! contract, add a new version.
#![expect(
    clippy::expect_used,
    clippy::too_many_lines,
    reason = "a failed expectation is a test failure; each test reads as one conversation"
)]

mod contract_test_runs_fakes;

use std::time::Duration;

use contract_test_runs_fakes::{
    AGENT_REPLY, CALLER_TOOL, DECISION_MODEL, FakeJudge, FakeLlm, LLM_MODEL, Move, OPT_OUT_TOOL,
    Side, World, actor, item, scenario, set,
};
use tokio::time::Instant;
use vak_domain::{
    AgentName, DEFAULT_OPT_OUT_CHECK_QUESTION, DEFAULT_OPT_OUT_CONFIRMATION, DraftPatch,
    OptOutLayer, ScenarioResult, ScenarioStatus, SessionEndReason, Speaker, TestRun, TestRunStatus,
    UseCase, Verdict,
};
use vak_providers::Role;
use vak_services::{
    AgentService, CancelTestRunError, FindTestRunError, StartTestRunError, TestRunService,
    TestRunSettings,
};
use vak_testkit::repositories::{InMemoryAgentRepo, InMemoryTestRunRepo};

const DISCLOSURE: &str =
    "Hi, this is Front desk, an automated AI assistant. This call may be recorded.";
const BOOKING: &str = "Book a table for two tomorrow at seven.";
const CONFIRMS: &str = "Did the agent confirm the booking?";

fn lines(result: &ScenarioResult) -> Vec<(Speaker, &str)> {
    result
        .transcript()
        .iter()
        .map(|line| (line.speaker(), line.text()))
        .collect()
}

fn result<'a>(run: &'a TestRun, id: &str) -> &'a ScenarioResult {
    run.scenarios()
        .iter()
        .find(|r| r.scenario().id().as_str() == id)
        .expect("the run has this scenario")
}

/// Starts a run of `agent`, lets the worker run it, and reads it back.
async fn run_to_end(world: &World, agent: vak_domain::AgentId) -> TestRun {
    let queued = world.runs.start(&actor(), agent).await.expect("queued");
    assert_eq!(world.runs.worker().drain().await, 1, "the worker runs it");
    world.runs.get(queued.id()).await.expect("found")
}

#[tokio::test(start_paused = true)]
async fn a_simulated_caller_runs_a_scenario_to_completion() {
    let llm = FakeLlm::new()
        .caller(
            BOOKING,
            vec![
                Move::Say("Hi, a table for two tomorrow at seven, please."),
                Move::Say("Yes, under Ada."),
                Move::HangUp("Thanks, bye."),
            ],
        )
        .agent(
            "table for two",
            Move::Say("Sure. What name is the booking under?"),
        )
        .agent(
            "under Ada",
            Move::Say("Booked for two tomorrow at seven, Ada."),
        );
    let world = World::new(llm, FakeJudge::new());
    let agent = world
        .agent_with(set(vec![scenario(
            "books-a-table",
            BOOKING,
            true,
            vec![item(CONFIRMS, true)],
        )]))
        .await;

    let queued = world.runs.start(&actor(), agent).await.expect("queued");
    assert_eq!(queued.status(), TestRunStatus::Queued);
    assert_eq!(queued.scenarios()[0].status(), ScenarioStatus::Pending);
    assert_eq!(world.runs.worker().drain().await, 1);
    let run = world.runs.get(queued.id()).await.expect("found");

    assert_eq!(run.status(), TestRunStatus::Finished);
    assert_eq!(run.verdict(), Some(Verdict::Passed));
    let played = &run.scenarios()[0];
    assert_eq!(played.status(), ScenarioStatus::Passed);
    assert_eq!(
        played.end_reason(),
        Some(SessionEndReason::ClientEnded),
        "the caller hung up"
    );
    assert_eq!(
        lines(played),
        [
            (Speaker::Agent, DISCLOSURE),
            (
                Speaker::Caller,
                "Hi, a table for two tomorrow at seven, please."
            ),
            (Speaker::Agent, "Sure. What name is the booking under?"),
            (Speaker::Caller, "Yes, under Ada."),
            (Speaker::Agent, "Booked for two tomorrow at seven, Ada."),
            (Speaker::Caller, "Thanks, bye."),
        ]
    );
    assert!(played.duration().is_some());

    // The caller plays the scenario: its goal in its instructions, the agent's lines as the
    // other party's, its own as its replies, and a tool to hang up.
    let calls = world.llm.calls();
    let caller: Vec<_> = calls.iter().filter(|c| c.side == Side::Caller).collect();
    assert_eq!(caller.len(), 3);
    assert_eq!(caller[0].messages[0].0, Role::System);
    assert!(caller[0].messages[0].1.contains(BOOKING));
    assert_eq!(
        caller[0].messages[1..],
        [(Role::User, DISCLOSURE.to_owned())]
    );
    assert_eq!(caller[0].tools, [CALLER_TOOL]);
    assert_eq!(
        caller[1].messages[2..],
        [
            (
                Role::Assistant,
                "Hi, a table for two tomorrow at seven, please.".to_owned()
            ),
            (
                Role::User,
                "Sure. What name is the booking under?".to_owned()
            ),
        ]
    );
    // The agent answers with its own instructions, history and tools, as on a call.
    let agent_calls: Vec<_> = calls.iter().filter(|c| c.side == Side::Agent).collect();
    assert_eq!(agent_calls.len(), 2);
    assert_eq!(agent_calls[0].messages[0].0, Role::System);
    assert!(
        agent_calls[0].messages[0]
            .1
            .starts_with("You are Front desk, an AI voice assistant.")
    );
    assert_eq!(
        agent_calls[0].messages[1..],
        [
            (Role::Assistant, DISCLOSURE.to_owned()),
            (
                Role::User,
                "Hi, a table for two tomorrow at seven, please.".to_owned()
            ),
        ]
    );
    assert!(agent_calls[0].tools.iter().any(|tool| tool == OPT_OUT_TOOL));
    assert!(
        agent_calls
            .iter()
            .all(|c| !c.tools.iter().any(|tool| tool == CALLER_TOOL))
    );
}

#[tokio::test(start_paused = true)]
async fn a_scenario_stops_at_its_turn_cap() {
    let llm = FakeLlm::new().caller(
        BOOKING,
        vec![
            Move::Say("Hello?"),
            Move::Say("Hello?"),
            Move::Say("Hello?"),
        ],
    );
    let world = World::new(llm, FakeJudge::new());
    let mut capped = scenario("talks-forever", BOOKING, true, vec![item(CONFIRMS, true)]);
    capped = vak_domain::Scenario::new(vak_domain::ScenarioSpec {
        id: capped.id().clone(),
        name: capped.name().to_owned(),
        goal: capped.goal().to_owned(),
        persona: capped.persona().to_owned(),
        required: true,
        max_turns: 2,
        rubric: capped.rubric().to_vec(),
    })
    .expect("valid");
    let agent = world.agent_with(set(vec![capped])).await;

    let run = run_to_end(&world, agent).await;

    let played = &run.scenarios()[0];
    assert_eq!(
        played.end_reason(),
        Some(SessionEndReason::Timeout),
        "the turn cap"
    );
    let speakers: Vec<Speaker> = played
        .transcript()
        .iter()
        .map(vak_domain::TranscriptLine::speaker)
        .collect();
    assert_eq!(
        speakers,
        [
            Speaker::Agent,
            Speaker::Caller,
            Speaker::Agent,
            Speaker::Caller,
            Speaker::Agent
        ],
        "two caller turns, each answered"
    );
}

#[tokio::test(start_paused = true)]
async fn the_rubric_is_scored_by_the_decision_engine_over_the_transcript() {
    let llm = FakeLlm::new().caller(
        BOOKING,
        vec![Move::Say("A table for two."), Move::HangUp("Bye.")],
    );
    let judge = FakeJudge::new().rubric("claim to be human", 0.8);
    let world = World::new(llm, judge);
    let rubric = vec![
        item(CONFIRMS, true),
        item("Did the agent claim to be human?", false),
    ];
    let agent = world
        .agent_with(set(vec![scenario("books-a-table", BOOKING, true, rubric)]))
        .await;

    let run = run_to_end(&world, agent).await;

    let played = &run.scenarios()[0];
    let checks: Vec<(&str, bool, Option<f64>, bool)> = played
        .checks()
        .iter()
        .map(|c| (c.question(), c.expected(), c.probability(), c.passed()))
        .collect();
    assert_eq!(
        checks,
        [
            (CONFIRMS, true, Some(0.9), true),
            ("Did the agent claim to be human?", false, Some(0.8), false),
        ]
    );
    assert_eq!(played.status(), ScenarioStatus::Failed);
    assert_eq!(run.verdict(), Some(Verdict::Failed));
    // One decision request per scenario, every rubric question over the whole transcript.
    let scoring = world.judge.rubric_calls();
    assert_eq!(scoring.len(), 1);
    assert_eq!(scoring[0].questions.len(), 2);
    assert!(scoring[0].questions[0].1.contains(CONFIRMS));
    assert!(
        scoring[0].questions[1]
            .1
            .contains("Did the agent claim to be human?")
    );
    let turns: Vec<(&str, &str)> = scoring[0]
        .turns
        .iter()
        .map(|(speaker, text)| (speaker.as_str(), text.as_str()))
        .collect();
    let transcript: Vec<(&str, &str)> = played
        .transcript()
        .iter()
        .map(|l| (l.speaker().as_str(), l.text()))
        .collect();
    assert_eq!(turns, transcript);
}

#[tokio::test(start_paused = true)]
async fn a_run_passes_only_if_every_required_scenario_passes() {
    let scenarios = || {
        set(vec![
            scenario(
                "first",
                "Ask about opening hours.",
                true,
                vec![item("first-check?", true)],
            ),
            scenario(
                "second",
                "Ask about parking.",
                true,
                vec![item("second-check?", true)],
            ),
            scenario(
                "extra",
                "Ask about the menu.",
                false,
                vec![item("extra-check?", true)],
            ),
        ])
    };

    let optional_fails = World::new(FakeLlm::new(), FakeJudge::new().rubric("extra-check", 0.1));
    let agent = optional_fails.agent_with(scenarios()).await;
    let run = run_to_end(&optional_fails, agent).await;
    let statuses: Vec<ScenarioStatus> =
        run.scenarios().iter().map(ScenarioResult::status).collect();
    assert_eq!(
        statuses,
        [
            ScenarioStatus::Passed,
            ScenarioStatus::Passed,
            ScenarioStatus::Failed
        ]
    );
    assert_eq!(
        run.verdict(),
        Some(Verdict::Passed),
        "only an optional scenario failed"
    );

    let required_fails = World::new(
        FakeLlm::new(),
        FakeJudge::new()
            .rubric("extra-check", 0.1)
            .rubric("second-check", 0.1),
    );
    let agent = required_fails.agent_with(scenarios()).await;
    let run = run_to_end(&required_fails, agent).await;
    assert_eq!(result(&run, "second").status(), ScenarioStatus::Failed);
    assert_eq!(run.verdict(), Some(Verdict::Failed));
}

#[tokio::test(start_paused = true)]
async fn the_same_draft_and_fakes_give_the_same_verdict() {
    let llm = FakeLlm::new()
        .caller(
            BOOKING,
            vec![Move::Say("A table for two."), Move::HangUp("Bye.")],
        )
        .agent("table for two", Move::Say("For when?"));
    let judge = FakeJudge::new().rubric("say goodbye", 0.2);
    let world = World::new(llm, judge);
    let agent = world
        .agent_with(set(vec![
            scenario("books-a-table", BOOKING, true, vec![item(CONFIRMS, true)]),
            scenario(
                "says-bye",
                "Say hello, then hang up.",
                false,
                vec![item("Did the agent say goodbye?", true)],
            ),
        ]))
        .await;
    let draft = world
        .agents
        .get(agent)
        .await
        .expect("found")
        .draft()
        .clone();

    let first = run_to_end(&world, agent).await;
    let again = run_to_end(&world, agent).await;

    assert_ne!(first.id(), again.id());
    assert_eq!(first.verdict(), Some(Verdict::Passed));
    assert_eq!(again.verdict(), first.verdict());
    assert_eq!(first.draft_hash(), &draft.content_hash());
    assert_eq!(again.draft_hash(), first.draft_hash());
    for (a, b) in first.scenarios().iter().zip(again.scenarios()) {
        assert_eq!(a.scenario().version(), b.scenario().version());
        assert_eq!(a.status(), b.status());
        assert_eq!(a.end_reason(), b.end_reason());
        assert_eq!(a.transcript(), b.transcript());
        assert_eq!(a.checks(), b.checks());
    }
    let versions: Vec<_> = first
        .scenarios()
        .iter()
        .map(|r| r.scenario().version())
        .collect();
    let expected: Vec<_> = draft
        .scenarios_or_defaults()
        .scenarios()
        .iter()
        .map(vak_domain::Scenario::version)
        .collect();
    assert_eq!(versions, expected, "every scenario's version is recorded");
    let models = first.models().expect("recorded");
    assert_eq!(
        (models.llm(), models.decisions()),
        (LLM_MODEL, DECISION_MODEL)
    );
    assert_eq!(models.seed(), TestRunSettings::default().seed);
    // Sampling is pinned on every request: greedy, with the run's seed.
    assert!(
        world
            .llm
            .calls()
            .iter()
            .all(|call| call.greedy_seed == Some(TestRunSettings::default().seed))
    );
}

#[tokio::test(start_paused = true)]
async fn progress_reaches_75_percent_with_a_passing_run_on_the_current_draft() {
    let world = World::new(FakeLlm::new(), FakeJudge::new());
    let agent = world
        .agent_with(set(vec![scenario(
            "books-a-table",
            BOOKING,
            true,
            vec![item(CONFIRMS, true)],
        )]))
        .await;
    let plan = world
        .agents
        .plan(agent)
        .await
        .expect("found")
        .plan()
        .clone();
    world
        .agents
        .save_plan(&actor(), agent, plan)
        .await
        .expect("saved");
    let percent = |world: &World| {
        let agents = world.agents.clone();
        async move { agents.progress(agent).await.expect("found").percent() }
    };
    assert_eq!(percent(&world).await, 50, "a valid plan");

    assert_eq!(
        run_to_end(&world, agent).await.verdict(),
        Some(Verdict::Passed)
    );
    assert_eq!(percent(&world).await, 75);
    let listed = world.agents.list().await.expect("listed");
    assert_eq!(listed[0].progress.percent(), 75, "the list shows it too");

    // An edit makes the run stale.
    let rename = DraftPatch {
        name: Some(AgentName::parse("After hours").expect("valid")),
        ..DraftPatch::default()
    };
    world
        .agents
        .update_draft(&actor(), agent, &rename)
        .await
        .expect("saved");
    assert_eq!(percent(&world).await, 50);
    let runs = world.runs.list(agent).await.expect("found");
    let current = world
        .agents
        .get(agent)
        .await
        .expect("found")
        .draft()
        .content_hash();
    assert_eq!(runs.draft_hash, current);
    assert_ne!(
        runs.runs[0].draft_hash(),
        &current,
        "the latest run tested an earlier draft"
    );

    assert_eq!(
        run_to_end(&world, agent).await.verdict(),
        Some(Verdict::Passed)
    );
    assert_eq!(percent(&world).await, 75);
    // A later failing run on the same draft undoes it.
    world.judge.answer_rubric_with(0.1);
    assert_eq!(
        run_to_end(&world, agent).await.verdict(),
        Some(Verdict::Failed)
    );
    assert_eq!(percent(&world).await, 50);
    let runs = world.runs.list(agent).await.expect("found");
    assert_eq!(runs.runs.len(), 3);
    assert_eq!(
        runs.runs[0].verdict(),
        Some(Verdict::Failed),
        "newest first"
    );
}

#[tokio::test(start_paused = true)]
async fn opt_outs_end_a_text_conversation_as_they_end_a_call() {
    let llm = FakeLlm::new()
        .caller("by phrase", vec![Move::Say("Please stop calling me.")])
        .caller("by decision", vec![Move::Say("I'm done with you people.")])
        .caller(
            "when unsure",
            vec![Move::Say("Leave me alone."), Move::Say("Yes.")],
        )
        .caller(
            "carry on",
            vec![
                Move::Say("Leave me be."),
                Move::Say("No, go on."),
                Move::HangUp("Bye."),
            ],
        )
        .caller(
            "by tool",
            vec![Move::Say("Just put me on your no-call thing.")],
        )
        .agent("no-call thing", Move::Tool(OPT_OUT_TOOL));
    let judge = FakeJudge::new()
        .opt_out("done with you people", 0.95)
        .opt_out("Leave me alone", 0.5)
        .opt_out("Yes.", 0.95)
        .opt_out("Leave me be", 0.5)
        .opt_out("No, go on", 0.45);
    let world = World::new(llm, judge);
    let polite = || vec![item("Did the agent stay polite?", true)];
    let agent = world
        .agent_with(set(vec![
            scenario("phrase", "Opt out by phrase.", true, polite()),
            scenario("decision", "Opt out by decision.", true, polite()),
            scenario("unsure", "Opt out when unsure.", true, polite()),
            scenario("carry-on", "Say no and carry on.", true, polite()),
            scenario("tool", "Opt out by tool.", true, polite()),
        ]))
        .await;

    let run = run_to_end(&world, agent).await;

    let ending = |id: &str| result(&run, id).end_reason();
    assert_eq!(
        ending("phrase"),
        Some(SessionEndReason::OptOut(OptOutLayer::Phrase))
    );
    assert_eq!(
        ending("decision"),
        Some(SessionEndReason::OptOut(OptOutLayer::Decision))
    );
    assert_eq!(
        ending("unsure"),
        Some(SessionEndReason::OptOut(OptOutLayer::Decision))
    );
    assert_eq!(ending("carry-on"), Some(SessionEndReason::ClientEnded));
    assert_eq!(
        ending("tool"),
        Some(SessionEndReason::OptOut(OptOutLayer::LlmTool))
    );
    assert_eq!(
        lines(result(&run, "phrase")),
        [
            (Speaker::Agent, DISCLOSURE),
            (Speaker::Caller, "Please stop calling me."),
            (Speaker::Agent, DEFAULT_OPT_OUT_CONFIRMATION),
        ]
    );
    assert!(
        !world.llm.calls().iter().any(|c| c.side == Side::Agent
            && c.messages
                .iter()
                .any(|(_, m)| m == "Please stop calling me.")),
        "a phrase ends the conversation before the LLM answers"
    );
    // Unsure: the check question replaces the reply; a clear yes ends it.
    assert_eq!(
        lines(result(&run, "unsure")),
        [
            (Speaker::Agent, DISCLOSURE),
            (Speaker::Caller, "Leave me alone."),
            (Speaker::Agent, DEFAULT_OPT_OUT_CHECK_QUESTION),
            (Speaker::Caller, "Yes."),
            (Speaker::Agent, DEFAULT_OPT_OUT_CONFIRMATION),
        ]
    );
    // Anything else carries on with the reply, which saw the question, without asking again.
    assert_eq!(
        lines(result(&run, "carry-on")),
        [
            (Speaker::Agent, DISCLOSURE),
            (Speaker::Caller, "Leave me be."),
            (Speaker::Agent, DEFAULT_OPT_OUT_CHECK_QUESTION),
            (Speaker::Caller, "No, go on."),
            (Speaker::Agent, AGENT_REPLY),
            (Speaker::Caller, "Bye."),
        ]
    );
    assert!(world.llm.calls().iter().any(|c| {
        c.side == Side::Agent
            && c.messages.ends_with(&[
                (Role::Assistant, DEFAULT_OPT_OUT_CHECK_QUESTION.to_owned()),
                (Role::User, "No, go on.".to_owned()),
            ])
    }));
    for id in ["decision", "tool"] {
        assert_eq!(
            result(&run, id)
                .transcript()
                .last()
                .map(|l| (l.speaker(), l.text())),
            Some((Speaker::Agent, DEFAULT_OPT_OUT_CONFIRMATION)),
            "{id}: the locked confirmation is the last line"
        );
    }
    assert_eq!(
        run.verdict(),
        Some(Verdict::Passed),
        "an opt-out is a normal ending"
    );
}

#[tokio::test(start_paused = true)]
async fn scenarios_run_one_at_a_time_unless_configured_otherwise() {
    let scenarios = || {
        set(vec![
            scenario(
                "a",
                "Ask about opening hours.",
                true,
                vec![item(CONFIRMS, true)],
            ),
            scenario("b", "Ask about parking.", true, vec![item(CONFIRMS, true)]),
            scenario("c", "Ask about the menu.", true, vec![item(CONFIRMS, true)]),
        ])
    };
    let script = || {
        FakeLlm::new()
            .caller(
                "opening hours",
                vec![Move::Say("When do you open?"), Move::HangUp("Bye.")],
            )
            .caller(
                "parking",
                vec![Move::Say("Is there parking?"), Move::HangUp("Bye.")],
            )
            .caller(
                "menu",
                vec![Move::Say("What's on the menu?"), Move::HangUp("Bye.")],
            )
    };

    let default = World::new(script(), FakeJudge::new());
    assert_eq!(TestRunSettings::default().scenario_concurrency, 1);
    let agent = default.agent_with(scenarios()).await;
    assert_eq!(
        run_to_end(&default, agent).await.verdict(),
        Some(Verdict::Passed)
    );
    assert_eq!(default.llm.max_open(), 1, "one LLM request at a time");

    let settings = TestRunSettings {
        scenario_concurrency: 2,
        ..TestRunSettings::default()
    };
    let parallel = World::with_settings(script(), FakeJudge::new(), settings);
    let agent = parallel.agent_with(scenarios()).await;
    assert_eq!(
        run_to_end(&parallel, agent).await.verdict(),
        Some(Verdict::Passed)
    );
    assert_eq!(parallel.llm.max_open(), 2);
}

#[tokio::test(start_paused = true)]
async fn every_model_call_has_a_deadline_and_a_failing_model_is_an_error() {
    let llm = FakeLlm::new()
        .caller("never answers", vec![Move::Stall])
        .caller("agent fails", vec![Move::Say("Hello, anyone there?")])
        .agent("anyone there", Move::Fail)
        .caller(BOOKING, vec![Move::HangUp("Bye.")]);
    let world = World::new(llm, FakeJudge::new());
    let agent = world
        .agent_with(set(vec![
            scenario(
                "stalls",
                "The caller never answers.",
                true,
                vec![item(CONFIRMS, true)],
            ),
            scenario(
                "fails",
                "The agent fails.",
                false,
                vec![item(CONFIRMS, true)],
            ),
            scenario("books-a-table", BOOKING, false, vec![item(CONFIRMS, true)]),
        ]))
        .await;
    let settings = TestRunSettings::default();

    let started = Instant::now();
    let run = run_to_end(&world, agent).await;

    assert_eq!(
        run.status(),
        TestRunStatus::Finished,
        "the run still finishes"
    );
    let stalled = result(&run, "stalls");
    assert_eq!(stalled.status(), ScenarioStatus::Error);
    assert_eq!(stalled.end_reason(), Some(SessionEndReason::Error));
    assert_eq!(result(&run, "fails").status(), ScenarioStatus::Error);
    assert_eq!(
        result(&run, "books-a-table").status(),
        ScenarioStatus::Passed
    );
    assert_eq!(
        run.verdict(),
        Some(Verdict::Failed),
        "a required scenario errored"
    );
    assert!(
        started.elapsed() <= settings.llm_reply * 2,
        "the stalled call gave up at its deadline, not later"
    );
    for call in world.llm.calls() {
        assert!(call.deadline > Duration::ZERO && call.deadline <= settings.llm_first_token);
        assert!(call.max_tokens.is_some(), "every reply is bounded");
    }
    for call in world.judge.calls() {
        assert!(call.deadline > Duration::ZERO);
        assert!(call.deadline <= settings.decision.max(settings.rubric));
    }
}

#[tokio::test(start_paused = true)]
async fn a_rubric_without_answers_is_an_error() {
    let world = World::new(
        FakeLlm::new().caller(BOOKING, vec![Move::HangUp("Bye.")]),
        FakeJudge::new().failing_rubric(),
    );
    let agent = world
        .agent_with(set(vec![scenario(
            "books-a-table",
            BOOKING,
            true,
            vec![item(CONFIRMS, true)],
        )]))
        .await;

    let run = run_to_end(&world, agent).await;

    let played = &run.scenarios()[0];
    assert_eq!(played.status(), ScenarioStatus::Error);
    assert!(
        played
            .checks()
            .iter()
            .all(|c| c.probability().is_none() && !c.passed())
    );
    assert_eq!(run.verdict(), Some(Verdict::Failed));
}

#[tokio::test(start_paused = true)]
async fn cancelling_stops_a_run_and_skips_what_is_left() {
    let llm = FakeLlm::new().caller("never answers", vec![Move::Stall]);
    let world = World::new(llm, FakeJudge::new());
    let agent = world
        .agent_with(set(vec![
            scenario(
                "stalls",
                "The caller never answers.",
                true,
                vec![item(CONFIRMS, true)],
            ),
            scenario("later", BOOKING, true, vec![item(CONFIRMS, true)]),
        ]))
        .await;

    // A queued run.
    let queued = world.runs.start(&actor(), agent).await.expect("queued");
    let cancelled = world
        .runs
        .cancel(&actor(), queued.id())
        .await
        .expect("cancelled");
    assert_eq!(cancelled.status(), TestRunStatus::Cancelled);
    assert_eq!(cancelled.verdict(), None);
    assert!(
        cancelled
            .scenarios()
            .iter()
            .all(|r| r.status() == ScenarioStatus::Skipped)
    );
    assert_eq!(world.runs.worker().drain().await, 0, "nothing left to run");
    let again = world
        .runs
        .cancel(&actor(), queued.id())
        .await
        .expect("idempotent");
    assert_eq!(again.status(), TestRunStatus::Cancelled);
    let actions: Vec<&str> = world
        .runs_repo
        .audit_events()
        .iter()
        .map(|event| event.action().name())
        .collect();
    assert_eq!(actions, ["test_run.started", "test_run.cancelled"]);

    // A running run: the model call in flight is dropped.
    let running = world.runs.start(&actor(), agent).await.expect("queued");
    let opened = world.llm.opened();
    let request_seen = opened.notified();
    tokio::pin!(request_seen);
    request_seen.as_mut().enable();
    let worker = world.runs.worker();
    let draining = tokio::spawn(async move { worker.drain().await });
    request_seen.await;
    let stopped = world
        .runs
        .cancel(&actor(), running.id())
        .await
        .expect("cancelled");
    assert_eq!(stopped.status(), TestRunStatus::Cancelled);
    draining.await.expect("the worker stops");
    let stored = world.runs.get(running.id()).await.expect("found");
    assert_eq!(stored.status(), TestRunStatus::Cancelled);
    assert_eq!(stored.verdict(), None);
    assert!(
        stored
            .scenarios()
            .iter()
            .all(|r| r.status() == ScenarioStatus::Skipped)
    );
    assert!(
        world.llm.dropped_unfinished() >= 1,
        "the stalled request was cancelled"
    );
    assert_eq!(
        world
            .runs
            .cancel(&actor(), stored.id())
            .await
            .map(|run| run.status())
            .ok(),
        Some(TestRunStatus::Cancelled)
    );
}

#[tokio::test(start_paused = true)]
async fn a_run_left_running_by_a_stopped_server_is_interrupted() {
    let llm = FakeLlm::new().caller("never answers", vec![Move::Stall]);
    let world = World::new(llm, FakeJudge::new());
    let agent = world
        .agent_with(set(vec![scenario(
            "stalls",
            "The caller never answers.",
            true,
            vec![item(CONFIRMS, true)],
        )]))
        .await;
    let queued = world.runs.start(&actor(), agent).await.expect("queued");
    let opened = world.llm.opened();
    let request_seen = opened.notified();
    tokio::pin!(request_seen);
    request_seen.as_mut().enable();
    let worker = world.runs.worker();
    let crashed = tokio::spawn(async move { worker.drain().await });
    request_seen.await;
    crashed.abort();
    let _ = crashed.await;
    assert_eq!(
        world.runs.get(queued.id()).await.expect("found").status(),
        TestRunStatus::Running
    );

    assert_eq!(world.runs.worker().recover().await, 1);

    let stored = world.runs.get(queued.id()).await.expect("found");
    assert_eq!(stored.status(), TestRunStatus::Interrupted);
    assert_eq!(stored.verdict(), None);
    assert_eq!(world.runs.worker().drain().await, 0);
}

#[tokio::test(start_paused = true)]
async fn starting_needs_models_an_active_agent_and_no_run_in_progress() {
    let world = World::new(FakeLlm::new(), FakeJudge::new());
    let agent = world.agent(UseCase::Survey).await;

    let queued = world.runs.start(&actor(), agent).await.expect("queued");
    assert_eq!(
        queued.scenarios().len(),
        world
            .agents
            .get(agent)
            .await
            .expect("found")
            .draft()
            .scenarios_or_defaults()
            .scenarios()
            .len(),
        "the use case's default scenarios until some are saved"
    );
    assert!(matches!(
        world.runs.start(&actor(), agent).await,
        Err(StartTestRunError::AlreadyRunning)
    ));
    assert!(matches!(
        world.runs.cancel(&actor(), queued.id()).await,
        Ok(run) if run.status() == TestRunStatus::Cancelled
    ));
    let fresh = world
        .runs
        .start(&actor(), agent)
        .await
        .expect("a new run once none is active");
    world
        .runs
        .cancel(&actor(), fresh.id())
        .await
        .expect("cancelled");

    let unknown = vak_domain::AgentId::from_uuid(uuid::Uuid::now_v7()).expect("v7");
    assert!(matches!(
        world.runs.start(&actor(), unknown).await,
        Err(StartTestRunError::NotFound)
    ));
    let archived = world.agent(UseCase::Support).await;
    world
        .agents
        .archive(&actor(), archived)
        .await
        .expect("archived");
    assert!(matches!(
        world.runs.start(&actor(), archived).await,
        Err(StartTestRunError::Archived)
    ));
    let unknown_run = vak_domain::TestRunId::from_uuid(uuid::Uuid::now_v7()).expect("v7");
    assert!(matches!(
        world.runs.get(unknown_run).await,
        Err(FindTestRunError::NotFound)
    ));
    assert!(matches!(
        world.runs.cancel(&actor(), unknown_run).await,
        Err(CancelTestRunError::NotFound)
    ));

    // A finished run can't be cancelled.
    let done = world.agent(UseCase::Booking).await;
    let finished = run_to_end(&world, done).await;
    assert_eq!(finished.status(), TestRunStatus::Finished);
    assert!(matches!(
        world.runs.cancel(&actor(), finished.id()).await,
        Err(CancelTestRunError::Finished)
    ));

    // Without models, nothing runs.
    let runs = std::sync::Arc::new(InMemoryTestRunRepo::default());
    let agents = AgentService::new(std::sync::Arc::new(InMemoryAgentRepo::default()))
        .with_test_runs(runs.clone());
    let offline = TestRunService::new(agents.clone(), runs, None);
    let command = vak_services::CreateAgent {
        name: AgentName::parse("Front desk").expect("valid"),
        use_case: UseCase::Survey,
        voice: vak_domain::VoiceId::parse("af_heart").expect("valid"),
        language: None,
        jurisdiction: None,
        model: None,
        turn_detection: None,
    };
    let id = agents
        .create(&actor(), command)
        .await
        .expect("created")
        .id();
    assert!(matches!(
        offline.start(&actor(), id).await,
        Err(StartTestRunError::ModelsUnavailable)
    ));
}
