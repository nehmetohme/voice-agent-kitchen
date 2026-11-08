//! JSONB shapes of a purpose definition (`purposes.draft`, `purpose_versions.definition`).
//! Every value read back goes through the domain's constructors, and the stored hash is
//! recomputed and compared, so a row the domain would reject never becomes a `Purpose`.

use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use vak_domain::{
    BuiltinTool, ComplianceAdditions, ComplianceAdditionsSpec, ContentHash, EscalationAction,
    EscalationRule, Guardrails, GuardrailsSpec, HttpMethod, HttpTool, HttpToolSpec, ParamKind,
    PersonalData, PurposeDefinition, PurposeName, PurposeSpec, PurposeTools, SecretRef, ToolInputs,
    ToolName, ToolParam, ToolUrl, UseCase,
};
use vak_repositories::RepoError;

use crate::records::{PlanRecord, corrupt};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DefinitionRecord {
    name: String,
    description: String,
    compliance: String,
    persona: String,
    flow: PlanRecord,
    tools: ToolsRecord,
    guardrails: GuardrailsRecord,
    additions: AdditionsRecord,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolsRecord {
    builtins: Vec<String>,
    http: Vec<HttpToolRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HttpToolRecord {
    name: String,
    description: String,
    inputs: Vec<ParamRecord>,
    method: String,
    url: String,
    timeout_ms: u32,
    /// The secret's name; never a value.
    secret: Option<String>,
    confirm: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParamRecord {
    name: String,
    kind: String,
    description: String,
    required: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GuardrailsRecord {
    allowed_topics: Vec<String>,
    never: Vec<String>,
    escalations: Vec<EscalationRecord>,
    max_call_minutes: u32,
    never_ask: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EscalationRecord {
    when: String,
    action: String,
    message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AdditionsRecord {
    opt_out_phrases: Vec<String>,
    forbidden_claims: Vec<String>,
    recordings_days: Option<u32>,
    transcripts_days: Option<u32>,
}

impl DefinitionRecord {
    pub(crate) fn of(definition: &PurposeDefinition) -> Json<Self> {
        let tools = definition.tools();
        let guardrails = definition.guardrails();
        let additions = definition.additions();
        Json(Self {
            name: definition.name().as_str().to_owned(),
            description: definition.description().to_owned(),
            compliance: definition.compliance().as_str().to_owned(),
            persona: definition.persona().to_owned(),
            flow: PlanRecord::from_plan(definition.flow()),
            tools: ToolsRecord {
                builtins: tools
                    .builtins()
                    .iter()
                    .map(|tool| tool.name().to_owned())
                    .collect(),
                http: tools.http().iter().map(HttpToolRecord::of).collect(),
            },
            guardrails: GuardrailsRecord {
                allowed_topics: guardrails.allowed_topics().to_vec(),
                never: guardrails.never().to_vec(),
                escalations: guardrails
                    .escalations()
                    .iter()
                    .map(|rule| EscalationRecord {
                        when: rule.when().to_owned(),
                        action: rule.action().as_str().to_owned(),
                        message: match rule.action() {
                            EscalationAction::EndCall { message } => Some(message.clone()),
                            EscalationAction::Transfer | EscalationAction::Flag => None,
                        },
                    })
                    .collect(),
                max_call_minutes: guardrails.max_call_minutes(),
                never_ask: guardrails
                    .never_ask()
                    .iter()
                    .map(|kind| kind.as_str().to_owned())
                    .collect(),
            },
            additions: AdditionsRecord {
                opt_out_phrases: additions.opt_out_phrases().to_vec(),
                forbidden_claims: additions.forbidden_claims().to_vec(),
                recordings_days: additions.recordings_days(),
                transcripts_days: additions.transcripts_days(),
            },
        })
    }

    /// The definition, through the domain's constructors, if it hashes to `stored_hash`.
    pub(crate) fn into_definition(self, stored_hash: &str) -> Result<PurposeDefinition, RepoError> {
        let builtins = self
            .tools
            .builtins
            .iter()
            .map(|name| BuiltinTool::parse(name).map_err(corrupt("purpose.tools.builtins")))
            .collect::<Result<Vec<_>, _>>()?;
        let http = self
            .tools
            .http
            .into_iter()
            .map(HttpToolRecord::into_tool)
            .collect::<Result<Vec<_>, _>>()?;
        let escalations = self
            .guardrails
            .escalations
            .into_iter()
            .map(EscalationRecord::into_rule)
            .collect::<Result<Vec<_>, _>>()?;
        let never_ask = self
            .guardrails
            .never_ask
            .iter()
            .map(|kind| PersonalData::parse(kind).map_err(corrupt("purpose.guardrails.neverAsk")))
            .collect::<Result<Vec<_>, _>>()?;
        let definition = PurposeDefinition::new(PurposeSpec {
            name: PurposeName::parse(&self.name).map_err(corrupt("purpose.name"))?,
            description: self.description,
            compliance: UseCase::parse(&self.compliance).map_err(corrupt("purpose.compliance"))?,
            persona: self.persona,
            flow: self.flow.into_plan()?,
            tools: PurposeTools::new(builtins, http).map_err(corrupt("purpose.tools"))?,
            guardrails: Guardrails::new(GuardrailsSpec {
                allowed_topics: self.guardrails.allowed_topics,
                never: self.guardrails.never,
                escalations,
                max_call_minutes: self.guardrails.max_call_minutes,
                never_ask,
            })
            .map_err(corrupt("purpose.guardrails"))?,
            additions: ComplianceAdditions::new(ComplianceAdditionsSpec {
                opt_out_phrases: self.additions.opt_out_phrases,
                forbidden_claims: self.additions.forbidden_claims,
                recordings_days: self.additions.recordings_days,
                transcripts_days: self.additions.transcripts_days,
            })
            .map_err(corrupt("purpose.additions"))?,
        })
        .map_err(corrupt("purpose"))?;
        let stored = ContentHash::parse(stored_hash).map_err(corrupt("purpose.hash"))?;
        if definition.content_hash() != stored {
            return Err(corrupt("purpose.hash")(()));
        }
        Ok(definition)
    }
}

impl HttpToolRecord {
    fn of(tool: &HttpTool) -> Self {
        Self {
            name: tool.name().as_str().to_owned(),
            description: tool.description().to_owned(),
            inputs: tool
                .inputs()
                .params()
                .iter()
                .map(|param| ParamRecord {
                    name: param.name().as_str().to_owned(),
                    kind: param.kind().as_str().to_owned(),
                    description: param.description().to_owned(),
                    required: param.required(),
                })
                .collect(),
            method: tool.method().as_str().to_owned(),
            url: tool.url().as_str().to_owned(),
            timeout_ms: tool.timeout_ms(),
            secret: tool.secret().map(|secret| secret.as_str().to_owned()),
            confirm: tool.confirm(),
        }
    }

    fn into_tool(self) -> Result<HttpTool, RepoError> {
        let params = self
            .inputs
            .into_iter()
            .map(|param| {
                ToolParam::new(
                    ToolName::parse(&param.name).map_err(corrupt("purpose.tool.input.name"))?,
                    ParamKind::parse(&param.kind).map_err(corrupt("purpose.tool.input.kind"))?,
                    &param.description,
                    param.required,
                )
                .map_err(corrupt("purpose.tool.input"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let secret = self
            .secret
            .map(|name| SecretRef::parse(&name).map_err(corrupt("purpose.tool.secret")))
            .transpose()?;
        HttpTool::new(HttpToolSpec {
            name: ToolName::parse(&self.name).map_err(corrupt("purpose.tool.name"))?,
            description: self.description,
            inputs: ToolInputs::new(params).map_err(corrupt("purpose.tool.inputs"))?,
            method: HttpMethod::parse(&self.method).map_err(corrupt("purpose.tool.method"))?,
            url: ToolUrl::parse(&self.url).map_err(corrupt("purpose.tool.url"))?,
            timeout_ms: self.timeout_ms,
            secret,
            confirm: self.confirm,
        })
        .map_err(corrupt("purpose.tool"))
    }
}

impl EscalationRecord {
    fn into_rule(self) -> Result<EscalationRule, RepoError> {
        let action = match self.action.as_str() {
            "transfer" => EscalationAction::Transfer,
            "flag" => EscalationAction::Flag,
            "end_call" => EscalationAction::EndCall {
                message: self.message.unwrap_or_default(),
            },
            _ => return Err(corrupt("purpose.escalation.action")(())),
        };
        EscalationRule::new(&self.when, action).map_err(corrupt("purpose.escalation"))
    }
}
