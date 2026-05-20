use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::fmt::Formatter;
use tracing::error;

/// Errors returned by API handlers
#[derive(Debug)]
pub enum AppError {
    /// Requested resource(s) not found
    NotFound(String),
    /// Requested resource(s) exist, but data is invalid and cannot be returned
    BadResource(String),
    /// Internal Service Error
    ///
    /// Error(s)/Messages are logged server side, returned status code should be 500
    Internal(anyhow::Error),
}

impl core::fmt::Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let content = match self {
            Self::NotFound(msg) => format!("Not Found, msg: {msg}"),
            Self::BadResource(msg) => format!("Bad Resource, msg: {msg}"),
            Self::Internal(err) => format!("Internal, err: {err}"),
        };

        write!(f, "{content}")
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            Self::NotFound(msg) => {
                error!("not found: {msg}");
                (StatusCode::NOT_FOUND, msg)
            }
            Self::Internal(err) => {
                error!("internal error: {err:#}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
            }
            Self::BadResource(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        (status, Json(serde_json::json!({ "error": msg }))).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err)
    }
}
