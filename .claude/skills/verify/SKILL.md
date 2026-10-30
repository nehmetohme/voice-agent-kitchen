---
name: verify
description: Run the repository's verify gate on demand and explain any failure. Use when the user asks to verify, check or validate the work, or before you say a task is done. `/verify` runs `just verify`, `/verify fast` runs `just verify-fast`, and `/verify gate` runs the full Stop gate (verify plus the reviewer agents).
argument-hint: "[fast|gate]"
allowed-tools: Bash(just verify) Bash(just verify-fast) Bash(just backend-verify) Bash(just frontend-verify) Bash(just repo-verify) Bash(.claude/hooks/stop-gate.sh --manual)
---

# /verify

Run the gate from the root of the current worktree. Choose the command from the argument `$0`:

| Argument | Command | What it runs |
|---|---|---|
| none | `just verify` | Every gate: format, lint, types, architecture, tests, coverage, security, contracts |
| `fast` | `just verify-fast` | The quick subset that also runs on commit |
| `gate` | `.claude/hooks/stop-gate.sh --manual` | The Stop gate: `just verify`, then the reviewer agents for the changed paths |

`gate` starts one headless Claude session per selected reviewer, and each one costs money (capped at $2 each by default). Run it only when the user asks for it. The Stop hook runs the same gate on its own when you finish.

## Then report

- **Green:** say which command passed. For `gate`, list the reviewers that passed. It prints `{"systemMessage": ...}` and exits 0.
- **Red:** name the failing check and, for each problem, the file and line, what is wrong and the fix. Read `.claude/state/stop-gate/verify.log` or `<reviewer>.json` there for the full output. `gate` exits 2 and prints the findings on stderr.
- Fix the code, not the gate. Never edit contract-tagged tests, generated files, `.claude/` gate config or `backend/layers.toml` to get green, and never set `VAK_REVIEWERS`. That switch is for humans only.
- If a check can't pass for reasons outside the task, such as a missing tool or a broken main, say so and ask the user. Don't work around it.
