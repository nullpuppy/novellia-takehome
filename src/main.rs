#![warn(clippy::pedantic, clippy::all, clippy::nursery)]

use axum::ServiceExt;
use novellia_takehome::route::build_router;
use novellia_takehome::store;
use std::sync::Arc;
use tower::Layer;
use tower_http::normalize_path::NormalizePathLayer;
use tracing::info;

const DEFAULT_DATA_PATH: &str = "data/backend-takehome-fhir-resources.jsonl";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug,axum::rejection=trace".into()),
        )
        .init();

    // Load data set
    let data_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_DATA_PATH.to_string());
    let store = store::Store::load(data_path)?;

    info!(
        "Loaded {} patients, {} data quality issues",
        &store.patients.len(),
        &store.quality_issues.len()
    );

    let state = Arc::new(store);

    let app = build_router(state);
    let app = NormalizePathLayer::trim_trailing_slash().layer(app);

    let addr = "0.0.0.0:3100";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("{} api running at {}", env!("CARGO_PKG_NAME"), addr);
    axum::serve(
        listener,
        ServiceExt::<axum::extract::Request>::into_make_service(app),
    )
    .await?;

    Ok(())
}
