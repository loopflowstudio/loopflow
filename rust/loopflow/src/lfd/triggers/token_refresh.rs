use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::lfd::events::EventHub;
use crate::lfd::provider_auth::{refresh_provider_token, Provider};
use crate::lfd::store::rows::now_unix;
use crate::lfd::store::{ProviderToken, SharedStore};
use crate::lfd::types::Event;

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
    event_hub: EventHub,
    cancel: CancellationToken,
    refresher: Arc<dyn TokenRefresher>,
    refresh_timeout: Duration,
}

pub fn spawn_token_refresh(
    store: SharedStore,
    event_hub: EventHub,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    spawn_token_refresh_with_refresher(
        store,
        event_hub,
        cancel,
        Arc::new(ProviderAuthTokenRefresher),
        REFRESH_INTERVAL,
        REFRESH_THRESHOLD,
        REFRESH_TIMEOUT,
    )
}

fn spawn_token_refresh_with_refresher(
    store: SharedStore,
    event_hub: EventHub,
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
                        event_hub.clone(),
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
    event_hub: EventHub,
    cancel: CancellationToken,
    refresher: Arc<dyn TokenRefresher>,
    locks: Arc<HashMap<Provider, Arc<Mutex<()>>>>,
    refresh_threshold: Duration,
    refresh_timeout: Duration,
) -> Vec<JoinHandle<()>> {
    let deps = RefreshTaskDeps {
        store,
        event_hub,
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
                    emit_refresh_failure(&deps.event_hub, provider, reason);
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
                emit_refresh_failure(
                    &deps.event_hub,
                    provider,
                    "refreshed token is already expired".to_string(),
                );
                return;
            }

            if refreshed_token.login.is_none() {
                refreshed_token.login = current_token.login.clone();
            }
            let login = refreshed_token.login.clone();

            if let Err(err) = deps.store.upsert_provider_token(&refreshed_token).await {
                emit_refresh_failure(&deps.event_hub, provider, err.to_string());
                return;
            }

            deps.event_hub
                .send(Event::auth_token_refreshed(provider, login));
        }
        Err(reason) => {
            emit_refresh_failure(&deps.event_hub, provider, reason);
        }
    }
}

/// Emit the appropriate failure event based on provider identity.
/// Providers that can't self-refresh (Claude, OpenCodeZen) get `auth.refresh_required`
/// because user re-authentication is the only path forward. Providers with CLI refresh
/// (GitHub, Codex) get `auth.refresh_failed` since the next scheduled attempt may succeed.
fn emit_refresh_failure(event_hub: &EventHub, provider: Provider, reason: String) {
    if provider.supports_cli_refresh() {
        event_hub.send(Event::auth_refresh_failed(provider, reason));
    } else {
        event_hub.send(Event::auth_refresh_required(provider, reason));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::id::LfdId;
    use crate::lfd::store::{open_store, StorageConfig};
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
            credential_type: crate::lfd::store::CredentialType::OAuth,
        };
        store
            .upsert_provider_token(&token)
            .await
            .expect("insert token");
    }

    async fn collect_event(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Event {
        tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("event timeout")
            .expect("event receive")
    }

    #[tokio::test]
    async fn refresh_due_token_updates_store_and_emits_success_event() {
        let store = create_store().await;
        upsert_expiring_token(&store, Provider::GitHub, Some("jackdanger")).await;

        let refreshed = ProviderToken {
            provider: "github".to_string(),
            access_token: "new-token".to_string(),
            refresh_token: None,
            expires_at: Some(now_unix() + 3600),
            login: None,
            updated_at: now_unix(),
            credential_type: crate::lfd::store::CredentialType::OAuth,
        };
        let refresher = Arc::new(FakeTokenRefresher::new(HashMap::from([(
            Provider::GitHub,
            vec![RefreshPlan::Success {
                token: refreshed,
                delay: Duration::ZERO,
            }],
        )])));
        let event_hub = EventHub::new(16);
        let mut rx = event_hub.subscribe();

        let handles = schedule_due_refreshes(
            store.clone(),
            event_hub,
            CancellationToken::new(),
            refresher,
            Arc::new(provider_refresh_locks()),
            Duration::from_secs(20 * 60),
            Duration::from_secs(5),
        )
        .await;
        for handle in handles {
            handle.await.expect("refresh task join");
        }

        let event = collect_event(&mut rx).await;
        match event {
            Event::AuthTokenRefreshed {
                provider, login, ..
            } => {
                assert_eq!(provider, Provider::GitHub);
                assert_eq!(login, Some("jackdanger".to_string()));
            }
            other => panic!("unexpected event: {other:?}"),
        }

        let stored = store
            .get_provider_token("github")
            .await
            .expect("load token")
            .expect("token row");
        assert_eq!(stored.access_token, "new-token");
        assert_eq!(stored.login, Some("jackdanger".to_string()));
    }

    #[tokio::test]
    async fn refresh_timeout_emits_failure_event() {
        let store = create_store().await;
        upsert_expiring_token(&store, Provider::GitHub, None).await;

        let refreshed = ProviderToken {
            provider: "github".to_string(),
            access_token: "never-used".to_string(),
            refresh_token: None,
            expires_at: Some(now_unix() + 3600),
            login: None,
            updated_at: now_unix(),
            credential_type: crate::lfd::store::CredentialType::OAuth,
        };
        let refresher = Arc::new(FakeTokenRefresher::new(HashMap::from([(
            Provider::GitHub,
            vec![RefreshPlan::Success {
                token: refreshed,
                delay: Duration::from_millis(200),
            }],
        )])));
        let event_hub = EventHub::new(16);
        let mut rx = event_hub.subscribe();

        let handles = schedule_due_refreshes(
            store.clone(),
            event_hub,
            CancellationToken::new(),
            refresher,
            Arc::new(provider_refresh_locks()),
            Duration::from_secs(20 * 60),
            Duration::from_millis(20),
        )
        .await;
        for handle in handles {
            handle.await.expect("refresh task join");
        }

        let event = collect_event(&mut rx).await;
        match event {
            Event::AuthRefreshFailed {
                provider, reason, ..
            } => {
                assert_eq!(provider, Provider::GitHub);
                assert!(reason.contains("timed out"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
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
            credential_type: crate::lfd::store::CredentialType::OAuth,
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
        let event_hub = EventHub::new(16);
        let mut rx = event_hub.subscribe();

        let handles = schedule_due_refreshes(
            store.clone(),
            event_hub,
            CancellationToken::new(),
            refresher,
            Arc::new(provider_refresh_locks()),
            Duration::from_secs(20 * 60),
            Duration::from_secs(5),
        )
        .await;
        for handle in handles {
            handle.await.expect("refresh task join");
        }

        let first = collect_event(&mut rx).await;
        let second = collect_event(&mut rx).await;
        let mut saw_failure = false;
        let mut saw_success = false;
        for event in [first, second] {
            match event {
                Event::AuthRefreshFailed {
                    provider, reason, ..
                } => {
                    if provider == Provider::GitHub {
                        assert_eq!(reason, "refresh failed");
                        saw_failure = true;
                    }
                }
                Event::AuthTokenRefreshed { provider, .. } => {
                    if provider == Provider::Claude {
                        saw_success = true;
                    }
                }
                _ => {}
            }
        }
        assert!(saw_failure);
        assert!(saw_success);

        let stored = store
            .get_provider_token("claude")
            .await
            .expect("load claude token")
            .expect("claude token should exist");
        assert_eq!(stored.access_token, "claude-new");
        assert_eq!(stored.login, Some("user@example.com".to_string()));
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
            credential_type: crate::lfd::store::CredentialType::OAuth,
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
        let event_hub = EventHub::new(32);
        let mut rx = event_hub.subscribe();
        let cancel = CancellationToken::new();

        let handle = spawn_token_refresh_with_refresher(
            store.clone(),
            event_hub,
            cancel.clone(),
            refresher,
            Duration::from_millis(50),
            Duration::from_secs(20 * 60),
            Duration::from_secs(5),
        );

        let first = collect_event(&mut rx).await;
        let second = collect_event(&mut rx).await;
        cancel.cancel();
        handle.await.expect("loop join");

        assert!(matches!(
            first,
            Event::AuthRefreshFailed {
                provider: Provider::GitHub,
                ..
            }
        ));
        assert!(matches!(
            second,
            Event::AuthTokenRefreshed {
                provider: Provider::GitHub,
                ..
            }
        ));

        let stored = store
            .get_provider_token("github")
            .await
            .expect("load github token")
            .expect("github token should exist");
        assert_eq!(stored.access_token, "github-new");
    }

    #[tokio::test]
    async fn non_refreshable_provider_failure_emits_refresh_required() {
        let store = create_store().await;
        upsert_expiring_token(&store, Provider::Claude, Some("user@example.com")).await;

        let refresher = Arc::new(FakeTokenRefresher::new(HashMap::from([(
            Provider::Claude,
            vec![RefreshPlan::Failure {
                reason: "token file not found".to_string(),
                delay: Duration::ZERO,
            }],
        )])));
        let event_hub = EventHub::new(16);
        let mut rx = event_hub.subscribe();

        let handles = schedule_due_refreshes(
            store.clone(),
            event_hub,
            CancellationToken::new(),
            refresher,
            Arc::new(provider_refresh_locks()),
            Duration::from_secs(20 * 60),
            Duration::from_secs(5),
        )
        .await;
        for handle in handles {
            handle.await.expect("refresh task join");
        }

        let event = collect_event(&mut rx).await;
        match event {
            Event::AuthRefreshRequired {
                provider, reason, ..
            } => {
                assert_eq!(provider, Provider::Claude);
                assert_eq!(reason, "token file not found");
            }
            other => panic!("expected AuthRefreshRequired, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn refreshable_provider_failure_emits_refresh_failed() {
        let store = create_store().await;
        upsert_expiring_token(&store, Provider::GitHub, None).await;

        let refresher = Arc::new(FakeTokenRefresher::new(HashMap::from([(
            Provider::GitHub,
            vec![RefreshPlan::Failure {
                reason: "gh auth refresh failed".to_string(),
                delay: Duration::ZERO,
            }],
        )])));
        let event_hub = EventHub::new(16);
        let mut rx = event_hub.subscribe();

        let handles = schedule_due_refreshes(
            store.clone(),
            event_hub,
            CancellationToken::new(),
            refresher,
            Arc::new(provider_refresh_locks()),
            Duration::from_secs(20 * 60),
            Duration::from_secs(5),
        )
        .await;
        for handle in handles {
            handle.await.expect("refresh task join");
        }

        let event = collect_event(&mut rx).await;
        match event {
            Event::AuthRefreshFailed {
                provider, reason, ..
            } => {
                assert_eq!(provider, Provider::GitHub);
                assert_eq!(reason, "gh auth refresh failed");
            }
            other => panic!("expected AuthRefreshFailed, got {other:?}"),
        }
    }
}
