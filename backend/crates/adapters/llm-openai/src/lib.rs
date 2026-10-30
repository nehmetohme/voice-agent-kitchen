//! `LlmEngine` adapter for OpenAI-compatible streaming chat completions with
//! tools. One code path serves Databricks Foundation Model APIs in production and
//! the local llama-server in development; only base URL and auth differ.
