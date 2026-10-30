use super::*;

/// A small layered workspace: `domain` ← `ports` ← `app`, a dev-only `kit` and a `tool`.
const FIXTURE: &str = r#"
dev-only = ["kit"]

[crates.domain]
deps = []
external = ["serde"]
external-dev = ["proptest"]

[crates.ports]
deps = ["domain"]
dev-deps = ["kit"]

[crates.app]
deps = ["ports", "domain"]
dev-deps = ["kit", "tool"]

[crates.kit]
deps = ["domain", "ports"]

[crates.tool]
deps = []
"#;

fn config(text: &str) -> Config {
    Config::parse(text).expect("fixture layers.toml parses")
}

fn internal(name: &str, kind: Kind) -> Dep {
    Dep { name: name.to_owned(), kind, target: None, internal: true }
}

fn external(name: &str, kind: Kind) -> Dep {
    Dep { name: name.to_owned(), kind, target: None, internal: false }
}

fn krate(name: &str, deps: Vec<Dep>) -> Crate {
    Crate { name: name.to_owned(), manifest: format!("crates/{name}/Cargo.toml"), deps }
}

/// The fixture workspace with every edge allowed.
fn clean_workspace() -> Vec<Crate> {
    vec![
        krate("domain", vec![external("serde", Kind::Normal), external("proptest", Kind::Dev)]),
        krate("ports", vec![internal("domain", Kind::Normal), internal("kit", Kind::Dev)]),
        krate(
            "app",
            vec![
                internal("ports", Kind::Normal),
                internal("domain", Kind::Normal),
                internal("kit", Kind::Dev),
                internal("tool", Kind::Dev),
                external("tokio", Kind::Normal),
            ],
        ),
        krate("kit", vec![internal("domain", Kind::Normal), internal("ports", Kind::Normal)]),
        krate("tool", vec![]),
    ]
}

/// The clean workspace plus one extra dependency on `on`.
fn with_dep(on: &str, dep: Dep) -> Vec<Crate> {
    let mut crates = clean_workspace();
    let target = crates.iter_mut().find(|c| c.name == on).expect("fixture crate exists");
    target.deps.push(dep);
    crates
}

fn messages(violations: &[Violation]) -> Vec<String> {
    violations.iter().map(ToString::to_string).collect()
}

#[test]
fn clean_workspace_has_no_violations() {
    assert_eq!(check(&config(FIXTURE), &clean_workspace()), vec![]);
}

#[test]
fn disallowed_normal_edge_names_crate_dependency_and_manifest() {
    let found = check(&config(FIXTURE), &with_dep("domain", internal("app", Kind::Normal)));

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(matches!(&found[0], Violation::Disallowed { at, .. } if at.krate == "domain" && at.dep == "app"));
    let message = found[0].to_string();
    for part in ["crates/domain/Cargo.toml", "[dependencies]", "domain", "app", "none"] {
        assert!(message.contains(part), "{message:?} should mention {part:?}");
    }
}

#[test]
fn disallowed_dev_edge_is_reported() {
    let found = check(&config(FIXTURE), &with_dep("ports", internal("app", Kind::Dev)));

    assert_eq!(found.len(), 1, "{found:?}");
    let message = found[0].to_string();
    assert!(message.contains("[dev-dependencies]"), "{message}");
    assert!(message.contains("ports") && message.contains("app"), "{message}");
}

#[test]
fn disallowed_build_edge_is_reported() {
    let found = check(&config(FIXTURE), &with_dep("ports", internal("app", Kind::Build)));

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].to_string().contains("[build-dependencies]"), "{found:?}");
}

#[test]
fn allowed_list_is_shown_for_a_disallowed_edge() {
    let found = check(&config(FIXTURE), &with_dep("ports", internal("tool", Kind::Normal)));

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].to_string().contains("domain"), "{}", found[0]);
}

#[test]
fn dev_deps_entries_are_not_allowed_as_normal_dependencies() {
    let found = check(&config(FIXTURE), &with_dep("app", internal("tool", Kind::Normal)));

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(matches!(&found[0], Violation::Disallowed { at, .. } if at.dep == "tool"));
}

#[test]
fn dev_only_crate_as_normal_dependency_is_reported() {
    let found = check(&config(FIXTURE), &with_dep("app", internal("kit", Kind::Normal)));

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(matches!(&found[0], Violation::DevOnly { at } if at.krate == "app" && at.dep == "kit"));
    let message = found[0].to_string();
    assert!(message.contains("dev-only") && message.contains("kit"), "{message}");
}

#[test]
fn dev_only_crate_as_build_dependency_is_reported() {
    let found = check(&config(FIXTURE), &with_dep("app", internal("kit", Kind::Build)));

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(matches!(&found[0], Violation::DevOnly { .. }), "{found:?}");
}

#[test]
fn external_outside_allowlist_is_reported() {
    let found = check(&config(FIXTURE), &with_dep("domain", external("tokio", Kind::Normal)));

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(matches!(&found[0], Violation::External { at, .. } if at.krate == "domain" && at.dep == "tokio"));
    let message = found[0].to_string();
    assert!(message.contains("tokio") && message.contains("serde"), "{message}");
}

#[test]
fn external_build_dependency_outside_allowlist_is_reported() {
    let found = check(&config(FIXTURE), &with_dep("domain", external("cc", Kind::Build)));

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(matches!(&found[0], Violation::External { .. }), "{found:?}");
}

#[test]
fn external_dev_entries_are_allowed_only_as_dev_dependencies() {
    let found = check(&config(FIXTURE), &with_dep("domain", external("proptest", Kind::Normal)));

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(matches!(&found[0], Violation::External { .. }), "{found:?}");
}

#[test]
fn external_allowlist_also_covers_dev_dependencies() {
    let found = check(&config(FIXTURE), &with_dep("domain", external("serde", Kind::Dev)));

    assert_eq!(found, vec![]);
}

#[test]
fn unknown_external_dev_dependency_is_reported_when_allowlist_is_set() {
    let found = check(&config(FIXTURE), &with_dep("domain", external("tokio", Kind::Dev)));

    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn crates_without_external_allowlist_accept_any_external() {
    let found = check(&config(FIXTURE), &with_dep("ports", external("anything", Kind::Normal)));

    assert_eq!(found, vec![]);
}

#[test]
fn unlisted_workspace_member_is_reported() {
    let mut crates = clean_workspace();
    crates.push(krate("newcomer", vec![]));

    let found = check(&config(FIXTURE), &crates);

    assert_eq!(found.len(), 1, "{found:?}");
    let message = found[0].to_string();
    assert!(message.contains("newcomer") && message.contains("crates/newcomer/Cargo.toml"), "{message}");
}

#[test]
fn stale_crate_entry_is_reported() {
    let stale = format!("{FIXTURE}\n[crates.gone]\ndeps = []\n");

    let found = check(&config(&stale), &clean_workspace());

    assert_eq!(found, vec![Violation::UnknownCrate { entry: "[crates.gone]".into(), name: "gone".into() }]);
    assert!(found[0].to_string().contains("gone"));
}

#[test]
fn unknown_names_in_rules_are_reported() {
    let text = FIXTURE
        .replace(r#"deps = ["ports", "domain"]"#, r#"deps = ["ports", "domain", "ghost"]"#)
        .replace(r#"dev-only = ["kit"]"#, r#"dev-only = ["kit", "phantom"]"#);

    let found = check(&config(&text), &clean_workspace());

    let text = messages(&found).join("\n");
    assert_eq!(found.len(), 2, "{text}");
    assert!(text.contains("ghost") && text.contains("phantom"), "{text}");
}

#[test]
fn dev_only_crate_listed_under_deps_is_a_config_error() {
    let text = FIXTURE.replace(r#"deps = ["ports", "domain"]"#, r#"deps = ["ports", "domain", "kit"]"#);

    let found = check(&config(&text), &clean_workspace());

    assert_eq!(found, vec![Violation::DevOnlyInDeps { krate: "app".into(), dep: "kit".into() }]);
    assert!(found[0].to_string().contains("dev-deps"));
}

#[test]
fn unknown_keys_are_rejected() {
    assert!(Config::parse("[crates.a]\ndep = []\n").is_err());
    assert!(Config::parse("[crates.a]\n").is_err(), "deps is required");
    assert!(Config::parse("devonly = []\n[crates.a]\ndeps = []\n").is_err());
}

#[test]
fn target_specific_section_is_named_in_the_message() {
    let dep = Dep { target: Some("cfg(unix)".into()), ..internal("app", Kind::Dev) };

    let found = check(&config(FIXTURE), &with_dep("ports", dep));

    assert_eq!(found.len(), 1, "{found:?}");
    let message = found[0].to_string();
    assert!(message.contains("[target.'cfg(unix)'.dev-dependencies]"), "{message}");
}

#[test]
fn violations_are_reported_in_a_stable_order() {
    let mut crates = with_dep("ports", internal("app", Kind::Normal));
    crates.iter_mut().find(|c| c.name == "domain").expect("domain").deps.push(internal("app", Kind::Normal));
    crates.reverse();

    let found = check(&config(FIXTURE), &crates);

    let names: Vec<&str> = found
        .iter()
        .map(|v| match v {
            Violation::Disallowed { at, .. } => at.krate.as_str(),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(names, ["domain", "ports"]);
}

/// Guards on the repository's own `layers.toml`: the research table's core rules.
mod repo_layers_toml {
    use super::*;

    fn repo() -> Config {
        config(include_str!("../../../layers.toml"))
    }

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    #[test]
    fn domain_is_pure() {
        let config = repo();
        let domain = &config.crates["vak-domain"];
        assert!(domain.deps.is_empty() && domain.dev_deps.is_empty());
        assert_eq!(domain.external, Some(set(&["serde", "thiserror", "uuid", "jiff"])));
    }

    #[test]
    fn testkit_and_contract_tests_are_dev_only() {
        assert_eq!(repo().dev_only, set(&["vak-testkit", "vak-contract-tests"]));
    }

    #[test]
    fn api_reaches_the_rest_only_through_services() {
        assert_eq!(repo().crates["vak-api"].deps, set(&["vak-services", "vak-domain"]));
    }

    #[test]
    fn config_and_telemetry_depend_on_nothing_internal() {
        let config = repo();
        for name in ["vak-config", "vak-telemetry"] {
            assert!(config.crates[name].deps.is_empty(), "{name}");
        }
    }
}
