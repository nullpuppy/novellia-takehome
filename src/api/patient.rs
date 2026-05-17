pub mod models;
pub mod document;

use crate::AppState;
use crate::api::error::AppError;
use crate::store::normalize_id;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use models::{Document, PatientSummary, PatientTimeline, document_with_content};
use tracing::{error, warn};

/// GET patients
///
/// List all patients in a single response. No pagination, and the
/// most recent patient first.
///
/// # Returns
/// Vec<[`PatientSummary`]> serialized to json.
///
/// If nothing is found, an empty [Vec] serialized to json is returned.
///
/// # Errors
/// None
pub async fn list_patients(State(store): State<AppState>) -> impl IntoResponse {
    let mut summaries: Vec<PatientSummary> = store
        .patients
        .values()
        .filter_map(|r| {
            let p = r.patient.as_ref()?;
            Some(p.into())
        })
        .collect();

    summaries.sort_by(|a, b| a.id.cmp(&b.id));
    Json(summaries)
}

/// GET patients/{id}
///
/// Get a specific patient by the patient's id.
///
/// # Returns
/// [`models::Patient`] serialized to JSON
///
/// # Errors
/// [`AppError::NotFound`] Patient doesn't exist or could not be found
pub async fn get_patient(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<models::Patient>, AppError> {
    let patient = store
        .require_patient(&id)?
        .patient
        .as_ref()
        .ok_or_else(|| AppError::NotFound(format!("patient '{id}' not found")))?;
    Ok(Json(patient.into()))
}

/// GET patients/{id}/conditions
///
/// Get all conditions for a patient, most recent first
///
/// # Returns
/// Vec<[`models::Condition`]> serialized to json.
///
/// If nothing is found, an empty [Vec] serialized to json is returned.
///
/// # Errors
/// [`AppError::NotFound`] could not find a patient for the id requested
pub async fn get_patient_conditions(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<models::Condition>>, AppError> {
    let mut conditions = store.require_patient(&id)?.conditions.clone();
    conditions.sort_by(|a, b| b.onset_date_time.cmp(&a.onset_date_time));
    Ok(Json(conditions.iter().map(Into::into).collect()))
}

/// GET patients/{id}/medications
///
/// Get all medications for a patient, most recent first
///
/// # Returns
/// Vec of [`models::Medication`] serialized to JSON
///
/// If nothing is found, an empty [Vec] serialized to json is returned.
///
/// # Errors
/// [`AppError::NotFound`] could not find a patient for the id requested
pub async fn get_patient_medications(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<models::Medication>>, AppError> {
    let mut medications = store.require_patient(&id)?.medications.clone();
    medications.sort_by(|a, b| b.authored_on.cmp(&a.authored_on));
    Ok(Json(medications.iter().map(Into::into).collect()))
}

/// GET patients/{id}/observations
///
/// Get all observations for a patient, most recent first
///
/// # Returns
/// Vec<[`models::Observation`]> serialized to json.
///
/// If nothing is found, an empty [Vec] serialized to json is returned.
///
/// # Errors
/// [`AppError::NotFound`] could not find a patient for the id requested
pub async fn get_patient_observations(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<models::Observation>>, AppError> {
    let record = store.require_patient(&id)?;

    let mut observations = record.normalized_observations.clone();
    observations.sort_by(|a, b| {
        b.effective_date_time
            .as_deref()
            .unwrap_or_default()
            .cmp(a.effective_date_time.as_deref().unwrap_or_default())
    });
    Ok(Json(observations.iter().map(Into::into).collect()))
}

/// GET patients/{id}/procedures
///
/// Get all procedures for a patient, most recent first
///
/// # Returns
/// Vec of [`models::Procedure`] serialized to JSON
///
/// If nothing is found, an empty [Vec] serialized to json is returned.
///
/// # Errors
/// [`AppError::NotFound`] could not find a patient for the id requested
pub async fn get_patient_procedures(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<models::Procedure>>, AppError> {
    let mut procedures = store.require_patient(&id)?.procedures.clone();
    procedures.sort_by(|a, b| b.performed_date_time.cmp(&a.performed_date_time));
    Ok(Json(procedures.iter().map(Into::into).collect()))
}

/// GET patients/{id}/documents
///
/// Get all documents for a patient without pagination and in no particular order.
///
/// # Returns
/// Vec of [`Document`] serialized to JSON
///
/// If nothing is found, an empty [Vec] serialized to json is returned.
///
/// # Errors
/// [`AppError::NotFound`] could not find a patient for the id requested
pub async fn get_patient_documents(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let record = store.require_patient(&id)?;

    let mut docs: Vec<Document> = record
        .documents
        .iter()
        .map(|d| document_with_content(d, &store))
        .collect();

    docs.sort_by(|a, b| b.date.cmp(&a.date));

    Ok(Json(docs))
}

#[allow(clippy::doc_markdown)]
/// GET patients/{id}/documents/{doc_id}
///
/// Get all documents for a patient without pagination and in no particular order.
///
/// # Returns
/// Vec of [`Document`] serialized to JSON
///
/// If nothing is found, an empty [Vec] serialized to json is returned.
///
/// # Errors
/// [`AppError::NotFound`] could not find a patient for the id requested
///
/// # Panics
///
pub async fn get_patient_document(
    State(store): State<AppState>,
    Path((id, doc_id)): Path<(String, String)>,
) -> Result<(StatusCode, HeaderMap, Vec<u8>), AppError> {
    let record = store.require_patient(&id)?;

    let filtered_docs: Vec<Document> = record
        .documents
        .iter()
        .filter_map(|d| {
            (d.id == doc_id && d.subject.patient_id().map(normalize_id) == Some(normalize_id(&id)))
                .then_some(Into::<Document>::into(d))
        })
        .collect();

    if filtered_docs.is_empty() {
        error!("document {}", doc_id);
        return Err(AppError::NotFound(format!(
            "document '{doc_id}' not found -- "
        )));
    } else if filtered_docs.len() > 1 {
        // ?? what do?
        warn!("multiple documents found for '{doc_id}'");
        return Err(AppError::BadResource(format!(
            "multiple documents found for '{doc_id}'"
        )));
    }

    let doc = filtered_docs.first().ok_or(AppError::Internal(
        AppError::BadResource(format!("document '{doc_id}' found, but could not load")).into(),
    ))?;

    let binary = doc
        .binary_url
        .clone()
        .ok_or(AppError::BadResource(format!(
            "invalid or missing binary on document '{doc_id}'"
        )))?
        .split_once('/')
        .map(|(_, binary_id)| store.binaries.get(&normalize_id(binary_id)))
        .and_then(|b| b)
        .ok_or(AppError::BadResource(format!(
            "missing binary '{:?}' for document '{doc_id}'",
            doc.binary_url
        )))?;

    let content_type = binary.content_type.clone().unwrap_or_default().parse();
    let content =
        models::decode_document_content(doc, &store).ok_or(AppError::BadResource(format!(
            "missing binary '{:?}' for document '{doc_id}'",
            doc.binary_url
        )))?;

    let mut headers = HeaderMap::new();
    if let Ok(content_type) = content_type {
        headers.append("content-type", content_type);
    }

    Ok((StatusCode::OK, headers, content))
}

/// GET patients/{id}/timeline
///
/// Get all data related to a patient, sorted in descending chronological order.
///
/// # Returns
/// Vec of [`PatientTimeline`] serialized to json
///
/// If nothing is found, an empty [Vec] serialized to json is returned.
///
/// # Errors
/// [`AppError::NotFound`] could not find a patient for the id requested
pub async fn get_patient_timeline(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PatientTimeline>, AppError> {
    let record = store.require_patient(&id)?;

    let patient_summary: PatientSummary = record
        .patient
        .as_ref()
        .ok_or_else(|| AppError::NotFound(format!("patient '{id}' not found")))?
        .into();

    let mut timeline: PatientTimeline = PatientTimeline {
        patient: patient_summary,
        timeline: Vec::new(),
    };

    // Get all chronological patient data
    timeline
        .timeline
        .extend(record.conditions.iter().map(Into::into));
    timeline
        .timeline
        .extend(record.medications.iter().map(Into::into));
    timeline
        .timeline
        .extend(record.normalized_observations.iter().map(Into::into));
    timeline
        .timeline
        .extend(record.procedures.iter().map(Into::into));
    timeline
        .timeline
        .extend(record.documents.iter().map(Into::into));

    // sort timeline entries using date (ISO8601 / YYYYMMDD), newest first
    timeline
        .timeline
        .sort_by(|a, b| b.date.as_deref().cmp(&a.date.as_deref()));

    Ok(Json(timeline))
}
