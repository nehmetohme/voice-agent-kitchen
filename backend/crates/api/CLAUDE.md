# vak-api: HTTP and WebSocket API

## Responsibility

This crate holds:

- the axum routers and handlers, plus WebSocket handlers (media sessions)
- DTOs and request validation
- RFC 9457 `application/problem+json` errors
- the OpenAPI document (utoipa)

Today it exposes:

- `pub fn router() -> axum::Router`: the unauthenticated routes, `GET /health` (200 and `{"status":"ok","version":"<CARGO_PKG_VERSION>"}`).
- `pub fn app(AppState) -> axum::Router`: `router()` plus `/api/v1`, the app `vak-server` serves. `AppState::new(AgentService)` accepts only the forwarded identity; `.with_dev_operator(Actor)` (local profile only, `vak-server` enforces it) lets requests without identity headers act as that operator.
- `pub fn openapi_json()`: the OpenAPI 3.1 document for the same routes, as pretty JSON with sorted keys and a trailing newline. `vak-server openapi` writes it to `contracts/openapi.json`.

| Route (`/api/v1`) | Handler (`operationId`) |
|---|---|
| `POST /agents` | `createAgent`: only `name`, `useCase`, `voice` required; 201 with `Location` |
| `GET /agents` | `listAgents`: active agents, newest first, with `progressPercent` |
| `GET`, `PATCH`, `DELETE /agents/{id}` | `getAgent`, `updateAgent` (autosave; send changed fields only), `archiveAgent` (204, idempotent) |
| `GET /agents/{id}/progress` | `getAgentProgress`: four steps, `percent`, `nextStep` |
| `GET /catalog/voices`, `/catalog/languages` | `listVoices`, `listLanguages` |

- **Identity** (`src/identity.rs`): middleware on the whole `/api/v1` router, unknown paths included. Databricks Apps signs the user in and forwards `X-Forwarded-User` (stable IdP user ID) and `X-Forwarded-Email`; both must be present and valid, or the answer is 401. The app is reachable only through that proxy, which sets the headers, so they are trusted.
- **Errors** (`src/problem.rs`): `ApiError` → `application/problem+json` with `type: about:blank`; 400 malformed JSON, 401, 404 (also for an ID that can't name an agent), 409 (archived, or still conflicting after retries), 415, 422 (with `errors: [{field, message}]`, one per invalid field), 503 (source logged once, never sent).
- **Body limit:** 64 KiB on `/api/v1`.

Every route comes from one `OpenApiRouter` (`src/openapi.rs`), so a route can't exist without its documentation. Contracts: `tests/contract_health.rs`, `tests/contract_agent_provisioning_api.rs`.

## Dependencies

- **May depend on:** `vak-services`, `vak-domain`. Dev-only: `vak-testkit`.
- **External crates:** `axum` (this crate and `vak-server` are the `deny.toml` wrappers), `serde`, `serde_json`, `utoipa` and `utoipa-axum` (only here: other crates get the document through `openapi_json()`), `tracing` (logs 5xx sources). Later: `thiserror`, `tower-http`, `garde`.
- **Must not depend on:** repositories, repositories-pg, providers, pipeline, any adapter, sqlx, reqwest. Port types the API needs are re-exported by services.

## Conventions

- **Thin handlers:** parse, validate, call **one** service function, map the result. No business logic, no direct I/O.
- **DTOs** live only here, never in the domain. Map DTO ↔ domain explicitly with `From`/`TryFrom`.
- **Validation:** validate at the boundary, and accept flexible input but produce strict output. Request DTOs take strings and parse each field with its domain type (`AgentName::parse`, `UseCase::parse`…), collecting every invalid field into one 422; required fields are `Option` in Rust so a missing one is named too, and `#[schema(required = true)]` keeps the document strict.
- **Errors:** `ApiError: IntoResponse` maps service errors to problem+json. Log the source chain once, at the boundary. Never put internal error text or PII in responses.
- **Every route** is registered in the OpenAPI document (`OpenApiRouter` + `routes!`) and has auth. `/health` is the only unauthenticated route.
- **The OpenAPI contract** (`contracts/openapi.json`) is generated with `just openapi`. Never hand-edit it.
- **Middleware:** identity and a body size limit on `/api/v1` today; request IDs, tracing and a request timeout (tower-http) come with the first route that needs them. Repository calls already run under a 5 s deadline in the services.
- **Visibility:** `pub(crate)` for handlers and DTOs, unless the OpenAPI export needs them public.

## Testing

- **Contract tests first:** `tests/contract_<feature>.rs`, for example `tests/contract_health.rs`. Never edit a contract file once it's tagged `contract/<feature>/vN`; bump the version instead.
- **Router tests** run in-process with `tower::ServiceExt::oneshot`, against services backed by testkit fakes.
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
