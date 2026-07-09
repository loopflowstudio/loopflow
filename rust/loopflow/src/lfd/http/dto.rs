use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use time::OffsetDateTime;

use crate::lfd::queue::QueueRunView;
use crate::lfd::types::{AttentionItem, LivePullRequestState, Run, RunStatus, Session};

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
pub struct CommitEntryDto {
    pub sha: String,
    pub message: String,
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
    pub has_stale_pr_state: bool,
    pub workers: u32,
    /// The single repo this wave targets (wave = 1 repo). The remaining
    /// execution fields below describe that repo's live git + PR state, inferred
    /// at DTO-build time.
    pub repo: String,
    pub iteration: u32,
    /// Live worktree path inferred from git at build time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_worktree: Option<String>,
    /// Live branch inferred from git at build time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_branch: Option<String>,
    pub commits: Vec<CommitEntryDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_stat: Option<String>,
    pub open_pr_count: u32,
    pub stack_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_run: Option<RunDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr: Option<PullRequestDto>,
    /// Parent wave in the chord tree. `null` for a root wave. Always emitted
    /// (no `skip_serializing_if`) so the Python/Swift mirrors stay in lockstep.
    pub parent_wave_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunDto {
    pub id: String,
    pub object: String,
    pub wave_id: String,
    pub flow: String,
    pub task: Option<String>,
    pub repo: String,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub iteration: u32,
    pub step_index: u32,
    pub status: String,
    pub local_worktree: String,
    pub remote_branch: String,
    pub target_branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr: Option<PullRequestDto>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub error: Option<String>,
    pub flow_parents: Vec<String>,
    pub stack_position: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_pr_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_pr_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_pr_is_draft: Option<bool>,
    pub pr_state_stale: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_block_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_blocked_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PullRequestDto {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
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
pub struct CreateSessionRequestDto {
    pub wave_id: String,
    pub flow: String,
    pub worktree: String,
    pub agent: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSessionResponseDto {
    pub session: SessionDto,
    pub connection: SessionConnectionInfoDto,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionConnectionInfoDto {
    pub kind: String,
    pub session_name: String,
    pub host: String,
    pub cwd: String,
    pub status: String,
}

pub fn session_connection_info_dto(session: &Session, host: String) -> SessionConnectionInfoDto {
    SessionConnectionInfoDto {
        kind: "tmux".to_string(),
        session_name: session.tmux_name.clone(),
        host,
        cwd: session.cwd.clone(),
        status: session.status.as_str().to_string(),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WaveAgentTreeSessionDto {
    pub session: SessionDto,
    pub connection: Option<SessionConnectionInfoDto>,
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
pub struct StopWaveResponse {
    pub stopped: bool,
}

#[derive(Debug, Serialize)]
pub struct LandWaveResponse {
    pub merged: bool,
}

#[derive(Debug, Serialize)]
pub struct NextWaveResponse {
    pub new_branch: String,
}

#[derive(Debug, Serialize)]
pub struct CombineResponse {
    pub ok: bool,
    pub result: CombineResponseResult,
}

#[derive(Debug, Serialize)]
pub struct CombineResponseResult {
    pub new_pr_url: Option<String>,
    pub closed_prs: Vec<u64>,
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

pub fn run_dto(
    run: Run,
    live_pr_state: Option<&LivePullRequestState>,
    pr_state_stale: bool,
    queue_view: Option<&QueueRunView>,
) -> RunDto {
    RunDto {
        id: run.id.to_string(),
        object: "run".to_string(),
        wave_id: run.wave_id.to_string(),
        flow: run.flow.clone(),
        task: run.task.clone(),
        repo: run.repo.clone(),
        direction: run.direction.clone(),
        area: run.area.clone(),
        iteration: run.iteration,
        step_index: run.step_index,
        status: run_status_str(run.status),
        local_worktree: run.worktree,
        remote_branch: run.branch,
        target_branch: run.target_branch,
        pr: run.pr.map(|pr| PullRequestDto {
            url: pr.url,
            number: pr.number,
            state: pr.state,
            title: pr.title,
            branch: pr.branch,
        }),
        started_at: format_datetime(run.started_at),
        ended_at: format_datetime(run.ended_at),
        error: run.error,
        flow_parents: run.flow_parents,
        stack_position: run.stack_position,
        parent_run_id: run.parent_run_id.map(|value| value.to_string()),
        parent_pr_number: run.parent_pr_number,
        live_pr_state: live_pr_state.map(|value| value.state.as_str().to_string()),
        live_pr_is_draft: live_pr_state.map(|value| value.is_draft),
        pr_state_stale,
        queue_role: queue_view.map(|value| value.role.as_str().to_string()),
        queue_block_reason: queue_view
            .and_then(|value| value.block_reason.map(|reason| reason.as_str().to_string())),
        queue_blocked_at: format_datetime(queue_view.and_then(|value| value.blocked_at)),
        next_action: queue_view.map(|value| value.next_action.as_str().to_string()),
        created_at: format_datetime(run.started_at),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeDto {
    pub object: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub merged: bool,
    pub prunable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave_id: Option<String>,
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

pub fn run_status_str(status: RunStatus) -> String {
    match status {
        RunStatus::Pending => "pending",
        RunStatus::Running => "running",
        RunStatus::Waiting => "waiting",
        RunStatus::Completed => "completed",
        RunStatus::Failed => "failed",
        RunStatus::Unspecified => "unknown",
    }
    .to_string()
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
            workers: 1,
            created_at: None,
            flow_steps: Vec::new(),
            has_stale_pr_state: false,
            repo: "/home/user/project".to_string(),
            iteration: 0,
            local_worktree: None,
            remote_branch: None,
            commits: Vec::new(),
            diff_stat: None,
            open_pr_count: 0,
            stack_count: 0,
            active_run: None,
            pr: None,
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
