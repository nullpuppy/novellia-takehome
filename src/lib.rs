#![warn(clippy::pedantic, clippy::all, clippy::nursery)]

use std::sync::Arc;

pub mod api;
pub mod audit;
pub mod fhir;
pub mod route;
pub mod store;

pub type AppState = Arc<store::Store>;
