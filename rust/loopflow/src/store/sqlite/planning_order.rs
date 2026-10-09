//! A reorder is one Project intention. Its individual provider effects retain
//! before/after lists so acquisition can recognize our own partial progress.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::durable::{ProjectId, TaskId};
use crate::pm::PmItem;
use crate::store::{StoreError, StoreResult};

use super::planning_changes::PlanningChanges;
use super::SqliteStore;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
        next_delivery(&conn, project)
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
        super::planning_peers::require_projected_effects(&tx, PlanningChanges::Project(project))?;
        let Some(mut delivery) = next_delivery(&tx, project)? else {
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
        WHERE project_id=?1 AND id IN (SELECT id FROM planning_order_deliveries) ORDER BY id")?;
    let rows = query.query_and_then([project.as_str()], |row| -> StoreResult<_> {
        let value: String = row.get(1)?;
        let base: Option<String> = row.get(2)?;
        let effects: String = row.get(3)?;
        let mut desired = Vec::new();
        for id in resolved_order(conn, &serde_json::from_str::<Vec<String>>(&value)?)? {
            let known: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1)",
                [&id],
                |r| r.get(0),
            )?;
            // An unprojected alias is not evidence of a removed member. Retain
            // it so delivery waits for correspondence rather than dropping it.
            if members.contains(&id) || !known {
                desired.push(id);
            }
        }
        Ok(OrderDelivery {
            id: row.get(0)?,
            baseline: base.map(|v| serde_json::from_str(&v)).transpose()?,
            effects: serde_json::from_str(&effects)?,
            desired,
        })
    })?;
    rows.collect()
}

// Resolve only for projection/comparison. Persisted effect inputs, desired
// lists and receipt histories retain the IDs captured by their original writer.
fn resolved_order(conn: &Connection, order: &[String]) -> StoreResult<Vec<String>> {
    let mut resolved = Vec::with_capacity(order.len());
    for id in order {
        let id = super::planning_peers::associated_local_id(
            conn,
            crate::engine::planning_exchange::PlanningKind::Task,
            id,
            None,
        )?
        .unwrap_or_else(|| id.clone());
        if !resolved.contains(&id) {
            resolved.push(id);
        }
    }
    Ok(resolved)
}

fn same_projected_order(conn: &Connection, left: &[String], right: &[String]) -> StoreResult<bool> {
    Ok(same_order(
        &resolved_order(conn, left)?,
        &resolved_order(conn, right)?,
    ))
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
    let previous = observed_order(conn, &project)?;
    let pending = deliveries(conn, &project)?;
    for id in pending.iter().flat_map(|delivery| &delivery.desired) {
        let known: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1)",
            [id],
            |r| r.get(0),
        )?;
        if !known {
            return Ok(false);
        }
    }
    let mut retained_lists = previous.iter().collect::<Vec<_>>();
    for delivery in &pending {
        retained_lists.extend(&delivery.baseline);
        if let Some(effect) = delivery.effects.last() {
            retained_lists.extend([&effect.before, &effect.after]);
        }
    }
    // A cold peer may have captured attempts but no local list inventory. Their
    // known members still prevent an empty/partial read from settling the move.
    let mut membership = conn
        .prepare("SELECT project_id=?2 AND planning_deleted_at IS NULL FROM tasks WHERE id=?1")?;
    let retained = retained_lists
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    for id in resolved_order(conn, &retained)?
        .iter()
        .filter(|id| !observed.contains(id))
    {
        let retained: Option<bool> = membership
            .query_row(params![id, project.as_str()], |row| row.get(0))
            .optional()?;
        if retained != Some(false) {
            return Ok(false);
        }
    }
    conn.execute(
        "UPDATE pm_projects SET task_order_json=?4 WHERE repo=?1 AND provider=?2 AND id=?3",
        params![repo, provider, external, serde_json::to_string(&observed)?],
    )?;
    let observation = json!({"value":observed}).to_string();
    for mut delivery in pending {
        if let Some(last) = delivery.effects.last_mut() {
            if !last.settled && same_projected_order(conn, &observed, &last.after)? {
                last.settled = true;
                // Every unattempted save against this baseline retains its value.
                // Arrival sequence on another machine cannot order these intentions.
                for later in deliveries(conn, &project)? {
                    if later.effects.is_empty()
                        && later
                            .baseline
                            .as_ref()
                            .map(|base| same_projected_order(conn, base, &last.before))
                            .transpose()?
                            .unwrap_or(true)
                    {
                        conn.execute(
                            "UPDATE project_changes SET base_json=?2 WHERE id=?1",
                            params![later.id, observation],
                        )?;
                    }
                }
                conn.execute(
                    "UPDATE project_changes SET order_effects_json=?2,error=NULL WHERE id=?1",
                    params![delivery.id, serde_json::to_string(&delivery.effects)?],
                )?;
            }
        }
    }
    for delivery in deliveries(conn, &project)? {
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
        if expected
            .map(|base| same_projected_order(conn, base, &observed))
            .transpose()?
            == Some(false)
        {
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
    if !project_pending(conn, &project)? {
        apply_order(conn, &project, &observed)?;
    }
    Ok(true)
}

/// Ranks render the selected intention; they never acknowledge a move.
pub(super) fn project_pending(conn: &Connection, project: &ProjectId) -> StoreResult<bool> {
    let pending = PlanningChanges::Project(project)
        .pending(conn)?
        .into_iter()
        .find(|c| c.field == "task_order");
    let Some(change) = pending else {
        return Ok(false);
    };
    let members = project_members(conn, project)?;
    let desired = resolved_order(conn, &serde_json::from_value::<Vec<String>>(change.value)?)?
        .into_iter()
        .filter(|id| members.contains(id))
        .collect::<Vec<_>>();
    apply_order(conn, project, &include_members(&desired, &members))?;
    Ok(true)
}

fn next_delivery(conn: &Connection, project: &ProjectId) -> StoreResult<Option<OrderDelivery>> {
    // The view contains only the selected save and unresolved effects. Any
    // uncertain effect, including a losing order, takes precedence over the save.
    Ok(deliveries(conn, project)?
        .into_iter()
        .min_by_key(|delivery| delivery.effects.last().is_none_or(|effect| effect.settled)))
}

fn project_members(conn: &Connection, project: &ProjectId) -> StoreResult<Vec<String>> {
    let mut query = conn.prepare(
        "SELECT id FROM tasks WHERE project_id=?1 AND planning_deleted_at IS NULL
         ORDER BY planning_rank,created_at,id",
    )?;
    let rows = query.query_map([project.as_str()], |row| row.get(0))?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}

fn apply_order(conn: &Connection, project: &ProjectId, order: &[String]) -> StoreResult<()> {
    for (rank, id) in order.iter().enumerate() {
        conn.execute(
            "UPDATE tasks SET planning_rank=?2,planning_revision=planning_revision+1
            WHERE id=?1 AND project_id=?3 AND planning_rank!=?2 AND planning_deleted_at IS NULL",
            params![id, rank as u32, project.as_str()],
        )?;
    }
    Ok(())
}

/// The common receipt is the transport unit. No list observation is synthesized
/// from an imported rank, baseline, settlement or checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OrderReceipt {
    desired: Vec<String>,
    base: Option<Value>,
    effects: Vec<OrderEffect>,
    attempted: bool,
    acknowledged: bool,
    conflict: Option<Value>,
    error: Option<String>,
}

pub(crate) fn validate_peer_receipt(value: &Value) -> StoreResult<()> {
    let receipt: OrderReceipt = serde_json::from_value(value.clone())?;
    let valid_list = |list: &[String]| {
        list.iter().all(|id| !id.is_empty())
            && list.iter().collect::<std::collections::BTreeSet<_>>().len() == list.len()
    };
    let valid_observation = |value: &Value| {
        value.as_object().is_some_and(|fields| {
            fields
                .keys()
                .all(|key| matches!(key.as_str(), "value" | "revision"))
                && fields.get("value").is_some_and(|value| {
                    value.is_null()
                        || serde_json::from_value::<Vec<String>>(value.clone())
                            .is_ok_and(|ids| valid_list(&ids))
                })
                && fields
                    .get("revision")
                    .is_none_or(|v| v.is_null() || v.is_string())
        })
    };
    if serde_json::to_value(&receipt)? != *value
        || !valid_list(&receipt.desired)
        || receipt.base.as_ref().is_some_and(|v| !valid_observation(v))
        || receipt
            .conflict
            .as_ref()
            .is_some_and(|v| !valid_observation(v))
        || receipt.attempted != !receipt.effects.is_empty()
        || receipt.acknowledged
            && (receipt.conflict.is_some() || receipt.effects.last().is_some_and(|e| !e.settled))
    {
        return Err(StoreError::InvalidData(
            "invalid planning order receipt".into(),
        ));
    }
    for (index, effect) in receipt.effects.iter().enumerate() {
        if !valid_list(&effect.before)
            || !valid_list(&effect.after)
            || effect
                .before
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                != effect.after.iter().collect()
            || effect.issue.is_empty()
            || !effect.input.as_object().is_some_and(|input| {
                // Moving between priority groups leaves the secondary key alone.
                input.get("prioritySortOrder").is_some_and(Value::is_number)
                    && input.iter().all(|(key, value)| {
                        matches!(key.as_str(), "sortOrder" | "prioritySortOrder")
                            && value.is_number()
                    })
            })
            || index > 0
                && (!receipt.effects[index - 1].settled
                    || !same_order(&receipt.effects[index - 1].after, &effect.before))
        {
            return Err(StoreError::InvalidData(
                "invalid planning order effect".into(),
            ));
        }
    }
    Ok(())
}

pub(super) fn import_peer_receipt<'a>(
    conn: &Connection,
    project: &ProjectId,
    id: &str,
    history: impl Iterator<Item = &'a Value>,
    mut heads: impl Iterator<Item = &'a Value>,
) -> StoreResult<()> {
    let conflict = || StoreError::PlanningReceiptConflict { effect: "ordering" };
    let mut merged: OrderReceipt = serde_json::from_value(
        heads
            .next()
            .expect("retained order history has a head")
            .clone(),
    )?;
    // Concurrent baseline changes have no list revision with which to order them.
    // Causally retired baselines stay in history but do not veto confirmed progress.
    for head in heads {
        let head: OrderReceipt = serde_json::from_value(head.clone())?;
        match (&merged.base, &head.base) {
            (Some(left), Some(right)) if left["value"] != right["value"] => {
                let left: Option<Vec<String>> = serde_json::from_value(left["value"].clone())?;
                let right: Option<Vec<String>> = serde_json::from_value(right["value"].clone())?;
                match (left, right) {
                    (Some(left), Some(right))
                        if resolved_order(conn, &left)? == resolved_order(conn, &right)? => {}
                    _ => return Err(conflict()),
                }
            }
            (None, Some(_)) => merged.base = head.base,
            _ => {}
        }
    }
    for value in history {
        let receipt: OrderReceipt = serde_json::from_value(value.clone())?;
        if merged.desired != receipt.desired {
            return Err(conflict());
        }
        for (index, effect) in receipt.effects.into_iter().enumerate() {
            if let Some(saved) = merged.effects.get_mut(index) {
                if saved.before != effect.before
                    || saved.after != effect.after
                    || saved.issue != effect.issue
                    || saved.input != effect.input
                {
                    return Err(conflict());
                }
                saved.settled |= effect.settled;
            } else {
                merged.effects.push(effect);
            }
        }
        merged.attempted |= receipt.attempted;
        merged.acknowledged |= receipt.acknowledged;
        if let Some(observed) = receipt.conflict {
            if merged
                .conflict
                .as_ref()
                .is_some_and(|saved| saved != &observed)
            {
                return Err(conflict());
            }
            merged.conflict = Some(observed);
        }
        merged.error = merged.error.max(receipt.error);
    }
    if merged.acknowledged && merged.conflict.is_some() {
        return Err(conflict());
    }
    if merged.acknowledged || merged.conflict.is_some() {
        merged.error = None;
    }
    validate_peer_receipt(&serde_json::to_value(&merged)?).map_err(|_| conflict())?;
    let local: Option<(String, String, String)> = conn
        .query_row(
            "SELECT project_id,field,value_json FROM project_changes WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if let Some((owner, field, desired)) = local {
        if owner != project.as_str()
            || field != "task_order"
            || serde_json::from_str::<Vec<String>>(&desired)? != merged.desired
        {
            return Err(conflict());
        }
    }
    conn.execute(
        "INSERT INTO project_changes(id,project_id,field,value_json,base_json,order_effects_json,attempted,acknowledged,conflict_json,error)
         VALUES(?1,?2,'task_order',?3,?4,?5,?6,?7,?8,?9)
         ON CONFLICT(id) DO UPDATE SET base_json=excluded.base_json,order_effects_json=excluded.order_effects_json,
            attempted=excluded.attempted,acknowledged=excluded.acknowledged,conflict_json=excluded.conflict_json,error=excluded.error
         WHERE base_json IS NOT excluded.base_json OR order_effects_json IS NOT excluded.order_effects_json
            OR attempted IS NOT excluded.attempted OR acknowledged IS NOT excluded.acknowledged
            OR conflict_json IS NOT excluded.conflict_json OR error IS NOT excluded.error",
        params![id,project.as_str(),serde_json::to_string(&merged.desired)?,
            merged.base.map(|v| v.to_string()),serde_json::to_string(&merged.effects)?,
            merged.attempted,merged.acknowledged,merged.conflict.map(|v| v.to_string()),merged.error],
    )?;
    Ok(())
}
