//! Captured Session inputs and append-only provider evidence.

pub mod active;
pub(crate) mod activity;
pub(crate) mod recovery;
mod runtime;

pub(crate) use runtime::finish_session_driver;

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
#[cfg(test)]
use std::io::BufRead;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::chat::types::{ConversationEvent, ConversationItem, ItemDelta, Lifecycle, TurnUsage};
use crate::engine::stream::{ResultSubtype, StreamEvent};
use crate::store::{StoreError, StoreResult};

/// An opaque artifact directory key; it grants no conversation or Flow authority.
pub(crate) fn new_artifact_key() -> String {
    Uuid::new_v4().simple().to_string()
}

/// Historical run-prefixed paths retain their spelling as imported evidence.
pub(crate) fn parse_artifact_key(value: &str) -> Result<String, crate::durable::DurableDataError> {
    Uuid::parse_str(value.strip_prefix("run_").unwrap_or(value))
        .map_err(|error| crate::durable::DurableDataError::InvalidId(error.to_string()))?;
    Ok(value.to_owned())
}

pub const CAPTURE_KEY_ENV: &str = "LF_CAPTURE_KEY";
pub(crate) const PROVIDER_ACCOUNT_ID_ENV: &str = "LF_PROVIDER_ACCOUNT_ID";
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub(crate) struct SessionCaptureSpec {
    pub harness: String,
    pub model: Option<String>,
    pub surface: String,
    pub cwd: PathBuf,
    pub repo: Option<PathBuf>,
    pub worktree: Option<PathBuf>,
    pub skill: Option<String>,
    pub subjects: Vec<SubjectAttribution>,
    pub flow: SessionFlowMembership,
    pub work: Option<crate::session::SessionWork>,
}

/// The managed Task or standalone Flow position captured for a conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionFlowStep {
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

impl SessionFlowStep {
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

/// Membership at capture time; absence in historical manifests remains unknown.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionFlowMembership {
    Step(SessionFlowStep),
    Independent,
}

/// Replayable, provider-facing inputs for one ordinary headless exec.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentExecRequest {
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

impl AgentExecRequest {
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
pub struct SessionContextRef {
    pub path: String,
    pub content_sha256: String,
    pub bytes: u64,
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
    pub flow: Option<SessionFlowMembership>,
    pub exec: Option<AgentExecRequest>,
    pub context: Option<SessionContextRef>,
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
    event: CaptureEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ProviderSessionRef {
    schema_version: u32,
    pub(crate) provider_session_id: String,
    pub(crate) account_id: Option<crate::store::ProviderAccountId>,
}

impl ProviderSessionRef {
    pub(crate) fn validate(&self) -> std::io::Result<()> {
        if self.schema_version != SCHEMA_VERSION || self.provider_session_id.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid provider session reference",
            ));
        }
        Ok(())
    }
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
    Retired,
    Moved,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ProviderClientStop {
    schema_version: u32,
    reason: ProviderClientStopReason,
}

#[derive(Serialize)]
struct SessionContextArtifact<'a> {
    schema_version: u32,
    context: &'a crate::trace::PreparedTurnContext,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum CaptureEvent {
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
/// count of direct provider receipts; Recorder settlement never upgrades it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionUsage {
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

/// History beneath one conversation's immutable captured input. This is not a
/// resumable object. Provider records retain separate outcomes and driving Execs;
/// recorder completion is historical evidence, never native success or process exit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionHistory {
    pub session_id: String,
    pub captured: Option<i64>,
    pub artifact_key: Option<String>,
    pub caller_artifact_key: Option<String>,
    pub task_pr_id: Option<crate::work::task::TaskPrId>,
    pub repo: Option<String>,
    pub worktree: Option<String>,
    pub task_id: Option<crate::durable::TaskId>,
    pub wave_id: Option<crate::id::WaveId>,
    pub task_identifier: Option<String>,
    pub work_source: Option<crate::session::WorkSource>,
    pub wave_name: Option<String>,
    pub skill: Option<String>,
    pub observed_at: i64,
    pub first_provider_attempt_at: Option<i64>,
    pub recorded_outcome: Option<String>,
    pub recorded_at: Option<i64>,
    pub providers: Vec<ProviderHistory>,
    pub usage: SessionUsage,
    pub evidence_gaps: usize,
    pub harness: String,
    pub model: Option<String>,
    pub surface: String,
}

/// An exact native turn, or an older provider observation with unknown native
/// identity. References identify evidence; they confer no exec/settlement API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderHistory {
    pub reference: ProviderHistoryReference,
    pub exec_id: Option<crate::id::ExecId>,
    pub task_id: Option<crate::durable::TaskId>,
    pub wave_id: Option<crate::id::WaveId>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub outcome: Option<String>,
    pub usage: SessionUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderHistoryReference {
    NativeTurn {
        thread: String,
        turn: String,
        start_seq: Option<i64>,
        completion_seq: Option<i64>,
    },
    RecordedAttempt {
        captured: i64,
        attempt_key: String,
    },
}

impl SessionHistory {
    pub fn selector(&self) -> &str {
        self.artifact_key.as_deref().unwrap_or(&self.session_id)
    }

    pub fn label(&self) -> &str {
        self.skill.as_deref().unwrap_or(&self.harness)
    }

    /// Display all recorded provider results; a later success never erases failure.
    pub fn status(&self) -> String {
        if self.providers.is_empty() {
            return self
                .recorded_outcome
                .as_ref()
                .map(|outcome| format!("recorded {outcome}"))
                .unwrap_or_else(|| "unknown".into());
        }
        self.providers
            .iter()
            .map(|record| match record.reference {
                ProviderHistoryReference::NativeTurn { .. } => {
                    record.outcome.clone().unwrap_or_else(|| "unknown".into())
                }
                ProviderHistoryReference::RecordedAttempt { .. } => format!(
                    "recorded {}",
                    record.outcome.as_deref().unwrap_or("unknown")
                ),
            })
            .collect::<Vec<_>>()
            .join(" → ")
    }

    pub fn total_tokens(&self) -> Option<i64> {
        self.usage
            .input_tokens
            .zip(self.usage.output_tokens)
            .and_then(|(input, output)| input.checked_add(output))
    }

    pub fn work_label(&self) -> String {
        if self
            .providers
            .iter()
            .any(|provider| provider.task_id != self.task_id || provider.wave_id != self.wave_id)
        {
            return "mixed/unknown · see provider history".into();
        }
        if let Some(task) = &self.task_id {
            return format!(
                "task/{}",
                self.task_identifier.as_deref().unwrap_or(task.as_str())
            );
        }
        self.wave_id
            .as_ref()
            .map(|wave| {
                format!(
                    "wave/{}",
                    self.wave_name.as_deref().unwrap_or(wave.as_str())
                )
            })
            .unwrap_or_else(|| "-".into())
    }
}

/// The part of one capture's event stream that SQLite history has yet to keep.
///
/// `events.jsonl` holds every provider increment verbatim. History keeps what
/// the increments add up to: a run of deltas becomes one event at the first
/// delta's position, a Turn's cumulative diff is kept once, and the raw
/// notification behind each increment stays in the file alone. Storing one row
/// per streamed token made increments two thirds of all history rows.
/// A process that dies mid-run leaves that run in the file only.
#[derive(Debug, Default)]
struct StreamedHistory {
    run: Option<EventEnvelope>,
    diff: Option<EventEnvelope>,
}

impl StreamedHistory {
    /// The events history keeps now that `event` has arrived. Readers order
    /// history by capture position, so a held event may be kept late.
    fn admit(&mut self, event: EventEnvelope) -> Vec<EventEnvelope> {
        let conversation = match &event.event {
            CaptureEvent::ProviderOutput { stream, line }
                if stream == "notification" && is_provider_increment(line) =>
            {
                return Vec::new();
            }
            CaptureEvent::Conversation { event } => Some(&**event),
            _ => None,
        };
        match conversation {
            Some(ConversationEvent::DiffUpdated { turn_id, .. }) => {
                let earlier_turn = self
                    .diff
                    .take()
                    .filter(|held| diff_turn(held) != Some(turn_id));
                self.diff = Some(event);
                earlier_turn.into_iter().collect()
            }
            Some(
                increment @ (ConversationEvent::TextDelta { .. }
                | ConversationEvent::ReasoningDelta { .. }
                | ConversationEvent::ItemUpdated { .. }),
            ) => {
                if self
                    .run
                    .as_mut()
                    .is_some_and(|run| extend_run(run, increment))
                {
                    return Vec::new();
                }
                self.run.replace(event).into_iter().collect()
            }
            Some(ConversationEvent::TurnCompleted { .. }) => {
                let mut kept = self.settle();
                kept.push(event);
                kept
            }
            _ => self.run.take().into_iter().chain([event]).collect(),
        }
    }

    /// Everything still held.
    fn settle(&mut self) -> Vec<EventEnvelope> {
        self.run
            .take()
            .into_iter()
            .chain(self.diff.take())
            .collect()
    }
}

fn diff_turn(held: &EventEnvelope) -> Option<&String> {
    match &held.event {
        CaptureEvent::Conversation { event } => match &**event {
            ConversationEvent::DiffUpdated { turn_id, .. } => Some(turn_id),
            _ => None,
        },
        _ => None,
    }
}

fn extend_run(run: &mut EventEnvelope, increment: &ConversationEvent) -> bool {
    let CaptureEvent::Conversation { event } = &mut run.event else {
        return false;
    };
    match (&mut **event, increment) {
        (
            ConversationEvent::TextDelta { turn_id, content },
            ConversationEvent::TextDelta {
                turn_id: turn,
                content: more,
            },
        )
        | (
            ConversationEvent::ReasoningDelta { turn_id, content },
            ConversationEvent::ReasoningDelta {
                turn_id: turn,
                content: more,
            },
        ) if turn_id == turn => content.push_str(more),
        (
            ConversationEvent::ItemUpdated {
                turn_id,
                item_id,
                data,
            },
            ConversationEvent::ItemUpdated {
                turn_id: turn,
                item_id: item,
                data: more,
            },
        ) if turn_id == turn && item_id == item => match (data, more) {
            (ItemDelta::Output { content }, ItemDelta::Output { content: more })
            | (ItemDelta::PlanText { content }, ItemDelta::PlanText { content: more }) => {
                content.push_str(more)
            }
            _ => return false,
        },
        _ => return false,
    }
    true
}

/// A provider notification that only adds to, or restates, a later complete one:
/// `…/delta`, `…Delta`, `…_delta` and the cumulative Turn diff.
fn is_provider_increment(line: &str) -> bool {
    #[derive(Deserialize)]
    struct Notification {
        method: String,
    }
    serde_json::from_str::<Notification>(line).is_ok_and(|notification| {
        notification.method.ends_with("elta") || notification.method == "turn/diff/updated"
    })
}

#[derive(Debug)]
enum RecorderMessage {
    Event(EventEnvelope),
    Drain(mpsc::Sender<()>),
}

#[derive(Debug)]
struct SessionRecorder {
    sender: Option<SyncSender<RecorderMessage>>,
}

impl SessionRecorder {
    fn start(dir: &Path, manifest: &SessionCaptureManifest) -> Self {
        let artifact_key = &manifest.artifact_key;
        let history = if manifest.harness == "loopflow" {
            Ok(None)
        } else {
            row_store(dir).and_then(|store| {
                let session = store.session_for_artifact(artifact_key)?;
                Ok(session.map(|session| (store, session)))
            })
        };
        let manifest = manifest.clone();
        let (sender, receiver) = mpsc::sync_channel(256);
        let writer_dir = dir.to_path_buf();
        let writer_artifact_key = artifact_key.clone();
        let thread = std::thread::Builder::new()
            .name(format!("lf-session-recorder-{}", &artifact_key.as_str()[..8]))
            .spawn(move || {
                let mut warned = false;
                let history = match history {
                    Ok(history) => history,
                    Err(error) => {
                        tracing::warn!(%error, "Session history unavailable");
                        None
                    }
                };
                let observe = |source: String, at: OffsetDateTime, evidence: serde_json::Value| {
                    let Some((store, session)) = &history else { return Ok(()) };
                    store.retain_session_observation(session, &crate::session::SessionObservation {
                        artifact_key: writer_artifact_key.clone(), source: source.clone(),
                        observed_at: at.unix_timestamp(), task_id: session.task_id.clone(),
                        wave_id: session.wave_id.clone(),
                        payload: serde_json::json!({"input_id": writer_artifact_key, "source": source, "evidence": evidence}),
                    }).map_err(std::io::Error::other)
                };
                if let Err(error) = observe("manifest.json".into(), manifest.created_at, serde_json::json!(manifest)) {
                    tracing::warn!(%error, "Session capture observation unavailable");
                }
                let retain = |events: Vec<EventEnvelope>| {
                    events.into_iter().try_for_each(|event| {
                        observe(format!("events.jsonl:{}", event.seq), event.observed_at, serde_json::json!(event))
                    })
                };
                let mut streamed = StreamedHistory::default();
                while let Ok(message) = receiver.recv() {
                    let result = match message {
                        RecorderMessage::Event(event) => {
                            let result = append_json_line(&writer_dir.join("events.jsonl"), &event);
                            result.and(retain(streamed.admit(event)))
                        }
                        RecorderMessage::Drain(acknowledge) => {
                            let result = retain(streamed.settle()).and(sync_telemetry(&writer_dir));
                            let _ = acknowledge.send(());
                            result
                        }
                    };
                    if let Err(error) = result {
                        if !warned {
                            tracing::warn!(
                                %error,
                                artifact_key = %writer_artifact_key,
                                "Session recorder lost telemetry; harness execution continues"
                            );
                            warned = true;
                        } else {
                            tracing::debug!(%error, artifact_key = %writer_artifact_key, "Session recorder telemetry write failed");
                        }
                    }
                }
                if let Err(error) = retain(streamed.settle()) {
                    tracing::debug!(%error, artifact_key = %writer_artifact_key, "Session recorder telemetry write failed");
                }
            });
        match thread {
            Ok(_) => Self {
                sender: Some(sender),
            },
            Err(error) => {
                tracing::warn!(
                    %error,
                    artifact_key = %artifact_key,
                    "Session recorder unavailable; harness execution continues"
                );
                Self { sender: None }
            }
        }
    }

    fn record(&self, message: RecorderMessage) -> std::io::Result<()> {
        let Some(sender) = &self.sender else {
            return Err(std::io::Error::other("Session recorder is unavailable"));
        };
        sender.try_send(message).map_err(|error| match error {
            TrySendError::Full(_) => std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "Session recorder queue is full",
            ),
            TrySendError::Disconnected(_) => {
                std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Session recorder stopped")
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

/// Native notifications and recorder checkpoints describe the same turn. Prefer
/// a final recorder receipt; otherwise supplement its stream instead of adding
/// the provider's thread lifetime total as another measurement.
fn recover_native_usage(
    events: &mut Vec<EventEnvelope>,
    history: &[crate::session::SessionEvent],
) -> usize {
    use crate::session::SessionEventKind;

    let mut turns = BTreeMap::<(&str, &str), Vec<&crate::session::SessionEvent>>::new();
    for event in history {
        if event.kind != SessionEventKind::Observed {
            if let (Some(thread), Some(turn)) = (
                event.provider_thread.as_deref(),
                event.provider_turn.as_deref(),
            ) {
                turns.entry((thread, turn)).or_default().push(event);
            }
        }
    }
    let threads: HashMap<_, _> = events
        .iter()
        .filter_map(|event| match &event.event {
            CaptureEvent::ProviderSessionObserved {
                attempt_key,
                provider_session_id,
            } => Some((attempt_key.clone(), provider_session_id.clone())),
            _ => None,
        })
        .collect();
    let recorder_len = events.len();
    let mut gaps = 0;
    for ((thread, turn), native) in turns {
        let receipts: Vec<_> = native
            .iter()
            .filter(|event| event.kind == SessionEventKind::Usage)
            .collect();
        let Some(first) = receipts.first() else {
            continue;
        };
        let provider = if first.payload["provider"] == "opencode" {
            "opencode"
        } else {
            "codex"
        };
        let matching: Vec<_> = events[..recorder_len]
            .iter()
            .filter_map(|event| match &event.event {
                CaptureEvent::Usage {
                    provider: recorded_provider,
                    attempt_key,
                    turn_key,
                    usage_stream_id,
                    observation_seq,
                    final_receipt,
                    ..
                } if recorded_provider == provider
                    && turn_key == turn
                    && threads.get(attempt_key).is_none_or(|known| known == thread) =>
                {
                    Some((
                        usage_stream_id.clone(),
                        *observation_seq,
                        *final_receipt,
                        threads.get(attempt_key).map(String::as_str) == Some(thread),
                    ))
                }
                _ => None,
            })
            .collect();
        // A turn label alone cannot correlate a different or unknown thread.
        if matching.iter().any(|receipt| !receipt.3)
            || matching.iter().any(|receipt| receipt.0 != matching[0].0)
        {
            gaps += 1;
            continue;
        }
        if matching.iter().any(|receipt| receipt.2) {
            continue;
        }
        if provider == "opencode" {
            // Each completed assistant message owns a native usage counter.
            // Preserve snapshots for the existing max/missingness reducer;
            // tool-call and terminal assistants are distinct model calls.
            for receipt in receipts {
                let info = &receipt.payload["message"];
                let Some(message) = info["id"].as_str() else {
                    gaps += 1;
                    continue;
                };
                let tokens = &info["tokens"];
                let input = tokens["input"].as_u64();
                let cached = tokens["cache"]["read"].as_u64();
                let written = tokens["cache"]["write"].as_u64();
                let total = input
                    .zip(cached)
                    .zip(written)
                    .and_then(|((a, b), c)| a.checked_add(b)?.checked_add(c));
                let usage = TurnUsage {
                    input_tokens: input,
                    output_tokens: tokens["output"].as_u64(),
                    total_input_tokens: total,
                    peak_input_tokens: total,
                    context_window_tokens: None,
                    reasoning_tokens: tokens["reasoning"].as_u64(),
                    cache_read_tokens: cached,
                    cache_write_tokens: written,
                    model: info["modelID"].as_str().map(str::to_owned),
                    cost_usd: info["cost"].as_f64(),
                };
                let Ok(observed_at) = OffsetDateTime::from_unix_timestamp(receipt.observed_at)
                else {
                    gaps += 1;
                    continue;
                };
                // A recorder's partial request total cannot be combined with
                // independent message totals without double counting.
                if !matching.is_empty() {
                    gaps += 1;
                    break;
                }
                events.push(EventEnvelope {
                    schema_version: SCHEMA_VERSION,
                    seq: events
                        .iter()
                        .map(|event| event.seq)
                        .max()
                        .map_or(0, |seq| seq + 1),
                    observed_at,
                    event: CaptureEvent::Usage {
                        usage_stream_id: format!("native:{thread}:{turn}:{message}"),
                        provider: provider.into(),
                        model: usage.model.clone(),
                        attempt_key: String::new(),
                        turn_key: turn.into(),
                        observation_seq: receipt.seq as u64,
                        counter_kind: "cumulative".into(),
                        start_known: native
                            .iter()
                            .any(|event| event.kind == SessionEventKind::Started),
                        final_receipt: true,
                        usage: Box::new(usage),
                    },
                });
            }
            continue;
        }
        let field = |value: &serde_json::Value, part: &str, key: &str| value[part][key].as_u64();
        // `last` is one request, not a complete turn. Without earlier snapshots,
        // retain that lower bound and its following deltas, explicitly partial.
        let start = native
            .iter()
            .find(|event| event.kind == SessionEventKind::Started);
        let previous = start
            .and_then(|start| {
                history
                    .iter()
                    .filter(|event| {
                        event.kind != SessionEventKind::Observed
                            && event.provider_thread.as_deref() == Some(thread)
                            && event.provider_turn.as_deref() != Some(turn)
                            && event.seq < start.seq
                    })
                    .max_by_key(|event| event.seq)
            })
            .filter(|previous| {
                history.iter().any(|event| {
                    event.kind == SessionEventKind::Completed
                        && event.provider_thread == previous.provider_thread
                        && event.provider_turn == previous.provider_turn
                        && event.seq < start.expect("previous turn requires a start").seq
                })
            });
        // Only the immediately preceding turn can supply a baseline. A missing
        // usage notification cannot make an older turn stand in for it.
        // A retained predecessor receipt supplies the baseline when reconnect
        // misses the first request. Otherwise only the observed suffix is known.
        let prior_total = |key: &str| {
            previous.and_then(|previous| {
                history
                    .iter()
                    .filter(|event| {
                        event.kind == SessionEventKind::Usage
                            && event.provider_thread == previous.provider_thread
                            && event.provider_turn == previous.provider_turn
                            && event.seq <= previous.seq
                    })
                    .filter_map(|event| field(&event.payload, "total", key))
                    .max()
            })
        };
        let baseline = |key: &str| {
            prior_total(key).or_else(|| {
                field(&first.payload, "total", key)?.checked_sub(field(
                    &first.payload,
                    "last",
                    key,
                )?)
            })
        };
        let known = start.is_some()
            && [
                "inputTokens",
                "outputTokens",
                "cachedInputTokens",
                "reasoningOutputTokens",
            ]
            .iter()
            .all(|key| prior_total(key).is_some() || baseline(key) == Some(0));
        let (stream, seq) = matching
            .iter()
            .max_by_key(|receipt| receipt.1)
            .map(|receipt| (receipt.0.clone(), receipt.1 + 1))
            .unwrap_or_else(|| (format!("native:{thread}:{turn}"), 0));
        for (index, last) in receipts.iter().enumerate() {
            let Ok(observed_at) = OffsetDateTime::from_unix_timestamp(last.observed_at) else {
                gaps += 1;
                continue;
            };
            let delta = |key: &str| field(&last.payload, "total", key)?.checked_sub(baseline(key)?);
            let gross = delta("inputTokens");
            let cached = delta("cachedInputTokens");
            let usage = TurnUsage {
                input_tokens: gross
                    .zip(cached)
                    .and_then(|(gross, cached)| gross.checked_sub(cached)),
                output_tokens: delta("outputTokens"),
                total_input_tokens: gross,
                peak_input_tokens: field(&last.payload, "last", "inputTokens"),
                context_window_tokens: last.payload["modelContextWindow"].as_u64(),
                reasoning_tokens: delta("reasoningOutputTokens"),
                cache_read_tokens: cached,
                cache_write_tokens: delta("cacheWriteInputTokens"),
                model: None,
                cost_usd: None,
            };
            events.push(EventEnvelope {
                schema_version: SCHEMA_VERSION,
                seq: events
                    .iter()
                    .map(|event| event.seq)
                    .max()
                    .map_or(0, |seq| seq + 1),
                observed_at,
                event: CaptureEvent::Usage {
                    usage_stream_id: stream.clone(),
                    provider: "codex".into(),
                    model: None,
                    attempt_key: String::new(),
                    turn_key: turn.into(),
                    observation_seq: seq + index as u64,
                    counter_kind: "cumulative".into(),
                    start_known: known,
                    // Completion does not include a final usage receipt in the native schema.
                    final_receipt: false,
                    usage: Box::new(usage),
                },
            });
        }
    }
    gaps
}

fn providers_first_start(history: &[crate::session::SessionEvent]) -> Option<i64> {
    history
        .iter()
        .filter(|event| event.kind == crate::session::SessionEventKind::Started)
        .map(|event| event.observed_at)
        .min()
}

fn project_provider_history(
    captured: Option<i64>,
    input: Option<&str>,
    envelopes: &[EventEnvelope],
    history: &[crate::session::SessionEvent],
) -> std::io::Result<Vec<ProviderHistory>> {
    use crate::session::SessionEventKind;
    let mut native = BTreeMap::<(&str, &str), Vec<&crate::session::SessionEvent>>::new();
    for event in history
        .iter()
        .filter(|event| event.kind != SessionEventKind::Observed)
    {
        if let (Some(thread), Some(turn)) = (
            event.provider_thread.as_deref(),
            event.provider_turn.as_deref(),
        ) {
            native.entry((thread, turn)).or_default().push(event);
        }
    }
    let threads: HashMap<_, _> = envelopes
        .iter()
        .filter_map(|event| match &event.event {
            CaptureEvent::ProviderSessionObserved {
                attempt_key,
                provider_session_id,
            } => Some((attempt_key.as_str(), provider_session_id.as_str())),
            _ => None,
        })
        .collect();
    let matches_native = |attempt: &str, turn: &str| {
        threads
            .get(attempt)
            .is_some_and(|thread| native.contains_key(&(*thread, turn)))
    };
    let mut records = Vec::new();
    for ((thread, turn), events) in &native {
        let start = events
            .iter()
            .find(|event| event.kind == SessionEventKind::Started);
        let completed = events
            .iter()
            .find(|event| event.kind == SessionEventKind::Completed);
        let usage = reduce_usage_events(envelopes.iter().filter(|event| match &event.event {
            CaptureEvent::Usage {
                attempt_key,
                turn_key,
                usage_stream_id,
                ..
            } => turn_key.as_str() == *turn
                && (threads.get(attempt_key.as_str()) == Some(thread)
                    || (attempt_key.is_empty()
                        && (usage_stream_id == &format!("native:{thread}:{turn}")
                            || usage_stream_id.starts_with(&format!("native:{thread}:{turn}:"))))),
            _ => false,
        }))
        .usage;
        records.push(ProviderHistory {
            reference: ProviderHistoryReference::NativeTurn {
                thread: (*thread).into(),
                turn: (*turn).into(),
                start_seq: start.map(|event| event.seq),
                completion_seq: completed.map(|event| event.seq),
            },
            exec_id: start
                .and_then(|event| event.exec_id.as_deref())
                .map(crate::id::ExecId::parse)
                .transpose()
                .map_err(std::io::Error::other)?,
            task_id: start
                .and_then(|event| event.task_id.as_deref())
                .map(crate::durable::TaskId::parse)
                .transpose()
                .map_err(std::io::Error::other)?,
            wave_id: start
                .and_then(|event| event.wave_id.as_deref())
                .map(crate::id::WaveId::parse)
                .transpose()
                .map_err(std::io::Error::other)?,
            started_at: start.map(|event| event.observed_at),
            completed_at: completed.map(|event| event.observed_at),
            outcome: completed
                .and_then(|event| event.payload["status"].as_str())
                .map(str::to_owned),
            usage,
        });
    }
    // Old provider records have no reliable Exec or native-turn identity. Keep
    // their own key and result, never substitute the enclosing recorder exit.
    let mut attempts = BTreeMap::<&str, Vec<&EventEnvelope>>::new();
    for event in envelopes {
        let key = match &event.event {
            CaptureEvent::ProviderAttemptStarted { attempt_key, .. }
            | CaptureEvent::ProviderAttemptFinished { attempt_key, .. }
            | CaptureEvent::Usage { attempt_key, .. }
                if !attempt_key.is_empty() =>
            {
                attempt_key
            }
            _ => continue,
        };
        attempts.entry(key).or_default().push(event);
    }
    for (attempt, events) in attempts {
        // The recorded attempt's outcome remains evidence even with native history;
        // its usage excludes exactly correlated native streams to avoid double count.
        let start = events
            .iter()
            .find(|event| matches!(event.event, CaptureEvent::ProviderAttemptStarted { .. }));
        let finished = events.iter().find_map(|event| match &event.event {
            CaptureEvent::ProviderAttemptFinished { outcome, .. } => {
                Some((event.observed_at.unix_timestamp(), outcome.clone()))
            }
            _ => None,
        });
        let usage =
            reduce_usage_events(events.iter().copied().filter(|event| match &event.event {
                CaptureEvent::Usage { turn_key, .. } => !matches_native(attempt, turn_key),
                _ => false,
            }))
            .usage;
        let origin = history.iter().find(|event| {
            event.kind == SessionEventKind::Observed
                && event.payload["input_id"].as_str() == input
                && event.payload["source"] == "manifest.json"
        });
        records.push(ProviderHistory {
            reference: ProviderHistoryReference::RecordedAttempt {
                captured: captured.ok_or_else(|| {
                    std::io::Error::other("recorded provider evidence has no captured event")
                })?,
                attempt_key: attempt.into(),
            },
            exec_id: None,
            task_id: origin
                .and_then(|event| event.task_id.as_deref())
                .map(crate::durable::TaskId::parse)
                .transpose()
                .map_err(std::io::Error::other)?,
            wave_id: origin
                .and_then(|event| event.wave_id.as_deref())
                .map(crate::id::WaveId::parse)
                .transpose()
                .map_err(std::io::Error::other)?,
            started_at: start.map(|event| event.observed_at.unix_timestamp()),
            completed_at: finished.as_ref().map(|(at, _)| *at),
            outcome: finished.map(|(_, outcome)| outcome),
            usage,
        });
    }
    records.sort_by_key(|record| record.started_at.or(record.completed_at));
    Ok(records)
}

/// Project typed input and provider history from the existing Session owner.
/// No current assignment, provider or completion can rewrite an earlier input.
pub(crate) fn project_input_history(
    session: &crate::session::HistoryCapture,
    artifact_key: Option<&str>,
    history: &[crate::session::SessionEvent],
    names: (Option<String>, Option<String>),
) -> std::io::Result<SessionHistory> {
    let mut events = Vec::new();
    let mut terminal: Option<TerminalReceipt> = None;
    let mut manifest: Option<SessionCaptureManifest> = None;
    let mut gaps = 0;
    for event in history {
        if event.kind != crate::session::SessionEventKind::Observed {
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
        } else if let Some(ordinal) = source.strip_prefix("events.jsonl:") {
            match (
                ordinal.parse::<u64>(),
                serde_json::from_value::<EventEnvelope>(evidence.clone()),
            ) {
                (Ok(ordinal), Ok(envelope)) => events.push((ordinal, envelope)),
                _ => gaps += 1,
            }
        }
    }
    // Keep event order for first-attempt timing and cumulative evidence.
    events.sort_by_key(|(ordinal, _)| *ordinal);
    let mut events: Vec<_> = events.into_iter().map(|(_, envelope)| envelope).collect();
    gaps += recover_native_usage(&mut events, history);
    let providers = project_provider_history(session.captured, artifact_key, &events, history)?;
    let evidence = reduce_usage_events(&events);
    gaps += evidence.gaps + usize::from(manifest.is_none());
    let current = session.captured.is_some() && session.current_capture == session.captured;
    Ok(SessionHistory {
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
        task_pr_id: manifest.as_ref().and_then(|manifest| match &manifest.flow {
            Some(SessionFlowMembership::Step(step)) => step.task_pr_id.clone(),
            _ => None,
        }),
        repo: manifest
            .as_ref()
            .and_then(|m| m.repo.as_ref().map(|p| p.to_string_lossy().into_owned()))
            .or_else(|| current.then(|| session.repo.clone()).flatten()),
        worktree: manifest
            .as_ref()
            .map(|m| m.cwd.to_string_lossy().into_owned())
            .or_else(|| current.then(|| session.cwd.to_string_lossy().into_owned())),
        skill: manifest
            .as_ref()
            .map(|m| m.skill.clone())
            .unwrap_or_else(|| current.then(|| session.skill.clone()).flatten()),
        recorded_outcome: terminal.as_ref().map(|receipt| receipt.outcome.clone()),
        recorded_at: terminal
            .as_ref()
            .map(|receipt| receipt.ended_at.unix_timestamp()),
        observed_at: session.observed_at,
        first_provider_attempt_at: providers_first_start(history)
            .into_iter()
            .chain(evidence.first_provider_attempt_at)
            .min(),
        usage: evidence.usage,
        evidence_gaps: gaps,
        harness: manifest
            .as_ref()
            .map(|m| m.harness.clone())
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

pub(crate) fn resolve_manifest(
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
    let store = crate::store::sqlite::SqliteStore::open_execs_read_only(&database)
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
        .filter(|session| session.id == artifact)
        .map(|session| session.artifact_key)
        .unwrap_or(artifact);
    let dir = record_dir(lf_home, &artifact).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid capture key")
    })?;
    let manifest = read_manifest(&dir)?;
    validate_manifest_path(&dir, &manifest)?;
    Ok((dir, manifest))
}

pub(crate) fn read_provider_session(dir: &Path) -> std::io::Result<Option<ProviderSessionRef>> {
    let input = input_id_from_dir(dir)?;
    crate::store::sqlite::SqliteStore::open_execs_read_only(
        &row_database(dir).map_err(std::io::Error::other)?,
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

/// Read the current conversation’s published provider identity.
pub(crate) fn provider_session_from_history(
    history: impl IntoIterator<Item = serde_json::Value>,
) -> std::io::Result<Option<ProviderSessionRef>> {
    let mut published = None;
    let mut events = Vec::new();
    for observation in history {
        let Some(source) = observation["source"].as_str() else {
            continue;
        };
        let evidence = &observation["evidence"];
        if source.starts_with("provider-session:") {
            published = Some(evidence.clone());
        } else if let Some(ordinal) = source.strip_prefix("events.jsonl:") {
            let ordinal = ordinal.parse::<u64>().map_err(std::io::Error::other)?;
            events.push((ordinal, evidence.clone()));
        }
    }
    if let Some(reference) = published {
        let reference: ProviderSessionRef =
            serde_json::from_value(reference).map_err(std::io::Error::other)?;
        reference.validate()?;
        return Ok(Some(reference));
    }
    events.sort_by_key(|(ordinal, _)| *ordinal);
    provider_session_from_events(events.into_iter().map(|(_, event)| event).collect())
}

fn provider_session_from_events(
    events: Vec<serde_json::Value>,
) -> std::io::Result<Option<ProviderSessionRef>> {
    let mut provider_session = None;
    let mut accounts = HashMap::new();
    for event in events {
        let envelope: EventEnvelope =
            serde_json::from_value(event).map_err(std::io::Error::other)?;
        if envelope.schema_version != SCHEMA_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unsupported capture event schema",
            ));
        }
        match envelope.event {
            CaptureEvent::ProviderAttemptStarted {
                attempt_key,
                account_id,
                ..
            }
            | CaptureEvent::ProviderAccountSelected {
                attempt_key,
                account_id,
            } => {
                accounts.insert(attempt_key, account_id);
            }
            CaptureEvent::ProviderSessionObserved {
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

/// Reduce retained observations; only a successful provider turn supplies a conclusion.
pub(crate) fn final_answer(events: Vec<serde_json::Value>) -> std::io::Result<Option<FinalAnswer>> {
    let mut turns = HashMap::<String, TurnProse>::new();
    let mut answer = None;
    for event in events {
        let envelope: EventEnvelope =
            serde_json::from_value(event).map_err(std::io::Error::other)?;
        if envelope.schema_version != SCHEMA_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unsupported capture event schema",
            ));
        }
        let CaptureEvent::Conversation { event } = envelope.event else {
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
    let input = input_id_from_dir(dir)?;
    let store = row_store(dir).map_err(std::io::Error::other)?;
    let session = store
        .session_for_artifact(&input)
        .map_err(std::io::Error::other)?
        .ok_or_else(|| std::io::Error::other("provider history has no admitted Session"))?;
    let source = format!("provider-session:{}", Uuid::new_v4());
    store.retain_session_observation(&session, &crate::session::SessionObservation {
        artifact_key: input.clone(), source: source.clone(),
        observed_at: OffsetDateTime::now_utc().unix_timestamp(),
        task_id: session.task_id.clone(), wave_id: session.wave_id.clone(),
        payload: serde_json::json!({"input_id":input,"source":source,"evidence":ProviderSessionRef {
            schema_version: SCHEMA_VERSION,
            provider_session_id: provider_session_id.to_string(), account_id,
        }}),
    }).map_err(std::io::Error::other)
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
    let input = input_id_from_dir(dir)?;
    if row_store(dir)
        .map_err(std::io::Error::other)?
        .session_for_artifact(&input)
        .map_err(std::io::Error::other)?
        .is_none()
    {
        return Err(std::io::Error::other(
            "provider client has no admitted Session",
        ));
    }
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

pub(crate) fn read_manifest(dir: &Path) -> std::io::Result<SessionCaptureManifest> {
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

#[derive(Debug)]
struct CaptureEvidence {
    usage: SessionUsage,
    gaps: usize,
    first_provider_attempt_at: Option<i64>,
}

#[cfg(test)]
fn reduce_usage_reader(mut reader: impl BufRead) -> std::io::Result<CaptureEvidence> {
    let mut events = Vec::new();
    let mut trailing_gap = 0;
    loop {
        let mut line = Vec::new();
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        if line.last() != Some(&b'\n') {
            trailing_gap += 1;
            break;
        }
        events.push(
            serde_json::from_slice::<EventEnvelope>(&line).map_err(|error| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("malformed complete capture event: {error}"),
                )
            })?,
        );
    }
    let mut evidence = reduce_usage_events(&events);
    evidence.gaps += trailing_gap;
    Ok(evidence)
}

fn reduce_usage_events<'a>(events: impl IntoIterator<Item = &'a EventEnvelope>) -> CaptureEvidence {
    let mut streams = BTreeMap::<String, UsageStream>::new();
    let mut gaps = 0;
    let mut envelope_seq = None;
    let mut first_provider_attempt_at = None;
    for envelope in events {
        if envelope.schema_version != SCHEMA_VERSION {
            gaps += 1;
        }
        if envelope_seq.is_some_and(|previous| envelope.seq <= previous) {
            gaps += 1;
        }
        envelope_seq = Some(envelope_seq.map_or(envelope.seq, |seen| seen.max(envelope.seq)));
        if first_provider_attempt_at.is_none()
            && envelope.schema_version == SCHEMA_VERSION
            && matches!(&envelope.event, CaptureEvent::ProviderAttemptStarted { .. })
        {
            first_provider_attempt_at = Some(envelope.observed_at.unix_timestamp());
        }
        let (usage_stream_id, observation_seq, counter_kind, start_known, final_receipt, usage) =
            match &envelope.event {
                CaptureEvent::Usage {
                    usage_stream_id,
                    observation_seq,
                    counter_kind,
                    start_known,
                    final_receipt,
                    usage,
                    ..
                } => (
                    usage_stream_id.clone(),
                    *observation_seq,
                    counter_kind.as_str(),
                    *start_known,
                    *final_receipt,
                    usage,
                ),
                CaptureEvent::Unknown => {
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
    let usage = SessionUsage {
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
    CaptureEvidence {
        usage,
        gaps,
        first_provider_attempt_at,
    }
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
pub(crate) struct CaptureHandle(Arc<Mutex<SessionCapture>>);

pub(crate) fn register_session_driver_interrupt(
    store: &crate::store::sqlite::SqliteStore,
    session: String,
    driver: crate::exec::SessionDriver,
) {
    let store = store.clone();
    crate::engine::agent::register_interrupt_cleanup(move || {
        match finish_session_driver(&store, &session, &driver, "interrupted") {
            Ok(()) | Err(StoreError::InvalidAuthority(_)) => {}
            Err(error) => tracing::warn!(%error, %session, "record interrupted Session connection"),
        }
    });
}

impl CaptureHandle {
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
        // This is not a CaptureHandle: dropping preparation must not settle a
        // Session whose provider has never been launched.
        Ok(id)
    }

    pub(crate) fn start_prepared(
        home: &Path,
        id: &str,
        spec: SessionCaptureSpec,
        context: &crate::trace::PreparedTurnContext,
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
        manifest.context = Some(SessionContextRef {
            path: "context.json".to_string(),
            content_sha256: hex::encode(Sha256::digest(&bytes)),
            bytes: bytes.len() as u64,
        });
        // Finalize exec provenance on the existing identity. Preparation did
        // not freeze a prompt, runtime, or model selection before exec.
        manifest.harness = spec.harness;
        manifest.model = spec.model;
        manifest.surface = spec.surface;
        manifest.cwd = spec.cwd;
        manifest.repo = spec.repo;
        manifest.worktree = spec.worktree;
        manifest.skill = spec.skill;
        // Preparation owns Work attribution and a human Flow step's membership;
        // the child's Task prompt context cannot reassign that captured input.
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
        Ok(Self(Arc::new(Mutex::new(SessionCapture::from_manifest(
            manifest, dir,
        )))))
    }

    pub(crate) fn begin_with_request(
        spec: SessionCaptureSpec,
        exec: AgentExecRequest,
    ) -> StoreResult<Self> {
        let context =
            crate::trace::PreparedTurnContext::from_prompts(&exec.system_prompt, &exec.task_prompt);
        Self::begin_with_context(spec, &context, Some(exec))
    }

    pub(crate) fn begin_with_context(
        spec: SessionCaptureSpec,
        context: &crate::trace::PreparedTurnContext,
        exec: Option<AgentExecRequest>,
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
            exec,
            Some(context),
        )
    }

    pub(crate) fn begin_reserved_with_context(
        spec: SessionCaptureSpec,
        artifact_key: String,
        exec: Option<AgentExecRequest>,
        context: &crate::trace::PreparedTurnContext,
        publish: impl FnOnce(&String) -> StoreResult<()>,
    ) -> StoreResult<Self> {
        let home = crate::store::lf_home_dir();
        let caller = inherited_capture_key()?;
        Self::begin_reserved_at(&home, spec, artifact_key, caller, exec, context, publish)
    }

    fn begin_reserved_at(
        home: &Path,
        spec: SessionCaptureSpec,
        artifact_key: String,
        caller: Option<String>,
        exec: Option<AgentExecRequest>,
        context: &crate::trace::PreparedTurnContext,
        publish: impl FnOnce(&String) -> StoreResult<()>,
    ) -> StoreResult<Self> {
        let (manifest, context) = prepare_manifest(spec, artifact_key, caller, exec, Some(context))
            .map_err(record_error)?;
        let (manifest, dir) = reconcile_reserved_manifest(home, manifest, context.as_deref())
            .map_err(record_error)?;
        // Only the reservation transaction grants exec authority. A rejected
        // publication must not start a recorder or settle somebody else's capture.
        publish(&manifest.artifact_key)?;
        Ok(Self(Arc::new(Mutex::new(SessionCapture::from_manifest(
            manifest, dir,
        )))))
    }

    /// Retain the source key already resolved by the replay reader.
    pub(crate) fn begin_replay_at(
        lf_home: &Path,
        spec: SessionCaptureSpec,
        exec: AgentExecRequest,
        caller_artifact_key: String,
    ) -> StoreResult<Self> {
        let context =
            crate::trace::PreparedTurnContext::from_prompts(&exec.system_prompt, &exec.task_prompt);
        Self::begin_at_with_id(
            lf_home,
            spec,
            new_artifact_key(),
            Some(caller_artifact_key),
            Some(exec),
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
        exec: AgentExecRequest,
    ) -> StoreResult<Self> {
        let context =
            crate::trace::PreparedTurnContext::from_prompts(&exec.system_prompt, &exec.task_prompt);
        Self::begin_at_with_id(
            lf_home,
            spec,
            new_artifact_key(),
            inherited_capture_key()?,
            Some(exec),
            Some(&context),
        )
    }

    fn begin_at_with_id(
        lf_home: &Path,
        spec: SessionCaptureSpec,
        artifact_key: String,
        caller_artifact_key: Option<String>,
        exec: Option<AgentExecRequest>,
        context: Option<&crate::trace::PreparedTurnContext>,
    ) -> StoreResult<Self> {
        let work = spec.work.clone();
        let (manifest, context_bytes) =
            prepare_manifest(spec, artifact_key, caller_artifact_key, exec, context)
                .map_err(record_error)?;
        let dir = record_dir(lf_home, &manifest.artifact_key).expect("artifact key is a UUID");
        let reserved = SessionCapture::record_row(&manifest, &dir, work)?;
        publish_manifest(lf_home, &manifest, context_bytes.as_deref()).map_err(record_error)?;
        if let Some(session) = reserved {
            row_store(&dir)?.publish_capture(&session.id, session.captured)?;
        }
        Ok(Self(Arc::new(Mutex::new(SessionCapture::from_manifest(
            manifest, dir,
        )))))
    }

    pub(crate) fn artifact_key(&self) -> String {
        self.0
            .lock()
            .expect("Session capture mutex poisoned")
            .manifest
            .artifact_key
            .clone()
    }

    pub(crate) fn artifact_dir(&self) -> PathBuf {
        self.0
            .lock()
            .expect("Session capture mutex poisoned")
            .dir
            .clone()
    }

    /// Claim an admitted conversation and retain the exact provider provenance
    /// used by its tools. A later driver transfer never rewrites this exec.
    pub(crate) fn claim_conversation_driver(&self) -> StoreResult<()> {
        let Some(exec_id) = crate::journal::current_exec_id() else {
            if crate::journal::is_cli_process() {
                return Err(StoreError::InvalidAuthority(
                    "agent Exec requires an admitted Exec; command observation failed".into(),
                ));
            }
            // Library callers outside an actual lf process have no Exec to name.
            return Ok(());
        };
        let mut capture = self.0.lock().expect("Session capture mutex poisoned");
        if capture.driver.is_some() {
            return Ok(());
        }
        let store = row_store(&capture.dir)?;
        let Some(session) = store.session_for_artifact(&capture.manifest.artifact_key)? else {
            if crate::journal::is_cli_process() {
                return Err(StoreError::InvalidAuthority(
                    "agent Exec requires an admitted conversation".into(),
                ));
            }
            return Ok(());
        };
        let expected = store.session_driver(&session.id)?;
        if let Some(exec) = expected.as_ref().and_then(|driver| driver.exec_id.as_ref()) {
            let receipt =
                crate::journal::read_exec_process_receipts_at(&crate::store::lf_home_dir())
                    .ok()
                    .and_then(|receipts| {
                        receipts
                            .into_iter()
                            .find(|receipt| receipt.exec_id == exec.as_str())
                    });
            let dead = receipt.is_some_and(|receipt| {
                match crate::journal::process_started_at(receipt.pid) {
                    Ok(Some(current)) => (current - receipt.started_at).abs() > 3,
                    Ok(None) => true,
                    Err(_) => false,
                }
            });
            if !dead {
                return Err(StoreError::InvalidAuthority(
                    "Conversation already has a driver; connect to it".into(),
                ));
            }
        }
        let mut replace_provider = expected.is_none()
            || store.session_provider_unstarted(&session.id)?
            || conversation_engine_exited(&store, &session.id)?;
        let connection = store.session_connection(&session.id)?;
        if !replace_provider && connection.is_none() {
            replace_provider =
                recovery::prepare_after_restart(&store, &session.id, expected.as_ref())?;
            if !replace_provider {
                return Err(StoreError::InvalidAuthority(
                    "Conversation has no connection and no confirmed engine exit; retry requires exact process evidence or an observed restart of the same host".into(),
                ));
            }
        }
        let driver = store.claim_session_driver(
            &session.id,
            expected.as_ref(),
            &exec_id,
            replace_provider,
        )?;
        if replace_provider {
            store.record_session_provider_launch(&session.id, &driver, false)?;
        }
        capture.driver = Some((session.id, driver));
        drop(capture);
        let capture = Arc::downgrade(&self.0);
        crate::engine::agent::register_interrupt_cleanup(move || {
            if let Some(capture) = capture.upgrade() {
                let mut capture = capture.lock().expect("Session capture mutex poisoned");
                if capture.settled_outcome.is_none() {
                    if let Err(error) = capture.finish("interrupted") {
                        tracing::warn!(%error, "record interrupted Session exit");
                    }
                }
            }
        });
        Ok(())
    }

    pub(crate) fn conversation_resume_token(&self) -> StoreResult<Option<String>> {
        let capture = self.0.lock().expect("Session capture mutex poisoned");
        let store = row_store(&capture.dir)?;
        let Some(session) = store.session_for_artifact(&capture.manifest.artifact_key)? else {
            return Ok(None);
        };
        store.session_thread(&session.id)
    }

    pub(crate) fn begin_provider_spawn(&self) -> StoreResult<()> {
        let capture = self.0.lock().expect("Session capture mutex poisoned");
        if let Some((session, driver)) = &capture.driver {
            row_store(&capture.dir)?.record_session_provider_launch(session, driver, true)?;
        }
        Ok(())
    }

    pub(crate) fn record_provider_process(&self, pid: u32) -> StoreResult<()> {
        let capture = self.0.lock().expect("Session capture mutex poisoned");
        if let Some((session, driver)) = &capture.driver {
            if let Some(started) = crate::journal::process_started_at(pid).map_err(record_error)? {
                row_store(&capture.dir)?
                    .record_session_provider_process(session, driver, pid, started)?;
            }
        }
        Ok(())
    }

    pub(crate) fn flow_turn_selection(
        &self,
    ) -> StoreResult<Option<crate::durable::FlowTurnSelection>> {
        let capture = self.0.lock().expect("Session capture mutex poisoned");
        row_store(&capture.dir)?.flow_turn_selection(&capture.manifest.artifact_key)
    }

    pub(crate) fn session_driver(&self) -> Option<(String, crate::exec::SessionDriver)> {
        self.0
            .lock()
            .expect("Session capture mutex poisoned")
            .driver
            .clone()
    }

    pub(crate) fn environment(&self) -> BTreeMap<String, String> {
        let capture = self.0.lock().expect("Session capture mutex poisoned");
        let mut environment = BTreeMap::from([(
            CAPTURE_KEY_ENV.to_string(),
            capture.manifest.artifact_key.to_string(),
        )]);
        if let Ok(declaration) = std::env::var(crate::lf::WORK_DECLARATION_ENV) {
            environment.insert(crate::lf::WORK_DECLARATION_ENV.to_string(), declaration);
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
        self.with_capture(SessionCapture::start_attempt);
    }

    pub(crate) fn mark_handoff(&self, surface: &str) {
        self.with_capture(|capture| {
            capture.append_event(CaptureEvent::Handoff {
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
            capture.append_event(CaptureEvent::UserInput {
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
                capture.append_event(CaptureEvent::ProviderAccountSelected {
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
            capture.append_event(CaptureEvent::ProviderSessionObserved {
                attempt_key: capture.attempt_key(),
                provider_session_id: session_id.clone(),
            })?;
            capture.provider_session_id = Some(session_id);
            Ok(())
        });
    }

    pub(crate) fn final_answer(&self) -> StoreResult<Option<FinalAnswer>> {
        let capture = self.0.lock().expect("Session capture mutex poisoned");
        row_store(&capture.dir)?.input_final_answer(&capture.manifest.artifact_key)
    }

    pub(crate) fn finish(&self, outcome: &str) -> StoreResult<()> {
        self.0
            .lock()
            .expect("Session capture mutex poisoned")
            .finish(outcome)
            .map_err(record_error)
    }

    fn with_capture(&self, operation: impl FnOnce(&mut SessionCapture) -> std::io::Result<()>) {
        let mut capture = self.0.lock().expect("Session capture mutex poisoned");
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
            tracing::warn!("Session capture was poisoned before terminal settlement");
            return;
        };
        if capture.settled_outcome.is_some() {
            return;
        }
        if let Err(error) = capture.finish("failed") {
            tracing::warn!(
                %error,
                artifact_key = %capture.manifest.artifact_key,
                "failed to settle dropped Session capture"
            );
        }
    }
}

#[derive(Debug)]
struct SessionCapture {
    driver: Option<(String, crate::exec::SessionDriver)>,
    manifest: SessionCaptureManifest,
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
    recorder: SessionRecorder,
    telemetry_warned: bool,
    settled_outcome: Option<String>,
    activity: activity::Observer,
}

impl SessionCapture {
    /// Independent agent launches, including helpers, admit their conversation
    /// before provider work. Flow reservations have their own fenced publisher.
    fn record_row(
        manifest: &SessionCaptureManifest,
        dir: &Path,
        work: Option<crate::session::SessionWork>,
    ) -> StoreResult<Option<crate::session::AgentSession>> {
        let invocation_id = match &manifest.flow {
            Some(SessionFlowMembership::Step(step)) => Some(step.invocation_id.clone()),
            Some(SessionFlowMembership::Independent) | None => None,
        };
        // Mechanical commands have an Exec and, in a Flow, operation history.
        // Capturing their diagnostics does not create an agent conversation.
        if manifest.harness == "loopflow" {
            return Ok(None);
        }
        let store = row_store(dir)?;
        if invocation_id.is_some() {
            return Err(crate::store::StoreError::InvalidAuthority(
                "Flow agent input must be published through its reservation".into(),
            ));
        }
        let session = store.create_session(
            crate::session::AgentSession {
                captured: None,
                caller_artifact_key: manifest.caller_artifact_key.clone(),
                id: format!("session_{}", Uuid::new_v4().simple()),
                artifact_key: manifest.artifact_key.clone(),
                input_published: false,
                cwd: manifest.cwd.clone(),
                skill: manifest.skill.clone(),
                provider: Some(manifest.harness.clone()),
                model: manifest.model.clone(),
                node: None,
                iterations: None,
                task_id: work.as_ref().and_then(|work| work.task_id.clone()),
                wave_id: work.as_ref().and_then(|work| work.wave_id.clone()),
                work_source: work.as_ref().map(|work| work.source),
                flow_session_id: None,
                bound_at: None,
                kind: crate::session::SessionKind::Conversation,
                interactive: manifest.surface != "headless",
                repo: None,
                title: manifest.skill.clone().unwrap_or_else(|| {
                    crate::engine::naming::word_pair(manifest.artifact_key.as_str())
                }),
                title_source: crate::session::TitleSource::Generated,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: manifest.created_at.unix_timestamp(),
            },
            None,
            crate::journal::current_exec_id().as_ref(),
        )?;
        Ok(Some(session))
    }

    fn from_manifest(manifest: SessionCaptureManifest, dir: PathBuf) -> Self {
        let recorder = SessionRecorder::start(&dir, &manifest);
        Self {
            driver: None,
            provider: manifest.harness.clone(),
            model: manifest.model.clone(),
            account_id: manifest
                .exec
                .as_ref()
                .and_then(|exec| exec.account_id.clone()),
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
        self.attempt_started = true;
        self.append_event(CaptureEvent::ProviderAttemptStarted {
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
            self.append_event(CaptureEvent::ProviderAttemptFinished {
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
        self.append_event(CaptureEvent::ProviderOutput {
            stream: stream.to_string(),
            line: line.to_string(),
        })
    }

    fn record_stream_event(&mut self, event: &StreamEvent) -> std::io::Result<()> {
        match event {
            StreamEvent::Text(text) => self.append_event(CaptureEvent::Text { text: text.clone() }),
            StreamEvent::ToolUse { name, summary } => self.append_event(CaptureEvent::ToolUse {
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
                self.append_event(CaptureEvent::Result {
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
                self.append_event(CaptureEvent::Conversation {
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
            event => self.append_event(CaptureEvent::Conversation {
                event: Box::new(event),
            }),
        }
    }

    fn append_usage(&mut self, usage: TurnUsage, final_receipt: bool) -> std::io::Result<()> {
        if !usage.is_reported() {
            return Ok(());
        }
        self.usage_seq += 1;
        self.append_event(CaptureEvent::Usage {
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
        let terminal = TerminalReceipt {
            schema_version: SCHEMA_VERSION,
            outcome: outcome.to_string(),
            ended_at: OffsetDateTime::now_utc(),
            result_ref: None,
        };
        let terminal = write_terminal(&self.dir, terminal)?;
        if self.manifest.harness != "loopflow" {
            let store = row_store(&self.dir).map_err(std::io::Error::other)?;
            let input = &self.manifest.artifact_key;
            if let Some(session) = store
                .session_for_artifact(input)
                .map_err(std::io::Error::other)?
            {
                store
                    .retain_session_observation(
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
                    .map_err(std::io::Error::other)?;
            }
        }
        self.settled_outcome = Some(outcome.to_string());
        if self.attempt_started {
            if let Err(error) = self.append_event(CaptureEvent::ProviderAttemptFinished {
                attempt_key: self.attempt_key(),
                outcome: outcome.to_string(),
            }) {
                tracing::warn!(
                    %error,
                    artifact_key = %self.manifest.artifact_key,
                    "final recorder lifecycle event unavailable"
                );
            }
        }
        self.recorder.drain_after_settlement();
        if let Some((session, driver)) = self.driver.take() {
            match row_store(&self.dir)
                .and_then(|store| finish_session_driver(&store, &session, &driver, outcome))
            {
                Ok(_) | Err(StoreError::InvalidAuthority(_)) => {}
                Err(error) => return Err(std::io::Error::other(error)),
            }
        }
        Ok(())
    }

    fn append_event(&mut self, event: CaptureEvent) -> std::io::Result<()> {
        if !matches!(&event, CaptureEvent::Activity { .. }) {
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
}

/// The store that holds the conversation for the capture recorded at `dir`.
/// An absent socket alone says nothing about an engine. Require its recorded
/// process to have exited; a surviving endpoint wins over launcher death.
pub(crate) fn conversation_engine_exited(
    store: &crate::store::sqlite::SqliteStore,
    session: &str,
) -> StoreResult<bool> {
    let Some((pid, started)) = store.session_provider_process(session)? else {
        return Ok(false);
    };
    let exited = match crate::journal::process_started_at(pid) {
        Ok(Some(current)) => (current - started).abs() > 3,
        Ok(None) => true,
        Err(_) => false,
    };
    if !exited {
        return Ok(false);
    }
    Ok(match store.session_connection(session)? {
        Some((endpoint, _)) => match std::os::unix::net::UnixStream::connect(endpoint) {
            Ok(_) => false,
            Err(error) => matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ),
        },
        None => true,
    })
}

fn row_store(dir: &Path) -> StoreResult<crate::store::sqlite::SqliteStore> {
    crate::store::sqlite::SqliteStore::new(&row_database(dir)?)
}

fn row_database(dir: &Path) -> StoreResult<PathBuf> {
    let home = dir
        .ancestors()
        .nth(3)
        .ok_or_else(|| record_error(std::io::Error::other("Session capture has no Home")))?;
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
        if owner.id != caller.session_id {
            return Err(record_error(std::io::Error::other(
                "capture belongs to another Session",
            )));
        }
    }
    Ok(Some(key))
}

/// A capture is subordinate to its recorded Session in the selected Home.
pub(crate) fn capture_dir(key: &str) -> StoreResult<PathBuf> {
    resolve_capture(key).map(|(dir, _)| dir)
}

fn resolve_capture(key: &str) -> StoreResult<(PathBuf, crate::session::AgentSession)> {
    let home = crate::store::lf_home_dir();
    let dir = record_dir(&home, key)
        .ok_or_else(|| record_error(std::io::Error::other("invalid capture key")))?;
    let store = crate::store::sqlite::SqliteStore::open_execs_read_only(&database_in(&home)?)?;
    let owner = store.session_for_artifact(key)?.ok_or_else(|| {
        record_error(std::io::Error::other(
            "capture does not belong to a recorded Session in this Home",
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

// Session captures retain their published on-disk layout; the directory name
// does not make Run an owner. SQLite selects captures by artifact key.
pub(crate) fn record_dir(lf_home: &Path, artifact_key: &str) -> Option<PathBuf> {
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
    exec: Option<AgentExecRequest>,
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
        flow: Some(spec.flow),
        exec,
        context: context_ref,
        runtime_path,
        runtime_digest,
        host: gethostname::gethostname().to_string_lossy().into_owned(),
        boot_id: boot_id(),
    };
    Ok((manifest, context_bytes))
}

/// Resume artifact publication only. The caller must still claim the SQL
/// reservation before launching; readable artifacts confer no exec authority.
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
        read_provider_clients, read_provider_session, remove_provider_client,
        write_provider_client, AgentExecRequest, CaptureHandle, SessionCaptureManifest,
        SessionCaptureSpec, SubjectAttribution, TerminalReceipt,
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
                        "session_record::tests::terminal_attachment_probe",
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
            .args([
                "--exact",
                "session_record::tests::terminal_attachment_probe",
            ])
            .env("LF_TERMINAL_ID", "shell-one")
            .env("LF_TERMINAL_TTY", &tty)
            .env("LF_TEST_TERMINAL_ATTACHMENT", "")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(output.status.success());
    }

    fn spec(cwd: &std::path::Path) -> SessionCaptureSpec {
        SessionCaptureSpec {
            harness: "proof".to_string(),
            model: Some("model".to_string()),
            surface: "headless".to_string(),
            cwd: cwd.to_path_buf(),
            repo: Some(cwd.to_path_buf()),
            worktree: Some(cwd.to_path_buf()),
            skill: Some("implement".to_string()),
            subjects: Vec::new(),
            flow: crate::session_record::SessionFlowMembership::Independent,
            work: None,
        }
    }

    #[test]
    fn inherited_capture_requires_a_recorded_owner_and_matching_payload() {
        let _lock = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _home = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        std::env::set_var("LF_HOME", home.path());
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        let key = capture.artifact_key();
        std::env::set_var(super::CAPTURE_KEY_ENV, &key);
        assert_eq!(super::inherited_capture_key().unwrap(), Some(key.clone()));
        std::env::set_var(super::CAPTURE_KEY_ENV, super::new_artifact_key());
        assert!(super::inherited_capture_key().is_err());
        std::env::set_var(super::CAPTURE_KEY_ENV, "../another-home");
        assert!(super::inherited_capture_key().is_err());
        std::env::set_var(super::CAPTURE_KEY_ENV, &key);
        let path = capture.artifact_dir().join("manifest.json");
        let mut manifest = super::read_manifest(&capture.artifact_dir()).unwrap();
        manifest.artifact_key = super::new_artifact_key();
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(super::inherited_capture_key().is_err());
        fs::write(&path, b"invalid manifest").unwrap();
        assert!(super::inherited_capture_key().is_err());
        fs::remove_file(path).unwrap();
        assert_eq!(super::inherited_capture_key().unwrap(), Some(key));
    }

    #[test]
    fn prepared_run_projects_its_first_provider_attempt_separately_from_creation() {
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        let id = CaptureHandle::prepare_at(home.path(), spec(home.path()), None).unwrap();
        let (dir, mut manifest) = super::resolve_manifest(home.path(), id.as_str()).unwrap();
        manifest.created_at = time::OffsetDateTime::from_unix_timestamp(1).unwrap();
        fs::write(
            dir.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert!(!dir.join("events.jsonl").exists());
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
                    super::CaptureEvent::ProviderAttemptStarted { .. }
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

        let evidence = super::reduce_usage_reader(std::io::BufReader::new(
            fs::File::open(dir.join("events.jsonl")).unwrap(),
        ))
        .unwrap();
        assert_eq!(
            super::read_manifest(&dir)
                .unwrap()
                .created_at
                .unix_timestamp(),
            1
        );
        assert_eq!(evidence.first_provider_attempt_at, Some(10));
    }

    #[test]
    fn prepared_run_keeps_its_recorded_pr_instead_of_the_launching_pr() {
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        let original = crate::work::task::TaskPrId::new();
        let mut step = super::SessionFlowStep {
            task_id: Some(crate::work::task::TaskId::new()),
            task_pr_id: Some(original.clone()),
            invocation_id: "invocation".into(),
            flow: "feature".into(),
            step: "review".into(),
            node: Some("1".into()),
            iterations: Some(Vec::new()),
        };
        let mut prepared = spec(home.path());
        prepared.flow = super::SessionFlowMembership::Step(step.clone());
        let id = CaptureHandle::prepare_at(home.path(), prepared, None).unwrap();
        let (dir, _) = super::resolve_manifest(home.path(), id.as_str()).unwrap();
        step.task_pr_id = Some(crate::work::task::TaskPrId::new());
        let mut exec = spec(home.path());
        exec.flow = super::SessionFlowMembership::Step(step);
        let context = crate::trace::PreparedTurnContext::from_prompts("system", "review");
        let capture = CaptureHandle::start_prepared(home.path(), &id, exec, &context).unwrap();
        capture.finish("completed").unwrap();

        let manifest = super::read_manifest(&dir).unwrap();
        let Some(super::SessionFlowMembership::Step(step)) = manifest.flow else {
            panic!("prepared membership retained");
        };
        assert_eq!(step.task_pr_id, Some(original));
    }

    #[test]
    fn helper_capture_admits_a_headless_conversation_with_its_input_and_outcome() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let home = tempfile::tempdir().unwrap();
        let exec = AgentExecRequest::from_prepared(
            &AgentConfig {
                task_prompt: "repair the failed operation".into(),
                ..Default::default()
            },
            &AgentCapabilities::default(),
        );
        let capture =
            CaptureHandle::begin_at_with_request(home.path(), spec(home.path()), exec).unwrap();
        let store = super::row_store(&capture.artifact_dir()).unwrap();
        let session = store
            .session_for_artifact(&capture.artifact_key())
            .unwrap()
            .unwrap();
        let run = session.clone();
        assert!(!session.interactive);
        assert_eq!(session.kind, crate::session::SessionKind::Conversation);
        assert_eq!(session.title, "implement");
        assert_eq!(run.provider.as_deref(), Some("proof"));
        assert_eq!(
            super::read_manifest(&capture.artifact_dir())
                .unwrap()
                .exec
                .unwrap()
                .task_prompt,
            "repair the failed operation"
        );
        capture.finish("failed").unwrap();
        let after = store
            .session_for_artifact(&capture.artifact_key())
            .unwrap()
            .unwrap();

        assert_eq!(after.id, session.id);
        assert_eq!(
            super::row_store(&capture.artifact_dir())
                .unwrap()
                .input_history(capture.artifact_key().as_str())
                .unwrap()
                .recorded_outcome
                .as_deref(),
            Some("failed")
        );
        assert!(after.completed_at.is_none());
    }

    #[test]
    fn reserved_publication_recovers_artifacts_without_repeating_exec_authority() {
        for boundary in [
            "before_artifacts",
            "context_staged",
            "manifest_staged",
            "artifacts_published",
        ] {
            let home = tempfile::tempdir().unwrap();
            let id = crate::session_record::new_artifact_key();
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
            // no capture Drop receipt falsely settling the prepared capture.
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
            assert_eq!(capture.artifact_key(), id);
            assert_eq!(std::fs::read(dir.join("manifest.json")).unwrap(), original);
            assert_eq!(
                std::fs::read(dir.join("context.json")).unwrap(),
                bytes.unwrap()
            );
            // SQL has granted authority once, but no provider has started.
            // An absent provider receipt does not grant a second exec.
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
        let id = crate::session_record::new_artifact_key();
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
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        let caller = crate::session_record::new_artifact_key();
        let id = CaptureHandle::prepare_at(home.path(), spec(home.path()), Some(caller.clone()))
            .unwrap();
        let (dir, prepared) = super::resolve_manifest(home.path(), id.as_str()).unwrap();
        assert_eq!(prepared.artifact_key, id);
        assert_eq!(prepared.caller_artifact_key.as_ref(), Some(&caller));
        // Recover a crash after the manifest was published but before the prepared marker.
        fs::remove_file(dir.join("prepared")).unwrap();
        assert_eq!(
            CaptureHandle::prepare_at_with_key(
                home.path(),
                spec(home.path()),
                Some(caller.clone()),
                id.clone()
            )
            .unwrap(),
            id
        );
        assert_eq!(
            super::read_manifest(&dir).unwrap().created_at,
            prepared.created_at
        );
        assert_eq!(
            CaptureHandle::prepare_at_with_key(
                home.path(),
                spec(home.path()),
                Some(caller.clone()),
                id.clone()
            )
            .unwrap(),
            id
        );
        assert!(!dir.join("terminal.json").exists());
        assert!(!dir.join("provider-clients").exists());
        let context = crate::trace::PreparedTurnContext::from_prompts("system", "human prompt");
        let capture =
            CaptureHandle::start_prepared(home.path(), &id, spec(home.path()), &context).unwrap();
        let launched = super::read_manifest(&dir).unwrap();
        assert_eq!(capture.artifact_key(), id);
        assert!(CaptureHandle::prepare_at_with_key(
            home.path(),
            spec(home.path()),
            Some(caller.clone()),
            id.clone()
        )
        .is_err());
        assert!(
            !dir.join("prepared").exists(),
            "recovery cannot rearm an already claimed launch"
        );
        assert_eq!(launched.created_at, prepared.created_at);
        assert_eq!(launched.caller_artifact_key, Some(caller));
        let context_ref = launched.context.as_ref().unwrap();
        let context_bytes = fs::read(dir.join(&context_ref.path)).unwrap();
        assert_eq!(context_ref.bytes, context_bytes.len() as u64);
        assert_eq!(
            context_ref.content_sha256,
            hex::encode(<sha2::Sha256 as sha2::Digest>::digest(&context_bytes))
        );
        assert!(
            CaptureHandle::start_prepared(home.path(), &id, spec(home.path()), &context).is_err()
        );
        capture.mark_spawn_requested();
        capture.finish("completed").unwrap();
        assert_eq!(
            serde_json::from_slice::<super::TerminalReceipt>(
                &fs::read(dir.join("terminal.json")).unwrap()
            )
            .unwrap()
            .outcome,
            "completed"
        );
        assert!(super::record_dir(home.path(), &id).unwrap().is_dir());
    }

    #[test]
    fn manifest_round_trips_the_exact_prepared_exec_without_ambient_authority() {
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
            AgentExecRequest::from_prepared(&config, &AgentCapabilities { chrome: true });
        let capture =
            CaptureHandle::begin_at_with_request(home.path(), spec(home.path()), expected.clone())
                .unwrap();

        let bytes = fs::read(capture.artifact_dir().join("manifest.json")).unwrap();
        let saved: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(saved["artifact_key"], capture.artifact_key());
        assert!(saved.get("caller_artifact_key").is_some());
        assert!(saved.get("exec").is_some());
        assert!(saved.get("run_id").is_none());
        assert!(saved.get("launch").is_none());
        let manifest: SessionCaptureManifest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(manifest.exec, Some(expected));
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
        // Interrupted/tampered context reconciliation is covered by
        // reserved_publication_retains_conflicting_inputs.
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
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path()))
            .expect("publish Session capture manifest");
        capture.record_input("initial", "do the work");
        let dir = capture.artifact_dir();
        let artifact_key = capture.artifact_key().to_string();

        assert!(dir.join("manifest.json").is_file());
        assert_eq!(
            dir.parent().and_then(|path| path.file_name()),
            Some(std::ffi::OsStr::new(
                &artifact_key.strip_prefix("run_").unwrap_or(&artifact_key)[..2]
            ))
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
        capture.finish("completed").expect("settle capture");

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
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
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

        fs::remove_dir_all(capture.artifact_dir()).unwrap();
        assert_eq!(
            capture.final_answer().unwrap(),
            Some(super::FinalAnswer {
                text: "final report".to_string(),
                exact: true,
            })
        );
    }

    #[test]
    fn history_keeps_what_streamed_increments_add_up_to() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        let turn = || "turn-1".to_string();
        capture.record_conversation(ConversationEvent::TurnStarted { turn_id: turn() });
        for word in ["streamed ", "one ", "token ", "at a time"] {
            capture.record_raw(
                "notification",
                &serde_json::json!({"method": "item/agentMessage/delta", "params": {"delta": word}})
                    .to_string(),
            );
            capture.record_conversation(ConversationEvent::TextDelta {
                turn_id: turn(),
                content: word.to_string(),
            });
        }
        for diff in ["first", "first\nsecond"] {
            capture.record_conversation(ConversationEvent::DiffUpdated {
                turn_id: turn(),
                diff: diff.to_string(),
            });
        }
        capture.record_raw(
            "notification",
            &serde_json::json!({"method": "item/completed", "params": {}}).to_string(),
        );
        capture.record_conversation(ConversationEvent::TurnCompleted {
            turn_id: turn(),
            status: crate::chat::types::Lifecycle::Completed,
        });
        capture.finish("completed").unwrap();

        let file = fs::read_to_string(capture.artifact_dir().join("events.jsonl")).unwrap();
        assert_eq!(file.matches("item/agentMessage/delta").count(), 4);
        assert_eq!(file.matches("\"type\":\"text_delta\"").count(), 4);
        assert_eq!(file.matches("\"type\":\"diff_updated\"").count(), 2);

        let history = super::row_store(&capture.artifact_dir())
            .unwrap()
            .input_events(&capture.artifact_key())
            .unwrap();
        let kept: Vec<_> = history
            .iter()
            .filter(|event| event["type"] == "conversation" || event["type"] == "provider_output")
            .map(|event| {
                let detail = &event["event"];
                match event["type"].as_str().unwrap() {
                    "provider_output" => event["line"].as_str().unwrap().to_string(),
                    _ => format!(
                        "{}:{}",
                        detail["type"].as_str().unwrap(),
                        detail["content"]
                            .as_str()
                            .or(detail["diff"].as_str())
                            .unwrap_or_default()
                    ),
                }
            })
            .collect();
        assert_eq!(
            kept,
            [
                "turn_started:",
                "text_delta:streamed one token at a time",
                "diff_updated:first\nsecond",
                r#"{"method":"item/completed","params":{}}"#,
                "turn_completed:",
            ]
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

        fs::remove_dir_all(capture.artifact_dir()).unwrap();
        assert_eq!(
            capture.final_answer().unwrap(),
            Some(super::FinalAnswer {
                text: "working\nfinal report".to_string(),
                exact: false,
            })
        );
    }

    #[test]
    fn pre_spawn_failure_can_reclaim_conversation_without_inventing_engine_exit() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let capture = CaptureHandle::begin_at_with_request(
            ledger.home(),
            spec(ledger.home()),
            AgentExecRequest::from_prepared(&AgentConfig::default(), &AgentCapabilities::default()),
        )
        .unwrap();
        let store = super::row_store(&capture.artifact_dir()).unwrap();
        let session = store
            .session_for_artifact(&capture.artifact_key())
            .unwrap()
            .unwrap();
        let command = vec!["lf".into(), "skill".into()];
        crate::journal::with_runtime(ledger.home(), &command, || {
            capture.claim_conversation_driver()?;
            let (_, driver) = capture.session_driver().unwrap();
            store.record_session_connection(
                &session.id,
                &driver,
                "/missing.sock",
                "saved-thread",
            )?;
            // The previous engine exited; the next admission replaces it.
            store.record_session_provider_process(&session.id, &driver, std::process::id(), 1)?;
            capture.finish("failed")?;
            Ok(())
        })
        .unwrap();
        for _ in 0..2 {
            let manifest = super::read_manifest(&capture.artifact_dir()).unwrap();
            let retry = CaptureHandle(std::sync::Arc::new(std::sync::Mutex::new(
                super::SessionCapture::from_manifest(manifest, capture.artifact_dir()),
            )));
            crate::journal::with_runtime(ledger.home(), &command, || {
                assert_eq!(
                    retry.conversation_resume_token()?.as_deref(),
                    Some("saved-thread")
                );
                retry.claim_conversation_driver()?;
                assert!(store.session_provider_unstarted(&session.id)?);
                assert!(!super::conversation_engine_exited(&store, &session.id)?);
                retry.finish("failed")?;
                Ok(())
            })
            .unwrap();
        }
        let expected = store.session_driver(&session.id).unwrap().unwrap();
        crate::journal::with_runtime(ledger.home(), &command, || {
            let exec = crate::journal::current_exec_id().unwrap();
            let driver = store.claim_session_driver(&session.id, Some(&expected), &exec, true)?;
            store.record_session_provider_launch(&session.id, &driver, false)?;
            store.record_session_provider_launch(&session.id, &driver, true)?;
            assert!(!store.session_provider_unstarted(&session.id)?);
            assert!(!super::conversation_engine_exited(&store, &session.id)?);
            assert!(store
                .record_session_provider_launch(&session.id, &expected, false)
                .is_err());
            store.record_session_provider_process(
                &session.id,
                &driver,
                std::process::id(),
                crate::journal::process_started_at(std::process::id())?.unwrap(),
            )?;
            assert!(!super::conversation_engine_exited(&store, &session.id)?);
            store.release_session_driver(&session.id, &driver)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            store.session_thread(&session.id).unwrap().as_deref(),
            Some("saved-thread")
        );
        assert!(store
            .session_history(&session.id, 0, 100)
            .unwrap()
            .iter()
            .all(|event| event.kind != crate::session::SessionEventKind::Completed));
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

        fs::remove_dir_all(capture.artifact_dir()).unwrap();
        let session = read_provider_session(&capture.artifact_dir())
            .unwrap()
            .expect("provider session reference");
        assert_eq!(session.provider_session_id, "provider-session");
        assert_eq!(session.account_id, Some(account_id));
    }

    #[test]
    fn provider_account_observation_precedes_session_and_preserves_attempts() {
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
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
        assert!(!dir.join("provider-session.json").exists());
        rusqlite::Connection::open(super::row_database(&dir).unwrap()).unwrap().execute(
            "DELETE FROM session_events WHERE kind='observed' AND json_extract(payload,'$.input_id')=?1 AND json_extract(payload,'$.source') LIKE 'provider-session:%'",
            [capture.artifact_key().as_str()],
        ).unwrap();
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
        assert!(!dir.join("provider-session.json").exists());
        rusqlite::Connection::open(super::row_database(&dir).unwrap()).unwrap().execute(
            "DELETE FROM session_events WHERE kind='observed' AND json_extract(payload,'$.input_id')=?1 AND json_extract(payload,'$.source') LIKE 'provider-session:%'",
            [capture.artifact_key().as_str()],
        ).unwrap();
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
        let first_id = first.artifact_key();
        let second_id = second.artifact_key();

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
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path()))
            .expect("publish Session capture manifest");
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
    fn terminal_history_survives_an_unavailable_telemetry_recorder() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        capture.0.lock().unwrap().recorder.sender = None;

        capture.finish("completed").unwrap();
        capture.finish("completed").unwrap();
        let store = super::row_store(&capture.artifact_dir()).unwrap();
        fs::remove_dir_all(capture.artifact_dir()).unwrap();

        let history = store
            .input_history(capture.artifact_key().as_str())
            .unwrap();
        assert_eq!(history.recorded_outcome.as_deref(), Some("completed"));
    }

    #[test]
    fn dropping_the_last_capture_settles_unexpected_control_flow_as_failed() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path()))
            .expect("publish Session capture manifest");
        let dir = capture.artifact_dir();

        drop(capture);

        let terminal: TerminalReceipt =
            serde_json::from_slice(&fs::read(dir.join("terminal.json")).unwrap()).unwrap();
        assert_eq!(terminal.outcome, "failed");
    }

    #[test]
    fn retry_usage_keeps_provider_cumulative_values_in_distinct_streams() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path()))
            .expect("publish Session capture manifest");
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
        capture.finish("completed").expect("settle capture");

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
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path()))
            .expect("publish Session capture manifest");
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

        let run = super::row_store(&capture.artifact_dir())
            .unwrap()
            .input_history(capture.artifact_key().as_str())
            .unwrap();
        assert_eq!(run.recorded_outcome.as_deref(), Some("completed"));
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

        let error = super::reduce_usage_reader(std::io::BufReader::new(
            fs::File::open(dir.join("events.jsonl")).unwrap(),
        ))
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("malformed complete capture event"));
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

        let snapshot =
            super::reduce_usage_reader(std::io::BufReader::new(fs::File::open(&path).unwrap()))
                .unwrap();
        assert_eq!(snapshot.gaps, 1);
        assert!(snapshot.first_provider_attempt_at.is_some());
    }

    #[test]
    fn reader_does_not_invent_an_attempt_from_incomplete_or_unsupported_evidence() {
        let mut attempt = super::EventEnvelope {
            schema_version: 1,
            seq: 0,
            observed_at: time::OffsetDateTime::from_unix_timestamp(10).unwrap(),
            event: super::CaptureEvent::ProviderAttemptStarted {
                provider: "codex".into(),
                model: None,
                account_id: None,
                attempt_key: "first".into(),
            },
        };
        // An otherwise valid attempt without a final newline is not committed evidence.
        let incomplete = serde_json::to_vec(&attempt).unwrap();
        let snapshot = super::reduce_usage_reader(incomplete.as_slice()).unwrap();
        assert_eq!(snapshot.first_provider_attempt_at, None);
        assert_eq!(snapshot.gaps, 1);
        attempt.schema_version = 999;
        let unsupported = format!("{}\n", serde_json::to_string(&attempt).unwrap());
        let snapshot = super::reduce_usage_reader(unsupported.as_bytes()).unwrap();
        assert_eq!(snapshot.first_provider_attempt_at, None);
        assert_eq!(snapshot.gaps, 1);
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

        let error = super::read_manifest(&dir).unwrap_err();
        assert!(error
            .to_string()
            .contains("unsupported Session capture manifest schema 999"));
    }
}
