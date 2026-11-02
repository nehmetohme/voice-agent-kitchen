//! The session actor.

use vak_domain::SessionEndReason;
use vak_providers::media::MediaTransport;

use crate::{Engines, SessionConfig, SessionOutcome};

/// Runs one voice session until the client ends it, an opt-out ends it, or the transport
/// closes, and reports how it went.
pub async fn run_session(
    config: SessionConfig,
    engines: Engines,
    transport: MediaTransport,
) -> SessionOutcome {
    // CONTRACT-STUB
    drop((config, engines, transport));
    SessionOutcome {
        end_reason: SessionEndReason::Error,
        turns: Vec::new(),
    }
}
