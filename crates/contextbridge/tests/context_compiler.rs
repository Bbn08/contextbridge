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
async fn context_prefers_current_decisions_and_is_deterministic() {
    let mut tokens = HashMap::new();
    tokens.insert("agent-a-token".into(), AgentId("agent-a".into()));
    let app = local_app("sqlite::memory:", tokens).await.unwrap();

    for memory in [
        json!({
            "workspace_id": "m4-alpha",
            "kind": "finding",
            "subject": "persistence",
            "content": "Persistence has a storage adapter."
        }),
        json!({
            "workspace_id": "m4-alpha",
            "kind": "decision",
            "subject": "persistence",
            "content": "Persistence uses DynamoDB."
        }),
    ] {
        let (status, _) = request(&app, "agent-a-token", "POST", "/v1/memories", memory).await;
        assert_eq!(status, StatusCode::OK);
    }

    let body = json!({
        "workspace_id": "m4-alpha",
        "query": "Implement persistence",
        "max_tokens": 100
    });
    let (first_status, first) =
        request(&app, "agent-a-token", "POST", "/v1/context", body.clone()).await;
    let (second_status, second) = request(&app, "agent-a-token", "POST", "/v1/context", body).await;
    assert_eq!(first_status, StatusCode::OK);
    assert_eq!(second_status, StatusCode::OK);
    assert_eq!(first["evidence"][0]["kind"], "decision");
    assert_eq!(first["evidence"], second["evidence"]);
    assert_eq!(first["summary"], second["summary"]);
    assert_eq!(first["token_usage"], second["token_usage"]);
    assert!(
        first["evidence"][0]["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| reason == "subject match")
    );
    assert!(
        first["token_usage"]["selected"].as_u64().unwrap()
            <= first["token_usage"]["budget"].as_u64().unwrap()
    );
}
