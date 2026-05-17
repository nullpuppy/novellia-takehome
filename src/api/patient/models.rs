use crate::fhir::{self, ResourceType};
use crate::store;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Serialize;
//                      //
// ======= DTOs ======= //
//                      //

#[derive(Debug, Default, Serialize)]
pub struct Code {
    pub system: Option<String>,
    pub code: Option<String>,
    pub display: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Patient {
    pub id: String,
    pub name: Option<String>,
    pub gender: Option<String>,
    pub birth_date: Option<String>,
    pub active: bool,
}

#[derive(Debug, Serialize)]
pub struct Condition {
    pub id: String,
    pub code: Option<Code>,
    pub clinical_status: Option<String>,
    pub verification_status: Option<String>,
    pub onset: Option<String>,
    pub abatement: Option<String>,
    pub recorder: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Dosage {
    pub text: Option<String>,
    pub frequency: Option<u16>,
    pub period: Option<f32>,
    pub period_unit: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Medication {
    pub id: String,
    pub status: Option<String>,
    pub intent: Option<String>,
    pub medication: Option<Code>,
    pub prescribed_at: Option<String>,
    pub requester: Option<String>,
    pub dosage: Vec<Dosage>,
}

#[derive(Debug, Serialize)]
pub struct ObservationComponent {
    pub code: Option<Code>,
    pub value: f64,
    pub unit: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObservationValue {
    Quantity { value: f64, unit: Option<String> },
    Text { value: String },
    Components { items: Vec<ObservationComponent> },
}

#[derive(Debug, Serialize)]
pub struct Observation {
    pub id: String,
    pub status: Option<String>,
    pub code: Option<Code>,
    pub recorded_at: Option<String>,
    pub performers: Vec<String>,
    pub value: Option<ObservationValue>,
}

#[derive(Debug, Serialize)]
pub struct Procedure {
    pub id: String,
    pub status: Option<String>,
    pub code: Option<Code>,
    pub performed_at: Option<String>,
    pub performers: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PatientSummary {
    pub id: String,
    pub name: Option<String>,
    pub gender: Option<String>,
    pub birth_date: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct Document {
    pub id: String,
    pub status: String,
    pub date: String,
    pub author: Vec<String>,
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<u8>>,
    pub binary_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PatientTimelineEntry {
    pub date: Option<String>,
    pub resource_type: &'static str,
    pub resource: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct PatientTimeline {
    pub patient: PatientSummary,
    pub timeline: Vec<PatientTimelineEntry>,
}

//                           //
// ======= DTO impls ======= //
//                           //

impl From<&fhir::CodeableConcept> for Code {
    fn from(value: &fhir::CodeableConcept) -> Self {
        Code {
            system: value
                .coding
                .first()
                .map(|code| code.system.clone())
                .unwrap_or_default(),
            code: value
                .coding
                .first()
                .map(|code| code.code.clone())
                .unwrap_or_default(),
            display: value.text.clone(),
        }
    }
}

impl From<&fhir::Patient> for Patient {
    fn from(value: &fhir::Patient) -> Self {
        Patient {
            id: value.id.clone(),
            name: value.display_name(),
            gender: value.gender.clone().filter(|s| !s.is_empty()),
            birth_date: value.birth_date.clone().filter(|s| !s.is_empty()),
            active: value.active.unwrap_or_default(),
        }
    }
}

fn codeable_concept_summary(cc: &fhir::CodeableConcept) -> Option<String> {
    cc.coding
        .first()
        .and_then(|c| c.code.clone())
        .or_else(|| cc.text.clone().filter(|t| !t.is_empty()))
}

impl From<&fhir::Condition> for Condition {
    fn from(value: &fhir::Condition) -> Self {
        Condition {
            id: value.id.clone(),
            code: value.code.as_ref().map(Into::into),
            clinical_status: value
                .clinical_status
                .as_ref()
                .and_then(codeable_concept_summary),
            verification_status: value
                .verification_status
                .as_ref()
                .and_then(codeable_concept_summary),
            onset: value.onset_date_time.clone(),
            abatement: value.abatement_date_time.clone(),
            recorder: match &value.recorder {
                None => None,
                Some(reference) => match &reference.display {
                    Some(r) => Some(r.clone()),
                    None => reference.reference.clone(),
                },
            },
        }
    }
}

impl From<&fhir::DosageInstruction> for Dosage {
    fn from(value: &fhir::DosageInstruction) -> Self {
        let (frequency, period, period_unit) = match &value.timing {
            None => (None, None, None),
            Some(dt) => match &dt.repeat {
                None => (None, None, None),
                Some(tr) => (tr.frequency, tr.period, tr.period_unit.clone()),
            },
        };

        Dosage {
            text: value.text.clone(),
            frequency,
            period,
            period_unit,
        }
    }
}

impl From<&fhir::MedicationRequest> for Medication {
    fn from(value: &fhir::MedicationRequest) -> Self {
        Medication {
            id: value.id.clone(),
            status: value.status.clone(),
            intent: value.intent.clone(),
            medication: value.medication_codeable_concept.as_ref().map(Into::into),
            prescribed_at: value.authored_on.clone(),
            requester: value
                .requester
                .as_ref()
                .and_then(|r| r.display.clone().or_else(|| r.reference.clone())),
            dosage: value.dosage_instruction.iter().map(Into::into).collect(),
        }
    }
}

impl From<&fhir::ObservationComponent> for ObservationComponent {
    fn from(value: &fhir::ObservationComponent) -> Self {
        ObservationComponent {
            code: value.code.as_ref().map(Into::into),
            value: value
                .value_quantity
                .as_ref()
                .and_then(|q| q.value)
                .unwrap_or_default(),
            unit: value.value_quantity.as_ref().and_then(|u| u.unit.clone()),
        }
    }
}

impl From<&fhir::Observation> for Observation {
    fn from(value: &fhir::Observation) -> Self {
        Observation {
            id: value.id.clone(),
            status: value.status.clone(),
            code: value.code.as_ref().map(Into::into),
            recorded_at: value.effective_date_time.clone(),
            performers: value
                .performer
                .iter()
                .map(|r| match &r.display {
                    Some(d) => d.clone(),
                    None => r.reference.clone().unwrap_or_default(),
                })
                .collect(),
            // Intentional ordering by the best expected quality of data
            value: if let Some(quality) = &value.value_quantity {
                Some(ObservationValue::Quantity {
                    value: quality.value.unwrap_or_default(),
                    unit: quality.unit.clone(),
                })
            } else if let Some(text) = &value.value_string {
                Some(ObservationValue::Text {
                    value: text.clone(),
                })
            } else if !value.component.is_empty() {
                Some(ObservationValue::Components {
                    items: value.component.iter().map(Into::into).collect(),
                })
            } else {
                None
            },
        }
    }
}

impl From<&fhir::Procedure> for Procedure {
    fn from(value: &fhir::Procedure) -> Self {
        Procedure {
            id: value.id.clone(),
            status: value.status.clone(),
            code: value.code.as_ref().map(Into::into),
            performed_at: value.performed_date_time.clone(),
            performers: value
                .performer
                .iter()
                .map(|p| {
                    p.actor
                        .as_ref()
                        .and_then(|r| r.display.clone().or_else(|| r.reference.clone()))
                        .unwrap_or_default()
                })
                .collect(),
        }
    }
}

impl From<&fhir::DocumentReference> for Document {
    fn from(value: &fhir::DocumentReference) -> Self {
        document_without_content(value)
    }
}

impl From<&fhir::Patient> for PatientSummary {
    fn from(value: &fhir::Patient) -> Self {
        PatientSummary {
            id: value.id.clone(),
            name: value.display_name(),
            gender: value.gender.clone().filter(|s| !s.is_empty()),
            birth_date: value.birth_date.clone().filter(|s| !s.is_empty()),
            active: value.active,
        }
    }
}

fn document_without_content(doc: &fhir::DocumentReference) -> Document {
    let attachment = doc.content.first().and_then(|c| c.attachment.as_ref());
    let binary_url = attachment
        .and_then(|a| a.url.clone())
        .filter(|s| !s.is_empty());
    let content_type = attachment
        .and_then(|a| a.content_type.clone())
        .filter(|s| !s.is_empty());

    Document {
        id: doc.id.clone(),
        status: doc.status.clone().unwrap_or_default(),
        date: doc.date.clone().unwrap_or_default(),
        author: doc
            .author
            .iter()
            .filter_map(|r| r.reference.clone())
            .collect(),
        content_type,
        content: None,
        binary_url,
    }
}

#[must_use]
pub fn decode_document_content(doc: &Document, store: &store::Store) -> Option<Vec<u8>> {
    doc.binary_url.as_ref().and_then(|url| {
        let normalized_id = store::typed_url("Binary", url)
            .map(store::normalize_id)
            .unwrap_or_default();
        let binary = store.binaries.get(&normalized_id)?;
        let data = binary.data.as_deref()?;
        STANDARD.decode(data).ok()
    })
}

#[must_use]
pub fn document_with_content(doc: &fhir::DocumentReference, store: &store::Store) -> Document {
    let mut resolved = document_without_content(doc);
    resolved.content = decode_document_content(&resolved, store);

    resolved
}

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
            resource: serde_json::to_value(Into::<Document>::into(value)).unwrap_or_default(),
        }
    }
}
