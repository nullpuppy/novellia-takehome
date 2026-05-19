//! FHIR R4 resource models.
//!
//! Each struct maps directly to a FHIR resource or reusable data type. Fields are
//! optional where the FHIR spec allows absence or where real-world clinic data has
//! been observed to omit them.

use serde::Deserialize;
use std::fmt::Formatter;
use std::str::FromStr;

/// Enum to distinguish between FHIR Resources
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
    Unknown {
        resource_type: Option<String>,
        id: Option<String>,
    },
}

/// All resource type names, allows conversion from and to strings
/// so we don't have error-prone strings all over the place
#[derive(Debug, Clone, Deserialize)]
pub enum ResourceType {
    // FHIR Standard Resource types
    Patient,
    Condition,
    MedicationRequest,
    Observation,
    Procedure,
    Binary,
    DocumentReference,

    // Non-standard/extensions
    ClinicalNote,

    /// invalid, new, or otherwise unseen/handled resource
    Unknown(String),
}

impl<'a> From<&'a ResourceType> for &'a str {
    fn from(value: &'a ResourceType) -> Self {
        match value {
            ResourceType::Patient => "Patient",
            ResourceType::Condition => "Condition",
            ResourceType::MedicationRequest => "MedicationRequest",
            ResourceType::Observation => "Observation",
            ResourceType::Procedure => "Procedure",
            ResourceType::Binary => "Binary",
            ResourceType::DocumentReference => "DocumentReference",
            ResourceType::ClinicalNote => "ClinicalNote",
            ResourceType::Unknown(resource_type) => resource_type.as_str(),
        }
    }
}

impl From<&ResourceType> for String {
    fn from(value: &ResourceType) -> Self {
        Into::<&str>::into(value).to_string()
    }
}

impl FromStr for ResourceType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        match s.to_lowercase().as_ref() {
            "patient" => Ok(ResourceType::Patient),
            "condition" => Ok(ResourceType::Condition),
            "medicationrequest" => Ok(ResourceType::MedicationRequest),
            "observation" => Ok(ResourceType::Observation),
            "procedure" => Ok(ResourceType::Procedure),
            "binary" => Ok(ResourceType::Binary),
            "documentreference" => Ok(ResourceType::DocumentReference),
            "clinicalnote" => Ok(ResourceType::ClinicalNote),
            _ => Ok(ResourceType::Unknown(s.to_string())),
        }
    }
}

impl core::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let resource_type = Into::<&str>::into(self);
        write!(f, "{resource_type}")
    }
}

// --- FHIR resource models ---

/// An individual receiving or registered for healthcare services.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Patient {
    pub id: String,
    pub name: Vec<Name>,
    pub gender: Option<String>,
    /// YYYY-MM-DD
    #[serde(alias = "birthDate")]
    pub birth_date: Option<String>,
    pub active: Option<bool>,
}

impl Patient {
    #[must_use]
    pub fn display_name(&self) -> Option<String> {
        self.name.first()?.display_name()
    }
}

/// A clinical condition, problem, or diagnosis associated with a patient.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Condition {
    pub id: String,
    #[serde(alias = "clinicalStatus")]
    pub clinical_status: Option<CodeableConcept>,
    #[serde(alias = "verificationStatus")]
    pub verification_status: Option<CodeableConcept>,
    pub code: Option<CodeableConcept>,
    pub subject: Reference,
    #[serde(alias = "onsetDateTime")]
    pub onset_date_time: Option<String>,
    #[serde(alias = "abatementDateTime")]
    pub abatement_date_time: Option<String>,
    pub recorder: Option<Reference>,
}

/// An order or request for a medication to be dispensed or administered to a patient.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct MedicationRequest {
    pub id: String,
    pub status: Option<String>,
    pub intent: Option<String>,
    #[serde(alias = "medicationCodeableConcept")]
    pub medication_codeable_concept: Option<CodeableConcept>,
    pub subject: Reference,
    #[serde(alias = "authoredOn")]
    pub authored_on: Option<String>,
    pub requester: Option<Reference>,
    #[serde(alias = "dosageInstruction", default)]
    pub dosage_instruction: Vec<DosageInstruction>,
}

/// A measurement, assessment, or simple assertion made about a patient (e.g., vitals, labs).
///
/// The value is represented by exactly one of `value_quantity`, `value_string`, or `component`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Observation {
    pub id: String,
    pub status: Option<String>,
    pub code: Option<CodeableConcept>,
    pub subject: Reference,
    #[serde(alias = "effectiveDateTime")]
    pub effective_date_time: Option<String>,
    #[serde(default)]
    pub performer: Vec<Reference>,
    #[serde(alias = "valueQuantity")]
    pub value_quantity: Option<Quantity>,
    #[serde(alias = "valueString")]
    pub value_string: Option<String>,
    /// Used for panel observations such as blood pressure (systolic + diastolic).
    #[serde(default)]
    pub component: Vec<ObservationComponent>,
}

/// An action that was performed on or for a patient (e.g. surgery, vaccination, exam).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Procedure {
    pub id: String,
    pub status: Option<String>,
    pub code: Option<CodeableConcept>,
    pub subject: Reference,
    #[serde(alias = "performedDateTime")]
    pub performed_date_time: Option<String>,
    pub performer: Vec<ProcedurePerformer>,
}

/// Raw binary content attached to a patient record, stored as base64.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Binary {
    pub id: String,
    #[serde(alias = "contentType")]
    pub content_type: Option<String>,
    /// base64-encoded content
    pub data: Option<String>,
}

/// A pointer to a clinical document (e.g. a consult note) with attachment metadata.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DocumentReference {
    pub id: String,
    pub status: Option<String>,
    #[serde(rename = "type")]
    pub ref_type: Option<CodeableConcept>,
    pub subject: Reference,
    pub date: Option<String>,
    #[serde(default)]
    pub author: Vec<Reference>,
    #[serde(default)]
    pub content: Vec<DocumentContent>,
}

/// A human name with a usage classification (e.g. "official", "nickname").
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Name {
    #[serde(rename = "use")]
    pub name_use: Option<String>,
    pub family: Option<String>,
    #[serde(default)]
    pub given: Vec<String>,
}

impl Name {
    #[must_use]
    pub fn display_name(&self) -> Option<String> {
        let given = self
            .given
            .iter()
            .filter_map(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let family = self.family.as_deref().unwrap_or_default();
        let full_name = format!("{given} {family}").trim().to_string();
        (!full_name.is_empty()).then_some(full_name)
    }
}

/// A concept that may be defined by one or more coded entries (FHIR [`CodeableConcept`]).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CodeableConcept {
    #[serde(default)]
    pub coding: Vec<Coding>,
    pub text: Option<String>,
}

impl CodeableConcept {
    #[must_use]
    pub fn as_summary(&self) -> Option<String> {
        self.coding
            .first()
            .and_then(|c| c.code.clone())
            .or_else(|| self.text.clone().filter(|t| !t.is_empty()))
    }
}

/// A single coded entry within a [`CodeableConcept`], identified by system and code.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Coding {
    pub system: Option<String>,
    pub code: Option<String>,
    pub display: Option<String>,
}

/// A reference to another FHIR resource. Both fields are optional because real-world
/// data may supply only a display name (no URL) or only a URL (no display).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Reference {
    /// Relative or absolute URL, e.g. "Patient/noah-wyle"
    pub reference: Option<String>,
    pub display: Option<String>,
}

impl Reference {
    #[must_use]
    fn typed_id(&self, resource_type: &str) -> Option<&str> {
        let (kind, id) = &self.reference.as_deref()?.split_once('/')?;
        kind.eq_ignore_ascii_case(resource_type).then_some(id)
    }

    #[must_use]
    pub fn patient_id(&self) -> Option<&str> {
        self.typed_id("Patient")
    }

    #[must_use]
    pub fn binary_id(&self) -> Option<&str> {
        self.typed_id("Binary")
    }
}

/// Dosage instructions for a medication, including optional structured timing.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DosageInstruction {
    pub text: Option<String>,
    pub timing: Option<DosageTiming>,
}

/// Wraps the repeat schedule for a dosage timing (mirrors FHIR Timing.repeat structure).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DosageTiming {
    pub repeat: Option<TimingRepeat>,
}

/// Frequency and period defining how often a medication dose repeats.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct TimingRepeat {
    pub frequency: Option<u16>,
    /// FHIR defines this as a decimal to support fractional periods (e.g. 1.5 days).
    pub period: Option<f32>,
    /// Unit code per UCUM: "s", "min", "h", "d", "wk", "mo", "a"
    #[serde(alias = "periodUnit")]
    pub period_unit: Option<String>,
}

/// A measured value with optional unit and coding system.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Quantity {
    pub value: Option<f64>,
    pub unit: Option<String>,
    pub system: Option<String>,
    pub code: Option<String>,
}

/// A single component of a multi-part observation (e.g. systolic or diastolic pressure).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ObservationComponent {
    pub code: Option<CodeableConcept>,
    #[serde(alias = "valueQuantity")]
    pub value_quantity: Option<Quantity>,
}

/// The actor who performed a procedure, wrapped to match FHIR's `performer[].actor` shape.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProcedurePerformer {
    pub actor: Option<Reference>,
}

/// An entry in `DocumentReference.content`, wrapping an attachment.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DocumentContent {
    pub attachment: Option<Attachment>,
}

/// A file or document accessible via URL, referenced from a [`DocumentReference`]e.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Attachment {
    #[serde(alias = "contentType")]
    pub content_type: Option<String>,
    pub url: Option<String>,
}

// === Non-standard resources === //

/// A free-text clinical note written by a provider. Non-standard FHIR resource type.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ClinicalNote {
    pub id: String,
    pub status: Option<String>,
    pub subject: Reference,
    pub author: Reference,
    pub date: Option<String>,
    pub content: Option<String>,
}

// === Other === //
impl FhirResource {
    /// Parse FHIR resource from a raw JSON blob
    ///
    /// Returns
    /// The specific [`FhirResource`] that this blob parses to, using the `resourceType` attribute.
    ///
    /// For blobs that do not contain a `resource_type` or one not seen before,
    /// a [`FhirResource::Unknown`] is returned.
    ///
    /// # Errors
    /// [`serde_json::Error`] Parse error occurred while reading and converting json blob to a
    /// [`serde_json::Value`]
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_str(s)?;
        let resource_type = value["resourceType"]
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
                resource_type: Some(resource_type.clone()),
                id: value["id"].as_str().map(String::from),
            },
        };

        Ok(resource)
    }
}
