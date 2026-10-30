# vak-config: typed settings

## Responsibility

This crate holds the application `Settings` with defaults and overrides:

- **Local and Databricks profiles:** switching between them is configuration only.
- **Validation at startup:** fail fast.
- **Secrets:** every secret is typed as `secrecy::SecretString`.

Today:

- **Shape:** `Settings { profile, http: { addr }, log: { filter }, database: { url: Option<SecretString> }, auth: { dev_operator_email: Option<String> } }`.
- **Defaults:** profile `local`, `127.0.0.1:3000`, filter `info`, no database URL, no dev operator.
- **Overrides:** `Settings::from_lookup(lookup)` applies these; an empty value counts as unset.

| Variable | Sets | Notes |
|---|---|---|
| `VAK_PROFILE` | `profile` | `local` (default) or `databricks`, any case |
| `VAK_HTTP_ADDR` | `http.addr` | `IP:port` |
| `VAK_LOG` | `log.filter` | `EnvFilter` syntax |
| `DATABASE_URL` | `database.url` | `postgres://…`; a secret (`Debug` prints `[REDACTED]`). The conventional name, so sqlx and psql read it too |
| `VAK_DEV_OPERATOR_EMAIL` | `auth.dev_operator_email` | Local development only: an error in any other profile, so a deployment can't fall back to a fixed identity |

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
