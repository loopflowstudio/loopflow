use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use time::OffsetDateTime;

use crate::lfd::types::{AttentionItem, Session};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: i64,
    pub database: bool,
    pub waves_running: u32,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub pid: u32,
    /// What this daemon is: one honest line, additive-safe for clients.
    pub role: String,
    pub waves_defined: u32,
    pub waves_running: u32,
}

#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    pub waves_total: u32,
    pub waves_running: u32,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub object: String,
    pub data: Vec<T>,
    pub has_more: bool,
}

impl<T> ListResponse<T> {
    pub fn new(data: Vec<T>, has_more: bool) -> Self {
        Self {
            object: "list".to_string(),
            data,
            has_more,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WaveDto {
    pub id: String,
    pub object: String,
    pub name: String,
    pub goal: String,
    pub metrics: Vec<String>,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_agents: Option<HashMap<String, String>>,
    pub created_at: Option<String>,
    /// Wave-level status rolled up over `repos` (see `Wave::status`).
    pub status: String,
    pub flow_steps: Vec<String>,
    pub task_capacity: u32,
    /// The single repository whose main checkout is this Wave's control plane.
    pub repo: String,
    pub iteration: u32,
    /// Parent wave in the chord tree. `null` for a root wave. Always emitted
    /// (no `skip_serializing_if`) so the Python/Swift mirrors stay in lockstep.
    pub parent_wave_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttentionItemDto {
    pub id: String,
    pub object: String,
    pub wave_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub kind: String,
    pub status: String,
    pub title: String,
    pub summary: String,
    pub context: serde_json::Value,
    pub surfaced_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<String>,
}

pub fn attention_item_dto(item: AttentionItem) -> AttentionItemDto {
    AttentionItemDto {
        id: item.id.to_string(),
        object: "attention_item".to_string(),
        wave_id: item.wave_id.to_string(),
        run_id: item.run_id.map(|value| value.to_string()),
        kind: item.kind.as_str().to_string(),
        status: item.status.as_str().to_string(),
        title: item.title,
        summary: item.summary,
        context: item.context,
        surfaced_at: format_datetime(Some(item.surfaced_at)).unwrap_or_default(),
        viewed_at: format_datetime(item.viewed_at),
        resolved_at: format_datetime(item.resolved_at),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionDto {
    pub id: String,
    pub object: String,
    pub wave_id: String,
    pub run_id: Option<String>,
    pub parent_session_id: Option<String>,
    #[serde(rename = "use")]
    pub session_use: String,
    pub skill: String,
    pub agent: String,
    pub cwd: String,
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub source: String,
    pub tmux_name: String,
    pub status: String,
    pub created_at: String,
    pub attached_at: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

pub fn session_dto(session: Session) -> SessionDto {
    SessionDto {
        id: session.id.to_string(),
        object: "session".to_string(),
        wave_id: session.wave_id.to_string(),
        run_id: session.run_id.map(|value| value.to_string()),
        parent_session_id: session.parent_session_id.map(|value| value.to_string()),
        session_use: session.session_use.as_str().to_string(),
        skill: session.skill,
        agent: session.agent,
        cwd: session.cwd,
        argv: session.argv,
        env: session.env,
        source: session.source,
        tmux_name: session.tmux_name,
        status: session.status.as_str().to_string(),
        created_at: format_datetime(Some(session.created_at)).unwrap_or_default(),
        attached_at: format_datetime(session.attached_at),
        started_at: format_datetime(session.started_at),
        completed_at: format_datetime(session.completed_at),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WaveAgentTreeSessionDto {
    pub session: SessionDto,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WaveAgentTreeDto {
    pub object: String,
    pub id: String,
    pub wave: WaveDto,
    pub child_waves: Vec<WaveDto>,
    pub sessions: Vec<WaveAgentTreeSessionDto>,
}

#[derive(Debug, Serialize)]
pub struct DeletedResourceResponse {
    pub id: String,
    pub object: String,
    pub deleted: bool,
}

pub fn format_datetime(datetime: Option<OffsetDateTime>) -> Option<String> {
    datetime?
        .format(&time::format_description::well_known::Rfc3339)
        .ok()
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoDto {
    pub object: String,
    pub path: String,
    pub name: String,
    pub repo_id: String,
    pub wave_count: u32,
    pub registered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added_at: Option<String>,
}

#[cfg(test)]
mod contract_tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(name);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
    }

    /// The golden nested-wave payload, shared with the Python and Swift
    /// contract suites. Parsing and shape are covered by
    /// `tests/dto_fixtures.rs`; these tests only pin required-field behavior.
    fn wave_fixture() -> serde_json::Value {
        serde_json::from_str(&fixture("wave.json")).expect("wave.json parses")
    }

    #[test]
    fn wave_goal_serializes_required_value() {
        let wave = WaveDto {
            id: "wave_abc123".to_string(),
            object: "wave".to_string(),
            name: "engbot".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            direction: Vec::new(),
            area: Vec::new(),
            agent: None,
            skill_agents: None,
            status: "idle".to_string(),
            task_capacity: 1,
            created_at: None,
            flow_steps: Vec::new(),
            repo: "/home/user/project".to_string(),
            iteration: 0,
            parent_wave_id: None,
        };

        let json = serde_json::to_value(&wave).unwrap();
        assert_eq!(json["goal"], "ship-roadmap");
        assert_eq!(json["metrics"], serde_json::json!([]));
        assert_eq!(json["repo"], "/home/user/project");
    }

    #[test]
    fn wave_goal_is_required() {
        let mut json = wave_fixture();
        json.as_object_mut()
            .expect("wave payload should be an object")
            .remove("goal");

        let err = serde_json::from_value::<WaveDto>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `goal`"));
    }

    #[test]
    fn wave_metrics_is_required() {
        let mut json = wave_fixture();
        json.as_object_mut()
            .expect("wave payload should be an object")
            .remove("metrics");

        let err = serde_json::from_value::<WaveDto>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `metrics`"));
    }

    #[test]
    fn wave_repo_is_required() {
        let mut json = wave_fixture();
        json.as_object_mut()
            .expect("wave payload should be an object")
            .remove("repo");

        let err = serde_json::from_value::<WaveDto>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `repo`"));
    }
}
