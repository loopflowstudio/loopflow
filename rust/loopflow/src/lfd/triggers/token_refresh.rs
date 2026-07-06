use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::lfdb::rows::now_unix;
use crate::lfdb::{ProviderToken, SharedStore};
use crate::provider_auth::{refresh_provider_token, Provider};

const REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);
const REFRESH_THRESHOLD: Duration = Duration::from_secs(20 * 60);
const REFRESH_TIMEOUT: Duration = Duration::from_secs(30);

#[async_trait]
trait TokenRefresher: Send + Sync {
    async fn refresh(&self, provider: Provider) -> Result<ProviderToken, String>;
}

#[derive(Debug)]
struct ProviderAuthTokenRefresher;

#[async_trait]
impl TokenRefresher for ProviderAuthTokenRefresher {
    async fn refresh(&self, provider: Provider) -> Result<ProviderToken, String> {
        refresh_provider_token(provider)
            .await
            .map_err(|err| err.to_string())
    }
}

#[derive(Clone)]
struct RefreshTaskDeps {
    store: SharedStore,
    cancel: CancellationToken,
    refresher: Arc<dyn TokenRefresher>,
    refresh_timeout: Duration,
}

pub fn spawn_token_refresh(store: SharedStore, cancel: CancellationToken) -> JoinHandle<()> {
    spawn_token_refresh_with_refresher(
        store,
        cancel,
        Arc::new(ProviderAuthTokenRefresher),
        REFRESH_INTERVAL,
        REFRESH_THRESHOLD,
        REFRESH_TIMEOUT,
    )
}

fn spawn_token_refresh_with_refresher(
    store: SharedStore,
    cancel: CancellationToken,
    refresher: Arc<dyn TokenRefresher>,
    refresh_interval: Duration,
    refresh_threshold: Duration,
    refresh_timeout: Duration,
) -> JoinHandle<()> {
    let locks = Arc::new(provider_refresh_locks());
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(refresh_interval);
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("token_refresh shutting down");
                    break;
                }
                _ = interval.tick() => {
                    let _ = schedule_due_refreshes(
                        store.clone(),
                        cancel.clone(),
                        refresher.clone(),
                        locks.clone(),
                        refresh_threshold,
                        refresh_timeout,
                    ).await;
                }
            }
        }
    })
}

fn provider_refresh_locks() -> HashMap<Provider, Arc<Mutex<()>>> {
    Provider::all()
        .into_iter()
        .map(|provider| (provider, Arc::new(Mutex::new(()))))
        .collect()
}

async fn schedule_due_refreshes(
    store: SharedStore,
    cancel: CancellationToken,
    refresher: Arc<dyn TokenRefresher>,
    locks: Arc<HashMap<Provider, Arc<Mutex<()>>>>,
    refresh_threshold: Duration,
    refresh_timeout: Duration,
) -> Vec<JoinHandle<()>> {
    let deps = RefreshTaskDeps {
        store,
        cancel,
        refresher,
        refresh_timeout,
    };
    let refresh_before = now_unix() + refresh_threshold.as_secs() as i64;
    let tokens = match deps.store.list_provider_tokens().await {
        Ok(tokens) => tokens,
        Err(err) => {
            tracing::warn!(error = %err, "failed to list provider tokens for refresh");
            return Vec::new();
        }
    };

    let mut handles = Vec::new();
    for token in tokens {
        let Some(expires_at) = token.expires_at else {
            continue;
        };
        if expires_at >= refresh_before {
            continue;
        }

        let provider = match token.provider.parse::<Provider>() {
            Ok(provider) => provider,
            Err(err) => {
                tracing::warn!(
                    provider = %token.provider,
                    error = %err,
                    "skipping token refresh for unsupported provider"
                );
                continue;
            }
        };

        let Some(lock) = locks.get(&provider).cloned() else {
            continue;
        };

        handles.push(tokio::spawn(refresh_provider_token_row(
            provider,
            token,
            lock,
            deps.clone(),
        )));
    }

    handles
}

async fn refresh_provider_token_row(
    provider: Provider,
    current_token: ProviderToken,
    lock: Arc<Mutex<()>>,
    deps: RefreshTaskDeps,
) {
    let _guard = match lock.try_lock_owned() {
        Ok(guard) => guard,
        Err(_) => {
            tracing::debug!(provider = %provider, "token refresh already in-flight; skipping");
            return;
        }
    };

    let refreshed = tokio::select! {
        _ = deps.cancel.cancelled() => {
            return;
        }
        result = tokio::time::timeout(deps.refresh_timeout, deps.refresher.refresh(provider)) => {
            match result {
                Ok(result) => result,
                Err(_) => {
                    let reason = format!(
                        "token refresh timed out after {} seconds",
                        deps.refresh_timeout.as_secs()
                    );
                    log_refresh_failure(provider, reason);
                    return;
                }
            }
        }
    };

    match refreshed {
        Ok(mut refreshed_token) => {
            if refreshed_token
                .expires_at
                .is_some_and(|expires_at| expires_at <= now_unix())
            {
                log_refresh_failure(provider, "refreshed token is already expired".to_string());
                return;
            }

            if refreshed_token.login.is_none() {
                refreshed_token.login = current_token.login.clone();
            }

            if let Err(err) = deps.store.upsert_provider_token(&refreshed_token).await {
                log_refresh_failure(provider, err.to_string());
                return;
            }

            tracing::info!(provider = %provider, "refreshed provider token");
        }
        Err(reason) => {
            log_refresh_failure(provider, reason);
        }
    }
}

/// Log a refresh failure. Auth is poll-only in the base wave model (see
/// `scratch/eventing.md` §5): there is no machine-wide push, so a failed
/// refresh surfaces in the logs and on the next `lf auth`/provider-list poll.
/// Providers that can't self-refresh (Claude, OpenCodeZen) need user
/// re-authentication; providers with CLI refresh (GitHub, Codex) may recover on
/// the next scheduled attempt.
fn log_refresh_failure(provider: Provider, reason: String) {
    if provider.supports_cli_refresh() {
        tracing::warn!(provider = %provider, reason = %reason, "provider token refresh failed");
    } else {
        tracing::warn!(provider = %provider, reason = %reason, "provider token refresh requires re-auth");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::id::LfdId;
    use crate::lfdb::{open_store, StorageConfig};
    use std::collections::VecDeque;
    use std::sync::Mutex as StdMutex;

    #[derive(Debug, Clone)]
    enum RefreshPlan {
        Success {
            token: ProviderToken,
            delay: Duration,
        },
        Failure {
            reason: String,
            delay: Duration,
        },
    }

    #[derive(Debug)]
    struct FakeTokenRefresher {
        plans: StdMutex<HashMap<Provider, VecDeque<RefreshPlan>>>,
    }

    impl FakeTokenRefresher {
        fn new(plans: HashMap<Provider, Vec<RefreshPlan>>) -> Self {
            let plans = plans
                .into_iter()
                .map(|(provider, provider_plans)| (provider, VecDeque::from(provider_plans)))
                .collect();
            Self {
                plans: StdMutex::new(plans),
            }
        }
    }

    #[async_trait]
    impl TokenRefresher for FakeTokenRefresher {
        async fn refresh(&self, provider: Provider) -> Result<ProviderToken, String> {
            let plan = {
                let mut plans = self.plans.lock().expect("plans mutex poisoned");
                plans.get_mut(&provider).and_then(VecDeque::pop_front)
            };
            let Some(plan) = plan else {
                return Err(format!("no refresh plan configured for {provider}"));
            };

            match plan {
                RefreshPlan::Success { token, delay } => {
                    tokio::time::sleep(delay).await;
                    Ok(token)
                }
                RefreshPlan::Failure { reason, delay } => {
                    tokio::time::sleep(delay).await;
                    Err(reason)
                }
            }
        }
    }

    async fn create_store() -> SharedStore {
        let path = std::env::temp_dir().join(format!("lfd-token-refresh-{}.db", LfdId::new()));
        open_store(&StorageConfig::sqlite(path))
            .await
            .map(Arc::new)
            .expect("store")
    }

    async fn upsert_expiring_token(store: &SharedStore, provider: Provider, login: Option<&str>) {
        let token = ProviderToken {
            provider: provider.as_str().to_string(),
            access_token: "old-token".to_string(),
            refresh_token: None,
            expires_at: Some(now_unix() + 60),
            login: login.map(str::to_string),
            updated_at: now_unix(),
            credential_type: crate::lfdb::CredentialType::OAuth,
        };
        store
            .upsert_provider_token(&token)
            .await
            .expect("insert token");
    }

    /// Poll the store until a provider's access token becomes `expected`, or
    /// time out. Auth is poll-only now — the store is the truth, not a stream.
    async fn wait_for_token(store: &SharedStore, provider: &str, expected: &str) -> bool {
        for _ in 0..40 {
            if let Ok(Some(token)) = store.get_provider_token(provider).await {
                if token.access_token == expected {
                    return true;
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        false
    }

    async fn run_due_refreshes(
        store: SharedStore,
        refresher: Arc<dyn TokenRefresher>,
        refresh_timeout: Duration,
    ) {
        let handles = schedule_due_refreshes(
            store,
            CancellationToken::new(),
            refresher,
            Arc::new(provider_refresh_locks()),
            Duration::from_secs(20 * 60),
            refresh_timeout,
        )
        .await;
        for handle in handles {
            handle.await.expect("refresh task join");
        }
    }

    #[tokio::test]
    async fn refresh_due_token_updates_store() {
        let store = create_store().await;
        upsert_expiring_token(&store, Provider::GitHub, Some("jackdanger")).await;

        let refreshed = ProviderToken {
            provider: "github".to_string(),
            access_token: "new-token".to_string(),
            refresh_token: None,
            expires_at: Some(now_unix() + 3600),
            login: None,
            updated_at: now_unix(),
            credential_type: crate::lfdb::CredentialType::OAuth,
        };
        let refresher = Arc::new(FakeTokenRefresher::new(HashMap::from([(
            Provider::GitHub,
            vec![RefreshPlan::Success {
                token: refreshed,
                delay: Duration::ZERO,
            }],
        )])));

        run_due_refreshes(store.clone(), refresher, Duration::from_secs(5)).await;

        let stored = store
            .get_provider_token("github")
            .await
            .expect("load token")
            .expect("token row");
        assert_eq!(stored.access_token, "new-token");
        // The prior login carries onto a refresh that returns none.
        assert_eq!(stored.login, Some("jackdanger".to_string()));
    }

    #[tokio::test]
    async fn refresh_timeout_leaves_store_unchanged() {
        let store = create_store().await;
        upsert_expiring_token(&store, Provider::GitHub, None).await;

        let refreshed = ProviderToken {
            provider: "github".to_string(),
            access_token: "never-used".to_string(),
            refresh_token: None,
            expires_at: Some(now_unix() + 3600),
            login: None,
            updated_at: now_unix(),
            credential_type: crate::lfdb::CredentialType::OAuth,
        };
        let refresher = Arc::new(FakeTokenRefresher::new(HashMap::from([(
            Provider::GitHub,
            vec![RefreshPlan::Success {
                token: refreshed,
                delay: Duration::from_millis(200),
            }],
        )])));

        run_due_refreshes(store.clone(), refresher, Duration::from_millis(20)).await;

        // The refresh timed out before the new token landed — the store keeps
        // the old one, and the failure surfaces in the logs, not a push.
        let stored = store
            .get_provider_token("github")
            .await
            .expect("load token")
            .expect("token row");
        assert_eq!(stored.access_token, "old-token");
    }

    #[tokio::test]
    async fn refresh_handles_provider_failure_without_blocking_other_providers() {
        let store = create_store().await;
        upsert_expiring_token(&store, Provider::GitHub, None).await;
        upsert_expiring_token(&store, Provider::Claude, Some("user@example.com")).await;

        let claude_token = ProviderToken {
            provider: "claude".to_string(),
            access_token: "claude-new".to_string(),
            refresh_token: None,
            expires_at: Some(now_unix() + 3600),
            login: None,
            updated_at: now_unix(),
            credential_type: crate::lfdb::CredentialType::OAuth,
        };
        let refresher = Arc::new(FakeTokenRefresher::new(HashMap::from([
            (
                Provider::GitHub,
                vec![RefreshPlan::Failure {
                    reason: "refresh failed".to_string(),
                    delay: Duration::ZERO,
                }],
            ),
            (
                Provider::Claude,
                vec![RefreshPlan::Success {
                    token: claude_token,
                    delay: Duration::ZERO,
                }],
            ),
        ])));

        run_due_refreshes(store.clone(), refresher, Duration::from_secs(5)).await;

        // The failing provider leaves its row untouched; the healthy one still
        // refreshed — one provider's failure never blocks another.
        let github = store
            .get_provider_token("github")
            .await
            .expect("load github token")
            .expect("github token row");
        assert_eq!(github.access_token, "old-token");

        let claude = store
            .get_provider_token("claude")
            .await
            .expect("load claude token")
            .expect("claude token row");
        assert_eq!(claude.access_token, "claude-new");
        assert_eq!(claude.login, Some("user@example.com".to_string()));
    }

    #[tokio::test]
    async fn loop_continues_after_refresh_failure_on_next_tick() {
        let store = create_store().await;
        upsert_expiring_token(&store, Provider::GitHub, None).await;

        let github_refreshed = ProviderToken {
            provider: "github".to_string(),
            access_token: "github-new".to_string(),
            refresh_token: None,
            expires_at: Some(now_unix() + 3600),
            login: None,
            updated_at: now_unix(),
            credential_type: crate::lfdb::CredentialType::OAuth,
        };
        let refresher = Arc::new(FakeTokenRefresher::new(HashMap::from([(
            Provider::GitHub,
            vec![
                RefreshPlan::Failure {
                    reason: "first attempt failed".to_string(),
                    delay: Duration::ZERO,
                },
                RefreshPlan::Success {
                    token: github_refreshed,
                    delay: Duration::ZERO,
                },
            ],
        )])));
        let cancel = CancellationToken::new();

        let handle = spawn_token_refresh_with_refresher(
            store.clone(),
            cancel.clone(),
            refresher,
            Duration::from_millis(50),
            Duration::from_secs(20 * 60),
            Duration::from_secs(5),
        );

        // First tick fails and leaves the old token; the loop keeps going and
        // the second tick's success lands the refreshed token in the store.
        let refreshed = wait_for_token(&store, "github", "github-new").await;
        cancel.cancel();
        handle.await.expect("loop join");
        assert!(refreshed, "loop should recover and refresh on a later tick");
    }
}
