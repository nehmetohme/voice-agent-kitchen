//! What every handler can reach: the services and the identity settings.

use std::time::Duration;

use vak_domain::Actor;
use vak_services::{AgentService, PurposeService, TestRunService, VoiceService};

use crate::identity::Identity;

/// Shared state of the `/api/v1` routes.
#[derive(Clone)]
pub struct AppState {
    pub(crate) agents: AgentService,
    pub(crate) purposes: PurposeService,
    pub(crate) voice: VoiceService,
    /// `None`: this server runs no test runs.
    pub(crate) test_runs: Option<TestRunService>,
    pub(crate) identity: Identity,
    pub(crate) request_timeout: Duration,
}

/// The deadline of an `/api/v1` request unless `vak-server` configures another.
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

impl AppState {
    /// State whose routes accept only the identity Databricks Apps forwards. Purposes share
    /// the agents' store (`AgentService::with_purposes`). Voice sessions are off until
    /// [`AppState::with_voice`] provides a service with models, and test runs until
    /// [`AppState::with_test_runs`] does.
    #[must_use]
    pub fn new(agents: AgentService) -> Self {
        Self {
            purposes: PurposeService::new(&agents),
            voice: VoiceService::new(agents.clone(), None),
            test_runs: None,
            agents,
            identity: Identity::default(),
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }

    /// The same state with voice sessions served by `voice`.
    #[must_use]
    pub fn with_voice(self, voice: VoiceService) -> Self {
        Self { voice, ..self }
    }

    /// The same state with test runs served by `test_runs`.
    #[must_use]
    pub fn with_test_runs(self, test_runs: TestRunService) -> Self {
        Self {
            test_runs: Some(test_runs),
            ..self
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
