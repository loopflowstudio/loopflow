//! Notion delivery and observation. Work owns local content; this adapter owns provider facts.
//!
//! The repository's planning scope is a Notion page, its team. The page holds a
//! Tasks database and a Projects database linked by a `Project` relation, the
//! shape of Notion's Projects & Tasks template; Loopflow finds both on first use
//! by title, or creates them. Each Task is one page of the Tasks database. Notion accepts no caller IDs, so every exported page carries its
//! Loopflow UUID in a `Loopflow ID` text property and every request finds the
//! page by that exact value. The Task's planning identity stays the one Loopflow
//! assigned, and a lost creation reply is resolved by lookup rather than a second
//! create. Notion returns the shared planning observations, so acceptance,
//! receipts and steering stay one path with Linear and Asana.
//!
//! The Tasks database adapts to the mirror on first use: Loopflow adds the `Loopflow ID`
//! property, and a `Status` property when the database tracks no state. A status
//! property's Notion groups (To-do, In progress, Complete) carry Task state; a
//! checkbox works too. A `date` property named Due and a `people` property
//! named Assignee mirror those fields when the database has them.

mod blocks;

use std::sync::Arc;

use reqwest::{Method, StatusCode};
use serde_json::{json, Value};
use tokio::sync::OnceCell;

use crate::linear::{
    local_identifier, LinearComment, LinearError, LinearIssue, LinearIssueObservation,
    LinearResult, OrderedIssue, PlanningResponse,
};

const NOTION_BASE_URL: &str = "https://api.notion.com/v1";
const NOTION_VERSION: &str = "2022-06-28";
const IDENTITY_PROPERTY: &str = "Loopflow ID";
const PROJECT_PROPERTY: &str = "Project";
const TASKS: &str = "Tasks";
const PROJECTS: &str = "Projects";
/// Notion's largest page, and its largest children append.
const PAGE_SIZE: usize = 100;

#[derive(Debug, Clone)]
pub struct NotionClient {
    client: reqwest::Client,
    token: String,
    /// The team page holding the databases: the planning scope, like a Linear Team.
    team: String,
    base_url: String,
    databases: Arc<OnceCell<Databases>>,
    schema: Arc<OnceCell<Schema>>,
}

/// The team page's two databases.
#[derive(Debug, Clone)]
struct Databases {
    tasks: String,
    projects: String,
}

/// The database properties the mirror reads and writes.
#[derive(Debug, Clone)]
struct Schema {
    title: String,
    state: StateProperty,
    due: Option<String>,
    assignee: Option<String>,
}

#[derive(Debug, Clone)]
enum StateProperty {
    /// Option names by Notion status group.
    Status {
        name: String,
        todo: Vec<String>,
        started: Vec<String>,
        complete: Vec<String>,
    },
    Checkbox(String),
}

impl NotionClient {
    pub fn new(token: String, team: &str) -> Self {
        Self::with_base_url(token, team, NOTION_BASE_URL.to_string())
    }

    pub(crate) fn with_base_url(token: String, team: &str, base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
            team: notion_id(team),
            base_url,
            databases: Arc::default(),
            schema: Arc::default(),
        }
    }

    /// Create the exported page in the repository database. `input` is the saved
    /// export receipt: `id`, `title`, `description`, `assigneeId`, `dueDate`, `stateId`.
    pub(crate) async fn create_task(&self, input: &Value) -> LinearResult<PlanningResponse> {
        let id = input["id"]
            .as_str()
            .ok_or_else(|| message("Notion export has no Loopflow identity"))?;
        let schema = self.schema().await?;
        let mut properties = properties(schema, input)?;
        properties[IDENTITY_PROPERTY] = json!({"rich_text": blocks::plain(id)});
        if input.get("stateId").is_none() {
            properties[&state_name(schema)] = state_value(schema, "unstarted")?;
        }
        let body = blocks::to_blocks(input["description"].as_str().unwrap_or_default());
        let (first, rest) = body.split_at(body.len().min(PAGE_SIZE));
        let page: Value = self
            .send(
                Method::POST,
                "/pages",
                Some(json!({
                    "parent": {"database_id": self.tasks().await?},
                    "properties": properties,
                    "children": first,
                })),
            )
            .await?;
        let page_id = object_id(&page)?;
        self.append(&page_id, rest).await?;
        self.response(id, &page_id).await
    }

    pub(crate) async fn task(&self, id: &str) -> LinearResult<Option<LinearIssue>> {
        match self.page(id).await? {
            Some(page) => Ok(Some(self.issue(id, &page).await?)),
            None => Ok(None),
        }
    }

    /// Field edits share the export input's names. A description replaces the page body.
    pub(crate) async fn update_task(
        &self,
        id: &str,
        input: &Value,
    ) -> LinearResult<PlanningResponse> {
        let page_id = object_id(&self.require_page(id).await?)?;
        let properties = properties(self.schema().await?, input)?;
        if properties
            .as_object()
            .is_some_and(|fields| !fields.is_empty())
        {
            let _: Value = self
                .send(
                    Method::PATCH,
                    &format!("/pages/{page_id}"),
                    Some(json!({"properties": properties})),
                )
                .await?;
        }
        if let Some(description) = input.get("description") {
            for block in self.children(&page_id).await? {
                let _: Value = self
                    .send(
                        Method::DELETE,
                        &format!("/blocks/{}", object_id(&block)?),
                        None,
                    )
                    .await?;
            }
            let body = blocks::to_blocks(description.as_str().unwrap_or_default());
            self.append(&page_id, &body).await?;
        }
        self.response(id, &page_id).await
    }

    /// `completed`, `unstarted` or `canceled`; canceled closes the page too.
    pub(crate) async fn set_state(&self, id: &str, target: &str) -> LinearResult<LinearIssue> {
        let page_id = object_id(&self.require_page(id).await?)?;
        let schema = self.schema().await?;
        let page: Value = self
            .send(
                Method::PATCH,
                &format!("/pages/{page_id}"),
                Some(json!({"properties": {state_name(schema): state_value(schema, target)?}})),
            )
            .await?;
        self.issue(id, &page).await
    }

    /// Notion archives pages; an archived page no longer answers lookups.
    pub(crate) async fn delete_task(&self, id: &str) -> LinearResult<()> {
        let page_id = object_id(&self.require_page(id).await?)?;
        let _: Value = self
            .send(
                Method::PATCH,
                &format!("/pages/{page_id}"),
                Some(json!({"archived": true})),
            )
            .await?;
        Ok(())
    }

    /// Post `text` once. Notion comments carry no caller identity, so a Loopflow
    /// comment is its visible text on the page: an existing match is the delivery.
    pub(crate) async fn sync_comment(&self, issue: &str, text: &str) -> LinearResult<()> {
        let page_id = object_id(&self.require_page(issue).await?)?;
        let comments = self.comments(&page_id).await.map_err(|error| match error {
            Failure::Restricted => message(
                "Notion refused comment access; enable Read comments and Insert comments \
                 for the Loopflow integration in Notion's integration settings",
            ),
            Failure::Provider(error) => error,
        })?;
        if comments.iter().any(|comment| comment.body == text) {
            return Ok(());
        }
        let _: Value = self
            .send(
                Method::POST,
                "/comments",
                Some(json!({"parent": {"page_id": page_id}, "rich_text": blocks::spans(text)})),
            )
            .await?;
        Ok(())
    }

    pub(crate) async fn observe(&self, id: &str) -> LinearResult<LinearIssueObservation> {
        self.observation(&self.require_page(id).await?).await
    }

    /// Pages in the repository database edited in `since..`, with their
    /// observations. A page someone created in Notion has no Loopflow ID yet; it
    /// takes its own page ID, so every machine claims the same one. Completed
    /// pages Loopflow never tracked, and `finished` ones, are skipped.
    pub(crate) async fn changes(
        &self,
        since: Option<&str>,
        live: &[String],
        finished: &[String],
    ) -> LinearResult<Vec<(LinearIssue, LinearIssueObservation)>> {
        let schema = self.schema().await?;
        let mut pages = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut query = json!({"page_size": PAGE_SIZE});
            if let Some(since) = since {
                query["filter"] = json!({
                    "timestamp": "last_edited_time",
                    "last_edited_time": {"on_or_after": since},
                });
            }
            if let Some(cursor) = &cursor {
                query["start_cursor"] = json!(cursor);
            }
            let found: Value = self
                .send(
                    Method::POST,
                    &format!("/databases/{}/query", self.tasks().await?),
                    Some(query),
                )
                .await?;
            pages.extend(found["results"].as_array().cloned().unwrap_or_default());
            match found["next_cursor"].as_str() {
                Some(next) if found["has_more"] == true => cursor = Some(next.to_owned()),
                _ => break,
            }
        }
        let mut changes = Vec::new();
        for mut page in pages {
            let claimed = identity(&page);
            let id = if claimed.is_empty() {
                object_id(&page)?
            } else {
                claimed.clone()
            };
            let complete = matches!(observed_state(schema, &page), "completed" | "canceled");
            if finished.contains(&id) || (complete && !live.contains(&id)) {
                continue;
            }
            if claimed.is_empty() {
                page = self
                    .send(
                        Method::PATCH,
                        &format!("/pages/{}", object_id(&page)?),
                        Some(json!({"properties": {IDENTITY_PROPERTY: {"rich_text": blocks::plain(&id)}}})),
                    )
                    .await?;
            }
            let observation = self.observation(&page).await?;
            changes.push((self.issue(&id, &page).await?, observation));
        }
        Ok(changes)
    }

    async fn observation(&self, page: &Value) -> LinearResult<LinearIssueObservation> {
        let page_id = object_id(page)?;
        let schema = self.schema().await?;
        // Without comment access, title and description still direct the Task.
        let mut comments = match self.comments(&page_id).await {
            Ok(comments) => comments,
            Err(Failure::Restricted) => {
                tracing::warn!("Notion integration cannot read comments; observing the page only");
                Vec::new()
            }
            Err(Failure::Provider(error)) => return Err(error),
        };
        comments.sort_by(|left, right| {
            left.revision
                .cmp(&right.revision)
                .then(left.id.cmp(&right.id))
        });
        Ok(LinearIssueObservation {
            revision: page["last_edited_time"].as_str().unwrap_or_default().into(),
            title: title(schema, page),
            description: self.body(&page_id).await?,
            comments,
        })
    }

    /// Whether the page at `url` is archived or in the trash.
    pub(crate) async fn removed(&self, url: Option<&str>) -> LinearResult<bool> {
        let Some(page) = url
            .map(notion_id)
            .filter(|id| uuid::Uuid::parse_str(id).is_ok())
        else {
            return Ok(false);
        };
        match self
            .request::<Value>(Method::GET, &format!("/pages/{page}"), None)
            .await
        {
            Ok(page) => Ok(page["archived"] == true || page["in_trash"] == true),
            Err(Failure::Provider(error)) if error.to_string().contains("Could not find") => {
                Ok(true)
            }
            Err(error) => Err(error.into()),
        }
    }

    // ── Pages ───────────────────────────────────────────────────────

    /// The live page carrying `id`, found by its exact `Loopflow ID`.
    async fn page(&self, id: &str) -> LinearResult<Option<Value>> {
        self.schema().await?;
        let found: Value = self
            .send(
                Method::POST,
                &format!("/databases/{}/query", self.tasks().await?),
                Some(json!({
                    "filter": {"property": IDENTITY_PROPERTY, "rich_text": {"equals": id}},
                    "page_size": 2,
                })),
            )
            .await?;
        let pages = found["results"].as_array().cloned().unwrap_or_default();
        match pages.as_slice() {
            [] => Ok(None),
            [page] => Ok(Some(page.clone())),
            _ => Err(message(format!(
                "Notion database holds several pages with Loopflow ID {id}"
            ))),
        }
    }

    async fn require_page(&self, id: &str) -> LinearResult<Value> {
        self.page(id)
            .await?
            .ok_or_else(|| message("Notion page is unavailable"))
    }

    async fn response(&self, id: &str, page_id: &str) -> LinearResult<PlanningResponse> {
        let page: Value = self
            .send(Method::GET, &format!("/pages/{page_id}"), None)
            .await?;
        Ok(PlanningResponse::Task {
            issue: Box::new(OrderedIssue {
                item: self.issue(id, &page).await?,
                priority_sort_order: 0.0,
                sort_order: 0.0,
            }),
            project: None,
        })
    }

    async fn issue(&self, id: &str, page: &Value) -> LinearResult<LinearIssue> {
        let schema = self.schema().await?;
        let properties = &page["properties"];
        if identity(page) != id {
            return Err(message(
                "Notion returned a page with a different Loopflow identity",
            ));
        }
        let state = observed_state(schema, page);
        let tasks = self.tasks().await?;
        let in_scope = page["parent"]["database_id"]
            .as_str()
            .is_some_and(|parent| notion_id(parent) == tasks);
        Ok(LinearIssue {
            revision: page["last_edited_time"].as_str().map(str::to_owned),
            id: id.to_string(),
            identifier: local_identifier(id),
            branch_name: None,
            url: page["url"].as_str().map(str::to_owned),
            name: title(schema, page),
            description: self.body(&object_id(page)?).await?,
            rank: 0,
            completed: matches!(state, "completed" | "canceled"),
            completed_at: None,
            due_date: schema.due.as_ref().and_then(|due| {
                properties[due]["date"]["start"]
                    .as_str()
                    .map(|start| start.chars().take(10).collect())
            }),
            state: Some(state.into()),
            project_id: None,
            team_id: in_scope.then(|| self.team.clone()),
            assignee: schema.assignee.as_ref().and_then(|assignee| {
                properties[assignee]["people"][0]["id"]
                    .as_str()
                    .map(str::to_owned)
            }),
        })
    }

    // ── Blocks and comments ─────────────────────────────────────────

    async fn body(&self, page_id: &str) -> LinearResult<String> {
        Ok(blocks::to_markdown(&self.tree(page_id).await?))
    }

    /// The block tree under `parent`, each block's children under its `children`.
    async fn tree(&self, parent: &str) -> LinearResult<Vec<Value>> {
        let mut children = self.children(parent).await?;
        for block in &mut children {
            if block["has_children"].as_bool() != Some(true) || block["type"] == "child_page" {
                continue;
            }
            let nested = Box::pin(self.tree(&object_id(block)?)).await?;
            let block_type = block["type"].as_str().unwrap_or_default().to_string();
            block[&block_type]["children"] = json!(nested);
        }
        Ok(children)
    }

    async fn children(&self, parent: &str) -> LinearResult<Vec<Value>> {
        Ok(self
            .list(&format!("/blocks/{parent}/children"), &[])
            .await?)
    }

    async fn append(&self, page_id: &str, body: &[Value]) -> LinearResult<()> {
        for chunk in body.chunks(PAGE_SIZE) {
            let _: Value = self
                .send(
                    Method::PATCH,
                    &format!("/blocks/{page_id}/children"),
                    Some(json!({"children": chunk})),
                )
                .await?;
        }
        Ok(())
    }

    async fn comments(&self, page_id: &str) -> Result<Vec<LinearComment>, Failure> {
        let comments = self.list("/comments", &[("block_id", page_id)]).await?;
        let mut names = std::collections::HashMap::new();
        for comment in &comments {
            if let Some(author) = comment["created_by"]["id"].as_str() {
                if !names.contains_key(author) {
                    names.insert(author.to_owned(), self.user_name(author).await);
                }
            }
        }
        Ok(comments
            .into_iter()
            .map(|comment| LinearComment {
                id: comment["id"].as_str().unwrap_or_default().into(),
                revision: comment["created_time"].as_str().map(str::to_owned),
                created_at: comment["created_time"].as_str().map(str::to_owned),
                body: blocks::rich_text_markdown(
                    comment["rich_text"]
                        .as_array()
                        .map(Vec::as_slice)
                        .unwrap_or(&[]),
                ),
                author_id: comment["created_by"]["id"].as_str().map(str::to_owned),
                author_name: comment["created_by"]["id"]
                    .as_str()
                    .and_then(|author| names.get(author).cloned().flatten()),
            })
            .collect())
    }

    /// A comment author's display name. Comments name only the user's ID, and
    /// reading names needs a capability the integration may lack.
    async fn user_name(&self, id: &str) -> Option<String> {
        let user: Value = self
            .request(Method::GET, &format!("/users/{id}"), None)
            .await
            .ok()?;
        user["name"].as_str().map(str::to_owned)
    }

    // ── Databases ───────────────────────────────────────────────────

    async fn tasks(&self) -> LinearResult<&str> {
        Ok(&self.databases().await?.tasks)
    }

    /// The team page's Tasks and Projects databases, found by title or created
    /// once per client. A database people made keeps its own properties.
    async fn databases(&self) -> LinearResult<&Databases> {
        self.databases
            .get_or_try_init(|| async {
                let children = self.children(&self.team).await?;
                let titled = |title: &str| -> LinearResult<Option<String>> {
                    let found: Vec<&Value> = children
                        .iter()
                        .filter(|block| {
                            block["type"] == "child_database"
                                && block["child_database"]["title"]
                                    .as_str()
                                    .is_some_and(|name| name.trim().eq_ignore_ascii_case(title))
                        })
                        .collect();
                    match found.as_slice() {
                        [] => Ok(None),
                        [database] => object_id(database).map(Some),
                        _ => Err(message(format!(
                            "Notion page holds several {title} databases; rename all but one"
                        ))),
                    }
                };
                let (tasks, projects) = (titled(TASKS)?, titled(PROJECTS)?);
                let projects = match projects {
                    Some(projects) => projects,
                    None => {
                        self.create_database(
                            PROJECTS,
                            json!({
                                "Name": {"title": {}},
                                "Status": {"status": {}},
                                IDENTITY_PROPERTY: {"rich_text": {}},
                            }),
                        )
                        .await?
                    }
                };
                let tasks = match tasks {
                    Some(tasks) => tasks,
                    None => {
                        self.create_database(
                            TASKS,
                            json!({
                                "Name": {"title": {}},
                                "Status": {"status": {}},
                                "Due": {"date": {}},
                                "Assignee": {"people": {}},
                                IDENTITY_PROPERTY: {"rich_text": {}},
                                PROJECT_PROPERTY: relation(&projects),
                            }),
                        )
                        .await?
                    }
                };
                Ok(Databases {
                    tasks: notion_id(&tasks),
                    projects: notion_id(&projects),
                })
            })
            .await
    }

    async fn create_database(&self, title: &str, properties: Value) -> LinearResult<String> {
        let database: Value = self
            .send(
                Method::POST,
                "/databases",
                Some(json!({
                    "parent": {"type": "page_id", "page_id": self.team},
                    "title": blocks::plain(title),
                    "properties": properties,
                })),
            )
            .await?;
        object_id(&database)
    }

    /// Read the Tasks database once per client, adding the mirror's missing properties.
    async fn schema(&self) -> LinearResult<&Schema> {
        self.schema
            .get_or_try_init(|| async {
                let databases = self.databases().await?;
                let path = format!("/databases/{}", databases.tasks);
                let mut database: Value = self.send(Method::GET, &path, None).await?;
                let mut missing = serde_json::Map::new();
                if database["properties"][IDENTITY_PROPERTY]["type"] != "rich_text" {
                    missing.insert(IDENTITY_PROPERTY.into(), json!({"rich_text": {}}));
                }
                if state_property(&database["properties"]).is_none() {
                    missing.insert("Status".into(), json!({"status": {}}));
                }
                if database["properties"][PROJECT_PROPERTY]["type"] != "relation" {
                    missing.insert(PROJECT_PROPERTY.into(), relation(&databases.projects));
                }
                if !missing.is_empty() {
                    database = self
                        .send(Method::PATCH, &path, Some(json!({"properties": missing})))
                        .await?;
                }
                read_schema(&database["properties"])
            })
            .await
    }

    // ── Transport ───────────────────────────────────────────────────

    async fn list(&self, path: &str, query: &[(&str, &str)]) -> Result<Vec<Value>, Failure> {
        let mut results = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut url = format!("{path}?page_size={PAGE_SIZE}");
            for (key, value) in query {
                url.push_str(&format!("&{key}={value}"));
            }
            if let Some(cursor) = &cursor {
                url.push_str(&format!("&start_cursor={cursor}"));
            }
            let page: Value = self.request(Method::GET, &url, None).await?;
            results.extend(page["results"].as_array().cloned().unwrap_or_default());
            match page["next_cursor"].as_str() {
                Some(next) if page["has_more"].as_bool() == Some(true) => {
                    cursor = Some(next.to_owned())
                }
                _ => break,
            }
        }
        Ok(results)
    }

    async fn send<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        data: Option<Value>,
    ) -> LinearResult<T> {
        self.request(method, path, data).await.map_err(Into::into)
    }

    async fn request<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        data: Option<Value>,
    ) -> Result<T, Failure> {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base_url))
            .bearer_auth(&self.token)
            .header("Notion-Version", NOTION_VERSION);
        if let Some(data) = data {
            request = request.json(&data);
        }
        let response = request
            .send()
            .await
            .map_err(|error| message(format!("Notion request failed: {error}")))?;
        let status = response.status();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<i64>().ok());
        let body = response
            .bytes()
            .await
            .map_err(|error| message(format!("failed to read Notion response: {error}")))?;
        if status == StatusCode::TOO_MANY_REQUESTS {
            let now = time::OffsetDateTime::now_utc().unix_timestamp() * 1000;
            return Err(LinearError::RateLimited {
                reset_at_ms: now + retry_after.unwrap_or(60) * 1000,
            }
            .into());
        }
        if !status.is_success() {
            let error = serde_json::from_slice::<Value>(&body).unwrap_or_default();
            if error["code"] == "restricted_resource" {
                return Err(Failure::Restricted);
            }
            let detail = error["message"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("HTTP {status}"));
            return Err(message(format!("Notion rejected the request: {detail}")).into());
        }
        serde_json::from_slice(&body)
            .map_err(|error| message(format!("invalid Notion response: {error}")).into())
    }
}

#[derive(Debug)]
enum Failure {
    /// The integration lacks the capability this endpoint needs.
    Restricted,
    Provider(LinearError),
}

impl From<LinearError> for Failure {
    fn from(error: LinearError) -> Self {
        Self::Provider(error)
    }
}

impl From<Failure> for LinearError {
    fn from(failure: Failure) -> Self {
        match failure {
            Failure::Restricted => message("Notion integration lacks access to this resource"),
            Failure::Provider(error) => error,
        }
    }
}

fn message(text: impl Into<String>) -> LinearError {
    LinearError::Message(text.into())
}

/// A Loopflow comment as Notion returns it: its visible text read back from rich text.
pub(crate) fn comment_markdown(text: &str) -> String {
    blocks::rich_text_markdown(&blocks::spans(text))
}

/// A two-way relation to the Projects database, as Notion's template links them.
fn relation(projects: &str) -> Value {
    json!({"relation": {"database_id": projects, "type": "dual_property", "dual_property": {}}})
}

/// Notion IDs appear hyphenated, bare, or at the end of a page URL.
pub(crate) fn notion_id(raw: &str) -> String {
    let hex: String = raw
        .rsplit(['/', '-', '?', '#'])
        .find(|part| part.len() == 32 && part.chars().all(|ch| ch.is_ascii_hexdigit()))
        .map(str::to_owned)
        .unwrap_or_else(|| raw.chars().filter(|ch| *ch != '-').collect());
    match uuid::Uuid::parse_str(&hex) {
        Ok(uuid) => uuid.hyphenated().to_string(),
        Err(_) => raw.to_string(),
    }
}

fn object_id(object: &Value) -> LinearResult<String> {
    object["id"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| message("Notion response has no id"))
}

fn identity(page: &Value) -> String {
    blocks::plain_text(
        page["properties"][IDENTITY_PROPERTY]["rich_text"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]),
    )
}

fn title(schema: &Schema, page: &Value) -> String {
    blocks::plain_text(
        page["properties"][&schema.title]["title"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]),
    )
}

/// The property that carries Task state: `Status` when present, else the first
/// status property, else a checkbox named Done.
fn state_property(properties: &Value) -> Option<(String, String)> {
    let properties = properties.as_object()?;
    let typed = |name: &str| {
        properties
            .get(name)
            .and_then(|property| property["type"].as_str())
            .map(|kind| (name.to_string(), kind.to_string()))
    };
    typed("Status")
        .filter(|(_, kind)| kind == "status")
        .or_else(|| {
            properties
                .iter()
                .find(|(_, property)| property["type"] == "status")
                .map(|(name, _)| (name.clone(), "status".into()))
        })
        .or_else(|| typed("Done").filter(|(_, kind)| kind == "checkbox"))
}

fn read_schema(properties: &Value) -> LinearResult<Schema> {
    let fields = properties
        .as_object()
        .ok_or_else(|| message("Notion database has no properties"))?;
    let named = |kind: &str, names: &[&str]| {
        fields
            .iter()
            .find(|(name, property)| {
                property["type"] == kind
                    && names.iter().any(|wanted| name.eq_ignore_ascii_case(wanted))
            })
            .map(|(name, _)| name.clone())
    };
    let title = fields
        .iter()
        .find(|(_, property)| property["type"] == "title")
        .map(|(name, _)| name.clone())
        .ok_or_else(|| message("Notion database has no title property"))?;
    let (state_name, kind) = state_property(properties)
        .ok_or_else(|| message("Notion database has no Status property"))?;
    let state = if kind == "checkbox" {
        StateProperty::Checkbox(state_name)
    } else {
        let status = &fields[&state_name]["status"];
        let options = status["options"].as_array().cloned().unwrap_or_default();
        let group = |group: &str| -> Vec<String> {
            let ids: Vec<&str> = status["groups"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|candidate| candidate["name"] == group)
                .flat_map(|candidate| candidate["option_ids"].as_array().into_iter().flatten())
                .filter_map(Value::as_str)
                .collect();
            options
                .iter()
                .filter(|option| option["id"].as_str().is_some_and(|id| ids.contains(&id)))
                .filter_map(|option| option["name"].as_str().map(str::to_owned))
                .collect()
        };
        StateProperty::Status {
            name: state_name,
            todo: group("To-do"),
            started: group("In progress"),
            complete: group("Complete"),
        }
    };
    Ok(Schema {
        title,
        state,
        due: named("date", &["Due", "Due date"]),
        assignee: named("people", &["Assignee", "Assignees"]),
    })
}

fn state_name(schema: &Schema) -> String {
    match &schema.state {
        StateProperty::Status { name, .. } | StateProperty::Checkbox(name) => name.clone(),
    }
}

fn is_canceled(option: &str) -> bool {
    let option = option.to_ascii_lowercase();
    option.starts_with("cancel") || option.contains("won't") || option.contains("wont")
}

fn state_value(schema: &Schema, target: &str) -> LinearResult<Value> {
    match &schema.state {
        StateProperty::Checkbox(_) => Ok(json!({"checkbox": completed(target)?})),
        StateProperty::Status {
            todo,
            started,
            complete,
            ..
        } => {
            let choice = match target {
                "unstarted" => todo.first(),
                "started" => started.first().or(todo.first()),
                "completed" => complete
                    .iter()
                    .find(|option| !is_canceled(option))
                    .or(complete.first()),
                "canceled" => complete
                    .iter()
                    .find(|option| is_canceled(option))
                    .or(complete.first()),
                other => return Err(message(format!("Notion has no Task state `{other}`"))),
            };
            let choice = choice.ok_or_else(|| {
                message(format!(
                    "Notion Status has no option for `{target}`; add one to its groups"
                ))
            })?;
            Ok(json!({"status": {"name": choice}}))
        }
    }
}

fn observed_state(schema: &Schema, page: &Value) -> &'static str {
    let properties = &page["properties"];
    match &schema.state {
        StateProperty::Checkbox(name) if properties[name]["checkbox"] == true => "completed",
        StateProperty::Checkbox(_) => "unstarted",
        StateProperty::Status {
            name,
            started,
            complete,
            ..
        } => {
            let option = properties[name]["status"]["name"]
                .as_str()
                .unwrap_or_default();
            if complete.iter().any(|done| done == option) {
                if is_canceled(option) {
                    "canceled"
                } else {
                    "completed"
                }
            } else if started.iter().any(|active| active == option) {
                "started"
            } else {
                "unstarted"
            }
        }
    }
}

fn completed(target: &str) -> LinearResult<bool> {
    match target {
        "completed" | "canceled" => Ok(true),
        "unstarted" | "started" => Ok(false),
        other => Err(message(format!("Notion has no Task state `{other}`"))),
    }
}

/// Translate the shared planning input names into Notion page properties. The
/// description is the page body, written separately.
fn properties(schema: &Schema, input: &Value) -> LinearResult<Value> {
    let mut properties = serde_json::Map::new();
    let input = input
        .as_object()
        .ok_or_else(|| message("Notion input must be an object"))?;
    for (key, value) in input {
        match key.as_str() {
            "title" => {
                let title = value.as_str().unwrap_or_default();
                properties.insert(schema.title.clone(), json!({"title": blocks::plain(title)}));
            }
            "stateId" => {
                let target = value
                    .as_str()
                    .ok_or_else(|| message("Notion state must be named"))?;
                properties.insert(state_name(schema), state_value(schema, target)?);
            }
            "dueDate" => {
                match (&schema.due, value.as_str()) {
                    (Some(due), Some(date)) => {
                        properties.insert(due.clone(), json!({"date": {"start": date}}));
                    }
                    (Some(due), None) => {
                        properties.insert(due.clone(), json!({"date": null}));
                    }
                    (None, None) => {}
                    (None, Some(_)) => return Err(message(
                        "Notion database has no Due date property to mirror the Task's due date",
                    )),
                }
            }
            "assigneeId" => {
                match (&schema.assignee, value.as_str()) {
                    (Some(assignee), Some(person)) => {
                        properties.insert(assignee.clone(), json!({"people": [{"id": person}]}));
                    }
                    (Some(assignee), None) => {
                        properties.insert(assignee.clone(), json!({"people": []}));
                    }
                    (None, None) => {}
                    (None, Some(_)) => return Err(message(
                        "Notion database has no Assignee property to mirror the Task's assignee",
                    )),
                }
            }
            // Loopflow identity, scope and body are not page properties.
            "id" | "teamId" | "description" => {}
            "projectId" if value.is_null() => {}
            other => {
                return Err(message(format!(
                    "Notion does not mirror the planning field `{other}` yet"
                )))
            }
        }
    }
    Ok(Value::Object(properties))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> Schema {
        read_schema(&json!({
            "Name": {"type": "title", "title": {}},
            "Status": {"type": "status", "status": {
                "options": [
                    {"id": "a", "name": "Not started"}, {"id": "b", "name": "In progress"},
                    {"id": "c", "name": "Done"}, {"id": "d", "name": "Canceled"}
                ],
                "groups": [
                    {"name": "To-do", "option_ids": ["a"]},
                    {"name": "In progress", "option_ids": ["b"]},
                    {"name": "Complete", "option_ids": ["c", "d"]}
                ]
            }},
            "Due": {"type": "date", "date": {}},
        }))
        .unwrap()
    }

    #[test]
    fn status_groups_carry_task_state() {
        let schema = schema();
        assert_eq!(
            state_value(&schema, "completed").unwrap(),
            json!({"status": {"name": "Done"}})
        );
        assert_eq!(
            state_value(&schema, "canceled").unwrap(),
            json!({"status": {"name": "Canceled"}})
        );
        assert_eq!(
            state_value(&schema, "unstarted").unwrap(),
            json!({"status": {"name": "Not started"}})
        );
        let page = |option: &str| json!({"properties": {"Status": {"status": {"name": option}}}});
        assert_eq!(observed_state(&schema, &page("In progress")), "started");
        assert_eq!(observed_state(&schema, &page("Canceled")), "canceled");
    }

    #[test]
    fn properties_translate_shared_planning_names() {
        let fields = properties(
            &schema(),
            &json!({
                "id": "4b0e7d0c-5e8f-4f5c-9a8e-1d2c3b4a5f6e", "teamId": "1", "projectId": null,
                "title": "Ship", "description": "Body", "dueDate": "2026-10-20",
                "assigneeId": null, "stateId": "completed"
            }),
        )
        .unwrap();
        assert_eq!(fields["Name"]["title"][0]["text"]["content"], "Ship");
        assert_eq!(fields["Due"], json!({"date": {"start": "2026-10-20"}}));
        assert_eq!(fields["Status"], json!({"status": {"name": "Done"}}));
        assert!(fields.get("Assignee").is_none());
        assert!(properties(&schema(), &json!({"projectId": "p"})).is_err());
    }

    #[test]
    fn notion_ids_read_from_urls_and_bare_ids() {
        let id = "3fff8f99-3d81-8141-bd3d-f63d878a7c74";
        assert_eq!(notion_id(id), id);
        assert_eq!(notion_id("3fff8f993d818141bd3df63d878a7c74"), id);
        assert_eq!(
            notion_id("https://www.notion.so/team/Demo-3fff8f993d818141bd3df63d878a7c74?v=1"),
            id
        );
    }
}
