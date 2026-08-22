//! SQLite persistence for Project and Tasks.

use std::path::PathBuf;

// Product writes read before they write (for example, validating parent Work),
// so a deferred transaction has to upgrade its read lock to a write
// lock. Under WAL, SQLite fails that upgrade immediately rather than waiting —
// `busy_timeout` is never consulted, because waiting on an upgrade can deadlock
// two upgraders. Beginning IMMEDIATE takes the write lock up front, where
// `busy_timeout` does apply, so a second `lf` process queues instead of dying
// with `database is locked`.
use rusqlite::{params, Connection, OptionalExtension, ToSql, TransactionBehavior};
use time::OffsetDateTime;

use crate::child::{
    AbandonIntent, ChildBodyHandoff, ChildBodyHandoffRequest, ChildRef, ObservationRecipient,
};
use crate::durable::{Author, Basis, Run, RunContext, RunTrigger};
use crate::id::WaveId;
use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
use crate::project::{
    ChildEventPayload, ObservationOutboxRow, Project, ProjectEvent, ProjectEventKind, ProjectId,
};
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::task::{
    AfterMerge, CiObservation, GithubObservation, GithubPr, LinearObservationApply,
    LinearObservationOutcome, PrMergeRequest, PrPhase, PrPresentation, PrPublication, Task,
    TaskEvent, TaskEventKind, TaskId, TaskLifecyclePhase, TaskLifecyclePlan, TaskLinearObservation,
    TaskPhasePlan, TaskPr, TaskPrId, TaskPrRepairKind,
};

use super::durable::{
    create_project_spine, create_task_spine, current_epoch_in, end_run_for_context, validate_basis,
    validate_completion_readiness_in, validate_run_context, validate_stop_context,
    work_for_child_in, work_status_in,
};
use super::SqliteStore;

impl SqliteStore {
    // Durable Tasks: Linear identity, immutable placement, commands,
    // and lifecycle events share one sqlite transaction boundary.

    pub fn insert_task(&self, task: &Task, pr: &TaskPr) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        insert_initial_task(&transaction, task, pr)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn insert_task_run(
        &self,
        task: &Task,
        pr: &TaskPr,
        caller: Option<&RunContext>,
        text: &str,
    ) -> StoreResult<(Run, RunContext)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        insert_initial_task(&transaction, task, pr)?;
        let work = work_for_child_in(&transaction, &ChildRef::Task(task.id.clone()))?;
        super::durable::validate_actor(&transaction, caller)?;
        let author = caller.map_or(Author::User, |actor| Author::Run(actor.run_id.clone()));
        let steer = Self::append_steer_in(&transaction, &work, &author, text)?;
        insert_task_event_in(
            &transaction,
            task,
            &TaskEventKind::WorktreeInitializing {
                pr_id: pr.id.clone(),
                sequence: pr.sequence,
                branch: pr.branch.clone(),
                path: task.worktree.display().to_string(),
                base_commit: pr.base_commit.clone(),
            },
        )?;
        let reservation = super::durable::reserve_run_in(
            &transaction,
            &work,
            &RunTrigger::Input {
                basis: steer.steer.basis,
            },
        )?;
        transaction.commit()?;
        Ok(reservation)
    }

    /// Reopen the stable Task as a new Epoch. Product identity, PR history,
    /// Linear cursors, and authored direction stay attached to the same Task.
    pub fn reopen_task(
        &self,
        task: &Task,
        pr: Option<&TaskPr>,
        caller: Option<&RunContext>,
        text: &str,
    ) -> StoreResult<()> {
        validate_task(task)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::durable::validate_actor(&transaction, caller)?;
        let author = caller.map_or(Author::User, |actor| Author::Run(actor.run_id.clone()));
        let previous: String = transaction
            .query_row(
                "SELECT state FROM epochs WHERE task_id=?1 ORDER BY number DESC LIMIT 1",
                [task.id.as_str()],
                |row| row.get(0),
            )
            .map_err(|error| {
                StoreError::InvalidData(format!("read Task before reopen: {error}"))
            })?;
        if !matches!(previous.as_str(), "completed" | "abandoned") {
            return Err(StoreError::InvalidData(format!(
                "Task {} is {previous}; only terminal Work can open a new Epoch",
                task.id
            )));
        }
        validate_task_project(&transaction, task).map_err(|error| {
            StoreError::InvalidData(format!("validate reopened Task owner: {error}"))
        })?;
        let parameters = task_params(task);
        transaction
            .execute(
                TASK_LIFECYCLE_UPDATE,
                rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
            )
            .map_err(|error| StoreError::InvalidData(format!("update reopened Task: {error}")))?;
        create_task_spine(&transaction, task)
            .map_err(|error| StoreError::InvalidData(format!("open Task Epoch: {error}")))?;
        if let Some(pr) = pr {
            validate_task_pr(pr)?;
            if pr.task_id != task.id || pr.phase() != PrPhase::Working {
                return Err(StoreError::InvalidData(
                    "a reopened Task requires its own Working PR".to_string(),
                ));
            }
            insert_task_pr(&transaction, pr)?;
        }
        let work =
            work_for_child_in(&transaction, &ChildRef::Task(task.id.clone())).map_err(|error| {
                StoreError::InvalidData(format!("resolve reopened Task Work: {error}"))
            })?;
        Self::append_steer_in(&transaction, &work, &author, text)
            .map_err(|error| StoreError::InvalidData(format!("steer reopened Task: {error}")))?;
        transaction.commit()?;
        Ok(())
    }

    pub fn update_task(&self, task: &Task) -> StoreResult<()> {
        validate_task(task)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_task_project(&transaction, task)?;
        let parameters = task_control_params(task);
        let changed = transaction.execute(
            TASK_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn validate_task_run_route(
        &self,
        task: &Task,
        lease: &RunContext,
        current_external_project_id: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        require_run_owns_child(&conn, &ChildRef::Task(task.id.clone()), lease)?;
        let durable_external_project_id: String = conn.query_row(
            "SELECT p.external_project_id
             FROM tasks t JOIN projects p ON p.id=t.project_id
             WHERE t.id=?1",
            [task.id.as_str()],
            |row| row.get(0),
        )?;
        if durable_external_project_id != current_external_project_id {
            return Err(StoreError::InvalidAuthority(format!(
                "Task {} durable history belongs to Linear Project {}, but current PM routing names {}; refusing automated Task authority",
                task.plan.identifier, durable_external_project_id, current_external_project_id
            )));
        }
        Ok(())
    }

    pub fn rebind_task_issue_identifier(
        &self,
        issue_id: &str,
        old_identifier: &str,
        new_identifier: &str,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some((task_id, current_identifier)) = tx
            .query_row(
                "SELECT id, issue_identifier FROM tasks WHERE external_issue_id=?1",
                [issue_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        else {
            return Ok(false);
        };
        if current_identifier == new_identifier {
            return Ok(false);
        }
        if current_identifier != old_identifier {
            return Err(StoreError::InvalidData(format!(
                "Task {task_id} identifies issue {issue_id} as {current_identifier}, not {old_identifier}"
            )));
        }
        let active_run: Option<String> = tx
            .query_row(
                "SELECT r.id
                 FROM epochs e JOIN runs r ON r.epoch_id=e.id
                 WHERE e.task_id=?1 AND e.state='open' AND r.state != 'ended'
                 ORDER BY r.created_at DESC LIMIT 1",
                [&task_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(run_id) = active_run {
            return Err(StoreError::InvalidData(format!(
                "Task {task_id} has active Run {run_id}; stop it before changing {old_identifier} to {new_identifier}"
            )));
        }
        let changed = tx.execute(
            "UPDATE tasks SET issue_identifier=?3 WHERE id=?1 AND issue_identifier=?2",
            params![task_id, old_identifier, new_identifier],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidData(format!(
                "Task {task_id} changed during its team migration"
            )));
        }
        tx.execute(
            "UPDATE tasks SET updated_at=?2 WHERE id=?1",
            params![task_id, now_unix()],
        )?;
        tx.commit()?;
        Ok(true)
    }

    pub(crate) fn update_task_for_run(&self, task: &Task, lease: &RunContext) -> StoreResult<()> {
        validate_task(task)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        if update_task_for_run_in(&conn, task, lease)? == 0 {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot update Task {}",
                lease.run_id, task.id
            )));
        }
        Ok(())
    }

    pub(crate) fn finish_task_run(
        &self,
        task: &Task,
        lease: &RunContext,
        outcome: crate::durable::BoundaryState,
    ) -> StoreResult<()> {
        validate_task(task)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_cleanup_run_owns_child(&transaction, &ChildRef::Task(task.id.clone()), lease)?;
        let parameters = task_params(task);
        if transaction.execute(
            TASK_RUN_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )? == 0
        {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot finish Task {}",
                lease.run_id, task.id
            )));
        }
        end_run_for_context(&transaction, lease, outcome)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn complete_task(&self, task: &Task, skipped_pr: Option<&TaskPr>) -> StoreResult<()> {
        self.complete_task_with_authority(task, skipped_pr, None)
    }

    pub(crate) fn complete_task_for_run(
        &self,
        task: &Task,
        skipped_pr: Option<&TaskPr>,
        lease: &RunContext,
    ) -> StoreResult<()> {
        self.complete_task_with_authority(task, skipped_pr, Some(lease))
    }

    fn complete_task_with_authority(
        &self,
        task: &Task,
        skipped_pr: Option<&TaskPr>,
        run_context: Option<&RunContext>,
    ) -> StoreResult<()> {
        validate_task(task)?;
        if let Some(pr) = skipped_pr {
            validate_task_pr(pr)?;
            if pr.task_id != task.id || pr.phase() != PrPhase::Working {
                return Err(StoreError::InvalidData(
                    "empty completion requires an unpublished Working Task PR".to_string(),
                ));
            }
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_task_project(&transaction, task)?;
        if let Some(lease) = run_context {
            require_run_owns_child(&transaction, &ChildRef::Task(task.id.clone()), lease)?;
        }
        if let Some(pr) = skipped_pr {
            if transaction.execute(
                "DELETE FROM task_prs
                 WHERE id=?1 AND task_id=?2
                   AND publication_requested_at IS NULL
                   AND merge_commit IS NULL AND abandoned_at IS NULL",
                params![pr.id.as_str(), pr.task_id.as_str()],
            )? == 0
            {
                return Err(StoreError::NotFound);
            }
        }
        let changed = match run_context {
            Some(lease) => update_task_for_run_in(&transaction, task, lease)?,
            None => {
                let parameters = task_control_params(task);
                transaction.execute(
                    TASK_LIFECYCLE_UPDATE,
                    rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
                )?
            }
        };
        if changed == 0 {
            if let Some(lease) = run_context {
                return Err(StoreError::InvalidAuthority(format!(
                    "Run {} cannot complete Task {}",
                    lease.run_id, task.id
                )));
            }
            return Err(StoreError::NotFound);
        }
        if run_context.is_none() {
            complete_task_work_in(&transaction, task)?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn handoff_task_body(
        &self,
        task_id: &TaskId,
        request: &ChildBodyHandoffRequest,
    ) -> StoreResult<Task> {
        validate_handoff_request(request)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut task = transaction
            .query_row(TASK_SELECT, params![task_id.as_str()], map_task_row)
            .optional()?
            .ok_or(StoreError::NotFound)?;
        let work = work_for_child_in(&transaction, &ChildRef::Task(task.id.clone()))?;
        validate_handoff_state(
            "Task",
            &task.plan.identifier,
            &work_status_in(&transaction, &work)?,
            task.abandon_intent.as_ref(),
        )?;
        let handoff = apply_handoff(
            &mut task.agent,
            &mut task.provider,
            &mut task.provider_session_id,
            request,
        );
        task.updated_at = OffsetDateTime::now_utc();
        validate_task(&task)?;
        let parameters = task_control_params(&task);
        transaction.execute(
            TASK_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        insert_task_event_in(
            &transaction,
            &task,
            &TaskEventKind::BodyHandedOff { handoff },
        )?;
        transaction.commit()?;
        Ok(task)
    }

    pub fn task(&self, task_id: &TaskId) -> StoreResult<Option<Task>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(TASK_SELECT, params![task_id.as_str()], map_task_row)
            .optional()
            .map_err(StoreError::from)
    }

    pub fn task_by_issue(&self, issue: &str) -> StoreResult<Option<Task>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = format!("{TASK_COLUMNS} WHERE t.external_issue_id=?1 OR t.issue_identifier=?1");
        let mut statement = conn.prepare(&query)?;
        let rows = statement.query_map(params![issue], map_task_row)?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        resolve_current_task(issue, tasks)
    }

    pub fn task_by_worktree(&self, worktree: &str) -> StoreResult<Option<Task>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = format!("{TASK_COLUMNS} WHERE t.worktree=?1");
        let mut statement = conn.prepare(&query)?;
        let rows = statement.query_map(params![worktree], map_task_row)?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        resolve_current_task(worktree, tasks)
    }

    pub fn list_tasks(&self, wave_id: Option<&WaveId>) -> StoreResult<Vec<Task>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (query, parameter): (String, Option<&dyn ToSql>) = match wave_id {
            Some(wave_id) => (
                format!("{TASK_COLUMNS} WHERE p.wave_id=?1 ORDER BY t.updated_at DESC"),
                Some(wave_id as &dyn ToSql),
            ),
            None => (format!("{TASK_COLUMNS} ORDER BY t.updated_at DESC"), None),
        };
        let mut statement = conn.prepare(&query)?;
        let mut tasks = Vec::new();
        if let Some(parameter) = parameter {
            let rows = statement.query_map([parameter], map_task_row)?;
            for row in rows {
                tasks.push(row?);
            }
        } else {
            let rows = statement.query_map([], map_task_row)?;
            for row in rows {
                tasks.push(row?);
            }
        }
        Ok(tasks)
    }

    pub fn update_task_pr(&self, pr: &TaskPr) -> StoreResult<()> {
        validate_task_pr(pr)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = update_task_pr(&conn, pr)?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub(crate) fn update_task_pr_for_run(
        &self,
        pr: &TaskPr,
        lease: &RunContext,
    ) -> StoreResult<()> {
        validate_task_pr(pr)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_run_owns_child(&transaction, &ChildRef::Task(pr.task_id.clone()), lease)?;
        if update_task_pr(&transaction, pr)? == 0 {
            return Err(StoreError::NotFound);
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn record_task_pr_repair_incident(
        &self,
        pr_id: &TaskPrId,
        kind: TaskPrRepairKind,
        occurred_at: OffsetDateTime,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        record_task_pr_repair_incident_on(&conn, pr_id, kind, occurred_at)
    }

    pub(crate) fn record_task_pr_repair_incident_for_run(
        &self,
        pr_id: &TaskPrId,
        kind: TaskPrRepairKind,
        occurred_at: OffsetDateTime,
        lease: &RunContext,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task_id = transaction
            .query_row(
                "SELECT task_id FROM task_prs WHERE id=?1",
                [pr_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .map(TaskId::from_raw)
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                error => StoreError::from(error),
            })?;
        require_run_owns_child(&transaction, &ChildRef::Task(task_id), lease)?;
        let inserted = record_task_pr_repair_incident_on(&transaction, pr_id, kind, occurred_at)?;
        transaction.commit()?;
        Ok(inserted)
    }

    pub fn heal_task_pr_base(&self, pr: &TaskPr) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if heal_task_pr_base(&conn, pr)? == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub(crate) fn heal_task_pr_base_for_run(
        &self,
        pr: &TaskPr,
        lease: &RunContext,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_run_owns_child(&transaction, &ChildRef::Task(pr.task_id.clone()), lease)?;
        if heal_task_pr_base(&transaction, pr)? == 0 {
            return Err(StoreError::NotFound);
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn task_prs(&self, task_id: &TaskId) -> StoreResult<Vec<TaskPr>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(&format!(
            "{TASK_PR_COLUMNS} WHERE task_id=?1 ORDER BY sequence"
        ))?;
        let rows = statement.query_map(params![task_id.as_str()], map_task_pr_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn task_pr(&self, pr_id: &TaskPrId) -> StoreResult<Option<TaskPr>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        task_pr_on(&conn, pr_id)
    }

    pub fn active_task_pr(&self, task_id: &TaskId) -> StoreResult<Option<TaskPr>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        active_task_pr_on(&conn, task_id)
    }

    pub fn settle_task_pr(&self, settled: &TaskPr, next: Option<&TaskPr>) -> StoreResult<()> {
        validate_task_pr_settlement(settled, next)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        settle_task_pr_in(&transaction, settled, next)?;
        transaction.commit()?;
        Ok(())
    }

    /// Move a stacked Task PR to its parent's current tip, or clear the parent
    /// after that work reaches the default branch. This deliberately moves the
    /// otherwise-immutable `base_commit` through a dedicated transition.
    pub fn rebase_task_pr(
        &self,
        pr_id: &TaskPrId,
        new_base: &str,
        clear_parent: bool,
        updated_at: OffsetDateTime,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = if clear_parent {
            conn.execute(
                "UPDATE task_prs SET base_commit=?2, parent_pr_id=NULL, updated_at=?3 WHERE id=?1",
                params![pr_id.as_str(), new_base, updated_at.unix_timestamp()],
            )?
        } else {
            conn.execute(
                "UPDATE task_prs SET base_commit=?2, updated_at=?3 WHERE id=?1 AND parent_pr_id IS NOT NULL",
                params![pr_id.as_str(), new_base, updated_at.unix_timestamp()],
            )?
        };
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub(crate) fn settle_task_pr_for_run(
        &self,
        settled: &TaskPr,
        next: Option<&TaskPr>,
        lease: &RunContext,
    ) -> StoreResult<()> {
        validate_task_pr_settlement(settled, next)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_run_owns_child(
            &transaction,
            &ChildRef::Task(settled.task_id.clone()),
            lease,
        )?;
        settle_task_pr_in(&transaction, settled, next)?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn settle_task_pr_merged(
        &self,
        settled: &TaskPr,
        merged_at: Option<OffsetDateTime>,
    ) -> StoreResult<crate::store::TaskPrMergeEvidenceOutcome> {
        validate_task_pr_settlement(settled, None)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let outcome = settle_task_pr_merged_in(&transaction, settled, merged_at)?;
        transaction.commit()?;
        Ok(outcome)
    }

    pub(crate) fn settle_task_pr_merged_for_run(
        &self,
        settled: &TaskPr,
        merged_at: Option<OffsetDateTime>,
        lease: &RunContext,
    ) -> StoreResult<crate::store::TaskPrMergeEvidenceOutcome> {
        validate_task_pr_settlement(settled, None)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_run_owns_child(
            &transaction,
            &ChildRef::Task(settled.task_id.clone()),
            lease,
        )?;
        let outcome = settle_task_pr_merged_in(&transaction, settled, merged_at)?;
        transaction.commit()?;
        Ok(outcome)
    }

    pub fn complete_task_after_pr(&self, task: &Task, pr: &TaskPr) -> StoreResult<()> {
        validate_task(task)?;
        validate_task_pr(pr)?;
        if pr.task_id != task.id
            || pr.phase() != PrPhase::Merged
            || pr.after_merge() != AfterMerge::CompleteTask
        {
            return Err(StoreError::InvalidData(
                "Task completion after merge requires its merged CompleteTask PR".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_task_project(&transaction, task)?;
        settle_task_pr_on(&transaction, pr)?;
        let parameters = task_control_params(task);
        if transaction.execute(
            TASK_LIFECYCLE_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )? == 0
        {
            return Err(StoreError::NotFound);
        }
        complete_task_work_in(&transaction, task)?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn complete_task_after_pr_for_run(
        &self,
        task: &Task,
        pr: &TaskPr,
        lease: &RunContext,
    ) -> StoreResult<()> {
        validate_task(task)?;
        validate_task_pr(pr)?;
        if pr.task_id != task.id
            || pr.phase() != PrPhase::Merged
            || pr.after_merge() != AfterMerge::CompleteTask
        {
            return Err(StoreError::InvalidData(
                "Task completion after merge requires its merged CompleteTask PR".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_task_project(&transaction, task)?;
        require_run_owns_child(&transaction, &ChildRef::Task(task.id.clone()), lease)?;
        settle_task_pr_on(&transaction, pr)?;
        if update_task_for_run_in(&transaction, task, lease)? == 0 {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot complete Task {}",
                lease.run_id, task.id
            )));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn task_linear_observation(
        &self,
        task_id: &TaskId,
    ) -> StoreResult<Option<TaskLinearObservation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT task_id, last_revision, last_title, last_description,
                    last_success_at, degraded_reason, updated_at
             FROM task_linear_observations WHERE task_id=?1",
            params![task_id.as_str()],
            map_task_linear_observation_row,
        )
        .optional()
        .map_err(StoreError::from)
    }

    /// Persist one Linear observation as Task direction, atomically. Exactly-once
    /// lives here: a first observation seeds the baseline and emits nothing; a
    /// stale (older-revision) response is dropped; a title/description edit
    /// becomes a Steer only if the stored content still differs; and a comment
    /// becomes a Steer only on its first entry into the ledger.
    pub fn apply_linear_observation(
        &self,
        apply: &LinearObservationApply,
    ) -> StoreResult<LinearObservationOutcome> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let work = work_for_child_in(&transaction, &ChildRef::Task(apply.task_id.clone()))?;

        let existing = transaction
            .query_row(
                "SELECT last_revision, last_title, last_description
                 FROM task_linear_observations WHERE task_id=?1",
                params![apply.task_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;

        let observed_at = apply.observed_at.unix_timestamp();

        let Some((last_revision, last_title, last_description)) = existing else {
            // Baseline: seed the cursor and mark every observed comment seen, so
            // pre-existing direction is never replayed as a surprise.
            transaction.execute(
                "INSERT INTO task_linear_observations (
                    task_id, last_revision, last_title, last_description,
                    last_success_at, degraded_reason, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?5)",
                params![
                    apply.task_id.as_str(),
                    apply.revision,
                    apply.title,
                    apply.description,
                    observed_at,
                ],
            )?;
            for follow_up in &apply.follow_ups {
                transaction.execute(
                    "INSERT OR IGNORE INTO task_linear_ingested_comments
                        (task_id, comment_id, ingested_at) VALUES (?1, ?2, ?3)",
                    params![apply.task_id.as_str(), follow_up.comment_id, observed_at],
                )?;
            }
            transaction.commit()?;
            return Ok(LinearObservationOutcome {
                baselined: true,
                content_steer_applied: false,
                follow_ups_created: Vec::new(),
            });
        };

        // Monotonic guard: an out-of-order response older than what we have
        // carries stale content, so drop it rather than let it revert direction.
        if apply.revision.as_str() < last_revision.as_str() {
            transaction.commit()?;
            return Ok(LinearObservationOutcome {
                baselined: false,
                content_steer_applied: false,
                follow_ups_created: Vec::new(),
            });
        }

        let mut content_steer_applied = false;
        if let Some(text) = &apply.content_steer {
            if last_title != apply.title || last_description != apply.description {
                Self::append_steer_in(&transaction, &work, &Author::User, text)?;
                content_steer_applied = true;
            }
        }

        // Each new human comment → one FIFO follow-up, guarded by the ledger.
        let mut follow_ups_created = Vec::new();
        for follow_up in &apply.follow_ups {
            if let Some(id) = ingest_linear_comment(
                &transaction,
                apply.task_id.as_str(),
                &follow_up.comment_id,
                &work,
                &follow_up.text,
                observed_at,
            )? {
                follow_ups_created.push(id);
            }
        }

        transaction.execute(
            "UPDATE task_linear_observations
             SET last_revision=?2, last_title=?3, last_description=?4,
                 last_success_at=?5, degraded_reason=NULL, updated_at=?5
             WHERE task_id=?1",
            params![
                apply.task_id.as_str(),
                apply.revision,
                apply.title,
                apply.description,
                observed_at,
            ],
        )?;
        transaction.commit()?;
        Ok(LinearObservationOutcome {
            baselined: false,
            content_steer_applied,
            follow_ups_created,
        })
    }

    /// Persist one human Linear comment as a FIFO Task Steer, exactly once.
    /// Webhook comments arrive one at a time (unlike the snapshot edit path), and
    /// Linear delivers at-least-once — so the `task_linear_ingested_comments`
    /// ledger is the guard: the Steer is created only on the comment id's first
    /// insertion. Returns the created Steer id, or `None` for a
    /// duplicate delivery.
    pub fn apply_linear_comment(
        &self,
        task_id: &TaskId,
        comment_id: &str,
        text: &str,
        observed_at: OffsetDateTime,
    ) -> StoreResult<Option<crate::durable::SteerId>> {
        let observed_at = observed_at.unix_timestamp();
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let work = work_for_child_in(&transaction, &ChildRef::Task(task_id.clone()))?;
        let created = ingest_linear_comment(
            &transaction,
            task_id.as_str(),
            comment_id,
            &work,
            text,
            observed_at,
        )?;
        if created.is_some() {
            // Best-effort freshness for status; a Task missing its seed row
            // (legacy) simply has nothing to update.
            transaction.execute(
                "UPDATE task_linear_observations
                 SET last_success_at=?2, degraded_reason=NULL, updated_at=?2
                 WHERE task_id=?1",
                params![task_id.as_str(), observed_at],
            )?;
        }
        transaction.commit()?;
        Ok(created)
    }

    /// Record that the latest observation failed, without moving the cursor. A
    /// Task with no baseline yet has no row to mark, which is fine — status
    /// then simply shows no observation.
    pub fn mark_task_linear_degraded(&self, task_id: &TaskId, reason: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE task_linear_observations SET degraded_reason=?2, updated_at=?3
             WHERE task_id=?1",
            params![task_id.as_str(), reason, now_unix()],
        )?;
        Ok(())
    }

    pub fn append_task_event(
        &self,
        task_id: &TaskId,
        kind: &TaskEventKind,
    ) -> StoreResult<TaskEvent> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task = transaction.query_row(TASK_SELECT, params![task_id.as_str()], map_task_row)?;
        let event = insert_task_event_in(&transaction, &task, kind)?;
        transaction.commit()?;
        Ok(event)
    }

    pub(crate) fn append_task_event_for_run(
        &self,
        task_id: &TaskId,
        lease: &RunContext,
        kind: &TaskEventKind,
    ) -> StoreResult<TaskEvent> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_run_owns_child(&transaction, &ChildRef::Task(task_id.clone()), lease)?;
        let task = transaction.query_row(TASK_SELECT, params![task_id.as_str()], map_task_row)?;
        let event = insert_task_event_in(&transaction, &task, kind)?;
        transaction.commit()?;
        Ok(event)
    }

    pub(crate) fn fail_task_run(
        &self,
        task_id: &TaskId,
        lease: &RunContext,
        error: &str,
        resumable: bool,
    ) -> StoreResult<TaskEvent> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_cleanup_run_owns_child(&transaction, &ChildRef::Task(task_id.clone()), lease)?;
        let task = transaction.query_row(TASK_SELECT, params![task_id.as_str()], map_task_row)?;
        validate_task(&task)?;
        let event = insert_task_event_in(
            &transaction,
            &task,
            &TaskEventKind::Failed {
                error: error.to_string(),
                resumable,
            },
        )?;
        end_run_for_context(&transaction, lease, crate::durable::BoundaryState::Failed)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn task_events_after(&self, task_id: &TaskId, cursor: i64) -> StoreResult<Vec<TaskEvent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        task_events_after_in(&conn, task_id, cursor)
    }

    pub fn task_event(&self, task_id: &TaskId, event_id: i64) -> StoreResult<Option<TaskEvent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT id, task_id, kind_json, created_at
             FROM task_events WHERE task_id = ?1 AND id = ?2",
            params![task_id.as_str(), event_id],
            map_task_event_row,
        )
        .optional()
        .map_err(StoreError::from)
    }

    /// When this Task last appended a durable event. This is the progress
    /// signal the body observation reads: a live body that has written nothing to
    /// its event log past the stall deadline is stalled, not working. `None` means
    /// no events yet (the status change is the only progress the caller can use).
    pub fn latest_task_event_at(&self, task_id: &TaskId) -> StoreResult<Option<OffsetDateTime>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let seconds: Option<i64> = conn.query_row(
            "SELECT MAX(created_at) FROM task_events WHERE task_id = ?1",
            params![task_id.as_str()],
            |row| row.get(0),
        )?;
        Ok(seconds.map(crate::store::rows::unix_to_datetime))
    }

    /// The newest `limit` events, newest first. Recovery reads a bounded window
    /// rather than the whole log: a long-lived Task accumulates thousands of
    /// events, and the attempt count only ever looks at the recent tail.
    pub fn recent_task_events(&self, task_id: &TaskId, limit: u32) -> StoreResult<Vec<TaskEvent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT id, task_id, kind_json, created_at
             FROM task_events WHERE task_id = ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![task_id.as_str(), limit], map_task_event_row)?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    pub fn latest_task_event(&self, task_id: &TaskId) -> StoreResult<Option<TaskEvent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT id, task_id, kind_json, created_at
             FROM task_events WHERE task_id = ?1 ORDER BY id DESC LIMIT 1",
            params![task_id.as_str()],
            map_task_event_row,
        )
        .optional()
        .map_err(StoreError::from)
    }

    // Projects are durable KR-pursuit children. They share the same
    // process/receipt shape as Tasks but deliberately own no worktree.

    pub fn insert_project(&self, project: &Project) -> StoreResult<()> {
        validate_project(project)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            PROJECT_INSERT,
            rusqlite::params_from_iter(project_params(project).iter().map(|value| value.as_ref())),
        )?;
        create_project_spine(&transaction, project)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn insert_project_with_steer(
        &self,
        project: &Project,
        author: &Author,
        text: &str,
    ) -> StoreResult<()> {
        validate_project(project)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let parameters = project_params(project);
        transaction.execute(
            PROJECT_INSERT,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        create_project_spine(&transaction, project)?;
        let work = work_for_child_in(&transaction, &ChildRef::Project(project.id.clone()))?;
        Self::append_steer_in(&transaction, &work, author, text)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn reopen_project(
        &self,
        project: &Project,
        author: &Author,
        text: &str,
    ) -> StoreResult<()> {
        validate_project(project)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous: String = transaction.query_row(
            "SELECT state FROM epochs WHERE project_id=?1 ORDER BY number DESC LIMIT 1",
            [project.id.as_str()],
            |row| row.get(0),
        )?;
        if !matches!(previous.as_str(), "completed" | "abandoned") {
            return Err(StoreError::InvalidData(format!(
                "Project {} is {previous}; only terminal Work can open a new Epoch",
                project.id
            )));
        }
        let parameters = project_params(project);
        transaction.execute(
            PROJECT_REOPEN_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        create_project_spine(&transaction, project)?;
        let work = work_for_child_in(&transaction, &ChildRef::Project(project.id.clone()))?;
        Self::append_steer_in(&transaction, &work, author, text)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn update_project(&self, project: &Project) -> StoreResult<()> {
        validate_project(project)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let parameters = project_control_params(project);
        let changed = transaction.execute(
            PROJECT_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn update_project_for_run(
        &self,
        project: &Project,
        lease: &RunContext,
    ) -> StoreResult<()> {
        validate_project(project)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        if update_project_for_run_in(&conn, project, lease)? == 0 {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot update Project {}",
                lease.run_id, project.id
            )));
        }
        Ok(())
    }

    pub(crate) fn adopt_project_plan_for_run(
        &self,
        project_id: &ProjectId,
        plan: &ProjectPlan,
        lease: &RunContext,
    ) -> StoreResult<(Project, bool)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_run_owns_child(&transaction, &ChildRef::Project(project_id.clone()), lease)?;
        let mut project = transaction.query_row(
            PROJECT_SELECT,
            params![project_id.as_str()],
            map_project_row,
        )?;
        if plan.id != project.plan.id {
            return Err(StoreError::InvalidData(format!(
                "Project {} cannot adopt planning for Linear Project {}",
                project.id,
                plan.id.as_str()
            )));
        }
        if plan.pm_snapshot_synced_at < project.plan.pm_snapshot_synced_at {
            return Err(StoreError::InvalidData(format!(
                "Project {} cannot move its PM snapshot backward from {} to {}",
                project.id, project.plan.pm_snapshot_synced_at, plan.pm_snapshot_synced_at
            )));
        }
        let changed = !project.plan.has_same_content(plan);
        if project.plan == *plan {
            transaction.commit()?;
            return Ok((project, false));
        }
        project.plan = plan.clone();
        if changed {
            project.updated_at = OffsetDateTime::now_utc();
        }
        validate_project(&project)?;
        let parameters = project_params(&project);
        if transaction.execute(
            PROJECT_RUN_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )? == 0
        {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot adopt planning for Project {}",
                lease.run_id, project.id
            )));
        }
        transaction.commit()?;
        Ok((project, changed))
    }

    pub(crate) fn finish_project_run(
        &self,
        project: &Project,
        lease: &RunContext,
        outcome: crate::durable::BoundaryState,
    ) -> StoreResult<()> {
        validate_project(project)?;
        if outcome == crate::durable::BoundaryState::Failed {
            return Err(StoreError::InvalidData(
                "Project failure must use fail_project_run so its event and Run settle atomically"
                    .to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_cleanup_run_owns_child(
            &transaction,
            &ChildRef::Project(project.id.clone()),
            lease,
        )?;
        let parameters = project_params(project);
        if transaction.execute(
            PROJECT_RUN_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )? == 0
        {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot finish Project {}",
                lease.run_id, project.id
            )));
        }
        end_run_for_context(&transaction, lease, outcome)?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn fail_project_run(
        &self,
        project: &Project,
        lease: &RunContext,
        error: &str,
    ) -> StoreResult<ProjectEvent> {
        validate_project(project)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_cleanup_run_owns_child(
            &transaction,
            &ChildRef::Project(project.id.clone()),
            lease,
        )?;
        let parameters = project_params(project);
        if transaction.execute(
            PROJECT_RUN_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )? == 0
        {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot fail Project {}",
                lease.run_id, project.id
            )));
        }
        let event = insert_project_event_in(
            &transaction,
            project,
            &ProjectEventKind::Failed {
                error: error.to_string(),
                resumable: true,
            },
            Some(&lease.run_id),
        )?;
        end_run_for_context(&transaction, lease, crate::durable::BoundaryState::Failed)?;
        transaction.commit()?;
        Ok(event)
    }

    pub(crate) fn complete_project_run(
        &self,
        project: &Project,
        lease: &RunContext,
        basis: &Basis,
        summary: &str,
    ) -> StoreResult<ProjectEvent> {
        validate_project(project)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_run_owns_child(&transaction, &ChildRef::Project(project.id.clone()), lease)?;
        let run = validate_run_context(&transaction, lease)?;
        let epoch = current_epoch_in(&transaction, &run.work)?;
        if epoch.id != run.epoch_id {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} does not own the current Project Epoch {}",
                run.id, epoch.id
            )));
        }
        validate_basis(&epoch.current_basis, basis)?;
        validate_completion_readiness_in(&transaction, &run)?;
        if update_project_for_run_in(&transaction, project, lease)? == 0 {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot complete Project {}",
                lease.run_id, project.id
            )));
        }
        let event = insert_project_event_in(
            &transaction,
            project,
            &ProjectEventKind::Completed {
                summary: summary.to_string(),
            },
            Some(&lease.run_id),
        )?;
        if transaction.execute(
            "UPDATE epochs SET state='done', terminal_at=?2
             WHERE id=?1 AND state='open' AND current_rev=?3",
            params![epoch.id.as_str(), now_unix(), basis.revision as i64,],
        )? != 1
        {
            return Err(StoreError::InvalidData(format!(
                "Project {} Work changed while completion was being recorded",
                project.id
            )));
        }
        end_run_for_context(
            &transaction,
            lease,
            crate::durable::BoundaryState::Succeeded,
        )?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn handoff_project_body(
        &self,
        project_id: &ProjectId,
        request: &ChildBodyHandoffRequest,
    ) -> StoreResult<Project> {
        validate_handoff_request(request)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut project = transaction
            .query_row(
                PROJECT_SELECT,
                params![project_id.as_str()],
                map_project_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        let work = work_for_child_in(&transaction, &ChildRef::Project(project.id.clone()))?;
        validate_handoff_state(
            "Project",
            &project.plan.slug,
            &work_status_in(&transaction, &work)?,
            project.abandon_intent.as_ref(),
        )?;
        let handoff = apply_handoff(
            &mut project.agent,
            &mut project.provider,
            &mut project.provider_session_id,
            request,
        );
        project.updated_at = OffsetDateTime::now_utc();
        validate_project(&project)?;
        let parameters = project_control_params(&project);
        transaction.execute(
            PROJECT_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        insert_project_event_in(
            &transaction,
            &project,
            &ProjectEventKind::BodyHandedOff { handoff },
            None,
        )?;
        transaction.commit()?;
        Ok(project)
    }

    pub fn project(&self, project_id: &ProjectId) -> StoreResult<Option<Project>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            PROJECT_SELECT,
            params![project_id.as_str()],
            map_project_row,
        )
        .optional()
        .map_err(StoreError::from)
    }

    pub fn project_by_project(&self, project: &str) -> StoreResult<Option<Project>> {
        if let Ok(project_id) = ProjectId::parse(project) {
            return self.project(&project_id);
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = format!(
            "{PROJECT_COLUMNS}
             WHERE external_project_id=?1 OR project_slug=?1
             ORDER BY created_at DESC, id DESC
             LIMIT 1"
        );
        conn.query_row(&query, params![project], map_project_row)
            .optional()
            .map_err(StoreError::from)
    }

    pub fn list_projects(&self, wave_id: Option<&WaveId>) -> StoreResult<Vec<Project>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = match wave_id {
            Some(_) => {
                format!("{PROJECT_COLUMNS} WHERE wave_id=?1 ORDER BY updated_at DESC")
            }
            None => format!("{PROJECT_COLUMNS} ORDER BY updated_at DESC"),
        };
        let mut statement = conn.prepare(&query)?;
        let mut projects = Vec::new();
        if let Some(wave_id) = wave_id {
            let rows = statement.query_map(params![wave_id], map_project_row)?;
            for row in rows {
                projects.push(row?);
            }
        } else {
            let rows = statement.query_map([], map_project_row)?;
            for row in rows {
                projects.push(row?);
            }
        }
        Ok(projects)
    }

    pub fn append_project_event(
        &self,
        project_id: &ProjectId,
        kind: &ProjectEventKind,
    ) -> StoreResult<ProjectEvent> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let project = transaction.query_row(
            PROJECT_SELECT,
            params![project_id.as_str()],
            map_project_row,
        )?;
        let event = insert_project_event_in(&transaction, &project, kind, None)?;
        transaction.commit()?;
        Ok(event)
    }

    pub(crate) fn append_project_event_for_run(
        &self,
        project_id: &ProjectId,
        lease: &RunContext,
        kind: &ProjectEventKind,
    ) -> StoreResult<ProjectEvent> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_run_owns_child(&transaction, &ChildRef::Project(project_id.clone()), lease)?;
        let project = transaction.query_row(
            PROJECT_SELECT,
            params![project_id.as_str()],
            map_project_row,
        )?;
        let event = insert_project_event_in(&transaction, &project, kind, Some(&lease.run_id))?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn project_events_after(
        &self,
        project_id: &ProjectId,
        cursor: i64,
    ) -> StoreResult<Vec<ProjectEvent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT id, project_id, kind_json, run_id, created_at
             FROM project_events WHERE project_id=?1 AND id>?2 ORDER BY id",
        )?;
        let rows =
            statement.query_map(params![project_id.as_str(), cursor], map_project_event_row)?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    /// When this Project last appended a durable event. The progress
    /// signal for the Project body observation, mirroring [`Self::latest_task_event_at`].
    pub fn latest_project_event_at(
        &self,
        project_id: &ProjectId,
    ) -> StoreResult<Option<OffsetDateTime>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let seconds: Option<i64> = conn.query_row(
            "SELECT MAX(created_at) FROM project_events WHERE project_id = ?1",
            params![project_id.as_str()],
            |row| row.get(0),
        )?;
        Ok(seconds.map(crate::store::rows::unix_to_datetime))
    }

    pub fn latest_project_event(
        &self,
        project_id: &ProjectId,
    ) -> StoreResult<Option<ProjectEvent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT id, project_id, kind_json, run_id, created_at
             FROM project_events WHERE project_id=?1 ORDER BY id DESC LIMIT 1",
            params![project_id.as_str()],
            map_project_event_row,
        )
        .optional()
        .map_err(StoreError::from)
    }

    pub fn latest_project_failure(
        &self,
        project_id: &ProjectId,
    ) -> StoreResult<Option<crate::project::HistoricalFailure>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let event = conn
            .query_row(
                "SELECT id, project_id, kind_json, run_id, created_at
                 FROM project_events
                 WHERE project_id=?1 AND json_extract(kind_json, '$.kind')='failed'
                 ORDER BY id DESC LIMIT 1",
                params![project_id.as_str()],
                map_project_event_row,
            )
            .optional()?;
        Ok(event
            .as_ref()
            .and_then(crate::project::HistoricalFailure::from_event))
    }

    pub fn pending_observations(
        &self,
        recipient: &ObservationRecipient,
    ) -> StoreResult<Vec<ObservationOutboxRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (kind, id) = recipient_columns(recipient);
        let mut statement = conn.prepare(
            "SELECT id, recipient_kind, recipient_id, source_kind, source_id,
                    event_id, payload_json, delivered_at
             FROM observation_outbox
             WHERE recipient_kind=?1 AND recipient_id=?2 AND delivered_at IS NULL
             ORDER BY id",
        )?;
        let rows = statement.query_map(params![kind, id], map_observation_row)?;
        let mut observations = Vec::new();
        for row in rows {
            observations.push(row?);
        }
        Ok(observations)
    }

    pub fn pending_project_observations(
        &self,
        project_id: &ProjectId,
    ) -> StoreResult<Vec<ObservationOutboxRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT id, recipient_kind, recipient_id, source_kind, source_id,
                    event_id, payload_json, delivered_at
             FROM observation_outbox
             WHERE recipient_kind='project'
               AND recipient_id=?1
               AND delivered_at IS NULL
             ORDER BY id",
        )?;
        let rows = statement.query_map(params![project_id.as_str()], map_observation_row)?;
        let mut observations = Vec::new();
        for row in rows {
            observations.push(row?);
        }
        Ok(observations)
    }

    pub fn mark_observation_delivered(&self, id: i64) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE observation_outbox SET delivered_at=?1
             WHERE id=?2 AND delivered_at IS NULL",
            params![now_unix(), id],
        )?;
        Ok(())
    }

    pub fn consume_task_observation_for_project(
        &self,
        project_id: &ProjectId,
        observation: &ObservationOutboxRow,
    ) -> StoreResult<bool> {
        self.consume_task_observation_for_project_with_authority(project_id, observation, None)
    }

    pub(crate) fn consume_task_observation_for_project_for_run(
        &self,
        project_id: &ProjectId,
        observation: &ObservationOutboxRow,
        lease: &RunContext,
    ) -> StoreResult<bool> {
        self.consume_task_observation_for_project_with_authority(
            project_id,
            observation,
            Some(lease),
        )
    }

    fn consume_task_observation_for_project_with_authority(
        &self,
        project_id: &ProjectId,
        observation: &ObservationOutboxRow,
        run_context: Option<&RunContext>,
    ) -> StoreResult<bool> {
        let (
            ObservationRecipient::Project {
                project_id: recipient_id,
            },
            ChildRef::Task(task_id),
            ChildEventPayload::Task { event },
        ) = (
            &observation.recipient,
            &observation.source,
            &observation.payload,
        )
        else {
            return Err(StoreError::InvalidData(
                "Project can consume only supervised Task observations".to_string(),
            ));
        };
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(lease) = run_context {
            require_run_owns_child(&transaction, &ChildRef::Project(project_id.clone()), lease)?;
        }
        if recipient_id != project_id {
            return Err(StoreError::InvalidData(format!(
                "observation {} belongs to Project {recipient_id}, not {project_id}",
                observation.id
            )));
        }
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM project_events
                WHERE project_id=?1
                  AND json_extract(kind_json, '$.kind')='task_observed'
                  AND json_extract(kind_json, '$.task_id')=?2
                  AND json_extract(kind_json, '$.task_event_id')=?3
             )",
            params![project_id.as_str(), task_id.as_str(), observation.event_id,],
            |row| row.get(0),
        )?;
        if !exists {
            let kind = ProjectEventKind::TaskObserved {
                task_id: task_id.clone(),
                task_event_id: observation.event_id,
                event: Box::new(event.clone()),
            };
            let project = transaction.query_row(
                PROJECT_SELECT,
                params![project_id.as_str()],
                map_project_row,
            )?;
            insert_project_event_in(
                &transaction,
                &project,
                &kind,
                run_context.map(|context| &context.run_id),
            )?;
        }
        let now = now_unix();
        transaction.execute(
            "UPDATE observation_outbox SET delivered_at=?1
             WHERE id=?2 AND delivered_at IS NULL",
            params![now, observation.id],
        )?;
        transaction.execute(
            "UPDATE projects
             SET observation_cursor=MAX(observation_cursor, ?1), updated_at=?2
             WHERE id=?3",
            params![observation.id, now, project_id.as_str()],
        )?;
        transaction.commit()?;
        Ok(!exists)
    }
}

fn validate_task(task: &Task) -> StoreResult<()> {
    task.validate()
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

// A controller-proven Task completion is its own authority boundary. It must
// not fabricate a Run merely to reuse the agent-owned `done` transition.
fn complete_task_work_in(conn: &Connection, task: &Task) -> StoreResult<()> {
    let proposal = task
        .gate_proposal
        .as_ref()
        .filter(|proposal| proposal.done)
        .ok_or_else(|| {
            StoreError::InvalidData("completed Task requires a done gate proposal".to_string())
        })?;
    let (epoch_id, state) = conn.query_row(
        "SELECT id, state FROM epochs
         WHERE task_id=?1 ORDER BY number DESC LIMIT 1",
        [task.id.as_str()],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    match state.as_str() {
        "done" => return Ok(()),
        "open" => {}
        "abandoned" => {
            return Err(StoreError::InvalidData(format!(
                "Task {} Work is abandoned and cannot be completed",
                task.id
            )))
        }
        other => {
            return Err(StoreError::InvalidData(format!(
                "Task {} Work has invalid Epoch state {other:?}",
                task.id
            )))
        }
    }
    let active_run: Option<String> = conn
        .query_row(
            "SELECT id FROM runs WHERE epoch_id=?1 AND state!='ended' LIMIT 1",
            [epoch_id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(run_id) = active_run {
        return Err(StoreError::InvalidData(format!(
            "Task {} Work cannot complete while Run {run_id} is active",
            task.id
        )));
    }
    if conn.execute(
        "UPDATE epochs SET state='done', terminal_at=?2
         WHERE id=?1 AND state='open'",
        params![epoch_id, now_unix()],
    )? != 1
    {
        return Err(StoreError::InvalidData(format!(
            "Task {} Work changed while completion was being recorded",
            task.id
        )));
    }
    insert_task_event_in(
        conn,
        task,
        &TaskEventKind::Completed {
            summary: proposal.reason.clone(),
        },
    )?;
    Ok(())
}

fn resolve_current_task(key: &str, mut tasks: Vec<Task>) -> StoreResult<Option<Task>> {
    if tasks.len() > 1 {
        return Err(StoreError::InvalidData(format!(
            "multiple stable Tasks resolve to {key:?}"
        )));
    }
    Ok(tasks.pop())
}

fn validate_handoff_request(request: &ChildBodyHandoffRequest) -> StoreResult<()> {
    if request.agent.trim().is_empty() || request.provider.trim().is_empty() {
        return Err(StoreError::InvalidData(
            "body handoff requires an agent and provider".to_string(),
        ));
    }
    if request.reason.trim().is_empty() {
        return Err(StoreError::InvalidData(
            "body handoff requires an audit reason".to_string(),
        ));
    }
    Ok(())
}

fn validate_handoff_state(
    kind: &str,
    label: &str,
    status: &crate::durable::WorkStatus,
    abandon_intent: Option<&AbandonIntent>,
) -> StoreResult<()> {
    if matches!(
        status,
        crate::durable::WorkStatus::Done | crate::durable::WorkStatus::Abandoned
    ) {
        return Err(StoreError::InvalidData(format!(
            "{kind} {label} is terminal; Work cannot hand off bodies"
        )));
    }
    if let Some(intent) = abandon_intent {
        return Err(StoreError::InvalidData(format!(
            "{kind} {label} is being abandoned: {}",
            intent.reason
        )));
    }
    if matches!(status, crate::durable::WorkStatus::Running { .. }) {
        return Err(StoreError::InvalidData(format!(
            "{kind} {label} already has an active writer; interrupt it before changing providers"
        )));
    }
    Ok(())
}

fn apply_handoff(
    agent: &mut String,
    provider: &mut String,
    provider_session_id: &mut Option<String>,
    request: &ChildBodyHandoffRequest,
) -> ChildBodyHandoff {
    let handoff = ChildBodyHandoff {
        from_agent: agent.clone(),
        to_agent: request.agent.clone(),
        from_provider: provider.clone(),
        to_provider: request.provider.clone(),
        reason: request.reason.clone(),
    };
    if *provider != request.provider {
        *provider_session_id = None;
    }
    *agent = request.agent.clone();
    *provider = request.provider.clone();
    handoff
}

fn validate_task_pr(pr: &TaskPr) -> StoreResult<()> {
    pr.validate()
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn validate_initial_task_pr(task: &Task, pr: &TaskPr) -> StoreResult<()> {
    validate_task_pr(pr)?;
    if pr.task_id != task.id || pr.sequence != 1 || pr.phase() != PrPhase::Working {
        return Err(StoreError::InvalidData(
            "Task requires its sequence-1 Working PR".to_string(),
        ));
    }
    Ok(())
}

fn insert_initial_task(
    conn: &rusqlite::Transaction<'_>,
    task: &Task,
    pr: &TaskPr,
) -> StoreResult<()> {
    validate_task(task)?;
    validate_initial_task_pr(task, pr)?;
    validate_task_project(conn, task)?;
    let parameters = task_params(task);
    conn.execute(
        TASK_INSERT,
        rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
    )?;
    create_task_spine(conn, task)?;
    insert_task_pr(conn, pr)?;
    seed_task_linear_observation(conn, task)
}

/// Seed the Linear observation cursor from the planning directive, in the Task's
/// creation transaction. Webhooks only fire for changes *after* subscription, so
/// there is no cursor to build lazily on a first poll — seeding here means the
/// first issue-edit webhook diffs against the directive title/description instead of
/// baselining (and swallowing) it. The revision seeds empty so any real Linear
/// `updatedAt` wins the monotonic guard.
fn seed_task_linear_observation(conn: &Connection, task: &Task) -> StoreResult<()> {
    conn.execute(
        "INSERT OR IGNORE INTO task_linear_observations (
            task_id, last_revision, last_title, last_description,
            last_success_at, degraded_reason, updated_at
         ) VALUES (?1, '', ?2, ?3, ?4, NULL, ?4)",
        params![
            task.id.as_str(),
            task.plan.title,
            task.plan.description,
            now_unix(),
        ],
    )?;
    Ok(())
}

fn validate_task_project(conn: &Connection, task: &Task) -> StoreResult<()> {
    let owner = conn
        .query_row(
            "SELECT wave_id FROM projects WHERE id=?1",
            params![task.project_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(wave_id) = owner else {
        return Err(StoreError::InvalidData(format!(
            "Task {} requires Project {}",
            task.id, task.project_id
        )));
    };
    if wave_id != task.wave_id.as_str() {
        return Err(StoreError::InvalidData(format!(
            "Project {} does not belong to Task {}'s Wave {}",
            task.project_id, task.id, task.wave_id
        )));
    }
    Ok(())
}

fn validate_project(project: &Project) -> StoreResult<()> {
    project
        .validate()
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

const TASK_INSERT: &str = "INSERT INTO tasks (
    id, project_id, external_issue_id, issue_identifier, issue_title,
    issue_description, pm_snapshot_synced_at, pm_writeback_json,
    worktree, workspace_slug,
    agent, provider, provider_session_id, abandon_requested_at, abandon_reason,
    iterate_flow, phase_cursor, phase_iteration,
    kickoff_flow, gate_flow,
    lifecycle_phase, phase_epoch, gate_cycle, gate_proposal_json,
    created_at, updated_at
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
    ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26
)";
const TASK_COLUMNS: &str = "SELECT
    t.id, t.external_issue_id, t.issue_identifier, t.issue_title, t.issue_description,
    p.wave_id, t.worktree, t.workspace_slug,
    t.agent, t.provider, t.provider_session_id, t.created_at, t.updated_at,
    t.pm_snapshot_synced_at, t.pm_writeback_json, t.project_id,
    t.abandon_requested_at, t.abandon_reason,
    t.iterate_flow, t.phase_cursor, t.phase_iteration,
    t.kickoff_flow, t.gate_flow,
    t.lifecycle_phase, t.phase_epoch, t.gate_cycle, t.gate_proposal_json
    FROM tasks t JOIN projects p ON p.id=t.project_id";
pub(super) const TASK_SELECT: &str = "SELECT
    t.id, t.external_issue_id, t.issue_identifier, t.issue_title, t.issue_description,
    p.wave_id, t.worktree, t.workspace_slug,
    t.agent, t.provider, t.provider_session_id, t.created_at, t.updated_at,
    t.pm_snapshot_synced_at, t.pm_writeback_json, t.project_id,
    t.abandon_requested_at, t.abandon_reason,
    t.iterate_flow, t.phase_cursor, t.phase_iteration,
    t.kickoff_flow, t.gate_flow,
    t.lifecycle_phase, t.phase_epoch, t.gate_cycle, t.gate_proposal_json
    FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?1";
const TASK_UPDATE: &str = "UPDATE tasks SET
    project_id=?2, external_issue_id=?3, issue_identifier=?4,
    issue_title=?5, issue_description=?6, pm_snapshot_synced_at=?7,
    pm_writeback_json=?8, worktree=?9, workspace_slug=?10, agent=?11, provider=?12,
    provider_session_id=?13, abandon_requested_at=?14, abandon_reason=?15,
    iterate_flow=?16, kickoff_flow=?19, gate_flow=?20,
    created_at=?25, updated_at=?26
    WHERE id=?1";
const TASK_LIFECYCLE_UPDATE: &str = "UPDATE tasks SET
    project_id=?2, external_issue_id=?3, issue_identifier=?4,
    issue_title=?5, issue_description=?6, pm_snapshot_synced_at=?7,
    pm_writeback_json=?8, worktree=?9, workspace_slug=?10, agent=?11, provider=?12,
    provider_session_id=?13, abandon_requested_at=?14, abandon_reason=?15,
    iterate_flow=?16, phase_cursor=?17,
    phase_iteration=?18, kickoff_flow=?19,
    gate_flow=?20, lifecycle_phase=?21,
    phase_epoch=?22, gate_cycle=?23, gate_proposal_json=?24,
    created_at=?25, updated_at=?26
    WHERE id=?1";
const TASK_RUN_UPDATE: &str = "UPDATE tasks SET
    project_id=?2, external_issue_id=?3, issue_identifier=?4,
    issue_title=?5, issue_description=?6, pm_snapshot_synced_at=?7,
    pm_writeback_json=?8, worktree=?9, workspace_slug=?10, agent=?11, provider=?12,
    provider_session_id=?13, abandon_requested_at=?14, abandon_reason=?15,
    iterate_flow=?16,
    lifecycle_phase=CASE WHEN ?22>=phase_epoch THEN ?21 ELSE lifecycle_phase END,
    phase_cursor=CASE
        WHEN ?22>phase_epoch OR
             (?22=phase_epoch AND (?18>phase_iteration OR
                                   (?18=phase_iteration AND ?17>phase_cursor)))
        THEN ?17 ELSE phase_cursor
    END,
    phase_iteration=CASE
        WHEN ?22>phase_epoch THEN ?18
        WHEN ?22=phase_epoch THEN MAX(phase_iteration, ?18)
        ELSE phase_iteration
    END,
    kickoff_flow=?19, gate_flow=?20,
    phase_epoch=MAX(phase_epoch, ?22),
    gate_cycle=CASE WHEN ?22>=phase_epoch THEN ?23 ELSE gate_cycle END,
    gate_proposal_json=CASE WHEN ?22>=phase_epoch THEN ?24 ELSE gate_proposal_json END,
    created_at=?25, updated_at=?26
    WHERE id=?1";
const TASK_PR_COLUMNS: &str = "SELECT
    id, task_id, sequence, slug, branch, base_commit,
    publication_requested_at, after_merge, next_slug, github_number, github_url,
    merge_commit, abandoned_at, created_at, updated_at,
    github_head_sha, ci_observation, parent_pr_id, github_observation,
    linear_attachment_id, linear_comment_id, linear_link_error,
    merge_mode, merge_requested_at, merge_head_sha,
    pr_title, pr_body, pr_copy_head_sha
    FROM task_prs";
const TASK_PR_SELECT: &str = "SELECT
    id, task_id, sequence, slug, branch, base_commit,
    publication_requested_at, after_merge, next_slug, github_number, github_url,
    merge_commit, abandoned_at, created_at, updated_at,
    github_head_sha, ci_observation, parent_pr_id, github_observation,
    linear_attachment_id, linear_comment_id, linear_link_error,
    merge_mode, merge_requested_at, merge_head_sha,
    pr_title, pr_body, pr_copy_head_sha
    FROM task_prs WHERE id=?1";
/// Persist one Linear comment as a Steer exactly once. The insert
/// into `task_linear_ingested_comments` is the guard — the command is written
/// only when the comment id is new to the ledger, so a redelivered webhook or an
/// overlapping catch-up read cannot double-deliver. Shared by the snapshot apply
/// loop and the single-comment webhook path.
fn ingest_linear_comment(
    conn: &rusqlite::Transaction<'_>,
    task_id: &str,
    comment_id: &str,
    work: &crate::durable::WorkRef,
    text: &str,
    observed_at: i64,
) -> StoreResult<Option<crate::durable::SteerId>> {
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO task_linear_ingested_comments
            (task_id, comment_id, ingested_at) VALUES (?1, ?2, ?3)",
        params![task_id, comment_id, observed_at],
    )?;
    if inserted == 1 {
        let receipt = SqliteStore::append_steer_in(conn, work, &Author::User, text)?;
        Ok(Some(receipt.steer.id))
    } else {
        Ok(None)
    }
}

fn task_params(task: &Task) -> Vec<Box<dyn ToSql>> {
    vec![
        Box::new(task.id.as_str().to_string()),
        Box::new(task.project_id.as_str().to_string()),
        Box::new(task.plan.id.as_str().to_string()),
        Box::new(task.plan.identifier.clone()),
        Box::new(task.plan.title.clone()),
        Box::new(task.plan.description.clone()),
        Box::new(task.plan.pm_snapshot_synced_at),
        Box::new(
            serde_json::to_string(&task.pm_writeback)
                .expect("Task PM writeback state must serialize"),
        ),
        Box::new(task.worktree.display().to_string()),
        Box::new(task.workspace_slug.clone()),
        Box::new(task.agent.clone()),
        Box::new(task.provider.clone()),
        Box::new(task.provider_session_id.clone()),
        Box::new(
            task.abandon_intent
                .as_ref()
                .map(|intent| intent.requested_at.unix_timestamp()),
        ),
        Box::new(
            task.abandon_intent
                .as_ref()
                .map(|intent| intent.reason.clone()),
        ),
        Box::new(task.lifecycle.loop_.flow.clone()),
        Box::new(i64::from(task.phase_cursor)),
        Box::new(i64::from(task.phase_iteration)),
        Box::new(task.lifecycle.first.flow.clone()),
        Box::new(task.lifecycle.finally.flow.clone()),
        Box::new(task.lifecycle_phase.storage_str().to_string()),
        Box::new(i64::from(task.phase_epoch)),
        Box::new(i64::from(task.gate_cycle)),
        Box::new(task.gate_proposal.as_ref().map(|proposal| {
            serde_json::to_string(proposal).expect("Task gate proposal must serialize")
        })),
        Box::new(task.created_at.unix_timestamp()),
        Box::new(task.updated_at.unix_timestamp()),
    ]
}

fn task_control_params(task: &Task) -> Vec<Box<dyn ToSql>> {
    task_params(task)
}

fn update_task_for_run_in(
    conn: &Connection,
    task: &Task,
    lease: &RunContext,
) -> StoreResult<usize> {
    require_run_owns_child(conn, &ChildRef::Task(task.id.clone()), lease)?;
    let parameters = task_params(task);
    Ok(conn.execute(
        TASK_RUN_UPDATE,
        rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
    )?)
}
fn require_run_owns_child(
    conn: &Connection,
    target: &ChildRef,
    lease: &RunContext,
) -> StoreResult<()> {
    let run = validate_run_context(conn, lease)?;
    let work = work_for_child_in(conn, target)?;
    if run.work != work {
        return Err(StoreError::InvalidAuthority(format!(
            "Run {} does not own {} Work {}",
            run.id,
            target.target_kind(),
            work.id()
        )));
    }
    Ok(())
}

fn require_cleanup_run_owns_child(
    conn: &Connection,
    target: &ChildRef,
    lease: &RunContext,
) -> StoreResult<()> {
    let run = validate_stop_context(conn, lease)?;
    let work = work_for_child_in(conn, target)?;
    if run.work != work {
        return Err(StoreError::InvalidAuthority(format!(
            "Run {} does not own {} Work {}",
            run.id,
            target.target_kind(),
            work.id()
        )));
    }
    Ok(())
}

fn insert_task_pr(conn: &Connection, pr: &TaskPr) -> StoreResult<()> {
    validate_task_pr(pr)?;
    let publication = pr.publication.as_ref();
    let presentation = publication.and_then(|publication| publication.presentation.as_ref());
    let github = publication.and_then(|publication| publication.github.as_ref());
    let merge = publication.and_then(|publication| publication.merge.as_ref());
    conn.execute(
        "INSERT INTO task_prs (
            id, task_id, sequence, slug, branch, base_commit,
            publication_requested_at, after_merge, next_slug,
            github_number, github_url, merge_commit, abandoned_at,
            created_at, updated_at, github_head_sha, ci_observation, parent_pr_id,
            github_observation,
            linear_attachment_id, linear_comment_id, linear_link_error,
            merge_mode, merge_requested_at, merge_head_sha,
            pr_title, pr_body, pr_copy_head_sha
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28)",
        params![
            pr.id.as_str(),
            pr.task_id.as_str(),
            i64::from(pr.sequence),
            pr.slug,
            pr.branch,
            pr.base_commit,
            publication.map(|publication| publication.requested_at.unix_timestamp()),
            merge.map(|request| request.after_merge.as_str()),
            merge.and_then(|request| request.next_slug.as_deref()),
            github.map(|github| i64::from(github.number)),
            github.map(|github| github.url.as_str()),
            pr.merge_commit,
            pr.abandoned_at.map(OffsetDateTime::unix_timestamp),
            pr.created_at.unix_timestamp(),
            pr.updated_at.unix_timestamp(),
            github.and_then(|github| github.head_sha.as_deref()),
            task_pr_ci_json(pr)?,
            pr.parent_pr_id.as_ref().map(TaskPrId::as_str),
            task_pr_github_observation_json(pr)?,
            pr.linear_attachment_id.as_deref(),
            pr.linear_comment_id.as_deref(),
            pr.linear_link_error.as_deref(),
            merge.map(|request| request.mode.as_str()),
            merge.map(|request| request.requested_at.unix_timestamp()),
            merge.map(|request| request.head_sha.as_str()),
            presentation.map(|copy| copy.title.as_str()),
            presentation.map(|copy| copy.body.as_str()),
            presentation.map(|copy| copy.head_sha.as_str()),
        ],
    )?;
    Ok(())
}

fn record_task_pr_repair_incident_on(
    conn: &Connection,
    pr_id: &TaskPrId,
    kind: TaskPrRepairKind,
    occurred_at: OffsetDateTime,
) -> StoreResult<bool> {
    Ok(conn.execute(
        "INSERT INTO task_pr_repair_incidents (task_pr_id, kind, occurred_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(task_pr_id, kind) DO NOTHING",
        params![pr_id.as_str(), kind.as_str(), occurred_at.unix_timestamp()],
    )? == 1)
}

fn update_task_pr(conn: &Connection, pr: &TaskPr) -> StoreResult<usize> {
    validate_task_pr(pr)?;
    let publication = pr.publication.as_ref();
    let presentation = publication.and_then(|publication| publication.presentation.as_ref());
    let github = publication.and_then(|publication| publication.github.as_ref());
    let merge = publication.and_then(|publication| publication.merge.as_ref());
    conn.execute(
        "UPDATE task_prs SET
            publication_requested_at=?7, after_merge=?8, next_slug=?9,
            github_number=?10, github_url=?11, merge_commit=?12,
            abandoned_at=?13, updated_at=?15, github_head_sha=?16,
            ci_observation=?17, parent_pr_id=?18, github_observation=?19,
            linear_attachment_id=?20, linear_comment_id=?21, linear_link_error=?22,
            merge_mode=?23, merge_requested_at=?24, merge_head_sha=?25,
            pr_title=?26, pr_body=?27, pr_copy_head_sha=?28
         WHERE id=?1 AND task_id=?2 AND sequence=?3 AND slug=?4
           AND branch=?5 AND base_commit=?6 AND created_at=?14",
        params![
            pr.id.as_str(),
            pr.task_id.as_str(),
            i64::from(pr.sequence),
            pr.slug,
            pr.branch,
            pr.base_commit,
            publication.map(|publication| publication.requested_at.unix_timestamp()),
            merge.map(|request| request.after_merge.as_str()),
            merge.and_then(|request| request.next_slug.as_deref()),
            github.map(|github| i64::from(github.number)),
            github.map(|github| github.url.as_str()),
            pr.merge_commit,
            pr.abandoned_at.map(OffsetDateTime::unix_timestamp),
            pr.created_at.unix_timestamp(),
            pr.updated_at.unix_timestamp(),
            github.and_then(|github| github.head_sha.as_deref()),
            task_pr_ci_json(pr)?,
            pr.parent_pr_id.as_ref().map(TaskPrId::as_str),
            task_pr_github_observation_json(pr)?,
            pr.linear_attachment_id.as_deref(),
            pr.linear_comment_id.as_deref(),
            pr.linear_link_error.as_deref(),
            merge.map(|request| request.mode.as_str()),
            merge.map(|request| request.requested_at.unix_timestamp()),
            merge.map(|request| request.head_sha.as_str()),
            presentation.map(|copy| copy.title.as_str()),
            presentation.map(|copy| copy.body.as_str()),
            presentation.map(|copy| copy.head_sha.as_str()),
        ],
    )
    .map_err(StoreError::from)
}

/// Move a Task PR's `base_commit` range anchor forward. `base_commit` is part of
/// `update_task_pr`'s optimistic identity, so healing it needs a dedicated write
/// keyed on the row's true identity (id + task + sequence).
fn heal_task_pr_base(conn: &Connection, pr: &TaskPr) -> StoreResult<usize> {
    validate_task_pr(pr)?;
    conn.execute(
        "UPDATE task_prs SET base_commit=?4, updated_at=?5
         WHERE id=?1 AND task_id=?2 AND sequence=?3",
        params![
            pr.id.as_str(),
            pr.task_id.as_str(),
            i64::from(pr.sequence),
            pr.base_commit,
            pr.updated_at.unix_timestamp(),
        ],
    )
    .map_err(StoreError::from)
}

/// Serialize a Task PR's CI observation to JSON for the `ci_observation` column,
/// or `None` when the head has not been observed.
fn task_pr_ci_json(pr: &TaskPr) -> StoreResult<Option<String>> {
    pr.ci_observation
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(StoreError::from)
}

fn task_pr_github_observation_json(pr: &TaskPr) -> StoreResult<Option<String>> {
    pr.github_observation
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(StoreError::from)
}

fn task_pr_on(conn: &Connection, pr_id: &TaskPrId) -> StoreResult<Option<TaskPr>> {
    conn.query_row(TASK_PR_SELECT, params![pr_id.as_str()], map_task_pr_row)
        .optional()
        .map_err(StoreError::from)
}

fn active_task_pr_on(conn: &Connection, task_id: &TaskId) -> StoreResult<Option<TaskPr>> {
    let query = format!(
        "{TASK_PR_COLUMNS}
         WHERE task_id=?1 AND merge_commit IS NULL AND abandoned_at IS NULL"
    );
    conn.query_row(&query, [task_id.as_str()], map_task_pr_row)
        .optional()
        .map_err(StoreError::from)
}

fn settle_task_pr_on(conn: &Connection, settled: &TaskPr) -> StoreResult<()> {
    let current = task_pr_on(conn, &settled.id)?.ok_or(StoreError::NotFound)?;
    if current.is_settled() {
        if !same_task_pr(&current, settled) {
            return Err(StoreError::InvalidData(format!(
                "Task PR {} is already settled differently",
                settled.id
            )));
        }
    } else if update_task_pr(conn, settled)? == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

fn validate_task_pr_settlement(settled: &TaskPr, next: Option<&TaskPr>) -> StoreResult<()> {
    validate_task_pr(settled)?;
    if !settled.is_settled() {
        return Err(StoreError::InvalidData(
            "Task PR transition requires a settled PR".to_string(),
        ));
    }
    if let Some(next) = next {
        validate_task_pr(next)?;
        if next.task_id != settled.task_id
            || next.sequence != settled.sequence + 1
            || next.phase() != PrPhase::Working
        {
            return Err(StoreError::InvalidData(
                "next Task PR must be the following Working PR for the same Task".to_string(),
            ));
        }
    }
    Ok(())
}

fn settle_task_pr_in(
    conn: &Connection,
    settled: &TaskPr,
    next: Option<&TaskPr>,
) -> StoreResult<()> {
    settle_task_pr_on(conn, settled)?;
    let Some(next) = next else {
        return Ok(());
    };
    let query = format!("{TASK_PR_COLUMNS} WHERE task_id=?1 AND sequence=?2");
    let existing = conn
        .query_row(
            &query,
            params![next.task_id.as_str(), i64::from(next.sequence)],
            map_task_pr_row,
        )
        .optional()?;
    match existing {
        Some(existing) if same_task_pr(&existing, next) => Ok(()),
        Some(existing) => Err(StoreError::InvalidData(format!(
            "Task PR sequence {} already belongs to {}",
            next.sequence, existing.id
        ))),
        None => insert_task_pr(conn, next),
    }
}

fn settle_task_pr_merged_in(
    conn: &Connection,
    settled: &TaskPr,
    merged_at: Option<OffsetDateTime>,
) -> StoreResult<crate::store::TaskPrMergeEvidenceOutcome> {
    let has_authority_column: bool = conn.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM pragma_table_info('task_prs') WHERE name='merged_at'
         )",
        [],
        |row| row.get(0),
    )?;
    if !has_authority_column {
        settle_task_pr_in(conn, settled, None)?;
        return Ok(crate::store::TaskPrMergeEvidenceOutcome::SchemaUnavailable);
    }

    let accepted_at = conn.query_row(
        "SELECT merged_at FROM task_prs WHERE id=?1",
        [settled.id.as_str()],
        |row| row.get::<_, Option<i64>>(0),
    )?;
    let observed_at = merged_at.map(OffsetDateTime::unix_timestamp);
    let outcome = match (accepted_at, observed_at) {
        (None, Some(_)) => crate::store::TaskPrMergeEvidenceOutcome::Accepted,
        (Some(accepted), Some(observed)) if accepted == observed => {
            crate::store::TaskPrMergeEvidenceOutcome::Repeated
        }
        (Some(accepted), Some(_)) => crate::store::TaskPrMergeEvidenceOutcome::Conflict {
            accepted_at: accepted,
        },
        (_, None) => crate::store::TaskPrMergeEvidenceOutcome::Missing,
    };

    settle_task_pr_in(conn, settled, None)?;
    if let crate::store::TaskPrMergeEvidenceOutcome::Conflict { accepted_at } = outcome {
        let checked_at = settled
            .github_observation
            .as_ref()
            .map(|observation| observation.checked_at)
            .unwrap_or_else(OffsetDateTime::now_utc);
        let observation = GithubObservation {
            checked_at,
            result: crate::task::GithubObservationResult::Partial {
                reason: format!(
                    "GitHub merged_at conflicts with first accepted value {accepted_at}"
                ),
            },
        };
        conn.execute(
            "UPDATE task_prs SET github_observation=?2 WHERE id=?1",
            params![settled.id.as_str(), serde_json::to_string(&observation)?],
        )?;
    }
    if let Some(observed_at) = observed_at {
        conn.execute(
            "UPDATE task_prs SET merged_at=COALESCE(merged_at, ?2) WHERE id=?1",
            params![settled.id.as_str(), observed_at],
        )?;
    }
    Ok(outcome)
}

fn same_task_pr(left: &TaskPr, right: &TaskPr) -> bool {
    left.id == right.id
        && left.task_id == right.task_id
        && left.sequence == right.sequence
        && left.slug == right.slug
        && left.branch == right.branch
        && left.base_commit == right.base_commit
        && left.publication == right.publication
        && left.merge_commit == right.merge_commit
        && same_settle_instant(left.abandoned_at, right.abandoned_at)
}

/// The column stores unix seconds, so a settle re-presented with the same
/// wall-clock instant but nanosecond precision is the same settle, not a
/// conflicting one.
fn same_settle_instant(
    left: Option<time::OffsetDateTime>,
    right: Option<time::OffsetDateTime>,
) -> bool {
    left.map(time::OffsetDateTime::unix_timestamp)
        == right.map(time::OffsetDateTime::unix_timestamp)
}

fn invalid_column(
    index: usize,
    error: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(error))
}

pub(super) fn map_task_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let abandon_intent = match (
        row.get::<_, Option<i64>>(16)?,
        row.get::<_, Option<String>>(17)?,
    ) {
        (Some(requested_at), Some(reason)) => Some(AbandonIntent {
            requested_at: crate::store::rows::unix_to_datetime(requested_at),
            reason,
        }),
        _ => None,
    };
    Ok(Task {
        id: TaskId::from_raw(row.get::<_, String>(0)?),
        plan: TaskPlan {
            id: LinearIssueId::from_raw(row.get::<_, String>(1)?),
            identifier: row.get(2)?,
            title: row.get(3)?,
            description: row.get(4)?,
            pm_snapshot_synced_at: row.get(13)?,
        },
        pm_writeback: serde_json::from_str(&row.get::<_, String>(14)?)
            .map_err(|error| invalid_column(14, error))?,
        wave_id: row.get(5)?,
        project_id: ProjectId::from_raw(row.get::<_, String>(15)?),
        worktree: PathBuf::from(row.get::<_, String>(6)?),
        workspace_slug: row.get(7)?,
        lifecycle: TaskLifecyclePlan {
            first: TaskPhasePlan { flow: row.get(21)? },
            loop_: TaskPhasePlan { flow: row.get(18)? },
            finally: TaskPhasePlan { flow: row.get(22)? },
        },
        lifecycle_phase: TaskLifecyclePhase::from_storage_str(&row.get::<_, String>(23)?)
            .map_err(|error| invalid_column(23, error))?,
        phase_epoch: row.get::<_, i64>(24)? as u32,
        phase_cursor: row.get::<_, i64>(19)? as u32,
        phase_iteration: row.get::<_, i64>(20)? as u32,
        gate_cycle: row.get::<_, i64>(25)? as u32,
        gate_proposal: row
            .get::<_, Option<String>>(26)?
            .map(|json| serde_json::from_str(&json))
            .transpose()
            .map_err(|error| invalid_column(26, error))?,
        agent: row.get(8)?,
        provider: row.get(9)?,
        provider_session_id: row.get(10)?,
        abandon_intent,
        created_at: crate::store::rows::unix_to_datetime(row.get(11)?),
        updated_at: crate::store::rows::unix_to_datetime(row.get(12)?),
        // Runtime freshness is derived when reconciliation decides whether the
        // durable GitHub observation can be reused.
        observation: crate::task::Observation::NotRequired,
    })
}

fn map_task_pr_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskPr> {
    let publication_requested_at = row.get::<_, Option<i64>>(6)?;
    let after_merge = row
        .get::<_, Option<String>>(7)?
        .map(|value| value.parse())
        .transpose()
        .map_err(|error| invalid_column(7, error))?;
    let github_number = row.get::<_, Option<i64>>(9)?.map(|number| number as u32);
    let github_url = row.get::<_, Option<String>>(10)?;
    let github_head_sha = row.get::<_, Option<String>>(15)?;
    let ci_observation = row
        .get::<_, Option<String>>(16)?
        .map(|json| serde_json::from_str::<CiObservation>(&json))
        .transpose()
        .map_err(|error| invalid_column(16, error))?;
    let github_observation = row
        .get::<_, Option<String>>(18)?
        .map(|json| serde_json::from_str::<GithubObservation>(&json))
        .transpose()
        .map_err(|error| invalid_column(18, error))?;
    let merge = match (
        row.get::<_, Option<String>>(22)?,
        row.get::<_, Option<i64>>(23)?,
        row.get::<_, Option<String>>(24)?,
        after_merge,
    ) {
        (Some(mode), Some(requested_at), Some(head_sha), Some(after_merge)) => {
            Some(PrMergeRequest {
                mode: mode.parse().map_err(|error| invalid_column(22, error))?,
                requested_at: crate::store::rows::unix_to_datetime(requested_at),
                head_sha,
                after_merge,
                next_slug: row.get(8)?,
            })
        }
        (None, None, None, None) => None,
        _ => {
            return Err(invalid_column(
                22,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "PR merge request fields must all be present or absent",
                ),
            ))
        }
    };
    let presentation = match (
        row.get::<_, Option<String>>(25)?,
        row.get::<_, Option<String>>(26)?,
        row.get::<_, Option<String>>(27)?,
    ) {
        (Some(title), Some(body), Some(head_sha)) => Some(PrPresentation {
            title,
            body,
            head_sha,
        }),
        (None, None, None) => None,
        _ => {
            return Err(invalid_column(
                25,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "PR presentation fields must all be present or absent",
                ),
            ))
        }
    };
    let publication = match publication_requested_at {
        Some(requested_at) => Some(PrPublication {
            requested_at: crate::store::rows::unix_to_datetime(requested_at),
            presentation,
            github: match (github_number, github_url) {
                (Some(number), Some(url)) => Some(GithubPr {
                    number,
                    url,
                    head_sha: github_head_sha,
                }),
                (None, None) => None,
                _ => {
                    return Err(invalid_column(
                        9,
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "GitHub PR number and URL must both be present or absent",
                        ),
                    ))
                }
            },
            merge,
        }),
        None => None,
    };
    let pr = TaskPr {
        id: TaskPrId::from_raw(row.get::<_, String>(0)?),
        task_id: TaskId::from_raw(row.get::<_, String>(1)?),
        sequence: row.get::<_, i64>(2)? as u32,
        slug: row.get(3)?,
        branch: row.get(4)?,
        base_commit: row.get(5)?,
        parent_pr_id: row.get::<_, Option<String>>(17)?.map(TaskPrId::from_raw),
        publication,
        merge_commit: row.get(11)?,
        abandoned_at: row
            .get::<_, Option<i64>>(12)?
            .map(crate::store::rows::unix_to_datetime),
        ci_observation,
        github_observation,
        linear_attachment_id: row.get::<_, Option<String>>(19)?,
        linear_comment_id: row.get::<_, Option<String>>(20)?,
        linear_link_error: row.get::<_, Option<String>>(21)?,
        created_at: crate::store::rows::unix_to_datetime(row.get(13)?),
        updated_at: crate::store::rows::unix_to_datetime(row.get(14)?),
    };
    pr.validate_persisted()
        .map_err(|error| invalid_column(6, error))?;
    Ok(pr)
}

fn map_task_linear_observation_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<TaskLinearObservation> {
    let last_success_at = OffsetDateTime::from_unix_timestamp(row.get::<_, i64>(4)?)
        .map_err(|error| invalid_column(4, error))?;
    let updated_at = OffsetDateTime::from_unix_timestamp(row.get::<_, i64>(6)?)
        .map_err(|error| invalid_column(6, error))?;
    Ok(TaskLinearObservation {
        task_id: TaskId::from_raw(row.get::<_, String>(0)?),
        last_revision: row.get(1)?,
        last_title: row.get(2)?,
        last_description: row.get(3)?,
        last_success_at,
        degraded_reason: row.get(5)?,
        updated_at,
    })
}

fn map_task_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskEvent> {
    let kind_json: String = row.get(2)?;
    let kind: TaskEventKind =
        serde_json::from_str(&kind_json).map_err(|error| invalid_column(2, error))?;
    Ok(TaskEvent {
        id: row.get(0)?,
        task_id: TaskId::from_raw(row.get::<_, String>(1)?),
        kind,
        created_at: crate::store::rows::unix_to_datetime(row.get(3)?),
    })
}

fn task_events_after_in(
    conn: &Connection,
    task_id: &TaskId,
    cursor: i64,
) -> StoreResult<Vec<TaskEvent>> {
    let mut statement = conn.prepare(
        "SELECT id, task_id, kind_json, created_at
         FROM task_events WHERE task_id=?1 AND id>?2 ORDER BY id",
    )?;
    let rows = statement.query_map(params![task_id.as_str(), cursor], map_task_event_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

const PROJECT_INSERT: &str = "INSERT INTO projects (
    id, wave_id, external_project_id, project_slug, project_name,
    project_prompt_context, pm_snapshot_synced_at,
    iteration, observation_cursor, last_state_fingerprint,
    agent, provider, provider_session_id, abandon_requested_at, abandon_reason,
    created_at, updated_at
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
    ?11, ?12, ?13, ?14, ?15, ?16, ?17
)";
const PROJECT_COLUMNS: &str = "SELECT
    id, external_project_id, project_slug, project_name, project_prompt_context,
    wave_id, pm_snapshot_synced_at,
    iteration, observation_cursor, last_state_fingerprint, agent, provider,
    provider_session_id, abandon_requested_at, abandon_reason, created_at, updated_at
    FROM projects";
const PROJECT_SELECT: &str = "SELECT
    id, external_project_id, project_slug, project_name, project_prompt_context,
    wave_id, pm_snapshot_synced_at,
    iteration, observation_cursor, last_state_fingerprint, agent, provider,
    provider_session_id, abandon_requested_at, abandon_reason, created_at, updated_at
    FROM projects WHERE id=?1";
const PROJECT_UPDATE: &str = "UPDATE projects SET
    wave_id=?2, external_project_id=?3, project_slug=?4, project_name=?5,
    project_prompt_context=?6, pm_snapshot_synced_at=?7, agent=?11, provider=?12,
    provider_session_id=?13, abandon_requested_at=?14, abandon_reason=?15,
    created_at=?16, updated_at=?17
    WHERE id=?1";
const PROJECT_REOPEN_UPDATE: &str = "UPDATE projects SET
    wave_id=?2, external_project_id=?3, project_slug=?4, project_name=?5,
    project_prompt_context=?6, pm_snapshot_synced_at=?7, iteration=?8,
    observation_cursor=?9, last_state_fingerprint=?10, agent=?11, provider=?12,
    provider_session_id=?13, abandon_requested_at=?14, abandon_reason=?15,
    created_at=?16, updated_at=?17
    WHERE id=?1";
const PROJECT_RUN_UPDATE: &str = "UPDATE projects SET
    wave_id=?2, external_project_id=?3, project_slug=?4, project_name=?5,
    project_prompt_context=?6, pm_snapshot_synced_at=?7, iteration=?8,
    observation_cursor=?9, last_state_fingerprint=?10, agent=?11, provider=?12,
    provider_session_id=?13, abandon_requested_at=?14, abandon_reason=?15,
    created_at=?16, updated_at=?17
    WHERE id=?1";
fn project_params(project: &Project) -> Vec<Box<dyn ToSql>> {
    vec![
        Box::new(project.id.as_str().to_string()),
        Box::new(project.wave_id.clone()),
        Box::new(project.plan.id.as_str().to_string()),
        Box::new(project.plan.slug.clone()),
        Box::new(project.plan.name.clone()),
        Box::new(project.plan.prompt_context.clone()),
        Box::new(project.plan.pm_snapshot_synced_at),
        Box::new(i64::from(project.iteration)),
        Box::new(project.observation_cursor),
        Box::new(project.last_state_fingerprint.clone()),
        Box::new(project.agent.clone()),
        Box::new(project.provider.clone()),
        Box::new(project.provider_session_id.clone()),
        Box::new(
            project
                .abandon_intent
                .as_ref()
                .map(|intent| intent.requested_at.unix_timestamp()),
        ),
        Box::new(
            project
                .abandon_intent
                .as_ref()
                .map(|intent| intent.reason.clone()),
        ),
        Box::new(project.created_at.unix_timestamp()),
        Box::new(project.updated_at.unix_timestamp()),
    ]
}

fn project_control_params(project: &Project) -> Vec<Box<dyn ToSql>> {
    project_params(project)
}

fn update_project_for_run_in(
    conn: &Connection,
    project: &Project,
    lease: &RunContext,
) -> StoreResult<usize> {
    require_run_owns_child(conn, &ChildRef::Project(project.id.clone()), lease)?;
    let parameters = project_params(project);
    Ok(conn.execute(
        PROJECT_RUN_UPDATE,
        rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
    )?)
}

fn map_project_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    let abandon_intent = match (
        row.get::<_, Option<i64>>(13)?,
        row.get::<_, Option<String>>(14)?,
    ) {
        (Some(requested_at), Some(reason)) => Some(AbandonIntent {
            requested_at: crate::store::rows::unix_to_datetime(requested_at),
            reason,
        }),
        _ => None,
    };
    Ok(Project {
        id: ProjectId::from_raw(row.get::<_, String>(0)?),
        plan: ProjectPlan {
            id: LinearProjectId::from_raw(row.get::<_, String>(1)?),
            slug: row.get(2)?,
            name: row.get(3)?,
            prompt_context: row.get(4)?,
            pm_snapshot_synced_at: row.get(6)?,
        },
        wave_id: row.get(5)?,
        iteration: row.get::<_, i64>(7)? as u32,
        observation_cursor: row.get(8)?,
        last_state_fingerprint: row.get(9)?,
        agent: row.get(10)?,
        provider: row.get(11)?,
        provider_session_id: row.get(12)?,
        abandon_intent,
        created_at: crate::store::rows::unix_to_datetime(row.get(15)?),
        updated_at: crate::store::rows::unix_to_datetime(row.get(16)?),
    })
}

fn map_project_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectEvent> {
    let kind: ProjectEventKind = serde_json::from_str(&row.get::<_, String>(2)?)
        .map_err(|error| invalid_column(2, error))?;
    Ok(ProjectEvent {
        id: row.get(0)?,
        project_id: ProjectId::from_raw(row.get::<_, String>(1)?),
        kind,
        run_id: row
            .get::<_, Option<String>>(3)?
            .map(|id| crate::durable::RunId::parse(&id))
            .transpose()
            .map_err(|error| invalid_column(3, error))?,
        created_at: crate::store::rows::unix_to_datetime(row.get(4)?),
    })
}

fn recipient_columns(recipient: &ObservationRecipient) -> (&'static str, String) {
    match recipient {
        ObservationRecipient::Wave { wave_id } => ("wave", wave_id.as_str().to_string()),
        ObservationRecipient::Project { project_id } => {
            ("project", project_id.as_str().to_string())
        }
    }
}

fn child_columns(source: &ChildRef) -> (&'static str, String) {
    match source {
        ChildRef::Project(project_id) => ("project", project_id.as_str().to_string()),
        ChildRef::Task(task_id) => ("task", task_id.as_str().to_string()),
    }
}

pub(super) fn insert_task_event_in(
    conn: &Connection,
    task: &Task,
    kind: &TaskEventKind,
) -> StoreResult<TaskEvent> {
    let created_at = now_unix();
    conn.execute(
        "INSERT INTO task_events (task_id, kind_json, created_at) VALUES (?1, ?2, ?3)",
        params![task.id.as_str(), serde_json::to_string(kind)?, created_at],
    )?;
    let event_id = conn.last_insert_rowid();
    if kind.is_project_observable() {
        insert_observation(
            conn,
            &ObservationRecipient::Project {
                project_id: task.project_id.clone(),
            },
            &ChildRef::Task(task.id.clone()),
            event_id,
            &ChildEventPayload::Task {
                event: kind.clone(),
            },
            created_at,
        )?;
        if kind.is_root_wave_observable() {
            insert_observation(
                conn,
                &ObservationRecipient::Wave {
                    wave_id: task.wave_id.clone(),
                },
                &ChildRef::Task(task.id.clone()),
                event_id,
                &ChildEventPayload::Task {
                    event: kind.clone(),
                },
                created_at,
            )?;
        }
    }
    Ok(TaskEvent {
        id: event_id,
        task_id: task.id.clone(),
        kind: kind.clone(),
        created_at: crate::store::rows::unix_to_datetime(created_at),
    })
}

fn insert_project_event_in(
    conn: &Connection,
    project: &Project,
    kind: &ProjectEventKind,
    run_id: Option<&crate::durable::RunId>,
) -> StoreResult<ProjectEvent> {
    let created_at = now_unix();
    conn.execute(
        "INSERT INTO project_events (project_id, kind_json, run_id, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            project.id.as_str(),
            serde_json::to_string(kind)?,
            run_id.map(crate::durable::RunId::as_str),
            created_at
        ],
    )?;
    let event_id = conn.last_insert_rowid();
    if kind.is_wave_observable() {
        insert_observation(
            conn,
            &ObservationRecipient::Wave {
                wave_id: project.wave_id.clone(),
            },
            &ChildRef::Project(project.id.clone()),
            event_id,
            &ChildEventPayload::Project {
                event: kind.clone(),
            },
            created_at,
        )?;
    }
    Ok(ProjectEvent {
        id: event_id,
        project_id: project.id.clone(),
        kind: kind.clone(),
        run_id: run_id.cloned(),
        created_at: crate::store::rows::unix_to_datetime(created_at),
    })
}

fn insert_observation(
    conn: &Connection,
    recipient: &ObservationRecipient,
    source: &ChildRef,
    event_id: i64,
    payload: &ChildEventPayload,
    created_at: i64,
) -> StoreResult<()> {
    let (recipient_kind, recipient_id) = recipient_columns(recipient);
    let (source_kind, source_id) = child_columns(source);
    conn.execute(
        "INSERT OR IGNORE INTO observation_outbox (
            recipient_kind, recipient_id, source_kind, source_id,
            event_id, payload_json, created_at, delivered_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
        params![
            recipient_kind,
            recipient_id,
            source_kind,
            source_id,
            event_id,
            serde_json::to_string(payload)?,
            created_at,
        ],
    )?;
    Ok(())
}

fn map_observation_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ObservationOutboxRow> {
    let recipient_kind: String = row.get(1)?;
    let recipient_id: String = row.get(2)?;
    let recipient = match recipient_kind.as_str() {
        "wave" => ObservationRecipient::Wave {
            wave_id: WaveId::parse(&recipient_id).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
        },
        "project" => ObservationRecipient::Project {
            project_id: ProjectId::from_raw(recipient_id),
        },
        value => {
            return Err(invalid_column(
                1,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown observation recipient {value:?}"),
                ),
            ))
        }
    };
    let source_kind: String = row.get(3)?;
    let source_id: String = row.get(4)?;
    let source = match source_kind.as_str() {
        "project" => ChildRef::Project(ProjectId::from_raw(source_id)),
        "task" => ChildRef::Task(TaskId::from_raw(source_id)),
        value => {
            return Err(invalid_column(
                3,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown observation source {value:?}"),
                ),
            ))
        }
    };
    let payload: ChildEventPayload = serde_json::from_str(&row.get::<_, String>(6)?)
        .map_err(|error| invalid_column(6, error))?;
    Ok(ObservationOutboxRow {
        id: row.get(0)?,
        recipient,
        source,
        event_id: row.get(5)?,
        payload,
        delivered_at: row
            .get::<_, Option<i64>>(7)?
            .map(crate::store::rows::unix_to_datetime),
    })
}
