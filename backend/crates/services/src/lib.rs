//! Application use cases: agents, plans, versions and publishing, test runs,
//! sessions and audit. One public function per use case.
//!
//! Business rules come from `vak-domain`; the outside world is reached only
//! through repository and provider ports.

mod agent_purpose;
mod agents;
pub mod catalog;
pub mod latency;
mod plans;
mod purposes;
pub mod rule_packs;
mod scenarios;
mod test_runs;
mod voice;

pub use agent_purpose::{AgentPurpose, UpdateAgentPurpose, UpdateAgentPurposeError};
pub use agents::{
    AgentService, AgentSummary, ArchiveAgentError, CreateAgent, CreateAgentError, FindAgentError,
    LIST_LIMIT, ListAgentsError, UpdateAgent, UpdateDraftError,
};
pub use purposes::{
    ComplianceBaseline, DuplicatePurpose, DuplicatePurposeError, FindPurposeError,
    PURPOSE_LIST_LIMIT, PurposeService, PurposeSummary, PurposeView, SavePurposeVersionError,
    ToolCatalog, UpdatePurposeError,
};
pub use scenarios::AgentScenarios;
pub use test_runs::{
    AgentTestRuns, CancelTestRunError, FindTestRunError, RUNS_LISTED, StartTestRunError,
    TestEngines, TestRunService, TestRunSettings, TestRunWorker,
};
pub use voice::{
    MediaSession, OpenMediaError, SessionTicket, StartSessionError, VoiceService, VoiceSettings,
};

/// The voice engines a server wires into [`VoiceService`].
pub use vak_pipeline::{Engines, StageDeadlines};

/// The media transport the API bridges a WebSocket to: the `vak.media.v1` events and the
/// channel pair between the socket and a session.
pub mod media {
    pub use vak_providers::media::{
        InputFramer, MediaPeer, MediaTransport, Outbound, PROTOCOL, PeerSender, PushAudio,
        ServerEvent, media_channel,
    };
}
