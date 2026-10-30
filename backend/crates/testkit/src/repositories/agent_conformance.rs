//! The `AgentRepo` conformance suite. Every implementation, the in-memory fake and Postgres,
//! must pass it, so the fake services are tested against can't drift from production.

use std::future::Future;

use jiff::{SignedDuration, Timestamp};
use uuid::Uuid;
use vak_domain::{
    Actor, Agent, AgentDraft, AgentId, AgentName, AuditAction, AuditEvent, AuditEventId,
    DraftPatch, UseCase, VoiceId,
};
use vak_repositories::{AgentQuery, AgentRepo, RepoError, Revision};

/// One stored audit event, as the suite reads it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRow {
    pub action: String,
    pub actor_user_id: String,
    pub actor_email: String,
    pub agent_id: AgentId,
}

impl From<&AuditEvent> for AuditRow {
    fn from(event: &AuditEvent) -> Self {
        Self {
            action: event.action().name().to_owned(),
            actor_user_id: event.actor().user_id().to_owned(),
            actor_email: event.actor().email().to_owned(),
            agent_id: event.agent_id(),
        }
    }
}

/// Builds test agents and events with fixed, microsecond-precise timestamps.
struct Fixture {
    actor: Actor,
    start: Timestamp,
}

impl Fixture {
    #[expect(
        clippy::expect_used,
        reason = "fixed fixture values; a failure is a test bug"
    )]
    fn new() -> Self {
        Self {
            actor: Actor::new("4815162342", "ada@example.com").expect("valid actor"),
            start: Timestamp::from_microsecond(1_793_000_000_123_456).expect("valid time"),
        }
    }

    fn at(&self, minutes: i64) -> Timestamp {
        self.start + SignedDuration::from_mins(minutes)
    }

    #[expect(
        clippy::expect_used,
        reason = "fixed fixture values; a failure is a test bug"
    )]
    fn agent(&self, name: &str, minutes: i64) -> Agent {
        let id = AgentId::from_uuid(Uuid::now_v7()).expect("v7");
        let draft = AgentDraft::new(
            AgentName::parse(name).expect("valid name"),
            UseCase::Support,
            VoiceId::parse("af_heart").expect("valid voice"),
        );
        Agent::create(id, draft, self.at(minutes))
    }

    #[expect(
        clippy::expect_used,
        reason = "fixed fixture values; a failure is a test bug"
    )]
    fn event(&self, agent: &Agent, action: AuditAction) -> AuditEvent {
        let id = AuditEventId::from_uuid(Uuid::now_v7()).expect("v7");
        AuditEvent::new(
            id,
            agent.updated_at(),
            self.actor.clone(),
            agent.id(),
            action,
        )
    }

    fn created(&self, agent: &Agent) -> AuditEvent {
        self.event(
            agent,
            AuditAction::AgentCreated {
                draft_hash: agent.draft().content_hash(),
            },
        )
    }

    fn row(&self, action: &str, agent: &Agent) -> AuditRow {
        AuditRow {
            action: action.to_owned(),
            actor_user_id: self.actor.user_id().to_owned(),
            actor_email: self.actor.email().to_owned(),
            agent_id: agent.id(),
        }
    }
}

/// Runs every port rule against `repo`, which must start empty. `audit_trail` reads back the
/// audit events stored for one agent, oldest first.
///
/// # Panics
///
/// When `repo` breaks a rule of the `AgentRepo` port; the message names the rule.
pub async fn agent_repo_conformance<R, F, Fut>(repo: &R, audit_trail: F)
where
    R: AgentRepo + ?Sized,
    F: Fn(AgentId) -> Fut,
    Fut: Future<Output = Vec<AuditRow>>,
{
    let fx = Fixture::new();
    let old = lists_active_agents_newest_first(repo, &fx).await;
    reads_back_what_create_stored(repo, &fx, &old, &audit_trail).await;
    updates_need_the_stored_revision(repo, &fx, &old, &audit_trail).await;
}

/// list: active agents only, most recently updated first, at most `limit`. Returns the oldest
/// agent, still active, for the other rules.
#[expect(
    clippy::expect_used,
    reason = "a conformance suite fails the calling test"
)]
async fn lists_active_agents_newest_first<R: AgentRepo + ?Sized>(repo: &R, fx: &Fixture) -> Agent {
    let old = fx.agent("Old", 0);
    let new = fx.agent("New", 10);
    let gone = fx.agent("Gone", 20);
    for agent in [&old, &new, &gone] {
        repo.create(agent, &fx.created(agent))
            .await
            .expect("create stores a new agent");
    }
    let archived = gone.archive(fx.at(30)).expect("active");
    repo.update(
        &archived,
        Revision::FIRST,
        &fx.event(&archived, AuditAction::AgentArchived),
    )
    .await
    .expect("update archives");

    let listed: Vec<AgentId> = repo
        .list(AgentQuery { limit: 10 })
        .await
        .expect("list")
        .iter()
        .map(Agent::id)
        .collect();
    assert_eq!(
        listed,
        [new.id(), old.id()],
        "list: active only, newest first"
    );
    let limited = repo.list(AgentQuery { limit: 1 }).await.expect("list");
    assert_eq!(limited.len(), 1, "list honours the limit");
    assert_eq!(
        repo.get(gone.id())
            .await
            .expect("archived agents stay readable")
            .value,
        archived,
        "get returns archived agents too"
    );
    old
}

/// create + get: the agent reads back exactly, at revision 1, with its audit event; a
/// duplicate ID is a conflict; an unknown ID is not found.
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "a conformance suite fails the calling test"
)]
async fn reads_back_what_create_stored<R, F, Fut>(
    repo: &R,
    fx: &Fixture,
    old: &Agent,
    audit_trail: &F,
) where
    R: AgentRepo + ?Sized,
    F: Fn(AgentId) -> Fut,
    Fut: Future<Output = Vec<AuditRow>>,
{
    let stored = repo.get(old.id()).await.expect("get finds a created agent");
    assert_eq!(
        &stored.value, old,
        "get returns the agent exactly as created"
    );
    assert_eq!(
        stored.revision,
        Revision::FIRST,
        "a new agent is at revision 1"
    );
    assert_eq!(
        audit_trail(old.id()).await,
        [fx.row("agent.created", old)],
        "create writes its audit event"
    );

    match repo.create(old, &fx.created(old)).await {
        Err(RepoError::Conflict) => {}
        other => panic!("create with an existing ID must be Conflict, got {other:?}"),
    }
    assert_eq!(
        audit_trail(old.id()).await.len(),
        1,
        "a failed create writes no event"
    );

    let unknown = AgentId::from_uuid(Uuid::now_v7()).expect("v7");
    assert!(
        matches!(repo.get(unknown).await, Err(RepoError::NotFound)),
        "get of an unknown ID is NotFound"
    );
}

/// update: needs the stored revision, bumps it and writes its event; a stale revision is a
/// conflict that changes nothing; an unknown agent is not found.
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "a conformance suite fails the calling test"
)]
async fn updates_need_the_stored_revision<R, F, Fut>(
    repo: &R,
    fx: &Fixture,
    old: &Agent,
    audit_trail: &F,
) where
    R: AgentRepo + ?Sized,
    F: Fn(AgentId) -> Fut,
    Fut: Future<Output = Vec<AuditRow>>,
{
    let patch = DraftPatch {
        name: Some(AgentName::parse("Renamed").expect("valid")),
        ..DraftPatch::default()
    };
    let change = old
        .update_draft(&patch, fx.at(40))
        .expect("active")
        .expect("changed");
    let event = fx.event(
        &change.agent,
        AuditAction::AgentDraftUpdated {
            fields: change.fields.clone(),
            draft_hash: change.agent.draft().content_hash(),
        },
    );
    let revision = repo
        .update(&change.agent, Revision::FIRST, &event)
        .await
        .expect("update with the stored revision");
    assert_eq!(
        revision,
        Revision::FIRST.next(),
        "update bumps the revision"
    );
    let reread = repo.get(old.id()).await.expect("get");
    assert_eq!(reread.value, change.agent, "update stores the new draft");
    assert_eq!(reread.revision, revision, "get returns the new revision");

    match repo.update(old, Revision::FIRST, &event).await {
        Err(RepoError::Conflict) => {}
        other => panic!("update with a stale revision must be Conflict, got {other:?}"),
    }
    assert_eq!(
        repo.get(old.id()).await.expect("get").value,
        change.agent,
        "a stale update changes nothing"
    );
    assert_eq!(
        audit_trail(old.id()).await,
        [
            fx.row("agent.created", old),
            fx.row("agent.draft_updated", old)
        ],
        "only successful mutations write events"
    );

    let stranger = fx.agent("Stranger", 50);
    assert!(
        matches!(
            repo.update(&stranger, Revision::FIRST, &fx.created(&stranger))
                .await,
            Err(RepoError::NotFound)
        ),
        "update of an unknown agent is NotFound"
    );
    assert_eq!(
        audit_trail(stranger.id()).await,
        [],
        "a failed update writes no event"
    );
}
