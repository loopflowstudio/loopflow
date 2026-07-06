use crate::lfd::id::LfdId;
use crate::lfd::types::{
    ChatMemoryBlock, ChatMessage, LivePrState, LivePullRequestState, PullRequest, Repo, RepoEdge,
    RepoId, Run, RunStackStatus, RunStatus, Summary, Wave, WaveStatus,
};
use crate::lfdb::{
    ForkRun, ForkRunStatus, RepoProviderUsage, StoreError, StoreResult, WaveProviderUsage,
};

// -- Row helpers --------------------------------------------------------------

fn text(row: &rusqlite::Row<'_>, idx: usize) -> StoreResult<String> {
    Ok(row.get(idx)?)
}

fn opt_text(row: &rusqlite::Row<'_>, idx: usize) -> StoreResult<Option<String>> {
    Ok(row.get(idx)?)
}

fn int(row: &rusqlite::Row<'_>, idx: usize) -> StoreResult<i32> {
    Ok(row.get::<_, i64>(idx)? as i32)
}

fn bigint(row: &rusqlite::Row<'_>, idx: usize) -> StoreResult<i64> {
    Ok(row.get(idx)?)
}

fn opt_bigint(row: &rusqlite::Row<'_>, idx: usize) -> StoreResult<Option<i64>> {
    Ok(row.get(idx)?)
}

// -- Shared utilities --------------------------------------------------------

pub fn unix_to_datetime(seconds: i64) -> time::OffsetDateTime {
    time::OffsetDateTime::from_unix_timestamp(seconds).unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
}

pub fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

pub fn parse_json_vec(value: &str) -> StoreResult<Vec<String>> {
    serde_json::from_str::<Vec<String>>(value).map_err(StoreError::Serde)
}

pub fn serialize_pr(value: &Option<PullRequest>) -> StoreResult<Option<String>> {
    match value {
        Some(pr) => Ok(Some(serde_json::to_string(pr)?)),
        None => Ok(None),
    }
}

pub fn parse_pr(value: Option<String>) -> StoreResult<Option<PullRequest>> {
    match value {
        Some(raw) if !raw.trim().is_empty() => serde_json::from_str::<PullRequest>(&raw)
            .map(Some)
            .map_err(StoreError::Serde),
        _ => Ok(None),
    }
}

// -- Shared row mappers ------------------------------------------------------

/// SELECT id, name, direction, area, paused, created_at, workers,
///        primary_flow, goal, metrics, parent_wave_id,
///        repo, worktree, branch, status, iteration, cycle_start_iteration
pub fn map_wave_row(row: &rusqlite::Row<'_>) -> StoreResult<Wave> {
    let direction = parse_json_vec(&text(row, 2)?)?;
    let area = parse_json_vec(&text(row, 3)?)?;
    let paused = int(row, 4)? != 0;
    let created_at = unix_to_datetime(bigint(row, 5)?);
    let workers = int(row, 6)? as u32;
    let primary_flow = text(row, 7)?;
    // Legacy rows predating migration 037 (goal NOT NULL DEFAULT) can hold
    // NULL; fall back to the same default so `lf ls`/reads stay robust.
    let goal = opt_text(row, 8)?.unwrap_or_else(|| "ship-roadmap".to_string());
    let metrics = parse_json_vec(&text(row, 9)?)?;
    let parent_wave_id = opt_text(row, 10)?.map(LfdId::from_raw);

    Ok(Wave {
        id: LfdId::from_raw(text(row, 0)?),
        name: text(row, 1)?,
        primary_flow,
        goal,
        metrics,
        repo: text(row, 11)?,
        worktree: text(row, 12)?,
        branch: text(row, 13)?,
        status: WaveStatus::from_i32(int(row, 14)?),
        iteration: int(row, 15)? as u32,
        cycle_start_iteration: int(row, 16)? as u32,
        direction,
        area,
        paused,
        created_at: Some(created_at),
        workers,
        parent_wave_id,
    })
}

/// SELECT path, repo_id, name, added_at
pub fn map_repo_row(row: &rusqlite::Row<'_>) -> StoreResult<Repo> {
    Ok(Repo {
        path: text(row, 0)?,
        repo_id: RepoId::from_raw(text(row, 1)?),
        name: text(row, 2)?,
        added_at: unix_to_datetime(bigint(row, 3)?),
    })
}

/// SELECT parent_repo_id, child_repo_id
pub fn map_repo_edge_row(row: &rusqlite::Row<'_>) -> StoreResult<RepoEdge> {
    Ok(RepoEdge {
        parent_repo_id: RepoId::from_raw(text(row, 0)?),
        child_repo_id: RepoId::from_raw(text(row, 1)?),
    })
}

/// SELECT id, wave_id, iteration, step_index, status, worktree, branch,
///        started_at, ended_at, error, snapshot_repo, snapshot_flow,
///        snapshot_task, snapshot_direction, snapshot_area, snapshot_pr,
///        flow_parents, execution_cursor, parent_run_id,
///        parent_pr_number, stack_position, stack_group_id, stack_status,
///        lineage_inferred, target_branch, repair_of
pub fn map_run_row(row: &rusqlite::Row<'_>) -> StoreResult<Run> {
    let started_at = unix_to_datetime(bigint(row, 7)?);
    let ended_at = opt_bigint(row, 8)?;
    let snapshot_direction = parse_json_vec(&text(row, 13)?)?;
    let snapshot_area = parse_json_vec(&text(row, 14)?)?;
    let snapshot_pr = parse_pr(opt_text(row, 15)?)?;
    let flow_parents = parse_json_vec(&text(row, 16)?)?;
    let execution_cursor = opt_text(row, 17)?;
    let parent_run_id = opt_text(row, 18)?.map(LfdId::from_raw);
    let parent_pr_number = opt_bigint(row, 19)?.map(|value| value as u32);
    let stack_position = int(row, 20)? as u32;
    let stack_group_id = text(row, 21)?;
    let stack_status = RunStackStatus::from_i32(int(row, 22)?);
    let lineage_inferred = int(row, 23)? != 0;
    let target_branch = text(row, 24)?;
    let repair_of = opt_text(row, 25)?.map(LfdId::from_raw);

    Ok(Run {
        id: LfdId::from_raw(text(row, 0)?),
        wave_id: LfdId::from_raw(text(row, 1)?),
        repo: text(row, 10)?,
        flow: text(row, 11)?,
        task: opt_text(row, 12)?,
        direction: snapshot_direction,
        area: snapshot_area,
        iteration: int(row, 2)? as u32,
        step_index: int(row, 3)? as u32,
        status: RunStatus::from_i32(int(row, 4)?),
        worktree: text(row, 5)?,
        branch: text(row, 6)?,
        started_at: Some(started_at),
        ended_at: ended_at.map(unix_to_datetime),
        error: opt_text(row, 9)?,
        flow_parents,
        execution_cursor,
        parent_run_id,
        parent_pr_number,
        stack_position,
        stack_group_id,
        stack_status,
        lineage_inferred,
        target_branch,
        repair_of,
        pr: snapshot_pr,
    })
}

/// SELECT repo_id, pr_number, state, is_draft, head_ref, head_sha, base_ref,
///        updated_at, merged_at, synced_at
pub fn map_live_pr_state_row(row: &rusqlite::Row<'_>) -> StoreResult<LivePullRequestState> {
    Ok(LivePullRequestState {
        repo_id: text(row, 0)?,
        pr_number: bigint(row, 1)? as u32,
        state: LivePrState::from_i32(int(row, 2)?),
        is_draft: int(row, 3)? != 0,
        head_ref: text(row, 4)?,
        head_sha: text(row, 5)?,
        base_ref: text(row, 6)?,
        updated_at: unix_to_datetime(bigint(row, 7)?),
        merged_at: opt_bigint(row, 8)?.map(unix_to_datetime),
        synced_at: unix_to_datetime(bigint(row, 9)?),
    })
}

/// SELECT id, run_id, step_index, branch_index, status, worktree
pub fn map_fork_run_row(row: &rusqlite::Row<'_>) -> StoreResult<ForkRun> {
    let status = ForkRunStatus::from_i64(int(row, 4)? as i64)
        .ok_or_else(|| StoreError::InvalidData("invalid fork run status".to_string()))?;

    Ok(ForkRun {
        id: LfdId::from_raw(text(row, 0)?),
        run_id: LfdId::from_raw(text(row, 1)?),
        step_index: int(row, 2)? as u32,
        branch_index: int(row, 3)? as u32,
        status,
        worktree: text(row, 5)?,
    })
}

/// SELECT id, wave_id, content, source_hash, token_budget, model, created_at
pub fn map_summary_row(row: &rusqlite::Row<'_>) -> StoreResult<Summary> {
    Ok(Summary {
        id: LfdId::from_raw(text(row, 0)?),
        wave_id: LfdId::from_raw(text(row, 1)?),
        content: text(row, 2)?,
        source_hash: text(row, 3)?,
        token_budget: int(row, 4)? as u32,
        agent: text(row, 5)?,
        created_at: Some(unix_to_datetime(bigint(row, 6)?)),
    })
}

/// SELECT wave_id, name, content, position, updated_at
pub fn map_chat_memory_block_row(row: &rusqlite::Row<'_>) -> StoreResult<ChatMemoryBlock> {
    Ok(ChatMemoryBlock {
        wave_id: LfdId::from_raw(text(row, 0)?),
        name: text(row, 1)?,
        content: text(row, 2)?,
        position: int(row, 3)? as u32,
        updated_at: Some(unix_to_datetime(bigint(row, 4)?)),
    })
}

/// SELECT wave, provider, SUM(input_tokens), SUM(output_tokens), SUM(cache_read_tokens)
pub fn map_wave_provider_usage_row(row: &rusqlite::Row<'_>) -> StoreResult<WaveProviderUsage> {
    Ok(WaveProviderUsage {
        wave: LfdId::from_raw(text(row, 0)?),
        provider: text(row, 1)?,
        input_tokens: bigint(row, 2)?.max(0) as u64,
        output_tokens: bigint(row, 3)?.max(0) as u64,
        cache_read_tokens: bigint(row, 4)?.max(0) as u64,
    })
}

/// SELECT repo, provider, SUM(input_tokens), SUM(output_tokens), SUM(cache_read_tokens)
pub fn map_repo_provider_usage_row(row: &rusqlite::Row<'_>) -> StoreResult<RepoProviderUsage> {
    Ok(RepoProviderUsage {
        repo: opt_text(row, 0)?,
        provider: text(row, 1)?,
        input_tokens: bigint(row, 2)?.max(0) as u64,
        output_tokens: bigint(row, 3)?.max(0) as u64,
        cache_read_tokens: bigint(row, 4)?.max(0) as u64,
    })
}

/// SELECT id, wave_id, role, content, created_at
pub fn map_chat_message_row(row: &rusqlite::Row<'_>) -> StoreResult<ChatMessage> {
    Ok(ChatMessage {
        id: LfdId::from_raw(text(row, 0)?),
        wave_id: LfdId::from_raw(text(row, 1)?),
        role: text(row, 2)?,
        content: text(row, 3)?,
        created_at: unix_to_datetime(bigint(row, 4)?),
    })
}
