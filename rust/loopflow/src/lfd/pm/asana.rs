use async_trait::async_trait;
use reqwest::{Method, StatusCode, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio::time::sleep;
use tracing::warn;

use crate::engine::config::AsanaConfig;
use crate::lfd::pm::{
    PmError, PmItem, PmItemCreate, PmItemUpdate, PmProject, PmProvider, PmResult, PriorityBucket,
    RATE_LIMIT_RETRIES,
};

const ASANA_BASE_URL: &str = "https://app.asana.com/api/1.0";
const TASK_FIELDS: &str = "name,notes,completed,custom_fields.gid,custom_fields.name,custom_fields.resource_subtype,custom_fields.enum_value.gid,custom_fields.enum_value.name,custom_fields.enum_options.gid,custom_fields.enum_options.name";
const TASK_PRIORITY_FIELDS: &str = "custom_fields.gid,custom_fields.name,custom_fields.resource_subtype,custom_fields.enum_value.gid,custom_fields.enum_value.name,custom_fields.enum_options.gid,custom_fields.enum_options.name";
const PROJECT_PRIORITY_FIELD_FIELDS: &str =
    "custom_field.gid,custom_field.name,custom_field.resource_subtype,custom_field.enum_options.gid,custom_field.enum_options.name";
const DEFAULT_LOOPFLOW_TEAM_NAME: &str = "Loopflow";
const PRIORITY_FIELD_NAME: &str = "Priority";

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

    async fn create_team(&self, workspace_id: &str, name: &str) -> PmResult<String> {
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

        self.create_team(workspace_id, DEFAULT_LOOPFLOW_TEAM_NAME)
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
                        { "name": "Urgent" },
                        { "name": "High" },
                        { "name": "Medium" },
                        { "name": "Low" }
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

    async fn priority_field_for_task(&self, item_id: &str) -> PmResult<AsanaPriorityField> {
        let path = task_path(item_id);
        let response: AsanaResponse<AsanaTaskDetails> = self
            .send_json(|| self.request(Method::GET, &path, &[("opt_fields", TASK_PRIORITY_FIELDS)]))
            .await?;

        response
            .data
            .custom_fields
            .into_iter()
            .find_map(AsanaPriorityField::from_value)
            .ok_or_else(|| {
                PmError::Message(format!(
                    "asana task {item_id} is missing a priority custom field"
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
                        .priority
                        .order()
                        .cmp(&right.1.priority.order())
                        .then_with(|| left.0.cmp(&right.0))
                });
                return Ok(items.into_iter().map(|(_, item)| item).collect());
            }
        }
    }

    async fn create_item(&self, project_id: &str, item: &PmItemCreate) -> PmResult<String> {
        let priority_field = self.ensure_priority_field_for_project(project_id).await?;
        let priority_field_id = priority_field.gid.clone();
        let priority_option_id = priority_field.option_gid(item.priority).to_string();
        let mut data = Map::new();
        data.insert("name".to_string(), json!(item.name));
        data.insert("notes".to_string(), json!(item.description));
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
        if update.is_noop() {
            return Ok(());
        }

        let mut data = Map::new();
        if let Some(name) = update.name.as_deref() {
            data.insert("name".to_string(), json!(name));
        }
        if let Some(description) = update.description.as_deref() {
            data.insert("notes".to_string(), json!(description));
        }
        if let Some(priority) = update.priority {
            let field = self.priority_field_for_task(item_id).await?;
            let field_id = field.gid.clone();
            let option_id = field.option_gid(priority).to_string();
            data.insert(
                "custom_fields".to_string(),
                json!({
                    field_id: option_id,
                }),
            );
        }

        let body = json!({ "data": data });
        let path = task_path(item_id);
        let _: AsanaResponse<Value> = self
            .send_json(|| self.request(Method::PUT, &path, &[]).json(&body))
            .await?;
        Ok(())
    }

    async fn complete_item(&self, item_id: &str) -> PmResult<()> {
        let body = AsanaRequest {
            data: UpdateTaskRequest::completed(),
        };
        let path = task_path(item_id);
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
struct UpdateTaskRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    completed: Option<bool>,
    #[serde(skip_serializing)]
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> UpdateTaskRequest<'a> {
    fn completed() -> Self {
        Self {
            completed: Some(true),
            _marker: std::marker::PhantomData,
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
    completed: bool,
    #[serde(default)]
    custom_fields: Vec<AsanaCustomFieldValue>,
}

#[derive(Deserialize)]
struct AsanaTaskDetails {
    #[serde(default)]
    custom_fields: Vec<AsanaCustomFieldValue>,
}

#[derive(Deserialize)]
struct AsanaGid {
    gid: String,
}

impl AsanaTask {
    fn into_pm_item(self) -> PmItem {
        let priority = self.priority_bucket();
        PmItem {
            id: self.gid,
            name: self.name,
            description: self.notes,
            priority,
            completed: self.completed,
        }
    }

    fn priority_bucket(&self) -> PriorityBucket {
        self.custom_fields
            .iter()
            .find_map(AsanaPriorityField::priority_from_value)
            .unwrap_or(PriorityBucket::Low)
    }
}

#[derive(Clone, Deserialize)]
struct AsanaCustomFieldValue {
    gid: String,
    #[serde(default)]
    resource_subtype: String,
    #[serde(default)]
    enum_value: Option<AsanaEnumOption>,
    #[serde(default)]
    enum_options: Vec<AsanaEnumOption>,
}

#[derive(Clone, Deserialize)]
struct AsanaCustomFieldSetting {
    custom_field: AsanaCustomFieldMetadata,
}

#[derive(Clone, Deserialize)]
struct AsanaCustomFieldMetadata {
    gid: String,
    #[serde(default)]
    resource_subtype: String,
    #[serde(default)]
    enum_options: Vec<AsanaEnumOption>,
}

#[derive(Clone, Deserialize)]
struct AsanaEnumOption {
    gid: String,
    #[serde(default)]
    name: String,
}

#[derive(Debug, Clone)]
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

    fn from_value(field: AsanaCustomFieldValue) -> Option<Self> {
        Self::from_metadata(AsanaCustomFieldMetadata {
            gid: field.gid,
            resource_subtype: field.resource_subtype,
            enum_options: field.enum_options,
        })
    }

    fn priority_from_value(field: &AsanaCustomFieldValue) -> Option<PriorityBucket> {
        Self::from_value(field.clone())?;
        let current = field.enum_value.as_ref()?;
        PriorityBucket::from_semantic_label(&current.name)
    }

    fn option_gid(&self, priority: PriorityBucket) -> &str {
        &self.options[usize::from(priority.order())]
    }
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

fn task_path(item_id: &str) -> String {
    format!("/tasks/{item_id}")
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use super::*;
    use crate::engine::config::AsanaConfig;
    use crate::lfd::pm::test_server::{self, json_response, response};
    use crate::lfd::pm::PmProvider;

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
                        { "gid": "team-1", "name": "Loopflow" },
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
                    "name": "Loopflow",
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
                            "notes": "two",
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
                            "notes": "three",
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
                    description: "two".to_string(),
                    priority: PriorityBucket::Urgent,
                    completed: true,
                },
                PmItem {
                    id: "task-3".to_string(),
                    name: "Third".to_string(),
                    description: "three".to_string(),
                    priority: PriorityBucket::High,
                    completed: false,
                },
                PmItem {
                    id: "task-1".to_string(),
                    name: "First".to_string(),
                    description: "one".to_string(),
                    priority: PriorityBucket::Medium,
                    completed: false,
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
            json_response(
                StatusCode::OK,
                json!({
                    "data": {
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
                }),
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
                    priority: PriorityBucket::Urgent,
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
                    priority: Some(PriorityBucket::High),
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
        assert_eq!(requests.len(), 6);

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
                    "notes": "Build the HTTP adapter",
                    "projects": ["project-123"],
                    "custom_fields": {
                        "field-priority": "opt-p0"
                    }
                }
            })
        );

        assert_eq!(requests[2].method, "GET");
        assert_eq!(requests[2].path, "/tasks/task-123");
        assert_eq!(requests[3].method, "PUT");
        assert_eq!(requests[3].path, "/tasks/task-123");
        assert_eq!(
            serde_json::from_str::<Value>(&requests[3].body).expect("json body"),
            json!({
                "data": {
                    "name": "Implement Asana client",
                    "notes": "Build the HTTP adapter and tests",
                    "custom_fields": {
                        "field-priority": "opt-p1"
                    }
                }
            })
        );

        assert_eq!(requests[4].method, "PUT");
        assert_eq!(requests[4].path, "/tasks/task-123");
        assert_eq!(
            serde_json::from_str::<Value>(&requests[4].body).expect("json body"),
            json!({ "data": { "completed": true } })
        );

        assert_eq!(requests[5].method, "POST");
        assert_eq!(requests[5].path, "/tasks/task-123/stories");
        assert_eq!(
            serde_json::from_str::<Value>(&requests[5].body).expect("json body"),
            json!({ "data": { "text": "Shipped in v0.9.9" } })
        );
    }

    #[tokio::test]
    async fn update_item_sends_priority_only_updates() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({
                    "data": {
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
                }),
            ),
            json_response(StatusCode::OK, json!({ "data": { "gid": "task-123" } })),
        ])
        .await;
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
                    priority: Some(PriorityBucket::Medium),
                },
            )
            .await
            .expect("priority-only update should succeed");

        let requests = requests.lock().await;
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].path, "/tasks/task-123");
        assert_eq!(
            serde_json::from_str::<Value>(&requests[1].body).expect("json body"),
            json!({
                "data": {
                    "custom_fields": {
                        "field-priority": "opt-p2"
                    }
                }
            })
        );
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
                    priority: PriorityBucket::High,
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
}
