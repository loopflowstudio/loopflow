//! Task planning operations against stateful, isolated Linear responses.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser;
use serde_json::json;

use super::test_fixture::{now, Fixture};
use super::{PmRefresh, PmTestContext, PM_TEST_CONTEXT};
use crate::child::ChildRef;
use crate::durable::WorkStatus;
use crate::ops::NullProgress;
use crate::planning::{LinearProjectId, ProjectPlan};
use crate::pm::PmSnapshot;
use crate::store::{open_ephemeral_store, StorageConfig};
use crate::work::project::{Project, ProjectId};
use crate::work::task::{
    AfterMerge, GithubObservation, GithubObservationResult, GithubPr, Observation,
    PmWritebackState, PrMergeMode, PrMergeRequest, PrPhase, PrPresentation, PrPublication, Task,
    TaskEventKind, TaskId, TaskPr, TaskPrId,
};
use crate::work::wave::Wave;

impl Fixture {
    async fn seed_current_chapter(&self, wave: &Wave) {
        let now = now();
        self.store
            .save_chapter(
                &crate::work::chapter::Chapter {
                    id: crate::work::chapter::ChapterId::parse("fixture").unwrap(),
                    wave_id: wave.id().clone(),
                    wave: wave.name().into(),
                    project_id: "project-1".into(),
                    content: crate::ops::chapter::empty_plan(),
                    predecessors: vec![],
                    predecessor_metrics: vec![],
                    tasks: vec![],
                    phase: crate::work::chapter::ChapterPhase::Complete,
                    created_at: now,
                    activated_at: Some(now),
                    completed_at: Some(now),
                    error: None,
                },
                true,
            )
            .await
            .unwrap();
    }
}

async fn serve(
    state: Arc<tokio::sync::Mutex<PlanningState>>,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new()
        .route("/", axum::routing::post(planning_graphql))
        .with_state(state.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, server)
}

impl PlanningEnvironment {
    fn isolate() -> Self {
        let mut saved = vec![("PATH".into(), std::env::var_os("PATH"))];
        for (name, value) in
            std::env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("LF_"))
        {
            std::env::remove_var(&name);
            saved.push((name, Some(value)));
        }
        // Status reconciliation checks launch authority, but never launches lf.
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        Self(saved)
    }
}

// Stateful provider evidence for the creation/preparation boundary. The same
// fixture retains an issue after a lost response, just as Linear would.
#[derive(Default)]
struct PlanningState {
    issues: Vec<serde_json::Value>,
    fail_confirmation: bool,
    fail_snapshot: bool,
    fail_completion: bool,
    lose_completion: bool,
    lose_comment: bool,
    completion_writes: usize,
    trashed: bool,
    deletion_writes: usize,
    lose_deletion: bool,
    refuse_deletion: bool,
    unreadable_trash: bool,
    fail_deleted_snapshot: bool,
    omit_trashed_issues: bool,
    current_project_id: Option<String>,
    completion_state: Option<String>,
    comments: Vec<serde_json::Value>,
}

async fn planning_graphql(
    axum::extract::State(state): axum::extract::State<Arc<tokio::sync::Mutex<PlanningState>>>,
    axum::Json(request): axum::Json<serde_json::Value>,
) -> axum::Json<serde_json::Value> {
    let query = request["query"].as_str().unwrap();
    let vars = &request["variables"];
    let page =
        |nodes| json!({"nodes": nodes, "pageInfo": {"hasNextPage": false, "endCursor": null}});
    let project_id = state
        .lock()
        .await
        .current_project_id
        .clone()
        .unwrap_or_else(|| "project-1".into());
    let project = json!({"id":project_id, "name":"Chapter", "description":"", "content":"",
        "initiatives":{"nodes":[{"id":"initiative-1"}]}, "teams":{"nodes":[{"id":"team-1"}]}});
    let data =
        if query.contains("query ListTeams") {
            json!({"teams":{"nodes":[{"id":"team-1","name":"Fixture","key":"FIX",
            "description":"<!-- loopflow-repository: loopflowstudio/fixture -->"}]}})
        } else if query.contains("query ListInitiativeProjects") {
            let mut state = state.lock().await;
            if !state.issues.is_empty() && state.fail_snapshot {
                state.fail_snapshot = false;
                return axum::Json(json!({"errors":[{"message":"snapshot unavailable"}]}));
            }
            json!({"initiative":{"projects":page(vec![project])}})
        } else if query.contains("query ListProjectIssues") {
            let mut state = state.lock().await;
            if !state.issues.is_empty() && state.fail_confirmation {
                state.fail_confirmation = false;
                return axum::Json(json!({"errors":[{"message":"confirmation unavailable"}]}));
            }
            let issues = if state.trashed && state.omit_trashed_issues {
                vec![]
            } else {
                state.issues.clone()
            };
            json!({"project":{"issues":page(issues)}})
        } else if query.contains("query IssueOwnership") {
            let state = state.lock().await;
            if state.trashed {
                return axum::Json(
                    json!({"errors":[{"message":"ordinary ownership unavailable after trash"}]}),
                );
            }
            let mut issue = state
                .issues
                .iter()
                .find(|issue| issue["id"] == vars["id"] || issue["identifier"] == vars["id"])
                .unwrap()
                .clone();
            issue["project"] = project;
            json!({"issue":issue})
        } else if query.contains("query IssueDeletion") {
            let state = state.lock().await;
            if state.trashed && state.unreadable_trash {
                json!({"issue":null})
            } else {
                json!({"issue":{"trashed":state.trashed}})
            }
        } else if query.contains("mutation DeleteIssue") {
            let mut state = state.lock().await;
            if state.refuse_deletion || state.trashed {
                return axum::Json(json!({"data":{"issueDelete":{"success":false}}}));
            }
            state.trashed = true;
            state.deletion_writes += 1;
            state.fail_snapshot = state.fail_deleted_snapshot;
            if state.lose_deletion {
                state.lose_deletion = false;
                return axum::Json(json!({"errors":[{"message":"lost deletion response"}]}));
            }
            json!({"issueDelete":{"success":true}})
        } else if query.contains("query IssueTeam") {
            json!({"issue":{"team":{"id":"team-1"}}})
        } else if query.contains("query CompletedWorkflowStates") {
            json!({"workflowStates":{"nodes":[{"id":"completed"}]}})
        } else if query.contains("mutation SetIssueState") {
            let mut state = state.lock().await;
            if state.fail_completion {
                state.fail_completion = false;
                return axum::Json(json!({"errors":[{"message":"completion unavailable"}]}));
            }
            state.completion_writes += 1;
            let outcome = state
                .completion_state
                .take()
                .unwrap_or_else(|| "completed".into());
            state.issues[0]["state"] = json!({"type":outcome});
            if state.lose_completion {
                state.lose_completion = false;
                return axum::Json(json!({"errors":[{"message":"lost completion response"}]}));
            }
            json!({"issueUpdate":{"issue":{"id":"issue-1"}}})
        } else if query.contains("query IssueComments") {
            json!({"issue":{"comments":page(state.lock().await.comments.clone())}})
        } else if query.contains("mutation CreateComment") {
            let mut state = state.lock().await;
            let id = format!("comment-{}", state.comments.len() + 1);
            state
                .comments
                .push(json!({"id":id,"body":vars["body"],"user":null}));
            if state.lose_comment {
                state.lose_comment = false;
                return axum::Json(json!({"errors":[{"message":"lost comment response"}]}));
            }
            json!({"commentCreate":{"comment":{"id":id}}})
        } else if query.contains("mutation UpdateIssue") {
            let mut state = state.lock().await;
            let issue = state
                .issues
                .iter_mut()
                .find(|issue| issue["id"] == vars["id"])
                .unwrap();
            for key in ["title", "description"] {
                if let Some(value) = vars["input"].get(key) {
                    issue[key] = value.clone();
                }
            }
            json!({"issueUpdate":{"success":true}})
        } else if query.contains("query UnstartedWorkflowStates") {
            json!({"workflowStates":{"nodes":[{"id":"unstarted"}]}})
        } else if query.contains("mutation CreateIssue") {
            state.lock().await.issues.push(
                json!({"id":"issue-1", "identifier":"FIX-1", "url":null,
            "title":vars["title"], "description":vars["description"], "prioritySortOrder":0.0,
            "sortOrder":0.0, "assignee":null, "state":{"type":"unstarted"},
            "team":{"id":"team-1"}, "project":{"id":"project-1","name":"Chapter"}}),
            );
            return axum::Json(json!({"errors":[{"message":"lost response after commit"}]}));
        } else {
            panic!("unexpected creation fixture query: {query}");
        };
    axum::Json(json!({"data":data}))
}

#[tokio::test]
async fn task_creation_refusal_preserves_inventory_and_marker_retry_reuses_provider_title() {
    let fixture = Fixture::new().await;
    fixture.seed(now() + 86_400).await;
    let (repo, wave) = fixture.planning_repo().await;
    fixture.seed_current_chapter(&wave).await;
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState::default()));
    let (url, server) = serve(state.clone()).await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let marker = "<!-- loopflow-task-start:fixture -->";
            let refused = super::pm_create_task_idempotent(
                &repo,
                "product",
                "Original title",
                "Report",
                marker,
                |_, _| async {
                    Err::<(), _>(crate::ops::error::OpsError::Message(
                        "placement refused".into(),
                    ))
                },
            )
            .await;
            assert!(refused
                .unwrap_err()
                .to_string()
                .contains("placement refused"));
            assert!(
                state.lock().await.issues.is_empty(),
                "refusal must leave provider inventory unchanged"
            );

            let (created, ()) = super::pm_create_task_idempotent(
                &repo,
                "product",
                "Original title",
                "Report",
                marker,
                |_, _| async { Ok(()) },
            )
            .await
            .unwrap();
            assert_eq!(
                created, "issue-1",
                "lost response resolves the committed issue"
            );
            assert_eq!(state.lock().await.issues.len(), 1);
            super::pm_update_async(
                &repo,
                &super::PmUpdateOptions {
                    wave: None,
                    id: "FIX-1".into(),
                    update: super::PmTaskUpdate::Edit(crate::pm::PmItemUpdate {
                        name: Some("Persisted edited title".into()),
                        description: Some("Edited notes".into()),
                    }),
                },
                &NullProgress,
            )
            .await
            .unwrap();
            let description = state.lock().await.issues[0]["description"]
                .as_str()
                .unwrap()
                .to_string();
            assert_eq!(description, format!("Edited notes\n\n{marker}"));
            let (retried, title) = super::pm_create_task_idempotent(
                &repo,
                "product",
                "Original title",
                "Report",
                marker,
                |existing, _| async move { Ok(existing.unwrap().name) },
            )
            .await
            .unwrap();
            assert_eq!(retried, created);
            assert_eq!(title, "Persisted edited title");
            assert_eq!(state.lock().await.issues[0]["description"], description);
            assert!(fixture.store.list_tasks(None).await.unwrap().is_empty());
            // Lookup is valid for terminal planning items; launch owns eligibility.
            state.lock().await.issues[0]["state"]["type"] = json!("completed");
            let resolved =
                crate::ops::task_pm::resolve_task_async(&repo, "FIX-1", PmRefresh::Force)
                    .await
                    .unwrap();
            assert!(resolved.item.completed);
            assert_eq!(resolved.item.description, description);
            assert_eq!(
                state.lock().await.issues.len(),
                1,
                "retry cannot file a second issue"
            );

            // A second independent creation loses both the mutation response
            // and its confirmation read. Keep uncertainty and the retry path.
            state.lock().await.fail_confirmation = true;
            state.lock().await.issues.clear();
            let uncertain = super::pm_create_task_idempotent(
                &repo,
                "product",
                "Unconfirmed task",
                "Another report",
                "<!-- loopflow-task-start:unconfirmed -->",
                |_, _| async { Ok(()) },
            )
            .await;
            let error = uncertain.unwrap_err().to_string();
            assert!(error.contains("lost response after commit"), "{error}");
            assert!(error.contains("confirmation unavailable"), "{error}");
            assert!(error.contains("issue may already exist"), "{error}");
            assert!(error.contains("Retry the same `lf task create`"), "{error}");
            assert_eq!(state.lock().await.issues.len(), 1);
            let (recovered, ()) = super::pm_create_task_idempotent(
                &repo,
                "product",
                "Unconfirmed task",
                "Another report",
                "<!-- loopflow-task-start:unconfirmed -->",
                |_, _| async { Ok(()) },
            )
            .await
            .unwrap();
            assert_eq!(recovered, "issue-1");
            assert_eq!(state.lock().await.issues.len(), 1);
        })
        .await;
    server.abort();
}

#[test]
fn task_creation_snapshot_failure_retries_without_starting_backlog() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    std::env::set_var("LF_DB_PATH", &fixture.database);
    let (repo, wave) = runtime.block_on(fixture.planning_repo());
    // Deliberately no commit, Project Work, agent route or execution credential.
    std::fs::write(repo.join("authored.txt"), "keep this unfinished work").unwrap();
    runtime.block_on(fixture.seed(now() + 86_400));
    runtime.block_on(fixture.seed_current_chapter(&wave));
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState {
        fail_snapshot: true,
        ..Default::default()
    }));
    let (url, server) = runtime.block_on(serve(state.clone()));
    PM_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        let create = || {
            crate::ops::task::task_create(
                &repo,
                Some("product"),
                Some("Future work".into()),
                Some("Full directive".into()),
                None,
            )
        };
        {
            let error = create().unwrap_err().to_string();
            assert!(
                error.contains("Linear task issue-1 is committed"),
                "{error}"
            );
            assert!(error.contains("snapshot unavailable"), "{error}");
            assert!(error.contains("Retry the same `lf task create`"), "{error}");
            assert!(!error.contains("lf task run"), "{error}");
            assert_eq!(
                runtime.block_on(async { state.lock().await.issues.len() }),
                1
            );
        }
        let (first, task) = create().unwrap();
        assert!(task.is_none());
        let edit = || {
            crate::ops::task::task_edit(
                &repo,
                "FIX-1",
                None,
                Some("Edited future work".into()),
                Some("Edited full directive".into()),
            )
        };
        {
            runtime.block_on(async {
                state.lock().await.fail_snapshot = true;
            });
            let error = edit().unwrap_err().to_string();
            assert!(
                error.contains("was updated, but local refresh failed"),
                "{error}"
            );
            assert!(error.contains("Retry the same Task command"), "{error}");
        }
        edit().unwrap();
        let (retry, task) = create().unwrap();
        assert!(task.is_none());
        assert_eq!(first.id, retry.id);
        assert_eq!(first.name, "Future work");
        assert_eq!(retry.name, "Edited future work");
        assert!(retry.description.starts_with("Edited full directive"));
        let marker = first
            .description
            .split("<!-- loopflow-task-start:")
            .nth(1)
            .unwrap();
        assert!(retry.description.ends_with(marker));
        let snapshot = crate::ops::task_pm::load_wave(&repo, "product", PmRefresh::Never).unwrap();
        assert_eq!(snapshot.items, vec![retry]);
    });
    assert_eq!(
        runtime.block_on(async { state.lock().await.issues.len() }),
        1
    );
    assert!(runtime
        .block_on(fixture.store.list_tasks(None))
        .unwrap()
        .is_empty());
    assert_eq!(
        crate::engine::worktrees::list_worktrees(&repo)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        std::fs::read_to_string(repo.join("authored.txt")).unwrap(),
        "keep this unfinished work"
    );
    server.abort();
}

#[test]
#[ignore = "entry point for the deletion fixture's isolated subprocess"]
fn deletion_process_entry() {
    let input: serde_json::Value =
        serde_json::from_str(&std::env::var("LOOPFLOW_DELETION_FIXTURE").unwrap()).unwrap();
    let database = PathBuf::from(input["database"].as_str().unwrap());
    std::env::set_var("LF_HOME", database.parent().unwrap());
    std::env::set_var("LF_DB_PATH", &database);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = Arc::new(
        runtime
            .block_on(open_ephemeral_store(&StorageConfig::sqlite(
                database.clone(),
            )))
            .unwrap(),
    );
    let cli = crate::lf::Cli::try_parse_from(["lf", "task", "delete", "FIX-1"]).unwrap();
    let Some(crate::lf::Commands::Task {
        cmd: crate::lf::TaskCommand::Delete { issue },
    }) = cli.command
    else {
        panic!("expected public deletion command");
    };
    PM_TEST_CONTEXT.sync_scope(
        PmTestContext {
            path: database,
            store,
            graphql_url: input["url"].as_str().unwrap().into(),
        },
        || {
            assert_eq!(
                crate::ops::task::task_delete(Path::new(input["repo"].as_str().unwrap()), &issue,)
                    .unwrap(),
                "FIX-1"
            );
        },
    );
}

#[test]
fn task_deletion_planning_preserves_completed_outcome_and_retries_confirmation() {
    assert_planning_deletion(false, false, false);
}

#[test]
fn task_deletion_planning_recovers_lost_response_without_ordinary_ownership() {
    assert_planning_deletion(true, false, false);
}

#[test]
fn task_deletion_planning_recovers_after_local_confirmation_failure() {
    assert_planning_deletion(false, true, false);
}

#[test]
fn task_deletion_planning_retries_snapshot_failure_without_repeating_deletion() {
    assert_planning_deletion(false, false, true);
}

fn assert_planning_deletion(lost: bool, fail_local: bool, fail_snapshot: bool) {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    std::env::set_var("LF_DB_PATH", &fixture.database);
    let (repo, wave) = runtime.block_on(fixture.planning_repo());
    runtime.block_on(fixture.seed(now() + 86_400));
    runtime.block_on(fixture.seed_current_chapter(&wave));
    std::fs::write(repo.join("authored.txt"), "keep authored work").unwrap();
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState::default()));
    let (url, server) = runtime.block_on(serve(state.clone()));
    PM_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        crate::ops::task::task_create(
            &repo,
            Some("product"),
            Some("Future work".into()),
            Some("Preserve this issue's outcome".into()),
            None,
        )
        .unwrap();
    });
    runtime.block_on(async {
        let mut state = state.lock().await;
        state.issues[0]["state"] = json!({"type":"completed"});
        state.lose_deletion = lost;
        state.unreadable_trash = lost;
        state.fail_deleted_snapshot = fail_snapshot;
        state.refuse_deletion = true;
    });
    let delete = || {
        PM_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
            crate::ops::task::task_delete(&repo, "FIX-1")
        })
    };
    // A refused effect leaves the issue in ordinary discovery.
    let error = delete().unwrap_err().to_string();
    assert!(error.contains("lf task delete FIX-1"), "{error}");
    assert!(runtime
        .block_on(fixture.store.deleted_task_issues(wave.id()))
        .unwrap()
        .is_empty());
    assert_eq!(
        runtime.block_on(async { state.lock().await.deletion_writes }),
        0
    );
    let connection = rusqlite::Connection::open(&fixture.database).unwrap();
    if !lost && !fail_local && !fail_snapshot {
        // Retained observation must not authorize a mutation after ownership moves.
        runtime.block_on(async {
            let mut state = state.lock().await;
            state.refuse_deletion = false;
            state.issues[0]["team"]["id"] = json!("other-team");
        });
        let error = delete().unwrap_err().to_string();
        assert!(error.contains("belongs to Team other-team"), "{error}");
        assert_eq!(
            runtime.block_on(async { state.lock().await.deletion_writes }),
            0
        );
        runtime.block_on(async { state.lock().await.issues[0]["team"]["id"] = json!("team-1") });

        connection
            .execute_batch(
                "CREATE TRIGGER fail_identity BEFORE INSERT ON task_issue_identities
             BEGIN SELECT RAISE(FAIL, 'fixture identity unavailable'); END;",
            )
            .unwrap();
        let error = delete().unwrap_err().to_string();
        assert!(
            error.contains("no provider deletion was attempted"),
            "{error}"
        );
        assert_eq!(
            runtime.block_on(async { state.lock().await.deletion_writes }),
            0
        );
        connection
            .execute_batch("DROP TRIGGER fail_identity")
            .unwrap();
    }
    runtime.block_on(async { state.lock().await.refuse_deletion = false });
    if fail_local {
        connection
            .execute_batch(
                "CREATE TRIGGER fail_confirmation BEFORE INSERT ON task_deletions
            BEGIN SELECT RAISE(FAIL, 'fixture confirmation unavailable'); END;",
            )
            .unwrap();
    }
    if lost || fail_local || fail_snapshot {
        let error = delete().unwrap_err().to_string();
        assert!(error.contains("lf task delete FIX-1"), "{error}");
        let confirmed = runtime
            .block_on(fixture.store.deleted_task_issues(wave.id()))
            .unwrap();
        assert_eq!(confirmed.contains("issue-1"), fail_snapshot, "{error}");
        let row = runtime
            .block_on(fixture.store.pm_snapshot(wave.id()))
            .unwrap()
            .unwrap();
        let snapshot: PmSnapshot = serde_json::from_str(&row.payload).unwrap();
        assert_eq!(snapshot.items.is_empty(), fail_snapshot);
        if fail_local {
            connection
                .execute_batch("DROP TRIGGER fail_confirmation")
                .unwrap();
        }
        if lost || fail_local {
            // Ordinary refresh and replacement of the chapter both lose membership,
            // while the provider still retains the original issue in trash.
            runtime.block_on(async { state.lock().await.omit_trashed_issues = true });
            PM_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
                runtime.block_on(async {
                    let ctx = super::resolve_context(&repo, "product").await.unwrap();
                    super::refresh_pm_snapshot(&repo, "product", &ctx)
                        .await
                        .unwrap();

                    let mut successor = fixture
                        .store
                        .chapter(wave.id(), None)
                        .await
                        .unwrap()
                        .unwrap();
                    successor.id = crate::work::chapter::ChapterId::parse("successor").unwrap();
                    successor.project_id = "project-2".into();
                    fixture.store.save_chapter(&successor, true).await.unwrap();
                    state.lock().await.current_project_id = Some("project-2".into());
                    let snapshot = super::refresh_pm_snapshot(&repo, "product", &ctx)
                        .await
                        .unwrap();
                    assert!(snapshot.items.is_empty());
                    assert_eq!(snapshot.projects[0].id, "project-2");
                    assert!(fixture
                        .store
                        .deleted_task_issues(wave.id())
                        .await
                        .unwrap()
                        .is_empty());
                });
            });
        }
        if lost {
            // A second operation still cannot turn ordinary absence into success.
            assert!(delete().is_err());
        }
        runtime.block_on(async { state.lock().await.unreadable_trash = false });
    }
    if lost || fail_local {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ops::pm::task_planning_tests::deletion_process_entry",
                "--ignored",
                "--nocapture",
            ])
            .env_clear()
            .envs(
                std::env::vars_os().filter(|(name, _)| !name.to_string_lossy().starts_with("LF_")),
            )
            .env(
                "LOOPFLOW_DELETION_FIXTURE",
                json!({
                    "database": fixture.database, "repo": repo, "url": url,
                })
                .to_string(),
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        assert_eq!(delete().unwrap(), "FIX-1");
    }
    let confirmed_at: i64 = connection
        .query_row(
            "SELECT confirmed_at FROM task_deletions WHERE issue_id='issue-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    // The retained confirmation works even when all future trash reads fail.
    runtime.block_on(async { state.lock().await.unreadable_trash = true });
    assert_eq!(delete().unwrap(), "FIX-1");
    assert_eq!(
        connection
            .query_row(
                "SELECT confirmed_at FROM task_deletions WHERE issue_id='issue-1'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        confirmed_at
    );
    let row = runtime
        .block_on(fixture.store.pm_snapshot(wave.id()))
        .unwrap()
        .unwrap();
    assert!(serde_json::from_str::<PmSnapshot>(&row.payload)
        .unwrap()
        .items
        .is_empty());
    runtime.block_on(async {
        let state = state.lock().await;
        assert_eq!(state.deletion_writes, 1);
        assert_eq!(state.completion_writes, 0);
        assert_eq!(state.issues[0]["state"]["type"], "completed");
    });
    assert!(runtime
        .block_on(fixture.store.list_tasks(None))
        .unwrap()
        .is_empty());
    assert_eq!(
        std::fs::read_to_string(repo.join("authored.txt")).unwrap(),
        "keep authored work"
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    server.abort();
}

struct PlanningEnvironment(Vec<(std::ffi::OsString, Option<std::ffi::OsString>)>);

#[test]
fn task_completion_preserves_planning_identity_and_summary_on_retry() {
    assert_task_completion_retry(false, false, None);
}

#[test]
fn task_completion_retries_pending_writeback_after_local_done() {
    assert_task_completion_retry(true, false, None);
}

#[test]
fn task_completion_reconciles_lost_planning_response() {
    assert_task_completion_retry(false, true, None);
}

#[test]
fn task_completion_reconciles_lost_registered_response() {
    assert_task_completion_retry(true, true, None);
}

#[test]
fn task_completion_reconciles_provider_outcome_after_user_merge() {
    assert_task_completion_retry(true, false, Some(PrMergeMode::User));
}

#[test]
fn task_completion_reconciles_provider_outcome_after_landing() {
    assert_task_completion_retry(true, true, Some(PrMergeMode::Auto));
}

fn assert_task_completion_retry(registered: bool, lose_response: bool, merge: Option<PrMergeMode>) {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    std::env::set_var("LF_DB_PATH", &fixture.database);
    let (repo, wave) = runtime.block_on(fixture.planning_repo());
    if merge.is_some() {
        let bin = fixture.directory.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let gh = bin.join("gh");
        std::fs::write(&gh, r#"#!/bin/sh
if [ "$1" = "api" ]; then
  head=$(git rev-parse HEAD)
  printf '{"merged":true,"state":"closed","draft":false,"merge_commit_sha":"merge-42","merged_at":"2026-09-27T01:00:00Z","number":42,"html_url":"https://github.com/loopflowstudio/fixture/pull/42","head":{"sha":"%s"}}\n' "$head"
fi
"#).unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut paths = vec![bin];
        paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
        std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
    }

    runtime.block_on(fixture.seed(now() + 86_400));
    runtime.block_on(fixture.seed_current_chapter(&wave));
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState::default()));
    let (url, server) = runtime.block_on(serve(state.clone()));
    PM_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        let (item, _) = crate::ops::task::task_create(
            &repo,
            Some("product"),
            Some("Future work".into()),
            Some("A directive".into()),
            None,
        )
        .unwrap();
        let task = registered.then(|| {
            for args in [
                vec!["add", "."],
                vec![
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@example.com",
                    "commit",
                    "-qm",
                    "Baseline",
                ],
            ] {
                assert!(std::process::Command::new("git")
                    .args(args)
                    .current_dir(&repo)
                    .status()
                    .unwrap()
                    .success());
            }
            let timestamp = time::OffsetDateTime::now_utc();
            let project = Project {
                id: ProjectId::new(),
                plan: ProjectPlan {
                    id: LinearProjectId::new("project-1").unwrap(),
                    slug: "chapter".into(),
                    name: "Chapter".into(),
                    prompt_context: String::new(),
                    pm_snapshot_synced_at: 1,
                },
                wave_id: wave.id().clone(),
                iteration: 0,
                abandon_intent: None,
                created_at: timestamp,
                updated_at: timestamp,
            };
            runtime
                .block_on(fixture.store.create_project(&project))
                .unwrap();
            let task = Task {
                id: TaskId::new(),
                plan: crate::planning::TaskPlan {
                    id: crate::planning::LinearIssueId::new(&item.id).unwrap(),
                    identifier: item.identifier.clone(),
                    title: item.name.clone(),
                    description: item.description.clone(),
                    pm_snapshot_synced_at: 1,
                },
                pm_writeback: PmWritebackState::Current,
                wave_id: wave.id().clone(),
                project_id: project.id,
                worktree: repo.clone(),
                workspace_slug: "completion".into(),
                abandon_intent: None,
                created_at: timestamp,
                updated_at: timestamp,
                observation: Observation::NotRequired,
                agent: None,
            };
            let mut pr = TaskPr {
                id: TaskPrId::new(),
                task_id: task.id.clone(),
                sequence: 1,
                slug: "completion".into(),
                branch: crate::engine::git::current_branch(&repo).unwrap().unwrap(),
                base_commit: crate::engine::git::rev_parse(&repo, "HEAD").unwrap(),
                parent_pr_id: None,
                publication: None,
                merge_commit: None,
                abandoned_at: None,
                ci_observation: None,
                github_observation: None,
                linear_attachment_id: None,
                linear_comment_id: None,
                linear_link_error: None,
                created_at: timestamp,
                updated_at: timestamp,
            };
            runtime
                .block_on(fixture.store.create_task(&task, &pr))
                .unwrap();
            pr.abandoned_at = merge.is_none().then_some(timestamp);
            pr.publication = Some(PrPublication {
                requested_at: timestamp,
                presentation: merge.map(|_| PrPresentation {
                    title: "Deliver the requested outcome".into(),
                    body: "Completion reconciles the provider outcome.".into(),
                    head_sha: pr.base_commit.clone(),
                }),
                github: Some(GithubPr {
                    number: 42,
                    url: "https://github.com/loopflowstudio/fixture/pull/42".into(),
                    head_sha: Some(pr.base_commit.clone()),
                }),
                merge: merge.map(|mode| PrMergeRequest {
                    mode,
                    requested_at: timestamp,
                    head_sha: pr.base_commit.clone(),
                    after_merge: AfterMerge::CompleteTask,
                    next_slug: None,
                }),
            });
            pr.github_observation = merge.is_none().then_some(GithubObservation {
                checked_at: timestamp,
                result: GithubObservationResult::Fresh,
            });
            runtime.block_on(fixture.store.update_task_pr(&pr)).unwrap();
            task
        });
        let selector = task
            .as_ref()
            .map_or(item.identifier.as_str(), |task| task.id.as_str());
        let complete = |summary: &str| match merge {
            None => crate::ops::task::task_complete(&repo, selector, summary.into()),
            Some(PrMergeMode::User) => crate::ops::task::task_status(Some(selector)).map(Some),
            Some(PrMergeMode::Auto) => runtime.block_on(async {
                let task = task.as_ref().unwrap();
                let pr = fixture.store.task_prs(&task.id).await.unwrap().remove(0);
                let landing = crate::pr_landing::PrLanding::new(
                    crate::pr_landing::NewPrLanding {
                        repo: "loopflowstudio/fixture".into(),
                        pr_number: 42,
                        worktree: repo.clone(),
                        branch: pr.branch,
                        task_id: Some(task.id.clone()),
                        requested_head_sha: pr.base_commit,
                        after_merge: Some(AfterMerge::CompleteTask),
                        next_slug: None,
                    },
                    time::OffsetDateTime::now_utc(),
                )
                .unwrap();
                crate::ops::task::settle_task_landing(&fixture.store, &landing).await?;
                Ok(fixture.store.get_task(&task.id).await.unwrap())
            }),
        };
        if let Some(task) = &task {
            let original_prs = runtime.block_on(fixture.store.task_prs(&task.id)).unwrap();
            for terminal in ["canceled", "duplicate"] {
                runtime.block_on(async {
                    state.lock().await.issues[0]["state"] = json!({"type":terminal});
                });
                let error = complete("Cannot change the outcome").unwrap_err();
                assert!(
                    matches!(
                        error,
                        crate::ops::error::OpsError::TaskCompletionConflict { .. }
                    ),
                    "{error}"
                );
                runtime.block_on(async {
                    let work = fixture
                        .store
                        .work_for_child(&ChildRef::Task(task.id.clone()))
                        .await
                        .unwrap();
                    assert_eq!(
                        fixture.store.work_status(&work).await.unwrap(),
                        WorkStatus::Ready
                    );
                    let events = fixture.store.task_events_after(&task.id, 0).await.unwrap();
                    assert!(!events.iter().any(|event| matches!(
                        event.kind,
                        TaskEventKind::Completed { .. } | TaskEventKind::Progress { .. }
                    )));
                    assert_eq!(state.lock().await.completion_writes, 0);
                    let prs = fixture.store.task_prs(&task.id).await.unwrap();
                    if merge.is_some() {
                        assert_eq!(prs[0].phase(), PrPhase::Merged);
                        assert_eq!(prs[0].merge_commit.as_deref(), Some("merge-42"));
                        assert_eq!(
                            events
                                .iter()
                                .filter(|event| matches!(
                                    event.kind,
                                    TaskEventKind::PrMerged { .. }
                                ))
                                .count(),
                            1
                        );
                    } else {
                        assert_eq!(prs, original_prs);
                    }
                });
            }
            if merge.is_some() {
                // The merge is now durable. Provider completion must remain
                // retryable even when GitHub can no longer be reached.
                std::fs::write(
                    fixture.directory.path().join("bin/gh"),
                    "#!/bin/sh\necho 'GitHub unavailable after recorded merge' >&2\nexit 1\n",
                )
                .unwrap();
            }
            // Linear can change between the eligibility read and snapshot
            // confirmation. Known conflicts at either boundary refuse success.
            for terminal in ["canceled", "duplicate"] {
                runtime.block_on(async {
                    let mut provider = state.lock().await;
                    provider.issues[0]["state"] = json!({"type":"unstarted"});
                    provider.completion_state = Some(terminal.into());
                });
                assert!(matches!(
                    complete("Outcome changed during completion").unwrap_err(),
                    crate::ops::error::OpsError::TaskCompletionConflict { .. }
                ));
                runtime.block_on(async {
                    let work = fixture.store.work_for_child(&ChildRef::Task(task.id.clone())).await.unwrap();
                    assert_eq!(fixture.store.work_status(&work).await.unwrap(), WorkStatus::Ready);
                    let events = fixture.store.task_events_after(&task.id, 0).await.unwrap();
                    assert!(!events.iter().any(|event| matches!(event.kind, TaskEventKind::Completed { .. } | TaskEventKind::Progress { .. })));
                });
            }
            runtime.block_on(async {
                state.lock().await.issues[0]["state"] = json!({"type":"unstarted"});
            });
        }
        runtime.block_on(async {
            let mut provider = state.lock().await;
            provider.fail_completion = !lose_response;
            provider.lose_completion = lose_response;
        });
        let first = complete("Delivered the requested outcome");
        let mut before = None;
        if let Some(task) = &task {
            let first = first.unwrap().unwrap();
            if merge.is_none() {
                assert!(matches!(first.observation, Observation::Cached { .. }));
            } else {
                assert_eq!(first.observation, Observation::NotRequired);
            }
            assert!(matches!(
                first.pm_writeback,
                PmWritebackState::Pending { .. }
            ));
            let events = runtime
                .block_on(fixture.store.task_events_after(&task.id, 0))
                .unwrap();
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(event.kind, TaskEventKind::Completed { .. }))
                    .count(),
                1
            );
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            let terminal: i64 = conn
                .query_row(
                    "SELECT work_terminal_at FROM tasks WHERE id=?1",
                    [task.id.as_str()],
                    |row| row.get(0),
                )
                .unwrap();
            before = Some((events, terminal));
        } else {
            assert!(first.unwrap_err().to_string().contains(if lose_response {
                "lost completion response"
            } else {
                "completion unavailable"
            }));
            runtime.block_on(async {
                let mut provider = state.lock().await;
                provider.fail_snapshot = true;
                provider.lose_comment = true;
            });
            assert!(complete("Delivered the requested outcome")
                .unwrap_err()
                .to_string()
                .contains("local refresh failed"));
        }
        if registered {
            runtime.block_on(async {
                let mut provider = state.lock().await;
                provider.fail_snapshot = true;
                provider.issues[0]["title"] = json!("Updated before completion retry");
                provider.issues[0]["description"] = json!("Retain the provider's latest notes");
            });
            let retry = complete("Delivered the requested outcome")
                .unwrap()
                .unwrap();
            assert!(matches!(
                retry.pm_writeback,
                PmWritebackState::Pending { ref error, .. }
                    if error.contains("local refresh failed")
            ));
        }
        let result = complete("Delivered the requested outcome").unwrap();
        if let Some(task) = &task {
            let result = result.unwrap();
            assert_eq!(result.pm_writeback, PmWritebackState::Current);
            assert_eq!(result.plan.title, "Updated before completion retry");
            assert_eq!(
                result.plan.description,
                "Retain the provider's latest notes"
            );
            let retained = runtime
                .block_on(fixture.store.get_task(&task.id))
                .unwrap()
                .unwrap();
            assert_eq!(retained.plan.title, "Updated before completion retry");
            assert_eq!(
                retained.plan.description,
                "Retain the provider's latest notes"
            );
            complete("Must not replace the first completion").unwrap();
            for terminal in ["canceled", "duplicate"] {
                runtime.block_on(async {
                    state.lock().await.issues[0]["state"] = json!({"type":terminal});
                    fixture.store.update_task_pm_writeback(
                        &task.id,
                        &PmWritebackState::Pending {
                            operation: crate::work::task::PmWritebackOperation::CompleteTask,
                            error: "retry after provider interruption".into(),
                        },
                        retained.updated_at,
                    ).await.unwrap();
                });
                let historical = complete("Must preserve recorded success").unwrap().unwrap();
                assert!(matches!(historical.pm_writeback, PmWritebackState::Pending { ref error, .. } if error.contains("cannot be completed")));
                runtime.block_on(async {
                    let work = fixture.store.work_for_child(&ChildRef::Task(task.id.clone())).await.unwrap();
                    assert_eq!(fixture.store.work_status(&work).await.unwrap(), WorkStatus::Done);
                });
            }
            let (events, terminal) = before.unwrap();
            assert_eq!(
                runtime
                    .block_on(fixture.store.task_events_after(&task.id, 0))
                    .unwrap(),
                events
            );
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            assert_eq!(
                conn.query_row(
                    "SELECT work_terminal_at FROM tasks WHERE id=?1",
                    [task.id.as_str()],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
                terminal
            );
        } else {
            assert!(result.is_none());
            complete("Must not replace the first completion").unwrap();
            assert!(runtime
                .block_on(fixture.store.list_tasks(None))
                .unwrap()
                .is_empty());
            let comments = runtime.block_on(async { state.lock().await.comments.clone() });
            assert_eq!(comments.len(), 1);
            assert!(comments[0]["body"]
                .as_str()
                .unwrap()
                .contains("Delivered the requested outcome"));
            assert!(!crate::ops::linear_observe::is_direction_comment(
                comments[0]["body"].as_str().unwrap(),
                Some("person-1"),
            ));
            for terminal in ["canceled", "duplicate"] {
                runtime.block_on(async {
                    state.lock().await.issues[0]["state"] = json!({"type":terminal});
                });
                assert!(complete("Cannot change the outcome")
                    .unwrap_err()
                    .to_string()
                    .contains("cannot be completed"));
            }
        }
        assert_eq!(
            runtime.block_on(async { state.lock().await.completion_writes }),
            if registered { 3 } else { 1 }
        );
        assert_eq!(
            crate::engine::worktrees::list_worktrees(&repo)
                .unwrap()
                .len(),
            1
        );
    });
    server.abort();
}

impl Drop for PlanningEnvironment {
    fn drop(&mut self) {
        for (name, _) in
            std::env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("LF_"))
        {
            std::env::remove_var(name);
        }
        for (name, value) in &self.0 {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}
