//! The wire format of `/api/v1/agents/{id}/scenarios`: the set as the editor shows it, and the
//! request that replaces it. Every field is parsed by its domain type; every invalid field is
//! reported at once by its path (`scenarios[1].rubric[0].question`), and a malformed set
//! stores nothing.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use vak_domain::{RubricItem, Scenario, ScenarioError, ScenarioId, ScenarioSet, ScenarioSpec};
use vak_services::AgentScenarios;

use crate::problem::{ApiError, FieldError};

/// The answer a rubric question expects.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = ExpectedAnswer)]
pub(crate) enum ExpectedDto {
    Yes,
    No,
}

impl ExpectedDto {
    pub(crate) fn of(expected: bool) -> Self {
        if expected { Self::Yes } else { Self::No }
    }
}

/// A yes/no question the decision model answers about the conversation, and the answer that
/// passes.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = RubricItem)]
pub(crate) struct RubricItemDto {
    question: String,
    expected: ExpectedDto,
}

/// A test scenario: what a simulated caller plays, and how its conversation is scored.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = Scenario)]
pub(crate) struct ScenarioDto {
    #[schema(example = "completes-survey")]
    id: String,
    name: String,
    /// What the simulated caller wants, and what to say or not say.
    goal: String,
    /// Who the simulated caller is and how they talk; may be empty.
    persona: String,
    /// A run passes only if every required scenario passes.
    required: bool,
    /// The most caller turns before the conversation stops.
    #[schema(minimum = 1, maximum = 20)]
    max_turns: u8,
    rubric: Vec<RubricItemDto>,
}

impl From<&Scenario> for ScenarioDto {
    fn from(scenario: &Scenario) -> Self {
        Self {
            id: scenario.id().to_string(),
            name: scenario.name().to_owned(),
            goal: scenario.goal().to_owned(),
            persona: scenario.persona().to_owned(),
            required: scenario.required(),
            max_turns: scenario.max_turns(),
            rubric: scenario
                .rubric()
                .iter()
                .map(|item| RubricItemDto {
                    question: item.question().to_owned(),
                    expected: ExpectedDto::of(item.expected()),
                })
                .collect(),
        }
    }
}

/// The agent's test scenarios: the saved ones, or its use case's defaults with
/// `saved: false`.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = AgentScenarios)]
pub(crate) struct AgentScenariosDto {
    /// `false` while these are the use case's defaults.
    saved: bool,
    scenarios: Vec<ScenarioDto>,
}

impl From<&AgentScenarios> for AgentScenariosDto {
    fn from(shown: &AgentScenarios) -> Self {
        Self {
            saved: shown.saved,
            scenarios: shown
                .scenarios
                .scenarios()
                .iter()
                .map(ScenarioDto::from)
                .collect(),
        }
    }
}

/// Every scenario of the agent; it replaces the saved set.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = ScenariosInput)]
pub(crate) struct ScenariosRequest {
    /// 1 to 8 scenarios with unique IDs, at least one of them required.
    #[schema(value_type = Vec<ScenarioRequest>, required = true, min_items = 1, max_items = 8)]
    scenarios: Option<Vec<ScenarioRequest>>,
}

/// A scenario to save.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = ScenarioInput)]
pub(crate) struct ScenarioRequest {
    /// Unique within the set: 1 to 40 lowercase letters, digits and hyphens, starting with a
    /// letter.
    #[schema(value_type = String, required = true, pattern = "^[a-z][a-z0-9-]{0,39}$", example = "completes-survey")]
    id: Option<String>,
    #[schema(value_type = String, required = true, min_length = 1, max_length = 80)]
    name: Option<String>,
    #[schema(value_type = String, required = true, min_length = 1, max_length = 1000)]
    goal: Option<String>,
    #[schema(value_type = String, required = false, default = "", max_length = 1000)]
    persona: Option<String>,
    #[schema(value_type = bool, required = false, default = false)]
    required: Option<bool>,
    /// 1 to 20 caller turns.
    #[schema(value_type = u8, required = false, default = 8, minimum = 1, maximum = 20)]
    max_turns: Option<u64>,
    #[schema(value_type = Vec<RubricItemRequest>, required = true, min_items = 1, max_items = 8)]
    rubric: Option<Vec<RubricItemRequest>>,
}

/// A rubric item to save.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = RubricItemInput)]
pub(crate) struct RubricItemRequest {
    #[schema(value_type = String, required = true, min_length = 1, max_length = 300)]
    question: Option<String>,
    #[schema(value_type = ExpectedDto, required = true)]
    expected: Option<String>,
}

const REQUIRED: &str = "This field is required.";
/// A scenario's turn cap when the request leaves it out.
const DEFAULT_MAX_TURNS: u8 = 8;

/// Collects the invalid fields of one set.
#[derive(Default)]
struct Fields {
    errors: Vec<FieldError>,
}

impl Fields {
    fn push(&mut self, field: String, message: &'static str) {
        self.errors.push(FieldError { field, message });
    }

    fn required<T>(&mut self, field: String, raw: Option<T>) -> Option<T> {
        if raw.is_none() {
            self.push(field, REQUIRED);
        }
        raw
    }

    fn rubric_item(&mut self, at: &str, item: RubricItemRequest) -> Option<RubricItem> {
        let question = self.required(format!("{at}.question"), item.question);
        let expected = self
            .required(format!("{at}.expected"), item.expected)
            .and_then(|raw| match raw.trim().to_ascii_lowercase().as_str() {
                "yes" => Some(true),
                "no" => Some(false),
                _ => {
                    self.push(format!("{at}.expected"), "Choose yes or no.");
                    None
                }
            });
        let (question, expected) = (question?, expected?);
        let item = RubricItem::new(&question, expected).ok();
        if item.is_none() {
            self.push(
                format!("{at}.question"),
                "Write a question of 1 to 300 characters.",
            );
        }
        item
    }

    fn scenario(&mut self, at: usize, scenario: ScenarioRequest) -> Option<Scenario> {
        let field = |name: &str| format!("scenarios[{at}].{name}");
        let id = self.required(field("id"), scenario.id).and_then(|raw| {
            let parsed = ScenarioId::parse(&raw).ok();
            if parsed.is_none() {
                self.push(
                    field("id"),
                    "Use 1 to 40 lowercase letters, digits and hyphens, starting with a letter.",
                );
            }
            parsed
        });
        let name = self.required(field("name"), scenario.name);
        let goal = self.required(field("goal"), scenario.goal);
        let max_turns = match scenario.max_turns.map(u8::try_from) {
            None => Some(DEFAULT_MAX_TURNS),
            Some(Ok(turns)) => Some(turns),
            Some(Err(_)) => {
                self.push(field("maxTurns"), "Allow 1 to 20 caller turns.");
                None
            }
        };
        let rubric = self
            .required(field("rubric"), scenario.rubric)
            .map(|items| {
                items
                    .into_iter()
                    .enumerate()
                    .map(|(i, item)| self.rubric_item(&field(&format!("rubric[{i}]")), item))
                    .collect::<Vec<_>>()
            });
        let rubric = rubric?.into_iter().collect::<Option<Vec<_>>>();
        let spec = ScenarioSpec {
            id: id?,
            name: name?,
            goal: goal?,
            persona: scenario.persona.unwrap_or_default(),
            required: scenario.required.unwrap_or(false),
            max_turns: max_turns?,
            rubric: rubric?,
        };
        match Scenario::new(spec) {
            Ok(scenario) => Some(scenario),
            Err(error) => {
                let (name, message) = scenario_message(&error);
                self.push(field(name), message);
                None
            }
        }
    }
}

/// The field a scenario's rule names, and what to change.
fn scenario_message(error: &ScenarioError) -> (&'static str, &'static str) {
    match error {
        ScenarioError::Goal => ("goal", "Write a goal of 1 to 1,000 characters."),
        ScenarioError::Persona => ("persona", "Keep the persona to 1,000 characters."),
        ScenarioError::MaxTurns => ("maxTurns", "Allow 1 to 20 caller turns."),
        ScenarioError::RubricCount => ("rubric", "Add 1 to 8 rubric questions."),
        ScenarioError::Question => ("rubric", "Write each question in 1 to 300 characters."),
        ScenarioError::InvalidId => (
            "id",
            "Use 1 to 40 lowercase letters, digits and hyphens, starting with a letter.",
        ),
        ScenarioError::Name
        | ScenarioError::Count
        | ScenarioError::DuplicateId(_)
        | ScenarioError::NoneRequired => {
            ("name", "Give the scenario a name of 1 to 80 characters.")
        }
    }
}

impl TryFrom<ScenariosRequest> for ScenarioSet {
    type Error = ApiError;

    fn try_from(body: ScenariosRequest) -> Result<Self, ApiError> {
        let mut fields = Fields::default();
        let Some(requested) = fields.required("scenarios".to_owned(), body.scenarios) else {
            return Err(ApiError::Invalid(fields.errors));
        };
        let scenarios: Vec<Option<Scenario>> = requested
            .into_iter()
            .enumerate()
            .map(|(at, scenario)| fields.scenario(at, scenario))
            .collect();
        let Some(scenarios) = scenarios.into_iter().collect::<Option<Vec<_>>>() else {
            return Err(ApiError::Invalid(fields.errors));
        };
        ScenarioSet::new(scenarios.clone()).map_err(|error| {
            let (field, message) = match &error {
                ScenarioError::DuplicateId(id) => {
                    let at = scenarios
                        .iter()
                        .enumerate()
                        .filter(|(_, scenario)| scenario.id() == id)
                        .nth(1)
                        .map_or(0, |(at, _)| at);
                    (
                        format!("scenarios[{at}].id"),
                        "Give each scenario its own ID.",
                    )
                }
                ScenarioError::NoneRequired => (
                    "scenarios".to_owned(),
                    "Mark at least one scenario as required.",
                ),
                _ => ("scenarios".to_owned(), "Keep 1 to 8 scenarios."),
            };
            ApiError::Invalid(vec![FieldError { field, message }])
        })
    }
}
