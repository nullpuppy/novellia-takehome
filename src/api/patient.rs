pub mod document;
pub mod models;
pub mod timeline;

use crate::AppState;
use crate::api::error::AppError;
use crate::store::normalize_id;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use models::{DocumentSummary, PatientSummary, PatientTimeline};
use tracing::warn;

/// GET `/patients`
///
/// List all [`PatientSummary`]s, sorted by patient id
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

/// GET `/patients/{patient_id}`
///
/// Returns a single [`models::Patient`] by id
///
/// # Errors
/// [`AppError::NotFound`] Patient doesn't exist or could not be found
pub async fn get_patient(
    State(store): State<AppState>,
    Path(patient_id): Path<String>,
) -> Result<Json<models::Patient>, AppError> {
    let patient = store
        .require_patient(&patient_id)?
        .patient
        .as_ref()
        .ok_or_else(|| AppError::NotFound(format!("patient '{patient_id}' not found")))?;
    Ok(Json(patient.into()))
}

/// GET `/patients/{patient_id}/conditions`
///
/// Returns patient [`models::Condition`]s, newest first
///
/// # Errors
/// [`AppError::NotFound`] patient does not exist
pub async fn get_patient_conditions(
    State(store): State<AppState>,
    Path(patient_id): Path<String>,
) -> Result<Json<Vec<models::Condition>>, AppError> {
    let mut conditions = store.require_patient(&patient_id)?.conditions.clone();
    conditions.sort_by(|a, b| b.onset_date_time.cmp(&a.onset_date_time));
    Ok(Json(conditions.iter().map(Into::into).collect()))
}

/// GET `/patients/{patient_id}/conditions/{condition_id}`
///
/// Returns specified patient [`models::Condition`]
///
/// # Errors
/// [`AppError::NotFound`] patient does not exist
pub async fn get_patient_condition(
    State(store): State<AppState>,
    Path((patient_id, condition_id)): Path<(String, String)>,
) -> Result<Json<Vec<models::Condition>>, AppError> {
    let mut conditions: Vec<_> = store
        .require_patient(&patient_id)?
        .conditions
        .iter()
        .filter_map(|condition| {
            if condition_id == condition.id {
                Some(condition.clone())
            } else {
                None
            }
        })
        .collect();
    conditions.sort_by(|a, b| b.onset_date_time.cmp(&a.onset_date_time));

    if conditions.is_empty() {
        return Err(AppError::NotFound(
            "condition '{condition_id}' for patient '{id}' not found".into(),
        ));
    } else if conditions.len() > 1 {
        // Note: This should probably return either BadResource, or maybe Conflict
        // Just logging a warning for now and returning all found
        warn!("multiple conditions with '{condition_id}' for patient '{patient_id}' found");
    }

    Ok(Json(conditions.iter().map(Into::into).collect()))
}

/// GET `/patients/{patient_id}/medications`
///
/// Returns patient [`models::Medication`]s, newest first
///
/// # Errors
/// [`AppError::NotFound`] patient does not exist
pub async fn get_patient_medications(
    State(store): State<AppState>,
    Path(p_id): Path<String>,
) -> Result<Json<Vec<models::Medication>>, AppError> {
    let mut medications = store.require_patient(&p_id)?.medications.clone();
    medications.sort_by(|a, b| b.authored_on.cmp(&a.authored_on));
    Ok(Json(medications.iter().map(Into::into).collect()))
}

/// GET `/patients/{patient_id}/medications/{medication_id}`
///
/// Returns specified patient [`models::Medication`]
///
/// # Errors
/// [`AppError::NotFound`] patient does not exist
pub async fn get_patient_medication(
    State(store): State<AppState>,
    Path((patient_id, medication_id)): Path<(String, String)>,
) -> Result<Json<Vec<models::Medication>>, AppError> {
    let mut medications: Vec<_> = store
        .require_patient(&patient_id)?
        .medications
        .iter()
        .filter_map(|medication| {
            if medication_id == medication.id {
                Some(medication.clone())
            } else {
                None
            }
        })
        .collect();
    medications.sort_by(|a, b| b.authored_on.cmp(&a.authored_on));

    if medications.is_empty() {
        return Err(AppError::NotFound(format!(
            "medication '{medication_id}' for patient '{patient_id}' not found"
        )));
    } else if medications.len() > 1 {
        // Note: This should probably return either BadResource, or maybe Conflict
        // Just logging a warning for now and returning all found
        warn!("multiple medications with '{medication_id}' for patient '{patient_id}' found");
    }

    Ok(Json(medications.iter().map(Into::into).collect()))
}

/// GET `/patients/{patient_id}/observations`
///
/// Returns patient [`models::Observation`]s, newest first
///
/// # Errors
/// [`AppError::NotFound`] patient does not exist
pub async fn get_patient_observations(
    State(store): State<AppState>,
    Path(patient_id): Path<String>,
) -> Result<Json<Vec<models::Observation>>, AppError> {
    let record = store.require_patient(&patient_id)?;

    let mut observations = record.normalized_observations.clone();
    observations.sort_by(|a, b| {
        b.effective_date_time
            .as_deref()
            .unwrap_or_default()
            .cmp(a.effective_date_time.as_deref().unwrap_or_default())
    });
    Ok(Json(observations.iter().map(Into::into).collect()))
}

/// GET `/patients/{patient_id}/observation/{observation_id}`
///
/// Returns specified patient [`models::Observation`]
///
/// # Errors
/// [`AppError::NotFound`] patient does not exist
pub async fn get_patient_observation(
    State(store): State<AppState>,
    Path((patient_id, observation_id)): Path<(String, String)>,
) -> Result<Json<Vec<models::Observation>>, AppError> {
    let mut observations: Vec<_> = store
        .require_patient(&patient_id)?
        .observations
        .iter()
        .filter_map(|observation| {
            if observation_id == observation.id {
                Some(observation.clone())
            } else {
                None
            }
        })
        .collect();
    observations.sort_by(|a, b| b.effective_date_time.cmp(&a.effective_date_time));

    if observations.is_empty() {
        return Err(AppError::NotFound(format!(
            "condition '{observation_id}' for patient '{patient_id}' not found"
        )));
    } else if observations.len() > 1 {
        // Note: This should probably return either BadResource, or maybe Conflict
        // Just logging a warning for now and returning all found
        warn!("multiple observations with '{observation_id}' for patient '{patient_id}' found");
    }

    Ok(Json(observations.iter().map(Into::into).collect()))
}

/// GET `/patients/{patient_id}/procedures`
///
/// Returns patient [`models::Procedure`]s, newest first
///
/// # Errors
/// [`AppError::NotFound`] patient does not exist
pub async fn get_patient_procedures(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<models::Procedure>>, AppError> {
    let mut procedures = store.require_patient(&id)?.procedures.clone();
    procedures.sort_by(|a, b| b.performed_date_time.cmp(&a.performed_date_time));
    Ok(Json(procedures.iter().map(Into::into).collect()))
}

/// GET `/patients/{patient_id}/procedures/{procedure_id}`
///
/// Returns specified patient [`models::Procedure`]
///
/// # Errors
/// [`AppError::NotFound`] patient does not exist
pub async fn get_patient_procedure(
    State(store): State<AppState>,
    Path((patient_id, procedure_id)): Path<(String, String)>,
) -> Result<Json<Vec<models::Procedure>>, AppError> {
    let mut procedures: Vec<_> = store
        .require_patient(&patient_id)?
        .procedures
        .iter()
        .filter_map(|procedure| {
            if procedure_id == procedure.id {
                Some(procedure.clone())
            } else {
                None
            }
        })
        .collect();
    procedures.sort_by(|a, b| b.performed_date_time.cmp(&a.performed_date_time));

    if procedures.is_empty() {
        return Err(AppError::NotFound(format!(
            "procedure '{procedure_id}' for patient '{patient_id}' not found"
        )));
    } else if procedures.len() > 1 {
        // Note: This should probably return either BadResource, or maybe Conflict
        // Just logging a warning for now and returning all found
        warn!("multiple procedures with '{procedure_id}' for patient '{patient_id}' found");
    }

    Ok(Json(procedures.iter().map(Into::into).collect()))
}

/// GET `/patients/{patient_id}/documents`
///
/// Returns [`DocumentSummary`]s for a patient, newest first.
/// use GET /patients/{id}/documents/id to get the actual contents
///
/// # Errors
/// [`AppError::NotFound`] patient does not exist
pub async fn get_patient_documents(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let record = store.require_patient(&id)?;

    let mut docs: Vec<DocumentSummary> = record
        .documents
        .iter()
        .map(Into::<DocumentSummary>::into)
        .collect();

    docs.sort_by(|a, b| b.date.cmp(&a.date));

    Ok(Json(docs))
}

/// GET `/patients/{patient_id}/documents/{doc_id}`
///
/// Returns decoded binary content for a specific patient document
///
/// # Errors
/// [`AppError::NotFound`] patient and/or document does not exist
/// [`AppError::BadResource`] invalid required data in document or binary
pub async fn get_patient_document(
    State(store): State<AppState>,
    Path((id, doc_id)): Path<(String, String)>,
) -> Result<(StatusCode, HeaderMap, Vec<u8>), AppError> {
    let record = store.require_patient(&id)?;

    let mut matching_docs = record.documents.iter().filter(|d| {
        d.id == doc_id && d.subject.patient_id().map(normalize_id) == Some(normalize_id(&id))
    });

    let Some(doc_ref) = matching_docs.next() else {
        return Err(AppError::NotFound(format!("document '{doc_id}' not found")));
    };

    if matching_docs.next().is_some() {
        return Err(AppError::BadResource(format!(
            "multiple documents found for '{doc_id}'"
        )));
    }

    let doc = DocumentSummary::from(doc_ref);
    let content_type = doc.content_type(&store).unwrap_or_default().parse();

    let mut headers = HeaderMap::new();
    if let Ok(content_type) = content_type {
        headers.append("content-type", content_type);
    } else {
        warn!("invalid or missing content-type for document '{doc_id}'");
    }

    let content = doc.content(&store).map_err(|err| {
        AppError::BadResource(format!(
            "document '{doc_id}' content could not be loaded: {err}"
        ))
    })?;

    Ok((StatusCode::OK, headers, content))
}

/// GET `/patients/{patient_id}/timeline`
///
/// Returns a combined [`PatientTimeline`], newest resource first.
///
/// # Errors
/// [`AppError::NotFound`] patient does not exist
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
