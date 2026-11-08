//! The wire format of `/api/v1/purposes` responses and the tool catalog: strict output, mapped
//! from the domain.

use std::collections::BTreeMap;

use serde::Serialize;
use utoipa::ToSchema;
use vak_domain::{
    BuiltinTool, EffectiveCompliance, EscalationAction, HttpMethod, HttpTool, ParamKind,
    PersonalData, PlanError, Purpose, PurposeDefinition, PurposeProblem, Retention,
};
use vak_services::{ComplianceBaseline, PurposeSummary, PurposeView, ToolCatalog};

use crate::agents::{RulePackDto, UseCaseDto};
use crate::plans::{LockedTextDto, plan_message};

/// A built-in tool's LLM name.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = BuiltinTool)]
pub(crate) enum BuiltinToolDto {
    TransferToHuman,
    EndCall,
    RecordAnswer,
    ScheduleCallback,
    MarkDoNotCall,
}

impl From<BuiltinTool> for BuiltinToolDto {
    fn from(tool: BuiltinTool) -> Self {
        match tool {
            BuiltinTool::TransferToHuman => Self::TransferToHuman,
            BuiltinTool::EndCall => Self::EndCall,
            BuiltinTool::RecordAnswer => Self::RecordAnswer,
            BuiltinTool::ScheduleCallback => Self::ScheduleCallback,
            BuiltinTool::MarkDoNotCall => Self::MarkDoNotCall,
        }
    }
}

/// The HTTP method a tool calls with.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
#[schema(as = HttpMethod)]
pub(crate) enum HttpMethodDto {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl From<HttpMethod> for HttpMethodDto {
    fn from(method: HttpMethod) -> Self {
        match method {
            HttpMethod::Get => Self::Get,
            HttpMethod::Post => Self::Post,
            HttpMethod::Put => Self::Put,
            HttpMethod::Patch => Self::Patch,
            HttpMethod::Delete => Self::Delete,
        }
    }
}

/// The JSON type of a tool input.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = ToolInputType)]
pub(crate) enum ParamKindDto {
    String,
    Number,
    Integer,
    Boolean,
}

impl From<ParamKind> for ParamKindDto {
    fn from(kind: ParamKind) -> Self {
        match kind {
            ParamKind::String => Self::String,
            ParamKind::Number => Self::Number,
            ParamKind::Integer => Self::Integer,
            ParamKind::Boolean => Self::Boolean,
        }
    }
}

/// Always `object`: a tool's inputs are the properties of one JSON object.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = ToolInputsType)]
pub(crate) enum ObjectTypeDto {
    Object,
}

/// One input of a tool.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = ToolInputProperty)]
pub(crate) struct ToolPropertyDto {
    #[serde(rename = "type")]
    kind: ParamKindDto,
    description: String,
}

/// A tool's inputs, as a small JSON Schema: an object of typed properties, listed by name.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = ToolInputs)]
pub(crate) struct ToolInputsDto {
    #[serde(rename = "type")]
    kind: ObjectTypeDto,
    properties: BTreeMap<String, ToolPropertyDto>,
    /// The properties the call must carry.
    required: Vec<String>,
}

/// A custom HTTP tool. The secret is a name in the workspace's secret store, never a value.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = HttpTool)]
pub(crate) struct HttpToolDto {
    #[schema(example = "book_slot")]
    name: String,
    description: String,
    inputs: ToolInputsDto,
    method: HttpMethodDto,
    #[schema(example = "https://calendar.example.com/v1/slots")]
    url: String,
    #[schema(minimum = 100, maximum = 10000)]
    timeout_ms: u32,
    /// The secret's name, e.g. `CALENDAR_API_KEY`; `null` when the API needs none.
    #[schema(required = true)]
    secret: Option<String>,
    /// Whether the agent confirms with the caller before calling it.
    confirm: bool,
}

impl From<&HttpTool> for HttpToolDto {
    fn from(tool: &HttpTool) -> Self {
        let params = tool.inputs().params();
        Self {
            name: tool.name().as_str().to_owned(),
            description: tool.description().to_owned(),
            inputs: ToolInputsDto {
                kind: ObjectTypeDto::Object,
                properties: params
                    .iter()
                    .map(|param| {
                        (
                            param.name().as_str().to_owned(),
                            ToolPropertyDto {
                                kind: param.kind().into(),
                                description: param.description().to_owned(),
                            },
                        )
                    })
                    .collect(),
                required: params
                    .iter()
                    .filter(|param| param.required())
                    .map(|param| param.name().as_str().to_owned())
                    .collect(),
            },
            method: tool.method().into(),
            url: tool.url().as_str().to_owned(),
            timeout_ms: tool.timeout_ms(),
            secret: tool.secret().map(|secret| secret.as_str().to_owned()),
            confirm: tool.confirm(),
        }
    }
}

/// The tools a purpose turns on.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PurposeTools)]
pub(crate) struct ToolsDto {
    /// Built-in tools turned on; `mark_do_not_call` always.
    builtins: Vec<BuiltinToolDto>,
    http: Vec<HttpToolDto>,
}

/// What an escalation does.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = EscalationAction)]
pub(crate) enum EscalationActionDto {
    Transfer,
    EndCall,
    Flag,
}

/// When to escalate, and what to do then.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = EscalationRule)]
pub(crate) struct EscalationDto {
    /// A yes/no question for the decision engine.
    when: String,
    action: EscalationActionDto,
    /// What the agent says, word for word, before it ends the call; `null` for other actions.
    #[schema(required = true)]
    message: Option<String>,
}

/// Personal data the agent must never ask for.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = PersonalData)]
pub(crate) enum PersonalDataDto {
    CardNumber,
    NationalId,
    Password,
    BankAccount,
    Health,
}

impl From<PersonalData> for PersonalDataDto {
    fn from(kind: PersonalData) -> Self {
        match kind {
            PersonalData::CardNumber => Self::CardNumber,
            PersonalData::NationalId => Self::NationalId,
            PersonalData::Password => Self::Password,
            PersonalData::BankAccount => Self::BankAccount,
            PersonalData::Health => Self::Health,
        }
    }
}

/// The scope, the never-rules, escalations and limits.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = Guardrails)]
pub(crate) struct GuardrailsDto {
    /// Empty means any topic.
    allowed_topics: Vec<String>,
    never: Vec<String>,
    escalations: Vec<EscalationDto>,
    #[schema(minimum = 1, maximum = 30)]
    max_call_minutes: u32,
    never_ask: Vec<PersonalDataDto>,
}

/// What a purpose adds to its rule pack. Tighten-only: there is no audit period to shorten.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = ComplianceAdditions)]
pub(crate) struct ComplianceDto {
    opt_out_phrases: Vec<String>,
    forbidden_claims: Vec<String>,
    /// Keep recordings at most this many days; `null` keeps the pack's period.
    #[schema(required = true)]
    recordings_days: Option<u32>,
    /// Keep transcripts at most this many days; `null` keeps the pack's period.
    #[schema(required = true)]
    transcripts_days: Option<u32>,
}

/// A purpose's draft (or version), without its starting flow (`…/flow`).
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = PurposeDraft)]
pub(crate) struct DefinitionDto {
    name: String,
    description: String,
    compliance_class: UseCaseDto,
    persona: String,
    tools: ToolsDto,
    guardrails: GuardrailsDto,
    compliance: ComplianceDto,
}

impl From<&PurposeDefinition> for DefinitionDto {
    fn from(definition: &PurposeDefinition) -> Self {
        let guardrails = definition.guardrails();
        let additions = definition.additions();
        Self {
            name: definition.name().as_str().to_owned(),
            description: definition.description().to_owned(),
            compliance_class: definition.compliance().into(),
            persona: definition.persona().to_owned(),
            tools: ToolsDto {
                builtins: definition
                    .tools()
                    .builtins()
                    .iter()
                    .copied()
                    .map(Into::into)
                    .collect(),
                http: definition.tools().http().iter().map(Into::into).collect(),
            },
            guardrails: GuardrailsDto {
                allowed_topics: guardrails.allowed_topics().to_vec(),
                never: guardrails.never().to_vec(),
                escalations: guardrails
                    .escalations()
                    .iter()
                    .map(|rule| {
                        let (action, message) = match rule.action() {
                            EscalationAction::Transfer => (EscalationActionDto::Transfer, None),
                            EscalationAction::EndCall { message } => {
                                (EscalationActionDto::EndCall, Some(message.clone()))
                            }
                            EscalationAction::Flag => (EscalationActionDto::Flag, None),
                        };
                        EscalationDto {
                            when: rule.when().to_owned(),
                            action,
                            message,
                        }
                    })
                    .collect(),
                max_call_minutes: guardrails.max_call_minutes(),
                never_ask: guardrails
                    .never_ask()
                    .iter()
                    .copied()
                    .map(Into::into)
                    .collect(),
            },
            compliance: ComplianceDto {
                opt_out_phrases: additions.opt_out_phrases().to_vec(),
                forbidden_claims: additions.forbidden_claims().to_vec(),
                recordings_days: additions.recordings_days(),
                transcripts_days: additions.transcripts_days(),
            },
        }
    }
}

/// Something that keeps the draft from being saved as a version, located by `field` (and
/// the step or path for the flow).
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = PurposeProblem)]
pub(crate) struct ProblemDto {
    /// A flow rule's code (`missing_exit`, …) or `host_not_allowed`.
    code: &'static str,
    message: String,
    /// `flow`, or the tool's URL, e.g. `tools.http[0].url`.
    field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edge_id: Option<String>,
}

/// The field a problem is about.
pub(crate) fn problem_field(definition: &PurposeDefinition, problem: &PurposeProblem) -> String {
    match problem {
        PurposeProblem::Flow(_) => "flow".to_owned(),
        PurposeProblem::HostNotAllowed { tool, .. } => definition
            .tools()
            .http()
            .iter()
            .position(|http| http.name() == tool)
            .map_or_else(
                || "tools.http".to_owned(),
                |at| format!("tools.http[{at}].url"),
            ),
    }
}

fn problem(definition: &PurposeDefinition, problem: &PurposeProblem) -> ProblemDto {
    let field = problem_field(definition, problem);
    match problem {
        PurposeProblem::Flow(error) => flow_problem(error, field),
        PurposeProblem::HostNotAllowed { host, .. } => ProblemDto {
            code: "host_not_allowed",
            message: format!(
                "{host} isn't on this workspace's tool host allow-list. Use an allowed host, or \
                 ask an admin to add it to VAK_TOOL_HOSTS."
            ),
            field,
            node_id: None,
            edge_id: None,
        },
    }
}

fn flow_problem(error: &PlanError, field: String) -> ProblemDto {
    ProblemDto {
        code: error.code(),
        message: plan_message(error),
        field,
        node_id: error.node().map(ToString::to_string),
        edge_id: error.edge().map(ToString::to_string),
    }
}

/// How long call data is kept, in days.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = Retention)]
#[expect(
    clippy::struct_field_names,
    reason = "the wire names say the unit, as the rule packs do"
)]
pub(crate) struct RetentionDto {
    recordings_days: u32,
    transcripts_days: u32,
    audit_days: u32,
}

impl From<Retention> for RetentionDto {
    fn from(retention: Retention) -> Self {
        Self {
            recordings_days: retention.recordings_days(),
            transcripts_days: retention.transcripts_days(),
            audit_days: retention.audit_days(),
        }
    }
}

/// The policy that applies with the purpose's additions.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = EffectiveCompliance)]
pub(crate) struct EffectiveDto {
    opt_out_phrases: Vec<String>,
    forbidden_claims: Vec<String>,
    retention: RetentionDto,
}

impl From<&EffectiveCompliance> for EffectiveDto {
    fn from(effective: &EffectiveCompliance) -> Self {
        Self {
            opt_out_phrases: effective.opt_out_phrases().to_vec(),
            forbidden_claims: effective.forbidden_claims().to_vec(),
            retention: effective.retention().into(),
        }
    }
}

/// A shipped jurisdiction's rule pack for the purpose's class: its baseline, read-only, and
/// the effective policy with the purpose's additions.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = ComplianceBaseline)]
pub(crate) struct BaselineDto {
    #[schema(example = "US")]
    jurisdiction: String,
    rule_pack: RulePackDto,
    locked_texts: Vec<LockedTextDto>,
    opt_out_phrases: Vec<String>,
    opt_out_confirmation: String,
    forbidden_claims: Vec<String>,
    retention: RetentionDto,
    effective: EffectiveDto,
}

impl From<&ComplianceBaseline> for BaselineDto {
    fn from(baseline: &ComplianceBaseline) -> Self {
        let pack = &baseline.pack;
        Self {
            jurisdiction: pack.jurisdiction().to_string(),
            rule_pack: pack.reference().into(),
            locked_texts: pack.locked_texts().iter().map(Into::into).collect(),
            opt_out_phrases: pack.opt_out_phrases().to_vec(),
            opt_out_confirmation: pack.opt_out_confirmation().to_owned(),
            forbidden_claims: pack.forbidden_claims().to_vec(),
            retention: pack.retention().into(),
            effective: (&baseline.effective).into(),
        }
    }
}

/// A purpose: built-in, or one of the workspace's, with its draft.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = Purpose)]
pub(crate) struct PurposeDto {
    #[schema(format = "uuid")]
    id: String,
    /// Built-in purposes are read-only.
    system: bool,
    /// The latest saved version; `null` before the first.
    #[schema(required = true, minimum = 1)]
    latest_version: Option<u32>,
    /// Whether the draft differs from the latest version.
    unsaved_changes: bool,
    /// Active agents that pin a version of it.
    used_by: u32,
    draft: DefinitionDto,
    /// What keeps the draft from being saved as a version.
    problems: Vec<ProblemDto>,
    /// Per shipped jurisdiction, by code.
    baselines: Vec<BaselineDto>,
}

impl From<&PurposeView> for PurposeDto {
    fn from(view: &PurposeView) -> Self {
        let purpose: &Purpose = &view.purpose;
        let draft = purpose.draft();
        Self {
            id: purpose.id().to_string(),
            system: purpose.is_system(),
            latest_version: purpose.latest_version(),
            unsaved_changes: purpose.has_unsaved_changes(),
            used_by: view.used_by,
            draft: draft.into(),
            problems: view
                .problems
                .iter()
                .map(|found| problem(draft, found))
                .collect(),
            baselines: view.baselines.iter().map(Into::into).collect(),
        }
    }
}

/// A purpose as lists show it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = PurposeSummary)]
pub(crate) struct PurposeSummaryDto {
    #[schema(format = "uuid")]
    id: String,
    name: String,
    description: String,
    compliance_class: UseCaseDto,
    system: bool,
    #[schema(required = true, minimum = 1)]
    latest_version: Option<u32>,
    unsaved_changes: bool,
    used_by: u32,
}

impl From<&PurposeSummary> for PurposeSummaryDto {
    fn from(summary: &PurposeSummary) -> Self {
        Self {
            id: summary.id.to_string(),
            name: summary.name.as_str().to_owned(),
            description: summary.description.clone(),
            compliance_class: summary.compliance.into(),
            system: summary.system,
            latest_version: summary.latest_version,
            unsaved_changes: summary.unsaved_changes,
            used_by: summary.used_by,
        }
    }
}

/// The built-in purposes in use-case order, then the workspace's own, newest first.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PurposeList)]
pub(crate) struct PurposeListDto {
    pub(crate) items: Vec<PurposeSummaryDto>,
}

/// A built-in tool, as pickers show it.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = BuiltinToolInfo)]
pub(crate) struct BuiltinToolInfoDto {
    name: BuiltinToolDto,
    label: &'static str,
    description: &'static str,
    /// Always on: a purpose can't turn it off.
    locked: bool,
}

/// What purposes can use: the built-in tools and the hosts HTTP tools may call.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = ToolCatalog)]
pub(crate) struct ToolCatalogDto {
    builtins: Vec<BuiltinToolInfoDto>,
    /// Exact host names, or `*.example.com` for subdomains. Set by the deployment
    /// (`VAK_TOOL_HOSTS`), not by operators.
    allowed_hosts: Vec<String>,
}

/// The operator-facing name of a built-in tool.
const fn label(tool: BuiltinTool) -> &'static str {
    match tool {
        BuiltinTool::TransferToHuman => "Transfer to a person",
        BuiltinTool::EndCall => "End the call",
        BuiltinTool::RecordAnswer => "Record an answer",
        BuiltinTool::ScheduleCallback => "Schedule a callback",
        BuiltinTool::MarkDoNotCall => "Opt out",
    }
}

impl From<&ToolCatalog> for ToolCatalogDto {
    fn from(catalog: &ToolCatalog) -> Self {
        Self {
            builtins: catalog
                .builtins
                .iter()
                .map(|tool| BuiltinToolInfoDto {
                    name: (*tool).into(),
                    label: label(*tool),
                    description: tool.description(),
                    locked: tool.locked(),
                })
                .collect(),
            allowed_hosts: catalog.allowed_hosts.clone(),
        }
    }
}
