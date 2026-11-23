//! Task planning operations against stateful, isolated Linear responses.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::response::IntoResponse;
use serde_json::json;

use crate::ops::linear::test_fixture::{now, Fixture, PlanningEnvironment};
use crate::ops::linear::{LinearRefresh, LINEAR_TEST_CONTEXT};
use loopflow_storage::work::task::Task;

pub async fn planning_repo(fixture: &Fixture) -> (PathBuf, crate::work::wave::Wave) {
    let (repo, wave) = fixture.planning_repo().await;
    let project = serde_json::from_value(json!({
        "id": "project-1", "slug": "Project", "name": "Project", "summary": "",
        "metric_targets": [], "workflow": "feature", "status": "started", "krs": [],
        "initiative_ids": ["initiative-1"], "team_ids": ["team-1"]
    }))
    .unwrap();
    loopflow_storage::sqlite::linear_events::accept_linear_project_in(
        &fixture.store.sqlite,
        wave.id(),
        "initiative-1",
        &project,
        now(),
        loopflow_storage::planning_sync::TrackerKind::Linear,
    )
    .unwrap();
    loopflow_storage::sqlite::project_selection::write_project_binding(
        &fixture.store.sqlite,
        wave.id(),
        None,
        "project-1",
        &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
    )
    .unwrap();
    (repo, wave)
}

pub async fn serve(
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
pub struct PlanningState {
    pub requests: usize,
    /// Requests by GraphQL operation name.
    pub operations: BTreeMap<String, usize>,
    /// Added to every answer: a provider taking its time.
    pub response_delay: Option<std::time::Duration>,
    pub rate_limit_query: Option<&'static str>,
    pub wave_summary: String,
    pub reject_wave_update: bool,
    pub wave_updates: usize,
    pub export_mode: bool,
    pub creation_writes: usize,
    pub attachment_writes: usize,
    pub lose_creation_reply: bool,
    pub lose_attachment_reply: bool,
    pub hide_exports: bool,
    pub slow_export_id: Option<String>,
    pub deletion_writes: usize,
    pub lose_deletion_reply: bool,
    pub deletion_unconfirmed: bool,
    pub trash_unavailable: bool,
    pub field_outage: bool,
    pub field_reply_lost: bool,
    pub field_reads_blocked: bool,
    pub field_writes: usize,
    pub field_reject_write: bool,
    pub project_fields: Option<serde_json::Value>,
    pub field_arrived: Option<Arc<tokio::sync::Notify>>,
    pub field_release: Option<Arc<tokio::sync::Notify>>,
    pub issues: Vec<serde_json::Value>,
    pub extra_projects: Vec<serde_json::Value>,
    pub fail_snapshot: bool,
    // Discovery and confirmation under the Wave lock consume two reads before mutation.
    pub fail_issue_read_after: Option<usize>,
    pub fail_completion: bool,
    pub missing_canceled_state: bool,
    pub lose_completion: bool,
    pub reopen_during_completion: bool,
    pub lose_comment: bool,
    pub comment_moves_issue: bool,
    pub slow_team_check: bool,
    pub team_filter: Option<serde_json::Value>,
    pub history_unavailable: bool,
    pub completion_writes: usize,
    pub current_project_id: Option<String>,
    pub initial_project_id: Option<String>,
    pub completion_state: Option<String>,
    pub comments: Vec<serde_json::Value>,
    pub attachments: Vec<String>,
    pub move_on_attachment_read: bool,
}

/// Every issue edit enters its Linear history with the state it left behind.
pub fn mark_issue_updated(issue: &mut serde_json::Value) {
    issue["updatedAt"] = json!(time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap());
    if !issue["state"].is_null() {
        let entry = json!({"updatedAt": issue["updatedAt"], "toState": issue["state"]});
        match issue["_history"].as_array_mut() {
            Some(history) => history.push(entry),
            None => issue["_history"] = json!([entry]),
        }
    }
}

pub fn planning_project(id: &str, current: &str) -> serde_json::Value {
    let completed = id == "prior-project" || (id == "project-1" && current == "project-2");
    let name = match id {
        "project-1" => "Project",
        "prior-project" => "Previous project",
        _ => "Next project",
    };
    json!({"id":id, "name":name, "description":"", "content":"workflow: feature",
        "status":{"type":if completed { "completed" } else { "started" }},
        "updatedAt":if completed { "2026-09-30T12:00:01Z" } else { "2026-09-30T12:00:00Z" },
        "archivedAt":null,
        "initiatives":{"nodes":[{"id":"initiative-1"}]}, "teams":{"nodes":[{"id":"team-1"}]}})
}

pub async fn planning_graphql(
    axum::extract::State(state): axum::extract::State<Arc<tokio::sync::Mutex<PlanningState>>>,
    axum::Json(request): axum::Json<serde_json::Value>,
) -> axum::response::Response {
    {
        let mut provider = state.lock().await;
        provider.requests += 1;
        *provider
            .operations
            .entry(operation_name(request["query"].as_str().unwrap()))
            .or_default() += 1;
        if provider
            .rate_limit_query
            .is_some_and(|query| request["query"].as_str().unwrap().contains(query))
        {
            provider.rate_limit_query = None;
            return (axum::http::StatusCode::BAD_REQUEST,
                [("x-ratelimit-requests-reset", ((time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) + 2000).to_string())],
                axum::Json(json!({"errors":[{"message":"Rate limit exceeded", "extensions":{"code":"RATELIMITED"}}]}))).into_response();
        }
    }
    let slow_export = {
        let provider = state.lock().await;
        request["query"]
            .as_str()
            .unwrap()
            .contains("query FindExportIssue")
            && provider.slow_export_id.as_deref() == request["variables"]["id"].as_str()
    };
    let slow_team_check = request["query"]
        .as_str()
        .unwrap()
        .contains("query TeamChanges")
        && state.lock().await.slow_team_check;
    if slow_export || slow_team_check {
        tokio::time::sleep(std::time::Duration::from_secs(6)).await;
    }
    let delay = state.lock().await.response_delay;
    if let Some(delay) = delay {
        tokio::time::sleep(delay).await;
    }
    planning_response(axum::extract::State(state), axum::Json(request))
        .await
        .into_response()
}

impl PlanningState {
    /// Requests sent for one operation since `operations` was last cleared.
    pub fn sent(&self, operation: &str) -> usize {
        self.operations.get(operation).copied().unwrap_or_default()
    }
}

/// The operation name a request log records, such as `FindExportIssue`.
pub fn operation_name(query: &str) -> String {
    query
        .split(|c: char| c.is_whitespace() || c == '(' || c == '{')
        .filter(|word| !word.is_empty())
        .nth(1)
        .unwrap_or("anonymous")
        .to_owned()
}

pub async fn planning_response(
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
    if query.contains("query TeamChanges") {
        state.team_filter = Some(vars["issues"].clone());
    }
    let full_issue = |issue: &serde_json::Value| {
        let mut issue = issue.clone();
        if !issue["project"].is_null() {
            issue["project"] = state
                .extra_projects
                .iter()
                .find(|p| p["id"] == issue["project"]["id"])
                .cloned()
                .unwrap_or_else(|| {
                    planning_project(issue["project"]["id"].as_str().unwrap(), &project_id)
                });
        }
        issue
    };
    let data = if query.contains("query TeamChanges") {
        let eligible = |issue: &serde_json::Value| {
            let filter = &vars["issues"];
            let finished = filter["id"]["nin"].as_array().unwrap();
            let live = filter["or"][0]["id"]["in"].as_array().unwrap();
            !finished.contains(&issue["id"])
                && (live.contains(&issue["id"])
                    || !matches!(
                        issue["state"]["type"].as_str(),
                        Some("completed" | "canceled")
                    ))
        };
        let recent = |value: &serde_json::Value, filter: &serde_json::Value| {
            let revision = value["updatedAt"].as_str().unwrap_or("");
            filter["gte"].as_str().is_none_or(|since| revision >= since)
                && filter["lte"].as_str().is_none_or(|until| revision <= until)
        };
        let issues: Vec<_> = state
            .issues
            .iter()
            .filter(|issue| eligible(issue) && recent(issue, &vars["issues"]["updatedAt"]))
            .map(full_issue)
            .collect();
        let comments: Vec<_> = state
            .comments
            .iter()
            .filter_map(|comment| {
                let id = comment["issue"]["id"].as_str().unwrap_or("issue-1");
                let issue = state.issues.iter().find(|issue| issue["id"] == id)?;
                if !eligible(issue) || !recent(comment, &vars["comments"]["updatedAt"]) {
                    return None;
                }
                let mut comment = comment.clone();
                comment["issue"] = full_issue(issue);
                Some(comment)
            })
            .collect();
        json!({"issues":page(issues),"comments":page(comments)})
    } else if query.contains("query ListTeams") {
        json!({"teams":{"nodes":[{"id":"team-1","name":"Fixture","key":"FIX",
            "description":"<!-- loopflow-repository: loopflowstudio/fixture -->"}]}})
    } else if query.contains("mutation UpdateInitiative") {
        state.wave_updates += 1;
        if state.reject_wave_update {
            return axum::Json(json!({"errors":[{"message":"summary update refused"}]}));
        }
        if let Some(summary) = vars["input"]["description"].as_str() {
            state.wave_summary = summary.into();
        }
        json!({"initiativeUpdate":{"success":true,"initiative":{"id":vars["id"]}}})
    } else if query.contains("query ListInitiatives") {
        json!({"initiatives":page(vec![json!({"id":"initiative-1", "name":"Product", "description":state.wave_summary})])})
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
        let input = &vars["input"];
        let is_project = query.contains("DeliverProjectCreation");
        // Linear keeps client-supplied ids unique: a re-sent creation whose first
        // send did land is refused, never duplicated.
        let exists = if is_project {
            state.extra_projects.iter().any(|p| p["id"] == input["id"])
        } else {
            state.issues.iter().any(|i| i["id"] == input["id"])
        };
        if exists {
            return axum::Json(
                json!({"errors":[{"message":"an entity with this id already exists"}]}),
            );
        }
        state.creation_writes += 1;
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
                "branchName":null,"completedAt":null,"dueDate":input["dueDate"],"trashed":null,"parent":null,"prioritySortOrder":0.0,"sortOrder":0.0,"state":{"type":input["stateId"]},
                "assignee":null,"team":{"id":input["teamId"]},"project":{"id":input["projectId"],"name":"Local project"}});
            if input["projectId"].is_null() {
                issue["project"] = json!(null);
            }
            if !input["parentId"].is_null() {
                issue["parent"] = json!({"id": input["parentId"]});
            }
            mark_issue_updated(&mut issue);
            state.issues.push(issue);
        }
        let created = if is_project {
            state.extra_projects.last().unwrap().clone()
        } else {
            let mut issue = state.issues.last().unwrap().clone();
            if !issue["project"].is_null() {
                let id = issue["project"]["id"].as_str().unwrap().to_owned();
                issue["project"] = state
                    .extra_projects
                    .iter()
                    .find(|p| p["id"] == id)
                    .cloned()
                    .unwrap_or_else(|| planning_project(&id, "project-1"));
            }
            issue
        };
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
            json!({"data":{"projectCreate":{"success":true,"project":created}}})
        } else {
            json!({"data":{"issueCreate":{"success":true,"issue":created}}})
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
        let attached = project.clone();
        if std::mem::take(&mut state.lose_attachment_reply) {
            return axum::Json(json!({"errors":[{"message":"attachment reply lost"}]}));
        }
        json!({"initiativeToProjectCreate":{"success":true,"initiativeToProject":{"project":attached}}})
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
                "projectId" => {
                    target["project"] = if value.is_null() {
                        json!(null)
                    } else {
                        json!({"id":value,"name":"Project"})
                    }
                }
                _ => target[field] = value.clone(),
            }
        }
        mark_issue_updated(target);
        let mut confirmed = target.clone();
        if !is_project && !confirmed["project"].is_null() {
            confirmed["project"] = state
                .extra_projects
                .iter()
                .find(|p| p["id"] == confirmed["project"]["id"])
                .cloned()
                .unwrap_or_else(|| {
                    planning_project(confirmed["project"]["id"].as_str().unwrap(), &project_id)
                });
        }
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
            json!({"data":{"projectUpdate":{"success":true,"project":confirmed}}})
        } else {
            json!({"data":{"issueUpdate":{"success":true,"issue":confirmed}}})
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
        if !issue["project"].is_null() {
            issue["project"] = state
                .extra_projects
                .iter()
                .find(|p| p["id"] == issue["project"]["id"])
                .cloned()
                .unwrap_or(project);
        }
        json!({"issue":issue})
    } else if query.contains("query IssueStateHistory") {
        if state.history_unavailable {
            return axum::Json(json!({"errors":[{"message":"history unavailable"}]}));
        }
        let issue = state.issues.iter().find(|issue| issue["id"] == vars["id"]);
        json!({"issue":issue.map(|issue| json!({"history":page(
            issue["_history"].as_array().cloned().unwrap_or_default()
        )}))})
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
        state.issues[0]["state"] = json!({"type":vars["stateId"]});
        mark_issue_updated(&mut state.issues[0]);
        if let Some(outcome) = state.completion_state.take() {
            // A separate Linear edit after our mutation, visible to readback.
            state.issues[0]["state"] = json!({"type":outcome});
            mark_issue_updated(&mut state.issues[0]);
        }
        if state.lose_completion {
            state.lose_completion = false;
            return axum::Json(json!({"errors":[{"message":"lost completion response"}]}));
        }
        let mut confirmed = state.issues[0].clone();
        confirmed["project"] = project;
        json!({"issueUpdate":{"success":true,"issue":confirmed}})
    } else if query.contains("query IssueAttachments") {
        if state.move_on_attachment_read {
            state.move_on_attachment_read = false;
            state.current_project_id = Some(initial_project_id.clone());
            state.issues[0]["project"]["id"] = json!(initial_project_id);
            mark_issue_updated(&mut state.issues[0]);
        }
        json!({"issue":{"attachments":page(state.attachments.iter().map(|url| json!({"id":format!("att-{url}"),"url":url})).collect::<Vec<_>>())}})
    } else if query.contains("query IssueEvent") {
        let mut issue = state
            .issues
            .iter()
            .find(|issue| issue["id"] == vars["id"])
            .unwrap()
            .clone();
        issue["comments"] = page(
            state
                .comments
                .iter()
                .filter(|comment| {
                    // Unqualified fixture comments belong to the initial issue,
                    // never to every issue a repository-wide reader discovers.
                    comment["issue"]["id"].as_str().unwrap_or("issue-1")
                        == vars["id"].as_str().unwrap()
                })
                .cloned()
                .collect::<Vec<_>>(),
        );
        json!({"issue":issue})
    } else if query.contains("query IssueComments") {
        json!({"issue":{"comments":page(state.comments.iter().filter(|comment| {
            comment["issue"]["id"].as_str().unwrap_or("issue-1") == vars["id"].as_str().unwrap()
        }).cloned().collect::<Vec<_>>())}})
    } else if query.contains("query CommentDelivery") {
        let mut comment = state
            .comments
            .iter()
            .find(|c| c["id"] == vars["id"])
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        if !comment.is_null() && comment["issue"].is_null() {
            comment["issue"] = json!({"id":"issue-1"});
        }
        json!({"comment":comment})
    } else if query.contains("mutation SyncComment") {
        let id = vars["id"].as_str().unwrap();
        if uuid::Uuid::parse_str(id).unwrap().get_version_num() != 4 {
            return axum::Json(json!({"errors":[{"message":"id must be a UUID."}]}));
        }
        if state.comments.iter().any(|c| c["id"] == id) {
            return axum::Json(json!({"errors":[{"message":"ID already exists"}]}));
        }
        state.comments.push(
            json!({"id":id,"body":vars["body"],"user":null,"issue":{"id":vars["issueId"]},
            "createdAt":"2026-10-08T12:00:01Z","updatedAt":"2026-10-08T12:00:01Z"}),
        );
        if state.comment_moves_issue {
            let issue = state
                .issues
                .iter_mut()
                .find(|issue| issue["id"] == vars["issueId"])
                .unwrap();
            mark_issue_updated(issue);
        }
        if std::mem::take(&mut state.lose_comment) {
            return axum::Json(json!({"errors":[{"message":"lost comment response"}]}));
        }
        json!({"commentCreate":{"comment":{"id":id}}})
    } else if query.contains("mutation CreateComment") {
        let id = format!("comment-{}", state.comments.len() + 1);
        state
            .comments
            .push(json!({"id":id,"body":vars["body"],"user":null,"issue":{"id":vars["issueId"]}}));
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

pub fn seed_provider_task(
    runtime: &tokio::runtime::Runtime,
    state: &Arc<tokio::sync::Mutex<PlanningState>>,
    repo: &Path,
    title: &str,
    description: &str,
) -> crate::ops::OpsResult<loopflow_storage::planning::TrackerItem> {
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
            "parent":null,"prioritySortOrder":0.0,"sortOrder":index as f64,"assignee":null,"trashed":null,
            "updatedAt":"2026-09-29T12:00:00.123Z","state":{"type":"unstarted"},
            "team":{"id":"team-1"},"project":{"id":project,"name":"Project"}
        }));
        drop(state);
        crate::ops::linear::refresh_wave(repo, "product").await?;
        Ok(
            crate::ops::linear::test_fixture::refresh_task_input(repo, &id, LinearRefresh::Force)
                .await?
                .item,
        )
    })
}

pub fn with_planning_task(
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
    LINEAR_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
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
