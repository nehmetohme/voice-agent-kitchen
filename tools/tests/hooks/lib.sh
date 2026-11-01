# shellcheck shell=bash
# Helpers for the Claude Code hook tests. run.sh sources this in each test's subshell.
# Every test gets a sandbox: a temp dir, a throwaway git repo with a local "origin",
# stubbed tools on PATH, and an isolated HOME and git config. No network, no real claude.

TESTS_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "$TESTS_DIR/../../.." && pwd)
HOOKS_DIR="$REPO_ROOT/.claude/hooks"
FIXTURES="$TESTS_DIR/fixtures"
STUBS="$TESTS_DIR/stubs"
ORIG_PATH=$PATH
T=""
R=""
HOOK_RC=""
HOOK_OUT=""
HOOK_ERR=""

# ---- assertions -----------------------------------------------------------------------

fail() {
  printf 'FAILED: %s\n' "$*" >&2
  exit 1
}

assert_eq() { # expected actual [message]
  [ "$1" = "$2" ] || fail "${3:-values differ}"$'\n'"  expected: [$1]"$'\n'"  actual:   [$2]"
}

assert_contains() { # haystack needle [message]
  case "$1" in
    *"$2"*) ;;
    *) fail "${3:-text not found}: [$2] in:"$'\n'"$1" ;;
  esac
}

assert_not_contains() { # haystack needle [message]
  case "$1" in
    *"$2"*) fail "${3:-unexpected text}: [$2] in:"$'\n'"$1" ;;
  esac
}

assert_rc() { # expected-exit-code
  [ "$HOOK_RC" = "$1" ] \
    || fail "exit code: expected $1, got $HOOK_RC"$'\n'"stdout: $HOOK_OUT"$'\n'"stderr: $HOOK_ERR"
}

out_json() { # jq-filter: read a field from the hook's stdout
  printf '%s' "$HOOK_OUT" | jq -r "$1" 2>/dev/null || fail "stdout is not JSON: [$HOOK_OUT]"
}

assert_json() { # jq-filter expected
  assert_eq "$2" "$(out_json "$1")" "stdout $1"
}

assert_file() { [ -e "$1" ] || fail "missing file: $1"; }
assert_no_file() { [ ! -e "$1" ] || fail "file should not exist: $1"; }

# ---- sandbox ----------------------------------------------------------------------------

find_on_orig_path() { # tool : first executable of that name on the caller's PATH
  local dir IFS=:
  for dir in $ORIG_PATH; do
    if [ -n "$dir" ] && [ -x "$dir/$1" ]; then
      printf '%s\n' "$dir/$1"
      return 0
    fi
  done
}

sandbox() {
  T=$(mktemp -d "${TMPDIR:-/tmp}/vak-hooks.XXXXXX")
  T=$(cd "$T" && pwd -P)
  mkdir -p "$T/bin" "$T/home" "$T/claude"
  local tool real
  for tool in git jq python3 perl shasum sha256sum; do
    real=$(find_on_orig_path "$tool")
    [ -n "$real" ] && ln -s "$real" "$T/bin/$tool"
  done
  export PATH="$T/bin:/usr/bin:/bin"
  export HOME="$T/home"
  # Git exports these to its hooks, and lefthook's pre-push runs these tests (`just verify`).
  # Inherited, they aim every sandbox git command at the real repository: `git init --bare`
  # then sets core.bare in its shared config, and the fixture commit lands on the real branch.
  unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR GIT_OBJECT_DIRECTORY \
    GIT_ALTERNATE_OBJECT_DIRECTORIES
  export GIT_CONFIG_GLOBAL="$T/home/.gitconfig" GIT_CONFIG_NOSYSTEM=1
  git config --global user.name "Hook Test"
  git config --global user.email "hook-test@example.invalid"
  git config --global init.defaultBranch main
  git config --global commit.gpgsign false
  git config --global tag.gpgsign false
  export STUB_LOG="$T/stub.log" STUB_CLAUDE_DIR="$T/claude"
  : >"$STUB_LOG"
  unset CLAUDE_REVIEW_CHILD CLAUDE_PROJECT_DIR VAK_REVIEWERS VAK_HOOK_JSON VAK_HOOK_TOML STUB_JUST_RECIPES \
    VAK_GRAPHIFY VAK_STOP_MAX_REPEATS VAK_REVIEW_MAX_TURNS VAK_REVIEW_MAX_BUDGET_USD \
    VAK_REVIEW_CONCURRENCY VAK_LINT_BUDGET_S XDG_BIN_HOME
  # Safety net: a real claude must never be reachable from a test.
  if command -v claude >/dev/null 2>&1; then
    fail "a real claude is on PATH inside the sandbox: $(command -v claude)"
  fi
}

cleanup() {
  wait 2>/dev/null || true
  if [ -n "${VAK_TEST_KEEP:-}" ]; then
    printf 'kept sandbox: %s\n' "$T" >&2
  elif [ -n "$T" ]; then
    rm -rf "$T"
  fi
}

stub_bin() { # tool... : put fake executables on PATH
  local tool src
  for tool in "$@"; do
    case "$tool" in
      claude | graphify | just) src="$STUBS/$tool" ;;
      *) src="$STUBS/generic" ;;
    esac
    ln -sf "$src" "$T/bin/$tool"
  done
}

make_repo() { # creates $R: a git repo on main, pushed to a local bare "origin"
  R="$T/repo"
  git init -q --bare "$T/origin.git"
  git init -q "$R"
  printf '%s\n' '.claude/state/' 'graphify-out/' 'node_modules/' 'target/' >"$R/.gitignore"
  printf '%s\n' '# fixture repo' >"$R/README.md"
  git -C "$R" add -A
  git -C "$R" commit -q -m "chore: init"
  git -C "$R" remote add origin "$T/origin.git"
  git -C "$R" push -q origin main
  git -C "$R" checkout -q -b task/test
}

write_file() { # repo-relative-path [content]
  mkdir -p "$(dirname "$R/$1")"
  printf '%s\n' "${2:-// test content}" >"$R/$1"
}

commit_all() { # message
  git -C "$R" add -A
  git -C "$R" commit -q -m "$1"
}

hook_input() { # fixture-name KEY=value... : render fixtures/<name>.json into $T/input.json
  local text kv key value
  text=$(cat "$FIXTURES/$1.json")
  shift
  for kv in "$@"; do
    key=${kv%%=*}
    value=${kv#*=}
    text=${text//__${key}__/$value}
  done
  printf '%s\n' "$text" >"$T/input.json"
}

run_hook() { # script [input-file] : sets HOOK_RC, HOOK_OUT, HOOK_ERR
  local input=${2:-$T/input.json}
  set +e
  "$HOOKS_DIR/$1" <"$input" >"$T/hook.out" 2>"$T/hook.err"
  HOOK_RC=$?
  set -e
  HOOK_OUT=$(cat "$T/hook.out")
  HOOK_ERR=$(cat "$T/hook.err")
}

# ---- stub log queries -------------------------------------------------------------------

calls() { # tool : number of logged calls
  awk -F'\t' -v n="$1" '$1 == n' "$STUB_LOG" | wc -l | tr -d ' '
}

call_line() { # tool : args of the first logged call
  awk -F'\t' -v n="$1" '$1 == n { print $3; exit }' "$STUB_LOG"
}

call_cwd() { # tool : working directory of the first logged call
  awk -F'\t' -v n="$1" '$1 == n { print $2; exit }' "$STUB_LOG"
}

reviewers_called() { # sorted, space-separated agent names passed to claude --agent
  awk -F'\t' '$1 == "claude"' "$STUB_LOG" \
    | sed -n 's/.*agent=\([a-z-]*\).*/\1/p' | sort | tr '\n' ' ' | sed 's/ $//'
}
