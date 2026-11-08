//! Contract `purposes` v1 (services): the system purposes against the rule packs the product
//! ships.
//!
//! - For every shipped pack, the system purpose of its use case adds nothing: the effective
//!   policy is exactly the pack (locked texts, opt-out phrases and confirmation, forbidden
//!   claims, retention).
//! - Its starting flow, opened with the pack's locked texts, is the pack's template and is
//!   valid against the pack.
//! - Every shipped pack's jurisdiction has a pack for every compliance class, so any purpose
//!   can be combined with any shipped jurisdiction.
//!
//! Contract file: never edit it once it is tagged `contract/purposes/v1`. To change the
//! contract, add a new version.

use vak_domain::{EffectiveCompliance, Plan, PurposeVersion, RulePackRef, UseCase};
use vak_services::rule_packs::shipped;

#[test]
fn every_system_purpose_is_exactly_its_shipped_pack() {
    let catalog = shipped().expect("the shipped rule packs load");

    assert_ne!(catalog.len(), 0);
    for pack in catalog.iter() {
        let system = PurposeVersion::system(pack.use_case());
        let definition = system.definition();

        let effective = EffectiveCompliance::of(pack, definition.additions());

        let id = pack.reference().id();
        assert_eq!(effective.rule_pack(), pack.reference(), "{id}");
        assert_eq!(effective.locked_texts(), pack.locked_texts(), "{id}");
        assert_eq!(effective.opt_out_phrases(), pack.opt_out_phrases(), "{id}");
        assert_eq!(
            effective.opt_out_confirmation(),
            pack.opt_out_confirmation(),
            "{id}"
        );
        assert_eq!(
            effective.forbidden_claims(),
            pack.forbidden_claims(),
            "{id}"
        );
        assert_eq!(effective.retention(), pack.retention(), "{id}");

        let opened = definition.opened_flow(pack.locked_texts());
        assert_eq!(
            opened,
            Plan::template(pack.use_case(), pack.locked_texts()),
            "{id}"
        );
        assert_eq!(catalog.validate(&opened, pack.reference()), Ok(()), "{id}");
    }
}

#[test]
fn every_shipped_jurisdiction_covers_every_compliance_class() {
    let catalog = shipped().expect("the shipped rule packs load");

    for pack in catalog.iter() {
        for class in UseCase::ALL {
            let reference = RulePackRef::for_agent(class, pack.jurisdiction());
            assert!(
                catalog.get(&reference).is_some(),
                "{} has no {class} pack",
                pack.jurisdiction()
            );
        }
    }
}
