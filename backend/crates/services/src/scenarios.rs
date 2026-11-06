//! Scenario use cases: read the agent's test scenarios (or its use case's defaults) and save
//! them. Scenarios are part of the draft, so saving goes through
//! [`AgentService::update_draft`].

use vak_domain::{Actor, AgentDraft, AgentId, DraftPatch, ScenarioSet};

use crate::{AgentService, FindAgentError, UpdateDraftError};

/// The scenarios an agent is tested with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentScenarios {
    pub scenarios: ScenarioSet,
    /// `false` while these are the use case's defaults.
    pub saved: bool,
}

impl AgentScenarios {
    fn of(draft: &AgentDraft) -> Self {
        Self {
            scenarios: draft.scenarios_or_defaults(),
            saved: draft.scenarios().is_some(),
        }
    }
}

impl AgentService {
    /// The agent's saved scenarios, or its use case's defaults until some are saved.
    pub async fn scenarios(&self, id: AgentId) -> Result<AgentScenarios, FindAgentError> {
        Ok(AgentScenarios::of(self.get(id).await?.draft()))
    }

    /// Replaces the agent's scenarios: one `agent.draft_updated` audit event for `scenarios`,
    /// nothing stored or audited when they are unchanged.
    pub async fn save_scenarios(
        &self,
        actor: &Actor,
        id: AgentId,
        scenarios: ScenarioSet,
    ) -> Result<AgentScenarios, UpdateDraftError> {
        let patch = DraftPatch {
            scenarios: Some(scenarios),
            ..DraftPatch::default()
        };
        let agent = self.update_draft(actor, id, &patch).await?;
        Ok(AgentScenarios::of(agent.draft()))
    }
}
