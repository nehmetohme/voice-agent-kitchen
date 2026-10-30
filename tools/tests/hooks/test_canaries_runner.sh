# shellcheck shell=bash
# tools/canaries/run.sh, the gate canary runner (docs/ci.md, "Gate self-tests"): how it
# classifies each canary, that only the command's own output counts, and that it always
# removes its worktrees. Dummy canaries in the fixture repo; no real gate runs.

# A dummy canary in $R whose apply.sh plants bad.txt.
canary() { # name command expect
  local dir="$R/tools/canaries/$1"
  mkdir -p "$dir"
  printf 'gate=dummy gate\nviolation=plants bad.txt\ncommand=%s\nexpect=%s\n' "$2" "$3" >"$dir/canary.conf"
  printf '%s\n' 'echo "violation: planted" >bad.txt' >"$dir/apply.sh"
}

# A dummy canary whose violation is a patch to the fixture README.
patch_canary() { # name from-line to-line command expect
  canary "$1" "$4" "$5"
  rm "$R/tools/canaries/$1/apply.sh"
  printf '%s\n' "diff --git a/README.md b/README.md" "--- a/README.md" "+++ b/README.md" \
    "@@ -1 +1 @@" "-$2" "+$3" >"$R/tools/canaries/$1/patch.diff"
}

run_canaries() { # name... : runs the current runner from $R; sets RUN_RC and RUN_OUT
  cp "$REPO_ROOT/tools/canaries/run.sh" "$R/tools/canaries/run.sh"
  set +e
  RUN_OUT=$(cd "$R" && tools/canaries/run.sh "$@" 2>&1)
  RUN_RC=$?
  set -e
}

assert_no_canary_worktrees() {
  assert_eq "1" "$(git -C "$R" worktree list | wc -l | tr -d ' ')" "worktrees left behind"
  assert_eq "" "$(find "$R/.claude/worktrees" -mindepth 1 -maxdepth 1 2>/dev/null)" "worktree folders left behind"
}

test_canaries_caught_needs_a_failure_and_the_expected_message() {
  make_repo
  canary planted 'cat bad.txt; exit 3' 'violation: planted'
  patch_canary patched '# fixture repo' '# patched heading' 'cat README.md; exit 1' 'patched heading'
  run_canaries planted patched
  assert_eq 0 "$RUN_RC" "exit code: $RUN_OUT"
  assert_contains "$RUN_OUT" "caught in"
  assert_contains "$RUN_OUT" "exit 3"
  assert_contains "$RUN_OUT" "all 2 caught"
  assert_no_canary_worktrees
}

test_canaries_a_passing_command_is_missed_and_named() {
  make_repo
  canary planted 'cat bad.txt; exit 1' 'violation: planted'
  canary lenient 'true' 'violation: planted'
  run_canaries planted lenient
  assert_eq 1 "$RUN_RC" "exit code: $RUN_OUT"
  assert_contains "$RUN_OUT" "MISSED"
  assert_contains "$RUN_OUT" "1 of 2 caught"
  assert_contains "$RUN_OUT" "missed   lenient"
  assert_not_contains "$RUN_OUT" "missed   planted"
  assert_no_canary_worktrees
}

test_canaries_a_failure_without_the_message_is_wrong() {
  make_repo
  canary other 'echo something else broke; exit 1' 'violation: planted'
  run_canaries other
  assert_eq 1 "$RUN_RC" "exit code: $RUN_OUT"
  assert_contains "$RUN_OUT" "WRONG FAILURE"
  assert_contains "$RUN_OUT" "wrong    other"
}

test_canaries_setup_output_never_counts_as_the_gate_failing() {
  make_repo
  canary chatty 'exit 1' 'violation: planted'
  printf '%s\n' 'echo "violation: planted"; touch bad.txt' >"$R/tools/canaries/chatty/apply.sh"
  run_canaries chatty
  assert_eq 1 "$RUN_RC" "exit code: $RUN_OUT"
  assert_contains "$RUN_OUT" "wrong    chatty"
}

test_canaries_a_patch_that_no_longer_applies_is_stale() {
  make_repo
  patch_canary drifted '# a heading that is gone' '# patched heading' 'exit 1' 'anything'
  run_canaries drifted
  assert_eq 1 "$RUN_RC" "exit code: $RUN_OUT"
  assert_contains "$RUN_OUT" "STALE"
  assert_contains "$RUN_OUT" "stale    drifted"
  assert_no_canary_worktrees
}

test_canaries_a_violation_that_changes_nothing_is_stale() {
  make_repo
  canary noop 'exit 1' 'anything'
  printf '%s\n' 'true' >"$R/tools/canaries/noop/apply.sh"
  run_canaries noop
  assert_eq 1 "$RUN_RC" "exit code: $RUN_OUT"
  assert_contains "$RUN_OUT" "changes nothing"
  assert_no_canary_worktrees
}

test_canaries_reject_an_unknown_name() {
  make_repo
  canary planted 'exit 1' 'anything'
  run_canaries nope
  assert_eq 2 "$RUN_RC" "exit code: $RUN_OUT"
  assert_contains "$RUN_OUT" "unknown canary 'nope'; known: planted"
}
