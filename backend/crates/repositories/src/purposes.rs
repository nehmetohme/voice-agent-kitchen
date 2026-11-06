//! The purpose aggregate's port: the workspace's own purposes, their drafts and their
//! immutable versions. The six system purposes live in the domain, not in storage.

use vak_domain::{
    Purpose, PurposeAuditEvent, PurposeId, PurposeName, PurposeRef, PurposeVersion, UseCase,
};

use crate::{BoxFuture, RepoError, Revision, Versioned};

/// Which purposes [`PurposeRepo::list`] returns: the workspace's own, most recently updated
/// first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PurposeQuery {
    /// At most this many purposes.
    pub limit: u16,
}

/// A purpose as lists show it, without its draft or flows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurposeListing {
    pub id: PurposeId,
    /// The draft's name.
    pub name: PurposeName,
    /// The draft's description.
    pub description: String,
    /// The draft's compliance class.
    pub compliance: UseCase,
    /// The latest saved version's number, if any.
    pub latest_version: Option<u32>,
    /// Whether the draft differs from the latest version (always, before the first).
    pub unsaved_changes: bool,
}

/// Stores the workspace's purposes. Every mutation writes its audit event in the same
/// transaction. Versions are append-only: once stored, a version never changes.
pub trait PurposeRepo: Send + Sync {
    /// Stores a new purpose (a draft, no version yet) and `event`. Fails with
    /// [`RepoError::Conflict`] when the ID already exists.
    fn create<'a>(
        &'a self,
        purpose: &'a Purpose,
        event: &'a PurposeAuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>>;

    /// The purpose, with its draft and latest version, and its revision.
    fn get(&self, id: PurposeId) -> BoxFuture<'_, Result<Versioned<Purpose>, RepoError>>;

    /// The workspace's purposes, most recently updated first.
    fn list(&self, query: PurposeQuery) -> BoxFuture<'_, Result<Vec<PurposeListing>, RepoError>>;

    /// Replaces the draft and stores `event`, if the stored revision is `expected`. Fails
    /// with [`RepoError::Conflict`] on a stale revision, [`RepoError::NotFound`] when the
    /// purpose doesn't exist.
    fn update_draft<'a>(
        &'a self,
        purpose: &'a Purpose,
        expected: Revision,
        event: &'a PurposeAuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>>;

    /// Stores `version` as the purpose's new latest version, with `event`, if the stored
    /// revision is `expected`. Fails with [`RepoError::Conflict`] on a stale revision or a
    /// version number that is already stored.
    fn save_version<'a>(
        &'a self,
        purpose: &'a Purpose,
        expected: Revision,
        version: &'a PurposeVersion,
        event: &'a PurposeAuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>>;

    /// Exactly that version. [`RepoError::NotFound`] when it isn't stored.
    fn version(&self, reference: PurposeRef) -> BoxFuture<'_, Result<PurposeVersion, RepoError>>;
}
