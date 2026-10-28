//! Linear acquisition and delivery. Work owns local content; this adapter owns provider facts.

pub(crate) mod acceptance;
pub(crate) mod observations;

use crate::work::project::{ProjectKR, ProjectStatus};
use crate::work::wave::metrics::MetricTarget;

use self::project_document::parse_project_document;
mod cooldown;
pub mod request_log;
mod team_changes;

pub(crate) use cooldown::{delta_fresh_for, delta_fresh_for_healthy, urgent};

use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::work::task::comment::{TaskComment, TaskCommentAuthor};

pub(crate) mod project_document;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LinearBindingError {
    #[error("invalid Linear id: {0}")]
    InvalidId(String),
    #[error("planning record has no Linear mapping")]
    NoLinearMapping,
}

macro_rules! validated_string_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = <String as serde::Deserialize>::deserialize(deserializer)?;
                Ok(Self::from_raw(value))
            }
        }

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, LinearBindingError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(LinearBindingError::InvalidId(format!(
                        "{} cannot be empty",
                        $label
                    )));
                }
                Ok(Self(crate::id::canonical_uuid(&value)))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub(crate) fn from_raw(value: impl Into<String>) -> Self {
                Self(crate::id::canonical_uuid(&value.into()))
            }
        }
    };
}

validated_string_id!(LinearIssueId, "Linear issue id");
validated_string_id!(LinearProjectId, "Linear project id");

/// The result of adopting or creating a provider team for a repository. The stable
/// `id` owns identity; `key` is mutable presentation (the Task prefix).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamBinding {
    pub id: String,
    pub key: String,
    pub created: bool,
}

pub fn terminal_reason(state: Option<&str>) -> Option<&'static str> {
    match state {
        Some("canceled") => Some("Linear Task is canceled"),
        Some("duplicate") => Some("Linear Task is duplicate"),
        Some("completed") => Some("Linear Task is complete"),
        _ => None,
    }
}

pub(crate) fn validate_project_ownership(
    wave: &str,
    initiative_id: &str,
    expected_team_id: Option<&str>,
    project: &LinearProject,
) -> LinearResult<()> {
    if project.initiative_ids.as_slice() != [initiative_id] {
        return Err(LinearError::Message(format!(
            "Linear Project `{}` ({}) in wave/{wave} belongs to Initiatives [{}]; expected exactly {initiative_id}",
            project.name,
            project.id,
            project.initiative_ids.join(", ")
        )));
    }
    let [team_id] = project.team_ids.as_slice() else {
        return Err(LinearError::Message(format!(
            "Linear Project `{}` ({}) in wave/{wave} belongs to {} Teams [{}]; expected exactly one repository Team",
            project.name,
            project.id,
            project.team_ids.len(),
            project.team_ids.join(", ")
        )));
    };
    if expected_team_id.is_some_and(|expected| expected != team_id) {
        return Err(LinearError::Message(format!(
            "Linear Project `{}` ({}) in wave/{wave} belongs to Team {team_id}; expected repository Team {}",
            project.name,
            project.id,
            expected_team_id.expect("checked as Some")
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LinearError {
    #[error("Linear rate limit reached; retry after epoch milliseconds {reset_at_ms}")]
    RateLimited { reset_at_ms: i64 },
    #[error("{0}")]
    Message(String),
}

pub type LinearResult<T> = Result<T, LinearError>;

pub fn project_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in name.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character);
            separator = false;
        } else if !slug.is_empty() {
            separator = true;
        }
    }
    slug
}

#[cfg(test)]
pub(crate) mod test_server {
    use std::collections::VecDeque;
    use std::sync::Arc;

    use axum::body::Bytes;
    use axum::extract::State;
    use axum::http::{HeaderMap, Method, StatusCode, Uri};
    use axum::response::Response;
    use axum::routing::any;
    use axum::Router;
    use serde_json::{json, Value};
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CapturedRequest {
        pub method: String,
        pub path: String,
        pub query: Option<String>,
        pub authorization: Option<String>,
        pub body: String,
    }

    #[derive(Debug, Clone)]
    pub struct QueuedResponse {
        pub status: StatusCode,
        pub headers: Vec<(String, String)>,
        pub body: String,
        pub gate: Option<(Arc<tokio::sync::Barrier>, Arc<tokio::sync::Barrier>)>,
    }

    #[derive(Clone)]
    struct ServerState {
        requests: Arc<Mutex<Vec<CapturedRequest>>>,
        responses: Arc<Mutex<VecDeque<QueuedResponse>>>,
        authorization: Option<String>,
    }

    pub async fn spawn(
        responses: Vec<QueuedResponse>,
    ) -> (String, Arc<Mutex<Vec<CapturedRequest>>>) {
        spawn_authorized(responses, None).await
    }

    pub async fn spawn_authorized(
        responses: Vec<QueuedResponse>,
        authorization: Option<String>,
    ) -> (String, Arc<Mutex<Vec<CapturedRequest>>>) {
        let state = ServerState {
            requests: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
            authorization,
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
        State(state): State<ServerState>,
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

        if state.authorization.as_deref().is_some_and(|expected| {
            headers
                .get(reqwest::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                != Some(expected)
        }) {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body("unauthorized".into())
                .expect("build rejection");
        }

        let response = state.responses.lock().await.pop_front().unwrap_or_else(|| {
            json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "errors": [{ "message": "unexpected request" }] }),
            )
        });

        if let Some((entered, release)) = response.gate {
            entered.wait().await;
            release.wait().await;
        }
        let mut builder = Response::builder().status(response.status);
        for (name, value) in response.headers {
            builder = builder.header(name, value);
        }
        builder.body(response.body.into()).expect("build response")
    }

    pub fn json_response(status: StatusCode, body: Value) -> QueuedResponse {
        response(
            status,
            vec![("content-type", "application/json")],
            body.to_string(),
        )
    }

    pub fn response(
        status: StatusCode,
        headers: Vec<(&str, &str)>,
        body: String,
    ) -> QueuedResponse {
        QueuedResponse {
            status,
            gate: None,
            headers: headers
                .into_iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
            body,
        }
    }
}

/// Provider evidence about an issue's state since a delivery's base revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateHistory {
    /// No evidence was read; an uncertain write stays uncertain.
    Unknown,
    /// Only unrelated edits followed the base: an earlier write had no effect.
    Unchanged,
    /// The state left its base: a write landed, or someone moved the issue.
    Changed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LinearIssue {
    /// Provider revision; absent only in historical planning.
    pub revision: Option<String>,
    #[serde(deserialize_with = "crate::id::deserialize_uuid")]
    pub id: String,
    #[serde(deserialize_with = "crate::id::deserialize_task_identifier")]
    pub identifier: String,
    /// Provider branch name used to find existing Git work.
    pub branch_name: Option<String>,
    /// Provider-owned issue URL captured during PM sync. `None` stays explicit
    /// when the provider did not return one; status and roadmap reads never
    /// fetch it on demand.
    pub url: Option<String>,
    pub name: String,
    pub description: String,
    pub rank: u32,
    pub completed: bool,
    pub completed_at: Option<String>,
    pub due_date: Option<String>,
    /// Provider workflow category; absent in historical snapshots.
    pub state: Option<String>,
    /// Stable owning Project id. Task-to-Wave resolution follows this edge.
    pub project_id: Option<String>,
    /// Stable owning repository Team id.
    pub team_id: Option<String>,
    /// Provider user ID of the assignee, if any.
    pub assignee: Option<String>,
}

/// Asana and Notion have no short identifiers; their Tasks keep the local
/// `lf-` identifier Loopflow assigned.
pub(crate) fn local_identifier(id: &str) -> String {
    uuid::Uuid::parse_str(id)
        .map(|uuid| format!("lf-{}", uuid.simple()))
        .unwrap_or_else(|_| id.to_string())
}

impl LinearIssue {
    /// Linear calls the item complete, not canceled or duplicate.
    pub fn is_complete(&self) -> bool {
        match self.state.as_deref() {
            Some(state) => state == "completed",
            None => self.completed,
        }
    }
}

/// A read of one issue's editable content plus its comments, taken to
/// stream Linear edits into a Task. `revision` is the provider's
/// last-updated marker (Linear `updatedAt`), monotonic per issue, and is
/// compared — not trusted as identity — so out-of-order responses never move
/// the cursor backward.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinearIssueObservation {
    pub revision: String,
    pub title: String,
    pub description: String,
    pub comments: Vec<LinearComment>,
}

/// Linear's comment and attribution metadata, retained unchanged in conflict receipts.
/// A provider user or an explicit requester steer supplies person attribution;
/// an integration's display name alone does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LinearComment {
    #[serde(deserialize_with = "crate::id::deserialize_uuid")]
    pub id: String,
    /// Provider creation time; display order. `revision` orders direction.
    pub created_at: Option<String>,
    pub revision: Option<String>,
    pub body: String,
    pub author_id: Option<String>,
    /// Provider display name for attribution, independent of the provider user ID.
    pub author_name: Option<String>,
}

impl From<&LinearComment> for TaskComment {
    fn from(comment: &LinearComment) -> Self {
        let author =
            if comment.author_id.is_some() || crate::ops::linear_observe::is_steer(&comment.body) {
                TaskCommentAuthor::Person {
                    name: crate::ops::linear_observe::comment_requester(
                        &comment.body,
                        comment.author_name.as_deref(),
                    ),
                }
            } else {
                TaskCommentAuthor::Integration
            };
        Self {
            id: comment.id.clone(),
            body: comment.body.clone(),
            author,
            created_at: comment.created_at.clone(),
        }
    }
}

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

const UPDATE_INITIATIVE_MUTATION: &str = r#"mutation UpdateInitiative($id: String!, $input: InitiativeUpdateInput!) {
  initiativeUpdate(id: $id, input: $input) {
    success
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

const SET_PROJECT_TEAMS_MUTATION: &str = r#"mutation SetProjectTeams($id: String!, $teamIds: [String!]!) {
  projectUpdate(id: $id, input: { teamIds: $teamIds }) {
    project {
      id
    }
  }
}"#;

const PROJECT_STATUSES_QUERY: &str = r#"query ProjectStatuses($after: String, $first: Int!) {
  projectStatuses(after: $after, first: $first) {
    nodes { id type teamId position }
    pageInfo { hasNextPage endCursor }
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
        dueDate
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
        project { id }
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

/// An issue's own fields, without its Project.
macro_rules! issue_fields {
    () => {
        r#"    id
    identifier
    branchName
    completedAt
    dueDate
    updatedAt
    url
    title
    description
    prioritySortOrder
    sortOrder
    assignee { id }
    state { type }
    team { id }
"#
    };
}

macro_rules! project_fields {
    () => {
        r#"    id
    name
    updatedAt
    archivedAt
    description
    content
    status { type }
    initiatives(first: 50) { nodes { id } }
    teams(first: 50) { nodes { id } }"#
    };
}

const ISSUE_RESPONSE_FIELDS: &str = concat!(
    issue_fields!(),
    "    project {\n",
    project_fields!(),
    "\n    }"
);
/// Linear scores a query by the page sizes it requests, so a delta that carried
/// each issue's Project connections cost thousands of points even when empty.
/// It names the Project; changed Projects are read once each.
const CHANGED_ISSUE_FIELDS: &str = concat!(issue_fields!(), "    project { id }");
const PROJECT_RESPONSE_FIELDS: &str = project_fields!();

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

const LIST_WORKFLOW_STATES_QUERY: &str = r#"query WorkflowStates($teamId: ID!, $type: String!) {
  workflowStates(filter: { team: { id: { eq: $teamId } }, type: { eq: $type } }) {
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

const VIEWER_QUERY: &str = r#"query Viewer {
  viewer {
    id
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
    repository: Option<std::path::PathBuf>,
}

impl LinearClient {
    pub fn new(token: String, team_id: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
            team_id,
            base_url: LINEAR_BASE_URL.to_string(),
            repository: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_base_url(token: String, team_id: Option<String>, base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
            team_id,
            base_url,
            repository: None,
        }
    }

    pub(crate) fn for_repository(mut self, repo: &std::path::Path) -> Self {
        self.repository = Some(repo.to_owned());
        self
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
    ) -> LinearResult<TeamBinding> {
        let requested_key = key.trim().to_ascii_uppercase();
        if requested_key.is_empty() {
            return Err(LinearError::Message(
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
            return Err(LinearError::Message(format!(
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
            return Err(LinearError::Message(format!(
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

    async fn claim_team(&self, team: &TeamNode, repository: &str) -> LinearResult<TeamBinding> {
        match repository_claim(team.description.as_deref().unwrap_or_default())? {
            Some(claimed) if claimed == repository => {}
            Some(claimed) => {
                return Err(LinearError::Message(format!(
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
    ) -> LinearResult<TeamBinding> {
        let response: TeamsData = self.graphql(LIST_TEAMS_QUERY, json!({})).await?;
        let team = response
            .teams
            .nodes
            .into_iter()
            .find(|team| team.id == team_id)
            .ok_or_else(|| LinearError::Message(format!("no Linear team with id {team_id}")))?;
        if let Some(name) = expected_name.filter(|name| !team.name.eq_ignore_ascii_case(name)) {
            return Err(LinearError::Message(format!(
                "repository {repository} is already bound to Linear Team {} ({}, key {}); \
                 it cannot rebind through --team-name {name:?}. Run repository-wide `lf reteam` instead.",
                team.name, team.id, team.key
            )));
        }
        if let Some(key) = expected_key.filter(|key| !team.key.eq_ignore_ascii_case(key.trim())) {
            return Err(LinearError::Message(format!(
                "repository {repository} is already bound to Linear Team {} ({}, key {}); \
                 it cannot rebind through --team-key {key:?}. Run repository-wide `lf reteam` instead.",
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
    ) -> LinearResult<TeamBinding> {
        let response: TeamsData = self.graphql(LIST_TEAMS_QUERY, json!({})).await?;
        let team = response
            .teams
            .nodes
            .into_iter()
            .find(|team| team.id == team_id)
            .ok_or_else(|| LinearError::Message(format!("no Linear team with id {team_id}")))?;
        match repository_claim(team.description.as_deref().unwrap_or_default())? {
            Some(claimed) if claimed == repository => Ok(TeamBinding {
                id: team.id,
                key: team.key,
                created: false,
            }),
            Some(claimed) => Err(LinearError::Message(format!(
                "Linear team {} ({}, key {}) is claimed by repository {claimed}; \
                 configured repository {repository} must choose another Team",
                team.name, team.id, team.key
            ))),
            None => Err(LinearError::Message(format!(
                "Linear team {} ({}, key {}) has no Loopflow repository claim; \
                 run `lf wave connect --team-key {}` to claim it for {repository}",
                team.name, team.id, team.key, team.key
            ))),
        }
    }

    fn require_team_id(&self) -> LinearResult<String> {
        self.team_id.clone().ok_or_else(|| {
            LinearError::Message(
                "Linear write requires `planning.team` in .lf/config.yaml; \
                 run `lf wave connect <wave> --team-key <KEY>`"
                    .to_string(),
            )
        })
    }

    async fn graphql<T>(&self, query: &str, variables: Value) -> LinearResult<T>
    where
        T: DeserializeOwned,
    {
        if query.trim_start().starts_with("mutation") {
            if let Some(repo) = &self.repository {
                // The delta reader uses an unbound client and performs no writes.
                if let Err(error) =
                    Box::pin(crate::ops::linear_observe::check_before_write(repo)).await
                {
                    tracing::debug!(%error, "pre-write Linear check pending");
                }
            }
        }
        let urgent = cooldown::is_urgent();
        let log = request_log::PendingRequest::start(
            &cooldown::home(&self.base_url),
            query,
            &variables,
            self.repository.as_deref(),
            urgent,
        );
        let finish = |log: Option<request_log::PendingRequest>,
                      outcome: request_log::RequestOutcome,
                      status: Option<StatusCode>,
                      allowances: cooldown::Allowances| {
            if let Some(log) = log {
                log.finish(outcome, status.map(|status| status.as_u16()), allowances);
            }
        };
        let request = GraphqlRequest { query, variables };

        let cooldown = cooldown::Cooldown::for_endpoint(&self.base_url);
        if let Err(error) = cooldown.check().await {
            finish(
                log,
                request_log::RequestOutcome::Paused,
                None,
                Default::default(),
            );
            return Err(error);
        }
        if !urgent {
            if let Err(error) = cooldown.admit().await {
                finish(
                    log,
                    request_log::RequestOutcome::Deferred,
                    None,
                    Default::default(),
                );
                return Err(error);
            }
        }
        let sent = self
            .client
            .post(&self.base_url)
            .bearer_auth(&self.token)
            .json(&request)
            .send()
            .await;
        let response = match sent {
            Ok(response) => response,
            Err(err) => {
                finish(
                    log,
                    request_log::RequestOutcome::Failed,
                    None,
                    Default::default(),
                );
                return Err(LinearError::Message(format!(
                    "linear request failed: {err}"
                )));
            }
        };
        let status = response.status();
        let headers = response.headers().clone();
        let allowances = cooldown::Allowances::from_headers(&headers);
        let body = match response.bytes().await {
            Ok(body) => body,
            Err(err) => {
                finish(
                    log,
                    request_log::RequestOutcome::Failed,
                    Some(status),
                    allowances,
                );
                return Err(LinearError::Message(format!(
                    "failed to read Linear response: {err}"
                )));
            }
        };
        let observed = cooldown.observe(status, &headers, &body).await;
        let outcome = match &observed {
            Err(LinearError::RateLimited { .. }) => request_log::RequestOutcome::RateLimited,
            _ => request_log::RequestOutcome::Sent,
        };
        finish(log, outcome, Some(status), allowances);
        observed?;
        parse_graphql_response(status, &body)
    }

    /// The owning team id of an existing issue. State transitions resolve
    /// against this, not the wave-configured team, because a Project can span
    /// teams and Linear rejects a state that belongs to another team.
    async fn item_team_id(&self, item_id: &str) -> LinearResult<String> {
        let response: IssueTeamData = self
            .graphql(ISSUE_TEAM_QUERY, json!({ "id": item_id }))
            .await?;
        response
            .issue
            .map(|issue| issue.team.id)
            .ok_or_else(|| LinearError::Message(format!("no Linear issue with id {item_id}")))
    }

    /// Resolve before reserving a write: failed reads leave delivery retryable.
    pub(crate) async fn item_state_id(&self, item_id: &str, target: &str) -> LinearResult<String> {
        if !matches!(target, "completed" | "started" | "unstarted" | "canceled") {
            return Err(LinearError::Message(format!(
                "unsupported Task state {target}"
            )));
        }
        let team_id = self.item_team_id(item_id).await?;
        self.team_state_id(&team_id, target).await
    }

    pub(crate) async fn team_state_id(&self, team_id: &str, target: &str) -> LinearResult<String> {
        let response: WorkflowStatesData = self
            .graphql(
                LIST_WORKFLOW_STATES_QUERY,
                json!({ "teamId": team_id, "type": target }),
            )
            .await?;
        response
            .workflow_states
            .nodes
            .into_iter()
            .min_by(|left, right| left.position.total_cmp(&right.position))
            .map(|state| state.id)
            .ok_or_else(|| {
                LinearError::Message(format!(
                    "no {target} Linear workflow state found for team {team_id}"
                ))
            })
    }

    pub async fn create_wave(&self, name: &str, summary: &str) -> LinearResult<String> {
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

    pub async fn rename_wave(&self, initiative_id: &str, name: &str) -> LinearResult<()> {
        self.update_wave(initiative_id, Some(name), None).await
    }

    pub async fn update_wave(
        &self,
        initiative_id: &str,
        name: Option<&str>,
        summary: Option<&str>,
    ) -> LinearResult<()> {
        let mut input = serde_json::Map::new();
        if let Some(name) = name {
            input.insert("name".into(), json!(name));
        }
        if let Some(summary) = summary {
            input.insert("description".into(), json!(linear_description(summary)));
        }
        let response: Value = self
            .graphql(
                UPDATE_INITIATIVE_MUTATION,
                json!({"id": initiative_id, "input": input}),
            )
            .await?;
        if response["initiativeUpdate"]["success"] != true {
            return Err(LinearError::Message(format!(
                "Linear did not confirm update of Initiative {initiative_id}"
            )));
        }
        Ok(())
    }

    pub(crate) async fn initiatives(&self) -> LinearResult<Vec<InitiativeNode>> {
        let mut after = None;
        let mut initiatives = Vec::new();
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
            initiatives.extend(page.nodes);
            if !page.page_info.has_next_page {
                return Ok(initiatives);
            }
            after = page.page_info.end_cursor;
        }
    }

    async fn project_status_id(
        &self,
        status: crate::work::project::ProjectStatus,
    ) -> LinearResult<String> {
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
                LinearError::Message(format!(
                    "no {} Project status for team {team}",
                    status.as_str()
                ))
            })
    }

    async fn project_statuses(&self) -> LinearResult<Vec<ProjectStatusChoice>> {
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
                LinearError::Message("Linear Project statuses have no next-page cursor".into())
            })?;
            if after.as_ref() == Some(&cursor) {
                return Err(LinearError::Message(
                    "Linear Project statuses repeated a page cursor".into(),
                ));
            }
            after = Some(cursor);
        }
    }

    pub(crate) async fn list_projects(
        &self,
        initiative_id: &str,
    ) -> LinearResult<Vec<LinearProject>> {
        self.list_projects_including_archived(initiative_id, false)
            .await
    }

    pub(crate) async fn list_projects_including_archived(
        &self,
        initiative_id: &str,
        include_archived: bool,
    ) -> LinearResult<Vec<LinearProject>> {
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
                    .map(ProjectNode::into_observation)
                    .collect::<LinearResult<Vec<_>>>()?,
            );
            if !page.page_info.has_next_page {
                return Ok(projects);
            }
            after = page.page_info.end_cursor;
        }
    }

    pub(crate) async fn ordered_items(&self, project: &str) -> LinearResult<Vec<OrderedIssue>> {
        self.list_issue_nodes(project, false)
            .await?
            .into_iter()
            .enumerate()
            .map(|(rank, issue)| {
                let priority_sort_order = issue.fields.priority_sort_order;
                let sort_order = issue.fields.sort_order;
                Ok(OrderedIssue {
                    item: issue.into_issue(rank as u32)?,
                    priority_sort_order,
                    sort_order,
                })
            })
            .collect()
    }

    pub(crate) async fn list_project_issues(
        &self,
        projects: &[LinearProject],
    ) -> LinearResult<Vec<LinearIssue>> {
        let lists = futures_util::future::try_join_all(
            projects.iter().map(|project| self.list_items(&project.id)),
        )
        .await?;
        Ok(lists.into_iter().flatten().collect())
    }

    pub(crate) async fn list_items(&self, project_id: &str) -> LinearResult<Vec<LinearIssue>> {
        self.list_items_including_archived(project_id, false).await
    }

    pub(crate) async fn list_items_including_archived(
        &self,
        project_id: &str,
        include_archived: bool,
    ) -> LinearResult<Vec<LinearIssue>> {
        self.list_issue_nodes(project_id, include_archived)
            .await?
            .into_iter()
            .enumerate()
            .map(|(rank, issue)| issue.into_issue(rank as u32))
            .collect()
    }

    async fn list_issue_nodes(
        &self,
        project_id: &str,
        include_archived: bool,
    ) -> LinearResult<Vec<IssueNode>> {
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

    pub(crate) async fn ensure_follow_up_relation(
        &self,
        source: &str,
        target: &str,
        id: &str,
    ) -> LinearResult<()> {
        if self.follow_up_relation_exists(source, target).await? {
            return Ok(());
        }
        let result: LinearResult<serde_json::Value> = self.graphql(
            "mutation FollowUpRelation($input: IssueRelationCreateInput!) { issueRelationCreate(input: $input) { success issueRelation { id } } }",
            json!({"input": {"id": id, "issueId": source, "relatedIssueId": target, "type": "related"}}),
        ).await;
        let response = result?;
        if response["issueRelationCreate"]["success"] == true
            && response["issueRelationCreate"]["issueRelation"]["id"] == id
        {
            return Ok(());
        }
        Err(LinearError::Message(
            "Linear did not confirm the follow-up relation".into(),
        ))
    }

    async fn follow_up_relation_exists(&self, source: &str, target: &str) -> LinearResult<bool> {
        for (issue, related) in [(source, target), (target, source)] {
            let mut after: Option<String> = None;
            loop {
                let data: serde_json::Value = self.graphql(
                    "query FollowUpRelationExists($id: String!, $after: String) { issue(id: $id) { relations(first: 100, after: $after) { nodes { type relatedIssue { id } } pageInfo { hasNextPage endCursor } } } }",
                    json!({"id": issue, "after": after}),
                ).await?;
                let page = data.pointer("/issue/relations").ok_or_else(|| {
                    LinearError::Message("Linear omitted relation lookup results".into())
                })?;
                let nodes = page["nodes"]
                    .as_array()
                    .ok_or_else(|| LinearError::Message("Linear omitted relation nodes".into()))?;
                if nodes
                    .iter()
                    .any(|node| node["type"] == "related" && node["relatedIssue"]["id"] == related)
                {
                    return Ok(true);
                }
                match page["pageInfo"]["hasNextPage"].as_bool() {
                    Some(false) => break,
                    Some(true) => {
                        after = Some(
                            page["pageInfo"]["endCursor"]
                                .as_str()
                                .ok_or_else(|| {
                                    LinearError::Message("Linear omitted relation cursor".into())
                                })?
                                .into(),
                        )
                    }
                    None => {
                        return Err(LinearError::Message(
                            "Linear omitted relation pagination".into(),
                        ))
                    }
                }
            }
        }
        Ok(false)
    }

    pub async fn move_item_to_team(&self, item_id: &str, team_id: &str) -> LinearResult<String> {
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
    pub async fn set_project_teams(
        &self,
        project_id: &str,
        team_ids: &[String],
    ) -> LinearResult<()> {
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
    pub(crate) async fn issue_ownership(
        &self,
        issue_id: &str,
    ) -> LinearResult<Option<(LinearIssue, Option<LinearProject>)>> {
        let response: IssueOwnershipData = self
            .graphql(
                &format!("query IssueOwnership($id: String!) {{ issue(id: $id) {{ {ISSUE_RESPONSE_FIELDS} }} }}"),
                json!({ "id": issue_id }),
            )
            .await?;
        response.issue.map(IssueNode::into_ownership).transpose()
    }

    /// Whether the issue's state left `base_state` after `base_revision`.
    /// Linear's history has no filter or sort direction, so the client pages it;
    /// an unread remainder is unknown, never unchanged.
    pub(crate) async fn state_history_since(
        &self,
        issue_id: &str,
        base_revision: &str,
        base_state: &str,
    ) -> LinearResult<StateHistory> {
        #[derive(Deserialize)]
        struct Data {
            issue: Option<Issue>,
        }
        #[derive(Deserialize)]
        struct Issue {
            history: Connection<Entry>,
        }
        #[derive(Deserialize)]
        struct Entry {
            #[serde(rename = "updatedAt")]
            revision: String,
            #[serde(rename = "toState")]
            to_state: Option<State>,
        }
        #[derive(Deserialize)]
        struct State {
            r#type: String,
        }
        let nanos = |revision: &str| {
            crate::linear::observations::revision_nanos(Some(revision))
                .map_err(|error| LinearError::Message(error.to_string()))
        };
        let base = nanos(base_revision)?;
        let mut after: Option<String> = None;
        for _ in 0..10 {
            let data: Data = self
                .graphql(
                    r#"query IssueStateHistory($id: String!, $after: String) {
                issue(id: $id) { history(first: 50, after: $after) {
                    nodes { updatedAt toState { type } }
                    pageInfo { hasNextPage endCursor }
                } }
            }"#,
                    json!({"id": issue_id, "after": after}),
                )
                .await?;
            let Some(issue) = data.issue else {
                return Ok(StateHistory::Unknown);
            };
            for entry in issue.history.nodes {
                if nanos(&entry.revision)? > base
                    && entry
                        .to_state
                        .is_some_and(|state| !state.r#type.eq_ignore_ascii_case(base_state))
                {
                    return Ok(StateHistory::Changed);
                }
            }
            let page = issue.history.page_info;
            match page.end_cursor {
                Some(cursor) if page.has_next_page => after = Some(cursor),
                _ => return Ok(StateHistory::Unchanged),
            }
        }
        Ok(StateHistory::Unknown)
    }

    pub(crate) async fn issue_trash(&self, id: &str) -> LinearResult<Option<(bool, String)>> {
        #[derive(Deserialize)]
        struct Data {
            #[serde(deserialize_with = "Option::deserialize")]
            issue: Option<Issue>,
        }
        #[derive(Deserialize)]
        struct Issue {
            id: String,
            #[serde(rename = "updatedAt")]
            revision: String,
            #[serde(deserialize_with = "Option::deserialize")]
            trashed: Option<bool>,
        }
        let data: Data = self
            .graphql(
                r#"query IssueTrash($id: String!) {
                issue(id: $id) { id updatedAt trashed }
            }"#,
                json!({"id":id}),
            )
            .await?;
        data.issue
            .map(|issue| {
                if issue.id != id {
                    return Err(LinearError::Message(
                        "Linear trash observation returned a different issue".into(),
                    ));
                }
                Ok((issue.trashed == Some(true), issue.revision))
            })
            .transpose()
    }

    pub(crate) async fn delete_issue(&self, id: &str) -> LinearResult<()> {
        #[derive(Deserialize)]
        struct Data {
            #[serde(rename = "issueDelete")]
            deletion: Payload,
        }
        #[derive(Deserialize)]
        struct Payload {
            success: bool,
        }
        let data: Data = self
            .graphql(
                r#"mutation DeliverTaskDeletion($id: String!) {
                issueDelete(id: $id) { success }
            }"#,
                json!({"id":id}),
            )
            .await?;
        if !data.deletion.success {
            return Err(LinearError::Message(
                "Linear deletion was not acknowledged; receipt retained".into(),
            ));
        }
        Ok(())
    }

    pub(crate) async fn find_project(
        &self,
        project_id: &str,
    ) -> LinearResult<Option<LinearProject>> {
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
                    return Err(LinearError::Message(format!(
                        "Project {project_id} is archived; reconcile its status in Linear"
                    )));
                }
                project.into_observation()
            })
            .transpose()
    }

    pub(crate) async fn project_ownership(&self, project_id: &str) -> LinearResult<LinearProject> {
        self.project_node(project_id).await?.into_observation()
    }

    pub(crate) async fn project_content_observation(
        &self,
        id: &str,
    ) -> LinearResult<(LinearProject, String)> {
        let node = self.project_node(id).await?;
        let content = node.content.clone().unwrap_or_default();
        Ok((node.into_observation()?, content))
    }

    pub(crate) async fn project_field_input(
        &self,
        value: &crate::work::edit::WorkValue,
        content: &str,
    ) -> LinearResult<Value> {
        use crate::work::edit::WorkValue;
        match value {
            WorkValue::ProjectName(value) => Ok(json!({"name": value})),
            WorkValue::ProjectSummary(value) => Ok(json!({"description": value})),
            WorkValue::ProjectStatus(value) => {
                Ok(json!({"statusId": self.project_status_id(*value).await?}))
            }
            WorkValue::ProjectDescription(_) | WorkValue::Krs(_) | WorkValue::MetricTargets(_) => {
                Ok(json!({"content": patch_project_field(content, value)?}))
            }
            _ => Err(LinearError::Message(format!(
                "unsupported Project field {}",
                value.field()
            ))),
        }
    }

    pub(crate) async fn find_export_issue(
        &self,
        id: &str,
    ) -> LinearResult<Option<(LinearIssue, Option<LinearProject>)>> {
        let response: Value = self
            .graphql(
                r#"query FindExportIssue($id: ID!) {
            issues(filter: { id: { eq: $id } }, first: 2, includeArchived: true) { nodes { id } }
        }"#,
                json!({"id":id}),
            )
            .await?;
        let nodes = response["issues"]["nodes"]
            .as_array()
            .ok_or_else(|| LinearError::Message("Issue lookup is incomplete".into()))?;
        if nodes.is_empty() {
            return Ok(None);
        }
        if nodes.len() != 1 || nodes[0]["id"] != id {
            return Err(LinearError::Message(
                "Issue lookup returned a different identity".into(),
            ));
        }
        self.issue_ownership(id).await
    }

    pub(crate) async fn deliver_planning_creation(
        &self,
        project: bool,
        input: Value,
    ) -> LinearResult<PlanningResponse> {
        let (name, operation, input_type, object, fields) = if project {
            (
                "DeliverProjectCreation",
                "projectCreate",
                "ProjectCreateInput",
                "project",
                PROJECT_RESPONSE_FIELDS,
            )
        } else {
            (
                "DeliverTaskCreation",
                "issueCreate",
                "IssueCreateInput",
                "issue",
                ISSUE_RESPONSE_FIELDS,
            )
        };
        let query = format!("mutation {name}($input: {input_type}!) {{ {operation}(input: $input) {{ success {object} {{ {fields} }} }} }}");
        let response: Value = self.graphql(&query, json!({"input":input})).await?;
        planning_response(
            &response[operation],
            project,
            input["id"]
                .as_str()
                .ok_or_else(|| LinearError::Message("creation identity is missing".into()))?,
        )
    }

    pub(crate) async fn deliver_project_attachment(
        &self,
        id: &str,
        project: &str,
        initiative: &str,
    ) -> LinearResult<LinearProject> {
        let query = format!("mutation DeliverProjectAttachment($input: InitiativeToProjectCreateInput!) {{ initiativeToProjectCreate(input: $input) {{ success initiativeToProject {{ project {{ {PROJECT_RESPONSE_FIELDS} }} }} }} }}");
        let response: Value = self
            .graphql(
                &query,
                json!({"input":{"id":id,"projectId":project,"initiativeId":initiative}}),
            )
            .await?;
        let payload = &response["initiativeToProjectCreate"];
        if payload["success"] != true {
            return Err(LinearError::Message(
                "Project attachment was not confirmed; receipt retained".into(),
            ));
        }
        let node: ProjectNode = serde_json::from_value(
            payload["initiativeToProject"]["project"].clone(),
        )
        .map_err(|error| LinearError::Message(format!("invalid attached Project: {error}")))?;
        if node.id != project {
            return Err(LinearError::Message(
                "Project attachment returned a different identity".into(),
            ));
        }
        node.into_observation()
    }

    /// The mutation's own response supplies confirmation; no follow-up read.
    pub(crate) async fn deliver_planning_field(
        &self,
        id: &str,
        project: bool,
        input: Value,
    ) -> LinearResult<PlanningResponse> {
        let (name, operation, input_type, object, fields) = if project {
            (
                "DeliverProjectField",
                "projectUpdate",
                "ProjectUpdateInput",
                "project",
                PROJECT_RESPONSE_FIELDS,
            )
        } else {
            (
                "DeliverTaskField",
                "issueUpdate",
                "IssueUpdateInput",
                "issue",
                ISSUE_RESPONSE_FIELDS,
            )
        };
        let query = format!("mutation {name}($id: String!, $input: {input_type}!) {{ {operation}(id: $id,input: $input) {{ success {object} {{ {fields} }} }} }}");
        let response: Value = self.graphql(&query, json!({"id":id,"input":input})).await?;
        planning_response(&response[operation], project, id)
    }

    // Adoption resolves Project status and membership. Historical workflow prose
    // grants no selection authority and is left intact.
    pub(crate) async fn adopt_project(
        &self,
        project_id: &str,
        initiative: &str,
        team: &str,
        promote: bool,
        apply: bool,
    ) -> LinearResult<LinearProject> {
        let mut node = self.project_node(project_id).await?;
        let promote = promote
            && node.archived_at.is_none()
            && matches!(
                node.status.type_,
                crate::work::project::ProjectStatus::Backlog
                    | crate::work::project::ProjectStatus::Planned
            );
        if promote {
            node.status.type_ = crate::work::project::ProjectStatus::Started;
        }
        let mut project = node.into_observation()?;
        crate::linear::validate_project_ownership("adoption", initiative, Some(team), &project)?;
        if apply && promote {
            let input = json!({
                "statusId": self.project_status_id(crate::work::project::ProjectStatus::Started).await?
            });
            let response: Value = self
                .graphql(
                    &format!("mutation AdoptProject($id: String!, $input: ProjectUpdateInput!) {{ projectUpdate(id: $id, input: $input) {{ success project {{ {PROJECT_RESPONSE_FIELDS} }} }} }}"),
                    json!({"id":project_id,"input":input}),
                )
                .await?;
            let PlanningResponse::Project {
                project: confirmed, ..
            } = planning_response(&response["projectUpdate"], true, project_id)?
            else {
                unreachable!("Project adoption returns a Project");
            };
            project.revision.clone_from(&confirmed.revision);
            if *confirmed != project {
                return Err(LinearError::Message(format!(
                    "Project {project_id} changed during adoption; refresh and retry"
                )));
            }
        }
        Ok(project)
    }

    async fn project_node(&self, project_id: &str) -> LinearResult<ProjectNode> {
        let response: ProjectOwnershipData = self
            .graphql(
                &format!("query ProjectOwnership($id: String!) {{ project(id: $id) {{ {PROJECT_RESPONSE_FIELDS} }} }}"),
                json!({ "id": project_id }),
            )
            .await?;
        response
            .project
            .ok_or_else(|| LinearError::Message(format!("no Linear Project with id {project_id}")))
    }

    pub(crate) async fn set_item_state(
        &self,
        item_id: &str,
        state_id: &str,
    ) -> LinearResult<LinearIssue> {
        let query = format!("mutation SetIssueState($id: String!, $stateId: String!) {{ issueUpdate(id: $id,input: {{stateId: $stateId}}) {{ success issue {{ {ISSUE_RESPONSE_FIELDS} }} }} }}");
        let response: Value = self
            .graphql(&query, json!({"id":item_id,"stateId":state_id}))
            .await?;
        match planning_response(&response["issueUpdate"], false, item_id)? {
            PlanningResponse::Task { issue, .. } => Ok(issue.item),
            PlanningResponse::Project { .. } => unreachable!("issue mutation returns an issue"),
        }
    }

    /// Post a new comment and return its Linear id so callers can update it in
    /// place later instead of posting a duplicate.
    pub async fn comment(&self, item_id: &str, body: &str) -> LinearResult<String> {
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

    /// Deliver the caller's UUID. An uncertain reply is resolved by exact identity.
    pub async fn sync_comment(&self, id: &str, issue: &str, body: &str) -> LinearResult<()> {
        let id = comment_delivery_id(id)?;
        let id = id.as_str();
        let response: LinearResult<CommentData> = self
            .graphql(
                r#"mutation SyncComment($id: String!, $issueId: String!, $body: String!) {
                commentCreate(input: { id: $id, issueId: $issueId, body: $body }) {
                    comment { id }
                }
            }"#,
                json!({"id": id, "issueId": issue, "body": body}),
            )
            .await;
        match response {
            Ok(response) if response.comment_create.comment.id == id => Ok(()),
            result => {
                let read: LinearResult<Value> = self
                    .graphql(
                        r#"query CommentDelivery($id: String!) {
                        comment(id: $id) { id body issue { id } }
                    }"#,
                        json!({"id": id}),
                    )
                    .await;
                let read = match read {
                    Ok(read) => read,
                    Err(error) => {
                        return Err(LinearError::Message(format!(
                            "Comment {id} is pending sync: {}; confirmation unavailable: {error}",
                            match result {
                                Err(error) => error.to_string(),
                                Ok(_) => "creation returned a different identity".into(),
                            }
                        )))
                    }
                };
                let comment = &read["comment"];
                if comment["id"].as_str() == Some(id)
                    && comment["body"].as_str() == Some(body)
                    && comment["issue"]["id"].as_str() == Some(issue)
                {
                    return Ok(());
                }
                Err(LinearError::Message(match result {
                    Err(error) => format!("Comment {id} is pending sync: {error}"),
                    Ok(_) => format!("Comment {id} was not confirmed with its saved identity"),
                }))
            }
        }
    }

    pub async fn update_comment(&self, comment_id: &str, body: &str) -> LinearResult<()> {
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

    pub(crate) async fn item_attachment_urls(&self, item_id: &str) -> LinearResult<Vec<String>> {
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
                    LinearError::Message(format!("issue {item_id} attachments are unavailable"))
                })?
                .attachments;
            urls.extend(page.nodes.into_iter().map(|node| node.url));
            if !page.page_info.has_next_page {
                return Ok(urls);
            }
            let cursor = page.page_info.end_cursor.ok_or_else(|| {
                LinearError::Message("attachment pagination omitted its continuation".into())
            })?;
            if after.as_ref() == Some(&cursor) {
                return Err(LinearError::Message(
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
    ) -> LinearResult<String> {
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
    ) -> LinearResult<()> {
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

    /// Identify the authenticated Linear user for machine login transfers.
    pub async fn viewer_id(&self) -> LinearResult<String> {
        let response: ViewerData = self.graphql(VIEWER_QUERY, json!({})).await?;
        Ok(response.viewer.id)
    }

    /// Read one issue's title, description, comments, and revision marker.
    pub(crate) async fn observe_issue(
        &self,
        issue_id: &str,
    ) -> LinearResult<LinearIssueObservation> {
        let response: IssueObservationData = self
            .graphql(
                ISSUE_OBSERVATION_QUERY,
                json!({ "id": issue_id, "comments": OBSERVATION_COMMENT_PAGE }),
            )
            .await?;
        let issue = response
            .issue
            .ok_or_else(|| LinearError::Message(format!("linear issue {issue_id} not found")))?;
        let mut page = issue.comments;
        let mut comments = Vec::new();
        loop {
            comments.extend(page.nodes.into_iter().map(LinearComment::from));
            if !page.page_info.has_next_page {
                break;
            }
            let after = page.page_info.end_cursor.ok_or_else(|| {
                LinearError::Message(
                    "Linear comment page is missing its continuation cursor".into(),
                )
            })?;
            let response: IssueCommentsData = self
                .graphql(
                    ISSUE_COMMENTS_QUERY,
                    json!({"id": issue_id, "comments": OBSERVATION_COMMENT_PAGE, "after": after}),
                )
                .await?;
            page = response
                .issue
                .ok_or_else(|| LinearError::Message(format!("linear issue {issue_id} not found")))?
                .comments;
        }
        // A correction to an older comment follows the original direction.
        comments.sort_by(|left, right| {
            left.revision
                .cmp(&right.revision)
                .then(left.id.cmp(&right.id))
        });
        Ok(LinearIssueObservation {
            revision: issue.updated_at,
            title: issue.title,
            description: issue.description.unwrap_or_default(),
            comments,
        })
    }
}

#[derive(Debug)]
pub(crate) enum PlanningResponse {
    Task {
        issue: Box<OrderedIssue>,
        project: Option<Box<LinearProject>>,
    },
    Project {
        project: Box<LinearProject>,
        content: String,
    },
}

/// Historical filing comments use name-derived UUIDs. Linear accepts the v4
/// version bits only; keep the same 122 identity bits and retain the local receipt.
pub(crate) fn comment_delivery_id(id: &str) -> LinearResult<String> {
    let uuid =
        uuid::Uuid::parse_str(id).map_err(|error| LinearError::Message(error.to_string()))?;
    if uuid.get_version_num() != 5 {
        return Ok(uuid.to_string());
    }
    let mut bytes = *uuid.as_bytes();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    Ok(uuid::Uuid::from_bytes(bytes).to_string())
}

fn planning_response(payload: &Value, project: bool, id: &str) -> LinearResult<PlanningResponse> {
    let object = if project { "project" } else { "issue" };
    if payload["success"] != true || payload[object]["id"] != id {
        return Err(LinearError::Message(
            "Linear mutation lacks an exact acknowledgement; receipt retained".into(),
        ));
    }
    if project {
        let node: ProjectNode = serde_json::from_value(payload[object].clone())
            .map_err(|error| LinearError::Message(error.to_string()))?;
        let content = node.content.clone().unwrap_or_default();
        Ok(PlanningResponse::Project {
            project: Box::new(node.into_observation()?),
            content,
        })
    } else {
        let node: IssueNode<ProjectNode> = serde_json::from_value(payload[object].clone())
            .map_err(|error| LinearError::Message(error.to_string()))?;
        let priority_sort_order = node.fields.priority_sort_order;
        let sort_order = node.fields.sort_order;
        let (item, project) = node.into_ownership()?;
        Ok(PlanningResponse::Task {
            issue: Box::new(OrderedIssue {
                item,
                priority_sort_order,
                sort_order,
            }),
            project: project.map(Box::new),
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
struct InitiativeCreateData {
    #[serde(rename = "initiativeCreate")]
    initiative_create: InitiativePayload,
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
    #[serde(deserialize_with = "crate::id::deserialize_task_identifier")]
    identifier: String,
}

#[derive(Deserialize)]
struct InitiativePayload {
    initiative: IdNode,
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
    #[serde(deserialize_with = "crate::id::deserialize_uuid")]
    id: String,
}

#[derive(Deserialize)]
struct ViewerData {
    viewer: IdNode,
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
    comments: Connection<CommentNode>,
}

#[derive(Deserialize)]
struct IssueCommentsData {
    issue: Option<IssueCommentsNode>,
}

#[derive(Deserialize)]
struct IssueCommentsNode {
    comments: Connection<CommentNode>,
}

#[derive(Deserialize)]
struct CommentNode {
    #[serde(deserialize_with = "crate::id::deserialize_uuid")]
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

impl From<CommentNode> for LinearComment {
    fn from(node: CommentNode) -> Self {
        Self {
            id: node.id,
            created_at: node.created_at,
            revision: node.updated_at,
            body: node.body,
            author_name: node.user.as_ref().and_then(|user| {
                user.display_name
                    .as_deref()
                    .and_then(crate::config::normalize_user_name)
                    .or_else(|| {
                        user.name
                            .as_deref()
                            .and_then(crate::config::normalize_user_name)
                    })
            }),
            author_id: node.user.map(|user| user.id),
        }
    }
}

#[derive(Deserialize)]
struct CommentUser {
    #[serde(deserialize_with = "crate::id::deserialize_uuid")]
    id: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(bound(deserialize = "P: Deserialize<'de>"))]
struct IssueNode<P = IdNode> {
    #[serde(flatten)]
    fields: IssueFields,
    #[serde(deserialize_with = "Option::deserialize")]
    project: Option<P>,
}

#[derive(Debug)]
pub(crate) struct OrderedIssue {
    pub item: LinearIssue,
    pub priority_sort_order: f64,
    pub sort_order: f64,
}

// List and detail observations require the same complete issue fields.
// Nullable fields must be present; omission is not a value to store.
#[derive(Deserialize)]
struct IssueFields {
    #[serde(rename = "completedAt", deserialize_with = "Option::deserialize")]
    completed_at: Option<String>,
    #[serde(rename = "dueDate", deserialize_with = "Option::deserialize")]
    due_date: Option<String>,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    #[serde(deserialize_with = "crate::id::deserialize_uuid")]
    id: String,
    #[serde(deserialize_with = "crate::id::deserialize_task_identifier")]
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
    fn into_issue(self, rank: u32) -> LinearResult<LinearIssue> {
        self.fields
            .into_issue(rank, self.project.map(|project| project.id))
    }
}

impl IssueFields {
    fn into_issue(self, rank: u32, project_id: Option<String>) -> LinearResult<LinearIssue> {
        if let Some(date) = &self.completed_at {
            time::OffsetDateTime::parse(date, &time::format_description::well_known::Rfc3339)
                .map_err(|error| {
                    LinearError::Message(format!("invalid Linear completion time: {error}"))
                })?;
        }
        if let Some(date) = &self.due_date {
            let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")
                .expect("constant date format is valid");
            time::Date::parse(date, &format).map_err(|error| {
                LinearError::Message(format!("invalid Linear due date: {error}"))
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
        let team = self.team.ok_or_else(|| {
            LinearError::Message(format!("Linear issue {identifier} has no Team"))
        })?;
        // Validate revision precision before admitting provider facts.
        time::OffsetDateTime::parse(
            &self.updated_at,
            &time::format_description::well_known::Rfc3339,
        )
        .map_err(|error| LinearError::Message(format!("invalid Linear issue revision: {error}")))?;
        Ok(LinearIssue {
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
            due_date: self.due_date,
            state: self.state.map(|state| state.r#type),
            project_id,
            team_id: Some(team.id),
            assignee: self.assignee.map(|assignee| assignee.id),
        })
    }
}

#[derive(Deserialize)]
struct IssueOwnershipData {
    #[serde(deserialize_with = "Option::deserialize")]
    issue: Option<IssueNode<ProjectNode>>,
}

impl IssueNode<ProjectNode> {
    fn into_ownership(self) -> LinearResult<(LinearIssue, Option<LinearProject>)> {
        let item = self
            .fields
            .into_issue(0, self.project.as_ref().map(|project| project.id.clone()))?;
        Ok((
            item,
            self.project
                .map(ProjectNode::into_observation)
                .transpose()?,
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
    issues: Connection<IssueNode>,
}

#[derive(Deserialize)]
struct Connection<T> {
    nodes: Vec<T>,
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
    #[serde(deserialize_with = "crate::id::deserialize_uuid")]
    id: String,
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
    #[serde(deserialize_with = "crate::id::deserialize_uuid")]
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
    type_: crate::work::project::ProjectStatus,
}

#[derive(Deserialize)]
struct ProjectStatusChoice {
    #[serde(deserialize_with = "crate::id::deserialize_uuid")]
    id: String,
    #[serde(rename = "type")]
    type_: crate::work::project::ProjectStatus,
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
    #[serde(deserialize_with = "crate::id::deserialize_uuid")]
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

// Replace only the selected planning field, retaining unrelated provider prose.
fn patch_project_field(
    content: &str,
    value: &crate::work::edit::WorkValue,
) -> LinearResult<String> {
    use crate::work::edit::WorkValue;
    use project_document::ProjectField::{Krs, Targets};
    let document = parse_project_document(content)?;
    let (owner, replacement) = match value {
        WorkValue::ProjectDescription(Some(description)) => {
            return document.replace_description(description)
        }
        WorkValue::Krs(krs) => (Krs, project_document::render_krs(krs)),
        WorkValue::MetricTargets(targets) => {
            crate::work::project::validate_metric_targets(targets).map_err(LinearError::Message)?;
            (Targets, project_document::render_metric_targets(targets))
        }
        _ => {
            return Err(LinearError::Message(format!(
                "unsupported content field {}",
                value.field()
            )))
        }
    };
    Ok(document.rewrite(owner, &format!("{replacement}\n\n")))
}

impl ProjectNode {
    fn into_observation(self) -> LinearResult<LinearProject> {
        let content = parse_project_document(self.content.as_deref().unwrap_or_default())?;
        Ok(LinearProject {
            description: Some(content.observed_description()),
            id: self.id,
            revision: self.revision,
            slug: project_slug(&self.name),
            name: self.name,
            summary: self.description.unwrap_or_default(),

            metric_targets: content.metric_targets,
            // Older rotation archived its predecessor without changing status.
            // Retain that history without presenting it as a current plan.
            status: if self.archived_at.is_some()
                && self.status.type_ != crate::work::project::ProjectStatus::Canceled
            {
                crate::work::project::ProjectStatus::Completed
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
    initiatives: Connection<InitiativeNode>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct InitiativeNode {
    #[serde(deserialize_with = "crate::id::deserialize_uuid")]
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Deserialize)]
struct ProjectsData {
    projects: Connection<ProjectNode>,
}

#[derive(Deserialize)]
struct InitiativeProjectsData {
    initiative: InitiativeWithProjects,
}

#[derive(Deserialize)]
struct InitiativeWithProjects {
    projects: Connection<ProjectNode>,
}

fn parse_graphql_response<T: DeserializeOwned>(status: StatusCode, body: &[u8]) -> LinearResult<T> {
    let parsed = serde_json::from_slice::<GraphqlResponse>(body)
        .map_err(|err| LinearError::Message(format!("failed to decode Linear response: {err}")))?;

    if let Some(error) = parsed.errors.first() {
        if status.is_success() {
            return Err(LinearError::Message(error.display_message().to_string()));
        }
        return Err(LinearError::Message(format!(
            "linear request failed with status {status}: {}",
            error.display_message()
        )));
    }

    if !status.is_success() {
        let body_text = String::from_utf8_lossy(body).trim().to_string();
        if body_text.is_empty() {
            return Err(LinearError::Message(format!(
                "linear request failed with status {status}"
            )));
        }
        return Err(LinearError::Message(format!(
            "linear request failed with status {status}: {body_text}"
        )));
    }

    let data = parsed
        .data
        .ok_or_else(|| LinearError::Message("linear response missing data".to_string()))?;
    serde_json::from_value(data)
        .map_err(|err| LinearError::Message(format!("failed to decode Linear response: {err}")))
}

fn repository_claim_marker(repository: &str) -> String {
    format!("{REPOSITORY_CLAIM_PREFIX} {repository} -->")
}

fn repository_claim(description: &str) -> LinearResult<Option<String>> {
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
            return Err(LinearError::Message(format!(
                "Linear Team has a malformed Loopflow repository marker: {trimmed:?}"
            )));
        };
        claims.push(value.to_string());
    }
    match claims.as_slice() {
        [] => Ok(None),
        [claim] => Ok(Some(claim.clone())),
        _ => Err(LinearError::Message(format!(
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

pub(crate) fn linear_description(description: &str) -> String {
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
    use crate::work::edit::WorkValue;
    #[tokio::test]
    async fn project_markdown_prose_replacement_preserves_managed_fields() {
        let client =
            super::LinearClient::with_base_url("fixture".into(), None, "http://127.0.0.1:1".into());
        let source = "Old introduction.\nworkflow: code\n## Metric targets\nTarget rationale.\n```json\n[{\"metric_id\":\"proof\",\"target\":{\"kind\":\"at_least\",\"value\":1.0}}]\n```\n## KRs\nPreamble.\n- [ ] Keep this criterion\n  across lines.\n\nOld closing.\n";
        let before = super::parse_project_document(source).unwrap();
        for description in ["", "New authored prose.", "## Examples\n````markdown\n```yaml\nworkflow: example\n```\n## KRs\n- [x] Example only\n````\nClosing prose.\n"] {
            let input = client.project_field_input(&WorkValue::ProjectDescription(Some(description.into())), source).await.unwrap();
            let after = input["content"].as_str().unwrap();
            let parsed = super::parse_project_document(after).unwrap();
            assert_eq!(parsed.description().trim(), description.trim());
            assert!(after.contains("workflow: code\n"));
            assert_eq!(parsed.krs, before.krs);
            assert_eq!(parsed.metric_targets, before.metric_targets);
            assert!(!after.contains("Old introduction"));
            assert!(!after.contains("Target rationale"));
            assert!(!after.contains("Old closing"));
        }
        for description in [
            "workflow: replacement",
            "## KRs\n- [x] Replacement",
            "```markdown\nUnclosed example",
        ] {
            assert!(
                client
                    .project_field_input(
                        &WorkValue::ProjectDescription(Some(description.into())),
                        source
                    )
                    .await
                    .is_err(),
                "{description}"
            );
        }
        assert!(client
            .project_field_input(&WorkValue::ProjectDescription(None), source)
            .await
            .is_err());
    }

    #[test]
    fn project_markdown_edits_preserve_examples_and_unknown_prose() {
        for example in [
            "````markdown\n```yaml\nworkflow: example\nflow: legacy\n```\n## KRs\n- [x] Example only\n````\n",
            "~~~yaml\nworkflow: example\n## Metric targets\nnot JSON\n~~~\n",
        ] {
            let source = format!("## Example\n{example}\nworkflow: actual\n## Metric targets\nTarget rationale.\n```json\n[]\n```\nTarget note.\n## KRs\nKR preamble.\n- [ ] Original criterion\n\nParagraph after the criterion.\n{example}## Notes\nKeep these bytes.\n");
            let before = super::parse_project_document(&source).unwrap();
            assert_eq!(before.krs.len(), 1);
            assert_eq!(before.krs[0].text, "Original criterion");
            assert!(before.metric_targets.is_empty());
            for (field, value) in [
                ("krs", serde_json::json!([{"text":"New criterion","holds":true}])),
                ("metric_targets", serde_json::json!([{"metric_id":"proof","target":{"kind":"at_least","value":1.0}}])),
            ] {
                let after = super::patch_project_field(&source, &serde_json::from_value(serde_json::json!({"field":field,"value":value})).unwrap()).unwrap();
                assert_eq!(after.matches(example).count(), 2, "{field}: {after}");
                for prose in ["Target rationale.\n", "Target note.\n", "KR preamble.\n", "Paragraph after the criterion.\n", "## Notes\nKeep these bytes.\n"] {
                    assert!(after.contains(prose), "{field}: {after}");
                }
                let parsed = super::parse_project_document(&after).unwrap();
                assert!(after.contains("workflow: actual\n"));
                assert_eq!(parsed.krs, if field == "krs" { serde_json::from_value(value.clone()).unwrap() } else { before.krs.clone() });
                assert_eq!(parsed.metric_targets, if field == "metric_targets" { serde_json::from_value(value).unwrap() } else { before.metric_targets.clone() });
            }
        }
    }

    #[test]
    fn project_markdown_edits_reject_duplicate_targets() {
        let targets = serde_json::from_value(serde_json::json!([
            {"metric_id":"rate","target":{"kind":"at_least","value":1.0}},
            {"metric_id":"rate","target":{"kind":"at_most","value":2.0}}
        ]))
        .unwrap();
        assert!(
            super::patch_project_field("workflow: code", &WorkValue::MetricTargets(targets))
                .is_err()
        );
    }

    #[test]
    fn project_markdown_edits_keep_unfenced_targets_separate_from_kr_prose() {
        let source = "## Metric targets\n[]\n## KRs\nPreamble.\n- [ ] Existing proof\n";
        let after = super::patch_project_field(
            source,
            &WorkValue::Krs(vec![crate::work::project::ProjectKR {
                text: "New proof".into(),
                holds: true,
            }]),
        )
        .unwrap();
        assert!(after.contains("Preamble.\n"));
        let content = super::parse_project_document(&after).unwrap();
        assert!(content.metric_targets.is_empty());
        assert_eq!(content.krs[0].text, "New proof");
    }

    use super::*;
    use crate::linear::test_server::{self, json_response};
    use axum::http::StatusCode;
    use serde_json::json;

    #[tokio::test]
    async fn zero_retry_after_still_pauses_other_callers_without_more_requests() {
        let mut limited = json_response(
            StatusCode::TOO_MANY_REQUESTS,
            json!({"errors":[{"message":"Rate limit exceeded"}]}),
        );
        limited.headers.push(("retry-after".into(), "0".into()));
        limited
            .headers
            .push(("x-ratelimit-requests-reset".into(), "1".into()));
        let (url, requests) = test_server::spawn(vec![limited]).await;
        for _ in 0..5 {
            let client = LinearClient::with_base_url("token".into(), None, url.clone());
            assert!(matches!(
                client.initiatives().await,
                Err(crate::linear::LinearError::RateLimited { .. })
            ));
        }
        assert_eq!(requests.lock().await.len(), 1);
    }

    #[tokio::test]
    async fn last_allowed_response_is_returned_before_shared_pause() {
        let mut last = json_response(
            StatusCode::OK,
            json!({"data":{"initiatives":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}),
        );
        last.headers
            .push(("x-ratelimit-complexity-remaining".into(), "0".into()));
        last.headers.push((
            "x-ratelimit-complexity-reset".into(),
            ((time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) + 60_000)
                .to_string(),
        ));
        let (url, requests) = test_server::spawn(vec![last]).await;
        let client = LinearClient::with_base_url("token".into(), None, url);
        assert!(client.initiatives().await.unwrap().is_empty());
        assert!(matches!(
            client.initiatives().await,
            Err(crate::linear::LinearError::RateLimited { .. })
        ));
        assert_eq!(requests.lock().await.len(), 1);
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
        let (base_url, requests) =
            test_server::spawn(vec![json_response(StatusCode::OK, response)]).await;
        let client = LinearClient::with_base_url("fixture".into(), Some("team-1".into()), base_url);
        let adopted = client
            .adopt_project("project-1", "initiative-1", "team-1", false, true)
            .await
            .unwrap();
        assert_eq!(adopted.id, "project-1");
        assert_eq!(adopted.name, "Summer work — customer requests");
        assert_eq!(adopted.krs[0].text, "Preserve this proof");
        assert_eq!(adopted.status, crate::work::project::ProjectStatus::Started);
        assert_eq!(requests.lock().await.len(), 1);
    }

    #[tokio::test]
    async fn project_adoption_uses_the_mutation_acknowledgement() {
        let mut project = json!({"id":"project-1", "name":"Chapter", "description":"Summary",
            "content":"## KRs\n- [ ] Preserve this proof\n", "status":{"type":"planned"},
            "updatedAt":"2026-10-10T12:00:00Z", "archivedAt":null,
            "initiatives":{"nodes":[{"id":"initiative-1"}]}, "teams":{"nodes":[{"id":"team-1"}]}});
        let before = json!({"data":{"project":project}});
        project["status"]["type"] = json!("started");
        project["updatedAt"] = json!("2026-10-11T12:00:00Z");
        let (url, requests) = test_server::spawn(vec![
            json_response(StatusCode::OK, before),
            json_response(
                StatusCode::OK,
                json!({"data":{"projectStatuses":{
                    "nodes":[{"id":"started", "type":"started", "teamId":"team-1", "position":0.0}],
                    "pageInfo":{"hasNextPage":false, "endCursor":null}
                }}}),
            ),
            json_response(
                StatusCode::OK,
                json!({"data":{"projectUpdate":{"success":true,"project":project}}}),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url("fixture".into(), Some("team-1".into()), url);
        let adopted = client
            .adopt_project("project-1", "initiative-1", "team-1", true, true)
            .await
            .unwrap();
        assert_eq!(adopted.revision.as_deref(), Some("2026-10-11T12:00:00Z"));
        assert_eq!(requests.lock().await.len(), 3);
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
    fn task_history_provider_facts_require_valid_observed_completion_dates() {
        let planning: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        for item in planning.items {
            let mut wire = json!({
                "id":item.id, "identifier":item.identifier, "updatedAt":item.revision,
                "branchName":null,"url":null,"title":item.name,"description":item.description,
                "prioritySortOrder":0.0,"sortOrder":0.0,"assignee":null,
                "state":{"type":item.state},"team":{"id":"team"},
                "completedAt":item.completed_at, "dueDate":item.due_date
            });
            let fields: super::IssueFields = serde_json::from_value(wire.clone()).unwrap();
            let observed = fields.into_issue(item.rank, None).unwrap();
            assert_eq!(observed.completed, item.completed);
            assert_eq!(observed.state, item.state);
            assert_eq!(observed.completed_at, item.completed_at);
            wire["completedAt"] = json!("not a timestamp");
            assert!(serde_json::from_value::<super::IssueFields>(wire.clone())
                .unwrap()
                .into_issue(0, None)
                .is_err());
            wire.as_object_mut().unwrap().remove("completedAt");
            assert!(serde_json::from_value::<super::IssueFields>(wire).is_err());
        }
        assert_eq!(
            crate::linear::terminal_reason(Some("duplicate")),
            Some("Linear Task is duplicate")
        );
        assert_eq!(crate::linear::terminal_reason(Some("unknown")), None);
    }

    #[test]
    fn issue_due_date_survives_reads_without_changing_completion() {
        let mut wire = json!({
            "id":"issue", "identifier":"LOO-1", "updatedAt":"2026-10-07T00:00:00Z",
            "branchName":null, "url":null, "title":"Check installation", "description":null,
            "prioritySortOrder":0.0, "sortOrder":0.0, "assignee":null,
            "state":{"type":"unstarted"}, "team":{"id":"team"},
            "completedAt":null, "dueDate":"2026-10-08"
        });
        let item = serde_json::from_value::<super::IssueFields>(wire.clone())
            .unwrap()
            .into_issue(0, None)
            .unwrap();
        assert_eq!(item.due_date.as_deref(), Some("2026-10-08"));
        assert!(!item.completed);
        wire["dueDate"] = json!("2026-02-30");
        assert!(serde_json::from_value::<super::IssueFields>(wire.clone())
            .unwrap()
            .into_issue(0, None)
            .is_err());
        wire["dueDate"] = serde_json::Value::Null;
        assert_eq!(
            serde_json::from_value::<super::IssueFields>(wire.clone())
                .unwrap()
                .into_issue(0, None)
                .unwrap()
                .due_date,
            None
        );
        wire.as_object_mut().unwrap().remove("dueDate");
        assert!(serde_json::from_value::<super::IssueFields>(wire).is_err());
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
                LinearComment {
                    author_name: Some("Jack".to_string()),
                    id: "c-1".to_string(),
                    created_at: None,
                    revision: None,
                    body: "please prioritize".to_string(),
                    author_id: Some("user-human".to_string()),
                },
                LinearComment {
                    author_name: None,
                    id: "c-2".to_string(),
                    created_at: None,
                    revision: None,
                    body: "PR: https://x".to_string(),
                    author_id: Some("user-loopflow".to_string()),
                },
                LinearComment {
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
                                    "completedAt": null, "dueDate": null, "prioritySortOrder": 10.0,
                                    "sortOrder": 10.0, "updatedAt":"2026-09-29T12:00:00.123Z",
                                    "assignee": { "id": "user-1" },
                                    "state": { "type": "unstarted" },
                                    "project": { "id": "project-123" },
                                    "team": { "id": "team-9" }
                                },
                                {
                                    "id": "issue-2",
                                    "identifier": "LOO-2",
                                    "url": null,
                                    "assignee": null,
                                    "title": "Second",
                                    "description": "two",
                                    "completedAt": null, "dueDate": null, "prioritySortOrder": 0.0,
                                    "sortOrder": 0.0, "updatedAt":"2026-09-29T12:00:00.123Z",
                                    "state": { "type": "completed" },
                                    "project": { "id": "project-123" },
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
        assert!(items
            .iter()
            .all(|item| item.project_id.as_deref() == Some("project-123")));
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
    async fn project_status_selection_prefers_team_then_position_and_id() {
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
        for (team, preferred) in [("selected", "a-tie"), ("missing", "workspace")] {
            let responses = pages
                .iter()
                .map(|page| json_response(StatusCode::OK, page.clone()))
                .collect();
            let (base_url, _) = test_server::spawn(responses).await;
            let client = LinearClient::with_base_url("fixture".into(), Some(team.into()), base_url);
            assert_eq!(
                client
                    .project_status_id(crate::work::project::ProjectStatus::Completed)
                    .await
                    .unwrap(),
                preferred
            );
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
                .project_status_id(crate::work::project::ProjectStatus::Completed)
                .await
                .unwrap_err();
            assert!(error.to_string().contains("cursor"));
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
        assert!(error.to_string().contains("lf reteam"));
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
                "completedAt": null, "dueDate": null, "prioritySortOrder": 0.0, "sortOrder": 0.0, "updatedAt":"2026-09-29T12:00:00.123Z",
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
        assert_eq!(item.team_id.as_deref(), Some("team-loo"));
        assert_eq!(project.initiative_ids, ["initiative-product"]);
        assert_eq!(project.team_ids, ["team-loo"]);
    }

    #[tokio::test]
    async fn item_state_uses_the_issue_team_and_lowest_position_for_each_outcome() {
        for target in ["completed", "unstarted", "canceled"] {
            let (base_url, requests) = test_server::spawn(vec![
                json_response(
                    StatusCode::OK,
                    json!({ "data": { "issue": { "team": { "id": "team-eng" } } } }),
                ),
                json_response(
                    StatusCode::OK,
                    json!({ "data": { "workflowStates": { "nodes": [
                        { "id": "state-secondary", "position": 2.0 },
                        { "id": "state-default", "position": 1.0 }
                    ] } } }),
                ),
                json_response(
                    StatusCode::OK,
                    json!({ "data": { "issueUpdate": { "success":true,"issue": {
                        "id":"ENG-7","identifier":"ENG-7","title":"Fixture","description":null,
                        "branchName":null,"completedAt":null,"dueDate":null,"url":null,
                        "updatedAt":"2026-10-11T12:00:00Z","prioritySortOrder":0.0,"sortOrder":0.0,
                        "state":{"type":target},"assignee":null,"team":{"id":"team-eng"},"project":null
                    } } } }),
                ),
            ])
            .await;
            // The issue belongs to a different Team from the repository binding.
            let client = LinearClient::with_base_url(
                "linear-secret".to_string(),
                Some("team-wave".to_string()),
                base_url,
            );
            let state = client.item_state_id("ENG-7", target).await.unwrap();
            assert_eq!(state, "state-default");
            client.set_item_state("ENG-7", &state).await.unwrap();

            let requests = requests.lock().await;
            let lookup: Value = serde_json::from_str(&requests[1].body).unwrap();
            assert_eq!(
                lookup["variables"],
                json!({"teamId":"team-eng", "type":target})
            );
            let update: Value = serde_json::from_str(&requests[2].body).unwrap();
            assert_eq!(
                update["variables"],
                json!({"id":"ENG-7", "stateId":"state-default"})
            );
        }
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

#[cfg(test)]
mod retained_relation_tests;

/// Retained Linear facts, including foreign and not-yet-associated Projects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct LinearProject {
    #[serde(deserialize_with = "crate::id::deserialize_uuid")]
    pub id: String,
    pub revision: Option<String>,
    pub slug: String,
    pub name: String,
    pub summary: String,
    /// None means historical/unobserved prose, never an observed empty value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub metric_targets: Vec<MetricTarget>,
    pub status: ProjectStatus,
    pub krs: Vec<ProjectKR>,
    #[serde(deserialize_with = "crate::id::deserialize_uuid_vec")]
    pub initiative_ids: Vec<String>,
    /// Stable ids of the Linear teams this Project belongs to. A managed Project
    /// must carry exactly the repository Team.
    #[serde(deserialize_with = "crate::id::deserialize_uuid_vec")]
    pub team_ids: Vec<String>,
}
#[cfg(test)]
mod model_tests {
    use super::{project_slug, LinearProject};

    #[test]
    fn project_slug_is_deterministic() {
        assert_eq!(project_slug("Mac Surface UX"), "mac-surface-ux");
        assert_eq!(project_slug("  API / Auditability  "), "api-auditability");
    }

    #[test]
    fn project_snapshot_requires_flow_and_status() {
        let value = serde_json::json!({
            "id":"project-1", "slug":"plan", "name":"Plan", "summary":"",
            "metric_targets":[], "krs":[], "initiative_ids":[], "team_ids":[]
        });
        assert!(serde_json::from_value::<LinearProject>(value).is_err());
    }
}
