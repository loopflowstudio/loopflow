use loopflow_model::chat::types::TurnUsage;
use loopflow_model::id::AgentSessionId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use time::OffsetDateTime;
use uuid::Uuid;

/// An opaque artifact directory key; it grants no conversation or Graph authority.
pub fn new_artifact_key() -> String {
    Uuid::new_v4().simple().to_string()
}

/// Historical run-prefixed paths retain their spelling as imported evidence.
pub fn parse_artifact_key(
    value: &str,
) -> Result<String, loopflow_model::durable::DurableDataError> {
    Uuid::parse_str(value.strip_prefix("run_").unwrap_or(value))
        .map_err(|error| loopflow_model::durable::DurableDataError::InvalidId(error.to_string()))?;
    Ok(value.to_owned())
}

pub const CAPTURE_KEY_ENV: &str = "LF_CAPTURE_KEY";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderSessionRef {
    schema_version: u32,
    // Capture files retain their original encoding.
    #[serde(rename = "provider_session_id")]
    pub agent_session: AgentSessionId,
    pub account_id: Option<crate::ProviderAccountId>,
}

impl ProviderSessionRef {
    pub fn new(
        agent_session: AgentSessionId,
        account_id: Option<crate::ProviderAccountId>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            agent_session,
            account_id,
        }
    }

    pub fn validate(&self) -> std::io::Result<()> {
        if self.schema_version != SCHEMA_VERSION || self.agent_session.as_str().is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid provider session reference",
            ));
        }
        Ok(())
    }
}

/// What SQLite history keeps of a capture: its provider attempts, how each
/// turn ended, and each silence longer than
/// [`SILENCE_SECONDS`]. Provider text and usage have no variant: the provider's
/// transcript holds the conversation and native rows hold usage. Facts keep the
/// `events.jsonl:{seq}` source their receipt keys and migrations already index.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CaptureFact {
    ProviderAttemptStarted {
        provider: String,
        model: Option<String>,
        account_id: Option<crate::ProviderAccountId>,
        attempt_key: String,
    },
    ProviderAttemptFinished {
        attempt_key: String,
        outcome: String,
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
pub enum TurnEnd {
    TurnCompleted {
        turn_id: String,
        status: loopflow_model::chat::types::Lifecycle,
    },
    Error {
        code: String,
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        evidence: Option<Box<loopflow_model::chat::types::FailureEvidence>>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FactEnvelope {
    schema_version: u32,
    pub seq: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    #[serde(flatten)]
    pub fact: CaptureFact,
}

impl CaptureFact {
    /// The history observation of this fact at stream position `seq`, as
    /// `(source, evidence)`.
    pub fn observation(&self, seq: u64, at: OffsetDateTime) -> (String, serde_json::Value) {
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
pub fn read_fact(evidence: &serde_json::Value) -> Option<FactEnvelope> {
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

/// The Machine a collected Session was read from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct SessionOrigin {
    pub machine_id: loopflow_model::durable::MachineId,
    #[serde(deserialize_with = "Option::deserialize")]
    pub label: Option<String>,
}

impl SessionOrigin {
    pub fn name(&self) -> &str {
        self.label.as_deref().unwrap_or(self.machine_id.as_str())
    }
}

/// History beneath one conversation's immutable captured input. This is not a
/// resumable object. Provider records retain separate outcomes and driving Processes;
/// recorder completion is historical evidence, never native success or process exit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct SessionHistory {
    pub session_id: loopflow_model::id::LfSessionId,
    pub captured: Option<i64>,
    pub artifact_key: Option<String>,
    pub caller_artifact_key: Option<String>,
    pub task_pr_id: Option<crate::work::task::TaskPrId>,
    pub repo: Option<String>,
    pub worktree: Option<String>,
    pub task_id: Option<loopflow_model::durable::TaskId>,
    pub wave_id: Option<loopflow_model::id::WaveId>,
    pub task_identifier: Option<String>,
    pub work_source: Option<crate::session::WorkSource>,
    pub wave_name: Option<String>,
    pub skill: Option<String>,
    /// SHA-256 of the resolved skill text the run launched with; `null` when unrecorded.
    pub skill_version: Option<String>,
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
    pub lf_process_id: Option<loopflow_model::id::LfProcessId>,
    pub task_id: Option<loopflow_model::durable::TaskId>,
    pub wave_id: Option<loopflow_model::id::WaveId>,
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
pub const SILENCE_SECONDS: i64 = 600;

/// One turn's usage from a provider's reading of it, observed at `at`. A
/// `usage` object is already the turn's; Codex's `total`/`last` counts the
/// whole thread, so the turn's share is `total` less the thread's count when
/// the turn began (its `baseline`, else `prior`, else `total` less `last`); an
/// OpenCode `message.tokens` is one model call. `None` for a shape it cannot
/// read.
pub fn resolve_usage(
    reading: &serde_json::Value,
    prior: Option<&serde_json::Value>,
    at: i64,
) -> Option<crate::session::RecordedUsage> {
    let recorded = |usage, final_reading, partial| crate::session::RecordedUsage {
        observed_at: at,
        final_reading,
        partial,
        usage,
    };
    if reading["usage"].is_object() {
        let usage = serde_json::from_value(reading["usage"].clone()).ok()?;
        return Some(recorded(usage, reading["final"] == true, false));
    }
    if reading["message"]["tokens"].is_object() {
        let info = &reading["message"];
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
        return Some(recorded(usage, true, false));
    }
    reading.get("total")?;
    let count = |value: &serde_json::Value, part: &str, key: &str| value[part][key].as_u64();
    let first_request = |key: &str| {
        count(reading, "total", key)?.checked_sub(count(reading, "last", key).unwrap_or(0))
    };
    let baseline = |key: &str| {
        if reading["baseline"].is_object() {
            return count(reading, "baseline", key);
        }
        prior
            .and_then(|prior| prior[key].as_u64())
            .or_else(|| first_request(key))
    };
    // Without the turn's start, `last` is one request: the turn's share is
    // known only when that request was the thread's first.
    let partial = reading["partial"] == true
        || reading["regressed"] == true
        || (!reading["baseline"].is_object()
            && prior.is_none()
            && [
                "inputTokens",
                "outputTokens",
                "cachedInputTokens",
                "reasoningOutputTokens",
            ]
            .iter()
            .any(|key| first_request(key).is_some_and(|before| before > 0)));
    let delta = |key: &str| count(reading, "total", key)?.checked_sub(baseline(key)?);
    let gross = delta("inputTokens");
    let cached = delta("cachedInputTokens");
    let usage = TurnUsage {
        input_tokens: gross
            .zip(cached)
            .and_then(|(gross, cached)| gross.checked_sub(cached)),
        output_tokens: delta("outputTokens"),
        total_input_tokens: gross,
        peak_input_tokens: reading["peakInputTokens"].as_u64().or(count(
            reading,
            "last",
            "inputTokens",
        )),
        context_window_tokens: reading["modelContextWindow"].as_u64(),
        reasoning_tokens: delta("reasoningOutputTokens"),
        cache_read_tokens: cached,
        cache_write_tokens: delta("cacheWriteInputTokens"),
        model: None,
        cost_usd: None,
    };
    Some(recorded(usage, false, partial))
}

/// When the first of `turns` started.
pub fn providers_first_start(turns: &[crate::session::Turn]) -> Option<i64> {
    turns.iter().filter_map(|turn| turn.started_at).min()
}

pub fn project_provider_history(
    captured: Option<i64>,
    input: Option<&str>,
    facts: &[FactEnvelope],
    turns: &[crate::session::Turn],
    history: &[crate::session::SessionEvent],
) -> std::io::Result<Vec<ProviderHistory>> {
    use crate::session::SessionEventKind;
    let mut records = Vec::new();
    for turn in turns {
        records.push(ProviderHistory {
            reference: ProviderHistoryReference::NativeTurn {
                thread: turn.thread.clone(),
                turn: turn.turn.clone(),
                start_seq: turn.started_at.map(|_| turn.id),
                completion_seq: turn.ended_at.map(|_| turn.id),
            },
            lf_process_id: turn
                .lf_process_id
                .as_deref()
                .map(loopflow_model::id::LfProcessId::parse)
                .transpose()
                .map_err(std::io::Error::other)?,
            task_id: turn
                .task_id
                .as_deref()
                .map(loopflow_model::durable::TaskId::parse)
                .transpose()
                .map_err(std::io::Error::other)?,
            wave_id: turn
                .wave_id
                .as_deref()
                .map(loopflow_model::id::WaveId::parse)
                .transpose()
                .map_err(std::io::Error::other)?,
            started_at: turn.started_at,
            completed_at: turn.ended_at,
            outcome: turn.status.clone(),
            usage: reduce_usage(std::slice::from_ref(turn)),
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
                .map(loopflow_model::durable::TaskId::parse)
                .transpose()
                .map_err(std::io::Error::other)?,
            wave_id: origin
                .and_then(|event| event.wave_id.as_deref())
                .map(loopflow_model::id::WaveId::parse)
                .transpose()
                .map_err(std::io::Error::other)?,
            started_at: start.map(|event| event.observed_at.unix_timestamp()),
            completed_at: finished.as_ref().map(|(at, _)| *at),
            outcome: finished.map(|(_, outcome)| outcome),
            usage: reduce_usage(&[]),
        });
    }
    records.sort_by_key(|record| record.started_at.or(record.completed_at));
    Ok(records)
}

/// The resume reference: the newest one names the native conversation and
/// account `lf resume` reopens.
pub fn provider_session_observation(
    session: &crate::session::LfSession,
    input: &str,
    agent_session: &AgentSessionId,
    account_id: Option<crate::ProviderAccountId>,
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

/// Sum the usage `turns` kept. A turn whose start is unknown, or whose
/// usage was counted part way, is a gap.
pub fn reduce_usage(turns: &[crate::session::Turn]) -> SessionUsage {
    let readings: Vec<_> = turns
        .iter()
        .filter_map(|turn| Some((turn, turn.usage.as_ref()?)))
        .collect();
    let mut gaps = readings
        .iter()
        .filter(|(turn, usage)| turn.started_at.is_none() || usage.partial)
        .count();
    let values = |field: fn(&TurnUsage) -> Option<u64>| {
        readings
            .iter()
            .map(move |(_, usage)| field(&usage.usage))
            .collect::<Vec<_>>()
    };
    let sum = |field: fn(&TurnUsage) -> Option<u64>, gaps: &mut usize| {
        sum_u64(values(field).into_iter(), gaps)
    };
    let max = |field: fn(&TurnUsage) -> Option<u64>, gaps: &mut usize| {
        max_u64(values(field).into_iter(), gaps)
    };
    SessionUsage {
        streams: readings.len(),
        final_streams: readings
            .iter()
            .filter(|(_, usage)| usage.final_reading)
            .count(),
        input_tokens: sum(|usage| usage.input_tokens, &mut gaps),
        output_tokens: sum(|usage| usage.output_tokens, &mut gaps),
        total_input_tokens: sum(|usage| usage.total_input_tokens, &mut gaps),
        peak_input_tokens: max(|usage| usage.peak_input_tokens, &mut gaps),
        context_window_tokens: max(|usage| usage.context_window_tokens, &mut gaps),
        reasoning_tokens: sum(|usage| usage.reasoning_tokens, &mut gaps),
        cache_read_tokens: sum(|usage| usage.cache_read_tokens, &mut gaps),
        cache_write_tokens: sum(|usage| usage.cache_write_tokens, &mut gaps),
        cost_usd: sum_f64(readings.iter().map(|(_, usage)| usage.usage.cost_usd)),
        gaps,
    }
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
