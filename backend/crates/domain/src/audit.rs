//! Audit events: an append-only record of who changed what, and when.

use jiff::Timestamp;

use crate::{Actor, AgentId, AuditEventId, ContentHash, DraftField, PurposeId, TestRunId};

/// What happened to an agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditAction {
    /// The agent was created with this draft.
    AgentCreated { draft_hash: ContentHash },
    /// The draft changed; `fields` lists what changed.
    AgentDraftUpdated {
        fields: Vec<DraftField>,
        draft_hash: ContentHash,
    },
    /// The agent was archived.
    AgentArchived,
    /// An operator started a test run of this draft.
    TestRunStarted {
        run: TestRunId,
        draft_hash: ContentHash,
    },
    /// An operator cancelled a test run.
    TestRunCancelled { run: TestRunId },
}

impl AuditAction {
    /// The stored action name, e.g. `agent.created`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::AgentCreated { .. } => "agent.created",
            Self::AgentDraftUpdated { .. } => "agent.draft_updated",
            Self::AgentArchived => "agent.archived",
            Self::TestRunStarted { .. } => "test_run.started",
            Self::TestRunCancelled { .. } => "test_run.cancelled",
        }
    }
}

/// One audit record. Written in the same transaction as the change it records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    id: AuditEventId,
    at: Timestamp,
    actor: Actor,
    agent_id: AgentId,
    action: AuditAction,
}

impl AuditEvent {
    #[must_use]
    pub fn new(
        id: AuditEventId,
        at: Timestamp,
        actor: Actor,
        agent_id: AgentId,
        action: AuditAction,
    ) -> Self {
        Self {
            id,
            at,
            actor,
            agent_id,
            action,
        }
    }

    #[must_use]
    pub fn id(&self) -> AuditEventId {
        self.id
    }

    #[must_use]
    pub fn at(&self) -> Timestamp {
        self.at
    }

    #[must_use]
    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    /// The agent the event is about.
    #[must_use]
    pub fn agent_id(&self) -> AgentId {
        self.agent_id
    }

    #[must_use]
    pub fn action(&self) -> &AuditAction {
        &self.action
    }
}

/// What happened to a purpose of the workspace's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PurposeAuditAction {
    /// The purpose was made as a copy of `source`: of `version`, or of its draft.
    Created {
        source: PurposeId,
        version: Option<u32>,
    },
    /// The draft changed.
    DraftUpdated { draft_hash: ContentHash },
    /// The draft was saved as `version`.
    VersionSaved {
        version: u32,
        content_hash: ContentHash,
    },
}

impl PurposeAuditAction {
    /// The stored action name, e.g. `purpose.version_saved`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Created { .. } => "purpose.created",
            Self::DraftUpdated { .. } => "purpose.draft_updated",
            Self::VersionSaved { .. } => "purpose.version_saved",
        }
    }
}

/// One audit record about a purpose. Written in the same transaction as the change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurposeAuditEvent {
    id: AuditEventId,
    at: Timestamp,
    actor: Actor,
    purpose_id: PurposeId,
    action: PurposeAuditAction,
}

impl PurposeAuditEvent {
    #[must_use]
    pub fn new(
        id: AuditEventId,
        at: Timestamp,
        actor: Actor,
        purpose_id: PurposeId,
        action: PurposeAuditAction,
    ) -> Self {
        Self {
            id,
            at,
            actor,
            purpose_id,
            action,
        }
    }

    #[must_use]
    pub fn id(&self) -> AuditEventId {
        self.id
    }

    #[must_use]
    pub fn at(&self) -> Timestamp {
        self.at
    }

    #[must_use]
    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    /// The purpose the event is about.
    #[must_use]
    pub fn purpose_id(&self) -> PurposeId {
        self.purpose_id
    }

    #[must_use]
    pub fn action(&self) -> &PurposeAuditAction {
        &self.action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_names_are_namespaced_by_entity() {
        let hash = ContentHash::of(b"draft");
        let names = [
            AuditAction::AgentCreated {
                draft_hash: hash.clone(),
            },
            AuditAction::AgentDraftUpdated {
                fields: vec![DraftField::Name],
                draft_hash: hash,
            },
            AuditAction::AgentArchived,
        ]
        .map(|action| action.name());

        assert_eq!(
            names,
            ["agent.created", "agent.draft_updated", "agent.archived"]
        );
    }
}
