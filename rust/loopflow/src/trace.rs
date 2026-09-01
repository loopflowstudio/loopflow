//! Provider-facing prompt assembly and attribution.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::engine::prompt::{account_prompt_tokens, count_tokens};
use crate::store::{StoreError, StoreResult};

pub const TOKENIZER: &str = "cl100k_base";

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

#[derive(Debug, Clone, Serialize)]
pub struct RenderedPromptChannel {
    pub text: String,
    pub tokens: u64,
    pub assets: Vec<ContextAsset>,
}

#[derive(Debug, Clone, Serialize)]
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

#[cfg(test)]
mod tests {
    use super::{
        ContextAssetKind, ContextAssetSpec, ContextChannel, ContextScope, PreparedTurnContext,
    };

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
                    label: "inherited invocation goal".to_string(),
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
}
