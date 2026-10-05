//! Task planning operations against stateful, isolated Linear responses.

use std::cell::RefCell;
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
use crate::store::{open_ephemeral_store, StorageConfig};
use crate::work::task::{
    AfterMerge, GithubObservation, GithubObservationResult, GithubPr, Observation,
    PmWritebackState, PrMergeMode, PrMergeRequest, PrPhase, PrPresentation, PrPublication, Task,
    TaskEventKind, TaskId, TaskPr, TaskPrId,
};

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
    extra_projects: Vec<serde_json::Value>,
    project_name: Option<String>,
    fail_confirmation: bool,
    fail_snapshot: bool,
    fail_issue_read_after: Option<usize>,
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
    attachments: Vec<String>,
    move_on_attachment_read: bool,
}

fn mark_issue_updated(issue: &mut serde_json::Value) {
    issue["updatedAt"] = json!(time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap());
}

fn planning_project(id: &str, current: &str) -> serde_json::Value {
    let completed = id == "prior-project" || (id == "project-1" && current == "project-2");
    let name = match id {
        "project-1" => "Chapter",
        "prior-project" => "Previous chapter",
        _ => "Next chapter",
    };
    json!({"id":id, "name":name, "description":"", "content":"flow: feature",
        "status":{"type":if completed { "completed" } else { "started" }},
        "updatedAt":if completed { "2026-09-30T12:00:01Z" } else { "2026-09-30T12:00:00Z" },
        "archivedAt":null,
        "initiatives":{"nodes":[{"id":"initiative-1"}]}, "teams":{"nodes":[{"id":"team-1"}]}})
}

async fn planning_graphql(
    axum::extract::State(state): axum::extract::State<Arc<tokio::sync::Mutex<PlanningState>>>,
    axum::Json(request): axum::Json<serde_json::Value>,
) -> axum::Json<serde_json::Value> {
    let query = request["query"].as_str().unwrap();
    let vars = &request["variables"];
    let page =
        |nodes| json!({"nodes": nodes, "pageInfo": {"hasNextPage": false, "endCursor": null}});
    let mut state = state.lock().await;
    let project_id = state
        .current_project_id
        .clone()
        .unwrap_or_else(|| "project-1".into());
    let mut project = planning_project(&project_id, &project_id);
    if let Some(name) = &state.project_name {
        project["name"] = json!(name);
    }
    let data = if query.contains("query ListTeams") {
        json!({"teams":{"nodes":[{"id":"team-1","name":"Fixture","key":"FIX",
            "description":"<!-- loopflow-repository: loopflowstudio/fixture -->"}]}})
    } else if query.contains("query ListInitiatives") {
        json!({"initiatives":page(vec![json!({"id":"initiative-1", "name":"Product", "description":""})])})
    } else if query.contains("mutation RenameProject") {
        if let Some(project) = state
            .extra_projects
            .iter_mut()
            .find(|project| project["id"] == vars["id"])
        {
            project["name"] = vars["name"].clone();
        } else {
            state.project_name = Some(vars["name"].as_str().unwrap().into());
        }
        json!({"projectUpdate":{"success":true}})
    } else if query.contains("query ListInitiativeProjects") {
        if !state.issues.is_empty() && state.fail_snapshot {
            state.fail_snapshot = false;
            return axum::Json(json!({"errors":[{"message":"snapshot unavailable"}]}));
        }
        let mut projects = if project_id == "prior-project" {
            vec![project, planning_project("project-1", &project_id)]
        } else {
            vec![project]
        };
        projects.extend(state.extra_projects.clone());
        json!({"initiative":{"projects":page(projects)}})
    } else if query.contains("query ListProjectIssues") {
        if state
            .extra_projects
            .iter()
            .any(|project| project["id"] == vars["projectId"])
        {
            return axum::Json(
                json!({"errors":[{"message":"foreign Project issues are unavailable"}]}),
            );
        }
        if !state.issues.is_empty() && state.fail_confirmation {
            state.fail_confirmation = false;
            return axum::Json(json!({"errors":[{"message":"confirmation unavailable"}]}));
        }
        let issues = if state.trashed && state.omit_trashed_issues {
            vec![]
        } else {
            state.issues.clone()
        };
        let issues = issues
            .into_iter()
            .filter(|issue| issue["project"]["id"] == vars["projectId"])
            .collect::<Vec<_>>();
        json!({"project":{"issues":page(issues)}})
    } else if query.contains("query ProjectOwnership") {
        let owned = planning_project(vars["id"].as_str().unwrap(), &project_id);
        json!({"project": owned})
    } else if query.contains("query IssueOwnership") {
        if let Some(remaining) = state.fail_issue_read_after.as_mut() {
            if *remaining == 0 {
                state.fail_issue_read_after = None;
                return axum::Json(
                    json!({"errors":[{"message":"issue confirmation unavailable"}]}),
                );
            }
            *remaining -= 1;
        }
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
        if state.trashed && state.unreadable_trash {
            json!({"issue":null})
        } else {
            json!({"issue":{"trashed":state.trashed}})
        }
    } else if query.contains("mutation DeleteIssue") {
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
    } else if query.contains("query CanceledWorkflowStates") {
        json!({"workflowStates":{"nodes":[{"id":"canceled"}]}})
    } else if query.contains("query CompletedWorkflowStates") {
        json!({"workflowStates":{"nodes":[{"id":"completed"}]}})
    } else if query.contains("mutation SetIssueState") {
        if state.fail_completion {
            state.fail_completion = false;
            return axum::Json(json!({"errors":[{"message":"completion unavailable"}]}));
        }
        state.completion_writes += 1;
        let outcome = state
            .completion_state
            .take()
            .unwrap_or_else(|| vars["stateId"].as_str().unwrap().to_string());
        state.issues[0]["state"] = json!({"type":outcome});
        mark_issue_updated(&mut state.issues[0]);
        if state.lose_completion {
            state.lose_completion = false;
            return axum::Json(json!({"errors":[{"message":"lost completion response"}]}));
        }
        json!({"issueUpdate":{"issue":{"id":"issue-1"}}})
    } else if query.contains("query IssueAttachments") {
        if state.move_on_attachment_read {
            state.move_on_attachment_read = false;
            state.current_project_id = Some("project-1".into());
            state.issues[0]["project"]["id"] = json!("project-1");
            mark_issue_updated(&mut state.issues[0]);
        }
        json!({"issue":{"attachments":page(state.attachments.iter().map(|url| json!({"url":url})).collect::<Vec<_>>())}})
    } else if query.contains("query IssueComments") {
        json!({"issue":{"comments":page(state.comments.clone())}})
    } else if query.contains("mutation CreateComment") {
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
        mark_issue_updated(issue);
        json!({"issueUpdate":{"success":true}})
    } else if query.contains("query UnstartedWorkflowStates") {
        json!({"workflowStates":{"nodes":[{"id":"unstarted"}]}})
    } else if query.contains("mutation CreateIssue") {
        state
            .issues
            .push(json!({"id":"issue-1", "identifier":"FIX-1", "url":null,
            "title":vars["title"], "description":vars["description"], "completedAt": null, "prioritySortOrder":0.0,
            "sortOrder":0.0, "updatedAt":"2026-09-29T12:00:00.123Z", "assignee":null, "state":{"type":"unstarted"},
            "team":{"id":"team-1"}, "project":{"id":"project-1","name":"Chapter"}}));
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
    let (repo, _wave) = fixture.planning_repo().await;
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
            mark_issue_updated(&mut state.lock().await.issues[0]);
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
fn task_creation_and_edit_do_not_require_a_post_write_wave_snapshot() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    let (repo, _wave) = runtime.block_on(fixture.planning_repo());
    runtime.block_on(fixture.seed(now() + 86_400));
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState {
        // The initial Project read works, but after creation a Wave snapshot
        // is unavailable. Exact issue reads still work.
        fail_snapshot: true,
        ..Default::default()
    }));
    let (url, server) = runtime.block_on(serve(state.clone()));
    PM_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        let crate::ops::task::TaskCreateResult::Created(created) = crate::ops::task::task_create(
            &repo,
            Some("product"),
            Some("Continue training".into()),
            Some("Retain the issue".into()),
            None,
        )
        .unwrap() else {
            panic!("backlog creation unexpectedly launched work")
        };
        crate::ops::task::task_edit(
            &repo,
            &created.identifier,
            None,
            Some("Continue the existing training Task".into()),
            None,
        )
        .unwrap();
        let record =
            crate::ops::task_pm::resolve_task(&repo, &created.id, PmRefresh::Never).unwrap();
        assert_eq!(record.item.name, "Continue the existing training Task");
        assert_eq!(record.item.id, created.id);
        assert_eq!(
            runtime.block_on(async { state.lock().await.issues.len() }),
            1
        );
        assert!(runtime
            .block_on(fixture.store.list_tasks(None))
            .unwrap()
            .is_empty());
    });
    server.abort();
}

#[test]
fn task_creation_confirmation_failure_retries_without_starting_backlog() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    let (repo, _wave) = runtime.block_on(fixture.planning_repo());
    // Deliberately no commit, Project Work, agent route or execution credential.
    std::fs::write(repo.join("authored.txt"), "keep this unfinished work").unwrap();
    runtime.block_on(fixture.seed(now() + 86_400));
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState {
        fail_issue_read_after: Some(0),
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
            assert!(error.contains("issue confirmation unavailable"), "{error}");
            assert!(error.contains("Retry the same `lf task create`"), "{error}");
            assert!(!error.contains("lf task run"), "{error}");
            assert_eq!(
                runtime.block_on(async { state.lock().await.issues.len() }),
                1
            );
        }
        let crate::ops::task::TaskCreateResult::Created(first) = create().unwrap() else {
            panic!("creation without --run must return the issue");
        };
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
                state.lock().await.fail_issue_read_after = Some(1);
            });
            let error = edit().unwrap_err().to_string();
            assert!(
                error.contains("was updated, but local refresh failed"),
                "{error}"
            );
            assert!(error.contains("Retry the same Task command"), "{error}");
        }
        edit().unwrap();
        let crate::ops::task::TaskCreateResult::Created(retry) = create().unwrap() else {
            panic!("creation without --run must return the issue");
        };
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
        assert_eq!(snapshot.items, vec![*retry]);
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
    let (repo, wave) = runtime.block_on(fixture.planning_repo());
    runtime.block_on(fixture.seed(now() + 86_400));
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
        mark_issue_updated(&mut state.issues[0]);
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
        let snapshot = row.snapshot;
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
    assert!(row.snapshot.items.is_empty());
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
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState::default()));
    let (url, server) = runtime.block_on(serve(state.clone()));
    PM_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        let crate::ops::task::TaskCreateResult::Created(item) = crate::ops::task::task_create(
            &repo,
            Some("product"),
            Some("Future work".into()),
            Some("A directive".into()),
            None,
        )
        .unwrap() else {
            panic!("creation without --run must return the issue");
        };
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
            let project = runtime
                .block_on(fixture.store.get_project_by_project("project-1"))
                .unwrap()
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
            None | Some(PrMergeMode::User) => crate::ops::task::task_complete(&repo, selector, summary.into()),
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
                let settlement = crate::ops::task::settle_task_landing(&fixture.store, &landing).await;
                let retained = fixture.store.get_task(&task.id).await.unwrap().unwrap();
                if let PmWritebackState::Pending { error, .. } = &retained.pm_writeback {
                    assert_eq!(
                        settlement.unwrap_err().to_string(),
                        format!("Linear completion pending: {error}")
                    );
                } else {
                    settlement?;
                }
                Ok(Some(retained))
            }),
        };
        if let Some(task) = &task {
            let original_prs = runtime.block_on(fixture.store.task_prs(&task.id)).unwrap();
            for terminal in ["canceled", "duplicate"] {
                runtime.block_on(async {
                    state.lock().await.issues[0]["state"] = json!({"type":terminal});
                    mark_issue_updated(&mut state.lock().await.issues[0]);
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
                    mark_issue_updated(&mut provider.issues[0]);
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
                mark_issue_updated(&mut state.lock().await.issues[0]);
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
                provider.fail_issue_read_after = Some(1);
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
                provider.fail_issue_read_after = Some(1);
                provider.issues[0]["title"] = json!("Updated before completion retry");
                provider.issues[0]["description"] = json!("Retain the provider's latest notes");
                mark_issue_updated(&mut provider.issues[0]);
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
                    mark_issue_updated(&mut state.lock().await.issues[0]);
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
                    mark_issue_updated(&mut state.lock().await.issues[0]);
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

#[test]
fn task_abandon_and_delete_compose_cancellation_pr_and_git_from_anywhere() {
    let _lock = crate::journal::test_env_lock();
    for (selector, delete) in [
        (Some("FIX-1"), false),
        (Some("cancel-me"), false),
        (None, false),
        (Some("FIX-1"), true),
    ] {
        let _restore = PlanningEnvironment::isolate();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let fixture = runtime.block_on(Fixture::new());
        std::env::set_var("LF_HOME", fixture.directory.path());
        let (repo, wave) = runtime.block_on(fixture.planning_repo());
        runtime.block_on(fixture.seed(now() + 86_400));
        let remote = fixture.directory.path().join("loopflowstudio/fixture.git");
        std::fs::create_dir_all(&remote).unwrap();
        let git = |cwd: &Path, args: &[&str]| {
            let output = std::process::Command::new("git")
                .args(args)
                .current_dir(cwd)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap()
        };
        git(&remote, &["init", "--bare", "-q"]);
        git(&repo, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        git(&repo, &["config", "user.name", "Fixture"]);
        git(&repo, &["config", "user.email", "fixture@example.com"]);
        git(
            &repo,
            &["remote", "set-url", "origin", remote.to_str().unwrap()],
        );
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-qm", "baseline"]);
        git(&repo, &["push", "-u", "origin", "main"]);
        let checkout = fixture.directory.path().join("task");
        git(
            &repo,
            &[
                "worktree",
                "add",
                "-b",
                "cancel-me",
                checkout.to_str().unwrap(),
            ],
        );
        git(&checkout, &["push", "-u", "origin", "cancel-me"]);
        let bin = fixture.directory.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let gh = bin.join("gh");
        // Git is real; only GitHub is simulated. State survives separate calls.
        std::fs::write(&gh, r#"#!/bin/sh
root=$(git rev-parse --git-common-dir)
case "$2" in
  list) if [ -f "$root/pr-closed" ]; then echo '[{"number":42,"state":"CLOSED","headRepository":{"nameWithOwner":"loopflowstudio/fixture"}}]'; else echo '[{"number":42,"state":"OPEN","headRepository":{"nameWithOwner":"loopflowstudio/fixture"}}]'; fi ;;
  close) if [ -f "$root/fail-close" ]; then echo 'GitHub unavailable' >&2; exit 1; fi; touch "$root/pr-closed" ;;
  *) echo 'Unexpected GitHub operation' >&2; exit 1 ;;
esac
"#).unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::env::var_os("PATH").unwrap();
        std::env::set_var(
            "PATH",
            std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path))).unwrap(),
        );
        let state = Arc::new(tokio::sync::Mutex::new(PlanningState::default()));
        let (url, server) = runtime.block_on(serve(state.clone()));
        PM_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
            let crate::ops::task::TaskCreateResult::Created(item) = crate::ops::task::task_create(
                &repo,
                Some("product"),
                Some("Cancel me".into()),
                Some("Keep history".into()),
                None,
            )
            .unwrap() else {
                panic!("planning issue")
            };
            let timestamp = time::OffsetDateTime::now_utc();
            let project = runtime
                .block_on(fixture.store.list_projects(Some(wave.id())))
                .unwrap()
                .into_iter()
                .find(|project| project.plan.id.as_str() == "project-1")
                .unwrap();
            let task = Task {
                id: TaskId::new(),
                plan: crate::planning::TaskPlan {
                    id: crate::planning::LinearIssueId::new(&item.id).unwrap(),
                    identifier: item.identifier.clone(),
                    title: item.name,
                    description: item.description,
                    pm_snapshot_synced_at: 1,
                },
                pm_writeback: PmWritebackState::Current,
                wave_id: wave.id().clone(),
                project_id: project.id,
                worktree: checkout.clone(),
                workspace_slug: "cancel-me".into(),
                abandon_intent: None,
                created_at: timestamp,
                updated_at: timestamp,
                observation: Observation::NotRequired,
                agent: None,
            };
            let pr = TaskPr {
                id: TaskPrId::new(),
                task_id: task.id.clone(),
                sequence: 1,
                slug: "cancel-me".into(),
                branch: "cancel-me".into(),
                base_commit: git(&repo, &["rev-parse", "HEAD"]).trim().into(),
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
            runtime
                .block_on(fixture.store.start_task_flow(
                    &task.id,
                    crate::durable::FlowSession {
                        task_id: Some(task.id.clone()),
                        wave_id: Some(task.wave_id.clone()),
                        cwd: task.worktree.clone(),
                        message: None,
                        model: None,
                        finished: false,
                        invocation: crate::durable::test_flow_invocation(
                            "review",
                            0,
                            "review",
                            Some("review"),
                            true,
                        ),
                        current_attempt: None,
                        pending_session_id: None,
                        ready_summary: None,
                        cursor: Default::default(),
                        version: 0,
                        worker_generation: 0,
                        claim: None,
                        failure: None,
                        updated_at: timestamp,
                    },
                ))
                .unwrap();
            assert_eq!(
                crate::ops::task::task_repository(&checkout, None).unwrap(),
                checkout.canonicalize().unwrap()
            );
            let caller = match selector {
                None => &checkout,
                Some("FIX-1") => fixture.directory.path(),
                Some(_) => &repo,
            };
            // A provider refusal cannot be mistaken for local cancellation.
            runtime.block_on(async {
                state.lock().await.fail_completion = true;
            });
            assert!(crate::ops::task::task_abandon(caller, selector, false).is_err());
            assert_eq!(
                runtime
                    .block_on(
                        fixture
                            .store
                            .work_status(&crate::durable::WorkRef::Task(task.id.clone()))
                    )
                    .unwrap(),
                WorkStatus::Ready
            );
            assert!(checkout.exists());
            if selector == Some("cancel-me") {
                let progress = LifecycleMessages::default();
                crate::ops::abandon_branch(
                    &repo,
                    &crate::ops::AbandonOptions {
                        branch: Some("cancel-me".into()),
                        force: true,
                    },
                    &progress,
                )
                .unwrap();
                assert!(progress
                    .0
                    .borrow()
                    .iter()
                    .any(|message| message.contains("Task FIX-1")
                        && message.contains("remain unchanged")));
                assert_eq!(
                    runtime
                        .block_on(
                            fixture
                                .store
                                .work_status(&crate::durable::WorkRef::Task(task.id.clone()))
                        )
                        .unwrap(),
                    WorkStatus::Ready
                );
                assert_eq!(
                    runtime
                        .block_on(async { state.lock().await.issues[0]["state"]["type"].clone() }),
                    json!("unstarted")
                );
                assert!(!checkout.exists());
                // A reopened PR on an already-abandoned record must be excluded
                // by preview as well as apply, even after its checkout is gone.
                std::fs::remove_file(repo.join(".git/pr-closed")).unwrap();
                runtime.block_on(async {
                    let mut state = state.lock().await;
                    state.current_project_id = Some("prior-project".into());
                    state.issues[0]["project"]["id"] = json!("prior-project");
                    mark_issue_updated(&mut state.issues[0]);
                });
                for apply in [false, true] {
                    let entries = crate::ops::task::task_sweep(&repo, apply).unwrap();
                    assert_eq!(entries.len(), 1);
                    assert!(entries[0].outcome.contains("open PR"), "{:?}", entries);
                    assert_eq!(
                        runtime.block_on(async {
                            state.lock().await.issues[0]["state"]["type"].clone()
                        }),
                        json!("unstarted")
                    );
                    assert!(!repo.join(".git/pr-closed").exists());
                }
                std::fs::write(repo.join(".git/pr-closed"), "").unwrap();
                runtime.block_on(async {
                    let mut state = state.lock().await;
                    state.current_project_id = None;
                    state.issues[0]["project"]["id"] = json!("project-1");
                    mark_issue_updated(&mut state.issues[0]);
                });
            } else {
                // A later GitHub failure preserves enough evidence for a retry.
                std::fs::write(repo.join(".git/fail-close"), "").unwrap();
                assert!(crate::ops::task::task_abandon(caller, selector, false)
                    .unwrap_err()
                    .to_string()
                    .contains("GitHub unavailable"));
                assert_eq!(
                    runtime
                        .block_on(async { state.lock().await.issues[0]["state"]["type"].clone() }),
                    json!("canceled")
                );
                assert!(checkout.exists());
                std::fs::remove_file(repo.join(".git/fail-close")).unwrap();
            }
            assert_eq!(
                if delete {
                    crate::ops::task::task_delete(caller, "FIX-1").unwrap()
                } else {
                    crate::ops::task::task_abandon(caller, selector, false).unwrap()
                },
                "FIX-1"
            );
            assert!(!checkout.exists());
            assert!(git(
                &repo,
                &["ls-remote", "--heads", "origin", "refs/heads/cancel-me"]
            )
            .is_empty());
            assert!(git(&repo, &["branch", "--list", "cancel-me"]).is_empty());
            assert!(repo.join(".git/pr-closed").exists());
            assert!(runtime
                .block_on(fixture.store.task_flow(&task.id))
                .unwrap()
                .is_none());
            assert_eq!(
                runtime
                    .block_on(
                        fixture
                            .store
                            .work_status(&crate::durable::WorkRef::Task(task.id.clone()))
                    )
                    .unwrap(),
                WorkStatus::Abandoned
            );
            assert_eq!(
                runtime.block_on(fixture.store.task_prs(&task.id)).unwrap()[0].phase(),
                PrPhase::Abandoned
            );
            assert_eq!(
                runtime.block_on(async { state.lock().await.trashed }),
                delete
            );
            // Both retry paths survive deletion of the checkout and refs.
            if delete {
                crate::ops::task::task_delete(caller, "FIX-1").unwrap();
            } else {
                crate::ops::task::task_abandon(&repo, Some("cancel-me"), false).unwrap();
            }
        });
        server.abort();
        std::env::set_var("PATH", path);
    }
}

#[test]
fn foreign_projects_do_not_block_sweep_refresh_or_sync() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    let (repo, wave) = runtime.block_on(fixture.planning_repo());
    runtime.block_on(fixture.seed(now() + 86_400));
    let foreign = json!({
        "id":"foreign-project", "name":"Other Repository — Technical Architecture",
        "description":"", "content":"", "status":{"type":"started"},
        "initiatives":{"nodes":[{"id":"initiative-1"},{"id":"other-initiative"}]},
        "teams":{"nodes":[{"id":"other-team"}]}
    });
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState {
        extra_projects: vec![foreign.clone()],
        ..PlanningState::default()
    }));
    let (url, server) = runtime.block_on(serve(state.clone()));
    PM_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        // Creation refreshes planning and must still find the repository chapter.
        crate::ops::task::task_create(
            &repo,
            Some("product"),
            Some("Eligible work".into()),
            Some("Cancel only repository work".into()),
            None,
        )
        .unwrap();
        for plan in [true, false] {
            let result = super::pm_sync(
                &repo,
                &super::PmSyncOptions {
                    wave: Some("product".into()),
                    plan,
                },
                &NullProgress,
            )
            .unwrap();
            assert_eq!(
                result
                    .diagnostics
                    .iter()
                    .filter(|message| message.contains("foreign-project"))
                    .count(),
                1
            );
            assert!(!result
                .actions
                .iter()
                .any(|action| action.contains("Technical Architecture")));
        }
        runtime.block_on(async {
            let provider = state.lock().await;
            assert_eq!(provider.extra_projects, vec![foreign.clone()]);
            assert_eq!(provider.project_name.as_deref(), Some("Product — Chapter"));
            let row = fixture.store.pm_snapshot(wave.id()).await.unwrap().unwrap();
            let snapshot = row.snapshot;
            assert_eq!(snapshot.projects.len(), 1);
            assert_eq!(snapshot.projects[0].id, "project-1");
        });
        runtime.block_on(async {
            let mut provider = state.lock().await;
            provider.current_project_id = Some("prior-project".into());
            provider.issues[0]["project"]["id"] = json!("prior-project");
            mark_issue_updated(&mut provider.issues[0]);
            // Duplicate foreign membership still yields one preview entry.
            provider.project_name = None;
            provider.extra_projects.push(foreign.clone());
        });
        let preview = crate::ops::task::task_sweep(&repo, false).unwrap();
        assert_eq!(preview.len(), 2);
        assert_eq!(preview[0].issue, None);
        assert_eq!(preview[0].project, foreign["name"].as_str().unwrap());
        assert!(preview[0].outcome.contains("foreign-project"));
        assert_eq!(
            serde_json::to_value(&preview).unwrap()[0]["issue"],
            json!(null)
        );
        assert_eq!(preview[1].issue.as_deref(), Some("FIX-1"));
        assert!(preview[1].outcome.starts_with("would cancel"));
        assert_eq!(
            runtime.block_on(async { state.lock().await.issues[0]["state"]["type"].clone() }),
            json!("unstarted")
        );
        let applied = crate::ops::task::task_sweep(&repo, true).unwrap();
        assert_eq!(applied.len(), 2);
        assert_eq!(applied[1].outcome, "canceled");
        assert_eq!(
            runtime.block_on(async { state.lock().await.issues[0]["state"]["type"].clone() }),
            json!("canceled")
        );
        let repeated = crate::ops::task::task_sweep(&repo, true).unwrap();
        assert_eq!(repeated.len(), 1);
        assert_eq!(repeated[0].issue, None);
        // Missing or shared ownership must not silently disappear from reads.
        for teams in [json!([]), json!([{"id":"team-1"}, {"id":"other-team"}])] {
            runtime.block_on(async {
                let mut provider = state.lock().await;
                let mut ambiguous = foreign.clone();
                ambiguous["initiatives"]["nodes"] = json!([{"id":"initiative-1"}]);
                ambiguous["teams"]["nodes"] = teams;
                provider.extra_projects = vec![ambiguous];
            });
            let error = crate::ops::task::task_sweep(&repo, false)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("expected exactly one repository Team"),
                "{error}"
            );
        }
    });
    server.abort();
}

#[test]
fn task_sweep_previews_old_chapters_and_preserves_current_and_terminal_issues() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    let (repo, _wave) = runtime.block_on(fixture.planning_repo());
    runtime.block_on(fixture.seed(now() + 86_400));
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState::default()));
    let (url, server) = runtime.block_on(serve(state.clone()));
    PM_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        crate::ops::task::task_create(
            &repo,
            Some("product"),
            Some("Planning work".into()),
            Some("Retain evidence".into()),
            None,
        )
        .unwrap();
        assert!(crate::ops::task::task_sweep(&repo, true)
            .unwrap()
            .is_empty());
        runtime.block_on(async {
            let mut state = state.lock().await;
            state.current_project_id = Some("prior-project".into());
            state.issues[0]["project"]["id"] = json!("prior-project");
            mark_issue_updated(&mut state.issues[0]);
        });
        let bin = fixture.directory.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let gh = bin.join("gh");
        std::fs::write(&gh, "#!/bin/sh\necho '{\"state\":\"OPEN\"}'\n").unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::env::var_os("PATH").unwrap();
        std::env::set_var(
            "PATH",
            std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path))).unwrap(),
        );
        runtime.block_on(async {
            state.lock().await.attachments =
                vec!["https://github.com/loopflowstudio/fixture/pull/99".into()];
        });
        let excluded = crate::ops::task::task_sweep(&repo, true).unwrap();
        assert!(excluded[0].outcome.contains("untracked and OPEN"));
        assert_eq!(
            runtime.block_on(async { state.lock().await.issues[0]["state"]["type"].clone() }),
            json!("unstarted")
        );
        runtime.block_on(async {
            state.lock().await.attachments.clear();
        });
        std::env::set_var("PATH", path);
        let preview = crate::ops::task::task_sweep(&repo, false).unwrap();
        assert_eq!(preview.len(), 1);
        assert!(preview[0].outcome.starts_with("would cancel"));
        assert_eq!(
            runtime.block_on(async { state.lock().await.issues[0]["state"]["type"].clone() }),
            json!("unstarted")
        );
        for terminal in ["completed", "canceled", "duplicate"] {
            runtime.block_on(async {
                state.lock().await.issues[0]["state"]["type"] = json!(terminal);
                mark_issue_updated(&mut state.lock().await.issues[0]);
            });
            assert!(crate::ops::task::task_sweep(&repo, true)
                .unwrap()
                .is_empty());
        }
        runtime.block_on(async {
            state.lock().await.issues[0]["state"]["type"] = json!("unstarted");
            mark_issue_updated(&mut state.lock().await.issues[0]);
        });
        // Membership changes after candidate enumeration still prevent writes.
        runtime.block_on(async {
            state.lock().await.move_on_attachment_read = true;
        });
        let moved = crate::ops::task::task_sweep(&repo, true).unwrap();
        assert!(moved[0].outcome.contains("current chapter"));
        assert_eq!(
            runtime.block_on(async { state.lock().await.issues[0]["state"]["type"].clone() }),
            json!("unstarted")
        );
        runtime.block_on(async {
            let mut state = state.lock().await;
            state.current_project_id = Some("prior-project".into());
            state.issues[0]["project"]["id"] = json!("prior-project");
            mark_issue_updated(&mut state.issues[0]);
        });
        let applied = crate::ops::task::task_sweep(&repo, true).unwrap();
        assert_eq!(applied[0].outcome, "canceled");
        assert!(!runtime.block_on(async { state.lock().await.trashed }));
        assert!(crate::ops::task::task_sweep(&repo, true)
            .unwrap()
            .is_empty());
    });
    server.abort();
}

#[derive(Default)]
struct LifecycleMessages(RefCell<Vec<String>>);

impl crate::ops::Progress for LifecycleMessages {
    fn status(&self, message: &str) {
        self.0.borrow_mut().push(message.to_string());
    }
    fn error(&self, message: &str) {
        self.status(message);
    }
    fn confirm(&self, _: &str) -> bool {
        true
    }
}
