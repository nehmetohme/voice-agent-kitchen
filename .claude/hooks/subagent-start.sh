#!/usr/bin/env bash
# SubagentStart: remember the work tree's fingerprint when a subagent starts, keyed by its
# agent_id, in .claude/state/subagents/. On SubagentStop, stop-gate.sh compares: a subagent
# that changed nothing (research, review, exploration) exits at once, even when it shares a
# tree that its parent has already changed. That tree is the parent's own gate's job.
#
# Never blocks and prints nothing, so it adds no context to the subagent. Always exits 0.
set -euo pipefail

HOOK_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib/common.sh
. "$HOOK_DIR/lib/common.sh"

[ "${CLAUDE_REVIEW_CHILD:-}" = 1 ] && exit 0

input=$(cat)
agent_id=$(json_get "$input" agent_id)
[ -n "$agent_id" ] || exit 0
root=$(repo_root "$(json_get "$input" cwd)") || exit 0
cd "$root"
records="$(state_dir "$root")/subagents"
mkdir -p "$records"
find "$records" -name '*.start' -mtime +2 -exec rm -f {} + 2>/dev/null || true
tree_fingerprint "$(state_dir "$root")" >"$records/$(safe_name "$agent_id").start" 2>/dev/null || true
exit 0
