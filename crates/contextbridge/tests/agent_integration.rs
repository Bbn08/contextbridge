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

#[tokio::test]
async fn independent_agent_http_boundary_records_context_and_recovers_evidence() {
    let mut tokens = HashMap::new();
    tokens.insert("agent-a-token".into(), AgentId("agent-a".into()));
    tokens.insert("agent-b-token".into(), AgentId("agent-b".into()));
    let app = local_app("sqlite::memory:", tokens).await.unwrap();

    let (status, bytes) = request(
        &app,
        "agent-a-token",
        "POST",
        "/v1/events",
        json!({
            "workspace_id": "m5-alpha",
            "events": [{
                "event_id": "m5-decision",
                "kind": "decision",
                "subject": "persistence",
                "content": "Persistence uses DynamoDB.",
                "raw_evidence": "Agent A's full persistence decision."
            }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let event_result: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(event_result[0]["outcome"], "PROMOTED");

    let (status, bytes) = request(
        &app,
        "agent-b-token",
        "POST",
        "/v1/context",
        json!({
            "workspace_id": "m5-alpha",
            "query": "Implement persistence",
            "max_tokens": 100
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let packet: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        packet["evidence"][0]["provenance"]["source_agent"],
        "agent-a"
    );
    let handle = packet["evidence"][0]["raw_handle"].as_str().unwrap();
    let hash = handle.strip_prefix("artifact://m5-alpha/").unwrap();

    let (status, raw) = request(
        &app,
        "agent-b-token",
        "GET",
        &format!("/v1/evidence/{hash}?workspace_id=m5-alpha"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(raw, b"Agent A's full persistence decision.");
}
