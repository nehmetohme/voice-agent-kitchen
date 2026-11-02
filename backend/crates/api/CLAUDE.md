# vak-api: HTTP and WebSocket API

## Responsibility

This crate holds:

- the axum routers and handlers, plus WebSocket handlers (media sessions)
- DTOs and request validation
- RFC 9457 `application/problem+json` errors
- the OpenAPI document (utoipa)

Today it exposes:

- `pub fn router() -> axum::Router`: the unauthenticated routes, `GET /health` (200 and `{"status":"ok","version":"<CARGO_PKG_VERSION>"}`).
- `pub fn app(AppState) -> axum::Router`: `router()` plus `/api/v1`, the app `vak-server` serves. `AppState::new(AgentService)` accepts only the forwarded identity, and voice sessions are off (a `VoiceService` without engines). `.with_voice(VoiceService)` turns them on, `.with_request_timeout(Duration)` sets the request deadline, and `.with_dev_operator(Actor)` (local profile only, `vak-server` enforces it) lets requests without identity headers act as that operator.
- `pub fn openapi_json()`: the OpenAPI 3.1 document for the same routes, as pretty JSON with sorted keys and a trailing newline. `vak-server openapi` writes it to `contracts/openapi.json`.

| Route (`/api/v1`) | Handler (`operationId`) |
|---|---|
| `POST /agents` | `createAgent`: only `name`, `useCase`, `voice` required; 201 with `Location` |
| `GET /agents` | `listAgents`: active agents, newest first, with `progressPercent` |
| `GET`, `PATCH`, `DELETE /agents/{id}` | `getAgent`, `updateAgent` (autosave; send changed fields only), `archiveAgent` (204, idempotent) |
| `GET /agents/{id}/progress` | `getAgentProgress`: four steps, `percent`, `nextStep` |
| `GET`, `PUT /agents/{id}/plan` | `getAgentPlan` (the saved plan, or the template with `saved: false`, with `rulePack`, `lockedTexts`, `optOutConfirmation` and `validation`), `saveAgentPlan` (replaces the plan; graph problems come back as located errors, malformed fields are a 422; 409 when archived) |
| `POST /agents/{id}/plan/validate` | `validateAgentPlan`: `{valid, errors: [{code, message, nodeId?, edgeId?}]}`, nothing saved |
| `GET /catalog/voices`, `/catalog/languages` | `listVoices`, `listLanguages` |
| `POST /agents/{id}/sessions` | `startSession`: 201 with `{id, mediaUrl}`; `mediaUrl` is `/api/v1/sessions/{id}/media`, to open within a minute, once, by the same operator |
| `GET /sessions/{id}/media` | `openSessionMedia`: the media WebSocket. The upgrade must offer the subprotocol `vak.media.v1` (`contracts/media/`), or the answer is 400. Messages and frames are capped at 64 KiB |
| `GET /voice/latency` | `getVoiceLatency`: per stage (`stt`, `decision`, `llm_ttft`, `tts_first_audio`, `mouth_to_ear`), the count, `p50Ms`, `p95Ms` and histogram `buckets` (`leMs`, `count`; `null` bounds the overflow bucket) |

- **Identity** (`src/identity.rs`): middleware on the whole `/api/v1` router, unknown paths included. Databricks Apps signs the user in and forwards `X-Forwarded-User` (stable IdP user ID) and `X-Forwarded-Email`; both must be present and valid, or the answer is 401. The app is reachable only through that proxy, which sets the headers, so they are trusted.
- **Errors** (`src/problem.rs`): `ApiError` → `application/problem+json` with `type: about:blank`:
  - 400: malformed JSON, or a media request that isn't a WebSocket upgrade offering `vak.media.v1`.
  - 401.
  - 403: a media handshake from a page on another site.
  - 404: also for an ID that can't name an agent, and for a session ticket that is unknown, expired, used or another operator's.
  - 409: archived (it can't be edited or take calls), or still conflicting after retries.
  - 415.
  - 422: with `errors: [{field, message}]`, one per invalid field; plan fields are paths such as `nodes[2].label`.
  - 503: storage unavailable or the request deadline passed (a source is logged once, never sent), or a refusal with a detail for people that logs nothing (`ApiError::Refused`: voice sessions aren't set up on this server, or too many test calls are running).
- **Body limit:** 64 KiB on `/api/v1`; the plan routes raise it to 512 KiB (`plans::BODY_LIMIT`), enough for 50 steps of 2,000 characters. The media WebSocket has none: its messages and frames are capped at 64 KiB.
- **Request deadline** (`src/deadline.rs`): every `/api/v1` request except the media WebSocket runs under `AppState::with_request_timeout` (default 10 s; `vak-server` passes `VAK_HTTP_REQUEST_TIMEOUT_MS`); past it the handler is dropped and the answer is a 503 problem. `/health` is outside it too. The media WebSocket's route comes from its own `OpenApiRouter` (`openapi::v1_media()`), which `app()` gives the identity layer only and merges under `/api/v1`, so a call can outlast any request.
- **Media bridge** (`src/sessions/bridge.rs`): one task per socket moves frames between the WebSocket and the session's `MediaPeer`, and does nothing else. The session itself runs in a spawned task (`MediaSession::run`). Client audio goes through an `InputFramer` and is pushed without waiting: when the pipeline falls behind, frames are dropped and counted (`dropped_frames`, logged when the socket closes). Agent frames are written into one reused `BytesMut`, so no frame allocates. Each socket write has a 5 s timeout: a client that stops reading is gone, and its session ends instead of holding a slot. When either side ends, the other one ends too.
- **Same origin** (`sessions::same_origin`): a media handshake whose `Origin` isn't the app's own host (`X-Forwarded-Host` behind the Databricks Apps proxy, else `Host`) is refused with 403, so a page on another site can't open a call with the operator's session (cross-site WebSocket hijacking). A handshake without `Origin` isn't a browser and carries no ambient credentials, so it passes.

Every route comes from an `OpenApiRouter` (`src/openapi.rs`: `public()`, `v1()` and `v1_media()`), so a route can't exist without its documentation. Contracts: `tests/contract_health.rs`, `tests/contract_agent_provisioning_api.rs`, `tests/contract_plan_api.rs`. Voice sessions: `tests/sessions.rs`.

## Dependencies

- **May depend on:** `vak-services`, `vak-domain`. Dev-only: `vak-testkit`.
- **External crates:** `axum` with the `ws` feature (this crate and `vak-server` are the `deny.toml` wrappers), `bytes` (`BytesMut` for outbound audio), `serde`, `serde_json`, `utoipa` and `utoipa-axum` (only here: other crates get the document through `openapi_json()`), `tracing` (logs 5xx sources), `tokio` (`time`, for the request deadline). Dev-only: `tower`, `http-body-util`, and `tokio-tungstenite` with `futures-util` (a real WebSocket client for `tests/sessions.rs`). Later: `thiserror`, `tower-http`, `garde`.
- **Must not depend on:** repositories, repositories-pg, providers, pipeline, any adapter, sqlx, reqwest. Port types the API needs are re-exported by services (`vak_services::media`, `vak_services::latency`).

## Conventions

- **Thin handlers:** parse, validate, call **one** service function, map the result. No business logic, no direct I/O.
- **DTOs** live only here, never in the domain. Map DTO ↔ domain explicitly with `From`/`TryFrom`.
- **Validation:** validate at the boundary, and accept flexible input but produce strict output. Request DTOs take strings and parse each field with its domain type (`AgentName::parse`, `UseCase::parse`…), collecting every invalid field into one 422; required fields are `Option` in Rust so a missing one is named too, and `#[schema(required = true)]` keeps the document strict.
- **Errors:** `ApiError: IntoResponse` maps service errors to problem+json. Log the source chain once, at the boundary. Never put internal error text or PII in responses.
- **Every route** is registered in the OpenAPI document (`OpenApiRouter` + `routes!`) and has auth. `/health` is the only unauthenticated route.
- **The OpenAPI contract** (`contracts/openapi.json`) is generated with `just openapi`. Never hand-edit it.
- **Middleware:** on `/api/v1`, outermost first: the request deadline, the body size limit, identity. The media WebSocket gets identity only. Request IDs and tracing (tower-http) come with the first route that needs them. Repository calls also run under a 5 s deadline in the services, below the request deadline.
- **Visibility:** `pub(crate)` for handlers and DTOs, unless the OpenAPI export needs them public.

## Testing

- **Contract tests first:** `tests/contract_<feature>.rs`, for example `tests/contract_health.rs`. Never edit a contract file once it's tagged `contract/<feature>/vN`; bump the version instead.
- **Router tests** run in-process with `tower::ServiceExt::oneshot`, against services backed by testkit fakes.
- **WebSocket tests** (`tests/sessions.rs`) serve the app on `127.0.0.1:0` and connect with `tokio-tungstenite`, so a whole `vak.media.v1` conversation runs over a real socket against scripted engines.
- **OpenAPI:** `tests/openapi.rs` checks the document's shape and determinism, and fails when the committed `contracts/openapi.json` is stale (run `just openapi`). `vak-contract-tests` validates responses against `contracts/openapi.json`.
- **Annotate every handler** with `#[utoipa::path]`, including an explicit `operation_id` (it names the generated frontend function) and a `tag` (it names the generated file).

## Coverage floor

**85%** line coverage (`coverage.toml`).

## Example

```rust
#[tokio::test]
async fn health_returns_ok() {
    let response = vak_api::router()
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
```
