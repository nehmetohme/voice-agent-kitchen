//! Setup progress (Zeigarnik): four steps, derived from state and never stored, so it can't
//! drift from what the agent really is.

use crate::{AgentDraft, ContentHash};

/// A step of the setup flow, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SetupStep {
    /// Required fields are set. True for every created agent.
    Provision,
    /// The plan validates (M3).
    Plan,
    /// A test run passed on the current draft hash (M5).
    Test,
    /// The live version is the current draft hash (M6).
    Publish,
}

impl SetupStep {
    /// Every step, in flow order.
    pub const ALL: [Self; 4] = [Self::Provision, Self::Plan, Self::Test, Self::Publish];

    /// The wire name, e.g. `provision`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Provision => "provision",
            Self::Plan => "plan",
            Self::Test => "test",
            Self::Publish => "publish",
        }
    }
}

/// What the other steps know about the agent. Later milestones fill these in; the default
/// says nothing has happened yet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgressFacts {
    /// The plan validates.
    pub plan_valid: bool,
    /// The draft hash of the latest passing test run, if any.
    pub passing_test_run: Option<ContentHash>,
    /// The draft hash of the live version, if any.
    pub live_version: Option<ContentHash>,
}

/// One step and whether it is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepProgress {
    step: SetupStep,
    done: bool,
}

impl StepProgress {
    #[must_use]
    pub fn step(&self) -> SetupStep {
        self.step
    }

    #[must_use]
    pub fn done(&self) -> bool {
        self.done
    }
}

/// The four steps and how far the agent is: 25 % per completed step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupProgress {
    steps: [StepProgress; 4],
}

impl SetupProgress {
    /// Derives progress from the draft and what later steps know about it.
    #[must_use]
    pub fn derive(draft: &AgentDraft, facts: &ProgressFacts) -> Self {
        // CONTRACT-STUB
        let _ = (draft, facts);
        Self {
            steps: SetupStep::ALL.map(|step| StepProgress { step, done: false }),
        }
    }

    /// Every step, in flow order.
    #[must_use]
    pub fn steps(&self) -> &[StepProgress] {
        &self.steps
    }

    /// 0, 25, 50, 75 or 100.
    #[must_use]
    pub fn percent(&self) -> u8 {
        // CONTRACT-STUB
        0
    }

    /// The first step not done yet, or `None` when setup is complete.
    #[must_use]
    pub fn next_step(&self) -> Option<SetupStep> {
        // CONTRACT-STUB
        None
    }
}
