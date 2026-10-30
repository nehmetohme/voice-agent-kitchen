//! Application use cases: agents, plans, versions and publishing, test runs,
//! sessions and audit. One public function per use case.
//!
//! Business rules come from `vak-domain`; the outside world is reached only
//! through repository and provider ports.

mod agents;

pub use agents::AgentService;
