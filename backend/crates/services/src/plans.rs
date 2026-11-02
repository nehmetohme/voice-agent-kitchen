//! Plan use cases: read the agent's plan (or its template), save it (autosave), and check a
//! plan without saving it. The plan is part of the draft, so saving goes through
//! [`AgentService::update_draft`]: one `agent.draft_updated` audit event for `plan`, nothing
//! stored or audited when the plan is unchanged.

use vak_domain::{Actor, AgentId, DraftPatch, DraftPlan, Plan, PlanError};

use crate::{AgentService, FindAgentError, UpdateDraftError};

impl AgentService {
    /// The agent's saved plan, or its use case's template until one is saved, checked against
    /// the agent's pinned rule pack.
    pub async fn plan(&self, id: AgentId) -> Result<DraftPlan, FindAgentError> {
        let agent = self.get(id).await?;
        Ok(DraftPlan::of(agent.draft(), &self.rule_packs))
    }

    /// Replaces the agent's plan. A plan that breaks a graph rule is saved too: the answer
    /// carries its errors, and the Plan step stays open until they are fixed.
    pub async fn save_plan(
        &self,
        actor: &Actor,
        id: AgentId,
        plan: Plan,
    ) -> Result<DraftPlan, UpdateDraftError> {
        let patch = DraftPatch {
            plan: Some(plan),
            ..DraftPatch::default()
        };
        let agent = self.update_draft(actor, id, &patch).await?;
        Ok(DraftPlan::of(agent.draft(), &self.rule_packs))
    }

    /// Checks `plan` against the agent's pinned rule pack, without saving it.
    pub async fn validate_plan(
        &self,
        id: AgentId,
        plan: &Plan,
    ) -> Result<Result<(), Vec<PlanError>>, FindAgentError> {
        let agent = self.get(id).await?;
        Ok(self.rule_packs.validate(plan, agent.draft().rule_pack()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vak_domain::{
        AgentName, AuditAction, DraftField, NodeId, NodeKind, PlanNode, RulePackCatalog, UseCase,
        VoiceId,
    };
    use vak_testkit::repositories::InMemoryAgentRepo;

    use super::*;
    use crate::CreateAgent;

    fn actor() -> Actor {
        Actor::new("7364019283746501", "ada@example.com").expect("valid")
    }

    async fn booking(agents: &AgentService) -> AgentId {
        let command = CreateAgent {
            name: AgentName::parse("Bookings").expect("valid"),
            use_case: UseCase::Booking,
            voice: VoiceId::parse("af_heart").expect("valid"),
            language: None,
            jurisdiction: None,
            model: None,
            turn_detection: None,
        };
        agents
            .create(&actor(), command)
            .await
            .expect("created")
            .id()
    }

    fn service() -> (AgentService, Arc<InMemoryAgentRepo>) {
        let repo = Arc::new(InMemoryAgentRepo::default());
        (AgentService::new(repo.clone()), repo)
    }

    #[tokio::test]
    async fn the_template_is_shown_until_a_plan_is_saved() {
        let (agents, repo) = service();
        let id = booking(&agents).await;

        let template = agents.plan(id).await.expect("found");
        let saved = agents
            .save_plan(&actor(), id, template.plan().clone())
            .await
            .expect("saved");

        assert!(!template.saved());
        assert_eq!(template.validation(), &Ok(()));
        assert!(saved.saved());
        assert_eq!(saved.plan(), template.plan());
        assert_eq!(agents.progress(id).await.expect("found").percent(), 50);
        let events = repo.audit_events();
        let fields = events.last().map(|event| event.action().clone());
        assert!(matches!(
            fields,
            Some(AuditAction::AgentDraftUpdated { fields, .. }) if fields == [DraftField::Plan]
        ));
    }

    #[tokio::test]
    async fn validating_saves_nothing_and_uses_the_agents_pack() {
        let (agents, repo) = service();
        let id = booking(&agents).await;
        let lonely = PlanNode::new(
            NodeId::parse("bye").expect("valid"),
            NodeKind::End,
            "Bye",
            "",
        )
        .expect("valid");
        let plan = Plan::new(vec![lonely], vec![]).expect("well-formed");

        let checked = agents.validate_plan(id, &plan).await.expect("found");

        assert_eq!(checked, Err(vec![PlanError::MissingStart]));
        assert_eq!(repo.audit_events().len(), 1, "only the creation");
        assert!(!agents.plan(id).await.expect("found").saved());
    }

    #[tokio::test]
    async fn without_rule_packs_no_plan_is_valid() {
        let (agents, _) = service();
        let agents = agents.with_rule_packs(RulePackCatalog::default());
        let id = booking(&agents).await;

        let shown = agents.plan(id).await.expect("found");

        assert!(shown.validation().is_err());
        assert_eq!(shown.locked_texts(), []);
        assert_eq!(shown.rule_pack().id(), "us-booking");
    }

    #[tokio::test]
    async fn unknown_agents_and_archived_ones_fail_like_other_use_cases() {
        let (agents, _) = service();
        let id = booking(&agents).await;
        let unknown = AgentId::from_uuid(uuid::Uuid::now_v7()).expect("v7");
        agents.archive(&actor(), id).await.expect("archived");

        assert!(matches!(
            agents.plan(unknown).await,
            Err(FindAgentError::NotFound)
        ));
        assert!(matches!(
            agents.validate_plan(unknown, &Plan::default()).await,
            Err(FindAgentError::NotFound)
        ));
        assert!(matches!(
            agents.save_plan(&actor(), id, Plan::default()).await,
            Err(UpdateDraftError::Archived)
        ));
    }
}
