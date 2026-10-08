use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::{extract::State, routing::post, Json, Router};
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
    completed: bool,
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
    } else if query.contains("IssueTeam") {
        json!({"issue": {"team": {"id": "team-1"}}})
    } else if query.contains("CompletedWorkflowStates") {
        json!({"workflowStates": {"nodes": [{"id": "completed"}]}})
    } else if query.contains("SetIssueState") {
        linear.completed = true;
        json!({"issueUpdate": {"issue": {"id": "source-issue"}}})
    } else if query.contains("IssueComments") {
        json!({"issue": {"comments": {"nodes": [], "pageInfo": {"hasNextPage": false, "endCursor": null}}}})
    } else if query.contains("CreateComment") {
        json!({"commentCreate": {"comment": {"id": "completion-comment"}}})
    } else if query.contains("workflowStates") {
        json!({"workflowStates": {"nodes": [{"id": "todo", "position": 1.0}]}})
    } else if query.contains("FollowUpExists") {
        let id = vars["id"].as_str().unwrap();
        json!({"issues": {"nodes": if linear.issues.contains_key(id) { vec![json!({"id": id})] } else { vec![] }}})
    } else if query.contains("FollowUpCreate") || query.contains("mutation FollowUpRelation") {
        let input = vars["input"].clone();
        let id = input["id"].as_str().unwrap().to_string();
        let rows = if query.contains("FollowUpCreate") {
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
        if query.contains("FollowUpCreate") {
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
        let completed = source && linear.completed;
        json!({"issue": {"id": if source { "source-issue" } else { id }, "identifier": if source { "FIX-1" } else { "FIX-2" }, "url": "https://linear.app/fixture/issue/FIX-2",
            "title": issue["title"], "description": issue["description"], "dueDate": issue["dueDate"],
            "updatedAt": if completed { "2026-10-08T00:00:00Z" } else { "2026-10-07T00:00:00Z" }, "completedAt": if completed { json!("2026-10-08T00:00:00Z") } else { Value::Null }, "branchName": null,
            "prioritySortOrder": 1.0, "sortOrder": 1.0, "assignee": null, "state": {"type": if completed { "completed" } else { "unstarted" }},
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
fn operation_retries_pinned_filing_after_lost_responses_and_chapter_change() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (directory, store, repo, task, wave) = fixture(&runtime);
    let database = directory.path().join("loopflow.db");
    let now = time::OffsetDateTime::now_utc();
    let mut pr = runtime
        .block_on(store.active_task_pr(&task.id))
        .unwrap()
        .unwrap();
    pr.merge_commit = Some(repo.head_sha());
    runtime.block_on(store.update_task_pr(&pr)).unwrap();
    let select_chapter = |id: &str, expected: Option<&str>| {
        let plan = serde_json::from_value(json!({"id": id, "slug": id, "name": id, "summary": "",
            "metric_targets": [], "workflow": "feature", "status": "started", "krs": [],
            "initiative_ids": ["initiative-1"], "team_ids": ["team-1"]}))
        .unwrap();
        store
            .sqlite
            .put_pm_project(
                wave.id(),
                "linear",
                "initiative-1",
                &plan,
                now.unix_timestamp(),
            )
            .unwrap();
        crate::store::sqlite::project_selection::write_project_binding(
            &store.sqlite,
            wave.id(),
            expected,
            id,
            &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
        )
        .unwrap();
    };
    select_chapter("project-1", Some("project-1"));
    let provider = Arc::new(Mutex::new(Linear {
        lose_responses: true,
        ..Default::default()
    }));
    let (url, server) = runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route("/", post(respond))
            .with_state(provider.clone());
        (
            url,
            tokio::spawn(async move { axum::serve(listener, app).await.unwrap() }),
        )
    });
    PM_TEST_CONTEXT.sync_scope(
        PmTestContext {
            path: database,
            store: store.clone(),
            graphql_url: url,
        },
        || {
            let options = FollowUpOptions {
                key: Some("installed".into()),
                title: Some("Verify installed release".into()),
                notes: Some("Run the released command and retain its result".into()),
                due: Some("2026-10-08".into()),
                ..Default::default()
            };
            let finish = FollowUpOptions {
                finish: Some("Accepted installed check filed".into()),
                ..Default::default()
            };
            let pending = || {
                let receipt = store.sqlite.task_follow_through(&task.id).unwrap();
                assert!(!receipt.resolved());
                assert!(!runtime
                    .block_on(task_completion_gate(&store, &task))
                    .unwrap()
                    .satisfied());
                assert!(runtime
                    .block_on(store.complete_task(&task, store.sqlite.request_task_completion(&task.id, None).unwrap().unwrap_or(0)))
                    .is_err());
                assert_ne!(store.sqlite.task_state(&task.id).unwrap(), TaskState::Done);
                receipt
            };

            let error = task_follow_up(repo.path(), "FIX-1", &options).unwrap_err();
            assert!(error.to_string().contains("remains pending"), "{error}");
            let original = pending().intents.into_iter().next().unwrap();
            assert_eq!(
                uuid::Uuid::parse_str(&original.issue_id)
                    .unwrap()
                    .get_version_num(),
                4
            );
            assert_eq!(
                uuid::Uuid::parse_str(&original.relation_id)
                    .unwrap()
                    .get_version_num(),
                4
            );
            assert_eq!(original.project_id, "project-1");
            assert_eq!(original.team_id, "team-1");
            assert_eq!(original.state_id.as_deref(), Some("todo"));
            assert!(original
                .notes
                .contains("Follow-up to FIX-1 https://github.com/loopflowstudio/fixture/pull/1"));
            assert!(task_follow_up(repo.path(), "FIX-1", &finish).is_err());
            pending();

            select_chapter("project-2", Some("project-1"));
            {
                let mut linear = provider.lock().unwrap();
                assert_eq!(linear.issues.len(), 1);
                assert_eq!(linear.issues[&original.issue_id], json!({
                    "id": original.issue_id, "teamId": original.team_id,
                    "projectId": original.project_id, "stateId": original.state_id,
                    "title": original.title, "description": original.notes, "dueDate": original.due,
                }));
                let issue = linear.issues.get_mut(&original.issue_id).unwrap();
                issue["title"] = json!("Edited after filing");
                issue["projectId"] = json!("moved-project");
                issue["dueDate"] = json!("2026-10-12");
                linear.unavailable = false;
            }
            // Changed arguments and a now-invalid destination must not replace a receipt.
            let retry = FollowUpOptions {
                title: Some("Different title".into()),
                wave: Some("removed-wave".into()),
                due: Some("2026-10-09".into()),
                ..options.clone()
            };
            assert!(task_follow_up(repo.path(), "FIX-1", &retry).is_err());
            let receipt = pending();
            assert_eq!(receipt.intents, std::slice::from_ref(&original));
            assert!(receipt.links.is_empty());
            {
                let mut linear = provider.lock().unwrap();
                assert_eq!(linear.relations.len(), 1);
                assert_eq!(
                    linear.relations[&original.relation_id]["relatedIssueId"],
                    original.issue_id
                );
                assert_eq!(
                    linear.relations[&original.relation_id]["issueId"],
                    "source-issue"
                );
                assert_eq!(linear.relations[&original.relation_id]["type"], "related");
                linear.unavailable = false;
            }
            assert!(task_follow_up(repo.path(), "FIX-1", &retry)
                .unwrap()
                .contains("FIX-2 linked"));
            let linked = pending();
            assert_eq!(linked.intents, std::slice::from_ref(&original));
            assert_eq!(linked.links.len(), 1);
            assert_eq!(linked.links[0].issue_id, original.issue_id);
            assert_eq!(linked.links[0].due.as_deref(), Some("2026-10-12"));
            provider.lock().unwrap().relations_unavailable = true;
            let error = task_follow_up(repo.path(), "FIX-1", &finish).unwrap_err();
            assert!(
                error.to_string().contains("relation read unavailable"),
                "{error}"
            );
            assert_eq!(pending(), linked);
            provider.lock().unwrap().relations_unavailable = false;
            provider.lock().unwrap().issues.get_mut(&original.issue_id).unwrap()["dueDate"] =
                Value::Null;
            task_follow_up(repo.path(), "FIX-1", &finish).unwrap();
            let filed = store.sqlite.task_follow_through(&task.id).unwrap();
            assert!(filed.resolved());
            assert_eq!(filed.intents, std::slice::from_ref(&original));
            assert_eq!(filed.links.len(), 1);
            assert_eq!(filed.links[0].issue_id, original.issue_id);
            assert_eq!(filed.links[0].due, None);
            assert_eq!(filed.reason, finish.finish);
            assert!(runtime
                .block_on(task_completion_gate(&store, &task))
                .unwrap()
                .satisfied());
            assert!(runtime
                .block_on(store.complete_task(&task, store.sqlite.request_task_completion(&task.id, None).unwrap().unwrap_or(0)))
                .unwrap());
            assert_eq!(store.sqlite.task_state(&task.id).unwrap(), TaskState::Done);
            let events = runtime
                .block_on(store.task_events_after(&task.id, 0))
                .unwrap();
            assert!(runtime
                .block_on(store.complete_task(&task, store.sqlite.request_task_completion(&task.id, None).unwrap().unwrap_or(0)))
                .unwrap());
            assert_eq!(
                runtime
                    .block_on(store.task_events_after(&task.id, 0))
                    .unwrap(),
                events
            );
            let linear = provider.lock().unwrap();
            assert_eq!(linear.issues.len(), 1);
            assert_eq!(linear.relations.len(), 1);
            assert_eq!(
                linear.issues[&original.issue_id]["title"],
                "Edited after filing"
            );
            assert_eq!(
                linear.issues[&original.issue_id]["projectId"],
                "moved-project"
            );
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
    let now = time::OffsetDateTime::now_utc();
    let wave = Wave::new(
        WaveId::new(),
        "product".into(),
        repo.path().canonicalize().unwrap().display().to_string(),
    );
    let project = Project {
        id: ProjectId::new(),
        plan: ProjectPlan {
            id: LinearProjectId::new("project-1").unwrap(),
            slug: "chapter".into(),
            name: "Chapter".into(),
            workflow: "feature".into(),
            status: crate::pm::ProjectStatus::Started,
            prompt_context: String::new(),
            pm_snapshot_synced_at: now.unix_timestamp(),
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
            id: LinearIssueId::new("source-issue").unwrap(),
            identifier: "FIX-1".into(),
            title: "Deliver the command".into(),
            description: String::new(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        pm_writeback: PmWritebackState::Current,
        wave_id: wave.id().clone(),
        project_id: project.id.clone(),
        worktree: repo.path().canonicalize().unwrap(),
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
            project.plan.id.as_str(),
            &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
        )
        .unwrap();
        store.create_task(&task, Some(&pr), None).await.unwrap();
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
