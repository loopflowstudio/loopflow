use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use time::OffsetDateTime;

use crate::child::ChildRef;
use crate::durable::{
    AbandonReceipt, Author, Home, HomeId, Placement, ProjectId, Steer, SteerComment, TaskId,
    ToolResponseId, ToolResponseReceipt, ToolResponseWrite, WorkRef, WorkStatus,
};
use crate::id::WaveId;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::work::project::{Project, ProjectEventKind};
use crate::work::task::{Task, TaskEventKind};

use super::SqliteStore;

impl SqliteStore {
    /// A Task may launch a Flow while it is ready, not being abandoned, and
    /// planned in its Wave's current chapter.
    pub(crate) fn require_task_launch(&self, task_id: &TaskId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
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
        require_current_task_chapter(&conn, &work)
    }

    pub fn begin_task_abandon(&self, task_id: &TaskId) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        match work_status_in(&tx, &WorkRef::Task(task_id.clone()))? {
            WorkStatus::Done => {
                return Err(StoreError::InvalidAuthority(
                    "completed Task cannot be abandoned".into(),
                ))
            }
            WorkStatus::Abandoned => return Ok(()),
            WorkStatus::Ready => {}
        }
        tx.execute(
            "UPDATE tasks SET abandon_requested_at=COALESCE(abandon_requested_at,?2),
             abandon_reason=COALESCE(abandon_reason,'explicit Task abandonment') WHERE id=?1",
            params![task_id.as_str(), now_unix()],
        )?;
        tx.commit()?;
        Ok(())
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

    pub fn home_by_id(&self, home_id: &HomeId) -> StoreResult<Option<Home>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        map_home_by_id(&conn, home_id)
    }

    pub fn local_home(&self) -> StoreResult<Home> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        map_local_home(&conn)
    }

    pub fn observe_home(&self, home_id: &HomeId, route: &str) -> StoreResult<Home> {
        let route = route.trim();
        if route.is_empty() {
            return Err(StoreError::InvalidData(
                "Home route cannot be empty".to_string(),
            ));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        if map_home_by_id(&conn, home_id)?
            .is_some_and(|home| home.route == "local" && route != "local")
        {
            return Err(StoreError::InvalidData(format!(
                "cannot replace local Home {home_id} with remote route {route:?}"
            )));
        }
        let existing_id = conn
            .query_row("SELECT id FROM homes WHERE route=?1", [route], |row| {
                row.get::<_, String>(0)
            })
            .optional()?;
        if existing_id
            .as_deref()
            .is_some_and(|id| id != home_id.as_str())
        {
            return Err(StoreError::InvalidData(format!(
                "Home route {route:?} is already observed for {}",
                existing_id.expect("checked as present")
            )));
        }
        let now = now_unix();
        conn.execute(
            "INSERT INTO homes (id, route, created_at, observed_at)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(id) DO UPDATE SET
                route=excluded.route, observed_at=excluded.observed_at",
            params![home_id.as_str(), route, now],
        )?;
        map_home_by_id(&conn, home_id)?.ok_or(StoreError::NotFound)
    }

    pub fn placement(&self, work: &WorkRef) -> StoreResult<Placement> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        placement_in(&conn, work)
    }

    pub(crate) fn place_work(&self, work: &WorkRef, home_id: &HomeId) -> StoreResult<Placement> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_ready_work(&tx, work)?;
        tx.query_row(
            "SELECT 1 FROM homes WHERE id=?1",
            [home_id.as_str()],
            |_| Ok(()),
        )?;
        if let Some(current) = find_placement_in(&tx, work)? {
            if current.home_id == *home_id {
                tx.commit()?;
                return Ok(current);
            }
        }
        write_placement(&tx, work, home_id, now_unix())?;
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
        if tx.execute(
            &format!(
                "UPDATE {table} SET work_state='abandoned', work_terminal_at=?2
                 WHERE id=?1 AND work_state='ready'"
            ),
            params![id, now],
        )? != 1
        {
            return Err(StoreError::InvalidAuthority(format!(
                "{} {} is not ready",
                work.kind(),
                work.id()
            )));
        }
        if let WorkRef::Task(task_id) = work {
            tx.execute(
                "UPDATE tasks SET abandon_requested_at=NULL,abandon_reason=NULL WHERE id=?1",
                [task_id.as_str()],
            )?;
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
    /// seconds) — the cross-Work `lf activity` timeline. Scans the Task and
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

fn map_local_home(conn: &Connection) -> StoreResult<Home> {
    conn.query_row(
        "SELECT id, route, created_at, observed_at FROM homes WHERE route='local'",
        [],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    )
    .map_err(StoreError::from)
    .and_then(|(id, route, created_at, observed_at)| {
        Ok(Home {
            id: HomeId::parse(&id).map_err(invalid_durable)?,
            route,
            created_at: OffsetDateTime::from_unix_timestamp(created_at).map_err(invalid_durable)?,
            observed_at: OffsetDateTime::from_unix_timestamp(observed_at)
                .map_err(invalid_durable)?,
        })
    })
}

fn map_home_by_id(conn: &Connection, home_id: &HomeId) -> StoreResult<Option<Home>> {
    conn.query_row(
        "SELECT id, route, created_at, observed_at FROM homes WHERE id=?1",
        [home_id.as_str()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    )
    .optional()
    .map_err(StoreError::from)?
    .map(|(id, route, created_at, observed_at)| {
        Ok(Home {
            id: HomeId::parse(&id).map_err(invalid_durable)?,
            route,
            created_at: OffsetDateTime::from_unix_timestamp(created_at).map_err(invalid_durable)?,
            observed_at: OffsetDateTime::from_unix_timestamp(observed_at)
                .map_err(invalid_durable)?,
        })
    })
    .transpose()
}

fn placement_in(conn: &Connection, work: &WorkRef) -> StoreResult<Placement> {
    find_placement_in(conn, work)?.ok_or_else(|| {
        StoreError::InvalidData(format!(
            "{} {} has no Home placement",
            work.kind(),
            work.id()
        ))
    })
}

fn find_placement_in(conn: &Connection, work: &WorkRef) -> StoreResult<Option<Placement>> {
    let row = match work {
        WorkRef::Wave(id) => conn.query_row(
            "SELECT home_id, placed_at FROM work_placements WHERE wave_id=?1",
            [id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        ),
        WorkRef::Project(id) => conn.query_row(
            "SELECT home_id, placed_at FROM work_placements WHERE project_id=?1",
            [id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        ),
        WorkRef::Task(id) => conn.query_row(
            "SELECT home_id, placed_at FROM work_placements WHERE task_id=?1",
            [id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        ),
    }
    .optional()?;
    row.map(|(home_id, placed_at)| {
        Ok(Placement {
            work: work.clone(),
            home_id: HomeId::parse(&home_id).map_err(invalid_durable)?,
            placed_at: OffsetDateTime::from_unix_timestamp(placed_at).map_err(invalid_durable)?,
        })
    })
    .transpose()
}

fn write_placement(
    tx: &Transaction<'_>,
    work: &WorkRef,
    home_id: &HomeId,
    placed_at: i64,
) -> StoreResult<()> {
    match work {
        WorkRef::Wave(id) => tx.execute(
            "INSERT INTO work_placements (wave_id, home_id, enabled, placed_at)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(wave_id) DO UPDATE SET
                home_id=excluded.home_id, placed_at=excluded.placed_at",
            params![id.as_str(), home_id.as_str(), placed_at],
        )?,
        WorkRef::Project(id) => tx.execute(
            "INSERT INTO work_placements (project_id, home_id, enabled, placed_at)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(project_id) DO UPDATE SET
                home_id=excluded.home_id, placed_at=excluded.placed_at",
            params![id.as_str(), home_id.as_str(), placed_at],
        )?,
        WorkRef::Task(id) => tx.execute(
            "INSERT INTO work_placements (task_id, home_id, enabled, placed_at)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(task_id) DO UPDATE SET
                home_id=excluded.home_id, placed_at=excluded.placed_at",
            params![id.as_str(), home_id.as_str(), placed_at],
        )?,
    };
    Ok(())
}

fn inherit_placement(
    tx: &Transaction<'_>,
    work: &WorkRef,
    parent: Option<&WorkRef>,
    placed_at: i64,
) -> StoreResult<()> {
    if find_placement_in(tx, work)?.is_some() {
        return Ok(());
    }
    let home_id = match parent {
        Some(parent) => placement_in(tx, parent)?.home_id,
        None => map_local_home(tx)?.id,
    };
    write_placement(tx, work, &home_id, placed_at)
}

pub(crate) fn work_status_in(conn: &Connection, work: &WorkRef) -> StoreResult<WorkStatus> {
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

pub(super) fn require_current_task_chapter(conn: &Connection, work: &WorkRef) -> StoreResult<()> {
    let WorkRef::Task(task) = work else {
        return Ok(());
    };
    let expired: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
         WHERE t.id=?1 AND t.started_at IS NULL AND
         (p.status != 'started' OR (SELECT count(*) FROM projects current
          WHERE current.wave_id=p.wave_id AND current.status='started') != 1))",
        [task.as_str()],
        |row| row.get(0),
    )?;
    if expired {
        return Err(StoreError::InvalidAuthority("this Task belongs to chapter history; resume the chapter transition before starting work".into()));
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

pub(crate) fn create_wave_work(
    tx: &Transaction<'_>,
    wave_id: &WaveId,
    created_at: i64,
) -> StoreResult<()> {
    let work = WorkRef::Wave(wave_id.clone());
    inherit_placement(tx, &work, None, created_at)
}

pub(crate) fn create_project_work(tx: &Transaction<'_>, project: &Project) -> StoreResult<()> {
    let project_id = tx
        .query_row(
            "SELECT id FROM projects WHERE external_project_id=?1",
            [project.plan.id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .unwrap_or_else(|| ProjectId::new().to_string());
    tx.execute(
        "INSERT OR IGNORE INTO projects (
            id, wave_id, external_project_id, created_at
         ) VALUES (?1, ?2, ?3, ?4)",
        params![
            project_id,
            project.wave_id.as_str(),
            project.plan.id.as_str(),
            project.created_at.unix_timestamp(),
        ],
    )?;
    let work = WorkRef::Project(ProjectId::parse(&project_id).map_err(invalid_durable)?);
    let parent = WorkRef::Wave(project.wave_id.clone());
    inherit_placement(
        tx,
        &work,
        Some(&parent),
        project.created_at.unix_timestamp(),
    )?;
    Ok(())
}

pub(crate) fn create_task_work(tx: &Transaction<'_>, task: &Task) -> StoreResult<()> {
    let project_id = task.project_id.as_str().to_string();
    let task_id = tx
        .query_row(
            "SELECT id FROM tasks WHERE external_issue_id=?1",
            [task.plan.id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .unwrap_or_else(|| TaskId::new().to_string());
    tx.execute(
        "INSERT OR IGNORE INTO tasks (
            id, project_id, external_issue_id, issue_identifier, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            task_id,
            project_id,
            task.plan.id.as_str(),
            task.plan.identifier,
            task.created_at.unix_timestamp(),
        ],
    )?;
    let work = WorkRef::Task(TaskId::parse(&task_id).map_err(invalid_durable)?);
    let parent = WorkRef::Project(ProjectId::parse(&project_id).map_err(invalid_durable)?);
    inherit_placement(tx, &work, Some(&parent), task.created_at.unix_timestamp())?;
    Ok(())
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
    use crate::durable::{ProjectId, TaskId};

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
            flow_id: None,
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
        // Reach the crate-private spine builder the public upsert path calls.
        {
            let mut raw = rusqlite::Connection::open(&path).unwrap();
            let tx = raw.transaction().unwrap();
            super::create_wave_work(&tx, &wave_id, 1_700_000_000).unwrap();
            tx.commit().unwrap();
        }
        let project = Project {
            id: project_id.clone(),
            plan: ProjectPlan {
                workflow: "feature".into(),
                status: crate::pm::ProjectStatus::Started,
                id: LinearProjectId::new("project-uuid").unwrap(),
                slug: "probe".to_string(),
                name: "Probe".to_string(),
                prompt_context: "Probe Task execution".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave_id.clone(),
            iteration: 0,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store.insert_project(&project).unwrap();
        let task = Task {
            id: task_id.clone(),
            plan: TaskPlan {
                id: LinearIssueId::new("task-uuid").unwrap(),
                identifier: "PROBE-1".to_string(),
                title: "Probe Task execution".to_string(),
                description: "Exercise the Task Flow store".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id,
            project_id,
            worktree: PathBuf::from("/repo.probe"),
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
        store.insert_task(&task, &pr).unwrap();
        (dir, store, task_id)
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
        flow_id: Option<String>,
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
            flow_id,
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
            .contains("Project would change AgentSession ancestry"));
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
            "UPDATE tasks SET work_state='done',work_terminal_at=1 WHERE id=?1",
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
                flow_id: None,
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
