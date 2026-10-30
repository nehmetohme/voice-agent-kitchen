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
        // CONTRACT-STUB
        Ok(Self {
            user_id: user_id.to_owned(),
            email: email.to_owned(),
        })
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
