//! Asana delivery and observation. Work owns local content; this adapter owns provider facts.
//!
//! Every task Loopflow exports carries its Loopflow UUID as the OAuth app's
//! `external.gid`, and every request addresses it as `external:<uuid>`. The
//! Task's planning identity stays the one Loopflow assigned, exactly as Linear
//! accepts a caller-supplied issue ID, so a lost creation reply is resolved by
//! lookup rather than a second create. Asana returns the shared planning
//! observations, so acceptance, receipts and steering stay one path.

use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::linear::{
    LinearComment, LinearError, LinearIssue, LinearIssueObservation, LinearResult, OrderedIssue,
    PlanningResponse,
};

const ASANA_BASE_URL: &str = "https://app.asana.com/api/1.0";
const TASK_FIELDS: &str = "gid,external,name,notes,completed,completed_at,modified_at,permalink_url,due_on,assignee.gid,memberships.project.gid";
const STORY_FIELDS: &str = "gid,created_at,text,resource_subtype,created_by.gid,created_by.name";

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
}

#[derive(Debug, Deserialize)]
struct Task {
    gid: String,
    external: Option<External>,
    name: String,
    #[serde(default)]
    notes: String,
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
struct External {
    gid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Reference {
    gid: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Membership {
    project: Option<Reference>,
}

#[derive(Debug, Deserialize)]
struct Story {
    gid: String,
    created_at: Option<String>,
    #[serde(default)]
    text: String,
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

    /// Post `body` once. Asana stories carry no caller identity, so a Loopflow
    /// comment is its visible text on the task: an existing match is the delivery.
    pub(crate) async fn sync_comment(&self, issue: &str, body: &str) -> LinearResult<()> {
        let text = comment_text(body);
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
                Some(json!({"text": text})),
            )
            .await?;
        Ok(())
    }

    pub(crate) async fn observe(&self, id: &str) -> LinearResult<LinearIssueObservation> {
        let task: Task = self
            .send(Method::GET, &format!("/tasks/external:{id}"), None)
            .await?;
        let mut comments = self.comments(&task.gid).await?;
        comments.sort_by(|left, right| {
            left.revision
                .cmp(&right.revision)
                .then(left.id.cmp(&right.id))
        });
        Ok(LinearIssueObservation {
            revision: task.modified_at,
            title: task.name,
            description: task.notes,
            comments,
        })
    }

    /// Story listing accepts only Asana's own task gid, not `external:`.
    async fn comments(&self, gid: &str) -> LinearResult<Vec<LinearComment>> {
        let stories: Vec<Story> = self
            .send(Method::GET, &format!("/tasks/{gid}/stories"), None)
            .await?;
        Ok(stories
            .into_iter()
            .filter(|story| story.resource_subtype.as_deref() == Some("comment_added"))
            .map(|story| LinearComment {
                id: story.gid,
                revision: story.created_at.clone(),
                created_at: story.created_at,
                body: story.text,
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
            identifier: identifier(id),
            branch_name: None,
            url: task.permalink_url,
            name: task.name,
            description: task.notes,
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
        let fields = if path.ends_with("/stories") {
            STORY_FIELDS
        } else {
            TASK_FIELDS
        };
        let mut request = self
            .client
            .request(
                method,
                format!("{}{path}?opt_fields={fields}", self.base_url),
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
        let envelope: Envelope<T> = serde_json::from_slice(&body)
            .map_err(|error| message(format!("invalid Asana response: {error}")))?;
        Ok(envelope.data)
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

fn message(text: impl Into<String>) -> LinearError {
    LinearError::Message(text.into())
}

/// Asana renders plain text, so Loopflow's hidden `<!-- loopflow-… -->` markers
/// would show. Asana receives the visible text; read-back claims it by that text.
pub(crate) fn comment_text(body: &str) -> String {
    let mut text = String::new();
    let mut rest = body;
    while let Some(start) = rest.find("<!-- loopflow-") {
        text.push_str(&rest[..start]);
        rest = match rest[start..].find("-->") {
            Some(end) => &rest[start + end + 3..],
            None => "",
        };
    }
    text.push_str(rest);
    text.trim().to_string()
}

/// Asana has no short identifiers; the Task keeps its local `lf-` identifier.
fn identifier(id: &str) -> String {
    uuid::Uuid::parse_str(id)
        .map(|uuid| format!("lf-{}", uuid.simple()))
        .unwrap_or_else(|_| id.to_string())
}

fn completed(target: &str) -> LinearResult<bool> {
    match target {
        "completed" | "canceled" => Ok(true),
        "unstarted" => Ok(false),
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
                data.insert("notes".into(), value.clone());
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
            json!({"name": "Ship", "notes": "Notes", "due_on": null, "completed": true})
        );
        assert!(task_fields(&json!({"projectId": "section"})).is_err());
    }

    #[test]
    fn comment_text_hides_loopflow_markers() {
        assert_eq!(
            comment_text(
                "Ship it\n\n<!-- loopflow-requester:\"Jack\" -->\n\n<!-- loopflow-steer:1 -->"
            ),
            "Ship it"
        );
        assert_eq!(comment_text("<!-- other --> kept"), "<!-- other --> kept");
    }

    #[test]
    fn identifier_matches_the_local_task_identifier() {
        let task = crate::durable::TaskId::new();
        let uuid = uuid::Uuid::parse_str(task.as_str().trim_start_matches("task_")).unwrap();
        assert_eq!(
            identifier(&uuid.to_string()),
            format!("lf-{}", task.as_str().trim_start_matches("task_"))
        );
    }
}
