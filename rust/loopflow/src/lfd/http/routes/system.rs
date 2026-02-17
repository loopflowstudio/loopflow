use axum::extract::State;
use axum::Json;
use time::OffsetDateTime;

use crate::lfd::http::dto::{HealthResponse, MetricsResponse, StatusResponse};
use crate::lfd::http::state::HttpState;
use crate::lfd::registration::RegistrationState;
use crate::lfd::types::{AgentStatus, WaveRunStatus};

pub async fn health_handler(State(state): State<HttpState>) -> Json<HealthResponse> {
    let counts = counts(&state).await;
    let registration = registration_state(&state).await;
    Json(HealthResponse {
        status: if counts.database_ok { "ok" } else { "degraded" }.to_string(),
        uptime_seconds: (OffsetDateTime::now_utc() - state.started_at).whole_seconds(),
        database: counts.database_ok,
        waves_running: counts.waves_running,
        agents_active: counts.agents_active,
        registration,
    })
}

pub async fn status_handler(State(state): State<HttpState>) -> Json<StatusResponse> {
    let counts = counts(&state).await;
    let registration = registration_state(&state).await;
    Json(StatusResponse {
        pid: std::process::id(),
        waves_defined: counts.waves_defined,
        waves_running: counts.waves_running,
        agents_active: counts.agents_active,
        slots_used: state.scheduler.slots_used(),
        slots_total: state.scheduler.max_slots() as u32,
        registration,
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
    let waves_fut = state.store.list_waves(None);
    let agents_fut = state.store.list_agents();
    let wave_runs_fut = state.store.list_wave_runs(None, None);
    let health_fut = state.store.health_check();
    let (waves, agents, wave_runs, health) =
        tokio::join!(waves_fut, agents_fut, wave_runs_fut, health_fut);

    let waves = waves.unwrap_or_default();
    let agents = agents.unwrap_or_default();
    let wave_runs = wave_runs.unwrap_or_default();
    let database_ok = health.is_ok();

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

async fn registration_state(state: &HttpState) -> Option<RegistrationState> {
    let client = state.registration.as_ref()?;
    Some(client.status().await)
}
