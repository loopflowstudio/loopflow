use axum::extract::State;
use axum::Json;
use time::OffsetDateTime;

use crate::lfd::http::dto::{HealthResponse, MetricsResponse, StatusResponse};
use crate::lfd::http::state::HttpState;
use crate::lfd::registration::RegistrationState;
use crate::lfd::types::{AgentStatus, WaveRunStatus};

pub async fn health_handler(State(state): State<HttpState>) -> Json<HealthResponse> {
    let counts = counts(&state).await;
    let registration = registration_state(&state)
        .await
        .map(|registration| registration.public_summary());
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
    let registration = registration_state(&state)
        .await
        .map(RegistrationState::sanitized);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::auth::{AuthFailureThrottle, AuthProvider};
    use crate::lfd::config::{ExecutorConfig, GitHubConfig, HttpSecurityConfig};
    use crate::lfd::events::EventHub;
    use crate::lfd::executor::WaveExecutor;
    use crate::lfd::output::OutputHub;
    use crate::lfd::registration::RegistrationClient;
    use crate::lfd::scheduler::Scheduler;
    use crate::lfd::sessions::SessionManager;
    use crate::lfd::store::{open_store, SharedStore, StorageConfig};
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::Mutex;

    async fn test_http_state(registration: Option<RegistrationClient>) -> HttpState {
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
        let sessions = SessionManager::new(store.clone());
        let executor = Arc::new(
            WaveExecutor::new(
                store.clone(),
                scheduler.clone(),
                output_hub.clone(),
                event_hub.clone(),
                sessions.clone(),
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
            auth: AuthProvider::Local {
                session_token: secrecy::SecretString::from("test-token".to_string()),
            },
            registration,
            started_at: OffsetDateTime::now_utc(),
            github: GitHubConfig::default(),
            http_security: HttpSecurityConfig::default(),
            auth_failure_throttle: AuthFailureThrottle::new(),
            ci_failure_cache: Arc::new(Mutex::new(std::collections::HashSet::new())),
            sessions,
        }
    }

    #[tokio::test]
    async fn health_handler_returns_public_registration_summary() {
        let registration = RegistrationClient::new("https://auth.example.test");
        registration
            .set_state_for_test(RegistrationState {
                enabled: true,
                registered: true,
                machine_id: Some("machine-123".to_string()),
                machine_name: Some("host-abc".to_string()),
                ..RegistrationState::default()
            })
            .await;
        let state = test_http_state(Some(registration)).await;

        let Json(payload) = health_handler(State(state)).await;
        let registration = payload.registration.expect("registration summary");
        assert!(registration.enabled);
        assert!(registration.registered);
        let value = serde_json::to_value(registration).expect("serialize summary");
        assert_eq!(value.as_object().expect("summary object").len(), 2);
    }

    #[tokio::test]
    async fn status_handler_sanitizes_registration_last_error() {
        let registration = RegistrationClient::new("https://auth.example.test");
        registration
            .set_state_for_test(RegistrationState {
                enabled: true,
                registered: true,
                last_error: Some(
                    "failed with Bearer abcdef0123456789abcdef0123456789 at /tmp/private".into(),
                ),
                machine_id: Some("machine-123".into()),
                machine_name: Some("host-abc".into()),
                ..RegistrationState::default()
            })
            .await;
        let state = test_http_state(Some(registration)).await;

        let Json(payload) = status_handler(State(state)).await;
        let registration = payload.registration.expect("registration state");
        let last_error = registration.last_error.expect("last_error");
        assert!(!last_error.contains("abcdef0123456789abcdef0123456789"));
        assert!(!last_error.contains("/tmp/private"));
        assert!(last_error.contains("[REDACTED_TOKEN]"));
        assert!(last_error.contains("[REDACTED_PATH]"));
        assert_eq!(registration.machine_id, Some("machine-123".to_string()));
        assert_eq!(registration.machine_name, Some("host-abc".to_string()));
    }
}
