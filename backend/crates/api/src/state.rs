//! What every handler can reach: the services and the identity settings.

use vak_services::AgentService;

/// Shared state of the `/api/v1` routes.
#[derive(Clone)]
pub struct AppState {
    // CONTRACT-STUB
    _agents: AgentService,
}

impl AppState {
    /// State whose routes accept only the identity Databricks Apps forwards.
    #[must_use]
    pub fn new(agents: AgentService) -> Self {
        Self { _agents: agents }
    }
}
