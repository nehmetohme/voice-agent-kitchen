//! `TtsEngine` adapter that calls a text-to-speech model over the Databricks
//! Model Serving invocations contract (`contracts/serving/tts.json`). The local
//! model shim speaks the same contract, so dev and prod share this code.
