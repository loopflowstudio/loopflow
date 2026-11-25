use crate::steer::Steer;
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use time::OffsetDateTime;

use crate::child::ChildRef;
use crate::execution::{AbandonReceipt, TaskState};
use crate::rows::now_unix;
use crate::work::task::TaskEventKind;
use crate::{StoreError, StoreResult};
use loopflow_model::durable::{Author, MachineId, TaskId, WorkRef, WorkStatus};
use loopflow_model::id::WaveId;

use super::SqliteStore;

impl SqliteStore {
    /// A Task may launch a Graph while it is ready and not being abandoned.
    pub fn require_task_launch(&self, task_id: &TaskId) -> StoreResult<()> {
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let conn = connection.savepoint()?;
        let task = super::children::task_on(&conn, task_id)?.ok_or(StoreError::NotFound)?;
        super::children::require_task_planning(&conn, &task)?;
        let work = WorkRef::Task(task_id.clone());
        require_ready_work(&conn, &work)
    }

    pub fn task_issue_identifier(&self, external_issue_id: &str) -> StoreResult<Option<String>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        let id = conn
            .query_row(
                "SELECT id FROM tasks WHERE external_issue_id=?1",
                [external_issue_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        id.map(|id| super::children::short_task_id(&conn, &TaskId::from_raw(id)))
            .transpose()
            .map(Option::flatten)
    }

    /// The name teammates see for this Machine, when it is public.
    pub fn local_public_name(&self) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        local_public_name_in(&conn)
    }

    /// Make this Machine public under `name`, or private again with `None`.
    pub fn set_local_public_name(&self, name: Option<&str>) -> StoreResult<()> {
        if name.is_some_and(|name| name.trim().is_empty()) {
            return Err(StoreError::InvalidData(
                "a public Machine name cannot be empty".into(),
            ));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE machines SET public_name=?1 WHERE is_local=1",
            [name.map(str::trim)],
        )?;
        Ok(())
    }

    pub fn register_machine(&self, machine_id: &MachineId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO machines(id,is_local,created_at,observed_at) VALUES(?1,0,?2,?2)
             ON CONFLICT(id) DO UPDATE SET observed_at=excluded.observed_at",
            params![machine_id.as_str(), now_unix()],
        )?;
        Ok(())
    }

    pub fn saved_machine(&self, machine_id: &MachineId) -> StoreResult<Option<SavedMachine>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        map_machine_by_id(&conn, machine_id)
    }

    pub fn saved_machines(&self) -> StoreResult<Vec<SavedMachine>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query =
            conn.prepare_cached("SELECT id,is_local,created_at,observed_at FROM machines ORDER BY id")?;
        let rows = query
            .query_map([], |row| {
                Ok(SavedMachine {
                    id: MachineId::from_raw(row.get::<_, String>(0)?),
                    local: row.get(1)?,
                    created_at: row.get(2)?,
                    observed_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn saved_local_machine(&self) -> StoreResult<SavedMachine> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        map_local_machine(&conn)
    }

    pub fn abandon(&self, work: &WorkRef, reason: &str) -> StoreResult<AbandonReceipt> {
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(StoreError::InvalidData(
                "abandon reason cannot be empty".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = now_unix();
        let (table, id) = work_table(work);
        if let WorkRef::Task(task_id) = work {
            let retained: Option<(i64, Option<String>)> = tx.query_row(
                "SELECT abandoned_at,abandon_reason FROM tasks WHERE id=?1 AND abandoned_at IS NOT NULL",
                [task_id.as_str()], |row| Ok((row.get(0)?, row.get(1)?)),
            ).optional()?;
            if let Some((at, saved_reason)) = retained {
                // Migrated cancellations can predate their state. An explicit
                // repeat repairs it; one already saved changes nothing.
                super::task_state::decide_in(&tx, task_id, "canceled")?;
                tx.commit()?;
                return Ok(AbandonReceipt {
                    work: work.clone(),
                    reason: saved_reason.unwrap_or_else(|| reason.to_string()),
                    abandoned_at: OffsetDateTime::from_unix_timestamp(at)
                        .map_err(|error| StoreError::InvalidData(error.to_string()))?,
                });
            }
        }
        let abandoned = match work {
            WorkRef::Task(task_id) => abandon_task_in(&tx, task_id, reason, now)?,
            _ => {
                tx.execute(
                    &format!(
                        "UPDATE {table} SET work_state='abandoned', work_terminal_at=?2
                         WHERE id=?1 AND work_state='ready'"
                    ),
                    params![id, now],
                )? == 1
            }
        };
        if !abandoned {
            return Err(StoreError::InvalidAuthority(format!(
                "{} {} is not ready",
                work.kind(),
                work.id()
            )));
        }
        tx.commit()?;
        Ok(AbandonReceipt {
            work: work.clone(),
            reason: reason.to_string(),
            abandoned_at: OffsetDateTime::from_unix_timestamp(now)
                .expect("current Unix timestamp must be valid"),
        })
    }

    pub fn work_status(&self, work: &WorkRef) -> StoreResult<WorkStatus> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        work_status_in(&conn, work)
    }

    pub fn task_state(&self, task: &TaskId) -> StoreResult<TaskState> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        task_state_in(&conn, task)
    }

    pub fn work_for_child(&self, target: &ChildRef) -> StoreResult<WorkRef> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        work_for_child_in(&conn, target)
    }

    pub fn task_steers(&self, task_id: &TaskId) -> StoreResult<Vec<Steer>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT {} FROM steers p WHERE task_id=?1 ORDER BY seq",
            super::steers::COLUMNS
        ))?;
        let steers = query
            .query_map([task_id.as_str()], super::steers::read_steer)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(steers)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn save_steer(&self, work: &WorkRef, author: &Author, text: &str) -> StoreResult<Steer> {
        let WorkRef::Task(task) = work else {
            return Err(StoreError::InvalidData(
                "only Tasks take test steers".into(),
            ));
        };
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let steer = Self::save_task_steer_in(
            &tx,
            task,
            author,
            text,
            &uuid::Uuid::new_v4().to_string(),
            "1",
        )?;
        tx.commit()?;
        Ok(steer)
    }

    pub fn save_task_steer_in(
        tx: &Transaction<'_>,
        task: &TaskId,
        author: &Author,
        text: &str,
        source: &str,
        revision: &str,
    ) -> StoreResult<Steer> {
        let steer =
            super::steers::save_in(tx, ("task", task.as_str()), source, revision, author, text)?;
        super::children::insert_task_event_in(
            tx,
            task,
            &TaskEventKind::Steer {
                steer_id: steer.id.clone(),
            },
        )?;
        Ok(steer)
    }

    /// Append a Task interrupt. A live run ends its current turn when it sees
    /// an interrupt newer than its launch cursor.
    pub fn append_interrupt(&self, task: &TaskId) -> StoreResult<i64> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_ready_work(&tx, &WorkRef::Task(task.clone()))?;
        let id = super::children::insert_task_event_in(&tx, task, &TaskEventKind::Interrupt)?.id;
        tx.commit()?;
        Ok(id)
    }

    /// The id of the newest interrupt request on the Task, or 0 when there is
    /// none. A run stamps this at launch and re-reads it; a higher value means a
    /// new interrupt to act on.
    pub fn latest_interrupt_id(&self, task: &TaskId) -> StoreResult<i64> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT COALESCE(MAX(id), 0) FROM task_events
             WHERE task_id=?1 AND json_extract(kind_json, '$.kind')='interrupt'",
            [task.as_str()],
            |row| row.get(0),
        )
        .map_err(StoreError::from)
    }
}

pub(super) fn local_public_name_in(conn: &Connection) -> StoreResult<Option<String>> {
    conn.query_row(
        "SELECT public_name FROM machines WHERE is_local=1",
        [],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

/// The saved Machine identity and observation times; routes and labels are configuration.
#[derive(Debug, Clone)]
pub struct SavedMachine {
    pub id: MachineId,
    pub local: bool,
    pub created_at: i64,
    pub observed_at: i64,
}

pub(super) fn map_local_machine(conn: &Connection) -> StoreResult<SavedMachine> {
    let id: String = conn.query_row("SELECT id FROM machines WHERE is_local=1", [], |row| {
        row.get(0)
    })?;
    map_machine_by_id(conn, &MachineId::from_raw(id))?.ok_or(StoreError::NotFound)
}

pub(super) fn map_machine_by_id(
    conn: &Connection,
    machine_id: &MachineId,
) -> StoreResult<Option<SavedMachine>> {
    Ok(conn
        .query_row(
            "SELECT is_local,created_at,observed_at FROM machines WHERE id=?1",
            [machine_id.as_str()],
            |row| {
                Ok(SavedMachine {
                    id: machine_id.clone(),
                    local: row.get(0)?,
                    created_at: row.get(1)?,
                    observed_at: row.get(2)?,
                })
            },
        )
        .optional()?)
}

/// Completion and abandonment are durable. An open Task's state follows
/// what it is doing: its Task run's carrier still has an OS process, or a
/// Graph run in its work has not ended. The auto setting says who takes the
/// interactive steps; state never reads it.
/// A Task with neither a checkout nor a Graph taken up is unstarted; creation
/// admits a Graph before setup makes the checkout.
pub(crate) fn task_state_sql(t: &str) -> String {
    format!(
        "CASE WHEN {t}.abandoned_at IS NOT NULL THEN 'abandoned' \
         WHEN {t}.planning_state='completed' THEN 'done' \
         WHEN EXISTS(SELECT 1 FROM task_graphs tg WHERE tg.task_id={t}.id AND tg.run_pid IS NOT NULL) \
             OR EXISTS({open_graphs}) \
         THEN 'moving' \
         WHEN EXISTS(SELECT 1 FROM task_graphs tg WHERE tg.task_id={t}.id \
             AND tg.lf_process_id IS NULL AND tg.step NOT IN ('start','end')) THEN 'active' \
         WHEN NOT EXISTS(SELECT 1 FROM task_checkouts c WHERE c.task_id={t}.id) \
             AND NOT EXISTS(SELECT 1 FROM task_graphs tg WHERE tg.task_id={t}.id) THEN 'unstarted' \
         ELSE 'idle' END",
        open_graphs = super::task_work::task_open_graph_pairs(&format!("{t}.id")),
    )
}

/// SQL: Task row `t` is neither done nor abandoned.
pub(crate) fn task_open_sql(t: &str) -> String {
    format!("({t}.abandoned_at IS NULL AND {t}.planning_state IS NOT 'completed')")
}

/// Abandon an open Task at `at`. `false` when it is already done or abandoned.
pub(super) fn abandon_task_in(
    conn: &Connection,
    task: &TaskId,
    reason: &str,
    at: i64,
) -> StoreResult<bool> {
    let abandoned = conn.execute(
        &format!(
            "UPDATE tasks AS t SET abandoned_at=?2,abandon_reason=?3 WHERE t.id=?1 AND {}",
            task_open_sql("t")
        ),
        params![task.as_str(), at, reason],
    )? == 1;
    if abandoned {
        super::task_state::decide_in(conn, task, "canceled")?;
        super::task_summaries::write_in(conn, task, super::task_summaries::TaskEnd::Abandoned)?;
    }
    Ok(abandoned)
}

pub(crate) fn task_state_in(conn: &Connection, task: &TaskId) -> StoreResult<TaskState> {
    let state: String = conn.query_row(
        &format!("SELECT {} FROM tasks t WHERE t.id=?1", task_state_sql("t")),
        [task.as_str()],
        |row| row.get(0),
    )?;
    TaskState::parse(&state)
        .ok_or_else(|| StoreError::InvalidData(format!("invalid Task state {state:?}")))
}

pub(crate) fn work_status_in(conn: &Connection, work: &WorkRef) -> StoreResult<WorkStatus> {
    if let WorkRef::Task(task) = work {
        return Ok(task_state_in(conn, task)?.work_status());
    }
    let (table, id) = work_table(work);
    let state: String = conn.query_row(
        &format!("SELECT work_state FROM {table} WHERE id=?1"),
        [id],
        |row| row.get(0),
    )?;
    match state.as_str() {
        "ready" => Ok(WorkStatus::Ready),
        "done" => Ok(WorkStatus::Done),
        "abandoned" => Ok(WorkStatus::Abandoned),
        other => Err(StoreError::InvalidData(format!(
            "invalid {} Work state {other:?}",
            work.kind()
        ))),
    }
}

pub(super) fn require_ready_work(conn: &Connection, work: &WorkRef) -> StoreResult<()> {
    match work_status_in(conn, work)? {
        WorkStatus::Ready => Ok(()),
        status => Err(StoreError::InvalidAuthority(format!(
            "{} {} is {status}",
            work.kind(),
            work.id()
        ))),
    }
}

fn work_table(work: &WorkRef) -> (&'static str, &str) {
    match work {
        WorkRef::Wave(id) => ("waves", id.as_str()),
        WorkRef::Project(id) => ("projects", id.as_str()),
        WorkRef::Task(id) => ("tasks", id.as_str()),
    }
}

pub(super) fn task_wave_in(conn: &Connection, task: &TaskId) -> StoreResult<Option<WaveId>> {
    let wave: Option<String> = conn
        .query_row(
            "SELECT p.wave_id FROM tasks t LEFT JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
            [task.as_str()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| invalid_durable(format!("Task {task} does not exist")))?;
    wave.map(|wave| WaveId::parse(&wave).map_err(invalid_durable))
        .transpose()
}

fn invalid_durable(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

pub(crate) fn work_for_child_in(conn: &Connection, target: &ChildRef) -> StoreResult<WorkRef> {
    match target {
        ChildRef::Project(project_id) => {
            conn.query_row(
                "SELECT 1 FROM projects WHERE id=?1",
                [project_id.as_str()],
                |_| Ok(()),
            )?;
            Ok(WorkRef::Project(project_id.clone()))
        }
        ChildRef::Task(task_id) => {
            conn.query_row(
                "SELECT 1 FROM tasks WHERE id=?1",
                [task_id.as_str()],
                |_| Ok(()),
            )?;
            Ok(WorkRef::Task(task_id.clone()))
        }
    }
}

#[cfg(test)]
mod durable_store_tests {
    use crate::test_fixture::{
        conversation, store_with_task, unpublished_conversation, unregistered_task,
    };

    use std::sync::{Arc, Barrier};
    use std::thread;

    use super::super::sessions::reserve_session_in;
    use loopflow_model::durable::TaskId;

    use crate::session::WorkSource;
    use loopflow_model::id::WaveId;

    use crate::work::task::TaskEventKind;

    #[test]
    fn task_events_reject_missing_tasks_without_returning_a_previous_event() {
        let (_dir, store, task) = store_with_task();
        let kind = TaskEventKind::Progress {
            summary: "work retained".into(),
        };
        let event = store.append_task_event(&task, &kind).unwrap();
        assert_eq!(event.task_id, task);
        assert_eq!(event.kind, kind);
        assert!(matches!(
            store.append_task_event(&TaskId::new(), &kind),
            Err(crate::StoreError::NotFound)
        ));
        assert_eq!(store.task_events_after(&task, 0).unwrap(), vec![event]);
    }

    #[test]
    fn session_admission_infers_ancestors_and_rejects_conflicts_atomically() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let mut conn = store.conn.lock().unwrap();
        let other_wave = WaveId::new();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'other','/repo',1)",
            [other_wave.as_str()],
        )
        .unwrap();
        let other_task = TaskId::new();
        conn.execute("INSERT INTO tasks(id,repository_id,project_id,external_issue_id,issue_identifier,created_at)
            SELECT ?1,repository_id,project_id,?1,?1,1 FROM tasks WHERE id=?2",
            rusqlite::params![other_task.as_str(), task_id.as_str()]).unwrap();
        conn.execute(
            "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,'/repo.other')",
            [other_task.as_str()],
        )
        .unwrap();
        for (invocation, task_input, wave_input, expected_task, expected_wave) in [
            (None, None, None, None, None),
            (
                None,
                None,
                Some(other_wave.clone()),
                None,
                Some(other_wave.clone()),
            ),
            (
                None,
                Some(task_id.clone()),
                None,
                Some(task_id.clone()),
                Some(task.wave_id.clone().unwrap()),
            ),
            (
                None,
                Some(task_id.clone()),
                Some(task.wave_id.clone().unwrap()),
                Some(task_id.clone()),
                Some(task.wave_id.clone().unwrap()),
            ),
        ] {
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .unwrap();
            let saved = super::super::sessions::reserve_session_in(
                &tx,
                conversation(invocation, task_input, wave_input),
                None,
            )
            .unwrap();
            assert_eq!(saved.task_id, expected_task);
            assert_eq!(saved.wave_id, expected_wave);
            tx.commit().unwrap();
            let read = super::super::sessions::session_in(&conn, &saved.id)
                .unwrap()
                .unwrap();
            assert_eq!(read, saved);
        }
        let run_count: i64 = conn
            .query_row("SELECT count(*) FROM agent_sessions", [], |row| row.get(0))
            .unwrap();
        for (invocation, task_input, wave_input) in [
            (None, Some(task_id.clone()), Some(other_wave.clone())),
            (None, Some(other_task), Some(other_wave.clone())),
            (None, Some(TaskId::new()), None),
            (None, None, Some(WaveId::new())),
        ] {
            let mut requested = conversation(invocation, task_input, wave_input);
            requested.id = "failed-conversation".into();
            {
                let tx = conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .unwrap();
                assert!(super::super::sessions::reserve_session_in(&tx, requested, None).is_err());
            }
            assert_eq!(
                conn.query_row("SELECT count(*) FROM agent_sessions", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                run_count
            );
            assert_eq!(
                conn.query_row(
                    "SELECT count(*) FROM agent_sessions WHERE id='failed-conversation'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
                0
            );
        }
    }

    #[test]
    fn session_reservation_serializes_with_project_ancestry_changes() {
        let (dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let other_wave = WaveId::new();
        let mut conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'other','/repo',1)",
            [other_wave.as_str()],
        )
        .unwrap();
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let writer_barrier = barrier.clone();
        let project_id = task.project_id.unwrap();
        let destination = other_wave.clone();
        let db = dir.path().join("loopflow.db");
        let writer = thread::spawn(move || {
            let conn = rusqlite::Connection::open(db).unwrap();
            conn.busy_timeout(super::super::SQLITE_WRITE_BUSY_TIMEOUT)
                .unwrap();
            conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
            writer_barrier.wait();
            conn.execute(
                "UPDATE projects SET wave_id=?1 WHERE id=?2",
                rusqlite::params![destination.as_str(), project_id.as_str()],
            )
        });
        barrier.wait();
        let mut input = unpublished_conversation(Some(task_id), None, 100);
        input.work_source = Some(WorkSource::Checkout);
        let saved = reserve_session_in(&tx, input, None).unwrap();
        tx.commit().unwrap();
        assert_eq!(writer.join().unwrap().unwrap(), 1);
        let (run_wave, project_wave): (String, String) = conn
            .query_row(
                "SELECT r.wave_id,p.wave_id FROM agent_sessions r JOIN tasks t ON t.id=r.task_id
             JOIN projects p ON p.id=t.project_id WHERE r.id=?1",
                [saved.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(saved.wave_id, task.wave_id);
        assert_eq!(run_wave, other_wave.as_str());
        assert_eq!(run_wave, project_wave);
    }

    #[test]
    fn project_evidence_marks_taskless_graphs_started_in_their_checkout() {
        let (dir, store, existing) = store_with_task();
        // A checkout recorded without an ownership entry, so only the Graph is evidence.
        let (task, _) = unregistered_task(&store, &existing, dir.path().join("unclaimed"));
        store.seed_unplaced_task(&task);
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,?2)",
                rusqlite::params![
                    task.id.as_str(),
                    task.worktree.as_ref().unwrap().to_str().unwrap()
                ],
            )
            .unwrap();
        assert!(!store.task_begun(&task.id).unwrap());
        // A Graph run from the checkout begins its Task without an explicit bind.
        store.test_graph(
            "sync",
            &task.worktree.as_ref().unwrap().to_string_lossy(),
            &[("sync --plan", None)],
            None,
        );
        assert!(store.task_begun(&task.id).unwrap());
        // A Graph is evidence of work, not a claim.
        assert!(!store.task_owned(&task.id).unwrap());
    }

    #[test]
    fn session_assignment_rejects_reassignment_atomically() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let mut conn = store.conn.lock().unwrap();
        let bound_task = TaskId::new();
        let losing_task = TaskId::new();
        for id in [&bound_task, &losing_task] {
            conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,created_at)
                VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,?1,?1,1)",
                rusqlite::params![id.as_str(), task.project_id.as_ref().unwrap().as_str()]).unwrap();
        }
        // A done Task is still a valid assignment target.
        conn.execute(
            "UPDATE tasks SET planning_state='completed' WHERE id=?1",
            [bound_task.as_str()],
        )
        .unwrap();
        let other_wave = WaveId::new();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'other','/repo',1)",
            [other_wave.as_str()],
        )
        .unwrap();
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        let reserved = reserve_session_in(
            &tx,
            unpublished_conversation(Some(task_id.clone()), None, 1),
            None,
        )
        .unwrap();
        let orphan =
            reserve_session_in(&tx, unpublished_conversation(None, None, 2), None).unwrap();
        let wave_only = reserve_session_in(
            &tx,
            unpublished_conversation(None, Some(task.wave_id.clone().unwrap()), 3),
            None,
        )
        .unwrap();
        let conflicting = reserve_session_in(
            &tx,
            unpublished_conversation(None, Some(other_wave.clone()), 4),
            None,
        )
        .unwrap();
        tx.commit().unwrap();
        let session_task = |conn: &rusqlite::Connection, id: &str| -> Option<String> {
            conn.query_row(
                "SELECT task_id FROM agent_sessions WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .unwrap()
        };
        conn.execute(
            "UPDATE agent_sessions SET task_id=?2,wave_id=?3,work_source='bound' WHERE id=?1",
            rusqlite::params![
                orphan.id.as_str(),
                bound_task.as_str(),
                task.wave_id.as_ref().unwrap().as_str()
            ],
        )
        .unwrap();
        conn.execute(
            "UPDATE agent_sessions SET task_id=?2,work_source='bound' WHERE id=?1",
            rusqlite::params![wave_only.id.as_str(), bound_task.as_str()],
        )
        .unwrap();
        for (id, new_task, new_wave) in [
            (
                &orphan.id,
                Some(losing_task.as_str()),
                Some(task.wave_id.as_ref().unwrap().as_str()),
            ),
            (
                &orphan.id,
                None,
                Some(task.wave_id.as_ref().unwrap().as_str()),
            ),
            (
                &conflicting.id,
                Some(losing_task.as_str()),
                Some(task.wave_id.as_ref().unwrap().as_str()),
            ),
            (&conflicting.id, None, None),
        ] {
            assert!(conn
                .execute(
                    "UPDATE agent_sessions SET task_id=?2,wave_id=?3 WHERE id=?1",
                    rusqlite::params![id.as_str(), new_task, new_wave]
                )
                .is_err());
            assert_eq!(
                session_task(&conn, orphan.id.as_str()).as_deref(),
                Some(bound_task.as_str())
            );
            assert_eq!(session_task(&conn, conflicting.id.as_str()), None);
        }
        assert!(conn
            .execute(
                "DELETE FROM agent_sessions WHERE id=?1",
                [reserved.id.as_str()]
            )
            .is_err());
        // Assignment claims nothing.
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM task_ownership WHERE task_id IN (?1,?2)",
                [bound_task.as_str(), losing_task.as_str()],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    }
}
