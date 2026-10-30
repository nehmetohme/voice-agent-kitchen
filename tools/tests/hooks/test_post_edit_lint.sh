# shellcheck shell=bash
# post-edit-lint.sh (PostToolUse on Edit|Write): formats and lints the one edited file,
# dispatching on its extension. Failures come back as {"decision":"block","reason":...}.

lint() { # repo-relative-path
  hook_input post-edit-write CWD="$R" FILE="$R/$1"
  run_hook post-edit-lint.sh
}

assert_quiet_success() {
  assert_rc 0
  assert_eq "" "$HOOK_OUT" "a clean lint prints nothing"
}

assert_blocked() { # reason-substring
  assert_rc 0
  assert_json '.decision' block
  assert_contains "$(out_json '.reason')" "$1"
}

backend_package() { # a workspace with one member crate
  write_file backend/Cargo.toml '[workspace]
resolver = "3"
members = ["crates/domain"]'
  write_file backend/crates/domain/Cargo.toml '[package]
name = "vak-domain"
version = "0.1.0"
edition = "2024"'
  write_file backend/crates/domain/src/lib.rs 'pub fn add(a: i32, b: i32) -> i32 { a + b }'
}

frontend_ready() { # package.json plus the formatter and linter binaries
  write_file frontend/package.json '{ "name": "@vak/frontend", "private": true }'
  mkdir -p "$R/frontend/node_modules/.bin"
  ln -s "$STUBS/generic" "$R/frontend/node_modules/.bin/oxfmt"
  ln -s "$STUBS/generic" "$R/frontend/node_modules/.bin/oxlint"
}

test_lint_rust_formats_the_file_then_clippy_on_its_package() {
  make_repo
  stub_bin rustfmt cargo
  backend_package
  lint backend/crates/domain/src/lib.rs
  assert_quiet_success
  assert_contains "$(call_line rustfmt)" "--edition 2024"
  assert_contains "$(call_line rustfmt)" "$R/backend/crates/domain/src/lib.rs"
  assert_contains "$(call_line cargo)" "clippy"
  assert_eq "$R/backend/crates/domain" "$(call_cwd cargo)" "clippy runs in the package dir"
}

test_lint_rust_format_error_blocks() {
  make_repo
  stub_bin rustfmt cargo
  backend_package
  export STUB_RUSTFMT_RC=1 STUB_RUSTFMT_OUT="error: expected one of ! or ::, found x"
  lint backend/crates/domain/src/lib.rs
  assert_blocked "expected one of"
  assert_eq 0 "$(calls cargo)" "no clippy after a format error"
}

test_lint_rust_clippy_error_blocks() {
  make_repo
  stub_bin rustfmt cargo
  backend_package
  export STUB_CARGO_RC=101 STUB_CARGO_OUT="error: unused variable: x"
  lint backend/crates/domain/src/lib.rs
  assert_blocked "unused variable"
}

test_lint_rust_outside_a_package_skips_clippy() {
  make_repo
  stub_bin rustfmt cargo
  write_file scripts/tool.rs 'fn main() {}'
  lint scripts/tool.rs
  assert_quiet_success
  assert_eq 1 "$(calls rustfmt)"
  assert_eq 0 "$(calls cargo)"
}

test_lint_rust_without_tools_skips() {
  make_repo
  backend_package
  lint backend/crates/domain/src/lib.rs
  assert_quiet_success
}

test_lint_rust_slow_clippy_does_not_block() {
  make_repo
  stub_bin rustfmt cargo
  backend_package
  export VAK_LINT_BUDGET_S=1 STUB_CARGO_SLEEP=4 STUB_CARGO_RC=101 STUB_CARGO_OUT="error: late"
  lint backend/crates/domain/src/lib.rs
  assert_rc 0
  assert_not_contains "$HOOK_OUT" '"block"' "a timeout is not a lint failure"
}

test_lint_ts_runs_frontend_format_and_lint_on_the_file() {
  make_repo
  frontend_ready
  write_file frontend/src/features/agents/views/AgentList.tsx 'export const A = () => null'
  lint frontend/src/features/agents/views/AgentList.tsx
  assert_quiet_success
  assert_contains "$(call_line oxfmt)" "src/features/agents/views/AgentList.tsx"
  assert_eq "$R/frontend" "$(call_cwd oxfmt)" "formatter runs in frontend/"
  assert_contains "$(call_line oxlint)" "src/features/agents/views/AgentList.tsx"
  assert_eq "$R/frontend" "$(call_cwd oxlint)" "linter runs in frontend/"
}

test_lint_ts_error_blocks() {
  make_repo
  frontend_ready
  write_file frontend/src/shared/lib/format.ts 'export const x = 1'
  export STUB_OXLINT_RC=1 STUB_OXLINT_OUT="eslint(no-unused-vars): y is never used"
  lint frontend/src/shared/lib/format.ts
  assert_blocked "no-unused-vars"
}

test_lint_ts_skips_when_frontend_is_not_set_up() {
  make_repo
  write_file frontend/src/main.tsx 'export {}'
  lint frontend/src/main.tsx
  assert_quiet_success
  assert_eq "" "$(cat "$STUB_LOG")" "no tool ran"
}

test_lint_ts_skips_generated_client() {
  make_repo
  frontend_ready
  write_file frontend/src/shared/api/generated/client.ts 'export {}'
  lint frontend/src/shared/api/generated/client.ts
  assert_quiet_success
  assert_eq 0 "$(calls oxfmt)"
}

test_lint_toml_runs_taplo_on_the_file() {
  make_repo
  stub_bin taplo
  write_file deploy/local/settings.toml 'a = 1'
  lint deploy/local/settings.toml
  assert_quiet_success
  assert_contains "$(call_line taplo)" "fmt"
  assert_contains "$(call_line taplo)" "$R/deploy/local/settings.toml"
}

test_lint_toml_error_blocks() {
  make_repo
  stub_bin taplo
  write_file deploy/local/settings.toml 'a = '
  export STUB_TAPLO_RC=1 STUB_TAPLO_OUT="error: invalid TOML: expected value"
  lint deploy/local/settings.toml
  assert_blocked "invalid TOML"
}

test_lint_toml_without_taplo_skips() {
  make_repo
  write_file deploy/local/settings.toml 'a = 1'
  lint deploy/local/settings.toml
  assert_quiet_success
}

test_lint_other_files_run_nothing() {
  make_repo
  stub_bin rustfmt cargo taplo
  frontend_ready
  write_file docs/plan/notes.md '# notes'
  lint docs/plan/notes.md
  assert_quiet_success
  assert_eq "" "$(cat "$STUB_LOG")" "no tool ran"
}

test_lint_skips_inside_review_children() {
  make_repo
  stub_bin rustfmt cargo
  backend_package
  export CLAUDE_REVIEW_CHILD=1
  lint backend/crates/domain/src/lib.rs
  assert_quiet_success
  assert_eq 0 "$(calls rustfmt)"
}
