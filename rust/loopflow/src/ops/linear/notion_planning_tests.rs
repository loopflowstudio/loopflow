//! Task planning through the Notion provider against a stateful Notion stand-in.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Path as Segment, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::Json;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::test_fixture::{now, token, Fixture, PlanningEnvironment};
use super::LINEAR_TEST_CONTEXT;

/// The team page, and the Tasks database people already made on it.
const TEAM: &str = "2aaa8f99-3d81-8141-bd3d-f63d878a7c74";
const DATABASE: &str = "3fff8f99-3d81-8141-bd3d-f63d878a7c74";

struct Notion {
    /// Properties by database.
    databases: BTreeMap<String, Value>,
    pages: BTreeMap<String, Value>,
    /// Child blocks by parent page or block, without their own children.
    children: BTreeMap<String, Vec<Value>>,
    comments: BTreeMap<String, Vec<Value>>,
    creations: usize,
    ids: u32,
    clock: u32,
    offline: bool,
}

impl Default for Notion {
    fn default() -> Self {
        Self {
            // A database people made: a title, and Status with Notion's default groups.
            databases: BTreeMap::from([(
                DATABASE.to_string(),
                json!({
                    "Name": {"type": "title", "title": {}},
                    "Status": {"type": "status", "status": {
                        "options": [
                            {"id": "s1", "name": "Not started"},
                            {"id": "s2", "name": "In progress"},
                            {"id": "s3", "name": "Done"}
                        ],
                        "groups": [
                            {"name": "To-do", "option_ids": ["s1"]},
                            {"name": "In progress", "option_ids": ["s2"]},
                            {"name": "Complete", "option_ids": ["s3"]}
                        ]
                    }},
                }),
            )]),
            pages: BTreeMap::new(),
            children: BTreeMap::from([(
                TEAM.to_string(),
                vec![json!({
                    "id": DATABASE, "type": "child_database", "has_children": false,
                    "child_database": {"title": "Tasks"},
                })],
            )]),
            comments: BTreeMap::new(),
            creations: 0,
            ids: 0,
            clock: 0,
            offline: false,
        }
    }
}

impl Notion {
    fn tick(&mut self) -> String {
        self.clock += 1;
        format!("2026-10-20T12:{:02}:00.000Z", self.clock)
    }

    fn id(&mut self) -> String {
        self.ids += 1;
        format!("00000000-0000-4000-8000-{:012}", self.ids)
    }

    /// Store blocks as Notion does: ids assigned, rich text read back with plain text.
    fn append(&mut self, parent: &str, blocks: &[Value]) {
        for block in blocks {
            let mut block = block.clone();
            let id = self.id();
            let block_type = block["type"].as_str().unwrap().to_string();
            let nested = block[&block_type]
                .as_object_mut()
                .and_then(|data| data.remove("children"));
            if let Some(rich_text) = block[&block_type]["rich_text"].as_array_mut() {
                for span in rich_text {
                    span["plain_text"] = span["text"]["content"].clone();
                    span["href"] = span["text"]["link"]["url"].clone();
                }
            }
            block["id"] = json!(id);
            block["has_children"] = json!(nested.is_some());
            self.children
                .entry(parent.to_string())
                .or_default()
                .push(block);
            if let Some(Value::Array(nested)) = nested {
                self.append(&id, &nested);
            }
        }
    }

    fn touch(&mut self, page: &str) {
        let edited = self.tick();
        if let Some(page) = self.pages.get_mut(page) {
            page["last_edited_time"] = json!(edited);
        }
    }

    fn identity(page: &Value) -> &str {
        page["properties"]["Loopflow ID"]["rich_text"][0]["text"]["content"]
            .as_str()
            .unwrap_or_default()
    }
}

type Shared = Arc<Mutex<Notion>>;

fn missing() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"object": "error", "code": "object_not_found", "message": "missing"})),
    )
        .into_response()
}

fn offline() -> Response {
    StatusCode::SERVICE_UNAVAILABLE.into_response()
}

/// Properties as Notion reports them, each carrying its type. A new status
/// property takes Notion's default options and groups.
fn typed(properties: &Value) -> serde_json::Map<String, Value> {
    let default_status = Notion::default().databases[DATABASE]["Status"]["status"].clone();
    properties
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, property)| {
            let kind = property.as_object().unwrap().keys().next().unwrap().clone();
            let mut config = property[&kind].clone();
            if kind == "status" && config == json!({}) {
                config = default_status.clone();
            }
            (name.clone(), json!({"type": kind, kind.clone(): config}))
        })
        .collect()
}

async fn database(State(state): State<Shared>, Segment(db): Segment<String>) -> Response {
    let notion = state.lock().await;
    if notion.offline {
        return offline();
    }
    match notion.databases.get(&db) {
        Some(properties) => Json(json!({"id": db, "properties": properties})).into_response(),
        None => missing(),
    }
}

async fn update_database(
    State(state): State<Shared>,
    Segment(db): Segment<String>,
    Json(body): Json<Value>,
) -> Response {
    let mut notion = state.lock().await;
    let properties = notion.databases.get_mut(&db).unwrap();
    for (name, property) in typed(&body["properties"]) {
        properties[name] = property;
    }
    Json(json!({"id": db, "properties": properties})).into_response()
}

async fn create_database(State(state): State<Shared>, Json(body): Json<Value>) -> Response {
    let mut notion = state.lock().await;
    let id = notion.id();
    let parent = body["parent"]["page_id"].as_str().unwrap().to_string();
    let title = body["title"][0]["text"]["content"].clone();
    notion
        .databases
        .insert(id.clone(), Value::Object(typed(&body["properties"])));
    notion.children.entry(parent).or_default().push(json!({
        "id": id, "type": "child_database", "has_children": false,
        "child_database": {"title": title},
    }));
    Json(json!({"object": "database", "id": id})).into_response()
}

async fn query(
    State(state): State<Shared>,
    Segment(db): Segment<String>,
    Json(body): Json<Value>,
) -> Response {
    let notion = state.lock().await;
    if notion.offline {
        return offline();
    }
    // Lookups filter by Loopflow ID; the change feed reads every live page.
    let wanted = body["filter"]["rich_text"]["equals"].as_str();
    let results: Vec<Value> = notion
        .pages
        .values()
        .filter(|page| {
            page["parent"]["database_id"] == db.as_str()
                && page["archived"] == false
                && wanted.is_none_or(|id| Notion::identity(page) == id)
        })
        .cloned()
        .collect();
    Json(json!({"results": results, "has_more": false, "next_cursor": null})).into_response()
}

async fn create(State(state): State<Shared>, Json(body): Json<Value>) -> Response {
    let mut notion = state.lock().await;
    if notion.offline {
        return offline();
    }
    let id = notion.id();
    let edited = notion.tick();
    let page = json!({
        "object": "page", "id": id, "archived": false, "last_edited_time": edited,
        "url": format!("https://www.notion.so/{}", id.replace('-', "")),
        "parent": {"type": "database_id", "database_id": body["parent"]["database_id"]},
        "properties": body["properties"],
    });
    notion.pages.insert(id.clone(), page.clone());
    notion.creations += 1;
    if let Some(children) = body["children"].as_array() {
        notion.append(&id, children);
    }
    Json(page).into_response()
}

async fn read(State(state): State<Shared>, Segment(id): Segment<String>) -> Response {
    match state.lock().await.pages.get(&id) {
        Some(page) => Json(page.clone()).into_response(),
        None => missing(),
    }
}

async fn update(
    State(state): State<Shared>,
    Segment(id): Segment<String>,
    Json(body): Json<Value>,
) -> Response {
    let mut notion = state.lock().await;
    let edited = notion.tick();
    let Some(page) = notion.pages.get_mut(&id) else {
        return missing();
    };
    if let Some(archived) = body.get("archived") {
        page["archived"] = archived.clone();
    }
    if let Some(properties) = body["properties"].as_object() {
        for (name, value) in properties {
            page["properties"][name] = value.clone();
        }
    }
    page["last_edited_time"] = json!(edited);
    Json(page.clone()).into_response()
}

async fn children(State(state): State<Shared>, Segment(parent): Segment<String>) -> Response {
    let notion = state.lock().await;
    let results = notion.children.get(&parent).cloned().unwrap_or_default();
    Json(json!({"results": results, "has_more": false, "next_cursor": null})).into_response()
}

async fn append(
    State(state): State<Shared>,
    Segment(parent): Segment<String>,
    Json(body): Json<Value>,
) -> Response {
    let mut notion = state.lock().await;
    notion.append(&parent, body["children"].as_array().unwrap());
    notion.touch(&parent);
    Json(json!({"results": []})).into_response()
}

async fn remove_block(State(state): State<Shared>, Segment(id): Segment<String>) -> Response {
    let mut notion = state.lock().await;
    let owner = notion
        .children
        .iter()
        .find(|(_, blocks)| blocks.iter().any(|block| block["id"] == id))
        .map(|(parent, _)| parent.clone());
    let Some(owner) = owner else {
        return missing();
    };
    notion
        .children
        .get_mut(&owner)
        .unwrap()
        .retain(|block| block["id"] != id);
    notion.touch(&owner);
    Json(json!({"id": id, "archived": true})).into_response()
}

async fn comments(
    State(state): State<Shared>,
    Query(query): Query<BTreeMap<String, String>>,
) -> Response {
    let notion = state.lock().await;
    let results = notion
        .comments
        .get(&query["block_id"])
        .cloned()
        .unwrap_or_default();
    Json(json!({"results": results, "has_more": false, "next_cursor": null})).into_response()
}

async fn comment(State(state): State<Shared>, Json(body): Json<Value>) -> Response {
    let mut notion = state.lock().await;
    let id = notion.id();
    let created = notion.tick();
    let mut rich_text = body["rich_text"].clone();
    for span in rich_text.as_array_mut().unwrap() {
        span["plain_text"] = span["text"]["content"].clone();
        span["href"] = span["text"]["link"]["url"].clone();
    }
    let comment = json!({
        "id": id, "created_time": created, "rich_text": rich_text,
        "created_by": {"object": "user", "id": "b0000000-0000-4000-8000-000000000001"},
    });
    let page = body["parent"]["page_id"].as_str().unwrap().to_string();
    notion
        .comments
        .entry(page)
        .or_default()
        .push(comment.clone());
    Json(comment).into_response()
}

async fn user(Segment(id): Segment<String>) -> Response {
    let name = if id.starts_with('b') {
        "Loopflow"
    } else {
        "Jack Heart"
    };
    Json(json!({"id": id, "name": name})).into_response()
}

async fn serve(state: Shared) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new()
        .route("/databases", post(create_database))
        .route("/databases/{db}", get(database).patch(update_database))
        .route("/databases/{db}/query", post(query))
        .route("/pages", post(create))
        .route("/pages/{id}", get(read).patch(update))
        .route("/blocks/{id}/children", get(children).patch(append))
        .route("/blocks/{id}", delete(remove_block))
        .route("/comments", get(comments).post(comment))
        .route("/users/{id}", get(user))
        .with_state(state);
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, server)
}

async fn notion_repo(fixture: &Fixture) -> PathBuf {
    let (repo, _) = fixture.planning_repo().await;
    // People paste the team page's URL; the bare ID it carries is the scope.
    std::fs::write(
        repo.join(".lf/config.yaml"),
        format!(
            "planning:\n  provider: notion\n  team: \"https://www.notion.so/acme/Engineering-{}\"\n",
            TEAM.replace('-', "")
        ),
    )
    .unwrap();
    let mut credential = token("notion-access", "notion-refresh", now() + 86_400);
    credential.provider = "notion".into();
    fixture
        .store
        .upsert_provider_token(&credential)
        .await
        .unwrap();
    repo
}

fn with_notion(test: impl FnOnce(&tokio::runtime::Runtime, &Fixture, &Path, Shared)) {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    let repo = runtime.block_on(notion_repo(&fixture));
    let state = Shared::default();
    let (url, server) = runtime.block_on(serve(state.clone()));
    LINEAR_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        test(&runtime, &fixture, &repo, state)
    });
    server.abort();
}

/// The live page carrying a Loopflow ID.
fn mirrored(runtime: &tokio::runtime::Runtime, state: &Shared, id: &str) -> Value {
    runtime
        .block_on(state.lock())
        .pages
        .values()
        .find(|page| Notion::identity(page) == id)
        .cloned()
        .expect("Task reached Notion")
}

fn body_types(runtime: &tokio::runtime::Runtime, state: &Shared, page: &Value) -> Vec<String> {
    runtime.block_on(state.lock()).children[page["id"].as_str().unwrap()]
        .iter()
        .map(|block| block["type"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn notion_task_create_mirrors_task_as_a_database_page() {
    with_notion(|runtime, _fixture, repo, state| {
        let saved = crate::ops::task::task_create(
            repo,
            None,
            Some("Ship the Notion provider".into()),
            Some("Mirror **Tasks** into Notion\n\n- pages\n- comments".into()),
            None,
        )
        .unwrap();
        let id = saved
            .linear_id
            .as_ref()
            .expect("Task reached Notion")
            .as_str();
        // Notion has no ticket numbers; the Task keeps Loopflow's own identifier.
        assert!(id.replace('-', "").starts_with(&saved.identifier));
        let page = mirrored(runtime, &state, id);
        assert_eq!(
            page["properties"]["Name"]["title"][0]["text"]["content"],
            "Ship the Notion provider"
        );
        assert_eq!(
            page["properties"]["Status"]["status"]["name"],
            "Not started"
        );
        // The description is the page body, as blocks.
        assert_eq!(
            body_types(runtime, &state, &page),
            [
                "paragraph",
                "paragraph",
                "bulleted_list_item",
                "bulleted_list_item"
            ]
        );
        // The Tasks database adapted to the mirror instead of refusing it, and
        // the page gained the Projects database its relation links to.
        let notion = runtime.block_on(state.lock());
        let tasks = &notion.databases[DATABASE];
        assert_eq!(tasks["Loopflow ID"]["type"], "rich_text");
        let projects = tasks["Project"]["relation"]["database_id"]
            .as_str()
            .unwrap();
        assert_eq!(notion.databases[projects]["Name"]["type"], "title");
        assert_eq!(
            notion.children[TEAM][1]["child_database"]["title"],
            "Projects"
        );
    });
}

#[test]
fn notion_team_page_without_databases_gains_tasks_and_projects() {
    with_notion(|runtime, _fixture, repo, state| {
        runtime.block_on(async {
            let mut notion = state.lock().await;
            notion.children.clear();
            notion.databases.clear();
        });
        let saved =
            crate::ops::task::task_create(repo, None, Some("First Task".into()), None, None)
                .unwrap();
        let page = mirrored(runtime, &state, saved.linear_id.as_ref().unwrap().as_str());
        let notion = runtime.block_on(state.lock());
        let titles: Vec<&Value> = notion.children[TEAM]
            .iter()
            .map(|block| &block["child_database"]["title"])
            .collect();
        assert_eq!(titles, [&json!("Projects"), &json!("Tasks")]);
        let tasks = notion.children[TEAM][1]["id"].as_str().unwrap();
        assert_eq!(page["parent"]["database_id"], tasks);
        assert_eq!(notion.databases[tasks]["Project"]["type"], "relation");
    });
}

#[test]
fn notion_status_title_body_and_comments_write_back_to_the_page() {
    with_notion(|runtime, fixture, repo, state| {
        let saved =
            crate::ops::task::task_create(repo, None, Some("Write back".into()), None, None)
                .unwrap();
        let id = saved.linear_id.clone().unwrap();
        let id = id.as_str();
        let status = |state: &Shared| {
            mirrored(runtime, state, id)["properties"]["Status"]["status"]["name"].clone()
        };

        crate::ops::task::task_complete(repo, saved.id.as_str(), None).unwrap();
        assert_eq!(status(&state), "Done");

        crate::ops::task::task_reopen(saved.id.as_str(), None).unwrap();
        assert_eq!(status(&state), "Not started");

        crate::ops::task::task_edit(
            repo,
            saved.id.as_str(),
            None,
            crate::work::task::TaskUpdate {
                title: Some("Write back, renamed".into()),
                description: Some("# Renamed\nNew body".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let page = mirrored(runtime, &state, id);
        assert_eq!(
            page["properties"]["Name"]["title"][0]["text"]["content"],
            "Write back, renamed"
        );
        assert_eq!(
            body_types(runtime, &state, &page),
            ["heading_1", "paragraph"]
        );

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
        let thread = runtime.block_on(state.lock()).comments[page["id"].as_str().unwrap()].clone();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0]["rich_text"][1]["annotations"]["bold"], true);
        let comments = fixture.store.sqlite.task_comments(&saved.id).unwrap();
        assert_eq!(comments.comments.len(), 1);
        assert!(comments.pending_sync.is_empty());
    });
}

#[test]
fn notion_outage_keeps_the_local_save_and_publishes_once_on_retry() {
    with_notion(|runtime, fixture, repo, state| {
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
fn notion_comments_and_edits_by_people_become_task_direction() {
    with_notion(|runtime, fixture, repo, state| {
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
        let page = mirrored(runtime, &state, &id)["id"]
            .as_str()
            .unwrap()
            .to_string();
        runtime.block_on(async {
            let mut notion = state.lock().await;
            let created = notion.tick();
            notion
                .comments
                .entry(page.clone())
                .or_default()
                .push(json!({
                    "id": "a0000000-0000-4000-8000-000000007001", "created_time": created,
                    "rich_text": [
                        {"plain_text": "Also cover ", "annotations": {}},
                        {"plain_text": "retries", "annotations": {"italic": true}}
                    ],
                    "created_by": {"object": "user", "id": "a0000000-0000-4000-8000-000000000002"},
                }));
            let edited = notion.tick();
            let page = notion.pages.get_mut(&page).unwrap();
            page["properties"]["Name"]["title"] =
                json!([{"plain_text": "Take direction from Notion"}]);
            page["last_edited_time"] = json!(edited);
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
            .any(|steer| steer.text.contains("Take direction from Notion")));
        let local = runtime
            .block_on(fixture.store.get_task(&saved.id))
            .unwrap()
            .unwrap();
        assert_eq!(local.title, "Take direction from Notion");
    });
}

#[test]
fn notion_renames_and_completion_reach_the_task() {
    with_notion(|runtime, fixture, repo, state| {
        let saved =
            crate::ops::task::task_create(repo, None, Some("Original".into()), None, None).unwrap();
        let id = saved.linear_id.clone().unwrap().as_str().to_string();
        let page = mirrored(runtime, &state, &id)["id"]
            .as_str()
            .unwrap()
            .to_string();
        let refresh = || {
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
            runtime
                .block_on(fixture.store.get_task(&saved.id))
                .unwrap()
                .unwrap()
        };
        let edit_in_notion = |title: &str, status: &str| {
            runtime.block_on(async {
                let mut notion = state.lock().await;
                let edited = notion.tick();
                let page = notion.pages.get_mut(&page).unwrap();
                page["properties"]["Name"]["title"] = json!([{"plain_text": title}]);
                page["properties"]["Status"]["status"]["name"] = json!(status);
                page["last_edited_time"] = json!(edited);
            })
        };

        // The change sent last wins: a rename saved offline goes out after a
        // concurrent rename in Notion, so it replaces it.
        runtime.block_on(state.lock()).offline = true;
        crate::ops::task::task_edit(
            repo,
            saved.id.as_str(),
            None,
            crate::work::task::TaskUpdate {
                title: Some("Renamed locally".into()),
                ..Default::default()
            },
        )
        .unwrap();
        runtime.block_on(state.lock()).offline = false;
        edit_in_notion("Renamed in Notion", "Not started");
        assert_eq!(refresh().title, "Renamed locally");
        runtime.block_on(crate::ops::linear_observe::sync_repository(
            &fixture.store,
            repo.to_str().unwrap(),
        ));
        assert_eq!(
            runtime.block_on(state.lock()).pages[&page]["properties"]["Name"]["title"][0]["text"]
                ["content"],
            "Renamed locally"
        );

        // Notion's next rename and completion, sent after it, reach the Task.
        edit_in_notion("Renamed in Notion again", "Done");
        let task = refresh();
        assert_eq!(task.title, "Renamed in Notion again");
        assert!(runtime
            .block_on(fixture.store.task_state(&saved.id))
            .unwrap()
            .is_terminal());
    });
}

#[test]
fn notion_pages_coworkers_create_become_tasks() {
    with_notion(|runtime, fixture, repo, state| {
        runtime.block_on(async {
            let mut notion = state.lock().await;
            let id = notion.id();
            let edited = notion.tick();
            notion.pages.insert(
                id.clone(),
                json!({
                    "object": "page", "id": id, "archived": false, "last_edited_time": edited,
                    "url": "https://www.notion.so/filed", "parent": {"database_id": DATABASE},
                    "properties": {
                        "Name": {"title": [{"plain_text": "Filed in Notion"}]},
                        "Status": {"status": {"name": "Not started"}},
                    },
                }),
            );
            notion.append(
                &id,
                &[json!({"type": "paragraph", "paragraph": {"rich_text": [
                    {"type": "text", "text": {"content": "From the meeting"}}
                ]}})],
            );
        });
        runtime.block_on(crate::ops::linear_observe::sync_repository(
            &fixture.store,
            repo.to_str().unwrap(),
        ));
        let tasks = runtime.block_on(fixture.store.list_tasks(None)).unwrap();
        let task = tasks
            .iter()
            .find(|task| task.title == "Filed in Notion")
            .expect("the Notion page became a Task");
        assert_eq!(task.description, "From the meeting");
        // The page now carries the Loopflow ID every machine derives: its own ID.
        let id = task.linear_id.as_ref().unwrap().as_str();
        let page = mirrored(runtime, &state, id);
        assert_eq!(page["id"], id);
    });
}

#[test]
fn notion_pages_coworkers_archive_leave_the_plan() {
    with_notion(|runtime, fixture, repo, state| {
        let saved = crate::ops::task::task_create(
            repo,
            None,
            Some("Archived in Notion".into()),
            None,
            None,
        )
        .unwrap();
        let id = saved.linear_id.clone().unwrap().as_str().to_string();
        let page = mirrored(runtime, &state, &id)["id"]
            .as_str()
            .unwrap()
            .to_string();
        runtime.block_on(state.lock()).pages.get_mut(&page).unwrap()["archived"] = json!(true);
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
fn notion_pages_move_to_in_progress_when_work_starts() {
    with_notion(|runtime, fixture, repo, state| {
        let saved =
            crate::ops::task::task_create(repo, None, Some("Start here".into()), None, None)
                .unwrap();
        let id = saved.linear_id.clone().unwrap().as_str().to_string();
        // Checkout claims the Task; the fixture has no fetchable origin.
        fixture.store.sqlite.claim_task(&saved.id).unwrap();
        runtime.block_on(crate::ops::linear_observe::sync_repository(
            &fixture.store,
            repo.to_str().unwrap(),
        ));
        assert_eq!(
            mirrored(runtime, &state, &id)["properties"]["Status"]["status"]["name"],
            "In progress"
        );
    });
}

#[test]
fn notion_pages_show_the_pr() {
    use crate::work::task::{GithubPr, PrPublication, TaskPr, TaskPrId};
    with_notion(|runtime, fixture, repo, state| {
        let saved =
            crate::ops::task::task_create(repo, None, Some("Ship it".into()), None, None).unwrap();
        let id = saved.linear_id.clone().unwrap().as_str().to_string();
        let task = runtime
            .block_on(fixture.store.get_task(&saved.id))
            .unwrap()
            .unwrap();
        let at = time::OffsetDateTime::now_utc();
        let mut pr = TaskPr {
            id: TaskPrId::new(),
            task_id: task.id.clone(),
            sequence: 1,
            slug: "ship".into(),
            branch: "jack/ship".into(),
            base_commit: "0".repeat(40),
            parent_pr_id: None,
            publication: Some(PrPublication {
                requested_at: at,
                presentation: None,
                github: Some(GithubPr {
                    number: 42,
                    url: "https://github.com/loopflowstudio/fixture/pull/42".into(),
                    head_sha: None,
                }),
                merge: None,
            }),
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: at,
            updated_at: at,
        };
        // Publishing twice in one state comments once.
        for _ in 0..2 {
            runtime.block_on(crate::ops::task::link_pr_to_planning(
                &fixture.store,
                &task,
                &mut pr,
            ));
        }
        assert_eq!(pr.linear_link_error, None);
        let page = mirrored(runtime, &state, &id)["id"]
            .as_str()
            .unwrap()
            .to_string();
        let thread = runtime.block_on(state.lock()).comments[&page].clone();
        assert_eq!(thread.len(), 1);
        let spans = &thread[0]["rich_text"];
        assert_eq!(spans[0]["plain_text"], "GitHub PR #42");
        assert_eq!(
            spans[0]["href"],
            "https://github.com/loopflowstudio/fixture/pull/42"
        );
        assert_eq!(spans[1]["plain_text"], " — Open");
    });
}

#[test]
fn notion_wave_projects_become_project_pages_related_to_their_tasks() {
    with_notion(|runtime, fixture, repo, state| {
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
        let page = mirrored(
            runtime,
            &state,
            saved
                .linear_id
                .as_ref()
                .expect("the Task reached Notion after its Project")
                .as_str(),
        );
        let project = runtime
            .block_on(
                fixture
                    .store
                    .get_project(saved.project_id.as_ref().unwrap()),
            )
            .unwrap()
            .unwrap();
        let project_page = mirrored(
            runtime,
            &state,
            project
                .linear_id
                .as_ref()
                .expect("the Project reached Notion")
                .as_str(),
        );
        assert_eq!(
            project_page["properties"]["Name"]["title"][0]["text"]["content"],
            project.name.as_str()
        );
        assert_eq!(
            page["properties"]["Project"]["relation"],
            json!([{"id": project_page["id"]}])
        );

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
