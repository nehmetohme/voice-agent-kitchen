//! `/api/v1/catalog`: the voices and languages operators choose from.

use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;
use vak_domain::catalog::{LanguageInfo, VoiceInfo};
use vak_services::catalog;

use crate::problem::Problem;

/// A voice the agent can speak with.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Voice)]
pub(crate) struct VoiceDto {
    /// The voice ID to send as `voice`.
    #[schema(example = "af_heart")]
    id: &'static str,
    /// Friendly name.
    #[schema(example = "Heart")]
    name: &'static str,
    /// Accent and gender.
    #[schema(example = "American English, female")]
    description: &'static str,
    /// The language the voice speaks.
    #[schema(example = "en-US")]
    language: &'static str,
}

impl From<&VoiceInfo> for VoiceDto {
    fn from(voice: &VoiceInfo) -> Self {
        Self {
            id: voice.id,
            name: voice.name,
            description: voice.description,
            language: voice.language,
        }
    }
}

/// Every voice, recommended first.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = VoiceList)]
pub(crate) struct VoiceListDto {
    items: Vec<VoiceDto>,
}

/// A language the agent can speak and understand.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Language)]
pub(crate) struct LanguageDto {
    /// The language code to send as `language`.
    #[schema(example = "en-US")]
    code: &'static str,
    /// Friendly name.
    #[schema(example = "English (United States)")]
    name: &'static str,
}

impl From<&LanguageInfo> for LanguageDto {
    fn from(language: &LanguageInfo) -> Self {
        Self {
            code: language.code,
            name: language.name,
        }
    }
}

/// Every language, the default first.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = LanguageList)]
pub(crate) struct LanguageListDto {
    items: Vec<LanguageDto>,
}

/// List the voices an agent can use.
#[utoipa::path(
    get,
    path = "/catalog/voices",
    operation_id = "listVoices",
    tag = "catalog",
    responses(
        (status = OK, description = "Every voice, recommended first", body = VoiceListDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn voices() -> Json<VoiceListDto> {
    Json(VoiceListDto {
        items: catalog::voices().iter().map(VoiceDto::from).collect(),
    })
}

/// List the languages an agent can speak.
#[utoipa::path(
    get,
    path = "/catalog/languages",
    operation_id = "listLanguages",
    tag = "catalog",
    responses(
        (status = OK, description = "Every language, the default first", body = LanguageListDto),
        (status = UNAUTHORIZED, description = "No signed-in operator", body = Problem, content_type = "application/problem+json"),
    ),
    security(("databricksApps" = []))
)]
pub(crate) async fn languages() -> Json<LanguageListDto> {
    Json(LanguageListDto {
        items: catalog::languages().iter().map(LanguageDto::from).collect(),
    })
}
