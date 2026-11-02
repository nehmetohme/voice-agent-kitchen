//! Contract `plan` v1 (domain): the plan graph and its validation, a template per use case,
//! versioned rule packs, and the Plan step of setup progress.
//!
//! - A plan is steps (start, say, ask, collect, tool, transfer, end) joined by paths, each
//!   with a plain-language condition. IDs, labels, texts and sizes are checked when it is
//!   built (`PlanShapeError`); graph rules are checked by `validate` (`PlanError`).
//! - Validation reports every broken rule, each located on its step or path: exactly one
//!   start step; every step reachable from a start; every step but an end has a path out;
//!   every path has a condition and joins two existing steps; and each start step opens with
//!   the rule pack's locked texts, word for word and in the pack's order. A reference model in
//!   this file pins the rules on random graphs (property-based).
//! - Every use case has a template that is valid for any rule pack.
//! - Rule packs are versioned data: a pack is checked when it is built, a catalog holds each
//!   (id, version) once, and the version an agent pins is the one its plan is checked against.
//!   Besides the locked opening, a pack has an opt-out confirmation, said word for word when
//!   the caller opts out, checked like a locked text.
//! - A saved plan is part of the draft and its content hash. Plan is done when the saved plan
//!   validates against the agent's pinned rule pack: a new agent stays at 25 %, a valid saved
//!   plan makes 50 %.
//!
//! Contract file: never edit it once it is tagged `contract/plan/v1`. To change the contract,
//! add a new version.
#![expect(
    clippy::expect_used,
    reason = "fixture helpers outside #[test] functions fail the test on invalid input"
)]

use std::collections::{HashSet, VecDeque};

use jiff::civil::date;
use proptest::prelude::*;
use vak_domain::{
    AgentDraft, AgentName, DraftField, DraftPatch, EdgeId, Jurisdiction, LockedText, NodeId,
    NodeKind, Plan, PlanEdge, PlanError, PlanNode, PlanShapeError, ProgressFacts, Retention,
    RulePack, RulePackCatalog, RulePackError, RulePackRef, RulePackSpec, SetupProgress, SetupStep,
    UseCase, VoiceId,
};

const DISCLOSURE: &str = "Hello, you're speaking with an AI assistant.";
const NOTICE: &str = "This call is recorded and transcribed.";
const CONFIRMATION: &str = "Understood. We won't contact you again. Goodbye.";

fn node_id(raw: &str) -> NodeId {
    NodeId::parse(raw).expect("valid step ID")
}

fn edge_id(raw: &str) -> EdgeId {
    EdgeId::parse(raw).expect("valid path ID")
}

/// A step labelled after its ID.
fn step(id: &str, kind: NodeKind, text: &str) -> PlanNode {
    PlanNode::new(node_id(id), kind, &format!("Step {id}"), text).expect("valid step")
}

fn path(id: &str, from: &str, to: &str, condition: &str) -> PlanEdge {
    PlanEdge::new(edge_id(id), node_id(from), node_id(to), condition).expect("valid path")
}

fn plan(nodes: Vec<PlanNode>, edges: Vec<PlanEdge>) -> Plan {
    Plan::new(nodes, edges).expect("well-formed plan")
}

fn locked(texts: &[(&str, &str)]) -> Vec<LockedText> {
    texts
        .iter()
        .map(|(key, text)| LockedText::new(key, text).expect("valid locked text"))
        .collect()
}

fn spec(
    jurisdiction: &str,
    use_case: UseCase,
    version: u32,
    texts: &[(&str, &str)],
) -> RulePackSpec {
    let jurisdiction = Jurisdiction::parse(jurisdiction).expect("valid jurisdiction");
    RulePackSpec {
        id: RulePackRef::for_agent(use_case, &jurisdiction)
            .id()
            .to_owned(),
        version,
        jurisdiction,
        use_case,
        effective: date(2026, 11, 1),
        locked: locked(texts),
        opt_out_phrases: vec!["take me off your list".to_owned()],
        opt_out_confirmation: CONFIRMATION.to_owned(),
        forbidden_claims: vec!["Saying it is a human.".to_owned()],
        retention: Retention::new(180, 365, 1_826).expect("valid retention"),
    }
}

const STANDARD: [(&str, &str); 2] = [
    (LockedText::AI_DISCLOSURE, DISCLOSURE),
    (LockedText::RECORDING_NOTICE, NOTICE),
];

/// `us-support` v1 with the two required locked texts.
fn pack() -> RulePack {
    RulePack::new(spec("US", UseCase::Support, 1, &STANDARD)).expect("valid pack")
}

/// The pack's locked texts, then `rest`.
fn opening(pack: &RulePack, rest: &str) -> String {
    let mut parts: Vec<&str> = pack.locked_texts().iter().map(LockedText::text).collect();
    parts.push(rest);
    parts.join(" ")
}

/// start → ask → end: the smallest plan that asks something.
fn minimal(pack: &RulePack) -> Plan {
    plan(
        vec![
            step("start", NodeKind::Start, &opening(pack, "How can I help?")),
            step("ask", NodeKind::Ask, "What do you need?"),
            step("bye", NodeKind::End, "Goodbye."),
        ],
        vec![
            path("p1", "start", "ask", "The caller answers the greeting"),
            path("p2", "ask", "bye", "The caller has what they need"),
        ],
    )
}

fn errors_of(result: Result<(), Vec<PlanError>>) -> Vec<PlanError> {
    result.err().unwrap_or_default()
}

/// The errors as a set, after checking there are no duplicates.
fn error_set(result: Result<(), Vec<PlanError>>) -> HashSet<PlanError> {
    let errors = errors_of(result);
    let set: HashSet<PlanError> = errors.iter().cloned().collect();
    assert_eq!(set.len(), errors.len(), "duplicate errors: {errors:?}");
    set
}

#[test]
fn a_plan_that_follows_every_rule_is_valid() {
    let pack = pack();

    assert_eq!(minimal(&pack).validate(&pack), Ok(()));
}

#[test]
fn a_plan_has_exactly_one_start_step() {
    let pack = pack();
    let no_start = plan(vec![step("bye", NodeKind::End, "Goodbye.")], vec![]);
    let two_starts = plan(
        vec![
            step("a", NodeKind::Start, &opening(&pack, "Hi.")),
            step("b", NodeKind::Start, &opening(&pack, "Hello.")),
            step("bye", NodeKind::End, "Goodbye."),
        ],
        vec![
            path("p1", "a", "bye", "Always"),
            path("p2", "b", "bye", "Always"),
        ],
    );

    assert_eq!(no_start.validate(&pack), Err(vec![PlanError::MissingStart]));
    assert_eq!(
        Plan::new(vec![], vec![]).map(|p| p.validate(&pack)),
        Ok(Err(vec![PlanError::MissingStart]))
    );
    assert_eq!(
        error_set(two_starts.validate(&pack)),
        HashSet::from([
            PlanError::MultipleStarts { node: node_id("a") },
            PlanError::MultipleStarts { node: node_id("b") },
        ])
    );
}

#[test]
fn every_step_is_reachable_from_the_start() {
    let pack = pack();
    let mut nodes = minimal(&pack).nodes().to_vec();
    nodes.push(step("orphan", NodeKind::End, "Goodbye."));
    nodes.push(step("island", NodeKind::Say, "Nobody hears this."));
    let mut edges = minimal(&pack).edges().to_vec();
    // A path out of an unreachable step doesn't make it reachable, nor what it leads to.
    edges.push(path("p3", "island", "orphan", "Always"));

    assert_eq!(
        error_set(plan(nodes, edges).validate(&pack)),
        HashSet::from([
            PlanError::UnreachableNode {
                node: node_id("orphan")
            },
            PlanError::UnreachableNode {
                node: node_id("island")
            },
        ])
    );
}

#[test]
fn every_step_but_an_end_has_a_path_out() {
    let pack = pack();
    let looping = plan(
        vec![
            step("start", NodeKind::Start, &opening(&pack, "Hi.")),
            step("ask", NodeKind::Ask, "Anything else?"),
            step("tool", NodeKind::Tool, "Look up the order."),
        ],
        vec![
            path("p1", "start", "ask", "Always"),
            path("p2", "ask", "tool", "The caller gives an order number"),
        ],
    );

    assert_eq!(
        looping.validate(&pack),
        Err(vec![PlanError::MissingExit {
            node: node_id("tool")
        }])
    );
    // Cycles are fine, and an end step needs no path out.
    let mut edges = looping.edges().to_vec();
    edges.push(path("p3", "tool", "ask", "The order is found"));
    assert_eq!(
        plan(looping.nodes().to_vec(), edges).validate(&pack),
        Ok(())
    );
}

#[test]
fn every_path_has_a_condition_and_joins_two_existing_steps() {
    let pack = pack();
    let mut edges = minimal(&pack).edges().to_vec();
    edges.push(path("blank", "ask", "bye", "   "));
    edges.push(path(
        "ghost",
        "ask",
        "nowhere",
        "The caller asks for nobody",
    ));
    edges.push(path("from-ghost", "nowhere", "bye", "Always"));

    let condition = edges
        .iter()
        .find(|edge| edge.id().as_str() == "blank")
        .map(PlanEdge::condition);
    assert_eq!(condition, Some(""), "conditions are trimmed");
    assert_eq!(
        error_set(plan(minimal(&pack).nodes().to_vec(), edges).validate(&pack)),
        HashSet::from([
            PlanError::EmptyCondition {
                edge: edge_id("blank")
            },
            PlanError::DanglingEdge {
                edge: edge_id("ghost")
            },
            PlanError::DanglingEdge {
                edge: edge_id("from-ghost")
            },
        ])
    );
}

/// The minimal plan with `text` as its opening.
fn opening_with(pack: &RulePack, text: &str) -> Plan {
    let mut nodes = minimal(pack).nodes().to_vec();
    nodes[0] = step("start", NodeKind::Start, text);
    plan(nodes, minimal(pack).edges().to_vec())
}

fn missing(keys: &[&str]) -> Result<(), Vec<PlanError>> {
    let errors: Vec<PlanError> = keys
        .iter()
        .map(|key| PlanError::MissingLockedText {
            node: node_id("start"),
            key: (*key).to_owned(),
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[test]
fn the_start_step_opens_with_the_locked_texts_word_for_word_and_in_order() {
    let pack = pack();
    let (ai, notice) = (LockedText::AI_DISCLOSURE, LockedText::RECORDING_NOTICE);
    let cases = [
        (
            format!("{DISCLOSURE} {NOTICE} How can I help?"),
            missing(&[]),
        ),
        (format!("{DISCLOSURE}\n\n{NOTICE}"), missing(&[])),
        (format!("{DISCLOSURE}{NOTICE}"), missing(&[])),
        (format!("{DISCLOSURE} How can I help?"), missing(&[notice])),
        (format!("{NOTICE} {DISCLOSURE}"), missing(&[ai])),
        (format!("Hi! {DISCLOSURE} {NOTICE}"), missing(&[ai, notice])),
        (
            // Word for word: a lower-case disclosure is missing, and so the notice isn't next.
            format!("{} {NOTICE}", DISCLOSURE.to_lowercase()),
            missing(&[ai, notice]),
        ),
        ("How can I help?".to_owned(), missing(&[ai, notice])),
    ];

    for (text, expected) in cases {
        assert_eq!(
            opening_with(&pack, &text).validate(&pack),
            expected,
            "{text:?}"
        );
    }
}

#[test]
fn errors_name_their_rule_and_where_they_are() {
    let (node, edge) = (node_id("ask"), edge_id("p1"));
    let pack_ref = RulePackRef::new("zz-support", 1).expect("valid reference");
    let cases = [
        (PlanError::MissingStart, "missing_start", None, None),
        (
            PlanError::MultipleStarts { node: node.clone() },
            "multiple_starts",
            Some(&node),
            None,
        ),
        (
            PlanError::UnreachableNode { node: node.clone() },
            "unreachable_node",
            Some(&node),
            None,
        ),
        (
            PlanError::MissingExit { node: node.clone() },
            "missing_exit",
            Some(&node),
            None,
        ),
        (
            PlanError::EmptyCondition { edge: edge.clone() },
            "empty_condition",
            None,
            Some(&edge),
        ),
        (
            PlanError::DanglingEdge { edge: edge.clone() },
            "dangling_edge",
            None,
            Some(&edge),
        ),
        (
            PlanError::MissingLockedText {
                node: node.clone(),
                key: "ai_disclosure".to_owned(),
            },
            "missing_locked_text",
            Some(&node),
            None,
        ),
        (
            PlanError::RulePackUnavailable { pack: pack_ref },
            "rule_pack_unavailable",
            None,
            None,
        ),
    ];

    for (error, code, on_node, on_edge) in cases {
        assert_eq!(error.code(), code);
        assert_eq!(error.node(), on_node, "{code}");
        assert_eq!(error.edge(), on_edge, "{code}");
        assert!(!error.to_string().is_empty(), "{code}");
    }
}

#[test]
fn step_and_path_ids_are_checked() {
    for raw in ["start", "step-2", "Ask_Need", "7", &"a".repeat(NodeId::MAX)] {
        assert_eq!(
            NodeId::parse(raw).map(|id| id.as_str().to_owned()),
            Ok(raw.to_owned())
        );
        assert_eq!(
            EdgeId::parse(raw).map(|id| id.as_str().to_owned()),
            Ok(raw.to_owned())
        );
    }
    for raw in [
        "",
        " start",
        "a b",
        "é",
        "step.2",
        &"a".repeat(NodeId::MAX + 1),
    ] {
        assert_eq!(
            NodeId::parse(raw),
            Err(PlanShapeError::InvalidId),
            "{raw:?}"
        );
        assert_eq!(
            EdgeId::parse(raw),
            Err(PlanShapeError::InvalidId),
            "{raw:?}"
        );
    }
    assert_eq!((NodeId::MAX, EdgeId::MAX), (64, 64));
    assert_ne!(node_id("Start"), node_id("start"), "IDs are case-sensitive");
}

#[test]
fn step_kinds_are_a_short_fixed_list_parsed_liberally() {
    let names: Vec<&str> = NodeKind::ALL.iter().map(|kind| kind.as_str()).collect();

    assert_eq!(
        names,
        ["start", "say", "ask", "collect", "tool", "transfer", "end"]
    );
    for kind in NodeKind::ALL {
        assert_eq!(NodeKind::parse(kind.as_str()), Ok(kind));
    }
    assert_eq!(NodeKind::parse(" Ask "), Ok(NodeKind::Ask));
    assert_eq!(NodeKind::parse("TRANSFER"), Ok(NodeKind::Transfer));
    assert_eq!(NodeKind::parse("dance"), Err(PlanShapeError::UnknownKind));
    assert_eq!(NodeKind::parse(""), Err(PlanShapeError::UnknownKind));
}

#[test]
fn steps_and_paths_are_trimmed_and_their_sizes_checked() {
    let new = |label: &str, text: &str| PlanNode::new(node_id("s"), NodeKind::Say, label, text);
    let long = |n: usize| "é".repeat(n);

    let trimmed = new("  Greeting \n", "  Hello there.  ").expect("valid step");
    assert_eq!(
        (trimmed.label(), trimmed.text()),
        ("Greeting", "Hello there.")
    );
    assert_eq!(
        (trimmed.id(), trimmed.kind()),
        (&node_id("s"), NodeKind::Say)
    );
    assert!(new(&long(PlanNode::MAX_LABEL), &long(PlanNode::MAX_TEXT)).is_ok());
    assert!(new("Silent", "").is_ok(), "a step's text may be empty");
    assert_eq!(new(" ", ""), Err(PlanShapeError::EmptyLabel));
    assert_eq!(
        new(&long(81), ""),
        Err(PlanShapeError::LabelTooLong { max: 80 })
    );
    assert_eq!(new("Bell\u{7}", ""), Err(PlanShapeError::ControlCharacter));
    assert_eq!(
        new("Long", &long(2_001)),
        Err(PlanShapeError::TextTooLong { max: 2_000 })
    );

    let edge = |condition: &str| PlanEdge::new(edge_id("p"), node_id("a"), node_id("b"), condition);
    let kept = edge(" The caller agrees ").expect("valid path");
    assert_eq!(
        (kept.id(), kept.from(), kept.to(), kept.condition()),
        (
            &edge_id("p"),
            &node_id("a"),
            &node_id("b"),
            "The caller agrees"
        )
    );
    assert!(edge(&long(PlanEdge::MAX_CONDITION)).is_ok());
    assert_eq!(
        edge(&long(201)),
        Err(PlanShapeError::ConditionTooLong { max: 200 })
    );
}

#[test]
fn a_plan_has_unique_ids_and_at_most_50_steps_and_200_paths() {
    let steps = |n: usize| -> Vec<PlanNode> {
        (0..n)
            .map(|i| step(&format!("s{i}"), NodeKind::Say, ""))
            .collect()
    };
    let paths = |n: usize| -> Vec<PlanEdge> {
        (0..n)
            .map(|i| path(&format!("p{i}"), "s0", "s1", "Always"))
            .collect()
    };

    assert_eq!((Plan::MAX_NODES, Plan::MAX_EDGES), (50, 200));
    let full = Plan::new(steps(50), paths(200)).expect("at the limits");
    assert_eq!(
        (full.nodes(), full.edges()),
        (steps(50).as_slice(), paths(200).as_slice())
    );
    assert_eq!(
        Plan::new(steps(51), vec![]),
        Err(PlanShapeError::TooManyNodes { max: 50 })
    );
    assert_eq!(
        Plan::new(steps(2), paths(201)),
        Err(PlanShapeError::TooManyEdges { max: 200 })
    );
    let mut twins = steps(3);
    twins.push(step("s1", NodeKind::End, ""));
    assert_eq!(
        Plan::new(twins, vec![]),
        Err(PlanShapeError::DuplicateNode { id: node_id("s1") })
    );
    let mut twin_paths = paths(2);
    twin_paths.push(path("p0", "s1", "s0", "Back"));
    assert_eq!(
        Plan::new(steps(2), twin_paths),
        Err(PlanShapeError::DuplicateEdge { id: edge_id("p0") })
    );
}

// --- The reference model: the rules, stated once more, independently of the domain. ---

/// Which locked texts `text` fails to open with: walking the pack's texts in order, each
/// must come next, after optional whitespace.
fn model_missing_locked(text: &str, pack: &RulePack) -> Vec<String> {
    let mut rest = text.trim_start();
    let mut missing = Vec::new();
    for locked in pack.locked_texts() {
        match rest.strip_prefix(locked.text()) {
            Some(after) => rest = after.trim_start(),
            None => missing.push(locked.key().to_owned()),
        }
    }
    missing
}

fn model_errors(plan: &Plan, pack: &RulePack) -> HashSet<PlanError> {
    let ids: HashSet<&NodeId> = plan.nodes().iter().map(PlanNode::id).collect();
    let starts: Vec<&PlanNode> = plan
        .nodes()
        .iter()
        .filter(|n| n.kind() == NodeKind::Start)
        .collect();
    let mut errors = HashSet::new();
    match starts.len() {
        0 => {
            errors.insert(PlanError::MissingStart);
        }
        1 => {}
        _ => errors.extend(starts.iter().map(|s| PlanError::MultipleStarts {
            node: s.id().clone(),
        })),
    }
    if !starts.is_empty() {
        let mut reached: HashSet<&NodeId> = starts.iter().map(|s| s.id()).collect();
        let mut queue: VecDeque<&NodeId> = reached.iter().copied().collect();
        while let Some(at) = queue.pop_front() {
            for edge in plan
                .edges()
                .iter()
                .filter(|e| e.from() == at && ids.contains(e.to()))
            {
                if reached.insert(edge.to()) {
                    queue.push_back(edge.to());
                }
            }
        }
        let unreached = plan.nodes().iter().filter(|n| !reached.contains(n.id()));
        errors.extend(unreached.map(|n| PlanError::UnreachableNode {
            node: n.id().clone(),
        }));
    }
    for node in plan.nodes().iter().filter(|n| n.kind() != NodeKind::End) {
        if !plan.edges().iter().any(|e| e.from() == node.id()) {
            errors.insert(PlanError::MissingExit {
                node: node.id().clone(),
            });
        }
    }
    for edge in plan.edges() {
        if edge.condition().trim().is_empty() {
            errors.insert(PlanError::EmptyCondition {
                edge: edge.id().clone(),
            });
        }
        if !ids.contains(edge.from()) || !ids.contains(edge.to()) {
            errors.insert(PlanError::DanglingEdge {
                edge: edge.id().clone(),
            });
        }
    }
    for start in &starts {
        errors.extend(
            model_missing_locked(start.text(), pack)
                .into_iter()
                .map(|key| PlanError::MissingLockedText {
                    node: start.id().clone(),
                    key,
                }),
        );
    }
    errors
}

const CONDITIONS: [&str; 4] = ["", "   ", "Always", "The caller agrees"];

/// Openings that keep, drop or reorder the locked texts of [`pack`].
fn openings() -> [String; 5] {
    [
        format!("{DISCLOSURE} {NOTICE} How can I help?"),
        format!("{DISCLOSURE} How can I help?"),
        format!("{NOTICE} {DISCLOSURE}"),
        "Hello there.".to_owned(),
        String::new(),
    ]
}

/// Random graphs of 0–8 steps of any kind and up to 3 paths per step; a path end of `n`
/// names a step that doesn't exist.
fn graphs() -> impl Strategy<Value = Plan> {
    (0_usize..=8)
        .prop_flat_map(|n| {
            (
                prop::collection::vec(
                    (
                        prop::sample::select(NodeKind::ALL.to_vec()),
                        0..openings().len(),
                    ),
                    n,
                ),
                prop::collection::vec((0..=n, 0..=n, 0..CONDITIONS.len()), 0..=3 * n),
            )
        })
        .prop_map(|(steps, paths)| {
            let id = |i: usize| {
                if i < steps.len() {
                    format!("s{i}")
                } else {
                    "ghost".to_owned()
                }
            };
            let nodes = steps
                .iter()
                .enumerate()
                .map(|(i, (kind, text))| step(&id(i), *kind, &openings()[*text]))
                .collect();
            let edges = paths
                .iter()
                .enumerate()
                .map(|(i, (from, to, c))| {
                    path(&format!("p{i}"), &id(*from), &id(*to), CONDITIONS[*c])
                })
                .collect();
            plan(nodes, edges)
        })
}

/// Rule packs for any use case and jurisdiction, with 2–3 locked texts of random words.
fn packs() -> impl Strategy<Value = RulePack> {
    let sentence = "[A-Z][a-z]{1,10}( [a-z]{1,10}){0,8}[.!]";
    (
        prop::sample::select(UseCase::ALL.to_vec()),
        prop::sample::select(vec!["US", "US-CA", "UK", "EU-FR"]),
        1_u32..=9,
        (sentence, sentence, prop::option::of(sentence)),
    )
        .prop_map(|(use_case, jurisdiction, version, (ai, notice, extra))| {
            let mut texts = vec![
                (LockedText::AI_DISCLOSURE, ai),
                (LockedText::RECORDING_NOTICE, notice),
            ];
            texts.extend(extra.map(|text| ("opt_out_notice", text)));
            let texts: Vec<(&str, &str)> = texts.iter().map(|(k, t)| (*k, t.as_str())).collect();
            RulePack::new(spec(jurisdiction, use_case, version, &texts)).expect("valid pack")
        })
}

proptest! {
    #[test]
    fn validation_reports_exactly_the_rules_a_plan_breaks(plan in graphs()) {
        let pack = pack();

        let result = plan.validate(&pack);

        let expected = model_errors(&plan, &pack);
        prop_assert_eq!(result.is_ok(), expected.is_empty());
        prop_assert_eq!(error_set(result), expected);
    }

    #[test]
    fn every_error_is_on_a_step_or_path_of_the_plan(plan in graphs()) {
        for error in errors_of(plan.validate(&pack())) {
            if let Some(node) = error.node() {
                prop_assert!(plan.nodes().iter().any(|n| n.id() == node), "{error:?}");
            }
            if let Some(edge) = error.edge() {
                prop_assert!(plan.edges().iter().any(|e| e.id() == edge), "{error:?}");
            }
        }
    }

    #[test]
    fn the_order_of_steps_and_paths_does_not_change_the_result(plan in graphs()) {
        let pack = pack();
        let mut nodes = plan.nodes().to_vec();
        let mut edges = plan.edges().to_vec();
        nodes.reverse();
        edges.reverse();

        let reordered = Plan::new(nodes, edges).expect("same steps and paths");

        prop_assert_eq!(error_set(reordered.validate(&pack)), error_set(plan.validate(&pack)));
    }

    #[test]
    fn every_template_is_valid_for_any_rule_pack(pack in packs(), use_case in prop::sample::select(UseCase::ALL.to_vec())) {
        let template = Plan::template(use_case, pack.locked_texts());

        prop_assert_eq!(template.validate(&pack), Ok(()));
        prop_assert!(template.nodes().iter().any(|n| n.kind() == NodeKind::End), "{use_case:?}");
        prop_assert_eq!(&template, &Plan::template(use_case, pack.locked_texts()), "deterministic");
    }
}

#[test]
fn each_use_case_has_its_own_template_and_custom_starts_blank() {
    let pack = pack();
    let templates: Vec<Plan> = UseCase::ALL
        .iter()
        .map(|use_case| Plan::template(*use_case, pack.locked_texts()))
        .collect();

    let distinct: HashSet<&Plan> = templates.iter().collect();
    assert_eq!(
        distinct.len(),
        UseCase::ALL.len(),
        "one template per use case"
    );
    let blank = Plan::template(UseCase::Custom, pack.locked_texts());
    let kinds: Vec<NodeKind> = blank.nodes().iter().map(PlanNode::kind).collect();
    assert_eq!(
        kinds,
        [NodeKind::Start, NodeKind::End],
        "custom is a start and an end"
    );
    for template in &templates {
        assert!(template.nodes().len() <= Plan::MAX_NODES);
        assert_eq!(template.validate(&pack), Ok(()));
    }
}

#[test]
fn a_rule_pack_is_checked_when_it_is_built() {
    let built = pack();
    assert_eq!(
        built.reference(),
        &RulePackRef::new("us-support", 1).expect("valid")
    );
    assert_eq!(
        (
            built.jurisdiction().as_str(),
            built.use_case(),
            built.effective()
        ),
        ("US", UseCase::Support, date(2026, 11, 1))
    );
    let keys: Vec<&str> = built.locked_texts().iter().map(LockedText::key).collect();
    assert_eq!(
        keys,
        [LockedText::AI_DISCLOSURE, LockedText::RECORDING_NOTICE]
    );
    assert_eq!(built.forbidden_claims(), ["Saying it is a human."]);
    let retention = built.retention();
    assert_eq!(
        (
            retention.recordings_days(),
            retention.transcripts_days(),
            retention.audit_days()
        ),
        (180, 365, 1_826)
    );

    let with = |change: fn(&mut RulePackSpec)| {
        let mut spec = spec("US", UseCase::Support, 1, &STANDARD);
        change(&mut spec);
        RulePack::new(spec)
    };
    assert_eq!(
        with(|s| s.id = "us-booking".to_owned()),
        Err(RulePackError::IdMismatch {
            expected: "us-support".to_owned()
        })
    );
    assert_eq!(with(|s| s.version = 0), Err(RulePackError::InvalidVersion));
    assert_eq!(
        with(|s| s.locked.retain(|t| t.key() != LockedText::AI_DISCLOSURE)),
        Err(RulePackError::MissingLockedText {
            key: LockedText::AI_DISCLOSURE.to_owned()
        })
    );
    assert_eq!(
        with(|s| s.locked.retain(|t| t.key() != LockedText::RECORDING_NOTICE)),
        Err(RulePackError::MissingLockedText {
            key: LockedText::RECORDING_NOTICE.to_owned()
        })
    );
    assert_eq!(
        with(|s| s.locked.push(s.locked[0].clone())),
        Err(RulePackError::DuplicateLockedText {
            key: LockedText::AI_DISCLOSURE.to_owned()
        })
    );
    assert_eq!(
        with(|s| s.opt_out_phrases.clear()),
        Err(RulePackError::NoOptOutPhrases)
    );
    assert!(with(|s| s.opt_out_phrases.push("  ".to_owned())).is_err());
    let liberal = with(|s| s.opt_out_phrases = vec![" Take me OFF your list ".to_owned()]);
    assert_eq!(
        liberal.map(|p| p.opt_out_phrases().to_vec()),
        Ok(vec!["take me off your list".to_owned()])
    );
}

#[test]
fn the_opt_out_confirmation_is_checked_like_a_locked_text() {
    let with = |confirmation: String| {
        let mut spec = spec("US", UseCase::Support, 1, &STANDARD);
        spec.opt_out_confirmation = confirmation;
        RulePack::new(spec)
    };

    let built = pack();
    assert_eq!(built.opt_out_confirmation(), CONFIRMATION);
    let keys: Vec<&str> = built.locked_texts().iter().map(LockedText::key).collect();
    assert!(
        !keys.contains(&"opt_out_confirmation"),
        "not part of the opening"
    );
    assert_eq!(
        with(format!("  {CONFIRMATION} \n")).map(|p| p.opt_out_confirmation().to_owned()),
        Ok(CONFIRMATION.to_owned())
    );
    let longest = "é".repeat(LockedText::MAX);
    assert_eq!(
        with(longest.clone()).map(|p| p.opt_out_confirmation().to_owned()),
        Ok(longest)
    );
    for invalid in [
        String::new(),
        " \n ".to_owned(),
        "é".repeat(LockedText::MAX + 1),
    ] {
        assert_eq!(with(invalid), Err(RulePackError::InvalidOptOutConfirmation));
    }
}

#[test]
fn locked_texts_and_retention_are_checked() {
    let text = LockedText::new("opt_out_notice", "  Say 'stop' at any time.  ").expect("valid");
    assert_eq!(
        (text.key(), text.text()),
        ("opt_out_notice", "Say 'stop' at any time.")
    );
    for key in ["", "AI_disclosure", "ai disclosure", &"k".repeat(33)] {
        assert!(LockedText::new(key, DISCLOSURE).is_err(), "{key:?}");
    }
    assert!(LockedText::new(&"k".repeat(32), DISCLOSURE).is_ok());
    assert!(LockedText::new(LockedText::AI_DISCLOSURE, " \n ").is_err());
    assert_eq!(LockedText::MAX, 500);
    let longest = "é".repeat(LockedText::MAX);
    assert!(LockedText::new(LockedText::AI_DISCLOSURE, &longest).is_ok());
    assert!(LockedText::new(LockedText::AI_DISCLOSURE, &format!("{longest}é")).is_err());
    assert_eq!(
        Retention::new(0, 365, 1_826),
        Err(RulePackError::InvalidRetention)
    );
    assert_eq!(
        Retention::new(180, 0, 1_826),
        Err(RulePackError::InvalidRetention)
    );
    assert_eq!(
        Retention::new(180, 365, 0),
        Err(RulePackError::InvalidRetention)
    );
}

/// `us-support` v2: a reworded recording notice.
fn pack_v2() -> RulePack {
    let texts = [
        (LockedText::AI_DISCLOSURE, DISCLOSURE),
        (LockedText::RECORDING_NOTICE, "This call is recorded."),
    ];
    RulePack::new(spec("US", UseCase::Support, 2, &texts)).expect("valid pack")
}

#[test]
fn a_catalog_holds_each_version_once_and_resolves_the_pinned_one() {
    let california = RulePack::new(spec("US-CA", UseCase::Booking, 1, &STANDARD)).expect("valid");
    let catalog =
        RulePackCatalog::new(vec![pack_v2(), california.clone(), pack()]).expect("distinct");
    let reference = |id: &str, version: u32| RulePackRef::new(id, version).expect("valid");

    assert_eq!((catalog.len(), catalog.is_empty()), (3, false));
    assert_eq!(catalog.get(&reference("us-support", 1)), Some(&pack()));
    assert_eq!(catalog.get(&reference("us-support", 2)), Some(&pack_v2()));
    assert_eq!(
        catalog.get(&reference("us-ca-booking", 1)),
        Some(&california)
    );
    assert_eq!(catalog.get(&reference("us-support", 3)), None);
    assert_eq!(catalog.get(&reference("uk-support", 1)), None);
    assert_eq!(catalog.latest("us-support"), Some(&pack_v2()));
    assert_eq!(catalog.latest("uk-support"), None);
    let listed: Vec<(&str, u32)> = catalog
        .iter()
        .map(|p| (p.reference().id(), p.reference().version()))
        .collect();
    assert_eq!(
        listed,
        [("us-ca-booking", 1), ("us-support", 1), ("us-support", 2)]
    );
    assert!(RulePackCatalog::default().is_empty());

    // A published version never changes: a second definition of it is refused, whatever it says.
    let rewritten = RulePack::new(spec(
        "US",
        UseCase::Support,
        1,
        &[
            (LockedText::AI_DISCLOSURE, "Something else."),
            (LockedText::RECORDING_NOTICE, NOTICE),
        ],
    ))
    .expect("valid");
    assert_eq!(
        RulePackCatalog::new(vec![pack(), rewritten]),
        Err(RulePackError::Duplicate {
            id: "us-support".to_owned(),
            version: 1
        })
    );
}

#[test]
fn a_plan_is_checked_against_the_pinned_version_of_its_rule_pack() {
    let catalog = RulePackCatalog::new(vec![pack(), pack_v2()]).expect("distinct");
    let v1 = RulePackRef::new("us-support", 1).expect("valid");
    let v2 = RulePackRef::new("us-support", 2).expect("valid");
    let unknown = RulePackRef::new("zz-support", 1).expect("valid");
    let written_for_v1 = minimal(&pack());

    assert_eq!(catalog.validate(&written_for_v1, &v1), Ok(()));
    assert_eq!(
        catalog.validate(&written_for_v1, &v2),
        Err(vec![PlanError::MissingLockedText {
            node: node_id("start"),
            key: LockedText::RECORDING_NOTICE.to_owned()
        }])
    );
    assert_eq!(catalog.validate(&minimal(&pack_v2()), &v2), Ok(()));
    assert_eq!(
        catalog.validate(&written_for_v1, &unknown),
        Err(vec![PlanError::RulePackUnavailable {
            pack: unknown.clone()
        }])
    );
}

fn front_desk() -> AgentDraft {
    AgentDraft::new(
        AgentName::parse("Front desk").expect("valid name"),
        UseCase::Support,
        VoiceId::parse("af_heart").expect("valid voice"),
    )
}

fn with_plan(draft: &AgentDraft, plan: Plan) -> AgentDraft {
    draft.apply(&DraftPatch {
        plan: Some(plan),
        ..DraftPatch::default()
    })
}

#[test]
fn a_saved_plan_is_part_of_the_draft_and_its_content_hash() {
    let draft = front_desk();
    let template = Plan::template(UseCase::Support, pack().locked_texts());

    let saved = with_plan(&draft, template.clone());

    assert_eq!(draft.plan(), None, "a new draft has no saved plan");
    assert_eq!(saved.plan(), Some(&template));
    assert_eq!(draft.changed_fields(&saved), [DraftField::Plan]);
    assert_eq!(DraftField::Plan.as_str(), "plan");
    assert_ne!(saved.content_hash(), draft.content_hash());
    assert_eq!(
        with_plan(&saved, template.clone()),
        saved,
        "saving the same plan changes nothing"
    );
    assert_eq!(saved.apply(&DraftPatch::default()), saved);
    let edited = with_plan(&saved, minimal(&pack()));
    assert_ne!(edited.content_hash(), saved.content_hash());
    // The operator's plan stays when the use case or jurisdiction moves the rule pack.
    let moved = saved.apply(&DraftPatch {
        use_case: Some(UseCase::Booking),
        jurisdiction: Some(Jurisdiction::parse("US-CA").expect("valid")),
        ..DraftPatch::default()
    });
    assert_eq!(moved.plan(), Some(&template));
    assert_eq!(moved.rule_pack().id(), "us-ca-booking");
}

fn percent(draft: &AgentDraft, catalog: &RulePackCatalog) -> (u8, Option<SetupStep>) {
    let progress = SetupProgress::derive(draft, &ProgressFacts::from_draft(draft, catalog));
    (progress.percent(), progress.next_step())
}

#[test]
fn a_saved_plan_that_validates_completes_the_plan_step() {
    let catalog = RulePackCatalog::new(vec![pack()]).expect("one pack");
    let draft = front_desk();
    let valid = with_plan(
        &draft,
        Plan::template(UseCase::Support, pack().locked_texts()),
    );
    let invalid = with_plan(
        &draft,
        plan(vec![step("bye", NodeKind::End, "Goodbye.")], vec![]),
    );
    let unknown_pack = valid.apply(&DraftPatch {
        jurisdiction: Some(Jurisdiction::parse("ZZ").expect("valid")),
        ..DraftPatch::default()
    });

    assert_eq!(
        percent(&draft, &catalog),
        (25, Some(SetupStep::Plan)),
        "no plan saved yet"
    );
    assert!(ProgressFacts::from_draft(&valid, &catalog).plan_valid);
    assert_eq!(percent(&valid, &catalog), (50, Some(SetupStep::Test)));
    assert_eq!(percent(&invalid, &catalog), (25, Some(SetupStep::Plan)));
    assert_eq!(
        percent(&unknown_pack, &catalog),
        (25, Some(SetupStep::Plan))
    );
    assert_eq!(
        percent(&valid, &RulePackCatalog::default()),
        (25, Some(SetupStep::Plan))
    );
    let facts = ProgressFacts::from_draft(&valid, &catalog);
    assert_eq!((facts.passing_test_run, facts.live_version), (None, None));
}
