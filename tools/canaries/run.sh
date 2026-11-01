#!/usr/bin/env bash
# Gate self-tests ("canaries"): prove that every gate fails on a deliberate violation.
#
#   tools/canaries/run.sh              run every canary
#   tools/canaries/run.sh NAME...      run only these
#   tools/canaries/run.sh --list       list the canaries
#
# A canary is a folder tools/canaries/<name>/ holding:
#   canary.conf  key=value lines: gate= and violation= (for the reader), command= (run from the
#                worktree root) and expect= (an extended regex the failure output must match)
#   patch.diff   the violation as a git patch against main, or
#   apply.sh     a script that makes it, run from the worktree root, for a violation a diff
#                can't hold: a secret assembled at run time, or an edit that must survive churn
#
# For each canary: add a temporary detached worktree of HEAD under .claude/worktrees/, make the
# violation and commit it there, run `command`, and require a non-zero exit AND `expect` in the
# output. The commit uses plumbing (write-tree, commit-tree), so no git hook runs: the gate
# under test must be the one that catches the violation. The worktree is always removed.
#
# Exit 0 when every gate failed as expected, 1 when a canary was not caught (the summary names
# it), 2 on a usage error.
#
# Environment:
#   E2E_PORT               Playwright port for the e2e canaries (default 4176)
#   VAK_CANARY_TARGET_DIR  cargo target dir shared by every canary worktree, so dependencies
#                          build once (default backend/target/canaries)
#   VAK_CANARY_LOG_DIR     full output of each canary (default .claude/state/canaries)
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
log_dir=${VAK_CANARY_LOG_DIR:-$root/.claude/state/canaries}
export E2E_PORT="${E2E_PORT:-4176}"
export CARGO_TARGET_DIR="${VAK_CANARY_TARGET_DIR:-$root/backend/target/canaries}"

usage_error() {
  echo "canaries: $*" >&2
  exit 2
}

all_names() {
  local dir
  for dir in "$here"/*/; do
    if [ -f "$dir/canary.conf" ]; then basename "$dir"; fi
  done
}

# Sets gate, violation, cmd and expect from <dir>/canary.conf.
read_conf() {
  local dir=$1 line
  gate='' violation='' cmd='' expect=''
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      '' | '#'*) ;;
      gate=*) gate=${line#*=} ;;
      violation=*) violation=${line#*=} ;;
      command=*) cmd=${line#*=} ;;
      expect=*) expect=${line#*=} ;;
      *) usage_error "$dir/canary.conf: unknown line: $line" ;;
    esac
  done <"$dir/canary.conf"
  if [ -z "$cmd" ] || [ -z "$expect" ]; then
    usage_error "$dir/canary.conf: command= and expect= are required"
  fi
  if [ -f "$dir/patch.diff" ] && [ -f "$dir/apply.sh" ]; then
    usage_error "$dir: has both patch.diff and apply.sh; keep one"
  fi
  if [ ! -f "$dir/patch.diff" ] && [ ! -f "$dir/apply.sh" ]; then
    usage_error "$dir: needs patch.diff or apply.sh"
  fi
}

list() {
  local name
  printf '%-20s %-50s %s\n' CANARY GATE VIOLATION
  for name in $(all_names); do
    read_conf "$here/$name"
    printf '%-20s %-50s %s\n' "$name" "$gate" "$violation"
  done
}

worktree=''
remove_worktree() {
  if [ -n "$worktree" ]; then
    git -C "$root" worktree remove --force "$worktree" >/dev/null 2>&1 || rm -rf "$worktree"
    git -C "$root" worktree prune
    worktree=''
  fi
}
trap remove_worktree EXIT
trap 'exit 130' INT TERM

strip_ansi() {
  sed -e $'s/\033\\[[0-9;]*[A-Za-z]//g'
}

# Makes the violation in $worktree. Returns 1 if it no longer applies to HEAD.
make_violation() {
  local dir=$1 log=$2
  if [ -f "$dir/patch.diff" ]; then
    # --3way merges with the patch's base blobs, so a patch still applies after unrelated
    # lines near it move. A conflict or a missing file is stale.
    git -C "$worktree" apply --3way "$dir/patch.diff" >>"$log" 2>&1 || return 1
  else
    (cd "$worktree" && bash "$dir/apply.sh") >>"$log" 2>&1 || return 1
  fi
}

# Commits the worktree's changes on its detached HEAD, so checks that read commits (the
# contract check) see the violation too. Returns 1 if there is no change, 2 if git fails.
commit_violation() {
  local name=$1 tree commit
  git -C "$worktree" add --all || return 2
  if git -C "$worktree" diff --cached --quiet; then return 1; fi
  tree=$(git -C "$worktree" write-tree) || return 2
  commit=$(git -C "$worktree" -c user.name=vak-canary -c user.email=vak-canary@invalid \
    commit-tree --no-gpg-sign -p HEAD -m "canary: $name" "$tree") || return 2
  git -C "$worktree" update-ref --no-deref HEAD "$commit" || return 2
}

# Runs one canary; prints its result and sets `result` to caught, missed, wrong, stale or error.
run_canary() {
  local name=$1 dir="$here/$1" log="$log_dir/$1.log" start status
  read_conf "$dir"
  echo "== $name: $gate"
  echo "   violation: $violation"
  : >"$log"
  start=$SECONDS

  mkdir -p "$root/.claude/worktrees"
  worktree=$(mktemp -d "$root/.claude/worktrees/canary-$name.XXXXXX")
  # No hooks: the worktree is only a build input (the same as contracts-check-ci).
  if ! git -C "$root" -c core.hooksPath=/dev/null worktree add --quiet --detach "$worktree" "$head" >>"$log" 2>&1; then
    result=error
    echo "   ERROR: could not add a worktree of HEAD (log: $log)"
    remove_worktree
    return
  fi
  if ! make_violation "$dir" "$log"; then
    result=stale
    echo "   STALE: the violation no longer applies to HEAD ($head_short). Refresh the canary:"
    echo "          see docs/ci.md, Gate self-tests. Output:"
    sed 's/^/     | /' "$log" | tail -n 15
    remove_worktree
    return
  fi
  status=0
  commit_violation "$name" >>"$log" 2>&1 || status=$?
  if [ "$status" -ne 0 ]; then
    if [ "$status" -eq 1 ]; then
      result=stale
      echo "   STALE: the violation changes nothing in HEAD ($head_short); refresh the canary"
    else
      result=error
      echo "   ERROR: could not commit the violation (log: $log)"
    fi
    remove_worktree
    return
  fi

  echo "   running: $cmd"
  printf '== %s\n' "$cmd" >>"$log"
  # Only the command's own output counts, not the setup output above it.
  local from
  from=$(($(wc -l <"$log") + 1))
  set +e
  (cd "$worktree" && bash -c "$cmd") </dev/null 2>&1 | strip_ansi >>"$log"
  status=${PIPESTATUS[0]}
  set -e
  remove_worktree

  local took=$((SECONDS - start))s
  if [ "$status" -eq 0 ]; then
    result=missed
    echo "   MISSED in $took: \`$cmd\` passed with the violation in place: the gate let it through"
  elif grep -Eq -- "$expect" <(tail -n "+$from" "$log"); then
    result=caught
    echo "   caught in $took: exit $status, and the output matches /$expect/"
    return
  else
    result=wrong
    echo "   WRONG FAILURE in $took: exit $status, but the output never matches /$expect/,"
    echo "   so it failed for another reason and this gate is unproven"
  fi
  echo "   last lines of $log:"
  tail -n 25 "$log" | sed 's/^/     | /'
}

names=''
for arg in "$@"; do
  case "$arg" in
    --list)
      list
      exit 0
      ;;
    -h | --help)
      sed -n '2,8p' "${BASH_SOURCE[0]}"
      exit 0
      ;;
    -*) usage_error "unknown option $arg" ;;
    *)
      [ -f "$here/$arg/canary.conf" ] ||
        usage_error "unknown canary '$arg'; known: $(all_names | paste -sd ' ' -)"
      names="$names $arg"
      ;;
  esac
done
[ -n "$names" ] || names=$(all_names)

head=$(git -C "$root" rev-parse --verify HEAD)
head_short=$(git -C "$root" rev-parse --short HEAD)
mkdir -p "$log_dir"
echo "canaries: against HEAD $head_short (uncommitted changes are not included); logs in $log_dir"

total=0
caught=0
failures=''
for name in $names; do
  total=$((total + 1))
  run_canary "$name"
  if [ "$result" = caught ]; then
    caught=$((caught + 1))
  else
    failures="$failures$(printf '  %-8s %s\n' "$result" "$name")"$'\n'
  fi
done

echo
if [ -z "$failures" ]; then
  echo "canaries: all $total caught: every gate failed on its violation"
  exit 0
fi
echo "canaries: $caught of $total caught. Not caught (missed = the gate let a violation through):"
printf '%s' "$failures"
exit 1
