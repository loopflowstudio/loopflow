use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Method, StatusCode, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::sleep;
use tracing::warn;

use crate::engine::config::AsanaConfig;
use crate::lfd::pm::{PmError, PmItem, PmItemCreate, PmItemUpdate, PmProvider, PmResult};

const ASANA_BASE_URL: &str = "https://app.asana.com/api/1.0";
const ASANA_RATE_LIMIT_RETRIES: u8 = 3;
const ASANA_RETRY_AFTER_FALLBACK: Duration = Duration::from_secs(60);
const TASK_FIELDS: &str = "name,notes,completed";
const DEFAULT_LOOPFLOW_TEAM_NAME: &str = "Loopflow";

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
        for attempt in 0..=ASANA_RATE_LIMIT_RETRIES {
            let response = make_request()
                .send()
                .await
                .map_err(|err| PmError::Message(format!("asana request failed: {err}")))?;

            if response.status() == StatusCode::TOO_MANY_REQUESTS
                && attempt < ASANA_RATE_LIMIT_RETRIES
            {
                let delay = retry_after_delay(response.headers());
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
}

#[async_trait]
impl PmProvider for AsanaClient {
    async fn create_project(&self, name: &str, description: &str) -> PmResult<String> {
        let workspace = self.resolve_workspace().await?;
        let team = self.resolve_team_for_project_bootstrap(&workspace).await?;
        self.create_project_for_team(&team, name, description).await
    }

    async fn list_items(&self, project_id: &str) -> PmResult<Vec<PmItem>> {
        let path = format!("/projects/{project_id}/tasks");
        let mut offset = None;
        let mut items = Vec::new();

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
                items.push(task.into_pm_item(items.len() as u32));
            }

            offset = response.next_page.and_then(|page| page.offset);
            if offset.is_none() {
                return Ok(items);
            }
        }
    }

    async fn create_item(&self, project_id: &str, item: &PmItemCreate) -> PmResult<String> {
        let body = AsanaRequest {
            data: CreateTaskRequest {
                name: &item.name,
                notes: &item.description,
                projects: [project_id],
            },
        };

        let response: AsanaResponse<AsanaGid> = self
            .send_json(|| self.request(Method::POST, "/tasks", &[]).json(&body))
            .await?;
        Ok(response.data.gid)
    }

    async fn update_item(&self, item_id: &str, update: &PmItemUpdate) -> PmResult<()> {
        let Some(update) = update.text_update() else {
            return Ok(());
        };

        let body = AsanaRequest {
            data: UpdateTaskRequest::from(update),
        };
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
struct CreateTaskRequest<'a> {
    name: &'a str,
    notes: &'a str,
    projects: [&'a str; 1],
}

#[derive(Serialize)]
struct UpdateTaskRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed: Option<bool>,
}

impl<'a> UpdateTaskRequest<'a> {
    fn completed() -> Self {
        Self {
            name: None,
            notes: None,
            completed: Some(true),
        }
    }
}

impl<'a> From<super::PmTextUpdate<'a>> for UpdateTaskRequest<'a> {
    fn from(update: super::PmTextUpdate<'a>) -> Self {
        Self {
            name: update.name,
            notes: update.description,
            completed: None,
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
}

#[derive(Deserialize)]
struct AsanaGid {
    gid: String,
}

impl AsanaTask {
    fn into_pm_item(self, rank: u32) -> PmItem {
        PmItem {
            id: self.gid,
            name: self.name,
            description: self.notes,
            rank,
            completed: self.completed,
        }
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

fn retry_after_delay(headers: &reqwest::header::HeaderMap) -> Duration {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(ASANA_RETRY_AFTER_FALLBACK)
}

fn task_path(item_id: &str) -> String {
    format!("/tasks/{item_id}")
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;

    use axum::body::Bytes;
    use axum::extract::State;
    use axum::http::{HeaderMap, Method, StatusCode, Uri};
    use axum::routing::any;
    use axum::{response::Response, Router};
    use serde_json::json;
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    use super::*;
    use crate::engine::config::AsanaConfig;
    use crate::lfd::pm::PmProvider;

    #[tokio::test]
    async fn create_project_uses_workspace_and_team() {
        let (base_url, requests) = spawn_test_server(vec![json_response(
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
        let (base_url, requests) = spawn_test_server(vec![
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
        let (base_url, requests) = spawn_test_server(vec![
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
            spawn_test_server(vec![json_response(StatusCode::OK, json!({ "data": [] }))]).await;
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
    async fn list_items_collects_all_pages_and_assigns_rank_by_response_order() {
        let (base_url, requests) = spawn_test_server(vec![
            json_response(
                StatusCode::OK,
                json!({
                    "data": [
                        { "gid": "task-1", "name": "First", "notes": "one", "completed": false },
                        { "gid": "task-2", "name": "Second", "notes": "two", "completed": true }
                    ],
                    "next_page": { "offset": "cursor-2" }
                }),
            ),
            json_response(
                StatusCode::OK,
                json!({
                    "data": [
                        { "gid": "task-3", "name": "Third", "notes": "three", "completed": false }
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
                    id: "task-1".to_string(),
                    name: "First".to_string(),
                    description: "one".to_string(),
                    rank: 0,
                    completed: false,
                },
                PmItem {
                    id: "task-2".to_string(),
                    name: "Second".to_string(),
                    description: "two".to_string(),
                    rank: 1,
                    completed: true,
                },
                PmItem {
                    id: "task-3".to_string(),
                    name: "Third".to_string(),
                    description: "three".to_string(),
                    rank: 2,
                    completed: false,
                },
            ]
        );
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].path, "/projects/project-123/tasks");
        assert_eq!(
            requests[0].query.as_deref(),
            Some("opt_fields=name%2Cnotes%2Ccompleted")
        );
        assert_eq!(
            requests[1].query.as_deref(),
            Some("opt_fields=name%2Cnotes%2Ccompleted&offset=cursor-2")
        );
    }

    #[tokio::test]
    async fn create_update_complete_and_comment_map_to_asana_endpoints() {
        let (base_url, requests) = spawn_test_server(vec![
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
                    rank: 7,
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
                    rank: Some(0),
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
        assert_eq!(requests.len(), 4);

        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].path, "/tasks");
        assert_eq!(
            serde_json::from_str::<Value>(&requests[0].body).expect("json body"),
            json!({
                "data": {
                    "name": "Implement client",
                    "notes": "Build the HTTP adapter",
                    "projects": ["project-123"]
                }
            })
        );

        assert_eq!(requests[1].method, "PUT");
        assert_eq!(requests[1].path, "/tasks/task-123");
        assert_eq!(
            serde_json::from_str::<Value>(&requests[1].body).expect("json body"),
            json!({
                "data": {
                    "name": "Implement Asana client",
                    "notes": "Build the HTTP adapter and tests"
                }
            })
        );

        assert_eq!(requests[2].method, "PUT");
        assert_eq!(requests[2].path, "/tasks/task-123");
        assert_eq!(
            serde_json::from_str::<Value>(&requests[2].body).expect("json body"),
            json!({ "data": { "completed": true } })
        );

        assert_eq!(requests[3].method, "POST");
        assert_eq!(requests[3].path, "/tasks/task-123/stories");
        assert_eq!(
            serde_json::from_str::<Value>(&requests[3].body).expect("json body"),
            json!({ "data": { "text": "Shipped in v0.9.9" } })
        );
    }

    #[tokio::test]
    async fn update_item_skips_rank_only_updates() {
        let (base_url, requests) = spawn_test_server(Vec::new()).await;
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
                    rank: Some(1),
                },
            )
            .await
            .expect("rank-only update should no-op");

        assert!(requests.lock().await.is_empty());
    }

    #[tokio::test]
    async fn retries_after_rate_limit_response() {
        let (base_url, requests) = spawn_test_server(vec![
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
                    rank: 0,
                },
            )
            .await
            .expect("request should succeed after retry");

        assert_eq!(item_id, "task-123");
        assert_eq!(requests.lock().await.len(), 2);
    }

    #[tokio::test]
    async fn surfaces_asana_error_messages() {
        let (base_url, _requests) = spawn_test_server(vec![json_response(
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

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CapturedRequest {
        method: String,
        path: String,
        query: Option<String>,
        authorization: Option<String>,
        body: String,
    }

    #[derive(Debug, Clone)]
    struct QueuedResponse {
        status: StatusCode,
        headers: Vec<(String, String)>,
        body: String,
    }

    #[derive(Clone)]
    struct TestServerState {
        requests: Arc<Mutex<Vec<CapturedRequest>>>,
        responses: Arc<Mutex<VecDeque<QueuedResponse>>>,
    }

    async fn spawn_test_server(
        responses: Vec<QueuedResponse>,
    ) -> (String, Arc<Mutex<Vec<CapturedRequest>>>) {
        let state = TestServerState {
            requests: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
        };
        let requests = state.requests.clone();
        let app = Router::new()
            .route("/", any(handle_request))
            .route("/{*path}", any(handle_request))
            .with_state(state);
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test app");
        });
        (format!("http://{addr}"), requests)
    }

    async fn handle_request(
        State(state): State<TestServerState>,
        method: Method,
        uri: Uri,
        headers: HeaderMap,
        body: Bytes,
    ) -> Response {
        state.requests.lock().await.push(CapturedRequest {
            method: method.to_string(),
            path: uri.path().to_string(),
            query: uri.query().map(str::to_string),
            authorization: headers
                .get(reqwest::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
            body: String::from_utf8(body.to_vec()).expect("utf8 body"),
        });

        let response = state.responses.lock().await.pop_front().unwrap_or_else(|| {
            json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "errors": [{ "message": "unexpected request" }] }),
            )
        });

        let mut builder = Response::builder().status(response.status);
        for (name, value) in response.headers {
            builder = builder.header(name, value);
        }
        builder.body(response.body.into()).expect("build response")
    }

    fn json_response(status: StatusCode, body: Value) -> QueuedResponse {
        response(
            status,
            vec![("content-type", "application/json")],
            body.to_string(),
        )
    }

    fn response(status: StatusCode, headers: Vec<(&str, &str)>, body: String) -> QueuedResponse {
        QueuedResponse {
            status,
            headers: headers
                .into_iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
            body,
        }
    }
}
