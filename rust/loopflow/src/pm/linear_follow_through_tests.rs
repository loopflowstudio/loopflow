use std::collections::BTreeMap;
use std::sync::Arc;

use axum::{extract::State, routing::post, Json, Router};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::pm::linear::LinearClient;
use crate::work::task::follow_through::FollowThroughIntent;

#[derive(Debug, Default)]
struct Provider {
    issues: BTreeMap<String, Value>,
    related: bool,
}

async fn respond(
    State(state): State<Arc<Mutex<Provider>>>,
    Json(request): Json<Value>,
) -> Json<Value> {
    let mut provider = state.lock().await;
    let query = request["query"].as_str().unwrap();
    let vars = &request["variables"];
    let data = if query.contains("FollowUpExists") {
        let id = vars["id"].as_str().unwrap();
        json!({"issues": {"nodes": if provider.issues.contains_key(id) { vec![json!({"id": id})] } else { vec![] }}})
    } else if query.contains("FollowUpCreate") {
        let input = vars["input"].clone();
        let id = input["id"].as_str().unwrap().to_string();
        provider.issues.entry(id).or_insert(input);
        // The provider commits but its response is lost; even the first caller
        // must recover the same issue through the durable UUID.
        return Json(json!({"errors": [{"message": "response lost after commit"}]}));
    } else if query.contains("FollowUpRelationExists") {
        json!({"issue": {"relations": {"nodes": if provider.related { vec![json!({"type": "related", "relatedIssue": {"id": provider.issues.keys().next().unwrap()}})] } else { vec![] }, "pageInfo": {"hasNextPage": false, "endCursor": null}}}})
    } else if query.contains("FollowUpRelation") {
        provider.related = true;
        return Json(json!({"errors": [{"message": "relation response lost after commit"}]}));
    } else if query.contains("IssueOwnership") {
        let id = vars["id"].as_str().unwrap();
        let input = &provider.issues[id];
        json!({"issue": {"id": id, "identifier": "LOO-900", "url": "https://linear.app/fixture/issue/LOO-900",
            "title": input["title"], "description": input["description"], "dueDate": input["dueDate"],
            "updatedAt": "2026-10-07T00:00:00Z", "completedAt": null, "branchName": null,
            "prioritySortOrder": 1.0, "sortOrder": 1.0, "assignee": null, "state": {"type": "unstarted"},
            "team": {"id": "team"}, "project": null}})
    } else if query.contains("workflowStates") {
        json!({"workflowStates": {"nodes": [{"id": "todo", "position": 1.0}]}})
    } else {
        panic!("unexpected fixture query: {query}");
    };
    Json(json!({"data": data}))
}

#[tokio::test]
async fn lost_responses_and_concurrent_retries_preserve_one_issue_and_relation() {
    let state = Arc::new(Mutex::new(Provider::default()));
    let app = Router::new()
        .route("/", post(respond))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = LinearClient::with_base_url("fixture-token".into(), Some("team".into()), endpoint);
    let intent = FollowThroughIntent {
        key: "installed".into(),
        issue_id: uuid::Uuid::new_v4().to_string(),
        relation_id: uuid::Uuid::new_v4().to_string(),
        project_id: "pinned-project".into(),
        team_id: "team".into(),
        state_id: Some("todo".into()),
        wave: "product".into(),
        title: "Installed proof".into(),
        notes: "Follow-up to LOO-418; check the installed release".into(),
        due: Some("2026-10-08".into()),
        existing: false,
    };
    let (first, second) = tokio::join!(
        client.confirm_follow_up(&intent),
        client.confirm_follow_up(&intent)
    );
    assert_eq!(first.unwrap().id, intent.issue_id);
    assert_eq!(second.unwrap().id, intent.issue_id);
    client
        .ensure_follow_up_relation("source", &intent.issue_id, &intent.relation_id)
        .await
        .unwrap();
    client
        .ensure_follow_up_relation("source", &intent.issue_id, &intent.relation_id)
        .await
        .unwrap();
    {
        let mut provider = state.lock().await;
        assert_eq!(provider.issues.len(), 1);
        assert_eq!(
            provider.issues[&intent.issue_id]["projectId"],
            "pinned-project"
        );
        assert_eq!(provider.issues[&intent.issue_id]["dueDate"], "2026-10-08");
        provider.issues.get_mut(&intent.issue_id).unwrap()["title"] = json!("Edited after filing");
        assert!(provider.related);
    }
    let recovered = client.confirm_follow_up(&intent).await.unwrap();
    assert_eq!(recovered.name, "Edited after filing");
    assert_eq!(recovered.id, intent.issue_id);
    server.abort();
}
