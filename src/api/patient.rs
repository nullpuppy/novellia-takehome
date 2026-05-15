pub mod models;

use crate::api::error::AppError;
use crate::store::{normalize_id, typed_url};
use crate::{AppState, fhir};
use axum::Json;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use models::{PatientSummary, PatientTimeline, PatientTimelineEntry, ResolvedDocument};

/// GET patients
///
/// List all patients in a single response. No pagination, no specific order.
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
/// # Return
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
/// Vec<[`models::Condition`]> serialized to json.
///
/// If nothing is found, an empty [Vec] serialized to json is returned.
///
/// # Errors
/// [`AppError::NotFound`] could not find a patient for the id requested
pub async fn get_patient_observations(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<models::Observation>>, AppError> {
    let record = match store.require_patient(&id) {
        Ok(r) => r,
        Err(err) => return Err(err),
    };

    let mut observations: std::collections::HashMap<(String, String), fhir::Observation> =
        std::collections::HashMap::new();

    for obs in &record.observations {
        let code_key = obs
            .code
            .coding
            .first()
            .and_then(|c| c.code.clone())
            .unwrap_or_default();
        let date_time_key = obs.effective_date_time.clone().unwrap_or_default();
        let entry = observations
            .entry((code_key, date_time_key))
            .or_insert_with(|| obs.clone());
        if obs.status == "amended" {
            *entry = obs.clone();
        }
    }

    let mut result: Vec<fhir::Observation> = observations.into_values().collect();
    result.sort_by(|a, b| {
        b.effective_date_time
            .as_deref()
            .unwrap_or_default()
            .cmp(a.effective_date_time.as_deref().unwrap_or_default())
    });
    Ok(Json(result.iter().map(Into::into).collect()))
}

/// GET patients/{id}/procedures
///
/// Get all procedures for a patient, most recent first
///
/// # Returns
/// Vec of [`models::Medication`] serialized to JSON
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
/// Vec of [`ResolvedDocument`] serialized to JSON
///
/// If nothing is found, an empty [Vec] serialized to json is returned.
///
/// # Errors
/// [`AppError::NotFound`] could not find a patient for the id requested
pub async fn get_patient_documents(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<ResolvedDocument>>, AppError> {
    let record = match store.require_patient(&id) {
        Ok(r) => r,
        Err(err) => return Err(err),
    };

    let mut docs: Vec<ResolvedDocument> = record
        .documents
        .iter()
        .map(|d| {
            let attachment = d.content.first().map(|c| &c.attachment);
            let binary_url = attachment.map(|a| a.url.clone());
            let content_type = attachment.map(|a| a.content_type.clone());

            let content = binary_url.as_deref().and_then(|url| {
                let normalized_id = typed_url("Binary", url)
                    .map(normalize_id)
                    .unwrap_or_default();
                let binary = store.binaries.get(&normalized_id)?;
                STANDARD
                    .decode(&binary.data)
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
            });

            ResolvedDocument {
                id: d.id.clone(),
                status: d.status.clone(),
                date: d.date.clone(),
                author: d
                    .author
                    .iter()
                    .filter_map(|a| a.reference.clone())
                    .collect(),
                content_type,
                content,
                binary_url,
            }
        })
        .collect();

    docs.sort_by(|a, b| b.date.cmp(&a.date));

    Ok(Json(docs))
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

    let patient_summary = match &record.patient {
        Some(p) => p.into(),
        None => PatientSummary {
            id: "unknown".to_string(),
            name: None,
            gender: None,
            birth_date: None,
            active: None,
        },
    };

    let mut timeline: PatientTimeline = PatientTimeline {
        patient: patient_summary,
        timeline: Vec::new(),
    };

    // Get all chronological patient data
    for c in &record.conditions {
        timeline.timeline.push(PatientTimelineEntry {
            date: c.onset_date_time.clone(),
            resource_type: "Condition",
            resource: serde_json::to_value(c).unwrap_or_default(),
        });
    }

    for m in &record.medications {
        timeline.timeline.push(PatientTimelineEntry {
            date: m.authored_on.clone(),
            resource_type: "MedicationRequest",
            resource: serde_json::to_value(m).unwrap_or_default(),
        });
    }

    for o in &record.observations {
        timeline.timeline.push(PatientTimelineEntry {
            date: o.effective_date_time.clone(),
            resource_type: "Observation",
            resource: serde_json::to_value(o).unwrap_or_default(),
        });
    }

    for p in &record.procedures {
        timeline.timeline.push(PatientTimelineEntry {
            date: p.performed_date_time.clone(),
            resource_type: "Procedure",
            resource: serde_json::to_value(p).unwrap_or_default(),
        });
    }

    // sort timeline entries using date (ISO8601 / YYYYMMDD), newest first
    timeline
        .timeline
        .sort_by(|a, b| b.date.as_deref().cmp(&a.date.as_deref()));

    Ok(Json(timeline))
}
