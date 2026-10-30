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
| `nightly.yml` | nightly, manual | placeholders for mutation tests and benchmarks |

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
