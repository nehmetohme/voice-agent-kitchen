//! Audit events: an append-only record of who changed what, and when.

use jiff::Timestamp;

use crate::{Actor, AgentId, AuditEventId, ContentHash, DraftField};

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
}

impl AuditAction {
    /// The stored action name, e.g. `agent.created`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::AgentCreated { .. } => "agent.created",
            Self::AgentDraftUpdated { .. } => "agent.draft_updated",
            Self::AgentArchived => "agent.archived",
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
