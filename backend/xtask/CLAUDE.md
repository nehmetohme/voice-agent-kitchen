# vak-xtask: workspace automation

## Responsibility

This crate builds the `xtask` binary, run as `cargo xtask <command>` from `backend/`. The alias is defined in `backend/.cargo/config.toml`.

| Command | Does |
|---|---|
| `check-layers` | Reads `layers.toml` and `cargo metadata --no-deps`, then fails on any of the following (see below). |
| `coverage-gate <summary.json>` | Reads `cargo llvm-cov report --json --summary-only` output and `coverage.toml`. Fails any crate below its floor, and fails if total coverage is below the global floor. |

`check-layers` fails on:
- any internal edge (normal, dev or build) that `layers.toml` doesn't allow
- an external dependency outside a crate's `external` allowlist
- a dev-only crate (`vak-testkit`, `vak-contract-tests`) used as a normal or build dependency
- a workspace member with no `layers.toml` entry, or an entry naming a crate that doesn't exist

`coverage-gate` also follows these rules:
- crates with zero instrumented lines pass
- floors naming unknown crates are errors

Planned commands: `openapi` (export `contracts/openapi.json`) and `contract-check` (contract files byte-identical to their signed tags).

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

## Coverage floor

`xtask/` is excluded from coverage. Its rules are still fully unit-tested.

## Example

```sh
cd backend
cargo xtask check-layers
cargo llvm-cov report --json --summary-only --output-path target/llvm-cov/summary.json
cargo xtask coverage-gate target/llvm-cov/summary.json
```
