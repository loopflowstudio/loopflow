//! Rank edits reorder siblings within a Project or the repository's standalone
//! Tasks. A Project's new order is one `order` action: the desired list, which
//! its tracker turns into moves against the tracker's complete list.

use super::planning_write::{self, WriteOrigin};
use super::tracker_actions::{self, ActionKind, ActionOwner};
use crate::work::edit::WorkValue as Edit;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use crate::work::task::Task;
use crate::{StoreError, StoreResult};
use loopflow_model::durable::{ProjectId, TaskId};

pub(super) fn reorder_in(conn: &Connection, task: &Task, rank: u32) -> StoreResult<()> {
    place_in_order(
        conn,
        &task.id,
        task.project_id.as_ref(),
        siblings_in(conn, task)?,
        rank as usize,
    )?;
    Ok(())
}

fn place_in_order(
    conn: &Connection,
    task: &TaskId,
    project: Option<&ProjectId>,
    mut order: Vec<String>,
    position: usize,
) -> StoreResult<Vec<String>> {
    let from = order
        .iter()
        .position(|id| id == task.as_str())
        .ok_or(StoreError::NotFound)?;
    let to = position.min(order.len() - 1);
    if from != to {
        let moved = order.remove(from);
        order.insert(to, moved);
        if let Some(project) = project {
            enqueue_order(conn, project, &order)?;
        }
        apply_order(conn, &order, WriteOrigin::Local)?;
    }
    Ok(order)
}

/// Queue a newly bound Project's local order, which its Tasks may have taken
/// while no tracker held it. One Task has no order to send.
pub(super) fn send_local_order_in(conn: &Connection, project: &ProjectId) -> StoreResult<()> {
    let order = conn
        .prepare(
            "SELECT id FROM tasks WHERE project_id=?1 AND planning_deleted_at IS NULL
             ORDER BY planning_rank,id",
        )?
        .query_map([project.as_str()], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if order.len() > 1 {
        enqueue_order(conn, project, &order)?;
    }
    Ok(())
}

/// Queue `order` as `project`'s desired order, against the tracker's.
fn enqueue_order(conn: &Connection, project: &ProjectId, order: &[String]) -> StoreResult<()> {
    let base = observed_order(conn, project)?.map(|value| json!({"value":value}));
    tracker_actions::enqueue(
        conn,
        &ActionOwner::Project(project.clone()),
        ActionKind::Order,
        None,
        &json!(order),
        base.as_ref(),
    )?;
    Ok(())
}

/// `task` takes `target`'s place among `target`'s Project's Tasks, or among
/// the repository's standalone Tasks, moving into that Project first.
pub(super) fn move_in(conn: &Connection, task: &Task, target: &Task) -> StoreResult<Vec<String>> {
    if task.repository_id != target.repository_id {
        return Err(StoreError::InvalidData(format!(
            "{} and {} are in different repositories; nothing changed.",
            task.identifier, target.identifier
        )));
    }
    let project = target.project_id.as_ref();
    if task.project_id != target.project_id {
        super::task_content::move_task_in(conn, task, task.project_id.as_ref(), project)?;
    }
    let order = siblings_in(conn, target)?;
    let position = order
        .iter()
        .position(|id| id == target.id.as_str())
        .ok_or(StoreError::NotFound)?;
    place_in_order(conn, &task.id, project, order, position)
}

/// The Tasks `task` is ranked among, in rank order. Ties break by id, as
/// `TaskOrder::key` does on every board, so a Move lands where it was dropped.
fn siblings_in(conn: &Connection, task: &Task) -> StoreResult<Vec<String>> {
    let mut query = conn.prepare(
        "SELECT id FROM tasks WHERE repository_id=?1 AND project_id IS ?2
         AND planning_deleted_at IS NULL ORDER BY planning_rank,id",
    )?;
    let rows = query.query_map(
        params![
            task.repository_id.as_str(),
            task.project_id.as_ref().map(ProjectId::as_str)
        ],
        |row| row.get::<_, String>(0),
    )?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}

/// The tracker's order of a Project's Tasks: their observed items by rank.
/// `None` when the Project has no tracker Project.
pub(super) fn observed_order(
    conn: &Connection,
    project: &ProjectId,
) -> StoreResult<Option<Vec<String>>> {
    let bound: Option<(String, String, String)> = conn
        .query_row(
            "SELECT o.repo,o.tracker,o.id FROM projects p JOIN waves w ON w.id=p.wave_id
             JOIN tracker_projects o ON o.id=p.external_project_id AND o.repo=w.repo AND o.tracker IN ('linear','asana')
             WHERE p.id=?1",
            [project.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((repo, tracker, external)) = bound else {
        return Ok(None);
    };
    let mut query = conn.prepare(
        "SELECT t.id FROM tracker_items i JOIN tasks t ON t.external_issue_id=i.id
         WHERE i.repo=?1 AND i.tracker=?2 AND i.project_id=?3
         AND t.project_id=?4 AND t.planning_deleted_at IS NULL
         ORDER BY i.rank,i.id",
    )?;
    let rows = query.query_map(params![repo, tracker, external, project.as_str()], |row| {
        row.get::<_, String>(0)
    })?;
    Ok(Some(rows.collect::<Result<_, _>>()?))
}

/// Keep newly observed members next to their next known neighbour; retain local
/// members awaiting export. Both projection and delivery use this same merge.
pub fn include_members(desired: &[String], observed: &[String]) -> Vec<String> {
    let mut result = desired.to_vec();
    for (position, id) in observed.iter().enumerate() {
        if !result.contains(id) {
            let before = observed[position + 1..]
                .iter()
                .find_map(|next| result.iter().position(|known| known == next))
                .unwrap_or(result.len());
            result.insert(before, id.clone());
        }
    }
    result
}

pub(crate) fn observe_in(
    conn: &Connection,
    project: &ProjectId,
    observed: Vec<String>,
) -> StoreResult<bool> {
    if let Some(previous) = observed_order(conn, project)? {
        // List omission proves neither deletion nor a completed membership move.
        if previous.iter().any(|id| !observed.contains(id)) {
            return Ok(false);
        }
    }
    // The tracker's complete list answers an attempted order the way the
    // drain's look would; the newest unconfirmed order is local intent.
    let mut pending = None;
    for action in tracker_actions::unconfirmed_for_in(conn, &ActionOwner::Project(project.clone()))?
    {
        if action.kind != ActionKind::Order {
            continue;
        }
        if action.phase == tracker_actions::ActionPhase::Sent {
            let seen = crate::planning::rules::Observed {
                order: Some(observed.clone()),
                ..Default::default()
            };
            if let crate::planning::rules::Decision::Resolve(resolution) =
                crate::planning::rules::decide(&action, Some(&seen), false)
            {
                tracker_actions::resolve_in(conn, &action.id, &resolution)?;
                continue;
            }
        }
        pending = Some(action);
    }
    let order = match pending {
        Some(action) => include_members(
            &current_members(conn, project, serde_json::from_value(action.value)?)?,
            &observed,
        ),
        None => observed,
    };
    apply_order(conn, &order, WriteOrigin::Tracker)?;
    Ok(true)
}

fn current_members(
    conn: &Connection,
    project: &ProjectId,
    mut desired: Vec<String>,
) -> StoreResult<Vec<String>> {
    let mut query =
        conn.prepare("SELECT id FROM tasks WHERE project_id=?1 AND planning_deleted_at IS NULL")?;
    let members = query
        .query_map([project.as_str()], |row| row.get(0))?
        .collect::<rusqlite::Result<std::collections::HashSet<String>>>()?;
    desired.retain(|id| members.contains(id));
    Ok(desired)
}

// Both the sibling list and the accepted tracker list select current members
// inside the transaction that writes their ranks.
fn apply_order(conn: &Connection, order: &[String], origin: WriteOrigin) -> StoreResult<()> {
    for (rank, id) in order.iter().enumerate() {
        planning_write::write(
            conn,
            &crate::work::edit::WorkEdit::for_target(id, &[Edit::TaskRank(rank as u32)]),
            origin,
        )?;
    }
    Ok(())
}
