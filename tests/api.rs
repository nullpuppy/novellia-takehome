use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use novellia_takehome::route::build_router;
use novellia_takehome::store;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

mod common;

async fn request_json(app: axum::Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    let json = serde_json::from_slice(&bytes).expect("response should be json");

    (status, json)
}

#[tokio::test]
async fn get_patient_returns_patient_json() {
    let app = common::test_app();

    let (status, json) = request_json(app, "/patients/patient-1").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["id"], "patient-1");
    assert_eq!(json["name"], "Alice Test");
}

#[tokio::test]
async fn patient_lookup_is_case_insensitive() {
    let app = common::test_app();

    let (status, json) = request_json(app.clone(), "/patients/patient-1").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["id"], "patient-1");
    assert_eq!(json["name"], "Alice Test");

    let (status, json) = request_json(app, "/patients/PATient-1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["id"], "patient-1");
    assert_eq!(json["name"], "Alice Test");
}

#[tokio::test]
async fn missing_patient_returns_json_404() {
    let app = common::test_app();

    let (status, json) = request_json(app, "/patients/missing").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        json["error"]
            .as_str()
            .is_some_and(|msg| msg.contains("missing"))
    );
}

#[tokio::test]
async fn get_patient_document_returns_decoded_binary_content() {
    let app = common::test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/patients/patient-1/documents/document-1")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/plain")
    );

    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");

    assert_eq!(&bytes[..], b"Hello document");
}

#[tokio::test]
async fn document_list_returns_summaries_without_content() {
    let app = common::test_app();

    let (status, json) = request_json(app, "/patients/patient-1/documents").await;

    assert_eq!(status, StatusCode::OK);

    let docs = json.as_array().expect("documents should be an array");

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0]["id"], "document-1");
    assert_eq!(docs[0]["binary_id"], "binary-1");
    assert!(docs[0].get("content").is_none());
}

#[tokio::test]
async fn document_with_missing_binary_returns_bad_resource() {
    let path = common::write_temp_data(
        r#"
{"resourceType":"Patient","id":"patient-1","name":[{"family":"Test","given":["Alice"]}]}
{"resourceType":"DocumentReference","id":"document-1","status":"current","subject":{"reference":"Patient/patient-1"},"date":"2023-01-01","content":[{"attachment":{"contentType":"text/plain","url":"Binary/missing-binary"}}]}
    "#,
    );

    let store = store::Store::load(&path).expect("store should load");
    let app = build_router(Arc::new(store));

    let (status, json) = request_json(app, "/patients/patient-1/documents/document-1").await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        json["error"]
            .as_str()
            .is_some_and(|msg| msg.contains("content could not be loaded"))
    );
}

#[tokio::test]
async fn timeline_contains_resources_newest_first() {
    let app = common::test_app();

    let (status, json) = request_json(app, "/patients/patient-1/timeline").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["patient"]["id"], "patient-1");

    let timeline = json["timeline"]
        .as_array()
        .expect("timeline should be an array");

    assert!(timeline.len() >= 5);

    let dates: Vec<&str> = timeline
        .iter()
        .filter_map(|entry| entry["date"].as_str())
        .collect();

    let mut sorted = dates.clone();
    sorted.sort_by(|a, b| b.cmp(a));

    assert_eq!(dates, sorted);

    let resource_types: Vec<&str> = timeline
        .iter()
        .filter_map(|entry| entry["resource_type"].as_str())
        .collect();

    assert!(resource_types.contains(&"Condition"));
    assert!(resource_types.contains(&"MedicationRequest"));
    assert!(resource_types.contains(&"Observation"));
    assert!(resource_types.contains(&"Procedure"));
    assert!(resource_types.contains(&"DocumentReference"));
}

#[tokio::test]
async fn data_quality_endpoint_returns_parse_and_audit_issues() {
    let path = common::write_temp_data(
        r#"
{"resourceType":"Patient","id":"patient-1","name":[]}
{not valid json
        "#,
    );

    let store = store::Store::load(&path).expect("store should load valid rows");
    let app = build_router(Arc::new(store));

    let (status, json) = request_json(app, "/data-quality").await;

    assert_eq!(status, StatusCode::OK);
    let issues = json
        .as_array()
        .expect("data-quality should return an array");

    assert!(issues.iter().any(|issue| issue.get("ParseError").is_some()));
    assert!(
        issues
            .iter()
            .any(|issue| issue.get("MissingRequiredField").is_some())
    );
}
