# 06: Graphify, GitHub gates, CI design and contract enforcement

> Research for brief §5–§9. Checked on 2026-10-30 against docs.github.com, the GitHub changelog, and each tool's repo (see [Sources](#sources)). Prices are US list prices.

## TL;DR

- **The merge queue decides the account type.** GitHub offers it only on organization-owned repos: public repos (a free org is enough), or private repos on Enterprise Cloud. Personal accounts and private repos on the Team plan don't get it. Your GitHub login is a personal account with no orgs, so you need to create an org in every scenario.
- **The cheapest setup with every gate costs $0: a public repo in a free org.** That gives you the merge queue, rulesets, CodeQL (Rust support has been GA since 2025-10), secret scanning with push protection, dependency review and Dependabot. To keep the repo private with the same gates, you need Enterprise Cloud plus both security add-ons, about **$70 per human committer per month**.
- **Solo-developer trap: you can't approve your own PR.** Required code-owner review only works if agents open PRs under a separate identity, such as a GitHub App or one machine account. You stay the code owner and the only person who can sign contract tags.
- **Contract design:** a manifest pins each feature to a signed annotated tag `contract/<feature>/vN`. CI runs the base branch's copy of the checker. The checker verifies the tag signature against the base branch's list of allowed signers, then checks that every file in the tag's own manifest is byte-identical at HEAD. Contract-like files that no tag covers fail the check. A tag ruleset, CODEOWNERS and a Claude Code edit guard add further layers.
- **Graphify is [Graphify-Labs/graphify](https://github.com/Graphify-Labs/graphify).** `safishamsi/graphify` redirects there. PyPI package `graphifyy`, Apache-2.0, version 0.9.97, pre-1.0 and released several times a week, so pin the version.
  - It parses Rust and TS locally with tree-sitter and **no LLM**. Only the optional docs and media pass calls an LLM.
  - Its built-in git hooks **skip linked worktrees**, so we trigger `graphify update .` ourselves from hooks. Keep `graphify-out/` gitignored.
- **Graphify helps agents navigate code; it does not enforce layer rules.** Enforcement comes from crate boundaries, a `cargo metadata` architecture test, cargo-deny `wrappers` and dependency-cruiser.
  - CI is one `ci.yml` whose **only required check is an `if: always()` gate job**.
  - Pin actions by SHA with pinact. Move to Actions lockfiles (GA since 2026-10-28) once zizmor supports them.
  - Use lefthook for git hooks, and gitleaks plus a weekly trufflehog scan for secrets.

## 1. Availability matrix

✅ = included · 💲 = paid add-on · ❌ = not available. GitHub Free on a **private personal** repo gets only Dependabot from this list.

| Gate | Personal, public (Free/Pro) | Personal, private (Pro, $4) | Org, public (Free+) | Org, private, Team ($4/user) | Org, private, Enterprise Cloud ($21/user) |
| --- | --- | --- | --- | --- | --- |
| **Merge queue** | ❌ | ❌ | ✅ | ❌ | ✅ |
| Branch and tag rulesets, required status checks, linear history, block force-push and deletion | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Push rulesets** (file-path, extension, size restrictions) | ❌ | ❌ | ❌ (private and internal repos only) | ✅ | ✅ |
| CODEOWNERS with required code-owner review | ✅ | ✅ | ✅ | ✅ | ✅ |
| **CodeQL** (Rust GA, JS/TS, Actions) | ✅ free | ❌ (can't be bought) | ✅ free | 💲 Code Security, $30/committer | 💲 same |
| **Secret scanning and push protection** | ✅ free | ❌ | ✅ free | 💲 Secret Protection, $19/committer | 💲 same |
| Dependency review action | ✅ | ❌ | ✅ | 💲 Code Security | 💲 Code Security |
| Dependabot alerts, security and version updates, groups | ✅ | ✅ | ✅ | ✅ | ✅ |
| Actions lockfile and "require full-SHA pinning" policy | ✅ | ✅¹ | ✅ | ✅ | ✅ |
| Harden-Runner (StepSecurity) free tier | ✅ | ❌ (paid) | ✅ | ❌ (paid) | ❌ (paid) |
| OpenSSF Scorecard action | ✅ | ❌ (needs GHAS) | ✅ | 💲 | 💲 |
| Standard GitHub-hosted runner minutes | free | included $ quota | free | included $ quota | included $ quota |

¹ The lockfile GA changelog names the Free, Team and Enterprise Cloud plans. It doesn't mention Pro.

Notes:
- The merge-queue column comes straight from the docs: "available in any public repository owned by an organization, or in private repositories owned by organizations using GitHub Enterprise Cloud".
- Code Security and Secret Protection can only be bought on Team or Enterprise plans.
- GHAS is billed per *active committer*: anyone who pushed in the last 90 days. GitHub App bots don't count.

## 2. Recommended setup

### 2.1 Decision: account and visibility (yours to make)

| Option | Cost/month (1 human) | What you get | What you lose |
| --- | --- | --- | --- |
| **A. Public repo in a free org** | **$0** | Every gate, including the merge queue, CodeQL, push protection and dependency review. Harden-Runner and Scorecard are free. Actions minutes are free. | Push rulesets. The code is public: you must choose a license, and the product is visible. |
| **B. Private repo, org on Enterprise Cloud + Code Security + Secret Protection** | **~$70** ($21 + $30 + $19), plus ~$70 per extra human | Everything, including push rulesets | Harden-Runner needs a paid tier (or drop it). Actions minutes are metered beyond the included amount. |
| C. Private repo, org on Team + both add-ons | ~$53 | Everything except the native merge queue | The brief requires a merge queue; you would need a third-party queue (Mergify, Trunk). |

**Recommendation:** Option A if you're willing to open-source the code; otherwise Option B, which you can start on the 30-day Enterprise trial. Either way, create the org first and transfer the repo into it. Push rulesets aren't required: the signed-tag contract check (§4) covers what they would do.

### 2.2 Identities (needed for code-owner review to work)

- **You:** admin, code owner, and the only contract-tag signer.
  - Sign with an SSH key that needs your presence: a FIDO2 `ed25519-sk` key, or a key held in an agent that prompts per use, such as 1Password.
  - Otherwise an agent running in your shell could sign tags too.
- **Agents:** a **GitHub App** (preferred) or **one machine account** with the *write* role.
  - Agents must not be admins and must not appear in any bypass list.
  - A GitHub App needs no seat and is ignored for GHAS billing. Its tokens expire after 1 hour.
  - GitHub's terms allow one free machine account. In an org, a machine account costs a seat and a GHAS committer license.
- PR authors can't approve their own PRs. Without this split, required code-owner review blocks every PR you open.

### 2.3 Settings checklist

- **Rulesets** (§3): `main` and `contract-tags`, applied with `gh api -X POST repos/ORG/REPO/rulesets --input .github/rulesets/<name>.json`. GitHub doesn't read these JSON files from the repo; they're our source of truth.
- **Merges:** squash only, linear history, auto-delete head branches.
- **Merge queue:** squash, `ALLGREEN`, build concurrency 5, 60-minute timeout.
  - Leave "require branches up to date" (`strict`) **off**. The queue already guarantees the same thing.
- **Required check:** `gate` only, with its source pinned to the GitHub Actions app (`integration_id: 15368`).
- **CodeQL:** enforced through the `code_scanning` rule, not as a status check.
- **Actions settings:**
  - Require actions to be pinned to a full-length commit SHA.
  - Allow only GitHub-owned actions plus an allowlist.
  - Set the default `GITHUB_TOKEN` to read-only.
  - Don't let Actions create or approve PRs.
- **Security:** turn on the dependency graph, Dependabot alerts and security updates, secret scanning with push protection, and CodeQL **advanced setup**.
  - Use advanced setup because the workflow must also listen to `merge_group`.
  - Public repos: also turn on private vulnerability reporting.

## 3. Proposed `.github/` tree

```text
.github/
├── CODEOWNERS
├── dependabot.yml
├── dependency-review.yml        # fail-on-severity, allow-licenses (config for the action)
├── contract-signers             # SSH allowed_signers for contract tags; CI reads the BASE copy
├── rulesets/                    # source of truth, applied with `gh api` (GitHub doesn't read them)
│   ├── main.json
│   ├── contract-tags.json
│   └── contracts-push.json      # optional, private org repos on Team+ only
└── workflows/
    ├── ci.yml                   # PR + merge_group + push(main): changes → rust/web/contracts/deps/secrets/workflows → gate
    ├── codeql.yml               # rust, javascript-typescript, actions; PR + merge_group + push(main) + weekly
    ├── scorecard.yml            # public repo only; push(main) + weekly
    └── secrets-deep-scan.yml    # weekly trufflehog over full history (verified/unknown results only)
contracts/contracts.toml          # contract manifest (§4)
tools/ci/contract-check.sh        # or `cargo xtask contract-check`
tools/hooks/graph-refresh.sh      # Graphify refresh with lock/debounce (§5.3)
lefthook.yml  .graphifyignore  .worktreeinclude  justfile
```

**`workflows/ci.yml`**, abridged. pinact rewrites each `@SHA` to the form `@<40-hex> # vX.Y.Z`.

```yaml
name: ci
on:
  pull_request:
  merge_group: { types: [checks_requested] }
  push: { branches: [main] }          # trusted event → keeps caches warm in main's scope
permissions: {}                       # every job opts in to what it needs
concurrency:
  group: ci-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}   # never cancel queue runs
env:
  BASE_SHA: ${{ github.event.pull_request.base.sha || github.event.merge_group.base_sha || github.event.before }}
jobs:
  changes:                            # path filter on PRs only; queue + main run everything
    runs-on: ubuntu-24.04
    permissions: { contents: read, pull-requests: read }
    outputs:
      rust: ${{ github.event_name != 'pull_request' || steps.f.outputs.rust == 'true' }}
      web:  ${{ github.event_name != 'pull_request' || steps.f.outputs.web == 'true' }}
    steps:
      - uses: actions/checkout@SHA # v7.0.1
        with: { persist-credentials: false }
      - id: f
        if: github.event_name == 'pull_request'
        uses: dorny/paths-filter@SHA # v4.0.3
        with:
          filters: |
            rust: [crates/**, Cargo.*, rust-toolchain.toml, deny.toml, justfile]
            web:  [apps/**, packages/**, package.json, pnpm-*.yaml, api/openapi.json, justfile]
  rust:
    needs: changes
    if: needs.changes.outputs.rust == 'true'
    runs-on: ubuntu-24.04
    timeout-minutes: 30
    permissions: { contents: read }
    steps:
      - uses: step-security/harden-runner@SHA # v2.22.3, free on public repos only
        with: { egress-policy: audit }
      - uses: actions/checkout@SHA # v7.0.1
        with: { persist-credentials: false }
      - uses: Swatinem/rust-cache@SHA # v2.9.3
        with: { save-if: "${{ github.ref == 'refs/heads/main' }}" }
      - uses: taiki-e/install-action@SHA # v2.88.0, prebuilt binaries with checksums
        with: { tool: "just,cargo-nextest,cargo-llvm-cov,cargo-deny,cargo-audit" }
      - run: just verify-rust         # fmt --check, clippy -D warnings, nextest, llvm-cov ≥ 80%, deny, audit
  web: {}                             # same shape: pnpm/action-setup, setup-node (cache: pnpm), `just verify-web`
  contracts:
    runs-on: ubuntu-24.04
    permissions: { contents: read }
    steps:
      - uses: actions/checkout@SHA # v7.0.1
        with: { persist-credentials: false }
      - run: |                        # run the BASE copy of the checker so a PR can't neuter it
          git fetch --no-tags --depth=1 origin "$BASE_SHA" '+refs/tags/contract/*:refs/tags/contract/*'
          git show "$BASE_SHA:tools/ci/contract-check.sh" > "$RUNNER_TEMP/cc.sh"
          bash "$RUNNER_TEMP/cc.sh" "$BASE_SHA" HEAD
  dependency-review:
    if: github.event_name != 'push'   # the action supports pull_request and merge_group
    runs-on: ubuntu-24.04
    permissions: { contents: read }
    steps:
      - uses: actions/checkout@SHA # v7.0.1
        with: { persist-credentials: false }
      - uses: actions/dependency-review-action@SHA # v5.0.0
        with: { config-file: ./.github/dependency-review.yml }
  secrets: {}                         # gitleaks binary (pinned + checksum): gitleaks git --redact --log-opts="$BASE_SHA..HEAD"
  workflows: {}                       # actionlint, zizmor-action (advanced-security: false without GHAS), pinact run --check
  gate:                               # the ONLY required status check
    if: always()                      # without this, a failed dependency → gate "skipped" → counts as success
    needs: [changes, rust, web, contracts, dependency-review, secrets, workflows]
    runs-on: ubuntu-24.04
    permissions: {}
    steps:
      - if: contains(needs.*.result, 'failure') || contains(needs.*.result, 'cancelled')
        run: exit 1
```

**`rulesets/main.json`**. The `code_scanning` rule needs CodeQL to be available (see §1).

```json
{ "name": "main", "target": "branch", "enforcement": "active",
  "conditions": { "ref_name": { "include": ["~DEFAULT_BRANCH"], "exclude": [] } },
  "bypass_actors": [],
  "rules": [
    { "type": "deletion" }, { "type": "non_fast_forward" }, { "type": "required_linear_history" },
    { "type": "pull_request", "parameters": {
        "required_approving_review_count": 0, "require_code_owner_review": true,
        "dismiss_stale_reviews_on_push": true, "require_last_push_approval": false,
        "required_review_thread_resolution": true, "allowed_merge_methods": ["squash"] } },
    { "type": "required_status_checks", "parameters": {
        "strict_required_status_checks_policy": false,
        "required_status_checks": [{ "context": "gate", "integration_id": 15368 }] } },
    { "type": "merge_queue", "parameters": {
        "merge_method": "SQUASH", "grouping_strategy": "ALLGREEN", "max_entries_to_build": 5,
        "min_entries_to_merge": 1, "max_entries_to_merge": 5,
        "min_entries_to_merge_wait_minutes": 0, "check_response_timeout_minutes": 60 } },
    { "type": "code_scanning", "parameters": { "code_scanning_tools": [
        { "tool": "CodeQL", "security_alerts_threshold": "high_or_higher", "alerts_threshold": "errors" } ] } } ] }
```

**`rulesets/contract-tags.json`**. Only admins (you) can create, move or delete `contract/*/*` tags. Role ID 5 = Admin is undocumented; verify it after import.

```json
{ "name": "contract-tags", "target": "tag", "enforcement": "active",
  "conditions": { "ref_name": { "include": ["refs/tags/contract/*/*"], "exclude": [] } },
  "bypass_actors": [{ "actor_type": "RepositoryRole", "actor_id": 5, "bypass_mode": "always" }],
  "rules": [{ "type": "creation" }, { "type": "update" }, { "type": "deletion" }, { "type": "non_fast_forward" }] }
```

**`dependabot.yml`**

```yaml
version: 2
updates:
  - package-ecosystem: cargo
    directory: /
    schedule: { interval: weekly }
    cooldown: { default-days: 7, semver-major-days: 14 }   # default is already 3 days
    groups:
      cargo-minor-patch: { patterns: ["*"], update-types: [minor, patch] }
      cargo-security:    { applies-to: security-updates, patterns: ["*"] }
  - package-ecosystem: npm            # also covers pnpm (pnpm-lock.yaml)
    directory: /
    schedule: { interval: weekly }
    cooldown: { default-days: 7 }
    groups:
      npm-minor-patch: { patterns: ["*"], update-types: [minor, patch] }
      npm-security:    { applies-to: security-updates, patterns: ["*"] }
  - package-ecosystem: github-actions # bumps the SHA and its "# vX.Y.Z" comment, and actions.lock if adopted
    directory: /
    schedule: { interval: weekly }
    groups: { actions: { patterns: ["*"] } }
  - { package-ecosystem: docker,         directories: ["/deploy/**", "/crates/*", "/apps/*"], schedule: { interval: weekly } }
  - { package-ecosystem: docker-compose, directory: /, schedule: { interval: weekly } }
  - { package-ecosystem: rust-toolchain, directory: /, schedule: { interval: monthly } }
```

**`CODEOWNERS`**. GitHub always uses the base branch's copy, so a PR can't weaken it.

```text
# One pattern per line; the LAST matching line wins, so sensitive paths go at the bottom.
# Layers: in an org, use teams (@org/backend-domain …). With one human, each layer line means
# that human approves every PR touching the layer (decision 3 in §10).
/crates/domain/              @OWNER
/crates/repositories/        @OWNER
/crates/services/            @OWNER
/crates/routers/             @OWNER
/crates/voice-pipeline/      @OWNER
/apps/web/                   @OWNER
# Sensitive paths: always the human, even when a layer team owns the folder.
/.github/                    @OWNER
/tools/ci/                   @OWNER
/contracts/                  @OWNER
**/tests/contract_*          @OWNER
**/*.contract.test.ts        @OWNER
/deny.toml                   @OWNER
/lefthook.yml                @OWNER
/justfile                    @OWNER
```

## 4. Contract-check design

### Options compared

| Option | Strength | Weakness |
| --- | --- | --- |
| `git diff` against the **latest** `contract/*` tag | Simple | Tags are global: while a v2 PR is open, every other branch fails. Anyone with write access can push unsigned tags. Still needs a map from feature to files. |
| Manifest of hashes | Explicit file list; fast | The agent can just update the hashes. It only works if a human reviews every manifest change. |
| CODEOWNERS on contract paths | Server-side human approval | Needs a separate bot identity. It only checks who approved, not that files are unchanged since the tag. |
| Push ruleset path restriction | Hard server-side block | Private org repos on Team+ only. Also blocks the agent from writing the first contract tests, so you would push them yourself. |
| **Signed tag + manifest + blob check (recommended)** | Approval is cryptographic: your signature *is* the approval. Works with squash merges. No per-branch race. | Needs a small script and a signing key. |

### Flow

1. In its worktree, the agent writes failing contract tests and adds `[features.<f>] version = 1, paths = [...]` to `contracts/contracts.toml`. It commits. CI fails with "untagged contract file", which is the approval gate.
2. You review the tests, then run `just contract-approve <f> 1`. That runs `git tag -s contract/<f>/v1 -m "<f> v1" <commit>` and `git push origin contract/<f>/v1`. Only admins can create these tags (tag ruleset).
3. The agent implements the feature. CI stays green as long as every contract file matches the tag byte for byte. This works with squash merges because it compares blobs, not ancestry.
4. To change a contract, edit the files and bump `version = 2`. CI fails until you sign `contract/<f>/v2`. Lowering the version below the base branch's value fails.

**Layout:** contract tests live in places that match `contract_globs`:
- Rust: `crates/*/tests/contract_<f>.rs` and `crates/*/tests/contract_<f>/**`.
- TypeScript: `apps/web/**/*.contract.test.ts`.
- End-to-end: `e2e/contract/<f>/**`.

```toml
# contracts/contracts.toml: HEAD's copy pins versions; each TAG's copy defines that version's file set
contract_globs = ["**/tests/contract_*.rs", "**/tests/contract_*/**", "**/*.contract.test.ts", "e2e/contract/**"]
[features.agent-provisioning]
version = 1
paths = ["crates/services/tests/contract_agent_provisioning.rs", "apps/web/src/features/agents/**/*.contract.test.ts"]
```

### Checker logic (CI runs the base-branch copy; `just verify` runs it locally too)

```bash
# contract-check.sh BASE HEAD
set -euo pipefail; base=$1; head=$2; rc=0
err() { echo "::error::$*"; rc=1; }
signers=$(mktemp); git show "$base:.github/contract-signers" > "$signers"     # keys come from BASE only
covered=$(mktemp)
for f in $(features "$head"); do                     # parse $head:contracts/contracts.toml (taplo/dasel/xtask)
  v=$(version "$head" "$f"); tag="contract/$f/v$v"
  git -c gpg.ssh.allowedSignersFile="$signers" verify-tag "$tag" >/dev/null 2>&1 \
    || { err "$tag missing or not signed by an approved key"; continue; }
  (( v >= $(version "$base" "$f" || echo 0) )) || err "$f: contract version went down"
  while read -r p; do                                # file set comes from the TAG's own manifest
    echo "$p" >> "$covered"
    [[ "$(git rev-parse -q --verify "$tag:$p" || true)" == "$(git rev-parse -q --verify "$head:$p" || true)" ]] \
      || err "$p changed or deleted since $tag"
  done < <(git ls-tree -r --name-only "$tag" -- $(pathspecs "$tag" "$f"))   # ':(glob)…' pathspecs
done
while read -r p; do err "$p looks like a contract but no signed tag covers it"; done \
  < <(comm -23 <(git ls-tree -r --name-only "$head" -- $(globs "$base") | sort -u) <(sort -u "$covered"))
exit $rc
```

**Defence layers:**
1. The **tag ruleset**: only admins can create, update or delete `contract/*/*` tags.
2. **Signatures checked against the base branch's** `contract-signers` file, so a PR can't add its own key.
3. **CODEOWNERS** on `contracts/`, contract globs, `tools/ci/`, `.github/`, and the signers file.
4. The **base-branch copy** of the checker runs in CI.
5. A **Claude Code `PreToolUse` hook** denies `Edit`/`Write` on files covered by a tag, plus permission deny rules for `git tag` and `git push … refs/tags`.
6. Optional, private org only: a **push ruleset** restricting `contracts/**` to admins.
7. Optional, Enterprise Cloud: the ruleset `workflows` rule can force a workflow pinned to a fixed ref.

## 5. Graphify

### 5.1 What it is

- **Graphify** is `Graphify-Labs/graphify`. `github.com/safishamsi/graphify` returns *Moved Permanently* to it.
  - License Apache-2.0, ~139k stars, default branch `v8`.
  - PyPI package **`graphifyy`** (double y); the CLI command is `graphify`.
  - Latest version 0.9.97, released 2026-10-27. Five releases shipped between 2026-10-20 and 2026-10-27.
  - The README warns that other `graphify*` packages on PyPI are not affiliated.
- Same vendor, different product: **Graphify Cloud** (hosted, `api.graphify.com/mcp`). It indexes your GitHub repo on their servers.
  - Their docs' agent instructions say to "recommend [hosted] by default". For a private product, **use Graphify Local**.
- Similar names that are not this tool: forks (e.g. `Thizeidler/graphify`), Microsoft GraphRAG, and Zep's Graphiti (agent memory).

### 5.2 Capabilities

- **Install:** `uv tool install "graphifyy[mcp,watch]==0.9.97"` (Python 3.10+). Then `graphify install --project --platform claude` writes `.claude/skills/graphify/SKILL.md`; commit it.
- **Languages:** about 40 through tree-sitter, including `.rs`, `.ts`, `.tsx`, `.sql` and Markdown. `extract --cargo` adds crate-to-crate edges from `Cargo.toml`; `--postgres DSN` reads a live schema over a read-only connection.
- **Outputs** in `graphify-out/`: `graph.json` (NetworkX node-link format; nodes carry `source_file`, edges carry relation and confidence), `GRAPH_REPORT.md` (god nodes, communities), `graph.html`, `manifest.json` and `cache/`.
- **Commands:**
  - Build: `extract` (full; `--code-only` needs no API key), `update .` (incremental, AST-only, re-parses changed files only), `watch .` (rebuild on save).
  - Ask: `query "<q>" --budget N`, `explain X`, `path A B`, `affected X --depth N`, `god-nodes`. Exit codes: `0` answer, `3` ambiguous name, `4` no match.
  - Other: `export wiki|html|neo4j`, `merge-graphs`, `prs` (PR impact through `gh`).
- **Speed:** a full code-only build of the 19.6k-node graphify repo took ~26 s, and httpx (78 files) 2.6 s. After a one-line edit, `update` re-parsed 1 file.
- **Edge confidence:** every edge is tagged `EXTRACTED`, `INFERRED` or `AMBIGUOUS`.
- **The graph is syntactic.** Macro-generated code (axum, sqlx, utoipa) and calls through traits (our provider adapters) are partial or only `INFERRED`.

### 5.3 Keeping it current: hooks design

- **Built-in option:** `graphify hook install` adds background rebuilds on post-commit, post-checkout, post-merge and post-rewrite, plus a union merge driver.
- **Its hook script exits early in linked worktrees** (`git-dir != git-common-dir`). All our work happens in worktrees, so the built-in hooks would only ever refresh the main checkout.
  - It also writes into `.git/hooks`, which competes with lefthook.
  - **Don't use it.**
- **Our design** runs the same refresh script from three places: Claude Code, lefthook, and worktree creation.

```bash
# tools/hooks/graph-refresh.sh: code-only, no LLM; one run at a time, re-runs once if edits arrived meanwhile
cd "$(git rev-parse --show-toplevel)"; mkdir -p graphify-out
export GRAPHIFY_BACKEND=none PYTHONHASHSEED=0 GRAPHIFY_NO_TIPS=1 GRAPHIFY_NO_CTA=1 GRAPHIFY_QUERY_LOG_DISABLE=1
lock=graphify-out/.refresh.lock
mkdir "$lock" 2>/dev/null || { touch graphify-out/.dirty; exit 0; }
trap 'rmdir "$lock"' EXIT
while :; do rm -f graphify-out/.dirty; graphify update . >>graphify-out/refresh.log 2>&1 || true
  [ -e graphify-out/.dirty ] || break; done
```

- **Claude Code** (`.claude/settings.json`). `async: true` runs the hook in the background without blocking Claude:
  `{"hooks":{"PostToolUse":[{"matcher":"Edit|Write|MultiEdit","hooks":[{"type":"command","command":"\"$CLAUDE_PROJECT_DIR\"/tools/hooks/graph-refresh.sh","async":true}]}]}}`
- **Git:** lefthook runs `(tools/hooks/graph-refresh.sh &)` from `post-commit`, `post-checkout`, `post-merge` and `post-rewrite`. These fire in every worktree, and `post-checkout` also fires on `git worktree add`.
- **New worktrees:** add `graphify-out/` to `.worktreeinclude` so `claude --worktree` copies the main checkout's graph and cache. Confirm the cache actually gets reused.
- **`.graphifyignore`:** `target/`, `node_modules/`, `dist/`, `coverage/`, the generated OpenAPI client, and `.claude/worktrees/`. Without the last entry, the main checkout would index every nested worktree.
- **Gotcha:** `update` refuses to overwrite a graph that has shrunk, so after large deletions it can quietly keep the old graph. Check `refresh.log` and run `graphify update . --force` when that happens.

### 5.4 Claude Code integration

- **Skill:** `/graphify .` builds the graph, including the docs pass, which costs tokens. `/graphify . --update` refreshes it.
- **Always-on guidance:** `graphify claude install` adds a `## graphify` section to `CLAUDE.md` and PreToolUse hooks (`graphify hook-guard search|read`). Before Grep, Bash search, Read or Glob, these hooks add a reminder pointing at the graph.
  - They never block unless you install with `--strict`, which blocks the first raw read in each session until the agent has queried the graph.
  - The installer writes the **absolute path** to `graphify` into the hook command. If you work on more than one machine, change it to plain `graphify`.
- **MCP:** a stdio server with 10 tools (`query_graph`, `get_node`, `get_neighbors`, `shortest_path`, `god_nodes`, `graph_stats`, `get_community`, `list_prs`, `get_pr_impact`, `triage_prs`).
  - Add it in `.mcp.json` as `graphify-mcp graphify-out/graph.json`.
  - The path is relative, so inside a worktree it resolves to that worktree's graph.
- **How agents should query:**
  - Start with `graphify query "<task>" --budget 1500`, then open only the files it cites.
  - Before changing a public symbol, run `graphify affected <sym> --depth 3`.
  - Use `path` and `explain` to understand relationships.
  - Read your own uncommitted edits from disk.
  - For exact Rust references (trait impls, macros), fall back to rust-analyzer through an LSP tool.

### 5.5 LLM use, cost and privacy

- **Code:** parsed locally with tree-sitter; nothing leaves the machine and there's no API cost. Video and audio are transcribed locally with Whisper.
- **Docs, PDFs and images:** a semantic pass by your assistant's model (`/graphify` dispatches Claude subagents and uses your plan's tokens) or by an API key.
  - The backend is auto-detected in this order: Gemini → Kimi (Moonshot servers in China) → Claude → OpenAI → DeepSeek → Azure → Bedrock → Ollama.
  - `graphify label`, `prs --triage` and the `claude -p` fallback for naming communities also call LLMs.
- **Policy:**
  - Set `GRAPHIFY_BACKEND=none` in hooks and CI.
  - Run the docs pass by hand, rarely, or with `--backend ollama` for a fully local run.
  - No telemetry. The README says queries are logged to `~/.cache/graphify-queries.log`, but the env-var docs say logging is off by default. Set `GRAPHIFY_QUERY_LOG_DISABLE=1` to be safe.

### 5.6 Commit or gitignore? **Gitignore `graphify-out/` entirely**

- `graph.json` is a single line of JSON, so every PR that touches code changes it, and concurrent PRs always conflict.
- Graphify's union merge driver is configured per clone in local git config. GitHub's server-side PR and merge-queue merges never run it.
- Union merging also keeps deleted nodes alive.
- Rebuilding locally takes seconds and costs nothing. If you want graph context in PRs, build it in CI as an artifact (`graphify extract . --code-only`), or use the vendor's recipe for posting a PR impact comment.

### 5.7 Can it detect layer violations?

**Not as a gate.** Graphify Local has no rules engine. "Formal verification of architectural invariants" exists only in the hosted product, and `INFERRED` edges would produce false positives.

It is useful as **advisory input** for the Separation reviewer subagent:
- `graphify path "<domain type>" "sqlx"`, `affected`, communities.
- A small script over `graph.json` that flags `imports`/`calls` edges going upward between `crates/<layer>/` prefixes, counting `EXTRACTED` edges only.

**The authoritative checks are:**
1. **Crate boundaries.** An upward `use` fails to compile.
2. **A `cargo metadata` architecture test** (an xtask or test) that asserts the allowed workspace dependency DAG and that `domain` has no I/O crates.
3. **cargo-deny `[bans]` with `wrappers`**, e.g. `{ crate = "sqlx", wrappers = ["vak-repositories"] }` and `{ crate = "axum", wrappers = ["vak-routers"] }`.
4. **dependency-cruiser** (or eslint-plugin-boundaries) for the TS `views → state → api` direction.

## 6. CI design notes

- **`merge_group` is mandatory.** Without it, required checks never report and the queue times out. Add `merge_group` to `ci.yml` and the CodeQL advanced-setup workflow. `dependency-review-action` and `paths-filter` both support it.
- **Skipped required checks:**
  - A *workflow* skipped by `paths:`, a branch filter or a commit message leaves its check **Pending**, which blocks the merge forever.
  - A *job* skipped by `if:` reports **Success**.
  - A job whose dependency failed is skipped, and "skipped" counts as success.
  - So: no `paths:` on required workflows; filter at job level with `dorny/paths-filter`; require only `gate`, with `if: always()`.
  - Run everything on `merge_group` and `push`, so the queue always tests the full suite.
- **Concurrency:** cancel superseded runs on PRs only. Never cancel `merge_group` runs; each merge group has its own ref.
- **Caching:**
  - Runs can restore caches from their own branch or the default branch. Only trusted events (like `push` to `main`) write to the default branch's scope. The limit is **10 GB per repo**, and entries unused for 7 days are evicted.
  - Use `Swatinem/rust-cache` with `save-if: refs/heads/main`, so PR and queue runs read without causing churn. It sets `CARGO_INCREMENTAL=0` and caches the registry and dependency builds.
  - Add `sccache` (with `SCCACHE_GHA_ENABLED`) only if builds get slow. It overlaps with rust-cache and competes for the same 10 GB.
  - pnpm: `actions/setup-node` with `cache: pnpm`. Install tools with `taiki-e/install-action` (prebuilt binaries), not `cargo install`.
- **CodeQL:**
  - Languages: `rust` (build-mode `none`; needs rustup and cargo on the runner), `javascript-typescript`, and `actions`.
  - The CodeQL CLI license forbids use on closed-source code without a GHAS license, so you can't "just run the CLI" on a private repo.

## 7. Workflow hardening

- **Pinning (today):** SHA-pin every `uses:` with `pinact run` and add `pinact run --check` to CI. Dependabot's `github-actions` ecosystem updates the SHA together with its `# vX.Y.Z` comment. Turn on the repo's "require full-length SHA" policy.
  - Downside: Dependabot doesn't raise *alerts* for actions pinned by SHA.
- **Actions lockfiles (GA 2026-10-28; Free, Team and Enterprise Cloud):**
  - `gh actions-lock` writes `.github/workflows/actions.lock`. It records every action **including transitive ones**, plus the source repo's identity.
  - You can keep tags in the workflow file, so Dependabot alerts keep working. `gh actions-lock --verify` runs in CI, and an admin policy can require the lockfile.
  - **Adopt it once zizmor releases its lockfile support:** the issue was closed 2026-10-23, but the latest release, v1.31.1 from 2026-10-15, predates it.
  - pinact already handles lockfiles: it leaves locked actions alone and pins reusable workflows.
- **Permissions:** workflow-level `permissions: {}`, and each job grants only what it needs. CodeQL needs `security-events: write` (plus `actions: read` on private repos); Scorecard needs `security-events: write` and `id-token: write`.
  - Use `persist-credentials: false` on every checkout, and never use `pull_request_target`.
- **Linters** (run in CI and through lefthook):
  - `actionlint` checks syntax and expressions.
  - `zizmor` is a security linter: template injection, credential persistence, unpinned uses, excessive permissions. With `zizmor-action`, set `advanced-security: false` when there's no GHAS, so findings fail the job instead of uploading SARIF.
- **harden-runner:** blocks or audits network egress. It's free only on **public** repos with GitHub-hosted runners; private repos need StepSecurity Enterprise. Start in `egress-policy: audit`, then switch to `block` with an allowlist.
- **OpenSSF Scorecard:** a weekly run plus one on push to `main`. Free on public repos; private repos need GHAS.
  - Expect a low "Code-Review" score unless agent PRs (bot identity) get your approval.
- **Runner images:** pin `ubuntu-24.04` instead of `-latest`, and set `timeout-minutes` on every job.

## 8. Local hooks manager: lefthook vs pre-commit/prek vs husky

| | lefthook (v2.2.2) | pre-commit (v4.7) / **prek** (v0.5.7, Rust) | husky (v9.1.7) |
| --- | --- | --- | --- |
| Runtime | Single Go binary | Python / single Rust binary | Node |
| Parallel jobs, globs, `{staged_files}` | ✅ built in | ✅ (prek also runs jobs concurrently) | ❌ (needs lint-staged) |
| Monorepo | `root:` per command | prek workspace mode | Manual |
| Worktrees | Reads config per worktree at run time. **Open bugs:** #1398 (an npm-installed shim bakes in one worktree's `node_modules` path; it silently stops running if that worktree is deleted) and #1077 (wrong cwd with `root:`) | prek honours worktree-local `core.hooksPath` | `core.hooksPath=.husky/_` is relative, so it works per worktree |
| Maintenance | Active | Active; Dependabot supports the `pre-commit` ecosystem | Last release 2024-11 |

**Recommendation: lefthook.**
- Install it as a **standalone binary** (mise or brew), **not** from `node_modules`, and run `lefthook install` from the main checkout.
- Every hook calls a `just` recipe, so local hooks, Claude hooks and CI run the same commands.
- Suggested hooks:
  - `pre-commit`: fast format and lint on staged files, gitleaks, local contract check.
  - `pre-push`: `just verify`.
  - `post-*`: graph refresh.
- If the worktree bugs cause trouble, prek is the fallback.

## 9. Secrets scanning: gitleaks vs trufflehog

- **gitleaks** (MIT, v8.30.1): fast regex and entropy rules, `.gitleaks.toml`, SARIF output.
  - Pre-commit: `gitleaks git --pre-commit --staged --redact`. CI: `gitleaks git --log-opts "<base>..HEAD"`.
  - **The README now says it is feature-complete and will get security patches only.** The author moved to **Betterleaks** (MIT, v1.9.0, adds secret validation); re-evaluate it in 2027.
  - Since v2, `gitleaks-action` has a non-MIT license and needs a (free) license key for org repos. **Run the pinned binary instead.**
- **trufflehog** (AGPL-3.0, v3.99.7): 800+ detectors with **live verification** (`--results=verified,unknown`).
  - Slower. Verification sends the candidate secrets to the providers' APIs.
- **Use both, for different jobs:**
  - gitleaks in `pre-commit` and in CI on the PR range: deterministic and offline.
  - trufflehog weekly over full history in `secrets-deep-scan.yml`.
  - GitHub push protection as the server-side backstop: free on public repos, Secret Protection on private ones.

## 10. Risks, open questions, decisions for you

1. **Decide the account and visibility (blocking):** A, a public repo in a free org at $0, or B, private on Enterprise Cloud with GHAS at about $70 per committer per month. Either way, create an org and move the repo into it.
2. **Decide the agent identity:** a GitHub App (no seat or GHAS cost; tokens need minting) or a machine account (simpler; costs a seat in a paid org).
3. **Decide the CODEOWNERS scope:** layer owners (so you approve every PR that touches a layer), or only sensitive paths plus contracts. With one human these lead to different amounts of review work.
4. **Decide the contract signing key:** a hardware-backed SSH key? Should anyone besides you be allowed to sign?
5. **Verify after setup:**
   - Does `required_approving_review_count: 0` together with `require_code_owner_review: true` still enforce owner approval? If not, set the count to 1.
   - Is Admin really `actor_id 5`?
   - Do dependency review and secret scanning run on the public repo? GitHub's GHAS feature table conflicts with the dependency review page.
6. **pnpm 12 lockfile.** If `pnpm-lock.yaml` is written as two YAML documents (the default when pnpm pins its version), the dependency graph doesn't see npm dependencies. Dependabot alerts, security updates and dependency review then go blind. Keep `osv-scanner` in `verify`, and check this on day 1.
7. **Graphify risks:**
   - Pre-1.0, with several releases a week. Pin the version and set `GRAPHIFY_NO_AUTO_REFRESH=1` if the skill files are pinned.
   - The vendor's docs push the hosted product.
   - The graph is syntactic, so its answers about macros and trait dispatch are incomplete.
   - The shrink guard can leave a stale graph after large deletions.
8. **New platform features:** Actions lockfiles went GA 2 days ago and zizmor's support isn't released yet. Treat them as phase 2.
9. **Cost on private repos:** CI on `push` to `main` plus `merge_group` runs the suite twice per merge, against a dollar-based included quota ($12 Free, $18 Pro/Team, $300 Enterprise Cloud). Harden-Runner on private repos is a paid product.

## Sources

- **Graphify:** [repo/README](https://github.com/Graphify-Labs/graphify) (`safishamsi/graphify` redirects here) · [PyPI `graphifyy`](https://pypi.org/project/graphifyy/) · [CLI reference](https://docs.graphify.com/open-source/configuration/cli) · [languages](https://docs.graphify.com/open-source/features/languages) · [incremental updates](https://docs.graphify.com/open-source/features/incremental-updates) · [Rust crates and Postgres](https://docs.graphify.com/open-source/features/databases) · [env vars](https://docs.graphify.com/open-source/configuration/environment-variables) · [Claude Code monorepo](https://docs.graphify.com/cookbooks/claude-code-monorepo) · [make agents use the graph](https://docs.graphify.com/cookbooks/make-agents-use-the-graph) · [team graph](https://docs.graphify.com/cookbooks/shared-team-graph) · [MCP tools](https://docs.graphify.com/open-source/features/mcp-tools) · [hosted vs local](https://docs.graphify.com/platform/graphify-vs-local) · [hook worktree guard (source)](https://github.com/Graphify-Labs/graphify/blob/v8/graphify/hooks.py)
- **Claude Code:** [hooks, including `async`](https://code.claude.com/docs/en/hooks) · [worktrees and `.worktreeinclude`](https://code.claude.com/docs/en/worktrees)
- **GitHub, merging and rulesets:** [merge queue](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue) · [about rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets) · [available rules](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets) · [creating rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/creating-rulesets-for-a-repository) · [protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches) · [code owners](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners) · [authors can't approve their own PRs](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/approving-a-pull-request-with-required-reviews) · [REST ruleset schema](https://github.com/github/rest-api-description) · [undocumented RepositoryRole IDs](https://github.com/github/rest-api-description/issues/7111)
- **GitHub, plans and pricing:** [plans](https://docs.github.com/en/get-started/learning-about-github/githubs-plans) · [pricing](https://github.com/pricing) · [security pricing](https://github.com/security/plans) · [machine accounts (Terms of Service)](https://docs.github.com/en/site-policy/github-terms/github-terms-of-service)
- **GitHub, code security:** [about GHAS](https://docs.github.com/en/get-started/learning-about-github/about-github-advanced-security) · [GHAS billing](https://docs.github.com/en/billing/concepts/product-billing/github-advanced-security) · [CodeQL code scanning](https://docs.github.com/en/code-security/code-scanning/introduction-to-code-scanning/about-code-scanning-with-codeql) · [CodeQL languages](https://codeql.github.com/docs/codeql-overview/supported-languages-and-frameworks/) · [Rust GA changelog](https://github.blog/changelog/2025-10-14-codeql-scanning-rust-and-c-c-without-builds-is-now-generally-available/) · [build modes](https://docs.github.com/en/code-security/reference/code-scanning/codeql/codeql-build-options-and-steps-for-compiled-languages) · [default setup](https://docs.github.com/en/code-security/code-scanning/enabling-code-scanning/configuring-default-setup-for-code-scanning) · [advanced setup and `merge_group`](https://docs.github.com/en/code-security/code-scanning/creating-an-advanced-setup-for-code-scanning/customizing-your-advanced-setup-for-code-scanning) · [CodeQL CLI license](https://github.com/github/codeql-cli-binaries/blob/main/LICENSE.md) · [secret scanning](https://docs.github.com/en/code-security/secret-scanning/introduction/about-secret-scanning) · [push protection](https://docs.github.com/en/code-security/secret-scanning/introduction/about-push-protection) · [dependency review](https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/about-dependency-review) · [dependency-review-action](https://github.com/actions/dependency-review-action)
- **GitHub, Dependabot:** [ecosystems](https://docs.github.com/en/code-security/dependabot/ecosystems-supported-by-dependabot/supported-ecosystems-and-repositories) · [options reference](https://docs.github.com/en/code-security/reference/supply-chain-security/dependabot-options-reference)
- **GitHub Actions:** [`merge_group` event](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#merge_group) · [skipped required checks](https://docs.github.com/en/pull-requests/how-tos/merge-and-close-pull-requests/troubleshooting-required-status-checks) · [caching](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching) · [secure use](https://docs.github.com/en/actions/reference/security/secure-use) · [SHA pinning policy](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/enabling-features-for-your-repository/managing-github-actions-settings-for-a-repository) · [workflow lockfiles](https://docs.github.com/en/actions/concepts/security/workflow-dependency-lockfiles) · [lockfiles GA changelog](https://github.blog/changelog/2026-10-28-github-actions-workflow-lockfiles-are-generally-available/) · [gh-actions-lock](https://github.com/github/gh-actions-lock)
- **CI and hardening tools:** [rust-cache](https://github.com/Swatinem/rust-cache) · [sccache-action](https://github.com/mozilla-actions/sccache-action) · [paths-filter](https://github.com/dorny/paths-filter) · [harden-runner](https://github.com/step-security/harden-runner) · [zizmor-action](https://github.com/zizmorcore/zizmor-action) · [zizmor lockfile issue #2371](https://github.com/zizmorcore/zizmor/issues/2371) · [actionlint](https://github.com/rhysd/actionlint) · [scorecard-action](https://github.com/ossf/scorecard-action) · [pinact](https://github.com/suzuki-shunsuke/pinact)
- **Hook managers:** [lefthook install](https://lefthook.dev/usage/commands/install/) · [lefthook #1398](https://github.com/evilmartians/lefthook/issues/1398) · [lefthook #1077](https://github.com/evilmartians/lefthook/issues/1077) · [prek](https://github.com/j178/prek) · [husky](https://github.com/typicode/husky)
- **Secrets scanning:** [gitleaks](https://github.com/gitleaks/gitleaks) · [gitleaks-action](https://github.com/gitleaks/gitleaks-action) · [Betterleaks](https://github.com/betterleaks/betterleaks) · [trufflehog](https://github.com/trufflesecurity/trufflehog)
- **Architecture checks:** [cargo-deny `wrappers`](https://embarkstudios.github.io/cargo-deny/checks/bans/cfg.html) · [dependency-cruiser](https://github.com/sverweij/dependency-cruiser) · [eslint-plugin-boundaries](https://github.com/javierbrea/eslint-plugin-boundaries)
