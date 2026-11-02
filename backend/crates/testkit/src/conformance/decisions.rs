//! The `DecisionEngine` contract.

use std::collections::BTreeSet;
use std::time::Duration;

use vak_domain::{Answer, ChoiceOption, Question, QuestionId, Speaker};
use vak_providers::{DecisionEngine, DecisionRequest};

const DEADLINE: Duration = Duration::from_secs(20);
const INTENTS: [&str; 4] = ["book", "cancel", "opt_out", "other"];

/// Checks the `DecisionEngine` contract on two clear-cut conversations, each asking a
/// yes/no `opt_out` question and an `intent` choice over `book`, `cancel`, `opt_out` and
/// `other`:
///
/// 1. Every question asked is answered, and nothing else; the answering model is named.
/// 2. `opt_out` is a yes/no answer with a probability from 0 to 1: at least 0.5 for "No
///    thanks, and please stop calling me.", below 0.5 for "Yes please, a table for two at
///    seven tonight."
/// 3. `intent` is a choice answer: the chosen label is `opt_out`, then `book`; its
///    confidence is from 0 to 1; it gives every option exactly one probability from 0 to 1,
///    and those add up to 1 (within 0.05).
///
/// # Panics
/// When `engine` breaks a rule.
#[expect(clippy::panic, reason = "a broken rule fails the calling test")]
pub async fn decision_conformance(engine: &dyn DecisionEngine) {
    let cases = [
        ("No thanks, and please stop calling me.", true, "opt_out"),
        (
            "Yes please, a table for two at seven tonight.",
            false,
            "book",
        ),
    ];
    for (caller, opts_out, intent) in cases {
        let request = request(caller);
        let decisions = engine
            .decide(&request)
            .await
            .unwrap_or_else(|error| panic!("decision rule 1: {caller:?} failed: {error}"));
        assert!(
            !decisions.model().is_empty(),
            "decision rule 1: no model ID"
        );
        let answered: BTreeSet<&str> = decisions
            .answers()
            .iter()
            .map(|(id, _)| id.as_str())
            .collect();
        assert_eq!(
            answered,
            BTreeSet::from(["intent", "opt_out"]),
            "decision rule 1: answered the wrong questions for {caller:?}"
        );

        match decisions.answer("opt_out") {
            Some(Answer::YesNo { probability, .. }) => {
                assert!(
                    (0.0..=1.0).contains(probability),
                    "decision rule 2: opt_out probability {probability} is out of range"
                );
                assert_eq!(
                    *probability >= 0.5,
                    opts_out,
                    "decision rule 2: opt_out is {probability} for {caller:?}"
                );
            }
            other => panic!("decision rule 2: opt_out should be a yes/no answer, got {other:?}"),
        }

        match decisions.answer("intent") {
            Some(Answer::Choice {
                label,
                confidence,
                probabilities,
                ..
            }) => {
                assert_eq!(label, intent, "decision rule 3: intent for {caller:?}");
                assert!(
                    (0.0..=1.0).contains(confidence),
                    "decision rule 3: confidence {confidence} is out of range"
                );
                let mut labels: Vec<&str> = probabilities
                    .iter()
                    .map(|(label, _)| label.as_str())
                    .collect();
                labels.sort_unstable();
                assert_eq!(
                    labels, INTENTS,
                    "decision rule 3: one probability per option"
                );
                assert!(
                    probabilities.iter().all(|(_, p)| (0.0..=1.0).contains(p)),
                    "decision rule 3: a probability is out of range: {probabilities:?}"
                );
                let sum: f64 = probabilities.iter().map(|(_, p)| p).sum();
                assert!(
                    (0.95..=1.05).contains(&sum),
                    "decision rule 3: the probabilities add up to {sum}"
                );
            }
            other => panic!("decision rule 3: intent should be a choice answer, got {other:?}"),
        }
    }
}

#[expect(
    clippy::expect_used,
    reason = "the questions are constants that are valid"
)]
fn request(caller: &str) -> DecisionRequest {
    let id = |raw: &str| QuestionId::parse(raw).expect("a valid question ID");
    let intent = Question::choice(
        "What does the caller want?",
        [
            ChoiceOption::new("book").with_description("book a table"),
            ChoiceOption::new("cancel").with_description("cancel a booking"),
            ChoiceOption::new("opt_out").with_description("stop being contacted"),
            ChoiceOption::new("other"),
        ],
    )
    .expect("a valid choice");
    DecisionRequest::new(DEADLINE)
        .with_turn(Speaker::Agent, "Would you like to book a table?")
        .with_turn(Speaker::Caller, caller)
        .with_question(
            id("opt_out"),
            Question::yes_no("Is the caller asking not to be contacted again?"),
        )
        .with_question(id("intent"), intent)
}
