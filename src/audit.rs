use crate::fhir;
// use crate::fhir::{FhirResource, Observation, Reference};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Enum of possible parse and validation errors
#[derive(Debug, Clone, Serialize)]
pub enum DataQualityIssue {
    /// Could not parse data for import
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

#[allow(clippy::doc_markdown)]
/// GET data_quality
///
/// During data import on service startup, a record of data quality issues is saved.
/// This will return the detail of each problem encountered.
///
/// # Returns
/// Vec of [`DataQualityIssue`] serialized to json
///
/// If no issues were found, an empty [Vec] serialized to json is returned.
///
/// # Errors
/// [None]
pub(crate) fn audit_data_quality(resources: &Vec<fhir::FhirResource>) -> Vec<DataQualityIssue> {
    let mut issues = Vec::new();
    let mut obs_groups: HashMap<(String, String, String), Vec<&fhir::Observation>> = HashMap::new();

    let patient_ids: HashSet<String> = collect_patient_ids(resources);

    for r in resources {
        match r {
            fhir::FhirResource::Condition(c) => {
                if c.code.is_none() {
                    issues.push(DataQualityIssue::MissingRequiredField {
                        resource_type: "Condition".into(),
                        id: c.id.clone(),
                        field: "code".into(),
                    });
                }
                check_patient_ref(&c.subject, "Condition", &c.id, &patient_ids, &mut issues);
            }
            fhir::FhirResource::MedicationRequest(m) => {
                check_patient_ref(
                    &m.subject,
                    "MedicationRequest",
                    &m.id,
                    &patient_ids,
                    &mut issues,
                );
            }
            fhir::FhirResource::Observation(o) => {
                // Track observations found to detect duplicates
                let patient_key = o
                    .subject
                    .patient_id()
                    .map(str::to_lowercase)
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
                    });
                }
                check_patient_ref(&o.subject, "Observation", &o.id, &patient_ids, &mut issues);
            }
            fhir::FhirResource::Procedure(p) => {
                check_patient_ref(&p.subject, "Procedure", &p.id, &patient_ids, &mut issues);
            }
            fhir::FhirResource::ClinicalNote(c) => {
                issues.push(DataQualityIssue::NonStandardResourceType {
                    resource_type: "ClinicalNote".into(),
                    id: c.id.clone(),
                });
                check_patient_ref(&c.subject, "ClinicalNote", &c.id, &patient_ids, &mut issues);
            }
            _ => {}
        }
    }

    audit_observation_groups(obs_groups, &mut issues);

    issues
}

/// # Returns
/// A [`HashSet`] of all patient ids found in the array of resources that have been loaded.
fn collect_patient_ids(resources: &[fhir::FhirResource]) -> HashSet<String> {
    resources
        .iter()
        .filter_map(|r| match &r {
            fhir::FhirResource::Patient(p) => Some(p.id.to_lowercase()),
            _ => None,
        })
        .collect()
}

/// Look for duplicate resources
fn audit_observation_groups(
    observation_groups: HashMap<(String, String, String), Vec<&fhir::Observation>>,
    issues: &mut Vec<DataQualityIssue>,
) {
    for ((patient_id, code, effective_date_time), group) in observation_groups {
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
                if o.status == "amended" {
                    None
                } else {
                    Some(o.id.clone())
                }
            })
            .collect();

        if !amended.is_empty() && !others.is_empty() {
            issues.push(DataQualityIssue::AmendedResource {
                resource_type: "Observation".into(),
                patient_id,
                amended_ids: amended,
                original_ids: others,
            });
        } else {
            issues.push(DataQualityIssue::DuplicateResource {
                resource_type: "Observation".into(),
                ids: others,
                patient_id,
                code,
                effective_date_time,
            });
        }
    }
}

/// Validates imported data is in the correct format and has all required fields
///
/// # Returns
/// Nothing
///
/// # Errors
///
fn check_patient_ref(
    subject: &fhir::Reference,
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
    }
}
