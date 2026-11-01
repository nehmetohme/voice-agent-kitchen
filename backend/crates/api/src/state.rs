//! What every handler can reach: the services and the identity settings.

use std::time::Duration;

use vak_domain::Actor;
use vak_services::AgentService;

use crate::identity::Identity;

/// Shared state of the `/api/v1` routes.
#[derive(Clone)]
pub struct AppState {
    pub(crate) agents: AgentService,
    pub(crate) identity: Identity,
    pub(crate) request_timeout: Duration,
}

/// The deadline of an `/api/v1` request unless `vak-server` configures another.
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

impl AppState {
    /// State whose routes accept only the identity Databricks Apps forwards.
    #[must_use]
    pub fn new(agents: AgentService) -> Self {
        Self {
            agents,
            identity: Identity::default(),
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }

    /// The same state with another request deadline (default 10 s).
    #[must_use]
    pub fn with_request_timeout(self, request_timeout: Duration) -> Self {
        Self {
            request_timeout,
            ..self
        }
    }

    /// Local development only: requests without identity headers act as `operator`.
    /// `vak-server` allows this only in the local profile.
    #[must_use]
    pub fn with_dev_operator(self, operator: Actor) -> Self {
        Self {
            identity: Identity::with_dev_operator(operator),
            ..self
        }
    }
}
