//! The starting plan of each use case (Parkinson: start from a template, not a blank page).
//! Every template is small, has unique step labels and at least one end step, and is valid
//! for any rule pack once its opening carries the pack's locked texts.

use super::{EdgeId, NodeId, NodeKind, Plan, PlanEdge, PlanNode};
use crate::{LockedText, UseCase};

use NodeKind::{Ask, Collect, End, Say, Start, Tool, Transfer};

/// A step: ID, kind, label, text. A start step's text is the greeting after the locked texts.
type Step = (&'static str, NodeKind, &'static str, &'static str);
/// A path: ID, from, to, condition.
type Path = (&'static str, &'static str, &'static str, &'static str);

const SUPPORT_STEPS: &[Step] = &[
    (
        "start",
        Start,
        "Greeting",
        "Thanks for calling. How can I help you today?",
    ),
    (
        "understand",
        Ask,
        "Understand the problem",
        "Ask what the problem is, and which product or order it is about.",
    ),
    (
        "help",
        Say,
        "Help",
        "Help with the knowledge base, one step at a time, and check that each step worked.",
    ),
    (
        "anything-else",
        Ask,
        "Anything else",
        "Ask whether there is anything else you can help with.",
    ),
    (
        "to-person",
        Transfer,
        "Hand over to a person",
        "Transfer the call to a support specialist, with a short summary of the problem.",
    ),
    (
        "take-message",
        Say,
        "Take a message",
        "Say that nobody is free right now and that a specialist will call back.",
    ),
    (
        "goodbye",
        End,
        "Goodbye",
        "Thank the caller and say goodbye.",
    ),
];

const SUPPORT_PATHS: &[Path] = &[
    (
        "p1",
        "start",
        "understand",
        "The caller answers the greeting",
    ),
    (
        "p2",
        "understand",
        "help",
        "The caller describes the problem",
    ),
    (
        "p3",
        "understand",
        "to-person",
        "The caller asks for a person",
    ),
    ("p4", "help", "anything-else", "The problem is solved"),
    (
        "p5",
        "help",
        "to-person",
        "The problem can't be solved on this call",
    ),
    (
        "p6",
        "anything-else",
        "understand",
        "The caller has another question",
    ),
    (
        "p7",
        "anything-else",
        "goodbye",
        "The caller has nothing else",
    ),
    (
        "p8",
        "to-person",
        "take-message",
        "Nobody answers the transfer",
    ),
    (
        "p9",
        "take-message",
        "goodbye",
        "The caller has left their details",
    ),
];

const SERVICE_STEPS: &[Step] = &[
    (
        "start",
        Start,
        "Greeting",
        "Thanks for calling. What can I do for your account or order today?",
    ),
    (
        "request",
        Ask,
        "Ask what they need",
        "Ask what they want to check or change: an order, a booking or their account.",
    ),
    (
        "identify",
        Collect,
        "Find the account",
        "Collect the order number or the email address on the account.",
    ),
    (
        "look-up",
        Tool,
        "Look it up",
        "Look up the order or the account.",
    ),
    (
        "result",
        Say,
        "Make the change",
        "Make the change or explain the status, and confirm it with the caller.",
    ),
    (
        "to-person",
        Transfer,
        "Hand over to a person",
        "Transfer the call to the service team, with a short summary.",
    ),
    (
        "goodbye",
        End,
        "Goodbye",
        "Thank the caller and say goodbye.",
    ),
];

const SERVICE_PATHS: &[Path] = &[
    ("p1", "start", "request", "The caller answers the greeting"),
    (
        "p2",
        "request",
        "identify",
        "The caller says what they need",
    ),
    (
        "p3",
        "identify",
        "look-up",
        "The caller gives an order number or an email address",
    ),
    (
        "p4",
        "identify",
        "to-person",
        "The caller can't find their details",
    ),
    (
        "p5",
        "look-up",
        "result",
        "The order or the account is found",
    ),
    ("p6", "look-up", "identify", "Nothing matches the details"),
    ("p7", "result", "goodbye", "The caller is satisfied"),
    (
        "p8",
        "result",
        "to-person",
        "The caller wants something you can't do",
    ),
    ("p9", "to-person", "goodbye", "The transfer doesn't connect"),
];

const BOOKING_STEPS: &[Step] = &[
    (
        "start",
        Start,
        "Greeting",
        "Thanks for calling. Would you like to book, move or cancel an appointment?",
    ),
    (
        "service",
        Ask,
        "Ask what to book",
        "Ask which service they want, and whether it is a new booking, a change or a cancellation.",
    ),
    (
        "slot",
        Collect,
        "Find a time",
        "Collect the preferred day and time, and a name for the booking.",
    ),
    (
        "book",
        Tool,
        "Book it",
        "Check availability, then book, move or cancel the appointment.",
    ),
    (
        "confirm",
        Say,
        "Confirm",
        "Read back the service, the day and the time, and say how to change them later.",
    ),
    (
        "goodbye",
        End,
        "Goodbye",
        "Thank the caller and say goodbye.",
    ),
];

const BOOKING_PATHS: &[Path] = &[
    ("p1", "start", "service", "The caller answers the greeting"),
    ("p2", "service", "slot", "The caller says what to book"),
    ("p3", "slot", "book", "The caller gives a day and a time"),
    ("p4", "book", "confirm", "The appointment is booked"),
    ("p5", "book", "slot", "That time is not available"),
    (
        "p6",
        "confirm",
        "service",
        "The caller wants another appointment",
    ),
    ("p7", "confirm", "goodbye", "The caller has nothing else"),
];

const SURVEY_STEPS: &[Step] = &[
    (
        "start",
        Start,
        "Greeting",
        "Do you have two minutes for three short questions?",
    ),
    (
        "q1",
        Ask,
        "Question 1",
        "Ask: how satisfied were you with your last visit, from 1 to 5?",
    ),
    (
        "q2",
        Ask,
        "Question 2",
        "Ask: what is one thing we could do better?",
    ),
    (
        "q3",
        Ask,
        "Question 3",
        "Ask: would you recommend us to a friend?",
    ),
    (
        "thanks",
        Say,
        "Thank them",
        "Thank the caller for their answers.",
    ),
    ("goodbye", End, "Goodbye", "Say goodbye."),
    (
        "declined",
        End,
        "Not now",
        "Say that's fine, thank them for their time and say goodbye.",
    ),
];

const SURVEY_PATHS: &[Path] = &[
    ("p1", "start", "q1", "The caller agrees to answer"),
    ("p2", "start", "declined", "The caller says no or not now"),
    ("p3", "q1", "q2", "The caller answers"),
    ("p4", "q2", "q3", "The caller answers"),
    ("p5", "q3", "thanks", "The caller answers"),
    ("p6", "thanks", "goodbye", "The thanks are said"),
];

const TELEMARKETING_STEPS: &[Step] = &[
    (
        "start",
        Start,
        "Greeting",
        "Do you have a minute to hear about it?",
    ),
    (
        "interest",
        Ask,
        "Check the need",
        "Ask what they use today for the need this offer covers.",
    ),
    (
        "pitch",
        Say,
        "Explain the offer",
        "Explain the offer in two or three sentences, with only the facts in the knowledge base.",
    ),
    (
        "next-step",
        Ask,
        "Ask for a next step",
        "Ask whether they would like a follow-up call or an email with the details.",
    ),
    (
        "details",
        Collect,
        "Take their details",
        "Collect the best time and way to follow up.",
    ),
    (
        "goodbye",
        End,
        "Goodbye",
        "Thank the caller and say goodbye.",
    ),
    (
        "not-interested",
        End,
        "Not interested",
        "Thank them for their time and say goodbye. Never push after a no.",
    ),
];

const TELEMARKETING_PATHS: &[Path] = &[
    ("p1", "start", "interest", "The caller has a minute"),
    (
        "p2",
        "start",
        "not-interested",
        "The caller has no time or isn't interested",
    ),
    ("p3", "interest", "pitch", "The caller describes their need"),
    ("p4", "pitch", "next-step", "The caller wants to know more"),
    (
        "p5",
        "pitch",
        "not-interested",
        "The caller isn't interested",
    ),
    ("p6", "next-step", "details", "The caller wants a follow-up"),
    ("p7", "next-step", "not-interested", "The caller declines"),
    (
        "p8",
        "details",
        "goodbye",
        "The caller has given their details",
    ),
];

/// Custom agents start blank: a greeting and a goodbye.
const CUSTOM_STEPS: &[Step] = &[
    (
        "start",
        Start,
        "Greeting",
        "Hello! How can I help you today?",
    ),
    (
        "goodbye",
        End,
        "Goodbye",
        "Thank the caller and say goodbye.",
    ),
];

const CUSTOM_PATHS: &[Path] = &[("p1", "start", "goodbye", "The conversation is done")];

/// The steps and paths of `use_case`'s template.
const fn parts(use_case: UseCase) -> (&'static [Step], &'static [Path]) {
    match use_case {
        UseCase::Support => (SUPPORT_STEPS, SUPPORT_PATHS),
        UseCase::Service => (SERVICE_STEPS, SERVICE_PATHS),
        UseCase::Booking => (BOOKING_STEPS, BOOKING_PATHS),
        UseCase::Survey => (SURVEY_STEPS, SURVEY_PATHS),
        UseCase::Telemarketing => (TELEMARKETING_STEPS, TELEMARKETING_PATHS),
        UseCase::Custom => (CUSTOM_STEPS, CUSTOM_PATHS),
    }
}

/// The opening: the locked texts in order, then the greeting (if any).
pub(super) fn opening(locked: &[LockedText], greeting: &str) -> String {
    let mut parts: Vec<&str> = locked.iter().map(LockedText::text).collect();
    if !greeting.is_empty() {
        parts.push(greeting);
    }
    parts.join(" ")
}

pub(super) fn template(use_case: UseCase, locked: &[LockedText]) -> Plan {
    let (steps, paths) = parts(use_case);
    // The tables are static and every entry keeps the shape rules (a unit test checks them
    // through the public constructors), so they are built directly.
    let nodes = steps
        .iter()
        .map(|&(id, kind, label, text)| PlanNode {
            id: NodeId(id.to_owned()),
            kind,
            label: label.to_owned(),
            text: if kind == Start {
                opening(locked, text)
            } else {
                text.to_owned()
            },
        })
        .collect();
    let edges = paths
        .iter()
        .map(|&(id, from, to, condition)| PlanEdge {
            id: EdgeId(id.to_owned()),
            from: NodeId(from.to_owned()),
            to: NodeId(to.to_owned()),
            condition: condition.to_owned(),
        })
        .collect();
    Plan { nodes, edges }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The template rebuilt through the checking constructors.
    fn rebuilt(plan: &Plan) -> Result<Plan, super::super::PlanShapeError> {
        let nodes = plan
            .nodes()
            .iter()
            .map(|n| {
                PlanNode::new(
                    NodeId::parse(n.id().as_str())?,
                    n.kind(),
                    n.label(),
                    n.text(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let edges = plan
            .edges()
            .iter()
            .map(|e| {
                PlanEdge::new(
                    EdgeId::parse(e.id().as_str())?,
                    NodeId::parse(e.from().as_str())?,
                    NodeId::parse(e.to().as_str())?,
                    e.condition(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Plan::new(nodes, edges)
    }

    #[test]
    fn every_template_keeps_the_shape_rules_and_has_unique_labels() {
        let locked = [LockedText::new("ai_disclosure", "You're talking to an AI.").expect("valid")];
        for use_case in UseCase::ALL {
            let plan = template(use_case, &locked);

            assert_eq!(rebuilt(&plan).as_ref(), Ok(&plan), "{use_case}");
            let labels: BTreeSet<&str> = plan.nodes().iter().map(PlanNode::label).collect();
            assert_eq!(
                labels.len(),
                plan.nodes().len(),
                "{use_case}: unique labels"
            );
            assert!(
                plan.nodes().len() <= 12,
                "{use_case}: a starting point, not a sprawl"
            );
        }
    }

    #[test]
    fn the_opening_is_the_locked_texts_then_the_greeting() {
        let locked = [
            LockedText::new("ai_disclosure", "You're talking to an AI.").expect("valid"),
            LockedText::new("recording_notice", "This call is recorded.").expect("valid"),
        ];

        let plan = template(UseCase::Custom, &locked);

        let start = plan.nodes().first().map(PlanNode::text);
        assert_eq!(
            start,
            Some(
                "You're talking to an AI. This call is recorded. Hello! How can I help you today?"
            )
        );
        assert_eq!(
            template(UseCase::Custom, &[])
                .nodes()
                .first()
                .map(PlanNode::text),
            Some("Hello! How can I help you today?")
        );
    }
}
