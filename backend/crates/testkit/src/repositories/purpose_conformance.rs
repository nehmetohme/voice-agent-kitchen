//! The `PurposeRepo` conformance suite. Every implementation, the in-memory fake and Postgres,
//! must pass it, so the fake services are tested against can't drift from production.

use jiff::{SignedDuration, Timestamp};
use uuid::Uuid;
use vak_domain::{
    Actor, AuditEventId, BuiltinTool, EscalationAction, EscalationRule, Guardrails, GuardrailsSpec,
    HttpMethod, HttpTool, HttpToolSpec, ParamKind, PersonalData, Purpose, PurposeAuditAction,
    PurposeAuditEvent, PurposeId, PurposeName, PurposePatch, PurposeRef, PurposeTools,
    PurposeVersion, SecretRef, ToolHosts, ToolInputs, ToolName, ToolParam, ToolUrl, UseCase,
};
use vak_repositories::{PurposeQuery, PurposeRepo, RepoError, Revision};

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
    fn purpose(&self, name: &str, minutes: i64) -> Purpose {
        let id = PurposeId::from_uuid(Uuid::now_v7()).expect("v7");
        Purpose::duplicate(
            id,
            PurposeVersion::system(UseCase::Booking).definition(),
            PurposeName::parse(name).expect("valid name"),
            self.at(minutes),
        )
    }

    #[expect(
        clippy::expect_used,
        reason = "fixed fixture values; a failure is a test bug"
    )]
    fn event(&self, purpose: &Purpose, action: PurposeAuditAction) -> PurposeAuditEvent {
        PurposeAuditEvent::new(
            AuditEventId::from_uuid(Uuid::now_v7()).expect("v7"),
            purpose.updated_at(),
            self.actor.clone(),
            purpose.id(),
            action,
        )
    }

    fn created(&self, purpose: &Purpose) -> PurposeAuditEvent {
        self.event(
            purpose,
            PurposeAuditAction::Created {
                source: PurposeId::system(UseCase::Booking),
                version: Some(1),
            },
        )
    }

    fn updated(&self, purpose: &Purpose) -> PurposeAuditEvent {
        self.event(
            purpose,
            PurposeAuditAction::DraftUpdated {
                draft_hash: purpose.draft().content_hash(),
            },
        )
    }
}

/// A draft with every part filled in, so a round trip covers the whole definition.
#[expect(
    clippy::expect_used,
    reason = "fixed fixture values; a failure is a test bug"
)]
fn rich(purpose: &Purpose, now: Timestamp) -> Purpose {
    let input = ToolParam::new(
        ToolName::parse("date").expect("valid"),
        ParamKind::String,
        "The day",
        true,
    )
    .expect("valid");
    let tool = HttpTool::new(HttpToolSpec {
        name: ToolName::parse("book_slot").expect("valid"),
        description: "Book a slot.".to_owned(),
        inputs: ToolInputs::new(vec![input]).expect("valid"),
        method: HttpMethod::Post,
        url: ToolUrl::parse("https://calendar.example.com/v1/slots").expect("valid"),
        timeout_ms: 2_500,
        secret: Some(SecretRef::parse("CALENDAR_API_KEY").expect("valid")),
        confirm: true,
    })
    .expect("valid");
    let patch = PurposePatch {
        description: Some("Clinic appointments.".to_owned()),
        persona: Some("Your job: book clinic appointments.".to_owned()),
        tools: Some(PurposeTools::new([BuiltinTool::TransferToHuman], vec![tool]).expect("valid")),
        guardrails: Some(
            Guardrails::new(GuardrailsSpec {
                allowed_topics: vec!["Appointments".to_owned()],
                never: vec!["Give medical advice".to_owned()],
                escalations: vec![
                    EscalationRule::new(
                        "Is the caller abusive?",
                        EscalationAction::EndCall {
                            message: "Goodbye.".to_owned(),
                        },
                    )
                    .expect("valid"),
                ],
                max_call_minutes: 12,
                never_ask: vec![PersonalData::CardNumber],
            })
            .expect("valid"),
        ),
        ..PurposePatch::default()
    };
    let draft = purpose.draft().apply(&patch).expect("valid");
    purpose
        .update_draft(draft, now)
        .expect("custom")
        .expect("changed")
}

/// Runs every port rule against `repo`, which must start empty.
///
/// # Panics
///
/// When `repo` breaks a rule of the `PurposeRepo` port; the message names the rule.
pub async fn purpose_repo_conformance<R: PurposeRepo + ?Sized>(repo: &R) {
    let fx = Fixture::new();
    let purpose = creates_reads_and_lists(repo, &fx).await;
    let (edited, revision) = updates_need_the_stored_revision(repo, &fx, &purpose).await;
    stores_each_version_once(repo, &fx, &edited, revision).await;
}

/// create, get and list: what create stored reads back, IDs are unique, lists go newest
/// first. Returns the newest purpose.
#[expect(
    clippy::expect_used,
    reason = "a conformance suite fails the calling test"
)]
async fn creates_reads_and_lists<R: PurposeRepo + ?Sized>(repo: &R, fx: &Fixture) -> Purpose {
    let older = fx.purpose("Older", 0);
    let purpose = fx.purpose("Clinic bookings", 1);

    // create, get, list
    repo.create(&older, &fx.created(&older))
        .await
        .expect("create");
    assert_eq!(
        repo.create(&purpose, &fx.created(&purpose)).await.ok(),
        Some(Revision::FIRST),
        "create answers the first revision"
    );
    assert!(
        matches!(
            repo.create(&purpose, &fx.created(&purpose)).await,
            Err(RepoError::Conflict)
        ),
        "a second create with the same ID is a conflict"
    );
    let stored = repo.get(purpose.id()).await.expect("get");
    assert_eq!(stored.value, purpose, "get reads back what create stored");
    assert_eq!(stored.revision, Revision::FIRST);
    let missing = PurposeId::from_uuid(Uuid::now_v7()).expect("v7");
    assert!(
        matches!(repo.get(missing).await, Err(RepoError::NotFound)),
        "an unknown purpose is NotFound"
    );
    let listed = repo.list(PurposeQuery { limit: 10 }).await.expect("list");
    let ids: Vec<PurposeId> = listed.iter().map(|listing| listing.id).collect();
    assert_eq!(ids, [purpose.id(), older.id()], "newest first");
    let first = listed.first().expect("listed");
    assert_eq!(first.name.as_str(), "Clinic bookings");
    assert_eq!(first.compliance, UseCase::Booking);
    assert_eq!(first.latest_version, None);
    assert!(
        first.unsaved_changes,
        "a draft without a version is unsaved"
    );
    let one = repo.list(PurposeQuery { limit: 1 }).await.expect("list");
    assert_eq!(one.len(), 1, "list honours its limit");
    purpose
}

/// `update_draft`: every part of a definition round-trips; a stale revision is a conflict.
#[expect(
    clippy::expect_used,
    reason = "a conformance suite fails the calling test"
)]
async fn updates_need_the_stored_revision<R: PurposeRepo + ?Sized>(
    repo: &R,
    fx: &Fixture,
    purpose: &Purpose,
) -> (Purpose, Revision) {
    let edited = rich(purpose, fx.at(2));
    let revision = repo
        .update_draft(&edited, Revision::FIRST, &fx.updated(&edited))
        .await
        .expect("update");
    assert_eq!(revision, Revision::FIRST.next());
    assert_eq!(
        repo.get(purpose.id()).await.expect("get").value,
        edited,
        "every part of the draft round-trips"
    );
    assert!(
        matches!(
            repo.update_draft(&edited, Revision::FIRST, &fx.updated(&edited))
                .await,
            Err(RepoError::Conflict)
        ),
        "a stale revision is a conflict"
    );
    let ghost = fx.purpose("Ghost", 3);
    assert!(
        matches!(
            repo.update_draft(&ghost, Revision::FIRST, &fx.updated(&ghost))
                .await,
            Err(RepoError::NotFound)
        ),
        "updating an unknown purpose is NotFound"
    );
    (edited, revision)
}

/// `save_version` and `version`: a saved version reads back exactly, becomes the latest, and is
/// stored once.
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "a conformance suite fails the calling test"
)]
async fn stores_each_version_once<R: PurposeRepo + ?Sized>(
    repo: &R,
    fx: &Fixture,
    edited: &Purpose,
    revision: Revision,
) {
    let purpose = edited;
    let (saved, version) = edited
        .save_version(
            &ToolHosts::new(["calendar.example.com"]).expect("valid"),
            fx.at(4),
        )
        .expect("valid draft");
    let event = fx.event(
        &saved,
        PurposeAuditAction::VersionSaved {
            version: 1,
            content_hash: version.definition().content_hash(),
        },
    );
    let revision = repo
        .save_version(&saved, revision, &version, &event)
        .await
        .expect("save version");
    let stored = repo.get(purpose.id()).await.expect("get");
    assert_eq!(stored.revision, revision);
    assert_eq!(stored.value, saved, "get reads back the latest version");
    assert_eq!(
        repo.version(version.reference()).await.expect("version"),
        version,
        "version reads back exactly what was saved"
    );
    let listed = repo.list(PurposeQuery { limit: 10 }).await.expect("list");
    let first = listed.first().expect("listed");
    assert_eq!(first.latest_version, Some(1));
    assert!(!first.unsaved_changes, "the draft is the latest version");
    assert!(
        matches!(
            repo.save_version(&saved, revision, &version, &event).await,
            Err(RepoError::Conflict)
        ),
        "a version number is stored once: versions never change"
    );
    let next = PurposeRef::new(purpose.id(), 2).expect("valid");
    assert!(
        matches!(repo.version(next).await, Err(RepoError::NotFound)),
        "an unsaved version is NotFound"
    );
    let Err(RepoError::Conflict) = repo
        .save_version(&saved, Revision::FIRST, &version, &event)
        .await
    else {
        panic!("saving a version needs the stored revision");
    };
}
