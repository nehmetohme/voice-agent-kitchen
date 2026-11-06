# vak-server: composition root

## Responsibility

This crate builds the `vak-server` binary. It is the only place that:

- reads the process environment
- picks implementations (local or Databricks adapters, Postgres repositories) and wires them to ports
- owns the tokio runtime and process lifecycle

Today the CLI (clap) has three subcommands:

- `vak-server serve` loads `Settings` from `vak-config`, initialises `vak-telemetry`, and wires `Database::connect_lazy` (needs `DATABASE_URL`) → `PgAgentRepo` → `AgentService` (with `vak_services::rule_packs::shipped()`; a pack that doesn't load stops the server), then `VoiceService` → `AppState::with_voice` → `vak_api::app`. API requests run under the `VAK_HTTP_REQUEST_TIMEOUT_MS` deadline (default 10 s). If `VAK_DEV_OPERATOR_EMAIL` is set (local profile only; `vak-config` rejects it elsewhere), API requests without identity headers act as that operator, and a warning says so at startup. It binds `settings.http.addr`; on Ctrl-C or SIGTERM it shuts down gracefully, letting in-flight requests finish.
- **Test runs:** `voice::test_engines(&settings.models)` builds only the LLM and the decision model (test runs are text, with no speech), or `None` with a warning, and `vak-server` wires `PgTestRunRepo` into `AgentService::with_test_runs` (the Test step of progress) and a `TestRunService` into `AppState::with_test_runs`. `serve` spawns the service's `TestRunWorker::run`, which interrupts runs a stopped server left running, then runs queued runs; it stops with the server (shutdown waits at most 5 s for it).
- **Voice** (`src/voice.rs`): `voice::engines(&settings.models)` builds the four adapters from `ModelSettings`: `OpenAiLlm` (with the API key and the parsed `VAK_LLM_EXTRA_BODY`), `ServingStt`, `ServingTts` and `SystemOneDecisions` (with `VAK_DECISION_PATH` when set). When a model isn't configured, it returns `None` with a warning that names the first missing variable. Voice sessions are then off, and `POST /api/v1/agents/{id}/sessions` answers 503. A setting that can't be used (an extra body that isn't JSON, or an `https://` base URL before M7) fails startup.
- `vak-server migrate` applies the pending `vak-repositories-pg` migrations and exits. Run it before each deploy (and before `serve` on a fresh database).
- `vak-server openapi [--output FILE]` writes `vak_api::openapi_json()` to the file, or to stdout. It reads no settings or environment. `just openapi` uses it to write `contracts/openapi.json`, and `just contracts-verify` uses it to check that file for drift.

Planned subcommand: `worker` (a separate job-queue process). Until then the test run worker runs inside `serve`.

**Deployment assumption.** The API trusts `X-Forwarded-User` and `X-Forwarded-Email` because, on Databricks Apps, the app is reachable only through the platform proxy, which signs the user in and sets them. Never expose `vak-server` directly (for example a public `VAK_HTTP_ADDR` outside Databricks Apps) without a proxy that strips and sets those headers. The local dev operator is refused outside the `local` profile.

## Dependencies

- **May depend on:** every workspace crate. `layers.toml` lists all of them. Dev-only: `vak-testkit`.
- **External crates:** `anyhow` (allowed here, unlike in libraries), `clap`, `tokio` (rt-multi-thread, macros, net, signal, sync and time: the worker's stop signal and shutdown wait), `axum` (for `axum::serve`; a `deny.toml` wrapper), `tracing`, `serde_json` (parses `VAK_LLM_EXTRA_BODY`).
- **Must not:** contain business logic, handlers, SQL or vendor protocol code. If it isn't wiring or lifecycle, it belongs in a library crate.

## Conventions

- **`main` stays small:** parse the CLI → load settings → init telemetry → build dependencies → run the subcommand.
- **Environment:** read it in one place, `Settings::from_lookup(|name| std::env::var(name).ok())`, and pass typed config down.
- **Errors:** use `anyhow::Context` to say which step failed. Never print secrets or full config.
- **Panics:** no `unwrap`/`expect`/`panic!`; return `anyhow::Result` from `main`.
- **Graceful shutdown:** every long-running task gets a `CancellationToken` or shutdown future. Shutdown waits for tracked tasks with a timeout.
- **Visibility:** binary crate, so keep items private. `unreachable_pub` flags any `pub`.

## Testing

- **Unit tests:** CLI parsing (`Cli::command().debug_assert()`), and serving on `127.0.0.1:0` with a shutdown trigger. Assert that `/health` answers, that `/api/v1` needs an identity unless a dev operator is set, and that serve returns after shutdown. The pool connects lazily, so these need no database. `voice::engines` is tested on settings: every model set turns voice on, a missing one turns it off, and bad settings fail. `catalog_voices_are_pinned_for_local_tts` reads `deploy/local/models.sh` (`include_str!`) and fails when a voice of the domain catalog has no pinned file beside the Kokoro weights: offline, mlx-audio can't speak it, and the call fails before the disclosure.
- **End-to-end with Postgres:** the frontend's Playwright run builds this binary, migrates a throwaway database and serves it (`frontend/e2e/support/backend-server.ts`).
- **End-to-end:** a caller bot against the full Compose stack. These don't run here.

## Coverage floor

`apps/` is excluded from coverage. Logic worth measuring belongs in a library crate.

## Example

```sh
cd backend
export DATABASE_URL="$(tools/test-db.sh url)"   # or your local Postgres
cargo run -p vak-server -- migrate
VAK_DEV_OPERATOR_EMAIL=dev@localhost VAK_LOG=debug cargo run -p vak-server -- serve
curl -s localhost:3000/health            # {"status":"ok","version":"0.1.0"}
curl -s localhost:3000/api/v1/agents     # {"items":[]} as the dev operator
```
