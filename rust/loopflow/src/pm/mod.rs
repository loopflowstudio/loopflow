pub mod linear;

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

    /// GOAL.md frontmatter key binding a wave to its provider team — the team
    /// whose key prefixes every Task identifier (`PRD-*`, `INF-*`, …).
    pub fn team_key(self) -> &'static str {
        match self {
            Self::Linear => "linear_team",
        }
    }
}

/// The result of adopting or creating a provider team for a wave. The stable
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
pub struct PmProject {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub definition: String,
    pub krs: Vec<PmKr>,
    pub initiative_ids: Vec<String>,
    /// Stable ids of the Linear teams this Project belongs to. `None` when the
    /// read did not resolve teams (a snapshot written before the field existed);
    /// `Some(vec)` is authoritative. Team ownership drives `doctor`/`reteam`
    /// repair; a Project's team is otherwise invisible to the team-agnostic read
    /// path. Optional so an older cached snapshot still decodes.
    pub team_ids: Option<Vec<String>>,
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
    pub project: Option<String>,
    /// Provider user ID of the assignee, if any.
    pub assignee: Option<String>,
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

pub fn parse_project_content(content: &str) -> (String, Vec<PmKr>) {
    enum Section {
        None,
        Definition,
        Krs,
    }

    let mut section = Section::None;
    let mut definition = Vec::new();
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

    (definition.join("\n").trim().to_string(), krs)
}

pub fn render_project_content(definition: &str, krs: &[PmKr]) -> String {
    let mut content = format!("## Definition\n\n{}\n\n## KRs", definition.trim());
    for kr in krs {
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
        let rendered = render_project_content("A measured bet.", &krs);
        assert_eq!(
            parse_project_content(&rendered),
            ("A measured bet.".to_string(), krs.clone())
        );

        let local = "# Project Name\n\nA measured bet.\n\n## KRs\n\n- One proof holds\n  across wrapped lines.\n";
        assert_eq!(
            parse_project_content(local),
            (
                "A measured bet.".to_string(),
                vec![PmKr {
                    text: "One proof holds across wrapped lines.".to_string(),
                    holds: false,
                }]
            )
        );
    }
}
