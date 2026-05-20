use crate::api::error::AppError;
use crate::api::patient;
use crate::{AppState, api};
use axum::response::IntoResponse;
use axum::routing::get;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::trace::TraceLayer;

pub fn build_router(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/patients", get(patient::list_patients))
        .route("/patients/{id}", get(patient::get_patient))
        .route(
            "/patients/{id}/conditions",
            get(patient::get_patient_conditions),
        )
        .route(
            "/patients/{id}/conditions/{condition_id}",
            get(patient::get_patient_condition),
        )
        .route(
            "/patients/{id}/medications",
            get(patient::get_patient_medications),
        )
        .route(
            "/patients/{id}/medications/{medication_id}",
            get(patient::get_patient_medication),
        )
        .route(
            "/patients/{id}/observations",
            get(patient::get_patient_observations),
        )
        .route(
            "/patients/{id}/observations/{observation_id}",
            get(patient::get_patient_observation),
        )
        .route(
            "/patients/{id}/procedures",
            get(patient::get_patient_procedures),
        )
        .route(
            "/patients/{id}/procedures/{procedure_id}",
            get(patient::get_patient_procedure),
        )
        .route(
            "/patients/{id}/documents",
            get(patient::get_patient_documents),
        )
        .route(
            "/patients/{id}/documents/{doc_id}",
            get(patient::get_patient_document),
        )
        .route(
            "/patients/{id}/timeline",
            get(patient::get_patient_timeline),
        )
        .route("/binary", get(api::binary::list_binaries))
        .route("/binary/{binary_id}", get(api::binary::get_binary))
        // .route(
        //     "/resources/orphaned",
        //     resources::get_resources
        // )
        .route("/data-quality", get(api::get_data_quality))
        .layer(CatchPanicLayer::custom(|_| {
            AppError::Internal(anyhow::anyhow!("panic in handler")).into_response()
        }))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
