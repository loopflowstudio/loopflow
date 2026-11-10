//! Capture publication and recording handoff. No process control lives here.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    boot_id, inherited_capture_key, new_artifact_key, prepare_manifest, publish_manifest,
    read_manifest, reconcile_private_file, reconcile_reserved_manifest, record_dir, record_error,
    resolve_manifest, row_store, runtime_identity, sync_dir, validate_manifest_path,
    write_private_exclusive, AgentProcessRequest, SessionCaptureManifest, SessionCaptureSpec,
    SessionContextArtifact, SessionContextRef, SessionGraphMembership, SessionRecorder,
    SCHEMA_VERSION,
};
use crate::naming::generated_session_title;
use crate::store::StoreResult;

/// Recording position and last provider observation for the native-exit owner.
/// This carries no attachment authority or live runtime resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CaptureHandoff {
    pub key: String,
    attempt: u32,
    attempt_started: bool,
    event_seq: u64,
    account_id: Option<crate::store::ProviderAccountId>,
    account_observed: bool,
    agent_session: Option<crate::id::AgentSessionId>,
}

impl SessionRecorder {
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

    pub(crate) fn prepare_at_with_key(
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
        Ok(Self::from_manifest(manifest, dir))
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

    pub(crate) fn begin_reserved_at(
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
        Ok(Self::from_manifest(manifest, dir))
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
    pub(crate) fn begin_at_with_request(
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
        let reserved = Self::record_row(&manifest, &dir, work, context)?;
        publish_manifest(lf_home, &manifest, context_bytes.as_deref()).map_err(record_error)?;
        if let Some(session) = reserved {
            row_store(&dir)?.publish_capture(&session.id, session.captured)?;
        }
        Ok(Self::from_manifest(manifest, dir))
    }

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

    pub(crate) fn artifact_key(&self) -> String {
        self.manifest.artifact_key.clone()
    }

    pub(crate) fn artifact_dir(&self) -> PathBuf {
        self.dir.clone()
    }

    pub(crate) fn hand_off(
        &self,
        account_id: Option<crate::store::ProviderAccountId>,
        account_observed: bool,
        agent_session: Option<crate::id::AgentSessionId>,
    ) -> CaptureHandoff {
        CaptureHandoff {
            key: self.artifact_key(),
            attempt: self.attempt,
            attempt_started: self.attempt_started,
            event_seq: self.event_seq,
            account_id,
            account_observed,
            agent_session,
        }
    }

    pub(crate) fn adopt(handoff: CaptureHandoff) -> StoreResult<Self> {
        let dir = record_dir(&crate::store::lf_home_dir(), &handoff.key).ok_or_else(|| {
            record_error(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid capture key",
            ))
        })?;
        let manifest = read_manifest(&dir).map_err(record_error)?;
        validate_manifest_path(&dir, &manifest).map_err(record_error)?;
        let mut recorder = Self::from_manifest(manifest, dir);
        recorder.attempt = handoff.attempt;
        recorder.attempt_started = handoff.attempt_started;
        recorder.event_seq = handoff.event_seq;
        Ok(recorder)
    }
}

#[cfg(test)]
mod tests {
    use super::SessionRecorder;
    use crate::chat::types::ConversationEvent;
    use crate::session_record::{SessionCaptureSpec, SessionGraphMembership};
    use time::OffsetDateTime;

    fn spec(home: &std::path::Path) -> SessionCaptureSpec {
        SessionCaptureSpec {
            harness: "proof".into(),
            model: None,
            surface: "headless".into(),
            cwd: home.into(),
            repo: None,
            worktree: None,
            skill: None,
            subjects: Vec::new(),
            graph: SessionGraphMembership::Independent,
            work: None,
        }
    }
    #[test]
    fn history_records_a_long_gap_as_a_silence() {
        let home = tempfile::tempdir().unwrap();
        let mut capture =
            crate::session_record::SessionRecorder::begin_at(home.path(), spec(home.path()))
                .unwrap();
        let say = |capture: &mut SessionRecorder, content: &str| {
            capture
                .record_conversation(ConversationEvent::TextDelta {
                    turn_id: "turn-1".into(),
                    content: content.into(),
                })
                .unwrap()
        };
        say(&mut capture, "thinking");
        let since = OffsetDateTime::now_utc() - time::Duration::seconds(700);
        capture.previous_event = Some(since);
        say(&mut capture, "still thinking");
        say(&mut capture, "done");

        let store = crate::session_record::row_store(&capture.artifact_dir()).unwrap();
        let silences = store.input_silences(&capture.artifact_key()).unwrap();
        assert_eq!(silences.len(), 1);
        assert_eq!(silences[0].0, since.unix_timestamp());
        assert!(store
            .input_events(&capture.artifact_key())
            .unwrap()
            .is_empty());
    }
}
