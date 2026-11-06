//! In-memory `AgentRepo`: deterministic, with switches to make calls fail.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, MutexGuard, PoisonError};

use vak_domain::{Agent, AgentId, AuditEvent};
use vak_repositories::{
    AgentQuery, AgentRepo, BoxFuture, PurposeUsage, RepoError, Revision, Versioned,
};

#[derive(Debug, Default)]
struct State {
    agents: BTreeMap<AgentId, (Agent, Revision)>,
    audit: Vec<AuditEvent>,
    unavailable: bool,
    stalled: bool,
    conflicts: u32,
}

/// An [`AgentRepo`] in memory. Clone the `Arc` you hand to a service to inspect it later.
#[derive(Debug, Default)]
pub struct InMemoryAgentRepo {
    state: Mutex<State>,
}

impl InMemoryAgentRepo {
    /// Every audit event written so far, oldest first.
    #[must_use]
    pub fn audit_events(&self) -> Vec<AuditEvent> {
        self.lock().audit.clone()
    }

    /// While `true`, every call fails with [`RepoError::Unavailable`].
    pub fn set_unavailable(&self, unavailable: bool) {
        self.lock().unavailable = unavailable;
    }

    /// While `true`, every call waits forever, like a database that stopped answering. Pair it
    /// with `#[tokio::test(start_paused = true)]` to reach a deadline without real waiting.
    pub fn set_stalled(&self, stalled: bool) {
        self.lock().stalled = stalled;
    }

    /// The next `count` updates fail with [`RepoError::Conflict`], as if another writer got
    /// there first.
    pub fn fail_next_updates_with_conflict(&self, count: u32) {
        self.lock().conflicts = count;
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A panicking test may poison the lock; the state is still usable for inspection.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    async fn wait_unless_stalled(&self) {
        let stalled = self.lock().stalled;
        if stalled {
            std::future::pending::<()>().await;
        }
    }

    fn check_available(state: &State) -> Result<(), RepoError> {
        if state.unavailable {
            Err(RepoError::unavailable("in-memory repo switched off"))
        } else {
            Ok(())
        }
    }
}

impl AgentRepo for InMemoryAgentRepo {
    fn create<'a>(
        &'a self,
        agent: &'a Agent,
        event: &'a AuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(async move {
            self.wait_unless_stalled().await;
            let mut state = self.lock();
            Self::check_available(&state)?;
            if state.agents.contains_key(&agent.id()) {
                return Err(RepoError::Conflict);
            }
            state
                .agents
                .insert(agent.id(), (agent.clone(), Revision::FIRST));
            state.audit.push(event.clone());
            Ok(Revision::FIRST)
        })
    }

    fn get(&self, id: AgentId) -> BoxFuture<'_, Result<Versioned<Agent>, RepoError>> {
        Box::pin(async move {
            self.wait_unless_stalled().await;
            let state = self.lock();
            Self::check_available(&state)?;
            state
                .agents
                .get(&id)
                .map(|(agent, revision)| Versioned {
                    value: agent.clone(),
                    revision: *revision,
                })
                .ok_or(RepoError::NotFound)
        })
    }

    fn list(&self, query: AgentQuery) -> BoxFuture<'_, Result<Vec<Agent>, RepoError>> {
        Box::pin(async move {
            self.wait_unless_stalled().await;
            let state = self.lock();
            Self::check_available(&state)?;
            let mut agents: Vec<Agent> = state
                .agents
                .values()
                .map(|(agent, _)| agent)
                .filter(|agent| !agent.is_archived())
                .cloned()
                .collect();
            // Most recently updated first; the ID (UUID v7, time-ordered) breaks ties.
            agents.sort_by_key(|agent| Reverse((agent.updated_at(), agent.id())));
            agents.truncate(usize::from(query.limit));
            Ok(agents)
        })
    }

    fn purpose_usage(&self) -> BoxFuture<'_, Result<Vec<PurposeUsage>, RepoError>> {
        Box::pin(async move {
            self.wait_unless_stalled().await;
            let state = self.lock();
            Self::check_available(&state)?;
            let active: Vec<&Agent> = state
                .agents
                .values()
                .map(|(agent, _)| agent)
                .filter(|agent| !agent.is_archived())
                .collect();
            let purposes: BTreeSet<_> = active
                .iter()
                .map(|agent| agent.draft().purpose().id())
                .collect();
            Ok(purposes
                .into_iter()
                .map(|purpose| PurposeUsage {
                    purpose,
                    agents: u32::try_from(
                        active
                            .iter()
                            .filter(|agent| agent.draft().purpose().id() == purpose)
                            .count(),
                    )
                    .unwrap_or(u32::MAX),
                })
                .collect())
        })
    }

    fn update<'a>(
        &'a self,
        agent: &'a Agent,
        expected: Revision,
        event: &'a AuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(async move {
            self.wait_unless_stalled().await;
            let mut state = self.lock();
            Self::check_available(&state)?;
            if state.conflicts > 0 {
                state.conflicts -= 1;
                return Err(RepoError::Conflict);
            }
            let (stored, revision) = state
                .agents
                .get_mut(&agent.id())
                .ok_or(RepoError::NotFound)?;
            if *revision != expected {
                return Err(RepoError::Conflict);
            }
            *stored = agent.clone();
            *revision = expected.next();
            let next = *revision;
            state.audit.push(event.clone());
            Ok(next)
        })
    }
}
