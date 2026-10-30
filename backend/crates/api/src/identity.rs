//! Who is calling. On Databricks Apps the platform signs the user in and forwards their
//! identity in headers (`X-Forwarded-User`, `X-Forwarded-Email`); the app is reachable only
//! through that proxy, so the headers are trusted. Every `/api/v1` route runs behind
//! [`require_actor`], which turns them into an [`Actor`] or answers 401.

use axum::extract::{Request, State};
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::{IntoResponse as _, Response};
use vak_domain::Actor;

use crate::problem::ApiError;

/// The identity provider's stable user ID.
pub(crate) const USER_HEADER: &str = "x-forwarded-user";
/// The user's email address.
pub(crate) const EMAIL_HEADER: &str = "x-forwarded-email";

/// How callers are identified.
#[derive(Debug, Clone, Default)]
pub(crate) struct Identity {
    /// Local development only: the operator to assume when a request carries no identity
    /// headers. `None` (the default) means every request needs the headers.
    dev_operator: Option<Actor>,
}

impl Identity {
    pub(crate) fn with_dev_operator(dev_operator: Actor) -> Self {
        Self {
            dev_operator: Some(dev_operator),
        }
    }

    /// The caller: both headers when present (a malformed pair is rejected, never replaced),
    /// else the dev operator if one is configured.
    fn actor(&self, headers: &HeaderMap) -> Option<Actor> {
        let text = |name| headers.get(name).and_then(|value| value.to_str().ok());
        match (text(USER_HEADER), text(EMAIL_HEADER)) {
            (Some(user), Some(email)) => Actor::new(user, email).ok(),
            (None, None)
                if !headers.contains_key(USER_HEADER) && !headers.contains_key(EMAIL_HEADER) =>
            {
                self.dev_operator.clone()
            }
            _ => None,
        }
    }
}

/// Middleware: puts the caller's [`Actor`] in the request extensions, or answers 401.
pub(crate) async fn require_actor(
    State(identity): State<Identity>,
    mut request: Request,
    next: Next,
) -> Response {
    match identity.actor(request.headers()) {
        Some(actor) => {
            request.extensions_mut().insert(actor);
            next.run(request).await
        }
        None => ApiError::Unauthenticated.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn headers(pairs: &[(&'static str, &[u8])]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(*name, HeaderValue::from_bytes(value).expect("valid header"));
        }
        headers
    }

    fn dev() -> Identity {
        Identity::with_dev_operator(Actor::new("dev", "dev@localhost").expect("valid"))
    }

    #[test]
    fn forwarded_headers_identify_the_caller() {
        let pair = headers(&[(USER_HEADER, b"42"), (EMAIL_HEADER, b"ada@example.com")]);

        let actor = Identity::default().actor(&pair).expect("identified");

        assert_eq!((actor.user_id(), actor.email()), ("42", "ada@example.com"));
        assert_eq!(
            dev().actor(&pair),
            Some(actor),
            "headers win over the dev operator"
        );
    }

    #[test]
    fn without_headers_only_the_dev_operator_is_assumed() {
        assert_eq!(Identity::default().actor(&HeaderMap::new()), None);
        assert_eq!(
            dev()
                .actor(&HeaderMap::new())
                .map(|actor| actor.email().to_owned()),
            Some("dev@localhost".to_owned())
        );
    }

    #[test]
    fn partial_or_malformed_headers_never_fall_back_to_the_dev_operator() {
        for pair in [
            headers(&[(EMAIL_HEADER, b"ada@example.com")]),
            headers(&[(USER_HEADER, b"42")]),
            headers(&[(USER_HEADER, b"42"), (EMAIL_HEADER, b"not an email")]),
            headers(&[(USER_HEADER, b"42"), (EMAIL_HEADER, b"\xff@example.com")]),
            headers(&[(USER_HEADER, b"\xff"), (EMAIL_HEADER, b"\xfe")]),
        ] {
            assert_eq!(dev().actor(&pair), None, "{pair:?}");
        }
    }
}
