//! The wire format of `/api/v1/agents/{id}/plan` responses: the plan, the rules it is checked
//! against, and its problems in words for operators.

use serde::Serialize;
use utoipa::ToSchema;
use vak_domain::{DraftPlan, LockedText, NodeKind, PlanEdge, PlanError, PlanNode};

use crate::agents::RulePackDto;

/// What a step does: `start` opens the call (the locked texts first), `end` ends it.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = StepKind)]
pub(crate) enum StepKindDto {
    Start,
    Say,
    Ask,
    Collect,
    Tool,
    Transfer,
    End,
}

impl From<NodeKind> for StepKindDto {
    fn from(kind: NodeKind) -> Self {
        match kind {
            NodeKind::Start => Self::Start,
            NodeKind::Say => Self::Say,
            NodeKind::Ask => Self::Ask,
            NodeKind::Collect => Self::Collect,
            NodeKind::Tool => Self::Tool,
            NodeKind::Transfer => Self::Transfer,
            NodeKind::End => Self::End,
        }
    }
}

/// A step of the plan.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PlanStep)]
pub(crate) struct StepDto {
    id: String,
    kind: StepKindDto,
    label: String,
    text: String,
}

/// A path between two steps.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PlanPath)]
pub(crate) struct PathDto {
    id: String,
    from: String,
    to: String,
    condition: String,
}

/// Text the agent says word for word, from the rule pack.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = LockedText)]
pub(crate) struct LockedTextDto {
    /// E.g. `ai_disclosure`, `recording_notice`.
    #[schema(example = "ai_disclosure")]
    key: String,
    text: String,
}

impl From<&LockedText> for LockedTextDto {
    fn from(locked: &LockedText) -> Self {
        Self {
            key: locked.key().to_owned(),
            text: locked.text().to_owned(),
        }
    }
}

/// The rule a plan breaks, in words, located on a step (`nodeId`), a path (`edgeId`), or
/// neither (the whole plan).
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = PlanProblem)]
pub(crate) struct ProblemDto {
    /// `missing_start`, `multiple_starts`, `unreachable_node`, `missing_exit`,
    /// `empty_condition`, `dangling_edge`, `missing_locked_text` or `rule_pack_unavailable`.
    #[schema(example = "unreachable_node")]
    code: &'static str,
    /// What is wrong and how to fix it.
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edge_id: Option<String>,
}

/// Whether the plan follows every rule, and every rule it breaks.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PlanValidation)]
pub(crate) struct ValidationDto {
    valid: bool,
    errors: Vec<ProblemDto>,
}

/// An agent's plan: the saved one, or its use case's template until one is saved.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = AgentPlan)]
pub(crate) struct AgentPlanDto {
    /// `false` while this is the template: nothing is stored until a plan is saved.
    saved: bool,
    /// The rule pack the plan is checked against.
    rule_pack: RulePackDto,
    /// What the start step must open with, word for word and in order. Empty when no rule
    /// pack covers the agent yet.
    locked_texts: Vec<LockedTextDto>,
    /// What the agent says, word for word, when the caller opts out, before the call ends.
    /// `null` when no rule pack covers the agent yet.
    #[schema(required = true)]
    opt_out_confirmation: Option<String>,
    nodes: Vec<StepDto>,
    edges: Vec<PathDto>,
    validation: ValidationDto,
}

impl From<&PlanNode> for StepDto {
    fn from(node: &PlanNode) -> Self {
        Self {
            id: node.id().to_string(),
            kind: node.kind().into(),
            label: node.label().to_owned(),
            text: node.text().to_owned(),
        }
    }
}

impl From<&PlanEdge> for PathDto {
    fn from(edge: &PlanEdge) -> Self {
        Self {
            id: edge.id().to_string(),
            from: edge.from().to_string(),
            to: edge.to().to_string(),
            condition: edge.condition().to_owned(),
        }
    }
}

/// The locked text's name in a sentence: `ai_disclosure` is "AI disclosure".
fn locked_name(key: &str) -> String {
    match key {
        LockedText::AI_DISCLOSURE => "AI disclosure".to_owned(),
        other => other.replace('_', " "),
    }
}

/// What an operator reads on the step or path, or above the plan.
pub(crate) fn plan_message(error: &PlanError) -> String {
    match error {
        PlanError::MissingStart => "Add a start step: the call needs a place to begin.".to_owned(),
        PlanError::MultipleStarts { .. } => {
            "Only one step can start the call. Change this one to another kind.".to_owned()
        }
        PlanError::UnreachableNode { .. } => {
            "No path leads here from the start. Add a path to this step.".to_owned()
        }
        PlanError::MissingExit { .. } => {
            "This step has no path out. Add one, or make it an end step.".to_owned()
        }
        PlanError::EmptyCondition { .. } => {
            "This path needs a condition: say when to take it.".to_owned()
        }
        PlanError::DanglingEdge { .. } => {
            "This path leads to or from a step that doesn't exist.".to_owned()
        }
        PlanError::MissingLockedText { key, .. } => format!(
            "The opening must start with the rule pack's {}, word for word.",
            locked_name(key)
        ),
        PlanError::RulePackUnavailable { .. } => {
            "No rule pack covers this agent's jurisdiction and purpose yet, so the plan can't \
             be checked. Change the jurisdiction on the Provision step."
                .to_owned()
        }
    }
}

impl From<&Result<(), Vec<PlanError>>> for ValidationDto {
    fn from(validation: &Result<(), Vec<PlanError>>) -> Self {
        let errors = validation.as_ref().err().map_or(&[][..], Vec::as_slice);
        Self {
            valid: validation.is_ok(),
            errors: errors
                .iter()
                .map(|error| ProblemDto {
                    code: error.code(),
                    message: plan_message(error),
                    node_id: error.node().map(ToString::to_string),
                    edge_id: error.edge().map(ToString::to_string),
                })
                .collect(),
        }
    }
}

impl From<&DraftPlan> for AgentPlanDto {
    fn from(shown: &DraftPlan) -> Self {
        Self {
            saved: shown.saved(),
            rule_pack: shown.rule_pack().into(),
            locked_texts: shown.locked_texts().iter().map(Into::into).collect(),
            opt_out_confirmation: shown.opt_out_confirmation().map(str::to_owned),
            nodes: shown.plan().nodes().iter().map(StepDto::from).collect(),
            edges: shown.plan().edges().iter().map(PathDto::from).collect(),
            validation: shown.validation().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use vak_domain::{EdgeId, NodeId, RulePackRef};

    use super::*;

    #[test]
    fn every_rule_has_its_own_message() {
        let node = NodeId::parse("s").expect("valid");
        let edge = EdgeId::parse("p").expect("valid");
        let errors = [
            PlanError::MissingStart,
            PlanError::MultipleStarts { node: node.clone() },
            PlanError::UnreachableNode { node: node.clone() },
            PlanError::MissingExit { node: node.clone() },
            PlanError::EmptyCondition { edge: edge.clone() },
            PlanError::DanglingEdge { edge },
            PlanError::MissingLockedText {
                node: node.clone(),
                key: LockedText::AI_DISCLOSURE.to_owned(),
            },
            PlanError::MissingLockedText {
                node,
                key: LockedText::RECORDING_NOTICE.to_owned(),
            },
            PlanError::RulePackUnavailable {
                pack: RulePackRef::new("zz-support", 1).expect("valid"),
            },
        ];

        let messages: BTreeSet<String> = errors.iter().map(plan_message).collect();

        assert_eq!(messages.len(), errors.len());
        assert!(
            messages.contains(
                "The opening must start with the rule pack's AI disclosure, word for word."
            )
        );
        assert!(messages.contains(
            "The opening must start with the rule pack's recording notice, word for word."
        ));
    }
}
