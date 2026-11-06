//! The agent's purpose: which version it pins, whether a newer one is out and what moving to
//! it changes, and the move itself. An edited plan is never overwritten unless the operator
//! asks (`replace_plan`); the rule lives in `vak_domain::PurposeUpdate`.

use vak_domain::{Actor, AgentId, PurposeError, PurposeName, PurposeRef, PurposeUpdate};
use vak_repositories::RepoError;

use crate::{AgentService, FindAgentError, UpdateDraftError};

/// The purpose version an agent pins, and the update to the latest one if there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPurpose {
    pub reference: PurposeRef,
    /// The pinned version's name.
    pub name: PurposeName,
    pub system: bool,
    pub latest_version: Option<u32>,
    /// Moving to the latest version, when it is newer: what changes, and what happens to the
    /// plan if the operator doesn't ask to replace an edited one.
    pub update: Option<PurposeUpdate>,
}

/// Move an agent to `version` of its purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateAgentPurpose {
    pub version: u32,
    /// Replace a plan the operator edited with the new starting flow.
    pub replace_plan: bool,
}

/// Why [`AgentService::update_purpose`] failed.
#[derive(Debug, thiserror::Error)]
pub enum UpdateAgentPurposeError {
    #[error("no such agent")]
    NotFound,
    #[error("the agent is archived")]
    Archived,
    #[error("the purpose has no such version")]
    VersionNotFound,
    #[error("the agent already pins that version")]
    SameVersion,
    #[error("the agent kept changing while it was being saved")]
    Conflict,
    #[error("storage is unavailable")]
    Unavailable(#[source] RepoError),
}

impl AgentService {
    /// The agent's pinned purpose version, and the update to its purpose's latest version
    /// when that one is newer.
    pub async fn purpose(&self, id: AgentId) -> Result<AgentPurpose, FindAgentError> {
        let agent = self.get(id).await?;
        let draft = agent.draft();
        let pinned = self
            .pinned_purpose(draft)
            .await
            .map_err(FindAgentError::Unavailable)?;
        let purpose = self
            .load_purpose(pinned.reference().id())
            .await
            .map_err(FindAgentError::Unavailable)?
            .value;
        let update = purpose
            .latest()
            .filter(|latest| latest.reference().version() > pinned.reference().version())
            // An update the domain can't build (the latest version isn't newer, or isn't this
            // purpose's) is no update to offer.
            .and_then(|latest| {
                PurposeUpdate::new(draft, &pinned, latest, &self.rule_packs, false).ok()
            });
        Ok(AgentPurpose {
            reference: pinned.reference(),
            name: pinned.definition().name().clone(),
            system: purpose.is_system(),
            latest_version: purpose.latest_version(),
            update,
        })
    }

    /// Moves the agent to another version of its purpose: one `agent.draft_updated` audit
    /// event for `purpose` (and `use_case`, `rule_pack` and `plan` when they change).
    pub async fn update_purpose(
        &self,
        actor: &Actor,
        id: AgentId,
        cmd: UpdateAgentPurpose,
    ) -> Result<AgentPurpose, UpdateAgentPurposeError> {
        let agent = self.get(id).await.map_err(|error| match error {
            FindAgentError::NotFound => UpdateAgentPurposeError::NotFound,
            FindAgentError::Unavailable(source) => UpdateAgentPurposeError::Unavailable(source),
        })?;
        if agent.is_archived() {
            return Err(UpdateAgentPurposeError::Archived);
        }
        let pinned = self
            .pinned_purpose(agent.draft())
            .await
            .map_err(UpdateAgentPurposeError::Unavailable)?;
        let reference = PurposeRef::new(pinned.reference().id(), cmd.version)
            .map_err(|_| UpdateAgentPurposeError::VersionNotFound)?;
        let next = self
            .purpose_version(reference)
            .await
            .map_err(|error| match error {
                RepoError::NotFound => UpdateAgentPurposeError::VersionNotFound,
                other => UpdateAgentPurposeError::Unavailable(other),
            })?;
        let update = PurposeUpdate::new(
            agent.draft(),
            &pinned,
            &next,
            &self.rule_packs,
            cmd.replace_plan,
        )
        .map_err(|error| match error {
            PurposeError::SameVersion => UpdateAgentPurposeError::SameVersion,
            _ => UpdateAgentPurposeError::VersionNotFound,
        })?;
        self.update_draft(actor, id, update.patch())
            .await
            .map_err(|error| match error {
                UpdateDraftError::NotFound => UpdateAgentPurposeError::NotFound,
                UpdateDraftError::Archived => UpdateAgentPurposeError::Archived,
                UpdateDraftError::Conflict => UpdateAgentPurposeError::Conflict,
                UpdateDraftError::Unavailable(source) => {
                    UpdateAgentPurposeError::Unavailable(source)
                }
                UpdateDraftError::PurposeNotFound | UpdateDraftError::PurposeHasNoVersion => {
                    UpdateAgentPurposeError::VersionNotFound
                }
            })?;
        self.purpose(id).await.map_err(|error| match error {
            FindAgentError::NotFound => UpdateAgentPurposeError::NotFound,
            FindAgentError::Unavailable(source) => UpdateAgentPurposeError::Unavailable(source),
        })
    }
}
