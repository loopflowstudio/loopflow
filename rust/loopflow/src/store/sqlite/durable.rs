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

pub(super) fn require_task_worker_eligible(conn: &Connection, work: &WorkRef) -> StoreResult<()> {
    require_ready_work(conn, work)?;
    require_current_task_chapter(conn, work)
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
    use crate::durable::{
        FlowSession, ProjectId, TaskFlowBlocker, TaskId, TaskWorkerClaim, TaskWorkerClaimOutcome,
        TaskWorkerOwner,
    };
    use crate::engine::execution::NestedCursor;
    use crate::engine::flow::{ConcretePath, ConcreteXor};
    use crate::engine::transitions::{FlowDecision, FlowVerdict};
    use crate::engine::{ConcreteStep, ExecutionCursor, Skill};
    use crate::id::{ExecId, TraceId, WaveId};
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::session::{AgentSession, SessionKind, TitleSource, WorkSource};
    use crate::store::sqlite::SqliteStore;
    use crate::store::StoreResult;
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
            artifact_key: crate::run_record::new_artifact_key(),
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
            flow_session_id: None,
            work_source: Some(WorkSource::Declared),
            bound_at: None,
            kind: SessionKind::Conversation,
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
                flow: "feature".into(),
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

    fn autonomous_position(task_id: &TaskId) -> FlowSession {
        FlowSession {
            parent_id: None,
            invocation: crate::durable::test_flow_invocation(
                "task",
                0,
                "implement",
                Some("implement"),
                false,
            ),
            cursor: Default::default(),
            version: 0,
            task_id: Some(task_id.clone()),
            wave_id: None,
            cwd: "/repo.probe".into(),
            message: None,
            model: None,
            current_attempt: None,
            pending_session_id: None,
            ready_summary: None,
            worker_generation: 0,
            claim: None,
            failure: None,
            finished: false,
            updated_at: time::OffsetDateTime::now_utc(),
        }
    }

    fn review_position(task_id: &TaskId) -> FlowSession {
        let mut position = autonomous_position(task_id);
        position.invocation = crate::durable::test_flow_invocation(
            "review",
            0,
            "review-design",
            Some("review"),
            true,
        );
        position
    }

    fn owner(pid: u32) -> TaskWorkerOwner {
        TaskWorkerOwner {
            trace_id: TraceId::new(),
            exec_id: ExecId::new(),
            pid,
            started_at: 1_700_000_000,
        }
    }

    fn claim(store: &SqliteStore, task: &TaskId, flow: &FlowSession, pid: u32) -> TaskWorkerClaim {
        match store
            .claim_task_worker(
                task,
                &flow.invocation.id,
                flow.version,
                &owner(pid),
                time::OffsetDateTime::now_utc(),
            )
            .unwrap()
        {
            TaskWorkerClaimOutcome::Claimed(claim) => claim,
            outcome => panic!("unexpected claim outcome: {outcome:?}"),
        }
    }

    /// The Run the claim reserved for the current step.
    fn reserved_run(store: &SqliteStore, task: &TaskId) -> String {
        store
            .task_flow(task)
            .unwrap()
            .unwrap()
            .current_attempt
            .expect("the claim reserved the step's Run")
            .run_id
    }

    /// Launch the reserved Run: the worker's publication of its attempt.
    fn publish(
        store: &SqliteStore,
        flow: &FlowSession,
        run: &str,
        claim: &TaskWorkerClaim,
    ) -> StoreResult<()> {
        store.publish_attempt(
            flow.id(),
            flow.version,
            store.captured_sequence(run).unwrap().unwrap(),
            Some(claim),
            "codex",
            None,
        )
    }

    /// Park a Task's Flow at its review with the review's Run launched and,
    /// when given, its feedback ready.
    fn parked_review(
        store: &SqliteStore,
        task: &TaskId,
        position: &FlowSession,
        ready: Option<&str>,
    ) -> (FlowSession, String, String) {
        let flow = store.start_task_flow(task, position).unwrap();
        let flow = store.reserve_task_review(flow.id(), flow.version).unwrap();
        let session_id = crate::ops::human_session::flow_id(&flow).unwrap();
        let run = store.session(&session_id).unwrap().unwrap().artifact_key;
        store
            .publish_review_run(
                &session_id,
                store.captured_sequence(&run).unwrap().unwrap(),
                flow.version,
                "codex",
                None,
            )
            .unwrap();
        if let Some(summary) = ready {
            store
                .ready_session(&session_id, store.captured_sequence(&run).unwrap(), summary)
                .unwrap();
        }
        (store.task_flow(task).unwrap().unwrap(), session_id, run)
    }

    fn find_decision_index(steps: &[ConcreteStep]) -> usize {
        steps
            .iter()
            .position(
                |step| matches!(step, ConcreteStep::Skill(skill) if skill.policy.repeat.is_some()),
            )
            .expect("fixture Flow has a repeating decision")
    }

    fn retained_invocation(store: &SqliteStore, id: &str) -> (FlowSession, String) {
        let flow = store.flow(id).unwrap().unwrap();
        let state = store
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT state FROM flow_sessions WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .unwrap();
        (flow, state)
    }

    /// `flow` as its row reads once the Flow ended.
    fn ended(flow: &FlowSession) -> FlowSession {
        FlowSession {
            finished: true,
            claim: None,
            ..flow.clone()
        }
    }

    fn conversation(
        flow_session_id: Option<String>,
        task_id: Option<TaskId>,
        wave_id: Option<WaveId>,
    ) -> crate::session::AgentSession {
        crate::session::AgentSession {
            captured: None,
            id: uuid::Uuid::new_v4().to_string(),
            artifact_key: crate::run_record::new_artifact_key(),
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
            flow_session_id,
            work_source: Some(WorkSource::Declared),
            bound_at: None,
            kind: crate::session::SessionKind::Conversation,
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
        let position = store
            .start_task_flow(&task_id, &autonomous_position(&task_id))
            .unwrap();
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
        let taskless = crate::engine::invocation::QueuedInvocation::new(
            "taskless",
            position.invocation.steps.clone(),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flow_sessions(id,invocation_json,cwd,step_index,iteration,
            position_version,worker_generation,updated_at,state)
            VALUES(?1,?2,'/repo',0,0,1,0,1,'current')",
            rusqlite::params![taskless.id, serde_json::to_string(&taskless).unwrap()],
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
                Some(task.wave_id.clone()),
            ),
            (
                Some(position.invocation.id.clone()),
                None,
                None,
                Some(task_id.clone()),
                Some(task.wave_id.clone()),
            ),
            (
                Some(position.invocation.id.clone()),
                Some(task_id.clone()),
                Some(task.wave_id.clone()),
                Some(task_id.clone()),
                Some(task.wave_id.clone()),
            ),
            (Some(taskless.id.clone()), None, None, None, None),
            (
                Some(taskless.id.clone()),
                None,
                Some(other_wave.clone()),
                None,
                Some(other_wave.clone()),
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
        // Callers cannot publish a Run under a different node or loop pass.
        for (node, iterations) in [(Some(u32::MAX), None), (None, Some(vec![vec![99]]))] {
            let mut requested = conversation(Some(position.invocation.id.clone()), None, None);
            requested.node = node;
            requested.iterations = iterations;
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
        }
        for (invocation, task_input, wave_input) in [
            (None, Some(task_id.clone()), Some(other_wave.clone())),
            (Some(position.invocation.id.clone()), Some(other_task), None),
            (Some(taskless.id.clone()), Some(task_id.clone()), None),
            (Some("missing".into()), None, None),
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
                "UPDATE flow_sessions SET task_id=NULL WHERE id=?1",
                [&position.invocation.id]
            )
            .is_err());
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
                flow_session_id: None,
                work_source: None,
                bound_at: None,
                id: id.to_string(),
                artifact_key: crate::run_record::new_artifact_key(),
                input_published: true,
                cwd: "/repo".into(),
                skill: None,
                provider: None,
                model: None,
                node: None,
                iterations: None,
                kind: crate::session::SessionKind::Conversation,
                interactive: true,
                repo: None,
                title: id.to_string(),
                title_source: crate::session::TitleSource::Generated,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: 1,
            };
            store.create_session(session, None, None).unwrap()
        };
        let first = open("orphan", None);
        let replacement = store
            .replace_session_input(
                first.captured,
                crate::session::AgentSession {
                    caller_artifact_key: None,
                    artifact_key: crate::run_record::new_artifact_key(),
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
                    artifact_key: crate::run_record::new_artifact_key(),
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
    fn a_task_points_at_one_invocation_while_other_flows_name_it() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let managed = store
            .start_task_flow(&task_id, &autonomous_position(&task_id))
            .unwrap();
        // `lf --task X flow review-design`: a Flow about the Task, not its Flow.
        let about = crate::engine::invocation::QueuedInvocation::new(
            "review-design",
            managed.invocation.steps.clone(),
        )
        .unwrap();
        store
            .create_flow(&crate::durable::FlowSession {
                parent_id: None,
                invocation: about.clone(),
                cursor: ExecutionCursor::default(),
                version: 0,
                task_id: Some(task_id.clone()),
                wave_id: Some(task.wave_id.clone()),
                cwd: "/repo".into(),
                message: None,
                model: None,
                current_attempt: None,
                pending_session_id: None,
                ready_summary: None,
                worker_generation: 0,
                claim: None,
                failure: None,
                finished: false,
                updated_at: time::OffsetDateTime::now_utc(),
            })
            .unwrap();
        assert_eq!(store.task_flow(&task_id).unwrap(), Some(managed.clone()));

        // Attributed conversations inherit this Flow's Task without changing its managed selection.
        let session = store
            .create_session(conversation(Some(about.id.clone()), None, None), None, None)
            .unwrap();
        assert_eq!(session.task_id, Some(task_id.clone()));
        assert_eq!(session.wave_id, Some(task.wave_id.clone()));
        let other_task = TaskId::new();
        store.conn.lock().unwrap().execute(
            "INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at)
            SELECT ?1,project_id,?1,?1,'/repo.other',1 FROM tasks WHERE id=?2",
            rusqlite::params![other_task.as_str(), task_id.as_str()],
        )
        .unwrap();
        assert!(store
            .create_session(
                conversation(Some(about.id.clone()), Some(other_task.clone()), None),
                None,
                None
            )
            .is_err());
        assert_eq!(
            store
                .sessions(&crate::session::SessionFilter {
                    task: Some(task_id.to_string()),
                    interactive: None,
                    history: true,
                    ..Default::default()
                })
                .unwrap()
                .len(),
            1
        );

        // Restart closes the Task's Flow and clears the pointer; the other Flow
        // stays current, and the pointer accepts only an invocation naming X.
        store
            .restart_task_flow(
                &task,
                store.task_flow(&task.id).unwrap().as_ref(),
                "checkpoint",
            )
            .unwrap();
        assert_eq!(store.task_flow(&task_id).unwrap(), None);
        assert_eq!(
            retained_invocation(&store, &managed.invocation.id).1,
            "replaced"
        );
        let conn = store.conn.lock().unwrap();
        let state: String = conn
            .query_row(
                "SELECT state FROM flow_sessions WHERE id=?1",
                [&about.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, "current");
        let point = |task: &TaskId, invocation: &str| {
            conn.execute(
                "UPDATE tasks SET current_invocation_id=?2 WHERE id=?1",
                rusqlite::params![task.as_str(), invocation],
            )
        };
        assert!(point(&other_task, &about.id).is_err());
        point(&task_id, &about.id).unwrap();
        drop(conn);
        assert_eq!(
            store
                .task_flow(&task_id)
                .unwrap()
                .map(|position| position.invocation.id),
            Some(about.id)
        );
    }

    #[test]
    fn competing_session_assignments_start_only_the_winning_task() {
        let (dir, store, task_id) = store_with_task();
        let wave = store.task(&task_id).unwrap().unwrap().wave_id;
        let other_task = TaskId::new();
        let session = store
            .create_session(unpublished_conversation(None, None, 1), None, None)
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

    #[test]
    fn task_started_tracks_reserved_review_and_retained_history() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        assert!(!store.task_started(&task_id).unwrap());
        let position = store
            .start_task_flow(&task_id, &review_position(&task_id))
            .unwrap();
        assert!(!store.task_started(&task_id).unwrap());
        let position = store
            .reserve_task_review(position.id(), position.version)
            .unwrap();
        let session_id = crate::ops::human_session::flow_id(&position).unwrap();
        // The review's reserved Run starts the Task before anything launches.
        assert!(store.task_started(&task_id).unwrap());
        assert!(store.chapter_task_evidence(&task_id).unwrap().begun);
        assert!(!store.retire_chapter_backlog(&task_id).unwrap());

        // Activation may precede transfer. A reserved Task is already started
        // and must retain execution while its Project is still the predecessor.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE projects SET status='completed' WHERE id=?1",
                [task.project_id.as_str()],
            )
            .unwrap();
        let (reserved, run) = store.reserve_review_run(&position).unwrap();
        assert_eq!(store.session(&session_id).unwrap().unwrap(), run);
        assert!(!run.input_published);

        store
            .publish_review_run(
                &session_id,
                run.captured.unwrap(),
                reserved.version,
                "codex",
                None,
            )
            .unwrap();
        assert!(store.task_started(&task_id).unwrap());
        store
            .ready_session(&session_id, run.captured, "approved scope")
            .unwrap();
        let position = store.task_flow(&task_id).unwrap().unwrap();
        store
            .complete_task_review(&task_id, &position, "approved scope")
            .unwrap();
        assert!(store.task_flow(&task_id).unwrap().is_none());
        assert!(store.task_started(&task_id).unwrap());
    }

    #[test]
    fn review_session_retains_feedback_and_history_across_replacement_and_corrupt_neighbors() {
        let (_dir, store, task_id) = store_with_task();
        let (mut position, session_id, first_run) = parked_review(
            &store,
            &task_id,
            &review_position(&task_id),
            Some("Keep the reviewed parser behavior"),
        );
        store
            .rename_session(
                &session_id,
                None,
                "Parser review",
                crate::session::TitleSource::Human,
            )
            .unwrap();
        let first_attempt = store.session(&session_id).unwrap().unwrap();
        assert_eq!(first_attempt.work_source, Some(WorkSource::Inherited));
        let mut runs = vec![first_run];
        for _ in 0..2 {
            let stale = position.clone();
            let (reserved, run) = store.reserve_review_run(&position).unwrap();
            assert!(!run.input_published);
            assert_eq!(run.task_id, first_attempt.task_id);
            assert_eq!(run.wave_id, first_attempt.wave_id);
            assert_eq!(run.flow_session_id, first_attempt.flow_session_id);
            assert_eq!(run.work_source, Some(WorkSource::Inherited));
            assert_eq!(
                store
                    .session_inputs(&session_id)
                    .unwrap()
                    .iter()
                    .find(|input| **input == first_attempt.artifact_key),
                Some(&first_attempt.artifact_key)
            );
            assert!(store.reserve_review_run(&stale).is_err());
            assert!(store
                .ready_session(
                    &session_id,
                    store.captured_sequence(runs.last().unwrap()).unwrap(),
                    "late feedback"
                )
                .is_err());
            assert!(store
                .complete_task_review(&task_id, &stale, "late completion")
                .is_err());
            assert!(store
                .publish_review_run(
                    &session_id,
                    store
                        .captured_sequence(runs.last().unwrap())
                        .unwrap()
                        .unwrap(),
                    reserved.version,
                    "codex",
                    None
                )
                .is_err());
            store
                .publish_review_run(
                    &session_id,
                    run.captured.unwrap(),
                    reserved.version,
                    "codex",
                    None,
                )
                .unwrap();
            assert!(store
                .publish_review_run(
                    &session_id,
                    run.captured.unwrap(),
                    reserved.version,
                    "codex",
                    None
                )
                .is_err());
            runs.push(run.artifact_key);
            position = store.task_flow(&task_id).unwrap().unwrap();
            assert_eq!(
                position.ready_summary.as_deref(),
                Some("Keep the reviewed parser behavior")
            );
        }
        store
            .rename_session(
                &session_id,
                None,
                "generated suggestion",
                crate::session::TitleSource::Generated,
            )
            .unwrap();
        let session = store.session(&session_id).unwrap().unwrap();
        let current = session.clone();
        assert_eq!(session.title, "Parser review");
        assert_eq!(current.artifact_key, *runs.last().unwrap());
        let history = store.session_inputs(&session_id).unwrap();
        assert_eq!(current.node, first_attempt.node);
        assert_eq!(current.iterations, first_attempt.iterations);
        assert_eq!(history.len(), 3);
        for run in &runs {
            assert!(history.contains(run));
        }

        // Checkpointing execution cannot replace or erase conversation state.
        store
            .checkpoint_flow(
                position.id(),
                position.version,
                &position.cursor,
                None,
                None,
            )
            .unwrap();
        assert_eq!(store.session(&session_id).unwrap(), Some(session.clone()));
        assert_eq!(store.session_inputs(&session_id).unwrap(), history);

        // An unrelated malformed autonomous capture remains an identified error,
        // while direct Session discovery does not deserialize that invocation.
        let broken = TaskId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at)
                SELECT ?1,project_id,?1,?1,'/repo.broken',1 FROM tasks WHERE id=?2",
                rusqlite::params![broken.as_str(), task_id.as_str()]).unwrap();
            conn.execute("INSERT INTO flow_sessions(id,task_id,wave_id,invocation_json,step_index,iteration,
                position_version,worker_generation,updated_at,state)
                SELECT 'broken',?1,p.wave_id,'{\"id\":\"broken\",\"flow\":\"broken\",\"steps\":\"unreadable\"}',0,0,1,0,1,'current'
                FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
                [broken.as_str()]).unwrap();
            conn.execute(
                "UPDATE tasks SET current_invocation_id='broken' WHERE id=?1",
                [broken.as_str()],
            )
            .unwrap();
        }
        assert_eq!(
            store
                .sessions(&crate::session::SessionFilter::default())
                .unwrap(),
            vec![session.clone()]
        );
        assert!(store
            .task_flow(&broken)
            .unwrap_err()
            .to_string()
            .contains(broken.as_str()));
        let summary = position.ready_summary.clone().unwrap();
        store
            .complete_task_review(&task_id, &position, &summary)
            .unwrap();
        assert!(store
            .complete_task_review(&task_id, &position, &summary)
            .is_err());
        assert!(store
            .ready_session(
                &session_id,
                store.captured_sequence(runs.last().unwrap()).unwrap(),
                "after completion"
            )
            .is_err());
        let completed = store.session(&session_id).unwrap().unwrap();
        assert!(completed.completed_at.is_some());
        assert_eq!(completed.ready_summary, session.ready_summary);
        assert_eq!(store.session_inputs(&session_id).unwrap(), history);
        assert!(store
            .sessions(&crate::session::SessionFilter::default())
            .unwrap()
            .is_empty());
        let events = store.task_events_after(&task_id, 0).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.kind, TaskEventKind::FlowFinished { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn a_new_flow_replaces_the_unclaimed_current_one_and_rejects_stale_writes() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let (original, _, _) = parked_review(
            &store,
            &task_id,
            &review_position(&task_id),
            Some("keep this feedback"),
        );
        assert!(store.start_task_flow(&task_id, &original).is_err());

        // `lf task run --flow other` while the review waits: the review closes
        // as replaced and its Session stops listing.
        let replacement = store
            .start_task_flow(&task_id, &autonomous_position(&task_id))
            .unwrap();
        assert_eq!(replacement.version, 1);
        assert!(store
            .sessions(&crate::session::SessionFilter::default())
            .unwrap()
            .is_empty());
        assert_eq!(
            retained_invocation(&store, &original.invocation.id),
            (ended(&original), "replaced".into())
        );
        assert!(store
            .complete_task_review(&task_id, &original, "late")
            .is_err());
        assert_eq!(
            store.task_flow(&task_id).unwrap(),
            Some(replacement.clone())
        );

        // A worker holding the replacement fences another selection.
        let held = claim(&store, &task_id, &replacement, 401);
        assert!(store
            .start_task_flow(&task_id, &autonomous_position(&task_id))
            .is_err());
        assert_eq!(
            store.task_flow(&task_id).unwrap().unwrap().claim,
            Some(held.clone())
        );

        let stopped = store
            .release_flow(replacement.id(), held.position_version, Some(&held))
            .unwrap();
        store
            .restart_task_flow(&task, Some(&stopped), "checkpoint")
            .unwrap();
        assert!(store.task_flow(&task_id).unwrap().is_none());
        assert_eq!(
            retained_invocation(&store, &original.invocation.id),
            (ended(&original), "replaced".into())
        );
        assert!(store.chapter_task_evidence(&task_id).unwrap().begun);
        assert!(!store.retire_chapter_backlog(&task_id).unwrap());
    }

    #[test]
    fn human_completion_retains_feedback_without_leaving_a_pending_review() {
        let (_dir, store, task_id) = store_with_task();
        let (position, _, _) = parked_review(
            &store,
            &task_id,
            &review_position(&task_id),
            Some("approved scope"),
        );
        store
            .complete_task_review(&task_id, &position, "approved scope")
            .unwrap();

        assert!(store.task_flow(&task_id).unwrap().is_none());
        assert!(store
            .sessions(&crate::session::SessionFilter::default())
            .unwrap()
            .is_empty());
        assert_eq!(
            retained_invocation(&store, &position.invocation.id),
            (ended(&position), "completed".into())
        );
        assert!(store
            .complete_task_review(&task_id, &position, "duplicate")
            .is_err());
        let events = store.task_events_after(&task_id, 0).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.kind, TaskEventKind::FlowFinished { .. }))
                .count(),
            1
        );
        assert_eq!(
            store
                .work_status(&crate::durable::WorkRef::Task(task_id))
                .unwrap(),
            crate::durable::WorkStatus::Ready
        );
    }

    #[test]
    fn historical_review_cursor_can_reserve_a_replacement_attempt() {
        for progress in [
            r#"{"node_id":"decide","completed_passes":2,"direction":"keep scope"}"#,
            r#"{"repeats":{"decide":2},"direction":"keep scope"}"#,
        ] {
            let (_dir, store, task_id) = store_with_task();
            let mut position = review_position(&task_id);
            position.invocation = crate::durable::test_flow_invocation(
                "review",
                1,
                "review-design",
                Some("review"),
                true,
            );
            position.cursor.index = 1;
            position.cursor.iteration = 3;
            let (position, session_id, _) =
                parked_review(&store, &task_id, &position, Some("retained answer"));
            let original = store.session(&session_id).unwrap().unwrap();
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE flow_sessions SET review_json=?2 WHERE id=?1",
                    rusqlite::params![position.invocation.id, progress],
                )
                .unwrap();

            let recovered = store.task_flow(&task_id).unwrap().unwrap();
            let (reserved, replacement) = store.reserve_review_run(&recovered).unwrap();
            assert_eq!(reserved.cursor, recovered.cursor);
            assert_eq!(replacement.node, Some(1));
            assert_eq!(replacement.iterations, original.iterations);
            assert_ne!(replacement.artifact_key, original.artifact_key);
            let session = store.session(&session_id).unwrap().unwrap();
            assert_eq!(session.id, original.id);
            assert_eq!(session.title, original.title);
            assert_eq!(session.ready_summary, original.ready_summary);
            assert_eq!(session.artifact_key, replacement.artifact_key);
            assert_eq!(store.session_inputs(&session_id).unwrap().len(), 2);
            let retained: String = store
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT review_json FROM flow_sessions WHERE id=?1",
                    [&position.invocation.id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(retained, progress);
        }
    }

    #[test]
    fn legacy_flow_decisions_preserve_pinned_progress() {
        let (_dir, store, task_id) = store_with_task();
        let mut position = autonomous_position(&task_id);
        position.invocation = crate::engine::invocation::QueuedInvocation::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
            "pursue",
        )
        .unwrap();
        position.cursor.index = find_decision_index(&position.invocation.steps);
        position.cursor.iteration = 5;
        let position = store.start_task_flow(&task_id, &position).unwrap();
        for (saved, expected) in [
            ("continue", FlowDecision::Iterate),
            ("complete", FlowDecision::Advance),
            ("repeat", FlowDecision::Iterate),
            ("next", FlowDecision::Advance),
        ] {
            for legacy_shape in [true, false] {
                let verdict = serde_json::json!({"decision": saved, "summary": "saved proof"});
                let json = if legacy_shape {
                    serde_json::json!({"node_id": "decide", "completed_passes": 2,
                        "direction": "previous direction", "verdict": verdict})
                } else {
                    serde_json::json!({"repeats": {"decide": 2},
                        "direction": "previous direction", "verdict": verdict})
                }
                .to_string();
                store
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "UPDATE flow_sessions SET review_json=?2 WHERE state='current' AND task_id=?1",
                        rusqlite::params![task_id.as_str(), json],
                    )
                    .unwrap();
                let recovered = store.task_flow(&task_id).unwrap().unwrap();
                assert_eq!(recovered.invocation, position.invocation);
                assert_eq!(recovered.cursor.index, position.cursor.index);
                assert_eq!(recovered.cursor.iteration, position.cursor.iteration);
                assert_eq!(recovered.cursor.progress.repeats["decide"], 2);
                assert_eq!(
                    recovered.cursor.progress.direction.as_deref(),
                    Some("previous direction")
                );
                // Saved command verdicts remain historical bytes; only an exact
                // successful native completion can supply current navigation.
                assert!(recovered.cursor.progress.verdict.is_none());
                let retained: String = store
                    .conn
                    .lock()
                    .unwrap()
                    .query_row(
                        "SELECT review_json FROM flow_sessions WHERE id=?1",
                        [position.id()],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(retained, json);
                let saved_verdict: crate::engine::transitions::FlowVerdict =
                    serde_json::from_value(verdict).unwrap();
                assert_eq!(saved_verdict.decision, expected);
                assert!(recovered.failure.is_none());
            }
        }
    }

    #[test]
    fn legacy_blocked_verdict_stops_and_retries_without_advancing() {
        for legacy_shape in [true, false] {
            let (_dir, store, task_id) = store_with_task();
            let mut position = autonomous_position(&task_id);
            position.invocation = crate::engine::invocation::QueuedInvocation::load(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
                "pursue",
            )
            .unwrap();
            position.cursor.index = find_decision_index(&position.invocation.steps);
            let position = store.start_task_flow(&task_id, &position).unwrap();
            let verdict =
                serde_json::json!({"decision": "blocked", "summary": "need a policy choice"});
            let json = if legacy_shape {
                serde_json::json!({"node_id": "decide", "completed_passes": 2,
                    "direction": "previous direction", "verdict": verdict})
            } else {
                serde_json::json!({"repeats": {"decide": 2},
                    "direction": "previous direction", "verdict": verdict})
            }
            .to_string();
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE flow_sessions SET review_json=?2 WHERE state='current' AND task_id=?1",
                    rusqlite::params![task_id.as_str(), json],
                )
                .unwrap();
            let blocked = store.task_flow(&task_id).unwrap().unwrap();
            let failure = blocked.failure.as_ref().unwrap();
            assert_eq!(failure.reason, "need a policy choice");
            assert!(!failure.restart_required);
            assert_eq!(failure.observed_at, blocked.updated_at);
            assert!(!blocked.has_pending_decision());
            assert_eq!(blocked.invocation, position.invocation);
            assert_eq!(blocked.cursor.index, position.cursor.index);
            assert_eq!(blocked.cursor.progress.repeats["decide"], 2);
            assert_eq!(store.task_flow(&task_id).unwrap().unwrap(), blocked);
            claim(&store, &task_id, &blocked, 303);
            let retry = store.task_flow(&task_id).unwrap().unwrap();
            assert!(retry.failure.is_none());
            assert!(!retry.has_pending_decision());
            assert_eq!(retry.cursor.index, blocked.cursor.index);
            assert_eq!(retry.invocation, blocked.invocation);
            assert_eq!(retry.cursor.progress, blocked.cursor.progress);
        }
    }

    #[test]
    fn worker_handoff_moves_process_ownership_without_replacing_progress() {
        let (_dir, store, task_id) = store_with_task();
        let position = store
            .start_task_flow(&task_id, &autonomous_position(&task_id))
            .unwrap();
        let launcher = claim(&store, &task_id, &position, 301);
        let before = store.task_flow(&task_id).unwrap().unwrap();
        let worker = owner(302);
        let handed = store
            .handoff_task_worker(&task_id, &launcher, &worker)
            .unwrap();
        let mut expected = before;
        expected.claim = Some(handed.clone());
        assert_eq!(store.task_flow(&task_id).unwrap().unwrap(), expected);
        assert_eq!(handed.owner, worker);
        assert_eq!(handed.generation, launcher.generation);
        assert_eq!(handed.claimed_at, launcher.claimed_at);
        assert!(store
            .handoff_task_worker(&task_id, &launcher, &owner(303))
            .is_err());
        assert!(store
            .release_flow(position.id(), position.version, Some(&launcher))
            .is_err());
        assert_eq!(store.task_flow(&task_id).unwrap().unwrap(), expected);
        store
            .release_flow(position.id(), position.version, Some(&handed))
            .unwrap();
        assert!(store.task_flow(&task_id).unwrap().unwrap().claim.is_none());
    }

    #[test]
    fn loop_verdict_survives_recovery_and_rejects_unrelated_runs() {
        let (_dir, store, task_id) = store_with_task();
        let mut position = autonomous_position(&task_id);
        position.invocation = crate::engine::invocation::QueuedInvocation::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
            "feature",
        )
        .unwrap();
        position.cursor.index = find_decision_index(&position.invocation.steps);
        let position = store.start_task_flow(&task_id, &position).unwrap();
        let first = claim(&store, &task_id, &position, 301);
        let run = reserved_run(&store, &task_id);
        publish(&store, &position, &run, &first).unwrap();
        let actor = store.test_flow_turn(&run);
        let id = position.id();
        let verdict = crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Iterate,
            summary: "repair the missing case".into(),
        };
        let before = store.task_flow(&task_id).unwrap().unwrap();
        assert!(store
            .test_decision_output(&ExecId::new(), &verdict)
            .is_err());
        assert_eq!(store.task_flow(&task_id).unwrap().unwrap(), before);
        store.test_decision_output(&actor, &verdict).unwrap();
        store.test_decision_output(&actor, &verdict).unwrap();
        let different = crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Advance,
            summary: "changed mind".into(),
        };
        let before = store.task_flow(&task_id).unwrap().unwrap();
        assert!(store.test_decision_output(&actor, &different).is_err());
        assert_eq!(store.task_flow(&task_id).unwrap().unwrap(), before);
        // The worker died: retain its output as history, without a navigation candidate.
        store.test_finish_flow_turn(&actor, "interrupted");
        let replacement = store
            .reclaim_task_worker(
                &task_id,
                &first,
                &owner(302),
                time::OffsetDateTime::now_utc(),
            )
            .unwrap();
        store.test_decision_output(&actor, &verdict).unwrap();
        assert!(store.flow_output(id).is_err());
        let recovered = store.task_flow(&task_id).unwrap().unwrap();
        assert_eq!(recovered.cursor.progress.verdict, None);
        assert_eq!(recovered.claim, Some(replacement.clone()));
        assert_eq!(
            recovered
                .current_attempt
                .as_ref()
                .unwrap()
                .outcome
                .as_deref(),
            Some("interrupted")
        );
        // A reported provider failure differs from a crash: its result cannot
        // authorize a later retry; its output remains historical evidence.
        let released = store
            .release_flow(id, recovered.version, Some(&replacement))
            .unwrap();
        assert!(released.cursor.progress.verdict.is_none());
        assert!(released.claim.is_none() && released.current_attempt.is_none());
    }

    #[test]
    fn stale_driver_recovery_preserves_the_replacement_claim_and_attempt() {
        let (_dir, store, task_id) = store_with_task();
        let position = store
            .start_task_flow(&task_id, &autonomous_position(&task_id))
            .unwrap();
        let first = claim(&store, &task_id, &position, 301);
        let run = reserved_run(&store, &task_id);
        publish(&store, &position, &run, &first).unwrap();
        let replacement = store
            .reclaim_task_worker(
                &task_id,
                &first,
                &owner(302),
                time::OffsetDateTime::now_utc(),
            )
            .unwrap();
        let before = store.task_flow(&task_id).unwrap().unwrap();
        let events = store.task_events_after(&task_id, 0).unwrap();
        assert_eq!(before.claim.as_ref(), Some(&replacement));
        assert!(store.recover_flow(position.id(), Some(&first)).is_err());
        assert!(store.recover_flow(position.id(), None).is_err());
        assert_eq!(store.task_flow(&task_id).unwrap().unwrap(), before);
        assert_eq!(store.task_events_after(&task_id, 0).unwrap(), events);
        assert!(store
            .recover_flow(position.id(), Some(&replacement))
            .is_err());
        assert_eq!(store.task_flow(&task_id).unwrap().unwrap(), before);
        assert_eq!(before.current_attempt.unwrap().run_id, run);
        assert_eq!(store.task_events_after(&task_id, 0).unwrap(), events);
    }

    #[test]
    fn review_discovery_follows_the_captured_nested_step() {
        let (_dir, store, task_id) = store_with_task();
        let mut position = autonomous_position(&task_id);
        assert!(store
            .sessions(&crate::session::SessionFilter::default())
            .unwrap()
            .is_empty());
        let review = crate::durable::test_flow_invocation(
            "captured",
            0,
            "review-design",
            Some("review"),
            true,
        );
        position.invocation.steps = vec![
            ConcreteStep::Xor(ConcreteXor {
                router: Skill::named("route"),
                paths: [(
                    "selected".into(),
                    ConcretePath {
                        description: "captured path".into(),
                        steps: review.steps,
                    },
                )]
                .into_iter()
                .collect(),
                flow_parents: vec![],
            }),
            position.invocation.steps[0].clone(),
        ];
        position.cursor.child = Some(Box::new(NestedCursor::Xor {
            selected: "selected".into(),
            cursor: ExecutionCursor::default(),
        }));
        let saved = store.start_task_flow(&task_id, &position).unwrap();
        let saved = store
            .reserve_task_review(saved.id(), saved.version)
            .unwrap();
        let sessions = store
            .sessions(&crate::session::SessionFilter::default())
            .unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(
            Some(&sessions[0].artifact_key),
            saved
                .current_attempt
                .as_ref()
                .map(|attempt| &attempt.run_id)
        );
        assert_eq!(saved.current().step, "review-design");
        assert_eq!(saved.current().policy.id.as_deref(), Some("review"));
        assert_eq!(store.task_flow(&task_id).unwrap(), Some(saved.clone()));

        let mut moved = saved.cursor.clone();
        moved.child = None;
        moved.index = 1;
        store
            .checkpoint_flow(saved.id(), saved.version, &moved, None, None)
            .unwrap();
        let next = store.task_flow(&task_id).unwrap().unwrap();
        assert_eq!(next.current().step, "implement");
        assert!(next.pending_session_id.is_none());
        assert!(store
            .sessions(&crate::session::SessionFilter::default())
            .unwrap()
            .is_empty());
        assert!(store
            .checkpoint_flow(saved.id(), saved.version, &saved.cursor, None, None)
            .is_err());
        assert_eq!(store.task_flow(&task_id).unwrap(), Some(next));
    }

    #[test]
    fn nested_cursor_recovers_routes_and_verdicts_under_exact_worker_authority() {
        for routing in [true, false] {
            let (_dir, store, task_id) = store_with_task();
            let mut position = autonomous_position(&task_id);
            let body = crate::engine::invocation::QueuedInvocation::load(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
                "pursue",
            )
            .unwrap()
            .steps;
            let decision_index = find_decision_index(&body);
            let branch = ConcreteStep::Xor(ConcreteXor {
                router: Skill::named("nested-router"),
                paths: ["chosen", "other"]
                    .into_iter()
                    .map(|name| {
                        (
                            name.into(),
                            ConcretePath {
                                description: name.into(),
                                steps: body.clone(),
                            },
                        )
                    })
                    .collect(),
                flow_parents: Vec::new(),
            });
            let steps = if routing { vec![branch] } else { body };
            position.invocation.steps = vec![ConcreteStep::Xor(ConcreteXor {
                router: Skill::named("outer-router"),
                paths: [(
                    "outer".into(),
                    ConcretePath {
                        description: "outer".into(),
                        steps,
                    },
                )]
                .into_iter()
                .collect(),
                flow_parents: Vec::new(),
            })];
            position.cursor.iteration = 7;
            position.cursor.progress.repeats.insert("root".into(), 3);
            let mut leaf = ExecutionCursor {
                index: if routing { 0 } else { decision_index },
                iteration: 2,
                ..Default::default()
            };
            leaf.progress.repeats.insert("decide".into(), 1);
            leaf.progress.direction = Some("preserved direction".into());
            position.cursor.child = Some(Box::new(NestedCursor::Xor {
                selected: "outer".into(),
                cursor: leaf,
            }));
            let saved = store.start_task_flow(&task_id, &position).unwrap();
            assert_eq!(saved.current().iteration, 7);
            assert_eq!(
                saved.current().step,
                if routing {
                    "nested-router"
                } else {
                    "loop-decide"
                }
            );
            assert_eq!(saved.cursor, position.cursor);
            let id = saved.id();
            let held = claim(&store, &task_id, &saved, 501);
            let run = reserved_run(&store, &task_id);
            publish(&store, &saved, &run, &held).unwrap();
            let actor = store.test_flow_turn(&run);
            let verdict = FlowVerdict {
                decision: FlowDecision::Iterate,
                summary: "next pass".into(),
            };
            if routing {
                let before = store.task_flow(&task_id).unwrap().unwrap();
                assert!(store
                    .test_output(&ExecId::new(), &serde_json::json!({"path": "chosen"}))
                    .is_err());
                assert_eq!(store.task_flow(&task_id).unwrap().unwrap(), before);
                store
                    .test_output(&actor, &serde_json::json!({"path": "chosen"}))
                    .unwrap();
                store
                    .test_output(&actor, &serde_json::json!({"path": "chosen"}))
                    .unwrap();
                let before = store.task_flow(&task_id).unwrap().unwrap();
                assert!(store
                    .test_output(&actor, &serde_json::json!({"path": "other"}))
                    .is_err());
                assert_eq!(store.task_flow(&task_id).unwrap().unwrap(), before);
            } else {
                store.test_decision_output(&actor, &verdict).unwrap();
                assert!(store
                    .test_output(&actor, &serde_json::json!({"path": "chosen"}))
                    .is_err());
            }
            let pending = store.task_flow(&task_id).unwrap().unwrap();
            assert!(!pending.has_pending_decision());
            assert_eq!(pending.cursor.progress, saved.cursor.progress);
            store.test_finish_flow_turn(&actor, "interrupted");
            let replacement = store
                .reclaim_task_worker(
                    &task_id,
                    &held,
                    &owner(502),
                    time::OffsetDateTime::now_utc(),
                )
                .unwrap();
            let recovered = store.task_flow(&task_id).unwrap().unwrap();
            assert_eq!(recovered.cursor, pending.cursor);
            assert!(
                store.flow_output(id).is_err(),
                "interrupted outputs cannot navigate"
            );
            assert_eq!(store.task_flow(&task_id).unwrap().unwrap(), recovered);
            let cleared = if routing {
                store
                    .fail_flow(
                        id,
                        recovered.version,
                        Some(&replacement),
                        &TaskFlowBlocker {
                            captured: None,
                            reason: "router failed after publishing".into(),
                            restart_required: false,
                            observed_at: time::OffsetDateTime::now_utc(),
                        },
                    )
                    .unwrap()
            } else {
                store
                    .release_flow(id, recovered.version, Some(&replacement))
                    .unwrap()
            };
            assert_eq!(cleared.cursor, saved.cursor);
            assert!(!cleared.has_pending_decision());
            assert!(cleared.claim.is_none());
        }
    }

    #[test]
    fn human_session_runtime_survives_a_store_round_trip() {
        let (_dir, store, work) = store_with_task();
        let mut position = review_position(&work);
        position.invocation = crate::durable::test_flow_invocation(
            "review",
            1,
            "review-design",
            Some("human_review"),
            true,
        );
        position.cursor.index = 1;
        position.cursor.iteration = 2;
        let (_, _, run_id) = parked_review(&store, &work, &position, Some("Ready for review"));

        let stored = store.task_flow(&work).unwrap().unwrap();
        assert_eq!(stored.review_artifact_key(), Some(&run_id));
        assert_eq!(stored.ready_summary.as_deref(), Some("Ready for review"));
    }

    #[test]
    fn headless_position_retains_attempts_and_rejects_the_replaced_run() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let mut initial = autonomous_position(&task_id);
        let mut decision = initial.invocation.steps[0].clone();
        let ConcreteStep::Skill(skill) = &mut decision else {
            unreachable!()
        };
        skill.skill = Skill::named("loop-decide");
        skill.policy.id = Some("decide".into());
        skill.policy.repeat = Some(crate::engine::flow::RepeatPolicy {
            from: "implement".into(),
        });
        initial.invocation.steps.push(decision);
        initial.cursor.index = 1;
        let position = store.start_task_flow(&task_id, &initial).unwrap();
        let id = position.id();
        let node = position.invocation.node_id(&position.cursor).unwrap();
        let tuple = crate::engine::flow_graph::flow_iterations(
            &position.invocation.steps,
            &position.cursor,
        );
        let first_claim = claim(&store, &task_id, &position, 501);
        let first = reserved_run(&store, &task_id);
        publish(&store, &position, &first, &first_claim).unwrap();
        let first_actor = store.test_flow_turn(&first);
        let verdict = FlowVerdict {
            decision: FlowDecision::Advance,
            summary: "candidate before failure".into(),
        };
        store.test_decision_output(&first_actor, &verdict).unwrap();
        store
            .release_flow(id, position.version, Some(&first_claim))
            .unwrap();
        let failed = store.task_flow(&task_id).unwrap().unwrap();
        assert_eq!(failed.cursor.index, position.cursor.index);
        assert_eq!(failed.cursor.iteration, position.cursor.iteration);
        assert!(!failed.has_pending_decision());
        let second_claim = claim(&store, &task_id, &failed, 502);
        let second = reserved_run(&store, &task_id);
        assert_ne!(first, second);
        publish(&store, &failed, &second, &second_claim).unwrap();
        let second_actor = store.test_flow_turn(&second);
        let before = store.task_flow(&task_id).unwrap().unwrap();
        store.test_decision_output(&first_actor, &verdict).unwrap();
        assert!(
            store.flow_output(id).is_err(),
            "old output cannot satisfy the replacement"
        );
        assert!(store
            .end_flow(id, position.version, Some(&first_claim), "")
            .is_err());
        assert_eq!(store.task_flow(&task_id).unwrap().unwrap(), before);
        let session = store.session_for_artifact(&second).unwrap().unwrap();
        assert_eq!(
            store.session_for_artifact(&first).unwrap().unwrap().id,
            session.id
        );
        assert_eq!(session.node, Some(node));
        assert_eq!(session.iterations.as_ref(), Some(&tuple));
        let inputs = store.session_inputs(&session.id).unwrap();
        assert_eq!(inputs.len(), 2);
        assert!(inputs.contains(&first) && inputs.contains(&second));
        let history = store.session_history(&session.id, 0, 0).unwrap();
        store.test_decision_output(&second_actor, &verdict).unwrap();
        store.test_finish_flow_turn(&second_actor, "completed");
        store
            .end_flow(
                id,
                failed.version,
                Some(&second_claim),
                "successful attempt",
            )
            .unwrap();
        assert!(store.task_flow(&task_id).unwrap().is_none());
        assert_eq!(store.session_inputs(&session.id).unwrap(), inputs);
        assert!(store
            .end_flow(id, failed.version, Some(&second_claim), "")
            .is_err());

        let listed = store
            .sessions(&crate::session::SessionFilter {
                task: Some(task.plan.identifier.clone()),
                interactive: None,
                history: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].task_id, Some(task_id));
        assert_eq!(listed[0].wave_id, Some(task.wave_id));
        assert_eq!(
            listed[0].flow_session_id.as_ref(),
            Some(&position.invocation.id)
        );
        assert_eq!(listed[0].provider.as_deref(), Some("codex"));
        let retained = store.session_history(&session.id, 0, 0).unwrap();
        assert_eq!(&retained[..history.len()], history.as_slice());
        assert_eq!(retained.last().unwrap().payload["status"], "completed");
        store.assert_no_historical_runs();
    }

    #[test]
    fn a_claim_reserves_conversation_input_before_its_launch() {
        let (_dir, store, work) = store_with_task();
        let position = store
            .start_task_flow(&work, &autonomous_position(&work))
            .unwrap();
        claim(&store, &work, &position, 77);
        let conn = store.conn.lock().unwrap();
        let reserved: i64 = conn
            .query_row(
                "SELECT count(*) FROM agent_sessions WHERE flow_session_id=?1 AND input_published=0",
                [&position.invocation.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            reserved, 1,
            "the claim reserves the conversation's input before publication"
        );
        drop(conn);
        assert!(store.task_started(&work).unwrap());
        // The launch publishes that Run under the claim; nothing else can.
        let flow = store.task_flow(&work).unwrap().unwrap();
        let run = flow.current_attempt.clone().unwrap();
        assert!(!run.published);
        assert!(store
            .publish_attempt(flow.id(), flow.version, run.captured, None, "codex", None)
            .is_err());
        publish(&store, &flow, &run.run_id, flow.claim.as_ref().unwrap()).unwrap();
        assert!(
            store
                .task_flow(&work)
                .unwrap()
                .unwrap()
                .current_attempt
                .unwrap()
                .published
        );
        assert_eq!(
            store
                .sessions(&crate::session::SessionFilter {
                    task: Some(work.to_string()),
                    interactive: None,
                    history: true,
                    ..Default::default()
                })
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn concurrent_task_worker_claims_choose_one_worker() {
        let (dir, store, work) = store_with_task();
        let position = store
            .start_task_flow(&work, &autonomous_position(&work))
            .unwrap();
        let barrier = Arc::new(Barrier::new(20));
        let stores = (0..20)
            .map(|_| SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap())
            .collect::<Vec<_>>();
        let handles = stores
            .into_iter()
            .enumerate()
            .map(|(index, store)| {
                let work = work.clone();
                let position = position.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    store
                        .claim_task_worker(
                            &work,
                            &position.invocation.id,
                            position.version,
                            &owner(1_000 + u32::try_from(index).unwrap()),
                            time::OffsetDateTime::from_unix_timestamp(1_700_000_100).unwrap(),
                        )
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let outcomes = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        let claimed = outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                TaskWorkerClaimOutcome::Claimed(claim) => Some(claim),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].generation, 1);
        assert!(outcomes.iter().all(|outcome| match outcome {
            TaskWorkerClaimOutcome::Claimed(claim) | TaskWorkerClaimOutcome::Busy(claim) =>
                claim == claimed[0],
            TaskWorkerClaimOutcome::Stale { .. } => false,
        }));
    }

    #[test]
    fn previously_disabled_tasks_can_launch_and_recover() {
        let (_dir, store, task_id) = store_with_task();
        let position = store
            .start_task_flow(&task_id, &autonomous_position(&task_id))
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE work_placements SET enabled=0 WHERE task_id=?1",
                [task_id.as_str()],
            )
            .unwrap();
        let claim = match store
            .claim_task_worker(
                &task_id,
                &position.invocation.id,
                position.version,
                &owner(201),
                time::OffsetDateTime::now_utc(),
            )
            .unwrap()
        {
            TaskWorkerClaimOutcome::Claimed(claim) => claim,
            outcome => panic!("unexpected claim outcome: {outcome:?}"),
        };
        let replacement = store
            .reclaim_task_worker(
                &task_id,
                &claim,
                &owner(202),
                time::OffsetDateTime::now_utc(),
            )
            .unwrap();
        let recovered = store.task_flow(&task_id).unwrap().unwrap();
        assert_eq!(recovered.invocation, position.invocation);
        assert_eq!(recovered.cursor, position.cursor);
        assert_eq!(replacement.generation, claim.generation + 1);
        assert_eq!(recovered.claim, Some(replacement));
    }

    #[test]
    fn only_the_claim_holder_settles_a_position() {
        let (_dir, store, work) = store_with_task();
        let mut initial = autonomous_position(&work);
        initial.invocation = crate::durable::test_flow_invocation("task", 1, "review", None, false);
        let position = store.start_task_flow(&work, &initial).unwrap();
        let held = claim(&store, &work, &position, 101);
        let mut next = position.cursor.clone();
        next.index = 1;

        assert!(store
            .checkpoint_flow(
                position.id(),
                position.version,
                &next,
                None,
                Some("advanced")
            )
            .is_err());
        let version = store
            .checkpoint_flow(
                position.id(),
                position.version,
                &next,
                Some(&held),
                Some("advanced"),
            )
            .unwrap();
        let settled = store.task_flow(&work).unwrap().unwrap();
        assert_eq!(
            (settled.version, version.version),
            (position.version + 1, position.version + 1)
        );
        assert_eq!(settled.current().step, "review");
        assert_eq!(settled.invocation, position.invocation);
        assert_eq!(
            settled.claim,
            Some(held.clone()),
            "the driver keeps the position until it stops"
        );
        assert!(
            settled.current_attempt.is_none(),
            "a new position has no attempt"
        );
        assert!(store
            .checkpoint_flow(position.id(), position.version, &next, Some(&held), None)
            .is_err());
        let events = store.task_events_after(&work, 0).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0].kind,
            TaskEventKind::Progress { summary } if summary == "advanced"
        ));
    }

    #[test]
    fn only_the_claim_holder_finishes_a_position_without_a_successor() {
        let (_dir, store, work) = store_with_task();
        let position = store
            .start_task_flow(&work, &autonomous_position(&work))
            .unwrap();
        let held = claim(&store, &work, &position, 111);
        let final_position = store.task_flow(&work).unwrap().unwrap();

        assert!(store
            .end_flow(position.id(), position.version, None, "finished")
            .is_err());
        store
            .end_flow(position.id(), position.version, Some(&held), "finished")
            .unwrap();
        assert!(store.task_flow(&work).unwrap().is_none());
        assert_eq!(
            retained_invocation(&store, &position.invocation.id),
            (ended(&final_position), "completed".into())
        );
        let evidence = store.chapter_task_evidence(&work).unwrap();
        assert!(evidence.begun);
        assert!(!evidence.worker_claimed);
        assert!(store
            .end_flow(position.id(), position.version, Some(&held), "finished")
            .is_err());
        let events = store.task_events_after(&work, 0).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0].kind,
            TaskEventKind::FlowFinished { summary, .. } if summary == "finished"
        ));
        let next = store
            .start_task_flow(&work, &autonomous_position(&work))
            .unwrap();
        assert_eq!(next.version, position.version);
        assert!(store
            .end_flow(position.id(), position.version, Some(&held), "late finish")
            .is_err());
        assert_eq!(store.task_flow(&work).unwrap(), Some(next));
        assert_eq!(
            retained_invocation(&store, &position.invocation.id),
            (ended(&final_position), "completed".into())
        );
    }

    #[test]
    fn reclaim_fences_the_old_worker_and_increments_generation() {
        let (_dir, store, work) = store_with_task();
        let position = store
            .start_task_flow(&work, &autonomous_position(&work))
            .unwrap();
        let first = claim(&store, &work, &position, 201);
        let replacement = store
            .reclaim_task_worker(&work, &first, &owner(202), time::OffsetDateTime::now_utc())
            .unwrap();

        assert_eq!(replacement.generation, 2);
        let flow = store.task_flow(&work).unwrap().unwrap();
        let run = reserved_run(&store, &work);
        assert!(publish(&store, &flow, &run, &first).is_err());
        publish(&store, &flow, &run, &replacement).unwrap();
        let failure = TaskFlowBlocker {
            captured: None,
            reason: "provider exited".to_string(),
            restart_required: false,
            observed_at: time::OffsetDateTime::now_utc(),
        };
        let failed = store
            .fail_flow(flow.id(), flow.version, Some(&replacement), &failure)
            .unwrap();
        assert!(failed.claim.is_none());
        assert_eq!(
            failed.failure.as_ref(),
            Some(&TaskFlowBlocker {
                captured: store.captured_sequence(&run).unwrap(),
                ..failure.clone()
            })
        );
        assert!(store
            .fail_flow(flow.id(), failed.version, Some(&first), &failure)
            .is_err());
        let events = store.task_events_after(&work, 0).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0].kind,
            TaskEventKind::Failed { error, resumable: true } if error == &failure.reason
        ));

        let retried = claim(&store, &work, &failed, 205);
        assert_eq!(retried.generation, 3);
        assert!(store.task_flow(&work).unwrap().unwrap().failure.is_none());
    }
}
