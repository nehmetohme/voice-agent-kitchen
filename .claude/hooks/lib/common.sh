# shellcheck shell=bash
# Shared helpers for the hooks in .claude/hooks/. Bash 3.2 compatible (the macOS /bin/bash).
#
# Hooks find the repo from the `cwd` field of their JSON input, never from
# $CLAUDE_PROJECT_DIR: that variable keeps pointing at the main checkout when a
# session works in a worktree.

# ---- JSON: jq when installed, else python3. VAK_HOOK_JSON=jq|python3 forces one. ----------

if [ "${VAK_HOOK_JSON:-}" = jq ] || [ "${VAK_HOOK_JSON:-}" = python3 ]; then
  JSON_TOOL=$VAK_HOOK_JSON
elif command -v jq >/dev/null 2>&1; then
  JSON_TOOL=jq
elif command -v python3 >/dev/null 2>&1; then
  JSON_TOOL=python3
else
  JSON_TOOL=none
fi

# json_get JSON DOTTED.PATH: print the value; "" when missing or null; true/false for
# booleans; compact JSON for objects and lists. Never fails.
json_get() {
  case "$JSON_TOOL" in
    jq)
      printf '%s' "$1" | jq -r --arg p "$2" \
        'getpath($p | split(".")) | if . == null then "" elif type == "string" then . else tojson end' \
        2>/dev/null || true
      ;;
    python3)
      printf '%s' "$1" | python3 -I -c '
import json, sys
try:
    value = json.load(sys.stdin)
    for key in sys.argv[1].split("."):
        value = value.get(key) if isinstance(value, dict) else None
except Exception:
    value = None
if value is None:
    print("")
elif isinstance(value, str):
    print(value)
else:
    print(json.dumps(value, separators=(",", ":")))
' "$2" 2>/dev/null || true
      ;;
  esac
}

# json_string TEXT: print TEXT as a JSON string literal, quotes included.
json_string() {
  case "$JSON_TOOL" in
    jq) printf '%s' "$1" | jq -Rs . ;;
    python3) printf '%s' "$1" | python3 -I -c 'import json, sys; sys.stdout.write(json.dumps(sys.stdin.read()))' ;;
    *)
      local s
      s=$(printf '%s' "$1" | tr -d '\000-\010\013\014\016-\037')
      s=${s//\\/\\\\}
      s=${s//\"/\\\"}
      s=${s//$'\n'/\\n}
      s=${s//$'\t'/\\t}
      s=${s//$'\r'/\\r}
      printf '"%s"' "$s"
      ;;
  esac
}

# json_compact FILE: print the JSON document in FILE on one line.
json_compact() {
  case "$JSON_TOOL" in
    jq) jq -c . "$1" ;;
    *) python3 -I -c 'import json, sys; print(json.dumps(json.load(open(sys.argv[1])), separators=(",", ":")))' "$1" ;;
  esac
}

# ---- Paths and repos --------------------------------------------------------------------

# repo_root DIR: print the top level of the git work tree that holds DIR.
repo_root() {
  [ -n "${1:-}" ] || return 1
  git -C "$1" rev-parse --show-toplevel 2>/dev/null
}

# normalize_path ABS-PATH: drop "." and ".." segments without touching the disk.
normalize_path() {
  case "$1" in
    */./* | */../* | */. | */..) ;;
    *)
      printf '%s\n' "$1"
      return 0
      ;;
  esac
  local part out="" IFS=/
  set -f
  # shellcheck disable=SC2086 # split on "/" on purpose
  set -- $1
  set +f
  for part in "$@"; do
    case "$part" in
      '' | .) ;;
      ..) out=${out%/*} ;;
      *) out="$out/$part" ;;
    esac
  done
  printf '%s\n' "${out:-/}"
}

# locate_in_repo FILE [CWD]: set REPO_ROOT, REPO_REL (repo-relative) and REPO_ABS for an
# absolute file path, which may not exist yet. Returns 1 when it isn't inside a git repo.
locate_in_repo() {
  local file dir real root
  file=$(normalize_path "$1")
  dir=$(dirname "$file")
  while [ ! -d "$dir" ]; do dir=$(dirname "$dir"); done
  real=$(cd "$dir" && pwd -P) || return 1
  if [ "$dir" = / ]; then REPO_ABS=$file; else REPO_ABS="$real${file#"$dir"}"; fi
  root=$(repo_root "$real") || root=$(repo_root "${2:-}") || return 1
  case "$REPO_ABS" in
    "$root"/*) ;;
    *) return 1 ;;
  esac
  # shellcheck disable=SC2034 # read by the hooks that call this
  REPO_ROOT=$root
  # shellcheck disable=SC2034
  REPO_REL=${REPO_ABS#"$root"/}
}

# state_dir ROOT: print (and create) the gitignored hook state folder of a work tree.
state_dir() {
  mkdir -p "$1/.claude/state"
  printf '%s\n' "$1/.claude/state"
}

# ---- Processes ----------------------------------------------------------------------------

# run_with_timeout SECONDS CMD ARGS...: run CMD; exit 124 if it outlives SECONDS.
# Uses timeout/gtimeout when installed, else perl (macOS ships it). Kills the process group.
run_with_timeout() {
  local secs=$1
  shift
  if command -v timeout >/dev/null 2>&1; then
    timeout "$secs" "$@"
  elif command -v gtimeout >/dev/null 2>&1; then
    gtimeout "$secs" "$@"
  elif command -v perl >/dev/null 2>&1; then
    perl -MPOSIX=WNOHANG -MTime::HiRes=sleep -e '
      my $secs = shift @ARGV;
      my $pid = fork() // die "fork: $!\n";
      if ($pid == 0) { setpgrp(0, 0); exec { $ARGV[0] } @ARGV or die "exec $ARGV[0]: $!\n"; }
      setpgrp($pid, $pid);
      $SIG{ALRM} = sub {
        kill "TERM", -$pid;
        for (1 .. 20) { last if waitpid($pid, WNOHANG) != 0; sleep 0.1; }
        kill "KILL", -$pid;
        exit 124;
      };
      alarm $secs;
      waitpid($pid, 0);
      alarm 0;
      exit(($? & 127) ? 128 + ($? & 127) : $? >> 8);
    ' "$secs" "$@"
  else
    "$@"
  fi
}

# find_graphify: print the Graphify CLI from the pip package graphifyy (it has `update <path>`).
# An unrelated npm package also installs a `graphify` binary, so check before trusting PATH.
# VAK_GRAPHIFY=/path/to/graphify overrides the search.
find_graphify() {
  local candidate help
  for candidate in "${VAK_GRAPHIFY:-}" "${XDG_BIN_HOME:-${HOME:-/nonexistent}/.local/bin}/graphify" \
    "$(command -v graphify 2>/dev/null || true)"; do
    [ -n "$candidate" ] && [ -x "$candidate" ] || continue
    help=$("$candidate" --help 2>/dev/null || true)
    case "$help" in
      *"update <path>"*)
        printf '%s\n' "$candidate"
        return 0
        ;;
    esac
  done
  return 1
}
