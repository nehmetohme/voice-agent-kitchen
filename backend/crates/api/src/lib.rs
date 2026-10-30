//! HTTP and WebSocket API: axum routers, DTOs, request validation, RFC 9457
//! problem+json errors and the OpenAPI document.
//!
//! Handlers are thin: parse, validate, call one service, map the result.
