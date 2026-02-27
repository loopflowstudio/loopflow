use axum::extract::State;
use axum::Json;

use crate::lfd::http::state::HttpState;
use crate::lfd::http::ApiResult;
use crate::lfd::provider_auth::ProviderAuthSnapshot;
use crate::lfd::provider_models::{
    billing_for_provider, model_capable_providers, models_for_provider, ProviderInfo,
};

pub async fn list_providers_handler(
    State(state): State<HttpState>,
) -> ApiResult<Vec<ProviderInfo>> {
    let mut providers = Vec::with_capacity(model_capable_providers().len());
    for provider in model_capable_providers() {
        let ProviderAuthSnapshot { status, .. } = state
            .provider_auth
            .status(provider)
            .await
            .map_err(super::auth::map_auth_error)?;
        providers.push(ProviderInfo {
            provider,
            auth_status: status.as_str().to_string(),
            login: status.login(),
            billing: billing_for_provider(provider),
            models: models_for_provider(provider).to_vec(),
        });
    }
    Ok(Json(providers))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use tempfile::tempdir;
    use time::OffsetDateTime;
    use tokio::sync::Mutex;

    use super::*;
    use crate::lfd::auth::{AuthFailureThrottle, AuthProvider};
    use crate::lfd::config::{ExecutorConfig, GitHubConfig, HttpSecurityConfig};
    use crate::lfd::events::EventHub;
    use crate::lfd::executor::WaveExecutor;
    use crate::lfd::output::OutputHub;
    use crate::lfd::provider_auth::{
        AuthBroker, AuthError, AuthFlowHandle, AuthStatus, Provider, ProviderAuthService,
    };
    use crate::lfd::scheduler::Scheduler;
    use crate::lfd::sessions::SessionManager;
    use crate::lfd::store::{open_store, SharedStore, StorageConfig};

    #[derive(Debug)]
    struct FakeBroker {
        provider: Provider,
        status: AuthStatus,
    }

    #[async_trait]
    impl AuthBroker for FakeBroker {
        fn provider(&self) -> Provider {
            self.provider
        }

        async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError> {
            Err(AuthError::CommandFailed {
                provider: self.provider,
                message: "start_auth not needed for this test".to_string(),
            })
        }

        async fn check_status(&self) -> Result<AuthStatus, AuthError> {
            Ok(self.status.clone())
        }

        async fn disconnect(&self) -> Result<(), AuthError> {
            Ok(())
        }
    }

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
        let provider_auth = ProviderAuthService::with_brokers(vec![
            Arc::new(FakeBroker {
                provider: Provider::Claude,
                status: AuthStatus::Active {
                    login: Some("claude@example.com".to_string()),
                },
            }),
            Arc::new(FakeBroker {
                provider: Provider::Codex,
                status: AuthStatus::None,
            }),
            Arc::new(FakeBroker {
                provider: Provider::OpenCodeZen,
                status: AuthStatus::Pending,
            }),
        ]);

        HttpState {
            store,
            scheduler,
            executor,
            event_hub,
            output_hub,
            provider_auth,
            auth: AuthProvider::Local {
                session_token: secrecy::SecretString::from("test-token".to_string()),
            },
            registration: None,
            started_at: OffsetDateTime::now_utc(),
            github: GitHubConfig::default(),
            http_security: HttpSecurityConfig::default(),
            auth_failure_throttle: AuthFailureThrottle::new(),
            ci_failure_cache: Arc::new(Mutex::new(std::collections::HashSet::new())),
            sessions,
        }
    }

    #[tokio::test]
    async fn list_providers_returns_model_capable_providers() {
        let state = test_http_state().await;

        let Json(payload) = list_providers_handler(State(state))
            .await
            .expect("list providers");

        assert_eq!(payload.len(), 3);
        assert_eq!(payload[0].provider, Provider::Claude);
        assert_eq!(payload[0].auth_status, "active");
        assert_eq!(payload[0].billing, "subscription");
        assert!(!payload[0].models.is_empty());
        assert_eq!(payload[1].provider, Provider::Codex);
        assert_eq!(payload[1].auth_status, "none");
        assert_eq!(payload[2].provider, Provider::OpenCodeZen);
        assert_eq!(payload[2].auth_status, "pending");
        assert_eq!(payload[2].billing, "per_token");
        assert!(payload.iter().all(|provider| provider
            .models
            .iter()
            .all(|model| model.cost_rates.is_none())));
    }
}
