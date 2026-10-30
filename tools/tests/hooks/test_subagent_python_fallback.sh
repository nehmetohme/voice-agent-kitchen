# shellcheck shell=bash
# The SubagentStop path without jq: the transcript reader and the SubagentStart record fall
# back to python3.

test_subagent_transcript_and_start_record_without_jq() {
  make_repo
  stub_bin just claude
  export VAK_HOOK_JSON=python3 STUB_JUST_RC=1
  local wt="$T/worktrees/agent-f00d"
  mkdir -p "$T/worktrees" "$T/transcripts"
  git -C "$R" worktree add -q -b worktree-agent-f00d "$wt" HEAD
  printf '%s\n' "{\"type\":\"user\",\"cwd\":\"$wt\"}" 'not json' >"$T/transcripts/agent-f00d.jsonl"

  printf '%s\n' 'pub fn parent_work() {}' >"$wt/lib.rs"
  hook_input subagent-start CWD="$wt" AGENT_ID=f00d AGENT_TYPE=general-purpose
  run_hook subagent-start.sh
  assert_rc 0
  hook_input subagent-stop CWD="$R" AGENT_ID=f00d AGENT_TYPE=general-purpose \
    AGENT_TRANSCRIPT="$T/transcripts/agent-f00d.jsonl"
  run_hook stop-gate.sh
  assert_rc 0
  assert_eq 0 "$(calls just)" "unchanged since start"

  hook_input subagent-start CWD="$wt" AGENT_ID=f00d AGENT_TYPE=general-purpose
  run_hook subagent-start.sh
  printf '%s\n' 'pub fn subagent_work() {}' >"$wt/more.rs"
  hook_input subagent-stop CWD="$R" AGENT_ID=f00d AGENT_TYPE=general-purpose \
    AGENT_TRANSCRIPT="$T/transcripts/agent-f00d.jsonl"
  run_hook stop-gate.sh
  assert_rc 2
  assert_eq "$wt" "$(call_cwd just)"
}
