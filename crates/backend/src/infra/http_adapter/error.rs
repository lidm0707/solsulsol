//! Central error -> HTTP response mapping via newtype (orphan-rule safe).

use axum::response::{IntoResponse, Response};
use pos_api::error::ApiError;

pub struct ApiErrorReponse(pub ApiError);

impl From<ApiError> for ApiErrorReponse {
    fn from(e: ApiError) -> Self {
        Self(e)
    }
}

impl IntoResponse for ApiErrorReponse {
    fn into_response(self) -> Response {
        let status = self.0.status();
        tracing::warn!(error = %self.0, "api error");
        (status, self.0.to_string()).into_response()
    }
}
