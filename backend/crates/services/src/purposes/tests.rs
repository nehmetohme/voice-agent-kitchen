use vak_domain::{AgentId, AgentName, VoiceId};
use vak_testkit::repositories::{InMemoryAgentRepo, InMemoryPurposeRepo};

use super::*;
use crate::{CreateAgent, UpdateAgentPurpose, UpdateAgentPurposeError};

fn actor() -> Actor {
    Actor::new("7364019283746501", "ada@example.com").expect("valid")
}

struct Fixture {
    agents: AgentService,
    purposes: PurposeService,
    store: Arc<InMemoryPurposeRepo>,
}

fn fixture() -> Fixture {
    let store = Arc::new(InMemoryPurposeRepo::default());
    let agents =
        AgentService::new(Arc::new(InMemoryAgentRepo::default())).with_purposes(store.clone());
    let purposes = PurposeService::new(&agents);
    Fixture {
        agents,
        purposes,
        store,
    }
}

fn copy_of(source: PurposeId, version: Option<u32>) -> DuplicatePurpose {
    DuplicatePurpose {
        source,
        version,
        name: None,
    }
}

fn booking() -> PurposeId {
    PurposeId::system(UseCase::Booking)
}

fn describe(text: &str) -> PurposePatch {
    PurposePatch {
        description: Some(text.to_owned()),
        ..PurposePatch::default()
    }
}

#[tokio::test]
async fn duplicate_copies_a_named_version_and_audits_its_source() {
    let fixture = fixture();

    let copy = fixture
        .purposes
        .duplicate(&actor(), copy_of(booking(), Some(1)))
        .await
        .expect("copied");

    assert_eq!(
        copy.purpose.draft().name().as_str(),
        "Copy of Appointment booking"
    );
    assert_eq!(copy.purpose.latest_version(), None);
    assert!(!copy.purpose.is_system());
    let events = fixture.store.audit_events();
    assert_eq!(
        events.first().map(PurposeAuditEvent::action),
        Some(&PurposeAuditAction::Created {
            source: booking(),
            version: Some(1),
        })
    );
}

#[tokio::test]
async fn duplicate_says_which_part_is_missing() {
    let fixture = fixture();
    let unknown = PurposeId::from_uuid(Uuid::now_v7()).expect("v7");

    let missing_version = fixture
        .purposes
        .duplicate(&actor(), copy_of(booking(), Some(2)))
        .await;
    let version_zero = fixture
        .purposes
        .duplicate(&actor(), copy_of(booking(), Some(0)))
        .await;
    let missing_source = fixture
        .purposes
        .duplicate(&actor(), copy_of(unknown, None))
        .await;

    assert!(matches!(
        missing_version,
        Err(DuplicatePurposeError::VersionNotFound)
    ));
    assert!(matches!(
        version_zero,
        Err(DuplicatePurposeError::VersionNotFound)
    ));
    assert!(matches!(
        missing_source,
        Err(DuplicatePurposeError::SourceNotFound)
    ));
}

#[tokio::test]
async fn a_copy_of_a_purpose_without_versions_copies_its_draft() {
    let fixture = fixture();
    let first = fixture
        .purposes
        .duplicate(&actor(), copy_of(booking(), None))
        .await
        .expect("copied");
    let id = first.purpose.id();
    fixture
        .purposes
        .update_draft(&actor(), id, &describe("Dental clinics."))
        .await
        .expect("saved");

    let second = fixture
        .purposes
        .duplicate(&actor(), copy_of(id, None))
        .await
        .expect("copied");
    let no_version = fixture
        .purposes
        .duplicate(&actor(), copy_of(id, Some(1)))
        .await;

    assert_eq!(second.purpose.draft().description(), "Dental clinics.");
    assert_eq!(
        second.purpose.draft().name().as_str(),
        "Copy of Copy of Appointment booking"
    );
    assert!(matches!(
        no_version,
        Err(DuplicatePurposeError::VersionNotFound)
    ));
}

#[tokio::test]
async fn save_version_refuses_built_ins_unchanged_drafts_and_unknown_purposes() {
    let fixture = fixture();
    let copy = fixture
        .purposes
        .duplicate(&actor(), copy_of(booking(), None))
        .await
        .expect("copied");
    let id = copy.purpose.id();

    let saved = fixture
        .purposes
        .save_version(&actor(), id)
        .await
        .expect("saved");
    let again = fixture.purposes.save_version(&actor(), id).await;
    let built_in = fixture.purposes.save_version(&actor(), booking()).await;
    let unknown = PurposeId::from_uuid(Uuid::now_v7()).expect("v7");
    let missing = fixture.purposes.save_version(&actor(), unknown).await;

    assert_eq!(saved.purpose.latest_version(), Some(1));
    assert!(matches!(
        again,
        Err(SavePurposeVersionError::Unchanged { version: 1 })
    ));
    assert!(matches!(built_in, Err(SavePurposeVersionError::ReadOnly)));
    assert!(matches!(missing, Err(SavePurposeVersionError::NotFound)));
    assert_eq!(fixture.store.versions().len(), 1);
}

#[tokio::test]
async fn a_change_retries_after_another_writer_and_gives_up_after_three_tries() {
    let fixture = fixture();
    let copy = fixture
        .purposes
        .duplicate(&actor(), copy_of(booking(), None))
        .await
        .expect("copied");
    let id = copy.purpose.id();

    fixture.store.fail_next_updates_with_conflict(2);
    let retried = fixture
        .purposes
        .update_draft(&actor(), id, &describe("Dental clinics."))
        .await
        .expect("saved on the third try");
    fixture.store.fail_next_updates_with_conflict(3);
    let lost = fixture
        .purposes
        .update_draft(&actor(), id, &describe("Eye clinics."))
        .await;
    fixture.store.fail_next_updates_with_conflict(2);
    let saved = fixture
        .purposes
        .save_version(&actor(), id)
        .await
        .expect("saved on the third try");

    assert_eq!(retried.purpose.draft().description(), "Dental clinics.");
    assert!(matches!(lost, Err(UpdatePurposeError::Conflict)));
    assert_eq!(saved.purpose.latest_version(), Some(1));
}

#[tokio::test]
async fn a_version_save_that_keeps_conflicting_is_a_conflict() {
    let fixture = fixture();
    let copy = fixture
        .purposes
        .duplicate(&actor(), copy_of(booking(), None))
        .await
        .expect("copied");
    fixture.store.fail_next_updates_with_conflict(3);

    let saved = fixture
        .purposes
        .save_version(&actor(), copy.purpose.id())
        .await;

    assert!(matches!(saved, Err(SavePurposeVersionError::Conflict)));
    assert_eq!(fixture.store.versions().len(), 0);
}

#[tokio::test]
async fn storage_failures_are_unavailable() {
    let fixture = fixture();
    let copy = fixture
        .purposes
        .duplicate(&actor(), copy_of(booking(), None))
        .await
        .expect("copied");
    fixture.store.set_unavailable(true);

    let list = fixture.purposes.list().await;
    let duplicate = fixture
        .purposes
        .duplicate(&actor(), copy_of(copy.purpose.id(), None))
        .await;
    let save = fixture
        .purposes
        .save_version(&actor(), copy.purpose.id())
        .await;

    assert!(matches!(list, Err(FindPurposeError::Unavailable(_))));
    assert!(matches!(
        duplicate,
        Err(DuplicatePurposeError::Unavailable(_))
    ));
    assert!(matches!(save, Err(SavePurposeVersionError::Unavailable(_))));
}

#[tokio::test]
async fn without_a_purpose_store_only_built_in_purposes_exist() {
    let agents = AgentService::new(Arc::new(InMemoryAgentRepo::default()));
    let purposes = PurposeService::new(&agents);

    let list = purposes.list().await.expect("listed");
    let duplicate = purposes.duplicate(&actor(), copy_of(booking(), None)).await;

    assert_eq!(list.len(), UseCase::ALL.len());
    assert!(list.iter().all(|summary| summary.system));
    assert!(matches!(
        duplicate,
        Err(DuplicatePurposeError::Unavailable(_))
    ));
    assert_eq!(purposes.tool_catalog().allowed_hosts, Vec::<String>::new());
}

/// An agent on version 1 of a copy of the booking purpose, and version 2 of that copy.
async fn agent_with_an_update(fixture: &Fixture) -> AgentId {
    let copy = fixture
        .purposes
        .duplicate(&actor(), copy_of(booking(), None))
        .await
        .expect("copied");
    let id = copy.purpose.id();
    fixture
        .purposes
        .save_version(&actor(), id)
        .await
        .expect("version 1");
    let agent = fixture
        .agents
        .create_with_purpose(
            &actor(),
            CreateAgent {
                name: AgentName::parse("Clinic line").expect("valid"),
                use_case: UseCase::Booking,
                voice: VoiceId::parse("af_heart").expect("valid"),
                language: None,
                jurisdiction: None,
                model: None,
                turn_detection: None,
            },
            Some(id),
        )
        .await
        .expect("created");
    fixture
        .purposes
        .update_draft(&actor(), id, &describe("Dental clinics."))
        .await
        .expect("saved");
    fixture
        .purposes
        .save_version(&actor(), id)
        .await
        .expect("version 2");
    agent.id()
}

fn to(version: u32) -> UpdateAgentPurpose {
    UpdateAgentPurpose {
        version,
        replace_plan: false,
    }
}

#[tokio::test]
async fn an_agent_moves_only_to_another_existing_version() {
    let fixture = fixture();
    let agent = agent_with_an_update(&fixture).await;

    let same = fixture.agents.update_purpose(&actor(), agent, to(1)).await;
    let missing = fixture.agents.update_purpose(&actor(), agent, to(3)).await;
    let zero = fixture.agents.update_purpose(&actor(), agent, to(0)).await;
    let unknown = AgentId::from_uuid(Uuid::now_v7()).expect("v7");
    let nobody = fixture
        .agents
        .update_purpose(&actor(), unknown, to(2))
        .await;

    assert!(matches!(same, Err(UpdateAgentPurposeError::SameVersion)));
    assert!(matches!(
        missing,
        Err(UpdateAgentPurposeError::VersionNotFound)
    ));
    assert!(matches!(
        zero,
        Err(UpdateAgentPurposeError::VersionNotFound)
    ));
    assert!(matches!(nobody, Err(UpdateAgentPurposeError::NotFound)));

    let moved = fixture
        .agents
        .update_purpose(&actor(), agent, to(2))
        .await
        .expect("moved");
    assert_eq!(moved.reference.version(), 2);
    assert_eq!(moved.update, None);
}

#[tokio::test]
async fn an_archived_agent_or_unavailable_storage_stays_where_it_is() {
    let fixture = fixture();
    let agent = agent_with_an_update(&fixture).await;
    fixture.store.set_unavailable(true);

    let unavailable = fixture.agents.update_purpose(&actor(), agent, to(2)).await;
    fixture.store.set_unavailable(false);
    fixture
        .agents
        .archive(&actor(), agent)
        .await
        .expect("archived");
    let archived = fixture.agents.update_purpose(&actor(), agent, to(2)).await;

    assert!(matches!(
        unavailable,
        Err(UpdateAgentPurposeError::Unavailable(_))
    ));
    assert!(matches!(archived, Err(UpdateAgentPurposeError::Archived)));
}
