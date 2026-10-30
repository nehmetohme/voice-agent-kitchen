# Voice Agent Kitchen: implementation plan

Status: **draft for review**, 2026-10-30. Built on [the brief](../../PROMPT.md) and [the research summary](../research/README.md).

## What we are building

A platform where an operator creates a voice agent in four steps:

1. **Provision**: name, purpose, voice, language; everything else has defaults.
2. **Plan**: goals, a conversation flow (graph of nodes and edges), knowledge, guardrails, escalation rules, and a compliance rule pack chosen by use case and jurisdiction.
3. **Test**: talk to the agent in the browser, run scripted scenarios against simulated callers, and see transcripts, latency and rubric scores.
4. **Publish**: freeze an immutable version that passed its tests; roll back at any time.

## Decisions this plan is built on

| Area | Decision |
|---|---|
| Runtime | Production on **Databricks**: Apps (hosting), Model Serving (LLM, STT, TTS, decisions), Lakebase (Postgres). Local dev uses **local models** and local Postgres behind the same interfaces. No outside model vendors. |
| Channels | **Browser only** in the first release. Phone calls and the public relay come later. |
| Use case and market | **Configurable** per agent: use case + jurisdiction select a versioned compliance rule pack. |
| Backend | Rust, layered Cargo workspace: axum, tokio, sqlx, utoipa |
| Frontend | React + Vite SPA, TypeScript, shadcn/Base UI, TanStack, XState, React Flow |
| Repo | Public GitHub repo in a free organization; merge queue, rulesets, CodeQL, secret scanning, Dependabot, dependency review |
| Process | Contract tests first, signed `contract/<feature>/vN` tags, one worktree per task, merge only through the merge queue when CI is green |

## Plan documents

| Document | Contents |
|---|---|
| [architecture.md](architecture.md) | System diagrams (prod and local), backend crates and layer rules, voice pipeline, frontend modules, deployment |
| [domain.md](domain.md) | Domain model, lifecycles, database tables, API surface |
| [quality.md](quality.md) | Verify contract, TDD and contract tags, worktrees, Claude Code hooks, 14 reviewer subagents, CI, GitHub settings, Graphify |
| [milestones.md](milestones.md) | Build order, spikes, contract tests per milestone, exit criteria |
| [skeleton.md](skeleton.md) | Full folder tree and what each `CLAUDE.md` says |

## Roadmap at a glance

| Milestone | Outcome | Depends on |
|---|---|---|
| **M0 Foundation** | Repo, skeleton, `CLAUDE.md` files, verify contract, hooks, reviewers, CI, GitHub gates, Graphify | — |
| **M1 Spikes** | Proof that Rust runs on Databricks Apps, WebSockets last, and the STT/TTS/LLM/decision latencies are acceptable | M0 |
| **M2 Provision** | Create, list and edit agents with defaults; autosave | M0 |
| **M3 Plan** | Flow editor, plan validation, rule packs, guardrails | M2 |
| **M4 Voice core** | Real-time pipeline: VAD, turn detection, STT, LLM streaming, TTS, barge-in; local adapters | M1, M2 |
| **M5 Test** | Browser test call, transcripts, scenarios with simulated callers, rubric scoring | M3, M4 |
| **M6 Publish** | Immutable versions, test-gated publish, rollback, audit log | M5 |
| **M7 Databricks deploy** | Bundles, serving endpoints, Lakebase, OIDC deploy from CI, dev/staging/prod | M1, M6 |
| Later | Telephony + relay, outbound campaigns with the full compliance layer, web widget, EU region | M7 |

M2 and M1 can run in parallel in separate worktrees.

## Settled after review (2026-10-30)

- **License:** Apache-2.0.
- **Stacks:** approved as planned, including the two small pieces of Python that Databricks requires under `deploy/databricks/`.
- **Agent PRs:** opened by a GitHub App, so you can approve them as code owner.

## Still needed from you

- **Create the GitHub organization** in the GitHub web UI (the CLI can't), and tell me its name. M0 needs it for the repo, rulesets, merge queue and the GitHub App.
