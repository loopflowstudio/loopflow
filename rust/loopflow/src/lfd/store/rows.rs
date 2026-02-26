use crate::lfd::id::LfdId;
use crate::lfd::store::{ForkRun, ForkRunStatus, StoreError, StoreResult};
use crate::lfd::types::{
    ActivationLog, ActivationOutcome, ActivationSource, AgentRun, AgentStatus, ChatMemoryBlock,
    ChatMessage, Chord, LivePrState, LivePullRequestState, PendingActivation, PullRequest, Signal,
    Stimulus, Summary, Wave, WaveRun, WaveRunSnapshot, WaveRunStackStatus, WaveRunStatus,
    WaveStatus,
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

/// SELECT id, name, repo, flow, direction, area, paused, status, iteration,
///        created_at, serialized
pub fn map_wave_row(row: &impl StoreRow) -> StoreResult<Wave> {
    let direction = parse_json_vec(&row.text(4)?)?;
    let area = parse_json_vec(&row.text(5)?)?;
    let paused = row.int(6)? != 0;
    let status_value = row.int(7)?;
    let iteration = row.int(8)? as u32;
    let created_at = unix_to_datetime(row.bigint(9)?);
    let serialized = row.int(10)? != 0;
    let mut status = WaveStatus::from_i32(status_value);
    if paused {
        status = WaveStatus::Paused;
    }

    Ok(Wave {
        id: LfdId::from_raw(row.text(0)?),
        name: row.text(1)?,
        repo: row.text(2)?,
        flow: row.text(3)?,
        direction,
        area,
        status,
        iteration,
        created_at: Some(created_at),
        serialized,
    })
}

/// SELECT id, name, is_default, created_at
pub fn map_chord_row(row: &impl StoreRow) -> StoreResult<Chord> {
    Ok(Chord {
        id: LfdId::from_raw(row.text(0)?),
        name: row.text(1)?,
        is_default: row.int(2)? != 0,
        created_at: Some(unix_to_datetime(row.bigint(3)?)),
    })
}

/// SELECT id, wave_id, iteration, step_index, status, worktree, branch,
///        started_at, ended_at, error, snapshot_repo, snapshot_flow,
///        snapshot_direction, snapshot_area, snapshot_pr, flow_parents,
///        activation_log_id, parent_run_id, parent_pr_number,
///        stack_position, stack_group_id, stack_status,
///        lineage_inferred
pub fn map_wave_run_row(row: &impl StoreRow) -> StoreResult<WaveRun> {
    let started_at = unix_to_datetime(row.bigint(7)?);
    let ended_at = row.opt_bigint(8)?;
    let snapshot_direction = parse_json_vec(&row.text(12)?)?;
    let snapshot_area = parse_json_vec(&row.text(13)?)?;
    let snapshot_pr = parse_pr(row.opt_text(14)?)?;
    let flow_parents = parse_json_vec(&row.text(15)?)?;
    let activation_log_id = row.opt_text(16)?.map(LfdId::from_raw);
    let parent_run_id = row.opt_text(17)?.map(LfdId::from_raw);
    let parent_pr_number = row.opt_bigint(18)?.map(|value| value as u32);
    let stack_position = row.int(19)? as u32;
    let stack_group_id = row.text(20)?;
    let stack_status = WaveRunStackStatus::from_i32(row.int(21)?);
    let lineage_inferred = row.int(22)? != 0;

    let snapshot = WaveRunSnapshot {
        repo: row.text(10)?,
        flow: row.text(11)?,
        direction: snapshot_direction,
        area: snapshot_area,
        pr: snapshot_pr,
    };

    Ok(WaveRun {
        id: LfdId::from_raw(row.text(0)?),
        wave_id: LfdId::from_raw(row.text(1)?),
        snapshot,
        iteration: row.int(2)? as u32,
        step_index: row.int(3)? as u32,
        status: WaveRunStatus::from_i32(row.int(4)?),
        worktree: row.text(5)?,
        branch: row.text(6)?,
        started_at: Some(started_at),
        ended_at: ended_at.map(unix_to_datetime),
        error: row.opt_text(9)?,
        flow_parents,
        activation_log_id,
        parent_run_id,
        parent_pr_number,
        stack_position,
        stack_group_id,
        stack_status,
        lineage_inferred,
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

/// SELECT id, wave_id, signal, flow, cron, last_main_sha, last_triggered_at, created_at,
///        enabled, source_wave_id
pub fn map_stimulus_row(row: &impl StoreRow) -> StoreResult<Stimulus> {
    let created_at = unix_to_datetime(row.bigint(7)?);

    Ok(Stimulus {
        id: LfdId::from_raw(row.text(0)?),
        wave_id: LfdId::from_raw(row.text(1)?),
        signal: Signal::from_i32(row.int(2)?),
        flow: row.opt_text(3)?,
        cron: row.opt_text(4)?,
        last_main_sha: row.opt_text(5)?,
        last_triggered_at: row.opt_bigint(6)?,
        created_at: Some(created_at),
        enabled: row.int(8)? != 0,
        source_wave_id: row.opt_text(9)?.map(LfdId::from_raw),
    })
}

/// SELECT id, wave_id, stimulus_id, source, reason, from_sha, to_sha, queued_at
pub fn map_pending_activation_row(row: &impl StoreRow) -> StoreResult<PendingActivation> {
    Ok(PendingActivation {
        id: LfdId::from_raw(row.text(0)?),
        wave_id: LfdId::from_raw(row.text(1)?),
        stimulus_id: LfdId::from_raw(row.text(2)?),
        source: ActivationSource::from_i32(row.int(3)?),
        reason: row.text(4)?,
        from_sha: row.text(5)?,
        to_sha: row.text(6)?,
        queued_at: row.bigint(7)?,
    })
}

/// SELECT id, wave_id, stimulus_id, source, reason, outcome, created_at
pub fn map_activation_log_row(row: &impl StoreRow) -> StoreResult<ActivationLog> {
    let outcome = ActivationOutcome::parse(&row.text(5)?)
        .ok_or_else(|| StoreError::InvalidData("invalid activation log outcome".to_string()))?;

    Ok(ActivationLog {
        id: LfdId::from_raw(row.text(0)?),
        wave_id: LfdId::from_raw(row.text(1)?),
        stimulus_id: LfdId::from_raw(row.text(2)?),
        source: ActivationSource::from_i32(row.int(3)?),
        reason: row.text(4)?,
        outcome,
        created_at: row.bigint(6)?,
    })
}

/// SELECT id, wave_run_id, step_index, branch_index, status, worktree
pub fn map_fork_run_row(row: &impl StoreRow) -> StoreResult<ForkRun> {
    let status = ForkRunStatus::from_i64(row.int(4)? as i64)
        .ok_or_else(|| StoreError::InvalidData("invalid fork run status".to_string()))?;

    Ok(ForkRun {
        id: LfdId::from_raw(row.text(0)?),
        wave_run_id: LfdId::from_raw(row.text(1)?),
        step_index: row.int(2)? as u32,
        branch_index: row.int(3)? as u32,
        status,
        worktree: row.text(5)?,
    })
}

/// SELECT id, step, repo, worktree, wave_run_id, status,
///        started_at, ended_at, pid, container_id, model, run_mode
pub fn map_agent_row(row: &impl StoreRow) -> StoreResult<AgentRun> {
    let started_at = unix_to_datetime(row.bigint(6)?);
    let ended_at = row.opt_bigint(7)?;
    let pid = row.opt_int(8)?;
    let container_id = row.opt_text(9)?;
    let wave_run_id = row.opt_text(4)?;

    Ok(AgentRun {
        id: LfdId::from_raw(row.text(0)?),
        step: row.text(1)?,
        repo: row.text(2)?,
        worktree: row.text(3)?,
        wave_run_id: wave_run_id.map(LfdId::from_raw),
        status: AgentStatus::from_i32(row.int(5)?),
        started_at: Some(started_at),
        ended_at: ended_at.map(unix_to_datetime),
        pid: pid.map(|v| v as u32),
        container_id,
        agent: row.text(10)?,
        run_mode: row.text(11)?,
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
