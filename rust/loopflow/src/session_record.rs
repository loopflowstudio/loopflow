//! Captured Session inputs and append-only provider evidence.

pub(crate) mod activity;
mod runtime;

use crate::id::AgentSessionId;
pub(crate) use runtime::finish_session_attachment;

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::chat::types::{ConversationEvent, TurnUsage};
use crate::naming::generated_session_title;
use crate::store::{StoreError, StoreResult};

/// An opaque artifact directory key; it grants no conversation or Graph authority.
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
    pub graph: SessionGraphMembership,
    pub work: Option<crate::session::SessionWork>,
}

/// A Graph position older manifests captured. New captures record none: a step
/// is an ordinary command, and its Graph is read from the Graph process record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionGraphStep {
    pub task_id: Option<crate::work::task::TaskId>,
    pub task_pr_id: Option<crate::work::task::TaskPrId>,
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
    pub write_scope: crate::agent::AgentWriteScope,
    pub execution_boundary: Option<crate::agent::AgentExecutionBoundary>,
    pub skip_permissions: bool,
    pub chrome: bool,
}

impl AgentProcessRequest {
    pub(crate) fn codex_initial_input(&self) -> String {
        match &self.skill_invocation {
            Some(skill) => format!("{}\n\n{}", skill.codex_prompt(), self.task_prompt),
            None => self.task_prompt.clone(),
        }
    }

    pub(crate) fn from_prepared(config: &crate::agent::AgentConfig) -> Self {
        Self {
            system_prompt: crate::agent::system_prompt_with_structured_replies(config),
            conversation_context: config.conversation_context.clone(),
            task_prompt: config.task_prompt.clone(),
            skill_invocation: config.skill_invocation.clone(),
            output_schema: config.output_schema.clone(),
            agent: config.agent().to_string(),
            account_id: config.provider_account_id.clone(),
            max_turns: config.max_turns,
            write_scope: config.write_scope,
            execution_boundary: config.execution_boundary.clone(),
            skip_permissions: config.skip_permissions,
            chrome: config.chrome,
        }
    }

    pub(crate) fn replay_unavailable_reason(&self) -> Option<&'static str> {
        let (harness, _) = crate::config::parse_agent(&self.agent);
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalReceipt {
    pub schema_version: u32,
    pub outcome: String,
    #[serde(with = "time::serde::rfc3339")]
    pub ended_at: OffsetDateTime,
    pub result_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ProviderSessionRef {
    schema_version: u32,
    // Capture files retain their original encoding.
    #[serde(rename = "provider_session_id")]
    pub(crate) agent_session: AgentSessionId,
    pub(crate) account_id: Option<crate::store::ProviderAccountId>,
}

impl ProviderSessionRef {
    pub(crate) fn validate(&self) -> std::io::Result<()> {
        if self.schema_version != SCHEMA_VERSION || self.agent_session.as_str().is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid provider session reference",
            ));
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct SessionContextArtifact<'a> {
    schema_version: u32,
    context: &'a crate::trace::PreparedTurnContext,
}

/// What SQLite history keeps of a capture: its provider attempts, accounts and
/// sessions, how each turn ended, and each silence longer than
/// [`SILENCE_SECONDS`]. Provider text and usage have no variant: the provider's
/// transcript holds the conversation and native rows hold usage. Facts keep the
/// `events.jsonl:{seq}` source their receipt keys and migrations already index.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum CaptureFact {
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
        #[serde(rename = "provider_session_id")]
        agent_session: AgentSessionId,
    },
    Conversation {
        event: TurnEnd,
    },
    /// Stored under its own source; see [`CaptureFact::observation`].
    #[serde(skip)]
    Silence {
        since: OffsetDateTime,
    },
}

/// How a provider turn ended, stored as the conversation event it was.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum TurnEnd {
    TurnCompleted {
        turn_id: String,
        status: crate::chat::types::Lifecycle,
    },
    Error {
        code: String,
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        evidence: Option<Box<crate::chat::types::FailureEvidence>>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct FactEnvelope {
    schema_version: u32,
    seq: u64,
    #[serde(with = "time::serde::rfc3339")]
    observed_at: OffsetDateTime,
    #[serde(flatten)]
    fact: CaptureFact,
}

impl CaptureFact {
    /// The history observation of this fact at stream position `seq`, as
    /// `(source, evidence)`.
    pub(crate) fn observation(&self, seq: u64, at: OffsetDateTime) -> (String, serde_json::Value) {
        match self {
            Self::Silence { since } => (
                format!("silence:{seq}"),
                serde_json::json!({"since": since.unix_timestamp(), "observed_at": at.unix_timestamp()}),
            ),
            fact => (
                format!("events.jsonl:{seq}"),
                serde_json::json!(FactEnvelope {
                    schema_version: SCHEMA_VERSION,
                    seq,
                    observed_at: at,
                    fact: fact.clone(),
                }),
            ),
        }
    }
}

/// A history row's fact, or `None` for a stream event an earlier lf stored.
fn read_fact(evidence: &serde_json::Value) -> Option<FactEnvelope> {
    serde_json::from_value::<FactEnvelope>(evidence.clone())
        .ok()
        .filter(|envelope| envelope.schema_version == SCHEMA_VERSION)
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
/// resumable object. Provider records retain separate outcomes and driving Processes;
/// recorder completion is historical evidence, never native success or process exit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionHistory {
    pub session_id: crate::id::LfSessionId,
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
/// identity. References identify evidence; they confer no process/settlement API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderHistory {
    pub reference: ProviderHistoryReference,
    pub lf_process_id: Option<crate::id::LfProcessId>,
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
        thread: AgentSessionId,
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

/// A gap between consecutive provider events long enough to count as a silence.
pub(crate) const SILENCE_SECONDS: i64 = 600;

/// The key of a native usage receipt within its turn. OpenCode reports one
/// counter per assistant message; every other provider one per turn.
pub(crate) fn native_usage_key(payload: &serde_json::Value) -> String {
    match payload["provider"].as_str() {
        Some("opencode") => payload["message"]["id"].as_str().unwrap_or_default().into(),
        _ => String::new(),
    }
}

/// The key of native usage receipts that arrived after the thread's next turn
/// began at `started`.
pub(crate) fn late_usage_key(key: &str, started: i64) -> String {
    format!("{key}:after:{started}")
}

/// Merge a later native usage receipt into the retained one. Codex reports
/// thread-cumulative `total` and the latest request as `last`: the turn keeps
/// its first receipt's baseline (`total - last`), the largest total of each
/// counter, whether any counter ever went backwards, and its largest request.
/// Other providers report the turn's or message's own cumulative counter.
pub(crate) fn merge_native_usage(
    saved: &serde_json::Value,
    later: &serde_json::Value,
) -> serde_json::Value {
    let mut merged = later.clone();
    if later.get("total").is_some() {
        merged["baseline"] = native_baseline(saved);
        merged["peakInputTokens"] = native_peak(saved).max(native_peak(later)).into();
        let mut regressed = saved["regressed"] == true;
        if let Some(totals) = merged["total"].as_object_mut() {
            for (key, total) in totals {
                if let Some(saved) = saved["total"][key.as_str()].as_u64() {
                    regressed |= total.as_u64().is_some_and(|later| later < saved);
                    *total = total
                        .as_u64()
                        .map_or(saved, |later| later.max(saved))
                        .into();
                }
            }
        }
        if regressed {
            merged["regressed"] = true.into();
        }
    }
    if saved["final"] == true {
        merged["final"] = true.into();
    }
    merged
}

/// Codex counters before the turn's first request.
fn native_baseline(receipt: &serde_json::Value) -> serde_json::Value {
    if receipt["baseline"].is_object() {
        return receipt["baseline"].clone();
    }
    let total = receipt["total"].as_object().into_iter().flatten();
    total
        .filter_map(|(key, value)| {
            let before = value
                .as_u64()?
                .checked_sub(receipt["last"][key].as_u64()?)?;
            Some((key.clone(), serde_json::json!(before)))
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}

/// The largest single Codex request in the turn.
fn native_peak(receipt: &serde_json::Value) -> Option<u64> {
    receipt["peakInputTokens"]
        .as_u64()
        .or(receipt["last"]["inputTokens"].as_u64())
}

/// One reading of a native usage counter: a provider turn's cumulative usage so
/// far, or an OpenCode message's.
#[derive(Debug)]
struct UsageReading {
    thread: AgentSessionId,
    turn: String,
    /// The counter this reading belongs to; later readings of it supersede it.
    counter: String,
    seq: u64,
    start_known: bool,
    final_receipt: bool,
    usage: TurnUsage,
}

/// Every native usage row in `history`, as readings of its counters, and the
/// rows that could not be read. A row is read by its shape: a `usage` object is
/// a turn's [`TurnUsage`]; `total`/`last` is Codex's thread-cumulative counter;
/// `message.tokens` is an OpenCode message.
fn native_usage(history: &[crate::session::SessionEvent]) -> (Vec<UsageReading>, usize) {
    use crate::session::SessionEventKind;

    let mut turns = BTreeMap::<(&AgentSessionId, &str), Vec<&crate::session::SessionEvent>>::new();
    for event in history {
        if event.kind != SessionEventKind::Observed {
            if let (Some(thread), Some(turn)) =
                (event.agent_session.as_ref(), event.provider_turn.as_deref())
            {
                turns.entry((thread, turn)).or_default().push(event);
            }
        }
    }
    let mut readings = Vec::new();
    let mut gaps = 0;
    for ((thread, turn), native) in turns {
        let receipts: Vec<_> = native
            .iter()
            .filter(|event| event.kind == SessionEventKind::Usage)
            .collect();
        let Some(first) = receipts.first() else {
            continue;
        };
        let start_known = native.iter().any(|e| e.kind == SessionEventKind::Started);
        let reading =
            |counter: String, seq: i64, final_receipt: bool, usage: TurnUsage| UsageReading {
                thread: thread.clone(),
                turn: turn.into(),
                counter,
                seq: seq as u64,
                start_known,
                final_receipt,
                usage,
            };
        if first.payload["usage"].is_object() {
            for receipt in receipts {
                match serde_json::from_value::<TurnUsage>(receipt.payload["usage"].clone()) {
                    Ok(usage) => readings.push(reading(
                        format!("native:{thread}:{turn}"),
                        receipt.seq,
                        receipt.payload["final"] == true,
                        usage,
                    )),
                    Err(_) => gaps += 1,
                }
            }
            continue;
        }
        if first.payload["message"]["tokens"].is_object() {
            // Each completed assistant message owns a native usage counter;
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
                    model: info["model"]["id"]
                        .as_str()
                        .or_else(|| info["modelID"].as_str())
                        .map(str::to_owned),
                    cost_usd: info["cost"].as_f64(),
                };
                readings.push(reading(
                    format!("native:{thread}:{turn}:{message}"),
                    receipt.seq,
                    true,
                    usage,
                ));
            }
            continue;
        }
        if first.payload.get("total").is_none() {
            gaps += receipts.len();
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
                            && event.agent_session.as_ref() == Some(thread)
                            && event.provider_turn.as_deref() != Some(turn)
                            && event.seq < start.seq
                    })
                    .max_by_key(|event| event.seq)
            })
            .filter(|previous| {
                history.iter().any(|event| {
                    event.kind == SessionEventKind::Completed
                        && event.agent_session == previous.agent_session
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
                            && event.agent_session == previous.agent_session
                            && event.provider_turn == previous.provider_turn
                            && event.seq <= previous.seq
                    })
                    .filter_map(|event| field(&event.payload, "total", key))
                    .max()
            })
        };
        let baseline =
            |key: &str| prior_total(key).or_else(|| native_baseline(&first.payload)[key].as_u64());
        let known = start.is_some()
            && [
                "inputTokens",
                "outputTokens",
                "cachedInputTokens",
                "reasoningOutputTokens",
            ]
            .iter()
            .all(|key| prior_total(key).is_some() || baseline(key) == Some(0));
        for last in receipts {
            if last.payload["regressed"] == true {
                gaps += 1;
            }
            let delta = |key: &str| field(&last.payload, "total", key)?.checked_sub(baseline(key)?);
            let gross = delta("inputTokens");
            let cached = delta("cachedInputTokens");
            let usage = TurnUsage {
                input_tokens: gross
                    .zip(cached)
                    .and_then(|(gross, cached)| gross.checked_sub(cached)),
                output_tokens: delta("outputTokens"),
                total_input_tokens: gross,
                peak_input_tokens: native_peak(&last.payload),
                context_window_tokens: last.payload["modelContextWindow"].as_u64(),
                reasoning_tokens: delta("reasoningOutputTokens"),
                cache_read_tokens: cached,
                cache_write_tokens: delta("cacheWriteInputTokens"),
                model: None,
                cost_usd: None,
            };
            // Completion does not include a final usage receipt in the native schema.
            let mut reading = reading(format!("native:{thread}:{turn}"), last.seq, false, usage);
            reading.start_known = known;
            readings.push(reading);
        }
    }
    (readings, gaps)
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
    facts: &[FactEnvelope],
    readings: &[UsageReading],
    history: &[crate::session::SessionEvent],
) -> std::io::Result<Vec<ProviderHistory>> {
    use crate::session::SessionEventKind;
    let mut native = BTreeMap::<(&AgentSessionId, &str), Vec<&crate::session::SessionEvent>>::new();
    for event in history
        .iter()
        .filter(|event| event.kind != SessionEventKind::Observed)
    {
        if let (Some(thread), Some(turn)) =
            (event.agent_session.as_ref(), event.provider_turn.as_deref())
        {
            native.entry((thread, turn)).or_default().push(event);
        }
    }
    let mut records = Vec::new();
    for ((thread, turn), events) in &native {
        let start = events
            .iter()
            .find(|event| event.kind == SessionEventKind::Started);
        let completed = events
            .iter()
            .find(|event| event.kind == SessionEventKind::Completed);
        let usage = reduce_usage(
            readings
                .iter()
                .filter(|reading| &reading.thread == *thread && reading.turn == *turn),
        );
        records.push(ProviderHistory {
            reference: ProviderHistoryReference::NativeTurn {
                thread: (*thread).clone(),
                turn: (*turn).into(),
                start_seq: start.map(|event| event.seq),
                completion_seq: completed.map(|event| event.seq),
            },
            lf_process_id: start
                .and_then(|event| event.lf_process_id.as_deref())
                .map(crate::id::LfProcessId::parse)
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
    // Provider attempts have no reliable Process or native-turn identity. Keep
    // their own key and result, never substitute the enclosing recorder exit.
    // Their usage is on the native turns they drove.
    let mut attempts = BTreeMap::<&str, Vec<&FactEnvelope>>::new();
    for event in facts {
        let key = match &event.fact {
            CaptureFact::ProviderAttemptStarted { attempt_key, .. }
            | CaptureFact::ProviderAttemptFinished { attempt_key, .. }
                if !attempt_key.is_empty() =>
            {
                attempt_key
            }
            _ => continue,
        };
        attempts.entry(key).or_default().push(event);
    }
    for (attempt, events) in attempts {
        let start = events
            .iter()
            .find(|event| matches!(event.fact, CaptureFact::ProviderAttemptStarted { .. }));
        let finished = events.iter().find_map(|event| match &event.fact {
            CaptureFact::ProviderAttemptFinished { outcome, .. } => {
                Some((event.observed_at.unix_timestamp(), outcome.clone()))
            }
            _ => None,
        });
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
            lf_process_id: None,
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
            usage: reduce_usage(std::iter::empty()),
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
    let mut facts = Vec::new();
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
        } else if source.starts_with("events.jsonl:") {
            facts.extend(read_fact(evidence));
        }
    }
    facts.sort_by_key(|fact| fact.seq);
    let (readings, unread) = native_usage(history);
    let providers =
        project_provider_history(session.captured, artifact_key, &facts, &readings, history)?;
    let usage = reduce_usage(&readings);
    gaps += unread + usage.gaps + usize::from(manifest.is_none());
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
        task_pr_id: manifest
            .as_ref()
            .and_then(|manifest| match &manifest.graph {
                Some(SessionGraphMembership::Step(step)) => step.task_pr_id.clone(),
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
    let store = crate::store::sqlite::SqliteStore::open_processes_read_only(&database)
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
        .filter(|session| session.id.as_str() == crate::id::canonical_session(&artifact))
        .and_then(|session| session.artifact_key)
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
    crate::store::sqlite::SqliteStore::open_processes_read_only(
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
    for envelope in events.iter().filter_map(read_fact) {
        match envelope.fact {
            CaptureFact::ProviderAttemptStarted {
                attempt_key,
                account_id,
                ..
            }
            | CaptureFact::ProviderAccountSelected {
                attempt_key,
                account_id,
            } => {
                accounts.insert(attempt_key, account_id);
            }
            CaptureFact::ProviderSessionObserved {
                attempt_key,
                agent_session,
            } => {
                provider_session = Some(ProviderSessionRef {
                    schema_version: SCHEMA_VERSION,
                    agent_session,
                    account_id: accounts.get(&attempt_key).cloned().flatten(),
                });
            }
            _ => {}
        }
    }
    Ok(provider_session)
}

pub(crate) fn write_provider_session(
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

/// The resume reference: the newest one names the native conversation and
/// account `lf resume` reopens.
pub(crate) fn provider_session_observation(
    session: &crate::session::LfSession,
    input: &str,
    agent_session: &AgentSessionId,
    account_id: Option<crate::store::ProviderAccountId>,
) -> crate::session::SessionObservation {
    let source = format!("provider-session:{}", Uuid::new_v4());
    crate::session::SessionObservation {
        artifact_key: input.to_owned(),
        source: source.clone(),
        observed_at: OffsetDateTime::now_utc().unix_timestamp(),
        task_id: session.task_id.clone(),
        wave_id: session.wave_id.clone(),
        payload: serde_json::json!({"input_id":input,"source":source,"evidence":ProviderSessionRef {
            schema_version: SCHEMA_VERSION,
            agent_session: agent_session.clone(), account_id,
        }}),
    }
}

/// A terminal marker is valid only on the PTY where the shell installed it.
/// Inherited environment after app, SSH, or background handoff is not attachment.
pub(crate) fn current_terminal_id() -> Option<String> {
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

/// Who chose a Session's current title. A human name is never replaced by a
/// generated suggestion. `Unavailable` is never stored: the canonical name
/// lives on another Machine and the reader only has a local display label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionTitleSource {
    Generated,
    Human,
    Unavailable,
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

/// Reduce usage readings once per counter: each counter's largest values,
/// summed across counters.
fn reduce_usage<'a>(readings: impl IntoIterator<Item = &'a UsageReading>) -> SessionUsage {
    let mut streams = BTreeMap::<&str, UsageStream>::new();
    let mut gaps = 0;
    for reading in readings {
        if !reading.start_known {
            gaps += 1;
        }
        let stream = streams.entry(reading.counter.as_str()).or_default();
        if stream
            .observation_seq
            .is_some_and(|previous| reading.seq <= previous)
        {
            gaps += 1;
        }
        stream.observation_seq = Some(
            stream
                .observation_seq
                .map_or(reading.seq, |seen| seen.max(reading.seq)),
        );
        stream.final_receipt |= reading.final_receipt;
        let usage = &reading.usage;
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
    usage
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
        Ok(Self(Arc::new(Mutex::new(SessionCapture::from_manifest(
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
                crate::harness::agent_process::close_session_agent_process(&store, session)
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
                    .expect("Session capture mutex poisoned")
                    .attachment = Some((session.to_string(), attachment));
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
        Ok(Self(Arc::new(Mutex::new(SessionCapture::from_manifest(
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
        let reserved = SessionCapture::record_row(&manifest, &dir, work, context)?;
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
    /// used by its tools. A later attachment transfer never rewrites this process.
    pub(crate) fn claim_session_attachment(&self) -> StoreResult<()> {
        let lf_process_id = crate::journal::current_lf_process_id().ok_or_else(|| {
            StoreError::InvalidAuthority("AgentProcess requires an admitted invocation".into())
        })?;
        let mut capture = self.0.lock().expect("Session capture mutex poisoned");
        if capture.attachment.is_some() {
            return Ok(());
        }
        let store = row_store(&capture.dir)?;
        let session = store
            .session_for_artifact(&capture.manifest.artifact_key)?
            .ok_or_else(|| {
                StoreError::InvalidAuthority(
                    "AgentProcess requires an admitted conversation".into(),
                )
            })?;
        let attachment = resume_session_agent_process(&store, &session.id, &lf_process_id)?;
        if let Some(owner) = crate::transport::current() {
            store.bind_session_transport(&session.id, &attachment, owner)?;
        }
        capture.attachment = Some((session.id.to_string(), attachment));
        drop(capture);
        self.register_interrupt();
        Ok(())
    }

    fn register_interrupt(&self) {
        let capture = Arc::downgrade(&self.0);
        crate::agent::register_interrupt_cleanup(move || {
            if let Some(capture) = capture.upgrade() {
                let mut capture = capture.lock().expect("Session capture mutex poisoned");
                if capture.settled_outcome.is_none() {
                    if let Err(error) = capture.finish("interrupted") {
                        tracing::warn!(%error, "record interrupted Session exit");
                    }
                }
            }
        });
    }

    pub(crate) fn conversation_resume_token(&self) -> StoreResult<Option<AgentSessionId>> {
        let capture = self.0.lock().expect("Session capture mutex poisoned");
        let store = row_store(&capture.dir)?;
        let Some(session) = store.session_for_artifact(&capture.manifest.artifact_key)? else {
            return Ok(None);
        };
        store.session_thread(&session.id)
    }

    pub(crate) fn spawn_native_agent(
        &self,
        mut command: std::process::Command,
    ) -> anyhow::Result<(std::process::Child, crate::process::SessionAttachment)> {
        let mut capture = self.0.lock().expect("Session capture mutex poisoned");
        if capture.settled_outcome.is_some() {
            return Err(StoreError::InvalidAuthority("Capture already settled".into()).into());
        }
        let (session, expected) = capture.attachment.as_ref().ok_or_else(|| {
            StoreError::InvalidAuthority("AgentProcess launch has no attachment".into())
        })?;
        let store = row_store(&capture.dir)?;
        let session = session.clone();
        let attachment = store.prepare_session_agent_process(&session, expected)?;
        command.env(
            crate::process::AGENT_CALLER_ENV,
            serde_json::to_string(&attachment.caller(session.clone().into()))?,
        );
        capture.attachment = Some((session.clone(), attachment.clone()));
        // Keep the capture's settlement snapshot fixed through admission. The
        // recorder only touches the store, never this mutex.
        let child = crate::harness::agent_process::spawn_native(
            command,
            &(store, session, attachment.clone()),
        )?;
        Ok((child, attachment))
    }

    pub(crate) fn record_native_agent_exit(
        &self,
        expected: &crate::process::SessionAttachment,
    ) -> StoreResult<()> {
        let capture = self.0.lock().expect("Session capture mutex poisoned");
        let Some((session, attachment)) = &capture.attachment else {
            return Err(StoreError::InvalidAuthority(
                "Capture has no attachment".into(),
            ));
        };
        if attachment != expected {
            return Err(StoreError::InvalidAuthority(
                "Capture attachment changed".into(),
            ));
        }
        row_store(&capture.dir)?.record_agent_process_exit(session, expected, true)
    }

    pub(crate) fn session_attachment(&self) -> Option<(String, crate::process::SessionAttachment)> {
        self.0
            .lock()
            .expect("Session capture mutex poisoned")
            .attachment
            .clone()
    }

    pub(crate) fn attachment_replaced(&self) -> StoreResult<bool> {
        let capture = self.0.lock().expect("Session capture mutex poisoned");
        let Some((session, expected)) = &capture.attachment else {
            return Ok(false);
        };
        Ok(row_store(&capture.dir)?
            .session_attachment(session)?
            .as_ref()
            != Some(expected))
    }

    /// Only the invocation's capture owner advances its settlement snapshot.
    /// Dispatch and history retain the immutable snapshot they already took.
    pub(crate) fn prepare_agent_process(
        &self,
        session: &str,
        expected: &crate::process::SessionAttachment,
    ) -> StoreResult<crate::process::SessionAttachment> {
        let mut capture = self.0.lock().expect("Session capture mutex poisoned");
        if capture.settled_outcome.is_some()
            || capture
                .attachment
                .as_ref()
                .map(|(id, owner)| (id.as_str(), owner))
                != Some((session, expected))
        {
            return Err(StoreError::InvalidAuthority(
                "Capture attachment changed".into(),
            ));
        }
        let next = row_store(&capture.dir)?.prepare_session_agent_process(session, expected)?;
        capture.attachment = Some((session.to_owned(), next.clone()));
        Ok(next)
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
        if let Some((session, attachment)) = &capture.attachment {
            environment.insert(
                crate::process::AGENT_CALLER_ENV.into(),
                serde_json::to_string(&attachment.caller(session.clone().into()))
                    .expect("caller provenance serializes"),
            );
        }
        environment
    }

    pub(crate) fn mark_spawn_requested(&self) {
        self.with_capture(SessionCapture::start_attempt);
    }

    pub(crate) fn record_conversation(&self, event: ConversationEvent) {
        self.with_capture(|capture| capture.record_conversation(event));
    }

    pub(crate) fn retry_agent_process(
        &self,
        provider: String,
        model: Option<String>,
        account_id: Option<crate::store::ProviderAccountId>,
        resume_thread: Option<&AgentSessionId>,
    ) -> anyhow::Result<()> {
        let mut capture = self.0.lock().expect("Session capture mutex poisoned");
        if capture.settled_outcome.is_some() {
            anyhow::bail!("Capture already settled");
        }
        let (session, expected) = capture.attachment.as_ref().ok_or_else(|| {
            StoreError::InvalidAuthority("AgentProcess retry has no attachment".into())
        })?;
        let store = row_store(&capture.dir)?;
        let saved_thread = store
            .session_thread(session)?
            .or_else(|| capture.agent_session.clone());
        let account_changed = capture.account_observed && capture.account_id != account_id;
        if (account_changed && resume_thread.is_some())
            || (!account_changed && saved_thread.as_ref() != resume_thread)
        {
            anyhow::bail!(
                "AgentProcess retry must retain its thread unless the selected account changes"
            );
        }
        let next = store.replace_session_agent_process(session, expected, resume_thread, || {
            crate::harness::agent_process::close_session_agent_process(&store, session)
        })?;
        capture.attachment = Some((session.clone(), next));
        if let Err(error) = capture.fail_and_begin_attempt(provider, model, account_id) {
            capture.warn_telemetry(error);
        }
        Ok(())
    }

    pub(crate) fn observe_provider(
        &self,
        session_id: Option<AgentSessionId>,
        account_id: Option<crate::store::ProviderAccountId>,
    ) {
        self.with_capture(|capture| {
            let account_changed = !capture.account_observed || capture.account_id != account_id;
            if account_changed {
                capture.record_fact(CaptureFact::ProviderAccountSelected {
                    attempt_key: capture.attempt_key(),
                    account_id: account_id.clone(),
                })?;
                capture.account_id = account_id;
                capture.account_observed = true;
            }
            let Some(session_id) = session_id else {
                return Ok(());
            };
            if !account_changed && capture.agent_session.as_ref() == Some(&session_id) {
                return Ok(());
            }
            write_provider_session(&capture.dir, &session_id, capture.account_id.clone())?;
            capture.record_fact(CaptureFact::ProviderSessionObserved {
                attempt_key: capture.attempt_key(),
                agent_session: session_id.clone(),
            })?;
            capture.agent_session = Some(session_id);
            Ok(())
        });
    }

    pub(crate) fn final_answer(&self) -> StoreResult<Option<String>> {
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
    attachment: Option<(String, crate::process::SessionAttachment)>,
    manifest: SessionCaptureManifest,
    dir: PathBuf,
    provider: String,
    model: Option<String>,
    account_id: Option<crate::store::ProviderAccountId>,
    account_observed: bool,
    agent_session: Option<AgentSessionId>,
    attempt: u32,
    attempt_started: bool,
    event_seq: u64,
    /// Where facts are kept; none for a mechanical command's capture.
    history: Option<(crate::store::sqlite::SqliteStore, crate::session::LfSession)>,
    /// When the stream's previous event arrived, to record silences.
    previous_event: Option<OffsetDateTime>,
    telemetry_warned: bool,
    settled_outcome: Option<String>,
    activity: activity::Observer,
}

impl SessionCapture {
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
        let history = if manifest.harness == "loopflow" {
            Ok(None)
        } else {
            row_store(&dir).and_then(|store| {
                let session = store.session_for_artifact(&manifest.artifact_key)?;
                Ok(session.map(|session| (store, session)))
            })
        };
        let history = history.unwrap_or_else(|error| {
            tracing::warn!(%error, "Session history unavailable");
            None
        });
        if let Some((store, session)) = &history {
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
            attachment: None,
            provider: manifest.harness.clone(),
            model: manifest.model.clone(),
            account_id: manifest
                .process
                .as_ref()
                .and_then(|process| process.account_id.clone()),
            account_observed: false,
            agent_session: None,
            manifest,
            dir,
            attempt: 1,
            attempt_started: false,
            event_seq: 0,
            history,
            previous_event: None,
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
        self.record_fact(CaptureFact::ProviderAttemptStarted {
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
            self.record_fact(CaptureFact::ProviderAttemptFinished {
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
        self.agent_session = None;
        self.attempt += 1;
        self.attempt_started = false;
        let start_error = self.start_attempt().err();
        match (finish_error, start_error) {
            (None, None) => Ok(()),
            (Some(error), None) | (None, Some(error)) => Err(error),
            (Some(finish), Some(start)) => Err(std::io::Error::other(format!(
                "failed to record prior attempt outcome: {finish}; failed to record new attempt: {start}"
            ))),
        }
    }

    /// Keep how a turn ended; any other event only ticks the silence clock.
    fn record_conversation(&mut self, event: ConversationEvent) -> std::io::Result<()> {
        let end = match event {
            ConversationEvent::TurnCompleted { turn_id, status } => {
                TurnEnd::TurnCompleted { turn_id, status }
            }
            ConversationEvent::Error {
                code,
                message,
                evidence,
            } => TurnEnd::Error {
                code,
                message,
                evidence: evidence.map(Box::new),
            },
            _ => return self.next_position().map(drop),
        };
        self.record_fact(CaptureFact::Conversation { event: end })
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
            if let Err(error) = self.record_fact(CaptureFact::ProviderAttemptFinished {
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
        if let Some((session, attachment)) = self.attachment.take() {
            match row_store(&self.dir)
                .and_then(|store| finish_session_attachment(&store, &session, &attachment, outcome))
            {
                Ok(_) | Err(StoreError::InvalidAuthority(_)) => {}
                Err(error) => return Err(std::io::Error::other(error)),
            }
        }
        Ok(())
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
        let Some((store, session)) = &self.history else {
            return Ok(());
        };
        store
            .record_capture_fact(session, &self.manifest.artifact_key, seq, at, fact)
            .map_err(std::io::Error::other)
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

/// The store that holds the conversation for the capture recorded at `dir`.
fn row_store(dir: &Path) -> StoreResult<crate::store::sqlite::SqliteStore> {
    crate::store::sqlite::SqliteStore::new(&row_database(dir)?)
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
        if owner.id.as_str() != crate::id::canonical_session(&caller.session_id) {
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
    let store = crate::store::sqlite::SqliteStore::open_processes_read_only(&database_in(&home)?)?;
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

fn record_error(error: std::io::Error) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use super::{
        read_provider_session, AgentProcessRequest, CaptureHandle, SessionCaptureManifest,
        SessionCaptureSpec, SubjectAttribution, TerminalReceipt,
    };
    use crate::agent::AgentConfig;
    use crate::chat::types::{ConversationEvent, TurnUsage};
    use time::OffsetDateTime;

    #[test]
    fn native_usage_reads_each_providers_counter_once_by_its_shape() {
        use crate::session::SessionEventKind::{Started, Usage};
        let event = |seq, thread: &str, kind, payload| crate::session::SessionEvent {
            seq,
            session_id: "session".into(),
            agent_session: Some(crate::id::AgentSessionId::from(thread)),
            provider_turn: Some("turn".into()),
            kind,
            agent_process_id: None,
            lf_process_id: None,
            task_id: None,
            wave_id: None,
            observed_at: 1,
            payload,
        };
        let tokens = |id: &str, input: u64, output: u64| {
            serde_json::json!({"provider":"opencode","message":{"id":id,
                "tokens":{"input":input,"output":output,"reasoning":0,"cache":{"read":0,"write":0}}}})
        };
        let codex = serde_json::json!({"inputTokens":30,"outputTokens":5,"cachedInputTokens":0,"reasoningOutputTokens":0});
        let history = vec![
            event(1, "pi", Started, serde_json::json!({})),
            event(
                2,
                "pi",
                Usage,
                serde_json::json!({"provider":"pi","usage":{"input_tokens":10},"final":false}),
            ),
            event(
                3,
                "pi",
                Usage,
                serde_json::json!({"provider":"pi","usage":{"input_tokens":20,"output_tokens":6},"final":true}),
            ),
            event(4, "opencode", Started, serde_json::json!({})),
            event(5, "opencode", Usage, tokens("m1", 3, 1)),
            event(6, "opencode", Usage, tokens("m2", 4, 2)),
            event(7, "codex", Started, serde_json::json!({})),
            event(
                8,
                "codex",
                Usage,
                serde_json::json!({"total":codex,"last":codex}),
            ),
        ];
        let (readings, unread) = super::native_usage(&history);
        assert_eq!(unread, 0);
        let usage = super::reduce_usage(&readings);
        assert_eq!((usage.streams, usage.final_streams, usage.gaps), (4, 3, 0));
        assert_eq!(usage.input_tokens, Some(20 + 3 + 4 + 30));
        assert_eq!(usage.output_tokens, Some(6 + 1 + 2 + 5));
    }

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
            graph: crate::session_record::SessionGraphMembership::Independent,
            work: None,
        }
    }

    #[test]
    fn native_capture_replacement_retains_each_spawn_and_rejects_late_exit() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let home = ledger.home();
        crate::journal::with_runtime(home, &["lf".into(), "skill".into()], || {
            let capture = CaptureHandle::begin_at(home, spec(home))?;
            capture.claim_session_attachment()?;
            let (session, first) = capture.session_attachment().unwrap();
            let store = super::row_store(&capture.artifact_dir())?;
            let command = || {
                let mut command = std::process::Command::new("/bin/sh");
                command.env_clear().args(["-c", "exit 42"]);
                command
            };
            let (mut child, snapshot) = capture.spawn_native_agent(command())?;
            assert_eq!(snapshot, first);
            assert_eq!(child.wait()?.code(), Some(42));
            // Observed wait, not successful spawn or a finished capture, ends it.
            capture.record_native_agent_exit(&first)?;
            let ended = store.process(&first.agent_process_id)?.unwrap();
            assert_eq!(ended.pid, Some(child.id()));
            assert!(ended.completed_at.is_some());

            let (mut child, snapshot) = capture.spawn_native_agent(command())?;
            let second = snapshot;
            assert_ne!(first.agent_process_id, second.agent_process_id);
            assert_eq!(
                capture.session_attachment(),
                Some((session.clone(), second.clone()))
            );
            assert_eq!(child.wait()?.code(), Some(42));
            assert!(capture.record_native_agent_exit(&first).is_err());
            assert!(store
                .process(&second.agent_process_id)?
                .unwrap()
                .completed_at
                .is_none());
            // The child exited but without its wait receipt another launch is refused.
            assert!(capture.spawn_native_agent(command()).is_err());
            capture.record_native_agent_exit(&second)?;
            assert_eq!(store.process(&first.agent_process_id)?, Some(ended));
            capture.finish("completed")?;
            assert!(capture.spawn_native_agent(command()).is_err());
            assert!(store
                .session_attachment(&session)?
                .unwrap()
                .lf_process_id
                .is_none());
            Ok(())
        })
        .unwrap();
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
    fn history_times_provider_work_from_its_first_recorded_attempt() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        capture.mark_spawn_requested();
        capture
            .0
            .lock()
            .unwrap()
            .fail_and_begin_attempt("claude".to_string(), None, None)
            .unwrap();
        capture.finish("completed").unwrap();

        let history = super::row_store(&capture.artifact_dir())
            .unwrap()
            .input_history(capture.artifact_key().as_str())
            .unwrap();
        let attempts = history
            .providers
            .iter()
            .filter(|provider| {
                matches!(
                    provider.reference,
                    super::ProviderHistoryReference::RecordedAttempt { .. }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0].outcome.as_deref(), Some("failed"));
        assert_eq!(attempts[1].outcome.as_deref(), Some("completed"));
        assert!(attempts[0].started_at.is_some());
        assert_eq!(history.first_provider_attempt_at, attempts[0].started_at);
    }

    #[test]
    fn prepared_run_keeps_its_recorded_pr_instead_of_the_launching_pr() {
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        let original = crate::work::task::TaskPrId::new();
        let mut step = super::SessionGraphStep {
            task_id: Some(crate::work::task::TaskId::new()),
            task_pr_id: Some(original.clone()),
            invocation_id: "invocation".into(),
            graph: "feature".into(),
            step: "review".into(),
            node: Some("1".into()),
            key: Some(1),
            iterations: Some(Vec::new()),
        };
        let mut prepared = spec(home.path());
        prepared.graph = super::SessionGraphMembership::Step(step.clone());
        let id = CaptureHandle::prepare_at(home.path(), prepared, None).unwrap();
        let (dir, _) = super::resolve_manifest(home.path(), id.as_str()).unwrap();
        step.task_pr_id = Some(crate::work::task::TaskPrId::new());
        let mut process = spec(home.path());
        process.graph = super::SessionGraphMembership::Step(step);
        let context = crate::trace::PreparedTurnContext::from_prompts("system", "review");
        let capture =
            CaptureHandle::start_prepared(home.path(), &id, process, &context, None).unwrap();
        capture.finish("completed").unwrap();

        let manifest = super::read_manifest(&dir).unwrap();
        let Some(super::SessionGraphMembership::Step(step)) = manifest.graph else {
            panic!("prepared membership retained");
        };
        assert_eq!(step.task_pr_id, Some(original));
    }

    #[test]
    fn helper_capture_admits_a_headless_conversation_with_its_input_and_outcome() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let home = tempfile::tempdir().unwrap();
        let process = AgentProcessRequest::from_prepared(&AgentConfig {
            task_prompt: "repair the failed operation".into(),
            ..Default::default()
        });
        let capture =
            CaptureHandle::begin_at_with_request(home.path(), spec(home.path()), process).unwrap();
        let store = super::row_store(&capture.artifact_dir()).unwrap();
        let session = store
            .session_for_artifact(&capture.artifact_key())
            .unwrap()
            .unwrap();
        let run = session.clone();
        assert!(!session.interactive);
        assert_eq!(session.title, "repair failed operation");
        assert_eq!(run.provider.as_deref(), Some("proof"));
        assert_eq!(
            super::read_manifest(&capture.artifact_dir())
                .unwrap()
                .process
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
    fn reserved_publication_recovers_artifacts_without_repeating_process_authority() {
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
            // An absent provider receipt does not grant a second process.
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
            CaptureHandle::start_prepared(home.path(), &id, spec(home.path()), &context, None)
                .unwrap();
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
            CaptureHandle::start_prepared(home.path(), &id, spec(home.path()), &context, None)
                .is_err()
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
    fn manifest_round_trips_the_exact_prepared_process_without_ambient_authority() {
        let home = tempfile::tempdir().unwrap();
        let config = AgentConfig {
            chrome: true,
            system_prompt: "system context\r\nwith unicode λ\n \t".to_string(),
            task_prompt: "authored task\n\nwith trailing space ".to_string(),
            agent: Some("codex:gpt-5.6".to_string()),
            provider_account_id: Some(
                crate::store::ProviderAccountId::parse("engineering").unwrap(),
            ),
            max_turns: Some(7),
            resume_token: Some("resume-secret".into()),
            skip_permissions: true,
            directive_relay: Some("/tmp/directive-secret".into()),
            env: BTreeMap::from([("TOKEN".to_string(), "ambient-secret".to_string())]),
            ..AgentConfig::default()
        };
        let expected = AgentProcessRequest::from_prepared(&config);
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
        assert_eq!(manifest.process, Some(expected));
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
        capture.record_conversation(ConversationEvent::TextDelta {
            turn_id: "provider-turn".to_string(),
            content: "provider response".to_string(),
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
            ["manifest.json", "terminal.json"].map(std::ffi::OsString::from)
        );

        let error = capture.finish("failed").unwrap_err();
        assert!(error.to_string().contains("already settled as completed"));
        let unchanged: TerminalReceipt =
            serde_json::from_slice(&fs::read(dir.join("terminal.json")).unwrap()).unwrap();
        assert_eq!(unchanged.outcome, "completed");
    }

    #[test]
    fn history_keeps_only_how_a_turn_ended() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        let turn = || "turn-1".to_string();
        capture.record_conversation(ConversationEvent::TurnStarted { turn_id: turn() });
        for word in ["streamed ", "one ", "token ", "at a time"] {
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
        capture.record_conversation(ConversationEvent::UsageCheckpoint {
            turn_id: turn(),
            usage: TurnUsage {
                input_tokens: Some(10),
                ..TurnUsage::default()
            },
            final_receipt: true,
        });
        capture.record_conversation(ConversationEvent::TurnCompleted {
            turn_id: turn(),
            status: crate::chat::types::Lifecycle::Completed,
        });
        capture.finish("completed").unwrap();

        // Capture prose remains readable, but cannot substitute for native turn output.
        assert_eq!(capture.final_answer().unwrap(), None);
        assert!(!capture.artifact_dir().join("events.jsonl").exists());

        let history = super::row_store(&capture.artifact_dir())
            .unwrap()
            .input_events(&capture.artifact_key())
            .unwrap();
        let kept: Vec<_> = history
            .iter()
            .map(|event| match event["type"].as_str().unwrap() {
                "conversation" => event["event"]["type"].as_str().unwrap().to_string(),
                other => other.to_string(),
            })
            .collect();
        assert_eq!(kept, ["turn_completed"]);
    }

    #[test]
    fn history_records_a_long_gap_as_a_silence() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        let say = |content: &str| {
            capture.record_conversation(ConversationEvent::TextDelta {
                turn_id: "turn-1".into(),
                content: content.into(),
            })
        };
        say("thinking");
        let since = OffsetDateTime::now_utc() - time::Duration::seconds(700);
        capture.0.lock().unwrap().previous_event = Some(since);
        say("still thinking");
        say("done");

        let store = super::row_store(&capture.artifact_dir()).unwrap();
        let silences = store.input_silences(&capture.artifact_key()).unwrap();
        assert_eq!(silences.len(), 1);
        assert_eq!(silences[0].0, since.unix_timestamp());
        assert!(store
            .input_events(&capture.artifact_key())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn continuation_resumes_the_saved_thread_on_a_fresh_agent_process() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let original = CaptureHandle::begin_at(ledger.home(), spec(ledger.home())).unwrap();
        let store = super::row_store(&original.artifact_dir()).unwrap();
        let session = store
            .session_for_artifact(&original.artifact_key())
            .unwrap()
            .unwrap();
        let command = vec!["lf".into(), "skill".into()];
        crate::journal::with_runtime(ledger.home(), &command, || {
            original.claim_session_attachment()?;
            let (_, attachment) = original.session_attachment().unwrap();
            store.record_session_connection(
                &session.id,
                &attachment,
                "/retained.sock",
                &"native-thread".into(),
            )?;
            // The previous provider is a throwaway child whose exit is observed
            // before continuation; a live one would refuse replacement.
            let mut provider = std::process::Command::new("/bin/sleep").arg("60").spawn()?;
            store.record_agent_process_identity(
                &session.id,
                &attachment,
                provider.id(),
                crate::journal::process_started_at(provider.id())?.unwrap(),
            )?;
            provider.kill()?;
            provider.wait()?;
            original.finish("completed")?;
            Ok(())
        })
        .unwrap();
        assert!(store
            .session(&session.id)
            .unwrap()
            .unwrap()
            .completed_at
            .is_some());
        let history = store.session_history(&session.id, 0, 0).unwrap();
        let attachment = store.session_attachment(&session.id).unwrap().unwrap();
        let manifest = fs::read(original.artifact_dir().join("manifest.json")).unwrap();
        let request = AgentProcessRequest::from_prepared(&AgentConfig {
            task_prompt: "the next instruction".into(),
            ..AgentConfig::default()
        });
        let context = crate::trace::PreparedTurnContext::from_prompts("", "the next instruction");
        crate::journal::with_runtime(ledger.home(), &command, || {
            let next = CaptureHandle::continue_with_context(
                &session.id,
                spec(ledger.home()),
                &context,
                request.clone(),
            )?;
            let (id, next_attachment) = next.session_attachment().unwrap();
            assert_eq!(id, session.id.as_str());
            assert_eq!(
                next.conversation_resume_token()?,
                Some("native-thread".into())
            );
            // The finished attachment's AgentProcess is never adopted.
            assert_ne!(
                next_attachment.agent_process_id,
                attachment.agent_process_id
            );
            assert_eq!(
                Some(&next_attachment.provider_lf_process_id),
                next_attachment.lf_process_id.as_ref()
            );
            assert!(store.session_connection(&session.id)?.is_none());
            assert!(store.agent_process_identity(&session.id)?.is_none());
            assert_ne!(next_attachment.token, attachment.token);
            let saved = super::read_manifest(&next.artifact_dir()).unwrap();
            assert_eq!(
                serde_json::to_value(saved.process).unwrap(),
                serde_json::to_value(Some(&request)).unwrap()
            );
            assert!(saved.context.is_some());
            let current = store.session(&session.id)?.unwrap();
            assert!(current.completed_at.is_some());
            assert!(store
                .session_history(&session.id, 0, 0)?
                .starts_with(&history));
            assert_ne!(current.captured, session.captured);
            assert_eq!(current.cwd, session.cwd);
            assert_eq!(current.title, session.title);
            assert!(current.input_published);
            assert_eq!(current.task_id, session.task_id);
            assert_eq!(current.wave_id, session.wave_id);
            assert_eq!(
                store
                    .session_for_artifact(&original.artifact_key())?
                    .unwrap()
                    .id,
                session.id
            );
            assert_eq!(
                fs::read(original.artifact_dir().join("manifest.json"))?,
                manifest
            );
            next.finish("failed")?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn continuation_publication_failure_keeps_reservation_without_settling_a_turn() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let original = CaptureHandle::begin_at(ledger.home(), spec(ledger.home())).unwrap();
        let store = super::row_store(&original.artifact_dir()).unwrap();
        let session = store
            .session_for_artifact(&original.artifact_key())
            .unwrap()
            .unwrap();
        fs::rename(
            ledger.home().join("runs"),
            ledger.home().join("retained-runs"),
        )
        .unwrap();
        fs::write(ledger.home().join("runs"), "unavailable capture directory").unwrap();
        let command = vec!["lf".into(), "skill".into()];
        crate::journal::with_runtime(ledger.home(), &command, || {
            assert!(CaptureHandle::continue_with_context(
                &session.id,
                spec(ledger.home()),
                &crate::trace::PreparedTurnContext::from_prompts("", "next"),
                AgentProcessRequest::from_prepared(&AgentConfig::default()),
            )
            .is_err());
            let next = store.session(&session.id)?.unwrap();
            assert_ne!(next.captured, session.captured);
            assert!(!next.input_published);
            assert!(next.completed_at.is_none());
            assert!(store
                .session_attachment(&session.id)?
                .unwrap()
                .lf_process_id
                .is_none());
            assert!(!store
                .session_history(&session.id, 0, 0)?
                .iter()
                .any(|event| {
                    matches!(
                        event.kind,
                        crate::session::SessionEventKind::Started
                            | crate::session::SessionEventKind::Completed
                    )
                }));
            Ok(())
        })
        .unwrap();
        fs::remove_file(ledger.home().join("runs")).unwrap();
        fs::rename(
            ledger.home().join("retained-runs"),
            ledger.home().join("runs"),
        )
        .unwrap();
    }

    /// A Session whose attached LfProcess is a throwaway child, so a test can kill it.
    #[cfg(unix)]
    fn attached_session(
        ledger: &crate::journal::TestLedgerGuard,
    ) -> (
        crate::store::sqlite::SqliteStore,
        crate::process::SessionAttachment,
        std::process::Child,
        crate::id::LfProcessId,
    ) {
        let path = ledger.home().join("loopflow.db");
        let store = crate::store::sqlite::SqliteStore::open_ephemeral(&path).unwrap();
        store.test_session("conversation", &super::new_artifact_key());
        let first = crate::id::LfProcessId::new();
        let second = crate::id::LfProcessId::new();
        let sql = rusqlite::Connection::open(&path).unwrap();
        let trace = crate::id::TraceId::new();
        for process in [&first, &second] {
            sql.execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?2,1)",
                rusqlite::params![process, trace],
            )
            .unwrap();
        }
        sql.execute(
            "UPDATE agent_sessions SET provider='codex',interactive=0",
            [],
        )
        .unwrap();
        let process = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let receipts = ledger.home().join(crate::journal::PROCESS_RECEIPT_ROOT);
        fs::create_dir_all(&receipts).unwrap();
        fs::write(
            receipts.join(format!("{first}.json")),
            serde_json::to_vec(&crate::journal::ProcessReceipt {
                schema_version: 1,
                trace_id: trace.to_string(),
                lf_process_id: first.to_string(),
                pid: process.id(),
                started_at: crate::journal::process_started_at(process.id())
                    .unwrap()
                    .unwrap(),
            })
            .unwrap(),
        )
        .unwrap();
        let attachment = store
            .claim_session_attachment("conversation", None, &first, true)
            .unwrap();
        (store, attachment, process, second)
    }

    #[test]
    #[cfg(unix)]
    fn resume_dead_attachment_ends_its_agent_before_starting_another() {
        use std::os::unix::process::CommandExt;

        let ledger = crate::journal::TestLedgerGuard::new();
        let (store, attachment, mut process, next) = attached_session(&ledger);
        let mut agent_process = std::process::Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let started = crate::journal::process_started_at(agent_process.id())
            .unwrap()
            .unwrap();
        store
            .record_agent_process_identity("conversation", &attachment, agent_process.id(), started)
            .unwrap();
        let socket = ledger.home().join("agent.sock");
        store
            .record_session_connection(
                "conversation",
                &attachment,
                socket.to_str().unwrap(),
                &"saved-thread".into(),
            )
            .unwrap();

        // A live attachment keeps both the conversation and its AgentProcess.
        let error = super::resume_session_agent_process(&store, "conversation", &next).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Conversation already has an attached LfProcess; connect to it"),
            "{error}"
        );
        assert!(agent_process.try_wait().unwrap().is_none());

        process.kill().unwrap();
        process.wait().unwrap();
        let replacement =
            super::resume_session_agent_process(&store, "conversation", &next).unwrap();
        assert_eq!(
            crate::journal::process_identity_evidence(agent_process.id(), started),
            crate::journal::ProcessIdentityEvidence::Dead,
        );
        // Ending the AgentProcess may already have reaped this exact child.
        let _ = agent_process.wait();
        assert_ne!(replacement.agent_process_id, attachment.agent_process_id);
        assert_eq!(replacement.provider_lf_process_id, next);
        assert!(store.session_connection("conversation").unwrap().is_none());
        assert!(store
            .agent_process_identity("conversation")
            .unwrap()
            .is_none());
        assert_eq!(
            store.session_thread("conversation").unwrap(),
            Some("saved-thread".into())
        );
    }

    #[test]
    #[cfg(unix)]
    fn resume_refuses_live_agent_that_cannot_be_ended() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let (store, attachment, mut process, next) = attached_session(&ledger);
        store
            .record_session_connection(
                "conversation",
                &attachment,
                ledger.home().join("absent.sock").to_str().unwrap(),
                &"saved-thread".into(),
            )
            .unwrap();
        // A provider outside a group of its own is never signalled.
        let mut provider = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let started = crate::journal::process_started_at(provider.id())
            .unwrap()
            .unwrap();
        store
            .record_agent_process_identity("conversation", &attachment, provider.id(), started)
            .unwrap();
        process.kill().unwrap();
        process.wait().unwrap();

        let error = super::resume_session_agent_process(&store, "conversation", &next).unwrap_err();
        let remained_alive = provider.try_wait().unwrap().is_none();
        let retained_attachment = store.session_attachment("conversation").unwrap();
        provider.kill().unwrap();
        provider.wait().unwrap();
        assert!(
            error
                .to_string()
                .contains("does not lead its own process group"),
            "{error}"
        );
        assert!(remained_alive);
        assert_eq!(retained_attachment, Some(attachment.clone()));

        let replacement =
            super::resume_session_agent_process(&store, "conversation", &next).unwrap();
        assert_ne!(replacement.agent_process_id, attachment.agent_process_id);
    }

    #[test]
    fn agent_session_identity_is_durable_before_the_provider_starts() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        capture.observe_provider(Some("provider-session".into()), None);

        assert_eq!(
            read_provider_session(&capture.artifact_dir())
                .unwrap()
                .map(|session| session.agent_session),
            Some("provider-session".into())
        );
    }

    #[test]
    fn provider_session_preserves_the_selected_account() {
        let home = tempfile::tempdir().unwrap();
        let capture = CaptureHandle::begin_at(home.path(), spec(home.path())).unwrap();
        let account_id = crate::store::ProviderAccountId::parse("primary").unwrap();
        super::write_provider_session(
            &capture.artifact_dir(),
            &"provider-session".into(),
            Some(account_id.clone()),
        )
        .unwrap();

        fs::remove_dir_all(capture.artifact_dir()).unwrap();
        let session = read_provider_session(&capture.artifact_dir())
            .unwrap()
            .expect("provider session reference");
        assert_eq!(session.agent_session, "provider-session".into());
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
        let facts = || {
            super::row_store(&dir)
                .unwrap()
                .input_events(&capture.artifact_key())
                .unwrap()
        };
        let first = crate::store::ProviderAccountId::parse("primary").unwrap();
        let second = crate::store::ProviderAccountId::parse("fallback").unwrap();
        capture.mark_spawn_requested();
        capture.observe_provider(None, Some(first.clone()));

        assert!(!dir.join("terminal.json").exists());
        assert!(read_provider_session(&dir).unwrap().is_none());
        assert!(facts().iter().any(|fact| fact["account_id"] == "primary"));

        capture.observe_provider(Some("first-session".into()), Some(first.clone()));
        capture.observe_provider(Some("first-session".into()), Some(first.clone()));
        assert_eq!(
            read_provider_session(&dir).unwrap().unwrap().account_id,
            Some(first)
        );
        capture
            .0
            .lock()
            .unwrap()
            .fail_and_begin_attempt("proof".into(), None, Some(second.clone()))
            .unwrap();
        capture.observe_provider(Some("second-session".into()), Some(second.clone()));
        assert!(!dir.join("provider-session.json").exists());
        rusqlite::Connection::open(super::row_database(&dir).unwrap()).unwrap().execute(
            "DELETE FROM session_events WHERE kind='observed' AND json_extract(payload,'$.input_id')=?1 AND json_extract(payload,'$.source') LIKE 'provider-session:%'",
            [capture.artifact_key().as_str()],
        ).unwrap();
        let recovered = read_provider_session(&dir).unwrap().unwrap();
        assert_eq!(recovered.agent_session, "second-session".into());
        assert_eq!(recovered.account_id, Some(second));
        capture
            .0
            .lock()
            .unwrap()
            .fail_and_begin_attempt("proof".into(), None, None)
            .unwrap();
        capture.observe_provider(Some("ambient-session".into()), None);
        capture.finish("completed").unwrap();

        let events = facts();
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
        assert_eq!(ambient.agent_session, "ambient-session".into());
        assert_eq!(ambient.account_id, None);
        assert!(!dir.join("provider-session.json").exists());
        rusqlite::Connection::open(super::row_database(&dir).unwrap()).unwrap().execute(
            "DELETE FROM session_events WHERE kind='observed' AND json_extract(payload,'$.input_id')=?1 AND json_extract(payload,'$.source') LIKE 'provider-session:%'",
            [capture.artifact_key().as_str()],
        ).unwrap();
        assert_eq!(read_provider_session(&dir).unwrap(), Some(ambient));
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
