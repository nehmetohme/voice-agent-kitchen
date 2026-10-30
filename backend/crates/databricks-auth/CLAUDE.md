# vak-databricks-auth: OAuth M2M and Lakebase credentials

## Responsibility

This crate handles Databricks authentication for the service principal the app runs as:

- **OAuth M2M token cache:** client-credentials flow, proactive refresh before expiry, a single in-flight refresh shared by all callers.
- **Lakebase credentials:** short-lived Postgres credentials, where the OAuth token is the password and is refreshed hourly.

The adapters and `vak-repositories-pg` share it. Operators' own identities come from forwarded headers in `vak-api`, not from here.

## Dependencies

- **May depend on:** no workspace crate.
- **External crates:** `reqwest` with rustls (this crate is a `deny.toml` wrapper), `secrecy`, `serde`, `thiserror`, `tokio` (sync, time), `tracing`.
- **Must not depend on:** any other `vak-*` crate, sqlx, axum, openssl.
- The adapters, `vak-repositories-pg` and `vak-server` may depend on this crate.

## Conventions

- **Secrets:** every token, client secret and password is `secrecy::SecretString`. `expose_secret()` is called only where a header or connection option is built.
- **Logging:** log token expiry times and refresh outcomes, never token values, client secrets or full URLs with query strings.
- **Timeouts and retries:** every token request has a timeout. A refresh failure while the old token is still valid keeps the old token and retries with backoff.
- **Errors:** `AuthError { Timeout, Rejected, Unavailable }` (`thiserror`). Adapters map it to `ProviderError::Auth` or `Unavailable`.
- **Configuration:** host, client ID and secret come from a typed config struct passed in by `vak-server`. Never read the environment here.
- **Visibility:** `pub(crate)` is the default.

## Testing

- **wiremock** for the token endpoint: first fetch, cached reuse, refresh before expiry, concurrent callers sharing one refresh, timeout mapping.
- **Paused time:** `#[tokio::test(start_paused = true)]` drives expiry without real waiting.
- **Debug output:** assert that `Debug` output of every credential type contains `[REDACTED]`.

## Coverage floor

No per-crate floor; this crate counts toward the global 80%.

## Example

This shows the pattern:

```rust
pub struct TokenCache { /* client, credentials, RwLock<Option<CachedToken>> */ }

impl TokenCache {
    /// Returns a token valid for at least `margin`, refreshing once if needed.
    pub async fn token(&self, margin: Duration) -> Result<SecretString, AuthError> {
        if let Some(t) = self.cached_valid_for(margin).await {
            return Ok(t);
        }
        self.refresh().await
    }
}
```
