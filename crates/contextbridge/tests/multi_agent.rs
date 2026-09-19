use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::AUTHORIZATION},
};
use contextbridge::{AgentId, local_app};
use serde_json::{Value, json};
use std::collections::HashMap;
use tower::ServiceExt;

async fn request(
    app: &axum::Router,
    token: &str,
    method: &str,
    uri: &str,
    body: Value,
) -> (StatusCode, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
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
    (status, bytes.to_vec())
}

async fn json_request(
    app: &axum::Router,
    token: &str,
    method: &str,
    uri: &str,
    body: Value,
) -> (StatusCode, Value) {
    let (status, bytes) = request(app, token, method, uri, body).await;
    (status, serde_json::from_slice(&bytes).unwrap_or(json!({})))
}

#[tokio::test]
async fn local_two_agent_handoff_preserves_current_truth_and_history() {
    let mut tokens = HashMap::new();
    tokens.insert("agent-a-token".into(), AgentId("agent-a".into()));
    tokens.insert("agent-b-token".into(), AgentId("agent-b".into()));
    let app = local_app("sqlite::memory:", tokens).await.unwrap();

    let (status, redis_event) = json_request(
        &app,
        "agent-a-token",
        "POST",
        "/v1/events",
        json!({
            "workspace_id": "project-alpha",
            "events": [{
                "event_id": "m6-redis",
                "kind": "decision",
                "subject": "persistence",
                "content": "Persistence uses Redis.",
                "raw_evidence": "Agent A evidence: Redis was the original persistence choice."
            }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(redis_event[0]["outcome"], "PROMOTED");
    assert!(
        redis_event[0]["raw_handle"]["handle"]
            .as_str()
            .unwrap()
            .starts_with("artifact://project-alpha/")
    );

    let (status, before) = json_request(
        &app,
        "agent-b-token",
        "POST",
        "/v1/context",
        json!({
            "workspace_id": "project-alpha",
            "query": "Implement persistence",
            "max_tokens": 100
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(before["summary"].as_str().unwrap().contains("Redis"));
    assert_eq!(
        before["evidence"][0]["provenance"]["source_agent"],
        "agent-a"
    );
    let redis_handle = before["evidence"][0]["raw_handle"]
        .as_str()
        .unwrap()
        .to_owned();

    let (status, dynamo_event) = json_request(
        &app,
        "agent-a-token",
        "POST",
        "/v1/events",
        json!({
            "workspace_id": "project-alpha",
            "events": [{
                "event_id": "m6-dynamo",
                "kind": "decision",
                "subject": "persistence",
                "content": "Persistence uses DynamoDB.",
                "raw_evidence": "Agent A evidence: DynamoDB replaced Redis."
            }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dynamo_event[0]["outcome"], "PROMOTED");

    let (status, after) = json_request(
        &app,
        "agent-b-token",
        "POST",
        "/v1/context",
        json!({
            "workspace_id": "project-alpha",
            "query": "Implement persistence",
            "max_tokens": 100
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(after["summary"].as_str().unwrap().contains("DynamoDB"));
    assert!(!after["summary"].as_str().unwrap().contains("Redis"));
    assert_eq!(
        after["evidence"][0]["provenance"]["source_agent"],
        "agent-a"
    );
    assert!(
        after["token_usage"]["selected"].as_u64().unwrap()
            <= after["token_usage"]["budget"].as_u64().unwrap()
    );

    let (status, history) = json_request(
        &app,
        "agent-b-token",
        "GET",
        "/v1/memories?workspace_id=project-alpha&status=superseded",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(history.as_array().unwrap().len(), 1);
    assert_eq!(history[0]["content"], "Persistence uses Redis.");

    let (status, postgres) = json_request(
        &app,
        "agent-a-token",
        "POST",
        "/v1/events",
        json!({
            "workspace_id": "project-beta",
            "events": [{
                "event_id": "m6-postgres",
                "kind": "decision",
                "subject": "persistence",
                "content": "Persistence uses PostgreSQL."
            }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(postgres[0]["outcome"], "PROMOTED");

    let (status, isolated) = json_request(
        &app,
        "agent-b-token",
        "POST",
        "/v1/context",
        json!({
            "workspace_id": "project-alpha",
            "query": "Implement persistence",
            "max_tokens": 100
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!isolated["summary"].as_str().unwrap().contains("PostgreSQL"));

    let redis_hash = redis_handle
        .strip_prefix("artifact://project-alpha/")
        .unwrap();
    let (status, raw) = request(
        &app,
        "agent-b-token",
        "GET",
        &format!("/v1/evidence/{redis_hash}?workspace_id=project-alpha"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        raw,
        b"Agent A evidence: Redis was the original persistence choice."
    );

    let (status, wrong_workspace) = request(
        &app,
        "agent-b-token",
        "GET",
        &format!("/v1/evidence/{redis_hash}?workspace_id=project-beta"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!wrong_workspace.is_empty());
}
