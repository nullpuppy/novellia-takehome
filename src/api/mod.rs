pub mod binary;
pub mod error;
pub mod patient;

use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
// --- Request Handlers ---

/// GET `/data-quality`
///
/// Returns parse and validation issues found while loading the dataset.
///
/// if no issues were found, returns an empty json array.
pub async fn get_data_quality(State(store): State<AppState>) -> impl IntoResponse {
    Json(store.quality_issues.clone())
}
