use crate::pm::linear::LinearProject;
pub mod linear;
pub(crate) mod project_document;

use std::time::Duration;

use serde::{Deserialize, Serialize};
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
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, LinearBindingError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(LinearBindingError::InvalidId(format!(
                        "{} cannot be empty",
                        $label
                    )));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub(crate) fn from_raw(value: impl Into<String>) -> Self {
                Self(value.into())
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmWave {
    pub id: String,
    pub name: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmItem {
    /// Provider revision; absent only in historical planning.
    pub revision: Option<String>,
    pub id: String,
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
    /// Canonical Project slug for display only.
    pub project: Option<String>,
    /// Stable owning repository Team id.
    pub team_id: Option<String>,
    /// Provider user ID of the assignee, if any.
    pub assignee: Option<String>,
}

impl PmItem {
    pub fn terminal_reason(&self) -> Option<&'static str> {
        terminal_reason(self.state.as_deref(), self.completed)
    }

    /// Linear calls the item complete, not canceled or duplicate.
    pub fn is_complete(&self) -> bool {
        match self.state.as_deref() {
            Some(state) => state == "completed",
            None => self.completed,
        }
    }
}

pub fn terminal_reason(state: Option<&str>, completed: bool) -> Option<&'static str> {
    match state {
        Some("canceled") => Some("Linear Task is canceled"),
        Some("duplicate") => Some("Linear Task is duplicate"),
        Some("completed") => Some("Linear Task is complete"),
        None if completed => Some("Linear Task is complete"),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PmSnapshot {
    pub projects: Vec<LinearProject>,
    pub items: Vec<PmItem>,
}

pub(crate) fn validate_project_ownership(
    wave: &str,
    initiative_id: &str,
    expected_team_id: Option<&str>,
    project: &LinearProject,
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PmItemUpdate {
    pub rank: Option<u32>,
    /// Omitted preserves assignment; explicit None clears it.
    pub assignee: Option<Option<String>>,
    pub name: Option<String>,
    pub description: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

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
