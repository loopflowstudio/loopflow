use async_trait::async_trait;
use reqwest::{Method, StatusCode, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio::time::sleep;
use tracing::warn;

use crate::engine::config::AsanaConfig;
use crate::lfd::pm::asana_html::{asana_html_to_markdown, markdown_to_asana_html};
use crate::lfd::pm::{
    PmError, PmItem, PmItemCreate, PmItemUpdate, PmProject, PmProvider, PmResult, PriorityBucket,
    RATE_LIMIT_RETRIES,
};

const ASANA_BASE_URL: &str = "https://app.asana.com/api/1.0";
const TASK_FIELDS: &str = "name,notes,html_notes,completed,assignee.gid,custom_fields.gid,custom_fields.name,custom_fields.resource_subtype,custom_fields.text_value,custom_fields.enum_value.gid,custom_fields.enum_value.name,custom_fields.enum_options.gid,custom_fields.enum_options.name";
const PROJECT_PRIORITY_FIELD_FIELDS: &str =
    "custom_field.gid,custom_field.name,custom_field.resource_subtype,custom_field.enum_options.gid,custom_field.enum_options.name";
const PROJECT_TEXT_FIELD_FIELDS: &str =
    "custom_field.gid,custom_field.name,custom_field.resource_subtype";
const WORKSPACE_TEXT_FIELD_FIELDS: &str = "gid,name,resource_subtype";
const CLAIM_TASK_FIELDS: &str = "assignee.gid,custom_fields.gid,custom_fields.name,custom_fields.resource_subtype,custom_fields.text_value";
const DEFAULT_LOOPFLOW_TEAM_NAME: &str = "Waves";
const PRIORITY_FIELD_NAME: &str = "Priority";
const WORKING_BRANCH_FIELD_NAME: &str = "Working branch";

#[derive(Debug, Clone)]
pub struct AsanaClient {
    client: reqwest::Client,
    token: String,
    config: AsanaConfig,
    base_url: String,
}

impl AsanaClient {
    pub fn new(token: String, config: AsanaConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
            config,
            base_url: ASANA_BASE_URL.to_string(),
        }
    }

    #[cfg(test)]
    fn with_base_url(token: String, config: AsanaConfig, base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
            config,
            base_url,
        }
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
    ) -> reqwest::RequestBuilder {
        let mut url = Url::parse(&format!("{}{}", self.base_url, path))
            .expect("asana base URL should be valid");
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in query {
                pairs.append_pair(key, value);
            }
        }
        self.client.request(method, url).bearer_auth(&self.token)
    }

    async fn send_json<T, F>(&self, make_request: F) -> PmResult<T>
    where
        T: DeserializeOwned,
        F: Fn() -> reqwest::RequestBuilder,
    {
        for attempt in 0..=RATE_LIMIT_RETRIES {
            let response = make_request()
                .send()
                .await
                .map_err(|err| PmError::Message(format!("asana request failed: {err}")))?;

            if response.status() == StatusCode::TOO_MANY_REQUESTS && attempt < RATE_LIMIT_RETRIES {
                let delay = super::retry_after_delay(response.headers());
                warn!(
                    attempt = attempt + 1,
                    delay_seconds = delay.as_secs(),
                    "asana rate limited; retrying"
                );
                sleep(delay).await;
                continue;
            }

            return parse_response(response).await;
        }

        Err(PmError::Message(
            "asana request failed after retries".to_string(),
        ))
    }

    /// List all workspaces visible to the authenticated user.
    pub async fn list_workspaces(&self) -> PmResult<Vec<AsanaWorkspace>> {
        let response: AsanaResponse<Vec<AsanaWorkspace>> = self
            .send_json(|| self.request(Method::GET, "/workspaces", &[]))
            .await?;
        Ok(response.data)
    }

    /// Resolve the workspace GID: use config if set, otherwise auto-detect.
    /// Fails if the user has zero or multiple workspaces and none is configured.
    pub async fn resolve_workspace(&self) -> PmResult<String> {
        if let Some(workspace) = &self.config.workspace {
            return Ok(workspace.clone());
        }

        let workspaces = self.list_workspaces().await?;
        match workspaces.len() {
            0 => Err(PmError::Message(
                "no asana workspaces found for this token".to_string(),
            )),
            1 => Ok(workspaces.into_iter().next().expect("len is 1").gid),
            n => {
                let list: Vec<String> = workspaces
                    .iter()
                    .map(|ws| format!("  {} ({})", ws.name, ws.gid))
                    .collect();
                Err(PmError::Message(format!(
                    "found {n} asana workspaces — set asana.workspace in .lf/config.yaml:\n{}",
                    list.join("\n")
                )))
            }
        }
    }

    async fn list_teams(&self, workspace_id: &str) -> PmResult<Vec<AsanaTeam>> {
        let path = format!("/workspaces/{workspace_id}/teams");
        let response: AsanaResponse<Vec<AsanaTeam>> = self
            .send_json(|| self.request(Method::GET, &path, &[]))
            .await?;
        Ok(response.data)
    }

    async fn create_team_in_workspace(&self, workspace_id: &str, name: &str) -> PmResult<String> {
        let body = AsanaRequest {
            data: CreateTeamRequest {
                name,
                organization: workspace_id,
            },
        };
        let response: AsanaResponse<AsanaGid> = self
            .send_json(|| self.request(Method::POST, "/teams", &[]).json(&body))
            .await?;
        Ok(response.data.gid)
    }

    async fn resolve_team_for_project_bootstrap(&self, workspace_id: &str) -> PmResult<String> {
        if let Some(team) = self.config.default_team.as_deref() {
            return Ok(team.to_string());
        }

        let teams = self.list_teams(workspace_id).await?;
        if let Some(existing) = teams
            .iter()
            .find(|team| team.name.eq_ignore_ascii_case(DEFAULT_LOOPFLOW_TEAM_NAME))
        {
            return Ok(existing.gid.clone());
        }

        self.create_team_in_workspace(workspace_id, DEFAULT_LOOPFLOW_TEAM_NAME)
            .await
    }

    async fn create_project_for_team(
        &self,
        team_id: &str,
        name: &str,
        description: &str,
    ) -> PmResult<String> {
        let body = AsanaRequest {
            data: CreateProjectForTeamRequest {
                name,
                notes: description,
            },
        };
        let path = format!("/teams/{team_id}/projects");
        let response: AsanaResponse<AsanaGid> = self
            .send_json(|| self.request(Method::POST, &path, &[]).json(&body))
            .await?;
        Ok(response.data.gid)
    }

    async fn priority_field_for_project(
        &self,
        project_id: &str,
    ) -> PmResult<Option<AsanaPriorityField>> {
        let path = format!("/projects/{project_id}/custom_field_settings");
        let response: AsanaResponse<Vec<AsanaCustomFieldSetting>> = self
            .send_json(|| {
                self.request(
                    Method::GET,
                    &path,
                    &[("opt_fields", PROJECT_PRIORITY_FIELD_FIELDS)],
                )
            })
            .await?;

        Ok(response
            .data
            .into_iter()
            .find_map(|setting| AsanaPriorityField::from_metadata(setting.custom_field)))
    }

    async fn ensure_priority_field_for_project(
        &self,
        project_id: &str,
    ) -> PmResult<AsanaPriorityField> {
        if let Some(field) = self.priority_field_for_project(project_id).await? {
            return Ok(field);
        }

        let body = json!({
            "data": {
                "custom_field": {
                    "name": PRIORITY_FIELD_NAME,
                    "resource_subtype": "enum",
                    "enum_options": [
                        { "name": PriorityBucket::Urgent.semantic_label() },
                        { "name": PriorityBucket::High.semantic_label() },
                        { "name": PriorityBucket::Medium.semantic_label() },
                        { "name": PriorityBucket::Low.semantic_label() }
                    ]
                }
            }
        });
        let path = format!("/projects/{project_id}/addCustomFieldSetting");
        let _: AsanaResponse<Value> = self
            .send_json(|| self.request(Method::POST, &path, &[]).json(&body))
            .await?;

        self.priority_field_for_project(project_id)
            .await?
            .ok_or_else(|| {
                PmError::Message(format!(
                    "asana project {project_id} is missing a priority custom field after creation"
                ))
            })
    }

    async fn working_branch_field_for_project(&self, project_id: &str) -> PmResult<Option<String>> {
        let path = format!("/projects/{project_id}/custom_field_settings");
        let response: AsanaResponse<Vec<AsanaCustomFieldSetting>> = self
            .send_json(|| {
                self.request(
                    Method::GET,
                    &path,
                    &[("opt_fields", PROJECT_TEXT_FIELD_FIELDS)],
                )
            })
            .await?;

        Ok(response
            .data
            .into_iter()
            .find_map(|setting| working_branch_gid_from_metadata(setting.custom_field)))
    }

    async fn workspace_working_branch_field(&self, workspace_id: &str) -> PmResult<Option<String>> {
        let path = format!("/workspaces/{workspace_id}/custom_fields");
        let response: AsanaResponse<Vec<AsanaCustomFieldMetadata>> = self
            .send_json(|| {
                self.request(
                    Method::GET,
                    &path,
                    &[("opt_fields", WORKSPACE_TEXT_FIELD_FIELDS)],
                )
            })
            .await?;

        Ok(response
            .data
            .into_iter()
            .find_map(working_branch_gid_from_metadata))
    }

    async fn create_workspace_working_branch_field(&self, workspace_id: &str) -> PmResult<String> {
        let body = json!({
            "data": {
                "name": WORKING_BRANCH_FIELD_NAME,
                "resource_subtype": "text",
                "workspace": workspace_id
            }
        });
        let response: AsanaResponse<AsanaGid> = self
            .send_json(|| {
                self.request(Method::POST, "/custom_fields", &[])
                    .json(&body)
            })
            .await?;
        Ok(response.data.gid)
    }

    async fn attach_working_branch_field_to_project(
        &self,
        project_id: &str,
        field_id: &str,
    ) -> PmResult<()> {
        let body = json!({ "data": { "custom_field": field_id } });
        let path = format!("/projects/{project_id}/addCustomFieldSetting");
        let _: AsanaResponse<Value> = self
            .send_json(|| self.request(Method::POST, &path, &[]).json(&body))
            .await?;
        Ok(())
    }

    async fn ensure_working_branch_field_for_project(&self, project_id: &str) -> PmResult<String> {
        if let Some(field_id) = self.working_branch_field_for_project(project_id).await? {
            return Ok(field_id);
        }

        let workspace_id = self.resolve_workspace().await?;
        let field_id = match self.workspace_working_branch_field(&workspace_id).await? {
            Some(field_id) => field_id,
            None => {
                self.create_workspace_working_branch_field(&workspace_id)
                    .await?
            }
        };
        self.attach_working_branch_field_to_project(project_id, &field_id)
            .await?;

        self.working_branch_field_for_project(project_id)
            .await?
            .ok_or_else(|| {
                PmError::Message(format!(
                    "asana project {project_id} is missing a Working branch custom field after setup"
                ))
            })
    }
}

#[async_trait]
impl PmProvider for AsanaClient {
    async fn create_project(&self, name: &str, description: &str) -> PmResult<String> {
        let workspace = self.resolve_workspace().await?;
        let team = self.resolve_team_for_project_bootstrap(&workspace).await?;
        self.create_project_for_team(&team, name, description).await
    }

    async fn list_projects(&self, team_id: &str) -> PmResult<Vec<PmProject>> {
        let path = format!("/teams/{team_id}/projects");
        let response: AsanaResponse<Vec<AsanaProjectNode>> = self
            .send_json(|| self.request(Method::GET, &path, &[("opt_fields", "name")]))
            .await?;
        Ok(response
            .data
            .into_iter()
            .map(|p| PmProject {
                id: p.gid,
                name: p.name,
            })
            .collect())
    }

    async fn init_project(&self, project_id: &str) -> PmResult<()> {
        self.ensure_working_branch_field_for_project(project_id)
            .await
            .map(|_| ())
    }

    async fn list_items(&self, project_id: &str) -> PmResult<Vec<PmItem>> {
        let path = format!("/projects/{project_id}/tasks");
        let mut offset = None;
        let mut items = Vec::new();
        let mut response_index = 0usize;

        loop {
            let page_offset = offset.clone();
            let response: AsanaListResponse<AsanaTask> = self
                .send_json(|| {
                    let mut query = vec![("opt_fields", TASK_FIELDS)];
                    if let Some(offset) = page_offset.as_deref() {
                        query.push(("offset", offset));
                    }
                    self.request(Method::GET, &path, &query)
                })
                .await?;

            for task in response.data {
                items.push((response_index, task.into_pm_item()));
                response_index += 1;
            }

            offset = response.next_page.and_then(|page| page.offset);
            if offset.is_none() {
                items.sort_by(|left, right| {
                    left.1
                        .rank
                        .cmp(&right.1.rank)
                        .then_with(|| left.0.cmp(&right.0))
                });
                return Ok(items.into_iter().map(|(_, item)| item).collect());
            }
        }
    }

    async fn create_item(&self, project_id: &str, item: &PmItemCreate) -> PmResult<String> {
        let priority_field = self.ensure_priority_field_for_project(project_id).await?;
        let priority_field_id = priority_field.gid.clone();
        let priority_option_id = priority_field
            .option_gid(PriorityBucket::from_rank(item.rank))
            .to_string();
        let html_notes = markdown_to_asana_html(&item.description);
        let mut data = Map::new();
        data.insert("name".to_string(), json!(item.name));
        data.insert("html_notes".to_string(), json!(html_notes));
        data.insert("projects".to_string(), json!([project_id]));
        data.insert(
            "custom_fields".to_string(),
            json!({
                priority_field_id: priority_option_id,
            }),
        );
        let body = json!({ "data": data });

        let response: AsanaResponse<AsanaGid> = self
            .send_json(|| self.request(Method::POST, "/tasks", &[]).json(&body))
            .await?;
        Ok(response.data.gid)
    }

    async fn update_item(&self, item_id: &str, update: &PmItemUpdate) -> PmResult<()> {
        let Some(update) = update.text_update() else {
            return Ok(());
        };

        let mut data = Map::new();
        if let Some(name) = update.name {
            data.insert("name".to_string(), json!(name));
        }
        if let Some(description) = update.description {
            data.insert(
                "html_notes".to_string(),
                json!(markdown_to_asana_html(description)),
            );
        }

        let body = json!({ "data": data });
        let path = format!("/tasks/{item_id}");
        let _: AsanaResponse<Value> = self
            .send_json(|| self.request(Method::PUT, &path, &[]).json(&body))
            .await?;
        Ok(())
    }

    async fn complete_item(&self, item_id: &str) -> PmResult<()> {
        let body = AsanaRequest {
            data: UpdateTaskRequest::completed(),
        };
        let path = format!("/tasks/{item_id}");
        let _: AsanaResponse<Value> = self
            .send_json(|| self.request(Method::PUT, &path, &[]).json(&body))
            .await?;
        Ok(())
    }

    async fn comment(&self, item_id: &str, body: &str) -> PmResult<()> {
        let request = AsanaRequest {
            data: CreateStoryRequest { text: body },
        };
        let path = format!("/tasks/{item_id}/stories");
        let _: AsanaResponse<Value> = self
            .send_json(|| self.request(Method::POST, &path, &[]).json(&request))
            .await?;
        Ok(())
    }

    async fn claim_item(&self, item_id: &str, branch: &str) -> PmResult<()> {
        let me: AsanaResponse<AsanaGid> = self
            .send_json(|| self.request(Method::GET, "/users/me", &[("opt_fields", "gid")]))
            .await?;
        let my_gid = me.data.gid;

        let path = format!("/tasks/{item_id}");
        let task: AsanaResponse<Value> = self
            .send_json(|| self.request(Method::GET, &path, &[("opt_fields", CLAIM_TASK_FIELDS)]))
            .await?;
        let field_id = working_branch_field_from_task(&task.data).ok_or_else(|| {
            PmError::Message(
                "asana task is missing a Working branch custom field; run `lf op pm init` for this wave"
                    .to_string(),
            )
        })?;
        let current_branch = working_branch_value(&task.data, &field_id);
        if current_branch.is_some_and(|actual| !actual.is_empty() && actual != branch) {
            return Err(PmError::Message(format!(
                "item claimed by another worker (expected branch {branch}, got {current_branch:?})"
            )));
        }

        let mut custom_fields = Map::new();
        custom_fields.insert(field_id.clone(), json!(branch));
        let body = json!({
            "data": {
                "assignee": "me",
                "custom_fields": custom_fields
            }
        });
        let _: AsanaResponse<Value> = self
            .send_json(|| self.request(Method::PUT, &path, &[]).json(&body))
            .await?;

        let task: AsanaResponse<Value> = self
            .send_json(|| self.request(Method::GET, &path, &[("opt_fields", CLAIM_TASK_FIELDS)]))
            .await?;
        let actual_assignee = task.data.pointer("/assignee/gid").and_then(|v| v.as_str());
        if actual_assignee != Some(&my_gid) {
            return Err(PmError::Message(format!(
                "item claimed by another assignee (expected {my_gid}, got {actual_assignee:?})"
            )));
        }

        let actual_branch = working_branch_value(&task.data, &field_id);
        if actual_branch != Some(branch) {
            return Err(PmError::Message(format!(
                "item claimed by another worker (expected branch {branch}, got {actual_branch:?})"
            )));
        }

        self.comment(item_id, &format!("Working branch: `{branch}`"))
            .await?;
        Ok(())
    }
}

#[derive(Serialize)]
struct AsanaRequest<T> {
    data: T,
}

#[derive(Serialize)]
struct CreateProjectForTeamRequest<'a> {
    name: &'a str,
    notes: &'a str,
}

#[derive(Serialize)]
struct CreateTeamRequest<'a> {
    name: &'a str,
    organization: &'a str,
}

#[derive(Serialize)]
struct UpdateTaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    completed: Option<bool>,
}

impl UpdateTaskRequest {
    fn completed() -> Self {
        Self {
            completed: Some(true),
        }
    }
}

#[derive(Serialize)]
struct CreateStoryRequest<'a> {
    text: &'a str,
}

#[derive(Deserialize)]
struct AsanaResponse<T> {
    data: T,
}

#[derive(Deserialize)]
struct AsanaListResponse<T> {
    data: Vec<T>,
    next_page: Option<AsanaPageInfo>,
}

#[derive(Deserialize)]
struct AsanaTask {
    gid: String,
    name: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    html_notes: String,
    #[serde(default)]
    completed: bool,
    #[serde(default)]
    assignee: Option<AsanaGid>,
    #[serde(default)]
    custom_fields: Vec<AsanaCustomFieldValue>,
}

#[derive(Deserialize)]
struct AsanaGid {
    gid: String,
}

impl AsanaTask {
    fn into_pm_item(self) -> PmItem {
        let rank = self.priority_bucket().rank();
        let assignee = self.assignee.map(|a| a.gid);
        let description = if self.html_notes.is_empty() {
            self.notes
        } else {
            asana_html_to_markdown(&self.html_notes)
        };

        PmItem {
            id: self.gid,
            name: self.name,
            description,
            rank,
            completed: self.completed,
            assignee,
        }
    }

    fn priority_bucket(&self) -> PriorityBucket {
        self.custom_fields
            .iter()
            .find_map(AsanaPriorityField::priority_from_value)
            .unwrap_or(PriorityBucket::Low)
    }
}

#[derive(Deserialize)]
struct AsanaCustomFieldValue {
    #[serde(default)]
    resource_subtype: String,
    #[serde(default)]
    enum_value: Option<AsanaEnumOption>,
    #[serde(default)]
    enum_options: Vec<AsanaEnumOption>,
}

#[derive(Deserialize)]
struct AsanaCustomFieldSetting {
    custom_field: AsanaCustomFieldMetadata,
}

#[derive(Deserialize)]
struct AsanaCustomFieldMetadata {
    gid: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    resource_subtype: String,
    #[serde(default)]
    enum_options: Vec<AsanaEnumOption>,
}

#[derive(Deserialize)]
struct AsanaEnumOption {
    gid: String,
    #[serde(default)]
    name: String,
}

#[derive(Debug)]
struct AsanaPriorityField {
    gid: String,
    options: [String; 4],
}

impl AsanaPriorityField {
    fn from_metadata(field: AsanaCustomFieldMetadata) -> Option<Self> {
        if field.resource_subtype != "enum" {
            return None;
        }

        let mut options: [Option<String>; 4] = std::array::from_fn(|_| None);

        for option in field.enum_options {
            if let Some(bucket) = PriorityBucket::from_semantic_label(&option.name) {
                options[usize::from(bucket.order())] = Some(option.gid);
            }
        }

        Some(Self {
            gid: field.gid,
            options: [
                options[0].take()?,
                options[1].take()?,
                options[2].take()?,
                options[3].take()?,
            ],
        })
    }

    fn priority_from_value(field: &AsanaCustomFieldValue) -> Option<PriorityBucket> {
        if field.resource_subtype != "enum" {
            return None;
        }
        let mut buckets = [false; 4];
        for option in &field.enum_options {
            if let Some(bucket) = PriorityBucket::from_semantic_label(&option.name) {
                buckets[usize::from(bucket.order())] = true;
            }
        }
        if buckets.into_iter().any(|present| !present) {
            return None;
        }
        let current = field.enum_value.as_ref()?;
        PriorityBucket::from_semantic_label(&current.name)
    }

    fn option_gid(&self, priority: PriorityBucket) -> &str {
        &self.options[usize::from(priority.order())]
    }
}

fn working_branch_gid_from_metadata(field: AsanaCustomFieldMetadata) -> Option<String> {
    if field.name == WORKING_BRANCH_FIELD_NAME && field.resource_subtype == "text" {
        Some(field.gid)
    } else {
        None
    }
}

fn working_branch_field_from_task(task: &Value) -> Option<String> {
    task.pointer("/custom_fields")?
        .as_array()?
        .iter()
        .find(|field| {
            field.pointer("/name").and_then(|v| v.as_str()) == Some(WORKING_BRANCH_FIELD_NAME)
                && field.pointer("/resource_subtype").and_then(|v| v.as_str()) == Some("text")
        })
        .and_then(|field| field.pointer("/gid"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn working_branch_value<'a>(task: &'a Value, field_id: &str) -> Option<&'a str> {
    task.pointer("/custom_fields")?
        .as_array()?
        .iter()
        .find(|field| field.pointer("/gid").and_then(|v| v.as_str()) == Some(field_id))
        .and_then(|field| field.pointer("/text_value"))
        .and_then(|value| value.as_str())
}

#[derive(Debug, Clone, Deserialize)]
pub struct AsanaWorkspace {
    pub gid: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AsanaTeam {
    gid: String,
    name: String,
}

#[derive(Deserialize)]
struct AsanaProjectNode {
    gid: String,
    name: String,
}

#[derive(Deserialize)]
struct AsanaPageInfo {
    offset: Option<String>,
}

#[derive(Deserialize)]
struct AsanaErrorBody {
    errors: Vec<AsanaErrorMessage>,
}

#[derive(Deserialize)]
struct AsanaErrorMessage {
    message: String,
}

async fn parse_response<T: DeserializeOwned>(response: reqwest::Response) -> PmResult<T> {
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|err| PmError::Message(format!("failed to read asana response: {err}")))?;

    if !status.is_success() {
        return Err(PmError::Message(parse_error_message(status, &body)));
    }

    serde_json::from_slice(&body)
        .map_err(|err| PmError::Message(format!("failed to decode asana response: {err}")))
}

fn parse_error_message(status: StatusCode, body: &[u8]) -> String {
    if let Ok(error_body) = serde_json::from_slice::<AsanaErrorBody>(body) {
        if let Some(error) = error_body.errors.first() {
            if error.message.contains("Missing required `team` field") {
                return format!(
                    "asana request failed with status {status}: Missing required `team` field. Set `pm.team` in wave/<name>/<name>.yaml or `asana.default_team` in .lf/config.yaml."
                );
            }
            return format!(
                "asana request failed with status {status}: {}",
                error.message
            );
        }
    }

    let body_text = String::from_utf8_lossy(body).trim().to_string();
    if body_text.is_empty() {
        format!("asana request failed with status {status}")
    } else {
        format!("asana request failed with status {status}: {body_text}")
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{json, Map, Value};

    use super::*;
    use crate::engine::config::AsanaConfig;
    use crate::lfd::pm::test_server::{self, json_response, response};
    use crate::lfd::pm::PmProvider;

    fn working_branch_metadata() -> Value {
        json!({
            "gid": "field-branch",
            "name": "Working branch",
            "resource_subtype": "text"
        })
    }

    fn working_branch_setting() -> Value {
        json!({ "custom_field": working_branch_metadata() })
    }

    fn working_branch_field(branch: Option<&str>) -> Value {
        let mut field = Map::new();
        field.insert("gid".to_string(), json!("field-branch"));
        field.insert("name".to_string(), json!("Working branch"));
        field.insert("resource_subtype".to_string(), json!("text"));
        if let Some(branch) = branch {
            field.insert("text_value".to_string(), json!(branch));
        }
        Value::Object(field)
    }

    fn task_with_working_branch(assignee: Option<&str>, branch: Option<&str>) -> Value {
        let mut task = Map::new();
        task.insert("gid".to_string(), json!("task-1"));
        if let Some(assignee) = assignee {
            task.insert("assignee".to_string(), json!({ "gid": assignee }));
        }
        task.insert(
            "custom_fields".to_string(),
            json!([working_branch_field(branch)]),
        );
        Value::Object(task)
    }

    #[tokio::test]
    async fn create_project_uses_workspace_and_team() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::CREATED,
            json!({ "data": { "gid": "project-123" } }),
        )])
        .await;
        let client = AsanaClient::with_base_url(
            "secret-token".to_string(),
            AsanaConfig {
                workspace: Some("workspace-1".to_string()),
                default_team: Some("team-9".to_string()),
            },
            base_url,
        );

        let project_id = client
            .create_project("Wave PM", "Ship the Asana client")
            .await
            .expect("create project should succeed");

        assert_eq!(project_id, "project-123");
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].path, "/teams/team-9/projects");
        assert_eq!(
            requests[0].authorization.as_deref(),
            Some("Bearer secret-token")
        );
        let body: Value = serde_json::from_str(&requests[0].body).expect("json body");
        assert_eq!(
            body,
            json!({
                "data": {
                    "name": "Wave PM",
                    "notes": "Ship the Asana client"
                }
            })
        );
    }

    #[tokio::test]
    async fn create_project_reuses_existing_loopflow_team_when_default_missing() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({
                    "data": [
                        { "gid": "team-1", "name": "Waves" },
                        { "gid": "team-2", "name": "Other" }
                    ]
                }),
            ),
            json_response(
                StatusCode::CREATED,
                json!({ "data": { "gid": "project-123" } }),
            ),
        ])
        .await;
        let client = AsanaClient::with_base_url(
            "secret-token".to_string(),
            AsanaConfig {
                workspace: Some("workspace-1".to_string()),
                default_team: None,
            },
            base_url,
        );

        let project_id = client
            .create_project("Wave PM", "Ship the Asana client")
            .await
            .expect("create project should succeed");

        assert_eq!(project_id, "project-123");
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].path, "/workspaces/workspace-1/teams");
        assert_eq!(requests[1].method, "POST");
        assert_eq!(requests[1].path, "/teams/team-1/projects");
    }

    #[tokio::test]
    async fn create_project_creates_loopflow_team_when_missing() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(StatusCode::OK, json!({ "data": [] })),
            json_response(
                StatusCode::CREATED,
                json!({ "data": { "gid": "team-loopflow" } }),
            ),
            json_response(
                StatusCode::CREATED,
                json!({ "data": { "gid": "project-123" } }),
            ),
        ])
        .await;
        let client = AsanaClient::with_base_url(
            "secret-token".to_string(),
            AsanaConfig {
                workspace: Some("workspace-1".to_string()),
                default_team: None,
            },
            base_url,
        );

        let project_id = client
            .create_project("Wave PM", "Ship the Asana client")
            .await
            .expect("create project should succeed");

        assert_eq!(project_id, "project-123");
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].path, "/workspaces/workspace-1/teams");
        assert_eq!(requests[1].method, "POST");
        assert_eq!(requests[1].path, "/teams");
        assert_eq!(
            serde_json::from_str::<Value>(&requests[1].body).expect("json body"),
            json!({
                "data": {
                    "name": "Waves",
                    "organization": "workspace-1"
                }
            })
        );
        assert_eq!(requests[2].method, "POST");
        assert_eq!(requests[2].path, "/teams/team-loopflow/projects");
    }

    #[tokio::test]
    async fn create_project_fails_with_no_workspaces() {
        let (base_url, _requests) =
            test_server::spawn(vec![json_response(StatusCode::OK, json!({ "data": [] }))]).await;
        let client = AsanaClient::with_base_url(
            "secret-token".to_string(),
            AsanaConfig::default(),
            base_url,
        );

        let error = client
            .create_project("Wave PM", "Ship the Asana client")
            .await
            .expect_err("should fail with no workspaces");

        assert_eq!(
            error,
            PmError::Message("no asana workspaces found for this token".to_string())
        );
    }

    #[test]
    fn parse_error_message_for_missing_team_is_actionable() {
        let body = serde_json::to_vec(&json!({
            "errors": [{ "message": "Missing required `team` field" }]
        }))
        .expect("json body");

        let message = parse_error_message(StatusCode::BAD_REQUEST, &body);

        assert!(message.contains("pm.team"));
        assert!(message.contains("asana.default_team"));
    }

    #[tokio::test]
    async fn list_items_collects_all_pages_and_maps_priority_buckets() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({
                    "data": [
                        {
                            "gid": "task-1",
                            "name": "First",
                            "notes": "one",
                            "completed": false,
                            "custom_fields": [{
                                "gid": "field-priority",
                                "name": "Priority",
                                "resource_subtype": "enum",
                                "enum_value": { "gid": "opt-p2", "name": "Medium" },
                                "enum_options": [
                                    { "gid": "opt-p0", "name": "Urgent" },
                                    { "gid": "opt-p1", "name": "High" },
                                    { "gid": "opt-p2", "name": "Medium" },
                                    { "gid": "opt-p3", "name": "Low" }
                                ]
                            }]
                        },
                        {
                            "gid": "task-2",
                            "name": "Second",
                            "notes": "fallback two",
                            "html_notes": "<p><strong>two</strong></p>",
                            "completed": true,
                            "custom_fields": [{
                                "gid": "field-priority",
                                "name": "Priority",
                                "resource_subtype": "enum",
                                "enum_value": { "gid": "opt-p0", "name": "Urgent" },
                                "enum_options": [
                                    { "gid": "opt-p0", "name": "Urgent" },
                                    { "gid": "opt-p1", "name": "High" },
                                    { "gid": "opt-p2", "name": "Medium" },
                                    { "gid": "opt-p3", "name": "Low" }
                                ]
                            }]
                        }
                    ],
                    "next_page": { "offset": "cursor-2" }
                }),
            ),
            json_response(
                StatusCode::OK,
                json!({
                    "data": [
                        {
                            "gid": "task-3",
                            "name": "Third",
                            "notes": "fallback three",
                            "html_notes": "<p>three</p>",
                            "completed": false,
                            "custom_fields": [{
                                "gid": "field-priority",
                                "name": "Priority",
                                "resource_subtype": "enum",
                                "enum_value": { "gid": "opt-p1", "name": "High" },
                                "enum_options": [
                                    { "gid": "opt-p0", "name": "Urgent" },
                                    { "gid": "opt-p1", "name": "High" },
                                    { "gid": "opt-p2", "name": "Medium" },
                                    { "gid": "opt-p3", "name": "Low" }
                                ]
                            }]
                        }
                    ],
                    "next_page": null
                }),
            ),
        ])
        .await;
        let client = AsanaClient::with_base_url(
            "secret-token".to_string(),
            AsanaConfig::default(),
            base_url,
        );

        let items = client
            .list_items("project-123")
            .await
            .expect("list items should succeed");

        assert_eq!(
            items,
            vec![
                PmItem {
                    id: "task-2".to_string(),
                    name: "Second".to_string(),
                    description: "**two**".to_string(),
                    rank: 0,
                    completed: true,
                    assignee: None,
                },
                PmItem {
                    id: "task-3".to_string(),
                    name: "Third".to_string(),
                    description: "three".to_string(),
                    rank: 1,
                    completed: false,
                    assignee: None,
                },
                PmItem {
                    id: "task-1".to_string(),
                    name: "First".to_string(),
                    description: "one".to_string(),
                    rank: 2,
                    completed: false,
                    assignee: None,
                },
            ]
        );
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].path, "/projects/project-123/tasks");
        assert!(requests[0]
            .query
            .as_deref()
            .expect("query")
            .starts_with("opt_fields="));
        assert!(requests[1]
            .query
            .as_deref()
            .expect("query")
            .contains("offset=cursor-2"));
    }

    #[test]
    fn into_pm_item_prefers_html_notes_and_falls_back_to_notes() {
        let html_task = AsanaTask {
            gid: "task-html".to_string(),
            name: "Rich".to_string(),
            notes: "plaintext".to_string(),
            html_notes: "<p><strong>formatted</strong></p>".to_string(),
            completed: false,
            assignee: Some(AsanaGid {
                gid: "user-1".to_string(),
            }),
            custom_fields: Vec::new(),
        };
        let plain_task = AsanaTask {
            gid: "task-plain".to_string(),
            name: "Plain".to_string(),
            notes: "plaintext".to_string(),
            html_notes: String::new(),
            completed: true,
            assignee: None,
            custom_fields: Vec::new(),
        };

        assert_eq!(html_task.into_pm_item().description, "**formatted**");
        let item = plain_task.into_pm_item();
        assert_eq!(item.description, "plaintext");
        assert!(item.completed);
        assert_eq!(item.assignee, None);
    }

    #[tokio::test]
    async fn create_update_complete_and_comment_map_to_asana_endpoints() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({
                    "data": [{
                        "custom_field": {
                            "gid": "field-priority",
                            "name": "Priority",
                            "resource_subtype": "enum",
                            "enum_options": [
                                { "gid": "opt-p0", "name": "Urgent" },
                                { "gid": "opt-p1", "name": "High" },
                                { "gid": "opt-p2", "name": "Medium" },
                                { "gid": "opt-p3", "name": "Low" }
                            ]
                        }
                    }]
                }),
            ),
            json_response(
                StatusCode::CREATED,
                json!({ "data": { "gid": "task-123" } }),
            ),
            json_response(StatusCode::OK, json!({ "data": { "gid": "task-123" } })),
            json_response(StatusCode::OK, json!({ "data": { "gid": "task-123" } })),
            json_response(StatusCode::CREATED, json!({ "data": { "gid": "story-1" } })),
        ])
        .await;
        let client = AsanaClient::with_base_url(
            "secret-token".to_string(),
            AsanaConfig::default(),
            base_url,
        );

        let item_id = client
            .create_item(
                "project-123",
                &PmItemCreate {
                    name: "Implement client".to_string(),
                    description: "Build the HTTP adapter".to_string(),
                    rank: 0,
                },
            )
            .await
            .expect("create item should succeed");
        client
            .update_item(
                &item_id,
                &PmItemUpdate {
                    name: Some("Implement Asana client".to_string()),
                    description: Some("Build the HTTP adapter and tests".to_string()),
                    rank: Some(1),
                },
            )
            .await
            .expect("update item should succeed");
        client
            .complete_item(&item_id)
            .await
            .expect("complete item should succeed");
        client
            .comment(&item_id, "Shipped in v0.9.9")
            .await
            .expect("comment should succeed");

        assert_eq!(item_id, "task-123");
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 5);

        assert_eq!(requests[0].method, "GET");
        assert_eq!(
            requests[0].path,
            "/projects/project-123/custom_field_settings"
        );
        assert_eq!(requests[1].method, "POST");
        assert_eq!(requests[1].path, "/tasks");
        assert_eq!(
            serde_json::from_str::<Value>(&requests[1].body).expect("json body"),
            json!({
                "data": {
                    "name": "Implement client",
                    "html_notes": "<body>Build the HTTP adapter\n</body>",
                    "projects": ["project-123"],
                    "custom_fields": {
                        "field-priority": "opt-p0"
                    }
                }
            })
        );

        assert_eq!(requests[2].method, "PUT");
        assert_eq!(requests[2].path, "/tasks/task-123");
        assert_eq!(
            serde_json::from_str::<Value>(&requests[2].body).expect("json body"),
            json!({
                "data": {
                    "name": "Implement Asana client",
                    "html_notes": "<body>Build the HTTP adapter and tests\n</body>"
                }
            })
        );

        assert_eq!(requests[3].method, "PUT");
        assert_eq!(requests[3].path, "/tasks/task-123");
        assert_eq!(
            serde_json::from_str::<Value>(&requests[3].body).expect("json body"),
            json!({ "data": { "completed": true } })
        );

        assert_eq!(requests[4].method, "POST");
        assert_eq!(requests[4].path, "/tasks/task-123/stories");
        assert_eq!(
            serde_json::from_str::<Value>(&requests[4].body).expect("json body"),
            json!({ "data": { "text": "Shipped in v0.9.9" } })
        );
    }

    #[tokio::test]
    async fn update_item_skips_rank_only_updates() {
        let (base_url, requests) = test_server::spawn(Vec::new()).await;
        let client = AsanaClient::with_base_url(
            "secret-token".to_string(),
            AsanaConfig::default(),
            base_url,
        );

        client
            .update_item(
                "task-123",
                &PmItemUpdate {
                    name: None,
                    description: None,
                    rank: Some(2),
                },
            )
            .await
            .expect("rank-only update should no-op");

        assert!(requests.lock().await.is_empty());
    }

    #[tokio::test]
    async fn retries_after_rate_limit_response() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({
                    "data": [{
                        "custom_field": {
                            "gid": "field-priority",
                            "name": "Priority",
                            "resource_subtype": "enum",
                            "enum_options": [
                                { "gid": "opt-p0", "name": "Urgent" },
                                { "gid": "opt-p1", "name": "High" },
                                { "gid": "opt-p2", "name": "Medium" },
                                { "gid": "opt-p3", "name": "Low" }
                            ]
                        }
                    }]
                }),
            ),
            response(
                StatusCode::TOO_MANY_REQUESTS,
                vec![("retry-after", "0")],
                json!({ "errors": [{ "message": "slow down" }] }).to_string(),
            ),
            json_response(
                StatusCode::CREATED,
                json!({ "data": { "gid": "task-123" } }),
            ),
        ])
        .await;
        let client = AsanaClient::with_base_url(
            "secret-token".to_string(),
            AsanaConfig::default(),
            base_url,
        );

        let item_id = client
            .create_item(
                "project-123",
                &PmItemCreate {
                    name: "Implement client".to_string(),
                    description: "Build the HTTP adapter".to_string(),
                    rank: 1,
                },
            )
            .await
            .expect("request should succeed after retry");

        assert_eq!(item_id, "task-123");
        assert_eq!(requests.lock().await.len(), 3);
    }

    #[tokio::test]
    async fn surfaces_asana_error_messages() {
        let (base_url, _requests) = test_server::spawn(vec![json_response(
            StatusCode::BAD_REQUEST,
            json!({ "errors": [{ "message": "workspace is required" }] }),
        )])
        .await;
        let client = AsanaClient::with_base_url(
            "secret-token".to_string(),
            AsanaConfig {
                workspace: Some("workspace-1".to_string()),
                default_team: None,
            },
            base_url,
        );

        let error = client
            .create_project("Wave PM", "Ship the Asana client")
            .await
            .expect_err("asana error should surface");

        assert_eq!(
            error,
            PmError::Message(
                "asana request failed with status 400 Bad Request: workspace is required"
                    .to_string()
            )
        );
    }

    #[tokio::test]
    async fn init_project_attaches_workspace_working_branch_field() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(StatusCode::OK, json!({ "data": [] })),
            json_response(
                StatusCode::OK,
                json!({ "data": [working_branch_metadata()] }),
            ),
            json_response(StatusCode::OK, json!({ "data": { "gid": "setting-1" } })),
            json_response(
                StatusCode::OK,
                json!({ "data": [working_branch_setting()] }),
            ),
        ])
        .await;
        let client = AsanaClient::with_base_url(
            "secret-token".to_string(),
            AsanaConfig {
                workspace: Some("workspace-1".to_string()),
                default_team: None,
            },
            base_url,
        );

        client
            .init_project("project-123")
            .await
            .expect("init should attach the shared field");

        let requests = requests.lock().await;
        assert_eq!(requests.len(), 4);
        assert_eq!(
            requests[0].path,
            "/projects/project-123/custom_field_settings"
        );
        assert_eq!(requests[1].path, "/workspaces/workspace-1/custom_fields");
        assert_eq!(
            requests[2].path,
            "/projects/project-123/addCustomFieldSetting"
        );
        assert_eq!(
            serde_json::from_str::<Value>(&requests[2].body).expect("json body"),
            json!({ "data": { "custom_field": "field-branch" } })
        );
    }

    #[tokio::test]
    async fn init_project_creates_workspace_working_branch_field_when_missing() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(StatusCode::OK, json!({ "data": [] })),
            json_response(StatusCode::OK, json!({ "data": [] })),
            json_response(
                StatusCode::CREATED,
                json!({ "data": { "gid": "field-branch" } }),
            ),
            json_response(StatusCode::OK, json!({ "data": { "gid": "setting-1" } })),
            json_response(
                StatusCode::OK,
                json!({ "data": [working_branch_setting()] }),
            ),
        ])
        .await;
        let client = AsanaClient::with_base_url(
            "secret-token".to_string(),
            AsanaConfig {
                workspace: Some("workspace-1".to_string()),
                default_team: None,
            },
            base_url,
        );

        client
            .init_project("project-123")
            .await
            .expect("init should create and attach the shared field");

        let requests = requests.lock().await;
        assert_eq!(requests.len(), 5);
        assert_eq!(requests[2].path, "/custom_fields");
        assert_eq!(
            serde_json::from_str::<Value>(&requests[2].body).expect("json body"),
            json!({
                "data": {
                    "name": "Working branch",
                    "resource_subtype": "text",
                    "workspace": "workspace-1"
                }
            })
        );
    }

    #[tokio::test]
    async fn claim_item_verifies_working_branch_matches() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(StatusCode::OK, json!({ "data": { "gid": "user-42" } })),
            json_response(
                StatusCode::OK,
                json!({ "data": task_with_working_branch(None, None) }),
            ),
            json_response(StatusCode::OK, json!({ "data": { "gid": "task-1" } })),
            json_response(
                StatusCode::OK,
                json!({ "data": task_with_working_branch(Some("user-42"), Some("feature/test")) }),
            ),
            json_response(StatusCode::CREATED, json!({ "data": { "gid": "story-1" } })),
        ])
        .await;
        let client = AsanaClient::with_base_url(
            "secret-token".to_string(),
            AsanaConfig::default(),
            base_url,
        );

        client
            .claim_item("task-1", "feature/test")
            .await
            .expect("claim should succeed when branch matches");

        let requests = requests.lock().await;
        assert_eq!(requests.len(), 5);
        assert_eq!(requests[0].method, "GET");
        assert!(requests[0].path.contains("/users/me"));
        assert_eq!(requests[1].method, "GET");
        assert_eq!(requests[2].method, "PUT");
        assert_eq!(
            serde_json::from_str::<Value>(&requests[2].body).expect("json body"),
            json!({
                "data": {
                    "assignee": "me",
                    "custom_fields": { "field-branch": "feature/test" }
                }
            })
        );
        assert_eq!(requests[3].method, "GET");
        assert_eq!(requests[4].method, "POST");
    }

    #[tokio::test]
    async fn claim_item_fails_when_another_assignee_wins() {
        let (base_url, _requests) = test_server::spawn(vec![
            json_response(StatusCode::OK, json!({ "data": { "gid": "user-42" } })),
            json_response(
                StatusCode::OK,
                json!({ "data": task_with_working_branch(None, None) }),
            ),
            json_response(StatusCode::OK, json!({ "data": { "gid": "task-1" } })),
            json_response(
                StatusCode::OK,
                json!({ "data": task_with_working_branch(Some("user-99"), Some("feature/test")) }),
            ),
        ])
        .await;
        let client = AsanaClient::with_base_url(
            "secret-token".to_string(),
            AsanaConfig::default(),
            base_url,
        );

        let error = client
            .claim_item("task-1", "feature/test")
            .await
            .expect_err("claim should fail when another assignee wins");
        assert!(error.to_string().contains("another assignee"));
    }

    #[tokio::test]
    async fn claim_item_fails_same_account_concurrent() {
        let (base_url, _requests) = test_server::spawn(vec![
            json_response(StatusCode::OK, json!({ "data": { "gid": "user-42" } })),
            json_response(
                StatusCode::OK,
                json!({ "data": task_with_working_branch(None, None) }),
            ),
            json_response(StatusCode::OK, json!({ "data": { "gid": "task-1" } })),
            json_response(
                StatusCode::OK,
                json!({ "data": task_with_working_branch(Some("user-42"), Some("worker-a")) }),
            ),
            json_response(StatusCode::CREATED, json!({ "data": { "gid": "story-1" } })),
            json_response(StatusCode::OK, json!({ "data": { "gid": "user-42" } })),
            json_response(
                StatusCode::OK,
                json!({ "data": task_with_working_branch(None, Some("worker-a")) }),
            ),
            json_response(StatusCode::OK, json!({ "data": { "gid": "task-1" } })),
            json_response(
                StatusCode::OK,
                json!({ "data": task_with_working_branch(Some("user-42"), Some("worker-a")) }),
            ),
        ])
        .await;
        let client = AsanaClient::with_base_url(
            "secret-token".to_string(),
            AsanaConfig::default(),
            base_url,
        );

        client
            .claim_item("task-1", "worker-a")
            .await
            .expect("first branch should win");
        let error = client
            .claim_item("task-1", "worker-b")
            .await
            .expect_err("second branch should lose even with the same account");
        assert!(error.to_string().contains("another worker"));
    }
}
