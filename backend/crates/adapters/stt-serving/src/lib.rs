//! `SttEngine` adapter that calls a speech-to-text model over the Databricks
//! Model Serving invocations contract (`contracts/serving/stt.json`). The local
//! model shim speaks the same contract, so dev and prod share this code.
