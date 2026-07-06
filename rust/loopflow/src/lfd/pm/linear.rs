use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::sleep;
use tracing::warn;

use crate::lfd::pm::{
    PmError, PmItem, PmItemCreate, PmItemUpdate, PmProject, PmResult, RATE_LIMIT_RETRIES,
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

const CREATE_TEAM_MUTATION: &str = r#"mutation CreateTeam($name: String!, $key: String!) {
  teamCreate(input: { name: $name, key: $key }) {
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
        prioritySortOrder
        sortOrder
        assignee {
          id
        }
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

const CREATE_ITEM_MUTATION: &str = r#"mutation CreateIssue($teamId: String!, $projectId: String!, $title: String!, $description: String!, $stateId: String) {
  issueCreate(input: { teamId: $teamId, projectId: $projectId, title: $title, description: $description, stateId: $stateId }) {
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

const LIST_COMPLETED_WORKFLOW_STATES_QUERY: &str = r#"query CompletedWorkflowStates($teamId: ID!) {
  workflowStates(filter: { team: { id: { eq: $teamId } }, type: { eq: "completed" } }) {
    nodes {
      id
    }
  }
}"#;

const LIST_UNSTARTED_WORKFLOW_STATES_QUERY: &str = r#"query UnstartedWorkflowStates($teamId: ID!) {
  workflowStates(filter: { team: { id: { eq: $teamId } }, type: { eq: "unstarted" } }) {
    nodes {
      id
      position
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
    pub(crate) fn with_base_url(token: String, team_id: Option<String>, base_url: String) -> Self {
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

        let response: TeamsData = self.graphql(LIST_TEAMS_QUERY, json!({})).await?;
        if let Some(team) = response
            .teams
            .nodes
            .iter()
            .find(|team| team.name.eq_ignore_ascii_case(DEFAULT_LOOPFLOW_TEAM_NAME))
        {
            return Ok(team.id.clone());
        }

        let response: TeamCreateData = self
            .graphql(
                CREATE_TEAM_MUTATION,
                json!({
                    "name": DEFAULT_LOOPFLOW_TEAM_NAME,
                    "key": team_key_from_name(DEFAULT_LOOPFLOW_TEAM_NAME),
                }),
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
                .bearer_auth(&self.token)
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

    /// Resolve the team's default active state (`type == "unstarted"`, e.g. Todo),
    /// preferring the lowest-position state. Returns `None` when the team has no
    /// unstarted state so the caller can fall back to Linear's own default.
    async fn unstarted_state_id(&self, team_id: &str) -> PmResult<Option<String>> {
        let response: WorkflowStatesData = self
            .graphql(
                LIST_UNSTARTED_WORKFLOW_STATES_QUERY,
                json!({ "teamId": team_id }),
            )
            .await?;

        Ok(response
            .workflow_states
            .nodes
            .into_iter()
            .min_by(|left, right| left.position.total_cmp(&right.position))
            .map(|state| state.id))
    }

    pub async fn create_project(&self, name: &str, description: &str) -> PmResult<String> {
        let team_id = self.resolve_team_id().await?;
        let response: ProjectCreateData = self
            .graphql(
                CREATE_PROJECT_MUTATION,
                json!({
                    "name": name,
                    "description": linear_project_description(description),
                    "teamId": team_id,
                }),
            )
            .await?;

        Ok(response.project_create.project.id)
    }

    pub async fn list_projects(&self, team_id: &str) -> PmResult<Vec<PmProject>> {
        let response: TeamProjectsData = self
            .graphql(LIST_TEAM_PROJECTS_QUERY, json!({ "teamId": team_id }))
            .await?;
        Ok(response
            .team
            .projects
            .nodes
            .into_iter()
            .map(|project| PmProject {
                id: project.id,
                name: project.name,
            })
            .collect())
    }

    pub async fn list_items(&self, project_id: &str) -> PmResult<Vec<PmItem>> {
        let mut after = None;
        let mut issues = Vec::new();

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

            let page = response.project.issues;
            issues.extend(page.nodes);

            if !page.page_info.has_next_page {
                issues.sort_by(|left, right| {
                    left.priority_sort_order
                        .total_cmp(&right.priority_sort_order)
                        .then_with(|| left.sort_order.total_cmp(&right.sort_order))
                });
                return Ok(issues
                    .into_iter()
                    .enumerate()
                    .map(|(rank, issue)| issue.into_pm_item(rank as u32))
                    .collect());
            }

            after = page.page_info.end_cursor;
        }
    }

    pub async fn create_item(&self, project_id: &str, item: &PmItemCreate) -> PmResult<String> {
        let team_id = self.resolve_team_id().await?;
        let state_id = self.unstarted_state_id(&team_id).await?;
        let response: IssueCreateData = self
            .graphql(
                CREATE_ITEM_MUTATION,
                json!({
                    "teamId": team_id,
                    "projectId": project_id,
                    "title": item.name,
                    "description": item.description,
                    "stateId": state_id,
                }),
            )
            .await?;

        Ok(response.issue_create.issue.id)
    }

    pub async fn update_item(&self, item_id: &str, update: &PmItemUpdate) -> PmResult<()> {
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

    pub async fn complete_item(&self, item_id: &str) -> PmResult<()> {
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

    pub async fn comment(&self, item_id: &str, body: &str) -> PmResult<()> {
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
    #[serde(default)]
    extensions: Option<GraphqlErrorExtensions>,
}

impl GraphqlError {
    fn display_message(&self) -> &str {
        self.extensions
            .as_ref()
            .and_then(|extensions| extensions.user_presentable_message.as_deref())
            .filter(|message| !message.trim().is_empty())
            .unwrap_or(&self.message)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlErrorExtensions {
    #[serde(default)]
    user_presentable_message: Option<String>,
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
    #[serde(rename = "prioritySortOrder", default)]
    priority_sort_order: f64,
    #[serde(rename = "sortOrder", default)]
    sort_order: f64,
    #[serde(default)]
    assignee: Option<IdNode>,
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
            assignee: self.assignee.map(|assignee| assignee.id),
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
    #[serde(default)]
    position: f64,
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
            return Err(PmError::Message(error.display_message().to_string()));
        }
        return Err(PmError::Message(format!(
            "linear request failed with status {status}: {}",
            error.display_message()
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

fn team_key_from_name(name: &str) -> String {
    let key: String = name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .map(|ch| ch.to_ascii_uppercase())
        .collect();
    if key.is_empty() {
        "LF".to_string()
    } else {
        key[..key.len().min(5)].to_string()
    }
}

fn linear_project_description(description: &str) -> String {
    let summary = first_meaningful_paragraph(description);
    if summary.is_empty() {
        return String::new();
    }

    const MAX_DESCRIPTION_LEN: usize = 255;
    summary.chars().take(MAX_DESCRIPTION_LEN).collect()
}

fn first_meaningful_paragraph(description: &str) -> String {
    let mut lines = description.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let mut paragraph = vec![trimmed];
        while let Some(next_line) = lines.peek() {
            let trimmed = next_line.trim();
            if trimmed.is_empty() {
                break;
            }
            if trimmed.starts_with('#') {
                lines.next();
                break;
            }
            paragraph.push(trimmed);
            lines.next();
        }

        return paragraph.join(" ");
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::pm::test_server::{self, json_response};
    use axum::http::StatusCode;
    use serde_json::json;

    #[tokio::test]
    async fn list_items_maps_linear_project_issues() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
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
                                    "prioritySortOrder": 10.0,
                                    "sortOrder": 10.0,
                                    "assignee": { "id": "user-1" },
                                    "state": { "type": "unstarted" }
                                },
                                {
                                    "id": "issue-2",
                                    "title": "Second",
                                    "description": "two",
                                    "prioritySortOrder": 0.0,
                                    "sortOrder": 0.0,
                                    "state": { "type": "completed" }
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
        )])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-9".to_string()),
            base_url,
        );

        let items = client
            .list_items("project-123")
            .await
            .expect("list items succeeds");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "issue-2");
        assert!(items[0].completed);
        assert_eq!(items[1].assignee.as_deref(), Some("user-1"));
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].authorization.as_deref(),
            Some("Bearer linear-secret")
        );
    }

    #[tokio::test]
    async fn create_update_and_comment_map_to_linear_mutations() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({
                    "data": {
                        "workflowStates": {
                            "nodes": [
                                { "id": "state-in-progress", "position": 2.0 },
                                { "id": "state-todo", "position": 1.0 }
                            ]
                        }
                    }
                }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueCreate": { "issue": { "id": "issue-123" } } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueUpdate": { "issue": { "id": "issue-123" } } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "commentCreate": { "comment": { "id": "comment-1" } } } }),
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
            .expect("create item succeeds");
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
            .expect("update item succeeds");
        client
            .comment(&item_id, "Shipped in v0.9.9")
            .await
            .expect("comment succeeds");

        assert_eq!(item_id, "issue-123");
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 4);

        // create_item first resolves the team's active (unstarted) state, then
        // sends that lowest-position state id as stateId on issueCreate so new
        // issues land in Todo rather than the hidden Backlog.
        let states_body: Value =
            serde_json::from_str(&requests[0].body).expect("states body is json");
        assert!(states_body["query"]
            .as_str()
            .expect("query present")
            .contains("UnstartedWorkflowStates"));

        let create_body: Value =
            serde_json::from_str(&requests[1].body).expect("create body is json");
        assert_eq!(create_body["variables"]["stateId"], json!("state-todo"));
    }

    #[tokio::test]
    async fn create_item_omits_state_when_team_has_no_unstarted_state() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({ "data": { "workflowStates": { "nodes": [] } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueCreate": { "issue": { "id": "issue-123" } } } }),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-9".to_string()),
            base_url,
        );

        client
            .create_item(
                "project-123",
                &PmItemCreate {
                    name: "Implement client".to_string(),
                    description: "Build the GraphQL adapter".to_string(),
                    rank: 7,
                },
            )
            .await
            .expect("create item succeeds");

        let requests = requests.lock().await;
        let create_body: Value =
            serde_json::from_str(&requests[1].body).expect("create body is json");
        assert_eq!(create_body["variables"]["stateId"], Value::Null);
    }

    #[test]
    fn linear_project_description_skips_headings_and_truncates() {
        let summary = linear_project_description(
            "## Vision\n\nThis is the first paragraph.\n\n## Strategy\n\nSecond paragraph.",
        );
        assert_eq!(summary, "This is the first paragraph.");

        let long = "a".repeat(300);
        assert_eq!(linear_project_description(&long).len(), 255);
        assert_eq!(linear_project_description(""), "");
    }
}
