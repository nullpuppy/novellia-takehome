use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tracing::error;

pub enum AppError {
    NotFound(String),
    /// Catch-all for unexpected errors. Logged at error level and returned to clients
    /// as a generic 500 — used by `?` propagation of `anyhow::Error` and by the panic
    /// recovery layer.
    Internal(anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Internal(err) => {
                error!("internal error: {err:#}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
            }
        };

        (status, Json(serde_json::json!({ "error": msg }))).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err)
    }
}
