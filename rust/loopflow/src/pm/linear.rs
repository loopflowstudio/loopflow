use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::sleep;
use tracing::warn;

#[cfg(test)]
mod deletion_tests;

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
const REPOSITORY_CLAIM_PREFIX: &str = "<!-- loopflow-repository:";

const LIST_TEAMS_QUERY: &str = r#"query ListTeams {
  teams {
    nodes {
      id
      name
      key
      description
    }
  }
}"#;

const CREATE_TEAM_MUTATION: &str = r#"mutation CreateTeam($name: String!, $key: String!, $description: String!) {
  teamCreate(input: { name: $name, key: $key, description: $description }) {
    team {
      id
    }
  }
}"#;

const UPDATE_TEAM_DESCRIPTION_MUTATION: &str = r#"mutation UpdateTeamDescription($id: String!, $description: String!) {
  teamUpdate(id: $id, input: { description: $description }) {
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

const LIST_INITIATIVE_PROJECTS_QUERY: &str = r#"query ListInitiativeProjects($initiativeId: String!, $after: String, $first: Int!, $includeArchived: Boolean!) {
  initiative(id: $initiativeId) {
    projects(after: $after, first: $first, includeSubInitiatives: false, includeArchived: $includeArchived) {
      nodes {
        id
        name
        updatedAt
        description
        content
        archivedAt
        status { type }
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

const CREATE_PROJECT_MUTATION: &str = r#"mutation CreateProject($id: String, $name: String!, $description: String!, $content: String!, $teamId: String!, $statusId: String!) {
  projectCreate(input: { id: $id, name: $name, description: $description, content: $content, teamIds: [$teamId], statusId: $statusId }) {
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

const SET_PROJECT_TEAMS_MUTATION: &str = r#"mutation SetProjectTeams($id: String!, $teamIds: [String!]!) {
  projectUpdate(id: $id, input: { teamIds: $teamIds }) {
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

const PROJECT_LIFECYCLE_QUERY: &str = r#"query ProjectLifecycle($id: String!) {
  project(id: $id) {
    archivedAt
    status { id type teamId }
  }
}"#;

const PROJECT_STATUSES_QUERY: &str = r#"query ProjectStatuses($after: String, $first: Int!) {
  projectStatuses(after: $after, first: $first) {
    nodes { id type teamId position }
    pageInfo { hasNextPage endCursor }
  }
}"#;

const COMPLETE_PROJECT_MUTATION: &str = r#"mutation CompleteProject($id: String!, $statusId: String!) {
  projectUpdate(id: $id, input: { statusId: $statusId }) {
    success
    project { status { id type teamId } }
  }
}"#;

const ATTACH_PROJECT_MUTATION: &str = r#"mutation AttachProject($initiativeId: String!, $projectId: String!) {
  initiativeToProjectCreate(input: { initiativeId: $initiativeId, projectId: $projectId }) {
    initiativeToProject {
      id
    }
  }
}"#;

const LIST_ITEMS_QUERY: &str = r#"query ListProjectIssues($projectId: String!, $after: String, $first: Int!, $includeArchived: Boolean!) {
  project(id: $projectId) {
    issues(first: $first, after: $after, includeArchived: $includeArchived) {
      nodes {
        id
        identifier
        branchName
        completedAt
        updatedAt
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
        project {
          id
          name
        }
        team {
          id
        }
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
}"#;

const ISSUE_OWNERSHIP_QUERY: &str = r#"query IssueOwnership($id: String!) {
  issue(id: $id) {
    id
    identifier
    branchName
    completedAt
    updatedAt
    url
    title
    description
    prioritySortOrder
    sortOrder
    assignee { id }
    state { type }
    team { id }
    project {
      id
      name
      updatedAt
      description
      content
      status { type }
      initiatives(first: 50) { nodes { id } }
      teams(first: 50) { nodes { id } }
    }
  }
}"#;

const PROJECT_OWNERSHIP_QUERY: &str = r#"query ProjectOwnership($id: String!) {
  project(id: $id) {
    id
    name
    updatedAt
    archivedAt
    description
    content
    status { type }
    initiatives(first: 50) { nodes { id } }
    teams(first: 50) { nodes { id } }
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

const ISSUE_DELETION_QUERY: &str = r#"query IssueDeletion($id: String!) {
  issue(id: $id) {
    trashed
  }
}"#;

const DELETE_ITEM_MUTATION: &str = r#"mutation DeleteIssue($id: String!) {
  issueDelete(id: $id) {
    success
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

// Loopflow's own OAuth user. Its id lets the observer distinguish a participant's edit or
// comment from Loopflow's own writeback, so ingestion never feeds itself.
const VIEWER_QUERY: &str = r#"query Viewer {
  viewer {
    id
  }
}"#;

// Register the webhook that streams issue/comment changes for this repository's
// one Team. The caller owns the signing secret and public URL.

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

// One issue's editable content plus a `createdAt`-ordered page of its comments.
// Each comment carries its author's ID and display name, but not `botActor`;
// integration-authored comments have no user author. Explicit steering carries
// separate requester metadata. `updatedAt` is the revision marker. The
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
        createdAt
        updatedAt
        user {
          id
          displayName
          name
        }
      }
      pageInfo { hasNextPage endCursor }
    }
  }
}"#;

const ISSUE_COMMENTS_QUERY: &str = r#"query IssueComments($id: String!, $comments: Int!, $after: String) {
  issue(id: $id) {
    comments(first: $comments, after: $after, orderBy: createdAt) {
      nodes {
        id
        body
        createdAt
        updatedAt
        user {
          id
          displayName
          name
        }
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
/// this many unseen participant comments between polls is not a real case; the cursor
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

    /// Adopt or create the team a repository should own, keyed by `key`.
    /// Diagnoses conflicts and claims the Team with the canonical Git origin:
    /// - the requested key already belongs to a team with the same name → adopt;
    /// - the requested key belongs to a *different*-named team → refuse;
    /// - the name exists under a different key → refuse (name the existing key);
    /// - neither exists → create.
    pub async fn ensure_team(
        &self,
        name: &str,
        key: &str,
        repository: &str,
    ) -> PmResult<TeamBinding> {
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
                return self.claim_team(team, repository).await;
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
            if team.key.eq_ignore_ascii_case(&requested_key) {
                return self.claim_team(team, repository).await;
            }
            return Err(PmError::Message(format!(
                "a Linear team named {name:?} already exists with key {:?} (id {}). \
                 Pass --team-key {} to adopt it, or rename the team.",
                team.key, team.id, team.key
            )));
        }

        let description = repository_claim_marker(repository);
        let response: TeamCreateData = self
            .graphql(
                CREATE_TEAM_MUTATION,
                json!({
                    "name": name,
                    "key": requested_key,
                    "description": description,
                }),
            )
            .await?;
        let team_id = response.team_create.team.id;
        let binding = self.validate_team_claim(&team_id, repository).await?;
        Ok(TeamBinding {
            created: true,
            ..binding
        })
    }

    async fn claim_team(&self, team: &TeamNode, repository: &str) -> PmResult<TeamBinding> {
        match repository_claim(team.description.as_deref().unwrap_or_default())? {
            Some(claimed) if claimed == repository => {}
            Some(claimed) => {
                return Err(PmError::Message(format!(
                    "Linear team {} ({}, key {}) is claimed by repository {claimed}; \
                     repository {repository} cannot use it",
                    team.name, team.id, team.key
                )))
            }
            None => {
                let description = description_with_repository_claim(
                    team.description.as_deref().unwrap_or_default(),
                    repository,
                );
                let _: Value = self
                    .graphql(
                        UPDATE_TEAM_DESCRIPTION_MUTATION,
                        json!({ "id": team.id, "description": description }),
                    )
                    .await?;
            }
        }
        let mut binding = self.validate_team_claim(&team.id, repository).await?;
        binding.created = false;
        Ok(binding)
    }

    pub async fn claim_configured_team(
        &self,
        team_id: &str,
        repository: &str,
        expected_name: Option<&str>,
        expected_key: Option<&str>,
    ) -> PmResult<TeamBinding> {
        let response: TeamsData = self.graphql(LIST_TEAMS_QUERY, json!({})).await?;
        let team = response
            .teams
            .nodes
            .into_iter()
            .find(|team| team.id == team_id)
            .ok_or_else(|| PmError::Message(format!("no Linear team with id {team_id}")))?;
        if let Some(name) = expected_name.filter(|name| !team.name.eq_ignore_ascii_case(name)) {
            return Err(PmError::Message(format!(
                "repository {repository} is already bound to Linear Team {} ({}, key {}); \
                 it cannot rebind through --team-name {name:?}. Run repository-wide `lf repo reteam` instead.",
                team.name, team.id, team.key
            )));
        }
        if let Some(key) = expected_key.filter(|key| !team.key.eq_ignore_ascii_case(key.trim())) {
            return Err(PmError::Message(format!(
                "repository {repository} is already bound to Linear Team {} ({}, key {}); \
                 it cannot rebind through --team-key {key:?}. Run repository-wide `lf repo reteam` instead.",
                team.name, team.id, team.key
            )));
        }
        self.claim_team(&team, repository).await
    }

    /// Validate that a stable Team id is claimed by this repository.
    pub async fn validate_team_claim(
        &self,
        team_id: &str,
        repository: &str,
    ) -> PmResult<TeamBinding> {
        let response: TeamsData = self.graphql(LIST_TEAMS_QUERY, json!({})).await?;
        let team = response
            .teams
            .nodes
            .into_iter()
            .find(|team| team.id == team_id)
            .ok_or_else(|| PmError::Message(format!("no Linear team with id {team_id}")))?;
        match repository_claim(team.description.as_deref().unwrap_or_default())? {
            Some(claimed) if claimed == repository => Ok(TeamBinding {
                id: team.id,
                key: team.key,
                created: false,
            }),
            Some(claimed) => Err(PmError::Message(format!(
                "Linear team {} ({}, key {}) is claimed by repository {claimed}; \
                 configured repository {repository} must choose another Team",
                team.name, team.id, team.key
            ))),
            None => Err(PmError::Message(format!(
                "Linear team {} ({}, key {}) has no Loopflow repository claim; \
                 run `lf repo connect --team-key {}` to claim it for {repository}",
                team.name, team.id, team.key, team.key
            ))),
        }
    }

    fn require_team_id(&self) -> PmResult<String> {
        self.team_id.clone().ok_or_else(|| {
            PmError::Message(
                "Linear write requires repository `pm.linear_team` in .lf/config.yaml; \
                 run `lf repo connect <wave> --team-key <KEY>`"
                    .to_string(),
            )
        })
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

    async fn project_status_id(&self, status: crate::pm::ProjectStatus) -> PmResult<String> {
        let team = self.require_team_id()?;
        self.project_statuses()
            .await?
            .into_iter()
            .filter(|choice| {
                choice.type_ == status && choice.team_id.as_deref().is_none_or(|id| id == team)
            })
            .min_by(|left, right| {
                right
                    .team_id
                    .is_some()
                    .cmp(&left.team_id.is_some())
                    .then_with(|| left.position.total_cmp(&right.position))
                    .then_with(|| left.id.cmp(&right.id))
            })
            .map(|choice| choice.id)
            .ok_or_else(|| {
                PmError::Message(format!(
                    "no {} Project status for team {team}",
                    status.as_str()
                ))
            })
    }

    pub async fn set_project_status(
        &self,
        project_id: &str,
        status: crate::pm::ProjectStatus,
    ) -> PmResult<()> {
        let status_id = self.project_status_id(status).await?;
        let response: Value = self
            .graphql(
                r#"mutation SetProjectStatus($id: String!, $statusId: String!) {
                projectUpdate(id: $id, input: { statusId: $statusId }) { success }
            }"#,
                json!({ "id": project_id, "statusId": status_id }),
            )
            .await?;
        if response["projectUpdate"]["success"] != true {
            return Err(PmError::Message(
                "Linear did not confirm Project status update".into(),
            ));
        }
        Ok(())
    }

    pub async fn create_project(
        &self,
        initiative_id: &str,
        name: &str,
        content: &ProjectContent,
        id: Option<&str>,
    ) -> PmResult<String> {
        content.validate()?;
        let team_id = self.require_team_id()?;
        let status_id = self
            .project_status_id(crate::pm::ProjectStatus::Planned)
            .await?;
        let response: ProjectCreateData = self
            .graphql(
                CREATE_PROJECT_MUTATION,
                json!({
                    "id": id,
                    "name": name,
                    "description": project_description(content),
                    "content": render_project_content(content),
                    "teamId": team_id,
                    "statusId": status_id,
                }),
            )
            .await?;
        let project_id = response.project_create.project.id;
        self.attach_project(initiative_id, &project_id).await?;
        Ok(project_id)
    }

    pub async fn attach_project(&self, initiative_id: &str, project_id: &str) -> PmResult<()> {
        let _: Value = self
            .graphql(
                ATTACH_PROJECT_MUTATION,
                json!({
                    "initiativeId": initiative_id,
                    "projectId": project_id,
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn update_project(
        &self,
        project_id: &str,
        name: &str,
        content: &ProjectContent,
    ) -> PmResult<()> {
        let _: Value = self
            .graphql(
                UPDATE_PROJECT_MUTATION,
                json!({
                    "id": project_id,
                    "name": name,
                    "description": project_description(content),
                    "content": render_project_content(content),
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn complete_and_archive_project(&self, project_id: &str) -> PmResult<()> {
        let response: ProjectLifecycleData = self
            .graphql(PROJECT_LIFECYCLE_QUERY, json!({ "id": project_id }))
            .await?;
        let project = response.project.ok_or_else(|| {
            PmError::Message(format!("Linear Project {project_id} is unavailable"))
        })?;
        if project.status.r#type != COMPLETED_STATE_TYPE {
            let status = self.completed_project_status(&project.status).await?;
            let response: ProjectCompleteData = self
                .graphql(
                    COMPLETE_PROJECT_MUTATION,
                    json!({ "id": project_id, "statusId": status }),
                )
                .await?;
            if !response.project_update.success
                || !response
                    .project_update
                    .project
                    .is_some_and(|project| project.status.r#type == COMPLETED_STATE_TYPE)
            {
                return Err(PmError::Message(format!(
                    "Linear did not complete Project {project_id}"
                )));
            }
        }
        if project.archived_at.is_some() {
            return Ok(());
        }
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

    async fn completed_project_status(&self, current: &ProjectStatusRef) -> PmResult<String> {
        self.project_statuses()
            .await?
            .into_iter()
            .filter(|status| {
                status.type_ == crate::pm::ProjectStatus::Completed
                    && status.team_id == current.team_id
            })
            .min_by(|left, right| {
                left.position
                    .total_cmp(&right.position)
                    .then_with(|| left.id.cmp(&right.id))
            })
            .map(|status| status.id)
            .ok_or_else(|| {
                PmError::Message(format!(
                    "Linear has no completed Project status in the scope of status {}",
                    current.id
                ))
            })
    }

    async fn project_statuses(&self) -> PmResult<Vec<ProjectStatusChoice>> {
        let mut after = None;
        let mut statuses = Vec::new();
        loop {
            let response: ProjectStatusesData = self
                .graphql(
                    PROJECT_STATUSES_QUERY,
                    json!({ "after": after, "first": LIST_PROJECTS_PAGE_SIZE }),
                )
                .await?;
            let page = response.project_statuses;
            statuses.extend(page.nodes);
            if !page.page_info.has_next_page {
                return Ok(statuses);
            }
            let cursor = page.page_info.end_cursor.ok_or_else(|| {
                PmError::Message("Linear Project statuses have no next-page cursor".into())
            })?;
            if after.as_ref() == Some(&cursor) {
                return Err(PmError::Message(
                    "Linear Project statuses repeated a page cursor".into(),
                ));
            }
            after = Some(cursor);
        }
    }

    pub async fn list_projects(&self, initiative_id: &str) -> PmResult<Vec<PmProject>> {
        self.list_projects_including_archived(initiative_id, false)
            .await
    }

    pub(crate) async fn list_projects_including_archived(
        &self,
        initiative_id: &str,
        include_archived: bool,
    ) -> PmResult<Vec<PmProject>> {
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
                        "includeArchived": include_archived,
                    }),
                )
                .await?;
            let page = response.initiative.projects;
            projects.extend(
                page.nodes
                    .into_iter()
                    .map(ProjectNode::into_pm_project)
                    .collect::<PmResult<Vec<_>>>()?,
            );
            if !page.page_info.has_next_page {
                return Ok(projects);
            }
            after = page.page_info.end_cursor;
        }
    }

    pub async fn list_items(&self, project_id: &str) -> PmResult<Vec<PmItem>> {
        self.list_items_including_archived(project_id, false).await
    }

    pub(crate) async fn list_items_including_archived(
        &self,
        project_id: &str,
        include_archived: bool,
    ) -> PmResult<Vec<PmItem>> {
        self.list_issue_nodes(project_id, include_archived)
            .await?
            .into_iter()
            .enumerate()
            .map(|(rank, issue)| issue.into_pm_item(rank as u32))
            .collect()
    }

    async fn list_issue_nodes(
        &self,
        project_id: &str,
        include_archived: bool,
    ) -> PmResult<Vec<IssueNode>> {
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
                        "includeArchived": include_archived,
                    }),
                )
                .await?;

            let page = response.project.issues;
            issues.extend(page.nodes);

            if !page.page_info.has_next_page {
                issues.sort_by(|left, right| {
                    left.fields
                        .priority_sort_order
                        .total_cmp(&right.fields.priority_sort_order)
                        .then_with(|| left.fields.sort_order.total_cmp(&right.fields.sort_order))
                });
                return Ok(issues);
            }

            after = page.page_info.end_cursor;
        }
    }

    pub async fn create_item(&self, project_id: &str, item: &PmItemCreate) -> PmResult<String> {
        let team_id = self.require_team_id()?;
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

    /// Replace the Project's Team set, preserving its identity and content.
    pub async fn set_project_teams(&self, project_id: &str, team_ids: &[String]) -> PmResult<()> {
        let _: Value = self
            .graphql(
                SET_PROJECT_TEAMS_MUTATION,
                json!({ "id": project_id, "teamIds": team_ids }),
            )
            .await?;
        Ok(())
    }

    /// Resolve one Issue directly by UUID or identifier, including its owning
    /// Project and Team. This is the online Task-to-Wave ownership edge.
    pub async fn issue_ownership(
        &self,
        issue_id: &str,
    ) -> PmResult<Option<(PmItem, Option<PmProject>)>> {
        let response: IssueOwnershipData = self
            .graphql(ISSUE_OWNERSHIP_QUERY, json!({ "id": issue_id }))
            .await?;
        response.issue.map(IssueNode::into_ownership).transpose()
    }

    pub async fn find_project(&self, project_id: &str) -> PmResult<Option<PmProject>> {
        // A filtered collection has an explicit empty result. The singular Project
        // endpoint can return NOT_FOUND as a GraphQL error, indistinguishable from
        // inaccessible evidence without provider-specific error guessing.
        let response: ProjectsData = self
            .graphql(
                r#"query FindProject($id: ID!) {
                projects(filter: { id: { eq: $id } }, first: 2, includeArchived: true) {
                    nodes { id name updatedAt archivedAt description content status { type }
                        initiatives(first: 50) { nodes { id } }
                        teams(first: 50) { nodes { id } }
                    }
                    pageInfo { hasNextPage endCursor }
                }
            }"#,
                json!({ "id": project_id }),
            )
            .await?;
        response
            .projects
            .nodes
            .into_iter()
            .next()
            .map(|project| {
                if project.archived_at.is_some() {
                    return Err(PmError::Message(format!(
                        "Project {project_id} is archived; reconcile its status in Linear"
                    )));
                }
                project.into_pm_project()
            })
            .transpose()
    }

    pub async fn project_ownership(&self, project_id: &str) -> PmResult<PmProject> {
        self.project_node(project_id).await?.into_pm_project()
    }

    // Only migration-marked Projects reach this path. Ordinary parsing accepts
    // flow: exclusively; no provider mutation occurs during read projection.
    pub(crate) async fn adopt_project(
        &self,
        project_id: &str,
        initiative: &str,
        team: &str,
        promote: bool,
        apply: bool,
    ) -> PmResult<PmProject> {
        let mut node = self.project_node(project_id).await?;
        let original = node.content.clone().unwrap_or_default();
        let converted = convert_legacy_project_content(&original)?;
        let promote = promote
            && node.archived_at.is_none()
            && matches!(
                node.status.type_,
                crate::pm::ProjectStatus::Backlog | crate::pm::ProjectStatus::Planned
            );
        node.content = Some(converted.clone());
        if promote {
            node.status.type_ = crate::pm::ProjectStatus::Started;
        }
        let mut project = node.into_pm_project()?;
        crate::pm::validate_project_ownership("adoption", initiative, Some(team), &project)?;
        if apply && (converted != original || promote) {
            let mut input = json!({"content":converted});
            if promote {
                input["statusId"] = json!(
                    self.project_status_id(crate::pm::ProjectStatus::Started)
                        .await?
                );
            }
            let response: Value = self
                .graphql(
                    r#"mutation AdoptProject($id: String!, $input: ProjectUpdateInput!) {
                    projectUpdate(id: $id, input: $input) { success }
                }"#,
                    json!({"id":project_id,"input":input}),
                )
                .await?;
            if response["projectUpdate"]["success"] != true {
                return Err(PmError::Message(
                    "Linear did not confirm Project adoption".into(),
                ));
            }
        }
        if apply {
            let confirmed = self.project_ownership(project_id).await?;
            // The accepted write advances updatedAt; compare authored facts and
            // retain the provider's confirmed revision for subsequent ingestion.
            project.revision.clone_from(&confirmed.revision);
            if confirmed != project {
                return Err(PmError::Message(format!(
                    "Project {project_id} changed during adoption; refresh and retry"
                )));
            }
        }
        Ok(project)
    }

    async fn project_node(&self, project_id: &str) -> PmResult<ProjectNode> {
        let response: ProjectOwnershipData = self
            .graphql(PROJECT_OWNERSHIP_QUERY, json!({ "id": project_id }))
            .await?;
        response
            .project
            .ok_or_else(|| PmError::Message(format!("no Linear Project with id {project_id}")))
    }

    /// Only an explicit trash flag confirms deletion. Inaccessible or missing
    /// issues are unresolved, including after a lost mutation response.
    pub async fn item_is_deleted(&self, item_id: &str) -> PmResult<bool> {
        let response: IssueDeletionData = self
            .graphql(ISSUE_DELETION_QUERY, json!({ "id": item_id }))
            .await
            .map_err(|cause| {
                PmError::Message(format!(
                    "cannot read deletion state for Linear issue {item_id}: {cause}"
                ))
            })?;
        let issue = response.issue.ok_or_else(|| {
            PmError::Message(format!(
                "Linear issue {item_id} is unavailable; absence does not confirm deletion"
            ))
        })?;
        Ok(issue.trashed == Some(true))
    }

    /// Trash with Linear's ordinary retention, preserving workflow outcome.
    pub async fn delete_item(&self, item_id: &str) -> PmResult<()> {
        if matches!(self.item_is_deleted(item_id).await, Ok(true)) {
            return Ok(());
        }
        let result: PmResult<DeleteItemData> = self
            .graphql(DELETE_ITEM_MUTATION, json!({ "id": item_id }))
            .await;
        let cause = match result {
            Ok(response) if response.issue_delete.success => return Ok(()),
            Ok(_) => "Linear returned issueDelete.success=false".to_string(),
            Err(cause) => cause.to_string(),
        };
        let confirmation = match self.item_is_deleted(item_id).await {
            Ok(true) => return Ok(()),
            Ok(false) => "the issue is not confirmed in trash".to_string(),
            Err(cause) => cause.to_string(),
        };
        Err(PmError::Message(format!(
            "Linear deletion of {item_id} is unconfirmed: {cause}; readback: {confirmation}"
        )))
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

    /// Preserve the issue and its history while recording a canceled outcome.
    pub async fn cancel_item(&self, item_id: &str) -> PmResult<()> {
        let (item, _) = self
            .issue_ownership(item_id)
            .await?
            .ok_or_else(|| PmError::Message(format!("Linear issue {item_id} is unavailable")))?;
        match item.state.as_deref() {
            Some("canceled") => return Ok(()),
            Some("completed" | "duplicate") => {
                return Err(PmError::Message(format!(
                    "{} is already terminal; cancellation would replace its outcome",
                    item.identifier
                )));
            }
            None => {
                return Err(PmError::Message(format!(
                    "{} has no observed workflow state",
                    item.identifier
                )))
            }
            Some(_) => {}
        }
        let response: WorkflowStatesData = self
            .graphql(
                r#"query CanceledWorkflowStates($teamId: ID!) {
              workflowStates(filter: { team: { id: { eq: $teamId } }, type: { eq: "canceled" } }) {
                nodes { id position }
              }
            }"#,
                json!({ "teamId": item.team_id }),
            )
            .await?;
        let state = response
            .workflow_states
            .nodes
            .into_iter()
            .min_by(|left, right| left.position.total_cmp(&right.position))
            .ok_or_else(|| PmError::Message("no canceled Linear workflow state found".into()))?;
        let result: PmResult<Value> = self
            .graphql(
                SET_ITEM_STATE_MUTATION,
                json!({ "id": item.id, "stateId": state.id }),
            )
            .await;
        // Read back even after an uncertain mutation response. HTTP success alone
        // is not evidence that Linear accepted the state transition.
        match self.issue_ownership(&item.id).await {
            Ok(Some((confirmed, _))) if confirmed.state.as_deref() == Some("canceled") => Ok(()),
            confirmation => Err(PmError::Message(format!("cancellation of {} is unconfirmed (mutation: {}; readback: {}); retry task abandon", item.identifier,
                result.err().map_or_else(|| "acknowledged".into(), |error| error.to_string()),
                confirmation.err().map_or_else(|| "issue is not canceled".into(), |error| error.to_string())))),
        }
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
    /// Comment publication records an attempt before calling Linear. If that call
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

    pub(crate) async fn item_attachment_urls(&self, item_id: &str) -> PmResult<Vec<String>> {
        #[derive(Deserialize)]
        struct Data {
            issue: Option<Issue>,
        }
        #[derive(Deserialize)]
        struct Issue {
            attachments: Attachments,
        }
        #[derive(Deserialize)]
        struct Attachments {
            nodes: Vec<Attachment>,
            #[serde(rename = "pageInfo")]
            page_info: PageInfo,
        }
        #[derive(Deserialize)]
        struct Attachment {
            url: String,
        }
        let mut after = None;
        let mut urls = Vec::new();
        loop {
            let response: Data = self
                .graphql(
                    r#"query IssueAttachments($id: String!, $after: String) {
                issue(id: $id) { attachments(first: 100, after: $after) {
                    nodes { url } pageInfo { hasNextPage endCursor }
                } }
            }"#,
                    json!({ "id": item_id, "after": after }),
                )
                .await?;
            let page = response
                .issue
                .ok_or_else(|| {
                    PmError::Message(format!("issue {item_id} attachments are unavailable"))
                })?
                .attachments;
            urls.extend(page.nodes.into_iter().map(|node| node.url));
            if !page.page_info.has_next_page {
                return Ok(urls);
            }
            let cursor = page.page_info.end_cursor.ok_or_else(|| {
                PmError::Message("attachment pagination omitted its continuation".into())
            })?;
            if after.as_ref() == Some(&cursor) {
                return Err(PmError::Message(
                    "attachment pagination did not advance".into(),
                ));
            }
            after = Some(cursor);
        }
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
        let mut page = issue.comments;
        let mut comments = Vec::new();
        loop {
            comments.extend(page.nodes.into_iter().map(|node| IssueComment {
                id: node.id,
                created_at: node.created_at,
                revision: node.updated_at,
                body: node.body,
                author_name: node.user.as_ref().and_then(|user| {
                    user.display_name
                        .as_deref()
                        .and_then(crate::engine::config::normalize_user_name)
                        .or_else(|| {
                            user.name
                                .as_deref()
                                .and_then(crate::engine::config::normalize_user_name)
                        })
                }),
                author_id: node.user.map(|user| user.id),
            }));
            if !page.page_info.has_next_page {
                break;
            }
            let after = page.page_info.end_cursor.ok_or_else(|| {
                PmError::Message("Linear comment page is missing its continuation cursor".into())
            })?;
            let response: IssueCommentsData = self
                .graphql(
                    ISSUE_COMMENTS_QUERY,
                    json!({"id": issue_id, "comments": OBSERVATION_COMMENT_PAGE, "after": after}),
                )
                .await?;
            page = response
                .issue
                .ok_or_else(|| PmError::Message(format!("linear issue {issue_id} not found")))?
                .comments;
        }
        // A correction to an older comment follows the original direction.
        comments.sort_by(|left, right| {
            left.revision
                .cmp(&right.revision)
                .then(left.id.cmp(&right.id))
        });
        Ok(IssueObservation {
            revision: issue.updated_at,
            title: issue.title,
            description: issue.description.unwrap_or_default(),
            comments,
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
struct IssueDeletionData {
    issue: Option<IssueDeletionNode>,
}

#[derive(Deserialize)]
struct IssueDeletionNode {
    trashed: Option<bool>,
}

#[derive(Deserialize)]
struct DeleteItemData {
    #[serde(rename = "issueDelete")]
    issue_delete: SuccessPayload,
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
    comments: PagedCommentConnection,
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
struct CommentNode {
    id: String,
    #[serde(rename = "createdAt", default)]
    created_at: Option<String>,
    #[serde(rename = "updatedAt")]
    updated_at: Option<String>,
    #[serde(default)]
    body: String,
    #[serde(default)]
    user: Option<CommentUser>,
}

#[derive(Deserialize)]
struct CommentUser {
    id: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(bound(deserialize = "P: Deserialize<'de>"))]
struct IssueNode<P = ProjectRef> {
    #[serde(flatten)]
    fields: IssueFields,
    #[serde(deserialize_with = "Option::deserialize")]
    project: Option<P>,
}

// List and detail observations require the same complete issue fields.
// Nullable fields must be present; omission is not a value to store.
#[derive(Deserialize)]
struct IssueFields {
    #[serde(rename = "completedAt", deserialize_with = "Option::deserialize")]
    completed_at: Option<String>,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    id: String,
    identifier: String,
    #[serde(rename = "branchName")]
    branch_name: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    url: Option<String>,
    title: String,
    #[serde(deserialize_with = "Option::deserialize")]
    description: Option<String>,
    #[serde(rename = "prioritySortOrder")]
    priority_sort_order: f64,
    #[serde(rename = "sortOrder")]
    sort_order: f64,
    #[serde(deserialize_with = "Option::deserialize")]
    assignee: Option<IdNode>,
    #[serde(deserialize_with = "Option::deserialize")]
    state: Option<WorkflowStateRef>,
    #[serde(deserialize_with = "Option::deserialize")]
    team: Option<IdNode>,
}

impl IssueNode {
    fn into_pm_item(self, rank: u32) -> PmResult<PmItem> {
        self.fields.into_pm_item(rank, self.project)
    }
}

impl IssueFields {
    fn into_pm_item(self, rank: u32, project: Option<ProjectRef>) -> PmResult<PmItem> {
        if let Some(date) = &self.completed_at {
            time::OffsetDateTime::parse(date, &time::format_description::well_known::Rfc3339)
                .map_err(|error| {
                    PmError::Message(format!("invalid Linear completion time: {error}"))
                })?;
        }
        let completed = self
            .state
            .as_ref()
            .is_some_and(|state| state.r#type.eq_ignore_ascii_case(COMPLETED_STATE_TYPE));

        let identifier = if self.identifier.is_empty() {
            self.id.clone()
        } else {
            self.identifier
        };
        let project_id = project.as_ref().map(|project| project.id.clone());
        let project = project.map(|project| project_slug(&project.name));
        let team = self
            .team
            .ok_or_else(|| PmError::Message(format!("Linear issue {identifier} has no Team")))?;
        // Validate revision precision before admitting provider facts.
        time::OffsetDateTime::parse(
            &self.updated_at,
            &time::format_description::well_known::Rfc3339,
        )
        .map_err(|error| PmError::Message(format!("invalid Linear issue revision: {error}")))?;
        Ok(PmItem {
            revision: Some(self.updated_at),
            id: self.id,
            identifier,
            branch_name: self.branch_name,
            url: self.url,
            name: self.title,
            description: self.description.unwrap_or_default(),
            rank,
            completed,
            completed_at: self.completed_at,
            state: self.state.map(|state| state.r#type),
            project_id,
            project,
            team_id: team.id,
            assignee: self.assignee.map(|assignee| assignee.id),
        })
    }
}

#[derive(Deserialize)]
struct ProjectRef {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct IssueOwnershipData {
    #[serde(deserialize_with = "Option::deserialize")]
    issue: Option<IssueNode<ProjectNode>>,
}

impl IssueNode<ProjectNode> {
    fn into_ownership(self) -> PmResult<(PmItem, Option<PmProject>)> {
        let item = self.fields.into_pm_item(
            0,
            self.project.as_ref().map(|project| ProjectRef {
                id: project.id.clone(),
                name: project.name.clone(),
            }),
        )?;
        Ok((
            item,
            self.project.map(ProjectNode::into_pm_project).transpose()?,
        ))
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
    #[serde(default)]
    description: Option<String>,
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
struct ProjectStatusNode {
    #[serde(rename = "type")]
    type_: crate::pm::ProjectStatus,
}

#[derive(Deserialize)]
struct ProjectStatusChoice {
    id: String,
    #[serde(rename = "type")]
    type_: crate::pm::ProjectStatus,
    position: f64,
    #[serde(rename = "teamId")]
    team_id: Option<String>,
}

#[derive(Deserialize)]
struct ProjectStatusesData {
    #[serde(rename = "projectStatuses")]
    project_statuses: ProjectStatusesConnection,
}

#[derive(Deserialize)]
struct ProjectStatusesConnection {
    nodes: Vec<ProjectStatusChoice>,
    #[serde(rename = "pageInfo")]
    page_info: PageInfo,
}

#[derive(Deserialize)]
struct ProjectNode {
    id: String,
    #[serde(rename = "updatedAt")]
    revision: Option<String>,
    name: String,
    #[serde(rename = "archivedAt")]
    archived_at: Option<String>,
    status: ProjectStatusNode,
    #[serde(deserialize_with = "Option::deserialize")]
    description: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    content: Option<String>,
    initiatives: IdConnection,
    teams: IdConnection,
}

fn convert_legacy_project_content(content: &str) -> PmResult<String> {
    let current = parse_project_content(content)?.flow;
    let mut section = false;
    let mut legacy: Option<String> = None;
    let mut converted = String::new();
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") {
            section = trimmed.eq_ignore_ascii_case("## Flows");
        }
        if let Some(value) = section
            .then(|| trimmed.strip_prefix("recommended:"))
            .flatten()
        {
            let value = value.trim();
            if (!current.is_empty() && current != value)
                || legacy.as_deref().is_some_and(|previous| previous != value)
            {
                return Err(PmError::Message("Project has conflicting legacy and current default Flows; resolve its content in Linear".into()));
            }
            if current.is_empty() && legacy.is_none() {
                converted.push_str(&line.replacen("recommended:", "flow:", 1));
            }
            legacy = Some(value.to_owned());
        } else {
            converted.push_str(line);
        }
    }
    Ok(converted)
}

#[derive(Deserialize)]
struct ProjectLifecycleData {
    #[serde(deserialize_with = "Option::deserialize")]
    project: Option<ProjectLifecycle>,
}

#[derive(Deserialize)]
struct ProjectLifecycle {
    #[serde(rename = "archivedAt", deserialize_with = "Option::deserialize")]
    archived_at: Option<String>,
    status: ProjectStatusRef,
}

#[derive(Deserialize)]
struct ProjectStatusRef {
    id: String,
    r#type: String,
    #[serde(rename = "teamId", deserialize_with = "Option::deserialize")]
    team_id: Option<String>,
}

#[derive(Deserialize)]
struct ProjectCompleteData {
    #[serde(rename = "projectUpdate")]
    project_update: ProjectCompletePayload,
}

#[derive(Deserialize)]
struct ProjectCompletePayload {
    success: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    project: Option<ProjectWithStatus>,
}

#[derive(Deserialize)]
struct ProjectWithStatus {
    status: ProjectStatusRef,
}

impl ProjectNode {
    fn into_pm_project(self) -> PmResult<PmProject> {
        let content = parse_project_content(self.content.as_deref().unwrap_or_default())?;
        Ok(PmProject {
            id: self.id,
            revision: self.revision,
            slug: project_slug(&self.name),
            name: self.name,
            summary: self.description.unwrap_or_default(),

            metric_targets: content.metric_targets,
            flow: content.flow,
            // Older rotation archived its predecessor without changing status.
            // Retain that history without presenting it as a current plan.
            status: if self.archived_at.is_some()
                && self.status.type_ != crate::pm::ProjectStatus::Canceled
            {
                crate::pm::ProjectStatus::Completed
            } else {
                self.status.type_
            },
            krs: content.krs,
            initiative_ids: self
                .initiatives
                .nodes
                .into_iter()
                .map(|initiative| initiative.id)
                .collect(),
            team_ids: self.teams.nodes.into_iter().map(|team| team.id).collect(),
        })
    }
}

#[derive(Deserialize)]
struct ProjectOwnershipData {
    project: Option<ProjectNode>,
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
struct ProjectsData {
    projects: ProjectsConnection,
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

fn repository_claim_marker(repository: &str) -> String {
    format!("{REPOSITORY_CLAIM_PREFIX} {repository} -->")
}

fn repository_claim(description: &str) -> PmResult<Option<String>> {
    let mut claims = Vec::new();
    for line in description.lines() {
        if !line.contains(REPOSITORY_CLAIM_PREFIX) {
            continue;
        }
        let trimmed = line.trim();
        let Some(value) = trimmed
            .strip_prefix(REPOSITORY_CLAIM_PREFIX)
            .and_then(|value| value.strip_suffix("-->"))
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Err(PmError::Message(format!(
                "Linear Team has a malformed Loopflow repository marker: {trimmed:?}"
            )));
        };
        claims.push(value.to_string());
    }
    match claims.as_slice() {
        [] => Ok(None),
        [claim] => Ok(Some(claim.clone())),
        _ => Err(PmError::Message(format!(
            "Linear Team has multiple Loopflow repository markers: {}",
            claims.join(", ")
        ))),
    }
}

fn description_with_repository_claim(description: &str, repository: &str) -> String {
    let human = description.trim_end();
    let marker = repository_claim_marker(repository);
    if human.is_empty() {
        marker
    } else {
        format!("{human}\n\n{marker}")
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

fn project_description(content: &ProjectContent) -> String {
    linear_description(
        &content
            .krs
            .iter()
            .map(|kr| kr.text.as_str())
            .collect::<Vec<_>>()
            .join("; "),
    )
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
    fn adoption_converts_only_the_legacy_flow_key_and_preserves_authored_content() {
        let before =
            "Retain prose.\n\n## Flows\nrecommended: custom\n\n## KRs\n- [ ] Original proof\n";
        let after = convert_legacy_project_content(before).unwrap();
        assert_eq!(after, before.replace("recommended:", "flow:"));
        assert_eq!(parse_project_content(&after).unwrap().flow, "custom");
        assert_eq!(convert_legacy_project_content(&after).unwrap(), after);
        assert!(parse_project_content(before).unwrap().flow.is_empty());
        assert!(
            convert_legacy_project_content("flow: different\n\n## Flows\nrecommended: custom")
                .is_err()
        );
        assert_eq!(
            convert_legacy_project_content("## Notes\nrecommended: prose").unwrap(),
            "## Notes\nrecommended: prose"
        );
    }

    #[tokio::test]
    async fn migration_adoption_accepts_a_started_project_without_flow() {
        let original = "Original prose.\n\n## KRs\n- [ ] Preserve this proof\n";
        let response = json!({"data":{"project": {
            "id":"project-1", "name":"Summer work — customer requests",
            "description":"Original description", "content":original,
            "status":{"type":"started"}, "archivedAt":null,
            "initiatives":{"nodes":[{"id":"initiative-1"}]},
            "teams":{"nodes":[{"id":"team-1"}]}
        }}});
        let (base_url, _) = test_server::spawn(vec![
            json_response(StatusCode::OK, response.clone()),
            json_response(StatusCode::OK, response),
        ])
        .await;
        let client = LinearClient::with_base_url("fixture".into(), Some("team-1".into()), base_url);
        let adopted = client
            .adopt_project("project-1", "initiative-1", "team-1", false, true)
            .await
            .unwrap();
        assert_eq!(adopted.id, "project-1");
        assert_eq!(adopted.name, "Summer work — customer requests");
        assert!(adopted.flow.is_empty());
        assert_eq!(adopted.krs[0].text, "Preserve this proof");
        assert_eq!(adopted.status, crate::pm::ProjectStatus::Started);
    }

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

    #[test]
    fn task_history_provider_facts_require_valid_observed_completion_dates() {
        let planning: crate::pm::PmSnapshot = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        for item in planning.items {
            let mut wire = json!({
                "id":item.id, "identifier":item.identifier, "updatedAt":item.revision,
                "branchName":null,"url":null,"title":item.name,"description":item.description,
                "prioritySortOrder":0.0,"sortOrder":0.0,"assignee":null,
                "state":{"type":item.state},"team":{"id":"team"},
                "completedAt":item.completed_at
            });
            let fields: super::IssueFields = serde_json::from_value(wire.clone()).unwrap();
            let observed = fields.into_pm_item(item.rank, None).unwrap();
            assert_eq!(observed.completed, item.completed);
            assert_eq!(observed.state, item.state);
            assert_eq!(observed.completed_at, item.completed_at);
            wire["completedAt"] = json!("not a timestamp");
            assert!(serde_json::from_value::<super::IssueFields>(wire.clone())
                .unwrap()
                .into_pm_item(0, None)
                .is_err());
            wire.as_object_mut().unwrap().remove("completedAt");
            assert!(serde_json::from_value::<super::IssueFields>(wire).is_err());
        }
        assert_eq!(
            crate::pm::terminal_reason(Some("duplicate"), false),
            Some("Linear Task is duplicate")
        );
        assert_eq!(crate::pm::terminal_reason(Some("unknown"), false), None);
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
                            "pageInfo": {"hasNextPage": false, "endCursor": null},
                            "nodes": [
                                { "id": "c-1", "body": "please prioritize", "user": { "id": "user-human", "displayName": "Jack", "name": "Jack F" } },
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
                    author_name: Some("Jack".to_string()),
                    id: "c-1".to_string(),
                    created_at: None,
                    revision: None,
                    body: "please prioritize".to_string(),
                    author_id: Some("user-human".to_string()),
                },
                IssueComment {
                    author_name: None,
                    id: "c-2".to_string(),
                    created_at: None,
                    revision: None,
                    body: "PR: https://x".to_string(),
                    author_id: Some("user-loopflow".to_string()),
                },
                IssueComment {
                    author_name: None,
                    id: "c-3".to_string(),
                    created_at: None,
                    revision: None,
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
    async fn observe_issue_reads_every_comment_page_in_revision_order() {
        // Model GraphQL's field selection so a response fixture cannot supply
        // author names that a continuation query forgot to request.
        let app = axum::Router::new().route("/", axum::routing::post(|axum::Json(request): axum::Json<Value>| async move {
            let first = request["variables"]["after"].is_null();
            let (id, date, name) = if first {
                ("newer", "2026-09-23T00:00:00Z", "Jack")
            } else {
                ("older", "2026-09-22T00:00:00Z", "Maya")
            };
            let mut user = json!({"id": format!("person-{id}")});
            let query = request["query"].as_str().unwrap();
            if query.contains("displayName") {
                user["displayName"] = json!(name);
            }
            axum::Json(json!({"data": {"issue": {
                "updatedAt": "2026-09-23T00:00:00Z", "title": "Task", "description": "",
                "comments": {
                    "nodes": [{"id": id, "body": "advice", "updatedAt": date, "user": user}],
                    "pageInfo": {"hasNextPage": first, "endCursor": if first { Some("cursor-1") } else { None }}
                }
            }}}))
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = LinearClient::with_base_url("fixture-token".into(), None, url);
        let observed = client.observe_issue("issue-1").await.unwrap();
        server.abort();
        assert_eq!(
            observed
                .comments
                .iter()
                .map(|comment| comment.id.as_str())
                .collect::<Vec<_>>(),
            ["older", "newer"]
        );
        assert_eq!(
            observed.comments[1].revision.as_deref(),
            Some("2026-09-23T00:00:00Z")
        );
        assert_eq!(
            observed
                .comments
                .iter()
                .map(|comment| comment.author_name.as_deref())
                .collect::<Vec<_>>(),
            [Some("Maya"), Some("Jack")]
        );
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
                                    "identifier": "LOO-1",
                                    "url": "https://linear.app/loopflow/issue/INF-1/first",
                                    "title": "First",
                                    "description": "one",
                                    "completedAt": null, "prioritySortOrder": 10.0,
                                    "sortOrder": 10.0, "updatedAt":"2026-09-29T12:00:00.123Z",
                                    "assignee": { "id": "user-1" },
                                    "state": { "type": "unstarted" },
                                    "project": { "id": "project-123", "name": "Scan" },
                                    "team": { "id": "team-9" }
                                },
                                {
                                    "id": "issue-2",
                                    "identifier": "LOO-2",
                                    "url": null,
                                    "assignee": null,
                                    "title": "Second",
                                    "description": "two",
                                    "completedAt": null, "prioritySortOrder": 0.0,
                                    "sortOrder": 0.0, "updatedAt":"2026-09-29T12:00:00.123Z",
                                    "state": { "type": "completed" },
                                    "project": { "id": "project-123", "name": "Scan" },
                                    "team": { "id": "team-9" }
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
                    "status": {"type":"started"},
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
        assert_eq!(projects[0].team_ids, ["team-cadenza".to_string()]);
        let request: Value =
            serde_json::from_str(&requests.lock().await[0].body).expect("query json");
        assert!(request["query"]
            .as_str()
            .expect("query string")
            .contains("teams(first: 50)"));
    }

    #[tokio::test]
    async fn create_project_without_flow_retains_krs_and_attaches_to_initiative() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(StatusCode::OK, json!({"data":{"projectStatuses":{
                "nodes":[{"id":"planned","type":"planned","position":0.0,"teamId":null}],
                "pageInfo":{"hasNextPage":false,"endCursor":null}
            }}})),
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
                &ProjectContent {
                    metric_targets: Vec::new(),
                    flow: String::new(),
                    krs: vec![PmKr {
                        text: "Replies stream".to_string(),
                        holds: false,
                    }],
                },
                None,
            )
            .await
            .expect("create project");

        assert_eq!(project_id, "project-1");
        let requests = requests.lock().await;
        let create: Value = serde_json::from_str(&requests[1].body).expect("create json");
        assert_eq!(create["variables"]["name"], "Wave Chat");
        let content = create["variables"]["content"].as_str().unwrap();
        assert!(content.contains("- [ ] Replies stream"));
        assert!(!content.contains("flow:"));
        let attach: Value = serde_json::from_str(&requests[2].body).expect("attach json");
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
                &ProjectContent {
                    metric_targets: Vec::new(),
                    flow: "task-design".to_string(),
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
            .contains("flow: task-design"));
    }

    #[tokio::test]
    async fn project_status_selection_preserves_team_preference_and_completion_scope() {
        let pages = [
            json!({"data":{"projectStatuses":{"nodes":[
                {"id":"workspace", "type":"completed", "teamId":null, "position":0.0},
                {"id":"later", "type":"completed", "teamId":"selected", "position":10.0}
            ], "pageInfo":{"hasNextPage":true, "endCursor":"page-2"}}}}),
            json!({"data":{"projectStatuses":{"nodes":[
                {"id":"z-tie", "type":"completed", "teamId":"selected", "position":5.0},
                {"id":"a-tie", "type":"completed", "teamId":"selected", "position":5.0},
                {"id":"current-done", "type":"completed", "teamId":"current", "position":1.0},
                {"id":"started", "type":"started", "teamId":"selected", "position":0.0}
            ], "pageInfo":{"hasNextPage":false, "endCursor":null}}}}),
        ];
        for (team, preferred, scope, expected) in [
            ("selected", "a-tie", Some("selected"), Some("a-tie")),
            ("selected", "a-tie", Some("current"), Some("current-done")),
            ("selected", "a-tie", None, Some("workspace")),
            ("missing", "workspace", Some("missing"), None),
        ] {
            let responses = pages
                .iter()
                .cycle()
                .take(4)
                .map(|page| json_response(StatusCode::OK, page.clone()))
                .collect();
            let (base_url, _) = test_server::spawn(responses).await;
            let client = LinearClient::with_base_url("fixture".into(), Some(team.into()), base_url);
            assert_eq!(
                client
                    .project_status_id(crate::pm::ProjectStatus::Completed)
                    .await
                    .unwrap(),
                preferred
            );
            let completion = client
                .completed_project_status(&ProjectStatusRef {
                    id: "current-status".into(),
                    r#type: "started".into(),
                    team_id: scope.map(str::to_string),
                })
                .await;
            match expected {
                Some(id) => assert_eq!(completion.unwrap(), id),
                None => assert!(completion
                    .unwrap_err()
                    .to_string()
                    .contains("no completed Project status in the scope of status current-status")),
            }
        }
    }

    #[tokio::test]
    async fn project_status_selection_rejects_incomplete_pagination() {
        for cursor in [Value::Null, json!("page-2")] {
            let responses = [json!("page-2"), cursor]
                .into_iter()
                .map(|cursor| {
                    json_response(
                        StatusCode::OK,
                        json!({"data":{"projectStatuses":{"nodes":[],
                            "pageInfo":{"hasNextPage":true, "endCursor":cursor}}}}),
                    )
                })
                .collect();
            let (base_url, _) = test_server::spawn(responses).await;
            let client =
                LinearClient::with_base_url("fixture".into(), Some("selected".into()), base_url);
            let error = client
                .project_status_id(crate::pm::ProjectStatus::Completed)
                .await
                .unwrap_err();
            assert!(error.to_string().contains("cursor"));
        }
    }

    #[tokio::test]
    async fn complete_and_archive_project_reports_provider_refusal() {
        let (base_url, _requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({ "data": { "project": { "archivedAt": null,
                    "status": { "id": "done", "type": "completed", "teamId": null } } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "projectArchive": { "success": false } } }),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-9".to_string()),
            base_url,
        );

        let error = client
            .complete_and_archive_project("project-1")
            .await
            .expect_err("provider refused archive");
        assert!(error
            .to_string()
            .contains("Linear did not archive Project project-1"));
    }

    #[tokio::test]
    async fn complete_and_archive_project_requires_confirmed_completion() {
        for project in [
            Value::Null,
            json!({"status":{"id":"started", "type":"started", "teamId":null}}),
        ] {
            let (base_url, _) = test_server::spawn(vec![
                json_response(
                    StatusCode::OK,
                    json!({"data":{"project":{"archivedAt":null,
                        "status":{"id":"started", "type":"started", "teamId":null}}}}),
                ),
                json_response(
                    StatusCode::OK,
                    json!({"data":{"projectStatuses":{"nodes":[
                        {"id":"done", "type":"completed", "teamId":null, "position":0.0}
                    ], "pageInfo":{"hasNextPage":false, "endCursor":null}}}}),
                ),
                json_response(
                    StatusCode::OK,
                    json!({"data":{"projectUpdate":{"success":true, "project":project}}}),
                ),
            ])
            .await;
            let client = LinearClient::with_base_url("fixture".into(), None, base_url);
            let error = client
                .complete_and_archive_project("project-1")
                .await
                .unwrap_err();
            assert!(error
                .to_string()
                .contains("did not complete Project project-1"));
        }
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
        let claimed = "<!-- loopflow-repository: loopflowstudio/loopflow -->";
        let teams = json_response(
            StatusCode::OK,
            json!({ "data": { "teams": { "nodes": [
                { "id": "team-prd", "name": "Product", "key": "PRD", "description": claimed },
                { "id": "team-inf", "name": "Infrastructure", "key": "INF", "description": null },
            ] } } }),
        );
        let (base_url, requests) = test_server::spawn(vec![teams.clone(), teams]).await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        let binding = client
            .ensure_team("Product", "PRD", "loopflowstudio/loopflow")
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
        assert_eq!(requests.lock().await.len(), 2);
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
            json_response(
                StatusCode::OK,
                json!({ "data": { "teams": { "nodes": [{
                    "id": "team-new", "name": "Product", "key": "PRD",
                    "description": "<!-- loopflow-repository: loopflowstudio/loopflow -->"
                }] } } }),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        let binding = client
            .ensure_team("Product", "prd", "loopflowstudio/loopflow")
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
        assert_eq!(
            create_body["variables"]["description"],
            "<!-- loopflow-repository: loopflowstudio/loopflow -->"
        );
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
            .ensure_team("Product", "PRD", "loopflowstudio/loopflow")
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
            .ensure_team("Product", "PRD", "loopflowstudio/loopflow")
            .await
            .expect_err("name reused under a different key is refused");
        assert!(err.to_string().contains("PROD"), "{err}");
    }

    #[tokio::test]
    async fn ensure_team_claims_unmarked_team_without_erasing_human_description() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({ "data": { "teams": { "nodes": [{
                    "id": "team-loo", "name": "Loopflow", "key": "LOO",
                    "description": "Maintainer-owned team notes."
                }] } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "teamUpdate": { "team": { "id": "team-loo" } } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "teams": { "nodes": [{
                    "id": "team-loo", "name": "Loopflow", "key": "LOO",
                    "description": "Maintainer-owned team notes.\n\n<!-- loopflow-repository: loopflowstudio/loopflow -->"
                }] } } }),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        let binding = client
            .ensure_team("Loopflow", "LOO", "loopflowstudio/loopflow")
            .await
            .expect("claim unmarked Team");
        assert_eq!(binding.id, "team-loo");

        let requests = requests.lock().await;
        let update: Value = serde_json::from_str(&requests[1].body).unwrap();
        let description = update["variables"]["description"].as_str().unwrap();
        assert!(description.starts_with("Maintainer-owned team notes."));
        assert!(description.contains("loopflowstudio/loopflow"));
    }

    #[tokio::test]
    async fn ensure_team_refuses_a_foreign_repository_claim_before_mutation() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "teams": { "nodes": [{
                "id": "team-loo", "name": "Loopflow", "key": "LOO",
                "description": "<!-- loopflow-repository: acme/other -->"
            }] } } }),
        )])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        let error = client
            .ensure_team("Loopflow", "LOO", "loopflowstudio/loopflow")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("acme/other"));
        assert!(error.to_string().contains("loopflowstudio/loopflow"));
        assert_eq!(requests.lock().await.len(), 1);
    }

    #[tokio::test]
    async fn configured_team_rebind_is_refused_before_claiming_another_team() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "teams": { "nodes": [{
                "id": "team-loo", "name": "Loopflow", "key": "LOO",
                "description": "Maintainer-owned team notes."
            }] } } }),
        )])
        .await;
        let client = LinearClient::with_base_url("linear-secret".to_string(), None, base_url);

        let error = client
            .claim_configured_team("team-loo", "loopflowstudio/loopflow", None, Some("HOO"))
            .await
            .unwrap_err();

        assert!(error.to_string().contains("already bound"));
        assert!(error.to_string().contains("lf repo reteam"));
        assert_eq!(requests.lock().await.len(), 1);
    }

    #[test]
    fn repository_claim_rejects_ambiguous_markers() {
        let description =
            "<!-- loopflow-repository: acme/one -->\n<!-- loopflow-repository: acme/two -->";
        assert!(repository_claim(description).is_err());
    }

    #[tokio::test]
    async fn issue_ownership_resolves_project_and_team_by_stable_ids() {
        let (base_url, _requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "issue": {
                "id": "issue-uuid", "identifier": "LOO-42", "url": null,
                "title": "Resolve ownership", "description": "",
                "completedAt": null, "prioritySortOrder": 0.0, "sortOrder": 0.0, "updatedAt":"2026-09-29T12:00:00.123Z",
                "assignee": null, "state": { "type": "unstarted" },
                "team": { "id": "team-loo" },
                "project": {
                    "id": "project-api", "name": "Product — Loopflow API",
                    "description": "", "status": {"type":"started"},
                    "content": "## Definition\n\nOne model.\n\n## KRs\n",
                    "initiatives": { "nodes": [{ "id": "initiative-product" }] },
                    "teams": { "nodes": [{ "id": "team-loo" }] }
                }
            } } }),
        )])
        .await;
        let client = LinearClient::with_base_url(
            "linear-secret".to_string(),
            Some("team-loo".to_string()),
            base_url,
        );

        let (item, project) = client.issue_ownership("LOO-42").await.unwrap().unwrap();
        let project = project.unwrap();
        assert_eq!(item.project_id.as_deref(), Some("project-api"));
        assert_eq!(item.team_id, "team-loo");
        assert_eq!(project.initiative_ids, ["initiative-product"]);
        assert_eq!(project.team_ids, ["team-loo"]);
    }

    #[tokio::test]
    async fn complete_item_resolves_state_from_the_issue_team_not_the_wave_team() {
        // The client is bound to the repository Team, but this fixture puts the
        // issue in a different Team. The completed state must be
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
