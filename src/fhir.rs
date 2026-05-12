//! FHIR R4 resource models.
//!
//! Each struct maps directly to a FHIR resource or reusable data type. Fields are
//! optional where the FHIR spec allows absence or where real-world clinic data has
//! been observed to omit them.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum FhirResource {
    Patient(Patient),
    Condition(Condition),
    MedicationRequest(MedicationRequest),
    Observation(Observation),
    Procedure(Procedure),
    Binary(Binary),
    DocumentReference(DocumentReference),
    ClinicalNote(ClinicalNote),
    Unknown { resource_type: String, id: Option<String>},
}


/// An individual receiving or registered for healthcare services.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patient {
    pub id: String,
    pub name: Vec<Name>,
    pub gender: String,
    /// YYYY-MM-DD
    #[serde(rename = "birthDate")]
    pub birth_date: String,
    pub active: bool,
}

/// A clinical condition, problem, or diagnosis associated with a patient.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub id: String,
    #[serde(rename = "clinicalStatus")]
    pub clinical_status: CodeableConcept,
    #[serde(rename = "verificationStatus")]
    pub verification_status: CodeableConcept,
    pub code: Option<CodeableConcept>,
    pub subject: Reference,
    #[serde(rename = "onsetDateTime")]
    pub onset_date_time: Option<String>,
    #[serde(rename = "abatementDateTime")]
    pub abatement_date_time: Option<String>,
    pub recorder: Option<Reference>,
}

/// An order or request for a medication to be dispensed or administered to a patient.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationRequest {
    pub id: String,
    pub status: String,
    pub intent: String,
    #[serde(rename = "medicationCodeableConcept")]
    pub medication_codeable_concept: CodeableConcept,
    pub subject: Reference,
    #[serde(rename = "authoredOn")]
    pub authored_on: Option<String>,
    pub requester: Reference,
    #[serde(rename = "dosageInstruction", default)]
    pub dosage_instruction: Vec<DosageInstruction>,
}

/// A measurement, assessment, or simple assertion made about a patient (e.g., vitals, labs).
///
/// The value is represented by exactly one of `value_quantity`, `value_string`, or `component`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: String,
    pub status: String,
    pub code: CodeableConcept,
    pub subject: Reference,
    #[serde(rename = "effectiveDateTime")]
    pub effective_date_time: Option<String>,
    pub performer: Option<Vec<Reference>>,
    #[serde(rename = "valueQuantity")]
    pub value_quantity: Option<Quantity>,
    #[serde(rename = "valueString")]
    pub value_string: Option<String>,
    /// Used for panel observations such as blood pressure (systolic + diastolic).
    pub component: Option<Vec<ObservationComponent>>,
}

/// An action that was performed on or for a patient (e.g. surgery, vaccination, exam).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Procedure {
    pub id: String,
    pub status: String,
    pub code: CodeableConcept,
    pub subject: Reference,
    #[serde(rename = "performedDateTime")]
    pub performed_date_time: Option<String>,
    pub performer: Vec<ProcedurePerformer>,
}

/// Raw binary content attached to a patient record, stored as base64.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binary {
    pub id: String,
    #[serde(rename = "contentType")]
    pub content_type: String,
    /// base64-encoded content
    pub data: String,
}

/// A pointer to a clinical document (e.g. a consult note) with attachment metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentReference {
    pub id: String,
    pub status: String,
    #[serde(rename = "type")]
    pub ref_type: CodeableConcept,
    pub subject: Reference,
    pub date: String,
    #[serde(default)]
    pub author: Vec<Reference>,
    #[serde(default)]
    pub content: Vec<DocumentContent>,
}

/// A free-text clinical note written by a provider. Non-standard FHIR resource type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClinicalNote {
    pub id: String,
    pub status: String,
    pub subject: Reference,
    pub author: Reference,
    pub date: String,
    pub content: String,
}

/// A human name with a usage classification (e.g. "official", "nickname").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Name {
    #[serde(rename = "use")]
    pub name_use: String,
    pub family: String,
    #[serde(default)]
    pub given: Vec<String>,
}

/// A concept that may be defined by one or more coded entries (FHIR CodeableConcept).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeableConcept {
    #[serde(default)]
    pub coding: Vec<Coding>,
    pub text: Option<String>,
}

/// A single coded entry within a CodeableConcept, identified by system and code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coding {
    pub system: Option<String>,
    pub code: Option<String>,
    pub display: Option<String>,
}

/// A reference to another FHIR resource. Both fields are optional because real-world
/// data may supply only a display name (no URL) or only a URL (no display).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    /// Relative or absolute URL, e.g. "Patient/noah-wyle"
    pub reference: Option<String>,
    pub display: Option<String>,
}

impl Reference {
    pub fn patient_id(&self) -> Option<&str> {
        self.reference.as_deref()?.strip_prefix("Patient/")
    }
}

/// Dosage instructions for a medication, including optional structured timing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DosageInstruction {
    pub text: Option<String>,
    pub timing: Option<DosageTiming>,
}

/// Wraps the repeat schedule for a dosage timing (mirrors FHIR Timing.repeat structure).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DosageTiming {
    pub repeat: TimingRepeat,
}

/// Frequency and period defining how often a medication dose repeats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingRepeat {
    pub frequency: u16,
    /// FHIR defines this as a decimal to support fractional periods (e.g. 1.5 days).
    pub period: f32,
    /// Unit code per UCUM: "s", "min", "h", "d", "wk", "mo", "a"
    #[serde(rename = "periodUnit")]
    pub period_unit: String,
}

/// A measured value with optional unit and coding system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quantity {
    pub value: Option<f64>,
    pub unit: Option<String>,
    pub system: Option<String>,
    pub code: Option<String>,
}

/// A single component of a multi-part observation (e.g. systolic or diastolic pressure).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationComponent {
    pub code: CodeableConcept,
    #[serde(rename = "valueQuantity")]
    pub value_quantity: Quantity,
}

/// The actor who performed a procedure, wrapped to match FHIR's `performer[].actor` shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedurePerformer {
    pub actor: Reference,
}

/// An entry in `DocumentReference.content`, wrapping an attachment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentContent {
    pub attachment: Attachment,
}

/// A file or document accessible via URL, referenced from a DocumentReference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    #[serde(rename = "contentType")]
    pub content_type: String,
    pub url: String,
}

pub fn parse_resource(line: &str) -> Result<FhirResource, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(line)?;
    let resource_type = value["resource_type"]
        .as_str()
        .unwrap_or("unknown")
        .to_lowercase();

    let resource: FhirResource = match resource_type.as_str() {
        "patient" => FhirResource::Patient(serde_json::from_value(value)?),
        "condition" => FhirResource::Condition(serde_json::from_value(value)?),
        "medicationrequest" => FhirResource::MedicationRequest(serde_json::from_value(value)?),
        "observation" => FhirResource::Observation(serde_json::from_value(value)?),
        "procedure" => FhirResource::Procedure(serde_json::from_value(value)?),
        "binary" => FhirResource::Binary(serde_json::from_value(value)?),
        "documentreference" => FhirResource::DocumentReference(serde_json::from_value(value)?),
        "clinicalnote" => FhirResource::ClinicalNote(serde_json::from_value(value)?),
        _ => FhirResource::Unknown {
            resource_type: resource_type.to_string(),
            id: value["id"].as_str().map(String::from),
        }
    };

    Ok(resource)
}
