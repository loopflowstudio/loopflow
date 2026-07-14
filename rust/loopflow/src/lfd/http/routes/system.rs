use axum::extract::State;
use axum::Json;
use time::OffsetDateTime;

use crate::lfd::http::dto::{HealthResponse, MetricsResponse, StatusResponse};
use crate::lfd::http::state::HttpState;
use crate::lfd::types::{SessionUse, LIVE_SESSION_STATUSES};

pub async fn health_handler(State(state): State<HttpState>) -> Json<HealthResponse> {
    let counts = counts(&state).await;
    Json(HealthResponse {
        status: if counts.database_ok { "ok" } else { "degraded" }.to_string(),
        uptime_seconds: (OffsetDateTime::now_utc() - state.started_at).whole_seconds(),
        database: counts.database_ok,
        waves_running: counts.waves_running,
    })
}

pub async fn status_handler(State(state): State<HttpState>) -> Json<StatusResponse> {
    let counts = counts(&state).await;
    Json(StatusResponse {
        pid: std::process::id(),
        role: "gatekeeper: reads, push, webhook ingress; mutations exec lf".to_string(),
        waves_defined: counts.waves_defined,
        waves_running: counts.waves_running,
    })
}

pub async fn metrics_handler(State(state): State<HttpState>) -> Json<MetricsResponse> {
    let counts = counts(&state).await;
    Json(MetricsResponse {
        waves_total: counts.waves_defined,
        waves_running: counts.waves_running,
    })
}

struct Counts {
    waves_defined: u32,
    waves_running: u32,
    database_ok: bool,
}

async fn counts(state: &HttpState) -> Counts {
    let waves_fut = state.store.list_waves(None);
    let sessions_fut = state
        .store
        .list_control_sessions(None, Some(LIVE_SESSION_STATUSES));
    let health_fut = state.store.health_check();
    let (waves, sessions, health) = tokio::join!(waves_fut, sessions_fut, health_fut);

    let waves = waves.unwrap_or_default();
    let sessions = sessions.unwrap_or_default();
    let database_ok = health.is_ok();

    let waves_defined = waves.len() as u32;
    let waves_running = sessions
        .iter()
        .filter(|session| session.session_use == SessionUse::WaveAgent)
        .map(|session| &session.wave_id)
        .collect::<std::collections::HashSet<_>>()
        .len() as u32;

    Counts {
        waves_defined,
        waves_running,
        database_ok,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::http::routes::test_helpers::test_http_state;

    #[tokio::test]
    async fn health_handler_reports_core_status() {
        let state = test_http_state().await;

        let Json(payload) = health_handler(State(state)).await;
        assert_eq!(payload.waves_running, 0);
        assert!(payload.database);
    }

    #[tokio::test]
    async fn status_handler_names_the_gatekeeper_role() {
        let state = test_http_state().await;

        let Json(payload) = status_handler(State(state)).await;
        assert_eq!(payload.waves_defined, 0);
        assert!(payload.role.contains("gatekeeper"));
    }
}
