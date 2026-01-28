use std::sync::Arc;

use axum::{routing::get, Json, Router};
use serde::Serialize;
use time::OffsetDateTime;

use crate::scheduler::Scheduler;
use crate::store::SharedStore;

#[derive(Clone)]
pub struct HttpState {
    pub store: SharedStore,
    pub scheduler: Arc<Scheduler>,
    pub started_at: OffsetDateTime,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    uptime_seconds: i64,
    database: bool,
    waves_running: u32,
    step_runs_active: u32,
}

#[derive(Serialize)]
struct StatusResponse {
    pid: u32,
    waves_defined: u32,
    waves_running: u32,
    step_runs_active: u32,
    slots_used: u32,
    slots_total: u32,
}

#[derive(Serialize)]
struct MetricsResponse {
    waves_total: u32,
    waves_running: u32,
    step_runs_active: u32,
    slots_used: u32,
    slots_total: u32,
}

pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/status", get(status_handler))
        .route("/metrics", get(metrics_handler))
        .with_state(state)
}

async fn health_handler(state: axum::extract::State<HttpState>) -> Json<HealthResponse> {
    let counts = counts(&state).await;
    Json(HealthResponse {
        status: if counts.database_ok { "ok" } else { "degraded" }.to_string(),
        uptime_seconds: (OffsetDateTime::now_utc() - state.started_at).whole_seconds(),
        database: counts.database_ok,
        waves_running: counts.waves_running,
        step_runs_active: counts.step_runs_active,
    })
}

async fn status_handler(state: axum::extract::State<HttpState>) -> Json<StatusResponse> {
    let counts = counts(&state).await;
    Json(StatusResponse {
        pid: std::process::id(),
        waves_defined: counts.waves_defined,
        waves_running: counts.waves_running,
        step_runs_active: counts.step_runs_active,
        slots_used: state.scheduler.slots_used(),
        slots_total: state.scheduler.max_slots() as u32,
    })
}

async fn metrics_handler(state: axum::extract::State<HttpState>) -> Json<MetricsResponse> {
    let counts = counts(&state).await;
    Json(MetricsResponse {
        waves_total: counts.waves_defined,
        waves_running: counts.waves_running,
        step_runs_active: counts.step_runs_active,
        slots_used: state.scheduler.slots_used(),
        slots_total: state.scheduler.max_slots() as u32,
    })
}

struct Counts {
    waves_defined: u32,
    waves_running: u32,
    step_runs_active: u32,
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
    let step_runs = tokio::task::spawn_blocking(move || store.list_step_runs())
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
    let waves_running = waves
        .iter()
        .filter(|wave| wave.status == crate::proto::control::WaveStatus::WaveRunning as i32)
        .count() as u32;
    let step_runs_active = step_runs
        .iter()
        .filter(|run| {
            run.status == crate::proto::control::StepRunStatus::StepRunning as i32
                || run.status == crate::proto::control::StepRunStatus::StepWaiting as i32
        })
        .count() as u32;

    Counts {
        waves_defined,
        waves_running,
        step_runs_active,
        database_ok,
    }
}
