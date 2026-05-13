use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use axum::response::IntoResponse;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Serialize;
use crate::{fhir, AppState};

// --- Response Types ---

#[derive(Serialize)]
struct PatientSummary {
    id: String,
    name: Option<String>,
    gender: Option<String>,
    birth_date: Option<String>,
    active: Option<bool>,
}

#[derive(Serialize)]
struct ResolvedDocument {
    id: String,
    status: String,
    date: String,
    author: Vec<String>,
    content_type: Option<String>,
    content: Option<String>,
    binary_url: Option<String>,
}

#[derive(Serialize)]
struct PatientTimelineEntry {
    date: Option<String>,
    resource_type: &'static str,
    resource: serde_json::Value,
}

#[derive(Serialize)]
struct PatientTimeline {
    patient: PatientSummary,
    timeline: Vec<PatientTimelineEntry>,
}

// --- Error responses ---

fn not_found(msg: &str) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Json(serde_json::json!({
        "error": msg,
    })))
}

// --- Request Handlers ---

/// /patients -- List all patients in a single response. No pagination.
pub async fn list_patients(State(store): State<AppState>) -> impl IntoResponse {
    let mut summaries:  Vec<PatientSummary> = store
        .patients
        .values()
        .filter_map(|r| {
            let p = r.patient.as_ref()?;
            Some(PatientSummary {
                id: p.id.clone(),
                name: p.display_name(),
                gender: None,
                birth_date: None,
                active: None,
            })
        })
        .collect();

    summaries.sort_by(|a, b| a.id.cmp(&b.id));
    Json(summaries)
}

/// /patient/{id} -- Get a specific, requested patient by the patient's id.
/// returns 404 not found if patient isn't in our dataset
pub async fn get_patient(State(store): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match store.get_patient(&id).and_then(|r| r.patient.as_ref()) {
        Some(r) => Json(serde_json::to_value(r).unwrap()).into_response(),
        None => not_found(&format!("patient '{}' not found", id)).into_response(),
    }
}

pub async fn get_patient_conditions(State(store): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match store.get_patient(&id) {
        Some(r) => Json(serde_json::to_value(&r.conditions).unwrap()).into_response(),
        None => not_found(&format!("patient '{}' not found", id)).into_response(),
    }
}

pub async fn get_patient_medications(State(store): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match store.get_patient(&id) {
        Some(r) => Json(serde_json::to_value(&r.medications).unwrap()).into_response(),
        None => not_found(&format!("patient '{}' not found", id)).into_response(),
    }
}

pub async fn get_patient_observations(State(store): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let record = match store.get_patient(&id) {
        Some(r) => r,
        None => {
            return not_found(&format!("patient '{}' not found", id)).into_response()
        },
    };

    let mut observations: std::collections::HashMap<(String, String), &fhir::Observation> =
        std::collections::HashMap::new();

    for obs in &record.observations {
        let code_key = obs
            .code
            .coding
            .first()
            .and_then(|c| c.code.clone())
            .unwrap_or_default();
        let date_time_key = obs.effective_date_time.clone().unwrap_or_default();
        let entry = observations.entry((code_key, date_time_key))
            .or_insert(obs);
        if obs.status == "amended" {
            *entry = obs;
        }
    }

    let mut result: Vec<&fhir::Observation> = observations.values().copied().collect();
    result.sort_by_key(|o| o.effective_date_time.as_deref().unwrap_or(""));
    Json(result).into_response()
}

pub async fn get_patient_procedures(State(store): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match store.get_patient(&id) {
        Some(r) => Json(serde_json::to_value(&r.procedures).unwrap()).into_response(),
        None => not_found(&format!("patient '{}' not found", id)).into_response(),
    }
}
pub async fn get_patient_documents(State(store): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let record = match store.get_patient(&id) {
        Some(r) => r,
        None => {
            return not_found(&format!("patient '{}' not found", id)).into_response()
        },
    };

    let docs: Vec<ResolvedDocument> = record
        .documents
        .iter()
        .map(|d| {
            let attachment = d.content.first().map(|c| &c.attachment);
            let binary_url = attachment.map(|a| a.url.clone());
            let content_type = attachment.map(|a| a.content_type.clone());

            let content = binary_url.as_deref().and_then(|url| {
                let binary_id = url.strip_prefix("Binary/")?;
                let binary = store.binaries.get(binary_id)?;
                STANDARD.decode(&binary.data).ok().and_then(|bytes| String::from_utf8(bytes).ok())
            });

            ResolvedDocument {
                id: d.id.clone(),
                status: d.status.clone(),
                date: d.date.clone(),
                author: d.author.iter().filter_map(|a| a.reference.clone()).collect(),
                content_type,
                content,
                binary_url,
            }
        })
        .collect();

    Json(docs).into_response()
}

pub async fn get_patient_timeline(State(store): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let record = match store.get_patient(&id) {
        Some(r) => r,
        _ => {
            return not_found(&format!("patient '{}' timeline not found", id)).into_response()
        }};

    let patient_summary = match &record.patient {
        Some(p) => {
            PatientSummary {
                id: p.id.clone(),
                name: p.display_name(),
                gender: None,
                birth_date: None,
                active: None,
            }
        },
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
            resource: serde_json::to_value(c).unwrap_or_default()
        })
    }

    for m in &record.medications {
        timeline.timeline.push(PatientTimelineEntry {
            date: m.authored_on.clone(),
            resource_type: "MedicationRequest",
            resource: serde_json::to_value(m).unwrap_or_default()
        })
    }

    for o in &record.observations {
        timeline.timeline.push(PatientTimelineEntry {
            date: o.effective_date_time.clone(),
            resource_type: "Observation",
            resource: serde_json::to_value(o).unwrap_or_default()
        })
    }

    for p in &record.procedures {
        timeline.timeline.push(PatientTimelineEntry {
            date: p.performed_date_time.clone(),
            resource_type: "Procedure",
            resource: serde_json::to_value(p).unwrap_or_default()
        })
    }

    // sort timeline entries using date (ISO8601 / YYYYMMDD), newest first
    timeline.timeline.sort_by(|a, b| b.date.as_deref().cmp(&a.date.as_deref()));

    Json(timeline).into_response()
}
