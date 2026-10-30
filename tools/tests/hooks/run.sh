#!/usr/bin/env bash
# Test runner for the Claude Code hook scripts in .claude/hooks/.
#
#   tools/tests/hooks/run.sh [filter]
#
# Runs every test_* function in tools/tests/hooks/test_*.sh, each in its own subshell and
# sandbox (see lib.sh). `filter` keeps only tests whose name contains it.
# VAK_TEST_KEEP=1 keeps each sandbox for debugging.
set -euo pipefail

TESTS_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
filter=${1:-}

for tool in git jq; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "run.sh: $tool is required for the hook tests" >&2
    exit 1
  }
done

passed=0
failed=0
failed_names=""
log=$(mktemp "${TMPDIR:-/tmp}/vak-hook-test.XXXXXX")
trap 'rm -f "$log"' EXIT

for file in "$TESTS_DIR"/test_*.sh; do
  names=$(bash -c '. "$1"; compgen -A function test_ || true' _ "$file")
  for name in $names; do
    case "$name" in *"$filter"*) ;; *) continue ;; esac
    set +e
    (
      set -euo pipefail
      # shellcheck source=lib.sh
      . "$TESTS_DIR/lib.sh"
      # shellcheck disable=SC1090
      . "$file"
      sandbox
      trap cleanup EXIT
      "$name"
    ) >"$log" 2>&1
    rc=$?
    set -e
    if [ "$rc" -eq 0 ]; then
      passed=$((passed + 1))
      printf 'ok    %s\n' "$name"
    else
      failed=$((failed + 1))
      failed_names="$failed_names $name"
      printf 'FAIL  %s\n' "$name"
      sed 's/^/      | /' "$log"
    fi
  done
done

printf '\nhook tests: %d passed, %d failed\n' "$passed" "$failed"
if [ "$failed" -gt 0 ]; then
  printf 'failed:%s\n' "$failed_names"
  exit 1
fi
[ "$passed" -gt 0 ] || {
  echo "no tests matched '$filter'" >&2
  exit 1
}
