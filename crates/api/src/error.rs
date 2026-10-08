use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("not authorized")]
    Unauthorized,
    #[error("forbidden for role")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("invalid request: {0}")]
    Invalid(String),
    #[error("internal error")]
    Internal,
}

#[cfg(feature = "axum")]
mod axum_impl {
    use super::ApiError;
    use axum::Json;
    use axum::http::StatusCode;
    use axum::response::{IntoResponse, Response};
    use serde::Serialize;

    impl ApiError {
        #[must_use]
        pub const fn status(&self) -> StatusCode {
            match self {
                Self::Unauthorized => StatusCode::UNAUTHORIZED,
                Self::Forbidden => StatusCode::FORBIDDEN,
                Self::NotFound => StatusCode::NOT_FOUND,
                Self::Conflict(_) => StatusCode::CONFLICT,
                Self::Invalid(_) => StatusCode::BAD_REQUEST,
                Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct ErrorResponse {
        pub error: String,
    }

    impl IntoResponse for ApiError {
        fn into_response(self) -> Response {
            let status = self.status();
            let body = ErrorResponse {
                error: self.to_string(),
            };
            (status, Json(body)).into_response()
        }
    }
}
