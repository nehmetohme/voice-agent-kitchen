//! The error every port returns. Driver errors are a source, never a variant.

/// Any error, boxed: the source of [`RepoError::Unavailable`].
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Why a persistence call failed.
#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    /// No such aggregate.
    #[error("not found")]
    NotFound,
    /// A stale revision, or a duplicate key.
    #[error("conflict with the stored state")]
    Conflict,
    /// The store could not answer: connection, timeout, or a row the domain rejects.
    #[error("storage unavailable")]
    Unavailable(#[source] BoxError),
}

impl RepoError {
    /// Wraps any error as [`RepoError::Unavailable`].
    pub fn unavailable(source: impl Into<BoxError>) -> Self {
        Self::Unavailable(source.into())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;

    #[test]
    fn unavailable_keeps_the_source_out_of_the_message() {
        let error = RepoError::unavailable("connection refused to db.internal:5432");

        assert_eq!(error.to_string(), "storage unavailable");
        assert_eq!(
            error.source().map(ToString::to_string),
            Some("connection refused to db.internal:5432".to_owned())
        );
    }
}
