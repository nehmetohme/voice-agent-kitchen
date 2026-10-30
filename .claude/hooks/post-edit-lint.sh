#!/usr/bin/env bash
# PostToolUse on Edit|Write: format and fast-lint the one file Claude just edited.
#
#   .rs          rustfmt the file, then clippy the package that owns it
#   .ts/.tsx/.js the frontend's own per-file check: `just frontend-check-file <path>`
#   .toml        taplo fmt the file
#
# A failure prints {"decision":"block","reason":...}; Claude sees it next to the edit.
# An area or tool that isn't set up yet is skipped. A step that runs out of time is
# reported as context, not as a failure (`just verify` covers it). Always exits 0.
set -euo pipefail

# ---- Commands by file type. Adjust here as the backend and frontend land. ----------------
# Each is a command line; the hook appends the file's absolute path, except RUST_LINT, which
# runs in the package folder. Every value can be overridden by the matching VAK_LINT_* variable.
RUST_FORMAT=${VAK_LINT_RUST_FORMAT:-"rustfmt --edition 2024"}
RUST_LINT=${VAK_LINT_RUST_LINT:-"cargo clippy --quiet --all-targets -- -D warnings"}
# Frontend: the recipe in frontend/frontend.just formats and lints one file, exits non-zero on
# lint errors, and runs from the repo root. Skipped until the recipe and node_modules exist.
TS_CHECK=${VAK_LINT_TS_CHECK:-"just frontend-check-file"}
TOML_FORMAT=${VAK_LINT_TOML_FORMAT:-"taplo fmt"}
# Time budgets in seconds. The hook timeout is 60 s, and clippy can run long on a cold build.
SLOW_BUDGET_S=${VAK_LINT_BUDGET_S:-40}
FAST_BUDGET_S=15
REASON_LINES=60

HOOK_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib/common.sh
. "$HOOK_DIR/lib/common.sh"

[ "${CLAUDE_REVIEW_CHILD:-}" = 1 ] && exit 0

input=$(cat)
file=$(json_get "$input" tool_input.file_path)
[ -n "$file" ] && [ -f "$file" ] || exit 0
locate_in_repo "$file" "$(json_get "$input" cwd)" || exit 0
root=$REPO_ROOT
rel=$REPO_REL

output=$(mktemp "${TMPDIR:-/tmp}/vak-lint.XXXXXX")
trap 'rm -f "$output"' EXIT
problems=""
notes=""

# available DIR COMMAND-LINE: is the command's program installed (relative to DIR if it
# names a path)?
available() {
  local program=${2%% *}
  case "$program" in
    /*) [ -x "$program" ] ;;
    */*) [ -x "$1/$program" ] ;;
    *) command -v "$program" >/dev/null 2>&1 ;;
  esac
}

# ready COMMAND-LINE: installed, and for `just <recipe>`, the recipe exists.
ready() {
  local -a cmd
  local summary
  read -r -a cmd <<<"$1"
  available "$root" "$1" || return 1
  [ "${cmd[0]}" = just ] && [ -n "${cmd[1]:-}" ] || return 0
  summary=$(cd "$root" && just --summary 2>/dev/null) || return 1
  case " $summary " in *" ${cmd[1]} "*) return 0 ;; esac
  return 1
}

# step LABEL DIR BUDGET COMMAND-LINE [ARGS...]: run one check. Returns non-zero on failure.
step() {
  local label=$1 dir=$2 budget=$3 rc=0
  local -a cmd
  read -r -a cmd <<<"$4"
  shift 4
  (cd "$dir" && run_with_timeout "$budget" "${cmd[@]}" "$@") >"$output" 2>&1 </dev/null || rc=$?
  if [ "$rc" -eq 0 ]; then
    return 0
  fi
  if [ "$rc" -eq 124 ]; then
    notes="$notes$label did not finish within ${budget}s, so it was skipped; \`just verify\` will check it."$'\n'
    return 0
  fi
  problems="$problems$label failed on $rel (exit $rc):"$'\n'"$(tail -n "$REASON_LINES" "$output")"$'\n\n'
  return 1
}

# package_dir FILE: nearest folder up to the repo root with a Cargo.toml that has [package].
package_dir() {
  local dir
  dir=$(dirname "$1")
  while :; do
    if [ -f "$dir/Cargo.toml" ] && grep -q '^\[package\]' "$dir/Cargo.toml"; then
      printf '%s\n' "$dir"
      return 0
    fi
    [ "$dir" != "$root" ] && [ "$dir" != / ] || return 1
    dir=$(dirname "$dir")
  done
}

case "$rel" in
  *.rs)
    formatted=ok
    if available "$root" "$RUST_FORMAT"; then
      step rustfmt "$root" "$FAST_BUDGET_S" "$RUST_FORMAT" "$REPO_ABS" || formatted=failed
    fi
    if [ "$formatted" = ok ] && pkg=$(package_dir "$REPO_ABS") && available "$pkg" "$RUST_LINT"; then
      step "clippy (${pkg#"$root"/})" "$pkg" "$SLOW_BUDGET_S" "$RUST_LINT" || true
    fi
    ;;
  frontend/src/shared/api/generated/* | frontend/src/app/routeTree.gen.ts | frontend/src/shared/ui/tokens.css)
    ;; # generated: never formatted here
  frontend/*.ts | frontend/*.tsx | frontend/*.mts | frontend/*.cts | frontend/*.js | frontend/*.jsx | frontend/*.mjs | frontend/*.cjs)
    [ -f "$root/frontend/package.json" ] && [ -d "$root/frontend/node_modules" ] || exit 0
    if ready "$TS_CHECK"; then
      step "frontend check (${TS_CHECK})" "$root" "$SLOW_BUDGET_S" "$TS_CHECK" "$REPO_ABS" || true
    fi
    ;;
  *.toml)
    if available "$root" "$TOML_FORMAT"; then
      step "taplo fmt" "$root" "$FAST_BUDGET_S" "$TOML_FORMAT" "$REPO_ABS" || true
    fi
    ;;
esac

if [ -n "$problems" ]; then
  printf '{"decision":"block","reason":%s}\n' \
    "$(json_string "Post-edit check failed. Fix it before moving on."$'\n\n'"$problems")"
elif [ -n "$notes" ]; then
  printf '{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":%s}}\n' \
    "$(json_string "$notes")"
fi
exit 0
