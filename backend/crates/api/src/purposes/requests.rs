//! Request bodies of `/api/v1/purposes` and their validation. Every field is parsed by its
//! domain type; every invalid field is reported at once, by its path
//! (`tools.http[0].url`), and an invalid body stores nothing.

use std::collections::BTreeMap;

use serde::Deserialize;
use utoipa::ToSchema;
use vak_domain::{
    BuiltinTool, ComplianceAdditions, ComplianceAdditionsSpec, ComplianceError, EscalationAction,
    EscalationRule, GuardrailError, Guardrails, GuardrailsSpec, HttpMethod, HttpTool, HttpToolSpec,
    ParamKind, PersonalData, PurposeDefinition, PurposeId, PurposeName, PurposePatch, PurposeTools,
    SecretRef, ToolError, ToolInputs, ToolName, ToolParam, ToolUrl, UseCase,
};
use vak_services::DuplicatePurpose;

use super::dto::{
    BuiltinToolDto, EscalationActionDto, HttpMethodDto, ObjectTypeDto, ParamKindDto,
    PersonalDataDto,
};
use crate::agents::UseCaseDto;
use crate::problem::{ApiError, FieldError};

const REQUIRED: &str = "This field is required.";

/// Make a purpose of the workspace's as a copy of another one.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = CreatePurposeInput)]
pub(crate) struct CreatePurposeRequest {
    /// The purpose to copy: built-in or the workspace's.
    #[schema(value_type = String, format = "uuid", required = true)]
    duplicate_of: Option<String>,
    /// The version to copy; by default the latest (the draft when there is none).
    #[schema(value_type = u32, required = false, minimum = 1, nullable = false)]
    version: Option<u32>,
    /// By default "Copy of …".
    #[schema(value_type = String, required = false, min_length = 1, max_length = 80)]
    name: Option<String>,
}

/// One input of a tool.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = ToolInputPropertyInput)]
pub(crate) struct ToolPropertyRequest {
    #[serde(rename = "type")]
    #[schema(value_type = ParamKindDto, required = true)]
    kind: Option<String>,
    /// Up to 200 characters.
    #[schema(value_type = String, required = false, default = "", max_length = 200)]
    description: Option<String>,
}

/// A tool's inputs as a small JSON Schema: `type: object`, typed properties, `required`.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = ToolInputsInput)]
pub(crate) struct ToolInputsRequest {
    #[serde(rename = "type")]
    #[schema(value_type = ObjectTypeDto, required = false)]
    kind: Option<String>,
    /// At most 10, each named like a tool.
    #[schema(value_type = BTreeMap<String, ToolPropertyRequest>, required = false)]
    properties: Option<BTreeMap<String, ToolPropertyRequest>>,
    #[schema(value_type = Vec<String>, required = false)]
    required: Option<Vec<String>>,
}

/// A custom HTTP tool. `secret` names a secret in the workspace's secret store; the API has
/// no field for a secret's value.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = HttpToolInput)]
pub(crate) struct HttpToolRequest {
    /// 1-64 lowercase letters, digits or `_`, starting with a letter.
    #[schema(value_type = String, required = true, pattern = "^[a-z][a-z0-9_]{0,63}$")]
    name: Option<String>,
    /// What the tool does, 1-500 characters: the LLM reads it.
    #[schema(value_type = String, required = true, min_length = 1, max_length = 500)]
    description: Option<String>,
    #[schema(value_type = ToolInputsRequest, required = false)]
    inputs: Option<ToolInputsRequest>,
    #[schema(value_type = HttpMethodDto, required = true)]
    method: Option<String>,
    /// `https://`, with a host on the tool host allow-list.
    #[schema(value_type = String, required = true, max_length = 2048)]
    url: Option<String>,
    #[schema(value_type = u32, required = false, default = 3000, minimum = 100, maximum = 10000)]
    timeout_ms: Option<u32>,
    /// A secret's name, e.g. `CALENDAR_API_KEY`.
    #[schema(value_type = Option<String>, required = false, pattern = "^[A-Za-z][A-Za-z0-9_-]{0,63}$")]
    secret: Option<String>,
    #[schema(value_type = bool, required = false, default = false)]
    confirm: Option<bool>,
}

/// The tools: built-in ones to turn on (`mark_do_not_call` always is) and HTTP tools.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = PurposeToolsInput)]
pub(crate) struct ToolsRequest {
    #[schema(value_type = Vec<BuiltinToolDto>, required = false)]
    builtins: Option<Vec<String>>,
    /// At most 10.
    #[schema(value_type = Vec<HttpToolRequest>, required = false, max_items = 10)]
    http: Option<Vec<HttpToolRequest>>,
}

/// When to escalate, and what to do then.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = EscalationRuleInput)]
pub(crate) struct EscalationRequest {
    /// A yes/no question, 1-200 characters.
    #[schema(value_type = String, required = true, min_length = 1, max_length = 200)]
    when: Option<String>,
    #[schema(value_type = EscalationActionDto, required = true)]
    action: Option<String>,
    /// For `end_call`: what to say before ending, 1-500 characters.
    #[schema(value_type = Option<String>, required = false, max_length = 500)]
    message: Option<String>,
}

/// Guardrails; a list left out is empty, the call length 30 minutes.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = GuardrailsInput)]
pub(crate) struct GuardrailsRequest {
    #[schema(value_type = Vec<String>, required = false, max_items = 20)]
    allowed_topics: Option<Vec<String>>,
    #[schema(value_type = Vec<String>, required = false, max_items = 20)]
    never: Option<Vec<String>>,
    #[schema(value_type = Vec<EscalationRequest>, required = false, max_items = 5)]
    escalations: Option<Vec<EscalationRequest>>,
    #[schema(value_type = u32, required = false, default = 30, minimum = 1, maximum = 30)]
    max_call_minutes: Option<u32>,
    #[schema(value_type = Vec<PersonalDataDto>, required = false)]
    never_ask: Option<Vec<String>>,
}

/// Compliance additions: tighten-only. There is no field for the audit period.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schema(as = ComplianceAdditionsInput)]
pub(crate) struct ComplianceRequest {
    #[schema(value_type = Vec<String>, required = false, max_items = 20)]
    opt_out_phrases: Option<Vec<String>>,
    #[schema(value_type = Vec<String>, required = false, max_items = 20)]
    forbidden_claims: Option<Vec<String>>,
    #[schema(value_type = Option<u32>, required = false, minimum = 1, maximum = 36500)]
    recordings_days: Option<u32>,
    #[schema(value_type = Option<u32>, required = false, minimum = 1, maximum = 36500)]
    transcripts_days: Option<u32>,
}

/// Change the draft (autosave). Send only the sections that changed; each replaces that
/// section whole.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = UpdatePurposeInput)]
pub(crate) struct UpdatePurposeRequest {
    #[schema(value_type = String, required = false, min_length = 1, max_length = 80)]
    name: Option<String>,
    #[schema(value_type = String, required = false, max_length = 500)]
    description: Option<String>,
    #[schema(value_type = UseCaseDto, required = false)]
    compliance_class: Option<String>,
    /// The default persona prompt, up to 2,000 characters.
    #[schema(value_type = String, required = false, max_length = 2000)]
    persona: Option<String>,
    #[schema(value_type = ToolsRequest, required = false)]
    tools: Option<ToolsRequest>,
    #[schema(value_type = GuardrailsRequest, required = false)]
    guardrails: Option<GuardrailsRequest>,
    #[schema(value_type = ComplianceRequest, required = false)]
    compliance: Option<ComplianceRequest>,
}

/// Collects the invalid fields of one body.
#[derive(Default)]
struct Fields {
    errors: Vec<FieldError>,
}

impl Fields {
    fn push(&mut self, field: impl Into<String>, message: &'static str) {
        self.errors.push(FieldError {
            field: field.into(),
            message,
        });
    }

    /// `result`'s value, or `None` after recording `field` with what its error says to fix.
    fn check<T, E>(
        &mut self,
        field: impl Into<String>,
        result: Result<T, E>,
        message: impl FnOnce(&E) -> &'static str,
    ) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.push(field, message(&error));
                None
            }
        }
    }

    /// `raw`, or `None` after recording `field` as missing.
    fn required<T>(&mut self, field: impl Into<String>, raw: Option<T>) -> Option<T> {
        if raw.is_none() {
            self.push(field, REQUIRED);
        }
        raw
    }

    fn finish<T>(self, value: Option<T>) -> Result<T, ApiError> {
        match value {
            Some(value) if self.errors.is_empty() => Ok(value),
            _ => Err(ApiError::Invalid(self.errors)),
        }
    }
}

pub(crate) fn purpose_id(raw: &str) -> Result<PurposeId, ApiError> {
    PurposeId::parse(raw).map_err(|_| ApiError::NotFound)
}

fn name_message(_: &vak_domain::PurposeNameError) -> &'static str {
    "Enter a name of 1 to 80 characters, without line breaks."
}

impl TryFrom<CreatePurposeRequest> for DuplicatePurpose {
    type Error = ApiError;

    fn try_from(body: CreatePurposeRequest) -> Result<Self, ApiError> {
        let mut fields = Fields::default();
        let source = fields
            .required("duplicateOf", body.duplicate_of)
            .and_then(|raw| {
                fields.check(
                    "duplicateOf",
                    PurposeId::parse(&raw),
                    |_| "Choose a purpose to copy.",
                )
            });
        let name = body.name.and_then(|raw| {
            fields
                .check("name", PurposeName::parse(&raw), name_message)
                .map(Some)
        });
        let command = source.map(|source| Self {
            source,
            version: body.version,
            name: name.flatten(),
        });
        fields.finish(command)
    }
}

impl UpdatePurposeRequest {
    /// The patch, checked field by field.
    pub(crate) fn into_patch(self) -> Result<PurposePatch, ApiError> {
        let mut fields = Fields::default();
        let name = self
            .name
            .and_then(|raw| fields.check("name", PurposeName::parse(&raw), name_message));
        if self
            .description
            .as_ref()
            .is_some_and(|text| text.trim().chars().count() > PurposeDefinition::MAX_DESCRIPTION)
        {
            fields.push("description", "Use at most 500 characters.");
        }
        if self
            .persona
            .as_ref()
            .is_some_and(|text| text.trim().chars().count() > PurposeDefinition::MAX_PERSONA)
        {
            fields.push("persona", "Use at most 2,000 characters.");
        }
        let compliance = self.compliance_class.and_then(|raw| {
            fields.check(
                "complianceClass",
                UseCase::parse(&raw),
                |_| "Choose support, service, booking, survey, telemarketing or custom.",
            )
        });
        let tools = self.tools.and_then(|tools| tools_of(&mut fields, tools));
        let guardrails = self
            .guardrails
            .and_then(|guardrails| guardrails_of(&mut fields, guardrails));
        let additions = self
            .compliance
            .and_then(|compliance| compliance_of(&mut fields, compliance));
        let patch = PurposePatch {
            name,
            description: self.description,
            compliance,
            persona: self.persona,
            flow: None,
            tools,
            guardrails,
            additions,
        };
        fields.finish(Some(patch))
    }
}

fn tool_message(error: &ToolError) -> &'static str {
    match error {
        ToolError::InvalidName => {
            "Use 1 to 64 lowercase letters, digits or '_', starting with a letter."
        }
        ToolError::ReservedName { .. } => "A built-in tool has this name. Choose another.",
        ToolError::DuplicateName { .. } => "Another tool or input has this name.",
        ToolError::InvalidDescription => "Say what it does in 1 to 500 characters.",
        ToolError::TooManyTools { .. } => "A purpose has at most 10 HTTP tools.",
        ToolError::TooManyInputs { .. } => "A tool has at most 10 inputs.",
        ToolError::InvalidUrl(_) => {
            "Use an https:// URL with a host name (not an IP address), without user names or a #fragment."
        }
        ToolError::TimeoutOutOfRange { .. } => "Use a timeout from 100 to 10000 ms.",
        ToolError::InvalidSecretRef => {
            "Use a secret's name: letters, digits, '_' or '-', starting with a letter."
        }
        ToolError::InvalidHostPattern => "Use a host name.",
        ToolError::UnknownBuiltin => "Choose a built-in tool from the catalog.",
        ToolError::UnknownMethod => "Choose GET, POST, PUT, PATCH or DELETE.",
        ToolError::UnknownParamKind => "Choose string, number, integer or boolean.",
    }
}

fn tools_of(fields: &mut Fields, body: ToolsRequest) -> Option<PurposeTools> {
    let before = fields.errors.len();
    let builtins: Vec<BuiltinTool> = body
        .builtins
        .unwrap_or_default()
        .iter()
        .enumerate()
        .filter_map(|(at, raw)| {
            fields.check(
                format!("tools.builtins[{at}]"),
                BuiltinTool::parse(raw),
                tool_message,
            )
        })
        .collect();
    let http: Vec<HttpTool> = body
        .http
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .filter_map(|(at, tool)| http_tool_of(fields, at, tool))
        .collect();
    if fields.errors.len() > before {
        return None;
    }
    let names: Vec<String> = http
        .iter()
        .map(|tool| tool.name().as_str().to_owned())
        .collect();
    match PurposeTools::new(builtins, http) {
        Ok(tools) => Some(tools),
        Err(error) => {
            let field = match &error {
                ToolError::ReservedName { name } | ToolError::DuplicateName { name } => names
                    .iter()
                    .rposition(|candidate| candidate == name)
                    .map_or_else(
                        || "tools.http".to_owned(),
                        |at| format!("tools.http[{at}].name"),
                    ),
                _ => "tools.http".to_owned(),
            };
            fields.push(field, tool_message(&error));
            None
        }
    }
}

fn http_tool_of(fields: &mut Fields, at: usize, body: HttpToolRequest) -> Option<HttpTool> {
    let key = |name: &str| format!("tools.http[{at}].{name}");
    let name = fields
        .required(key("name"), body.name)
        .and_then(|raw| fields.check(key("name"), ToolName::parse(&raw), tool_message));
    let description = fields.required(key("description"), body.description);
    let inputs = inputs_of(fields, at, body.inputs);
    let method = fields
        .required(key("method"), body.method)
        .and_then(|raw| fields.check(key("method"), HttpMethod::parse(&raw), tool_message));
    let url = fields
        .required(key("url"), body.url)
        .and_then(|raw| fields.check(key("url"), ToolUrl::parse(&raw), tool_message));
    let secret = match body.secret {
        Some(raw) if !raw.trim().is_empty() => fields
            .check(key("secret"), SecretRef::parse(&raw), tool_message)
            .map(Some),
        _ => Some(None),
    };
    let (name, description, inputs, method, url, secret) =
        (name?, description?, inputs?, method?, url?, secret?);
    let spec = HttpToolSpec {
        name,
        description,
        inputs,
        method,
        url,
        timeout_ms: body.timeout_ms.unwrap_or(HttpTool::DEFAULT_TIMEOUT_MS),
        secret,
        confirm: body.confirm.unwrap_or(false),
    };
    match HttpTool::new(spec) {
        Ok(tool) => Some(tool),
        Err(error) => {
            let field = match error {
                ToolError::TimeoutOutOfRange { .. } => key("timeoutMs"),
                _ => key("description"),
            };
            fields.push(field, tool_message(&error));
            None
        }
    }
}

fn inputs_of(
    fields: &mut Fields,
    at: usize,
    body: Option<ToolInputsRequest>,
) -> Option<ToolInputs> {
    let key = |name: &str| format!("tools.http[{at}].inputs{name}");
    let Some(body) = body else {
        return Some(ToolInputs::default());
    };
    if body
        .kind
        .as_deref()
        .is_some_and(|kind| !kind.trim().eq_ignore_ascii_case("object"))
    {
        fields.push(key(".type"), "A tool's inputs are an object.");
    }
    let required = body.required.unwrap_or_default();
    let properties = body.properties.unwrap_or_default();
    if required.iter().any(|name| !properties.contains_key(name)) {
        fields.push(key(".required"), "List only inputs the tool has.");
    }
    let before = fields.errors.len();
    let params: Vec<ToolParam> = properties
        .into_iter()
        .filter_map(|(name, property)| {
            let field = key(&format!(".properties.{name}"));
            let is_required = required.contains(&name);
            let name = fields.check(field.clone(), ToolName::parse(&name), tool_message)?;
            let kind = fields
                .required(format!("{field}.type"), property.kind)
                .and_then(|raw| {
                    fields.check(
                        format!("{field}.type"),
                        ParamKind::parse(&raw),
                        tool_message,
                    )
                })?;
            let description = property.description.unwrap_or_default();
            fields.check(
                format!("{field}.description"),
                ToolParam::new(name, kind, &description, is_required),
                |_| "Use at most 200 characters.",
            )
        })
        .collect();
    if fields.errors.len() > before {
        return None;
    }
    fields.check(key(""), ToolInputs::new(params), tool_message)
}

fn guardrail_message(error: &GuardrailError) -> &'static str {
    match error {
        GuardrailError::EmptyItem => "Remove the empty item.",
        GuardrailError::ItemTooLong { .. } => "Use at most 200 characters per item.",
        GuardrailError::TooManyItems { .. } => "Keep at most 20 items.",
        GuardrailError::TooManyEscalations { .. } => "Keep at most 5 escalation rules.",
        GuardrailError::InvalidQuestion => "Ask a yes/no question of 1 to 200 characters.",
        GuardrailError::InvalidMessage => "Say what to say before ending, in 1 to 500 characters.",
        GuardrailError::CallLengthOutOfRange { .. } => "Use 1 to 30 minutes.",
        GuardrailError::UnknownPersonalData => {
            "Choose card_number, national_id, password, bank_account or health."
        }
    }
}

fn escalation_of(
    fields: &mut Fields,
    at: usize,
    body: EscalationRequest,
) -> Option<EscalationRule> {
    let key = |name: &str| format!("guardrails.escalations[{at}].{name}");
    let when = fields.required(key("when"), body.when);
    let action = fields.required(key("action"), body.action).and_then(|raw| {
        match raw.trim().to_ascii_lowercase().as_str() {
            "transfer" => Some(EscalationAction::Transfer),
            "flag" => Some(EscalationAction::Flag),
            "end_call" => Some(EscalationAction::EndCall {
                message: body.message.clone().unwrap_or_default(),
            }),
            _ => {
                fields.push(key("action"), "Choose transfer, end_call or flag.");
                None
            }
        }
    });
    let (when, action) = (when?, action?);
    match EscalationRule::new(&when, action) {
        Ok(rule) => Some(rule),
        Err(error) => {
            let field = match error {
                GuardrailError::InvalidMessage => key("message"),
                _ => key("when"),
            };
            fields.push(field, guardrail_message(&error));
            None
        }
    }
}

fn guardrails_of(fields: &mut Fields, body: GuardrailsRequest) -> Option<Guardrails> {
    let before = fields.errors.len();
    let escalations: Vec<EscalationRule> = body
        .escalations
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .filter_map(|(at, rule)| escalation_of(fields, at, rule))
        .collect();
    let max_call_minutes = body
        .max_call_minutes
        .unwrap_or(Guardrails::MAX_CALL_MINUTES);
    let minutes = Guardrails::MIN_CALL_MINUTES..=Guardrails::MAX_CALL_MINUTES;
    if !minutes.contains(&max_call_minutes) {
        fields.push("guardrails.maxCallMinutes", "Use 1 to 30 minutes.");
    }
    let never_ask: Vec<PersonalData> = body
        .never_ask
        .unwrap_or_default()
        .iter()
        .enumerate()
        .filter_map(|(at, raw)| {
            fields.check(
                format!("guardrails.neverAsk[{at}]"),
                PersonalData::parse(raw),
                guardrail_message,
            )
        })
        .collect();
    if fields.errors.len() > before {
        return None;
    }
    let topics = body.allowed_topics.unwrap_or_default();
    let never = body.never.unwrap_or_default();
    let spec = GuardrailsSpec {
        allowed_topics: topics.clone(),
        never: never.clone(),
        escalations,
        max_call_minutes,
        never_ask,
    };
    match Guardrails::new(spec) {
        Ok(guardrails) => Some(guardrails),
        Err(error) => {
            // Lists are checked in order: name the first one that breaks the rule.
            let topics_ok = Guardrails::new(GuardrailsSpec {
                allowed_topics: topics,
                never: Vec::new(),
                escalations: Vec::new(),
                max_call_minutes: Guardrails::MAX_CALL_MINUTES,
                never_ask: Vec::new(),
            })
            .is_ok();
            let field = match error {
                GuardrailError::TooManyEscalations { .. } => "guardrails.escalations",
                _ if !topics_ok => "guardrails.allowedTopics",
                _ => "guardrails.never",
            };
            fields.push(field, guardrail_message(&error));
            None
        }
    }
}

fn compliance_of(fields: &mut Fields, body: ComplianceRequest) -> Option<ComplianceAdditions> {
    let phrases = body.opt_out_phrases.unwrap_or_default();
    let claims = body.forbidden_claims.unwrap_or_default();
    let day_ok = |days: Option<u32>| {
        days.is_none_or(|days| (1..=ComplianceAdditions::MAX_DAYS).contains(&days))
    };
    let mut ok = true;
    for (field, days) in [
        ("compliance.recordingsDays", body.recordings_days),
        ("compliance.transcriptsDays", body.transcripts_days),
    ] {
        if !day_ok(days) {
            fields.push(
                field,
                "Use 1 to 36500 days, or leave it empty for the pack's period.",
            );
            ok = false;
        }
    }
    let phrases_ok = ComplianceAdditions::new(ComplianceAdditionsSpec {
        opt_out_phrases: phrases.clone(),
        ..ComplianceAdditionsSpec::default()
    });
    if let Err(error) = &phrases_ok {
        fields.push("compliance.optOutPhrases", compliance_message(error));
        ok = false;
    }
    let claims_ok = ComplianceAdditions::new(ComplianceAdditionsSpec {
        forbidden_claims: claims.clone(),
        ..ComplianceAdditionsSpec::default()
    });
    if let Err(error) = &claims_ok {
        fields.push("compliance.forbiddenClaims", compliance_message(error));
        ok = false;
    }
    if !ok {
        return None;
    }
    let spec = ComplianceAdditionsSpec {
        opt_out_phrases: phrases,
        forbidden_claims: claims,
        recordings_days: body.recordings_days,
        transcripts_days: body.transcripts_days,
    };
    fields.check(
        "compliance",
        ComplianceAdditions::new(spec),
        compliance_message,
    )
}

fn compliance_message(error: &ComplianceError) -> &'static str {
    match error {
        ComplianceError::EmptyItem => "Remove the empty item.",
        ComplianceError::ItemTooLong { .. } => {
            "Use at most 100 characters per phrase and 200 per claim."
        }
        ComplianceError::TooManyItems { .. } => "Keep at most 20 items.",
        ComplianceError::InvalidRetention => "Use 1 to 36500 days.",
    }
}
