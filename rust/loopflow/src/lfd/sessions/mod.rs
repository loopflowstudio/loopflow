mod harness;
pub(crate) mod opencode_runtime;
pub mod types;
pub mod usage;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, Mutex};

use crate::engine::agent::AgentConfig;
use crate::engine::config::load_config_or_default;
use crate::engine::launch::{prepare_launch_prompt, LaunchPromptInput};
use crate::engine::prompt::ContextBreakdown;
use crate::engine::prompt::{write_prompt_log, Surface};
use crate::engine::structured_reply::ClientContext;
use crate::lfd::id::LfdId;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::sessions::harness::{is_terminal_harness_error, CreateHarnessFn, Harness};
use crate::lfd::sessions::types::{
    ContextSnapshot, CreateSessionParams, PersistedSessionEvent, Session, SessionConfig,
    SessionEvent, SessionItem, SessionStatus,
};
use crate::lfd::store::{SharedStore, StoreError};

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
    #[error("unsupported harness: {0}")]
    UnsupportedHarness(String),
    #[error("harness not implemented yet: {0}")]
    HarnessNotImplemented(String),
    #[error("wave run already has an active session: {0}")]
    WaveRunSessionConflict(String),
    #[error("invalid session config: {0}")]
    InvalidConfig(String),
    #[error("invalid repo_root: {0}")]
    InvalidRepoRoot(String),
    #[error("turn already in progress")]
    TurnAlreadyInProgress,
    #[error("harness error: {0}")]
    Harness(String),
}

struct SessionRuntime {
    harness: Mutex<Box<dyn Harness>>,
    events_tx: broadcast::Sender<PersistedSessionEvent>,
    next_seq: AtomicI64,
    seeded_user_prompt: Option<String>,
    /// Paths of injected skill directories for cleanup.
    injected_skills: Mutex<Vec<PathBuf>>,
}

impl std::fmt::Debug for SessionRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionRuntime").finish()
    }
}

struct SessionManagerInner {
    store: SharedStore,
    create_harness: CreateHarnessFn,
    scheduler: Option<Arc<Scheduler>>,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionStartupRecovery {
    pub sessions_failed: u32,
    pub opencode_servers_reaped: u32,
    pub reap_errors: u32,
}

impl SessionManager {
    pub fn new(store: SharedStore) -> Self {
        Self::with_create_harness_and_scheduler(store, harness::default_create_harness, None)
    }

    pub fn new_with_scheduler(store: SharedStore, scheduler: Arc<Scheduler>) -> Self {
        Self::with_create_harness_and_scheduler(
            store,
            harness::default_create_harness,
            Some(scheduler),
        )
    }

    #[cfg(test)]
    fn with_create_harness(store: SharedStore, create_harness: CreateHarnessFn) -> Self {
        Self::with_create_harness_and_scheduler(store, create_harness, None)
    }

    fn with_create_harness_and_scheduler(
        store: SharedStore,
        create_harness: CreateHarnessFn,
        scheduler: Option<Arc<Scheduler>>,
    ) -> Self {
        Self {
            inner: Arc::new(SessionManagerInner {
                store,
                create_harness,
                scheduler,
                runtimes: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub async fn create_session(
        &self,
        params: CreateSessionParams,
    ) -> Result<Session, SessionManagerError> {
        let harness_name = resolve_harness(&params.harness)?;
        let (session_config, prepared_prompt, breakdown) =
            self.prepare_session_prompt(params.config).await?;

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
            harness: harness_name.clone(),
            status: SessionStatus::Starting,
            wave_run_id: params.wave_run_id,
            provider_session_id: None,
            config: session_config,
            created_at: time::OffsetDateTime::now_utc(),
            ended_at: None,
        };
        let (harness_events_tx, harness_events_rx) = mpsc::unbounded_channel();
        let harness = (self.inner.create_harness)(&harness_name, harness_events_tx)
            .map_err(|err| SessionManagerError::Harness(err.to_string()))?;
        self.inner.store.create_session(&session).await?;

        let (events_tx, _) = broadcast::channel(LIVE_EVENT_BUFFER);
        let seeded_user_prompt = normalized_seeded_user_prompt(&prepared_prompt.task_prompt);
        let runtime = Arc::new(SessionRuntime {
            harness: Mutex::new(harness),
            events_tx,
            next_seq: AtomicI64::new(0),
            seeded_user_prompt,
            injected_skills: Mutex::new(Vec::new()),
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
        self.append_runtime_event(
            &session.id,
            &runtime,
            SessionEvent::ContextSnapshot {
                snapshot: ContextSnapshot::from(&breakdown),
            },
        )
        .await?;

        self.register_wave_session(session.wave_run_id.as_deref())
            .await;
        self.spawn_harness_event_bridge(session.id.clone(), runtime.clone(), harness_events_rx);
        let auto_start = session.wave_run_id.is_some();
        let repo_root = PathBuf::from(&session.config.repo_root);
        self.spawn_harness_startup(
            session.id.clone(),
            runtime.clone(),
            prepared_prompt,
            auto_start,
            harness_name.clone(),
            repo_root,
        );

        Ok(session)
    }

    async fn prepare_session_prompt(
        &self,
        mut config: SessionConfig,
    ) -> Result<(SessionConfig, AgentConfig, ContextBreakdown), SessionManagerError> {
        let repo_root = validate_repo_root(&config.repo_root)?;
        let step = config.step.trim().to_string();
        if step.is_empty() {
            return Err(SessionManagerError::InvalidConfig(
                "step is required".to_string(),
            ));
        }

        let cwd = resolve_cwd(&repo_root, config.cwd.as_deref())?;
        config.step = step.clone();
        config.repo_root = repo_root.to_string_lossy().to_string();
        config.cwd = cwd.as_ref().map(|path| path.to_string_lossy().to_string());
        // Sessions are primarily a Concerto feature; default to macOS surface.
        let surface = config.surface.unwrap_or(Surface::ConcertoMac);

        let file_config = load_config_or_default(Some(&repo_root));
        let prepared = prepare_launch_prompt(
            &file_config,
            LaunchPromptInput {
                repo_root: repo_root.to_path_buf(),
                step: Some(step.clone()),
                resolved_step: None,
                surface,
                directions: config.directions.clone(),
                area: config.area.clone(),
                wave: config.wave.clone(),
                message: config.message.clone(),
                agent: config.agent.clone(),
                cwd,
                max_turns: config.max_turns,
                yolo_mode: config.yolo_mode,
                include_config_directions: false,
                include_config_area: true,
                source_overrides: Default::default(),
                summary: None,
                client_context: ClientContext {
                    has_ui: config.client_has_ui.unwrap_or(false),
                    compact: config.client_compact.unwrap_or(false),
                },
            },
        )
        .map_err(|err| SessionManagerError::InvalidConfig(err.to_string()))?;

        let _ = write_prompt_log(&repo_root, &prepared.prompt, &step, None);
        Ok((config, prepared.config, prepared.breakdown))
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

            self.mark_session_failed(session_id, &runtime, "send_input_failed", err.to_string())
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

        if session.status == SessionStatus::Starting {
            let runtime = self.runtime(session_id).await;
            self.set_status(
                session_id,
                SessionStatus::Failed,
                Some(time::OffsetDateTime::now_utc().unix_timestamp()),
                runtime.clone(),
            )
            .await?;
            return self.get_session(session_id).await;
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

        // Clean up injected skill files.
        if let Some(ref runtime) = runtime {
            let injected = runtime.injected_skills.lock().await;
            if !injected.is_empty() {
                crate::engine::skills::cleanup_injected_skills(&injected);
                tracing::debug!(
                    session_id = %session_id,
                    count = injected.len(),
                    "cleaned up injected skills"
                );
            }
        }

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

    pub async fn seeded_user_prompt(
        &self,
        session_id: &LfdId,
    ) -> Result<Option<String>, SessionManagerError> {
        let _ = self.get_session(session_id).await?;
        Ok(self
            .runtime(session_id)
            .await
            .and_then(|runtime| runtime.seeded_user_prompt.clone()))
    }

    pub async fn recover_orphaned_sessions(
        &self,
    ) -> Result<SessionStartupRecovery, SessionManagerError> {
        let mut recovery = SessionStartupRecovery::default();
        let now = time::OffsetDateTime::now_utc();
        let now_ts = now.unix_timestamp();
        let sessions = self
            .inner
            .store
            .list_sessions_by_statuses(&[SessionStatus::Starting, SessionStatus::Active])
            .await?;

        for session in &sessions {
            let existing_events = self
                .inner
                .store
                .list_session_events(&session.id, None)
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
                .append_session_event(&session.id, next_seq, &error_event, now_ts)
                .await?;
            next_seq += 1;

            let status_event = SessionEvent::StatusChanged {
                status: SessionStatus::Failed,
            };
            self.inner
                .store
                .append_session_event(&session.id, next_seq, &status_event, now_ts)
                .await?;

            self.inner
                .store
                .update_session_status(&session.id, SessionStatus::Failed, Some(now_ts))
                .await?;
            recovery.sessions_failed += 1;
        }

        let opencode_reap = opencode_runtime::reap_orphaned_opencode_servers();
        recovery.opencode_servers_reaped = opencode_reap.reaped;
        recovery.reap_errors = opencode_reap.errors;
        Ok(recovery)
    }

    fn spawn_harness_event_bridge(
        &self,
        session_id: LfdId,
        runtime: Arc<SessionRuntime>,
        mut harness_events_rx: mpsc::UnboundedReceiver<SessionEvent>,
    ) {
        let manager = self.clone();
        tokio::spawn(async move {
            while let Some(event) = harness_events_rx.recv().await {
                match event {
                    SessionEvent::ProviderSessionId {
                        provider_session_id,
                    } => {
                        manager
                            .handle_provider_session_id(&session_id, &runtime, provider_session_id)
                            .await;
                    }
                    SessionEvent::Error {
                        ref code,
                        ref message,
                    } => {
                        let fatal = is_terminal_harness_error(code);
                        manager
                            .append_runtime_event_or_warn(
                                &session_id,
                                &runtime,
                                SessionEvent::Error {
                                    code: code.clone(),
                                    message: message.clone(),
                                },
                                "error",
                            )
                            .await;
                        if fatal {
                            manager
                                .mark_session_failed(&session_id, &runtime, code, message.clone())
                                .await;
                        }
                    }
                    event => {
                        manager
                            .append_runtime_event_or_warn(&session_id, &runtime, event, "event")
                            .await;
                    }
                }
            }
        });
    }

    fn spawn_harness_startup(
        &self,
        session_id: LfdId,
        runtime: Arc<SessionRuntime>,
        launch: AgentConfig,
        auto_start: bool,
        harness_name: String,
        repo_root: PathBuf,
    ) {
        let manager = self.clone();
        tokio::spawn(async move {
            // Inject steps and directions as .agents/skills/ for agent auto-discovery.
            // repo_root: where to read .lf/steps/ and .lf/directions/.
            // cwd: where to write .agents/skills/ (agents discover relative to cwd).
            if let Some(cwd) = &launch.cwd {
                let injected = crate::engine::skills::inject_skills(&repo_root, cwd);
                if !injected.is_empty() {
                    tracing::debug!(
                        session_id = %session_id,
                        harness = %harness_name,
                        count = injected.len(),
                        "injected skills into .agents/skills/"
                    );
                    *runtime.injected_skills.lock().await = injected;
                }
            }

            let result = {
                let mut harness = runtime.harness.lock().await;
                harness.start(&launch).await
            };

            let current_status = match manager.inner.store.get_session(&session_id).await {
                Ok(Some(session)) => Some(session.status),
                Ok(None) => None,
                Err(err) => {
                    tracing::warn!(
                        session_id = %session_id,
                        error = %err,
                        "failed to read session status after startup"
                    );
                    None
                }
            };

            match result {
                Ok(()) => {
                    if current_status != Some(SessionStatus::Starting) {
                        if current_status.is_some_and(SessionStatus::is_terminal) {
                            manager
                                .stop_harness_runtime(&session_id, &runtime, "startup_aborted")
                                .await;
                        }
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
                        return;
                    }

                    // Auto-start: send empty input so the harness delivers
                    // the task_prompt (step content) as the agent's first turn.
                    if auto_start {
                        if let Err(err) = manager
                            .append_seeded_user_prompt_event(
                                &session_id,
                                &runtime,
                                launch.task_prompt.trim(),
                            )
                            .await
                        {
                            tracing::warn!(
                                session_id = %session_id,
                                error = %err,
                                "failed to append seeded user prompt event"
                            );
                        }
                        let send_result = {
                            let mut harness = runtime.harness.lock().await;
                            harness.send_input("").await
                        };
                        if let Err(err) = send_result {
                            tracing::warn!(
                                session_id = %session_id,
                                error = %err,
                                "session auto-start failed"
                            );
                        }
                    }
                }
                Err(err) => {
                    if current_status.is_some_and(SessionStatus::is_terminal) {
                        manager
                            .stop_harness_runtime(&session_id, &runtime, "startup_aborted")
                            .await;
                        return;
                    }
                    manager
                        .mark_session_failed(
                            &session_id,
                            &runtime,
                            "session_start_failed",
                            err.to_string(),
                        )
                        .await;
                }
            }
        });
    }

    async fn append_seeded_user_prompt_event(
        &self,
        session_id: &LfdId,
        runtime: &Arc<SessionRuntime>,
        prompt: &str,
    ) -> Result<(), SessionManagerError> {
        if prompt.is_empty() {
            return Ok(());
        }

        self.append_runtime_event(
            session_id,
            runtime,
            SessionEvent::ItemCompleted {
                turn_id: "turn_seed_user_prompt".to_string(),
                item: SessionItem::Message {
                    id: "msg_seed_user_prompt".to_string(),
                    text: prompt.to_string(),
                    phase: Some("user".to_string()),
                },
            },
        )
        .await
        .map(|_| ())
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
        code: &str,
        message: String,
    ) {
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

        if status.is_terminal() {
            self.on_session_terminal(session_id).await;
        }

        Ok(())
    }

    async fn append_runtime_event_or_warn(
        &self,
        session_id: &LfdId,
        runtime: &Arc<SessionRuntime>,
        event: SessionEvent,
        context: &'static str,
    ) {
        if let Err(err) = self.append_runtime_event(session_id, runtime, event).await {
            tracing::warn!(
                session_id = %session_id,
                error = %err,
                context,
                "failed to persist harness event"
            );
        }
    }

    async fn append_runtime_event(
        &self,
        session_id: &LfdId,
        runtime: &Arc<SessionRuntime>,
        event: SessionEvent,
    ) -> Result<(), SessionManagerError> {
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
        let _ = runtime.events_tx.send(persisted);
        Ok(())
    }

    async fn runtime(&self, session_id: &LfdId) -> Option<Arc<SessionRuntime>> {
        let runtimes = self.inner.runtimes.lock().await;
        runtimes.get(session_id).cloned()
    }

    async fn remove_runtime(&self, session_id: &LfdId) {
        let mut runtimes = self.inner.runtimes.lock().await;
        runtimes.remove(session_id);
    }

    async fn stop_harness_runtime(
        &self,
        session_id: &LfdId,
        runtime: &Arc<SessionRuntime>,
        context: &'static str,
    ) {
        let stop_result = {
            let mut harness = runtime.harness.lock().await;
            harness.stop().await
        };
        if let Err(err) = stop_result {
            tracing::warn!(
                session_id = %session_id,
                error = %err,
                context,
                "failed to stop session harness"
            );
        }
        self.remove_runtime(session_id).await;
    }

    async fn register_wave_session(&self, wave_run_id: Option<&str>) {
        let Some((scheduler, wave_id)) = self.scheduler_wave_id_for_run(wave_run_id).await else {
            return;
        };
        if !scheduler.register_session(&wave_id) {
            tracing::debug!(wave_id, "session already registered for wave");
        }
    }

    async fn on_session_terminal(&self, session_id: &LfdId) {
        let session = match self.inner.store.get_session(session_id).await {
            Ok(Some(session)) => session,
            Ok(None) => return,
            Err(err) => {
                tracing::warn!(session_id = %session_id, error = %err, "failed to load terminal session");
                return;
            }
        };

        let Some((scheduler, wave_id)) = self
            .scheduler_wave_id_for_run(session.wave_run_id.as_deref())
            .await
        else {
            return;
        };
        scheduler.unregister_session(&wave_id);
    }

    async fn scheduler_wave_id_for_run(
        &self,
        wave_run_id: Option<&str>,
    ) -> Option<(Arc<Scheduler>, String)> {
        let scheduler = self.inner.scheduler.clone()?;
        let wave_run_id = wave_run_id?;
        let wave_id = self.wave_id_for_wave_run(wave_run_id).await?;
        Some((scheduler, wave_id))
    }

    async fn wave_id_for_wave_run(&self, wave_run_id: &str) -> Option<String> {
        let run_id = match wave_run_id.parse::<LfdId>() {
            Ok(run_id) => run_id,
            Err(err) => {
                tracing::warn!(wave_run_id, error = %err, "invalid wave_run_id on session");
                return None;
            }
        };

        match self.inner.store.get_wave_run(&run_id).await {
            Ok(Some(run)) => Some(run.wave_id.to_string()),
            Ok(None) => {
                tracing::warn!(wave_run_id, "wave run referenced by session not found");
                None
            }
            Err(err) => {
                tracing::warn!(wave_run_id, error = %err, "failed to resolve wave for session");
                None
            }
        }
    }
}

fn resolve_harness(name: &str) -> Result<String, SessionManagerError> {
    let requested = name.trim().to_ascii_lowercase();
    if matches!(requested.as_str(), "gemini") {
        return Err(SessionManagerError::HarnessNotImplemented(requested));
    }

    harness::canonical_harness(&requested)
        .map(ToString::to_string)
        .ok_or(SessionManagerError::UnsupportedHarness(requested))
}

fn validate_repo_root(repo_root: &str) -> Result<PathBuf, SessionManagerError> {
    let raw = repo_root.trim();
    if raw.is_empty() {
        return Err(SessionManagerError::InvalidRepoRoot(
            "path is empty".to_string(),
        ));
    }

    let path = PathBuf::from(raw);
    if !path.exists() {
        return Err(SessionManagerError::InvalidRepoRoot(format!(
            "path does not exist: {raw}"
        )));
    }
    if !path.is_dir() {
        return Err(SessionManagerError::InvalidRepoRoot(format!(
            "path is not a directory: {raw}"
        )));
    }
    let canonical = path.canonicalize().map_err(|err| {
        SessionManagerError::InvalidRepoRoot(format!("failed to resolve repo_root '{raw}': {err}"))
    })?;
    if !canonical.join(".lf").is_dir() {
        return Err(SessionManagerError::InvalidRepoRoot(format!(
            "missing .lf/ in repo root: {raw}"
        )));
    }
    Ok(canonical)
}

fn resolve_cwd(
    repo_root: &Path,
    cwd: Option<&str>,
) -> Result<Option<PathBuf>, SessionManagerError> {
    let Some(trimmed) = cwd.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    let path = PathBuf::from(trimmed);
    let resolved = if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    };

    if !resolved.exists() {
        return Err(SessionManagerError::InvalidConfig(format!(
            "cwd does not exist: {}",
            resolved.display()
        )));
    }
    if !resolved.is_dir() {
        return Err(SessionManagerError::InvalidConfig(format!(
            "cwd is not a directory: {}",
            resolved.display()
        )));
    }

    let canonical_cwd = resolved.canonicalize().map_err(|err| {
        SessionManagerError::InvalidConfig(format!(
            "failed to resolve cwd '{}': {err}",
            resolved.display()
        ))
    })?;
    if !canonical_cwd.starts_with(repo_root) {
        return Err(SessionManagerError::InvalidConfig(format!(
            "cwd must be inside repo_root: {}",
            canonical_cwd.display()
        )));
    }

    Ok(Some(canonical_cwd))
}

fn normalized_seeded_user_prompt(prompt: &str) -> Option<String> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::scheduler::Scheduler;
    use crate::lfd::sessions::harness::{Harness, HarnessError};
    use crate::lfd::sessions::types::{SessionConfig, SessionEvent, SessionItem, TurnStatus};
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{Wave, WaveRun};
    use anyhow::Result;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::tempdir;
    use tokio::sync::mpsc;

    #[derive(Debug)]
    struct FakeHarness {
        tx: mpsc::UnboundedSender<SessionEvent>,
    }

    #[async_trait]
    impl Harness for FakeHarness {
        async fn start(&mut self, _config: &AgentConfig) -> Result<()> {
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
        _harness: &str,
        event_tx: mpsc::UnboundedSender<SessionEvent>,
    ) -> Result<Box<dyn Harness>> {
        Ok(Box::new(FakeHarness { tx: event_tx }))
    }

    #[derive(Debug)]
    struct BusyHarness;

    #[async_trait]
    impl Harness for BusyHarness {
        async fn start(&mut self, _config: &AgentConfig) -> Result<()> {
            Ok(())
        }

        async fn send_input(&mut self, _content: &str) -> Result<()> {
            Err(HarnessError::TurnAlreadyInProgress.into())
        }

        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }
    }

    fn busy_create_harness(
        _harness: &str,
        _event_tx: mpsc::UnboundedSender<SessionEvent>,
    ) -> Result<Box<dyn Harness>> {
        Ok(Box::new(BusyHarness))
    }

    #[derive(Debug)]
    struct ResumeAwareHarness {
        tx: mpsc::UnboundedSender<SessionEvent>,
        send_count: usize,
        provider_session_id: Option<String>,
    }

    #[async_trait]
    impl Harness for ResumeAwareHarness {
        async fn start(&mut self, _config: &AgentConfig) -> Result<()> {
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
        _harness: &str,
        event_tx: mpsc::UnboundedSender<SessionEvent>,
    ) -> Result<Box<dyn Harness>> {
        Ok(Box::new(ResumeAwareHarness {
            tx: event_tx,
            send_count: 0,
            provider_session_id: None,
        }))
    }

    static STARTING_STOP_CALLED: AtomicBool = AtomicBool::new(false);

    #[derive(Debug)]
    struct SlowStartHarness;

    #[async_trait]
    impl Harness for SlowStartHarness {
        async fn start(&mut self, _config: &AgentConfig) -> Result<()> {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok(())
        }

        async fn send_input(&mut self, _content: &str) -> Result<()> {
            Ok(())
        }

        async fn stop(&mut self) -> Result<()> {
            STARTING_STOP_CALLED.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    fn slow_start_create_harness(
        _harness: &str,
        _event_tx: mpsc::UnboundedSender<SessionEvent>,
    ) -> Result<Box<dyn Harness>> {
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

    fn test_session_config(repo_root: &std::path::Path) -> SessionConfig {
        std::fs::create_dir_all(repo_root.join(".lf")).expect("create .lf for tests");
        SessionConfig {
            step: "design".to_string(),
            repo_root: repo_root.to_string_lossy().to_string(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn prepare_session_prompt_uses_surface_from_config() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        std::fs::create_dir_all(tmp.path().join(".lf/steps")).expect("create .lf/steps");
        std::fs::write(tmp.path().join(".lf/steps/design.md"), "Design it.").expect("write step");

        let manager = SessionManager::with_create_harness(store, fake_create_harness);
        let (_, prompt, _) = manager
            .prepare_session_prompt(SessionConfig {
                step: "design".to_string(),
                repo_root: tmp.path().to_string_lossy().to_string(),
                surface: Some(Surface::ConcertoIphone),
                ..Default::default()
            })
            .await
            .expect("prepare session prompt");

        assert!(prompt.system_prompt.contains("Surface: Concerto (iPhone)"));
    }

    #[tokio::test]
    async fn create_session_emits_context_snapshot_event() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );

        std::fs::create_dir_all(tmp.path().join(".lf/steps")).expect("create .lf/steps");
        std::fs::write(tmp.path().join(".lf/steps/design.md"), "Design the system.")
            .expect("write step");

        let manager = SessionManager::with_create_harness(store, fake_create_harness);
        let created = manager
            .create_session(CreateSessionParams {
                harness: "codex".to_string(),
                wave_run_id: None,
                config: SessionConfig {
                    step: "design".to_string(),
                    repo_root: tmp.path().to_string_lossy().to_string(),
                    ..Default::default()
                },
            })
            .await
            .expect("create session");

        let events = manager
            .list_events(&created.id, None)
            .await
            .expect("list events");

        let snapshot = events.iter().find_map(|event| match &event.event {
            SessionEvent::ContextSnapshot { snapshot } => Some(snapshot.clone()),
            _ => None,
        });
        let snapshot = snapshot.expect("context snapshot event");
        assert!(snapshot.total > 0);
        assert!(snapshot.sources.contains_key("step"));
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
        std::fs::create_dir_all(tmp.path().join(".lf")).expect("create .lf for tests");
        let created = manager
            .create_session(CreateSessionParams {
                harness: "codex".to_string(),
                wave_run_id: Some("run_1".to_string()),
                config: SessionConfig {
                    step: "design".to_string(),
                    repo_root: tmp.path().to_string_lossy().to_string(),
                    agent: Some("gpt-5.1-codex".to_string()),
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
    async fn auto_start_seeds_user_prompt_event() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );

        let steps_dir = tmp.path().join(".lf").join("steps");
        std::fs::create_dir_all(&steps_dir).expect("create steps dir");
        std::fs::write(
            steps_dir.join("seed-step.md"),
            "Seeded prompt text for interactive wave start.",
        )
        .expect("write test step");

        let manager = SessionManager::with_create_harness(store, fake_create_harness);
        let created = manager
            .create_session(CreateSessionParams {
                harness: "codex".to_string(),
                wave_run_id: Some("run_seed".to_string()),
                config: SessionConfig {
                    step: "seed-step".to_string(),
                    repo_root: tmp.path().to_string_lossy().to_string(),
                    agent: Some("gpt-5.1-codex".to_string()),
                    cwd: Some(tmp.path().to_string_lossy().to_string()),
                    ..Default::default()
                },
            })
            .await
            .expect("create session");

        let _ = wait_for_status(&manager, &created.id, SessionStatus::Active).await;

        let mut saw_seeded_prompt = false;
        for _ in 0..50 {
            let events = manager
                .list_events(&created.id, None)
                .await
                .expect("list events");
            saw_seeded_prompt = events.iter().any(|event| {
                matches!(
                    &event.event,
                    SessionEvent::ItemCompleted {
                        item:
                            SessionItem::Message {
                                text,
                                phase: Some(phase),
                                ..
                            },
                        ..
                    } if phase == "user" && text.contains("Seeded prompt text for interactive wave start.")
                )
            });
            if saw_seeded_prompt {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        assert!(saw_seeded_prompt);
    }

    #[tokio::test]
    async fn create_session_rejects_unsupported_harness() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        // Use default_create_harness (not fake) so unsupported harnesses are rejected.
        let manager = SessionManager::new(store);

        let err = manager
            .create_session(CreateSessionParams {
                harness: "nonexistent".to_string(),
                wave_run_id: None,
                config: SessionConfig::default(),
            })
            .await
            .expect_err("unsupported harness should fail");

        assert!(matches!(
            err,
            SessionManagerError::UnsupportedHarness(ref name) if name == "nonexistent"
        ));
    }

    #[tokio::test]
    async fn create_session_marks_known_unimplemented_harness() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        let manager = SessionManager::new(store);

        let err = manager
            .create_session(CreateSessionParams {
                harness: "gemini".to_string(),
                wave_run_id: None,
                config: SessionConfig::default(),
            })
            .await
            .expect_err("gemini should be explicitly not implemented");

        assert!(matches!(
            err,
            SessionManagerError::HarnessNotImplemented(ref name) if name == "gemini"
        ));
    }

    #[tokio::test]
    async fn create_session_accepts_opencode_harness_name() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let repo_root = tmp.path().join("repo");
        std::fs::create_dir_all(repo_root.join(".lf")).expect("create .lf for tests");
        std::fs::create_dir_all(repo_root.join(".lf/steps")).expect("create .lf/steps");
        std::fs::write(repo_root.join(".lf/steps/design.md"), "stub design")
            .expect("write stub step");

        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        let manager = SessionManager::with_create_harness(store, fake_create_harness);

        let created = manager
            .create_session(CreateSessionParams {
                harness: "opencode".to_string(),
                wave_run_id: None,
                config: SessionConfig {
                    step: "design".to_string(),
                    repo_root: repo_root.to_string_lossy().to_string(),
                    ..Default::default()
                },
            })
            .await
            .expect("opencode should resolve as a supported harness");

        assert_eq!(created.harness, "opencode");
    }

    #[tokio::test]
    async fn create_session_rejects_invalid_repo_root() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        let manager = SessionManager::with_create_harness(store, fake_create_harness);

        let err = manager
            .create_session(CreateSessionParams {
                harness: "claude".to_string(),
                wave_run_id: None,
                config: SessionConfig {
                    step: "design".to_string(),
                    repo_root: tmp.path().join("missing").to_string_lossy().to_string(),
                    ..Default::default()
                },
            })
            .await
            .expect_err("invalid repo root should fail");

        assert!(matches!(err, SessionManagerError::InvalidRepoRoot(_)));
    }

    #[tokio::test]
    async fn create_session_rejects_cwd_outside_repo_root() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let repo_root = tmp.path().join("repo");
        std::fs::create_dir_all(repo_root.join(".lf")).expect("create .lf for tests");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&outside).expect("create outside dir");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        let manager = SessionManager::with_create_harness(store, fake_create_harness);

        let err = manager
            .create_session(CreateSessionParams {
                harness: "claude".to_string(),
                wave_run_id: None,
                config: SessionConfig {
                    step: "design".to_string(),
                    repo_root: repo_root.to_string_lossy().to_string(),
                    cwd: Some(outside.to_string_lossy().to_string()),
                    ..Default::default()
                },
            })
            .await
            .expect_err("cwd outside repo root should fail");

        assert!(matches!(err, SessionManagerError::InvalidConfig(_)));
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
                harness: "codex".to_string(),
                wave_run_id: Some("run_1".to_string()),
                config: test_session_config(tmp.path()),
            })
            .await
            .expect("first session should create");
        let _ = wait_for_status(&manager, &created.id, SessionStatus::Active).await;

        let err = manager
            .create_session(CreateSessionParams {
                harness: "codex".to_string(),
                wave_run_id: Some("run_1".to_string()),
                config: test_session_config(tmp.path()),
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
                harness: "claude".to_string(),
                wave_run_id: None,
                config: test_session_config(tmp.path()),
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
                harness: "claude".to_string(),
                wave_run_id: None,
                config: test_session_config(tmp.path()),
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
    async fn recover_orphaned_sessions_marks_active_as_failed() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );

        let manager = SessionManager::with_create_harness(store.clone(), fake_create_harness);
        let created = manager
            .create_session(CreateSessionParams {
                harness: "claude".to_string(),
                wave_run_id: None,
                config: test_session_config(tmp.path()),
            })
            .await
            .expect("create session");
        let _ = wait_for_status(&manager, &created.id, SessionStatus::Active).await;

        // Simulate a new SessionManager on restart (no runtimes, orphaned session in DB).
        let fresh_manager = SessionManager::with_create_harness(store, fake_create_harness);
        let recovered = fresh_manager
            .recover_orphaned_sessions()
            .await
            .expect("orphan recovery");
        assert_eq!(recovered.sessions_failed, 1);

        let session = fresh_manager
            .get_session(&created.id)
            .await
            .expect("session should exist");
        assert_eq!(session.status, SessionStatus::Failed);

        let events = fresh_manager
            .list_events(&created.id, None)
            .await
            .expect("list events");
        let has_orphan_error = events.iter().any(|event| {
            matches!(
                &event.event,
                SessionEvent::Error { code, .. } if code == "lfd_restarted_orphaned_session"
            )
        });
        assert!(has_orphan_error);
    }

    #[tokio::test(start_paused = true)]
    async fn stop_session_while_starting_marks_failed_and_cleans_up_harness() {
        STARTING_STOP_CALLED.store(false, Ordering::SeqCst);
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
                harness: "claude".to_string(),
                wave_run_id: None,
                config: test_session_config(tmp.path()),
            })
            .await
            .expect("create session");

        let stopped = manager
            .stop_session(&created.id)
            .await
            .expect("stop session while starting");
        assert_eq!(stopped.status, SessionStatus::Failed);
        assert!(
            !STARTING_STOP_CALLED.load(Ordering::SeqCst),
            "stop_session should not block on harness.stop while startup is running"
        );

        tokio::time::sleep(Duration::from_millis(250)).await;
        let after_start_finishes = manager
            .get_session(&created.id)
            .await
            .expect("session should exist");
        assert_eq!(after_start_finishes.status, SessionStatus::Failed);
        assert!(
            STARTING_STOP_CALLED.load(Ordering::SeqCst),
            "startup cleanup should stop the harness once startup returns"
        );
    }

    #[tokio::test]
    async fn wave_session_registers_and_unregisters_scheduler_session() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        let scheduler = Arc::new(Scheduler::new(1));
        let manager = SessionManager::with_create_harness_and_scheduler(
            store.clone(),
            fake_create_harness,
            Some(scheduler.clone()),
        );

        std::fs::create_dir_all(tmp.path().join(".lf")).expect("create .lf for tests");
        let wave_id = LfdId::new();
        let wave = Wave::new(
            wave_id.clone(),
            "wave-a".to_string(),
            tmp.path().to_string_lossy().to_string(),
        );
        store.create_wave(&wave).await.expect("create wave");

        let run_id = LfdId::new();
        let mut run = WaveRun::new(run_id.clone(), wave_id.clone());
        run.worktree = tmp.path().to_string_lossy().to_string();
        run.branch = "wave-a".to_string();
        store.create_wave_run(&run).await.expect("create wave run");

        let created = manager
            .create_session(CreateSessionParams {
                harness: "codex".to_string(),
                wave_run_id: Some(run_id.to_string()),
                config: test_session_config(tmp.path()),
            })
            .await
            .expect("create session");
        let _ = wait_for_status(&manager, &created.id, SessionStatus::Active).await;

        assert!(scheduler.has_active_session(wave_id.as_str()));

        manager
            .stop_session(&created.id)
            .await
            .expect("stop session");
        assert!(!scheduler.has_active_session(wave_id.as_str()));
    }
}
