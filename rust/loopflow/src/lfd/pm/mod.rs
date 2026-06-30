pub mod asana;
mod asana_html;

use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PriorityBucket {
    Urgent,
    High,
    Medium,
    Low,
}

impl PriorityBucket {
    pub(crate) fn from_filename_prefix(prefix: &str) -> Option<Self> {
        match prefix {
            "1" => Some(Self::Urgent),
            "2" => Some(Self::High),
            "3" => Some(Self::Medium),
            "4" => Some(Self::Low),
            _ => None,
        }
    }

    pub(crate) fn from_semantic_label(label: &str) -> Option<Self> {
        match label.trim().to_ascii_lowercase().as_str() {
            "urgent" => Some(Self::Urgent),
            "high" => Some(Self::High),
            "medium" => Some(Self::Medium),
            "low" => Some(Self::Low),
            _ => None,
        }
    }

    pub(crate) fn from_rank(rank: u32) -> Self {
        match rank {
            0 => Self::Urgent,
            1 => Self::High,
            2 => Self::Medium,
            _ => Self::Low,
        }
    }

    pub(crate) fn order(self) -> u8 {
        match self {
            Self::Urgent => 0,
            Self::High => 1,
            Self::Medium => 2,
            Self::Low => 3,
        }
    }

    pub(crate) fn rank(self) -> u32 {
        u32::from(self.order())
    }

    pub(crate) fn semantic_label(self) -> &'static str {
        match self {
            Self::Urgent => "Urgent",
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmProject {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub rank: u32,
    pub completed: bool,
    /// Provider user ID of the assignee, if any.
    pub assignee: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmItemCreate {
    pub name: String,
    pub description: String,
    pub rank: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PmItemUpdate {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub rank: Option<u32>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RoadmapItemFrontmatter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<PriorityBucket>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asana_id: Option<String>,
}

impl RoadmapItemFrontmatter {
    fn is_empty(&self) -> bool {
        self.priority.is_none()
            && self.rank.is_none()
            && self.status.is_none()
            && self.claimed_by.is_none()
            && self.claimed_at.is_none()
            && self.asana_id.is_none()
    }

    pub fn set_priority_rank(&mut self, rank: u32) {
        self.priority = Some(PriorityBucket::from_rank(rank));
        self.rank = None;
    }

    pub fn mark_in_progress(&mut self, claimed_by: String, claimed_at: String) {
        self.status = Some("in-progress".to_string());
        self.claimed_by = Some(claimed_by);
        self.claimed_at = Some(claimed_at);
    }
}

#[derive(Debug, Clone, PartialEq)]
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
    fn pm_item_update_text_update_skips_rank_only_changes() {
        let update = PmItemUpdate {
            rank: Some(1),
            ..PmItemUpdate::default()
        };

        assert_eq!(update.text_update(), None);
    }

    #[test]
    fn pm_item_update_text_update_preserves_name_and_description() {
        let update = PmItemUpdate {
            name: Some("Ship the client".to_string()),
            description: Some("Build the API client".to_string()),
            rank: Some(1),
        };

        assert_eq!(
            update.text_update(),
            Some(PmTextUpdate {
                name: Some("Ship the client"),
                description: Some("Build the API client"),
            })
        );
    }

    #[test]
    fn roadmap_item_frontmatter_mark_in_progress_sets_claim_metadata() {
        let mut frontmatter = RoadmapItemFrontmatter::default();
        frontmatter.mark_in_progress("run-123".to_string(), "2026-03-29T12:00:00Z".to_string());

        assert_eq!(frontmatter.status.as_deref(), Some("in-progress"));
        assert_eq!(frontmatter.claimed_by.as_deref(), Some("run-123"));
        assert_eq!(
            frontmatter.claimed_at.as_deref(),
            Some("2026-03-29T12:00:00Z")
        );
    }
}
