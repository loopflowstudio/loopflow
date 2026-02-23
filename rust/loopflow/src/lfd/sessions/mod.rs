mod adapter;
pub mod store;
pub mod types;

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use tokio::sync::{broadcast, Mutex};

use crate::lfd::id::LfdId;
use crate::lfd::sessions::adapter::{
    DefaultSessionAdapterFactory, SessionAdapter, SharedSessionAdapterFactory,
};
use crate::lfd::sessions::store::SessionStore;
use crate::lfd::sessions::types::{
    CreateSessionParams, PersistedSessionEvent, Session, SessionEvent, SessionStatus,
};
use crate::lfd::store::{SharedStore, StoreError};

const ADAPTER_EVENT_BUFFER: usize = 256;
const LIVE_EVENT_BUFFER: usize = 256;

#[derive(Debug, thiserror::Error)]
pub enum SessionManagerError {
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("session not found")]
    NotFound,
    #[error("invalid session state: expected {expected}, got {actual:?}")]
    InvalidState {
        expected: &'static str,
        actual: SessionStatus,
    },
    #[error("unsupported provider: {0}")]
    UnsupportedProvider(String),
    #[error("wave run already has an active session: {0}")]
    WaveRunSessionConflict(String),
    #[error("adapter error: {0}")]
    Adapter(String),
}

struct SessionRuntime {
    adapter: Mutex<Box<dyn SessionAdapter>>,
    events_tx: broadcast::Sender<PersistedSessionEvent>,
    next_seq: AtomicI64,
}

impl std::fmt::Debug for SessionRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionRuntime").finish()
    }
}

struct SessionManagerInner {
    store: SessionStore,
    adapter_factory: SharedSessionAdapterFactory,
    runtimes: Mutex<HashMap<LfdId, Arc<SessionRuntime>>>,
}

impl std::fmt::Debug for SessionManagerInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionManagerInner").finish()
    }
}

#[derive(Clone, Debug)]
pub struct SessionManager {
    inner: Arc<SessionManagerInner>,
}

impl SessionManager {
    pub fn new(store: SharedStore) -> Self {
        Self::with_factory(store, Arc::new(DefaultSessionAdapterFactory))
    }

    pub fn with_factory(store: SharedStore, adapter_factory: SharedSessionAdapterFactory) -> Self {
        Self {
            inner: Arc::new(SessionManagerInner {
                store: SessionStore::new(store),
                adapter_factory,
                runtimes: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub async fn create_session(
        &self,
        params: CreateSessionParams,
    ) -> Result<Session, SessionManagerError> {
        let provider = params.provider.trim().to_lowercase();
        if provider.is_empty() {
            return Err(SessionManagerError::UnsupportedProvider(provider));
        }

        if let Some(wave_run_id) = params.wave_run_id.as_deref() {
            if let Some(existing) = self
                .inner
                .store
                .get_active_session_for_wave_run(wave_run_id)
                .await?
            {
                if !existing.status.is_terminal() {
                    return Err(SessionManagerError::WaveRunSessionConflict(
                        wave_run_id.to_string(),
                    ));
                }
            }
        }

        let session = Session {
            id: LfdId::new(),
            provider: provider.clone(),
            status: SessionStatus::Starting,
            wave_run_id: params.wave_run_id,
            provider_session_id: None,
            config: params.config,
            created_at: time::OffsetDateTime::now_utc(),
            ended_at: None,
        };
        let (adapter_events_tx, mut adapter_events_rx) = broadcast::channel(ADAPTER_EVENT_BUFFER);
        let adapter = self
            .inner
            .adapter_factory
            .create(&provider, adapter_events_tx)
            .map_err(|err| {
                if err.to_string().contains("unsupported session provider") {
                    SessionManagerError::UnsupportedProvider(provider.clone())
                } else {
                    SessionManagerError::Adapter(err.to_string())
                }
            })?;
        self.inner.store.create_session(&session).await?;

        let (events_tx, _) = broadcast::channel(LIVE_EVENT_BUFFER);
        let runtime = Arc::new(SessionRuntime {
            adapter: Mutex::new(adapter),
            events_tx,
            next_seq: AtomicI64::new(0),
        });

        {
            let mut runtimes = self.inner.runtimes.lock().await;
            runtimes.insert(session.id.clone(), runtime.clone());
        }

        self.append_runtime_event(
            &session.id,
            &runtime,
            SessionEvent::SessionStatus {
                status: SessionStatus::Starting,
            },
        )
        .await?;

        let manager = self.clone();
        let session_id = session.id.clone();
        let bridge_runtime = runtime.clone();
        tokio::spawn(async move {
            loop {
                match adapter_events_rx.recv().await {
                    Ok(event) => {
                        if let Err(err) = manager
                            .append_runtime_event(&session_id, &bridge_runtime, event)
                            .await
                        {
                            tracing::warn!(session_id = %session_id, error = %err, "failed to persist adapter event");
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(session_id = %session_id, skipped, "session adapter lagged")
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        let manager = self.clone();
        let session_id = session.id.clone();
        let startup_runtime = runtime.clone();
        let startup_config = session.config.clone();
        tokio::spawn(async move {
            let result = {
                let mut adapter = startup_runtime.adapter.lock().await;
                adapter.start(&startup_config).await
            };

            match result {
                Ok(()) => {
                    if let Err(err) = manager
                        .set_status(
                            &session_id,
                            SessionStatus::Active,
                            None,
                            Some(startup_runtime.clone()),
                        )
                        .await
                    {
                        tracing::warn!(session_id = %session_id, error = %err, "failed to set active session status");
                    }
                }
                Err(err) => {
                    let _ = manager
                        .append_runtime_event(
                            &session_id,
                            &startup_runtime,
                            SessionEvent::Error {
                                code: "session_start_failed".to_string(),
                                message: err.to_string(),
                            },
                        )
                        .await;
                    let _ = manager
                        .set_status(
                            &session_id,
                            SessionStatus::Failed,
                            Some(time::OffsetDateTime::now_utc().unix_timestamp()),
                            Some(startup_runtime.clone()),
                        )
                        .await;
                    let _ = manager.remove_runtime(&session_id).await;
                }
            }
        });

        Ok(session)
    }

    pub async fn get_session(&self, session_id: &LfdId) -> Result<Session, SessionManagerError> {
        self.inner
            .store
            .get_session(session_id)
            .await?
            .ok_or(SessionManagerError::NotFound)
    }

    pub async fn send_input(
        &self,
        session_id: &LfdId,
        content: &str,
    ) -> Result<(), SessionManagerError> {
        let session = self.get_session(session_id).await?;
        if session.status != SessionStatus::Active {
            return Err(SessionManagerError::InvalidState {
                expected: "active",
                actual: session.status,
            });
        }

        let runtime = self
            .runtime(session_id)
            .await
            .ok_or(SessionManagerError::InvalidState {
                expected: "runtime available",
                actual: session.status,
            })?;

        let send_result = {
            let mut adapter = runtime.adapter.lock().await;
            adapter.send_input(content).await
        };

        if let Err(err) = send_result {
            let _ = self
                .append_runtime_event(
                    session_id,
                    &runtime,
                    SessionEvent::Error {
                        code: "send_input_failed".to_string(),
                        message: err.to_string(),
                    },
                )
                .await;
            let _ = self
                .set_status(
                    session_id,
                    SessionStatus::Failed,
                    Some(time::OffsetDateTime::now_utc().unix_timestamp()),
                    Some(runtime.clone()),
                )
                .await;
            let _ = self.remove_runtime(session_id).await;
            return Err(SessionManagerError::Adapter(err.to_string()));
        }

        Ok(())
    }

    pub async fn stop_session(&self, session_id: &LfdId) -> Result<Session, SessionManagerError> {
        let session = self.get_session(session_id).await?;
        if session.status.is_terminal() {
            return Ok(session);
        }

        let runtime = self.runtime(session_id).await;
        if session.status != SessionStatus::Ending {
            self.set_status(session_id, SessionStatus::Ending, None, runtime.clone())
                .await?;
        }

        if let Some(runtime) = runtime {
            let stop_result = {
                let mut adapter = runtime.adapter.lock().await;
                adapter.stop().await
            };
            if let Err(err) = stop_result {
                self.append_runtime_event(
                    session_id,
                    &runtime,
                    SessionEvent::Error {
                        code: "session_stop_failed".to_string(),
                        message: err.to_string(),
                    },
                )
                .await?;
                self.set_status(
                    session_id,
                    SessionStatus::Failed,
                    Some(time::OffsetDateTime::now_utc().unix_timestamp()),
                    Some(runtime.clone()),
                )
                .await?;
                self.remove_runtime(session_id).await;
                return self.get_session(session_id).await;
            }

            self.set_status(
                session_id,
                SessionStatus::Ended,
                Some(time::OffsetDateTime::now_utc().unix_timestamp()),
                Some(runtime.clone()),
            )
            .await?;
            self.remove_runtime(session_id).await;
            return self.get_session(session_id).await;
        }

        self.inner
            .store
            .update_session_status(
                session_id,
                SessionStatus::Ended,
                Some(time::OffsetDateTime::now_utc().unix_timestamp()),
            )
            .await?;
        self.get_session(session_id).await
    }

    pub async fn list_events(
        &self,
        session_id: &LfdId,
        after_seq: Option<i64>,
    ) -> Result<Vec<PersistedSessionEvent>, SessionManagerError> {
        // Ensure we return a clean 404 for unknown sessions.
        let _ = self.get_session(session_id).await?;
        Ok(self.inner.store.list_events(session_id, after_seq).await?)
    }

    pub async fn subscribe(
        &self,
        session_id: &LfdId,
    ) -> Result<Option<broadcast::Receiver<PersistedSessionEvent>>, SessionManagerError> {
        let _ = self.get_session(session_id).await?;
        Ok(self
            .runtime(session_id)
            .await
            .map(|runtime| runtime.events_tx.subscribe()))
    }

    async fn set_status(
        &self,
        session_id: &LfdId,
        status: SessionStatus,
        ended_at: Option<i64>,
        runtime: Option<Arc<SessionRuntime>>,
    ) -> Result<(), SessionManagerError> {
        self.inner
            .store
            .update_session_status(session_id, status, ended_at)
            .await?;

        if let Some(runtime) = runtime {
            self.append_runtime_event(session_id, &runtime, SessionEvent::SessionStatus { status })
                .await?;
        }

        Ok(())
    }

    async fn append_runtime_event(
        &self,
        session_id: &LfdId,
        runtime: &Arc<SessionRuntime>,
        event: SessionEvent,
    ) -> Result<PersistedSessionEvent, SessionManagerError> {
        let now = time::OffsetDateTime::now_utc();
        let seq = runtime.next_seq.fetch_add(1, Ordering::Relaxed);

        self.inner
            .store
            .append_event(session_id, seq, &event, now.unix_timestamp())
            .await?;

        let persisted = PersistedSessionEvent {
            session_id: session_id.clone(),
            seq,
            event,
            created_at: now,
        };
        let _ = runtime.events_tx.send(persisted.clone());
        Ok(persisted)
    }

    async fn runtime(&self, session_id: &LfdId) -> Option<Arc<SessionRuntime>> {
        let runtimes = self.inner.runtimes.lock().await;
        runtimes.get(session_id).cloned()
    }

    async fn remove_runtime(&self, session_id: &LfdId) {
        let mut runtimes = self.inner.runtimes.lock().await;
        runtimes.remove(session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::sessions::adapter::{SessionAdapter, SessionAdapterFactory};
    use crate::lfd::sessions::types::{SessionConfig, SessionEvent, TurnStatus};
    use crate::lfd::store::{open_store, StorageConfig};
    use anyhow::anyhow;
    use anyhow::Result;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[derive(Debug)]
    struct FakeAdapter {
        tx: broadcast::Sender<SessionEvent>,
    }

    #[async_trait]
    impl SessionAdapter for FakeAdapter {
        async fn start(&mut self, _config: &SessionConfig) -> Result<()> {
            Ok(())
        }

        async fn send_input(&mut self, content: &str) -> Result<()> {
            let _ = self.tx.send(SessionEvent::TurnStarted);
            let _ = self.tx.send(SessionEvent::TextDelta {
                content: content.to_string(),
            });
            let _ = self.tx.send(SessionEvent::TurnCompleted {
                status: TurnStatus::Completed,
            });
            Ok(())
        }

        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct FakeFactory;

    impl SessionAdapterFactory for FakeFactory {
        fn create(
            &self,
            provider: &str,
            event_tx: broadcast::Sender<SessionEvent>,
        ) -> Result<Box<dyn SessionAdapter>> {
            if provider != "codex" {
                return Err(anyhow!("unsupported session provider: {provider}"));
            }
            Ok(Box::new(FakeAdapter { tx: event_tx }))
        }
    }

    async fn wait_for_status(
        manager: &SessionManager,
        session_id: &LfdId,
        expected: SessionStatus,
    ) -> Session {
        for _ in 0..50 {
            let session = manager
                .get_session(session_id)
                .await
                .expect("session should exist");
            if session.status == expected {
                return session;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("session never reached expected status");
    }

    #[tokio::test]
    async fn session_lifecycle_create_input_events_end() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );

        let manager = SessionManager::with_factory(store, Arc::new(FakeFactory));
        let created = manager
            .create_session(CreateSessionParams {
                provider: "codex".to_string(),
                wave_run_id: Some("run_1".to_string()),
                config: SessionConfig {
                    model: Some("gpt-5.1-codex".to_string()),
                    cwd: Some(tmp.path().to_string_lossy().to_string()),
                },
            })
            .await
            .expect("create session");

        assert_eq!(created.status, SessionStatus::Starting);
        let _ = wait_for_status(&manager, &created.id, SessionStatus::Active).await;

        manager
            .send_input(&created.id, "fix the failing tests")
            .await
            .expect("send input");

        let mut saw_text_delta = false;
        for _ in 0..50 {
            let events = manager
                .list_events(&created.id, None)
                .await
                .expect("list events");
            saw_text_delta = events.iter().any(|event| {
                matches!(
                    event.event,
                    SessionEvent::TextDelta {
                        ref content
                    } if content == "fix the failing tests"
                )
            });
            if saw_text_delta {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(saw_text_delta);

        let ended = manager
            .stop_session(&created.id)
            .await
            .expect("stop session");
        assert_eq!(ended.status, SessionStatus::Ended);

        let replay = manager
            .list_events(&created.id, Some(0))
            .await
            .expect("replay events");
        assert!(!replay.is_empty());
    }
}
