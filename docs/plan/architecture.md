# Architecture

Sources: research reports [01](../research/01-voice-pipeline-providers.md), [02](../research/02-rust-backend.md), [03](../research/03-frontend.md), [07](../research/07-databricks-platform.md), [08](../research/08-system-one-models.md), [09](../research/09-nemotron-bolt.md).

## 1. System context

### Production (Databricks, browser only)

```
 Operator browser ──SSO──► Databricks App "vak" (Rust binary, shipped as a wheel from a UC Volume)
   SPA + REST + WebSocket     │  serves the SPA, /api/* REST, /api/sessions/{id}/media (WebSocket audio)
                              │
                              ├─► Model Serving: llm        Claude Haiku 5.5 via FMAPI (default), streamed over SSE
                              │                              or self-hosted Qwen3.6-35B-A3B (decided in spike S5)
                              ├─► Model Serving: stt        Parakeet TDT 0.6B v2, MLflow pyfunc, warm A10G
                              ├─► Model Serving: tts        Kokoro-82M, MLflow pyfunc, warm A10G
                              ├─► Unity Gateway: decisions  System One API, openjev-qwen35-4b
                              ├─► Model Serving: sim-caller pay-per-token FMAPI model (tests only; no GPU floor)
                              ├─► Lakebase Postgres         sqlx over TLS, OAuth token as password, refreshed hourly
                              └─► OpenTelemetry collector   → Unity Catalog otel_* tables
```

- The app runs under one service principal. Operators sign in through Databricks SSO; the app reads their identity from forwarded headers.
- Every model sits behind a Rust port (trait). Switching between local and Databricks changes only base URL and auth.
- Phone calls (later) add a small public Rust relay outside Databricks. The pipeline already treats the media transport as a port, so this is an adapter, not a redesign.

### Local development (macOS, M4 Max, 36 GB)

```
 Browser ──► Vite dev server :5173 ──proxy──► vak-server :3000 (cargo run, native)
                                                ├─► llama-server :8080   LLM, OpenAI-compatible chat
                                                │     profile "full":  Qwen3.6-35B-A3B Q4 (~20 GB)
                                                │     profile "light": Nemotron 3.5 Bolt E4B (~5 GB)
                                                ├─► llama-server :8081   decisions, /v1/systemone, Kev-4B (~3 GB)
                                                ├─► model shim :8001     same request shapes as Databricks
                                                │     /serving-endpoints/{stt,tts}/invocations
                                                │     → mlx-audio :8000  Parakeet (STT), Kokoro (TTS)
                                                └─► Postgres 18 in Docker :5432
```

- Models run natively, because Docker on macOS can't use the GPU. Only Postgres runs in Docker.
- `just dev` starts everything with process-compose. `just dev-light` uses the light LLM profile.
- CI uses fakes for unit tests, Postgres in a container for repository tests, and tiny CPU models only for adapter smoke tests.

## 2. Backend: crates and layers

```
            apps/server (composition root: the only place that wires implementations)
                 │
   api ──► services ──► pipeline ──► providers (ports) ◄── adapters/* (implementations)
                 │                         ▲
                 └──► repositories (ports) ◄── repositories-pg (sqlx)
                                    │
                               domain (pure: types, rules, state machines; no I/O)
```

| Crate | Responsibility |
|---|---|
| `vak-domain` | Entities, value objects, IDs, plan graph and validation, rule packs, decision types, lifecycle state machines. No I/O, no async. |
| `vak-audio` | Audio frames, PCM/Opus/μ-law codecs, resampling, VAD trait and implementations |
| `vak-providers` | Ports: `LlmEngine`, `SttEngine`, `TtsEngine`, `DecisionEngine`, `MediaTransport`, `TelephonyCarrier` (later) |
| `vak-repositories` | Persistence ports: `AgentRepo`, `PlanRepo`, `VersionRepo`, `TestRunRepo`, `SessionRepo`, `AuditRepo`, `JobQueue` |
| `vak-repositories-pg` | sqlx implementations, migrations, `.sqlx/` offline cache, Lakebase credential refresh |
| `vak-adapter-llm-openai` | OpenAI-compatible streaming chat with tools: Databricks FMAPI and local llama-server |
| `vak-adapter-stt-serving` | STT over the Databricks invocations contract; the local shim speaks the same contract |
| `vak-adapter-tts-serving` | TTS over the same contract |
| `vak-adapter-decision-systemone` | System One API: Databricks Unity Gateway and local llama-server |
| `vak-databricks-auth` | OAuth M2M token cache, shared by adapters and repositories-pg |
| `vak-pipeline` | Per-session actor: turn-taking, barge-in, plan execution, decision fan-out, latency budgets |
| `vak-services` | Use cases: agents, plans, versions and publish, test runs, sessions, audit |
| `vak-api` | axum routers, DTOs, validation, RFC 9457 errors, WebSocket handlers, OpenAPI document |
| `vak-config`, `vak-telemetry` | Typed settings with secrets; tracing, OpenTelemetry and metrics setup |
| `vak-testkit`, `vak-contract-tests` | Dev-only: fakes, builders, audio fixtures, Postgres container; HTTP tests against `contracts/openapi.json` |
| `xtask` | `check-layers`, `coverage-gate`, `openapi`, `contract-check` |

**Enforcement** (report 02):
- Each layer is a crate, so a missing dependency fails the compile.
- `xtask check-layers` compares `cargo metadata` against `layers.toml`.
- cargo-deny `wrappers` allows only named crates to depend on sqlx, axum and reqwest.

## 3. Voice pipeline

One actor task per session, with bounded channels between stages. Each channel declares what happens when it's full. Barge-in cancels downstream work with a `CancellationToken`.

```
mic audio ─► decode/resample ─► VAD (Silero) ─► end-of-turn (Smart Turn v3) ─┬─► STT (per utterance)
                                                                             │
           ┌─────────────────────────────────────────────────────────────────┘
           ▼
   transcript ─┬─► DecisionEngine fan-out (branch, intent, opt-out, escalation)  ─┐  in parallel
               └─► LLM (current plan node prompt, streamed) ─► clause chunker ─┐  │
                                                                               ▼  ▼
                                         if a decision says branch / opt-out / escalate: cancel + re-prompt
                                                                               │
                                                     TTS per clause ─► encode ─► speaker
```

- **Plan execution.** The domain plan graph is a state machine. The LLM only sees the current node's prompt and tools. Moving between nodes is decided by the `DecisionEngine` (`choice` over outgoing edges), with an LLM tool call as fallback when confidence is low.
- **Opt-out has three layers.** These are a deterministic phrase list, the decision model (`noul`, recall-biased threshold) and an LLM tool. A timeout never suppresses an opt-out.
- **Rule packs** inject locked text (AI disclosure, recording notice) that the LLM can't change. They also inject hard guardrails, e.g. never claiming to be human.
- **Latency budget.** The target is P50 ≤ 1.2 s and P95 ≤ 2.0 s mouth-to-ear in the browser; the aspiration is ≤ 1.0 s. Each stage records a histogram. Spikes S3–S5 and S8 replace these targets with measured numbers.
- **Determinism in tests.** Use `tokio::time::pause`, recorded audio fixtures and fake engines with scripted timings.

## 4. Frontend

React + Vite SPA, built to static files and served by the Rust app.

```
src/
  app/        bootstrap, providers, error boundary
  routes/     thin TanStack file routes: /agents, /agents/$id/{provision,plan,test,publish}
  features/   agents (Provision) · plans (Plan) · testing (Test) · publishing (Publish)
                each: index.ts (public API) → views/ → state/ → api/
  shared/     api/generated (orval) · ui (shadcn on Base UI) · voice (session port + adapters) · lib
```

- dependency-cruiser enforces the following:
  - No imports between features.
  - Views never call the API directly.
  - Only `shared/ui` imports Base UI.
  - No cycles.
- The API client, Zod schemas and MSW mocks are generated from `contracts/openapi.json`, so the frontend can't drift from the backend.
- The voice session (connect, listen, speak, barge-in, end) is an XState machine. It's tested with a fake microphone.

## 5. Contracts between parts

| Contract | Source of truth | Checked by |
|---|---|---|
| REST + WebSocket API | `contracts/openapi.json`, generated from Rust (utoipa) and committed | Drift check, `oasdiff breaking`, regenerated-client drift, `vak-contract-tests` |
| Model serving | `contracts/serving/{stt,tts}.json` request/response schemas | Adapter contract tests run against the local shim and, in staging, against Databricks |
| Decisions | `contracts/decisions/*.toml`: platform questions (opt-out, escalation, outcome) | Domain unit tests; offline eval set |
| Feature behaviour | Contract tests pinned by signed `contract/<feature>/vN` tags | `contract-check` in CI |

## 6. Deployment

- `deploy/databricks/databricks.yml` is a Declarative Automation Bundle (formerly Asset Bundles). It defines:
  - the app
  - the serving endpoints (stt, tts, sim-caller, and the LLM if self-hosted)
  - the Lakebase database
  - a UC Volume for the app wheel
- There are three targets: `dev`, `staging` and `prod`. Dev and staging endpoints scale to zero; prod keeps STT and TTS warm.
- **Build.** CI cross-compiles `vak-server` for Linux (the architecture is confirmed in spike S1) with the SPA embedded. It then wraps the binary in a Python wheel, which is the only way to put a binary over 10 MB onto Apps, and uploads it to the Volume.
- **Deploy.** GitHub Actions authenticates to Databricks with OIDC (no stored secrets).
  - A merge to `main` deploys to staging.
  - A signed release tag deploys to prod.
- **Migrations.** Run as `vak-server migrate` before the app restarts. Each PR gets a Lakebase branch for integration tests (spike S7).
- **STT/TTS wrappers.** These are MLflow pyfuncs that live in `deploy/databricks/serving/`. They are the only Python in the repo apart from the wheel launcher.
