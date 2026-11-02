//! The graph rules a plan is validated against.

use std::collections::{BTreeSet, VecDeque};

use super::{NodeId, NodeKind, Plan, PlanError, PlanNode};
use crate::RulePack;

/// Every rule `plan` breaks against `pack`, in a stable order.
pub(super) fn errors(plan: &Plan, pack: &RulePack) -> Vec<PlanError> {
    let ids: BTreeSet<&NodeId> = plan.nodes.iter().map(PlanNode::id).collect();
    let starts: Vec<&PlanNode> = plan
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Start)
        .collect();
    let mut errors = Vec::new();
    match starts.as_slice() {
        [] => errors.push(PlanError::MissingStart),
        [_] => {}
        several => errors.extend(several.iter().map(|start| PlanError::MultipleStarts {
            node: start.id.clone(),
        })),
    }
    if !starts.is_empty() {
        let reached = reachable(plan, &ids, &starts);
        errors.extend(
            plan.nodes
                .iter()
                .filter(|node| !reached.contains(&node.id))
                .map(|node| PlanError::UnreachableNode {
                    node: node.id.clone(),
                }),
        );
    }
    errors.extend(
        plan.nodes
            .iter()
            .filter(|node| node.kind != NodeKind::End)
            .filter(|node| !plan.edges.iter().any(|edge| edge.from == node.id))
            .map(|node| PlanError::MissingExit {
                node: node.id.clone(),
            }),
    );
    for edge in &plan.edges {
        if edge.condition.is_empty() {
            errors.push(PlanError::EmptyCondition {
                edge: edge.id.clone(),
            });
        }
        if !ids.contains(&edge.from) || !ids.contains(&edge.to) {
            errors.push(PlanError::DanglingEdge {
                edge: edge.id.clone(),
            });
        }
    }
    for start in &starts {
        errors.extend(missing_locked_texts(&start.text, pack).map(|key| {
            PlanError::MissingLockedText {
                node: start.id.clone(),
                key,
            }
        }));
    }
    errors
}

/// The steps a path leads to from any start step, the starts included.
fn reachable<'a>(
    plan: &'a Plan,
    ids: &BTreeSet<&NodeId>,
    starts: &[&'a PlanNode],
) -> BTreeSet<&'a NodeId> {
    let mut reached: BTreeSet<&NodeId> = starts.iter().map(|start| &start.id).collect();
    let mut queue: VecDeque<&NodeId> = reached.iter().copied().collect();
    while let Some(at) = queue.pop_front() {
        for edge in plan
            .edges
            .iter()
            .filter(|edge| &edge.from == at && ids.contains(&edge.to))
        {
            if reached.insert(&edge.to) {
                queue.push_back(&edge.to);
            }
        }
    }
    reached
}

/// The keys of the locked texts `opening` doesn't open with. The pack's texts are walked in
/// order: each must come next, word for word, after optional whitespace. A text that isn't
/// there is missing, and the next one must then stand where it would have.
fn missing_locked_texts<'a>(
    opening: &'a str,
    pack: &'a RulePack,
) -> impl Iterator<Item = String> + 'a {
    let mut rest = opening.trim_start();
    pack.locked_texts().iter().filter_map(move |locked| {
        if let Some(after) = rest.strip_prefix(locked.text()) {
            rest = after.trim_start();
            None
        } else {
            Some(locked.key().to_owned())
        }
    })
}
