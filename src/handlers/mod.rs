pub mod patient;

use axum::response::IntoResponse;
use axum::extract::State;
use axum::Json;
use axum::http::StatusCode;
use crate::AppState;

// --- Error responses ---
fn not_found(msg: &str) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": msg,
        })),
    )
}

// --- Request Handlers ---

pub async fn get_data_quality(State(store): State<AppState>) -> impl IntoResponse {
    Json(store.quality_issues.clone())
}
