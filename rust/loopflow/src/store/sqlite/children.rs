//! SQLite persistence for Project and Tasks.

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

use crate::child::AbandonIntent;
use crate::durable::{Author, TaskState};
use crate::id::WaveId;
use crate::planning::{LinearIssueId, LinearProjectId, NewTask, ProjectPlan, TaskPlan};
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::work::project::{Project, ProjectEvent, ProjectEventKind, ProjectId};
use crate::work::task::{
    CiObservation, GithubObservation, GithubPr, LinearObservationOutcome, PmWritebackState,
    PrMergeRequest, PrPhase, PrPresentation, PrPublication, Task, TaskEvent, TaskEventKind, TaskId,
    TaskLinearObservation, TaskPr, TaskPrId, TaskPrRepairKind,
};

use super::durable::{inherit_project_placement, inherit_task_placement};
use super::SqliteStore;

fn task_creation_in(conn: &Connection, task: &TaskId) -> StoreResult<Option<NewTask>> {
    conn.query_row(
        "SELECT project_id,title,description FROM task_creation_intents WHERE task_id=?1",
        [task.as_str()],
        |row| {
            Ok(NewTask {
                id: task.clone(),
                project_id: ProjectId::from_raw(row.get::<_, String>(0)?),
                title: row.get(1)?,
                description: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(StoreError::from)
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
        super::durable::require_selected_project(&tx, &task.project_id)?;
        require_task_planning(&tx, &task)?;
        task.worktree = Some(worktree.to_path_buf());
        task.workspace_slug = workspace_slug.to_string();
        validate_initial_task_pr(&task, pr)?;
        tx.execute(
            "UPDATE tasks SET worktree=?2,workspace_slug=?3,updated_at=?4 WHERE id=?1",
            params![
                task_id.as_str(),
                worktree.display().to_string(),
                workspace_slug,
                now_unix()
            ],
        )?;
        inherit_task_placement(&tx, &task)?;
        insert_task_pr(&tx, pr)?;
        seed_task_linear_observation(&tx, &task)?;
        insert_task_event_in(
            &tx,
            task_id,
            &TaskEventKind::WorktreeInitializing {
                pr_id: pr.id.clone(),
                sequence: pr.sequence,
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
        if let Some(intent) = task_creation_in(&tx, &input.id)? {
            if intent != *input {
                return Err(StoreError::InvalidData(
                    "creation identity already belongs to a different request".into(),
                ));
            }
            return task_on(&tx, &input.id)?.ok_or(StoreError::NotFound);
        }
        if input.title.trim().is_empty() {
            return Err(StoreError::InvalidData("Task title cannot be empty".into()));
        }
        let wave_id: WaveId = tx.query_row(
            "SELECT wave_id FROM projects WHERE id=?1",
            [input.project_id.as_str()],
            |row| row.get(0),
        )?;

        super::durable::require_selected_project(&tx, &input.project_id)?;
        let now = OffsetDateTime::now_utc();
        let task = Task {
            id: input.id.clone(),
            plan: TaskPlan {
                revision: 0,
                linear_id: None,
                identifier: format!("lf-{}", input.id.as_str().trim_start_matches("task_")),
                title: input.title.clone(),
                description: input.description.clone(),
                pm_snapshot_synced_at: None,
            },
            pm_writeback: PmWritebackState::Current,
            wave_id,
            project_id: input.project_id.clone(),
            worktree: None,
            workspace_slug: String::new(),
            agent: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::work::task::Observation::NotRequired,
        };
        validate_task(&task)?;
        insert_task_row(&tx, &task)?;
        tx.execute("UPDATE tasks SET planning_state='unstarted',planning_rank=COALESCE((SELECT max(planning_rank)+1 FROM tasks WHERE project_id=?2 AND id!=?1),0) WHERE id=?1",params![task.id.as_str(),task.project_id.as_str()])?;
        tx.execute(
            "INSERT INTO task_creation_intents(task_id, project_id, title, description)
             VALUES(?1, ?2, ?3, ?4)",
            params![
                input.id.as_str(),
                input.project_id.as_str(),
                input.title,
                input.description
            ],
        )?;
        let task = task_on(&tx, &input.id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(task)
    }

    pub(crate) fn task_checkouts(&self) -> StoreResult<Vec<super::TaskCheckout>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT t.id, t.issue_identifier, t.worktree, p.machine_id
             FROM tasks t LEFT JOIN work_placements p ON p.task_id=t.id WHERE t.worktree IS NOT NULL",
        )?;
        let rows = statement.query_map([], |row| {
            let home: Option<String> = row.get(3)?;
            Ok(super::TaskCheckout {
                task_id: TaskId::from_raw(row.get::<_, String>(0)?),
                issue_identifier: row.get(1)?,
                worktree: PathBuf::from(row.get::<_, String>(2)?),
                machine_id: home
                    .map(|id| crate::durable::MachineId::parse(&id))
                    .transpose()
                    .map_err(|error| invalid_column(3, error))?,
            })
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

    /// Put the Task at `end` of its Workflow by `how`, with what completion
    /// settles beside it. Returns false, writing nothing, when the move no
    /// longer applies to where the Task stands.
    pub(crate) fn complete_task(
        &self,
        task: &Task,
        skipped_pr: Option<&TaskPr>,
        how: &super::task_work::EndMove,
        by: Option<&crate::id::ProcessLfid>,
        note: Option<&str>,
    ) -> StoreResult<bool> {
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
        if let Some(TaskEventKind::FollowUp {
            remaining: Some(work),
            ..
        }) = task_follow_up_in(&transaction, &task.id)?
        {
            return Err(StoreError::InvalidAuthority(work.summary(now_unix())));
        }
        if let Some(pr) = skipped_pr {
            if transaction.execute(
                "UPDATE task_prs SET abandoned_at=?3, updated_at=?3
                 WHERE id=?1 AND task_id=?2
                   AND publication_requested_at IS NULL
                   AND merge_commit IS NULL AND abandoned_at IS NULL",
                params![pr.id.as_str(), pr.task_id.as_str(), now_unix()],
            )? == 0
            {
                return Err(StoreError::NotFound);
            }
        }
        if super::durable::task_state_in(&transaction, &task.id)? == TaskState::Abandoned {
            return Err(StoreError::InvalidData(format!(
                "Task {} is abandoned and cannot be completed",
                task.id
            )));
        }
        if super::task_work::reach_end_in(&transaction, &task.id, how, by, note)? {
            if let Some(summary) = note {
                insert_task_event_in(
                    &transaction,
                    &task.id,
                    &TaskEventKind::Progress {
                        summary: summary.into(),
                    },
                )?;
            }
            insert_task_event_in(
                &transaction,
                &task.id,
                &TaskEventKind::Completed {
                    summary: "Task completed".to_string(),
                },
            )?;
            super::task_state_delivery::queue_in(&transaction, &task.id, "completed")?;
        } else if super::durable::task_state_in(&transaction, &task.id)? != TaskState::Done {
            return Ok(false);
        }
        transaction.commit()?;
        Ok(true)
    }

    pub fn task(&self, task_id: &TaskId) -> StoreResult<Option<Task>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        task_on(&conn, task_id)
    }

    pub fn task_by_issue(&self, issue: &str) -> StoreResult<Option<Task>> {
        match self.resolve_task_id(issue, None)? {
            Some(id) => self.task(&id),
            None => Ok(None),
        }
    }

    /// Resolve identity without requiring Project metadata or checkout placement.
    pub(crate) fn resolve_task_id(
        &self,
        issue: &str,
        repo: Option<&str>,
    ) -> StoreResult<Option<TaskId>> {
        let prefix = issue
            .strip_prefix("lf-")
            .or_else(|| issue.strip_prefix("task_"))
            .unwrap_or(issue);
        let local_prefix = ((4..=32).contains(&prefix.len())
            && prefix.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| prefix.to_ascii_lowercase());
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT t.id, COALESCE(t.issue_title, t.issue_identifier, t.id) FROM tasks t
             LEFT JOIN projects p ON p.id=t.project_id LEFT JOIN waves w ON w.id=p.wave_id
             WHERE t.id=?1 OR ((t.external_issue_id=?1 OR t.issue_identifier=?1
                OR substr(lower(t.id), 6, length(?2))=?2) AND (?3 IS NULL OR w.repo=?3))
             ORDER BY t.id",
        )?;
        let mut tasks = statement
            .query_map(params![issue, local_prefix, repo], |row| {
                Ok((
                    TaskId::from_raw(row.get::<_, String>(0)?),
                    row.get::<_, String>(1)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if tasks.len() > 1 {
            let candidates = tasks
                .iter()
                .map(|(id, title)| format!("  {id} ({title})"))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(StoreError::InvalidData(format!(
                "multiple stable Tasks resolve to {issue:?}; use a longer Task ID:\n{candidates}"
            )));
        }
        Ok(tasks.pop().map(|(id, _)| id))
    }

    pub fn task_by_branch(&self, branch: &str) -> StoreResult<Option<Task>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = format!(
            "{} WHERE EXISTS (
                SELECT 1 FROM task_prs matched
                WHERE matched.task_id=t.id AND matched.branch=?1
                  AND matched.abandoned_at IS NULL
                  AND (
                    matched.merge_commit IS NULL
                    OR NOT EXISTS (
                        SELECT 1 FROM task_prs active
                        WHERE active.task_id=t.id
                          AND active.merge_commit IS NULL
                          AND active.abandoned_at IS NULL
                    )
                  )
            )",
            task_columns()
        );
        let mut statement = conn.prepare(&query)?;
        let tasks = statement
            .query_map(params![branch], map_task_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        resolve_current_task(branch, tasks)
    }

    pub fn list_tasks(&self, wave_id: Option<&WaveId>) -> StoreResult<Vec<Task>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let filter = match wave_id {
            Some(_) => "p.wave_id=?1 AND ",
            None => "",
        };
        let query = format!(
            "{} WHERE {filter}{TASK_VISIBLE} ORDER BY t.updated_at DESC",
            task_columns()
        );
        let mut statement = conn.prepare(&query)?;
        let rows = statement.query_map(params_from_iter(wave_id), map_task_row)?;
        rows.collect::<rusqlite::Result<_>>()
            .map_err(StoreError::from)
    }

    /// Select a dependency without claiming that Git or GitHub has moved.
    pub fn stack_task_pr(&self, expected: &TaskPr, parent_id: &TaskPrId) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let child = active_task_pr_on(&tx, &expected.task_id)?.ok_or(StoreError::NotFound)?;
        if child.id != expected.id || child.base_commit != expected.base_commit {
            return Err(StoreError::InvalidAuthority(
                "Task PR changed; retry preparation".into(),
            ));
        }
        if child.parent_pr_id.as_ref() == Some(parent_id) {
            return Ok(());
        }
        if child.parent_pr_id.is_some() {
            return Err(StoreError::InvalidAuthority(
                "Task PR already has a parent; retain its dependency until explicit reparenting"
                    .into(),
            ));
        }
        if child
            .publication
            .as_ref()
            .is_some_and(|publication| publication.merge.is_some())
        {
            return Err(StoreError::InvalidAuthority(
                "Task PR has a merge request; cancel its delivery before selecting a parent".into(),
            ));
        }
        if super::durable::task_state_in(&tx, &child.task_id)?.is_terminal() {
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
            "SELECT cw.repo=pw.repo FROM tasks c
             JOIN projects cp ON cp.id=c.project_id JOIN waves cw ON cw.id=cp.wave_id
             JOIN tasks p ON p.id=?2
             JOIN projects pp ON pp.id=p.project_id JOIN waves pw ON pw.id=pp.wave_id
             WHERE c.id=?1",
            params![child.task_id.as_str(), parent.task_id.as_str()],
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
            if pr.task_id == child.task_id || !seen.insert(pr.id.clone()) {
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
            "UPDATE task_prs SET parent_pr_id=?2, updated_at=?3 WHERE id=?1",
            params![child.id.as_str(), parent_id.as_str(), now_unix()],
        )?;
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

    pub fn heal_task_pr_base(&self, pr: &TaskPr) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if heal_task_pr_base(&conn, pr)? == 0 {
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
    pub fn sync_task_pr(
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
    pub fn apply_linear_observation(
        &self,
        task_id: &TaskId,
        observation: &crate::pm::IssueObservation,
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
            if !super::task_comments::ingest_task_comment(
                &transaction,
                task_id,
                comment,
                observed_at,
            )? {
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
                &text,
                observed_at,
            )? {
                follow_ups_created.push(id);
            }
        }

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
                "The linked Linear task was edited; use this current definition.\n\n\
                 Title: {}\n\n{}",
                observation.title, observation.description,
            );
            Self::append_task_steer_in(&transaction, task_id, &Author::User, &text)?;
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

    pub fn task_follow_up(
        &self,
        task_id: &TaskId,
    ) -> StoreResult<Option<crate::work::task::TaskFollowUp>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(match task_follow_up_in(&conn, task_id)? {
            Some(TaskEventKind::FollowUp { remaining, .. }) => remaining,
            _ => None,
        })
    }

    pub(crate) fn task_follow_up_resolved(&self, task_id: &TaskId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(matches!(
            task_follow_up_in(&conn, task_id)?,
            Some(TaskEventKind::FollowUp {
                remaining: None,
                ..
            })
        ))
    }

    pub(crate) fn set_task_follow_up(
        &self,
        task_id: &TaskId,
        remaining: Option<crate::work::task::TaskFollowUp>,
        reason: &str,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous = task_follow_up_in(&tx, task_id)?;
        if matches!(&previous, Some(TaskEventKind::FollowUp { remaining: old, .. }) if old == &remaining)
        {
            return Ok(());
        }
        if super::durable::task_state_in(&tx, task_id)?.is_terminal() {
            return Err(StoreError::InvalidAuthority(
                "Task is terminal; its outcome is preserved".into(),
            ));
        }
        insert_task_event_in(
            &tx,
            task_id,
            &TaskEventKind::FollowUp {
                remaining,
                reason: reason.into(),
            },
        )?;
        tx.commit()?;
        Ok(())
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
        transaction.execute(
            PROJECT_INSERT,
            params![
                project.id.as_str(),
                project.wave_id,
                project.plan.linear_id.as_ref().map(LinearProjectId::as_str),
                project.plan.slug,
                project.plan.name,
                project.plan.prompt_context,
                project.plan.pm_snapshot_synced_at,
                project
                    .abandon_intent
                    .as_ref()
                    .map(|intent| intent.requested_at.unix_timestamp()),
                project
                    .abandon_intent
                    .as_ref()
                    .map(|intent| intent.reason.as_str()),
                project.created_at.unix_timestamp(),
                project.updated_at.unix_timestamp(),
                project.iteration,
                project.plan.workflow,
                project.plan.status.as_str(),
                project.plan.summary,
            ],
        )?;
        super::project_content::capture_content(&transaction, &project.id)?;
        inherit_project_placement(&transaction, &project.id)?;
        transaction.commit()?;
        Ok(())
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
             ORDER BY (external_project_id=?1) DESC, created_at DESC, id DESC
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
            task.plan.identifier
        )));
    }
    Ok(())
}

/// Saved planning admits work without acquisition. Retained contrary provider
/// evidence still applies; missing inventory cannot erase the saved Task.
pub(super) fn require_task_planning(conn: &Connection, task: &Task) -> StoreResult<()> {
    require_task_not_deleted(conn, task)?;
    let (state, completed): (Option<String>, bool) = conn.query_row(
        "SELECT planning_state,planning_completed FROM tasks WHERE id=?1",
        [task.id.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if super::durable::task_state_in(conn, &task.id)? != crate::durable::TaskState::Active
        && crate::pm::terminal_reason(state.as_deref(), completed).is_some()
    {
        return Err(StoreError::InvalidAuthority(
            "terminal planning state cannot start work; its execution history is preserved".into(),
        ));
    }
    let Some(issue) = &task.plan.linear_id else {
        return Ok(());
    };
    let (repo, project): (String, Option<String>) = conn.query_row(
        "SELECT w.repo,p.external_project_id FROM projects p
         JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
        [task.project_id.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let observation =
        super::planning::pm_task_observation_in(conn, &repo, "linear", issue.as_str())?;
    if matches!(
        observation.state,
        crate::store::PlanningState::Invalid | crate::store::PlanningState::Removed
    ) {
        return Err(StoreError::InvalidAuthority(
            "Task planning has invalidation or removal evidence; refresh its planning".into(),
        ));
    }
    if let Some(record) = observation.record {
        if record.item.project_id != project {
            return Err(StoreError::InvalidAuthority(
                "Task planning no longer matches its owning Project; its history is preserved"
                    .into(),
            ));
        }
        if let Some(project) = record.project {
            if !record
                .item
                .team_id
                .as_ref()
                .is_some_and(|team| project.team_ids.contains(team))
            {
                return Err(StoreError::InvalidAuthority(
                    "Task planning no longer matches its Project's Team; its history is preserved"
                        .into(),
                ));
            }
        }
    }
    Ok(())
}

fn task_deleted_on(conn: &Connection, task: &Task) -> StoreResult<bool> {
    // Either receipt applies regardless of planning ownership.
    // Registration also checks provider evidence before a Task row exists.
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND planning_deleted_at IS NOT NULL)
         OR EXISTS(SELECT 1 FROM task_deletions WHERE wave_id=?2 AND issue_id=?3)",
        params![
            task.id.as_str(),
            task.wave_id.as_str(),
            task.plan.linear_id.as_ref().map(|id| id.as_str())
        ],
        |row| row.get(0),
    )?)
}

fn insert_task_row(conn: &Connection, task: &Task) -> StoreResult<()> {
    conn.execute(
        TASK_INSERT,
        params![
            task.id.as_str(),
            task.project_id.as_str(),
            task.plan.linear_id.as_ref().map(LinearIssueId::as_str),
            task.plan.identifier,
            task.plan.title,
            task.plan.description,
            task.plan.pm_snapshot_synced_at,
            task.worktree
                .as_ref()
                .map(|path| path.display().to_string()),
            task.workspace_slug,
            task.abandon_intent
                .as_ref()
                .map(|intent| intent.requested_at.unix_timestamp()),
            task.abandon_intent
                .as_ref()
                .map(|intent| intent.reason.as_str()),
            task.created_at.unix_timestamp(),
            task.updated_at.unix_timestamp(),
            task.agent,
        ],
    )?;
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

const TASK_INSERT: &str = "INSERT INTO tasks (
    id, project_id, external_issue_id, issue_identifier, issue_title,
    issue_description, pm_snapshot_synced_at,
    worktree, workspace_slug,
    abandon_requested_at, abandon_reason, created_at, updated_at, agent
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14
)";
const TASK_VISIBLE: &str = "t.planning_deleted_at IS NULL
    AND NOT EXISTS(SELECT 1 FROM task_deletions d WHERE d.wave_id=p.wave_id AND d.issue_id=t.external_issue_id)";
// Both Work records and planning projections display the same unique local prefix.
pub(super) const TASK_IDENTIFIER: &str = "CASE WHEN t.issue_identifier='lf-' || substr(t.id,6) THEN
        'lf-' || substr(t.id,6,COALESCE((WITH RECURSIVE selector_lengths(n) AS (
            SELECT 7 UNION ALL SELECT n+1 FROM selector_lengths WHERE n<32
        ) SELECT min(n) FROM selector_lengths
            WHERE NOT EXISTS(SELECT 1 FROM tasks other WHERE other.id!=t.id
                AND substr(other.id,6,n)=substr(t.id,6,n))),32))
        ELSE t.issue_identifier END";

fn task_columns() -> String {
    format!("SELECT t.id,t.external_issue_id,{TASK_IDENTIFIER},
    t.issue_title, t.issue_description,
    p.wave_id, t.worktree, t.workspace_slug,
    t.created_at, t.updated_at, t.pm_snapshot_synced_at,
    COALESCE((SELECT json_object('state','pending','operation',
        CASE d.target WHEN 'completed' THEN 'complete_task' WHEN 'canceled' THEN 'cancel_task' ELSE 'reopen_task' END,
        'error',COALESCE(d.error,'Saved locally; pending Linear synchronization'))
        FROM task_state_deliveries d WHERE d.task_id=t.id AND d.settled=0
            AND t.external_issue_id IS NOT NULL
            AND d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=t.id)),
        json_object('state','current')),
    t.project_id, t.abandon_requested_at, t.abandon_reason, t.agent, t.planning_revision
    FROM tasks t JOIN projects p ON p.id=t.project_id")
}
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
/// only when the comment revision is new to the ledger. Overlapping observations
/// cannot deliver it twice, and a local comment echo adds no direction.
fn ingest_linear_comment(
    conn: &rusqlite::Transaction<'_>,
    task_id: &str,
    comment_id: &str,
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
        let steer = SqliteStore::append_task_steer_in(
            conn,
            &TaskId::from_raw(task_id),
            &Author::User,
            text,
        )?;
        Ok(Some(steer.id))
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
            ci_observation=?17, github_observation=?19,
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

pub(super) fn task_on(conn: &Connection, task_id: &TaskId) -> StoreResult<Option<Task>> {
    conn.query_row(
        &format!("{} WHERE t.id=?1", task_columns()),
        [task_id.as_str()],
        map_task_row,
    )
    .optional()
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

fn map_task_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
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
    Ok(Task {
        id: TaskId::from_raw(row.get::<_, String>(0)?),
        plan: TaskPlan {
            revision: row.get::<_, i64>(16)? as u64,
            linear_id: row
                .get::<_, Option<String>>(1)?
                .map(LinearIssueId::from_raw),
            identifier: row.get(2)?,
            title: row.get(3)?,
            description: row.get(4)?,
            pm_snapshot_synced_at: row.get(10)?,
        },
        pm_writeback: serde_json::from_str(&row.get::<_, String>(11)?)
            .map_err(|error| invalid_column(11, error))?,
        wave_id: row.get(5)?,
        project_id: ProjectId::from_raw(row.get::<_, String>(12)?),
        worktree: row.get::<_, Option<String>>(6)?.map(PathBuf::from),
        workspace_slug: row.get(7)?,
        agent: row.get(15)?,
        abandon_intent,
        created_at: crate::store::rows::unix_to_datetime(row.get(8)?),
        updated_at: crate::store::rows::unix_to_datetime(row.get(9)?),
        // Runtime freshness is derived when reconciliation decides whether the
        // durable GitHub observation can be reused.
        observation: crate::work::task::Observation::NotRequired,
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

const PROJECT_INSERT: &str = "INSERT INTO projects (
    id, wave_id, external_project_id, project_slug, project_name,
    project_prompt_context, pm_snapshot_synced_at,
    abandon_requested_at, abandon_reason,
    created_at, updated_at, iteration, workflow, status, project_summary
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
)";
const PROJECT_COLUMNS: &str = "SELECT
    id, external_project_id, project_slug, project_name, project_prompt_context,
    wave_id, pm_snapshot_synced_at, abandon_requested_at, abandon_reason,
    created_at, updated_at, iteration, workflow, status, project_summary
    FROM projects";
pub(super) const PROJECT_SELECT: &str = "SELECT
    id, external_project_id, project_slug, project_name, project_prompt_context,
    wave_id, pm_snapshot_synced_at, abandon_requested_at, abandon_reason,
    created_at, updated_at, iteration, workflow, status, project_summary
    FROM projects WHERE id=?1";
pub(super) fn map_project_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    let abandon_intent = match (
        row.get::<_, Option<i64>>(7)?,
        row.get::<_, Option<String>>(8)?,
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
            summary: row.get(14)?,
            linear_id: row
                .get::<_, Option<String>>(1)?
                .map(LinearProjectId::from_raw),
            slug: row.get(2)?,
            name: row.get(3)?,
            prompt_context: row.get(4)?,
            pm_snapshot_synced_at: row.get(6)?,
            workflow: row.get(12)?,
            status: serde_json::from_value(serde_json::Value::String(row.get(13)?))
                .map_err(|error| invalid_column(13, error))?,
        },
        wave_id: row.get(5)?,
        iteration: row.get::<_, i64>(11)? as u32,
        abandon_intent,
        created_at: crate::store::rows::unix_to_datetime(row.get(9)?),
        updated_at: crate::store::rows::unix_to_datetime(row.get(10)?),
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

fn task_follow_up_in(conn: &Connection, task_id: &TaskId) -> StoreResult<Option<TaskEventKind>> {
    let json: Option<String> = conn
        .query_row(
            "SELECT kind_json FROM task_events WHERE task_id=?1
         AND json_extract(kind_json, '$.kind')='follow_up' ORDER BY id DESC LIMIT 1",
            [task_id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    json.map(|json| serde_json::from_str(&json).map_err(StoreError::from))
        .transpose()
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
        inherit_task_placement(&tx, &task).unwrap();
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
                "UPDATE tasks SET worktree=?2,workspace_slug=?3,agent=?4 WHERE id=?1",
                params![
                    task.id.as_str(),
                    task.worktree.as_ref().map(|p| p.display().to_string()),
                    task.workspace_slug,
                    task.agent
                ],
            )?;
        }
        inherit_task_placement(&tx, task)?;
        insert_task_pr(&tx, pr)?;
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

    use crate::durable::{ProjectId, TaskId};
    use crate::id::WaveId;
    use crate::planning::NewTask;
    use crate::pm::PmItemUpdate;
    use crate::store::sqlite::SqliteStore;

    fn local_project(store: &SqliteStore) -> ProjectId {
        store.ensure_wave_project("/local", "inbox").unwrap().id
    }

    #[test]
    fn local_planning_creation_retry_keeps_identity_after_edit_and_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let input = NewTask {
            id: TaskId::new(),
            project_id: local_project(&store),
            title: "Fix parser".into(),
            description: "Keep the original input".into(),
        };
        let task = store.create_task(&input).unwrap();
        assert!(task.worktree.is_none());
        assert!(task.plan.linear_id.is_none());
        assert!(store.task_prs(&task.id).unwrap().is_empty());
        assert!(store.task_checkouts().unwrap().is_empty());
        let edited = store
            .edit_task(
                &task.id,
                task.plan.revision,
                &PmItemUpdate {
                    name: Some("Fix quoted input".into()),
                    description: None,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(edited.plan.revision, 1);
        assert_eq!(edited.plan.description, input.description);
        assert!(store
            .edit_task(
                &task.id,
                0,
                &PmItemUpdate {
                    name: Some("Stale".into()),
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
                id: TaskId::new(),
                ..input.clone()
            })
            .unwrap();
        assert_ne!(second.id, task.id);
        assert_eq!(store.list_tasks(None).unwrap().len(), 2);
        assert_eq!(store.list_tasks(Some(&task.wave_id)).unwrap().len(), 2);
        assert!(store.list_tasks(Some(&WaveId::new())).unwrap().is_empty());
        assert!(store
            .create_task(&NewTask {
                title: "Different request".into(),
                ..input
            })
            .is_err());
        assert_eq!(store.task(&task.id).unwrap().unwrap(), edited);
        let conn = store.conn.lock().unwrap();
        for table in ["agent_sessions", "processes", "task_workflows", "task_prs"] {
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
            id: TaskId::new(),
            project_id: local_project(&store),
            title: "Keep identity".into(),
            description: String::new(),
        };
        let local = store.create_task(&input).unwrap();
        assert_eq!(local.plan.pm_snapshot_synced_at, None);
        // Export is a separate slice; this proves the schema accepts a different
        // provider UUID without changing local identity or creation history.
        store.conn.lock().unwrap().execute(
            "UPDATE tasks SET external_issue_id='different-provider-uuid',issue_identifier='TEAM-9' WHERE id=?1",
            [local.id.as_str()],
        ).unwrap();
        let mapped = store.task_by_issue("TEAM-9").unwrap().unwrap();
        assert_eq!(mapped.id, local.id);
        assert_eq!(
            store
                .task_by_issue(&local.plan.identifier)
                .unwrap()
                .unwrap()
                .id,
            local.id
        );
        assert_eq!(store.create_task(&input).unwrap(), mapped);
        let edited = store
            .edit_task(
                &local.id,
                0,
                &PmItemUpdate {
                    name: Some("A mapping does not transfer authority".into()),
                    description: None,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(edited.plan.title, "A mapping does not transfer authority");
        store.conn.lock().unwrap().execute("INSERT INTO task_deletions(wave_id,issue_id,identifier,confirmed_at) VALUES(?1,'different-provider-uuid','TEAM-9',1)", [&mapped.wave_id]).unwrap();
        assert!(store.task_deleted(&mapped).unwrap());
        assert!(store.list_tasks(None).unwrap().is_empty());
        let retained = store.task(&mapped.id).unwrap().unwrap();
        assert_eq!(retained.plan, edited.plan);
        assert_eq!(store.create_task(&input).unwrap(), retained);
    }

    #[test]
    fn local_planning_prefix_collision_is_explicit() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&directory.path().join("loopflow.db")).unwrap();
        let input = NewTask {
            id: TaskId::parse("task_0123456789ab40008000000000000001").unwrap(),
            project_id: local_project(&store),
            title: "One".into(),
            description: String::new(),
        };
        let first = store.create_task(&input).unwrap();
        assert_eq!(first.plan.identifier, "lf-0123456");
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
        assert_eq!(first.plan.identifier.len(), 35);
        assert_eq!(second.plan.identifier.len(), 35);
        for task in [first, second] {
            assert_eq!(
                store.task_by_issue(task.id.as_str()).unwrap().unwrap().id,
                task.id
            );
            assert_eq!(
                store
                    .task_by_issue(&task.plan.identifier)
                    .unwrap()
                    .unwrap()
                    .id,
                task.id
            );
        }
    }
    #[test]
    fn local_planning_migration_preserves_released_links_and_orphan_deletion_evidence() {
        use crate::store::migrations::{apply_before_current_draft, current_draft_sql};
        use std::sync::{Arc, Mutex};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_current_draft(&conn, "local_planning");
        let renamed: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='processes')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let (processes, lfid, reference) = if renamed {
            ("processes", "lfid", "process_lfid")
        } else {
            ("execs", "id", "exec_id")
        };
        let wave = WaveId::new();
        let orphan_wave = WaveId::new();
        let project = ProjectId::new();
        let task = TaskId::new();
        conn.execute("INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'shared','/repo',1),(?2,'recovery','/repo',1)", params![wave,orphan_wave]).unwrap();
        conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at,project_slug,project_name,project_prompt_context,pm_snapshot_synced_at,updated_at) VALUES(?1,?2,'linear-project',1,'shared','Shared','Retain KRs',7,7)",params![project.as_str(),wave]).unwrap();
        conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at,issue_title,issue_description,pm_snapshot_synced_at,worktree,workspace_slug,updated_at) VALUES(?1,?2,'linear-task','LOO-1',1,'Retain title','Retain brief',7,'/repo/task','task',7)",params![task.as_str(),project.as_str()]).unwrap();
        conn.execute("INSERT INTO task_issue_identities(wave_id,issue_id,identifier) VALUES(?1,'orphan-issue','LOO-2')",[&orphan_wave]).unwrap();
        let payload: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        let project_body = payload["projects"][0].to_string();
        let item_body = payload["items"][0].to_string();
        conn.execute("INSERT INTO pm_projects(repo,provider,id,observed_at,body) VALUES('/repo','linear','current',7,?1)",[&project_body]).unwrap();
        conn.execute("INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body) VALUES('/repo','linear','LOO-318','LOO-318','current',7,?1)",[&item_body]).unwrap();
        conn.execute("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at) VALUES('pr-retained',?1,1,'task','retain/branch','retained-base',1,7)",[task.as_str()]).unwrap();
        conn.execute_batch(&format!("INSERT INTO {processes}({lfid},trace_id,command,cwd,started_at) VALUES('process-retained','trace','lf run code','/repo/task',2);")).unwrap();
        conn.execute(&format!("INSERT INTO agent_sessions(id,title,title_source,created_at,cwd,task_id,wave_id,driver_{reference},provider_thread,input_published) VALUES('session-retained','Conversation','human',2,'/repo/task',?1,?2,'process-retained','native-retained',1)"),params![task.as_str(),wave]).unwrap();
        conn.execute(&format!("INSERT INTO task_workflows(task_id,graph,node,edge,{reference},updated_at) VALUES(?1,'{{}}','review',0,'process-retained',3)"),[task.as_str()]).unwrap();
        conn.execute(&format!("INSERT INTO task_workflow_moves(task_id,workflow,kind,from_node,to_node,edge,{reference},note,at) VALUES(?1,'code','chose','start','review',0,'process-retained','Original choice',3)"),[task.as_str()]).unwrap();
        // The released frontier can precede another Task's required vocabulary migration.
        if !renamed {
            conn.execute_batch(&current_draft_sql("process_names"))
                .unwrap();
        }
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
        let rows = |conn: &rusqlite::Connection, table: &str| {
            let mut query = conn.prepare(&format!("SELECT * FROM {table}")).unwrap();
            // Preserve every released column; the migration adds the ordering baseline.
            let columns = query.column_count()
                - usize::from(
                    table == "pm_projects" && query.column_names().contains(&"task_order_json"),
                );
            query
                .query_map([], |row| {
                    (0..columns)
                        .map(|i| row.get::<_, rusqlite::types::Value>(i))
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        let before: Vec<_> = tables.iter().map(|table| rows(&conn, table)).collect();
        conn.execute_batch("PRAGMA foreign_keys=OFF; BEGIN IMMEDIATE;")
            .unwrap();
        conn.execute_batch(&current_draft_sql("local_planning"))
            .unwrap();
        let violations: i64 = conn
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(violations, 0);
        conn.execute_batch("COMMIT; PRAGMA foreign_keys=ON;")
            .unwrap();
        for (table, expected) in tables.iter().zip(before) {
            assert_eq!(rows(&conn, table), expected, "{table}");
        }
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
        let observation = store
            .pm_task_observation("/repo", "linear", "LOO-318")
            .unwrap()
            .record
            .unwrap();
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
        assert_eq!(imported.project_id, imported_project);
        assert!(imported.worktree.is_none());
        assert!(store.task_prs(&imported.id).unwrap().is_empty());
        assert_eq!(imported.plan.pm_snapshot_synced_at, Some(7));
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
        assert_eq!(retained.project_id, project);
        assert_eq!(retained.plan.linear_id.unwrap().as_str(), "linear-task");
        assert_eq!(retained.plan.identifier, "LOO-1");
        assert_eq!(retained.worktree.unwrap().to_str(), Some("/repo/task"));
        let project = store.project(&project).unwrap().unwrap();
        assert_eq!(project.plan.linear_id.unwrap().as_str(), "linear-project");
        assert_eq!(project.plan.prompt_context, "Retain KRs");
        assert_eq!(
            store.conn.lock().unwrap().query_row("SELECT issue_id,identifier FROM task_issue_identities WHERE wave_id=?1 AND identifier='LOO-2'", [&orphan_wave], |row| Ok((row.get::<_, String>(0)?,row.get::<_, String>(1)?))).optional().unwrap(),
            Some(("orphan-issue".into(), "LOO-2".into()))
        );
        assert!(store.task_by_issue("LOO-2").unwrap().is_none());
    }
}
