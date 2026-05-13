use axum::routing::get;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::info;

pub mod fhir;
pub mod handlers;
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

    info!("Loaded {} patients", &store.patients.len());

    let state = Arc::new(store);

    let app = axum::Router::new()
        .route("/patients", get(handlers::list_patients))
        .route("/patient/{id}", get(handlers::get_patient))
        .route(
            "/patient/{id}/conditions",
            get(handlers::get_patient_conditions),
        )
        .route(
            "/patient/{id}/medications",
            get(handlers::get_patient_medications),
        )
        .route(
            "/patient/{id}/observations",
            get(handlers::get_patient_observations),
        )
        .route(
            "/patient/{id}/procedures",
            get(handlers::get_patient_procedures),
        )
        .route(
            "/patient/{id}/documents",
            get(handlers::get_patient_documents),
        )
        .route(
            "/patient/{id}/timeline",
            get(handlers::get_patient_timeline),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3100").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
