# shellcheck shell=bash
# stop-gate.sh keeps its own deadline below the hook timeout. A killed hook would let
# Claude stop, so reviewers that can't run in time count as failures (fail closed).

test_stop_reviewers_without_time_left_fail_closed() {
  make_repo
  stub_bin just claude
  write_file backend/crates/domain/src/lib.rs
  export VAK_STOP_DEADLINE_S=80
  hook_input stop CWD="$R"
  run_hook stop-gate.sh
  assert_rc 2
  assert_contains "$HOOK_ERR" "ran out of time"
  assert_eq 0 "$(calls claude)" "no reviewer starts without time left"
}

test_stop_reviewer_timeout_is_capped_by_the_deadline() {
  make_repo
  stub_bin just claude
  write_file backend/crates/domain/src/lib.rs
  export VAK_STOP_DEADLINE_S=1000 VAK_REVIEW_TIMEOUT_S=5000
  hook_input stop CWD="$R"
  run_hook stop-gate.sh
  assert_rc 0
}
