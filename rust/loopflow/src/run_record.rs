//! Authoritative, Home-local evidence for one Loopflow harness launch.

pub mod active;
pub(crate) mod activity;

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
/// A restriction inherited across Home changes, never execution authority.
pub(crate) const TASK_ORIGIN_ENV: &str = "LF_TASK_ORIGIN";

pub(crate) fn task_origin() -> bool {
    if std::env::var_os(TASK_ORIGIN_ENV).is_some()
        || std::env::var_os(crate::durable::TASK_WORKER_CLAIM_ENV).is_some()
    {
        return true;
    }
    let Some(run_id) = std::env::var_os(RUN_ID_ENV) else {
        return false;
    };
    let manifest = std::env::var_os(RUN_DIR_ENV)
        .map(PathBuf::from)
        .and_then(|dir| read_manifest(&dir).ok());
    // An unreadable or mismatched inherited Run cannot establish permission
    // to change the machine installation. Ordinary commands still work.
    let Some(manifest) = manifest.filter(|manifest| manifest.run_id.as_str() == run_id) else {
        return true;
    };
    manifest
        .subjects
        .iter()
        .any(|subject| subject.selector.starts_with("task:"))
        || matches!(manifest.flow, Some(RunFlowMembership::Step(step)) if step.task_id.is_some())
}

pub(crate) fn preserve_task_origin() {
    if task_origin() {
        std::env::set_var(TASK_ORIGIN_ENV, "1");
    }
}

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
    pub flow: RunFlowMembership,
    pub work: Option<crate::session::RunWork>,
}

/// The exact managed Task or standalone Flow occurrence a Run executes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunFlowStep {
    pub task_id: Option<crate::work::task::TaskId>,
    pub task_pr_id: Option<crate::work::task::TaskPrId>,
    pub invocation_id: String,
    pub flow: String,
    pub step: String,
    /// Older manifests omitted the structural path; never infer it from a leaf index.
    pub node: Option<String>,
    /// Older captures have no tuple; never derive it from their scalar visit token.
    pub iterations: Option<Vec<Vec<u32>>>,
}

impl RunFlowStep {
    /// The step an invocation's cursor selects; its Task is the invocation's.
    pub(crate) fn of(flow: &crate::durable::FlowSession) -> anyhow::Result<Self> {
        let step = flow
            .step_name()
            .ok_or_else(|| anyhow::anyhow!("Flow position has no current step"))?;
        Ok(Self {
            task_id: flow.task_id.clone(),
            task_pr_id: None,
            invocation_id: flow.invocation.id.clone(),
            flow: flow.invocation.flow.clone(),
            step,
            node: Some(flow.cursor.node_key()),
            iterations: Some(crate::engine::flow_graph::flow_iterations(
                &flow.invocation.steps,
                &flow.cursor,
            )),
        })
    }
}

/// Recorded when the Run is captured: a Flow step, or a Run outside a Flow. Manifests written before this field existed have none;
/// readers treat that absence as unknown, never as independent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunFlowMembership {
    Step(RunFlowStep),
    Independent,
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

pub(crate) fn find_subject<'a>(subjects: &'a [SubjectAttribution], kind: &str) -> Option<&'a str> {
    subjects
        .iter()
        .find_map(|subject| subject.selector.strip_prefix(kind)?.strip_prefix(':'))
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
    pub flow: Option<RunFlowMembership>,
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

#[derive(Serialize)]
struct RunContextArtifact<'a> {
    schema_version: u32,
    context: &'a crate::trace::PreparedTurnContext,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RunEvent {
    Activity {
        observation: activity::Observation,
    },
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
    ProviderAccountSelected {
        attempt_key: String,
        account_id: Option<crate::store::ProviderAccountId>,
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
    pub task_pr_id: Option<crate::work::task::TaskPrId>,
    pub repo: Option<String>,
    pub worktree: Option<String>,
    pub subjects: Vec<SubjectAttribution>,
    pub skill: Option<String>,
    pub outcome: Option<String>,
    pub started: i64,
    pub first_provider_attempt_at: Option<i64>,
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
        find_subject(&self.subjects, kind)
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

/// Listed Runs as snapshots: identity, parentage, Work, state and times from
/// each row, usage and launch evidence from its record. A Run launched on
/// another Home keeps its record there and is left out.
pub(crate) fn run_snapshots(
    lf_home: &Path,
    runs: Vec<crate::store::sqlite::ListedRun>,
) -> std::io::Result<Vec<(crate::session::Run, RunSnapshot)>> {
    let mut snapshots = Vec::new();
    for listed in runs {
        let run = listed.run;
        let dir = record_dir(lf_home, &run.id)
            .ok_or_else(|| std::io::Error::other(format!("Run {} has an invalid id", run.id)))?;
        let evidence = match read_run_snapshot(&dir) {
            Ok(evidence) => evidence,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let source = match run.work_source {
            Some(crate::session::WorkSource::Inherited) => AttributionSource::Inherited,
            _ => AttributionSource::Declared,
        };
        let subjects = [
            ("wave", listed.wave),
            ("project", listed.project),
            ("task", listed.task),
        ]
        .into_iter()
        .filter_map(|(kind, name)| {
            Some(SubjectAttribution {
                selector: format!("{kind}:{}", name?),
                source,
            })
        })
        .collect();
        let snapshot = RunSnapshot {
            id: run.id.to_string(),
            parent_run_id: run.caller_run_id.as_ref().map(ToString::to_string),
            subjects,
            skill: run.skill.clone(),
            outcome: run.ended.as_ref().map(|end| end.outcome.clone()),
            started: run.created_at,
            ended: run.ended.as_ref().map(|end| end.at),
            harness: run.provider.clone().unwrap_or(evidence.harness),
            model: run.model.clone(),
            ..evidence
        };
        snapshots.push((run, snapshot));
    }
    Ok(snapshots)
}

pub(crate) fn record_dirs(lf_home: &Path) -> std::io::Result<Vec<PathBuf>> {
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
    project_run(dir, manifest)
}

fn project_run(dir: &Path, manifest: RunManifest) -> std::io::Result<RunSnapshot> {
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
    let evidence = reduce_events(&dir.join("events.jsonl"))?;
    evidence_gaps += evidence.gaps;

    Ok(RunSnapshot {
        id: manifest.run_id.to_string(),
        parent_run_id: manifest.parent_run_id.map(|id| id.to_string()),
        task_pr_id: match manifest.flow {
            Some(RunFlowMembership::Step(step)) => step.task_pr_id,
            _ => None,
        },
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
        first_provider_attempt_at: evidence.first_provider_attempt_at,
        ended: terminal.map(|receipt| receipt.ended_at.unix_timestamp()),
        usage: evidence.usage,
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
    let mut accounts = HashMap::new();
    for line in BufReader::new(file).lines() {
        let envelope: EventEnvelope =
            serde_json::from_str(&line?).map_err(std::io::Error::other)?;
        if envelope.schema_version != SCHEMA_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unsupported Run event schema",
            ));
        }
        match envelope.event {
            RunEvent::ProviderAttemptStarted {
                attempt_key,
                account_id,
                ..
            }
            | RunEvent::ProviderAccountSelected {
                attempt_key,
                account_id,
            } => {
                accounts.insert(attempt_key, account_id);
            }
            RunEvent::ProviderSessionObserved {
                attempt_key,
                provider_session_id,
            } => {
                provider_session = Some(ProviderSessionRef {
                    schema_version: SCHEMA_VERSION,
                    provider_session_id,
                    account_id: accounts.get(&attempt_key).cloned().flatten(),
                });
            }
            _ => {}
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

pub(crate) fn remove_provider_client(dir: &Path, pid: u32) -> std::io::Result<()> {
    let path = dir.join("provider-clients").join(format!("{pid}.json"));
    match fs::remove_file(path) {
        Ok(()) => sync_dir(&dir.join("provider-clients")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Who chose a Session's current title. A human name is never replaced by a
/// generated suggestion. `Unavailable` is never stored: the canonical name
/// lives on another Home and the reader only has a local display label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionTitleSource {
    Generated,
    Human,
    Unavailable,
}

const SESSION_TITLE_MAX_CHARS: usize = 80;

/// One trimmed, non-empty line of at most `SESSION_TITLE_MAX_CHARS` characters.
pub(crate) fn validate_session_title(title: &str) -> std::io::Result<&str> {
    let title = title.trim();
    if title.is_empty() || title.contains(['\n', '\r']) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Session name must be one non-empty line",
        ));
    }
    if title.chars().count() > SESSION_TITLE_MAX_CHARS {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Session name must be at most {SESSION_TITLE_MAX_CHARS} characters"),
        ));
    }
    Ok(title)
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

#[derive(Debug)]
struct RunEvidence {
    usage: RunUsage,
    gaps: usize,
    first_provider_attempt_at: Option<i64>,
}

fn reduce_events(path: &Path) -> std::io::Result<RunEvidence> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RunEvidence {
                usage: RunUsage::empty(),
                gaps: 0,
                first_provider_attempt_at: None,
            });
        }
        Err(error) => return Err(error),
    };
    let mut streams = BTreeMap::<String, UsageStream>::new();
    let mut gaps = 0;
    let mut envelope_seq = None;
    let mut first_provider_attempt_at = None;
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
        if first_provider_attempt_at.is_none()
            && envelope.schema_version == SCHEMA_VERSION
            && matches!(&envelope.event, RunEvent::ProviderAttemptStarted { .. })
        {
            first_provider_attempt_at = Some(envelope.observed_at.unix_timestamp());
        }
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
    Ok(RunEvidence {
        usage,
        gaps,
        first_provider_attempt_at,
    })
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
        let (manifest, _) =
            prepare_manifest(spec, id.clone(), parent, None, None).map_err(record_error)?;
        let dir = publish_manifest(home, &manifest, None).map_err(record_error)?;
        write_private_exclusive(&dir.join("prepared"), b"").map_err(record_error)?;
        sync_dir(&dir).map_err(record_error)?;
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
        // Preparation is the owning transaction for a human Flow step's
        // membership; a launch cannot reassign the prepared occurrence.
        manifest.flow = manifest.flow.or(Some(spec.flow));
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

    pub(crate) fn begin_with_context(
        spec: RunSpec,
        context: &crate::trace::PreparedTurnContext,
        launch: Option<RunLaunchRequest>,
    ) -> StoreResult<Self> {
        Self::begin_with_id_and_parent(spec, RunId::new(), None, true, launch, Some(context))
    }

    pub(crate) fn begin_reserved_with_context(
        spec: RunSpec,
        run_id: RunId,
        launch: Option<RunLaunchRequest>,
        context: &crate::trace::PreparedTurnContext,
        publish: impl FnOnce(&RunId) -> StoreResult<()>,
    ) -> StoreResult<Self> {
        let home = crate::store::authority_home_dir();
        let parent = inherited_parent().and_then(|id| verified_parent(&home, id));
        Self::begin_reserved_at(&home, spec, run_id, parent, launch, context, publish)
    }

    fn begin_reserved_at(
        home: &Path,
        spec: RunSpec,
        run_id: RunId,
        parent: Option<RunId>,
        launch: Option<RunLaunchRequest>,
        context: &crate::trace::PreparedTurnContext,
        publish: impl FnOnce(&RunId) -> StoreResult<()>,
    ) -> StoreResult<Self> {
        let (manifest, context) =
            prepare_manifest(spec, run_id, parent, launch, Some(context)).map_err(record_error)?;
        let (manifest, dir) = reconcile_reserved_manifest(home, manifest, context.as_deref())
            .map_err(record_error)?;
        // Only the reservation transaction grants launch authority. A rejected
        // publication must not start a recorder or settle somebody else's Run.
        publish(&manifest.run_id)?;
        Ok(Self(Arc::new(Mutex::new(RunCapture::from_manifest(
            manifest, dir,
        )))))
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
        let work = spec.work.clone();
        let (manifest, context_bytes) =
            prepare_manifest(spec, run_id, parent_run_id, launch, context).map_err(record_error)?;
        let dir =
            publish_manifest(lf_home, &manifest, context_bytes.as_deref()).map_err(record_error)?;
        RunCapture::record_row(&manifest, &dir, work)?;
        Ok(Self(Arc::new(Mutex::new(RunCapture::from_manifest(
            manifest, dir,
        )))))
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

    /// Claim an admitted conversation and retain the exact provider provenance
    /// used by its tools. A later driver transfer never rewrites this launch.
    pub(crate) fn claim_conversation_driver(&self) -> StoreResult<()> {
        let Some(exec_id) = crate::journal::current_exec_id() else {
            // Library callers outside an actual lf process have no Exec to name.
            return Ok(());
        };
        let mut capture = self.0.lock().expect("Run capture mutex poisoned");
        if capture.driver.is_some() {
            return Ok(());
        }
        let store = row_store(&capture.dir)?;
        let Some((session, _)) = store.session_for_run(&capture.manifest.run_id)? else {
            return Ok(());
        };
        let expected = store.session_driver(&session.id)?;
        if expected
            .as_ref()
            .is_some_and(|driver| driver.exec_id.is_some())
        {
            return Err(StoreError::InvalidAuthority(
                "Conversation already has a driver; connect to it".into(),
            ));
        }
        let driver = store.claim_session_driver(&session.id, expected.as_ref(), &exec_id, true)?;
        capture.driver = Some((session.id, driver));
        Ok(())
    }

    pub(crate) fn session_driver(&self) -> Option<(String, crate::exec::SessionDriver)> {
        self.0
            .lock()
            .expect("Run capture mutex poisoned")
            .driver
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
        if let Some((session, driver)) = &capture.driver {
            environment.insert(
                crate::exec::AGENT_CALLER_ENV.into(),
                serde_json::to_string(&driver.caller(session.clone()))
                    .expect("caller provenance serializes"),
            );
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

    pub(crate) fn observe_provider(
        &self,
        session_id: Option<String>,
        account_id: Option<crate::store::ProviderAccountId>,
    ) {
        self.with_capture(|capture| {
            let account_changed = !capture.account_observed || capture.account_id != account_id;
            if account_changed {
                capture.append_event(RunEvent::ProviderAccountSelected {
                    attempt_key: capture.attempt_key(),
                    account_id: account_id.clone(),
                })?;
                capture.account_id = account_id;
                capture.account_observed = true;
            }
            let Some(session_id) = session_id else {
                return Ok(());
            };
            if !account_changed && capture.provider_session_id.as_ref() == Some(&session_id) {
                return Ok(());
            }
            write_provider_session(&capture.dir, &session_id, capture.account_id.clone())?;
            capture.append_event(RunEvent::ProviderSessionObserved {
                attempt_key: capture.attempt_key(),
                provider_session_id: session_id.clone(),
            })?;
            capture.provider_session_id = Some(session_id);
            Ok(())
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
    driver: Option<(String, crate::exec::SessionDriver)>,
    binding: Option<active::RunBindingGuard>,
    manifest: RunManifest,
    dir: PathBuf,
    provider: String,
    model: Option<String>,
    account_id: Option<crate::store::ProviderAccountId>,
    account_observed: bool,
    provider_session_id: Option<String>,
    attempt: u32,
    attempt_started: bool,
    turn_key: String,
    usage_stream_id: String,
    event_seq: u64,
    usage_seq: u64,
    recorder: RunRecorder,
    telemetry_warned: bool,
    settled_outcome: Option<String>,
    activity: activity::Observer,
}

impl RunCapture {
    /// Independent agent launches, including helpers, admit their conversation
    /// before provider work. Flow reservations have their own fenced publisher.
    fn record_row(
        manifest: &RunManifest,
        dir: &Path,
        work: Option<crate::session::RunWork>,
    ) -> StoreResult<()> {
        let invocation_id = match &manifest.flow {
            Some(RunFlowMembership::Step(step)) => Some(step.invocation_id.clone()),
            Some(RunFlowMembership::Independent) | None => None,
        };
        let mut run = crate::session::Run {
            id: manifest.run_id.clone(),
            session_id: None,
            node: None,
            iterations: None,
            attempt: None,
            task_id: work.as_ref().and_then(|work| work.task_id.clone()),
            wave_id: work.as_ref().and_then(|work| work.wave_id.clone()),
            work_source: work.as_ref().map(|work| work.source),
            invocation_id,
            created_at: manifest.created_at.unix_timestamp(),
            published: true,
            cwd: manifest.cwd.clone(),
            skill: manifest.skill.clone(),
            provider: Some(manifest.harness.clone()),
            model: manifest.model.clone(),
            caller_run_id: manifest.parent_run_id.clone(),
            ended: None,
        };
        let store = row_store(dir)?;
        if manifest.harness != "loopflow" && run.invocation_id.is_none() {
            let id = format!("session_{}", Uuid::new_v4().simple());
            run.session_id = Some(id.clone());
            let session = crate::session::AgentSession {
                task_id: None,
                wave_id: None,
                flow_session_id: None,
                work_source: None,
                bound_at: None,
                id,
                current_run_id: run.id.clone(),
                kind: crate::session::SessionKind::Conversation,
                interactive: manifest.surface != "headless",
                repo: None,
                title: run
                    .skill
                    .clone()
                    .unwrap_or_else(|| crate::engine::naming::word_pair(run.id.as_str())),
                title_source: crate::session::TitleSource::Generated,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: run.created_at,
            };
            store.create_session(
                session,
                run,
                None,
                crate::journal::current_exec_id().as_ref(),
            )?;
        } else {
            store.create_run(run, crate::journal::current_exec_id().as_ref())?;
        }
        Ok(())
    }

    fn from_manifest(manifest: RunManifest, dir: PathBuf) -> Self {
        let recorder = RunRecorder::start(&dir, &manifest.run_id);
        Self {
            driver: None,
            binding: None,
            provider: manifest.harness.clone(),
            model: manifest.model.clone(),
            account_id: manifest
                .launch
                .as_ref()
                .and_then(|launch| launch.account_id.clone()),
            account_observed: false,
            provider_session_id: None,
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
            activity: activity::Observer::default(),
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
        self.account_observed = false;
        self.provider_session_id = None;
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
        let end = crate::session::RunEnd {
            outcome: outcome.to_string(),
            at: OffsetDateTime::now_utc().unix_timestamp(),
        };
        if let Err(error) =
            row_store(&self.dir).and_then(|store| store.end_run(&self.manifest.run_id, &end))
        {
            tracing::warn!(%error, run_id = %self.manifest.run_id, "Run end is not recorded");
        }
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
        if let Some((session, driver)) = self.driver.take() {
            match row_store(&self.dir)
                .and_then(|store| store.release_session_driver(&session, &driver))
            {
                Ok(_) | Err(StoreError::InvalidAuthority(_)) => {}
                Err(error) => return Err(std::io::Error::other(error)),
            }
        }
        Ok(())
    }

    fn append_event(&mut self, event: RunEvent) -> std::io::Result<()> {
        if !matches!(&event, RunEvent::Activity { .. }) {
            self.activity.note_event(OffsetDateTime::now_utc());
        }
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

/// The store that holds the row of the Run recorded at `dir`.
fn row_store(dir: &Path) -> StoreResult<crate::store::sqlite::SqliteStore> {
    #[cfg(test)]
    let path = std::env::var_os("LF_DB_PATH")
        .map(PathBuf::from)
        .or_else(|| Some(dir.ancestors().nth(3)?.join("loopflow.db")))
        .ok_or_else(|| record_error(std::io::Error::other("Run record has no Home")))?;
    #[cfg(not(test))]
    let path = {
        let _ = dir;
        crate::store::database_path_from_env().map_err(record_error)?
    };
    crate::store::sqlite::SqliteStore::new(&path)
}

pub(crate) fn inherited_parent() -> Option<RunId> {
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

pub(crate) fn record_dir(lf_home: &Path, run_id: &RunId) -> Option<PathBuf> {
    let prefix = run_id.as_str().strip_prefix("run_")?.get(..2)?;
    Some(lf_home.join("runs").join(prefix).join(run_id.as_str()))
}

fn prepare_manifest(
    spec: RunSpec,
    run_id: RunId,
    parent_run_id: Option<RunId>,
    launch: Option<RunLaunchRequest>,
    context: Option<&crate::trace::PreparedTurnContext>,
) -> std::io::Result<(RunManifest, Option<Vec<u8>>)> {
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
        run_id,
        parent_run_id,
        created_at: OffsetDateTime::now_utc(),
        harness: spec.harness,
        model: spec.model,
        surface: spec.surface,
        cwd: spec.cwd,
        repo: spec.repo,
        worktree: spec.worktree,
        skill: spec.skill,
        subjects: spec.subjects,
        flow: Some(spec.flow),
        launch,
        context: context_ref,
        runtime_path,
        runtime_digest,
        host: gethostname::gethostname().to_string_lossy().into_owned(),
        boot_id: boot_id(),
    };
    Ok((manifest, context_bytes))
}

/// Resume artifact publication only. The caller must still claim the SQL
/// reservation before launching; readable artifacts confer no launch authority.
fn reconcile_reserved_manifest(
    home: &Path,
    mut manifest: RunManifest,
    context: Option<&[u8]>,
) -> std::io::Result<(RunManifest, PathBuf)> {
    let published =
        record_dir(home, &manifest.run_id).expect("Run ids always contain a UUID prefix");
    let parent = published
        .parent()
        .expect("Run record has a prefix directory");
    create_private_dir(parent)?;
    let staging = parent.join(format!(".{}.staging", manifest.run_id));
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
            "reserved Run {} already has terminal evidence; retained unchanged",
            manifest.run_id
        )));
    }
    let manifest_path = dir.join("manifest.json");
    if manifest_path.try_exists()? {
        let existing = read_manifest(dir)?;
        // Creation time belongs to the first publication attempt. All other
        // immutable inputs, including exact context digest and parent, must match.
        manifest.created_at = existing.created_at;
        if serde_json::to_value(&manifest)? != serde_json::to_value(&existing)? {
            return Err(std::io::Error::other(format!(
                "reserved Run {} has different immutable launch inputs at {}",
                manifest.run_id,
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
        sync_dir(parent)?;
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
                    "reserved Run artifact differs at {}; retained unchanged",
                    path.display()
                )))
            }
        }
        Err(error) => Err(error),
    }
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs::{self, OpenOptions};
    use std::io::Write;

    use super::{
        read_final_answer, read_provider_clients, read_provider_session, read_run_snapshot,
        remove_provider_client, write_provider_client, CaptureHandle, RunLaunchRequest,
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
            flow: crate::run_record::RunFlowMembership::Independent,
            work: None,
        }
    }

    #[test]
    fn task_installation_restriction_survives_removing_run_authority() {
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let home = tempfile::tempdir().unwrap();
        assert!(!super::task_origin());
        for bound in [false, true] {
            let mut request = spec(home.path());
            if bound {
                request.subjects.push(SubjectAttribution::declared(format!(
                    "task:{}",
                    crate::durable::TaskId::new()
                )));
            }
            let run = CaptureHandle::prepare_at(home.path(), request, None).unwrap();
            let dir = super::record_dir(home.path(), &run).unwrap();
            std::env::set_var(super::RUN_ID_ENV, run.as_str());
            std::env::set_var(super::RUN_DIR_ENV, dir);
            assert_eq!(super::task_origin(), bound);
            super::preserve_task_origin();
            std::env::remove_var(super::RUN_ID_ENV);
            std::env::remove_var(super::RUN_DIR_ENV);
            assert_eq!(super::task_origin(), bound);
        }
        std::env::remove_var(super::TASK_ORIGIN_ENV);
        std::env::set_var(super::RUN_ID_ENV, "run_missing");
        assert!(super::task_origin());
    }

    #[test]
    fn prepared_run_projects_its_first_provider_attempt_separately_from_creation() {
        let home = tempfile::tempdir().unwrap();
        let id = CaptureHandle::prepare_at(home.path(), spec(home.path()), None).unwrap();
        let (dir, mut manifest) = super::resolve_manifest(home.path(), id.as_str()).unwrap();
        manifest.created_at = time::OffsetDateTime::from_unix_timestamp(1).unwrap();
        fs::write(
            dir.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let prepared = serde_json::to_value(read_run_snapshot(&dir).unwrap()).unwrap();
        assert!(prepared["first_provider_attempt_at"].is_null());
        let context = crate::trace::PreparedTurnContext::from_prompts("system", "review");
        let capture =
            CaptureHandle::start_prepared(home.path(), &id, spec(home.path()), &context).unwrap();
        capture.mark_spawn_requested();
        capture.fail_and_begin_attempt("claude".to_string(), None, None);
        capture.finish("completed").unwrap();

        // Fixed recorded times distinguish preparation, first attempt and retry
        // without making the proof wait for the wall clock to advance.
        let events = fs::read_to_string(dir.join("events.jsonl")).unwrap();
        let mut attempt = 0;
        let events = events
            .lines()
            .map(|line| {
                let mut envelope: super::EventEnvelope = serde_json::from_str(line).unwrap();
                if matches!(
                    envelope.event,
                    super::RunEvent::ProviderAttemptStarted { .. }
                ) {
                    attempt += 1;
                    envelope.observed_at =
                        time::OffsetDateTime::from_unix_timestamp(attempt * 10).unwrap();
                }
                serde_json::to_string(&envelope).unwrap()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(attempt, 2);
        fs::write(dir.join("events.jsonl"), format!("{events}\n")).unwrap();

        let snapshot = serde_json::to_value(read_run_snapshot(&dir).unwrap()).unwrap();
        assert_eq!(snapshot["started"], 1);
        assert_eq!(snapshot["first_provider_attempt_at"], 10);
    }

    #[test]
    fn prepared_run_keeps_its_recorded_pr_instead_of_the_launching_pr() {
        for historical in [false, true] {
            let home = tempfile::tempdir().unwrap();
            let original = crate::work::task::TaskPrId::new();
            let mut step = super::RunFlowStep {
                task_id: Some(crate::work::task::TaskId::new()),
                task_pr_id: Some(original.clone()),
                invocation_id: "invocation".into(),
                flow: "feature".into(),
                step: "review".into(),
                node: Some("1".into()),
                iterations: Some(Vec::new()),
            };
            let mut prepared = spec(home.path());
            prepared.flow = super::RunFlowMembership::Step(step.clone());
            let id = CaptureHandle::prepare_at(home.path(), prepared, None).unwrap();
            let (dir, _) = super::resolve_manifest(home.path(), id.as_str()).unwrap();
            if historical {
                let mut manifest: serde_json::Value =
                    serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
                manifest["flow"]
                    .as_object_mut()
                    .unwrap()
                    .remove("task_pr_id");
                fs::write(
                    dir.join("manifest.json"),
                    serde_json::to_vec(&manifest).unwrap(),
                )
                .unwrap();
            }
            step.task_pr_id = Some(crate::work::task::TaskPrId::new());
            let mut launch = spec(home.path());
            launch.flow = super::RunFlowMembership::Step(step);
            let context = crate::trace::PreparedTurnContext::from_prompts("system", "review");
            let capture =
                CaptureHandle::start_prepared(home.path(), &id, launch, &context).unwrap();
            capture.finish("completed").unwrap();

            let snapshot = read_run_snapshot(&dir).unwrap();
            assert_eq!(snapshot.task_pr_id, (!historical).then_some(original));
            assert_eq!(snapshot.first_provider_attempt_at, None);
        }
    }

    #[test]
    fn helper_capture_admits_a_headless_conversation_with_its_input_and_outcome() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let home = tempfile::tempdir().unwrap();
        let launch = RunLaunchRequest::from_prepared(
            &AgentConfig {
                task_prompt: "repair the failed operation".into(),
                ..Default::default()
            },
            &AgentCapabilities::default(),
        );
        let capture =
            CaptureHandle::begin_at_with_launch(home.path(), spec(home.path()), launch).unwrap();
        let store = super::row_store(&capture.artifact_dir()).unwrap();
        let (session, run) = store.session_for_run(&capture.run_id()).unwrap().unwrap();
        assert!(!session.interactive);
        assert_eq!(session.kind, crate::session::SessionKind::Conversation);
        assert_eq!(session.title, "implement");
        assert_eq!(run.provider.as_deref(), Some("proof"));
        assert_eq!(
            super::read_manifest(&capture.artifact_dir())
                .unwrap()
                .launch
                .unwrap()
                .task_prompt,
            "repair the failed operation"
        );
        capture.finish("failed").unwrap();
        let (after, run) = store.session_for_run(&capture.run_id()).unwrap().unwrap();
        assert_eq!(after.id, session.id);
        assert_eq!(run.ended.unwrap().outcome, "failed");
        assert!(after.completed_at.is_none());
    }

    #[test]
    fn reserved_publication_recovers_artifacts_without_repeating_launch_authority() {
        for boundary in [
            "before_artifacts",
            "context_staged",
            "manifest_staged",
            "artifacts_published",
        ] {
            let home = tempfile::tempdir().unwrap();
            let id = crate::durable::RunId::new();
            let context = crate::trace::PreparedTurnContext::from_prompts("system", "review");
            let (manifest, bytes) =
                super::prepare_manifest(spec(home.path()), id.clone(), None, None, Some(&context))
                    .unwrap();
            let dir = super::record_dir(home.path(), &id).unwrap();
            let staging = dir.parent().unwrap().join(format!(".{id}.staging"));
            if boundary == "context_staged" || boundary == "manifest_staged" {
                std::fs::create_dir_all(&staging).unwrap();
                std::fs::write(staging.join("context.json"), bytes.as_ref().unwrap()).unwrap();
                if boundary == "manifest_staged" {
                    std::fs::write(
                        staging.join("manifest.json"),
                        serde_json::to_vec_pretty(&manifest).unwrap(),
                    )
                    .unwrap();
                }
            }
            if boundary == "artifacts_published" {
                super::publish_manifest(home.path(), &manifest, bytes.as_deref()).unwrap();
            }
            // Interrupt after artifacts, before SQL publication. There must be
            // no capture Drop receipt falsely settling the prepared Run.
            let denied = || {
                Err(crate::store::StoreError::InvalidAuthority(
                    "interrupted publication".into(),
                ))
            };
            assert!(CaptureHandle::begin_reserved_at(
                home.path(),
                spec(home.path()),
                id.clone(),
                None,
                None,
                &context,
                |_| denied()
            )
            .is_err());
            let original = std::fs::read(dir.join("manifest.json")).unwrap();
            assert!(!dir.join("terminal.json").exists());
            assert!(!dir.join("events.jsonl").exists());
            let capture = CaptureHandle::begin_reserved_at(
                home.path(),
                spec(home.path()),
                id.clone(),
                None,
                None,
                &context,
                |_| Ok(()),
            )
            .unwrap();
            assert_eq!(capture.run_id(), id);
            assert_eq!(std::fs::read(dir.join("manifest.json")).unwrap(), original);
            assert_eq!(
                std::fs::read(dir.join("context.json")).unwrap(),
                bytes.unwrap()
            );
            // SQL has granted authority once, but no provider has started.
            // An absent provider receipt does not grant a second launch.
            assert!(CaptureHandle::begin_reserved_at(
                home.path(),
                spec(home.path()),
                id.clone(),
                None,
                None,
                &context,
                |_| denied()
            )
            .is_err());
            assert!(!dir.join("terminal.json").exists());
            capture.finish("interrupted").unwrap();
            let terminal = std::fs::read(dir.join("terminal.json")).unwrap();
            assert!(CaptureHandle::begin_reserved_at(
                home.path(),
                spec(home.path()),
                id,
                None,
                None,
                &context,
                |_| Ok(())
            )
            .is_err());
            assert_eq!(std::fs::read(dir.join("terminal.json")).unwrap(), terminal);
        }
    }

    #[test]
    fn reserved_publication_retains_conflicting_inputs() {
        let home = tempfile::tempdir().unwrap();
        let id = crate::durable::RunId::new();
        let context = crate::trace::PreparedTurnContext::from_prompts("system", "review");
        let (manifest, bytes) =
            super::prepare_manifest(spec(home.path()), id.clone(), None, None, Some(&context))
                .unwrap();
        let dir = super::publish_manifest(home.path(), &manifest, bytes.as_deref()).unwrap();
        let original = std::fs::read(dir.join("manifest.json")).unwrap();
        let changed = crate::trace::PreparedTurnContext::from_prompts("system", "different review");
        let error = CaptureHandle::begin_reserved_at(
            home.path(),
            spec(home.path()),
            id.clone(),
            None,
            None,
            &changed,
            |_| Ok(()),
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("different immutable launch inputs"));
        assert_eq!(std::fs::read(dir.join("manifest.json")).unwrap(), original);
        assert_eq!(
            std::fs::read(dir.join("context.json")).unwrap(),
            bytes.unwrap()
        );
        assert!(!dir.join("terminal.json").exists());

        std::fs::write(dir.join("context.json"), b"interrupted write").unwrap();
        assert!(CaptureHandle::begin_reserved_at(
            home.path(),
            spec(home.path()),
            id,
            None,
            None,
            &context,
            |_| Ok(())
        )
        .is_err());
        assert_eq!(
            std::fs::read(dir.join("context.json")).unwrap(),
            b"interrupted write"
        );
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
        capture.observe_provider(Some("provider-session".to_string()), None);

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
    fn provider_account_observation_precedes_session_and_preserves_attempts() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        let dir = capture.artifact_dir();
        let first = crate::store::ProviderAccountId::parse("primary").unwrap();
        let second = crate::store::ProviderAccountId::parse("fallback").unwrap();
        capture.mark_spawn_requested();
        capture.observe_provider(None, Some(first.clone()));
        capture.0.lock().unwrap().recorder.drain_after_settlement();

        assert!(!dir.join("terminal.json").exists());
        assert!(read_provider_session(&dir).unwrap().is_none());
        let events = fs::read_to_string(dir.join("events.jsonl")).unwrap();
        assert!(events.contains("\"account_id\":\"primary\""));

        capture.observe_provider(Some("first-session".into()), Some(first.clone()));
        capture.observe_provider(Some("first-session".into()), Some(first.clone()));
        assert_eq!(
            read_provider_session(&dir).unwrap().unwrap().account_id,
            Some(first)
        );
        capture.fail_and_begin_attempt("proof".into(), None, Some(second.clone()));
        capture.observe_provider(Some("second-session".into()), Some(second.clone()));
        capture.0.lock().unwrap().recorder.drain_after_settlement();
        fs::remove_file(dir.join("provider-session.json")).unwrap();
        let recovered = read_provider_session(&dir).unwrap().unwrap();
        assert_eq!(recovered.provider_session_id, "second-session");
        assert_eq!(recovered.account_id, Some(second));
        capture.fail_and_begin_attempt("proof".into(), None, None);
        capture.observe_provider(Some("ambient-session".into()), None);
        capture.finish("completed").unwrap();

        let events: Vec<serde_json::Value> = fs::read_to_string(dir.join("events.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let accounts: Vec<_> = events
            .iter()
            .filter(|event| event["type"] == "provider_account_selected")
            .map(|event| (event["attempt_key"].clone(), event["account_id"].clone()))
            .collect();
        assert_eq!(
            accounts,
            vec![
                (serde_json::json!("attempt-1"), serde_json::json!("primary")),
                (
                    serde_json::json!("attempt-2"),
                    serde_json::json!("fallback")
                ),
                (serde_json::json!("attempt-3"), serde_json::Value::Null),
            ]
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event["type"] == "provider_session_observed")
                .count(),
            3
        );
        let ambient = read_provider_session(&dir).unwrap().unwrap();
        assert_eq!(ambient.provider_session_id, "ambient-session");
        assert_eq!(ambient.account_id, None);
        fs::remove_file(dir.join("provider-session.json")).unwrap();
        assert_eq!(read_provider_session(&dir).unwrap(), Some(ambient));
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
        for capture in [&first, &second] {
            let manifest = super::read_manifest(&capture.artifact_dir()).unwrap();
            assert_eq!(manifest.subjects[0].selector, selector);
        }
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
    fn reader_reduces_each_cumulative_stream_once_and_keeps_provider_finality() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();

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

        let run = read_run_snapshot(&capture.artifact_dir()).unwrap();
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
        assert!(snapshot.first_provider_attempt_at.is_some());
    }

    #[test]
    fn reader_does_not_invent_an_attempt_from_incomplete_or_unsupported_evidence() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        capture.mark_spawn_requested();
        capture.finish("completed").unwrap();
        let dir = capture.artifact_dir();
        let path = dir.join("events.jsonl");
        let events = fs::read_to_string(&path).unwrap();
        let mut attempt = events
            .lines()
            .map(|line| serde_json::from_str::<super::EventEnvelope>(line).unwrap())
            .find(|envelope| {
                matches!(
                    envelope.event,
                    super::RunEvent::ProviderAttemptStarted { .. }
                )
            })
            .unwrap();
        // An otherwise valid attempt without a final newline is not committed evidence.
        fs::write(&path, serde_json::to_vec(&attempt).unwrap()).unwrap();
        let snapshot = read_run_snapshot(&dir).unwrap();
        assert_eq!(snapshot.first_provider_attempt_at, None);
        assert_eq!(snapshot.evidence_gaps, 1);
        attempt.schema_version = 999;
        fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&attempt).unwrap()),
        )
        .unwrap();
        let snapshot = read_run_snapshot(&dir).unwrap();
        assert_eq!(snapshot.first_provider_attempt_at, None);
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
