//! Shared execution state for launch, retry and settlement. Only its current
//! attachment may close an AgentProcess; a replaced invocation cannot stop
//! the new owner's provider.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    boot_id, inherited_capture_key, new_artifact_key, prepare_manifest, publish_manifest,
    read_manifest, reconcile_private_file, reconcile_reserved_manifest, record_dir, record_error,
    resolve_manifest, row_store, runtime_identity, sync_dir, validate_manifest_path,
    write_private_exclusive, AgentProcessRequest, SessionCaptureManifest, SessionCaptureSpec,
    SessionContextArtifact, SessionContextRef, SessionGraphMembership, SessionRecorder,
    CAPTURE_KEY_ENV, SCHEMA_VERSION,
};
use crate::chat::types::ConversationEvent;
use crate::harness::agent_process::{close_session_agent_process, AttachmentOwner};
use crate::id::AgentSessionId;
use crate::naming::generated_session_title;
use crate::process::SessionAttachment;
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};

pub(crate) fn finish_session_attachment(
    store: &SqliteStore,
    session: &str,
    attachment: &SessionAttachment,
    outcome: &str,
) -> StoreResult<()> {
    store.finish_session_attachment(session, attachment, outcome, None, || {
        close_session_agent_process(store, session)
    })
}

/// One invocation's provider lifecycle and recording. Clones share attachment
/// changes and settlement; harnesses retain immutable snapshots for dispatch.
#[derive(Debug, Clone)]
pub struct AgentExecution(Arc<Mutex<ExecutionState>>);

/// A capture's state as it crosses from an exec'd launcher to the execution
/// owner that records its outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CaptureHandoff {
    pub key: String,
    attempt: u32,
    attempt_started: bool,
    event_seq: u64,
    account_id: Option<crate::store::ProviderAccountId>,
    account_observed: bool,
    agent_session: Option<AgentSessionId>,
}

pub(crate) fn register_session_attachment_interrupt(
    store: &crate::store::sqlite::SqliteStore,
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
/// another account or provider may leave it behind, and only to start fresh.
fn retry_keeps_its_thread(
    saved: Option<&AgentSessionId>,
    resume: Option<&AgentSessionId>,
    moved: bool,
) -> bool {
    saved == resume || (resume.is_none() && moved)
}

impl AgentExecution {
    /// Publish identity before a human boundary becomes visible. No provider or
    /// terminal receipt exists until this prepared Session executes.
    pub(crate) fn prepare(
        spec: SessionCaptureSpec,
        caller: Option<String>,
        id: String,
    ) -> StoreResult<String> {
        #[cfg(test)]
        let home = std::env::var_os("LF_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!("loopflow-test-run-home-{}", std::process::id()))
            });
        #[cfg(not(test))]
        let home = crate::store::lf_home_dir();
        Self::prepare_at_with_key(&home, spec, caller, id)
    }

    #[cfg(test)]
    pub(crate) fn prepare_at(
        home: &Path,
        spec: SessionCaptureSpec,
        caller: Option<String>,
    ) -> StoreResult<String> {
        Self::prepare_at_with_key(home, spec, caller, new_artifact_key())
    }

    fn prepare_at_with_key(
        home: &Path,
        spec: SessionCaptureSpec,
        caller: Option<String>,
        id: String,
    ) -> StoreResult<String> {
        let (manifest, _) =
            prepare_manifest(spec, id.clone(), caller, None, None).map_err(record_error)?;
        let (_, dir) = reconcile_reserved_manifest(home, manifest, None).map_err(record_error)?;
        if dir.join("launching").try_exists().map_err(record_error)? {
            return Err(record_error(std::io::Error::other(
                "prepared input was already claimed",
            )));
        }
        reconcile_private_file(&dir.join("prepared"), b"").map_err(record_error)?;
        sync_dir(&dir).map_err(record_error)?;
        // Preparation creates no execution owner: dropping it must not settle a
        // Session whose provider has never been launched.
        Ok(id)
    }

    pub(crate) fn start_prepared(
        home: &Path,
        id: &str,
        spec: SessionCaptureSpec,
        context: &crate::trace::PreparedTurnContext,
        request: Option<AgentProcessRequest>,
    ) -> StoreResult<Self> {
        let (dir, mut manifest) = resolve_manifest(home, id).map_err(record_error)?;
        // Atomically claim this preparation. A second launcher cannot record
        // another provider attempt into the same capture.
        fs::rename(dir.join("prepared"), dir.join("launching")).map_err(record_error)?;
        let bytes = serde_json::to_vec_pretty(&SessionContextArtifact {
            schema_version: SCHEMA_VERSION,
            context,
        })?;
        write_private_exclusive(&dir.join("context.json"), &bytes).map_err(record_error)?;
        manifest.process = request;
        manifest.context = Some(SessionContextRef {
            path: "context.json".to_string(),
            content_sha256: hex::encode(Sha256::digest(&bytes)),
            bytes: bytes.len() as u64,
        });
        // Finalize process provenance on the existing identity. Preparation did
        // not freeze a prompt, runtime, or model selection before process.
        manifest.harness = spec.harness;
        manifest.model = spec.model;
        manifest.surface = spec.surface;
        manifest.cwd = spec.cwd;
        manifest.repo = spec.repo;
        manifest.worktree = spec.worktree;
        manifest.skill = spec.skill;
        // Preparation owns Work attribution and a human Graph step's membership;
        // the child's Task prompt context cannot reassign that captured input.
        manifest.graph = manifest.graph.or(Some(spec.graph));
        (manifest.runtime_path, manifest.runtime_digest) = runtime_identity();
        manifest.host = gethostname::gethostname().to_string_lossy().into_owned();
        manifest.boot_id = boot_id();
        let staged = dir.join(".manifest-launching.json");
        write_private_exclusive(&staged, &serde_json::to_vec_pretty(&manifest)?)
            .map_err(record_error)?;
        fs::rename(staged, dir.join("manifest.json")).map_err(record_error)?;
        fs::remove_file(dir.join("launching")).map_err(record_error)?;
        sync_dir(&dir).map_err(record_error)?;
        Ok(Self(Arc::new(Mutex::new(ExecutionState::from_manifest(
            manifest, dir,
        )))))
    }

    /// Append an input only after the same transaction admits its attachment.
    pub(crate) fn continue_with_context(
        session: &str,
        mut spec: SessionCaptureSpec,
        context: &crate::trace::PreparedTurnContext,
        process: AgentProcessRequest,
    ) -> StoreResult<Self> {
        let home = crate::store::lf_home_dir();
        let store = crate::store::sqlite::SqliteStore::new(
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
        let (next, attachment) =
            store.claim_session_input(next, expected.as_ref(), &lf_process_id, || {
                close_session_agent_process(&store, session)
            })?;
        // Continue the Session's attribution; the manifest still describes the
        // actual process cwd supplied by the caller.
        spec.subjects = crate::ops::human_session::capture_subjects(&next);
        let result = Self::begin_reserved_at(
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
        match result {
            Ok(capture) => {
                capture
                    .0
                    .lock()
                    .expect("Agent execution mutex poisoned")
                    .owner = Some((store.clone(), session.to_string(), attachment));
                capture.register_interrupt();
                Ok(capture)
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

    pub(crate) fn begin_with_request(
        spec: SessionCaptureSpec,
        process: AgentProcessRequest,
    ) -> StoreResult<Self> {
        let context = crate::trace::PreparedTurnContext::from_prompts(
            &process.system_prompt,
            &process.task_prompt,
        );
        Self::begin_with_context(spec, &context, Some(process))
    }

    pub(crate) fn begin_with_context(
        spec: SessionCaptureSpec,
        context: &crate::trace::PreparedTurnContext,
        process: Option<AgentProcessRequest>,
    ) -> StoreResult<Self> {
        #[cfg(test)]
        let home = std::env::var_os("LF_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!("loopflow-test-run-home-{}", std::process::id()))
            });
        #[cfg(not(test))]
        let home = crate::store::lf_home_dir();
        Self::begin_at_with_id(
            &home,
            spec,
            new_artifact_key(),
            inherited_capture_key()?,
            process,
            Some(context),
        )
    }

    fn begin_reserved_at(
        home: &Path,
        spec: SessionCaptureSpec,
        artifact_key: String,
        caller: Option<String>,
        process: Option<AgentProcessRequest>,
        context: &crate::trace::PreparedTurnContext,
        publish: impl FnOnce(&String) -> StoreResult<()>,
    ) -> StoreResult<Self> {
        let (manifest, context) =
            prepare_manifest(spec, artifact_key, caller, process, Some(context))
                .map_err(record_error)?;
        let (manifest, dir) = reconcile_reserved_manifest(home, manifest, context.as_deref())
            .map_err(record_error)?;
        // Only the reservation transaction grants process authority. A rejected
        // publication must not start a recorder or settle somebody else's capture.
        publish(&manifest.artifact_key)?;
        Ok(Self(Arc::new(Mutex::new(ExecutionState::from_manifest(
            manifest, dir,
        )))))
    }

    /// Retain the source key already resolved by the replay reader.
    pub(crate) fn begin_replay_at(
        lf_home: &Path,
        spec: SessionCaptureSpec,
        process: AgentProcessRequest,
        caller_artifact_key: String,
    ) -> StoreResult<Self> {
        let context = crate::trace::PreparedTurnContext::from_prompts(
            &process.system_prompt,
            &process.task_prompt,
        );
        Self::begin_at_with_id(
            lf_home,
            spec,
            new_artifact_key(),
            Some(caller_artifact_key),
            Some(process),
            Some(&context),
        )
    }

    #[cfg(test)]
    pub(crate) fn begin_at(lf_home: &Path, spec: SessionCaptureSpec) -> StoreResult<Self> {
        Self::begin_at_with_id(
            lf_home,
            spec,
            new_artifact_key(),
            inherited_capture_key()?,
            None,
            None,
        )
    }

    #[cfg(test)]
    fn begin_at_with_request(
        lf_home: &Path,
        spec: SessionCaptureSpec,
        process: AgentProcessRequest,
    ) -> StoreResult<Self> {
        let context = crate::trace::PreparedTurnContext::from_prompts(
            &process.system_prompt,
            &process.task_prompt,
        );
        Self::begin_at_with_id(
            lf_home,
            spec,
            new_artifact_key(),
            inherited_capture_key()?,
            Some(process),
            Some(&context),
        )
    }

    fn begin_at_with_id(
        lf_home: &Path,
        spec: SessionCaptureSpec,
        artifact_key: String,
        caller_artifact_key: Option<String>,
        process: Option<AgentProcessRequest>,
        context: Option<&crate::trace::PreparedTurnContext>,
    ) -> StoreResult<Self> {
        let work = spec.work.clone();
        let (manifest, context_bytes) =
            prepare_manifest(spec, artifact_key, caller_artifact_key, process, context)
                .map_err(record_error)?;
        let dir = record_dir(lf_home, &manifest.artifact_key).expect("artifact key is a UUID");
        let reserved = ExecutionState::record_row(&manifest, &dir, work, context)?;
        publish_manifest(lf_home, &manifest, context_bytes.as_deref()).map_err(record_error)?;
        if let Some(session) = reserved {
            row_store(&dir)?.publish_capture(&session.id, session.captured)?;
        }
        Ok(Self(Arc::new(Mutex::new(ExecutionState::from_manifest(
            manifest, dir,
        )))))
    }

    pub(crate) fn artifact_key(&self) -> String {
        self.0
            .lock()
            .expect("Agent execution mutex poisoned")
            .recorder
            .manifest
            .artifact_key
            .clone()
    }

    pub(crate) fn artifact_dir(&self) -> PathBuf {
        self.0
            .lock()
            .expect("Agent execution mutex poisoned")
            .recorder
            .dir
            .clone()
    }

    /// Claim an admitted conversation and retain the exact provider provenance
    /// used by its tools. A later attachment transfer never rewrites this process.
    pub(crate) fn claim_session_attachment(&self) -> StoreResult<()> {
        let lf_process_id = crate::journal::current_lf_process_id().ok_or_else(|| {
            StoreError::InvalidAuthority("AgentProcess requires an admitted invocation".into())
        })?;
        let mut capture = self.0.lock().expect("Agent execution mutex poisoned");
        if capture.owner.is_some() {
            return Ok(());
        }
        let store = row_store(&capture.recorder.dir)?;
        let session = store
            .session_for_artifact(&capture.recorder.manifest.artifact_key)?
            .ok_or_else(|| {
                StoreError::InvalidAuthority(
                    "AgentProcess requires an admitted conversation".into(),
                )
            })?;
        let attachment = resume_session_agent_process(&store, &session.id, &lf_process_id)?;
        if let Some(owner) = crate::transport::current() {
            store.bind_session_transport(&session.id, &attachment, owner)?;
        }
        capture.owner = Some((store, session.id.to_string(), attachment));
        drop(capture);
        self.register_interrupt();
        Ok(())
    }

    fn register_interrupt(&self) {
        let capture = Arc::downgrade(&self.0);
        crate::agent::register_interrupt_cleanup(move || {
            if let Some(capture) = capture.upgrade() {
                let mut capture = capture.lock().expect("Agent execution mutex poisoned");
                if capture.settled_outcome.is_none() {
                    if let Err(error) = capture.finish("interrupted") {
                        tracing::warn!(%error, "record interrupted Session exit");
                    }
                }
            }
        });
    }

    pub(crate) fn conversation_resume_token(&self) -> StoreResult<Option<AgentSessionId>> {
        let capture = self.0.lock().expect("Agent execution mutex poisoned");
        if let Some((store, session, _)) = &capture.owner {
            return store.session_thread(session);
        }
        let store = row_store(&capture.recorder.dir)?;
        let Some(session) = store.session_for_artifact(&capture.recorder.manifest.artifact_key)?
        else {
            return Ok(None);
        };
        store.session_thread(&session.id)
    }

    pub(crate) fn spawn_native_agent(
        &self,
        mut command: std::process::Command,
    ) -> anyhow::Result<(std::process::Child, crate::process::SessionAttachment)> {
        let mut capture = self.0.lock().expect("Agent execution mutex poisoned");
        if capture.settled_outcome.is_some() {
            return Err(StoreError::InvalidAuthority("Capture already settled".into()).into());
        }
        let owner = capture.owner.as_mut().ok_or_else(|| {
            StoreError::InvalidAuthority("AgentProcess launch has no attachment".into())
        })?;
        let (store, session, attachment) = owner;
        *attachment = store.prepare_session_agent_process(session, attachment)?;
        command.env(
            crate::process::AGENT_CALLER_ENV,
            serde_json::to_string(&attachment.caller(session.clone().into()))?,
        );
        let snapshot = attachment.clone();
        // Keep the execution's settlement snapshot fixed through admission.
        let child = crate::harness::agent_process::spawn_native(command, owner)?;
        Ok((child, snapshot))
    }

    pub(crate) fn record_native_agent_exit(
        &self,
        expected: &crate::process::SessionAttachment,
    ) -> StoreResult<()> {
        let capture = self.0.lock().expect("Agent execution mutex poisoned");
        let Some((store, session, attachment)) = &capture.owner else {
            return Err(StoreError::InvalidAuthority(
                "Capture has no attachment".into(),
            ));
        };
        if attachment != expected {
            return Err(StoreError::InvalidAuthority(
                "Capture attachment changed".into(),
            ));
        }
        store.record_agent_process_exit(session, expected, true)
    }

    pub(crate) fn session_attachment(&self) -> Option<(String, crate::process::SessionAttachment)> {
        self.0
            .lock()
            .expect("Agent execution mutex poisoned")
            .owner
            .as_ref()
            .map(|(_, session, attachment)| (session.clone(), attachment.clone()))
    }

    pub(crate) fn attachment_replaced(&self) -> StoreResult<bool> {
        let capture = self.0.lock().expect("Agent execution mutex poisoned");
        let Some((store, session, expected)) = &capture.owner else {
            return Ok(false);
        };
        Ok(store.session_attachment(session)?.as_ref() != Some(expected))
    }

    /// Only the invocation's capture owner advances its settlement snapshot.
    /// Dispatch and history retain the immutable snapshot they already took.
    pub(crate) fn prepare_agent_process(
        &self,
        session: &str,
        expected: &crate::process::SessionAttachment,
    ) -> StoreResult<crate::process::SessionAttachment> {
        let mut capture = self.0.lock().expect("Agent execution mutex poisoned");
        let settled = capture.settled_outcome.is_some();
        let (store, id, attachment) = capture
            .owner
            .as_mut()
            .filter(|(_, id, attachment)| !settled && id == session && attachment == expected)
            .ok_or_else(|| StoreError::InvalidAuthority("Capture attachment changed".into()))?;
        let next = store.prepare_session_agent_process(id, attachment)?;
        *attachment = next.clone();
        Ok(next)
    }

    pub(crate) fn environment(&self) -> BTreeMap<String, String> {
        let capture = self.0.lock().expect("Agent execution mutex poisoned");
        let mut environment = BTreeMap::from([(
            CAPTURE_KEY_ENV.to_string(),
            capture.recorder.manifest.artifact_key.to_string(),
        )]);
        if let Ok(declaration) = std::env::var(crate::lf::WORK_DECLARATION_ENV) {
            environment.insert(crate::lf::WORK_DECLARATION_ENV.to_string(), declaration);
        }
        if let Some((_, session, attachment)) = &capture.owner {
            environment.insert(
                crate::process::AGENT_CALLER_ENV.into(),
                serde_json::to_string(&attachment.caller(session.clone().into()))
                    .expect("caller provenance serializes"),
            );
        }
        environment
    }

    pub(crate) fn mark_spawn_requested(&self) {
        self.record_best_effort(ExecutionState::start_attempt);
    }

    pub(crate) fn record_conversation(&self, event: ConversationEvent) {
        self.record_best_effort(|capture| capture.recorder.record_conversation(event));
    }

    pub(crate) fn retry_agent_process(
        &self,
        provider: String,
        model: Option<String>,
        account_id: Option<crate::store::ProviderAccountId>,
        resume_thread: Option<&AgentSessionId>,
    ) -> anyhow::Result<()> {
        let mut capture = self.0.lock().expect("Agent execution mutex poisoned");
        if capture.settled_outcome.is_some() {
            anyhow::bail!("Capture already settled");
        }
        let (store, session, expected) = capture.owner.as_ref().ok_or_else(|| {
            StoreError::InvalidAuthority("AgentProcess retry has no attachment".into())
        })?;
        // The provider may have switched threads while history was unavailable.
        // Its latest observation outranks an older persisted projection.
        let saved_thread = capture
            .agent_session
            .clone()
            .or(store.session_thread(session)?);
        let moved = (capture.account_observed && capture.account_id != account_id)
            || capture.provider != provider;
        if !retry_keeps_its_thread(saved_thread.as_ref(), resume_thread, moved) {
            anyhow::bail!("AgentProcess retry must retain its thread");
        }
        let next = store.replace_session_agent_process(session, expected, resume_thread, || {
            close_session_agent_process(store, session)
        })?;
        capture.owner = Some((store.clone(), session.clone(), next));
        if let Err(error) = capture.fail_and_begin_attempt(provider, model, account_id) {
            capture.recorder.warn_telemetry(error);
        }
        Ok(())
    }

    pub(crate) fn observe_provider(
        &self,
        session_id: Option<AgentSessionId>,
        account_id: Option<crate::store::ProviderAccountId>,
    ) {
        self.record_best_effort(|execution| execution.observe_provider(session_id, account_id));
    }

    pub(crate) fn final_answer(&self) -> StoreResult<Option<String>> {
        let capture = self.0.lock().expect("Agent execution mutex poisoned");
        row_store(&capture.recorder.dir)?
            .input_final_answer(&capture.recorder.manifest.artifact_key)
    }

    pub(crate) fn finish(&self, outcome: &str) -> StoreResult<()> {
        self.0
            .lock()
            .expect("Agent execution mutex poisoned")
            .finish(outcome)
            .map_err(record_error)
    }

    /// This capture's live state, for the execution owner that settles it
    /// after the launcher has exec'd its provider.
    pub(crate) fn hand_off(&self) -> CaptureHandoff {
        let capture = self.0.lock().expect("Agent execution mutex poisoned");
        CaptureHandoff {
            key: capture.recorder.manifest.artifact_key.clone(),
            attempt: capture.recorder.attempt,
            attempt_started: capture.recorder.attempt_started,
            event_seq: capture.recorder.event_seq,
            account_id: capture.account_id.clone(),
            account_observed: capture.account_observed,
            agent_session: capture.agent_session.clone(),
        }
    }

    /// Continue a handed-off capture in this process, at the stream position
    /// its launcher left. Settling it writes the same terminal receipt and
    /// attempt facts the launcher would have.
    pub(crate) fn adopt(handoff: CaptureHandoff) -> StoreResult<Self> {
        // The handoff names the exact capture, so its directory needs no store:
        // the terminal receipt is written even when the store is unavailable.
        let dir = record_dir(&crate::store::lf_home_dir(), &handoff.key).ok_or_else(|| {
            record_error(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid capture key",
            ))
        })?;
        let manifest = read_manifest(&dir).map_err(record_error)?;
        validate_manifest_path(&dir, &manifest).map_err(record_error)?;
        let mut capture = ExecutionState::from_manifest(manifest, dir);
        capture.recorder.attempt = handoff.attempt;
        capture.recorder.attempt_started = handoff.attempt_started;
        capture.recorder.event_seq = handoff.event_seq;
        capture.account_id = handoff.account_id;
        capture.account_observed = handoff.account_observed;
        capture.agent_session = handoff.agent_session;
        Ok(Self(Arc::new(Mutex::new(capture))))
    }

    fn record_best_effort(
        &self,
        operation: impl FnOnce(&mut ExecutionState) -> std::io::Result<()>,
    ) {
        let mut capture = self.0.lock().expect("Agent execution mutex poisoned");
        if let Err(error) = operation(&mut capture) {
            capture.recorder.warn_telemetry(error);
        }
    }
}

impl Drop for ExecutionState {
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
struct ExecutionState {
    owner: Option<AttachmentOwner>,
    provider: String,
    model: Option<String>,
    account_id: Option<crate::store::ProviderAccountId>,
    account_observed: bool,
    agent_session: Option<AgentSessionId>,
    settled_outcome: Option<String>,
    recorder: SessionRecorder,
}

impl ExecutionState {
    /// Independent agent launches, including helpers, admit their conversation
    /// before provider work. Graph reservations have their own fenced publisher.
    fn record_row(
        manifest: &SessionCaptureManifest,
        dir: &Path,
        work: Option<crate::session::SessionWork>,
        context: Option<&crate::trace::PreparedTurnContext>,
    ) -> StoreResult<Option<crate::session::LfSession>> {
        let step = match &manifest.graph {
            Some(SessionGraphMembership::Step(step)) => Some(step),
            Some(SessionGraphMembership::Independent) | None => None,
        };
        // Mechanical commands have a process and, in a Graph, operation history.
        // Capturing their diagnostics does not create an agent conversation.
        if manifest.harness == "loopflow" {
            return Ok(None);
        }
        let store = row_store(dir)?;
        let task = work
            .as_ref()
            .and_then(|work| work.task_id.as_ref())
            .map(|id| store.task(id))
            .transpose()?
            .flatten();
        let title = generated_session_title(
            context,
            manifest.skill.as_deref(),
            task.as_ref().map(|task| task.title.as_str()),
            &manifest.cwd,
        );
        let session = store.create_session(
            crate::session::LfSession {
                captured: None,
                caller_artifact_key: manifest.caller_artifact_key.clone(),
                id: format!("session_{}", Uuid::new_v4().simple()).into(),
                artifact_key: Some(manifest.artifact_key.clone()),
                input_published: false,
                cwd: manifest.cwd.clone(),
                skill: manifest.skill.clone(),
                provider: Some(manifest.harness.clone()),
                model: manifest.model.clone(),
                node: step.and_then(|step| step.key),
                iterations: step.and_then(|step| step.iterations.clone()),
                task_id: work.as_ref().and_then(|work| work.task_id.clone()),
                wave_id: work.as_ref().and_then(|work| work.wave_id.clone()),
                work_source: work.as_ref().map(|work| work.source),
                graph_lf_process_id: None,
                bound_at: None,
                interactive: manifest.surface != "headless",
                repo: None,
                title,
                title_source: crate::session::TitleSource::Generated,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: manifest.created_at.unix_timestamp(),
            },
            crate::journal::current_lf_process_id().as_ref(),
        )?;
        Ok(Some(session))
    }

    fn from_manifest(manifest: SessionCaptureManifest, dir: PathBuf) -> Self {
        Self {
            owner: None,
            provider: manifest.harness.clone(),
            model: manifest.model.clone(),
            account_id: manifest
                .process
                .as_ref()
                .and_then(|process| process.account_id.clone()),
            account_observed: false,
            agent_session: None,
            settled_outcome: None,
            recorder: SessionRecorder::from_manifest(manifest, dir),
        }
    }

    fn observe_provider(
        &mut self,
        session: Option<AgentSessionId>,
        account: Option<crate::store::ProviderAccountId>,
    ) -> std::io::Result<()> {
        let account_changed = !self.account_observed || self.account_id != account;
        let thread_changed = session
            .as_ref()
            .is_some_and(|session| account_changed || self.agent_session.as_ref() != Some(session));
        // Live identity advances even if keeping its history fails.
        self.account_id = account;
        self.account_observed = true;
        if account_changed {
            self.agent_session = None;
        }
        if let Some(session) = session {
            self.agent_session = Some(session);
        }
        self.recorder.observe_provider(
            self.agent_session.as_ref(),
            self.account_id.as_ref(),
            account_changed,
            thread_changed,
        )
    }

    fn start_attempt(&mut self) -> std::io::Result<()> {
        self.recorder.start_attempt(
            &self.provider,
            self.model.as_deref(),
            self.account_id.as_ref(),
        )
    }

    fn fail_and_begin_attempt(
        &mut self,
        provider: String,
        model: Option<String>,
        account_id: Option<crate::store::ProviderAccountId>,
    ) -> std::io::Result<()> {
        self.provider = provider;
        self.model = model;
        self.account_id = account_id;
        self.account_observed = false;
        self.agent_session = None;
        self.recorder.next_attempt(
            &self.provider,
            self.model.as_deref(),
            self.account_id.as_ref(),
        )
    }

    fn finish(&mut self, outcome: &str) -> std::io::Result<()> {
        if !matches!(outcome, "completed" | "failed" | "interrupted") {
            return Err(std::io::Error::other(format!(
                "invalid recorder outcome: {outcome}"
            )));
        }
        if let Some(settled) = &self.settled_outcome {
            if settled == outcome {
                return Ok(());
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("Recorder already settled as {settled}; refusing {outcome}"),
            ));
        }
        self.recorder.finish(outcome)?;
        self.settled_outcome = Some(outcome.to_string());
        if let Some((store, session, attachment)) = self.owner.take() {
            match finish_session_attachment(&store, &session, &attachment, outcome) {
                Ok(_) | Err(StoreError::InvalidAuthority(_)) => {}
                Err(error) => {
                    tracing::warn!(%error, %session, "cannot record the Session attachment's end")
                }
            }
        }
        Ok(())
    }
}

/// Resume the saved conversation after settling its exact former AgentProcess.
pub(crate) fn resume_session_agent_process(
    store: &crate::store::sqlite::SqliteStore,
    session: &str,
    lf_process_id: &crate::id::LfProcessId,
) -> StoreResult<crate::process::SessionAttachment> {
    let expected = store.session_attachment(session)?;
    store.resume_session_attachment(session, expected.as_ref(), lf_process_id, || {
        crate::harness::agent_process::close_session_agent_process(store, session)
    })
}

impl AgentExecution {
    pub(crate) async fn observe_activity(&self, body_pid: Option<u32>) {
        let processes = super::activity::sample(body_pid).await;
        self.record_best_effort(|capture| {
            let now = time::OffsetDateTime::now_utc();
            let quiet_since = capture.recorder.activity.observe(now, body_pid, processes);
            let Some((store, session, attachment)) = &capture.owner else {
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
    use std::io::{BufRead, BufReader};
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    use crate::id::LfProcessId;
    use crate::store::sqlite::SqliteStore;

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
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("resume", &crate::session_record::new_artifact_key());
        let sql = rusqlite::Connection::open(home.path().join("store.db")).unwrap();
        sql.execute(
            "UPDATE agent_sessions SET provider='codex',interactive=0",
            [],
        )
        .unwrap();
        let attached = store
            .claim_session_attachment("resume", None, &store.test_lf_process(), true)
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
            let store = SqliteStore::new(&home.join("store.db")).unwrap();
            std::fs::write(home.join("recovering"), "ready").unwrap();
            let result = crate::session_record::resume_session_agent_process(
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
            .create_task(&crate::work::task::NewTask {
                id: crate::durable::TaskId::new(),
                repository_id: store
                    .ensure_repository(home.path().to_str().unwrap())
                    .unwrap(),
                project_id: None,
                title: "Retain Session work".into(),
                description: "Existing work before owner replacement".into(),
                due_date: None,
            })
            .unwrap();
        let capture = store.session("resume").unwrap().unwrap().captured;
        store.bind_session("resume", capture, &task.id).unwrap();
        let attached = store
            .claim_session_attachment("resume", Some(&detached), &store.test_lf_process(), false)
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
        let birth = crate::journal::process_started_at(pid).unwrap().unwrap();
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
        assert!(!crate::journal::OsProcess::group(pid).unwrap().is_empty());
        let test = if exits {
            "resume_waits_for_a_dead_owners_helpers_without_lifeline_authority"
        } else {
            "resume_refuses_a_dead_owners_persistent_helpers_without_settling"
        };
        let mut recovery = Child(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    &format!("session_record::runtime::tests::{test}"),
                ])
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
        let attached = store
            .claim_session_attachment("resume", Some(&detached), &LfProcessId::new(), false)
            .unwrap();
        let mut command = std::process::Command::new("/bin/sleep");
        command.env_clear().arg("60");
        let mut child = Child(
            crate::harness::agent_process::spawn_native(
                command,
                &(store.clone(), "resume".into(), attached.clone()),
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
        assert!(crate::session_record::resume_session_agent_process(
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
        assert!(crate::session_record::resume_session_agent_process(
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

        let next = crate::session_record::resume_session_agent_process(
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
        assert!(ended.outcome.is_none());
    }

    #[test]
    fn resume_detached_live_agent_uses_recorded_settings_and_preserves_history() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (home, store, detached) = resume_fixture();
        let attached = store
            .claim_session_attachment("resume", Some(&detached), &LfProcessId::new(), false)
            .unwrap();
        let mut command = std::process::Command::new("/bin/sleep");
        command.env_clear().arg("60").process_group(0);
        let child = Child(command.spawn().unwrap());
        let pid = child.0.id();
        let birth = crate::journal::process_started_at(pid).unwrap().unwrap();
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
        let next = crate::session_record::resume_session_agent_process(
            &store,
            "resume",
            &store.test_lf_process(),
        )
        .unwrap();
        assert_ne!(next.agent_process_id, attached.agent_process_id);
        assert_eq!(
            crate::journal::process_identity_evidence(pid, birth),
            crate::journal::ProcessIdentityEvidence::Dead
        );
        let ended = store
            .agent_process(&attached.agent_process_id)
            .unwrap()
            .unwrap();
        assert!(ended.completed_at.is_some());
        assert!(ended.outcome.is_none());
        assert_eq!(ended.pid, Some(pid));
        assert_eq!(
            store.session_thread("resume").unwrap(),
            Some("saved-thread".into())
        );
        assert!(store.session_connection("resume").unwrap().is_none());
    }

    #[test]
    fn resume_unknown_spawn_preserves_record_attachment_and_input() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (_home, store, detached) = resume_fixture();
        let attached = store
            .claim_session_attachment("resume", Some(&detached), &LfProcessId::new(), false)
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
        assert!(store
            .claim_session_input(next, Some(&detached), &store.test_lf_process(), || {
                super::close_session_agent_process(&store, "resume")
            })
            .is_err());
        assert!(crate::session_record::resume_session_agent_process(
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
        let attached = store
            .claim_session_attachment("resume", Some(&detached), &LfProcessId::new(), false)
            .unwrap();
        let mut command = std::process::Command::new("/bin/sleep");
        command.env_clear().arg("60").process_group(0);
        let mut child = Child(command.spawn().unwrap());
        let pid = child.0.id();
        let birth = crate::journal::process_started_at(pid).unwrap().unwrap();
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
        let duplicate = store
            .claim_session_attachment("duplicate", None, &store.test_lf_process(), true)
            .unwrap();
        store
            .record_agent_process_identity("duplicate", &duplicate, pid, birth)
            .unwrap();
        let error = crate::session_record::resume_session_agent_process(
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
        let attached = store
            .claim_session_attachment("resume", Some(&detached), &LfProcessId::new(), false)
            .unwrap();
        let mut child = Child(
            std::process::Command::new("/bin/sleep")
                .env_clear()
                .arg("60")
                .spawn()
                .unwrap(),
        );
        let pid = child.0.id();
        let birth = crate::journal::process_started_at(pid).unwrap().unwrap();
        store
            .record_agent_process_identity("resume", &attached, pid, birth)
            .unwrap();
        store
            .release_session_attachment("resume", &attached)
            .unwrap();
        child.0.kill().unwrap();
        // Observe this fixture's zombie without reaping it through Child::wait.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while crate::journal::process_identity_evidence(pid, birth)
            != crate::journal::ProcessIdentityEvidence::Dead
        {
            assert!(
                std::time::Instant::now() < deadline,
                "fixture child did not exit"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let next = crate::session_record::resume_session_agent_process(
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
        assert!(ended.outcome.is_none());
    }

    #[test]
    fn resume_fences_takeover_until_settlement_and_claim_commit() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (_home, store, detached) = resume_fixture();
        let attached = store
            .claim_session_attachment("resume", Some(&detached), &LfProcessId::new(), false)
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
            let next = store
                .resume_session_attachment("resume", Some(&detached), &resumer, || {
                    let store = &store;
                    let expected = &detached;
                    scope.spawn(move || {
                        started.send(()).unwrap();
                        let result = store.claim_session_attachment(
                            "resume",
                            Some(expected),
                            &LfProcessId::new(),
                            false,
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
    fn headless_close_obeys_the_current_attachment_for_every_provider() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        for provider in ["claude", "opencode", "codex"] {
            let (home, store, detached) = resume_fixture();
            rusqlite::Connection::open(home.path().join("store.db"))
                .unwrap()
                .execute("UPDATE agent_sessions SET provider=?1", [provider])
                .unwrap();
            let launch = LfProcessId::new();
            let takeover = LfProcessId::new();
            let sql = rusqlite::Connection::open(home.path().join("store.db")).unwrap();
            for process in [&launch, &takeover] {
                sql.execute(
                    "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
                    [process],
                )
                .unwrap();
            }
            // A fresh AgentProcess records the provider at launch, not the
            // Session's mutable next-launch settings.
            let first = store
                .claim_session_attachment("resume", Some(&detached), &launch, true)
                .unwrap();
            let mut command = std::process::Command::new("/bin/sleep");
            command.env_clear().arg("60").process_group(0);
            store
                .record_agent_process_launch("resume", &first, &command, false)
                .unwrap();
            let child = Child(command.spawn().unwrap());
            let pid = child.0.id();
            let birth = crate::journal::process_started_at(pid).unwrap().unwrap();
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
            let second = store
                .claim_session_attachment("resume", Some(&first), &takeover, false)
                .unwrap();
            assert_eq!(first.agent_process_id, second.agent_process_id);
            assert!(
                super::finish_session_attachment(&store, "resume", &first, "interrupted").is_err()
            );
            assert_eq!(
                crate::journal::process_identity_evidence(pid, birth),
                crate::journal::ProcessIdentityEvidence::Live
            );
            assert_eq!(
                store.session_attachment("resume").unwrap(),
                Some(second.clone())
            );
            let returned = store
                .claim_session_attachment("resume", Some(&second), &launch, false)
                .unwrap();
            assert_eq!(returned.agent_process_id, first.agent_process_id);
            assert_ne!(returned.token, first.token);
            for stale in [&first, &second] {
                assert!(
                    super::finish_session_attachment(&store, "resume", stale, "interrupted")
                        .is_err()
                );
            }
            assert_eq!(
                crate::journal::process_identity_evidence(pid, birth),
                crate::journal::ProcessIdentityEvidence::Live
            );
            super::finish_session_attachment(&store, "resume", &returned, "interrupted").unwrap();
            assert_eq!(
                crate::journal::process_identity_evidence(pid, birth),
                crate::journal::ProcessIdentityEvidence::Dead
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
            let attachment = store
                .claim_session_attachment("resume", Some(&detached), &store.test_lf_process(), true)
                .unwrap();
            let mut command = std::process::Command::new("/bin/sleep");
            command.env_clear().arg("60").process_group(0);
            store
                .record_agent_process_launch("resume", &attachment, &command, true)
                .unwrap();
            let mut child = Child(command.spawn().unwrap());
            let pid = child.0.id();
            let birth = crate::journal::process_started_at(pid).unwrap().unwrap();
            store
                .record_agent_process_identity("resume", &attachment, pid, birth)
                .unwrap();
            assert!(!store
                .with_session_attachment("resume", &attachment, || {
                    super::close_session_agent_process(&store, "resume")
                })
                .unwrap());
            assert_eq!(
                crate::journal::process_identity_evidence(pid, birth),
                crate::journal::ProcessIdentityEvidence::Live
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
        let store = SqliteStore::open_ephemeral(&database).unwrap();
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
        let attachment = store
            .claim_session_attachment("retry", None, &parent, true)
            .unwrap();
        let mut command = std::process::Command::new("/bin/sleep");
        command.env_clear().arg("60").process_group(0);
        store
            .record_agent_process_launch("retry", &attachment, &command, false)
            .unwrap();
        let child = Child(command.spawn().unwrap());
        let pid = child.0.id();
        let birth = crate::journal::process_started_at(pid).unwrap().unwrap();
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
        let next = store
            .replace_session_agent_process(
                "retry",
                &attachment,
                Some(&"retained-thread".into()),
                || super::close_session_agent_process(&store, "retry"),
            )
            .unwrap();
        assert_ne!(next.agent_process_id, attachment.agent_process_id);
        assert_eq!(
            crate::journal::process_identity_evidence(pid, birth),
            crate::journal::ProcessIdentityEvidence::Dead
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
#[path = "execution_tests.rs"]
mod capture_tests;
