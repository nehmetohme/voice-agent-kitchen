# vak-config: typed settings

## Responsibility

This crate holds the application `Settings` with defaults and overrides:

- **Local and Databricks profiles:** switching between them is configuration only.
- **Validation at startup:** fail fast.
- **Secrets:** every secret is typed as `secrecy::SecretString`.

Today:

- **Shape:** `Settings { profile, http: { addr, request_timeout }, log: { filter }, database: { url: Option<SecretString> }, auth: { dev_operator_email: Option<String> }, models }`.
- **Models** (`src/models.rs`): `ModelSettings { llm, stt, tts, decisions }`, each `Option`:
  - `llm`: `LlmSettings { base_url, model, api_key: Option<SecretString>, extra_body: Option<String> }`. `extra_body` is JSON text, which the composition root parses.
  - `stt`, `tts`: `ServingSettings { base_url, endpoint }`.
  - `decisions`: `DecisionSettings { base_url, model, path: Option<String> }`.

  A model is set when its base URL is. Voice sessions run only when all four are set: `complete()` says whether they are, and `first_missing()` names the first base-URL variable left unset, for the startup notice.
- **Defaults:** profile `local`, `127.0.0.1:3000`, a 10 s request timeout, filter `info`, no database URL, no dev operator, no models.
- **Overrides:** `Settings::from_lookup(lookup)` applies these; an empty value counts as unset.

| Variable | Sets | Notes |
|---|---|---|
| `VAK_PROFILE` | `profile` | `local` (default) or `databricks`, any case |
| `VAK_HTTP_ADDR` | `http.addr` | `IP:port` |
| `VAK_HTTP_REQUEST_TIMEOUT_MS` | `http.request_timeout` | Whole milliseconds, 1–300000; default 10000. The `/api/v1` request deadline |
| `VAK_LOG` | `log.filter` | `EnvFilter` syntax |
| `DATABASE_URL` | `database.url` | `postgres://…`; a secret (`Debug` prints `[REDACTED]`). The conventional name, so sqlx and psql read it too |
| `VAK_DEV_OPERATOR_EMAIL` | `auth.dev_operator_email` | Local development only: an error in any other profile, so a deployment can't fall back to a fixed identity |
| `VAK_LLM_BASE_URL` | `models.llm.base_url` | The OpenAI-compatible API root, e.g. `http://host:8080/v1`. Sets the LLM |
| `VAK_LLM_MODEL` | `models.llm.model` | Required with `VAK_LLM_BASE_URL` (`ConfigError::Missing`) |
| `VAK_LLM_API_KEY` | `models.llm.api_key` | A bearer token, if the server wants one. A secret |
| `VAK_LLM_EXTRA_BODY` | `models.llm.extra_body` | JSON text: an object merged into every chat request |
| `VAK_STT_BASE_URL` | `models.stt.base_url` | The serving root or the local model shim. Sets STT |
| `VAK_STT_ENDPOINT` | `models.stt.endpoint` | Default `stt` |
| `VAK_TTS_BASE_URL` | `models.tts.base_url` | Sets TTS |
| `VAK_TTS_ENDPOINT` | `models.tts.endpoint` | Default `tts` |
| `VAK_DECISION_BASE_URL` | `models.decisions.base_url` | The System One server root. Sets decisions |
| `VAK_DECISION_MODEL` | `models.decisions.model` | Required with `VAK_DECISION_BASE_URL` (`ConfigError::Missing`) |
| `VAK_DECISION_PATH` | `models.decisions.path` | Unset means the adapter's `/v1/systemone` (llama-server); Databricks serves `/ai-gateway/typesafe/v1/systemone` |

## Dependencies

- **May depend on:** no workspace crate.
- **External crates:** `thiserror`, `secrecy`, and later `serde` and `config` (layered files).
- **Must not depend on:** any other `vak-*` crate, tokio, axum, sqlx, reqwest.
- Only `vak-server` (and the dev-only `vak-model-shim`) may depend on this crate. Inner crates receive their own typed config structs from the composition root.

## Conventions

- **Never read the process environment here.** `vak-server` passes a lookup function (`|name| std::env::var(name).ok()`), which keeps loading pure and testable.
- **Variable names** are `VAK_*` (`DATABASE_URL` is the one conventional exception). Declare each one as a `const` and list it in the doc comment of the field it sets.
- **Errors:** `ConfigError` names the variable and the problem but **never echoes the value**, which may be a secret.
- **Secrets:** every credential field is `SecretString`. `Debug` must not print it.
- **Defaults** suit local development. Production values come from the Databricks app config, never from files in the repo.
- **Visibility:** `pub(crate)` is the default; `Settings` and its sections are `pub`.

## Testing

- **Unit tests pass a closure lookup** over a small map. Never set real environment variables in tests.
- **Cover:** defaults, each override, empty-as-unset, each invalid value (and that the error names the variable without the value), and `Debug` redaction of secrets.

## Coverage floor

No per-crate floor; this crate counts toward the global 80%.

## Example

```rust
let settings = Settings::from_lookup(|name| match name {
    "VAK_HTTP_ADDR" => Some("0.0.0.0:8080".to_owned()),
    _ => None,
})?;
assert_eq!(settings.http.addr.port(), 8080);
```
