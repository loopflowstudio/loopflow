use std::sync::Arc;

use axum::{extract::State, routing::post, Json, Router};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::LinearClient;

#[derive(Default)]
struct Issue {
    trashed: bool,
    state: &'static str,
    deletions: usize,
    lose_response: bool,
    refuse: bool,
    unreadable: bool,
    hide_after_delete: bool,
    read_error: bool,
}

async fn graphql(
    State(state): State<Arc<Mutex<Issue>>>,
    Json(request): Json<Value>,
) -> Json<Value> {
    let mut issue = state.lock().await;
    let query = request["query"].as_str().unwrap();
    let data = if query.contains("query IssueDeletion") {
        if issue.unreadable {
            if issue.read_error {
                return Json(json!({"errors": [{"message": "issue access unavailable"}]}));
            }
            json!({"issue": null})
        } else {
            json!({"issue": {"trashed": issue.trashed}})
        }
    } else if query.contains("mutation DeleteIssue") {
        if issue.trashed {
            // One possible provider response, not an assumed idempotency contract.
            return Json(json!({"errors": [{"message": "deleted issue inaccessible"}]}));
        }
        if issue.refuse {
            return Json(json!({"data": {"issueDelete": {"success": false}}}));
        }
        issue.deletions += 1;
        issue.trashed = true;
        issue.unreadable = issue.hide_after_delete;
        if issue.lose_response {
            issue.lose_response = false;
            return Json(json!({"errors": [{"message": "response lost after deletion"}]}));
        }
        // The schema permits a null entity. Its absence cannot negate success.
        json!({"issueDelete": {"success": true, "entity": null}})
    } else {
        panic!("unexpected fixture operation: {query}");
    };
    Json(json!({"data": data}))
}

async fn serve(issue: Issue) -> (String, Arc<Mutex<Issue>>, tokio::task::JoinHandle<()>) {
    let state = Arc::new(Mutex::new(issue));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/", post(graphql))
        .with_state(state.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, state, server)
}

fn client(url: &str) -> LinearClient {
    LinearClient::with_base_url("fixture-token".into(), None, url.into())
}

#[tokio::test]
async fn deletion_preserves_outcome_and_repeated_removal() {
    for outcome in ["unstarted", "completed", "canceled", "duplicate"] {
        let (url, state, server) = serve(Issue {
            state: outcome,
            ..Issue::default()
        })
        .await;
        client(&url).delete_item("FIX-1").await.unwrap();
        // A fresh client has no local confirmation to rely on.
        client(&url).delete_item("FIX-1").await.unwrap();
        let issue = state.lock().await;
        assert!(issue.trashed);
        assert_eq!(issue.state, outcome);
        assert_eq!(issue.deletions, 1);
        server.abort();
    }
}

#[tokio::test]
async fn deletion_confirms_lost_response_only_with_explicit_trash_evidence() {
    let (url, state, server) = serve(Issue {
        state: "unstarted",
        lose_response: true,
        ..Issue::default()
    })
    .await;
    client(&url).delete_item("FIX-1").await.unwrap();
    client(&url).delete_item("FIX-1").await.unwrap();
    let issue = state.lock().await;
    assert!(issue.trashed);
    assert_eq!(issue.state, "unstarted");
    assert_eq!(issue.deletions, 1);
    server.abort();
}

#[tokio::test]
async fn deletion_keeps_unreadable_lost_response_unresolved_across_clients() {
    let (url, state, server) = serve(Issue {
        state: "completed",
        lose_response: true,
        hide_after_delete: true,
        ..Issue::default()
    })
    .await;
    let error = client(&url).delete_item("FIX-1").await.unwrap_err();
    assert!(error.to_string().contains("response lost after deletion"));
    assert!(error
        .to_string()
        .contains("absence does not confirm deletion"));
    let error = client(&url).delete_item("FIX-1").await.unwrap_err();
    assert!(error
        .to_string()
        .contains("absence does not confirm deletion"));
    {
        let mut issue = state.lock().await;
        assert!(issue.trashed);
        assert_eq!(issue.deletions, 1);
        assert_eq!(issue.state, "completed");
        issue.unreadable = false;
    }
    // Readability is a fixture variant, not a promise of Linear trash access.
    client(&url).delete_item("FIX-1").await.unwrap();
    assert_eq!(state.lock().await.deletions, 1);
    server.abort();
}

#[tokio::test]
async fn deletion_does_not_treat_refusal_as_success() {
    let (url, state, server) = serve(Issue {
        state: "unstarted",
        refuse: true,
        ..Issue::default()
    })
    .await;
    let error = client(&url).delete_item("FIX-1").await.unwrap_err();
    assert!(error.to_string().contains("success=false"));
    {
        let mut issue = state.lock().await;
        assert!(!issue.trashed);
        assert_eq!(issue.deletions, 0);
        issue.refuse = false;
    }
    client(&url).delete_item("FIX-1").await.unwrap();
    assert!(state.lock().await.trashed);
    server.abort();
}

#[tokio::test]
async fn deletion_acknowledgement_survives_unreadable_trash() {
    let (url, state, server) = serve(Issue {
        state: "completed",
        hide_after_delete: true,
        read_error: true,
        ..Issue::default()
    })
    .await;
    client(&url).delete_item("FIX-1").await.unwrap();
    let error = client(&url).delete_item("FIX-1").await.unwrap_err();
    assert!(error.to_string().contains("issue access unavailable"));
    let issue = state.lock().await;
    assert!(issue.trashed);
    assert_eq!(issue.deletions, 1);
    assert_eq!(issue.state, "completed");
    server.abort();
}

#[tokio::test]
async fn deletion_can_confirm_from_mutation_when_read_is_unavailable() {
    let (url, state, server) = serve(Issue {
        state: "unstarted",
        unreadable: true,
        read_error: true,
        hide_after_delete: true,
        ..Issue::default()
    })
    .await;
    client(&url).delete_item("FIX-1").await.unwrap();
    let issue = state.lock().await;
    assert!(issue.trashed);
    assert_eq!(issue.deletions, 1);
    assert_eq!(issue.state, "unstarted");
    server.abort();
}
