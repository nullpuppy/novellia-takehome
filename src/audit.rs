use crate::fhir::{self, ResourceType};
use crate::store::normalize_id;
use base64::Engine;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Parse and validation issues for auditing new resource loading
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

/// Audits parsed FHIR resources for quality issues in the data.
///
/// # Returns
/// [`DataQualityIssue`] All validation issues found during audit
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
                audit_observation(o, &mut obs_groups, &mut issues);

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

fn audit_observation<'a>(
    o: &'a fhir::Observation,
    groups: &mut HashMap<(String, String, String), Vec<&'a fhir::Observation>>,
    issues: &mut Vec<DataQualityIssue>,
) {
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
    groups
        .entry((patient_key, code_key, dt_key))
        .or_default()
        .push(o);

    if o.status.as_deref() == Some("unknown") {
        issues.push(DataQualityIssue::IndeterminateStatus {
            resource_type: (&ResourceType::Observation).into(),
            id: o.id.clone(),
            status: o.status.clone().unwrap_or_default(),
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
                    id,
                    field: "data".into(),
                    reason: "data is not valid base64".into(),
                });
            }
        }
        _ => {
            issues.push(DataQualityIssue::MissingRequiredField {
                resource_type: resource_type.to_string(),
                id,
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::fhir::Name;

    fn patient(id: &str) -> fhir::FhirResource {
        fhir::FhirResource::Patient(fhir::Patient {
            id: id.to_string(),
            name: vec![fhir::Name {
                family: Some("Test".into()),
                given: vec!["Alice".into()],
                ..Default::default()
            }],
            birth_date: Some("1980-01-01".into()),
            active: Some(true),
            ..Default::default()
        })
    }

    fn patient_ref(id: &str) -> fhir::Reference {
        fhir::Reference {
            reference: Some(format!("Patient/{id}")),
            ..Default::default()
        }
    }

    fn code(code: &str) -> fhir::CodeableConcept {
        fhir::CodeableConcept {
            coding: vec![fhir::Coding {
                system: Some("test-system".into()),
                code: Some(code.into()),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn observation(
        id: &str,
        status: &str,
        patient_id: &str,
        code_value: &str,
    ) -> fhir::Observation {
        fhir::Observation {
            id: id.to_string(),
            status: Some(status.to_string()),
            code: Some(code(code_value)),
            subject: patient_ref(patient_id),
            effective_date_time: Some("2024-01-01".into()),
            ..Default::default()
        }
    }

    #[test]
    fn audit_patient_reports_missing_name_and_invalid_birth_date() {
        let resources = vec![fhir::FhirResource::Patient(fhir::Patient {
            id: "patient-1".into(),
            name: vec![Name::default()],
            birth_date: Some("notadate".into()),
            ..Default::default()
        })];

        let issues = audit_data_quality(&resources);

        assert!(issues.iter().any(|issue| {
            matches!(issue, DataQualityIssue::MissingRequiredField {
                    resource_type,
                    id,
                    field,
                } if resource_type == "Patient" && id == "patient-1" && field == "name"
            )
        }));

        assert!(issues.iter().any(|issue| {
            matches!(issue, DataQualityIssue::InvalidField {
                    resource_type,
                    id,
                    field,
                    reason,
                } if resource_type == "Patient"
                    && id == "patient-1"
                    && field == "birthDate"
                    && reason.contains("YYYY-MM-DD")
            )
        }));
    }

    #[test]
    fn audit_binary_reports_missing_content_type_and_invalid_base64() {
        let resources = vec![fhir::FhirResource::Binary(fhir::Binary {
            id: "binary-1".into(),
            content_type: None,
            data: Some("not base64".into()),
        })];

        let issues = audit_data_quality(&resources);

        assert!(issues.iter().any(|issue| {
            matches!(
                issue,
                DataQualityIssue::MissingRequiredField {
                    resource_type,
                    id,
                    field
                } if resource_type == "Binary"
                    && id == "binary-1"
                    && field == "contentType"
            )
        }));

        assert!(issues.iter().any(|issue| {
            matches!(
                issue,
                DataQualityIssue::InvalidField {
                    resource_type,
                    id,
                    field,
                    reason,
                } if resource_type == "Binary"
                    && id == "binary-1"
                    && field == "data"
                    && reason.contains("base64")
            )
        }));
    }

    #[test]
    fn audit_condition_reports_missing_code_and_unknown_patient_reference() {
        let resources = vec![fhir::FhirResource::Condition(fhir::Condition {
            id: "condition-1".into(),
            subject: patient_ref("missing-patient"),
            ..Default::default()
        })];

        let issues = audit_data_quality(&resources);

        assert!(issues.iter().any(|issue| {
            matches!(
                issue,
                DataQualityIssue::MissingRequiredField {
                    resource_type,
                    id,
                    field
                } if resource_type == "Condition"
                    && id == "condition-1"
                    && field == "code"
            )
        }));

        assert!(issues.iter().any(|issue| {
            matches!(
                issue,
                DataQualityIssue::UnresolvableReference {
                    resource_type,
                    id,
                    reason,
                } if resource_type == "Condition"
                    && id == "condition-1"
                    && reason.contains("missing-patient")
            )
        }));
    }

    #[test]
    fn audit_document_reference_reports_missing_binary_reference() {
        let resources = vec![
            patient("patient-1"),
            fhir::FhirResource::DocumentReference(fhir::DocumentReference {
                id: "document-1".into(),
                status: Some("current".into()),
                subject: patient_ref("patient-1"),
                content: vec![fhir::DocumentContent {
                    attachment: Some(fhir::Attachment {
                        content_type: Some("text/plain".into()),
                        url: Some("Binary/missing-binary".into()),
                    }),
                }],
                ..Default::default()
            }),
        ];

        let issues = audit_data_quality(&resources);

        assert!(issues.iter().any(|issue| {
            matches!(
                issue,
                DataQualityIssue::UnresolvableReference {
                    resource_type,
                    id,
                    reason,
                } if resource_type == "DocumentReference"
                    && id == "document-1"
                    && reason.contains("missing-binary")
            )
        }));
    }

    #[test]
    fn audit_document_reference_reports_content_type_mismatch() {
        let resources = vec![
            patient("patient-1"),
            fhir::FhirResource::Binary(fhir::Binary {
                id: "binary-1".into(),
                content_type: Some("application/pdf".into()),
                data: Some("SGVsbG8=".into()),
            }),
            fhir::FhirResource::DocumentReference(fhir::DocumentReference {
                id: "document-1".into(),
                status: Some("current".into()),
                subject: patient_ref("patient-1"),
                content: vec![fhir::DocumentContent {
                    attachment: Some(fhir::Attachment {
                        content_type: Some("text/plain".into()),
                        url: Some("Binary/binary-1".into()),
                    }),
                }],
                ..Default::default()
            }),
        ];

        let issues = audit_data_quality(&resources);

        assert!(issues.iter().any(|issue| {
            matches!(
                issue,
                DataQualityIssue::InvalidField {
                    resource_type,
                    id,
                    field,
                    reason,
                } if resource_type == "DocumentReference"
                    && id == "document-1"
                    && field == "content[0].attachment.contentType"
                    && reason.contains("application/pdf")
            )
        }));
    }

    #[test]
    fn audit_observation_reports_indeterminate_status() {
        let resources = vec![
            patient("patient-1"),
            fhir::FhirResource::Observation(observation(
                "observation-1",
                "unknown",
                "patient-1",
                "8302-2",
            )),
        ];

        let issues = audit_data_quality(&resources);

        assert!(issues.iter().any(|issue| {
            matches!(
                issue,
                DataQualityIssue::IndeterminateStatus {
                    resource_type,
                    id,
                    status,
                } if resource_type == "Observation"
                    && id == "observation-1"
                    && status == "unknown"
            )
        }));
    }

    #[test]
    fn audit_duplicate_observations_reports_amended_resource() {
        let resources = vec![
            patient("patient-1"),
            fhir::FhirResource::Observation(observation(
                "observation-original",
                "final",
                "patient-1",
                "8302-2",
            )),
            fhir::FhirResource::Observation(observation(
                "observation-amended",
                "amended",
                "patient-1",
                "8302-2",
            )),
        ];

        let issues = audit_data_quality(&resources);

        assert!(issues.iter().any(|issue| {
            matches!(
                issue,
                DataQualityIssue::AmendedResource {
                    resource_type,
                    amended_ids,
                    original_ids,
                    patient_id,
                } if resource_type == "Observation"
                    && patient_id == "patient-1"
                    && amended_ids == &vec!["observation-amended".to_string()]
                    && original_ids == &vec!["observation-original".to_string()]
            )
        }));
    }

    #[test]
    fn audit_duplicate_observations_without_amended_reports_duplicate_resource() {
        let resources = vec![
            patient("patient-1"),
            fhir::FhirResource::Observation(observation(
                "observation-1",
                "final",
                "patient-1",
                "8302-2",
            )),
            fhir::FhirResource::Observation(observation(
                "observation-2",
                "final",
                "patient-1",
                "8302-2",
            )),
        ];

        let issues = audit_data_quality(&resources);

        assert!(issues.iter().any(|issue| {
            matches!(
                issue,
                DataQualityIssue::DuplicateResource {
                    resource_type,
                    ids,
                    patient_id,
                    code,
                    effective_date_time,
                } if resource_type == "Observation"
                    && patient_id == "patient-1"
                    && ids == &vec!["observation-1".to_string(), "observation-2".to_string()]
                    && code == "8302-2"
                    && effective_date_time == "2024-01-01"
            )
        }));
    }

    #[test]
    fn audit_unknown_resource_reports_reports_non_standard_resource() {
        let resources = vec![fhir::FhirResource::Unknown {
            resource_type: Some("CustomThing".into()),
            id: Some("custom-1".into()),
        }];

        let issues = audit_data_quality(&resources);

        assert!(issues.iter().any(|issue| {
            matches!(
                issue,
                DataQualityIssue::NonStandardResourceType {
                    resource_type,
                    id,
                } if resource_type == "CustomThing" && id == "custom-1"
            )
        }));
    }
}
