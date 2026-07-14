use crate::lfd::id::LfdId;
use crate::lfd::types::{ChatMemoryBlock, ChatMessage, Repo, RepoEdge, RepoId, Summary, Wave};
use crate::lfdb::StoreResult;

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

// -- Shared utilities --------------------------------------------------------

pub fn unix_to_datetime(seconds: i64) -> time::OffsetDateTime {
    time::OffsetDateTime::from_unix_timestamp(seconds).unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
}

pub fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

// -- Shared row mappers ------------------------------------------------------

/// SELECT id, name, direction, area, paused, created_at, workers,
///        goal, metrics, parent_wave_id,
///        repo, legacy_worktree, legacy_branch, status, iteration,
///        cycle_start_iteration
pub fn map_wave_row(row: &rusqlite::Row<'_>) -> StoreResult<Wave> {
    let created_at = unix_to_datetime(bigint(row, 5)?);
    let task_capacity = int(row, 6)? as u32;
    let parent_wave_id = opt_text(row, 9)?.map(LfdId::from_raw);

    Ok(Wave {
        id: LfdId::from_raw(text(row, 0)?),
        name: text(row, 1)?,
        repo: text(row, 10)?,
        created_at: Some(created_at),
        task_capacity,
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
