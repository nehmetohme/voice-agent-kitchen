//! Application use cases: agents, plans, versions and publishing, test runs,
//! sessions and audit. One public function per use case.
//!
//! Business rules come from `vak-domain`; the outside world is reached only
//! through repository and provider ports.

mod agents;
pub mod catalog;
pub mod latency;
mod plans;
pub mod rule_packs;
mod voice;

pub use agents::{
    AgentService, AgentSummary, ArchiveAgentError, CreateAgent, CreateAgentError, FindAgentError,
    LIST_LIMIT, ListAgentsError, UpdateDraftError,
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
