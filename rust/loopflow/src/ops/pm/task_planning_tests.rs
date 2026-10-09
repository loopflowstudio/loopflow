//! Task planning operations against stateful, isolated Linear responses.

use std::cell::RefCell;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::json;

use super::test_fixture::{now, Fixture, PlanningEnvironment};
use super::{PmRefresh, PM_TEST_CONTEXT};
use crate::durable::WorkStatus;
use crate::ops::NullProgress;
use crate::work::task::{
    GithubPr, Observation, PmWritebackState, PrMergeMode, PrMergeRequest, PrPhase, PrPresentation,
    PrPublication, Task, TaskEventKind, TaskPr, TaskPrId,
};

async fn planning_repo(fixture: &Fixture) -> (PathBuf, crate::work::wave::Wave) {
    let (repo, wave) = fixture.planning_repo().await;
    let project = serde_json::from_value(json!({
        "id": "project-1", "slug": "Chapter", "name": "Chapter", "summary": "",
        "metric_targets": [], "workflow": "feature", "status": "started", "krs": [],
        "initiative_ids": ["initiative-1"], "team_ids": ["team-1"]
    }))
    .unwrap();
    fixture
        .store
        .sqlite
        .put_pm_project(wave.id(), "linear", "initiative-1", &project, now())
        .unwrap();
    crate::store::sqlite::project_selection::write_project_binding(
        &fixture.store.sqlite,
        wave.id(),
        None,
        "project-1",
        &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
    )
    .unwrap();
    (repo, wave)
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

// Stateful provider evidence for planning fields, state, membership, and comments.
#[derive(Default)]
struct PlanningState {
    export_mode: bool,
    creation_writes: usize,
    attachment_writes: usize,
    lose_creation_reply: bool,
    lose_attachment_reply: bool,
    hide_exports: bool,
    deletion_writes: usize,
    lose_deletion_reply: bool,
    deletion_unconfirmed: bool,
    trash_unavailable: bool,
    field_outage: bool,
    field_reply_lost: bool,
    field_reads_blocked: bool,
    field_writes: usize,
    field_reject_write: bool,
    project_fields: Option<serde_json::Value>,
    field_arrived: Option<Arc<tokio::sync::Notify>>,
    field_release: Option<Arc<tokio::sync::Notify>>,
    issues: Vec<serde_json::Value>,
    extra_projects: Vec<serde_json::Value>,
    fail_snapshot: bool,
    // Discovery and confirmation under the Wave lock consume two reads before mutation.
    fail_issue_read_after: Option<usize>,
    fail_completion: bool,
    missing_canceled_state: bool,
    lose_completion: bool,
    reopen_during_completion: bool,
    lose_comment: bool,
    completion_writes: usize,
    current_project_id: Option<String>,
    initial_project_id: Option<String>,
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
    json!({"id":id, "name":name, "description":"", "content":"workflow: feature",
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
    if state.field_outage
        || (state.field_reads_blocked
            && (query.contains("IssueOwnership")
                || query.contains("ProjectOwnership")
                || query.contains("ListInitiativeProjects")
                || query.contains("ListProjectIssues")))
    {
        return axum::Json(json!({"errors":[{"message":"planning offline"}]}));
    }
    let initial_project_id = state
        .initial_project_id
        .clone()
        .unwrap_or_else(|| "project-1".into());
    let project_id = state
        .current_project_id
        .clone()
        .unwrap_or_else(|| initial_project_id.clone());
    let project = state
        .project_fields
        .clone()
        .unwrap_or_else(|| planning_project(&project_id, &project_id));
    let data = if query.contains("query ListTeams") {
        json!({"teams":{"nodes":[{"id":"team-1","name":"Fixture","key":"FIX",
            "description":"<!-- loopflow-repository: loopflowstudio/fixture -->"}]}})
    } else if query.contains("query ListInitiatives") {
        json!({"initiatives":page(vec![json!({"id":"initiative-1", "name":"Product", "description":""})])})
    } else if query.contains("query ListInitiativeProjects") {
        if !state.issues.is_empty() && state.fail_snapshot {
            state.fail_snapshot = false;
            return axum::Json(json!({"errors":[{"message":"snapshot unavailable"}]}));
        }
        let mut projects = if state.export_mode {
            vec![]
        } else if project_id == "prior-project" {
            vec![project, planning_project(&initial_project_id, &project_id)]
        } else {
            vec![project]
        };
        projects.extend(state.extra_projects.clone());
        json!({"initiative":{"projects":page(projects)}})
    } else if query.contains("query ListProjectIssues") {
        if !state.export_mode
            && state
                .extra_projects
                .iter()
                .any(|project| project["id"] == vars["projectId"])
        {
            return axum::Json(
                json!({"errors":[{"message":"foreign Project issues are unavailable"}]}),
            );
        }
        let issues = state
            .issues
            .iter()
            .filter(|issue| issue["project"]["id"] == vars["projectId"])
            .cloned()
            .collect::<Vec<_>>();
        json!({"project":{"issues":page(issues)}})
    } else if query.contains("query FindProject") {
        let projects = if state.export_mode {
            if state.hide_exports {
                vec![]
            } else {
                state
                    .extra_projects
                    .iter()
                    .filter(|p| p["id"] == vars["id"])
                    .cloned()
                    .collect()
            }
        } else {
            vec![planning_project(vars["id"].as_str().unwrap(), &project_id)]
        };
        json!({"projects":page(projects)})
    } else if query.contains("query ProjectOwnership") {
        let owned = state
            .extra_projects
            .iter()
            .find(|p| p["id"] == vars["id"])
            .cloned()
            .or_else(|| state.project_fields.clone())
            .unwrap_or_else(|| planning_project(vars["id"].as_str().unwrap(), &project_id));
        json!({"project": owned})
    } else if query.contains("query ProjectStatuses") {
        json!({"projectStatuses":page(["planned","started","completed","paused","canceled"].iter()
            .map(|status| json!({"id":status,"type":status,"position":0,"teamId":"team-1"})).collect::<Vec<_>>())})
    } else if query.contains("query FindExportIssue") {
        json!({"issues":{"nodes":if state.hide_exports { vec![] } else {
            state.issues.iter().filter(|i| i["id"]==vars["id"]).map(|i| json!({"id":i["id"]})).collect::<Vec<_>>()
        }}})
    } else if query.contains("mutation DeliverProjectCreation")
        || query.contains("mutation DeliverTaskCreation")
    {
        state.creation_writes += 1;
        let input = &vars["input"];
        let is_project = query.contains("DeliverProjectCreation");
        if is_project {
            let mut project = planning_project(input["id"].as_str().unwrap(), "project-1");
            project["name"] = input["name"].clone();
            project["description"] = input["description"].clone();
            project["content"] = input["content"].clone();
            project["status"] = json!({"type":input["statusId"]});
            project["initiatives"] = json!({"nodes":[]});
            mark_issue_updated(&mut project);
            state.extra_projects.push(project);
        } else {
            let mut issue = json!({"id":input["id"],"identifier":format!("FIX-{}",state.issues.len()+1),
                "title":input["title"],"description":input["description"],"url":"https://fixture.invalid/task",
                "branchName":null,"completedAt":null,"dueDate":input["dueDate"],"trashed":null,"prioritySortOrder":0.0,"sortOrder":0.0,"state":{"type":input["stateId"]},
                "assignee":null,"team":{"id":input["teamId"]},"project":{"id":input["projectId"],"name":"Local chapter"}});
            mark_issue_updated(&mut issue);
            state.issues.push(issue);
        }
        let lost = std::mem::take(&mut state.lose_creation_reply);
        let arrived = state.field_arrived.take();
        let release = state.field_release.take();
        drop(state);
        if let Some(arrived) = arrived {
            arrived.notify_one();
        }
        if let Some(release) = release {
            release.notified().await;
        }
        return axum::Json(if lost {
            json!({"errors":[{"message":"creation reply lost"}]})
        } else if is_project {
            json!({"data":{"projectCreate":{"success":true,"project":{"id":input["id"]}}}})
        } else {
            json!({"data":{"issueCreate":{"success":true,"issue":{"id":input["id"]}}}})
        });
    } else if query.contains("mutation DeliverProjectAttachment") {
        state.attachment_writes += 1;
        let project = state
            .extra_projects
            .iter_mut()
            .find(|p| p["id"] == vars["input"]["projectId"])
            .unwrap();
        project["initiatives"] = json!({"nodes":[{"id":vars["input"]["initiativeId"]}]});
        mark_issue_updated(project);
        if std::mem::take(&mut state.lose_attachment_reply) {
            return axum::Json(json!({"errors":[{"message":"attachment reply lost"}]}));
        }
        json!({"initiativeToProjectCreate":{"success":true}})
    } else if query.contains("mutation DeliverTaskField")
        || query.contains("mutation DeliverProjectField")
    {
        let is_project = query.contains("DeliverProjectField");
        state.field_writes += 1;
        if state.field_reject_write {
            return axum::Json(json!({"errors":[{"message":"write not confirmed"}]}));
        }
        let target = if is_project {
            state.project_fields.get_or_insert(project)
        } else {
            state
                .issues
                .iter_mut()
                .find(|issue| issue["id"] == vars["id"])
                .unwrap()
        };
        for (field, value) in vars["input"].as_object().unwrap() {
            match field.as_str() {
                "assigneeId" => {
                    target["assignee"] = if value.is_null() {
                        json!(null)
                    } else {
                        json!({"id":value})
                    }
                }
                "statusId" => target["status"] = json!({"type":value}),
                "projectId" => target["project"] = json!({"id":value,"name":"Chapter"}),
                _ => target[field] = value.clone(),
            }
        }
        mark_issue_updated(target);
        let lost = state.field_reply_lost;
        if lost {
            state.field_reply_lost = false;
            state.field_reads_blocked = true;
        }
        let arrived = state.field_arrived.take();
        let release = state.field_release.take();
        drop(state);
        if let Some(arrived) = arrived {
            arrived.notify_one();
        }
        if let Some(release) = release {
            release.notified().await;
        }
        return axum::Json(if lost {
            json!({"errors":[{"message":"lost field response"}]})
        } else if is_project {
            json!({"data":{"projectUpdate":{"success":true}}})
        } else {
            json!({"data":{"issueUpdate":{"success":true}}})
        });
    } else if query.contains("query IssueTrash") {
        let issue = if state.trash_unavailable {
            None
        } else {
            state
                .issues
                .iter()
                .find(|issue| issue["id"] == vars["id"])
                .cloned()
        };
        json!({"issue":issue})
    } else if query.contains("mutation DeliverTaskDeletion") {
        state.deletion_writes += 1;
        if state.deletion_unconfirmed {
            return axum::Json(json!({"data":{"issueDelete":{"success":false}}}));
        }
        let issue = state
            .issues
            .iter_mut()
            .find(|issue| issue["id"] == vars["id"])
            .unwrap();
        issue["trashed"] = json!(true);
        mark_issue_updated(issue);
        if state.lose_deletion_reply {
            state.lose_deletion_reply = false;
            return axum::Json(json!({"errors":[{"message":"deletion reply lost"}]}));
        }
        json!({"issueDelete":{"success":true}})
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
        let mut issue = state
            .issues
            .iter()
            .find(|issue| issue["id"] == vars["id"] || issue["identifier"] == vars["id"])
            .unwrap()
            .clone();
        issue["project"] = state
            .extra_projects
            .iter()
            .find(|p| p["id"] == issue["project"]["id"])
            .cloned()
            .unwrap_or(project);
        json!({"issue":issue})
    } else if query.contains("query IssueTeam") {
        json!({"issue":{"team":{"id":"team-1"}}})
    } else if query.contains("query WorkflowStates") {
        if vars["type"] == "completed" && state.reopen_during_completion {
            state.reopen_during_completion = false;
            // Another Linear client completes and explicitly reopens after our
            // ownership read, before our unconditional issueUpdate arrives.
            state.issues[0]["state"] = json!({"type":"completed"});
            mark_issue_updated(&mut state.issues[0]);
            state.issues[0]["state"] = json!({"type":"unstarted"});
            mark_issue_updated(&mut state.issues[0]);
        }
        json!({"workflowStates":{"nodes":if vars["type"] == "canceled" && state.missing_canceled_state {
            vec![]
        } else {
            vec![json!({"id":vars["type"],"position":0})]
        }}})
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
            state.current_project_id = Some(initial_project_id.clone());
            state.issues[0]["project"]["id"] = json!(initial_project_id);
            mark_issue_updated(&mut state.issues[0]);
        }
        json!({"issue":{"attachments":page(state.attachments.iter().map(|url| json!({"url":url})).collect::<Vec<_>>())}})
    } else if query.contains("query IssueObservation") {
        let mut issue = state
            .issues
            .iter()
            .find(|issue| issue["id"] == vars["id"])
            .unwrap()
            .clone();
        issue["comments"] = page(state.comments.clone());
        json!({"issue":issue})
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
    } else {
        panic!("unexpected planning fixture query: {query}");
    };
    axum::Json(json!({"data":data}))
}

#[tokio::test]
async fn project_workflow_uses_stored_definition_offline() {
    let fixture = Fixture::new().await;
    let (repo, wave) = planning_repo(&fixture).await;
    let definition = "nodes: {}\nedges: [{from: start, to: end}]\n";
    let source = repo.join("workflow.yaml");
    std::fs::write(&source, definition).unwrap();
    PM_TEST_CONTEXT
        .scope(fixture.context("http://127.0.0.1:1"), async {
            let selected =
                crate::ops::project::workflow(&repo, "project-1", Some("code"), Some(&source))
                    .await
                    .unwrap();
            assert_eq!(selected.project.workflow, "code");
            assert_eq!(selected.project.id, "project-1");
            assert_eq!(selected.project.initiative_ids, ["initiative-1"]);
            assert_eq!(selected.project.team_ids, ["team-1"]);
            std::fs::remove_file(&source).unwrap();
            assert_eq!(
                crate::ops::project::workflow_source(&repo, "project-1", "code")
                    .await
                    .unwrap(),
                definition
            );
            assert_eq!(
                fixture
                    .store
                    .sqlite
                    .wave_workflow(wave.id(), "code")
                    .unwrap()
                    .as_deref(),
                Some(definition)
            );
            let catalog = crate::ops::project::workflow_catalog(&repo, Some("project-1"))
                .await
                .unwrap();
            let entries = catalog
                .iter()
                .filter(|entry| entry.name == "code")
                .collect::<Vec<_>>();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].source.as_deref(), Some("stored"));
            assert!(entries[0].workflow.as_ref().unwrap().nodes.is_empty());
            assert!(!repo.join(".lf/workflows").exists());
            assert!(fixture.store.list_tasks(None).await.unwrap().is_empty());
        })
        .await;
}

fn seed_provider_task(
    runtime: &tokio::runtime::Runtime,
    state: &Arc<tokio::sync::Mutex<PlanningState>>,
    repo: &Path,
    title: &str,
    description: &str,
) -> crate::ops::OpsResult<crate::pm::PmItem> {
    runtime.block_on(async {
        let mut state = state.lock().await;
        let project = state
            .initial_project_id
            .as_deref()
            .unwrap_or("project-1")
            .to_string();
        let index = state.issues.len() + 1;
        let id = format!("issue-{index}");
        state.issues.push(json!({
            "id":id,"identifier":format!("FIX-{index}"),"url":null,
            "title":title,"description":description,"completedAt":null,"dueDate":null,
            "prioritySortOrder":0.0,"sortOrder":index as f64,"assignee":null,"trashed":null,
            "updatedAt":"2026-09-29T12:00:00.123Z","state":{"type":"unstarted"},
            "team":{"id":"team-1"},"project":{"id":project,"name":"Chapter"}
        }));
        drop(state);
        super::load_show_snapshot(repo, "product", PmRefresh::Force, &NullProgress).await?;
        Ok(super::read_task_planning_async(repo, &id, PmRefresh::Force)
            .await?
            .item)
    })
}

fn with_planning_task(
    test: impl FnOnce(
        &tokio::runtime::Runtime,
        &Fixture,
        &Path,
        &Task,
        Arc<tokio::sync::Mutex<PlanningState>>,
    ),
) {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    let (repo, _) = runtime.block_on(planning_repo(&fixture));
    runtime.block_on(fixture.seed(now() + 86_400));
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState::default()));
    let (url, server) = runtime.block_on(serve(state.clone()));
    PM_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        let item = seed_provider_task(
            &runtime,
            &state,
            &repo,
            "Deliver locally",
            "Retain decisions",
        )
        .unwrap();
        let task = runtime
            .block_on(fixture.store.get_task_by_issue(&item.id))
            .unwrap()
            .unwrap();
        test(&runtime, &fixture, &repo, &task, state);
    });
    server.abort();
}

#[test]
fn task_deletion_active_sync_reconnect_retains_execution_and_history() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
        conn.execute(
            "UPDATE tasks SET worktree=?2,branch='retain/branch',base_commit='retained-base' WHERE id=?1",
            rusqlite::params![task.id.as_str(), repo.to_str().unwrap()],
        )
        .unwrap();
        conn.execute("INSERT INTO processes(lfid,trace_id,command,cwd,started_at) VALUES('11111111-1111-4111-8111-111111111111','22222222-2222-4222-8222-222222222222','lf run code',?1,2)",[repo.to_str().unwrap()]).unwrap();
        conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,cwd,task_id,wave_id,driver_process_lfid,provider_thread,input_published) VALUES('session-retained','Conversation','human',2,?1,?2,?3,'11111111-1111-4111-8111-111111111111','native-retained',1)",rusqlite::params![repo.to_str().unwrap(),task.id.as_str(),task.wave_id.as_str()]).unwrap();
        conn.execute("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at) VALUES('pr-retained',?1,1,'task','retain/branch','retained-base',1,7)",[task.id.as_str()]).unwrap();
        let graph = json!({"name":"code", "nodes":[{"name":"review","skill":"review","description":null}],
            "edges":[{"from":"start","to":"review","flow":"implement"},{"from":"review","to":"end","flow":null}]});
        conn.execute("INSERT INTO task_workflows(task_id,graph,node,edge,process_lfid,updated_at) VALUES(?1,?2,'start',0,'11111111-1111-4111-8111-111111111111',3)",rusqlite::params![task.id.as_str(),graph.to_string()]).unwrap();
        conn.execute("INSERT INTO task_workflow_moves(task_id,workflow,kind,from_node,to_node,edge,process_lfid,note,at) VALUES(?1,'code','chose','start','review',0,'11111111-1111-4111-8111-111111111111','Original choice',3)",[task.id.as_str()]).unwrap();
        let rows = || {
            [
                "processes",
                "agent_sessions",
                "task_prs",
                "task_workflows",
                "task_workflow_moves",
            ]
            .iter()
            .map(|table| {
                let mut query = conn
                    .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
                    .unwrap();
                let columns = query.column_count();
                query
                    .query_map([], |row| {
                        (0..columns)
                            .map(|i| row.get::<_, rusqlite::types::Value>(i))
                            .collect::<rusqlite::Result<Vec<_>>>()
                    })
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap()
            })
            .collect::<Vec<_>>()
        };
        let before = rows();
        let workflow = fixture.store.sqlite.workflow(&task.id).unwrap().unwrap();
        std::fs::write(repo.join("retained-work"), "unfinished").unwrap();
        runtime.block_on(async { state.lock().await.field_outage = true });
        crate::ops::task::task_delete(repo, task.id.as_str()).unwrap();
        let receipt = fixture
            .store
            .sqlite
            .pending_task_changes(&task.id)
            .unwrap()
            .remove(0);
        let sync =
            crate::ops::linear_observe::PlanningSync::start(fixture.store.clone(), task.clone())
                .unwrap();
        runtime.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(8), async {
                loop {
                    let error: Option<String> = conn
                        .query_row(
                            "SELECT error FROM task_changes WHERE id=?1",
                            [&receipt.id],
                            |row| row.get(0),
                        )
                        .unwrap();
                    if error.is_some() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
                state.lock().await.field_outage = false;
                while !fixture
                    .store
                    .sqlite
                    .pending_task_changes(&task.id)
                    .unwrap()
                    .is_empty()
                {
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
            })
            .await
            .unwrap();
            let provider = state.lock().await;
            assert_eq!(provider.issues[0]["trashed"], true);
            assert_eq!(provider.deletion_writes, 1);
        });
        drop(sync);
        assert_eq!(rows(), before);
        assert_eq!(
            fixture.store.sqlite.workflow(&task.id).unwrap().unwrap(),
            workflow
        );
        assert_eq!(
            std::fs::read_to_string(repo.join("retained-work")).unwrap(),
            "unfinished"
        );
        let reopened =
            crate::store::sqlite::SqliteStore::open_ephemeral(&fixture.database).unwrap();
        assert!(reopened.list_tasks(None).unwrap().is_empty());
        assert_eq!(
            reopened.task(&task.id).unwrap().unwrap().plan.linear_id,
            task.plan.linear_id
        );
        assert!(reopened.pending_task_changes(&task.id).unwrap().is_empty());
        assert_eq!(
            conn.query_row(
                "SELECT id FROM task_changes WHERE task_id=?1 AND field='deleted'",
                [task.id.as_str()],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            receipt.id
        );
    });
}

#[test]
fn task_deletion_lost_reply_requires_positive_trash_evidence_without_replay() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        crate::ops::task::task_delete(repo, task.id.as_str()).unwrap();
        let receipt = fixture
            .store
            .sqlite
            .pending_task_changes(&task.id)
            .unwrap()
            .remove(0);
        let work = crate::durable::WorkRef::Task(task.id.clone());
        runtime.block_on(async {
            state.lock().await.lose_deletion_reply = true;
            assert!(
                crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                    .await
                    .is_err()
            );
            assert_eq!(state.lock().await.issues[0]["trashed"], true);
            state.lock().await.trash_unavailable = true;
            assert!(
                crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                    .await
                    .is_err()
            );
            assert_eq!(
                fixture.store.sqlite.pending_task_changes(&task.id).unwrap()[0].id,
                receipt.id
            );
            state.lock().await.trash_unavailable = false;
            crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                .await
                .unwrap();
            crate::ops::planning_delivery::sync_repository_fields(&fixture.store, task)
                .await
                .unwrap();
            assert_eq!(state.lock().await.deletion_writes, 1);
        });
        assert!(fixture
            .store
            .sqlite
            .pending_task_changes(&task.id)
            .unwrap()
            .is_empty());
    });
}

#[test]
fn task_deletion_concurrent_delivery_has_one_effect() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        crate::ops::task::task_delete(repo, task.id.as_str()).unwrap();
        let work = crate::durable::WorkRef::Task(task.id.clone());
        runtime.block_on(async {
            let (first, second) = tokio::join!(
                crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work),
                crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work),
            );
            first.unwrap();
            second.unwrap();
            assert_eq!(state.lock().await.deletion_writes, 1);
        });
        assert!(fixture
            .store
            .sqlite
            .pending_task_changes(&task.id)
            .unwrap()
            .is_empty());
    });
}

#[test]
fn task_deletion_old_trash_observation_cannot_settle_newer_removal() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        crate::ops::task::task_delete(repo, task.id.as_str()).unwrap();
        let receipt = fixture
            .store
            .sqlite
            .pending_task_changes(&task.id)
            .unwrap()
            .remove(0);
        runtime.block_on(async {
            let mut provider = state.lock().await;
            provider.issues[0]["trashed"] = json!(true);
            provider.issues[0]["updatedAt"] = json!("2026-09-28T12:00:00Z");
            drop(provider);
            assert!(crate::ops::planning_delivery::sync_fields(
                &fixture.store,
                repo,
                &crate::durable::WorkRef::Task(task.id.clone())
            )
            .await
            .is_err());
            assert_eq!(state.lock().await.deletion_writes, 0);
        });
        assert_eq!(
            fixture.store.sqlite.pending_task_changes(&task.id).unwrap()[0].id,
            receipt.id
        );
    });
}

#[test]
fn task_deletion_unconfirmed_write_stays_uncertain_without_replay() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        crate::ops::task::task_delete(repo, task.id.as_str()).unwrap();
        let work = crate::durable::WorkRef::Task(task.id.clone());
        runtime.block_on(async {
            state.lock().await.deletion_unconfirmed = true;
            for _ in 0..2 {
                assert!(
                    crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                        .await
                        .is_err()
                );
            }
            assert_eq!(state.lock().await.deletion_writes, 1);
            assert_ne!(state.lock().await.issues[0]["trashed"], true);
        });
        assert_eq!(
            fixture
                .store
                .sqlite
                .pending_task_changes(&task.id)
                .unwrap()
                .len(),
            1
        );
        assert!(fixture.store.sqlite.list_tasks(None).unwrap().is_empty());
    });
}

#[test]
fn task_deletion_adopts_newer_linear_edit_and_retains_losing_removal() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        crate::ops::task::task_delete(repo, task.id.as_str()).unwrap();
        let receipt = fixture
            .store
            .sqlite
            .pending_task_changes(&task.id)
            .unwrap()
            .remove(0);
        runtime.block_on(async {
            let mut provider = state.lock().await;
            provider.issues[0]["title"] = json!("Keep this work");
            mark_issue_updated(&mut provider.issues[0]);
            drop(provider);
            crate::ops::planning_delivery::sync_fields(
                &fixture.store,
                repo,
                &crate::durable::WorkRef::Task(task.id.clone()),
            )
            .await
            .unwrap();
            assert_eq!(state.lock().await.deletion_writes, 0);
        });
        assert_eq!(
            fixture.store.sqlite.list_tasks(None).unwrap()[0].plan.title,
            "Keep this work"
        );
        assert!(fixture
            .store
            .sqlite
            .pending_task_changes(&task.id)
            .unwrap()
            .is_empty());
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        let (value, conflict): (String, String) = conn
            .query_row(
                "SELECT value_json,conflict_json FROM task_changes WHERE id=?1",
                [receipt.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(value, "true");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&conflict).unwrap()["value"],
            false
        );
    });
}

#[test]
fn task_abandonment_saves_offline_in_both_connection_modes() {
    for connected in [false, true] {
        with_planning_task(|runtime, fixture, repo, task, _provider| {
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            if !connected {
                std::fs::write(repo.join(".lf/config.yaml"), "{}\n").unwrap();
                conn.execute(
                    "UPDATE tasks SET external_issue_id=NULL WHERE id=?1",
                    [task.id.as_str()],
                )
                .unwrap();
                conn.execute(
                    "UPDATE projects SET external_project_id=NULL WHERE id=?1",
                    [task.project_id.as_str()],
                )
                .unwrap();
            }
            let original = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
            let history = runtime
                .block_on(fixture.store.task_events_after(&task.id, 0))
                .unwrap();
            // Every provider operation would fail; the existing Task needs none.
            PM_TEST_CONTEXT.sync_scope(fixture.context("http://127.0.0.1:1"), || {
                assert_eq!(crate::ops::task::task_abandon(repo, Some(task.id.as_str()), false).unwrap(), task.plan.identifier);
                let work = crate::durable::WorkRef::Task(task.id.clone());
                assert_eq!(fixture.store.sqlite.work_status(&work).unwrap(), WorkStatus::Abandoned);
                let receipt = || conn.query_row(
                    "SELECT id,target,attempted,settled FROM task_state_deliveries WHERE task_id=?1",
                    [task.id.as_str()], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, bool>(2)?, row.get::<_, bool>(3)?)),
                ).unwrap();
                let saved = receipt();
                assert_eq!((&saved.1, saved.2, saved.3), (&"canceled".to_string(), false, false));
                crate::ops::task::task_abandon(repo, Some(task.id.as_str()), false).unwrap();
                assert_eq!(receipt(), saved);
                let retained = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
                assert_eq!(retained.id, original.id);
                assert_eq!(retained.project_id, original.project_id);
                let mut expected = original.plan.clone();
                expected.revision += 1;
                assert_eq!(retained.plan, expected);
                assert!(retained.worktree.is_none());
                assert!(fixture.store.sqlite.workflow(&task.id).unwrap().is_none());
                assert_eq!(runtime.block_on(fixture.store.task_events_after(&task.id, 0)).unwrap(), history);
                if connected {
                    assert!(matches!(retained.pm_writeback, PmWritebackState::Pending {
                        operation: crate::work::task::PmWritebackOperation::CancelTask, ..
                    }));
                    runtime.block_on(crate::ops::linear_observe::sync_task_state(&fixture.store, &retained)).unwrap();
                    // Cancellation must never fall into the old sender's reopen arm.
                    assert_eq!(receipt(), saved);
                }
                let reopened = crate::store::sqlite::SqliteStore::open_ephemeral(&fixture.database).unwrap();
                assert_eq!(reopened.work_status(&work).unwrap(), WorkStatus::Abandoned);
                assert_eq!(receipt(), saved);
            });
        });
    }
}

#[test]
fn task_abandonment_active_sync_delivers_after_reconnect() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async { state.lock().await.field_outage = true });
        crate::ops::task::task_abandon(repo, Some(task.id.as_str()), false).unwrap();
        let receipt = fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .unwrap();
        let sync =
            crate::ops::linear_observe::PlanningSync::start(fixture.store.clone(), task.clone())
                .unwrap();
        runtime.block_on(async {
            // Observe the foreground connection's failed attempt, then restore the
            // provider without another command, agent turn or manual refresh.
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(8), async {
                loop {
                    let error: Option<String> = conn
                        .query_row(
                            "SELECT error FROM task_state_deliveries WHERE id=?1",
                            [&receipt.id],
                            |row| row.get(0),
                        )
                        .unwrap();
                    if error.is_some() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
                state.lock().await.field_outage = false;
                while fixture
                    .store
                    .sqlite
                    .pending_task_state(&task.id)
                    .unwrap()
                    .is_some()
                {
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
            })
            .await
            .unwrap();
            let provider = state.lock().await;
            assert_eq!(provider.issues[0]["state"]["type"], "canceled");
            assert_eq!(provider.completion_writes, 1);
            assert_eq!(
                conn.query_row(
                    "SELECT id FROM task_state_deliveries WHERE task_id=?1",
                    [task.id.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
                receipt.id
            );
        });
        drop(sync);
        let saved = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
        assert!(matches!(saved.pm_writeback, PmWritebackState::Current));
        assert_eq!(
            fixture
                .store
                .sqlite
                .work_status(&crate::durable::WorkRef::Task(task.id.clone()),)
                .unwrap(),
            WorkStatus::Abandoned
        );
        assert!(fixture.store.sqlite.workflow(&task.id).unwrap().is_none());
    });
}

#[test]
fn task_abandonment_lost_reply_reconciles_without_repeating_cancellation() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        crate::ops::task::task_abandon(repo, Some(task.id.as_str()), false).unwrap();
        let receipt = fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .unwrap();
        runtime.block_on(async {
            state.lock().await.lose_completion = true;
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            let pending = fixture
                .store
                .sqlite
                .pending_task_state(&task.id)
                .unwrap()
                .unwrap();
            assert_eq!(pending.id, receipt.id);
            assert!(pending.attempted);
            assert_eq!(state.lock().await.issues[0]["state"]["type"], "canceled");
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            assert!(fixture
                .store
                .sqlite
                .pending_task_state(&task.id)
                .unwrap()
                .is_none());
            assert_eq!(state.lock().await.completion_writes, 1);
        });
    });
}

#[test]
fn task_abandonment_adopts_linear_conflict_and_retains_local_decision() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        crate::ops::task::task_abandon(repo, Some(task.id.as_str()), false).unwrap();
        let receipt = fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .unwrap();
        runtime.block_on(async {
            let mut provider = state.lock().await;
            provider.issues[0]["state"] = json!({"type":"completed"});
            mark_issue_updated(&mut provider.issues[0]);
            drop(provider);
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            assert_eq!(state.lock().await.completion_writes, 0);
        });
        assert!(fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .is_none());
        assert_eq!(
            fixture
                .store
                .sqlite
                .planning_task(&task.id)
                .unwrap()
                .record
                .unwrap()
                .item
                .state
                .as_deref(),
            Some("completed")
        );
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        let (target, conflict): (String, String) = conn
            .query_row(
                "SELECT target,conflict_json FROM task_state_deliveries WHERE id=?1",
                [&receipt.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(target, "canceled");
        assert!(conflict.contains("completed"));
        assert_eq!(
            fixture
                .store
                .sqlite
                .work_status(&crate::durable::WorkRef::Task(task.id.clone()),)
                .unwrap(),
            WorkStatus::Abandoned
        );
    });
}

#[test]
fn task_abandonment_missing_linear_state_leaves_the_save_retryable() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        crate::ops::task::task_abandon(repo, Some(task.id.as_str()), false).unwrap();
        let receipt = fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .unwrap();
        runtime.block_on(async {
            state.lock().await.missing_canceled_state = true;
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            let pending = fixture
                .store
                .sqlite
                .pending_task_state(&task.id)
                .unwrap()
                .unwrap();
            assert_eq!(pending.id, receipt.id);
            assert!(!pending.attempted);
            assert_eq!(state.lock().await.issues[0]["state"]["type"], "unstarted");
            state.lock().await.missing_canceled_state = false;
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            assert!(fixture
                .store
                .sqlite
                .pending_task_state(&task.id)
                .unwrap()
                .is_none());
            assert_eq!(state.lock().await.issues[0]["state"]["type"], "canceled");
        });
    });
}

#[test]
fn task_abandonment_rolls_back_if_delivery_cannot_commit() {
    with_planning_task(|_runtime, fixture, repo, task, _state| {
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER fail_delivery BEFORE INSERT ON task_state_deliveries
            BEGIN SELECT RAISE(ABORT,'fixture delivery storage failed'); END;",
        )
        .unwrap();
        let error =
            crate::ops::task::task_abandon(repo, Some(task.id.as_str()), false).unwrap_err();
        assert!(error
            .to_string()
            .contains("fixture delivery storage failed"));
        assert_eq!(
            fixture
                .store
                .sqlite
                .work_status(&crate::durable::WorkRef::Task(task.id.clone()))
                .unwrap(),
            WorkStatus::Ready
        );
        assert!(fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .is_none());
        assert_eq!(
            conn.query_row(
                "SELECT abandon_reason FROM tasks WHERE id=?1",
                [task.id.as_str()],
                |row| row.get::<_, Option<String>>(0)
            )
            .unwrap(),
            None
        );
    });
}

#[test]
fn task_completion_retains_request_but_rolls_back_status_if_delivery_cannot_commit() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER fail_delivery BEFORE INSERT ON task_state_deliveries
            BEGIN SELECT RAISE(ABORT,'fixture delivery storage failed'); END;",
        )
        .unwrap();
        let before = runtime
            .block_on(fixture.store.task_events_after(&task.id, 0))
            .unwrap();
        let error =
            crate::ops::task::task_complete(repo, task.id.as_str(), Some("No partial decision"))
                .unwrap_err();
        assert!(error
            .to_string()
            .contains("fixture delivery storage failed"));
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
        let events = runtime
            .block_on(fixture.store.task_events_after(&task.id, 0))
            .unwrap();
        assert_eq!(&events[..before.len()], before.as_slice());
        assert_eq!(events.len(), before.len() + 1);
        assert!(matches!(&events.last().unwrap().kind,
            TaskEventKind::CompletionRequested { reason } if reason.as_deref() == Some("No partial decision")));
        assert!(fixture
            .store
            .sqlite
            .task_completion_pending(&task.id)
            .unwrap()
            .is_some());
        assert!(fixture.store.sqlite.workflow(&task.id).unwrap().is_none());
        assert!(fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .is_none());
        assert_eq!(
            runtime.block_on(async { state.lock().await.completion_writes }),
            0
        );
        conn.execute_batch("DROP TRIGGER fail_delivery").unwrap();
        crate::ops::task::task_complete(repo, task.id.as_str(), None).unwrap();
        assert!(fixture
            .store
            .sqlite
            .task_completion_pending(&task.id)
            .unwrap()
            .is_none());
        assert!(fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .is_some());
    });
}

#[test]
fn task_completion_ingestion_preserves_baseline_and_atomically_adopts_linear() {
    with_planning_task(|runtime, fixture, repo, task, _state| {
        let mut remote = fixture
            .store
            .sqlite
            .planning_task(&task.id)
            .unwrap()
            .record
            .unwrap();
        crate::ops::task::task_complete(repo, task.id.as_str(), Some("Saved offline")).unwrap();
        let delivery = fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .unwrap();
        let workflow = fixture.store.sqlite.workflow(&task.id).unwrap();
        // A changed unrelated field does not conflict with the pending state save.
        remote.item.name = "Linear title".into();
        remote.item.revision = Some("2026-10-08T01:00:00Z".into());
        remote.observed_at += 1;
        fixture
            .store
            .sqlite
            .put_pm_task(&repo.display().to_string(), "linear", &remote, None)
            .unwrap();
        assert_eq!(
            fixture
                .store
                .sqlite
                .pending_task_state(&task.id)
                .unwrap()
                .unwrap()
                .id,
            delivery.id
        );
        let saved = fixture
            .store
            .sqlite
            .planning_task(&task.id)
            .unwrap()
            .record
            .unwrap()
            .item;
        assert_eq!(saved.state.as_deref(), Some("completed"));
        assert_eq!(saved.name, "Linear title");
        remote.item.state = Some("canceled".into());
        remote.item.revision = Some("2026-10-08T02:00:00Z".into());
        remote.observed_at += 1;
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        conn.execute_batch("CREATE TRIGGER fail_adoption BEFORE INSERT ON task_events BEGIN SELECT RAISE(ABORT,'fixture rollback'); END;").unwrap();
        assert!(fixture
            .store
            .sqlite
            .put_pm_task(&repo.display().to_string(), "linear", &remote, None)
            .is_err());
        assert_eq!(
            fixture
                .store
                .sqlite
                .pending_task_state(&task.id)
                .unwrap()
                .unwrap()
                .id,
            delivery.id
        );
        assert_eq!(
            fixture
                .store
                .sqlite
                .planning_task(&task.id)
                .unwrap()
                .record
                .unwrap()
                .item
                .state
                .as_deref(),
            Some("completed")
        );
        conn.execute_batch("DROP TRIGGER fail_adoption").unwrap();
        fixture
            .store
            .sqlite
            .put_pm_task(&repo.display().to_string(), "linear", &remote, None)
            .unwrap();
        fixture
            .store
            .sqlite
            .settle_task_state(&delivery, None)
            .unwrap();
        assert!(fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .is_none());
        assert_eq!(
            fixture
                .store
                .sqlite
                .planning_task(&task.id)
                .unwrap()
                .record
                .unwrap()
                .item
                .state
                .as_deref(),
            Some("canceled")
        );
        assert_eq!(fixture.store.sqlite.workflow(&task.id).unwrap(), workflow);
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
        let conflict: String = conn
            .query_row(
                "SELECT conflict_json FROM task_state_deliveries WHERE id=?1",
                [&delivery.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&conflict).unwrap()["state"],
            "canceled"
        );
    });
}

#[test]
fn task_completion_late_acknowledgement_preserves_explicit_reopening() {
    with_planning_task(|runtime, fixture, repo, task, _state| {
        crate::ops::task::task_complete(repo, task.id.as_str(), Some("Delivered")).unwrap();
        let completed = fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .unwrap();
        assert!(fixture.store.sqlite.attempt_task_state(&completed).unwrap());
        let process = crate::process::Process {
            lfid: crate::id::ProcessLfid::new(),
            pid: None,
            trace_id: crate::id::TraceId::new(),
            parent_process_lfid: None,
            via_agent: None,
            caller_session_id: None,
            caller_provider_generation: None,
            command: Some("task move start".into()),
            repo: None,
            cwd: None,
            started_at: 1,
            completed_at: None,
            outcome: None,
            exit_code: None,
            signal: None,
            error: None,
        };
        fixture.store.sqlite.record_process(&process).unwrap();
        fixture
            .store
            .sqlite
            .set_workflow_node(&task.id, "start", &process.lfid, Some("New scope"))
            .unwrap();
        assert_eq!(
            fixture
                .store
                .sqlite
                .pending_task_state(&task.id)
                .unwrap()
                .unwrap()
                .id,
            completed.id
        );
        crate::ops::task::task_reopen(task.id.as_str(), Some("New scope")).unwrap();
        let reopened = fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .unwrap();
        assert_ne!(reopened.id, completed.id);
        let mut late = fixture
            .store
            .sqlite
            .planning_task(&task.id)
            .unwrap()
            .record
            .unwrap()
            .item;
        late.state = Some("canceled".into());
        late.revision = Some("2026-10-08T03:00:00Z".into());
        assert!(!fixture
            .store
            .sqlite
            .observe_task_state(&completed, &late)
            .unwrap());
        assert_eq!(
            fixture
                .store
                .sqlite
                .planning_task(&task.id)
                .unwrap()
                .record
                .unwrap()
                .item
                .state
                .as_deref(),
            Some("unstarted")
        );
        assert_eq!(reopened.target, "unstarted");
        fixture
            .store
            .sqlite
            .settle_task_state(&completed, None)
            .unwrap();
        // A stale failure cannot erase the receipt for that completed effect.
        fixture
            .store
            .sqlite
            .settle_task_state(&completed, Some("late failure"))
            .unwrap();
        assert_eq!(
            fixture
                .store
                .sqlite
                .pending_task_state(&task.id)
                .unwrap()
                .unwrap()
                .id,
            reopened.id
        );
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
        assert!(!fixture.store.sqlite.attempt_task_state(&completed).unwrap());
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        let receipt: (bool, Option<String>) = conn
            .query_row(
                "SELECT settled,error FROM task_state_deliveries WHERE id=?1",
                [&completed.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(receipt, (true, None));
        assert!(conn
            .query_row(
                "SELECT attempted FROM task_state_deliveries WHERE id=?1",
                [&completed.id],
                |row| row.get::<_, bool>(0)
            )
            .unwrap());
    });
}

#[test]
fn task_completion_active_sync_acquires_membership_while_delivery_is_pending() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        crate::ops::task::task_complete(repo, task.id.as_str(), Some("Saved during outage"))
            .unwrap();
        runtime.block_on(async {
            let mut provider = state.lock().await;
            provider.issues[0]["state"] = json!({"type":"canceled"});
            mark_issue_updated(&mut provider.issues[0]);
            let mut added = provider.issues[0].clone();
            added["id"] = json!("issue-2");
            added["identifier"] = json!("FIX-2");
            added["state"] = json!({"type":"completed"});
            provider.issues.push(added);
        });
        let sync =
            crate::ops::linear_observe::PlanningSync::start(fixture.store.clone(), task.clone())
                .unwrap();
        runtime.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(8), async {
                loop {
                    let saved = fixture.store.get_task(&task.id).await.unwrap().unwrap();
                    let added = fixture.store.get_task_by_issue("FIX-2").await.unwrap();
                    if let (PmWritebackState::Current, Some(added)) = (&saved.pm_writeback, added) {
                        if fixture
                            .store
                            .sqlite
                            .planning_task(&task.id)
                            .unwrap()
                            .record
                            .unwrap()
                            .item
                            .state
                            .as_deref()
                            == Some("canceled")
                        {
                            // Remote completion is a planning observation, never Workflow authority.
                            assert_eq!(
                                fixture
                                    .store
                                    .work_status(&crate::durable::WorkRef::Task(added.id.clone()))
                                    .await
                                    .unwrap(),
                                WorkStatus::Done
                            );
                            assert!(fixture.store.sqlite.workflow(&added.id).unwrap().is_none());
                            break;
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(state.lock().await.completion_writes, 0);
            assert_eq!(
                fixture
                    .store
                    .work_status(&crate::durable::WorkRef::Task(task.id.clone()))
                    .await
                    .unwrap(),
                WorkStatus::Ready
            );
        });
        drop(sync);
    });
}

#[test]
fn task_completion_documents_unseen_linear_reopening_overwrite() {
    // Counterexample, not an atomic-write guarantee. Observed conflicts adopt
    // Linear (covered above/below); an edit between read and mutation is best effort.
    with_planning_task(|runtime, fixture, repo, task, state| {
        crate::ops::task::task_complete(repo, task.id.as_str(), Some("Delivered locally")).unwrap();
        runtime.block_on(async {
            state.lock().await.reopen_during_completion = true;
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            let provider = state.lock().await;
            assert!(!provider.reopen_during_completion);
            let retained = fixture.store.sqlite.pending_task_state(&task.id).unwrap();
            assert_eq!(
                (&provider.issues[0]["state"]["type"], retained.is_some()),
                (&json!("completed"), false),
                "an unconditional write can overwrite an unseen reopening; matching readback cannot detect it"
            );
        });
    });
}

#[test]
fn task_completion_lost_reply_adopts_linear_reopening() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        crate::ops::task::task_complete(repo, task.id.as_str(), Some("Delivered")).unwrap();
        let original = fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .unwrap();
        runtime.block_on(async {
            state.lock().await.lose_completion = true;
        });
        runtime
            .block_on(crate::ops::linear_observe::sync_task_state(
                &fixture.store,
                task,
            ))
            .unwrap();
        runtime.block_on(async {
            let mut provider = state.lock().await;
            provider.issues[0]["state"] = json!({"type":"unstarted"});
            mark_issue_updated(&mut provider.issues[0]);
        });
        runtime
            .block_on(crate::ops::linear_observe::sync_task_state(
                &fixture.store,
                task,
            ))
            .unwrap();
        assert!(fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .is_none());
        let adopted = fixture
            .store
            .sqlite
            .planning_task(&task.id)
            .unwrap()
            .record
            .unwrap()
            .item;
        assert_eq!(adopted.state.as_deref(), Some("unstarted"));
        assert!(!adopted.completed);
        assert!(fixture.store.sqlite.workflow(&task.id).unwrap().is_none());
        assert!(fixture
            .store
            .sqlite
            .task_completion_pending(&task.id)
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
            WorkStatus::Ready
        );
        // A delayed acknowledgement cannot erase an already observed collision.
        fixture
            .store
            .sqlite
            .settle_task_state(&original, None)
            .unwrap();
        assert!(fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .is_none());
        runtime
            .block_on(crate::ops::linear_observe::sync_task_state(
                &fixture.store,
                task,
            ))
            .unwrap();
        assert_eq!(
            runtime.block_on(async { state.lock().await.completion_writes }),
            1
        );
        // The original uncertain effect and both conflicting values remain history.
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM task_state_deliveries WHERE task_id=?1",
                [task.id.as_str()],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        let retained: String = conn
            .query_row(
                "SELECT conflict_json FROM task_state_deliveries WHERE id=?1",
                [original.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(retained.contains("unstarted"));
    });
}

#[test]
fn task_completion_preserves_planning_identity_and_summary_on_retry() {
    assert_unplaced_completion_retry(false, false);
}

#[test]
fn task_completion_retries_pending_writeback_after_local_done() {
    assert_task_completion_retry(false, None);
}

#[test]
fn task_completion_reconciles_lost_planning_response() {
    assert_unplaced_completion_retry(true, false);
}

#[test]
fn task_completion_reconciles_lost_registered_response() {
    assert_task_completion_retry(true, None);
}

#[test]
fn task_completion_reconciles_provider_outcome_after_user_merge() {
    assert_task_completion_retry(false, Some(PrMergeMode::User));
}

#[test]
fn task_completion_reconciles_provider_outcome_after_landing() {
    assert_task_completion_retry(true, Some(PrMergeMode::Auto));
}

#[test]
fn uncached_task_completion_retains_local_done_after_a_lost_reply() {
    assert_unplaced_completion_retry(true, true);
}

fn assert_unplaced_completion_retry(lose_response: bool, uncached: bool) {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    let (repo, _) = runtime.block_on(planning_repo(&fixture));
    runtime.block_on(fixture.seed(now() + 86_400));
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState::default()));
    let (url, server) = runtime.block_on(serve(state.clone()));
    PM_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        let identifier = if uncached {
            runtime.block_on(async {
                state.lock().await.issues.push(json!({
                    "id":"issue-1", "identifier":"FIX-1", "url":null,
                    "title":"Finish without checkout", "description":"Retain the completion",
                    "completedAt":null,"dueDate":null, "prioritySortOrder":0.0, "sortOrder":0.0,
                    "updatedAt":"2026-09-29T12:00:00.123Z", "assignee":null,
                    "state":{"type":"unstarted"}, "team":{"id":"team-1"},
                    "project":{"id":"project-1","name":"Chapter"}
                }));
            });
            assert!(runtime.block_on(fixture.store.list_tasks(None)).unwrap().is_empty());
            "FIX-1".to_string()
        } else {
            seed_provider_task(&runtime, &state, &repo, "Finish without checkout", "Retain the completion")
            .unwrap()
            .identifier
        };
        runtime.block_on(async {
            let mut provider = state.lock().await;
            provider.fail_issue_read_after = (!lose_response && !uncached).then_some(1);
            provider.lose_completion = lose_response;
        });
        let complete = || {
            crate::ops::task::task_complete(
                &repo,
                &identifier,
                Some("Delivered the requested outcome"),
            )
        };
        complete().unwrap();
        let pending = runtime
            .block_on(fixture.store.get_task_by_issue(&identifier))
            .unwrap()
            .unwrap();
        assert!(matches!(
            pending.pm_writeback,
            PmWritebackState::Pending { .. }
        ));
        assert!(pending.worktree.is_none());
        assert_eq!(
            runtime.block_on(fixture.store.work_status(&crate::durable::WorkRef::Task(pending.id.clone()))).unwrap(),
            WorkStatus::Done
        );
        assert_eq!(runtime.block_on(async { state.lock().await.completion_writes }), 0);
        let delivery = fixture.store.sqlite.pending_task_state(&pending.id).unwrap().unwrap();
        complete().unwrap();
        assert_eq!(fixture.store.sqlite.pending_task_state(&pending.id).unwrap().unwrap().id, delivery.id);
        runtime.block_on(crate::ops::linear_observe::sync_task_state(&fixture.store, &pending)).unwrap();
        runtime.block_on(crate::ops::linear_observe::sync_task_state(&fixture.store, &pending)).unwrap();
        let retained = runtime
            .block_on(fixture.store.get_task(&pending.id))
            .unwrap()
            .unwrap();
        assert_eq!(retained.pm_writeback, PmWritebackState::Current);
        assert!(retained.worktree.is_none());
        let events = runtime
            .block_on(fixture.store.task_events_after(&pending.id, 0))
            .unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.kind, TaskEventKind::Completed { .. }))
                .count(),
            1
        );
        assert_eq!(
            events.iter().filter(|event| matches!(&event.kind, TaskEventKind::CompletionRequested { reason } if reason.as_deref() == Some("Delivered the requested outcome"))).count(),
            1
        );
        assert_eq!(runtime.block_on(fixture.store.list_tasks(None)).unwrap().len(), 1);
        assert!(runtime
            .block_on(fixture.store.task_prs(&pending.id))
            .unwrap()
            .is_empty());
    });
    server.abort();
}

fn assert_task_completion_retry(lose_response: bool, merge: Option<PrMergeMode>) {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    let (repo, wave) = runtime.block_on(planning_repo(&fixture));
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
        let item =
            seed_provider_task(&runtime, &state, &repo, "Future work", "A directive").unwrap();
        let task = {
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
            let mut task = Task {
                id: runtime
                    .block_on(fixture.store.get_task_by_issue(&item.id))
                    .unwrap()
                    .unwrap()
                    .id,
                plan: crate::planning::TaskPlan {
                    revision: 0,
                    linear_id: Some(crate::planning::LinearIssueId::new(&item.id).unwrap()),
                    identifier: item.identifier.clone(),
                    title: item.name.clone(),
                    description: item.description.clone(),
                    pm_snapshot_synced_at: Some(1),
                },
                pm_writeback: PmWritebackState::Current,
                wave_id: wave.id().clone(),
                project_id: project.id,
                worktree: Some(repo.clone()),
                workspace_slug: "completion".into(),
                branch: crate::engine::git::current_branch(&repo).unwrap().unwrap(),
                base_commit: crate::engine::git::rev_parse(&repo, "HEAD").unwrap(),
                parent_pr_id: None,
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
            task = runtime
                .block_on(fixture.store.place_task(
                    &task.id,
                    &repo,
                    &task.workspace_slug,
                    &pr,
                    None,
                ))
                .unwrap();
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
                }),
            });
            if merge.is_some() {
                runtime.block_on(fixture.store.insert_task_pr(&pr)).unwrap();
                // Observe the merge before recording the accepted disposition.
                // Provider completion itself is the operation exercised below.
                runtime
                    .block_on(crate::ops::task::reconcile_delivered_task(
                        &fixture.store,
                        &mut task,
                    ))
                    .unwrap();
                fixture
                    .store
                    .sqlite
                    .finish_follow_through(
                        &task.id,
                        "Accepted checks complete; no later obligation",
                        true,
                    )
                    .unwrap();
            }
            task
        };
        let selector = task.id.as_str();
        let complete = |summary: &str| match merge {
            None | Some(PrMergeMode::User) => {
                crate::ops::task::task_complete(&repo, selector, Some(summary))
            }
            Some(PrMergeMode::Auto) => runtime
                .block_on(async {
                    let task = &task;
                    let pr = fixture.store.task_prs(&task.id).await.unwrap().remove(0);
                    let landing = crate::pr_landing::PrLanding::new(
                        crate::pr_landing::NewPrLanding {
                            repo: "loopflowstudio/fixture".into(),
                            pr_number: 42,
                            worktree: repo.clone(),
                            branch: pr.branch,
                            task_id: Some(task.id.clone()),
                            requested_head_sha: pr.base_commit,
                        },
                        time::OffsetDateTime::now_utc(),
                    )
                    .unwrap();
                    let settlement =
                        crate::ops::task::settle_task_landing(&fixture.store, &landing).await;
                    settlement?;
                    Ok(())
                })
                .and_then(|()| crate::ops::task::task_complete(&repo, selector, Some(summary))),
        };

        let first = complete("Delivered the requested outcome").unwrap();
        assert!(matches!(
            first.pm_writeback,
            PmWritebackState::Pending { .. }
        ));
        assert_eq!(
            runtime.block_on(async { state.lock().await.completion_writes }),
            0
        );
        let before = runtime
            .block_on(fixture.store.task_events_after(&task.id, 0))
            .unwrap();
        assert_eq!(
            before
                .iter()
                .filter(|event| matches!(event.kind, TaskEventKind::Completed { .. }))
                .count(),
            1
        );
        runtime.block_on(async {
            let mut provider = state.lock().await;
            provider.fail_issue_read_after = (!lose_response).then_some(1);
            provider.lose_completion = lose_response;
        });
        runtime
            .block_on(crate::ops::linear_observe::sync_task_state(
                &fixture.store,
                &first,
            ))
            .unwrap();
        runtime
            .block_on(crate::ops::linear_observe::sync_task_state(
                &fixture.store,
                &first,
            ))
            .unwrap();
        let result = complete("Must preserve the first decision").unwrap();
        assert_eq!(result.pm_writeback, PmWritebackState::Current);
        assert_eq!(
            runtime
                .block_on(fixture.store.task_events_after(&task.id, 0))
                .unwrap(),
            before
        );
        assert_eq!(
            runtime.block_on(async { state.lock().await.completion_writes }),
            1
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

#[test]
fn task_abandon_composes_cancellation_pr_and_git_from_anywhere() {
    let _lock = crate::journal::test_env_lock();
    for selector in [Some("FIX-1"), Some("cancel-me"), None] {
        let _restore = PlanningEnvironment::isolate();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let fixture = runtime.block_on(Fixture::new());
        std::env::set_var("LF_HOME", fixture.directory.path());
        let (repo, wave) = runtime.block_on(planning_repo(&fixture));
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
            let item =
                seed_provider_task(&runtime, &state, &repo, "Cancel me", "Keep history").unwrap();
            let timestamp = time::OffsetDateTime::now_utc();
            let project = runtime
                .block_on(fixture.store.list_projects(Some(wave.id())))
                .unwrap()
                .into_iter()
                .find(|project| project.plan.linear_id.as_ref().unwrap().as_str() == "project-1")
                .unwrap();
            let task = Task {
                id: runtime
                    .block_on(fixture.store.get_task_by_issue(&item.id))
                    .unwrap()
                    .unwrap()
                    .id,
                plan: crate::planning::TaskPlan {
                    revision: 0,
                    linear_id: Some(crate::planning::LinearIssueId::new(&item.id).unwrap()),
                    identifier: item.identifier.clone(),
                    title: item.name,
                    description: item.description,
                    pm_snapshot_synced_at: Some(1),
                },
                pm_writeback: PmWritebackState::Current,
                wave_id: wave.id().clone(),
                project_id: project.id,
                worktree: Some(checkout.clone()),
                workspace_slug: "cancel-me".into(),
                branch: "cancel-me".into(),
                base_commit: git(&repo, &["rev-parse", "HEAD"]).trim().into(),
                parent_pr_id: None,
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
                .block_on(fixture.store.seed_task(&task, &pr))
                .unwrap();
            fixture.store.sqlite.test_flow(
                "review",
                &task.worktree.as_ref().unwrap().to_string_lossy(),
                &[("review", Some("failed"))],
                Some("failed"),
            );
            assert_eq!(
                crate::ops::task::task_repository(&checkout, None).unwrap(),
                checkout.canonicalize().unwrap()
            );
            let caller = match selector {
                None => &checkout,
                Some("FIX-1") => fixture.directory.path(),
                Some(_) => &repo,
            };
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
                // Cleanup failure cannot undo cancellation; retry retains the PR and checkout.
                std::fs::write(repo.join(".git/fail-close"), "").unwrap();
                assert_eq!(
                    crate::ops::task::task_abandon(caller, selector, false).unwrap(),
                    "FIX-1"
                );
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
                    runtime
                        .block_on(async { state.lock().await.issues[0]["state"]["type"].clone() }),
                    json!("unstarted")
                );
                assert!(matches!(
                    fixture
                        .store
                        .sqlite
                        .task(&task.id)
                        .unwrap()
                        .unwrap()
                        .pm_writeback,
                    PmWritebackState::Pending {
                        operation: crate::work::task::PmWritebackOperation::CancelTask,
                        ..
                    }
                ));
                assert!(checkout.exists());
                std::fs::remove_file(repo.join(".git/fail-close")).unwrap();
            }
            assert_eq!(
                crate::ops::task::task_abandon(caller, selector, false).unwrap(),
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
            // The Flow's Processes remain as history; abandonment retires nothing.
            assert_eq!(fixture.store.sqlite.task_flows(&task.id).unwrap().len(), 1);
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
            // Retry survives deletion of the checkout and refs.
            crate::ops::task::task_abandon(&repo, Some("cancel-me"), false).unwrap();
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
    let (repo, wave) = runtime.block_on(planning_repo(&fixture));
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
        // Import owned planning while foreign Projects remain independently unavailable.
        seed_provider_task(
            &runtime,
            &state,
            &repo,
            "Eligible work",
            "Cancel only repository work",
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
            let row = fixture.store.pm_snapshot(wave.id()).await.unwrap().unwrap();
            let snapshot = row.snapshot;
            assert_eq!(snapshot.projects.len(), 1);
            assert_eq!(snapshot.projects[0].id, "project-1");
            assert_eq!(snapshot.projects[0].name, "Chapter");
        });
        runtime.block_on(async {
            let mut provider = state.lock().await;
            provider.current_project_id = Some("prior-project".into());
            provider.issues[0]["project"]["id"] = json!("prior-project");
            mark_issue_updated(&mut provider.issues[0]);
            // Duplicate foreign membership still yields one preview entry.
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
            json!("unstarted")
        );
        let saved = fixture
            .store
            .sqlite
            .task_by_issue("FIX-1")
            .unwrap()
            .unwrap();
        assert_eq!(
            fixture
                .store
                .sqlite
                .work_status(&crate::durable::WorkRef::Task(saved.id.clone()))
                .unwrap(),
            WorkStatus::Abandoned
        );
        assert_eq!(
            fixture
                .store
                .sqlite
                .pending_task_state(&saved.id)
                .unwrap()
                .unwrap()
                .target,
            "canceled"
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
    let selected = "00000000-0000-4000-8000-000000000001";
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState {
        initial_project_id: Some(selected.into()),
        ..PlanningState::default()
    }));
    let (url, server) = runtime.block_on(serve(state.clone()));
    PM_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        seed_provider_task(&runtime, &state, &repo, "Planning work", "Retain evidence").unwrap();
        runtime
            .block_on(crate::ops::project::bind_project(
                &repo, "product", selected,
            ))
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

#[test]
fn planning_fields_active_connection_delivers_offline_saves_after_recovery() {
    with_planning_task(|runtime, fixture, _repo, task, state| {
        runtime.block_on(async {
            state.lock().await.field_outage = true;
            let sync = crate::ops::linear_observe::PlanningSync::start(
                fixture.store.clone(),
                task.clone(),
            )
            .unwrap();
            fixture
                .store
                .sqlite
                .edit_task(
                    &task.id,
                    task.plan.revision,
                    &crate::pm::PmItemUpdate {
                        name: Some("Offline title".into()),
                        description: Some("Offline body".into()),
                        assignee: Some(Some("person-1".into())),
                        ..Default::default()
                    },
                )
                .unwrap();
            fixture
                .store
                .sqlite
                .edit_project(
                    &task.project_id,
                    Some("Offline Project"),
                    Some("Offline summary"),
                )
                .unwrap();
            let task_receipts = fixture.store.sqlite.pending_task_changes(&task.id).unwrap();
            let project_receipts = fixture
                .store
                .sqlite
                .pending_project_changes(&task.project_id)
                .unwrap();
            // Observe an actual failed attempt on the still-open foreground connection.
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(8), async {
                loop {
                    let failures: i64 = conn
                        .query_row(
                            "SELECT count(*) FROM task_changes WHERE error IS NOT NULL",
                            [],
                            |row| row.get(0),
                        )
                        .unwrap();
                    if failures > 0 {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(
                fixture.store.sqlite.pending_task_changes(&task.id).unwrap(),
                task_receipts
            );
            assert_eq!(
                fixture
                    .store
                    .sqlite
                    .pending_project_changes(&task.project_id)
                    .unwrap(),
                project_receipts
            );
            state.lock().await.field_outage = false;
            tokio::time::timeout(std::time::Duration::from_secs(8), async {
                loop {
                    if fixture
                        .store
                        .sqlite
                        .pending_task_changes(&task.id)
                        .unwrap()
                        .is_empty()
                        && fixture
                            .store
                            .sqlite
                            .pending_project_changes(&task.project_id)
                            .unwrap()
                            .is_empty()
                    {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
            })
            .await
            .unwrap();
            let provider = state.lock().await;
            assert_eq!(provider.issues[0]["title"], "Offline title");
            assert_eq!(provider.issues[0]["description"], "Offline body");
            assert_eq!(provider.issues[0]["assignee"]["id"], "person-1");
            assert_eq!(
                provider.project_fields.as_ref().unwrap()["name"],
                "Offline Project"
            );
            assert_eq!(
                provider.project_fields.as_ref().unwrap()["description"],
                "Offline summary"
            );
            assert_eq!(provider.field_writes, 5);
            assert!(fixture.store.sqlite.workflow(&task.id).unwrap().is_none());
            assert_eq!(
                fixture
                    .store
                    .work_status(&crate::durable::WorkRef::Task(task.id.clone()))
                    .await
                    .unwrap(),
                WorkStatus::Ready
            );
            drop(provider);
            drop(sync);
        });
    });
}

#[test]
fn planning_fields_lost_reply_retains_identity_and_readback_settles_without_rewrite() {
    for project in [false, true] {
        with_planning_task(|runtime, fixture, repo, task, state| {
            runtime.block_on(async {
                let work = save_field_name(fixture, task, project, "Saved once");
                let receipts = field_receipts(fixture, task, project);
                state.lock().await.field_reply_lost = true;
                crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                    .await
                    .unwrap();
                assert_eq!(field_receipts(fixture, task, project), receipts);
                assert_eq!(state.lock().await.field_writes, 1);
                assert!(
                    crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                        .await
                        .is_err()
                );
                assert_eq!(field_receipts(fixture, task, project), receipts);
                state.lock().await.field_reads_blocked = false;
                // A new store connection recovers the same persisted mutation identity.
                let reopened = crate::store::open_ephemeral_store(
                    &crate::store::StorageConfig::sqlite(fixture.database.clone()),
                )
                .await
                .unwrap();
                crate::ops::planning_delivery::sync_fields(&reopened, repo, &work)
                    .await
                    .unwrap();
                assert!(field_receipts(fixture, task, project).is_empty());
                assert_eq!(state.lock().await.field_writes, 1);
                let conn = rusqlite::Connection::open(&fixture.database).unwrap();
                let table = if project {
                    "project_changes"
                } else {
                    "task_changes"
                };
                assert_eq!(
                    conn.query_row(
                        &format!("SELECT id FROM {table} WHERE acknowledged=1"),
                        [],
                        |row| row.get::<_, String>(0)
                    )
                    .unwrap(),
                    receipts[0].id
                );
                let provider = state.lock().await;
                let revision = if project {
                    &provider.project_fields.as_ref().unwrap()["updatedAt"]
                } else {
                    &provider.issues[0]["updatedAt"]
                };
                assert_eq!(
                    conn.query_row(
                        &format!("SELECT acknowledged_revision FROM {table} WHERE id=?1"),
                        [&receipts[0].id],
                        |row| row.get::<_, String>(0)
                    )
                    .unwrap(),
                    revision.as_str().unwrap()
                );
            });
        });
    }
}

#[test]
fn planning_fields_older_acknowledgement_preserves_and_delivers_newer_save() {
    for project in [false, true] {
        with_planning_task(|runtime, fixture, repo, task, state| {
            runtime.block_on(async {
                let work = save_field_name(fixture, task, project, "First save");
                let first = field_receipts(fixture, task, project)[0].clone();
                let arrived = Arc::new(tokio::sync::Notify::new());
                let release = Arc::new(tokio::sync::Notify::new());
                {
                    let mut provider = state.lock().await;
                    provider.field_arrived = Some(arrived.clone());
                    provider.field_release = Some(release.clone());
                }
                let delivery =
                    crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work);
                let edit = async {
                    arrived.notified().await;
                    save_field_name(fixture, task, project, "Newer save");
                    let newer = field_receipts(fixture, task, project)[0].clone();
                    assert_ne!(first.id, newer.id);
                    release.notify_one();
                    newer
                };
                let (result, newer) = tokio::join!(delivery, edit);
                result.unwrap();
                let pending = field_receipts(fixture, task, project);
                assert_eq!(pending.len(), 1);
                assert_eq!(pending[0].id, newer.id);
                assert_eq!(pending[0].value, "Newer save");
                assert_eq!(pending[0].base.as_ref().unwrap()["value"], "First save");
                if project {
                    assert_eq!(
                        fixture
                            .store
                            .sqlite
                            .planning_project(&task.project_id)
                            .unwrap()
                            .name,
                        "Newer save"
                    );
                } else {
                    assert_eq!(
                        fixture
                            .store
                            .get_task(&task.id)
                            .await
                            .unwrap()
                            .unwrap()
                            .plan
                            .title,
                        "Newer save"
                    );
                }
                crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                    .await
                    .unwrap();
                assert!(field_receipts(fixture, task, project).is_empty());
                let provider = state.lock().await;
                assert_eq!(provider.field_writes, 2);
                let name = if project {
                    &provider.project_fields.as_ref().unwrap()["name"]
                } else {
                    &provider.issues[0]["title"]
                };
                assert_eq!(name, "Newer save");
            });
        });
    }
}

fn save_field_name(
    fixture: &Fixture,
    task: &Task,
    project: bool,
    name: &str,
) -> crate::durable::WorkRef {
    if project {
        fixture
            .store
            .sqlite
            .edit_project(&task.project_id, Some(name), None)
            .unwrap();
        crate::durable::WorkRef::Project(task.project_id.clone())
    } else {
        let saved = fixture
            .store
            .sqlite
            .planning_task(&task.id)
            .unwrap()
            .record
            .unwrap()
            .item;
        let current = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
        assert_eq!(current.plan.title, saved.name);
        fixture
            .store
            .sqlite
            .edit_task(
                &task.id,
                current.plan.revision,
                &crate::pm::PmItemUpdate {
                    name: Some(name.into()),
                    ..Default::default()
                },
            )
            .unwrap();
        crate::durable::WorkRef::Task(task.id.clone())
    }
}

fn field_receipts(
    fixture: &Fixture,
    task: &Task,
    project: bool,
) -> Vec<crate::planning::PlanningChange> {
    if project {
        fixture
            .store
            .sqlite
            .pending_project_changes(&task.project_id)
            .unwrap()
    } else {
        fixture.store.sqlite.pending_task_changes(&task.id).unwrap()
    }
}

#[test]
fn planning_fields_observed_conflicts_adopt_linear_without_writing_or_moving_workflow() {
    for project in [false, true] {
        with_planning_task(|runtime, fixture, repo, task, state| {
            runtime.block_on(async {
                let work = save_field_name(fixture, task, project, "Losing local name");
                let receipt = field_receipts(fixture, task, project)[0].clone();
                {
                    let mut provider = state.lock().await;
                    if project {
                        let mut value = planning_project("project-1", "project-1");
                        value["name"] = json!("Linear name");
                        mark_issue_updated(&mut value);
                        provider.project_fields = Some(value);
                    } else {
                        provider.issues[0]["title"] = json!("Linear name");
                        mark_issue_updated(&mut provider.issues[0]);
                    }
                }
                crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                    .await
                    .unwrap();
                assert!(field_receipts(fixture, task, project).is_empty());
                assert_eq!(state.lock().await.field_writes, 0);
                let conn = rusqlite::Connection::open(&fixture.database).unwrap();
                let table = if project {
                    "project_changes"
                } else {
                    "task_changes"
                };
                let (value, conflict): (String, String) = conn
                    .query_row(
                        &format!("SELECT value_json,conflict_json FROM {table} WHERE id=?1"),
                        [&receipt.id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .unwrap();
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(&value).unwrap(),
                    "Losing local name"
                );
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(&conflict).unwrap()["value"],
                    "Linear name"
                );
                assert_eq!(
                    fixture
                        .store
                        .work_status(&crate::durable::WorkRef::Task(task.id.clone()))
                        .await
                        .unwrap(),
                    WorkStatus::Ready
                );
                assert!(fixture.store.sqlite.workflow(&task.id).unwrap().is_none());
            });
        });
    }
}

#[test]
fn planning_fields_project_content_preserves_prose_and_task_assignee_can_be_cleared() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            let mut provider = planning_project("project-1", "project-1");
            provider["content"] = json!(
                "Keep the introduction.\n\nworkflow: feature\n\n## Notes\nKeep these notes.\n"
            );
            mark_issue_updated(&mut provider);
            state.lock().await.project_fields = Some(provider);
            fixture
                .store
                .sqlite
                .update_project_content(
                    &task.project_id,
                    &crate::pm::ProjectContent {
                        workflow: "code".into(),
                        metric_targets: Vec::new(),
                        krs: vec![crate::pm::PmKr {
                            text: "Keep working offline".into(),
                            holds: true,
                        }],
                    },
                )
                .unwrap();
            crate::ops::planning_delivery::sync_fields(
                &fixture.store,
                repo,
                &crate::durable::WorkRef::Project(task.project_id.clone()),
            )
            .await
            .unwrap();
            assert!(fixture
                .store
                .sqlite
                .pending_project_changes(&task.project_id)
                .unwrap()
                .is_empty());
            let content = state.lock().await.project_fields.as_ref().unwrap()["content"]
                .as_str()
                .unwrap()
                .to_owned();
            assert!(content.contains("Keep the introduction."));
            assert!(content.contains("## Notes\nKeep these notes."));
            let parsed = crate::pm::parse_project_content(&content).unwrap();
            assert_eq!(parsed.workflow, "code");
            assert!(parsed.krs[0].holds);
            assert_eq!(parsed.krs[0].text, "Keep working offline");
            for assignee in [Some("person".to_string()), None] {
                let saved = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
                fixture
                    .store
                    .sqlite
                    .edit_task(
                        &task.id,
                        saved.plan.revision,
                        &crate::pm::PmItemUpdate {
                            assignee: Some(assignee.clone()),
                            ..Default::default()
                        },
                    )
                    .unwrap();
                crate::ops::planning_delivery::sync_fields(
                    &fixture.store,
                    repo,
                    &crate::durable::WorkRef::Task(task.id.clone()),
                )
                .await
                .unwrap();
                assert!(fixture
                    .store
                    .sqlite
                    .pending_task_changes(&task.id)
                    .unwrap()
                    .is_empty());
                assert_eq!(
                    state.lock().await.issues[0]["assignee"],
                    assignee.map_or(json!(null), |id| json!({"id":id}))
                );
            }
        });
    });
}

#[test]
fn planning_fields_uncertain_attempt_does_not_rewrite_an_unchanged_provider() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            let work = save_field_name(fixture, task, false, "Unconfirmed save");
            let receipt = field_receipts(fixture, task, false)[0].clone();
            state.lock().await.field_reject_write = true;
            crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                .await
                .unwrap();
            state.lock().await.field_reject_write = false;
            crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                .await
                .unwrap();
            assert_eq!(field_receipts(fixture, task, false), vec![receipt]);
            assert_eq!(state.lock().await.field_writes, 1);
            assert_eq!(state.lock().await.issues[0]["title"], "Deliver locally");
            assert_eq!(
                fixture
                    .store
                    .sqlite
                    .task(&task.id)
                    .unwrap()
                    .unwrap()
                    .plan
                    .title,
                "Unconfirmed save"
            );
        });
    });
}

#[test]
fn planning_fields_deliver_project_activation_and_task_membership_without_execution() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            let project_work = crate::durable::WorkRef::Project(task.project_id.clone());
            let mut provider = planning_project("project-1", "project-1");
            provider["status"] = json!({"type":"planned"});
            mark_issue_updated(&mut provider);
            state.lock().await.project_fields = Some(provider);
            crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &project_work)
                .await
                .unwrap();
            fixture
                .store
                .sqlite
                .ensure_project(&task.wave_id, "Chapter")
                .unwrap();
            crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &project_work)
                .await
                .unwrap();
            assert!(fixture
                .store
                .sqlite
                .pending_project_changes(&task.project_id)
                .unwrap()
                .is_empty());
            assert_eq!(
                state.lock().await.project_fields.as_ref().unwrap()["status"]["type"],
                "started"
            );
            let mut destination = fixture
                .store
                .sqlite
                .planning_project(&task.project_id)
                .unwrap();
            destination.id = "project-2".into();
            destination.name = "Next chapter".into();
            destination.slug = "next-chapter".into();
            destination.revision = Some("2026-09-30T12:00:00Z".into());
            state
                .lock()
                .await
                .extra_projects
                .push(planning_project("project-2", "project-2"));
            fixture
                .store
                .sqlite
                .put_pm_project(&task.wave_id, "linear", "initiative-1", &destination, now())
                .unwrap();
            let destination = fixture
                .store
                .get_project_by_project("project-2")
                .await
                .unwrap()
                .unwrap();
            crate::ops::chapter::rotate(
                repo,
                &crate::ops::chapter::ChapterPlan {
                    name: "Next chapter".into(),
                    waves: vec![crate::ops::chapter::WaveChapterPlan {
                        wave_id: task.wave_id.clone(),
                        successor_id: destination.id.to_string(),
                        create: false,
                        project_name: "Next chapter".into(),
                        content: crate::pm::ProjectContent {
                            workflow: "feature".into(),
                            metric_targets: Vec::new(),
                            krs: vec![crate::pm::PmKr {
                                text: "Retain Task identity".into(),
                                holds: false,
                            }],
                        },
                    }],
                },
                None,
                false,
            )
            .await
            .unwrap();
            fixture
                .store
                .sqlite
                .refile_unplaced_task(&task.id, &task.project_id, &destination.id)
                .unwrap();
            crate::ops::planning_delivery::sync_fields(
                &fixture.store,
                repo,
                &crate::durable::WorkRef::Task(task.id.clone()),
            )
            .await
            .unwrap();
            assert!(fixture
                .store
                .sqlite
                .pending_task_changes(&task.id)
                .unwrap()
                .is_empty());
            assert_eq!(state.lock().await.issues[0]["project"]["id"], "project-2");
            let retained = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
            assert_eq!(retained.id, task.id);
            assert_eq!(retained.project_id, destination.id);
            assert!(retained.worktree.is_none());
            assert!(fixture.store.sqlite.workflow(&task.id).unwrap().is_none());
        });
    });
}

fn with_export_plan(
    test: impl FnOnce(
        &tokio::runtime::Runtime,
        &Fixture,
        &Path,
        &Task,
        Arc<tokio::sync::Mutex<PlanningState>>,
    ),
) {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    let (repo, wave) = runtime.block_on(fixture.planning_repo());
    let project = fixture
        .store
        .sqlite
        .ensure_project(wave.id(), "Local chapter")
        .unwrap();
    let task = fixture
        .store
        .sqlite
        .create_task(&crate::planning::NewTask {
            due_date: None,
            id: crate::durable::TaskId::new(),
            project_id: project,
            title: "Same title".into(),
            description: "Saved offline".into(),
        })
        .unwrap();
    runtime.block_on(fixture.seed(now() + 86_400));
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState {
        export_mode: true,
        ..Default::default()
    }));
    let (url, server) = runtime.block_on(serve(state.clone()));
    PM_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        test(&runtime, &fixture, &repo, &task, state)
    });
    server.abort();
}

#[test]
fn planning_export_foreground_reconnect_keeps_distinct_saved_identities() {
    with_export_plan(|runtime, fixture, repo, task, state| {
        let second = fixture
            .store
            .sqlite
            .create_task(&crate::planning::NewTask {
                due_date: None,
                id: crate::durable::TaskId::new(),
                project_id: task.project_id.clone(),
                title: task.plan.title.clone(),
                description: task.plan.description.clone(),
            })
            .unwrap();
        state.blocking_lock().field_outage = true;
        let sync =
            crate::ops::linear_observe::PlanningSync::start(fixture.store.clone(), task.clone())
                .unwrap();
        runtime.block_on(async {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            assert_eq!(state.lock().await.creation_writes, 0);
            state.lock().await.field_outage = false;
            tokio::time::timeout(std::time::Duration::from_secs(12), async {
                loop {
                    if fixture
                        .store
                        .get_task(&second.id)
                        .await
                        .unwrap()
                        .unwrap()
                        .plan
                        .linear_id
                        .is_some()
                        && fixture
                            .store
                            .get_task(&task.id)
                            .await
                            .unwrap()
                            .unwrap()
                            .plan
                            .linear_id
                            .is_some()
                    {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
            })
            .await
            .unwrap();
        });
        drop(sync);
        let tasks = runtime.block_on(fixture.store.list_tasks(None)).unwrap();
        assert_eq!(tasks.len(), 2);
        for saved in tasks {
            assert!(saved.worktree.is_none());
            assert_eq!(saved.plan.title, "Same title");
            assert_eq!(
                saved.plan.linear_id.unwrap().as_str(),
                uuid::Uuid::parse_str(saved.id.as_str().trim_start_matches("task_"))
                    .unwrap()
                    .to_string()
            );
        }
        assert_eq!(state.blocking_lock().creation_writes, 3);
        assert_eq!(state.blocking_lock().attachment_writes, 1);
        assert_eq!(
            fixture
                .store
                .sqlite
                .planning_export_owners(&repo.to_string_lossy())
                .unwrap(),
            vec![]
        );
        let reopened = runtime
            .block_on(crate::store::open_ephemeral_store(
                &crate::store::StorageConfig::sqlite(fixture.database.clone()),
            ))
            .unwrap();
        assert_eq!(
            runtime.block_on(reopened.list_tasks(None)).unwrap().len(),
            2
        );
    });
}

#[test]
fn planning_export_lost_creation_and_attachment_recover_without_duplicate_effects() {
    with_export_plan(|runtime, fixture, repo, task, state| {
        let project = crate::durable::WorkRef::Project(task.project_id.clone());
        let work = crate::durable::WorkRef::Task(task.id.clone());
        for owner in [&project, &work] {
            state.blocking_lock().lose_creation_reply = true;
            assert!(runtime
                .block_on(crate::ops::planning_export::sync_export(
                    &fixture.store,
                    repo,
                    owner
                ))
                .is_err());
            let writes = state.blocking_lock().creation_writes;
            state.blocking_lock().hide_exports = true;
            assert!(runtime
                .block_on(crate::ops::planning_export::sync_export(
                    &fixture.store,
                    repo,
                    owner
                ))
                .is_err());
            assert_eq!(state.blocking_lock().creation_writes, writes);
            state.blocking_lock().hide_exports = false;
            let reopened = runtime
                .block_on(crate::store::open_ephemeral_store(
                    &crate::store::StorageConfig::sqlite(fixture.database.clone()),
                ))
                .unwrap();
            if owner == &project {
                state.blocking_lock().lose_attachment_reply = true;
                assert!(runtime
                    .block_on(crate::ops::planning_export::sync_export(
                        &reopened, repo, owner
                    ))
                    .is_err());
            }
            runtime
                .block_on(crate::ops::planning_export::sync_export(
                    &reopened, repo, owner,
                ))
                .unwrap();
            runtime
                .block_on(crate::ops::planning_export::sync_export(
                    &reopened, repo, owner,
                ))
                .unwrap();
        }
        assert_eq!(state.blocking_lock().creation_writes, 2);
        assert_eq!(state.blocking_lock().attachment_writes, 1);
        assert_eq!(
            runtime
                .block_on(fixture.store.list_tasks(None))
                .unwrap()
                .len(),
            1
        );
        assert!(runtime
            .block_on(fixture.store.get_task(&task.id))
            .unwrap()
            .unwrap()
            .plan
            .linear_id
            .is_some());
        let saved = runtime
            .block_on(fixture.store.get_task(&task.id))
            .unwrap()
            .unwrap();
        fixture
            .store
            .sqlite
            .edit_task(
                &task.id,
                saved.plan.revision,
                &crate::pm::PmItemUpdate {
                    name: Some("After export".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        runtime
            .block_on(crate::ops::planning_delivery::sync_fields(
                &fixture.store,
                repo,
                &work,
            ))
            .unwrap();
        assert_eq!(state.blocking_lock().issues[0]["title"], "After export");
        assert!(fixture
            .store
            .sqlite
            .pending_task_changes(&task.id)
            .unwrap()
            .is_empty());
    });
}

#[test]
fn planning_export_late_readback_preserves_newer_edits_and_inventory_identity() {
    with_export_plan(|runtime, fixture, repo, task, state| {
        let project = crate::durable::WorkRef::Project(task.project_id.clone());
        let work = crate::durable::WorkRef::Task(task.id.clone());
        for owner in [&project, &work] {
            let arrived = Arc::new(tokio::sync::Notify::new());
            let release = Arc::new(tokio::sync::Notify::new());
            {
                let mut state = state.blocking_lock();
                state.field_arrived = Some(arrived.clone());
                state.field_release = Some(release.clone());
            }
            runtime.block_on(async {
                let save = async {
                    arrived.notified().await;
                    match owner {
                        crate::durable::WorkRef::Project(id) => fixture
                            .store
                            .sqlite
                            .edit_project(id, Some("Later chapter"), None)
                            .unwrap(),
                        crate::durable::WorkRef::Task(id) => {
                            let saved = fixture.store.get_task(id).await.unwrap().unwrap();
                            fixture
                                .store
                                .sqlite
                                .edit_task(
                                    id,
                                    saved.plan.revision,
                                    &crate::pm::PmItemUpdate {
                                        name: Some("Later task".into()),
                                        ..Default::default()
                                    },
                                )
                                .unwrap();
                            // Inventory observes the committed creation before its response returns.
                            super::load_show_snapshot(
                                repo,
                                "product",
                                PmRefresh::Force,
                                &NullProgress,
                            )
                            .await
                            .unwrap();
                        }
                        _ => unreachable!(),
                    }
                    release.notify_one();
                };
                let (result, ()) = tokio::join!(
                    crate::ops::planning_export::sync_export(&fixture.store, repo, owner),
                    save
                );
                result.unwrap();
            });
        }
        assert_eq!(
            fixture
                .store
                .sqlite
                .planning_project(&task.project_id)
                .unwrap()
                .name,
            "Later chapter"
        );
        assert_eq!(
            fixture
                .store
                .sqlite
                .planning_task(&task.id)
                .unwrap()
                .record
                .unwrap()
                .item
                .name,
            "Later task"
        );
        assert_eq!(
            runtime
                .block_on(fixture.store.list_tasks(None))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            fixture
                .store
                .sqlite
                .pending_project_changes(&task.project_id)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            fixture
                .store
                .sqlite
                .pending_task_changes(&task.id)
                .unwrap()
                .len(),
            1
        );
        // A later Linear edit conflicts with the captured baseline and wins.
        runtime.block_on(async {
            {
                let mut state = state.lock().await;
                state.issues[0]["title"] = json!("Linear decision");
                mark_issue_updated(&mut state.issues[0]);
            }
            crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                .await
                .unwrap();
        });
        assert_eq!(
            fixture
                .store
                .sqlite
                .planning_task(&task.id)
                .unwrap()
                .record
                .unwrap()
                .item
                .name,
            "Linear decision"
        );
        assert!(fixture
            .store
            .sqlite
            .pending_task_changes(&task.id)
            .unwrap()
            .is_empty());
        assert_eq!(state.blocking_lock().creation_writes, 2);
    });
}

#[test]
fn planning_export_removal_during_uncertain_creation_retains_identity() {
    with_export_plan(|runtime, fixture, repo, task, state| {
        let project = crate::durable::WorkRef::Project(task.project_id.clone());
        let work = crate::durable::WorkRef::Task(task.id.clone());
        runtime
            .block_on(crate::ops::planning_export::sync_export(
                &fixture.store,
                repo,
                &project,
            ))
            .unwrap();
        state.blocking_lock().lose_creation_reply = true;
        assert!(runtime
            .block_on(crate::ops::planning_export::sync_export(
                &fixture.store,
                repo,
                &work
            ))
            .is_err());
        fixture.store.sqlite.delete_task(&task.id).unwrap();
        assert!(fixture
            .store
            .sqlite
            .planning_export_owners(&repo.to_string_lossy())
            .unwrap()
            .contains(&work));
        runtime
            .block_on(crate::ops::planning_export::sync_export(
                &fixture.store,
                repo,
                &work,
            ))
            .unwrap();
        assert_eq!(
            fixture.store.sqlite.planning_task(&task.id).unwrap().state,
            crate::store::PlanningState::Removed
        );
        runtime
            .block_on(crate::ops::planning_delivery::sync_fields(
                &fixture.store,
                repo,
                &work,
            ))
            .unwrap();
        assert!(fixture
            .store
            .sqlite
            .pending_task_changes(&task.id)
            .unwrap()
            .is_empty());
        assert_eq!(state.blocking_lock().issues[0]["trashed"], true);
        assert_eq!(state.blocking_lock().creation_writes, 2);
        assert_eq!(state.blocking_lock().deletion_writes, 1);
        let never_exported = fixture
            .store
            .sqlite
            .create_task(&crate::planning::NewTask {
                due_date: None,
                id: crate::durable::TaskId::new(),
                project_id: task.project_id.clone(),
                title: "Remove offline".into(),
                description: String::new(),
            })
            .unwrap();
        fixture
            .store
            .sqlite
            .delete_task(&never_exported.id)
            .unwrap();
        assert!(fixture
            .store
            .sqlite
            .planning_export_owners(&repo.to_string_lossy())
            .unwrap()
            .is_empty());
    });
}

#[path = "planning_order_tests.rs"]
mod planning_order_tests;
