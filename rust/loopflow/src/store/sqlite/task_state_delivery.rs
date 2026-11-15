//! Delivery evidence for local Task decisions; never another lifecycle owner.

use super::planning_write;
use crate::work::edit::WorkValue as Edit;
use rusqlite::{params, Connection, OptionalExtension};

use crate::durable::TaskId;
use crate::linear::StateHistory;
use crate::store::planning_sync::{PlanningChangeKind, PlanningChangeRef};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

#[derive(Debug, Clone)]
pub(crate) struct TaskStateDelivery {
    pub id: String,
    pub task_id: TaskId,
    pub target: String,
    pub attempted: bool,
    pub base_revision: Option<String>,
    pub base_state: Option<String>,
}

fn read_delivery(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskStateDelivery> {
    Ok(TaskStateDelivery {
        id: row.get(0)?,
        task_id: TaskId::from_raw(row.get::<_, String>(1)?),
        target: row.get(2)?,
        attempted: row.get(3)?,
        base_revision: row.get(4)?,
        base_state: row.get(5)?,
    })
}

pub(super) fn queue_in(
    conn: &Connection,
    task: &TaskId,
    target: &str,
) -> StoreResult<PlanningChangeRef> {
    let completed_at = (target == "completed")
        .then(|| {
            time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)
        })
        .transpose()
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    planning_write::local(
        conn,
        &crate::work::edit::WorkEdit::for_target(
            task.as_str(),
            &[Edit::Disposition(crate::work::edit::Disposition {
                planning_state: Some(target.into()),
                planning_completed: i64::from(target == "completed"),
                planning_completed_at: completed_at,
            })],
        ),
    )?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO task_state_deliveries(id,task_id,target,base_revision,base_state)
         SELECT ?2,t.id,?3,i.revision,i.state
         FROM tasks t JOIN repository_plans w ON w.id=t.repository_id
         LEFT JOIN pm_items i ON i.id=t.external_issue_id AND i.repo=w.repo AND i.provider='linear'
         WHERE t.id=?1",
        params![task.as_str(), id, target],
    )?;
    Ok(PlanningChangeRef {
        kind: PlanningChangeKind::TaskState,
        owner: task.to_string(),
        id,
    })
}

/// Acquisition and delivery use the same transaction rule. No execution rows change.
/// Only state delivery, holding the Task's delivery lock, supplies known history.
pub(crate) fn reconcile_in(
    conn: &Connection,
    task: &TaskId,
    observed_revision: Option<&str>,
    observed: &crate::work::edit::Disposition,
    evidence: &str,
    history: StateHistory,
) -> StoreResult<bool> {
    let pending: Option<(TaskStateDelivery, Option<String>)> = conn.query_row(
        "SELECT d.id,d.task_id,d.target,d.attempted,d.base_revision,d.base_state,t.planning_provider_revision
         FROM task_state_deliveries d JOIN tasks t ON t.id=d.task_id
         WHERE d.task_id=?1 AND d.settled=0
         AND d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=?1)",
        [task.as_str()],
        |row| Ok((read_delivery(row)?, row.get(6)?)),
    ).optional()?;
    let Some((delivery, revision)) = pending else {
        return Ok(false);
    };
    let TaskStateDelivery {
        id,
        target,
        base_state,
        attempted,
        ..
    } = delivery;
    if crate::linear::observations::revision_nanos(observed_revision)?
        < crate::linear::observations::revision_nanos(revision.as_deref())?
    {
        return Ok(false);
    }
    let Some(state) = observed.planning_state.as_deref() else {
        return Ok(true);
    };
    if state == target {
        conn.execute(
            "UPDATE task_state_deliveries SET settled=1,error=NULL WHERE id=?1",
            [&id],
        )?;
        return Ok(false);
    }
    // An unchanged state is pending unless history proves an uncertain write
    // landed or the issue moved. Only proof of no effect re-arms the write.
    if observed.planning_state == base_state {
        match (attempted, history) {
            (false, _) | (true, StateHistory::Unknown) => return Ok(true),
            (true, StateHistory::Unchanged) => {
                conn.execute(
                    "UPDATE task_state_deliveries SET attempted=0,base_revision=?2 WHERE id=?1",
                    params![id, observed_revision],
                )?;
                return Ok(true);
            }
            (true, StateHistory::Changed) => {}
        }
    }
    conn.execute(
        "UPDATE task_state_deliveries SET settled=1,conflict_json=?2,error=NULL WHERE id=?1",
        params![id, evidence],
    )?;
    planning_write::local(
        conn,
        &crate::work::edit::WorkEdit::for_target(
            task.as_str(),
            &[Edit::Disposition(observed.clone())],
        ),
    )?;
    conn.execute(
        "UPDATE tasks SET planning_provider_revision=?2 WHERE id=?1",
        params![task.as_str(), observed_revision],
    )?;
    super::children::insert_task_event_in(conn, task, &crate::work::task::TaskEventKind::Progress {
        summary: format!("Adopted Linear state {state}; local intention {target} remains in delivery {id}. Local Workflow unchanged."),
    })?;
    Ok(false)
}

impl SqliteStore {
    pub(crate) fn pending_planning_tasks(
        &self,
        repo: &str,
    ) -> StoreResult<Vec<crate::work::task::Task>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare("SELECT t.id FROM tasks t JOIN repository_plans w ON w.id=t.repository_id
            WHERE w.repo=?1 AND t.external_issue_id IS NOT NULL AND (EXISTS(
                SELECT 1 FROM task_state_deliveries d WHERE d.task_id=t.id AND d.settled=0
                AND d.conflict_json IS NULL
                AND d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=t.id)) OR EXISTS(
                SELECT 1 FROM task_comments c JOIN task_comment_deliveries d ON d.comment_id=c.id
                WHERE c.task_id=t.id AND d.acknowledged=0 AND d.conflicting_comment_json IS NULL))")?;
        let ids = query
            .query_map([repo], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| {
                super::children::task_on(&conn, &TaskId::from_raw(id))?.ok_or(StoreError::NotFound)
            })
            .collect()
    }

    pub(crate) fn pending_task_state(
        &self,
        task: &TaskId,
    ) -> StoreResult<Option<TaskStateDelivery>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT d.id,d.task_id,d.target,d.attempted,d.base_revision,d.base_state
             FROM task_state_deliveries d JOIN tasks t ON t.id=d.task_id
             WHERE d.task_id=?1 AND d.settled=0 AND t.external_issue_id IS NOT NULL
             AND d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=t.id)",
            [task.as_str()],
            read_delivery,
        )
        .optional()
        .map_err(StoreError::from)
    }

    /// Acquire the effect only while the captured decision is still current.
    /// A canceled request remains attempted, so a later read must resolve it.
    pub(crate) fn attempt_task_state(
        &self,
        delivery: &TaskStateDelivery,
        retry: bool,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE task_state_deliveries SET attempted=1
             WHERE id=?1 AND (attempted=0 OR ?3) AND settled=0 AND conflict_json IS NULL
             AND seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=?2)",
            params![delivery.id, delivery.task_id.as_str(), retry],
        )? == 1)
    }

    /// Positive rate-limit rejection is not an uncertain provider effect.
    pub(crate) fn reject_task_state(&self, delivery: &TaskStateDelivery) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute("UPDATE task_state_deliveries SET attempted=0 WHERE id=?1 AND settled=0 AND conflict_json IS NULL", [&delivery.id])?;
        Ok(())
    }

    /// Failed delivery retains its receipt; only provider observation settles state.
    pub(crate) fn task_state_error(
        &self,
        delivery: &TaskStateDelivery,
        error: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE task_state_deliveries SET error=?2
             WHERE id=?1 AND settled=0 AND conflict_json IS NULL AND error IS NOT ?2",
            params![delivery.id, error],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::durable::TaskId;
    use crate::store::planning_sync::PlanningChangeKind;
    use crate::store::sqlite::operations::PlanningDeliveryState;
    use crate::store::sqlite::SqliteStore;
    use crate::work::task::NewTask;

    #[test]
    fn completion_and_reopen_keep_their_exact_state_delivery_across_restart() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let repository_id = store.ensure_repository("/state-recovery").unwrap();
        let task = store
            .create_task(&NewTask {
                id: TaskId::new(),
                repository_id,
                project_id: None,
                title: "State recovery".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let epoch = store.reading_identity().unwrap().1;
        for (id, kind) in [
            ("complete", "task.complete"),
            ("reopen", "task.reopen"),
            ("noop", "task.reopen"),
        ] {
            store
                .admit_operation(&epoch, id, kind, "hash", &serde_json::json!({}))
                .unwrap();
            store.dispatch_operation(id).unwrap();
            crate::journal::request::with_operation(Some(id.into()), || {
                if id == "complete" {
                    let request = store
                        .request_task_completion(&task.id, None)
                        .unwrap()
                        .unwrap();
                    assert!(store.complete_task(&task, request).unwrap());
                } else {
                    store.reopen_task(&task.id, None).unwrap();
                }
            });
            store.finish_operation(id, Err("output lost")).unwrap();
        }
        let completed = store.operation_receipt("complete").unwrap().unwrap();
        let reopened = store.operation_receipt("reopen").unwrap().unwrap();
        let completion = &completed.commits.last().unwrap().planning_changes[0];
        let reopening = &reopened.commits[0].planning_changes[0];
        assert_eq!(completion.kind, PlanningChangeKind::TaskState);
        assert_eq!(completion.owner, task.id.as_str());
        assert_ne!(completion.id, reopening.id);
        assert_eq!(
            completed.planning.as_ref().unwrap().changes[0].state,
            PlanningDeliveryState::Superseded
        );
        assert_eq!(
            reopened.planning.as_ref().unwrap().changes[0].state,
            PlanningDeliveryState::Pending
        );
        let noop = store.operation_receipt("noop").unwrap().unwrap();
        assert!(noop.commits[0].planning_changes.is_empty());
        assert!(noop.planning.is_none());

        // A newer intent cannot certify an earlier external writer's outcome.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE task_state_deliveries SET attempted=1,error='response lost' WHERE id=?1",
                [&completion.id],
            )
            .unwrap();
        assert_eq!(
            store
                .operation_receipt("complete")
                .unwrap()
                .unwrap()
                .planning
                .unwrap()
                .changes[0]
                .state,
            PlanningDeliveryState::Uncertain
        );
        let completion_id = completion.id.clone();
        let reopening_id = reopening.id.clone();
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        for (id, stage, saved) in [
            ("complete", "task.completed", &completed),
            ("reopen", "task.reopened", &reopened),
        ] {
            assert!(store.recover_operation_result(id, stage).unwrap());
            let recovered = store.operation_receipt(id).unwrap().unwrap();
            assert_eq!(
                recovered.result.as_ref(),
                Some(&saved.commits.last().unwrap().result)
            );
            assert_eq!(recovered.error.as_deref(), Some("output lost"));
            assert_eq!(
                recovered.planning.unwrap().changes[0].change,
                saved.commits.last().unwrap().planning_changes[0]
            );
        }
        assert_eq!(
            store.task(&task.id).unwrap().unwrap().state.as_deref(),
            Some("unstarted")
        );
        // A settled conflict records adoption, not delivery of the local value.
        store.conn.lock().unwrap().execute(
            "UPDATE task_state_deliveries SET settled=1,conflict_json='{}',error=NULL WHERE id=?1",
            [&reopening_id],
        ).unwrap();
        assert_eq!(
            store
                .operation_receipt("reopen")
                .unwrap()
                .unwrap()
                .planning
                .unwrap()
                .changes[0]
                .state,
            PlanningDeliveryState::Conflict
        );
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM task_state_deliveries WHERE id=?1",
                [&completion_id],
            )
            .unwrap();
        assert_eq!(
            store
                .operation_receipt("complete")
                .unwrap()
                .unwrap()
                .planning
                .unwrap()
                .changes[0]
                .state,
            PlanningDeliveryState::Unavailable
        );
    }
}
