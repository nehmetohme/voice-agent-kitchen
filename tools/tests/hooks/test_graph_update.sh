# shellcheck shell=bash
# graph-update.sh (async PostToolUse on Edit|Write): runs `graphify update .` in the repo
# root, one run at a time, with one more run if edits arrived during a run.

graph_input() {
  hook_input post-edit-write CWD="$R" FILE="$R/backend/x.rs"
}

update_calls() {
  awk -F'\t' '$1 == "graphify" && $3 == "update ."' "$STUB_LOG" | wc -l | tr -d ' '
}

wait_for_lock() {
  local i=0
  while [ ! -d "$R/.claude/state/graph-update.lock" ]; do
    i=$((i + 1))
    [ "$i" -lt 100 ] || fail "graph-update never took the lock"
    sleep 0.05
  done
}

test_graph_runs_graphify_update_in_the_repo_root() {
  make_repo
  stub_bin graphify
  mkdir -p "$R/backend"
  hook_input post-edit-write CWD="$R/backend" FILE="$R/backend/x.rs"
  run_hook graph-update.sh
  assert_rc 0
  assert_eq 1 "$(update_calls)"
  assert_eq "$R" "$(call_cwd graphify)" "graphify runs in the repo root"
  assert_no_file "$R/.claude/state/graph-update.lock"
}

test_graph_debounces_edits_that_arrive_during_a_run() {
  make_repo
  stub_bin graphify
  graph_input
  export STUB_GRAPHIFY_SLEEP=1
  "$HOOKS_DIR/graph-update.sh" <"$T/input.json" >"$T/first.out" 2>&1 &
  local first=$!
  wait_for_lock
  run_hook graph-update.sh
  assert_rc 0
  run_hook graph-update.sh
  assert_rc 0
  wait "$first"
  assert_eq 2 "$(update_calls)" "three edits during one run give exactly one more run"
  assert_no_file "$R/.claude/state/graph-update.lock"
  assert_no_file "$R/.claude/state/graph-update.pending"
}

test_graph_recovers_a_stale_lock() {
  make_repo
  stub_bin graphify
  graph_input
  mkdir -p "$R/.claude/state/graph-update.lock"
  # A pid that is not running: start a short process and wait for it to exit.
  sleep 0 &
  local dead=$!
  wait "$dead"
  printf '%s\n' "$dead" >"$R/.claude/state/graph-update.lock/pid"
  run_hook graph-update.sh
  assert_rc 0
  assert_eq 1 "$(update_calls)"
}

test_graph_ignores_an_unrelated_graphify_on_path() {
  make_repo
  stub_bin graphify
  graph_input
  export STUB_GRAPHIFY_FLAVOR=npm
  run_hook graph-update.sh
  assert_rc 0
  assert_eq 0 "$(update_calls)"
}

test_graph_prefers_vak_graphify_override() {
  make_repo
  mkdir -p "$T/tools"
  ln -s "$STUBS/graphify" "$T/tools/graphify"
  graph_input
  export VAK_GRAPHIFY="$T/tools/graphify"
  run_hook graph-update.sh
  assert_rc 0
  assert_eq 1 "$(update_calls)"
}

test_graph_without_graphify_is_a_no_op() {
  make_repo
  graph_input
  run_hook graph-update.sh
  assert_rc 0
  assert_eq "" "$(cat "$STUB_LOG")"
}

test_graph_skips_inside_review_children() {
  make_repo
  stub_bin graphify
  graph_input
  export CLAUDE_REVIEW_CHILD=1
  run_hook graph-update.sh
  assert_rc 0
  assert_eq 0 "$(update_calls)"
}
