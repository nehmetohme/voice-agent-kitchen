//! Reads the workspace through `cargo metadata` and converts it into the plain data the
//! checks work on.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Context as _;
use cargo_metadata::{DependencyKind, Metadata, MetadataCommand};

use crate::layers::{Crate, Dep, Kind};

/// `cargo metadata --no-deps` for the workspace that owns `manifest` (or the current
/// directory). Only workspace members and their declared dependencies are needed.
pub(crate) fn metadata(manifest: Option<&Path>) -> anyhow::Result<Metadata> {
    let mut command = MetadataCommand::new();
    command.no_deps();
    if let Some(manifest) = manifest {
        command.manifest_path(manifest);
    }
    command.exec().context("running `cargo metadata`")
}

/// Every workspace member with the dependencies its manifest declares.
pub(crate) fn crates(metadata: &Metadata) -> Vec<Crate> {
    let packages = metadata.workspace_packages();
    let members: BTreeSet<&str> = packages.iter().map(|p| p.name.as_str()).collect();
    let root = metadata.workspace_root.as_std_path();

    packages
        .iter()
        .map(|package| Crate {
            name: package.name.to_string(),
            manifest: relative(package.manifest_path.as_std_path(), root),
            deps: package
                .dependencies
                .iter()
                .map(|dep| Dep {
                    name: dep.name.clone(),
                    kind: kind(dep.kind),
                    target: dep.target.as_ref().map(ToString::to_string),
                    internal: members.contains(dep.name.as_str()),
                })
                .collect(),
        })
        .collect()
}

fn kind(kind: DependencyKind) -> Kind {
    match kind {
        DependencyKind::Development => Kind::Dev,
        DependencyKind::Build => Kind::Build,
        // Normal, and any kind added to cargo later: check it as strictly as a normal dep.
        _ => Kind::Normal,
    }
}

fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers;

    fn this_workspace() -> Metadata {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../Cargo.toml");
        metadata(Some(&manifest)).expect("cargo metadata succeeds")
    }

    #[test]
    fn reads_members_with_relative_manifests() {
        let crates = crates(&this_workspace());

        let api = crates
            .iter()
            .find(|c| c.name == "vak-api")
            .expect("vak-api is a member");
        assert_eq!(api.manifest, "crates/api/Cargo.toml");
        let xtask = crates
            .iter()
            .find(|c| c.name == "vak-xtask")
            .expect("xtask is a member");
        let anyhow = xtask
            .deps
            .iter()
            .find(|d| d.name == "anyhow")
            .expect("xtask uses anyhow");
        assert!(!anyhow.internal);
        assert_eq!(anyhow.kind, Kind::Normal);
    }

    #[test]
    fn this_workspace_satisfies_layers_toml() {
        let config = layers::Config::parse(include_str!("../../layers.toml")).expect("parses");

        let violations = layers::check(&config, &crates(&this_workspace()));

        let report: Vec<String> = violations.iter().map(ToString::to_string).collect();
        assert!(violations.is_empty(), "{}", report.join("\n"));
    }

    #[test]
    fn dependency_kinds_map_to_sections() {
        assert_eq!(kind(DependencyKind::Normal), Kind::Normal);
        assert_eq!(kind(DependencyKind::Development), Kind::Dev);
        assert_eq!(kind(DependencyKind::Build), Kind::Build);
    }
}
