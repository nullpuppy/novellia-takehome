use crate::api::patient::models::{
    Condition, DocumentSummary, Medication, Observation, PatientTimelineEntry, Procedure,
};
use crate::fhir;
use crate::fhir::ResourceType;

impl From<&fhir::Condition> for PatientTimelineEntry {
    fn from(value: &fhir::Condition) -> Self {
        Self {
            date: value.onset_date_time.clone(),
            resource_type: (&ResourceType::Condition).into(),
            resource: serde_json::to_value(Into::<Condition>::into(value)).unwrap_or_default(),
        }
    }
}

impl From<&fhir::MedicationRequest> for PatientTimelineEntry {
    fn from(value: &fhir::MedicationRequest) -> Self {
        Self {
            date: value.authored_on.clone(),
            resource_type: (&ResourceType::MedicationRequest).into(),
            resource: serde_json::to_value(Into::<Medication>::into(value)).unwrap_or_default(),
        }
    }
}

impl From<&fhir::Observation> for PatientTimelineEntry {
    fn from(value: &fhir::Observation) -> Self {
        Self {
            date: value.effective_date_time.clone(),
            resource_type: (&ResourceType::Observation).into(),
            resource: serde_json::to_value(Into::<Observation>::into(value)).unwrap_or_default(),
        }
    }
}

impl From<&fhir::Procedure> for PatientTimelineEntry {
    fn from(value: &fhir::Procedure) -> Self {
        Self {
            date: value.performed_date_time.clone(),
            resource_type: (&ResourceType::Procedure).into(),
            resource: serde_json::to_value(Into::<Procedure>::into(value)).unwrap_or_default(),
        }
    }
}

impl From<&fhir::DocumentReference> for PatientTimelineEntry {
    fn from(value: &fhir::DocumentReference) -> Self {
        Self {
            date: value.date.clone(),
            resource_type: (&ResourceType::DocumentReference).into(),
            resource: serde_json::to_value(Into::<DocumentSummary>::into(value))
                .unwrap_or_default(),
        }
    }
}
