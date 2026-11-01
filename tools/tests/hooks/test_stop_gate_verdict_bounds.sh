# shellcheck shell=bash
# stop-gate.sh caps the time it spends reading one verdict (VAK_REVIEW_PARSE_TIMEOUT_S).
# Scanning a reviewer's text for the last JSON object is slow on crafted input: 50000
# unclosed levels take python3 over a minute and jq far longer. A gate killed by the hook
# timeout would let Claude stop, so running over the cap counts as no verdict (fail closed).
# Runs on the jq path and on the python3 fallback.

case_crafted_result_fails_closed_in_bounded_time() { # json-tool
  local started
  make_repo
  stub_bin just claude
  export VAK_HOOK_JSON=$1 VAK_REVIEW_PARSE_TIMEOUT_S=1
  jq -n '{type: "result", subtype: "success", is_error: false, total_cost_usd: 0.1,
    result: ("{\"a\":" * 50000)}' >"$STUB_CLAUDE_DIR/docs.json"
  write_file docs/plan/notes.md "# notes"
  hook_input stop CWD="$R"
  started=$SECONDS
  run_hook stop-gate.sh
  [ $((SECONDS - started)) -lt 15 ] || fail "the gate took $((SECONDS - started))s to read one verdict"
  assert_eq docs "$(reviewers_called)" "only the docs reviewer runs"
  assert_rc 2
  assert_contains "$HOOK_ERR" "[docs] NO VERDICT"
}
test_stop_verdict_crafted_result_fails_closed_in_bounded_time_jq() { case_crafted_result_fails_closed_in_bounded_time jq; }
test_stop_verdict_crafted_result_fails_closed_in_bounded_time_python() { case_crafted_result_fails_closed_in_bounded_time python3; }
