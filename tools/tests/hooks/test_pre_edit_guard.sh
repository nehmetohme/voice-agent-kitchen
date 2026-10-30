# shellcheck shell=bash
# pre-edit-guard.sh (PreToolUse on Edit|Write): denies edits to generated files,
# owner-only files and files pinned by a signed contract tag. Allows everything else.

guard() { # repo-relative-path [fixture] [content]
  hook_input "${2:-pre-edit-write}" CWD="$R" FILE="$R/$1" CONTENT="${3:-x}"
  run_hook pre-edit-guard.sh
}

assert_denied() { # reason-substring
  assert_rc 0
  assert_json '.hookSpecificOutput.hookEventName' PreToolUse
  assert_json '.hookSpecificOutput.permissionDecision' deny
  assert_contains "$(out_json '.hookSpecificOutput.permissionDecisionReason')" "$1"
}

assert_allowed() {
  assert_rc 0
  assert_eq "" "$HOOK_OUT" "an allowed edit prints nothing"
}

write_contract_manifest() { # version
  mkdir -p "$R/contracts"
  cat >"$R/contracts/contracts.toml" <<EOF
contract_globs = ["**/tests/contract_*.rs", "**/*.contract.test.ts", "e2e/contract/**"]

[features.health]
version = $1
paths = [
  "backend/crates/api/tests/contract_health.rs",
  "frontend/src/features/agents/**/*.contract.test.ts",
]
EOF
}

tag_health_contract() { # version : commit the contract files and tag them
  write_contract_manifest "$1"
  write_file backend/crates/api/tests/contract_health.rs '#[test] fn health_returns_200() {}'
  write_file frontend/src/features/agents/__contract__/create.contract.test.ts "test('creates', () => {})"
  commit_all "test(contract): health v$1"
  git -C "$R" tag "contract/health/v$1"
}

test_guard_denies_openapi_contract() {
  make_repo
  guard contracts/openapi.json
  assert_denied "just openapi"
}

test_guard_denies_generated_frontend_client() {
  make_repo
  guard frontend/src/shared/api/generated/client.ts pre-edit-edit
  assert_denied "generated"
}

test_guard_denies_generated_route_tree() {
  make_repo
  guard frontend/src/app/routeTree.gen.ts pre-edit-edit
  assert_denied "generated"
}

test_guard_denies_generated_tokens_css() {
  make_repo
  guard frontend/src/shared/ui/tokens.css
  assert_denied "frontend/tokens/"
}

test_guard_denies_sqlx_offline_cache() {
  make_repo
  guard backend/crates/repositories-pg/.sqlx/query-1a2b3c.json
  assert_denied ".sqlx"
}

test_guard_denies_layers_toml() {
  make_repo
  guard backend/layers.toml pre-edit-edit
  assert_denied "layers.toml"
}

test_guard_allows_normal_files() {
  make_repo
  guard backend/crates/domain/src/lib.rs
  assert_allowed
  guard frontend/src/features/agents/views/AgentList.tsx pre-edit-edit
  assert_allowed
  guard contracts/contracts.toml
  assert_allowed
  guard docs/plan/notes.md
  assert_allowed
}

test_guard_allows_files_outside_the_repo() {
  make_repo
  hook_input pre-edit-write CWD="$R" FILE="$T/elsewhere/contracts/openapi.json" CONTENT=x
  run_hook pre-edit-guard.sh
  assert_allowed
}

test_guard_normalizes_dot_dot_paths() {
  make_repo
  mkdir -p "$R/frontend"
  hook_input pre-edit-write CWD="$R" FILE="$R/frontend/../contracts/openapi.json" CONTENT=x
  run_hook pre-edit-guard.sh
  assert_denied "just openapi"
}

test_guard_finds_repo_root_when_cwd_is_a_subfolder() {
  make_repo
  mkdir -p "$R/backend/crates"
  hook_input pre-edit-write CWD="$R/backend/crates" FILE="$R/backend/layers.toml" CONTENT=x
  run_hook pre-edit-guard.sh
  assert_denied "layers.toml"
}

test_guard_python_json_fallback() {
  make_repo
  export VAK_HOOK_JSON=python3
  guard contracts/openapi.json
  assert_denied "just openapi"
  guard README.md
  assert_allowed
}

test_guard_skips_contract_check_without_manifest_or_tags() {
  make_repo
  write_file backend/crates/api/tests/contract_health.rs
  guard backend/crates/api/tests/contract_health.rs
  assert_allowed
}

test_guard_allows_contract_files_before_they_are_signed() {
  make_repo
  write_contract_manifest 1
  write_file backend/crates/api/tests/contract_health.rs
  commit_all "test(contract): health v1"
  guard backend/crates/api/tests/contract_health.rs
  assert_allowed
}

test_guard_denies_files_covered_by_a_contract_tag() {
  make_repo
  tag_health_contract 1
  guard backend/crates/api/tests/contract_health.rs pre-edit-edit
  assert_denied "contract/health/v1"
  guard frontend/src/features/agents/__contract__/create.contract.test.ts
  assert_denied "contract/health/v1"
  guard backend/crates/api/src/lib.rs
  assert_allowed
}

test_guard_contract_check_without_tomllib() {
  make_repo
  tag_health_contract 1
  export VAK_HOOK_TOML=mini
  guard frontend/src/features/agents/__contract__/create.contract.test.ts
  assert_denied "contract/health/v1"
  guard backend/crates/api/src/lib.rs
  assert_allowed
}

test_guard_allows_contract_files_after_a_version_bump() {
  make_repo
  tag_health_contract 1
  write_contract_manifest 2
  guard backend/crates/api/tests/contract_health.rs
  assert_allowed
}

test_guard_denies_again_once_the_new_version_is_signed() {
  make_repo
  tag_health_contract 1
  tag_health_contract 2
  guard backend/crates/api/tests/contract_health.rs
  assert_denied "contract/health/v2"
}

tag_ui_contract() { # paths-line : a "ui" feature whose contract tests are .tsx and .spec.ts
  mkdir -p "$R/contracts"
  printf '%s\n' '[features.ui]' 'version = 1' "$1" >"$R/contracts/contracts.toml"
  write_file frontend/src/shared/ui/button.contract.test.tsx "test('renders', () => {})"
  write_file frontend/e2e/contract/agents/create.contract.spec.ts "test('creates', () => {})"
  write_file backend/crates/api/tests/contract_health.rs '#[test] fn health() {}'
  write_file frontend/src/shared/ui/button.tsx 'export const Button = () => null'
  commit_all "test(contract): ui v1"
  git -C "$R" tag contract/ui/v1
}

test_guard_denies_tsx_and_spec_contract_files_listed_in_the_manifest() {
  make_repo
  tag_ui_contract 'paths = ["frontend/src/shared/ui/*.contract.test.tsx", "frontend/e2e/contract/**"]'
  guard frontend/src/shared/ui/button.contract.test.tsx
  assert_denied "contract/ui/v1"
  guard frontend/e2e/contract/agents/create.contract.spec.ts pre-edit-edit
  assert_denied "contract/ui/v1"
  guard backend/crates/api/tests/contract_health.rs
  assert_allowed
  guard frontend/src/shared/ui/button.tsx
  assert_allowed
}

test_guard_uses_the_default_contract_globs_when_a_feature_lists_no_paths() {
  make_repo
  tag_ui_contract '# no paths: the default contract globs apply'
  guard frontend/src/shared/ui/button.contract.test.tsx
  assert_denied "contract/ui/v1"
  guard frontend/e2e/contract/agents/create.contract.spec.ts
  assert_denied "contract/ui/v1"
  guard backend/crates/api/tests/contract_health.rs
  assert_denied "contract/ui/v1"
  guard frontend/src/shared/ui/button.tsx
  assert_allowed
}

test_guard_denies_turning_off_reviewers_in_settings() {
  make_repo
  guard .claude/settings.local.json pre-edit-write "env VAK_REVIEWERS off"
  assert_denied "VAK_REVIEWERS"
  guard .claude/settings.json pre-edit-edit "VAK_REVIEWERS"
  assert_denied "human"
  guard .claude/settings.json pre-edit-edit "permissions"
  assert_allowed
}
