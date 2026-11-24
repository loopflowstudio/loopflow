//! Captured Session inputs and append-only provider evidence.

pub mod activity;
mod capture;
#[cfg(test)]
mod capture_tests;

pub use capture::CaptureHandoff;
use loopflow_model::id::AgentSessionId;

use std::fs::{self, File, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::store::{StoreError, StoreResult};
use loopflow_model::chat::types::ConversationEvent;

pub use loopflow_storage::session_record::*;

#[derive(Debug, Clone)]
pub struct SessionCaptureSpec {
    pub harness: String,
    pub model: Option<String>,
    pub surface: String,
    pub cwd: PathBuf,
    pub repo: Option<PathBuf>,
    pub worktree: Option<PathBuf>,
    pub skill: Option<String>,
    pub subjects: Vec<SubjectAttribution>,
    pub graph: SessionGraphMembership,
    pub work: Option<loopflow_storage::session::SessionWork>,
}

/// A Graph position older manifests captured. New captures record none: a step
/// is an ordinary command, and its Graph is read from the Graph process record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionGraphStep {
    pub task_id: Option<loopflow_storage::work::task::TaskId>,
    pub task_pr_id: Option<loopflow_storage::work::task::TaskPrId>,
    pub invocation_id: String,
    #[serde(rename = "flow")] // Existing capture manifests remain replayable.
    pub graph: String,
    pub step: String,
    /// Older manifests omitted the structural path; never infer it from a leaf index.
    pub node: Option<String>,
    /// The step's node in its Graph process graph; absent in older manifests.
    #[serde(default)]
    pub key: Option<u32>,
    /// Older captures have no tuple; never derive it from their scalar visit token.
    pub iterations: Option<Vec<Vec<u32>>>,
}

/// Membership at capture time; absence in historical manifests remains unknown.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionGraphMembership {
    Step(SessionGraphStep),
    Independent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubjectAttribution {
    pub selector: String,
    pub source: AttributionSource,
}

impl SubjectAttribution {
    pub fn declared(selector: String) -> Self {
        Self {
            selector,
            source: AttributionSource::Declared,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttributionSource {
    Declared,
    Inherited,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionContextRef {
    pub path: String,
    pub content_sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalReceipt {
    pub schema_version: u32,
    pub outcome: String,
    #[serde(with = "time::serde::rfc3339")]
    pub ended_at: OffsetDateTime,
    pub result_ref: Option<String>,
}

pub fn resolve_manifest(
    lf_home: &Path,
    selector: &str,
) -> std::io::Result<(PathBuf, SessionCaptureManifest)> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Capture selector cannot be empty",
        ));
    }
    if let Some(dir) = record_dir(lf_home, selector) {
        if dir.join("prepared").is_file() {
            let manifest = read_manifest(&dir)?;
            validate_manifest_path(&dir, &manifest)?;
            return Ok((dir, manifest));
        }
    }
    let database = database_in(lf_home).map_err(std::io::Error::other)?;
    let store = loopflow_storage::sqlite::SqliteStore::open_read_only(
        &database,
        crate::store::token_key::encryption_key,
    )
    .map_err(std::io::Error::other)?;
    let artifact = store
        .resolve_history_input(selector)
        .map_err(|error| match error {
            StoreError::NotFound => {
                std::io::Error::new(std::io::ErrorKind::NotFound, "Capture not found")
            }
            error => std::io::Error::other(error),
        })?;
    let session = store.session(&artifact).map_err(std::io::Error::other)?;
    let artifact = session
        .filter(|session| session.id.as_str() == loopflow_model::id::canonical_session(&artifact))
        .and_then(|session| session.artifact_key)
        .unwrap_or(artifact);
    let dir = record_dir(lf_home, &artifact).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid capture key")
    })?;
    let manifest = read_manifest(&dir)?;
    validate_manifest_path(&dir, &manifest)?;
    Ok((dir, manifest))
}

pub fn read_provider_session(dir: &Path) -> std::io::Result<Option<ProviderSessionRef>> {
    let input = input_id_from_dir(dir)?;
    loopflow_storage::sqlite::SqliteStore::open_read_only(
        &row_database(dir).map_err(std::io::Error::other)?,
        crate::store::token_key::encryption_key,
    )
    .and_then(|store| store.input_provider_session(&input))
    .map_err(std::io::Error::other)
}

pub(crate) fn input_id_from_dir(dir: &Path) -> std::io::Result<String> {
    let id = dir
        .file_name()
        .and_then(|id| id.to_str())
        .ok_or_else(|| std::io::Error::other("input path has no identifier"))?;
    parse_artifact_key(id).map_err(std::io::Error::other)
}

pub fn write_provider_session(
    dir: &Path,
    agent_session: &AgentSessionId,
    account_id: Option<crate::store::ProviderAccountId>,
) -> std::io::Result<()> {
    if agent_session.as_str().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "provider session id cannot be empty",
        ));
    }
    let input = input_id_from_dir(dir)?;
    let store = row_store(dir).map_err(std::io::Error::other)?;
    let session = store
        .session_for_artifact(&input)
        .map_err(std::io::Error::other)?
        .ok_or_else(|| std::io::Error::other("provider history has no admitted Session"))?;
    store
        .retain_session_observation(
            &session,
            &provider_session_observation(&session, &input, agent_session, account_id),
        )
        .map_err(std::io::Error::other)
}

/// A terminal marker is valid only on the PTY where the shell installed it.
/// Inherited environment after app, SSH, or background handoff is not attachment.
pub fn current_terminal_id() -> Option<String> {
    let id = std::env::var("LF_TERMINAL_ID")
        .ok()
        .filter(|id| !id.is_empty())?;
    let expected = std::env::var("LF_TERMINAL_TTY").ok()?;
    (stdin_tty()? == expected).then_some(id)
}

/// The device of this process's terminal, such as `/dev/ttys004`.
pub(crate) fn stdin_tty() -> Option<String> {
    let mut name = [0 as libc::c_char; 1024];
    // SAFETY: name is a writable buffer of the supplied length. ttyname_r
    // writes a NUL-terminated name on success and retains no pointers.
    let result = unsafe { libc::ttyname_r(libc::STDIN_FILENO, name.as_mut_ptr(), name.len()) };
    if result != 0 {
        return None;
    }
    // SAFETY: successful ttyname_r above terminated the buffer.
    unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }
        .to_str()
        .ok()
        .map(str::to_owned)
}

fn validate_manifest_path(dir: &Path, manifest: &SessionCaptureManifest) -> std::io::Result<()> {
    parse_artifact_key(manifest.artifact_key.as_str()).map_err(std::io::Error::other)?;
    if dir.file_name().and_then(|name| name.to_str()) != Some(manifest.artifact_key.as_str())
        || dir
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            != manifest
                .artifact_key
                .as_str()
                .strip_prefix("run_")
                .unwrap_or(&manifest.artifact_key)
                .get(..2)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Session capture manifest identity does not match its record path",
        ));
    }
    Ok(())
}

pub fn read_manifest(dir: &Path) -> std::io::Result<SessionCaptureManifest> {
    let bytes = fs::read(dir.join("manifest.json"))?;
    let manifest =
        serde_json::from_slice::<SessionCaptureManifest>(&bytes).map_err(std::io::Error::other)?;
    if manifest.schema_version != SCHEMA_VERSION {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "unsupported Session capture manifest schema {}; expected {SCHEMA_VERSION}",
                manifest.schema_version
            ),
        ));
    }
    Ok(manifest)
}

/// An attempt that ended so says nothing its turn rows do not.
const COMPLETED: &str = "completed";

/// Append-only capture data. This value has no attachment or process authority.
#[derive(Debug)]
pub struct SessionRecorder {
    pub manifest: SessionCaptureManifest,
    pub dir: PathBuf,
    /// The live identity is known, but its last observation could not be kept.
    provider_observation_pending: bool,
    attempt: u32,
    attempt_started: bool,
    /// The current attempt's start, at its stream position: kept only when
    /// the attempt is retried or does not complete, since an ordinary turn's
    /// row in history already says when it ran.
    attempt_start: Option<(u64, OffsetDateTime, CaptureFact)>,
    event_seq: u64,
    /// The store the input's row and its history are kept in; none for a
    /// mechanical command's capture.
    store: Option<(
        loopflow_storage::sqlite::SqliteStore,
        crate::session::LfSession,
    )>,
    /// When the stream's previous event arrived, to record silences.
    previous_event: Option<OffsetDateTime>,
    telemetry_warned: bool,
    terminal_recorded: bool,
    pub activity: activity::Observer,
}

impl SessionRecorder {
    fn from_manifest(manifest: SessionCaptureManifest, dir: PathBuf) -> Self {
        let store = if manifest.harness == "loopflow" {
            Ok(None)
        } else {
            row_store(&dir).and_then(|store| {
                let session = store.session_for_artifact(&manifest.artifact_key)?;
                Ok(session.map(|session| (store, session)))
            })
        };
        let store = store.unwrap_or_else(|error| {
            tracing::warn!(%error, "Session history unavailable");
            None
        });
        if let Some((store, session)) = &store {
            // Replay reads the prompt from the capture's own manifest file.
            let retained = SessionCaptureManifest {
                process: None,
                ..manifest.clone()
            };
            let input = &manifest.artifact_key;
            if let Err(error) = store.retain_session_observation(
                session,
                &crate::session::SessionObservation {
                    artifact_key: input.clone(),
                    source: "manifest.json".into(),
                    observed_at: retained.created_at.unix_timestamp(),
                    task_id: session.task_id.clone(),
                    wave_id: session.wave_id.clone(),
                    payload: serde_json::json!({"input_id": input, "source": "manifest.json", "evidence": retained}),
                },
            ) {
                tracing::warn!(%error, "Session capture observation unavailable");
            }
        }
        Self {
            manifest,
            dir,
            provider_observation_pending: false,
            attempt: 1,
            attempt_started: false,
            attempt_start: None,
            event_seq: 0,
            store,
            previous_event: None,
            telemetry_warned: false,
            terminal_recorded: false,
            activity: activity::Observer::default(),
        }
    }

    /// Publish the conversation `lf resume` reopens. A failed write is retried
    /// at the next observation even when nothing changed.
    pub fn observe_provider(
        &mut self,
        session: Option<&AgentSessionId>,
        thread_changed: bool,
    ) -> std::io::Result<()> {
        if !(thread_changed || self.provider_observation_pending) {
            return Ok(());
        }
        let Some(session) = session else {
            return Ok(());
        };
        self.provider_observation_pending = true;
        write_provider_session(&self.dir, session, None)?;
        self.provider_observation_pending = false;
        Ok(())
    }

    pub(crate) fn start_attempt(
        &mut self,
        provider: &str,
        model: Option<&str>,
    ) -> std::io::Result<()> {
        if self.attempt_started {
            return Ok(());
        }
        self.attempt_started = true;
        let (seq, at) = self.next_position()?;
        self.attempt_start = Some((
            seq,
            at,
            CaptureFact::ProviderAttemptStarted {
                provider: provider.to_owned(),
                model: model.map(str::to_owned),
                account_id: None,
                attempt_key: self.attempt_key(),
            },
        ));
        Ok(())
    }

    /// Keep how the current attempt ended, with its start, unless it
    /// completed: then history holds nothing the turn's row does not.
    fn finish_attempt(&mut self, outcome: &str) -> std::io::Result<()> {
        if !self.attempt_started {
            return Ok(());
        }
        let start = self.attempt_start.take();
        if outcome == COMPLETED {
            return Ok(());
        }
        if let Some((seq, at, fact)) = start {
            self.keep_fact(seq, at, &fact)?;
        }
        self.record_fact(CaptureFact::ProviderAttemptFinished {
            attempt_key: self.attempt_key(),
            outcome: outcome.to_owned(),
        })
    }

    pub(crate) fn next_attempt(
        &mut self,
        provider: &str,
        model: Option<&str>,
    ) -> std::io::Result<()> {
        let finish_error = self.finish_attempt("failed").err();
        self.provider_observation_pending = false;
        self.attempt += 1;
        self.attempt_started = false;
        let start_error = self.start_attempt(provider, model).err();
        match (finish_error, start_error) {
            (None, None) => Ok(()),
            (Some(error), None) | (None, Some(error)) => Err(error),
            (Some(finish), Some(start)) => Err(std::io::Error::other(format!(
                "failed to record prior attempt outcome: {finish}; failed to record new attempt: {start}"
            ))),
        }
    }

    fn attempt_key(&self) -> String {
        format!("attempt-{}", self.attempt)
    }

    /// Keep a turn that did not complete and any error, the error also on
    /// the input's row for the Graph; any other event only ticks the silence
    /// clock.
    pub(crate) fn record_conversation(&mut self, event: ConversationEvent) -> std::io::Result<()> {
        let end = match event {
            ConversationEvent::TurnCompleted { turn_id, status }
                if status != loopflow_model::chat::types::Lifecycle::Completed =>
            {
                TurnEnd::TurnCompleted { turn_id, status }
            }
            ConversationEvent::Error {
                code,
                message,
                evidence,
            } => {
                let error = format!("{code}: {message}");
                let kept = self.record_fact(CaptureFact::Conversation {
                    event: TurnEnd::Error {
                        code,
                        message,
                        evidence: evidence.map(Box::new),
                    },
                });
                if let Some((store, _)) = &self.store {
                    if let Err(error) =
                        store.record_input_error(&self.manifest.artifact_key, &error)
                    {
                        self.warn_telemetry(std::io::Error::other(error));
                    }
                }
                return kept;
            }
            _ => return self.next_position().map(drop),
        };
        self.record_fact(CaptureFact::Conversation { event: end })
    }

    /// Keep `fact` in history.
    fn record_fact(&mut self, fact: CaptureFact) -> std::io::Result<()> {
        let (seq, observed_at) = self.next_position()?;
        self.keep_fact(seq, observed_at, &fact)
    }

    /// The next stream position, after keeping the silence that preceded it.
    /// Every provider event the capture hears takes one.
    fn next_position(&mut self) -> std::io::Result<(u64, OffsetDateTime)> {
        let seq = self.event_seq;
        let at = OffsetDateTime::now_utc();
        self.activity.note_event(at);
        self.event_seq += 1;
        let silence = self
            .previous_event
            .replace(at)
            .filter(|since| (at - *since).whole_seconds() > SILENCE_SECONDS);
        match silence {
            Some(since) => self
                .keep_fact(seq, at, &CaptureFact::Silence { since })
                .map(|()| (seq, at)),
            None => Ok((seq, at)),
        }
    }

    fn keep_fact(&self, seq: u64, at: OffsetDateTime, fact: &CaptureFact) -> std::io::Result<()> {
        let Some((store, session)) = &self.store else {
            return Ok(());
        };
        store
            .record_capture_fact(session, &self.manifest.artifact_key, seq, at, fact)
            .map_err(std::io::Error::other)
    }

    pub(crate) fn warn_telemetry(&mut self, error: std::io::Error) {
        if self.telemetry_warned {
            tracing::debug!(%error, artifact_key = %self.manifest.artifact_key, "Session telemetry write failed");
            return;
        }
        self.telemetry_warned = true;
        tracing::warn!(
            %error,
            artifact_key = %self.manifest.artifact_key,
            "Session telemetry write failed; harness execution continues"
        );
    }

    pub fn finish(&mut self, outcome: &str) -> std::io::Result<()> {
        if self.terminal_recorded {
            return Ok(());
        }
        let terminal = TerminalReceipt {
            schema_version: SCHEMA_VERSION,
            outcome: outcome.to_string(),
            ended_at: OffsetDateTime::now_utc(),
            result_ref: None,
        };
        let terminal = write_terminal(&self.dir, terminal)?;
        self.terminal_recorded = true;
        // The receipt file records settlement, which no turn row says: a
        // completed turn is not the command's outcome. A missed store
        // observation must not fail the exit.
        if self.manifest.harness != "loopflow" {
            let input = &self.manifest.artifact_key;
            let observed = row_store(&self.dir).and_then(|store| {
                let Some(session) = store.session_for_artifact(input)? else {
                    return Ok(());
                };
                store.retain_session_observation(
                    &session,
                    &crate::session::SessionObservation {
                        artifact_key: input.clone(),
                        source: "terminal.json".into(),
                        observed_at: terminal.ended_at.unix_timestamp(),
                        task_id: session.task_id.clone(),
                        wave_id: session.wave_id.clone(),
                        payload: serde_json::json!({
                            "input_id": input,
                            "source": "terminal.json",
                            "evidence": terminal,
                        }),
                    },
                )
            });
            if let Err(error) = observed {
                tracing::warn!(%error, artifact_key = %input, "terminal receipt written; its Session observation was not recorded");
            }
        }
        if let Err(error) = self.finish_attempt(outcome) {
            tracing::warn!(
                %error,
                artifact_key = %self.manifest.artifact_key,
                "final recorder lifecycle event unavailable"
            );
        }
        Ok(())
    }
}

/// The store that holds the conversation for the capture recorded at `dir`.
/// The final answer of the input recorded at `dir`, from its provider's
/// transcript.
///
/// # Errors
/// When the store cannot be opened or the transcript cannot be read.
pub fn final_answer(dir: &Path) -> anyhow::Result<Option<String>> {
    crate::transcript::answer::input_final_answer(&row_store(dir)?, &input_id_from_dir(dir)?)
}

pub(crate) fn row_store(dir: &Path) -> StoreResult<loopflow_storage::sqlite::SqliteStore> {
    crate::store::open_sqlite(&row_database(dir)?)
}

fn row_database(dir: &Path) -> StoreResult<PathBuf> {
    let home = dir
        .ancestors()
        .nth(3)
        .ok_or_else(|| record_error(std::io::Error::other("Session capture has no Machine")))?;
    database_in(home)
}

fn database_in(home: &Path) -> StoreResult<PathBuf> {
    #[cfg(test)]
    {
        Ok(home.join("loopflow.db"))
    }
    #[cfg(not(test))]
    {
        let _ = home;
        crate::store::database_path_from_env().map_err(record_error)
    }
}

pub(crate) fn inherited_capture_key() -> StoreResult<Option<String>> {
    let Some(value) = std::env::var_os(CAPTURE_KEY_ENV) else {
        return Ok(None);
    };
    let key = value
        .into_string()
        .map_err(|_| record_error(std::io::Error::other("capture key is not valid UTF-8")))?;
    let (_, owner) = resolve_capture(&key)?;
    if let Some(caller) = crate::journal::agent_caller() {
        if owner.id.as_str() != loopflow_model::id::canonical_session(&caller.session_id) {
            return Err(record_error(std::io::Error::other(
                "capture belongs to another Session",
            )));
        }
    }
    Ok(Some(key))
}

/// A capture is subordinate to its recorded Session in the selected Machine.
pub(crate) fn capture_dir(key: &str) -> StoreResult<PathBuf> {
    resolve_capture(key).map(|(dir, _)| dir)
}

fn resolve_capture(key: &str) -> StoreResult<(PathBuf, crate::session::LfSession)> {
    let home = crate::store::lf_home_dir();
    let dir = record_dir(&home, key)
        .ok_or_else(|| record_error(std::io::Error::other("invalid capture key")))?;
    let store = loopflow_storage::sqlite::SqliteStore::open_read_only(
        &database_in(&home)?,
        crate::store::token_key::encryption_key,
    )?;
    let owner = store.session_for_artifact(key)?.ok_or_else(|| {
        record_error(std::io::Error::other(
            "capture does not belong to a recorded Session in this Machine",
        ))
    })?;
    match read_manifest(&dir) {
        Ok(manifest) if manifest.artifact_key != key => {
            return Err(record_error(std::io::Error::other(
                "capture manifest key does not match",
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(record_error(error)),
    }
    Ok((dir, owner))
}

/// Where an input's capture lives here: a collected input's is the copy of
/// its origin's.
pub fn history_record_dir(lf_home: &Path, history: &SessionHistory) -> Option<PathBuf> {
    let home = match &history.origin {
        Some(origin) => crate::machine_records::copies_home(lf_home, &origin.machine_id),
        None => lf_home.to_path_buf(),
    };
    record_dir(&home, history.artifact_key.as_deref()?)
}

// Session captures retain their published on-disk layout; the directory name
// does not make Run an owner. SQLite selects captures by artifact key.
pub fn record_dir(lf_home: &Path, artifact_key: &str) -> Option<PathBuf> {
    parse_artifact_key(artifact_key).ok()?;
    let prefix = artifact_key
        .strip_prefix("run_")
        .unwrap_or(artifact_key)
        .get(..2)?;
    Some(lf_home.join("runs").join(prefix).join(artifact_key))
}

fn prepare_manifest(
    spec: SessionCaptureSpec,
    artifact_key: String,
    caller_artifact_key: Option<String>,
    process: Option<AgentProcessRequest>,
    context: Option<&crate::trace::PreparedTurnContext>,
) -> std::io::Result<(SessionCaptureManifest, Option<Vec<u8>>)> {
    let (runtime_path, runtime_digest) = runtime_identity();
    let context_bytes = context
        .map(|context| {
            serde_json::to_vec_pretty(&SessionContextArtifact {
                schema_version: SCHEMA_VERSION,
                context,
            })
            .map_err(std::io::Error::other)
        })
        .transpose()?;
    let context_ref = context_bytes.as_ref().map(|bytes| SessionContextRef {
        path: "context.json".to_string(),
        content_sha256: hex::encode(Sha256::digest(bytes)),
        bytes: bytes.len() as u64,
    });
    let manifest = SessionCaptureManifest {
        schema_version: SCHEMA_VERSION,
        artifact_key,
        caller_artifact_key,
        created_at: OffsetDateTime::now_utc(),
        harness: spec.harness,
        model: spec.model,
        surface: spec.surface,
        cwd: spec.cwd,
        repo: spec.repo,
        worktree: spec.worktree,
        skill: spec.skill,
        subjects: spec.subjects,
        graph: Some(spec.graph),
        process,
        context: context_ref,
        runtime_path,
        runtime_digest,
        host: gethostname::gethostname().to_string_lossy().into_owned(),
        boot_id: boot_id(),
    };
    Ok((manifest, context_bytes))
}

/// Resume artifact publication only. The caller must still claim the SQL
/// reservation before launching; readable artifacts confer no process authority.
fn reconcile_reserved_manifest(
    home: &Path,
    mut manifest: SessionCaptureManifest,
    context: Option<&[u8]>,
) -> std::io::Result<(SessionCaptureManifest, PathBuf)> {
    let published = record_dir(home, &manifest.artifact_key)
        .expect("Artifact keys always contain a UUID prefix");
    let directory = published
        .parent()
        .expect("Session capture has a prefix directory");
    create_private_dir(directory)?;
    let staging = directory.join(format!(".{}.staging", manifest.artifact_key));
    let dir = if published.try_exists()? {
        &published
    } else {
        &staging
    };
    if !dir.try_exists()? {
        create_private_dir_exclusive(dir)?;
    }
    if dir.join("terminal.json").try_exists()? {
        return Err(std::io::Error::other(format!(
            "reserved Session capture {} already has terminal evidence; retained unchanged",
            manifest.artifact_key
        )));
    }
    let manifest_path = dir.join("manifest.json");
    if manifest_path.try_exists()? {
        let existing = read_manifest(dir)?;
        // Creation time belongs to the first publication attempt. All other
        // immutable inputs, including exact context digest and caller, must match.
        manifest.created_at = existing.created_at;
        if serde_json::to_value(&manifest)? != serde_json::to_value(&existing)? {
            return Err(std::io::Error::other(format!(
                "reserved Session capture {} has different immutable launch inputs at {}",
                manifest.artifact_key,
                dir.display()
            )));
        }
        manifest = existing;
    }
    if let Some(context) = context {
        reconcile_private_file(&dir.join("context.json"), context)?;
    }
    reconcile_private_file(&manifest_path, &serde_json::to_vec_pretty(&manifest)?)?;
    sync_dir(dir)?;
    if dir == &staging {
        fs::rename(&staging, &published)?;
        sync_dir(directory)?;
    }
    Ok((manifest, published))
}

fn reconcile_private_file(path: &Path, expected: &[u8]) -> std::io::Result<()> {
    match write_private_exclusive(path, expected) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if fs::read(path)? == expected {
                Ok(())
            } else {
                Err(std::io::Error::other(format!(
                    "reserved Session capture artifact differs at {}; retained unchanged",
                    path.display()
                )))
            }
        }
        Err(error) => Err(error),
    }
}

fn publish_manifest(
    lf_home: &Path,
    manifest: &SessionCaptureManifest,
    context: Option<&[u8]>,
) -> std::io::Result<PathBuf> {
    let artifact_key = manifest.artifact_key.as_str();
    let published = record_dir(lf_home, &manifest.artifact_key)
        .expect("Artifact keys always contain a UUID prefix");
    let directory = published
        .parent()
        .expect("Session capture always has a prefix directory");
    create_private_dir(directory)?;
    let staging = directory.join(format!(".{artifact_key}.staging"));
    create_private_dir_exclusive(&staging)?;
    if let Some(context) = context {
        write_private_exclusive(&staging.join("context.json"), context)?;
    }
    write_private_exclusive(
        &staging.join("manifest.json"),
        &serde_json::to_vec_pretty(manifest).map_err(std::io::Error::other)?,
    )?;
    sync_dir(&staging)?;
    fs::rename(&staging, &published)?;
    sync_dir(directory)?;
    Ok(published)
}

fn write_terminal(dir: &Path, receipt: TerminalReceipt) -> std::io::Result<TerminalReceipt> {
    let path = dir.join("terminal.json");
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(std::io::Error::other)?;
    match write_private_exclusive(&path, &bytes) {
        Ok(()) => {
            sync_dir(dir)?;
            Ok(receipt)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = fs::read(&path)?;
            let existing = serde_json::from_slice::<TerminalReceipt>(&existing)
                .map_err(std::io::Error::other)?;
            if existing.outcome == receipt.outcome {
                Ok(existing)
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!(
                        "Recorder already settled as {}; refusing {}",
                        existing.outcome, receipt.outcome
                    ),
                ))
            }
        }
        Err(error) => Err(error),
    }
}

fn runtime_identity() -> (Option<PathBuf>, Option<String>) {
    static IDENTITY: OnceLock<(Option<PathBuf>, Option<String>)> = OnceLock::new();
    if let Some(identity) = IDENTITY.get() {
        return identity.clone();
    }
    let path = std::env::current_exe()
        .ok()
        .and_then(|path| fs::canonicalize(path).ok());
    let digest = path.as_ref().and_then(|path| {
        crate::installation::selection_for_current_executable()
            .ok()
            .flatten()
            .and_then(|selection| {
                selection
                    .artifact_set
                    .artifact(&crate::installation::ArtifactRole::Cli)
                    .filter(|artifact| &artifact.path == path)
                    .map(|artifact| artifact.sha256.clone())
            })
    });
    let identity = (path, digest);
    let _ = IDENTITY.set(identity.clone());
    identity
}

fn boot_id() -> Option<String> {
    fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn create_private_dir(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)?;
    set_private_dir_permissions(path)
}

fn create_private_dir_exclusive(path: &Path) -> std::io::Result<()> {
    fs::create_dir(path)?;
    set_private_dir_permissions(path)
}

fn set_private_dir_permissions(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn write_private_exclusive(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(unix)]
fn sync_dir(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_dir(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

pub(crate) fn record_error(error: std::io::Error) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

#[derive(Serialize)]
pub(crate) struct SessionContextArtifact<'a> {
    schema_version: u32,
    context: &'a crate::trace::PreparedTurnContext,
}

/// Saved provider inputs for native continuation and explicit headless replay.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentProcessRequest {
    pub system_prompt: String,
    pub conversation_context: Option<crate::context_block::ContextDelivery>,
    pub task_prompt: String,
    pub skill_invocation: Option<crate::skills::invocation::SkillInvocation>,
    pub output_schema: Option<serde_json::Value>,
    pub agent: String,
    pub account_id: Option<crate::store::ProviderAccountId>,
    pub max_turns: Option<u32>,
    /// The launch's `yolo` setting, under the key recorded requests already carry.
    pub skip_permissions: bool,
    pub chrome: Option<bool>,
    pub codex_service_tier: Option<crate::config::CodexServiceTier>,
}

impl AgentProcessRequest {
    pub(crate) fn codex_initial_input(&self) -> String {
        match &self.skill_invocation {
            Some(skill) => format!("{}\n\n{}", skill.codex_prompt(), self.task_prompt),
            None => self.task_prompt.clone(),
        }
    }

    pub fn from_prepared(
        config: &crate::agent::AgentConfig,
    ) -> Result<Self, crate::error::LoadError> {
        Ok(Self {
            system_prompt: crate::agent::system_prompt_with_structured_replies(config),
            conversation_context: config.conversation_context.clone(),
            task_prompt: config.task_prompt.clone(),
            skill_invocation: config.skill_invocation.clone(),
            output_schema: config.output_schema.clone(),
            agent: config.agent.to_string(),
            account_id: None,
            max_turns: config.max_turns,
            skip_permissions: config.yolo,
            chrome: config.chrome,
            codex_service_tier: config.codex_service_tier,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCaptureManifest {
    pub schema_version: u32,
    pub artifact_key: String,
    pub caller_artifact_key: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub harness: String,
    pub model: Option<String>,
    pub surface: String,
    pub cwd: PathBuf,
    pub repo: Option<PathBuf>,
    pub worktree: Option<PathBuf>,
    pub skill: Option<String>,
    pub subjects: Vec<SubjectAttribution>,
    #[serde(rename = "flow")] // Existing capture manifests remain replayable.
    pub graph: Option<SessionGraphMembership>,
    #[serde(rename = "exec")] // Existing capture manifests remain replayable.
    pub process: Option<AgentProcessRequest>,
    pub context: Option<SessionContextRef>,
    pub runtime_path: Option<PathBuf>,
    pub runtime_digest: Option<String>,
    pub host: String,
    pub boot_id: Option<String>,
}

pub(crate) fn project_input_history(
    session: &loopflow_storage::session::HistoryCapture,
    artifact_key: Option<&str>,
    history: &[loopflow_storage::session::SessionEvent],
    turns: &[loopflow_storage::session::Turn],
    names: (Option<String>, Option<String>),
    origin: Option<SessionOrigin>,
) -> std::io::Result<SessionHistory> {
    let mut facts = Vec::new();
    let mut terminal: Option<TerminalReceipt> = None;
    let mut manifest: Option<SessionCaptureManifest> = None;
    let mut gaps = 0;
    for event in history {
        if event.kind != loopflow_storage::session::SessionEventKind::Observed {
            continue;
        }
        let input = event.payload["input_id"].as_str();
        let source = event.payload["source"].as_str().unwrap_or("");
        let evidence = &event.payload["evidence"];
        if source == "terminal.json" && input == artifact_key {
            match serde_json::from_value(evidence.clone()) {
                Ok(receipt) => terminal = Some(receipt),
                Err(_) => gaps += 1,
            }
        } else if source == "manifest.json" && input == artifact_key {
            match serde_json::from_value(evidence.clone()) {
                Ok(saved) => manifest = Some(saved),
                Err(_) => gaps += 1,
            }
        } else if source.starts_with("events.jsonl:") {
            facts.extend(read_fact(evidence));
        }
    }
    facts.sort_by_key(|fact| fact.seq);
    let providers =
        project_provider_history(session.captured, artifact_key, &facts, turns, history)?;
    let usage = reduce_usage(turns);
    gaps += usage.gaps + usize::from(manifest.is_none());
    let current = session.captured.is_some() && session.current_capture == session.captured;
    let native_turn = turns.first().filter(|_| artifact_key.is_none());
    Ok(SessionHistory {
        origin,
        session_id: session.id.clone(),
        captured: session.captured,
        artifact_key: artifact_key.map(str::to_owned),
        caller_artifact_key: session.caller_artifact_key.clone(),
        task_id: session.task_id.clone(),
        wave_id: session.wave_id.clone(),
        task_identifier: names.1,
        work_source: session.work_source,
        wave_name: names.0,
        providers,
        task_pr_id: manifest
            .as_ref()
            .and_then(|manifest| match &manifest.graph {
                Some(SessionGraphMembership::Step(step)) => step.task_pr_id.clone(),
                _ => None,
            }),
        repo: manifest
            .as_ref()
            .and_then(|m| m.repo.as_ref().map(|p| p.to_string_lossy().into_owned()))
            .or_else(|| native_turn.and_then(|turn| turn.repository.clone()))
            .or_else(|| current.then(|| session.repo.clone()).flatten()),
        worktree: manifest
            .as_ref()
            .map(|m| m.cwd.to_string_lossy().into_owned())
            .or_else(|| current.then(|| session.cwd.to_string_lossy().into_owned())),
        skill: manifest
            .as_ref()
            .map(|m| m.skill.clone())
            .unwrap_or_else(|| {
                native_turn
                    .and_then(|turn| turn.skill.clone())
                    .or_else(|| current.then(|| session.skill.clone()).flatten())
            }),
        skill_version: session.skill_version.clone(),
        recorded_outcome: terminal.as_ref().map(|receipt| receipt.outcome.clone()),
        recorded_at: terminal
            .as_ref()
            .map(|receipt| receipt.ended_at.unix_timestamp()),
        observed_at: session.observed_at,
        first_provider_attempt_at: providers_first_start(turns)
            .into_iter()
            .chain(facts.iter().find_map(|fact| {
                matches!(fact.fact, CaptureFact::ProviderAttemptStarted { .. })
                    .then(|| fact.observed_at.unix_timestamp())
            }))
            .min(),
        usage,
        evidence_gaps: gaps,
        harness: manifest
            .as_ref()
            .map(|m| m.harness.clone())
            .or_else(|| native_turn.and_then(|turn| turn.provider.clone()))
            .or_else(|| current.then(|| session.provider.clone()).flatten())
            .unwrap_or_else(|| "unknown".into()),
        model: manifest
            .as_ref()
            .map(|m| m.model.clone())
            .unwrap_or_else(|| current.then(|| session.model.clone()).flatten()),
        surface: manifest
            .as_ref()
            .map(|m| m.surface.clone())
            .unwrap_or_else(|| {
                if current && session.interactive {
                    "interactive"
                } else if current {
                    "headless"
                } else {
                    "unknown"
                }
                .into()
            }),
    })
}
