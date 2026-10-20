//! Asana delivery and observation. Work owns local content; this adapter owns provider facts.
//!
//! Every task Loopflow exports carries its Loopflow UUID as the OAuth app's
//! `external.gid`, and every request addresses it as `external:<uuid>`. The
//! Task's planning identity stays the one Loopflow assigned, exactly as Linear
//! accepts a caller-supplied issue ID, so a lost creation reply is resolved by
//! lookup rather than a second create. Asana returns the shared planning
//! observations, so acceptance, receipts and steering stay one path.
//! Descriptions and comments travel as Asana rich text (see [`rich_text`]).

mod rich_text;

use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::linear::{
    local_identifier, LinearComment, LinearError, LinearIssue, LinearIssueObservation,
    LinearResult, OrderedIssue, PlanningResponse,
};

const ASANA_BASE_URL: &str = "https://app.asana.com/api/1.0";
const TASK_FIELDS: &str = "gid,external,name,html_notes,completed,completed_at,modified_at,permalink_url,due_on,assignee.gid,memberships.project.gid";
const STORY_FIELDS: &str =
    "gid,created_at,html_text,resource_subtype,created_by.gid,created_by.name";
/// Asana's largest page; a long-lived Task can outgrow one.
const PAGE_LIMIT: &str = "100";

#[derive(Debug, Clone)]
pub struct AsanaClient {
    client: reqwest::Client,
    token: String,
    /// The repository's Asana project: its planning scope, like a Linear Team.
    project: String,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    data: T,
    next_page: Option<NextPage>,
}

#[derive(Debug, Deserialize)]
struct NextPage {
    offset: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Task {
    gid: String,
    external: Option<External>,
    name: String,
    #[serde(default)]
    html_notes: String,
    completed: bool,
    completed_at: Option<String>,
    modified_at: String,
    permalink_url: Option<String>,
    due_on: Option<String>,
    assignee: Option<Reference>,
    #[serde(default)]
    memberships: Vec<Membership>,
}

#[derive(Debug, Clone, Deserialize)]
struct External {
    gid: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Reference {
    gid: String,
    name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Membership {
    project: Option<Reference>,
}

#[derive(Debug, Deserialize)]
struct Story {
    gid: String,
    created_at: Option<String>,
    #[serde(default)]
    html_text: String,
    resource_subtype: Option<String>,
    created_by: Option<Reference>,
}

impl AsanaClient {
    pub fn new(token: String, project: String) -> Self {
        Self::with_base_url(token, project, ASANA_BASE_URL.to_string())
    }

    pub(crate) fn with_base_url(token: String, project: String, base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
            project,
            base_url,
        }
    }

    /// Create the exported task in the repository project. `input` is the saved
    /// export receipt: `id`, `title`, `description`, `assigneeId`, `dueDate`, `stateId`.
    pub(crate) async fn create_task(&self, input: &Value) -> LinearResult<PlanningResponse> {
        let id = input["id"]
            .as_str()
            .ok_or_else(|| message("Asana export has no Loopflow identity"))?;
        let mut data = task_fields(input)?;
        data["projects"] = json!([self.project]);
        data["external"] = json!({"gid": id});
        let task: Task = self.send(Method::POST, "/tasks", Some(data)).await?;
        Ok(self.response(id, task))
    }

    pub(crate) async fn task(&self, id: &str) -> LinearResult<Option<LinearIssue>> {
        match self
            .send::<Task>(Method::GET, &format!("/tasks/external:{id}"), None)
            .await
        {
            Ok(task) => Ok(Some(self.issue(id, task)?)),
            Err(AsanaFailure::NotFound) => Ok(None),
            Err(AsanaFailure::Provider(error)) => Err(error),
        }
    }

    /// Field edits share the export input's names.
    pub(crate) async fn update_task(
        &self,
        id: &str,
        input: &Value,
    ) -> LinearResult<PlanningResponse> {
        let data = task_fields(input)?;
        let task: Task = self
            .send(Method::PUT, &format!("/tasks/external:{id}"), Some(data))
            .await?;
        Ok(self.response(id, task))
    }

    /// Asana has open and completed tasks; a canceled Task is closed there too.
    pub(crate) async fn set_state(&self, id: &str, target: &str) -> LinearResult<LinearIssue> {
        let task: Task = self
            .send(
                Method::PUT,
                &format!("/tasks/external:{id}"),
                Some(json!({"completed": completed(target)?})),
            )
            .await?;
        self.issue(id, task)
    }

    pub(crate) async fn delete_task(&self, id: &str) -> LinearResult<()> {
        let _: Value = self
            .send(Method::DELETE, &format!("/tasks/external:{id}"), None)
            .await?;
        Ok(())
    }

    /// Post `text` once. Asana stories carry no caller identity, so a Loopflow
    /// comment is its visible text on the task: an existing match is the delivery.
    pub(crate) async fn sync_comment(&self, issue: &str, text: &str) -> LinearResult<()> {
        let task: Task = self
            .send(Method::GET, &format!("/tasks/external:{issue}"), None)
            .await?;
        if self
            .comments(&task.gid)
            .await?
            .iter()
            .any(|comment| comment.body == text)
        {
            return Ok(());
        }
        let _: Story = self
            .send(
                Method::POST,
                &format!("/tasks/external:{issue}/stories"),
                Some(json!({"html_text": rich_text::to_html(text)})),
            )
            .await?;
        Ok(())
    }

    pub(crate) async fn observe(&self, id: &str) -> LinearResult<LinearIssueObservation> {
        let task: Task = self
            .send(Method::GET, &format!("/tasks/external:{id}"), None)
            .await?;
        self.observation(task).await
    }

    async fn observation(&self, task: Task) -> LinearResult<LinearIssueObservation> {
        let mut comments = self.comments(&task.gid).await?;
        comments.sort_by(|left, right| {
            left.revision
                .cmp(&right.revision)
                .then(left.id.cmp(&right.id))
        });
        Ok(LinearIssueObservation {
            revision: task.modified_at,
            title: task.name,
            description: rich_text::to_markdown(&task.html_notes),
            comments,
        })
    }

    /// Tasks in the repository project modified in `since..`, with their
    /// observations. A task someone created in Asana has no Loopflow identity
    /// yet; it takes one derived from its gid, so every machine claims the same
    /// one. Completed tasks Loopflow never tracked, and `finished` ones, are skipped.
    pub(crate) async fn changes(
        &self,
        since: Option<&str>,
        live: &[String],
        finished: &[String],
    ) -> LinearResult<Vec<(LinearIssue, LinearIssueObservation)>> {
        let mut tasks: Vec<Task> = Vec::new();
        let mut offset = None;
        loop {
            let mut path = format!("/projects/{}/tasks?limit={PAGE_LIMIT}", self.project);
            if let Some(since) = since {
                path.push_str(&format!("&modified_since={since}"));
            }
            if let Some(offset) = &offset {
                path.push_str(&format!("&offset={offset}"));
            }
            let page: Envelope<Vec<Task>> = self.request(Method::GET, &path, None).await?;
            tasks.extend(page.data);
            match page.next_page {
                Some(next) => offset = Some(next.offset),
                None => break,
            }
        }
        let mut changes = Vec::new();
        for mut task in tasks {
            let known = task.external.as_ref().and_then(|e| e.gid.clone());
            let id = known
                .clone()
                .unwrap_or_else(|| acquired_identity(&task.gid));
            if finished.contains(&id) || (task.completed && !live.contains(&id)) {
                continue;
            }
            if known.is_none() {
                task = self
                    .send(
                        Method::PUT,
                        &format!("/tasks/{}", task.gid),
                        Some(json!({"external": {"gid": id}})),
                    )
                    .await?;
            }
            let observation = self.observation(task.clone()).await?;
            changes.push((self.issue(&id, task)?, observation));
        }
        Ok(changes)
    }

    /// Story listing accepts only Asana's own task gid, not `external:`.
    async fn comments(&self, gid: &str) -> LinearResult<Vec<LinearComment>> {
        let mut stories = Vec::new();
        let mut offset = None;
        loop {
            let mut path = format!("/tasks/{gid}/stories?limit={PAGE_LIMIT}");
            if let Some(offset) = &offset {
                path.push_str(&format!("&offset={offset}"));
            }
            let page: Envelope<Vec<Story>> = self.request(Method::GET, &path, None).await?;
            stories.extend(page.data);
            match page.next_page {
                Some(next) => offset = Some(next.offset),
                None => break,
            }
        }
        Ok(stories
            .into_iter()
            .filter(|story| story.resource_subtype.as_deref() == Some("comment_added"))
            .map(|story| LinearComment {
                id: story.gid,
                revision: story.created_at.clone(),
                created_at: story.created_at,
                body: rich_text::to_markdown(&story.html_text),
                author_id: story.created_by.as_ref().map(|author| author.gid.clone()),
                author_name: story.created_by.and_then(|author| author.name),
            })
            .collect())
    }

    fn response(&self, id: &str, task: Task) -> PlanningResponse {
        let item = self.issue_unchecked(id, task);
        PlanningResponse::Task {
            issue: Box::new(OrderedIssue {
                item,
                priority_sort_order: 0.0,
                sort_order: 0.0,
            }),
            project: None,
        }
    }

    fn issue(&self, id: &str, task: Task) -> LinearResult<LinearIssue> {
        if task.external.as_ref().and_then(|e| e.gid.as_deref()) != Some(id) {
            return Err(message(
                "Asana returned a task with a different Loopflow identity",
            ));
        }
        Ok(self.issue_unchecked(id, task))
    }

    fn issue_unchecked(&self, id: &str, task: Task) -> LinearIssue {
        let in_scope = task
            .memberships
            .iter()
            .any(|m| m.project.as_ref().is_some_and(|p| p.gid == self.project));
        LinearIssue {
            revision: Some(task.modified_at),
            id: id.to_string(),
            identifier: local_identifier(id),
            branch_name: None,
            url: task.permalink_url,
            name: task.name,
            description: rich_text::to_markdown(&task.html_notes),
            rank: 0,
            completed: task.completed,
            completed_at: task.completed_at,
            due_date: task.due_on,
            state: Some(
                if task.completed {
                    "completed"
                } else {
                    "unstarted"
                }
                .into(),
            ),
            project_id: None,
            team_id: in_scope.then(|| self.project.clone()),
            assignee: task.assignee.map(|assignee| assignee.gid),
        }
    }

    async fn send<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        data: Option<Value>,
    ) -> Result<T, AsanaFailure> {
        Ok(self.request(method, path, data).await?.data)
    }

    async fn request<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        data: Option<Value>,
    ) -> Result<Envelope<T>, AsanaFailure> {
        let (route, query) = path.split_once('?').unwrap_or((path, ""));
        let fields = if route.ends_with("/stories") {
            STORY_FIELDS
        } else {
            TASK_FIELDS
        };
        let query = if query.is_empty() {
            String::new()
        } else {
            format!("{query}&")
        };
        let mut request = self
            .client
            .request(
                method,
                format!("{}{route}?{query}opt_fields={fields}", self.base_url),
            )
            .bearer_auth(&self.token);
        if let Some(data) = data {
            request = request.json(&json!({"data": data}));
        }
        let response = request
            .send()
            .await
            .map_err(|error| message(format!("Asana request failed: {error}")))?;
        let status = response.status();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<i64>().ok());
        let body = response
            .bytes()
            .await
            .map_err(|error| message(format!("failed to read Asana response: {error}")))?;
        if status == StatusCode::NOT_FOUND {
            return Err(AsanaFailure::NotFound);
        }
        if status == StatusCode::TOO_MANY_REQUESTS {
            let now = time::OffsetDateTime::now_utc().unix_timestamp() * 1000;
            return Err(LinearError::RateLimited {
                reset_at_ms: now + retry_after.unwrap_or(60) * 1000,
            }
            .into());
        }
        if !status.is_success() {
            let detail = serde_json::from_slice::<Value>(&body)
                .ok()
                .and_then(|value| value["errors"][0]["message"].as_str().map(str::to_owned))
                .unwrap_or_else(|| format!("HTTP {status}"));
            return Err(message(format!("Asana rejected the request: {detail}")).into());
        }
        serde_json::from_slice(&body)
            .map_err(|error| message(format!("invalid Asana response: {error}")).into())
    }
}

#[derive(Debug)]
enum AsanaFailure {
    NotFound,
    Provider(LinearError),
}

impl From<LinearError> for AsanaFailure {
    fn from(error: LinearError) -> Self {
        Self::Provider(error)
    }
}

impl From<AsanaFailure> for LinearError {
    fn from(failure: AsanaFailure) -> Self {
        match failure {
            AsanaFailure::NotFound => message("Asana task is unavailable"),
            AsanaFailure::Provider(error) => error,
        }
    }
}

/// The Loopflow identity of a task first created in Asana.
fn acquired_identity(gid: &str) -> String {
    uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_URL,
        format!("https://app.asana.com/task/{gid}").as_bytes(),
    )
    .to_string()
}

fn message(text: impl Into<String>) -> LinearError {
    LinearError::Message(text.into())
}

/// A Loopflow comment's visible text as Asana returns it: the markdown its rich
/// text reads back as.
pub(crate) fn comment_markdown(text: &str) -> String {
    rich_text::to_markdown(&rich_text::to_html(text))
}

fn completed(target: &str) -> LinearResult<bool> {
    match target {
        "completed" | "canceled" => Ok(true),
        // Asana tasks are open or complete; work in progress is open.
        "unstarted" | "started" => Ok(false),
        other => Err(message(format!("Asana has no Task state `{other}`"))),
    }
}

/// Translate the shared planning input names into Asana task fields.
fn task_fields(input: &Value) -> LinearResult<Value> {
    let mut data = serde_json::Map::new();
    let input = input
        .as_object()
        .ok_or_else(|| message("Asana input must be an object"))?;
    for (key, value) in input {
        match key.as_str() {
            "title" => {
                data.insert("name".into(), value.clone());
            }
            "description" => {
                let markdown = value.as_str().unwrap_or_default();
                data.insert("html_notes".into(), json!(rich_text::to_html(markdown)));
            }
            "assigneeId" => {
                data.insert("assignee".into(), value.clone());
            }
            "dueDate" => {
                data.insert("due_on".into(), value.clone());
            }
            "stateId" => {
                let target = value
                    .as_str()
                    .ok_or_else(|| message("Asana state must be named"))?;
                data.insert("completed".into(), json!(completed(target)?));
            }
            // Loopflow identity and scope are request fields, not task content.
            "id" | "teamId" => {}
            "projectId" if value.is_null() => {}
            other => {
                return Err(message(format!(
                    "Asana does not mirror the planning field `{other}` yet"
                )))
            }
        }
    }
    Ok(Value::Object(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_fields_translate_shared_planning_names() {
        let fields = task_fields(&json!({
            "id": "4b0e7d0c-5e8f-4f5c-9a8e-1d2c3b4a5f6e", "teamId": "1", "projectId": null,
            "title": "Ship", "description": "Notes", "dueDate": null, "stateId": "canceled"
        }))
        .unwrap();
        assert_eq!(
            fields,
            json!({"name": "Ship", "html_notes": "<body>Notes</body>", "due_on": null, "completed": true})
        );
        assert!(task_fields(&json!({"projectId": "section"})).is_err());
    }
}
