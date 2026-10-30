# shellcheck shell=bash
# shellcheck disable=SC2086 # $CORE and $UX are word lists on purpose
# stop-gate.sh (Stop): if the tree changed since the last green run, runs `just verify`,
# then the reviewer agents picked from the changed paths, in parallel, as headless
# `claude -p` children. Exit 2 with findings on stderr blocks; exit 0 lets Claude stop.

CORE="code-quality hygiene security separation testing"
UX="ux-cognitive ux-complexity ux-interaction ux-memory ux-perception"

stop() { # [fixture]
  hook_input "${1:-stop}" CWD="$R"
  run_hook stop-gate.sh
}

gate_repo() {
  make_repo
  stub_bin just claude
}

verdict() { # agent fixture-name
  cp "$FIXTURES/$2.json" "$STUB_CLAUDE_DIR/$1.json"
}

sorted() { # words... : sorted, space-separated
  printf '%s\n' "$@" | tr ' ' '\n' | sed '/^$/d' | sort | tr '\n' ' ' | sed 's/ $//'
}

assert_reviewers() { # expected-agent-names...
  assert_eq "$(sorted "$@")" "$(reviewers_called)" "reviewers selected"
}

# ---- guards and fingerprint -------------------------------------------------------------

test_stop_recursion_guard_exits_0_for_review_children() {
  gate_repo
  write_file backend/crates/domain/src/lib.rs
  export CLAUDE_REVIEW_CHILD=1
  stop
  assert_rc 0
  assert_eq 0 "$(calls just)" "children never run verify"
  assert_eq 0 "$(calls claude)" "children never start reviewers"
}

test_stop_without_changes_against_base_exits_0() {
  gate_repo
  stop
  assert_rc 0
  assert_eq 0 "$(calls just)"
}

test_stop_unchanged_fingerprint_exits_0_without_rerun() {
  gate_repo
  write_file backend/crates/domain/src/lib.rs
  stop
  assert_rc 0
  assert_file "$R/.claude/state/last-green"
  local reviewers
  reviewers=$(calls claude)
  stop
  assert_rc 0
  assert_eq 1 "$(calls just)" "verify ran once"
  assert_eq "$reviewers" "$(calls claude)" "reviewers ran once"
}

test_stop_reruns_after_the_tree_changes() {
  gate_repo
  write_file backend/crates/domain/src/lib.rs "pub fn one() {}"
  stop
  assert_rc 0
  write_file backend/crates/domain/src/lib.rs "pub fn two() {}"
  stop
  assert_rc 0
  assert_eq 2 "$(calls just)"
}

test_stop_counts_committed_work_on_the_branch() {
  gate_repo
  write_file backend/crates/domain/src/lib.rs
  commit_all "feat: domain"
  stop
  assert_rc 0
  assert_eq 1 "$(calls just)" "committed changes against origin/main are gated too"
  assert_reviewers $CORE
}

# ---- verify -----------------------------------------------------------------------------

test_stop_runs_just_verify_in_the_repo_root() {
  gate_repo
  write_file backend/crates/domain/src/lib.rs
  hook_input stop CWD="$R/backend"
  run_hook stop-gate.sh
  assert_rc 0
  assert_eq "verify" "$(call_line just)"
  assert_eq "$R" "$(call_cwd just)"
}

test_stop_verify_failure_exits_2_with_the_output() {
  gate_repo
  write_file backend/crates/domain/src/lib.rs
  export STUB_JUST_RC=1 STUB_JUST_OUT="error: Recipe \`backend-verify\` failed on line 4"
  stop
  assert_rc 2
  assert_contains "$HOOK_ERR" "just verify"
  assert_contains "$HOOK_ERR" "Recipe \`backend-verify\` failed"
  assert_eq 0 "$(calls claude)" "no reviewers when verify fails"
  assert_no_file "$R/.claude/state/last-green"
}

# ---- reviewer selection -----------------------------------------------------------------

test_stop_selects_core_and_ux_reviewers_for_frontend_source() {
  gate_repo
  write_file frontend/src/features/agents/views/AgentList.tsx
  stop
  assert_rc 0
  assert_reviewers $CORE $UX
}

test_stop_adds_performance_for_the_frontend_testing_feature() {
  gate_repo
  write_file frontend/src/features/testing/state/session.ts
  stop
  assert_rc 0
  assert_reviewers $CORE $UX performance
}

test_stop_selects_core_reviewers_for_backend_source() {
  gate_repo
  write_file backend/crates/domain/src/lib.rs
  stop
  assert_rc 0
  assert_reviewers $CORE
}

test_stop_adds_performance_for_the_voice_pipeline() {
  gate_repo
  write_file backend/crates/pipeline/src/turns.rs
  stop
  assert_rc 0
  assert_reviewers $CORE performance
}

test_stop_selects_ci_cd_for_github_paths() {
  gate_repo
  write_file .github/workflows/ci.yml "name: ci"
  stop
  assert_rc 0
  assert_reviewers ci-cd
}

test_stop_selects_docs_for_docs_paths() {
  gate_repo
  write_file docs/plan/notes.md "# notes"
  stop
  assert_rc 0
  assert_reviewers docs
}

test_stop_selects_dependencies_for_manifests() {
  gate_repo
  write_file backend/Cargo.toml "[workspace]"
  stop
  assert_rc 0
  assert_reviewers dependencies
}

test_stop_runs_each_selected_reviewer_once() {
  gate_repo
  export VAK_REVIEW_CONCURRENCY=2
  write_file frontend/src/features/agents/views/AgentList.tsx
  write_file backend/crates/domain/src/lib.rs
  write_file .github/workflows/ci.yml "name: ci"
  write_file docs/plan/notes.md "# notes"
  write_file backend/Cargo.toml "[workspace]"
  stop
  assert_rc 0
  assert_reviewers $CORE $UX ci-cd docs dependencies
  assert_eq 13 "$(calls claude)"
}

# ---- reviewer runs and verdicts -----------------------------------------------------------

test_stop_reviewers_are_capped_headless_children() {
  gate_repo
  export VAK_REVIEW_MAX_TURNS=12 VAK_REVIEW_MAX_BUDGET_USD=0.75
  write_file backend/crates/domain/src/lib.rs
  stop
  assert_rc 0
  local line
  line=$(call_line claude)
  assert_contains "$line" "child=1"
  assert_contains "$line" " -p"
  assert_contains "$line" "--agent "
  assert_contains "$line" "--output-format json"
  assert_contains "$line" "--json-schema "
  assert_contains "$line" '"verdict"'
  assert_contains "$line" "--max-turns 12"
  assert_contains "$line" "--max-budget-usd 0.75"
  assert_eq "$R" "$(call_cwd claude)" "reviewers run in the repo root"
  assert_contains "$(cat "$STUB_CLAUDE_DIR/prompt-security.txt")" "backend/crates/domain/src/lib.rs"
}

test_stop_green_run_reports_the_reviewers() {
  gate_repo
  write_file backend/crates/domain/src/lib.rs
  stop
  assert_rc 0
  assert_contains "$(out_json '.systemMessage')" "security"
}

test_stop_reviewer_fail_exits_2_with_findings() {
  gate_repo
  verdict security verdict-fail
  write_file backend/crates/domain/src/lib.rs
  stop
  assert_rc 2
  assert_contains "$HOOK_ERR" "security"
  assert_contains "$HOOK_ERR" "SQL built with format!()"
  assert_contains "$HOOK_ERR" "backend/crates/api/src/routes.rs:42"
  assert_no_file "$R/.claude/state/last-green"
}

test_stop_major_finding_fails_even_with_a_pass_verdict() {
  gate_repo
  verdict testing verdict-pass-major
  write_file backend/crates/domain/src/lib.rs
  stop
  assert_rc 2
  assert_contains "$HOOK_ERR" "testing"
  assert_contains "$HOOK_ERR" "no unit test"
}

test_stop_minor_findings_do_not_block() {
  gate_repo
  verdict hygiene verdict-pass-minor
  write_file backend/crates/domain/src/lib.rs
  stop
  assert_rc 0
}

test_stop_missing_verdict_fails() {
  gate_repo
  verdict code-quality verdict-no-output
  write_file backend/crates/domain/src/lib.rs
  stop
  assert_rc 2
  assert_contains "$HOOK_ERR" "code-quality"
}

test_stop_reviewer_crash_fails() {
  gate_repo
  echo 1 >"$STUB_CLAUDE_DIR/separation.rc"
  write_file backend/crates/domain/src/lib.rs
  stop
  assert_rc 2
  assert_contains "$HOOK_ERR" "separation"
}

test_stop_unparseable_verdict_fails() {
  gate_repo
  cp "$FIXTURES/claude-garbage.txt" "$STUB_CLAUDE_DIR/security.json"
  write_file backend/crates/domain/src/lib.rs
  stop
  assert_rc 2
  assert_contains "$HOOK_ERR" "security"
}

test_stop_python_fallback_aggregates_verdicts() {
  gate_repo
  export VAK_HOOK_JSON=python3
  verdict security verdict-fail
  write_file backend/crates/domain/src/lib.rs
  stop
  assert_rc 2
  assert_contains "$HOOK_ERR" "backend/crates/api/src/routes.rs:42"
}

# ---- human switch -------------------------------------------------------------------------

test_stop_reviewers_off_still_runs_verify() {
  gate_repo
  export VAK_REVIEWERS=off
  write_file backend/crates/domain/src/lib.rs
  stop
  assert_rc 0
  assert_eq 1 "$(calls just)"
  assert_eq 0 "$(calls claude)"
  assert_contains "$(out_json '.systemMessage')" "VAK_REVIEWERS=off"
}

test_stop_reviewers_off_does_not_hide_verify_failures() {
  gate_repo
  export VAK_REVIEWERS=off STUB_JUST_RC=1 STUB_JUST_OUT="error: tests failed"
  write_file backend/crates/domain/src/lib.rs
  stop
  assert_rc 2
}

test_stop_turning_reviewers_back_on_reruns_the_gate() {
  gate_repo
  write_file backend/crates/domain/src/lib.rs
  export VAK_REVIEWERS=off
  stop
  assert_rc 0
  unset VAK_REVIEWERS
  stop
  assert_rc 0
  assert_reviewers $CORE
}

# ---- loop breaker -------------------------------------------------------------------------

test_stop_loop_breaker_asks_a_human_when_nothing_changed() {
  gate_repo
  export STUB_JUST_RC=1 STUB_JUST_OUT="error: test domain::adds failed"
  write_file backend/crates/domain/src/lib.rs
  stop
  assert_rc 2
  stop stop-active
  assert_rc 0
  assert_eq 1 "$(calls just)" "no rerun on an unchanged tree"
  assert_contains "$(out_json '.systemMessage')" "human"
}

test_stop_replays_the_failure_on_a_new_turn_without_rerun() {
  gate_repo
  export STUB_JUST_RC=1 STUB_JUST_OUT="error: test domain::adds failed"
  write_file backend/crates/domain/src/lib.rs
  stop
  assert_rc 2
  stop
  assert_rc 2
  assert_contains "$HOOK_ERR" "domain::adds failed"
  assert_eq 1 "$(calls just)"
}

test_stop_loop_breaker_on_a_repeated_identical_failure() {
  gate_repo
  export VAK_STOP_MAX_REPEATS=2 STUB_JUST_RC=1 STUB_JUST_OUT="error: test domain::adds failed"
  write_file backend/crates/domain/src/lib.rs "pub fn one() {}"
  stop
  assert_rc 2
  write_file backend/crates/domain/src/lib.rs "pub fn two() {}"
  stop stop-active
  assert_rc 0
  assert_eq 2 "$(calls just)"
  assert_contains "$(out_json '.systemMessage')" "human"
}

test_stop_a_different_failure_keeps_blocking() {
  gate_repo
  export VAK_STOP_MAX_REPEATS=2 STUB_JUST_RC=1 STUB_JUST_OUT="error: test domain::adds failed"
  write_file backend/crates/domain/src/lib.rs "pub fn one() {}"
  stop
  assert_rc 2
  export STUB_JUST_OUT="error: clippy: unused import"
  write_file backend/crates/domain/src/lib.rs "pub fn two() {}"
  stop stop-active
  assert_rc 2
}

test_stop_goes_green_after_a_fix() {
  gate_repo
  export STUB_JUST_RC=1 STUB_JUST_OUT="error: test domain::adds failed"
  write_file backend/crates/domain/src/lib.rs "pub fn one() {}"
  stop
  assert_rc 2
  export STUB_JUST_RC=0 STUB_JUST_OUT=""
  write_file backend/crates/domain/src/lib.rs "pub fn two() {}"
  stop stop-active
  assert_rc 0
  assert_file "$R/.claude/state/last-green"
}
