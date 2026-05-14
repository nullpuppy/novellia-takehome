pub mod error;
pub mod patient;

use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;

// --- Request Handlers ---

pub async fn get_data_quality(State(store): State<AppState>) -> impl IntoResponse {
    Json(store.quality_issues.clone())
}
