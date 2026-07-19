use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::sleep;
use tracing::warn;

#[cfg(test)]
use crate::pm::PmKr;
use crate::pm::{
    parse_project_content, project_slug, render_project_content, IssueComment, IssueObservation,
    PmError, PmItem, PmItemCreate, PmItemUpdate, PmProject, PmResult, PmWave, ProjectContent,
    TeamBinding, RATE_LIMIT_RETRIES,
};

const LINEAR_BASE_URL: &str = "https://api.linear.app/graphql";
const LIST_ITEMS_PAGE_SIZE: u32 = 50;
const LIST_PROJECTS_PAGE_SIZE: u32 = 50;
const COMPLETED_STATE_TYPE: &str = "completed";
const DEFAULT_LOOPFLOW_TEAM_NAME: &str = "Loopflow";

const LIST_TEAMS_QUERY: &str = r#"query ListTeams {
  teams {
    nodes {
      id
      name
      key
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

const CREATE_INITIATIVE_MUTATION: &str = r#"mutation CreateInitiative($name: String!, $description: String!) {
  initiativeCreate(input: { name: $name, description: $description }) {
    initiative {
      id
    }
  }
}"#;

const UPDATE_INITIATIVE_MUTATION: &str = r#"mutation UpdateInitiative($id: String!, $name: String!) {
  initiativeUpdate(id: $id, input: { name: $name }) {
    initiative {
      id
    }
  }
}"#;

const LIST_INITIATIVES_QUERY: &str = r#"query ListInitiatives($after: String, $first: Int!) {
  initiatives(after: $after, first: $first) {
    nodes {
      id
      name
      description
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}"#;

const LIST_INITIATIVE_PROJECTS_QUERY: &str = r#"query ListInitiativeProjects($initiativeId: String!, $after: String, $first: Int!) {
  initiative(id: $initiativeId) {
    projects(after: $after, first: $first, includeSubInitiatives: false) {
      nodes {
        id
        name
        description
        content
        initiatives(first: 50) {
          nodes {
            id
          }
        }
        teams(first: 50) {
          nodes {
            id
          }
        }
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
}"#;

const CREATE_PROJECT_MUTATION: &str = r#"mutation CreateProject($name: String!, $description: String!, $content: String!, $teamId: String!) {
  projectCreate(input: { name: $name, description: $description, content: $content, teamIds: [$teamId] }) {
    project {
      id
    }
  }
}"#;

const UPDATE_PROJECT_MUTATION: &str = r#"mutation UpdateProject($id: String!, $name: String!, $description: String!, $content: String!) {
  projectUpdate(id: $id, input: { name: $name, description: $description, content: $content }) {
    project {
      id
    }
  }
}"#;

// Sets the Project's teams to exactly `[$teamId]` — a replacement, not an add —
// so a Project stranded on the shared team lands on the wave's team. Projects
// keep their id/slug across a team move (only issues renumber).
const MOVE_PROJECT_TO_TEAM_MUTATION: &str = r#"mutation MoveProjectToTeam($id: String!, $teamId: String!) {
  projectUpdate(id: $id, input: { teamIds: [$teamId] }) {
    project {
      id
    }
  }
}"#;

const ARCHIVE_PROJECT_MUTATION: &str = r#"mutation ArchiveProject($id: String!) {
  projectArchive(id: $id) {
    success
  }
}"#;

const ATTACH_PROJECT_MUTATION: &str = r#"mutation AttachProject($initiativeId: String!, $projectId: String!) {
  initiativeToProjectCreate(input: { initiativeId: $initiativeId, projectId: $projectId }) {
    initiativeToProject {
      id
    }
  }
}"#;

const LIST_ITEMS_QUERY: &str = r#"query ListProjectIssues($projectId: String!, $after: String, $first: Int!) {
  project(id: $projectId) {
    issues(first: $first, after: $after) {
      nodes {
        id
        identifier
        url
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

const UPDATE_ITEM_MUTATION: &str = r#"mutation UpdateIssue($id: String!, $input: IssueUpdateInput!) {
  issueUpdate(id: $id, input: $input) {
    issue {
      id
    }
  }
}"#;

const MOVE_ITEM_MUTATION: &str = r#"mutation MoveIssueToProject($id: String!, $projectId: String!) {
  issueUpdate(id: $id, input: { projectId: $projectId }) {
    issue {
      id
    }
  }
}"#;

const SET_ITEM_STATE_MUTATION: &str = r#"mutation SetIssueState($id: String!, $stateId: String!) {
  issueUpdate(id: $id, input: { stateId: $stateId }) {
    issue {
      id
    }
  }
}"#;

// Selects `identifier` back because Linear reassigns the issue number on a team
// move (`W2-155` → `PRD-<next>`); the caller cannot predict the new value.
const MOVE_ITEM_TO_TEAM_MUTATION: &str = r#"mutation MoveIssueToTeam($id: String!, $teamId: String!) {
  issueUpdate(id: $id, input: { teamId: $teamId }) {
    issue {
      id
      identifier
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

const CREATE_COMMENT_MUTATION: &str = r#"mutation CreateComment($issueId: String!, $body: String!) {
  commentCreate(input: { issueId: $issueId, body: $body }) {
    comment {
      id
    }
  }
}"#;

// Loopflow's own OAuth user. Its id lets the observer tell a human's edit or
// comment from Loopflow's own writeback, so ingestion never feeds itself.
const VIEWER_QUERY: &str = r#"query Viewer {
  viewer {
    id
  }
}"#;

// Register the webhook that streams issue/comment changes to a Loopflow receiver.
// `allPublicTeams: true` covers every team the token can see; the caller owns the
// signing secret (from Doppler) and the public URL.
const CREATE_WEBHOOK_MUTATION: &str = r#"mutation CreateWebhook($url: String!, $secret: String!, $resourceTypes: [String!]!) {
  webhookCreate(input: { url: $url, secret: $secret, resourceTypes: $resourceTypes, allPublicTeams: true }) {
    webhook {
      id
    }
  }
}"#;

const UPDATE_COMMENT_MUTATION: &str = r#"mutation UpdateComment($id: String!, $body: String!) {
  commentUpdate(id: $id, input: { body: $body }) {
    comment {
      id
    }
  }
}"#;

// `attachmentLinkURL` links a URL and dedupes on it — re-linking the same PR
// returns the existing attachment rather than duplicating. It accepts `issueId`,
// `url`, and `title`, but not `subtitle` (that lives on `attachmentUpdate`'s
// input). PR state is carried in the managed comment body and filled onto the
// attachment as a subtitle by the later `attachmentUpdate`.
const LINK_ATTACHMENT_MUTATION: &str = r#"mutation LinkAttachment($issueId: String!, $url: String!, $title: String!) {
  attachmentLinkURL(issueId: $issueId, url: $url, title: $title) {
    attachment {
      id
    }
  }
}"#;

// One issue's human-editable content plus a `createdAt`-ordered page of its
// comments. Each comment carries `user { id }` (the human author) but not
// `botActor`, so an integration-authored comment decodes to a null author and is
// never treated as human direction. `updatedAt` is the revision marker. The
// reconciler orders delivery itself and dedupes against the cursor, so page
// order only bounds how many comments one read can surface (OBSERVATION_COMMENT_PAGE).
const ISSUE_OBSERVATION_QUERY: &str = r#"query IssueObservation($id: String!, $comments: Int!) {
  issue(id: $id) {
    updatedAt
    title
    description
    comments(first: $comments, orderBy: createdAt) {
      nodes {
        id
        body
        user {
          id
        }
      }
    }
  }
}"#;

const ISSUE_COMMENTS_QUERY: &str = r#"query IssueComments($id: String!, $comments: Int!, $after: String) {
  issue(id: $id) {
    comments(first: $comments, after: $after, orderBy: createdAt) {
      nodes {
        id
        body
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
}"#;

// Reads the owning team of an existing issue so state transitions resolve a
// workflow state from the issue's team, not the wave-configured team. A Project
// can span teams (e.g. ENG-* and W2-*), and Linear rejects a state that belongs
// to a different team than the issue.
const ISSUE_TEAM_QUERY: &str = r#"query IssueTeam($id: String!) {
  issue(id: $id) {
    team {
      id
    }
  }
}"#;

const UPDATE_ATTACHMENT_MUTATION: &str = r#"mutation UpdateAttachment($id: String!, $title: String!, $subtitle: String!) {
  attachmentUpdate(id: $id, input: { title: $title, subtitle: $subtitle }) {
    attachment {
      id
    }
  }
}"#;

/// How many recent comments one observation reads. A Task accumulating more than
/// this many unseen human comments between polls is not a real case; the cursor
/// still refuses to double-deliver any it does see.
const OBSERVATION_COMMENT_PAGE: u32 = 50;

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

    /// Adopt or create the team a wave should own, keyed by `key`. Returns the
    /// stable team id. Diagnoses conflicts instead of guessing:
    /// - the requested key already belongs to a team with the same name → adopt;
    /// - the requested key belongs to a *different*-named team → refuse;
    /// - the name exists under a different key → refuse (name the existing key);
    /// - neither exists → create.
    pub async fn ensure_team(&self, name: &str, key: &str) -> PmResult<TeamBinding> {
        let requested_key = key.trim().to_ascii_uppercase();
        if requested_key.is_empty() {
            return Err(PmError::Message(
                "team key cannot be empty; pass --team-key <KEY>".to_string(),
            ));
        }

        let response: TeamsData = self.graphql(LIST_TEAMS_QUERY, json!({})).await?;
        let teams = response.teams.nodes;

        if let Some(team) = teams
            .iter()
            .find(|team| team.key.eq_ignore_ascii_case(&requested_key))
        {
            if team.name.eq_ignore_ascii_case(name) {
                return Ok(TeamBinding {
                    id: team.id.clone(),
                    key: team.key.clone(),
                    created: false,
                });
            }
            return Err(PmError::Message(format!(
                "Linear team key {requested_key:?} already belongs to team {:?} (id {}). \
                 Pass a different --team-key or rename that team.",
                team.name, team.id
            )));
        }

        if let Some(team) = teams
            .iter()
            .find(|team| team.name.eq_ignore_ascii_case(name))
        {
            return Err(PmError::Message(format!(
                "a Linear team named {name:?} already exists with key {:?} (id {}). \
                 Pass --team-key {} to adopt it, or rename the team.",
                team.key, team.id, team.key
            )));
        }

        let response: TeamCreateData = self
            .graphql(
                CREATE_TEAM_MUTATION,
                json!({ "name": name, "key": requested_key }),
            )
            .await?;
        Ok(TeamBinding {
            id: response.team_create.team.id,
            key: requested_key,
            created: true,
        })
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

    /// The owning team id of an existing issue. State transitions resolve
    /// against this, not the wave-configured team, because a Project can span
    /// teams and Linear rejects a state that belongs to another team.
    async fn item_team_id(&self, item_id: &str) -> PmResult<String> {
        let response: IssueTeamData = self
            .graphql(ISSUE_TEAM_QUERY, json!({ "id": item_id }))
            .await?;
        response
            .issue
            .map(|issue| issue.team.id)
            .ok_or_else(|| PmError::Message(format!("no Linear issue with id {item_id}")))
    }

    async fn completed_state_id(&self, team_id: &str) -> PmResult<String> {
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

    pub async fn create_wave(&self, name: &str, summary: &str) -> PmResult<String> {
        let response: InitiativeCreateData = self
            .graphql(
                CREATE_INITIATIVE_MUTATION,
                json!({
                    "name": name,
                    "description": linear_description(summary),
                }),
            )
            .await?;
        Ok(response.initiative_create.initiative.id)
    }

    pub async fn rename_wave(&self, initiative_id: &str, name: &str) -> PmResult<()> {
        let _: Value = self
            .graphql(
                UPDATE_INITIATIVE_MUTATION,
                json!({
                    "id": initiative_id,
                    "name": name,
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn list_waves(&self) -> PmResult<Vec<PmWave>> {
        let mut after = None;
        let mut waves = Vec::new();
        loop {
            let response: InitiativesData = self
                .graphql(
                    LIST_INITIATIVES_QUERY,
                    json!({
                        "after": after,
                        "first": LIST_PROJECTS_PAGE_SIZE,
                    }),
                )
                .await?;
            let page = response.initiatives;
            waves.extend(page.nodes.into_iter().map(|initiative| PmWave {
                id: initiative.id,
                name: initiative.name,
                summary: initiative.description.unwrap_or_default(),
            }));
            if !page.page_info.has_next_page {
                return Ok(waves);
            }
            after = page.page_info.end_cursor;
        }
    }

    pub async fn create_project(
        &self,
        initiative_id: &str,
        name: &str,
        summary: &str,
        content: &ProjectContent,
    ) -> PmResult<String> {
        let team_id = self.resolve_team_id().await?;
        let response: ProjectCreateData = self
            .graphql(
                CREATE_PROJECT_MUTATION,
                json!({
                    "name": name,
                    "description": linear_description(summary),
                    "content": render_project_content(content),
                    "teamId": team_id,
                }),
            )
            .await?;
        let project_id = response.project_create.project.id;
        let _: Value = self
            .graphql(
                ATTACH_PROJECT_MUTATION,
                json!({
                    "initiativeId": initiative_id,
                    "projectId": project_id,
                }),
            )
            .await?;
        Ok(project_id)
    }

    pub async fn update_project(
        &self,
        project_id: &str,
        name: &str,
        summary: &str,
        content: &ProjectContent,
    ) -> PmResult<()> {
        let _: Value = self
            .graphql(
                UPDATE_PROJECT_MUTATION,
                json!({
                    "id": project_id,
                    "name": name,
                    "description": linear_description(summary),
                    "content": render_project_content(content),
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn archive_project(&self, project_id: &str) -> PmResult<()> {
        let response: ProjectArchiveData = self
            .graphql(
                ARCHIVE_PROJECT_MUTATION,
                json!({
                    "id": project_id,
                }),
            )
            .await?;
        if !response.project_archive.success {
            return Err(PmError::Message(format!(
                "Linear did not archive Project {project_id}"
            )));
        }
        Ok(())
    }

    pub async fn list_projects(&self, initiative_id: &str) -> PmResult<Vec<PmProject>> {
        let mut after = None;
        let mut projects = Vec::new();
        loop {
            let response: InitiativeProjectsData = self
                .graphql(
                    LIST_INITIATIVE_PROJECTS_QUERY,
                    json!({
                        "initiativeId": initiative_id,
                        "after": after,
                        "first": LIST_PROJECTS_PAGE_SIZE,
                    }),
                )
                .await?;
            let page = response.initiative.projects;
            projects.extend(page.nodes.into_iter().map(ProjectNode::into_pm_project));
            if !page.page_info.has_next_page {
                return Ok(projects);
            }
            after = page.page_info.end_cursor;
        }
    }

    pub async fn list_items(&self, project_id: &str) -> PmResult<Vec<PmItem>> {
        Ok(self
            .list_issue_nodes(project_id)
            .await?
            .into_iter()
            .enumerate()
            .map(|(rank, issue)| issue.into_pm_item(rank as u32))
            .collect())
    }

    async fn list_issue_nodes(&self, project_id: &str) -> PmResult<Vec<IssueNode>> {
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
                return Ok(issues);
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
        let mut input = serde_json::Map::new();
        if let Some(name) = update.name {
            input.insert("title".to_string(), json!(name));
        }
        if let Some(description) = update.description {
            input.insert("description".to_string(), json!(description));
        }

        let _: Value = self
            .graphql(
                UPDATE_ITEM_MUTATION,
                json!({
                    "id": item_id,
                    "input": input,
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn move_item_to_project(&self, item_id: &str, project_id: &str) -> PmResult<()> {
        let _: Value = self
            .graphql(
                MOVE_ITEM_MUTATION,
                json!({
                    "id": item_id,
                    "projectId": project_id,
                }),
            )
            .await?;
        Ok(())
    }

    /// Move an issue into another team and return its **new** identifier. The
    /// issue UUID is preserved (Task/PR/comment ownership survives); only the
    /// number changes, and Linear assigns it at move time, so we read it back.
    pub async fn move_item_to_team(&self, item_id: &str, team_id: &str) -> PmResult<String> {
        let response: IssueUpdateIdentifierData = self
            .graphql(
                MOVE_ITEM_TO_TEAM_MUTATION,
                json!({
                    "id": item_id,
                    "teamId": team_id,
                }),
            )
            .await?;
        Ok(response.issue_update.issue.identifier)
    }

    /// Reassign a Project to exactly one team. `teamIds` is a set replacement, so
    /// this pulls the Project off whatever team(s) it was on (e.g. the shared
    /// team) and onto the wave's team. Unlike issues, a Project keeps its id and
    /// slug across the move.
    pub async fn move_project_to_team(&self, project_id: &str, team_id: &str) -> PmResult<()> {
        let _: Value = self
            .graphql(
                MOVE_PROJECT_TO_TEAM_MUTATION,
                json!({
                    "id": project_id,
                    "teamId": team_id,
                }),
            )
            .await?;
        Ok(())
    }

    /// The key (Task prefix) of a team by its id, e.g. `PRD` for the product
    /// team. Errors if no team carries that id.
    pub async fn team_key(&self, team_id: &str) -> PmResult<String> {
        let response: TeamsData = self.graphql(LIST_TEAMS_QUERY, json!({})).await?;
        response
            .teams
            .nodes
            .into_iter()
            .find(|team| team.id == team_id)
            .map(|team| team.key)
            .ok_or_else(|| PmError::Message(format!("no Linear team with id {team_id}")))
    }

    pub async fn complete_item(&self, item_id: &str) -> PmResult<()> {
        let team_id = self.item_team_id(item_id).await?;
        let state_id = self.completed_state_id(&team_id).await?;
        let _: Value = self
            .graphql(
                SET_ITEM_STATE_MUTATION,
                json!({
                    "id": item_id,
                    "stateId": state_id,
                }),
            )
            .await?;
        Ok(())
    }

    /// Reopen a completed issue by moving it back to the team's default active
    /// (`unstarted`) workflow state. Mirrors [`complete_item`]; the repair path
    /// uses it when a Task was prematurely completed while its gates were open.
    /// Errors when the team has no unstarted state to return to.
    pub async fn reopen_item(&self, item_id: &str) -> PmResult<()> {
        let team_id = self.item_team_id(item_id).await?;
        let Some(state_id) = self.unstarted_state_id(&team_id).await? else {
            return Err(PmError::Message(format!(
                "no active Linear workflow state found to reopen issue {item_id}"
            )));
        };
        let _: Value = self
            .graphql(
                SET_ITEM_STATE_MUTATION,
                json!({
                    "id": item_id,
                    "stateId": state_id,
                }),
            )
            .await?;
        Ok(())
    }

    /// Post a new comment and return its Linear id so callers can update it in
    /// place later instead of posting a duplicate.
    pub async fn comment(&self, item_id: &str, body: &str) -> PmResult<String> {
        let response: CommentData = self
            .graphql(
                CREATE_COMMENT_MUTATION,
                json!({
                    "issueId": item_id,
                    "body": body,
                }),
            )
            .await?;
        Ok(response.comment_create.comment.id)
    }

    /// Find a previously-created comment by its stable body marker.
    ///
    /// Ask publication records an attempt before calling Linear. If that call
    /// succeeds but the local process dies before recording the returned id, a
    /// retry scans the issue's comments and adopts the existing one rather than
    /// creating a duplicate.
    pub async fn find_comment_with_marker(
        &self,
        issue_id: &str,
        marker: &str,
    ) -> PmResult<Option<String>> {
        let mut after = None;
        loop {
            let response: IssueCommentsData = self
                .graphql(
                    ISSUE_COMMENTS_QUERY,
                    json!({
                        "id": issue_id,
                        "comments": OBSERVATION_COMMENT_PAGE,
                        "after": after,
                    }),
                )
                .await?;
            let issue = response
                .issue
                .ok_or_else(|| PmError::Message(format!("linear issue {issue_id} not found")))?;
            if let Some(comment) = issue
                .comments
                .nodes
                .into_iter()
                .find(|comment| comment.body.contains(marker))
            {
                return Ok(Some(comment.id));
            }
            if !issue.comments.page_info.has_next_page {
                return Ok(None);
            }
            after = issue.comments.page_info.end_cursor;
            if after.is_none() {
                return Err(PmError::Message(format!(
                    "Linear comments for issue {issue_id} have another page without a cursor"
                )));
            }
        }
    }

    pub async fn update_comment(&self, comment_id: &str, body: &str) -> PmResult<()> {
        let _: Value = self
            .graphql(
                UPDATE_COMMENT_MUTATION,
                json!({
                    "id": comment_id,
                    "body": body,
                }),
            )
            .await?;
        Ok(())
    }

    /// Link an external URL to an issue as a first-class attachment. Returns the
    /// attachment id for in-place updates on later publishes.
    pub async fn link_attachment(
        &self,
        issue_id: &str,
        url: &str,
        title: &str,
    ) -> PmResult<String> {
        let response: AttachmentLinkData = self
            .graphql(
                LINK_ATTACHMENT_MUTATION,
                json!({
                    "issueId": issue_id,
                    "url": url,
                    "title": title,
                }),
            )
            .await?;
        Ok(response.attachment_link_url.attachment.id)
    }

    pub async fn update_attachment(
        &self,
        attachment_id: &str,
        title: &str,
        subtitle: &str,
    ) -> PmResult<()> {
        let _: Value = self
            .graphql(
                UPDATE_ATTACHMENT_MUTATION,
                json!({
                    "id": attachment_id,
                    "title": title,
                    "subtitle": subtitle,
                }),
            )
            .await?;
        Ok(())
    }

    /// Loopflow's own Linear user id, used to skip its own comments and edits.
    pub async fn viewer_id(&self) -> PmResult<String> {
        let response: ViewerData = self.graphql(VIEWER_QUERY, json!({})).await?;
        Ok(response.viewer.id)
    }

    /// Register a webhook for `Issue` and `Comment` changes, signed with `secret`.
    /// Returns the created webhook id.
    pub async fn create_webhook(&self, url: &str, secret: &str) -> PmResult<String> {
        let response: WebhookCreateData = self
            .graphql(
                CREATE_WEBHOOK_MUTATION,
                json!({
                    "url": url,
                    "secret": secret,
                    "resourceTypes": ["Issue", "Comment"],
                }),
            )
            .await?;
        Ok(response.webhook_create.webhook.id)
    }

    /// Read one issue's title, description, comments, and revision marker.
    pub async fn observe_issue(&self, issue_id: &str) -> PmResult<IssueObservation> {
        let response: IssueObservationData = self
            .graphql(
                ISSUE_OBSERVATION_QUERY,
                json!({ "id": issue_id, "comments": OBSERVATION_COMMENT_PAGE }),
            )
            .await?;
        let issue = response
            .issue
            .ok_or_else(|| PmError::Message(format!("linear issue {issue_id} not found")))?;
        Ok(IssueObservation {
            revision: issue.updated_at,
            title: issue.title,
            description: issue.description.unwrap_or_default(),
            comments: issue
                .comments
                .nodes
                .into_iter()
                .map(|node| IssueComment {
                    id: node.id,
                    body: node.body,
                    author_id: node.user.map(|user| user.id),
                })
                .collect(),
        })
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
struct ProjectArchiveData {
    #[serde(rename = "projectArchive")]
    project_archive: SuccessPayload,
}

#[derive(Deserialize)]
struct SuccessPayload {
    success: bool,
}

#[derive(Deserialize)]
struct InitiativeCreateData {
    #[serde(rename = "initiativeCreate")]
    initiative_create: InitiativePayload,
}

#[derive(Deserialize)]
struct IssueCreateData {
    #[serde(rename = "issueCreate")]
    issue_create: IssuePayload,
}

#[derive(Deserialize)]
struct IssueUpdateIdentifierData {
    #[serde(rename = "issueUpdate")]
    issue_update: IssueIdentifierPayload,
}

#[derive(Deserialize)]
struct IssueIdentifierPayload {
    issue: IssueIdentifierNode,
}

#[derive(Deserialize)]
struct IssueIdentifierNode {
    identifier: String,
}

#[derive(Deserialize)]
struct ProjectPayload {
    project: IdNode,
}

#[derive(Deserialize)]
struct InitiativePayload {
    initiative: IdNode,
}

#[derive(Deserialize)]
struct IssuePayload {
    issue: IdNode,
}

#[derive(Deserialize)]
struct CommentData {
    #[serde(rename = "commentCreate")]
    comment_create: CommentPayload,
}

#[derive(Deserialize)]
struct CommentPayload {
    comment: IdNode,
}

#[derive(Deserialize)]
struct AttachmentLinkData {
    #[serde(rename = "attachmentLinkURL")]
    attachment_link_url: AttachmentPayload,
}

#[derive(Deserialize)]
struct AttachmentPayload {
    attachment: IdNode,
}

#[derive(Deserialize)]
struct IdNode {
    id: String,
}

#[derive(Deserialize)]
struct ViewerData {
    viewer: IdNode,
}

#[derive(Deserialize)]
struct WebhookCreateData {
    #[serde(rename = "webhookCreate")]
    webhook_create: WebhookCreateNode,
}

#[derive(Deserialize)]
struct WebhookCreateNode {
    webhook: IdNode,
}

#[derive(Deserialize)]
struct IssueTeamData {
    issue: Option<IssueTeamNode>,
}

#[derive(Deserialize)]
struct IssueTeamNode {
    team: IdNode,
}

#[derive(Deserialize)]
struct IssueObservationData {
    issue: Option<IssueObservationNode>,
}

#[derive(Deserialize)]
struct IssueObservationNode {
    #[serde(rename = "updatedAt")]
    updated_at: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: Option<String>,
    comments: CommentConnection,
}

#[derive(Deserialize)]
struct IssueCommentsData {
    issue: Option<IssueCommentsNode>,
}

#[derive(Deserialize)]
struct IssueCommentsNode {
    comments: PagedCommentConnection,
}

#[derive(Deserialize)]
struct PagedCommentConnection {
    nodes: Vec<CommentNode>,
    #[serde(rename = "pageInfo")]
    page_info: PageInfo,
}

#[derive(Deserialize)]
struct CommentConnection {
    nodes: Vec<CommentNode>,
}

#[derive(Deserialize)]
struct CommentNode {
    id: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    user: Option<IdNode>,
}

#[derive(Deserialize)]
struct IssueNode {
    id: String,
    #[serde(default)]
    identifier: String,
    url: Option<String>,
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

        let identifier = if self.identifier.is_empty() {
            self.id.clone()
        } else {
            self.identifier
        };
        PmItem {
            id: self.id,
            identifier,
            url: self.url,
            name: self.title,
            description: self.description.unwrap_or_default(),
            rank,
            completed,
            project: None,
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
    key: String,
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
struct ProjectNode {
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    initiatives: IdConnection,
    #[serde(default)]
    teams: IdConnection,
}

impl ProjectNode {
    fn into_pm_project(self) -> PmProject {
        let content = parse_project_content(self.content.as_deref().unwrap_or_default());
        PmProject {
            id: self.id,
            slug: project_slug(&self.name),
            name: self.name,
            summary: self.description.unwrap_or_default(),
            definition: content.definition,
            flows: Some(content.flows),
            krs: content.krs,
            initiative_ids: self
                .initiatives
                .nodes
                .into_iter()
                .map(|initiative| initiative.id)
                .collect(),
            team_ids: Some(self.teams.nodes.into_iter().map(|team| team.id).collect()),
        }
    }
}

#[derive(Default, Deserialize)]
struct IdConnection {
    #[serde(default)]
    nodes: Vec<IdNode>,
}

#[derive(Deserialize)]
struct InitiativesData {
    initiatives: InitiativesConnection,
}

#[derive(Deserialize)]
struct InitiativesConnection {
    nodes: Vec<InitiativeNode>,
    #[serde(rename = "pageInfo")]
    page_info: PageInfo,
}

#[derive(Deserialize)]
struct InitiativeNode {
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Deserialize)]
struct InitiativeProjectsData {
    initiative: InitiativeWithProjects,
}

#[derive(Deserialize)]
struct InitiativeWithProjects {
    projects: ProjectsConnection,
}

#[derive(Deserialize)]
struct ProjectsConnection {
    nodes: Vec<ProjectNode>,
    #[serde(rename = "pageInfo")]
    page_info: PageInfo,
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

fn linear_description(description: &str) -> String {
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
    use crate::pm::test_server::{self, json_response};
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn issue_mutations_use_linear_string_ids() {
        for query in [
            UPDATE_ITEM_MUTATION,
            MOVE_ITEM_MUTATION,
            SET_ITEM_STATE_MUTATION,
            CREATE_COMMENT_MUTATION,
        ] {
            assert!(!query.contains(": ID!"));
        }
        assert!(MOVE_ITEM_MUTATION.contains("$projectId: String!"));
        assert!(SET_ITEM_STATE_MUTATION.contains("$stateId: String!"));
        assert!(CREATE_COMMENT_MUTATION.contains("$issueId: String!"));
    }

    /// A wrong argument on a Linear mutation must fail here, not ship a 400 on
    /// every publish (#1010, where `subtitle` on `attachmentLinkURL` shipped green
    /// because the mock echoed success). `subtitle` is legal on `attachmentUpdate`
    /// but not on `attachmentLinkURL`, so pin each mutation directly.
    #[test]
    fn attachment_link_omits_subtitle_and_update_keeps_it() {
        assert!(
            !LINK_ATTACHMENT_MUTATION.contains("subtitle"),
            "attachmentLinkURL rejects subtitle; it must not appear in the create mutation"
        );
        assert!(
            UPDATE_ATTACHMENT_MUTATION.contains("subtitle: $subtitle"),
            "attachmentUpdate carries PR state as its input subtitle"
        );
    }

    #[test]
    fn workflow_state_filters_use_linear_team_id() {
        assert!(CREATE_ITEM_MUTATION.contains("$teamId: String!"));
        assert!(LIST_COMPLETED_WORKFLOW_STATES_QUERY.contains("$teamId: ID!"));
        assert!(LIST_UNSTARTED_WORKFLOW_STATES_QUERY.contains("$teamId: ID!"));
    }

    #[tokio::test]
    async fn create_webhook_registers_issue_and_comment_resources() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "webhookCreate": { "webhook": { "id": "wh-1" } } } }),
        )])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        let id = client
            .create_webhook("https://loopflow.example/linear/webhook", "whsec")
            .await
            .expect("create webhook");
        assert_eq!(id, "wh-1");

        let requests = requests.lock().await;
        let body: Value = serde_json::from_str(&requests[0].body).expect("body is json");
        assert_eq!(
            body["variables"]["url"],
            "https://loopflow.example/linear/webhook"
        );
        assert_eq!(
            body["variables"]["resourceTypes"],
            json!(["Issue", "Comment"])
        );
        // Webhook input ids are String!, never ID! (see the position-sensitive
        // Linear id trap).
        assert!(CREATE_WEBHOOK_MUTATION.contains("$url: String!"));
        assert!(!CREATE_WEBHOOK_MUTATION.contains(": ID!"));
    }

    #[tokio::test]
    async fn viewer_id_reads_loopflows_own_user() {
        let (base_url, _requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "viewer": { "id": "user-loopflow" } } }),
        )])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        assert_eq!(
            client.viewer_id().await.expect("viewer id"),
            "user-loopflow"
        );
    }

    #[tokio::test]
    async fn observe_issue_reads_revision_content_and_comment_authors() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({
                "data": {
                    "issue": {
                        "updatedAt": "2026-07-15T18:00:00.000Z",
                        "title": "Stream Linear edits",
                        "description": "New body",
                        "comments": {
                            "nodes": [
                                { "id": "c-1", "body": "please prioritize", "user": { "id": "user-human" } },
                                { "id": "c-2", "body": "PR: https://x", "user": { "id": "user-loopflow" } },
                                { "id": "c-3", "body": "integration note", "user": null }
                            ]
                        }
                    }
                }
            }),
        )])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        let observation = client
            .observe_issue("issue-1")
            .await
            .expect("observe issue");
        assert_eq!(observation.revision, "2026-07-15T18:00:00.000Z");
        assert_eq!(observation.title, "Stream Linear edits");
        assert_eq!(observation.description, "New body");
        assert_eq!(
            observation.comments,
            vec![
                IssueComment {
                    id: "c-1".to_string(),
                    body: "please prioritize".to_string(),
                    author_id: Some("user-human".to_string()),
                },
                IssueComment {
                    id: "c-2".to_string(),
                    body: "PR: https://x".to_string(),
                    author_id: Some("user-loopflow".to_string()),
                },
                IssueComment {
                    id: "c-3".to_string(),
                    body: "integration note".to_string(),
                    author_id: None,
                },
            ]
        );

        let requests = requests.lock().await;
        let body: Value = serde_json::from_str(&requests[0].body).expect("body is json");
        assert_eq!(body["variables"]["id"], "issue-1");
        assert_eq!(body["variables"]["comments"], OBSERVATION_COMMENT_PAGE);
    }

    #[tokio::test]
    async fn observe_issue_reports_a_missing_issue() {
        let (base_url, _requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "issue": null } }),
        )])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        let error = client
            .observe_issue("issue-missing")
            .await
            .expect_err("missing issue errors");
        assert!(error.to_string().contains("issue-missing"));
    }

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
                                    "url": "https://linear.app/loopflow/issue/INF-1/first",
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
        assert_eq!(
            items[1].url.as_deref(),
            Some("https://linear.app/loopflow/issue/INF-1/first")
        );
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].authorization.as_deref(),
            Some("Bearer linear-secret")
        );
        let request: Value = serde_json::from_str(&requests[0].body).expect("request body is json");
        assert!(request["query"]
            .as_str()
            .expect("query string")
            .contains("$projectId: String!"));
        assert!(request["query"]
            .as_str()
            .expect("query string")
            .contains("\n        url\n"));
    }

    #[tokio::test]
    async fn list_projects_resolves_owning_teams() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "initiative": { "projects": {
                "nodes": [{
                    "id": "project-1",
                    "name": "Unified Practice Targets",
                    "description": "",
                    "content": "## Definition\n\nA bet.\n\n## KRs\n",
                    "initiatives": { "nodes": [{ "id": "initiative-1" }] },
                    "teams": { "nodes": [{ "id": "team-cadenza" }] }
                }],
                "pageInfo": { "hasNextPage": false, "endCursor": null }
            } } } }),
        )])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        let projects = client
            .list_projects("initiative-1")
            .await
            .expect("list projects");

        assert_eq!(projects.len(), 1);
        assert_eq!(
            projects[0].team_ids.as_deref(),
            Some(["team-cadenza".to_string()].as_slice())
        );
        let request: Value =
            serde_json::from_str(&requests.lock().await[0].body).expect("query json");
        assert!(request["query"]
            .as_str()
            .expect("query string")
            .contains("teams(first: 50)"));
    }

    #[tokio::test]
    async fn create_project_writes_content_then_attaches_to_initiative() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({ "data": { "projectCreate": { "project": { "id": "project-1" } } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "initiativeToProjectCreate": { "initiativeToProject": { "id": "link-1" } } } }),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-9".to_string()),
            base_url,
        );

        let project_id = client
            .create_project(
                "initiative-1",
                "Wave Chat",
                "Conversation stays in flow.",
                &ProjectContent {
                    definition: "Conversation stays in flow.".to_string(),
                    flows: crate::pm::ProjectFlowPlan::empty(),
                    krs: vec![PmKr {
                        text: "Replies stream".to_string(),
                        holds: false,
                    }],
                },
            )
            .await
            .expect("create project");

        assert_eq!(project_id, "project-1");
        let requests = requests.lock().await;
        let create: Value = serde_json::from_str(&requests[0].body).expect("create json");
        assert_eq!(create["variables"]["name"], "Wave Chat");
        assert!(create["variables"]["content"]
            .as_str()
            .expect("content")
            .contains("- [ ] Replies stream"));
        let attach: Value = serde_json::from_str(&requests[1].body).expect("attach json");
        assert_eq!(attach["variables"]["initiativeId"], "initiative-1");
        assert_eq!(attach["variables"]["projectId"], "project-1");
    }

    #[tokio::test]
    async fn update_project_replaces_definition_and_krs() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "projectUpdate": { "project": { "id": "project-1" } } } }),
        )])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-9".to_string()),
            base_url,
        );

        client
            .update_project(
                "project-1",
                "Wave Chat",
                "Conversation stays in flow.",
                &ProjectContent {
                    definition: "Conversation stays in flow.".to_string(),
                    flows: crate::pm::ProjectFlowPlan {
                        first: Some("incident".to_string()),
                        loop_: Some("ship-5whys".to_string()),
                        finally: Some("ship".to_string()),
                    },
                    krs: vec![PmKr {
                        text: "Replies survive every restart boundary".to_string(),
                        holds: false,
                    }],
                },
            )
            .await
            .expect("update project");

        let requests = requests.lock().await;
        let update: Value = serde_json::from_str(&requests[0].body).expect("update json");
        assert_eq!(update["variables"]["id"], "project-1");
        assert!(update["variables"]["content"]
            .as_str()
            .expect("content")
            .contains("Replies survive every restart boundary"));
        assert!(update["variables"]["content"]
            .as_str()
            .expect("content")
            .contains("loop: ship-5whys"));
    }

    #[tokio::test]
    async fn archive_project_uses_linear_archive_mutation() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "projectArchive": { "success": true } } }),
        )])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-9".to_string()),
            base_url,
        );

        client
            .archive_project("project-1")
            .await
            .expect("archive project");

        let requests = requests.lock().await;
        let archive: Value = serde_json::from_str(&requests[0].body).expect("archive json");
        assert!(archive["query"]
            .as_str()
            .expect("query")
            .contains("projectArchive"));
        assert_eq!(archive["variables"]["id"], "project-1");
    }

    #[tokio::test]
    async fn move_item_to_team_returns_the_reassigned_identifier() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "issueUpdate": { "issue": { "id": "issue-9", "identifier": "PRD-4" } } } }),
        )])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-old".to_string()),
            base_url,
        );

        let new_identifier = client
            .move_item_to_team("issue-9", "team-prd")
            .await
            .expect("move succeeds");

        assert_eq!(new_identifier, "PRD-4");
        let requests = requests.lock().await;
        let body: Value = serde_json::from_str(&requests[0].body).expect("move json");
        assert!(body["query"]
            .as_str()
            .expect("query")
            .contains("issueUpdate"));
        assert_eq!(body["variables"]["id"], "issue-9");
        assert_eq!(body["variables"]["teamId"], "team-prd");
    }

    #[tokio::test]
    async fn move_project_to_team_sets_the_team_ids() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "projectUpdate": { "project": { "id": "project-1" } } } }),
        )])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-old".to_string()),
            base_url,
        );

        client
            .move_project_to_team("project-1", "team-cadenza")
            .await
            .expect("move project succeeds");

        let requests = requests.lock().await;
        let body: Value = serde_json::from_str(&requests[0].body).expect("move json");
        let query = body["query"].as_str().expect("query");
        assert!(query.contains("projectUpdate"));
        // teamIds is a set replacement: exactly the wave's team, pulling the
        // Project off the shared team.
        assert!(query.contains("teamIds: [$teamId]"));
        assert_eq!(body["variables"]["id"], "project-1");
        assert_eq!(body["variables"]["teamId"], "team-cadenza");
    }

    #[tokio::test]
    async fn team_key_resolves_the_prefix_for_a_team_id() {
        let (base_url, _requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "teams": { "nodes": [
                { "id": "team-w2", "name": "Wave 2", "key": "W2" },
                { "id": "team-prd", "name": "Product", "key": "PRD" }
            ] } } }),
        )])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        assert_eq!(client.team_key("team-prd").await.expect("key"), "PRD");
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
        let update_body: Value =
            serde_json::from_str(&requests[2].body).expect("update body is json");
        assert_eq!(
            update_body["variables"]["input"],
            json!({
                "title": "Implement Linear client",
                "description": "Build the GraphQL adapter and tests",
            })
        );
    }

    #[tokio::test]
    async fn pr_linkage_maps_to_attachment_and_comment_mutations() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({ "data": { "attachmentLinkURL": { "attachment": { "id": "att-1" } } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "attachmentUpdate": { "attachment": { "id": "att-1" } } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "commentUpdate": { "comment": { "id": "comment-1" } } } }),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        let attachment_id = client
            .link_attachment("issue-1", "https://example/pr/7", "GitHub PR #7")
            .await
            .expect("link attachment succeeds");
        assert_eq!(attachment_id, "att-1");
        client
            .update_attachment("att-1", "GitHub PR #7", "Merged")
            .await
            .expect("update attachment succeeds");
        client
            .update_comment("comment-1", "updated body")
            .await
            .expect("update comment succeeds");

        let requests = requests.lock().await;
        assert_eq!(requests.len(), 3);

        let link: Value = serde_json::from_str(&requests[0].body).expect("link body is json");
        assert!(link["query"]
            .as_str()
            .expect("query present")
            .contains("attachmentLinkURL"));
        assert_eq!(link["variables"]["issueId"], json!("issue-1"));
        assert_eq!(link["variables"]["url"], json!("https://example/pr/7"));
        // The create path must not send an argument Linear rejects.
        // `attachmentLinkURL` has no `subtitle`; sending one is the 400 that
        // shipped in #1010. PR state rides the managed comment body instead.
        assert!(
            link["variables"].get("subtitle").is_none(),
            "attachmentLinkURL must not send a subtitle variable"
        );

        let update: Value =
            serde_json::from_str(&requests[1].body).expect("attachment update body is json");
        assert!(update["query"]
            .as_str()
            .expect("query present")
            .contains("attachmentUpdate"));
        assert_eq!(update["variables"]["subtitle"], json!("Merged"));

        let comment: Value =
            serde_json::from_str(&requests[2].body).expect("comment update body is json");
        assert!(comment["query"]
            .as_str()
            .expect("query present")
            .contains("commentUpdate"));
        assert_eq!(comment["variables"]["id"], json!("comment-1"));
    }

    #[tokio::test]
    async fn update_item_omits_absent_text_fields() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "issueUpdate": { "issue": { "id": "issue-123" } } } }),
        )])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        client
            .update_item(
                "issue-123",
                &PmItemUpdate {
                    name: None,
                    description: Some("Only the description changes".to_string()),
                },
            )
            .await
            .expect("description-only update succeeds");

        let requests = requests.lock().await;
        let update_body: Value =
            serde_json::from_str(&requests[0].body).expect("update body is json");
        assert_eq!(
            update_body["variables"]["input"],
            json!({ "description": "Only the description changes" })
        );
        assert!(update_body["variables"]["input"].get("title").is_none());
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
                },
            )
            .await
            .expect("create item succeeds");

        let requests = requests.lock().await;
        let create_body: Value =
            serde_json::from_str(&requests[1].body).expect("create body is json");
        assert_eq!(create_body["variables"]["stateId"], Value::Null);
    }

    #[tokio::test]
    async fn ensure_team_adopts_matching_key_without_creating() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "teams": { "nodes": [
                { "id": "team-prd", "name": "Product", "key": "PRD" },
                { "id": "team-inf", "name": "Infrastructure", "key": "INF" },
            ] } } }),
        )])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        let binding = client
            .ensure_team("Product", "PRD")
            .await
            .expect("adopt existing team");

        assert_eq!(
            binding,
            TeamBinding {
                id: "team-prd".to_string(),
                key: "PRD".to_string(),
                created: false,
            }
        );
        // Only the list query fired — no team was created.
        assert_eq!(requests.lock().await.len(), 1);
    }

    #[tokio::test]
    async fn ensure_team_creates_when_absent() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({ "data": { "teams": { "nodes": [] } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "teamCreate": { "team": { "id": "team-new" } } } }),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        let binding = client
            .ensure_team("Product", "prd")
            .await
            .expect("create team");

        assert_eq!(
            binding,
            TeamBinding {
                id: "team-new".to_string(),
                key: "PRD".to_string(),
                created: true,
            }
        );
        let requests = requests.lock().await;
        let create_body: Value =
            serde_json::from_str(&requests[1].body).expect("create body is json");
        assert_eq!(create_body["variables"]["key"], "PRD");
        assert_eq!(create_body["variables"]["name"], "Product");
    }

    #[tokio::test]
    async fn ensure_team_refuses_key_owned_by_another_team() {
        let (base_url, _requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "teams": { "nodes": [
                { "id": "team-x", "name": "Platform", "key": "PRD" },
            ] } } }),
        )])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        let err = client
            .ensure_team("Product", "PRD")
            .await
            .expect_err("conflicting key is refused");
        let message = err.to_string();
        assert!(message.contains("PRD"), "{message}");
        assert!(message.contains("Platform"), "{message}");
    }

    #[tokio::test]
    async fn ensure_team_refuses_name_owned_under_a_different_key() {
        let (base_url, _requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "teams": { "nodes": [
                { "id": "team-y", "name": "Product", "key": "PROD" },
            ] } } }),
        )])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        let err = client
            .ensure_team("Product", "PRD")
            .await
            .expect_err("name reused under a different key is refused");
        assert!(err.to_string().contains("PROD"), "{err}");
    }

    #[tokio::test]
    async fn complete_item_resolves_state_from_the_issue_team_not_the_wave_team() {
        // The client is bound to the wave's team, but the issue lives in a
        // *different* team (the ENG-*/W2-* split). The completed state must be
        // resolved from the issue's own team or Linear rejects the transition.
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({ "data": { "issue": { "team": { "id": "team-eng" } } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "workflowStates": { "nodes": [{ "id": "state-done" }] } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueUpdate": { "issue": { "id": "ENG-7" } } } }),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-wave".to_string()),
            base_url,
        );

        client.complete_item("ENG-7").await.expect("complete item");

        let requests = requests.lock().await;
        assert_eq!(requests.len(), 3);

        let team_body: Value = serde_json::from_str(&requests[0].body).expect("team body is json");
        assert!(team_body["query"]
            .as_str()
            .expect("query present")
            .contains("IssueTeam"));
        assert_eq!(team_body["variables"]["id"], json!("ENG-7"));

        // The state lookup carries the issue's team, never the wave-bound team.
        // Sabotage the fix (resolve from `team_id`) and this assertion goes red.
        let states_body: Value =
            serde_json::from_str(&requests[1].body).expect("states body is json");
        assert_eq!(states_body["variables"]["teamId"], json!("team-eng"));

        let set_body: Value = serde_json::from_str(&requests[2].body).expect("set body is json");
        assert_eq!(set_body["variables"]["stateId"], json!("state-done"));
        assert_eq!(set_body["variables"]["id"], json!("ENG-7"));
    }

    #[tokio::test]
    async fn reopen_item_resolves_state_from_the_issue_team_not_the_wave_team() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({ "data": { "issue": { "team": { "id": "team-eng" } } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "workflowStates": { "nodes": [
                    { "id": "state-todo", "position": 1.0 }
                ] } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueUpdate": { "issue": { "id": "ENG-7" } } } }),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-wave".to_string()),
            base_url,
        );

        client.reopen_item("ENG-7").await.expect("reopen item");

        let requests = requests.lock().await;
        let states_body: Value =
            serde_json::from_str(&requests[1].body).expect("states body is json");
        assert_eq!(states_body["variables"]["teamId"], json!("team-eng"));
    }

    #[test]
    fn linear_description_skips_headings_and_truncates() {
        let summary = linear_description(
            "## Vision\n\nThis is the first paragraph.\n\n## Strategy\n\nSecond paragraph.",
        );
        assert_eq!(summary, "This is the first paragraph.");

        let long = "a".repeat(300);
        assert_eq!(linear_description(&long).len(), 255);
        assert_eq!(linear_description(""), "");
    }
}
