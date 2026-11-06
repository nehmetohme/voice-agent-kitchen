//! Purpose use cases: list, read, duplicate, autosave the draft and its flow, and save a
//! version. The six system purposes come from the domain; the workspace's own are stored
//! through the `PurposeRepo` that [`AgentService::with_purposes`] sets, and agents reach the
//! same store, so both services see the same purposes.
//!
//! Every mutation hands its audit event to the same `PurposeRepo` call that stores the
//! change, so both land in one transaction.

use std::collections::BTreeMap;
use std::sync::Arc;

use uuid::Uuid;
use vak_domain::{
    Actor, AgentDraft, AuditEventId, BuiltinTool, DraftPlan, EffectiveCompliance, Jurisdiction,
    Plan, Purpose, PurposeAuditAction, PurposeAuditEvent, PurposeDefinition, PurposeError,
    PurposeId, PurposeName, PurposePatch, PurposeProblem, PurposeRef, PurposeVersion, RulePack,
    UseCase,
};
use vak_repositories::{PurposeQuery, PurposeRepo, RepoError, Revision, Versioned};

use crate::agents::{AgentService, now};

/// The most purposes of the workspace's own [`PurposeService::list`] returns.
pub const PURPOSE_LIST_LIMIT: u16 = 200;
/// How often a change is retried when another writer saved first.
const ATTEMPTS: u32 = 3;

/// A purpose as lists show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurposeSummary {
    pub id: PurposeId,
    pub name: PurposeName,
    pub description: String,
    pub compliance: UseCase,
    pub system: bool,
    pub latest_version: Option<u32>,
    pub unsaved_changes: bool,
    /// Active agents that pin a version of it.
    pub used_by: u32,
}

/// One shipped jurisdiction's pack for a purpose's compliance class, and the policy that
/// applies with the purpose's additions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplianceBaseline {
    pub pack: RulePack,
    pub effective: EffectiveCompliance,
}

/// A purpose as its page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurposeView {
    pub purpose: Purpose,
    /// What keeps the draft from being saved as a version.
    pub problems: Vec<PurposeProblem>,
    pub used_by: u32,
    /// Per shipped jurisdiction, in code order.
    pub baselines: Vec<ComplianceBaseline>,
}

/// Duplicate a purpose: its `version`, or its latest version (its draft if it has none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicatePurpose {
    pub source: PurposeId,
    pub version: Option<u32>,
    /// Defaults to "Copy of <source name>".
    pub name: Option<PurposeName>,
}

/// Why a use case on a purpose failed to read it.
#[derive(Debug, thiserror::Error)]
pub enum FindPurposeError {
    #[error("no such purpose")]
    NotFound,
    #[error("purpose storage is unavailable")]
    Unavailable(#[source] RepoError),
}

/// Why [`PurposeService::duplicate`] failed.
#[derive(Debug, thiserror::Error)]
pub enum DuplicatePurposeError {
    #[error("no such purpose to duplicate")]
    SourceNotFound,
    #[error("the purpose has no such version")]
    VersionNotFound,
    #[error("purpose storage is unavailable")]
    Unavailable(#[source] RepoError),
}

/// Why a draft change failed.
#[derive(Debug, thiserror::Error)]
pub enum UpdatePurposeError {
    #[error("no such purpose")]
    NotFound,
    #[error("a built-in purpose can't change")]
    ReadOnly,
    #[error("the change breaks a rule")]
    Invalid(#[source] PurposeError),
    #[error("the purpose kept changing while it was being saved")]
    Conflict,
    #[error("purpose storage is unavailable")]
    Unavailable(#[source] RepoError),
}

/// Why [`PurposeService::save_version`] failed.
#[derive(Debug, thiserror::Error)]
pub enum SavePurposeVersionError {
    #[error("no such purpose")]
    NotFound,
    #[error("a built-in purpose can't change")]
    ReadOnly,
    #[error("nothing changed since version {version}")]
    Unchanged { version: u32 },
    #[error("the draft has problems to fix first")]
    Invalid { problems: Vec<PurposeProblem> },
    #[error("the purpose kept changing while it was being saved")]
    Conflict,
    #[error("purpose storage is unavailable")]
    Unavailable(#[source] RepoError),
}

impl From<RepoError> for FindPurposeError {
    fn from(error: RepoError) -> Self {
        match error {
            RepoError::NotFound => Self::NotFound,
            other => Self::Unavailable(other),
        }
    }
}

/// The built-in tools and the hosts HTTP tools may call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCatalog {
    pub builtins: Vec<BuiltinTool>,
    pub allowed_hosts: Vec<String>,
}

/// The purpose use cases. Built from an [`AgentService`], whose purpose store, rule packs,
/// tool host allow-list and deadline it shares.
#[derive(Clone)]
pub struct PurposeService {
    agents: AgentService,
}

impl PurposeService {
    #[must_use]
    pub fn new(agents: &AgentService) -> Self {
        Self {
            agents: agents.clone(),
        }
    }

    /// The six system purposes, in use-case order, then the workspace's own, most recently
    /// updated first (at most [`PURPOSE_LIST_LIMIT`]).
    pub async fn list(&self) -> Result<Vec<PurposeSummary>, FindPurposeError> {
        let usage = self.agents.purpose_usage().await?;
        let used_by = |id: PurposeId| usage.get(&id).copied().unwrap_or(0);
        let mut summaries: Vec<PurposeSummary> = UseCase::ALL
            .into_iter()
            .map(|use_case| {
                let purpose = Purpose::system(use_case);
                PurposeSummary {
                    id: purpose.id(),
                    name: purpose.draft().name().clone(),
                    description: purpose.draft().description().to_owned(),
                    compliance: use_case,
                    system: true,
                    latest_version: purpose.latest_version(),
                    unsaved_changes: false,
                    used_by: used_by(purpose.id()),
                }
            })
            .collect();
        if let Some(repo) = self.agents.purposes.as_ref() {
            let query = PurposeQuery {
                limit: PURPOSE_LIST_LIMIT,
            };
            let own = self.agents.within(repo.list(query)).await?;
            summaries.extend(own.into_iter().map(|listing| PurposeSummary {
                used_by: used_by(listing.id),
                id: listing.id,
                name: listing.name,
                description: listing.description,
                compliance: listing.compliance,
                system: false,
                latest_version: listing.latest_version,
                unsaved_changes: listing.unsaved_changes,
            }));
        }
        Ok(summaries)
    }

    /// One purpose with its problems, usage and compliance baselines.
    pub async fn get(&self, id: PurposeId) -> Result<PurposeView, FindPurposeError> {
        let purpose = self.agents.load_purpose(id).await?.value;
        self.view(purpose).await
    }

    /// A new purpose of the workspace's: a draft copy of another one, without a version.
    pub async fn duplicate(
        &self,
        actor: &Actor,
        cmd: DuplicatePurpose,
    ) -> Result<PurposeView, DuplicatePurposeError> {
        let source = self
            .agents
            .load_purpose(cmd.source)
            .await
            .map_err(|error| match error {
                RepoError::NotFound => DuplicatePurposeError::SourceNotFound,
                other => DuplicatePurposeError::Unavailable(other),
            })?
            .value;
        let copied = cmd.version.or_else(|| source.latest_version());
        let definition = match cmd.version {
            Some(number) => {
                let reference = PurposeRef::new(source.id(), number)
                    .map_err(|_| DuplicatePurposeError::VersionNotFound)?;
                self.agents
                    .purpose_version(reference)
                    .await
                    .map_err(|error| match error {
                        RepoError::NotFound => DuplicatePurposeError::VersionNotFound,
                        other => DuplicatePurposeError::Unavailable(other),
                    })?
                    .definition()
                    .clone()
            }
            None => source.latest().map_or_else(
                || source.draft().clone(),
                |latest| latest.definition().clone(),
            ),
        };
        let name = match cmd.name {
            Some(name) => name,
            None => copy_name(definition.name()),
        };
        let id = PurposeId::from_uuid(Uuid::now_v7())
            .map_err(|error| DuplicatePurposeError::Unavailable(RepoError::unavailable(error)))?;
        let purpose = Purpose::duplicate(id, &definition, name, now());
        let event = audit_event(
            actor,
            &purpose,
            PurposeAuditAction::Created {
                source: source.id(),
                version: copied,
            },
        )
        .map_err(DuplicatePurposeError::Unavailable)?;
        let repo = self
            .agents
            .purpose_repo()
            .map_err(DuplicatePurposeError::Unavailable)?;
        self.agents
            .within(repo.create(&purpose, &event))
            .await
            .map_err(DuplicatePurposeError::Unavailable)?;
        self.view(purpose).await.map_err(|error| match error {
            FindPurposeError::NotFound => DuplicatePurposeError::SourceNotFound,
            FindPurposeError::Unavailable(source) => DuplicatePurposeError::Unavailable(source),
        })
    }

    /// Applies `patch` to the draft (autosave). A patch that changes nothing stores nothing
    /// and writes no audit event.
    pub async fn update_draft(
        &self,
        actor: &Actor,
        id: PurposeId,
        patch: &PurposePatch,
    ) -> Result<PurposeView, UpdatePurposeError> {
        let purpose = self.change_draft(actor, id, patch).await?;
        self.view(purpose).await.map_err(update_error)
    }

    /// The starting flow, as the flow editor shows it.
    pub async fn flow(&self, id: PurposeId) -> Result<DraftPlan, FindPurposeError> {
        let purpose = self.agents.load_purpose(id).await?.value;
        Ok(DraftPlan::of_flow(purpose.draft(), &self.agents.rule_packs))
    }

    /// Replaces the draft's starting flow (autosave). A flow that breaks a graph rule is
    /// saved too, and comes back with its problems.
    pub async fn save_flow(
        &self,
        actor: &Actor,
        id: PurposeId,
        flow: Plan,
    ) -> Result<DraftPlan, UpdatePurposeError> {
        let patch = PurposePatch {
            flow: Some(flow),
            ..PurposePatch::default()
        };
        let purpose = self.change_draft(actor, id, &patch).await?;
        Ok(DraftPlan::of_flow(purpose.draft(), &self.agents.rule_packs))
    }

    /// Freezes the draft as the next version.
    pub async fn save_version(
        &self,
        actor: &Actor,
        id: PurposeId,
    ) -> Result<PurposeView, SavePurposeVersionError> {
        for _ in 0..ATTEMPTS {
            let stored = self
                .agents
                .load_purpose(id)
                .await
                .map_err(|error| match error {
                    RepoError::NotFound => SavePurposeVersionError::NotFound,
                    other => SavePurposeVersionError::Unavailable(other),
                })?;
            let (purpose, version) = stored
                .value
                .save_version(&self.agents.tool_hosts, now())
                .map_err(|error| match error {
                    PurposeError::Unchanged { version } => {
                        SavePurposeVersionError::Unchanged { version }
                    }
                    PurposeError::Invalid { problems } => {
                        SavePurposeVersionError::Invalid { problems }
                    }
                    _ => SavePurposeVersionError::ReadOnly,
                })?;
            let event = audit_event(
                actor,
                &purpose,
                PurposeAuditAction::VersionSaved {
                    version: version.reference().version(),
                    content_hash: version.definition().content_hash(),
                },
            )
            .map_err(SavePurposeVersionError::Unavailable)?;
            let repo = self
                .agents
                .purpose_repo()
                .map_err(SavePurposeVersionError::Unavailable)?;
            match self
                .agents
                .within(repo.save_version(&purpose, stored.revision, &version, &event))
                .await
            {
                Err(RepoError::Conflict) => {}
                Err(error) => return Err(SavePurposeVersionError::Unavailable(error)),
                Ok(_) => {
                    return self.view(purpose).await.map_err(|error| match error {
                        FindPurposeError::NotFound => SavePurposeVersionError::NotFound,
                        FindPurposeError::Unavailable(source) => {
                            SavePurposeVersionError::Unavailable(source)
                        }
                    });
                }
            }
        }
        Err(SavePurposeVersionError::Conflict)
    }

    /// The built-in tools and the tool host allow-list.
    #[must_use]
    pub fn tool_catalog(&self) -> ToolCatalog {
        ToolCatalog {
            builtins: BuiltinTool::ALL.to_vec(),
            allowed_hosts: self.agents.tool_hosts.entries().to_vec(),
        }
    }

    async fn change_draft(
        &self,
        actor: &Actor,
        id: PurposeId,
        patch: &PurposePatch,
    ) -> Result<Purpose, UpdatePurposeError> {
        for _ in 0..ATTEMPTS {
            let stored = self
                .agents
                .load_purpose(id)
                .await
                .map_err(|error| update_error(FindPurposeError::from(error)))?;
            let draft = stored
                .value
                .draft()
                .apply(patch)
                .map_err(UpdatePurposeError::Invalid)?;
            let changed = stored
                .value
                .update_draft(draft, now())
                .map_err(|_read_only| UpdatePurposeError::ReadOnly)?;
            let Some(purpose) = changed else {
                return Ok(stored.value);
            };
            let event = audit_event(
                actor,
                &purpose,
                PurposeAuditAction::DraftUpdated {
                    draft_hash: purpose.draft().content_hash(),
                },
            )
            .map_err(UpdatePurposeError::Unavailable)?;
            let repo = self
                .agents
                .purpose_repo()
                .map_err(UpdatePurposeError::Unavailable)?;
            match self
                .agents
                .within(repo.update_draft(&purpose, stored.revision, &event))
                .await
            {
                Err(RepoError::Conflict) => {}
                Err(error) => return Err(update_error(FindPurposeError::from(error))),
                Ok(_) => return Ok(purpose),
            }
        }
        Err(UpdatePurposeError::Conflict)
    }

    async fn view(&self, purpose: Purpose) -> Result<PurposeView, FindPurposeError> {
        let usage = self.agents.purpose_usage().await?;
        Ok(PurposeView {
            problems: purpose.problems(&self.agents.tool_hosts),
            used_by: usage.get(&purpose.id()).copied().unwrap_or(0),
            baselines: self.baselines(purpose.draft()),
            purpose,
        })
    }

    /// The class's pack in every shipped jurisdiction, by jurisdiction code.
    fn baselines(&self, definition: &PurposeDefinition) -> Vec<ComplianceBaseline> {
        let packs = &self.agents.rule_packs;
        let mut jurisdictions: Vec<&Jurisdiction> =
            packs.iter().map(RulePack::jurisdiction).collect();
        jurisdictions.sort_by_key(|jurisdiction| jurisdiction.as_str());
        jurisdictions.dedup();
        jurisdictions
            .into_iter()
            .filter_map(|jurisdiction| {
                let reference =
                    vak_domain::RulePackRef::for_agent(definition.compliance(), jurisdiction);
                packs.get(&reference)
            })
            .map(|pack| ComplianceBaseline {
                effective: EffectiveCompliance::of(pack, definition.additions()),
                pack: pack.clone(),
            })
            .collect()
    }
}

fn update_error(error: FindPurposeError) -> UpdatePurposeError {
    match error {
        FindPurposeError::NotFound => UpdatePurposeError::NotFound,
        FindPurposeError::Unavailable(source) => UpdatePurposeError::Unavailable(source),
    }
}

/// "Copy of <name>", cut to the longest name.
fn copy_name(source: &PurposeName) -> PurposeName {
    let full = format!("Copy of {}", source.as_str());
    let cut: String = full.chars().take(PurposeName::MAX).collect();
    PurposeName::parse(&cut).unwrap_or_else(|_| source.clone())
}

fn audit_event(
    actor: &Actor,
    purpose: &Purpose,
    action: PurposeAuditAction,
) -> Result<PurposeAuditEvent, RepoError> {
    let id = AuditEventId::from_uuid(Uuid::now_v7()).map_err(RepoError::unavailable)?;
    Ok(PurposeAuditEvent::new(
        id,
        purpose.updated_at(),
        actor.clone(),
        purpose.id(),
        action,
    ))
}

/// Why an agent can't take a purpose's latest version.
#[derive(Debug, thiserror::Error)]
pub(crate) enum LatestVersionError {
    #[error("no such purpose")]
    NotFound,
    #[error("the purpose has no saved version yet")]
    NoVersion,
    #[error("purpose storage is unavailable")]
    Unavailable(#[source] RepoError),
}

/// How an agent reaches purposes: system ones from the domain, the workspace's from the store.
impl AgentService {
    /// The purpose store, or `Unavailable` when this service has none.
    pub(crate) fn purpose_repo(&self) -> Result<&Arc<dyn PurposeRepo>, RepoError> {
        self.purposes
            .as_ref()
            .ok_or_else(|| RepoError::unavailable("no purpose storage is configured"))
    }

    /// The purpose, system or the workspace's. Without a store, only system purposes exist.
    pub(crate) async fn load_purpose(
        &self,
        id: PurposeId,
    ) -> Result<Versioned<Purpose>, RepoError> {
        if let Some(use_case) = id.system_use_case() {
            return Ok(Versioned {
                value: Purpose::system(use_case),
                revision: Revision::FIRST,
            });
        }
        let repo = self.purposes.as_ref().ok_or(RepoError::NotFound)?;
        self.within(repo.get(id)).await
    }

    /// Exactly that version.
    pub(crate) async fn purpose_version(
        &self,
        reference: PurposeRef,
    ) -> Result<PurposeVersion, RepoError> {
        if let Some(use_case) = reference.id().system_use_case() {
            let system = PurposeVersion::system(use_case);
            return if system.reference() == reference {
                Ok(system)
            } else {
                Err(RepoError::NotFound)
            };
        }
        let repo = self.purposes.as_ref().ok_or(RepoError::NotFound)?;
        self.within(repo.version(reference)).await
    }

    /// The latest saved version of the purpose.
    pub(crate) async fn latest_version(
        &self,
        id: PurposeId,
    ) -> Result<PurposeVersion, LatestVersionError> {
        let purpose = self.load_purpose(id).await.map_err(|error| match error {
            RepoError::NotFound => LatestVersionError::NotFound,
            other => LatestVersionError::Unavailable(other),
        })?;
        purpose
            .value
            .latest()
            .cloned()
            .ok_or(LatestVersionError::NoVersion)
    }

    /// The version `draft` pins. A pinned version that can't be found is a storage fault:
    /// versions are never deleted.
    pub(crate) async fn pinned_purpose(
        &self,
        draft: &AgentDraft,
    ) -> Result<PurposeVersion, RepoError> {
        self.purpose_version(*draft.purpose())
            .await
            .map_err(|error| match error {
                RepoError::NotFound => {
                    RepoError::unavailable("an agent pins a missing purpose version")
                }
                other => other,
            })
    }

    /// Active agents per purpose.
    pub(crate) async fn purpose_usage(&self) -> Result<BTreeMap<PurposeId, u32>, RepoError> {
        let usage = self.within(self.repo.purpose_usage()).await?;
        Ok(usage
            .into_iter()
            .map(|usage| (usage.purpose, usage.agents))
            .collect())
    }
}

#[cfg(test)]
mod tests;
