#![warn(clippy::pedantic)]

use axum::ServiceExt;
use novellia_takehome::route::build_router;
use novellia_takehome::store;
use std::sync::Arc;
use tower::Layer;
use tower_http::normalize_path::NormalizePathLayer;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug,axum::rejection=trace".into()),
        )
        .init();

    // Load data set
    let data_path = "data/backend-takehome-fhir-resources.txt";
    let store = store::Store::load(data_path)?;

    info!(
        "Loaded {} patients, {} data quality issues",
        &store.patients.len(),
        &store.quality_issues.len()
    );

    let state = Arc::new(store);

    let app = build_router(state);
    let app = NormalizePathLayer::trim_trailing_slash().layer(app);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3100").await?;
    axum::serve(
        listener,
        ServiceExt::<axum::extract::Request>::into_make_service(app),
    )
    .await?;

    Ok(())
}
