# shellcheck shell=bash
# stop-gate.sh reads each reviewer's verdict from `.structured_output` when it is an object,
# else from the reviewer's text in `.result`: the whole text as JSON, else the last ```json
# fence, else the last top-level {...} with a "verdict" key. Validation fails closed.
#
# `claude -p --agent <name> --json-schema` leaves `structured_output` out when the agent's
# `tools:` allowlist doesn't name StructuredOutput (Claude Code 2.1.314), so real reviewers
# answer in `.result`. The verdict-result-{json,prose,fenced} fixtures are trimmed from
# real reviewer runs.
#
# Each case runs twice: on the jq path and on the python3 fallback (VAK_HOOK_JSON).

verdict_case() { # json-tool fixture : one gate run where the docs reviewer returns fixture
  make_repo
  stub_bin just claude
  export VAK_HOOK_JSON=$1
  cp "$FIXTURES/$2.json" "$STUB_CLAUDE_DIR/docs.json"
  write_file docs/plan/notes.md "# notes"
  hook_input stop CWD="$R"
  run_hook stop-gate.sh
  assert_eq docs "$(reviewers_called)" "only the docs reviewer runs"
}

assert_gate_passes() {
  assert_rc 0
  assert_contains "$(out_json '.systemMessage')" "reviewers docs"
  assert_file "$R/.claude/state/last-green"
}

assert_gate_fails() { # text-on-stderr...
  local needle
  assert_rc 2
  for needle in "$@"; do assert_contains "$HOOK_ERR" "$needle"; done
  assert_no_file "$R/.claude/state/last-green"
}

# ---- structured_output ----------------------------------------------------------------------

case_structured_output() {
  verdict_case "$1" verdict-pass
  assert_gate_passes
}
test_stop_verdict_structured_output_jq() { case_structured_output jq; }
test_stop_verdict_structured_output_python() { case_structured_output python3; }

case_structured_output_wins_over_result() {
  verdict_case "$1" verdict-structured-over-result
  assert_gate_fails "[docs] FAIL" "docs/plan/notes.md:3" "names a model vendor"
}
test_stop_verdict_structured_output_wins_over_result_jq() { case_structured_output_wins_over_result jq; }
test_stop_verdict_structured_output_wins_over_result_python() { case_structured_output_wins_over_result python3; }

# ---- verdict in .result -----------------------------------------------------------------------

case_result_exact_json() {
  verdict_case "$1" verdict-result-json
  assert_gate_passes
}
test_stop_verdict_result_exact_json_jq() { case_result_exact_json jq; }
test_stop_verdict_result_exact_json_python() { case_result_exact_json python3; }

case_result_prose_then_json() {
  verdict_case "$1" verdict-result-prose
  assert_gate_passes
}
test_stop_verdict_result_prose_then_json_jq() { case_result_prose_then_json jq; }
test_stop_verdict_result_prose_then_json_python() { case_result_prose_then_json python3; }

case_result_fenced() {
  verdict_case "$1" verdict-result-fenced
  assert_gate_passes
}
test_stop_verdict_result_fenced_jq() { case_result_fenced jq; }
test_stop_verdict_result_fenced_python() { case_result_fenced python3; }

case_result_last_verdict_wins() {
  verdict_case "$1" verdict-result-last-wins
  assert_gate_fails "[docs] FAIL" "backend/crates/api/src/routes.rs:42" 'format!("{}", id)'
}
test_stop_verdict_result_last_verdict_wins_jq() { case_result_last_verdict_wins jq; }
test_stop_verdict_result_last_verdict_wins_python() { case_result_last_verdict_wins python3; }

case_result_major_finding() {
  verdict_case "$1" verdict-result-major
  assert_gate_fails "[docs] FAIL" "major backend/crates/domain/src/rules.rs:12" "no unit test"
}
test_stop_verdict_result_major_finding_fails_jq() { case_result_major_finding jq; }
test_stop_verdict_result_major_finding_fails_python() { case_result_major_finding python3; }

# ---- fail closed ------------------------------------------------------------------------------

case_result_without_json() {
  verdict_case "$1" verdict-result-none
  assert_gate_fails "[docs] NO VERDICT" "no verdict"
}
test_stop_verdict_result_without_json_fails_jq() { case_result_without_json jq; }
test_stop_verdict_result_without_json_fails_python() { case_result_without_json python3; }

case_result_invalid_verdict() {
  verdict_case "$1" verdict-result-invalid
  assert_gate_fails "[docs] NO VERDICT" "verdict is neither pass nor fail"
}
test_stop_verdict_result_invalid_verdict_fails_jq() { case_result_invalid_verdict jq; }
test_stop_verdict_result_invalid_verdict_fails_python() { case_result_invalid_verdict python3; }

case_result_findings_not_a_list() {
  verdict_case "$1" verdict-result-findings-not-list
  assert_gate_fails "[docs] NO VERDICT" "findings is not a list"
}
test_stop_verdict_result_findings_not_a_list_fails_jq() { case_result_findings_not_a_list jq; }
test_stop_verdict_result_findings_not_a_list_fails_python() { case_result_findings_not_a_list python3; }

case_error_run_with_a_pass_result() {
  verdict_case "$1" verdict-result-error
  assert_gate_fails "[docs] NO VERDICT" "the run ended with an error (error_during_execution)"
}
test_stop_verdict_error_run_fails_despite_a_pass_result_jq() { case_error_run_with_a_pass_result jq; }
test_stop_verdict_error_run_fails_despite_a_pass_result_python() { case_error_run_with_a_pass_result python3; }
