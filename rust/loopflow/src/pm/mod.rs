pub mod linear;

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PmProviderKind {
    Linear,
}

impl PmProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
        }
    }

    pub fn initiative_key(self) -> &'static str {
        match self {
            Self::Linear => "linear_initiative",
        }
    }
}

/// The result of adopting or creating a provider team for a repository. The stable
/// `id` owns identity; `key` is mutable presentation (the Task prefix).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamBinding {
    pub id: String,
    pub key: String,
    pub created: bool,
}

impl std::fmt::Display for PmProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PmProviderKind {
    type Err = PmError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "linear" => Ok(Self::Linear),
            other => Err(PmError::Message(format!(
                "unsupported PM provider {other:?}; expected \"linear\""
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmKr {
    pub text: String,
    pub holds: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectFlowPlan {
    pub first: Option<String>,
    #[serde(rename = "loop")]
    pub loop_: Option<String>,
    pub finally: Option<String>,
}

impl ProjectFlowPlan {
    pub fn empty() -> Self {
        Self {
            first: None,
            loop_: None,
            finally: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectContent {
    pub definition: String,
    pub flows: ProjectFlowPlan,
    pub krs: Vec<PmKr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmProject {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub definition: String,
    /// `None` means this provider snapshot predates Project flow configuration.
    /// Fresh provider reads always resolve it to `Some`, including an empty plan.
    pub flows: Option<ProjectFlowPlan>,
    pub krs: Vec<PmKr>,
    pub initiative_ids: Vec<String>,
    /// Stable ids of the Linear teams this Project belongs to. A managed Project
    /// must carry exactly the repository Team.
    pub team_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmWave {
    pub id: String,
    pub name: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmItem {
    pub id: String,
    pub identifier: String,
    /// Provider-owned issue URL captured during PM sync. `None` stays explicit
    /// when the provider did not return one; status and roadmap reads never
    /// fetch it on demand.
    pub url: Option<String>,
    pub name: String,
    pub description: String,
    pub rank: u32,
    pub completed: bool,
    /// Stable owning Project id. Task-to-Wave resolution follows this edge.
    pub project_id: String,
    /// Canonical Project slug for display only.
    pub project: String,
    /// Stable owning repository Team id.
    pub team_id: String,
    /// Provider user ID of the assignee, if any.
    pub assignee: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PmSnapshot {
    pub(crate) projects: Vec<PmProject>,
    pub(crate) items: Vec<PmItem>,
}

/// Validates provider ownership across cached Wave snapshots presented together.
#[derive(Debug, Default)]
pub(crate) struct PmPortfolioValidator {
    initiative_owners: BTreeMap<String, String>,
    project_owners: BTreeMap<String, String>,
    task_owners: BTreeMap<String, String>,
}

impl PmPortfolioValidator {
    pub(crate) fn validate(
        &mut self,
        wave: &str,
        initiative_id: &str,
        expected_team_id: Option<&str>,
        projects: &[PmProject],
        items: &[PmItem],
    ) -> PmResult<()> {
        validate_snapshot_ownership(wave, initiative_id, expected_team_id, projects, items)?;

        if let Some(owner) = self
            .initiative_owners
            .insert(initiative_id.to_string(), wave.to_string())
        {
            return Err(PmError::Message(format!(
                "Linear Initiative {initiative_id} is bound by both wave/{owner} and wave/{wave} snapshots"
            )));
        }
        for project in projects {
            if let Some(owner) = self
                .project_owners
                .insert(project.id.clone(), wave.to_string())
            {
                return Err(PmError::Message(format!(
                    "Linear Project {} belongs to both wave/{owner} and wave/{wave} snapshots",
                    project.id
                )));
            }
        }
        for item in items {
            if let Some(owner) = self.task_owners.insert(item.id.clone(), wave.to_string()) {
                return Err(PmError::Message(format!(
                    "Linear task {} belongs to both wave/{owner} and wave/{wave} snapshots",
                    item.identifier
                )));
            }
        }
        Ok(())
    }
}

fn validate_snapshot_ownership(
    wave: &str,
    initiative_id: &str,
    expected_team_id: Option<&str>,
    projects: &[PmProject],
    items: &[PmItem],
) -> PmResult<()> {
    let mut projects_by_id = BTreeMap::new();
    for project in projects {
        if projects_by_id
            .insert(project.id.as_str(), project)
            .is_some()
        {
            return Err(PmError::Message(format!(
                "Linear Project {} appears more than once in wave/{wave}",
                project.id
            )));
        }
        validate_project_ownership(wave, initiative_id, expected_team_id, project)?;
    }

    let mut issue_ids = BTreeSet::new();
    for item in items {
        if !issue_ids.insert(item.id.as_str()) {
            return Err(PmError::Message(format!(
                "Linear task {} appears more than once in wave/{wave}",
                item.identifier
            )));
        }
        let project = projects_by_id
            .get(item.project_id.as_str())
            .ok_or_else(|| {
                PmError::Message(format!(
                    "Linear task {} in wave/{wave} points to missing Project {}",
                    item.identifier, item.project_id
                ))
            })?;
        if item.project != project.slug {
            return Err(PmError::Message(format!(
                "Linear task {} in wave/{wave} names Project slug `{}`, but Project {} has slug `{}`",
                item.identifier, item.project, project.id, project.slug
            )));
        }
        if project.team_ids.as_slice() != [item.team_id.as_str()] {
            return Err(PmError::Message(format!(
                "Linear task {} in wave/{wave} belongs to Team {}, but Project {} belongs to Teams [{}]",
                item.identifier,
                item.team_id,
                project.id,
                project.team_ids.join(", ")
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_project_ownership(
    wave: &str,
    initiative_id: &str,
    expected_team_id: Option<&str>,
    project: &PmProject,
) -> PmResult<()> {
    if project.initiative_ids.as_slice() != [initiative_id] {
        return Err(PmError::Message(format!(
            "Linear Project `{}` ({}) in wave/{wave} belongs to Initiatives [{}]; expected exactly {initiative_id}",
            project.name,
            project.id,
            project.initiative_ids.join(", ")
        )));
    }
    let [team_id] = project.team_ids.as_slice() else {
        return Err(PmError::Message(format!(
            "Linear Project `{}` ({}) in wave/{wave} belongs to {} Teams [{}]; expected exactly one repository Team",
            project.name,
            project.id,
            project.team_ids.len(),
            project.team_ids.join(", ")
        )));
    };
    if expected_team_id.is_some_and(|expected| expected != team_id) {
        return Err(PmError::Message(format!(
            "Linear Project `{}` ({}) in wave/{wave} belongs to Team {team_id}; expected repository Team {}",
            project.name,
            project.id,
            expected_team_id.expect("checked as Some")
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmItemCreate {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PmItemUpdate {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// A read of one issue's human-editable content plus its comments, taken to
/// stream Linear edits into a Task. `revision` is the provider's
/// last-updated marker (Linear `updatedAt`), monotonic per issue, and is
/// compared — not trusted as identity — so out-of-order responses never move
/// the cursor backward.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueObservation {
    pub revision: String,
    pub title: String,
    pub description: String,
    pub comments: Vec<IssueComment>,
}

/// One issue comment, with just enough authorship to tell a human's direction
/// from Loopflow's own writeback. `author_id` is the provider user id; `None`
/// for an integration/bot actor with no backing user, which is never treated
/// as human direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueComment {
    pub id: String,
    pub body: String,
    pub author_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PmTextUpdate<'a> {
    pub(crate) name: Option<&'a str>,
    pub(crate) description: Option<&'a str>,
}

impl PmItemUpdate {
    pub(crate) fn text_update(&self) -> Option<PmTextUpdate<'_>> {
        if self.name.is_none() && self.description.is_none() {
            return None;
        }

        Some(PmTextUpdate {
            name: self.name.as_deref(),
            description: self.description.as_deref(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PmError {
    #[error("{0}")]
    Message(String),
}

pub type PmResult<T> = Result<T, PmError>;

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

pub fn parse_project_content(content: &str) -> ProjectContent {
    enum Section {
        None,
        Definition,
        Flows,
        Krs,
    }

    let mut section = Section::None;
    let mut definition = Vec::new();
    let mut flows = ProjectFlowPlan::empty();
    let mut cycle = None;
    let mut krs = Vec::new();
    let mut current_kr: Option<PmKr> = None;
    for line in content.lines() {
        let trimmed = line.trim();
        match trimmed {
            "## Definition" => {
                if let Some(kr) = current_kr.take() {
                    krs.push(kr);
                }
                section = Section::Definition;
                continue;
            }
            "## KRs" => {
                if let Some(kr) = current_kr.take() {
                    krs.push(kr);
                }
                section = Section::Krs;
                continue;
            }
            "## Flows" => {
                if let Some(kr) = current_kr.take() {
                    krs.push(kr);
                }
                section = Section::Flows;
                continue;
            }
            _ => {}
        }

        if matches!(section, Section::None)
            && trimmed.starts_with("# ")
            && !trimmed.starts_with("## ")
        {
            section = Section::Definition;
            continue;
        }

        match section {
            Section::Definition => definition.push(line),
            Section::Flows => {
                let Some((name, value)) = trimmed.split_once(':') else {
                    continue;
                };
                let value = match value.trim() {
                    "" => None,
                    value => Some(value.to_string()),
                };
                match name.trim() {
                    "first" => flows.first = value,
                    "loop" => flows.loop_ = value,
                    "finally" => flows.finally = value,
                    "cycle" => {
                        cycle = value
                            .as_deref()
                            .and_then(crate::ops::task::TaskCycle::parse)
                    }
                    _ => {}
                }
            }
            Section::Krs => {
                let item = if let Some(text) = trimmed.strip_prefix("- [x] ") {
                    Some((true, text))
                } else if let Some(text) = trimmed.strip_prefix("- [X] ") {
                    Some((true, text))
                } else if let Some(text) = trimmed.strip_prefix("- [ ] ") {
                    Some((false, text))
                } else {
                    trimmed.strip_prefix("- ").map(|text| (false, text))
                };

                if let Some((holds, text)) = item {
                    if let Some(kr) = current_kr.take() {
                        krs.push(kr);
                    }
                    if !text.trim().is_empty() {
                        current_kr = Some(PmKr {
                            text: text.trim().to_string(),
                            holds,
                        });
                    }
                } else if !trimmed.is_empty() {
                    if let Some(kr) = current_kr.as_mut() {
                        kr.text.push(' ');
                        kr.text.push_str(trimmed);
                    }
                }
            }
            Section::None => {}
        }
    }
    if let Some(kr) = current_kr {
        krs.push(kr);
    }

    // `cycle:` is sugar for the preset's flows; explicit keys win, and
    // writeback normalizes the sugar into explicit first/finally lines.
    if let Some(cycle) = cycle {
        let (first, finally) = cycle.flows();
        flows.first = flows.first.or_else(|| Some(first.to_string()));
        flows.finally = flows.finally.or_else(|| Some(finally.to_string()));
    }

    ProjectContent {
        definition: definition.join("\n").trim().to_string(),
        flows,
        krs,
    }
}

pub fn render_project_content(project: &ProjectContent) -> String {
    let mut content = format!("## Definition\n\n{}", project.definition.trim());
    if project.flows != ProjectFlowPlan::empty() {
        content.push_str("\n\n## Flows\n");
        if let Some(flow) = &project.flows.first {
            content.push_str(&format!("\nfirst: {}", flow.trim()));
        }
        if let Some(flow) = &project.flows.loop_ {
            content.push_str(&format!("\nloop: {}", flow.trim()));
        }
        if let Some(flow) = &project.flows.finally {
            content.push_str(&format!("\nfinally: {}", flow.trim()));
        }
    }
    content.push_str("\n\n## KRs");
    for kr in &project.krs {
        let marker = if kr.holds { "x" } else { " " };
        content.push_str(&format!("\n\n- [{marker}] {}", kr.text.trim()));
    }
    content.push('\n');
    content
}

pub(crate) const RATE_LIMIT_RETRIES: u8 = 3;
const RETRY_AFTER_FALLBACK: Duration = Duration::from_secs(60);

pub(crate) fn retry_after_delay(headers: &reqwest::header::HeaderMap) -> Duration {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(RETRY_AFTER_FALLBACK)
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
    }

    #[derive(Clone)]
    struct ServerState {
        requests: Arc<Mutex<Vec<CapturedRequest>>>,
        responses: Arc<Mutex<VecDeque<QueuedResponse>>>,
    }

    pub async fn spawn(
        responses: Vec<QueuedResponse>,
    ) -> (String, Arc<Mutex<Vec<CapturedRequest>>>) {
        let state = ServerState {
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
            headers: headers
                .into_iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
            body,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pm_item_update_text_update_skips_empty_changes() {
        let update = PmItemUpdate::default();

        assert_eq!(update.text_update(), None);
    }

    #[test]
    fn pm_item_update_text_update_preserves_name_and_description() {
        let update = PmItemUpdate {
            name: Some("Ship roadmap".to_string()),
            description: Some("Build the roadmap client".to_string()),
        };

        assert_eq!(
            update.text_update(),
            Some(PmTextUpdate {
                name: Some("Ship roadmap"),
                description: Some("Build the roadmap client"),
            })
        );
    }

    #[test]
    fn project_slug_is_deterministic() {
        assert_eq!(project_slug("Mac Surface UX"), "mac-surface-ux");
        assert_eq!(project_slug("  API / Auditability  "), "api-auditability");
    }

    #[test]
    fn project_snapshot_without_flows_remains_readable() {
        let project: PmProject = serde_json::from_str(
            r#"{
                "id":"project-1",
                "slug":"incident-management",
                "name":"Incident Management",
                "summary":"Restore service and prevent recurrence.",
                "definition":"Incidents are resolved at every causal layer.",
                "krs":[],
                "initiative_ids":["initiative-1"],
                "team_ids":["team-1"]
            }"#,
        )
        .expect("legacy project snapshot");

        assert_eq!(project.flows, None);
    }

    #[test]
    fn project_cycle_is_sugar_for_preset_flows() {
        let content = parse_project_content(
            "## Definition\n\nFix things.\n\n## Flows\n\ncycle: fix\n\n## KRs\n\n- [ ] holds\n",
        );
        assert_eq!(content.flows.first.as_deref(), Some("incident"));
        assert_eq!(content.flows.loop_, None);
        assert_eq!(content.flows.finally.as_deref(), Some("ship-demo"));

        let explicit = parse_project_content(
            "## Definition\n\nFix things.\n\n## Flows\n\ncycle: fix\nfirst: task-design\n",
        );
        assert_eq!(explicit.flows.first.as_deref(), Some("task-design"));
        assert_eq!(explicit.flows.finally.as_deref(), Some("ship-demo"));
    }

    #[test]
    fn project_content_round_trips_linear_markdown() {
        let krs = vec![
            PmKr {
                text: "One proof holds".to_string(),
                holds: true,
            },
            PmKr {
                text: "Another remains".to_string(),
                holds: false,
            },
        ];
        let project = ProjectContent {
            definition: "A measured bet.".to_string(),
            flows: ProjectFlowPlan {
                first: Some("incident".to_string()),
                loop_: Some("ship-5whys".to_string()),
                finally: Some("ship".to_string()),
            },
            krs: krs.clone(),
        };
        let rendered = render_project_content(&project);
        assert_eq!(parse_project_content(&rendered), project);

        let local = "# Project Name\n\nA measured bet.\n\n## KRs\n\n- One proof holds\n  across wrapped lines.\n";
        assert_eq!(
            parse_project_content(local),
            ProjectContent {
                definition: "A measured bet.".to_string(),
                flows: ProjectFlowPlan::empty(),
                krs: vec![PmKr {
                    text: "One proof holds across wrapped lines.".to_string(),
                    holds: false,
                }],
            }
        );
    }
}
