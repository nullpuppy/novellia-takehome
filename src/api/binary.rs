use crate::AppState;
use crate::api::error::AppError;
use crate::api::patient::models;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use tracing::warn;

/// GET `/binaries`
///
/// List all [`Binary`]s
pub async fn list_binaries(State(store): State<AppState>) -> impl IntoResponse {
    let binaries: Vec<models::Binary> = store
        .binaries
        .values()
        .map(Into::<models::Binary>::into)
        .collect();

    Json(binaries)
}

/// GET `/binaries/{binary_id}`
///
/// # Errors
/// [`AppError::NotFound`] binary could not be found
/// [`AppError::BadResource`] binary found, but could not be loaded, decoded, or multiple entries found
pub async fn get_binary(
    State(store): State<AppState>,
    Path(binary_id): Path<String>,
) -> Result<(StatusCode, HeaderMap, Vec<u8>), AppError> {
    let mut binaries = store
        .binaries
        .values()
        .filter(|binary| binary.id == binary_id);

    let Some(binary) = binaries.next() else {
        return Err(AppError::NotFound(format!(
            "no binary data found for binary '{binary_id}"
        )));
    };

    if binaries.next().is_some() {
        return Err(AppError::BadResource(format!(
            "multiple binaries found for '{binary_id}'"
        )));
    }

    let doc = Into::<models::DocumentSummary>::into(binary);
    let content_type = doc.content_type(&store).unwrap_or_default().parse();

    let mut headers = HeaderMap::new();
    if let Ok(content_type) = content_type {
        headers.append("content-type", content_type);
    } else {
        warn!("invalid or missing content-type for binary '{binary_id}'");
    }

    let content = doc.content(&store).map_err(|err| {
        AppError::BadResource(format!(
            "binary '{binary_id}' content could not be loaded: {err}"
        ))
    })?;

    Ok((StatusCode::OK, headers, content))
}
