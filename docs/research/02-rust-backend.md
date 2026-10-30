# 02: Rust backend ecosystem and workspace architecture

**Status:** research. Decisions marked ⚑ need approval. **Verified:** 2026-10-30 against crates.io, docs.rs, GitHub and official docs. Rust stable is **1.99.1**.

## TL;DR

- **Stack:** axum 0.8 + tokio ≥1.54.1, sqlx 0.9 on Postgres 18, and utoipa 6 (code-first OpenAPI) committed to `contracts/openapi.json`, which drives orval on the frontend. For logs and traces, tracing → OpenTelemetry (OTLP).
- **Layering is enforced three ways.** (1) Each layer is its own crate, so code that imports a crate not in its `Cargo.toml` does not compile. (2) An `xtask check-layers` reads `cargo metadata` and rejects any `Cargo.toml` edge not in `layers.toml`. (3) `cargo-deny` `[bans] wrappers` limits who may depend on axum, sqlx, reqwest and tungstenite.
- **Ports are separate from implementations.** `vak-repositories` holds traits only, and `vak-repositories-pg` holds the sqlx code. Likewise `vak-providers` holds traits and `vak-adapter-*` holds the vendor code. Services therefore compile without sqlx, HTTP or vendor SDKs, and unit tests use fakes from `vak-testkit`.
- **Real-time audio design:** one actor task per call, with bounded `mpsc` channels between stages. Each channel edge has an explicit overflow policy, and barge-in uses a `CancellationToken`. Cheap DSP runs inline: μ-law, rubato, and the pure-Rust Earshot VAD at about 5 µs per frame. ONNX models (Silero, turn detection) run on dedicated inference threads.
- **Transport:** start with WebSocket for both the browser test console and the telephony media streams. Add WebRTC later, using webrtc-rs 0.21 (now built on a sans-I/O core) or str0m. We don't need Redis, and no job queue beyond Postgres: apalis-postgres 1.0 sits behind our own `JobQueue` port.
- **One `just verify`** runs fmt, clippy (`-D warnings`), nextest, doctests, llvm-cov (global and per-crate gates), cargo-deny, cargo-audit, cargo-shear, check-layers, `sqlx prepare --check` and gitleaks. The same recipe runs locally, in hooks and in CI.

## Recommendations

| # | Area | Recommendation | Confidence | ⚑ |
|---|------|----------------|------------|---|
| R1 | Web framework | **axum 0.8.9** (tower, hyper 1, tokio-native WebSocket) | High | ⚑ |
| R2 | Runtime | **tokio ≥ 1.54.1** (multi-thread), tokio-util `CancellationToken` and `TaskTracker` | High | |
| R3 | Database | **Postgres 18 + sqlx 0.9**: raw SQL, compile-time checked, `.sqlx/` offline cache committed | High | ⚑ |
| R4 | Migrations | `sqlx migrate`: forward-only SQL files, embedded with `migrate!()`, run by `vak-server migrate` | High | |
| R5 | Jobs | Postgres only. **apalis-postgres 1.0** behind a `JobQueue` port. No Redis in v1. | Medium | ⚑ |
| R6 | OpenAPI | **Code-first utoipa 6 + utoipa-axum**, emitting OpenAPI 3.1 to the committed `contracts/openapi.json`. CI gates on spec drift and `oasdiff`. | High | ⚑ |
| R7 | Audio | `opus` 0.4 (libopus 1.6), rubato 5, G.711 via `audio-codec-algorithms`, Earshot VAD (Silero via `ort` as the alternative) | Medium | |
| R8 | Transport | WebSocket first: axum `ws` server side, tokio-tungstenite for provider clients. WebRTC library chosen in a later milestone. | Medium | ⚑ |
| R9 | Observability | tracing + tracing-opentelemetry 0.34 + OTLP, OTel metrics histograms per stage, tokio-metrics | High | |
| R10 | Config/secrets | `config` 0.15 layered into a typed `Settings`, with `secrecy::SecretString` for every credential | High | |
| R11 | Errors | `thiserror` in every library crate, `anyhow` only in `apps/`, `xtask` and tests. RFC 9457 problem+json at the API. | High | |
| R12 | Testing | nextest, llvm-cov, `#[sqlx::test]` + testcontainers, hand-written fakes (mockall sparingly), wiremock, insta, proptest, cargo-mutants (`--in-diff`), criterion + gungraun | High | |
| R13 | Supply chain | cargo-deny (primary), cargo-audit (binary scan via cargo-auditable), cargo-shear. cargo-vet deferred. | High | |
| R14 | Build/tasks | **just**, cargo-chef, `gcr.io/distroless/cc-debian13:nonroot` | High | |

## Comparison tables

### Web framework

| | **axum 0.8.9** | actix-web 4.18.1 | poem 3.1.12 |
|---|---|---|---|
| Last release | 2026-04 (main active, bumps tungstenite to 0.30) | 2026-10 | 2025-07 |
| Runtime model | Plain tokio multi-thread; handlers are `Send` | Per-worker single-threaded runtimes (actix-rt) | tokio |
| Middleware | tower/tower-http, shared with reqwest/tonic | Own middleware system | Own |
| WebSocket | Built in (`ws` feature, tokio-tungstenite) | `actix-ws` | Built in |
| OpenAPI | utoipa-axum, aide | utoipa-actix-web | poem-openapi (built in) |
| Fit here | Same runtime and task model as the voice pipeline, so moving to the pipeline is just a `tokio::spawn` | Fast, but a second concurrency model | Smaller community, slower releases |

Raw framework throughput is irrelevant against a voice budget of about 500–800 ms dominated by STT, LLM and TTS. What matters is ecosystem fit and a single concurrency model.

### Database access

| | **sqlx 0.9.0** | SeaORM 2.0.4 | diesel 2.3.14 + diesel-async 0.9.2 |
|---|---|---|---|
| Query style | Raw SQL with macros (`query!`, `query_as!`) | ORM entities + query builder (on sqlx 0.9) | Type-safe DSL |
| Compile-time checking | Against a live DB or `.sqlx/` offline cache | No (runtime) | Yes, via the type system with generated `schema.rs` |
| Async | Native | Native | Separate crate (`diesel-async`) |
| Migrations | `sqlx migrate` / `migrate!()` | sea-orm-migration (Rust or SQL); 2.0 adds entity-first schema sync | `diesel_migrations` |
| Notes | 0.9: repo moved to `transact-rs`. Adds per-crate `sqlx.toml`. `SqlSafeStr` rejects `format!`-built SQL. MSRV 1.94. | Fastest for CRUD; another abstraction for reviewers to learn | 2026 advisories are SQLite/MySQL-only; steep learning curve |

### OpenAPI

| | **utoipa 6.0 + utoipa-axum 0.3** | aide 0.15.1 | poem-openapi 5.1.16 | Spec-first (openapi-generator `rust-axum`) |
|---|---|---|---|---|
| Activity | 6.0 released 2026-09; 3.2 opt-in | Last release 2025-08 | Last release 2025-07 | Generator is **BETA** |
| Route/spec sync | `OpenApiRouter` + `routes!` register both together | Typed routing produces the docs | Inherent | Generated traits |
| Ecosystem | Largest; Swagger UI, Scalar | schemars-based | Tied to poem | Mustache templates |
| Contract-first TDD | Stubs + DTOs → spec → failing tests (see §4) | Same | Same | Natural, but Rust codegen is weak and drift is hard to police |

### Real-time transport and audio

| Need | Pick | Alternatives | Notes |
|---|---|---|---|
| WebSocket server | axum `ws` | actix-ws | Browser test console, Twilio/Telnyx-style media streams |
| WebSocket client (providers) | tokio-tungstenite **0.29** (match axum) | tokio-websockets 0.13 (SIMD); fastwebsockets 0.10 (stale since 2025-01) | axum 0.8.9 pins `^0.29` while the latest is 0.30, so align to avoid duplicates |
| WebRTC | Defer. webrtc-rs **0.21.1** (sans-I/O `rtc` core, heading to 1.0) or **str0m 0.24** (sans-I/O, minimal) | LiveKit Rust SDK 0.10 (libwebrtc C++ build, needs a LiveKit SFU) | LiveKit Agents is Python/Node only |
| Opus | `opus` 0.4.0 (libopus 1.6 via `opusic-sys`, needs cmake) | `opus-rs` 0.1.40 (restsend, pure Rust, young); `audiopus` stale (2019) | |
| Resampling | `rubato` 5.0.1 (`process_into_buffer` does not allocate) | soxr bindings | v5 uses `audioadapter` traits |
| G.711 μ-law/A-law | `audio-codec-algorithms` 0.8.1 (no_std, no unsafe, no alloc) | 40-line in-house LUT | Proptest round-trip either way |
| VAD | **Earshot 1.2.2**: pure Rust, MIT/Apache, 16 ms frames at 16 kHz, vendor-claimed RTF 0.0003 | Silero VAD v6 via `ort` 2.0.0-rc.15 (8/16 kHz, <1 ms per 30 ms chunk); `voice_activity_detector` 0.2.1; `webrtc-vad` (2019, avoid) | Benchmark both on 8 kHz telephony audio |
| WAV fixtures | `hound` 3.5.1 | `symphonia` 0.6.1 (multi-format) | |

### Supply-chain and quality tools

| Tool | Version | Role | Where |
|---|---|---|---|
| cargo-deny | 0.20.2 | advisories, licenses, bans (+ `wrappers`), sources | verify + CI |
| cargo-audit | 0.22.2 | RustSec; `cargo audit bin` on release binaries | CI release job |
| cargo-auditable | 0.7.7 | Embeds the dependency list in binaries, so Trivy/Grype/osv-scanner can read it | Docker build |
| cargo-shear | 1.14.0 | Unused and misplaced deps, unlinked files (stable toolchain, fast) | verify |
| cargo-machete / cargo-udeps | 0.9.2 / 0.1.61 | Regex heuristic / precise but nightly-only | Optional |
| cargo-vet | 0.10.3 | Audit attestations (import Mozilla/Google/Bytecode Alliance) | Later |
| cargo-mutants | 27.1.0 | Mutation testing, `--in-diff` on PRs | Nightly + PR (domain/services) |
| just / cargo-make | 1.59.0 / 0.37.24 (2025-01) | Task runner | **just** |
| cargo-chef | 0.1.78 | Docker dependency-layer cache | Dockerfile |

## Proposed workspace

The tree mirrors `frontend/` and shares `contracts/` with the frontend report.

```
voice/
├── contracts/openapi.json            # generated by `just openapi`, committed; orval input; drift-gated
├── backend/
│   ├── Cargo.toml                    # virtual manifest: resolver="3", [workspace.package|dependencies|lints]
│   ├── Cargo.lock                    # committed (application)
│   ├── rust-toolchain.toml           # channel="1.99.1", components=[rustfmt, clippy, llvm-tools-preview]
│   ├── rustfmt.toml  clippy.toml  deny.toml  layers.toml  coverage.toml  sqlx.toml
│   ├── .config/nextest.toml          # profiles: default, ci (junit, no retries), sim (pipeline sims)
│   ├── justfile                      # verify, verify-fast, openapi, db-prepare, bench, mutants
│   ├── crates/
│   │   ├── domain/                   # vak-domain: entities, value objects, IDs, state machines, rules. No I/O.
│   │   ├── audio/                    # vak-audio: AudioFrame, codecs (μ-law, Opus), resample, Vad trait+impls
│   │   ├── repositories/             # vak-repositories: persistence PORTS (traits) + RepoError. No sqlx.
│   │   ├── repositories-pg/          # vak-repositories-pg: sqlx impls, migrations/, .sqlx/
│   │   ├── providers/                # vak-providers: STT/TTS/LLM/Telephony PORTS + stream event types
│   │   ├── adapters/                 # one crate per vendor (names follow provider research)
│   │   │   ├── stt-<vendor>/  tts-<vendor>/  llm-<vendor>/  telephony-<vendor>/
│   │   ├── pipeline/                 # vak-pipeline: per-call session actor, turn-taking, barge-in, budgets
│   │   ├── services/                 # vak-services: agents, plans, versions/publish, test runs, campaigns, calls
│   │   ├── api/                      # vak-api: axum routers, DTOs, validation, problem+json, WS, OpenAPI doc
│   │   ├── config/                   # vak-config: typed Settings, secrecy, layered loading, validation
│   │   ├── telemetry/                # vak-telemetry: subscriber/OTel/metrics init, metric names & buckets
│   │   ├── testkit/                  # vak-testkit (dev-only): fakes, builders, audio fixtures, PG container
│   │   └── contract-tests/           # vak-contract-tests (dev-only): HTTP tests vs contracts/openapi.json
│   ├── apps/server/                  # vak-server bin: serve | worker | migrate | openapi  (composition root)
│   ├── fixtures/audio/               # short WAVs: 8 kHz μ-law calls, 16 kHz PCM, barge-in, silence, noise
│   └── xtask/                        # check-layers, coverage-gate, openapi export, contract-tag check
└── frontend/ …
```

Every folder above gets a `CLAUDE.md`, as the brief requires.

### Allowed internal dependencies (`layers.toml`)

| Crate | May depend on (normal deps) | Must never depend on |
|---|---|---|
| vak-domain | none. External allowlist: `thiserror`, `uuid`, `jiff`, `serde` (derive) | tokio, sqlx, axum, reqwest, any adapter |
| vak-audio | none (rubato, opus, earshot, `ort` behind a feature) | tokio net, sqlx, axum |
| vak-repositories | domain | sqlx, services, api |
| vak-providers | domain, audio | reqwest, tungstenite, sqlx |
| vak-repositories-pg | repositories, domain | services, api, providers |
| vak-adapter-* | providers, domain, audio | repositories, services, api, other adapters |
| vak-pipeline | providers, domain, audio | repositories*, api, adapters, axum, sqlx |
| vak-services | repositories, providers, pipeline, domain | repositories-pg, adapters, api, axum, sqlx |
| vak-api | services, domain | repositories(-pg), providers, pipeline, adapters, sqlx |
| vak-config, vak-telemetry | none | everything internal |
| vak-server | all (composition root: the only place implementations are wired) | — |
| vak-testkit, vak-contract-tests | domain, audio, repositories, providers (+ api for contract tests); **dev-dependency only** | appearing as a normal dep |

\*The pipeline persists transcripts and events through a service-provided sink, not through repositories directly. Services re-export the few port types the API needs, such as a `MediaTransport` trait for WebSocket bridging, so the API depends only on services.

### Enforcement: three independent nets

1. **Crate boundaries (compile time).** Code cannot `use` a crate that isn't in its `Cargo.toml`. Default to `pub(crate)`, and enable rustc's `unreachable_pub` lint so crate APIs stay narrow.
2. **`xtask check-layers` (CI and pre-commit).** About 100 lines using `cargo_metadata` 0.23. It walks every workspace package's dependencies (normal, dev, build) and fails on any internal edge not allowed in `layers.toml`. It also fails on any domain external dependency outside the allowlist, and on testkit used as a non-dev dependency. Messages name the offending `Cargo.toml` line. Add CODEOWNERS on `layers.toml`.
3. **`cargo-deny` bans with `wrappers`.** These are a framework firewall: `wrappers` allows only the listed crates to depend directly on the banned crate.
   ```toml
   [bans]
   multiple-versions = "deny"
   deny = [
     { crate = "sqlx",   wrappers = ["vak-repositories-pg", "apalis-postgres"], reason = "persistence only in repositories-pg" },
     { crate = "axum",   wrappers = ["vak-api", "vak-server", "utoipa-axum", "axum-extra"], reason = "HTTP only in api" },
     { crate = "reqwest", wrappers = ["vak-adapter-stt-x", "..."], reason = "HTTP clients only in adapters" },
     { crate = "openssl", reason = "use rustls" },
   ]
   [bans.workspace-dependencies]   # versions declared once in [workspace.dependencies]
   duplicates = "deny"
   unused = "deny"
   ```
   - `cargo-modules` 0.28 (`dependencies`/`orphans`) and DataDog's **`cargo-pup`** 0.1.9 (ArchUnit-style module and trait rules) can also enforce rules *inside* a crate. pup requires a pinned nightly, so treat it as optional.
   - Graphify's layer checks can consume the same `layers.toml`.

### Workspace manifest essentials

```toml
[workspace]
resolver = "3"                       # edition-2024 default for packages; virtual workspaces must set it
members = ["crates/*", "crates/adapters/*", "apps/*", "xtask"]
[workspace.package]
edition = "2024"
rust-version = "1.99"                # = pinned toolchain (application); resolver v3 then prefers compatible deps
publish = false
[workspace.lints.rust]
unsafe_code = "forbid"
unreachable_pub = "warn"
[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
module_name_repetitions = "allow"
missing_errors_doc = "allow"
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
todo = "deny"
dbg_macro = "deny"
print_stdout = "deny"
allow_attributes_without_reason = "deny"
large_futures = "warn"
cast_possible_truncation = "warn"   # audio math: force explicit, documented casts
```

- **Lint inheritance:** every member sets `[lints] workspace = true`. Cargo still **cannot override inherited lints per package** (cargo#13157 is open). Use crate-level `#![expect(lint, reason = "...")]` where needed.
- **Why `forbid` works:** `forbid` cannot be relaxed, which is fine because our code needs no `unsafe`. The FFI lives inside `opus` and `ort`.
- **Warnings as errors:** CI runs clippy with `-D warnings`.
- **MSRV:** for an application, MSRV equals the pinned toolchain. Bump it deliberately with Dependabot or Renovate. Several dependencies already need ≥1.92–1.94: sqlx and SeaORM 1.94, ort 1.92, cargo-modules 1.98.

## 1. Web framework

Use axum: it runs on tokio, uses tower middleware, and has strong OpenAPI support. Keep `vak-api` thin:

- **Extract and validate** with `garde` 0.23 or `validator` 0.21.
- **Map DTO ↔ domain.** DTOs live only in `vak-api`, never in domain.
- **Call a service, then map errors** to problem+json.
- **tower-http** provides tracing, request IDs, timeouts, CORS, compression and size limits.
- **Router tests** use `tower::ServiceExt::oneshot` or `axum-test` 21.

## 2. Real-time audio on tokio

- **Task design.** Each call has one `CallSession` actor task that owns the turn state machine. It `select!`s over five event sources:
  - inbound frames
  - VAD/STT events
  - LLM tokens
  - TTS audio
  - control: hangup, barge-in, timers

  Stage I/O runs in child tasks (transport rx/tx, STT stream, LLM stream, TTS stream), tracked by a `TaskTracker`. This follows the actor pattern from Alice Ryhl's "Actors with Tokio".
- **Frames.** 20 ms mono PCM16 `AudioFrame` (320 samples at 16 kHz) in reused buffers (`bytes::BytesMut` or fixed arrays). Don't allocate, `format!` or open a tracing span per frame.
- **Bounded channels with an explicit overflow policy per edge.**

  | Edge | Policy |
  |---|---|
  | Transport rx → VAD/STT | Capacity about 10 frames. On full, `try_send` and drop the frame, increment `frames_dropped`. Never stall the socket reader; stale audio is worthless. |
  | TTS → tx pacer | Await `send`. Backpressure is fine because TTS runs faster than real time. |
  | Session state | `watch` channel |
  | Barge-in | Child `CancellationToken` per turn cancels the LLM and TTS tasks. Frames carry a turn/generation id, so the pacer drops stale frames in O(1). It then sends the transport's "clear" message. |
- **Pacing out.** Use `tokio::time::interval(20ms)` with `MissedTickBehavior::Delay`, keeping 2–3 frames ahead of real time. **tokio 1.54.0 had a use-after-free race in `Interval` (RUSTSEC-2026-0384)**, so require **≥ 1.54.1**.
- **No blocking.** Allow at most about 10–100 µs between `.await`s (Ryhl).
  - **Inline is fine:** μ-law decoding (ns), rubato on a 20 ms chunk (µs), Earshot (≈5 µs per 16 ms frame at the claimed RTF).
  - **Move to dedicated threads:** ONNX inference (Silero <1 ms per chunk, turn-detection models), noise suppression and any local ASR. Use a fixed pool of `std::thread` workers sized to cores, fed by `crossbeam-channel`/`flume`, or an `rtrb` single-producer/single-consumer ring per call. Results return via tokio `oneshot`/`mpsc`. Set ort sessions to `intra_op_threads = 1` to avoid oversubscription.
  - **`spawn_blocking`** is for occasional work, such as model loading and file I/O, not steady per-frame streams.
- **Isolation (option, medium confidence).** Run a separate tokio runtime for media, so API spikes and DB-pool waits can't delay frame pacing. Decide after load tests using `tokio-metrics` 0.5 (poll times, scheduled delay). Use `tokio-console` in development only.
- **Async ports.** Prefer native `async fn` in traits with generics in services. Where `dyn` is needed, for example to pick a provider per agent at runtime, use `dynosaur` 0.3 or `async-trait`. Streaming providers return channel-based session handles: `SttSession { audio_tx, events_rx }` is dyn-friendly, cancellable and easy to fake.

## 3. Database

- **sqlx 0.9 with Postgres 18.6** (19 is at RC1).
  - Use JSONB for immutable published-version snapshots, `FOR UPDATE SKIP LOCKED` for job claims, advisory locks for per-campaign concurrency, and `LISTEN/NOTIFY` for wake-ups.
  - Generate UUIDv7 IDs in the domain (`uuid` v7 feature). PG18's native `uuidv7()` is the DB-side default.
- **Compile-time checks in CI.**
  1. `cargo sqlx prepare --workspace` writes `.sqlx/` (one file per query). Commit it.
  2. Build, lint and test jobs set `SQLX_OFFLINE=true`, and so do Claude hooks and the Docker build, which copies `.sqlx`. No database is needed to compile.
  3. One DB-backed CI job runs migrations and then `cargo sqlx prepare --workspace --check`, so a stale cache fails the merge.
  4. `just db-prepare` starts Compose Postgres for local regeneration.
- **sqlx 0.9 changes that matter here:**
  - per-crate `sqlx.toml` (type overrides, renamed `DATABASE_URL`)
  - `SqlSafeStr`: dynamic SQL must be wrapped in `AssertSqlSafe`, which makes it easy to spot in security review
  - MSRV 1.94
- **Migrations.** Use forward-only `migrations/<timestamp>_<name>.sql` in `vak-repositories-pg`, embedded with `sqlx::migrate!()`. Run them with `vak-server migrate` as a deploy step. Auto-migrate only in dev Compose.
- **Why not SeaORM or diesel.** SeaORM 2 is productive, but its queries are checked at runtime and it adds a second abstraction. Diesel has the strongest typing, but it's sync-first, async lives in a separate crate, and the DSL is costly for reviewer agents. Both are viable if you prefer them.
- **Redis or a job queue? A durable queue, yes; Redis, no.**
  - **What campaigns need:** `run_at` scheduling (calling windows per recipient timezone), retries with backoff, max attempts, per-campaign and per-trunk concurrency and calls-per-second caps, and crash recovery. Postgres covers all of this.
  - **Choice:** **apalis-postgres 1.0** (2026-10-12, on sqlx 0.9). It provides `run_after`, priority, heartbeats with orphan re-enqueue, NOTIFY-based storage, tower middleware and apalis-board. Wrap it in our `JobQueue` port so pacing and compliance rules stay in services, which keeps them testable.
  - **Alternatives:** `graphile_worker` 0.14.1, or about 200 lines of SKIP LOCKED.
  - **When Redis would be needed:** multi-node pub/sub fan-out at scale, sub-millisecond distributed rate limiting, or hot ephemeral state. None of these apply in v1.

## 4. OpenAPI and contracts

- **Code-first is the better fit.**
  - The Rust types *are* the implementation, so the spec can't silently drift.
  - `utoipa-axum`'s `OpenApiRouter` registers route and doc together.
  - Emit **OpenAPI 3.1**, utoipa's default. 3.2 is opt-in until orval and the other generators support it.
- **Contract-first TDD with code-first:**
  1. Write DTOs and handler signatures (stubs returning 501) with `#[utoipa::path]`.
  2. Run `just openapi`, which writes `contracts/openapi.json`.
  3. Write failing contract tests (Rust and TS).
  4. Commit and tag `contract/<feature>/v1`.
  5. Implement until green. The spec diff appears in the PR for review.
- **CI gates:**
  - regenerated spec ≠ committed spec → fail
  - `oasdiff breaking` against `main` → fail unless a new contract version is approved
  - orval output drift → fail (frontend)
- **Shared contract tests:**
  - `vak-contract-tests` boots the app with testkit fakes and validates every response against the committed spec's schemas, using the `jsonschema` crate 0.60.
  - **Schemathesis** v4.32 fuzzes the running server from the same file in CI.
  - The frontend gets types, Zod schemas and MSW mocks from the same file through orval.
- **WebSocket messages.** OpenAPI doesn't describe them. Register the WS message enums as schema components (utoipa `ToSchema`) so TS gets the types, and consider AsyncAPI later.
- **Rejected alternatives:**
  - *Spec-first* (hand-written YAML plus openapi-generator `rust-axum`, still BETA) has weaker Rust codegen and needs two sources of truth reconciled.
  - **Dropshot API traits** (Oxide) are the cleanest Rust contract-first design, but they require Dropshot instead of axum.

## 5. Audio crates

- **Telephony path:** μ-law 8 kHz → decode to PCM16 → rubato to 16 kHz for VAD/STT. The reverse runs for TTS: request the provider's native 8 kHz μ-law where possible, which avoids resampling.
- **Opus.**
  - **Browser and WebRTC:** use Opus 48 kHz.
  - **`opus` 0.4:** now builds libopus 1.6 through `opusic-sys`. It needs cmake in the build image.
  - **Pure-Rust `opus-rs`:** worth watching, since it would remove the C toolchain.
- **VAD.**
  - **Interface:** keep it behind `trait Vad { fn push(&mut self, frame) -> VadEvent }` so implementations can be swapped and the pipeline tests stay deterministic.
  - **Earshot (default):** inline and pure Rust. Its accuracy claim (better than Silero v6) is the vendor's own, so verify it on our fixtures.
  - **Silero v6 via `ort`:** native 8 kHz support.
- **`ort` caveats.**
  - **Still a release candidate:** `ort` is at 2.0.0-rc.15 after years of RCs, so pin it with `=`.
  - **Build-time downloads:** by default it downloads prebuilt ONNX Runtime binaries at build time, a supply-chain gap that cargo-deny `sources` doesn't cover. For hermetic Docker builds, use `ORT_LIB_PATH` or `load-dynamic`.
- **WebRTC.**
  - **Recommendation:** don't build it in M1.
  - **When to add it:** later, for the public web widget, where packet loss and jitter matter. Browser echo cancellation works over WebSocket too, through `getUserMedia` constraints.
  - **Options:**
    - webrtc-rs 0.21: a full W3C-style API on the sans-I/O `rtc` core, with virtual-time support for tests and pluggable crypto.
    - str0m: a small sans-I/O library that is easy to drive from our own task.
    - LiveKit: rooms, an SFU and SIP, at the cost of extra infrastructure and a C++ libwebrtc build. Its agent framework doesn't support Rust.

## 6. Observability

- **Logs.** `tracing` 0.1.44 + `tracing-subscriber` 0.3.23 (`EnvFilter`): JSON in production, pretty output in development.
- **Traces.** `tracing-opentelemetry` 0.34 + `opentelemetry`/`opentelemetry_sdk`/`opentelemetry-otlp` 0.33 send OTLP to a Collector. For local Compose, use the single `grafana/otel-lgtm` container (0.39). OTel Rust status: metrics and logs SDKs are **Stable**; traces are **Beta**.
- **Span shape.**
  - Spans go `call` → `turn` → `stage` (`vad`, `stt`, `llm`, `tts`, `transport`), with attributes `call_id`, `agent_version_id`, `provider`, `model`.
  - **No per-frame spans.**
  - A pure domain `TurnTimeline` records stage timestamps (`user_speech_end`, `stt_final`, `llm_first_token`, `tts_first_byte`, `first_audio_out`). Metrics are derived from it at turn end, so the timing logic can be unit-tested with paused time.
- **Histograms** (OTel metrics API, explicit buckets in ms: 10, 25, 50, 75, 100, 150, 200, 300, 400, 500, 750, 1000, 1500, 2000, 3000, 5000):
  - `voice.turn.latency`: end of user speech → first agent audio
  - `voice.stage.latency{stage}`
  - `voice.llm.ttft`, `voice.tts.ttfb`
  - `voice.frames.dropped`, `voice.queue.depth`, `voice.pacer.lateness`
  - Exemplars link slow turns to their traces.
  - The `metrics` 0.24 + Prometheus exporter stack is a lighter alternative, but it means running two pipelines.
- **Runtime health.** `tokio-metrics` task and runtime metrics.
- **PII.** Never log transcripts or audio above `debug`. Wrap transcript text in a redacting newtype, and keep secrets in `SecretString`, which prints `[REDACTED]`.

## 7. Config, secrets, errors

- **Config.**
  - `vak-config` loads defaults, then `config/{env}.toml`, then env vars (`VAK__DB__URL`), into a typed `Settings` struct. Validate at startup and fail fast.
  - **Only `vak-server` reads the environment.** Inner crates receive typed config structs.
  - `config` 0.15.27 is active. `figment` hasn't released since 2024-05, and `confique` is a small alternative.
- **Secrets.**
  - **Typing:** every credential is a `secrecy::SecretString` (0.10.3, `serde` feature). `Debug` is redacted and the value is zeroized on drop.
  - **Exposure:** call `expose_secret()` only inside the adapter that builds the auth header, and mark the header with `HeaderValue::set_sensitive(true)`.
  - **Sources:** env or Docker secrets locally, a cloud secret manager later. gitleaks runs in verify.
- **Errors per layer:**

  | Layer | Convention |
  |---|---|
  | domain | `thiserror` enums of rule violations; no I/O error types |
  | repositories | `RepoError { NotFound, Conflict, Unavailable(#[source] BoxError) }`; sqlx types never cross the port |
  | providers/adapters | `ProviderError { Timeout, RateLimited, Auth, Protocol, Unavailable }` with a retryability flag |
  | services | `ServiceError` per use case, built from domain, repo and provider errors |
  | api | `ApiError: IntoResponse` producing RFC 9457 `application/problem+json`, logs the source chain once |
  | binaries, xtask, tests | `anyhow` |

  Never use `unwrap`, `expect` or `panic` outside tests; clippy enforces this, with `allow-unwrap-in-tests`.

## 8. Workspace

See the [Proposed workspace](#proposed-workspace) section above.

## 9. Testing stack

| Layer | Approach |
|---|---|
| domain | Unit tests + **proptest** 1.12 (state machines, schedule windows, μ-law round-trip). Coverage gate 95%. |
| repositories-pg | **`#[sqlx::test]`**: a fresh database per test with migrations and fixtures applied. Postgres comes from **testcontainers** 0.28 (`testcontainers-modules` postgres) locally, or a CI service container. One *port conformance suite* runs against both the PG implementation and the in-memory fake. |
| services | Hand-written **fakes** from `vak-testkit` (in-memory repos, scripted providers). **mockall** 0.15 only for "must not call X" interaction checks. |
| adapters | **wiremock** 0.6.5 for REST (no WebSocket support, last release 2025-08); a testkit fake WebSocket server (tokio-tungstenite) replays recorded provider sessions; **insta** 1.49 snapshots of request bodies. Live-vendor smoke tests run in an `#[ignore]` nextest profile, never in verify. |
| api | `oneshot`/axum-test router tests; insta snapshot of `openapi.json`; `vak-contract-tests` |
| pipeline | Deterministic simulation (below) + benchmarks |
| e2e | A "caller bot" streams fixture WAVs over WebSocket into the Compose stack and asserts transcripts and events |

**Deterministic pipeline tests:**

- Run under `#[tokio::test(flavor = "current_thread", start_paused = true)]`.
- A fake transport feeds WAV fixtures (`hound`) as 20 ms frames on a virtual-time interval. Scripted fake STT, LLM and TTS emit events after virtual delays, for example "final transcript at +180 ms" or "first TTS byte at +120 ms".
- Tests assert on the session's **event timeline**, snapshotted with insta: barge-in cancels TTS within one frame, end-of-turn fires after the configured silence, and the stage budgets are met.

Gotchas:

- **Auto-advance:** paused time auto-advances whenever the runtime is idle. It pauses only while `spawn_blocking` tasks run, *not* while waiting on our own OS threads. In simulations, inline the VAD/inference stage behind its trait, or the clock will jump nondeterministically.
- **Virtual vs wall time:** virtual-time tests prove *logic* latency. CPU cost per frame is measured separately:
  - **criterion** 0.8.2 (maintained again under `criterion-rs`) runs locally.
  - **gungraun** 0.20 (formerly iai-callgrind) counts instructions, deterministically enough to gate CI.
  - **divan** 0.1.21 hasn't released since 2025-04.

**Runners and gates:**

- **nextest** 0.9.150 with a `ci` profile and JUnit output. Doctests still need `cargo test --doc`, and setup scripts are still experimental.
- **Coverage.** `cargo llvm-cov nextest --no-report` runs once, then `report --json --summary-only`. `xtask coverage-gate` applies per-crate thresholds from `coverage.toml`, such as domain 95, services 90, api/pipeline 85 and adapters 70, plus a global `--fail-under-lines 80`. Exclude `apps/`, `xtask/` and `testkit/` with `--ignore-filename-regex`.
- **cargo-mutants.** Run it nightly in full, and on PRs only for changed code with `--in-diff`, scoped to domain and services.

## 10. Quality and security tooling

- **rustfmt.** Use only stable options (`edition`, `style_edition = "2024"`, `max_width = 100`, `use_field_init_shorthand`, `use_try_shorthand`). `imports_granularity` and `group_imports` are nightly-only, so skip them to keep one toolchain.
- **clippy.** Lint set as in the workspace manifest above. Add `clippy.toml` entries:
  - `allow-unwrap-in-tests`, `allow-expect-in-tests`
  - `disallowed-methods`: `std::thread::sleep` and blocking `std::fs::*` in async crates, `tokio::task::block_in_place`
  - `disallowed-types`: `std::sync::Mutex` in pipeline/services, in favor of `parking_lot` or a message passing redesign

  `await_holding_lock` and `let_underscore_future` are on by default.
- **cargo-deny vs cargo-audit.** cargo-deny covers advisories (the same RustSec DB), licenses (allowlist: MIT, Apache-2.0, BSD-2/3, ISC, Unicode-3.0, Zlib, MPL-2.0 reviewed), bans and sources (crates.io only, no git deps without an allowlist). Set `unmaintained = "workspace"` and give every `ignore` an expiry. Keep cargo-audit for `cargo audit bin` on the auditable release binary, as the brief lists both.
- **cargo-vet.** Defer until after M1; it adds a per-dependency audit burden.
- **Unused deps.** cargo-shear in verify (stable, fast, also catches misplaced dev-deps). cargo-udeps needs nightly.
- **Task runner.** **just** 1.59 is very active, polyglot (Rust and pnpm) and readable by agents. cargo-make's last release was 2025-01.
  - `verify-fast` (post-edit hook): fmt on changed files + `clippy -p <changed crates>`.
  - `verify`: everything, run in CI and pre-push.
- **Container.**
  - **Planner, cook, build:** cargo-chef planner → `cargo chef cook --release` → `cargo auditable build --release` with `SQLX_OFFLINE=true`.
  - **Runtime image:** `gcr.io/distroless/cc-debian13:nonroot` (glibc + libgcc, nonroot).
  - **Builder image:** `rust:1.99.1-trixie` with cmake for libopus.
  - **libstdc++:** if `ort` is dynamically linked, it needs `libstdc++` (not in `cc`). Prefer the static build or copy the `.so` in.
  - **Allocator:** benchmark `mimalloc` against glibc malloc before adopting it.

## Risks and open questions

| Risk / question | Impact | Mitigation |
|---|---|---|
| Fresh majors everywhere: sqlx 0.9 (May), utoipa 6 (Sep), apalis 1.0 (Oct 12), SeaORM 2 | Medium | Pin exact versions in `[workspace.dependencies]`, use a Dependabot weekly group, and run a skeleton spike first |
| `ort` still an RC and downloads binaries at build time | Medium | Keep it behind a feature and the `Vad` trait; Earshot (pure Rust) is the default |
| axum 0.8.9 (tungstenite 0.29) vs latest tokio-tungstenite 0.30 | Low | Align to 0.29 until the next axum release; `multiple-versions = "deny"` catches drift |
| Tokio Interval UAF in 1.54.0 | High if hit | Require `tokio = ">=1.54.1"`; cargo-deny advisories |
| Earshot accuracy on 8 kHz noisy telephony audio is unproven (vendor benchmark) | Medium | A/B against Silero on recorded fixtures before choosing |
| Paused-time tests misbehave if work runs on our OS threads | Medium | Inline fakes in simulations; real threads only in benchmarks and e2e |
| A single runtime may let API load hurt frame pacing | Medium | tokio-metrics in load tests; split runtimes if pacer lateness p99 > 5 ms |
| sqlx offline cache friction for agents and hooks | Low | `SQLX_OFFLINE=true` everywhere; `just db-prepare`; CI `--check` |
| Cargo can't override inherited lints per crate | Low | Crate-level `#![expect(..., reason)]` |

Open questions for the user:

1. ⚑ Approve axum, sqlx/Postgres 18 and code-first utoipa.
2. ⚑ One binary with subcommands (`serve`/`worker`/`migrate`) or separate server and worker images?
3. ⚑ Use apalis-postgres or a hand-rolled SKIP LOCKED queue?
4. ⚑ When does WebRTC enter the roadmap, and do you have a preference for LiveKit infrastructure vs self-hosted (webrtc-rs/str0m)?
5. Should cargo-vet be adopted now or after M1?
6. Are the per-crate coverage thresholds above acceptable?

## Sources

Versions: crates.io API, queried 2026-10-30, for every crate named here (`https://crates.io/api/v1/crates/<name>`). Rust stable: https://static.rust-lang.org/dist/channel-rust-stable.toml

- **Web:** https://github.com/tokio-rs/axum · https://github.com/tokio-rs/axum/blob/main/axum/Cargo.toml (tungstenite 0.30 bump on main) · https://github.com/actix/actix-web · https://github.com/poem-web/poem
- **tokio:** https://docs.rs/tokio/latest/tokio/time/fn.pause.html · https://github.com/rustsec/advisory-db/blob/main/crates/tokio/RUSTSEC-2026-0384.md · https://github.com/rustsec/advisory-db/blob/main/crates/tokio/RUSTSEC-2026-0386.md
- **Concurrency (Alice Ryhl):** https://ryhl.io/blog/async-what-is-blocking/ · https://ryhl.io/blog/actors-with-tokio/
- **sqlx:** https://github.com/transact-rs/sqlx/blob/main/CHANGELOG.md · https://docs.rs/sqlx/latest/sqlx/attr.test.html
- **ORMs and queues:** https://www.sea-ql.org/blog/2025-10-30-sea-orm-2.0/ · https://github.com/SeaQL/sea-orm · https://github.com/weiznich/diesel_async · https://github.com/rustsec/advisory-db/tree/main/crates/diesel · https://github.com/apalis-dev/apalis-postgres · https://github.com/leo91000/graphile_worker_rs
- **Postgres:** https://www.postgresql.org/about/news/postgresql-18-released-3142/ (`uuidv7()`) · https://www.postgresql.org/versions.json
- **OpenAPI and contracts:** https://github.com/juhaku/utoipa/blob/master/utoipa/CHANGELOG.md · https://github.com/tamasfe/aide · https://docs.rs/dropshot/latest/dropshot/attr.api_description.html · https://openapi-generator.tech/docs/generators/rust-axum/ · https://github.com/schemathesis/schemathesis · https://github.com/oasdiff/oasdiff
- **WebRTC:** https://github.com/webrtc-rs/webrtc · https://webrtc.rs/blog/2026/01/31/async-friendly-webrtc-architecture.html · https://github.com/algesten/str0m · https://github.com/livekit/rust-sdks
- **Audio:** https://github.com/pykeio/ort · https://ort.pyke.io/setup/linking · https://github.com/pykeio/earshot · https://github.com/snakers4/silero-vad · https://github.com/HEnquist/rubato · https://github.com/SpaceManiac/opus-rs · https://github.com/restsend/opus-rs · https://github.com/karip/audio-codec-algorithms
- **WebSocket clients:** https://github.com/snapview/tokio-tungstenite · https://github.com/Gelbpunkt/tokio-websockets
- **Observability:** https://github.com/open-telemetry/opentelemetry-rust (status table) · https://github.com/tokio-rs/tracing-opentelemetry
- **Config, secrets, async traits:** https://github.com/iqlusioninc/crates/tree/main/secrecy · https://github.com/rust-cli/config-rs · https://github.com/spastorino/dynosaur
- **Cargo and lints:** https://doc.rust-lang.org/cargo/reference/workspaces.html · https://doc.rust-lang.org/cargo/reference/resolver.html · https://github.com/rust-lang/cargo/issues/13157 · clippy groups verified with `clippy-driver -W help` (clippy 0.1.93)
- **Architecture checks:** https://github.com/EmbarkStudios/cargo-deny/blob/main/docs/src/checks/bans/cfg.md · https://github.com/EmbarkStudios/cargo-deny/blob/main/docs/src/checks/advisories/cfg.md · https://github.com/DataDog/cargo-pup · https://github.com/regexident/cargo-modules
- **Testing:** https://nexte.st/docs/configuration/setup-scripts/ · https://nexte.st/docs/running/ (doctests) · https://github.com/taiki-e/cargo-llvm-cov · https://mutants.rs/in-diff.html · https://github.com/gungraun/gungraun · https://github.com/criterion-rs/criterion.rs
- **Dependency hygiene:** https://github.com/Boshen/cargo-shear · https://github.com/bnjbvr/cargo-machete · https://github.com/rust-secure-code/cargo-auditable · https://github.com/mozilla/cargo-vet
- **Build:** https://github.com/LukeMathWalker/cargo-chef · https://github.com/GoogleContainerTools/distroless · https://github.com/casey/just · https://github.com/sagiegurari/cargo-make
