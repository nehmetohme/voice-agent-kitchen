# Quality system

Sources: research reports [02](../research/02-rust-backend.md), [03](../research/03-frontend.md), [05](../research/05-claude-code-tooling.md), [06](../research/06-graphify-github-ci.md).

The same checks run in four places, from fastest to slowest:

1. after every edit (Claude Code hook)
2. before Claude finishes a task (Stop hook)
3. on commit and push (git hooks)
4. in CI before the merge queue merges

## 1. Verify contract

`just verify` is the single entry point. `just verify-fast` runs the subset marked ⚡.

| Check | Backend (Rust) | Frontend (TypeScript) | Repo-wide |
|---|---|---|---|
| Format ⚡ | `cargo fmt --check` | `oxfmt --check` | `taplo fmt --check` (TOML) |
| Lint ⚡ | `cargo clippy --all-targets -- -D warnings` | `oxlint --type-aware`, custom UX lint rules | `actionlint`, `zizmor` |
| Types ⚡ | `cargo check` (via clippy) | `tsc --noEmit` (strict) | |
| Architecture ⚡ | `cargo xtask check-layers` | `depcruise` | |
| Unit tests | `cargo nextest run` | `vitest run` | |
| Integration | `#[sqlx::test]` + testcontainers, wiremock | Vitest browser mode, MSW | |
| End-to-end | pipeline simulations with recorded audio | Playwright `@golden` paths + `auditScreen()` | |
| Coverage | `cargo llvm-cov` + `xtask coverage-gate` | Vitest coverage | |
| Security | `cargo deny check` (advisories, licenses, bans, sources), `cargo audit` | `pnpm audit`, `osv-scanner` | `gitleaks` |
| Hygiene | `cargo shear` (unused deps), `sqlx prepare --check` | `knip` (unused files and exports) | |
| Contracts ⚡ | OpenAPI drift, `oasdiff breaking` | generated-client drift | `xtask contract-check` |
| Performance | criterion benchmarks (nightly, trend only) | `size-limit`, INP budget in Playwright | |

**Coverage floors** (line coverage; CI fails below):

| Area | Floor |
|---|---|
| `vak-domain` | 95% |
| `vak-services`, `vak-pipeline` | 90% |
| `vak-api`, `vak-repositories-pg` | 85% |
| adapters | 75% |
| frontend `features/*/state`, `shared/lib` | 90% |
| frontend overall | 80% |

Mutation testing (`cargo mutants --in-diff`) runs nightly on domain and services, as a report, not a gate.

## 2. Test-driven development with contract tags

For every feature:

1. **Write the contract tests first** in the feature's worktree. They live in paths matching the contract globs:
   - `**/tests/contract_*.rs`
   - `**/*.contract.test.ts`
   - `e2e/contract/**`

   Register them in `contracts/contracts.toml`, then commit. CI fails with "untagged contract file", and that failure is the approval gate.
2. **You review and sign.** `just contract-approve <feature> 1` creates and pushes the signed tag `contract/<feature>/v1`. Only admins can create these tags (tag ruleset).
3. **Implement.** CI stays green only while every contract file is byte-identical to its tag.
4. **To change a contract**, bump the version in `contracts.toml` and sign `contract/<feature>/v2`.

Defence layers:
- signature checked against the base branch's `.github/contract-signers` file
- CI runs the base branch's copy of the checker
- CODEOWNERS on contract paths
- a Claude Code `PreToolUse` hook that blocks edits to tagged files
- permission rules that deny `git tag` and pushing tags

## 3. Worktrees and branches

```
just task <name>         → git worktree at .claude/worktrees/<name>, branch task/<name>
  write contract tests   → commit → draft PR → you sign the contract tag
  implement              → hooks run on every edit; Stop hook runs verify + reviewers
  PR ready               → CI green → merge queue → re-run on the merged result → merge
  just task-done <name>  → remove the worktree
```

- One task, one worktree, one branch, one PR.
- Nothing is pushed directly to `main`.
- Agents open PRs under a separate bot identity (a GitHub App), so you can approve them as code owner.

## 4. Claude Code hooks

| When | Hook | Does | Blocks? |
|---|---|---|---|
| Before an edit | `PreToolUse` on `Edit\|Write` | Denies edits to contract-tagged files, generated files (`shared/api/generated/**`, `.sqlx/**`) and `layers.toml` | Yes |
| After an edit | `PostToolUse` on `Edit\|Write` (sync, ~60 s) | Formats the file, lints its package | Feeds errors back |
| After an edit | `PostToolUse` on `Edit\|Write` (async) | `graphify update .`, debounced with a lock file | No |
| Task done | `Stop` | If the tree changed since the last green run: `just verify`, then the relevant reviewers in parallel; exit 2 with findings on failure | Yes |
| Outside Claude | lefthook `pre-commit` / `pre-push` | `verify-fast` on commit, full `verify` on push, gitleaks on staged files | Yes |
| Outside Claude | lefthook `post-commit/checkout/merge/rewrite` | Graphify refresh | No |

Notes from research:
- Hook scripts read `cwd` from the hook input, because `${CLAUDE_PROJECT_DIR}` doesn't follow worktrees.
- Reviewers run as `claude -p --agent <name> --json-schema .claude/schemas/review-verdict.json`. Each run has a turn cap and a budget cap. `CLAUDE_REVIEW_CHILD=1` prevents recursion.
- The Stop hook stops retrying after a repeated, unchanged failure and asks you instead.

## 5. Reviewer subagents

Each reviewer is read-only (no Edit or Write). It returns `{verdict: pass|fail, findings: [{severity, file, line, message}]}`, and any `blocker` or `major` finding fails the gate. `stop-gate.sh` picks reviewers from the changed paths, so the model can't skip one.

### Engineering reviewers

| Reviewer | Runs when | Checks |
|---|---|---|
| `security` | any source | auth on every route, input validation at the router, secrets, injection, PII in logs, CVEs |
| `code-quality` | any source | readability, complexity, duplication, error handling |
| `hygiene` | any source | dead code, leftover TODOs, naming, unused dependencies |
| `separation` | any source | layer rules (`layers.toml`, dependency-cruiser), business logic in the right layer |
| `testing` | any source | tests written first, contract files untouched, meaningful assertions, coverage |
| `performance` | `vak-pipeline`, `vak-audio`, adapters, `features/testing` | latency budget, blocking calls in async code, allocations on the hot path, N+1 queries |
| `ci-cd` | `.github/**`, `justfile`, `lefthook.yml`, `deploy/**` | workflow correctness, pinned actions, least-privilege permissions |
| `dependencies` | manifests, lockfiles, `deny.toml` | necessity, license, maintenance, duplicates |
| `docs` | `docs/**`, `CLAUDE.md`, READMEs, public API changes | docs and `CLAUDE.md` files match the code |

### UX reviewers (run on `frontend/**`)

| Reviewer | Laws |
|---|---|
| `ux-cognitive` | Hick, Miller, Occam's razor, Pareto |
| `ux-interaction` | Fitts (including target distance), Doherty threshold |
| `ux-perception` | Jakob, proximity, similarity, uniform connectedness, Prägnanz, Von Restorff |
| `ux-memory` | serial position, peak-end, Zeigarnik |
| `ux-complexity` | Tesler, Postel, Parkinson |

Each UX reviewer reads its checklist in `docs/ux-guardrails/` and uses the `auditScreen()` results from Playwright as evidence. Twelve of the 18 laws also have hard CI gates (report 03 §4). Examples:
- at most one primary action per screen
- targets of at least 44×44 px
- feedback within 400 ms
- create-agent needs no more than 3 required fields
- autosave within 1 s

## 6. CI (GitHub Actions)

One workflow, `ci.yml`, triggered on `pull_request`, `merge_group` and pushes to `main`:

```
changes ─┬─► rust       fmt, clippy, nextest, llvm-cov gate, deny, audit, shear, check-layers, sqlx check
         ├─► web        oxfmt, oxlint, tsc, vitest + coverage, depcruise, knip, size-limit
         ├─► e2e        Playwright golden paths + auditScreen against the built app and Postgres
         ├─► contracts  openapi drift, oasdiff, client drift, contract-check (base-branch copy)
         ├─► deps       dependency-review (PR only)
         ├─► secrets    gitleaks
         └─► workflows  actionlint, zizmor
                 ▼
               gate   (if: always(); the only required status check)
```

Other workflows:

| Workflow | Runs |
|---|---|
| `codeql.yml` | Rust, JS/TS, Actions; on PRs, merge queue, `main` and weekly |
| `scorecard.yml` | OpenSSF Scorecard, weekly |
| `secrets-deep-scan.yml` | trufflehog over full history, weekly |
| `nightly.yml` | mutation tests, benchmarks, Lighthouse user flows |
| `deploy.yml` (M7) | `main` → staging, signed release tag → prod |

Hardening:
- default `permissions: {}`
- actions pinned to full SHAs with pinact, plus Dependabot updates
- harden-runner on every job
- caching with `Swatinem/rust-cache`

## 7. GitHub settings

| Setting | Value |
|---|---|
| Ruleset `main` | Require a PR, required check `gate`, merge queue, linear history, code-owner review, dismiss stale approvals, resolve conversations, no force push or deletion |
| Ruleset `contract-tags` | Only admins create, update or delete `contract/*/*` tags |
| Security | Secret scanning + push protection, Dependabot alerts and security updates, private vulnerability reporting |
| `dependabot.yml` | cargo, npm, github-actions, docker; weekly, grouped, with a cooldown before new releases |
| `dependency-review.yml` | Fail on moderate or higher; license allowlist |
| `CODEOWNERS` | You on everything; always on `contracts/`, `.github/`, `layers.toml`, `deny.toml`, `.claude/` |
| Rulesets as code | `.github/rulesets/*.json`, applied with `gh api` |

## 8. Graphify

- Pinned tool: `uv tool install graphifyy==0.9.97`. It parses Rust and TypeScript locally with tree-sitter, with no LLM and no data leaving the machine.
- Kept current by the async post-edit hook and the lefthook post-commit, checkout, merge and rewrite hooks. Its built-in hooks skip worktrees, so we don't use them.
- Output goes to `graphify-out/`, which is gitignored. Each worktree builds its own graph.
- Agents query it with `graphify query|explain|path|affected` or through `graphify-mcp`. The root `CLAUDE.md` tells agents to check the graph before large changes.
- It doesn't enforce rules; `check-layers` and dependency-cruiser do. Later, `graphify affected` can choose which tests `verify-fast` runs.
