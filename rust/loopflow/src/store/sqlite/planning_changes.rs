//! Field receipts share one protocol; each object's table retains its foreign key.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use super::SqliteStore;
use crate::durable::{ProjectId, TaskId};
use crate::planning::PlanningChange;
use crate::store::StoreResult;

impl SqliteStore {
    pub(crate) fn planning_field_owners(
        &self,
        repo: &str,
    ) -> StoreResult<Vec<crate::durable::WorkRef>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT 'task',t.id FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id
             WHERE w.repo=?1 AND t.external_issue_id IS NOT NULL AND EXISTS(
                 SELECT 1 FROM task_changes c WHERE c.task_id=t.id
                 AND ((t.planning_deleted_at IS NULL AND c.field!='deleted')
                     OR (t.planning_deleted_at IS NOT NULL AND c.field='deleted'))
                 AND c.acknowledged=0 AND c.conflict_json IS NULL
                 AND (c.attempted=1 OR c.seq=(SELECT max(seq) FROM task_changes WHERE task_id=t.id AND field=c.field)))
             UNION ALL
             SELECT 'project',p.id FROM projects p JOIN waves w ON w.id=p.wave_id
             WHERE w.repo=?1 AND p.external_project_id IS NOT NULL AND EXISTS(
                 SELECT 1 FROM project_changes c WHERE c.project_id=p.id AND c.acknowledged=0 AND c.conflict_json IS NULL
                 AND (c.attempted=1 OR c.seq=(SELECT max(seq) FROM project_changes WHERE project_id=p.id AND field=c.field)))"
        )?;
        let rows = query.query_map([repo], |row| {
            let kind: String = row.get(0)?;
            let id: String = row.get(1)?;
            Ok(if kind == "task" {
                crate::durable::WorkRef::Task(TaskId::from_raw(id))
            } else {
                crate::durable::WorkRef::Project(ProjectId::from_raw(id))
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub(crate) fn attempt_planning_field(
        &self,
        owner: PlanningChanges<'_>,
        change: &PlanningChange,
        revision: Option<&str>,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let observation = owner.observation(&tx)?;
        if observation
            .as_ref()
            .and_then(|body| body["revision"].as_str())
            != revision
        {
            return Ok(false);
        }
        if let PlanningChanges::Task(id) = owner {
            let deleted: bool = tx.query_row(
                "SELECT planning_deleted_at IS NOT NULL FROM tasks WHERE id=?1",
                [id.as_str()],
                |row| row.get(0),
            )?;
            if deleted != (change.field == "deleted") {
                return Ok(false);
            }
        }
        let (owner, id) = owner.owner();
        let changed = tx.execute(
            &format!(
                "UPDATE {owner}_changes SET attempted=1,error=NULL WHERE id=?1 AND {owner}_id=?2
             AND attempted=0 AND acknowledged=0 AND conflict_json IS NULL
             AND seq=(SELECT max(seq) FROM {owner}_changes WHERE {owner}_id=?2 AND field=?3)
             AND NOT EXISTS(SELECT 1 FROM {owner}_changes WHERE {owner}_id=?2 AND field=?3
                 AND attempted=1 AND acknowledged=0 AND conflict_json IS NULL)"
            ),
            params![change.id, id, change.field],
        )? == 1;
        tx.commit()?;
        Ok(changed)
    }

    pub(crate) fn observe_task_deletion(&self, id: &TaskId, revision: &str) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let owner = PlanningChanges::Task(id);
        let retained = owner.observation(&tx)?;
        if retained.as_ref().and_then(|body| body["revision"].as_str()) != Some(revision) {
            return Ok(());
        }
        if let Some(change) = owner
            .pending(&tx)?
            .into_iter()
            .find(|c| c.field == "deleted")
        {
            let baseline = change
                .base
                .as_ref()
                .and_then(|base| base["revision"].as_str());
            // Deletion competes with the whole issue. Only an explicitly active
            // issue at a newer revision can restore planning visibility.
            if baseline.is_some()
                && super::planning::revision_nanos(Some(revision))?
                    > super::planning::revision_nanos(baseline)?
            {
                tx.execute(
                    "UPDATE task_changes SET conflict_json=?2,error=NULL WHERE id=?1 AND conflict_json IS NULL",
                    params![change.id, serde_json::json!({"revision":revision,"value":false}).to_string()],
                )?;
                tx.execute(
                    "UPDATE tasks SET planning_deleted_at=NULL,planning_revision=planning_revision+1
                     WHERE id=?1 AND planning_deleted_at IS NOT NULL",
                    [id.as_str()],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Only a positive trash observation or the exact mutation acknowledgement settles removal.
    pub(crate) fn acknowledge_task_deletion(
        &self,
        id: &TaskId,
        change: &PlanningChange,
        revision: Option<&str>,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(revision) = revision {
            let observed = PlanningChanges::Task(id).observation(&tx)?;
            let retained = observed.as_ref().and_then(|body| body["revision"].as_str());
            if super::planning::revision_nanos(Some(revision))?
                < super::planning::revision_nanos(retained)?
            {
                return Ok(false);
            }
        }
        let changed = tx.execute(
            "UPDATE task_changes SET acknowledged=1,acknowledged_revision=?3,error=NULL
             WHERE id=?1 AND task_id=?2 AND field='deleted'
             AND acknowledged=0 AND conflict_json IS NULL",
            params![change.id, id.as_str(), revision],
        )? == 1;
        tx.commit()?;
        Ok(changed)
    }

    pub(crate) fn planning_field_error(
        &self,
        owner: PlanningChanges<'_>,
        change: &PlanningChange,
        error: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (owner, id) = owner.owner();
        conn.execute(
            &format!(
                "UPDATE {owner}_changes SET error=?3 WHERE id=?1 AND {owner}_id=?2
            AND acknowledged=0 AND conflict_json IS NULL AND error IS NOT ?3"
            ),
            params![change.id, id, error],
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PlanningChanges<'a> {
    Task(&'a TaskId),
    Project(&'a ProjectId),
}

impl<'a> PlanningChanges<'a> {
    // A matching attempted write can arrive through either acquisition or readback.
    // Rebase later saves before conflict reconciliation; never acknowledge them.
    fn observe_attempts(self, conn: &Connection, observed: &Value) -> StoreResult<()> {
        let (owner, id) = self.owner();
        let mut query = conn.prepare(&format!(
            "SELECT id,field,value_json,base_json FROM {owner}_changes
             WHERE {owner}_id=?1 AND attempted=1 AND acknowledged=0 AND conflict_json IS NULL ORDER BY seq"
        ))?;
        let changes = query
            .query_and_then([id], read_change)?
            .collect::<StoreResult<Vec<_>>>()?;
        for PlanningChange {
            id: receipt,
            field,
            value,
            base,
        } in changes
        {
            let Some(remote) = observed.get(&field) else {
                continue;
            };
            let remote = self.normalize(conn, &field, remote.clone())?;
            if super::planning::revision_nanos(observed["revision"].as_str())?
                < super::planning::revision_nanos(
                    base.as_ref().and_then(|b| b["revision"].as_str()),
                )?
            {
                continue;
            }
            if remote == value {
                let baseline =
                    serde_json::json!({"revision": observed["revision"], "value": remote});
                conn.execute(
                    &format!(
                        "UPDATE {owner}_changes SET base_json=?2 WHERE {owner}_id=?3 AND field=?4
                     AND seq>(SELECT seq FROM {owner}_changes WHERE id=?1)
                     AND attempted=0 AND acknowledged=0 AND conflict_json IS NULL
                     AND (base_json IS ?5 OR (base_json IS NOT NULL AND ?5 IS NOT NULL
                         AND json_extract(base_json,'$.value') IS json_extract(?5,'$.value')))"
                    ),
                    params![
                        receipt,
                        baseline.to_string(),
                        id,
                        field,
                        base.as_ref().map(Value::to_string)
                    ],
                )?;
                conn.execute(
                    &format!("UPDATE {owner}_changes SET acknowledged=1,acknowledged_revision=?2,error=NULL WHERE id=?1"),
                    params![receipt, observed["revision"].as_str()],
                )?;
            } else if base.as_ref().is_none_or(|base| base["value"] != remote) {
                conn.execute(
                    &format!("UPDATE {owner}_changes SET conflict_json=?2,error=NULL WHERE id=?1"),
                    params![
                        receipt,
                        serde_json::json!({"revision":observed["revision"],"value":remote})
                            .to_string()
                    ],
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn owner(self) -> (&'static str, &'a str) {
        match self {
            Self::Task(id) => ("task", id.as_str()),
            Self::Project(id) => ("project", id.as_str()),
        }
    }

    fn observation(self, conn: &Connection) -> StoreResult<Option<Value>> {
        let (_, id) = self.owner();
        let observation = match self {
            Self::Task(_) => "SELECT i.body FROM tasks t JOIN projects p ON p.id=t.project_id
                JOIN waves w ON w.id=p.wave_id
                JOIN pm_items i ON i.id=t.external_issue_id AND i.repo=w.repo AND i.provider='linear'
                WHERE t.id=?1",
            Self::Project(_) => "SELECT o.body FROM projects p JOIN waves w ON w.id=p.wave_id
                JOIN pm_projects o ON o.id=p.external_project_id AND o.repo=w.repo AND o.provider='linear'
                WHERE p.id=?1",
        };
        let body: Option<String> = conn
            .query_row(observation, [id], |row| row.get(0))
            .optional()?;
        body.map(|body| serde_json::from_str(&body).map_err(Into::into))
            .transpose()
    }

    pub(super) fn record(
        self,
        conn: &Connection,
        field: &str,
        previous: Value,
        value: Value,
    ) -> StoreResult<bool> {
        let previous = self.normalize(conn, field, previous)?;
        let value = self.normalize(conn, field, value)?;
        if previous == value {
            return Ok(false);
        }
        let (owner, id) = self.owner();
        let body = self.observation(conn)?;
        let base = body
            .map(|body| -> StoreResult<Value> {
                Ok(serde_json::json!({"revision": body["revision"], "value": self.normalize(conn, field, body[field].clone())?}))
            })
            .transpose()?;
        conn.execute(
            &format!(
                "INSERT INTO {owner}_changes(id,{owner}_id,field,value_json,base_json)
                VALUES(?1,?2,?3,?4,?5)"
            ),
            params![
                uuid::Uuid::new_v4().to_string(),
                id,
                field,
                value.to_string(),
                base.map(|v| v.to_string())
            ],
        )?;
        Ok(true)
    }

    // Membership receipts retain durable identity even when a provider mapping arrives later.
    pub(super) fn normalize(
        self,
        conn: &Connection,
        field: &str,
        value: Value,
    ) -> StoreResult<Value> {
        if matches!(self, Self::Task(_)) && field == "project_id" {
            if let Some(selector) = value.as_str() {
                let id: Option<String> = conn
                    .query_row(
                        "SELECT id FROM projects WHERE id=?1 OR external_project_id=?1",
                        [selector],
                        |row| row.get(0),
                    )
                    .optional()?;
                if let Some(id) = id {
                    return Ok(Value::String(id));
                }
            }
        }
        Ok(value)
    }

    pub(super) fn pending(self, conn: &Connection) -> StoreResult<Vec<PlanningChange>> {
        let (owner, id) = self.owner();
        let mut query = conn.prepare(&format!(
            "SELECT id,field,value_json,base_json FROM {owner}_changes c
             WHERE {owner}_id=?1 AND acknowledged=0 AND conflict_json IS NULL AND seq=(SELECT max(seq) FROM {owner}_changes
                 WHERE {owner}_id=c.{owner}_id AND field=c.field) ORDER BY seq"
        ))?;
        let changes = query.query_and_then([id], read_change)?;
        changes.collect()
    }

    /// Unchanged baselines preserve saves; observed conflicts retire their delivery.
    /// Both values remain in the receipt. Only attempted writes can be acknowledged.
    pub(super) fn reconcile<T: Serialize + DeserializeOwned>(
        self,
        conn: &Connection,
        observed: &T,
    ) -> StoreResult<T> {
        let (owner, _) = self.owner();
        let mut saved = serde_json::to_value(observed)?;
        self.observe_attempts(conn, &saved)?;
        for change in self.pending(conn)? {
            // Ordinary inventory does not establish whether an issue is trashed.
            let Some(value) = saved.get(&change.field) else {
                continue;
            };
            let remote = self.normalize(conn, &change.field, value.clone())?;
            if remote != change.value
                && change
                    .base
                    .as_ref()
                    .is_none_or(|base| base["value"] != remote)
            {
                conn.execute(
                    &format!("UPDATE {owner}_changes SET conflict_json=?2 WHERE id=?1 AND conflict_json IS NULL"),
                    params![change.id, serde_json::json!({"revision": saved["revision"], "value": remote}).to_string()],
                )?;
            } else {
                saved[&change.field] = change.value;
            }
        }
        Ok(serde_json::from_value(saved)?)
    }
}

fn read_change(row: &rusqlite::Row<'_>) -> StoreResult<PlanningChange> {
    let value: String = row.get(2)?;
    let base: Option<String> = row.get(3)?;
    Ok(PlanningChange {
        id: row.get(0)?,
        field: row.get(1)?,
        value: serde_json::from_str(&value)?,
        base: base.map(|value| serde_json::from_str(&value)).transpose()?,
    })
}
