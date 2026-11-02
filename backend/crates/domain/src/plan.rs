//! The plan: a graph of steps joined by paths, each path with a plain-language condition.
//! The decision model picks among a step's paths at call time (M4); here the graph is built,
//! checked for shape, and validated against the agent's rule pack.

mod templates;
mod validate;

use std::fmt;

use crate::{LockedText, RulePack, RulePackRef, UseCase};

/// A step's ID within its plan: 1–64 ASCII letters, digits, `-` or `_`. Case-sensitive.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(String);

/// A path's ID within its plan, with the same rules as [`NodeId`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeId(String);

/// Longest step or path ID.
const MAX_ID: usize = 64;

fn valid_id(raw: &str) -> bool {
    (1..=MAX_ID).contains(&raw.len())
        && raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

impl NodeId {
    /// Longest ID, in characters.
    pub const MAX: usize = MAX_ID;

    /// Accepts the ID exactly as given: IDs are machine names, so nothing is trimmed.
    pub fn parse(raw: &str) -> Result<Self, PlanShapeError> {
        if valid_id(raw) {
            Ok(Self(raw.to_owned()))
        } else {
            Err(PlanShapeError::InvalidId)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl EdgeId {
    /// Longest ID, in characters.
    pub const MAX: usize = MAX_ID;

    /// Accepts the ID exactly as given, like [`NodeId::parse`].
    pub fn parse(raw: &str) -> Result<Self, PlanShapeError> {
        if valid_id(raw) {
            Ok(Self(raw.to_owned()))
        } else {
            Err(PlanShapeError::InvalidId)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EdgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a step does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// Where the call begins: the opening, locked texts first.
    Start,
    /// Says something.
    Say,
    /// Asks a question.
    Ask,
    /// Collects details (a date, an order number).
    Collect,
    /// Calls a tool (look up, book).
    Tool,
    /// Hands the call to a person.
    Transfer,
    /// Ends the call.
    End,
}

impl NodeKind {
    /// Every kind, in the order pickers show them.
    pub const ALL: [Self; 7] = [
        Self::Start,
        Self::Say,
        Self::Ask,
        Self::Collect,
        Self::Tool,
        Self::Transfer,
        Self::End,
    ];

    /// The wire and storage name, e.g. `ask`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Say => "say",
            Self::Ask => "ask",
            Self::Collect => "collect",
            Self::Tool => "tool",
            Self::Transfer => "transfer",
            Self::End => "end",
        }
    }

    /// Accepts a name in any case, with surrounding whitespace (Postel).
    pub fn parse(raw: &str) -> Result<Self, PlanShapeError> {
        let raw = raw.trim();
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str().eq_ignore_ascii_case(raw))
            .ok_or(PlanShapeError::UnknownKind)
    }
}

/// One step of a plan.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlanNode {
    id: NodeId,
    kind: NodeKind,
    label: String,
    text: String,
}

impl PlanNode {
    /// Longest label, in characters.
    pub const MAX_LABEL: usize = 80;
    /// Longest text, in characters.
    pub const MAX_TEXT: usize = 2_000;

    /// A step. The label (1–80 characters, no control characters) and the text (what the
    /// agent says or does, up to 2,000 characters, line endings normalised to `\n`) are
    /// trimmed.
    pub fn new(
        id: NodeId,
        kind: NodeKind,
        label: &str,
        text: &str,
    ) -> Result<Self, PlanShapeError> {
        let label = label.trim();
        match label.chars().count() {
            0 => return Err(PlanShapeError::EmptyLabel),
            n if n > Self::MAX_LABEL => {
                return Err(PlanShapeError::LabelTooLong {
                    max: Self::MAX_LABEL,
                });
            }
            _ if label.chars().any(char::is_control) => {
                return Err(PlanShapeError::ControlCharacter);
            }
            _ => {}
        }
        let text = text.trim().replace("\r\n", "\n").replace('\r', "\n");
        if text.chars().count() > Self::MAX_TEXT {
            return Err(PlanShapeError::TextTooLong {
                max: Self::MAX_TEXT,
            });
        }
        Ok(Self {
            id,
            kind,
            label: label.to_owned(),
            text,
        })
    }

    #[must_use]
    pub fn id(&self) -> &NodeId {
        &self.id
    }

    #[must_use]
    pub fn kind(&self) -> NodeKind {
        self.kind
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// A path from one step to another, taken when its condition holds.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlanEdge {
    id: EdgeId,
    from: NodeId,
    to: NodeId,
    condition: String,
}

impl PlanEdge {
    /// Longest condition, in characters.
    pub const MAX_CONDITION: usize = 200;

    /// A path. The condition is trimmed; an empty one is a validation error, not a shape
    /// error, so a half-finished plan can still be saved.
    pub fn new(
        id: EdgeId,
        from: NodeId,
        to: NodeId,
        condition: &str,
    ) -> Result<Self, PlanShapeError> {
        let condition = condition.trim();
        if condition.chars().count() > Self::MAX_CONDITION {
            return Err(PlanShapeError::ConditionTooLong {
                max: Self::MAX_CONDITION,
            });
        }
        Ok(Self {
            id,
            from,
            to,
            condition: condition.to_owned(),
        })
    }

    #[must_use]
    pub fn id(&self) -> &EdgeId {
        &self.id
    }

    #[must_use]
    pub fn from(&self) -> &NodeId {
        &self.from
    }

    #[must_use]
    pub fn to(&self) -> &NodeId {
        &self.to
    }

    #[must_use]
    pub fn condition(&self) -> &str {
        &self.condition
    }
}

/// A plan: steps and paths, in the order they were given.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Plan {
    nodes: Vec<PlanNode>,
    edges: Vec<PlanEdge>,
}

/// Why steps and paths don't make a well-formed plan. Shape errors are rejected before
/// anything is stored; graph rules are [`PlanError`]s.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanShapeError {
    #[error("an ID must be 1-64 letters, digits, '-' or '_'")]
    InvalidId,
    #[error("unknown step kind; expected start, say, ask, collect, tool, transfer or end")]
    UnknownKind,
    #[error("a step needs a label")]
    EmptyLabel,
    #[error("a step label is longer than {max} characters")]
    LabelTooLong { max: usize },
    #[error("a step label contains a control character")]
    ControlCharacter,
    #[error("a step text is longer than {max} characters")]
    TextTooLong { max: usize },
    #[error("a path condition is longer than {max} characters")]
    ConditionTooLong { max: usize },
    #[error("a plan has more than {max} steps")]
    TooManyNodes { max: usize },
    #[error("a plan has more than {max} paths")]
    TooManyEdges { max: usize },
    #[error("two steps share the ID {id}")]
    DuplicateNode { id: NodeId },
    #[error("two paths share the ID {id}")]
    DuplicateEdge { id: EdgeId },
}

/// A graph rule the plan breaks, located on the step or path it concerns.
#[derive(Debug, Clone, PartialEq, Eq, Hash, thiserror::Error)]
pub enum PlanError {
    #[error("the plan has no start step")]
    MissingStart,
    #[error("step {node} is one of several start steps")]
    MultipleStarts { node: NodeId },
    #[error("no path leads to step {node} from the start")]
    UnreachableNode { node: NodeId },
    #[error("step {node} has no path out, and only an end step may stop there")]
    MissingExit { node: NodeId },
    #[error("path {edge} has no condition")]
    EmptyCondition { edge: EdgeId },
    #[error("path {edge} leads to or from a step that doesn't exist")]
    DanglingEdge { edge: EdgeId },
    #[error("start step {node} doesn't open with the rule pack's {key} text")]
    MissingLockedText { node: NodeId, key: String },
    #[error("there is no rule pack {} v{} to check the plan against", pack.id(), pack.version())]
    RulePackUnavailable { pack: RulePackRef },
}

impl PlanError {
    /// The stable wire name, e.g. `unreachable_node`.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MissingStart => "missing_start",
            Self::MultipleStarts { .. } => "multiple_starts",
            Self::UnreachableNode { .. } => "unreachable_node",
            Self::MissingExit { .. } => "missing_exit",
            Self::EmptyCondition { .. } => "empty_condition",
            Self::DanglingEdge { .. } => "dangling_edge",
            Self::MissingLockedText { .. } => "missing_locked_text",
            Self::RulePackUnavailable { .. } => "rule_pack_unavailable",
        }
    }

    /// The step the error is on, if any.
    #[must_use]
    pub fn node(&self) -> Option<&NodeId> {
        match self {
            Self::MultipleStarts { node }
            | Self::UnreachableNode { node }
            | Self::MissingExit { node }
            | Self::MissingLockedText { node, .. } => Some(node),
            Self::MissingStart
            | Self::EmptyCondition { .. }
            | Self::DanglingEdge { .. }
            | Self::RulePackUnavailable { .. } => None,
        }
    }

    /// The path the error is on, if any.
    #[must_use]
    pub fn edge(&self) -> Option<&EdgeId> {
        match self {
            Self::EmptyCondition { edge } | Self::DanglingEdge { edge } => Some(edge),
            _ => None,
        }
    }
}

/// The first value that occurs twice in `values`, in order.
fn first_duplicate<'a, T: Ord>(values: impl Iterator<Item = &'a T>) -> Option<&'a T> {
    let mut seen = std::collections::BTreeSet::new();
    values.into_iter().find(|value| !seen.insert(*value))
}

impl Plan {
    /// Most steps in a plan.
    pub const MAX_NODES: usize = 50;
    /// Most paths in a plan.
    pub const MAX_EDGES: usize = 200;

    /// A well-formed plan: unique step and path IDs, within the size limits.
    pub fn new(nodes: Vec<PlanNode>, edges: Vec<PlanEdge>) -> Result<Self, PlanShapeError> {
        if nodes.len() > Self::MAX_NODES {
            return Err(PlanShapeError::TooManyNodes {
                max: Self::MAX_NODES,
            });
        }
        if edges.len() > Self::MAX_EDGES {
            return Err(PlanShapeError::TooManyEdges {
                max: Self::MAX_EDGES,
            });
        }
        if let Some(id) = first_duplicate(nodes.iter().map(PlanNode::id)) {
            return Err(PlanShapeError::DuplicateNode { id: id.clone() });
        }
        if let Some(id) = first_duplicate(edges.iter().map(PlanEdge::id)) {
            return Err(PlanShapeError::DuplicateEdge { id: id.clone() });
        }
        Ok(Self { nodes, edges })
    }

    /// The use case's starting plan. Its start step opens with `locked`, in order.
    #[must_use]
    pub fn template(use_case: UseCase, locked: &[LockedText]) -> Self {
        templates::template(use_case, locked)
    }

    /// Every graph rule this plan breaks against `pack`, or `Ok` when it has none. Errors
    /// come in a stable order: the whole plan, then steps, then paths, then locked texts in
    /// the pack's order.
    pub fn validate(&self, pack: &RulePack) -> Result<(), Vec<PlanError>> {
        let errors = validate::errors(self, pack);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    #[must_use]
    pub fn nodes(&self) -> &[PlanNode] {
        &self.nodes
    }

    #[must_use]
    pub fn edges(&self) -> &[PlanEdge] {
        &self.edges
    }
}
