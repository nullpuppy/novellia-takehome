pub mod error;
pub mod patient;

use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
// --- Request Handlers ---

/// GET data-quality
///
/// During data import on service startup, a record of data quality issues is saved.
/// This will return the detail of each problem encountered.
///
/// # Returns
/// Vec of [`DataQualityIssue`] serialized to json
///
/// If no issues were found, an empty [Vec] serialized to json is returned.
///
/// # Errors
/// [None]
pub async fn get_data_quality(State(store): State<AppState>) -> impl IntoResponse {
    Json(store.quality_issues.clone())
}
