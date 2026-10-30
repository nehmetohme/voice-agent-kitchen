//! Agent use cases: create, list, get, update the draft (autosave), archive, progress.

use std::sync::Arc;

use vak_repositories::AgentRepo;

/// The agent use cases, over an [`AgentRepo`].
#[derive(Clone)]
pub struct AgentService {
    // CONTRACT-STUB
    _repo: Arc<dyn AgentRepo>,
}

impl AgentService {
    #[must_use]
    pub fn new(repo: Arc<dyn AgentRepo>) -> Self {
        Self { _repo: repo }
    }
}
