//! Field receipts share one protocol; each object's table retains its foreign key.

use super::planning_write;
use crate::work::edit::WorkValue as Edit;
use rusqlite::{params, Connection, OptionalExtension};

use super::SqliteStore;
use crate::durable::{ProjectId, TaskId};
use crate::store::planning_sync::{PlanningBaseline, PlanningChange, TaskDeletion};
use crate::store::{StoreError, StoreResult};

impl SqliteStore {
    /// Delivery requires the fetched revision to have reached this Task's Work row.
    /// Retaining an observation (including a foreign or rejected one) is insufficient.
    pub(crate) fn accepted_task_revision(
        &self,
        task: &crate::durable::TaskId,
        issue: &str,
        revision: Option<&str>,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND external_issue_id=?2
             AND planning_provider_revision=?3)",
            params![task.as_str(), issue, revision],
            |row| row.get(0),
        )?)
    }

    /// Retained observations do not authorize delivery until accepted into Work.
    pub(crate) fn accepted_project_revision(
        &self,
        project: &ProjectId,
        external: &str,
        revision: Option<&str>,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects p
             JOIN waves w ON w.id=p.wave_id
             JOIN pm_projects observed ON observed.repo=w.repo
                 AND observed.provider='linear' AND observed.id=p.external_project_id
             WHERE p.id=?1 AND p.external_project_id=?2 AND p.planning_provider_revision=?3
                 AND observed.archived=0 AND observed.membership_unresolved=0)",
            params![project.as_str(), external, revision],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn planning_field_owners(
        &self,
        repo: &str,
    ) -> StoreResult<Vec<crate::durable::WorkRef>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT 'task',t.id FROM tasks t JOIN repository_plans w ON w.id=t.repository_id
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
        retry: bool,
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
            if deleted {
                return Ok(false);
            }
        }
        let (owner, id) = owner.owner();
        let changed = tx.execute(
            &format!(
                "UPDATE {owner}_changes SET attempted=1,error=NULL WHERE id=?1 AND {owner}_id=?2
             AND (attempted=0 OR ?4) AND acknowledged=0 AND conflict_json IS NULL
             AND seq=(SELECT max(seq) FROM {owner}_changes WHERE {owner}_id=?2 AND field=?3)
             AND NOT EXISTS(SELECT 1 FROM {owner}_changes WHERE {owner}_id=?2 AND field=?3
                 AND attempted=1 AND acknowledged=0 AND conflict_json IS NULL AND (id!=?1 OR NOT ?4))"
            ),
            params![change.id, id, change.value.field(), retry],
        )? == 1;
        tx.commit()?;
        Ok(changed)
    }

    pub(crate) fn reject_planning_field(
        &self,
        owner: PlanningChanges<'_>,
        change_id: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (kind, id) = owner.owner();
        conn.execute(&format!("UPDATE {kind}_changes SET attempted=0 WHERE id=?1 AND {kind}_id=?2 AND acknowledged=0 AND conflict_json IS NULL"), params![change_id, id])?;
        Ok(())
    }

    pub(crate) fn observe_task_deletion(&self, id: &TaskId, revision: &str) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let owner = PlanningChanges::Task(id);
        if owner.observed_revision(&tx)?.as_deref() != Some(revision) {
            return Ok(());
        }
        if let Some(change) = pending_deletion(&tx, id)? {
            let baseline = change.base_revision.as_deref();
            // Deletion competes with the whole issue. Only an explicitly active
            // issue at a newer revision can restore planning visibility.
            if baseline.is_some()
                && crate::linear::observations::revision_nanos(Some(revision))?
                    > crate::linear::observations::revision_nanos(baseline)?
            {
                tx.execute(
                    "UPDATE task_changes SET conflict_json=?2,error=NULL WHERE id=?1 AND conflict_json IS NULL",
                    params![change.id, serde_json::json!({"revision":revision,"value":false}).to_string()],
                )?;
                planning_write::local(
                    &tx,
                    &crate::work::edit::WorkEdit::for_target(
                        id.as_str(),
                        &[Edit::TaskDeletedAt(None)],
                    ),
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
        change: &TaskDeletion,
        revision: Option<&str>,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(revision) = revision {
            let retained = PlanningChanges::Task(id).observed_revision(&tx)?;
            if crate::linear::observations::revision_nanos(Some(revision))?
                < crate::linear::observations::revision_nanos(retained.as_deref())?
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
        receipt: &str,
        error: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (owner, id) = owner.owner();
        conn.execute(
            &format!(
                "UPDATE {owner}_changes SET error=?3 WHERE id=?1 AND {owner}_id=?2
            AND acknowledged=0 AND conflict_json IS NULL AND error IS NOT ?3"
            ),
            params![receipt, id, error],
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
        let table = match self {
            Self::Task(_) => "tasks",
            Self::Project(_) => "projects",
        };
        let current = planning_write::read(conn, table, self.owner().1)?;
        for edit in edits {
            let previous = current
                .iter()
                .find(|previous| previous.field() == edit.field())
                .ok_or_else(|| StoreError::InvalidData("Edit belongs to another owner".into()))?;
            self.record(conn, previous, edit)?;
        }
        planning_write::local(
            conn,
            &crate::work::edit::WorkEdit::for_target(self.owner().1, edits),
        )?;
        Ok(())
    }

    // A matching attempted write can arrive through acquisition or readback.
    // Rebase later unsent saves before reconciliation, without acknowledging them.
    fn observe_attempts(
        self,
        conn: &Connection,
        revision: Option<&str>,
        observed: &[Edit],
    ) -> StoreResult<()> {
        let (owner, id) = self.owner();
        let mut query = conn.prepare(&format!(
            "SELECT id,value_json,base_json FROM {owner}_changes
             WHERE {owner}_id=?1 AND field NOT IN ('deleted','task_order')
             AND attempted=1 AND acknowledged=0 AND conflict_json IS NULL ORDER BY seq"
        ))?;
        let changes = query
            .query_and_then([id], read_change)?
            .collect::<StoreResult<Vec<_>>>()?;
        for change in changes {
            let Some(remote) = observed
                .iter()
                .find(|value| value.field() == change.value.field())
            else {
                continue;
            };
            if crate::linear::observations::revision_nanos(revision)?
                < crate::linear::observations::revision_nanos(
                    change.base.as_ref().and_then(|b| b.revision.as_deref()),
                )?
            {
                continue;
            }
            let baseline = PlanningBaseline {
                revision: revision.map(str::to_owned),
                value: Some(remote.clone()),
            };
            if remote == &change.value {
                let mut later = conn.prepare(&format!(
                    "SELECT id,value_json,base_json FROM {owner}_changes
                     WHERE {owner}_id=?1 AND field=?2 AND seq>(SELECT seq FROM {owner}_changes WHERE id=?3)
                     AND attempted=0 AND acknowledged=0 AND conflict_json IS NULL"
                ))?;
                let later = later
                    .query_and_then(params![id, change.value.field(), change.id], read_change)?
                    .collect::<StoreResult<Vec<_>>>()?;
                for saved in later {
                    if saved.base.as_ref().map(|b| &b.value)
                        == change.base.as_ref().map(|b| &b.value)
                    {
                        conn.execute(
                            &format!("UPDATE {owner}_changes SET base_json=?2 WHERE id=?1"),
                            params![saved.id, serde_json::to_string(&baseline)?],
                        )?;
                    }
                }
                conn.execute(&format!("UPDATE {owner}_changes SET acknowledged=1,acknowledged_revision=?2,error=NULL WHERE id=?1"), params![change.id,revision])?;
            } else if change.base.as_ref().and_then(|b| b.value.as_ref()) != Some(remote) {
                conn.execute(
                    &format!("UPDATE {owner}_changes SET conflict_json=?2,error=NULL WHERE id=?1"),
                    params![change.id, serde_json::to_string(&baseline)?],
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
            Self::Task(_) => "tasks t JOIN repository_plans w ON w.id=t.repository_id
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

    // Read only the edited field's observation. Prose keeps its own revision;
    // an explicitly null membership is distinct from an omitted observation.
    fn baseline(self, conn: &Connection, edit: &Edit) -> StoreResult<Option<PlanningBaseline>> {
        let (column, presence) = match edit {
            Edit::TaskTitle(_) | Edit::ProjectName(_) => ("o.name", "o.name IS NOT NULL"),
            Edit::TaskDescription(_) => ("o.description", "o.description IS NOT NULL"),
            Edit::ProjectDescription(_) => ("o.description", "o.description_observed_at IS NOT NULL"),
            Edit::ProjectSummary(_) => ("o.summary", "o.summary IS NOT NULL"),
            Edit::ProjectStatus(_) => ("o.status", "o.status IS NOT NULL"),
            Edit::HistoricalProjectWorkflow(_) => return Ok(None),
            Edit::Krs(_) => ("o.krs", "o.krs IS NOT NULL"),
            Edit::MetricTargets(_) => ("o.metric_targets", "o.metric_targets IS NOT NULL"),
            Edit::TaskAssignee(_) => ("o.assignee", "o.assignee_observed"),
            Edit::TaskProject(_) => ("COALESCE((SELECT id FROM projects WHERE id=o.claimed_project_id OR external_project_id=o.claimed_project_id),o.claimed_project_id)", "o.project_id_observed"),
            _ => return Err(StoreError::InvalidData("Edit has no Linear field receipt".into())),
        };
        let revision = if matches!(edit, Edit::ProjectDescription(_)) {
            "o.description_revision"
        } else {
            "o.revision"
        };
        let mut query = conn.prepare(&format!(
            "SELECT {revision},{column} FROM {} AND {presence}",
            self.observation_source()
        ))?;
        let baseline = query
            .query_and_then([self.owner().1], |row| -> StoreResult<PlanningBaseline> {
                let value = match edit {
                    Edit::TaskTitle(_) => Some(Edit::TaskTitle(row.get(1)?)),
                    Edit::ProjectName(_) => Some(Edit::ProjectName(row.get(1)?)),
                    Edit::TaskDescription(_) => Some(Edit::TaskDescription(row.get(1)?)),
                    Edit::ProjectDescription(_) => Some(Edit::ProjectDescription(row.get(1)?)),
                    Edit::ProjectSummary(_) => Some(Edit::ProjectSummary(row.get(1)?)),
                    Edit::ProjectStatus(_) => Some(Edit::ProjectStatus(serde_json::from_value(
                        serde_json::Value::String(row.get(1)?),
                    )?)),
                    Edit::Krs(_) => {
                        Some(Edit::Krs(serde_json::from_str(&row.get::<_, String>(1)?)?))
                    }
                    Edit::MetricTargets(_) => Some(Edit::MetricTargets(serde_json::from_str(
                        &row.get::<_, String>(1)?,
                    )?)),
                    Edit::TaskAssignee(_) => Some(Edit::TaskAssignee(row.get(1)?)),
                    Edit::TaskProject(_) => {
                        Some(Edit::TaskProject(row.get::<_, Option<String>>(1)?))
                    }
                    _ => unreachable!("receipt field was selected above"),
                };
                Ok(PlanningBaseline {
                    revision: row.get(0)?,
                    value,
                })
            })?
            .next()
            .transpose();
        baseline
    }

    pub(super) fn record(
        self,
        conn: &Connection,
        previous: &Edit,
        value: &Edit,
    ) -> StoreResult<bool> {
        if previous == value {
            return Ok(false);
        }
        let (owner, id) = self.owner();
        let base = self.baseline(conn, value)?;
        conn.execute(&format!("INSERT INTO {owner}_changes(id,{owner}_id,value_json,base_json) VALUES(?1,?2,?3,?4)"),
            params![uuid::Uuid::new_v4().to_string(),id,serde_json::to_string(value)?,base.map(|b|serde_json::to_string(&b)).transpose()?])?;
        Ok(true)
    }

    pub(super) fn pending(self, conn: &Connection) -> StoreResult<Vec<PlanningChange>> {
        let (owner, id) = self.owner();
        let mut query = conn.prepare(&format!(
            "SELECT id,value_json,base_json FROM {owner}_changes c
             WHERE {owner}_id=?1 AND field NOT IN ('deleted','task_order') AND acknowledged=0 AND conflict_json IS NULL AND seq=(SELECT max(seq) FROM {owner}_changes
                 WHERE {owner}_id=c.{owner}_id AND field=c.field) ORDER BY seq"
        ))?;
        let changes = query.query_and_then([id], read_change)?;
        changes.collect()
    }

    /// The change sent last wins. An unsent save still goes out after a provider
    /// edit, so it rebases onto that edit and stays pending, naming the value it
    /// will replace. Only attempted writes can be acknowledged.
    pub(crate) fn reconcile_edits(
        self,
        conn: &Connection,
        revision: Option<&str>,
        edits: &mut [Edit],
    ) -> StoreResult<()> {
        let (owner, _) = self.owner();
        self.observe_attempts(conn, revision, edits)?;
        for change in self.pending(conn)? {
            let Some(edit) = edits
                .iter_mut()
                .find(|edit| edit.field() == change.value.field())
            else {
                continue;
            };
            if edit != &change.value
                && change.base.as_ref().and_then(|b| b.value.as_ref()) != Some(edit)
            {
                let replaced = PlanningBaseline {
                    revision: revision.map(str::to_owned),
                    value: Some(edit.clone()),
                };
                conn.execute(
                    &format!("UPDATE {owner}_changes SET base_json=?2 WHERE id=?1"),
                    params![change.id, serde_json::to_string(&replaced)?],
                )?;
            }
            *edit = change.value;
        }
        Ok(())
    }
}

fn read_change(row: &rusqlite::Row<'_>) -> StoreResult<PlanningChange> {
    let value: String = row.get(1)?;
    let base: Option<String> = row.get(2)?;
    Ok(PlanningChange {
        id: row.get(0)?,
        value: serde_json::from_str(&value)?,
        base: base.map(|value| serde_json::from_str(&value)).transpose()?,
    })
}

pub(super) fn pending_deletion(
    conn: &Connection,
    task: &TaskId,
) -> StoreResult<Option<TaskDeletion>> {
    Ok(conn
        .query_row(
            "SELECT id,json_extract(base_json,'$.revision') FROM task_changes
        WHERE task_id=?1 AND field='deleted' AND acknowledged=0 AND conflict_json IS NULL
        AND seq=(SELECT max(seq) FROM task_changes WHERE task_id=?1 AND field='deleted')",
            [task.as_str()],
            |row| {
                Ok(TaskDeletion {
                    id: row.get(0)?,
                    base_revision: row.get(1)?,
                })
            },
        )
        .optional()?)
}

impl SqliteStore {
    pub fn pending_task_deletion(&self, task: &TaskId) -> StoreResult<Option<TaskDeletion>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        pending_deletion(&conn, task)
    }

    pub(crate) fn attempt_task_deletion(
        &self,
        task: &TaskId,
        receipt: &TaskDeletion,
        revision: &str,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if PlanningChanges::Task(task)
            .observed_revision(&tx)?
            .as_deref()
            != Some(revision)
        {
            return Ok(false);
        }
        let changed=tx.execute("UPDATE task_changes SET attempted=1,error=NULL WHERE id=?1 AND task_id=?2
            AND field='deleted' AND attempted=0 AND acknowledged=0 AND conflict_json IS NULL
            AND EXISTS(SELECT 1 FROM tasks WHERE id=?2 AND planning_deleted_at IS NOT NULL)
            AND seq=(SELECT max(seq) FROM task_changes WHERE task_id=?2 AND field='deleted')
            AND NOT EXISTS(SELECT 1 FROM task_changes WHERE task_id=?2 AND field='deleted' AND attempted=1 AND acknowledged=0 AND conflict_json IS NULL)",params![receipt.id,task.as_str()])?==1;
        tx.commit()?;
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};
    use serde_json::json;

    use super::PlanningChanges;
    use crate::durable::TaskId;
    use crate::store::migrations::{
        apply_before_migration, migration_section_sql, upgrade_from_migration,
    };
    use crate::work::edit::WorkValue;

    #[test]
    fn released_attempt_acknowledgment_rebases_without_clearing_later_save() {
        let conn = Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "planning_peers");
        conn.execute_batch(include_str!(
            "../../../tests/fixtures/migrations/creation_baselines.sql"
        ))
        .unwrap();
        let sql = migration_section_sql("planning_peers");
        conn.execute_batch(&sql).unwrap();
        crate::store::migrations::convert_released_data(&conn, &sql).unwrap();
        let task = TaskId::from_raw("task-receipt");
        let owner = PlanningChanges::Task(&task);
        let before = owner.pending(&conn).unwrap();
        let membership = before.iter().find(|c| c.id == "null-membership").unwrap();
        assert!(membership.base.is_some());
        assert_eq!(membership.base.as_ref().unwrap().value, None);
        let mut observed = vec![WorkValue::TaskTitle(Some("Attempted title".into()))];
        owner
            .reconcile_edits(&conn, Some("2026-10-10T00:00:00Z"), &mut observed)
            .unwrap();
        assert_eq!(
            observed,
            vec![WorkValue::TaskTitle(Some("Later save".into()))]
        );
        assert!(conn
            .query_row(
                "SELECT acknowledged FROM task_changes WHERE id='attempted-title'",
                [],
                |row| row.get::<_, bool>(0)
            )
            .unwrap());
        assert!(!conn
            .query_row(
                "SELECT acknowledged FROM task_changes WHERE id='later-title'",
                [],
                |row| row.get::<_, bool>(0)
            )
            .unwrap());
        let pending = owner.pending(&conn).unwrap();
        let later = pending.iter().find(|c| c.id == "later-title").unwrap();
        assert_eq!(
            later.base.as_ref().unwrap().value,
            Some(WorkValue::TaskTitle(Some("Attempted title".into())))
        );
        assert_eq!(later.value, WorkValue::TaskTitle(Some("Later save".into())));
        assert_eq!(
            pending.iter().find(|c| c.id == "null-membership").unwrap(),
            membership
        );
    }

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
                apply_before_migration(&conn, "planning_peers");
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
                upgrade_from_migration(&conn, "planning_peers");
                let task = TaskId::from_raw("task-proof");
                assert!(PlanningChanges::Task(&task)
                    .record(
                        &conn,
                        &if field_name == "project_id" {
                            WorkValue::TaskProject(Some("project-proof".into()))
                        } else {
                            WorkValue::TaskAssignee(Some("before".into()))
                        },
                        &if field_name == "project_id" {
                            WorkValue::TaskProject(Some("desired-project".into()))
                        } else {
                            WorkValue::TaskAssignee(Some("after".into()))
                        }
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
                    expected.map(|mut expected| {
                        expected["value"]=json!({"field":if field_name=="project_id" {"task_project"} else {"task_assignee"},"value":expected["value"]});
                        expected
                    })
                );
            }
        }
    }
}
