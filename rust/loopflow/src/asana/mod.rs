//! Asana delivery and observation. Work owns local content; this adapter owns provider facts.
//!
//! Every task Loopflow exports carries its Loopflow UUID as the OAuth app's
//! `external.gid`, and every request addresses it as `external:<uuid>`. The
//! Task's planning identity stays the one Loopflow assigned, exactly as Linear
//! accepts a caller-supplied issue ID, so a lost creation reply is resolved by
//! lookup rather than a second create. Asana returns the shared planning
//! observations, so acceptance, receipts and steering stay one path.
//! Descriptions and comments travel as Asana rich text (see [`rich_text`]).
//!
//! The repository's planning scope is an Asana team, like a Linear Team. Every
//! Task lives in the team project named after the repository; Loopflow finds
//! that project on first use, or creates it. A Wave Project is its own Asana
//! project in the team, and a Task in it belongs to both projects. Asana
//! projects accept no caller ID, so their notes end with a `Loopflow ID:` line.

mod rich_text;

use std::sync::Arc;

use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::OnceCell;

use crate::linear::{
    local_identifier, LinearComment, LinearError, LinearIssue, LinearIssueObservation,
    LinearResult, OrderedIssue, PlanningResponse,
};

const ASANA_BASE_URL: &str = "https://app.asana.com/api/1.0";
const TASK_FIELDS: &str = "gid,external,name,html_notes,completed,completed_at,modified_at,permalink_url,due_on,assignee.gid,memberships.project.gid";
const STORY_FIELDS: &str =
    "gid,created_at,html_text,resource_subtype,created_by.gid,created_by.name";
const PROJECT_FIELDS: &str = "gid,name,html_notes,modified_at,permalink_url";
/// The last line of a mirrored Project's notes, naming its Loopflow identity.
const IDENTITY_LABEL: &str = "Loopflow ID:";
/// Asana's largest page; a long-lived Task can outgrow one.
const PAGE_LIMIT: &str = "100";

#[derive(Debug, Clone)]
pub struct AsanaClient {
    client: reqwest::Client,
    token: String,
    /// The repository's Asana team: its planning scope, like a Linear Team.
    team: String,
    /// The repository's name, which names its team project.
    repository: String,
    /// The team project's gid, resolved once per client.
    project: Arc<OnceCell<String>>,
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

#[derive(Debug, Deserialize)]
struct Project {
    gid: String,
    name: String,
    html_notes: Option<String>,
    modified_at: Option<String>,
}

impl Project {
    /// The Loopflow Project this Asana project mirrors, from its notes' last line.
    fn identity(&self) -> Option<String> {
        let notes = rich_text::to_markdown(self.html_notes.as_deref()?);
        let line = notes.lines().rev().find(|line| !line.trim().is_empty())?;
        let id = line.trim().strip_prefix(IDENTITY_LABEL)?.trim();
        uuid::Uuid::parse_str(id).ok().map(|id| id.to_string())
    }
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
    pub fn new(token: String, team: &str, repository: String) -> Self {
        Self::with_base_url(token, team, repository, ASANA_BASE_URL.to_string())
    }

    pub(crate) fn with_base_url(
        token: String,
        team: &str,
        repository: String,
        base_url: String,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
            team: team_gid(team),
            repository,
            project: Arc::default(),
            base_url,
        }
    }

    /// The team project named after the repository, created when the team has none.
    async fn project(&self) -> LinearResult<&str> {
        self.project
            .get_or_try_init(|| async {
                let path = format!("/teams/{}/projects?archived=false", self.team);
                let named: Vec<Project> = self
                    .pages(&path)
                    .await?
                    .into_iter()
                    .filter(|project: &Project| project.name == self.repository)
                    .collect();
                match named.as_slice() {
                    [project] => Ok(project.gid.clone()),
                    [] => {
                        let project: Project = self
                            .send(
                                Method::POST,
                                &format!("/teams/{}/projects", self.team),
                                Some(json!({"name": self.repository})),
                            )
                            .await?;
                        Ok(project.gid)
                    }
                    _ => Err(message(format!(
                        "Asana team {} holds several projects named `{}`; rename all but one",
                        self.team, self.repository
                    ))),
                }
            })
            .await
            .map(String::as_str)
    }

    async fn team_projects(&self) -> LinearResult<Vec<Project>> {
        self.pages(&format!("/teams/{}/projects?archived=false", self.team))
            .await
    }

    /// The team project mirroring Loopflow Project `id`.
    async fn wave_project(&self, id: &str) -> LinearResult<Option<Project>> {
        Ok(self
            .team_projects()
            .await?
            .into_iter()
            .find(|project| project.identity().as_deref() == Some(id)))
    }

    /// Create a Wave Project's Asana project in the team from its export input
    /// (`id`, `name`, `description`), answering its revision.
    pub(crate) async fn create_project(&self, input: &Value) -> LinearResult<Option<String>> {
        let id = input["id"]
            .as_str()
            .ok_or_else(|| message("Asana Project export has no Loopflow identity"))?;
        let summary = input["description"].as_str().unwrap_or_default().trim();
        let footer = format!("{IDENTITY_LABEL} {id}");
        let notes = if summary.is_empty() {
            footer
        } else {
            format!("{summary}\n\n{footer}")
        };
        let project: Project = self
            .send(
                Method::POST,
                &format!("/teams/{}/projects", self.team),
                Some(json!({"name": input["name"], "html_notes": rich_text::to_html(&notes)})),
            )
            .await?;
        Ok(project.modified_at)
    }

    /// The revision of the Asana project mirroring Loopflow Project `id`, if any.
    pub(crate) async fn project_revision(&self, id: &str) -> LinearResult<Option<Option<String>>> {
        Ok(self
            .wave_project(id)
            .await?
            .map(|project| project.modified_at))
    }

    /// The Asana project gid for a Task's `projectId`, the Loopflow Project's identity.
    async fn project_gid(&self, project: &Value) -> LinearResult<Option<String>> {
        let Some(id) = project.as_str() else {
            return Ok(None);
        };
        match self.wave_project(id).await? {
            Some(project) => Ok(Some(project.gid)),
            None => Err(message(format!(
                "Asana team {} has no project for Loopflow Project {id}",
                self.team
            ))),
        }
    }

    /// Move the task into the Wave Project `target`, leaving any other mirrored
    /// Wave Project. The repository project and people's own projects stay.
    async fn set_project(&self, task: &Task, target: Option<String>) -> LinearResult<()> {
        let mirrored: Vec<String> = self
            .team_projects()
            .await?
            .into_iter()
            .filter(|project| project.identity().is_some())
            .map(|project| project.gid)
            .collect();
        for member in task.memberships.iter().filter_map(|m| m.project.as_ref()) {
            if mirrored.contains(&member.gid) && target.as_ref() != Some(&member.gid) {
                let _: Value = self
                    .send(
                        Method::POST,
                        &format!("/tasks/{}/removeProject", task.gid),
                        Some(json!({"project": member.gid})),
                    )
                    .await?;
            }
        }
        if let Some(target) = target {
            let _: Value = self
                .send(
                    Method::POST,
                    &format!("/tasks/{}/addProject", task.gid),
                    Some(json!({"project": target})),
                )
                .await?;
        }
        Ok(())
    }

    /// Create the exported task in the repository project, and in its Wave
    /// Project's when it has one. `input` is the saved export receipt: `id`,
    /// `title`, `description`, `assigneeId`, `dueDate`, `stateId`, `projectId`.
    pub(crate) async fn create_task(&self, input: &Value) -> LinearResult<PlanningResponse> {
        let id = input["id"]
            .as_str()
            .ok_or_else(|| message("Asana export has no Loopflow identity"))?;
        let mut data = task_fields(input)?;
        let mut projects = vec![self.project().await?.to_string()];
        projects.extend(self.project_gid(&input["projectId"]).await?);
        data["projects"] = json!(projects);
        data["external"] = json!({"gid": id});
        let task: Task = self.send(Method::POST, "/tasks", Some(data)).await?;
        self.response(id, task).await
    }

    pub(crate) async fn task(&self, id: &str) -> LinearResult<Option<LinearIssue>> {
        match self
            .send::<Task>(Method::GET, &format!("/tasks/external:{id}"), None)
            .await
        {
            Ok(task) => Ok(Some(self.issue(id, task).await?)),
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
        let mut task: Task = self
            .send(Method::PUT, &format!("/tasks/external:{id}"), Some(data))
            .await?;
        if let Some(project) = input.get("projectId") {
            let target = self.project_gid(project).await?;
            self.set_project(&task, target).await?;
            task = self
                .send(Method::GET, &format!("/tasks/external:{id}"), None)
                .await?;
        }
        self.response(id, task).await
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
        self.issue(id, task).await
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
        let mut path = format!("/projects/{}/tasks", self.project().await?);
        if let Some(since) = since {
            path.push_str(&format!("?modified_since={since}"));
        }
        let tasks: Vec<Task> = self.pages(&path).await?;
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
            changes.push((self.issue(&id, task).await?, observation));
        }
        Ok(changes)
    }

    /// Story listing accepts only Asana's own task gid, not `external:`.
    async fn comments(&self, gid: &str) -> LinearResult<Vec<LinearComment>> {
        let stories: Vec<Story> = self.pages(&format!("/tasks/{gid}/stories")).await?;
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

    async fn response(&self, id: &str, task: Task) -> LinearResult<PlanningResponse> {
        let item = self.issue_unchecked(id, task).await?;
        Ok(PlanningResponse::Task {
            issue: Box::new(OrderedIssue {
                item,
                priority_sort_order: 0.0,
                sort_order: 0.0,
            }),
            project: None,
        })
    }

    async fn issue(&self, id: &str, task: Task) -> LinearResult<LinearIssue> {
        if task.external.as_ref().and_then(|e| e.gid.as_deref()) != Some(id) {
            return Err(message(
                "Asana returned a task with a different Loopflow identity",
            ));
        }
        self.issue_unchecked(id, task).await
    }

    /// A task in the repository's team project belongs to the team's plan; one
    /// in a mirrored Wave Project too belongs to that Project.
    async fn issue_unchecked(&self, id: &str, task: Task) -> LinearResult<LinearIssue> {
        let repository = self.project().await?;
        let members: Vec<&str> = task
            .memberships
            .iter()
            .filter_map(|m| m.project.as_ref().map(|p| p.gid.as_str()))
            .collect();
        let in_scope = members.contains(&repository);
        let project_id = if members.iter().any(|gid| *gid != repository) {
            self.team_projects()
                .await?
                .into_iter()
                .filter(|project| members.contains(&project.gid.as_str()))
                .find_map(|project| project.identity())
        } else {
            None
        };
        Ok(LinearIssue {
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
            project_id,
            team_id: in_scope.then(|| self.team.clone()),
            assignee: task.assignee.map(|assignee| assignee.gid),
        })
    }

    /// Every page of a list read, at Asana's largest page size.
    async fn pages<T: serde::de::DeserializeOwned>(&self, path: &str) -> LinearResult<Vec<T>> {
        let separator = if path.contains('?') { '&' } else { '?' };
        let mut items = Vec::new();
        let mut offset = None;
        loop {
            let mut page_path = format!("{path}{separator}limit={PAGE_LIMIT}");
            if let Some(offset) = &offset {
                page_path.push_str(&format!("&offset={offset}"));
            }
            let page: Envelope<Vec<T>> = self.request(Method::GET, &page_path, None).await?;
            items.extend(page.data);
            match page.next_page {
                Some(next) => offset = Some(next.offset),
                None => break,
            }
        }
        Ok(items)
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
        } else if route.ends_with("/projects") {
            PROJECT_FIELDS
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

/// An Asana team gid, bare or read from a team URL
/// (`https://app.asana.com/0/team/<gid>/…` or `…/team/<gid>`).
pub(crate) fn team_gid(raw: &str) -> String {
    let segments: Vec<&str> = raw.trim().split(['/', '?', '#']).collect();
    let numeric =
        |segment: &str| !segment.is_empty() && segment.bytes().all(|b| b.is_ascii_digit());
    segments
        .iter()
        .position(|segment| *segment == "team")
        .and_then(|team| segments.get(team + 1).copied().filter(|gid| numeric(gid)))
        .or_else(|| segments.iter().rev().copied().find(|gid| numeric(gid)))
        .map_or_else(|| raw.trim().to_string(), str::to_string)
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
            // Loopflow identity, scope and Project membership are not task fields.
            "id" | "teamId" | "projectId" => {}
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
        assert!(task_fields(&json!({"parentId": "task"})).is_err());
    }

    #[test]
    fn team_gids_read_from_urls_and_bare_ids() {
        assert_eq!(team_gid("1209876543210"), "1209876543210");
        assert_eq!(
            team_gid("https://app.asana.com/0/team/1209876543210/overview"),
            "1209876543210"
        );
        assert_eq!(
            team_gid("https://app.asana.com/1/1100000000000001/team/1209876543210/all"),
            "1209876543210"
        );
    }
}
