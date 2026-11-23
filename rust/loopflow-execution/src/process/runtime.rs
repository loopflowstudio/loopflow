//! Live resources used by an lf invocation to run its provider.
//!
//! LfProcess supplies the identity. Retries replace AgentProcess under that same
//! invocation; recording has no authority to launch, replace or stop a process.

use crate::process::attachment;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::harness::agent_process::close_session_agent_process;
use crate::harness::Recording;
use crate::process::SessionAttachment;
use crate::session_record::{
    new_artifact_key, record_error, row_store, AgentProcessRequest, CaptureHandoff,
    SessionCaptureSpec, SessionRecorder, CAPTURE_KEY_ENV,
};
use crate::store::{StoreError, StoreResult};
use loopflow_model::chat::types::ConversationEvent;
use loopflow_model::id::AgentSessionId;
use loopflow_storage::sqlite::SqliteStore;

/// Shared live resources for an lf invocation's provider work, with no separate
/// execution identity. A skill keeps one runtime across provider retries.
/// Harnesses retain immutable attachment snapshots for dispatch.
#[derive(Debug, Clone)]
pub struct LfProcessRuntime(Arc<Mutex<RuntimeState>>);

pub fn finish_session_attachment(
    store: &SqliteStore,
    session: &str,
    attachment: &SessionAttachment,
    outcome: &str,
) -> StoreResult<()> {
    attachment::finish_session_attachment(store, session, attachment, outcome, None, || {
        close_session_agent_process(store, session)
    })
}

pub fn register_session_attachment_interrupt(
    store: &loopflow_storage::sqlite::SqliteStore,
    session: String,
    attachment: crate::process::SessionAttachment,
) {
    let store = store.clone();
    crate::agent::register_interrupt_cleanup(move || {
        match finish_session_attachment(&store, &session, &attachment, "interrupted") {
            Ok(()) | Err(StoreError::InvalidAuthority(_)) => {}
            Err(error) => tracing::warn!(%error, %session, "record interrupted Session connection"),
        }
    });
}

/// Whether a retry resuming `resume` keeps the conversation whose saved
/// thread is `saved`. The thread goes on under any model; only a move to
/// another provider may leave it behind, and only to start fresh.
fn retry_keeps_its_thread(
    saved: Option<&AgentSessionId>,
    resume: Option<&AgentSessionId>,
    moved: bool,
) -> bool {
    saved == resume || (resume.is_none() && moved)
}

impl LfProcessRuntime {
    pub fn new(recorder: SessionRecorder) -> Self {
        Self(Arc::new(Mutex::new(RuntimeState::new(recorder))))
    }

    /// Append an input only after the same transaction admits its attachment.
    pub fn continue_with_context(
        session: &str,
        mut spec: SessionCaptureSpec,
        context: &crate::trace::PreparedTurnContext,
        process: AgentProcessRequest,
    ) -> StoreResult<Self> {
        let home = crate::store::lf_home_dir();
        let store = crate::store::open_sqlite(
            &crate::store::database_path_from_env().map_err(record_error)?,
        )?;
        let lf_process_id = crate::journal::current_lf_process_id().ok_or_else(|| {
            StoreError::InvalidAuthority("Session input requires an admitted Process".into())
        })?;
        let mut next = store.session(session)?.ok_or(StoreError::NotFound)?;
        let expected = store.session_attachment(session)?;
        next.artifact_key = Some(new_artifact_key());
        next.input_published = false;
        // The row records the agent this input actually launches.
        next.provider = Some(spec.harness.clone());
        next.model = spec.model.clone();
        let (next, attachment) = attachment::claim_session_input(
            &store,
            next,
            expected.as_ref(),
            &lf_process_id,
            || close_session_agent_process(&store, session),
        )?;
        // Continue the Session's attribution; the manifest still describes the
        // actual process cwd supplied by the caller.
        spec.subjects = crate::ops::human_session::capture_subjects(&next);
        let result = SessionRecorder::begin_reserved_at(
            &home,
            spec,
            next.input_key()
                .map_err(|e| record_error(std::io::Error::other(e)))?
                .clone(),
            next.caller_artifact_key.clone(),
            Some(process),
            context,
            |_| {
                store.with_session_attachment(session, &attachment, || {
                    store.publish_capture(session, next.captured)
                })
            },
        );
        match result.map(Self::new) {
            Ok(runtime) => {
                runtime
                    .0
                    .lock()
                    .expect("Process runtime mutex poisoned")
                    .owner = Some(crate::harness::Recording {
                    store: store.clone(),
                    session: session.to_string(),
                    attachment,
                });
                runtime.register_interrupt();
                Ok(runtime)
            }
            Err(error) => {
                // Publication did not start a provider or complete a turn. Keep
                // the reservation and release only the attachment acquired above.
                if let Err(release) = store.release_session_attachment(session, &attachment) {
                    tracing::warn!(%release, %session, "release attachment after capture publication failure");
                }
                Err(error)
            }
        }
    }

    pub fn artifact_key(&self) -> String {
        self.0
            .lock()
            .expect("Process runtime mutex poisoned")
            .recorder
            .artifact_key()
    }

    pub fn artifact_dir(&self) -> PathBuf {
        self.0
            .lock()
            .expect("Process runtime mutex poisoned")
            .recorder
            .artifact_dir()
    }

    /// Record which skill text this launch runs. A store that cannot take it
    /// costs the grouping of this run, never the launch.
    pub fn record_skill_version(&self, version: &str) {
        let runtime = self.0.lock().expect("Process runtime mutex poisoned");
        let recorded = row_store(&runtime.recorder.dir).and_then(|store| {
            store.record_skill_version(&runtime.recorder.manifest.artifact_key, version)
        });
        if let Err(error) = recorded {
            tracing::warn!(%error, "could not record the skill version");
        }
    }

    /// Claim an admitted conversation and retain the exact provider provenance
    /// used by its tools. A later attachment transfer never rewrites this process.
    pub fn claim_session_attachment(&self) -> StoreResult<()> {
        let lf_process_id = crate::journal::current_lf_process_id().ok_or_else(|| {
            StoreError::InvalidAuthority("AgentProcess requires an admitted invocation".into())
        })?;
        let mut runtime = self.0.lock().expect("Process runtime mutex poisoned");
        if runtime.owner.is_some() {
            return Ok(());
        }
        let store = row_store(&runtime.recorder.dir)?;
        let session = store
            .session_for_artifact(&runtime.recorder.manifest.artifact_key)?
            .ok_or_else(|| {
                StoreError::InvalidAuthority(
                    "AgentProcess requires an admitted conversation".into(),
                )
            })?;
        let attachment = resume_session_agent_process(&store, &session.id, &lf_process_id)?;
        if let Some(terminal) = crate::transport::current() {
            store.bind_session_terminal(&session.id, &attachment, terminal)?;
        }
        runtime.owner = Some(crate::harness::Recording {
            store,
            session: session.id.to_string(),
            attachment,
        });
        drop(runtime);
        self.register_interrupt();
        Ok(())
    }

    fn register_interrupt(&self) {
        let runtime = Arc::downgrade(&self.0);
        crate::agent::register_interrupt_cleanup(move || {
            if let Some(runtime) = runtime.upgrade() {
                let mut runtime = runtime.lock().expect("Process runtime mutex poisoned");
                if runtime.settled_outcome.is_none() {
                    if let Err(error) = runtime.finish("interrupted") {
                        tracing::warn!(%error, "record interrupted Session exit");
                    }
                }
            }
        });
    }

    pub(crate) fn conversation_resume_token(&self) -> StoreResult<Option<AgentSessionId>> {
        let runtime = self.0.lock().expect("Process runtime mutex poisoned");
        if let Some(Recording { store, session, .. }) = &runtime.owner {
            return store.session_thread(session);
        }
        let store = row_store(&runtime.recorder.dir)?;
        let Some(session) = store.session_for_artifact(&runtime.recorder.manifest.artifact_key)?
        else {
            return Ok(None);
        };
        store.session_thread(&session.id)
    }

    pub fn recording(&self) -> Option<Recording> {
        self.0
            .lock()
            .expect("Process runtime mutex poisoned")
            .owner
            .clone()
    }

    pub fn session_attachment(&self) -> Option<(String, crate::process::SessionAttachment)> {
        self.0
            .lock()
            .expect("Process runtime mutex poisoned")
            .owner
            .as_ref()
            .map(
                |Recording {
                     session,
                     attachment,
                     ..
                 }| (session.clone(), attachment.clone()),
            )
    }

    pub fn attachment_replaced(&self) -> StoreResult<bool> {
        let runtime = self.0.lock().expect("Process runtime mutex poisoned");
        let Some(Recording {
            store,
            session,
            attachment: expected,
        }) = &runtime.owner
        else {
            return Ok(false);
        };
        Ok(store.session_attachment(session)?.as_ref() != Some(expected))
    }

    /// Only the invocation's runtime advances its settlement snapshot.
    /// Dispatch and history retain the immutable snapshot they already took.
    pub(crate) fn prepare_agent_process(
        &self,
        session: &str,
        expected: &crate::process::SessionAttachment,
    ) -> StoreResult<crate::process::SessionAttachment> {
        let mut runtime = self.0.lock().expect("Process runtime mutex poisoned");
        let settled = runtime.settled_outcome.is_some();
        let Recording {
            store,
            session: id,
            attachment,
        } = runtime
            .owner
            .as_mut()
            .filter(
                |Recording {
                     session: id,
                     attachment,
                     ..
                 }| !settled && id == session && attachment == expected,
            )
            .ok_or_else(|| StoreError::InvalidAuthority("Process attachment changed".into()))?;
        let next = attachment::prepare_session_agent_process(store, id, attachment)?;
        *attachment = next.clone();
        Ok(next)
    }

    pub fn environment(&self) -> BTreeMap<String, String> {
        let runtime = self.0.lock().expect("Process runtime mutex poisoned");
        let mut environment = BTreeMap::from([(
            CAPTURE_KEY_ENV.to_string(),
            runtime.recorder.manifest.artifact_key.to_string(),
        )]);
        if let Ok(declaration) = std::env::var(crate::ops::WORK_DECLARATION_ENV) {
            environment.insert(crate::ops::WORK_DECLARATION_ENV.to_string(), declaration);
        }
        if let Some(Recording {
            session,
            attachment,
            ..
        }) = &runtime.owner
        {
            environment.insert(
                crate::process::AGENT_CALLER_ENV.into(),
                serde_json::to_string(
                    &attachment.caller(session.clone().into(), crate::journal::current_trace_id()),
                )
                .expect("caller provenance serializes"),
            );
        }
        environment
    }

    pub(crate) fn mark_spawn_requested(&self) {
        self.record_best_effort(RuntimeState::start_attempt);
    }

    pub(crate) fn record_conversation(&self, event: ConversationEvent) {
        self.record_best_effort(|runtime| runtime.recorder.record_conversation(event));
    }

    pub(crate) fn retry_agent_process(
        &self,
        provider: String,
        model: Option<String>,
        resume_thread: Option<&AgentSessionId>,
    ) -> anyhow::Result<()> {
        let mut runtime = self.0.lock().expect("Process runtime mutex poisoned");
        if runtime.settled_outcome.is_some() {
            anyhow::bail!("Process runtime already settled");
        }
        let Recording {
            store,
            session,
            attachment: expected,
        } = runtime.owner.as_ref().ok_or_else(|| {
            StoreError::InvalidAuthority("AgentProcess retry has no attachment".into())
        })?;
        // The provider may have switched threads while history was unavailable.
        // Its latest observation outranks an older persisted projection.
        let saved_thread = match &runtime.agent_session {
            Some(thread) => Some(thread.clone()),
            None => store.session_thread(session)?,
        };
        let moved = runtime.provider != provider;
        if !retry_keeps_its_thread(saved_thread.as_ref(), resume_thread, moved) {
            anyhow::bail!("AgentProcess retry must retain its thread");
        }
        let next = attachment::replace_session_agent_process(
            store,
            session,
            expected,
            resume_thread,
            || close_session_agent_process(store, session),
        )?;
        let recorded = store.record_session_agent(session, &next, &provider, model.as_deref());
        // The old attachment is gone either way; a failed write must not
        // leave the runtime holding it.
        runtime.owner = Some(crate::harness::Recording {
            store: store.clone(),
            session: session.clone(),
            attachment: next,
        });
        recorded?;
        if let Err(error) = runtime.fail_and_begin_attempt(provider, model) {
            runtime.recorder.warn_telemetry(error);
        }
        Ok(())
    }

    pub fn observe_provider(&self, session_id: Option<AgentSessionId>) {
        self.record_best_effort(|execution| execution.observe_provider(session_id));
    }

    pub fn finish(&self, outcome: &str) -> StoreResult<()> {
        self.0
            .lock()
            .expect("Process runtime mutex poisoned")
            .finish(outcome)
            .map_err(record_error)
    }

    pub fn hand_off(&self) -> CaptureHandoff {
        let runtime = self.0.lock().expect("Process runtime mutex poisoned");
        runtime.recorder.hand_off()
    }

    fn record_best_effort(&self, operation: impl FnOnce(&mut RuntimeState) -> std::io::Result<()>) {
        let mut runtime = self.0.lock().expect("Process runtime mutex poisoned");
        if let Err(error) = operation(&mut runtime) {
            runtime.recorder.warn_telemetry(error);
        }
    }
}

impl Drop for RuntimeState {
    fn drop(&mut self) {
        if self.settled_outcome.is_some() {
            return;
        }
        if let Err(error) = self.finish("failed") {
            tracing::warn!(
                %error,
                artifact_key = %self.recorder.manifest.artifact_key,
                "failed to settle dropped Session capture"
            );
        }
    }
}

#[derive(Debug)]
struct RuntimeState {
    owner: Option<Recording>,
    provider: String,
    model: Option<String>,
    agent_session: Option<AgentSessionId>,
    settled_outcome: Option<String>,
    recorder: SessionRecorder,
}

impl RuntimeState {
    fn new(recorder: SessionRecorder) -> Self {
        Self {
            owner: None,
            provider: recorder.manifest.harness.clone(),
            model: recorder.manifest.model.clone(),
            agent_session: None,
            settled_outcome: None,
            recorder,
        }
    }

    fn observe_provider(&mut self, session: Option<AgentSessionId>) -> std::io::Result<()> {
        let thread_changed = session
            .as_ref()
            .is_some_and(|session| self.agent_session.as_ref() != Some(session));
        // Live identity advances even if keeping its history fails.
        if let Some(session) = session {
            self.agent_session = Some(session);
        }
        self.recorder
            .observe_provider(self.agent_session.as_ref(), thread_changed)
    }

    fn start_attempt(&mut self) -> std::io::Result<()> {
        self.recorder
            .start_attempt(&self.provider, self.model.as_deref())
    }

    fn fail_and_begin_attempt(
        &mut self,
        provider: String,
        model: Option<String>,
    ) -> std::io::Result<()> {
        self.provider = provider;
        self.model = model;
        self.agent_session = None;
        self.recorder
            .next_attempt(&self.provider, self.model.as_deref())
    }

    fn finish(&mut self, outcome: &str) -> std::io::Result<()> {
        if !matches!(outcome, "completed" | "failed" | "interrupted") {
            return Err(std::io::Error::other(format!(
                "invalid process outcome: {outcome}"
            )));
        }
        if let Some(settled) = &self.settled_outcome {
            if settled != outcome {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!("Process runtime already settled as {settled}; refusing {outcome}"),
                ));
            }
        } else {
            self.settled_outcome = Some(outcome.to_string());
        }
        // Ending the invocation cannot depend on writing its receipt. A retry
        // may record the same outcome, but cannot reopen or relabel this owner.
        if let Some(Recording {
            store,
            session,
            attachment,
        }) = self.owner.take()
        {
            match finish_session_attachment(&store, &session, &attachment, outcome) {
                Ok(_) | Err(StoreError::InvalidAuthority(_)) => {}
                Err(error) => {
                    tracing::warn!(%error, %session, "cannot record the Session attachment's end")
                }
            }
            crate::transcript::facts::record_session(
                &store,
                &session,
                crate::transcript::facts::Moment::Exit,
            );
        }
        self.recorder.finish(outcome)
    }
}

/// Resume the saved conversation after settling its exact former AgentProcess.
pub fn resume_session_agent_process(
    store: &loopflow_storage::sqlite::SqliteStore,
    session: &str,
    lf_process_id: &loopflow_model::id::LfProcessId,
) -> StoreResult<crate::process::SessionAttachment> {
    attachment::resume_session_attachment(store, session, lf_process_id, || {
        crate::harness::agent_process::close_session_agent_process(store, session)
    })
}

impl LfProcessRuntime {
    pub(crate) async fn observe_activity(&self, body_pid: Option<u32>) {
        let processes = crate::session_record::activity::sample(body_pid).await;
        self.record_best_effort(|runtime| {
            let now = time::OffsetDateTime::now_utc();
            let quiet_since = runtime.recorder.activity.observe(now, body_pid, processes);
            let Some(Recording {
                store,
                session,
                attachment,
            }) = &runtime.owner
            else {
                return Ok(());
            };
            store
                .record_session_cpu(
                    session,
                    attachment,
                    now.unix_timestamp(),
                    quiet_since.map(time::OffsetDateTime::unix_timestamp),
                )
                .map_err(std::io::Error::other)
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::process::attachment;

    use std::io::{BufRead, BufReader};
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    use loopflow_model::id::LfProcessId;
    use loopflow_storage::sqlite::SqliteStore;

    struct Child(std::process::Child);

    impl Drop for Child {
        fn drop(&mut self) {
            if matches!(self.0.try_wait(), Ok(None)) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }

    fn resume_fixture() -> (
        tempfile::TempDir,
        SqliteStore,
        crate::process::SessionAttachment,
    ) {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("resume", &crate::session_record::new_artifact_key());
        let sql = rusqlite::Connection::open(home.path().join("store.db")).unwrap();
        sql.execute(
            "UPDATE agent_sessions SET provider='codex',interactive=0",
            [],
        )
        .unwrap();
        let attached =
            attachment::claim_session_attachment(&store, "resume", None, &store.test_lf_process())
                .unwrap();
        let detached = store
            .release_session_attachment("resume", &attached)
            .unwrap();
        (home, store, detached)
    }

    #[test]
    fn resume_waits_for_a_dead_owners_helpers_without_lifeline_authority() {
        resume_after_leader_exit(true);
    }

    #[test]
    fn resume_refuses_a_dead_owners_persistent_helpers_without_settling() {
        resume_after_leader_exit(false);
    }

    fn resume_after_leader_exit(exits: bool) {
        const RECOVERY_HOME: &str = "LOOPFLOW_TEST_SESSION_RECOVERY";
        if let Some(home) = std::env::var_os(RECOVERY_HOME) {
            let home = std::path::PathBuf::from(home);
            let store = crate::store::open_sqlite(&home.join("store.db")).unwrap();
            std::fs::write(home.join("recovering"), "ready").unwrap();
            let result = crate::process::resume_session_agent_process(
                &store,
                "resume",
                &store.test_lf_process(),
            )
            .map_err(|error| error.to_string());
            std::fs::write(home.join("result"), serde_json::to_vec(&result).unwrap()).unwrap();
            return;
        }
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (home, store, detached) = resume_fixture();
        let task = store
            .create_task(&loopflow_storage::work::task::NewTask {
                id: loopflow_model::durable::TaskId::new(),
                repository_id: crate::repository::location::ensure_repository(
                    &store,
                    home.path().to_str().unwrap(),
                )
                .unwrap(),
                project_id: None,
                title: "Retain Session work".into(),
                description: "Existing work before owner replacement".into(),
                due_date: None,
            })
            .unwrap();
        let runtime = store.session("resume").unwrap().unwrap().captured;
        crate::store::sessions::bind_session(&store, "resume", runtime, &task.id).unwrap();
        let attached = attachment::claim_session_attachment(
            &store,
            "resume",
            Some(&detached),
            &store.test_lf_process(),
        )
        .unwrap();
        // These children have no local lifeline writer, as when a different
        // invocation recovers a Session after its launcher has disappeared.
        let mut command = Command::new("/bin/sleep");
        command.env_clear().arg("60").process_group(0);
        store
            .record_agent_process_launch("resume", &attached, &command, false)
            .unwrap();
        let mut leader = Child(command.spawn().unwrap());
        let pid = leader.0.id();
        let birth = crate::process::os_process::process_started_at(pid)
            .unwrap()
            .unwrap();
        store
            .record_agent_process_identity("resume", &attached, pid, birth)
            .unwrap();
        store
            .record_session_connection(
                "resume",
                &attached,
                home.path().join("absent.sock").to_str().unwrap(),
                &"saved-thread".into(),
            )
            .unwrap();
        let mut helper = Child(
            Command::new("/bin/sh")
                .env_clear()
                .args([
                    "-c",
                    "printf 'ready\\n'; read -r done; printf 'natural exit\\n'",
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .process_group(pid as i32)
                .spawn()
                .unwrap(),
        );
        let mut output = BufReader::new(helper.0.stdout.take().unwrap());
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        assert_eq!(line, "ready\n");
        leader.0.kill().unwrap();
        leader.0.wait().unwrap();
        let detached = store
            .release_session_attachment("resume", &attached)
            .unwrap();
        store
            .save_session_steer("resume", "Test author", "Keep this direction")
            .unwrap();
        let pending =
            serde_json::to_value(store.session_steers("resume", 0, 100).unwrap()).unwrap();
        let before = store.session("resume").unwrap().unwrap();
        let history = store.session_history("resume", 0, 0).unwrap();
        let agent = store
            .agent_process(&attached.agent_process_id)
            .unwrap()
            .unwrap();
        assert!(!crate::process::OsProcess::group(pid).unwrap().is_empty());
        let test = if exits {
            "resume_waits_for_a_dead_owners_helpers_without_lifeline_authority"
        } else {
            "resume_refuses_a_dead_owners_persistent_helpers_without_settling"
        };
        let mut recovery = Child(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", &format!("process::runtime::tests::{test}")])
                .env(RECOVERY_HOME, home.path())
                .stdin(Stdio::null())
                .spawn()
                .unwrap(),
        );
        if exits {
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            while !home.path().join("recovering").exists() {
                assert!(recovery.0.try_wait().unwrap().is_none(), "recovery exited");
                assert!(
                    std::time::Instant::now() < deadline,
                    "recovery did not start"
                );
                std::thread::sleep(Duration::from_millis(25));
            }
            // Keep a survivor alive into the recovery attempt, then let it exit.
            std::thread::sleep(Duration::from_millis(500));
            drop(helper.0.stdin.take());
        }
        assert!(recovery.0.wait().unwrap().success());
        let result: Result<crate::process::SessionAttachment, String> =
            serde_json::from_slice(&std::fs::read(home.path().join("result")).unwrap()).unwrap();
        if exits {
            let next = result.unwrap();
            helper.0.wait().unwrap();
            line.clear();
            output.read_line(&mut line).unwrap();
            assert_eq!(line, "natural exit\n", "recovery must not signal helpers");
            assert_ne!(next.agent_process_id, attached.agent_process_id);
            assert_ne!(next.token, detached.token);
            assert!(store
                .agent_process(&attached.agent_process_id)
                .unwrap()
                .unwrap()
                .completed_at
                .is_some());
            assert!(
                super::finish_session_attachment(&store, "resume", &attached, "interrupted")
                    .is_err()
            );
            assert_eq!(store.session_attachment("resume").unwrap(), Some(next));
        } else {
            let error = result.unwrap_err();
            assert!(
                error.contains(&format!("pid {} (", helper.0.id())),
                "{error}"
            );
            assert!(helper.0.try_wait().unwrap().is_none());
            assert_eq!(store.session_attachment("resume").unwrap(), Some(detached));
            assert_eq!(
                store.agent_process(&attached.agent_process_id).unwrap(),
                Some(agent)
            );
        }
        assert_eq!(store.session("resume").unwrap(), Some(before));
        assert_eq!(store.task(&task.id).unwrap(), Some(task));
        assert_eq!(
            store.session_thread("resume").unwrap(),
            Some("saved-thread".into())
        );
        assert_eq!(
            serde_json::to_value(store.session_steers("resume", 0, 100).unwrap()).unwrap(),
            pending
        );
        assert_eq!(store.session_history("resume", 0, 0).unwrap(), history);
    }

    #[test]
    fn native_resume_requires_observed_death_and_preserves_history() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (home, store, detached) = resume_fixture();
        let attached = attachment::claim_session_attachment(
            &store,
            "resume",
            Some(&detached),
            &LfProcessId::new(),
        )
        .unwrap();
        let mut command = std::process::Command::new("/bin/sleep");
        command.env_clear().arg("60");
        let mut child = Child(
            crate::harness::agent_process::spawn_native(
                command,
                &crate::harness::Recording {
                    store: store.clone(),
                    session: "resume".into(),
                    attachment: attached.clone(),
                },
                crate::harness::LaunchKind::Batch,
            )
            .unwrap(),
        );
        let detached = store
            .release_session_attachment("resume", &attached)
            .unwrap();
        let sql = rusqlite::Connection::open(home.path().join("store.db")).unwrap();
        sql.execute(
            "UPDATE agent_sessions SET provider_thread='saved-native' WHERE id='resume'",
            [],
        )
        .unwrap();
        let before = store
            .agent_process(&attached.agent_process_id)
            .unwrap()
            .unwrap();

        // A live foreground provider must not be stopped or launched twice.
        assert!(crate::process::resume_session_agent_process(
            &store,
            "resume",
            &store.test_lf_process(),
        )
        .is_err());
        assert!(child.0.try_wait().unwrap().is_none());
        assert_eq!(
            store.agent_process(&attached.agent_process_id).unwrap(),
            Some(before.clone())
        );
        assert_eq!(
            store.session_attachment("resume").unwrap(),
            Some(detached.clone())
        );

        child.0.kill().unwrap();
        child.0.wait().unwrap();
        // Missing identity still grants no launch, even after this child died.
        sql.execute(
            "UPDATE processes SET pid=NULL WHERE id=?1",
            [&attached.agent_process_id],
        )
        .unwrap();
        assert!(crate::process::resume_session_agent_process(
            &store,
            "resume",
            &store.test_lf_process(),
        )
        .is_err());
        sql.execute(
            "UPDATE processes SET pid=?2 WHERE id=?1",
            rusqlite::params![attached.agent_process_id, before.pid],
        )
        .unwrap();

        let next = crate::process::resume_session_agent_process(
            &store,
            "resume",
            &store.test_lf_process(),
        )
        .unwrap();
        assert_ne!(next.agent_process_id, attached.agent_process_id);
        assert_eq!(
            store.session_thread("resume").unwrap(),
            Some("saved-native".into())
        );
        let ended = store
            .agent_process(&attached.agent_process_id)
            .unwrap()
            .unwrap();
        assert_eq!(ended.pid, before.pid);
        assert_eq!(ended.os_started_at, before.os_started_at);
        assert!(ended.completed_at.is_some());
    }

    #[test]
    fn resume_detached_live_agent_uses_recorded_settings_and_preserves_history() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (home, store, detached) = resume_fixture();
        let attached = attachment::claim_session_attachment(
            &store,
            "resume",
            Some(&detached),
            &store.test_lf_process(),
        )
        .unwrap();
        let mut command = std::process::Command::new("/bin/sleep");
        command.env_clear().arg("60").process_group(0);
        let child = Child(command.spawn().unwrap());
        let pid = child.0.id();
        let birth = crate::process::os_process::process_started_at(pid)
            .unwrap()
            .unwrap();
        store
            .record_agent_process_identity("resume", &attached, pid, birth)
            .unwrap();
        store
            .record_session_connection(
                "resume",
                &attached,
                home.path().join("absent.sock").to_str().unwrap(),
                &"saved-thread".into(),
            )
            .unwrap();
        store
            .release_session_attachment("resume", &attached)
            .unwrap();
        rusqlite::Connection::open(home.path().join("store.db"))
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET provider='claude',interactive=1",
                [],
            )
            .unwrap();
        let next = crate::process::resume_session_agent_process(
            &store,
            "resume",
            &store.test_lf_process(),
        )
        .unwrap();
        assert_ne!(next.agent_process_id, attached.agent_process_id);
        assert_eq!(
            crate::process::os_process::process_identity_evidence(pid, birth),
            crate::store::ProcessIdentityEvidence::Dead
        );
        let ended = store
            .agent_process(&attached.agent_process_id)
            .unwrap()
            .unwrap();
        assert!(ended.completed_at.is_some());
        assert_eq!(ended.pid, Some(pid));
        assert_eq!(
            store.session_thread("resume").unwrap(),
            Some("saved-thread".into())
        );
        assert!(attachment::session_connection(&store, "resume")
            .unwrap()
            .is_none());
    }

    #[test]
    fn resume_unknown_owner_cannot_close_or_claim_an_unlaunched_agent() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (_home, store, detached) = resume_fixture();
        let owner = store.test_lf_process();
        // The fixture's epoch start would prove death before this boot.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE processes SET started_at=?2 WHERE id=?1",
                rusqlite::params![owner, time::OffsetDateTime::now_utc().unix_timestamp()],
            )
            .unwrap();
        let attached =
            attachment::claim_session_attachment(&store, "resume", Some(&detached), &owner)
                .unwrap();
        let session = store.session("resume").unwrap().unwrap();
        let agent = store.agent_process(&attached.agent_process_id).unwrap();
        let history = store.session_history("resume", 0, 0).unwrap();
        let resumer = store.test_lf_process();
        assert!(
            attachment::resume_session_attachment(&store, "resume", &resumer, || {
                panic!("unknown owner cannot authorize closing its agent")
            })
            .is_err()
        );
        let mut next = session.clone();
        next.artifact_key = Some(crate::session_record::new_artifact_key());
        next.input_published = false;
        assert!(
            attachment::claim_session_input(&store, next, Some(&attached), &resumer, || {
                panic!("unknown owner cannot authorize replacing its input")
            })
            .is_err()
        );
        assert_eq!(store.session_attachment("resume").unwrap(), Some(attached));
        assert_eq!(
            store.agent_process(&agent.as_ref().unwrap().id).unwrap(),
            agent
        );
        assert_eq!(store.session("resume").unwrap(), Some(session));
        assert_eq!(store.session_history("resume", 0, 0).unwrap(), history);
    }

    #[test]
    fn resume_unknown_spawn_preserves_record_attachment_and_input() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (_home, store, detached) = resume_fixture();
        let attached = attachment::claim_session_attachment(
            &store,
            "resume",
            Some(&detached),
            &LfProcessId::new(),
        )
        .unwrap();
        store
            .record_agent_process_launch(
                "resume",
                &attached,
                &std::process::Command::new("fixture"),
                false,
            )
            .unwrap();
        let detached = store
            .release_session_attachment("resume", &attached)
            .unwrap();
        let before = store.agent_process(&detached.agent_process_id).unwrap();
        let session = store.session("resume").unwrap().unwrap();
        let mut next = session.clone();
        next.artifact_key = Some(crate::session_record::new_artifact_key());
        next.input_published = false;
        assert!(attachment::claim_session_input(
            &store,
            next,
            Some(&detached),
            &store.test_lf_process(),
            || { super::close_session_agent_process(&store, "resume") }
        )
        .is_err());
        assert!(crate::process::resume_session_agent_process(
            &store,
            "resume",
            &store.test_lf_process()
        )
        .is_err());
        assert_eq!(
            store.agent_process(&detached.agent_process_id).unwrap(),
            before
        );
        assert_eq!(store.session_attachment("resume").unwrap(), Some(detached));
        assert_eq!(store.session("resume").unwrap(), Some(session));
    }

    #[test]
    fn resume_duplicate_live_identity_cannot_signal_either_session() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (home, store, detached) = resume_fixture();
        let attached = attachment::claim_session_attachment(
            &store,
            "resume",
            Some(&detached),
            &store.test_lf_process(),
        )
        .unwrap();
        let mut command = std::process::Command::new("/bin/sleep");
        command.env_clear().arg("60").process_group(0);
        let mut child = Child(command.spawn().unwrap());
        let pid = child.0.id();
        let birth = crate::process::os_process::process_started_at(pid)
            .unwrap()
            .unwrap();
        store
            .record_agent_process_identity("resume", &attached, pid, birth)
            .unwrap();
        store
            .record_session_connection(
                "resume",
                &attached,
                home.path().join("absent.sock").to_str().unwrap(),
                &"saved".into(),
            )
            .unwrap();
        let detached = store
            .release_session_attachment("resume", &attached)
            .unwrap();
        store.test_session("duplicate", &crate::session_record::new_artifact_key());
        let duplicate = attachment::claim_session_attachment(
            &store,
            "duplicate",
            None,
            &store.test_lf_process(),
        )
        .unwrap();
        store
            .record_agent_process_identity("duplicate", &duplicate, pid, birth)
            .unwrap();
        let error = crate::process::resume_session_agent_process(
            &store,
            "resume",
            &store.test_lf_process(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("multiple owners"), "{error}");
        assert!(child.0.try_wait().unwrap().is_none());
        assert_eq!(store.session_attachment("resume").unwrap(), Some(detached));
        assert_eq!(store.agent_processes().unwrap().len(), 2);
    }

    #[test]
    fn resume_zombie_is_dead_without_signaling_or_inventing_success() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (_home, store, detached) = resume_fixture();
        let attached = attachment::claim_session_attachment(
            &store,
            "resume",
            Some(&detached),
            &LfProcessId::new(),
        )
        .unwrap();
        let mut child = Child(
            std::process::Command::new("/bin/sleep")
                .env_clear()
                .arg("60")
                .spawn()
                .unwrap(),
        );
        let pid = child.0.id();
        let birth = crate::process::os_process::process_started_at(pid)
            .unwrap()
            .unwrap();
        store
            .record_agent_process_identity("resume", &attached, pid, birth)
            .unwrap();
        store
            .release_session_attachment("resume", &attached)
            .unwrap();
        child.0.kill().unwrap();
        // Observe this fixture's zombie without reaping it through Child::wait.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while crate::process::os_process::process_identity_evidence(pid, birth)
            != crate::store::ProcessIdentityEvidence::Dead
        {
            assert!(
                std::time::Instant::now() < deadline,
                "fixture child did not exit"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let next = crate::process::resume_session_agent_process(
            &store,
            "resume",
            &store.test_lf_process(),
        )
        .unwrap();
        assert_ne!(next.agent_process_id, attached.agent_process_id);
        let ended = store
            .agent_process(&attached.agent_process_id)
            .unwrap()
            .unwrap();
        assert!(ended.completed_at.is_some());
    }

    #[test]
    fn resume_fences_takeover_until_settlement_and_claim_commit() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (_home, store, detached) = resume_fixture();
        let attached = attachment::claim_session_attachment(
            &store,
            "resume",
            Some(&detached),
            &LfProcessId::new(),
        )
        .unwrap();
        store
            .record_agent_process_launch(
                "resume",
                &attached,
                &std::process::Command::new("fixture"),
                false,
            )
            .unwrap();
        let detached = store
            .release_session_attachment("resume", &attached)
            .unwrap();
        let (started, starting) = std::sync::mpsc::channel();
        let (finished, finishing) = std::sync::mpsc::channel();
        let resumer = store.test_lf_process();
        std::thread::scope(|scope| {
            let next = attachment::resume_session_attachment(&store, "resume", &resumer, || {
                let store = &store;
                let expected = &detached;
                scope.spawn(move || {
                    started.send(()).unwrap();
                    let result = attachment::claim_session_attachment(
                        store,
                        "resume",
                        Some(expected),
                        &LfProcessId::new(),
                    );
                    finished.send(result).unwrap();
                });
                starting.recv().unwrap();
                assert!(matches!(
                    finishing.recv_timeout(std::time::Duration::from_millis(100)),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                ));
                // Other SQLite reads/writes remain available during provider I/O.
                assert_eq!(
                    store.session_attachment("resume").unwrap(),
                    Some(detached.clone())
                );
                Ok(true)
            })
            .unwrap();
            assert!(finishing
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .is_err());
            assert_eq!(
                store.session_attachment("resume").unwrap(),
                Some(next.clone())
            );
            assert_ne!(next.agent_process_id, detached.agent_process_id);
            assert!(store
                .agent_process(&detached.agent_process_id)
                .unwrap()
                .unwrap()
                .completed_at
                .is_some());
        });
    }

    #[test]
    fn headless_close_ends_the_attached_agent_for_every_provider() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        for provider in ["claude", "opencode", "codex"] {
            let (home, store, detached) = resume_fixture();
            rusqlite::Connection::open(home.path().join("store.db"))
                .unwrap()
                .execute("UPDATE agent_sessions SET provider=?1", [provider])
                .unwrap();
            let launch = LfProcessId::new();
            rusqlite::Connection::open(home.path().join("store.db"))
                .unwrap()
                .execute(
                    "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
                    [&launch],
                )
                .unwrap();
            // A fresh AgentProcess records the provider at launch, not the
            // Session's mutable next-launch settings.
            let first =
                attachment::claim_session_attachment(&store, "resume", Some(&detached), &launch)
                    .unwrap();
            let mut command = std::process::Command::new("/bin/sleep");
            command.env_clear().arg("60").process_group(0);
            store
                .record_agent_process_launch("resume", &first, &command, false)
                .unwrap();
            let child = Child(command.spawn().unwrap());
            let pid = child.0.id();
            let birth = crate::process::os_process::process_started_at(pid)
                .unwrap()
                .unwrap();
            store
                .record_agent_process_identity("resume", &first, pid, birth)
                .unwrap();
            if provider == "codex" {
                store
                    .record_session_connection(
                        "resume",
                        &first,
                        home.path().join("absent.sock").to_str().unwrap(),
                        &"saved-thread".into(),
                    )
                    .unwrap();
            }
            super::finish_session_attachment(&store, "resume", &first, "interrupted").unwrap();
            assert_eq!(
                crate::process::os_process::process_identity_evidence(pid, birth),
                crate::store::ProcessIdentityEvidence::Dead
            );
            assert!(store
                .agent_process(&first.agent_process_id)
                .unwrap()
                .unwrap()
                .completed_at
                .is_some());
        }
    }

    #[test]
    fn foreground_agent_is_observed_dead_without_headless_group_control() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        for provider in ["claude", "opencode", "codex"] {
            let (home, store, detached) = resume_fixture();
            rusqlite::Connection::open(home.path().join("store.db"))
                .unwrap()
                .execute(
                    "UPDATE agent_sessions SET provider=?1, interactive=1",
                    [provider],
                )
                .unwrap();
            let attachment = attachment::claim_session_attachment(
                &store,
                "resume",
                Some(&detached),
                &store.test_lf_process(),
            )
            .unwrap();
            let mut command = std::process::Command::new("/bin/sleep");
            command.env_clear().arg("60").process_group(0);
            store
                .record_agent_process_launch("resume", &attachment, &command, true)
                .unwrap();
            let mut child = Child(command.spawn().unwrap());
            let pid = child.0.id();
            let birth = crate::process::os_process::process_started_at(pid)
                .unwrap()
                .unwrap();
            store
                .record_agent_process_identity("resume", &attachment, pid, birth)
                .unwrap();
            assert!(!store
                .with_session_attachment("resume", &attachment, || {
                    super::close_session_agent_process(&store, "resume")
                })
                .unwrap());
            assert_eq!(
                crate::process::os_process::process_identity_evidence(pid, birth),
                crate::store::ProcessIdentityEvidence::Live
            );
            assert_eq!(
                store.session_attachment("resume").unwrap(),
                Some(attachment.clone())
            );
            child.0.kill().unwrap();
            child.0.wait().unwrap();
            let before = store.agent_process(&attachment.agent_process_id).unwrap();
            assert!(store
                .with_session_attachment("resume", &attachment, || {
                    super::close_session_agent_process(&store, "resume")
                })
                .unwrap());
            assert_eq!(
                store.agent_process(&attachment.agent_process_id).unwrap(),
                before
            );
            assert_eq!(
                store.session_attachment("resume").unwrap(),
                Some(attachment)
            );
        }
    }

    #[test]
    fn invocation_replacement_ends_only_its_exact_throwaway_group() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("store.db");
        let store = crate::store::opening::open_ephemeral(&database).unwrap();
        store.test_session("retry", "run_00000000000000000000000000000001");
        let sql = rusqlite::Connection::open(&database).unwrap();
        sql.execute(
            "UPDATE agent_sessions SET provider='codex',interactive=0 WHERE id='retry'",
            [],
        )
        .unwrap();
        let parent = LfProcessId::new();
        sql.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
            [&parent],
        )
        .unwrap();
        let attachment =
            attachment::claim_session_attachment(&store, "retry", None, &parent).unwrap();
        let mut command = std::process::Command::new("/bin/sleep");
        command.env_clear().arg("60").process_group(0);
        store
            .record_agent_process_launch("retry", &attachment, &command, false)
            .unwrap();
        let child = Child(command.spawn().unwrap());
        let pid = child.0.id();
        let birth = crate::process::os_process::process_started_at(pid)
            .unwrap()
            .unwrap();
        store
            .record_agent_process_identity("retry", &attachment, pid, birth)
            .unwrap();
        store
            .record_session_connection(
                "retry",
                &attachment,
                home.path().join("absent.sock").to_str().unwrap(),
                &"retained-thread".into(),
            )
            .unwrap();
        // Later launch settings do not change which kind of process was launched.
        sql.execute(
            "UPDATE agent_sessions SET provider='claude' WHERE id='retry'",
            [],
        )
        .unwrap();
        let next = attachment::replace_session_agent_process(
            &store,
            "retry",
            &attachment,
            Some(&"retained-thread".into()),
            || super::close_session_agent_process(&store, "retry"),
        )
        .unwrap();
        assert_ne!(next.agent_process_id, attachment.agent_process_id);
        assert_eq!(
            crate::process::os_process::process_identity_evidence(pid, birth),
            crate::store::ProcessIdentityEvidence::Dead
        );
        assert!(store
            .agent_process(&attachment.agent_process_id)
            .unwrap()
            .unwrap()
            .completed_at
            .is_some());
        assert!(store
            .agent_process(&next.agent_process_id)
            .unwrap()
            .unwrap()
            .pid
            .is_none());
        assert_eq!(
            store.session_thread("retry").unwrap(),
            Some("retained-thread".into())
        );
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod state_tests;
