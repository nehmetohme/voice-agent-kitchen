# 05. Claude Code tooling for quality gates

Status: research, 2026-10-30. Claims come from the official Claude Code docs at code.claude.com, fetched on this date. Inferences and unverified points are marked.

## TL;DR

- Enforce gates with hooks. CLAUDE.md is context the model reads, not an enforcement layer (memory page).
- Per-edit: a `PostToolUse` command hook on `Edit|Write` formats and lints the file. Graph regeneration is a second `async` hook on the same matcher, since async hooks cannot block.
- Done: a `Stop` command hook. It fires after every response, so it first compares a tree fingerprint with the last green run. If the tree changed, it runs `verify` and the relevant reviewers, then exits 2 with findings on stderr so Claude keeps working.
- Hooks cannot call subagents directly. The reliable pattern is a command hook that runs `claude -p --agent <reviewer> --json-schema ...` per relevant reviewer. Agent hooks are experimental, with a 60 s default timeout.
- Worktrees are built in: `claude -w <name>`, plus the `EnterWorktree` tool. Scripts must read `cwd` from hook input, because `${CLAUDE_PROJECT_DIR}` does not follow a worktree session.
- Nested CLAUDE.md files load on demand, when Claude reads or edits files in that folder, not at launch. For always-on layer rules, use `.claude/rules/*.md` with `paths:`. Keep each CLAUDE.md under about 200 lines.

## Recommended gate architecture

| When | Event (matcher) | Handler | Action | Blocks? |
|---|---|---|---|---|
| After each edit | `PostToolUse` (`Edit\|Write`) | command, sync, timeout ~60 s | Format file; fast lint. On failure print `{"decision":"block","reason":...}` | Feeds back to Claude (tool already ran) |
| After each edit | `PostToolUse` (`Edit\|Write`) | command, `async: true` | Graphify update, debounced by a lock file | No |
| Task done | `Stop` (no matcher) | command, timeout 900-1200 s | If tree fingerprint differs from last green: `just verify`, then reviewers in parallel. Exit 2 on any failure | Yes |
| Task list done (optional) | `TaskCompleted` | command | Verify scoped to the task; exit 2 keeps it open | Yes |
| Outside Claude | Git `pre-commit` / `pre-push` (plain git, not Claude hooks) | shell | Fast subset on commit, full `just verify` on push; `core.hooksPath=.githooks` | Yes |
| Merge | Merge queue + required CI checks | CI | `just verify` plus GitHub gates | Yes |

Claude hooks do not run for a `git commit` typed in a plain terminal, so git hooks and CI are the backstop.

## Example config snippets

### `.claude/settings.json` hooks block

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "\"${CLAUDE_PROJECT_DIR}\"/.claude/hooks/post-edit-lint.sh",
            "timeout": 60,
            "statusMessage": "Formatting and linting"
          }
        ]
      },
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "\"${CLAUDE_PROJECT_DIR}\"/.claude/hooks/graph-update.sh",
            "async": true
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"${CLAUDE_PROJECT_DIR}\"/.claude/hooks/stop-gate.sh",
            "timeout": 1200,
            "statusMessage": "Running verify and reviewers"
          }
        ]
      }
    ]
  }
}
```

### `post-edit-lint.sh` (sketch)

```bash
#!/usr/bin/env bash
input=$(cat)
file=$(jq -r '.tool_input.file_path // empty' <<<"$input")
cwd=$(jq -r '.cwd' <<<"$input")          # worktree root when running in a worktree
[ -n "$file" ] && cd "$cwd" || exit 0

case "$file" in
  *.rs) rustfmt --edition 2021 "$file" ;;
esac

if ! out=$(/* fast lint for $file */ 2>&1); then
  jq -n --arg r "$out" '{decision: "block", reason: ("Lint failed:\n" + $r)}'
fi
exit 0
```

### `stop-gate.sh` (outline)

1. If `CLAUDE_REVIEW_CHILD=1` is set, exit 0 (recursion guard, see Q2).
2. Read `cwd` and `stop_hook_active` from stdin. Compute a tree fingerprint (tracked diff plus untracked files). If it equals `.claude/state/last-green`, exit 0.
3. Run `just verify`. On failure, print the failing check to stderr and exit 2.
4. Select reviewers from `git diff --name-only HEAD` using the path map below. Run each as a headless child in parallel and collect its verdict JSON.
5. Any `fail`, partial result, or unparseable verdict: print findings to stderr, exit 2. Otherwise write the fingerprint to `last-green` and exit 0.
6. If `stop_hook_active` is `true` and the failure fingerprint is unchanged since the last attempt, exit 0 with a `systemMessage` for a human. This stops a loop on an unfixable failure.

### `.claude/agents/security-reviewer.md`

```markdown
---
name: security-reviewer
description: Read-only security gate. Reviews the uncommitted change for authn/authz gaps, unvalidated input, secrets, injection, and PII in logs. Use before a task is marked done.
tools: Read, Grep, Glob, Bash
disallowedTools: Edit, Write, Agent
model: sonnet
maxTurns: 25
effort: high
color: red
---

You are the security gate for this repository. You never edit files.

The change under review is `git diff HEAD` plus untracked files. Read the CLAUDE.md
of each folder you open; it defines the layer rules.

Check: authentication and authorization on every new route, input validation at the
router layer, secrets in code or config, SQL and command injection, PII in log
statements, and new dependencies with known CVEs.

Return only the JSON object the caller requested:
- "verdict": "pass" only if there is no blocker or major finding.
- "findings": each with "severity" (blocker|major|minor), "file", "line", "message",
  and a one-sentence fix. Cite file and line. Do not report speculation.
```

### `.claude/schemas/review-verdict.json`

```json
{
  "type": "object",
  "required": ["verdict", "findings"],
  "properties": {
    "verdict": { "enum": ["pass", "fail"] },
    "findings": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["severity", "file", "message"],
        "properties": {
          "severity": { "enum": ["blocker", "major", "minor"] },
          "file": { "type": "string" },
          "line": { "type": "integer" },
          "message": { "type": "string" }
        }
      }
    }
  }
}
```

### Reviewer selection map (in `stop-gate.sh`; a design proposal, not a Claude Code feature)

| Changed paths | Reviewers |
|---|---|
| Any Rust or TS source | security, code-quality, hygiene, separation, testing |
| Voice pipeline crate(s) | + performance |
| `frontend/**` | + the five UX reviewers |
| `.github/**` | + ci-cd |
| `Cargo.toml`, `Cargo.lock`, `package*.json`, `deny.toml` | + dependencies |
| `docs/**`, any `CLAUDE.md`, `README*` | + docs |

The script decides the list from `git diff --name-only`, so the model cannot skip a reviewer.

## Q1. Hooks

Sources: https://code.claude.com/docs/en/hooks.md and https://code.claude.com/docs/en/hooks-guide.md

**Events (33 in the reference table):**
- Session and config: `SessionStart`, `Setup`, `SessionEnd`, `InstructionsLoaded`, `ConfigChange`, `CwdChanged`, `DirectoryAdded`, `FileChanged`
- Prompt: `UserPromptSubmit`, `UserPromptExpansion`
- Tools: `PreToolUse`, `PermissionRequest`, `PermissionDenied`, `PostToolUse`, `PostToolUseFailure`, `PostToolBatch`
- Subagents and tasks: `SubagentStart`, `SubagentStop`, `TaskCreated`, `TaskCompleted`, `TeammateIdle`
- Turn end: `Stop`, `StopFailure`
- Other: `Notification`, `MessageDisplay`, `Elicitation`, `ElicitationResult`, `PreCompact`, `PostCompact`, `PreModelSwitch`, `PostModelSwitch`, `WorktreeCreate`, `WorktreeRemove`

Blocking events in the table: `PreToolUse`, `UserPromptSubmit`, `UserPromptExpansion`, `PostToolBatch`, `SubagentStop`, `TaskCreated`, `TaskCompleted`, `Stop`, `TeammateIdle`, `ConfigChange`, `WorktreeCreate`, `WorktreeRemove`, `PreCompact`, `PreModelSwitch`, `Elicitation`, `ElicitationResult`. `PermissionRequest` ignores exit 2 and needs its decision object. The table lists `PostToolUse` as non-blocking, yet its section documents `decision: "block"` as feedback next to the result. Verify in your build.

**Handler types:** `command`, `http`, `mcp_tool`, `prompt`, `agent`. `prompt` and `agent` are LLM-evaluated. `agent` is experimental. `PostToolUse`, `PreToolUse`, `Stop`, `SubagentStop`, `TaskCompleted`, `TaskCreated`, `TeammateIdle`, `UserPromptSubmit`, `UserPromptExpansion`, `PostToolBatch`, and `PermissionDenied` accept all five. `SessionStart` and `Setup` accept only `command` and `mcp_tool`.

- Common: `type`, `if`, `timeout`, `statusMessage`, `once` (skill frontmatter only).
- `if` takes permission-rule syntax (`"Bash(git *)"`, `"Edit(*.ts)"`) and runs only on tool events.
- `command`: `command` (required), `args` (exec form), `async`, `asyncRewake`, `shell` (`bash` or `powershell`), `onFailure` (`continue` or `block`, v2.1.295+).
- `http`: `url`, `headers`, `allowedEnvVars`, `onFailure`. The body is the JSON input.
- `mcp_tool`: `server`, `tool`, `input`.
- `prompt` / `agent`: `prompt` (`$ARGUMENTS` inserts the input JSON), `model`.

**Input JSON.** Common: `session_id`, `prompt_id`, `transcript_path`, `cwd`, `scratchpad_dir`, `permission_mode`, `effort`, `hook_event_name`; subagents add `agent_id`, `agent_type`. Tool events add `tool_name`, `tool_input`, `tool_use_id`. `PostToolUse` adds `tool_response` and `duration_ms`. For `Write`, `Edit`, `Read`, `tool_input.file_path` is absolute. `Stop` adds `stop_hook_active`, `last_assistant_message`, `background_tasks`, `session_crons`. `SubagentStop` adds `stop_hook_active`, `agent_id`, `agent_type`, `agent_transcript_path`, `last_assistant_message`. `TaskCompleted` adds `task_id`, `task_subject`, optional `task_description`, `teammate_name`.

**Output.** Universal fields: `continue` (false stops Claude), `stopReason`, `suppressOutput`, `systemMessage`.
- Top-level `decision: "block"` with `reason`. Omit `decision` to allow.
- `hookSpecificOutput` (requires `hookEventName`): `PreToolUse` takes `permissionDecision` (`allow`/`deny`/`ask`/`defer`), `permissionDecisionReason`, `updatedInput` (replaces the whole input), `additionalContext`. Multiple hooks resolve deny > defer > ask > allow. `PostToolUse` takes `updatedToolOutput` and `additionalContext`. `Stop` takes `additionalContext`, which continues the turn but is labeled as hook feedback rather than an error.

**Exit codes.** `0`: success; stdout is parsed as JSON only if it is a single object, and plain text reaches context only on `UserPromptSubmit`, `UserPromptExpansion`, `SessionStart`, `PostModelSwitch`. `2`: blocking error; stderr is the reason. JSON cannot override a 2. Any other code: non-blocking, unless `onFailure: "block"`.

**Matchers.**
- `"*"`, empty, or omitted: all.
- Letters, digits, `_`, `-`, space, `,`, `|` only: exact name or list (`Edit|Write` = `Edit, Write`).
- Anything else: unanchored JavaScript regex. Use `^...$` for whole-string match.
- MCP tools are `mcp__<server>__<tool>`.
- Events without matcher support always fire: `Stop`, `SubagentStop`, `UserPromptSubmit`, `PostToolBatch`, `TaskCompleted`, `TaskCreated`, `TeammateIdle`, `CwdChanged`, `WorktreeCreate`, `WorktreeRemove`, `MessageDisplay`.

**Timeouts.** `command`, `http`, `mcp_tool`: 600 s default (30 s on `UserPromptSubmit`, `PreModelSwitch`, `PostModelSwitch`; 10 s on `MessageDisplay`). `prompt`: 30 s. `agent`: 60 s, up to 50 turns. Set `timeout` on every gate.

**Async.** `"async": true` (command only) runs in the background; Claude continues. `decision`, `permissionDecision`, and `continue` have no effect. Output arrives on the next turn, or waits for user input if idle. `timeout` is not enforced on async hooks. `asyncRewake`: exit 2 wakes Claude immediately, even when idle. Use it for background tests that should interrupt on failure. In `-p` mode, async hooks still running at teardown are killed as `cancelled`.

**Stopping loops.**
- `Stop` fires whenever Claude finishes responding. It does not fire on user interrupts (`StopFailure` covers API errors).
- `stop_hook_active` is `true` when Claude is already continuing because a stop hook blocked. Check it, or the transcript, so you do not block on a condition that can never resolve.
- A built-in cap applies: after eight consecutive stop-hook continuations, the next block is overridden and the turn ends. The counter resets when Claude calls a tool. Raise it with `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`.
- Prompt hooks can return `"impossible": true` with `ok: false` to end the turn when the condition can never be met.

**Parallelism.** Matching hooks for one event run in parallel. A deny from any hook beats an allow from another.

## Q2. Running a reviewer subagent from a hook

| Option | How | Cost | Latency | Reliability | Use for |
|---|---|---|---|---|---|
| A. Command hook runs `claude -p --agent <reviewer>` | Stop script calls headless Claude per reviewer, parses JSON, exits 2 on fail | One full extra session per reviewer, billed separately; fresh context | About the slowest reviewer if parallel | High: deterministic exit code and validated JSON | Recommended for the reviewer fleet |
| B. Stop hook asks the main agent to call the Agent tool | `decision: "block"` with reason "run security-reviewer on these files" | Cheapest setup; verdicts land in main context | Serial, within the main turn | Lower: model may skip or summarize; eight-continuation cap | One or two reviewers |
| C. Agent hook (`type: "agent"`) | One verifier with Read, Grep, Glob, up to 50 turns, returns `{ok, reason}` | One agent run per hook | 60 s default | Experimental; no per-reviewer agent file | A single small check |
| D. Prompt hook | One tool-less LLM call returning `{ok, reason}` | Cheapest | 30 s default | Sees only hook input, no file reads | Diff-only judgments |

**Recommendation: Option A** for the Stop gate, with these safeguards:
- **Recursion guard.** The child `claude -p` runs its own hooks, including Stop. Export `CLAUDE_REVIEW_CHILD=1` and make every hook script exit 0 when it is set. `--bare` also skips hooks, but it also skips CLAUDE.md, which reviewers need.
- **Budgets.** Pass `--max-turns` and `--max-budget-usd` per reviewer. Subagent spend counts toward the cap. A turn-limit partial result is a fail.
- **Structure.** Pass `--json-schema` (CLI reference) so the verdict is validated, then parse it in the script.
- **Context.** Put the diff and layer pointers in the prompt or in `--append-system-prompt`. Do not make each reviewer rediscover the diff.
- **Concurrency.** Fourteen parallel sessions can hit rate limits. Run only the selected subset, and cap concurrency in the script.
- **No isolation for reviewers.** `isolation: worktree` branches from the default branch, so the reviewer would not see your change.

**Relevance.** Select by path in the script (map above). The `if` field filters tool hooks by tool and argument, but it does not decide which reviewer processes start. Skills can use `paths` to limit auto-loading, which suits frontend-only UX checklists. Verify glob behavior in your build.

## Q3. Custom subagents in `.claude/agents/*.md`

Source: https://code.claude.com/docs/en/sub-agents.md

Frontmatter (case-sensitive camelCase; unknown keys are silently ignored, so run `claude --debug` and search for `does not recognize or apply`):

| Field | Required | Notes |
|---|---|---|
| `name` | yes | Unique, up to 256 chars, no leading `-`, no `:` |
| `description` | yes | Tells Claude when to delegate |
| `tools` | no | Omitted inherits all tools |
| `disallowedTools` | no | Removes tools from the set; `Bash(git push *)` removes Bash entirely |
| `model` | no | `sonnet`, `opus`, `haiku`, `fable`, a full ID, or `inherit` |
| `permissionMode` | no | `default`, `acceptEdits`, `auto`, `dontAsk`, `bypassPermissions`, `plan`, `manual`. Ignored for plugin agents |
| `maxTurns` | no | On the limit, output is returned as partial and can be resumed |
| `skills` | no | Preloads up to 32 skills in full |
| `mcpServers` | no | Names or inline definitions |
| `hooks` | no | Scoped to this agent |
| `memory` | no | `user`, `project`, `local` |
| `background` | no | `true` keeps it in the background |
| `omitClaudeMd` | no | `true` skips CLAUDE.md at startup (v2.1.271+) |
| `effort` | no | `low`, `medium`, `high`, `xhigh`, `max` |
| `isolation` | no | Only `worktree` |
| `color` | no | Display color |
| `initialPrompt` | no | First turn when used as the session agent |
| `autoCompactWindow`, `experimental` | no | Newer options; see the page |

Invocation: automatic delegation by `description`, a natural-language request, `@agent-<name>`, or the whole session via `claude --agent <name>` or the `agent` setting.

Scope precedence: managed, then `--agents` flag, then `.claude/agents/` (project), then `~/.claude/agents/` (user), then plugin `agents/`.

Nesting: subagents can spawn subagents, up to three levels by default (`CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH` changes it). Each reviewer should set `disallowedTools: Agent`.

Structured output: the sub-agents page documents no schema field. Enforce the verdict in the caller with `--json-schema`.

Best practices for a verdict reviewer:
- Read-only: restrict `tools`, add `disallowedTools: Edit, Write, Agent`, and scope Bash with permission rules.
- Fixed output contract in the body (one JSON object), validated by the caller.
- A specific `description` so automatic delegation does not fire on unrelated work.
- Set `maxTurns` and treat partial output as `fail`.
- Keep layer rules in CLAUDE.md or `.claude/rules/`, not in each agent body, so reviewers share one source.

## Q4. Worktrees

Sources: https://code.claude.com/docs/en/worktrees.md and the CLI reference.

- **Start:** `claude --worktree <name>` (`-w`). Creates `<repo>/.claude/worktrees/<name>/` on branch `worktree-<name>`. Omit the name for a generated one. `claude -w "#1234"` or a PR/MR URL branches from that change.
- **Mid-session:** `EnterWorktree` creates or switches (`path` form for existing). `ExitWorktree` leaves. Entering a path outside `.claude/worktrees/` needs approval. Subagents with `isolation: worktree` get only the `path` form.
- **Base branch:** `worktree.baseRef` is `"fresh"` (default, origin's default branch) or `"head"` (current local HEAD). Use `"head"` if implementers must start from unpushed work.
- **Setup:** a worktree is a fresh checkout, so install dependencies. Put gitignored secrets such as `.env` in `.worktreeinclude` (gitignore syntax; only gitignored matches are copied).
- Add `.claude/worktrees/` to `.gitignore`.
- **Isolation enforcement:** in a worktree session, Claude Code blocks Edit, Write, and NotebookEdit into the main checkout, shell commands whose working directory is the main checkout, and git commands that redirect into it.
- **Hooks:** `${CLAUDE_PROJECT_DIR}` keeps pointing at the original root after entering a worktree. Input `cwd` is the worktree root and moves with `cd`. Read `cwd`.
- **Cleanup:** on exit, a clean worktree is removed with its branch (named sessions prompt first); a worktree with changes prompts keep or remove. `-p` runs do not clean up. A periodic sweep removes subagent and background worktrees older than `cleanupPeriodDays` that hold no work.
- **Shared:** `.git`, project-scope plugins, and "don't ask again" approvals (written to the main checkout's `.claude/settings.local.json`). Auto memory is shared across worktrees of a repo.

**One-branch-per-task workflow** (recommendation; not a documented Claude Code feature):
1. `claude -w <task-id>` from the main checkout.
2. Write and commit contract tests first, tagged `contract/<feature>/v1` per the brief.
3. Implement. The Stop gate runs `verify` and reviewers. Commit.
4. Push the branch, open a PR, and merge only through the merge queue.
5. After merge, `git worktree remove <path>`. Run `git worktree unlock` first if locked.

## Q5. Nested CLAUDE.md files and path-scoped rules

Source: https://code.claude.com/docs/en/memory.md

**Loading.**
- CLAUDE.md and CLAUDE.local.md in ancestor folders load at launch.
- CLAUDE.md in a subdirectory loads on demand, the first time Claude reads, writes, or edits a file there. Viewing one file with `cat` or `head` through Bash counts.
- Nested files load again on demand after `/compact`. The root CLAUDE.md is re-read from disk.
- `@path` imports expand in place, up to four hops, but do not reduce context cost.
- `claudeMdExcludes` skips files by absolute-path glob.
- The `InstructionsLoaded` hook logs what loaded and why.

**Size.** Target under 200 lines per CLAUDE.md. Subdirectory files and rules have a per-file limit; over it, Claude gets only the start and is told to read the rest.

**Rules: `.claude/rules/*.md`.** Discovered recursively. Rules without `paths` load at launch with the same priority as `.claude/CLAUDE.md`. Rules with `paths` load only when Claude reads, writes, or edits a matching file.

```markdown
---
paths:
  - "backend/crates/services/**/*.rs"
  - "backend/crates/routers/**/*.rs"
---

# Services and routers

- Services may depend on repositories and domain only. Never import a router type.
- No I/O in the domain crate.
```

`paths` is the only frontmatter key Claude Code reads from a rule. Brace expansion works (`src/*.{ts,tsx}`). Invalid YAML silently drops `paths`, so check with `claude --debug`. User-level rules live in `~/.claude/rules/`.

**Recommendation for this repo:** a root CLAUDE.md for global rules, a short CLAUDE.md per layer folder for responsibilities and dependency rules, and `paths:` rule files for guidance that must apply whenever a layer's files are touched. Enforce layer boundaries with Separation and the Graphify check, not prose.

## Q6. Skills, plugins, settings, and file layout

- **Skills** (https://code.claude.com/docs/en/skills.md): `context: fork` with `agent: <name>` runs a skill in a forked subagent. `paths` limits auto-loading. `disable-model-invocation: true` makes it manual-only. Skills cannot block completion, so use one for an on-demand `/verify` and keep the hard gate in `settings.json`.
- **Plugins** (https://code.claude.com/docs/en/plugins/overview.md): bundle hooks, agents, skills, and MCP servers for reuse across repos. For one repo, committed project files are simpler. Plugin agents ignore `hooks`, `mcpServers`, and `permissionMode`.
- **Settings** (https://code.claude.com/docs/en/settings.md): precedence is managed, then `--settings`, then `.claude/settings.local.json`, then `.claude/settings.json`, then `~/.claude/settings.json`. Lists such as `permissions.allow` merge. Hook edits reload without restart.

**Decision:** hooks for anything that must happen or block; subagents for judgment; skills for on-demand procedures; CLAUDE.md and rules for guidance.

**Layout to commit:**

```
.claude/
  settings.json            # hooks, permissions (commit)
  settings.local.json      # personal (gitignore)
  agents/                  # one file per reviewer (9 engineering, 5 UX)
  hooks/                   # post-edit-lint.sh, graph-update.sh, stop-gate.sh
  rules/                   # backend-layers.md (paths: backend/**), frontend.md (paths: frontend/**)
  schemas/review-verdict.json
  skills/verify/SKILL.md   # optional on-demand wrapper
  worktrees/               # gitignored
.worktreeinclude           # gitignored secrets copied into new worktrees
.githooks/                 # pre-commit, pre-push; core.hooksPath points here
CLAUDE.md                  # root; each layer folder has its own
justfile                   # verify
```

## Gotchas

- Stop fires after every response, including answers to questions. Always check the tree fingerprint first, or the gate runs constantly.
- A reviewer that hits `maxTurns` returns partial output. Treat it as fail.
- PostToolUse runs after the write, so it cannot undo the edit. To stop a bad edit before it lands, use `PreToolUse` with `permissionDecision: "deny"`.
- Async hooks cannot block, and their output arrives on the next turn. Never use them as gates.
- Agent hooks are experimental and time out at 60 s by default.
- Multiple `PreToolUse` hooks rewriting `updatedInput` are not merged. Avoid it.
- Subagents can spawn subagents by default. Set `disallowedTools: Agent` on reviewers.
- `isolation: worktree` reviewers see the default branch, not your change.
- `${CLAUDE_PROJECT_DIR}` does not follow a worktree session. Use `cwd`.
- Worktree sessions cannot write to the main checkout. Keep the main checkout clean.
- Unknown frontmatter keys fail silently. Check with `claude --debug`.
- Subfolder CLAUDE.md files do not load at launch. "Every folder has a CLAUDE.md" does not mean Claude sees all of them. Path-scoped rules are the reliable always-on mechanism.
- Plain `git commit` does not trigger Claude hooks, so git hooks and CI must run the same `just verify`.
- Graphify is outside the Claude Code docs. Its CLI and incremental-update behavior were not checked; verify before writing the post-edit hook.
- Cost and latency of fourteen reviewers per task were not measured. Measure on the first real task.
- Not verified: `--append-system-prompt-file`, the exact `--json-schema` result field, and the `if` path-glob behavior. Confirm in the headless page and your build.

## Sources

- Hooks reference: https://code.claude.com/docs/en/hooks.md
- Hooks guide: https://code.claude.com/docs/en/hooks-guide.md
- Subagents: https://code.claude.com/docs/en/sub-agents.md
- Worktrees: https://code.claude.com/docs/en/worktrees.md
- Memory, CLAUDE.md, and rules: https://code.claude.com/docs/en/memory.md
- Skills: https://code.claude.com/docs/en/skills.md
- Settings files and precedence: https://code.claude.com/docs/en/settings.md
- CLI reference: https://code.claude.com/docs/en/cli-reference.md
- Tools reference: https://code.claude.com/docs/en/tools-reference.md
- Headless mode: https://code.claude.com/docs/en/headless.md
- Plugins overview: https://code.claude.com/docs/en/plugins/overview.md
- Docs index: https://code.claude.com/docs/llms.txt
