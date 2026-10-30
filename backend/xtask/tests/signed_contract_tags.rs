//! `xtask contract-check` and `xtask contract-status` against throwaway git repositories.
//!
//! Every test builds a fresh repository in the system temp dir, makes throwaway SSH keys
//! with `ssh-keygen`, signs tags with them and runs the real `xtask` binary.
//!
//! Isolation (nothing here may touch the real repository, `~/.ssh` or the user's config):
//! - every inherited `GIT_*` and `SSH_*` variable is removed;
//! - `HOME` points into the sandbox, global and system git config are disabled;
//! - `GIT_CEILING_DIRECTORIES` stops git from discovering any enclosing repository, and
//!   each sandbox checks that its repository root really is its own directory.
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "sandbox helpers outside #[test] functions fail the test on a setup error"
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const XTASK: &str = env!("CARGO_BIN_EXE_xtask");

const MANIFEST_V1: &str = r#"contract_globs = ["**/tests/contract_*.rs", "e2e/contract/**"]

[features.alpha]
version = 1
paths = ["crates/a/tests/contract_alpha.rs", "e2e/contract/alpha/**"]
"#;

const MANIFEST_V2: &str = r#"contract_globs = ["**/tests/contract_*.rs", "e2e/contract/**"]

[features.alpha]
version = 2
paths = ["crates/a/tests/contract_alpha.rs", "e2e/contract/alpha/**"]
"#;

const MANIFEST_EMPTY: &str = r#"contract_globs = ["**/tests/contract_*.rs", "e2e/contract/**"]
"#;

const ALPHA_TEST: &str = "crates/a/tests/contract_alpha.rs";
const ALPHA_SPEC: &str = "e2e/contract/alpha/flow.spec.ts";
const LIB: &str = "crates/a/src/lib.rs";

/// A temporary repository plus the keys and config that go with it.
struct Sandbox {
    root: PathBuf,
    repo: PathBuf,
}

/// Result of one `xtask` run.
struct Run {
    code: Option<i32>,
    stderr: String,
}

impl Run {
    fn assert_passed(&self) {
        assert_eq!(self.code, Some(0), "expected a pass:\n{}", self.stderr);
    }

    fn assert_failed_with(&self, needle: &str) {
        assert_eq!(
            self.code,
            Some(1),
            "expected a check failure:\n{}",
            self.stderr
        );
        assert!(
            self.stderr.contains(needle),
            "expected {needle:?} in:\n{}",
            self.stderr
        );
    }
}

impl Sandbox {
    fn new(name: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let unique = format!(
            "vak-contract-check-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let root = std::env::temp_dir().join(unique);
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("remove a stale sandbox");
        }
        for dir in ["home", "keys", "repo"] {
            std::fs::create_dir_all(root.join(dir)).expect("create sandbox dirs");
        }
        let root = root.canonicalize().expect("canonical sandbox path");
        std::fs::write(root.join("gitconfig"), "").expect("empty global git config");
        let sandbox = Self {
            repo: root.join("repo"),
            root,
        };

        sandbox.git(&["init", "-q", "-b", "main"]);
        let top = sandbox.git_stdout(&["rev-parse", "--show-toplevel"]);
        assert_eq!(
            Path::new(top.trim())
                .canonicalize()
                .expect("toplevel exists"),
            sandbox.repo,
            "git must operate on the sandbox repository only"
        );
        for (key, value) in [
            ("user.name", "Test Tagger"),
            ("user.email", "tagger@example.invalid"),
            ("commit.gpgsign", "false"),
            ("tag.gpgsign", "false"),
            ("gpg.format", "ssh"),
        ] {
            sandbox.git(&["config", key, value]);
        }
        sandbox.keygen("owner");
        sandbox.keygen("stranger");
        sandbox
    }

    /// A command with the sandbox environment: no inherited git or ssh state.
    fn command(&self, program: &str) -> Command {
        let mut command = Command::new(program);
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy();
            if name.starts_with("GIT_") || name.starts_with("SSH_") {
                command.env_remove(&key);
            }
        }
        command
            .current_dir(&self.repo)
            .env("HOME", self.root.join("home"))
            .env("XDG_CONFIG_HOME", self.root.join("home/.config"))
            .env("GIT_CONFIG_GLOBAL", self.root.join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CEILING_DIRECTORIES", &self.root)
            .env("GIT_TERMINAL_PROMPT", "0");
        command
    }

    fn output(&self, program: &str, args: &[&str]) -> Output {
        self.command(program)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("cannot run {program}: {error}"))
    }

    fn git(&self, args: &[&str]) {
        let output = self.output("git", args);
        assert!(
            output.status.success(),
            "git {args:?} failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_stdout(&self, args: &[&str]) -> String {
        let output = self.output("git", args);
        assert!(
            output.status.success(),
            "git {args:?} failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git prints UTF-8")
    }

    fn key(&self, name: &str) -> PathBuf {
        self.root.join("keys").join(name)
    }

    fn keygen(&self, name: &str) {
        let key = self.key(name);
        let key = key.to_str().expect("UTF-8 key path");
        let output = self.output(
            "ssh-keygen",
            &["-q", "-t", "ed25519", "-N", "", "-C", name, "-f", key],
        );
        assert!(
            output.status.success(),
            "ssh-keygen failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// An allowed-signers line for one of the sandbox keys. The principal is a login, not
    /// the tagger's email: git finds the signer with `ssh-keygen -Y find-principals`.
    fn signer_line(&self, name: &str) -> String {
        let public =
            std::fs::read_to_string(self.key(name).with_extension("pub")).expect("read public key");
        format!("{name} namespaces=\"git\" {}", public.trim())
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.repo.join(path);
        std::fs::create_dir_all(path.parent().expect("file has a parent")).expect("mkdir");
        std::fs::write(path, text).expect("write file");
    }

    fn remove(&self, path: &str) {
        self.git(&["rm", "-q", path]);
    }

    fn commit(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "--allow-empty", "-m", message]);
        self.git_stdout(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    fn signers(&self, names: &[&str]) {
        let mut text = String::from("# allowed signers for contract tags\n");
        for name in names {
            text.push_str(&self.signer_line(name));
            text.push('\n');
        }
        self.write(".github/contract-signers", &text);
    }

    /// The base every scenario starts from: signers, the alpha contract at v1, other code.
    fn alpha_v1(&self) -> String {
        self.signers(&["owner"]);
        self.write("contracts/contracts.toml", MANIFEST_V1);
        self.write(ALPHA_TEST, "// alpha v1\n");
        self.write(ALPHA_SPEC, "// alpha flow v1\n");
        self.write(LIB, "pub fn a() {}\n");
        self.commit("alpha v1 contract")
    }

    fn sign(&self, key: &str, tag: &str, target: &str) {
        let key = self.key(key);
        let config = format!("user.signingkey={}", key.display());
        self.git(&["-c", &config, "tag", "-s", "-m", tag, tag, target]);
    }

    fn branch(&self, name: &str, from: &str) {
        self.git(&["checkout", "-q", "-b", name, from]);
    }

    fn xtask(&self, args: &[&str]) -> Run {
        let output = self.output(XTASK, args);
        Run {
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    fn check(&self, base: &str, head: &str) -> Run {
        self.xtask(&["contract-check", "--base", base, "--head", head])
    }

    /// The local mode: contracts awaiting the owner's signature warn instead of failing.
    fn check_allowing_pending(&self, base: &str, head: &str) -> Run {
        self.xtask(&[
            "contract-check",
            "--allow-pending",
            "--base",
            base,
            "--head",
            head,
        ])
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        // Best effort: a leftover temp dir is harmless.
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn passes_when_every_contract_file_matches_its_signed_tag() {
    let sandbox = Sandbox::new("pass");
    let base = sandbox.alpha_v1();
    sandbox.sign("owner", "contract/alpha/v1", &base);
    sandbox.branch("work", &base);
    sandbox.write(LIB, "pub fn a() -> u8 { 1 }\n");
    let head = sandbox.commit("implement alpha");

    let run = sandbox.check(&base, &head);

    run.assert_passed();
    assert!(run.stderr.contains("contract-check: ok"), "{}", run.stderr);
}

#[test]
fn passes_after_a_squash_merge_because_blobs_not_ancestry_are_compared() {
    let sandbox = Sandbox::new("squash");
    sandbox.signers(&["owner"]);
    sandbox.write("contracts/contracts.toml", MANIFEST_EMPTY);
    sandbox.write(LIB, "pub fn a() {}\n");
    let main = sandbox.commit("main before the feature");
    sandbox.branch("feature", &main);
    sandbox.write("contracts/contracts.toml", MANIFEST_V1);
    sandbox.write(ALPHA_TEST, "// alpha v1\n");
    sandbox.write(ALPHA_SPEC, "// alpha flow v1\n");
    let tests_commit = sandbox.commit("alpha contract tests");
    sandbox.sign("owner", "contract/alpha/v1", &tests_commit);
    sandbox.write(LIB, "pub fn a() -> u8 { 1 }\n");
    sandbox.commit("implement alpha");
    sandbox.git(&["checkout", "-q", "main"]);
    sandbox.git(&["merge", "-q", "--squash", "feature"]);
    let squashed = sandbox.commit("feat: alpha (squashed)");

    sandbox.check(&main, &squashed).assert_passed();
}

#[test]
fn fails_when_a_tagged_file_changed() {
    let sandbox = Sandbox::new("modified");
    let base = sandbox.alpha_v1();
    sandbox.sign("owner", "contract/alpha/v1", &base);
    sandbox.branch("work", &base);
    sandbox.write(ALPHA_TEST, "// alpha v1, quietly relaxed\n");
    let head = sandbox.commit("relax the contract");

    let run = sandbox.check(&base, &head);

    run.assert_failed_with(&format!("{ALPHA_TEST} changed since contract/alpha/v1"));
    assert!(run.stderr.contains("bump"), "{}", run.stderr);
}

#[test]
fn fails_when_a_tagged_file_was_deleted() {
    let sandbox = Sandbox::new("deleted");
    let base = sandbox.alpha_v1();
    sandbox.sign("owner", "contract/alpha/v1", &base);
    sandbox.branch("work", &base);
    sandbox.remove(ALPHA_SPEC);
    let head = sandbox.commit("drop a contract spec");

    sandbox
        .check(&base, &head)
        .assert_failed_with(&format!("{ALPHA_SPEC} was deleted since contract/alpha/v1"));
}

#[test]
fn fails_when_the_tag_is_annotated_but_unsigned() {
    let sandbox = Sandbox::new("unsigned");
    let base = sandbox.alpha_v1();
    sandbox.git(&["tag", "-a", "-m", "alpha v1", "contract/alpha/v1", &base]);

    sandbox
        .check(&base, &base)
        .assert_failed_with("contract/alpha/v1 is not signed by an allowed key");
}

#[test]
fn fails_when_the_tag_is_lightweight() {
    let sandbox = Sandbox::new("lightweight");
    let base = sandbox.alpha_v1();
    sandbox.git(&["tag", "contract/alpha/v1", &base]);

    sandbox
        .check(&base, &base)
        .assert_failed_with("contract/alpha/v1 is not signed by an allowed key");
}

#[test]
fn fails_when_the_tag_is_signed_by_an_unknown_key() {
    let sandbox = Sandbox::new("unknown-key");
    let base = sandbox.alpha_v1();
    sandbox.sign("stranger", "contract/alpha/v1", &base);

    sandbox
        .check(&base, &base)
        .assert_failed_with("contract/alpha/v1 is not signed by an allowed key");
}

#[test]
fn fails_when_the_signer_was_added_only_in_head() {
    let sandbox = Sandbox::new("signer-in-head");
    sandbox.signers(&["owner"]);
    sandbox.write("contracts/contracts.toml", MANIFEST_EMPTY);
    sandbox.write(LIB, "pub fn a() {}\n");
    let base = sandbox.commit("main");
    sandbox.branch("work", &base);
    sandbox.signers(&["owner", "stranger"]);
    sandbox.write("contracts/contracts.toml", MANIFEST_V1);
    sandbox.write(ALPHA_TEST, "// alpha v1\n");
    sandbox.write(ALPHA_SPEC, "// alpha flow v1\n");
    let head = sandbox.commit("add my own key and approve my own contract");
    sandbox.sign("stranger", "contract/alpha/v1", &head);

    sandbox
        .check(&base, &head)
        .assert_failed_with("contract/alpha/v1 is not signed by an allowed key");
    // The same tag verifies against HEAD's signers file, so only the base's copy counts.
    sandbox.check(&head, &head).assert_passed();
}

#[test]
fn fails_when_the_base_has_no_signers_file() {
    let sandbox = Sandbox::new("no-signers");
    sandbox.write("contracts/contracts.toml", MANIFEST_V1);
    sandbox.write(ALPHA_TEST, "// alpha v1\n");
    sandbox.write(ALPHA_SPEC, "// alpha flow v1\n");
    let base = sandbox.commit("no signers yet");
    sandbox.sign("owner", "contract/alpha/v1", &base);

    let run = sandbox.check(&base, &base);

    run.assert_failed_with("contract/alpha/v1 is not signed by an allowed key");
    assert!(
        run.stderr.contains("has no .github/contract-signers"),
        "{}",
        run.stderr
    );
}

#[test]
fn a_base_from_before_the_contract_system_trusts_heads_signers_with_a_warning() {
    let sandbox = Sandbox::new("bootstrap");
    sandbox.write(LIB, "pub fn a() {}\n");
    let base = sandbox.commit("before contracts existed");
    sandbox.branch("work", &base);
    let head = sandbox.alpha_v1();
    sandbox.sign("owner", "contract/alpha/v1", &head);

    let run = sandbox.check(&base, &head);

    run.assert_passed();
    assert!(
        run.stderr
            .contains("BOOTSTRAP: the base has no contracts/contracts.toml"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.contains("HEAD's .github/contract-signers"),
        "{}",
        run.stderr
    );
}

#[test]
fn bootstrap_still_rejects_a_key_heads_signers_file_does_not_list() {
    let sandbox = Sandbox::new("bootstrap-stranger");
    sandbox.write(LIB, "pub fn a() {}\n");
    let base = sandbox.commit("before contracts existed");
    sandbox.branch("work", &base);
    let head = sandbox.alpha_v1();
    sandbox.sign("stranger", "contract/alpha/v1", &head);

    sandbox
        .check(&base, &head)
        .assert_failed_with("contract/alpha/v1 is not signed by an allowed key");
}

#[test]
fn fails_when_the_version_goes_down() {
    let sandbox = Sandbox::new("version-down");
    let v1 = sandbox.alpha_v1();
    sandbox.sign("owner", "contract/alpha/v1", &v1);
    sandbox.write("contracts/contracts.toml", MANIFEST_V2);
    sandbox.write(ALPHA_TEST, "// alpha v2\n");
    let base = sandbox.commit("alpha v2 contract");
    sandbox.sign("owner", "contract/alpha/v2", &base);
    sandbox.branch("work", &base);
    sandbox.write("contracts/contracts.toml", MANIFEST_V1);
    sandbox.write(ALPHA_TEST, "// alpha v1\n");
    let head = sandbox.commit("back to the old contract");

    let run = sandbox.check(&base, &head);

    run.assert_failed_with("alpha: contract version went down from 2 to 1");
    // Every v1 file matches its signed tag: the version is the only problem.
    assert!(!run.stderr.contains("changed since"), "{}", run.stderr);
}

#[test]
fn passes_when_the_version_goes_up_and_the_new_tag_is_signed() {
    let sandbox = Sandbox::new("version-up");
    let base = sandbox.alpha_v1();
    sandbox.sign("owner", "contract/alpha/v1", &base);
    sandbox.branch("work", &base);
    sandbox.write("contracts/contracts.toml", MANIFEST_V2);
    sandbox.write(ALPHA_TEST, "// alpha v2\n");
    let head = sandbox.commit("alpha v2 contract");

    sandbox
        .check(&base, &head)
        .assert_failed_with("ask the owner to run `just contract-approve alpha 2`");

    sandbox.sign("owner", "contract/alpha/v2", &head);
    sandbox.check(&base, &head).assert_passed();
}

#[test]
fn fails_on_an_untagged_contract_file() {
    let sandbox = Sandbox::new("untagged");
    let base = sandbox.alpha_v1();
    sandbox.sign("owner", "contract/alpha/v1", &base);
    sandbox.branch("work", &base);
    sandbox.write(
        "crates/b/tests/contract_beta.rs",
        "// beta, never registered\n",
    );
    let head = sandbox.commit("sneak in a contract");

    let run = sandbox.check(&base, &head);

    run.assert_failed_with("untagged contract file crates/b/tests/contract_beta.rs");
    assert!(
        run.stderr
            .contains("register it in contracts/contracts.toml"),
        "{}",
        run.stderr
    );
}

#[test]
fn fails_on_a_new_file_under_a_tagged_feature() {
    let sandbox = Sandbox::new("new-file-in-feature");
    let base = sandbox.alpha_v1();
    sandbox.sign("owner", "contract/alpha/v1", &base);
    sandbox.branch("work", &base);
    sandbox.write("e2e/contract/alpha/extra.spec.ts", "// not in the tag\n");
    let head = sandbox.commit("extend the contract without a bump");

    let run = sandbox.check(&base, &head);

    run.assert_failed_with("untagged contract file e2e/contract/alpha/extra.spec.ts");
    assert!(
        run.stderr.contains("bump alpha to version 2"),
        "{}",
        run.stderr
    );
}

#[test]
fn registered_contract_without_a_tag_asks_the_owner_to_approve() {
    let sandbox = Sandbox::new("awaiting-approval");
    let head = sandbox.alpha_v1();

    let run = sandbox.check(&head, &head);

    run.assert_failed_with(&format!(
        "untagged contract file {ALPHA_TEST}: ask the owner to run `just contract-approve alpha 1`"
    ));
    assert!(
        run.stderr.contains("contract/alpha/v1: tag not found"),
        "{}",
        run.stderr
    );
}

#[test]
fn fails_when_a_contract_is_removed_from_the_manifest() {
    let sandbox = Sandbox::new("removed");
    let base = sandbox.alpha_v1();
    sandbox.sign("owner", "contract/alpha/v1", &base);
    sandbox.branch("work", &base);
    sandbox.write("contracts/contracts.toml", MANIFEST_EMPTY);
    sandbox.remove(ALPHA_TEST);
    sandbox.remove(ALPHA_SPEC);
    let head = sandbox.commit("delete the whole contract");

    sandbox
        .check(&base, &head)
        .assert_failed_with("alpha: removed from contracts/contracts.toml");
}

#[test]
fn fails_when_a_tag_ref_points_at_another_tags_signature() {
    let sandbox = Sandbox::new("renamed-tag");
    sandbox.signers(&["owner"]);
    sandbox.write(
        "contracts/contracts.toml",
        &format!(
            "{MANIFEST_V1}\n[features.beta]\nversion = 1\npaths = [\"crates/b/tests/contract_beta.rs\"]\n"
        ),
    );
    sandbox.write(ALPHA_TEST, "// alpha v1\n");
    sandbox.write(ALPHA_SPEC, "// alpha flow v1\n");
    sandbox.write("crates/b/tests/contract_beta.rs", "// beta v1\n");
    let base = sandbox.commit("alpha and beta");
    sandbox.sign("owner", "contract/alpha/v1", &base);
    // Reuse alpha's signed tag object under beta's name.
    let object = sandbox.git_stdout(&["rev-parse", "refs/tags/contract/alpha/v1"]);
    sandbox.git(&["update-ref", "refs/tags/contract/beta/v1", object.trim()]);

    sandbox.check(&base, &base).assert_failed_with(
        "refs/tags/contract/beta/v1 holds the tag object named contract/alpha/v1",
    );
}

#[test]
fn an_unknown_ref_is_a_usage_error() {
    let sandbox = Sandbox::new("unknown-ref");
    let head = sandbox.alpha_v1();

    let run = sandbox.check("no-such-branch", &head);

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(run.stderr.contains("no-such-branch"), "{}", run.stderr);
}

// --allow-pending (local gates): contracts awaiting the owner's signature warn and pass;
// violations of signed contracts still fail. Without the flag (CI), both fail.

#[test]
fn allow_pending_passes_with_a_warning_while_a_new_contract_awaits_its_signature() {
    let sandbox = Sandbox::new("pending-new");
    let head = sandbox.alpha_v1();

    let strict = sandbox.check(&head, &head);
    let local = sandbox.check_allowing_pending(&head, &head);

    strict.assert_failed_with("untagged contract file");
    local.assert_passed();
    assert!(
        local.stderr.contains(&format!(
            "awaiting owner signature: untagged contract file {ALPHA_TEST}: ask the owner to run \
             `just contract-approve alpha 1`"
        )),
        "{}",
        local.stderr
    );
    assert!(
        local
            .stderr
            .contains("awaiting owner signature: contract/alpha/v1: tag not found"),
        "{}",
        local.stderr
    );
    assert!(!local.stderr.contains("error:"), "{}", local.stderr);
}

#[test]
fn allow_pending_passes_while_a_bumped_version_awaits_its_signature() {
    let sandbox = Sandbox::new("pending-bump");
    let base = sandbox.alpha_v1();
    sandbox.sign("owner", "contract/alpha/v1", &base);
    sandbox.branch("work", &base);
    sandbox.write("contracts/contracts.toml", MANIFEST_V2);
    sandbox.write(ALPHA_TEST, "// alpha v2\n");
    let head = sandbox.commit("alpha v2 contract, awaiting review");

    sandbox
        .check(&base, &head)
        .assert_failed_with("`just contract-approve alpha 2`");
    let local = sandbox.check_allowing_pending(&base, &head);
    local.assert_passed();
    assert!(
        local.stderr.contains("awaiting owner signature"),
        "{}",
        local.stderr
    );
}

#[test]
fn allow_pending_still_fails_when_a_tagged_file_changed() {
    let sandbox = Sandbox::new("pending-modified");
    let base = sandbox.alpha_v1();
    sandbox.sign("owner", "contract/alpha/v1", &base);
    sandbox.branch("work", &base);
    sandbox.write(ALPHA_TEST, "// alpha v1, quietly relaxed\n");
    let head = sandbox.commit("relax the contract");

    sandbox
        .check_allowing_pending(&base, &head)
        .assert_failed_with(&format!(
            "error: {ALPHA_TEST} changed since contract/alpha/v1"
        ));
}

#[test]
fn allow_pending_still_fails_when_a_tagged_file_was_deleted() {
    let sandbox = Sandbox::new("pending-deleted");
    let base = sandbox.alpha_v1();
    sandbox.sign("owner", "contract/alpha/v1", &base);
    sandbox.branch("work", &base);
    sandbox.remove(ALPHA_SPEC);
    let head = sandbox.commit("drop a contract spec");

    sandbox
        .check_allowing_pending(&base, &head)
        .assert_failed_with(&format!(
            "error: {ALPHA_SPEC} was deleted since contract/alpha/v1"
        ));
}

#[test]
fn allow_pending_still_fails_on_an_unknown_signer() {
    let sandbox = Sandbox::new("pending-unknown-key");
    let base = sandbox.alpha_v1();
    sandbox.sign("stranger", "contract/alpha/v1", &base);

    sandbox
        .check_allowing_pending(&base, &base)
        .assert_failed_with("error: contract/alpha/v1 is not signed by an allowed key");
}

#[test]
fn allow_pending_still_fails_on_an_unsigned_tag() {
    let sandbox = Sandbox::new("pending-unsigned");
    let base = sandbox.alpha_v1();
    sandbox.git(&["tag", "-a", "-m", "alpha v1", "contract/alpha/v1", &base]);

    sandbox
        .check_allowing_pending(&base, &base)
        .assert_failed_with("error: contract/alpha/v1 is not signed by an allowed key");
}

#[test]
fn allow_pending_still_fails_on_a_signer_added_only_in_head() {
    let sandbox = Sandbox::new("pending-signer-in-head");
    sandbox.signers(&["owner"]);
    sandbox.write("contracts/contracts.toml", MANIFEST_EMPTY);
    sandbox.write(LIB, "pub fn a() {}\n");
    let base = sandbox.commit("main");
    sandbox.branch("work", &base);
    sandbox.signers(&["owner", "stranger"]);
    sandbox.write("contracts/contracts.toml", MANIFEST_V1);
    sandbox.write(ALPHA_TEST, "// alpha v1\n");
    sandbox.write(ALPHA_SPEC, "// alpha flow v1\n");
    let head = sandbox.commit("add my own key and approve my own contract");
    sandbox.sign("stranger", "contract/alpha/v1", &head);

    sandbox
        .check_allowing_pending(&base, &head)
        .assert_failed_with("error: contract/alpha/v1 is not signed by an allowed key");
}

#[test]
fn allow_pending_still_fails_when_the_version_goes_down() {
    let sandbox = Sandbox::new("pending-version-down");
    let v1 = sandbox.alpha_v1();
    sandbox.sign("owner", "contract/alpha/v1", &v1);
    sandbox.write("contracts/contracts.toml", MANIFEST_V2);
    sandbox.write(ALPHA_TEST, "// alpha v2\n");
    let base = sandbox.commit("alpha v2 contract");
    sandbox.sign("owner", "contract/alpha/v2", &base);
    sandbox.branch("work", &base);
    sandbox.write("contracts/contracts.toml", MANIFEST_V1);
    sandbox.write(ALPHA_TEST, "// alpha v1\n");
    let head = sandbox.commit("back to the old contract");

    sandbox
        .check_allowing_pending(&base, &head)
        .assert_failed_with("error: alpha: contract version went down from 2 to 1");
}

#[test]
fn allow_pending_still_fails_when_a_contract_is_removed() {
    let sandbox = Sandbox::new("pending-removed");
    let base = sandbox.alpha_v1();
    sandbox.sign("owner", "contract/alpha/v1", &base);
    sandbox.branch("work", &base);
    sandbox.write("contracts/contracts.toml", MANIFEST_EMPTY);
    sandbox.remove(ALPHA_TEST);
    sandbox.remove(ALPHA_SPEC);
    let head = sandbox.commit("delete the whole contract");

    sandbox
        .check_allowing_pending(&base, &head)
        .assert_failed_with("error: alpha: removed from contracts/contracts.toml");
}

#[test]
fn status_lists_each_feature_with_its_tag_state() {
    let sandbox = Sandbox::new("status");
    sandbox.signers(&["owner"]);
    sandbox.write(
        "contracts/contracts.toml",
        &format!(
            "{MANIFEST_V1}\n[features.beta]\nversion = 3\npaths = [\"crates/b/tests/contract_beta.rs\"]\n"
        ),
    );
    sandbox.write(ALPHA_TEST, "// alpha v1\n");
    sandbox.write(ALPHA_SPEC, "// alpha flow v1\n");
    sandbox.write("crates/b/tests/contract_beta.rs", "// beta v3\n");
    let head = sandbox.commit("alpha and beta");
    sandbox.sign("owner", "contract/alpha/v1", &head);

    let run = sandbox.xtask(&["contract-status", "--base", &head, "--head", &head]);

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let alpha = run
        .stderr
        .lines()
        .find(|line| line.starts_with("alpha "))
        .unwrap_or_else(|| panic!("no alpha row:\n{}", run.stderr));
    assert!(alpha.contains("contract/alpha/v1"), "{alpha}");
    assert!(alpha.contains("signed"), "{alpha}");
    let beta = run
        .stderr
        .lines()
        .find(|line| line.starts_with("beta "))
        .unwrap_or_else(|| panic!("no beta row:\n{}", run.stderr));
    assert!(beta.contains("contract/beta/v3"), "{beta}");
    assert!(beta.contains("tag not found"), "{beta}");
}
