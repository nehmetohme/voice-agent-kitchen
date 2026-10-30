# vak-api: HTTP and WebSocket API

## Responsibility

This crate holds:

- the axum routers and handlers, plus WebSocket handlers (media sessions)
- DTOs and request validation
- RFC 9457 `application/problem+json` errors
- the OpenAPI document (utoipa)

Today it exposes `pub fn router() -> axum::Router` with `GET /health`, which returns 200 and `{"status":"ok","version":"<CARGO_PKG_VERSION>"}`.

## Dependencies

- **May depend on:** `vak-services`, `vak-domain`. Dev-only: `vak-testkit`.
- **External crates:** `axum` (this crate and `vak-server` are the `deny.toml` wrappers), `serde`, `tracing`, `thiserror`. Later: `tower-http`, `utoipa`, `utoipa-axum`, `garde`.
- **Must not depend on:** repositories, repositories-pg, providers, pipeline, any adapter, sqlx, reqwest. Port types the API needs are re-exported by services.

## Conventions

- **Thin handlers:** parse, validate, call **one** service function, map the result. No business logic, no direct I/O.
- **DTOs** live only here, never in the domain. Map DTO ↔ domain explicitly with `From`/`TryFrom`.
- **Validation:** validate at the boundary (`garde`), and accept flexible input but produce strict output.
- **Errors:** `ApiError: IntoResponse` maps service errors to problem+json. Log the source chain once, at the boundary. Never put internal error text or PII in responses.
- **Every route** is registered in the OpenAPI document (`OpenApiRouter` + `routes!`) and has auth. `/health` is the only unauthenticated route.
- **The OpenAPI contract** (`contracts/openapi.json`) is generated with `just openapi`. Never hand-edit it.
- **Middleware** (tower-http): request IDs, tracing, timeouts and body size limits on every router.
- **Visibility:** `pub(crate)` for handlers and DTOs, unless the OpenAPI export needs them public.

## Testing

- **Contract tests first:** `tests/contract_<feature>.rs`, for example `tests/contract_health.rs`. Never edit a contract file once it's tagged `contract/<feature>/vN`; bump the version instead.
- **Router tests** run in-process with `tower::ServiceExt::oneshot`, against services backed by testkit fakes.
- **OpenAPI:** an insta snapshot of the generated document; `vak-contract-tests` validates responses against `contracts/openapi.json`.

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
