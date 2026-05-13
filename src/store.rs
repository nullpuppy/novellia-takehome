use crate::fhir;
use crate::fhir::{
    Binary, ClinicalNote, Condition, DocumentReference, FhirResource, MedicationRequest,
    Observation, Patient, Procedure, Reference,
};
use crate::store::DataQualityIssue::NonStandardResourceType;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use tracing::error;

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

#[derive(Debug, Clone, Serialize)]
pub enum DataQualityIssue {
    ParseError {
        line: String,
        message: String,
    },
    MissingRequiredField {
        resource_type: String,
        id: String,
        field: String,
    },
    UnresolvableReference {
        resource_type: String,
        id: String,
        reason: String,
    },
    UnknownStatus {
        resource_type: String,
        id: String,
        status: String,
    },
    CaseMismatchReference {
        resource_type: String,
        id: String,
        reference: String,
        canonical: String,
    },
    DuplicateResource {
        resource_type: String,
        ids: Vec<String>,
        patient_id: String,
        code: String,
        effective_date_time: String,
    },
    AmendedResource {
        resource_type: String,
        amended_ids: Vec<String>,
        original_ids: Vec<String>,
        patient_id: String,
    },
    NonStandardResourceType {
        resource_type: String,
        id: String,
    },
}

pub struct Store {
    // Keyed on id
    pub patients: HashMap<String, PatientRecord>,
    // Keyed on id
    pub binaries: HashMap<String, Binary>,
    pub quality_issues: Vec<DataQualityIssue>,
}

impl Store {
    pub fn load(path: std::path::PathBuf) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut resources: Vec<FhirResource> = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match fhir::parse_resource(line) {
                Ok(record) => resources.push(record),
                Err(err) => error!("line {}: parse error: {}", line_num, err),
            }
        }

        let quality_issues = audit_data_quality(&resources);

        let mut store = Store {
            patients: HashMap::new(),
            binaries: HashMap::new(),
            quality_issues,
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

fn audit_data_quality(resources: &Vec<FhirResource>) -> Vec<DataQualityIssue> {
    let mut issues = Vec::new();
    let mut obs_groups: HashMap<(String, String, String), Vec<&fhir::Observation>> = HashMap::new();

    let patient_ids: HashSet<String> = resources
        .iter()
        .filter_map(|r| match &r {
            FhirResource::Patient(p) => Some(p.id.to_lowercase()),
            _ => None,
        })
        .collect();

    for r in resources {
        match r {
            FhirResource::Condition(c) => {
                if c.code.is_none() {
                    issues.push(DataQualityIssue::MissingRequiredField {
                        resource_type: "Condition".into(),
                        id: c.id.clone(),
                        field: "code".into(),
                    })
                }
                check_patient_ref(&c.subject, "Condition", &c.id, &patient_ids, &mut issues);
            }
            FhirResource::MedicationRequest(m) => {
                check_patient_ref(
                    &m.subject,
                    "MedicationRequest",
                    &m.id,
                    &patient_ids,
                    &mut issues,
                );
            }
            FhirResource::Observation(o) => {
                // Track observations found to detect duplicates
                let patient_key = o
                    .subject
                    .patient_id()
                    .map(|p| p.to_lowercase())
                    .unwrap_or_default();
                let code_key = o
                    .code
                    .coding
                    .first()
                    .and_then(|c| c.code.clone())
                    .unwrap_or_default();
                let dt_key = o.effective_date_time.clone().unwrap_or_default();
                obs_groups
                    .entry((patient_key, code_key, dt_key))
                    .or_default()
                    .push(o);

                if o.status == "unknown" {
                    issues.push(DataQualityIssue::UnknownStatus {
                        resource_type: "Observation".into(),
                        id: o.id.clone(),
                        status: o.status.clone(),
                    })
                }
                check_patient_ref(&o.subject, "Observation", &o.id, &patient_ids, &mut issues);
            }
            FhirResource::Procedure(p) => {
                check_patient_ref(&p.subject, "Procedure", &p.id, &patient_ids, &mut issues);
            }
            FhirResource::ClinicalNote(c) => {
                issues.push(NonStandardResourceType {
                    resource_type: "ClinicalNote".into(),
                    id: c.id.clone(),
                });
                check_patient_ref(&c.subject, "ClinicalNote", &c.id, &patient_ids, &mut issues);
            }
            _ => {}
        }
    }

    for ((patient_id, code, effective_date_time), group) in obs_groups {
        if group.len() < 2 {
            continue;
        }

        let amended: Vec<_> = group
            .iter()
            .filter_map(|o| {
                if o.status == "amended" {
                    Some(o.id.clone())
                } else {
                    None
                }
            })
            .collect();
        let others: Vec<_> = group
            .iter()
            .filter_map(|o| {
                if o.status != "amended" {
                    Some(o.id.clone())
                } else {
                    None
                }
            })
            .collect();

        if !amended.is_empty() && !others.is_empty() {
            issues.push(DataQualityIssue::AmendedResource {
                resource_type: "Observation".into(),
                patient_id,
                amended_ids: amended,
                original_ids: others,
            })
        } else {
            issues.push(DataQualityIssue::DuplicateResource {
                resource_type: "Observation".into(),
                ids: others,
                patient_id,
                code,
                effective_date_time,
            })
        }
    }

    issues
}

fn check_patient_ref(
    subject: &Reference,
    resource_type: &str,
    id: &str,
    patient_ids: &HashSet<String>,
    issues: &mut Vec<DataQualityIssue>,
) {
    match subject.patient_id() {
        Some(raw_id) => {
            let normalized = raw_id.to_lowercase();
            if patient_ids.contains(&normalized) {
                if raw_id != normalized {
                    issues.push(DataQualityIssue::CaseMismatchReference {
                        resource_type: resource_type.to_string(),
                        id: id.to_string(),
                        reference: format!("Patient/{raw_id}"),
                        canonical: format!("Patient/{normalized}"),
                    });
                }
            } else {
                issues.push(DataQualityIssue::UnresolvableReference {
                    resource_type: resource_type.to_string(),
                    id: id.to_string(),
                    reason: format!("Patient/{raw_id} does not match any known patient"),
                });
            }
        }
        None => {
            issues.push(DataQualityIssue::UnresolvableReference {
                resource_type: resource_type.to_string(),
                id: id.to_string(),
                reason: "subject is missing reference field".into(),
            });
        }
    };
}
