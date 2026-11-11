//! Captured Session inputs and append-only provider evidence.

pub(crate) mod activity;
mod capture;
#[cfg(test)]
mod capture_tests;

use crate::id::AgentSessionId;
pub(crate) use capture::CaptureHandoff;

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::chat::types::{ConversationEvent, TurnUsage};
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

    pub(crate) fn from_prepared(
        config: &crate::agent::AgentConfig,
    ) -> Result<Self, crate::error::LoadError> {
        Ok(Self {
            system_prompt: crate::agent::system_prompt_with_structured_replies(config),
            conversation_context: config.conversation_context.clone(),
            task_prompt: config.task_prompt.clone(),
            skill_invocation: config.skill_invocation.clone(),
            output_schema: config.output_schema.clone(),
            agent: config.agent()?.to_string(),
            account_id: config.provider_account_id.clone(),
            max_turns: config.max_turns,
            write_scope: config.write_scope,
            execution_boundary: config.execution_boundary.clone(),
            skip_permissions: config.skip_permissions,
            chrome: config.chrome,
            codex_service_tier: config.codex_service_tier,
        })
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
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

/// Codex counters before the turn's first request.
pub(crate) fn native_baseline(receipt: &serde_json::Value) -> serde_json::Value {
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
pub(crate) fn native_peak(receipt: &serde_json::Value) -> Option<u64> {
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

/// Who chose a Session's current title. A human name is never replaced by a
/// generated suggestion. `Unavailable` is never stored: the canonical name
/// lives on another Machine and the reader only has a local display label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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

/// Append-only capture data. This value has no attachment or process authority.
#[derive(Debug)]
pub(crate) struct SessionRecorder {
    pub(crate) manifest: SessionCaptureManifest,
    pub(crate) dir: PathBuf,
    /// The live identity is known, but its last observation could not be kept.
    provider_observation_pending: bool,
    attempt: u32,
    attempt_started: bool,
    event_seq: u64,
    /// Where facts are kept; none for a mechanical command's capture.
    history: Option<(crate::store::sqlite::SqliteStore, crate::session::LfSession)>,
    /// When the stream's previous event arrived, to record silences.
    previous_event: Option<OffsetDateTime>,
    telemetry_warned: bool,
    terminal_recorded: bool,
    pub(crate) activity: activity::Observer,
}

impl SessionRecorder {
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
            manifest,
            dir,
            provider_observation_pending: false,
            attempt: 1,
            attempt_started: false,
            event_seq: 0,
            history,
            previous_event: None,
            telemetry_warned: false,
            terminal_recorded: false,
            activity: activity::Observer::default(),
        }
    }

    pub(crate) fn observe_provider(
        &mut self,
        session: Option<&AgentSessionId>,
        account: Option<&crate::store::ProviderAccountId>,
        account_changed: bool,
        thread_changed: bool,
    ) -> std::io::Result<()> {
        let retry = self.provider_observation_pending;
        self.provider_observation_pending = true;
        if account_changed || retry {
            self.record_fact(CaptureFact::ProviderAccountSelected {
                attempt_key: self.attempt_key(),
                account_id: account.cloned(),
            })?;
        }
        if thread_changed || retry {
            if let Some(session) = session {
                write_provider_session(&self.dir, session, account.cloned())?;
                self.record_fact(CaptureFact::ProviderSessionObserved {
                    attempt_key: self.attempt_key(),
                    agent_session: session.clone(),
                })?;
            }
        }
        self.provider_observation_pending = false;
        Ok(())
    }

    pub(crate) fn start_attempt(
        &mut self,
        provider: &str,
        model: Option<&str>,
        account_id: Option<&crate::store::ProviderAccountId>,
    ) -> std::io::Result<()> {
        if self.attempt_started {
            return Ok(());
        }
        self.attempt_started = true;
        self.record_fact(CaptureFact::ProviderAttemptStarted {
            provider: provider.to_owned(),
            model: model.map(str::to_owned),
            account_id: account_id.cloned(),
            attempt_key: self.attempt_key(),
        })
    }

    fn finish_attempt(&mut self, outcome: &str) -> std::io::Result<()> {
        if !self.attempt_started {
            return Ok(());
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
        account_id: Option<&crate::store::ProviderAccountId>,
    ) -> std::io::Result<()> {
        let finish_error = self.finish_attempt("failed").err();
        self.provider_observation_pending = false;
        self.attempt += 1;
        self.attempt_started = false;
        let start_error = self.start_attempt(provider, model, account_id).err();
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

    /// Keep how a turn ended; any other event only ticks the silence clock.
    pub(crate) fn record_conversation(&mut self, event: ConversationEvent) -> std::io::Result<()> {
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

    pub(crate) fn finish(&mut self, outcome: &str) -> std::io::Result<()> {
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
        // The receipt file records settlement. A missed store observation
        // must not fail the exit.
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
pub(crate) fn row_store(dir: &Path) -> StoreResult<crate::store::sqlite::SqliteStore> {
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

pub(crate) fn record_error(error: std::io::Error) -> StoreError {
    StoreError::InvalidData(error.to_string())
}
