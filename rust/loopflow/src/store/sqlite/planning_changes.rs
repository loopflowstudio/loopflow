//! Field receipts share one protocol; each object's table retains its foreign key.

use super::planning_write::{self, PlanningEdit as Edit};
use crate::planning_exchange::{PlanningKind, PlanningObject};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use super::SqliteStore;
use crate::durable::{ProjectId, TaskId};
use crate::store::planning_sync::PlanningChange;
use crate::store::{StoreError, StoreResult};

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
        if owner.observed_revision(&tx)?.as_deref() != revision {
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
        if owner.observed_revision(&tx)?.as_deref() != Some(revision) {
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
                planning_write::local(
                    &tx,
                    PlanningKind::Task,
                    id.as_str(),
                    &[Edit::TaskDeletedAt(None)],
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
            let retained = PlanningChanges::Task(id).observed_revision(&tx)?;
            if super::planning::revision_nanos(Some(revision))?
                < super::planning::revision_nanos(retained.as_deref())?
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
    /// Save one typed edit slice to Work, Git history and Linear field receipts.
    /// The caller owns the transaction, including any selection or ordering changes.
    pub(super) fn save(self, conn: &Connection, edits: &[Edit]) -> StoreResult<()> {
        if edits.is_empty() {
            return Ok(());
        }
        let kind = match self {
            Self::Task(_) => PlanningKind::Task,
            Self::Project(_) => PlanningKind::Project,
        };
        let object = PlanningObject {
            kind,
            id: self.owner().1.to_owned(),
        };
        let current = planning_write::read(conn, &object)?;
        for edit in edits {
            let field = linear_field(edit)
                .ok_or_else(|| StoreError::InvalidData("Edit has no Linear field".into()))?;
            let previous = current
                .iter()
                .find(|previous| previous.field() == edit.field())
                .ok_or_else(|| StoreError::InvalidData("Edit belongs to another owner".into()))?;
            self.record(conn, field, previous.value()?, edit.value()?)?;
        }
        planning_write::local(conn, kind, &object.id, edits)?;
        Ok(())
    }

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

    fn observation_source(self) -> &'static str {
        match self {
            Self::Task(_) => "tasks t JOIN projects p ON p.id=t.project_id
                JOIN waves w ON w.id=p.wave_id
                JOIN pm_items o ON o.id=t.external_issue_id AND o.repo=w.repo AND o.provider='linear'
                WHERE t.id=?1",
            Self::Project(_) => "projects p JOIN waves w ON w.id=p.wave_id
                JOIN pm_projects o ON o.id=p.external_project_id AND o.repo=w.repo AND o.provider='linear'
                WHERE p.id=?1",
        }
    }

    fn observed_revision(self, conn: &Connection) -> StoreResult<Option<String>> {
        Ok(conn
            .query_row(
                &format!("SELECT o.revision FROM {}", self.observation_source()),
                [self.owner().1],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    // A receipt needs only its field's baseline. Partial prose keeps its own
    // revision; unrelated fields cannot become another writable provider model.
    fn baseline(self, conn: &Connection, field: &str) -> StoreResult<Option<Value>> {
        let fields = match self {
            Self::Task(_) => "o.revision, CASE ?2
                WHEN 'deleted' THEN 'false'
                WHEN 'name' THEN CASE WHEN o.name IS NOT NULL THEN json_quote(o.name) END
                WHEN 'description' THEN CASE WHEN o.description IS NOT NULL THEN json_quote(o.description) END
                WHEN 'project_id' THEN CASE WHEN o.project_id_observed THEN json_quote(o.claimed_project_id) END
                WHEN 'assignee' THEN CASE WHEN o.assignee_observed THEN json_quote(o.assignee) END
                WHEN 'due_date' THEN CASE WHEN o.due_date_observed THEN json_quote(o.due_date) END END",
            Self::Project(_) => "CASE WHEN ?2='description' THEN o.description_revision ELSE o.revision END,
                CASE ?2 WHEN 'description' THEN CASE WHEN o.description_observed_at IS NOT NULL THEN json_quote(o.description) END
                WHEN 'name' THEN CASE WHEN o.name IS NOT NULL THEN json_quote(o.name) END
                WHEN 'summary' THEN CASE WHEN o.summary IS NOT NULL THEN json_quote(o.summary) END
                WHEN 'workflow' THEN CASE WHEN o.workflow IS NOT NULL THEN json_quote(o.workflow) END
                WHEN 'status' THEN CASE WHEN o.status IS NOT NULL THEN json_quote(o.status) END
                WHEN 'krs' THEN o.krs
                WHEN 'metric_targets' THEN o.metric_targets
                WHEN 'task_order' THEN COALESCE(o.task_order_json,'null') END",
        };
        let baseline = conn
            .query_row(
                &format!("SELECT {fields} FROM {}", self.observation_source()),
                params![self.owner().1, field],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                },
            )
            .optional()?;
        let Some((revision, Some(value))) = baseline else {
            return Ok(None);
        };
        let value = if matches!(self, Self::Project(_)) && field == "task_order" {
            serde_json::to_value(serde_json::from_str::<Option<Vec<String>>>(&value)?)?
        } else {
            serde_json::from_str(&value)?
        };
        Ok(Some(serde_json::json!({
            "revision": revision,
            "value": self.normalize(conn, field, value)?,
        })))
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
        let base = self.baseline(conn, field)?;
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
    pub(super) fn reconcile_edits(
        self,
        conn: &Connection,
        revision: Option<&str>,
        edits: &mut [Edit],
    ) -> StoreResult<()> {
        let (owner, _) = self.owner();
        let mut observed = serde_json::json!({"revision": revision});
        for edit in edits.iter() {
            if let Some(field) = linear_field(edit) {
                observed[field] = self.normalize(conn, field, edit.value()?)?;
            }
        }
        self.observe_attempts(conn, &observed)?;
        for change in self.pending(conn)? {
            let Some(edit) = edits
                .iter_mut()
                .find(|edit| linear_field(edit) == Some(&change.field))
            else {
                continue;
            };
            let remote = &observed[&change.field];
            if remote != &change.value
                && change
                    .base
                    .as_ref()
                    .is_none_or(|base| &base["value"] != remote)
            {
                conn.execute(
                    &format!("UPDATE {owner}_changes SET conflict_json=?2 WHERE id=?1 AND conflict_json IS NULL"),
                    params![change.id, serde_json::json!({"revision": revision, "value": remote}).to_string()],
                )?;
            } else {
                // Decode only the saved field, never reconstruct a provider model.
                *edit = Edit::from_value(edit.kind(), edit.field(), change.value)?;
            }
        }
        Ok(())
    }
}

// Linear receipt names belong at the adapter boundary, not in the Work model.
pub(super) fn linear_field(edit: &Edit) -> Option<&'static str> {
    match edit {
        Edit::ProjectName(_) | Edit::TaskTitle(_) => Some("name"),
        Edit::ProjectSummary(_) => Some("summary"),
        Edit::ProjectDescription(_) | Edit::TaskDescription(_) => Some("description"),
        Edit::ProjectStatus(_) => Some("status"),
        Edit::Workflow(_) => Some("workflow"),
        Edit::Krs(_) => Some("krs"),
        Edit::MetricTargets(_) => Some("metric_targets"),
        Edit::TaskAssignee(_) => Some("assignee"),
        Edit::TaskProject(_) => Some("project_id"),
        _ => None,
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

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};
    use serde_json::json;

    use super::PlanningChanges;
    use crate::durable::TaskId;
    use crate::store::migrations::{apply_before_current_draft, current_draft_sql};

    #[test]
    fn membership_receipts_preserve_absent_null_and_contradictory_released_claims() {
        for field_name in ["project_id", "assignee"] {
            for (field, expected) in [
                (None, None),
                (
                    Some(serde_json::Value::Null),
                    Some(json!({"revision":"r1","value":null})),
                ),
                (
                    Some(json!("retained-claim")),
                    Some(json!({"revision":"r1","value":"retained-claim"})),
                ),
            ] {
                let conn = Connection::open_in_memory().unwrap();
                apply_before_current_draft(&conn, "planning_peers");
                conn.execute_batch(include_str!(
                    "../../../tests/fixtures/migrations/planning_relationships.sql"
                ))
                .unwrap();
                conn.execute(
                    "UPDATE tasks SET external_issue_id='issue' WHERE id='task-proof'",
                    [],
                )
                .unwrap();
                let mut input = json!({"revision":"r1"});
                if let Some(field) = field {
                    input[field_name] = field;
                }
                conn.execute(
                    "INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body)
                VALUES('/repo','linear','issue','PROOF-1','indexed-project',17,?1)",
                    [input.to_string()],
                )
                .unwrap();
                conn.execute_batch(&current_draft_sql("planning_peers"))
                    .unwrap();
                let task = TaskId::from_raw("task-proof");
                assert!(PlanningChanges::Task(&task)
                    .record(
                        &conn,
                        field_name,
                        json!("project-proof"),
                        json!("desired-project")
                    )
                    .unwrap());
                let base: Option<String> = conn
                    .query_row(
                        "SELECT base_json FROM task_changes WHERE task_id=?1",
                        params![task.as_str()],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(
                    base.map(|value| serde_json::from_str::<serde_json::Value>(&value).unwrap()),
                    expected
                );
            }
        }
    }
}
