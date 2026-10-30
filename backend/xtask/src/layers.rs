//! `check-layers`: compare the dependency edges declared in every workspace manifest with
//! `layers.toml`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Deserialize;

#[cfg(test)]
mod tests;

/// Parsed `layers.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct Config {
    /// Crates that may only ever be dev-dependencies.
    #[serde(default)]
    pub(crate) dev_only: BTreeSet<String>,
    /// One rule per workspace member, keyed by package name.
    pub(crate) crates: BTreeMap<String, Rule>,
}

/// What one crate may depend on.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct Rule {
    /// Workspace crates allowed as normal, build and dev dependencies.
    pub(crate) deps: BTreeSet<String>,
    /// Workspace crates allowed only as dev-dependencies.
    #[serde(default)]
    pub(crate) dev_deps: BTreeSet<String>,
    /// If set, the only external crates allowed as normal or build dependencies.
    #[serde(default)]
    pub(crate) external: Option<BTreeSet<String>>,
    /// Extra external crates allowed as dev-dependencies when `external` is set.
    #[serde(default)]
    pub(crate) external_dev: BTreeSet<String>,
}

impl Config {
    pub(crate) fn parse(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }
}

/// A workspace member and the dependencies its manifest declares.
#[derive(Clone, Debug)]
pub(crate) struct Crate {
    pub(crate) name: String,
    /// Manifest path relative to the workspace root, used in messages.
    pub(crate) manifest: String,
    pub(crate) deps: Vec<Dep>,
}

/// One dependency declaration.
#[derive(Clone, Debug)]
pub(crate) struct Dep {
    /// Package name (a `package = "..."` rename is already resolved).
    pub(crate) name: String,
    pub(crate) kind: Kind,
    /// `cfg(...)` or target triple for `[target.<t>.*dependencies]`.
    pub(crate) target: Option<String>,
    /// Whether the dependency is a workspace member.
    pub(crate) internal: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    Normal,
    Dev,
    Build,
}

/// One broken rule. `Display` gives the message shown to the user.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Violation {
    /// A workspace member has no `[crates.<name>]` entry.
    Unlisted { krate: String, manifest: String },
    /// `layers.toml` names a crate that is not a workspace member.
    UnknownCrate { entry: String, name: String },
    /// A dev-only crate is listed under `deps` in `layers.toml`.
    DevOnlyInDeps { krate: String, dep: String },
    /// An internal edge that `layers.toml` does not allow for this dependency kind.
    Disallowed { at: Location, allowed: Vec<String> },
    /// A dev-only crate used as a normal or build dependency.
    DevOnly { at: Location },
    /// An external dependency outside the crate's `external` allowlist.
    External { at: Location, allowed: Vec<String> },
}

/// Where a dependency is declared.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Location {
    pub(crate) krate: String,
    pub(crate) dep: String,
    pub(crate) manifest: String,
    pub(crate) section: String,
}

impl Dep {
    /// The manifest table this dependency is declared in, e.g. `[dev-dependencies]`.
    fn section(&self) -> String {
        let table = match self.kind {
            Kind::Normal => "dependencies",
            Kind::Dev => "dev-dependencies",
            Kind::Build => "build-dependencies",
        };
        match &self.target {
            Some(target) => format!("[target.'{target}'.{table}]"),
            None => format!("[{table}]"),
        }
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unlisted { krate, manifest } => write!(
                f,
                "{manifest}: {krate} has no [crates.{krate}] entry in layers.toml (ask the owner to add one)"
            ),
            Self::UnknownCrate { entry, name } => {
                write!(f, "layers.toml {entry}: {name} is not a workspace member")
            }
            Self::DevOnlyInDeps { krate, dep } => write!(
                f,
                "layers.toml [crates.{krate}] deps: {dep} is dev-only; list it under dev-deps"
            ),
            Self::Disallowed { at, allowed } => write!(
                f,
                "{at}: {krate} may not depend on {dep} (layers.toml allows: {list})",
                krate = at.krate,
                dep = at.dep,
                list = list(allowed),
            ),
            Self::DevOnly { at } => write!(
                f,
                "{at}: {dep} is dev-only; {krate} may use it only under [dev-dependencies]",
                krate = at.krate,
                dep = at.dep,
            ),
            Self::External { at, allowed } => write!(
                f,
                "{at}: {krate} may not use external crate {dep} (layers.toml allows: {list})",
                krate = at.krate,
                dep = at.dep,
                list = list(allowed),
            ),
        }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.manifest, self.section, self.dep)
    }
}

fn list(names: &[String]) -> String {
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}

/// Every violation of `config` in `crates`, in a stable order.
pub(crate) fn check(config: &Config, crates: &[Crate]) -> Vec<Violation> {
    let members: BTreeSet<&str> = crates.iter().map(|c| c.name.as_str()).collect();
    let mut found = check_config(config, &members);

    for krate in crates {
        let Some(rule) = config.crates.get(&krate.name) else {
            found.push(Violation::Unlisted {
                krate: krate.name.clone(),
                manifest: krate.manifest.clone(),
            });
            continue;
        };
        for dep in &krate.deps {
            if let Some(violation) = check_dep(config, rule, krate, dep) {
                found.push(violation);
            }
        }
    }

    found.sort();
    found.dedup();
    found
}

/// Problems in `layers.toml` itself: names that are not workspace members, and dev-only
/// crates listed as normal deps.
fn check_config(config: &Config, members: &BTreeSet<&str>) -> Vec<Violation> {
    let mut found = Vec::new();
    let mut unknown = |entry: String, name: &String| {
        if !members.contains(name.as_str()) {
            found.push(Violation::UnknownCrate {
                entry,
                name: name.clone(),
            });
        }
    };

    for name in &config.dev_only {
        unknown("dev-only".to_owned(), name);
    }
    for (krate, rule) in &config.crates {
        unknown(format!("[crates.{krate}]"), krate);
        for name in &rule.deps {
            unknown(format!("[crates.{krate}] deps"), name);
        }
        for name in &rule.dev_deps {
            unknown(format!("[crates.{krate}] dev-deps"), name);
        }
    }
    for (krate, rule) in &config.crates {
        for dep in rule.deps.intersection(&config.dev_only) {
            found.push(Violation::DevOnlyInDeps {
                krate: krate.clone(),
                dep: dep.clone(),
            });
        }
    }
    found
}

fn check_dep(config: &Config, rule: &Rule, krate: &Crate, dep: &Dep) -> Option<Violation> {
    let at = || Location {
        krate: krate.name.clone(),
        dep: dep.name.clone(),
        manifest: krate.manifest.clone(),
        section: dep.section(),
    };
    let is_dev = dep.kind == Kind::Dev;

    if dep.internal {
        if !is_dev && config.dev_only.contains(&dep.name) {
            return Some(Violation::DevOnly { at: at() });
        }
        let allowed =
            rule.deps.contains(&dep.name) || (is_dev && rule.dev_deps.contains(&dep.name));
        if allowed {
            return None;
        }
        let mut names: Vec<String> = rule.deps.iter().cloned().collect();
        if is_dev {
            names.extend(rule.dev_deps.iter().cloned());
            names.sort();
        }
        return Some(Violation::Disallowed {
            at: at(),
            allowed: names,
        });
    }

    let external = rule.external.as_ref()?;
    let allowed = external.contains(&dep.name) || (is_dev && rule.external_dev.contains(&dep.name));
    if allowed {
        return None;
    }
    let mut names: Vec<String> = external.iter().cloned().collect();
    if is_dev {
        names.extend(rule.external_dev.iter().cloned());
        names.sort();
    }
    Some(Violation::External {
        at: at(),
        allowed: names,
    })
}
