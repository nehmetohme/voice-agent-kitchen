# Voice Agent Kitchen

A platform to provision, plan, test and publish real-time AI voice agents. Read these before starting:

- Brief: `PROMPT.md`
- Plan: `docs/plan/README.md`
- Research: `docs/research/README.md`

## Ground rules

1. **Work in a task worktree, never on `main`.** Start with `just task <name>` and finish with a PR.
2. **Write contract tests first.** Register them in `contracts/contracts.toml` and commit. The owner reviews them and signs `contract/<feature>/vN`. Never edit a file covered by a contract tag. To change a contract, bump its version.
3. **`just verify` must pass before you say a task is done.** The Stop hook runs it, plus the relevant reviewer agents, and blocks on failure.
4. **Respect the layers.**
   - Backend: `backend/layers.toml`, enforced by crates, `xtask check-layers` and cargo-deny.
   - Frontend: dependency-cruiser.
   - Read the folder's `CLAUDE.md` before editing in it.
5. **Never hand-edit generated files.**
   - `contracts/openapi.json`: regenerate with `just openapi`.
   - `frontend/src/shared/api/generated/**`
   - `backend/**/.sqlx/**`
6. **Never create, move or push git tags.** Only the owner signs tags.
7. **Models:** production uses Databricks Model Serving, and local development uses local models. They switch through configuration only. Never add an outside model vendor.
8. **No secrets in files. No PII in logs or traces.**

## Repository map

| Path | What |
|---|---|
| `backend/` | Rust workspace: domain → repositories/providers (ports) → pipeline → services → api, plus adapters and apps |
| `frontend/` | React SPA: routes → features (views → state → api) → shared |
| `contracts/` | OpenAPI, media protocol, model serving shapes, decision questions, rule packs, contract-test manifest |
| `deploy/` | Local dev stack and Databricks bundle |
| `docs/` | Research, plan, ADRs, spikes, UX guardrails |
| `.claude/` | Hooks, reviewer agents, rules, verdict schema |
| `.github/` | CI workflows, rulesets, CODEOWNERS, Dependabot |

## Finding your way

Use the Graphify code graph before cross-cutting changes:
- `graphify query "<question>"`
- `graphify explain <symbol>`
- `graphify affected <file>`

The graph lives in `graphify-out/`, and hooks keep it current.

## Commands

| Command | Does |
|---|---|
| `just verify` | Every gate: format, lint, types, architecture, tests, coverage, security, contracts |
| `just verify-fast` | The quick subset, run on commit |
| `just dev` / `just dev-light` | Local stack with the full or light LLM profile |
| `just task <name>` / `just task-done <name>` | Create or remove a task worktree |
| `just openapi` | Regenerate the OpenAPI contract and frontend client |
| `just canaries [name...]` | Gate self-tests: each gate must still fail on a deliberate violation (slow; nightly runs it) |

## Status

M0 (foundation) is in progress, so some folders and commands don't exist yet. `docs/plan/milestones.md` lists them.
