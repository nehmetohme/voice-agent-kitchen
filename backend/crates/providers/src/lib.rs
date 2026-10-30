//! Provider ports: the traits the pipeline and services use to reach models
//! and media transports (`LlmEngine`, `SttEngine`, `TtsEngine`, `DecisionEngine`,
//! `MediaTransport`), plus their stream event and error types.
//!
//! Traits and port types only. Implementations live in `crates/adapters/*`;
//! fakes live in `vak-testkit`.
