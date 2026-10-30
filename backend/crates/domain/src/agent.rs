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
        // CONTRACT-STUB
        let _ = (patch, now);
        Ok(None)
    }

    /// The agent archived at `now`, or `None` if it already is.
    #[must_use]
    pub fn archive(&self, now: Timestamp) -> Option<Self> {
        // CONTRACT-STUB
        let _ = now;
        None
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
