//! The agent aggregate: an ID, its draft, and its lifecycle timestamps.

use jiff::Timestamp;

use crate::{AgentDraft, AgentId, DraftField, DraftPatch};

/// A voice agent the operator owns. It has one mutable draft; archiving hides it from lists
/// and freezes the draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    id: AgentId,
    draft: AgentDraft,
    created_at: Timestamp,
    updated_at: Timestamp,
    archived_at: Option<Timestamp>,
}

/// A rule an agent change breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AgentError {
    #[error("the agent is archived, so its draft can't change")]
    Archived,
}

/// A draft change: the updated agent and the fields that changed (never empty).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftChange {
    pub agent: Agent,
    pub fields: Vec<DraftField>,
}

impl Agent {
    /// A new agent, created at `now`.
    #[must_use]
    pub fn create(id: AgentId, draft: AgentDraft, now: Timestamp) -> Self {
        Self {
            id,
            draft,
            created_at: now,
            updated_at: now,
            archived_at: None,
        }
    }

    /// An agent read back from storage, exactly as it was saved.
    #[must_use]
    pub fn restore(
        id: AgentId,
        draft: AgentDraft,
        created_at: Timestamp,
        updated_at: Timestamp,
        archived_at: Option<Timestamp>,
    ) -> Self {
        Self {
            id,
            draft,
            created_at,
            updated_at,
            archived_at,
        }
    }

    /// Applies `patch` to the draft at `now`. `None` when the patch changes nothing, so there
    /// is nothing to save or audit. An archived agent can't change.
    pub fn update_draft(
        &self,
        patch: &DraftPatch,
        now: Timestamp,
    ) -> Result<Option<DraftChange>, AgentError> {
        if self.is_archived() {
            return Err(AgentError::Archived);
        }
        let draft = self.draft.apply(patch);
        let fields = self.draft.changed_fields(&draft);
        if fields.is_empty() {
            return Ok(None);
        }
        let agent = Self {
            draft,
            updated_at: self.later(now),
            ..self.clone()
        };
        Ok(Some(DraftChange { agent, fields }))
    }

    /// The agent archived at `now`, or `None` if it already is.
    #[must_use]
    pub fn archive(&self, now: Timestamp) -> Option<Self> {
        if self.is_archived() {
            return None;
        }
        let at = self.later(now);
        Some(Self {
            updated_at: at,
            archived_at: Some(at),
            ..self.clone()
        })
    }

    /// `now`, but never before the last update: a clock that steps back must not reorder
    /// history.
    fn later(&self, now: Timestamp) -> Timestamp {
        now.max(self.updated_at)
    }

    #[must_use]
    pub fn id(&self) -> AgentId {
        self.id
    }

    #[must_use]
    pub fn draft(&self) -> &AgentDraft {
        &self.draft
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    #[must_use]
    pub fn updated_at(&self) -> Timestamp {
        self.updated_at
    }

    #[must_use]
    pub fn archived_at(&self) -> Option<Timestamp> {
        self.archived_at
    }

    #[must_use]
    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use jiff::SignedDuration;
    use uuid::Uuid;

    use super::*;
    use crate::{AgentName, DraftField, UseCase, VoiceId};

    fn created_at() -> Timestamp {
        Timestamp::from_second(1_793_000_000).expect("valid")
    }

    fn agent() -> Agent {
        let id = AgentId::from_uuid(Uuid::from_u128(0x0192_0000_0000_7000_8000_0000_0000_0001))
            .expect("v7");
        let draft = AgentDraft::new(
            AgentName::parse("Front desk").expect("valid"),
            UseCase::Support,
            VoiceId::parse("af_heart").expect("valid"),
        );
        Agent::create(id, draft, created_at())
    }

    fn rename(to: &str) -> DraftPatch {
        DraftPatch {
            name: Some(AgentName::parse(to).expect("valid")),
            ..DraftPatch::default()
        }
    }

    #[test]
    fn a_new_agent_is_active_and_created_now() {
        let agent = agent();

        assert_eq!(agent.created_at(), created_at());
        assert_eq!(agent.updated_at(), created_at());
        assert_eq!(agent.archived_at(), None);
        assert!(!agent.is_archived());
    }

    #[test]
    fn a_draft_change_reports_its_fields_and_moves_updated_at() {
        let later = created_at() + SignedDuration::from_secs(60);

        let change = agent()
            .update_draft(&rename("After hours"), later)
            .expect("active")
            .expect("changed");

        assert_eq!(change.fields, [DraftField::Name]);
        assert_eq!(
            change.agent.draft().persona().name().as_str(),
            "After hours"
        );
        assert_eq!(change.agent.updated_at(), later);
        assert_eq!(change.agent.created_at(), created_at());
    }

    #[test]
    fn a_patch_that_changes_nothing_is_not_a_change() {
        let later = created_at() + SignedDuration::from_secs(60);

        assert_eq!(agent().update_draft(&rename("Front desk"), later), Ok(None));
        assert_eq!(
            agent().update_draft(&DraftPatch::default(), later),
            Ok(None)
        );
    }

    #[test]
    fn time_never_runs_backwards() {
        let earlier = created_at() - SignedDuration::from_secs(60);

        let change = agent()
            .update_draft(&rename("After hours"), earlier)
            .expect("active")
            .expect("changed");
        let archived = agent().archive(earlier).expect("active");

        assert_eq!(change.agent.updated_at(), created_at());
        assert_eq!(archived.archived_at(), Some(created_at()));
    }

    #[test]
    fn archiving_happens_once_and_freezes_the_draft() {
        let later = created_at() + SignedDuration::from_secs(60);

        let archived = agent().archive(later).expect("active");

        assert_eq!(archived.archived_at(), Some(later));
        assert_eq!(archived.updated_at(), later);
        assert_eq!(archived.archive(later + SignedDuration::from_secs(1)), None);
        assert_eq!(
            archived.update_draft(&rename("Too late"), later),
            Err(AgentError::Archived)
        );
    }

    #[test]
    fn restore_keeps_every_field() {
        let archived = agent()
            .archive(created_at() + SignedDuration::from_secs(5))
            .expect("active");

        let restored = Agent::restore(
            archived.id(),
            archived.draft().clone(),
            archived.created_at(),
            archived.updated_at(),
            archived.archived_at(),
        );

        assert_eq!(restored, archived);
    }
}
