use crate::lfd::id::LfdId;
use crate::lfd::store::{ForkRun, ForkRunStatus, StoreError, StoreResult};
use crate::lfd::types::{
    Agent, AgentStatus, ChatMemoryBlock, ChatMessage, LivePrState, LivePullRequestState,
    PendingActivation, PullRequest, SidecarKind, Stimulus, StimulusKind, Summary, Wave, WaveData,
    WaveRun, WaveRunKind, WaveRunSnapshot, WaveRunStackStatus, WaveRunStatus, WaveStatus,
};
use std::collections::HashMap;

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
///        created_at, schema_ref, schema_name, wave_type, parent_wave_id, position
pub fn map_wave_row(row: &impl StoreRow) -> StoreResult<Wave> {
    let direction = parse_json_vec(&row.text(4)?)?;
    let area = parse_json_vec(&row.text(5)?)?;
    let paused = row.int(6)? != 0;
    let status_value = row.int(7)?;
    let iteration = row.int(8)? as u32;
    let created_at = unix_to_datetime(row.bigint(9)?);
    let mut status = WaveStatus::from_i32(status_value);
    if paused {
        status = WaveStatus::Paused;
    }

    let data = WaveData {
        id: LfdId::from_raw(row.text(0)?),
        name: row.text(1)?,
        repo: row.text(2)?,
        flow: row.text(3)?,
        direction,
        area,
        status,
        iteration,
        schema_ref: row.opt_text(10)?,
        schema_name: row.opt_text(11)?,
        created_at: Some(created_at),
        parent_id: row.opt_text(13)?.map(LfdId::from_raw),
        position: row.int(14)? as u32,
    };
    match row.int(12)? {
        1 => Ok(Wave::Voice(data)),
        2 => Ok(Wave::Chord {
            data,
            children: Vec::new(),
        }),
        value => Err(StoreError::InvalidData(format!(
            "invalid wave_type: {value}"
        ))),
    }
}

#[derive(Debug)]
pub struct WaveTreeRow {
    pub wave: Wave,
    pub depth: u32,
}

/// Assemble a nested Wave tree from flat rows that include root + descendants.
pub fn assemble_wave_tree(rows: Vec<WaveTreeRow>, root_id: &LfdId) -> StoreResult<Wave> {
    #[derive(Debug)]
    struct RawNode {
        wave: Wave,
        parent_id: Option<LfdId>,
        position: u32,
    }

    fn build_wave(
        id: &LfdId,
        nodes: &mut HashMap<LfdId, RawNode>,
        children_by_parent: &HashMap<LfdId, Vec<LfdId>>,
    ) -> StoreResult<Wave> {
        let mut node = nodes
            .remove(id)
            .ok_or_else(|| StoreError::InvalidData(format!("missing wave in tree: {id}")))?;
        let child_ids = children_by_parent.get(id).cloned().unwrap_or_default();
        if let Wave::Chord { children, .. } = &mut node.wave {
            let mut built_children = Vec::with_capacity(child_ids.len());
            for child_id in child_ids {
                built_children.push(build_wave(&child_id, nodes, children_by_parent)?);
            }
            *children = built_children;
        } else if !child_ids.is_empty() {
            return Err(StoreError::InvalidData(format!(
                "voice wave cannot have children: {id}"
            )));
        }
        Ok(node.wave)
    }

    if rows.is_empty() {
        return Err(StoreError::NotFound);
    }

    let mut nodes: HashMap<LfdId, RawNode> = HashMap::new();
    for row in rows {
        let wave_id = row.wave.id().clone();
        nodes.insert(
            wave_id,
            RawNode {
                parent_id: row.wave.parent_id().clone(),
                position: row.wave.position(),
                wave: row.wave,
            },
        );
    }

    if !nodes.contains_key(root_id) {
        return Err(StoreError::NotFound);
    }

    let mut children_by_parent: HashMap<LfdId, Vec<(u32, LfdId)>> = HashMap::new();
    for (id, node) in &nodes {
        if let Some(parent_id) = &node.parent_id {
            children_by_parent
                .entry(parent_id.clone())
                .or_default()
                .push((node.position, id.clone()));
        }
    }

    let children_by_parent: HashMap<LfdId, Vec<LfdId>> = children_by_parent
        .into_iter()
        .map(|(parent_id, mut children)| {
            children.sort_by_key(|(position, _)| *position);
            (
                parent_id,
                children.into_iter().map(|(_, child_id)| child_id).collect(),
            )
        })
        .collect();

    build_wave(root_id, &mut nodes, &children_by_parent)
}

/// SELECT id, wave_id, iteration, step_index, status, worktree, branch,
///        started_at, ended_at, error, snapshot_repo, snapshot_flow,
///        snapshot_direction, snapshot_area, snapshot_pr, flow_parents,
///        run_kind, sidecar_kind, parent_run_id, parent_pr_number,
///        stack_position, stack_group_id, stack_status, lineage_inferred
pub fn map_wave_run_row(row: &impl StoreRow) -> StoreResult<WaveRun> {
    let started_at = unix_to_datetime(row.bigint(7)?);
    let ended_at = row.opt_bigint(8)?;
    let snapshot_direction = parse_json_vec(&row.text(12)?)?;
    let snapshot_area = parse_json_vec(&row.text(13)?)?;
    let snapshot_pr = parse_pr(row.opt_text(14)?)?;
    let flow_parents = parse_json_vec(&row.text(15)?)?;
    let run_kind = WaveRunKind::from_i32(row.int(16)?);
    let sidecar_kind = row.opt_int(17)?.and_then(SidecarKind::from_i32);
    let parent_run_id = row.opt_text(18)?.map(LfdId::from_raw);
    let parent_pr_number = row.opt_bigint(19)?.map(|value| value as u32);
    let stack_position = row.int(20)? as u32;
    let stack_group_id = row.text(21)?;
    let stack_status = WaveRunStackStatus::from_i32(row.int(22)?);
    let lineage_inferred = row.int(23)? != 0;

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
        run_kind,
        sidecar_kind,
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

/// SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, created_at, enabled
pub fn map_stimulus_row(row: &impl StoreRow) -> StoreResult<Stimulus> {
    let created_at = unix_to_datetime(row.bigint(6)?);

    Ok(Stimulus {
        id: LfdId::from_raw(row.text(0)?),
        wave_id: LfdId::from_raw(row.text(1)?),
        kind: StimulusKind::from_i32(row.int(2)?),
        cron: row.text(3)?,
        last_main_sha: row.opt_text(4)?,
        last_triggered_at: row.opt_bigint(5)?,
        created_at: Some(created_at),
        enabled: row.int(7)? != 0,
    })
}

/// SELECT id, wave_id, stimulus_id, from_sha, to_sha, queued_at
pub fn map_pending_activation_row(row: &impl StoreRow) -> StoreResult<PendingActivation> {
    Ok(PendingActivation {
        id: LfdId::from_raw(row.text(0)?),
        wave_id: LfdId::from_raw(row.text(1)?),
        stimulus_id: LfdId::from_raw(row.text(2)?),
        from_sha: row.text(3)?,
        to_sha: row.text(4)?,
        queued_at: row.bigint(5)?,
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
pub fn map_agent_row(row: &impl StoreRow) -> StoreResult<Agent> {
    let started_at = unix_to_datetime(row.bigint(6)?);
    let ended_at = row.opt_bigint(7)?;
    let pid = row.opt_int(8)?;
    let container_id = row.opt_text(9)?;
    let wave_run_id = row.opt_text(4)?;

    Ok(Agent {
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
        model: row.text(10)?,
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
        model: row.text(5)?,
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
