use async_trait::async_trait;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::sleep;
use tracing::warn;

use crate::lfd::pm::{
    PmError, PmItem, PmItemCreate, PmItemUpdate, PmProject, PmProvider, PmResult,
    RATE_LIMIT_RETRIES,
};

const LINEAR_BASE_URL: &str = "https://api.linear.app/graphql";
const LIST_ITEMS_PAGE_SIZE: u32 = 50;
const COMPLETED_STATE_TYPE: &str = "completed";
const DEFAULT_LOOPFLOW_TEAM_NAME: &str = "Loopflow";

const LIST_TEAMS_QUERY: &str = r#"query ListTeams {
  teams {
    nodes {
      id
      name
    }
  }
}"#;

const CREATE_TEAM_MUTATION: &str = r#"mutation CreateTeam($name: String!) {
  teamCreate(input: { name: $name }) {
    team {
      id
    }
  }
}"#;

const CREATE_PROJECT_MUTATION: &str = r#"mutation CreateProject($name: String!, $description: String!, $teamId: String!) {
  projectCreate(input: { name: $name, description: $description, teamIds: [$teamId] }) {
    project {
      id
    }
  }
}"#;

const LIST_ITEMS_QUERY: &str = r#"query ListProjectIssues($projectId: String!, $after: String, $first: Int!) {
  project(id: $projectId) {
    issues(first: $first, after: $after) {
      nodes {
        id
        title
        description
        state {
          type
        }
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
}"#;

const CREATE_ITEM_MUTATION: &str = r#"mutation CreateIssue($teamId: String!, $projectId: String!, $title: String!, $description: String!) {
  issueCreate(input: { teamId: $teamId, projectId: $projectId, title: $title, description: $description }) {
    issue {
      id
    }
  }
}"#;

const UPDATE_ITEM_MUTATION: &str = r#"mutation UpdateIssue($id: String!, $title: String, $description: String) {
  issueUpdate(id: $id, input: { title: $title, description: $description }) {
    issue {
      id
    }
  }
}"#;

const COMPLETE_ITEM_MUTATION: &str = r#"mutation CompleteIssue($id: String!, $stateId: String!) {
  issueUpdate(id: $id, input: { stateId: $stateId }) {
    issue {
      id
    }
  }
}"#;

const LIST_COMPLETED_WORKFLOW_STATES_QUERY: &str = r#"query CompletedWorkflowStates($teamId: String!) {
  workflowStates(filter: { team: { id: { eq: $teamId } }, type: { eq: "completed" } }) {
    nodes {
      id
      name
      type
    }
  }
}"#;

const LIST_TEAM_PROJECTS_QUERY: &str = r#"query ListTeamProjects($teamId: String!) {
  team(id: $teamId) {
    projects {
      nodes {
        id
        name
      }
    }
  }
}"#;

const CREATE_COMMENT_MUTATION: &str = r#"mutation CreateComment($issueId: String!, $body: String!) {
  commentCreate(input: { issueId: $issueId, body: $body }) {
    comment {
      id
    }
  }
}"#;

#[derive(Debug, Clone)]
pub struct LinearClient {
    client: reqwest::Client,
    token: String,
    team_id: Option<String>,
    base_url: String,
}

impl LinearClient {
    pub fn new(token: String, team_id: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
            team_id,
            base_url: LINEAR_BASE_URL.to_string(),
        }
    }

    #[cfg(test)]
    fn with_base_url(token: String, team_id: Option<String>, base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
            team_id,
            base_url,
        }
    }

    async fn resolve_team_id(&self) -> PmResult<String> {
        if let Some(team_id) = &self.team_id {
            return Ok(team_id.clone());
        }

        // Look for a team named "Loopflow", create if not found
        let response: TeamsData = self.graphql(LIST_TEAMS_QUERY, json!({})).await?;
        if let Some(team) = response
            .teams
            .nodes
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(DEFAULT_LOOPFLOW_TEAM_NAME))
        {
            return Ok(team.id.clone());
        }

        // Create the team
        let response: TeamCreateData = self
            .graphql(
                CREATE_TEAM_MUTATION,
                json!({ "name": DEFAULT_LOOPFLOW_TEAM_NAME }),
            )
            .await?;
        Ok(response.team_create.team.id)
    }

    async fn graphql<T>(&self, query: &str, variables: Value) -> PmResult<T>
    where
        T: DeserializeOwned,
    {
        let request = GraphqlRequest { query, variables };

        for attempt in 0..=RATE_LIMIT_RETRIES {
            let response = self
                .client
                .post(&self.base_url)
                .header(
                    reqwest::header::AUTHORIZATION,
                    format!("Bearer {}", self.token),
                )
                .json(&request)
                .send()
                .await
                .map_err(|err| PmError::Message(format!("linear request failed: {err}")))?;

            if response.status() == StatusCode::TOO_MANY_REQUESTS && attempt < RATE_LIMIT_RETRIES {
                let delay = super::retry_after_delay(response.headers());
                warn!(
                    attempt = attempt + 1,
                    delay_seconds = delay.as_secs(),
                    "linear rate limited; retrying"
                );
                sleep(delay).await;
                continue;
            }

            return parse_graphql_response(response).await;
        }

        Err(PmError::Message(
            "linear request failed after retries".to_string(),
        ))
    }

    async fn completed_state_id(&self) -> PmResult<String> {
        let team_id = self.resolve_team_id().await?;
        let response: WorkflowStatesData = self
            .graphql(
                LIST_COMPLETED_WORKFLOW_STATES_QUERY,
                json!({ "teamId": team_id }),
            )
            .await?;

        response
            .workflow_states
            .nodes
            .into_iter()
            .next()
            .map(|state| state.id)
            .ok_or_else(|| {
                PmError::Message(format!(
                    "no completed Linear workflow state found for team {team_id}"
                ))
            })
    }
}

#[async_trait]
impl PmProvider for LinearClient {
    async fn create_project(&self, name: &str, description: &str) -> PmResult<String> {
        let team_id = self.resolve_team_id().await?;
        let response: ProjectCreateData = self
            .graphql(
                CREATE_PROJECT_MUTATION,
                json!({
                    "name": name,
                    "description": description,
                    "teamId": team_id,
                }),
            )
            .await?;

        Ok(response.project_create.project.id)
    }

    async fn list_projects(&self, team_id: &str) -> PmResult<Vec<PmProject>> {
        let response: TeamProjectsData = self
            .graphql(LIST_TEAM_PROJECTS_QUERY, json!({ "teamId": team_id }))
            .await?;
        Ok(response
            .team
            .projects
            .nodes
            .into_iter()
            .map(|p| PmProject {
                id: p.id,
                name: p.name,
            })
            .collect())
    }

    async fn list_items(&self, project_id: &str) -> PmResult<Vec<PmItem>> {
        let mut after = None;
        let mut items = Vec::new();

        loop {
            let response: ProjectIssuesData = self
                .graphql(
                    LIST_ITEMS_QUERY,
                    json!({
                        "projectId": project_id,
                        "after": after,
                        "first": LIST_ITEMS_PAGE_SIZE,
                    }),
                )
                .await?;

            let issues = response.project.issues;
            for issue in issues.nodes {
                items.push(issue.into_pm_item(items.len() as u32));
            }

            if !issues.page_info.has_next_page {
                return Ok(items);
            }

            after = issues.page_info.end_cursor;
        }
    }

    async fn create_item(&self, project_id: &str, item: &PmItemCreate) -> PmResult<String> {
        let team_id = self.resolve_team_id().await?;
        let response: IssueCreateData = self
            .graphql(
                CREATE_ITEM_MUTATION,
                json!({
                    "teamId": team_id,
                    "projectId": project_id,
                    "title": item.name,
                    "description": item.description,
                }),
            )
            .await?;

        Ok(response.issue_create.issue.id)
    }

    async fn update_item(&self, item_id: &str, update: &PmItemUpdate) -> PmResult<()> {
        let Some(update) = update.text_update() else {
            return Ok(());
        };

        let _: Value = self
            .graphql(
                UPDATE_ITEM_MUTATION,
                json!({
                    "id": item_id,
                    "title": update.name,
                    "description": update.description,
                }),
            )
            .await?;
        Ok(())
    }

    async fn complete_item(&self, item_id: &str) -> PmResult<()> {
        let state_id = self.completed_state_id().await?;
        let _: Value = self
            .graphql(
                COMPLETE_ITEM_MUTATION,
                json!({
                    "id": item_id,
                    "stateId": state_id,
                }),
            )
            .await?;
        Ok(())
    }

    async fn comment(&self, item_id: &str, body: &str) -> PmResult<()> {
        let _: Value = self
            .graphql(
                CREATE_COMMENT_MUTATION,
                json!({
                    "issueId": item_id,
                    "body": body,
                }),
            )
            .await?;
        Ok(())
    }
}

#[derive(Serialize)]
struct GraphqlRequest<'a> {
    query: &'a str,
    variables: Value,
}

#[derive(Deserialize)]
struct GraphqlResponse {
    #[serde(default)]
    data: Option<Value>,
    #[serde(default)]
    errors: Vec<GraphqlError>,
}

#[derive(Debug, Deserialize)]
struct GraphqlError {
    message: String,
}

#[derive(Deserialize)]
struct ProjectCreateData {
    #[serde(rename = "projectCreate")]
    project_create: ProjectPayload,
}

#[derive(Deserialize)]
struct IssueCreateData {
    #[serde(rename = "issueCreate")]
    issue_create: IssuePayload,
}

#[derive(Deserialize)]
struct ProjectPayload {
    project: IdNode,
}

#[derive(Deserialize)]
struct IssuePayload {
    issue: IdNode,
}

#[derive(Deserialize)]
struct IdNode {
    id: String,
}

#[derive(Deserialize)]
struct IssueNode {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    state: Option<WorkflowStateRef>,
}

impl IssueNode {
    fn into_pm_item(self, rank: u32) -> PmItem {
        let completed = self
            .state
            .as_ref()
            .is_some_and(|state| state.r#type.eq_ignore_ascii_case(COMPLETED_STATE_TYPE));

        PmItem {
            id: self.id,
            name: self.title,
            description: self.description.unwrap_or_default(),
            rank,
            completed,
        }
    }
}

#[derive(Deserialize)]
struct WorkflowStateRef {
    #[serde(rename = "type")]
    r#type: String,
}

#[derive(Deserialize)]
struct ProjectIssuesData {
    project: ProjectWithIssues,
}

#[derive(Deserialize)]
struct ProjectWithIssues {
    issues: IssuesConnection,
}

#[derive(Deserialize)]
struct IssuesConnection {
    nodes: Vec<IssueNode>,
    #[serde(rename = "pageInfo")]
    page_info: PageInfo,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Deserialize)]
struct WorkflowStatesData {
    #[serde(rename = "workflowStates")]
    workflow_states: WorkflowStatesConnection,
}

#[derive(Deserialize)]
struct WorkflowStatesConnection {
    nodes: Vec<WorkflowStateNode>,
}

#[derive(Deserialize)]
struct WorkflowStateNode {
    id: String,
}

#[derive(Deserialize)]
struct TeamsData {
    teams: TeamsConnection,
}

#[derive(Deserialize)]
struct TeamsConnection {
    nodes: Vec<TeamNode>,
}

#[derive(Deserialize)]
struct TeamNode {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct TeamCreateData {
    #[serde(rename = "teamCreate")]
    team_create: TeamCreatePayload,
}

#[derive(Deserialize)]
struct TeamCreatePayload {
    team: IdNode,
}

#[derive(Deserialize)]
struct TeamProjectsData {
    team: TeamWithProjects,
}

#[derive(Deserialize)]
struct TeamWithProjects {
    projects: ProjectsConnection,
}

#[derive(Deserialize)]
struct ProjectsConnection {
    nodes: Vec<ProjectNode>,
}

#[derive(Deserialize)]
struct ProjectNode {
    id: String,
    name: String,
}

async fn parse_graphql_response<T: DeserializeOwned>(response: reqwest::Response) -> PmResult<T> {
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|err| PmError::Message(format!("failed to read Linear response: {err}")))?;

    let parsed = serde_json::from_slice::<GraphqlResponse>(&body)
        .map_err(|err| PmError::Message(format!("failed to decode Linear response: {err}")))?;

    if let Some(error) = parsed.errors.first() {
        if status.is_success() {
            return Err(PmError::Message(error.message.clone()));
        }
        return Err(PmError::Message(format!(
            "linear request failed with status {status}: {}",
            error.message
        )));
    }

    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&body).trim().to_string();
        if body_text.is_empty() {
            return Err(PmError::Message(format!(
                "linear request failed with status {status}"
            )));
        }
        return Err(PmError::Message(format!(
            "linear request failed with status {status}: {body_text}"
        )));
    }

    let data = parsed
        .data
        .ok_or_else(|| PmError::Message("linear response missing data".to_string()))?;

    serde_json::from_value(data)
        .map_err(|err| PmError::Message(format!("failed to decode Linear response: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::pm::test_server::{self, json_response, response};
    use axum::http::StatusCode;
    use serde_json::json;

    #[tokio::test]
    async fn create_project_sends_team_id_and_bare_api_key_header() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({
                "data": {
                    "projectCreate": {
                        "project": { "id": "project-123" }
                    }
                }
            }),
        )])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-9".to_string()),
            base_url,
        );

        let project_id = client
            .create_project("Wave PM", "Ship the Linear client")
            .await
            .expect("create project should succeed");

        assert_eq!(project_id, "project-123");
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].path, "/");
        assert_eq!(
            requests[0].authorization.as_deref(),
            Some("Bearer linear-secret")
        );
        assert_eq!(
            serde_json::from_str::<Value>(&requests[0].body).expect("json body"),
            json!({
                "query": CREATE_PROJECT_MUTATION,
                "variables": {
                    "name": "Wave PM",
                    "description": "Ship the Linear client",
                    "teamId": "team-9"
                }
            })
        );
    }

    #[tokio::test]
    async fn list_items_paginates_and_assigns_rank_by_response_order() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({
                    "data": {
                        "project": {
                            "issues": {
                                "nodes": [
                                    {
                                        "id": "issue-1",
                                        "title": "First",
                                        "description": "one",
                                        "state": { "type": "backlog" }
                                    },
                                    {
                                        "id": "issue-2",
                                        "title": "Second",
                                        "description": "two",
                                        "state": { "type": "completed" }
                                    }
                                ],
                                "pageInfo": {
                                    "hasNextPage": true,
                                    "endCursor": "cursor-2"
                                }
                            }
                        }
                    }
                }),
            ),
            json_response(
                StatusCode::OK,
                json!({
                    "data": {
                        "project": {
                            "issues": {
                                "nodes": [
                                    {
                                        "id": "issue-3",
                                        "title": "Third",
                                        "description": null,
                                        "state": { "type": "unstarted" }
                                    }
                                ],
                                "pageInfo": {
                                    "hasNextPage": false,
                                    "endCursor": null
                                }
                            }
                        }
                    }
                }),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-9".to_string()),
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
                    id: "issue-1".to_string(),
                    name: "First".to_string(),
                    description: "one".to_string(),
                    rank: 0,
                    completed: false,
                },
                PmItem {
                    id: "issue-2".to_string(),
                    name: "Second".to_string(),
                    description: "two".to_string(),
                    rank: 1,
                    completed: true,
                },
                PmItem {
                    id: "issue-3".to_string(),
                    name: "Third".to_string(),
                    description: "".to_string(),
                    rank: 2,
                    completed: false,
                },
            ]
        );

        let requests = requests.lock().await;
        assert_eq!(requests.len(), 2);
        assert_eq!(
            serde_json::from_str::<Value>(&requests[0].body).expect("json body"),
            json!({
                "query": LIST_ITEMS_QUERY,
                "variables": {
                    "projectId": "project-123",
                    "after": null,
                    "first": LIST_ITEMS_PAGE_SIZE,
                }
            })
        );
        assert_eq!(
            serde_json::from_str::<Value>(&requests[1].body).expect("json body"),
            json!({
                "query": LIST_ITEMS_QUERY,
                "variables": {
                    "projectId": "project-123",
                    "after": "cursor-2",
                    "first": LIST_ITEMS_PAGE_SIZE,
                }
            })
        );
    }

    #[tokio::test]
    async fn create_update_and_comment_map_to_linear_mutations() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({
                    "data": {
                        "issueCreate": {
                            "issue": { "id": "issue-123" }
                        }
                    }
                }),
            ),
            json_response(
                StatusCode::OK,
                json!({
                    "data": {
                        "issueUpdate": {
                            "issue": { "id": "issue-123" }
                        }
                    }
                }),
            ),
            json_response(
                StatusCode::OK,
                json!({
                    "data": {
                        "commentCreate": {
                            "comment": { "id": "comment-1" }
                        }
                    }
                }),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-9".to_string()),
            base_url,
        );

        let item_id = client
            .create_item(
                "project-123",
                &PmItemCreate {
                    name: "Implement client".to_string(),
                    description: "Build the GraphQL adapter".to_string(),
                    rank: 7,
                },
            )
            .await
            .expect("create item should succeed");
        client
            .update_item(
                &item_id,
                &PmItemUpdate {
                    name: Some("Implement Linear client".to_string()),
                    description: Some("Build the GraphQL adapter and tests".to_string()),
                    rank: Some(0),
                },
            )
            .await
            .expect("update item should succeed");
        client
            .comment(&item_id, "Shipped in v0.9.9")
            .await
            .expect("comment should succeed");

        assert_eq!(item_id, "issue-123");
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 3);
        assert_eq!(
            serde_json::from_str::<Value>(&requests[0].body).expect("json body"),
            json!({
                "query": CREATE_ITEM_MUTATION,
                "variables": {
                    "teamId": "team-9",
                    "projectId": "project-123",
                    "title": "Implement client",
                    "description": "Build the GraphQL adapter"
                }
            })
        );
        assert_eq!(
            serde_json::from_str::<Value>(&requests[1].body).expect("json body"),
            json!({
                "query": UPDATE_ITEM_MUTATION,
                "variables": {
                    "id": "issue-123",
                    "title": "Implement Linear client",
                    "description": "Build the GraphQL adapter and tests"
                }
            })
        );
        assert_eq!(
            serde_json::from_str::<Value>(&requests[2].body).expect("json body"),
            json!({
                "query": CREATE_COMMENT_MUTATION,
                "variables": {
                    "issueId": "issue-123",
                    "body": "Shipped in v0.9.9"
                }
            })
        );
    }

    #[tokio::test]
    async fn complete_item_queries_completed_workflow_state_before_update() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({
                    "data": {
                        "workflowStates": {
                            "nodes": [
                                {
                                    "id": "state-complete",
                                    "name": "Done",
                                    "type": "completed"
                                }
                            ]
                        }
                    }
                }),
            ),
            json_response(
                StatusCode::OK,
                json!({
                    "data": {
                        "issueUpdate": {
                            "issue": { "id": "issue-123" }
                        }
                    }
                }),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-9".to_string()),
            base_url,
        );

        client
            .complete_item("issue-123")
            .await
            .expect("complete item should succeed");

        let requests = requests.lock().await;
        assert_eq!(requests.len(), 2);
        assert_eq!(
            serde_json::from_str::<Value>(&requests[0].body).expect("json body"),
            json!({
                "query": LIST_COMPLETED_WORKFLOW_STATES_QUERY,
                "variables": {
                    "teamId": "team-9"
                }
            })
        );
        assert_eq!(
            serde_json::from_str::<Value>(&requests[1].body).expect("json body"),
            json!({
                "query": COMPLETE_ITEM_MUTATION,
                "variables": {
                    "id": "issue-123",
                    "stateId": "state-complete"
                }
            })
        );
    }

    #[tokio::test]
    async fn complete_item_fails_when_team_has_no_completed_state() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({
                "data": {
                    "workflowStates": {
                        "nodes": []
                    }
                }
            }),
        )])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-9".to_string()),
            base_url,
        );

        let error = client
            .complete_item("issue-123")
            .await
            .expect_err("completion should fail without a completed state");

        assert_eq!(
            error,
            PmError::Message(
                "no completed Linear workflow state found for team team-9".to_string()
            )
        );
        assert_eq!(requests.lock().await.len(), 1);
    }

    #[tokio::test]
    async fn update_item_skips_rank_only_updates() {
        let (base_url, requests) = test_server::spawn(Vec::new()).await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-9".to_string()),
            base_url,
        );

        client
            .update_item(
                "issue-123",
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
        let (base_url, requests) = test_server::spawn(vec![
            response(
                StatusCode::TOO_MANY_REQUESTS,
                vec![("retry-after", "0")],
                json!({ "errors": [{ "message": "slow down" }] }).to_string(),
            ),
            json_response(
                StatusCode::OK,
                json!({
                    "data": {
                        "issueCreate": {
                            "issue": { "id": "issue-123" }
                        }
                    }
                }),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-9".to_string()),
            base_url,
        );

        let item_id = client
            .create_item(
                "project-123",
                &PmItemCreate {
                    name: "Implement client".to_string(),
                    description: "Build the GraphQL adapter".to_string(),
                    rank: 0,
                },
            )
            .await
            .expect("request should succeed after retry");

        assert_eq!(item_id, "issue-123");
        assert_eq!(requests.lock().await.len(), 2);
    }

    #[tokio::test]
    async fn graphql_errors_override_data() {
        let (base_url, _requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({
                "data": {
                    "issueCreate": {
                        "issue": { "id": "issue-123" }
                    }
                },
                "errors": [{ "message": "project is required" }]
            }),
        )])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-9".to_string()),
            base_url,
        );

        let error = client
            .create_item(
                "project-123",
                &PmItemCreate {
                    name: "Implement client".to_string(),
                    description: "Build the GraphQL adapter".to_string(),
                    rank: 0,
                },
            )
            .await
            .expect_err("graphql errors should win over data");

        assert_eq!(error, PmError::Message("project is required".to_string()));
    }

    #[test]
    fn parse_graphql_response_surfaces_http_error_messages() {
        let message = parse_linear_error_message(
            StatusCode::BAD_REQUEST,
            &json!({ "errors": [{ "message": "team is required" }] }).to_string(),
        );

        assert_eq!(
            message,
            "linear request failed with status 400 Bad Request: team is required"
        );
    }

    fn parse_linear_error_message(status: StatusCode, body: &str) -> String {
        let response: GraphqlResponse =
            serde_json::from_str(body).expect("graphql response should parse");
        if let Some(error) = response.errors.first() {
            return format!(
                "linear request failed with status {status}: {}",
                error.message
            );
        }
        panic!("expected graphql error message")
    }
}
