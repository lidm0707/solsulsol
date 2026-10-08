//! JWT auth extractor. Requires `AppServices` in state (via FromRef).

use axum::extract::FromRef;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use pos_api::error::ApiError;
use crate::app::service::use_cases::AppServices;
use uuid::Uuid;

pub const AUTH_HEADER: &str = "authorization";
pub const BEARER_PREFIX: &str = "Bearer ";

pub struct AuthUser {
    pub user_id: Uuid,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    AppServices: FromRef<S>,
{
    type Rejection = crate::infra::http_adapter::error::ApiErrorReponse;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let services = AppServices::from_ref(state);
        let header = parts
            .headers
            .get(AUTH_HEADER)
            .and_then(|v| v.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;
        let token = header
            .strip_prefix(BEARER_PREFIX)
            .ok_or(ApiError::Unauthorized)?;
        let user_id = services
            .tokens
            .verify(token)
            .await
            .map_err(|_| crate::infra::http_adapter::error::ApiErrorReponse(ApiError::Unauthorized))?;
        Ok(AuthUser { user_id })
    }
}
