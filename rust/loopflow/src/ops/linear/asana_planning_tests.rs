//! Task planning through the Asana provider against a stateful Asana stand-in.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Path as Segment, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Json;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::test_fixture::{now, token, Fixture, PlanningEnvironment};
use super::LINEAR_TEST_CONTEXT;

const TEAM: &str = "1100000000000001";
/// The gid the stand-in gives the repository's team project when Loopflow creates it.
const PROJECT: &str = "1200000000000001";

#[derive(Default)]
struct Asana {
    /// The team's projects.
    projects: Vec<Value>,
    tasks: BTreeMap<String, Value>,
    stories: BTreeMap<String, Vec<Value>>,
    creations: usize,
    clock: u32,
    offline: bool,
}

impl Asana {
    fn tick(&mut self) -> String {
        self.clock += 1;
        format!("2026-10-19T12:00:{:02}.000Z", self.clock)
    }
}

type Shared = Arc<Mutex<Asana>>;

fn external(gid: &str) -> String {
    gid.strip_prefix("external:")
        .expect("Loopflow addresses tasks by external identity")
        .to_string()
}

/// A task addressed by `external:<id>` or by Asana's own gid.
fn key(asana: &Asana, gid: &str) -> Option<String> {
    match gid.strip_prefix("external:") {
        Some(id) => asana.tasks.contains_key(id).then(|| id.to_string()),
        None => owner(asana, gid),
    }
}

/// Asana lists stories only under its own task gid.
fn owner(asana: &Asana, gid: &str) -> Option<String> {
    asana
        .tasks
        .iter()
        .find(|(_, task)| task["gid"] == gid)
        .map(|(id, _)| id.clone())
}

fn data(value: Value) -> Response {
    Json(json!({ "data": value })).into_response()
}

fn missing() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"errors":[{"message":"task: Unknown object"}]})),
    )
        .into_response()
}

async fn create(State(state): State<Shared>, Json(body): Json<Value>) -> Response {
    let mut asana = state.lock().await;
    if asana.offline {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let input = &body["data"];
    let id = input["external"]["gid"].as_str().unwrap().to_string();
    let modified = asana.tick();
    let task = json!({
        "gid": format!("9{}", asana.creations), "external": {"gid": id, "data": null},
        "name": input["name"], "html_notes": input["html_notes"],
        "completed": input["completed"].as_bool().unwrap_or(false), "completed_at": null,
        "modified_at": modified, "permalink_url": format!("https://app.asana.com/0/{PROJECT}/{id}"),
        "due_on": input["due_on"], "assignee": null,
        "memberships": input["projects"].as_array().unwrap().iter()
            .map(|project| json!({"project": {"gid": project}})).collect::<Vec<_>>(),
    });
    asana.creations += 1;
    asana.tasks.insert(id, task.clone());
    data(task)
}

async fn read(State(state): State<Shared>, Segment(gid): Segment<String>) -> Response {
    let asana = state.lock().await;
    if asana.offline {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    match asana.tasks.get(&external(&gid)) {
        Some(task) => data(task.clone()),
        None => missing(),
    }
}

async fn update(
    State(state): State<Shared>,
    Segment(gid): Segment<String>,
    Json(body): Json<Value>,
) -> Response {
    let mut asana = state.lock().await;
    if asana.offline {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let modified = asana.tick();
    let Some(mut id) = key(&asana, &gid) else {
        return missing();
    };
    // Claiming a task created in Asana files it under its new Loopflow identity.
    if let Some(claimed) = body["data"]["external"]["gid"].as_str() {
        let task = asana.tasks.remove(&id).unwrap();
        let thread = asana.stories.remove(&id);
        id = claimed.to_string();
        asana.tasks.insert(id.clone(), task);
        if let Some(thread) = thread {
            asana.stories.insert(id.clone(), thread);
        }
    }
    let task = asana.tasks.get_mut(&id).unwrap();
    for (field, value) in body["data"].as_object().unwrap() {
        task[field] = value.clone();
    }
    task["modified_at"] = json!(modified);
    task["completed_at"] = if task["completed"] == true {
        json!(modified)
    } else {
        Value::Null
    };
    data(task.clone())
}

async fn team_projects(State(state): State<Shared>, Segment(team): Segment<String>) -> Response {
    assert_eq!(team, TEAM);
    data(json!(state.lock().await.projects))
}

async fn create_project(
    State(state): State<Shared>,
    Segment(team): Segment<String>,
    Json(body): Json<Value>,
) -> Response {
    assert_eq!(team, TEAM);
    let mut asana = state.lock().await;
    let gid = format!("12000000000000{:02}", asana.projects.len() + 1);
    let modified = asana.tick();
    let project = json!({"gid": gid, "name": body["data"]["name"],
        "html_notes": body["data"]["html_notes"], "modified_at": modified});
    asana.projects.push(project.clone());
    data(project)
}

/// `addProject` and `removeProject`, by Asana's own task gid.
async fn membership(
    State(state): State<Shared>,
    Segment((gid, action)): Segment<(String, String)>,
    Json(body): Json<Value>,
) -> Response {
    let mut asana = state.lock().await;
    let modified = asana.tick();
    let Some(id) = owner(&asana, &gid) else {
        return missing();
    };
    let project = body["data"]["project"].clone();
    let task = asana.tasks.get_mut(&id).unwrap();
    let memberships = task["memberships"].as_array_mut().unwrap();
    memberships.retain(|member| member["project"]["gid"] != project);
    if action == "addProject" {
        memberships.push(json!({"project": {"gid": project}}));
    }
    task["modified_at"] = json!(modified);
    data(json!({}))
}

async fn project_tasks(State(state): State<Shared>) -> Response {
    let asana = state.lock().await;
    if asana.offline {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    data(json!(asana.tasks.values().cloned().collect::<Vec<_>>()))
}

async fn remove(State(state): State<Shared>, Segment(gid): Segment<String>) -> Response {
    match state.lock().await.tasks.remove(&external(&gid)) {
        Some(_) => data(json!({})),
        None => missing(),
    }
}

async fn stories(State(state): State<Shared>, Segment(gid): Segment<String>) -> Response {
    let asana = state.lock().await;
    let Some(id) = owner(&asana, &gid) else {
        return missing();
    };
    data(json!(asana.stories.get(&id).cloned().unwrap_or_default()))
}

async fn comment(
    State(state): State<Shared>,
    Segment(gid): Segment<String>,
    Json(body): Json<Value>,
) -> Response {
    let mut asana = state.lock().await;
    let created = asana.tick();
    let id = external(&gid);
    let thread = asana.stories.entry(id).or_default();
    let story = json!({
        "gid": format!("8{}", thread.len()), "created_at": created,
        "html_text": body["data"]["html_text"], "resource_subtype": "comment_added",
        "created_by": {"gid": "100", "name": "Loopflow"},
    });
    thread.push(story.clone());
    data(story)
}

async fn serve(state: Shared) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new()
        .route(
            "/teams/{team}/projects",
            get(team_projects).post(create_project),
        )
        .route("/tasks", axum::routing::post(create))
        .route("/projects/{project}/tasks", get(project_tasks))
        .route("/tasks/{gid}", get(read).put(update).delete(remove))
        .route("/tasks/{gid}/stories", get(stories).post(comment))
        .route("/tasks/{gid}/{action}", axum::routing::post(membership))
        .with_state(state);
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, server)
}

async fn asana_repo(fixture: &Fixture) -> PathBuf {
    let (repo, _) = fixture.planning_repo().await;
    std::fs::write(
        repo.join(".lf/config.yaml"),
        format!("planning:\n  provider: asana\n  team: \"https://app.asana.com/0/team/{TEAM}/overview\"\n"),
    )
    .unwrap();
    let mut credential = token("asana-access", "asana-refresh", now() + 86_400);
    credential.provider = "asana".into();
    fixture
        .store
        .upsert_provider_token(&credential)
        .await
        .unwrap();
    repo
}

fn with_asana(test: impl FnOnce(&tokio::runtime::Runtime, &Fixture, &Path, Shared)) {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    let repo = runtime.block_on(asana_repo(&fixture));
    let state = Shared::default();
    let (url, server) = runtime.block_on(serve(state.clone()));
    LINEAR_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        test(&runtime, &fixture, &repo, state)
    });
    server.abort();
}

fn mirrored(runtime: &tokio::runtime::Runtime, state: &Shared, id: &str) -> Value {
    runtime.block_on(state.lock()).tasks[id].clone()
}

#[test]
fn asana_task_create_mirrors_task_under_its_loopflow_identity() {
    with_asana(|runtime, _fixture, repo, state| {
        let saved = crate::ops::task::task_create(
            repo,
            None,
            Some("Ship the Asana provider".into()),
            Some("Mirror **Tasks** into Asana".into()),
            None,
        )
        .unwrap();
        let id = saved
            .linear_id
            .as_ref()
            .expect("Task reached Asana")
            .as_str();
        // Asana has no ticket numbers; the Task keeps Loopflow's own identifier.
        assert!(id.replace('-', "").starts_with(&saved.identifier));
        let task = mirrored(runtime, &state, id);
        assert_eq!(task["name"], "Ship the Asana provider");
        // The Task lives in the team project Loopflow made for the repository.
        let projects = runtime.block_on(state.lock()).projects.clone();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0]["gid"], PROJECT);
        assert_eq!(projects[0]["name"], "fixture");
        assert_eq!(task["memberships"], json!([{"project": {"gid": PROJECT}}]));
        // Asana renders rich text, so markdown arrives formatted.
        assert_eq!(
            task["html_notes"],
            "<body>Mirror <strong>Tasks</strong> into Asana</body>"
        );
        assert_eq!(task["completed"], false);
    });
}

#[test]
fn asana_status_title_and_comments_write_back_to_the_mirrored_task() {
    with_asana(|runtime, fixture, repo, state| {
        let saved =
            crate::ops::task::task_create(repo, None, Some("Write back".into()), None, None)
                .unwrap();
        let id = saved.linear_id.clone().unwrap();
        let id = id.as_str();

        crate::ops::task::task_complete(repo, saved.id.as_str(), None).unwrap();
        assert_eq!(mirrored(runtime, &state, id)["completed"], true);

        crate::ops::task::task_reopen(saved.id.as_str(), None).unwrap();
        assert_eq!(mirrored(runtime, &state, id)["completed"], false);

        crate::ops::task::task_edit(
            repo,
            saved.id.as_str(),
            None,
            crate::work::task::TaskUpdate {
                title: Some("Write back, renamed".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(mirrored(runtime, &state, id)["name"], "Write back, renamed");

        crate::ops::task::task_comment(
            repo,
            saved.id.as_str(),
            None,
            Some("Delivered **once**"),
            false,
        )
        .unwrap();
        // A later delivery pass and observation neither repost nor echo it.
        runtime.block_on(crate::ops::linear_observe::sync_repository(
            &fixture.store,
            repo.to_str().unwrap(),
        ));
        let task = runtime
            .block_on(fixture.store.get_task(&saved.id))
            .unwrap()
            .unwrap();
        runtime
            .block_on(crate::ops::linear_observe::refresh_task_comments(
                &fixture.store,
                &task,
            ))
            .unwrap();
        let thread = runtime.block_on(state.lock()).stories[id].clone();
        assert_eq!(thread.len(), 1);
        assert_eq!(
            thread[0]["html_text"],
            "<body>Delivered <strong>once</strong></body>"
        );
        let comments = fixture.store.sqlite.task_comments(&saved.id).unwrap();
        assert_eq!(comments.comments.len(), 1);
        assert!(comments.pending_sync.is_empty());
    });
}

#[test]
fn asana_outage_keeps_the_local_save_and_publishes_once_on_retry() {
    with_asana(|runtime, fixture, repo, state| {
        runtime.block_on(state.lock()).offline = true;
        let saved =
            crate::ops::task::task_create(repo, None, Some("Saved offline".into()), None, None)
                .unwrap();
        assert!(saved.linear_id.is_none());

        runtime.block_on(state.lock()).offline = false;
        runtime.block_on(crate::ops::linear_observe::sync_repository(
            &fixture.store,
            repo.to_str().unwrap(),
        ));
        let task = runtime
            .block_on(fixture.store.get_task(&saved.id))
            .unwrap()
            .unwrap();
        assert!(task.linear_id.is_some());
        assert_eq!(runtime.block_on(state.lock()).creations, 1);
    });
}

#[test]
fn asana_comments_and_edits_by_people_become_task_direction() {
    with_asana(|runtime, fixture, repo, state| {
        let saved =
            crate::ops::task::task_create(repo, None, Some("Take direction".into()), None, None)
                .unwrap();
        let id = saved.linear_id.clone().unwrap().as_str().to_string();
        let task = runtime
            .block_on(fixture.store.get_task(&saved.id))
            .unwrap()
            .unwrap();
        let refresh = || {
            runtime
                .block_on(crate::ops::linear_observe::refresh_task_comments(
                    &fixture.store,
                    &task,
                ))
                .unwrap()
        };
        refresh();
        runtime.block_on(async {
            let mut asana = state.lock().await;
            let created = asana.tick();
            asana.stories.entry(id.clone()).or_default().push(json!({
                "gid": "7001", "created_at": created,
                "html_text": "<body>Also cover <em>retries</em></body>",
                "resource_subtype": "comment_added",
                "created_by": {"gid": "1204429107704617", "name": "Jack Heart"},
            }));
            let modified = asana.tick();
            let task = asana.tasks.get_mut(&id).unwrap();
            task["name"] = json!("Take direction from Asana");
            task["modified_at"] = json!(modified);
        });
        refresh();
        let steers = runtime
            .block_on(fixture.store.task_steers(&saved.id))
            .unwrap();
        assert!(steers
            .iter()
            .any(|steer| steer.text.contains("Also cover *retries*")
                && steer.text.contains("Jack Heart")));
        assert!(steers
            .iter()
            .any(|steer| steer.text.contains("Take direction from Asana")));
        let local = runtime
            .block_on(fixture.store.get_task(&saved.id))
            .unwrap()
            .unwrap();
        assert_eq!(local.title, "Take direction from Asana");
    });
}

#[test]
fn asana_tasks_coworkers_create_become_tasks() {
    with_asana(|runtime, fixture, repo, state| {
        runtime.block_on(async {
            let mut asana = state.lock().await;
            // The team already has the repository's project, beside others.
            asana.projects = vec![
                json!({"gid": "1200000000000009", "name": "Roadmap"}),
                json!({"gid": PROJECT, "name": "fixture"}),
            ];
            let modified = asana.tick();
            asana.tasks.insert(
                "coworker".into(),
                json!({
                    "gid": "7100", "external": null, "name": "Filed in Asana",
                    "html_notes": "<body>From the <strong>meeting</strong></body>",
                    "completed": false, "completed_at": null, "modified_at": modified,
                    "permalink_url": "https://app.asana.com/0/1/7100", "due_on": null,
                    "assignee": null, "memberships": [{"project": {"gid": PROJECT}}],
                }),
            );
        });
        runtime.block_on(crate::ops::linear_observe::sync_repository(
            &fixture.store,
            repo.to_str().unwrap(),
        ));
        let tasks = runtime.block_on(fixture.store.list_tasks(None)).unwrap();
        let task = tasks
            .iter()
            .find(|task| task.title == "Filed in Asana")
            .expect("the Asana task became a Task");
        assert_eq!(task.description, "From the **meeting**");
        // Asana now carries the Loopflow identity every machine derives.
        let asana = runtime.block_on(state.lock());
        let id = task.linear_id.as_ref().unwrap().as_str();
        assert_eq!(asana.tasks[id]["external"]["gid"], id);
        assert_eq!(asana.projects.len(), 2, "the existing project was reused");
    });
}

#[test]
fn asana_tasks_coworkers_delete_leave_the_plan() {
    with_asana(|runtime, fixture, repo, state| {
        let saved =
            crate::ops::task::task_create(repo, None, Some("Deleted in Asana".into()), None, None)
                .unwrap();
        let id = saved.linear_id.clone().unwrap().as_str().to_string();
        runtime.block_on(state.lock()).tasks.remove(&id);
        let task = runtime
            .block_on(fixture.store.get_task(&saved.id))
            .unwrap()
            .unwrap();
        runtime
            .block_on(crate::ops::linear_observe::refresh_task_comments(
                &fixture.store,
                &task,
            ))
            .unwrap();
        let tasks = runtime.block_on(fixture.store.list_tasks(None)).unwrap();
        assert!(tasks.iter().all(|task| task.id != saved.id));
    });
}

#[test]
fn asana_wave_projects_become_team_projects_holding_their_tasks() {
    with_asana(|runtime, fixture, repo, state| {
        runtime
            .block_on(crate::ops::project::ensure(repo, "product"))
            .unwrap();
        let saved = crate::ops::task::task_create(
            repo,
            Some("product"),
            Some("Planned work".into()),
            None,
            None,
        )
        .unwrap();
        let id = saved
            .linear_id
            .clone()
            .expect("the Task reached Asana after its Project");
        let project = runtime
            .block_on(
                fixture
                    .store
                    .get_project(saved.project_id.as_ref().unwrap()),
            )
            .unwrap()
            .unwrap();
        let mirrored = project
            .linear_id
            .clone()
            .expect("the Project reached Asana");
        let (repository, wave_project, task) = runtime.block_on(async {
            let asana = state.lock().await;
            let repository = asana
                .projects
                .iter()
                .find(|candidate| candidate["name"] == "fixture")
                .cloned()
                .unwrap();
            let wave_project = asana
                .projects
                .iter()
                .find(|candidate| candidate["name"] == project.name)
                .cloned()
                .expect("the Wave Project is an Asana project in the team");
            (repository, wave_project, asana.tasks[id.as_str()].clone())
        });
        // Asana projects take no caller ID; the notes' last line carries it.
        assert!(wave_project["html_notes"]
            .as_str()
            .unwrap()
            .contains(&format!("Loopflow ID: {}", mirrored.as_str())));
        let members: Vec<&Value> = task["memberships"]
            .as_array()
            .unwrap()
            .iter()
            .map(|member| &member["project"]["gid"])
            .collect();
        assert_eq!(members, [&repository["gid"], &wave_project["gid"]]);

        // Read back, the Task keeps its Project.
        runtime.block_on(crate::ops::linear_observe::sync_repository(
            &fixture.store,
            repo.to_str().unwrap(),
        ));
        let local = runtime
            .block_on(fixture.store.get_task(&saved.id))
            .unwrap()
            .unwrap();
        assert_eq!(local.project_id, saved.project_id);
    });
}
