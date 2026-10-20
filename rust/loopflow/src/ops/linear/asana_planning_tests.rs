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

const PROJECT: &str = "1200000000000001";

#[derive(Default)]
struct Asana {
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
        "name": input["name"], "notes": input["notes"],
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
    let Some(task) = asana.tasks.get_mut(&external(&gid)) else {
        return missing();
    };
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
        "text": body["data"]["text"], "resource_subtype": "comment_added",
        "created_by": {"gid": "100", "name": "Loopflow"},
    });
    thread.push(story.clone());
    data(story)
}

async fn serve(state: Shared) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new()
        .route("/tasks", axum::routing::post(create))
        .route("/tasks/{gid}", get(read).put(update).delete(remove))
        .route("/tasks/{gid}/stories", get(stories).post(comment))
        .with_state(state);
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, server)
}

async fn asana_repo(fixture: &Fixture) -> PathBuf {
    let (repo, _) = fixture.planning_repo().await;
    std::fs::write(
        repo.join(".lf/config.yaml"),
        format!("planning:\n  provider: asana\n  project: \"{PROJECT}\"\n"),
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
            Some("Mirror Tasks into Asana".into()),
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
        assert_eq!(task["notes"], "Mirror Tasks into Asana");
        assert_eq!(task["completed"], false);
    });
}

#[test]
fn asana_status_title_and_comments_write_back_to_the_mirrored_task() {
    with_asana(|runtime, fixture, repo, state| {
        let saved =
            crate::ops::task::task_create(repo, None, Some("Write back".into()), None).unwrap();
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
            Some("Delivered once"),
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
        assert_eq!(thread[0]["text"], "Delivered once");
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
            crate::ops::task::task_create(repo, None, Some("Saved offline".into()), None).unwrap();
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
            crate::ops::task::task_create(repo, None, Some("Take direction".into()), None).unwrap();
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
                "gid": "7001", "created_at": created, "text": "Also cover retries",
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
            .any(|steer| steer.text.contains("Also cover retries")
                && steer.text.contains("Jack Heart")));
        assert!(steers
            .iter()
            .any(|steer| steer.text.contains("Take direction from Asana")));
    });
}
