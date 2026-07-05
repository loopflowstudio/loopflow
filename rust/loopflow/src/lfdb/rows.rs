use crate::lfd::id::LfdId;
use crate::lfd::types::{
    ChatMemoryBlock, ChatMessage, LivePrState, LivePullRequestState, PullRequest, Repo, RepoEdge,
    RepoId, RepoWork, Run, RunStackStatus, RunStatus, Summary, Wave, WaveMode, WaveStatus,
};
use crate::lfdb::{
    ForkRun, ForkRunStatus, RepoProviderUsage, StoreError, StoreResult, WaveProviderUsage,
};

// -- Row adapter trait -------------------------------------------------------

/// Abstracts row access for both rusqlite::Row and tokio_postgres::Row.
///
/// INTEGER columns (status, iteration, etc.) are read via `int()` → i32.
/// BIGINT columns (timestamps) are read via `bigint()` → i64.
/// TEXT columns are read via `text()` → String.
pub trait StoreRow {
    fn text(&self, idx: usize) -> StoreResult<String>;
    fn opt_text(&self, idx: usize) -> StoreResult<Option<String>>;
    fn int(&self, idx: usize) -> StoreResult<i32>;
    fn opt_int(&self, idx: usize) -> StoreResult<Option<i32>>;
    fn bigint(&self, idx: usize) -> StoreResult<i64>;
    fn opt_bigint(&self, idx: usize) -> StoreResult<Option<i64>>;
}

impl StoreRow for rusqlite::Row<'_> {
    fn text(&self, idx: usize) -> StoreResult<String> {
        Ok(self.get(idx)?)
    }
    fn opt_text(&self, idx: usize) -> StoreResult<Option<String>> {
        Ok(self.get(idx)?)
    }
    fn int(&self, idx: usize) -> StoreResult<i32> {
        // SQLite stores all integers as i64; truncate for INTEGER columns
        Ok(self.get::<_, i64>(idx)? as i32)
    }
    fn opt_int(&self, idx: usize) -> StoreResult<Option<i32>> {
        Ok(self.get::<_, Option<i64>>(idx)?.map(|v| v as i32))
    }
    fn bigint(&self, idx: usize) -> StoreResult<i64> {
        Ok(self.get(idx)?)
    }
    fn opt_bigint(&self, idx: usize) -> StoreResult<Option<i64>> {
        Ok(self.get(idx)?)
    }
}

impl StoreRow for tokio_postgres::Row {
    fn text(&self, idx: usize) -> StoreResult<String> {
        Ok(self.try_get(idx)?)
    }
    fn opt_text(&self, idx: usize) -> StoreResult<Option<String>> {
        Ok(self.try_get(idx)?)
    }
    fn int(&self, idx: usize) -> StoreResult<i32> {
        Ok(self.try_get(idx)?)
    }
    fn opt_int(&self, idx: usize) -> StoreResult<Option<i32>> {
        Ok(self.try_get(idx)?)
    }
    fn bigint(&self, idx: usize) -> StoreResult<i64> {
        Ok(self.try_get(idx)?)
    }
    fn opt_bigint(&self, idx: usize) -> StoreResult<Option<i64>> {
        Ok(self.try_get(idx)?)
    }
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

/// SELECT id, name, direction, area, paused, created_at, workers, mode,
///        primary_flow, goal, metrics, parent_wave_id
pub fn map_wave_row(row: &impl StoreRow) -> StoreResult<Wave> {
    let direction = parse_json_vec(&row.text(2)?)?;
    let area = parse_json_vec(&row.text(3)?)?;
    let paused = row.int(4)? != 0;
    let created_at = unix_to_datetime(row.bigint(5)?);
    let workers = row.int(6)? as u32;
    let mode_str = row.text(7)?;
    let mode = mode_str.parse::<WaveMode>().unwrap_or_default();
    let primary_flow = row.text(8)?;
    let goal = row.text(9)?;
    let metrics = parse_json_vec(&row.text(10)?)?;
    let parent_wave_id = row.opt_text(11)?.map(LfdId::from_raw);

    Ok(Wave {
        id: LfdId::from_raw(row.text(0)?),
        name: row.text(1)?,
        mode,
        primary_flow,
        goal,
        metrics,
        repos: Vec::new(),
        direction,
        area,
        paused,
        created_at: Some(created_at),
        workers,
        parent_wave_id,
    })
}

/// SELECT wave_id, repo, worktree, branch, status, iteration,
///        cycle_start_iteration, position
pub fn map_wave_repo_row(row: &impl StoreRow) -> StoreResult<RepoWork> {
    Ok(RepoWork {
        repo: row.text(1)?,
        worktree: row.text(2)?,
        branch: row.text(3)?,
        status: WaveStatus::from_i32(row.int(4)?),
        iteration: row.int(5)? as u32,
        cycle_start_iteration: row.int(6)? as u32,
        position: row.int(7)? as u32,
    })
}

/// SELECT path, repo_id, name, added_at
pub fn map_repo_row(row: &impl StoreRow) -> StoreResult<Repo> {
    Ok(Repo {
        path: row.text(0)?,
        repo_id: RepoId::from_raw(row.text(1)?),
        name: row.text(2)?,
        added_at: unix_to_datetime(row.bigint(3)?),
    })
}

/// SELECT parent_repo_id, child_repo_id
pub fn map_repo_edge_row(row: &impl StoreRow) -> StoreResult<RepoEdge> {
    Ok(RepoEdge {
        parent_repo_id: RepoId::from_raw(row.text(0)?),
        child_repo_id: RepoId::from_raw(row.text(1)?),
    })
}

/// SELECT id, wave_id, iteration, step_index, status, worktree, branch,
///        started_at, ended_at, error, snapshot_repo, snapshot_flow,
///        snapshot_task, snapshot_direction, snapshot_area, snapshot_pr,
///        flow_parents, execution_cursor, parent_run_id,
///        parent_pr_number, stack_position, stack_group_id, stack_status,
///        lineage_inferred, target_branch, repair_of
pub fn map_run_row(row: &impl StoreRow) -> StoreResult<Run> {
    let started_at = unix_to_datetime(row.bigint(7)?);
    let ended_at = row.opt_bigint(8)?;
    let snapshot_direction = parse_json_vec(&row.text(13)?)?;
    let snapshot_area = parse_json_vec(&row.text(14)?)?;
    let snapshot_pr = parse_pr(row.opt_text(15)?)?;
    let flow_parents = parse_json_vec(&row.text(16)?)?;
    let execution_cursor = row.opt_text(17)?;
    let parent_run_id = row.opt_text(18)?.map(LfdId::from_raw);
    let parent_pr_number = row.opt_bigint(19)?.map(|value| value as u32);
    let stack_position = row.int(20)? as u32;
    let stack_group_id = row.text(21)?;
    let stack_status = RunStackStatus::from_i32(row.int(22)?);
    let lineage_inferred = row.int(23)? != 0;
    let target_branch = row.text(24)?;
    let repair_of = row.opt_text(25)?.map(LfdId::from_raw);

    Ok(Run {
        id: LfdId::from_raw(row.text(0)?),
        wave_id: LfdId::from_raw(row.text(1)?),
        repo: row.text(10)?,
        flow: row.text(11)?,
        task: row.opt_text(12)?,
        direction: snapshot_direction,
        area: snapshot_area,
        iteration: row.int(2)? as u32,
        step_index: row.int(3)? as u32,
        status: RunStatus::from_i32(row.int(4)?),
        worktree: row.text(5)?,
        branch: row.text(6)?,
        started_at: Some(started_at),
        ended_at: ended_at.map(unix_to_datetime),
        error: row.opt_text(9)?,
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
pub fn map_live_pr_state_row(row: &impl StoreRow) -> StoreResult<LivePullRequestState> {
    Ok(LivePullRequestState {
        repo_id: row.text(0)?,
        pr_number: row.bigint(1)? as u32,
        state: LivePrState::from_i32(row.int(2)?),
        is_draft: row.int(3)? != 0,
        head_ref: row.text(4)?,
        head_sha: row.text(5)?,
        base_ref: row.text(6)?,
        updated_at: unix_to_datetime(row.bigint(7)?),
        merged_at: row.opt_bigint(8)?.map(unix_to_datetime),
        synced_at: unix_to_datetime(row.bigint(9)?),
    })
}

/// SELECT id, run_id, step_index, branch_index, status, worktree
pub fn map_fork_run_row(row: &impl StoreRow) -> StoreResult<ForkRun> {
    let status = ForkRunStatus::from_i64(row.int(4)? as i64)
        .ok_or_else(|| StoreError::InvalidData("invalid fork run status".to_string()))?;

    Ok(ForkRun {
        id: LfdId::from_raw(row.text(0)?),
        run_id: LfdId::from_raw(row.text(1)?),
        step_index: row.int(2)? as u32,
        branch_index: row.int(3)? as u32,
        status,
        worktree: row.text(5)?,
    })
}

/// SELECT id, wave_id, content, source_hash, token_budget, model, created_at
pub fn map_summary_row(row: &impl StoreRow) -> StoreResult<Summary> {
    Ok(Summary {
        id: LfdId::from_raw(row.text(0)?),
        wave_id: LfdId::from_raw(row.text(1)?),
        content: row.text(2)?,
        source_hash: row.text(3)?,
        token_budget: row.int(4)? as u32,
        agent: row.text(5)?,
        created_at: Some(unix_to_datetime(row.bigint(6)?)),
    })
}

/// SELECT wave_id, name, content, position, updated_at
pub fn map_chat_memory_block_row(row: &impl StoreRow) -> StoreResult<ChatMemoryBlock> {
    Ok(ChatMemoryBlock {
        wave_id: LfdId::from_raw(row.text(0)?),
        name: row.text(1)?,
        content: row.text(2)?,
        position: row.int(3)? as u32,
        updated_at: Some(unix_to_datetime(row.bigint(4)?)),
    })
}

/// SELECT wave, provider, SUM(input_tokens), SUM(output_tokens), SUM(cache_read_tokens)
pub fn map_wave_provider_usage_row(row: &impl StoreRow) -> StoreResult<WaveProviderUsage> {
    Ok(WaveProviderUsage {
        wave: LfdId::from_raw(row.text(0)?),
        provider: row.text(1)?,
        input_tokens: row.bigint(2)?.max(0) as u64,
        output_tokens: row.bigint(3)?.max(0) as u64,
        cache_read_tokens: row.bigint(4)?.max(0) as u64,
    })
}

/// SELECT repo, provider, SUM(input_tokens), SUM(output_tokens), SUM(cache_read_tokens)
pub fn map_repo_provider_usage_row(row: &impl StoreRow) -> StoreResult<RepoProviderUsage> {
    Ok(RepoProviderUsage {
        repo: row.opt_text(0)?,
        provider: row.text(1)?,
        input_tokens: row.bigint(2)?.max(0) as u64,
        output_tokens: row.bigint(3)?.max(0) as u64,
        cache_read_tokens: row.bigint(4)?.max(0) as u64,
    })
}

/// SELECT id, wave_id, role, content, created_at
pub fn map_chat_message_row(row: &impl StoreRow) -> StoreResult<ChatMessage> {
    Ok(ChatMessage {
        id: LfdId::from_raw(row.text(0)?),
        wave_id: LfdId::from_raw(row.text(1)?),
        role: row.text(2)?,
        content: row.text(3)?,
        created_at: unix_to_datetime(row.bigint(4)?),
    })
}
