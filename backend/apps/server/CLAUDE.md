# vak-server: composition root

## Responsibility

This crate builds the `vak-server` binary. It is the only place that:

- reads the process environment
- picks implementations (local or Databricks adapters, Postgres repositories) and wires them to ports
- owns the tokio runtime and process lifecycle

Today the CLI (clap) has two subcommands:

- `vak-server serve` loads `Settings` from `vak-config`, initialises `vak-telemetry`, binds `settings.http.addr` and serves `vak_api::router()`. On Ctrl-C or SIGTERM it shuts down gracefully, letting in-flight requests finish.
- `vak-server openapi [--output FILE]` writes `vak_api::openapi_json()` to the file, or to stdout. It reads no settings or environment. `just openapi` uses it to write `contracts/openapi.json`, and `just contracts-verify` uses it to check that file for drift.

Planned subcommands: `worker` (job queue) and `migrate` (runs `vak-repositories-pg` migrations before a deploy).

## Dependencies

- **May depend on:** every workspace crate. `layers.toml` lists all of them. Dev-only: `vak-testkit`.
- **External crates:** `anyhow` (allowed here, unlike in libraries), `clap`, `tokio` (rt-multi-thread, macros, net, signal), `axum` (for `axum::serve`; a `deny.toml` wrapper), `tracing`.
- **Must not:** contain business logic, handlers, SQL or vendor protocol code. If it isn't wiring or lifecycle, it belongs in a library crate.

## Conventions

- **`main` stays small:** parse the CLI → load settings → init telemetry → build dependencies → run the subcommand.
- **Environment:** read it in one place, `Settings::from_lookup(|name| std::env::var(name).ok())`, and pass typed config down.
- **Errors:** use `anyhow::Context` to say which step failed. Never print secrets or full config.
- **Panics:** no `unwrap`/`expect`/`panic!`; return `anyhow::Result` from `main`.
- **Graceful shutdown:** every long-running task gets a `CancellationToken` or shutdown future. Shutdown waits for tracked tasks with a timeout.
- **Visibility:** binary crate, so keep items private. `unreachable_pub` flags any `pub`.

## Testing

- **Unit tests:** CLI parsing (`Cli::command().debug_assert()`), and serving on `127.0.0.1:0` with a shutdown trigger. Assert that `/health` answers and that serve returns after shutdown.
- **End-to-end:** a caller bot against the full Compose stack. These don't run here.

## Coverage floor

`apps/` is excluded from coverage. Logic worth measuring belongs in a library crate.

## Example

```sh
cd backend
VAK_HTTP_ADDR=127.0.0.1:3000 VAK_LOG=debug cargo run -p vak-server -- serve
curl -s localhost:3000/health   # {"status":"ok","version":"0.1.0"}
```
