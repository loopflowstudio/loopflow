use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::{extract::State, Json};
use serde_json::{json, Value};

use super::{task_follow_up, FollowUpOptions};
use crate::durable::TaskState;
use crate::id::WaveId;
use crate::ops::pm::{PmTestContext, PM_TEST_CONTEXT};
use crate::ops::task::task_completion_gate;
use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
use crate::store::{open_ephemeral_store, CredentialType, ProviderToken, StorageConfig};
use crate::work::project::{Project, ProjectId};
use crate::work::task::{
    GithubPr, Observation, PmWritebackState, PrPublication, Task, TaskId, TaskPr, TaskPrId,
};
use crate::work::wave::Wave;

#[derive(Default)]
struct Linear {
    issues: BTreeMap<String, Value>,
    relations: BTreeMap<String, Value>,
    unavailable: bool,
    relations_unavailable: bool,
    lose_responses: bool,
}

async fn respond(
    State(state): State<Arc<Mutex<Linear>>>,
    Json(request): Json<Value>,
) -> Json<Value> {
    let mut linear = state.lock().unwrap();
    let query = request["query"].as_str().unwrap();
    let vars = &request["variables"];
    if linear.unavailable {
        return Json(json!({"errors": [{"message": "provider unavailable"}]}));
    }
    let data = if query.contains("ListTeams") {
        json!({"teams": {"nodes": [{"id": "team-1", "name": "Fixture", "key": "FIX",
            "description": "<!-- loopflow-repository: loopflowstudio/fixture -->"}]}})
    } else if query.contains("IssueComments") {
        json!({"issue": {"comments": {"nodes": [], "pageInfo": {"hasNextPage": false, "endCursor": null}}}})
    } else if query.contains("workflowStates") {
        json!({"workflowStates": {"nodes": [{"id": "todo", "position": 1.0}]}})
    } else if query.contains("FindExportIssue") {
        let id = vars["id"].as_str().unwrap();
        json!({"issues": {"nodes": if linear.issues.contains_key(id) { vec![json!({"id": id})] } else { vec![] }}})
    } else if query.contains("DeliverTaskCreation") || query.contains("mutation FollowUpRelation") {
        let input = vars["input"].clone();
        let id = input["id"].as_str().unwrap().to_string();
        let rows = if query.contains("DeliverTaskCreation") {
            &mut linear.issues
        } else {
            &mut linear.relations
        };
        if rows.contains_key(&id) {
            return Json(json!({"errors": [{"message": "ID already exists"}]}));
        }
        rows.insert(id, input.clone());
        // Commit remotely, then lose both the response and subsequent readback.
        // Recovery therefore requires another operation invocation and its receipt.
        if linear.lose_responses {
            linear.unavailable = true;
            return Json(json!({"errors": [{"message": "response lost after commit"}]}));
        }
        if query.contains("DeliverTaskCreation") {
            json!({"issueCreate": {"success": true, "issue": {"id": input["id"]}}})
        } else {
            json!({"issueRelationCreate": {"success": true}})
        }
    } else if query.contains("FollowUpRelationExists") {
        if linear.relations_unavailable {
            return Json(json!({"errors": [{"message": "relation read unavailable"}]}));
        }
        let nodes: Vec<_> = linear.relations.values().filter_map(|relation| {
            if relation["issueId"] == vars["id"] {
                Some(json!({"type": "related", "relatedIssue": {"id": relation["relatedIssueId"]}}))
            } else if relation["relatedIssueId"] == vars["id"] {
                Some(json!({"type": "related", "relatedIssue": {"id": relation["issueId"]}}))
            } else { None }
        }).collect();
        json!({"issue": {"relations": {"nodes": nodes, "pageInfo": {"hasNextPage": false, "endCursor": null}}}})
    } else if query.contains("IssueOwnership") {
        let id = vars["id"].as_str().unwrap();
        let source = id == "source-issue" || id == "FIX-1";
        let source_issue = json!({"title": "Deliver the command", "description": "", "teamId": "team-1", "projectId": "project-1", "dueDate": null});
        let issue = if source {
            &source_issue
        } else {
            &linear.issues[id]
        };
        json!({"issue": {"id": if source { "source-issue" } else { id }, "identifier": if source { "FIX-1" } else { "FIX-2" }, "url": "https://linear.app/fixture/issue/FIX-2",
            "title": issue["title"], "description": issue["description"], "dueDate": issue["dueDate"],
            "updatedAt": "2026-10-07T00:00:00Z", "completedAt": null, "branchName": null,
            "prioritySortOrder": 1.0, "sortOrder": 1.0, "assignee": null, "state": {"type": "unstarted"},
            "team": {"id": issue["teamId"]}, "project": {"id": issue["projectId"],
                "name": "Provider destination", "description": "", "content": "workflow: feature",
                "status": {"type": "started"}, "updatedAt": "2026-10-07T00:00:00Z", "archivedAt": null,
                "initiatives": {"nodes": [{"id": "initiative-1"}]}, "teams": {"nodes": [{"id": issue["teamId"]}]}}}})
    } else {
        panic!("unexpected fixture query: {query}");
    };
    Json(json!({"data": data}))
}

#[test]
fn local_follow_up_retries_keep_identity_and_allow_independent_completion() {
    let _lock = crate::journal::test_env_lock();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (directory, store, repo, task, _) = fixture(&runtime);
    let mut pr = runtime
        .block_on(store.active_task_pr(&task.id))
        .unwrap()
        .unwrap();
    pr.merge_commit = Some(repo.head_sha());
    runtime.block_on(store.update_task_pr(&pr)).unwrap();
    PM_TEST_CONTEXT.sync_scope(
        PmTestContext {
            path: directory.path().join("loopflow.db"),
            store: store.clone(),
            graphql_url: "http://127.0.0.1:1".into(),
        },
        || {
            let options = FollowUpOptions {
                key: Some("installed".into()),
                title: Some("Verify installed release".into()),
                notes: Some("Run the released command and retain its result".into()),
                due: Some("2026-10-09".into()),
                ..Default::default()
            };
            task_follow_up(repo.path(), "FIX-1", &options).unwrap();
            let first = store.sqlite.task_follow_through(&task.id).unwrap();
            let child = runtime
                .block_on(store.get_task_by_issue(&first.intents[0].issue_id))
                .unwrap()
                .unwrap();
            assert!(child.worktree.is_none());
            assert!(child.plan.linear_id.is_none());
            assert!(runtime
                .block_on(store.task_prs(&child.id))
                .unwrap()
                .is_empty());
            assert_eq!(
                super::super::task_planning_item(&store, &child)
                    .unwrap()
                    .due_date
                    .as_deref(),
                Some("2026-10-09")
            );
            // Retry arguments cannot change the pinned destination, identity or payload.
            task_follow_up(
                repo.path(),
                "FIX-1",
                &FollowUpOptions {
                    title: Some("Changed retry".into()),
                    wave: Some("missing-wave".into()),
                    ..options
                },
            )
            .unwrap();
            assert_eq!(store.sqlite.task_follow_through(&task.id).unwrap(), first);
            assert_eq!(runtime.block_on(store.list_tasks(None)).unwrap().len(), 2);
            assert!(!runtime
                .block_on(task_completion_gate(&store, &task))
                .unwrap()
                .satisfied());
            task_follow_up(
                repo.path(),
                "FIX-1",
                &FollowUpOptions {
                    finish: Some("Installed proof belongs to the follow-up".into()),
                    ..Default::default()
                },
            )
            .unwrap();
            let request = store
                .sqlite
                .request_task_completion(&task.id, None)
                .unwrap()
                .unwrap();
            runtime
                .block_on(store.complete_task(&task, request))
                .unwrap();
            assert_eq!(store.sqlite.task_state(&task.id).unwrap(), TaskState::Done);
            assert_eq!(store.sqlite.workflow(&task.id).unwrap(), None);
            assert_ne!(store.sqlite.task_state(&child.id).unwrap(), TaskState::Done);
            let events = runtime
                .block_on(store.task_events_after(&task.id, 0))
                .unwrap();
            assert!(runtime
                .block_on(store.complete_task(&task, request))
                .unwrap());
            assert_eq!(
                runtime
                    .block_on(store.task_events_after(&task.id, 0))
                    .unwrap(),
                events
            );
            assert!(store
                .sqlite
                .follow_up_sources()
                .unwrap()
                .contains_key(child.id.as_str()));
        },
    );
}

#[test]
fn follow_up_export_recovers_lost_issue_and_relation_responses_after_completion() {
    let _lock = crate::journal::test_env_lock();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (directory, store, repo, task, wave) = fixture(&runtime);
    let _environment = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_DATABASE"]);
    std::env::set_var("LF_HOME", directory.path());
    let mut pr = runtime
        .block_on(store.active_task_pr(&task.id))
        .unwrap()
        .unwrap();
    pr.merge_commit = Some(repo.head_sha());
    runtime.block_on(store.update_task_pr(&pr)).unwrap();
    let linear = Arc::new(Mutex::new(Linear {
        lose_responses: true,
        ..Default::default()
    }));
    let (url, server) = runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let app = axum::Router::new()
            .route("/", axum::routing::post(respond))
            .with_state(linear.clone());
        (
            url,
            tokio::spawn(async move { axum::serve(listener, app).await.unwrap() }),
        )
    });
    PM_TEST_CONTEXT.sync_scope(
        PmTestContext {
            path: directory.path().join("loopflow.db"),
            store: store.clone(),
            graphql_url: url,
        },
        || {
            task_follow_up(
                repo.path(),
                "FIX-1",
                &FollowUpOptions {
                    title: Some("Check the installed release".into()),
                    notes: Some("Retain the released command result".into()),
                    due: Some("2026-10-09".into()),
                    ..Default::default()
                },
            )
            .unwrap();
            task_follow_up(
                repo.path(),
                "FIX-1",
                &FollowUpOptions {
                    finish: Some("The child owns installed proof".into()),
                    ..Default::default()
                },
            )
            .unwrap();
            let request = store
                .sqlite
                .request_task_completion(&task.id, None)
                .unwrap()
                .unwrap();
            runtime
                .block_on(store.complete_task(&task, request))
                .unwrap();
            let intent = store.sqlite.task_follow_through(&task.id).unwrap().intents[0].clone();
            let child = runtime
                .block_on(store.get_task_by_issue(&intent.issue_id))
                .unwrap()
                .unwrap();
            assert!(linear.lock().unwrap().issues.is_empty());
            let sync = || {
                runtime
                    .block_on(crate::ops::planning_export::sync_repository_exports(
                        &store,
                        repo.path().to_str().unwrap(),
                    ))
                    .unwrap()
            };
            // Remote creation commits, but both the reply and readback are lost.
            let lost_creation = runtime
                .block_on(crate::ops::planning_export::sync_export(
                    &store,
                    repo.path(),
                    &crate::durable::WorkRef::Task(child.id.clone()),
                ))
                .unwrap_err();
            assert_eq!(linear.lock().unwrap().issues.len(), 1, "{lost_creation}");
            assert!(runtime
                .block_on(store.get_task(&child.id))
                .unwrap()
                .unwrap()
                .plan
                .linear_id
                .is_none());
            {
                let mut provider = linear.lock().unwrap();
                provider.unavailable = false;
                provider.lose_responses = false;
                provider.relations_unavailable = true;
                let issue = provider.issues.values_mut().next().unwrap();
                issue["dueDate"] = Value::Null;
            }
            // Reconnect adopts the exact issue, including a removed due date. Failed
            // relation reads neither undo completion nor claim provider linkage.
            sync();
            let mapped = runtime
                .block_on(store.get_task(&child.id))
                .unwrap()
                .unwrap();
            assert!(mapped.plan.linear_id.is_some());
            let sources = store.sqlite.follow_up_sources().unwrap();
            let local_sources = &sources[child.id.as_str()];
            assert_eq!(local_sources.len(), 1);
            assert_eq!(local_sources[0].identifier, "FIX-1");
            assert_eq!(
                local_sources,
                &sources[mapped.plan.linear_id.as_ref().unwrap().as_str()]
            );
            let follow = store.sqlite.task_follow_through(&task.id).unwrap();
            assert!(follow.resolved());
            assert_eq!(follow.links[0].identifier, "FIX-2");
            assert!(follow.links[0].url.is_some());
            assert_eq!(follow.links[0].due, None);
            assert_eq!(follow.intents[0].due.as_deref(), Some("2026-10-09"));
            assert_eq!(
                store
                    .sqlite
                    .pending_follow_through_relations(wave.repo())
                    .unwrap()
                    .len(),
                1
            );
            {
                let mut provider = linear.lock().unwrap();
                provider.relations_unavailable = false;
                provider.lose_responses = true;
            }
            sync();
            assert_eq!(linear.lock().unwrap().relations.len(), 1);
            assert_eq!(
                store
                    .sqlite
                    .pending_follow_through_relations(wave.repo())
                    .unwrap()
                    .len(),
                1
            );
            {
                let mut provider = linear.lock().unwrap();
                provider.unavailable = false;
                provider.lose_responses = false;
            }
            sync();
            assert!(store
                .sqlite
                .pending_follow_through_relations(wave.repo())
                .unwrap()
                .is_empty());
            let events = runtime
                .block_on(store.task_events_after(&task.id, 0))
                .unwrap();
            sync();
            assert_eq!(
                runtime
                    .block_on(store.task_events_after(&task.id, 0))
                    .unwrap(),
                events
            );
            let provider = linear.lock().unwrap();
            assert_eq!(provider.issues.len(), 1);
            assert_eq!(provider.relations.len(), 1);
            assert_eq!(
                provider.relations[&intent.relation_id]["issueId"],
                "source-issue"
            );
            assert_eq!(
                provider.relations[&intent.relation_id]["relatedIssueId"],
                mapped.plan.linear_id.unwrap().as_str()
            );
            assert_eq!(store.sqlite.task_state(&task.id).unwrap(), TaskState::Done);
            assert_ne!(store.sqlite.task_state(&child.id).unwrap(), TaskState::Done);
        },
    );
    server.abort();
}

mod lifecycle;

fn fixture(
    runtime: &tokio::runtime::Runtime,
) -> (
    tempfile::TempDir,
    crate::store::SharedStore,
    loopflow_test_support::TestRepo,
    Task,
    Wave,
) {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("loopflow.db");
    let store = Arc::new(
        runtime
            .block_on(open_ephemeral_store(&StorageConfig::sqlite(
                database.clone(),
            )))
            .unwrap(),
    );
    let repo = loopflow_test_support::TestRepo::new();
    assert!(std::process::Command::new("git")
        .args([
            "remote",
            "set-url",
            "origin",
            "https://github.com/loopflowstudio/fixture.git"
        ])
        .current_dir(repo.path())
        .status()
        .unwrap()
        .success());
    std::fs::create_dir_all(repo.path().join(".lf")).unwrap();
    std::fs::write(
        repo.path().join(".lf/config.yaml"),
        "pm:\n  provider: linear\n  linear_team: team-1\n",
    )
    .unwrap();
    std::fs::create_dir_all(repo.path().join("wave/product")).unwrap();
    std::fs::write(
        repo.path().join("wave/product/GOAL.md"),
        "---\npm:\n  linear_initiative: initiative-1\n---\nProduct\n",
    )
    .unwrap();
    repo.create_file(
        ".lf/workflows/delivery.yaml",
        "edges:\n  - {from: start, to: end, flow: delivery}\n",
    );
    let now = time::OffsetDateTime::now_utc();
    let wave = Wave::new(
        WaveId::new(),
        "product".into(),
        repo.path().canonicalize().unwrap().display().to_string(),
    );
    let project = Project {
        id: ProjectId::new(),
        plan: ProjectPlan {
            linear_id: Some(LinearProjectId::new("project-1").unwrap()),
            summary: String::new(),
            slug: "chapter".into(),
            name: "Chapter".into(),
            workflow: "feature".into(),
            status: crate::pm::ProjectStatus::Started,
            prompt_context: String::new(),
            pm_snapshot_synced_at: Some(now.unix_timestamp()),
        },
        wave_id: wave.id().clone(),
        iteration: 0,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    let task = Task {
        id: TaskId::new(),
        plan: TaskPlan {
            linear_id: Some(LinearIssueId::new("source-issue").unwrap()),
            revision: 0,
            identifier: "FIX-1".into(),
            title: "Deliver the command".into(),
            description: String::new(),
            pm_snapshot_synced_at: Some(now.unix_timestamp()),
        },
        pm_writeback: PmWritebackState::Current,
        wave_id: wave.id().clone(),
        project_id: project.id.clone(),
        worktree: Some(repo.path().canonicalize().unwrap()),
        workspace_slug: "delivery".into(),
        branch: "main".into(),
        base_commit: repo.head_sha(),
        parent_pr_id: None,
        agent: None,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
        observation: Observation::NotRequired,
    };
    let pr = TaskPr {
        id: TaskPrId::new(),
        task_id: task.id.clone(),
        sequence: 1,
        slug: task.workspace_slug.clone(),
        branch: task.branch.clone(),
        base_commit: task.base_commit.clone(),
        parent_pr_id: None,
        publication: Some(PrPublication {
            requested_at: now,
            presentation: None,
            merge: None,
            github: Some(GithubPr {
                number: 1,
                url: "https://github.com/loopflowstudio/fixture/pull/1".into(),
                head_sha: Some(repo.head_sha()),
            }),
        }),
        merge_commit: None,
        abandoned_at: None,
        ci_observation: None,
        github_observation: None,
        linear_attachment_id: None,
        linear_comment_id: None,
        linear_link_error: None,
        created_at: now,
        updated_at: now,
    };
    runtime.block_on(async {
        store.create_wave(&wave).await.unwrap();
        store.create_project(&project).await.unwrap();
        crate::store::sqlite::project_selection::write_project_binding(
            &store.sqlite,
            wave.id(),
            None,
            project.plan.linear_id.as_ref().unwrap().as_str(),
            &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
        )
        .unwrap();
        store.seed_task(&task, &pr).await.unwrap();
        store
            .upsert_provider_token(&ProviderToken {
                provider: "linear".into(),
                access_token: "fixture-token".into(),
                refresh_token: None,
                oauth_client_id: None,
                expires_at: Some(now.unix_timestamp() + 3600),
                login: None,
                updated_at: now.unix_timestamp(),
                credential_type: CredentialType::OAuth,
            })
            .await
            .unwrap();
    });
    (directory, store, repo, task, wave)
}

#[test]
fn filing_is_atomic_and_reserved_recovery_keeps_the_original_chapter() {
    let _lock = crate::journal::test_env_lock();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (directory, store, _repo, task, wave) = fixture(&runtime);
    let intent = crate::work::task::follow_through::FollowThroughIntent {
        key: "installed".into(),
        issue_id: TaskId::new().to_string(),
        relation_id: uuid::Uuid::new_v4().to_string(),
        project_id: task.project_id.to_string(),
        team_id: String::new(),
        state_id: None,
        wave: "product".into(),
        title: "Verify installed release".into(),
        notes: "Retain the command result".into(),
        due: Some("2026-10-09".into()),
        existing: false,
    };
    let conn = rusqlite::Connection::open(directory.path().join("loopflow.db")).unwrap();
    conn.execute_batch("CREATE TRIGGER refuse_filing BEFORE INSERT ON task_events WHEN json_extract(NEW.kind_json,'$.kind')='follow_through_intent' BEGIN SELECT RAISE(ABORT,'unavailable'); END").unwrap();
    assert!(store
        .sqlite
        .reserve_follow_through(&task.id, &intent)
        .is_err());
    assert!(runtime
        .block_on(store.get_task_by_issue(&intent.issue_id))
        .unwrap()
        .is_none());
    assert!(store
        .sqlite
        .task_follow_through(&task.id)
        .unwrap()
        .intents
        .is_empty());
    conn.execute_batch("DROP TRIGGER refuse_filing").unwrap();
    // Simulate the previous version's crash after reservation, before creation.
    runtime
        .block_on(store.append_task_event(
            &task.id,
            &crate::work::task::TaskEventKind::FollowThroughIntent {
                intent: intent.clone(),
            },
        ))
        .unwrap();
    let mut next = runtime
        .block_on(store.get_project(&task.project_id))
        .unwrap()
        .unwrap();
    next.id = ProjectId::new();
    next.plan.linear_id = None;
    next.plan.slug = "next".into();
    runtime.block_on(store.create_project(&next)).unwrap();
    crate::store::sqlite::project_selection::write_project_binding(
        &store.sqlite,
        wave.id(),
        Some("project-1"),
        next.id.as_str(),
        &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
    )
    .unwrap();
    let mut retry = intent.clone();
    retry.project_id = next.id.to_string();
    retry.issue_id = TaskId::new().to_string();
    retry.title = "Changed retry".into();
    for _ in 0..2 {
        assert_eq!(
            store
                .sqlite
                .reserve_follow_through(&task.id, &retry)
                .unwrap(),
            intent
        );
    }
    let child = runtime
        .block_on(store.get_task_by_issue(&intent.issue_id))
        .unwrap()
        .unwrap();
    assert_eq!(child.project_id, task.project_id);
    assert_eq!(child.plan.title, intent.title);
    assert_eq!(child.plan.description, intent.notes);
    assert_eq!(
        super::super::task_planning_item(&store, &child)
            .unwrap()
            .due_date,
        intent.due
    );
    assert_eq!(runtime.block_on(store.list_tasks(None)).unwrap().len(), 2);
    let mut unrelated = intent.clone();
    unrelated.key = "another".into();
    unrelated.issue_id = TaskId::new().to_string();
    assert!(store
        .sqlite
        .reserve_follow_through(&task.id, &unrelated)
        .is_err());
    assert!(store
        .sqlite
        .create_task(&crate::planning::NewTask {
            id: TaskId::new(),
            project_id: task.project_id,
            title: "Old backlog".into(),
            description: String::new(),
            due_date: None,
        })
        .is_err());
}

#[test]
fn historical_filing_converts_to_one_local_child_and_preserves_export_uncertainty() {
    let _lock = crate::journal::test_env_lock();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for created_remotely in [false, true] {
        let (directory, store, repo, task, wave) = fixture(&runtime);
        let _environment = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_DATABASE"]);
        std::env::set_var("LF_HOME", directory.path());
        let intent = crate::work::task::follow_through::FollowThroughIntent {
            key: "installed".into(),
            issue_id: uuid::Uuid::new_v4().to_string(),
            relation_id: uuid::Uuid::new_v4().to_string(),
            project_id: "project-1".into(),
            team_id: "original-team".into(),
            state_id: Some("original-state".into()),
            wave: "product".into(),
            title: "Original title".into(),
            notes: "Original evidence condition".into(),
            due: Some("2026-10-09".into()),
            existing: false,
        };
        runtime
            .block_on(store.append_task_event(
                &task.id,
                &crate::work::task::TaskEventKind::FollowThroughIntent {
                    intent: intent.clone(),
                },
            ))
            .unwrap();
        let mut next = runtime
            .block_on(store.get_project(&task.project_id))
            .unwrap()
            .unwrap();
        next.id = ProjectId::new();
        next.plan.linear_id = None;
        next.plan.slug = "next".into();
        runtime.block_on(store.create_project(&next)).unwrap();
        crate::store::sqlite::project_selection::write_project_binding(
            &store.sqlite,
            wave.id(),
            Some("project-1"),
            next.id.as_str(),
            &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
        )
        .unwrap();
        let linear = Arc::new(Mutex::new(Linear::default()));
        if created_remotely {
            linear.lock().unwrap().issues.insert(
                intent.issue_id.clone(),
                json!({"id":intent.issue_id,
                "title":intent.title,"description":intent.notes,"dueDate":intent.due,
                "teamId":"team-1","projectId":"project-1"}),
            );
        }
        let (url, server) = runtime.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let app = axum::Router::new()
                .route("/", axum::routing::post(respond))
                .with_state(linear.clone());
            (
                url,
                tokio::spawn(async move { axum::serve(listener, app).await.unwrap() }),
            )
        });
        PM_TEST_CONTEXT.sync_scope(
            PmTestContext {
                path: directory.path().join("loopflow.db"),
                store: store.clone(),
                graphql_url: url,
            },
            || {
                for _ in 0..2 {
                    store
                        .sqlite
                        .reserve_follow_through(&task.id, &intent)
                        .unwrap();
                }
                let child = runtime
                    .block_on(store.get_task_by_issue(&intent.issue_id))
                    .unwrap()
                    .unwrap();
                assert_eq!(child.project_id, task.project_id);
                runtime
                    .block_on(super::link_intent(&store, repo.path(), &task.id, &intent))
                    .unwrap();
                assert!(store
                    .sqlite
                    .follow_up_sources()
                    .unwrap()
                    .contains_key(child.id.as_str()));
                let export = store
                    .sqlite
                    .prepare_planning_export(
                        crate::store::sqlite::planning_changes::PlanningChanges::Task(&child.id),
                        "changed-team",
                        "initiative-1",
                    )
                    .unwrap();
                assert_eq!(export.id, intent.issue_id);
                assert_eq!(export.input["teamId"], "original-team");
                assert_eq!(export.input["stateId"], "original-state");
                assert_eq!(export.input["dueDate"], "2026-10-09");
                assert_eq!(export.input["description"], intent.notes);
                let result = runtime.block_on(crate::ops::planning_export::sync_export(
                    &store,
                    repo.path(),
                    &crate::durable::WorkRef::Task(child.id.clone()),
                ));
                if created_remotely {
                    result.unwrap();
                } else {
                    assert!(result.unwrap_err().to_string().contains("uncertain"));
                }
                let retained = runtime
                    .block_on(store.get_task_by_issue(&intent.issue_id))
                    .unwrap()
                    .unwrap();
                assert_eq!(retained.id, child.id);
                assert_eq!(retained.plan.linear_id.is_some(), created_remotely);
                assert_eq!(
                    linear.lock().unwrap().issues.len(),
                    usize::from(created_remotely)
                );
                assert_eq!(runtime.block_on(store.list_tasks(None)).unwrap().len(), 2);
                assert_eq!(
                    store
                        .sqlite
                        .task_follow_through(&task.id)
                        .unwrap()
                        .intents
                        .as_slice(),
                    std::slice::from_ref(&intent)
                );
            },
        );
        server.abort();
    }
}
