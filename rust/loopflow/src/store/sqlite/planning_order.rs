//! A reorder is one Project intention. Its individual provider effects retain
//! before/after lists so acquisition can recognize our own partial progress.

use super::planning_write::{self, PlanningEdit as Edit};
use crate::engine::planning_exchange::PlanningKind;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::durable::{ProjectId, TaskId};
use crate::pm::PmItem;
use crate::store::StoreResult;

use super::planning_changes::PlanningChanges;
use super::SqliteStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OrderEffect {
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub issue: String,
    pub input: Value,
    pub settled: bool,
}

#[derive(Debug)]
pub(crate) struct OrderDelivery {
    pub id: String,
    pub baseline: Option<Vec<String>>,
    pub effects: Vec<OrderEffect>,
    pub desired: Vec<String>,
}

impl SqliteStore {
    pub(crate) fn project_order_delivery(
        &self,
        project: &ProjectId,
    ) -> StoreResult<Option<OrderDelivery>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(deliveries(&conn, project)?.into_iter().next())
    }

    /// Capture exactly one move before issuing it. Local edits and acquisition
    /// use the SQLite transaction, never the provider-effect lock.
    pub(crate) fn attempt_project_order(
        &self,
        project: &ProjectId,
        receipt: &str,
        effect: &OrderEffect,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let Some(mut delivery) = deliveries(&tx, project)?.into_iter().next() else {
            return Ok(false);
        };
        if delivery.id != receipt || delivery.effects.last().is_some_and(|e| !e.settled) {
            return Ok(false);
        }
        let observed = observed_order(&tx, project)?;
        if observed.as_ref() != Some(&effect.before) {
            return Ok(false);
        }
        // A membership save/deletion can race the read without changing the
        // provider observation. Never send an order across that local change.
        for id in &effect.before {
            let available: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND project_id=?2 AND planning_deleted_at IS NULL)",
                params![id, project.as_str()], |row| row.get(0))?;
            if !available {
                return Ok(false);
            }
        }
        delivery.effects.push(effect.clone());
        tx.execute(
            "UPDATE project_changes SET attempted=1,order_effects_json=?2,error=NULL WHERE id=?1",
            params![receipt, serde_json::to_string(&delivery.effects)?],
        )?;
        tx.commit()?;
        Ok(true)
    }
}

pub(super) fn reorder_in(
    conn: &Connection,
    project: &ProjectId,
    task: &TaskId,
    rank: u32,
) -> StoreResult<()> {
    let previous = project_members(conn, project)?;
    let mut desired = previous.clone();
    desired.retain(|id| id != task.as_str());
    desired.insert((rank as usize).min(desired.len()), task.to_string());
    if PlanningChanges::Project(project).record(
        conn,
        "task_order",
        json!(previous),
        json!(desired),
    )? {
        apply_order(conn, project, &desired)?;
    }
    Ok(())
}

pub(super) fn observed_order(
    conn: &Connection,
    project: &ProjectId,
) -> StoreResult<Option<Vec<String>>> {
    let body: Option<String> = conn.query_row(
        "SELECT o.task_order_json FROM projects p JOIN waves w ON w.id=p.wave_id
         JOIN pm_projects o ON o.id=p.external_project_id AND o.repo=w.repo AND o.provider='linear' WHERE p.id=?1",
        [project.as_str()], |row| row.get(0)).optional()?.flatten();
    body.map(|body| serde_json::from_str(&body).map_err(Into::into))
        .transpose()
}

fn deliveries(conn: &Connection, project: &ProjectId) -> StoreResult<Vec<OrderDelivery>> {
    let members = project_members(conn, project)?;
    let mut query = conn.prepare("SELECT id,value_json,json_extract(base_json,'$.value'),order_effects_json FROM project_changes
        WHERE project_id=?1 AND field='task_order' AND acknowledged=0 AND conflict_json IS NULL
        AND (attempted=1 OR seq=(SELECT max(seq) FROM project_changes WHERE project_id=?1 AND field='task_order')) ORDER BY seq")?;
    let rows = query.query_and_then([project.as_str()], |row| -> StoreResult<_> {
        let value: String = row.get(1)?;
        let base: Option<String> = row.get(2)?;
        let effects: String = row.get(3)?;
        Ok(OrderDelivery {
            id: row.get(0)?,
            baseline: base.map(|v| serde_json::from_str(&v)).transpose()?,
            effects: serde_json::from_str(&effects)?,
            desired: serde_json::from_str::<Vec<String>>(&value)?
                .into_iter()
                .filter(|id| members.contains(id))
                .collect(),
        })
    })?;
    rows.collect()
}

// Compare relative order among shared members. A new member does not conflict
// with a saved move; removal is handled separately as incomplete evidence.
fn same_order(left: &[String], right: &[String]) -> bool {
    left.iter()
        .filter(|id| right.contains(id))
        .eq(right.iter().filter(|id| left.contains(id)))
}

/// Keep newly observed members next to their next known neighbour; retain local
/// members awaiting export. Both projection and delivery use this same merge.
pub(crate) fn include_members(desired: &[String], observed: &[String]) -> Vec<String> {
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

pub(super) fn observe_in(
    conn: &Connection,
    repo: &str,
    provider: &str,
    external: &str,
    items: &[PmItem],
) -> StoreResult<bool> {
    if provider != "linear" {
        return Ok(false);
    }
    let project: Option<String> = conn.query_row("SELECT p.id FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.external_project_id=?1 AND w.repo=?2", params![external,repo], |row| row.get(0)).optional()?;
    let Some(project) = project.map(ProjectId::from_raw) else {
        return Ok(false);
    };
    let mut items = items.iter().collect::<Vec<_>>();
    items.sort_by_key(|i| i.rank);
    let mut observed = Vec::new();
    for item in items {
        // An older list must not order a mixture of old and retained newer facts.
        let retained: Option<(String, String)> = conn.query_row(
            "SELECT t.id,i.body FROM pm_items i JOIN tasks t ON t.external_issue_id=i.id
             WHERE i.repo=?1 AND i.provider=?2 AND i.id=?3 AND i.needs_refresh=0 AND t.project_id=?4 AND t.planning_deleted_at IS NULL",
            params![repo,provider,item.id,project.as_str()], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
        let Some((id, body)) = retained else {
            return Ok(false);
        };
        let retained: PmItem = serde_json::from_str(&body)?;
        if retained.revision != item.revision
            || retained.rank != item.rank
            || retained.project_id != item.project_id
        {
            return Ok(false);
        }
        observed.push(id);
    }
    if let Some(previous) = observed_order(conn, &project)? {
        // List omission proves neither deletion nor a completed membership move.
        for id in previous.iter().filter(|id| !observed.contains(id)) {
            let retained: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND project_id=?2 AND planning_deleted_at IS NULL)", params![id,project.as_str()], |row| row.get(0))?;
            if retained {
                return Ok(false);
            }
        }
    }
    conn.execute(
        "UPDATE pm_projects SET task_order_json=?4 WHERE repo=?1 AND provider=?2 AND id=?3",
        params![repo, provider, external, serde_json::to_string(&observed)?],
    )?;
    let observation = json!({"value":observed}).to_string();
    let mut pending = deliveries(conn, &project)?;
    let mut remaining = pending.as_mut_slice();
    while let Some((delivery, later)) = remaining.split_first_mut() {
        remaining = later;
        if let Some(last) = delivery.effects.last_mut() {
            if !last.settled && same_order(&observed, &last.after) {
                last.settled = true;
                // A later local move saved during this effect keeps its identity
                // and value, but now compares against the confirmed progress.
                for later in remaining.iter_mut() {
                    if later.effects.is_empty()
                        && later
                            .baseline
                            .as_ref()
                            .is_none_or(|base| same_order(base, &last.before))
                    {
                        conn.execute(
                            "UPDATE project_changes SET base_json=?2 WHERE id=?1",
                            params![later.id, observation],
                        )?;
                        later.baseline = Some(observed.clone());
                    }
                }
                conn.execute(
                    "UPDATE project_changes SET order_effects_json=?2,error=NULL WHERE id=?1",
                    params![delivery.id, serde_json::to_string(&delivery.effects)?],
                )?;
            }
        }
        let last = delivery.effects.last();
        let expected = last
            .map(|effect| {
                if effect.settled {
                    &effect.after
                } else {
                    &effect.before
                }
            })
            .or(delivery.baseline.as_ref());
        if expected.is_some_and(|base| !same_order(base, &observed)) {
            conn.execute(
                "UPDATE project_changes SET conflict_json=?2,error=NULL WHERE id=?1",
                params![delivery.id, observation],
            )?;
        } else if last.is_none_or(|effect| effect.settled)
            && delivery.desired.iter().all(|id| observed.contains(id))
            && same_order(&delivery.desired, &observed)
        {
            conn.execute(
                "UPDATE project_changes SET acknowledged=1,error=NULL WHERE id=?1",
                [&delivery.id],
            )?;
        } else if delivery.baseline.is_none() {
            conn.execute(
                "UPDATE project_changes SET base_json=?2 WHERE id=?1",
                params![delivery.id, observation],
            )?;
        }
    }
    // Only the latest pending intention projects locally; older attempts retain
    // delivery evidence without reviving a superseded local order.
    let pending = PlanningChanges::Project(&project)
        .pending(conn)?
        .into_iter()
        .find(|c| c.field == "task_order");
    let order = if let Some(change) = pending {
        include_members(
            &current_members(
                conn,
                &project,
                &serde_json::from_value::<Vec<String>>(change.value)?,
            )?,
            &observed,
        )
    } else {
        observed
    };
    apply_order(conn, &project, &order)?;
    Ok(true)
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

fn apply_order(conn: &Connection, project: &ProjectId, order: &[String]) -> StoreResult<()> {
    for (rank, id) in order.iter().enumerate() {
        let member:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND project_id=?2 AND planning_deleted_at IS NULL)",params![id,project.as_str()],|r|r.get(0))?;
        if member {
            planning_write::local(conn, PlanningKind::Task, id, &[Edit::TaskRank(rank as u32)])?;
        }
    }
    Ok(())
}
