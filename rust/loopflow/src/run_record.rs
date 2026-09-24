//! Authoritative, Home-local evidence for one Loopflow harness launch.

pub mod active;

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::chat::types::{ConversationEvent, ConversationItem, Lifecycle, TurnUsage};
use crate::durable::{RunId, RUN_ID_ENV};
use crate::engine::stream::{ResultSubtype, StreamEvent};
use crate::store::{StoreError, StoreResult};

pub const RUN_DIR_ENV: &str = "LF_RUN_DIR";
pub const PARENT_RUN_ID_ENV: &str = "LF_PARENT_RUN_ID";
pub(crate) const PROVIDER_ACCOUNT_ID_ENV: &str = "LF_PROVIDER_ACCOUNT_ID";

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub(crate) struct RunSpec {
    pub harness: String,
    pub model: Option<String>,
    pub surface: String,
    pub cwd: PathBuf,
    pub repo: Option<PathBuf>,
    pub worktree: Option<PathBuf>,
    pub skill: Option<String>,
    pub subjects: Vec<SubjectAttribution>,
}

/// Replayable, provider-facing inputs for one ordinary headless launch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunLaunchRequest {
    pub system_prompt: String,
    pub task_prompt: String,
    pub agent: String,
    pub account_id: Option<crate::store::ProviderAccountId>,
    pub max_turns: Option<u32>,
    pub write_scope: crate::engine::AgentWriteScope,
    pub execution_boundary: Option<crate::engine::AgentExecutionBoundary>,
    pub skip_permissions: bool,
    pub chrome: bool,
}

impl RunLaunchRequest {
    pub(crate) fn from_prepared(
        config: &crate::engine::AgentConfig,
        capabilities: &crate::engine::AgentCapabilities,
    ) -> Self {
        Self {
            system_prompt: crate::engine::agent::system_prompt_with_structured_replies(config),
            task_prompt: config.task_prompt.clone(),
            agent: config.agent().to_string(),
            account_id: config.provider_account_id.clone(),
            max_turns: config.max_turns,
            write_scope: config.write_scope,
            execution_boundary: config.execution_boundary.clone(),
            skip_permissions: config.skip_permissions,
            chrome: capabilities.chrome,
        }
    }

    pub(crate) fn replay_unavailable_reason(&self) -> Option<&'static str> {
        let (harness, _) = crate::engine::parse_agent(&self.agent);
        matches!(harness.as_str(), "claude" | "codex")
            .then_some("managed Claude/Codex replay requires a recorded account ID")
            .filter(|_| self.account_id.is_none())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubjectAttribution {
    pub selector: String,
    pub source: AttributionSource,
}

impl SubjectAttribution {
    pub(crate) fn declared(selector: String) -> Self {
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
pub struct RunContextRef {
    pub path: String,
    pub content_sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunManifest {
    pub schema_version: u32,
    pub run_id: RunId,
    pub parent_run_id: Option<RunId>,
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
    pub launch: Option<RunLaunchRequest>,
    pub context: Option<RunContextRef>,
    pub runtime_path: Option<PathBuf>,
    pub runtime_digest: Option<String>,
    pub host: String,
    pub boot_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalReceipt {
    pub schema_version: u32,
    pub outcome: String,
    #[serde(with = "time::serde::rfc3339")]
    pub ended_at: OffsetDateTime,
    pub result_ref: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EventEnvelope {
    schema_version: u32,
    seq: u64,
    #[serde(with = "time::serde::rfc3339")]
    observed_at: OffsetDateTime,
    #[serde(flatten)]
    event: RunEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ProviderSessionRef {
    schema_version: u32,
    pub(crate) provider_session_id: String,
    pub(crate) account_id: Option<crate::store::ProviderAccountId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ProviderClientRef {
    schema_version: u32,
    pub(crate) pid: u32,
    pub(crate) terminal_id: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub(crate) started_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProviderClientStopReason {
    Moved,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ProviderClientStop {
    schema_version: u32,
    reason: ProviderClientStopReason,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SessionResolution {
    schema_version: u32,
    #[serde(with = "time::serde::rfc3339")]
    resolved_at: OffsetDateTime,
}

#[derive(Serialize)]
struct RunContextArtifact<'a> {
    schema_version: u32,
    context: &'a crate::trace::PreparedTurnContext,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RunEvent {
    ProviderAttemptStarted {
        provider: String,
        model: Option<String>,
        account_id: Option<crate::store::ProviderAccountId>,
        attempt_key: String,
    },
    ProviderAttemptFinished {
        attempt_key: String,
        outcome: String,
    },
    ProviderSessionObserved {
        attempt_key: String,
        provider_session_id: String,
    },
    Handoff {
        surface: String,
    },
    Usage {
        usage_stream_id: String,
        provider: String,
        model: Option<String>,
        attempt_key: String,
        turn_key: String,
        observation_seq: u64,
        counter_kind: String,
        start_known: bool,
        final_receipt: bool,
        usage: Box<TurnUsage>,
    },
    UserInput {
        op: String,
        text: String,
    },
    Conversation {
        event: Box<ConversationEvent>,
    },
    Text {
        text: String,
    },
    ToolUse {
        name: String,
        summary: String,
    },
    Result {
        outcome: String,
        duration_secs: Option<f64>,
    },
    ProviderOutput {
        stream: String,
        line: String,
    },
    #[serde(other)]
    Unknown,
}

/// Provider-authored cumulative usage reduced once per independent stream.
///
/// Optional counters stay unknown when no stream reported them. Finality is a
/// count of direct provider receipts; Run settlement never upgrades it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunUsage {
    pub streams: usize,
    pub final_streams: usize,
    pub gaps: usize,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_input_tokens: Option<i64>,
    pub peak_input_tokens: Option<i64>,
    pub context_window_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub cost_usd: Option<f64>,
}

impl RunUsage {
    pub(crate) fn empty() -> Self {
        Self {
            streams: 0,
            final_streams: 0,
            gaps: 0,
            input_tokens: None,
            output_tokens: None,
            total_input_tokens: None,
            peak_input_tokens: None,
            context_window_tokens: None,
            reasoning_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            cost_usd: None,
        }
    }
}

/// Disposable projection of one Run's manifest and recorded evidence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunSnapshot {
    pub id: String,
    pub parent_run_id: Option<String>,
    pub repo: Option<String>,
    pub worktree: Option<String>,
    pub subjects: Vec<SubjectAttribution>,
    pub skill: Option<String>,
    pub outcome: Option<String>,
    pub started: i64,
    pub ended: Option<i64>,
    pub usage: RunUsage,
    pub evidence_gaps: usize,
    pub harness: String,
    pub model: Option<String>,
    pub surface: String,
}

impl RunSnapshot {
    pub fn label(&self) -> &str {
        self.skill.as_deref().unwrap_or(&self.harness)
    }

    pub fn status(&self) -> &str {
        self.outcome.as_deref().unwrap_or("unterminated")
    }

    pub fn subject(&self, kind: &str) -> Option<&str> {
        let prefix = format!("{kind}:");
        self.subjects
            .iter()
            .find_map(|subject| subject.selector.strip_prefix(&prefix))
    }

    pub fn total_tokens(&self) -> Option<i64> {
        self.usage
            .input_tokens
            .zip(self.usage.output_tokens)
            .and_then(|(input, output)| input.checked_add(output))
    }

    pub fn is_unterminated(&self) -> bool {
        self.outcome.is_none()
    }
}

#[derive(Debug)]
enum RecorderMessage {
    Event(EventEnvelope),
    Drain(mpsc::Sender<()>),
}

#[derive(Debug)]
struct RunRecorder {
    sender: Option<SyncSender<RecorderMessage>>,
}

impl RunRecorder {
    fn start(dir: &Path, run_id: &RunId) -> Self {
        let (sender, receiver) = mpsc::sync_channel(256);
        let writer_dir = dir.to_path_buf();
        let writer_run_id = run_id.clone();
        let thread = std::thread::Builder::new()
            .name(format!("lf-run-recorder-{}", &run_id.as_str()[..8]))
            .spawn(move || {
                let mut warned = false;
                while let Ok(message) = receiver.recv() {
                    let result = match message {
                        RecorderMessage::Event(event) => {
                            append_json_line(&writer_dir.join("events.jsonl"), &event)
                        }
                        RecorderMessage::Drain(acknowledge) => {
                            let result = sync_telemetry(&writer_dir);
                            let _ = acknowledge.send(());
                            result
                        }
                    };
                    if let Err(error) = result {
                        if !warned {
                            tracing::warn!(
                                %error,
                                run_id = %writer_run_id,
                                "Run recorder lost telemetry; harness execution continues"
                            );
                            warned = true;
                        } else {
                            tracing::debug!(%error, run_id = %writer_run_id, "Run recorder telemetry write failed");
                        }
                    }
                }
            });
        match thread {
            Ok(_) => Self {
                sender: Some(sender),
            },
            Err(error) => {
                tracing::warn!(
                    %error,
                    run_id = %run_id,
                    "Run recorder unavailable; harness execution continues"
                );
                Self { sender: None }
            }
        }
    }

    fn record(&self, message: RecorderMessage) -> std::io::Result<()> {
        let Some(sender) = &self.sender else {
            return Err(std::io::Error::other("Run recorder is unavailable"));
        };
        sender.try_send(message).map_err(|error| match error {
            TrySendError::Full(_) => {
                std::io::Error::new(std::io::ErrorKind::WouldBlock, "Run recorder queue is full")
            }
            TrySendError::Disconnected(_) => {
                std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Run recorder stopped")
            }
        })
    }

    fn drain_after_settlement(&self) {
        let Some(sender) = &self.sender else {
            return;
        };
        let (acknowledge, drained) = mpsc::channel();
        if sender.try_send(RecorderMessage::Drain(acknowledge)).is_ok() {
            let _ = drained.recv_timeout(std::time::Duration::from_millis(250));
        }
    }
}

/// Scan Home-local Run records without opening planning or journal SQLite.
///
/// A corrupt individual record is omitted with a warning so one damaged
/// record cannot make unrelated execution history unavailable. Partial JSONL
/// evidence remains visible through `evidence_gaps` on the owning Run.
pub fn scan_runs_since(lf_home: &Path, since: i64) -> std::io::Result<Vec<RunSnapshot>> {
    let records = record_dirs(lf_home)?;
    let mut runs = Vec::new();
    for record in records {
        match read_run_snapshot(&record) {
            Ok(run) if run.started >= since => runs.push(run),
            Ok(_) => {}
            Err(error) => tracing::warn!(
                %error,
                record = %record.display(),
                "invalid Run record omitted"
            ),
        }
    }
    runs.sort_by(|left, right| {
        right
            .started
            .cmp(&left.started)
            .then_with(|| right.id.cmp(&left.id))
    });
    Ok(runs)
}

/// Read unresolved native provider Sessions without reducing every Run's events.
pub(crate) fn scan_unresolved_provider_runs(
    lf_home: &Path,
) -> std::io::Result<Vec<(PathBuf, RunManifest)>> {
    let mut runs = Vec::new();
    for dir in record_dirs(lf_home)? {
        let manifest = match read_manifest(&dir).and_then(|manifest| {
            validate_manifest_path(&dir, &manifest)?;
            Ok(manifest)
        }) {
            Ok(manifest) => manifest,
            Err(error) => {
                tracing::warn!(
                    %error,
                    record = %dir.display(),
                    "invalid Run record omitted from Sessions"
                );
                continue;
            }
        };
        if !has_interactive_history(&dir, &manifest)? {
            continue;
        }
        let unresolved = match provider_session_is_resolved(&dir) {
            Ok(resolved) => !resolved,
            Err(error) => {
                tracing::warn!(
                    %error,
                    run_id = %manifest.run_id,
                    "invalid Session resolution omitted"
                );
                continue;
            }
        };
        if !unresolved {
            continue;
        }
        match read_provider_session(&dir) {
            Ok(Some(_)) => runs.push((dir, manifest)),
            Ok(None) => {}
            Err(error) => tracing::warn!(
                %error,
                run_id = %manifest.run_id,
                "invalid provider Session omitted"
            ),
        }
    }
    runs.sort_by(|(_, left), (_, right)| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.run_id.as_str().cmp(left.run_id.as_str()))
    });
    Ok(runs)
}

fn record_dirs(lf_home: &Path) -> std::io::Result<Vec<PathBuf>> {
    let root = lf_home.join("runs");
    let prefixes = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut records = Vec::new();
    for prefix in prefixes {
        let prefix = prefix?;
        if !prefix.file_type()?.is_dir() {
            continue;
        }
        let entries = fs::read_dir(prefix.path())?;
        for record in entries {
            let record = record?;
            if record.file_name().to_string_lossy().starts_with('.')
                || !record.file_type()?.is_dir()
            {
                continue;
            }
            records.push(record.path());
        }
    }
    Ok(records)
}

pub(crate) fn resolve_manifest(
    lf_home: &Path,
    selector: &str,
) -> std::io::Result<(PathBuf, RunManifest)> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Run id cannot be empty",
        ));
    }
    let mut matches = record_dirs(lf_home)?
        .into_iter()
        .filter(|dir| {
            let id = dir.file_name().and_then(|name| name.to_str()).unwrap_or("");
            id.starts_with(selector)
                || id
                    .strip_prefix("run_")
                    .is_some_and(|id| id.starts_with(selector))
        })
        .collect::<Vec<_>>();
    matches.sort();
    match matches.as_slice() {
        [] => Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Run {selector} was not found on this Home"),
        )),
        [dir] => {
            let manifest = read_manifest(dir)?;
            validate_manifest_path(dir, &manifest)?;
            Ok((dir.clone(), manifest))
        }
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Run prefix {selector} is ambiguous"),
        )),
    }
}

pub(crate) fn read_run_snapshot(dir: &Path) -> std::io::Result<RunSnapshot> {
    let manifest = read_manifest(dir)?;
    validate_manifest_path(dir, &manifest)?;
    let mut evidence_gaps = usize::from(
        !dir.join("prepared").is_file() && !context_ref_is_valid(dir, manifest.context.as_ref()),
    );
    let terminal = match fs::read(dir.join("terminal.json")) {
        Ok(bytes) => match serde_json::from_slice::<TerminalReceipt>(&bytes) {
            Ok(receipt)
                if receipt.schema_version == SCHEMA_VERSION
                    && matches!(
                        receipt.outcome.as_str(),
                        "completed" | "failed" | "interrupted"
                    ) =>
            {
                Some(receipt)
            }
            Ok(_) | Err(_) => {
                evidence_gaps += 1;
                None
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => {
            evidence_gaps += 1;
            None
        }
    };
    let (usage, event_gaps) = reduce_usage(&dir.join("events.jsonl"))?;
    evidence_gaps += event_gaps;

    Ok(RunSnapshot {
        id: manifest.run_id.to_string(),
        parent_run_id: manifest.parent_run_id.map(|id| id.to_string()),
        repo: manifest
            .repo
            .map(|path| path.to_string_lossy().into_owned()),
        worktree: manifest
            .worktree
            .map(|path| path.to_string_lossy().into_owned()),
        subjects: manifest.subjects,
        skill: manifest.skill,
        outcome: terminal.as_ref().map(|receipt| receipt.outcome.clone()),
        started: manifest.created_at.unix_timestamp(),
        ended: terminal.map(|receipt| receipt.ended_at.unix_timestamp()),
        usage,
        evidence_gaps,
        harness: manifest.harness,
        model: manifest.model,
        surface: manifest.surface,
    })
}

pub(crate) fn read_provider_session(dir: &Path) -> std::io::Result<Option<ProviderSessionRef>> {
    match fs::read(dir.join("provider-session.json")) {
        Ok(bytes) => {
            let session: ProviderSessionRef =
                serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
            if session.schema_version != SCHEMA_VERSION || session.provider_session_id.is_empty() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "invalid provider session reference",
                ));
            }
            return Ok(Some(session));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    let file = match File::open(dir.join("events.jsonl")) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut provider_session = None;
    for line in BufReader::new(file).lines() {
        let envelope: EventEnvelope =
            serde_json::from_str(&line?).map_err(std::io::Error::other)?;
        if envelope.schema_version != SCHEMA_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unsupported Run event schema",
            ));
        }
        if let RunEvent::ProviderSessionObserved {
            provider_session_id: observed,
            ..
        } = envelope.event
        {
            provider_session = Some(ProviderSessionRef {
                schema_version: SCHEMA_VERSION,
                provider_session_id: observed,
                account_id: None,
            });
        }
    }
    Ok(provider_session)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FinalAnswer {
    pub text: String,
    pub exact: bool,
}

#[derive(Default)]
struct TurnProse {
    tagged: Option<String>,
    untagged: Option<String>,
    streamed: String,
}

impl TurnProse {
    fn answer(self) -> Option<FinalAnswer> {
        self.tagged
            .map(|text| FinalAnswer { text, exact: true })
            .or_else(|| self.untagged.map(|text| FinalAnswer { text, exact: true }))
            .or_else(|| {
                (!self.streamed.is_empty()).then_some(FinalAnswer {
                    text: self.streamed,
                    exact: false,
                })
            })
    }
}

/// Read the last completed provider conclusion without exposing raw event shape.
pub(crate) fn read_final_answer(dir: &Path) -> std::io::Result<Option<FinalAnswer>> {
    let file = match File::open(dir.join("events.jsonl")) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut turns = HashMap::<String, TurnProse>::new();
    let mut answer = None;
    for line in BufReader::new(file).lines() {
        let envelope: EventEnvelope =
            serde_json::from_str(&line?).map_err(std::io::Error::other)?;
        if envelope.schema_version != SCHEMA_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unsupported Run event schema",
            ));
        }
        let RunEvent::Conversation { event } = envelope.event else {
            continue;
        };
        match *event {
            ConversationEvent::TextDelta { turn_id, content } => {
                turns
                    .entry(turn_id)
                    .or_default()
                    .streamed
                    .push_str(&content);
            }
            ConversationEvent::ItemCompleted {
                turn_id,
                item: ConversationItem::Message { text, phase, .. },
            } => match phase.as_deref() {
                Some("final_answer") => turns.entry(turn_id).or_default().tagged = Some(text),
                None => turns.entry(turn_id).or_default().untagged = Some(text),
                Some(_) => {}
            },
            ConversationEvent::TurnCompleted { turn_id, status } => {
                let prose = turns.remove(&turn_id).unwrap_or_default();
                if status == Lifecycle::Completed {
                    answer = prose.answer().or(answer);
                }
            }
            _ => {}
        }
    }
    Ok(answer)
}

pub(crate) fn write_provider_session(
    dir: &Path,
    provider_session_id: &str,
    account_id: Option<crate::store::ProviderAccountId>,
) -> std::io::Result<()> {
    if provider_session_id.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "provider session id cannot be empty",
        ));
    }
    read_manifest(dir)?;
    let path = dir.join("provider-session.json");
    let staging = dir.join(format!(".provider-session-{}.staging", Uuid::new_v4()));
    write_private_exclusive(
        &staging,
        &serde_json::to_vec_pretty(&ProviderSessionRef {
            schema_version: SCHEMA_VERSION,
            provider_session_id: provider_session_id.to_string(),
            account_id,
        })
        .map_err(std::io::Error::other)?,
    )?;
    fs::rename(staging, path)?;
    sync_dir(dir)
}

pub(crate) fn read_provider_clients(dir: &Path) -> std::io::Result<Vec<ProviderClientRef>> {
    let root = dir.join("provider-clients");
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut clients = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let client: ProviderClientRef =
            serde_json::from_slice(&fs::read(entry.path())?).map_err(std::io::Error::other)?;
        if client.schema_version != SCHEMA_VERSION || client.pid <= 1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid provider client reference",
            ));
        }
        clients.push(client);
    }
    clients.sort_by_key(|client| client.pid);
    Ok(clients)
}

pub(crate) fn write_provider_client(dir: &Path, pid: u32) -> std::io::Result<()> {
    if pid <= 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "provider client pid must identify a child process",
        ));
    }
    read_manifest(dir)?;
    remove_provider_client_stop(dir, pid)?;
    let root = dir.join("provider-clients");
    fs::create_dir_all(&root)?;
    let path = root.join(format!("{pid}.json"));
    let staging = root.join(format!(".{pid}-{}.staging", Uuid::new_v4()));
    write_private_exclusive(
        &staging,
        &serde_json::to_vec_pretty(&ProviderClientRef {
            schema_version: SCHEMA_VERSION,
            pid,
            terminal_id: current_terminal_id(),
            started_at: OffsetDateTime::now_utc(),
        })
        .map_err(std::io::Error::other)?,
    )?;
    fs::rename(staging, path)?;
    sync_dir(&root)
}

pub(crate) fn provider_client_matches(
    client: &ProviderClientRef,
    harness: &str,
    pid: u32,
    started_at: i64,
    command: &str,
) -> bool {
    pid == client.pid
        && (started_at - client.started_at.unix_timestamp()).abs() <= 5
        && command.split_whitespace().any(|word| {
            Path::new(word)
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == harness || name.starts_with(&format!("{harness}-")))
                || word.contains(&format!("/{harness}"))
        })
}

/// A terminal marker is valid only on the PTY where the shell installed it.
/// Inherited environment after app, SSH, or background handoff is not attachment.
fn current_terminal_id() -> Option<String> {
    let id = std::env::var("LF_TERMINAL_ID")
        .ok()
        .filter(|id| !id.is_empty())?;
    let expected = std::env::var("LF_TERMINAL_TTY").ok()?;
    let mut name = [0 as libc::c_char; 1024];
    // SAFETY: name is a writable buffer of the supplied length. ttyname_r
    // writes a NUL-terminated name on success and retains no pointers.
    let result = unsafe { libc::ttyname_r(libc::STDIN_FILENO, name.as_mut_ptr(), name.len()) };
    if result != 0 {
        return None;
    }
    // SAFETY: successful ttyname_r above terminated the buffer.
    let actual = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }
        .to_str()
        .ok()?;
    (actual == expected).then_some(id)
}

pub(crate) fn read_provider_client_stop(
    dir: &Path,
    pid: u32,
) -> std::io::Result<Option<ProviderClientStopReason>> {
    let path = dir
        .join("provider-client-stops")
        .join(format!("{pid}.json"));
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let stop: ProviderClientStop = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
    if stop.schema_version != SCHEMA_VERSION {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid provider client stop",
        ));
    }
    Ok(Some(stop.reason))
}

pub(crate) fn write_provider_client_stop(
    dir: &Path,
    pid: u32,
    reason: ProviderClientStopReason,
) -> std::io::Result<()> {
    if pid <= 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "provider client pid must identify a child process",
        ));
    }
    read_manifest(dir)?;
    let root = dir.join("provider-client-stops");
    fs::create_dir_all(&root)?;
    let path = root.join(format!("{pid}.json"));
    let staging = root.join(format!(".{pid}-{}.staging", Uuid::new_v4()));
    write_private_exclusive(
        &staging,
        &serde_json::to_vec_pretty(&ProviderClientStop {
            schema_version: SCHEMA_VERSION,
            reason,
        })
        .map_err(std::io::Error::other)?,
    )?;
    fs::rename(staging, path)?;
    sync_dir(&root)
}

pub(crate) fn remove_provider_client_stop(dir: &Path, pid: u32) -> std::io::Result<()> {
    let root = dir.join("provider-client-stops");
    let path = root.join(format!("{pid}.json"));
    match fs::remove_file(path) {
        Ok(()) => sync_dir(&root),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// The client directory survives its last client: it records that an initially
/// headless Run was opened interactively. Client files alone describe liveness.
pub(crate) fn has_interactive_history(dir: &Path, manifest: &RunManifest) -> std::io::Result<bool> {
    Ok(manifest.surface == "tui" || dir.join("provider-clients").try_exists()?)
}

pub(crate) fn remove_provider_client(dir: &Path, pid: u32) -> std::io::Result<()> {
    let path = dir.join("provider-clients").join(format!("{pid}.json"));
    match fs::remove_file(path) {
        Ok(()) => sync_dir(&dir.join("provider-clients")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub(crate) fn provider_session_is_resolved(dir: &Path) -> std::io::Result<bool> {
    match fs::read(dir.join("session-resolution.json")) {
        Ok(bytes) => {
            let resolution: SessionResolution =
                serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
            if resolution.schema_version != SCHEMA_VERSION {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "unsupported session resolution schema",
                ));
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(crate) fn resolve_provider_session(dir: &Path) -> std::io::Result<()> {
    read_manifest(dir)?;
    if provider_session_is_resolved(dir)? {
        return Ok(());
    }
    let path = dir.join("session-resolution.json");
    let staging = dir.join(format!(".session-resolution-{}.staging", Uuid::new_v4()));
    write_private_exclusive(
        &staging,
        &serde_json::to_vec_pretty(&SessionResolution {
            schema_version: SCHEMA_VERSION,
            resolved_at: OffsetDateTime::now_utc(),
        })
        .map_err(std::io::Error::other)?,
    )?;
    fs::rename(staging, path)?;
    sync_dir(dir)
}

fn context_ref_is_valid(dir: &Path, context: Option<&RunContextRef>) -> bool {
    let Some(context) = context else {
        return true;
    };
    if context.path != "context.json" {
        return false;
    }
    let Ok(bytes) = fs::read(dir.join(&context.path)) else {
        return false;
    };
    bytes.len() as u64 == context.bytes
        && hex::encode(Sha256::digest(&bytes)) == context.content_sha256
}

fn validate_manifest_path(dir: &Path, manifest: &RunManifest) -> std::io::Result<()> {
    RunId::parse(manifest.run_id.as_str()).map_err(std::io::Error::other)?;
    if dir.file_name().and_then(|name| name.to_str()) != Some(manifest.run_id.as_str())
        || dir
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            != manifest
                .run_id
                .as_str()
                .strip_prefix("run_")
                .and_then(|id| id.get(..2))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Run manifest identity does not match its record path",
        ));
    }
    Ok(())
}

#[derive(Debug, Default)]
struct UsageStream {
    observation_seq: Option<u64>,
    final_receipt: bool,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    total_input_tokens: Option<u64>,
    peak_input_tokens: Option<u64>,
    context_window_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    cost_usd: Option<f64>,
}

pub(crate) fn read_manifest(dir: &Path) -> std::io::Result<RunManifest> {
    let bytes = fs::read(dir.join("manifest.json"))?;
    let manifest = serde_json::from_slice::<RunManifest>(&bytes).map_err(std::io::Error::other)?;
    if manifest.schema_version != SCHEMA_VERSION {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "unsupported Run manifest schema {}; expected {SCHEMA_VERSION}",
                manifest.schema_version
            ),
        ));
    }
    Ok(manifest)
}

fn reduce_usage(path: &Path) -> std::io::Result<(RunUsage, usize)> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((RunUsage::empty(), 0));
        }
        Err(error) => return Err(error),
    };
    let mut streams = BTreeMap::<String, UsageStream>::new();
    let mut gaps = 0;
    let mut envelope_seq = None;
    let mut reader = BufReader::new(file);
    loop {
        let mut line = Vec::new();
        let read = reader.read_until(b'\n', &mut line)?;
        if read == 0 {
            break;
        }
        let complete = line.last() == Some(&b'\n');
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        if !complete {
            gaps += 1;
            break;
        }
        let envelope = match serde_json::from_slice::<EventEnvelope>(&line) {
            Ok(envelope) => envelope,
            Err(error) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("malformed complete Run event: {error}"),
                ));
            }
        };
        if envelope.schema_version != SCHEMA_VERSION {
            gaps += 1;
        }
        if envelope_seq.is_some_and(|previous| envelope.seq <= previous) {
            gaps += 1;
        }
        envelope_seq = Some(envelope_seq.map_or(envelope.seq, |seen| seen.max(envelope.seq)));
        let (usage_stream_id, observation_seq, counter_kind, start_known, final_receipt, usage) =
            match envelope.event {
                RunEvent::Usage {
                    usage_stream_id,
                    observation_seq,
                    counter_kind,
                    start_known,
                    final_receipt,
                    usage,
                    ..
                } => (
                    usage_stream_id,
                    observation_seq,
                    counter_kind,
                    start_known,
                    final_receipt,
                    usage,
                ),
                RunEvent::Unknown => {
                    gaps += 1;
                    continue;
                }
                _ => continue,
            };
        if counter_kind != "cumulative" {
            gaps += 1;
            continue;
        }
        if !start_known {
            gaps += 1;
        }
        let stream = streams.entry(usage_stream_id).or_default();
        if stream
            .observation_seq
            .is_some_and(|previous| observation_seq <= previous)
        {
            gaps += 1;
        }
        stream.observation_seq = Some(
            stream
                .observation_seq
                .map_or(observation_seq, |seen| seen.max(observation_seq)),
        );
        stream.final_receipt |= final_receipt;
        observe_u64(&mut stream.input_tokens, usage.input_tokens, &mut gaps);
        observe_u64(&mut stream.output_tokens, usage.output_tokens, &mut gaps);
        observe_u64(
            &mut stream.total_input_tokens,
            usage.total_input_tokens,
            &mut gaps,
        );
        observe_u64(
            &mut stream.peak_input_tokens,
            usage.peak_input_tokens,
            &mut gaps,
        );
        observe_u64(
            &mut stream.context_window_tokens,
            usage.context_window_tokens,
            &mut gaps,
        );
        observe_u64(
            &mut stream.reasoning_tokens,
            usage.reasoning_tokens,
            &mut gaps,
        );
        observe_u64(
            &mut stream.cache_read_tokens,
            usage.cache_read_tokens,
            &mut gaps,
        );
        observe_u64(
            &mut stream.cache_write_tokens,
            usage.cache_write_tokens,
            &mut gaps,
        );
        observe_f64(&mut stream.cost_usd, usage.cost_usd, &mut gaps);
    }

    let input_tokens = sum_u64(
        streams.values().map(|stream| stream.input_tokens),
        &mut gaps,
    );
    let output_tokens = sum_u64(
        streams.values().map(|stream| stream.output_tokens),
        &mut gaps,
    );
    let total_input_tokens = sum_u64(
        streams.values().map(|stream| stream.total_input_tokens),
        &mut gaps,
    );
    let peak_input_tokens = max_u64(
        streams.values().map(|stream| stream.peak_input_tokens),
        &mut gaps,
    );
    let context_window_tokens = max_u64(
        streams.values().map(|stream| stream.context_window_tokens),
        &mut gaps,
    );
    let reasoning_tokens = sum_u64(
        streams.values().map(|stream| stream.reasoning_tokens),
        &mut gaps,
    );
    let cache_read_tokens = sum_u64(
        streams.values().map(|stream| stream.cache_read_tokens),
        &mut gaps,
    );
    let cache_write_tokens = sum_u64(
        streams.values().map(|stream| stream.cache_write_tokens),
        &mut gaps,
    );
    let cost_usd = sum_f64(streams.values().map(|stream| stream.cost_usd));
    let usage = RunUsage {
        streams: streams.len(),
        final_streams: streams
            .values()
            .filter(|stream| stream.final_receipt)
            .count(),
        gaps,
        input_tokens,
        output_tokens,
        total_input_tokens,
        peak_input_tokens,
        context_window_tokens,
        reasoning_tokens,
        cache_read_tokens,
        cache_write_tokens,
        cost_usd,
    };
    Ok((usage, gaps))
}

fn observe_u64(current: &mut Option<u64>, observed: Option<u64>, gaps: &mut usize) {
    let Some(observed) = observed else {
        return;
    };
    if current.is_some_and(|previous| observed < previous) {
        *gaps += 1;
    }
    *current = Some(current.map_or(observed, |previous| previous.max(observed)));
}

fn observe_f64(current: &mut Option<f64>, observed: Option<f64>, gaps: &mut usize) {
    let Some(observed) = observed else {
        return;
    };
    if !observed.is_finite() {
        *gaps += 1;
        return;
    }
    if current.is_some_and(|previous| observed < previous) {
        *gaps += 1;
    }
    *current = Some(current.map_or(observed, |previous| previous.max(observed)));
}

fn sum_u64(values: impl Iterator<Item = Option<u64>>, gaps: &mut usize) -> Option<i64> {
    let mut total: Option<u64> = None;
    for value in values.flatten() {
        total = match total {
            Some(total) => match total.checked_add(value) {
                Some(total) => Some(total),
                None => {
                    *gaps += 1;
                    return None;
                }
            },
            None => Some(value),
        };
    }
    let total = total?;
    match i64::try_from(total) {
        Ok(total) => Some(total),
        Err(_) => {
            *gaps += 1;
            None
        }
    }
}

fn sum_f64(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    values.flatten().reduce(|total, value| total + value)
}

fn max_u64(values: impl Iterator<Item = Option<u64>>, gaps: &mut usize) -> Option<i64> {
    let value = values.flatten().max()?;
    i64::try_from(value).map_err(|_| *gaps += 1).ok()
}

#[derive(Debug, Clone)]
pub(crate) struct CaptureHandle(Arc<Mutex<RunCapture>>);

impl CaptureHandle {
    /// Publish identity before a human boundary becomes visible. No provider or
    /// terminal receipt exists until this prepared Run is launched.
    pub(crate) fn prepare(spec: RunSpec, parent: Option<RunId>) -> StoreResult<RunId> {
        #[cfg(test)]
        let home = std::env::var_os("LF_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!("loopflow-test-run-home-{}", std::process::id()))
            });
        #[cfg(not(test))]
        let home = crate::store::lf_home_dir();
        Self::prepare_at(&home, spec, parent)
    }

    pub(crate) fn prepare_at(
        home: &Path,
        spec: RunSpec,
        parent: Option<RunId>,
    ) -> StoreResult<RunId> {
        let id = RunId::new();
        let capture =
            RunCapture::begin(home, spec, id.clone(), parent, None, None).map_err(record_error)?;
        write_private_exclusive(&capture.dir.join("prepared"), b"").map_err(record_error)?;
        sync_dir(&capture.dir).map_err(record_error)?;
        // This is not a CaptureHandle: dropping preparation must not settle a
        // Run whose provider has never been launched.
        Ok(id)
    }

    pub(crate) fn start_prepared(
        home: &Path,
        id: &RunId,
        spec: RunSpec,
        context: &crate::trace::PreparedTurnContext,
    ) -> StoreResult<Self> {
        let (dir, mut manifest) = resolve_manifest(home, id.as_str()).map_err(record_error)?;
        // Atomically claim this preparation. A second launcher cannot record
        // another provider attempt into the same Run.
        fs::rename(dir.join("prepared"), dir.join("launching")).map_err(record_error)?;
        let bytes = serde_json::to_vec_pretty(&RunContextArtifact {
            schema_version: SCHEMA_VERSION,
            context,
        })?;
        write_private_exclusive(&dir.join("context.json"), &bytes).map_err(record_error)?;
        manifest.context = Some(RunContextRef {
            path: "context.json".to_string(),
            content_sha256: hex::encode(Sha256::digest(&bytes)),
            bytes: bytes.len() as u64,
        });
        // Finalize launch provenance on the existing identity. Preparation did
        // not freeze a prompt, runtime, or model selection before launch.
        manifest.harness = spec.harness;
        manifest.model = spec.model;
        manifest.surface = spec.surface;
        manifest.cwd = spec.cwd;
        manifest.repo = spec.repo;
        manifest.worktree = spec.worktree;
        manifest.skill = spec.skill;
        manifest.subjects = spec.subjects;
        (manifest.runtime_path, manifest.runtime_digest) = runtime_identity();
        manifest.host = gethostname::gethostname().to_string_lossy().into_owned();
        manifest.boot_id = boot_id();
        let staged = dir.join(".manifest-launching.json");
        write_private_exclusive(&staged, &serde_json::to_vec_pretty(&manifest)?)
            .map_err(record_error)?;
        fs::rename(staged, dir.join("manifest.json")).map_err(record_error)?;
        fs::remove_file(dir.join("launching")).map_err(record_error)?;
        sync_dir(&dir).map_err(record_error)?;
        Ok(Self(Arc::new(Mutex::new(RunCapture::from_manifest(
            manifest, dir,
        )))))
    }

    pub(crate) fn begin_with_launch(spec: RunSpec, launch: RunLaunchRequest) -> StoreResult<Self> {
        let context = crate::trace::PreparedTurnContext::from_prompts(
            &launch.system_prompt,
            &launch.task_prompt,
        );
        Self::begin_with_id_and_parent(spec, RunId::new(), None, true, Some(launch), Some(&context))
    }

    pub(crate) fn begin_with_launch_and_context(
        spec: RunSpec,
        launch: RunLaunchRequest,
        context: &crate::trace::PreparedTurnContext,
    ) -> StoreResult<Self> {
        Self::begin_with_id_and_parent(spec, RunId::new(), None, true, Some(launch), Some(context))
    }

    pub(crate) fn begin_with_context(
        spec: RunSpec,
        context: &crate::trace::PreparedTurnContext,
    ) -> StoreResult<Self> {
        Self::begin_with_id_and_parent(spec, RunId::new(), None, true, None, Some(context))
    }

    pub(crate) fn begin_replay_at(
        lf_home: &Path,
        spec: RunSpec,
        launch: RunLaunchRequest,
        parent_run_id: RunId,
    ) -> StoreResult<Self> {
        let parent_run_id = verified_parent(lf_home, parent_run_id);
        let context = crate::trace::PreparedTurnContext::from_prompts(
            &launch.system_prompt,
            &launch.task_prompt,
        );
        Self::begin_at_with_id(
            lf_home,
            spec,
            RunId::new(),
            parent_run_id,
            Some(launch),
            Some(&context),
        )
    }

    fn begin_with_id_and_parent(
        spec: RunSpec,
        run_id: RunId,
        parent_run_id: Option<RunId>,
        inherit_parent: bool,
        launch: Option<RunLaunchRequest>,
        context: Option<&crate::trace::PreparedTurnContext>,
    ) -> StoreResult<Self> {
        #[cfg(test)]
        let home = std::env::var_os("LF_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!("loopflow-test-run-home-{}", std::process::id()))
            });
        #[cfg(not(test))]
        let home = crate::store::lf_home_dir();
        let parent_run_id = if inherit_parent {
            inherited_parent()
        } else {
            parent_run_id.and_then(|candidate| verified_parent(&home, candidate))
        };
        Self::begin_at_with_id(&home, spec, run_id, parent_run_id, launch, context)
    }

    #[cfg(test)]
    pub(crate) fn begin_at(lf_home: &Path, spec: RunSpec) -> StoreResult<Self> {
        Self::begin_at_with_id(lf_home, spec, RunId::new(), inherited_parent(), None, None)
    }

    #[cfg(test)]
    fn begin_at_with_launch(
        lf_home: &Path,
        spec: RunSpec,
        launch: RunLaunchRequest,
    ) -> StoreResult<Self> {
        let context = crate::trace::PreparedTurnContext::from_prompts(
            &launch.system_prompt,
            &launch.task_prompt,
        );
        Self::begin_at_with_id(
            lf_home,
            spec,
            RunId::new(),
            inherited_parent(),
            Some(launch),
            Some(&context),
        )
    }

    fn begin_at_with_id(
        lf_home: &Path,
        spec: RunSpec,
        run_id: RunId,
        parent_run_id: Option<RunId>,
        launch: Option<RunLaunchRequest>,
        context: Option<&crate::trace::PreparedTurnContext>,
    ) -> StoreResult<Self> {
        let capture = RunCapture::begin(lf_home, spec, run_id, parent_run_id, launch, context)
            .map_err(record_error)?;
        Ok(Self(Arc::new(Mutex::new(capture))))
    }

    pub(crate) fn run_id(&self) -> RunId {
        self.0
            .lock()
            .expect("Run capture mutex poisoned")
            .manifest
            .run_id
            .clone()
    }

    pub(crate) fn artifact_dir(&self) -> PathBuf {
        self.0
            .lock()
            .expect("Run capture mutex poisoned")
            .dir
            .clone()
    }

    pub(crate) fn environment(&self) -> BTreeMap<String, String> {
        let capture = self.0.lock().expect("Run capture mutex poisoned");
        let mut environment = BTreeMap::from([
            (RUN_ID_ENV.to_string(), capture.manifest.run_id.to_string()),
            (RUN_DIR_ENV.to_string(), capture.dir.display().to_string()),
        ]);
        if let Some(parent_run_id) = &capture.manifest.parent_run_id {
            environment.insert(PARENT_RUN_ID_ENV.to_string(), parent_run_id.to_string());
        }
        environment
    }

    pub(crate) fn mark_spawn_requested(&self) {
        self.with_capture(RunCapture::start_attempt);
    }

    pub(crate) fn mark_handoff(&self, surface: &str) {
        self.with_capture(|capture| {
            capture.append_event(RunEvent::Handoff {
                surface: surface.to_string(),
            })
        });
    }

    pub(crate) fn record_raw(&self, stream: &str, line: &str) {
        self.with_capture(|capture| capture.record_raw(stream, line));
    }

    pub(crate) fn record_stream_event(&self, event: &StreamEvent) {
        self.with_capture(|capture| capture.record_stream_event(event));
    }

    pub(crate) fn record_conversation(&self, event: ConversationEvent) {
        self.with_capture(|capture| capture.record_conversation(event));
    }

    pub(crate) fn record_input(&self, op: &str, text: &str) {
        self.with_capture(|capture| {
            capture.append_event(RunEvent::UserInput {
                op: op.to_string(),
                text: text.to_string(),
            })
        });
    }

    pub(crate) fn fail_and_begin_attempt(
        &self,
        provider: String,
        model: Option<String>,
        account_id: Option<crate::store::ProviderAccountId>,
    ) {
        self.with_capture(|capture| capture.fail_and_begin_attempt(provider, model, account_id));
    }

    pub(crate) fn set_provider_session_id(&self, session_id: Option<String>) {
        let Some(session_id) = session_id else {
            return;
        };
        self.with_capture(|capture| {
            let account_id = read_provider_session(&capture.dir)?
                .and_then(|session| session.account_id)
                .or_else(|| {
                    capture
                        .manifest
                        .launch
                        .as_ref()
                        .and_then(|launch| launch.account_id.clone())
                });
            write_provider_session(&capture.dir, &session_id, account_id)?;
            capture.append_event(RunEvent::ProviderSessionObserved {
                attempt_key: capture.attempt_key(),
                provider_session_id: session_id,
            })
        });
    }

    pub(crate) fn finish(&self, outcome: &str) -> StoreResult<()> {
        self.0
            .lock()
            .expect("Run capture mutex poisoned")
            .finish(outcome)
            .map_err(record_error)
    }

    fn with_capture(&self, operation: impl FnOnce(&mut RunCapture) -> std::io::Result<()>) {
        let mut capture = self.0.lock().expect("Run capture mutex poisoned");
        if let Err(error) = operation(&mut capture) {
            capture.warn_telemetry(error);
        }
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        if Arc::strong_count(&self.0) != 1 {
            return;
        }
        let Ok(mut capture) = self.0.lock() else {
            tracing::warn!("Run capture was poisoned before terminal settlement");
            return;
        };
        if capture.settled_outcome.is_some() {
            return;
        }
        if let Err(error) = capture.finish("failed") {
            tracing::warn!(
                %error,
                run_id = %capture.manifest.run_id,
                "failed to settle dropped Run capture"
            );
        }
    }
}

#[derive(Debug)]
struct RunCapture {
    binding: Option<active::RunBindingGuard>,
    manifest: RunManifest,
    dir: PathBuf,
    provider: String,
    model: Option<String>,
    account_id: Option<crate::store::ProviderAccountId>,
    attempt: u32,
    attempt_started: bool,
    turn_key: String,
    usage_stream_id: String,
    event_seq: u64,
    usage_seq: u64,
    recorder: RunRecorder,
    telemetry_warned: bool,
    settled_outcome: Option<String>,
}

impl RunCapture {
    fn begin(
        lf_home: &Path,
        spec: RunSpec,
        run_id: RunId,
        parent_run_id: Option<RunId>,
        launch: Option<RunLaunchRequest>,
        context: Option<&crate::trace::PreparedTurnContext>,
    ) -> std::io::Result<Self> {
        let (runtime_path, runtime_digest) = runtime_identity();
        let context_bytes = context
            .map(|context| {
                serde_json::to_vec_pretty(&RunContextArtifact {
                    schema_version: SCHEMA_VERSION,
                    context,
                })
                .map_err(std::io::Error::other)
            })
            .transpose()?;
        let context_ref = context_bytes.as_ref().map(|bytes| RunContextRef {
            path: "context.json".to_string(),
            content_sha256: hex::encode(Sha256::digest(bytes)),
            bytes: bytes.len() as u64,
        });
        let manifest = RunManifest {
            schema_version: SCHEMA_VERSION,
            run_id: run_id.clone(),
            parent_run_id,
            created_at: OffsetDateTime::now_utc(),
            harness: spec.harness.clone(),
            model: spec.model.clone(),
            surface: spec.surface,
            cwd: spec.cwd,
            repo: spec.repo,
            worktree: spec.worktree,
            skill: spec.skill,
            subjects: spec.subjects,
            launch,
            context: context_ref,
            runtime_path,
            runtime_digest,
            host: gethostname::gethostname().to_string_lossy().into_owned(),
            boot_id: boot_id(),
        };
        let dir = publish_manifest(lf_home, &manifest, context_bytes.as_deref())?;
        Ok(Self::from_manifest(manifest, dir))
    }

    fn from_manifest(manifest: RunManifest, dir: PathBuf) -> Self {
        let recorder = RunRecorder::start(&dir, &manifest.run_id);
        Self {
            binding: None,
            provider: manifest.harness.clone(),
            model: manifest.model.clone(),
            account_id: manifest
                .launch
                .as_ref()
                .and_then(|launch| launch.account_id.clone()),
            manifest,
            dir,
            attempt: 1,
            attempt_started: false,
            turn_key: Uuid::new_v4().to_string(),
            usage_stream_id: Uuid::new_v4().to_string(),
            event_seq: 0,
            usage_seq: 0,
            recorder,
            telemetry_warned: false,
            settled_outcome: None,
        }
    }

    fn attempt_key(&self) -> String {
        format!("attempt-{}", self.attempt)
    }

    fn start_attempt(&mut self) -> std::io::Result<()> {
        if self.attempt_started {
            return Ok(());
        }
        if self.binding.is_none() {
            self.binding = Some(active::RunBindingGuard::publish(&self.dir)?);
        }
        self.attempt_started = true;
        self.append_event(RunEvent::ProviderAttemptStarted {
            provider: self.provider.clone(),
            model: self.model.clone(),
            account_id: self.account_id.clone(),
            attempt_key: self.attempt_key(),
        })
    }

    fn fail_and_begin_attempt(
        &mut self,
        provider: String,
        model: Option<String>,
        account_id: Option<crate::store::ProviderAccountId>,
    ) -> std::io::Result<()> {
        let finish_error = if self.attempt_started {
            self.append_event(RunEvent::ProviderAttemptFinished {
                attempt_key: self.attempt_key(),
                outcome: "failed".to_string(),
            })
            .err()
        } else {
            None
        };
        self.provider = provider;
        self.model = model;
        self.account_id = account_id;
        self.attempt += 1;
        self.attempt_started = false;
        self.turn_key = Uuid::new_v4().to_string();
        self.usage_stream_id = Uuid::new_v4().to_string();
        self.usage_seq = 0;
        let start_error = self.start_attempt().err();
        match (finish_error, start_error) {
            (None, None) => Ok(()),
            (Some(error), None) | (None, Some(error)) => Err(error),
            (Some(finish), Some(start)) => Err(std::io::Error::other(format!(
                "failed to record prior attempt outcome: {finish}; failed to record new attempt: {start}"
            ))),
        }
    }

    fn record_raw(&mut self, stream: &str, line: &str) -> std::io::Result<()> {
        self.append_event(RunEvent::ProviderOutput {
            stream: stream.to_string(),
            line: line.to_string(),
        })
    }

    fn record_stream_event(&mut self, event: &StreamEvent) -> std::io::Result<()> {
        match event {
            StreamEvent::Text(text) => self.append_event(RunEvent::Text { text: text.clone() }),
            StreamEvent::ToolUse { name, summary } => self.append_event(RunEvent::ToolUse {
                name: name.clone(),
                summary: summary.clone(),
            }),
            StreamEvent::Usage {
                input_tokens,
                output_tokens,
                cache_read_tokens,
            } => self.append_usage(
                TurnUsage {
                    input_tokens: *input_tokens,
                    output_tokens: *output_tokens,
                    cache_read_tokens: *cache_read_tokens,
                    ..TurnUsage::default()
                },
                false,
            ),
            StreamEvent::Result {
                subtype,
                cost_usd,
                duration_secs,
            } => {
                if cost_usd.is_some() {
                    self.append_usage(
                        TurnUsage {
                            cost_usd: *cost_usd,
                            ..TurnUsage::default()
                        },
                        true,
                    )?;
                }
                self.append_event(RunEvent::Result {
                    outcome: match subtype {
                        ResultSubtype::Success => "completed",
                        ResultSubtype::Error => "failed",
                    }
                    .to_string(),
                    duration_secs: *duration_secs,
                })
            }
        }
    }

    fn record_conversation(&mut self, event: ConversationEvent) -> std::io::Result<()> {
        match event {
            ConversationEvent::TurnStarted { turn_id } => {
                self.turn_key = turn_id.clone();
                self.usage_stream_id = Uuid::new_v4().to_string();
                self.usage_seq = 0;
                self.append_event(RunEvent::Conversation {
                    event: Box::new(ConversationEvent::TurnStarted { turn_id }),
                })
            }
            ConversationEvent::UsageCheckpoint {
                turn_id,
                usage,
                final_receipt,
            } => {
                self.turn_key = turn_id;
                self.append_usage(usage, final_receipt)
            }
            event => self.append_event(RunEvent::Conversation {
                event: Box::new(event),
            }),
        }
    }

    fn append_usage(&mut self, usage: TurnUsage, final_receipt: bool) -> std::io::Result<()> {
        if !usage.is_reported() {
            return Ok(());
        }
        self.usage_seq += 1;
        self.append_event(RunEvent::Usage {
            usage_stream_id: self.usage_stream_id.clone(),
            provider: self.provider.clone(),
            model: self.model.clone(),
            attempt_key: self.attempt_key(),
            turn_key: self.turn_key.clone(),
            observation_seq: self.usage_seq,
            counter_kind: "cumulative".to_string(),
            start_known: true,
            final_receipt,
            usage: Box::new(usage),
        })?;
        Ok(())
    }

    fn finish(&mut self, outcome: &str) -> std::io::Result<()> {
        if !matches!(outcome, "completed" | "failed" | "interrupted") {
            return Err(std::io::Error::other(format!(
                "invalid Run outcome: {outcome}"
            )));
        }
        if let Some(settled) = &self.settled_outcome {
            if settled == outcome {
                return Ok(());
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("Run already settled as {settled}; refusing {outcome}"),
            ));
        }
        write_terminal(
            &self.dir,
            &TerminalReceipt {
                schema_version: SCHEMA_VERSION,
                outcome: outcome.to_string(),
                ended_at: OffsetDateTime::now_utc(),
                result_ref: None,
            },
        )?;
        self.settled_outcome = Some(outcome.to_string());
        self.binding = None;
        if self.attempt_started {
            if let Err(error) = self.append_event(RunEvent::ProviderAttemptFinished {
                attempt_key: self.attempt_key(),
                outcome: outcome.to_string(),
            }) {
                tracing::warn!(
                    %error,
                    run_id = %self.manifest.run_id,
                    "final Run lifecycle event unavailable"
                );
            }
        }
        self.recorder.drain_after_settlement();
        Ok(())
    }

    fn append_event(&mut self, event: RunEvent) -> std::io::Result<()> {
        let envelope = EventEnvelope {
            schema_version: SCHEMA_VERSION,
            seq: self.event_seq,
            observed_at: OffsetDateTime::now_utc(),
            event,
        };
        self.event_seq += 1;
        self.recorder.record(RecorderMessage::Event(envelope))
    }

    fn warn_telemetry(&mut self, error: std::io::Error) {
        if self.telemetry_warned {
            tracing::debug!(%error, run_id = %self.manifest.run_id, "Run telemetry write failed");
            return;
        }
        self.telemetry_warned = true;
        tracing::warn!(
            %error,
            run_id = %self.manifest.run_id,
            "Run telemetry write failed; harness execution continues"
        );
    }
}

fn inherited_parent() -> Option<RunId> {
    let run_id = std::env::var(RUN_ID_ENV).ok()?;
    let run_dir = PathBuf::from(std::env::var_os(RUN_DIR_ENV)?);
    let manifest = fs::read(run_dir.join("manifest.json")).ok()?;
    let manifest = serde_json::from_slice::<RunManifest>(&manifest).ok()?;
    (manifest.run_id.as_str() == run_id).then_some(manifest.run_id)
}

fn verified_parent(lf_home: &Path, run_id: RunId) -> Option<RunId> {
    let dir = record_dir(lf_home, &run_id)?;
    let manifest = fs::read(dir.join("manifest.json")).ok()?;
    let manifest = serde_json::from_slice::<RunManifest>(&manifest).ok()?;
    (manifest.run_id == run_id).then_some(run_id)
}

fn record_dir(lf_home: &Path, run_id: &RunId) -> Option<PathBuf> {
    let prefix = run_id.as_str().strip_prefix("run_")?.get(..2)?;
    Some(lf_home.join("runs").join(prefix).join(run_id.as_str()))
}

#[cfg(test)]
pub(crate) fn observed_run_ids(selectors: &[String]) -> std::io::Result<Vec<RunId>> {
    observed_run_ids_at(&crate::store::lf_home_dir(), selectors)
}

#[cfg(test)]
fn observed_run_ids_at(lf_home: &Path, selectors: &[String]) -> std::io::Result<Vec<RunId>> {
    let root = lf_home.join("runs");
    let mut prefixes = match fs::read_dir(&root) {
        Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    prefixes.sort_by_key(|entry| entry.path());

    let mut observed = Vec::new();
    for prefix in prefixes {
        let Ok(entries) = fs::read_dir(prefix.path()) else {
            continue;
        };
        let mut runs = entries.filter_map(Result::ok).collect::<Vec<_>>();
        runs.sort_by_key(|entry| entry.path());
        for run in runs {
            let Ok(bytes) = fs::read(run.path().join("manifest.json")) else {
                continue;
            };
            let Ok(manifest) = serde_json::from_slice::<RunManifest>(&bytes) else {
                continue;
            };
            if manifest.subjects.iter().any(|subject| {
                selectors
                    .iter()
                    .any(|selector| selector == &subject.selector)
            }) {
                observed.push((manifest.created_at, manifest.run_id));
            }
        }
    }
    observed.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.as_str().cmp(right.1.as_str()))
    });
    observed.dedup_by(|left, right| left.1 == right.1);
    Ok(observed.into_iter().map(|(_, run_id)| run_id).collect())
}

fn publish_manifest(
    lf_home: &Path,
    manifest: &RunManifest,
    context: Option<&[u8]>,
) -> std::io::Result<PathBuf> {
    let run_id = manifest.run_id.as_str();
    let published =
        record_dir(lf_home, &manifest.run_id).expect("Run ids always contain a UUID prefix");
    let parent = published
        .parent()
        .expect("Run record always has a prefix directory");
    create_private_dir(parent)?;
    let staging = parent.join(format!(".{run_id}.staging"));
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
    sync_dir(parent)?;
    Ok(published)
}

fn write_terminal(dir: &Path, receipt: &TerminalReceipt) -> std::io::Result<()> {
    let path = dir.join("terminal.json");
    let bytes = serde_json::to_vec_pretty(receipt).map_err(std::io::Error::other)?;
    match write_private_exclusive(&path, &bytes) {
        Ok(()) => sync_dir(dir),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = fs::read(&path)?;
            let existing = serde_json::from_slice::<TerminalReceipt>(&existing)
                .map_err(std::io::Error::other)?;
            if existing.outcome == receipt.outcome {
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!(
                        "Run already settled as {}; refusing {}",
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
        crate::machine_install::selection_for_current_executable()
            .ok()
            .flatten()
            .and_then(|selection| {
                selection
                    .artifact_set
                    .artifact(&crate::machine_install::ArtifactRole::Cli)
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

fn append_json_line<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    bytes.push(b'\n');
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path)?;
    file.write_all(&bytes)
}

fn sync_telemetry(dir: &Path) -> std::io::Result<()> {
    let path = dir.join("events.jsonl");
    match OpenOptions::new().read(true).open(&path) {
        Ok(file) => file.sync_data(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn sync_dir(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_dir(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn record_error(error: std::io::Error) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

pub(crate) async fn attributed_work(
    store: &crate::store::SharedStore,
    manifest: &RunManifest,
) -> Option<crate::durable::WorkRef> {
    let selector = preferred_work_selector(manifest)?;
    let (kind, id) = selector.split_once(':')?;
    let exact = match kind {
        "task" => crate::work::task::TaskId::parse(id)
            .ok()
            .map(crate::durable::WorkRef::Task),
        "project" => crate::durable::ProjectId::parse(id)
            .ok()
            .map(crate::durable::WorkRef::Project),
        "wave" => crate::id::WaveId::parse(id)
            .ok()
            .map(crate::durable::WorkRef::Wave),
        _ => None,
    };
    if exact.is_some() {
        return exact;
    }
    // Historical Project subjects remain readable after Project launch selectors
    // are removed. Resolve stored identity without reopening a planning workflow.
    if kind == "project" {
        return match store.get_project_by_project(id).await {
            Ok(Some(project)) => Some(crate::durable::WorkRef::Project(project.id)),
            Ok(None) => {
                tracing::warn!(%selector, run_id = %manifest.run_id, "Historical Run subject unavailable");
                None
            }
            Err(error) => {
                tracing::warn!(%error, %selector, run_id = %manifest.run_id, "Historical Run subject unavailable");
                None
            }
        };
    }
    match crate::ops::resolve_work_binding(store, &manifest.cwd, &selector).await {
        Ok(binding) => Some(binding.work),
        Err(error) => {
            tracing::warn!(%error, %selector, run_id = %manifest.run_id, "Run subject unavailable");
            None
        }
    }
}

pub(crate) fn preferred_work_selector(manifest: &RunManifest) -> Option<String> {
    manifest
        .subjects
        .iter()
        .filter_map(|subject| {
            let rank = if subject.selector.starts_with("task:") {
                3
            } else if subject.selector.starts_with("project:") {
                2
            } else if subject.selector.starts_with("wave:") {
                1
            } else {
                return None;
            };
            Some((rank, subject.selector.clone()))
        })
        .max_by_key(|(rank, _)| *rank)
        .map(|(_, selector)| selector)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs::{self, OpenOptions};
    use std::io::Write;

    use super::{
        observed_run_ids_at, provider_session_is_resolved, read_final_answer,
        read_provider_clients, read_provider_session, read_run_snapshot, remove_provider_client,
        resolve_provider_session, scan_runs_since, scan_unresolved_provider_runs,
        write_provider_client, write_provider_session, CaptureHandle, RunLaunchRequest,
        RunManifest, RunSpec, SubjectAttribution, TerminalReceipt,
    };
    use crate::chat::types::{ConversationEvent, ConversationItem, TurnUsage};
    use crate::engine::stream::{ResultSubtype, StreamEvent};
    use crate::engine::{AgentCapabilities, AgentConfig};

    #[test]
    fn terminal_attachment_probe() {
        let Ok(expected) = std::env::var("LF_TEST_TERMINAL_ATTACHMENT") else {
            return;
        };
        assert_eq!(
            super::current_terminal_id().as_deref(),
            (!expected.is_empty()).then_some(expected.as_str())
        );
    }

    #[test]
    fn terminal_attachment_requires_the_original_pty() {
        use std::os::fd::FromRawFd;
        use std::process::{Command, Stdio};

        let mut master = -1;
        let mut slave = -1;
        let mut name = [0 as libc::c_char; 1024];
        // SAFETY: all output pointers are valid; null termios/winsize use defaults.
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    name.as_mut_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        // SAFETY: successful openpty returns two independently owned descriptors
        // and a NUL-terminated device name in the buffer.
        let (_master, slave, tty) = unsafe {
            (
                fs::File::from_raw_fd(master),
                fs::File::from_raw_fd(slave),
                std::ffi::CStr::from_ptr(name.as_ptr())
                    .to_str()
                    .unwrap()
                    .to_owned(),
            )
        };
        for (expected_tty, attached) in [(tty.as_str(), true), ("/dev/another-terminal", false)] {
            for _ in 0..2 {
                let output = Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "run_record::tests::terminal_attachment_probe",
                        "--nocapture",
                    ])
                    .env("LF_TERMINAL_ID", "shell-one")
                    .env("LF_TERMINAL_TTY", expected_tty)
                    .env(
                        "LF_TEST_TERMINAL_ATTACHMENT",
                        if attached { "shell-one" } else { "" },
                    )
                    .stdin(Stdio::from(slave.try_clone().unwrap()))
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stdout)
                );
            }
        }
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "run_record::tests::terminal_attachment_probe"])
            .env("LF_TERMINAL_ID", "shell-one")
            .env("LF_TERMINAL_TTY", &tty)
            .env("LF_TEST_TERMINAL_ATTACHMENT", "")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(output.status.success());
    }

    fn spec(cwd: &std::path::Path) -> RunSpec {
        RunSpec {
            harness: "proof".to_string(),
            model: Some("model".to_string()),
            surface: "headless".to_string(),
            cwd: cwd.to_path_buf(),
            repo: Some(cwd.to_path_buf()),
            worktree: Some(cwd.to_path_buf()),
            skill: Some("implement".to_string()),
            subjects: Vec::new(),
        }
    }

    #[test]
    fn prepared_session_run_is_resolvable_and_consumed_once() {
        let home = tempfile::tempdir().unwrap();
        let parent = crate::durable::RunId::new();
        let id = CaptureHandle::prepare_at(home.path(), spec(home.path()), Some(parent.clone()))
            .unwrap();
        let (dir, prepared) = super::resolve_manifest(home.path(), id.as_str()).unwrap();
        let snapshot = super::read_run_snapshot(&dir).unwrap();
        assert_eq!(snapshot.id, id.as_str());
        assert_eq!(snapshot.parent_run_id.as_deref(), Some(parent.as_str()));
        assert_eq!(snapshot.outcome, None);
        assert_eq!(snapshot.evidence_gaps, 0);
        assert!(!dir.join("terminal.json").exists());
        assert!(!dir.join("provider-clients").exists());
        let context = crate::trace::PreparedTurnContext::from_prompts("system", "human prompt");
        let capture =
            CaptureHandle::start_prepared(home.path(), &id, spec(home.path()), &context).unwrap();
        let launched = super::read_manifest(&dir).unwrap();
        assert_eq!(capture.run_id(), id);
        assert_eq!(launched.created_at, prepared.created_at);
        assert_eq!(launched.parent_run_id, Some(parent));
        assert!(super::context_ref_is_valid(&dir, launched.context.as_ref()));
        assert!(
            CaptureHandle::start_prepared(home.path(), &id, spec(home.path()), &context).is_err()
        );
        capture.mark_spawn_requested();
        capture.finish("completed").unwrap();
        assert_eq!(
            super::read_run_snapshot(&dir).unwrap().outcome.as_deref(),
            Some("completed")
        );
        assert_eq!(super::record_dirs(home.path()).unwrap().len(), 1);
    }

    #[test]
    fn manifest_round_trips_the_exact_prepared_launch_without_ambient_authority() {
        let home = tempfile::tempdir().unwrap();
        let config = AgentConfig {
            system_prompt: "system context\r\nwith unicode λ\n \t".to_string(),
            task_prompt: "authored task\n\nwith trailing space ".to_string(),
            agent: Some("codex:gpt-5.6".to_string()),
            provider_account_id: Some(
                crate::store::ProviderAccountId::parse("engineering").unwrap(),
            ),
            max_turns: Some(7),
            resume_token: Some("resume-secret".to_string()),
            skip_permissions: true,
            directive_relay: Some("/tmp/directive-secret".into()),
            env: BTreeMap::from([("TOKEN".to_string(), "ambient-secret".to_string())]),
            ..AgentConfig::default()
        };
        let expected =
            RunLaunchRequest::from_prepared(&config, &AgentCapabilities { chrome: true });
        let capture =
            CaptureHandle::begin_at_with_launch(home.path(), spec(home.path()), expected.clone())
                .unwrap();

        let bytes = fs::read(capture.artifact_dir().join("manifest.json")).unwrap();
        let manifest: RunManifest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(manifest.launch, Some(expected));
        let context_ref = manifest.context.expect("manifest references exact context");
        let context = fs::read(capture.artifact_dir().join(&context_ref.path)).unwrap();
        assert_eq!(context_ref.bytes, context.len() as u64);
        assert_eq!(
            context_ref.content_sha256,
            hex::encode(<sha2::Sha256 as sha2::Digest>::digest(&context))
        );
        let context: serde_json::Value = serde_json::from_slice(&context).unwrap();
        assert_eq!(
            context.pointer("/context/system/text").unwrap(),
            "system context\r\nwith unicode λ\n \t"
        );
        assert_eq!(
            context.pointer("/context/task/text").unwrap(),
            "authored task\n\nwith trailing space "
        );
        assert_eq!(
            read_run_snapshot(&capture.artifact_dir())
                .unwrap()
                .evidence_gaps,
            0
        );
        fs::write(
            capture.artifact_dir().join(&context_ref.path),
            b"tampered context",
        )
        .unwrap();
        assert_eq!(
            read_run_snapshot(&capture.artifact_dir())
                .unwrap()
                .evidence_gaps,
            1
        );
        assert!(bytes
            .windows(b"engineering".len())
            .any(|window| window == b"engineering"));
        assert!(!bytes
            .windows(b"resume-secret".len())
            .any(|window| window == b"resume-secret"));
        assert!(!bytes
            .windows(b"ambient-secret".len())
            .any(|window| window == b"ambient-secret"));
        assert!(!bytes
            .windows(b"directive-secret".len())
            .any(|window| window == b"directive-secret"));
    }

    #[test]
    fn record_keeps_direct_usage_and_one_immutable_terminal_without_an_owner_claim() {
        let home = tempfile::tempdir().unwrap();
        let capture =
            CaptureHandle::begin_at(home.path(), spec(home.path())).expect("publish Run manifest");
        capture.record_input("initial", "do the work");
        let dir = capture.artifact_dir();
        let run_id = capture.run_id().to_string();

        assert!(dir.join("manifest.json").is_file());
        assert_eq!(
            dir.parent().and_then(|path| path.file_name()),
            Some(std::ffi::OsStr::new(&run_id[4..6]))
        );
        assert!(!dir.join("inbox").exists());
        assert!(!dir.join("observations").exists());
        assert!(!dir.join("owner.json").exists());
        assert!(!dir.join("terminal.json").exists());

        capture.mark_spawn_requested();
        capture.record_raw("stderr", "provider diagnostic");
        capture.record_stream_event(&StreamEvent::Text("provider response".to_string()));
        capture.record_stream_event(&StreamEvent::ToolUse {
            name: "shell".to_string(),
            summary: "inspect files".to_string(),
        });
        capture.record_conversation(ConversationEvent::UsageCheckpoint {
            turn_id: "provider-turn".to_string(),
            usage: TurnUsage {
                input_tokens: Some(10),
                output_tokens: Some(4),
                ..TurnUsage::default()
            },
            final_receipt: false,
        });
        capture.finish("completed").expect("settle Run");

        let events = fs::read_to_string(dir.join("events.jsonl")).unwrap();
        assert!(events.contains("\"type\":\"usage\""));
        assert!(events.contains("\"counter_kind\":\"cumulative\""));
        assert!(events.contains("\"final_receipt\":false"));
        assert!(!events.contains("\"final_receipt\":true"));
        assert!(events.contains("\"type\":\"user_input\""));
        assert!(events.contains("\"type\":\"provider_output\""));
        assert!(events.contains("\"type\":\"text\""));
        assert!(events.contains("\"type\":\"tool_use\""));
        let terminal = fs::read(dir.join("terminal.json")).unwrap();
        let terminal: TerminalReceipt = serde_json::from_slice(&terminal).unwrap();
        assert_eq!(terminal.outcome, "completed");
        let mut files = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        files.sort();
        assert_eq!(
            files,
            ["events.jsonl", "manifest.json", "terminal.json"].map(std::ffi::OsString::from)
        );

        let error = capture.finish("failed").unwrap_err();
        assert!(error.to_string().contains("already settled as completed"));
        let unchanged: TerminalReceipt =
            serde_json::from_slice(&fs::read(dir.join("terminal.json")).unwrap()).unwrap();
        assert_eq!(unchanged.outcome, "completed");
    }

    #[test]
    fn final_answer_reader_returns_the_conclusion_without_commentary() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        capture.record_conversation(ConversationEvent::ItemCompleted {
            turn_id: "turn-1".to_string(),
            item: ConversationItem::Message {
                id: "commentary".to_string(),
                text: "still working".to_string(),
                phase: Some("commentary".to_string()),
            },
        });
        capture.record_conversation(ConversationEvent::ItemCompleted {
            turn_id: "turn-1".to_string(),
            item: ConversationItem::Message {
                id: "answer".to_string(),
                text: "final report".to_string(),
                phase: Some("final_answer".to_string()),
            },
        });
        capture.record_conversation(ConversationEvent::TurnCompleted {
            turn_id: "turn-1".to_string(),
            status: crate::chat::types::Lifecycle::Completed,
        });
        capture.finish("completed").unwrap();

        assert_eq!(
            read_final_answer(&capture.artifact_dir()).unwrap(),
            Some(super::FinalAnswer {
                text: "final report".to_string(),
                exact: true,
            })
        );
    }

    #[test]
    fn final_answer_reader_recovers_legacy_streamed_prose_honestly() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        capture.record_conversation(ConversationEvent::TurnStarted {
            turn_id: "turn-1".to_string(),
        });
        capture.record_conversation(ConversationEvent::TextDelta {
            turn_id: "turn-1".to_string(),
            content: "working\n".to_string(),
        });
        capture.record_conversation(ConversationEvent::TextDelta {
            turn_id: "turn-1".to_string(),
            content: "final report".to_string(),
        });
        capture.record_conversation(ConversationEvent::TurnCompleted {
            turn_id: "turn-1".to_string(),
            status: crate::chat::types::Lifecycle::Completed,
        });
        capture.finish("completed").unwrap();

        assert_eq!(
            read_final_answer(&capture.artifact_dir()).unwrap(),
            Some(super::FinalAnswer {
                text: "working\nfinal report".to_string(),
                exact: false,
            })
        );
    }

    #[test]
    fn provider_session_identity_is_durable_before_the_provider_starts() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        capture.set_provider_session_id(Some("provider-session".to_string()));

        assert_eq!(
            read_provider_session(&capture.artifact_dir())
                .unwrap()
                .map(|session| session.provider_session_id),
            Some("provider-session".to_string())
        );
    }

    #[test]
    fn provider_session_preserves_the_selected_account() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        let account_id = crate::store::ProviderAccountId::parse("primary").unwrap();
        super::write_provider_session(
            &capture.artifact_dir(),
            "provider-session",
            Some(account_id.clone()),
        )
        .unwrap();

        let session = read_provider_session(&capture.artifact_dir())
            .unwrap()
            .expect("provider session reference");
        assert_eq!(session.provider_session_id, "provider-session");
        assert_eq!(session.account_id, Some(account_id));
    }

    #[test]
    fn provider_clients_are_independent_and_remove_exactly() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        let dir = capture.artifact_dir();

        write_provider_client(&dir, 101).unwrap();
        write_provider_client(&dir, 202).unwrap();
        assert_eq!(
            read_provider_clients(&dir)
                .unwrap()
                .into_iter()
                .map(|client| client.pid)
                .collect::<Vec<_>>(),
            [101, 202]
        );

        remove_provider_client(&dir, 101).unwrap();
        assert_eq!(read_provider_clients(&dir).unwrap()[0].pid, 202);
    }

    #[test]
    fn resolving_a_session_keeps_its_provider_history() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        let dir = capture.artifact_dir();
        write_provider_session(&dir, "provider-session", None).unwrap();

        assert!(!provider_session_is_resolved(&dir).unwrap());
        resolve_provider_session(&dir).unwrap();

        assert!(provider_session_is_resolved(&dir).unwrap());
        assert_eq!(
            read_provider_session(&dir)
                .unwrap()
                .expect("provider history remains")
                .provider_session_id,
            "provider-session"
        );
    }

    #[test]
    fn unresolved_provider_scan_keeps_interactive_resumes_until_resolution() {
        let home = tempfile::tempdir().unwrap();

        let mut open_spec = spec(home.path());
        open_spec.surface = "tui".to_string();
        let open = CaptureHandle::begin_at(home.path(), open_spec).unwrap();
        write_provider_session(&open.artifact_dir(), "open-session", None).unwrap();

        let mut resolved_spec = spec(home.path());
        resolved_spec.surface = "tui".to_string();
        let resolved = CaptureHandle::begin_at(home.path(), resolved_spec).unwrap();
        write_provider_session(&resolved.artifact_dir(), "resolved-session", None).unwrap();
        resolve_provider_session(&resolved.artifact_dir()).unwrap();

        let headless = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        write_provider_session(&headless.artifact_dir(), "headless-session", None).unwrap();

        let runs = scan_unresolved_provider_runs(home.path()).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].1.run_id, open.run_id());

        let dir = headless.artifact_dir();
        write_provider_client(&dir, 101).unwrap();
        let resumed = scan_unresolved_provider_runs(home.path()).unwrap();
        assert!(resumed
            .iter()
            .any(|(_, manifest)| manifest.run_id == headless.run_id()));
        remove_provider_client(&dir, 101).unwrap();
        let closed = scan_unresolved_provider_runs(home.path()).unwrap();
        assert!(closed
            .iter()
            .any(|(_, manifest)| manifest.run_id == headless.run_id()));
        resolve_provider_session(&dir).unwrap();
        let resolved = scan_unresolved_provider_runs(home.path()).unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].1.run_id, open.run_id());
    }

    #[test]
    fn two_runs_about_one_task_keep_distinct_identity_and_shared_provenance() {
        let home = tempfile::tempdir().unwrap();
        let selector = "task:LOO-267".to_string();
        let mut first = spec(home.path());
        first.skill = Some("research".to_string());
        first.subjects = vec![SubjectAttribution::declared(selector.clone())];
        let second = first.clone();

        let first = CaptureHandle::begin_at(home.path(), first).unwrap();
        let second = CaptureHandle::begin_at(home.path(), second).unwrap();
        let first_id = first.run_id();
        let second_id = second.run_id();

        assert_ne!(first_id, second_id);
        let observed = observed_run_ids_at(home.path(), &[selector]).unwrap();
        assert_eq!(observed.len(), 2);
        assert!(observed.contains(&first_id));
        assert!(observed.contains(&second_id));
        assert!(!first.artifact_dir().join("terminal.json").exists());
        assert!(!second.artifact_dir().join("terminal.json").exists());
    }

    #[test]
    fn telemetry_failure_does_not_gate_terminal_settlement() {
        let home = tempfile::tempdir().unwrap();
        let capture =
            CaptureHandle::begin_at(home.path(), spec(home.path())).expect("publish Run manifest");
        let dir = capture.artifact_dir();
        fs::create_dir(dir.join("events.jsonl")).unwrap();

        capture.mark_spawn_requested();
        capture.record_conversation(ConversationEvent::UsageCheckpoint {
            turn_id: "provider-turn".to_string(),
            usage: TurnUsage {
                input_tokens: Some(10),
                ..TurnUsage::default()
            },
            final_receipt: true,
        });
        capture
            .finish("completed")
            .expect("settle without telemetry");

        assert!(dir.join("terminal.json").is_file());
    }

    #[test]
    fn dropping_the_last_capture_settles_unexpected_control_flow_as_failed() {
        let home = tempfile::tempdir().unwrap();
        let capture =
            CaptureHandle::begin_at(home.path(), spec(home.path())).expect("publish Run manifest");
        let dir = capture.artifact_dir();

        drop(capture);

        let terminal: TerminalReceipt =
            serde_json::from_slice(&fs::read(dir.join("terminal.json")).unwrap()).unwrap();
        assert_eq!(terminal.outcome, "failed");
    }

    #[test]
    fn retry_usage_keeps_provider_cumulative_values_in_distinct_streams() {
        let home = tempfile::tempdir().unwrap();
        let capture =
            CaptureHandle::begin_at(home.path(), spec(home.path())).expect("publish Run manifest");
        capture.record_input("initial", "do the work");
        let dir = capture.artifact_dir();

        capture.mark_spawn_requested();
        capture.record_stream_event(&StreamEvent::Usage {
            input_tokens: Some(10),
            output_tokens: Some(4),
            cache_read_tokens: None,
        });
        capture.fail_and_begin_attempt(
            "proof-fallback".to_string(),
            None,
            Some(crate::store::ProviderAccountId::parse("fallback-account").unwrap()),
        );
        capture.record_stream_event(&StreamEvent::Usage {
            input_tokens: Some(12),
            output_tokens: Some(5),
            cache_read_tokens: None,
        });
        capture.finish("completed").expect("settle Run");

        let events = fs::read_to_string(dir.join("events.jsonl")).unwrap();
        assert!(events.contains("\"account_id\":\"fallback-account\""));
        let usage = events
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter(|event| event["type"] == "usage")
            .collect::<Vec<_>>();
        assert_eq!(usage.len(), 2);
        assert_eq!(usage[0]["usage"]["input_tokens"], 10);
        assert_eq!(usage[1]["usage"]["input_tokens"], 12);
        assert_ne!(usage[0]["usage_stream_id"], usage[1]["usage_stream_id"]);
        assert_eq!(usage[0]["observation_seq"], 1);
        assert_eq!(usage[1]["observation_seq"], 1);
        assert_eq!(usage[0]["final_receipt"], false);
        assert_eq!(usage[1]["final_receipt"], false);
    }

    #[test]
    fn telemetry_loss_cannot_keep_retry_evidence_on_the_prior_attempt() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        let prior_stream = {
            let mut state = capture.0.lock().unwrap();
            let stream = state.usage_stream_id.clone();
            state.recorder.sender = None;
            stream
        };

        capture.mark_spawn_requested();
        capture.fail_and_begin_attempt("fallback".to_string(), Some("next".to_string()), None);

        let state = capture.0.lock().unwrap();
        assert_eq!(state.attempt, 2);
        assert_eq!(state.provider, "fallback");
        assert_eq!(state.model.as_deref(), Some("next"));
        assert!(state.attempt_started);
        assert_ne!(state.usage_stream_id, prior_stream);
    }

    #[test]
    fn usage_keeps_omissions_and_resets_sequence_for_each_provider_turn() {
        let home = tempfile::tempdir().unwrap();
        let capture =
            CaptureHandle::begin_at(home.path(), spec(home.path())).expect("publish Run manifest");
        let dir = capture.artifact_dir();

        capture.record_stream_event(&StreamEvent::Usage {
            input_tokens: Some(10),
            output_tokens: None,
            cache_read_tokens: None,
        });
        capture.record_stream_event(&StreamEvent::Usage {
            input_tokens: None,
            output_tokens: Some(4),
            cache_read_tokens: None,
        });
        capture.record_conversation(ConversationEvent::TurnStarted {
            turn_id: "provider-turn-2".to_string(),
        });
        capture.record_stream_event(&StreamEvent::Usage {
            input_tokens: Some(3),
            output_tokens: None,
            cache_read_tokens: None,
        });
        capture.finish("completed").unwrap();

        let usage = fs::read_to_string(dir.join("events.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter(|event| event["type"] == "usage")
            .collect::<Vec<_>>();
        assert_eq!(usage.len(), 3);
        assert_eq!(usage[1]["usage"]["input_tokens"], serde_json::Value::Null);
        assert_eq!(
            usage[1]["usage"]["total_input_tokens"],
            serde_json::Value::Null
        );
        assert_ne!(usage[1]["usage_stream_id"], usage[2]["usage_stream_id"]);
        assert_eq!(usage[1]["observation_seq"], 2);
        assert_eq!(usage[2]["observation_seq"], 1);
    }

    #[test]
    fn scanner_reduces_each_cumulative_stream_once_and_keeps_provider_finality() {
        let home = tempfile::tempdir().unwrap();
        let mut run_spec = spec(home.path());
        run_spec.subjects = vec![SubjectAttribution::declared("task:LOO-265".to_string())];
        let capture = CaptureHandle::begin_at(home.path(), run_spec).unwrap();

        capture.record_stream_event(&StreamEvent::Usage {
            input_tokens: Some(10),
            output_tokens: None,
            cache_read_tokens: None,
        });
        capture.record_stream_event(&StreamEvent::Usage {
            input_tokens: Some(15),
            output_tokens: Some(4),
            cache_read_tokens: None,
        });
        capture.fail_and_begin_attempt("fallback".to_string(), None, None);
        capture.record_stream_event(&StreamEvent::Usage {
            input_tokens: Some(12),
            output_tokens: Some(3),
            cache_read_tokens: None,
        });
        capture.record_stream_event(&StreamEvent::Result {
            subtype: ResultSubtype::Success,
            cost_usd: Some(0.25),
            duration_secs: Some(1.0),
        });
        capture.finish("completed").unwrap();

        let runs = scan_runs_since(home.path(), 0).unwrap();
        assert_eq!(runs.len(), 1);
        let run = &runs[0];
        assert_eq!(run.subject("task"), Some("LOO-265"));
        assert_eq!(run.outcome.as_deref(), Some("completed"));
        assert_eq!(run.usage.streams, 2);
        assert_eq!(run.usage.final_streams, 1);
        assert_eq!(run.usage.input_tokens, Some(27));
        assert_eq!(run.usage.output_tokens, Some(7));
        assert_eq!(run.usage.cost_usd, Some(0.25));
        assert_eq!(run.usage.gaps, 0);
    }

    #[test]
    fn reader_rejects_a_malformed_complete_event() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        let dir = capture.artifact_dir();
        capture.finish("completed").unwrap();
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("events.jsonl"))
            .unwrap()
            .write_all(b"{malformed}\n")
            .unwrap();

        let error = read_run_snapshot(&dir).unwrap_err();
        assert!(error.to_string().contains("malformed complete Run event"));
    }

    #[test]
    fn reader_ignores_a_valid_final_event_without_its_newline() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        let dir = capture.artifact_dir();
        capture.mark_spawn_requested();
        capture.finish("completed").unwrap();
        let path = dir.join("events.jsonl");
        let mut events = fs::read(&path).unwrap();
        assert_eq!(events.pop(), Some(b'\n'));
        fs::write(&path, events).unwrap();

        let snapshot = read_run_snapshot(&dir).unwrap();
        assert_eq!(snapshot.evidence_gaps, 1);
    }

    #[test]
    fn reader_rejects_an_unsupported_manifest_schema() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        let dir = capture.artifact_dir();
        capture.finish("completed").unwrap();
        let path = dir.join("manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        manifest["schema_version"] = serde_json::json!(999);
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();

        let error = read_run_snapshot(&dir).unwrap_err();
        assert!(error
            .to_string()
            .contains("unsupported Run manifest schema 999"));
    }
}
