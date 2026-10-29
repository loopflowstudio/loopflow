//! SQLite persistence for Project and Tasks.

use super::planning_write;
use crate::work::edit::WorkValue as Edit;
use std::collections::HashMap;
use std::path::PathBuf;

// Product writes read before they write (for example, validating parent Work),
// so a deferred transaction has to upgrade its read lock to a write
// lock. Under WAL, SQLite fails that upgrade immediately rather than waiting —
// `busy_timeout` is never consulted, because waiting on an upgrade can deadlock
// two upgraders. Beginning IMMEDIATE takes the write lock up front, where
// `busy_timeout` does apply, so a second `lf` process queues instead of dying
// with `database is locked`.
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, TransactionBehavior};
use time::OffsetDateTime;

use crate::durable::{Author, TaskState};
use crate::id::WaveId;
use crate::linear::{LinearIssueId, LinearProjectId};
use crate::store::rows::now_unix;
use crate::store::{PlanningState, StoreError, StoreResult};
use crate::work::project::{Project, ProjectEvent, ProjectEventKind, ProjectId};
use crate::work::task::NewTask;
use crate::work::task::{
    CiObservation, GithubObservation, GithubPr, LinearObservationOutcome, LinearWritebackState,
    PrMergeRequest, PrPhase, PrPresentation, PrPublication, Task, TaskEvent, TaskEventKind, TaskId,
    TaskLinearObservation, TaskPr, TaskPrId, TaskPrRepairKind,
};

use super::{SqliteStore, TaskCheckout};

const BRANCH_TASK_IDS: &str = "SELECT id FROM tasks WHERE branch=?1
    UNION SELECT task_id FROM task_prs WHERE branch=?1";

fn task_creation_in(conn: &Connection, task: &TaskId) -> StoreResult<Option<NewTask>> {
    conn.query_row(
        "SELECT project_id,title,description,due_date,repository_id FROM task_creation_intents WHERE task_id=?1",
        [task.as_str()],
        |row| {
            Ok(NewTask {
                due_date: row.get(3)?,
                id: task.clone(),
                project_id: row.get::<_, Option<String>>(0)?.map(ProjectId::from_raw),
                repository_id: crate::durable::RepositoryId::parse(&row.get::<_, String>(4)?).map_err(|error| invalid_column(4, error))?,
                title: row.get(1)?,
                description: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(StoreError::from)
}

// Shared creation writer. Callers either admit a new selected-Project request or
// recover a durable filing reservation in its original Project.
pub(super) fn create_task_in(conn: &Connection, input: &NewTask) -> StoreResult<Task> {
    if let Some(intent) = task_creation_in(conn, &input.id)? {
        if intent != *input {
            return Err(StoreError::InvalidData(
                "creation identity already belongs to a different request".into(),
            ));
        }
        return task_on(conn, &input.id)?.ok_or(StoreError::NotFound);
    }
    if input.title.trim().is_empty() {
        return Err(StoreError::InvalidData("Task title cannot be empty".into()));
    }
    let project = input.project_id.as_ref().map(ProjectId::as_str);
    let wave_id: Option<WaveId> = match project {
        Some(project) => conn.query_row(
            "SELECT wave_id FROM projects WHERE id=?1",
            [project],
            |row| row.get(0),
        )?,
        None => None,
    };
    let rank: u32 = conn.query_row(
        "SELECT COALESCE(max(planning_rank)+1,0) FROM tasks WHERE repository_id=?1 AND project_id IS ?2",
        params![input.repository_id.as_str(), project], |row| row.get(0),
    )?;
    let now = OffsetDateTime::now_utc();
    let task = Task {
        id: input.id.clone(),

        revision: 0,
        linear_id: None,
        identifier: format!("lf-{}", input.id.as_str().trim_start_matches("task_")),
        title: input.title.clone(),
        description: input.description.clone(),
        pm_snapshot_synced_at: None,

        pm_writeback: LinearWritebackState::Current,
        wave_id,
        repository_id: input.repository_id.clone(),
        project_id: input.project_id.clone(),
        worktree: None,
        workspace_slug: String::new(),
        branch: String::new(),
        base_commit: String::new(),
        parent_pr_id: None,
        agent: None,
        created_at: now,
        updated_at: now,
        rank,
        assignee: None,
        due_date: input.due_date.clone(),
        regressed_in: None,
        state: Some("unstarted".into()),
        completed_at: None,
        observation: crate::work::task::Observation::NotRequired,
    };
    validate_task(&task)?;
    insert_task_row(conn, &task)?;
    conn.execute(
        "INSERT INTO task_creation_intents(task_id, project_id, title, description, due_date, repository_id)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            input.id.as_str(),
            input.project_id.as_ref().map(ProjectId::as_str),
            input.title,
            input.description,
            input.due_date,
            input.repository_id.as_str()
        ],
    )?;
    task_on(conn, &input.id)?.ok_or(StoreError::NotFound)
}

/// Git planning carries the Task's branch, so whoever runs it next checks out
/// the same branch and finds the same PR. Remote providers supply their branch
/// through planning acquisition. An already-recorded branch writes nothing.
fn record_task_branch_in(tx: &Connection, task: &TaskId, branch: &str) -> StoreResult<()> {
    let (repo, recorded): (String, Option<String>) = tx.query_row(
        "SELECT w.repo,t.planning_branch_name FROM tasks t
         JOIN repository_plans w ON w.id=t.repository_id WHERE t.id=?1",
        [task.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if recorded.as_deref() == Some(branch) || crate::ops::planning_client::connected(&repo) {
        return Ok(());
    }
    super::planning_write::local(
        tx,
        &crate::work::edit::WorkEdit::for_target(
            task.as_str(),
            &[crate::work::edit::WorkValue::TaskBranch(Some(
                branch.into(),
            ))],
        ),
    )?;
    Ok(())
}

impl SqliteStore {
    /// Apply the placement transaction's admission rule before preparing Git work.
    pub fn validate_task_planning(&self, task: &Task) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        require_task_planning(&conn, task)
    }

    pub(crate) fn task_deleted(&self, task: &Task) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        task_deleted_on(&conn, task)
    }

    pub fn place_task(
        &self,
        task_id: &TaskId,
        worktree: &std::path::Path,
        workspace_slug: &str,
        pr: &TaskPr,
    ) -> StoreResult<Task> {
        let _admission = self.lock_checkout(worktree)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut task = task_on(&tx, task_id)?.ok_or(StoreError::NotFound)?;
        if task.worktree.is_some() {
            return Ok(task);
        }
        if super::durable::task_state_in(&tx, task_id)?.is_terminal() {
            return Err(StoreError::InvalidAuthority(
                "terminal Task cannot allocate a checkout".into(),
            ));
        }
        if let Some(project) = &task.project_id {
            super::durable::require_selected_project(&tx, project)?;
        }
        require_task_planning(&tx, &task)?;
        // Allocating a checkout is the actual work reservation: it claims an
        // unowned Task, and refuses one owned on another Machine.
        super::task_ownership::claim_in(&tx, task_id)?;
        record_task_branch_in(&tx, task_id, &pr.branch)?;
        let local = super::durable::map_local_machine(&tx)?.id;
        task.worktree = Some(worktree.to_path_buf());
        task.workspace_slug = workspace_slug.to_string();
        task.branch = pr.branch.clone();
        task.base_commit = pr.base_commit.clone();
        task.parent_pr_id = pr.parent_pr_id.clone();
        validate_initial_task_pr(&task, pr)?;
        tx.execute(
            "UPDATE tasks SET worktree=?2,workspace_slug=?3,updated_at=?4,branch=?5,base_commit=?6,parent_pr_id=?7,checkout_machine_id=?8 WHERE id=?1",
            params![
                task_id.as_str(),
                worktree.display().to_string(),
                workspace_slug,
                now_unix(), task.branch, task.base_commit, task.parent_pr_id.as_ref().map(TaskPrId::as_str), local.as_str()
            ],
        )?;
        if pr.publication.is_some() {
            insert_task_pr(&tx, pr)?;
        }
        seed_task_linear_observation(&tx, &task)?;
        insert_task_event_in(
            &tx,
            task_id,
            &TaskEventKind::CheckoutInitializing {
                branch: pr.branch.clone(),
                path: worktree.display().to_string(),
                base_commit: pr.base_commit.clone(),
            },
        )?;
        let task = task_on(&tx, task_id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(task)
    }

    pub fn create_task(&self, input: &NewTask) -> StoreResult<Task> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if task_creation_in(&tx, &input.id)?.is_none() {
            if let Some(project) = &input.project_id {
                super::durable::require_selected_project(&tx, project)?;
            }
        }
        let task = create_task_in(&tx, input)?;
        tx.commit()?;
        Ok(task)
    }

    pub(crate) fn task_checkout(&self, task: &TaskId) -> StoreResult<Option<TaskCheckout>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        task_checkout_in(&conn, task)
    }

    pub(crate) fn task_checkouts(&self) -> StoreResult<Vec<TaskCheckout>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        let identifiers = TaskIdentifiers::read(&conn)?;
        let mut statement = conn.prepare(TASK_CHECKOUT_COLUMNS)?;
        let rows = statement.query_map([], |row| {
            map_task_checkout(row, |id| Ok(identifiers.short(id)))
        })?;
        rows.map(|row| row.map_err(StoreError::from)).collect()
    }

    pub fn set_task_agent(&self, task_id: &TaskId, agent: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE tasks SET agent=?2, updated_at=?3 WHERE id=?1",
            params![task_id.as_str(), agent, now_unix()],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub(crate) fn request_task_completion(
        &self,
        task: &TaskId,
        note: Option<&str>,
    ) -> StoreResult<Option<i64>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let request = request_completion_in(&tx, task, note)?;
        tx.commit()?;
        Ok(request)
    }

    pub(crate) fn task_completion_pending(
        &self,
        task: &TaskId,
    ) -> StoreResult<Option<(i64, String)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT completion_request,COALESCE(completion_error,'Completion requested') FROM tasks WHERE id=?1 AND completion_request IS NOT NULL",
            [task.as_str()], |row| Ok((row.get(0)?,row.get(1)?))).optional()?)
    }

    /// Whether the Task completed at or after `since` (Unix seconds), even if
    /// it was reopened afterwards.
    pub(crate) fn task_completed_since(&self, task: &TaskId, since: i64) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_events WHERE task_id=?1
             AND json_extract(kind_json,'$.kind')='completed' AND created_at>=?2)",
            params![task.as_str(), since],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn fail_task_completion(
        &self,
        task: &TaskId,
        request: i64,
        error: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE tasks SET completion_error=?3 WHERE id=?1 AND completion_request=?2",
            params![task.as_str(), request, error],
        )?;
        Ok(())
    }

    /// Settle one completion request without changing execution.
    pub(crate) fn complete_task(&self, task: &Task, request: i64) -> StoreResult<bool> {
        validate_task(task)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_task_project(&transaction, task)?;
        let state = super::durable::task_state_in(&transaction, &task.id)?;
        if state == TaskState::Done {
            return Ok(true);
        }
        let current_request: Option<i64> = transaction.query_row(
            "SELECT completion_request FROM tasks WHERE id=?1",
            [task.id.as_str()],
            |row| row.get(0),
        )?;
        if current_request != Some(request) {
            return Ok(false);
        }
        let current = active_task_pr_on(&transaction, &task.id)?;
        if current.is_some_and(|pr| pr.phase() != PrPhase::Merged) {
            return Err(StoreError::InvalidAuthority(
                "Task requires a verified merge".into(),
            ));
        }
        if state == TaskState::Abandoned {
            return Err(StoreError::InvalidData(format!(
                "Task {} is abandoned and cannot be completed",
                task.id
            )));
        }
        transaction.execute(
            "UPDATE tasks SET completion_request=NULL,completion_error=NULL WHERE id=?1",
            params![task.id.as_str()],
        )?;
        super::task_state_delivery::queue_in(&transaction, &task.id, "completed")?;
        insert_task_event_in(
            &transaction,
            &task.id,
            &TaskEventKind::Completed {
                summary: "Task completed".into(),
            },
        )?;
        transaction.commit()?;
        Ok(true)
    }

    /// Change the explicit planning hold, never execution or Workflow position.
    pub(crate) fn set_task_deferred(&self, id: &TaskId, deferred: bool) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task = task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        require_task_not_deleted(&tx, &task)?;
        if super::durable::task_state_in(&tx, id)?.is_terminal() {
            return Err(StoreError::InvalidAuthority(
                "Reopen the Task before deferring or resuming it".into(),
            ));
        }
        super::planning_write::local(
            &tx,
            &crate::work::edit::WorkEdit::for_target(
                id.as_str(),
                &[crate::work::edit::WorkValue::TaskDeferred(deferred)],
            ),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Reopen planning without touching Workflow, placement, PR or Process history.
    pub(crate) fn reopen_task(&self, id: &TaskId, note: Option<&str>) -> StoreResult<Task> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task = task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        require_task_not_deleted(&tx, &task)?;
        if super::durable::task_state_in(&tx, id)? == TaskState::Abandoned {
            return Err(StoreError::InvalidAuthority(
                "An abandoned Task cannot be reopened".into(),
            ));
        }
        let (completed, pending): (bool, bool) = tx.query_row(
            "SELECT planning_completed,completion_request IS NOT NULL FROM tasks WHERE id=?1",
            [id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if completed || pending {
            tx.execute(
                "UPDATE tasks SET completion_request=NULL,completion_error=NULL WHERE id=?1",
                [id.as_str()],
            )?;
            super::task_state_delivery::queue_in(&tx, id, "unstarted")?;
            // Reopening withdraws Auto-Ship: the approval ended with the Task.
            super::task_work::unship_in(&tx, id)?;
            insert_task_event_in(
                &tx,
                id,
                &TaskEventKind::Progress {
                    summary: note.map_or_else(
                        || "Task reopened; Workflow unchanged".into(),
                        |note| format!("Task reopened; Workflow unchanged: {note}"),
                    ),
                },
            )?;
        }
        let task = task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(task)
    }

    pub fn task_acquisition(&self, id: &TaskId) -> StoreResult<crate::store::TaskAcquisition> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let task = task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        let result = task_acquisition_in(&tx, &task, true)?;
        tx.commit()?;
        Ok(result)
    }

    pub fn task(&self, task_id: &TaskId) -> StoreResult<Option<Task>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        task_on(&conn, task_id)
    }

    pub(crate) fn task_issue_url(&self, id: &TaskId) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT planning_url FROM tasks WHERE id=?1",
            [id.as_str()],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn task_issue_branch(&self, id: &TaskId) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT planning_branch_name FROM tasks WHERE id=?1",
            [id.as_str()],
            |row| row.get(0),
        )?)
    }

    pub fn task_by_issue(&self, issue: &str) -> StoreResult<Option<Task>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        match resolve_task_id_in(&snapshot, issue, None)? {
            Some(id) => task_on(&snapshot, &id),
            None => Ok(None),
        }
    }

    /// Resolve identity without requiring Project metadata or checkout placement.
    pub(crate) fn resolve_task_id(
        &self,
        issue: &str,
        repo: Option<&str>,
    ) -> StoreResult<Option<TaskId>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        resolve_task_id_in(&snapshot, issue, repo)
    }

    pub fn task_by_branch(&self, branch: &str) -> StoreResult<Option<Task>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        let query = format!("{TASK_COLUMNS} WHERE t.branch=?1");
        let mut statement = conn.prepare(&query)?;
        let tasks = statement
            .query_map(params![branch], |row| map_task_row(&conn, row))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        resolve_current_task(branch, tasks)
    }

    /// Match current checkout or retained delivery branches without hydrating unrelated history.
    pub(crate) fn historical_branch_task(&self, branch: &str) -> StoreResult<Option<Task>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let ids = snapshot
            .prepare(BRANCH_TASK_IDS)?
            .query_map([branch], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        match ids.as_slice() {
            [] => Ok(None),
            [id] => task_on(
                &snapshot,
                &TaskId::parse(id).map_err(|error| StoreError::InvalidData(error.to_string()))?,
            ),
            _ => Err(StoreError::InvalidData(format!(
                "branch {branch:?} belongs to multiple Tasks; use an issue ID"
            ))),
        }
    }

    pub fn list_tasks(&self, wave_id: Option<&WaveId>) -> StoreResult<Vec<Task>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        tasks_in(&conn, wave_id)
    }

    /// One repository's regression Tasks, open or done; abandoned ones covered nothing.
    pub fn regression_tasks(&self, repo: &str) -> StoreResult<Vec<Task>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        let query = format!(
            "{TASK_COLUMNS} WHERE t.repository_id=(SELECT id FROM repository_plans WHERE repo=?1)
             AND t.regressed_in IS NOT NULL AND t.abandoned_at IS NULL AND {TASK_VISIBLE}"
        );
        let identifiers = TaskIdentifiers::read(&conn)?;
        let mut statement = conn.prepare(&query)?;
        let tasks = statement
            .query_map([repo], |row| map_listed_task_row(&identifiers, row))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tasks)
    }

    /// Tasks that are neither done nor abandoned: the live plan.
    pub fn list_open_tasks(&self, wave_id: Option<&WaveId>) -> StoreResult<Vec<Task>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        tasks_where(&conn, wave_id, &super::durable::task_open_sql("t"))
    }

    /// Closed Tasks of one repository (and Wave), newest first, `limit` at a
    /// time, back to those closed at `since` (RFC 3339, UTC, whole seconds).
    pub fn closed_tasks_page(
        &self,
        repository: &crate::durable::RepositoryId,
        wave_id: Option<&WaveId>,
        since: Option<&str>,
        after: Option<&TaskId>,
        limit: usize,
    ) -> StoreResult<ClosedTaskPage> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        let open = super::durable::task_open_sql("t");
        let query = format!(
            "{TASK_COLUMNS} WHERE t.repository_id=?1 AND (?2 IS NULL OR p.wave_id=?2)
             AND NOT {open} AND {TASK_VISIBLE}
             AND (?3 IS NULL OR ({TASK_CLOSED_AT},t.id) < (
                SELECT {TASK_CLOSED_AT},t.id FROM tasks t WHERE t.id=?3))
             AND (?5 IS NULL OR {TASK_CLOSED_AT}>=?5)
             ORDER BY {TASK_CLOSED_AT} DESC, t.id DESC LIMIT ?4"
        );
        let identifiers = TaskIdentifiers::read(&conn)?;
        let mut statement = conn.prepare(&query)?;
        let mut tasks = statement
            .query_map(
                params![
                    repository.as_str(),
                    wave_id.map(WaveId::as_str),
                    after.map(TaskId::as_str),
                    limit as i64 + 1,
                    since
                ],
                |row| map_listed_task_row(&identifiers, row),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let next = (tasks.len() > limit).then(|| {
            tasks.truncate(limit);
            tasks
                .last()
                .expect("a full page has a last Task")
                .id
                .clone()
        });
        Ok(ClosedTaskPage { tasks, next })
    }

    /// Open Tasks of one repository whose title or identifier contains `text`,
    /// ignoring case, optionally within one Wave and one Project (its ID,
    /// provider ID, slug or name).
    pub fn open_tasks_matching(
        &self,
        repository: &crate::durable::RepositoryId,
        text: &str,
        wave_id: Option<&WaveId>,
        project: Option<&str>,
    ) -> StoreResult<Vec<Task>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        let open = super::durable::task_open_sql("t");
        let query = format!(
            "{TASK_COLUMNS} WHERE t.repository_id=?1 AND {open} AND {TASK_VISIBLE}
             AND (instr(lower(t.issue_title),?2) OR instr(lower(t.issue_identifier),?2)
                OR instr(lower(t.id),?2))
             AND (?3 IS NULL OR p.wave_id=?3)
             AND (?4 IS NULL OR ?4 IN (p.id,p.external_project_id,p.project_slug)
                OR lower(p.project_name)=lower(?4))"
        );
        let identifiers = TaskIdentifiers::read(&conn)?;
        let mut statement = conn.prepare(&query)?;
        let tasks = statement
            .query_map(
                params![
                    repository.as_str(),
                    text.to_lowercase(),
                    wave_id.map(WaveId::as_str),
                    project
                ],
                |row| map_listed_task_row(&identifiers, row),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tasks)
    }

    /// How many Tasks of a Wave are open, in one statement.
    pub fn open_task_count(&self, wave_id: &WaveId) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let open = super::durable::task_open_sql("t");
        Ok(conn.query_row(
            &format!(
                "SELECT count(*) FROM tasks t LEFT JOIN projects p ON p.id=t.project_id
                 WHERE p.wave_id=?1 AND {open} AND {TASK_VISIBLE}"
            ),
            [wave_id.as_str()],
            |row| row.get(0),
        )?)
    }

    pub fn sync_task_placement(
        &self,
        task_id: &TaskId,
        new_base: &str,
        clear_parent: bool,
        updated_at: OffsetDateTime,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE tasks SET base_commit=?2, parent_pr_id=CASE WHEN ?3 THEN NULL ELSE parent_pr_id END, updated_at=?4 WHERE id=?1",
            params![task_id.as_str(), new_base, clear_parent, updated_at.unix_timestamp()],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Select a dependency without claiming that Git or GitHub has moved.
    pub fn stack_task_placement(&self, expected: &Task, parent_id: &TaskPrId) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let child = task_on(&tx, &expected.id)?.ok_or(StoreError::NotFound)?;
        if child.base_commit != expected.base_commit {
            return Err(StoreError::InvalidAuthority(
                "Task placement changed; retry preparation".into(),
            ));
        }
        if child.parent_pr_id.as_ref() == Some(parent_id) {
            return Ok(());
        }
        if child.parent_pr_id != expected.parent_pr_id {
            return Err(StoreError::InvalidAuthority(
                "Task parent changed; retry preparation".into(),
            ));
        }
        if active_task_pr_on(&tx, &child.id)?
            .as_ref()
            .and_then(|pr| pr.publication.as_ref())
            .is_some_and(|publication| publication.merge.is_some())
        {
            return Err(StoreError::InvalidAuthority(
                "Task PR has a merge request; cancel its delivery before selecting a parent".into(),
            ));
        }
        if super::durable::task_state_in(&tx, &child.id)?.is_terminal() {
            return Err(StoreError::InvalidAuthority(
                "Task must be ready before selecting a parent".into(),
            ));
        }
        let parent = task_pr_on(&tx, parent_id)?.ok_or(StoreError::NotFound)?;
        if parent.is_settled() || parent.github().is_none() {
            return Err(StoreError::InvalidAuthority(
                "open the parent PR before stacking work on it".into(),
            ));
        }
        let same_repo: bool = tx.query_row(
            "SELECT c.repository_id=p.repository_id FROM tasks c
             JOIN tasks p ON p.id=?2
             WHERE c.id=?1",
            params![child.id.as_str(), parent.task_id.as_str()],
            |row| row.get(0),
        )?;
        if !same_repo {
            return Err(StoreError::InvalidAuthority(
                "stack parent belongs to another repository".into(),
            ));
        }
        let mut ancestor = Some(parent);
        let mut seen = std::collections::HashSet::new();
        while let Some(pr) = ancestor {
            if pr.task_id == child.id || !seen.insert(pr.id.clone()) {
                return Err(StoreError::InvalidAuthority(
                    "stack parent would create a dependency cycle".into(),
                ));
            }
            ancestor = match pr.parent_pr_id {
                Some(id) => Some(task_pr_on(&tx, &id)?.ok_or(StoreError::NotFound)?),
                None => None,
            };
        }
        tx.execute(
            "UPDATE tasks SET parent_pr_id=?2, updated_at=?3 WHERE id=?1",
            params![child.id.as_str(), parent_id.as_str(), now_unix()],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn insert_task_pr(&self, pr: &TaskPr) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(previous) = active_task_pr_on(&tx, &pr.task_id)? {
            if previous.is_settled() && pr.sequence > previous.sequence {
                tx.execute(
                    "UPDATE task_prs SET historical=1 WHERE id=?1",
                    [previous.id.as_str()],
                )?;
                tx.execute(
                    "UPDATE tasks SET completion_request=NULL,completion_error=NULL WHERE id=?1",
                    [pr.task_id.as_str()],
                )?;
                // A new PR respects completion; only `lf task reopen` uncompletes.
            }
        }
        insert_task_pr(&tx, pr)?;
        tx.commit()?;
        Ok(())
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

    pub(crate) fn record_task_pr_repair_incident(
        &self,
        pr_id: &TaskPrId,
        kind: TaskPrRepairKind,
        occurred_at: OffsetDateTime,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        record_task_pr_repair_incident_on(&conn, pr_id, kind, occurred_at)
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

    pub fn settle_task_pr(&self, settled: &TaskPr) -> StoreResult<()> {
        validate_task_pr_settlement(settled)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        settle_task_pr_on(&transaction, settled)?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn settle_task_pr_merged(
        &self,
        settled: &TaskPr,
        merged_at: Option<OffsetDateTime>,
    ) -> StoreResult<crate::store::TaskPrMergeEvidenceOutcome> {
        validate_task_pr_settlement(settled)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let outcome = settle_task_pr_merged_in(&transaction, settled, merged_at)?;
        transaction.commit()?;
        Ok(outcome)
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
    /// comments are imported on the first read and deduplicated by revision.
    /// Issue revisions guard definition changes independently of comments.
    pub(crate) fn apply_linear_observation(
        &self,
        task_id: &TaskId,
        observation: &crate::linear::LinearIssueObservation,
        observed_at: OffsetDateTime,
    ) -> StoreResult<LinearObservationOutcome> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let existing = transaction
            .query_row(
                "SELECT last_revision, last_title, last_description
                 FROM task_linear_observations WHERE task_id=?1",
                params![task_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;

        let observed_at = observed_at.unix_timestamp();
        let mut follow_ups_created = Vec::new();
        for comment in &observation.comments {
            if !super::task_comments::ingest_task_comment(&transaction, task_id, comment)? {
                continue;
            }
            if !crate::ops::linear_observe::is_direction_comment(
                &comment.body,
                comment.author_id.as_deref(),
            ) {
                continue;
            }
            let comment_id = crate::ops::linear_observe::comment_revision_id(
                &comment.id,
                comment.revision.as_deref(),
            );
            let text = crate::ops::linear_observe::render_comment(
                &comment.id,
                &comment.body,
                comment.author_id.as_deref(),
                comment.author_name.as_deref(),
            );
            if let Some(id) = ingest_linear_comment(
                &transaction,
                task_id.as_str(),
                &comment_id,
                comment.author_name.as_deref(),
                &text,
                observed_at,
            )? {
                follow_ups_created.push(id);
            }
        }
        super::task_ownership::reassert_in(&transaction, task_id)?;

        let Some((last_revision, last_title, last_description)) = existing else {
            // Baseline the definition; existing comments were imported above.
            transaction.execute(
                "INSERT INTO task_linear_observations (
                    task_id, last_revision, last_title, last_description,
                    last_success_at, degraded_reason, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?5)",
                params![
                    task_id.as_str(),
                    observation.revision,
                    observation.title,
                    observation.description,
                    observed_at,
                ],
            )?;
            transaction.commit()?;
            return Ok(LinearObservationOutcome {
                baselined: true,
                content_steer_applied: false,
                follow_ups_created,
            });
        };

        // Monotonic guard: an out-of-order response older than what we have
        // carries stale content, so drop it rather than let it revert direction.
        if observation.revision.as_str() < last_revision.as_str() {
            transaction.commit()?;
            return Ok(LinearObservationOutcome {
                baselined: false,
                content_steer_applied: false,
                follow_ups_created,
            });
        }

        let content_steer_applied =
            last_title != observation.title || last_description != observation.description;
        if content_steer_applied {
            let text = format!(
                "The linked planning task was edited; use this current definition.\n\n\
                 Title: {}\n\n{}",
                observation.title, observation.description,
            );
            Self::save_task_steer_in(
                &transaction,
                task_id,
                &Author::User,
                &text,
                &format!("task-definition:{task_id}"),
                &observation.revision,
            )?;
        }

        transaction.execute(
            "UPDATE task_linear_observations
             SET last_revision=?2, last_title=?3, last_description=?4,
                 last_success_at=?5, degraded_reason=NULL, updated_at=?5
             WHERE task_id=?1",
            params![
                task_id.as_str(),
                observation.revision,
                observation.title,
                observation.description,
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
        let event = insert_task_event_in(&transaction, task_id, kind)?;
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
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        planning_write::create(
            &transaction,
            "",
            &crate::work::edit::WorkEdit::for_target(
                project.id.as_str(),
                &[
                    Edit::ProjectWave(project.wave_id.to_string()),
                    Edit::ProjectLinearId(
                        project.linear_id.as_ref().map(|id| id.as_str().to_owned()),
                    ),
                    Edit::ProjectSlug(Some(project.slug.clone())),
                    Edit::ProjectName(Some(project.name.clone())),
                    Edit::ProjectSummary(project.summary.clone()),
                    Edit::ProjectDescription(Some(project.description.clone())),
                    Edit::ProjectStatus(project.status),
                    Edit::Krs(project.krs.clone()),
                    Edit::MetricTargets(project.metric_targets.clone()),
                ],
            ),
        )?;
        transaction.execute(
            "UPDATE projects SET pm_snapshot_synced_at=?2,created_at=?3,updated_at=?4 WHERE id=?1",
            params![
                project.id.as_str(),
                project.pm_snapshot_synced_at,
                project.created_at.unix_timestamp(),
                project.updated_at.unix_timestamp()
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn project(&self, project_id: &ProjectId) -> StoreResult<Option<Project>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        project_on(&conn, project_id)
    }

    pub fn project_by_project(&self, project: &str) -> StoreResult<Option<Project>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        if let Ok(project_id) = ProjectId::parse(project) {
            if let Some(project) = project_on(&snapshot, &project_id)? {
                return Ok(Some(project));
            }
        }
        let query = format!(
            "{PROJECT_COLUMNS}
             WHERE external_project_id=?2 OR project_slug=?1
             ORDER BY (external_project_id=?2) DESC, created_at DESC, id DESC
             LIMIT 1"
        );
        snapshot
            .query_row(
                &query,
                params![project, crate::id::canonical_uuid(project)],
                map_project_row,
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn list_projects(&self, wave_id: Option<&WaveId>) -> StoreResult<Vec<Project>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = match wave_id {
            Some(_) => {
                format!("{PROJECT_COLUMNS} WHERE wave_id=?1 ORDER BY planning_rank,created_at,id")
            }
            None => format!("{PROJECT_COLUMNS} ORDER BY wave_id,planning_rank,created_at,id"),
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
        let event = insert_project_event_in(&transaction, &project, kind)?;
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
            "SELECT id, project_id, kind_json, created_at
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
            "SELECT id, project_id, kind_json, created_at
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
    ) -> StoreResult<Option<crate::work::project::HistoricalFailure>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let event = conn
            .query_row(
                "SELECT id, project_id, kind_json, created_at
                 FROM project_events
                 WHERE project_id=?1 AND json_extract(kind_json, '$.kind')='failed'
                 ORDER BY id DESC LIMIT 1",
                params![project_id.as_str()],
                map_project_event_row,
            )
            .optional()?;
        Ok(event
            .as_ref()
            .and_then(crate::work::project::HistoricalFailure::from_event))
    }
}

fn validate_task(task: &Task) -> StoreResult<()> {
    task.validate()
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn resolve_current_task(key: &str, mut tasks: Vec<Task>) -> StoreResult<Option<Task>> {
    if tasks.len() > 1 {
        return Err(StoreError::InvalidData(format!(
            "multiple stable Tasks resolve to {key:?}"
        )));
    }
    Ok(tasks.pop())
}

fn validate_task_pr(pr: &TaskPr) -> StoreResult<()> {
    pr.validate()
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn validate_initial_task_pr(task: &Task, pr: &TaskPr) -> StoreResult<()> {
    validate_task_pr(pr)?;
    if pr.task_id != task.id || pr.sequence != 1 || !pr.is_active() {
        return Err(StoreError::InvalidData(
            "Task requires its sequence-1 active PR".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn require_task_not_deleted(conn: &Connection, task: &Task) -> StoreResult<()> {
    if task_deleted_on(conn, task)? {
        return Err(StoreError::InvalidAuthority(format!(
            "Task {} was deleted; create a new Task",
            task.identifier
        )));
    }
    Ok(())
}

/// Saved planning admits work without acquisition. Retained contrary provider
/// evidence still applies; missing inventory cannot erase the saved Task.
pub(super) fn require_task_planning(conn: &Connection, task: &Task) -> StoreResult<()> {
    require_task_planning_identity(conn, task)?;
    let state: Option<String> = conn.query_row(
        "SELECT COALESCE(planning_state,CASE WHEN planning_completed=1 THEN 'completed' END) FROM tasks WHERE id=?1",
        [task.id.as_str()],
        |row| row.get(0),
    )?;
    // Upstream cancellation stops a Task that hasn't set out; one under way keeps going.
    if crate::linear::terminal_reason(state.as_deref()).is_some() && !under_way(conn, task)? {
        return Err(StoreError::InvalidAuthority(
            "terminal planning state cannot start work; its execution history is preserved".into(),
        ));
    }
    Ok(())
}

/// Open, not deferred, and past its Graph's `start`.
fn under_way(conn: &Connection, task: &Task) -> StoreResult<bool> {
    Ok(!matches!(
        super::durable::task_state_in(conn, &task.id)?,
        TaskState::Deferred | TaskState::Done | TaskState::Abandoned
    ) && conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM task_graphs WHERE task_id=?1 \
             AND NOT (step='start' AND lf_process_id IS NULL))",
        [task.id.as_str()],
        |row| row.get(0),
    )?)
}

/// Planning ownership remains relevant when finishing already completed work.
pub(super) fn require_task_planning_identity(conn: &Connection, task: &Task) -> StoreResult<()> {
    require_task_not_deleted(conn, task)?;
    let acquisition = task_acquisition_in(conn, task, false)?;
    if matches!(
        acquisition.state,
        PlanningState::Invalid | PlanningState::Removed
    ) {
        return Err(StoreError::InvalidAuthority(
            "Task planning has invalid ownership, invalidation or removal evidence; refresh its planning".into(),
        ));
    }
    Ok(())
}

// Read only the retained facts that affect admission. Content belongs to Work;
// an unrelated malformed provider content field cannot erase membership or age.
fn task_acquisition_in(
    conn: &Connection,
    task: &Task,
    allow_pending_membership: bool,
) -> StoreResult<crate::store::TaskAcquisition> {
    task_acquisitions_in(conn, &[task], allow_pending_membership)?
        .remove(&task.id)
        .ok_or(StoreError::NotFound)
}

/// Each Task's planning acquisition, in one statement.
pub(super) fn task_acquisitions_in(
    conn: &Connection,
    tasks: &[&Task],
    allow_pending_membership: bool,
) -> StoreResult<HashMap<TaskId, crate::store::TaskAcquisition>> {
    let ids = ids_json(tasks.iter().map(|task| task.id.as_str()));
    let linear: std::collections::HashSet<&TaskId> = tasks
        .iter()
        .filter(|task| task.linear_id.is_some())
        .map(|task| &task.id)
        .collect();
    let mut statement = conn.prepare(
            "SELECT t.id,i.observed_at,
         COALESCE(i.needs_refresh,0) OR COALESCE(observed.membership_unresolved,0)
         OR COALESCE(observed.archived,0)
         OR COALESCE(parent.membership_unresolved,0) OR COALESCE(parent.archived,0)
         OR CASE WHEN i.id IS NULL THEN 0 ELSE
             -- A pending membership move is sent last, so it wins over the
             -- observed Project and its team until delivery.
             (NOT (?2 AND EXISTS(
                 SELECT 1 FROM task_changes c WHERE c.task_id=t.id AND c.field='task_project'
                 AND c.acknowledged=0 AND c.conflict_json IS NULL
                 AND c.seq=(SELECT max(seq) FROM task_changes WHERE task_id=t.id AND field=c.field)))
              AND (i.project_id IS NOT p.external_project_id
                   OR CASE WHEN p.id IS NOT NULL AND i.team_id IS NOT NULL THEN
                          NOT EXISTS(SELECT 1 FROM json_each(p.planning_teams) WHERE value=i.team_id)
                      ELSE 0 END))
             OR NOT i.project_id_observed OR i.claimed_project_id IS NOT i.project_id
             OR CASE WHEN observed.id IS NULL THEN 0
                WHEN NOT json_valid(observed.team_ids) THEN 1
                WHEN json_type(observed.team_ids) IS NOT 'array' THEN 1
                ELSE NOT EXISTS(SELECT 1 FROM json_each(observed.team_ids)
                                WHERE value=i.team_id) END
             END,
         t.planning_deleted_at IS NOT NULL
         OR EXISTS(SELECT 1 FROM task_deletions d JOIN waves dw ON dw.id=d.wave_id
                   WHERE dw.repo=w.repo AND d.issue_id=t.external_issue_id)
         OR COALESCE(changes.removed,0),
         changes.issue_id IS NOT NULL
         FROM tasks t JOIN repository_plans w ON w.id=t.repository_id LEFT JOIN projects p ON p.id=t.project_id
         LEFT JOIN pm_items i ON i.repo=w.repo AND i.provider='linear' AND i.id=t.external_issue_id
         LEFT JOIN pm_projects parent ON parent.repo=w.repo AND parent.provider='linear'
                                     AND parent.id=p.external_project_id
         LEFT JOIN pm_projects observed ON observed.repo=i.repo AND observed.provider=i.provider
                                       AND observed.id=i.project_id
         LEFT JOIN pm_issue_changes changes ON changes.issue_id=t.external_issue_id
         WHERE t.id IN (SELECT value FROM json_each(?1))",
    )?;
    let rows = statement.query_map(params![ids, allow_pending_membership], |row| {
        Ok((
            TaskId::from_raw(row.get::<_, String>(0)?),
            row.get::<_, Option<i64>>(1)?,
            row.get::<_, bool>(2)?,
            row.get::<_, bool>(3)?,
            row.get::<_, bool>(4)?,
        ))
    })?;
    rows.map(|row| {
        let (id, observed_at, invalid, removed, changed) = row?;
        let state = if removed {
            PlanningState::Removed
        } else if invalid || (observed_at.is_none() && changed) {
            PlanningState::Invalid
        } else if observed_at.is_some() || !linear.contains(&id) {
            PlanningState::Available
        } else {
            PlanningState::Unavailable
        };
        Ok((id, crate::store::TaskAcquisition { state, observed_at }))
    })
    .collect()
}

/// IDs as the JSON array batch reads expand with `json_each(?1)`.
pub(super) fn ids_json<'a>(ids: impl IntoIterator<Item = &'a str>) -> String {
    serde_json::Value::Array(
        ids.into_iter()
            .map(|id| serde_json::Value::String(id.to_owned()))
            .collect(),
    )
    .to_string()
}

fn task_deleted_on(conn: &Connection, task: &Task) -> StoreResult<bool> {
    // Either receipt applies regardless of planning ownership.
    // Registration also checks provider evidence before a Task row exists.
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND planning_deleted_at IS NOT NULL)
         OR EXISTS(SELECT 1 FROM task_deletions WHERE wave_id=?2 AND issue_id=?3)",
        params![
            task.id.as_str(),
            task.wave_id.as_ref().map(WaveId::as_str),
            task.linear_id.as_ref().map(|id| id.as_str())
        ],
        |row| row.get(0),
    )?)
}

fn insert_task_row(conn: &Connection, task: &Task) -> StoreResult<()> {
    let repo: String = conn.query_row(
        "SELECT repo FROM repository_plans WHERE id=?1",
        [task.repository_id.as_str()],
        |row| row.get(0),
    )?;
    planning_write::create(
        conn,
        &repo,
        &crate::work::edit::WorkEdit::for_target(
            task.id.as_str(),
            &[
                Edit::TaskProject(task.project_id.as_ref().map(ToString::to_string)),
                Edit::TaskLinearId(task.linear_id.as_ref().map(|id| id.as_str().to_owned())),
                Edit::TaskIdentifier(task.identifier.clone()),
                Edit::TaskTitle(Some(task.title.clone())),
                Edit::TaskDescription(Some(task.description.clone())),
                Edit::TaskRank(task.rank),
                Edit::TaskAssignee(task.assignee.clone()),
                Edit::TaskDueDate(task.due_date.clone()),
                Edit::Disposition(crate::work::edit::Disposition {
                    planning_state: task.state.clone(),
                    planning_completed: i64::from(task.state.as_deref() == Some("completed")),
                    planning_completed_at: task.completed_at.clone(),
                }),
            ],
        ),
    )?;
    conn.execute("UPDATE tasks SET pm_snapshot_synced_at=?2,worktree=?3,workspace_slug=?4,created_at=?5,updated_at=?6,agent=?7,branch=?8,base_commit=?9,parent_pr_id=?10,checkout_machine_id=?11 WHERE id=?1",params![
        task.id.as_str(),task.pm_snapshot_synced_at,task.worktree.as_ref().map(|p|p.display().to_string()),task.workspace_slug,
        task.created_at.unix_timestamp(),task.updated_at.unix_timestamp(),task.agent,task.branch,task.base_commit,task.parent_pr_id.as_ref().map(TaskPrId::as_str),task.worktree.as_ref().map(|_|super::durable::map_local_machine(conn).map(|m|m.id.to_string())).transpose()?])?;
    Ok(())
}

/// Seed the Linear observation cursor from the planning directive, in the Task's
/// creation transaction, so the first observed edit becomes direction rather than
/// a baseline. The empty revision lets any Linear `updatedAt` advance the cursor.
fn seed_task_linear_observation(conn: &Connection, task: &Task) -> StoreResult<()> {
    conn.execute(
        "INSERT OR IGNORE INTO task_linear_observations (
            task_id, last_revision, last_title, last_description,
            last_success_at, degraded_reason, updated_at
         ) VALUES (?1, '', ?2, ?3, ?4, NULL, ?4)",
        params![task.id.as_str(), task.title, task.description, now_unix(),],
    )?;
    Ok(())
}

fn validate_task_project(conn: &Connection, task: &Task) -> StoreResult<()> {
    let wave: Option<WaveId> = match &task.project_id {
        Some(project) => conn.query_row(
            "SELECT p.wave_id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo WHERE p.id=?1 AND r.id=?2",
            params![project.as_str(), task.repository_id.as_str()], |row| row.get(0),
        )?,
        None => None,
    };
    if wave != task.wave_id {
        return Err(StoreError::InvalidData(format!(
            "Task {} has inconsistent Project/Wave membership",
            task.id
        )));
    }
    Ok(())
}

pub(super) fn task_checkout_in(
    conn: &Connection,
    task: &TaskId,
) -> StoreResult<Option<TaskCheckout>> {
    conn.query_row(
        &format!("{TASK_CHECKOUT_COLUMNS} AND id=?1"),
        [task.as_str()],
        |row| map_task_checkout(row, |id| short_task_id(conn, id)),
    )
    .optional()
    .map_err(Into::into)
}

const TASK_CHECKOUT_COLUMNS: &str = "SELECT id, worktree, checkout_machine_id
    FROM tasks WHERE worktree IS NOT NULL";

fn map_task_checkout(
    row: &rusqlite::Row<'_>,
    short: impl FnOnce(&TaskId) -> StoreResult<Option<String>>,
) -> rusqlite::Result<TaskCheckout> {
    let task_id = TaskId::from_raw(row.get::<_, String>(0)?);
    let issue_identifier = short(&task_id)
        .map_err(|error| invalid_column(0, error))?
        .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    let machine: Option<String> = row.get(2)?;
    Ok(TaskCheckout {
        task_id,
        issue_identifier,
        worktree: PathBuf::from(row.get::<_, String>(1)?),
        machine_id: machine
            .map(|id| crate::durable::MachineId::parse(&id))
            .transpose()
            .map_err(|error| invalid_column(2, error))?,
    })
}

pub(super) const TASK_VISIBLE: &str = "t.planning_deleted_at IS NULL
    AND NOT EXISTS(SELECT 1 FROM task_deletions d WHERE d.wave_id=p.wave_id AND d.issue_id=t.external_issue_id)";
/// Present locally known Linear identity, otherwise the shortest unique local hex prefix.
pub(super) fn short_task_id(conn: &Connection, id: &TaskId) -> StoreResult<Option<String>> {
    let identifier: Option<String> = conn
        .query_row(
            "SELECT issue_identifier FROM tasks WHERE id=?1",
            [id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    let Some(identifier) = identifier else {
        return Ok(None);
    };
    let hex = id.as_str().trim_start_matches("task_").to_ascii_lowercase();
    if identifier != format!("lf-{hex}") {
        return Ok(Some(identifier));
    }
    let mut query = conn.prepare(
        "SELECT lower(substr(id,6)) FROM tasks WHERE id!=?1 AND substr(lower(id),6,6)=?2",
    )?;
    let others = query
        .query_map(params![id.as_str(), &hex[..6]], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Some(shortest_unique_prefix(&hex, &others)))
}

/// The local hex prefix, six characters or more, that tells `hex` apart from
/// every other Task sharing its first six.
fn shortest_unique_prefix(hex: &str, others: &[String]) -> String {
    // Distinguish every collision with one character beyond its shared prefix.
    let mut length = 6;
    for other in others {
        let shared = hex
            .bytes()
            .zip(other.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        length = length.max(shared + 1).min(hex.len());
    }
    hex[..length].to_string()
}

/// Every Task's short identifier, read in one statement for list reads.
#[derive(Debug)]
pub(super) struct TaskIdentifiers {
    issue_identifiers: HashMap<String, String>,
    by_prefix: HashMap<String, Vec<String>>,
}

impl TaskIdentifiers {
    pub(super) fn read(conn: &Connection) -> StoreResult<Self> {
        let mut statement =
            conn.prepare("SELECT lower(substr(id,6)),issue_identifier FROM tasks")?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut by_prefix = HashMap::<String, Vec<String>>::new();
        for (hex, _) in &rows {
            if hex.len() >= 6 {
                by_prefix
                    .entry(hex[..6].to_string())
                    .or_default()
                    .push(hex.clone());
            }
        }
        Ok(Self {
            issue_identifiers: rows.into_iter().collect(),
            by_prefix,
        })
    }

    /// What `short_task_id` answers for `id`, without another statement.
    pub(super) fn short(&self, id: &TaskId) -> Option<String> {
        let hex = id.as_str().trim_start_matches("task_").to_ascii_lowercase();
        let identifier = self.issue_identifiers.get(&hex)?;
        if *identifier != format!("lf-{hex}") {
            return Some(identifier.clone());
        }
        let others: Vec<String> = self
            .by_prefix
            .get(&hex[..6])
            .into_iter()
            .flatten()
            .filter(|other| **other != hex)
            .cloned()
            .collect();
        Some(shortest_unique_prefix(&hex, &others))
    }
}

pub(super) fn cached_short_task_id(
    conn: &Connection,
    labels: &mut HashMap<TaskId, Option<String>>,
    id: &TaskId,
) -> StoreResult<Option<String>> {
    if let Some(label) = labels.get(id) {
        return Ok(label.clone());
    }
    let label = short_task_id(conn, id)?;
    labels.insert(id.clone(), label.clone());
    Ok(label)
}

const TASK_COLUMNS: &str = "SELECT t.id,t.external_issue_id,
    t.issue_title, t.issue_description,
    p.wave_id, t.worktree, t.workspace_slug,
    t.created_at, t.updated_at, t.pm_snapshot_synced_at,
    COALESCE((SELECT json_object('state','pending','operation',
        CASE d.target WHEN 'completed' THEN 'complete_task' WHEN 'canceled' THEN 'cancel_task' ELSE 'reopen_task' END,
        'error',COALESCE(d.error,'Saved locally; pending planning synchronization'))
        FROM task_state_deliveries d WHERE d.task_id=t.id AND d.settled=0
            AND t.external_issue_id IS NOT NULL
            AND d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=t.id)),
        json_object('state','current')),
    t.project_id, t.agent, t.planning_revision, t.branch, t.base_commit, t.parent_pr_id,
    t.planning_rank,t.planning_assignee,t.planning_due_date,COALESCE(t.planning_state,CASE WHEN t.planning_completed=1 THEN 'completed' END),t.planning_completed_at,t.repository_id,t.regressed_in
    FROM tasks t LEFT JOIN projects p ON p.id=t.project_id";
pub(super) const TASK_PR_COLUMNS: &str = "SELECT
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
/// only when the comment revision is new to the ledger. Overlapping observations
/// cannot deliver it twice, and a local comment echo adds no direction.
fn ingest_linear_comment(
    conn: &rusqlite::Transaction<'_>,
    task_id: &str,
    comment_id: &str,
    author_name: Option<&str>,
    text: &str,
    observed_at: i64,
) -> StoreResult<Option<i64>> {
    if let Some((id, _)) = comment_id.split_once('@') {
        let prefix = format!("{id}@");
        let latest: Option<String> = conn.query_row(
            "SELECT MAX(comment_id) FROM task_linear_ingested_comments WHERE task_id=?1 AND substr(comment_id, 1, length(?2))=?2",
            params![task_id, prefix], |row| row.get(0),
        )?;
        if latest.as_deref().is_some_and(|latest| latest > comment_id) {
            return Ok(None);
        }
    }
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO task_linear_ingested_comments
            (task_id, comment_id, ingested_at) VALUES (?1, ?2, ?3)",
        params![task_id, comment_id, observed_at],
    )?;
    let id = comment_id.split_once('@').map_or(comment_id, |(id, _)| id);
    let own_echo: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM task_comments c JOIN task_comment_deliveries d
         ON d.comment_id=c.id WHERE c.id=?1 AND c.task_id=?2 AND d.acknowledged=1 AND d.conflicting_comment_json IS NULL
         AND json_extract(d.comment_json,'$.body')=c.body)",
        params![id, task_id], |row| row.get(0),
    )?;
    if inserted == 1 && !own_echo {
        let steer = SqliteStore::save_task_steer_in(
            conn,
            &TaskId::from_raw(task_id),
            &author_name
                .map(|name| Author::Named(name.into()))
                .unwrap_or(Author::User),
            text,
            &format!("comment:{id}"),
            comment_id
                .split_once('@')
                .map_or("local", |(_, revision)| revision),
        )?;
        Ok(Some(steer.sequence))
    } else {
        Ok(None)
    }
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
            publication_requested_at,
            github_number, github_url, merge_commit, abandoned_at,
            created_at, updated_at, github_head_sha, ci_observation, parent_pr_id,
            github_observation,
            linear_attachment_id, linear_comment_id, linear_link_error,
            merge_mode, merge_requested_at, merge_head_sha,
            pr_title, pr_body, pr_copy_head_sha
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
        params![
            pr.id.as_str(),
            pr.task_id.as_str(),
            i64::from(pr.sequence),
            pr.slug,
            pr.branch,
            pr.base_commit,
            publication.map(|publication| publication.requested_at.unix_timestamp()),
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
            publication_requested_at=?7,
            github_number=?8, github_url=?9, merge_commit=?10,
            abandoned_at=?11, updated_at=?13, github_head_sha=?14,
            ci_observation=?15, github_observation=?17,
            linear_attachment_id=?18, linear_comment_id=?19, linear_link_error=?20,
            merge_mode=?21, merge_requested_at=?22, merge_head_sha=?23,
            pr_title=?24, pr_body=?25, pr_copy_head_sha=?26
         WHERE id=?1 AND task_id=?2 AND sequence=?3 AND slug=?4
           AND branch=?5 AND base_commit=?6 AND created_at=?12",
        params![
            pr.id.as_str(),
            pr.task_id.as_str(),
            i64::from(pr.sequence),
            pr.slug,
            pr.branch,
            pr.base_commit,
            publication.map(|publication| publication.requested_at.unix_timestamp()),
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

// Keep physical lookup, correspondence validation and record hydration in the
// caller's read snapshot. A concurrent mapping change cannot mix generations.
pub(super) fn resolve_task_id_in(
    conn: &Connection,
    issue: &str,
    repo: Option<&str>,
) -> StoreResult<Option<TaskId>> {
    let selector = crate::id::TaskSelector::parse(issue);
    let mut statement = conn.prepare(
        "SELECT t.id, COALESCE(t.issue_title, t.issue_identifier, t.id), w.repo FROM tasks t
         JOIN repository_plans w ON w.id=t.repository_id
         WHERE (t.id=?1 OR t.external_issue_id=?3 OR t.issue_identifier=?4
            OR substr(t.id, 6, length(?2))=?2
            OR EXISTS(SELECT 1 FROM task_creation_intents c WHERE c.task_id=t.id
                AND json_extract(c.export_json,'$.id')=?3))
         ORDER BY t.id",
    )?;
    let mut tasks = statement
        .query_map(
            params![
                selector.id,
                selector.local_prefix,
                selector.external,
                selector.label
            ],
            |row| {
                Ok((
                    TaskId::from_raw(row.get::<_, String>(0)?),
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if let Some(repo) = repo {
        let canonical = crate::repository::CanonicalRepo::discover(std::path::Path::new(repo)).ok();
        tasks.retain(|(_, _, owner)| {
            owner.as_deref() == Some(repo)
                || owner.as_ref().is_some_and(|owner| {
                    canonical
                        .as_ref()
                        .is_some_and(|canonical| canonical.contains(std::path::Path::new(owner)))
                })
        });
    }
    if tasks.len() > 1 {
        let candidates = tasks
            .iter()
            .map(|(id, title, _)| format!("  {id} ({title})"))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(StoreError::InvalidData(format!(
            "multiple stable Tasks resolve to {issue:?}; use a longer Task ID:\n{candidates}"
        )));
    }
    if let Some((id, _, _)) = tasks.pop() {
        return Ok(Some(id));
    }
    Ok(None)
}

fn project_on(conn: &Connection, project_id: &ProjectId) -> StoreResult<Option<Project>> {
    conn.query_row(PROJECT_SELECT, [project_id.as_str()], map_project_row)
        .optional()
        .map_err(StoreError::from)
}

pub(super) fn task_on(conn: &Connection, task_id: &TaskId) -> StoreResult<Option<Task>> {
    conn.query_row(
        &format!("{TASK_COLUMNS} WHERE t.id=?1"),
        [task_id.as_str()],
        |row| map_task_row(conn, row),
    )
    .optional()
    .map_err(StoreError::from)
}

fn task_pr_on(conn: &Connection, pr_id: &TaskPrId) -> StoreResult<Option<TaskPr>> {
    conn.query_row(TASK_PR_SELECT, params![pr_id.as_str()], map_task_pr_row)
        .optional()
        .map_err(StoreError::from)
}

pub(super) fn active_task_pr_on(
    conn: &Connection,
    task_id: &TaskId,
) -> StoreResult<Option<TaskPr>> {
    let query = format!(
        "{TASK_PR_COLUMNS}
         WHERE task_id=?1 AND historical=0"
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

fn validate_task_pr_settlement(settled: &TaskPr) -> StoreResult<()> {
    validate_task_pr(settled)?;
    if !settled.is_settled() {
        return Err(StoreError::InvalidData(
            "Task PR transition requires a settled PR".to_string(),
        ));
    }
    Ok(())
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
        settle_task_pr_on(conn, settled)?;
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

    settle_task_pr_on(conn, settled)?;
    if let crate::store::TaskPrMergeEvidenceOutcome::Conflict { accepted_at } = outcome {
        let checked_at = settled
            .github_observation
            .as_ref()
            .map(|observation| observation.checked_at)
            .unwrap_or_else(OffsetDateTime::now_utc);
        let observation = GithubObservation {
            checked_at,
            result: crate::work::task::GithubObservationResult::Partial {
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

fn map_task_row(conn: &Connection, row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let id = TaskId::from_raw(row.get::<_, String>(0)?);
    let identifier = short_task_id(conn, &id)
        .map_err(|error| invalid_column(0, error))?
        .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    map_task_columns(row, id, identifier)
}

/// A Task row whose identifier comes from the list's one identifier read.
fn map_listed_task_row(
    identifiers: &TaskIdentifiers,
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<Task> {
    let id = TaskId::from_raw(row.get::<_, String>(0)?);
    let identifier = identifiers
        .short(&id)
        .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    map_task_columns(row, id, identifier)
}

fn map_task_columns(
    row: &rusqlite::Row<'_>,
    id: TaskId,
    identifier: String,
) -> rusqlite::Result<Task> {
    Ok(Task {
        id,

        revision: row.get::<_, i64>(13)? as u64,
        linear_id: row
            .get::<_, Option<String>>(1)?
            .map(LinearIssueId::from_raw),
        identifier,
        title: row.get(2)?,
        description: row.get(3)?,
        pm_snapshot_synced_at: row.get(9)?,

        pm_writeback: serde_json::from_str(&row.get::<_, String>(10)?)
            .map_err(|error| invalid_column(10, error))?,
        wave_id: row.get(4)?,
        project_id: row.get::<_, Option<String>>(11)?.map(ProjectId::from_raw),
        repository_id: crate::durable::RepositoryId::parse(&row.get::<_, String>(22)?)
            .map_err(|error| invalid_column(22, error))?,
        worktree: row.get::<_, Option<String>>(5)?.map(PathBuf::from),
        workspace_slug: row.get(6)?,
        branch: row.get(14)?,
        base_commit: row.get(15)?,
        parent_pr_id: row.get::<_, Option<String>>(16)?.map(TaskPrId::from_raw),
        agent: row.get(12)?,
        created_at: crate::store::rows::unix_to_datetime(row.get(7)?),
        updated_at: crate::store::rows::unix_to_datetime(row.get(8)?),
        rank: row.get(17)?,
        assignee: row.get(18)?,
        due_date: row.get(19)?,
        regressed_in: row.get(23)?,
        state: row.get(20)?,
        completed_at: row.get(21)?,
        // Runtime freshness is derived when reconciliation decides whether the
        // durable GitHub observation can be reused.
        observation: crate::work::task::Observation::NotRequired,
    })
}

pub(super) fn map_task_pr_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskPr> {
    let publication_requested_at = row.get::<_, Option<i64>>(6)?;
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
    ) {
        (Some(mode), Some(requested_at), Some(head_sha)) => Some(PrMergeRequest {
            mode: mode.parse().map_err(|error| invalid_column(22, error))?,
            requested_at: crate::store::rows::unix_to_datetime(requested_at),
            head_sha,
        }),
        (None, None, None) => None,
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

pub(super) fn map_task_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskEvent> {
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

pub(super) fn task_events_after_in(
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

const PROJECT_COLUMNS: &str = "SELECT
    id, external_project_id, project_slug, project_name, description,
    wave_id, pm_snapshot_synced_at,
    created_at, updated_at, status, project_summary, krs, metric_targets
    FROM projects";
pub(super) const PROJECT_SELECT: &str = "SELECT
    id, external_project_id, project_slug, project_name, description,
    wave_id, pm_snapshot_synced_at,
    created_at, updated_at, status, project_summary, krs, metric_targets
    FROM projects WHERE id=?1";
pub(super) fn project_in(conn: &Connection, id: &ProjectId) -> rusqlite::Result<Project> {
    conn.query_row(PROJECT_SELECT, [id.as_str()], map_project_row)
}

pub(super) fn map_project_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: ProjectId::from_raw(row.get::<_, String>(0)?),

        summary: row.get(10)?,
        linear_id: row
            .get::<_, Option<String>>(1)?
            .map(LinearProjectId::from_raw),
        slug: row.get(2)?,
        name: row.get(3)?,
        description: row.get(4)?,
        krs: serde_json::from_str(&row.get::<_, String>(11)?)
            .map_err(|error| invalid_column(11, error))?,
        metric_targets: serde_json::from_str(&row.get::<_, String>(12)?)
            .map_err(|error| invalid_column(12, error))?,
        pm_snapshot_synced_at: row.get(6)?,
        status: serde_json::from_value(serde_json::Value::String(row.get(9)?))
            .map_err(|error| invalid_column(9, error))?,

        wave_id: row.get(5)?,
        created_at: crate::store::rows::unix_to_datetime(row.get(7)?),
        updated_at: crate::store::rows::unix_to_datetime(row.get(8)?),
    })
}

fn map_project_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectEvent> {
    let kind: ProjectEventKind = serde_json::from_str(&row.get::<_, String>(2)?)
        .map_err(|error| invalid_column(2, error))?;
    Ok(ProjectEvent {
        id: row.get(0)?,
        project_id: ProjectId::from_raw(row.get::<_, String>(1)?),
        kind,
        created_at: crate::store::rows::unix_to_datetime(row.get(3)?),
    })
}

pub(super) fn insert_task_event_in(
    conn: &Connection,
    task_id: &TaskId,
    kind: &TaskEventKind,
) -> StoreResult<TaskEvent> {
    let created_at = now_unix();
    if conn.execute(
        "INSERT INTO task_events (task_id, kind_json, created_at)
         SELECT id,?2,?3 FROM tasks WHERE id=?1",
        params![task_id.as_str(), serde_json::to_string(kind)?, created_at],
    )? == 0
    {
        return Err(StoreError::NotFound);
    }
    let event_id = conn.last_insert_rowid();
    Ok(TaskEvent {
        id: event_id,
        task_id: task_id.clone(),
        kind: kind.clone(),
        created_at: crate::store::rows::unix_to_datetime(created_at),
    })
}

pub(super) fn insert_project_event_in(
    conn: &Connection,
    project: &Project,
    kind: &ProjectEventKind,
) -> StoreResult<ProjectEvent> {
    let created_at = now_unix();
    conn.execute(
        "INSERT INTO project_events (project_id, kind_json, created_at)
         VALUES (?1, ?2, ?3)",
        params![
            project.id.as_str(),
            serde_json::to_string(kind)?,
            created_at
        ],
    )?;
    let event_id = conn.last_insert_rowid();
    Ok(ProjectEvent {
        id: event_id,
        project_id: project.id.clone(),
        kind: kind.clone(),
        created_at: crate::store::rows::unix_to_datetime(created_at),
    })
}

pub(super) fn request_completion_in(
    conn: &Connection,
    task: &TaskId,
    note: Option<&str>,
) -> StoreResult<Option<i64>> {
    let (completed, pending): (Option<i64>, Option<i64>) = conn.query_row(
        "SELECT CASE WHEN planning_completed=1 THEN 1 END,completion_request FROM tasks WHERE id=?1",
        [task.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if completed.is_some() {
        return Ok(None);
    }
    if pending.is_some() {
        return Ok(pending);
    }
    insert_task_event_in(
        conn,
        task,
        &TaskEventKind::CompletionRequested {
            reason: note.map(str::to_string),
        },
    )?;
    let request = conn.last_insert_rowid();
    conn.execute(
        "UPDATE tasks SET completion_request=?2,completion_error=NULL WHERE id=?1",
        params![task.as_str(), request],
    )?;
    Ok(Some(request))
}

pub(super) fn tasks_in(conn: &Connection, wave_id: Option<&WaveId>) -> StoreResult<Vec<Task>> {
    tasks_where(conn, wave_id, "1")
}

fn tasks_where(
    conn: &Connection,
    wave_id: Option<&WaveId>,
    condition: &str,
) -> StoreResult<Vec<Task>> {
    let filter = match wave_id {
        Some(_) => "p.wave_id=?1 AND ",
        None => "",
    };
    let query = format!(
        "{TASK_COLUMNS} WHERE {filter}({condition}) AND {TASK_VISIBLE} ORDER BY t.updated_at DESC"
    );
    let identifiers = TaskIdentifiers::read(conn)?;
    let mut statement = conn.prepare(&query)?;
    let rows = statement.query_map(params_from_iter(wave_id), |row| {
        map_listed_task_row(&identifiers, row)
    })?;
    rows.collect::<rusqlite::Result<_>>()
        .map_err(StoreError::from)
}

/// When a closed Task closed, as RFC 3339 text: abandonment, else completion,
/// else its last update. Orders the closed-Task pages.
const TASK_CLOSED_AT: &str = "CASE WHEN t.abandoned_at IS NOT NULL
    THEN strftime('%Y-%m-%dT%H:%M:%SZ',t.abandoned_at,'unixepoch')
    ELSE COALESCE(t.planning_completed_at,strftime('%Y-%m-%dT%H:%M:%SZ',t.updated_at,'unixepoch')) END";

/// One page of closed (done or abandoned) Tasks, most recently closed first.
#[derive(Debug, Clone)]
pub struct ClosedTaskPage {
    pub tasks: Vec<Task>,
    /// The last Task of a full page; pass it as `after` for the next page.
    pub next: Option<TaskId>,
}

#[cfg(test)]
impl SqliteStore {
    pub(crate) fn seed_unplaced_task(&self, task: &Task) {
        let mut task = task.clone();
        task.worktree = None;
        task.workspace_slug.clear();
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().unwrap();
        insert_task_row(&tx, &task).unwrap();
        tx.commit().unwrap();
    }

    pub(crate) fn seed_task(&self, task: &Task, pr: &TaskPr) -> StoreResult<Task> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_task(task)?;
        validate_task_project(&tx, task)?;
        if task_on(&tx, &task.id)?.is_none() {
            insert_task_row(&tx, task)?;
        } else {
            tx.execute(
                "UPDATE tasks SET worktree=?2,workspace_slug=?3,agent=?4,branch=?5,base_commit=?6,parent_pr_id=?7,checkout_machine_id=?8 WHERE id=?1",
                params![
                    task.id.as_str(),
                    task.worktree.as_ref().map(|p| p.display().to_string()),
                    task.workspace_slug,
                    task.agent, task.branch, task.base_commit, task.parent_pr_id.as_ref().map(|id| id.as_str()),
                    task.worktree.as_ref().map(|_| super::durable::map_local_machine(&tx).map(|machine| machine.id.to_string())).transpose()?
                ],
            )?;
        }
        insert_task_pr(&tx, pr)?;
        if task.worktree.is_some() {
            // A seeded checkout is placed work, which claims the Task like `place_task`.
            super::task_ownership::claim_in(&tx, &task.id)?;
        }
        seed_task_linear_observation(&tx, task)?;
        let saved = task_on(&tx, &task.id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(saved)
    }
}

#[cfg(test)]
mod local_planning_tests {

    use rusqlite::params;
    use rusqlite::OptionalExtension;

    use crate::durable::{ProjectId, RepositoryId, TaskId};
    use crate::id::WaveId;
    use crate::store::sqlite::SqliteStore;
    use crate::work::task::NewTask;
    use crate::work::task::TaskUpdate;

    fn local_repository(store: &SqliteStore) -> RepositoryId {
        let repo = store.home_dir().unwrap();
        // Fence Git discovery before registering the fixture repository.
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        store.ensure_repository(repo.to_str().unwrap()).unwrap()
    }

    fn local_project(store: &SqliteStore) -> ProjectId {
        local_repository(store);
        store
            .ensure_wave_project(store.home_dir().unwrap().to_str().unwrap(), "inbox")
            .unwrap()
            .id
    }

    #[test]
    fn retired_branch_lookup_preserves_history_without_full_scans() {
        use crate::store::migrations::{apply_before_migration, migration_section_sql};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "retired_work_lookup");
        let wave = WaveId::new();
        let project = ProjectId::new();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
            [&wave],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO projects(id,wave_id,created_at) VALUES(?1,?2,1)",
            params![project.as_str(), wave],
        )
        .unwrap();
        let mut first = None;
        for n in 0..2_000 {
            let task = TaskId::new();
            first.get_or_insert_with(|| task.to_string());
            let branch = format!("retired-{n}");
            conn.execute(
                include_str!("../../../tests/fixtures/migrations/retired_work_lookup_task.sql"),
                params![task.as_str(), project.as_str(), branch],
            )
            .unwrap();
            conn.execute("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at)
                VALUES(?1,?2,1,?3,?3,'base',1,1)", params![format!("pr-{n}"),task.as_str(),branch]).unwrap();
        }
        let read = || {
            let mut query = conn.prepare(super::BRANCH_TASK_IDS).unwrap();
            let ids = query
                .query_map(["retired-0"], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert_eq!(ids, vec![first.clone().unwrap()]);
            query.get_status(rusqlite::StatementStatus::FullscanStep)
        };
        let before = read();
        conn.execute_batch(&migration_section_sql("retired_work_lookup"))
            .unwrap();
        let after = read();
        eprintln!("branch full-scan steps: {before} → {after}");
        assert!(
            before >= 3_998,
            "fixture did not reproduce historical scans: {before}"
        );
        assert_eq!(after, 0);
        assert_eq!(
            conn.query_row("SELECT count(*) FROM task_prs", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            2_000
        );
        assert_eq!(
            conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
    }

    #[test]
    fn task_labels_and_generated_names_follow_local_identity_and_linear_mapping() {
        use crate::session::{SessionFilter, TitleSource, WorkSource};
        use crate::work::edit::{WorkEdit, WorkValue};
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let task = store
            .create_task(&NewTask {
                repository_id: local_repository(&store),
                id: TaskId::from_raw("task_abcdef10000000000000000000000001"),
                project_id: Some(local_project(&store)),
                title: "Fix login".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        assert_eq!(task.identifier, "abcdef");
        let template = store.test_session("template", &crate::session_record::new_artifact_key());
        for (id, source, title) in [
            ("generated", TitleSource::Generated, "Fix login"),
            ("human", TitleSource::Human, "My login notes"),
        ] {
            let mut session = template.clone();
            session.id = id.into();
            session.artifact_key = Some(crate::session_record::new_artifact_key());
            session.captured = None;
            session.task_id = Some(task.id.clone());
            session.wave_id = Some(task.wave_id.clone().unwrap());
            session.work_source = Some(WorkSource::Declared);
            session.title = title.into();
            session.title_source = source;
            let saved = store.create_session(session, None).unwrap();
            assert_eq!(
                saved.title,
                if id == "generated" {
                    "abcdef Fix login"
                } else {
                    title
                }
            );
        }
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE tasks SET worktree='/retained',branch='old/branch',external_issue_id='pending-export' WHERE id=?1", [task.id.as_str()]).unwrap();
            conn.execute(
                "UPDATE agent_sessions SET title=?1 WHERE id='generated'",
                [format!(
                    "lf-{} Fix login",
                    task.id.as_str().trim_start_matches("task_")
                )],
            )
            .unwrap();
        }
        let summaries = store
            .session_summaries(&SessionFilter::default(), 0)
            .unwrap();
        let summary = summaries
            .iter()
            .find(|session| session.id == "generated")
            .unwrap();
        assert_eq!(summary.task_identifier.as_deref(), Some("abcdef"));
        assert!(
            summary.title.starts_with("lf-abcdef1000"),
            "passive reads preserve titles"
        );
        assert_eq!(
            store
                .task_checkout(&task.id)
                .unwrap()
                .unwrap()
                .issue_identifier,
            "abcdef"
        );
        let process = crate::id::LfProcessId::new();
        store.conn.lock().unwrap().execute(
            "INSERT INTO processes(id,trace_id,started_at,command,cwd) VALUES(?1,'trace',1,'lf session connect','/fixture')",
            [&process],
        ).unwrap();
        store
            .claim_session_attachment("generated", None, &process, false)
            .unwrap();
        assert_eq!(
            store.session("generated").unwrap().unwrap().title,
            "abcdef Fix login"
        );
        let history = store
            .conversation_history(None, None, Some("abcdef"), None, 0, true)
            .unwrap();
        assert_eq!(history.len(), 2);
        assert!(history
            .iter()
            .all(|input| input.task_identifier.as_deref() == Some("abcdef")));
        {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            super::planning_write::local(
                &tx,
                &WorkEdit::for_target(
                    task.id.as_str(),
                    &[WorkValue::TaskIdentifier("ENG-123".into())],
                ),
            )
            .unwrap();
            tx.commit().unwrap();
        }
        let mapped = store.task(&task.id).unwrap().unwrap();
        assert_eq!(mapped.identifier, "ENG-123");
        assert_eq!(mapped.branch, "old/branch");
        assert_eq!(
            store.task_checkouts().unwrap()[0].issue_identifier,
            "ENG-123"
        );
        assert_eq!(
            store.session("generated").unwrap().unwrap().title,
            "ENG-123 Fix login"
        );
        assert_eq!(
            store.session("human").unwrap().unwrap().title,
            "My login notes"
        );
        store
            .rename_session("generated", "Review login", TitleSource::Generated)
            .unwrap();
        assert_eq!(
            store.session("generated").unwrap().unwrap().title,
            "ENG-123 Review login"
        );
        let history = store
            .conversation_history(None, None, Some("eng-123"), None, 0, true)
            .unwrap();
        assert_eq!(history.len(), 2);
        assert!(history
            .iter()
            .all(|input| input.task_identifier.as_deref() == Some("ENG-123")));
        let summary = store.session_summary("generated", 0).unwrap().unwrap();
        assert_eq!(summary.task_identifier.as_deref(), Some("ENG-123"));
    }

    #[test]
    fn local_labels_extend_only_for_collisions_including_completed_tasks() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let project = local_project(&store);
        let first = TaskId::from_raw("task_abcdef10000000000000000000000001");
        let second = TaskId::from_raw("task_abcdef20000000000000000000000002");
        for id in [&first, &second] {
            store
                .create_task(&NewTask {
                    repository_id: local_repository(&store),
                    id: id.clone(),
                    project_id: Some(project.clone()),
                    title: "Fix login".into(),
                    description: String::new(),
                    due_date: None,
                })
                .unwrap();
        }
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_completed=1,issue_identifier='ENG-9' WHERE id=?1",
                [second.as_str()],
            )
            .unwrap();
        assert_eq!(store.task(&first).unwrap().unwrap().identifier, "abcdef1");
        assert_eq!(store.task(&second).unwrap().unwrap().identifier, "ENG-9");
        assert!(store.task_by_issue("abcdef").is_err());
        assert_eq!(store.task_by_issue("abcdef1").unwrap().unwrap().id, first);
        let conn = store.conn.lock().unwrap();
        assert_eq!(super::short_task_id(&conn, &TaskId::new()).unwrap(), None);
    }

    #[test]
    fn task_checkout_read_distinguishes_unplaced_and_unknown_machine() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&directory.path().join("db")).unwrap();
        let task = store
            .create_task(&NewTask {
                repository_id: local_repository(&store),
                due_date: None,
                id: TaskId::new(),
                project_id: Some(local_project(&store)),
                title: "Locate files".into(),
                description: String::new(),
            })
            .unwrap();
        assert!(store.task_checkout(&TaskId::new()).unwrap().is_none());
        assert!(store.task_checkout(&task.id).unwrap().is_none());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET worktree='/retained/checkout' WHERE id=?1",
                [task.id.as_str()],
            )
            .unwrap();
        let checkout = store.task_checkout(&task.id).unwrap().unwrap();
        assert_eq!(checkout.task_id, task.id);
        assert_eq!(checkout.issue_identifier, task.identifier);
        assert_eq!(
            checkout.worktree,
            std::path::Path::new("/retained/checkout")
        );
        assert!(checkout.machine_id.is_none());
        let local = store.local_machine().unwrap().id;
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET checkout_machine_id=?2 WHERE id=?1",
                params![task.id.as_str(), local.as_str()],
            )
            .unwrap();
        assert_eq!(
            store.task_checkout(&task.id).unwrap().unwrap().machine_id,
            Some(local)
        );
    }

    #[test]
    fn canonical_task_fields_survive_restart_and_peer_import() {
        use super::super::planning_write;
        use crate::planning_git::PlanningDestination;
        use crate::work::edit::{Disposition, WorkValue};

        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let path = source.path().join("loopflow.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let task = store
            .create_task(&NewTask {
                repository_id: local_repository(&store),
                id: TaskId::new(),
                project_id: Some(local_project(&store)),
                title: "Shared Task".into(),
                description: "Keep the local execution".into(),
                due_date: Some("2026-10-15".into()),
            })
            .unwrap();
        assert_eq!(task.due_date.as_deref(), Some("2026-10-15"));
        {
            let conn = store.conn.lock().unwrap();
            let tx = conn.unchecked_transaction().unwrap();
            planning_write::local(
                &tx,
                &crate::work::edit::WorkEdit::for_target(
                    task.id.as_str(),
                    &[
                        WorkValue::TaskRank(7),
                        WorkValue::TaskAssignee(Some("assignee-1".into())),
                        WorkValue::Disposition(Disposition {
                            planning_state: Some("completed".into()),
                            planning_completed: 1,
                            planning_completed_at: Some("2026-10-10T12:00:00Z".into()),
                        }),
                    ],
                ),
            )
            .unwrap();
            tx.commit().unwrap();
        }
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let saved = store.task(&task.id).unwrap().unwrap();
        assert_eq!(saved.rank, 7);
        assert_eq!(saved.assignee.as_deref(), Some("assignee-1"));
        assert_eq!(saved.state.as_deref(), Some("completed"));
        assert_eq!(saved.completed_at.as_deref(), Some("2026-10-10T12:00:00Z"));
        assert_eq!(saved.due_date, task.due_date);
        assert_eq!(saved.worktree, task.worktree);
        assert_eq!(saved.branch, task.branch);
        assert_eq!(saved.parent_pr_id, task.parent_pr_id);
        let peer = SqliteStore::open_ephemeral(&target.path().join("loopflow.db")).unwrap();
        let destination =
            PlanningDestination::new("/fixture.git", "refs/loopflow/planning/shared/test").unwrap();
        let wave = store
            .get_wave(saved.wave_id.as_ref().unwrap())
            .unwrap()
            .unwrap();
        let repo = wave.repo();
        let target_path = target.path().canonicalize().unwrap();
        let target_repo = target_path.to_str().unwrap();
        let destination_id = store.bind_peer_planning(repo, &destination).unwrap();
        peer.bind_peer_planning(target_repo, &destination).unwrap();
        let snapshot = store.export_peer_planning(repo, &destination_id).unwrap();
        for _ in 0..2 {
            peer.import_peer_planning(target_repo, &destination_id, "fixture", &snapshot)
                .unwrap();
            let imported = peer.task(&task.id).unwrap().unwrap();
            assert_eq!(imported.id, saved.id);
            assert_eq!(imported.project_id, saved.project_id);
            assert_eq!(imported.title, saved.title);
            assert_eq!(imported.description, saved.description);
            assert_eq!(imported.rank, saved.rank);
            assert_eq!(imported.assignee, saved.assignee);
            assert_eq!(imported.due_date, saved.due_date);
            assert_eq!(imported.state, saved.state);
            assert_eq!(imported.completed_at, saved.completed_at);
            assert!(imported.worktree.is_none());
            assert!(peer.task_prs(&task.id).unwrap().is_empty());
        }
    }

    #[test]
    fn project_edits_preserve_retired_iteration_and_events() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let id = local_project(&store);
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE projects SET iteration=7 WHERE id=?1", [id.as_str()])
            .unwrap();
        let event = store
            .append_project_event(
                &id,
                &crate::work::project::ProjectEventKind::IterationCompleted {
                    iteration: 7,
                    summary: "Retained historical judgment".into(),
                },
            )
            .unwrap();
        store
            .edit_project(&id, Some("Renamed body of work"), Some("New summary"), None)
            .unwrap();
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let project = store.project(&id).unwrap().unwrap();
        assert_eq!(project.name, "Renamed body of work");
        assert_eq!(project.summary, "New summary");
        assert_eq!(store.project_events_after(&id, 0).unwrap(), vec![event]);
        let iteration: u32 = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT iteration FROM projects WHERE id=?1",
                [id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(iteration, 7);
        let json = serde_json::to_value(project).unwrap();
        assert!(json.get("iteration").is_none());
        assert!(json.get("plan").is_none());
    }

    #[test]
    fn local_planning_creation_retry_keeps_identity_after_edit_and_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let input = NewTask {
            repository_id: local_repository(&store),
            due_date: None,
            id: TaskId::new(),
            project_id: Some(local_project(&store)),
            title: "Fix parser".into(),
            description: "Keep the original input".into(),
        };
        let task = store.create_task(&input).unwrap();
        assert!(task.worktree.is_none());
        assert!(task.linear_id.is_none());
        assert!(store.task_prs(&task.id).unwrap().is_empty());
        assert!(store.task_checkouts().unwrap().is_empty());
        let edited = store
            .edit_task(
                &task.id,
                task.revision,
                &TaskUpdate {
                    title: Some("Fix quoted input".into()),
                    description: None,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(edited.revision, task.revision + 1);
        assert_eq!(edited.description, input.description);
        assert!(store
            .edit_task(
                &task.id,
                task.revision,
                &TaskUpdate {
                    title: Some("Stale".into()),
                    description: None,
                    ..Default::default()
                }
            )
            .is_err());
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let retried = store.create_task(&input).unwrap();
        assert_eq!(retried, edited);
        let second = store
            .create_task(&NewTask {
                repository_id: local_repository(&store),
                due_date: None,
                id: TaskId::new(),
                ..input.clone()
            })
            .unwrap();
        assert_ne!(second.id, task.id);
        assert_eq!(store.list_tasks(None).unwrap().len(), 2);
        assert_eq!(
            store
                .list_tasks(Some(task.wave_id.as_ref().unwrap()))
                .unwrap()
                .len(),
            2
        );
        assert!(store.list_tasks(Some(&WaveId::new())).unwrap().is_empty());
        assert!(store
            .create_task(&NewTask {
                repository_id: local_repository(&store),
                due_date: None,
                title: "Different request".into(),
                ..input
            })
            .is_err());
        assert_eq!(store.task(&task.id).unwrap().unwrap(), edited);
        let conn = store.conn.lock().unwrap();
        for table in ["agent_sessions", "processes", "task_graphs", "task_prs"] {
            let count: i64 = conn
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "creation must not allocate {table}");
        }
    }

    #[test]
    fn local_planning_provider_mapping_preserves_identity_and_creation_receipt() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&directory.path().join("loopflow.db")).unwrap();
        let input = NewTask {
            repository_id: local_repository(&store),
            due_date: None,
            id: TaskId::new(),
            project_id: Some(local_project(&store)),
            title: "Keep identity".into(),
            description: String::new(),
        };
        let local = store.create_task(&input).unwrap();
        assert_eq!(local.pm_snapshot_synced_at, None);
        // Export is a separate slice; this proves the schema accepts a different
        // provider UUID without changing local identity or creation history.
        store.conn.lock().unwrap().execute(
            "UPDATE tasks SET external_issue_id='different-provider-uuid',issue_identifier='TEAM-9' WHERE id=?1",
            [local.id.as_str()],
        ).unwrap();
        let mapped = store.task_by_issue("TEAM-9").unwrap().unwrap();
        assert_eq!(mapped.id, local.id);
        for selector in ["team-9", "Team-9", "tEaM-9"] {
            let resolved = store.task_by_issue(selector).unwrap().unwrap();
            assert_eq!(resolved.id, local.id);
            assert_eq!(resolved.identifier, "TEAM-9");
        }
        assert_eq!(
            store.task_by_issue(&local.identifier).unwrap().unwrap().id,
            local.id
        );
        assert_eq!(store.create_task(&input).unwrap(), mapped);
        let edited = store
            .edit_task(
                &local.id,
                mapped.revision,
                &TaskUpdate {
                    title: Some("A mapping does not transfer authority".into()),
                    description: None,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(edited.title, "A mapping does not transfer authority");
        store.conn.lock().unwrap().execute("INSERT INTO task_deletions(wave_id,issue_id,identifier,confirmed_at) VALUES(?1,'different-provider-uuid','TEAM-9',1)", [&mapped.wave_id]).unwrap();
        assert!(store.task_deleted(&mapped).unwrap());
        assert!(store.list_tasks(None).unwrap().is_empty());
        let retained = store.task(&mapped.id).unwrap().unwrap();
        assert_eq!(retained.revision, edited.revision);
        assert_eq!(retained.linear_id, edited.linear_id);
        assert_eq!(retained.identifier, edited.identifier);
        assert_eq!(retained.title, edited.title);
        assert_eq!(retained.description, edited.description);
        assert_eq!(retained.pm_snapshot_synced_at, edited.pm_snapshot_synced_at);
        assert_eq!(store.create_task(&input).unwrap(), retained);
    }

    #[test]
    fn local_planning_prefix_collision_is_explicit() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&directory.path().join("loopflow.db")).unwrap();
        let input = NewTask {
            repository_id: local_repository(&store),
            due_date: None,
            id: TaskId::parse("task_0123456789ab40008000000000000001").unwrap(),
            project_id: Some(local_project(&store)),
            title: "One".into(),
            description: String::new(),
        };
        let first = store.create_task(&input).unwrap();
        assert_eq!(first.identifier, "012345");
        for selector in ["0123", "0123456789A", "lf-0123", "task_0123"] {
            assert_eq!(store.task_by_issue(selector).unwrap().unwrap().id, first.id);
        }
        for selector in [
            "012",
            "lf-012",
            "task_012",
            "012z",
            "0123%",
            "fffffffffffffffffffffffffffffffff",
        ] {
            assert!(store.task_by_issue(selector).unwrap().is_none());
        }
        assert_eq!(
            store.task_by_issue("lf-0123456789ab").unwrap().unwrap().id,
            first.id
        );
        let second = store
            .create_task(&NewTask {
                repository_id: local_repository(&store),
                due_date: None,
                id: TaskId::parse("task_0123456789ab40008000000000000002").unwrap(),
                ..input
            })
            .unwrap();
        for selector in ["0123", "lf-0123", "task_0123", "lf-0123456789ab"] {
            let error = store.task_by_issue(selector).unwrap_err().to_string();
            assert!(error.contains(first.id.as_str()));
            assert!(error.contains(second.id.as_str()));
            assert!(error.contains("use a longer Task ID"));
        }
        let first = store.task(&first.id).unwrap().unwrap();
        assert_eq!(first.identifier.len(), 32);
        assert_eq!(second.identifier.len(), 32);
        for task in [first, second] {
            assert_eq!(
                store.task_by_issue(task.id.as_str()).unwrap().unwrap().id,
                task.id
            );
            assert_eq!(
                store.task_by_issue(&task.identifier).unwrap().unwrap().id,
                task.id
            );
        }
    }
    #[test]
    fn local_planning_migration_preserves_released_links_and_orphan_deletion_evidence() {
        use crate::store::migrations::{
            apply_before_migration, migration_section_sql, upgrade_from_migration,
        };
        use std::sync::{Arc, Mutex};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "local_planning");
        let wave = WaveId::new();
        let orphan_wave = WaveId::new();
        let project = ProjectId::new();
        let task = TaskId::new();
        conn.execute("INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'shared','/repo',1),(?2,'recovery','/repo',1)", params![wave,orphan_wave]).unwrap();
        conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at,project_slug,project_name,project_prompt_context,pm_snapshot_synced_at,updated_at) VALUES(?1,?2,'linear-project',1,'shared','Shared','Retain KRs',7,7)",params![project.as_str(),wave]).unwrap();
        conn.execute(
            include_str!("../../../tests/fixtures/migrations/local_planning_retained_task.sql"),
            params![task.as_str(), project.as_str()],
        )
        .unwrap();
        conn.execute("INSERT INTO task_issue_identities(wave_id,issue_id,identifier) VALUES(?1,'orphan-issue','LOO-2')",[&orphan_wave]).unwrap();
        let payload: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        let mut historical_project = payload["projects"][0].clone();
        historical_project["workflow"] = serde_json::json!("");
        let project_body = historical_project.to_string();
        let item_body = payload["items"][0].to_string();
        conn.execute("INSERT INTO pm_projects(repo,provider,id,observed_at,body) VALUES('/repo','linear','current',7,?1)",[&project_body]).unwrap();
        conn.execute("INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body) VALUES('/repo','linear','LOO-318','LOO-318','current',7,?1)",[&item_body]).unwrap();
        conn.execute("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at) VALUES('pr-retained',?1,1,'task','retain/branch','retained-base',1,7)",[task.as_str()]).unwrap();
        conn.execute_batch(
            &include_str!("../../../tests/fixtures/migrations/local_planning_retained_process.sql")
                .replace("{task}", task.as_str())
                .replace("{wave}", wave.as_str()),
        )
        .unwrap();
        let imported_project = ProjectId::new();
        conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at,project_slug,project_name,project_prompt_context,pm_snapshot_synced_at,updated_at) VALUES(?1,?2,'current',1,'current','Current','',7,7)",params![imported_project.as_str(),wave]).unwrap();
        conn.execute("INSERT INTO pm_wave_sync(wave_id,provider,initiative,synced_at) VALUES(?1,'linear',?2,7)",params![wave,payload["projects"][0]["initiative_ids"][0].as_str().unwrap()]).unwrap();
        conn.execute(
            "INSERT INTO pm_wave_projects(wave_id,project_id,position) VALUES(?1,'current',0)",
            [&wave],
        )
        .unwrap();
        let tables = [
            "task_prs",
            "agent_sessions",
            "task_workflows",
            "task_workflow_moves",
            "task_issue_identities",
            "pm_projects",
            "pm_items",
        ];
        let queries: Vec<_> = tables
            .iter()
            .map(|table| {
                let statement = conn.prepare(&format!("SELECT * FROM {table}")).unwrap();
                format!("SELECT {} FROM {table}", statement.column_names().join(","))
            })
            .collect();
        let rows = |conn: &rusqlite::Connection, sql: &str| {
            let mut query = conn.prepare(sql).unwrap();
            let columns = query.column_count();
            query
                .query_map([], |row| {
                    (0..columns)
                        .map(|i| {
                            row.get::<_, rusqlite::types::Value>(i)
                                .map(|value| match value {
                                    rusqlite::types::Value::Text(text) => {
                                        serde_json::from_str::<serde_json::Value>(&text)
                                            .map(|json| {
                                                rusqlite::types::Value::Text(json.to_string())
                                            })
                                            .unwrap_or(rusqlite::types::Value::Text(text))
                                    }
                                    other => other,
                                })
                        })
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        let before: Vec<_> = queries.iter().map(|sql| rows(&conn, sql)).collect();
        conn.execute_batch("PRAGMA foreign_keys=OFF; BEGIN IMMEDIATE;")
            .unwrap();
        conn.execute_batch(&migration_section_sql("local_planning"))
            .unwrap();
        let violations: i64 = conn
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(violations, 0);
        conn.execute_batch("COMMIT; PRAGMA foreign_keys=ON;")
            .unwrap();
        for ((table, sql), expected) in tables.iter().zip(&queries).zip(before) {
            assert_eq!(rows(&conn, sql), expected, "{table}");
        }
        upgrade_from_migration(&conn, "optional_task_pr");
        let order: String = conn
            .query_row(
                "SELECT task_order_json FROM pm_projects WHERE id='current'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let store = SqliteStore {
            conn: Arc::new(Mutex::new(conn)),
        };
        let observation: crate::test_linear_input::IssueFixture =
            loopflow_test_support::retained_task_input_in(
                &store.conn.lock().unwrap(),
                "/repo",
                "LOO-318",
            );
        assert_eq!(
            serde_json::to_value(observation.item).unwrap(),
            payload["items"][0]
        );
        assert_eq!(
            serde_json::to_value(observation.project.unwrap()).unwrap(),
            payload["projects"][0]
        );
        assert_eq!(observation.observed_at, 7);
        let imported = store.task_by_issue("LOO-318").unwrap().unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&order).unwrap(),
            serde_json::json!([imported.id.as_str()])
        );
        assert_eq!(imported.project_id, Some(imported_project));
        assert!(imported.worktree.is_none());
        assert!(store.task_prs(&imported.id).unwrap().is_empty());
        assert_eq!(imported.pm_snapshot_synced_at, Some(7));
        let stored_state: (Option<String>, bool) = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT planning_state,planning_completed FROM tasks WHERE id=?1",
                [imported.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            stored_state.0.as_deref(),
            payload["items"][0]["state"].as_str()
        );
        assert_eq!(
            stored_state.1,
            payload["items"][0]["completed"].as_bool().unwrap()
        );
        let uuid = uuid::Uuid::parse_str(imported.id.as_str().trim_start_matches("task_")).unwrap();
        assert_eq!(uuid.get_version_num(), 4);
        assert_eq!(uuid.get_variant(), uuid::Variant::RFC4122);
        let retained = store.task(&task).unwrap().unwrap();
        assert_eq!(retained.id, task);
        assert_eq!(retained.project_id, Some(project.clone()));
        assert_eq!(retained.linear_id.unwrap().as_str(), "linear-task");
        assert_eq!(retained.identifier, "LOO-1");
        assert_eq!(retained.worktree.unwrap().to_str(), Some("/repo/task"));
        let project = store.project(&project).unwrap().unwrap();
        assert_eq!(project.linear_id.unwrap().as_str(), "linear-project");
        assert_eq!(project.description, "Retain KRs");
        assert_eq!(
            store.conn.lock().unwrap().query_row("SELECT issue_id,identifier FROM task_issue_identities WHERE wave_id=?1 AND identifier='LOO-2'", [&orphan_wave], |row| Ok((row.get::<_, String>(0)?,row.get::<_, String>(1)?))).optional().unwrap(),
            Some(("orphan-issue".into(), "LOO-2".into()))
        );
        assert!(store.task_by_issue("LOO-2").unwrap().is_none());
    }
}

#[cfg(test)]
#[path = "linear_observation_tests.rs"]
mod linear_observation_tests;
