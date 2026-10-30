# Repository skeleton

M0 creates this tree. Folders marked 📘 get a `CLAUDE.md`. Each `CLAUDE.md` stays under ~100 lines and covers:
- what the folder is responsible for
- what it may and may not depend on
- conventions for that layer
- testing expectations
- one short example

Nested `CLAUDE.md` files only load when Claude touches that folder. The rules that must always apply also live in `.claude/rules/*.md` with `paths:` scoping.

```
voice/
├── CLAUDE.md 📘                 global: TDD + contract tags, worktrees, `just verify`, layer map, use the graph
├── README.md  LICENSE  PROMPT.md
├── justfile                    verify, verify-fast, dev, dev-light, task, task-done, contract-approve, openapi
├── lefthook.yml                pre-commit, pre-push, post-commit/checkout/merge/rewrite
├── .gitignore  .editorconfig  .graphifyignore  .worktreeinclude
│
├── .claude/
│   ├── settings.json           hooks + permissions (deny `git tag`, pushing tags, editing generated files)
│   ├── agents/                 9 engineering + 5 UX reviewers (read-only)
│   ├── hooks/                  pre-edit-guard.sh, post-edit-lint.sh, graph-update.sh, stop-gate.sh
│   ├── rules/                  backend-layers.md, frontend-layers.md, contracts.md, deploy.md (paths-scoped)
│   ├── schemas/review-verdict.json
│   └── skills/verify/SKILL.md  on-demand /verify
│
├── .github/
│   ├── CODEOWNERS  dependabot.yml  dependency-review.yml  contract-signers
│   ├── rulesets/               main.json, contract-tags.json (applied with gh api)
│   └── workflows/              ci.yml, codeql.yml, scorecard.yml, secrets-deep-scan.yml, nightly.yml, deploy.yml
│
├── contracts/ 📘                the agreements between parts; changes need review
│   ├── openapi.json            generated from Rust, committed, drift-checked
│   ├── contracts.toml          contract-test manifest pinned by signed tags
│   ├── media/                  vak.media.v1 event schemas
│   ├── serving/                stt.json, tts.json: model request/response shapes
│   ├── decisions/              platform decision questions (opt-out, escalation, outcome)
│   └── rule-packs/             versioned compliance packs per use case and jurisdiction
│
├── backend/ 📘                  Rust workspace
│   ├── Cargo.toml  Cargo.lock  rust-toolchain.toml  rustfmt.toml  clippy.toml
│   ├── deny.toml  layers.toml  coverage.toml  .config/nextest.toml
│   ├── crates/
│   │   ├── domain/ 📘           pure types and rules; no I/O, no async, no frameworks
│   │   ├── audio/ 📘            frames, codecs, resampling, VAD
│   │   ├── providers/ 📘        model and transport ports (traits) only
│   │   ├── repositories/ 📘     persistence ports (traits) only
│   │   ├── repositories-pg/ 📘  sqlx implementations, migrations/, .sqlx/
│   │   ├── adapters/ 📘         one crate per external system; implement provider ports only
│   │   │   ├── llm-openai/  stt-serving/  tts-serving/  decision-systemone/
│   │   ├── databricks-auth/ 📘  OAuth M2M token cache, Lakebase credentials
│   │   ├── pipeline/ 📘         per-session actor, turn-taking, barge-in, plan execution, budgets
│   │   ├── services/ 📘         use cases; orchestrate repositories, providers, pipeline
│   │   ├── api/ 📘              axum routers, DTOs, validation, errors, WebSocket, OpenAPI
│   │   ├── config/ 📘           typed settings, secrets, local vs Databricks profiles
│   │   ├── telemetry/ 📘        tracing, OpenTelemetry, metric names and buckets
│   │   ├── testkit/ 📘          fakes, builders, audio fixtures, Postgres container (dev-only)
│   │   └── contract-tests/ 📘   HTTP tests against contracts/openapi.json (dev-only)
│   ├── apps/
│   │   ├── server/ 📘           composition root: serve | worker | migrate | openapi
│   │   └── model-shim/ 📘       dev-only: Databricks serving shapes → local mlx-audio
│   ├── fixtures/audio/         recorded utterances, barge-in, silence, noise
│   └── xtask/ 📘                check-layers, coverage-gate, openapi, contract-check
│
├── frontend/ 📘                 React + Vite SPA
│   ├── package.json  pnpm-lock.yaml  vite/vitest/playwright/orval configs
│   ├── .oxlintrc.json  .oxfmtrc.json  .dependency-cruiser.cjs  tsconfig.json  .size-limit.json
│   ├── tokens/                 DTCG design tokens → shared/ui/tokens.css
│   ├── tools/lint-rules/       custom Oxlint rules for UX laws
│   ├── src/
│   │   ├── app/ 📘              bootstrap, providers, error boundary
│   │   ├── routes/ 📘           thin file routes; compose feature views only
│   │   ├── features/ 📘         one folder per flow step; no cross-feature imports
│   │   │   ├── agents/          index.ts · views/ · state/ · api/ · __contract__/
│   │   │   ├── plans/           (same shape)
│   │   │   ├── testing/
│   │   │   └── publishing/
│   │   ├── shared/
│   │   │   ├── api/ 📘          generated/ (orval, never edited) + http.ts
│   │   │   ├── ui/ 📘           shadcn components on Base UI; the only Base UI importer
│   │   │   ├── voice/ 📘        voice session port + WebSocket/fake adapters, AudioWorklet
│   │   │   └── lib/             pure helpers
│   │   └── test/                MSW handlers, fixtures, fake-mic audio
│   ├── e2e/ 📘                  golden/, contract/, support/ux-audit.ts
│   └── .storybook/
│
├── deploy/ 📘
│   ├── local/                  docker-compose.yml (Postgres), process-compose.yml, models.sh
│   └── databricks/             databricks.yml bundle, app/ (app.yaml, wheel launcher),
│                               serving/stt/, serving/tts/ (MLflow pyfunc), resources/
│
└── docs/
    ├── research/  plan/        (this research and plan)
    ├── adr/ 📘                  one decision per file, numbered
    ├── spikes/                 spike reports
    └── ux-guardrails/ 📘        cognitive.md, interaction.md, perception.md, memory.md, complexity.md, golden-paths.md
```

## What the main `CLAUDE.md` files say

| File | Key rules |
|---|---|
| `/CLAUDE.md` | Work only in a task worktree. Write contract tests first and never edit tagged ones. `just verify` must pass. Check the Graphify graph before cross-cutting changes. Link to the layer map and the plan. |
| `backend/CLAUDE.md` | Layer diagram and `layers.toml` summary. `thiserror` in libraries and `anyhow` only in apps. No `unwrap` or `expect`. Every external call has a timeout. |
| `crates/domain` | No I/O, async, sqlx, axum or reqwest. Every rule is a pure function with unit and property tests. 95% coverage. |
| `crates/providers`, `crates/repositories` | Traits and port types only. Every port has a shared contract test suite in testkit that every implementation must pass. |
| `crates/adapters` | Implement exactly one port each. Map vendor errors to port errors. Never log request bodies (PII). Test against wiremock and the local server. |
| `crates/pipeline` | No blocking calls in async code. Bounded channels with a declared overflow policy. Every stage records a latency histogram. Tests use simulated time. |
| `crates/services` | Business rules come from domain. Talk to the outside only through ports. One public function per use case. Test with fakes from testkit. |
| `crates/api` | Thin: parse, validate, call one service, map errors to problem+json. Every route is in the OpenAPI document and has auth. |
| `frontend/CLAUDE.md` | Layer direction `routes → features → shared`. Inside a feature: `views → state → api`. Only `shared/ui` builds UI primitives. The UX law gates are listed with links to `docs/ux-guardrails/`. |
| `features/*/views` | Components only: no fetching, no generated-client imports. One primary action per screen. Targets of at least 44 px. |
| `features/*/state` | Query options, XState machines, small stores. No JSX. |
| `shared/api` | Never edit `generated/`; run `just openapi`. Responses are Zod-parsed in `http.ts`. |
| `contracts/` | Changes here break the other side. OpenAPI is generated, never edited by hand. Contract versions are bumped, never rewritten. |
| `deploy/` | Infra as code only. No secrets in files. Prod changes deploy only from CI. |
