mod harness;
pub mod types;

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use tokio::sync::{broadcast, Mutex};

use crate::lfd::id::LfdId;
use crate::lfd::sessions::harness::{CreateHarnessFn, SessionHarness};
use crate::lfd::sessions::types::{
    CreateSessionParams, PersistedSessionEvent, Session, SessionConfig, SessionEvent, SessionStatus,
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
        let provider = harness::canonical_provider(&params.provider)
            .map(ToString::to_string)
            .ok_or_else(|| {
                SessionManagerError::UnsupportedProvider(params.provider.trim().to_lowercase())
            })?;

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
        let (harness_events_tx, harness_events_rx) = broadcast::channel(HARNESS_EVENT_BUFFER);
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

        self.spawn_harness_event_bridge(session.id.clone(), runtime.clone(), harness_events_rx);
        self.spawn_harness_startup(session.id.clone(), runtime.clone(), session.config.clone());

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

            self.mark_session_failed(
                session_id,
                &runtime,
                Some("send_input_failed"),
                Some(err.to_string()),
            )
            .await;
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

    pub async fn recover_orphaned_sessions(&self) -> Result<u32, SessionManagerError> {
        let sessions = self
            .inner
            .store
            .list_sessions_by_statuses(&[SessionStatus::Starting, SessionStatus::Active])
            .await?;
        if sessions.is_empty() {
            return Ok(0);
        }

        for session in &sessions {
            self.fail_orphaned_session(&session.id).await?;
        }

        Ok(sessions.len() as u32)
    }

    fn spawn_harness_event_bridge(
        &self,
        session_id: LfdId,
        runtime: Arc<SessionRuntime>,
        mut harness_events_rx: broadcast::Receiver<SessionEvent>,
    ) {
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                match harness_events_rx.recv().await {
                    Ok(SessionEvent::ProviderSessionId {
                        provider_session_id,
                    }) => {
                        manager
                            .handle_provider_session_id(&session_id, &runtime, provider_session_id)
                            .await;
                    }
                    Ok(SessionEvent::Error { code, message }) => {
                        let fatal = harness::is_terminal_harness_error(&code);
                        if let Err(err) = manager
                            .append_runtime_event(
                                &session_id,
                                &runtime,
                                SessionEvent::Error {
                                    code: code.clone(),
                                    message: message.clone(),
                                },
                            )
                            .await
                        {
                            tracing::warn!(
                                session_id = %session_id,
                                error = %err,
                                "failed to persist harness error event"
                            );
                        }

                        if fatal {
                            manager
                                .mark_session_failed(&session_id, &runtime, None, None)
                                .await;
                        }
                    }
                    Ok(event) => {
                        if let Err(err) = manager
                            .append_runtime_event(&session_id, &runtime, event)
                            .await
                        {
                            tracing::warn!(
                                session_id = %session_id,
                                error = %err,
                                "failed to persist harness event"
                            );
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(session_id = %session_id, skipped, "session harness lagged")
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    fn spawn_harness_startup(
        &self,
        session_id: LfdId,
        runtime: Arc<SessionRuntime>,
        config: SessionConfig,
    ) {
        let manager = self.clone();
        tokio::spawn(async move {
            let result = {
                let mut harness = runtime.harness.lock().await;
                harness.start(&config).await
            };

            match result {
                Ok(()) => {
                    let can_activate = manager
                        .can_transition_to_active(&session_id)
                        .await
                        .unwrap_or(false);
                    if !can_activate {
                        return;
                    }
                    if let Err(err) = manager
                        .set_status(
                            &session_id,
                            SessionStatus::Active,
                            None,
                            Some(runtime.clone()),
                        )
                        .await
                    {
                        tracing::warn!(
                            session_id = %session_id,
                            error = %err,
                            "failed to set active session status"
                        );
                    }
                }
                Err(err) => {
                    manager
                        .mark_session_failed(
                            &session_id,
                            &runtime,
                            Some("session_start_failed"),
                            Some(err.to_string()),
                        )
                        .await;
                }
            }
        });
    }

    async fn handle_provider_session_id(
        &self,
        session_id: &LfdId,
        runtime: &Arc<SessionRuntime>,
        provider_session_id: String,
    ) {
        {
            let mut harness = runtime.harness.lock().await;
            harness.set_provider_session_id(Some(provider_session_id.clone()));
        }
        if let Err(err) = self
            .inner
            .store
            .update_provider_session_id(session_id, &provider_session_id)
            .await
        {
            tracing::warn!(
                session_id = %session_id,
                error = %err,
                "failed to persist provider session id"
            );
        }
    }

    async fn mark_session_failed(
        &self,
        session_id: &LfdId,
        runtime: &Arc<SessionRuntime>,
        code: Option<&str>,
        message: Option<String>,
    ) {
        if let (Some(code), Some(message)) = (code, message) {
            let _ = self
                .append_runtime_event(
                    session_id,
                    runtime,
                    SessionEvent::Error {
                        code: code.to_string(),
                        message,
                    },
                )
                .await;
        }
        let _ = self
            .set_status(
                session_id,
                SessionStatus::Failed,
                Some(time::OffsetDateTime::now_utc().unix_timestamp()),
                Some(runtime.clone()),
            )
            .await;
        self.remove_runtime(session_id).await;
    }

    async fn can_transition_to_active(
        &self,
        session_id: &LfdId,
    ) -> Result<bool, SessionManagerError> {
        let session = self.get_session(session_id).await?;
        Ok(session.status == SessionStatus::Starting)
    }

    async fn fail_orphaned_session(&self, session_id: &LfdId) -> Result<(), SessionManagerError> {
        let now = time::OffsetDateTime::now_utc();
        let existing_events = self
            .inner
            .store
            .list_session_events(session_id, None)
            .await?;
        let mut next_seq = existing_events
            .last()
            .map(|event| event.seq + 1)
            .unwrap_or(0);

        let error_event = SessionEvent::Error {
            code: "lfd_restarted_orphaned_session".to_string(),
            message: "session was orphaned when lfd restarted".to_string(),
        };
        self.inner
            .store
            .append_session_event(session_id, next_seq, &error_event, now.unix_timestamp())
            .await?;
        next_seq += 1;

        let status_event = SessionEvent::StatusChanged {
            status: SessionStatus::Failed,
        };
        self.inner
            .store
            .append_session_event(session_id, next_seq, &status_event, now.unix_timestamp())
            .await?;

        self.inner
            .store
            .update_session_status(
                session_id,
                SessionStatus::Failed,
                Some(now.unix_timestamp()),
            )
            .await?;

        self.remove_runtime(session_id).await;
        Ok(())
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

    #[derive(Debug)]
    struct ItemLifecycleHarness {
        tx: broadcast::Sender<SessionEvent>,
    }

    #[async_trait]
    impl SessionHarness for ItemLifecycleHarness {
        async fn start(&mut self, _config: &SessionConfig) -> Result<()> {
            Ok(())
        }

        async fn send_input(&mut self, _content: &str) -> Result<()> {
            let turn_id = "turn_trace".to_string();
            let item_id = "cmd_trace_1".to_string();

            let _ = self.tx.send(SessionEvent::TurnStarted {
                turn_id: turn_id.clone(),
            });
            let _ = self.tx.send(SessionEvent::ItemStarted {
                turn_id: turn_id.clone(),
                item: crate::lfd::sessions::types::SessionItem::Command {
                    id: item_id.clone(),
                    command: vec!["cargo".to_string(), "test".to_string()],
                    cwd: "/tmp".to_string(),
                    status: crate::lfd::sessions::types::ItemStatus::InProgress,
                    output: None,
                    exit_code: None,
                    duration_ms: None,
                },
            });
            let _ = self.tx.send(SessionEvent::ItemUpdated {
                turn_id: turn_id.clone(),
                item_id: item_id.clone(),
                data: crate::lfd::sessions::types::ItemDelta::Output {
                    content: "running".to_string(),
                },
            });
            let _ = self.tx.send(SessionEvent::ItemCompleted {
                turn_id: turn_id.clone(),
                item: crate::lfd::sessions::types::SessionItem::Command {
                    id: item_id,
                    command: vec!["cargo".to_string(), "test".to_string()],
                    cwd: "/tmp".to_string(),
                    status: crate::lfd::sessions::types::ItemStatus::Completed,
                    output: Some("ok".to_string()),
                    exit_code: Some(0),
                    duration_ms: Some(10),
                },
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

    fn item_lifecycle_create_harness(
        _provider: &str,
        event_tx: broadcast::Sender<SessionEvent>,
    ) -> Result<Box<dyn SessionHarness>> {
        Ok(Box::new(ItemLifecycleHarness { tx: event_tx }))
    }

    #[derive(Debug)]
    struct SlowStartHarness;

    #[async_trait]
    impl SessionHarness for SlowStartHarness {
        async fn start(&mut self, _config: &SessionConfig) -> Result<()> {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            Ok(())
        }

        async fn send_input(&mut self, _content: &str) -> Result<()> {
            Ok(())
        }

        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }
    }

    fn slow_start_create_harness(
        _provider: &str,
        _event_tx: broadcast::Sender<SessionEvent>,
    ) -> Result<Box<dyn SessionHarness>> {
        Ok(Box::new(SlowStartHarness))
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

    async fn collect_until_turn_completed(
        mut rx: broadcast::Receiver<PersistedSessionEvent>,
    ) -> Vec<PersistedSessionEvent> {
        let mut events = Vec::new();
        for _ in 0..16 {
            let result = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv()).await;
            let Ok(Ok(event)) = result else {
                break;
            };
            let done = matches!(event.event, SessionEvent::TurnCompleted { .. });
            events.push(event);
            if done {
                break;
            }
        }
        events
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

    #[tokio::test]
    async fn multi_client_subscribers_receive_identical_item_ids_and_seq_order() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        let manager = SessionManager::with_create_harness(store, item_lifecycle_create_harness);

        let created = manager
            .create_session(CreateSessionParams {
                provider: "codex".to_string(),
                wave_run_id: None,
                config: SessionConfig::default(),
            })
            .await
            .expect("create session");
        let _ = wait_for_status(&manager, &created.id, SessionStatus::Active).await;

        let client_a = manager
            .subscribe(&created.id)
            .await
            .expect("subscribe")
            .expect("runtime receiver");
        let client_b = manager
            .subscribe(&created.id)
            .await
            .expect("subscribe")
            .expect("runtime receiver");

        manager
            .send_input(&created.id, "run trace")
            .await
            .expect("send input");

        let events_a = collect_until_turn_completed(client_a).await;
        let events_b = collect_until_turn_completed(client_b).await;

        assert!(!events_a.is_empty());
        assert_eq!(events_a.len(), events_b.len());

        let seqs_a: Vec<i64> = events_a.iter().map(|event| event.seq).collect();
        let seqs_b: Vec<i64> = events_b.iter().map(|event| event.seq).collect();
        assert_eq!(seqs_a, seqs_b);
        assert!(seqs_a.windows(2).all(|window| window[0] < window[1]));

        let ids_a: Vec<String> = events_a
            .iter()
            .filter_map(|event| match &event.event {
                SessionEvent::ItemStarted { item, .. }
                | SessionEvent::ItemCompleted { item, .. } => match item {
                    crate::lfd::sessions::types::SessionItem::Command { id, .. }
                    | crate::lfd::sessions::types::SessionItem::File { id, .. }
                    | crate::lfd::sessions::types::SessionItem::Message { id, .. }
                    | crate::lfd::sessions::types::SessionItem::Thought { id, .. }
                    | crate::lfd::sessions::types::SessionItem::Tool { id, .. } => Some(id.clone()),
                },
                _ => None,
            })
            .collect();
        let ids_b: Vec<String> = events_b
            .iter()
            .filter_map(|event| match &event.event {
                SessionEvent::ItemStarted { item, .. }
                | SessionEvent::ItemCompleted { item, .. } => match item {
                    crate::lfd::sessions::types::SessionItem::Command { id, .. }
                    | crate::lfd::sessions::types::SessionItem::File { id, .. }
                    | crate::lfd::sessions::types::SessionItem::Message { id, .. }
                    | crate::lfd::sessions::types::SessionItem::Thought { id, .. }
                    | crate::lfd::sessions::types::SessionItem::Tool { id, .. } => Some(id.clone()),
                },
                _ => None,
            })
            .collect();

        assert_eq!(
            ids_a,
            vec!["cmd_trace_1".to_string(), "cmd_trace_1".to_string()]
        );
        assert_eq!(ids_a, ids_b);
    }

    #[tokio::test]
    async fn recover_orphaned_sessions_marks_starting_and_active_failed() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        let manager = SessionManager::with_create_harness(store.clone(), fake_create_harness);

        let now = time::OffsetDateTime::now_utc();
        let starting_session = Session {
            id: LfdId::new(),
            provider: "claude".to_string(),
            status: SessionStatus::Starting,
            wave_run_id: None,
            provider_session_id: None,
            config: SessionConfig::default(),
            created_at: now,
            ended_at: None,
        };
        let active_session = Session {
            id: LfdId::new(),
            provider: "codex".to_string(),
            status: SessionStatus::Active,
            wave_run_id: None,
            provider_session_id: None,
            config: SessionConfig::default(),
            created_at: now,
            ended_at: None,
        };
        let ended_session = Session {
            id: LfdId::new(),
            provider: "codex".to_string(),
            status: SessionStatus::Ended,
            wave_run_id: None,
            provider_session_id: None,
            config: SessionConfig::default(),
            created_at: now,
            ended_at: Some(now),
        };

        store
            .create_session(&starting_session)
            .await
            .expect("create starting session");
        store
            .create_session(&active_session)
            .await
            .expect("create active session");
        store
            .create_session(&ended_session)
            .await
            .expect("create ended session");

        let recovered = manager
            .recover_orphaned_sessions()
            .await
            .expect("recover orphaned sessions");
        assert_eq!(recovered, 2);

        let starting = manager
            .get_session(&starting_session.id)
            .await
            .expect("starting session");
        let active = manager
            .get_session(&active_session.id)
            .await
            .expect("active session");
        let ended = manager
            .get_session(&ended_session.id)
            .await
            .expect("ended session");
        assert_eq!(starting.status, SessionStatus::Failed);
        assert_eq!(active.status, SessionStatus::Failed);
        assert_eq!(ended.status, SessionStatus::Ended);

        for session_id in [&starting_session.id, &active_session.id] {
            let events = manager
                .list_events(session_id, None)
                .await
                .expect("list session events");
            assert!(events.iter().any(|event| {
                matches!(
                    &event.event,
                    SessionEvent::Error { code, .. } if code == "lfd_restarted_orphaned_session"
                )
            }));
            assert!(events.iter().any(|event| {
                matches!(
                    &event.event,
                    SessionEvent::StatusChanged { status } if *status == SessionStatus::Failed
                )
            }));
        }
    }

    #[tokio::test]
    async fn stop_session_is_idempotent_while_starting() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        let manager = SessionManager::with_create_harness(store, slow_start_create_harness);

        let created = manager
            .create_session(CreateSessionParams {
                provider: "claude".to_string(),
                wave_run_id: None,
                config: SessionConfig::default(),
            })
            .await
            .expect("create session");

        let first = manager
            .stop_session(&created.id)
            .await
            .expect("first stop should succeed");
        let second = manager
            .stop_session(&created.id)
            .await
            .expect("second stop should be idempotent");

        assert_eq!(first.status, SessionStatus::Ended);
        assert_eq!(second.status, SessionStatus::Ended);
    }
}
