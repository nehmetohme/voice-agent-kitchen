# shellcheck shell=bash
# `just contract-pin` and `just contract-push` (contracts/contracts.just), run against a
# fixture repo with a local bare "origin". Every tag they make lives in the sandbox only.
# Uses the real just and taplo; prints a note and passes without them.

have_tools() {
  local tool
  for tool in just taplo; do
    [ -n "$(find_on_orig_path "$tool")" ] || {
      echo "$tool is not installed; skipped"
      return 1
    }
  done
  ln -sf "$(find_on_orig_path taplo)" "$T/bin/taplo"
}

contract_just() { # args... : the real root justfile, run in the fixture repo
  local just
  just=$(find_on_orig_path just)
  "$just" --justfile "$REPO_ROOT/justfile" --working-directory "$R" "$@"
}

# shellcheck disable=SC2034 # HOOK_RC and HOOK_ERR are read by assert_rc in lib.sh
recipe() { # recipe args... : sets HOOK_RC and HOOK_OUT (stdout and stderr)
  set +e
  HOOK_OUT=$(contract_just "$@" 2>&1)
  HOOK_RC=$?
  set -e
  HOOK_ERR=""
}

contract_repo() { # $R with HEAD registering the health contract at version 1
  make_repo
  write_file contracts/contracts.toml "$(printf '%s\n' '[features.health]' 'version = 1' \
    'paths = ["tests/contract_health.rs"]')"
  write_file tests/contract_health.rs '#[test] fn health() {}'
  commit_all "test(contract): health v1"
}

local_tag() { git -C "$R" rev-parse --quiet --verify "refs/tags/$1" || true; }
origin_tag() { git -C "$T/origin.git" rev-parse --quiet --verify "refs/tags/$1" || true; }
origin_refs() { git -C "$T/origin.git" for-each-ref --format='%(refname)' | tr '\n' ' '; }
local_tags() { git -C "$R" for-each-ref --format='%(refname)' refs/tags | tr '\n' ' '; }

origin_tags_elsewhere() { # tag : origin gets its own tag of that name, on another commit
  git -C "$R" push -q origin task/test
  git -C "$T/origin.git" tag --annotate --message "made elsewhere" "$1" main
}

pre_push_hook() { # exit-code : a pre-push hook that logs its refs, then exits with the code
  # Inside a git hook (lefthook's pre-push runs these tests), git's exec-path leads PATH, so
  # the sandboxed git finds no templates and `git init` makes no hooks folder.
  mkdir -p "$R/.git/hooks"
  printf '%s\n' '#!/bin/sh' "cat >>\"$T/pre-push.log\"" "exit $1" >"$R/.git/hooks/pre-push"
  chmod +x "$R/.git/hooks/pre-push"
}

test_contract_pin_creates_an_unsigned_annotated_tag_at_head_without_pushing() {
  have_tools || return 0
  contract_repo
  # Signing would fail here (no key), so a pass proves the recipe never signs.
  git -C "$R" config tag.gpgSign true
  git -C "$R" config tag.forceSignAnnotated true
  recipe contract-pin health 1
  assert_rc 0
  assert_eq "tag" "$(git -C "$R" cat-file -t refs/tags/contract/health/v1)" "an annotated tag"
  assert_eq "$(git -C "$R" rev-parse HEAD)" "$(git -C "$R" rev-parse 'contract/health/v1^{commit}')"
  assert_eq "health v1" \
    "$(git -C "$R" for-each-ref --format='%(contents:subject)' refs/tags/contract/health/v1)"
  assert_contains "$(git -C "$R" cat-file tag refs/tags/contract/health/v1)" "tag contract/health/v1"
  assert_not_contains "$(git -C "$R" cat-file tag refs/tags/contract/health/v1)" "SIGNATURE" \
    "the tag is unsigned"
  assert_eq "" "$(origin_tag contract/health/v1)" "contract-pin never pushes"
  assert_contains "$HOOK_OUT" "just contract-push health 1"
}

test_contract_pin_rejects_invalid_features_and_versions() {
  have_tools || return 0
  contract_repo
  local args
  for args in "Health 1" "he_alth 1" "../health 1" "health 0" "health 01" "health v1" "health 1.0"; do
    # shellcheck disable=SC2086 # two words on purpose
    recipe contract-pin $args
    assert_rc 1
    assert_contains "$HOOK_OUT" "must be"
  done
  assert_eq "" "$(local_tags)" "no tag was created"
}

test_contract_pin_refuses_a_dirty_tree() {
  have_tools || return 0
  contract_repo
  write_file notes.txt "untracked"
  recipe contract-pin health 1
  assert_rc 1
  assert_contains "$HOOK_OUT" "not clean"
  rm "$R/notes.txt"
  printf '%s\n' '#[test] fn changed() {}' >"$R/tests/contract_health.rs"
  recipe contract-pin health 1
  assert_rc 1
  assert_contains "$HOOK_OUT" "not clean"
  assert_eq "" "$(local_tags)" "no tag was created"
}

test_contract_pin_refuses_a_version_heads_manifest_does_not_pin() {
  have_tools || return 0
  contract_repo
  recipe contract-pin health 2
  assert_rc 1
  assert_contains "$HOOK_OUT" "has health at version '1', not 2"
  recipe contract-pin billing 1
  assert_rc 1
  assert_contains "$HOOK_OUT" "has billing at version 'none', not 1"
  assert_eq "" "$(local_tags)" "no tag was created"
}

test_contract_pin_refuses_a_head_without_a_manifest() {
  have_tools || return 0
  make_repo
  recipe contract-pin health 1
  assert_rc 1
  assert_contains "$HOOK_OUT" "HEAD has no contracts/contracts.toml"
}

test_contract_pin_never_moves_an_existing_local_tag() {
  have_tools || return 0
  contract_repo
  recipe contract-pin health 1
  assert_rc 0
  local before
  before=$(local_tag contract/health/v1)
  write_file README.md "# moved on"
  commit_all "docs: later work"
  recipe contract-pin health 1
  assert_rc 1
  assert_contains "$HOOK_OUT" "already exists locally"
  assert_eq "$before" "$(local_tag contract/health/v1)" "the tag did not move"
}

test_contract_pin_refuses_a_tag_that_already_exists_on_origin() {
  have_tools || return 0
  contract_repo
  origin_tags_elsewhere contract/health/v1
  recipe contract-pin health 1
  assert_rc 1
  assert_contains "$HOOK_OUT" "already exists on origin"
  assert_eq "" "$(local_tags)" "no tag was created"
}

test_contract_pin_refuses_when_origin_cannot_be_listed() {
  have_tools || return 0
  contract_repo
  git -C "$R" remote set-url origin "$T/no-such-origin.git"
  recipe contract-pin health 1
  assert_rc 1
  assert_contains "$HOOK_OUT" "can't list origin's tags"
  assert_eq "" "$(local_tags)" "no tag was created"
}

test_contract_push_pushes_exactly_the_tag_through_the_pre_push_hook() {
  have_tools || return 0
  contract_repo
  recipe contract-pin health 1
  assert_rc 0
  # Neither another annotated tag on the same commit nor the branch may leave with it.
  git -C "$R" tag --annotate --message "unrelated" unrelated HEAD
  git -C "$R" config push.followTags true
  pre_push_hook 0
  recipe contract-push health 1
  assert_rc 0
  assert_eq "$(local_tag contract/health/v1)" "$(origin_tag contract/health/v1)" "origin has the same tag object"
  assert_eq "refs/heads/main refs/tags/contract/health/v1 " "$(origin_refs)" "only the tag was pushed"
  assert_contains "$(cat "$T/pre-push.log")" "refs/tags/contract/health/v1"
  assert_contains "$HOOK_OUT" "pushed contract/health/v1"
}

test_contract_push_stops_when_the_pre_push_hook_fails() {
  have_tools || return 0
  contract_repo
  recipe contract-pin health 1
  pre_push_hook 1
  recipe contract-push health 1
  assert_rc 1
  assert_file "$T/pre-push.log"
  assert_eq "" "$(origin_tag contract/health/v1)" "nothing was pushed"
}

test_contract_push_refuses_a_tag_that_does_not_exist_locally() {
  have_tools || return 0
  contract_repo
  recipe contract-push health 1
  assert_rc 1
  assert_contains "$HOOK_OUT" "doesn't exist locally"
  assert_contains "$HOOK_OUT" "just contract-pin health 1"
  recipe contract-push Health 1
  assert_rc 1
  assert_contains "$HOOK_OUT" "must be"
  assert_eq "refs/heads/main " "$(origin_refs)" "nothing was pushed"
}

test_contract_push_succeeds_without_pushing_when_origin_has_the_same_tag() {
  have_tools || return 0
  contract_repo
  recipe contract-pin health 1
  recipe contract-push health 1
  assert_rc 0
  # A second push would fail in this hook, so a pass proves nothing was pushed.
  pre_push_hook 1
  recipe contract-push health 1
  assert_rc 0
  assert_contains "$HOOK_OUT" "origin already has contract/health/v1"
  assert_no_file "$T/pre-push.log"
}

test_contract_push_never_moves_a_tag_origin_has_elsewhere() {
  have_tools || return 0
  contract_repo
  recipe contract-pin health 1
  origin_tags_elsewhere contract/health/v1
  local theirs
  theirs=$(origin_tag contract/health/v1)
  recipe contract-push health 1
  assert_rc 1
  assert_contains "$HOOK_OUT" "a contract tag never moves"
  assert_eq "$theirs" "$(origin_tag contract/health/v1)" "origin's tag did not move"
}

test_contract_recipes_are_shellcheck_clean_and_never_force() {
  have_tools || return 0
  make_repo
  local shellcheck name body
  shellcheck=$(find_on_orig_path shellcheck)
  for name in contract-pin contract-push; do
    body=$(contract_just --show "$name" | sed -n '/^    #!/,$p' | sed 's/^    //')
    assert_contains "$body" "#!/usr/bin/env bash" "$name is a bash script recipe"
    assert_not_contains "$body" "--force" "$name"
    assert_not_contains "$body" "--no-verify" "$name"
    assert_not_contains "$body" "LEFTHOOK" "$name"
    assert_not_contains "$body" " -d " "$name"
    assert_not_contains "$body" "--delete" "$name"
    if [ -n "$shellcheck" ]; then
      printf '%s\n' "$body" | "$shellcheck" - || fail "$name: shellcheck found issues"
    fi
  done
  if contract_just --show contract-approve >/dev/null 2>&1; then
    fail "contract-approve still exists; contract-pin and contract-push replace it"
  fi
}
