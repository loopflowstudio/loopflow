pub mod asana;
mod asana_html;
pub mod linear;

use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PmProviderKind {
    Asana,
    Linear,
}

impl PmProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Asana => "asana",
            Self::Linear => "linear",
        }
    }

    pub fn project_key(self) -> &'static str {
        match self {
            Self::Asana => "asana_project",
            Self::Linear => "linear_project",
        }
    }
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
            "asana" => Ok(Self::Asana),
            "linear" => Ok(Self::Linear),
            other => Err(PmError::Message(format!(
                "unsupported PM provider {other:?}; expected \"asana\" or \"linear\""
            ))),
        }
    }
}

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
            name: Some("Ship Asana".to_string()),
            description: Some("Build the roadmap client".to_string()),
            rank: Some(1),
        };

        assert_eq!(
            update.text_update(),
            Some(PmTextUpdate {
                name: Some("Ship Asana"),
                description: Some("Build the roadmap client"),
            })
        );
    }
}
