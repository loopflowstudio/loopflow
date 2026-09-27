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
        if verdict.summary.trim().is_empty() {
            return Err(StoreError::InvalidData(
                "review evidence cannot be empty".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_ready_work(&tx, &WorkRef::Task(task_id.clone()))?;
        let mut position = flow_position_in(&tx, task_id)?.ok_or(StoreError::NotFound)?;
        if position
            .claim
            .as_ref()
            .and_then(|claim| claim.worker_run_id.as_ref())
            != Some(run_id)
            || position.current().policy.repeat.is_none()
        {
            return Err(StoreError::InvalidAuthority(
                "only the current claimed loop reviewer may record a verdict".to_string(),
            ));
        }
        let progress = &mut position.cursor.leaf_mut().progress;
        if progress
            .verdict
            .as_ref()
            .is_some_and(|saved| saved != verdict)
        {
            return Err(StoreError::InvalidAuthority(
                "step already has a different decision".into(),
            ));
        }
        progress.verdict = Some(verdict.clone());
        tx.execute(
            "UPDATE task_flow_positions SET review_json=?2 WHERE task_id=?1",
            params![task_id.as_str(), serde_json::to_string(&position.cursor)?],
        )?;
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
        require_ready_work(&tx, &WorkRef::Task(task_id.clone()))?;
        let mut position = flow_position_in(&tx, task_id)?.ok_or(StoreError::NotFound)?;
        if position
            .claim
            .as_ref()
            .and_then(|claim| claim.worker_run_id.as_ref())
            != Some(run_id)
        {
            return Err(StoreError::InvalidAuthority(
                "only the current claimed router may select a path".into(),
            ));
        }
        let crate::engine::ConcreteStep::Xor(branch) = position.current_plan() else {
            return Err(StoreError::InvalidAuthority(
                "current Flow step is not a router".into(),
            ));
        };
        if !branch.paths.contains_key(path) {
            return Err(StoreError::InvalidData(format!(
                "unknown Flow route {path:?}"
            )));
        }
        let leaf = position.cursor.leaf_mut();
        if leaf.route.as_deref().is_some_and(|saved| saved != path) {
            return Err(StoreError::InvalidAuthority(
                "step already has a different route".into(),
            ));
        }
        leaf.route = Some(path.to_string());
        tx.execute(
            "UPDATE task_flow_positions SET review_json=?2 WHERE task_id=?1",
            params![task_id.as_str(), serde_json::to_string(&position.cursor)?],
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

    pub fn set_work_enabled(&self, work: &WorkRef, enabled: bool) -> StoreResult<Placement> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        placement_in(&tx, work)?;
        let enabled = if enabled { 1_i64 } else { 0_i64 };
        match work {
            WorkRef::Wave(id) => tx.execute(
                "UPDATE work_placements SET enabled=?2 WHERE wave_id=?1",
                params![id.as_str(), enabled],
            )?,
            WorkRef::Project(id) => tx.execute(
                "UPDATE work_placements SET enabled=?2 WHERE project_id=?1",
                params![id.as_str(), enabled],
            )?,
            WorkRef::Task(id) => tx.execute(
                "UPDATE work_placements SET enabled=?2 WHERE task_id=?1",
                params![id.as_str(), enabled],
            )?,
        };
        let placement = placement_in(&tx, work)?;
        tx.commit()?;
        Ok(placement)
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
            "UPDATE task_flow_positions
             SET worker_generation=?2, claim_json=?3, failure_json=NULL, updated_at=?4, review_json=?6
             WHERE task_id=?1 AND position_version=?5 AND claim_json IS NULL",
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
            "UPDATE task_flow_positions
             SET worker_generation=?2, claim_json=?3, failure_json=NULL, updated_at=?4, review_json=?7
             WHERE task_id=?1 AND position_version=?5 AND claim_json=?6",
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
            "UPDATE task_flow_positions SET claim_json=?2
             WHERE task_id=?1 AND position_version=?3 AND claim_json=?4",
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
        tx.commit()?;
        Ok(bound)
    }

    pub fn human_task_flow_positions(&self) -> StoreResult<Vec<FlowPosition>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT task_id, invocation_json, flow, step, node_id, human,
                    session_run_id, ready_summary, step_index, iteration,
                    position_version, worker_generation, claim_json, failure_json,
                    updated_at, review_json
             FROM task_flow_positions WHERE human=1 ORDER BY updated_at, task_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, bool>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, i64>(14)?,
                row.get::<_, Option<String>>(15)?,
            ))
        })?;
        let mut positions = Vec::new();
        for row in rows {
            let (
                task_id,
                invocation_json,
                flow,
                step,
                node_id,
                human,
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
            ) = row?;
            positions.push(decode_flow_position(
                TaskId::parse(&task_id).map_err(invalid_durable)?,
                (
                    invocation_json,
                    flow,
                    step,
                    node_id,
                    human,
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
                ),
            )?);
        }
        Ok(positions)
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
        let task = tx.query_row(
            super::children::TASK_SELECT,
            params![task_id.as_str()],
            super::children::map_task_row,
        )?;
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
                let task = tx.query_row(
                    super::children::TASK_SELECT,
                    params![task_id.as_str()],
                    super::children::map_task_row,
                )?;
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
            "SELECT home_id, enabled, placed_at FROM work_placements WHERE wave_id=?1",
            [id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        ),
        WorkRef::Project(id) => conn.query_row(
            "SELECT home_id, enabled, placed_at FROM work_placements WHERE project_id=?1",
            [id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        ),
        WorkRef::Task(id) => conn.query_row(
            "SELECT home_id, enabled, placed_at FROM work_placements WHERE task_id=?1",
            [id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        ),
    }
    .optional()?;
    row.map(|(home_id, enabled, placed_at)| {
        Ok(Placement {
            work: work.clone(),
            home_id: HomeId::parse(&home_id).map_err(invalid_durable)?,
            enabled,
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
    require_current_task_chapter(conn, work)?;
    if !placement_in(conn, work)?.enabled {
        return Err(StoreError::InvalidAuthority(format!(
            "{} {} is paused",
            work.kind(),
            work.id()
        )));
    }
    Ok(())
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
         AND NOT EXISTS(SELECT 1 FROM task_flow_positions WHERE task_id=t.id AND worker_generation>0))",
        [task.as_str()], |row| row.get(0),
    )?;
    if expired {
        return Err(StoreError::InvalidAuthority("this Task belongs to chapter history; resume the chapter transition before starting work".into()));
    }
    Ok(())
}

pub(crate) fn reopen_work_in(conn: &Connection, work: &WorkRef) -> StoreResult<()> {
    if let WorkRef::Task(task_id) = work {
        conn.execute(
            "DELETE FROM task_flow_positions WHERE task_id=?1",
            [task_id.as_str()],
        )?;
    }
    let (table, id) = work_table(work);
    if conn.execute(
        &format!(
            "UPDATE {table} SET work_state='ready', work_terminal_at=NULL
             WHERE id=?1 AND work_state IN ('done', 'abandoned')"
        ),
        [id],
    )? != 1
    {
        return Err(StoreError::InvalidAuthority(format!(
            "{} {} is not terminal",
            work.kind(),
            work.id()
        )));
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

type StoredFlowPosition = (
    String,
    String,
    String,
    Option<String>,
    bool,
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

pub(super) fn flow_position_in(
    conn: &Connection,
    task_id: &TaskId,
) -> StoreResult<Option<FlowPosition>> {
    let row = conn
        .query_row(
            "SELECT invocation_json, flow, step, node_id, human, session_run_id, ready_summary,
                    step_index, iteration, position_version, worker_generation,
                    claim_json, failure_json, updated_at, review_json
             FROM task_flow_positions WHERE task_id=?1",
            [task_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, bool>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, Option<String>>(12)?,
                    row.get::<_, i64>(13)?,
                    row.get::<_, Option<String>>(14)?,
                ))
            },
        )
        .optional()?;
    row.map(|row| decode_flow_position(task_id.clone(), row))
        .transpose()
}

pub(super) fn set_flow_position_in(
    conn: &Connection,
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
    let step = position.current();
    let changed = if position.version == 0 {
        conn.execute(
            "INSERT INTO task_flow_positions (
                task_id, invocation_json, flow, step, node_id, human, session_run_id,
                ready_summary, step_index, iteration, position_version,
                worker_generation, claim_json, failure_json, updated_at, review_json
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, 0, NULL, ?11, ?12, ?13
             )
             ON CONFLICT(task_id) DO NOTHING",
            params![
                task_id.as_str(),
                serde_json::to_string(&position.invocation)?,
                step.flow,
                step.step,
                step.policy.id,
                step.policy.human,
                position.session_run_id.as_ref().map(RunId::as_str),
                position.ready_summary,
                i64::try_from(position.cursor.index).map_err(invalid_durable)?,
                i64::from(position.cursor.iteration),
                failure_json,
                position.updated_at.unix_timestamp(),
                serde_json::to_string(&position.cursor)?
            ],
        )?
    } else {
        conn.execute(
            "UPDATE task_flow_positions
             SET invocation_json=?2, flow=?3, step=?4, node_id=?5, human=?6, session_run_id=?7,
                 ready_summary=?8, step_index=?9, iteration=?10,
                 position_version=position_version + 1,
                 worker_generation=0, claim_json=NULL, failure_json=?11,
                 updated_at=?12, review_json=?14
             WHERE task_id=?1 AND position_version=?13 AND claim_json IS NULL",
            params![
                task_id.as_str(),
                serde_json::to_string(&position.invocation)?,
                step.flow,
                step.step,
                step.policy.id,
                step.policy.human,
                position.session_run_id.as_ref().map(RunId::as_str),
                position.ready_summary,
                i64::try_from(position.cursor.index).map_err(invalid_durable)?,
                i64::from(position.cursor.iteration),
                failure_json,
                position.updated_at.unix_timestamp(),
                i64::try_from(position.version).map_err(invalid_durable)?,
                serde_json::to_string(&position.cursor)?
            ],
        )?
    };
    if changed != 1 {
        return Err(StoreError::InvalidAuthority(format!(
            "Flow position for Task {task_id} changed or is actively claimed"
        )));
    }
    flow_position_in(conn, task_id)?.ok_or(StoreError::NotFound)
}

pub(super) fn block_task_flow_in(
    conn: &Connection,
    task_id: &TaskId,
    expected: &TaskWorkerClaim,
    failure: &TaskFlowBlocker,
) -> StoreResult<FlowPosition> {
    require_ready_work(conn, &WorkRef::Task(task_id.clone()))?;
    let expected_json = serde_json::to_string(expected)?;
    let failure_json = serde_json::to_string(failure)?;
    let mut position = flow_position_in(conn, task_id)?.ok_or(StoreError::NotFound)?;
    let leaf = position.cursor.leaf_mut();
    leaf.progress.verdict = None;
    leaf.route = None;
    if conn.execute(
        "UPDATE task_flow_positions
         SET claim_json=NULL, failure_json=?2, updated_at=?3, review_json=?6
         WHERE task_id=?1 AND position_version=?4 AND claim_json=?5",
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
    let expected_json = serde_json::to_string(expected)?;
    let mut position = flow_position_in(conn, task_id)?.ok_or(StoreError::NotFound)?;
    let leaf = position.cursor.leaf_mut();
    leaf.progress.verdict = None;
    leaf.route = None;
    if conn.execute(
        "UPDATE task_flow_positions
         SET claim_json=NULL, failure_json=NULL, updated_at=?2,
             review_json=?5
         WHERE task_id=?1 AND position_version=?3 AND claim_json=?4",
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
    conn: &Connection,
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
    let expected_json = serde_json::to_string(expected)?;
    let step = next.current();
    if conn.execute(
        "UPDATE task_flow_positions
         SET invocation_json=?2, flow=?3, step=?4, node_id=?5, human=?6, session_run_id=?7,
             ready_summary=?8, step_index=?9, iteration=?10,
             position_version=position_version + 1,
             worker_generation=0, claim_json=NULL, failure_json=NULL,
             updated_at=?11, review_json=?14
         WHERE task_id=?1 AND position_version=?12 AND claim_json=?13",
        params![
            task_id.as_str(),
            serde_json::to_string(&next.invocation)?,
            step.flow,
            step.step,
            step.policy.id,
            step.policy.human,
            next.session_run_id.as_ref().map(RunId::as_str),
            next.ready_summary,
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
    let expected_json = serde_json::to_string(expected)?;
    if conn.execute(
        "DELETE FROM task_flow_positions
         WHERE task_id=?1 AND position_version=?2 AND claim_json=?3",
        params![
            task_id.as_str(),
            i64::try_from(expected.position_version).map_err(invalid_durable)?,
            expected_json
        ],
    )? != 1
    {
        return Err(stale_task_worker(task_id));
    }
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
    task_id: TaskId,
    (
        invocation_json,
        flow,
        step,
        node_id,
        human,
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
    let root_index = usize::try_from(step_index).map_err(invalid_durable)?;
    let root_iteration = u32::try_from(iteration).map_err(invalid_durable)?;
    let saved = review_json
        .as_deref()
        .map(serde_json::from_str::<serde_json::Value>)
        .transpose()?;
    let cursor = match saved {
        Some(value) if value.get("index").is_some() => serde_json::from_value(value)?,
        _ => crate::engine::ExecutionCursor {
            index: root_index,
            iteration: root_iteration,
            progress: decode_flow_progress(review_json.as_deref(), &mut failure, updated_at)?,
            ..Default::default()
        },
    };
    if cursor.index != root_index || cursor.iteration != root_iteration {
        return Err(StoreError::InvalidData(
            "stored Flow cursor does not match its root projection".into(),
        ));
    }
    let position = FlowPosition {
        cursor,
        task_id,
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
    let current = position
        .current_checked()
        .ok_or_else(|| StoreError::InvalidData("Flow position has no current step".to_string()))?;
    if current.flow != flow
        || current.step != step
        || current.policy.id != node_id
        || current.policy.human != human
    {
        return Err(StoreError::InvalidData(
            "stored Flow position projection does not match its invocation".to_string(),
        ));
    }
    Ok(position)
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
    use crate::store::sqlite::SqliteStore;
    use crate::work::project::Project;
    use crate::work::task::{PmWritebackState, Task, TaskEventKind, TaskPr, TaskPrId};

    fn store_with_task() -> (tempfile::TempDir, SqliteStore, TaskId) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loopflow.db");
        let store = SqliteStore::new(&path).expect("open a fresh store");
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

    #[test]
    fn legacy_flow_decisions_preserve_pinned_progress() {
        let (_dir, store, task_id) = store_with_task();
        let mut position = autonomous_position(&task_id);
        position.invocation = crate::engine::invocation::QueuedInvocation::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
            "pursue",
        )
        .unwrap();
        position.cursor.index = 4;
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
                        "UPDATE task_flow_positions SET review_json=?2 WHERE task_id=?1",
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
            position.cursor.index = 4;
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
                    "UPDATE task_flow_positions SET review_json=?2 WHERE task_id=?1",
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
        position.cursor.index = 6;
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
                index: if routing { 0 } else { 4 },
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
    fn concurrent_task_worker_claims_choose_one_worker() {
        let (dir, store, work) = store_with_task();
        let position = store
            .set_flow_position(&work, &autonomous_position(&work))
            .unwrap();
        let barrier = Arc::new(Barrier::new(20));
        let stores = (0..20)
            .map(|_| SqliteStore::new(&dir.path().join("loopflow.db")).unwrap())
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

        assert!(store
            .finish_task_flow(&task, &claim, Some("finished"))
            .is_err());
        store
            .finish_task_flow(&task, &bound, Some("finished"))
            .unwrap();
        assert!(store.flow_position(&work).unwrap().is_none());
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
            reason: "provider exited".to_string(),
            restart_required: false,
            observed_at: time::OffsetDateTime::now_utc(),
        };
        let failed = store.block_task_flow(&work, &bound, &failure).unwrap();
        assert!(failed.claim.is_none());
        assert_eq!(failed.failure.as_ref(), Some(&failure));
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
