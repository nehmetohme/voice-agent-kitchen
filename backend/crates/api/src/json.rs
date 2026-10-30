//! A JSON body extractor whose rejections are problem+json, not axum's plain text.

use axum::Json;
use axum::extract::{FromRequest, Request};
use serde::de::DeserializeOwned;

use crate::problem::ApiError;

/// Like [`axum::Json`], but a malformed body is a 400 problem, a body that breaks the schema
/// a 422 problem, and a missing `application/json` content type a 415 problem.
#[derive(Debug)]
pub(crate) struct JsonBody<T>(pub(crate) T);

impl<S, T> FromRequest<S> for JsonBody<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, ApiError> {
        let Json(value) = Json::<T>::from_request(request, state).await?;
        Ok(Self(value))
    }
}
