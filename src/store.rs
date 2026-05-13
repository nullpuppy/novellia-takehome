use crate::{audit, fhir};
use std::collections::HashMap;
use std::fs;
use tracing::error;

#[derive(Debug, Clone, Default)]
pub struct PatientRecord {
    pub patient: Option<fhir::Patient>,
    pub conditions: Vec<fhir::Condition>,
    pub medications: Vec<fhir::MedicationRequest>,
    pub observations: Vec<fhir::Observation>,
    pub procedures: Vec<fhir::Procedure>,
    pub documents: Vec<fhir::DocumentReference>,
    pub clinical_notes: Vec<fhir::ClinicalNote>,
}

pub struct Store {
    // Keyed on id
    pub patients: HashMap<String, PatientRecord>,
    // Keyed on id
    pub binaries: HashMap<String, fhir::Binary>,
    pub quality_issues: Vec<audit::DataQualityIssue>,
}

impl Store {
    pub fn load(path: std::path::PathBuf) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut resources: Vec<fhir::FhirResource> = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match fhir::FhirResource::from_json(line) {
                Ok(record) => resources.push(record),
                Err(err) => error!("line {}: parse error: {}", line_num, err),
            }
        }

        let quality_issues = audit::audit_data_quality(&resources);

        let mut store = Store {
            patients: HashMap::new(),
            binaries: HashMap::new(),
            quality_issues,
        };
        store.index(resources);

        Ok(store)
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
                    self.binaries.insert(b.id.clone(), b);
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
                    error!("unknown resource type {} ({:?})", resource_type, id);
                }
            }
        }
    }

    fn resolve_patient(&mut self, subject: &fhir::Reference) -> Option<&mut PatientRecord> {
        let patient_id = subject.patient_id()?.to_lowercase();
        self.patients.get_mut(&patient_id)
    }

    // -- Public methods ---

    pub fn get_patient(&self, id: &str) -> Option<&PatientRecord> {
        let key = id.to_lowercase();
        self.patients.get(&key)
    }
}
