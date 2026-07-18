//! SQLite persistence for Project and Task Sessions.

use std::path::PathBuf;

// Session writes read before they write (validating a parent Session, reading a
// generation), so a deferred transaction has to upgrade its read lock to a write
// lock. Under WAL, SQLite fails that upgrade immediately rather than waiting —
// `busy_timeout` is never consulted, because waiting on an upgrade can deadlock
// two upgraders. Beginning IMMEDIATE takes the write lock up front, where
// `busy_timeout` does apply, so a second `lf` process queues instead of dying
// with `database is locked`.
use rusqlite::{params, Connection, OptionalExtension, ToSql, TransactionBehavior};
use time::OffsetDateTime;

use crate::child_session::{
    AbandonIntent, ChildBodyHandoff, ChildBodyHandoffRequest, ChildBodyOutcome, ChildLeaseState,
    ChildProcessGeneration, ChildRef, ObservationRecipient,
};
use crate::durable::{Author, RunLease};
use crate::engine::InteractionPolicy;
use crate::id::WaveId;
use crate::project_session::{
    ChildEventPayload, ObservationOutboxRow, ProjectEvent, ProjectEventKind, ProjectSession,
    ProjectSessionId, ProjectSessionStatus,
};
use crate::session_context::{
    LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot,
};
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::task::{
    AfterMerge, CiObservation, GithubObservation, GithubPr, LinearObservationApply,
    LinearObservationOutcome, PrPhase, PrPublication, TaskEvent, TaskEventKind, TaskLifecyclePhase,
    TaskLifecyclePlan, TaskLinearObservation, TaskPhasePlan, TaskPr, TaskPrId, TaskSession,
    TaskSessionId, TaskSessionStatus, TaskSessionSuccession,
};

use super::durable::{
    create_project_spine, create_task_spine, end_run_for_child, end_run_for_lease,
    fence_run_for_child, validate_run_lease, work_for_child_in,
};
use super::SqliteStore;

impl SqliteStore {
    // Durable task sessions: Linear identity, immutable placement, commands,
    // and lifecycle events share one sqlite transaction boundary.

    pub fn insert_task_session(&self, session: &TaskSession, pr: &TaskPr) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        insert_initial_task(&transaction, session, pr)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn insert_task_session_with_steer(
        &self,
        session: &TaskSession,
        pr: &TaskPr,
        author: &Author,
        text: &str,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        insert_initial_task(&transaction, session, pr)?;
        let work = work_for_child_in(&transaction, &ChildRef::Task(session.id.clone()))?;
        Self::append_steer_in(&transaction, &work, author, text)?;
        transaction.commit()?;
        Ok(())
    }

    /// Carry a terminal Task Session's direction onto a successor, in one
    /// transaction with the successor's own creation.
    ///
    /// Inserts the successor Session, its sequence-1 Working PR, and its initial
    /// Steer; then transactionally re-keys the Linear observation cursor and
    /// the ingested-comment ledger from the predecessor onto the successor. The
    /// cursor is re-keyed (moved) rather than copied, so the successor resumes
    /// polling from the predecessor's last revision and a webhook redelivery of
    /// an already-applied edit cannot re-emit it; if the predecessor had no
    /// cursor row, the successor is seeded from its launch snapshot instead. The
    /// ledger re-key makes a comment already turned into a Steer by the
    /// predecessor land zero times on the successor. Historical receipts
    /// stay on the predecessor for attribution; the successor is soft-linked to it via
    /// `predecessor_session_id`.
    ///
    /// Idempotent: if a non-terminal Session for the same Linear issue already
    /// exists (a concurrent or retried run, or a crash after commit), it is
    /// returned unchanged with `created: false` and no re-key runs again.
    pub fn reserve_task_session_successor(
        &self,
        predecessor: &TaskSession,
        successor: &TaskSession,
        pr: &TaskPr,
        author: &Author,
        text: &str,
    ) -> StoreResult<TaskSessionSuccession> {
        validate_task_session(successor)?;
        validate_initial_task_pr(successor, pr)?;
        let issue_id = successor.launch.issue.id.as_str().to_string();
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) =
            non_terminal_successor_for_issue(&transaction, predecessor.id.as_str(), &issue_id)?
        {
            transaction.commit()?;
            return Ok(TaskSessionSuccession {
                session: existing,
                created: false,
            });
        }
        validate_task_project_session(&transaction, successor)?;
        let parameters = task_session_params(successor);
        transaction.execute(
            TASK_SESSION_INSERT,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        create_task_spine(&transaction, successor)?;
        insert_task_pr(&transaction, pr)?;
        let work = work_for_child_in(&transaction, &ChildRef::Task(successor.id.clone()))?;
        Self::append_steer_in(&transaction, &work, author, text)?;
        _carry_task_session_state(&transaction, predecessor, successor)?;
        transaction.commit()?;
        Ok(TaskSessionSuccession {
            session: successor.clone(),
            created: true,
        })
    }

    /// Recover an abandoned Task by creating one linked successor that adopts
    /// the predecessor's worktree and complete serial PR history.
    ///
    /// Unlike [`Self::reserve_task_session_successor`], this does not create a
    /// new worktree or sequence-1 PR. It re-keys the existing PR rows together
    /// with the Linear observation state, so the ownership move is atomic and a
    /// crash cannot leave two partial attempts. A repeated or concurrent call
    /// converges on the one live successor.
    pub fn recover_task_session_successor(
        &self,
        predecessor: &TaskSession,
        successor: &TaskSession,
        author: &Author,
        text: &str,
    ) -> StoreResult<TaskSessionSuccession> {
        if predecessor.status != TaskSessionStatus::Abandoned {
            return Err(StoreError::InvalidData(format!(
                "Task Session {} is {}; only abandoned Tasks can be recovered",
                predecessor.id,
                predecessor.status.as_str()
            )));
        }
        if successor.launch != predecessor.launch
            || successor.wave_id != predecessor.wave_id
            || successor.project_session_id != predecessor.project_session_id
        {
            return Err(StoreError::InvalidData(
                "a recovered Task successor must keep its predecessor's launch and ownership"
                    .to_string(),
            ));
        }
        if successor.worktree != predecessor.worktree
            || successor.workspace_slug != predecessor.workspace_slug
        {
            return Err(StoreError::InvalidData(
                "a recovered Task successor must adopt its predecessor's worktree and workspace"
                    .to_string(),
            ));
        }
        if successor.status != TaskSessionStatus::Waiting
            || successor.latest_process.is_some()
            || successor.provider_session_id.is_some()
            || successor.abandon_intent.is_some()
        {
            return Err(StoreError::InvalidData(
                "a recovered Task successor must begin waiting without a live body".to_string(),
            ));
        }
        validate_task_session(successor)?;
        let issue_id = successor.launch.issue.id.as_str().to_string();
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) =
            non_terminal_successor_for_issue(&transaction, predecessor.id.as_str(), &issue_id)?
        {
            if existing.worktree != predecessor.worktree {
                return Err(StoreError::InvalidData(format!(
                    "Task {} already has successor {} on {}; recovery cannot also adopt {}",
                    predecessor.launch.issue.identifier,
                    existing.id,
                    existing.worktree.display(),
                    predecessor.worktree.display()
                )));
            }
            transaction.commit()?;
            return Ok(TaskSessionSuccession {
                session: existing,
                created: false,
            });
        }
        validate_task_project_session(&transaction, successor)?;
        let parameters = task_session_params(successor);
        transaction.execute(
            TASK_SESSION_INSERT,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        create_task_spine(&transaction, successor)?;
        let moved = transaction.execute(
            "UPDATE task_prs SET task_session_id=?1, updated_at=?2 WHERE task_session_id=?3",
            params![successor.id.as_str(), now_unix(), predecessor.id.as_str()],
        )?;
        if moved == 0 {
            return Err(StoreError::InvalidData(format!(
                "abandoned Task Session {} has no PR history to recover",
                predecessor.id
            )));
        }
        let work = work_for_child_in(&transaction, &ChildRef::Task(successor.id.clone()))?;
        Self::append_steer_in(&transaction, &work, author, text)?;
        _carry_task_session_state(&transaction, predecessor, successor)?;
        transaction.commit()?;
        Ok(TaskSessionSuccession {
            session: successor.clone(),
            created: true,
        })
    }

    pub fn update_task_session(&self, session: &TaskSession) -> StoreResult<()> {
        validate_task_session(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_task_project_session(&transaction, session)?;
        let parameters = task_session_control_params(session);
        let changed = transaction.execute(
            TASK_SESSION_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        close_task_epoch_if_quiescent(&transaction, session)?;
        transaction.commit()?;
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
            "UPDATE task_sessions SET issue_identifier=?2, updated_at=?3
             WHERE epoch_id IN (SELECT id FROM epochs WHERE task_id=?1 AND state='open')",
            params![task_id, new_identifier, now_unix()],
        )?;
        tx.commit()?;
        Ok(true)
    }

    pub(crate) fn activate_task_process_for_run(
        &self,
        session: &TaskSession,
        lease: &RunLease,
    ) -> StoreResult<()> {
        validate_task_session(session)?;
        let process = session.latest_process.as_ref().ok_or_else(|| {
            StoreError::InvalidData("Task activation requires process evidence".to_string())
        })?;
        if process.state != ChildLeaseState::Active {
            return Err(StoreError::InvalidData(
                "Task activation requires Active process evidence".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if update_task_session_for_run_in(&transaction, session, lease)? == 0 {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot activate Task Session {}",
                lease.run_id, session.id
            )));
        }
        insert_task_event_in(
            &transaction,
            session,
            &TaskEventKind::BodyLeaseChanged {
                process: process.clone(),
            },
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn update_task_session_for_run(
        &self,
        session: &TaskSession,
        lease: &RunLease,
    ) -> StoreResult<()> {
        validate_task_session(session)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        if update_task_session_for_run_in(&conn, session, lease)? == 0 {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot update Task Session {}",
                lease.run_id, session.id
            )));
        }
        Ok(())
    }

    pub(crate) fn finish_task_process_for_run(
        &self,
        session: &TaskSession,
        lease: &RunLease,
    ) -> StoreResult<()> {
        validate_task_session(session)?;
        let process = session.latest_process.as_ref().ok_or_else(|| {
            StoreError::InvalidData("Task finish requires process evidence".to_string())
        })?;
        if process.state != ChildLeaseState::Finished || process.outcome.is_none() {
            return Err(StoreError::InvalidData(
                "Task finish requires Finished process evidence and outcome".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if update_task_session_for_run_in(&transaction, session, lease)? == 0 {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot finish Task Session {}",
                lease.run_id, session.id
            )));
        }
        insert_task_event_in(
            &transaction,
            session,
            &TaskEventKind::BodyLeaseChanged {
                process: process.clone(),
            },
        )?;
        end_run_for_lease(&transaction, lease)?;
        close_task_epoch_if_quiescent(&transaction, session)?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn revoke_task_process(
        &self,
        session_id: &TaskSessionId,
        outcome: &ChildBodyOutcome,
    ) -> StoreResult<ChildProcessGeneration> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut session = transaction
            .query_row(
                TASK_SESSION_SELECT,
                params![session_id.as_str()],
                map_task_session_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        let process = session.latest_process.as_mut().ok_or_else(|| {
            StoreError::InvalidData(format!("Task Session {session_id} has no body to revoke"))
        })?;
        if process.state == ChildLeaseState::Finished {
            return Err(StoreError::InvalidData(format!(
                "Task Session {session_id} generation {} is already finished",
                process.generation
            )));
        }
        process.state = ChildLeaseState::Revoked;
        process.outcome = Some(outcome.clone());
        let outcome_json = serde_json::to_string(outcome)?;
        let changed = transaction.execute(
            "UPDATE task_sessions
             SET process_lease_state='revoked', process_outcome_json=?3
             WHERE id=?1 AND process_generation=?2
               AND process_lease_state IN ('legacy', 'reserved', 'active')",
            params![
                session_id.as_str(),
                i64::from(process.generation),
                outcome_json
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidData(format!(
                "Task Session {session_id} body is already revoked"
            )));
        }
        let process = process.clone();
        insert_task_event_in(
            &transaction,
            &session,
            &TaskEventKind::BodyLeaseChanged {
                process: process.clone(),
            },
        )?;
        fence_run_for_child(
            &transaction,
            &ChildRef::Task(session_id.clone()),
            process.generation,
        )?;
        transaction.commit()?;
        Ok(process)
    }

    /// Revoke a body only if the progress evidence a supervisor observed is
    /// still current. The immediate transaction closes the gap between the
    /// final progress check and lease revocation: a body that completed or
    /// appended an event in that gap wins, and supervision leaves it alone.
    pub(crate) fn revoke_task_process_if_unchanged(
        &self,
        session_id: &TaskSessionId,
        generation: u32,
        status_at: OffsetDateTime,
        latest_event_id: Option<i64>,
        outcome: &ChildBodyOutcome,
    ) -> StoreResult<Option<ChildProcessGeneration>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut session = transaction
            .query_row(
                TASK_SESSION_SELECT,
                params![session_id.as_str()],
                map_task_session_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        let current_event_id: Option<i64> = transaction.query_row(
            "SELECT MAX(id) FROM task_events WHERE session_id = ?1",
            params![session_id.as_str()],
            |row| row.get(0),
        )?;
        if session.status_at != status_at
            || current_event_id != latest_event_id
            || !session.status.is_process_active()
        {
            transaction.commit()?;
            return Ok(None);
        }
        let Some(process) = session.latest_process.as_mut() else {
            transaction.commit()?;
            return Ok(None);
        };
        if process.generation != generation || process.state != ChildLeaseState::Active {
            transaction.commit()?;
            return Ok(None);
        }
        process.state = ChildLeaseState::Revoked;
        process.outcome = Some(outcome.clone());
        let outcome_json = serde_json::to_string(outcome)?;
        if transaction.execute(
            "UPDATE task_sessions
             SET process_lease_state='revoked', process_outcome_json=?3
             WHERE id=?1 AND process_generation=?2
               AND process_lease_state='active' AND status_at=?4",
            params![
                session_id.as_str(),
                i64::from(generation),
                outcome_json,
                status_at.unix_timestamp(),
            ],
        )? == 0
        {
            transaction.commit()?;
            return Ok(None);
        }
        let process = process.clone();
        insert_task_event_in(
            &transaction,
            &session,
            &TaskEventKind::BodyLeaseChanged {
                process: process.clone(),
            },
        )?;
        fence_run_for_child(
            &transaction,
            &ChildRef::Task(session_id.clone()),
            generation,
        )?;
        transaction.commit()?;
        Ok(Some(process))
    }

    pub(crate) fn finish_revoked_task_process(
        &self,
        session_id: &TaskSessionId,
        generation: u32,
    ) -> StoreResult<ChildProcessGeneration> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut session = transaction
            .query_row(
                TASK_SESSION_SELECT,
                params![session_id.as_str()],
                map_task_session_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        let process = session.latest_process.as_mut().ok_or_else(|| {
            StoreError::InvalidData(format!("Task Session {session_id} has no revoked body"))
        })?;
        if process.generation != generation || process.state != ChildLeaseState::Revoked {
            return Err(StoreError::InvalidData(format!(
                "Task Session {session_id} generation {generation} is not awaiting reap"
            )));
        }
        process.state = ChildLeaseState::Finished;
        if transaction.execute(
            "UPDATE task_sessions SET process_lease_state='finished'
             WHERE id=?1 AND process_generation=?2 AND process_lease_state='revoked'",
            params![session_id.as_str(), i64::from(generation)],
        )? == 0
        {
            return Err(StoreError::InvalidData(format!(
                "Task Session {session_id} generation {generation} changed during reap"
            )));
        }
        let process = process.clone();
        insert_task_event_in(
            &transaction,
            &session,
            &TaskEventKind::BodyLeaseChanged {
                process: process.clone(),
            },
        )?;
        end_run_for_child(
            &transaction,
            &ChildRef::Task(session_id.clone()),
            generation,
        )?;
        transaction.commit()?;
        Ok(process)
    }

    pub fn complete_task_session(
        &self,
        session: &TaskSession,
        skipped_pr: Option<&TaskPr>,
    ) -> StoreResult<()> {
        self.complete_task_session_with_authority(session, skipped_pr, None)
    }

    pub(crate) fn complete_task_session_for_run(
        &self,
        session: &TaskSession,
        skipped_pr: Option<&TaskPr>,
        lease: &RunLease,
    ) -> StoreResult<()> {
        self.complete_task_session_with_authority(session, skipped_pr, Some(lease))
    }

    fn complete_task_session_with_authority(
        &self,
        session: &TaskSession,
        skipped_pr: Option<&TaskPr>,
        run_lease: Option<&RunLease>,
    ) -> StoreResult<()> {
        validate_task_session(session)?;
        if session.status != TaskSessionStatus::Completed {
            return Err(StoreError::InvalidData(
                "Task completion transaction requires a Completed Session".to_string(),
            ));
        }
        if let Some(pr) = skipped_pr {
            validate_task_pr(pr)?;
            if pr.task_session_id != session.id || pr.phase() != PrPhase::Working {
                return Err(StoreError::InvalidData(
                    "empty completion requires an unpublished Working Task PR".to_string(),
                ));
            }
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_task_project_session(&transaction, session)?;
        if let Some(lease) = run_lease {
            require_run_owns_child(&transaction, &ChildRef::Task(session.id.clone()), lease)?;
        }
        if let Some(pr) = skipped_pr {
            if transaction.execute(
                "DELETE FROM task_prs
                 WHERE id=?1 AND task_session_id=?2
                   AND publication_requested_at IS NULL
                   AND merge_commit IS NULL AND abandoned_at IS NULL",
                params![pr.id.as_str(), pr.task_session_id.as_str()],
            )? == 0
            {
                return Err(StoreError::NotFound);
            }
        }
        let changed = match run_lease {
            Some(lease) => update_task_session_for_run_in(&transaction, session, lease)?,
            None => {
                let parameters = task_session_control_params(session);
                transaction.execute(
                    TASK_SESSION_UPDATE,
                    rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
                )?
            }
        };
        if changed == 0 {
            if let Some(lease) = run_lease {
                return Err(StoreError::InvalidAuthority(format!(
                    "Run {} cannot complete Task Session {}",
                    lease.run_id, session.id
                )));
            }
            return Err(StoreError::NotFound);
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn handoff_task_body(
        &self,
        session_id: &TaskSessionId,
        request: &ChildBodyHandoffRequest,
    ) -> StoreResult<TaskSession> {
        validate_handoff_request(request)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut session = transaction
            .query_row(
                TASK_SESSION_SELECT,
                params![session_id.as_str()],
                map_task_session_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        validate_handoff_state(
            "Task",
            &session.launch.issue.identifier,
            session.status.as_str(),
            session.status.is_process_active(),
            session.status.is_terminal(),
            session.abandon_intent.as_ref(),
        )?;
        let handoff = apply_handoff(
            &mut session.agent,
            &mut session.provider,
            &mut session.provider_session_id,
            request,
        );
        session.updated_at = OffsetDateTime::now_utc();
        validate_task_session(&session)?;
        let parameters = task_session_control_params(&session);
        transaction.execute(
            TASK_SESSION_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        insert_task_event_in(
            &transaction,
            &session,
            &TaskEventKind::BodyHandedOff { handoff },
        )?;
        transaction.commit()?;
        Ok(session)
    }

    pub fn task_session(&self, session_id: &TaskSessionId) -> StoreResult<Option<TaskSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            TASK_SESSION_SELECT,
            params![session_id.as_str()],
            map_task_session_row,
        )
        .optional()
        .map_err(StoreError::from)
    }

    /// The predecessor and successor ids linked to one Task Session.
    pub fn task_session_chain_neighbors(
        &self,
        session_id: &TaskSessionId,
    ) -> StoreResult<(Option<String>, Option<String>)> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let predecessor = conn
            .query_row(
                "SELECT predecessor_session_id FROM task_sessions WHERE id=?1",
                params![session_id.as_str()],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        let successor = conn
            .query_row(
                "SELECT id FROM task_sessions WHERE predecessor_session_id=?1",
                params![session_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok((predecessor, successor))
    }

    pub fn task_session_by_issue(&self, issue: &str) -> StoreResult<Option<TaskSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = format!("{TASK_SESSION_COLUMNS} WHERE issue_id = ?1 OR issue_identifier = ?1");
        let mut statement = conn.prepare(&query)?;
        let rows = statement.query_map(params![issue], map_task_session_row)?;
        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }
        resolve_current_task_session(issue, sessions)
    }

    pub fn task_session_by_worktree(&self, worktree: &str) -> StoreResult<Option<TaskSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = format!("{TASK_SESSION_COLUMNS} WHERE worktree = ?1");
        let mut statement = conn.prepare(&query)?;
        let rows = statement.query_map(params![worktree], map_task_session_row)?;
        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }
        resolve_current_task_session(worktree, sessions)
    }

    pub fn list_task_sessions(&self, wave_id: Option<&WaveId>) -> StoreResult<Vec<TaskSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (query, parameter): (String, Option<&dyn ToSql>) = match wave_id {
            Some(wave_id) => (
                format!("{TASK_SESSION_COLUMNS} WHERE wave_id = ?1 ORDER BY updated_at DESC"),
                Some(wave_id as &dyn ToSql),
            ),
            None => (
                format!("{TASK_SESSION_COLUMNS} ORDER BY updated_at DESC"),
                None,
            ),
        };
        let mut statement = conn.prepare(&query)?;
        let mut sessions = Vec::new();
        if let Some(parameter) = parameter {
            let rows = statement.query_map([parameter], map_task_session_row)?;
            for row in rows {
                sessions.push(row?);
            }
        } else {
            let rows = statement.query_map([], map_task_session_row)?;
            for row in rows {
                sessions.push(row?);
            }
        }
        Ok(sessions)
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

    pub(crate) fn update_task_pr_for_run(&self, pr: &TaskPr, lease: &RunLease) -> StoreResult<()> {
        validate_task_pr(pr)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_run_owns_child(
            &transaction,
            &ChildRef::Task(pr.task_session_id.clone()),
            lease,
        )?;
        if update_task_pr(&transaction, pr)? == 0 {
            return Err(StoreError::NotFound);
        }
        transaction.commit()?;
        Ok(())
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
        lease: &RunLease,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_run_owns_child(
            &transaction,
            &ChildRef::Task(pr.task_session_id.clone()),
            lease,
        )?;
        if heal_task_pr_base(&transaction, pr)? == 0 {
            return Err(StoreError::NotFound);
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn task_prs(&self, session_id: &TaskSessionId) -> StoreResult<Vec<TaskPr>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(&format!(
            "{TASK_PR_COLUMNS} WHERE task_session_id=?1 ORDER BY sequence"
        ))?;
        let rows = statement.query_map(params![session_id.as_str()], map_task_pr_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn task_pr(&self, pr_id: &TaskPrId) -> StoreResult<Option<TaskPr>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        task_pr_on(&conn, pr_id)
    }

    /// Every Task PR across all sessions — the scan surface for `pr:` receipt
    /// resolution, which names `owner/repo#N` rather than a loopflow PR id.
    pub fn all_task_prs(&self) -> StoreResult<Vec<TaskPr>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(&format!("{TASK_PR_COLUMNS} ORDER BY created_at"))?;
        let rows = statement.query_map([], map_task_pr_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn active_task_pr(&self, session_id: &TaskSessionId) -> StoreResult<Option<TaskPr>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = format!(
            "{TASK_PR_COLUMNS}
             WHERE task_session_id=?1 AND merge_commit IS NULL AND abandoned_at IS NULL"
        );
        conn.query_row(&query, params![session_id.as_str()], map_task_pr_row)
            .optional()
            .map_err(StoreError::from)
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
        lease: &RunLease,
    ) -> StoreResult<()> {
        validate_task_pr_settlement(settled, next)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_run_owns_child(
            &transaction,
            &ChildRef::Task(settled.task_session_id.clone()),
            lease,
        )?;
        settle_task_pr_in(&transaction, settled, next)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn complete_task_session_after_pr(
        &self,
        session: &TaskSession,
        pr: &TaskPr,
    ) -> StoreResult<()> {
        validate_task_session(session)?;
        validate_task_pr(pr)?;
        if session.status != TaskSessionStatus::Completed
            || pr.task_session_id != session.id
            || pr.phase() != PrPhase::Merged
            || pr
                .publication
                .as_ref()
                .is_none_or(|publication| publication.after_merge != AfterMerge::CompleteTask)
        {
            return Err(StoreError::InvalidData(
                "Task completion after merge requires its merged CompleteTask PR".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_task_project_session(&transaction, session)?;
        settle_task_pr_on(&transaction, pr)?;
        let parameters = task_session_control_params(session);
        if transaction.execute(
            TASK_SESSION_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )? == 0
        {
            return Err(StoreError::NotFound);
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn complete_task_session_after_pr_for_run(
        &self,
        session: &TaskSession,
        pr: &TaskPr,
        lease: &RunLease,
    ) -> StoreResult<()> {
        validate_task_session(session)?;
        validate_task_pr(pr)?;
        if session.status != TaskSessionStatus::Completed
            || pr.task_session_id != session.id
            || pr.phase() != PrPhase::Merged
            || pr
                .publication
                .as_ref()
                .is_none_or(|publication| publication.after_merge != AfterMerge::CompleteTask)
        {
            return Err(StoreError::InvalidData(
                "Task completion after merge requires its merged CompleteTask PR".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_task_project_session(&transaction, session)?;
        require_run_owns_child(&transaction, &ChildRef::Task(session.id.clone()), lease)?;
        settle_task_pr_on(&transaction, pr)?;
        if update_task_session_for_run_in(&transaction, session, lease)? == 0 {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot complete Task Session {}",
                lease.run_id, session.id
            )));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn task_linear_observation(
        &self,
        session_id: &TaskSessionId,
    ) -> StoreResult<Option<TaskLinearObservation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT session_id, last_revision, last_title, last_description,
                    last_success_at, degraded_reason, updated_at
             FROM task_linear_observations WHERE session_id=?1",
            params![session_id.as_str()],
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
        let work = work_for_child_in(&transaction, &ChildRef::Task(apply.session_id.clone()))?;

        let existing = transaction
            .query_row(
                "SELECT last_revision, last_title, last_description
                 FROM task_linear_observations WHERE session_id=?1",
                params![apply.session_id.as_str()],
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
                    session_id, last_revision, last_title, last_description,
                    last_success_at, degraded_reason, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?5)",
                params![
                    apply.session_id.as_str(),
                    apply.revision,
                    apply.title,
                    apply.description,
                    observed_at,
                ],
            )?;
            for follow_up in &apply.follow_ups {
                transaction.execute(
                    "INSERT OR IGNORE INTO task_linear_ingested_comments
                        (session_id, comment_id, ingested_at) VALUES (?1, ?2, ?3)",
                    params![apply.session_id.as_str(), follow_up.comment_id, observed_at],
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
                apply.session_id.as_str(),
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
             WHERE session_id=?1",
            params![
                apply.session_id.as_str(),
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
        session_id: &TaskSessionId,
        comment_id: &str,
        text: &str,
        observed_at: OffsetDateTime,
    ) -> StoreResult<Option<crate::durable::SteerId>> {
        let observed_at = observed_at.unix_timestamp();
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let work = work_for_child_in(&transaction, &ChildRef::Task(session_id.clone()))?;
        let created = ingest_linear_comment(
            &transaction,
            session_id.as_str(),
            comment_id,
            &work,
            text,
            observed_at,
        )?;
        if created.is_some() {
            // Best-effort freshness for status; a Session missing its seed row
            // (legacy) simply has nothing to update.
            transaction.execute(
                "UPDATE task_linear_observations
                 SET last_success_at=?2, degraded_reason=NULL, updated_at=?2
                 WHERE session_id=?1",
                params![session_id.as_str(), observed_at],
            )?;
        }
        transaction.commit()?;
        Ok(created)
    }

    /// Record that the latest observation failed, without moving the cursor. A
    /// Session with no baseline yet has no row to mark, which is fine — status
    /// then simply shows no observation.
    pub fn mark_task_linear_degraded(
        &self,
        session_id: &TaskSessionId,
        reason: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE task_linear_observations SET degraded_reason=?2, updated_at=?3
             WHERE session_id=?1",
            params![session_id.as_str(), reason, now_unix()],
        )?;
        Ok(())
    }

    pub(crate) fn stop_task_for_run(
        &self,
        session_id: &TaskSessionId,
        lease: &RunLease,
        stopped_status: TaskSessionStatus,
        reason: &str,
    ) -> StoreResult<TaskSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut session = transaction
            .query_row(
                TASK_SESSION_SELECT,
                params![session_id.as_str()],
                map_task_session_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        session.set_status(stopped_status, reason);
        if update_task_session_for_run_in(&transaction, &session, lease)? == 0 {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot stop Task Session {session_id}",
                lease.run_id
            )));
        }
        transaction.commit()?;
        Ok(session)
    }

    pub fn append_task_event(
        &self,
        session_id: &TaskSessionId,
        kind: &TaskEventKind,
    ) -> StoreResult<TaskEvent> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session = transaction.query_row(
            TASK_SESSION_SELECT,
            params![session_id.as_str()],
            map_task_session_row,
        )?;
        let event = insert_task_event_in(&transaction, &session, kind)?;
        transaction.commit()?;
        Ok(event)
    }

    pub(crate) fn append_task_event_for_run(
        &self,
        session_id: &TaskSessionId,
        lease: &RunLease,
        kind: &TaskEventKind,
    ) -> StoreResult<TaskEvent> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_run_owns_child(&transaction, &ChildRef::Task(session_id.clone()), lease)?;
        let session = transaction.query_row(
            TASK_SESSION_SELECT,
            params![session_id.as_str()],
            map_task_session_row,
        )?;
        let event = insert_task_event_in(&transaction, &session, kind)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn task_events_after(
        &self,
        session_id: &TaskSessionId,
        cursor: i64,
    ) -> StoreResult<Vec<TaskEvent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT id, session_id, kind_json, created_at
             FROM task_events WHERE session_id = ?1 AND id > ?2 ORDER BY id",
        )?;
        let rows = statement.query_map(params![session_id.as_str(), cursor], map_task_event_row)?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    pub fn task_event(
        &self,
        session_id: &TaskSessionId,
        event_id: i64,
    ) -> StoreResult<Option<TaskEvent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT id, session_id, kind_json, created_at
             FROM task_events WHERE session_id = ?1 AND id = ?2",
            params![session_id.as_str(), event_id],
            map_task_event_row,
        )
        .optional()
        .map_err(StoreError::from)
    }

    /// When this Task Session last appended a durable event. This is the progress
    /// signal the body observation reads: a live body that has written nothing to
    /// its event log past the stall deadline is stalled, not working. `None` means
    /// no events yet (the status change is the only progress the caller can use).
    pub fn latest_task_event_at(
        &self,
        session_id: &TaskSessionId,
    ) -> StoreResult<Option<OffsetDateTime>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let seconds: Option<i64> = conn.query_row(
            "SELECT MAX(created_at) FROM task_events WHERE session_id = ?1",
            params![session_id.as_str()],
            |row| row.get(0),
        )?;
        Ok(seconds.map(crate::store::rows::unix_to_datetime))
    }

    /// The newest `limit` events, newest first. Recovery reads a bounded window
    /// rather than the whole log: a long-lived Task accumulates thousands of
    /// events, and the attempt count only ever looks at the recent tail.
    pub fn recent_task_events(
        &self,
        session_id: &TaskSessionId,
        limit: u32,
    ) -> StoreResult<Vec<TaskEvent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT id, session_id, kind_json, created_at
             FROM task_events WHERE session_id = ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![session_id.as_str(), limit], map_task_event_row)?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    pub fn latest_task_event(&self, session_id: &TaskSessionId) -> StoreResult<Option<TaskEvent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT id, session_id, kind_json, created_at
             FROM task_events WHERE session_id = ?1 ORDER BY id DESC LIMIT 1",
            params![session_id.as_str()],
            map_task_event_row,
        )
        .optional()
        .map_err(StoreError::from)
    }

    // Project Sessions are durable KR-pursuit children. They share the same
    // process/receipt shape as Task Sessions but deliberately own no worktree.

    pub fn insert_project_session(&self, session: &ProjectSession) -> StoreResult<()> {
        validate_project_session(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            PROJECT_SESSION_INSERT,
            rusqlite::params_from_iter(
                project_session_params(session)
                    .iter()
                    .map(|value| value.as_ref()),
            ),
        )?;
        create_project_spine(&transaction, session)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn insert_project_session_with_steer(
        &self,
        session: &ProjectSession,
        author: &Author,
        text: &str,
    ) -> StoreResult<()> {
        validate_project_session(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let parameters = project_session_params(session);
        transaction.execute(
            PROJECT_SESSION_INSERT,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        create_project_spine(&transaction, session)?;
        let work = work_for_child_in(&transaction, &ChildRef::Project(session.id.clone()))?;
        Self::append_steer_in(&transaction, &work, author, text)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn update_project_session(&self, session: &ProjectSession) -> StoreResult<()> {
        validate_project_session(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let parameters = project_session_control_params(session);
        let changed = transaction.execute(
            PROJECT_SESSION_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        close_project_epoch_if_quiescent(&transaction, session)?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn activate_project_process_for_run(
        &self,
        session: &ProjectSession,
        lease: &RunLease,
    ) -> StoreResult<()> {
        validate_project_session(session)?;
        let process = session.latest_process.as_ref().ok_or_else(|| {
            StoreError::InvalidData("Project activation requires process evidence".to_string())
        })?;
        if process.state != ChildLeaseState::Active {
            return Err(StoreError::InvalidData(
                "Project activation requires Active process evidence".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if update_project_session_for_run_in(&transaction, session, lease)? == 0 {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot activate Project Session {}",
                lease.run_id, session.id
            )));
        }
        insert_project_event_in(
            &transaction,
            session,
            &ProjectEventKind::BodyLeaseChanged {
                process: process.clone(),
            },
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn update_project_session_for_run(
        &self,
        session: &ProjectSession,
        lease: &RunLease,
    ) -> StoreResult<()> {
        validate_project_session(session)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        if update_project_session_for_run_in(&conn, session, lease)? == 0 {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot update Project Session {}",
                lease.run_id, session.id
            )));
        }
        Ok(())
    }

    pub(crate) fn finish_project_process_for_run(
        &self,
        session: &ProjectSession,
        lease: &RunLease,
    ) -> StoreResult<()> {
        validate_project_session(session)?;
        let process = session.latest_process.as_ref().ok_or_else(|| {
            StoreError::InvalidData("Project finish requires process evidence".to_string())
        })?;
        if process.state != ChildLeaseState::Finished || process.outcome.is_none() {
            return Err(StoreError::InvalidData(
                "Project finish requires Finished process evidence and outcome".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if update_project_session_for_run_in(&transaction, session, lease)? == 0 {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot finish Project Session {}",
                lease.run_id, session.id
            )));
        }
        insert_project_event_in(
            &transaction,
            session,
            &ProjectEventKind::BodyLeaseChanged {
                process: process.clone(),
            },
        )?;
        end_run_for_lease(&transaction, lease)?;
        close_project_epoch_if_quiescent(&transaction, session)?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn revoke_project_process(
        &self,
        session_id: &ProjectSessionId,
        outcome: &ChildBodyOutcome,
    ) -> StoreResult<ChildProcessGeneration> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut session = transaction
            .query_row(
                PROJECT_SESSION_SELECT,
                params![session_id.as_str()],
                map_project_session_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        let process = session.latest_process.as_mut().ok_or_else(|| {
            StoreError::InvalidData(format!(
                "Project Session {session_id} has no body to revoke"
            ))
        })?;
        if process.state == ChildLeaseState::Finished {
            return Err(StoreError::InvalidData(format!(
                "Project Session {session_id} generation {} is already finished",
                process.generation
            )));
        }
        process.state = ChildLeaseState::Revoked;
        process.outcome = Some(outcome.clone());
        let outcome_json = serde_json::to_string(outcome)?;
        let changed = transaction.execute(
            "UPDATE project_sessions
             SET process_lease_state='revoked', process_outcome_json=?3
             WHERE id=?1 AND process_generation=?2
               AND process_lease_state IN ('legacy', 'reserved', 'active')",
            params![
                session_id.as_str(),
                i64::from(process.generation),
                outcome_json
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidData(format!(
                "Project Session {session_id} body is already revoked"
            )));
        }
        let process = process.clone();
        insert_project_event_in(
            &transaction,
            &session,
            &ProjectEventKind::BodyLeaseChanged {
                process: process.clone(),
            },
        )?;
        fence_run_for_child(
            &transaction,
            &ChildRef::Project(session_id.clone()),
            process.generation,
        )?;
        transaction.commit()?;
        Ok(process)
    }

    pub(crate) fn finish_revoked_project_process(
        &self,
        session_id: &ProjectSessionId,
        generation: u32,
    ) -> StoreResult<ChildProcessGeneration> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut session = transaction
            .query_row(
                PROJECT_SESSION_SELECT,
                params![session_id.as_str()],
                map_project_session_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        let process = session.latest_process.as_mut().ok_or_else(|| {
            StoreError::InvalidData(format!("Project Session {session_id} has no revoked body"))
        })?;
        if process.generation != generation || process.state != ChildLeaseState::Revoked {
            return Err(StoreError::InvalidData(format!(
                "Project Session {session_id} generation {generation} is not awaiting reap"
            )));
        }
        process.state = ChildLeaseState::Finished;
        if transaction.execute(
            "UPDATE project_sessions SET process_lease_state='finished'
             WHERE id=?1 AND process_generation=?2 AND process_lease_state='revoked'",
            params![session_id.as_str(), i64::from(generation)],
        )? == 0
        {
            return Err(StoreError::InvalidData(format!(
                "Project Session {session_id} generation {generation} changed during reap"
            )));
        }
        let process = process.clone();
        insert_project_event_in(
            &transaction,
            &session,
            &ProjectEventKind::BodyLeaseChanged {
                process: process.clone(),
            },
        )?;
        end_run_for_child(
            &transaction,
            &ChildRef::Project(session_id.clone()),
            generation,
        )?;
        transaction.commit()?;
        Ok(process)
    }

    pub fn handoff_project_body(
        &self,
        session_id: &ProjectSessionId,
        request: &ChildBodyHandoffRequest,
    ) -> StoreResult<ProjectSession> {
        validate_handoff_request(request)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut session = transaction
            .query_row(
                PROJECT_SESSION_SELECT,
                params![session_id.as_str()],
                map_project_session_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        validate_handoff_state(
            "Project",
            &session.launch.project.slug,
            session.status.as_str(),
            session.status.is_process_active(),
            session.status.is_terminal(),
            session.abandon_intent.as_ref(),
        )?;
        let handoff = apply_handoff(
            &mut session.agent,
            &mut session.provider,
            &mut session.provider_session_id,
            request,
        );
        session.updated_at = OffsetDateTime::now_utc();
        validate_project_session(&session)?;
        let parameters = project_session_control_params(&session);
        transaction.execute(
            PROJECT_SESSION_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        insert_project_event_in(
            &transaction,
            &session,
            &ProjectEventKind::BodyHandedOff { handoff },
        )?;
        transaction.commit()?;
        Ok(session)
    }

    pub fn project_session(
        &self,
        session_id: &ProjectSessionId,
    ) -> StoreResult<Option<ProjectSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            PROJECT_SESSION_SELECT,
            params![session_id.as_str()],
            map_project_session_row,
        )
        .optional()
        .map_err(StoreError::from)
    }

    pub fn project_session_by_project(&self, project: &str) -> StoreResult<Option<ProjectSession>> {
        if let Ok(session_id) = ProjectSessionId::parse(project) {
            return self.project_session(&session_id);
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = format!(
            "{PROJECT_SESSION_COLUMNS}
             WHERE project_id=?1 OR project_slug=?1
             ORDER BY created_at DESC, id DESC
             LIMIT 1"
        );
        conn.query_row(&query, params![project], map_project_session_row)
            .optional()
            .map_err(StoreError::from)
    }

    pub fn list_project_sessions(
        &self,
        wave_id: Option<&WaveId>,
    ) -> StoreResult<Vec<ProjectSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = match wave_id {
            Some(_) => {
                format!("{PROJECT_SESSION_COLUMNS} WHERE wave_id=?1 ORDER BY updated_at DESC")
            }
            None => format!("{PROJECT_SESSION_COLUMNS} ORDER BY updated_at DESC"),
        };
        let mut statement = conn.prepare(&query)?;
        let mut sessions = Vec::new();
        if let Some(wave_id) = wave_id {
            let rows = statement.query_map(params![wave_id], map_project_session_row)?;
            for row in rows {
                sessions.push(row?);
            }
        } else {
            let rows = statement.query_map([], map_project_session_row)?;
            for row in rows {
                sessions.push(row?);
            }
        }
        Ok(sessions)
    }

    pub(crate) fn stop_project_for_run(
        &self,
        session_id: &ProjectSessionId,
        lease: &RunLease,
        stopped_status: ProjectSessionStatus,
        reason: &str,
    ) -> StoreResult<ProjectSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut session = transaction
            .query_row(
                PROJECT_SESSION_SELECT,
                params![session_id.as_str()],
                map_project_session_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        session.set_status(stopped_status, reason);
        if update_project_session_for_run_in(&transaction, &session, lease)? == 0 {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} cannot stop Project Session {session_id}",
                lease.run_id
            )));
        }
        transaction.commit()?;
        Ok(session)
    }

    pub fn append_project_event(
        &self,
        session_id: &ProjectSessionId,
        kind: &ProjectEventKind,
    ) -> StoreResult<ProjectEvent> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session = transaction.query_row(
            PROJECT_SESSION_SELECT,
            params![session_id.as_str()],
            map_project_session_row,
        )?;
        let event = insert_project_event_in(&transaction, &session, kind)?;
        transaction.commit()?;
        Ok(event)
    }

    pub(crate) fn append_project_event_for_run(
        &self,
        session_id: &ProjectSessionId,
        lease: &RunLease,
        kind: &ProjectEventKind,
    ) -> StoreResult<ProjectEvent> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_run_owns_child(&transaction, &ChildRef::Project(session_id.clone()), lease)?;
        let session = transaction.query_row(
            PROJECT_SESSION_SELECT,
            params![session_id.as_str()],
            map_project_session_row,
        )?;
        let event = insert_project_event_in(&transaction, &session, kind)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn project_events_after(
        &self,
        session_id: &ProjectSessionId,
        cursor: i64,
    ) -> StoreResult<Vec<ProjectEvent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT id, session_id, kind_json, created_at
             FROM project_events WHERE session_id=?1 AND id>?2 ORDER BY id",
        )?;
        let rows =
            statement.query_map(params![session_id.as_str(), cursor], map_project_event_row)?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    /// When this Project Session last appended a durable event. The progress
    /// signal for the Project body observation, mirroring [`Self::latest_task_event_at`].
    pub fn latest_project_event_at(
        &self,
        session_id: &ProjectSessionId,
    ) -> StoreResult<Option<OffsetDateTime>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let seconds: Option<i64> = conn.query_row(
            "SELECT MAX(created_at) FROM project_events WHERE session_id = ?1",
            params![session_id.as_str()],
            |row| row.get(0),
        )?;
        Ok(seconds.map(crate::store::rows::unix_to_datetime))
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

    pub fn pending_project_observations_for_chain(
        &self,
        project_id: &str,
    ) -> StoreResult<Vec<ObservationOutboxRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT id, recipient_kind, recipient_id, source_kind, source_id,
                    event_id, payload_json, delivered_at
             FROM observation_outbox
             WHERE recipient_kind='project'
               AND recipient_id IN (SELECT id FROM project_sessions WHERE project_id=?1)
               AND delivered_at IS NULL
             ORDER BY id",
        )?;
        let rows = statement.query_map(params![project_id], map_observation_row)?;
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
        project_session_id: &ProjectSessionId,
        observation: &ObservationOutboxRow,
    ) -> StoreResult<bool> {
        self.consume_task_observation_for_project_with_authority(
            project_session_id,
            observation,
            None,
        )
    }

    pub(crate) fn consume_task_observation_for_project_for_run(
        &self,
        project_session_id: &ProjectSessionId,
        observation: &ObservationOutboxRow,
        lease: &RunLease,
    ) -> StoreResult<bool> {
        self.consume_task_observation_for_project_with_authority(
            project_session_id,
            observation,
            Some(lease),
        )
    }

    fn consume_task_observation_for_project_with_authority(
        &self,
        project_session_id: &ProjectSessionId,
        observation: &ObservationOutboxRow,
        run_lease: Option<&RunLease>,
    ) -> StoreResult<bool> {
        let (
            ObservationRecipient::Project {
                session_id: recipient_id,
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
                "Project Session can consume only supervised Task observations".to_string(),
            ));
        };
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(lease) = run_lease {
            require_run_owns_child(
                &transaction,
                &ChildRef::Project(project_session_id.clone()),
                lease,
            )?;
        }
        // The outbox recipient is the Project Session the Task was born under
        // (provenance). A live successor consumes observations addressed to any
        // session in its project chain; the recipient must share the consuming
        // successor's Linear project id, but it need not be the successor itself.
        if recipient_id != project_session_id {
            let recipient_project_id: Option<String> = transaction
                .query_row(
                    "SELECT project_id FROM project_sessions WHERE id=?1",
                    params![recipient_id.as_str()],
                    |row| row.get(0),
                )
                .optional()?;
            let successor_project_id: String = transaction.query_row(
                "SELECT project_id FROM project_sessions WHERE id=?1",
                params![project_session_id.as_str()],
                |row| row.get(0),
            )?;
            match recipient_project_id {
                Some(recipient_project_id) if recipient_project_id == successor_project_id => {}
                Some(_) => {
                    return Err(StoreError::InvalidData(format!(
                        "observation {} belongs to Project Session {recipient_id} outside the chain of {project_session_id}",
                        observation.id
                    )));
                }
                None => {
                    return Err(StoreError::InvalidData(format!(
                        "observation {} belongs to unknown Project Session {recipient_id}",
                        observation.id
                    )));
                }
            }
        }
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM project_events
                WHERE session_id=?1
                  AND json_extract(kind_json, '$.kind')='task_observed'
                  AND json_extract(kind_json, '$.task_session_id')=?2
                  AND json_extract(kind_json, '$.task_event_id')=?3
             )",
            params![
                project_session_id.as_str(),
                task_id.as_str(),
                observation.event_id,
            ],
            |row| row.get(0),
        )?;
        if !exists {
            let kind = ProjectEventKind::TaskObserved {
                task_session_id: task_id.clone(),
                task_event_id: observation.event_id,
                event: Box::new(event.clone()),
            };
            transaction.execute(
                "INSERT INTO project_events (session_id, kind_json, created_at)
                 VALUES (?1, ?2, ?3)",
                params![
                    project_session_id.as_str(),
                    serde_json::to_string(&kind)?,
                    now_unix(),
                ],
            )?;
        }
        let now = now_unix();
        transaction.execute(
            "UPDATE observation_outbox SET delivered_at=?1
             WHERE id=?2 AND delivered_at IS NULL",
            params![now, observation.id],
        )?;
        transaction.execute(
            "UPDATE project_sessions
             SET observation_cursor=MAX(observation_cursor, ?1), updated_at=?2
             WHERE id=?3",
            params![observation.id, now, project_session_id.as_str()],
        )?;
        transaction.commit()?;
        Ok(!exists)
    }
}

fn validate_task_session(session: &TaskSession) -> StoreResult<()> {
    session
        .validate()
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

/// Select the current Task Session from every row sharing an issue, identifier,
/// or worktree key. The unique live successor wins; terminal predecessors
/// (completed/abandoned) are history and never win while a live successor
/// exists. When only terminal history remains, the most recent predecessor is
/// returned so `task status`, completion, and PR webhook reads still resolve a
/// Task rather than reporting none. Two or more live successors are actionable
/// ambiguity, not a silent pick — the partial unique indexes guarantee one live
/// per key, but a lookup keyed on `issue_id OR issue_identifier` can still match
/// two different live rows on the two columns.
fn resolve_current_task_session(
    key: &str,
    sessions: Vec<TaskSession>,
) -> StoreResult<Option<TaskSession>> {
    let mut live: Vec<TaskSession> = sessions
        .iter()
        .filter(|session| !session.status.is_terminal())
        .cloned()
        .collect();
    match live.len() {
        0 => Ok(sessions
            .into_iter()
            .max_by_key(|session| (session.updated_at, session.created_at))),
        1 => Ok(live.pop()),
        count => {
            live.sort_by_key(|session| session.created_at);
            let detail = live
                .iter()
                .map(|session| {
                    format!(
                        "{} ({}, {})",
                        session.id.as_str(),
                        session.launch.issue.identifier,
                        session.status.as_str()
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            Err(StoreError::InvalidData(format!(
                "{count} live Task Sessions resolve to {key:?}: {detail}; \
                 stop all but one before operating"
            )))
        }
    }
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
    status: &str,
    process_active: bool,
    terminal: bool,
    abandon_intent: Option<&AbandonIntent>,
) -> StoreResult<()> {
    if terminal {
        return Err(StoreError::InvalidData(format!(
            "{kind} {label} is {status}; terminal Sessions cannot hand off bodies"
        )));
    }
    if let Some(intent) = abandon_intent {
        return Err(StoreError::InvalidData(format!(
            "{kind} {label} is being abandoned: {}",
            intent.reason
        )));
    }
    if process_active {
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

fn validate_initial_task_pr(session: &TaskSession, pr: &TaskPr) -> StoreResult<()> {
    validate_task_pr(pr)?;
    if pr.task_session_id != session.id || pr.sequence != 1 || pr.phase() != PrPhase::Working {
        return Err(StoreError::InvalidData(
            "Task Session requires its sequence-1 Working PR".to_string(),
        ));
    }
    Ok(())
}

fn insert_initial_task(
    conn: &rusqlite::Transaction<'_>,
    session: &TaskSession,
    pr: &TaskPr,
) -> StoreResult<()> {
    validate_task_session(session)?;
    validate_initial_task_pr(session, pr)?;
    validate_task_project_session(conn, session)?;
    let parameters = task_session_params(session);
    conn.execute(
        TASK_SESSION_INSERT,
        rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
    )?;
    create_task_spine(conn, session)?;
    insert_task_pr(conn, pr)?;
    seed_task_linear_observation(conn, session)
}

fn close_task_epoch_if_quiescent(conn: &Connection, session: &TaskSession) -> StoreResult<()> {
    if !session
        .latest_process
        .as_ref()
        .is_none_or(|process| process.state == ChildLeaseState::Finished)
    {
        return Ok(());
    }
    let state = match session.status {
        TaskSessionStatus::Completed => "done",
        TaskSessionStatus::Abandoned => "abandoned",
        _ => return Ok(()),
    };
    conn.execute(
        "UPDATE epochs SET state=?2, terminal_at=?3
         WHERE id=(SELECT epoch_id FROM task_sessions WHERE id=?1)
           AND state='open'",
        params![
            session.id.as_str(),
            state,
            session.updated_at.unix_timestamp()
        ],
    )?;
    Ok(())
}

fn close_project_epoch_if_quiescent(
    conn: &Connection,
    session: &ProjectSession,
) -> StoreResult<()> {
    if !session
        .latest_process
        .as_ref()
        .is_none_or(|process| process.state == ChildLeaseState::Finished)
    {
        return Ok(());
    }
    let state = match session.status {
        ProjectSessionStatus::Completed => "done",
        ProjectSessionStatus::Abandoned => "abandoned",
        _ => return Ok(()),
    };
    conn.execute(
        "UPDATE epochs SET state=?2, terminal_at=?3
         WHERE id=(SELECT epoch_id FROM project_sessions WHERE id=?1)
           AND state='open'",
        params![
            session.id.as_str(),
            state,
            session.updated_at.unix_timestamp()
        ],
    )?;
    Ok(())
}

/// Seed the Linear observation cursor from the launch snapshot, in the Session's
/// creation transaction. Webhooks only fire for changes *after* subscription, so
/// there is no cursor to build lazily on a first poll — seeding here means the
/// first issue-edit webhook diffs against the launch title/description instead of
/// baselining (and swallowing) it. The revision seeds empty so any real Linear
/// `updatedAt` wins the monotonic guard.
fn seed_task_linear_observation(conn: &Connection, session: &TaskSession) -> StoreResult<()> {
    conn.execute(
        "INSERT OR IGNORE INTO task_linear_observations (
            session_id, last_revision, last_title, last_description,
            last_success_at, degraded_reason, updated_at
         ) VALUES (?1, '', ?2, ?3, ?4, NULL, ?4)",
        params![
            session.id.as_str(),
            session.launch.issue.title,
            session.launch.issue.description,
            now_unix(),
        ],
    )?;
    Ok(())
}

/// Move the live Linear ingress state onto a Task successor and link the
/// successor back to its terminal predecessor. Historical commands and
/// directives stay on the predecessor for attribution.
fn _carry_task_session_state(
    conn: &Connection,
    predecessor: &TaskSession,
    successor: &TaskSession,
) -> StoreResult<()> {
    // Move the cursor rather than copying it: an already-applied Linear edit
    // must not replay when a successor becomes current.
    conn.execute(
        "UPDATE task_linear_observations SET session_id=?2 WHERE session_id=?1",
        params![predecessor.id.as_str(), successor.id.as_str()],
    )?;
    // Legacy predecessors may have no cursor. Seed only when the move above
    // found nothing.
    seed_task_linear_observation(conn, successor)?;
    // A previously delivered Linear comment must not become a second follow-up
    // on the successor.
    conn.execute(
        "UPDATE task_linear_ingested_comments SET session_id=?2 WHERE session_id=?1",
        params![predecessor.id.as_str(), successor.id.as_str()],
    )?;
    conn.execute(
        "UPDATE task_sessions SET predecessor_session_id=?2 WHERE id=?1",
        params![successor.id.as_str(), predecessor.id.as_str()],
    )?;
    Ok(())
}

/// The single non-terminal Task Session for a Linear issue, other than the
/// predecessor. The partial unique index `idx_task_sessions_one_current_issue`
/// guarantees at most one, so this is the idempotency probe for a carry
/// transaction: a crash or concurrent run that already created the successor
/// surfaces here instead of re-keying direction a second time.
fn non_terminal_successor_for_issue(
    conn: &Connection,
    predecessor_id: &str,
    issue_id: &str,
) -> StoreResult<Option<TaskSession>> {
    let query = format!(
        "{TASK_SESSION_COLUMNS} WHERE issue_id=?1 AND id != ?2 \
         AND status NOT IN ('completed', 'abandoned') ORDER BY created_at"
    );
    let mut statement = conn.prepare(&query)?;
    let mut rows = statement.query_map(params![issue_id, predecessor_id], map_task_session_row)?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

fn validate_task_project_session(conn: &Connection, session: &TaskSession) -> StoreResult<()> {
    let owner = conn
        .query_row(
            "SELECT project_id, wave_id FROM project_sessions WHERE id=?1",
            params![session.project_session_id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let Some((project_id, wave_id)) = owner else {
        return Err(StoreError::InvalidData(format!(
            "Task Session {} requires Project Session {}",
            session.id, session.project_session_id
        )));
    };
    if project_id != session.launch.project.id.as_str() || wave_id != session.wave_id.as_str() {
        return Err(StoreError::InvalidData(format!(
            "Project Session {} does not own Task {}/{}",
            session.project_session_id, session.wave_id, session.launch.project.slug
        )));
    }
    Ok(())
}

fn validate_project_session(session: &ProjectSession) -> StoreResult<()> {
    session
        .validate()
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

const TASK_SESSION_INSERT: &str = "INSERT INTO task_sessions (
    id, issue_id, issue_identifier, issue_title, issue_description,
    project_id, project_slug, project_name, project_prompt_context, wave_id,
    status, status_reason, status_at, worktree, workspace_slug,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, created_at, updated_at,
    pm_snapshot_synced_at,
    pm_writeback_json, project_session_id,
    lf_bin, db_path, lf_home, abandon_requested_at, abandon_reason,
    iterate_flow, iterate_interaction_policy, phase_cursor, phase_iteration,
    process_group_id, process_agent, process_provider,
    process_provider_session_id, process_lease_state, process_outcome_json,
    kickoff_flow, kickoff_interaction_policy, gate_flow, gate_interaction_policy,
    lifecycle_phase, phase_epoch, gate_cycle, gate_proposal_json,
    process_provenance_json
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
    ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36,
    ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44, ?45, ?46, ?47, ?48, ?49, ?50, ?51
)";
const TASK_SESSION_COLUMNS: &str = "SELECT
    id, issue_id, issue_identifier, issue_title, issue_description,
    project_id, project_slug, project_name, project_prompt_context, wave_id,
    status, status_reason, status_at, worktree, workspace_slug,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, created_at, updated_at,
    pm_snapshot_synced_at,
    pm_writeback_json, project_session_id,
    lf_bin, db_path, lf_home, abandon_requested_at, abandon_reason,
    iterate_flow, iterate_interaction_policy, phase_cursor, phase_iteration,
    process_group_id, process_agent, process_provider,
    process_provider_session_id, process_lease_state, process_outcome_json,
    kickoff_flow, kickoff_interaction_policy, gate_flow, gate_interaction_policy,
    lifecycle_phase, phase_epoch, gate_cycle, gate_proposal_json,
    process_provenance_json
    FROM task_sessions";
pub(super) const TASK_SESSION_SELECT: &str = "SELECT
    id, issue_id, issue_identifier, issue_title, issue_description,
    project_id, project_slug, project_name, project_prompt_context, wave_id,
    status, status_reason, status_at, worktree, workspace_slug,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, created_at, updated_at,
    pm_snapshot_synced_at,
    pm_writeback_json, project_session_id,
    lf_bin, db_path, lf_home, abandon_requested_at, abandon_reason,
    iterate_flow, iterate_interaction_policy, phase_cursor, phase_iteration,
    process_group_id, process_agent, process_provider,
    process_provider_session_id, process_lease_state, process_outcome_json,
    kickoff_flow, kickoff_interaction_policy, gate_flow, gate_interaction_policy,
    lifecycle_phase, phase_epoch, gate_cycle, gate_proposal_json,
    process_provenance_json
    FROM task_sessions WHERE id = ?1";
const TASK_SESSION_UPDATE: &str = "UPDATE task_sessions SET
    issue_id=?2, issue_identifier=?3, issue_title=?4, issue_description=?5,
    project_id=?6, project_slug=?7, project_name=?8, project_prompt_context=?9,
    wave_id=?10, status=?11, status_reason=?12, status_at=?13,
    worktree=?14, workspace_slug=?15, agent=?16, provider=?17,
    provider_session_id=?18,
    created_at=?23, updated_at=?24,
    pm_snapshot_synced_at=?25, pm_writeback_json=?26,
    project_session_id=?27,
    lf_bin=?28, db_path=?29, lf_home=?30,
    abandon_requested_at=?31, abandon_reason=?32,
    iterate_flow=?33, iterate_interaction_policy=?34,
    kickoff_flow=?43, kickoff_interaction_policy=?44,
    gate_flow=?45, gate_interaction_policy=?46
    WHERE id=?1";
const TASK_SESSION_RUN_UPDATE: &str = "UPDATE task_sessions SET
    issue_id=?2, issue_identifier=?3, issue_title=?4, issue_description=?5,
    project_id=?6, project_slug=?7, project_name=?8, project_prompt_context=?9,
    wave_id=?10, status=?11, status_reason=?12, status_at=?13,
    worktree=?14, workspace_slug=?15, agent=?16, provider=?17,
    provider_session_id=?18, process_generation=?19, process_pid=?20,
    process_tmux_name=?21, process_started_at=?22,
    created_at=?23, updated_at=?24,
    pm_snapshot_synced_at=?25, pm_writeback_json=?26,
    project_session_id=?27,
    lf_bin=?28, db_path=?29, lf_home=?30,
    abandon_requested_at=?31, abandon_reason=?32,
    lifecycle_phase=CASE WHEN ?48>=phase_epoch THEN ?47 ELSE lifecycle_phase END,
    phase_cursor=CASE
        WHEN ?48>phase_epoch OR
             (?48=phase_epoch AND (?36>phase_iteration OR
                                   (?36=phase_iteration AND ?35>phase_cursor)))
        THEN ?35 ELSE phase_cursor
    END,
    phase_iteration=CASE
        WHEN ?48>phase_epoch THEN ?36
        WHEN ?48=phase_epoch THEN MAX(phase_iteration, ?36)
        ELSE phase_iteration
    END,
    phase_epoch=MAX(phase_epoch, ?48),
    gate_cycle=CASE WHEN ?48>=phase_epoch THEN ?49 ELSE gate_cycle END,
    gate_proposal_json=CASE WHEN ?48>=phase_epoch THEN ?50 ELSE gate_proposal_json END,
    process_group_id=?37, process_agent=?38, process_provider=?39,
    process_provider_session_id=?40, process_lease_state=?41,
    process_outcome_json=?42, process_provenance_json=?51
    WHERE id=?1 AND (
        status NOT IN ('completed', 'abandoned') OR status=?11 OR
        (status='completed' AND ?11='running' AND ?47='gate' AND ?48>phase_epoch)
    )";
const TASK_PR_COLUMNS: &str = "SELECT
    id, task_session_id, sequence, slug, branch, base_commit,
    publication_requested_at, after_merge, next_slug, github_number, github_url,
    merge_commit, abandoned_at, created_at, updated_at,
    github_head_sha, ci_observation, parent_pr_id, github_observation,
    linear_attachment_id, linear_comment_id, linear_link_error
    FROM task_prs";
const TASK_PR_SELECT: &str = "SELECT
    id, task_session_id, sequence, slug, branch, base_commit,
    publication_requested_at, after_merge, next_slug, github_number, github_url,
    merge_commit, abandoned_at, created_at, updated_at,
    github_head_sha, ci_observation, parent_pr_id, github_observation,
    linear_attachment_id, linear_comment_id, linear_link_error
    FROM task_prs WHERE id=?1";
/// Persist one Linear comment as a Steer exactly once. The insert
/// into `task_linear_ingested_comments` is the guard — the command is written
/// only when the comment id is new to the ledger, so a redelivered webhook or an
/// overlapping catch-up read cannot double-deliver. Shared by the snapshot apply
/// loop and the single-comment webhook path.
fn ingest_linear_comment(
    conn: &rusqlite::Transaction<'_>,
    session_id: &str,
    comment_id: &str,
    work: &crate::durable::WorkRef,
    text: &str,
    observed_at: i64,
) -> StoreResult<Option<crate::durable::SteerId>> {
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO task_linear_ingested_comments
            (session_id, comment_id, ingested_at) VALUES (?1, ?2, ?3)",
        params![session_id, comment_id, observed_at],
    )?;
    if inserted == 1 {
        let receipt = SqliteStore::append_steer_in(conn, work, &Author::User, text)?;
        Ok(Some(receipt.steer.id))
    } else {
        Ok(None)
    }
}

fn task_session_params(session: &TaskSession) -> Vec<Box<dyn ToSql>> {
    vec![
        Box::new(session.id.as_str().to_string()),
        Box::new(session.launch.issue.id.as_str().to_string()),
        Box::new(session.launch.issue.identifier.clone()),
        Box::new(session.launch.issue.title.clone()),
        Box::new(session.launch.issue.description.clone()),
        Box::new(session.launch.project.id.as_str().to_string()),
        Box::new(session.launch.project.slug.clone()),
        Box::new(session.launch.project.name.clone()),
        Box::new(session.launch.project.prompt_context.clone()),
        Box::new(session.wave_id.clone()),
        Box::new(session.status.as_str().to_string()),
        Box::new(session.status_reason.clone()),
        Box::new(session.status_at.unix_timestamp()),
        Box::new(session.worktree.display().to_string()),
        Box::new(session.workspace_slug.clone()),
        Box::new(session.agent.clone()),
        Box::new(session.provider.clone()),
        Box::new(session.provider_session_id.clone()),
        Box::new(
            session
                .latest_process
                .as_ref()
                .map(|process| i64::from(process.generation)),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .and_then(|process| process.pid.map(i64::from)),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .map(|process| process.tmux_name.clone()),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .map(|process| process.started_at.unix_timestamp()),
        ),
        Box::new(session.created_at.unix_timestamp()),
        Box::new(session.updated_at.unix_timestamp()),
        Box::new(session.launch.pm_snapshot_synced_at),
        Box::new(
            serde_json::to_string(&session.pm_writeback)
                .expect("Task Session PM writeback state must serialize"),
        ),
        Box::new(session.project_session_id.as_str().to_string()),
        // Legacy lf_bin/db_path/lf_home columns: a Session no longer pins a
        // binary. Launch resolves the current Home lf; BinaryProvenance on the
        // generation is the audit record. Written NULL, never read; the columns
        // are dropped by the earned table rebuild, not this change.
        Box::new(None::<String>),
        Box::new(None::<String>),
        Box::new(None::<String>),
        Box::new(
            session
                .abandon_intent
                .as_ref()
                .map(|intent| intent.requested_at.unix_timestamp()),
        ),
        Box::new(
            session
                .abandon_intent
                .as_ref()
                .map(|intent| intent.reason.clone()),
        ),
        Box::new(session.lifecycle.iterate.flow.clone()),
        Box::new(
            session
                .lifecycle
                .iterate
                .interaction_policy
                .as_str()
                .to_string(),
        ),
        Box::new(i64::from(session.phase_cursor)),
        Box::new(i64::from(session.phase_iteration)),
        Box::new(
            session
                .latest_process
                .as_ref()
                .and_then(|process| process.process_group_id.map(i64::from)),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .map(|process| process.agent.clone()),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .map(|process| process.provider.clone()),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .and_then(|process| process.provider_session_id.clone()),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .map(|process| process.state.as_str().to_string()),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .and_then(|process| process.outcome.as_ref())
                .map(|outcome| {
                    serde_json::to_string(outcome).expect("child body outcome must serialize")
                }),
        ),
        Box::new(session.lifecycle.kickoff.flow.clone()),
        Box::new(
            session
                .lifecycle
                .kickoff
                .interaction_policy
                .as_str()
                .to_string(),
        ),
        Box::new(session.lifecycle.gate.flow.clone()),
        Box::new(
            session
                .lifecycle
                .gate
                .interaction_policy
                .as_str()
                .to_string(),
        ),
        Box::new(session.lifecycle_phase.as_str().to_string()),
        Box::new(i64::from(session.phase_epoch)),
        Box::new(i64::from(session.gate_cycle)),
        Box::new(session.gate_proposal.as_ref().map(|proposal| {
            serde_json::to_string(proposal).expect("Task gate proposal must serialize")
        })),
        Box::new(
            session
                .latest_process
                .as_ref()
                .and_then(|process| process.provenance.as_ref())
                .map(|provenance| {
                    serde_json::to_string(provenance).expect("child body provenance must serialize")
                }),
        ),
    ]
}

/// `TASK_SESSION_UPDATE`'s highest bound parameter. The control statement owns
/// configuration; the lease statement owns execution state. Parameters
/// ?37..?44 are bound but unreferenced because lease state is interleaved with
/// the lifecycle configuration in `task_session_params`.
const TASK_SESSION_CONTROL_PARAMS: usize = 46;

fn task_session_control_params(session: &TaskSession) -> Vec<Box<dyn ToSql>> {
    let mut parameters = task_session_params(session);
    parameters.truncate(TASK_SESSION_CONTROL_PARAMS);
    parameters
}

fn update_task_session_for_run_in(
    conn: &Connection,
    session: &TaskSession,
    lease: &RunLease,
) -> StoreResult<usize> {
    require_run_owns_child(conn, &ChildRef::Task(session.id.clone()), lease)?;
    let parameters = task_session_params(session);
    Ok(conn.execute(
        TASK_SESSION_RUN_UPDATE,
        rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
    )?)
}

fn require_run_owns_child(
    conn: &Connection,
    target: &ChildRef,
    lease: &RunLease,
) -> StoreResult<()> {
    let run = validate_run_lease(conn, lease)?;
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
    let github = publication.and_then(|publication| publication.github.as_ref());
    conn.execute(
        "INSERT INTO task_prs (
            id, task_session_id, sequence, slug, branch, base_commit,
            publication_requested_at, after_merge, next_slug,
            github_number, github_url, merge_commit, abandoned_at,
            created_at, updated_at, github_head_sha, ci_observation, parent_pr_id,
            github_observation,
            linear_attachment_id, linear_comment_id, linear_link_error
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
        params![
            pr.id.as_str(),
            pr.task_session_id.as_str(),
            i64::from(pr.sequence),
            pr.slug,
            pr.branch,
            pr.base_commit,
            publication.map(|publication| publication.requested_at.unix_timestamp()),
            publication.map(|publication| publication.after_merge.as_str()),
            publication.and_then(|publication| publication.next_slug.as_deref()),
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
        ],
    )?;
    Ok(())
}

fn update_task_pr(conn: &Connection, pr: &TaskPr) -> StoreResult<usize> {
    validate_task_pr(pr)?;
    let publication = pr.publication.as_ref();
    let github = publication.and_then(|publication| publication.github.as_ref());
    conn.execute(
        "UPDATE task_prs SET
            publication_requested_at=?7, after_merge=?8, next_slug=?9,
            github_number=?10, github_url=?11, merge_commit=?12,
            abandoned_at=?13, updated_at=?15, github_head_sha=?16,
            ci_observation=?17, parent_pr_id=?18, github_observation=?19,
            linear_attachment_id=?20, linear_comment_id=?21, linear_link_error=?22
         WHERE id=?1 AND task_session_id=?2 AND sequence=?3 AND slug=?4
           AND branch=?5 AND base_commit=?6 AND created_at=?14",
        params![
            pr.id.as_str(),
            pr.task_session_id.as_str(),
            i64::from(pr.sequence),
            pr.slug,
            pr.branch,
            pr.base_commit,
            publication.map(|publication| publication.requested_at.unix_timestamp()),
            publication.map(|publication| publication.after_merge.as_str()),
            publication.and_then(|publication| publication.next_slug.as_deref()),
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
        ],
    )
    .map_err(StoreError::from)
}

/// Move a Task PR's `base_commit` range anchor forward. `base_commit` is part of
/// `update_task_pr`'s optimistic identity, so healing it needs a dedicated write
/// keyed on the row's true identity (id + session + sequence).
fn heal_task_pr_base(conn: &Connection, pr: &TaskPr) -> StoreResult<usize> {
    validate_task_pr(pr)?;
    conn.execute(
        "UPDATE task_prs SET base_commit=?4, updated_at=?5
         WHERE id=?1 AND task_session_id=?2 AND sequence=?3",
        params![
            pr.id.as_str(),
            pr.task_session_id.as_str(),
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
        if next.task_session_id != settled.task_session_id
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
    let query = format!("{TASK_PR_COLUMNS} WHERE task_session_id=?1 AND sequence=?2");
    let existing = conn
        .query_row(
            &query,
            params![next.task_session_id.as_str(), i64::from(next.sequence)],
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

fn same_task_pr(left: &TaskPr, right: &TaskPr) -> bool {
    left.id == right.id
        && left.task_session_id == right.task_session_id
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

pub(super) fn map_task_session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskSession> {
    let status_text: String = row.get(10)?;
    let status = status_text
        .parse()
        .map_err(|error| invalid_column(10, error))?;
    let process_generation: Option<i64> = row.get(18)?;
    let process_started_at: Option<i64> = row.get(21)?;
    let process = match (process_generation, process_started_at) {
        (Some(generation), Some(started_at)) => {
            let state_text: String = row.get(40)?;
            let outcome_json: Option<String> = row.get(41)?;
            let provenance_json: Option<String> = row.get(50)?;
            Some(ChildProcessGeneration {
                generation: generation as u32,
                pid: row.get::<_, Option<i64>>(19)?.map(|pid| pid as u32),
                process_group_id: row.get::<_, Option<i64>>(36)?.map(|id| id as u32),
                tmux_name: row.get::<_, Option<String>>(20)?.unwrap_or_default(),
                agent: row.get(37)?,
                provider: row.get(38)?,
                provider_session_id: row.get(39)?,
                started_at: crate::store::rows::unix_to_datetime(started_at),
                state: ChildLeaseState::parse(&state_text)
                    .map_err(|error| invalid_column(40, error))?,
                outcome: outcome_json
                    .map(|json| serde_json::from_str(&json))
                    .transpose()
                    .map_err(|error| invalid_column(41, error))?,
                provenance: provenance_json
                    .map(|json| serde_json::from_str(&json))
                    .transpose()
                    .map_err(|error| invalid_column(50, error))?,
            })
        }
        _ => None,
    };
    // Columns 27/28/29 (lf_bin/db_path/lf_home) are legacy dead schema: a
    // Session no longer pins a binary, so they are not read into domain state.
    let abandon_intent = match (
        row.get::<_, Option<i64>>(30)?,
        row.get::<_, Option<String>>(31)?,
    ) {
        (Some(requested_at), Some(reason)) => Some(AbandonIntent {
            requested_at: crate::store::rows::unix_to_datetime(requested_at),
            reason,
        }),
        _ => None,
    };
    Ok(TaskSession {
        id: TaskSessionId::from_raw(row.get::<_, String>(0)?),
        launch: crate::session_context::TaskLaunchReceipt {
            issue: LinearIssueSnapshot {
                id: LinearIssueId::from_raw(row.get::<_, String>(1)?),
                identifier: row.get(2)?,
                title: row.get(3)?,
                description: row.get(4)?,
            },
            project: LinearProjectSnapshot {
                id: LinearProjectId::from_raw(row.get::<_, String>(5)?),
                slug: row.get(6)?,
                name: row.get(7)?,
                prompt_context: row.get(8)?,
            },
            pm_snapshot_synced_at: row.get(24)?,
        },
        pm_writeback: serde_json::from_str(&row.get::<_, String>(25)?)
            .map_err(|error| invalid_column(25, error))?,
        wave_id: row.get(9)?,
        project_session_id: ProjectSessionId::from_raw(row.get::<_, String>(26)?),
        status,
        status_reason: row.get(11)?,
        status_at: crate::store::rows::unix_to_datetime(row.get(12)?),
        worktree: PathBuf::from(row.get::<_, String>(13)?),
        workspace_slug: row.get(14)?,
        lifecycle: TaskLifecyclePlan {
            kickoff: TaskPhasePlan {
                flow: row.get(42)?,
                interaction_policy: row
                    .get::<_, String>(43)?
                    .parse::<InteractionPolicy>()
                    .map_err(|error| invalid_column(43, error))?,
            },
            iterate: TaskPhasePlan {
                flow: row.get(32)?,
                interaction_policy: row
                    .get::<_, String>(33)?
                    .parse::<InteractionPolicy>()
                    .map_err(|error| invalid_column(33, error))?,
            },
            gate: TaskPhasePlan {
                flow: row.get(44)?,
                interaction_policy: row
                    .get::<_, String>(45)?
                    .parse::<InteractionPolicy>()
                    .map_err(|error| invalid_column(45, error))?,
            },
        },
        lifecycle_phase: row
            .get::<_, String>(46)?
            .parse::<TaskLifecyclePhase>()
            .map_err(|error| invalid_column(46, error))?,
        phase_epoch: row.get::<_, i64>(47)? as u32,
        phase_cursor: row.get::<_, i64>(34)? as u32,
        phase_iteration: row.get::<_, i64>(35)? as u32,
        gate_cycle: row.get::<_, i64>(48)? as u32,
        gate_proposal: row
            .get::<_, Option<String>>(49)?
            .map(|json| serde_json::from_str(&json))
            .transpose()
            .map_err(|error| invalid_column(49, error))?,
        agent: row.get(15)?,
        provider: row.get(16)?,
        provider_session_id: row.get(17)?,
        latest_process: process,
        abandon_intent,
        created_at: crate::store::rows::unix_to_datetime(row.get(22)?),
        updated_at: crate::store::rows::unix_to_datetime(row.get(23)?),
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
    let publication = match (publication_requested_at, after_merge) {
        (Some(requested_at), Some(after_merge)) => Some(PrPublication {
            requested_at: crate::store::rows::unix_to_datetime(requested_at),
            after_merge,
            next_slug: row.get(8)?,
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
        }),
        (None, None) => None,
        _ => {
            return Err(invalid_column(
                6,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "PR publication timestamp and disposition must both be present or absent",
                ),
            ))
        }
    };
    let pr = TaskPr {
        id: TaskPrId::from_raw(row.get::<_, String>(0)?),
        task_session_id: TaskSessionId::from_raw(row.get::<_, String>(1)?),
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
    pr.validate().map_err(|error| invalid_column(6, error))?;
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
        session_id: TaskSessionId::from_raw(row.get::<_, String>(0)?),
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
        session_id: TaskSessionId::from_raw(row.get::<_, String>(1)?),
        kind,
        created_at: crate::store::rows::unix_to_datetime(row.get(3)?),
    })
}

const PROJECT_SESSION_INSERT: &str = "INSERT INTO project_sessions (
    id, project_id, project_slug, project_name, project_prompt_context,
    wave_id, pm_snapshot_synced_at, status,
    status_reason, status_at, iteration, observation_cursor,
    last_state_fingerprint, agent, provider, provider_session_id,
    process_generation, process_pid, process_tmux_name,
    process_started_at, created_at, updated_at,
    lf_bin, db_path, lf_home, abandon_requested_at, abandon_reason,
    process_group_id, process_agent, process_provider,
    process_provider_session_id, process_lease_state, process_outcome_json,
    process_provenance_json
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
    ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34
)";
const PROJECT_SESSION_COLUMNS: &str = "SELECT
    id, project_id, project_slug, project_name, project_prompt_context,
    wave_id, pm_snapshot_synced_at, status,
    status_reason, status_at, iteration, observation_cursor, last_state_fingerprint,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, created_at, updated_at,
    lf_bin, db_path, lf_home, abandon_requested_at, abandon_reason,
    process_group_id, process_agent, process_provider,
    process_provider_session_id, process_lease_state, process_outcome_json,
    process_provenance_json
    FROM project_sessions";
const PROJECT_SESSION_SELECT: &str = "SELECT
    id, project_id, project_slug, project_name, project_prompt_context,
    wave_id, pm_snapshot_synced_at, status,
    status_reason, status_at, iteration, observation_cursor, last_state_fingerprint,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, created_at, updated_at,
    lf_bin, db_path, lf_home, abandon_requested_at, abandon_reason,
    process_group_id, process_agent, process_provider,
    process_provider_session_id, process_lease_state, process_outcome_json,
    process_provenance_json
    FROM project_sessions WHERE id=?1";
const PROJECT_SESSION_UPDATE: &str = "UPDATE project_sessions SET
    project_id=?2, project_slug=?3, project_name=?4, project_prompt_context=?5,
    wave_id=?6, pm_snapshot_synced_at=?7, status=?8,
    status_reason=?9, status_at=?10, iteration=?11,
    observation_cursor=?12, last_state_fingerprint=?13, agent=?14, provider=?15,
    provider_session_id=?16, created_at=?21,
    updated_at=?22,
    lf_bin=?23, db_path=?24, lf_home=?25,
    abandon_requested_at=?26, abandon_reason=?27
    WHERE id=?1";
const PROJECT_SESSION_RUN_UPDATE: &str = "UPDATE project_sessions SET
    project_id=?2, project_slug=?3, project_name=?4, project_prompt_context=?5,
    wave_id=?6, pm_snapshot_synced_at=?7, status=?8,
    status_reason=?9, status_at=?10, iteration=?11,
    observation_cursor=?12, last_state_fingerprint=?13, agent=?14, provider=?15,
    provider_session_id=?16, process_generation=?17, process_pid=?18,
    process_tmux_name=?19, process_started_at=?20, created_at=?21,
    updated_at=?22,
    lf_bin=?23, db_path=?24, lf_home=?25,
    abandon_requested_at=?26, abandon_reason=?27,
    process_group_id=?28, process_agent=?29, process_provider=?30,
    process_provider_session_id=?31, process_lease_state=?32,
    process_outcome_json=?33, process_provenance_json=?34
    WHERE id=?1 AND (status NOT IN ('completed', 'abandoned') OR status=?8)";
fn project_session_params(session: &ProjectSession) -> Vec<Box<dyn ToSql>> {
    vec![
        Box::new(session.id.as_str().to_string()),
        Box::new(session.launch.project.id.as_str().to_string()),
        Box::new(session.launch.project.slug.clone()),
        Box::new(session.launch.project.name.clone()),
        Box::new(session.launch.project.prompt_context.clone()),
        Box::new(session.wave_id.clone()),
        Box::new(session.launch.pm_snapshot_synced_at),
        Box::new(session.status.as_str().to_string()),
        Box::new(session.status_reason.clone()),
        Box::new(session.status_at.unix_timestamp()),
        Box::new(i64::from(session.iteration)),
        Box::new(session.observation_cursor),
        Box::new(session.last_state_fingerprint.clone()),
        Box::new(session.agent.clone()),
        Box::new(session.provider.clone()),
        Box::new(session.provider_session_id.clone()),
        Box::new(
            session
                .latest_process
                .as_ref()
                .map(|process| i64::from(process.generation)),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .and_then(|process| process.pid.map(i64::from)),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .map(|process| process.tmux_name.clone()),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .map(|process| process.started_at.unix_timestamp()),
        ),
        Box::new(session.created_at.unix_timestamp()),
        Box::new(session.updated_at.unix_timestamp()),
        // Legacy lf_bin/db_path/lf_home columns: a Session no longer pins a
        // binary. Launch resolves the current Home lf; BinaryProvenance on the
        // generation is the audit record. Written NULL, never read; the columns
        // are dropped by the earned table rebuild, not this change.
        Box::new(None::<String>),
        Box::new(None::<String>),
        Box::new(None::<String>),
        Box::new(
            session
                .abandon_intent
                .as_ref()
                .map(|intent| intent.requested_at.unix_timestamp()),
        ),
        Box::new(
            session
                .abandon_intent
                .as_ref()
                .map(|intent| intent.reason.clone()),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .and_then(|process| process.process_group_id.map(i64::from)),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .map(|process| process.agent.clone()),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .map(|process| process.provider.clone()),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .and_then(|process| process.provider_session_id.clone()),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .map(|process| process.state.as_str().to_string()),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .and_then(|process| process.outcome.as_ref())
                .map(|outcome| {
                    serde_json::to_string(outcome).expect("child body outcome must serialize")
                }),
        ),
        Box::new(
            session
                .latest_process
                .as_ref()
                .and_then(|process| process.provenance.as_ref())
                .map(|provenance| {
                    serde_json::to_string(provenance).expect("child body provenance must serialize")
                }),
        ),
    ]
}

fn project_session_control_params(session: &ProjectSession) -> Vec<Box<dyn ToSql>> {
    let mut parameters = project_session_params(session);
    parameters.truncate(27);
    parameters
}

fn update_project_session_for_run_in(
    conn: &Connection,
    session: &ProjectSession,
    lease: &RunLease,
) -> StoreResult<usize> {
    require_run_owns_child(conn, &ChildRef::Project(session.id.clone()), lease)?;
    let parameters = project_session_params(session);
    Ok(conn.execute(
        PROJECT_SESSION_RUN_UPDATE,
        rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
    )?)
}

fn map_project_session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectSession> {
    let status_text: String = row.get(7)?;
    let status = status_text
        .parse()
        .map_err(|error| invalid_column(7, error))?;
    let process_generation: Option<i64> = row.get(16)?;
    let process_started_at: Option<i64> = row.get(19)?;
    let process = match (process_generation, process_started_at) {
        (Some(generation), Some(started_at)) => {
            let state_text: String = row.get(31)?;
            let outcome_json: Option<String> = row.get(32)?;
            let provenance_json: Option<String> = row.get(33)?;
            Some(ChildProcessGeneration {
                generation: generation as u32,
                pid: row.get::<_, Option<i64>>(17)?.map(|pid| pid as u32),
                process_group_id: row.get::<_, Option<i64>>(27)?.map(|id| id as u32),
                tmux_name: row.get::<_, Option<String>>(18)?.unwrap_or_default(),
                agent: row.get(28)?,
                provider: row.get(29)?,
                provider_session_id: row.get(30)?,
                started_at: crate::store::rows::unix_to_datetime(started_at),
                state: ChildLeaseState::parse(&state_text)
                    .map_err(|error| invalid_column(31, error))?,
                outcome: outcome_json
                    .map(|json| serde_json::from_str(&json))
                    .transpose()
                    .map_err(|error| invalid_column(32, error))?,
                provenance: provenance_json
                    .map(|json| serde_json::from_str(&json))
                    .transpose()
                    .map_err(|error| invalid_column(33, error))?,
            })
        }
        _ => None,
    };
    // Columns 22/23/24 (lf_bin/db_path/lf_home) are legacy dead schema: a
    // Session no longer pins a binary, so they are not read into domain state.
    let abandon_intent = match (
        row.get::<_, Option<i64>>(25)?,
        row.get::<_, Option<String>>(26)?,
    ) {
        (Some(requested_at), Some(reason)) => Some(AbandonIntent {
            requested_at: crate::store::rows::unix_to_datetime(requested_at),
            reason,
        }),
        _ => None,
    };
    Ok(ProjectSession {
        id: ProjectSessionId::from_raw(row.get::<_, String>(0)?),
        launch: crate::session_context::ProjectLaunchReceipt {
            project: LinearProjectSnapshot {
                id: LinearProjectId::from_raw(row.get::<_, String>(1)?),
                slug: row.get(2)?,
                name: row.get(3)?,
                prompt_context: row.get(4)?,
            },
            pm_snapshot_synced_at: row.get(6)?,
        },
        wave_id: row.get(5)?,
        status,
        status_reason: row.get(8)?,
        status_at: crate::store::rows::unix_to_datetime(row.get(9)?),
        iteration: row.get::<_, i64>(10)? as u32,
        observation_cursor: row.get(11)?,
        last_state_fingerprint: row.get(12)?,
        agent: row.get(13)?,
        provider: row.get(14)?,
        provider_session_id: row.get(15)?,
        latest_process: process,
        abandon_intent,
        created_at: crate::store::rows::unix_to_datetime(row.get(20)?),
        updated_at: crate::store::rows::unix_to_datetime(row.get(21)?),
    })
}

fn map_project_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectEvent> {
    let kind: ProjectEventKind = serde_json::from_str(&row.get::<_, String>(2)?)
        .map_err(|error| invalid_column(2, error))?;
    Ok(ProjectEvent {
        id: row.get(0)?,
        session_id: ProjectSessionId::from_raw(row.get::<_, String>(1)?),
        kind,
        created_at: crate::store::rows::unix_to_datetime(row.get(3)?),
    })
}

fn recipient_columns(recipient: &ObservationRecipient) -> (&'static str, String) {
    match recipient {
        ObservationRecipient::Wave { wave_id } => ("wave", wave_id.as_str().to_string()),
        ObservationRecipient::Project { session_id } => {
            ("project", session_id.as_str().to_string())
        }
    }
}

fn child_columns(source: &ChildRef) -> (&'static str, String) {
    match source {
        ChildRef::Project(session_id) => ("project", session_id.as_str().to_string()),
        ChildRef::Task(session_id) => ("task", session_id.as_str().to_string()),
    }
}

pub(super) fn insert_task_event_in(
    conn: &Connection,
    session: &TaskSession,
    kind: &TaskEventKind,
) -> StoreResult<TaskEvent> {
    let created_at = now_unix();
    conn.execute(
        "INSERT INTO task_events (session_id, kind_json, created_at) VALUES (?1, ?2, ?3)",
        params![
            session.id.as_str(),
            serde_json::to_string(kind)?,
            created_at
        ],
    )?;
    let event_id = conn.last_insert_rowid();
    if kind.is_project_observable() {
        insert_observation(
            conn,
            &ObservationRecipient::Project {
                session_id: session.project_session_id.clone(),
            },
            &ChildRef::Task(session.id.clone()),
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
                    wave_id: session.wave_id.clone(),
                },
                &ChildRef::Task(session.id.clone()),
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
        session_id: session.id.clone(),
        kind: kind.clone(),
        created_at: crate::store::rows::unix_to_datetime(created_at),
    })
}

fn insert_project_event_in(
    conn: &Connection,
    session: &ProjectSession,
    kind: &ProjectEventKind,
) -> StoreResult<ProjectEvent> {
    let created_at = now_unix();
    conn.execute(
        "INSERT INTO project_events (session_id, kind_json, created_at) VALUES (?1, ?2, ?3)",
        params![
            session.id.as_str(),
            serde_json::to_string(kind)?,
            created_at
        ],
    )?;
    let event_id = conn.last_insert_rowid();
    if kind.is_wave_observable() {
        insert_observation(
            conn,
            &ObservationRecipient::Wave {
                wave_id: session.wave_id.clone(),
            },
            &ChildRef::Project(session.id.clone()),
            event_id,
            &ChildEventPayload::Project {
                event: kind.clone(),
            },
            created_at,
        )?;
    }
    Ok(ProjectEvent {
        id: event_id,
        session_id: session.id.clone(),
        kind: kind.clone(),
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
            session_id: ProjectSessionId::from_raw(recipient_id),
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
        "project" => ChildRef::Project(ProjectSessionId::from_raw(source_id)),
        "task" => ChildRef::Task(TaskSessionId::from_raw(source_id)),
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
