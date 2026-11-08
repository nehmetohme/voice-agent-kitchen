//! Repository fakes, and the conformance suites every implementation must pass.

mod agent_conformance;
mod agents;
mod purpose_conformance;
mod purposes;
mod test_run_conformance;
mod test_runs;

pub use agent_conformance::{AuditRow, agent_repo_conformance};
pub use agents::InMemoryAgentRepo;
pub use purpose_conformance::purpose_repo_conformance;
pub use purposes::InMemoryPurposeRepo;
pub use test_run_conformance::test_run_repo_conformance;
pub use test_runs::InMemoryTestRunRepo;

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

    #[tokio::test]
    async fn the_in_memory_test_run_repo_conforms() {
        test_run_repo_conformance(
            &InMemoryAgentRepo::default(),
            &InMemoryTestRunRepo::default(),
        )
        .await;
    }

    #[tokio::test]
    async fn the_in_memory_purpose_repo_conforms() {
        purpose_repo_conformance(&InMemoryPurposeRepo::default()).await;
    }
}
