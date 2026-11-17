//! What a Project or Task has waiting for its tracker, and what the tracker
//! won: open tracker actions, and the losing heads Git's journal keeps.
use std::collections::{BTreeMap, HashMap};

use rusqlite::Connection;

use super::children::ids_json;
use super::tracker_actions::{change_word, read_action, ActionKind, ActionPhase, COLUMNS, IN_REPO};
use super::SqliteStore;
use crate::planning_exchange::{winning_heads, PlanningMutation};
use crate::planning_sync::{TrackerActionStatus, TrackerReceipts};
use crate::StoreResult;
use loopflow_model::durable::{ProjectId, TaskId};

// An action is open while unconfirmed, or while it is its subject's latest
// conflict: a later save of the subject resolves it. A creation for an owner
// already bound waits only to be resolved.
const OPEN: &str = "NOT (e.kind='create' AND (EXISTS(SELECT 1 FROM tasks t WHERE t.id=e.task_id AND t.external_issue_id IS NOT NULL)
        OR EXISTS(SELECT 1 FROM projects p WHERE p.id=e.project_id AND p.external_project_id IS NOT NULL)))
    AND (e.phase IN ('queued','sent') OR NOT EXISTS(
        SELECT 1 FROM tracker_actions later WHERE (later.task_id=e.task_id OR later.project_id=e.project_id OR later.wave_id=e.wave_id)
        AND later.kind=e.kind AND later.subject IS e.subject AND later.seq>e.seq AND later.phase IN ('confirmed','conflicted')))";

impl SqliteStore {
    pub fn task_tracker_receipts(&self, task: &TaskId) -> StoreResult<TrackerReceipts> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let status = task_tracker_receipts_in(&tx, std::slice::from_ref(task))?
            .remove(task)
            .ok_or(crate::StoreError::NotFound)?;
        tx.commit()?;
        Ok(status)
    }

    pub fn project_tracker_receipts(&self, project: &ProjectId) -> StoreResult<TrackerReceipts> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let repo: String = tx.query_row(
            "SELECT w.repo FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
            [project.as_str()],
            |row| row.get(0),
        )?;
        let ids = ids_json([project.as_str()]);
        let mut actions = owned_actions(&tx, "project_id", &ids, false)?
            .remove(project.as_str())
            .unwrap_or_default();
        actions.extend(
            git_conflicts(&tx, &ids)?
                .remove(project.as_str())
                .unwrap_or_default(),
        );
        tx.commit()?;
        Ok(TrackerReceipts { repo, actions })
    }

    /// Every open action in `repo`, in send order: what `lf repo plan` lists.
    pub fn open_tracker_actions(&self, repo: &str) -> StoreResult<Vec<TrackerActionStatus>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // Two arms so each reads its own partial index.
        let mut query = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM tracker_actions e WHERE e.phase IN ('queued','sent') AND {IN_REPO} AND {OPEN}
             UNION ALL
             SELECT {COLUMNS} FROM tracker_actions e WHERE e.phase='conflicted' AND {IN_REPO} AND {OPEN}
             ORDER BY 1"
        ))?;
        let rows = query.query_and_then([repo], read_action)?;
        let mut actions = Vec::new();
        for row in rows {
            if let Ok(action) = row? {
                actions.push(action.status());
            }
        }
        Ok(actions)
    }
}

/// Each Task's receipts, in one statement per kind across all repositories.
pub(super) fn task_tracker_receipts_in(
    conn: &Connection,
    tasks: &[TaskId],
) -> StoreResult<HashMap<TaskId, TrackerReceipts>> {
    let ids = ids_json(tasks.iter().map(TaskId::as_str));
    let mut statement = conn.prepare(
        "SELECT t.id,w.repo,p.id FROM tasks t JOIN repository_plans w ON w.id=t.repository_id
         LEFT JOIN projects p ON p.id=t.project_id WHERE t.id IN (SELECT value FROM json_each(?1))",
    )?;
    let owners = statement
        .query_map([ids], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let task_ids = ids_json(owners.iter().map(|(task, _, _)| task.as_str()));
    let project_ids = ids_json(
        owners
            .iter()
            .filter_map(|(_, _, project)| project.as_deref()),
    );
    let mut task_actions = owned_actions(conn, "task_id", &task_ids, false)?;
    let mut task_conflicts = git_conflicts(conn, &task_ids)?;
    let project_actions = owned_actions(conn, "project_id", &project_ids, true)?;
    let mut receipts = HashMap::new();
    for (task, repo, project) in owners {
        let mut actions = task_actions.remove(&task).unwrap_or_default();
        if let Some(order) = project
            .as_ref()
            .and_then(|project| project_actions.get(project))
        {
            actions.extend(order.iter().cloned());
        }
        actions.extend(task_conflicts.remove(&task).unwrap_or_default());
        receipts.insert(TaskId::from_raw(task), TrackerReceipts { repo, actions });
    }
    Ok(receipts)
}

/// Open actions of every owner in `ids` (a JSON array), by owner id.
fn owned_actions(
    conn: &Connection,
    owner: &str,
    ids: &str,
    order_only: bool,
) -> StoreResult<HashMap<String, Vec<TrackerActionStatus>>> {
    let filter = if order_only { "AND e.kind='order'" } else { "" };
    let mut query = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM tracker_actions e
         WHERE e.{owner} IN (SELECT value FROM json_each(?1)) {filter}
         AND e.phase IN ('queued','sent','conflicted') AND {OPEN} ORDER BY e.seq"
    ))?;
    let mut owners = HashMap::<_, Vec<_>>::new();
    for row in query.query_and_then([ids], read_action)? {
        if let Ok(action) = row? {
            owners
                .entry(action.owner.id().to_owned())
                .or_default()
                .push(action.status());
        }
    }
    Ok(owners)
}

/// Git keeps a field's concurrent values as competing heads of its journal;
/// every head but the winner is a value that lost, open until a later save of
/// the field names all of them as parents.
fn git_conflicts(
    conn: &Connection,
    ids: &str,
) -> StoreResult<HashMap<String, Vec<TrackerActionStatus>>> {
    let mut query = conn.prepare(
        "SELECT c.record_table,c.id,c.mutation FROM planning_peer_changes c
         WHERE c.object_id IN (SELECT value FROM json_each(?1)) AND NOT EXISTS(
             SELECT 1 FROM planning_peer_changes child,json_each(child.mutation,'$.parents') parent
             WHERE child.object_id=c.object_id AND child.field=c.field AND parent.value=c.id)",
    )?;
    let mut heads = Vec::new();
    for row in query.query_map([ids], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })? {
        let (table, id, mutation) = row?;
        heads.push((
            table,
            id,
            serde_json::from_str::<PlanningMutation>(&mutation)?,
        ));
    }
    let winners: BTreeMap<_, _> = winning_heads(
        heads
            .iter()
            .map(|(_, id, mutation)| (id.as_str(), mutation)),
    )
    .map(|(id, mutation)| (mutation.edit.key(), (id, mutation)))
    .collect();
    let mut conflicts = HashMap::<_, Vec<_>>::new();
    for (table, id, mutation) in &heads {
        let (winner, won) = winners[&mutation.edit.key()];
        if winner == id {
            continue;
        }
        let kind = ActionKind::of(&mutation.edit.change).unwrap_or(ActionKind::Field);
        let shown = |mutation: &PlanningMutation| -> StoreResult<serde_json::Value> {
            let value = mutation.edit.change.value()?;
            Ok(match kind {
                ActionKind::State => value["planning_state"].clone(),
                _ => value,
            })
        };
        let field = mutation.edit.change.field();
        let local_value = shown(mutation)?;
        conflicts
            .entry(mutation.edit.target.clone())
            .or_default()
            .push(TrackerActionStatus {
                id: id.clone(),
                seq: None,
                owner_kind: match table.as_str() {
                    "projects" => "project",
                    _ => "task",
                }
                .to_owned(),
                owner_id: mutation.edit.target.clone(),
                kind,
                change: change_word(kind, Some(field), local_value.as_str()),
                subject: Some(field.to_owned()),
                phase: ActionPhase::Conflicted,
                tracker: "git".to_owned(),
                error: None,
                local_value,
                tracker_value: Some(shown(won)?),
            });
    }
    Ok(conflicts)
}
