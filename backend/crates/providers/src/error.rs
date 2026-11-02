//! `ProviderError`: how every model call fails, whatever the vendor.

use std::time::Duration;

/// A failed model call. Adapters map every vendor error into one of these; vendor types
/// never cross the port. The message of [`ProviderError::Protocol`] never quotes a
/// transcript, prompt or reply.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ProviderError {
    /// No answer before the call's deadline, or the stream stalled.
    #[error("the provider did not answer in time")]
    Timeout,
    /// The provider asked us to slow down.
    #[error("the provider is rate limiting")]
    RateLimited { retry_after: Option<Duration> },
    /// The credentials were refused.
    #[error("the provider refused the credentials")]
    Auth,
    /// The request or the answer broke the provider's contract: a bug or a configuration
    /// error, so retrying does not help.
    #[error("protocol error: {0}")]
    Protocol(String),
    /// The provider is down or unreachable for now.
    #[error("the provider is unavailable")]
    Unavailable,
}

impl ProviderError {
    /// Whether trying again, within the caller's budget, may succeed.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::RateLimited { .. } | Self::Unavailable
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_transient_failures_are_retryable() {
        assert!(ProviderError::Timeout.is_retryable());
        assert!(ProviderError::RateLimited { retry_after: None }.is_retryable());
        assert!(ProviderError::Unavailable.is_retryable());
        assert!(!ProviderError::Auth.is_retryable());
        assert!(!ProviderError::Protocol("bad".to_owned()).is_retryable());
    }
}
