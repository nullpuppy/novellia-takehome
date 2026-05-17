use crate::fhir::{self, ResourceType};
use crate::store::normalize_id;
use base64::Engine;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Enum of possible parse and validation errors
#[derive(Debug, Clone, Serialize)]
pub enum DataQualityIssue {
    /// Could not parse data for import
    ParseError {
        line_num: usize,
        content: String,
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
    IndeterminateStatus {
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
    InvalidField {
        resource_type: String,
        id: String,
        field: String,
        reason: String,
    },
    NonStandardResourceType {
        resource_type: String,
        id: String,
    },
}

/// Validates newly parsed resources to look for issues in the resources parsed
/// and preserves the issue so it can be looked up at a later time.
///
/// # Returns
/// Vec of [`DataQualityIssue`]
pub(crate) fn audit_data_quality(resources: &[fhir::FhirResource]) -> Vec<DataQualityIssue> {
    let mut issues = Vec::new();
    let mut obs_groups: HashMap<(String, String, String), Vec<&fhir::Observation>> = HashMap::new();

    let patient_ids: HashSet<String> = collect_patient_ids(resources);
    let binary_ids_content_types: HashMap<String, String> =
        collect_binary_ids_content_types(resources);

    for r in resources {
        match r {
            fhir::FhirResource::Patient(p) => {
                audit_patient(p, &mut issues);
            }
            fhir::FhirResource::Condition(c) => {
                if c.code.is_none() {
                    issues.push(DataQualityIssue::MissingRequiredField {
                        resource_type: ResourceType::Condition.to_string(),
                        id: c.id.clone(),
                        field: "code".into(),
                    });
                }
                check_patient_ref(&c.subject, "Condition", &c.id, &patient_ids, &mut issues);
            }
            fhir::FhirResource::MedicationRequest(m) => {
                check_patient_ref(
                    &m.subject,
                    &ResourceType::MedicationRequest.to_string(),
                    &m.id,
                    &patient_ids,
                    &mut issues,
                );
            }
            fhir::FhirResource::Observation(o) => {
                let resource_type = &ResourceType::Observation;

                // Track observations found to detect duplicates
                let patient_key = o
                    .subject
                    .patient_id()
                    .map(str::to_lowercase)
                    .unwrap_or_default();
                let code_key = o
                    .code
                    .as_ref()
                    .and_then(|cc| cc.coding.first())
                    .and_then(|c| c.code.clone())
                    .unwrap_or_default();
                let dt_key = o.effective_date_time.clone().unwrap_or_default();
                obs_groups
                    .entry((patient_key, code_key, dt_key))
                    .or_default()
                    .push(o);

                if o.status.as_deref() == Some("unknown") {
                    issues.push(DataQualityIssue::IndeterminateStatus {
                        resource_type: resource_type.into(),
                        id: o.id.clone(),
                        status: o.status.clone().unwrap_or_default(),
                    });
                }
                check_patient_ref(
                    &o.subject,
                    resource_type.into(),
                    &o.id,
                    &patient_ids,
                    &mut issues,
                );
            }
            fhir::FhirResource::Procedure(p) => {
                check_patient_ref(
                    &p.subject,
                    (&ResourceType::Procedure).into(),
                    &p.id,
                    &patient_ids,
                    &mut issues,
                );
            }
            fhir::FhirResource::Binary(b) => {
                audit_binary(b, &mut issues);
            }
            fhir::FhirResource::DocumentReference(d) => {
                check_patient_ref(
                    &d.subject,
                    &ResourceType::DocumentReference.to_string(),
                    &d.id,
                    &patient_ids,
                    &mut issues,
                );
                audit_document_reference(d, &binary_ids_content_types, &mut issues);
            }
            fhir::FhirResource::ClinicalNote(c) => {
                issues.push(DataQualityIssue::NonStandardResourceType {
                    resource_type: "ClinicalNote".into(),
                    id: c.id.clone(),
                });
                check_patient_ref(&c.subject, "ClinicalNote", &c.id, &patient_ids, &mut issues);
            }
            fhir::FhirResource::Unknown { resource_type, id } => {
                issues.push(DataQualityIssue::NonStandardResourceType {
                    resource_type: resource_type.clone().unwrap_or_default(),
                    id: id.clone().unwrap_or_default(),
                });
            }
        }
    }

    audit_observation_groups(obs_groups, &mut issues);

    issues
}

/// Validates Patient resources for required fields and field formats
fn audit_patient(patient: &fhir::Patient, issues: &mut Vec<DataQualityIssue>) {
    if patient.id.trim().is_empty() {
        issues.push(DataQualityIssue::MissingRequiredField {
            resource_type: ResourceType::Patient.to_string(),
            id: patient.id.clone(),
            field: "id".into(),
        });
    }

    if patient.name.is_empty() || patient.display_name().is_none() {
        issues.push(DataQualityIssue::MissingRequiredField {
            resource_type: ResourceType::Patient.to_string(),
            id: patient.id.clone(),
            field: "name".into(),
        });
    }

    if let Some(birth_date) = patient.birth_date.as_deref()
        && !validate_date_iso8601(birth_date)
    {
        issues.push(DataQualityIssue::InvalidField {
            resource_type: ResourceType::Patient.to_string(),
            id: patient.id.clone(),
            field: "birthDate".into(),
            reason: "expected 'YYYY-MM-DD'".into(),
        });
    }
}

/// Validates Binary resources for the required attachment metadata and base64 payload
fn audit_binary(binary: &fhir::Binary, issues: &mut Vec<DataQualityIssue>) {
    let resource_type = &ResourceType::Binary;
    let id = binary.id.clone();
    if binary.id.trim().is_empty() {
        issues.push(DataQualityIssue::MissingRequiredField {
            resource_type: resource_type.to_string(),
            id: id.clone(),
            field: "id".into(),
        });
    }

    if binary
        .content_type
        .as_deref()
        .is_none_or(|ct| ct.trim().is_empty())
    {
        issues.push(DataQualityIssue::MissingRequiredField {
            resource_type: resource_type.to_string(),
            id: id.clone(),
            field: "contentType".into(),
        });
    }

    match binary.data.as_deref() {
        Some(data) if !data.trim().is_empty() => {
            if base64::engine::general_purpose::STANDARD
                .decode(data)
                .is_err()
            {
                issues.push(DataQualityIssue::InvalidField {
                    resource_type: resource_type.to_string(),
                    id: id.clone(),
                    field: "data".into(),
                    reason: "data is not valid base64".into(),
                });
            }
        }
        _ => {
            issues.push(DataQualityIssue::MissingRequiredField {
                resource_type: resource_type.to_string(),
                id: id.clone(),
                field: "data".into(),
            });
        }
    }
}

/// Validates [`DocumentReference`] resources and their attachment refs
fn audit_document_reference(
    doc_reference: &fhir::DocumentReference,
    binary_ids_content_types: &HashMap<String, String>,
    issues: &mut Vec<DataQualityIssue>,
) {
    let resource_type = "DocumentReference";
    let id = &doc_reference.id;

    if doc_reference.content.is_empty() {
        issues.push(DataQualityIssue::MissingRequiredField {
            resource_type: resource_type.into(),
            id: id.clone(),
            field: "content".into(),
        });
    }

    for (idx, content) in doc_reference.content.iter().enumerate() {
        let Some(attachment) = &content.attachment else {
            issues.push(DataQualityIssue::MissingRequiredField {
                resource_type: resource_type.into(),
                id: id.clone(),
                field: format!("content[{idx}].attachment"),
            });
            continue;
        };

        let Some(url) = attachment
            .url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
        else {
            issues.push(DataQualityIssue::MissingRequiredField {
                resource_type: resource_type.into(),
                id: id.clone(),
                field: format!("content[{idx}].attachment.url"),
            });
            continue;
        };

        let Some(binary_id) = url
            .split_once('/')
            .map(|(rt, id)| rt.eq_ignore_ascii_case("Binary").then_some(id))
            .unwrap_or_default()
        else {
            issues.push(DataQualityIssue::UnresolvableReference {
                resource_type: resource_type.into(),
                id: id.clone(),
                reason: format!("attachment url '{url}' is not a Binary reference"),
            });
            continue;
        };

        let normalized = normalize_id(binary_id);
        if !binary_ids_content_types.contains_key(&normalized) {
            issues.push(DataQualityIssue::UnresolvableReference {
                resource_type: resource_type.into(),
                id: id.clone(),
                reason: format!("Binary/{binary_id} does not match any known binary"),
            });
            continue;
        }

        if binary_id != normalized {
            issues.push(DataQualityIssue::CaseMismatchReference {
                resource_type: resource_type.into(),
                id: id.clone(),
                reference: format!("Binary/{binary_id}"),
                canonical: format!("Binary/{normalized}"),
            });
        }

        if let (Some(attachment_content_type), Some(binary_content_type)) = (
            attachment.content_type.as_deref(),
            binary_ids_content_types.get(&normalized),
        ) && attachment_content_type != binary_content_type
        {
            issues.push(DataQualityIssue::InvalidField {
                    resource_type: resource_type.into(),
                    id: id.clone(),
                    field: format!("content[{idx}].attachment.contentType"),
                    reason: format!("attachment content type '{attachment_content_type}' does not match Binary/{normalized} content type '{binary_content_type}"),
                });
        }
    }
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

        let (amended, others): (Vec<&fhir::Observation>, Vec<&fhir::Observation>) = group
            .iter()
            .partition(|o| o.status.as_deref() == Some("amended"));
        let amended: Vec<String> = amended.iter().map(|o| o.id.clone()).collect();
        let others: Vec<String> = others.iter().map(|o| o.id.clone()).collect();

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
                ids: if others.is_empty() { amended } else { others },
                patient_id,
                code,
                effective_date_time,
            });
        }
    }
}

/// Validates imported data is in the correct format and has all required fields
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

/// # Returns
/// A [`HashSet`] of all patient ids found in the array of resources that have been loaded.
fn collect_patient_ids(resources: &[fhir::FhirResource]) -> HashSet<String> {
    resources
        .iter()
        .filter_map(|r| match r {
            fhir::FhirResource::Patient(p) => Some(p.id.to_lowercase()),
            _ => None,
        })
        .collect()
}

/// # Returns
/// A [`HashMap`] of all binary ids to their respective content type
fn collect_binary_ids_content_types(resources: &[fhir::FhirResource]) -> HashMap<String, String> {
    resources
        .iter()
        .filter_map(|r| match r {
            fhir::FhirResource::Binary(b) => Some((
                normalize_id(&b.id),
                b.content_type.clone().unwrap_or_default().trim().into(),
            )),
            _ => None,
        })
        .collect()
}

/// iso8601 defines a date as YYYY-MM-DD.
fn validate_date_iso8601(s: &str) -> bool {
    s.len() == 10
        && s.bytes().enumerate().all(|(idx, b)| match idx {
            4 | 7 => b == b'-',
            ..4 | 5..7 | 8..10 => b.is_ascii_digit(),
            _ => false,
        })
}
