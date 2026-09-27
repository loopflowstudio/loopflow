//! `lf pm task comments` against an isolated Linear GraphQL fixture.

use std::sync::Arc;

use axum::{extract::State, routing::post, Json, Router};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::{pm_task_comments_async, PmTestContext, TaskCommentAuthor, PM_TEST_CONTEXT};
use crate::store::{open_ephemeral_store, CredentialType, ProviderToken, StorageConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Thread {
    Paged,
    Empty,
    MissingCursor,
    Failing,
}

struct Provider {
    thread: Thread,
    queries: Vec<String>,
}

fn comment(id: &str, created: Option<&str>, body: Option<&str>, user: Value) -> Value {
    let mut node = json!({"id": id, "updatedAt": "2026-09-25T09:00:00Z", "user": user});
    if let Some(created) = created {
        node["createdAt"] = json!(created);
    }
    if let Some(body) = body {
        node["body"] = json!(body);
    }
    node
}

fn page(nodes: Vec<Value>, next: Option<&str>, has_next: bool) -> Value {
    json!({"nodes": nodes, "pageInfo": {"hasNextPage": has_next, "endCursor": next}})
}

async fn graphql(
    State(state): State<Arc<Mutex<Provider>>>,
    Json(request): Json<Value>,
) -> Json<Value> {
    let query = request["query"].as_str().unwrap().to_string();
    let vars = request["variables"].clone();
    let mut provider = state.lock().await;
    provider.queries.push(query.clone());
    let thread = provider.thread;
    let data = if query.contains("query ListTeams") {
        json!({"teams": {"nodes": [{"id":"team-1","name":"Fixture","key":"FIX",
            "description":"<!-- loopflow-repository: loopflowstudio/fixture -->"}]}})
    } else if query.contains("query IssueOwnership") {
        json!({"issue": {"id":"issue-uuid","identifier":"FIX-7","url":null,"title":"Comments",
            "description":"","prioritySortOrder":0.0,"sortOrder":0.0,"assignee":null,
            "state":{"type":"unstarted"},"team":{"id":"team-1"},
            "project":{"id":"project-1","name":"Chapter","description":"","content":"",
                "initiatives":{"nodes":[{"id":"initiative-1"}]},"teams":{"nodes":[{"id":"team-1"}]}}}})
    } else if query.contains("query IssueObservation") {
        assert_eq!(
            vars["id"], "issue-uuid",
            "read uses the resolved provider UUID"
        );
        if thread == Thread::Failing {
            return Json(json!({"errors":[{"message":"rate limited"}]}));
        }
        let comments = match thread {
            Thread::Empty => page(vec![], None, false),
            _ => page(
                vec![comment(
                    "c-3",
                    Some("2026-09-24T12:00:00Z"),
                    Some("Third, written last.\n\n```sh\nlf status\n```"),
                    json!({"id":"person-1","displayName":"Jack","name":"Jack H"}),
                )],
                Some("cursor-1"),
                true,
            ),
        };
        json!({"issue": {"updatedAt":"2026-09-25T09:00:00Z","title":"Comments",
            "description":"Current brief.","comments": comments}})
    } else if query.contains("query IssueComments") {
        let comments = match (thread, vars["after"].as_str()) {
            (Thread::MissingCursor, _) => page(vec![], None, true),
            (_, Some("cursor-1")) => page(
                vec![
                    comment(
                        "c-1",
                        Some("2026-09-22T08:00:00Z"),
                        Some("- first\n  - nested\n\n[spec](https://example.com)"),
                        json!({"id":"person-2","displayName":"Maya","name":null}),
                    ),
                    comment("c-bot", None, None, Value::Null),
                ],
                Some("cursor-2"),
                true,
            ),
            (_, Some("cursor-2")) => page(
                vec![comment(
                    "c-2",
                    Some("2026-09-23T08:00:00Z"),
                    Some("Keep the API.\n\n<!-- loopflow-requester:\"Jack\" -->\n\n<!-- loopflow-steer:x -->"),
                    Value::Null,
                )],
                None,
                false,
            ),
            other => panic!("unexpected continuation {other:?}"),
        };
        json!({"issue": {"comments": comments}})
    } else {
        panic!("unexpected fixture operation: {query}")
    };
    Json(json!({"data": data}))
}

#[tokio::test]
async fn task_comments_read_every_page_in_written_order_without_writes() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().join("repo");
    std::fs::create_dir_all(repo.join(".lf")).unwrap();
    std::fs::create_dir_all(repo.join("wave/product")).unwrap();
    std::fs::create_dir_all(repo.join("wave/other")).unwrap();
    for args in [
        vec!["init", "-q"],
        vec![
            "remote",
            "add",
            "origin",
            "https://github.com/loopflowstudio/fixture.git",
        ],
    ] {
        assert!(std::process::Command::new("git")
            .args(args)
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());
    }
    std::fs::write(
        repo.join(".lf/config.yaml"),
        "pm:\n  provider: linear\n  linear_team: team-1\n",
    )
    .unwrap();
    for (wave, initiative) in [("product", "initiative-1"), ("other", "initiative-2")] {
        std::fs::write(
            repo.join(format!("wave/{wave}/GOAL.md")),
            format!("---\npm:\n  linear_initiative: {initiative}\n---\nMandate.\n"),
        )
        .unwrap();
    }
    let repo = std::fs::canonicalize(repo).unwrap();
    let database = directory.path().join("registry.db");
    let store = Arc::new(
        open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
            .await
            .unwrap(),
    );
    store
        .upsert_provider_token(&ProviderToken {
            provider: "linear".into(),
            access_token: "fixture".into(),
            refresh_token: None,
            oauth_client_id: None,
            expires_at: None,
            login: None,
            updated_at: 1,
            credential_type: CredentialType::ApiKey,
        })
        .await
        .unwrap();
    let provider = Arc::new(Mutex::new(Provider {
        thread: Thread::Paged,
        queries: Vec::new(),
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/", post(graphql))
        .with_state(provider.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let context = PmTestContext {
        path: database,
        store: store.clone(),
        graphql_url: url,
    };
    PM_TEST_CONTEXT
        .scope(context, async {
            let read = pm_task_comments_async(&repo, Some("product"), "FIX-7")
                .await
                .unwrap();
            assert_eq!(read.identifier, "FIX-7");
            // Missing creation time sorts first; the rest follow when they were written.
            assert_eq!(
                read.comments
                    .iter()
                    .map(|c| c.id.as_str())
                    .collect::<Vec<_>>(),
                ["c-bot", "c-1", "c-2", "c-3"]
            );
            let bot = &read.comments[0];
            assert_eq!(bot.author, TaskCommentAuthor::Integration);
            assert_eq!((bot.body.as_str(), bot.created_at.as_deref()), ("", None));
            assert_eq!(
                read.comments[1].author,
                TaskCommentAuthor::Person {
                    name: Some("Maya".into())
                }
            );
            assert_eq!(
                read.comments[1].body,
                "- first\n  - nested\n\n[spec](https://example.com)"
            );
            // An explicit steer through an integration still speaks for its requester.
            assert_eq!(
                read.comments[2].author,
                TaskCommentAuthor::Person {
                    name: Some("Jack".into())
                }
            );
            assert!(read.comments[3].body.ends_with("```sh\nlf status\n```"));
            assert_eq!(
                read.comments[3].created_at.as_deref(),
                Some("2026-09-24T12:00:00Z")
            );

            let wrong_wave = pm_task_comments_async(&repo, Some("other"), "FIX-7")
                .await
                .unwrap_err();
            assert!(wrong_wave.to_string().contains("belongs to wave/product"));

            provider.lock().await.thread = Thread::Empty;
            let empty = pm_task_comments_async(&repo, Some("product"), "FIX-7")
                .await
                .unwrap();
            assert!(empty.comments.is_empty());

            provider.lock().await.thread = Thread::MissingCursor;
            let truncated = pm_task_comments_async(&repo, Some("product"), "FIX-7")
                .await
                .unwrap_err();
            assert!(truncated.to_string().contains("continuation cursor"));

            provider.lock().await.thread = Thread::Failing;
            let failed = pm_task_comments_async(&repo, Some("product"), "FIX-7")
                .await
                .unwrap_err();
            assert!(failed.to_string().contains("rate limited"));
        })
        .await;
    server.abort();

    let provider = provider.lock().await;
    assert!(provider
        .queries
        .iter()
        .all(|query| !query.trim_start().starts_with("mutation")));
    // Reading a thread registers no Work and prepares nothing.
    assert!(store.list_waves(None).await.unwrap().is_empty());
}
