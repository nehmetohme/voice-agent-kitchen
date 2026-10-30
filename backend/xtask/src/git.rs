//! The real [`Repo`]: runs `git` with `std::process::Command` (no libgit2).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Context as _;

use crate::contracts::{Repo, TagRef};

/// A repository, addressed by its top-level directory.
pub(crate) struct Git {
    root: PathBuf,
}

impl Git {
    /// The repository containing `dir` (default: the current directory).
    pub(crate) fn open(dir: Option<&Path>) -> anyhow::Result<Self> {
        let dir = dir.unwrap_or_else(|| Path::new("."));
        let output = run(dir, &["rev-parse", "--show-toplevel"])?;
        anyhow::ensure!(
            output.status.success(),
            "{} is not inside a git repository: {}",
            dir.display(),
            reason(&output)
        );
        let root = String::from_utf8(output.stdout).context("git printed a non-UTF-8 path")?;
        Ok(Self {
            root: PathBuf::from(root.trim_end_matches(['\n', '\r'])),
        })
    }

    /// The full commit id `rev` names.
    pub(crate) fn commit(&self, rev: &str) -> anyhow::Result<String> {
        anyhow::ensure!(!rev.starts_with('-'), "invalid revision {rev:?}");
        let output = self.output(&[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ])?;
        anyhow::ensure!(
            output.status.success(),
            "unknown revision {rev:?}: fetch it first (for example `git fetch origin main`)"
        );
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    fn output(&self, args: &[&str]) -> anyhow::Result<Output> {
        run(&self.root, args)
    }

    fn stdout(&self, args: &[&str]) -> anyhow::Result<Vec<u8>> {
        let output = self.output(args)?;
        anyhow::ensure!(
            output.status.success(),
            "`git {}` failed: {}",
            args.join(" "),
            reason(&output)
        );
        Ok(output.stdout)
    }
}

impl Repo for Git {
    fn read(&self, rev: &str, path: &str) -> anyhow::Result<Option<String>> {
        let spec = format!("{rev}:{path}");
        if !self.output(&["cat-file", "-e", &spec])?.status.success() {
            return Ok(None);
        }
        let bytes = self.stdout(&["cat-file", "blob", &spec])?;
        String::from_utf8(bytes)
            .map(Some)
            .with_context(|| format!("{spec} is not UTF-8"))
    }

    fn files(&self, rev: &str) -> anyhow::Result<BTreeMap<String, String>> {
        // -z: NUL-terminated records, paths never quoted. --full-tree: independent of cwd.
        let listing = self.stdout(&["ls-tree", "-r", "-z", "--full-tree", rev])?;
        let mut files = BTreeMap::new();
        for record in listing.split(|&byte| byte == 0).filter(|r| !r.is_empty()) {
            let record = String::from_utf8_lossy(record);
            // "<mode> SP <type> SP <object> TAB <path>"
            let parsed = record.split_once('\t').and_then(|(meta, path)| {
                let mut meta = meta.split(' ');
                let (_mode, kind, object) = (meta.next()?, meta.next()?, meta.next()?);
                Some((kind, object, path))
            });
            let Some((kind, object, path)) = parsed else {
                anyhow::bail!("unexpected `git ls-tree` output: {record:?}");
            };
            if kind == "blob" {
                files.insert(path.to_owned(), object.to_owned());
            }
        }
        Ok(files)
    }

    fn tag(&self, name: &str) -> anyhow::Result<Option<TagRef>> {
        let output = self.output(&[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/tags/{name}"),
        ])?;
        if !output.status.success() {
            return Ok(None);
        }
        let object = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let kind = self.stdout(&["cat-file", "-t", &object])?;
        if String::from_utf8_lossy(&kind).trim() != "tag" {
            return Ok(Some(TagRef { object_name: None }));
        }
        let body = self.stdout(&["cat-file", "tag", &object])?;
        let body = String::from_utf8_lossy(&body);
        let object_name = body
            .lines()
            .take_while(|line| !line.is_empty())
            .find_map(|line| line.strip_prefix("tag "))
            .map(str::to_owned);
        Ok(Some(TagRef { object_name }))
    }

    fn verify_tag(&self, name: &str, allowed_signers: &str) -> anyhow::Result<Result<(), String>> {
        let signers = TempFile::new("contract-signers", allowed_signers)?;
        let allowed = format!("gpg.ssh.allowedSignersFile={}", signers.path.display());
        let output = self.output(&[
            "-c",
            &allowed,
            // Verify with OpenSSH itself, whatever signing helper the user configured.
            "-c",
            "gpg.ssh.program=ssh-keygen",
            "verify-tag",
            &format!("refs/tags/{name}"),
        ])?;
        Ok(if output.status.success() {
            Ok(())
        } else {
            Err(reason(&output))
        })
    }
}

fn run(dir: &Path, args: &[&str]) -> anyhow::Result<Output> {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .context("running git (is it installed and on PATH?)")
}

/// The last non-empty line git wrote to stderr: usually the actual reason.
fn reason(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("no output")
        .to_owned()
}

/// A file in the system temp dir, removed on drop.
struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn new(name: &str, contents: &str) -> anyhow::Result<Self> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "vak-xtask-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, contents).with_context(|| format!("writing {}", path.display()))?;
        Ok(Self { path })
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        // Best effort: the file only ever holds public keys.
        let _ = std::fs::remove_file(&self.path);
    }
}
