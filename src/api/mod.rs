pub mod error;
pub mod patient;

use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
// --- Request Handlers ---

/// GET data-quality
///
/// Returns a list of quality issues found while parsing and validating importing
/// datasets
///
/// if no issues were found, returns an empty json array.
pub async fn get_data_quality(State(store): State<AppState>) -> impl IntoResponse {
    Json(store.quality_issues.clone())
}
