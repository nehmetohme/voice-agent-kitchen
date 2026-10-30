# Milestones and build order

Every task follows the same loop: worktree → contract tests first → you sign the contract tag → implement → `verify` + reviewers → PR → merge queue. The contract tests listed per milestone are what gets written and tagged **before** any implementation.

## M0: Foundation

Goal: an empty but fully gated repo. Each gate is proven to fail when it should.

| Task | Delivers |
|---|---|
| T0.1 Repo bootstrap | `git init`, org repo, license, `.gitignore`, root `README.md` and `CLAUDE.md`, root `justfile` |
| T0.2 Backend skeleton | Cargo workspace with every crate (empty `lib.rs`), `CLAUDE.md` per crate, `layers.toml`, `deny.toml`, lints, toolchain pin, `xtask check-layers`, `GET /health` |
| T0.3 Frontend skeleton | Vite + React + TS app with the feature folder tree, `CLAUDE.md` per layer, oxlint/oxfmt, dependency-cruiser, Vitest, Playwright + `auditScreen()`, Storybook, design tokens |
| T0.4 Contracts | utoipa → `contracts/openapi.json` → orval; drift and `oasdiff` checks; `contracts.toml` + `xtask contract-check`; signing key and `.github/contract-signers` |
| T0.5 Claude Code gates | `.claude/settings.json` hooks, hook scripts, 14 reviewer agents, verdict schema, path-scoped rules, `/verify` skill |
| T0.6 Git hooks + Graphify | `lefthook.yml`, gitleaks, Graphify pinned and wired to hooks, `.graphifyignore` |
| T0.7 CI + GitHub | Workflows, rulesets as code, Dependabot, dependency review, CodeQL, CODEOWNERS, merge queue, bot identity |
| T0.8 Local dev | Postgres in Docker, process-compose, `just dev` / `just dev-light`, model setup script (llama-server, mlx-audio, Kev-4B, Bolt E4B), STT/TTS shim |

**Contract tests first:** `contract/health/v1`, a test that `GET /health` returns 200 with the version.

**Exit criteria:**
- `just verify` is green on the skeleton.
- **Gate self-tests:** a canary PR for each of these turns CI red. Each is a planned violation:
  - layer violation
  - edit to a contract file
  - clippy warning
  - committed fake secret
  - extra primary button (UX gate)
  - coverage drop
  - vulnerable dependency

  **Status (2026-10-30):** `just canaries` (`tools/canaries/`, run nightly) catches all seven, plus a frontend boundary violation, OpenAPI drift and an unpinned action: 11 of 12 canaries. Draft PR [#7](https://github.com/nehmetohme/voice-agent-kitchen/pull/7) shows the live case: a clippy warning turns CI's `rust` job and `gate` red. Still open: zizmor misses `permissions: write-all` in a single-job workflow (`workflow-write-all`). Details are in [`docs/ci.md`](../ci.md#gate-self-tests).
- A green PR merges through the merge queue.
- The Stop hook blocks an unfinished task in Claude Code.

## M1: Spikes (timeboxed, about 2 weeks, parallel with M2)

Each spike writes a short report in `docs/spikes/` and an ADR in `docs/adr/`. Spikes are throwaway code on their own branches and are never merged into the product.

| Spike | Question | Pass bar |
|---|---|---|
| S1 | Does the Rust binary run on Databricks Apps when shipped as a wheel? Which CPU architecture? | Health endpoint served through SSO |
| S2 | Do browser WebSockets through the Apps proxy stay up for 30+ minutes? | No forced disconnect; reconnect works |
| S3 | STT on Model Serving (Parakeet, warm A10G): latency per utterance and accuracy on 16 kHz browser audio | P50 ≤ 250 ms per utterance |
| S4 | TTS on Model Serving (Kokoro): time to first audio per clause | P50 ≤ 250 ms |
| S5 | LLM time to first token over 24 h: FMAPI Haiku 5.5 vs self-hosted Qwen3.6-35B-A3B | P50 ≤ 400 ms, tool calls reliable |
| S6 | One full browser conversation end to end on Databricks | Mouth-to-ear P50 ≤ 1.2 s |
| S7 | Lakebase from sqlx: hourly token refresh; a database branch per CI run | No failed connections over 3 h; branch ready in < 60 s |
| S8 | Decision latency (System One) from the app | P95 ≤ 300 ms for a 4-question fan-out |

If a spike misses its bar, we revisit that part of the design with you before M4. Under decision D3, outside vendors are not a fallback.

## M2: Provision

| Contract tests first | |
|---|---|
| Domain | Creating an agent needs only name, purpose and voice; every other field gets a default; invalid input gives a typed error |
| API | `POST /agents` (≤ 3 required fields), `GET /agents`, `PATCH /agents/{id}`, `GET /agents/{id}/progress` |
| UI (`@golden`) | Create an agent in ≤ 8 interactions; autosave shows "Saved" within 2 s and survives reload; progress shows 25% |

Exit: the create-agent golden path passes, including `auditScreen()`.

## M3: Plan

| Contract tests first | |
|---|---|
| Domain | Plan graph validation, property-based: one start node, every node reachable, every non-end node has an exit, edge conditions are non-empty, rule-pack locked text present |
| Domain | Templates per use case produce valid plans; rule packs load and are versioned |
| API | `GET/PUT /agents/{id}/plan`, `POST /plan/validate` returns located errors |
| UI | Flow editor: add, connect and delete nodes; errors shown on the node; keyboard accessible |

Exit: an agent created from each template has a valid plan; progress shows 50%.

## M4: Voice core

| Contract tests first | |
|---|---|
| Pipeline (simulated time, fake engines) | Locked disclosure is spoken first; barge-in stops agent audio within 1 frame and cancels the LLM; turn order preserved; per-stage timings recorded |
| Pipeline | Opt-out: each of the three layers alone ends the session; a decision timeout never suppresses an opt-out |
| Pipeline | Plan routing: the decision picks the edge; low confidence falls back to an LLM tool call |
| Media protocol | `vak.media.v1` event schemas, checked on server and client |
| Adapters | Each adapter passes its port's shared contract suite against wiremock and the local servers |

Exit: a local browser conversation works with local models; latency histograms are visible.

## M5: Test

| Contract tests first | |
|---|---|
| UI (`@golden`, fake mic) | Start test call → see live transcript → interrupt the agent → end → summary with latency P50/P95 and a "Publish" call to action (peak-end rule) |
| Services | Simulated caller runs a scenario to completion; rubric scores via the decision engine; a run passes only if every required scenario passes |
| API | `POST /test-runs` creates a job; `GET /test-runs/{id}` reports progress and verdict |

Exit: test runs are reproducible on the same draft hash; progress shows 75%.

## M6: Publish

| Contract tests first | |
|---|---|
| Domain | The publish gate needs a passing run on the exact draft hash plus no regression against the live version |
| Repo | `agent_versions` and `audit_events` reject UPDATE and DELETE |
| API + UI (`@golden`) | Publish → version number + live channel + "Roll back" link; rollback asks for confirmation and writes an audit event |

Exit: publish and rollback golden paths pass; progress shows 100%.

## M7: Deploy to Databricks

| Contract tests first | |
|---|---|
| Adapters | The serving-contract suite passes against staging endpoints (STT, TTS, LLM, decisions) |
| Repo | Lakebase credential refresh under a forced token expiry |
| Smoke | Staging: create → plan → test → publish through the deployed app |

Delivers:
- the bundle (`deploy/databricks/`) and wheel packaging
- the STT and TTS pyfunc wrappers
- OIDC deploy workflow
- dev, staging and prod targets
- telemetry into Unity Catalog
- cost dashboards

## Later

| Item | Adds |
|---|---|
| L1 Telephony | Public Rust edge relay; Telnyx adapter (Twilio second); 8 kHz audio path |
| L2 Outbound campaigns | Consent ledger, Do-Not-Call scrubbing, calling windows in the recipient's time zone, frequency caps, 5-year call audit, voicemail detection |
| L3 Web widget | Public widget token through the relay; WebRTC (`str0m`) |
| L4 EU region | Region pinning, EU rule packs, DPA and sub-processor list |
| L5 Analytics | Lakebase → Delta sync; `ai_decide` QA and compliance audits at scale; lead scoring |
| L6 TTS quality | Bake-off of Qwen3-TTS, Ming-3.0, Chatterbox on Databricks |
