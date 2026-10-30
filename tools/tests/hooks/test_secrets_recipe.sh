# shellcheck shell=bash
# `just repo-secrets` (tools/repo.just) scans tracked and new files only: gitignored build
# output must never cause a finding. Uses the real just and gitleaks; passes with a note
# when either is missing. The fake token is assembled at run time, so this file holds none.

secrets_just() { # recipe
  "$(find_on_orig_path just)" --justfile "$REPO_ROOT/tools/repo.just" --working-directory "$R" "$@"
}

secrets_ready() {
  local tool path
  for tool in just gitleaks tar; do
    path=$(find_on_orig_path "$tool")
    if [ -z "$path" ]; then
      echo "$tool is not installed; skipped"
      return 1
    fi
    [ -e "$T/bin/$tool" ] || ln -s "$path" "$T/bin/$tool"
  done
}

fake_token_file() { # repo-relative-path
  local body
  body=$(printf '%s' 'a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8')
  write_file "$1" "github_token = \"ghp_${body}\""
}

test_secrets_ignores_gitignored_build_output() {
  secrets_ready || return 0
  make_repo
  printf '%s\n' 'frontend/dist/' 'storybook-static/' >>"$R/.gitignore"
  commit_all "chore: ignore build output"
  fake_token_file frontend/dist/assets/index.js
  fake_token_file storybook-static/main.js
  fake_token_file backend/target/debug/build.log
  secrets_just repo-secrets >"$T/scan.out" 2>&1 || fail "ignored files were scanned: $(cat "$T/scan.out")"
}

test_secrets_flags_tracked_files() {
  secrets_ready || return 0
  make_repo
  fake_token_file deploy/local/settings.toml
  commit_all "chore: oops"
  if secrets_just repo-secrets >"$T/scan.out" 2>&1; then
    fail "a committed token was not found"
  fi
}

test_secrets_flags_new_untracked_files() {
  secrets_ready || return 0
  make_repo
  fake_token_file backend/crates/config/src/defaults.rs
  if secrets_just repo-secrets >"$T/scan.out" 2>&1; then
    fail "a token in a new, untracked file was not found"
  fi
}
