//! What every handler can reach: the services and the identity settings.

use vak_domain::Actor;
use vak_services::AgentService;

use crate::identity::Identity;

/// Shared state of the `/api/v1` routes.
#[derive(Clone)]
pub struct AppState {
    pub(crate) agents: AgentService,
    pub(crate) identity: Identity,
}

impl AppState {
    /// State whose routes accept only the identity Databricks Apps forwards.
    #[must_use]
    pub fn new(agents: AgentService) -> Self {
        Self {
            agents,
            identity: Identity::default(),
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
