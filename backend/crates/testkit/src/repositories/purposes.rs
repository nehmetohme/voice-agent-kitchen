//! In-memory `PurposeRepo`: deterministic, with a switch to make calls fail.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use vak_domain::{Purpose, PurposeAuditEvent, PurposeId, PurposeRef, PurposeVersion};
use vak_repositories::{
    BoxFuture, PurposeListing, PurposeQuery, PurposeRepo, RepoError, Revision, Versioned,
};

#[derive(Debug, Default)]
struct State {
    purposes: BTreeMap<PurposeId, (Purpose, Revision)>,
    versions: BTreeMap<PurposeRef, PurposeVersion>,
    audit: Vec<PurposeAuditEvent>,
    unavailable: bool,
}

/// A [`PurposeRepo`] in memory. Clone the `Arc` you hand to a service to inspect it later.
#[derive(Debug, Default)]
pub struct InMemoryPurposeRepo {
    state: Mutex<State>,
}

impl InMemoryPurposeRepo {
    /// Every audit event written so far, oldest first.
    #[must_use]
    pub fn audit_events(&self) -> Vec<PurposeAuditEvent> {
        self.lock().audit.clone()
    }

    /// Every stored version, by purpose and number.
    #[must_use]
    pub fn versions(&self) -> Vec<PurposeVersion> {
        self.lock().versions.values().cloned().collect()
    }

    /// While `true`, every call fails with [`RepoError::Unavailable`].
    pub fn set_unavailable(&self, unavailable: bool) {
        self.lock().unavailable = unavailable;
    }

    fn lock(&self) -> MutexGuard<'_, State> {
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

/// The stored purpose's revision, if it is `expected`.
fn check_revision(state: &State, id: PurposeId, expected: Revision) -> Result<Revision, RepoError> {
    let (_, revision) = state.purposes.get(&id).ok_or(RepoError::NotFound)?;
    if *revision == expected {
        Ok(expected.next())
    } else {
        Err(RepoError::Conflict)
    }
}

impl PurposeRepo for InMemoryPurposeRepo {
    fn create<'a>(
        &'a self,
        purpose: &'a Purpose,
        event: &'a PurposeAuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(async move {
            let mut state = self.available()?;
            if state.purposes.contains_key(&purpose.id()) {
                return Err(RepoError::Conflict);
            }
            state
                .purposes
                .insert(purpose.id(), (purpose.clone(), Revision::FIRST));
            state.audit.push(event.clone());
            Ok(Revision::FIRST)
        })
    }

    fn get(&self, id: PurposeId) -> BoxFuture<'_, Result<Versioned<Purpose>, RepoError>> {
        Box::pin(async move {
            let state = self.available()?;
            state
                .purposes
                .get(&id)
                .map(|(purpose, revision)| Versioned {
                    value: purpose.clone(),
                    revision: *revision,
                })
                .ok_or(RepoError::NotFound)
        })
    }

    fn list(&self, query: PurposeQuery) -> BoxFuture<'_, Result<Vec<PurposeListing>, RepoError>> {
        Box::pin(async move {
            let state = self.available()?;
            let mut purposes: Vec<&Purpose> = state
                .purposes
                .values()
                .map(|(purpose, _)| purpose)
                .collect();
            // Most recently updated first; the ID (UUID v7, time-ordered) breaks ties.
            purposes.sort_by_key(|purpose| Reverse((purpose.updated_at(), purpose.id())));
            purposes.truncate(usize::from(query.limit));
            Ok(purposes
                .into_iter()
                .map(|purpose| PurposeListing {
                    id: purpose.id(),
                    name: purpose.draft().name().clone(),
                    description: purpose.draft().description().to_owned(),
                    compliance: purpose.draft().compliance(),
                    latest_version: purpose.latest_version(),
                    unsaved_changes: purpose.has_unsaved_changes(),
                })
                .collect())
        })
    }

    fn update_draft<'a>(
        &'a self,
        purpose: &'a Purpose,
        expected: Revision,
        event: &'a PurposeAuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(async move {
            let mut state = self.available()?;
            let next = check_revision(&state, purpose.id(), expected)?;
            state.purposes.insert(purpose.id(), (purpose.clone(), next));
            state.audit.push(event.clone());
            Ok(next)
        })
    }

    fn save_version<'a>(
        &'a self,
        purpose: &'a Purpose,
        expected: Revision,
        version: &'a PurposeVersion,
        event: &'a PurposeAuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(async move {
            let mut state = self.available()?;
            let next = check_revision(&state, purpose.id(), expected)?;
            if state.versions.contains_key(&version.reference()) {
                return Err(RepoError::Conflict);
            }
            state.versions.insert(version.reference(), version.clone());
            state.purposes.insert(purpose.id(), (purpose.clone(), next));
            state.audit.push(event.clone());
            Ok(next)
        })
    }

    fn version(&self, reference: PurposeRef) -> BoxFuture<'_, Result<PurposeVersion, RepoError>> {
        Box::pin(async move {
            let state = self.available()?;
            state
                .versions
                .get(&reference)
                .cloned()
                .ok_or(RepoError::NotFound)
        })
    }
}
