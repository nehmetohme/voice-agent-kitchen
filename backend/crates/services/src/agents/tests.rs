use std::future::pending;

use vak_domain::DraftField;
use vak_repositories::{BoxFuture, Versioned};
use vak_testkit::repositories::InMemoryAgentRepo;

use super::*;

fn actor() -> Actor {
    Actor::new("7364019283746501", "ada@example.com").expect("valid")
}

fn front_desk() -> CreateAgent {
    CreateAgent {
        name: AgentName::parse("Front desk").expect("valid"),
        use_case: UseCase::Support,
        voice: VoiceId::parse("af_heart").expect("valid"),
        language: None,
        jurisdiction: None,
        model: None,
        turn_detection: None,
    }
}

fn rename(to: &str) -> DraftPatch {
    DraftPatch {
        name: Some(AgentName::parse(to).expect("valid")),
        ..DraftPatch::default()
    }
}

fn service() -> (AgentService, Arc<InMemoryAgentRepo>) {
    let repo = Arc::new(InMemoryAgentRepo::default());
    (AgentService::new(repo.clone()), repo)
}

fn actions(repo: &InMemoryAgentRepo) -> Vec<&'static str> {
    repo.audit_events()
        .iter()
        .map(|event| event.action().name())
        .collect()
}

#[tokio::test]
async fn create_fills_in_the_defaults_and_audits_the_creation() {
    let (agents, repo) = service();

    let agent = agents
        .create(&actor(), front_desk())
        .await
        .expect("created");

    let draft = agent.draft();
    assert_eq!(draft.persona().language(), Language::default());
    assert_eq!(draft.jurisdiction(), &Jurisdiction::default());
    assert_eq!(draft.settings().model(), ModelProfile::Balanced);
    assert_eq!(agent.created_at(), agent.updated_at());
    assert_eq!(
        agent.created_at().subsec_nanosecond() % 1_000,
        0,
        "µs precision"
    );
    assert_eq!(agents.get(agent.id()).await.expect("stored"), agent);
    let events = repo.audit_events();
    let event = events.first().expect("one event");
    assert_eq!(events.len(), 1);
    assert_eq!(event.actor(), &actor());
    assert_eq!(event.agent_id(), agent.id());
    assert_eq!(event.at(), agent.created_at());
    assert_eq!(
        event.action(),
        &AuditAction::AgentCreated {
            draft_hash: draft.content_hash()
        }
    );
}

#[tokio::test]
async fn create_overrides_the_defaults_it_is_given() {
    let (agents, _repo) = service();
    let cmd = CreateAgent {
        language: Some(Language::parse("en-GB").expect("valid")),
        jurisdiction: Some(Jurisdiction::parse("UK").expect("valid")),
        model: Some(ModelProfile::Fast),
        turn_detection: Some(TurnDetection::new(900, false).expect("valid")),
        ..front_desk()
    };

    let agent = agents.create(&actor(), cmd).await.expect("created");

    let draft = agent.draft();
    assert_eq!(draft.persona().language().as_str(), "en-GB");
    assert_eq!(draft.rule_pack().id(), "uk-support");
    assert_eq!(draft.settings().model(), ModelProfile::Fast);
    assert_eq!(draft.settings().turn_detection().silence_ms(), 900);
}

#[tokio::test]
async fn every_use_case_fails_cleanly_when_storage_is_down() {
    let (agents, repo) = service();
    let id = agents
        .create(&actor(), front_desk())
        .await
        .expect("created")
        .id();
    repo.set_unavailable(true);

    assert!(matches!(
        agents.create(&actor(), front_desk()).await,
        Err(CreateAgentError::Unavailable(_))
    ));
    assert!(matches!(
        agents.list().await,
        Err(ListAgentsError::Unavailable(_))
    ));
    assert!(matches!(
        agents.get(id).await,
        Err(FindAgentError::Unavailable(_))
    ));
    assert!(matches!(
        agents.progress(id).await,
        Err(FindAgentError::Unavailable(_))
    ));
    assert!(matches!(
        agents.update_draft(&actor(), id, &rename("x")).await,
        Err(UpdateDraftError::Unavailable(_))
    ));
    assert!(matches!(
        agents.archive(&actor(), id).await,
        Err(ArchiveAgentError::Unavailable(_))
    ));
}

#[tokio::test]
async fn list_shows_active_agents_newest_first_with_their_progress() {
    let (agents, _repo) = service();
    let first = agents
        .create(&actor(), front_desk())
        .await
        .expect("created");
    let second = agents
        .create(&actor(), front_desk())
        .await
        .expect("created");
    let archived = agents
        .create(&actor(), front_desk())
        .await
        .expect("created");
    agents
        .archive(&actor(), archived.id())
        .await
        .expect("archived");

    let summaries = agents.list().await.expect("listed");

    let ids: Vec<AgentId> = summaries.iter().map(|summary| summary.agent.id()).collect();
    assert_eq!(ids, [second.id(), first.id()]);
    assert!(
        summaries
            .iter()
            .all(|summary| summary.progress.percent() == 25)
    );
}

#[tokio::test]
async fn unknown_agents_are_not_found() {
    let (agents, _repo) = service();
    let unknown = AgentId::from_uuid(Uuid::now_v7()).expect("v7");

    assert!(matches!(
        agents.get(unknown).await,
        Err(FindAgentError::NotFound)
    ));
    assert!(matches!(
        agents.progress(unknown).await,
        Err(FindAgentError::NotFound)
    ));
    assert!(matches!(
        agents.update_draft(&actor(), unknown, &rename("x")).await,
        Err(UpdateDraftError::NotFound)
    ));
    assert!(matches!(
        agents.archive(&actor(), unknown).await,
        Err(ArchiveAgentError::NotFound)
    ));
}

#[tokio::test]
async fn a_new_agent_is_25_percent_set_up() {
    let (agents, _repo) = service();
    let agent = agents
        .create(&actor(), front_desk())
        .await
        .expect("created");

    let progress = agents.progress(agent.id()).await.expect("found");

    assert_eq!(progress.percent(), 25);
}

#[tokio::test]
async fn update_draft_saves_the_change_and_audits_the_fields() {
    let (agents, repo) = service();
    let agent = agents
        .create(&actor(), front_desk())
        .await
        .expect("created");

    let updated = agents
        .update_draft(&actor(), agent.id(), &rename("After hours"))
        .await
        .expect("updated");

    assert_eq!(updated.draft().persona().name().as_str(), "After hours");
    assert!(updated.updated_at() >= agent.updated_at());
    assert_eq!(agents.get(agent.id()).await.expect("stored"), updated);
    let events = repo.audit_events();
    assert_eq!(
        events.last().map(|event| event.action().clone()),
        Some(AuditAction::AgentDraftUpdated {
            fields: vec![DraftField::Name],
            draft_hash: updated.draft().content_hash(),
        })
    );
}

#[tokio::test]
async fn a_patch_that_changes_nothing_stores_and_audits_nothing() {
    let (agents, repo) = service();
    let agent = agents
        .create(&actor(), front_desk())
        .await
        .expect("created");

    let unchanged = agents
        .update_draft(&actor(), agent.id(), &rename("Front desk"))
        .await
        .expect("no-op");

    assert_eq!(unchanged, agent);
    assert_eq!(actions(&repo), ["agent.created"]);
}

#[tokio::test]
async fn update_draft_retries_when_another_writer_saved_first() {
    let (agents, repo) = service();
    let agent = agents
        .create(&actor(), front_desk())
        .await
        .expect("created");
    repo.fail_next_updates_with_conflict(UPDATE_ATTEMPTS - 1);

    let updated = agents
        .update_draft(&actor(), agent.id(), &rename("After hours"))
        .await
        .expect("saved on the last attempt");

    assert_eq!(updated.draft().persona().name().as_str(), "After hours");
    assert_eq!(actions(&repo), ["agent.created", "agent.draft_updated"]);
}

#[tokio::test]
async fn update_draft_gives_up_after_repeated_conflicts() {
    let (agents, repo) = service();
    let agent = agents
        .create(&actor(), front_desk())
        .await
        .expect("created");
    repo.fail_next_updates_with_conflict(UPDATE_ATTEMPTS);

    let result = agents
        .update_draft(&actor(), agent.id(), &rename("After hours"))
        .await;

    assert!(matches!(result, Err(UpdateDraftError::Conflict)));
    assert_eq!(actions(&repo), ["agent.created"]);
}

#[tokio::test]
async fn archive_happens_once_and_freezes_the_draft() {
    let (agents, repo) = service();
    let agent = agents
        .create(&actor(), front_desk())
        .await
        .expect("created");

    agents
        .archive(&actor(), agent.id())
        .await
        .expect("archived");
    agents
        .archive(&actor(), agent.id())
        .await
        .expect("archiving again is fine");

    let archived = agents.get(agent.id()).await.expect("still readable");
    assert!(archived.is_archived());
    assert_eq!(actions(&repo), ["agent.created", "agent.archived"]);
    assert!(matches!(
        agents
            .update_draft(&actor(), agent.id(), &rename("Too late"))
            .await,
        Err(UpdateDraftError::Archived)
    ));
}

#[tokio::test]
async fn archive_gives_up_after_repeated_conflicts() {
    let (agents, repo) = service();
    let agent = agents
        .create(&actor(), front_desk())
        .await
        .expect("created");
    repo.fail_next_updates_with_conflict(UPDATE_ATTEMPTS);

    let result = agents.archive(&actor(), agent.id()).await;

    assert!(matches!(result, Err(ArchiveAgentError::Conflict)));
}

/// A repository whose calls never finish.
struct Hanging;

impl AgentRepo for Hanging {
    fn create<'a>(
        &'a self,
        _agent: &'a Agent,
        _event: &'a AuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(pending())
    }

    fn get(&self, _id: AgentId) -> BoxFuture<'_, Result<Versioned<Agent>, RepoError>> {
        Box::pin(pending())
    }

    fn list(&self, _query: AgentQuery) -> BoxFuture<'_, Result<Vec<Agent>, RepoError>> {
        Box::pin(pending())
    }

    fn update<'a>(
        &'a self,
        _agent: &'a Agent,
        _expected: Revision,
        _event: &'a AuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(pending())
    }
}

#[tokio::test(start_paused = true)]
async fn a_repository_call_that_hangs_fails_at_the_deadline() {
    let agents = AgentService::new(Arc::new(Hanging)).with_deadline(Duration::from_millis(50));

    let result = agents.create(&actor(), front_desk()).await;

    let Err(CreateAgentError::Unavailable(RepoError::Unavailable(source))) = result else {
        panic!("expected a deadline error, got {result:?}");
    };
    assert!(source.to_string().contains("deadline"), "{source}");
    assert!(matches!(
        agents.list().await,
        Err(ListAgentsError::Unavailable(_))
    ));
}
