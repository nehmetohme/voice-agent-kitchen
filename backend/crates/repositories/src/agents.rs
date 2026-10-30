//! The agent aggregate's port: the agent, its draft and its audit trail.

use vak_domain::{Agent, AgentId, AuditEvent};

use crate::{BoxFuture, RepoError, Revision, Versioned};

/// Which agents [`AgentRepo::list`] returns: active ones, most recently updated first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentQuery {
    /// At most this many agents.
    pub limit: u16,
}

/// Stores agents with their drafts. Every mutation writes its audit event in the same
/// transaction, so a change is never stored without its record, nor a record without its
/// change.
pub trait AgentRepo: Send + Sync {
    /// Stores a new agent and `event`. The acting operator is recorded from `event`.
    /// Fails with [`RepoError::Conflict`] when the ID already exists.
    fn create<'a>(
        &'a self,
        agent: &'a Agent,
        event: &'a AuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>>;

    /// The agent, archived or not, with its revision.
    fn get(&self, id: AgentId) -> BoxFuture<'_, Result<Versioned<Agent>, RepoError>>;

    /// Active (not archived) agents, most recently updated first.
    fn list(&self, query: AgentQuery) -> BoxFuture<'_, Result<Vec<Agent>, RepoError>>;

    /// Replaces the stored agent and draft, and stores `event`, if the stored revision is
    /// `expected`. Returns the new revision. Fails with [`RepoError::Conflict`] on a stale
    /// revision and [`RepoError::NotFound`] when the agent doesn't exist.
    fn update<'a>(
        &'a self,
        agent: &'a Agent,
        expected: Revision,
        event: &'a AuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>>;
}
