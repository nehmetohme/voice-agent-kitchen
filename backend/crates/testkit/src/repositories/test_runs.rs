//! In-memory `TestRunRepo`: deterministic, with a switch to make calls fail.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use vak_domain::{AgentId, AuditEvent, ContentHash, TestRun, TestRunId, TestRunStatus, Verdict};
use vak_repositories::{BoxFuture, RepoError, Revision, TestRunRepo, Versioned};

#[derive(Debug, Default)]
struct State {
    runs: BTreeMap<TestRunId, (TestRun, Revision)>,
    audit: Vec<AuditEvent>,
    unavailable: bool,
}

/// A [`TestRunRepo`] in memory. Clone the `Arc` you hand to a service to inspect it later.
#[derive(Debug, Default)]
pub struct InMemoryTestRunRepo {
    state: Mutex<State>,
}

impl InMemoryTestRunRepo {
    /// Every audit event written so far, oldest first.
    #[must_use]
    pub fn audit_events(&self) -> Vec<AuditEvent> {
        self.lock().audit.clone()
    }

    /// While `true`, every call fails with [`RepoError::Unavailable`].
    pub fn set_unavailable(&self, unavailable: bool) {
        self.lock().unavailable = unavailable;
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A panicking test may poison the lock; the state is still usable for inspection.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn available(&self) -> Result<MutexGuard<'_, State>, RepoError> {
        let state = self.lock();
        if state.unavailable {
            Err(RepoError::unavailable("in-memory repo switched off"))
        } else {
            Ok(state)
        }
    }
}

/// Oldest first: the creation time, then the ID (UUID v7, time-ordered).
fn age(run: &TestRun) -> (jiff::Timestamp, TestRunId) {
    (run.created_at(), run.id())
}

impl TestRunRepo for InMemoryTestRunRepo {
    fn create<'a>(
        &'a self,
        run: &'a TestRun,
        event: &'a AuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(async move {
            let mut state = self.available()?;
            let busy = state
                .runs
                .values()
                .any(|(other, _)| other.agent_id() == run.agent_id() && other.is_active());
            if busy || state.runs.contains_key(&run.id()) {
                return Err(RepoError::Conflict);
            }
            state.runs.insert(run.id(), (run.clone(), Revision::FIRST));
            state.audit.push(event.clone());
            Ok(Revision::FIRST)
        })
    }

    fn get(&self, id: TestRunId) -> BoxFuture<'_, Result<Versioned<TestRun>, RepoError>> {
        Box::pin(async move {
            let state = self.available()?;
            state
                .runs
                .get(&id)
                .map(|(run, revision)| Versioned {
                    value: run.clone(),
                    revision: *revision,
                })
                .ok_or(RepoError::NotFound)
        })
    }

    fn list(&self, agent: AgentId, limit: u16) -> BoxFuture<'_, Result<Vec<TestRun>, RepoError>> {
        Box::pin(async move {
            let state = self.available()?;
            let mut runs: Vec<TestRun> = state
                .runs
                .values()
                .map(|(run, _)| run)
                .filter(|run| run.agent_id() == agent)
                .cloned()
                .collect();
            runs.sort_by_key(|run| Reverse(age(run)));
            runs.truncate(usize::from(limit));
            Ok(runs)
        })
    }

    fn update<'a>(
        &'a self,
        run: &'a TestRun,
        expected: Revision,
        event: Option<&'a AuditEvent>,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(async move {
            let mut state = self.available()?;
            let (stored, revision) = state.runs.get_mut(&run.id()).ok_or(RepoError::NotFound)?;
            if *revision != expected {
                return Err(RepoError::Conflict);
            }
            *stored = run.clone();
            *revision = expected.next();
            let next = *revision;
            if let Some(event) = event {
                state.audit.push(event.clone());
            }
            Ok(next)
        })
    }

    fn next_queued(&self) -> BoxFuture<'_, Result<Option<Versioned<TestRun>>, RepoError>> {
        Box::pin(async move {
            let state = self.available()?;
            Ok(state
                .runs
                .values()
                .filter(|(run, _)| run.status() == TestRunStatus::Queued)
                .min_by_key(|(run, _)| age(run))
                .map(|(run, revision)| Versioned {
                    value: run.clone(),
                    revision: *revision,
                }))
        })
    }

    fn running(&self) -> BoxFuture<'_, Result<Vec<Versioned<TestRun>>, RepoError>> {
        Box::pin(async move {
            let state = self.available()?;
            let mut running: Vec<Versioned<TestRun>> = state
                .runs
                .values()
                .filter(|(run, _)| run.status() == TestRunStatus::Running)
                .map(|(run, revision)| Versioned {
                    value: run.clone(),
                    revision: *revision,
                })
                .collect();
            running.sort_by_key(|stored| age(&stored.value));
            Ok(running)
        })
    }

    fn latest_verdicts<'a>(
        &'a self,
        drafts: &'a [(AgentId, ContentHash)],
    ) -> BoxFuture<'a, Result<Vec<(AgentId, Verdict)>, RepoError>> {
        Box::pin(async move {
            let state = self.available()?;
            Ok(drafts
                .iter()
                .filter_map(|(agent, hash)| {
                    state
                        .runs
                        .values()
                        .map(|(run, _)| run)
                        .filter(|run| run.agent_id() == *agent && run.draft_hash() == hash)
                        .filter_map(|run| run.verdict().map(|verdict| (run, verdict)))
                        .max_by_key(|(run, _)| age(run))
                        .map(|(_, verdict)| (*agent, verdict))
                })
                .collect())
        })
    }
}
