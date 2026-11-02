# vak-services: use cases

## Responsibility

This crate holds the application use cases:

- agents
- plans
- versions and publishing (test-gated, immutable, with rollback)
- test runs
- sessions
- audit

It orchestrates repositories, providers and the pipeline. It re-exports the few pipeline and port types that `vak-api` and `vak-server` need, so neither depends on `vak-pipeline` or `vak-providers`:

- `Engines` and `StageDeadlines` from `vak-pipeline`.
- `media`: `MediaTransport`, `MediaPeer`, `PeerSender`, `InputFramer`, `PushAudio`, `Outbound`, `ServerEvent`, `PROTOCOL` and `media_channel` from `vak_providers::media`, for the WebSocket bridge.

Today (M2, M3 and M4a):

- `AgentService::new(Arc<dyn AgentRepo>)`: `create` (defaults from the domain), `list` (active agents with their `SetupProgress`, newest first, at most `LIST_LIMIT` = 200), `get`, `progress`, `update_draft` (autosave; a no-op patch stores and audits nothing; retries up to 3 times on a stale revision) and `archive` (idempotent). Errors: `CreateAgentError`, `ListAgentsError`, `FindAgentError`, `ArchiveAgentError`, `UpdateDraftError`.
- Plans (`src/plans.rs`): `plan` (the saved plan or the template, as a domain `DraftPlan`), `save_plan` (through `update_draft`, so one `agent.draft_updated` event for `plan`; a plan that breaks a graph rule is saved with its errors) and `validate_plan` (no save). Progress uses `ProgressFacts::from_draft`, so a saved valid plan makes 50 %.
- `rule_packs::shipped()` and `rule_packs::parse(file, text)`: the TOML packs in `contracts/rule-packs/`, embedded with `include_str!` (`FILES`) and checked by the domain. `AgentService::new` uses them (none if they don't load, so every plan is invalid); `with_rule_packs(catalog)` sets them, and `vak-server` loads them first and refuses to start when one is broken.
- `catalog::voices()` and `catalog::languages()`: the domain's static catalog.
- `VoiceService::new(AgentService, Option<Engines>)` (or `with_settings(…, VoiceSettings)`): live test calls. With `None` engines (no models configured), every session is refused. Sessions live in memory; storing sessions and turns comes with the test step (M5).
  - `start_session(&Actor, AgentId) -> SessionTicket { session_id, media_path }` needs engines, an active agent and a free slot. It issues a UUID v7 session ID and a ticket that holds the agent's `SessionScript::for_draft`. `media_path` is `/sessions/{id}/media`, relative to the API root. Errors (`StartSessionError`): `NotFound`, `Archived`, `VoiceUnavailable`, `Busy`, `Unavailable` (agent storage).
  - `open_media(&Actor, &str) -> MediaSession` claims the ticket: once, within its lifetime, and only by the operator who asked for it. Anything else is `OpenMediaError::NotFound`; no free slot is `Busy`. Each new ticket also prunes expired ones.
  - `MediaSession::run(MediaTransport) -> SessionEndReason` builds the pipeline's `SessionConfig` from the script and the settings, runs the session, and frees its slot when the session ends.
  - `VoiceSettings` defaults: a 60 s ticket lifetime, at most 4 live sessions (a local LLM box serves one request at a time), `StageDeadlines::default()`, a 300 ms opt-out decision wait, a 2 s decision deadline, and 30 minutes per session.
  - `latency() -> Vec<StageReport>`: the voice path's latency so far.
- `latency::LatencyStats`: the `SessionObserver` every session reports to. For each stage (`stt`, `decision`, `llm_ttft`, `tts_first_audio`, `mouth_to_ear`), it counts replies into `BUCKETS_MS` (10 to 5000 ms, the `vak-telemetry` buckets, plus an overflow bucket) and gives the p50 and p95 over the last 1000 replies. `decision` is the opt-out decision call, reported through `SessionObserver::decision_latency` for every caller turn (answered, timed out or failed); it runs beside the LLM. It is in memory, per process; telemetry export comes with M7.

## Dependencies

- **May depend on:** `vak-repositories`, `vak-providers`, `vak-pipeline`, `vak-domain`. Dev-only: `vak-testkit`.
- **External crates:** `thiserror`, `tokio` (`time`; only what orchestration needs), `serde` and `toml` (rule pack files), `uuid` (v7 IDs) and `jiff` (time). Dev-only: `serde_json`, and tokio's `test-util`.
- **Must not depend on:** repositories-pg, any adapter, api, axum, sqlx, reqwest.

## Conventions

- **One public function per use case,** such as `publish_version(&self, cmd: PublishVersion) -> Result<PublishedVersion, PublishError>`. Inputs are command structs of domain types, not DTOs.
- **Business rules live in `vak-domain`.** Services load, call domain rules, persist and emit audit events. If you're writing an `if` about business policy here, move it to the domain.
- **The outside world is reached only through ports.** Repository and provider ports are dyn-compatible and held as `Arc<dyn …>` (the provider engines inside `Engines`), so `vak-api` stays free of generics and `vak-server` picks the implementation.
- **IDs and time:** services generate UUID v7 IDs and read the clock (`now()` rounds to microseconds, Postgres precision); the domain never does.
- **Errors:** a `ServiceError` per use case (`thiserror`), built from domain, `RepoError` and `ProviderError`. Don't stringify errors.
- **Timeouts:** every port call runs under a deadline (`AgentService` default 5 s, `with_deadline` to change it; model calls under `VoiceSettings::deadlines`). Transactions are port methods, not service code.
- **Audit:** every state change writes an audit event. Build it with the domain's `AuditEvent` and pass it to the same repository call that stores the change, so both commit in one transaction.
- **No HTTP concepts** such as status codes or headers. The API maps errors.
- **Visibility:** `pub(crate)` is the default; use-case functions and their commands and errors are `pub`.

## Testing

- **Unit tests** use hand-written fakes from `vak-testkit`: in-memory repos and scripted providers.
- **mockall** only for "must not call X" interaction checks.
- **Coverage per use case:** the happy path, each error variant, and audit emission.
- **Voice sessions** (`src/voice/tests.rs`) run under paused tokio time, with scripted engines and testkit's fake media client: a ticket opens once and only for its operator, it expires, the session limit holds, and a session speaks the agent's script and records its latency.
- **Mutation testing** (`cargo mutants --in-diff`) runs nightly on this crate as a report.

## Coverage floor

**90%** line coverage (`coverage.toml`).

## Example

The shape of `AgentService::archive` (`src/agents.rs`):

```rust
pub async fn archive(&self, actor: &Actor, id: AgentId) -> Result<(), FindAgentError> {
    let stored = self.within(self.repo.get(id)).await?;
    let Some(archived) = stored.value.archive(now()) else {
        return Ok(()); // the rule lives in vak-domain: archiving twice changes nothing
    };
    let event = audit_event(actor, &archived, AuditAction::AgentArchived)?;
    // One port call: the change and its audit event commit together.
    self.save(&archived, stored.revision, &event).await?;
    Ok(())
}
```
