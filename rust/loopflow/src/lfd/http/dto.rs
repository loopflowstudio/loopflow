use serde::Serialize;
use std::collections::HashMap;
use time::OffsetDateTime;

use crate::lfd::queue::QueueRunView;
use crate::lfd::registration::{RegistrationPublicSummary, RegistrationState};
use crate::lfd::types::{
    ActivationLog, ChatMemoryBlock, ChatMessage, Chord, LivePullRequestState, Signal, Stimulus,
    WaveRun, WaveRunStatus,
};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: i64,
    pub database: bool,
    pub waves_running: u32,
    pub agents_active: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration: Option<RegistrationPublicSummary>,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub pid: u32,
    pub waves_defined: u32,
    pub waves_running: u32,
    pub agents_active: u32,
    pub slots_used: u32,
    pub slots_total: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration: Option<RegistrationState>,
}

#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    pub waves_total: u32,
    pub waves_running: u32,
    pub agents_active: u32,
    pub slots_used: u32,
    pub slots_total: u32,
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

#[derive(Debug, Serialize)]
pub struct CommitEntryDto {
    pub sha: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct WaveDto {
    pub id: String,
    pub object: String,
    pub name: String,
    pub repo: String,
    pub flow: String,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_agents: Option<HashMap<String, String>>,
    pub created_at: Option<String>,
    pub status: String,
    pub iteration: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_worktree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_branch: Option<String>,
    pub commits: Vec<CommitEntryDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_stat: Option<String>,
    pub flow_steps: Vec<String>,
    pub open_pr_count: u32,
    pub stack_count: u32,
    pub has_stale_pr_state: bool,
    pub serialized: bool,
    pub stimuli: Vec<StimulusDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_run: Option<WaveRunDto>,
}

#[derive(Debug, Serialize)]
pub struct ChordDto {
    pub id: String,
    pub object: String,
    pub name: String,
    pub is_default: bool,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WaveRunDto {
    pub id: String,
    pub object: String,
    pub wave_id: String,
    pub flow: String,
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
pub struct RunWaveResponse {
    pub started: bool,
    pub wave_id: String,
    pub wave_run_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StimulusDto {
    pub id: String,
    pub kind: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_wave_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_main_sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_triggered_at: Option<i64>,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ActivationLogDto {
    pub id: String,
    pub object: String,
    pub wave_id: String,
    pub stimulus_id: String,
    pub source: String,
    pub reason: String,
    pub outcome: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatMemoryBlockDto {
    pub object: String,
    pub name: String,
    pub content: String,
    pub position: u32,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatStartedDto {
    pub object: String,
    pub wave_id: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct ChatMessageDto {
    pub id: String,
    pub object: String,
    pub role: String,
    pub content: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StopWaveResponse {
    pub stopped: bool,
}

#[derive(Debug, Serialize)]
pub struct RestartStepResponse {
    pub restarted: bool,
    pub wave_id: String,
    pub wave_run_id: String,
    pub step_index: u32,
}

#[derive(Debug, Serialize)]
pub struct ContinueWaveResponse {
    pub continued: bool,
    pub wave_id: String,
    pub wave_run_id: String,
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

pub fn signal_str(signal: Signal) -> &'static str {
    match signal {
        Signal::Loop => "loop",
        Signal::Watch => "watch",
        Signal::Cron => "cron",
        Signal::Once => "once",
        Signal::Listen => "listen",
        Signal::CiFailure => "ci_failure",
        Signal::Unspecified => "unspecified",
    }
}

pub fn chord_dto(chord: Chord) -> ChordDto {
    ChordDto {
        id: chord.id.to_string(),
        object: "chord".to_string(),
        name: chord.name,
        is_default: chord.is_default,
        created_at: format_datetime(chord.created_at),
    }
}

pub fn format_datetime(datetime: Option<OffsetDateTime>) -> Option<String> {
    datetime?
        .format(&time::format_description::well_known::Rfc3339)
        .ok()
}

pub fn wave_run_dto(
    run: WaveRun,
    live_pr_state: Option<&LivePullRequestState>,
    pr_state_stale: bool,
    queue_view: Option<&QueueRunView>,
) -> WaveRunDto {
    WaveRunDto {
        id: run.id.to_string(),
        object: "wave_run".to_string(),
        wave_id: run.wave_id.to_string(),
        flow: run.snapshot.flow.clone(),
        repo: run.snapshot.repo.clone(),
        direction: run.snapshot.direction.clone(),
        area: run.snapshot.area.clone(),
        iteration: run.iteration,
        step_index: run.step_index,
        status: wave_run_status_str(run.status),
        local_worktree: run.worktree,
        remote_branch: run.branch,
        target_branch: run.target_branch,
        pr: run.snapshot.pr.map(|pr| PullRequestDto {
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

pub fn stimulus_dto(s: Stimulus) -> StimulusDto {
    StimulusDto {
        id: s.id.to_string(),
        kind: signal_str(s.signal).to_string(),
        enabled: s.enabled,
        flow: s.flow,
        source_wave_id: s.source_wave_id.map(|value| value.to_string()),
        cron: s.cron,
        last_main_sha: s.last_main_sha,
        last_triggered_at: s.last_triggered_at,
        created_at: format_datetime(s.created_at),
    }
}

pub fn activation_log_dto(log: ActivationLog) -> ActivationLogDto {
    ActivationLogDto {
        id: log.id.to_string(),
        object: "activation_log".to_string(),
        wave_id: log.wave_id.to_string(),
        stimulus_id: log.stimulus_id.to_string(),
        source: log.source.as_str().to_string(),
        reason: log.reason,
        outcome: log.outcome.as_str().to_string(),
        created_at: format_datetime(time::OffsetDateTime::from_unix_timestamp(log.created_at).ok()),
    }
}

pub fn chat_memory_block_dto(block: ChatMemoryBlock) -> ChatMemoryBlockDto {
    ChatMemoryBlockDto {
        object: "memory_block".to_string(),
        name: block.name,
        content: block.content,
        position: block.position,
        updated_at: format_datetime(block.updated_at),
    }
}

pub fn chat_message_dto(message: ChatMessage) -> ChatMessageDto {
    ChatMessageDto {
        id: message.id.to_string(),
        object: "chat_message".to_string(),
        role: message.role,
        content: message.content,
        created_at: format_datetime(Some(message.created_at)),
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
    pub wave_count: u32,
    pub registered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added_at: Option<String>,
}

pub fn wave_run_status_str(status: WaveRunStatus) -> String {
    match status {
        WaveRunStatus::Pending => "pending",
        WaveRunStatus::Running => "running",
        WaveRunStatus::Waiting => "waiting",
        WaveRunStatus::Completed => "completed",
        WaveRunStatus::Failed => "failed",
        WaveRunStatus::Unspecified => "unknown",
    }
    .to_string()
}
