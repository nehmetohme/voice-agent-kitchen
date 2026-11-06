//! The worker: claims queued runs, plays their scenarios and stores every step.

use std::collections::VecDeque;
use std::future::Future;
use std::time::Duration;

use tokio::task::JoinSet;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use vak_domain::{
    Question, QuestionId, RubricCheck, RunModels, Scenario, ScenarioResult, SessionEndReason,
    SessionScript, TestRun, TestRunId, TranscriptLine,
};
use vak_pipeline::{
    LlmCaller, OptOutPolicy, TextDeadlines, TextEngines, TextSessionConfig, run_text_session,
};
use vak_providers::{DecisionEngine, DecisionRequest};
use vak_repositories::{RepoError, Revision, Versioned};

use super::{TestEngines, TestRunService, TestRunSettings, within};
use crate::FindAgentError;
use crate::agents::now;

/// How often the worker looks for queued runs when nothing woke it: runs queued by another
/// process (only this one queues today).
const POLL: Duration = Duration::from_secs(5);

/// Runs queued test runs. `vak-server` runs one in the background ([`TestRunWorker::run`]);
/// tests drive it with [`TestRunWorker::drain`].
#[derive(Clone)]
pub struct TestRunWorker {
    service: TestRunService,
}

/// Whether a run's stored state still belongs to the worker.
enum Saved {
    Kept(Revision),
    /// Someone else changed it: the operator cancelled it, or a recovery interrupted it.
    Lost,
}

impl TestRunWorker {
    pub(super) fn new(service: TestRunService) -> Self {
        Self { service }
    }

    /// Interrupts the runs a stopped server left running, then runs queued runs as they come,
    /// until `shutdown` resolves. A run still playing then is left running, and the next
    /// start interrupts it.
    pub async fn run(self, shutdown: impl Future<Output = ()>) {
        tokio::pin!(shutdown);
        let interrupted = self.recover().await;
        if interrupted > 0 {
            tracing::warn!(interrupted, "test runs left running by a stopped server");
        }
        loop {
            tokio::select! {
                biased;
                () = &mut shutdown => return,
                _ = self.drain() => {}
            }
            tokio::select! {
                biased;
                () = &mut shutdown => return,
                () = self.service.inner.queued.notified() => {}
                () = tokio::time::sleep(POLL) => {}
            }
        }
    }

    /// Runs queued runs, oldest first, until none is left; returns how many it ran. It stops
    /// early when storage fails, so a run it can't claim is never retried in a loop.
    pub async fn drain(&self) -> usize {
        let mut ran = 0;
        loop {
            match within(self.service.inner.runs.next_queued()).await {
                Ok(Some(stored)) => match self.execute(stored).await {
                    Ok(true) => ran += 1,
                    Ok(false) => {}
                    Err(error) => {
                        tracing::error!(%error, "the test run worker stopped: storage failed");
                        return ran;
                    }
                },
                Ok(None) => return ran,
                Err(error) => {
                    tracing::error!(%error, "the test run worker can't read the queue");
                    return ran;
                }
            }
        }
    }

    /// Interrupts every run left running by a server that stopped (none of this process's
    /// own); returns how many.
    pub async fn recover(&self) -> usize {
        let inner = &self.service.inner;
        let running = match within(inner.runs.running()).await {
            Ok(running) => running,
            Err(error) => {
                tracing::error!(%error, "can't recover the test runs left running");
                return 0;
            }
        };
        let mut interrupted = 0;
        for stored in running {
            if self.service.cancels().contains_key(&stored.value.id()) {
                continue;
            }
            if let Some(run) = stored.value.interrupt(now())
                && within(inner.runs.update(&run, stored.revision, None))
                    .await
                    .is_ok()
            {
                interrupted += 1;
            }
        }
        interrupted
    }

    /// Plays one queued run. `Ok(false)` when it didn't run: it was cancelled meanwhile, or it
    /// can't run (no models, the agent is gone or archived) and is interrupted.
    async fn execute(&self, stored: Versioned<TestRun>) -> Result<bool, RepoError> {
        let inner = &self.service.inner;
        let agent = match inner.agents.get(stored.value.agent_id()).await {
            Ok(agent) if !agent.is_archived() => Some(agent),
            Ok(_) | Err(FindAgentError::NotFound) => None,
            Err(FindAgentError::Unavailable(error)) => return Err(error),
        };
        let (Some(agent), Some(engines)) = (agent, inner.engines.clone()) else {
            tracing::warn!(test_run = %stored.value.id(), "a queued test run can't run");
            return self.interrupt(&stored).await.map(|()| false);
        };
        let models = RunModels::new(
            engines.llm_model.as_str(),
            engines.decision_model.as_str(),
            inner.settings.seed,
        );
        let Some(running) = stored.value.start(agent.draft(), models, now()) else {
            return Ok(false);
        };
        let revision = match self.save(&running, stored.revision).await? {
            Saved::Kept(revision) => revision,
            Saved::Lost => return Ok(false),
        };
        let id = running.id();
        tracing::info!(test_run = %id, scenarios = running.scenarios().len(), "test run started");
        let signal = CancelSignal::register(&self.service, id);
        let script = SessionScript::for_draft(agent.draft());
        self.play(running, revision, &script, &engines, &signal.token)
            .await
            .map(|()| true)
    }

    async fn interrupt(&self, stored: &Versioned<TestRun>) -> Result<(), RepoError> {
        let Some(run) = stored.value.interrupt(now()) else {
            return Ok(());
        };
        self.save(&run, stored.revision).await.map(|_| ())
    }

    /// Plays the run's scenarios, at most `scenario_concurrency` at once, storing each one as
    /// it starts and finishes, then the verdict. Returns as soon as the run is cancelled; the
    /// model calls in flight are dropped with their tasks.
    async fn play(
        &self,
        mut run: TestRun,
        mut revision: Revision,
        script: &SessionScript,
        engines: &TestEngines,
        cancel: &CancellationToken,
    ) -> Result<(), RepoError> {
        let settings = self.service.inner.settings;
        let mut queue: VecDeque<ScenarioResult> = run.scenarios().iter().cloned().collect();
        let mut playing = JoinSet::new();
        loop {
            while playing.len() < settings.scenario_concurrency.max(1)
                && let Some(next) = queue.pop_front()
            {
                let started = next.running();
                if let Some(updated) = run.record(started.clone()) {
                    run = updated;
                }
                match self.save(&run, revision).await? {
                    Saved::Kept(next_revision) => revision = next_revision,
                    Saved::Lost => return stop(&mut playing).await,
                }
                let scenario = Scenario::clone(started.scenario());
                playing.spawn(play_scenario(
                    started,
                    conversation(script, &scenario, settings),
                    engines.clone(),
                    settings,
                ));
            }
            let joined = tokio::select! {
                biased;
                () = cancel.cancelled() => return stop(&mut playing).await,
                joined = playing.join_next() => joined,
            };
            let Some(joined) = joined else { break };
            let Ok(result) = joined else {
                // A scenario task that panicked: `finish` reports its scenario as an error.
                tracing::error!(test_run = %run.id(), "a test scenario's task failed");
                continue;
            };
            tracing::info!(
                test_run = %run.id(),
                scenario = %result.scenario().id(),
                status = result.status().as_str(),
                "test scenario done"
            );
            if let Some(updated) = run.record(result) {
                run = updated;
            }
            match self.save(&run, revision).await? {
                Saved::Kept(next_revision) => revision = next_revision,
                Saved::Lost => return stop(&mut playing).await,
            }
        }
        if let Some(finished) = run.finish(now()) {
            tracing::info!(
                test_run = %finished.id(),
                verdict = finished.verdict().map_or("", |verdict| verdict.as_str()),
                "test run finished"
            );
            self.save(&finished, revision).await?;
        }
        Ok(())
    }

    /// Stores `run` over `revision`. A stale revision means someone else changed it.
    async fn save(&self, run: &TestRun, revision: Revision) -> Result<Saved, RepoError> {
        match within(self.service.inner.runs.update(run, revision, None)).await {
            Ok(next) => Ok(Saved::Kept(next)),
            Err(RepoError::Conflict) => Ok(Saved::Lost),
            Err(error) => Err(error),
        }
    }
}

/// A running run's cancel signal, registered with the service while the worker plays it and
/// removed when the play ends, however it ends (dropped with the worker's future included).
struct CancelSignal {
    service: TestRunService,
    id: TestRunId,
    token: CancellationToken,
}

impl CancelSignal {
    fn register(service: &TestRunService, id: TestRunId) -> Self {
        let token = CancellationToken::new();
        service.cancels().insert(id, token.clone());
        Self {
            service: service.clone(),
            id,
            token,
        }
    }
}

impl Drop for CancelSignal {
    fn drop(&mut self) {
        self.service.cancels().remove(&self.id);
    }
}

/// Aborts the scenarios in flight and waits until their tasks (and model calls) are gone.
async fn stop(playing: &mut JoinSet<ScenarioResult>) -> Result<(), RepoError> {
    playing.abort_all();
    while playing.join_next().await.is_some() {}
    Ok(())
}

/// A scenario's conversation: the agent's script against the scenario's caller.
struct Conversation {
    config: TextSessionConfig,
    caller_instructions: String,
    deadlines: TextDeadlines,
}

fn conversation(
    script: &SessionScript,
    scenario: &Scenario,
    settings: TestRunSettings,
) -> Conversation {
    let deadlines = TextDeadlines {
        first_token: settings.llm_first_token,
        reply: settings.llm_reply,
    };
    let opt_out = OptOutPolicy::new(
        script.opt_out_phrases().iter().copied(),
        script.opt_out_confirmation(),
    )
    .with_check_question(script.opt_out_check_question())
    .with_decision_deadline(settings.decision);
    Conversation {
        config: TextSessionConfig::new(script.disclosure(), script.instructions())
            .with_opt_out(opt_out)
            .with_deadlines(deadlines)
            .with_max_caller_turns(scenario.max_turns())
            .with_seed(settings.seed),
        caller_instructions: scenario.caller_instructions(),
        deadlines,
    }
}

/// Plays one scenario and scores its rubric.
async fn play_scenario(
    result: ScenarioResult,
    conversation: Conversation,
    engines: TestEngines,
    settings: TestRunSettings,
) -> ScenarioResult {
    let started = Instant::now();
    let text = TextEngines::new(engines.llm.clone(), engines.decisions.clone());
    let mut caller = LlmCaller::new(
        engines.llm.clone(),
        conversation.caller_instructions,
        conversation.deadlines,
        settings.seed,
    );
    let outcome = run_text_session(&conversation.config, &text, &mut caller).await;
    let end = outcome.end_reason();
    let transcript = outcome.into_transcript();
    let scenario = result.scenario();
    let checks = if end == SessionEndReason::Error {
        // Nothing to judge: the conversation didn't happen as written.
        unanswered(scenario)
    } else {
        score(
            scenario,
            &transcript,
            engines.decisions.as_ref(),
            settings.rubric,
        )
        .await
    };
    result.finished(end, transcript, checks, started.elapsed())
}

/// The rubric questions' IDs in the decision request: `rubric_1`, `rubric_2`…
fn rubric_questions(scenario: &Scenario) -> Vec<(QuestionId, Question)> {
    scenario
        .rubric()
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let id = QuestionId::parse(&format!("rubric_{}", i + 1)).ok()?;
            Some((id, Question::yes_no(item.question())))
        })
        .collect()
}

/// One decision request with every rubric question over the whole transcript. A request
/// that fails leaves every check without an answer.
async fn score(
    scenario: &Scenario,
    transcript: &[TranscriptLine],
    decisions: &dyn DecisionEngine,
    deadline: Duration,
) -> Vec<RubricCheck> {
    let questions = rubric_questions(scenario);
    let request = transcript
        .iter()
        .fold(DecisionRequest::new(deadline), |request, line| {
            request.with_turn(line.speaker(), line.text())
        });
    let request = questions.iter().fold(request, |request, (id, question)| {
        request.with_question(id.clone(), question.clone())
    });
    let answers = tokio::time::timeout(deadline, decisions.decide(&request))
        .await
        .unwrap_or(Err(vak_providers::ProviderError::Timeout));
    let answers = match answers {
        Ok(answers) => Some(answers),
        Err(error) => {
            tracing::warn!(scenario = %scenario.id(), %error, "a test scenario's rubric got no answer");
            None
        }
    };
    scenario
        .rubric()
        .iter()
        .zip(&questions)
        .map(|(item, (id, _))| {
            let probability =
                answers
                    .as_ref()
                    .and_then(|answers| match answers.answer(id.as_str()) {
                        Some(vak_domain::Answer::YesNo { probability }) => Some(*probability),
                        _ => None,
                    });
            RubricCheck::score(item, probability)
        })
        .collect()
}

fn unanswered(scenario: &Scenario) -> Vec<RubricCheck> {
    scenario
        .rubric()
        .iter()
        .map(|item| RubricCheck::score(item, None))
        .collect()
}
