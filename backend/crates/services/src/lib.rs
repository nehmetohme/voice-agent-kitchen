//! Application use cases: agents, plans, versions and publishing, test runs,
//! sessions and audit. One public function per use case.
//!
//! Business rules come from `vak-domain`; the outside world is reached only
//! through repository and provider ports.

mod agents;
pub mod catalog;

pub use agents::{
    AgentService, AgentSummary, ArchiveAgentError, CreateAgent, CreateAgentError, FindAgentError,
    LIST_LIMIT, ListAgentsError, UpdateDraftError,
};
