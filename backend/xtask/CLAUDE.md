# vak-xtask: workspace automation

## Responsibility

This crate builds the `xtask` binary, run as `cargo xtask <command>` from `backend/`. The alias is defined in `backend/.cargo/config.toml`.

| Command | Does |
|---|---|
| `check-layers` | Reads `layers.toml` and `cargo metadata --no-deps`, then fails on any of the following (see below). |
| `coverage-gate <summary.json>` | Reads `cargo llvm-cov report --json --summary-only` output and `coverage.toml`. Fails any crate below its floor, and fails if total coverage is below the global floor. |
| `contract-check --base <ref> [--head <ref>]` | Fails unless every contract file at HEAD is byte-identical to a `contract/<feature>/v<N>` tag the owner signed (rules below; workflow in `contracts/CLAUDE.md`). |
| `contract-status [--base <ref>] [--head <ref>]` | Lists each contract feature with its version and tag state. Never fails on findings. |

`check-layers` fails on:
- any internal edge (normal, dev or build) that `layers.toml` doesn't allow
- an external dependency outside a crate's `external` allowlist
- a dev-only crate (`vak-testkit`, `vak-contract-tests`) used as a normal or build dependency
- a workspace member with no `layers.toml` entry, or an entry naming a crate that doesn't exist

`coverage-gate` also follows these rules:
- crates with zero instrumented lines pass
- floors naming unknown crates are errors

`contract-check` (design: `docs/research/06-graphify-github-ci.md` §4) fails on:
- a feature in HEAD's `contracts/contracts.toml` whose tag is missing, isn't a tag object with that name, or fails `git verify-tag` against the **base** ref's `.github/contract-signers`
- a version lower than the base's, or a feature the base has that HEAD dropped
- a tag whose own manifest doesn't pin that version, or whose paths match no file
- a file in the tag's file set (from the tag's own manifest) that changed or is missing at HEAD
- a file at HEAD matching any `contract_globs` (the base's or HEAD's) that no valid tag covers ("untagged contract file")

The rules live in `src/contracts.rs` over a `Repo` trait; `src/git.rs` runs `git` through `std::process::Command`. Globs are matched in Rust because `git ls-tree` rejects `:(glob)` pathspecs. CI runs the base branch's copy of this checker (`just contracts-check-ci`).

The OpenAPI export is `vak-server openapi`, not an xtask: xtask links no workspace crate.

## Dependencies

- **May depend on:** no workspace crate. It inspects the workspace through `cargo metadata`, never by linking it.
- **External crates:** `anyhow`, `cargo_metadata`, `clap`, `serde`, `serde_json`, `toml`.

## Conventions

- **Pure core, thin I/O:** each command is a pure function over plain data, for example `layers::check(&Config, &[Crate]) -> Vec<Violation>`. `main` only loads files and metadata and prints results.
- **Messages name the offending crate and dependency,** plus the manifest and section, for example `crates/domain/Cargo.toml [dependencies] vak-api: vak-domain may not depend on vak-api`.
- **Report everything:** collect all violations before failing; don't stop at the first. Output goes to stderr, and the exit code is non-zero on failure.
- **Errors:** `anyhow` is fine here. No `unwrap`/`expect`/`panic!` outside tests.
- **Strict config parsing:** `serde(deny_unknown_fields)` turns a typo in `layers.toml` or `coverage.toml` into an error.
- **Visibility:** binary crate, so items are private or `pub(crate)`.

## Testing

- **Tests come first,** with small fixture data: hand-built crate graphs, TOML strings and llvm-cov JSON snippets.
- **Cover every rule:** each violation kind, each passing case, the boundaries (exactly at the floor), and config errors.
- **Real config:** one test parses the real `layers.toml` and `coverage.toml`, so a broken config fails `nextest` too.
- **contract-check** also has black-box tests in `tests/signed_contract_tags.rs`. They use throwaway repos and `ssh-keygen` keys in the system temp dir. Every `GIT_*`/`SSH_*` variable is removed, `HOME` and git config are isolated, and `GIT_CEILING_DIRECTORIES` is set. They never touch `~/.ssh` or this repository. They need `git` ≥ 2.34 and `ssh-keygen` on `PATH`.

## Coverage floor

`xtask/` is excluded from coverage. Its rules are still fully unit-tested.

## Example

```sh
cd backend
cargo xtask check-layers
cargo llvm-cov report --json --summary-only --output-path target/llvm-cov/summary.json
cargo xtask coverage-gate target/llvm-cov/summary.json
```
