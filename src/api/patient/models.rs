use crate::fhir;
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
    pub status: String,
    pub intent: String,
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
    pub status: String,
    pub code: Option<Code>,
    pub recorded_at: Option<String>,
    pub performers: Vec<String>,
    pub value: Option<ObservationValue>,
}

#[derive(Debug, Serialize)]
pub struct Procedure {
    pub id: String,
    pub status: String,
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
pub struct ResolvedDocument {
    pub id: String,
    pub status: String,
    pub date: String,
    pub author: Vec<String>,
    pub content_type: Option<String>,
    pub content: Option<String>,
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
            gender: if value.gender.is_empty() {
                None
            } else {
                Some(value.gender.clone())
            },
            birth_date: if value.birth_date.is_empty() {
                None
            } else {
                Some(value.birth_date.clone())
            },
            active: value.active,
        }
    }
}

impl From<&fhir::Condition> for Condition {
    fn from(value: &fhir::Condition) -> Self {
        let clinical_status = if let Some(coding) = value.clinical_status.coding.first() {
            coding.code.clone()
        } else {
            value.clinical_status.text.clone()
        };
        let verification_status = if let Some(coding) = value.verification_status.coding.first() {
            coding.code.clone()
        } else {
            value.verification_status.text.clone()
        };
        Condition {
            id: value.id.clone(),
            code: value.code.as_ref().map(Into::into),
            clinical_status,
            verification_status,
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
        Dosage {
            text: value.text.clone(),
            frequency: value.timing.as_ref().map(|t| t.repeat.frequency),
            period: value.timing.as_ref().map(|t| t.repeat.period),
            period_unit: value.timing.as_ref().map(|t| t.repeat.period_unit.clone()),
        }
    }
}

impl From<&fhir::MedicationRequest> for Medication {
    fn from(value: &fhir::MedicationRequest) -> Self {
        Medication {
            id: value.id.clone(),
            status: value.status.clone(),
            intent: value.intent.clone(),
            medication: Some((&value.medication_codeable_concept).into()),
            prescribed_at: value.authored_on.clone(),
            requester: match &value.requester.display {
                Some(r) => Some(r.clone()),
                None => value.requester.reference.clone(),
            },
            dosage: value.dosage_instruction.iter().map(Into::into).collect(),
        }
    }
}

impl From<&fhir::ObservationComponent> for ObservationComponent {
    fn from(value: &fhir::ObservationComponent) -> Self {
        ObservationComponent {
            code: Some((&value.code).into()),
            value: value.value_quantity.value.unwrap_or_default(),
            unit: value.value_quantity.unit.clone(),
        }
    }
}

impl From<&fhir::Observation> for Observation {
    fn from(value: &fhir::Observation) -> Self {
        Observation {
            id: value.id.clone(),
            status: value.status.clone(),
            code: Some((&value.code).into()),
            recorded_at: value.effective_date_time.clone(),
            performers: value
                .performer
                .as_ref()
                .unwrap_or(&Vec::new())
                .iter()
                .map(|r| match &r.display {
                    Some(d) => d.clone(),
                    None => r.reference.clone().unwrap_or_default(),
                })
                .collect(),
            // Intentional ordering by the best expected quality of data
            value: if let Some(qual) = &value.value_quantity {
                Some(ObservationValue::Quantity {
                    value: qual.value.unwrap_or_default(),
                    unit: qual.unit.clone(),
                })
            } else if let Some(text) = &value.value_string {
                Some(ObservationValue::Text {
                    value: text.clone(),
                })
            } else {
                value
                    .component
                    .as_ref()
                    .map(|comps| ObservationValue::Components {
                        items: comps.iter().map(Into::into).collect(),
                    })
            },
        }
    }
}

impl From<&fhir::Procedure> for Procedure {
    fn from(value: &fhir::Procedure) -> Self {
        Procedure {
            id: value.id.clone(),
            status: value.status.clone(),
            code: Some((&value.code).into()),
            performed_at: value.performed_date_time.clone(),
            performers: value
                .performer
                .iter()
                .map(|p| match &p.actor.display {
                    Some(d) => d.clone(),
                    None => p.actor.reference.as_ref().unwrap_or(&String::new()).clone(),
                })
                .collect(),
        }
    }
}

impl From<&fhir::Patient> for PatientSummary {
    fn from(value: &fhir::Patient) -> Self {
        PatientSummary {
            id: value.id.clone(),
            name: value.display_name(),
            gender: if value.gender.is_empty() {
                None
            } else {
                Some(value.gender.clone())
            },
            birth_date: if value.birth_date.is_empty() {
                None
            } else {
                Some(value.birth_date.clone())
            },
            active: Some(value.active),
        }
    }
}

#[must_use]
pub fn resolve_document(doc: &fhir::DocumentReference, store: &store::Store) -> ResolvedDocument {
    let attachment = doc.content.first().map(|c| &c.attachment);
    let binary_url = attachment.and_then(|a| (!a.url.is_empty()).then(|| a.url.clone()));
    let content_type = attachment.map(|a| a.content_type.clone());

    let content = binary_url.as_deref().and_then(|url| {
        let normalized_id = store::typed_url("Binary", url)
            .map(store::normalize_id)
            .unwrap_or_default();
        let binary = store.binaries.get(&normalized_id)?;
        STANDARD
            .decode(&binary.data)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
    });

    ResolvedDocument {
        id: doc.id.clone(),
        status: doc.status.clone(),
        date: doc.date.clone(),
        author: doc
            .author
            .iter()
            .filter_map(|a| a.reference.clone())
            .collect(),
        content_type,
        content,
        binary_url,
    }
}

impl From<&fhir::Condition> for PatientTimelineEntry {
    fn from(value: &fhir::Condition) -> Self {
        Self {
            date: value.onset_date_time.clone(),
            resource_type: "Condition",
            resource: serde_json::to_value(value).unwrap_or_default(),
        }
    }
}

impl From<&fhir::MedicationRequest> for PatientTimelineEntry {
    fn from(value: &fhir::MedicationRequest) -> Self {
        Self {
            date: value.authored_on.clone(),
            resource_type: "MedicationRequest",
            resource: serde_json::to_value(value).unwrap_or_default(),
        }
    }
}

impl From<&fhir::Observation> for PatientTimelineEntry {
    fn from(value: &fhir::Observation) -> Self {
        Self {
            date: value.effective_date_time.clone(),
            resource_type: "Observation",
            resource: serde_json::to_value(value).unwrap_or_default(),
        }
    }
}

impl From<&fhir::Procedure> for PatientTimelineEntry {
    fn from(value: &fhir::Procedure) -> Self {
        Self {
            date: value.performed_date_time.clone(),
            resource_type: "Procedure",
            resource: serde_json::to_value(value).unwrap_or_default(),
        }
    }
}
