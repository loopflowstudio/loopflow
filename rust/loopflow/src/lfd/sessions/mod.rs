mod harness;
pub mod types;

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use tokio::sync::{broadcast, Mutex};

use crate::lfd::id::LfdId;
use crate::lfd::sessions::harness::{CreateHarnessFn, SessionHarness};
use crate::lfd::sessions::types::{
    CreateSessionParams, PersistedSessionEvent, Session, SessionEvent, SessionStatus,
};
use crate::lfd::store::{SharedStore, StoreError};

const HARNESS_EVENT_BUFFER: usize = 256;
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
    #[error("turn already in progress")]
    TurnAlreadyInProgress,
    #[error("harness error: {0}")]
    Harness(String),
}

struct SessionRuntime {
    harness: Mutex<Box<dyn SessionHarness>>,
    events_tx: broadcast::Sender<PersistedSessionEvent>,
    next_seq: AtomicI64,
}

impl std::fmt::Debug for SessionRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionRuntime").finish()
    }
}

struct SessionManagerInner {
    store: SharedStore,
    create_harness: CreateHarnessFn,
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
        Self::with_create_harness(store, harness::default_create_harness)
    }

    fn with_create_harness(store: SharedStore, create_harness: CreateHarnessFn) -> Self {
        Self {
            inner: Arc::new(SessionManagerInner {
                store,
                create_harness,
                runtimes: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub async fn create_session(
        &self,
        params: CreateSessionParams,
    ) -> Result<Session, SessionManagerError> {
        let provider = params.provider.trim().to_lowercase();
        if !harness::supports_provider(&provider) {
            return Err(SessionManagerError::UnsupportedProvider(provider));
        }

        if let Some(wave_run_id) = params.wave_run_id.as_deref() {
            if self
                .inner
                .store
                .get_active_session_for_wave_run(wave_run_id)
                .await?
                .is_some()
            {
                return Err(SessionManagerError::WaveRunSessionConflict(
                    wave_run_id.to_string(),
                ));
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
        let (harness_events_tx, mut harness_events_rx) = broadcast::channel(HARNESS_EVENT_BUFFER);
        let harness = (self.inner.create_harness)(&provider, harness_events_tx)
            .map_err(|err| SessionManagerError::Harness(err.to_string()))?;
        self.inner.store.create_session(&session).await?;

        let (events_tx, _) = broadcast::channel(LIVE_EVENT_BUFFER);
        let runtime = Arc::new(SessionRuntime {
            harness: Mutex::new(harness),
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
            SessionEvent::StatusChanged {
                status: SessionStatus::Starting,
            },
        )
        .await?;

        let manager = self.clone();
        let session_id = session.id.clone();
        let bridge_runtime = runtime.clone();
        tokio::spawn(async move {
            loop {
                match harness_events_rx.recv().await {
                    Ok(event) => {
                        // Intercept ProviderSessionId: persist to DB, don't forward.
                        if let SessionEvent::ProviderSessionId {
                            ref provider_session_id,
                        } = event
                        {
                            {
                                let mut harness = bridge_runtime.harness.lock().await;
                                harness.set_provider_session_id(Some(provider_session_id.clone()));
                            }
                            if let Err(err) = manager
                                .inner
                                .store
                                .update_provider_session_id(&session_id, provider_session_id)
                                .await
                            {
                                tracing::warn!(
                                    session_id = %session_id,
                                    error = %err,
                                    "failed to persist provider session id"
                                );
                            }
                            continue;
                        }

                        if let Err(err) = manager
                            .append_runtime_event(&session_id, &bridge_runtime, event)
                            .await
                        {
                            tracing::warn!(session_id = %session_id, error = %err, "failed to persist harness event");
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(session_id = %session_id, skipped, "session harness lagged")
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
                let mut harness = startup_runtime.harness.lock().await;
                harness.start(&startup_config).await
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
            let mut harness = runtime.harness.lock().await;
            harness.send_input(content).await
        };

        if let Err(err) = send_result {
            if harness::is_turn_in_progress(&err) {
                return Err(SessionManagerError::TurnAlreadyInProgress);
            }

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
            return Err(SessionManagerError::Harness(err.to_string()));
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

        let final_status = if let Some(ref runtime) = runtime {
            let stop_result = {
                let mut harness = runtime.harness.lock().await;
                harness.stop().await
            };
            if let Err(err) = stop_result {
                self.append_runtime_event(
                    session_id,
                    runtime,
                    SessionEvent::Error {
                        code: "session_stop_failed".to_string(),
                        message: err.to_string(),
                    },
                )
                .await?;
                SessionStatus::Failed
            } else {
                SessionStatus::Ended
            }
        } else {
            SessionStatus::Ended
        };

        let has_runtime = runtime.is_some();
        self.set_status(
            session_id,
            final_status,
            Some(time::OffsetDateTime::now_utc().unix_timestamp()),
            runtime,
        )
        .await?;
        if has_runtime {
            self.remove_runtime(session_id).await;
        }
        self.get_session(session_id).await
    }

    pub async fn list_events(
        &self,
        session_id: &LfdId,
        after_seq: Option<i64>,
    ) -> Result<Vec<PersistedSessionEvent>, SessionManagerError> {
        // Ensure we return a clean 404 for unknown sessions.
        let _ = self.get_session(session_id).await?;
        Ok(self
            .inner
            .store
            .list_session_events(session_id, after_seq)
            .await?)
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
            self.append_runtime_event(session_id, &runtime, SessionEvent::StatusChanged { status })
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
            .append_session_event(session_id, seq, &event, now.unix_timestamp())
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
    use crate::lfd::sessions::harness::{SessionHarness, SessionHarnessError};
    use crate::lfd::sessions::types::{SessionConfig, SessionEvent, TurnStatus};
    use crate::lfd::store::{open_store, StorageConfig};
    use anyhow::Result;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[derive(Debug)]
    struct FakeHarness {
        tx: broadcast::Sender<SessionEvent>,
    }

    #[async_trait]
    impl SessionHarness for FakeHarness {
        async fn start(&mut self, _config: &SessionConfig) -> Result<()> {
            Ok(())
        }

        async fn send_input(&mut self, content: &str) -> Result<()> {
            let turn_id = format!("turn_{}", uuid::Uuid::new_v4());
            let _ = self.tx.send(SessionEvent::TurnStarted {
                turn_id: turn_id.clone(),
            });
            let _ = self.tx.send(SessionEvent::TextDelta {
                turn_id: turn_id.clone(),
                content: content.to_string(),
            });
            let _ = self.tx.send(SessionEvent::TurnCompleted {
                turn_id,
                status: TurnStatus::Completed,
            });
            Ok(())
        }

        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }
    }

    fn fake_create_harness(
        _provider: &str,
        event_tx: broadcast::Sender<SessionEvent>,
    ) -> Result<Box<dyn SessionHarness>> {
        Ok(Box::new(FakeHarness { tx: event_tx }))
    }

    #[derive(Debug)]
    struct BusyHarness;

    #[async_trait]
    impl SessionHarness for BusyHarness {
        async fn start(&mut self, _config: &SessionConfig) -> Result<()> {
            Ok(())
        }

        async fn send_input(&mut self, _content: &str) -> Result<()> {
            Err(SessionHarnessError::TurnAlreadyInProgress.into())
        }

        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }
    }

    fn busy_create_harness(
        _provider: &str,
        _event_tx: broadcast::Sender<SessionEvent>,
    ) -> Result<Box<dyn SessionHarness>> {
        Ok(Box::new(BusyHarness))
    }

    #[derive(Debug)]
    struct ResumeAwareHarness {
        tx: broadcast::Sender<SessionEvent>,
        send_count: usize,
        provider_session_id: Option<String>,
    }

    #[async_trait]
    impl SessionHarness for ResumeAwareHarness {
        async fn start(&mut self, _config: &SessionConfig) -> Result<()> {
            Ok(())
        }

        async fn send_input(&mut self, _content: &str) -> Result<()> {
            self.send_count += 1;
            let turn_id = format!("turn_{}", uuid::Uuid::new_v4());
            let _ = self.tx.send(SessionEvent::TurnStarted {
                turn_id: turn_id.clone(),
            });
            if self.send_count == 1 {
                let _ = self.tx.send(SessionEvent::ProviderSessionId {
                    provider_session_id: "sess_resume_1".to_string(),
                });
            }
            let resume = self
                .provider_session_id
                .clone()
                .unwrap_or_else(|| "none".to_string());
            let _ = self.tx.send(SessionEvent::TextDelta {
                turn_id: turn_id.clone(),
                content: format!("resume:{resume}"),
            });
            let _ = self.tx.send(SessionEvent::TurnCompleted {
                turn_id,
                status: TurnStatus::Completed,
            });
            Ok(())
        }

        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }

        fn set_provider_session_id(&mut self, provider_session_id: Option<String>) {
            self.provider_session_id = provider_session_id;
        }
    }

    fn resume_aware_create_harness(
        _provider: &str,
        event_tx: broadcast::Sender<SessionEvent>,
    ) -> Result<Box<dyn SessionHarness>> {
        Ok(Box::new(ResumeAwareHarness {
            tx: event_tx,
            send_count: 0,
            provider_session_id: None,
        }))
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

    async fn wait_for_provider_session_id(
        manager: &SessionManager,
        session_id: &LfdId,
        expected: &str,
    ) {
        for _ in 0..50 {
            let session = manager
                .get_session(session_id)
                .await
                .expect("session should exist");
            if session.provider_session_id.as_deref() == Some(expected) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("session never captured expected provider session id");
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

        let manager = SessionManager::with_create_harness(store, fake_create_harness);
        let created = manager
            .create_session(CreateSessionParams {
                provider: "codex".to_string(),
                wave_run_id: Some("run_1".to_string()),
                config: SessionConfig {
                    model: Some("gpt-5.1-codex".to_string()),
                    cwd: Some(tmp.path().to_string_lossy().to_string()),
                    ..Default::default()
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
                    &event.event,
                    SessionEvent::TextDelta { content, .. } if content == "fix the failing tests"
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

    #[tokio::test]
    async fn create_session_rejects_unsupported_provider() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        // Use default_create_harness (not fake) so unsupported providers are rejected.
        let manager = SessionManager::new(store);

        let err = manager
            .create_session(CreateSessionParams {
                provider: "openai".to_string(),
                wave_run_id: None,
                config: SessionConfig::default(),
            })
            .await
            .expect_err("unsupported provider should fail");

        assert!(matches!(
            err,
            SessionManagerError::UnsupportedProvider(ref provider) if provider == "openai"
        ));
    }

    #[tokio::test]
    async fn create_session_enforces_single_active_session_per_wave_run() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        let manager = SessionManager::with_create_harness(store, fake_create_harness);

        let created = manager
            .create_session(CreateSessionParams {
                provider: "codex".to_string(),
                wave_run_id: Some("run_1".to_string()),
                config: SessionConfig::default(),
            })
            .await
            .expect("first session should create");
        let _ = wait_for_status(&manager, &created.id, SessionStatus::Active).await;

        let err = manager
            .create_session(CreateSessionParams {
                provider: "codex".to_string(),
                wave_run_id: Some("run_1".to_string()),
                config: SessionConfig::default(),
            })
            .await
            .expect_err("second active session should be rejected");

        assert!(matches!(
            err,
            SessionManagerError::WaveRunSessionConflict(ref wave_run_id) if wave_run_id == "run_1"
        ));
    }

    #[tokio::test]
    async fn send_input_busy_error_does_not_fail_session() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        let manager = SessionManager::with_create_harness(store, busy_create_harness);

        let created = manager
            .create_session(CreateSessionParams {
                provider: "claude".to_string(),
                wave_run_id: None,
                config: SessionConfig::default(),
            })
            .await
            .expect("create session");
        let _ = wait_for_status(&manager, &created.id, SessionStatus::Active).await;

        let err = manager
            .send_input(&created.id, "hello")
            .await
            .expect_err("busy harness should reject concurrent turn");
        assert!(matches!(err, SessionManagerError::TurnAlreadyInProgress));

        let session = manager
            .get_session(&created.id)
            .await
            .expect("session should still exist");
        assert_eq!(session.status, SessionStatus::Active);
    }

    #[tokio::test]
    async fn provider_session_id_is_applied_to_harness_for_resume() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        let manager = SessionManager::with_create_harness(store, resume_aware_create_harness);

        let created = manager
            .create_session(CreateSessionParams {
                provider: "claude".to_string(),
                wave_run_id: None,
                config: SessionConfig::default(),
            })
            .await
            .expect("create session");
        let _ = wait_for_status(&manager, &created.id, SessionStatus::Active).await;

        manager
            .send_input(&created.id, "first turn")
            .await
            .expect("first input should succeed");
        wait_for_provider_session_id(&manager, &created.id, "sess_resume_1").await;

        manager
            .send_input(&created.id, "second turn")
            .await
            .expect("second input should succeed");

        let mut saw_resume = false;
        for _ in 0..50 {
            let events = manager
                .list_events(&created.id, None)
                .await
                .expect("list events");
            saw_resume = events.iter().any(|event| {
                matches!(
                    &event.event,
                    SessionEvent::TextDelta { content, .. } if content == "resume:sess_resume_1"
                )
            });
            if saw_resume {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(saw_resume);
    }
}
