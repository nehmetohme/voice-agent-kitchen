//! The six built-in system purposes: today's use cases, at version 1, with today's templates,
//! instructions and packs, and fixed IDs.

use super::{
    ComplianceAdditions, Guardrails, PurposeDefinition, PurposeName, PurposeSpec, PurposeTools,
};
use crate::{Plan, PurposeId, UseCase};

/// `019a0000-0000-7000-8000-000000000000`; the system purposes are `+ 1` to `+ 6`, in
/// [`UseCase::ALL`] order.
const BASE: u128 = 0x019a_0000_0000_7000_8000_0000_0000_0000;

const fn number(use_case: UseCase) -> u128 {
    match use_case {
        UseCase::Support => 1,
        UseCase::Service => 2,
        UseCase::Booking => 3,
        UseCase::Survey => 4,
        UseCase::Telemarketing => 5,
        UseCase::Custom => 6,
    }
}

pub(super) const fn id(use_case: UseCase) -> PurposeId {
    PurposeId::from_v7_bits(BASE + number(use_case))
}

pub(super) fn use_case_of(id: PurposeId) -> Option<UseCase> {
    UseCase::ALL
        .into_iter()
        .find(|use_case| self::id(*use_case) == id)
}

/// The picker label, as the agent forms show it today.
pub(super) const fn label(use_case: UseCase) -> &'static str {
    match use_case {
        UseCase::Support => "Customer support",
        UseCase::Service => "Customer service",
        UseCase::Booking => "Appointment booking",
        UseCase::Survey => "Survey",
        UseCase::Telemarketing => "Outbound sales",
        UseCase::Custom => "Something else",
    }
}

pub(super) fn definition(use_case: UseCase) -> PurposeDefinition {
    PurposeDefinition::from_parts(PurposeSpec {
        name: PurposeName::fixed(label(use_case)),
        description: String::new(),
        compliance: use_case,
        persona: format!("Your job: {}.", job(use_case)),
        flow: Plan::template(use_case, &[]),
        tools: PurposeTools::default(),
        guardrails: Guardrails::default(),
        additions: ComplianceAdditions::default(),
    })
}

/// What an agent of the use case does, as its instructions said before purposes existed.
const fn job(use_case: UseCase) -> &'static str {
    match use_case {
        UseCase::Support => "answer customers' questions and help solve their problems",
        UseCase::Service => "handle account and order requests: changes, status and billing",
        UseCase::Booking => "book, move and cancel appointments",
        UseCase::Survey => "ask the survey's questions one at a time and note the answers",
        UseCase::Telemarketing => {
            "present the offer briefly and honestly, and respect a no at once"
        }
        UseCase::Custom => "help the caller with what they need",
    }
}
