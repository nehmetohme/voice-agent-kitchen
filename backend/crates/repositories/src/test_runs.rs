//! The test run aggregate's port: runs with their scenario results, and their audit trail.

use vak_domain::{AgentId, AuditEvent, ContentHash, TestRun, TestRunId, Verdict};

use crate::{BoxFuture, RepoError, Revision, Versioned};

/// Stores test runs. A run is one aggregate: its status, the models it used and every
/// scenario's result (transcript and rubric checks) are written together.
///
/// An agent has at most one active (queued or running) run: [`TestRunRepo::create`] fails
/// with [`RepoError::Conflict`] while another one is active.
pub trait TestRunRepo: Send + Sync {
    /// Stores a new queued run and `event` (the operator who started it) in one transaction.
    /// Fails with [`RepoError::Conflict`] when the ID exists or the agent has an active run.
    fn create<'a>(
        &'a self,
        run: &'a TestRun,
        event: &'a AuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>>;

    /// The run with its revision.
    fn get(&self, id: TestRunId) -> BoxFuture<'_, Result<Versioned<TestRun>, RepoError>>;

    /// The agent's runs, newest first, at most `limit`.
    fn list(&self, agent: AgentId, limit: u16) -> BoxFuture<'_, Result<Vec<TestRun>, RepoError>>;

    /// Replaces the stored run, and stores `event` when an operator made the change, if the
    /// stored revision is `expected`. Returns the new revision. Fails with
    /// [`RepoError::Conflict`] on a stale revision and [`RepoError::NotFound`] when the run
    /// doesn't exist.
    fn update<'a>(
        &'a self,
        run: &'a TestRun,
        expected: Revision,
        event: Option<&'a AuditEvent>,
    ) -> BoxFuture<'a, Result<Revision, RepoError>>;

    /// The oldest queued run, if any.
    fn next_queued(&self) -> BoxFuture<'_, Result<Option<Versioned<TestRun>>, RepoError>>;

    /// Every running run, oldest first.
    fn running(&self) -> BoxFuture<'_, Result<Vec<Versioned<TestRun>>, RepoError>>;

    /// For each `(agent, draft hash)`, the verdict of the agent's latest finished run on that
    /// draft hash; pairs without a finished run are left out.
    fn latest_verdicts<'a>(
        &'a self,
        drafts: &'a [(AgentId, ContentHash)],
    ) -> BoxFuture<'a, Result<Vec<(AgentId, Verdict)>, RepoError>>;
}
