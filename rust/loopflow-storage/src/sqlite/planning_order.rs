//! Rank edits reorder siblings within a Project or the repository's standalone
//! Tasks. A Project's new order is one `order` action: the desired list, which
//! its tracker turns into moves against the tracker's complete list.

use super::planning_write::{self, WriteOrigin};
use super::tracker_actions::{self, ActionKind, ActionOwner};
use crate::work::edit::WorkValue as Edit;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use crate::work::task::Task;
use crate::StoreResult;
use loopflow_model::durable::ProjectId;

pub(super) fn reorder_in(conn: &Connection, task: &Task, rank: u32) -> StoreResult<()> {
    let project = task.project_id.as_ref();
    let mut query = conn.prepare(
        "SELECT id FROM tasks WHERE repository_id=?1 AND project_id IS ?2
         AND planning_deleted_at IS NULL ORDER BY planning_rank,created_at,id",
    )?;
    let previous = query
        .query_map(
            params![task.repository_id.as_str(), project.map(ProjectId::as_str)],
            |row| row.get::<_, String>(0),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let mut desired = previous.clone();
    desired.retain(|id| id != task.id.as_str());
    desired.insert((rank as usize).min(desired.len()), task.id.to_string());
    if previous != desired {
        if let Some(project) = project {
            let base = observed_order(conn, project)?.map(|value| json!({"value":value}));
            tracker_actions::enqueue(
                conn,
                &ActionOwner::Project(project.clone()),
                ActionKind::Order,
                None,
                &json!(desired),
                base.as_ref(),
            )?;
        }
        apply_order(conn, project, &desired, WriteOrigin::Local)?;
    }
    Ok(())
}

pub(super) fn observed_order(
    conn: &Connection,
    project: &ProjectId,
) -> StoreResult<Option<Vec<String>>> {
    let body: Option<String> = conn.query_row(
        "SELECT o.task_order_json FROM projects p JOIN waves w ON w.id=p.wave_id
         JOIN tracker_projects o ON o.id=p.external_project_id AND o.repo=w.repo AND o.tracker IN ('linear','asana') WHERE p.id=?1",
        [project.as_str()], |row| row.get(0)).optional()?.flatten();
    body.map(|body| serde_json::from_str(&body).map_err(Into::into))
        .transpose()
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
    repo: &str,
    external: &str,
    project: &ProjectId,
    observed: Vec<String>,
) -> StoreResult<bool> {
    if let Some(previous) = observed_order(conn, project)? {
        // List omission proves neither deletion nor a completed membership move.
        for id in previous.iter().filter(|id| !observed.contains(id)) {
            let retained: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND project_id=?2 AND planning_deleted_at IS NULL)", params![id,project.as_str()], |row| row.get(0))?;
            if retained {
                return Ok(false);
            }
        }
    }
    conn.execute(
        "UPDATE tracker_projects SET task_order_json=?4 WHERE repo=?1 AND tracker=?2 AND id=?3",
        params![repo, "linear", external, serde_json::to_string(&observed)?],
    )?;
    // The tracker's complete list answers an attempted order the way the
    // drain's look would; the newest unconfirmed order is local intent.
    for action in unconfirmed_orders(conn, project)? {
        if action.phase == tracker_actions::ActionPhase::Sent {
            let seen = crate::planning::rules::Observed {
                order: Some(observed.clone()),
                ..Default::default()
            };
            if let crate::planning::rules::Decision::Resolve(resolution) =
                crate::planning::rules::decide(&action, Some(&seen), false)
            {
                tracker_actions::resolve_in(conn, &action.id, &resolution)?;
            }
        }
    }
    let pending = unconfirmed_orders(conn, project)?.pop();
    let order = match pending {
        Some(action) => include_members(
            &current_members(
                conn,
                project,
                &serde_json::from_value::<Vec<String>>(action.value)?,
            )?,
            &observed,
        ),
        None => observed,
    };
    apply_order(conn, Some(project), &order, WriteOrigin::Tracker)?;
    Ok(true)
}

fn unconfirmed_orders(
    conn: &Connection,
    project: &ProjectId,
) -> StoreResult<Vec<tracker_actions::TrackerAction>> {
    Ok(
        tracker_actions::unconfirmed_for_in(conn, &ActionOwner::Project(project.clone()))?
            .into_iter()
            .filter(|action| action.kind == ActionKind::Order)
            .collect(),
    )
}

fn project_members(conn: &Connection, project: &ProjectId) -> StoreResult<Vec<String>> {
    let mut query = conn.prepare(
        "SELECT id FROM tasks WHERE project_id=?1 AND planning_deleted_at IS NULL
         ORDER BY planning_rank,created_at,id",
    )?;
    let rows = query.query_map([project.as_str()], |row| row.get(0))?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}

fn current_members(
    conn: &Connection,
    project: &ProjectId,
    desired: &[String],
) -> StoreResult<Vec<String>> {
    let members = project_members(conn, project)?;
    Ok(desired
        .iter()
        .filter(|id| members.contains(id))
        .cloned()
        .collect())
}

fn apply_order(
    conn: &Connection,
    project: Option<&ProjectId>,
    order: &[String],
    origin: WriteOrigin,
) -> StoreResult<()> {
    for (rank, id) in order.iter().enumerate() {
        let member: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND project_id IS ?2 AND planning_deleted_at IS NULL)",
            params![id, project.map(ProjectId::as_str)], |row| row.get(0),
        )?;
        if member {
            planning_write::write(
                conn,
                &crate::work::edit::WorkEdit::for_target(id, &[Edit::TaskRank(rank as u32)]),
                origin,
            )?;
        }
    }
    Ok(())
}
