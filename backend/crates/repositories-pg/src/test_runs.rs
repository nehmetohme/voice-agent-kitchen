//! `TestRunRepo` on Postgres. A run is one row; its results are one JSONB array, written as a
//! whole on every step under the run's revision, so the worker and a cancel never overwrite
//! each other. An operator's change (start, cancel) commits with its audit event.

use jiff_sqlx::ToSqlx as _;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;
use vak_domain::{AgentId, AuditEvent, ContentHash, TestRun, TestRunId, Verdict};
use vak_repositories::{BoxFuture, RepoError, Revision, TestRunRepo, Versioned};

use crate::agents::{DEFAULT_WORKSPACE, insert_event, map_error, to_i64, upsert_operator};
use crate::records::corrupt;
use crate::scenario_records::{ModelColumns, ResultRecord, TestRunRow};

/// [`TestRunRepo`] on a Postgres pool.
#[derive(Debug, Clone)]
pub struct PgTestRunRepo {
    pool: PgPool,
}

impl PgTestRunRepo {
    /// The repository on `pool`. Production code gets one from `Database::test_runs`.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn create_run(&self, run: &TestRun, event: &AuditEvent) -> Result<Revision, RepoError> {
        let mut tx = self.pool.begin().await.map_err(map_error)?;
        let operator = upsert_operator(&mut tx, event.actor()).await?;
        let models = ModelColumns::of(run)?;
        sqlx::query!(
            "INSERT INTO test_runs (id, workspace_id, agent_id, created_by, draft_hash, status,
                                    verdict, llm_model, decision_model, seed, scenarios,
                                    revision, created_at, started_at, finished_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 1, $12, $13, $14)",
            run.id().as_uuid(),
            DEFAULT_WORKSPACE,
            run.agent_id().as_uuid(),
            operator,
            run.draft_hash().as_str(),
            run.status().as_str(),
            run.verdict().map(Verdict::as_str),
            models.llm,
            models.decisions,
            models.seed,
            ResultRecord::of_run(run) as _,
            run.created_at().to_sqlx() as _,
            run.started_at().map(jiff_sqlx::Timestamp::from) as _,
            run.finished_at().map(jiff_sqlx::Timestamp::from) as _,
        )
        .execute(&mut *tx)
        .await
        .map_err(map_error)?;
        insert_event(&mut tx, operator, event).await?;
        tx.commit().await.map_err(map_error)?;
        Ok(Revision::FIRST)
    }

    async fn get_run(&self, id: TestRunId) -> Result<Versioned<TestRun>, RepoError> {
        sqlx::query_as!(
            TestRunRow,
            r#"SELECT id, agent_id, draft_hash, status, verdict, llm_model, decision_model, seed,
                      scenarios AS "scenarios: _", revision,
                      created_at AS "created_at: jiff_sqlx::Timestamp",
                      started_at AS "started_at: jiff_sqlx::Timestamp",
                      finished_at AS "finished_at: jiff_sqlx::Timestamp"
               FROM test_runs
               WHERE id = $1 AND workspace_id = $2"#,
            id.as_uuid(),
            DEFAULT_WORKSPACE,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_error)?
        .ok_or(RepoError::NotFound)?
        .try_into()
    }

    async fn list_runs(&self, agent: AgentId, limit: u16) -> Result<Vec<TestRun>, RepoError> {
        let rows = sqlx::query_as!(
            TestRunRow,
            r#"SELECT id, agent_id, draft_hash, status, verdict, llm_model, decision_model, seed,
                      scenarios AS "scenarios: _", revision,
                      created_at AS "created_at: jiff_sqlx::Timestamp",
                      started_at AS "started_at: jiff_sqlx::Timestamp",
                      finished_at AS "finished_at: jiff_sqlx::Timestamp"
               FROM test_runs
               WHERE agent_id = $1 AND workspace_id = $2
               ORDER BY created_at DESC, id DESC
               LIMIT $3"#,
            agent.as_uuid(),
            DEFAULT_WORKSPACE,
            i64::from(limit),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_error)?;
        rows.into_iter()
            .map(|row| Versioned::<TestRun>::try_from(row).map(|stored| stored.value))
            .collect()
    }

    async fn update_run(
        &self,
        run: &TestRun,
        expected: Revision,
        event: Option<&AuditEvent>,
    ) -> Result<Revision, RepoError> {
        let mut tx = self.pool.begin().await.map_err(map_error)?;
        let models = ModelColumns::of(run)?;
        let revision = sqlx::query_scalar!(
            "UPDATE test_runs
             SET draft_hash = $3, status = $4, verdict = $5, llm_model = $6,
                 decision_model = $7, seed = $8, scenarios = $9, started_at = $10,
                 finished_at = $11, revision = revision + 1
             WHERE id = $1 AND workspace_id = $12 AND revision = $2
             RETURNING revision",
            run.id().as_uuid(),
            to_i64(expected)?,
            run.draft_hash().as_str(),
            run.status().as_str(),
            run.verdict().map(Verdict::as_str),
            models.llm,
            models.decisions,
            models.seed,
            ResultRecord::of_run(run) as _,
            run.started_at().map(jiff_sqlx::Timestamp::from) as _,
            run.finished_at().map(jiff_sqlx::Timestamp::from) as _,
            DEFAULT_WORKSPACE,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_error)?;
        let Some(revision) = revision else {
            return Err(missing_or_stale(&mut tx, run.id()).await);
        };
        if let Some(event) = event {
            let operator = upsert_operator(&mut tx, event.actor()).await?;
            insert_event(&mut tx, operator, event).await?;
        }
        tx.commit().await.map_err(map_error)?;
        u64::try_from(revision)
            .map(Revision::new)
            .map_err(RepoError::unavailable)
    }

    async fn next_queued_run(&self) -> Result<Option<Versioned<TestRun>>, RepoError> {
        sqlx::query_as!(
            TestRunRow,
            r#"SELECT id, agent_id, draft_hash, status, verdict, llm_model, decision_model, seed,
                      scenarios AS "scenarios: _", revision,
                      created_at AS "created_at: jiff_sqlx::Timestamp",
                      started_at AS "started_at: jiff_sqlx::Timestamp",
                      finished_at AS "finished_at: jiff_sqlx::Timestamp"
               FROM test_runs
               WHERE status = 'queued' AND workspace_id = $1
               ORDER BY created_at, id
               LIMIT 1"#,
            DEFAULT_WORKSPACE,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_error)?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn running_runs(&self) -> Result<Vec<Versioned<TestRun>>, RepoError> {
        let rows = sqlx::query_as!(
            TestRunRow,
            r#"SELECT id, agent_id, draft_hash, status, verdict, llm_model, decision_model, seed,
                      scenarios AS "scenarios: _", revision,
                      created_at AS "created_at: jiff_sqlx::Timestamp",
                      started_at AS "started_at: jiff_sqlx::Timestamp",
                      finished_at AS "finished_at: jiff_sqlx::Timestamp"
               FROM test_runs
               WHERE status = 'running' AND workspace_id = $1
               ORDER BY created_at, id"#,
            DEFAULT_WORKSPACE,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_error)?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    /// One query for every agent: the latest finished run per `(agent, draft hash)`.
    async fn verdicts(
        &self,
        drafts: &[(AgentId, ContentHash)],
    ) -> Result<Vec<(AgentId, Verdict)>, RepoError> {
        let (agents, hashes): (Vec<Uuid>, Vec<String>) = drafts
            .iter()
            .map(|(agent, hash)| (agent.as_uuid(), hash.as_str().to_owned()))
            .unzip();
        let rows = sqlx::query!(
            r#"SELECT DISTINCT ON (r.agent_id) r.agent_id, r.verdict AS "verdict!"
               FROM test_runs r
               JOIN unnest($1::uuid[], $2::text[]) AS d (agent_id, draft_hash)
                 ON r.agent_id = d.agent_id AND r.draft_hash = d.draft_hash
               WHERE r.status = 'finished' AND r.workspace_id = $3
               ORDER BY r.agent_id, r.created_at DESC, r.id DESC"#,
            &agents,
            &hashes,
            DEFAULT_WORKSPACE,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_error)?;
        rows.into_iter()
            .map(|row| {
                let agent = AgentId::from_uuid(row.agent_id).map_err(corrupt("agent_id"))?;
                let verdict = Verdict::parse(&row.verdict).ok_or_else(|| corrupt("verdict")(()))?;
                Ok((agent, verdict))
            })
            .collect()
    }
}

/// Why a conditional update matched no row: the run is gone, or its revision moved on.
async fn missing_or_stale(tx: &mut Transaction<'_, Postgres>, id: TestRunId) -> RepoError {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM test_runs WHERE id = $1 AND workspace_id = $2)
           AS "exists!""#,
        id.as_uuid(),
        DEFAULT_WORKSPACE,
    )
    .fetch_one(&mut **tx)
    .await;
    match exists {
        Ok(true) => RepoError::Conflict,
        Ok(false) => RepoError::NotFound,
        Err(error) => map_error(error),
    }
}

impl TestRunRepo for PgTestRunRepo {
    fn create<'a>(
        &'a self,
        run: &'a TestRun,
        event: &'a AuditEvent,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(self.create_run(run, event))
    }

    fn get(&self, id: TestRunId) -> BoxFuture<'_, Result<Versioned<TestRun>, RepoError>> {
        Box::pin(self.get_run(id))
    }

    fn list(&self, agent: AgentId, limit: u16) -> BoxFuture<'_, Result<Vec<TestRun>, RepoError>> {
        Box::pin(self.list_runs(agent, limit))
    }

    fn update<'a>(
        &'a self,
        run: &'a TestRun,
        expected: Revision,
        event: Option<&'a AuditEvent>,
    ) -> BoxFuture<'a, Result<Revision, RepoError>> {
        Box::pin(self.update_run(run, expected, event))
    }

    fn next_queued(&self) -> BoxFuture<'_, Result<Option<Versioned<TestRun>>, RepoError>> {
        Box::pin(self.next_queued_run())
    }

    fn running(&self) -> BoxFuture<'_, Result<Vec<Versioned<TestRun>>, RepoError>> {
        Box::pin(self.running_runs())
    }

    fn latest_verdicts<'a>(
        &'a self,
        drafts: &'a [(AgentId, ContentHash)],
    ) -> BoxFuture<'a, Result<Vec<(AgentId, Verdict)>, RepoError>> {
        Box::pin(self.verdicts(drafts))
    }
}
