//! Durable, local capture of provider-facing prompts and observable agent events.

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::chat::types::{ConversationEvent, TurnUsage};
use crate::engine::prompt::{account_prompt_tokens, count_tokens};
use crate::engine::stream::{ResultSubtype, StreamEvent};
use crate::id::{ExecId, TraceId};
use crate::store::{StoreError, StoreResult};

pub use crate::durable::{LaunchId, TurnId};

pub const TRACE_SCHEMA_VERSION: u32 = 1;
pub const TOKENIZER: &str = "cl100k_base";

#[derive(Debug, Clone)]
pub struct TraceCaptureContext {
    pub run_id: TraceId,
    pub process_id: ExecId,
    pub repo: PathBuf,
    pub worktree: PathBuf,
    pub wave: Option<String>,
    pub project: Option<String>,
    pub task: Option<String>,
    pub flow: Option<String>,
    pub skill: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ContextChannel {
    System,
    Task,
}

impl ContextChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Task => "task",
        }
    }

    pub fn parse(value: &str) -> StoreResult<Self> {
        match value {
            "system" => Ok(Self::System),
            "task" => Ok(Self::Task),
            _ => Err(StoreError::InvalidData(format!(
                "invalid context channel: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ContextCoverage {
    Assembled,
    ProviderTotalOnly,
    Unknown,
}

impl ContextCoverage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Assembled => "assembled",
            Self::ProviderTotalOnly => "provider_total_only",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ContextAssetKind {
    OperatingInstructions,
    SurfaceInstructions,
    ProviderInstructions,
    RepoInstructions,
    SkillInstructions,
    Direction,
    Goal,
    Memory,
    Chat,
    Summary,
    Document,
    Scratch,
    Diff,
    Clipboard,
    UserMessage,
    Assembly,
}

impl ContextAssetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OperatingInstructions => "operating_instructions",
            Self::SurfaceInstructions => "surface_instructions",
            Self::ProviderInstructions => "provider_instructions",
            Self::RepoInstructions => "repo_instructions",
            Self::SkillInstructions => "skill_instructions",
            Self::Direction => "direction",
            Self::Goal => "goal",
            Self::Memory => "memory",
            Self::Chat => "chat",
            Self::Summary => "summary",
            Self::Document => "document",
            Self::Scratch => "scratch",
            Self::Diff => "diff",
            Self::Clipboard => "clipboard",
            Self::UserMessage => "user_message",
            Self::Assembly => "assembly",
        }
    }

    pub fn parse(value: &str) -> StoreResult<Self> {
        match value {
            "operating_instructions" => Ok(Self::OperatingInstructions),
            "surface_instructions" => Ok(Self::SurfaceInstructions),
            "provider_instructions" => Ok(Self::ProviderInstructions),
            "repo_instructions" => Ok(Self::RepoInstructions),
            "skill_instructions" => Ok(Self::SkillInstructions),
            "direction" => Ok(Self::Direction),
            "goal" => Ok(Self::Goal),
            "memory" => Ok(Self::Memory),
            "chat" => Ok(Self::Chat),
            "summary" => Ok(Self::Summary),
            "document" => Ok(Self::Document),
            "scratch" => Ok(Self::Scratch),
            "diff" => Ok(Self::Diff),
            "clipboard" => Ok(Self::Clipboard),
            "user_message" => Ok(Self::UserMessage),
            "assembly" => Ok(Self::Assembly),
            _ => Err(StoreError::InvalidData(format!(
                "invalid context asset kind: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ContextScope {
    Global,
    Provider,
    Repo,
    Wave,
    Project,
    Task,
    Step,
    User,
}

impl ContextScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Provider => "provider",
            Self::Repo => "repo",
            Self::Wave => "wave",
            Self::Project => "project",
            Self::Task => "task",
            Self::Step => "step",
            Self::User => "user",
        }
    }

    pub fn parse(value: &str) -> StoreResult<Self> {
        match value {
            "global" => Ok(Self::Global),
            "provider" => Ok(Self::Provider),
            "repo" => Ok(Self::Repo),
            "wave" => Ok(Self::Wave),
            "project" => Ok(Self::Project),
            "task" => Ok(Self::Task),
            "step" => Ok(Self::Step),
            "user" => Ok(Self::User),
            _ => Err(StoreError::InvalidData(format!(
                "invalid context scope: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextAsset {
    pub position: u32,
    pub channel: ContextChannel,
    pub kind: ContextAssetKind,
    pub scope: ContextScope,
    pub label: String,
    pub source_path: Option<String>,
    pub included_by: String,
    pub content_sha256: String,
    pub byte_start: u64,
    pub byte_end: u64,
    pub bytes: u64,
    pub isolated_tokens: u64,
    pub attributed_tokens: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ContextDecisionKind {
    Included,
    Excluded,
    Summarized,
    StatOnly,
    Truncated,
    Deduplicated,
}

impl ContextDecisionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Included => "included",
            Self::Excluded => "excluded",
            Self::Summarized => "summarized",
            Self::StatOnly => "stat_only",
            Self::Truncated => "truncated",
            Self::Deduplicated => "deduplicated",
        }
    }

    pub fn parse(value: &str) -> StoreResult<Self> {
        match value {
            "included" => Ok(Self::Included),
            "excluded" => Ok(Self::Excluded),
            "summarized" => Ok(Self::Summarized),
            "stat_only" => Ok(Self::StatOnly),
            "truncated" => Ok(Self::Truncated),
            "deduplicated" => Ok(Self::Deduplicated),
            _ => Err(StoreError::InvalidData(format!(
                "invalid context decision: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextDecision {
    pub position: u32,
    pub kind: ContextAssetKind,
    pub scope: ContextScope,
    pub label: String,
    pub source_path: Option<String>,
    pub decision: ContextDecisionKind,
    pub reason: String,
    pub original_bytes: Option<u64>,
    pub original_tokens: Option<u64>,
    pub asset_position: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct RenderedPromptChannel {
    pub text: String,
    pub tokens: u64,
    pub assets: Vec<ContextAsset>,
}

#[derive(Debug, Clone)]
pub struct PreparedTurnContext {
    pub system: Option<RenderedPromptChannel>,
    pub task: RenderedPromptChannel,
    pub decisions: Vec<ContextDecision>,
    pub coverage: ContextCoverage,
    pub tokenizer: &'static str,
}

#[derive(Debug, Clone)]
pub struct ContextAssetSpec {
    pub channel: ContextChannel,
    pub kind: ContextAssetKind,
    pub scope: ContextScope,
    pub label: String,
    pub source_path: Option<String>,
    pub included_by: String,
    pub content: String,
    /// Attribute intentional duplicate renderings; false for speech and short vendor names.
    pub match_all_occurrences: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptSection {
    pub kind: ContextAssetKind,
    pub scope: ContextScope,
    pub label: String,
    pub source_path: Option<String>,
    pub included_by: String,
    pub text: String,
}

impl PreparedTurnContext {
    /// Capture the exact final provider strings. More specific attributed
    /// sections can replace these assembly assets without changing storage.
    pub fn from_prompts(system: &str, task: &str) -> Self {
        let mut position = 0;
        let system = (!system.is_empty()).then(|| {
            let channel = prompt_channel(
                system,
                ContextChannel::System,
                ContextScope::Global,
                "system prompt",
                position,
            );
            position += 1;
            channel
        });
        let task = prompt_channel(
            task,
            ContextChannel::Task,
            ContextScope::Task,
            "task prompt",
            position,
        );
        Self {
            system,
            task,
            decisions: Vec::new(),
            coverage: ContextCoverage::Assembled,
            tokenizer: TOKENIZER,
        }
    }

    /// Attribute semantic source slices inside the exact final provider
    /// strings. Unclaimed separators and wrappers become explicit assembly
    /// assets, so byte and token accounting still covers the whole prompt.
    pub fn from_attributed_prompts(
        system: &str,
        task: &str,
        specs: Vec<ContextAssetSpec>,
        decisions: Vec<ContextDecision>,
    ) -> Self {
        let mut position = 0;
        let system_specs = specs
            .iter()
            .filter(|spec| spec.channel == ContextChannel::System)
            .cloned()
            .collect::<Vec<_>>();
        let task_specs = specs
            .into_iter()
            .filter(|spec| spec.channel == ContextChannel::Task)
            .collect::<Vec<_>>();
        let system = (!system.is_empty()).then(|| {
            render_attributed_channel(system, ContextChannel::System, system_specs, &mut position)
        });
        let task = render_attributed_channel(task, ContextChannel::Task, task_specs, &mut position);
        let mut decisions = decisions;
        let first_decision_position = decisions
            .iter()
            .map(|decision| decision.position)
            .max()
            .map_or(0, |position| position + 1);
        for (offset, asset) in system
            .iter()
            .flat_map(|channel| channel.assets.iter())
            .chain(task.assets.iter())
            .filter(|asset| asset.kind != ContextAssetKind::Assembly)
            .enumerate()
        {
            decisions.push(ContextDecision {
                position: first_decision_position + offset as u32,
                kind: asset.kind,
                scope: asset.scope,
                label: asset.label.clone(),
                source_path: asset.source_path.clone(),
                decision: ContextDecisionKind::Included,
                reason: format!("included by {}", asset.included_by),
                original_bytes: Some(asset.bytes),
                original_tokens: Some(asset.isolated_tokens),
                asset_position: Some(asset.position),
            });
        }
        Self {
            system,
            task,
            decisions,
            coverage: ContextCoverage::Assembled,
            tokenizer: TOKENIZER,
        }
    }

    pub fn provider_total_only(task: &str) -> Self {
        let mut task = prompt_channel(
            task,
            ContextChannel::Task,
            ContextScope::User,
            "user message",
            0,
        );
        task.assets[0].kind = ContextAssetKind::UserMessage;
        task.assets[0].included_by = "provider_session_input".to_string();
        let asset = &task.assets[0];
        Self {
            system: None,
            decisions: vec![ContextDecision {
                position: 0,
                kind: asset.kind,
                scope: asset.scope,
                label: asset.label.clone(),
                source_path: None,
                decision: ContextDecisionKind::Included,
                reason: "sent as a follow-up provider-session input".to_string(),
                original_bytes: Some(asset.bytes),
                original_tokens: Some(asset.isolated_tokens),
                asset_position: Some(asset.position),
            }],
            task,
            coverage: ContextCoverage::ProviderTotalOnly,
            tokenizer: TOKENIZER,
        }
    }

    pub fn total_tokens(&self) -> u64 {
        self.system.as_ref().map_or(0, |channel| channel.tokens) + self.task.tokens
    }

    pub fn assets(&self) -> impl Iterator<Item = &ContextAsset> {
        self.system
            .iter()
            .flat_map(|channel| channel.assets.iter())
            .chain(self.task.assets.iter())
    }
}

fn render_attributed_channel(
    text: &str,
    channel: ContextChannel,
    specs: Vec<ContextAssetSpec>,
    position: &mut u32,
) -> RenderedPromptChannel {
    let mut claimed: Vec<(usize, usize, ContextAssetSpec)> = Vec::new();
    for spec in specs.into_iter().filter(|spec| !spec.content.is_empty()) {
        let mut offset = 0;
        while let Some(relative) = text[offset..].find(&spec.content) {
            let start = offset + relative;
            let end = start + spec.content.len();
            let mut blockers = claimed
                .iter()
                .filter_map(|(other_start, other_end, _)| {
                    let overlap_start = start.max(*other_start);
                    let overlap_end = end.min(*other_end);
                    (overlap_start < overlap_end).then_some((overlap_start, overlap_end))
                })
                .collect::<Vec<_>>();
            blockers.sort_unstable();

            // Specs are ordered from specific sources to enclosing messages.
            // Preserve earlier ownership and give the enclosing source its gaps.
            let mut cursor = start;
            for (blocker_start, blocker_end) in blockers {
                if blocker_start > cursor {
                    let mut fragment = spec.clone();
                    fragment.content = text[cursor..blocker_start].to_string();
                    claimed.push((cursor, blocker_start, fragment));
                }
                cursor = cursor.max(blocker_end);
            }
            if cursor < end {
                let mut fragment = spec.clone();
                fragment.content = text[cursor..end].to_string();
                claimed.push((cursor, end, fragment));
            }

            if !spec.match_all_occurrences {
                break;
            }
            offset = end;
            if offset >= text.len() {
                break;
            }
        }
    }
    claimed.sort_by_key(|(start, _, _)| *start);

    let assembly_spec = |content: &str| ContextAssetSpec {
        channel,
        kind: ContextAssetKind::Assembly,
        scope: if channel == ContextChannel::System {
            ContextScope::Global
        } else {
            ContextScope::Task
        },
        label: "prompt assembly".to_string(),
        source_path: None,
        included_by: "provider_invocation".to_string(),
        content: content.to_string(),
        match_all_occurrences: false,
    };

    let mut segments = Vec::new();
    let mut cursor = 0;
    for (start, end, spec) in claimed {
        if start > cursor {
            segments.push((cursor, start, assembly_spec(&text[cursor..start])));
        }
        segments.push((start, end, spec));
        cursor = end;
    }
    if cursor < text.len() || segments.is_empty() {
        segments.push((cursor, text.len(), assembly_spec(&text[cursor..])));
    }

    let prefix_ends = segments
        .iter()
        .take(segments.len().saturating_sub(1))
        .map(|(_, end, _)| *end)
        .collect::<Vec<_>>();
    let ranges = segments
        .iter()
        .map(|(start, end, _)| (*start, *end))
        .collect::<Vec<_>>();
    let accounting = account_prompt_tokens(text, &prefix_ends, &ranges);
    let total_tokens = accounting
        .as_ref()
        .map_or_else(|| token_count(text), |accounting| accounting.total as u64);
    let prefix_tokens = accounting.as_ref().map_or_else(
        || {
            prefix_ends
                .iter()
                .map(|end| token_count(&text[..*end]))
                .collect::<Vec<_>>()
        },
        |accounting| {
            accounting
                .prefixes
                .iter()
                .map(|count| *count as u64)
                .collect()
        },
    );
    let isolated_tokens = accounting.map(|accounting| accounting.isolated);
    let mut previous_tokens = 0_u64;
    let segment_count = segments.len();
    let mut assets = Vec::with_capacity(segment_count);
    for (index, (start, end, spec)) in segments.into_iter().enumerate() {
        let prefix_tokens = if index + 1 == segment_count {
            total_tokens
        } else {
            prefix_tokens[index]
        };
        let attributed_tokens = prefix_tokens.saturating_sub(previous_tokens);
        previous_tokens = prefix_tokens;
        let slice = &text[start..end];
        assets.push(ContextAsset {
            position: *position,
            channel,
            kind: spec.kind,
            scope: spec.scope,
            label: spec.label,
            source_path: spec.source_path,
            included_by: spec.included_by,
            content_sha256: hex::encode(Sha256::digest(slice.as_bytes())),
            byte_start: start as u64,
            byte_end: end as u64,
            bytes: (end - start) as u64,
            isolated_tokens: isolated_tokens
                .as_ref()
                .map_or_else(|| token_count(slice), |counts| counts[index] as u64),
            attributed_tokens,
        });
        *position += 1;
    }
    RenderedPromptChannel {
        text: text.to_string(),
        tokens: total_tokens,
        assets,
    }
}

fn prompt_channel(
    text: &str,
    channel: ContextChannel,
    scope: ContextScope,
    label: &str,
    position: u32,
) -> RenderedPromptChannel {
    let tokens = token_count(text);
    let bytes = text.len() as u64;
    let asset = ContextAsset {
        position,
        channel,
        kind: ContextAssetKind::Assembly,
        scope,
        label: label.to_string(),
        source_path: None,
        included_by: "provider_invocation".to_string(),
        content_sha256: hex::encode(Sha256::digest(text.as_bytes())),
        byte_start: 0,
        byte_end: bytes,
        bytes,
        isolated_tokens: tokens,
        attributed_tokens: tokens,
    };
    RenderedPromptChannel {
        text: text.to_string(),
        tokens,
        assets: vec![asset],
    }
}

fn token_count(text: &str) -> u64 {
    if text.is_empty() {
        0
    } else {
        count_tokens(text) as u64
    }
}

#[derive(Debug, Clone)]
pub struct ProviderInvocation {
    pub context: PreparedTurnContext,
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedConversationEvent {
    pub schema_version: u32,
    pub seq: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub ts: OffsetDateTime,
    pub turn_id: Option<TurnId>,
    pub payload: RecordedConversationPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawProviderRecord {
    pub schema_version: u32,
    pub seq: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub ts: OffsetDateTime,
    pub stream: String,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum RecordedConversationPayload {
    UserInput {
        op: String,
        text: String,
    },
    Conversation {
        event: ConversationEvent,
    },
    LegacyText {
        stream: String,
        text: String,
    },
    LegacyTool {
        name: String,
        summary: String,
    },
    Usage {
        usage: TurnUsage,
    },
    Result {
        status: String,
        cost_usd: Option<f64>,
        duration_secs: Option<f64>,
    },
    CaptureError {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentLaunchRow {
    pub id: String,
    pub run_id: String,
    pub process_id: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub repo: String,
    pub worktree: String,
    pub wave: Option<String>,
    pub flow: Option<String>,
    pub skill: Option<String>,
    pub project: Option<String>,
    pub task: Option<String>,
    pub provider: String,
    pub model: Option<String>,
    pub surface: String,
    pub capture_status: String,
    pub incomplete_reason: Option<String>,
    pub outcome: String,
    pub artifact_dir: String,
    pub conversation_path: String,
    pub provider_events_path: Option<String>,
    pub provider_session_id: Option<String>,
    pub provider_session_path: Option<String>,
    pub conversation_event_count: i64,
    pub conversation_bytes: i64,
    pub control: Option<ControlLaunch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlLaunch {
    pub run_id: crate::durable::RunId,
    pub home_id: crate::durable::HomeId,
    pub account_id: Option<String>,
    pub containment: crate::durable::Containment,
    pub resume_token: Option<String>,
    pub opaque_basis: Option<crate::durable::Basis>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentTurnRow {
    pub id: String,
    pub launch_id: String,
    pub ordinal: i64,
    pub provider_turn_id: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub status: String,
    pub input_op: String,
    pub context_coverage: String,
    pub tokenizer: String,
    pub system_prompt_path: Option<String>,
    pub task_prompt_path: String,
    pub system_tokens: i64,
    pub task_tokens: i64,
    pub supplied_context_tokens: i64,
    pub provider_input_tokens: Option<i64>,
    pub provider_total_input_tokens: Option<i64>,
    pub peak_input_tokens: Option<i64>,
    pub context_window_tokens: Option<i64>,
    pub provider_output_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub cost_usd: Option<f64>,
    pub context_gather_ms: i64,
    pub context_render_ms: i64,
    pub context_persist_ms: i64,
    pub first_event_seq: Option<i64>,
    pub last_event_seq: Option<i64>,
    pub root_output: Option<String>,
    pub basis: Option<crate::durable::Basis>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextAssetRow {
    pub turn_id: String,
    #[serde(flatten)]
    pub asset: ContextAsset,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextDecisionRow {
    pub turn_id: String,
    #[serde(flatten)]
    pub decision: ContextDecision,
}

#[derive(Debug, Clone)]
pub struct CaptureHandle(Arc<Mutex<TraceCapture>>);

#[derive(Debug, Clone)]
pub struct CaptureStart {
    pub provider: String,
    pub model: Option<String>,
    pub surface: String,
    pub input_op: String,
    pub gather_ms: u64,
    pub render_ms: u64,
    pub raw_provider: bool,
    pub basis: Option<crate::durable::Basis>,
    pub control: Option<ControlLaunch>,
}

impl CaptureHandle {
    pub fn launch_id(&self) -> crate::durable::LaunchId {
        let id = self
            .0
            .lock()
            .expect("trace capture mutex poisoned")
            .launch
            .id
            .clone();
        crate::durable::LaunchId::parse(&id)
            .expect("TraceCapture stores a generated durable Launch id")
    }

    pub fn artifact_dir(&self) -> PathBuf {
        let relative = self
            .0
            .lock()
            .expect("trace capture mutex poisoned")
            .launch
            .artifact_dir
            .clone();
        resolve_artifact(&relative).expect("capture stores a validated relative artifact path")
    }

    pub fn begin(
        context: TraceCaptureContext,
        prepared: PreparedTurnContext,
        start: CaptureStart,
    ) -> StoreResult<Self> {
        let capture = TraceCapture::begin(context, prepared, start)?;
        Ok(Self(Arc::new(Mutex::new(capture))))
    }

    pub fn record_raw(&self, stream: &str, line: &str) {
        self.with_capture(|capture| capture.record_raw(stream, line));
    }

    pub fn record_stream_event(&self, event: &StreamEvent) {
        self.with_capture(|capture| capture.record_stream_event(event));
    }

    pub fn record_conversation(&self, event: ConversationEvent) {
        self.with_capture(|capture| capture.record_conversation(event));
    }

    pub fn begin_turn(&self, input_op: &str, text: &str) -> StoreResult<()> {
        self.begin_turn_at(input_op, text, None)
    }

    pub fn begin_turn_at(
        &self,
        input_op: &str,
        text: &str,
        basis: Option<crate::durable::Basis>,
    ) -> StoreResult<()> {
        let mut capture = self.0.lock().expect("trace capture mutex poisoned");
        if let Some(message) = &capture.failed {
            return Err(StoreError::InvalidData(format!(
                "trace capture is already partial: {message}"
            )));
        }
        let result = capture.begin_turn(
            input_op,
            PreparedTurnContext::provider_total_only(text),
            basis,
        );
        if let Err(error) = &result {
            capture.failed = Some(error.to_string());
        }
        result
    }

    pub fn finish_turn(&self, status: &str) -> StoreResult<()> {
        if !matches!(status, "completed" | "failed" | "interrupted") {
            return Err(StoreError::InvalidData(format!(
                "invalid agent Turn outcome: {status}"
            )));
        }
        let mut capture = self.0.lock().expect("trace capture mutex poisoned");
        capture.finish_current_turn(status, OffsetDateTime::now_utc().unix_timestamp())
    }

    pub(crate) fn fail_and_begin_launch(
        &self,
        provider: String,
        model: Option<String>,
        text: &str,
    ) -> StoreResult<()> {
        let mut capture = self.0.lock().expect("trace capture mutex poisoned");
        if capture.launch.control.is_some() {
            return Err(StoreError::InvalidData(
                "a control Launch successor must be reserved before capture".to_string(),
            ));
        }
        let context = capture.context.clone();
        let start = CaptureStart {
            provider,
            model,
            surface: capture.launch.surface.clone(),
            input_op: "message".to_string(),
            gather_ms: 0,
            render_ms: 0,
            raw_provider: capture.provider_path.is_some(),
            basis: None,
            control: None,
        };
        capture.finish("failed", false)?;
        *capture = TraceCapture::begin(
            context,
            PreparedTurnContext::provider_total_only(text),
            start,
        )?;
        Ok(())
    }

    pub fn current_turn_id(&self) -> String {
        self.0
            .lock()
            .expect("trace capture mutex poisoned")
            .turn
            .id
            .clone()
    }

    pub fn set_provider_session_id(&self, session_id: Option<String>) {
        self.with_capture(|capture| {
            capture.launch.provider_session_id = session_id;
            crate::journal::open_ledger()?.update_agent_launch_receipt(&capture.launch)
        });
    }

    pub fn finish(&self, outcome: &str, prompt_only: bool) -> StoreResult<()> {
        self.0
            .lock()
            .expect("trace capture mutex poisoned")
            .finish(outcome, prompt_only)
    }

    fn with_capture(&self, operation: impl FnOnce(&mut TraceCapture) -> StoreResult<()>) {
        let mut capture = self.0.lock().expect("trace capture mutex poisoned");
        if capture.failed.is_some() {
            return;
        }
        if let Err(error) = operation(&mut capture) {
            let message = error.to_string();
            tracing::warn!(error = %message, launch_id = %capture.launch.id, "trace capture became partial");
            let _ = capture.append_payload(RecordedConversationPayload::CaptureError {
                message: message.clone(),
            });
            capture.failed = Some(message);
        }
    }
}

#[derive(Debug)]
struct TraceCapture {
    context: TraceCaptureContext,
    launch: AgentLaunchRow,
    turn: AgentTurnRow,
    conversation_path: PathBuf,
    provider_path: Option<PathBuf>,
    event_seq: u64,
    provider_seq: u64,
    usage: TurnUsage,
    usage_observed: bool,
    failed: Option<String>,
}

impl TraceCapture {
    fn begin(
        context: TraceCaptureContext,
        prepared: PreparedTurnContext,
        start: CaptureStart,
    ) -> StoreResult<Self> {
        validate_input_op(&start.input_op)?;
        let persist_start = Instant::now();
        let ledger = crate::journal::open_ledger()?;
        let registered_launch = start
            .control
            .as_ref()
            .map(|control| ledger.control_launch_for_run(&control.run_id))
            .transpose()?
            .flatten();
        let launch_id = registered_launch
            .as_ref()
            .map(|launch| launch.id.clone())
            .unwrap_or_default();
        let turn_id = TurnId::new();
        let root = trace_root();
        create_private_dir(&root)?;
        let process_dir = root
            .join(context.run_id.as_str())
            .join(context.process_id.as_str());
        create_private_dir(&process_dir)?;
        let artifact_dir = process_dir.join(launch_id.as_str());
        let staging_dir = process_dir.join(format!(".{}.staging", launch_id.as_str()));
        if staging_dir.exists() || artifact_dir.exists() {
            return Err(StoreError::InvalidData(format!(
                "trace artifact already exists for launch {launch_id}"
            )));
        }
        create_private_dir(&staging_dir)?;
        let turns_dir = staging_dir.join("turns");
        create_private_dir(&turns_dir)?;

        let system_path = prepared
            .system
            .as_ref()
            .map(|system| {
                let path = turns_dir.join("0001-system.md");
                write_private(&path, system.text.as_bytes()).map(|_| path)
            })
            .transpose()?;
        let task_path = turns_dir.join("0001-task.md");
        write_private(&task_path, prepared.task.text.as_bytes())?;
        let conversation_path = staging_dir.join("conversation.jsonl");
        let initial_event = RecordedConversationEvent {
            schema_version: TRACE_SCHEMA_VERSION,
            seq: 0,
            ts: OffsetDateTime::now_utc(),
            turn_id: Some(turn_id.clone()),
            payload: RecordedConversationPayload::UserInput {
                op: start.input_op.clone(),
                text: prepared.task.text.clone(),
            },
        };
        write_private(&conversation_path, b"")?;
        let initial_bytes = append_json_line(&conversation_path, &initial_event)?;
        let provider_path = start
            .raw_provider
            .then(|| staging_dir.join("provider.jsonl"));
        if let Some(path) = &provider_path {
            write_private(path, b"")?;
        }

        fs::rename(&staging_dir, &artifact_dir).map_err(|error| {
            StoreError::InvalidData(format!(
                "publish {} as {}: {error}",
                staging_dir.display(),
                artifact_dir.display()
            ))
        })?;
        let published_system_path = system_path
            .as_ref()
            .map(|path| artifact_dir.join(path.strip_prefix(&staging_dir).expect("staged path")));
        let published_task_path =
            artifact_dir.join(task_path.strip_prefix(&staging_dir).expect("staged path"));
        let published_conversation_path = artifact_dir.join("conversation.jsonl");
        let published_provider_path = provider_path
            .as_ref()
            .map(|path| artifact_dir.join(path.strip_prefix(&staging_dir).expect("staged path")));

        let started_at = registered_launch
            .as_ref()
            .map(|launch| launch.started_at.unix_timestamp())
            .unwrap_or_else(|| OffsetDateTime::now_utc().unix_timestamp());
        let system_tokens = prepared.system.as_ref().map_or(0, |channel| channel.tokens) as i64;
        let task_tokens = prepared.task.tokens as i64;
        let persist_ms = persist_start.elapsed().as_millis() as i64;
        let launch = AgentLaunchRow {
            id: launch_id.to_string(),
            run_id: context.run_id.to_string(),
            process_id: context.process_id.to_string(),
            started_at,
            ended_at: None,
            repo: context.repo.display().to_string(),
            worktree: context.worktree.display().to_string(),
            wave: context.wave.clone(),
            flow: context.flow.clone(),
            skill: context.skill.clone(),
            project: context.project.clone(),
            task: context.task.clone(),
            provider: start.provider,
            model: start.model,
            surface: start.surface,
            capture_status: "capturing".to_string(),
            incomplete_reason: None,
            outcome: "running".to_string(),
            artifact_dir: artifact_relative(&root, &artifact_dir)?,
            conversation_path: artifact_relative(&root, &published_conversation_path)?,
            provider_events_path: published_provider_path
                .as_ref()
                .map(|path| artifact_relative(&root, path))
                .transpose()?,
            provider_session_id: None,
            provider_session_path: None,
            conversation_event_count: 1,
            conversation_bytes: initial_bytes as i64,
            control: start.control,
        };
        let turn = AgentTurnRow {
            id: turn_id.to_string(),
            launch_id: launch.id.clone(),
            ordinal: 1,
            provider_turn_id: None,
            started_at,
            ended_at: None,
            status: "running".to_string(),
            input_op: start.input_op.clone(),
            context_coverage: prepared.coverage.as_str().to_string(),
            tokenizer: prepared.tokenizer.to_string(),
            system_prompt_path: published_system_path
                .as_ref()
                .map(|path| artifact_relative(&root, path))
                .transpose()?,
            task_prompt_path: artifact_relative(&root, &published_task_path)?,
            system_tokens,
            task_tokens,
            supplied_context_tokens: system_tokens + task_tokens,
            provider_input_tokens: None,
            provider_total_input_tokens: None,
            peak_input_tokens: None,
            context_window_tokens: None,
            provider_output_tokens: None,
            reasoning_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            cost_usd: None,
            context_gather_ms: start.gather_ms as i64,
            context_render_ms: start.render_ms as i64,
            context_persist_ms: persist_ms,
            first_event_seq: Some(0),
            last_event_seq: Some(0),
            root_output: None,
            basis: start.basis,
        };
        let assets = prepared
            .assets()
            .cloned()
            .map(|asset| ContextAssetRow {
                turn_id: turn_id.to_string(),
                asset,
            })
            .collect::<Vec<_>>();
        let decisions = prepared
            .decisions
            .into_iter()
            .map(|decision| ContextDecisionRow {
                turn_id: turn_id.to_string(),
                decision,
            })
            .collect::<Vec<_>>();
        if let Err(error) = ledger.insert_trace_capture(&launch, &turn, &assets, &decisions) {
            let _ = fs::remove_dir_all(&artifact_dir);
            return Err(error);
        }

        Ok(Self {
            context,
            launch,
            turn,
            conversation_path: published_conversation_path,
            provider_path: published_provider_path,
            event_seq: 1,
            provider_seq: 0,
            usage: TurnUsage::default(),
            usage_observed: false,
            failed: None,
        })
    }

    fn append_payload(&mut self, payload: RecordedConversationPayload) -> StoreResult<()> {
        let event = RecordedConversationEvent {
            schema_version: TRACE_SCHEMA_VERSION,
            seq: self.event_seq,
            ts: OffsetDateTime::now_utc(),
            turn_id: Some(
                TurnId::parse(&self.turn.id)
                    .expect("TraceCapture stores a generated durable Turn id"),
            ),
            payload,
        };
        let bytes = append_json_line(&self.conversation_path, &event)?;
        self.event_seq += 1;
        self.launch.conversation_event_count += 1;
        self.launch.conversation_bytes += bytes as i64;
        self.turn.last_event_seq = Some(event.seq as i64);
        Ok(())
    }

    fn begin_turn(
        &mut self,
        input_op: &str,
        prepared: PreparedTurnContext,
        basis: Option<crate::durable::Basis>,
    ) -> StoreResult<()> {
        validate_input_op(input_op)?;
        let now = OffsetDateTime::now_utc().unix_timestamp();
        self.finish_current_turn("partial", now)?;

        let persist_start = Instant::now();
        let ordinal = self.turn.ordinal + 1;
        let turn_id = TurnId::new();
        let provider_turn_id = self.turn.provider_turn_id.clone();
        let artifact_dir = resolve_artifact(&self.launch.artifact_dir)?;
        let turns_dir = artifact_dir.join("turns");
        let task_path = turns_dir.join(format!("{ordinal:04}-task.md"));
        let staging_path = turns_dir.join(format!(".{ordinal:04}-task.md.staging"));
        write_private(&staging_path, prepared.task.text.as_bytes())?;
        fs::rename(&staging_path, &task_path).map_err(|error| {
            StoreError::InvalidData(format!(
                "publish follow-up prompt {}: {error}",
                task_path.display()
            ))
        })?;

        let task_tokens = prepared.task.tokens as i64;
        let mut turn = AgentTurnRow {
            id: turn_id.to_string(),
            launch_id: self.launch.id.clone(),
            ordinal,
            provider_turn_id,
            started_at: now,
            ended_at: None,
            status: "running".to_string(),
            input_op: input_op.to_string(),
            context_coverage: prepared.coverage.as_str().to_string(),
            tokenizer: prepared.tokenizer.to_string(),
            system_prompt_path: None,
            task_prompt_path: artifact_relative(&trace_root(), &task_path)?,
            system_tokens: 0,
            task_tokens,
            supplied_context_tokens: task_tokens,
            provider_input_tokens: None,
            provider_total_input_tokens: None,
            peak_input_tokens: None,
            context_window_tokens: None,
            provider_output_tokens: None,
            reasoning_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            cost_usd: None,
            context_gather_ms: 0,
            context_render_ms: 0,
            context_persist_ms: persist_start.elapsed().as_millis() as i64,
            first_event_seq: Some(self.event_seq as i64),
            last_event_seq: None,
            root_output: None,
            basis,
        };
        let assets = prepared
            .assets()
            .cloned()
            .map(|asset| ContextAssetRow {
                turn_id: turn.id.clone(),
                asset,
            })
            .collect::<Vec<_>>();
        let decisions = prepared
            .decisions
            .into_iter()
            .map(|decision| ContextDecisionRow {
                turn_id: turn.id.clone(),
                decision,
            })
            .collect::<Vec<_>>();
        if let Err(error) =
            crate::journal::open_ledger()?.insert_agent_turn_capture(&turn, &assets, &decisions)
        {
            let _ = fs::remove_file(&task_path);
            return Err(error);
        }

        self.turn = turn.clone();
        self.usage = TurnUsage::default();
        self.usage_observed = false;
        self.append_payload(RecordedConversationPayload::UserInput {
            op: input_op.to_string(),
            text: prepared.task.text,
        })?;
        turn.last_event_seq = self.turn.last_event_seq;
        self.turn = turn;
        Ok(())
    }

    /// Copy the accumulated turn usage onto the turn row before persisting it.
    fn apply_usage_to_turn(&mut self) {
        if self.usage_observed {
            self.turn.provider_input_tokens = Some(self.usage.input_tokens as i64);
            self.turn.provider_total_input_tokens =
                self.usage.total_input_tokens.map(|value| value as i64);
            self.turn.peak_input_tokens = self.usage.peak_input_tokens.map(|value| value as i64);
            self.turn.context_window_tokens =
                self.usage.context_window_tokens.map(|value| value as i64);
            self.turn.provider_output_tokens = Some(self.usage.output_tokens as i64);
            self.turn.reasoning_tokens = self.usage.reasoning_tokens.map(|value| value as i64);
            self.turn.cache_read_tokens = self.usage.cache_read_tokens.map(|value| value as i64);
            self.turn.cache_write_tokens = self.usage.cache_write_tokens.map(|value| value as i64);
        }
        self.turn.cost_usd = self.usage.cost_usd;
    }

    fn finish_current_turn(&mut self, status: &str, ended_at: i64) -> StoreResult<()> {
        if self.turn.status != "running" {
            return Ok(());
        }
        self.turn.ended_at = Some(ended_at);
        self.turn.status = status.to_string();
        self.apply_usage_to_turn();
        crate::journal::open_ledger()?.finish_agent_turn_capture(&self.turn)
    }

    fn record_raw(&mut self, stream: &str, line: &str) -> StoreResult<()> {
        let Some(provider_path) = self.provider_path.as_ref() else {
            return Ok(());
        };
        let record = RawProviderRecord {
            schema_version: TRACE_SCHEMA_VERSION,
            seq: self.provider_seq,
            ts: OffsetDateTime::now_utc(),
            stream: stream.to_string(),
            line: line.to_string(),
        };
        append_json_line(provider_path, &record)?;
        self.provider_seq += 1;
        if self.launch.provider_session_id.is_none() {
            if let Some(session_id) = provider_session_id(line) {
                self.launch.provider_session_id = Some(session_id);
                crate::journal::open_ledger()?.update_agent_launch_receipt(&self.launch)?;
            }
        }
        Ok(())
    }

    fn record_stream_event(&mut self, event: &StreamEvent) -> StoreResult<()> {
        let payload = match event {
            StreamEvent::Text(text) => {
                self.record_root_output(text)?;
                RecordedConversationPayload::LegacyText {
                    stream: "assistant".to_string(),
                    text: text.clone(),
                }
            }
            StreamEvent::ToolUse { name, summary } => RecordedConversationPayload::LegacyTool {
                name: name.clone(),
                summary: summary.clone(),
            },
            StreamEvent::Usage {
                input_tokens,
                output_tokens,
                cache_read_tokens,
            } => {
                self.usage_observed = true;
                self.usage.input_tokens += input_tokens.unwrap_or(0);
                self.usage.output_tokens += output_tokens.unwrap_or(0);
                self.usage.cache_read_tokens = Some(
                    self.usage.cache_read_tokens.unwrap_or(0) + cache_read_tokens.unwrap_or(0),
                );
                self.usage.total_input_tokens =
                    Some(self.usage.input_tokens + self.usage.cache_read_tokens.unwrap_or(0));
                RecordedConversationPayload::Usage {
                    usage: self.usage.clone(),
                }
            }
            StreamEvent::Result {
                subtype,
                cost_usd,
                duration_secs,
            } => {
                self.usage.cost_usd = *cost_usd;
                RecordedConversationPayload::Result {
                    status: match subtype {
                        ResultSubtype::Success => "completed",
                        ResultSubtype::Error => "failed",
                    }
                    .to_string(),
                    cost_usd: *cost_usd,
                    duration_secs: *duration_secs,
                }
            }
        };
        self.append_payload(payload)
    }

    fn record_conversation(&mut self, event: ConversationEvent) -> StoreResult<()> {
        match &event {
            ConversationEvent::TurnStarted { turn_id } => {
                self.turn.provider_turn_id = Some(turn_id.clone());
            }
            ConversationEvent::TurnUsage { usage, .. } => {
                self.usage_observed = true;
                self.usage = usage.clone();
            }
            ConversationEvent::TextDelta { content, .. } => {
                self.record_root_output(content)?;
            }
            _ => {}
        }
        self.append_payload(RecordedConversationPayload::Conversation { event })
    }

    fn record_root_output(&mut self, text: &str) -> StoreResult<()> {
        self.turn
            .root_output
            .get_or_insert_with(String::new)
            .push_str(text);
        crate::journal::open_ledger()?.finish_agent_turn_capture(&self.turn)
    }

    fn finish(&mut self, outcome: &str, prompt_only: bool) -> StoreResult<()> {
        if !matches!(outcome, "completed" | "failed" | "interrupted") {
            return Err(StoreError::InvalidData(format!(
                "invalid agent launch outcome: {outcome}"
            )));
        }
        let now = OffsetDateTime::now_utc().unix_timestamp();
        self.launch.ended_at = Some(now);
        self.launch.outcome = outcome.to_string();
        let sync_error = sync_file(&self.conversation_path)
            .and_then(|_| self.provider_path.as_deref().map_or(Ok(()), sync_file))
            .err();
        if let Some(error) = sync_error {
            self.failed.get_or_insert_with(|| error.to_string());
        }
        self.launch.capture_status = if self.failed.is_some() {
            "partial".to_string()
        } else if prompt_only {
            "prompt_only".to_string()
        } else {
            "complete".to_string()
        };
        self.launch.incomplete_reason = match (&self.failed, prompt_only) {
            (Some(error), _) => Some(error.clone()),
            (None, true) => Some("provider conversation not captured by Loopflow".to_string()),
            (None, false) => None,
        };
        self.turn.ended_at = Some(now);
        self.turn.status = if self.failed.is_some() {
            "partial".to_string()
        } else {
            match outcome {
                "completed" => "completed",
                "interrupted" => "interrupted",
                _ => "failed",
            }
            .to_string()
        };
        self.apply_usage_to_turn();
        crate::journal::open_ledger()?.finish_trace_capture(&self.launch, &self.turn)
    }
}

fn validate_input_op(value: &str) -> StoreResult<()> {
    if matches!(value, "initial" | "message" | "steer" | "queued") {
        return Ok(());
    }
    Err(StoreError::InvalidData(format!(
        "invalid input op: {value}"
    )))
}

pub fn trace_root() -> PathBuf {
    // Resolve through the same home logic as the store so capture-time and
    // doctor-time roots never diverge. lf_home_dir() honors LF_HOME, plus
    // LF_CONTROL_HOME and the dev/release provenance split; anchoring traces on
    // it keeps a reference resolvable wherever the store that names it lives.
    crate::store::lf_home_dir().join("traces")
}

fn artifact_relative(root: &Path, path: &Path) -> StoreResult<String> {
    let relative = path.strip_prefix(root).map_err(|_| {
        StoreError::InvalidData(format!(
            "trace artifact {} is outside {}",
            path.display(),
            root.display()
        ))
    })?;
    validate_artifact_path(relative)?;
    Ok(relative.display().to_string())
}

pub fn resolve_artifact(relative: &str) -> StoreResult<PathBuf> {
    let relative = Path::new(relative);
    validate_artifact_path(relative)?;
    Ok(trace_root().join(relative))
}

pub fn list_launch_artifact_dirs() -> StoreResult<Vec<String>> {
    let root = trace_root();
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut directories = Vec::new();
    for run in read_dirs(&root)? {
        if !run.is_dir() {
            continue;
        }
        for process in read_dirs(&run)? {
            if !process.is_dir() {
                continue;
            }
            for launch in read_dirs(&process)? {
                if launch.is_dir() {
                    directories.push(artifact_relative(&root, &launch)?);
                }
            }
        }
    }
    directories.sort();
    Ok(directories)
}

fn read_dirs(path: &Path) -> StoreResult<Vec<PathBuf>> {
    fs::read_dir(path)
        .map_err(|error| StoreError::InvalidData(format!("read {}: {error}", path.display())))?
        .map(|entry| {
            entry.map(|entry| entry.path()).map_err(|error| {
                StoreError::InvalidData(format!("read {}: {error}", path.display()))
            })
        })
        .collect()
}

fn validate_artifact_path(path: &Path) -> StoreResult<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(StoreError::InvalidData(format!(
            "unsafe trace artifact path: {}",
            path.display()
        )));
    }
    Ok(())
}

fn create_private_dir(path: &Path) -> StoreResult<()> {
    fs::create_dir_all(path)
        .map_err(|error| StoreError::InvalidData(format!("create {}: {error}", path.display())))?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| {
        StoreError::InvalidData(format!("set permissions on {}: {error}", path.display()))
    })?;
    Ok(())
}

fn write_private(path: &Path, bytes: &[u8]) -> StoreResult<()> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(path)
        .map_err(|error| StoreError::InvalidData(format!("open {}: {error}", path.display())))?;
    file.write_all(bytes)
        .map_err(|error| StoreError::InvalidData(format!("write {}: {error}", path.display())))?;
    file.sync_all()
        .map_err(|error| StoreError::InvalidData(format!("sync {}: {error}", path.display())))?;
    #[cfg(unix)]
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| {
            StoreError::InvalidData(format!("set permissions on {}: {error}", path.display()))
        })?;
    Ok(())
}

fn append_json_line<T: Serialize>(path: &Path, value: &T) -> StoreResult<usize> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(path)
        .map_err(|error| StoreError::InvalidData(format!("open {}: {error}", path.display())))?;
    file.write_all(&bytes)
        .map_err(|error| StoreError::InvalidData(format!("append {}: {error}", path.display())))?;
    file.sync_data()
        .map_err(|error| StoreError::InvalidData(format!("sync {}: {error}", path.display())))?;
    Ok(bytes.len())
}

fn provider_session_id(line: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let session_id = [
        value.get("session_id"),
        value.get("sessionId"),
        value.get("thread_id"),
        value.pointer("/stream_event/event/session_id"),
        value.pointer("/params/thread/id"),
    ]
    .into_iter()
    .flatten()
    .find_map(|value| value.as_str().filter(|value| !value.is_empty()))
    .map(str::to_string);
    session_id
}

fn sync_file(path: &Path) -> StoreResult<()> {
    let file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|error| StoreError::InvalidData(format!("open {}: {error}", path.display())))?;
    file.sync_data()
        .map_err(|error| StoreError::InvalidData(format!("sync {}: {error}", path.display())))
}

#[derive(Debug)]
pub struct ConversationRead {
    pub events: Vec<RecordedConversationEvent>,
    pub incomplete_tail: bool,
}

pub fn read_conversation_status(path: &Path) -> StoreResult<ConversationRead> {
    let file = fs::File::open(path)
        .map_err(|error| StoreError::InvalidData(format!("open {}: {error}", path.display())))?;
    let mut events = Vec::new();
    let mut incomplete_tail = false;
    let mut reader = BufReader::new(file);
    loop {
        let mut line = String::new();
        let read = reader
            .read_line(&mut line)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        if read == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str(&line) {
            Ok(event) => events.push(event),
            Err(_) if !line.ends_with('\n') => {
                incomplete_tail = true;
                break;
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(ConversationRead {
        events,
        incomplete_tail,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        provider_session_id, read_conversation_status, resolve_artifact, CaptureHandle,
        ContextAssetKind, ContextAssetSpec, ContextChannel, ContextScope, PreparedTurnContext,
        TraceCaptureContext,
    };
    use crate::id::{ExecId, TraceId};

    #[test]
    fn prompt_manifest_covers_exact_bytes_and_tokens() {
        let prepared = PreparedTurnContext::from_prompts("system α", "task β");
        let system = prepared.system.as_ref().expect("system channel");
        assert_eq!(system.assets[0].channel, ContextChannel::System);
        assert_eq!(system.assets[0].byte_end, system.text.len() as u64);
        assert_eq!(
            prepared
                .assets()
                .map(|asset| asset.attributed_tokens)
                .sum::<u64>(),
            prepared.total_tokens()
        );
    }

    #[test]
    fn empty_system_prompt_is_explicitly_absent() {
        let prepared = PreparedTurnContext::from_prompts("", "task");
        assert!(prepared.system.is_none());
        assert_eq!(prepared.assets().count(), 1);
    }

    #[test]
    fn conversation_reader_keeps_complete_crash_tail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(
            &path,
            "{\"schema_version\":1,\"seq\":0,\"ts\":\"2026-07-10T00:00:00Z\",\"turn_id\":null,\"payload\":{\"type\":\"capture_error\",\"message\":\"x\"}}\n{\"schema_version\":",
        )
        .unwrap();
        assert_eq!(read_conversation_status(&path).unwrap().events.len(), 1);
    }

    #[test]
    fn provider_receipt_reads_session_ids_from_observed_frames() {
        assert_eq!(
            provider_session_id(r#"{"type":"system","session_id":"claude-1"}"#).as_deref(),
            Some("claude-1")
        );
        assert_eq!(
            provider_session_id(r#"{"params":{"thread":{"id":"codex-1"}}}"#).as_deref(),
            Some("codex-1")
        );
        assert_eq!(provider_session_id(r#"{"id":"message-1"}"#), None);
    }

    #[test]
    fn semantic_assets_and_assembly_cover_every_prompt_byte() {
        let prepared = PreparedTurnContext::from_attributed_prompts(
            "before GUIDE after",
            "task",
            vec![ContextAssetSpec {
                channel: ContextChannel::System,
                kind: ContextAssetKind::OperatingInstructions,
                scope: ContextScope::Global,
                label: "guide".to_string(),
                source_path: None,
                included_by: "test".to_string(),
                content: "GUIDE".to_string(),
                match_all_occurrences: false,
            }],
            Vec::new(),
        );
        assert_eq!(prepared.decisions.len(), 1);
        assert_eq!(prepared.decisions[0].asset_position, Some(1));
        let system = prepared.system.unwrap();
        assert_eq!(system.assets.len(), 3);
        assert_eq!(
            system.assets[1].kind,
            ContextAssetKind::OperatingInstructions
        );
        assert_eq!(
            system.assets.iter().map(|asset| asset.bytes).sum::<u64>(),
            system.text.len() as u64
        );
        assert_eq!(
            system
                .assets
                .iter()
                .map(|asset| asset.attributed_tokens)
                .sum::<u64>(),
            system.tokens
        );
    }

    #[test]
    fn semantic_attribution_covers_repeated_and_nested_sources() {
        let prepared = PreparedTurnContext::from_attributed_prompts(
            "",
            "GUIDE\nouter MEMORY remainder\nGUIDE",
            vec![
                ContextAssetSpec {
                    channel: ContextChannel::Task,
                    kind: ContextAssetKind::OperatingInstructions,
                    scope: ContextScope::Global,
                    label: "guide".to_string(),
                    source_path: None,
                    included_by: "test".to_string(),
                    content: "GUIDE".to_string(),
                    match_all_occurrences: true,
                },
                ContextAssetSpec {
                    channel: ContextChannel::Task,
                    kind: ContextAssetKind::Memory,
                    scope: ContextScope::Wave,
                    label: "memory".to_string(),
                    source_path: None,
                    included_by: "test".to_string(),
                    content: "MEMORY".to_string(),
                    match_all_occurrences: true,
                },
                ContextAssetSpec {
                    channel: ContextChannel::Task,
                    kind: ContextAssetKind::Goal,
                    scope: ContextScope::Step,
                    label: "inherited launch goal".to_string(),
                    source_path: None,
                    included_by: "message".to_string(),
                    content: "outer MEMORY remainder".to_string(),
                    match_all_occurrences: false,
                },
            ],
            Vec::new(),
        );

        assert_eq!(
            prepared
                .task
                .assets
                .iter()
                .filter(|asset| asset.kind == ContextAssetKind::OperatingInstructions)
                .count(),
            2
        );
        assert_eq!(
            prepared
                .task
                .assets
                .iter()
                .filter(|asset| asset.kind == ContextAssetKind::Goal)
                .count(),
            2
        );
        assert_eq!(
            prepared
                .task
                .assets
                .iter()
                .filter(|asset| asset.kind == ContextAssetKind::Assembly)
                .map(|asset| asset.bytes)
                .sum::<u64>(),
            2
        );
        assert_eq!(
            prepared.total_tokens(),
            prepared
                .assets()
                .map(|asset| asset.attributed_tokens)
                .sum::<u64>()
        );
    }

    #[test]
    fn first_match_sources_do_not_claim_matching_words_elsewhere() {
        let prepared = PreparedTurnContext::from_attributed_prompts(
            "",
            "go then go",
            vec![ContextAssetSpec {
                channel: ContextChannel::Task,
                kind: ContextAssetKind::UserMessage,
                scope: ContextScope::User,
                label: "user message".to_string(),
                source_path: None,
                included_by: "message".to_string(),
                content: "go".to_string(),
                match_all_occurrences: false,
            }],
            Vec::new(),
        );

        assert_eq!(
            prepared
                .task
                .assets
                .iter()
                .filter(|asset| asset.kind == ContextAssetKind::UserMessage)
                .count(),
            1
        );
        assert_eq!(
            prepared
                .task
                .assets
                .iter()
                .filter(|asset| asset.kind == ContextAssetKind::Assembly)
                .map(|asset| asset.bytes)
                .sum::<u64>(),
            8
        );
    }

    #[test]
    fn large_prompt_attribution_remains_exact() {
        let sections = (0..12)
            .map(|index| {
                format!(
                    "<section-{index}>{}</section-{index}>",
                    "context ".repeat(750)
                )
            })
            .collect::<Vec<_>>();
        let prompt = sections.join("\n");
        let specs = sections
            .into_iter()
            .enumerate()
            .map(|(index, content)| ContextAssetSpec {
                channel: ContextChannel::Task,
                kind: ContextAssetKind::Document,
                scope: ContextScope::Task,
                label: format!("section {index}"),
                source_path: None,
                included_by: "test".to_string(),
                content,
                match_all_occurrences: false,
            })
            .collect();

        let prepared = PreparedTurnContext::from_attributed_prompts("", &prompt, specs, Vec::new());
        assert_eq!(
            prepared.total_tokens(),
            prepared
                .assets()
                .map(|asset| asset.attributed_tokens)
                .sum::<u64>()
        );
        assert_eq!(
            prepared.task.assets.last().unwrap().byte_end,
            prompt.len() as u64
        );
    }

    #[test]
    fn capture_persists_private_artifacts_and_queryable_rows() {
        let guard = crate::journal::TestLedgerGuard::new();
        let run_id = TraceId::new();
        let context = TraceCaptureContext {
            run_id: run_id.clone(),
            process_id: ExecId::new(),
            repo: guard.home().to_path_buf(),
            worktree: guard.home().to_path_buf(),
            wave: Some("intelligence".to_string()),
            project: Some("context".to_string()),
            task: Some("W2-71".to_string()),
            flow: None,
            skill: Some("implement".to_string()),
        };
        let capture = CaptureHandle::begin(
            context,
            PreparedTurnContext::from_prompts("system", "task"),
            super::CaptureStart {
                provider: "codex".to_string(),
                model: Some("gpt-5".to_string()),
                surface: "headless".to_string(),
                input_op: "initial".to_string(),
                gather_ms: 1,
                render_ms: 2,
                raw_provider: true,
                basis: None,
                control: None,
            },
        )
        .unwrap();
        capture.record_conversation(crate::chat::types::ConversationEvent::TextDelta {
            turn_id: "provider-turn-1".to_string(),
            content: "partial child answer".to_string(),
        });
        capture.begin_turn("message", "follow up").unwrap();
        capture
            .fail_and_begin_launch(
                "codex".to_string(),
                Some("gpt-5.6".to_string()),
                "retry after provider failure",
            )
            .unwrap();
        capture.finish("completed", false).unwrap();

        let store = crate::journal::open_ledger().unwrap();
        let launches = store.agent_launches_matching(run_id.as_str()).unwrap();
        assert_eq!(launches.len(), 2);
        assert_eq!(launches[0].outcome, "failed");
        assert_eq!(launches[1].outcome, "completed");
        assert_eq!(launches[1].capture_status, "complete");
        assert_eq!(launches[0].project.as_deref(), Some("context"));
        assert_eq!(launches[0].task.as_deref(), Some("W2-71"));
        assert!(!std::path::Path::new(&launches[0].artifact_dir).is_absolute());
        let conversation = super::resolve_artifact(&launches[0].conversation_path).unwrap();
        assert!(conversation.is_file());
        let first_turns = store
            .agent_turns_for_launches(&[launches[0].id.clone()])
            .unwrap();
        assert_eq!(first_turns.len(), 2);
        assert_eq!(first_turns[0].status, "partial");
        assert_eq!(
            first_turns[0].root_output.as_deref(),
            Some("partial child answer")
        );
        assert_eq!(first_turns[1].status, "failed");
        let retry_turns = store
            .agent_turns_for_launches(&[launches[1].id.clone()])
            .unwrap();
        assert_eq!(retry_turns.len(), 1);
        assert_eq!(retry_turns[0].context_coverage, "provider_total_only");
        assert_eq!(retry_turns[0].provider_input_tokens, None);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(conversation)
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn capture_hydrates_the_registered_product_launch() {
        let guard = crate::journal::TestLedgerGuard::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (run, launch) = runtime.block_on(async {
            let store = crate::store::open_store(&crate::store::StorageConfig::sqlite(
                guard.home().join("loopflow.db"),
            ))
            .await
            .unwrap();
            let wave = crate::wave::Wave::new(
                crate::id::WaveId::new(),
                "capture-launch".to_string(),
                guard.home().display().to_string(),
            );
            store.create_wave(&wave).await.unwrap();
            let home = store.home("test-home").await.unwrap();
            let work = crate::durable::WorkRef::Wave(wave.id().clone());
            let (run, lease) = store
                .reserve_run(&work, &home.id, crate::durable::RunTrigger::User)
                .await
                .unwrap();
            let receipt = store
                .advance_run(
                    &lease,
                    crate::durable::RunAdvance::LaunchStarting {
                        route: crate::durable::LaunchRoute {
                            provider: "codex".to_string(),
                            model: Some("gpt-5".to_string()),
                            account_id: None,
                        },
                        containment: crate::durable::Containment::Tmux {
                            name: "lf-capture-launch".to_string(),
                        },
                        cwd: guard.home().to_path_buf(),
                        surface: "headless".to_string(),
                        opaque: false,
                        resume_token: None,
                    },
                )
                .await
                .unwrap();
            let crate::durable::AdvanceReceipt::Launch(launch) = receipt else {
                panic!("expected Launch receipt")
            };
            store
                .advance_run(
                    &lease,
                    crate::durable::RunAdvance::LaunchLive {
                        launch_id: launch.id.clone(),
                    },
                )
                .await
                .unwrap();
            (run, launch)
        });

        let capture = CaptureHandle::begin(
            TraceCaptureContext {
                run_id: TraceId::new(),
                process_id: ExecId::new(),
                repo: guard.home().to_path_buf(),
                worktree: guard.home().to_path_buf(),
                wave: Some("capture-launch".to_string()),
                project: None,
                task: None,
                flow: Some("wave".to_string()),
                skill: Some("pursue".to_string()),
            },
            PreparedTurnContext::from_prompts("system", "task"),
            super::CaptureStart {
                provider: "codex".to_string(),
                model: Some("gpt-5".to_string()),
                surface: "headless".to_string(),
                input_op: "initial".to_string(),
                gather_ms: 1,
                render_ms: 2,
                raw_provider: true,
                basis: None,
                control: Some(super::ControlLaunch {
                    run_id: run.id.clone(),
                    home_id: run.home_id.clone(),
                    account_id: None,
                    containment: launch.containment.clone(),
                    resume_token: None,
                    opaque_basis: None,
                }),
            },
        )
        .unwrap();
        assert_eq!(capture.launch_id(), launch.id);
        capture.finish("completed", false).unwrap();

        let launches = crate::journal::open_ledger()
            .unwrap()
            .agent_launches_since(0)
            .unwrap()
            .into_iter()
            .filter(|row| {
                row.control
                    .as_ref()
                    .is_some_and(|control| control.run_id == run.id)
            })
            .collect::<Vec<_>>();
        assert_eq!(launches.len(), 1);
        assert_eq!(launches[0].id, launch.id.as_str());
        assert_eq!(launches[0].capture_status, "complete");
        let surface = crate::journal::open_ledger()
            .unwrap()
            .launch_surface(&launch.id)
            .unwrap()
            .expect("captured Launch remains historical evidence");
        assert_eq!(surface.launch.state, crate::durable::LaunchState::Ended);
        assert_eq!(
            surface.handback,
            Some(crate::durable::BoundaryState::Succeeded)
        );
    }

    #[test]
    fn artifact_paths_cannot_escape_the_trace_root() {
        assert!(resolve_artifact("run/process/launch/conversation.jsonl").is_ok());
        assert!(resolve_artifact("../outside").is_err());
        assert!(resolve_artifact("/absolute").is_err());
    }

    #[test]
    fn the_trace_root_follows_the_home_that_owns_the_store() {
        // A reference is only resolvable if capture and doctor agree on the
        // root. When traces answered to LF_HOME alone while the store also
        // honored the control-home and dev/release split, a dev build wrote its
        // artifacts into the release trace root and its rows into a dev store —
        // manufacturing "orphan" artifacts and dangling references out of runs
        // that had in fact captured perfectly.
        let guard = crate::journal::TestLedgerGuard::new();

        assert_eq!(super::trace_root(), guard.home().join("traces"));
        assert_eq!(
            super::trace_root(),
            crate::store::lf_home_dir().join("traces"),
            "the trace root must be derived from the store's home resolver"
        );
    }
}
