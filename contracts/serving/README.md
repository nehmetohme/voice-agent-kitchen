# contracts/serving/

Request and response shapes for model serving: `stt.json` and `tts.json`. Production uses Databricks Model Serving and local development uses `vak-model-shim`; both speak these shapes.

Empty until M4 (voice core) defines them, alongside the adapters; M7 runs the same suite against staging endpoints. Adapter contract tests run against them; see `../CLAUDE.md`.
