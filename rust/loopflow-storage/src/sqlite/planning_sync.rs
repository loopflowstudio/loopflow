//! What a Project or Task has waiting for its tracker, and what the tracker
//! won: open and lost tracker actions, and a person's answer to a lost value.
use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension};

use super::children::ids_json;
use super::tracker_actions::{read_action, COLUMNS, IN_REPO};
use super::{planning_write, SqliteStore};
use crate::planning_sync::{LostAnswer, TrackerActionStatus, TrackerActions};
use crate::work::edit::{WorkEdit, WorkValue};
use crate::StoreResult;
use loopflow_model::durable::{ProjectId, TaskId};

// An action shows while queued or sent, and while conflicted: a value that
// lost stays until it is answered (`answer_lost`, a later save, or the
// tracker changing that field again). A creation for an owner already bound
// waits only to be resolved.
const SHOWN: &str = "NOT (e.kind='create' AND (EXISTS(SELECT 1 FROM tasks t WHERE t.id=e.task_id AND t.external_issue_id IS NOT NULL)
        OR EXISTS(SELECT 1 FROM projects p WHERE p.id=e.project_id AND p.external_project_id IS NOT NULL)))";

impl SqliteStore {
    pub fn task_tracker_actions(&self, task: &TaskId) -> StoreResult<TrackerActions> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let actions = task_tracker_actions_in(&tx, std::slice::from_ref(task))?
            .remove(task)
            .ok_or(crate::StoreError::NotFound)?;
        tx.commit()?;
        Ok(actions)
    }

    pub fn project_tracker_actions(&self, project: &ProjectId) -> StoreResult<TrackerActions> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let repo: String = tx.query_row(
            "SELECT w.repo FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
            [project.as_str()],
            |row| row.get(0),
        )?;
        let ids = ids_json([project.as_str()]);
        let actions = owned_actions(&tx, "project_id", &ids, false)?
            .remove(project.as_str())
            .unwrap_or_default();
        tx.commit()?;
        Ok(TrackerActions { repo, actions })
    }

    /// Where the tracker shows the Project, once a read of it saw a URL.
    pub fn project_url(&self, project: &ProjectId) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT t.url FROM projects p JOIN waves w ON w.id=p.wave_id
                 JOIN tracker_projects t ON t.repo=w.repo AND t.tracker IN ('linear','asana')
                      AND t.id=p.external_project_id
                 WHERE p.id=?1",
                [project.as_str()],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    /// What `lf repo tracker` lists: every open and lost action in send order.
    pub fn repository_tracker_actions(&self, repo: &str) -> StoreResult<Vec<TrackerActionStatus>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // Two arms so each reads its own partial index.
        let mut query = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM tracker_actions e WHERE e.phase IN ('queued','sent') AND {IN_REPO} AND {SHOWN}
             UNION ALL
             SELECT {COLUMNS} FROM tracker_actions e WHERE e.phase='conflicted' AND {IN_REPO} AND {SHOWN}
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

    /// Answer lost lines, whoever owns them: keep the tracker's value, or
    /// save the local one again, which queues it. A line answered meanwhile
    /// is left as it is.
    pub fn answer_lost(
        &self,
        lines: &[TrackerActionStatus],
        answer: LostAnswer,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for line in lines {
            answer_queued(&tx, &line.id, answer)?;
        }
        tx.commit()?;
        Ok(())
    }
}

fn answer_queued(conn: &Connection, id: &str, answer: LostAnswer) -> StoreResult<()> {
    let reason = match answer {
        LostAnswer::KeepTheirs => "kept the tracker's value",
        LostAnswer::UseMine => {
            let Some(action) = conn
                .prepare(&format!(
                    "SELECT {COLUMNS} FROM tracker_actions e WHERE e.id=?1 AND e.phase='conflicted'"
                ))?
                .query_and_then([id], read_action)?
                .next()
                .transpose()?
            else {
                return Ok(());
            };
            let action = action.map_err(|_| crate::StoreError::NotFound)?;
            let change = serde_json::from_value::<WorkValue>(action.value).map_err(|_| {
                crate::StoreError::InvalidData(format!(
                    "a lost {} cannot be saved again; make the change again",
                    action.kind.as_str()
                ))
            })?;
            planning_write::local(
                conn,
                &[WorkEdit {
                    target: action.owner.id().to_owned(),
                    change,
                }],
            )?;
            "answered by a later save"
        }
    };
    conn.execute(
        "UPDATE tracker_actions SET phase='retired',error=?2 WHERE id=?1 AND phase='conflicted'",
        rusqlite::params![id, reason],
    )?;
    Ok(())
}

/// Each Task's open and lost tracker actions, in one statement per kind
/// across all repositories.
pub(super) fn task_tracker_actions_in(
    conn: &Connection,
    tasks: &[TaskId],
) -> StoreResult<HashMap<TaskId, TrackerActions>> {
    let ids = ids_json(tasks.iter().map(TaskId::as_str));
    let mut statement = conn.prepare_cached(
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
    let project_actions = owned_actions(conn, "project_id", &project_ids, true)?;
    let mut open = HashMap::new();
    for (task, repo, project) in owners {
        let mut actions = task_actions.remove(&task).unwrap_or_default();
        if let Some(order) = project
            .as_ref()
            .and_then(|project| project_actions.get(project))
        {
            actions.extend(order.iter().cloned());
        }
        open.insert(TaskId::from_raw(task), TrackerActions { repo, actions });
    }
    Ok(open)
}

/// Open and lost actions of every owner in `ids` (a JSON array), by owner id.
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
         AND e.phase IN ('queued','sent','conflicted') AND {SHOWN} ORDER BY e.seq"
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
