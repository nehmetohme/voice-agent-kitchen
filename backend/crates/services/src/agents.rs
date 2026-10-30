//! Agent use cases: create, list, get, update the draft (autosave), archive, progress.
//!
//! Every mutation hands its audit event to the same `AgentRepo` call that stores the change,
//! so both land in one transaction.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use jiff::{Timestamp, Unit};
use uuid::Uuid;
use vak_domain::{
    Actor, Agent, AgentDraft, AgentId, AgentName, AuditAction, AuditEvent, AuditEventId,
    DraftPatch, Jurisdiction, Language, ModelProfile, ProgressFacts, SetupProgress, TurnDetection,
    UseCase, VoiceId,
};
use vak_repositories::{AgentQuery, AgentRepo, RepoError, Revision};

/// The most agents [`AgentService::list`] returns.
pub const LIST_LIMIT: u16 = 200;
/// How long one repository call may take before the use case gives up.
const DEFAULT_DEADLINE: Duration = Duration::from_secs(5);
/// How often an update is retried when another writer saved first.
const UPDATE_ATTEMPTS: u32 = 3;

/// Create an agent: the three required fields, plus any optional field to override its
/// default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAgent {
    pub name: AgentName,
    pub use_case: UseCase,
    pub voice: VoiceId,
    pub language: Option<Language>,
    pub jurisdiction: Option<Jurisdiction>,
    pub model: Option<ModelProfile>,
    pub turn_detection: Option<TurnDetection>,
}

/// An agent with its setup progress, as lists show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSummary {
    pub agent: Agent,
    pub progress: SetupProgress,
}

/// Why [`AgentService::create`] failed.
#[derive(Debug, thiserror::Error)]
pub enum CreateAgentError {
    #[error("agent storage is unavailable")]
    Unavailable(#[source] RepoError),
}

/// Why [`AgentService::list`] failed.
#[derive(Debug, thiserror::Error)]
pub enum ListAgentsError {
    #[error("agent storage is unavailable")]
    Unavailable(#[source] RepoError),
}

/// Why a use case on one agent (get, progress, archive) failed.
#[derive(Debug, thiserror::Error)]
pub enum FindAgentError {
    #[error("no such agent")]
    NotFound,
    #[error("agent storage is unavailable")]
    Unavailable(#[source] RepoError),
}

/// Why [`AgentService::update_draft`] failed.
#[derive(Debug, thiserror::Error)]
pub enum UpdateDraftError {
    #[error("no such agent")]
    NotFound,
    #[error("the agent is archived")]
    Archived,
    #[error("the agent kept changing while it was being saved")]
    Conflict,
    #[error("agent storage is unavailable")]
    Unavailable(#[source] RepoError),
}

impl From<RepoError> for FindAgentError {
    fn from(error: RepoError) -> Self {
        match error {
            RepoError::NotFound => Self::NotFound,
            other => Self::Unavailable(other),
        }
    }
}

impl From<FindAgentError> for UpdateDraftError {
    fn from(error: FindAgentError) -> Self {
        match error {
            FindAgentError::NotFound => Self::NotFound,
            FindAgentError::Unavailable(source) => Self::Unavailable(source),
        }
    }
}

/// The agent use cases, over an [`AgentRepo`].
#[derive(Clone)]
pub struct AgentService {
    repo: Arc<dyn AgentRepo>,
    deadline: Duration,
}

impl AgentService {
    #[must_use]
    pub fn new(repo: Arc<dyn AgentRepo>) -> Self {
        Self {
            repo,
            deadline: DEFAULT_DEADLINE,
        }
    }

    /// The same service with another per-call deadline.
    #[must_use]
    pub fn with_deadline(self, deadline: Duration) -> Self {
        Self { deadline, ..self }
    }

    /// Creates an agent; every field the command leaves out takes its default.
    pub async fn create(&self, actor: &Actor, cmd: CreateAgent) -> Result<Agent, CreateAgentError> {
        let defaults = DraftPatch {
            language: cmd.language,
            jurisdiction: cmd.jurisdiction,
            model: cmd.model,
            turn_detection: cmd.turn_detection,
            ..DraftPatch::default()
        };
        let draft = AgentDraft::new(cmd.name, cmd.use_case, cmd.voice).apply(&defaults);
        let id = AgentId::from_uuid(Uuid::now_v7())
            .map_err(|error| CreateAgentError::Unavailable(RepoError::unavailable(error)))?;
        let at = now();
        let agent = Agent::create(id, draft, at);
        let event = audit_event(
            actor,
            &agent,
            AuditAction::AgentCreated {
                draft_hash: agent.draft().content_hash(),
            },
        )
        .map_err(CreateAgentError::Unavailable)?;
        self.within(self.repo.create(&agent, &event))
            .await
            .map_err(CreateAgentError::Unavailable)?;
        Ok(agent)
    }

    /// Active agents with their progress, most recently updated first; at most
    /// [`LIST_LIMIT`].
    pub async fn list(&self) -> Result<Vec<AgentSummary>, ListAgentsError> {
        let agents = self
            .within(self.repo.list(AgentQuery { limit: LIST_LIMIT }))
            .await
            .map_err(ListAgentsError::Unavailable)?;
        Ok(agents
            .into_iter()
            .map(|agent| AgentSummary {
                progress: progress_of(&agent),
                agent,
            })
            .collect())
    }

    /// One agent, archived or not.
    pub async fn get(&self, id: AgentId) -> Result<Agent, FindAgentError> {
        Ok(self.within(self.repo.get(id)).await?.value)
    }

    /// The agent's setup progress, derived from its current state.
    pub async fn progress(&self, id: AgentId) -> Result<SetupProgress, FindAgentError> {
        Ok(progress_of(&self.get(id).await?))
    }

    /// Applies `patch` to the draft (autosave). A patch that changes nothing stores nothing
    /// and writes no audit event. Retries when another writer saved in between.
    pub async fn update_draft(
        &self,
        actor: &Actor,
        id: AgentId,
        patch: &DraftPatch,
    ) -> Result<Agent, UpdateDraftError> {
        for _ in 0..UPDATE_ATTEMPTS {
            let stored = self
                .within(self.repo.get(id))
                .await
                .map_err(FindAgentError::from)?;
            let change = stored
                .value
                .update_draft(patch, now())
                .map_err(|_archived| UpdateDraftError::Archived)?;
            let Some(change) = change else {
                return Ok(stored.value);
            };
            let event = audit_event(
                actor,
                &change.agent,
                AuditAction::AgentDraftUpdated {
                    fields: change.fields,
                    draft_hash: change.agent.draft().content_hash(),
                },
            )
            .map_err(UpdateDraftError::Unavailable)?;
            match self.save(&change.agent, stored.revision, &event).await {
                Err(RepoError::Conflict) => {}
                Err(error) => return Err(FindAgentError::from(error).into()),
                Ok(()) => return Ok(change.agent),
            }
        }
        Err(UpdateDraftError::Conflict)
    }

    /// Archives the agent: lists hide it and its draft can no longer change. Archiving an
    /// archived agent changes nothing.
    pub async fn archive(&self, actor: &Actor, id: AgentId) -> Result<(), FindAgentError> {
        for _ in 0..UPDATE_ATTEMPTS {
            let stored = self.within(self.repo.get(id)).await?;
            let Some(archived) = stored.value.archive(now()) else {
                return Ok(());
            };
            let event = audit_event(actor, &archived, AuditAction::AgentArchived)?;
            match self.save(&archived, stored.revision, &event).await {
                Err(RepoError::Conflict) => {}
                result => return result.map_err(FindAgentError::from),
            }
        }
        Err(FindAgentError::Unavailable(RepoError::Conflict))
    }

    async fn save(
        &self,
        agent: &Agent,
        expected: Revision,
        event: &AuditEvent,
    ) -> Result<(), RepoError> {
        self.within(self.repo.update(agent, expected, event))
            .await
            .map(|_revision| ())
    }

    /// Runs a repository call under the deadline.
    async fn within<T>(
        &self,
        call: impl Future<Output = Result<T, RepoError>>,
    ) -> Result<T, RepoError> {
        tokio::time::timeout(self.deadline, call)
            .await
            .unwrap_or_else(|elapsed| Err(RepoError::unavailable(elapsed)))
    }
}

/// Progress from the agent alone: plans, test runs and versions arrive in later milestones.
fn progress_of(agent: &Agent) -> SetupProgress {
    SetupProgress::derive(agent.draft(), &ProgressFacts::default())
}

fn audit_event(actor: &Actor, agent: &Agent, action: AuditAction) -> Result<AuditEvent, RepoError> {
    let id = AuditEventId::from_uuid(Uuid::now_v7()).map_err(RepoError::unavailable)?;
    Ok(AuditEvent::new(
        id,
        agent.updated_at(),
        actor.clone(),
        agent.id(),
        action,
    ))
}

/// The current time at Postgres precision (microseconds), so a value reads back exactly as
/// it was written.
fn now() -> Timestamp {
    let now = Timestamp::now();
    now.round(Unit::Microsecond).unwrap_or(now)
}

#[cfg(test)]
mod tests;
