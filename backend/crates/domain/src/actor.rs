//! Who performs an action: the signed-in operator.

/// The signed-in operator: the identity provider's stable user ID and the user's email.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Actor {
    user_id: String,
    email: String,
}

/// The identity is missing or malformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ActorError {
    #[error("the user ID is empty")]
    MissingUserId,
    #[error("the user ID is longer than {max} characters or contains a control character")]
    InvalidUserId { max: usize },
    #[error("the email address is not valid")]
    InvalidEmail,
}

impl Actor {
    /// Longest user ID, in characters.
    pub const MAX_USER_ID: usize = 256;
    /// Longest email address, in characters (RFC 5321 path limit).
    pub const MAX_EMAIL: usize = 254;

    /// Validates both parts after trimming surrounding whitespace.
    pub fn new(user_id: &str, email: &str) -> Result<Self, ActorError> {
        let user_id = user_id.trim();
        if user_id.is_empty() {
            return Err(ActorError::MissingUserId);
        }
        if user_id.chars().count() > Self::MAX_USER_ID || user_id.chars().any(char::is_control) {
            return Err(ActorError::InvalidUserId {
                max: Self::MAX_USER_ID,
            });
        }
        let email = email.trim();
        if !Self::is_email(email) {
            return Err(ActorError::InvalidEmail);
        }
        Ok(Self {
            user_id: user_id.to_owned(),
            email: email.to_owned(),
        })
    }

    /// A plausible address: one `@` between a non-empty local part and domain, no spaces or
    /// control characters, at most [`Self::MAX_EMAIL`] characters. The identity provider has
    /// verified it; this only rejects headers that are not addresses.
    fn is_email(email: &str) -> bool {
        let plain = email.chars().all(|c| !c.is_whitespace() && !c.is_control());
        match email.split_once('@') {
            Some((local, domain)) => {
                plain
                    && email.chars().count() <= Self::MAX_EMAIL
                    && !local.is_empty()
                    && !domain.is_empty()
                    && !domain.contains('@')
                    && !domain.starts_with('.')
                    && !domain.ends_with('.')
            }
            None => false,
        }
    }

    #[must_use]
    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_identity_provider_values() {
        let actor = Actor::new(" 7364019283746501 ", "ada@example.com").expect("valid");

        assert_eq!(actor.user_id(), "7364019283746501");
        assert_eq!(actor.email(), "ada@example.com");
        assert!(Actor::new("dev", "dev@localhost").is_ok());
    }

    #[test]
    fn rejects_missing_or_malformed_parts() {
        let long_id = "1".repeat(Actor::MAX_USER_ID + 1);
        let long_email = format!("{}@example.com", "a".repeat(Actor::MAX_EMAIL));

        assert_eq!(
            Actor::new(" ", "ada@example.com"),
            Err(ActorError::MissingUserId)
        );
        for user_id in [long_id.as_str(), "12\n34"] {
            assert_eq!(
                Actor::new(user_id, "ada@example.com"),
                Err(ActorError::InvalidUserId { max: 256 })
            );
        }
        for email in [
            "",
            "not an email",
            "ada",
            "@example.com",
            "ada@",
            "ada@@example.com",
            "ada@.example.com",
            "ada@example.com.",
            "ada@exa mple.com",
            long_email.as_str(),
        ] {
            assert_eq!(
                Actor::new("1", email),
                Err(ActorError::InvalidEmail),
                "{email}"
            );
        }
    }
}
