use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::AUTHORIZATION},
};
use contextbridge::{AgentId, local_app};
use serde_json::{Value, json};
use std::collections::HashMap;
use tower::ServiceExt;

async fn json_request(
    app: &axum::Router,
    token: &str,
    method: &str,
    uri: &str,
    body: Value,
) -> (StatusCode, Value) {
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
    (status, serde_json::from_slice(&bytes).unwrap_or(json!({})))
}

#[tokio::test]
async fn typed_events_promote_dedupe_and_supersede_without_aws() {
    let mut tokens = HashMap::new();
    tokens.insert("agent-a-token".into(), AgentId("agent-a".into()));
    tokens.insert("agent-b-token".into(), AgentId("agent-b".into()));
    let app = local_app("sqlite::memory:", tokens).await.unwrap();

    let (status, first) = json_request(
        &app,
        "agent-a-token",
        "POST",
        "/v1/events",
        json!({
            "workspace_id": "m3-alpha",
            "events": [{
                "event_id": "m3-redis",
                "kind": "decision",
                "subject": " persistence ",
                "content": "Persistence uses Redis."
            }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first[0]["outcome"], "PROMOTED");
    let redis_id = first[0]["promoted_memory_id"].as_str().unwrap().to_owned();

    let (status, duplicate) = json_request(
        &app,
        "agent-a-token",
        "POST",
        "/v1/events",
        json!({
            "workspace_id": "m3-alpha",
            "events": [{
                "event_id": "m3-redis-duplicate",
                "kind": "decision",
                "subject": "persistence",
                "content": "  Persistence   uses Redis. "
            }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(duplicate[0]["outcome"], "DEDUPED");
    assert_eq!(duplicate[0]["promoted_memory_id"], redis_id);

    let (status, replacement) = json_request(
        &app,
        "agent-a-token",
        "POST",
        "/v1/events",
        json!({
            "workspace_id": "m3-alpha",
            "events": [{
                "event_id": "m3-dynamo",
                "kind": "decision",
                "subject": "persistence",
                "content": "Persistence uses DynamoDB."
            }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replacement[0]["outcome"], "PROMOTED");

    let (status, packet) = json_request(
        &app,
        "agent-b-token",
        "POST",
        "/v1/context",
        json!({
            "workspace_id": "m3-alpha",
            "query": "Implement persistence",
            "max_tokens": 100
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(packet["summary"].as_str().unwrap().contains("DynamoDB"));
    assert!(!packet["summary"].as_str().unwrap().contains("Redis"));
    assert_eq!(
        packet["evidence"][0]["provenance"]["source_agent"],
        "agent-a"
    );
    assert!(
        packet["evidence"][0]["raw_handle"]
            .as_str()
            .unwrap()
            .starts_with("artifact://")
    );

    let (status, history) = json_request(
        &app,
        "agent-b-token",
        "GET",
        "/v1/memories?workspace_id=m3-alpha&status=superseded",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(history[0]["id"], redis_id);

    let (status, secret) = json_request(
        &app,
        "agent-a-token",
        "POST",
        "/v1/events",
        json!({
            "workspace_id": "m3-alpha",
            "events": [{
                "event_id": "m3-secret",
                "kind": "decision",
                "subject": "credentials",
                "content": "AWS_SECRET_ACCESS_KEY=synthetic-not-real"
            }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(secret[0]["outcome"], "RETAINED");
    assert!(secret[0]["promoted_memory_id"].is_null());

    let (status, current) = json_request(
        &app,
        "agent-b-token",
        "GET",
        "/v1/memories?workspace_id=m3-alpha&status=current",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(current.as_array().unwrap().len(), 1);
}
