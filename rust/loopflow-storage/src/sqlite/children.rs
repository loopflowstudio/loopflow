//! SQLite persistence for Project and Tasks.

use super::planning_write;

use crate::work::edit::WorkValue as Edit;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// Product writes read before they write (for example, validating parent Work),
// so a deferred transaction has to upgrade its read lock to a write
// lock. Under WAL, SQLite fails that upgrade immediately rather than waiting —
// `busy_timeout` is never consulted, because waiting on an upgrade can deadlock
// two upgraders. Beginning IMMEDIATE takes the write lock up front, where
// `busy_timeout` does apply, so a second `lf` process queues instead of dying
// with `database is locked`.
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, TransactionBehavior};
use time::OffsetDateTime;

use crate::execution::TaskState;
use crate::planning::{LinearIssueId, LinearProjectId};
use crate::rows::now_unix;
use crate::work::project::{Project, ProjectId};
use crate::work::task::NewTask;
use crate::work::task::{
    CiObservation, GithubObservation, GithubPr, LinearWritebackState, PrMergeRequest, PrPhase,
    PrPresentation, PrPublication, Task, TaskEvent, TaskEventKind, TaskId, TaskPr, TaskPrId,
    TaskTrackerCursor, TrackerEventOutcome,
};
use crate::{PlanningState, StoreError, StoreResult};
use loopflow_model::durable::{Author, RepositoryId};
use loopflow_model::id::WaveId;

use super::{SqliteStore, TaskCheckout};

/// What completing a parent did with one child, in the order they are reported.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ChildOutcome {
    /// Its PR had merged.
    Done,
    /// It was not merged and was canceled with its parent.
    Abandoned,
}

#[derive(Debug)]
pub struct ChildCompletion {
    pub task: Task,
    pub outcome: ChildOutcome,
    pub pr: Option<TaskPr>,
    pub step: Option<String>,
    /// What settling this child did with its own children.
    pub children: Vec<ChildCompletion>,
}

#[derive(Debug)]
pub enum CompletionSettlement {
    Settled(Vec<ChildCompletion>),
    WaitingForChildren,
    Superseded,
}

const BRANCH_TASK_IDS: &str = "SELECT task_id FROM task_checkouts WHERE branch=?1
    UNION SELECT task_id FROM task_prs WHERE branch=?1";

/// Record the checkout this Machine has for a Task, or that it has none.
/// Work already running in the checkout keeps the Task it started with: a
/// Session joins a Task when it starts there or by `lf session bind`.
fn write_task_checkout_in(conn: &Connection, task: &Task) -> StoreResult<()> {
    let Some(root) = task
        .worktree
        .as_ref()
        .map(|path| path.display().to_string())
    else {
        conn.execute(
            "DELETE FROM task_checkouts WHERE task_id=?1",
            [task.id.as_str()],
        )?;
        return Ok(());
    };
    conn.execute(
        "INSERT INTO task_checkouts(task_id,worktree,branch,base_commit,parent_pr_id)
         VALUES(?1,?2,?3,?4,?5) ON CONFLICT(task_id) DO UPDATE SET
         worktree=excluded.worktree,branch=excluded.branch,
         base_commit=excluded.base_commit,parent_pr_id=excluded.parent_pr_id",
        params![
            task.id.as_str(),
            root,
            task.branch,
            task.base_commit,
            task.parent_pr_id.as_ref().map(TaskPrId::as_str)
        ],
    )?;
    Ok(())
}

fn task_creation_in(conn: &Connection, task: &TaskId) -> StoreResult<Option<NewTask>> {
    conn.query_row(
        "SELECT project_id,title,description,due_date,repository_id,parent_task_id FROM task_creation_intents WHERE task_id=?1",
        [task.as_str()],
        |row| {
            Ok(NewTask {
                parent_task_id: row.get::<_, Option<String>>(5)?.map(TaskId::from_raw),
                due_date: row.get(3)?,
                id: task.clone(),
                project_id: row.get::<_, Option<String>>(0)?.map(ProjectId::from_raw),
                repository_id: loopflow_model::durable::RepositoryId::parse(&row.get::<_, String>(4)?).map_err(|error| invalid_column(4, error))?,
                title: row.get(1)?,
                description: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(StoreError::from)
}

// Shared writer for new Tasks and durable filing reservations.
/// `wave`: the Wave the Task is filed in, whose Project it joins.
pub(super) fn create_task_in(
    conn: &Connection,
    input: &NewTask,
    wave: Option<&WaveId>,
) -> StoreResult<Task> {
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
    let (project_id, wave_id) = match (wave, &input.project_id) {
        (Some(wave), _) => (Some(wave_project_in(conn, wave)?), Some(wave.clone())),
        (None, Some(project)) => {
            let wave = conn
                .prepare_cached("SELECT wave_id FROM projects WHERE id=?1")?
                .query_row([project.as_str()], |row| row.get(0))
                .optional()?;
            (Some(project.clone()), wave)
        }
        (None, None) => (None, None),
    };
    let project = project_id.as_ref().map(ProjectId::as_str);
    let rank: u32 = conn.query_row(
        "SELECT COALESCE(max(planning_rank)+1,0) FROM tasks WHERE repository_id=?1 AND project_id IS ?2",
        params![input.repository_id.as_str(), project], |row| row.get(0),
    )?;
    let now = OffsetDateTime::now_utc();
    let task = Task {
        id: input.id.clone(),

        revision: 0,
        tracker_id: None,
        identifier: format!("lf-{}", input.id.as_str().trim_start_matches("task_")),
        title: input.title.clone(),
        description: input.description.clone(),
        tracker_synced_at: None,

        tracker_writeback: LinearWritebackState::Current,
        wave_id,
        repository_id: input.repository_id.clone(),
        project_id,
        parent_task_id: input.parent_task_id.clone(),
        worktree: None,
        workspace_slug: String::new(),
        branch: String::new(),
        base_commit: String::new(),
        parent_pr_id: None,
        created_at: now,
        updated_at: now,
        rank,
        assignee: None,
        due_date: input.due_date.clone(),
        state: Some("unstarted".into()),
        completed_at: None,
        observation: crate::work::task::Observation::NotRequired,
    };
    validate_task(&task)?;
    insert_task_row(conn, &task)?;
    conn.execute(
        "INSERT INTO task_creation_intents(task_id, project_id, title, description, due_date, repository_id, parent_task_id)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            input.id.as_str(),
            input.project_id.as_ref().map(ProjectId::as_str),
            input.title,
            input.description,
            input.due_date,
            input.repository_id.as_str(),
            input.parent_task_id.as_ref().map(TaskId::as_str)
        ],
    )?;
    task_on(conn, &input.id)?.ok_or(StoreError::NotFound)
}

/// The plan carries the Task's branch, so whoever runs it next checks out
/// the same branch and finds the same PR. Remote trackers supply their branch
/// when the Task is fetched. An already-recorded branch writes nothing.
fn record_task_branch_in(
    tx: &Connection,
    task: &TaskId,
    branch: &str,
    planning_connected: bool,
) -> StoreResult<()> {
    let recorded: Option<String> = tx.query_row(
        "SELECT planning_branch_name FROM tasks WHERE id=?1",
        [task.as_str()],
        |row| row.get(0),
    )?;
    if recorded.as_deref() == Some(branch) || planning_connected {
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

fn place_task_in(
    conn: &Connection,
    task_id: &TaskId,
    worktree: &Path,
    workspace_slug: &str,
    pr: &TaskPr,
    me: &super::task_ownership::OwnershipFacts,
) -> StoreResult<()> {
    let mut task = task_on(conn, task_id)?.ok_or(StoreError::NotFound)?;
    if task.worktree.is_some() {
        return Ok(());
    }
    if super::durable::task_state_in(conn, task_id)?.is_terminal() {
        return Err(StoreError::InvalidAuthority(
            "terminal Task cannot allocate a checkout".into(),
        ));
    }
    require_task_planning(conn, &task)?;
    // Allocating a checkout is the actual work reservation: it claims an
    // unowned Task, and refuses one owned on another Machine.
    super::task_ownership::claim_in(conn, task_id, me)?;
    record_task_branch_in(conn, task_id, &pr.branch, me.planning_connected())?;
    task.worktree = Some(worktree.to_path_buf());
    task.workspace_slug = workspace_slug.to_string();
    task.branch = pr.branch.clone();
    task.base_commit = pr.base_commit.clone();
    task.parent_pr_id = pr.parent_pr_id.clone();
    validate_initial_task_pr(&task, pr)?;
    conn.execute(
        "UPDATE tasks SET workspace_slug=?2,updated_at=?3 WHERE id=?1",
        params![task_id.as_str(), workspace_slug, now_unix()],
    )?;
    write_task_checkout_in(conn, &task)?;
    if pr.publication.is_some() {
        insert_task_pr(conn, pr)?;
    }
    seed_task_tracker_cursor(conn, &task)?;
    insert_task_event_in(
        conn,
        task_id,
        &TaskEventKind::CheckoutInitializing {
            branch: pr.branch.clone(),
            path: worktree.display().to_string(),
            base_commit: pr.base_commit.clone(),
        },
    )?;
    Ok(())
}

/// Take up the admitted Graph unless the Task has one, and record its
/// primary conversation. Returns the Graph's name and the conversation.
fn admit_in(
    conn: &Connection,
    task_id: &TaskId,
    admission: &super::creation::TaskAdmission,
) -> StoreResult<(String, crate::session::LfSession)> {
    if let Some(graph) = &admission.graph {
        conn.execute(
            "INSERT INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,?2,?3,?4)
             ON CONFLICT(task_id) DO NOTHING",
            params![task_id.as_str(), graph, super::task_work::START, now_unix()],
        )?;
    }
    let graph: String = conn
        .query_row(
            "SELECT json_extract(graph,'$.name') FROM task_graphs WHERE task_id=?1",
            [task_id.as_str()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| StoreError::InvalidData("admitted Task has no Graph".into()))?;
    let session = super::sessions::admit_task_primary_in(
        conn,
        task_id,
        &admission.session,
        admission.caller.as_ref(),
    )?;
    Ok((graph, session))
}

impl SqliteStore {
    /// Apply the placement transaction's admission rule before preparing Git work.
    pub fn validate_task_planning(&self, task: &Task) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        require_task_planning(&conn, task)
    }

    pub fn task_deleted(&self, task: &Task) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        task_deleted_on(&conn, task)
    }

    pub fn place_task_prepared(
        &self,
        task_id: &TaskId,
        worktree: &std::path::Path,
        workspace_slug: &str,
        pr: &TaskPr,
        me: &super::task_ownership::OwnershipFacts,
    ) -> StoreResult<Option<Task>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !me.matches_task_in(&tx, task_id)? {
            return Ok(None);
        }
        place_task_in(&tx, task_id, worktree, workspace_slug, pr, me)?;
        let task = task_on(&tx, task_id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(Some(task))
    }

    /// Commit promotion as one local effect. The caller holds checkout admission
    /// across source resolution and this write; no provider or filesystem is changed.
    #[allow(clippy::too_many_arguments)] // These effects share the promotion transaction.
    pub fn promote_task_prepared(
        &self,
        task_id: &TaskId,
        creation: Option<(&NewTask, Option<&WaveId>)>,
        placement: Option<&(PathBuf, TaskPr)>,
        session: Option<&crate::session::LfSession>,
        receipt: Option<&super::creation::TaskCreationRecord>,
        me: &super::task_ownership::OwnershipFacts,
        caller: &super::SessionCaller,
    ) -> StoreResult<Option<Task>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let matches = match creation {
            Some((input, wave)) => me.matches_promotion_in(&tx, input, wave)?,
            None => me.matches_task_in(&tx, task_id)?,
        };
        if !matches {
            return Ok(None);
        }
        if let Some(session) = session {
            super::sessions::require_current_actor_in(&tx, &session.id, caller)?;
        }
        if let Some((input, wave)) = creation {
            create_task_in(&tx, input, wave)?;
        }
        if let Some((path, pr)) = placement {
            let current = task_on(&tx, task_id)?.ok_or(StoreError::NotFound)?;
            if current
                .worktree
                .as_ref()
                .is_some_and(|existing| existing != path)
            {
                return Err(StoreError::InvalidAuthority(
                    "Task acquired another checkout before promotion; existing work is unchanged"
                        .into(),
                ));
            }
            // Completion may have arrived after source preparation. Binding still
            // selects the conversation, without allocating or reopening work.
            if !super::durable::task_state_in(&tx, task_id)?.is_terminal() {
                place_task_in(&tx, task_id, path, &pr.slug, pr, me)?;
                insert_task_event_in(
                    &tx,
                    task_id,
                    &TaskEventKind::CheckoutReady {
                        branch: pr.branch.clone(),
                        base_commit: pr.base_commit.clone(),
                    },
                )?;
            }
        }
        let admission = receipt.and_then(|receipt| receipt.admission.as_ref());
        if session.is_some() || admission.is_some() {
            super::task_ownership::claim_in(&tx, task_id, me)?;
        }
        if let Some(session) = session {
            // Transfer only the selected conversation out of its shared primary
            // scope. The checkout and historical captures retain their owners.
            tx.execute(
                "UPDATE agent_sessions SET primary_scope=NULL,
                 wave_id=CASE WHEN task_id IS NULL THEN NULL ELSE wave_id END
                 WHERE id=?1 AND primary_scope IN ('repository','wave')",
                [session.id.as_str()],
            )?;
            // Re-read the capture under the transaction: promotion changes only
            // prospective ownership, so a concurrent input need not be replayed.
            let current =
                super::sessions::session_in(&tx, &session.id)?.ok_or(StoreError::NotFound)?;
            if current.cwd != session.cwd {
                return Err(StoreError::InvalidAuthority(
                    "Session workspace changed before binding".into(),
                ));
            }
            super::sessions::bind_session_in(&tx, &current, task_id)?;
            super::sessions::choose_task_primary_in(&tx, task_id, &session.id)?;
        }
        let admitted = admission
            .map(|admission| admit_in(&tx, task_id, admission))
            .transpose()?;
        let task = task_on(&tx, task_id)?.ok_or(StoreError::NotFound)?;
        if let Some(receipt) = receipt {
            let (graph, session_id, milestone) = match admitted {
                Some((graph, conversation)) => (Some(graph), Some(conversation.id), "admitted"),
                None => (None, session.map(|s| s.id.clone()), "filed"),
            };
            let result = crate::sqlite::creation::TaskCreateReceipt {
                task: task.clone(),
                graph,
                session_id: session_id.clone(),
                milestone: milestone.into(),
            }
            .stored_result()?;
            super::creation::insert_in(
                &tx,
                &receipt.identity,
                Some(task_id.as_str()),
                session_id.as_ref().map(|id| id.as_str()),
                None,
                milestone,
                Some(&result),
            )?;
        }
        tx.commit()?;
        Ok(Some(task))
    }

    pub fn create_task(&self, input: &NewTask) -> StoreResult<Task> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task = create_task_in(&tx, input, None)?;
        tx.commit()?;
        Ok(task)
    }

    pub fn task_checkout(&self, task: &TaskId) -> StoreResult<Option<TaskCheckout>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        task_checkout_in(&conn, task)
    }

    pub fn task_checkout_paths(&self) -> StoreResult<Vec<(TaskId, PathBuf)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare("SELECT task_id, worktree FROM task_checkouts")?;
        let rows = statement.query_map([], |row| {
            Ok((
                TaskId::from_raw(row.get::<_, String>(0)?),
                PathBuf::from(row.get::<_, String>(1)?),
            ))
        })?;
        rows.collect::<rusqlite::Result<_>>().map_err(Into::into)
    }

    pub fn request_task_completion(
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

    /// Explicit completion settles the Task and its children together.
    pub fn request_family_completion(
        &self,
        task: &TaskId,
        note: Option<&str>,
    ) -> StoreResult<Option<i64>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let request = request_completion(&tx, task, note, true)?;
        tx.commit()?;
        Ok(request)
    }

    pub fn open_task_child_count(&self, task: &TaskId) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        open_task_child_count(&conn, task)
    }

    pub fn task_completion_pending(&self, task: &TaskId) -> StoreResult<Option<(i64, String)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT completion_request,COALESCE(completion_error,'Completion requested') FROM tasks WHERE id=?1 AND completion_request IS NOT NULL",
            [task.as_str()], |row| Ok((row.get(0)?,row.get(1)?))).optional()?)
    }

    /// Whether the Task completed at or after `since` (Unix seconds), even if
    /// it was reopened afterwards.
    pub fn task_completed_since(&self, task: &TaskId, since: i64) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_events WHERE task_id=?1
             AND json_extract(kind_json,'$.kind')='completed' AND created_at>=?2)",
            params![task.as_str(), since],
            |row| row.get(0),
        )?)
    }

    pub fn fail_task_completion(
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
    pub fn complete_task(&self, task: &Task, request: i64) -> StoreResult<CompletionSettlement> {
        validate_task(task)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_task_project(&tx, task)?;
        let state = super::durable::task_state_in(&tx, &task.id)?;
        if state == TaskState::Done {
            return Ok(CompletionSettlement::Settled(Vec::new()));
        }
        let (current, family, _) = completion_request(&tx, &task.id)?;
        if current != Some(request) {
            return Ok(CompletionSettlement::Superseded);
        }
        let current_pr = active_task_pr_on(&tx, &task.id)?;
        if current_pr.is_some_and(|pr| pr.phase() != PrPhase::Merged) {
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
        if !family && open_task_child_count(&tx, &task.id)? > 0 {
            clear_completion_request(&tx, &task.id)?;
            tx.commit()?;
            return Ok(CompletionSettlement::WaitingForChildren);
        }
        record_task_completion(&tx, &task.id)?;
        let children = if family {
            complete_children(&tx, &task.id)?
        } else {
            Vec::new()
        };
        tx.commit()?;
        Ok(CompletionSettlement::Settled(children))
    }

    pub fn reopen_task(&self, id: &TaskId, note: Option<&str>) -> StoreResult<Task> {
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
            "SELECT planning_state IS 'completed',completion_request IS NOT NULL FROM tasks WHERE id=?1",
            [id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if completed || pending {
            tx.execute(
                "UPDATE tasks SET completion_request=NULL,completion_error=NULL WHERE id=?1",
                [id.as_str()],
            )?;
            let target = if has_prior_work(&tx, id)? {
                "started"
            } else {
                "unstarted"
            };
            super::task_state::decide_in(&tx, id, target)?;
            // Reopening clears auto: it ended with the Task.
            tx.execute("DELETE FROM task_auto WHERE task_id=?1", [id.as_str()])?;
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

    pub fn task_tracker_state(&self, id: &TaskId) -> StoreResult<crate::TrackerState> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let task = task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        let result = task_tracker_state_in(&tx, &task, true)?;
        tx.commit()?;
        Ok(result)
    }

    pub fn task(&self, task_id: &TaskId) -> StoreResult<Option<Task>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        task_on(&conn, task_id)
    }

    pub fn task_issue_url(&self, id: &TaskId) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT planning_url FROM tasks WHERE id=?1",
            [id.as_str()],
            |row| row.get(0),
        )?)
    }

    pub fn task_issue_branch(&self, id: &TaskId) -> StoreResult<Option<String>> {
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
        match resolve_task_id_in(&snapshot, issue, None, None)? {
            Some(id) => task_on(&snapshot, &id),
            None => Ok(None),
        }
    }

    pub fn task_identity_candidates(
        &self,
        issue: &str,
    ) -> StoreResult<Vec<(TaskId, String, Option<String>)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        task_candidates_in(&conn, issue, None)
    }

    /// Resolve a selector sent by a peer in the shared repository.
    pub fn repository_task_id(
        &self,
        repository: &RepositoryId,
        issue: &str,
    ) -> StoreResult<Option<TaskId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        unique_task_id(issue, task_candidates_in(&conn, issue, Some(repository))?)
    }

    /// Resolve from one saved snapshot, retrying changed scope above SQL.
    pub fn resolve_task_id(
        &self,
        issue: &str,
        scope: Option<&TaskRepositoryScope>,
    ) -> StoreResult<Option<TaskId>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        resolve_task_id_in(
            &snapshot,
            issue,
            scope.map(|scope| scope.matching.as_slice()),
            scope.map(|scope| scope.saved.as_slice()),
        )
    }

    pub fn task_by_branch(&self, branch: &str) -> StoreResult<Option<Task>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        let query = format!("{TASK_COLUMNS} WHERE c.branch=?1");
        let mut statement = conn.prepare(&query)?;
        let tasks = statement
            .query_map(params![branch], |row| map_task_row(&conn, row))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        resolve_current_task(branch, tasks)
    }

    /// Match current checkout or retained delivery branches without hydrating unrelated history.
    pub fn historical_branch_task(&self, branch: &str) -> StoreResult<Option<Task>> {
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

    /// Tasks that are neither done nor abandoned: the live plan.
    pub fn list_open_tasks(&self, wave_id: Option<&WaveId>) -> StoreResult<Vec<Task>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        tasks_where(&conn, wave_id, &super::durable::task_open_sql("t"))
    }

    /// The Tasks filed from `parent`, in filing order, open or closed.
    pub fn task_children(&self, parent: &TaskId) -> StoreResult<Vec<Task>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        children_where(&conn, parent, TASK_VISIBLE)
    }

    /// Closed Tasks of one repository (and Wave), newest first, `limit` at a
    /// time, back to those closed at `since` (RFC 3339, UTC, whole seconds).
    pub fn closed_tasks_page(
        &self,
        repository: &loopflow_model::durable::RepositoryId,
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
        repository: &loopflow_model::durable::RepositoryId,
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
                "SELECT count(*) FROM tasks t JOIN projects p ON p.id=t.project_id
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
        parent_pr_id: Option<&TaskPrId>,
        updated_at: OffsetDateTime,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE task_checkouts SET base_commit=?2, parent_pr_id=?3 WHERE task_id=?1",
            params![
                task_id.as_str(),
                new_base,
                parent_pr_id.map(TaskPrId::as_str)
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        tx.execute(
            "UPDATE tasks SET updated_at=?2 WHERE id=?1",
            params![task_id.as_str(), updated_at.unix_timestamp()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Select a dependency and its comparison fork without moving Git or GitHub.
    pub fn stack_task_placement(
        &self,
        expected: &Task,
        parent_id: &TaskPrId,
        base_commit: &str,
    ) -> StoreResult<()> {
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
        if parent.is_settled() {
            return Err(StoreError::InvalidAuthority(
                "stack parent PR has merged or closed; it has no live branch to stack on".into(),
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
        if tx.execute(
            "UPDATE task_checkouts SET parent_pr_id=?2, base_commit=?3 WHERE task_id=?1",
            params![child.id.as_str(), parent_id.as_str(), base_commit],
        )? == 0
        {
            return Err(StoreError::InvalidAuthority(
                "Task is not checked out on this Machine; stack it where it runs".into(),
            ));
        }
        tx.execute(
            "UPDATE tasks SET updated_at=?2 WHERE id=?1",
            params![child.id.as_str(), now_unix()],
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

    pub fn settle_task_pr_merged(
        &self,
        settled: &TaskPr,
        merged_at: Option<OffsetDateTime>,
    ) -> StoreResult<crate::TaskPrMergeEvidenceOutcome> {
        validate_task_pr_settlement(settled)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let outcome = settle_task_pr_merged_in(&transaction, settled, merged_at)?;
        transaction.commit()?;
        Ok(outcome)
    }

    pub fn task_tracker_cursor(&self, task_id: &TaskId) -> StoreResult<Option<TaskTrackerCursor>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT task_id, last_revision, last_title, last_description,
                    last_success_at, degraded_reason, updated_at
             FROM task_linear_observations WHERE task_id=?1",
            params![task_id.as_str()],
            map_task_tracker_cursor_row,
        )
        .optional()
        .map_err(StoreError::from)
    }

    /// Persist one Linear observation as Task direction, atomically. Exactly-once
    /// comments are imported on the first read and deduplicated by revision.
    /// Issue revisions guard definition changes independently of comments.
    pub fn apply_tracker_event_prepared(
        &self,
        task_id: &TaskId,
        event: &crate::planning::TrackerEvent,
        observed_at: OffsetDateTime,
        planning_connected: bool,
        machine_names: &std::collections::HashMap<loopflow_model::durable::MachineId, String>,
    ) -> StoreResult<TrackerEventOutcome> {
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
        for comment in &event.comments {
            if !super::task_comments::ingest_task_comment(
                &transaction,
                task_id,
                comment,
                machine_names,
            )? {
                continue;
            }
            if !loopflow_model::comment::is_direction_comment(
                &comment.body,
                comment.author_id.as_deref(),
            ) {
                continue;
            }
            let comment_id = loopflow_model::comment::comment_revision_id(
                &comment.id,
                comment.revision.as_deref(),
            );
            let text = loopflow_model::comment::render_comment(
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
        super::task_ownership::reassert_in(&transaction, task_id, planning_connected)?;

        let Some((last_revision, last_title, last_description)) = existing else {
            // Baseline the definition; existing comments were imported above.
            transaction.execute(
                "INSERT INTO task_linear_observations (
                    task_id, last_revision, last_title, last_description,
                    last_success_at, degraded_reason, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?5)",
                params![
                    task_id.as_str(),
                    event.revision,
                    event.title,
                    event.description,
                    observed_at,
                ],
            )?;
            transaction.commit()?;
            return Ok(TrackerEventOutcome {
                baselined: true,
                content_steer_applied: false,
                follow_ups_created,
            });
        };

        // Monotonic guard: an out-of-order response older than what we have
        // carries stale content, so drop it rather than let it revert direction.
        if event.revision.as_str() < last_revision.as_str() {
            transaction.commit()?;
            return Ok(TrackerEventOutcome {
                baselined: false,
                content_steer_applied: false,
                follow_ups_created,
            });
        }

        let content_steer_applied =
            last_title != event.title || last_description != event.description;
        if content_steer_applied {
            let text = format!(
                "The linked planning task was edited; use this current definition.\n\n\
                 Title: {}\n\n{}",
                event.title, event.description,
            );
            Self::save_task_steer_in(
                &transaction,
                task_id,
                &Author::User,
                &text,
                &format!("task-definition:{task_id}"),
                &event.revision,
            )?;
        }

        transaction.execute(
            "UPDATE task_linear_observations
             SET last_revision=?2, last_title=?3, last_description=?4,
                 last_success_at=?5, degraded_reason=NULL, updated_at=?5
             WHERE task_id=?1",
            params![
                task_id.as_str(),
                event.revision,
                event.title,
                event.description,
                observed_at,
            ],
        )?;
        transaction.commit()?;
        Ok(TrackerEventOutcome {
            baselined: false,
            content_steer_applied,
            follow_ups_created,
        })
    }

    /// Record that the latest event failed, without moving the cursor. A
    /// Task with no baseline yet has no row to mark, which is fine — status
    /// then simply shows no event.
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
        Ok(seconds.map(crate::rows::unix_to_datetime))
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

    // Projects group a Wave's Tasks and deliberately own no worktree.

    /// Save `project` as the store holds it, the Wave's own Project included;
    /// nothing is queued for the tracker. Fixtures seed saved Projects this way.
    #[cfg(any(test, feature = "test-support"))]
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
                        project.tracker_id.as_ref().map(|id| id.as_str().to_owned()),
                    ),
                    Edit::ProjectSlug(Some(project.slug.clone())),
                    Edit::ProjectName(Some(project.name.clone())),
                    Edit::ProjectSummary(project.summary.clone()),
                    Edit::ProjectDescription(Some(project.description.clone())),
                    Edit::ProjectStatus(project.status),
                ],
            ),
            crate::sqlite::planning_write::WriteOrigin::Tracker,
        )?;
        transaction.execute(
            "UPDATE projects SET pm_snapshot_synced_at=?2,created_at=?3,updated_at=?4 WHERE id=?1",
            params![
                project.id.as_str(),
                project.tracker_synced_at,
                project.created_at.unix_timestamp(),
                project.updated_at.unix_timestamp()
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn project(&self, project_id: &ProjectId) -> StoreResult<Option<Project>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(project_in(&conn, project_id).optional()?)
    }

    pub fn project_by_tracker_id(&self, external: &str) -> StoreResult<Option<Project>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let project = conn
            .prepare_cached(&format!(
                "{PROJECT_COLUMNS} JOIN waves w ON w.id=p.wave_id
                 WHERE p.external_project_id=?1 AND w.retired_at IS NULL"
            ))?
            .query_row(
                [loopflow_model::id::canonical_uuid(external)],
                map_project_row,
            )
            .optional()?;
        Ok(project)
    }

    /// Tracker actions compare membership by local Project ID (even after retirement)
    /// or, for a Project never linked here, by the observed ID.
    pub fn observed_project_id(&self, external: &str) -> StoreResult<String> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT COALESCE((SELECT id FROM projects WHERE external_project_id=?1), ?2)",
            params![loopflow_model::id::canonical_uuid(external), external],
            |row| row.get(0),
        )?)
    }

    /// The Wave's Project, which its Tasks are in; for a Wave whose Project
    /// row is not recorded yet, the id that Project is made with.
    pub fn wave_project(&self, wave: &WaveId) -> StoreResult<ProjectId> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(held_wave_project_in(&conn, wave)?.unwrap_or_else(|| ProjectId::for_wave(wave)))
    }

    /// The Wave's Project record, once one has been made.
    pub fn project_of_wave(&self, wave: &WaveId) -> StoreResult<Option<Project>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        match held_wave_project_in(&conn, wave)? {
            Some(id) => Ok(Some(project_in(&conn, &id)?)),
            None => Ok(None),
        }
    }

    pub fn list_projects(&self) -> StoreResult<Vec<Project>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement =
            conn.prepare(&format!("{PROJECT_COLUMNS} ORDER BY wave_id,created_at,id"))?;
        let projects = statement
            .query_map([], map_project_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(projects)
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
        "SELECT planning_state FROM tasks WHERE id=?1",
        [task.id.as_str()],
        |row| row.get(0),
    )?;
    // Upstream cancellation stops a Task that hasn't set out; one under way keeps going.
    if crate::planning::terminal_reason(state.as_deref()).is_some() && !under_way(conn, task)? {
        return Err(StoreError::InvalidAuthority(
            "terminal planning state cannot start work; its execution history is preserved".into(),
        ));
    }
    Ok(())
}

/// Open and past its Graph's `start`.
fn under_way(conn: &Connection, task: &Task) -> StoreResult<bool> {
    Ok(!matches!(
        super::durable::task_state_in(conn, &task.id)?,
        TaskState::Done | TaskState::Abandoned
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
    if matches!(
        task_tracker_state_in(conn, task, false)?.state,
        PlanningState::Invalid | PlanningState::Removed
    ) {
        return Err(StoreError::InvalidAuthority(format!(
            "Task planning has invalid ownership, invalidation or removal evidence; run `lf task sync {}`",
            task.identifier
        )));
    }
    Ok(())
}

// Read only the retained facts that affect admission. Content belongs to Work;
// an unrelated malformed provider content field cannot erase membership or age.
fn task_tracker_state_in(
    conn: &Connection,
    task: &Task,
    allow_pending_membership: bool,
) -> StoreResult<crate::TrackerState> {
    tracker_states_in(conn, &[task], allow_pending_membership)?
        .remove(&task.id)
        .ok_or(StoreError::NotFound)
}

/// Each Task's tracker state, in one statement.
pub(super) fn tracker_states_in(
    conn: &Connection,
    tasks: &[&Task],
    allow_pending_membership: bool,
) -> StoreResult<HashMap<TaskId, crate::TrackerState>> {
    let ids = ids_json(tasks.iter().map(|task| task.id.as_str()));
    let linear: std::collections::HashSet<&TaskId> = tasks
        .iter()
        .filter(|task| task.tracker_id.is_some())
        .map(|task| &task.id)
        .collect();
    let mut statement = conn.prepare(
            "SELECT t.id,i.observed_at,
         COALESCE(i.needs_refresh,0) OR COALESCE(observed.archived,0)
         OR COALESCE(parent.archived,0)
         OR CASE WHEN i.id IS NULL THEN 0 ELSE
             -- A pending membership move is sent last, so it wins over the
             -- observed Project and its team until delivery.
             (NOT (?2 AND EXISTS(
                 SELECT 1 FROM tracker_actions e WHERE e.task_id=t.id AND e.subject='task_project'
                 AND e.kind='field' AND e.phase IN ('queued','sent')))
              AND ((pw.retired_at IS NULL AND i.project_id IS NOT p.external_project_id)
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
         t.planning_deleted_at IS NOT NULL OR COALESCE(changes.removed,0),
         changes.issue_id IS NOT NULL
         FROM tasks t JOIN repository_plans w ON w.id=t.repository_id LEFT JOIN projects p ON p.id=t.project_id
         LEFT JOIN waves pw ON pw.id=p.wave_id
         LEFT JOIN tracker_items i ON i.repo=w.repo AND i.tracker IN ('linear','asana') AND i.id=t.external_issue_id
         LEFT JOIN tracker_projects parent ON parent.repo=w.repo AND parent.tracker IN ('linear','asana')
                                     AND parent.id=p.external_project_id
         LEFT JOIN tracker_projects observed ON observed.repo=i.repo AND observed.tracker=i.tracker
                                       AND observed.id=i.project_id
         LEFT JOIN tracker_events changes ON changes.issue_id=t.external_issue_id
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
        Ok((
            id,
            crate::TrackerState {
                state,
                synced_at: observed_at,
            },
        ))
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

/// The receipt applies regardless of planning ownership.
fn task_deleted_on(conn: &Connection, task: &Task) -> StoreResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND planning_deleted_at IS NOT NULL)",
        [task.id.as_str()],
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
                Edit::TaskParent(task.parent_task_id.as_ref().map(ToString::to_string)),
                Edit::TaskLinearId(task.tracker_id.as_ref().map(|id| id.as_str().to_owned())),
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
        crate::sqlite::planning_write::WriteOrigin::Local,
    )?;
    conn.execute("UPDATE tasks SET pm_snapshot_synced_at=?2,workspace_slug=?3,created_at=?4,updated_at=?5 WHERE id=?1",params![
        task.id.as_str(),task.tracker_synced_at,task.workspace_slug,
        task.created_at.unix_timestamp(),task.updated_at.unix_timestamp()])?;
    write_task_checkout_in(conn, task)
}

/// Seed the Linear observation cursor from the planning directive, in the Task's
/// creation transaction, so the first observed edit becomes direction rather than
/// a baseline. The empty revision lets any Linear `updatedAt` advance the cursor.
fn seed_task_tracker_cursor(conn: &Connection, task: &Task) -> StoreResult<()> {
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
    let path: Option<String> = conn
        .query_row(
            "SELECT worktree FROM task_checkouts WHERE task_id=?1",
            [task.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    let Some(path) = path else {
        return Ok(None);
    };
    Ok(Some(TaskCheckout {
        task_id: task.clone(),
        issue_identifier: short_task_id(conn, task)?.ok_or(StoreError::NotFound)?,
        worktree: path.into(),
    }))
}

pub(super) const TASK_VISIBLE: &str = "t.planning_deleted_at IS NULL";
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
    // Task IDs are canonical lowercase hex, so the six-character prefix is a
    // primary-key range rather than a scan of every Task.
    let mut query =
        conn.prepare("SELECT substr(id,6) FROM tasks WHERE id!=?1 AND id>=?2 AND id<?2||'~'")?;
    let others = query
        .query_map(params![id.as_str(), format!("task_{}", &hex[..6])], |row| {
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
    p.wave_id, c.worktree, t.workspace_slug,
    t.created_at, t.updated_at, t.pm_snapshot_synced_at,
    COALESCE((SELECT json_object('state','pending','operation',
        CASE json_extract(e.value_json,'$.value.planning_state') WHEN 'completed' THEN 'complete_task' WHEN 'canceled' THEN 'cancel_task' ELSE 'reopen_task' END,
        'error',COALESCE(e.error,'Saved locally; pending planning synchronization'))
        FROM tracker_actions e WHERE e.task_id=t.id AND e.subject IS NULL AND e.kind='state'
            AND e.phase IN ('queued','sent') AND t.external_issue_id IS NOT NULL
        ORDER BY e.seq DESC LIMIT 1),
        json_object('state','current')),
    t.project_id, t.planning_revision, COALESCE(c.branch,''), COALESCE(c.base_commit,''), c.parent_pr_id,
    t.planning_rank,t.planning_assignee,t.planning_due_date,t.planning_state,t.planning_completed_at,t.repository_id,t.parent_task_id
    FROM tasks t LEFT JOIN projects p ON p.id=t.project_id
    LEFT JOIN task_checkouts c ON c.task_id=t.id";
pub(super) const TASK_PR_COLUMNS: &str = "SELECT
    id, task_id, sequence, slug, branch, base_commit,
    publication_requested_at, github_number, github_url,
    merge_commit, abandoned_at, created_at, updated_at,
    github_head_sha, ci_observation, parent_pr_id, github_observation,
    linear_attachment_id, linear_comment_id, linear_link_error,
    merge_mode, merge_requested_at, merge_head_sha,
    pr_title, pr_body, pr_copy_head_sha
    FROM task_prs";
const TASK_PR_SELECT: &str = "SELECT
    id, task_id, sequence, slug, branch, base_commit,
    publication_requested_at, github_number, github_url,
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
        "SELECT EXISTS(SELECT 1 FROM task_comments c JOIN tracker_actions e
         ON e.task_id=c.task_id AND e.subject=c.id AND e.kind='comment' WHERE c.id=?1 AND c.task_id=?2
         AND e.phase='confirmed'
         AND json_extract(e.value_json,'$.value.body')=c.body)",
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
#[derive(Debug)]
pub struct TaskRepositoryScope {
    pub saved: Vec<(TaskId, String, Option<String>)>,
    pub matching: Vec<String>,
}

fn task_candidates_in(
    conn: &Connection,
    issue: &str,
    repository: Option<&RepositoryId>,
) -> StoreResult<Vec<(TaskId, String, Option<String>)>> {
    let selector = loopflow_model::id::TaskSelector::parse(issue);
    let mut statement = conn.prepare(
        "SELECT t.id, COALESCE(t.issue_title, t.issue_identifier, t.id), w.repo FROM tasks t
         JOIN repository_plans w ON w.id=t.repository_id
         WHERE (?5 IS NULL OR t.repository_id=?5)
           AND (t.id=?1 OR t.external_issue_id=?3 OR t.issue_identifier=?4
            OR (t.id >= 'task_'||?2 AND t.id < 'task_'||?2||'~')
            -- A Task created here is sent with its own uuid as the provider id.
            OR t.id='task_'||replace(lower(?3),'-',''))
         ORDER BY t.id",
    )?;
    let tasks = statement
        .query_map(
            params![
                selector.id,
                selector.local_prefix,
                selector.external,
                selector.label,
                repository.map(RepositoryId::as_str),
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
    Ok(tasks)
}

pub(super) fn resolve_task_id_in(
    conn: &Connection,
    issue: &str,
    repositories: Option<&[String]>,
    expected: Option<&[(TaskId, String, Option<String>)]>,
) -> StoreResult<Option<TaskId>> {
    let mut tasks = task_candidates_in(conn, issue, None)?;
    if expected.is_some_and(|expected| tasks != expected) {
        return Err(StoreError::Stale("Task identity candidates changed".into()));
    }
    if let Some(repositories) = repositories {
        tasks.retain(|(_, _, owner)| {
            owner
                .as_ref()
                .is_some_and(|owner| repositories.contains(owner))
        });
    }
    unique_task_id(issue, tasks)
}

fn unique_task_id(
    issue: &str,
    mut tasks: Vec<(TaskId, String, Option<String>)>,
) -> StoreResult<Option<TaskId>> {
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
) -> StoreResult<crate::TaskPrMergeEvidenceOutcome> {
    let has_authority_column: bool = conn.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM pragma_table_info('task_prs') WHERE name='merged_at'
         )",
        [],
        |row| row.get(0),
    )?;
    if !has_authority_column {
        settle_task_pr_on(conn, settled)?;
        return Ok(crate::TaskPrMergeEvidenceOutcome::SchemaUnavailable);
    }

    let accepted_at = conn.query_row(
        "SELECT merged_at FROM task_prs WHERE id=?1",
        [settled.id.as_str()],
        |row| row.get::<_, Option<i64>>(0),
    )?;
    let observed_at = merged_at.map(OffsetDateTime::unix_timestamp);
    let outcome = match (accepted_at, observed_at) {
        (None, Some(_)) => crate::TaskPrMergeEvidenceOutcome::Accepted,
        (Some(accepted), Some(observed)) if accepted == observed => {
            crate::TaskPrMergeEvidenceOutcome::Repeated
        }
        (Some(accepted), Some(_)) => crate::TaskPrMergeEvidenceOutcome::Conflict {
            accepted_at: accepted,
        },
        (_, None) => crate::TaskPrMergeEvidenceOutcome::Missing,
    };

    settle_task_pr_on(conn, settled)?;
    if let crate::TaskPrMergeEvidenceOutcome::Conflict { accepted_at } = outcome {
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

        revision: row.get::<_, i64>(12)? as u64,
        tracker_id: row
            .get::<_, Option<String>>(1)?
            .map(LinearIssueId::from_raw),
        identifier,
        title: row.get(2)?,
        description: row.get(3)?,
        tracker_synced_at: row.get(9)?,

        tracker_writeback: serde_json::from_str(&row.get::<_, String>(10)?)
            .map_err(|error| invalid_column(10, error))?,
        wave_id: row.get(4)?,
        project_id: row.get::<_, Option<String>>(11)?.map(ProjectId::from_raw),
        parent_task_id: row.get::<_, Option<String>>(22)?.map(TaskId::from_raw),
        repository_id: loopflow_model::durable::RepositoryId::parse(&row.get::<_, String>(21)?)
            .map_err(|error| invalid_column(21, error))?,
        worktree: row.get::<_, Option<String>>(5)?.map(PathBuf::from),
        workspace_slug: row.get(6)?,
        branch: row.get(13)?,
        base_commit: row.get(14)?,
        parent_pr_id: row.get::<_, Option<String>>(15)?.map(TaskPrId::from_raw),
        created_at: crate::rows::unix_to_datetime(row.get(7)?),
        updated_at: crate::rows::unix_to_datetime(row.get(8)?),
        rank: row.get(16)?,
        assignee: row.get(17)?,
        due_date: row.get(18)?,
        state: row.get(19)?,
        completed_at: row.get(20)?,
        // Runtime freshness is derived when reconciliation decides whether the
        // durable GitHub observation can be reused.
        observation: crate::work::task::Observation::NotRequired,
    })
}

pub(super) fn map_task_pr_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskPr> {
    let publication_requested_at = row.get::<_, Option<i64>>(6)?;
    let github_number = row.get::<_, Option<i64>>(7)?.map(|number| number as u32);
    let github_url = row.get::<_, Option<String>>(8)?;
    let github_head_sha = row.get::<_, Option<String>>(13)?;
    let ci_observation = row
        .get::<_, Option<String>>(14)?
        .map(|json| serde_json::from_str::<CiObservation>(&json))
        .transpose()
        .map_err(|error| invalid_column(14, error))?;
    let github_observation = row
        .get::<_, Option<String>>(16)?
        .map(|json| serde_json::from_str::<GithubObservation>(&json))
        .transpose()
        .map_err(|error| invalid_column(16, error))?;
    let merge = match (
        row.get::<_, Option<String>>(20)?,
        row.get::<_, Option<i64>>(21)?,
        row.get::<_, Option<String>>(22)?,
    ) {
        (Some(mode), Some(requested_at), Some(head_sha)) => Some(PrMergeRequest {
            mode: mode.parse().map_err(|error| invalid_column(20, error))?,
            requested_at: crate::rows::unix_to_datetime(requested_at),
            head_sha,
        }),
        (None, None, None) => None,
        _ => {
            return Err(invalid_column(
                20,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "PR merge request fields must all be present or absent",
                ),
            ))
        }
    };
    let presentation = match (
        row.get::<_, Option<String>>(23)?,
        row.get::<_, Option<String>>(24)?,
        row.get::<_, Option<String>>(25)?,
    ) {
        (Some(title), Some(body), Some(head_sha)) => Some(PrPresentation {
            title,
            body,
            head_sha,
        }),
        (None, None, None) => None,
        _ => {
            return Err(invalid_column(
                23,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "PR presentation fields must all be present or absent",
                ),
            ))
        }
    };
    let publication = match publication_requested_at {
        Some(requested_at) => Some(PrPublication {
            requested_at: crate::rows::unix_to_datetime(requested_at),
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
                        7,
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
        parent_pr_id: row.get::<_, Option<String>>(15)?.map(TaskPrId::from_raw),
        publication,
        merge_commit: row.get(9)?,
        abandoned_at: row
            .get::<_, Option<i64>>(10)?
            .map(crate::rows::unix_to_datetime),
        ci_observation,
        github_observation,
        linear_attachment_id: row.get::<_, Option<String>>(17)?,
        linear_comment_id: row.get::<_, Option<String>>(18)?,
        linear_link_error: row.get::<_, Option<String>>(19)?,
        created_at: crate::rows::unix_to_datetime(row.get(11)?),
        updated_at: crate::rows::unix_to_datetime(row.get(12)?),
    };
    pr.validate_persisted()
        .map_err(|error| invalid_column(6, error))?;
    Ok(pr)
}

fn map_task_tracker_cursor_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskTrackerCursor> {
    let last_success_at = OffsetDateTime::from_unix_timestamp(row.get::<_, i64>(4)?)
        .map_err(|error| invalid_column(4, error))?;
    let updated_at = OffsetDateTime::from_unix_timestamp(row.get::<_, i64>(6)?)
        .map_err(|error| invalid_column(6, error))?;
    Ok(TaskTrackerCursor {
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
        created_at: crate::rows::unix_to_datetime(row.get(3)?),
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

/// The Wave's one Project, made named for the Wave when it has none. Its id
/// comes from the Wave's, so Machines that each make it make the same record.
pub(crate) fn wave_project_in(conn: &Connection, wave: &WaveId) -> StoreResult<ProjectId> {
    if let Some(id) = held_wave_project_in(conn, wave)? {
        return Ok(id);
    }
    let name: String = conn.query_row("SELECT name FROM waves WHERE id=?1", [wave], |row| {
        row.get(0)
    })?;
    let id = ProjectId::for_wave(wave);
    planning_write::create(
        conn,
        "",
        &crate::work::edit::WorkEdit::for_target(
            id.as_str(),
            &[
                Edit::ProjectWave(wave.to_string()),
                Edit::ProjectSlug(Some(name.clone())),
                Edit::ProjectName(Some(name)),
                Edit::ProjectStatus(crate::work::project::ProjectStatus::Started),
            ],
        ),
        planning_write::WriteOrigin::Local,
    )?;
    Ok(id)
}

/// The Wave's Project, once one is made. A released writer can still add a
/// second Project to a Wave until the unique index on `projects.wave_id`
/// ships; the Wave's own record (`ProjectId::for_wave`) wins, else the oldest.
pub(crate) fn held_wave_project_in(
    conn: &Connection,
    wave: &WaveId,
) -> StoreResult<Option<ProjectId>> {
    Ok(conn
        .prepare_cached(
            "SELECT id FROM projects WHERE wave_id=?1
             ORDER BY id=?2 DESC, created_at, id LIMIT 1",
        )?
        .query_row(params![wave, ProjectId::for_wave(wave).as_str()], |row| {
            Ok(ProjectId::from_raw(row.get::<_, String>(0)?))
        })
        .optional()?)
}

const PROJECT_COLUMNS: &str = "SELECT
    p.id, p.external_project_id, p.project_slug, p.project_name, p.description,
    p.wave_id, p.pm_snapshot_synced_at,
    p.created_at, p.updated_at, p.status, p.project_summary
    FROM projects p";
pub(crate) fn project_in(conn: &Connection, id: &ProjectId) -> rusqlite::Result<Project> {
    conn.prepare_cached(&format!("{PROJECT_COLUMNS} WHERE id=?1"))?
        .query_row([id.as_str()], map_project_row)
}

fn map_project_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: ProjectId::from_raw(row.get::<_, String>(0)?),

        summary: row.get(10)?,
        tracker_id: row
            .get::<_, Option<String>>(1)?
            .map(LinearProjectId::from_raw),
        slug: row.get(2)?,
        name: row.get(3)?,
        description: row.get(4)?,
        tracker_synced_at: row.get(6)?,
        status: serde_json::from_value(serde_json::Value::String(row.get(9)?))
            .map_err(|error| invalid_column(9, error))?,

        wave_id: row.get(5)?,
        created_at: crate::rows::unix_to_datetime(row.get(7)?),
        updated_at: crate::rows::unix_to_datetime(row.get(8)?),
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
        created_at: crate::rows::unix_to_datetime(created_at),
    })
}

pub(super) fn open_task_children_where() -> String {
    format!("{} AND {TASK_VISIBLE}", super::durable::task_open_sql("t"))
}

pub(super) fn open_task_child_count(conn: &Connection, task: &TaskId) -> StoreResult<u32> {
    Ok(conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM tasks t WHERE t.parent_task_id=?1 AND {}",
            open_task_children_where()
        ),
        [task.as_str()],
        |row| row.get(0),
    )?)
}

fn completion_request(conn: &Connection, task: &TaskId) -> StoreResult<(Option<i64>, bool, bool)> {
    Ok(conn.query_row(
        "SELECT t.completion_request,
                COALESCE(json_extract(e.kind_json,'$.kind')='family_completion_requested',0),
                t.planning_state IS 'completed'
         FROM tasks t LEFT JOIN task_events e ON e.id=t.completion_request WHERE t.id=?1",
        [task.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?)
}

pub(super) fn request_completion_in(
    conn: &Connection,
    task: &TaskId,
    note: Option<&str>,
) -> StoreResult<Option<i64>> {
    request_completion(conn, task, note, false)
}

fn request_completion(
    conn: &Connection,
    task: &TaskId,
    note: Option<&str>,
    family: bool,
) -> StoreResult<Option<i64>> {
    let (pending, family_requested, completed) = completion_request(conn, task)?;
    if pending.is_some() && (!family || family_requested) {
        return Ok(pending);
    }
    if completed || (!family && open_task_child_count(conn, task)? > 0) {
        return Ok(None);
    }
    let reason = note.map(str::to_string);
    let event = if family {
        TaskEventKind::FamilyCompletionRequested { reason }
    } else {
        TaskEventKind::CompletionRequested { reason }
    };
    insert_task_event_in(conn, task, &event)?;
    let request = conn.last_insert_rowid();
    conn.execute(
        "UPDATE tasks SET completion_request=?2,completion_error=NULL WHERE id=?1",
        params![task.as_str(), request],
    )?;
    Ok(Some(request))
}

fn clear_completion_request(conn: &Connection, task: &TaskId) -> StoreResult<()> {
    conn.execute(
        "UPDATE tasks SET completion_request=NULL,completion_error=NULL WHERE id=?1",
        [task.as_str()],
    )?;
    Ok(())
}

fn record_task_completion(conn: &Connection, task: &TaskId) -> StoreResult<()> {
    clear_completion_request(conn, task)?;
    super::task_state::decide_in(conn, task, "completed")?;
    insert_task_event_in(
        conn,
        task,
        &TaskEventKind::Completed {
            summary: "Task completed".into(),
        },
    )?;
    super::task_summaries::write_in(conn, task, super::task_summaries::TaskEnd::Done)
}

/// The Tasks filed from `parent` that `filter` admits, in filing order.
fn children_where(conn: &Connection, parent: &TaskId, filter: &str) -> StoreResult<Vec<Task>> {
    let mut statement = conn.prepare(&format!(
        "{TASK_COLUMNS} WHERE t.parent_task_id=?1 AND {filter} ORDER BY t.created_at,t.rowid"
    ))?;
    // A few rows: each reads its own identifier, not every Task's.
    let children = statement
        .query_map([parent.as_str()], |row| map_task_row(conn, row))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(children)
}

/// Completing a Task means one thing by any path: a child closed here, done
/// or abandoned, settles its own children by the same rule.
fn complete_children(conn: &Connection, parent: &TaskId) -> StoreResult<Vec<ChildCompletion>> {
    let children = children_where(conn, parent, &open_task_children_where())?;
    let mut completions = Vec::with_capacity(children.len());
    for task in children {
        let pr = active_task_pr_on(conn, &task.id)?;
        let outcome = if pr.as_ref().is_some_and(|pr| pr.phase() == PrPhase::Merged) {
            record_task_completion(conn, &task.id)?;
            ChildOutcome::Done
        } else {
            super::durable::abandon_task_in(conn, &task.id, "parent Task completed", now_unix())?;
            ChildOutcome::Abandoned
        };
        let children = complete_children(conn, &task.id)?;
        let step = conn
            .query_row(
                "SELECT step FROM task_graphs WHERE task_id=?1 AND run_start IS NULL",
                [task.id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        completions.push(ChildCompletion {
            task,
            outcome,
            pr,
            step,
            children,
        });
    }
    completions.sort_by_key(|child| child.outcome);
    Ok(completions)
}

fn has_prior_work(conn: &Connection, task: &TaskId) -> StoreResult<bool> {
    if super::task_started::task_begun_in(conn, task)? {
        return Ok(true);
    }
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM task_prs WHERE task_id=?1)
            OR EXISTS(SELECT 1 FROM graph_processes WHERE task_id=?1)
            OR EXISTS(SELECT 1 FROM task_checkouts WHERE task_id=?1)
            OR EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=?1)",
        [task.as_str()],
        |row| row.get(0),
    )?)
}

pub(super) fn tasks_in(conn: &Connection, wave_id: Option<&WaveId>) -> StoreResult<Vec<Task>> {
    tasks_where(conn, wave_id, "1")
}

fn tasks_where(
    conn: &Connection,
    wave_id: Option<&WaveId>,
    condition: &str,
) -> StoreResult<Vec<Task>> {
    let filter = if wave_id.is_some() {
        "p.wave_id=?1 AND "
    } else {
        ""
    };
    let query = format!(
        "{TASK_COLUMNS} WHERE {filter}({condition}) AND {TASK_VISIBLE} ORDER BY t.updated_at DESC"
    );
    let identifiers = TaskIdentifiers::read(conn)?;
    let mut statement = conn.prepare(&query)?;
    let rows = statement.query_map(params_from_iter(wave_id.map(WaveId::as_str)), |row| {
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

#[cfg(any(test, feature = "test-support"))]
impl SqliteStore {
    pub fn seed_unplaced_task(&self, task: &Task) {
        let mut task = task.clone();
        task.worktree = None;
        task.workspace_slug.clear();
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().unwrap();
        insert_task_row(&tx, &task).unwrap();
        tx.commit().unwrap();
    }

    pub fn seed_task(
        &self,
        task: &Task,
        pr: &TaskPr,
        me: Option<&super::task_ownership::OwnershipFacts>,
    ) -> StoreResult<Task> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_task(task)?;
        validate_task_project(&tx, task)?;
        if task_on(&tx, &task.id)?.is_none() {
            insert_task_row(&tx, task)?;
        } else {
            tx.execute(
                "UPDATE tasks SET workspace_slug=?2 WHERE id=?1",
                params![task.id.as_str(), task.workspace_slug],
            )?;
            write_task_checkout_in(&tx, task)?;
        }
        insert_task_pr(&tx, pr)?;
        // A seeded checkout is placed work, which claims the Task like `place_task`.
        if let Some(me) = me {
            super::task_ownership::claim_in(&tx, &task.id, me)?;
        }
        seed_task_tracker_cursor(&tx, task)?;
        let saved = task_on(&tx, &task.id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(saved)
    }
}

#[cfg(test)]
mod local_planning_tests {

    use rusqlite::params;

    use loopflow_model::durable::{ProjectId, TaskId};
    use loopflow_model::id::WaveId;

    #[test]
    fn a_wave_is_made_with_one_project_and_its_tasks_are_in_it() {
        use crate::work::project::ProjectStatus;
        use crate::work::task::NewTask;
        use crate::work::wave::Wave;
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
        let billing = Wave::new(WaveId::new(), "billing".into(), "/repo".into());
        let refunds = Wave::new(WaveId::new(), "refunds".into(), "/repo".into())
            .with_parent(billing.id().clone());
        for wave in [&billing, &refunds] {
            crate::test_fixture::wave(&store, wave);
        }
        let project = |wave: &Wave| store.project_of_wave(wave.id()).unwrap().unwrap();
        let refunds_project = project(&refunds);
        assert_eq!(refunds_project.name, "refunds");
        assert_eq!(refunds_project.status, ProjectStatus::Started);
        assert_eq!(refunds_project.id, ProjectId::for_wave(refunds.id()));
        let billing_project = project(&billing).id;

        let repository = crate::test_fixture::repository(&store, "/repo");
        let task = {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            let task = super::create_task_in(
                &tx,
                &NewTask {
                    due_date: None,
                    id: TaskId::new(),
                    repository_id: repository.clone(),
                    project_id: None,
                    parent_task_id: None,
                    title: "x".into(),
                    description: String::new(),
                },
                Some(refunds.id()),
            )
            .unwrap();
            tx.commit().unwrap();
            task
        };
        let listed = |wave: &Wave| store.list_tasks(Some(wave.id())).unwrap().len();
        assert_eq!(task.project_id.as_ref(), Some(&refunds_project.id));
        assert_eq!(task.wave_id.as_ref(), Some(refunds.id()));
        assert_eq!((listed(&refunds), listed(&billing)), (1, 0));

        // An ended Project is still the Wave's; new Tasks join it.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE projects SET status='completed' WHERE id=?1",
                [refunds_project.id.as_str()],
            )
            .unwrap();
        assert_eq!(
            store.wave_project(refunds.id()).unwrap(),
            refunds_project.id
        );

        // A move takes the Task to the new Wave's Project, and out of every
        // Wave with none.
        store
            .move_task(&task.id, Some(&refunds_project.id), Some(billing.id()))
            .unwrap();
        let moved = store.task(&task.id).unwrap().unwrap();
        assert_eq!(
            (moved.wave_id.as_ref(), moved.project_id.as_ref()),
            (Some(billing.id()), Some(&billing_project))
        );
        assert_eq!((listed(&refunds), listed(&billing)), (0, 1));
        store
            .move_task(&task.id, Some(&billing_project), None)
            .unwrap();
        let moved = store.task(&task.id).unwrap().unwrap();
        assert_eq!((moved.wave_id, moved.project_id), (None, None));
    }

    #[test]
    fn tracker_project_lookup_requires_the_external_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
        let wave = crate::work::wave::Wave::new(WaveId::new(), "billing".into(), "/repo".into());
        crate::test_fixture::wave(&store, &wave);
        let mut project = store.project_of_wave(wave.id()).unwrap().unwrap();
        project.tracker_id = Some(crate::planning::LinearProjectId::from_raw(
            wave.id().as_str(),
        ));
        store.insert_project(&project).unwrap();

        let lookup = |id| store.project_by_tracker_id(id).unwrap().map(|p| p.id);
        assert_eq!(lookup(&project.slug), None);
        assert_eq!(lookup(project.id.as_str()), None);
        assert_eq!(
            lookup(&wave.id().to_string().to_uppercase()),
            Some(project.id)
        );
    }

    #[test]
    fn the_migration_leaves_every_live_wave_one_project_with_its_tasks() {
        use crate::migration_runner::{apply_before_migration, upgrade_from_migration};
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let (observers, build) = (
            crate::test_fixture::observers(),
            crate::test_fixture::build(),
        );
        apply_before_migration(&observers, &build, &conn, "remove_chapters");
        let (coverage, empty, retired) = (WaveId::new(), WaveId::new(), WaveId::new());
        let repository = loopflow_model::durable::RepositoryId::new();
        conn.execute(
            "INSERT INTO repository_plans(id,repo,selected) VALUES(?1,'/repo',1)",
            [repository.as_str()],
        )
        .unwrap();
        for (wave, name, retired_at) in [
            (&coverage, "coverage", None),
            (&empty, "empty", None),
            (&retired, "retired", Some(5)),
        ] {
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at,retired_at) VALUES(?1,?2,'/repo',1,?3)",
                params![wave, name, retired_at],
            )
            .unwrap();
        }
        // Newest is completed; the newest started one is kept.
        let (oldest, kept, newest) = (ProjectId::new(), ProjectId::new(), ProjectId::new());
        for (project, status, at) in [
            (&oldest, "started", 1),
            (&kept, "started", 2),
            (&newest, "completed", 3),
        ] {
            conn.execute(
                "INSERT INTO projects(id,wave_id,created_at,updated_at,description,status) VALUES(?1,?2,?3,?3,'',?4)",
                params![project.as_str(), coverage, at, status],
            )
            .unwrap();
        }
        let tasks: Vec<TaskId> = [&oldest, &kept, &newest]
            .into_iter()
            .map(|project| {
                let task = TaskId::new();
                conn.execute(
                    "INSERT INTO tasks(id,repository_id,project_id,issue_identifier,created_at) VALUES(?1,?2,?3,?1,1)",
                    params![task.as_str(), repository.as_str(), project.as_str()],
                )
                .unwrap();
                task
            })
            .collect();
        let old_remote = "former-chapter";
        conn.execute(
            "UPDATE projects SET external_project_id=?1 WHERE id=?2",
            params![old_remote, oldest.as_str()],
        )
        .unwrap();
        let membership =
            |project: &str| serde_json::json!({"field":"task_project","value":project});
        let unknown =
            serde_json::json!({"field":"future_field","value":{"chapter":oldest.as_str()}});
        let creation = serde_json::json!([unknown, membership(oldest.as_str())]);
        let observed = serde_json::json!({"revision":"r1","value":membership(oldest.as_str())});
        for (id, phase, kind, value) in [
            ("create", "queued", "create", &creation),
            ("move", "sent", "field", &membership(oldest.as_str())),
            ("lost", "conflicted", "field", &membership(newest.as_str())),
        ] {
            conn.execute(
                "INSERT INTO tracker_actions(id,task_id,subject,kind,value_json,phase,tracker,base_json,conflict_json)
                 VALUES(?1,?2,CASE WHEN ?3='field' THEN 'task_project' END,?3,?4,?5,'linear',
                 CASE WHEN ?3='field' THEN ?6 END,CASE WHEN ?5='conflicted' THEN ?6 END)",
                params![id, tasks[0].as_str(), kind, value.to_string(), phase, observed.to_string()],
            ).unwrap();
        }
        // A Project create released lf queued: the command that makes,
        // connects or syncs the Wave creates its Project now.
        conn.execute(
            "INSERT INTO tracker_actions(id,project_id,kind,value_json,phase,tracker)
             VALUES('made',?1,'create','[]','queued','linear')",
            [kept.as_str()],
        )
        .unwrap();
        upgrade_from_migration(&observers, &build, &conn, "remove_chapters");
        let made: String = conn
            .query_row(
                "SELECT phase FROM tracker_actions WHERE id='made'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(made, "retired");

        let actions: String = conn
            .query_row(
                "SELECT json_group_array(json_array(id,phase,json(value_json),
                 json_extract(base_json,'$.sent_project'),json_extract(base_json,'$.value.value'),
                 json_extract(conflict_json,'$.value.value')) ORDER BY seq)
             FROM tracker_actions WHERE task_id=?1",
                [tasks[0].as_str()],
                |row| row.get(0),
            )
            .unwrap();
        let desired = membership(kept.as_str());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&actions).unwrap(),
            serde_json::json!([
                ["create", "queued", [unknown, desired], null, null, null],
                ["move", "sent", desired, old_remote, old_remote, null],
                ["lost", "conflicted", desired, null, old_remote, old_remote],
            ])
        );

        let projects = |wave: &WaveId| -> Vec<String> {
            let mut query = conn
                .prepare("SELECT id FROM projects WHERE wave_id=?1")
                .unwrap();
            query
                .query_map([wave], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(projects(&coverage), [kept.to_string()]);
        for task in &tasks {
            let project: String = conn
                .query_row(
                    "SELECT project_id FROM tasks WHERE id=?1",
                    [task.as_str()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(project, kept.to_string());
        }
        assert_eq!(projects(&empty), [ProjectId::for_wave(&empty).to_string()]);
        let name: String = conn
            .query_row(
                "SELECT project_name FROM projects WHERE wave_id=?1",
                [&empty],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(name, "empty");
        assert!(projects(&retired).is_empty());
    }

    #[test]
    fn retired_branch_lookup_preserves_history_without_full_scans() {
        use crate::migration_runner::{apply_before_migration, migration_section_sql};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "retired_work_lookup",
        );
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
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/migrations/retired_work_lookup_task.sql"
                )),
                params![task.as_str(), project.as_str(), branch],
            )
            .unwrap();
            conn.execute("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at)
                VALUES(?1,?2,1,?3,?3,'base',1,1)", params![format!("pr-{n}"),task.as_str(),branch]).unwrap();
        }
        let read = || {
            // The lookup as `retired_work_lookup` shipped, before `task_checkouts`.
            let mut query = conn
                .prepare(
                    "SELECT id FROM tasks WHERE branch=?1
                     UNION SELECT task_id FROM task_prs WHERE branch=?1",
                )
                .unwrap();
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
}
