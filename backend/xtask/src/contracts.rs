//! `contract-check` and `contract-status`: contract test files are pinned by
//! `contract/<feature>/v<N>` tags. Design: `docs/research/06-graphify-github-ci.md` §4.
//!
//! For each feature in HEAD's `contracts/contracts.toml`:
//! - its version is not lower than the base's;
//! - tag `contract/<feature>/v<version>` exists and is a tag object of that name. By default
//!   ([`Signatures::Ignored`]) signed and unsigned tags pin alike. With
//!   [`Signatures::Required`] (`--require-signatures`), `git verify-tag` must also accept the
//!   tag with the **base** ref's `.github/contract-signers`;
//! - the tag's own manifest pins that version and defines the file set, and every file in
//!   it is blob-identical at HEAD (blobs, not ancestry, so squash merges pass).
//!
//! Every file at HEAD matching a contract glob (the base's or HEAD's) must be covered by
//! such a tag, and a feature in the base's manifest may not disappear from HEAD's.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use anyhow::Context as _;
use serde::Deserialize;

#[cfg(test)]
mod tests;

/// The contract manifest, relative to the repository root.
pub(crate) const MANIFEST: &str = "contracts/contracts.toml";
/// SSH allowed-signers file for contract tags. Read only with [`Signatures::Required`], and
/// then only the base ref's copy is trusted.
pub(crate) const SIGNERS: &str = ".github/contract-signers";

/// Whether a contract tag must carry a trusted signature to pin its files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Signatures {
    /// The default: an annotated tag of the right name pins its files, signed or not. No
    /// signature is verified and [`SIGNERS`] is never read.
    Ignored,
    /// `--require-signatures`: `git verify-tag` must also accept each tag with the base's
    /// [`SIGNERS`] (see [`trusted_signers`] for the one bootstrap exception).
    Required,
}

impl Signatures {
    /// The mode for the `--require-signatures` flag.
    pub(crate) fn required_if(flag: bool) -> Self {
        if flag { Self::Required } else { Self::Ignored }
    }

    /// How a valid tag is described in a `contract-status` row.
    fn verb(self) -> &'static str {
        match self {
            Self::Ignored => "pinned",
            Self::Required => "signed",
        }
    }
}

impl fmt::Display for Signatures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ignored => "signatures not checked",
            Self::Required => "signatures required",
        })
    }
}

/// Parsed `contracts/contracts.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    /// Files matching any of these must be covered by a contract tag.
    #[serde(default)]
    pub(crate) contract_globs: Vec<String>,
    /// One entry per feature, keyed by name.
    #[serde(default)]
    pub(crate) features: BTreeMap<String, Feature>,
}

/// One contract: the version HEAD pins and the files (globs) that make it up.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Feature {
    pub(crate) version: u32,
    pub(crate) paths: Vec<String>,
}

impl Manifest {
    pub(crate) fn parse(text: &str) -> anyhow::Result<Self> {
        let manifest: Self = toml::from_str(text)?;
        for glob in &manifest.contract_globs {
            anyhow::ensure!(!glob.trim().is_empty(), "contract_globs: empty pattern");
        }
        for (name, feature) in &manifest.features {
            anyhow::ensure!(
                valid_feature_name(name),
                "features.{name}: use lowercase letters, digits and '-' (it becomes a tag name)"
            );
            anyhow::ensure!(
                feature.version >= 1,
                "features.{name}.version: versions start at 1"
            );
            anyhow::ensure!(
                !feature.paths.is_empty() && feature.paths.iter().all(|p| !p.trim().is_empty()),
                "features.{name}.paths: list at least one file or glob, and no empty ones"
            );
        }
        Ok(manifest)
    }

    /// The first feature whose paths match `path`.
    fn owner_of(&self, path: &str) -> Option<(&str, &Feature)> {
        self.features
            .iter()
            .find(|(_, feature)| feature.matches(path))
            .map(|(name, feature)| (name.as_str(), feature))
    }
}

impl Feature {
    fn matches(&self, path: &str) -> bool {
        self.paths.iter().any(|pattern| glob_matches(pattern, path))
    }
}

fn valid_feature_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `contract/<feature>/v<version>`.
pub(crate) fn tag_name(feature: &str, version: u32) -> String {
    format!("contract/{feature}/v{version}")
}

/// Git-style glob over `/`-separated paths: a `**` segment matches any number of
/// segments, `*` any run of characters inside one segment and `?` one character. Like a
/// git pathspec, a pattern that matches a folder also matches everything below it.
pub(crate) fn glob_matches(pattern: &str, path: &str) -> bool {
    let pattern: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let path: Vec<&str> = path.split('/').collect();
    !pattern.is_empty() && segments_match(&pattern, &path)
}

fn segments_match(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        // Every pattern segment matched: `path` is the match itself or lies below it.
        None => true,
        Some((&"**", rest)) => (0..=path.len()).any(|skip| segments_match(rest, &path[skip..])),
        Some((first, rest)) => path.split_first().is_some_and(|(segment, below)| {
            let pattern: Vec<char> = first.chars().collect();
            let segment: Vec<char> = segment.chars().collect();
            segment_matches(&pattern, &segment) && segments_match(rest, below)
        }),
    }
}

fn segment_matches(pattern: &[char], text: &[char]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some(('*', rest)) => (0..=text.len()).any(|skip| segment_matches(rest, &text[skip..])),
        Some(('?', rest)) => text
            .split_first()
            .is_some_and(|(_, tail)| segment_matches(rest, tail)),
        Some((c, rest)) => text
            .split_first()
            .is_some_and(|(t, tail)| t == c && segment_matches(rest, tail)),
    }
}

/// What the checks need from git. `crate::git::Git` runs the real commands.
pub(crate) trait Repo {
    /// Text of `path` in `rev`'s tree, or `None` when the file does not exist there.
    fn read(&self, rev: &str, path: &str) -> anyhow::Result<Option<String>>;
    /// Every file in `rev`'s tree with its blob id.
    fn files(&self, rev: &str) -> anyhow::Result<BTreeMap<String, String>>;
    /// The tag `refs/tags/<name>`, or `None` when it does not exist.
    fn tag(&self, name: &str) -> anyhow::Result<Option<TagRef>>;
    /// `Ok(())` when `git verify-tag` accepts the tag using `allowed_signers` (the text of
    /// an SSH allowed-signers file); otherwise git's reason.
    fn verify_tag(&self, name: &str, allowed_signers: &str) -> anyhow::Result<Result<(), String>>;
}

/// A tag ref.
#[derive(Clone, Debug)]
pub(crate) struct TagRef {
    /// The `tag` header of the tag object; `None` for a lightweight tag.
    pub(crate) object_name: Option<String>,
}

/// Everything `check` found.
#[derive(Debug, Default)]
pub(crate) struct Report {
    /// One row per feature in HEAD's manifest.
    pub(crate) features: Vec<FeatureStatus>,
    pub(crate) problems: Vec<Problem>,
    /// Facts worth knowing that are not failures by themselves.
    pub(crate) notices: Vec<String>,
    /// Files covered by a valid contract tag.
    pub(crate) pinned: usize,
}

impl Report {
    /// Whether the check passes. Findings that only await a contract tag pass when
    /// `allow_pending` is set (local gates); violations never pass. CI runs with `false`.
    pub(crate) fn passed(&self, allow_pending: bool) -> bool {
        self.problems
            .iter()
            .all(|problem| allow_pending && problem.is_pending())
    }

    /// How many findings only await a contract tag.
    pub(crate) fn pending(&self) -> usize {
        self.problems.iter().filter(|p| p.is_pending()).count()
    }
}

/// A feature in HEAD's manifest and the state of its tag.
#[derive(Debug)]
pub(crate) struct FeatureStatus {
    pub(crate) name: String,
    pub(crate) version: u32,
    /// Short description, e.g. `pinned, 2 file(s) unchanged` or `tag not found`.
    pub(crate) state: String,
}

impl FeatureStatus {
    pub(crate) fn tag(&self) -> String {
        tag_name(&self.name, self.version)
    }
}

/// One failed rule. `Display` is the message shown to the user.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Problem {
    TagMissing {
        feature: String,
        version: u32,
    },
    /// Only with [`Signatures::Required`].
    TagUnverified {
        feature: String,
        version: u32,
        reason: String,
    },
    TagMisnamed {
        feature: String,
        version: u32,
        object_name: Option<String>,
    },
    /// The tagged commit's manifest does not list the feature at this version.
    TagManifestMismatch {
        feature: String,
        version: u32,
        found: Option<u32>,
    },
    TagCoversNothing {
        feature: String,
        version: u32,
    },
    VersionDown {
        feature: String,
        base: u32,
        head: u32,
    },
    Removed {
        feature: String,
        base: u32,
    },
    Changed {
        path: String,
        feature: String,
        version: u32,
    },
    Deleted {
        path: String,
        feature: String,
        version: u32,
    },
    Untagged {
        path: String,
        hint: Untagged,
    },
}

/// Why a contract file is not covered, so the message can say what to do.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Untagged {
    /// No feature in HEAD's manifest lists it.
    Unregistered,
    /// A feature lists it, but that feature's tag is missing or invalid.
    AwaitingTag { feature: String, version: u32 },
    /// A feature lists it and its tag is valid, but the tag does not contain it.
    NotInTag { feature: String, version: u32 },
}

impl Problem {
    /// Whether this finding only awaits its contract tag (the tag is missing, or a contract
    /// file isn't covered by any tag), rather than violating a pinned contract (a changed or
    /// deleted tagged file, a lower version, a removed feature, an inconsistent tag, or, with
    /// [`Signatures::Required`], a bad or unknown signature).
    pub(crate) fn is_pending(&self) -> bool {
        matches!(self, Self::TagMissing { .. } | Self::Untagged { .. })
    }

    /// The short form shown in a `contract-status` row.
    fn short(&self) -> String {
        match self {
            Self::TagMissing { .. } => "tag not found".to_owned(),
            Self::TagUnverified { .. } => "not signed by an allowed key".to_owned(),
            Self::TagMisnamed { object_name, .. } => match object_name {
                Some(name) => format!("tag object is named {name}"),
                None => "lightweight tag".to_owned(),
            },
            Self::TagManifestMismatch { .. } => "tagged manifest pins another version".to_owned(),
            Self::TagCoversNothing { .. } => "tag covers no files".to_owned(),
            other => other.to_string(),
        }
    }
}

/// Who pins a contract version, and how.
fn pin(feature: &str, version: u32) -> String {
    format!(
        "the coordinating session pins {} with `just contract-pin {feature} {version}` and \
         pushes it with `just contract-push {feature} {version}`",
        tag_name(feature, version)
    )
}

fn restore_or_bump(path: &str, feature: &str, version: u32) -> String {
    let tag = tag_name(feature, version);
    let next = version + 1;
    format!(
        "restore it with `git checkout {tag} -- {path}`, or bump {feature} to version {next} in \
         {MANIFEST} and {}",
        pin(feature, next)
    )
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TagMissing { feature, version } => write!(
                f,
                "{}: tag not found: once the {feature} v{version} contract tests are committed, {}",
                tag_name(feature, *version),
                pin(feature, *version)
            ),
            Self::TagUnverified {
                feature,
                version,
                reason,
            } => write!(
                f,
                "{} is not signed by an allowed key in the base's {SIGNERS} (git: {reason}); \
                 with --require-signatures, only a tag signed by a key in that file pins a \
                 contract",
                tag_name(feature, *version)
            ),
            Self::TagMisnamed {
                feature,
                version,
                object_name,
            } => write!(
                f,
                "refs/tags/{} holds the tag object named {}: only an annotated tag made under \
                 its own name pins a contract (`just contract-pin {feature} {version}` makes \
                 one); ask the owner to remove the wrong ref",
                tag_name(feature, *version),
                object_name.as_deref().unwrap_or("(none: lightweight tag)"),
            ),
            Self::TagManifestMismatch {
                feature,
                version,
                found,
            } => {
                let found = found.map_or_else(
                    || format!("does not list {feature}"),
                    |v| format!("has {feature} at version {v}"),
                );
                write!(
                    f,
                    "{}: the tagged commit's {MANIFEST} {found}; a contract tag points at the \
                     commit that sets {feature} to version {version}",
                    tag_name(feature, *version)
                )
            }
            Self::TagCoversNothing { feature, version } => write!(
                f,
                "{}: no file in the tagged commit matches {feature}'s paths; fix the paths, bump \
                 the version and {}",
                tag_name(feature, *version),
                pin(feature, version + 1)
            ),
            Self::VersionDown {
                feature,
                base,
                head,
            } => write!(
                f,
                "{feature}: contract version went down from {base} to {head}; versions only go \
                 up: to change the contract, bump it to {}",
                base + 1
            ),
            Self::Removed { feature, base } => write!(
                f,
                "{feature}: removed from {MANIFEST} (the base has v{base}); a pull request never \
                 removes a contract: restore the entry, or ask the owner to retire it"
            ),
            Self::Changed {
                path,
                feature,
                version,
            } => write!(
                f,
                "{path} changed since {}: {}",
                tag_name(feature, *version),
                restore_or_bump(path, feature, *version)
            ),
            Self::Deleted {
                path,
                feature,
                version,
            } => write!(
                f,
                "{path} was deleted since {}: {}",
                tag_name(feature, *version),
                restore_or_bump(path, feature, *version)
            ),
            Self::Untagged { path, hint } => write!(f, "untagged contract file {path}: {hint}"),
        }
    }
}

/// What to do about an untagged contract file.
impl fmt::Display for Untagged {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unregistered => write!(
                f,
                "register it in {MANIFEST} under a feature and commit; then the coordinating \
                 session pins that version with `just contract-pin <feature> <version>`"
            ),
            Self::AwaitingTag { feature, version } => f.write_str(&pin(feature, *version)),
            Self::NotInTag { feature, version } => write!(
                f,
                "it matches {feature}'s paths but is not in {}; bump {feature} to version {} in \
                 {MANIFEST} and {}",
                tag_name(feature, *version),
                version + 1,
                pin(feature, version + 1)
            ),
        }
    }
}

/// The allowed-signers text tags are verified with ([`Signatures::Required`] only): the
/// base's copy, except while the base predates the contract system (no manifest and no
/// signers file). There is no trusted copy yet then, so HEAD's is used and the run says so
/// loudly; the owner's review of the pull request that adds the file is the only protection
/// in that window.
fn trusted_signers(
    repo: &impl Repo,
    base: &str,
    head: &str,
    base_has_manifest: bool,
    notices: &mut Vec<String>,
) -> anyhow::Result<String> {
    if let Some(signers) = repo.read(base, SIGNERS)? {
        return Ok(signers);
    }
    if base_has_manifest {
        notices.push(format!(
            "the base has no {SIGNERS}, so no contract tag can verify: the owner adds their \
             signing key to it on the base branch first"
        ));
        return Ok(String::new());
    }
    notices.push(format!(
        "BOOTSTRAP: the base has no {MANIFEST} and no {SIGNERS}, so tags are verified with \
         HEAD's {SIGNERS}. Only the owner's review of this change protects that file; once the \
         contract system is on the base branch, only the base's copy counts"
    ));
    Ok(repo.read(head, SIGNERS)?.unwrap_or_default())
}

/// Checks `head` against the contract tags, trusting only `base` for minimum versions and,
/// with [`Signatures::Required`], for signers (see [`trusted_signers`] for the one bootstrap
/// exception). Both are revisions git can resolve, ideally full commit ids.
pub(crate) fn check(
    repo: &impl Repo,
    base: &str,
    head: &str,
    signatures: Signatures,
) -> anyhow::Result<Report> {
    let mut report = Report::default();
    let base_manifest =
        load_manifest(repo, base).with_context(|| format!("parsing {MANIFEST} at the base"))?;
    let head_manifest =
        load_manifest(repo, head).with_context(|| format!("parsing {MANIFEST} at the head"))?;
    if base_manifest.is_none() {
        report.notices.push(format!(
            "the base has no {MANIFEST}: versions are not compared with it"
        ));
    }
    let signers = match signatures {
        Signatures::Required => Some(trusted_signers(
            repo,
            base,
            head,
            base_manifest.is_some(),
            &mut report.notices,
        )?),
        Signatures::Ignored => None,
    };
    let base_manifest = base_manifest.unwrap_or_default();
    let head_manifest = head_manifest.unwrap_or_default();
    let head_files = repo.files(head)?;

    let mut covered = BTreeSet::new();
    let mut approved = BTreeSet::new();
    for (name, feature) in &head_manifest.features {
        if let Some(base_feature) = base_manifest.features.get(name)
            && feature.version < base_feature.version
        {
            report.problems.push(Problem::VersionDown {
                feature: name.clone(),
                base: base_feature.version,
                head: feature.version,
            });
        }
        let state = match tagged_files(repo, name, feature.version, signers.as_deref())? {
            Ok(files) => {
                approved.insert(name.as_str());
                let differences = compare(name, feature.version, &files, &head_files);
                let state = tag_state(signatures, files.len(), differences.len());
                report.problems.extend(differences);
                covered.extend(files.into_keys());
                state
            }
            Err(problem) => {
                let state = problem.short();
                report.problems.push(problem);
                state
            }
        };
        report.features.push(FeatureStatus {
            name: name.clone(),
            version: feature.version,
            state,
        });
    }
    for (name, feature) in &base_manifest.features {
        if !head_manifest.features.contains_key(name) {
            report.problems.push(Problem::Removed {
                feature: name.clone(),
                base: feature.version,
            });
        }
    }

    let globs: BTreeSet<&str> = base_manifest
        .contract_globs
        .iter()
        .chain(&head_manifest.contract_globs)
        .map(String::as_str)
        .collect();
    for path in head_files.keys() {
        if covered.contains(path) || !globs.iter().any(|glob| glob_matches(glob, path)) {
            continue;
        }
        let hint = match head_manifest.owner_of(path) {
            None => Untagged::Unregistered,
            Some((feature, entry)) if approved.contains(feature) => Untagged::NotInTag {
                feature: feature.to_owned(),
                version: entry.version,
            },
            Some((feature, entry)) => Untagged::AwaitingTag {
                feature: feature.to_owned(),
                version: entry.version,
            },
        };
        report.problems.push(Problem::Untagged {
            path: path.clone(),
            hint,
        });
    }
    report.pinned = covered.len();
    Ok(report)
}

fn load_manifest(repo: &impl Repo, rev: &str) -> anyhow::Result<Option<Manifest>> {
    repo.read(rev, MANIFEST)?
        .map(|text| Manifest::parse(&text))
        .transpose()
}

/// The files `contract/<feature>/v<version>` pins, with their blob ids, if the tag is
/// valid; otherwise the problem with it. With `signers` (only under
/// [`Signatures::Required`]), the tag must also verify against them.
fn tagged_files(
    repo: &impl Repo,
    feature: &str,
    version: u32,
    signers: Option<&str>,
) -> anyhow::Result<Result<BTreeMap<String, String>, Problem>> {
    let tag = tag_name(feature, version);
    let (feature_owned, rev) = (feature.to_owned(), format!("refs/tags/{tag}"));
    let Some(tag_ref) = repo.tag(&tag)? else {
        return Ok(Err(Problem::TagMissing {
            feature: feature_owned,
            version,
        }));
    };
    if let Some(signers) = signers
        && let Err(reason) = repo.verify_tag(&tag, signers)?
    {
        return Ok(Err(Problem::TagUnverified {
            feature: feature_owned,
            version,
            reason,
        }));
    }
    if tag_ref.object_name.as_deref() != Some(tag.as_str()) {
        return Ok(Err(Problem::TagMisnamed {
            feature: feature_owned,
            version,
            object_name: tag_ref.object_name,
        }));
    }
    // A tagged manifest this checker cannot parse pins nothing.
    let tagged = load_manifest(repo, &rev).ok().flatten().unwrap_or_default();
    let Some(entry) = tagged
        .features
        .get(feature)
        .filter(|entry| entry.version == version)
    else {
        return Ok(Err(Problem::TagManifestMismatch {
            feature: feature_owned,
            version,
            found: tagged.features.get(feature).map(|entry| entry.version),
        }));
    };
    let files: BTreeMap<String, String> = repo
        .files(&rev)?
        .into_iter()
        .filter(|(path, _)| entry.matches(path))
        .collect();
    if files.is_empty() {
        return Ok(Err(Problem::TagCoversNothing {
            feature: feature_owned,
            version,
        }));
    }
    Ok(Ok(files))
}

/// Every tagged file that differs from, or is missing at, HEAD.
fn compare(
    feature: &str,
    version: u32,
    tagged: &BTreeMap<String, String>,
    head: &BTreeMap<String, String>,
) -> Vec<Problem> {
    tagged
        .iter()
        .filter_map(|(path, blob)| {
            let (path, feature) = (path.clone(), feature.to_owned());
            match head.get(&path) {
                None => Some(Problem::Deleted {
                    path,
                    feature,
                    version,
                }),
                Some(head_blob) if head_blob != blob => Some(Problem::Changed {
                    path,
                    feature,
                    version,
                }),
                Some(_) => None,
            }
        })
        .collect()
}

/// The `contract-status` state of a valid tag: `signed, …` when signatures are required,
/// `pinned, …` otherwise.
fn tag_state(signatures: Signatures, files: usize, changed: usize) -> String {
    let verb = signatures.verb();
    if changed == 0 {
        format!("{verb}, {files} file(s) unchanged")
    } else {
        format!("{verb}, {changed} of {files} file(s) changed or deleted")
    }
}
