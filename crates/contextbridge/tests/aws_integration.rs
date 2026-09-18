#![cfg(feature = "aws-tests")]

use aws_config::BehaviorVersion;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::AUTHORIZATION},
};
use contextbridge::{
    AgentId, AppState, EvidenceStore, app,
    aws::{AwsBackendConfig, DynamoDbStorage, S3EvidenceStore, StorageMode},
};
use serde_json::{Value, json};
use std::{collections::HashMap, env, sync::Arc};
use tower::ServiceExt;
use uuid::Uuid;

fn require_aws() {
    assert_eq!(
        env::var("CONTEXTBRIDGE_RUN_AWS_TESTS").as_deref(),
        Ok("1"),
        "set CONTEXTBRIDGE_RUN_AWS_TESTS=1 to run real AWS tests"
    );
}

async fn storage() -> DynamoDbStorage {
    require_aws();
    let config = AwsBackendConfig::from_env().expect("valid AWS backend configuration");
    assert_eq!(config.mode, StorageMode::Aws);
    let sdk = match config.region {
        Some(region) => {
            aws_config::defaults(BehaviorVersion::latest())
                .region(aws_sdk_dynamodb::config::Region::new(region))
                .load()
                .await
        }
        None => aws_config::defaults(BehaviorVersion::latest()).load().await,
    };
    let evidence = Arc::new(S3EvidenceStore::new(
        aws_sdk_s3::Client::new(&sdk),
        config.s3_bucket.expect("bucket"),
    ));
    DynamoDbStorage::new(
        aws_sdk_dynamodb::Client::new(&sdk),
        config.dynamodb_table.expect("table"),
        evidence,
    )
}

#[tokio::test]
#[ignore = "real AWS resources and credentials required"]
async fn aws_storage_contract_is_opt_in() {
    let storage = storage().await;
    let namespace = format!("aws-{}", Uuid::new_v4().simple());
    contextbridge::test_support::storage_contract_named(&storage, &namespace)
        .await
        .expect("AWS adapter must satisfy the shared storage contract");
}

async fn send(app: &axum::Router, token: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 2_000_000).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(json!({})))
}

#[tokio::test]
#[ignore = "real AWS resources and credentials required"]
async fn aws_golden_path_preserves_current_truth_and_raw_evidence() {
    let storage = storage().await;
    let namespace = Uuid::new_v4().simple().to_string();
    let alpha = format!("m2-alpha-{namespace}");
    let beta = format!("m2-beta-{namespace}");
    let mut tokens = HashMap::new();
    tokens.insert("agent-a-token".into(), AgentId("agent-a".into()));
    tokens.insert("agent-b-token".into(), AgentId("agent-b".into()));
    let app = app(AppState {
        storage: Arc::new(storage.clone()),
        tokens: Arc::new(tokens),
    });

    let (status, first) = send(
        &app,
        "agent-a-token",
        "/v1/events",
        json!({
            "workspace_id": alpha,
            "events": [{
                "kind": "decision",
                "subject": "persistence",
                "content": "Persistence uses Redis.",
                "raw_evidence": "Agent A raw Redis evidence.",
                "promote": true
            }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let redis = first[0]["promoted_memory_id"]
        .as_str()
        .expect("Redis memory id")
        .to_owned();

    let (status, first_context) = send(
        &app,
        "agent-b-token",
        "/v1/context",
        json!({"workspace_id": alpha, "query": "Implement persistence", "max_tokens": 100}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(first_context["summary"].as_str().unwrap().contains("Redis"));
    assert_eq!(
        first_context["evidence"][0]["provenance"]["source_agent"],
        "agent-a"
    );
    let raw_handle = first_context["evidence"][0]["raw_handle"]
        .as_str()
        .unwrap()
        .to_owned();

    let (status, second) = send(
        &app,
        "agent-a-token",
        "/v1/events",
        json!({
            "workspace_id": alpha,
            "events": [{
                "kind": "decision",
                "subject": "persistence",
                "content": "Persistence uses DynamoDB.",
                "raw_evidence": "Agent A raw DynamoDB evidence.",
                "promote": true,
                "supersedes_memory_id": redis
            }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, current) = send(
        &app,
        "agent-b-token",
        "/v1/context",
        json!({"workspace_id": alpha, "query": "Implement persistence", "max_tokens": 100}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(current["summary"].as_str().unwrap().contains("DynamoDB"));
    assert!(!current["summary"].as_str().unwrap().contains("Redis"));
    assert!(current["token_usage"]["selected"].as_u64().unwrap() <= 100);
    assert_eq!(
        current["evidence"][0]["provenance"]["source_agent"],
        "agent-a"
    );

    let (status, beta_result) = send(
        &app,
        "agent-a-token",
        "/v1/events",
        json!({
            "workspace_id": beta,
            "events": [{
                "kind": "decision",
                "subject": "persistence",
                "content": "Persistence uses PostgreSQL.",
                "promote": true
            }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(beta_result[0]["promoted_memory_id"].is_string());
    let recovered = storage
        .recover_raw(&raw_handle)
        .await
        .expect("raw evidence must be recoverable from S3");
    assert_eq!(recovered, b"Agent A raw Redis evidence.");
    assert!(second[0]["raw_handle"].is_object());
}
