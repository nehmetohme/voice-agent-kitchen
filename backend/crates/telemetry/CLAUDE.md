# vak-telemetry: tracing, OpenTelemetry, metrics

## Responsibility

This crate handles telemetry setup. Today, `pub fn init(filter: &str) -> Result<(), TelemetryError>` installs a `tracing-subscriber` fmt subscriber with an `EnvFilter`.

Later it adds:

- **Output:** JSON in production, pretty output in development.
- **Export:** OpenTelemetry through `tracing-opentelemetry` and OTLP to a collector, which writes to Unity Catalog `otel_*` tables.
- **Metric names:** `voice.turn.latency`, `voice.stage.latency{stage}`, `voice.llm.ttft`, `voice.tts.ttfb`, `voice.frames.dropped`, ...
- **Buckets:** histogram buckets in ms (10, 25, 50, ... 5000).
- **Runtime health:** `tokio-metrics`.

## Dependencies

- **May depend on:** no workspace crate.
- **External crates:** `tracing`, `tracing-subscriber`, `thiserror`. Later: `tracing-opentelemetry`, `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`.
- **Must not depend on:** any other `vak-*` crate, axum, sqlx, reqwest.
- Only the composition root (`vak-server`, `vak-model-shim`) calls `init`.

## Conventions

- **Settings come in as arguments.** Never read the environment here; `vak-server` passes the filter from `vak-config`.
- **`init` returns errors** (invalid filter, subscriber already set). It never panics.
- **One home for metric names and buckets:** define them here as `const`s, so dashboards and code agree. Changing a name is a breaking change.
- **Span shape:** `call` → `turn` → `stage` (`vad`, `stt`, `llm`, `tts`, `transport`). Attributes: `call_id`, `agent_version_id`, `provider`, `model`. No per-frame spans.
- **PII:** never record transcripts, prompts or audio as span fields or log fields above `debug`.

## Testing

- Unit tests:
  - an invalid filter returns an error without installing anything
  - the first `init` succeeds and a second `init` fails
- nextest runs each test in its own process, so global subscriber state doesn't leak between tests.

## Coverage floor

No per-crate floor; this crate counts toward the global 80%.

## Example

```rust
// In vak-server's composition root:
vak_telemetry::init(&settings.log.filter)?;
tracing::info!(addr = %settings.http.addr, "listening");
```
