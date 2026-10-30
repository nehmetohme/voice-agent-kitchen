# CI, GitHub gates and git hooks

The same `just` recipes run in three places: git hooks (lefthook), Claude Code hooks, and GitHub Actions. Design: [`docs/plan/quality.md`](plan/quality.md) §4, §6 and §7, and [research 06](research/06-graphify-github-ci.md).

## One-time setup per clone

```sh
just setup        # runs `lefthook install`; linked worktrees share the hooks
```

Tools on `PATH`, as standalone binaries: `just`, `lefthook`, `gitleaks`, `taplo`, `shellcheck`, `jq` (or `python3`), `actionlint`, `zizmor`, `pinact`, `gh`, and Graphify (`uv tool install graphifyy==0.9.97`).

| Hook | Runs | Blocks? |
|---|---|---|
| `pre-commit` | `gitleaks git --pre-commit --staged`, then `just verify-fast` | Yes |
| `pre-push` | `just verify` | Yes |
| `post-commit`, `post-checkout`, `post-merge`, `post-rewrite` | `tools/hooks/graph-refresh.sh` | No: it detaches and exits 0 |

To skip hooks for one command, use `LEFTHOOK=0 git commit ...`. CI runs the same gates anyway.

### Graphify refresh

`tools/hooks/graph-refresh.sh` runs `graphify update .` (code only, `GRAPHIFY_BACKEND=none`) in the checkout's own root, so it works in linked worktrees. Graphify's own `graphify hook install` skips worktrees, so we don't use it.

- **One run at a time.** Calls that arrive during a run fold into one more pass. The lock (`.claude/state/graph-update.lock`) is shared with the Claude Code post-edit hook.
- **Log:** `graphify-out/refresh.log`.
- **Graphify binary:** the script uses the first `graphify` that is the pip package `graphifyy`, because an unrelated npm package has the same command name. Set `VAK_GRAPHIFY=/path/to/graphify` to choose one.
- **Debugging:** `VAK_GRAPH_FOREGROUND=1 tools/hooks/graph-refresh.sh` runs it in the foreground.
- **Shrinking graph:** if the log says an update was refused because the graph shrank, run `graphify update . --force`.

## GitHub Actions

| Workflow | Trigger | Jobs |
|---|---|---|
| `ci.yml` | PR, `merge_group`, push to `main` | `changes` → `rust`, `web`, `contracts`, `repo`, `workflows`, `deps` → `gate` |
| `codeql.yml` | PR, `merge_group`, push to `main`, weekly | `analyze` per language present: `actions`, `rust`, `javascript-typescript` (all `build-mode: none`) |
| `scorecard.yml` | push to `main`, weekly | OpenSSF Scorecard, published |
| `secrets-deep-scan.yml` | weekly, manual | TruffleHog over the full history, verified results only |
| `nightly.yml` | nightly, manual | `canaries` ([gate self-tests](#gate-self-tests)), plus placeholders for mutation tests and benchmarks |

`ci.yml` jobs:

| Job | Runs | Skips when |
|---|---|---|
| `rust` | `just backend-verify` | PR doesn't touch `backend/`, `contracts/`, `justfile` or CI files |
| `web` | `just frontend-verify` (pnpm 12, Node from `devEngines.runtime`, Playwright Chromium) | PR doesn't touch `frontend/`, `contracts/`, `justfile` or CI files, or there is no `frontend/package.json` |
| `contracts` | `just contracts-check-ci origin/<base> HEAD`, `just contracts-verify` | no `contracts/contracts.just` |
| `repo` | `just ci-secrets <base> HEAD` (gitleaks over the new commits), then `just repo-verify` (secrets in git-visible files, TOML format, Claude Code hook tests, shellcheck) | never |
| `workflows` | `just ci-verify` (actionlint, zizmor) | never |
| `deps` | dependency review (`.github/dependency-review.yml`) | not a PR |
| `gate` | fails if any job above failed or was cancelled; skipped is fine | never: **the only required check** |

Hardening:
- Every workflow sets `permissions: {}`, and each job grants only what it needs.
- Every checkout uses `persist-credentials: false`.
- Every job starts with harden-runner in audit mode.
- Actions are pinned to full SHAs. `just ci-pin` runs pinact, and Dependabot bumps the SHAs weekly.
- Tools that taiki-e/install-action doesn't ship (gitleaks, actionlint, oasdiff) go through `.github/actions/install-release-bin` with a SHA-256 check. Bump their URL and checksum by hand, and the TruffleHog image digest in `secrets-deep-scan.yml` too.

## Gate self-tests

A gate that silently stops failing is worse than no gate. Each canary in `tools/canaries/<name>/` makes one deliberate violation and checks that its gate still fails.

```sh
just canaries                  # every canary: about 2 minutes with warm caches
just canaries clippy secret    # only these
just canaries --list           # each canary's gate and violation
```

`tools/canaries/run.sh` handles each canary in five steps:

1. Adds a detached worktree of `HEAD` under `.claude/worktrees/canary-<name>.*`. Uncommitted changes are not included.
2. Makes the violation and commits it in that worktree. The commit uses plumbing commands, so no git hook runs and the gate under test has to be the one that catches it.
3. Runs the canary's `command`.
4. Counts the canary as **caught** only if the command exits non-zero **and** its output matches `expect`. The regex makes sure the right gate failed, not an earlier one.
5. Removes the worktree, whatever happened.

A canary that isn't caught gets one of these results, and the suite exits 1 and names it:

| Result | Meaning |
|---|---|
| `missed` | The command passed with the violation in place: the gate has a bug. |
| `wrong` | The command failed, but without the expected message. Something else broke first, so the gate is unproven. |
| `stale` | The violation no longer applies to `HEAD`: refresh the canary. |
| `error` | The worktree or the commit couldn't be made. |

Each canary's full output goes to `.claude/state/canaries/<name>.log` (`VAK_CANARY_LOG_DIR`). The e2e canary serves on `E2E_PORT`, default 4176. Every canary worktree shares one cargo target dir, `backend/target/canaries` (`VAK_CANARY_TARGET_DIR`), so dependencies build once.

The `canaries` job in `nightly.yml` runs `just canaries` every night and on demand. It doesn't block PRs. A red run uploads the logs as the `canary-logs` artifact.

### Canaries

Results from `just canaries` on 2026-10-30. The run used `main` at c51282d plus this suite, and caught 11 of 12 canaries in about 2 minutes:

| Canary | Gate (recipe) | Violation | Expected failure (`expect`) | Result |
|---|---|---|---|---|
| `layer-services` | `xtask check-layers` (`just backend-verify-fast`) | `vak-domain` depends on `vak-services` | `vak-domain may not depend on vak-services` | caught |
| `layer-tokio` | `xtask check-layers` (`just backend-verify-fast`) | `vak-domain` depends on `tokio` | `vak-domain may not use external crate tokio` | caught |
| `contract-edit` | `contract-check --allow-pending` (`just contracts-verify-fast`) | one assertion in `contract_health.rs`, signed as `contract/health/v1`, is loosened | `contract_health.rs changed since contract/health/v1` | caught |
| `clippy` | clippy `-D warnings` (`just backend-verify-fast`) | `unwrap()` in the `GET /health` handler | `clippy::unwrap-used` | caught; also live in [#7](https://github.com/nehmetohme/voice-agent-kitchen/pull/7) |
| `secret` | gitleaks (`just repo-secrets`) | a fake AWS access key ID in a new tracked file | `leaks found: 1` | caught |
| `ux-primary` | app-shell contract e2e and `auditScreen()` (`just frontend-verify`) | a second primary `Button` in the app shell's top bar | `2 visible primary actions (max 1)` | caught |
| `coverage-floor` | `xtask coverage-gate` (`just backend-coverage`) | an untested public function in `vak-domain` | `vak-domain … FAIL` | caught |
| `vulnerable-dep` | `cargo deny check` (`just backend-verify`) | `vak-audio` depends on `protobuf` 2.28.0 | `RUSTSEC-2024-0437` | caught (`cargo audit` flags it too) |
| `frontend-boundary` | dependency-cruiser (`just frontend-verify-fast`) | `features/plans` imports `features/agents/views/agents-overview` | `error no-cross-feature` | caught |
| `openapi-drift` | OpenAPI drift (`just contracts-verify-fast`) | the health response schema changes without `just openapi` | `contracts/openapi.json differs from the Rust API` | caught |
| `workflow-unpinned` | zizmor (`just ci-verify`) | a new workflow uses `actions/checkout@v4` | `unpinned-uses` | caught |
| `workflow-write-all` | zizmor (`just ci-verify`) | a new single-job workflow sets `permissions: write-all` | `excessive-permissions` | **missed**: see below |
| Claude Code hook guards | `tools/tests/hooks` (`just repo-hooks-test`, part of `just verify`) | edits to generated, owner-only and contract-tagged files; Stop and SubagentStop gates; post-edit lint | the hook tests' own assertions | covered there, not by a canary |

**Known gap: `workflow-write-all`.** zizmor's default `regular` persona doesn't report workflow-level `permissions: write-all`, or `contents: write`, in a workflow with one job. It does report `write-all` at job level, or at workflow level once a workflow has two or more jobs. `zizmor --persona=auditor` reports the single-job case, and it finds nothing else in `.github/` today. The fix belongs in `tools/ci.just` and is the owner's call. Until then the nightly run stays red on this canary, as it should.

### Writing or refreshing a canary

- **Folder:** `tools/canaries/<name>/` holds `canary.conf` (`gate=`, `violation=`, `command=`, `expect=`) and one of:
  - `patch.diff`: make the violation in a scratch worktree of `main`, then save it with `git diff --full-index`. The full blob IDs let `git apply --3way` still apply it after unrelated lines nearby move.
  - `apply.sh`: a script, for violations that a diff can't hold. The `secret` canary assembles its key at run time, so this repository never contains it. Manifest and lockfile edits use `cargo add`, which keeps working as dependencies change. Growing files are appended to, and the coverage canary sizes its untested code from the crate.
- **Make the violation trip only its own gate.** It must be formatted and pass every other check that runs before that gate, so that `expect` proves the right gate failed.
- **Use the gate's own recipe** as `command`, the one `just verify` and CI run, not the bare tool, so the canary also proves the recipe is wired up.
- **A `stale` canary:** in a scratch worktree of `main`, run `git apply --3way tools/canaries/<name>/patch.diff`, resolve any conflict, and save the patch again with `git diff --full-index`.
- **Don't use AWS's documented example key** (`AKIAIOSFODNN7EXAMPLE`) to test gitleaks: gitleaks allowlists key IDs that end in `EXAMPLE`.

## Repository settings as code

`.github/rulesets/*.json` is the source of truth; GitHub doesn't read these files. Keys starting with `_` are notes, and they're stripped before sending.

```sh
just rulesets-apply true   # plan: shows what would be created or updated, with a diff
just rulesets-apply        # create or update each ruleset by name (repo admin only)
```

- **`main`**:
  - require a PR, squash merges only, and linear history
  - required check `gate`, from the GitHub Actions app, with strict up-to-date branches
  - no force pushes and no deletion
  - conversations must be resolved
- **`contract-tags`**: only repository admins create, move or delete `refs/tags/contract/**`.

### Owner checklist after merging

1. Run `just setup` in each clone.
2. Run `just rulesets-apply true`, then `just rulesets-apply`. In the ruleset UI, check that `contract-tags` lists *Repository admin* as its bypass actor. Role ID 5 is undocumented.
3. Keep CodeQL **default setup** off; `codeql.yml` is the advanced setup.
4. Optional: in Settings → Actions, turn on "Require actions to be pinned to a full-length commit SHA".

### Later changes

- **Agents get a GitHub App identity:** in `main.json` set `require_code_owner_review: true` and `required_approving_review_count: 1`, then re-apply.
- **The repo moves to an organization:** add a `merge_queue` rule (`SQUASH`, `ALLGREEN`), set `strict_required_status_checks_policy: false`, and turn auto-merge into "merge when ready". `ci.yml` and `codeql.yml` already listen to `merge_group`.
