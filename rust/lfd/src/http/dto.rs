use serde::Serialize;
use time::OffsetDateTime;

use crate::registration::RegistrationState;
use crate::types::{Wave, WaveRun, WaveRunStatus};

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: i64,
    pub database: bool,
    pub waves_running: u32,
    pub agents_active: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration: Option<RegistrationState>,
}

#[derive(Serialize)]
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

#[derive(Serialize)]
pub struct MetricsResponse {
    pub waves_total: u32,
    pub waves_running: u32,
    pub agents_active: u32,
    pub slots_used: u32,
    pub slots_total: u32,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize)]
pub struct WaveSummaryDto {
    pub id: String,
    pub name: String,
    pub repo: String,
    pub flow: String,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub paused: bool,
    pub created_at: Option<String>,
}

#[derive(Serialize)]
pub struct WaveRunSummaryDto {
    pub id: String,
    pub wave_id: String,
    pub iteration: u32,
    pub step_index: u32,
    pub status: String,
    pub worktree: String,
    pub branch: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub error: Option<String>,
    pub flow_parents: Vec<String>,
}

#[derive(Serialize)]
pub struct WaveViewDto {
    pub wave: WaveSummaryDto,
    pub status: String,
    pub active_run: Option<WaveRunSummaryDto>,
}

#[derive(Serialize)]
pub struct ListWavesResponse {
    pub waves: Vec<WaveViewDto>,
}

#[derive(Serialize)]
pub struct WaveResponse {
    pub wave: WaveViewDto,
}

#[derive(Serialize)]
pub struct RunWaveResponse {
    pub started: bool,
    pub wave_id: String,
    pub wave_run_id: Option<String>,
}

#[derive(Serialize)]
pub struct StopWaveResponse {
    pub stopped: bool,
}

#[derive(Serialize)]
pub struct LandWaveResponse {
    pub merged: bool,
}

pub fn format_datetime(datetime: Option<OffsetDateTime>) -> Option<String> {
    datetime?
        .format(&time::format_description::well_known::Rfc3339)
        .ok()
}

pub fn wave_run_summary(run: WaveRun) -> WaveRunSummaryDto {
    WaveRunSummaryDto {
        id: run.id.to_string(),
        wave_id: run.wave_id.to_string(),
        iteration: run.iteration,
        step_index: run.step_index,
        status: wave_run_status_str(run.status),
        worktree: run.worktree,
        branch: run.branch,
        started_at: format_datetime(run.started_at),
        ended_at: format_datetime(run.ended_at),
        error: run.error,
        flow_parents: run.flow_parents,
    }
}

pub fn wave_summary(wave: &Wave) -> WaveSummaryDto {
    WaveSummaryDto {
        id: wave.id.to_string(),
        name: wave.name.clone(),
        repo: wave.repo.clone(),
        flow: wave.flow.clone(),
        direction: wave.direction.clone(),
        area: wave.area.clone(),
        paused: wave.paused,
        created_at: format_datetime(wave.created_at),
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
