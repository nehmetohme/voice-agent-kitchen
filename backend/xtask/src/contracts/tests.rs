use std::cell::RefCell;

use super::*;

/// The globs the real manifest uses.
const GLOBS: [&str; 6] = [
    "**/tests/contract_*.rs",
    "**/tests/contract_*/**",
    "**/*.contract.test.ts",
    "**/*.contract.test.tsx",
    "**/*.contract.spec.ts",
    "frontend/e2e/contract/**",
];

fn any_glob(path: &str) -> bool {
    GLOBS.iter().any(|glob| glob_matches(glob, path))
}

#[test]
fn double_star_matches_zero_or_more_folders() {
    assert!(glob_matches(
        "**/tests/contract_*.rs",
        "tests/contract_a.rs"
    ));
    assert!(glob_matches(
        "**/tests/contract_*.rs",
        "backend/crates/api/tests/contract_health.rs"
    ));
    assert!(glob_matches("a/**/b.rs", "a/b.rs"));
    assert!(glob_matches("a/**/b.rs", "a/x/y/b.rs"));
    assert!(!glob_matches("a/**/b.rs", "x/a/b.rs"));
}

#[test]
fn single_star_and_question_mark_stay_inside_one_segment() {
    assert!(glob_matches("src/*.ts", "src/a.ts"));
    assert!(!glob_matches("src/*.ts", "src/x/a.ts"));
    assert!(glob_matches("v?.rs", "v1.rs"));
    assert!(!glob_matches("v?.rs", "v10.rs"));
    assert!(!glob_matches("v?.rs", "v/.rs"));
}

#[test]
fn a_folder_pattern_covers_everything_below_it() {
    assert!(glob_matches(
        "frontend/e2e/contract/app-shell",
        "frontend/e2e/contract/app-shell/app-shell.contract.spec.ts"
    ));
    assert!(glob_matches(
        "frontend/e2e/contract/**",
        "frontend/e2e/contract/app-shell/a/b.ts"
    ));
    assert!(!glob_matches(
        "frontend/e2e/contract/app-shell",
        "frontend/e2e/contract/app-shell-2/x.ts"
    ));
}

#[test]
fn leading_slashes_and_empty_patterns() {
    assert!(glob_matches("/contracts/x.toml", "contracts/x.toml"));
    assert!(!glob_matches("", "anything"));
    assert!(!glob_matches("/", "anything"));
}

#[test]
fn the_real_contract_globs_match_every_kind_of_contract_file() {
    for path in [
        "backend/crates/api/tests/contract_health.rs",
        "backend/crates/services/tests/contract_agents/mod.rs",
        "backend/crates/services/tests/contract_agents/fixtures/a.json",
        "frontend/src/features/agents/state/agents.contract.test.ts",
        "frontend/src/shared/ui/button.contract.test.tsx",
        "frontend/src/features/plans/plan.contract.spec.ts",
        "frontend/e2e/contract/app-shell/app-shell.contract.spec.ts",
        "frontend/e2e/contract/app-shell/fixtures/data.json",
    ] {
        assert!(any_glob(path), "{path} should be a contract file");
    }
}

#[test]
fn the_real_contract_globs_skip_ordinary_files() {
    for path in [
        "backend/crates/api/tests/health.rs",
        "backend/crates/api/src/contract_health.rs",
        "backend/xtask/tests/pinned_contract_tags.rs",
        "frontend/src/shared/ui/button.test.tsx",
        "frontend/src/shared/ui/button.contract.tsx",
        "frontend/src/features/agents/__contract__/.gitkeep",
        "frontend/e2e/golden/publish.golden.spec.ts",
        "frontend/e2e/support/ux-audit.spec.ts",
        "contracts/contracts.toml",
    ] {
        assert!(!any_glob(path), "{path} should not be a contract file");
    }
}

#[test]
fn the_real_manifest_registers_the_existing_contracts() {
    let manifest = Manifest::parse(include_str!("../../../../contracts/contracts.toml"))
        .expect("contracts/contracts.toml parses");

    assert_eq!(manifest.contract_globs, GLOBS);
    let health = &manifest.features["health"];
    assert!(health.matches("backend/crates/api/tests/contract_health.rs"));
    let app_shell = &manifest.features["app-shell"];
    assert!(app_shell.matches("frontend/e2e/contract/app-shell/app-shell.contract.spec.ts"));
    assert!(app_shell.matches("frontend/src/shared/ui/button.contract.test.tsx"));
}

#[test]
fn parses_a_manifest() {
    let manifest = Manifest::parse(
        r#"
        contract_globs = ["**/tests/contract_*.rs"]
        [features.health]
        version = 2
        paths = ["backend/crates/api/tests/contract_health.rs"]
        "#,
    )
    .expect("valid manifest");

    assert_eq!(manifest.contract_globs, ["**/tests/contract_*.rs"]);
    assert_eq!(manifest.features["health"].version, 2);
    let owner = manifest.owner_of("backend/crates/api/tests/contract_health.rs");
    assert_eq!(owner.map(|(name, _)| name), Some("health"));
    assert_eq!(
        manifest
            .owner_of("backend/crates/api/src/lib.rs")
            .map(|(n, _)| n),
        None
    );
}

#[test]
fn rejects_invalid_manifests() {
    for (text, why) in [
        ("[features.a]\nversion = 0\npaths = [\"x\"]", "version 0"),
        ("[features.a]\nversion = 1\npaths = []", "no paths"),
        ("[features.a]\nversion = 1\npaths = [\" \"]", "blank path"),
        (
            "[features.A]\nversion = 1\npaths = [\"x\"]",
            "uppercase name",
        ),
        (
            "[features.a_b]\nversion = 1\npaths = [\"x\"]",
            "underscore in name",
        ),
        (
            "[features.-a]\nversion = 1\npaths = [\"x\"]",
            "leading dash",
        ),
        (
            "[features.a]\nversion = 1\npaths = [\"x\"]\ntypo = 1",
            "unknown field",
        ),
        ("contract_glob = []", "unknown top-level key"),
        ("contract_globs = [\"\"]", "empty glob"),
        (
            "[features.a]\nversion = -1\npaths = [\"x\"]",
            "negative version",
        ),
    ] {
        assert!(Manifest::parse(text).is_err(), "should reject: {why}");
    }
}

fn every_kind_of_problem() -> (Vec<Problem>, Vec<Problem>) {
    let (f, path) = ("x".to_owned(), "p.rs".to_owned());
    let pending = vec![
        Problem::TagMissing {
            feature: f.clone(),
            version: 1,
        },
        Problem::Untagged {
            path: path.clone(),
            hint: Untagged::Unregistered,
        },
        Problem::Untagged {
            path: path.clone(),
            hint: Untagged::AwaitingTag {
                feature: f.clone(),
                version: 1,
            },
        },
        Problem::Untagged {
            path: path.clone(),
            hint: Untagged::NotInTag {
                feature: f.clone(),
                version: 1,
            },
        },
    ];
    let violations = vec![
        Problem::TagUnverified {
            feature: f.clone(),
            version: 1,
            reason: "no principal matched".to_owned(),
        },
        Problem::TagMisnamed {
            feature: f.clone(),
            version: 1,
            object_name: None,
        },
        Problem::TagManifestMismatch {
            feature: f.clone(),
            version: 1,
            found: None,
        },
        Problem::TagCoversNothing {
            feature: f.clone(),
            version: 1,
        },
        Problem::VersionDown {
            feature: f.clone(),
            base: 2,
            head: 1,
        },
        Problem::Removed {
            feature: f.clone(),
            base: 1,
        },
        Problem::Changed {
            path: path.clone(),
            feature: f.clone(),
            version: 1,
        },
        Problem::Deleted {
            path,
            feature: f,
            version: 1,
        },
    ];
    (pending, violations)
}

#[test]
fn only_missing_tags_and_untagged_files_await_approval() {
    let (pending, violations) = every_kind_of_problem();

    for problem in &pending {
        assert!(problem.is_pending(), "should await approval: {problem}");
    }
    for problem in &violations {
        assert!(!problem.is_pending(), "should be a violation: {problem}");
    }
}

#[test]
fn pending_findings_pass_only_when_allowed_and_violations_never_pass() {
    let (pending, violations) = every_kind_of_problem();
    let only_pending = Report {
        problems: pending,
        ..Report::default()
    };

    assert!(only_pending.passed(true));
    assert!(!only_pending.passed(false));
    for violation in violations {
        let mut report = Report {
            problems: vec![violation],
            ..Report::default()
        };
        assert!(!report.passed(true), "{:?}", report.problems);
        report.problems.extend(every_kind_of_problem().0);
        assert!(!report.passed(true), "{:?}", report.problems);
    }
    assert!(Report::default().passed(false));
}

#[test]
fn messages_tell_the_reader_what_to_do() {
    let untagged = Problem::Untagged {
        path: "a/tests/contract_x.rs".to_owned(),
        hint: Untagged::AwaitingTag {
            feature: "x".to_owned(),
            version: 1,
        },
    };
    assert_eq!(
        untagged.to_string(),
        "untagged contract file a/tests/contract_x.rs: the coordinating session pins \
         contract/x/v1 with `just contract-pin x 1` and pushes it with `just contract-push x 1`"
    );
    let changed = Problem::Changed {
        path: "p.rs".to_owned(),
        feature: "x".to_owned(),
        version: 3,
    };
    assert_eq!(
        changed.to_string(),
        "p.rs changed since contract/x/v3: restore it with `git checkout contract/x/v3 -- p.rs`, \
         or bump x to version 4 in contracts/contracts.toml and the coordinating session pins \
         contract/x/v4 with `just contract-pin x 4` and pushes it with `just contract-push x 4`"
    );
}

#[test]
fn signatures_follow_the_flag() {
    assert_eq!(Signatures::required_if(false), Signatures::Ignored);
    assert_eq!(Signatures::required_if(true), Signatures::Required);
    assert_eq!(Signatures::Ignored.to_string(), "signatures not checked");
    assert_eq!(Signatures::Required.to_string(), "signatures required");
}

/// Both modes: the rules that don't depend on signatures must hold in each.
const MODES: [Signatures; 2] = [Signatures::Ignored, Signatures::Required];

/// An in-memory repository: commits are maps of path → contents (the blob id is the text).
#[derive(Default)]
struct FakeRepo {
    trees: BTreeMap<String, BTreeMap<String, String>>,
    /// Tag name → (object name, target tree, signer; `None` for an unsigned tag).
    tags: BTreeMap<String, (Option<String>, String, Option<String>)>,
    /// The allowed-signers text of every `verify_tag` call.
    verified: RefCell<Vec<String>>,
    /// The path of every `read` call.
    reads: RefCell<Vec<String>>,
}

impl FakeRepo {
    fn commit(&mut self, rev: &str, files: &[(&str, &str)]) {
        let tree = files
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect();
        self.trees.insert(rev.to_owned(), tree);
    }

    fn tag_as(&mut self, tag: &str, rev: &str, signer: Option<&str>) {
        let tree = self.trees[rev].clone();
        self.trees.insert(format!("refs/tags/{tag}"), tree);
        self.tags.insert(
            tag.to_owned(),
            (
                Some(tag.to_owned()),
                rev.to_owned(),
                signer.map(str::to_owned),
            ),
        );
    }

    /// An annotated tag signed by `signer`.
    fn sign(&mut self, tag: &str, rev: &str, signer: &str) {
        self.tag_as(tag, rev, Some(signer));
    }

    /// An annotated, unsigned tag, as `just contract-pin` makes it.
    fn pin(&mut self, tag: &str, rev: &str) {
        self.tag_as(tag, rev, None);
    }

    fn read_signers(&self) -> bool {
        self.reads.borrow().iter().any(|path| path == SIGNERS)
    }
}

impl Repo for FakeRepo {
    fn read(&self, rev: &str, path: &str) -> anyhow::Result<Option<String>> {
        self.reads.borrow_mut().push(path.to_owned());
        Ok(self.trees.get(rev).and_then(|tree| tree.get(path)).cloned())
    }

    fn files(&self, rev: &str) -> anyhow::Result<BTreeMap<String, String>> {
        self.trees
            .get(rev)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no such rev {rev}"))
    }

    fn tag(&self, name: &str) -> anyhow::Result<Option<TagRef>> {
        Ok(self.tags.get(name).map(|(object_name, _, _)| TagRef {
            object_name: object_name.clone(),
        }))
    }

    fn verify_tag(&self, name: &str, allowed_signers: &str) -> anyhow::Result<Result<(), String>> {
        self.verified.borrow_mut().push(allowed_signers.to_owned());
        let (_, _, signer) = &self.tags[name];
        Ok(match signer {
            None => Err("no signature found".to_owned()),
            Some(signer) if allowed_signers.lines().any(|line| line == signer) => Ok(()),
            Some(_) => Err("no principal matched".to_owned()),
        })
    }
}

const MANIFEST_V1: &str = r#"
contract_globs = ["**/tests/contract_*.rs"]
[features.alpha]
version = 1
paths = ["a/tests/contract_alpha.rs"]
"#;

fn alpha_repo() -> FakeRepo {
    let mut repo = FakeRepo::default();
    repo.commit(
        "base",
        &[
            (SIGNERS, "owner"),
            (MANIFEST, MANIFEST_V1),
            ("a/tests/contract_alpha.rs", "alpha v1"),
        ],
    );
    repo.sign("contract/alpha/v1", "base", "owner");
    repo
}

/// `alpha_repo` with `contract/alpha/v1` replaced by an unsigned tag.
fn pinned_alpha_repo() -> FakeRepo {
    let mut repo = alpha_repo();
    repo.pin("contract/alpha/v1", "base");
    repo
}

#[test]
fn check_passes_and_counts_pinned_files() {
    for (signatures, state) in [
        (Signatures::Ignored, "pinned, 1 file(s) unchanged"),
        (Signatures::Required, "signed, 1 file(s) unchanged"),
    ] {
        let repo = alpha_repo();

        let report = check(&repo, "base", "base", signatures).expect("check runs");

        assert!(report.passed(false), "{signatures}: {:?}", report.problems);
        assert_eq!(report.pinned, 1);
        assert_eq!(report.features.len(), 1);
        assert_eq!(report.features[0].state, state);
        assert_eq!(report.features[0].tag(), "contract/alpha/v1");
    }
}

#[test]
fn by_default_an_unsigned_tag_pins_and_no_signature_is_read() {
    let repo = pinned_alpha_repo();

    let report = check(&repo, "base", "base", Signatures::Ignored).expect("check runs");

    assert!(report.passed(false), "{:?}", report.problems);
    assert_eq!(report.pinned, 1);
    assert_eq!(report.features[0].state, "pinned, 1 file(s) unchanged");
    assert!(repo.verified.borrow().is_empty(), "verify-tag ran");
    assert!(!repo.read_signers(), "{SIGNERS} was read");
    assert!(report.notices.is_empty(), "{:?}", report.notices);
}

#[test]
fn by_default_a_tag_signed_by_any_key_pins_like_an_unsigned_one() {
    let mut repo = alpha_repo();
    repo.sign("contract/alpha/v1", "base", "stranger");

    let report = check(&repo, "base", "base", Signatures::Ignored).expect("check runs");

    assert!(report.passed(false), "{:?}", report.problems);
    assert!(repo.verified.borrow().is_empty(), "verify-tag ran");
}

#[test]
fn required_signatures_reject_an_unsigned_tag_or_an_unknown_key() {
    for signer in [None, Some("stranger")] {
        let mut repo = alpha_repo();
        repo.tag_as("contract/alpha/v1", "base", signer);

        let report = check(&repo, "base", "base", Signatures::Required).expect("check runs");

        assert!(!report.passed(true), "{signer:?}: {:?}", report.problems);
        assert!(
            matches!(
                report.problems.as_slice(),
                [Problem::TagUnverified { feature, version: 1, .. }, ..] if feature == "alpha"
            ),
            "{signer:?}: {:?}",
            report.problems
        );
        assert_eq!(report.features[0].state, "not signed by an allowed key");
    }
}

#[test]
fn check_verifies_with_the_base_signers_only() {
    let mut repo = alpha_repo();
    repo.commit(
        "head",
        &[
            (SIGNERS, "owner\nstranger"),
            (MANIFEST, MANIFEST_V1),
            ("a/tests/contract_alpha.rs", "alpha v1"),
        ],
    );

    check(&repo, "base", "head", Signatures::Required).expect("check runs");

    assert_eq!(*repo.verified.borrow(), ["owner"]);
}

#[test]
fn check_reports_a_changed_file_once() {
    for (signatures, state) in [
        (
            Signatures::Ignored,
            "pinned, 1 of 1 file(s) changed or deleted",
        ),
        (
            Signatures::Required,
            "signed, 1 of 1 file(s) changed or deleted",
        ),
    ] {
        let mut repo = alpha_repo();
        repo.commit(
            "head",
            &[
                (SIGNERS, "owner"),
                (MANIFEST, MANIFEST_V1),
                ("a/tests/contract_alpha.rs", "alpha v1, edited"),
            ],
        );

        let report = check(&repo, "base", "head", signatures).expect("check runs");

        assert_eq!(
            report.problems,
            [Problem::Changed {
                path: "a/tests/contract_alpha.rs".to_owned(),
                feature: "alpha".to_owned(),
                version: 1,
            }]
        );
        assert_eq!(report.features[0].state, state);
    }
}

#[test]
fn a_changed_file_under_an_unsigned_tag_is_a_violation() {
    let mut repo = pinned_alpha_repo();
    repo.commit(
        "head",
        &[
            (MANIFEST, MANIFEST_V1),
            ("a/tests/contract_alpha.rs", "alpha v1, edited"),
        ],
    );

    let report = check(&repo, "base", "head", Signatures::Ignored).expect("check runs");

    assert!(!report.passed(true), "{:?}", report.problems);
    assert_eq!(
        report.problems,
        [Problem::Changed {
            path: "a/tests/contract_alpha.rs".to_owned(),
            feature: "alpha".to_owned(),
            version: 1,
        }]
    );
}

#[test]
fn a_missing_tag_is_pending_in_both_modes() {
    for signatures in MODES {
        let mut repo = FakeRepo::default();
        repo.commit(
            "base",
            &[
                (SIGNERS, "owner"),
                (MANIFEST, MANIFEST_V1),
                ("a/tests/contract_alpha.rs", "alpha v1"),
            ],
        );

        let report = check(&repo, "base", "base", signatures).expect("check runs");

        assert_eq!(
            report.problems,
            [
                Problem::TagMissing {
                    feature: "alpha".to_owned(),
                    version: 1,
                },
                Problem::Untagged {
                    path: "a/tests/contract_alpha.rs".to_owned(),
                    hint: Untagged::AwaitingTag {
                        feature: "alpha".to_owned(),
                        version: 1,
                    },
                },
            ],
            "{signatures}"
        );
        assert!(report.passed(true), "{signatures}");
        assert!(!report.passed(false), "{signatures}");
    }
}

#[test]
fn a_lightweight_tag_never_pins() {
    for signatures in MODES {
        let mut repo = alpha_repo();
        repo.tags.insert(
            "contract/alpha/v1".to_owned(),
            (None, "base".to_owned(), Some("owner".to_owned())),
        );

        let report = check(&repo, "base", "base", signatures).expect("check runs");

        assert!(
            report.problems.contains(&Problem::TagMisnamed {
                feature: "alpha".to_owned(),
                version: 1,
                object_name: None,
            }),
            "{signatures}: {:?}",
            report.problems
        );
        assert!(!report.passed(true), "{signatures}");
    }
}

#[test]
fn check_without_any_manifest_passes_with_a_notice() {
    // Required adds the BOOTSTRAP notice about HEAD's signers file.
    for (signatures, notices) in [(Signatures::Ignored, 1), (Signatures::Required, 2)] {
        let mut repo = FakeRepo::default();
        repo.commit("base", &[("src/lib.rs", "")]);

        let report = check(&repo, "base", "base", signatures).expect("check runs");

        assert!(report.passed(false));
        assert_eq!(report.notices.len(), notices, "{:?}", report.notices);
    }
}

#[test]
fn check_uses_the_base_globs_even_when_head_drops_them() {
    for signatures in MODES {
        let mut repo = alpha_repo();
        repo.commit(
            "head",
            &[
                (SIGNERS, "owner"),
                (
                    MANIFEST,
                    "contract_globs = []\n[features.alpha]\nversion = 1\npaths = [\"a/tests/contract_alpha.rs\"]\n",
                ),
                ("a/tests/contract_alpha.rs", "alpha v1"),
                ("b/tests/contract_beta.rs", "beta"),
            ],
        );

        let report = check(&repo, "base", "head", signatures).expect("check runs");

        assert_eq!(
            report.problems,
            [Problem::Untagged {
                path: "b/tests/contract_beta.rs".to_owned(),
                hint: Untagged::Unregistered,
            }],
            "{signatures}"
        );
    }
}

#[test]
fn check_rejects_a_tag_whose_manifest_pins_another_version() {
    for signatures in MODES {
        let mut repo = FakeRepo::default();
        repo.commit(
            "base",
            &[
                (SIGNERS, "owner"),
                (MANIFEST, &MANIFEST_V1.replace("version = 1", "version = 2")),
                ("a/tests/contract_alpha.rs", "alpha"),
            ],
        );
        // This commit is tagged as v1, but its manifest says v2.
        repo.sign("contract/alpha/v2", "base", "owner");
        repo.sign("contract/alpha/v1", "base", "owner");
        repo.commit(
            "head",
            &[
                (SIGNERS, "owner"),
                (MANIFEST, MANIFEST_V1),
                ("a/tests/contract_alpha.rs", "alpha"),
            ],
        );

        let report = check(&repo, "head", "head", signatures).expect("check runs");

        assert!(
            report.problems.contains(&Problem::TagManifestMismatch {
                feature: "alpha".to_owned(),
                version: 1,
                found: Some(2),
            }),
            "{signatures}: {:?}",
            report.problems
        );
    }
}

#[test]
fn check_rejects_a_tag_that_covers_no_files() {
    for signatures in MODES {
        let mut repo = FakeRepo::default();
        repo.commit(
            "base",
            &[
                (SIGNERS, "owner"),
                (
                    MANIFEST,
                    "[features.alpha]\nversion = 1\npaths = [\"typo/contract_alpha.rs\"]\n",
                ),
            ],
        );
        repo.sign("contract/alpha/v1", "base", "owner");

        let report = check(&repo, "base", "base", signatures).expect("check runs");

        assert_eq!(
            report.problems,
            [Problem::TagCoversNothing {
                feature: "alpha".to_owned(),
                version: 1,
            }],
            "{signatures}"
        );
    }
}

#[test]
fn an_unparsable_head_manifest_is_an_error() {
    for signatures in MODES {
        let mut repo = alpha_repo();
        repo.commit("head", &[(MANIFEST, "features = 3")]);

        let error = check(&repo, "base", "head", signatures).expect_err("head manifest is invalid");

        assert!(format!("{error:#}").contains("at the head"), "{error:#}");
    }
}
