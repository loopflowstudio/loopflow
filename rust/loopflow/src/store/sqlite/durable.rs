use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use time::OffsetDateTime;

use crate::child::ChildRef;
use crate::durable::{
    AbandonReceipt, Author, FlowPosition, Home, HomeId, Placement, ProjectId, RunId, Steer,
    SteerComment, TaskFlowBlocker, TaskId, TaskWorkerClaim, TaskWorkerClaimOutcome,
    TaskWorkerOwner, ToolResponseId, ToolResponseReceipt, ToolResponseWrite, WorkRef, WorkStatus,
};
use crate::id::WaveId;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::work::project::{Project, ProjectEventKind};
use crate::work::task::{Task, TaskEventKind};

use super::SqliteStore;

impl SqliteStore {
    pub fn record_flow_verdict(
        &self,
        task_id: &TaskId,
        run_id: &RunId,
        verdict: &crate::engine::transitions::FlowVerdict,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let position = claimed_position_in(&tx, task_id, run_id)?;
        super::flows::record_verdict_in(&tx, &position.invocation.id, run_id, verdict)?;
        tx.commit()?;
        Ok(())
    }

    pub fn record_flow_route(
        &self,
        task_id: &TaskId,
        run_id: &RunId,
        path: &str,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let position = claimed_position_in(&tx, task_id, run_id)?;
        super::flows::record_route_in(&tx, &position.invocation.id, run_id, path)?;
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

    pub fn set_flow_position(
        &self,
        task_id: &TaskId,
        position: &FlowPosition,
    ) -> StoreResult<FlowPosition> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored = set_flow_position_in(&tx, task_id, position)?;
        tx.commit()?;
        Ok(stored)
    }

    pub fn flow_position(&self, task_id: &TaskId) -> StoreResult<Option<FlowPosition>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        flow_position_in(&conn, task_id)
    }

    pub fn claim_task_worker(
        &self,
        task_id: &TaskId,
        expected_invocation: &str,
        expected_version: u64,
        owner: &TaskWorkerOwner,
        claimed_at: OffsetDateTime,
    ) -> StoreResult<TaskWorkerClaimOutcome> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let work = WorkRef::Task(task_id.clone());
        require_task_worker_eligible(&tx, &work)?;
        let position = flow_position_in(&tx, task_id)?.ok_or(StoreError::NotFound)?;
        if position.is_human() {
            return Err(StoreError::InvalidAuthority(
                "Review Flow positions wait for their Session to complete".to_string(),
            ));
        }
        if position.invocation.id != expected_invocation || position.version != expected_version {
            return Ok(TaskWorkerClaimOutcome::Stale {
                actual_version: position.version,
            });
        }
        if let Some(claim) = position.claim {
            return Ok(TaskWorkerClaimOutcome::Busy(claim));
        }
        let generation = position.worker_generation.checked_add(1).ok_or_else(|| {
            StoreError::InvalidData("Task worker generation overflow".to_string())
        })?;
        let claim = TaskWorkerClaim {
            invocation_id: position.invocation.id.clone(),
            generation,
            position_version: position.version,
            owner: owner.clone(),
            worker_run_id: None,
            claimed_at,
        };
        let claim_json = serde_json::to_string(&claim)?;
        let changed = tx.execute(
            &format!(
                "UPDATE flow_invocations
                 SET worker_generation=?2, claim_json=?3, failure_json=NULL, updated_at=?4, review_json=?6
                 WHERE {TASK_INVOCATION} AND position_version=?5 AND claim_json IS NULL"
            ),
            params![
                task_id.as_str(),
                i64::try_from(generation).map_err(invalid_durable)?,
                claim_json,
                claimed_at.unix_timestamp(),
                i64::try_from(expected_version).map_err(invalid_durable)?,
                serde_json::to_string(&position.cursor)?
            ],
        )?;
        if changed != 1 {
            let current = flow_position_in(&tx, task_id)?.ok_or(StoreError::NotFound)?;
            return Ok(match current.claim {
                Some(claim) => TaskWorkerClaimOutcome::Busy(claim),
                None => TaskWorkerClaimOutcome::Stale {
                    actual_version: current.version,
                },
            });
        }
        tx.commit()?;
        Ok(TaskWorkerClaimOutcome::Claimed(claim))
    }

    pub fn reclaim_task_worker(
        &self,
        task_id: &TaskId,
        expected: &TaskWorkerClaim,
        owner: &TaskWorkerOwner,
        claimed_at: OffsetDateTime,
    ) -> StoreResult<TaskWorkerClaim> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let work = WorkRef::Task(task_id.clone());
        require_task_worker_eligible(&tx, &work)?;
        let position = flow_position_in(&tx, task_id)?.ok_or(StoreError::NotFound)?;
        if position.version != expected.position_version
            || position.claim.as_ref() != Some(expected)
        {
            return Err(stale_task_worker(task_id));
        }
        let generation = position.worker_generation.checked_add(1).ok_or_else(|| {
            StoreError::InvalidData("Task worker generation overflow".to_string())
        })?;
        let replacement = TaskWorkerClaim {
            invocation_id: position.invocation.id.clone(),
            generation,
            position_version: position.version,
            owner: owner.clone(),
            worker_run_id: None,
            claimed_at,
        };
        let expected_json = serde_json::to_string(expected)?;
        let replacement_json = serde_json::to_string(&replacement)?;
        if tx.execute(
            &format!(
                "UPDATE flow_invocations
                 SET worker_generation=?2, claim_json=?3, failure_json=NULL, updated_at=?4, review_json=?7
                 WHERE {TASK_INVOCATION} AND position_version=?5 AND claim_json=?6"
            ),
            params![
                task_id.as_str(),
                i64::try_from(generation).map_err(invalid_durable)?,
                replacement_json,
                claimed_at.unix_timestamp(),
                i64::try_from(position.version).map_err(invalid_durable)?,
                expected_json,
                serde_json::to_string(&position.cursor)?
            ],
        )? != 1
        {
            return Err(stale_task_worker(task_id));
        }
        tx.commit()?;
        Ok(replacement)
    }

    pub fn bind_task_worker_run(
        &self,
        task_id: &TaskId,
        expected: &TaskWorkerClaim,
        worker_run_id: &RunId,
        owner: &TaskWorkerOwner,
    ) -> StoreResult<TaskWorkerClaim> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_ready_work(&tx, &WorkRef::Task(task_id.clone()))?;
        let bound = TaskWorkerClaim {
            invocation_id: expected.invocation_id.clone(),
            generation: expected.generation,
            position_version: expected.position_version,
            owner: owner.clone(),
            worker_run_id: Some(worker_run_id.clone()),
            claimed_at: expected.claimed_at,
        };
        let expected_json = serde_json::to_string(expected)?;
        let bound_json = serde_json::to_string(&bound)?;
        if tx.execute(
            &format!(
                "UPDATE flow_invocations SET claim_json=?2
                 WHERE {TASK_INVOCATION} AND position_version=?3 AND claim_json=?4"
            ),
            params![
                task_id.as_str(),
                bound_json,
                i64::try_from(expected.position_version).map_err(invalid_durable)?,
                expected_json
            ],
        )? != 1
        {
            return Err(stale_task_worker(task_id));
        }
        if expected.worker_run_id.is_some() {
            return Err(StoreError::InvalidAuthority(
                "Task claim already has a Run".into(),
            ));
        }
        let position = flow_position_in(&tx, task_id)?.ok_or(StoreError::NotFound)?;
        let cwd: String = tx.query_row(
            "SELECT worktree FROM tasks WHERE id=?1",
            [task_id.as_str()],
            |row| row.get(0),
        )?;
        super::runs::insert_run_in(
            &tx,
            crate::session::Run {
                id: worker_run_id.clone(),
                session_id: None,
                invocation_id: Some(position.invocation.id.clone()),
                node: None,
                iterations: None,
                attempt: None,
                task_id: Some(task_id.clone()),
                wave_id: None,
                work_source: Some(crate::session::WorkSource::Inherited),
                created_at: now_unix(),
                published: true,
                cwd: cwd.into(),
                skill: Some(position.current().step),
                provider: None,
                model: None,
                caller_run_id: None,
                ended: None,
            },
        )?;
        super::runs::select_attempt_in(
            &tx,
            &position.invocation.id,
            position.version,
            worker_run_id,
        )?;
        tx.commit()?;
        Ok(bound)
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
        let task = super::children::task_on(tx, task_id)?.ok_or(StoreError::NotFound)?;
        let event = super::children::insert_task_event_in(
            tx,
            &task,
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
                let task = super::children::task_on(&tx, task_id)?.ok_or(StoreError::NotFound)?;
                super::children::insert_task_event_in(&tx, &task, &TaskEventKind::Interrupt)?.id
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

fn require_task_worker_eligible(conn: &Connection, work: &WorkRef) -> StoreResult<()> {
    require_ready_work(conn, work)?;
    require_current_task_chapter(conn, work)
}

pub(super) fn require_current_task_chapter(conn: &Connection, work: &WorkRef) -> StoreResult<()> {
    let WorkRef::Task(task) = work else {
        return Ok(());
    };
    let expired: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
         JOIN wave_chapters c ON c.wave_id=p.wave_id AND c.current=1
         WHERE t.id=?1 AND c.project_id != p.external_project_id
         AND NOT EXISTS(SELECT 1 FROM task_events WHERE task_id=t.id AND json_extract(kind_json,'$.kind')='started')
         AND NOT EXISTS(SELECT 1 FROM flow_invocations WHERE task_id=t.id AND worker_generation>0)
         AND t.started_at IS NULL)",
        [task.as_str()], |row| row.get(0),
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

/// The Task's Flow while `run_id` is its claimed worker Run.
fn claimed_position_in(
    conn: &Connection,
    task_id: &TaskId,
    run_id: &RunId,
) -> StoreResult<FlowPosition> {
    require_ready_work(conn, &WorkRef::Task(task_id.clone()))?;
    let position = flow_position_in(conn, task_id)?.ok_or(StoreError::NotFound)?;
    if position
        .claim
        .as_ref()
        .and_then(|claim| claim.worker_run_id.as_ref())
        != Some(run_id)
    {
        return Err(StoreError::InvalidAuthority(
            "only the current claimed Task worker Run may record on its Flow".to_string(),
        ));
    }
    Ok(position)
}

/// The invocation a Task points at: the one its worker advances. Every other
/// invocation naming the Task is a Flow about it. `?1` is the Task id.
pub(super) const TASK_INVOCATION: &str = "id=(SELECT current_invocation_id FROM tasks WHERE id=?1)";

const FLOW_POSITION_SELECT: &str = "SELECT task_id, invocation_json,
            (SELECT CASE WHEN r.published=1 THEN r.id END FROM sessions s
                JOIN runs r ON r.id=s.current_run_id WHERE s.id=flow_invocations.pending_session_id),
            (SELECT ready_summary FROM sessions WHERE id=flow_invocations.pending_session_id),
            step_index, iteration, position_version, worker_generation,
            claim_json, failure_json, updated_at, review_json
     FROM flow_invocations";

type StoredFlowPosition = (
    String,
    String,
    Option<String>,
    Option<String>,
    i64,
    i64,
    i64,
    i64,
    Option<String>,
    Option<String>,
    i64,
    Option<String>,
);

fn read_flow_position_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredFlowPosition> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
    ))
}

pub(super) fn flow_position_in(
    conn: &Connection,
    task_id: &TaskId,
) -> StoreResult<Option<FlowPosition>> {
    let row = conn
        .query_row(
            &format!("{FLOW_POSITION_SELECT} WHERE {TASK_INVOCATION}"),
            [task_id.as_str()],
            read_flow_position_row,
        )
        .optional()?;
    row.map(|row| {
        decode_flow_position(row).map_err(|error| {
            StoreError::InvalidData(format!(
                "Task {task_id} saved Invocation is unreadable; its bytes are unchanged: {error}"
            ))
        })
    })
    .transpose()
}

pub(super) fn set_flow_position_in(
    conn: &rusqlite::Transaction<'_>,
    task_id: &TaskId,
    position: &FlowPosition,
) -> StoreResult<FlowPosition> {
    require_ready_work(conn, &WorkRef::Task(task_id.clone()))?;
    require_current_task_chapter(conn, &WorkRef::Task(task_id.clone()))?;
    validate_flow_position(task_id, position)?;
    if position.claim.is_some() {
        return Err(StoreError::InvalidAuthority(
            "set_flow_position cannot write an advancement claim".to_string(),
        ));
    }
    let failure_json = position
        .failure
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    let changed = if position.version == 0 {
        let inserted = conn.execute(
            "INSERT INTO flow_invocations (
                id, task_id, wave_id, invocation_json,
                step_index, iteration, position_version, worker_generation,
                claim_json, failure_json, updated_at, review_json, state
             ) VALUES (
                ?8, ?1, (SELECT p.wave_id FROM tasks t JOIN projects p ON p.id=t.project_id
                    WHERE t.id=?1),
                ?2, ?3, ?4, 1, 0, NULL, ?5, ?6, ?7, 'current'
             )
             ON CONFLICT DO NOTHING",
            params![
                task_id.as_str(),
                serde_json::to_string(&position.invocation)?,
                i64::try_from(position.cursor.index).map_err(invalid_durable)?,
                i64::from(position.cursor.iteration),
                failure_json,
                position.updated_at.unix_timestamp(),
                serde_json::to_string(&position.cursor)?,
                position.invocation.id
            ],
        )?;
        if inserted == 1 {
            conn.execute(
                "UPDATE tasks SET current_invocation_id=?2
                 WHERE id=?1 AND current_invocation_id IS NULL",
                params![task_id.as_str(), position.invocation.id],
            )?
        } else {
            0
        }
    } else {
        conn.execute(
            &format!(
                "UPDATE flow_invocations
                 SET invocation_json=?2,
                 step_index=?3, iteration=?4, position_version=position_version + 1,
                 worker_generation=0, claim_json=NULL, failure_json=?5,
                 updated_at=?6, review_json=?8
                 WHERE {TASK_INVOCATION} AND position_version=?7 AND claim_json IS NULL AND id=?9"
            ),
            params![
                task_id.as_str(),
                serde_json::to_string(&position.invocation)?,
                i64::try_from(position.cursor.index).map_err(invalid_durable)?,
                i64::from(position.cursor.iteration),
                failure_json,
                position.updated_at.unix_timestamp(),
                i64::try_from(position.version).map_err(invalid_durable)?,
                serde_json::to_string(&position.cursor)?,
                position.invocation.id
            ],
        )?
    };
    if changed != 1 {
        return Err(StoreError::InvalidAuthority(format!(
            "Flow position for Task {task_id} changed or is actively claimed"
        )));
    }
    super::sessions::save_review_in(conn, position)?;
    flow_position_in(conn, task_id)?.ok_or(StoreError::NotFound)
}

pub(super) fn block_task_flow_in(
    conn: &Connection,
    task_id: &TaskId,
    expected: &TaskWorkerClaim,
    failure: &TaskFlowBlocker,
) -> StoreResult<FlowPosition> {
    require_ready_work(conn, &WorkRef::Task(task_id.clone()))?;
    if let Some(run) = &expected.worker_run_id {
        super::runs::require_attempt_in(conn, &expected.invocation_id, run)?;
    }
    let expected_json = serde_json::to_string(expected)?;
    let mut failure = failure.clone();
    failure.run_id = expected.worker_run_id.clone();
    let failure_json = serde_json::to_string(&failure)?;
    let mut position = flow_position_in(conn, task_id)?.ok_or(StoreError::NotFound)?;
    let leaf = position.cursor.leaf_mut();
    leaf.progress.verdict = None;
    leaf.route = None;
    if conn.execute(
        &format!(
            "UPDATE flow_invocations
             SET claim_json=NULL, failure_json=?2, updated_at=?3, review_json=?6
             WHERE {TASK_INVOCATION} AND position_version=?4 AND claim_json=?5"
        ),
        params![
            task_id.as_str(),
            failure_json,
            failure.observed_at.unix_timestamp(),
            i64::try_from(expected.position_version).map_err(invalid_durable)?,
            expected_json,
            serde_json::to_string(&position.cursor)?
        ],
    )? != 1
    {
        return Err(stale_task_worker(task_id));
    }
    flow_position_in(conn, task_id)?.ok_or(StoreError::NotFound)
}

pub(super) fn release_task_worker_in(
    conn: &Connection,
    task_id: &TaskId,
    expected: &TaskWorkerClaim,
) -> StoreResult<FlowPosition> {
    require_ready_work(conn, &WorkRef::Task(task_id.clone()))?;
    if let Some(run) = &expected.worker_run_id {
        super::runs::require_attempt_in(conn, &expected.invocation_id, run)?;
    }
    let expected_json = serde_json::to_string(expected)?;
    let mut position = flow_position_in(conn, task_id)?.ok_or(StoreError::NotFound)?;
    let leaf = position.cursor.leaf_mut();
    leaf.progress.verdict = None;
    leaf.route = None;
    if conn.execute(
        &format!(
            "UPDATE flow_invocations
             SET claim_json=NULL, failure_json=NULL, updated_at=?2,
             review_json=?5
             WHERE {TASK_INVOCATION} AND position_version=?3 AND claim_json=?4"
        ),
        params![
            task_id.as_str(),
            OffsetDateTime::now_utc().unix_timestamp(),
            i64::try_from(expected.position_version).map_err(invalid_durable)?,
            expected_json,
            serde_json::to_string(&position.cursor)?
        ],
    )? != 1
    {
        return Err(stale_task_worker(task_id));
    }
    flow_position_in(conn, task_id)?.ok_or(StoreError::NotFound)
}

pub(super) fn settle_task_worker_in(
    conn: &rusqlite::Transaction<'_>,
    task_id: &TaskId,
    expected: &TaskWorkerClaim,
    next: &FlowPosition,
) -> StoreResult<FlowPosition> {
    validate_flow_position(task_id, next)?;
    if expected.worker_run_id.is_none() {
        return Err(StoreError::InvalidAuthority(
            "only a bound Task worker Run may settle a Task Flow".to_string(),
        ));
    }
    if next.version != expected.position_version || next.claim.is_some() || next.failure.is_some() {
        return Err(stale_task_worker(task_id));
    }
    require_ready_work(conn, &WorkRef::Task(task_id.clone()))?;
    if let Some(run) = &expected.worker_run_id {
        super::runs::require_attempt_in(conn, &expected.invocation_id, run)?;
    }
    let expected_json = serde_json::to_string(expected)?;
    if conn.execute(
        &format!(
            "UPDATE flow_invocations
             SET invocation_json=?2,
             step_index=?3, iteration=?4, position_version=position_version + 1,
             worker_generation=0, claim_json=NULL, failure_json=NULL,
             updated_at=?5, review_json=?8
             WHERE {TASK_INVOCATION} AND position_version=?6 AND claim_json=?7"
        ),
        params![
            task_id.as_str(),
            serde_json::to_string(&next.invocation)?,
            i64::try_from(next.cursor.index).map_err(invalid_durable)?,
            i64::from(next.cursor.iteration),
            next.updated_at.unix_timestamp(),
            i64::try_from(expected.position_version).map_err(invalid_durable)?,
            expected_json,
            serde_json::to_string(&next.cursor)?
        ],
    )? != 1
    {
        return Err(stale_task_worker(task_id));
    }
    super::sessions::save_review_in(conn, next)?;
    flow_position_in(conn, task_id)?.ok_or(StoreError::NotFound)
}

pub(super) fn finish_task_flow_in(
    conn: &Connection,
    task_id: &TaskId,
    expected: &TaskWorkerClaim,
) -> StoreResult<()> {
    if expected.worker_run_id.is_none() {
        return Err(StoreError::InvalidAuthority(
            "only a bound Task worker Run may finish a Task Flow".to_string(),
        ));
    }
    require_ready_work(conn, &WorkRef::Task(task_id.clone()))?;
    if let Some(run) = &expected.worker_run_id {
        super::runs::require_attempt_in(conn, &expected.invocation_id, run)?;
    }
    let expected_json = serde_json::to_string(expected)?;
    if conn.execute(
        &format!(
            "UPDATE flow_invocations SET state='completed', ended_at=?4
             WHERE {TASK_INVOCATION} AND position_version=?2 AND claim_json=?3"
        ),
        params![
            task_id.as_str(),
            i64::try_from(expected.position_version).map_err(invalid_durable)?,
            expected_json,
            now_unix()
        ],
    )? != 1
    {
        return Err(stale_task_worker(task_id));
    }
    conn.execute(
        "UPDATE tasks SET current_invocation_id=NULL WHERE id=?1",
        [task_id.as_str()],
    )?;
    Ok(())
}

fn decode_flow_progress(
    json: Option<&str>,
    failure: &mut Option<TaskFlowBlocker>,
    observed_at: OffsetDateTime,
) -> StoreResult<crate::engine::transitions::FlowProgress> {
    let Some(json) = json else {
        return Ok(Default::default());
    };
    let mut value: serde_json::Value = serde_json::from_str(json)?;
    if let Some(node) = value
        .get("node_id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
    {
        let count = value
            .get("completed_passes")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        value = serde_json::json!({
            "repeats": {node: count}, "direction": value.get("direction"),
            "verdict": value.get("verdict"),
        });
    }
    // Blocked was historically serialized as a navigation verdict. Preserve
    // its evidence as a retryable stop without inventing a forward decision.
    if value.pointer("/verdict/decision").and_then(|v| v.as_str()) == Some("blocked") {
        let reason = value
            .pointer("/verdict/summary")
            .and_then(|v| v.as_str())
            .ok_or_else(|| StoreError::InvalidData("blocked Flow verdict has no summary".into()))?;
        let reason = if reason.trim().is_empty() {
            "legacy Flow verdict is blocked without evidence"
        } else {
            reason
        };
        match failure {
            Some(existing) if existing.reason != reason => {
                existing
                    .reason
                    .push_str(&format!("\nLegacy Flow blocker: {reason}"));
            }
            Some(_) => {}
            None => {
                *failure = Some(TaskFlowBlocker {
                    run_id: None,
                    reason: reason.into(),
                    restart_required: false,
                    observed_at,
                })
            }
        }
        value["verdict"] = serde_json::Value::Null;
    }
    Ok(serde_json::from_value(value)?)
}

fn decode_flow_position(
    (
        task_id,
        invocation_json,
        session_run_id,
        ready_summary,
        step_index,
        iteration,
        position_version,
        worker_generation,
        claim_json,
        failure_json,
        updated_at,
        review_json,
    ): StoredFlowPosition,
) -> StoreResult<FlowPosition> {
    let updated_at = OffsetDateTime::from_unix_timestamp(updated_at).map_err(invalid_durable)?;
    let mut failure = failure_json
        .map(|failure| serde_json::from_str::<TaskFlowBlocker>(&failure))
        .transpose()?;
    let cursor = decode_flow_cursor(
        review_json.as_deref(),
        step_index,
        iteration,
        &mut failure,
        updated_at,
    )?;
    let position = FlowPosition {
        cursor,
        task_id: TaskId::parse(&task_id).map_err(invalid_durable)?,
        invocation: serde_json::from_str(&invocation_json)?,
        session_run_id: session_run_id
            .map(|run_id| RunId::parse(&run_id).map_err(invalid_durable))
            .transpose()?,
        ready_summary,
        version: u64::try_from(position_version).map_err(invalid_durable)?,
        worker_generation: u64::try_from(worker_generation).map_err(invalid_durable)?,
        claim: claim_json
            .map(|claim| serde_json::from_str::<TaskWorkerClaim>(&claim))
            .transpose()?,
        failure,
        updated_at,
    };
    validate_flow_position(&position.task_id, &position)?;
    Ok(position)
}

pub(super) fn decode_flow_cursor(
    review_json: Option<&str>,
    step_index: i64,
    iteration: i64,
    failure: &mut Option<TaskFlowBlocker>,
    updated_at: OffsetDateTime,
) -> StoreResult<crate::engine::ExecutionCursor> {
    let root_index = usize::try_from(step_index).map_err(invalid_durable)?;
    let root_iteration = u32::try_from(iteration).map_err(invalid_durable)?;
    let saved = review_json
        .map(serde_json::from_str::<serde_json::Value>)
        .transpose()?;
    let cursor = match saved {
        Some(value) if value.get("index").is_some() => serde_json::from_value(value)?,
        _ => crate::engine::ExecutionCursor {
            index: root_index,
            iteration: root_iteration,
            progress: decode_flow_progress(review_json, failure, updated_at)?,
            ..Default::default()
        },
    };
    if cursor.index != root_index || cursor.iteration != root_iteration {
        return Err(StoreError::InvalidData(
            "stored Flow cursor does not match its root projection".into(),
        ));
    }
    Ok(cursor)
}

fn validate_flow_position(task_id: &TaskId, position: &FlowPosition) -> StoreResult<()> {
    crate::engine::flow::validate_repeats(&position.invocation.steps).map_err(invalid_durable)?;
    if &position.task_id != task_id {
        return Err(StoreError::InvalidAuthority(
            "Flow position does not belong to this Task".to_string(),
        ));
    }
    let step = position
        .current_checked()
        .ok_or_else(|| StoreError::InvalidData("Flow position has no current step".to_string()))?;
    if step.flow.trim().is_empty() || step.step.trim().is_empty() {
        return Err(StoreError::InvalidData(
            "flow and step cannot be empty".to_string(),
        ));
    }
    if step.policy.human && step.policy.id.is_none() {
        return Err(StoreError::InvalidData(
            "review flow positions require a stable node id".to_string(),
        ));
    }
    if position.claim.as_ref().is_some_and(|claim| {
        claim.invocation_id != position.invocation.id
            || claim.position_version != position.version
            || claim.generation != position.worker_generation
    }) {
        return Err(StoreError::InvalidData(
            "Task worker claim does not belong to its Flow position".to_string(),
        ));
    }
    Ok(())
}

fn stale_task_worker(task_id: &TaskId) -> StoreError {
    StoreError::InvalidAuthority(format!("Task worker for {task_id} is stale"))
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

    use super::super::runs::{insert_run_in, read_run};
    use crate::durable::{
        FlowPosition, ProjectId, RunId, TaskFlowBlocker, TaskId, TaskWorkerClaimOutcome,
        TaskWorkerOwner,
    };
    use crate::engine::execution::NestedCursor;
    use crate::engine::flow::{ConcretePath, ConcreteXor};
    use crate::engine::transitions::{FlowDecision, FlowVerdict};
    use crate::engine::{ConcreteStep, ExecutionCursor, Skill};
    use crate::id::{ExecId, TraceId, WaveId};
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::session::{Run, WorkSource};
    use crate::store::sqlite::SqliteStore;
    use crate::work::chapter::{Chapter, ChapterId, ChapterPhase};
    use crate::work::project::Project;
    use crate::work::task::{PmWritebackState, Task, TaskEventKind, TaskPr, TaskPrId};

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

    fn autonomous_position(task_id: &TaskId) -> FlowPosition {
        FlowPosition {
            task_id: task_id.clone(),
            invocation: crate::durable::test_flow_invocation(
                "task",
                0,
                "implement",
                Some("implement"),
                false,
            ),
            session_run_id: None,
            ready_summary: None,
            cursor: Default::default(),
            version: 0,
            worker_generation: 0,
            claim: None,
            failure: None,
            updated_at: time::OffsetDateTime::now_utc(),
        }
    }

    fn owner(pid: u32) -> TaskWorkerOwner {
        TaskWorkerOwner {
            trace_id: TraceId::new(),
            exec_id: ExecId::new(),
            pid,
            started_at: 1_700_000_000,
        }
    }

    fn find_decision_index(steps: &[ConcreteStep]) -> usize {
        steps
            .iter()
            .position(
                |step| matches!(step, ConcreteStep::Skill(skill) if skill.policy.repeat.is_some()),
            )
            .expect("fixture Flow has a repeating decision")
    }

    fn retained_invocation(store: &SqliteStore, id: &str) -> (FlowPosition, String) {
        let conn = store.conn.lock().unwrap();
        let position = conn
            .query_row(
                &format!("{} WHERE id=?1", super::FLOW_POSITION_SELECT),
                [id],
                super::read_flow_position_row,
            )
            .unwrap();
        let state = conn
            .query_row(
                "SELECT state FROM flow_invocations WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        (super::decode_flow_position(position).unwrap(), state)
    }

    #[test]
    fn run_constructor_infers_ancestors_and_rejects_conflicts_atomically() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let position = store
            .set_flow_position(&task_id, &autonomous_position(&task_id))
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
            "INSERT INTO flow_invocations(id,invocation_json,step_index,iteration,
            position_version,worker_generation,updated_at,state)
            VALUES(?1,?2,0,0,1,0,1,'current')",
            rusqlite::params![taskless.id, serde_json::to_string(&taskless).unwrap()],
        )
        .unwrap();
        let new_run = |invocation_id, task_id, wave_id| Run {
            id: RunId::new(),
            session_id: None,
            invocation_id,
            node: None,
            iterations: None,
            attempt: None,
            task_id,
            wave_id,
            work_source: Some(WorkSource::Declared),
            created_at: 100,
            published: false,
            cwd: "/repo".into(),
            skill: Some("implement".into()),
            provider: None,
            model: None,
            caller_run_id: None,
            ended: None,
        };
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
            let saved = insert_run_in(&tx, new_run(invocation, task_input, wave_input)).unwrap();
            assert_eq!(saved.task_id, expected_task);
            assert_eq!(saved.wave_id, expected_wave);
            tx.commit().unwrap();
            let read = conn
                .query_row(
                    &format!(
                        "SELECT {} FROM runs WHERE id=?1",
                        crate::store::sqlite::runs::RUN_COLUMNS
                    ),
                    [saved.id.as_str()],
                    |row| read_run(row, 0),
                )
                .unwrap()
                .unwrap();
            assert_eq!(read, saved);
        }
        let run_count: i64 = conn
            .query_row("SELECT count(*) FROM runs", [], |row| row.get(0))
            .unwrap();
        // Callers cannot publish a Run under a different node or loop pass.
        for (node, iterations) in [(Some(u32::MAX), None), (None, Some(vec![vec![99]]))] {
            let mut requested = new_run(Some(position.invocation.id.clone()), None, None);
            requested.node = node;
            requested.iterations = iterations;
            {
                let tx = conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .unwrap();
                assert!(insert_run_in(&tx, requested).is_err());
            }
            assert_eq!(
                conn.query_row("SELECT count(*) FROM runs", [], |row| row.get::<_, i64>(0))
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
            let mut requested = new_run(invocation, task_input, wave_input);
            requested.session_id = Some("failed-conversation".into());
            {
                let tx = conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .unwrap();
                tx.execute(
                    "INSERT INTO sessions(id,current_run_id,title,title_source,created_at)
                    VALUES('failed-conversation',?1,'Must roll back','generated',100)",
                    [requested.id.as_str()],
                )
                .unwrap();
                assert!(insert_run_in(&tx, requested).is_err());
            }
            assert_eq!(
                conn.query_row("SELECT count(*) FROM runs", [], |row| row.get::<_, i64>(0))
                    .unwrap(),
                run_count
            );
            assert_eq!(
                conn.query_row("SELECT count(*) FROM sessions", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        // Changing a parent cannot invalidate an already reserved child's ancestry.
        assert!(conn
            .execute(
                "UPDATE flow_invocations SET task_id=NULL WHERE id=?1",
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
    fn run_reservation_serializes_with_project_ancestry_changes() {
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
        let saved = insert_run_in(
            &tx,
            Run {
                id: RunId::new(),
                session_id: None,
                invocation_id: None,
                node: None,
                iterations: None,
                attempt: None,
                task_id: Some(task_id),
                wave_id: None,
                work_source: Some(WorkSource::Checkout),
                created_at: 100,
                published: false,
                cwd: "/repo".into(),
                skill: None,
                provider: None,
                model: None,
                caller_run_id: None,
                ended: None,
            },
        )
        .unwrap();
        tx.commit().unwrap();
        let error = writer.join().unwrap().unwrap_err();
        assert!(error
            .to_string()
            .contains("Project would change Run ancestry"));
        let (run_wave, project_wave): (String, String) = conn
            .query_row(
                "SELECT r.wave_id,p.wave_id FROM runs r JOIN tasks t ON t.id=r.task_id
             JOIN projects p ON p.id=t.project_id WHERE r.id=?1",
                [saved.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(run_wave, task.wave_id.as_str());
        assert_eq!(run_wave, project_wave);
    }

    #[test]
    fn run_assignment_starts_tasks_once_and_rejects_reassignment_atomically() {
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
        let make_run = |task_id, wave_id, created_at| Run {
            id: RunId::new(),
            session_id: None,
            invocation_id: None,
            node: None,
            iterations: None,
            attempt: None,
            task_id,
            wave_id,
            work_source: Some(WorkSource::Declared),
            created_at,
            published: false,
            cwd: "/repo".into(),
            skill: None,
            provider: None,
            model: None,
            caller_run_id: None,
            ended: None,
        };
        let before = crate::store::rows::now_unix();
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        let reserved = insert_run_in(&tx, make_run(Some(task_id.clone()), None, 1)).unwrap();
        let orphan = insert_run_in(&tx, make_run(None, None, 2)).unwrap();
        let wave_only = insert_run_in(&tx, make_run(None, Some(task.wave_id.clone()), 3)).unwrap();
        let conflicting = insert_run_in(&tx, make_run(None, Some(other_wave.clone()), 4)).unwrap();
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
            "UPDATE runs SET task_id=?2,wave_id=?3,work_source='bound' WHERE id=?1",
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
            "UPDATE runs SET task_id=?2,work_source='bound' WHERE id=?1",
            rusqlite::params![wave_only.id.as_str(), bound_task.as_str()],
        )
        .unwrap();
        for created_at in [0, bound_at + 1000] {
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .unwrap();
            insert_run_in(&tx, make_run(Some(bound_task.clone()), None, created_at)).unwrap();
            let later_bind = insert_run_in(&tx, make_run(None, None, created_at)).unwrap();
            tx.execute(
                "UPDATE runs SET task_id=?2,wave_id=?3 WHERE id=?1",
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
                    "UPDATE runs SET task_id=?2,wave_id=?3 WHERE id=?1",
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
            .execute("DELETE FROM runs WHERE id=?1", [reserved.id.as_str()])
            .is_err());
        // Failed enclosing writes roll back both the reservation and Started.
        {
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .unwrap();
            insert_run_in(&tx, make_run(Some(losing_task.clone()), None, 5)).unwrap();
            assert!(started(&tx, &losing_task).is_some());
        }
        assert_eq!(started(&conn, &losing_task), None);
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM tasks t WHERE
            (started_at IS NOT NULL) != EXISTS(SELECT 1 FROM runs WHERE task_id=t.id)",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn bind_fills_every_run_of_a_session_and_refuses_another_wave() {
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
            let run = Run {
                id: RunId::new(),
                session_id: Some(id.to_string()),
                invocation_id: None,
                node: None,
                iterations: None,
                attempt: None,
                task_id: None,
                wave_id: wave,
                work_source: None,
                created_at: 1,
                published: true,
                cwd: "/repo".into(),
                skill: None,
                provider: None,
                model: None,
                caller_run_id: None,
                ended: None,
            };
            let session = crate::session::Session {
                id: id.to_string(),
                current_run_id: run.id.clone(),
                kind: crate::session::SessionKind::Interactive,
                title: id.to_string(),
                title_source: crate::session::TitleSource::Generated,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: 1,
            };
            store.create_session(session, run, None).unwrap().1
        };
        let first = open("orphan", None);
        let replacement = store
            .replace_session_run(
                &first.id,
                Run {
                    id: RunId::new(),
                    ..first.clone()
                },
            )
            .unwrap();
        open("elsewhere", Some(other_wave));

        let (_, current) = store.bind_session("orphan", &task_id).unwrap();
        assert_eq!(current.id, replacement.id);
        for run in store.session_runs("orphan").unwrap() {
            assert_eq!(run.task_id, Some(task_id.clone()));
            assert_eq!(run.wave_id, Some(task.wave_id.clone()));
            assert_eq!(run.work_source, Some(WorkSource::Bound));
        }
        let refused = store.bind_session("elsewhere", &task_id).unwrap_err();
        assert!(refused.to_string().contains("Wave other"), "{refused}");
        assert_eq!(store.session("elsewhere").unwrap().unwrap().1.task_id, None);
        let listed = store.runs(None, None, Some(task_id.as_str()), None, 0);
        assert_eq!(listed.unwrap().len(), 2);
    }

    #[test]
    fn a_task_points_at_one_invocation_while_other_flows_name_it() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let managed = store
            .set_flow_position(&task_id, &autonomous_position(&task_id))
            .unwrap();
        // `lf --task X flow review-design`: a Flow about the Task, not its Flow.
        let about = crate::engine::invocation::QueuedInvocation::new(
            "review-design",
            managed.invocation.steps.clone(),
        )
        .unwrap();
        store
            .create_flow(&crate::durable::FlowInvocation {
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
                failure: None,
                finished: false,
            })
            .unwrap();
        assert_eq!(
            store.flow_position(&task_id).unwrap(),
            Some(managed.clone())
        );

        // Its Runs name the Task, filled from the invocation, and list under it.
        let new_run = |invocation: &str, task: Option<TaskId>| Run {
            id: RunId::new(),
            session_id: None,
            invocation_id: Some(invocation.to_string()),
            node: None,
            iterations: None,
            attempt: None,
            task_id: task,
            wave_id: None,
            work_source: Some(WorkSource::Declared),
            created_at: 100,
            published: true,
            cwd: "/repo".into(),
            skill: Some("review-design".into()),
            provider: None,
            model: None,
            caller_run_id: None,
            ended: None,
        };
        let run = store.create_run(new_run(&about.id, None)).unwrap();
        assert_eq!(run.task_id, Some(task_id.clone()));
        assert_eq!(run.wave_id, Some(task.wave_id.clone()));
        let other_task = TaskId::new();
        store.conn.lock().unwrap().execute(
            "INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at)
            SELECT ?1,project_id,?1,?1,'/repo.other',1 FROM tasks WHERE id=?2",
            rusqlite::params![other_task.as_str(), task_id.as_str()],
        )
        .unwrap();
        assert!(store
            .create_run(new_run(&about.id, Some(other_task.clone())))
            .is_err());
        assert_eq!(
            store
                .runs(None, None, Some(task_id.as_str()), None, 0)
                .unwrap()
                .len(),
            1
        );

        // Restart closes the Task's Flow and clears the pointer; the other Flow
        // stays current, and the pointer accepts only an invocation naming X.
        store.restart_task_flow(&task, "checkpoint").unwrap();
        assert_eq!(store.flow_position(&task_id).unwrap(), None);
        assert_eq!(
            retained_invocation(&store, &managed.invocation.id).1,
            "replaced"
        );
        let conn = store.conn.lock().unwrap();
        let state: String = conn
            .query_row(
                "SELECT state FROM flow_invocations WHERE id=?1",
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
                .flow_position(&task_id)
                .unwrap()
                .map(|position| position.invocation.id),
            Some(about.id)
        );
    }

    #[test]
    fn competing_run_assignments_start_only_the_winning_task() {
        let (dir, store, task_id) = store_with_task();
        let wave = store.task(&task_id).unwrap().unwrap().wave_id;
        let other_task = TaskId::new();
        let run = RunId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at)
                SELECT ?1,project_id,?1,?1,?1,1 FROM tasks WHERE id=?2",
                rusqlite::params![other_task.as_str(), task_id.as_str()]).unwrap();
            conn.execute(
                "INSERT INTO runs(id,created_at,cwd,published) VALUES(?1,1,'/repo',0)",
                [run.as_str()],
            )
            .unwrap();
        }
        let barrier = Arc::new(Barrier::new(2));
        let writers = [task_id, other_task].map(|target| {
            let path = dir.path().join("loopflow.db");
            let barrier = barrier.clone();
            let wave = wave.clone();
            let run = run.clone();
            thread::spawn(move || {
                let conn = rusqlite::Connection::open(path).unwrap();
                conn.busy_timeout(std::time::Duration::from_secs(5))
                    .unwrap();
                conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
                barrier.wait();
                let result = conn.execute(
                    "UPDATE runs SET task_id=?2,wave_id=?3 WHERE id=?1",
                    rusqlite::params![run.as_str(), target.as_str(), wave.as_str()],
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
            let (has_run, started): (bool, bool) = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM runs WHERE task_id=?1),started_at IS NOT NULL
                 FROM tasks WHERE id=?1",
                    [target.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(has_run, result.is_ok());
            assert_eq!(started, result.is_ok());
            if let Err(error) = result {
                assert!(error.to_string().contains("assignment cannot change"));
            }
        }
    }

    #[test]
    fn task_started_tracks_reserved_review_and_retained_history() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        assert!(!store.task_started(&task_id).unwrap());
        let mut initial = autonomous_position(&task_id);
        initial.invocation = crate::durable::test_flow_invocation(
            "review",
            0,
            "review-design",
            Some("review"),
            true,
        );
        let position = store.set_flow_position(&task_id, &initial).unwrap();
        let session_id = crate::ops::human_session::flow_id(&position).unwrap();
        let (reserved, run) = store.reserve_review_run(&position).unwrap();
        assert!(store.task_started(&task_id).unwrap());
        assert!(store.chapter_task_evidence(&task_id).unwrap().begun);
        assert!(!store.retire_chapter_backlog(&task_id).unwrap());

        // Activation may precede transfer. A reserved Task is already started
        // and must retain execution while its Project is still the predecessor.
        store
            .save_chapter(
                &Chapter {
                    id: ChapterId::parse("successor").unwrap(),
                    wave_id: task.wave_id.clone(),
                    wave: "infrastructure".into(),
                    project_id: "successor-project".into(),
                    content: crate::ops::chapter::empty_plan(),
                    predecessors: Vec::new(),
                    predecessor_metrics: Vec::new(),
                    tasks: Vec::new(),
                    phase: ChapterPhase::Transferring,
                    created_at: 1,
                    activated_at: Some(1),
                    completed_at: None,
                    error: None,
                },
                true,
            )
            .unwrap();
        let reserved = store.set_flow_position(&task_id, &reserved).unwrap();
        assert_eq!(store.session(&session_id).unwrap().unwrap().1, run);

        store
            .publish_review_run(&session_id, &run.id, reserved.version, "codex", None)
            .unwrap();
        assert!(store.task_started(&task_id).unwrap());
        store
            .ready_session(&session_id, &run.id, "approved scope")
            .unwrap();
        let position = store.flow_position(&task_id).unwrap().unwrap();
        store
            .finish_human_task_boundary(&task, &position, "approved scope")
            .unwrap();
        assert!(store.flow_position(&task_id).unwrap().is_none());
        assert!(store.task_started(&task_id).unwrap());
    }

    #[test]
    fn review_session_retains_feedback_and_history_across_replacement_and_corrupt_neighbors() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let mut initial = autonomous_position(&task_id);
        initial.invocation = crate::durable::test_flow_invocation(
            "review",
            0,
            "review-design",
            Some("review"),
            true,
        );
        let first_run = RunId::new();
        initial.session_run_id = Some(first_run.clone());
        let mut position = store.set_flow_position(&task_id, &initial).unwrap();
        let session_id = crate::ops::human_session::flow_id(&position).unwrap();
        store
            .rename_session(
                &session_id,
                None,
                "Parser review",
                crate::session::TitleSource::Human,
            )
            .unwrap();
        store
            .ready_session(&session_id, &first_run, "Keep the reviewed parser behavior")
            .unwrap();
        position = store.flow_position(&task_id).unwrap().unwrap();
        let first_attempt = store.session(&session_id).unwrap().unwrap().1;
        assert_eq!(first_attempt.work_source, Some(WorkSource::Inherited));
        let mut runs = vec![first_run];
        for _ in 0..2 {
            let stale = position.clone();
            let (reserved, run) = store.reserve_review_run(&position).unwrap();
            assert!(!run.published);
            assert_eq!(run.task_id, first_attempt.task_id);
            assert_eq!(run.wave_id, first_attempt.wave_id);
            assert_eq!(run.invocation_id, first_attempt.invocation_id);
            assert_eq!(run.work_source, Some(WorkSource::Inherited));
            assert_eq!(
                store
                    .session_runs(&session_id)
                    .unwrap()
                    .iter()
                    .find(|run| run.id == first_attempt.id),
                Some(&first_attempt)
            );
            assert!(store.reserve_review_run(&stale).is_err());
            assert!(store
                .ready_session(&session_id, runs.last().unwrap(), "late feedback")
                .is_err());
            assert!(store
                .finish_human_task_boundary(&task, &stale, "late completion")
                .is_err());
            assert!(store
                .publish_review_run(
                    &session_id,
                    runs.last().unwrap(),
                    reserved.version,
                    "codex",
                    None
                )
                .is_err());
            store
                .publish_review_run(&session_id, &run.id, reserved.version, "codex", None)
                .unwrap();
            assert!(store
                .publish_review_run(&session_id, &run.id, reserved.version, "codex", None)
                .is_err());
            runs.push(run.id);
            position = store.flow_position(&task_id).unwrap().unwrap();
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
        let (session, current) = store.session(&session_id).unwrap().unwrap();
        assert_eq!(session.title, "Parser review");
        assert_eq!(current.id, *runs.last().unwrap());
        let history = store.session_runs(&session_id).unwrap();
        let attempts = store
            .position_runs(
                &position.invocation.id,
                current.node.unwrap(),
                current.iterations.as_ref().unwrap(),
            )
            .unwrap();
        assert_eq!(
            attempts
                .iter()
                .map(|run| run.id.clone())
                .collect::<Vec<_>>(),
            runs
        );
        assert_eq!(
            attempts.iter().map(|run| run.attempt).collect::<Vec<_>>(),
            [Some(1), Some(2), Some(3)]
        );
        assert_eq!(history.len(), 3);
        for run in &runs {
            assert!(history.iter().any(|saved| &saved.id == run));
        }

        // Checkpointing execution cannot replace or erase conversation state.
        for summary in [Some("checkpoint feedback".to_string()), None] {
            let mut checkpoint = position.clone();
            checkpoint.session_run_id = Some(runs[0].clone());
            checkpoint.ready_summary = summary;
            position = store.set_flow_position(&task_id, &checkpoint).unwrap();
            assert_eq!(
                store.session(&session_id).unwrap(),
                Some((session.clone(), current.clone()))
            );
            assert_eq!(store.session_runs(&session_id).unwrap(), history);
        }

        // An unrelated malformed autonomous capture remains an identified error,
        // while direct Session discovery does not deserialize that invocation.
        let broken = TaskId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at)
                SELECT ?1,project_id,?1,?1,'/repo.broken',1 FROM tasks WHERE id=?2",
                rusqlite::params![broken.as_str(), task_id.as_str()]).unwrap();
            conn.execute("INSERT INTO flow_invocations(id,task_id,wave_id,invocation_json,step_index,iteration,
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
            store.open_sessions().unwrap(),
            vec![(session.clone(), current)]
        );
        assert!(store
            .flow_position(&broken)
            .unwrap_err()
            .to_string()
            .contains(broken.as_str()));
        let summary = position.ready_summary.as_deref().unwrap();
        store
            .finish_human_task_boundary(&task, &position, summary)
            .unwrap();
        assert!(store
            .finish_human_task_boundary(&task, &position, summary)
            .is_err());
        assert!(store
            .ready_session(&session_id, runs.last().unwrap(), "after completion")
            .is_err());
        let (completed, _) = store.session(&session_id).unwrap().unwrap();
        assert!(completed.completed_at.is_some());
        assert_eq!(completed.ready_summary, session.ready_summary);
        assert_eq!(store.session_runs(&session_id).unwrap(), history);
        assert!(store.open_sessions().unwrap().is_empty());
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
    fn restart_retains_review_and_rejects_stale_writes_at_the_same_version() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let mut initial = autonomous_position(&task_id);
        initial.invocation = crate::durable::test_flow_invocation(
            "review",
            0,
            "review-design",
            Some("review"),
            true,
        );
        initial.session_run_id = Some(RunId::new());
        initial.ready_summary = Some("keep this feedback".into());
        let original = store.set_flow_position(&task_id, &initial).unwrap();
        assert!(store
            .set_flow_position(&task_id, &autonomous_position(&task_id))
            .is_err());

        store.restart_task_flow(&task, "checkpoint").unwrap();
        assert!(store.flow_position(&task_id).unwrap().is_none());
        assert!(store.open_sessions().unwrap().is_empty());
        assert_eq!(
            retained_invocation(&store, &original.invocation.id),
            (original.clone(), "replaced".into())
        );
        assert!(store.set_flow_position(&task_id, &initial).is_err());

        let replacement = store
            .set_flow_position(&task_id, &autonomous_position(&task_id))
            .unwrap();
        assert_eq!(replacement.version, original.version);
        assert!(store.set_flow_position(&task_id, &original).is_err());
        assert!(store
            .finish_human_task_boundary(&task, &original, "late")
            .is_err());
        assert_eq!(store.flow_position(&task_id).unwrap(), Some(replacement));
        assert_eq!(
            retained_invocation(&store, &original.invocation.id),
            (original, "replaced".into())
        );
        assert!(store.chapter_task_evidence(&task_id).unwrap().begun);
        assert!(!store.retire_chapter_backlog(&task_id).unwrap());
    }

    #[test]
    fn human_completion_retains_feedback_without_leaving_a_pending_review() {
        let (_dir, store, task_id) = store_with_task();
        let task = store.task(&task_id).unwrap().unwrap();
        let mut position = autonomous_position(&task_id);
        position.invocation = crate::durable::test_flow_invocation(
            "review",
            0,
            "review-design",
            Some("review"),
            true,
        );
        position.session_run_id = Some(RunId::new());
        position.ready_summary = Some("approved scope".into());
        let position = store.set_flow_position(&task_id, &position).unwrap();
        store
            .finish_human_task_boundary(&task, &position, "approved scope")
            .unwrap();

        assert!(store.flow_position(&task_id).unwrap().is_none());
        assert!(store.open_sessions().unwrap().is_empty());
        assert_eq!(
            retained_invocation(&store, &position.invocation.id),
            (position.clone(), "completed".into())
        );
        assert!(store
            .finish_human_task_boundary(&task, &position, "duplicate")
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
            let mut position = autonomous_position(&task_id);
            position.invocation = crate::durable::test_flow_invocation(
                "review",
                1,
                "review-design",
                Some("review"),
                true,
            );
            position.cursor.index = 1;
            position.cursor.iteration = 3;
            position.session_run_id = Some(RunId::new());
            position.ready_summary = Some("retained answer".into());
            let position = store.set_flow_position(&task_id, &position).unwrap();
            let session_id = super::super::sessions::review_id(&position).unwrap();
            let original = store.session(&session_id).unwrap().unwrap();
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE flow_invocations SET review_json=?2 WHERE id=?1",
                    rusqlite::params![position.invocation.id, progress],
                )
                .unwrap();

            let recovered = store.flow_position(&task_id).unwrap().unwrap();
            let (reserved, replacement) = store.reserve_review_run(&recovered).unwrap();
            assert_eq!(reserved.cursor, recovered.cursor);
            assert_eq!(replacement.node, Some(1));
            assert_eq!(replacement.iterations, original.1.iterations);
            assert_eq!(replacement.attempt, Some(2));
            let (session, _) = store.session(&session_id).unwrap().unwrap();
            assert_eq!(session.id, original.0.id);
            assert_eq!(session.title, original.0.title);
            assert_eq!(session.ready_summary, original.0.ready_summary);
            assert_eq!(session.current_run_id, replacement.id);
            assert_eq!(store.session_runs(&session_id).unwrap().len(), 2);
            let retained: String = store
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT review_json FROM flow_invocations WHERE id=?1",
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
        let position = store.set_flow_position(&task_id, &position).unwrap();
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
                        "UPDATE flow_invocations SET review_json=?2 WHERE state='current' AND task_id=?1",
                        rusqlite::params![task_id.as_str(), json],
                    )
                    .unwrap();
                let recovered = store.flow_position(&task_id).unwrap().unwrap();
                assert_eq!(recovered.invocation, position.invocation);
                assert_eq!(recovered.cursor.index, position.cursor.index);
                assert_eq!(recovered.cursor.iteration, position.cursor.iteration);
                assert_eq!(recovered.cursor.progress.repeats["decide"], 2);
                assert_eq!(
                    recovered.cursor.progress.direction.as_deref(),
                    Some("previous direction")
                );
                assert_eq!(
                    recovered.cursor.progress.verdict.as_ref().unwrap().decision,
                    expected
                );
                assert_eq!(
                    recovered.cursor.progress.verdict.as_ref().unwrap().summary,
                    "saved proof"
                );
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
            let position = store.set_flow_position(&task_id, &position).unwrap();
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
                    "UPDATE flow_invocations SET review_json=?2 WHERE state='current' AND task_id=?1",
                    rusqlite::params![task_id.as_str(), json],
                )
                .unwrap();
            let blocked = store.flow_position(&task_id).unwrap().unwrap();
            let failure = blocked.failure.as_ref().unwrap();
            assert_eq!(failure.reason, "need a policy choice");
            assert!(!failure.restart_required);
            assert_eq!(failure.observed_at, blocked.updated_at);
            assert!(!blocked.has_pending_decision());
            assert_eq!(blocked.invocation, position.invocation);
            assert_eq!(blocked.cursor.index, position.cursor.index);
            assert_eq!(blocked.cursor.progress.repeats["decide"], 2);
            assert_eq!(store.flow_position(&task_id).unwrap().unwrap(), blocked);
            let claimed = store
                .claim_task_worker(
                    &task_id,
                    &blocked.invocation.id,
                    blocked.version,
                    &owner(303),
                    time::OffsetDateTime::now_utc(),
                )
                .unwrap();
            assert!(matches!(claimed, TaskWorkerClaimOutcome::Claimed(_)));
            let retry = store.flow_position(&task_id).unwrap().unwrap();
            assert!(retry.failure.is_none());
            assert!(!retry.has_pending_decision());
            assert_eq!(retry.cursor.index, blocked.cursor.index);
            assert_eq!(retry.invocation, blocked.invocation);
            assert_eq!(retry.cursor.progress, blocked.cursor.progress);
        }
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
        let position = store.set_flow_position(&task_id, &position).unwrap();
        let first_owner = owner(301);
        let claim = match store
            .claim_task_worker(
                &task_id,
                &position.invocation.id,
                position.version,
                &first_owner,
                time::OffsetDateTime::now_utc(),
            )
            .unwrap()
        {
            crate::durable::TaskWorkerClaimOutcome::Claimed(claim) => claim,
            other => panic!("unexpected {other:?}"),
        };
        let run = RunId::new();
        let bound = store
            .bind_task_worker_run(&task_id, &claim, &run, &first_owner)
            .unwrap();
        let verdict = crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Iterate,
            summary: "repair the missing case".into(),
        };
        let before = store.flow_position(&task_id).unwrap().unwrap();
        assert!(store
            .record_flow_verdict(&task_id, &RunId::new(), &verdict)
            .is_err());
        assert_eq!(store.flow_position(&task_id).unwrap().unwrap(), before);
        store.record_flow_verdict(&task_id, &run, &verdict).unwrap();
        store.record_flow_verdict(&task_id, &run, &verdict).unwrap();
        let different = crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Advance,
            summary: "changed mind".into(),
        };
        let before = store.flow_position(&task_id).unwrap().unwrap();
        assert!(store
            .record_flow_verdict(&task_id, &run, &different)
            .is_err());
        assert_eq!(store.flow_position(&task_id).unwrap().unwrap(), before);
        let replacement = store
            .reclaim_task_worker(
                &task_id,
                &bound,
                &owner(302),
                time::OffsetDateTime::now_utc(),
            )
            .unwrap();
        let before = store.flow_position(&task_id).unwrap().unwrap();
        assert!(store.record_flow_verdict(&task_id, &run, &verdict).is_err());
        assert_eq!(store.flow_position(&task_id).unwrap().unwrap(), before);
        let recovered = store.flow_position(&task_id).unwrap().unwrap();
        assert_eq!(recovered.cursor.progress.verdict, Some(verdict));
        assert_eq!(recovered.claim, Some(replacement.clone()));
        // A reported provider failure differs from a crash: its result cannot
        // authorize a later retry, even though the review direction survives.
        let released = store.release_task_worker(&task_id, &replacement).unwrap();
        assert!(released.cursor.progress.verdict.is_none());
    }

    #[test]
    fn review_discovery_follows_the_captured_nested_step() {
        let (_dir, store, task_id) = store_with_task();
        let mut position = autonomous_position(&task_id);
        assert!(store.open_sessions().unwrap().is_empty());
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
        position.session_run_id = Some(RunId::new());
        position.ready_summary = Some("retain the reviewed scope".into());
        let mut saved = store.set_flow_position(&task_id, &position).unwrap();
        let sessions = store.open_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].0.ready_summary, saved.ready_summary);
        assert_eq!(Some(&sessions[0].1.id), saved.session_run_id.as_ref());
        assert_eq!(saved.current().step, "review-design");
        assert_eq!(saved.current().policy.id.as_deref(), Some("review"));
        assert_eq!(store.flow_position(&task_id).unwrap(), Some(saved.clone()));

        saved.cursor.child = None;
        saved.cursor.index = 1;
        saved.session_run_id = None;
        saved.ready_summary = None;
        let saved = store.set_flow_position(&task_id, &saved).unwrap();
        assert_eq!(saved.current().step, "implement");
        assert!(store.open_sessions().unwrap().is_empty());
        assert!(store.set_flow_position(&task_id, &position).is_err());
        assert_eq!(store.flow_position(&task_id).unwrap(), Some(saved));
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
            let position = store.set_flow_position(&task_id, &position).unwrap();
            assert_eq!(position.current().iteration, 7);
            assert_eq!(
                position.current().step,
                if routing {
                    "nested-router"
                } else {
                    "loop-decide"
                }
            );
            let saved = store.set_flow_position(&task_id, &position).unwrap();
            assert_eq!(saved.cursor, position.cursor);
            let worker = owner(501);
            let TaskWorkerClaimOutcome::Claimed(claim) = store
                .claim_task_worker(
                    &task_id,
                    &saved.invocation.id,
                    saved.version,
                    &worker,
                    time::OffsetDateTime::now_utc(),
                )
                .unwrap()
            else {
                panic!("worker must be claimed")
            };
            let run = RunId::new();
            let bound = store
                .bind_task_worker_run(&task_id, &claim, &run, &worker)
                .unwrap();
            let verdict = FlowVerdict {
                decision: FlowDecision::Iterate,
                summary: "next pass".into(),
            };
            if routing {
                let before = store.flow_position(&task_id).unwrap().unwrap();
                assert!(store
                    .record_flow_route(&task_id, &RunId::new(), "chosen")
                    .is_err());
                assert!(store.record_flow_route(&task_id, &run, "missing").is_err());
                assert_eq!(store.flow_position(&task_id).unwrap().unwrap(), before);
                store.record_flow_route(&task_id, &run, "chosen").unwrap();
                store.record_flow_route(&task_id, &run, "chosen").unwrap();
                let before = store.flow_position(&task_id).unwrap().unwrap();
                assert!(store.record_flow_route(&task_id, &run, "other").is_err());
                assert_eq!(store.flow_position(&task_id).unwrap().unwrap(), before);
            } else {
                store.record_flow_verdict(&task_id, &run, &verdict).unwrap();
                assert!(store.record_flow_route(&task_id, &run, "chosen").is_err());
            }
            let pending = store.flow_position(&task_id).unwrap().unwrap();
            assert!(pending.has_pending_decision());
            assert_eq!(pending.cursor.progress, saved.cursor.progress);
            let replacement = store
                .reclaim_task_worker(
                    &task_id,
                    &bound,
                    &owner(502),
                    time::OffsetDateTime::now_utc(),
                )
                .unwrap();
            let recovered = store.flow_position(&task_id).unwrap().unwrap();
            assert_eq!(recovered.cursor, pending.cursor);
            assert!(store.record_flow_route(&task_id, &run, "chosen").is_err());
            assert!(store.record_flow_verdict(&task_id, &run, &verdict).is_err());
            assert_eq!(store.flow_position(&task_id).unwrap().unwrap(), recovered);
            let cleared = if routing {
                store
                    .block_task_flow(
                        &task_id,
                        &replacement,
                        &TaskFlowBlocker {
                            run_id: None,
                            reason: "router failed after publishing".into(),
                            restart_required: false,
                            observed_at: time::OffsetDateTime::now_utc(),
                        },
                    )
                    .unwrap()
            } else {
                store.release_task_worker(&task_id, &replacement).unwrap()
            };
            assert_eq!(cleared.cursor, saved.cursor);
            assert!(!cleared.has_pending_decision());
        }
    }

    #[test]
    fn human_session_runtime_survives_a_store_round_trip() {
        let (_dir, store, work) = store_with_task();
        let run_id = RunId::new();
        let position = FlowPosition {
            task_id: work.clone(),
            invocation: crate::durable::test_flow_invocation(
                "review",
                1,
                "review-design",
                Some("human_review"),
                true,
            ),
            session_run_id: Some(run_id.clone()),
            ready_summary: Some("Ready for review".to_string()),
            cursor: crate::engine::ExecutionCursor {
                index: 1,
                iteration: 2,
                ..Default::default()
            },
            version: 0,
            worker_generation: 0,
            claim: None,
            failure: None,
            updated_at: time::OffsetDateTime::now_utc(),
        };

        store.set_flow_position(&work, &position).unwrap();

        let stored = store.flow_position(&work).unwrap().unwrap();
        assert_eq!(stored.session_run_id, Some(run_id));
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
        let position = store.set_flow_position(&task_id, &initial).unwrap();
        let node = position.invocation.node_id(&position.cursor).unwrap();
        let tuple = crate::engine::flow_graph::flow_iterations(
            &position.invocation.steps,
            &position.cursor,
        );
        let claim = |position: &FlowPosition, pid| {
            let TaskWorkerClaimOutcome::Claimed(claim) = store
                .claim_task_worker(
                    &task_id,
                    &position.invocation.id,
                    position.version,
                    &owner(pid),
                    time::OffsetDateTime::now_utc(),
                )
                .unwrap()
            else {
                panic!("expected a claim")
            };
            claim
        };
        let first = RunId::new();
        let bound = store
            .bind_task_worker_run(&task_id, &claim(&position, 501), &first, &owner(501))
            .unwrap();
        let verdict = FlowVerdict {
            decision: FlowDecision::Advance,
            summary: "candidate before failure".into(),
        };
        store
            .record_flow_verdict(&task_id, &first, &verdict)
            .unwrap();
        store.release_task_worker(&task_id, &bound).unwrap();
        let failed = store.flow_position(&task_id).unwrap().unwrap();
        assert_eq!(failed.cursor.index, position.cursor.index);
        assert_eq!(failed.cursor.iteration, position.cursor.iteration);
        assert!(!failed.has_pending_decision());
        let second = RunId::new();
        let replacement = store
            .bind_task_worker_run(&task_id, &claim(&failed, 502), &second, &owner(502))
            .unwrap();
        let before = store.flow_position(&task_id).unwrap().unwrap();
        assert!(store
            .record_flow_verdict(&task_id, &first, &verdict)
            .is_err());
        assert!(store.finish_task_flow(&task, &bound, None).is_err());
        assert_eq!(store.flow_position(&task_id).unwrap().unwrap(), before);
        let attempts = store
            .position_runs(&position.invocation.id, node, &tuple)
            .unwrap();
        assert_eq!(
            attempts.iter().map(|run| &run.id).collect::<Vec<_>>(),
            [&first, &second]
        );
        assert_eq!(
            attempts.iter().map(|run| run.attempt).collect::<Vec<_>>(),
            [Some(1), Some(2)]
        );
        assert!(attempts.iter().all(|run| run.session_id.is_none()));
        store
            .record_flow_verdict(&task_id, &second, &verdict)
            .unwrap();
        store
            .finish_task_flow(&task, &replacement, Some("successful attempt"))
            .unwrap();
        assert!(store.flow_position(&task_id).unwrap().is_none());
        assert_eq!(
            store
                .position_runs(&position.invocation.id, node, &tuple)
                .unwrap(),
            attempts
        );
        assert!(store.finish_task_flow(&task, &replacement, None).is_err());

        // The launch names the provider the claim could not know, and the
        // Run's end is its row. Both attempts list once under their Task.
        let end = crate::session::RunEnd {
            outcome: "completed".into(),
            at: 9,
        };
        store.fill_run_provider(&second, "codex", None).unwrap();
        store.end_run(&second, &end).unwrap();
        let listed = store
            .runs(None, None, Some(task.plan.identifier.as_str()), None, 0)
            .unwrap();
        assert_eq!(listed.len(), 2);
        for listed in &listed {
            assert_eq!(listed.run.task_id, Some(task_id.clone()));
            assert_eq!(listed.run.wave_id, Some(task.wave_id.clone()));
            assert_eq!(
                listed.run.invocation_id.as_ref(),
                Some(&position.invocation.id)
            );
            assert_eq!(listed.task.as_deref(), Some(task.plan.identifier.as_str()));
        }
        let settled = listed
            .iter()
            .find(|listed| listed.run.id == second)
            .unwrap();
        assert_eq!(settled.run.provider.as_deref(), Some("codex"));
        assert_eq!(settled.run.ended, Some(end));
    }

    #[test]
    fn concurrent_task_worker_claims_choose_one_worker() {
        let (dir, store, work) = store_with_task();
        let position = store
            .set_flow_position(&work, &autonomous_position(&work))
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
            .set_flow_position(&task_id, &autonomous_position(&task_id))
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
        let recovered = store.flow_position(&task_id).unwrap().unwrap();
        assert_eq!(recovered.invocation, position.invocation);
        assert_eq!(recovered.cursor, position.cursor);
        assert_eq!(replacement.generation, claim.generation + 1);
        assert_eq!(recovered.claim, Some(replacement));
    }

    #[test]
    fn only_the_bound_worker_can_settle_a_position() {
        let (_dir, store, work) = store_with_task();
        let task = store.task(&work).unwrap().unwrap();
        let mut initial = autonomous_position(&work);
        initial.invocation = crate::durable::test_flow_invocation("task", 1, "review", None, false);
        let position = store.set_flow_position(&work, &initial).unwrap();
        let claim = match store
            .claim_task_worker(
                &work,
                &position.invocation.id,
                position.version,
                &owner(101),
                time::OffsetDateTime::now_utc(),
            )
            .unwrap()
        {
            TaskWorkerClaimOutcome::Claimed(claim) => claim,
            outcome => panic!("unexpected claim outcome: {outcome:?}"),
        };
        let worker_run_id = RunId::new();
        let bound = store
            .bind_task_worker_run(&work, &claim, &worker_run_id, &owner(102))
            .unwrap();
        let mut next = store.flow_position(&work).unwrap().unwrap();
        next.cursor.index = 1;
        next.claim = None;
        next.updated_at = time::OffsetDateTime::now_utc();

        assert!(store
            .settle_task_worker(&task, &claim, &next, Some("advanced"))
            .is_err());
        let settled = store
            .settle_task_worker(&task, &bound, &next, Some("advanced"))
            .unwrap();
        assert_eq!(settled.version, position.version + 1);
        assert_eq!(settled.current().step, "review");
        assert_eq!(settled.invocation, position.invocation);
        assert_eq!(settled.worker_generation, 0);
        assert!(settled.claim.is_none());
        assert!(store
            .settle_task_worker(&task, &bound, &next, Some("advanced"))
            .is_err());
        let events = store.task_events_after(&work, 0).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, TaskEventKind::Started);
        assert!(matches!(
            &events[1].kind,
            TaskEventKind::Progress { summary } if summary == "advanced"
        ));
    }

    #[test]
    fn only_the_bound_worker_can_finish_a_position_without_a_successor() {
        let (_dir, store, work) = store_with_task();
        let task = store.task(&work).unwrap().unwrap();
        let position = store
            .set_flow_position(&work, &autonomous_position(&work))
            .unwrap();
        let claim = match store
            .claim_task_worker(
                &work,
                &position.invocation.id,
                position.version,
                &owner(111),
                time::OffsetDateTime::now_utc(),
            )
            .unwrap()
        {
            TaskWorkerClaimOutcome::Claimed(claim) => claim,
            outcome => panic!("unexpected claim outcome: {outcome:?}"),
        };
        let bound = store
            .bind_task_worker_run(&work, &claim, &RunId::new(), &owner(112))
            .unwrap();
        let final_position = store.flow_position(&work).unwrap().unwrap();

        assert!(store
            .finish_task_flow(&task, &claim, Some("finished"))
            .is_err());
        store
            .finish_task_flow(&task, &bound, Some("finished"))
            .unwrap();
        assert!(store.flow_position(&work).unwrap().is_none());
        assert_eq!(
            retained_invocation(&store, &position.invocation.id),
            (final_position.clone(), "completed".into())
        );
        let evidence = store.chapter_task_evidence(&work).unwrap();
        assert!(evidence.begun);
        assert!(!evidence.worker_claimed);
        assert!(store
            .finish_task_flow(&task, &bound, Some("finished"))
            .is_err());
        let events = store.task_events_after(&work, 0).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, TaskEventKind::Started);
        assert!(matches!(
            &events[1].kind,
            TaskEventKind::FlowFinished { summary, .. } if summary == "finished"
        ));
        let next = store
            .set_flow_position(&work, &autonomous_position(&work))
            .unwrap();
        assert_eq!(next.version, position.version);
        assert!(store
            .finish_task_flow(&task, &bound, Some("late finish"))
            .is_err());
        assert_eq!(store.flow_position(&work).unwrap(), Some(next));
        assert_eq!(
            retained_invocation(&store, &position.invocation.id),
            (final_position, "completed".into())
        );
    }

    #[test]
    fn reclaim_fences_the_old_worker_and_increments_generation() {
        let (_dir, store, work) = store_with_task();
        let position = store
            .set_flow_position(&work, &autonomous_position(&work))
            .unwrap();
        let first = match store
            .claim_task_worker(
                &work,
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
            .reclaim_task_worker(&work, &first, &owner(202), time::OffsetDateTime::now_utc())
            .unwrap();

        assert_eq!(replacement.generation, 2);
        assert!(store
            .bind_task_worker_run(&work, &first, &RunId::new(), &owner(203))
            .is_err());

        let bound = store
            .bind_task_worker_run(&work, &replacement, &RunId::new(), &owner(204))
            .unwrap();
        let failure = TaskFlowBlocker {
            run_id: None,
            reason: "provider exited".to_string(),
            restart_required: false,
            observed_at: time::OffsetDateTime::now_utc(),
        };
        let failed = store.block_task_flow(&work, &bound, &failure).unwrap();
        assert!(failed.claim.is_none());
        assert_eq!(
            failed.failure.as_ref(),
            Some(&TaskFlowBlocker {
                run_id: bound.worker_run_id.clone(),
                ..failure.clone()
            })
        );
        assert!(store.block_task_flow(&work, &first, &failure).is_err());
        let events = store.task_events_after(&work, 0).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, TaskEventKind::Started);
        assert!(matches!(
            &events[1].kind,
            TaskEventKind::Failed { error, resumable: true } if error == &failure.reason
        ));

        let retried = match store
            .claim_task_worker(
                &work,
                &position.invocation.id,
                position.version,
                &owner(205),
                time::OffsetDateTime::now_utc(),
            )
            .unwrap()
        {
            TaskWorkerClaimOutcome::Claimed(claim) => claim,
            outcome => panic!("unexpected claim outcome: {outcome:?}"),
        };
        assert_eq!(retried.generation, 3);
        assert!(store
            .flow_position(&work)
            .unwrap()
            .unwrap()
            .failure
            .is_none());
    }
}
