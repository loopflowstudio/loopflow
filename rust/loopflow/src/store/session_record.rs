use loopflow_model::chat::types::TurnUsage;
use loopflow_model::id::AgentSessionId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use time::OffsetDateTime;
use uuid::Uuid;

/// An opaque artifact directory key; it grants no conversation or Graph authority.
pub(crate) fn new_artifact_key() -> String {
    Uuid::new_v4().simple().to_string()
}

/// Historical run-prefixed paths retain their spelling as imported evidence.
pub(crate) fn parse_artifact_key(
    value: &str,
) -> Result<String, loopflow_model::durable::DurableDataError> {
    Uuid::parse_str(value.strip_prefix("run_").unwrap_or(value))
        .map_err(|error| loopflow_model::durable::DurableDataError::InvalidId(error.to_string()))?;
    Ok(value.to_owned())
}

pub const CAPTURE_KEY_ENV: &str = "LF_CAPTURE_KEY";
pub(crate) const PROVIDER_ACCOUNT_ID_ENV: &str = "LF_PROVIDER_ACCOUNT_ID";
pub(crate) const SCHEMA_VERSION: u32 = 1;

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
pub(crate) struct FactEnvelope {
    schema_version: u32,
    pub(crate) seq: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub(crate) observed_at: OffsetDateTime,
    #[serde(flatten)]
    pub(crate) fact: CaptureFact,
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
pub(crate) fn read_fact(evidence: &serde_json::Value) -> Option<FactEnvelope> {
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
    pub session_id: loopflow_model::id::LfSessionId,
    pub captured: Option<i64>,
    pub artifact_key: Option<String>,
    pub caller_artifact_key: Option<String>,
    pub task_pr_id: Option<crate::store::work::task::TaskPrId>,
    pub repo: Option<String>,
    pub worktree: Option<String>,
    pub task_id: Option<loopflow_model::durable::TaskId>,
    pub wave_id: Option<loopflow_model::id::WaveId>,
    pub task_identifier: Option<String>,
    pub work_source: Option<crate::store::session::WorkSource>,
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
pub(crate) struct UsageReading {
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
pub(crate) fn native_usage(
    history: &[crate::store::session::SessionEvent],
) -> (Vec<UsageReading>, usize) {
    use crate::store::session::SessionEventKind;

    let mut turns =
        BTreeMap::<(&AgentSessionId, &str), Vec<&crate::store::session::SessionEvent>>::new();
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

pub(crate) fn providers_first_start(
    history: &[crate::store::session::SessionEvent],
) -> Option<i64> {
    history
        .iter()
        .filter(|event| event.kind == crate::store::session::SessionEventKind::Started)
        .map(|event| event.observed_at)
        .min()
}

pub(crate) fn project_provider_history(
    captured: Option<i64>,
    input: Option<&str>,
    facts: &[FactEnvelope],
    readings: &[UsageReading],
    history: &[crate::store::session::SessionEvent],
) -> std::io::Result<Vec<ProviderHistory>> {
    use crate::store::session::SessionEventKind;
    let mut native =
        BTreeMap::<(&AgentSessionId, &str), Vec<&crate::store::session::SessionEvent>>::new();
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
                .map(loopflow_model::id::LfProcessId::parse)
                .transpose()
                .map_err(std::io::Error::other)?,
            task_id: start
                .and_then(|event| event.task_id.as_deref())
                .map(loopflow_model::durable::TaskId::parse)
                .transpose()
                .map_err(std::io::Error::other)?,
            wave_id: start
                .and_then(|event| event.wave_id.as_deref())
                .map(loopflow_model::id::WaveId::parse)
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
            usage: reduce_usage(std::iter::empty()),
        });
    }
    records.sort_by_key(|record| record.started_at.or(record.completed_at));
    Ok(records)
}

/// Project typed input and provider history from the existing Session owner.
/// No current assignment, provider or completion can rewrite an earlier input.
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

pub(crate) fn provider_session_from_events(
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

/// The resume reference: the newest one names the native conversation and
/// account `lf resume` reopens.
pub(crate) fn provider_session_observation(
    session: &crate::store::session::LfSession,
    input: &str,
    agent_session: &AgentSessionId,
    account_id: Option<crate::store::ProviderAccountId>,
) -> crate::store::session::SessionObservation {
    let source = format!("provider-session:{}", Uuid::new_v4());
    crate::store::session::SessionObservation {
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

/// Reduce usage readings once per counter: each counter's largest values,
/// summed across counters.
pub(crate) fn reduce_usage<'a>(
    readings: impl IntoIterator<Item = &'a UsageReading>,
) -> SessionUsage {
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
