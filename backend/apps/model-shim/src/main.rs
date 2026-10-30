//! Dev-only model shim: serves the Databricks Model Serving invocation shapes
//! (`/serving-endpoints/{stt,tts}/invocations`) and forwards them to local
//! mlx-audio models, so the serving adapters run unchanged in development.
//!
//! The implementation lands with the local dev stack (T0.8).

fn main() {}
