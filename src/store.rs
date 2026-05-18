use crate::api::error::AppError;
use crate::audit::DataQualityIssue;
use crate::{audit, fhir};
use std::collections::HashMap;
use std::fs;
use tracing::error;

/// All resources associated with a single patient
///
/// all observations loaded at startup are in observations
/// deduplicated records are referenced via `normalized_observations`
#[derive(Debug, Clone, Default)]
pub struct PatientRecord {
    pub patient: Option<fhir::Patient>,
    pub conditions: Vec<fhir::Condition>,
    pub medications: Vec<fhir::MedicationRequest>,
    pub observations: Vec<fhir::Observation>,
    pub normalized_observations: Vec<fhir::Observation>,
    pub procedures: Vec<fhir::Procedure>,
    pub documents: Vec<fhir::DocumentReference>,
    pub clinical_notes: Vec<fhir::ClinicalNote>,
}

/// In-memory data-store that holds `raw` parsed FHIR
///
/// All binary data is found in binaries, all patient data is associated with the given
/// patient in patients
pub struct Store {
    // Keyed on id
    pub patients: HashMap<String, PatientRecord>,
    // Keyed on id
    pub binaries: HashMap<String, fhir::Binary>,
    pub quality_issues: Vec<DataQualityIssue>,
}

impl Store {
    /// Loads resources from a JSONL file
    ///
    /// loaded resources are audited for quality issues with [result][`audit::DataQualityIssue`]s saved in
    /// `quality_issues` for future inspection via api call
    ///
    /// # Errors
    /// [`std::io::Error`] file could not be openned or read
    pub fn load(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut resources: Vec<fhir::FhirResource> = Vec::new();
        let mut issues: Vec<DataQualityIssue> = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match fhir::FhirResource::from_json(line) {
                Ok(record) => resources.push(record),
                Err(err) => {
                    issues.push(DataQualityIssue::ParseError {
                        line_num,
                        content: line.to_string(),
                        message: err.to_string(),
                    });
                    error!("line {}: parse error: {}", line_num, err);
                }
            }
        }

        issues.extend(audit::audit_data_quality(&resources));

        let mut store = Store {
            patients: HashMap::new(),
            binaries: HashMap::new(),
            quality_issues: issues,
        };
        store.index(resources);

        Ok(store)
    }

    /// Retrieve a [`PatientRecord`] for the requested patient by their id.
    ///
    /// # Errors
    /// [`AppError::NotFound`] patient does not exist
    pub fn require_patient(&self, id: &str) -> Result<&PatientRecord, AppError> {
        self.get_patient(id)
            .ok_or_else(|| AppError::NotFound(format!("patient '{id}' not found")))
    }

    fn index(&mut self, resources: Vec<fhir::FhirResource>) {
        for resource in &resources {
            if let fhir::FhirResource::Patient(p) = resource {
                self.patients
                    .entry(p.id.to_lowercase())
                    .or_default()
                    .patient = Some(p.clone());
            }
        }

        for resource in resources {
            match resource {
                fhir::FhirResource::Patient(_) => {}
                fhir::FhirResource::Condition(c) => {
                    if let Some(record) = self.resolve_patient(&c.subject) {
                        record.conditions.push(c);
                    }
                }
                fhir::FhirResource::MedicationRequest(m) => {
                    if let Some(record) = self.resolve_patient(&m.subject) {
                        record.medications.push(m);
                    }
                }
                fhir::FhirResource::Observation(o) => {
                    if let Some(record) = self.resolve_patient(&o.subject) {
                        record.observations.push(o);
                    }
                }
                fhir::FhirResource::Procedure(p) => {
                    if let Some(record) = self.resolve_patient(&p.subject) {
                        record.procedures.push(p);
                    }
                }
                fhir::FhirResource::Binary(b) => {
                    self.binaries.insert(normalize_id(&b.id), b);
                }
                fhir::FhirResource::DocumentReference(d) => {
                    if let Some(record) = self.resolve_patient(&d.subject) {
                        record.documents.push(d);
                    }
                }
                fhir::FhirResource::ClinicalNote(n) => {
                    if let Some(record) = self.resolve_patient(&n.subject) {
                        record.clinical_notes.push(n);
                    }
                }
                fhir::FhirResource::Unknown { resource_type, id } => {
                    let kind = resource_type.as_deref().unwrap_or("<missing>");
                    error!("unknown resource type {kind} ({id:?})");
                }
            }
        }
        self.patients.values_mut().for_each(|entry| {
            entry.normalize_observations();
        });
    }

    fn resolve_patient(&mut self, subject: &fhir::Reference) -> Option<&mut PatientRecord> {
        let patient_id = subject.patient_id().map(normalize_id)?;
        self.patients.get_mut(&patient_id)
    }

    // -- Public methods ---

    #[must_use]
    pub fn get_patient(&self, id: &str) -> Option<&PatientRecord> {
        let key = normalize_id(id);
        self.patients.get(&key)
    }
}

impl PatientRecord {
    fn normalize_observations(&mut self) {
        let mut by_key: HashMap<(String, String), &fhir::Observation> = HashMap::new();
        for obs in &self.observations {
            let code = obs
                .code
                .as_ref()
                .and_then(|cc| cc.coding.first())
                .and_then(|c| c.code.clone())
                .unwrap_or_default();
            let time = obs.effective_date_time.clone().unwrap_or_default();
            by_key
                .entry((code, time))
                .and_modify(|existing| {
                    if obs.status.as_deref() == Some("amended") {
                        *existing = obs;
                    }
                })
                .or_insert(obs);
        }

        self.normalized_observations = by_key.into_values().cloned().collect();
    }
}

/// normalizes id as lowercase. keys are inserted in sets/maps to allow normalized
/// case-insensitive lookup
#[must_use]
pub fn normalize_id(id: &str) -> String {
    id.to_lowercase()
}

/// extracts id from FHIR references in the format of ResourceType/ResourceId
#[must_use]
pub fn resource_id_from_typed_fhir_uri<'a>(resource_type: &str, url: &'a str) -> Option<&'a str> {
    let (kind, raw_id) = &url.split_once('/')?;
    kind.eq_ignore_ascii_case(resource_type).then_some(raw_id)
}
