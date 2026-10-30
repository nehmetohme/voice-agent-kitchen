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
    ///
    /// - Provision is done for every draft: a draft can't exist without its required fields.
    /// - Plan is done when the plan validates.
    /// - Test is done when the latest passing run tested exactly this draft, so editing the
    ///   draft after a test clears the step until a new run passes.
    /// - Publish is done when the live version is exactly this draft.
    #[must_use]
    pub fn derive(draft: &AgentDraft, facts: &ProgressFacts) -> Self {
        let hash = draft.content_hash();
        let is_current = |tested: &Option<ContentHash>| tested.as_ref() == Some(&hash);
        let done = |step| match step {
            SetupStep::Provision => true,
            SetupStep::Plan => facts.plan_valid,
            SetupStep::Test => is_current(&facts.passing_test_run),
            SetupStep::Publish => is_current(&facts.live_version),
        };
        Self {
            steps: SetupStep::ALL.map(|step| StepProgress {
                step,
                done: done(step),
            }),
        }
    }

    /// Every step, in flow order.
    #[must_use]
    pub fn steps(&self) -> &[StepProgress] {
        &self.steps
    }

    /// 0, 25, 50, 75 or 100: 25 per completed step.
    #[must_use]
    pub fn percent(&self) -> u8 {
        let done = self.steps.iter().filter(|step| step.done).count();
        u8::try_from(done).unwrap_or(u8::MAX).saturating_mul(25)
    }

    /// The first step not done yet, or `None` when setup is complete.
    #[must_use]
    pub fn next_step(&self) -> Option<SetupStep> {
        self.steps
            .iter()
            .find(|step| !step.done)
            .map(|step| step.step)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::{AgentName, DraftPatch, UseCase, VoiceId};

    fn draft() -> AgentDraft {
        AgentDraft::new(
            AgentName::parse("Front desk").expect("valid"),
            UseCase::Support,
            VoiceId::parse("af_heart").expect("valid"),
        )
    }

    #[test]
    fn a_published_agent_is_complete() {
        let hash = draft().content_hash();
        let facts = ProgressFacts {
            plan_valid: true,
            passing_test_run: Some(hash.clone()),
            live_version: Some(hash),
        };

        let progress = SetupProgress::derive(&draft(), &facts);

        assert_eq!(progress.percent(), 100);
        assert_eq!(progress.next_step(), None);
    }

    #[test]
    fn editing_after_a_test_clears_test_and_publish() {
        let tested = draft().content_hash();
        let facts = ProgressFacts {
            plan_valid: true,
            passing_test_run: Some(tested.clone()),
            live_version: Some(tested),
        };
        let edited = draft().apply(&DraftPatch {
            name: Some(AgentName::parse("After hours").expect("valid")),
            ..DraftPatch::default()
        });

        let progress = SetupProgress::derive(&edited, &facts);

        assert_eq!(progress.percent(), 50);
        assert_eq!(progress.next_step(), Some(SetupStep::Test));
    }

    #[test]
    fn step_names_are_the_wire_names() {
        let names: Vec<&str> = SetupStep::ALL.iter().map(|step| step.as_str()).collect();

        assert_eq!(names, ["provision", "plan", "test", "publish"]);
    }

    proptest! {
        #[test]
        fn percent_is_25_per_done_step(plan_valid: bool, tested: bool, live: bool) {
            let hash = draft().content_hash();
            let facts = ProgressFacts {
                plan_valid,
                passing_test_run: tested.then(|| hash.clone()),
                live_version: live.then_some(hash),
            };

            let progress = SetupProgress::derive(&draft(), &facts);

            let done = 1 + u8::from(plan_valid) + u8::from(tested) + u8::from(live);
            prop_assert_eq!(progress.percent(), done * 25);
            prop_assert_eq!(progress.steps().len(), 4);
            prop_assert_eq!(
                progress.next_step(),
                progress.steps().iter().find(|step| !step.done()).map(StepProgress::step)
            );
        }
    }
}
