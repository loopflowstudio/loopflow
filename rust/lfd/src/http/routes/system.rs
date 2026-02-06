use axum::extract::State;
use axum::Json;
use time::OffsetDateTime;

use crate::http::dto::{HealthResponse, MetricsResponse, StatusResponse};
use crate::http::state::HttpState;
use crate::types::{AgentStatus, WaveRunStatus};

pub async fn health_handler(State(state): State<HttpState>) -> Json<HealthResponse> {
    let counts = counts(&state).await;
    Json(HealthResponse {
        status: if counts.database_ok { "ok" } else { "degraded" }.to_string(),
        uptime_seconds: (OffsetDateTime::now_utc() - state.started_at).whole_seconds(),
        database: counts.database_ok,
        waves_running: counts.waves_running,
        agents_active: counts.agents_active,
    })
}

pub async fn status_handler(State(state): State<HttpState>) -> Json<StatusResponse> {
    let counts = counts(&state).await;
    Json(StatusResponse {
        pid: std::process::id(),
        waves_defined: counts.waves_defined,
        waves_running: counts.waves_running,
        agents_active: counts.agents_active,
        slots_used: state.scheduler.slots_used(),
        slots_total: state.scheduler.max_slots() as u32,
    })
}

pub async fn metrics_handler(State(state): State<HttpState>) -> Json<MetricsResponse> {
    let counts = counts(&state).await;
    Json(MetricsResponse {
        waves_total: counts.waves_defined,
        waves_running: counts.waves_running,
        agents_active: counts.agents_active,
        slots_used: state.scheduler.slots_used(),
        slots_total: state.scheduler.max_slots() as u32,
    })
}

struct Counts {
    waves_defined: u32,
    waves_running: u32,
    agents_active: u32,
    database_ok: bool,
}

async fn counts(state: &HttpState) -> Counts {
    let store = state.store.clone();
    let waves = tokio::task::spawn_blocking(move || store.list_waves(None))
        .await
        .ok()
        .and_then(|result| result.ok())
        .unwrap_or_default();

    let store = state.store.clone();
    let agents = tokio::task::spawn_blocking(move || store.list_agents())
        .await
        .ok()
        .and_then(|result| result.ok())
        .unwrap_or_default();

    let store = state.store.clone();
    let wave_runs = tokio::task::spawn_blocking(move || store.list_wave_runs(None, None))
        .await
        .ok()
        .and_then(|result| result.ok())
        .unwrap_or_default();

    let store = state.store.clone();
    let database_ok = tokio::task::spawn_blocking(move || store.health_check())
        .await
        .ok()
        .and_then(|result| result.ok())
        .is_some();

    let waves_defined = waves.len() as u32;
    let waves_running = wave_runs
        .iter()
        .filter(|run| {
            matches!(
                run.status,
                WaveRunStatus::Running | WaveRunStatus::Waiting | WaveRunStatus::Pending
            )
        })
        .count() as u32;
    let agents_active = agents
        .iter()
        .filter(|agent| matches!(agent.status, AgentStatus::Running | AgentStatus::Waiting))
        .count() as u32;

    Counts {
        waves_defined,
        waves_running,
        agents_active,
        database_ok,
    }
}
