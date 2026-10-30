#!/usr/bin/env bash
# Refresh the Graphify code graph (graphify-out/) of this checkout without blocking git.
#
# lefthook runs it from post-commit, post-checkout, post-merge and post-rewrite. It detaches at
# once and always exits 0, so git never waits for it and never fails because of it.
#
# - Linked worktrees work: it runs in `git rev-parse --show-toplevel`, the worktree's own root.
#   (Graphify's built-in `graphify hook install` skips linked worktrees, so we don't use it.)
# - One update at a time per checkout. A call that arrives during a run leaves a pending mark,
#   and the running worker does one more pass. The first pass waits VAK_GRAPH_DEBOUNCE seconds
#   so bursts (a rebase, checkout then merge) fold into one update.
# - The lock is shared with Claude Code's async post-edit hook (.claude/hooks/graph-update.sh):
#   .claude/state/graph-update.lock/ holds the worker's pid, and a lock whose pid is gone is
#   stale. .claude/state/ is gitignored.
# - Code only, no LLM and no network: GRAPHIFY_BACKEND=none. Log: graphify-out/refresh.log.
#
# Environment:
#   VAK_GRAPHIFY           path to the graphify CLI. Default: the first `graphify` found that is
#                          the pip package graphifyy (an unrelated npm package has the same name)
#   VAK_GRAPH_DEBOUNCE     seconds to wait before the first pass (default 2)
#   VAK_GRAPH_FOREGROUND=1 run in the foreground and print the log (debugging)
set -uo pipefail

log_max_bytes=524288

find_graphify() {
  local candidate
  if [ -n "${VAK_GRAPHIFY:-}" ]; then
    [ -x "$VAK_GRAPHIFY" ] && printf '%s\n' "$VAK_GRAPHIFY"
    return
  fi
  while IFS= read -r candidate; do
    [ -n "$candidate" ] && [ -x "$candidate" ] || continue
    if "$candidate" --help 2>/dev/null | grep -q 'update <path>'; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done < <(
    type -ap graphify
    printf '%s\n' "$HOME/.local/bin/graphify"
    if command -v uv >/dev/null 2>&1; then
      printf '%s/graphify\n' "$(uv tool dir --bin 2>/dev/null)"
    fi
  )
  return 1
}

# Take the lock, or return 1 if a live worker holds it.
acquire() {
  local pid
  if mkdir "$lock" 2>/dev/null; then
    printf '%s\n' "$$" >"$lock/pid"
    return 0
  fi
  pid=$(cat "$lock/pid" 2>/dev/null || true)
  if [ -z "$pid" ]; then
    sleep 1 # the holder may be between mkdir and writing its pid
    pid=$(cat "$lock/pid" 2>/dev/null || true)
  fi
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
    return 1
  fi
  rm -rf "$lock" # stale: its worker is gone
  if mkdir "$lock" 2>/dev/null; then
    printf '%s\n' "$$" >"$lock/pid"
    return 0
  fi
  return 1
}

release() {
  rm -rf "$lock"
}

worker() {
  local graphify rc
  if ! acquire; then
    touch "$pending" # the running worker picks this up
    return 0
  fi
  # Every normal path releases explicitly; the trap only covers a worker killed mid-run.
  trap release EXIT
  if ! graphify=$(find_graphify); then
    echo "$(date '+%F %T') graphify (pip package graphifyy) not found; set VAK_GRAPHIFY. Skipping."
    rm -f "$pending"
    release
    trap - EXIT
    return 0
  fi
  sleep "${VAK_GRAPH_DEBOUNCE:-2}"
  while :; do
    while :; do
      rm -f "$pending"
      echo "$(date '+%F %T') $graphify update ."
      GRAPHIFY_BACKEND=none GRAPHIFY_NO_TIPS=1 GRAPHIFY_NO_CTA=1 GRAPHIFY_QUERY_LOG_DISABLE=1 \
        "$graphify" update .
      rc=$?
      [ "$rc" -eq 0 ] || echo "$(date '+%F %T') graphify update exited $rc"
      [ -e "$pending" ] || break
    done
    release
    # A call may have marked pending after the last check but before the release.
    [ -e "$pending" ] || break
    acquire || break # another worker took over and will see the mark
  done
  trap - EXIT
}

self="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
root=$(git rev-parse --show-toplevel 2>/dev/null) || exit 0
cd "$root" || exit 0
# Git sets these for some hooks; graphify should find the checkout from the working directory.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES

state="$root/.claude/state"
lock="$state/graph-update.lock"
pending="$state/graph-update.pending"
logfile="$root/graphify-out/refresh.log"
mkdir -p "$state" "$root/graphify-out" || exit 0

if [ "${1:-}" = "--worker" ]; then
  worker
  exit 0
fi

if [ "${VAK_GRAPH_FOREGROUND:-}" = "1" ]; then
  worker 2>&1 | tee -a "$logfile"
  exit 0
fi

# Keep the log small: once it passes the limit, keep its last 200 lines.
if [ -f "$logfile" ] && [ "$(wc -c <"$logfile")" -gt "$log_max_bytes" ]; then
  tail -n 200 "$logfile" >"$logfile.tmp" && mv "$logfile.tmp" "$logfile"
fi
nohup "$self" --worker </dev/null >>"$logfile" 2>&1 &
exit 0
