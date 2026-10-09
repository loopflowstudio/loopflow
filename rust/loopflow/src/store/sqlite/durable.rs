use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use time::OffsetDateTime;

use crate::child::ChildRef;
use crate::durable::{
    AbandonReceipt, Author, Machine, MachineId, Placement, PlacementProvenance, ProjectId, Steer,
    SteerComment, TaskId, TaskState, ToolResponseId, ToolResponseReceipt, ToolResponseWrite,
    WorkRef, WorkStatus,
};
use crate::id::WaveId;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::work::project::ProjectEventKind;
use crate::work::task::TaskEventKind;

use super::SqliteStore;

impl SqliteStore {
    /// A Task may launch a Flow while it is ready, not being abandoned, and
    /// planned in its Wave's current chapter.
    pub(crate) fn require_task_launch(&self, task_id: &TaskId) -> StoreResult<()> {
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let conn = connection.transaction()?;
        let task = super::children::task_on(&conn, task_id)?.ok_or(StoreError::NotFound)?;
        super::children::require_task_planning(&conn, &task)?;
        let work = WorkRef::Task(task_id.clone());
        require_ready_work(&conn, &work)?;
        let abandoning: bool = conn.query_row(
            "SELECT abandon_requested_at IS NOT NULL FROM tasks WHERE id=?1",
            [task_id.as_str()],
            |row| row.get(0),
        )?;
        if abandoning {
            return Err(StoreError::InvalidAuthority(
                "Task cancellation is pending; retry task abandon".into(),
            ));
        }
        require_current_task_project(&conn, &work)
    }

    pub(crate) fn task_issue_identifier(
        &self,
        external_issue_id: &str,
    ) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT issue_identifier FROM tasks WHERE external_issue_id=?1",
            [external_issue_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn machine_by_id(&self, machine_id: &MachineId) -> StoreResult<Option<Machine>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        map_machine_by_id(&conn, machine_id)
    }

    pub fn local_machine(&self) -> StoreResult<Machine> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        map_local_machine(&conn)
    }

    pub fn add_machine(
        &self,
        machine_id: &MachineId,
        target: &str,
        label: &str,
        repo: &str,
    ) -> StoreResult<Machine> {
        if target.is_empty() || target == "local" || label.trim().is_empty() || repo.is_empty() {
            return Err(StoreError::InvalidData(
                "machine target, label and repository must be nonempty; local is reserved".into(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if map_machine_by_id(&tx, machine_id)?.is_some_and(|machine| machine.route == "local") {
            return Err(StoreError::InvalidData(
                "cannot add the local machine as a remote".into(),
            ));
        }
        let now = now_unix();
        tx.execute(
            "INSERT INTO machines (id, route, label, repo, created_at, observed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(id) DO UPDATE SET route=excluded.route, label=excluded.label,
                repo=excluded.repo, observed_at=excluded.observed_at",
            params![machine_id.as_str(), target, label, repo, now],
        ).map_err(|error| StoreError::InvalidData(format!("could not save machine: {error}; choose another label or remove the old connection")))?;
        let machine = map_machine_by_id(&tx, machine_id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(machine)
    }

    pub fn machines(&self) -> StoreResult<Vec<Machine>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, route, created_at, observed_at, label, repo
             FROM machines WHERE label IS NOT NULL ORDER BY label",
        )?;
        let rows = stmt.query_map([], |row| Ok(map_machine_row(row)))?;
        rows.map(|row| row?).collect()
    }

    pub fn rename_machine(&self, label: &str, name: &str) -> StoreResult<()> {
        if name.trim().is_empty() {
            return Err(StoreError::InvalidData(
                "machine label cannot be empty".into(),
            ));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute(
            "UPDATE machines SET label=?2 WHERE label=?1",
            params![label, name],
        )? == 0
        {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn remove_machine(&self, label: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute(
            "UPDATE machines SET label=NULL, repo=NULL WHERE label=?1",
            [label],
        )? == 0
        {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn find_placement(&self, work: &WorkRef) -> StoreResult<Option<Placement>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        find_placement_in(&conn, work)
    }

    pub fn placement(&self, work: &WorkRef) -> StoreResult<Placement> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.transaction()?;
        placement_in(&snapshot, work)
    }

    pub(crate) fn place_work(
        &self,
        work: &WorkRef,
        machine_id: &MachineId,
    ) -> StoreResult<Placement> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_ready_work(&tx, work)?;
        tx.query_row(
            "SELECT 1 FROM machines WHERE id=?1",
            [machine_id.as_str()],
            |_| Ok(()),
        )?;
        if let Some(current) = find_placement_in(&tx, work)? {
            if current.machine_id == *machine_id
                && current.provenance == PlacementProvenance::Explicit
            {
                tx.commit()?;
                return Ok(current);
            }
        }
        let column = format!("{}_id", work.kind());
        tx.execute(
            &format!(
                "INSERT INTO work_placements ({column}, machine_id, enabled, placed_at, provenance)
                 VALUES (?1, ?2, 1, ?3, 'explicit')
                 ON CONFLICT({column}) DO UPDATE SET
                    machine_id=excluded.machine_id, placed_at=excluded.placed_at,
                    provenance=excluded.provenance"
            ),
            params![work.id(), machine_id.as_str(), now_unix()],
        )?;
        let placement = placement_in(&tx, work)?;
        tx.commit()?;
        Ok(placement)
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
                return Ok(AbandonReceipt {
                    work: work.clone(),
                    reason: saved_reason.unwrap_or_else(|| reason.to_string()),
                    abandoned_at: OffsetDateTime::from_unix_timestamp(at)
                        .map_err(|error| StoreError::InvalidData(error.to_string()))?,
                });
            }
        }
        let abandon = match work {
            WorkRef::Task(_) => format!(
                "UPDATE tasks AS t SET abandoned_at=?2 WHERE t.id=?1 AND {}",
                task_open_sql("t")
            ),
            _ => format!(
                "UPDATE {table} SET work_state='abandoned', work_terminal_at=?2
                 WHERE id=?1 AND work_state='ready'"
            ),
        };
        if tx.execute(&abandon, params![id, now])? != 1 {
            return Err(StoreError::InvalidAuthority(format!(
                "{} {} is not ready",
                work.kind(),
                work.id()
            )));
        }
        if let WorkRef::Task(task_id) = work {
            tx.execute(
                "UPDATE tasks SET abandon_requested_at=NULL,abandon_reason=?2 WHERE id=?1",
                params![task_id.as_str(), reason],
            )?;
            super::task_state_delivery::queue_in(&tx, task_id, "canceled")?;
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
        Ok(super::children::task_events_after_in(&conn, task_id, 0)?
            .into_iter()
            .filter_map(|event| match event.kind {
                TaskEventKind::Steer { author, text } => Some(Steer {
                    id: event.id,
                    author,
                    text,
                }),
                _ => None,
            })
            .collect())
    }

    /// Steer comments across every Work, issued at or after `since` (unix
    /// seconds) — the cross-Work `lf history` timeline. Scans the Task and
    /// Project event streams for the `Steer` comment; there is no steers table.
    pub fn steers_since(&self, since: i64) -> StoreResult<Vec<SteerComment>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut comments = Vec::new();
        let mut task_statement = conn.prepare(
            "SELECT task_id, id, kind_json, created_at FROM task_events
             WHERE json_extract(kind_json, '$.kind')='steer' AND created_at >= ?1
             ORDER BY created_at, id",
        )?;
        let task_rows = task_statement.query_map([since], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        for row in task_rows {
            let (task_id, id, kind_json, created_at) = row?;
            let kind: TaskEventKind = serde_json::from_str(&kind_json)?;
            if let TaskEventKind::Steer { author, text } = kind {
                comments.push(SteerComment {
                    work: WorkRef::Task(TaskId::parse(&task_id).map_err(invalid_durable)?),
                    steer: Steer { id, author, text },
                    issued_at: crate::store::rows::unix_to_datetime(created_at),
                });
            }
        }
        let mut project_statement = conn.prepare(
            "SELECT project_id, id, kind_json, created_at FROM project_events
             WHERE json_extract(kind_json, '$.kind')='steer' AND created_at >= ?1
             ORDER BY created_at, id",
        )?;
        let project_rows = project_statement.query_map([since], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        for row in project_rows {
            let (project_id, id, kind_json, created_at) = row?;
            let kind: ProjectEventKind = serde_json::from_str(&kind_json)?;
            if let ProjectEventKind::Steer { author, text } = kind {
                comments.push(SteerComment {
                    work: WorkRef::Project(ProjectId::parse(&project_id).map_err(invalid_durable)?),
                    steer: Steer { id, author, text },
                    issued_at: crate::store::rows::unix_to_datetime(created_at),
                });
            }
        }
        Ok(comments)
    }

    #[cfg(test)]
    pub fn append_steer(&self, work: &WorkRef, author: &Author, text: &str) -> StoreResult<Steer> {
        let WorkRef::Task(task_id) = work else {
            return Err(StoreError::InvalidData("only Tasks take steering".into()));
        };
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let steer = Self::append_task_steer_in(&tx, task_id, author, text)?;
        tx.commit()?;
        Ok(steer)
    }

    /// Local delivery projection. Authored input is published to Linear first.
    pub(crate) fn append_task_steer_in(
        tx: &Transaction<'_>,
        task_id: &TaskId,
        author: &Author,
        text: &str,
    ) -> StoreResult<Steer> {
        let text = text.trim();
        if text.is_empty() {
            return Err(StoreError::InvalidData("Steer text cannot be empty".into()));
        }
        let event = super::children::insert_task_event_in(
            tx,
            task_id,
            &TaskEventKind::Steer {
                author: author.clone(),
                text: text.to_string(),
            },
        )?;
        Ok(Steer {
            id: event.id,
            author: author.clone(),
            text: text.to_string(),
        })
    }

    /// Record an interrupt request as a durable comment on the Work's event
    /// stream. A live run observes it (past its launch cursor) and ends the
    /// current turn; with no live run it is inert history.
    pub fn append_interrupt(&self, work: &WorkRef) -> StoreResult<i64> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_ready_work(&tx, work)?;
        let id = match work {
            WorkRef::Task(task_id) => {
                super::children::insert_task_event_in(&tx, task_id, &TaskEventKind::Interrupt)?.id
            }
            WorkRef::Project(project_id) => {
                let project = tx.query_row(
                    super::children::PROJECT_SELECT,
                    params![project_id.as_str()],
                    super::children::map_project_row,
                )?;
                super::children::insert_project_event_in(
                    &tx,
                    &project,
                    &ProjectEventKind::Interrupt,
                )?
                .id
            }
            WorkRef::Wave(_) => {
                return Err(StoreError::InvalidData(
                    "Waves are not interrupted through the Work event stream".to_string(),
                ))
            }
        };
        tx.commit()?;
        Ok(id)
    }

    /// The id of the newest interrupt request on the Work, or 0 when there is
    /// none. A run stamps this at launch and re-reads it; a higher value means a
    /// new interrupt to act on.
    pub fn latest_interrupt_id(&self, work: &WorkRef) -> StoreResult<i64> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (table, column, id) = match work {
            WorkRef::Task(task_id) => ("task_events", "task_id", task_id.as_str().to_string()),
            WorkRef::Project(project_id) => (
                "project_events",
                "project_id",
                project_id.as_str().to_string(),
            ),
            WorkRef::Wave(_) => return Ok(0),
        };
        let sql = format!(
            "SELECT COALESCE(MAX(id), 0) FROM {table}
             WHERE {column}=?1 AND json_extract(kind_json, '$.kind')='interrupt'"
        );
        conn.query_row(&sql, params![id], |row| row.get(0))
            .map_err(StoreError::from)
    }

    pub fn write_tool_response(
        &self,
        work: &WorkRef,
        write: &ToolResponseWrite,
    ) -> StoreResult<(ToolResponseReceipt, bool)> {
        let choice = write.choice.trim();
        if choice.is_empty() {
            return Err(StoreError::InvalidData(
                "tool response choice cannot be empty".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_ready_work(&tx, work)?;
        if let Some(existing) = tool_response_in(&tx, work, &write.request_id)? {
            if existing.choice != choice {
                return Err(StoreError::InvalidData(format!(
                    "tool response {} is already resolved as {:?}",
                    write.request_id, existing.choice
                )));
            }
            return Ok((existing, false));
        }
        let receipt = ToolResponseReceipt {
            id: ToolResponseId::new(),
            work: work.clone(),
            request_id: write.request_id.clone(),
            choice: choice.to_string(),
            responded_at: OffsetDateTime::now_utc(),
        };
        tx.execute(
            "INSERT INTO tool_responses (
                id, work_kind, work_id, request_id, choice, responded_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                receipt.id.as_str(),
                work.kind(),
                work.id(),
                receipt.request_id,
                receipt.choice,
                receipt.responded_at.unix_timestamp(),
            ],
        )?;
        tx.commit()?;
        Ok((receipt, true))
    }

    pub fn tool_response(
        &self,
        work: &WorkRef,
        request_id: &str,
    ) -> StoreResult<Option<ToolResponseReceipt>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        tool_response_in(&conn, work, request_id)
    }
}

pub(super) fn map_local_machine(conn: &Connection) -> StoreResult<Machine> {
    conn.query_row(
        "SELECT id, route, created_at, observed_at, label, repo FROM machines WHERE route='local'",
        [],
        |row| Ok(map_machine_row(row)),
    )?
}

fn map_machine_by_id(conn: &Connection, machine_id: &MachineId) -> StoreResult<Option<Machine>> {
    conn.query_row(
        "SELECT id, route, created_at, observed_at, label, repo FROM machines WHERE id=?1",
        [machine_id.as_str()],
        |row| Ok(map_machine_row(row)),
    )
    .optional()?
    .transpose()
}

fn map_machine_row(row: &rusqlite::Row<'_>) -> StoreResult<Machine> {
    Ok(Machine {
        id: MachineId::parse(&row.get::<_, String>(0)?).map_err(invalid_durable)?,
        route: row.get(1)?,
        created_at: OffsetDateTime::from_unix_timestamp(row.get(2)?).map_err(invalid_durable)?,
        observed_at: OffsetDateTime::from_unix_timestamp(row.get(3)?).map_err(invalid_durable)?,
        label: row.get(4)?,
        repo: row.get(5)?,
    })
}

pub(super) fn placement_in(conn: &Connection, work: &WorkRef) -> StoreResult<Placement> {
    let mut source = work.clone();
    let mut visited = std::collections::HashSet::new();
    loop {
        if !visited.insert(source.clone()) {
            return Err(StoreError::InvalidData(
                "cycle in Work delegation ancestry".into(),
            ));
        }
        let (parent, created_at) = match &source {
            WorkRef::Task(id) => conn.query_row(
                "SELECT project_id,created_at FROM tasks WHERE id=?1",
                [id.as_str()],
                |row| {
                    Ok((
                        Some(WorkRef::Project(ProjectId::from_raw(
                            row.get::<_, String>(0)?,
                        ))),
                        row.get::<_, i64>(1)?,
                    ))
                },
            )?,
            WorkRef::Project(id) => conn.query_row(
                "SELECT wave_id,created_at FROM projects WHERE id=?1",
                [id.as_str()],
                |row| Ok((Some(WorkRef::Wave(row.get(0)?)), row.get(1)?)),
            )?,
            WorkRef::Wave(id) => conn.query_row(
                "SELECT parent_wave_id,created_at FROM waves WHERE id=?1",
                [id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, Option<WaveId>>(0)?.map(WorkRef::Wave),
                        row.get(1)?,
                    ))
                },
            )?,
        };
        if let Some(mut placement) = find_placement_in(conn, &source)? {
            placement.work = work.clone();
            return Ok(placement);
        }
        match parent {
            Some(parent) => source = parent,
            None => {
                return Ok(Placement {
                    work: work.clone(),
                    source,
                    provenance: PlacementProvenance::LocalDefault,
                    machine_id: map_local_machine(conn)?.id,
                    placed_at: OffsetDateTime::from_unix_timestamp(created_at)
                        .map_err(invalid_durable)?,
                })
            }
        }
    }
}

fn find_placement_in(conn: &Connection, work: &WorkRef) -> StoreResult<Option<Placement>> {
    let row = conn
        .query_row(
            &format!(
                "SELECT machine_id, placed_at, provenance FROM work_placements WHERE {}_id=?1",
                work.kind()
            ),
            [work.id()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    row.map(|(machine_id, placed_at, provenance)| {
        Ok(Placement {
            work: work.clone(),
            source: work.clone(),
            provenance: match provenance.as_str() {
                "explicit" => PlacementProvenance::Explicit,
                "legacy" => PlacementProvenance::Legacy,
                other => {
                    return Err(invalid_durable(format!(
                        "unknown placement provenance {other}"
                    )))
                }
            },
            machine_id: MachineId::parse(&machine_id).map_err(invalid_durable)?,
            placed_at: OffsetDateTime::from_unix_timestamp(placed_at).map_err(invalid_durable)?,
        })
    })
    .transpose()
}

/// SQL for the state of Task row `t`, read from its Workflow position.
pub(crate) fn task_state_sql(t: &str) -> String {
    format!(
        "CASE WHEN {t}.abandoned_at IS NOT NULL THEN 'abandoned' ELSE COALESCE((
            SELECT CASE WHEN wf.edge IS NOT NULL THEN 'active' WHEN wf.node='start' THEN 'ready'
                WHEN wf.node='end' THEN 'done' ELSE 'active' END
            FROM task_workflows wf WHERE wf.task_id={t}.id),'not_ready') END"
    )
}

/// SQL: Task row `t` is neither done nor abandoned.
pub(crate) fn task_open_sql(t: &str) -> String {
    format!("({}) NOT IN ('done','abandoned')", task_state_sql(t))
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

pub(super) fn require_current_task_project(conn: &Connection, work: &WorkRef) -> StoreResult<()> {
    let WorkRef::Task(task) = work else {
        return Ok(());
    };
    let (project, started): (String, bool) = conn.query_row(
        "SELECT project_id, started_at IS NOT NULL FROM tasks WHERE id=?1",
        [task.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if started {
        return Ok(());
    }
    require_selected_project(conn, &ProjectId::from_raw(project))
}

pub(super) fn require_selected_project(conn: &Connection, project: &ProjectId) -> StoreResult<()> {
    let (selected, status): (Option<String>, String) = conn.query_row(
        "SELECT w.current_project_id,p.status FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
        [project.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if selected.as_deref() != Some(project.as_str()) {
        return Err(StoreError::InvalidAuthority(
            "Task Project is not the Wave's configured Project; ensure the Wave before starting new work".into(),
        ));
    }
    if status != "started" {
        return Err(StoreError::InvalidAuthority(
            "configured Project is not In Progress; ensure the Wave before starting new work"
                .into(),
        ));
    }
    Ok(())
}

fn work_table(work: &WorkRef) -> (&'static str, &str) {
    match work {
        WorkRef::Wave(id) => ("waves", id.as_str()),
        WorkRef::Project(id) => ("projects", id.as_str()),
        WorkRef::Task(id) => ("tasks", id.as_str()),
    }
}

pub(super) fn task_wave_in(conn: &Connection, task: &TaskId) -> StoreResult<WaveId> {
    let wave: String = conn
        .query_row(
            "SELECT p.wave_id FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
            [task.as_str()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| invalid_durable(format!("Task {task} does not exist")))?;
    WaveId::parse(&wave).map_err(invalid_durable)
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

fn tool_response_in(
    conn: &Connection,
    work: &WorkRef,
    request_id: &str,
) -> StoreResult<Option<ToolResponseReceipt>> {
    let row = conn
        .query_row(
            "SELECT id, choice, responded_at FROM tool_responses
             WHERE work_kind=?1 AND work_id=?2 AND request_id=?3",
            params![work.kind(), work.id(), request_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((id, choice, responded_at)) = row else {
        return Ok(None);
    };
    Ok(Some(ToolResponseReceipt {
        id: ToolResponseId::parse(&id).map_err(|error| {
            StoreError::InvalidData(format!("invalid stored ToolResponse id: {error}"))
        })?,
        work: work.clone(),
        request_id: request_id.to_string(),
        choice,
        responded_at: OffsetDateTime::from_unix_timestamp(responded_at).map_err(|error| {
            StoreError::InvalidData(format!("invalid Decision timestamp: {error}"))
        })?,
    }))
}

#[cfg(test)]
mod durable_store_tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Barrier};
    use std::thread;

    use super::super::sessions::reserve_session_in;
    use crate::durable::{ProjectId, TaskId, WorkRef};

    use crate::id::WaveId;
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::session::{AgentSession, TitleSource, WorkSource};
    use crate::store::sqlite::SqliteStore;

    use crate::work::project::Project;
    use crate::work::task::{PmWritebackState, Task, TaskEventKind, TaskPr, TaskPrId};

    fn unpublished_conversation(
        task_id: Option<TaskId>,
        wave_id: Option<WaveId>,
        created_at: i64,
    ) -> AgentSession {
        AgentSession {
            captured: None,
            id: uuid::Uuid::new_v4().to_string(),
            artifact_key: crate::session_record::new_artifact_key(),
            caller_artifact_key: None,
            input_published: false,
            cwd: "/repo".into(),
            skill: None,
            provider: None,
            model: None,
            node: None,
            iterations: None,
            task_id,
            wave_id,
            flow_process_lfid: None,
            work_source: Some(WorkSource::Declared),
            bound_at: None,
            interactive: false,
            repo: None,
            title: "Investigation".into(),
            title_source: TitleSource::Generated,
            request: None,
            ready_summary: None,
            completed_at: None,
            created_at,
        }
    }

    fn store_with_task() -> (tempfile::TempDir, SqliteStore, TaskId) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loopflow.db");
        let store = SqliteStore::open_ephemeral(&path).expect("open a fresh store");
        let wave_id = WaveId::new();
        let project_id = ProjectId::new();
        let task_id = TaskId::new();
        let now = time::OffsetDateTime::now_utc();
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute(
            "INSERT INTO waves (id, name, repo, created_at, parent_wave_id)
             VALUES (?1, 'probe', '/repo', 1700000000, NULL)",
            [wave_id.as_str()],
        )
        .unwrap();
        drop(conn);
        let project = Project {
            id: project_id.clone(),
            plan: ProjectPlan {
                summary: String::new(),
                workflow: "feature".into(),
                status: crate::pm::ProjectStatus::Started,
                linear_id: Some(
                    LinearProjectId::new("999bdbdd-c045-41a6-8ffc-a97c4a40b0b3").unwrap(),
                ),
                slug: "probe".to_string(),
                name: "Probe".to_string(),
                prompt_context: "Probe Task execution".to_string(),
                pm_snapshot_synced_at: Some(now.unix_timestamp()),
            },
            wave_id: wave_id.clone(),
            iteration: 0,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store.insert_project(&project).unwrap();
        crate::store::sqlite::project_selection::write_project_binding(
            &store,
            &wave_id,
            None,
            project.plan.linear_id.as_ref().unwrap().as_str(),
            &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
        )
        .unwrap();
        let task = Task {
            id: task_id.clone(),
            plan: TaskPlan {
                revision: 0,
                linear_id: Some(LinearIssueId::new("task-uuid").unwrap()),
                identifier: "PROBE-1".to_string(),
                title: "Probe Task execution".to_string(),
                description: "Exercise the Task Flow store".to_string(),
                pm_snapshot_synced_at: Some(now.unix_timestamp()),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id,
            project_id,
            worktree: Some(PathBuf::from("/repo.probe")),
            workspace_slug: "probe".to_string(),
            agent: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::work::task::Observation::NotRequired,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_id: task_id.clone(),
            sequence: 1,
            slug: "probe".to_string(),
            branch: "probe".to_string(),
            base_commit: "deadbeef".to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        };
        store.seed_task(&task, &pr).unwrap();
        (dir, store, task_id)
    }

    #[test]
    fn task_routing_pins_checkouts_while_delegating_unstarted_work() {
        use crate::durable::TaskExecutionSource::{EffectiveDelegation, RecordedCheckout};
        let (dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let wave = store.get_wave(&task.wave_id).unwrap().unwrap();
        let repository = store.ensure_repository(wave.repo()).unwrap();
        let local = store.local_machine().unwrap().id;
        let remote = crate::durable::MachineId::new();
        let saved_machine = store
            .add_machine(&remote, "worker", "worker", "/saved/default")
            .unwrap();
        let work = WorkRef::Wave(task.wave_id.clone());
        store.place_work(&work, &remote).unwrap();
        let route = store.task_execution_route(&task_id).unwrap();
        assert_eq!(route.machine_id, local);
        assert_eq!(route.repository_id, repository);
        assert_eq!(route.source, RecordedCheckout);

        let (future, _) = unregistered_task(&store, &task_id, dir.path().join("future"));
        store.seed_unplaced_task(&future);
        assert_eq!(
            store.task_execution_route(&future.id).unwrap().machine_id,
            remote
        );
        assert_eq!(
            store.task_execution_route(&future.id).unwrap().source,
            EffectiveDelegation
        );
        // A retained checkout on a different Machine is execution evidence even
        // when there is no running provider. Subsequent delegation cannot move it.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET checkout_machine_id=?2 WHERE id=?1",
                rusqlite::params![task_id.as_str(), remote.as_str()],
            )
            .unwrap();
        store.place_work(&work, &local).unwrap();
        assert_eq!(
            store.task_execution_route(&task_id).unwrap().machine_id,
            remote
        );
        assert_eq!(
            store.task_execution_route(&future.id).unwrap().machine_id,
            local
        );
        assert_eq!(store.machine_by_id(&remote).unwrap(), Some(saved_machine));
        assert!(!store.task_started(&future.id).unwrap());
        assert!(store.task(&future.id).unwrap().unwrap().worktree.is_none());
        // Started evidence still pins a retired checkout, including provenance.
        store
            .create_session(
                unpublished_conversation(Some(task_id.clone()), None, 1),
                None,
            )
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET worktree=NULL WHERE id=?1",
                [task_id.as_str()],
            )
            .unwrap();
        let route = store.task_execution_route(&task_id).unwrap();
        assert_eq!(route.machine_id, remote);
        assert_eq!(route.source, RecordedCheckout);

        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET checkout_machine_id=NULL WHERE id=?1",
                [task_id.as_str()],
            )
            .unwrap();
        assert!(store
            .task_execution_route(&task_id)
            .unwrap_err()
            .to_string()
            .contains("unknown execution Machine"));
    }

    #[test]
    fn delegation_updates_existing_and_future_children_without_copying() {
        use crate::durable::{MachineId, PlacementProvenance, WorkRef};
        let (dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let work = WorkRef::Task(task_id.clone());
        let wave = WorkRef::Wave(task.wave_id.clone());
        let project = WorkRef::Project(task.project_id.clone());
        let local = store.local_machine().unwrap().id;
        let remote = MachineId::new();
        store
            .add_machine(&remote, "worker", "worker", "/other/repo")
            .unwrap();
        let initial = store.placement(&work).unwrap();
        assert_eq!(initial.source, wave);
        assert_eq!(initial.provenance, PlacementProvenance::LocalDefault);
        store.place_work(&wave, &remote).unwrap();
        let inherited = store.placement(&work).unwrap();
        assert_eq!(inherited.work, work);
        assert_eq!(inherited.source, wave);
        assert_eq!(inherited.provenance, PlacementProvenance::Explicit);
        assert_eq!(inherited.machine_id, remote);

        // Even selecting the same Machine establishes a narrower override.
        store.place_work(&project, &remote).unwrap();
        store.place_work(&wave, &local).unwrap();
        assert_eq!(store.placement(&work).unwrap().source, project);
        assert_eq!(store.placement(&work).unwrap().machine_id, remote);
        let (future, _) = unregistered_task(&store, &task_id, dir.path().join("future"));
        store.seed_unplaced_task(&future);
        let future_work = WorkRef::Task(future.id.clone());
        assert_eq!(store.placement(&future_work).unwrap().machine_id, remote);
        store.place_work(&work, &remote).unwrap();
        store.place_work(&project, &local).unwrap();
        assert_eq!(store.placement(&work).unwrap().source, work);
        assert_eq!(store.placement(&work).unwrap().machine_id, remote);
        assert_eq!(store.placement(&future_work).unwrap().machine_id, local);
        assert!(!store.task_started(&future.id).unwrap());
        assert!(store.task(&future.id).unwrap().unwrap().worktree.is_none());
    }

    #[test]
    fn delegation_preserves_legacy_provenance_until_explicitly_selected() {
        use crate::durable::{MachineId, PlacementProvenance, WorkRef};
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let work = WorkRef::Task(task_id.clone());
        let local = store.local_machine().unwrap().id;
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO work_placements(task_id,machine_id,placed_at) VALUES(?1,?2,1)",
                rusqlite::params![task_id.as_str(), local.as_str()],
            )
            .unwrap();
        let remote = MachineId::new();
        store
            .add_machine(&remote, "worker", "worker", "/repo")
            .unwrap();
        store
            .place_work(&WorkRef::Wave(task.wave_id), &remote)
            .unwrap();
        let legacy = store.placement(&work).unwrap();
        assert_eq!(legacy.machine_id, local);
        assert_eq!(legacy.source, work);
        assert_eq!(legacy.provenance, PlacementProvenance::Legacy);
        let selected = store.place_work(&work, &local).unwrap();
        assert_eq!(selected.provenance, PlacementProvenance::Explicit);
        assert_eq!(store.placement(&work).unwrap(), selected);
    }

    #[test]
    fn delegation_follows_wave_ancestry_and_rejects_cycles() {
        use crate::durable::{MachineId, WorkRef};
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let ancestor = WaveId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'parent','/repo',1)",
                [ancestor.as_str()],
            )
            .unwrap();
            conn.execute(
                "UPDATE waves SET parent_wave_id=?2 WHERE id=?1",
                rusqlite::params![task.wave_id.as_str(), ancestor.as_str()],
            )
            .unwrap();
        }
        let work = WorkRef::Task(task_id);
        let remote = MachineId::new();
        store
            .add_machine(&remote, "worker", "worker", "/repo")
            .unwrap();
        store
            .place_work(&WorkRef::Wave(ancestor.clone()), &remote)
            .unwrap();
        assert_eq!(
            store.placement(&work).unwrap().source,
            WorkRef::Wave(ancestor.clone())
        );
        assert_eq!(store.placement(&work).unwrap().machine_id, remote);
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "DELETE FROM work_placements WHERE wave_id=?1",
                [ancestor.as_str()],
            )
            .unwrap();
            conn.execute(
                "UPDATE waves SET parent_wave_id=?2 WHERE id=?1",
                rusqlite::params![ancestor.as_str(), task.wave_id.as_str()],
            )
            .unwrap();
        }
        assert!(store
            .placement(&work)
            .unwrap_err()
            .to_string()
            .contains("cycle"));
        assert!(store.placement(&WorkRef::Task(TaskId::new())).is_err());
    }

    #[test]
    fn configured_project_admission_preserves_started_continuation() {
        let (dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let mut other = store.project(&task.project_id).unwrap().unwrap();
        other.id = ProjectId::new();
        other.plan.linear_id =
            Some(LinearProjectId::new("218967b6-a760-4b7c-9a46-11d9d61a42c2").unwrap());
        other.plan.name = "An unrelated ordinary plan".into();
        store.insert_project(&other).unwrap();
        // Another Started Project does not compete with the configured identity.
        let (new_task, pr) = unregistered_task(&store, &task_id, dir.path().join("new"));
        store.seed_task(&new_task, &pr).unwrap();
        store.require_task_launch(&task_id).unwrap();
        assert!(!store.task_started(&task_id).unwrap());
        let mut input = unpublished_conversation(Some(new_task.id.clone()), None, 1);
        input.cwd = new_task.worktree.as_ref().unwrap().clone();
        store.create_session(input, None).unwrap();
        assert!(store.task_started(&new_task.id).unwrap());
        let guard = crate::store::PlanningLocks::new(tempfile::tempfile().unwrap());
        crate::store::sqlite::project_selection::write_project_binding(
            &store,
            &task.wave_id,
            Some("999bdbdd-c045-41a6-8ffc-a97c4a40b0b3"),
            other.plan.linear_id.as_ref().unwrap().as_str(),
            &guard,
        )
        .unwrap();
        let (mut rejected, mut rejected_pr) =
            unregistered_task(&store, &task_id, dir.path().join("rejected"));
        rejected.plan.linear_id = Some(LinearIssueId::new("third-issue").unwrap());
        rejected.plan.identifier = "PROBE-3".into();
        rejected.workspace_slug = "rejected".into();
        rejected_pr.slug = "rejected".into();
        rejected_pr.branch = "rejected".into();
        store.seed_unplaced_task(&rejected);
        assert!(store
            .place_task(
                &rejected.id,
                rejected.worktree.as_ref().unwrap(),
                &rejected.workspace_slug,
                &rejected_pr
            )
            .is_err());
        assert!(store
            .task(&rejected.id)
            .unwrap()
            .unwrap()
            .worktree
            .is_none());
        assert!(store.task_prs(&rejected.id).unwrap().is_empty());
        assert!(store.require_task_launch(&task_id).is_err());
        // An established start remains eligible after its Project becomes history.
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE projects SET status='completed' WHERE id=?1",
                [task.project_id.as_str()],
            )
            .unwrap();
        }
        store.require_task_launch(&new_task.id).unwrap();
        assert!(store.task_started(&new_task.id).unwrap());
        assert_eq!(
            store.task(&task_id).unwrap().unwrap().worktree,
            task.worktree
        );
    }

    #[test]
    fn configured_project_admission_requires_a_selected_active_project() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE waves SET current_project_id=NULL WHERE id=?1",
                [&task.wave_id],
            )
            .unwrap();
        assert!(store.require_task_launch(&task_id).is_err());
        assert!(!store.task_started(&task_id).unwrap());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE waves SET current_project_id=?2 WHERE id=?1",
                rusqlite::params![task.wave_id, task.project_id.as_str()],
            )
            .unwrap();
        for status in ["backlog", "planned", "paused", "completed", "canceled"] {
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE projects SET status=?2 WHERE id=?1",
                    rusqlite::params![task.project_id.as_str(), status],
                )
                .unwrap();
            assert!(store.require_task_launch(&task_id).is_err());
        }
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE projects SET status='started', workflow='' WHERE id=?1",
                [task.project_id.as_str()],
            )
            .unwrap();
        store.require_task_launch(&task_id).unwrap();
    }

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
            Err(crate::store::StoreError::NotFound)
        ));
        assert_eq!(store.task_events_after(&task, 0).unwrap(), vec![event]);
    }

    fn conversation(
        flow_process_lfid: Option<String>,
        task_id: Option<TaskId>,
        wave_id: Option<WaveId>,
    ) -> crate::session::AgentSession {
        crate::session::AgentSession {
            captured: None,
            id: uuid::Uuid::new_v4().to_string(),
            artifact_key: crate::session_record::new_artifact_key(),
            caller_artifact_key: None,
            input_published: false,
            cwd: "/repo".into(),
            skill: Some("implement".into()),
            provider: None,
            model: None,
            node: None,
            iterations: None,
            task_id,
            wave_id,
            flow_process_lfid,
            work_source: Some(WorkSource::Declared),
            bound_at: None,
            interactive: false,
            repo: None,
            title: "Implementation".into(),
            title_source: crate::session::TitleSource::Generated,
            request: None,
            ready_summary: None,
            completed_at: None,
            created_at: 100,
        }
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
        conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at)
            SELECT ?1,project_id,?1,?1,'/repo.other',1 FROM tasks WHERE id=?2",
            rusqlite::params![other_task.as_str(), task_id.as_str()]).unwrap();
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
                Some(task.wave_id.clone()),
            ),
            (
                None,
                Some(task_id.clone()),
                Some(task.wave_id.clone()),
                Some(task_id.clone()),
                Some(task.wave_id.clone()),
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
        // Changing a parent cannot invalidate an already reserved child's ancestry.
        assert!(conn
            .execute(
                "UPDATE projects SET wave_id=?1 WHERE id=?2",
                rusqlite::params![other_wave.as_str(), task.project_id.as_str()]
            )
            .is_err());
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
        let project_id = task.project_id.clone();
        let db = dir.path().join("loopflow.db");
        let writer = thread::spawn(move || {
            let conn = rusqlite::Connection::open(db).unwrap();
            conn.busy_timeout(super::super::SQLITE_WRITE_BUSY_TIMEOUT)
                .unwrap();
            conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
            writer_barrier.wait();
            conn.execute(
                "UPDATE projects SET wave_id=?1 WHERE id=?2",
                rusqlite::params![other_wave.as_str(), project_id.as_str()],
            )
        });
        barrier.wait();
        let mut input = unpublished_conversation(Some(task_id), None, 100);
        input.work_source = Some(WorkSource::Checkout);
        let saved = reserve_session_in(&tx, input, None).unwrap();
        tx.commit().unwrap();
        let error = writer.join().unwrap().unwrap_err();
        assert!(error
            .to_string()
            .contains("selected Project cannot change Wave ownership"));
        let (run_wave, project_wave): (String, String) = conn
            .query_row(
                "SELECT r.wave_id,p.wave_id FROM agent_sessions r JOIN tasks t ON t.id=r.task_id
             JOIN projects p ON p.id=t.project_id WHERE r.id=?1",
                [saved.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(run_wave, task.wave_id.as_str());
        assert_eq!(run_wave, project_wave);
    }

    #[test]
    fn chapter_evidence_marks_taskless_flows_started_in_their_checkout() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        assert!(!store.chapter_task_evidence(&task_id).unwrap().begun);
        // A Flow run from the checkout starts its Task without an explicit bind.
        store.test_flow(
            "sync",
            &task.worktree.as_ref().unwrap().to_string_lossy(),
            &[("sync --plan", None)],
            None,
        );
        assert!(store.chapter_task_evidence(&task_id).unwrap().begun);
        assert!(store.task_started(&task_id).unwrap());
    }

    fn unregistered_task(
        store: &SqliteStore,
        existing: &TaskId,
        checkout: PathBuf,
    ) -> (Task, TaskPr) {
        let mut task = store.task(existing).unwrap().unwrap();
        let mut pr = store.task_prs(existing).unwrap().remove(0);
        task.id = TaskId::new();
        task.plan.linear_id = Some(LinearIssueId::new("later-issue").unwrap());
        task.plan.identifier = "PROBE-2".into();
        task.worktree = Some(checkout);
        task.workspace_slug = "later-checkout".into();
        pr.id = TaskPrId::new();
        pr.task_id = task.id.clone();
        pr.slug = task.workspace_slug.clone();
        pr.branch = task.workspace_slug.clone();
        (task, pr)
    }

    #[test]
    fn descendant_admission_excludes_registration_of_a_missing_root() {
        let (dir, store, existing) = store_with_task();
        let (task, pr) = unregistered_task(&store, &existing, dir.path().join("missing-root"));
        let cwd = task.worktree.as_ref().unwrap().join("src/nested");
        let admission = store.lock_checkout(&cwd).unwrap();
        let mut earlier = unpublished_conversation(None, None, 1);
        earlier.cwd = cwd;
        earlier.work_source = None;
        store.seed_unplaced_task(&task);
        let register = || {
            store.place_task(
                &task.id,
                task.worktree.as_ref().unwrap(),
                &task.workspace_slug,
                &pr,
            )
        };
        assert!(register().is_err());
        assert!(store.task(&task.id).unwrap().unwrap().worktree.is_none());
        assert!(store.task_prs(&task.id).unwrap().is_empty());
        drop(admission);
        let earlier = store.create_session(earlier, None).unwrap();
        register().unwrap();
        assert_eq!(
            store.session_task_ids(&earlier.id).unwrap(),
            vec![task.id.clone()]
        );
        assert_eq!(store.session(&earlier.id).unwrap().unwrap(), earlier);
        assert!(!store.task_started(&task.id).unwrap());
    }

    #[test]
    fn missing_root_exclusion_preserves_taskless_session_admission() {
        let (dir, store, existing) = store_with_task();
        let (task, pr) = unregistered_task(&store, &existing, dir.path().join("missing-root"));
        let mut conversation = unpublished_conversation(None, None, 1);
        conversation.cwd = task.worktree.as_ref().unwrap().join("src/nested");
        conversation.work_source = None;
        let exclusion = store
            .lock_checkout(task.worktree.as_ref().unwrap())
            .unwrap();
        assert!(store.create_session(conversation.clone(), None).is_err());
        assert!(store.session(&conversation.id).unwrap().is_none());
        drop(exclusion);
        let session = store.create_session(conversation, None).unwrap();
        store.seed_unplaced_task(&task);
        store
            .place_task(
                &task.id,
                task.worktree.as_ref().unwrap(),
                &task.workspace_slug,
                &pr,
            )
            .unwrap();
        assert_eq!(
            store.session_task_ids(&session.id).unwrap(),
            vec![task.id.clone()]
        );
        assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
        assert!(!store.task_started(&task.id).unwrap());
    }

    #[test]
    fn unrelated_checkout_exclusion_does_not_block_task_registration() {
        let (dir, store, existing) = store_with_task();
        let (task, pr) =
            unregistered_task(&store, &existing, dir.path().join("independent-checkout"));

        let _unrelated = store
            .lock_checkout(&dir.path().join("unrelated-checkout/missing/src"))
            .unwrap();
        store.seed_unplaced_task(&task);
        store
            .place_task(
                &task.id,
                task.worktree.as_ref().unwrap(),
                &task.workspace_slug,
                &pr,
            )
            .unwrap();
        assert_eq!(
            store.task(&task.id).unwrap().unwrap().worktree,
            task.worktree
        );
        assert_eq!(store.task_prs(&task.id).unwrap(), vec![pr]);
    }

    #[test]
    fn checkout_exclusion_preserves_registration_and_retry() {
        let (dir, store, existing) = store_with_task();
        let (task, pr) = unregistered_task(&store, &existing, dir.path().join("missing-checkout"));

        let mut conversation = unpublished_conversation(None, None, 1);
        conversation.cwd = task.worktree.as_ref().unwrap().join("src");
        conversation.work_source = None;
        let session = store.create_session(conversation, None).unwrap();
        let exclusion = store
            .lock_checkout(task.worktree.as_ref().unwrap())
            .unwrap();
        store.seed_unplaced_task(&task);
        let register = || {
            store.place_task(
                &task.id,
                task.worktree.as_ref().unwrap(),
                &task.workspace_slug,
                &pr,
            )
        };
        assert!(register().is_err());
        assert!(store.task(&task.id).unwrap().unwrap().worktree.is_none());
        assert!(store.task_prs(&task.id).unwrap().is_empty());
        assert!(store.session_task_ids(&session.id).unwrap().is_empty());
        assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
        drop(exclusion);
        register().unwrap();
        assert_eq!(
            store.session_task_ids(&session.id).unwrap(),
            vec![task.id.clone()]
        );
        assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
        let work = store.task_work(&task.id).unwrap();
        assert_eq!(work.sessions.len(), 1);
        assert_eq!(work.sessions[0].id, session.id);
        assert!(!store.task_started(&task.id).unwrap());
        assert!(store.chapter_task_evidence(&task.id).unwrap().begun);
    }

    #[test]
    fn checkout_session_membership_survives_project_transfer_without_binding() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let mut conversation = unpublished_conversation(None, None, 1);
        conversation.cwd = task.worktree.as_ref().unwrap().join("src");
        conversation.work_source = None;
        let session = store.create_session(conversation, None).unwrap();
        let before = store.task_work(&task_id).unwrap();
        assert_eq!(before.sessions.len(), 1);
        assert_eq!(before.sessions[0].id, session.id);
        assert_eq!(session.task_id, None);
        assert!(!store.task_started(&task_id).unwrap());
        assert!(store.chapter_task_evidence(&task_id).unwrap().begun);

        let successor = ProjectId::new();
        store.conn.lock().unwrap().execute(
            "INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'successor',2)",
            rusqlite::params![successor.as_str(), task.wave_id.as_str()],
        ).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET project_id=?2 WHERE id=?1",
                rusqlite::params![task_id.as_str(), successor.as_str()],
            )
            .unwrap();

        let retained = store.task(&task_id).unwrap().unwrap();
        assert_eq!(retained.project_id, successor);
        assert_eq!(retained.worktree, task.worktree);
        let after = store.task_work(&task_id).unwrap();
        assert_eq!(after.sessions, before.sessions);
        assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
    }

    #[test]
    fn checkout_exclusion_preserves_unbound_history_until_binding_retries() {
        let (dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let mut conversation = unpublished_conversation(None, None, 1);
        conversation.cwd = dir.path().join("elsewhere");
        let session = store.create_session(conversation, None).unwrap();
        let exclusion = store
            .lock_checkout(task.worktree.as_ref().unwrap())
            .unwrap();
        assert!(store
            .bind_session(&session.id, session.captured, &task_id)
            .is_err());
        assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
        assert!(!store.task_started(&task_id).unwrap());
        drop(exclusion);
        let bound = store
            .bind_session(&session.id, session.captured, &task_id)
            .unwrap();
        assert_eq!(bound.task_id.as_ref(), Some(&task_id));
        assert!(store.task_started(&task_id).unwrap());
    }

    #[test]
    fn checkout_exclusion_preserves_bound_session_input_until_retry() {
        let (dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let mut input = unpublished_conversation(Some(task_id.clone()), None, 1);
        input.cwd = dir.path().join("elsewhere");
        let session = store.create_session(input, None).unwrap();
        let mut next = session.clone();
        next.artifact_key = crate::session_record::new_artifact_key();
        let exclusion = store
            .lock_checkout(task.worktree.as_ref().unwrap())
            .unwrap();
        let result = store.replace_session_input(session.captured, next.clone());
        assert!(
            result.is_err(),
            "input changed while its Task checkout was excluded"
        );
        assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
        drop(exclusion);
        let replaced = store.replace_session_input(session.captured, next).unwrap();
        assert_eq!(replaced.task_id.as_ref(), Some(&task_id));
        assert_eq!(replaced.cwd, session.cwd);
        assert_ne!(replaced.captured, session.captured);
    }

    #[test]
    fn checkout_exclusion_covers_missing_subdirectories_and_explicit_tasks() {
        let (dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let exclusion = store
            .lock_checkout(task.worktree.as_ref().unwrap())
            .unwrap();
        let mut conversation = unpublished_conversation(None, None, 1);
        conversation.cwd = task.worktree.as_ref().unwrap().join("missing/subdirectory");
        assert!(store.create_session(conversation.clone(), None).is_err());
        assert!(store.session(&conversation.id).unwrap().is_none());
        conversation.cwd = dir.path().join("elsewhere");
        conversation.task_id = Some(task_id.clone());
        assert!(store.create_session(conversation.clone(), None).is_err());
        assert!(!store.task_started(&task_id).unwrap());
        drop(exclusion);
        let created = store.create_session(conversation, None).unwrap();
        assert_eq!(created.task_id.as_ref(), Some(&task_id));
        assert!(store.task_started(&task_id).unwrap());
    }

    #[test]
    fn session_binding_retains_a_task_in_completed_project_history() {
        let (dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let mut conversation = unpublished_conversation(None, None, 1);
        conversation.cwd = dir.path().to_path_buf();
        let session = store.create_session(conversation, None).unwrap();
        assert!(!store.task_started(&task_id).unwrap());

        // Rotation can complete a predecessor after observing untouched backlog.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE projects SET status='completed' WHERE id=?1",
                [task.project_id.as_str()],
            )
            .unwrap();
        let bound = store
            .bind_session(&session.id, session.captured, &task_id)
            .unwrap();
        assert_eq!(bound.task_id.as_ref(), Some(&task_id));
        assert!(store.task_started(&task_id).unwrap());
        let retained = store.task(&task_id).unwrap().unwrap();
        assert_eq!(retained.project_id, task.project_id);
        assert_eq!(retained.worktree, task.worktree);
        assert_eq!(
            store
                .bind_session(&session.id, session.captured, &task_id)
                .unwrap(),
            bound
        );
    }

    #[test]
    fn session_assignment_starts_tasks_once_and_rejects_reassignment_atomically() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let mut conn = store.conn.lock().unwrap();
        let bound_task = TaskId::new();
        let losing_task = TaskId::new();
        for id in [&bound_task, &losing_task] {
            conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at)
                VALUES(?1,?2,?1,?1,?1,1)",
                rusqlite::params![id.as_str(), task.project_id.as_str()]).unwrap();
        }
        // A done Task is still a valid assignment target.
        conn.execute(
            "INSERT INTO task_workflows(task_id,graph,node,updated_at) VALUES(?1,'{\"name\":\"unplanned\",\"nodes\":[],\"edges\":[{\"from\":\"start\",\"to\":\"end\",\"flow\":null}]}','end',1)",
            [bound_task.as_str()],
        )
        .unwrap();
        let other_wave = WaveId::new();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'other','/repo',1)",
            [other_wave.as_str()],
        )
        .unwrap();
        let before = crate::store::rows::now_unix();
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
            unpublished_conversation(None, Some(task.wave_id.clone()), 3),
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
        let started = |conn: &rusqlite::Connection, id: &TaskId| -> Option<i64> {
            conn.query_row(
                "SELECT started_at FROM tasks WHERE id=?1",
                [id.as_str()],
                |row| row.get(0),
            )
            .unwrap()
        };
        let assigned = started(&conn, &task_id).unwrap();
        assert!((before..=crate::store::rows::now_unix()).contains(&assigned));
        assert_ne!(assigned, reserved.created_at);
        assert_eq!(started(&conn, &bound_task), None);
        let before_bind = crate::store::rows::now_unix();
        conn.execute(
            "UPDATE agent_sessions SET task_id=?2,wave_id=?3,work_source='bound' WHERE id=?1",
            rusqlite::params![
                orphan.id.as_str(),
                bound_task.as_str(),
                task.wave_id.as_str()
            ],
        )
        .unwrap();
        let bound_at = started(&conn, &bound_task).unwrap();
        assert!((before_bind..=crate::store::rows::now_unix()).contains(&bound_at));
        assert_ne!(bound_at, orphan.created_at);
        conn.execute(
            "UPDATE agent_sessions SET task_id=?2,work_source='bound' WHERE id=?1",
            rusqlite::params![wave_only.id.as_str(), bound_task.as_str()],
        )
        .unwrap();
        for created_at in [0, bound_at + 1000] {
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .unwrap();
            reserve_session_in(
                &tx,
                unpublished_conversation(Some(bound_task.clone()), None, created_at),
                None,
            )
            .unwrap();
            let later_bind =
                reserve_session_in(&tx, unpublished_conversation(None, None, created_at), None)
                    .unwrap();
            tx.execute(
                "UPDATE agent_sessions SET task_id=?2,wave_id=?3 WHERE id=?1",
                rusqlite::params![
                    later_bind.id.as_str(),
                    bound_task.as_str(),
                    task.wave_id.as_str()
                ],
            )
            .unwrap();
            tx.commit().unwrap();
            assert_eq!(started(&conn, &bound_task), Some(bound_at));
        }
        for (id, new_task, new_wave) in [
            (
                &orphan.id,
                Some(losing_task.as_str()),
                Some(task.wave_id.as_str()),
            ),
            (&orphan.id, None, Some(task.wave_id.as_str())),
            (
                &conflicting.id,
                Some(losing_task.as_str()),
                Some(task.wave_id.as_str()),
            ),
            (&conflicting.id, None, None),
        ] {
            assert!(conn
                .execute(
                    "UPDATE agent_sessions SET task_id=?2,wave_id=?3 WHERE id=?1",
                    rusqlite::params![id.as_str(), new_task, new_wave]
                )
                .is_err());
            assert_eq!(started(&conn, &losing_task), None);
            assert_eq!(started(&conn, &bound_task), Some(bound_at));
        }
        for timestamp in [None, Some(bound_at - 1), Some(bound_at + 1)] {
            assert!(conn
                .execute(
                    "UPDATE tasks SET started_at=?2 WHERE id=?1",
                    rusqlite::params![bound_task.as_str(), timestamp]
                )
                .is_err());
        }
        assert!(conn
            .execute(
                "UPDATE tasks SET started_at=1 WHERE id=?1",
                [losing_task.as_str()]
            )
            .is_err());
        assert!(conn
            .execute(
                "DELETE FROM agent_sessions WHERE id=?1",
                [reserved.id.as_str()]
            )
            .is_err());
        // Failed enclosing writes roll back both the reservation and Started.
        {
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .unwrap();
            reserve_session_in(
                &tx,
                unpublished_conversation(Some(losing_task.clone()), None, 5),
                None,
            )
            .unwrap();
            assert!(started(&tx, &losing_task).is_some());
        }
        assert_eq!(started(&conn, &losing_task), None);
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM tasks t WHERE
            (started_at IS NOT NULL) != EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=t.id)",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn bind_preserves_prior_work_and_usage_while_assigning_future_work() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let other_wave = WaveId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'other','/repo',1)",
                [other_wave.as_str()],
            )
            .unwrap();
        let open = |id: &str, wave: Option<WaveId>| {
            let session = crate::session::AgentSession {
                captured: None,
                caller_artifact_key: None,
                task_id: None,
                wave_id: wave,
                flow_process_lfid: None,
                work_source: None,
                bound_at: None,
                id: id.to_string(),
                artifact_key: crate::session_record::new_artifact_key(),
                input_published: true,
                cwd: "/repo".into(),
                skill: None,
                provider: None,
                model: None,
                node: None,
                iterations: None,
                interactive: true,
                repo: None,
                title: id.to_string(),
                title_source: crate::session::TitleSource::Generated,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: 1,
            };
            store.create_session(session, None).unwrap()
        };
        let first = open("orphan", None);
        let replacement = store
            .replace_session_input(
                first.captured,
                crate::session::AgentSession {
                    caller_artifact_key: None,
                    artifact_key: crate::session_record::new_artifact_key(),
                    ..first.clone()
                },
            )
            .unwrap();
        assert!(
            store
                .replace_session_input(
                    replacement.captured,
                    crate::session::AgentSession {
                        cwd: "/changed".into(),
                        provider: Some("changed".into()),
                        model: Some("changed".into()),
                        ..first.clone()
                    }
                )
                .is_err(),
            "an earlier input cannot become a fresh replacement"
        );
        assert_eq!(store.session("orphan").unwrap(), Some(replacement.clone()));
        let elsewhere = open("elsewhere", Some(other_wave));
        let old_runs = store.session_inputs("orphan").unwrap();
        store
            .record_session_event(
                "orphan",
                "thread",
                "active",
                crate::session::SessionEventKind::Started,
                &serde_json::json!({}),
            )
            .unwrap();
        store
            .record_session_event(
                "orphan",
                "thread",
                "active",
                crate::session::SessionEventKind::Usage,
                &serde_json::json!({"input": 20}),
            )
            .unwrap();
        let earlier = store.session_history("orphan", 0, 0).unwrap();
        assert!(store
            .bind_session("orphan", first.captured, &task_id)
            .is_err());
        let bound = store
            .bind_session("orphan", replacement.captured, &task_id)
            .unwrap();
        assert_eq!(bound.artifact_key, replacement.artifact_key);
        assert_eq!(bound.task_id, Some(task_id.clone()));
        assert_eq!(bound.wave_id, Some(task.wave_id.clone()));
        assert!(bound.bound_at.is_some());
        let binds = store.bound_sessions().unwrap();
        assert_eq!(binds.len(), 1);
        assert_eq!(binds[0].session_id, "orphan");
        assert_eq!(Some(binds[0].at), bound.bound_at);
        assert_eq!(store.session_inputs("orphan").unwrap(), old_runs);
        assert_eq!(store.session_history("orphan", 0, 0).unwrap(), earlier);
        assert_eq!(
            store
                .bind_session("orphan", replacement.captured, &task_id)
                .unwrap(),
            bound
        );
        // A late cumulative observation belongs to the old turn, never the new bind.
        store
            .record_session_event(
                "orphan",
                "thread",
                "active",
                crate::session::SessionEventKind::Usage,
                &serde_json::json!({"input": 30}),
            )
            .unwrap();
        store
            .record_session_event(
                "orphan",
                "thread",
                "future",
                crate::session::SessionEventKind::Started,
                &serde_json::json!({}),
            )
            .unwrap();
        let events = store.session_history("orphan", 0, 0).unwrap();
        assert!(events
            .iter()
            .filter(|event| event.provider_turn.as_deref() == Some("active"))
            .all(|event| event.task_id.is_none()));
        assert_eq!(
            events.last().unwrap().task_id.as_deref(),
            Some(task_id.as_str())
        );
        let future = store
            .replace_session_input(
                replacement.captured,
                crate::session::AgentSession {
                    caller_artifact_key: None,
                    artifact_key: crate::session_record::new_artifact_key(),
                    ..bound.clone()
                },
            )
            .unwrap();
        assert_eq!(future.task_id, Some(task_id.clone()));
        assert_eq!(future.work_source, Some(WorkSource::Bound));
        assert!(store
            .session_inputs("orphan")
            .unwrap()
            .contains(&first.artifact_key));
        let history = store.session_history("orphan", 0, 0).unwrap();
        assert_eq!(&history[..events.len()], events);
        assert_eq!(history.len(), events.len() + 1);
        assert_eq!(
            history.last().unwrap().kind,
            crate::session::SessionEventKind::Captured
        );
        assert_eq!(
            history.last().unwrap().task_id.as_deref(),
            Some(task_id.as_str())
        );
        let refused = store
            .bind_session("elsewhere", elsewhere.captured, &task_id)
            .unwrap_err();
        assert!(refused.to_string().contains("another Wave"), "{refused}");
        assert_eq!(store.session("elsewhere").unwrap().unwrap().task_id, None);
        let listed = store
            .sessions(&crate::session::SessionFilter {
                task: Some(task_id.to_string()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].artifact_key, future.artifact_key);
    }

    #[test]
    fn competing_session_assignments_start_only_the_winning_task() {
        let (dir, store, task_id) = store_with_task();
        let wave = store.task(&task_id).unwrap().unwrap().wave_id;
        let other_task = TaskId::new();
        let session = store
            .create_session(unpublished_conversation(None, None, 1), None)
            .unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at)
                SELECT ?1,project_id,?1,?1,?1,1 FROM tasks WHERE id=?2",
                rusqlite::params![other_task.as_str(), task_id.as_str()]).unwrap();
        }
        let barrier = Arc::new(Barrier::new(2));
        let writers = [task_id, other_task].map(|target| {
            let path = dir.path().join("loopflow.db");
            let barrier = barrier.clone();
            let wave = wave.clone();
            let session = session.id.clone();
            thread::spawn(move || {
                let conn = rusqlite::Connection::open(path).unwrap();
                conn.busy_timeout(std::time::Duration::from_secs(5))
                    .unwrap();
                conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
                barrier.wait();
                let result = conn.execute(
                    "UPDATE agent_sessions SET task_id=?2,wave_id=?3 WHERE id=?1",
                    rusqlite::params![session.as_str(), target.as_str(), wave.as_str()],
                );
                (target, result)
            })
        });
        let results = writers.map(|writer| writer.join().unwrap());
        assert_eq!(
            results.iter().filter(|(_, result)| result.is_ok()).count(),
            1
        );
        let conn = store.conn.lock().unwrap();
        for (target, result) in results {
            let (has_session, started): (bool, bool) = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=?1),started_at IS NOT NULL
                 FROM tasks WHERE id=?1",
                    [target.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(has_session, result.is_ok());
            assert_eq!(started, result.is_ok());
            if let Err(error) = result {
                assert!(error.to_string().contains("binding is permanent"));
            }
        }
    }
}
