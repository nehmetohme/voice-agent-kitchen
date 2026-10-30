# vak-config: typed settings

## Responsibility

This crate holds the application `Settings` with defaults and overrides:

- **Local and Databricks profiles:** switching between them is configuration only.
- **Validation at startup:** fail fast.
- **Secrets:** every secret is typed as `secrecy::SecretString`.

Today:

- **Shape:** `Settings { http: HttpSettings { addr: SocketAddr }, log: LogSettings { filter: String } }`.
- **Defaults:** `Settings::default()` is `127.0.0.1:3000` with filter `info`.
- **Overrides:** `Settings::from_lookup(lookup)` applies `VAK_HTTP_ADDR` and `VAK_LOG`; an empty value counts as unset.

## Dependencies

- **May depend on:** no workspace crate.
- **External crates:** `thiserror`, and later `serde`, `secrecy` and `config` (layered files).
- **Must not depend on:** any other `vak-*` crate, tokio, axum, sqlx, reqwest.
- Only `vak-server` (and the dev-only `vak-model-shim`) may depend on this crate. Inner crates receive their own typed config structs from the composition root.

## Conventions

- **Never read the process environment here.** `vak-server` passes a lookup function (`|name| std::env::var(name).ok()`), which keeps loading pure and testable.
- **Variable names** are `VAK_*`. Declare each one as a `const` and list it in the doc comment of the field it sets.
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
