#!/usr/bin/env bash
# Stop hook: the "done" gate. Claude may finish only on a green work tree.
#
#  1. Inside a reviewer child (CLAUDE_REVIEW_CHILD=1): exit 0 at once, so reviews never recurse.
#  2. Fingerprint the work tree (tracked and untracked files, as a git tree id). Exit 0 when it
#     equals the base branch (nothing to gate) or the last green run (.claude/state/last-green).
#  3. Run `just verify`. On failure: exit 2 with the end of its output on stderr.
#  4. Pick reviewers from the paths changed against origin/main (select_reviewers below), so
#     the model can't skip one.
#  5. Run them in parallel as headless `claude -p --agent <name>` children, with the verdict
#     schema, turn, budget and time caps, read-only tools and no permission prompts.
#  6. Any "fail" verdict, blocker or major finding, or missing or unparseable verdict:
#     exit 2 with the findings on stderr, so Claude keeps working.
#  7. Loop breaker: when Claude is already continuing because of this hook (stop_hook_active)
#     and the work tree hasn't changed since the failure, or the same failure has come back
#     VAK_STOP_MAX_REPEATS times in a row, exit 0 and ask a human instead of blocking again.
#
# Human-only switch. Export it in your own shell before you start `claude`. Claude must not
# set it, and the pre-edit guard refuses it in .claude/settings*.json:
#   VAK_REVIEWERS=off   skip the reviewer agents. `just verify` still runs, and every run
#                       shows a notice that reviewers were skipped.
#
# Tuning (env): VAK_REVIEW_MAX_TURNS, VAK_REVIEW_MAX_BUDGET_USD, VAK_REVIEW_TIMEOUT_S,
# VAK_REVIEW_CONCURRENCY, VAK_REVIEW_DIFF_MAX_BYTES, VAK_STOP_MAX_REPEATS,
# VAK_STOP_DEADLINE_S, VAK_BASE_REF, VAK_VERIFY_CMD.
#
# Run it by hand on the current folder:  .claude/hooks/stop-gate.sh --manual
# Logs of the last run: .claude/state/stop-gate/
set -euo pipefail

VERIFY_CMD=${VAK_VERIFY_CMD:-just verify}
BASE_REF=${VAK_BASE_REF:-origin/main}
MAX_TURNS=${VAK_REVIEW_MAX_TURNS:-40}
MAX_BUDGET_USD=${VAK_REVIEW_MAX_BUDGET_USD:-2.00}
REVIEW_TIMEOUT_S=${VAK_REVIEW_TIMEOUT_S:-600}
CONCURRENCY=${VAK_REVIEW_CONCURRENCY:-8}
# Whole-run deadline, below the hook's 1800 s timeout: a killed hook would let Claude stop,
# so verify and reviewers that can't finish in time count as failures instead.
DEADLINE_S=${VAK_STOP_DEADLINE_S:-1700}
DIFF_MAX_BYTES=${VAK_REVIEW_DIFF_MAX_BYTES:-60000}
MAX_REPEATS=${VAK_STOP_MAX_REPEATS:-3}
REVIEWERS_MODE=${VAK_REVIEWERS:-on}
[ "$REVIEWERS_MODE" = off ] || REVIEWERS_MODE=on

# Run order, and the full set of reviewer agents in .claude/agents/.
ALL_REVIEWERS="security code-quality hygiene separation testing performance ci-cd dependencies docs ux-cognitive ux-interaction ux-perception ux-memory ux-complexity"

# Reviewer children: read-only tools, no prompts. Glob and Grep must be named to be available.
ALLOWED_TOOLS="Read,Grep,Glob,Bash(git diff *),Bash(git log *),Bash(git show *),Bash(git status *),Bash(git ls-files *),Bash(git merge-base *),Bash(git rev-parse *),Bash(git for-each-ref *),Bash(graphify query *),Bash(graphify affected *),Bash(graphify explain *),Bash(graphify path *)"
DISALLOWED_TOOLS="Edit,Write,NotebookEdit,Agent"

# Reviewer map: docs/plan/quality.md §5. Reads changed paths on stdin and prints the
# selected reviewers in run order. Adjust the patterns here.
select_reviewers() {
  local path wanted=" " name
  while IFS= read -r path; do
    [ -n "$path" ] || continue
    case "$path" in
      *.rs | *.ts | *.tsx | *.mts | *.cts | *.js | *.jsx | *.mjs | *.cjs | *.sh | *.py | *.sql)
        wanted="$wanted security code-quality hygiene separation testing "
        ;;
    esac
    case "$path" in
      backend/crates/pipeline/* | backend/crates/audio/* | backend/crates/adapters/* | \
        backend/crates/repositories-pg/* | frontend/src/features/testing/* | frontend/src/shared/voice/*)
        wanted="$wanted performance "
        ;;
    esac
    case "$path" in
      frontend/*) wanted="$wanted ux-cognitive ux-interaction ux-perception ux-memory ux-complexity " ;;
    esac
    case "$path" in
      .github/* | justfile | *.just | lefthook.yml | deploy/* | .claude/hooks/* | .claude/settings.json)
        wanted="$wanted ci-cd "
        ;;
    esac
    case "$path" in
      Cargo.toml | */Cargo.toml | Cargo.lock | */Cargo.lock | package.json | */package.json | \
        pnpm-lock.yaml | */pnpm-lock.yaml | */package-lock.json | deny.toml | */deny.toml | \
        */rust-toolchain.toml | .github/dependabot.yml)
        wanted="$wanted dependencies "
        ;;
    esac
    case "$path" in
      docs/* | *.md | README* | */README* | contracts/openapi.json) wanted="$wanted docs " ;;
    esac
  done
  for name in $ALL_REVIEWERS; do
    case "$wanted" in *" $name "*) printf '%s\n' "$name" ;; esac
  done
}

HOOK_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib/common.sh
. "$HOOK_DIR/lib/common.sh"

# ---- 1. Recursion guard -------------------------------------------------------------------
[ "${CLAUDE_REVIEW_CHILD:-}" = 1 ] && exit 0

if [ "${1:-}" = "--manual" ]; then
  input="{\"cwd\":$(json_string "$PWD"),\"stop_hook_active\":false}"
else
  input=$(cat)
fi
active=$(json_get "$input" stop_hook_active)
root=$(repo_root "$(json_get "$input" cwd)") || exit 0
cd "$root"
state=$(state_dir "$root")
run_dir="$state/stop-gate"

# ---- 2. Fingerprint -----------------------------------------------------------------------

# The tree id git would commit for the whole work tree, untracked files included.
tree_fingerprint() {
  local index git_index
  index=$(mktemp "$state/fingerprint-index.XXXXXX")
  git_index=$(git rev-parse --git-path index)
  if [ -f "$git_index" ]; then cp "$git_index" "$index"; else rm -f "$index"; fi
  GIT_INDEX_FILE="$index" git add -A -- . ':(exclude).claude/state' ':(exclude)graphify-out' \
    >/dev/null 2>&1 || true
  GIT_INDEX_FILE="$index" git write-tree
  rm -f "$index"
}

base=$(git merge-base HEAD "$BASE_REF" 2>/dev/null || git rev-parse -q --verify HEAD 2>/dev/null || true)
[ -n "$base" ] || exit 0
tree=$(tree_fingerprint)
[ "$tree" != "$(git rev-parse "$base^{tree}")" ] || exit 0
fingerprint="$tree reviewers=$REVIEWERS_MODE"
last_green=$(cat "$state/last-green" 2>/dev/null || true)
[ "$last_green" != "$fingerprint" ] || exit 0
[ "$REVIEWERS_MODE" = off ] && [ "$last_green" = "$tree reviewers=on" ] && exit 0

# ---- Outcomes -----------------------------------------------------------------------------

failure_field() { # key : value from the last failure record, or nothing
  local line
  [ -f "$state/last-failure" ] || return 0
  while IFS= read -r line; do
    case "$line" in "$1="*)
      printf '%s\n' "${line#*=}"
      return 0
      ;;
    esac
  done <"$state/last-failure"
}

ask_human() { # why : stop blocking, tell the human
  local head
  head=$(head -n 20 "$state/last-failure.txt" 2>/dev/null || true)
  printf '{"systemMessage":%s}\n' "$(json_string "Stop gate: still failing, and Claude can't get past it ($1). A human needs to decide: fix it, or rerun with VAK_REVIEWERS=off if only a reviewer is wrong. Full report: .claude/state/last-failure.txt"$'\n\n'"$head")"
  exit 0
}

fail_gate() { # report signature
  local signature previous repeats=1
  signature=$(printf '%s' "$2" | git hash-object --stdin)
  if [ "$active" = true ] && [ "$signature" = "$(failure_field signature)" ]; then
    previous=$(failure_field repeats)
    repeats=$((${previous:-1} + 1))
  fi
  printf 'tree=%s\nsignature=%s\nrepeats=%s\n' "$fingerprint" "$signature" "$repeats" >"$state/last-failure"
  printf '%s\n' "$1" >"$state/last-failure.txt"
  if [ "$active" = true ] && [ "$repeats" -ge "$MAX_REPEATS" ]; then
    ask_human "the same failure came back $repeats times in a row"
  fi
  printf '%s\n' "$1" >&2
  exit 2
}

pass_gate() { # message
  printf '%s\n' "$fingerprint" >"$state/last-green"
  rm -f "$state/last-failure" "$state/last-failure.txt"
  printf '{"systemMessage":%s}\n' "$(json_string "$1")"
  exit 0
}

# ---- 7a. Loop breaker: nothing changed since the last failure -----------------------------
if [ -f "$state/last-failure" ] && [ "$(failure_field tree)" = "$fingerprint" ]; then
  [ "$active" = true ] && ask_human "the work tree has not changed since the last failure"
  {
    echo "Stop gate: still failing. The work tree hasn't changed since the last run:"
    echo
    cat "$state/last-failure.txt"
  } >&2
  exit 2
fi

# ---- 3. Verify ----------------------------------------------------------------------------
rm -rf "$run_dir"
mkdir -p "$run_dir"
read -r -a verify_cmd <<<"$VERIFY_CMD"
verify_rc=0
run_with_timeout "$((DEADLINE_S - 60 - SECONDS))" "${verify_cmd[@]}" >"$run_dir/verify.log" 2>&1 </dev/null \
  || verify_rc=$?
if [ "$verify_rc" -ne 0 ]; then
  tail_text=$(tail -n 60 "$run_dir/verify.log")
  [ "$verify_rc" -ne 124 ] || tail_text="$tail_text
(stopped: verify ran past the gate's ${DEADLINE_S}s deadline)"
  fail_gate "Stop gate: \`$VERIFY_CMD\` failed (exit $verify_rc). Fix it, then finish again.
Last lines of the output (full log: .claude/state/stop-gate/verify.log):

$tail_text" "verify $(printf '%s' "$tail_text" | tr -d '0-9')"
fi

# ---- 4. Reviewer selection ----------------------------------------------------------------
if [ "$REVIEWERS_MODE" = off ]; then
  pass_gate "Stop gate: \`$VERIFY_CMD\` passed. Reviewer agents skipped (VAK_REVIEWERS=off)."
fi
changed=$({
  git diff --name-only --no-renames "$base" -- 2>/dev/null || true
  git ls-files --others --exclude-standard 2>/dev/null || true
} | sort -u)
selected=$(printf '%s\n' "$changed" | select_reviewers)
printf '%s\n' "$selected" >"$run_dir/reviewers"
if [ -z "$selected" ]; then
  pass_gate "Stop gate passed: \`$VERIFY_CMD\` is green; no reviewer covers the changed paths."
fi

# ---- 5. Reviewers -------------------------------------------------------------------------
graphify=$(find_graphify || true)
if [ -n "$graphify" ]; then
  for verb in query affected explain path; do
    ALLOWED_TOOLS="$ALLOWED_TOOLS,Bash($graphify $verb *)"
  done
fi

write_prompts() {
  local diff="$run_dir/diff.patch" bytes file name
  git diff --no-color --no-ext-diff "$base" -- >"$diff" 2>/dev/null || true
  while IFS= read -r file; do
    [ -n "$file" ] || continue
    git diff --no-color --no-index -- /dev/null "$file" >>"$diff" 2>/dev/null || true
  done <<<"$(git ls-files --others --exclude-standard 2>/dev/null || true)"
  bytes=$(wc -c <"$diff" | tr -d ' ')
  {
    echo "Gate review requested by the Stop hook (.claude/hooks/stop-gate.sh)."
    echo
    echo "Base: $base, the merge-base of HEAD and $BASE_REF. The change under review is"
    echo "\`git diff $base\` (committed and uncommitted work) plus the untracked files."
    echo
    echo "Changed paths:"
    printf '%s\n' "$changed" | sed 's/^/- /'
    echo
    if [ -n "$graphify" ] && [ -f "$root/graphify-out/graph.json" ]; then
      echo "Code graph: graphify-out/graph.json. For context, run \`$graphify query \"<question>\"\`,"
      echo "\`$graphify affected \"<file or symbol>\"\` or \`$graphify explain \"<symbol>\"\`."
    else
      echo "Code graph: not built in this work tree; use git, Read and Grep."
    fi
    echo
    if [ "$bytes" -gt "$DIFF_MAX_BYTES" ]; then
      echo "Diff, untracked files included (first $DIFF_MAX_BYTES of $bytes bytes; read the files for the rest):"
    else
      echo "Diff, untracked files included:"
    fi
    echo '```diff'
    head -c "$DIFF_MAX_BYTES" "$diff"
    echo
    echo '```'
    echo
    echo "Review only your area, with your checklist. Read the CLAUDE.md of every folder that"
    echo "holds a changed file. Return only the verdict JSON."
  } >"$run_dir/prompt.txt"
  for name in $selected; do
    cp "$run_dir/prompt.txt" "$run_dir/prompt-$name.txt"
    case "$name" in
      ux-*)
        {
          echo
          echo "UX evidence: the Playwright and auditScreen() output of this run is in"
          echo ".claude/state/stop-gate/verify.log. Artifacts, if any, are under frontend/test-results/"
          echo "and frontend/playwright-report/. auditScreen() is in frontend/e2e/support/ux-audit.ts."
        } >>"$run_dir/prompt-$name.txt"
        ;;
    esac
  done
}

agent_model() { # name : the model in the agent file's front matter
  local file
  for file in "$root/.claude/agents/$1.md" "$HOOK_DIR/../agents/$1.md"; do
    if [ -f "$file" ]; then
      awk '/^---[[:space:]]*$/ { n++; next } n == 1 && /^model:/ { sub(/^model:[[:space:]]*/, ""); print; exit }' "$file"
      return 0
    fi
  done
  return 0
}

run_reviewer() { # name seconds : always runs in a background subshell
  local name=$1 seconds=$2 model rc=0
  local -a model_args=()
  export CLAUDE_REVIEW_CHILD=1
  model=$(agent_model "$name")
  if [ -n "$model" ] && [ "$model" != inherit ]; then model_args=(--model "$model"); fi
  run_with_timeout "$seconds" claude -p \
    --agent "$name" ${model_args[@]+"${model_args[@]}"} \
    --output-format json --json-schema "$schema" \
    --max-turns "$MAX_TURNS" --max-budget-usd "$MAX_BUDGET_USD" \
    --permission-mode dontAsk --permission-prompts none \
    --allowedTools "$ALLOWED_TOOLS" --disallowedTools "$DISALLOWED_TOOLS" \
    --no-session-persistence \
    <"$run_dir/prompt-$name.txt" >"$run_dir/$name.json" 2>"$run_dir/$name.err" || rc=$?
  printf '%s\n' "$rc" >"$run_dir/$name.rc"
}

schema=$(json_compact "$HOOK_DIR/../schemas/review-verdict.json")
write_prompts
for name in $selected; do
  while [ "$(($(jobs -rp | wc -l)))" -ge "$CONCURRENCY" ]; do sleep 0.2; done
  seconds=$((DEADLINE_S - 30 - SECONDS))
  [ "$seconds" -le "$REVIEW_TIMEOUT_S" ] || seconds=$REVIEW_TIMEOUT_S
  if [ "$seconds" -lt 60 ]; then
    printf '124\n' >"$run_dir/$name.rc" # no time left: counts as a timeout
    continue
  fi
  run_reviewer "$name" "$seconds" &
done
wait

# ---- 6. Verdicts --------------------------------------------------------------------------

# verdict_summary FILE: line 1 "status US cost US note" (status pass|fail|invalid), then one
# "severity US file US line US message" line per finding. US is the ASCII unit separator,
# which, unlike a tab, keeps empty fields when read splits on it.
US=$'\x1f'
# shellcheck disable=SC2016 # a jq program, not shell
JQ_VERDICT='
def clean: tostring | gsub("[\u001f\t\r\n]+"; " ");
(if length == 0 then null else .[-1] end) as $r
| if ($r | type) != "object" then "invalid\u001f0\u001foutput is not a JSON object"
  else (($r.total_cost_usd // 0) | tostring) as $cost
  | $r.structured_output as $v
  | if ($r.is_error // false) == true then "invalid\u001f\($cost)\u001fthe run ended with an error (\($r.subtype // "unknown"))"
    elif ($v | type) != "object" then "invalid\u001f\($cost)\u001fno verdict in the output (\($r.subtype // "unknown"))"
    elif ($v.verdict != "pass" and $v.verdict != "fail") then "invalid\u001f\($cost)\u001fverdict is neither pass nor fail"
    elif ($v.findings | type) != "array" then "invalid\u001f\($cost)\u001ffindings is not a list"
    else ($v.findings | map(select(type == "object"))) as $f
    | (if $v.verdict == "fail" or any($f[]; .severity == "blocker" or .severity == "major")
       then "fail" else "pass" end) as $status
    | "\($status)\u001f\($cost)\u001f",
      ($f[] | [(.severity // "major"), (.file // "?"), (.line // ""), (.message // "")]
            | map(clean) | join("\u001f"))
    end
  end'

PY_VERDICT='
import json, sys

US = "\x1f"

def clean(value):
    return " ".join(str(value).replace(US, " ").split())

def load(text):
    try:
        return json.loads(text)
    except ValueError:
        for line in reversed(text.splitlines()):
            try:
                return json.loads(line)
            except ValueError:
                continue
    return None

def row(*fields):
    print(US.join(str(x) for x in fields))

r = load(open(sys.argv[1], encoding="utf-8", errors="replace").read())
if not isinstance(r, dict):
    row("invalid", 0, "output is not a JSON object")
    sys.exit()
cost = r.get("total_cost_usd") or 0
v = r.get("structured_output")
subtype = r.get("subtype") or "unknown"
if r.get("is_error") is True:
    row("invalid", cost, "the run ended with an error (%s)" % subtype)
elif not isinstance(v, dict):
    row("invalid", cost, "no verdict in the output (%s)" % subtype)
elif v.get("verdict") not in ("pass", "fail"):
    row("invalid", cost, "verdict is neither pass nor fail")
elif not isinstance(v.get("findings"), list):
    row("invalid", cost, "findings is not a list")
else:
    findings = [f for f in v["findings"] if isinstance(f, dict)]
    failed = v["verdict"] == "fail" or any(f.get("severity") in ("blocker", "major") for f in findings)
    row("fail" if failed else "pass", cost, "")
    for f in findings:
        line = f.get("line")
        row(*(clean(x) for x in (f.get("severity") or "major", f.get("file") or "?",
                                 "" if line is None else line, f.get("message") or "")))
'

verdict_summary() {
  local out=""
  if [ -s "$1" ]; then
    if [ "$JSON_TOOL" = jq ]; then
      out=$(jq -r -s "$JQ_VERDICT" "$1" 2>/dev/null || true)
    else
      out=$(python3 -I -c "$PY_VERDICT" "$1" 2>/dev/null || true)
    fi
  fi
  printf '%s\n' "${out:-invalid${US}0${US}output is not JSON}"
}

report=""
signature=""
passed=""
total_cost=0
for name in $selected; do
  rc=$(cat "$run_dir/$name.rc" 2>/dev/null || echo 1)
  summary=$(verdict_summary "$run_dir/$name.json")
  head_line=${summary%%$'\n'*}
  status=${head_line%%"$US"*}
  rest=${head_line#*"$US"}
  cost=${rest%%"$US"*}
  note=${rest#*"$US"}
  total_cost=$(awk -v a="$total_cost" -v b="$cost" 'BEGIN { printf "%.2f", a + b }' 2>/dev/null || echo "$total_cost")
  if [ "$rc" != 0 ]; then
    status=invalid
    if [ "$rc" = 124 ]; then
      note="ran out of time (${REVIEW_TIMEOUT_S}s per reviewer, ${DEADLINE_S}s per gate run)"
    else
      note="claude exited $rc${note:+ ($note)}: $(tail -n 3 "$run_dir/$name.err" 2>/dev/null | tr '\n' ' ')"
    fi
  fi
  case "$status" in
    pass)
      passed="$passed${passed:+, }$name"
      continue
      ;;
    fail) report="${report}[$name] FAIL"$'\n' ;;
    *)
      report="${report}[$name] NO VERDICT: $note. A missing verdict counts as a fail."$'\n'
      signature="$signature$name|invalid"$'\n'
      continue
      ;;
  esac
  while IFS="$US" read -r severity file line message; do
    [ -n "$severity" ] || continue
    report="$report  - $severity $file${line:+:$line}  $message"$'\n'
    case "$severity" in
      blocker | major) signature="$signature$name|$severity|$file"$'\n' ;;
    esac
  done <<<"$(printf '%s\n' "$summary" | tail -n +2)"
done

if [ -n "$report" ]; then
  fail_gate "Stop gate: reviewer agents found problems. Fix the blocker and major findings, then finish again.

$report
Passed: ${passed:-none}
Logs: .claude/state/stop-gate/ (reviewer output in <name>.json)" "reviewers
$signature"
fi
pass_gate "Stop gate passed: \`$VERIFY_CMD\` and reviewers $passed (estimated cost \$$total_cost)."
