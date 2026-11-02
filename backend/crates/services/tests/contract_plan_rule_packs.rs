//! Contract `plan` v1 (services): the rule packs the product ships, and the templates on them.
//!
//! - Every file in `contracts/rule-packs/` is one pack named `<id>.v<version>.toml`, and the
//!   shipped catalog holds exactly those packs. A published pack never changes: this contract
//!   pins the version 1 files, and a change is a new file with the next version.
//! - The loader refuses a file whose name disagrees with its content, unknown fields, broken
//!   TOML, and anything the domain refuses.
//! - Version 1 covers every use case in the US (`US`) and California (`US-CA`).
//! - Every pack opens with the AI disclosure, then the recording notice. The outbound use
//!   cases (telemarketing, survey) also say how to opt out, and "stop" opts out. Every pack
//!   has an opt-out confirmation (said when the caller opts out, before the call ends), names
//!   claims the agent must never make, and keeps audit records for at least 5 years.
//! - Every use case's template is valid against every shipped pack.
//!
//! Contract file: never edit it once it is tagged `contract/plan/v1`. To change the contract,
//! add a new version.
#![expect(
    clippy::panic,
    reason = "the file-reading helper outside #[test] functions fails the test on an I/O error"
)]

use std::fs;
use std::path::PathBuf;

use vak_domain::{Jurisdiction, LockedText, Plan, RulePackRef, UseCase};
use vak_services::rule_packs::{parse, shipped};

/// `(file name, content)` of every `*.toml` in `contracts/rule-packs/`, by name.
fn pack_files() -> Vec<(String, String)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/rule-packs");
    let mut files: Vec<(String, String)> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
                .path()
        })
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .map(|path| {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned());
            let text = fs::read_to_string(&path);
            match (name, text) {
                (Some(name), Ok(text)) => (name, text),
                (_, text) => panic!("{}: {text:?}", path.display()),
            }
        })
        .collect();
    files.sort();
    files
}

#[test]
fn every_rule_pack_file_loads_and_the_catalog_ships_exactly_those() {
    let catalog = shipped().expect("the shipped rule packs load");
    let files = pack_files();

    assert_ne!(files.len(), 0, "contracts/rule-packs/ has packs");
    for (name, text) in &files {
        let pack = match parse(name, text) {
            Ok(pack) => pack,
            Err(error) => panic!("{name}: {error}"),
        };
        let reference = pack.reference();
        assert_eq!(
            name,
            &format!("{}.v{}.toml", reference.id(), reference.version())
        );
        assert_eq!(catalog.get(reference), Some(&pack), "{name}");
    }
    assert_eq!(catalog.len(), files.len());
}

#[test]
fn the_loader_refuses_a_file_that_disagrees_with_its_name_or_the_rules() {
    let (name, text) = pack_files()
        .into_iter()
        .find(|(name, _)| name == "us-support.v1.toml")
        .expect("us-support v1 ships");

    assert!(parse(&name, &text).is_ok());
    for wrong_name in [
        "us-support.v2.toml",
        "us-booking.v1.toml",
        "us-support.toml",
        "us-support.v1.json",
    ] {
        assert!(parse(wrong_name, &text).is_err(), "{wrong_name}");
    }
    assert!(
        parse(&name, &format!("surprise = true\n{text}")).is_err(),
        "unknown field"
    );
    assert!(
        parse(&name, "id = \"us-support\"\nversion = ").is_err(),
        "broken TOML"
    );
    assert!(
        parse(&name, &text.replace("ai_disclosure", "greeting")).is_err(),
        "no AI disclosure"
    );
    let unconfirmed = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("opt_out_confirmation"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(
        unconfirmed, text,
        "the pack has an opt-out confirmation line"
    );
    assert!(
        parse(&name, &unconfirmed).is_err(),
        "no opt-out confirmation"
    );
}

#[test]
fn version_1_covers_every_use_case_in_the_us_and_california() {
    let catalog = shipped().expect("the shipped rule packs load");

    for code in ["US", "US-CA"] {
        let jurisdiction = Jurisdiction::parse(code).expect("valid jurisdiction");
        for use_case in UseCase::ALL {
            let reference = RulePackRef::for_agent(use_case, &jurisdiction);
            let pack = catalog
                .get(&reference)
                .unwrap_or_else(|| panic!("{} v1 ships", reference.id()));
            assert_eq!(reference.version(), 1);
            assert_eq!(pack.jurisdiction(), &jurisdiction);
            assert_eq!(pack.use_case(), use_case);
        }
    }
}

#[test]
fn every_pack_opens_with_the_ai_disclosure_then_the_recording_notice() {
    let catalog = shipped().expect("the shipped rule packs load");

    for pack in catalog.iter() {
        let id = pack.reference().id();
        let keys: Vec<&str> = pack.locked_texts().iter().map(LockedText::key).collect();
        assert_eq!(
            keys.get(..2),
            Some([LockedText::AI_DISCLOSURE, LockedText::RECORDING_NOTICE].as_slice()),
            "{id}"
        );
        let outbound = matches!(pack.use_case(), UseCase::Telemarketing | UseCase::Survey);
        if outbound {
            assert!(
                keys.contains(&"opt_out_notice"),
                "{id}: says how to opt out"
            );
            assert!(
                pack.opt_out_phrases().iter().any(|phrase| phrase == "stop"),
                "{id}: 'stop' opts out"
            );
        }
        assert!(!pack.opt_out_phrases().is_empty(), "{id}");
        assert!(!pack.forbidden_claims().is_empty(), "{id}");
        assert!(
            !pack.opt_out_confirmation().trim().is_empty(),
            "{id}: confirms an opt-out"
        );
        assert!(
            pack.retention().audit_days() >= 1_826,
            "{id}: 5 years of audit"
        );
    }
}

#[test]
fn every_template_is_valid_against_every_shipped_pack() {
    let catalog = shipped().expect("the shipped rule packs load");

    for pack in catalog.iter() {
        for use_case in UseCase::ALL {
            let template = Plan::template(use_case, pack.locked_texts());
            assert_eq!(
                catalog.validate(&template, pack.reference()),
                Ok(()),
                "the {use_case} template on {}",
                pack.reference().id()
            );
        }
    }
}
