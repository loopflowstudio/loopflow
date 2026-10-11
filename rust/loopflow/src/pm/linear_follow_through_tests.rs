use std::sync::Arc;

use axum::{extract::State, routing::post, Json, Router};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::pm::linear::LinearClient;

async fn respond(
    State(related): State<Arc<Mutex<bool>>>,
    Json(request): Json<Value>,
) -> Json<Value> {
    let mut related = related.lock().await;
    let query = request["query"].as_str().unwrap();
    if query.contains("FollowUpRelationExists") {
        Json(json!({"data": {"issue": {"relations": {
            "nodes": if *related { vec![json!({"type":"related","relatedIssue":{"id":"target"}})] } else { vec![] },
            "pageInfo":{"hasNextPage":false,"endCursor":null}
        }}}}))
    } else if query.contains("FollowUpRelation") {
        *related = true;
        Json(json!({"errors":[{"message":"relation response lost after commit"}]}))
    } else {
        panic!("unexpected fixture query: {query}");
    }
}

#[tokio::test]
async fn lost_relation_response_is_reconciled_without_duplicate_linkage() {
    let state = Arc::new(Mutex::new(false));
    let app = Router::new()
        .route("/", post(respond))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = LinearClient::with_base_url("fixture-token".into(), Some("team".into()), endpoint);
    for _ in 0..2 {
        client
            .ensure_follow_up_relation("source", "target", "relation")
            .await
            .unwrap();
    }
    assert!(*state.lock().await);
    server.abort();
}
