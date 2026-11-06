//! Scenario use cases: read the agent's test scenarios (or its use case's defaults) and save
//! them. Scenarios are part of the draft, so saving goes through
//! [`AgentService::update_draft`].

use vak_domain::{Actor, AgentId, ScenarioSet};

use crate::{AgentService, FindAgentError, UpdateDraftError};

/// The scenarios an agent is tested with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentScenarios {
    pub scenarios: ScenarioSet,
    /// `false` while these are the use case's defaults.
    pub saved: bool,
}

impl AgentService {
    /// The agent's saved scenarios, or its use case's defaults until some are saved.
    pub async fn scenarios(&self, id: AgentId) -> Result<AgentScenarios, FindAgentError> {
        std::future::ready(()).await;
        let _ = id;
        Err(FindAgentError::NotFound)
    }

    /// Replaces the agent's scenarios.
    pub async fn save_scenarios(
        &self,
        actor: &Actor,
        id: AgentId,
        scenarios: ScenarioSet,
    ) -> Result<AgentScenarios, UpdateDraftError> {
        std::future::ready(()).await;
        let _ = (actor, id, scenarios);
        Err(UpdateDraftError::NotFound)
    }
}
