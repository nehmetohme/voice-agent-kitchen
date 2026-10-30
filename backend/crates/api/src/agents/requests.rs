//! Request bodies of `POST` and `PATCH /api/v1/agents`, and their validation. Every field is
//! parsed by its domain type; every invalid field is reported at once, by name.

use serde::Deserialize;
use utoipa::ToSchema;
use vak_domain::{
    AgentName, DraftPatch, Jurisdiction, Language, ModelProfile, TurnDetection, UseCase, VoiceId,
};
use vak_services::CreateAgent;

use super::dto::{ModelProfileDto, TurnDetectionDto, UseCaseDto, name_message};
use crate::problem::{ApiError, FieldError};

/// Create an agent. Only `name`, `useCase` and `voice` are required; every other field has
/// a default the server fills in.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateAgentRequest {
    /// Display name, 1 to 80 characters.
    #[schema(value_type = String, required = true, min_length = 1, max_length = 80, example = "Front desk")]
    name: Option<String>,
    /// What the agent is for (any case is accepted).
    #[schema(value_type = UseCaseDto, required = true)]
    use_case: Option<String>,
    /// A voice ID from `GET /api/v1/catalog/voices`.
    #[schema(value_type = String, required = true, example = "af_heart")]
    voice: Option<String>,
    /// A language code from `GET /api/v1/catalog/languages`.
    #[schema(value_type = String, required = false, default = "en-US")]
    language: Option<String>,
    /// Region code, e.g. `US`, `US-CA` or `UK`. Picks the rule-pack variant.
    #[schema(value_type = String, required = false, default = "US", example = "US")]
    jurisdiction: Option<String>,
    #[schema(value_type = ModelProfileDto, required = false, default = "balanced")]
    model: Option<String>,
    #[schema(value_type = TurnDetectionDto, required = false, default = json!({ "silenceMs": 500, "bargeIn": true }))]
    turn_detection: Option<TurnDetectionDto>,
}

/// Change the agent's draft (autosave). Send only the fields that changed.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateAgentRequest {
    #[schema(value_type = String, required = false, min_length = 1, max_length = 80)]
    name: Option<String>,
    #[schema(value_type = UseCaseDto, required = false)]
    use_case: Option<String>,
    #[schema(value_type = String, required = false)]
    voice: Option<String>,
    #[schema(value_type = String, required = false)]
    language: Option<String>,
    #[schema(value_type = String, required = false)]
    jurisdiction: Option<String>,
    #[schema(value_type = ModelProfileDto, required = false)]
    model: Option<String>,
    #[schema(value_type = TurnDetectionDto, required = false)]
    turn_detection: Option<TurnDetectionDto>,
}

/// Collects field errors while parsing.
#[derive(Default)]
struct Fields {
    errors: Vec<FieldError>,
}

impl Fields {
    /// Parses `raw` if present; records `field` as invalid when it doesn't parse.
    fn optional<R, T>(
        &mut self,
        field: &'static str,
        raw: Option<R>,
        parse: impl FnOnce(R) -> Result<T, &'static str>,
    ) -> Option<T> {
        match raw.map(parse)? {
            Ok(value) => Some(value),
            Err(message) => {
                self.errors.push(FieldError { field, message });
                None
            }
        }
    }

    /// Like [`Self::optional`], and records a missing value too.
    fn required<R, T>(
        &mut self,
        field: &'static str,
        raw: Option<R>,
        parse: impl FnOnce(R) -> Result<T, &'static str>,
    ) -> Option<T> {
        if raw.is_none() {
            self.errors.push(FieldError {
                field,
                message: "This field is required.",
            });
        }
        self.optional(field, raw, parse)
    }

    fn into_error(self) -> ApiError {
        ApiError::Invalid(self.errors)
    }
}

fn name(raw: &str) -> Result<AgentName, &'static str> {
    AgentName::parse(raw).map_err(name_message)
}

fn use_case(raw: &str) -> Result<UseCase, &'static str> {
    UseCase::parse(raw)
        .map_err(|_| "Choose support, service, booking, survey, telemarketing or custom.")
}

fn voice(raw: &str) -> Result<VoiceId, &'static str> {
    VoiceId::parse(raw).map_err(|_| "Choose a voice from the catalog.")
}

fn language(raw: &str) -> Result<Language, &'static str> {
    Language::parse(raw).map_err(|_| "Choose a language from the catalog.")
}

fn jurisdiction(raw: &str) -> Result<Jurisdiction, &'static str> {
    Jurisdiction::parse(raw).map_err(|_| "Use a region code such as US, US-CA or UK.")
}

fn model(raw: &str) -> Result<ModelProfile, &'static str> {
    ModelProfile::parse(raw).map_err(|_| "Choose balanced or fast.")
}

fn turn_detection(dto: TurnDetectionDto) -> Result<TurnDetection, &'static str> {
    TurnDetection::new(dto.silence_ms, dto.barge_in)
        .map_err(|_| "Use a silence between 200 and 2000 ms.")
}

impl TryFrom<CreateAgentRequest> for CreateAgent {
    type Error = ApiError;

    fn try_from(body: CreateAgentRequest) -> Result<Self, ApiError> {
        let mut fields = Fields::default();
        let name = fields.required("name", body.name.as_deref(), name);
        let use_case = fields.required("useCase", body.use_case.as_deref(), use_case);
        let voice = fields.required("voice", body.voice.as_deref(), voice);
        let language = fields.optional("language", body.language.as_deref(), language);
        let jurisdiction =
            fields.optional("jurisdiction", body.jurisdiction.as_deref(), jurisdiction);
        let model = fields.optional("model", body.model.as_deref(), model);
        let turn_detection = fields.optional("turnDetection", body.turn_detection, turn_detection);
        match (name, use_case, voice) {
            (Some(name), Some(use_case), Some(voice)) if fields.errors.is_empty() => Ok(Self {
                name,
                use_case,
                voice,
                language,
                jurisdiction,
                model,
                turn_detection,
            }),
            _ => Err(fields.into_error()),
        }
    }
}

impl TryFrom<UpdateAgentRequest> for DraftPatch {
    type Error = ApiError;

    fn try_from(body: UpdateAgentRequest) -> Result<Self, ApiError> {
        let mut fields = Fields::default();
        let patch = Self {
            name: fields.optional("name", body.name.as_deref(), name),
            use_case: fields.optional("useCase", body.use_case.as_deref(), use_case),
            voice: fields.optional("voice", body.voice.as_deref(), voice),
            language: fields.optional("language", body.language.as_deref(), language),
            jurisdiction: fields.optional(
                "jurisdiction",
                body.jurisdiction.as_deref(),
                jurisdiction,
            ),
            model: fields.optional("model", body.model.as_deref(), model),
            turn_detection: fields.optional("turnDetection", body.turn_detection, turn_detection),
        };
        if fields.errors.is_empty() {
            Ok(patch)
        } else {
            Err(fields.into_error())
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn fields_of(error: ApiError) -> Vec<(&'static str, &'static str)> {
        match error {
            ApiError::Invalid(errors) => errors
                .into_iter()
                .map(|error| (error.field, error.message))
                .collect(),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn create_reports_every_invalid_optional_field() {
        let body: CreateAgentRequest = serde_json::from_value(json!({
            "name": "Front\u{7}desk", "useCase": "support", "voice": "af_heart",
            "language": "klingon", "jurisdiction": "Mars", "model": "huge",
            "turnDetection": { "silenceMs": 10, "bargeIn": true },
        }))
        .expect("parses");

        let error = CreateAgent::try_from(body).expect_err("invalid");

        assert_eq!(
            fields_of(error),
            [
                ("name", "Remove line breaks and control characters."),
                ("language", "Choose a language from the catalog."),
                ("jurisdiction", "Use a region code such as US, US-CA or UK."),
                ("model", "Choose balanced or fast."),
                ("turnDetection", "Use a silence between 200 and 2000 ms."),
            ]
        );
    }

    #[test]
    fn create_names_each_missing_required_field() {
        let body: CreateAgentRequest = serde_json::from_value(json!({})).expect("parses");

        let error = CreateAgent::try_from(body).expect_err("invalid");

        assert_eq!(
            fields_of(error)
                .into_iter()
                .map(|(field, _)| field)
                .collect::<Vec<_>>(),
            ["name", "useCase", "voice"]
        );
    }

    #[test]
    fn update_accepts_any_subset_and_rejects_bad_values() {
        let body: UpdateAgentRequest = serde_json::from_value(json!({
            "useCase": " SURVEY ", "turnDetection": { "silenceMs": 700, "bargeIn": false },
        }))
        .expect("parses");
        let patch = DraftPatch::try_from(body).expect("valid");
        assert_eq!(patch.use_case, Some(UseCase::Survey));
        assert_eq!(
            patch.turn_detection.map(|turn| turn.silence_ms()),
            Some(700)
        );
        assert_eq!(patch.name, None);

        let body: UpdateAgentRequest =
            serde_json::from_value(json!({ "name": "", "voice": "x" })).expect("parses");
        let error = DraftPatch::try_from(body).expect_err("invalid");
        assert_eq!(
            fields_of(error),
            [
                ("name", "Enter a name."),
                ("voice", "Choose a voice from the catalog.")
            ]
        );
    }
}
