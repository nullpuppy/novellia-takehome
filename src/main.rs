use api::patient;
use axum::ServiceExt;
use axum::routing::get;
use std::sync::Arc;
use tower::Layer;
use tower_http::normalize_path::NormalizePathLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

pub mod api;
pub mod audit;
pub mod fhir;
pub mod store;

pub type AppState = Arc<store::Store>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug,axum::rejection=trace".into()),
        )
        .init();

    // Load data set
    let data_path = "docs/backend-takehome-fhir-resources.txt";
    let store = store::Store::load(data_path.into())?;

    info!(
        "Loaded {} patients, {} data quality issues",
        &store.patients.len(),
        &store.quality_issues.len()
    );

    let state = Arc::new(store);

    let app = axum::Router::new()
        .route("/patients", get(patient::list_patients))
        .route("/patients/{id}", get(patient::get_patient))
        .route(
            "/patients/{id}/conditions",
            get(patient::get_patient_conditions),
        )
        .route(
            "/patients/{id}/medications",
            get(patient::get_patient_medications),
        )
        .route(
            "/patients/{id}/observations",
            get(patient::get_patient_observations),
        )
        .route(
            "/patients/{id}/procedures",
            get(patient::get_patient_procedures),
        )
        .route(
            "/patients/{id}/documents",
            get(patient::get_patient_documents),
        )
        .route(
            "/patients/{id}/timeline",
            get(patient::get_patient_timeline),
        )
        .route("/data-quality", get(api::get_data_quality))
        .layer(TraceLayer::new_for_http())
        .with_state(state);
    let app = NormalizePathLayer::trim_trailing_slash().layer(app);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3100").await?;
    axum::serve(
        listener,
        ServiceExt::<axum::extract::Request>::into_make_service(app),
    )
    .await?;

    Ok(())
}
