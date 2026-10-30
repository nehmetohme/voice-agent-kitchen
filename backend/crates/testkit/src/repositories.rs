//! Repository fakes, and the conformance suites every implementation must pass.

mod agent_conformance;
mod agents;

pub use agent_conformance::{AuditRow, agent_repo_conformance};
pub use agents::InMemoryAgentRepo;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_in_memory_agent_repo_conforms() {
        let repo = InMemoryAgentRepo::default();

        agent_repo_conformance(&repo, |id| {
            let rows: Vec<AuditRow> = repo
                .audit_events()
                .iter()
                .filter(|event| event.agent_id() == id)
                .map(AuditRow::from)
                .collect();
            async move { rows }
        })
        .await;
    }
}
