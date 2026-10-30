# shellcheck shell=bash
# shellcheck disable=SC2086 # $CORE is a word list on purpose
# stop-gate.sh on SubagentStop. Task agents run as subagents in their own worktrees
# (Agent tool, isolation: worktree), and Claude Code fires SubagentStop, not Stop, when one
# finishes. The gate checks the subagent's own worktree; read-only subagents exit 0 fast.

CORE="code-quality hygiene security separation testing"
WT=""

subagent_worktree() { # creates $WT: a linked worktree like the Agent tool's isolation
  WT="$T/worktrees/agent-a1b2c3"
  mkdir -p "$T/worktrees"
  git -C "$R" worktree add -q -b worktree-agent-a1b2c3 "$WT" HEAD
}

transcript() { # dir : a subagent transcript whose entries record that working directory
  mkdir -p "$T/transcripts"
  printf '%s\n' \
    "{\"type\":\"user\",\"cwd\":\"$1\",\"message\":{\"role\":\"user\",\"content\":\"go\"}}" \
    "{\"type\":\"assistant\",\"cwd\":\"$1\",\"message\":{\"role\":\"assistant\",\"content\":\"done\"}}" \
    '{"type":"assistant","message":{"role":"assistant","conten' \
    >"$T/transcripts/agent-a1b2c3.jsonl"
  printf '%s\n' "$T/transcripts/agent-a1b2c3.jsonl"
}

subagent_stop() { # cwd agent-type [fixture] [transcript]
  hook_input "${3:-subagent-stop}" CWD="$1" AGENT_ID=a1b2c3 AGENT_TYPE="$2" \
    AGENT_TRANSCRIPT="${4:-$T/no-transcript.jsonl}"
  run_hook stop-gate.sh
}

subagent_start() { # cwd agent-type
  hook_input subagent-start CWD="$1" AGENT_ID=a1b2c3 AGENT_TYPE="$2"
  run_hook subagent-start.sh
}

gate_setup() {
  make_repo
  stub_bin just claude
  subagent_worktree
}

test_subagent_failing_verify_in_its_worktree_blocks() {
  gate_setup
  printf '%s\n' 'pub fn half_done() {}' >"$WT/lib.rs"
  export STUB_JUST_RC=1 STUB_JUST_OUT="error: test domain::adds failed"
  subagent_stop "$WT" general-purpose
  assert_rc 2
  assert_contains "$HOOK_ERR" "just verify"
  assert_contains "$HOOK_ERR" "domain::adds failed"
  assert_eq "$WT" "$(call_cwd just)" "verify runs in the subagent's worktree"
}

test_subagent_green_worktree_runs_reviewers_there() {
  gate_setup
  mkdir -p "$WT/backend/crates/domain/src"
  printf '%s\n' 'pub fn done() {}' >"$WT/backend/crates/domain/src/lib.rs"
  subagent_stop "$WT" general-purpose
  assert_rc 0
  assert_reviewers_called $CORE
  assert_eq "$WT" "$(call_cwd claude)" "reviewers run in the subagent's worktree"
  assert_file "$WT/.claude/state/last-green"
  assert_no_file "$R/.claude/state/last-green"
}

test_subagent_worktree_comes_from_its_transcript() {
  gate_setup
  printf '%s\n' 'pub fn half_done() {}' >"$WT/lib.rs"
  export STUB_JUST_RC=1
  subagent_stop "$R" general-purpose subagent-stop "$(transcript "$WT")"
  assert_rc 2
  assert_eq "$WT" "$(call_cwd just)" "the transcript's cwd wins over the input cwd"
}

test_subagent_clean_read_only_subagent_exits_0_fast() {
  gate_setup
  subagent_stop "$WT" general-purpose
  assert_rc 0
  assert_eq "" "$HOOK_OUT"
  assert_eq 0 "$(calls just)" "no verify without changes against base"
  assert_eq 0 "$(calls claude)"
}

test_subagent_read_only_agent_types_exit_0_even_in_a_dirty_tree() {
  gate_setup
  printf '%s\n' 'pub fn parent_work() {}' >"$WT/lib.rs"
  local agent
  for agent in Explore Plan claude-code-guide security ux-memory ""; do
    subagent_stop "$WT" "$agent"
    assert_rc 0
  done
  assert_eq 0 "$(calls just)" "read-only agents never run verify"
}

test_subagent_that_changed_nothing_exits_0_in_a_dirty_tree() {
  gate_setup
  printf '%s\n' 'pub fn parent_work() {}' >"$WT/lib.rs"
  subagent_start "$WT" general-purpose
  assert_rc 0
  assert_eq "" "$HOOK_OUT" "SubagentStart adds no context"
  subagent_stop "$WT" general-purpose
  assert_rc 0
  assert_eq 0 "$(calls just)" "the parent's changes are the parent's Stop gate's job"
}

test_subagent_that_changed_files_after_start_is_gated() {
  gate_setup
  printf '%s\n' 'pub fn parent_work() {}' >"$WT/lib.rs"
  subagent_start "$WT" general-purpose
  printf '%s\n' 'pub fn subagent_work() {}' >"$WT/more.rs"
  export STUB_JUST_RC=1
  subagent_stop "$WT" general-purpose
  assert_rc 2
  assert_eq 1 "$(calls just)"
}

test_subagent_loop_breaker_asks_a_human() {
  gate_setup
  printf '%s\n' 'pub fn half_done() {}' >"$WT/lib.rs"
  export STUB_JUST_RC=1 STUB_JUST_OUT="error: test domain::adds failed"
  subagent_stop "$WT" general-purpose
  assert_rc 2
  subagent_stop "$WT" general-purpose subagent-stop-active
  assert_rc 0
  assert_eq 1 "$(calls just)" "no rerun on an unchanged worktree"
  assert_contains "$(out_json '.systemMessage')" "human"
}

test_subagent_loop_breaker_on_a_repeated_failure() {
  gate_setup
  export VAK_STOP_MAX_REPEATS=2 STUB_JUST_RC=1 STUB_JUST_OUT="error: test domain::adds failed"
  printf '%s\n' 'pub fn one() {}' >"$WT/lib.rs"
  subagent_stop "$WT" general-purpose
  assert_rc 2
  printf '%s\n' 'pub fn two() {}' >"$WT/lib.rs"
  subagent_stop "$WT" general-purpose subagent-stop-active
  assert_rc 0
  assert_contains "$(out_json '.systemMessage')" "human"
}

test_subagent_recursion_guard() {
  gate_setup
  printf '%s\n' 'pub fn half_done() {}' >"$WT/lib.rs"
  export CLAUDE_REVIEW_CHILD=1
  subagent_stop "$WT" general-purpose
  assert_rc 0
  subagent_start "$WT" general-purpose
  assert_rc 0
  assert_eq 0 "$(calls just)"
  assert_no_file "$WT/.claude/state/subagents"
}

test_subagent_start_never_fails_outside_a_repo() {
  make_repo
  mkdir -p "$T/elsewhere"
  subagent_start "$T/elsewhere" general-purpose
  assert_rc 0
  assert_eq "" "$HOOK_OUT"
}

assert_reviewers_called() { # expected-agent-names...
  assert_eq "$(printf '%s\n' "$@" | sort | tr '\n' ' ' | sed 's/ $//')" "$(reviewers_called)" "reviewers selected"
}
