//! Persistence ports: one trait per aggregate (`AgentRepo` today; `PlanRepo`,
//! `VersionRepo`, `TestRunRepo`, `SessionRepo`, `AuditRepo` and the `JobQueue` port arrive
//! with their milestones), plus `RepoError`.
//!
//! Traits and port types only. The Postgres implementation lives in
//! `vak-repositories-pg`; in-memory fakes live in `vak-testkit`.
//!
//! Ports return boxed futures, so services can hold them as `Arc<dyn AgentRepo>` and the
//! composition root picks the implementation.

mod agents;
mod error;

pub use agents::{AgentQuery, AgentRepo};
pub use error::{BoxError, RepoError};

use std::future::Future;
use std::pin::Pin;

/// A boxed, sendable future: what every port method returns.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The stored revision of an aggregate, for optimistic concurrency. It starts at 1 and goes
/// up by one on every update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(u64);

impl Revision {
    /// The revision of a newly created aggregate.
    pub const FIRST: Self = Self(1);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The revision after one more update.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// A loaded aggregate and the revision it was loaded at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versioned<T> {
    pub value: T,
    pub revision: Revision,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revisions_start_at_one_and_count_up() {
        assert_eq!(Revision::FIRST.get(), 1);
        assert_eq!(Revision::FIRST.next(), Revision::new(2));
        assert_eq!(Revision::new(u64::MAX).next(), Revision::new(u64::MAX));
    }
}
