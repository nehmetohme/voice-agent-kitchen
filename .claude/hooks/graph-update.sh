#!/usr/bin/env bash
# Async PostToolUse on Edit|Write: keep the Graphify code graph (graphify-out/) current.
#
# Runs `graphify update .` (pip package graphifyy: local tree-sitter, no LLM) in the
# work tree's root. One run at a time per work tree, using a lock folder in .claude/state/.
# Edits that arrive during a run only leave a "pending" flag; the running instance then
# runs once more, so a burst of edits costs at most two runs. Never blocks, prints nothing;
# the last run's output is in .claude/state/graph-update.log.
set -euo pipefail

STALE_LOCK_MINUTES=10

HOOK_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib/common.sh
. "$HOOK_DIR/lib/common.sh"

[ "${CLAUDE_REVIEW_CHILD:-}" = 1 ] && exit 0

input=$(cat)
root=$(repo_root "$(json_get "$input" cwd)") || exit 0
graphify=$(find_graphify) || exit 0

state=$(state_dir "$root")
lock="$state/graph-update.lock"
pending="$state/graph-update.pending"
log="$state/graph-update.log"

release_lock() {
  rm -rf "$lock"
  trap - EXIT
}

take_lock() {
  if mkdir "$lock" 2>/dev/null; then
    printf '%s\n' "$$" >"$lock/pid"
    trap release_lock EXIT
    return 0
  fi
  # Someone holds it. Take it over only if the holder is gone or the lock is old.
  local holder
  holder=$(cat "$lock/pid" 2>/dev/null || true)
  if { [ -n "$holder" ] && ! kill -0 "$holder" 2>/dev/null; } \
    || [ -n "$(find "$lock" -maxdepth 0 -mmin +"$STALE_LOCK_MINUTES" 2>/dev/null)" ]; then
    rm -rf "$lock"
    if mkdir "$lock" 2>/dev/null; then
      printf '%s\n' "$$" >"$lock/pid"
      trap release_lock EXIT
      return 0
    fi
  fi
  return 1
}

: >"$pending"
while :; do
  take_lock || exit 0 # the running instance will see the pending flag
  while [ -e "$pending" ]; do
    rm -f "$pending"
    (cd "$root" && "$graphify" update .) >"$log" 2>&1 </dev/null || true
  done
  release_lock
  # An edit may have set the flag between the last check and the release.
  [ -e "$pending" ] || break
done
exit 0
