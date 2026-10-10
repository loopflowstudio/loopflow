//! `lf task comment ISSUE` against an isolated Linear GraphQL fixture.

use clap::Parser;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use axum::{extract::State, routing::post, Json, Router};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::{task_comment_async, PmTestContext, TaskCommentAuthor, PM_TEST_CONTEXT};
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
    posted: Vec<Value>,
    lose_reply: bool,
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
    } else if query.contains("query ListInitiativeProjects") {
        // Comment delivery remains independent of unavailable repository inventory.
        return Json(json!({"errors":[{"message":"inventory unavailable"}]}));
    } else if query.contains("query IssueOwnership") {
        json!({"issue": {"id":"issue-uuid","identifier":"FIX-7","url":null,"title":"Comments",
            "description":"","completedAt": null, "dueDate": null, "prioritySortOrder":0.0,"sortOrder":0.0, "updatedAt":"2026-09-29T12:00:00.123Z","assignee":null,
            "state":{"type":"unstarted"},"team":{"id":"team-1"},
            "project":{"id":"project-1","name":"Chapter","description":"","content":"workflow: feature",
                "status":{"type":"started"},
                "initiatives":{"nodes":[{"id":"initiative-1"}]},"teams":{"nodes":[{"id":"team-1"}]}}}})
    } else if query.contains("mutation SyncComment") {
        if thread == Thread::Failing {
            return Json(json!({"errors":[{"message":"offline"}]}));
        }
        let id = vars["id"].as_str().unwrap();
        if provider.posted.iter().any(|comment| comment["id"] == id) {
            return Json(json!({"errors":[{"message":"ID already exists"}]}));
        }
        provider.posted.push(comment(
            id,
            Some("2026-09-27T00:00:00Z"),
            vars["body"].as_str(),
            Value::Null,
        ));
        if provider.lose_reply {
            provider.lose_reply = false;
            return Json(json!({"errors":[{"message":"lost reply after commit"}]}));
        }
        json!({"commentCreate":{"comment":{"id":id}}})
    } else if query.contains("query CommentDelivery") {
        let mut comment = provider
            .posted
            .iter()
            .find(|c| c["id"] == vars["id"])
            .cloned()
            .unwrap_or(Value::Null);
        if !comment.is_null() {
            comment["issue"] = json!({"id":"issue-uuid"});
        }
        json!({"comment":comment})
    } else if query.contains("query IssueObservation") {
        assert_eq!(
            vars["id"], "issue-uuid",
            "read uses the resolved provider UUID"
        );
        if thread == Thread::Failing {
            return Json(json!({"errors":[{"message":"rate limited"}]}));
        }
        let comments = match thread {
            Thread::Empty => page(provider.posted.clone(), None, false),
            _ => page(
                vec![comment(
                    "c-3",
                    Some("2026-09-24T12:00:00Z"),
                    Some("Third, written last.\n\n```sh\nlf wave status\n```"),
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
                        Some("- first\n  - nested\n\n[spec](https://example.com)\n\n<!-- loopflow-steer:legacy -->"),
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
#[allow(clippy::await_holding_lock)] // isolates capture provenance
async fn task_comments_read_and_publish_without_starting_work() {
    let _lock = crate::journal::test_env_lock();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let directory = tempfile::tempdir().unwrap();
    std::env::set_var("LF_HOME", directory.path());
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
    let database = directory.path().join("loopflow.db");
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
    for name in ["product", "other"] {
        store
            .create_wave(&crate::work::wave::Wave::new(
                crate::id::WaveId::new(),
                name.into(),
                repo.display().to_string(),
            ))
            .await
            .unwrap();
    }
    let provider = Arc::new(Mutex::new(Provider {
        thread: Thread::Paged,
        queries: Vec::new(),
        posted: Vec::new(),
        lose_reply: true,
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
            let read = task_comment_async(&repo, None, "FIX-7", None, false)
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
                "- first\n  - nested\n\n[spec](https://example.com)\n\n<!-- loopflow-steer:legacy -->"
            );
            // An explicit steer through an integration still speaks for its requester.
            assert_eq!(
                read.comments[2].author,
                TaskCommentAuthor::Person {
                    name: Some("Jack".into())
                }
            );
            assert!(read.comments[3].body.ends_with("```sh\nlf wave status\n```"));
            assert_eq!(
                read.comments[3].created_at.as_deref(),
                Some("2026-09-24T12:00:00Z")
            );

            let wrong_wave = task_comment_async(&repo, Some("other"), "FIX-7", None, false)
                .await.unwrap_err();
            assert!(wrong_wave.to_string().contains("belongs to product"));

            provider.lock().await.thread = Thread::MissingCursor;
            let truncated = task_comment_async(&repo, None, "FIX-7", None, false).await.unwrap();
            assert!(truncated.refresh_error.unwrap().contains("continuation cursor"));
            assert_eq!(truncated.comments, read.comments);

            let task = store.get_task_by_issue("FIX-7").await.unwrap().unwrap();
            rusqlite::Connection::open(directory.path().join("loopflow.db")).unwrap()
                .execute("INSERT INTO task_state_deliveries(id,task_id,target,error,base_state,base_revision,attempted)
                    VALUES('pending-completion',?1,'completed','unrelated completion remains pending','unstarted','2026-09-29T12:00:00.123Z',1)",
                    [task.id.as_str()]).unwrap();
            provider.lock().await.thread = Thread::Failing;
            let saved = task_comment_async(&repo, None, "FIX-7", Some("Keep the public name"), true)
                .await.unwrap();
            assert_eq!(saved.comments.len(), 5);
            assert_eq!(saved.pending_sync.len(), 1);
            let id = saved.pending_sync[0].clone();
            assert!(provider.lock().await.posted.is_empty());
            assert!(!store.task_started(&task.id).await.unwrap());
            crate::ops::linear_observe::sync_task_comments(&store, &task).await.unwrap();
            assert_eq!(store.sqlite.pending_task_comments(&task.id).unwrap().len(), 1);
            let saved_steers = store.task_steers(&task.id).await.unwrap();

            provider.lock().await.thread = Thread::Empty;
            // The provider commits but loses its first reply. Concurrent catch-up
            // keeps the same UUID and confirms the exact issue and body.
            let (one, two) = tokio::join!(
                crate::ops::linear_observe::sync_task_comments(&store, &task),
                crate::ops::linear_observe::sync_task_comments(&store, &task),
            );
            one.unwrap(); two.unwrap();
            assert_eq!(provider.lock().await.posted.len(), 1);
            assert_eq!(provider.lock().await.posted[0]["id"], id);
            assert!(store.sqlite.pending_task_comments(&task.id).unwrap().is_empty());
            crate::ops::linear_observe::refresh_task_comments(&store, &task).await.unwrap();
            crate::ops::linear_observe::refresh_task_comments(&store, &task).await.unwrap();
            assert_eq!(store.task_steers(&task.id).await.unwrap(), saved_steers);
            assert_eq!(store.sqlite.task_comments(&task.id).unwrap().comments.len(), 5);

            provider.lock().await.posted.push(comment("incoming-after-reconnect",
                Some("2026-09-27T00:00:01Z"), Some("Keep inbound independent"),
                json!({"id":"person-1","displayName":"Maya","name":null})));
            // Exercise the ordinary runner, including the native branch and
            // Claude's batch subprocess branch. The stub never consumes stdin.
            let _homes = crate::test_ambient::EnvGuard::clear(&["HOME", "LF_HOME", "LF_BIN", "PATH", "CODEX_HOME", "CLAUDE_CONFIG_DIR"]);
            let home = directory.path().join("home");
            std::fs::create_dir_all(&home).unwrap();
            std::env::set_var("HOME", &home);
            std::env::set_var("LF_HOME", &home);
            let bin = directory.path().join("bin");
            std::fs::create_dir(&bin).unwrap();
            std::env::set_var("PATH", format!("{}:/usr/bin:/bin", bin.display()));
            std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
            let script = r#"#!/bin/sh
: > "$FIXTURE_STARTED"
i=0
while [ ! -e "$FIXTURE_STOP" ] && [ "$i" -lt 100 ]; do
    /bin/sleep 0.1
    i=$((i+1))
done
exit 0
"#;
            for name in ["claude", "codex"] {
                let executable = bin.join(name);
                std::fs::write(&executable, script).unwrap();
                std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            for (agent, auto) in [("codex", false), ("claude", false), ("claude", true)] {
                let marker = format!("{agent}-{auto}");
                let incoming = format!("incoming-{marker}");
                provider.lock().await.posted.push(comment(&incoming,
                    Some("2026-09-27T00:00:02Z"), Some(&marker),
                    json!({"id":"person-1","displayName":"Maya","name":null})));
                let started = home.join(format!("{marker}.started"));
                let stop = home.join(format!("{marker}.stop"));
                let seed = crate::ops::task_input::TaskSeed {
                    task: task.clone(), message: String::new(), steers: Vec::new(),
                    steer: 0, interrupt: 0,
                };
                let process = crate::engine::agent::ProcessConfig {
                    auto,
                    task_input: Some(crate::ops::task_input::TaskInput::new(store.clone(), seed)),
                    ..Default::default()
                };
                let launch = crate::engine::agent::AgentConfig {
                    agent: Some(agent.into()), cwd: Some(repo.clone()),
                    env: std::collections::BTreeMap::from([
                        ("PATH".into(), format!("{}:/usr/bin:/bin", bin.display())),
                        ("FIXTURE_STARTED".into(), started.display().to_string()),
                        ("FIXTURE_STOP".into(), stop.display().to_string()),
                    ]),
                    ..Default::default()
                };
                let context = PM_TEST_CONTEXT.with(Clone::clone);
                // The blocking invocation and its capture share this fixture's store.
                let ledger = crate::store::database_path_from_env().unwrap();
                let running = tokio::task::spawn_blocking(move || {
                    crate::journal::with_test_ledger(ledger, || {
                        PM_TEST_CONTEXT.sync_scope(context, || crate::engine::agent::run_agent(
                            &launch, &process, &crate::engine::agent::AgentCapabilities::default()))
                    })
                });
                let running_check = tokio::time::timeout(std::time::Duration::from_secs(7), async {
                    loop {
                        if started.exists() { break; }
                        if running.is_finished() { return false; }
                        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    }
                    let saved = task_comment_async(&repo, None, "FIX-7", Some(&marker), true).await.unwrap();
                    assert_eq!(saved.pending_sync.len(), 1);
                    loop {
                        if store.sqlite.pending_task_comments(&task.id).unwrap().is_empty()
                            && store.sqlite.task_comments(&task.id).unwrap().comments.iter()
                                .any(|c| c.id == incoming) { return true; }
                        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    }
                }).await;
                std::fs::write(&stop, "stop").unwrap();
                let result = running.await.unwrap().unwrap();
                assert_eq!(result.exit_code, 0);
                assert!(running_check.unwrap(), "{marker} exited before synchronization");
            }
            let saved = task_comment_async(&repo, None, "FIX-7", Some("Local value survives"), true)
                .await.unwrap();
            let collision = saved.pending_sync[0].clone();
            provider.lock().await.posted.push(comment(&collision,
                Some("2026-09-27T00:00:03Z"), Some("Provider value survives"),
                json!({"id":"person-1","displayName":"Maya","name":null})));
            crate::ops::linear_observe::refresh_task_comments(&store, &task).await.unwrap();
            crate::ops::linear_observe::sync_task_comments(&store, &task).await.unwrap();
            let conflict = store.sqlite.task_comments(&task.id).unwrap();
            assert!(conflict.conflicts[&collision].starts_with("Local value survives"));
            assert!(conflict.comments.iter().find(|c| c.id == collision).unwrap().body == "Provider value survives");
            assert!(!conflict.pending_sync.contains(&collision));
            // A late acknowledgement records delivery, not authority to discard
            // the newer remote value acquired while the create was in flight.
            store.sqlite.record_comment_delivery(&collision, None).unwrap();
            crate::ops::linear_observe::refresh_task_comments(&store, &task).await.unwrap();
            assert_eq!(store.sqlite.task_comments(&task.id).unwrap().conflicts, conflict.conflicts);

            // An acknowledged create with a retained collision must not hide
            // newer provider direction as an echo of the saved local body.
            {
                let mut remote = provider.lock().await;
                let amended = remote.posted.iter_mut().find(|c| c["id"] == collision).unwrap();
                amended["body"] = json!("Provider correction survives");
                amended["updatedAt"] = json!("2026-09-25T09:00:01Z");
            }
            crate::ops::linear_observe::refresh_task_comments(&store, &task).await.unwrap();
            assert_eq!(store.sqlite.task_comments(&task.id).unwrap().comments.iter().find(|c| c.id == collision).unwrap().body, "Provider correction survives");
            assert!(store.task_steers(&task.id).await.unwrap().iter().any(|steer| steer.text.contains("Provider correction survives")));

            let before_retry = store.task_steers(&task.id).await.unwrap();
            let posted_before = provider.lock().await.posted.len();
            crate::ops::linear_observe::sync_task_comments(&store, &task).await.unwrap();
            crate::ops::linear_observe::refresh_task_comments(&store, &task).await.unwrap();
            assert!(store.sqlite.pending_task_comments(&task.id).unwrap().is_empty());
            assert_eq!(provider.lock().await.posted.len(), posted_before);
            assert_eq!(store.task_steers(&task.id).await.unwrap(), before_retry);
            assert!(crate::lf::Cli::try_parse_from(["lf", "task", "sync", "FIX-7", "--resolve", "local"]).is_err());

            let delivered = store.task_steers(&task.id).await.unwrap();
            assert_eq!(delivered.iter().filter(|steer| steer.text.contains("incoming-after-reconnect")).count(), 1);
            assert!(matches!(store.get_task(&task.id).await.unwrap().unwrap().pm_writeback,
                crate::work::task::PmWritebackState::Pending {
                    operation: crate::work::task::PmWritebackOperation::CompleteTask, ..
                }));
            assert!(!store.task_started(&task.id).await.unwrap());

        })
        .await;
    server.abort();

    // Reading a thread leaves the existing Wave inventory unchanged.
    assert_eq!(store.list_waves(None).await.unwrap().len(), 2);
}
