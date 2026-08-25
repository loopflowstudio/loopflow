//! Authoritative, Home-local evidence for one Loopflow harness launch.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::chat::types::{ConversationEvent, TurnUsage};
use crate::durable::{RunId, RUN_ID_ENV};
use crate::engine::stream::{ResultSubtype, StreamEvent};
use crate::store::{StoreError, StoreResult};

pub const RUN_DIR_ENV: &str = "LF_RUN_DIR";
pub const PARENT_RUN_ID_ENV: &str = "LF_PARENT_RUN_ID";

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

/// Disposable projection of one immutable Run record.
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
            let _ = drained.recv_timeout(Duration::from_millis(250));
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

fn record_dirs(lf_home: &Path) -> std::io::Result<Vec<PathBuf>> {
    let root = lf_home.join("runs");
    let prefixes = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut records = Vec::new();
    for prefix in prefixes {
        let prefix = match prefix {
            Ok(prefix) if prefix.file_type().is_ok_and(|kind| kind.is_dir()) => prefix,
            Ok(_) => continue,
            Err(error) => {
                tracing::warn!(%error, root = %root.display(), "Run record prefix unavailable");
                continue;
            }
        };
        let entries = match fs::read_dir(prefix.path()) {
            Ok(entries) => entries,
            Err(error) => {
                tracing::warn!(%error, prefix = %prefix.path().display(), "Run record directory unavailable");
                continue;
            }
        };
        for record in entries {
            let record = match record {
                Ok(record)
                    if !record.file_name().to_string_lossy().starts_with('.')
                        && record.file_type().is_ok_and(|kind| kind.is_dir()) =>
                {
                    record
                }
                Ok(_) => continue,
                Err(error) => {
                    tracing::warn!(%error, prefix = %prefix.path().display(), "Run record unavailable");
                    continue;
                }
            };
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
    let mut evidence_gaps = 0;
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

fn read_manifest(dir: &Path) -> std::io::Result<RunManifest> {
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
    pub(crate) fn begin(spec: RunSpec) -> StoreResult<Self> {
        Self::begin_with_id(spec, RunId::new())
    }

    pub(crate) fn begin_with_launch(spec: RunSpec, launch: RunLaunchRequest) -> StoreResult<Self> {
        Self::begin_with_id_and_parent(spec, RunId::new(), None, true, Some(launch))
    }

    pub(crate) fn begin_replay_at(
        lf_home: &Path,
        spec: RunSpec,
        launch: RunLaunchRequest,
        parent_run_id: RunId,
    ) -> StoreResult<Self> {
        let parent_run_id = verified_parent(lf_home, parent_run_id);
        Self::begin_at_with_id(lf_home, spec, RunId::new(), parent_run_id, Some(launch))
    }

    pub(crate) fn begin_with_id(spec: RunSpec, run_id: RunId) -> StoreResult<Self> {
        Self::begin_with_id_and_parent(spec, run_id, None, true, None)
    }

    pub(crate) fn begin_with_verified_parent(
        spec: RunSpec,
        run_id: RunId,
        parent_run_id: Option<RunId>,
    ) -> StoreResult<Self> {
        Self::begin_with_id_and_parent(spec, run_id, parent_run_id, false, None)
    }

    fn begin_with_id_and_parent(
        spec: RunSpec,
        run_id: RunId,
        parent_run_id: Option<RunId>,
        inherit_parent: bool,
        launch: Option<RunLaunchRequest>,
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
        Self::begin_at_with_id(&home, spec, run_id, parent_run_id, launch)
    }

    #[cfg(test)]
    pub(crate) fn begin_at(lf_home: &Path, spec: RunSpec) -> StoreResult<Self> {
        Self::begin_at_with_id(lf_home, spec, RunId::new(), inherited_parent(), None)
    }

    #[cfg(test)]
    fn begin_at_with_launch(
        lf_home: &Path,
        spec: RunSpec,
        launch: RunLaunchRequest,
    ) -> StoreResult<Self> {
        Self::begin_at_with_id(
            lf_home,
            spec,
            RunId::new(),
            inherited_parent(),
            Some(launch),
        )
    }

    fn begin_at_with_id(
        lf_home: &Path,
        spec: RunSpec,
        run_id: RunId,
        parent_run_id: Option<RunId>,
        launch: Option<RunLaunchRequest>,
    ) -> StoreResult<Self> {
        let capture = RunCapture::begin(lf_home, spec, run_id, parent_run_id, launch)
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
    ) -> std::io::Result<Self> {
        let (runtime_path, runtime_digest) = runtime_identity();
        let account_id = launch
            .as_ref()
            .and_then(|request| request.account_id.clone());
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
            runtime_path,
            runtime_digest,
            host: gethostname::gethostname().to_string_lossy().into_owned(),
            boot_id: boot_id(),
        };
        let dir = publish_manifest(lf_home, &manifest)?;
        let recorder = RunRecorder::start(&dir, &run_id);
        Ok(Self {
            manifest,
            dir,
            provider: spec.harness,
            model: spec.model,
            account_id,
            attempt: 1,
            attempt_started: false,
            turn_key: Uuid::new_v4().to_string(),
            usage_stream_id: Uuid::new_v4().to_string(),
            event_seq: 0,
            usage_seq: 0,
            recorder,
            telemetry_warned: false,
            settled_outcome: None,
        })
    }

    fn attempt_key(&self) -> String {
        format!("attempt-{}", self.attempt)
    }

    fn start_attempt(&mut self) -> std::io::Result<()> {
        if self.attempt_started {
            return Ok(());
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

fn publish_manifest(lf_home: &Path, manifest: &RunManifest) -> std::io::Result<PathBuf> {
    let run_id = manifest.run_id.as_str();
    let published =
        record_dir(lf_home, &manifest.run_id).expect("Run ids always contain a UUID prefix");
    let parent = published
        .parent()
        .expect("Run record always has a prefix directory");
    create_private_dir(parent)?;
    let staging = parent.join(format!(".{run_id}.staging"));
    create_private_dir_exclusive(&staging)?;
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs::{self, OpenOptions};
    use std::io::Write;

    use super::{
        read_run_snapshot, scan_runs_since, CaptureHandle, RunLaunchRequest, RunManifest, RunSpec,
        SubjectAttribution, TerminalReceipt,
    };
    use crate::chat::types::{ConversationEvent, TurnUsage};
    use crate::engine::stream::{ResultSubtype, StreamEvent};
    use crate::engine::{AgentCapabilities, AgentConfig};

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
