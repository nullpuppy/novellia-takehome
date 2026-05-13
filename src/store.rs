use std::collections::HashMap;
use std::fs;
use tracing::error;
use crate::fhir;
use crate::fhir::{Condition, MedicationRequest, Observation, Patient, Binary, Procedure, DocumentReference, ClinicalNote, FhirResource};

#[derive(Debug, Clone, Default)]
pub struct PatientRecord {
    pub patient: Option<Patient>,
    pub conditions: Vec<Condition>,
    pub medications: Vec<MedicationRequest>,
    pub observations: Vec<Observation>,
    pub procedures: Vec<Procedure>,
    pub documents: Vec<DocumentReference>,
    pub clinical_notes: Vec<ClinicalNote>,
}

pub struct Store {
    // Keyed on id
    pub patients: HashMap<String, PatientRecord>,
    // Keyed on id
    pub binaries: HashMap<String, Binary>,
}

impl Store {
    pub fn load(path: std::path::PathBuf) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut resources: Vec<FhirResource> = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue
            }
            match fhir::parse_resource(line) {
                Ok(record) => resources.push(record),
                Err(err) => error!("line {}: parse error: {}", line_num, err),
            }
        }

        let mut store = Store {
            patients: HashMap::new(),
            binaries: HashMap::new(),
        };
        store.index(resources);

        Ok(store)

    }

    fn index(&mut self, resources: Vec<FhirResource>) {
        for resource in &resources {
            if let FhirResource::Patient(p) = resource {
                self.patients
                    .entry(p.id.to_lowercase())
                    .or_default()
                    .patient = Some(p.clone());
            }
        }

        for resource in resources {
            match resource {
                FhirResource::Patient(_) => {}
                FhirResource::Condition(c) => {
                    if let Some(record) = self.resolve_patient(&c.subject) {
                        record.conditions.push(c);
                    }
                }
                FhirResource::MedicationRequest(m) => {
                    if let Some(record) = self.resolve_patient(&m.subject) {
                        record.medications.push(m);
                    }
                }
                FhirResource::Observation(o) => {
                    if let Some(record) = self.resolve_patient(&o.subject) {
                        record.observations.push(o);
                    }
                }
                FhirResource::Procedure(p) => {
                    if let Some(record) = self.resolve_patient(&p.subject) {
                        record.procedures.push(p);
                    }
                }
                FhirResource::Binary(b) => {
                    self.binaries.insert(b.id.clone(), b);
                }
                FhirResource::DocumentReference(d) => {
                    if let Some(record) = self.resolve_patient(&d.subject) {
                        record.documents.push(d);
                    }
                }
                FhirResource::ClinicalNote(n) => {
                    if let Some(record) = self.resolve_patient(&n.subject) {
                        record.clinical_notes.push(n);
                    }
                }
                FhirResource::Unknown { resource_type, id } => {
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
