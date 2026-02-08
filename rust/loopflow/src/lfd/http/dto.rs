use serde::Serialize;
use time::OffsetDateTime;

use crate::lfd::registration::RegistrationState;
use crate::lfd::types::{Wave, WaveRun, WaveRunStatus};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: i64,
    pub database: bool,
    pub waves_running: u32,
    pub agents_active: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration: Option<RegistrationState>,
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
pub struct WaveDto {
    pub id: String,
    pub object: String,
    pub name: String,
    pub repo: String,
    pub flow: String,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub created_at: Option<String>,
    pub status: String,
    pub iteration: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_worktree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_run: Option<WaveRunDto>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr: Option<PullRequestDto>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub error: Option<String>,
    pub flow_parents: Vec<String>,
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
pub struct StopWaveResponse {
    pub stopped: bool,
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

pub fn wave_run_dto(run: WaveRun) -> WaveRunDto {
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
        created_at: format_datetime(run.started_at),
    }
}

pub fn wave_dto(
    wave: &Wave,
    active_run: Option<WaveRunDto>,
    local_worktree: Option<String>,
    remote_branch: Option<String>,
) -> WaveDto {
    WaveDto {
        id: wave.id.to_string(),
        object: "wave".to_string(),
        name: wave.name.clone(),
        repo: wave.repo.clone(),
        flow: wave.flow.clone(),
        direction: wave.direction.clone(),
        area: wave.area.clone(),
        created_at: format_datetime(wave.created_at),
        status: wave.status.as_str().to_string(),
        iteration: wave.iteration,
        local_worktree,
        remote_branch,
        active_run,
    }
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
