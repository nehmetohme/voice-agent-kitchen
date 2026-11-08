//! Voice sessions: a live test call with an agent. The browser asks for a session
//! ([`VoiceService::start_session`]), then opens its media WebSocket, which the API bridges
//! to the pipeline ([`VoiceService::open_media`], [`MediaSession::run`]).
//!
//! Sessions live in memory: a ticket waits at most a minute for its socket, can be used
//! once, and only by the operator who asked for it. Persisting sessions and turns comes with
//! the test step (M5).

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::time::Instant;
use uuid::Uuid;
use vak_domain::{Actor, AgentId, SessionEndReason, SessionScript};
use vak_pipeline::{Engines, OptOutPolicy, SessionConfig, StageDeadlines, TurnPolicy, run_session};
use vak_providers::media::MediaTransport;

use crate::agents::{AgentService, FindAgentError};
use crate::latency::{LatencyStats, StageReport};

/// How a voice session is run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceSettings {
    /// How long a ticket waits for its media socket.
    pub ticket_ttl: Duration,
    /// Sessions at once: a local LLM box serves one request at a time.
    pub max_live_sessions: usize,
    /// How long each model call may take.
    pub deadlines: StageDeadlines,
    /// How long the agent's reply waits for the opt-out decision.
    pub decision_wait: Duration,
    /// How long the opt-out decision may take before it counts as timed out.
    pub decision_deadline: Duration,
    /// The longest session.
    pub max_duration: Duration,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            ticket_ttl: Duration::from_mins(1),
            max_live_sessions: 4,
            deadlines: StageDeadlines::default(),
            decision_wait: Duration::from_millis(300),
            decision_deadline: Duration::from_secs(2),
            max_duration: Duration::from_mins(30),
        }
    }
}

/// A session waiting for its media socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTicket {
    pub session_id: String,
    /// Where the browser opens the media WebSocket, relative to the API root.
    pub media_path: String,
}

/// Why [`VoiceService::start_session`] failed.
#[derive(Debug, thiserror::Error)]
pub enum StartSessionError {
    #[error("no such agent")]
    NotFound,
    #[error("the agent is archived")]
    Archived,
    #[error("voice sessions are not configured on this server")]
    VoiceUnavailable,
    #[error("too many voice sessions are running")]
    Busy,
    #[error("agent or purpose storage is unavailable")]
    Unavailable(#[source] vak_repositories::RepoError),
}

/// Why [`VoiceService::open_media`] failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OpenMediaError {
    /// No ticket by that ID for this operator, or it expired or was used.
    #[error("no such session")]
    NotFound,
    #[error("too many voice sessions are running")]
    Busy,
}

struct Ticket {
    owner: String,
    script: SessionScript,
    issued: Instant,
}

struct Inner {
    agents: AgentService,
    engines: Option<Engines>,
    settings: VoiceSettings,
    tickets: Mutex<HashMap<String, Ticket>>,
    live: AtomicUsize,
    latency: Arc<LatencyStats>,
}

/// The voice-session use cases.
#[derive(Clone)]
pub struct VoiceService {
    inner: Arc<Inner>,
}

impl VoiceService {
    /// `engines` is `None` when this server has no models configured: every session is then
    /// refused with [`StartSessionError::VoiceUnavailable`].
    #[must_use]
    pub fn new(agents: AgentService, engines: Option<Engines>) -> Self {
        Self::with_settings(agents, engines, VoiceSettings::default())
    }

    #[must_use]
    pub fn with_settings(
        agents: AgentService,
        engines: Option<Engines>,
        settings: VoiceSettings,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                agents,
                engines,
                settings,
                tickets: Mutex::new(HashMap::new()),
                live: AtomicUsize::new(0),
                latency: Arc::new(LatencyStats::default()),
            }),
        }
    }

    /// Prepares a live test call with `agent_id` for `actor`.
    pub async fn start_session(
        &self,
        actor: &Actor,
        agent_id: AgentId,
    ) -> Result<SessionTicket, StartSessionError> {
        if self.inner.engines.is_none() {
            return Err(StartSessionError::VoiceUnavailable);
        }
        let agent = self
            .inner
            .agents
            .get(agent_id)
            .await
            .map_err(|error| match error {
                FindAgentError::NotFound => StartSessionError::NotFound,
                FindAgentError::Unavailable(source) => StartSessionError::Unavailable(source),
            })?;
        if agent.is_archived() {
            return Err(StartSessionError::Archived);
        }
        if self.inner.live.load(Ordering::SeqCst) >= self.inner.settings.max_live_sessions {
            return Err(StartSessionError::Busy);
        }
        let purpose = self
            .inner
            .agents
            .pinned_purpose(agent.draft())
            .await
            .map_err(StartSessionError::Unavailable)?;
        let script = SessionScript::for_agent(agent.draft(), purpose.definition());
        let session_id = Uuid::now_v7().to_string();
        let mut tickets = self.tickets();
        let ttl = self.inner.settings.ticket_ttl;
        tickets.retain(|_, ticket| ticket.issued.elapsed() < ttl);
        tickets.insert(
            session_id.clone(),
            Ticket {
                owner: actor.user_id().to_owned(),
                script,
                issued: Instant::now(),
            },
        );
        Ok(SessionTicket {
            media_path: format!("/sessions/{session_id}/media"),
            session_id,
        })
    }

    /// Claims the session's ticket for its media socket: once, within the ticket's lifetime,
    /// by the operator who asked for it.
    pub fn open_media(
        &self,
        actor: &Actor,
        session_id: &str,
    ) -> Result<MediaSession, OpenMediaError> {
        let engines = self.inner.engines.clone().ok_or(OpenMediaError::NotFound)?;
        let ticket = {
            let mut tickets = self.tickets();
            let owned = tickets.get(session_id).is_some_and(|ticket| {
                ticket.owner == actor.user_id()
                    && ticket.issued.elapsed() < self.inner.settings.ticket_ttl
            });
            if !owned {
                return Err(OpenMediaError::NotFound);
            }
            tickets.remove(session_id).ok_or(OpenMediaError::NotFound)?
        };
        let live = LiveSession::claim(&self.inner)?;
        Ok(MediaSession {
            config: self.config(session_id, &ticket.script),
            engines,
            _live: live,
        })
    }

    /// The voice path's latency so far, per stage.
    #[must_use]
    pub fn latency(&self) -> Vec<StageReport> {
        self.inner.latency.report()
    }

    fn config(&self, session_id: &str, script: &SessionScript) -> SessionConfig {
        let settings = &self.inner.settings;
        let silence = Duration::from_millis(u64::from(script.end_of_turn_silence_ms()));
        let turns = if script.barge_in() {
            TurnPolicy::new(silence)
        } else {
            TurnPolicy::new(silence).without_barge_in()
        };
        SessionConfig::new(session_id, script.disclosure())
            .with_system_prompt(script.instructions())
            .with_voice(script.voice())
            .with_turn_policy(turns)
            .with_opt_out(
                OptOutPolicy::new(
                    script.opt_out_phrases().iter().cloned(),
                    script.opt_out_confirmation(),
                )
                .with_check_question(script.opt_out_check_question())
                .with_decision_wait(settings.decision_wait)
                .with_decision_deadline(settings.decision_deadline),
            )
            .with_deadlines(settings.deadlines)
            .with_max_duration(max_duration(settings.max_duration, script))
            .with_observer(self.inner.latency.clone())
    }

    fn tickets(&self) -> std::sync::MutexGuard<'_, HashMap<String, Ticket>> {
        self.inner
            .tickets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// The platform's cap, or the purpose's shorter call limit.
fn max_duration(cap: Duration, script: &SessionScript) -> Duration {
    cap.min(Duration::from_mins(u64::from(script.max_call_minutes())))
}

/// Counts a running session until it ends.
struct LiveSession {
    inner: Arc<Inner>,
}

impl LiveSession {
    fn claim(inner: &Arc<Inner>) -> Result<Self, OpenMediaError> {
        let max = inner.settings.max_live_sessions;
        inner
            .live
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |live| {
                (live < max).then_some(live + 1)
            })
            .map_err(|_| OpenMediaError::Busy)?;
        Ok(Self {
            inner: Arc::clone(inner),
        })
    }
}

impl Drop for LiveSession {
    fn drop(&mut self) {
        self.inner.live.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A claimed session, ready to run over its media socket.
pub struct MediaSession {
    config: SessionConfig,
    engines: Engines,
    _live: LiveSession,
}

impl MediaSession {
    /// Runs the conversation until it ends; returns why it ended.
    pub async fn run(self, transport: MediaTransport) -> SessionEndReason {
        let outcome = run_session(self.config, self.engines, transport).await;
        outcome.end_reason()
    }
}

#[cfg(test)]
mod tests;
