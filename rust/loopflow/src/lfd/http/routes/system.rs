use axum::extract::State;
use axum::Json;
use time::OffsetDateTime;

use crate::lfd::http::dto::{HealthResponse, MetricsResponse, StatusResponse};
use crate::lfd::http::state::HttpState;
use crate::lfd::types::{ExecutionProcessStatus, RunStatus};

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
    let waves_fut = state.store.list_waves(None);
    let agents_fut = state.store.list_agents();
    let runs_fut = state.store.list_runs(None, None);
    let health_fut = state.store.health_check();
    let (waves, agents, runs, health) = tokio::join!(waves_fut, agents_fut, runs_fut, health_fut);

    let waves = waves.unwrap_or_default();
    let agents = agents.unwrap_or_default();
    let runs = runs.unwrap_or_default();
    let database_ok = health.is_ok();

    let waves_defined = waves.len() as u32;
    let waves_running = runs
        .iter()
        .filter(|run| {
            matches!(
                run.status,
                RunStatus::Running | RunStatus::Waiting | RunStatus::Pending
            )
        })
        .count() as u32;
    let agents_active = agents
        .iter()
        .filter(|agent| {
            matches!(
                agent.status,
                ExecutionProcessStatus::Running | ExecutionProcessStatus::Waiting
            )
        })
        .count() as u32;

    Counts {
        waves_defined,
        waves_running,
        agents_active,
        database_ok,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::auth::{AuthFailureThrottle, AuthProvider};
    use crate::lfd::config::{ExecutorConfig, GitHubConfig, HttpSecurityConfig};
    use crate::lfd::events::EventHub;
    use crate::lfd::executor::WaveExecutor;
    use crate::lfd::output::OutputHub;
    use crate::lfd::provider_auth::ProviderAuthService;
    use crate::lfd::scheduler::Scheduler;
    use crate::lfd::store::{open_store, SharedStore, StorageConfig};
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::Mutex;

    async fn test_http_state() -> HttpState {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        let scheduler = Arc::new(Scheduler::new(1));
        let output_hub = OutputHub::new(128, tmp.path().join("output"));
        let event_hub = EventHub::new(128);
        let executor = Arc::new(
            WaveExecutor::new(
                store.clone(),
                scheduler.clone(),
                output_hub.clone(),
                event_hub.clone(),
                ExecutorConfig::default(),
                GitHubConfig::default(),
            )
            .expect("build executor"),
        );

        HttpState {
            store: store.clone(),
            scheduler,
            executor,
            event_hub,
            output_hub,
            provider_auth: ProviderAuthService::new(store.clone()),
            auth: AuthProvider::Bearer {
                session_token: secrecy::SecretString::from("test-token".to_string()),
            },
            started_at: OffsetDateTime::now_utc(),
            github: GitHubConfig::default(),
            http_security: HttpSecurityConfig::default(),
            auth_failure_throttle: AuthFailureThrottle::new(),
            ci_failure_cache: Arc::new(Mutex::new(std::collections::HashSet::new())),
        }
    }

    #[tokio::test]
    async fn health_handler_reports_core_status() {
        let state = test_http_state().await;

        let Json(payload) = health_handler(State(state)).await;
        assert_eq!(payload.waves_running, 0);
        assert_eq!(payload.agents_active, 0);
    }

    #[tokio::test]
    async fn status_handler_reports_slot_counts() {
        let state = test_http_state().await;

        let Json(payload) = status_handler(State(state)).await;
        assert_eq!(payload.slots_total, 1);
        assert_eq!(payload.slots_used, 0);
    }
}
