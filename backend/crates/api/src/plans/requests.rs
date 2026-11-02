//! The request body of `PUT /agents/{id}/plan` and `POST …/plan/validate`. Every step and path
//! field is parsed by its domain type; every invalid field is reported at once, by its path
//! (`nodes[2].label`), and a malformed plan stores nothing.

use std::collections::BTreeSet;

use serde::Deserialize;
use utoipa::ToSchema;
use vak_domain::{EdgeId, NodeId, NodeKind, Plan, PlanEdge, PlanNode, PlanShapeError};

use super::dto::StepKindDto;
use crate::problem::{ApiError, FieldError};

/// A whole plan: every step and every path. It replaces the saved plan.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = PlanInput)]
pub(crate) struct PlanRequest {
    /// The steps, in order: at most 50.
    #[schema(value_type = Vec<StepRequest>, required = true, max_items = 50)]
    nodes: Option<Vec<StepRequest>>,
    /// The paths, in order: at most 200.
    #[schema(value_type = Vec<PathRequest>, required = true, max_items = 200)]
    edges: Option<Vec<PathRequest>>,
}

/// A step of the plan.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = PlanStepInput)]
pub(crate) struct StepRequest {
    /// Unique within the plan: 1 to 64 letters, digits, `-` or `_`.
    #[schema(value_type = String, required = true, pattern = "^[A-Za-z0-9_-]{1,64}$", example = "greeting")]
    id: Option<String>,
    #[schema(value_type = StepKindDto, required = true)]
    kind: Option<String>,
    /// The step's name on the canvas, 1 to 80 characters.
    #[schema(value_type = String, required = true, min_length = 1, max_length = 80, example = "Greeting")]
    label: Option<String>,
    /// What the agent says or does here, up to 2,000 characters.
    #[schema(value_type = String, required = false, default = "", max_length = 2000)]
    text: Option<String>,
}

/// A path between two steps, taken when its condition holds.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = PlanPathInput)]
pub(crate) struct PathRequest {
    /// Unique within the plan: 1 to 64 letters, digits, `-` or `_`.
    #[schema(value_type = String, required = true, pattern = "^[A-Za-z0-9_-]{1,64}$")]
    id: Option<String>,
    /// The ID of the step the path leaves.
    #[schema(value_type = String, required = true)]
    from: Option<String>,
    /// The ID of the step the path leads to.
    #[schema(value_type = String, required = true)]
    to: Option<String>,
    /// When to take the path, in plain words, up to 200 characters.
    #[schema(value_type = String, required = false, default = "", max_length = 200, example = "The caller wants to book")]
    condition: Option<String>,
}

const REQUIRED: &str = "This field is required.";
const BAD_ID: &str = "Use 1 to 64 letters, digits, '-' or '_'.";

/// Collects the invalid fields of one plan.
#[derive(Default)]
struct Fields {
    errors: Vec<FieldError>,
}

impl Fields {
    fn push(&mut self, field: String, message: &'static str) {
        self.errors.push(FieldError { field, message });
    }

    /// `raw`, or `None` after recording `field` as missing.
    fn required<T>(&mut self, field: &str, raw: Option<T>) -> Option<T> {
        if raw.is_none() {
            self.push(field.to_owned(), REQUIRED);
        }
        raw
    }

    /// `result`'s value, or `None` after recording `field` with `message`.
    fn check<T, E>(
        &mut self,
        result: Result<T, E>,
        field: String,
        message: &'static str,
    ) -> Option<T> {
        if result.is_err() {
            self.push(field, message);
        }
        result.ok()
    }

    fn id<T>(
        &mut self,
        field: String,
        raw: Option<&str>,
        parse: fn(&str) -> Result<T, PlanShapeError>,
    ) -> Option<T> {
        let raw = self.required(&field, raw)?;
        self.check(parse(raw), field, BAD_ID)
    }

    fn step(&mut self, at: usize, step: StepRequest) -> Option<PlanNode> {
        let field = |name: &str| format!("nodes[{at}].{name}");
        let id = self.id(field("id"), step.id.as_deref(), NodeId::parse);
        let kind = self.required(&field("kind"), step.kind).and_then(|kind| {
            self.check(
                NodeKind::parse(&kind),
                field("kind"),
                "Choose start, say, ask, collect, tool, transfer or end.",
            )
        });
        let label = self.required(&field("label"), step.label);
        let (id, kind, label) = (id?, kind?, label?);
        let text = step.text.unwrap_or_default();
        match PlanNode::new(id, kind, &label, &text) {
            Ok(node) => Some(node),
            Err(error) => {
                let (name, message) = step_message(&error);
                self.push(field(name), message);
                None
            }
        }
    }

    fn path(&mut self, at: usize, path: PathRequest) -> Option<PlanEdge> {
        let field = |name: &str| format!("edges[{at}].{name}");
        let id = self.id(field("id"), path.id.as_deref(), EdgeId::parse);
        let from = self.id(field("from"), path.from.as_deref(), NodeId::parse);
        let to = self.id(field("to"), path.to.as_deref(), NodeId::parse);
        let (id, from, to) = (id?, from?, to?);
        let condition = path.condition.unwrap_or_default();
        self.check(
            PlanEdge::new(id, from, to, &condition),
            field("condition"),
            "Use at most 200 characters.",
        )
    }

    /// Records the second and later use of each ID.
    fn unique<'a>(&mut self, list: &str, ids: impl Iterator<Item = (usize, &'a str)>) {
        let mut seen = BTreeSet::new();
        for (at, id) in ids {
            if !seen.insert(id) {
                let message = if list == "nodes" {
                    "Another step already has this ID."
                } else {
                    "Another path already has this ID."
                };
                self.push(format!("{list}[{at}].id"), message);
            }
        }
    }
}

/// The field a step's shape error is about, and how to fix it.
fn step_message(error: &PlanShapeError) -> (&'static str, &'static str) {
    match error {
        PlanShapeError::EmptyLabel => ("label", "Give the step a name."),
        PlanShapeError::LabelTooLong { .. } => ("label", "Use at most 80 characters."),
        PlanShapeError::ControlCharacter => ("label", "Remove line breaks and control characters."),
        _ => ("text", "Use at most 2,000 characters."),
    }
}

impl TryFrom<PlanRequest> for Plan {
    type Error = ApiError;

    fn try_from(body: PlanRequest) -> Result<Self, ApiError> {
        let mut fields = Fields::default();
        let steps = fields.required("nodes", body.nodes).unwrap_or_default();
        let paths = fields.required("edges", body.edges).unwrap_or_default();
        if steps.len() > Plan::MAX_NODES {
            fields.push("nodes".to_owned(), "A plan has at most 50 steps.");
        }
        if paths.len() > Plan::MAX_EDGES {
            fields.push("edges".to_owned(), "A plan has at most 200 paths.");
        }
        let step_ids: Vec<Option<String>> = steps.iter().map(|step| step.id.clone()).collect();
        let path_ids: Vec<Option<String>> = paths.iter().map(|path| path.id.clone()).collect();
        let nodes: Vec<Option<PlanNode>> = steps
            .into_iter()
            .enumerate()
            .map(|(at, step)| fields.step(at, step))
            .collect();
        let edges: Vec<Option<PlanEdge>> = paths
            .into_iter()
            .enumerate()
            .map(|(at, path)| fields.path(at, path))
            .collect();
        fields.unique("nodes", indexed(&step_ids));
        fields.unique("edges", indexed(&path_ids));
        if !fields.errors.is_empty() {
            return Err(ApiError::Invalid(fields.errors));
        }
        let nodes = nodes.into_iter().flatten().collect();
        let edges = edges.into_iter().flatten().collect();
        // Every shape rule was checked field by field above.
        Self::new(nodes, edges).map_err(|_| ApiError::Invalid(Vec::new()))
    }
}

/// `(index, id)` of every entry that has an ID.
fn indexed(ids: &[Option<String>]) -> impl Iterator<Item = (usize, &str)> {
    ids.iter()
        .enumerate()
        .filter_map(|(at, id)| id.as_deref().map(|id| (at, id)))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parsed(body: serde_json::Value) -> Result<Plan, Vec<(String, &'static str)>> {
        let request: PlanRequest = serde_json::from_value(body).expect("the request shape");
        Plan::try_from(request).map_err(|error| match error {
            ApiError::Invalid(errors) => errors
                .into_iter()
                .map(|error| (error.field, error.message))
                .collect(),
            other => panic!("expected Invalid, got {other:?}"),
        })
    }

    #[test]
    fn every_invalid_field_is_named_with_its_fix() {
        let errors = parsed(json!({
            "nodes": [
                { "kind": "say", "label": "No ID" },
                { "id": "b", "kind": "say", "label": "Bell\u{7}" },
                { "id": "c", "kind": "say", "label": "x".repeat(81) },
                { "id": "d", "kind": "say", "label": "Long", "text": "x".repeat(2_001) },
                { "id": "e", "label": "No kind" },
                { "id": "f", "kind": "say" },
            ],
            "edges": [
                { "id": "p", "from": "b", "to": "d", "condition": "x".repeat(201) },
                { "id": "p", "to": "d" },
            ],
        }));

        assert_eq!(
            errors.err(),
            Some(vec![
                ("nodes[0].id".to_owned(), REQUIRED),
                (
                    "nodes[1].label".to_owned(),
                    "Remove line breaks and control characters."
                ),
                ("nodes[2].label".to_owned(), "Use at most 80 characters."),
                ("nodes[3].text".to_owned(), "Use at most 2,000 characters."),
                ("nodes[4].kind".to_owned(), REQUIRED),
                ("nodes[5].label".to_owned(), REQUIRED),
                (
                    "edges[0].condition".to_owned(),
                    "Use at most 200 characters."
                ),
                ("edges[1].from".to_owned(), REQUIRED),
                (
                    "edges[1].id".to_owned(),
                    "Another path already has this ID."
                ),
            ])
        );
    }

    #[test]
    fn too_many_paths_is_one_error_on_the_list() {
        let paths: Vec<_> = (0..201)
            .map(|i| json!({ "id": format!("p{i}"), "from": "a", "to": "a" }))
            .collect();

        let errors = parsed(json!({ "nodes": [], "edges": paths }));

        assert_eq!(
            errors.err(),
            Some(vec![("edges".to_owned(), "A plan has at most 200 paths.")])
        );
    }

    #[test]
    fn text_and_condition_default_to_empty() {
        let plan = parsed(json!({
            "nodes": [{ "id": "a", "kind": " Start ", "label": "Hi" }],
            "edges": [{ "id": "p", "from": "a", "to": "a" }],
        }))
        .expect("valid");

        assert_eq!(plan.nodes().first().map(PlanNode::text), Some(""));
        assert_eq!(
            plan.nodes().first().map(PlanNode::kind),
            Some(NodeKind::Start)
        );
        assert_eq!(plan.edges().first().map(PlanEdge::condition), Some(""));
    }
}
