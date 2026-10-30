//! JSONB shapes and row mapping. Every value read back goes through the domain's
//! constructors, so a row the domain would reject never becomes an `Agent`.

use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;
use vak_domain::{
    Agent, AgentDraft, AgentId, AgentName, AgentSettings, AuditAction, ContentHash, Jurisdiction,
    Language, ModelProfile, Persona, RulePackRef, TurnDetection, UseCase, VoiceId,
};
use vak_repositories::{RepoError, Revision, Versioned};

/// `agent_drafts.persona`. Name and use case live in `agents`, where lists read them.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PersonaRecord {
    voice: String,
    language: String,
}

/// `agent_drafts.settings`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SettingsRecord {
    model: String,
    turn_detection: TurnRecord,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TurnRecord {
    silence_ms: u32,
    barge_in: bool,
}

/// `audit_events.details`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuditDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    draft_hash: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fields: Vec<&'static str>,
}

impl PersonaRecord {
    pub(crate) fn of(draft: &AgentDraft) -> Json<Self> {
        Json(Self {
            voice: draft.persona().voice().as_str().to_owned(),
            language: draft.persona().language().as_str().to_owned(),
        })
    }
}

impl SettingsRecord {
    pub(crate) fn of(draft: &AgentDraft) -> Json<Self> {
        let settings = draft.settings();
        Json(Self {
            model: settings.model().as_str().to_owned(),
            turn_detection: TurnRecord {
                silence_ms: settings.turn_detection().silence_ms(),
                barge_in: settings.turn_detection().barge_in(),
            },
        })
    }
}

impl AuditDetails {
    pub(crate) fn of(action: &AuditAction) -> Json<Self> {
        Json(match action {
            AuditAction::AgentCreated { draft_hash } => Self {
                draft_hash: Some(draft_hash.as_str().to_owned()),
                fields: Vec::new(),
            },
            AuditAction::AgentDraftUpdated { fields, draft_hash } => Self {
                draft_hash: Some(draft_hash.as_str().to_owned()),
                fields: fields.iter().map(|field| field.as_str()).collect(),
            },
            AuditAction::AgentArchived => Self {
                draft_hash: None,
                fields: Vec::new(),
            },
        })
    }
}

/// A stored row that the domain rejects: the database was changed outside this crate.
#[derive(Debug, thiserror::Error)]
#[error("stored agent row is invalid: {0}")]
pub(crate) struct CorruptRow(&'static str);

fn corrupt<E>(what: &'static str) -> impl FnOnce(E) -> RepoError {
    move |_| RepoError::unavailable(CorruptRow(what))
}

/// One `agents` row joined with its `agent_drafts` row.
#[derive(Debug)]
pub(crate) struct AgentRow {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) use_case: String,
    pub(crate) jurisdiction: String,
    pub(crate) revision: i64,
    pub(crate) created_at: jiff_sqlx::Timestamp,
    pub(crate) updated_at: jiff_sqlx::Timestamp,
    pub(crate) archived_at: Option<jiff_sqlx::Timestamp>,
    pub(crate) persona: Json<PersonaRecord>,
    pub(crate) settings: Json<SettingsRecord>,
    pub(crate) rule_pack_id: String,
    pub(crate) rule_pack_version: i32,
    pub(crate) content_hash: String,
}

impl TryFrom<AgentRow> for Versioned<Agent> {
    type Error = RepoError;

    fn try_from(row: AgentRow) -> Result<Self, RepoError> {
        let Json(persona) = row.persona;
        let Json(settings) = row.settings;
        let persona = Persona::new(
            AgentName::parse(&row.name).map_err(corrupt("name"))?,
            UseCase::parse(&row.use_case).map_err(corrupt("use_case"))?,
            VoiceId::parse(&persona.voice).map_err(corrupt("persona.voice"))?,
            Language::parse(&persona.language).map_err(corrupt("persona.language"))?,
        );
        let settings = AgentSettings::new(
            ModelProfile::parse(&settings.model).map_err(corrupt("settings.model"))?,
            TurnDetection::new(
                settings.turn_detection.silence_ms,
                settings.turn_detection.barge_in,
            )
            .map_err(corrupt("settings.turnDetection"))?,
        );
        let rule_pack = RulePackRef::new(
            &row.rule_pack_id,
            u32::try_from(row.rule_pack_version).map_err(corrupt("rule_pack_version"))?,
        )
        .map_err(corrupt("rule_pack"))?;
        let draft = AgentDraft::restore(
            persona,
            Jurisdiction::parse(&row.jurisdiction).map_err(corrupt("jurisdiction"))?,
            rule_pack,
            settings,
        );
        let stored_hash = ContentHash::parse(&row.content_hash).map_err(corrupt("content_hash"))?;
        if draft.content_hash() != stored_hash {
            return Err(corrupt("content_hash")(()));
        }
        let agent = Agent::restore(
            AgentId::from_uuid(row.id).map_err(corrupt("id"))?,
            draft,
            row.created_at.to_jiff(),
            row.updated_at.to_jiff(),
            row.archived_at.map(jiff_sqlx::Timestamp::to_jiff),
        );
        let revision = u64::try_from(row.revision).map_err(corrupt("revision"))?;
        Ok(Self {
            value: agent,
            revision: Revision::new(revision),
        })
    }
}
