//! Task planning operations against stateful, isolated Linear responses.

#[cfg(test)]
use crate::test_linear_input::SeedLinearSqlite;
use std::cell::RefCell;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::response::IntoResponse;
use serde_json::json;

use super::test_fixture::{now, Fixture, PlanningEnvironment};
use super::{LinearRefresh, LINEAR_TEST_CONTEXT};
use crate::durable::WorkStatus;
use crate::ops::NullProgress;
use crate::work::edit::WorkValue;
use crate::work::task::{
    GithubPr, LinearWritebackState, Observation, PrMergeMode, PrMergeRequest, PrPhase,
    PrPresentation, PrPublication, Task, TaskEventKind, TaskPr, TaskPrId,
};

async fn planning_repo(fixture: &Fixture) -> (PathBuf, crate::work::wave::Wave) {
    let (repo, wave) = fixture.planning_repo().await;
    let project = serde_json::from_value(json!({
        "id": "project-1", "slug": "Chapter", "name": "Chapter", "summary": "",
        "metric_targets": [], "workflow": "feature", "status": "started", "krs": [],
        "initiative_ids": ["initiative-1"], "team_ids": ["team-1"]
    }))
    .unwrap();
    crate::linear::acceptance::accept_linear_project_in(
        &fixture.store.sqlite,
        wave.id(),
        "initiative-1",
        &project,
        now(),
    )
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
pub(crate) struct PlanningState {
    pub(crate) requests: usize,
    /// TeamChanges deltas among `requests`.
    pub(crate) team_changes: usize,
    rate_limit_query: Option<&'static str>,
    wave_summary: String,
    reject_wave_update: bool,
    wave_updates: usize,
    export_mode: bool,
    creation_writes: usize,
    attachment_writes: usize,
    lose_creation_reply: bool,
    lose_attachment_reply: bool,
    hide_exports: bool,
    slow_export_id: Option<String>,
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
    pub(crate) issues: Vec<serde_json::Value>,
    extra_projects: Vec<serde_json::Value>,
    fail_snapshot: bool,
    // Discovery and confirmation under the Wave lock consume two reads before mutation.
    fail_issue_read_after: Option<usize>,
    fail_completion: bool,
    missing_canceled_state: bool,
    lose_completion: bool,
    reopen_during_completion: bool,
    lose_comment: bool,
    comment_moves_issue: bool,
    slow_team_check: bool,
    history_unavailable: bool,
    completion_writes: usize,
    current_project_id: Option<String>,
    initial_project_id: Option<String>,
    completion_state: Option<String>,
    comments: Vec<serde_json::Value>,
    attachments: Vec<String>,
    move_on_attachment_read: bool,
}

/// Every issue edit enters its Linear history with the state it left behind.
pub(crate) fn mark_issue_updated(issue: &mut serde_json::Value) {
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
) -> axum::response::Response {
    {
        let mut provider = state.lock().await;
        provider.requests += 1;
        if request["query"]
            .as_str()
            .unwrap()
            .contains("query TeamChanges")
        {
            provider.team_changes += 1;
        }
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
    planning_response(axum::extract::State(state), axum::Json(request))
        .await
        .into_response()
}

async fn planning_response(
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
            if input["projectId"].is_null() {
                issue["project"] = json!(null);
            }
            mark_issue_updated(&mut issue);
            state.issues.push(issue);
        }
        let created = if is_project {
            state.extra_projects.last().unwrap().clone()
        } else {
            let mut issue = state.issues.last().unwrap().clone();
            if !issue["project"].is_null() {
                issue["project"] = state
                    .extra_projects
                    .iter()
                    .find(|p| p["id"] == issue["project"]["id"])
                    .unwrap()
                    .clone();
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
                        json!({"id":value,"name":"Chapter"})
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
        json!({"issue":{"attachments":page(state.attachments.iter().map(|url| json!({"url":url})).collect::<Vec<_>>())}})
    } else if query.contains("query LinearIssueObservation")
        || query.contains("query IssueObservation")
    {
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

fn seed_provider_task(
    runtime: &tokio::runtime::Runtime,
    state: &Arc<tokio::sync::Mutex<PlanningState>>,
    repo: &Path,
    title: &str,
    description: &str,
) -> crate::ops::OpsResult<crate::linear::LinearIssue> {
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
        super::refresh_wave(repo, "product").await?;
        Ok(
            super::test_fixture::refresh_task_input(repo, &id, LinearRefresh::Force)
                .await?
                .item,
        )
    })
}

pub(crate) fn with_planning_task(
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

#[test]
fn task_deletion_lost_reply_requires_positive_trash_evidence_without_replay() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async { state.lock().await.field_outage = true });
        crate::ops::task::task_delete(repo, task.id.as_str()).unwrap();
        runtime.block_on(async { state.lock().await.field_outage = false });
        let receipt = fixture
            .store
            .sqlite
            .pending_task_deletion(&task.id)
            .unwrap()
            .unwrap();
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
                fixture
                    .store
                    .sqlite
                    .pending_task_deletion(&task.id)
                    .unwrap()
                    .unwrap()
                    .id,
                receipt.id
            );
            state.lock().await.trash_unavailable = false;
            crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                .await
                .unwrap();
            crate::ops::planning_delivery::sync_repository_fields(
                &fixture.store,
                repo.to_str().unwrap(),
            )
            .await
            .unwrap();
            assert_eq!(state.lock().await.deletion_writes, 1);
        });
        assert!(fixture
            .store
            .sqlite
            .pending_task_deletion(&task.id)
            .unwrap()
            .is_none());
    });
}

#[test]
fn task_deletion_concurrent_delivery_has_one_effect() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async { state.lock().await.field_outage = true });
        crate::ops::task::task_delete(repo, task.id.as_str()).unwrap();
        runtime.block_on(async { state.lock().await.field_outage = false });
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
            .pending_task_deletion(&task.id)
            .unwrap()
            .is_none());
    });
}

#[test]
fn task_deletion_old_trash_observation_cannot_settle_newer_removal() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async { state.lock().await.field_outage = true });
        crate::ops::task::task_delete(repo, task.id.as_str()).unwrap();
        runtime.block_on(async { state.lock().await.field_outage = false });
        let receipt = fixture
            .store
            .sqlite
            .pending_task_deletion(&task.id)
            .unwrap()
            .unwrap();
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
            fixture
                .store
                .sqlite
                .pending_task_deletion(&task.id)
                .unwrap()
                .unwrap()
                .id,
            receipt.id
        );
    });
}

#[test]
fn task_deletion_unconfirmed_write_stays_uncertain_without_replay() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async { state.lock().await.field_outage = true });
        crate::ops::task::task_delete(repo, task.id.as_str()).unwrap();
        runtime.block_on(async { state.lock().await.field_outage = false });
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
        assert!(fixture
            .store
            .sqlite
            .pending_task_deletion(&task.id)
            .unwrap()
            .is_some());
        assert!(fixture.store.sqlite.list_tasks(None).unwrap().is_empty());
    });
}

#[test]
fn task_deletion_adopts_newer_linear_edit_and_retains_losing_removal() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async { state.lock().await.field_outage = true });
        crate::ops::task::task_delete(repo, task.id.as_str()).unwrap();
        runtime.block_on(async { state.lock().await.field_outage = false });
        let receipt = fixture
            .store
            .sqlite
            .pending_task_deletion(&task.id)
            .unwrap()
            .unwrap();
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
            fixture.store.sqlite.list_tasks(None).unwrap()[0].title,
            "Keep this work"
        );
        assert!(fixture
            .store
            .sqlite
            .pending_task_deletion(&task.id)
            .unwrap()
            .is_none());
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
                    [task.project_id.as_ref().unwrap().as_str()],
                )
                .unwrap();
            }
            let original = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
            let history = runtime
                .block_on(fixture.store.task_events_after(&task.id, 0))
                .unwrap();
            // Every provider operation would fail; the existing Task needs none.
            LINEAR_TEST_CONTEXT.sync_scope(fixture.context("http://127.0.0.1:1"), || {
                assert_eq!(crate::ops::task::task_abandon(repo, Some(task.id.as_str()), false).unwrap(), task.identifier);
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
                let mut expected = original.clone();
                expected.revision += 1;
                assert_eq!(retained.revision, expected.revision);
assert_eq!(retained.linear_id, expected.linear_id);
assert_eq!(retained.identifier, expected.identifier);
assert_eq!(retained.title, expected.title);
assert_eq!(retained.description, expected.description);
assert_eq!(retained.pm_snapshot_synced_at, expected.pm_snapshot_synced_at);
                assert!(retained.worktree.is_none());
                assert!(fixture.store.sqlite.task_graph(&task.id).unwrap().is_none());
                assert_eq!(runtime.block_on(fixture.store.task_events_after(&task.id, 0)).unwrap(), history);
                if connected {
                    assert!(matches!(retained.pm_writeback, LinearWritebackState::Pending {
                        operation: crate::work::task::LinearWritebackOperation::CancelTask, ..
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
fn task_abandonment_lost_reply_reconciles_without_repeating_cancellation() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async { state.lock().await.field_outage = true });
        crate::ops::task::task_abandon(repo, Some(task.id.as_str()), false).unwrap();
        runtime.block_on(async { state.lock().await.field_outage = false });
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
        runtime.block_on(async { state.lock().await.field_outage = true });
        crate::ops::task::task_abandon(repo, Some(task.id.as_str()), false).unwrap();
        runtime.block_on(async { state.lock().await.field_outage = false });
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
                .task(&task.id)
                .unwrap()
                .unwrap()
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
        runtime.block_on(async { state.lock().await.field_outage = true });
        crate::ops::task::task_abandon(repo, Some(task.id.as_str()), false).unwrap();
        runtime.block_on(async { state.lock().await.field_outage = false });
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
        assert!(fixture.store.sqlite.task_graph(&task.id).unwrap().is_none());
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
            .is_none());
        assert_eq!(
            runtime.block_on(async { state.lock().await.completion_writes }),
            1
        );
    });
}

#[test]
fn task_completion_ingestion_preserves_baseline_and_atomically_adopts_linear() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        // Completion attempts foreground delivery; keep the receipt pending for ingestion.
        runtime.block_on(async { state.lock().await.field_outage = true });
        let mut remote = fixture.retained_task(repo, task.linear_id.as_ref().unwrap().as_str());
        crate::ops::task::task_complete(repo, task.id.as_str(), Some("Saved offline")).unwrap();
        let delivery = fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .unwrap();
        let workflow = fixture.store.sqlite.task_graph(&task.id).unwrap();
        // A changed unrelated field does not conflict with the pending state save.
        remote.item.name = "Linear title".into();
        remote.item.revision = Some("2026-10-08T01:00:00Z".into());
        remote.observed_at += 1;
        fixture
            .store
            .sqlite
            .seed_issue(&repo.display().to_string(), "linear", &remote, None)
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
        let saved = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
        assert_eq!(saved.state.as_deref(), Some("completed"));
        assert_eq!(saved.title, "Linear title");
        remote.item.state = Some("canceled".into());
        remote.item.revision = Some("2026-10-08T02:00:00Z".into());
        remote.observed_at += 1;
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        conn.execute_batch("CREATE TRIGGER fail_adoption BEFORE INSERT ON task_events BEGIN SELECT RAISE(ABORT,'fixture rollback'); END;").unwrap();
        assert!(fixture
            .store
            .sqlite
            .seed_issue(&repo.display().to_string(), "linear", &remote, None)
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
                .task(&task.id)
                .unwrap()
                .unwrap()
                .state
                .as_deref(),
            Some("completed")
        );
        conn.execute_batch("DROP TRIGGER fail_adoption").unwrap();
        fixture
            .store
            .sqlite
            .seed_issue(&repo.display().to_string(), "linear", &remote, None)
            .unwrap();
        let mut late = remote.item.clone();
        late.state = Some("completed".into());
        late.completed = true;
        assert!(!crate::linear::acceptance::observe_task_state(
            &fixture.store.sqlite,
            &delivery,
            &late,
            crate::linear::StateHistory::Unknown
        )
        .unwrap());
        fixture
            .store
            .sqlite
            .task_state_error(&delivery, "late failure")
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
                .task(&task.id)
                .unwrap()
                .unwrap()
                .state
                .as_deref(),
            Some("canceled")
        );
        assert_eq!(fixture.store.sqlite.task_graph(&task.id).unwrap(), workflow);
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
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            state.lock().await.field_outage = true;
        });
        crate::ops::task::task_complete(repo, task.id.as_str(), Some("Delivered")).unwrap();
        let completed = fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .unwrap();
        assert!(fixture
            .store
            .sqlite
            .attempt_task_state(&completed, false)
            .unwrap());
        let process = crate::process::LfProcess {
            kind: crate::process::ProcessKind::Lf,
            agent_session_id: None,
            os_started_at: None,
            id: crate::id::LfProcessId::new(),
            pid: None,
            trace_id: crate::id::TraceId::new(),
            parent_lf_process_id: None,
            via_agent: None,
            caller_session_id: None,
            caller_agent_process_id: None,
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
            .take_up_task_graph(
                &task.id,
                &crate::task_graph::CapturedGraph {
                    name: "reopen".into(),
                    steps: Vec::new(),
                },
            )
            .unwrap();
        fixture
            .store
            .sqlite
            .set_task_graph_step(&task.id, "start")
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
            .retained_task(repo, task.linear_id.as_ref().unwrap().as_str())
            .item;
        late.state = Some("canceled".into());
        late.revision = Some("2026-10-08T03:00:00Z".into());
        assert!(!crate::linear::acceptance::observe_task_state(
            &fixture.store.sqlite,
            &completed,
            &late,
            crate::linear::StateHistory::Unknown
        )
        .unwrap());
        assert_eq!(
            fixture
                .store
                .sqlite
                .task(&task.id)
                .unwrap()
                .unwrap()
                .state
                .as_deref(),
            Some("unstarted")
        );
        assert_eq!(reopened.target, "unstarted");
        // A superseded attempt stays uncertain. Neither its readback nor its
        // failure can settle the newer reopening or invent successful delivery.
        fixture
            .store
            .sqlite
            .task_state_error(&completed, "late failure")
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
        assert!(!fixture
            .store
            .sqlite
            .attempt_task_state(&completed, false)
            .unwrap());
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        let receipt: (bool, Option<String>) = conn
            .query_row(
                "SELECT settled,error FROM task_state_deliveries WHERE id=?1",
                [&completed.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(receipt, (false, Some("late failure".into())));
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
fn native_runner_is_quiet_until_its_turn_ends_codex_foreground() {
    assert_native_runner_is_quiet_until_its_turn_ends("codex", false, true);
}

#[test]
fn native_runner_is_quiet_until_its_turn_ends_claude_foreground() {
    assert_native_runner_is_quiet_until_its_turn_ends("claude", false, true);
}

#[test]
fn native_runner_is_quiet_until_its_turn_ends_claude_task_headless() {
    assert_native_runner_is_quiet_until_its_turn_ends("claude", true, true);
}

#[test]
fn native_runner_is_quiet_until_its_turn_ends_claude_taskless_headless() {
    assert_native_runner_is_quiet_until_its_turn_ends("claude", true, false);
}

fn assert_native_runner_is_quiet_until_its_turn_ends(agent: &str, auto: bool, task_bound: bool) {
    with_planning_task(|runtime, fixture, repo, task, state| {
        let _environment = crate::test_ambient::EnvGuard::clear(&[
            "HOME",
            "LF_HOME",
            "LF_BIN",
            "PATH",
            "CODEX_HOME",
            "CLAUDE_CONFIG_DIR",
        ]);
        let home = fixture.directory.path().join("provider-home");
        let bin = fixture.directory.path().join("bin");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        std::env::set_var("HOME", &home);
        std::env::set_var("LF_HOME", fixture.directory.path());
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let path = format!("{}:/usr/bin:/bin", bin.display());
        std::env::set_var("PATH", &path);
        let started = home.join("started");
        let stop = home.join("stop");
        if agent == "codex" {
            crate::test_support::install_native_codex(&bin);
        } else {
            let executable = bin.join(agent);
            std::fs::write(
                &executable,
                include_str!("../../../tests/support/holding_provider.py"),
            )
            .unwrap();
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        runtime.block_on(async {
            seed_finished_tasks(fixture, repo, &state).await;
            state.lock().await.requests = 0;
        });
        let process = crate::agent::ProcessConfig {
            auto,
            task_input: task_bound.then(|| {
                crate::ops::task_input::TaskInput::new(
                    fixture.store.clone(),
                    crate::ops::task_input::TaskSeed {
                        task: task.clone(),
                        message: String::new(),
                        interrupt: 0,
                    },
                )
            }),
            ..Default::default()
        };
        let launch = crate::agent::AgentConfig {
            agent: Some(agent.into()),
            cwd: Some(repo.to_path_buf()),
            env: std::collections::BTreeMap::from([
                ("PATH".into(), path),
                ("FIXTURE_STARTED".into(), started.display().to_string()),
                ("FIXTURE_STOP".into(), stop.display().to_string()),
            ]),
            ..Default::default()
        };
        let context = LINEAR_TEST_CONTEXT.with(Clone::clone);
        let database = fixture.database.clone();
        runtime.block_on(async {
            let running = tokio::task::spawn_blocking(move || {
                crate::journal::with_test_ledger(database, || {
                    LINEAR_TEST_CONTEXT
                        .sync_scope(context, || crate::agent::run_agent(&launch, &process))
                })
            });
            let observed = tokio::time::timeout(std::time::Duration::from_secs(70), async {
                while !started.exists() {
                    assert!(
                        !running.is_finished(),
                        "fixture provider exited before readiness"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
                // Native agent stays open without a watcher. Only the turn
                // boundary may acquire the change made while it was idle.
                tokio::time::sleep(std::time::Duration::from_secs(if agent == "codex" {
                    60
                } else {
                    1
                }))
                .await;
                assert_eq!(state.lock().await.requests, 0);
                let mut provider = state.lock().await;
                provider.issues[0]["title"] = json!("Changed during the turn");
                mark_issue_updated(&mut provider.issues[0]);
            })
            .await;
            std::fs::write(&stop, "stop").unwrap();
            assert_eq!(running.await.unwrap().unwrap().exit_code, 0);
            observed.unwrap();
            // The turn's delta, then the Project its changed issue names.
            assert_eq!(state.lock().await.requests, 2);
            assert_eq!(
                fixture
                    .store
                    .get_task(&task.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .title,
                "Changed during the turn"
            );
        });
    });
}

#[test]
fn task_completion_preserves_linear_reopening_during_delivery() {
    assert_completion_adopts_reopening(true);
}

#[test]
fn task_completion_preserves_linear_reopening_before_delivery() {
    assert_completion_adopts_reopening(false);
}

fn assert_completion_adopts_reopening(during_delivery: bool) {
    with_planning_task(|runtime, fixture, repo, task, state| {
        if !during_delivery {
            runtime.block_on(async {
                let mut provider = state.lock().await;
                provider.issues[0]["state"] = json!({"type":"started"});
                mark_issue_updated(&mut provider.issues[0]);
                drop(provider);
                super::refresh_wave(repo, "product").await.unwrap();
            });
        }
        runtime.block_on(async { state.lock().await.field_outage = true });
        crate::ops::task::task_complete(repo, task.id.as_str(), Some("Delivered locally")).unwrap();
        runtime.block_on(async { state.lock().await.field_outage = false });
        let original = fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .unwrap();
        assert!(!original.attempted);
        let planning = || fixture.store.sqlite.task(&task.id).unwrap().unwrap();
        assert!(
            planning().state.as_deref() == Some("completed"),
            "local completion saves before delivery"
        );
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        let execution = || {
            ["processes", "agent_sessions", "task_prs", "task_graphs"]
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
        let before = execution();
        runtime.block_on(async {
            let mut provider = state.lock().await;
            assert_eq!(provider.completion_writes, 0);
            if during_delivery {
                // Return to the original state, but at a new provider revision
                // after our attempted write: readback must still adopt it.
                provider.completion_state = Some("unstarted".into());
            } else {
                provider.issues[0]["state"] = json!({"type":"unstarted"});
                mark_issue_updated(&mut provider.issues[0]);
            }
            drop(provider);
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            assert!(fixture
                .store
                .sqlite
                .pending_task_state(&task.id)
                .unwrap()
                .is_none());
            // A retired loser must not overwrite Linear on a subsequent connection.
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            let provider = state.lock().await;
            assert_eq!(provider.issues[0]["state"]["type"], "unstarted");
            assert_eq!(provider.completion_writes, usize::from(during_delivery));
        });
        assert_eq!(planning().state.as_deref(), Some("unstarted"));
        assert_eq!(
            execution(),
            before,
            "planning adoption has no execution authority"
        );
        let sync = fixture.store.sqlite.task_planning_sync(&task.id).unwrap();
        let retained = sync
            .changes
            .iter()
            .find(|change| change.id == original.id)
            .unwrap();
        assert_eq!(
            retained.state,
            crate::store::planning_sync::PlanningSyncState::AdoptedLinear
        );
        assert_eq!(retained.local_value, json!("completed"));
        assert_eq!(retained.linear_value, Some(json!("unstarted")));
        assert!(retained.error.is_none());
        assert!(!fixture
            .store
            .sqlite
            .attempt_task_state(&original, false)
            .unwrap());
        let receipt: (bool, bool, String, String) = conn.query_row(
            "SELECT attempted,settled,target,conflict_json FROM task_state_deliveries WHERE id=?1",
            [&original.id],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
        ).unwrap();
        assert_eq!(
            (receipt.0, receipt.1, receipt.2.as_str()),
            (during_delivery, true, "completed")
        );
        let conflict: serde_json::Value = serde_json::from_str(&receipt.3).unwrap();
        assert_eq!(conflict["state"], "unstarted");
        assert!(conflict["revision"].is_string());
        assert_eq!(conflict["completed"], false);
        assert_eq!(conflict.as_object().unwrap().len(), 4);
        assert!(conflict.get("completed_at").is_some());
    });
}

#[test]
fn task_completion_unseen_reopening_can_be_overwritten_by_unconditional_delivery() {
    // Jack Heart selected observed Linear-wins, not unseen-write atomicity.
    // Keep the original counterexample: no ownership read sees this reopening.
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async { state.lock().await.field_outage = true });
        crate::ops::task::task_complete(repo, task.id.as_str(), Some("Delivered locally")).unwrap();
        runtime.block_on(async { state.lock().await.field_outage = false });
        let original = fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .unwrap();
        let workflow = fixture.store.sqlite.task_graph(&task.id).unwrap();
        runtime.block_on(async {
            state.lock().await.reopen_during_completion = true;
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            let provider = state.lock().await;
            assert!(!provider.reopen_during_completion);
            assert_eq!(
                provider.issues[0]["state"]["type"], "completed",
                "the unconditional write overwrites the unobserved reopening"
            );
            assert_eq!(provider.completion_writes, 1);
            drop(provider);
            assert!(fixture
                .store
                .sqlite
                .pending_task_state(&task.id)
                .unwrap()
                .is_none());
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            assert_eq!(state.lock().await.completion_writes, 1);
        });
        assert_eq!(fixture.store.sqlite.task_graph(&task.id).unwrap(), workflow);
        // Matching readback settles only the observed state. It cannot record a
        // conflict it never saw or certify that no intermediate edit was lost.
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        let receipt: (bool, bool, Option<String>) = conn
            .query_row(
                "SELECT attempted,settled,conflict_json FROM task_state_deliveries WHERE id=?1",
                [&original.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(receipt, (true, true, None));
    });
}

#[test]
fn task_completion_lost_reply_adopts_linear_reopening() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            state.lock().await.field_outage = true;
        });
        crate::ops::task::task_complete(repo, task.id.as_str(), Some("Delivered")).unwrap();
        let original = fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .unwrap();
        runtime.block_on(async {
            let mut provider = state.lock().await;
            provider.field_outage = false;
            provider.lose_completion = true;
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
        let adopted = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
        assert_eq!(adopted.state.as_deref(), Some("unstarted"));
        assert!(fixture.store.sqlite.task_graph(&task.id).unwrap().is_none());
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
        // A delayed matching readback cannot erase an already observed collision.
        let mut late = fixture
            .retained_task(repo, task.linear_id.as_ref().unwrap().as_str())
            .item;
        late.state = Some("completed".into());
        late.completed = true;
        assert!(!crate::linear::acceptance::observe_task_state(
            &fixture.store.sqlite,
            &original,
            &late,
            crate::linear::StateHistory::Unknown
        )
        .unwrap());
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

fn adoption_events(conn: &rusqlite::Connection, task: &Task) -> i64 {
    conn.query_row(
        "SELECT count(*) FROM task_events WHERE task_id=?1 AND kind_json LIKE '%Adopted Linear state%'",
        [task.id.as_str()],
        |row| row.get(0),
    )
    .unwrap()
}

/// A completion saved during a Linear outage, with a verification comment queued.
fn complete_during_outage(
    runtime: &tokio::runtime::Runtime,
    fixture: &Fixture,
    repo: &Path,
    task: &Task,
    state: &Arc<tokio::sync::Mutex<PlanningState>>,
) -> crate::store::sqlite::task_state_delivery::TaskStateDelivery {
    runtime.block_on(async { state.lock().await.field_outage = true });
    crate::ops::task::task_complete(repo, task.id.as_str(), Some("Verified")).unwrap();
    crate::ops::task::append_task_comment(&fixture.store, task, "Verified on main", false).unwrap();
    runtime.block_on(async {
        let mut provider = state.lock().await;
        provider.field_outage = false;
        provider.comment_moves_issue = true;
    });
    fixture
        .store
        .sqlite
        .pending_task_state(&task.id)
        .unwrap()
        .unwrap()
}

#[test]
fn task_completion_survives_rate_limit_and_unrelated_comment_revision() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            state.lock().await.rate_limit_query = Some("mutation SetIssueState");
        });
        crate::ops::task::task_complete(repo, task.id.as_str(), Some("Verified")).unwrap();
        crate::ops::task::append_task_comment(&fixture.store, task, "Verified on main", false)
            .unwrap();
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        runtime.block_on(async {
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            assert!(
                !fixture
                    .store
                    .sqlite
                    .pending_task_state(&task.id)
                    .unwrap()
                    .unwrap()
                    .attempted
            );
            // After the reset, the pre-write Linear check outlives the delivery
            // deadline. Nothing was sent, so nothing is uncertain.
            tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
            conn.execute("UPDATE linear_team_checks SET attempted_at=0", [])
                .unwrap();
            state.lock().await.slow_team_check = true;
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            let pending = fixture
                .store
                .sqlite
                .pending_task_state(&task.id)
                .unwrap()
                .unwrap();
            assert!(!pending.attempted);
            let mut provider = state.lock().await;
            assert_eq!(provider.completion_writes, 0);
            provider.slow_team_check = false;
            provider.comment_moves_issue = true;
            drop(provider);
            // The queued comment publishes first and moves the issue revision.
            crate::ops::linear_observe::sync_task_comments(&fixture.store, task)
                .await
                .unwrap();
            crate::ops::linear_observe::sync_repository_deliveries(
                &fixture.store,
                repo.to_str().unwrap(),
            )
            .await
            .unwrap();
            let provider = state.lock().await;
            assert_eq!(provider.issues[0]["state"]["type"], "completed");
            assert_eq!(provider.completion_writes, 1);
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
                .task(&task.id)
                .unwrap()
                .unwrap()
                .state
                .as_deref(),
            Some("completed")
        );
        assert_eq!(adoption_events(&conn, task), 0);
        let graphs: i64 = conn
            .query_row("SELECT count(*) FROM processes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(graphs, 0);
    });
}

#[test]
fn task_completion_uncertain_write_retries_after_unrelated_revision() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        let original = complete_during_outage(runtime, fixture, repo, task, &state);
        runtime.block_on(async {
            // The request is lost before Linear applies it.
            state.lock().await.fail_completion = true;
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            assert!(
                fixture
                    .store
                    .sqlite
                    .pending_task_state(&task.id)
                    .unwrap()
                    .unwrap()
                    .attempted
            );
            crate::ops::linear_observe::sync_task_comments(&fixture.store, task)
                .await
                .unwrap();
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            let provider = state.lock().await;
            assert_eq!(provider.issues[0]["state"]["type"], "completed");
            assert_eq!(provider.completion_writes, 1);
        });
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        let receipt: (bool, bool, Option<String>) = conn
            .query_row(
                "SELECT attempted,settled,conflict_json FROM task_state_deliveries WHERE id=?1",
                [&original.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(receipt, (true, true, None));
        assert_eq!(
            fixture
                .store
                .sqlite
                .task(&task.id)
                .unwrap()
                .unwrap()
                .state
                .as_deref(),
            Some("completed")
        );
        assert_eq!(adoption_events(&conn, task), 0);
    });
}

#[test]
fn task_completion_unknown_history_stays_pending_until_explicit_retry() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        complete_during_outage(runtime, fixture, repo, task, &state);
        runtime.block_on(async {
            state.lock().await.fail_completion = true;
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            crate::ops::linear_observe::sync_task_comments(&fixture.store, task)
                .await
                .unwrap();
            state.lock().await.history_unavailable = true;
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            let pending = fixture
                .store
                .sqlite
                .pending_task_state(&task.id)
                .unwrap()
                .unwrap();
            assert!(pending.attempted);
            assert_eq!(state.lock().await.completion_writes, 0);
            assert_eq!(
                fixture
                    .store
                    .sqlite
                    .task(&task.id)
                    .unwrap()
                    .unwrap()
                    .state
                    .as_deref(),
                Some("completed"),
                "unknown evidence neither adopts nor delivers"
            );
            // A person's explicit retry overrides only unknown evidence.
            crate::ops::linear_observe::deliver_task_state(&fixture.store, task, true)
                .await
                .unwrap();
            assert_eq!(state.lock().await.completion_writes, 1);
        });
        assert!(fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .is_none());
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        assert_eq!(adoption_events(&conn, task), 0);
    });
}

#[test]
fn task_completion_explicit_retry_adopts_reopening_proved_by_history() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        let original = complete_during_outage(runtime, fixture, repo, task, &state);
        runtime.block_on(async {
            // The write lands, its reply is lost, and someone reopens the issue.
            state.lock().await.lose_completion = true;
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            {
                let mut provider = state.lock().await;
                provider.issues[0]["state"] = json!({"type":"unstarted"});
                mark_issue_updated(&mut provider.issues[0]);
            }
            crate::ops::linear_observe::sync_task_comments(&fixture.store, task)
                .await
                .unwrap();
            crate::ops::linear_observe::deliver_task_state(&fixture.store, task, true)
                .await
                .unwrap();
            let provider = state.lock().await;
            assert_eq!(provider.issues[0]["state"]["type"], "unstarted");
            assert_eq!(provider.completion_writes, 1);
        });
        assert_eq!(
            fixture
                .store
                .sqlite
                .task(&task.id)
                .unwrap()
                .unwrap()
                .state
                .as_deref(),
            Some("unstarted")
        );
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        let conflict: String = conn
            .query_row(
                "SELECT conflict_json FROM task_state_deliveries WHERE id=?1 AND settled=1",
                [&original.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(conflict.contains("unstarted"));
        assert_eq!(adoption_events(&conn, task), 1);
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
    LINEAR_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
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
            provider.field_outage = !uncached;
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
            LinearWritebackState::Pending { .. }
        ));
        assert!(pending.worktree.is_none());
        assert_eq!(
            runtime.block_on(fixture.store.work_status(&crate::durable::WorkRef::Task(pending.id.clone()))).unwrap(),
            WorkStatus::Done
        );
        assert_eq!(runtime.block_on(async { state.lock().await.completion_writes }), usize::from(uncached));
        let delivery = fixture.store.sqlite.pending_task_state(&pending.id).unwrap().unwrap();
        runtime.block_on(async { state.lock().await.field_outage = false });
        complete().unwrap();
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        assert_eq!(conn.query_row("SELECT id FROM task_state_deliveries WHERE task_id=?1", [pending.id.as_str()], |r| r.get::<_, String>(0)).unwrap(), delivery.id);
        runtime.block_on(crate::ops::linear_observe::sync_task_state(&fixture.store, &pending)).unwrap();
        runtime.block_on(crate::ops::linear_observe::sync_task_state(&fixture.store, &pending)).unwrap();
        let retained = runtime
            .block_on(fixture.store.get_task(&pending.id))
            .unwrap()
            .unwrap();
        assert_eq!(retained.pm_writeback, LinearWritebackState::Current);
        assert_eq!(runtime.block_on(async { state.lock().await.completion_writes }), 1);
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
    LINEAR_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
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
                repository_id: fixture
                    .store
                    .sqlite
                    .ensure_repository(repo.to_str().unwrap())
                    .unwrap(),
                id: runtime
                    .block_on(fixture.store.get_task_by_issue(&item.id))
                    .unwrap()
                    .unwrap()
                    .id,

                revision: 0,
                linear_id: Some(crate::linear::LinearIssueId::new(&item.id).unwrap()),
                identifier: item.identifier.clone(),
                title: item.name.clone(),
                description: item.description.clone(),
                pm_snapshot_synced_at: Some(1),

                pm_writeback: LinearWritebackState::Current,
                wave_id: Some(wave.id().clone()),
                project_id: Some(project.id),
                worktree: Some(repo.clone()),
                workspace_slug: "completion".into(),
                branch: crate::git::current_branch(&repo).unwrap().unwrap(),
                base_commit: crate::git::rev_parse(&repo, "HEAD").unwrap(),
                parent_pr_id: None,
                created_at: timestamp,
                updated_at: timestamp,
                rank: 0,
                assignee: None,
                due_date: None,
                state: None,
                completed_at: None,
                observation: Observation::NotRequired,
                agent: None,
            };
            let mut pr = TaskPr {
                id: TaskPrId::new(),
                task_id: task.id.clone(),
                sequence: 1,
                slug: "completion".into(),
                branch: crate::git::current_branch(&repo).unwrap().unwrap(),
                base_commit: crate::git::rev_parse(&repo, "HEAD").unwrap(),
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
                // Observe the merge without requesting completion.
                // Provider completion itself is the operation exercised below.
                runtime
                    .block_on(crate::ops::task::reconcile_delivered_task(
                        &fixture.store,
                        &mut task,
                    ))
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

        runtime.block_on(async { state.lock().await.field_outage = true });
        let first = complete("Delivered the requested outcome").unwrap();
        assert!(matches!(
            first.pm_writeback,
            LinearWritebackState::Pending { .. }
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
            provider.field_outage = false;
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
        assert_eq!(result.pm_writeback, LinearWritebackState::Current);
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
            crate::git::worktrees::list_worktrees(&repo).unwrap().len(),
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
        LINEAR_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
            let item =
                seed_provider_task(&runtime, &state, &repo, "Cancel me", "Keep history").unwrap();
            let timestamp = time::OffsetDateTime::now_utc();
            let project = runtime
                .block_on(fixture.store.list_projects(Some(wave.id())))
                .unwrap()
                .into_iter()
                .find(|project| project.linear_id.as_ref().unwrap().as_str() == "project-1")
                .unwrap();
            let task = Task {
                repository_id: fixture
                    .store
                    .sqlite
                    .ensure_repository(repo.to_str().unwrap())
                    .unwrap(),
                id: runtime
                    .block_on(fixture.store.get_task_by_issue(&item.id))
                    .unwrap()
                    .unwrap()
                    .id,

                revision: 0,
                linear_id: Some(crate::linear::LinearIssueId::new(&item.id).unwrap()),
                identifier: item.identifier.clone(),
                title: item.name,
                description: item.description,
                pm_snapshot_synced_at: Some(1),

                pm_writeback: LinearWritebackState::Current,
                wave_id: Some(wave.id().clone()),
                project_id: Some(project.id),
                worktree: Some(checkout.clone()),
                workspace_slug: "cancel-me".into(),
                branch: "cancel-me".into(),
                base_commit: git(&repo, &["rev-parse", "HEAD"]).trim().into(),
                parent_pr_id: None,
                created_at: timestamp,
                updated_at: timestamp,
                rank: 0,
                assignee: None,
                due_date: None,
                state: None,
                completed_at: None,
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
            fixture.store.sqlite.test_graph(
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
                    json!("canceled")
                );
                assert!(matches!(
                    fixture
                        .store
                        .sqlite
                        .task(&task.id)
                        .unwrap()
                        .unwrap()
                        .pm_writeback,
                    LinearWritebackState::Current
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
            // The Graph's Processes remain as history; abandonment retires nothing.
            assert_eq!(fixture.store.sqlite.task_graphs(&task.id).unwrap().len(), 1);
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
    for args in [
        vec!["branch", "-M", "main"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "merged goals",
        ],
    ] {
        assert!(std::process::Command::new("git")
            .args(args)
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());
    }
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
    LINEAR_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
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
            let result = super::refresh(
                &repo,
                &super::LinearSyncOptions {
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
            let projects = fixture.store.list_projects(Some(wave.id())).await.unwrap();
            assert_eq!(projects.len(), 1);
            assert_eq!(
                projects[0].linear_id.as_ref().unwrap().as_str(),
                "project-1"
            );
            assert_eq!(projects[0].name, "Chapter");
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
            json!("canceled")
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
        assert!(fixture
            .store
            .sqlite
            .pending_task_state(&saved.id)
            .unwrap()
            .is_none());
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
    LINEAR_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
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
fn planning_fields_selected_before_deletion_cannot_be_attempted_afterward() {
    with_planning_task(|_runtime, fixture, _repo, task, _state| {
        fixture
            .store
            .sqlite
            .edit_task(
                &task.id,
                task.revision,
                &crate::work::task::TaskUpdate {
                    title: Some("Pending edit".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let change = fixture
            .store
            .sqlite
            .pending_task_changes(&task.id)
            .unwrap()
            .remove(0);
        fixture.store.sqlite.delete_task(&task.id).unwrap();
        assert!(!fixture
            .store
            .sqlite
            .attempt_planning_field(
                crate::store::sqlite::planning_changes::PlanningChanges::Task(&task.id),
                &change,
                change
                    .base
                    .as_ref()
                    .and_then(|base| base.revision.as_deref()),
                false,
            )
            .unwrap());
        assert!(fixture
            .store
            .sqlite
            .pending_task_deletion(&task.id)
            .unwrap()
            .is_some());
    });
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
                assert_eq!(
                    pending[0].value,
                    if project {
                        WorkValue::ProjectName(Some("Newer save".into()))
                    } else {
                        WorkValue::TaskTitle(Some("Newer save".into()))
                    }
                );
                assert_eq!(
                    pending[0]
                        .base
                        .as_ref()
                        .unwrap()
                        .value
                        .as_ref()
                        .unwrap()
                        .value()
                        .unwrap(),
                    "First save"
                );
                if project {
                    assert_eq!(
                        fixture
                            .store
                            .sqlite
                            .project(task.project_id.as_ref().unwrap())
                            .unwrap()
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
            .edit_project(task.project_id.as_ref().unwrap(), Some(name), None, None)
            .unwrap();
        crate::durable::WorkRef::Project(task.project_id.clone().unwrap())
    } else {
        let current = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
        fixture
            .store
            .sqlite
            .edit_task(
                &task.id,
                current.revision,
                &crate::work::task::TaskUpdate {
                    title: Some(name.into()),
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
) -> Vec<crate::store::planning_sync::PlanningChange> {
    if project {
        fixture
            .store
            .sqlite
            .pending_project_changes(task.project_id.as_ref().unwrap())
            .unwrap()
    } else {
        fixture.store.sqlite.pending_task_changes(&task.id).unwrap()
    }
}

#[test]
fn planning_fields_unsent_save_replaces_a_concurrent_linear_edit_without_moving_workflow() {
    for project in [false, true] {
        with_planning_task(|runtime, fixture, repo, task, state| {
            runtime.block_on(async {
                let work = save_field_name(fixture, task, project, "Local name");
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
                // The local save is sent after Linear's edit, so it wins.
                crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                    .await
                    .unwrap();
                assert!(field_receipts(fixture, task, project).is_empty());
                let saved_name = if project {
                    fixture
                        .store
                        .sqlite
                        .project(task.project_id.as_ref().unwrap())
                        .unwrap()
                        .unwrap()
                        .name
                } else {
                    fixture.store.sqlite.task(&task.id).unwrap().unwrap().title
                };
                assert_eq!(saved_name, "Local name");
                {
                    let provider = state.lock().await;
                    assert_eq!(provider.field_writes, 1);
                    let name = if project {
                        &provider.project_fields.as_ref().unwrap()["name"]
                    } else {
                        &provider.issues[0]["title"]
                    };
                    assert_eq!(name, "Local name");
                }
                let conn = rusqlite::Connection::open(&fixture.database).unwrap();
                let table = if project {
                    "project_changes"
                } else {
                    "task_changes"
                };
                let conflict: Option<String> = conn
                    .query_row(
                        &format!("SELECT conflict_json FROM {table} WHERE id=?1"),
                        [&receipt.id],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert!(conflict.is_none());
                assert_eq!(
                    fixture
                        .store
                        .work_status(&crate::durable::WorkRef::Task(task.id.clone()))
                        .await
                        .unwrap(),
                    WorkStatus::Ready
                );
                assert!(fixture.store.sqlite.task_graph(&task.id).unwrap().is_none());
            });
        });
    }
}

#[test]
fn planning_fields_project_content_preserves_prose_and_task_assignee_can_be_cleared() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            let mut provider = planning_project("project-1", "project-1");
            let example = "## Example\n```yaml\nworkflow: review\nflow: example\n```\n";
            provider["content"] = json!(format!(
                "Keep the introduction.\n\nworkflow: feature\n\n{example}## Notes\nKeep these notes.\n"
            ));
            mark_issue_updated(&mut provider);
            state.lock().await.project_fields = Some(provider);
            fixture
                .store
                .sqlite
                .update_project_content(
                    task.project_id.as_ref().unwrap(),
                    &[crate::work::project::ProjectKR { text: "Keep working offline".into(), holds: true }],
                    &[],
                )
                .unwrap();
            crate::ops::planning_delivery::sync_fields(
                &fixture.store,
                repo,
                &crate::durable::WorkRef::Project(task.project_id.clone().unwrap()),
            )
            .await
            .unwrap();
            assert!(fixture
                .store
                .sqlite
                .pending_project_changes(task.project_id.as_ref().unwrap())
                .unwrap()
                .is_empty());
            let content = state.lock().await.project_fields.as_ref().unwrap()["content"]
                .as_str()
                .unwrap()
                .to_owned();
            assert!(content.contains(example));
            assert!(content.contains("Keep the introduction."));
            assert!(content.contains("## Notes\nKeep these notes."));
            let parsed = crate::linear::project_document::parse_project_document(&content).unwrap();
            assert!(content.contains("workflow: feature"));
            assert!(parsed.krs[0].holds);
            assert_eq!(parsed.krs[0].text, "Keep working offline");
            for assignee in [Some("person".to_string()), None] {
                let saved = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
                fixture
                    .store
                    .sqlite
                    .edit_task(
                        &task.id,
                        saved.revision,
                        &crate::work::task::TaskUpdate {
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
                fixture.store.sqlite.task(&task.id).unwrap().unwrap().title,
                "Unconfirmed save"
            );
        });
    });
}

#[test]
fn planning_fields_deliver_project_activation_and_task_membership_without_execution() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            let project_work = crate::durable::WorkRef::Project(task.project_id.clone().unwrap());
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
                .ensure_project(task.wave_id.as_ref().unwrap(), "Chapter")
                .unwrap();
            crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &project_work)
                .await
                .unwrap();
            assert!(fixture
                .store
                .sqlite
                .pending_project_changes(task.project_id.as_ref().unwrap())
                .unwrap()
                .is_empty());
            assert_eq!(
                state.lock().await.project_fields.as_ref().unwrap()["status"]["type"],
                "started"
            );
            let destination = serde_json::from_value(json!({
                "id": "project-2", "slug": "next-chapter", "name": "Next chapter", "summary": "",
                "revision": "2026-09-30T12:00:00Z", "metric_targets": [], "workflow": "feature",
                "status": "started", "krs": [], "initiative_ids": ["initiative-1"], "team_ids": ["team-1"]
            })).unwrap();
            state
                .lock()
                .await
                .extra_projects
                .push(planning_project("project-2", "project-2"));
            crate::linear::acceptance::accept_linear_project_in(&fixture
                .store
                .sqlite, task.wave_id.as_ref().unwrap(), "initiative-1", &destination, now())
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
                        wave_id: task.wave_id.clone().unwrap(),
                        successor_id: destination.id.to_string(),
                        create: false,
                        project_name: "Next chapter".into(),
                        metric_targets: Vec::new(),
                        krs: vec![crate::work::project::ProjectKR {
                            text: "Retain Task identity".into(),
                            holds: false,
                        }],
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
                .move_task(&task.id, task.project_id.as_ref(), Some(&destination.id))
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
            assert_eq!(retained.project_id, Some(destination.id));
            assert!(retained.worktree.is_none());
            assert!(fixture.store.sqlite.task_graph(&task.id).unwrap().is_none());
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
        .create_task(&crate::work::task::NewTask {
            repository_id: fixture
                .store
                .sqlite
                .ensure_repository(repo.to_str().unwrap())
                .unwrap(),
            due_date: None,
            id: crate::durable::TaskId::new(),
            project_id: Some(project),
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
    LINEAR_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        test(&runtime, &fixture, &repo, &task, state)
    });
    server.abort();
}

#[test]
fn repository_sync_exports_later_tasks_after_an_earlier_export_times_out() {
    exports_later_tasks_after_timeout(false);
}

#[test]
fn save_sync_exports_later_tasks_after_an_earlier_export_times_out() {
    exports_later_tasks_after_timeout(true);
}

fn exports_later_tasks_after_timeout(after_save: bool) {
    with_export_plan(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            crate::ops::planning_export::sync_export(
                &fixture.store,
                repo,
                &crate::durable::WorkRef::Project(task.project_id.clone().unwrap()),
            )
            .await
            .unwrap();
            let later = fixture
                .store
                .sqlite
                .create_task(&crate::work::task::NewTask {
                    repository_id: task.repository_id.clone(),
                    due_date: None,
                    id: crate::durable::TaskId::new(),
                    project_id: task.project_id.clone(),
                    title: "Later saved Task".into(),
                    description: "Must not starve".into(),
                })
                .unwrap();
            let owners = fixture
                .store
                .sqlite
                .planning_export_owners(repo.to_str().unwrap())
                .unwrap();
            let crate::durable::WorkRef::Task(first) = &owners[0] else {
                panic!("expected unmapped Task")
            };
            let export = fixture
                .store
                .sqlite
                .prepare_planning_export(
                    crate::store::sqlite::planning_changes::PlanningChanges::Task(first),
                    "team-1",
                    Some("initiative-1"),
                )
                .unwrap();
            state.lock().await.slow_export_id = Some(export.id);
            if after_save {
                crate::ops::planning_sync::sync_after_save(&fixture.store, repo.to_str().unwrap())
                    .await;
            } else {
                crate::ops::linear_observe::sync_repository(&fixture.store, repo.to_str().unwrap())
                    .await;
            }
            let other = if first == &task.id {
                &later.id
            } else {
                &task.id
            };
            assert!(fixture
                .store
                .get_task(first)
                .await
                .unwrap()
                .unwrap()
                .linear_id
                .is_none());
            assert!(fixture
                .store
                .get_task(other)
                .await
                .unwrap()
                .unwrap()
                .linear_id
                .is_some());
        });
    });
}

#[test]
fn task_create_in_linear_repository_returns_published_task_without_execution() {
    with_export_plan(|runtime, fixture, repo, task, _state| {
        let wave = runtime
            .block_on(fixture.store.get_wave(task.wave_id.as_ref().unwrap()))
            .unwrap()
            .unwrap();
        let saved = crate::ops::task::task_create(
            repo,
            Some(wave.slug()),
            Some("New Linear Task".into()),
            Some("Retain this work".into()),
        )
        .unwrap();
        assert!(saved.linear_id.is_some());
        assert!(saved.identifier.starts_with("FIX-"));
        assert!(saved.worktree.is_none());
    });
}

#[test]
fn task_sync_explicit_updates_retry_saved_state_and_fields_without_execution() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            let mut provider = state.lock().await;
            provider.fail_completion = true;
            provider.field_reject_write = true;
        });
        crate::ops::task::task_edit(
            repo,
            task.id.as_str(),
            None,
            crate::work::task::TaskUpdate {
                title: Some("Saved new title".into()),
                ..Default::default()
            },
        )
        .unwrap();
        crate::ops::task::task_complete(repo, task.id.as_str(), None).unwrap();
        runtime.block_on(state.lock()).field_reject_write = false;
        let pending = crate::ops::task::task_sync(task.id.as_str(), false, false).unwrap();
        assert!(
            pending.contains("confirmation") || pending.contains("uncertain"),
            "{pending}"
        );
        crate::ops::task::task_sync(task.id.as_str(), false, true).unwrap();
        runtime.block_on(async {
            let provider = state.lock().await;
            assert_eq!(provider.issues[0]["title"], "Saved new title");
            assert_eq!(provider.issues[0]["state"]["type"], "completed");
            assert_eq!(provider.completion_writes, 1);
        });
        assert!(fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .is_none());
        let saved = runtime
            .block_on(fixture.store.get_task(&task.id))
            .unwrap()
            .unwrap();
        assert_eq!(saved.worktree, task.worktree);
    });
}

#[test]
fn task_sync_explicit_retry_still_adopts_a_newer_linear_state() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(state.lock()).fail_completion = true;
        crate::ops::task::task_complete(repo, task.id.as_str(), None).unwrap();
        runtime.block_on(async {
            let mut provider = state.lock().await;
            provider.issues[0]["state"] = json!({"type":"started"});
            mark_issue_updated(&mut provider.issues[0]);
        });
        crate::ops::task::task_sync(task.id.as_str(), false, true).unwrap();
        let saved = runtime
            .block_on(fixture.store.get_task(&task.id))
            .unwrap()
            .unwrap();
        assert_eq!(saved.state.as_deref(), Some("started"));
        assert_eq!(runtime.block_on(state.lock()).completion_writes, 0);
    });
}

#[test]
fn historical_comment_delivery_retains_local_identity_after_linear_readback() {
    with_planning_task(|runtime, fixture, _repo, task, state| {
        let comment = crate::work::task::comment::TaskComment {
            id: uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, b"retained-filing").to_string(),
            body: "Retained follow-up evidence".into(),
            author: crate::work::task::comment::TaskCommentAuthor::Person {
                name: Some("Jack Heart".into()),
            },
            created_at: Some("2026-10-08T01:00:00Z".into()),
        };
        fixture
            .store
            .sqlite
            .append_task_comment(&task.id, &comment)
            .unwrap();
        runtime.block_on(async {
            state.lock().await.lose_comment = true;
            crate::ops::linear_observe::sync_task_comments(&fixture.store, task)
                .await
                .unwrap();
            crate::ops::linear_observe::refresh_task_comments(&fixture.store, task)
                .await
                .unwrap();
            crate::ops::linear_observe::sync_task_comments(&fixture.store, task)
                .await
                .unwrap();
            let provider = state.lock().await;
            assert_eq!(provider.comments.len(), 1);
            assert_eq!(
                provider.comments[0]["id"],
                crate::linear::comment_delivery_id(&comment.id).unwrap()
            );
        });
        assert!(fixture
            .store
            .sqlite
            .pending_task_comments(&task.id)
            .unwrap()
            .is_empty());
        let thread = fixture.store.sqlite.task_comments(&task.id).unwrap();
        assert_eq!(thread.comments.len(), 1);
        assert_eq!(thread.comments[0].id, comment.id);
        assert_eq!(thread.comments[0].body, comment.body);
    });
}

#[test]
fn task_sync_publishes_saved_task_and_reuses_its_identity() {
    with_export_plan(|runtime, fixture, _repo, task, state| {
        assert!(task.linear_id.is_none());
        let result = crate::ops::task::task_sync(task.id.as_str(), false, false).unwrap();
        let saved = runtime
            .block_on(fixture.store.get_task(&task.id))
            .unwrap()
            .unwrap();
        assert!(saved.linear_id.is_some());
        assert!(result.contains(&saved.identifier));
        assert!(!saved.identifier.starts_with("lf-"));
        crate::ops::task::task_sync(task.id.as_str(), false, false).unwrap();
        assert_eq!(runtime.block_on(state.lock()).creation_writes, 2);
    });
}

#[test]
fn task_sync_does_not_claim_success_for_uncertain_publication() {
    with_export_plan(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            crate::ops::planning_export::sync_export(
                &fixture.store,
                repo,
                &crate::durable::WorkRef::Project(task.project_id.clone().unwrap()),
            )
            .await
            .unwrap();
            state.lock().await.lose_creation_reply = true;
            assert!(crate::ops::planning_export::sync_export(
                &fixture.store,
                repo,
                &crate::durable::WorkRef::Task(task.id.clone()),
            )
            .await
            .is_err());
            state.lock().await.hide_exports = true;
        });
        let error = crate::ops::task::task_sync(task.id.as_str(), false, false).unwrap_err();
        assert!(error.to_string().contains("uncertain"), "{error}");
        assert_eq!(runtime.block_on(state.lock()).creation_writes, 2);
    });
}

#[test]
fn task_sync_explicit_retry_preserves_saved_creation_identity() {
    with_export_plan(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            crate::ops::planning_export::sync_export(
                &fixture.store,
                repo,
                &crate::durable::WorkRef::Project(task.project_id.clone().unwrap()),
            )
            .await
            .unwrap();
            state.lock().await.lose_creation_reply = true;
            assert!(crate::ops::planning_export::sync_export(
                &fixture.store,
                repo,
                &crate::durable::WorkRef::Task(task.id.clone())
            )
            .await
            .is_err());
            // Simulate the absent issue that an explicit retry is allowed to recreate.
            state
                .lock()
                .await
                .issues
                .retain(|issue| issue["title"] != task.title);
        });
        assert!(crate::ops::task::task_sync(task.id.as_str(), false, false).is_err());
        runtime.block_on(state.lock()).rate_limit_query = Some("mutation DeliverTaskCreation");
        assert!(crate::ops::task::task_sync(task.id.as_str(), true, false).is_err());
        assert!(
            fixture
                .store
                .sqlite
                .planning_export_attempts(
                    crate::store::sqlite::planning_changes::PlanningChanges::Task(&task.id),
                )
                .unwrap()
                .0
        );
        runtime.block_on(async {
            tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
        });
        crate::ops::task::task_sync(task.id.as_str(), true, false).unwrap();
        let saved = runtime
            .block_on(fixture.store.get_task(&task.id))
            .unwrap()
            .unwrap();
        let receipt = fixture
            .store
            .sqlite
            .prepare_planning_export(
                crate::store::sqlite::planning_changes::PlanningChanges::Task(&task.id),
                "team-1",
                Some("initiative-1"),
            )
            .unwrap();
        assert_eq!(saved.linear_id.unwrap().as_str(), receipt.id);
        assert_eq!(saved.id, task.id);
        crate::ops::task::task_sync(task.id.as_str(), true, false).unwrap();
        assert_eq!(runtime.block_on(state.lock()).creation_writes, 3);
    });
}

#[test]
fn standalone_export_recovers_and_moves_without_duplicate_issues() {
    with_export_plan(|runtime, fixture, repo, task, state| {
        let project_id = task.project_id.as_ref().unwrap();
        let work = crate::durable::WorkRef::Task(task.id.clone());
        fixture
            .store
            .sqlite
            .move_task(&task.id, Some(project_id), None)
            .unwrap();
        state.blocking_lock().lose_creation_reply = true;
        assert!(runtime
            .block_on(crate::ops::planning_export::sync_export(
                &fixture.store,
                repo,
                &work,
            ))
            .is_err());
        let external = state.blocking_lock().issues[0]["id"].clone();
        assert!(state.blocking_lock().issues[0]["project"].is_null());
        assert_eq!(state.blocking_lock().issues[0]["team"]["id"], "team-1");
        state.blocking_lock().hide_exports = true;
        assert!(runtime
            .block_on(crate::ops::planning_export::sync_export(
                &fixture.store,
                repo,
                &work,
            ))
            .is_err());
        state.blocking_lock().hide_exports = false;
        let reopened = runtime
            .block_on(crate::store::open_ephemeral_store(
                &crate::store::StorageConfig::sqlite(fixture.database.clone()),
            ))
            .unwrap();
        runtime
            .block_on(crate::ops::planning_export::sync_export(
                &reopened, repo, &work,
            ))
            .unwrap();
        let saved = reopened.sqlite.task(&task.id).unwrap().unwrap();
        assert_eq!(
            saved.linear_id.as_ref().unwrap().as_str(),
            external.as_str().unwrap()
        );
        assert!(saved.project_id.is_none());
        assert_eq!(state.blocking_lock().creation_writes, 1);
        assert!(state.blocking_lock().extra_projects.is_empty());

        runtime
            .block_on(crate::ops::planning_export::sync_export(
                &reopened,
                repo,
                &crate::durable::WorkRef::Project(project_id.clone()),
            ))
            .unwrap();
        let project = reopened.sqlite.project(project_id).unwrap().unwrap();
        for (expected, destination) in [(None, Some(project_id)), (Some(project_id), None)] {
            reopened
                .sqlite
                .move_task(&task.id, expected, destination)
                .unwrap();
            runtime
                .block_on(crate::ops::planning_delivery::sync_fields(
                    &reopened, repo, &work,
                ))
                .unwrap();
            let saved = reopened.sqlite.task(&task.id).unwrap().unwrap();
            assert_eq!(saved.project_id.as_ref(), destination);
            let provider = state.blocking_lock();
            assert_eq!(provider.issues.len(), 1);
            assert_eq!(provider.issues[0]["id"], external);
            if destination.is_some() {
                assert_eq!(
                    provider.issues[0]["project"]["id"],
                    project.linear_id.as_ref().unwrap().as_str()
                );
            } else {
                assert!(provider.issues[0]["project"].is_null());
            }
        }
        assert!(reopened
            .sqlite
            .pending_task_changes(&task.id)
            .unwrap()
            .is_empty());
        assert_eq!(
            runtime.block_on(reopened.list_tasks(None)).unwrap().len(),
            1
        );
    });
}

#[test]
fn planning_export_lost_creation_and_attachment_recover_without_duplicate_effects() {
    with_export_plan(|runtime, fixture, repo, task, state| {
        let project = crate::durable::WorkRef::Project(task.project_id.clone().unwrap());
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
                saved.revision,
                &crate::work::task::TaskUpdate {
                    title: Some("After export".into()),
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
        let project = crate::durable::WorkRef::Project(task.project_id.clone().unwrap());
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
                            .edit_project(id, Some("Later chapter"), None, None)
                            .unwrap(),
                        crate::durable::WorkRef::Task(id) => {
                            let saved = fixture.store.get_task(id).await.unwrap().unwrap();
                            fixture
                                .store
                                .sqlite
                                .edit_task(
                                    id,
                                    saved.revision,
                                    &crate::work::task::TaskUpdate {
                                        title: Some("Later task".into()),
                                        ..Default::default()
                                    },
                                )
                                .unwrap();
                            // Inventory observes the committed creation before its response returns.
                            super::refresh_wave(repo, "product").await.unwrap();
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
                .project(task.project_id.as_ref().unwrap())
                .unwrap()
                .unwrap()
                .name,
            "Later chapter"
        );
        assert_eq!(
            fixture.store.sqlite.task(&task.id).unwrap().unwrap().title,
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
                .pending_project_changes(task.project_id.as_ref().unwrap())
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
        // A Linear edit before the pending save is sent loses to it: the save
        // goes out last and replaces it.
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
            fixture.store.sqlite.task(&task.id).unwrap().unwrap().title,
            "Later task"
        );
        assert_eq!(state.blocking_lock().issues[0]["title"], "Later task");
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
        let project = crate::durable::WorkRef::Project(task.project_id.clone().unwrap());
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
            fixture
                .store
                .sqlite
                .task_acquisition(&task.id)
                .unwrap()
                .state,
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
            .create_task(&crate::work::task::NewTask {
                repository_id: task.repository_id.clone(),
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

#[test]
fn wave_summary_sync_reads_committed_default_branch() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    let (repo, _) = runtime.block_on(planning_repo(&fixture));
    runtime.block_on(fixture.seed(now() + 86_400));
    let git = |repo: &Path, args: &[&str]| {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&repo, &["branch", "-M", "trunk"]);
    git(
        &repo,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/trunk",
        ],
    );
    let commit = |repo: &Path| {
        git(repo, &["add", "."]);
        git(
            repo,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "fixture goal",
            ],
        );
    };
    commit(&repo);
    let checkout = fixture.directory.path().join("checkout");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-qb",
            "summary",
            checkout.to_str().unwrap(),
        ],
    );
    let goal = repo.join(".lf/wave/product/GOAL.md");
    let write = |text: &str| {
        std::fs::write(
            &goal,
            format!("---\nlinear:\n  initiative: initiative-1\n---\n\n## Objective\n\n{text}\n"),
        )
        .unwrap();
    };
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState::default()));
    let (url, server) = runtime.block_on(serve(state.clone()));
    LINEAR_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        let sync = |plan| {
            crate::lf::commands::ops::sync_planning(&checkout, Some("product"), false, plan, false)
        };
        std::fs::write(
            checkout.join(".lf/wave/product/GOAL.md"),
            "---\nlinear:\n  initiative: initiative-1\n---\n\n## Objective\n\nEdited directly.\n",
        )
        .unwrap();
        commit(&checkout);
        sync(false).unwrap();
        assert_eq!(
            runtime.block_on(async { state.lock().await.wave_summary.clone() }),
            "Keep working."
        );
        // Even a dirty default checkout must not publish unmerged text.
        write("Edited directly.");
        sync(false).unwrap();
        assert_eq!(
            runtime.block_on(async { state.lock().await.wave_summary.clone() }),
            "Keep working."
        );
        commit(&repo);
        sync(false).unwrap();
        assert_eq!(
            runtime.block_on(async { state.lock().await.wave_summary.clone() }),
            "Edited directly."
        );
        // No duplicate provider effect for unchanged normalized text.
        sync(false).unwrap();
        assert_eq!(
            runtime.block_on(async { state.lock().await.wave_updates }),
            2
        );
        write("Plan only.");
        commit(&repo);
        sync(true).unwrap();
        assert_eq!(
            runtime.block_on(async { state.lock().await.wave_summary.clone() }),
            "Edited directly."
        );
        std::fs::remove_file(&goal).unwrap();
        commit(&repo);
        assert!(sync(false).unwrap_err().to_string().contains("missing"));
        assert_eq!(
            runtime.block_on(async { state.lock().await.wave_updates }),
            2
        );
        write("Rejected update.");
        commit(&repo);
        runtime.block_on(async { state.lock().await.reject_wave_update = true });
        assert!(sync(false)
            .unwrap_err()
            .to_string()
            .contains("summary update refused"));
        assert_eq!(
            runtime.block_on(async { state.lock().await.wave_summary.clone() }),
            "Edited directly."
        );
        assert_eq!(
            runtime.block_on(async { state.lock().await.wave_updates }),
            3
        );
        runtime.block_on(async { state.lock().await.reject_wave_update = false });
        write("");
        commit(&repo);
        sync(false).unwrap();
        assert_eq!(
            runtime.block_on(async { state.lock().await.wave_summary.clone() }),
            ""
        );
        assert!(
            std::fs::read_to_string(checkout.join(".lf/wave/product/GOAL.md"))
                .unwrap()
                .contains("Edited directly.")
        );
    });
    server.abort();
}

#[test]
fn linear_rate_limit_retries_field_without_losing_saved_identity() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        fixture
            .store
            .sqlite
            .edit_task(
                &task.id,
                task.revision,
                &crate::work::task::TaskUpdate {
                    title: Some("After reset".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let receipt = fixture
            .store
            .sqlite
            .pending_task_changes(&task.id)
            .unwrap()
            .remove(0);
        let work = crate::durable::WorkRef::Task(task.id.clone());
        runtime.block_on(async {
            state.lock().await.rate_limit_query = Some("mutation DeliverTaskField");
            crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                .await
                .unwrap();
            let pending = fixture
                .store
                .sqlite
                .pending_task_changes(&task.id)
                .unwrap()
                .remove(0);
            assert_eq!(pending.id, receipt.id);
            tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
            crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                .await
                .unwrap();
            assert!(fixture
                .store
                .sqlite
                .pending_task_changes(&task.id)
                .unwrap()
                .is_empty());
            assert_eq!(state.lock().await.issues[0]["title"], "After reset");
            assert_eq!(state.lock().await.field_writes, 1);
        });
    });
}

#[test]
fn linear_rate_limit_retries_creation_and_attachment_with_saved_ids() {
    for query in [
        "mutation DeliverProjectCreation",
        "mutation DeliverProjectAttachment",
        "mutation DeliverTaskCreation",
    ] {
        with_export_plan(|runtime, fixture, repo, task, state| {
            runtime.block_on(async {
                state.lock().await.rate_limit_query = Some(query);
                crate::ops::planning_export::sync_repository_exports(
                    &fixture.store,
                    repo.to_str().unwrap(),
                )
                .await
                .unwrap();
                assert!(
                    state.lock().await.rate_limit_query.is_none(),
                    "fixture must reject {query}"
                );
                tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
                crate::ops::planning_export::sync_repository_exports(
                    &fixture.store,
                    repo.to_str().unwrap(),
                )
                .await
                .unwrap();
                let saved = fixture.store.get_task(&task.id).await.unwrap().unwrap();
                assert_eq!(saved.id, task.id);
                assert!(
                    saved.linear_id.is_some(),
                    "{query} must leave creation retryable"
                );
                assert_eq!(state.lock().await.creation_writes, 2);
                assert_eq!(state.lock().await.attachment_writes, 1);
            });
        });
    }
}

async fn seed_finished_tasks(
    fixture: &Fixture,
    repo: &Path,
    state: &tokio::sync::Mutex<PlanningState>,
) {
    {
        let mut provider = state.lock().await;
        for index in 2..=351 {
            let mut issue = provider.issues[0].clone();
            issue["id"] = json!(format!("issue-{index}"));
            issue["identifier"] = json!(format!("FIX-{index}"));
            issue["state"]["type"] = json!(if index % 2 == 0 {
                "completed"
            } else {
                "canceled"
            });
            provider.issues.push(issue);
        }
    }
    // Bulk fixture setup is not an opportunistic foreground refresh. Its disk
    // writes must finish before measuring on-demand requests on slower CI hosts.
    let (context, initiative) = super::resolve_context(repo, "product").await.unwrap();
    super::refresh_inventory(repo, "product", &context, &initiative)
        .await
        .unwrap();
    rusqlite::Connection::open(&fixture.database)
        .unwrap()
        .execute(
            "UPDATE tasks SET abandoned_at=?1 WHERE planning_state='canceled'",
            [now()],
        )
        .unwrap();
}

#[test]
fn linear_on_demand_shares_checks_and_excludes_finished_tasks() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            seed_finished_tasks(fixture, repo, &state).await;
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            conn.execute("DELETE FROM linear_team_checks", []).unwrap();
            state.lock().await.requests = 0;
            let results = futures_util::future::join_all((0..4).map(|_| {
                crate::ops::linear_observe::check_repository(&fixture.store, repo.to_str().unwrap())
            }))
            .await;
            for result in results {
                result.unwrap();
            }
            // One shared delta, then one read of the Project its changed issue names.
            assert_eq!(state.lock().await.requests, 2);
            conn.execute(
                "UPDATE tasks SET abandoned_at=?1 WHERE id=?2",
                rusqlite::params![now(), task.id.as_str()],
            )
            .unwrap();
            assert!(!crate::ops::linear_observe::check_repository(
                &fixture.store,
                repo.to_str().unwrap()
            )
            .await
            .unwrap());
            assert_eq!(
                state.lock().await.requests,
                2,
                "a team without live Tasks is not checked"
            );
        });
    });
}

#[test]
fn linear_on_demand_acquires_comment_only_changes_and_completion() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            conn.execute("DELETE FROM linear_team_checks", []).unwrap();
            crate::ops::linear_observe::check_repository(&fixture.store, repo.to_str().unwrap()).await.unwrap();
            let timestamp = time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap();
            state.lock().await.comments.push(json!({"id":"new-direction","body":"Keep the saved identity", "createdAt":timestamp,"updatedAt":timestamp,"user":{"id":"person-1","displayName":"Jack Heart","name":"Jack Heart"}}));
            conn.execute("UPDATE linear_team_checks SET attempted_at=0", []).unwrap();
            crate::ops::linear_observe::check_repository(&fixture.store, repo.to_str().unwrap()).await.unwrap();
            assert!(fixture.store.task_steers(&task.id).await.unwrap().iter().any(|steer| steer.text.contains("Keep the saved identity")));
            {
                let mut provider = state.lock().await;
                provider.issues[0]["state"]["type"] = json!("completed");
                mark_issue_updated(&mut provider.issues[0]);
            }
            conn.execute("UPDATE linear_team_checks SET attempted_at=0", []).unwrap();
            crate::ops::linear_observe::check_repository(&fixture.store, repo.to_str().unwrap()).await.unwrap();
            assert!(fixture.store.task_state(&task.id).await.unwrap().is_terminal());
        });
    });
}

#[test]
fn linear_on_demand_failed_check_retains_watermark_and_pending_completion() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            state.lock().await.rate_limit_query = Some("mutation SetIssueState");
        });
        crate::ops::task::task_complete(repo, task.id.as_str(), Some("Complete after reset"))
            .unwrap();
        runtime.block_on(async {
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            let receipt = fixture
                .store
                .sqlite
                .pending_task_state(&task.id)
                .unwrap()
                .unwrap();
            assert!(!receipt.attempted);
            let before = state.lock().await.requests;
            crate::ops::linear_observe::sync_repository(&fixture.store, repo.to_str().unwrap())
                .await;
            assert_eq!(state.lock().await.requests, before);
            assert_eq!(
                fixture
                    .store
                    .sqlite
                    .pending_task_state(&task.id)
                    .unwrap()
                    .unwrap()
                    .id,
                receipt.id
            );
            tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
            crate::ops::linear_observe::sync_repository(&fixture.store, repo.to_str().unwrap())
                .await;
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
fn linear_on_demand_failure_throttles_attempts_without_losing_changes() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            conn.execute("DELETE FROM linear_team_checks", []).unwrap();
            crate::ops::linear_observe::check_repository(&fixture.store, repo.to_str().unwrap())
                .await
                .unwrap();
            let through: String = conn
                .query_row("SELECT synced_through FROM linear_team_checks", [], |row| {
                    row.get(0)
                })
                .unwrap();
            conn.execute("UPDATE linear_team_checks SET attempted_at=0", [])
                .unwrap();
            state.lock().await.field_outage = true;
            assert!(crate::ops::linear_observe::check_repository(
                &fixture.store,
                repo.to_str().unwrap()
            )
            .await
            .is_err());
            let requests = state.lock().await.requests;
            assert!(!crate::ops::linear_observe::check_repository(
                &fixture.store,
                repo.to_str().unwrap()
            )
            .await
            .unwrap());
            assert_eq!(state.lock().await.requests, requests);
            assert_eq!(
                conn.query_row("SELECT synced_through FROM linear_team_checks", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
                through
            );
            {
                let mut provider = state.lock().await;
                provider.field_outage = false;
                provider.issues[0]["title"] = json!("Retained through the outage");
                mark_issue_updated(&mut provider.issues[0]);
            }
            conn.execute("UPDATE linear_team_checks SET attempted_at=0", [])
                .unwrap();
            crate::ops::linear_observe::check_repository(&fixture.store, repo.to_str().unwrap())
                .await
                .unwrap();
            assert_eq!(
                fixture
                    .store
                    .get_task(&task.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .title,
                "Retained through the outage"
            );
        });
    });
}

#[test]
fn planning_fields_use_mutation_responses_without_reading_back_own_writes() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        fixture
            .store
            .sqlite
            .edit_task(
                &task.id,
                task.revision,
                &crate::work::task::TaskUpdate {
                    title: Some("Confirmed in the mutation".into()),
                    description: Some("Both fields share one initial observation".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        runtime.block_on(async {
            // Initial observation succeeds; any further issue read fails.
            state.lock().await.fail_issue_read_after = Some(1);
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
            let provider = state.lock().await;
            assert_eq!(provider.issues[0]["title"], "Confirmed in the mutation");
            assert_eq!(
                provider.issues[0]["description"],
                "Both fields share one initial observation"
            );
            assert_eq!(provider.field_writes, 2);
        });
    });
}

#[test]
fn planning_export_acquisition_acknowledges_canonical_membership() {
    with_export_plan(|runtime, fixture, repo, task, state| {
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        conn.execute("INSERT INTO task_changes(id,task_id,value_json) VALUES('before-creation-membership',?1,?2)",
            rusqlite::params![task.id.as_str(),serde_json::to_string(&WorkValue::TaskProject(task.project_id.as_ref().map(ToString::to_string))).unwrap()]).unwrap();
        for owner in [
            crate::durable::WorkRef::Project(task.project_id.clone().unwrap()),
            crate::durable::WorkRef::Task(task.id.clone()),
        ] {
            runtime
                .block_on(crate::ops::planning_export::sync_export(
                    &fixture.store,
                    repo,
                    &owner,
                ))
                .unwrap();
        }
        let project = fixture
            .store
            .sqlite
            .project(task.project_id.as_ref().unwrap())
            .unwrap()
            .unwrap();
        assert_ne!(
            project.id.as_str(),
            project.linear_id.as_ref().unwrap().as_str()
        );
        assert!(conn
            .query_row(
                "SELECT acknowledged FROM task_changes WHERE id='before-creation-membership'",
                [],
                |row| row.get::<_, bool>(0)
            )
            .unwrap());
        let retained = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
        assert_eq!(retained.project_id, Some(project.id));
        assert!(fixture
            .store
            .sqlite
            .pending_task_changes(&task.id)
            .unwrap()
            .is_empty());
        assert_eq!(state.blocking_lock().field_writes, 0);
    });
}

#[test]
fn planning_export_captures_shared_task_fields_without_local_execution() {
    with_export_plan(|runtime, fixture, repo, task, _state| {
        runtime
            .block_on(crate::ops::planning_export::sync_export(
                &fixture.store,
                repo,
                &crate::durable::WorkRef::Project(task.project_id.clone().unwrap()),
            ))
            .unwrap();
        let child = fixture
            .store
            .sqlite
            .create_task(&crate::work::task::NewTask {
                repository_id: task.repository_id.clone(),
                id: crate::durable::TaskId::new(),
                project_id: task.project_id.clone(),
                title: "Retained creation".into(),
                description: "Authored prose".into(),
                due_date: Some("2026-12-31".into()),
            })
            .unwrap();
        let owner = crate::store::sqlite::planning_changes::PlanningChanges::Task(&child.id);
        let first = fixture
            .store
            .sqlite
            .prepare_planning_export(owner, "team-1", Some("initiative"))
            .unwrap();
        assert_eq!(first.input["title"], child.title);
        assert_eq!(first.input["description"], child.description);
        assert_eq!(first.input["dueDate"], child.due_date.as_deref().unwrap());
        assert_eq!(
            first.baseline,
            vec![
                WorkValue::TaskTitle(Some(child.title.clone())),
                WorkValue::TaskDescription(Some(child.description.clone())),
                WorkValue::TaskAssignee(child.assignee.clone()),
                WorkValue::TaskDueDate(child.due_date.clone()),
                WorkValue::TaskProject(child.project_id.as_ref().map(ToString::to_string)),
            ]
        );
        fixture
            .store
            .sqlite
            .edit_task(
                &child.id,
                child.revision,
                &crate::work::task::TaskUpdate {
                    title: Some("Later save".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let retry = fixture
            .store
            .sqlite
            .prepare_planning_export(owner, "another-team", Some("another-initiative"))
            .unwrap();
        assert_eq!(retry.input, first.input);
        assert_eq!(retry.baseline, first.baseline);
        assert_eq!(retry.task_state, first.task_state);
        assert_eq!(retry.id, first.id);
        assert_eq!(
            fixture.store.sqlite.task(&child.id).unwrap().unwrap().title,
            "Later save"
        );
    });
}

#[test]
fn project_field_delivery_requires_revision_accepted_into_work() {
    for rejection in ["stale", "foreign", "archived", "unresolved", "unversioned"] {
        with_planning_task(|runtime, fixture, repo, task, state| {
            runtime.block_on(async {
                let work = crate::durable::WorkRef::Project(task.project_id.clone().unwrap());
                let mut provider = planning_project("project-1", "project-1");
                provider["content"] = json!("Accepted\n\nworkflow: feature\n");
                provider["updatedAt"] = json!("2026-10-10T12:00:00Z");
                state.lock().await.project_fields = Some(provider.clone());
                crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                    .await
                    .unwrap();
                fixture
                    .store
                    .sqlite
                    .edit_project(task.project_id.as_ref().unwrap(), None, None, Some("Saved"))
                    .unwrap();
                let pending = fixture
                    .store
                    .sqlite
                    .pending_project_changes(task.project_id.as_ref().unwrap())
                    .unwrap();
                let saved_task = fixture.store.sqlite.task(&task.id).unwrap();
                let workflow = fixture.store.sqlite.task_graph(&task.id).unwrap();
                match rejection {
                    "stale" => provider["updatedAt"] = json!("2026-10-10T11:59:00Z"),
                    "foreign" => {
                        provider["initiatives"] = json!({"nodes":[{"id":"foreign"}]});
                        provider["updatedAt"] = json!("2026-10-10T12:01:00Z");
                    }
                    "archived" => {
                        let client = super::linear_client(repo).await.unwrap();
                        let (observed, _) = client
                            .project_content_observation("project-1")
                            .await
                            .unwrap();
                        fixture.store.sqlite.conn.lock().unwrap().execute(
                            "UPDATE pm_projects SET archived=1 WHERE repo=?1 AND provider='linear' AND id=?2",
                            rusqlite::params![repo.to_string_lossy(), observed.id],
                        ).unwrap();
                    }
                    "unresolved" => {
                        let client = super::linear_client(repo).await.unwrap();
                        let (mut observed, _) = client
                            .project_content_observation("project-1")
                            .await
                            .unwrap();
                        observed.team_ids = vec!["another-team".into()];
                        assert!(crate::linear::acceptance::accept_linear_project_in(
                            &fixture.store.sqlite,
                            task.wave_id.as_ref().unwrap(),
                            "initiative-1",
                            &observed,
                            now(),
                        )
                        .is_err());
                        // Re-reading the original entity revision cannot order the
                        // disputed Team relation or authorize delivery.
                    }
                    "unversioned" => provider["updatedAt"] = json!(null),
                    _ => unreachable!(),
                }
                state.lock().await.project_fields = Some(provider);
                assert!(
                    crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                        .await
                        .is_err()
                );
                let saved = fixture
                    .store
                    .sqlite
                    .project(task.project_id.as_ref().unwrap())
                    .unwrap()
                    .unwrap();
                assert_eq!(saved.description, "Saved");

                assert_eq!(
                    fixture
                        .store
                        .sqlite
                        .pending_project_changes(task.project_id.as_ref().unwrap())
                        .unwrap(),
                    pending
                );
                assert_eq!(state.lock().await.field_writes, 0, "{rejection}");
                assert_eq!(fixture.store.sqlite.task(&task.id).unwrap(), saved_task);
                assert_eq!(fixture.store.sqlite.task_graph(&task.id).unwrap(), workflow);
                let attempted: bool = rusqlite::Connection::open(&fixture.database)
                    .unwrap()
                    .query_row(
                        "SELECT attempted FROM project_changes WHERE id=?1",
                        [&pending[0].id],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert!(!attempted, "{rejection}");
            });
        });
    }
}

#[test]
fn project_prose_older_acknowledgement_preserves_successor() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            let work = crate::durable::WorkRef::Project(task.project_id.clone().unwrap());
            let save = |text| {
                fixture
                    .store
                    .sqlite
                    .edit_project(task.project_id.as_ref().unwrap(), None, None, Some(text))
                    .unwrap()
            };
            save("First");
            let first = fixture
                .store
                .sqlite
                .pending_project_changes(task.project_id.as_ref().unwrap())
                .unwrap()[0]
                .clone();
            let arrived = Arc::new(tokio::sync::Notify::new());
            let release = Arc::new(tokio::sync::Notify::new());
            {
                let mut provider = state.lock().await;
                provider.field_arrived = Some(arrived.clone());
                provider.field_release = Some(release.clone());
            }
            let delivery = crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work);
            let edit = async {
                arrived.notified().await;
                save("Successor");
                release.notify_one();
            };
            let (result, ()) = tokio::join!(delivery, edit);
            result.unwrap();
            assert_eq!(
                fixture
                    .store
                    .sqlite
                    .project(task.project_id.as_ref().unwrap())
                    .unwrap()
                    .unwrap()
                    .description,
                "Successor"
            );
            let pending = fixture
                .store
                .sqlite
                .pending_project_changes(task.project_id.as_ref().unwrap())
                .unwrap();
            assert_eq!(pending.len(), 1);
            assert_ne!(pending[0].id, first.id);
            assert_eq!(
                pending[0]
                    .base
                    .as_ref()
                    .unwrap()
                    .value
                    .as_ref()
                    .unwrap()
                    .value()
                    .unwrap(),
                "First"
            );
            crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                .await
                .unwrap();
            assert!(fixture
                .store
                .sqlite
                .pending_project_changes(task.project_id.as_ref().unwrap())
                .unwrap()
                .is_empty());
        });
    });
}

#[test]
fn project_prose_omission_is_not_empty_or_receipt_settlement() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            let work = crate::durable::WorkRef::Project(task.project_id.clone().unwrap());
            let mut provider = planning_project("project-1", "project-1");
            provider["content"] = json!("Observed prose\n\nworkflow: feature\n");
            mark_issue_updated(&mut provider);
            state.lock().await.project_fields = Some(provider.clone());
            crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                .await
                .unwrap();
            fixture
                .store
                .sqlite
                .edit_project(
                    task.project_id.as_ref().unwrap(),
                    None,
                    None,
                    Some("Saved prose"),
                )
                .unwrap();
            let pending = fixture
                .store
                .sqlite
                .pending_project_changes(task.project_id.as_ref().unwrap())
                .unwrap();
            provider.as_object_mut().unwrap().remove("content");
            mark_issue_updated(&mut provider);
            state.lock().await.project_fields = Some(provider.clone());
            assert!(
                crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                    .await
                    .is_err()
            );
            assert_eq!(
                fixture
                    .store
                    .sqlite
                    .project(task.project_id.as_ref().unwrap())
                    .unwrap()
                    .unwrap()
                    .description,
                "Saved prose"
            );
            assert_eq!(
                fixture
                    .store
                    .sqlite
                    .pending_project_changes(task.project_id.as_ref().unwrap())
                    .unwrap(),
                pending
            );
            // An explicit null is an observed empty document. The unsent save is
            // sent after it, so the save wins and replaces it in Linear.
            provider["content"] = json!(null);
            mark_issue_updated(&mut provider);
            state.lock().await.project_fields = Some(provider);
            crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                .await
                .unwrap();
            assert_eq!(
                fixture
                    .store
                    .sqlite
                    .project(task.project_id.as_ref().unwrap())
                    .unwrap()
                    .unwrap()
                    .description,
                "Saved prose"
            );
            assert!(
                state.lock().await.project_fields.as_ref().unwrap()["content"]
                    .as_str()
                    .unwrap()
                    .contains("Saved prose")
            );
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            let (conflict, acknowledged): (Option<String>, bool) = conn
                .query_row(
                    "SELECT conflict_json,acknowledged FROM project_changes WHERE id=?1",
                    [&pending[0].id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert!(conflict.is_none());
            assert!(acknowledged);
        });
    });
}

#[test]
fn project_prose_save_delivers_and_settles_without_changing_managed_fields() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            conn.execute("UPDATE tasks SET worktree='/retained/task',branch='retained/branch' WHERE id=?1", [task.id.as_str()]).unwrap();
            conn.execute("INSERT INTO processes(id,trace_id,command,cwd,started_at) VALUES('11111111-1111-4111-8111-111111111111','22222222-2222-4222-8222-222222222222','lf run code','/retained/task',2)", []).unwrap();
            conn.execute("INSERT INTO task_graphs(task_id,graph,step,run_start,lf_process_id,updated_at) VALUES(?1,?2,'start','{\"index\":0}','11111111-1111-4111-8111-111111111111',3)", rusqlite::params![task.id.as_str(),json!({"name":"code","steps":[]}).to_string()]).unwrap();
            let workflow_before = fixture.store.sqlite.task_graph(&task.id).unwrap();
            let work = crate::durable::WorkRef::Project(task.project_id.clone().unwrap());
            let mut provider = planning_project("project-1", "project-1");
            provider["content"] = json!("Before.\n\nworkflow: feature\n\n## KRs\n- [x] Preserve work\n\n## Notes\nAfter.\n");
            mark_issue_updated(&mut provider);
            state.lock().await.project_fields = Some(provider);
            crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work).await.unwrap();
            let before = fixture.store.sqlite.project(task.project_id.as_ref().unwrap()).unwrap().unwrap();
            conn.execute("UPDATE tasks SET pm_snapshot_synced_at=1 WHERE id=?1", [task.id.as_str()]).unwrap();
            conn.execute("DELETE FROM linear_team_checks", []).unwrap();
            let task_before = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
            assert_eq!(before.description, "Before.\n\n## Notes\nAfter.");
            for prose in ["  New purpose.\n\n```yaml\nworkflow: example\n```\n  ", ""] {
                let (saved, _) = crate::ops::project::edit(repo, crate::ops::project::PlanTarget::Project(task.project_id.as_ref().unwrap().as_str()), None, None, Some(prose)).await.unwrap();
                assert_eq!(saved.description, prose.trim());
                assert_eq!(saved.id, before.id);

                assert_eq!(saved.krs, before.krs);
                assert_eq!(saved.metric_targets, before.metric_targets);
                assert!(fixture.store.sqlite.pending_project_changes(&saved.id).unwrap().is_empty());
                let conn = rusqlite::Connection::open(&fixture.database).unwrap();
                let (acknowledged, conflict): (bool, Option<String>) = conn.query_row(
                    "SELECT acknowledged,conflict_json FROM project_changes WHERE project_id=?1 AND field='project_description' ORDER BY seq DESC LIMIT 1",
                    [saved.id.as_str()], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
                assert!(acknowledged);
                assert_eq!(conflict, None);
            }
            let task_after = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
            // The pre-write team check may refresh acquisition time without editing the Task.
            assert!(task_after.pm_snapshot_synced_at > task_before.pm_snapshot_synced_at);
            assert_eq!(task_after, crate::work::task::Task {
                pm_snapshot_synced_at: task_after.pm_snapshot_synced_at,
                ..task_before
            });
            assert_eq!(fixture.store.sqlite.task_graph(&task.id).unwrap(), workflow_before);
            let outcome: Option<String> = conn.query_row("SELECT outcome FROM processes WHERE id='11111111-1111-4111-8111-111111111111'", [], |r| r.get(0)).unwrap();
            assert_eq!(outcome, None);
        });
    });
}

#[test]
fn task_field_delivery_requires_revision_accepted_into_work() {
    for foreign in [false, true] {
        with_planning_task(|runtime, fixture, repo, task, state| {
            runtime.block_on(async {
                let issue = task.linear_id.as_ref().unwrap().as_str();
                let original = state.lock().await.issues[0].clone();
                if !foreign {
                    let mut provider = state.lock().await;
                    mark_issue_updated(&mut provider.issues[0]);
                    drop(provider);
                    super::test_fixture::refresh_task_input(repo, issue, LinearRefresh::Force)
                        .await
                        .unwrap();
                }
                let work = save_field_name(fixture, task, false, "Pending local title");
                let saved = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
                let receipt = field_receipts(fixture, task, false)[0].clone();
                let workflow = fixture.store.sqlite.task_graph(&task.id).unwrap();
                {
                    let mut provider = state.lock().await;
                    if foreign {
                        let mut project = planning_project("foreign-project", "foreign-project");
                        project["initiatives"] = json!({"nodes":[{"id":"foreign-initiative"}]});
                        provider.extra_projects.push(project.clone());
                        provider.issues[0]["project"] = project;
                        mark_issue_updated(&mut provider.issues[0]);
                    } else {
                        provider.issues[0] = original;
                    }
                }
                let error = crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                    .await
                    .unwrap_err();
                assert!(
                    error.to_string().contains("field delivery deferred"),
                    "{error}"
                );
                assert_eq!(state.lock().await.field_writes, 0);
                assert_eq!(fixture.store.sqlite.task(&task.id).unwrap().unwrap(), saved);
                assert_eq!(fixture.store.sqlite.task_graph(&task.id).unwrap(), workflow);
                let pending = field_receipts(fixture, task, false);
                assert_eq!(pending.len(), 1);
                assert_eq!(pending[0].id, receipt.id);
                let attempted: bool = rusqlite::Connection::open(&fixture.database)
                    .unwrap()
                    .query_row(
                        "SELECT attempted FROM task_changes WHERE id=?1",
                        [&receipt.id],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert!(!attempted);
                if foreign {
                    let retained = fixture.retained_task(repo, issue);
                    assert_eq!(retained.item.project_id.as_deref(), Some("foreign-project"));
                    assert!(!fixture
                        .store
                        .sqlite
                        .accepted_task_revision(&task.id, issue, retained.item.revision.as_deref())
                        .unwrap());
                }
            });
        });
    }
}

#[test]
fn linear_on_demand_adopts_new_open_issues_without_allocating_execution() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            let mut issue = state.lock().await.issues[0].clone();
            issue["id"] = json!("new-open-issue");
            issue["identifier"] = json!("FIX-352");
            issue["title"] = json!("New work in another Project");
            issue["project"]["id"] = json!("new-project");
            mark_issue_updated(&mut issue);
            state.lock().await.issues.push(issue);
            rusqlite::Connection::open(&fixture.database)
                .unwrap()
                .execute("DELETE FROM linear_team_checks", [])
                .unwrap();
            crate::ops::linear_observe::check_repository(&fixture.store, repo.to_str().unwrap())
                .await
                .unwrap();
            let adopted = fixture
                .store
                .get_task_by_issue("new-open-issue")
                .await
                .unwrap()
                .unwrap();
            assert_eq!(adopted.wave_id, task.wave_id);
            assert_eq!(adopted.title, "New work in another Project");
            assert_ne!(adopted.project_id, task.project_id);
            assert!(adopted.worktree.is_none());
            assert!(fixture
                .store
                .sqlite
                .task_graph(&adopted.id)
                .unwrap()
                .is_none());
            assert!(fixture
                .store
                .sqlite
                .task_prs(&adopted.id)
                .unwrap()
                .is_empty());
        });
    });
}

// Child entry uses the real CLI parser and callback handler, with only the
// credential/network boundary replaced by the same stand-in as the parent.
#[test]
fn linear_on_demand_hook_child() {
    use clap::Parser;
    let Ok(args) = std::env::var("LINEAR_FIXTURE_ARGS") else {
        return;
    };
    let cli = crate::lf::Cli::try_parse_from(serde_json::from_str::<Vec<String>>(&args).unwrap())
        .unwrap();
    let Some(crate::lf::Commands::ContextBlock {
        delivery,
        moment: crate::context_block::ContextMoment::Stop,
        ..
    }) = cli.command
    else {
        panic!("generated turn hook must select the planning callback")
    };
    let delivery =
        delivery.unwrap_or_else(|| std::env::var_os("LF_CONTEXT_DELIVERY").unwrap().into());
    let context: crate::context_block::ContextDelivery =
        serde_json::from_slice(&std::fs::read(&delivery).unwrap()).unwrap();
    let path = context.home.join("loopflow.db");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = Arc::new(
        runtime
            .block_on(crate::store::open_store(
                &crate::store::StorageConfig::sqlite(path.clone()),
            ))
            .unwrap(),
    );
    let context = super::LinearTestContext {
        path,
        store,
        graphql_url: std::env::var("LINEAR_FIXTURE_URL").unwrap(),
    };
    let ready = std::env::var("LINEAR_FIXTURE_READY").unwrap();
    std::fs::write(ready, "ready").unwrap();
    let start = PathBuf::from(std::env::var("LINEAR_FIXTURE_START").unwrap());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !start.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "parent did not release hooks"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    LINEAR_TEST_CONTEXT.sync_scope(context, || {
        crate::lf::commands::context::emit_block(
            Some(&delivery),
            crate::context_block::ContextMoment::Stop,
        )
        .unwrap();
    });
}

#[test]
fn linear_on_demand_native_hooks_share_freshness_across_processes() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_BIN"]);
        let shim = fixture.directory.path().join("lf-hook");
        // A test-binary adapter replaces main's dispatch only. The generated
        // shell command, parser, callback, store and acquisition all execute.
        std::fs::write(&shim, format!(
            "#!/usr/bin/env python3\nimport json, os, sys\nos.environ['LINEAR_FIXTURE_ARGS'] = json.dumps(['lf'] + sys.argv[1:])\nos.execv({}, [{}, '--exact', 'ops::linear::task_planning_tests::linear_on_demand_hook_child', '--nocapture'])\n",
            serde_json::to_string(&std::env::current_exe().unwrap()).unwrap(),
            serde_json::to_string(&std::env::current_exe().unwrap()).unwrap(),
        )).unwrap();
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("LF_BIN", &shim);
        let delivery = crate::context_block::ContextDelivery {
            repository: None,
            repo: repo.to_owned(),
            home: fixture.directory.path().to_owned(),
            wave: None,
            skill_file: None,
            submitted_docs: None,
            references: Vec::new(),
        };
        let claude = crate::harness::context::claude_args(&delivery).unwrap();
        let settings: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&claude[1]).unwrap()).unwrap();
        let codex = crate::harness::context::codex_config(&delivery, None).unwrap();
        let hooks = [
            &settings["hooks"]["Stop"][0]["hooks"][0]["command"],
            &codex["hooks"]["Stop"][0]["hooks"][0]["command"],
        ];
        runtime.block_on(async {
            seed_finished_tasks(fixture, repo, &state).await;
            let mut provider = state.lock().await;
            provider.requests = 0;
            provider.issues[0]["title"] = json!("Acquired through native Stop");
            mark_issue_updated(&mut provider.issues[0]);
        });
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        conn.execute("DELETE FROM linear_team_checks", []).unwrap();
        let start = fixture.directory.path().join("release-hooks");
        let spawn_hook = |hook: &serde_json::Value, ready: &Path| {
            let mut command = std::process::Command::new("/bin/sh");
            for (name, _) in
                std::env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("LF_"))
            {
                command.env_remove(name);
            }
            delivery.configure_hooks(&mut command).unwrap();
            command
                .args(["-c", hook.as_str().unwrap()])
                .env(
                    "LINEAR_FIXTURE_URL",
                    LINEAR_TEST_CONTEXT.with(|ctx| ctx.graphql_url.clone()),
                )
                .env("LINEAR_FIXTURE_READY", ready)
                .env("LINEAR_FIXTURE_START", &start)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        };
        let mut children: Vec<_> = (0..4)
            .map(|index| {
                let ready = fixture.directory.path().join(format!("ready-{index}"));
                let child = spawn_hook(hooks[index % 2], &ready);
                (ready, child)
            })
            .collect();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while children.iter().any(|(ready, _)| !ready.exists()) {
            assert!(
                std::time::Instant::now() < deadline,
                "hooks did not reach barrier"
            );
            for (_, child) in &mut children {
                assert!(
                    child.try_wait().unwrap().is_none(),
                    "hook exited before barrier"
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        std::fs::write(&start, "go").unwrap();
        for (_, child) in children {
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        // A later process starts after the acquisition lock has been released;
        // only the persisted stamp can suppress this request.
        let output = spawn_hook(hooks[0], &fixture.directory.path().join("ready-later"))
            .wait_with_output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        runtime.block_on(async {
            // One delta and its changed Project across every process.
            assert_eq!(state.lock().await.requests, 2);
            assert_eq!(
                fixture
                    .store
                    .get_task(&task.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .title,
                "Acquired through native Stop"
            );
        });
        let watermark: Option<String> = conn
            .query_row("SELECT synced_through FROM linear_team_checks", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(watermark.is_some());
    });
}

#[test]
fn repeated_legacy_abandonment_delivers_missing_cancellation_once() {
    with_planning_task(|runtime, fixture, _repo, task, state| {
        fixture
            .store
            .sqlite
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET abandoned_at=1,planning_state='canceled' WHERE id=?1",
                [task.id.as_str()],
            )
            .unwrap();
        let work = crate::durable::WorkRef::Task(task.id.clone());
        runtime.block_on(async {
            fixture
                .store
                .abandon(&work, "explicit legacy cancellation")
                .await
                .unwrap();
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            fixture.store.abandon(&work, "repeat").await.unwrap();
            crate::ops::linear_observe::sync_task_state(&fixture.store, task)
                .await
                .unwrap();
            let provider = state.lock().await;
            assert_eq!(provider.issues[0]["state"]["type"], "canceled");
            assert_eq!(provider.completion_writes, 1);
        });
        assert!(fixture
            .store
            .sqlite
            .pending_task_state(&task.id)
            .unwrap()
            .is_none());
    });
}
