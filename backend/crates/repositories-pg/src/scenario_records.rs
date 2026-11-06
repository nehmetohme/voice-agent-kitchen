//! JSONB shapes of scenarios (`agent_drafts.scenarios`) and test runs (`test_runs.scenarios`).
//! Every value read back goes through the domain's constructors, so a row the domain would
//! reject never becomes a `ScenarioSet` or a `TestRun`.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;
use vak_domain::{
    AgentId, ContentHash, OptOutLayer, RubricCheck, RubricItem, RunModels, Scenario, ScenarioId,
    ScenarioResult, ScenarioSet, ScenarioSpec, ScenarioStatus, SessionEndReason, Speaker, TestRun,
    TestRunId, TestRunParts, TestRunStatus, TranscriptLine, Verdict,
};
use vak_repositories::{RepoError, Revision, Versioned};

use crate::records::corrupt;

/// One scenario, as the draft saves it and a run records it.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ScenarioRecord {
    id: String,
    name: String,
    goal: String,
    persona: String,
    required: bool,
    max_turns: u8,
    rubric: Vec<RubricRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RubricRecord {
    question: String,
    expected: bool,
}

impl ScenarioRecord {
    fn of(scenario: &Scenario) -> Self {
        Self {
            id: scenario.id().as_str().to_owned(),
            name: scenario.name().to_owned(),
            goal: scenario.goal().to_owned(),
            persona: scenario.persona().to_owned(),
            required: scenario.required(),
            max_turns: scenario.max_turns(),
            rubric: scenario
                .rubric()
                .iter()
                .map(|item| RubricRecord {
                    question: item.question().to_owned(),
                    expected: item.expected(),
                })
                .collect(),
        }
    }

    /// The scenario, through the domain's constructors.
    fn into_scenario(self) -> Result<Scenario, RepoError> {
        let rubric = self
            .rubric
            .into_iter()
            .map(|item| {
                RubricItem::new(&item.question, item.expected).map_err(corrupt("scenario.rubric"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Scenario::new(ScenarioSpec {
            id: ScenarioId::parse(&self.id).map_err(corrupt("scenario.id"))?,
            name: self.name,
            goal: self.goal,
            persona: self.persona,
            required: self.required,
            max_turns: self.max_turns,
            rubric,
        })
        .map_err(corrupt("scenario"))
    }

    /// `agent_drafts.scenarios`: `NULL` until the operator saves some.
    pub(crate) fn of_draft(set: Option<&ScenarioSet>) -> Option<Json<Vec<Self>>> {
        set.map(|set| Json(set.scenarios().iter().map(Self::of).collect()))
    }

    /// The draft's saved scenarios, as a checked set.
    pub(crate) fn into_set(records: Vec<Self>) -> Result<ScenarioSet, RepoError> {
        let scenarios = records
            .into_iter()
            .map(Self::into_scenario)
            .collect::<Result<Vec<_>, _>>()?;
        ScenarioSet::new(scenarios).map_err(corrupt("scenarios"))
    }
}

/// One scenario of a run and how it went.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ResultRecord {
    scenario: ScenarioRecord,
    status: String,
    end_reason: Option<String>,
    transcript: Vec<LineRecord>,
    /// The checks, in rubric order; `passed` is derived again on read.
    checks: Vec<CheckRecord>,
    duration_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LineRecord {
    speaker: String,
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckRecord {
    question: String,
    expected: bool,
    probability: Option<f64>,
}

/// How a scenario's conversation ended, as stored.
fn end_name(end: SessionEndReason) -> &'static str {
    match end {
        SessionEndReason::ClientEnded => "client_ended",
        SessionEndReason::OptOut(OptOutLayer::Phrase) => "opt_out_phrase",
        SessionEndReason::OptOut(OptOutLayer::Decision) => "opt_out_decision",
        SessionEndReason::OptOut(OptOutLayer::LlmTool) => "opt_out_llm_tool",
        SessionEndReason::Escalation => "escalation",
        SessionEndReason::EndNode => "end_node",
        SessionEndReason::Timeout => "timeout",
        SessionEndReason::Error => "error",
    }
}

fn parse_end(raw: &str) -> Option<SessionEndReason> {
    [
        SessionEndReason::ClientEnded,
        SessionEndReason::OptOut(OptOutLayer::Phrase),
        SessionEndReason::OptOut(OptOutLayer::Decision),
        SessionEndReason::OptOut(OptOutLayer::LlmTool),
        SessionEndReason::Escalation,
        SessionEndReason::EndNode,
        SessionEndReason::Timeout,
        SessionEndReason::Error,
    ]
    .into_iter()
    .find(|end| end_name(*end) == raw)
}

impl ResultRecord {
    fn of(result: &ScenarioResult) -> Self {
        Self {
            scenario: ScenarioRecord::of(result.scenario()),
            status: result.status().as_str().to_owned(),
            end_reason: result.end_reason().map(|end| end_name(end).to_owned()),
            transcript: result
                .transcript()
                .iter()
                .map(|line| LineRecord {
                    speaker: line.speaker().as_str().to_owned(),
                    text: line.text().to_owned(),
                })
                .collect(),
            checks: result
                .checks()
                .iter()
                .map(|check| CheckRecord {
                    question: check.question().to_owned(),
                    expected: check.expected(),
                    probability: check.probability(),
                })
                .collect(),
            duration_ms: result
                .duration()
                .and_then(|duration| u64::try_from(duration.as_millis()).ok()),
        }
    }

    fn into_result(self) -> Result<ScenarioResult, RepoError> {
        let scenario = self.scenario.into_scenario()?;
        let status =
            ScenarioStatus::parse(&self.status).ok_or_else(|| corrupt("result.status")(()))?;
        let end_reason = self
            .end_reason
            .map(|raw| parse_end(&raw).ok_or_else(|| corrupt("result.end_reason")(())))
            .transpose()?;
        let transcript = self
            .transcript
            .into_iter()
            .map(|line| {
                let speaker = Speaker::parse(&line.speaker)
                    .ok_or_else(|| corrupt("result.transcript.speaker")(()))?;
                Ok(TranscriptLine::new(speaker, line.text))
            })
            .collect::<Result<Vec<_>, RepoError>>()?;
        let checks = self
            .checks
            .into_iter()
            .map(|check| {
                let item = RubricItem::new(&check.question, check.expected)
                    .map_err(corrupt("result.checks"))?;
                Ok(RubricCheck::score(&item, check.probability))
            })
            .collect::<Result<Vec<_>, RepoError>>()?;
        Ok(ScenarioResult::restore(
            scenario,
            status,
            end_reason,
            transcript,
            checks,
            self.duration_ms.map(Duration::from_millis),
        ))
    }

    /// `test_runs.scenarios`.
    pub(crate) fn of_run(run: &TestRun) -> Json<Vec<Self>> {
        Json(run.scenarios().iter().map(Self::of).collect())
    }
}

/// The stored model columns of a run: all three or none.
pub(crate) struct ModelColumns {
    pub(crate) llm: Option<String>,
    pub(crate) decisions: Option<String>,
    pub(crate) seed: Option<i64>,
}

impl ModelColumns {
    pub(crate) fn of(run: &TestRun) -> Result<Self, RepoError> {
        let Some(models) = run.models() else {
            return Ok(Self {
                llm: None,
                decisions: None,
                seed: None,
            });
        };
        Ok(Self {
            llm: Some(models.llm().to_owned()),
            decisions: Some(models.decisions().to_owned()),
            seed: Some(i64::try_from(models.seed()).map_err(RepoError::unavailable)?),
        })
    }
}

/// One `test_runs` row.
#[derive(Debug)]
pub(crate) struct TestRunRow {
    pub(crate) id: Uuid,
    pub(crate) agent_id: Uuid,
    pub(crate) draft_hash: String,
    pub(crate) status: String,
    pub(crate) verdict: Option<String>,
    pub(crate) llm_model: Option<String>,
    pub(crate) decision_model: Option<String>,
    pub(crate) seed: Option<i64>,
    pub(crate) scenarios: Json<Vec<ResultRecord>>,
    pub(crate) revision: i64,
    pub(crate) created_at: jiff_sqlx::Timestamp,
    pub(crate) started_at: Option<jiff_sqlx::Timestamp>,
    pub(crate) finished_at: Option<jiff_sqlx::Timestamp>,
}

impl TryFrom<TestRunRow> for Versioned<TestRun> {
    type Error = RepoError;

    fn try_from(row: TestRunRow) -> Result<Self, RepoError> {
        let models = match (row.llm_model, row.decision_model, row.seed) {
            (Some(llm), Some(decisions), Some(seed)) => Some(RunModels::new(
                llm,
                decisions,
                u64::try_from(seed).map_err(corrupt("seed"))?,
            )),
            (None, None, None) => None,
            _ => return Err(corrupt("models")(())),
        };
        let Json(scenarios) = row.scenarios;
        let scenarios = scenarios
            .into_iter()
            .map(ResultRecord::into_result)
            .collect::<Result<Vec<_>, _>>()?;
        let run = TestRun::restore(TestRunParts {
            id: TestRunId::from_uuid(row.id).map_err(corrupt("id"))?,
            agent_id: AgentId::from_uuid(row.agent_id).map_err(corrupt("agent_id"))?,
            draft_hash: ContentHash::parse(&row.draft_hash).map_err(corrupt("draft_hash"))?,
            status: TestRunStatus::parse(&row.status).ok_or_else(|| corrupt("status")(()))?,
            models,
            scenarios,
            created_at: row.created_at.to_jiff(),
            started_at: row.started_at.map(jiff_sqlx::Timestamp::to_jiff),
            finished_at: row.finished_at.map(jiff_sqlx::Timestamp::to_jiff),
        });
        // The stored verdict must be the one the domain derives from the results.
        let verdict = row
            .verdict
            .map(|raw| Verdict::parse(&raw).ok_or_else(|| corrupt("verdict")(())))
            .transpose()?;
        if verdict != run.verdict() {
            return Err(corrupt("verdict")(()));
        }
        let revision = u64::try_from(row.revision).map_err(corrupt("revision"))?;
        Ok(Self {
            value: run,
            revision: Revision::new(revision),
        })
    }
}
