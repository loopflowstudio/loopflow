pub mod asana;
pub mod linear;

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PriorityBucket {
    Urgent,
    High,
    Medium,
    Low,
}

impl PriorityBucket {
    pub fn filename_prefix(self) -> &'static str {
        match self {
            Self::Urgent => "1",
            Self::High => "2",
            Self::Medium => "3",
            Self::Low => "4",
        }
    }

    pub fn order(self) -> u8 {
        match self {
            Self::Urgent => 0,
            Self::High => 1,
            Self::Medium => 2,
            Self::Low => 3,
        }
    }

    pub fn semantic_label(self) -> &'static str {
        match self {
            Self::Urgent => "Urgent",
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
        }
    }

    pub fn from_filename_prefix(value: &str) -> Option<Self> {
        match value.trim() {
            "1" => Some(Self::Urgent),
            "2" => Some(Self::High),
            "3" => Some(Self::Medium),
            "4" => Some(Self::Low),
            _ => None,
        }
    }

    pub fn from_semantic_label(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "urgent" => Some(Self::Urgent),
            "high" => Some(Self::High),
            "medium" | "med" => Some(Self::Medium),
            "low" => Some(Self::Low),
            _ => None,
        }
    }

    pub fn from_linear_value(value: i64) -> Self {
        match value {
            1 => Self::Urgent,
            2 => Self::High,
            3 => Self::Medium,
            _ => Self::Low,
        }
    }

    pub fn linear_value(self) -> i64 {
        match self {
            Self::Urgent => 1,
            Self::High => 2,
            Self::Medium => 3,
            Self::Low => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PmProviderKind {
    Asana,
    Linear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmConfig {
    pub provider: PmProviderKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asana_project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linear_project: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmProject {
    pub id: String,
    pub name: String,
}

impl PmConfig {
    pub fn project_for(&self, provider: PmProviderKind) -> Option<&str> {
        match provider {
            PmProviderKind::Asana => self.asana_project.as_deref(),
            PmProviderKind::Linear => self.linear_project.as_deref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub priority: PriorityBucket,
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmItemCreate {
    pub name: String,
    pub description: String,
    pub priority: PriorityBucket,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PmItemUpdate {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub priority: Option<PriorityBucket>,
}

impl PmItemUpdate {
    pub(crate) fn is_noop(&self) -> bool {
        self.name.is_none() && self.description.is_none() && self.priority.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PmError {
    #[error("{0}")]
    Message(String),
}

pub type PmResult<T> = Result<T, PmError>;

#[async_trait]
pub trait PmProvider: Send + Sync {
    /// Create a new team/workspace. Returns the team ID.
    async fn create_team(&self, name: &str) -> PmResult<String>;
    /// Check whether a team named `name` already exists. Returns its ID if so.
    async fn find_team(&self, name: &str) -> PmResult<Option<String>>;
    async fn create_project(&self, name: &str, description: &str) -> PmResult<String>;
    /// Create a project inside a specific team. Used by init to target a freshly created team.
    async fn create_project_in_team(
        &self,
        team_id: &str,
        name: &str,
        description: &str,
    ) -> PmResult<String>;
    async fn list_projects(&self, team_id: &str) -> PmResult<Vec<PmProject>>;
    async fn list_items(&self, project_id: &str) -> PmResult<Vec<PmItem>>;
    async fn create_item(&self, project_id: &str, item: &PmItemCreate) -> PmResult<String>;
    async fn update_item(&self, item_id: &str, update: &PmItemUpdate) -> PmResult<()>;
    async fn complete_item(&self, item_id: &str) -> PmResult<()>;
    async fn comment(&self, item_id: &str, body: &str) -> PmResult<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RoadmapItemFrontmatter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asana_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linear_id: Option<String>,
}

impl RoadmapItemFrontmatter {
    fn is_empty(&self) -> bool {
        self.asana_id.is_none() && self.linear_id.is_none()
    }

    pub fn id_for(&self, provider: PmProviderKind) -> Option<&str> {
        match provider {
            PmProviderKind::Asana => self.asana_id.as_deref(),
            PmProviderKind::Linear => self.linear_id.as_deref(),
        }
    }

    pub fn set_id(&mut self, provider: PmProviderKind, id: String) {
        match provider {
            PmProviderKind::Asana => self.asana_id = Some(id),
            PmProviderKind::Linear => self.linear_id = Some(id),
        }
    }

    pub fn clear_id(&mut self, provider: PmProviderKind) {
        match provider {
            PmProviderKind::Asana => self.asana_id = None,
            PmProviderKind::Linear => self.linear_id = None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoadmapItemDocument {
    pub frontmatter: RoadmapItemFrontmatter,
    pub body: String,
}

impl RoadmapItemDocument {
    pub fn parse(content: &str) -> PmResult<Self> {
        let Some((frontmatter, body)) = split_frontmatter(content) else {
            return Ok(Self {
                frontmatter: RoadmapItemFrontmatter::default(),
                body: content.to_string(),
            });
        };

        let frontmatter = serde_yaml_ng::from_str::<RoadmapItemFrontmatter>(&frontmatter)
            .map_err(|err| PmError::Message(format!("invalid roadmap frontmatter: {err}")))?;

        Ok(Self { frontmatter, body })
    }

    pub fn render(&self) -> PmResult<String> {
        if self.frontmatter.is_empty() {
            return Ok(self.body.clone());
        }

        let frontmatter = serde_yaml_ng::to_string(&self.frontmatter).map_err(|err| {
            PmError::Message(format!("failed to encode roadmap frontmatter: {err}"))
        })?;

        Ok(format!("---\n{}---\n{}", frontmatter, self.body))
    }
}

const RATE_LIMIT_RETRIES: u8 = 3;
const RETRY_AFTER_FALLBACK: Duration = Duration::from_secs(60);

fn retry_after_delay(headers: &reqwest::header::HeaderMap) -> Duration {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(RETRY_AFTER_FALLBACK)
}

fn split_frontmatter(content: &str) -> Option<(String, String)> {
    let rest = content.strip_prefix("---\n")?;
    let (frontmatter, body) = rest.split_once("\n---\n")?;
    Some((frontmatter.to_string(), body.to_string()))
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
    fn roadmap_item_document_parses_asana_id_frontmatter() {
        let doc = RoadmapItemDocument::parse(
            "---\nasana_id: \"9876543210\"\n---\n# 01: Ship offline sync\n",
        )
        .expect("frontmatter should parse");

        assert_eq!(doc.frontmatter.asana_id.as_deref(), Some("9876543210"));
        assert_eq!(doc.body, "# 01: Ship offline sync\n");
    }

    #[test]
    fn roadmap_item_document_parses_without_frontmatter() {
        let doc = RoadmapItemDocument::parse("# 01: Ship offline sync\n")
            .expect("body-only documents should parse");

        assert_eq!(doc.frontmatter.asana_id, None);
        assert_eq!(doc.body, "# 01: Ship offline sync\n");
    }

    #[test]
    fn roadmap_item_document_render_round_trips_provider_id() {
        let original = RoadmapItemDocument {
            frontmatter: RoadmapItemFrontmatter {
                asana_id: Some("9876543210".to_string()),
                ..RoadmapItemFrontmatter::default()
            },
            body: "# 01: Ship offline sync\n".to_string(),
        };

        let rendered = original.render().expect("document should render");
        let reparsed =
            RoadmapItemDocument::parse(&rendered).expect("rendered document should parse");

        assert_eq!(reparsed, original);
    }

    #[test]
    fn roadmap_item_document_render_omits_empty_frontmatter() {
        let doc = RoadmapItemDocument {
            frontmatter: RoadmapItemFrontmatter::default(),
            body: "# 01: Ship offline sync\n".to_string(),
        };

        let rendered = doc.render().expect("document should render");
        assert_eq!(rendered, "# 01: Ship offline sync\n");
    }

    #[test]
    fn pm_item_update_is_noop_skips_priority_only_changes() {
        let update = PmItemUpdate {
            priority: Some(PriorityBucket::High),
            ..PmItemUpdate::default()
        };

        assert!(!update.is_noop());
    }

    #[test]
    fn pm_item_update_is_noop_only_without_text_or_priority_changes() {
        assert!(PmItemUpdate::default().is_noop());
        assert!(!PmItemUpdate {
            priority: Some(PriorityBucket::Medium),
            ..PmItemUpdate::default()
        }
        .is_noop());
    }

    #[test]
    fn priority_bucket_parses_semantic_labels() {
        assert_eq!(
            PriorityBucket::from_semantic_label("urgent"),
            Some(PriorityBucket::Urgent)
        );
        assert_eq!(
            PriorityBucket::from_semantic_label("High"),
            Some(PriorityBucket::High)
        );
        assert_eq!(
            PriorityBucket::from_semantic_label("medium"),
            Some(PriorityBucket::Medium)
        );
        assert_eq!(
            PriorityBucket::from_semantic_label("med"),
            Some(PriorityBucket::Medium)
        );
        assert_eq!(
            PriorityBucket::from_semantic_label("LOW"),
            Some(PriorityBucket::Low)
        );
        assert_eq!(PriorityBucket::from_semantic_label("later"), None);
    }

    #[test]
    fn priority_bucket_round_trips_linear_values() {
        assert_eq!(PriorityBucket::Urgent.linear_value(), 1);
        assert_eq!(PriorityBucket::High.linear_value(), 2);
        assert_eq!(PriorityBucket::Medium.linear_value(), 3);
        assert_eq!(PriorityBucket::Low.linear_value(), 4);
        assert_eq!(PriorityBucket::from_linear_value(1), PriorityBucket::Urgent);
        assert_eq!(PriorityBucket::from_linear_value(2), PriorityBucket::High);
        assert_eq!(PriorityBucket::from_linear_value(3), PriorityBucket::Medium);
        assert_eq!(PriorityBucket::from_linear_value(4), PriorityBucket::Low);
        assert_eq!(PriorityBucket::from_linear_value(99), PriorityBucket::Low);
    }

    #[test]
    fn roadmap_item_frontmatter_clear_id_removes_selected_provider_only() {
        let mut frontmatter = RoadmapItemFrontmatter {
            asana_id: Some("asa-1".to_string()),
            linear_id: Some("lin-1".to_string()),
        };

        frontmatter.clear_id(PmProviderKind::Asana);
        assert_eq!(frontmatter.asana_id, None);
        assert_eq!(frontmatter.linear_id.as_deref(), Some("lin-1"));

        frontmatter.clear_id(PmProviderKind::Linear);
        assert_eq!(frontmatter.linear_id, None);
    }
}
