use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use time::OffsetDateTime;

use crate::lfd::queue::QueueRunView;
use crate::lfd::registration::{RegistrationPublicSummary, RegistrationState};
use crate::lfd::sessions::types::ContextSnapshot;
use crate::lfd::sessions::usage::{
    StepUsageAggregate, TokenTotals, UsageSummaryGroupAggregate, UsageTimeseriesBucketAggregate,
};
use crate::lfd::types::{
    ActivationLog, AttentionItem, ChatMemoryBlock, ChatMessage, LivePullRequestState,
    TerminalSession, Trigger, WaveRun, WaveRunStatus,
};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: i64,
    pub database: bool,
    pub waves_running: u32,
    pub agents_active: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration: Option<RegistrationPublicSummary>,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub pid: u32,
    pub waves_defined: u32,
    pub waves_running: u32,
    pub agents_active: u32,
    pub slots_used: u32,
    pub slots_total: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration: Option<RegistrationState>,
}

#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    pub waves_total: u32,
    pub waves_running: u32,
    pub agents_active: u32,
    pub slots_used: u32,
    pub slots_total: u32,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub object: String,
    pub data: Vec<T>,
    pub has_more: bool,
}

impl<T> ListResponse<T> {
    pub fn new(data: Vec<T>, has_more: bool) -> Self {
        Self {
            object: "list".to_string(),
            data,
            has_more,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitEntryDto {
    pub sha: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WaveDto {
    pub id: String,
    pub object: String,
    pub name: String,
    pub repo: String,
    pub mode: String,
    pub primary_flow: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_agents: Option<HashMap<String, String>>,
    pub created_at: Option<String>,
    pub status: String,
    pub iteration: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_worktree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_branch: Option<String>,
    pub commits: Vec<CommitEntryDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_stat: Option<String>,
    pub flow_steps: Vec<String>,
    pub open_pr_count: u32,
    pub stack_count: u32,
    pub has_stale_pr_state: bool,
    pub serialized: bool,
    pub triggers: Vec<TriggerDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_run: Option<WaveRunDto>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WaveRunDto {
    pub id: String,
    pub object: String,
    pub wave_id: String,
    pub flow: String,
    pub repo: String,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub iteration: u32,
    pub step_index: u32,
    pub status: String,
    pub local_worktree: String,
    pub remote_branch: String,
    pub target_branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr: Option<PullRequestDto>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub error: Option<String>,
    pub flow_parents: Vec<String>,
    pub stack_position: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_pr_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_pr_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_pr_is_draft: Option<bool>,
    pub pr_state_stale: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_block_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_blocked_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PullRequestDto {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RunWaveResponse {
    pub started: bool,
    pub wave_id: String,
    pub wave_run_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TriggerDto {
    pub id: String,
    pub signal: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_wave_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_main_sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_triggered_at: Option<i64>,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttentionItemDto {
    pub id: String,
    pub object: String,
    pub wave_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub kind: String,
    pub status: String,
    pub title: String,
    pub summary: String,
    pub context: serde_json::Value,
    pub surfaced_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<String>,
}

pub fn attention_item_dto(item: AttentionItem) -> AttentionItemDto {
    AttentionItemDto {
        id: item.id.to_string(),
        object: "attention_item".to_string(),
        wave_id: item.wave_id.to_string(),
        run_id: item.run_id.map(|value| value.to_string()),
        kind: item.kind.as_str().to_string(),
        status: item.status.as_str().to_string(),
        title: item.title,
        summary: item.summary,
        context: item.context,
        surfaced_at: format_datetime(Some(item.surfaced_at)).unwrap_or_default(),
        viewed_at: format_datetime(item.viewed_at),
        resolved_at: format_datetime(item.resolved_at),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TerminalSessionDto {
    pub id: String,
    pub object: String,
    pub wave_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave_run_id: Option<String>,
    pub step: String,
    pub agent: String,
    pub cwd: String,
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub source: String,
    pub status: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attached_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

pub fn terminal_session_dto(session: TerminalSession) -> TerminalSessionDto {
    TerminalSessionDto {
        id: session.id.to_string(),
        object: "terminal_session".to_string(),
        wave_id: session.wave_id.to_string(),
        wave_run_id: session.wave_run_id.map(|value| value.to_string()),
        step: session.step,
        agent: session.agent,
        cwd: session.cwd,
        argv: session.argv,
        env: session.env,
        source: session.source,
        status: session.status.as_str().to_string(),
        created_at: format_datetime(Some(session.created_at)).unwrap_or_default(),
        attached_at: format_datetime(session.attached_at),
        started_at: format_datetime(session.started_at),
        completed_at: format_datetime(session.completed_at),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TerminalLaunchSpecDto {
    pub session_id: String,
    pub wave_id: String,
    pub step: String,
    pub agent: String,
    pub cwd: String,
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub completion_token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActivationLogDto {
    pub id: String,
    pub object: String,
    pub wave_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_id: Option<String>,
    pub reason: String,
    pub outcome: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatMemoryBlockDto {
    pub object: String,
    pub name: String,
    pub content: String,
    pub position: u32,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatStartedDto {
    pub object: String,
    pub wave_id: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct ChatMessageDto {
    pub id: String,
    pub object: String,
    pub role: String,
    pub content: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StopWaveResponse {
    pub stopped: bool,
}

#[derive(Debug, Serialize)]
pub struct RestartStepResponse {
    pub restarted: bool,
    pub wave_id: String,
    pub wave_run_id: String,
    pub step_index: u32,
}

#[derive(Debug, Serialize)]
pub struct ContinueWaveResponse {
    pub continued: bool,
    pub wave_id: String,
    pub wave_run_id: String,
}

#[derive(Debug, Serialize)]
pub struct LandWaveResponse {
    pub merged: bool,
}

#[derive(Debug, Serialize)]
pub struct NextWaveResponse {
    pub new_branch: String,
}

#[derive(Debug, Serialize)]
pub struct CombineResponse {
    pub ok: bool,
    pub result: CombineResponseResult,
}

#[derive(Debug, Serialize)]
pub struct CombineResponseResult {
    pub new_pr_url: Option<String>,
    pub closed_prs: Vec<u64>,
}

#[derive(Debug, Serialize)]
pub struct DeletedResourceResponse {
    pub id: String,
    pub object: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionUsageSessionDto {
    pub step: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave: Option<String>,
    pub status: String,
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionUsageDto {
    pub object: String,
    pub session_id: String,
    pub tokens: TokenTotals,
    pub turns: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<ContextSnapshot>,
    pub models: BTreeMap<String, u64>,
    pub session: SessionUsageSessionDto,
}

#[derive(Debug, Clone, Serialize)]
pub struct WaveUsageDto {
    pub object: String,
    pub wave_id: String,
    pub tokens: TokenTotals,
    pub sessions: u64,
    pub turns: u64,
    pub models: BTreeMap<String, u64>,
    pub by_step: BTreeMap<String, StepUsageAggregate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageSummaryDto {
    pub object: String,
    pub group_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    pub groups: Vec<UsageSummaryGroupAggregate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageTimeseriesDto {
    pub object: String,
    pub bucket: String,
    pub group_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    pub buckets: Vec<UsageTimeseriesBucketAggregate>,
}

pub fn format_datetime(datetime: Option<OffsetDateTime>) -> Option<String> {
    datetime?
        .format(&time::format_description::well_known::Rfc3339)
        .ok()
}

pub fn wave_run_dto(
    run: WaveRun,
    live_pr_state: Option<&LivePullRequestState>,
    pr_state_stale: bool,
    queue_view: Option<&QueueRunView>,
) -> WaveRunDto {
    WaveRunDto {
        id: run.id.to_string(),
        object: "wave_run".to_string(),
        wave_id: run.wave_id.to_string(),
        flow: run.snapshot.flow.clone(),
        repo: run.snapshot.repo.clone(),
        direction: run.snapshot.direction.clone(),
        area: run.snapshot.area.clone(),
        iteration: run.iteration,
        step_index: run.step_index,
        status: wave_run_status_str(run.status),
        local_worktree: run.worktree,
        remote_branch: run.branch,
        target_branch: run.target_branch,
        pr: run.pr.map(|pr| PullRequestDto {
            url: pr.url,
            number: pr.number,
            state: pr.state,
            title: pr.title,
            branch: pr.branch,
        }),
        started_at: format_datetime(run.started_at),
        ended_at: format_datetime(run.ended_at),
        error: run.error,
        flow_parents: run.flow_parents,
        stack_position: run.stack_position,
        parent_run_id: run.parent_run_id.map(|value| value.to_string()),
        parent_pr_number: run.parent_pr_number,
        live_pr_state: live_pr_state.map(|value| value.state.as_str().to_string()),
        live_pr_is_draft: live_pr_state.map(|value| value.is_draft),
        pr_state_stale,
        queue_role: queue_view.map(|value| value.role.as_str().to_string()),
        queue_block_reason: queue_view
            .and_then(|value| value.block_reason.map(|reason| reason.as_str().to_string())),
        queue_blocked_at: format_datetime(queue_view.and_then(|value| value.blocked_at)),
        next_action: queue_view.map(|value| value.next_action.as_str().to_string()),
        created_at: format_datetime(run.started_at),
    }
}

pub fn trigger_dto(t: Trigger) -> TriggerDto {
    TriggerDto {
        id: t.id.to_string(),
        signal: t.signal.as_str().to_string(),
        enabled: t.enabled,
        flow: t.flow,
        source_wave_id: t.source_wave_id.map(|value| value.to_string()),
        last_main_sha: t.last_main_sha,
        last_triggered_at: t.last_triggered_at,
        created_at: format_datetime(t.created_at),
    }
}

pub fn activation_log_dto(log: ActivationLog) -> ActivationLogDto {
    ActivationLogDto {
        id: log.id.to_string(),
        object: "activation_log".to_string(),
        wave_id: log.wave_id.to_string(),
        trigger_id: log.trigger_id.map(|id| id.to_string()),
        reason: log.reason,
        outcome: log.outcome.as_str().to_string(),
        created_at: format_datetime(time::OffsetDateTime::from_unix_timestamp(log.created_at).ok()),
    }
}

pub fn chat_memory_block_dto(block: ChatMemoryBlock) -> ChatMemoryBlockDto {
    ChatMemoryBlockDto {
        object: "memory_block".to_string(),
        name: block.name,
        content: block.content,
        position: block.position,
        updated_at: format_datetime(block.updated_at),
    }
}

pub fn chat_message_dto(message: ChatMessage) -> ChatMessageDto {
    ChatMessageDto {
        id: message.id.to_string(),
        object: "chat_message".to_string(),
        role: message.role,
        content: message.content,
        created_at: format_datetime(Some(message.created_at)),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeDto {
    pub object: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub merged: bool,
    pub prunable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoDto {
    pub object: String,
    pub path: String,
    pub name: String,
    pub repo_id: String,
    pub wave_count: u32,
    pub registered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added_at: Option<String>,
}

pub fn wave_run_status_str(status: WaveRunStatus) -> String {
    match status {
        WaveRunStatus::Pending => "pending",
        WaveRunStatus::Running => "running",
        WaveRunStatus::Waiting => "waiting",
        WaveRunStatus::Completed => "completed",
        WaveRunStatus::Failed => "failed",
        WaveRunStatus::Unspecified => "unknown",
    }
    .to_string()
}

#[cfg(test)]
mod contract_tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(name);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
    }

    #[test]
    fn wave_fixture_deserializes() {
        let json = fixture("wave.json");
        let wave: WaveDto = serde_json::from_str(&json).unwrap();

        assert_eq!(wave.id, "wave_abc123");
        assert_eq!(wave.object, "wave");
        assert_eq!(wave.name, "engbot");
        assert_eq!(wave.primary_flow, "build");
        assert_eq!(wave.mode, "loop");
        assert_eq!(wave.status, "running");
        assert_eq!(wave.iteration, 3);
        assert_eq!(wave.direction, vec!["ux", "clarity"]);
        assert_eq!(wave.area, vec!["src/"]);
        assert_eq!(wave.open_pr_count, 1);

        assert_eq!(wave.triggers.len(), 2);
        assert_eq!(wave.triggers[0].signal, "repo");
        assert_eq!(wave.triggers[0].flow.as_deref(), Some("integrate"));
        assert_eq!(wave.triggers[1].signal, "ci_failure");

        assert_eq!(wave.commits.len(), 1);
        assert_eq!(wave.commits[0].sha, "abc1234");
    }

    #[test]
    fn trigger_fixture_deserializes() {
        let json = fixture("trigger.json");
        let trigger: TriggerDto = serde_json::from_str(&json).unwrap();

        assert_eq!(trigger.id, "trig_abc123");
        assert_eq!(trigger.signal, "wave");
        assert!(trigger.enabled);
        assert_eq!(trigger.flow.as_deref(), Some("build"));
        assert_eq!(trigger.source_wave_id.as_deref(), Some("wave_upstream"));
        assert_eq!(trigger.last_main_sha.as_deref(), Some("deadbeef1234"));
        assert_eq!(trigger.last_triggered_at, Some(1737000000));
    }

    #[test]
    fn activation_log_fixture_deserializes() {
        let json = fixture("activation_log.json");
        let log: ActivationLogDto = serde_json::from_str(&json).unwrap();

        assert_eq!(log.id, "act_abc123");
        assert_eq!(log.object, "activation_log");
        assert_eq!(log.wave_id, "wave_abc123");
        assert_eq!(log.trigger_id.as_deref(), Some("trig_001"));
        assert_eq!(log.outcome, "started");
        assert!(log.reason.contains("main"));
    }
}
