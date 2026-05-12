use tracing::info;

pub mod fhir;
pub mod store;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load data set
    let data_path = "backend-takehome-fhir-resources.txt";
    let store = store::Store::load(data_path.into())?;

    info!("Loaded {} patients",
        store.patients.len(),
    );

    Ok(())
}
