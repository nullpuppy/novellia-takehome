#![allow(dead_code)]
use axum::Router;
use novellia_takehome::{route::build_router, store};
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(0);

pub fn write_temp_data(contents: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let temp_file_id = NEXT_TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed);

    let path = std::env::temp_dir().join(format!("novellia-test-{temp_file_id}-{unique}.jsonl"));
    fs::write(&path, contents).expect("test data should be writable");
    path
}

pub fn fixture_data() -> &'static str {
    r#"
{"resourceType":"Patient","id":"patient-1","name":[{"family":"Test","given":["Alice"]}],"gender":"female","birthDate":"1980-01-01","active":true}
{"resourceType":"Condition","id":"condition-1","code":{"coding":[{"system":"http://snomed.info/sct","code":"44054006","display":"Diabetes"}]},"subject":{"reference":"Patient/patient-1"},"onsetDateTime":"2020-01-01"}
{"resourceType":"MedicationRequest","id":"medication-1","status":"active","intent":"order","medicationCodeableConcept":{"coding":[{"system":"http://www.nlm.nih.gov/research/umls/rxnorm","code":"860975","display":"Metformin"}]},"subject":{"reference":"Patient/patient-1"},"authoredOn":"2021-01-01"}
{"resourceType":"Observation","id":"observation-1","status":"final","code":{"coding":[{"system":"http://loinc.org","code":"8302-2","display":"Body height"}]},"subject":{"reference":"Patient/patient-1"},"effectiveDateTime":"2022-01-01","valueQuantity":{"value":170.0,"unit":"cm"}}
{"resourceType":"Procedure","id":"procedure-1","status":"completed","code":{"coding":[{"system":"http://snomed.info/sct","code":"73761001","display":"Colonoscopy"}]},"subject":{"reference":"Patient/patient-1"},"performedDateTime":"2019-01-01","performer":[]}
{"resourceType":"Binary","id":"binary-1","contentType":"text/plain","data":"SGVsbG8gZG9jdW1lbnQ="}
{"resourceType":"DocumentReference","id":"document-1","status":"current","subject":{"reference":"Patient/patient-1"},"date":"2023-01-01","content":[{"attachment":{"contentType":"text/plain","url":"Binary/binary-1"}}]}
        "#
}

fn test_store() -> store::Store {
    let path = write_temp_data(fixture_data());
    let store = store::Store::load(&path).expect("fixture store should load");

    assert!(
        store.get_patient("patient-1").is_some(),
        "fixture store should contain patient-1; quality issues: {:?}",
        store.quality_issues
    );

    store
}

pub fn test_app() -> Router {
    build_router(Arc::new(test_store()))
}
