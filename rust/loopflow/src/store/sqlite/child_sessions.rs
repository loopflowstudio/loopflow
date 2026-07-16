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
    AbandonIntent, BoundaryResult, ChildBodyHandoff, ChildBodyHandoffRequest, ChildBodyOutcome,
    ChildCommand, ChildCommandEffect, ChildCommandId, ChildCommandKind, ChildCommandSource,
    ChildCommandState, ChildDirective, ChildDirectiveId, ChildLeaseState, ChildLeaseToken,
    ChildProcessGeneration, ChildRef, ChildWriteLease, DirectiveKind, ObservationRecipient,
};
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

    pub fn reserve_task_session_with_directive(
        &self,
        session: &TaskSession,
        pr: &TaskPr,
        directive: &ChildDirective,
    ) -> StoreResult<()> {
        ensure_directive_target(directive, "task", session.id.as_str())?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        insert_initial_task(&transaction, session, pr)?;
        insert_child_directive(&transaction, directive)?;
        transaction.commit()?;
        Ok(())
    }

    /// Carry a terminal Task Session's direction onto a successor, in one
    /// transaction with the successor's own creation.
    ///
    /// Inserts the successor Session, its sequence-1 Working PR, and its initial
    /// directive; then transactionally re-keys the Linear observation cursor and
    /// the ingested-comment ledger from the predecessor onto the successor. The
    /// cursor is re-keyed (moved) rather than copied, so the successor resumes
    /// polling from the predecessor's last revision and a webhook redelivery of
    /// an already-applied edit cannot re-emit it; if the predecessor had no
    /// cursor row, the successor is seeded from its launch snapshot instead. The
    /// ledger re-key makes a comment already turned into a follow-up by the
    /// predecessor land zero times on the successor. Historical receipts
    /// (`child_commands`, `child_directives`) stay on the predecessor for
    /// attribution; the successor is soft-linked to it via
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
        directive: &ChildDirective,
    ) -> StoreResult<TaskSessionSuccession> {
        ensure_directive_target(directive, "task", successor.id.as_str())?;
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
        insert_task_pr(&transaction, pr)?;
        insert_child_directive(&transaction, directive)?;
        // Re-key the cursor: move the predecessor's single row onto the
        // successor. No successor cursor row exists yet (the successor was not
        // seeded), so the UPDATE cannot collide on the PRIMARY KEY.
        transaction.execute(
            "UPDATE task_linear_observations SET session_id=?2 WHERE session_id=?1",
            params![predecessor.id.as_str(), successor.id.as_str()],
        )?;
        // Seed-if-absent: a predecessor without a cursor (e.g. one predating
        // the cursor migration) leaves the successor with no row to move, so
        // seed from the launch snapshot. INSERT OR IGNORE is a no-op when the
        // re-key moved a row.
        seed_task_linear_observation(&transaction, successor)?;
        // Re-key the comment ledger so an already-delivered comment cannot
        // become a second follow-up on the successor.
        transaction.execute(
            "UPDATE task_linear_ingested_comments SET session_id=?2 WHERE session_id=?1",
            params![predecessor.id.as_str(), successor.id.as_str()],
        )?;
        // Soft-link the successor to its predecessor. Receipts stay on the
        // predecessor; this column is the attributable-history link.
        transaction.execute(
            "UPDATE task_sessions SET predecessor_session_id=?2 WHERE id=?1",
            params![successor.id.as_str(), predecessor.id.as_str()],
        )?;
        transaction.commit()?;
        Ok(TaskSessionSuccession {
            session: successor.clone(),
            created: true,
        })
    }

    pub fn update_task_session(&self, session: &TaskSession) -> StoreResult<()> {
        validate_task_session(session)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        validate_task_project_session(&conn, session)?;
        let parameters = task_session_control_params(session);
        let changed = conn.execute(
            TASK_SESSION_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn rebind_task_issue_identifier(
        &self,
        issue_id: &str,
        old_identifier: &str,
        new_identifier: &str,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // A team migration rebinds the *current* successor for an issue, not a
        // terminal predecessor kept as history. `issue_id` can now match several
        // rows (one live successor plus completed/abandoned predecessors), so
        // resolve the current one through the same rule as the operational
        // lookups rather than `query_row`, which errors on the second row.
        let query = format!("{TASK_SESSION_COLUMNS} WHERE issue_id=?1");
        let mut statement = conn.prepare(&query)?;
        let rows = statement.query_map(params![issue_id], map_task_session_row)?;
        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }
        let Some(current) = resolve_current_task_session(issue_id, sessions)? else {
            return Ok(false);
        };
        let session_id = current.id.as_str().to_string();
        let current_identifier = current.launch.issue.identifier.as_str();
        let status = current.status.as_str();
        let lease_state = current.latest_process.as_ref().map(|process| process.state);
        if current_identifier == new_identifier {
            return Ok(false);
        }
        if current_identifier != old_identifier {
            return Err(StoreError::InvalidData(format!(
                "Task Session {session_id} identifies issue {issue_id} as {current_identifier}, not {old_identifier}"
            )));
        }
        if matches!(status, "starting" | "running")
            || matches!(
                lease_state,
                Some(ChildLeaseState::Reserved) | Some(ChildLeaseState::Active)
            )
        {
            return Err(StoreError::InvalidData(format!(
                "Task Session {session_id} has an active body; stop it before changing {old_identifier} to {new_identifier}"
            )));
        }
        let changed = conn.execute(
            "UPDATE task_sessions
             SET issue_identifier=?3, updated_at=?4
             WHERE id=?1 AND issue_identifier=?2
               AND status NOT IN ('starting', 'running')
               AND COALESCE(process_lease_state, 'finished') NOT IN ('reserved', 'active')",
            params![session_id, old_identifier, new_identifier, now_unix()],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidData(format!(
                "Task Session for {old_identifier} became active during its team migration"
            )));
        }
        Ok(true)
    }

    pub(crate) fn activate_task_process(
        &self,
        session: &TaskSession,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        validate_task_session(session)?;
        let process = session.latest_process.as_ref().ok_or_else(|| {
            StoreError::InvalidData("Task activation requires a process generation".to_string())
        })?;
        if process.state != ChildLeaseState::Active || process.generation != lease.generation {
            return Err(StoreError::InvalidData(
                "Task activation requires the matching Active generation".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = update_task_session_for_lease_in(
            &transaction,
            session,
            lease,
            ChildLeaseState::Reserved,
        )?;
        if changed == 0 {
            return Err(lease_revoked("Task Session", session.id.as_str(), lease));
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

    pub(crate) fn update_task_session_for_lease(
        &self,
        session: &TaskSession,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        validate_task_session(session)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        if update_task_session_for_lease_in(&conn, session, lease, ChildLeaseState::Active)? == 0 {
            return Err(lease_revoked("Task Session", session.id.as_str(), lease));
        }
        Ok(())
    }

    pub(crate) fn finish_task_process(
        &self,
        session: &TaskSession,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        validate_task_session(session)?;
        let process = session.latest_process.as_ref().ok_or_else(|| {
            StoreError::InvalidData("Task finish requires a process generation".to_string())
        })?;
        if process.state != ChildLeaseState::Finished
            || process.outcome.is_none()
            || process.generation != lease.generation
        {
            return Err(StoreError::InvalidData(
                "Task finish requires the matching Finished generation and outcome".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if update_task_session_for_lease_in(&transaction, session, lease, ChildLeaseState::Active)?
            == 0
        {
            return Err(lease_revoked("Task Session", session.id.as_str(), lease));
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
        transaction.commit()?;
        Ok(process)
    }

    pub fn complete_task_session(
        &self,
        session: &TaskSession,
        skipped_pr: Option<&TaskPr>,
    ) -> StoreResult<()> {
        self.complete_task_session_with_lease(session, skipped_pr, None)
    }

    pub(crate) fn complete_task_session_for_lease(
        &self,
        session: &TaskSession,
        skipped_pr: Option<&TaskPr>,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        self.complete_task_session_with_lease(session, skipped_pr, Some(lease))
    }

    fn complete_task_session_with_lease(
        &self,
        session: &TaskSession,
        skipped_pr: Option<&TaskPr>,
        lease: Option<&ChildWriteLease>,
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
        if let Some(lease) = lease {
            require_child_write_lease(&transaction, &ChildRef::Task(session.id.clone()), lease)?;
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
        let changed = match lease {
            Some(lease) => update_task_session_for_lease_in(
                &transaction,
                session,
                lease,
                ChildLeaseState::Active,
            )?,
            None => {
                let parameters = task_session_control_params(session);
                transaction.execute(
                    TASK_SESSION_UPDATE,
                    rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
                )?
            }
        };
        if changed == 0 {
            if let Some(lease) = lease {
                return Err(lease_revoked("Task Session", session.id.as_str(), lease));
            }
            return Err(StoreError::NotFound);
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn reserve_task_process(
        &self,
        session: &TaskSession,
        expected_status: TaskSessionStatus,
    ) -> StoreResult<Option<ChildWriteLease>> {
        validate_task_session(session)?;
        let process = session.latest_process.as_ref().ok_or_else(|| {
            StoreError::InvalidData("Task process reservation requires a generation".to_string())
        })?;
        let previous_generation = process.generation.checked_sub(1).ok_or_else(|| {
            StoreError::InvalidData("Task process generation must start at one".to_string())
        })?;
        let token = ChildLeaseToken::new();
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE task_sessions SET
                status = ?2, status_reason = ?3, status_at = ?4,
                process_generation = ?5, process_pid = ?6,
                process_tmux_name = ?7, process_started_at = ?8,
                updated_at = ?9, process_lease_token = ?11,
                process_group_id = ?12, process_agent = ?13,
                process_provider = ?14, process_provider_session_id = ?15,
                process_lease_state = 'reserved', process_outcome_json = NULL,
                process_provenance_json = ?17
             WHERE id = ?1 AND status = ?10
               AND COALESCE(process_generation, 0) = ?16
               AND (process_lease_state IS NULL OR process_lease_state = 'finished')",
            params![
                session.id.as_str(),
                session.status.as_str(),
                session.status_reason,
                session.status_at.unix_timestamp(),
                i64::from(process.generation),
                process.pid.map(i64::from),
                process.tmux_name,
                process.started_at.unix_timestamp(),
                session.updated_at.unix_timestamp(),
                expected_status.as_str(),
                token.as_str(),
                process.process_group_id.map(i64::from),
                process.agent,
                process.provider,
                process.provider_session_id,
                i64::from(previous_generation),
                process
                    .provenance
                    .as_ref()
                    .map(|provenance| serde_json::to_string(provenance)
                        .expect("child body provenance must serialize")),
            ],
        )?;
        if changed == 0 {
            return Ok(None);
        }
        insert_task_event_in(
            &transaction,
            session,
            &TaskEventKind::BodyLeaseChanged {
                process: process.clone(),
            },
        )?;
        insert_task_event_in(
            &transaction,
            session,
            &TaskEventKind::StatusChanged {
                from: expected_status,
                to: session.status,
                reason: session.status_reason.clone(),
            },
        )?;
        transaction.commit()?;
        Ok(Some(ChildWriteLease {
            generation: process.generation,
            token,
        }))
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

    pub(crate) fn update_task_pr_for_lease(
        &self,
        pr: &TaskPr,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        validate_task_pr(pr)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(
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

    pub(crate) fn heal_task_pr_base_for_lease(
        &self,
        pr: &TaskPr,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(
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

    pub(crate) fn settle_task_pr_for_lease(
        &self,
        settled: &TaskPr,
        next: Option<&TaskPr>,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        validate_task_pr_settlement(settled, next)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(
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

    pub(crate) fn complete_task_session_after_pr_for_lease(
        &self,
        session: &TaskSession,
        pr: &TaskPr,
        lease: &ChildWriteLease,
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
        if update_task_session_for_lease_in(&transaction, session, lease, ChildLeaseState::Active)?
            == 0
        {
            return Err(lease_revoked("Task Session", session.id.as_str(), lease));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn insert_child_command(&self, command: &ChildCommand) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        insert_child_command(&conn, command)
    }

    /// Queue an Abandon command and stamp the Session's abandon intent in the
    /// same transaction.
    ///
    /// These two writes must not be separable. The intent is what every launch
    /// path reads to refuse a restart; if the command could land without it, the
    /// window between "abandon queued" and "runner consumed it" would still be a
    /// window in which a supervisor revives the Session.
    pub fn insert_child_abandon_command(
        &self,
        command: &ChildCommand,
        intent: &AbandonIntent,
    ) -> StoreResult<()> {
        let ChildCommandKind::Abandon { .. } = &command.kind else {
            return Err(StoreError::InvalidData(
                "abandon command required".to_string(),
            ));
        };
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        insert_child_command(&transaction, command)?;
        let table = match command.target {
            ChildRef::Project(_) => "project_sessions",
            ChildRef::Task(_) => "task_sessions",
        };
        let updated = transaction.execute(
            &format!(
                "UPDATE {table} SET abandon_requested_at=?2, abandon_reason=?3, updated_at=?4
                 WHERE id=?1"
            ),
            params![
                command.target.target_id(),
                intent.requested_at.unix_timestamp(),
                intent.reason,
                now_unix(),
            ],
        )?;
        if updated == 0 {
            return Err(StoreError::InvalidData(format!(
                "{} {} not found",
                command.target.target_kind(),
                command.target.target_id()
            )));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn ensure_child_decision_command(
        &self,
        command: &ChildCommand,
    ) -> StoreResult<(ChildCommand, bool)> {
        let ChildCommandKind::Decide { decision_id, .. } = &command.kind else {
            return Err(StoreError::InvalidData(
                "decision command required".to_string(),
            ));
        };
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = {
            let mut statement = transaction.prepare(&format!(
                "{CHILD_COMMAND_COLUMNS}
                 WHERE target_kind=?1 AND session_id=?2
                 ORDER BY created_at, id"
            ))?;
            let rows = statement.query_map(
                params![command.target.target_kind(), command.target.target_id()],
                map_child_command_row,
            )?;
            let mut existing = None;
            for row in rows {
                let candidate = row?;
                if matches!(
                    &candidate.kind,
                    ChildCommandKind::Decide {
                        decision_id: candidate_id,
                        ..
                    } if candidate_id == decision_id
                ) {
                    existing = Some(candidate);
                    break;
                }
            }
            existing
        };
        if let Some(existing) = existing {
            return Ok((existing, false));
        }
        insert_child_command(&transaction, command)?;
        transaction.commit()?;
        Ok((command.clone(), true))
    }

    /// Insert a `ci-fix` wake unless one already exists for the same incident
    /// identity, in which case return the existing row and `false`.
    ///
    /// This is the whole dedup: one (repo, PR, failed head, failure set) wakes
    /// exactly one body, and the fact is a durable row written before any process
    /// starts rather than a marker stamped once one has. A repeated observation,
    /// a coalesced multi-check delivery, and a restart between observation and
    /// arming all land here and all find the same command.
    ///
    /// Terminal state is *not* consulted: an identity that already woke a body
    /// and accepted must not wake a second one. Re-arming is the job of a new
    /// identity — a moved head or a changed failure set.
    pub fn ensure_child_ci_fix_command(
        &self,
        command: &ChildCommand,
    ) -> StoreResult<(ChildCommand, bool)> {
        let ChildCommandKind::CiFix {
            incident_identity, ..
        } = &command.kind
        else {
            return Err(StoreError::InvalidData(
                "ci-fix command required".to_string(),
            ));
        };
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = {
            let mut statement = transaction.prepare(&format!(
                "{CHILD_COMMAND_COLUMNS}
                 WHERE target_kind=?1 AND session_id=?2
                 ORDER BY created_at, id"
            ))?;
            let rows = statement.query_map(
                params![command.target.target_kind(), command.target.target_id()],
                map_child_command_row,
            )?;
            let mut existing = None;
            for row in rows {
                let candidate = row?;
                if matches!(
                    &candidate.kind,
                    ChildCommandKind::CiFix {
                        incident_identity: candidate_identity,
                        ..
                    } if candidate_identity == incident_identity
                ) {
                    existing = Some(candidate);
                    break;
                }
            }
            existing
        };
        if let Some(existing) = existing {
            return Ok((existing, false));
        }
        insert_child_command(&transaction, command)?;
        transaction.commit()?;
        Ok((command.clone(), true))
    }

    pub fn supersede_and_insert_child_command(
        &self,
        command: &ChildCommand,
    ) -> StoreResult<Vec<ChildCommandId>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let superseded = supersede_child_commands(
            &transaction,
            command.target.target_kind(),
            command.target.target_id(),
        )?;
        insert_child_command(&transaction, command)?;
        transaction.commit()?;
        Ok(superseded)
    }

    pub fn insert_child_command_with_directive(
        &self,
        command: &ChildCommand,
        directive: &ChildDirective,
    ) -> StoreResult<Vec<ChildCommandId>> {
        ensure_directive_target(
            directive,
            command.target.target_kind(),
            command.target.target_id(),
        )?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let superseded = supersede_child_commands(
            &transaction,
            command.target.target_kind(),
            command.target.target_id(),
        )?;
        insert_child_command(&transaction, command)?;
        insert_child_directive(&transaction, directive)?;
        let table = match command.target {
            ChildRef::Project(_) => "project_sessions",
            ChildRef::Task(_) => "task_sessions",
        };
        transaction.execute(
            &format!("UPDATE {table} SET current_directive_version=?2, updated_at=?3 WHERE id=?1"),
            params![
                command.target.target_id(),
                i64::from(directive.version),
                OffsetDateTime::now_utc().unix_timestamp(),
            ],
        )?;
        transaction.commit()?;
        Ok(superseded)
    }

    pub fn child_command(&self, command_id: &ChildCommandId) -> StoreResult<Option<ChildCommand>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &format!("{CHILD_COMMAND_COLUMNS} WHERE id=?1"),
            params![command_id.as_str()],
            map_child_command_row,
        )
        .optional()
        .map_err(StoreError::from)
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
    /// becomes a directive only if the stored content still differs; and a
    /// comment becomes a follow-up only on its first entry into the ledger.
    pub fn apply_linear_observation(
        &self,
        apply: &LinearObservationApply,
    ) -> StoreResult<LinearObservationOutcome> {
        if let Some((_, directive)) = &apply.directive {
            ensure_directive_target(directive, "task", apply.session_id.as_str())?;
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

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
                directive_applied: false,
                superseded: Vec::new(),
                follow_ups_created: Vec::new(),
            });
        };

        // Monotonic guard: an out-of-order response older than what we have
        // carries stale content, so drop it rather than let it revert direction.
        if apply.revision.as_str() < last_revision.as_str() {
            transaction.commit()?;
            return Ok(LinearObservationOutcome {
                baselined: false,
                directive_applied: false,
                superseded: Vec::new(),
                follow_ups_created: Vec::new(),
            });
        }

        // Title/description edit → replacement directive, but only if the stored
        // content still differs (a racing observer may have applied it already).
        let mut directive_applied = false;
        let mut superseded = Vec::new();
        if let Some((command, directive)) = &apply.directive {
            if last_title != apply.title || last_description != apply.description {
                // The version is stamped here, inside the transaction, from the
                // authoritative `current_directive_version` — not from the
                // caller's earlier session read. A concurrent webhook or a racing
                // `lf task steer` that bumped the version in the gap would
                // otherwise collide on UNIQUE(target, version) and fail the whole
                // apply.
                let current: i64 = transaction.query_row(
                    "SELECT current_directive_version FROM task_sessions WHERE id=?1",
                    params![apply.session_id.as_str()],
                    |row| row.get(0),
                )?;
                let next = current + 1;
                let mut directive = directive.clone();
                directive.version = next as u32;
                superseded =
                    supersede_child_commands(&transaction, "task", apply.session_id.as_str())?;
                insert_child_command(&transaction, command)?;
                insert_child_directive(&transaction, &directive)?;
                transaction.execute(
                    "UPDATE task_sessions SET current_directive_version=?2, updated_at=?3 WHERE id=?1",
                    params![apply.session_id.as_str(), next, observed_at],
                )?;
                directive_applied = true;
            }
        }

        // Each new human comment → one FIFO follow-up, guarded by the ledger.
        let mut follow_ups_created = Vec::new();
        for follow_up in &apply.follow_ups {
            if let Some(id) = ingest_linear_comment(
                &transaction,
                apply.session_id.as_str(),
                &follow_up.comment_id,
                &follow_up.command,
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
            directive_applied,
            superseded,
            follow_ups_created,
        })
    }

    /// Persist one human Linear comment as a FIFO Task follow-up, exactly once.
    /// Webhook comments arrive one at a time (unlike the snapshot edit path), and
    /// Linear delivers at-least-once — so the `task_linear_ingested_comments`
    /// ledger is the guard: the follow-up command is created only on the comment
    /// id's first insertion. Returns the created command id, or `None` for a
    /// duplicate delivery.
    pub fn apply_linear_comment(
        &self,
        session_id: &TaskSessionId,
        comment_id: &str,
        command: &ChildCommand,
        observed_at: OffsetDateTime,
    ) -> StoreResult<Option<ChildCommandId>> {
        let observed_at = observed_at.unix_timestamp();
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let created = ingest_linear_comment(
            &transaction,
            session_id.as_str(),
            comment_id,
            command,
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

    pub fn child_commands(&self, target: &ChildRef) -> StoreResult<Vec<ChildCommand>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(&format!(
            "{CHILD_COMMAND_COLUMNS}
             WHERE target_kind=?1 AND session_id=?2
             ORDER BY created_at, id"
        ))?;
        let rows = statement.query_map(
            params![target.target_kind(), target.target_id()],
            map_child_command_row,
        )?;
        let mut commands = Vec::new();
        for row in rows {
            commands.push(row?);
        }
        Ok(commands)
    }

    pub fn claim_child_commands(
        &self,
        target: &ChildRef,
        generation: u32,
    ) -> StoreResult<Vec<ChildCommand>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        claim_child_commands_in(&transaction, target, generation)?;
        let commands = read_claimed_child_commands(&transaction, target, generation)?;
        transaction.commit()?;
        Ok(commands)
    }

    pub(crate) fn claim_child_commands_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
    ) -> StoreResult<Vec<ChildCommand>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(&transaction, target, lease)?;
        claim_child_commands_in(&transaction, target, lease.generation)?;
        let commands = read_claimed_child_commands(&transaction, target, lease.generation)?;
        transaction.commit()?;
        Ok(commands)
    }

    pub fn claim_task_commands_or_stop(
        &self,
        session_id: &TaskSessionId,
        generation: u32,
        stopped_status: TaskSessionStatus,
        reason: &str,
    ) -> StoreResult<BoundaryResult<TaskSession>> {
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
        if session
            .latest_process
            .as_ref()
            .map(|process| process.generation)
            != Some(generation)
            || !session.status.is_process_active()
        {
            return Err(StoreError::InvalidData(format!(
                "Task Session {session_id} generation {generation} is not active"
            )));
        }
        let target = ChildRef::Task(session_id.clone());
        claim_child_commands_in(&transaction, &target, generation)?;
        let commands = read_claimed_child_commands(&transaction, &target, generation)?;
        if !commands.is_empty() {
            transaction.commit()?;
            return Ok(BoundaryResult::Commands(commands));
        }

        session.set_status(stopped_status, reason);
        let parameters = task_session_control_params(&session);
        let changed = transaction.execute(
            TASK_SESSION_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        transaction.commit()?;
        Ok(BoundaryResult::Stopped(session))
    }

    pub(crate) fn claim_task_commands_or_stop_for_lease(
        &self,
        session_id: &TaskSessionId,
        lease: &ChildWriteLease,
        stopped_status: TaskSessionStatus,
        reason: &str,
    ) -> StoreResult<BoundaryResult<TaskSession>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let target = ChildRef::Task(session_id.clone());
        require_child_write_lease(&transaction, &target, lease)?;
        let mut session = transaction
            .query_row(
                TASK_SESSION_SELECT,
                params![session_id.as_str()],
                map_task_session_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        claim_child_commands_in(&transaction, &target, lease.generation)?;
        let commands = read_claimed_child_commands(&transaction, &target, lease.generation)?;
        if !commands.is_empty() {
            transaction.commit()?;
            return Ok(BoundaryResult::Commands(commands));
        }
        session.set_status(stopped_status, reason);
        if update_task_session_for_lease_in(&transaction, &session, lease, ChildLeaseState::Active)?
            == 0
        {
            return Err(lease_revoked("Task Session", session_id.as_str(), lease));
        }
        transaction.commit()?;
        Ok(BoundaryResult::Stopped(session))
    }

    pub fn accept_child_command(
        &self,
        command_id: &ChildCommandId,
        effect: Option<ChildCommandEffect>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let accepted_at = time::OffsetDateTime::now_utc().unix_timestamp_nanos() as i64;
        let changed = transaction.execute(
            "UPDATE child_commands
             SET state = 'accepted', effect = ?1, accepted_at = ?2, error = NULL
             WHERE id = ?3 AND state IN ('persisted', 'claimed', 'delivering')",
            params![
                effect.map(ChildCommandEffect::as_str),
                accepted_at,
                command_id.as_str()
            ],
        )?;
        if changed == 0 {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM child_commands
                    WHERE id = ?1
                 )",
                params![command_id.as_str()],
                |row| row.get(0),
            )?;
            return if exists {
                Err(StoreError::InvalidData(format!(
                    "child command {command_id} is already resolved"
                )))
            } else {
                Err(StoreError::NotFound)
            };
        }
        transaction.execute(
            "UPDATE child_directives SET applied_at=COALESCE(applied_at, ?1)
             WHERE command_id=?2",
            params![accepted_at, command_id.as_str()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn accept_child_command_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
        command_id: &ChildCommandId,
        effect: Option<ChildCommandEffect>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(&transaction, target, lease)?;
        let accepted_at = time::OffsetDateTime::now_utc().unix_timestamp_nanos() as i64;
        let changed = transaction.execute(
            "UPDATE child_commands
             SET state = 'accepted', effect = ?1, accepted_at = ?2, error = NULL
             WHERE id = ?3 AND target_kind=?4 AND session_id=?5
               AND state IN ('persisted', 'claimed', 'delivering')",
            params![
                effect.map(ChildCommandEffect::as_str),
                accepted_at,
                command_id.as_str(),
                target.target_kind(),
                target.target_id()
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidData(format!(
                "child command {command_id} is already resolved or belongs to another Session"
            )));
        }
        transaction.execute(
            "UPDATE child_directives SET applied_at=COALESCE(applied_at, ?1)
             WHERE command_id=?2",
            params![accepted_at, command_id.as_str()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn mark_child_command_delivering(
        &self,
        command_id: &ChildCommandId,
        effect: ChildCommandEffect,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE child_commands SET state='delivering', effect=?1
             WHERE id=?2 AND state='claimed'",
            params![effect.as_str(), command_id.as_str()],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidData(format!(
                "child command {command_id} is not claimed"
            )));
        }
        Ok(())
    }

    pub(crate) fn mark_child_command_delivering_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
        command_id: &ChildCommandId,
        effect: ChildCommandEffect,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(&transaction, target, lease)?;
        let changed = transaction.execute(
            "UPDATE child_commands SET state='delivering', effect=?1
             WHERE id=?2 AND target_kind=?3 AND session_id=?4 AND state='claimed'",
            params![
                effect.as_str(),
                command_id.as_str(),
                target.target_kind(),
                target.target_id()
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidData(format!(
                "child command {command_id} is not claimed by this Session"
            )));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn mark_stale_child_deliveries_uncertain(
        &self,
        target: &ChildRef,
        generation: u32,
    ) -> StoreResult<Vec<ChildCommand>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut commands = {
            let mut statement = transaction.prepare(&format!(
                "{CHILD_COMMAND_COLUMNS}
                 WHERE target_kind=?1 AND session_id=?2 AND state='delivering'
                   AND claimed_by_generation<>?3
                 ORDER BY created_at, id"
            ))?;
            let rows = statement.query_map(
                params![
                    target.target_kind(),
                    target.target_id(),
                    i64::from(generation)
                ],
                map_child_command_row,
            )?;
            let mut commands = Vec::new();
            for row in rows {
                commands.push(row?);
            }
            commands
        };
        if commands.is_empty() {
            transaction.commit()?;
            return Ok(commands);
        }
        let error = "provider delivery outcome is unknown after process restart; inspect the child transcript before retrying";
        transaction.execute(
            "UPDATE child_commands
             SET state='uncertain',
                 error=?4
             WHERE target_kind=?1 AND session_id=?2 AND state='delivering'
               AND claimed_by_generation<>?3",
            params![
                target.target_kind(),
                target.target_id(),
                i64::from(generation),
                error
            ],
        )?;
        for command in &mut commands {
            command.state = ChildCommandState::Uncertain;
            command.error = Some(error.to_string());
        }
        transaction.commit()?;
        Ok(commands)
    }

    pub(crate) fn mark_stale_child_deliveries_uncertain_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
    ) -> StoreResult<Vec<ChildCommand>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(&transaction, target, lease)?;
        let mut commands = {
            let mut statement = transaction.prepare(&format!(
                "{CHILD_COMMAND_COLUMNS}
                 WHERE target_kind=?1 AND session_id=?2 AND state='delivering'
                   AND claimed_by_generation<>?3
                 ORDER BY created_at, id"
            ))?;
            let rows = statement.query_map(
                params![
                    target.target_kind(),
                    target.target_id(),
                    i64::from(lease.generation)
                ],
                map_child_command_row,
            )?;
            let mut commands = Vec::new();
            for row in rows {
                commands.push(row?);
            }
            commands
        };
        let error = "provider delivery outcome is unknown after process restart; inspect the child transcript before retrying";
        if !commands.is_empty() {
            transaction.execute(
                "UPDATE child_commands SET state='uncertain', error=?4
                 WHERE target_kind=?1 AND session_id=?2 AND state='delivering'
                   AND claimed_by_generation<>?3",
                params![
                    target.target_kind(),
                    target.target_id(),
                    i64::from(lease.generation),
                    error
                ],
            )?;
            for command in &mut commands {
                command.state = ChildCommandState::Uncertain;
                command.error = Some(error.to_string());
            }
        }
        transaction.commit()?;
        Ok(commands)
    }

    pub fn set_child_command_effect(
        &self,
        command_id: &ChildCommandId,
        effect: ChildCommandEffect,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE child_commands SET effect = ?1
             WHERE id = ?2 AND state = 'claimed'",
            params![effect.as_str(), command_id.as_str()],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidData(format!(
                "child command {command_id} is not claimed"
            )));
        }
        Ok(())
    }

    pub fn fail_child_command(
        &self,
        command_id: &ChildCommandId,
        effect: Option<ChildCommandEffect>,
        error: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE child_commands
             SET state = 'failed', effect = COALESCE(?1, effect), error = ?2
             WHERE id = ?3 AND state IN ('claimed', 'delivering')",
            params![
                effect.map(ChildCommandEffect::as_str),
                error,
                command_id.as_str()
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidData(format!(
                "child command {command_id} is already resolved"
            )));
        }
        Ok(())
    }

    pub(crate) fn fail_child_command_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
        command_id: &ChildCommandId,
        effect: Option<ChildCommandEffect>,
        error: &str,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(&transaction, target, lease)?;
        let changed = transaction.execute(
            "UPDATE child_commands
             SET state='failed', effect=COALESCE(?1, effect), error=?2
             WHERE id=?3 AND target_kind=?4 AND session_id=?5
               AND state IN ('claimed', 'delivering')",
            params![
                effect.map(ChildCommandEffect::as_str),
                error,
                command_id.as_str(),
                target.target_kind(),
                target.target_id()
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidData(format!(
                "child command {command_id} is already resolved or belongs to another Session"
            )));
        }
        transaction.commit()?;
        Ok(())
    }

    /// Displace one claimed command that circumstances made moot.
    ///
    /// Distinct from `fail_child_command_for_lease`: nothing went wrong, so the
    /// `error` column stays null and `lf task status` does not report a fault.
    /// The reason rides the event instead.
    pub(crate) fn supersede_child_command_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
        command_id: &ChildCommandId,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(&transaction, target, lease)?;
        let changed = transaction.execute(
            "UPDATE child_commands
             SET state='superseded'
             WHERE id=?1 AND target_kind=?2 AND session_id=?3
               AND state IN ('persisted', 'claimed', 'delivering')",
            params![
                command_id.as_str(),
                target.target_kind(),
                target.target_id()
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidData(format!(
                "child command {command_id} is already resolved or belongs to another Session"
            )));
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn validate_child_write_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        require_child_write_lease(&conn, target, lease)
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

    pub(crate) fn append_task_event_for_lease(
        &self,
        session_id: &TaskSessionId,
        lease: &ChildWriteLease,
        kind: &TaskEventKind,
    ) -> StoreResult<TaskEvent> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(&transaction, &ChildRef::Task(session_id.clone()), lease)?;
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
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            PROJECT_SESSION_INSERT,
            rusqlite::params_from_iter(
                project_session_params(session)
                    .iter()
                    .map(|value| value.as_ref()),
            ),
        )?;
        Ok(())
    }

    pub fn insert_project_session_with_directive(
        &self,
        session: &ProjectSession,
        directive: &ChildDirective,
    ) -> StoreResult<()> {
        validate_project_session(session)?;
        ensure_directive_target(directive, "project", session.id.as_str())?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let parameters = project_session_params(session);
        transaction.execute(
            PROJECT_SESSION_INSERT,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        insert_child_directive(&transaction, directive)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn update_project_session(&self, session: &ProjectSession) -> StoreResult<()> {
        validate_project_session(session)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        let parameters = project_session_control_params(session);
        let changed = conn.execute(
            PROJECT_SESSION_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub(crate) fn activate_project_process(
        &self,
        session: &ProjectSession,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        validate_project_session(session)?;
        let process = session.latest_process.as_ref().ok_or_else(|| {
            StoreError::InvalidData("Project activation requires a process generation".to_string())
        })?;
        if process.state != ChildLeaseState::Active || process.generation != lease.generation {
            return Err(StoreError::InvalidData(
                "Project activation requires the matching Active generation".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = update_project_session_for_lease_in(
            &transaction,
            session,
            lease,
            ChildLeaseState::Reserved,
        )?;
        if changed == 0 {
            return Err(lease_revoked("Project Session", session.id.as_str(), lease));
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

    pub(crate) fn update_project_session_for_lease(
        &self,
        session: &ProjectSession,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        validate_project_session(session)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        if update_project_session_for_lease_in(&conn, session, lease, ChildLeaseState::Active)? == 0
        {
            return Err(lease_revoked("Project Session", session.id.as_str(), lease));
        }
        Ok(())
    }

    pub(crate) fn finish_project_process(
        &self,
        session: &ProjectSession,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        validate_project_session(session)?;
        let process = session.latest_process.as_ref().ok_or_else(|| {
            StoreError::InvalidData("Project finish requires a process generation".to_string())
        })?;
        if process.state != ChildLeaseState::Finished
            || process.outcome.is_none()
            || process.generation != lease.generation
        {
            return Err(StoreError::InvalidData(
                "Project finish requires the matching Finished generation and outcome".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if update_project_session_for_lease_in(
            &transaction,
            session,
            lease,
            ChildLeaseState::Active,
        )? == 0
        {
            return Err(lease_revoked("Project Session", session.id.as_str(), lease));
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
        transaction.commit()?;
        Ok(process)
    }

    pub(crate) fn reserve_project_process(
        &self,
        session: &ProjectSession,
        expected_status: ProjectSessionStatus,
    ) -> StoreResult<Option<ChildWriteLease>> {
        validate_project_session(session)?;
        let process = session.latest_process.as_ref().ok_or_else(|| {
            StoreError::InvalidData("Project process reservation requires a generation".to_string())
        })?;
        let previous_generation = process.generation.checked_sub(1).ok_or_else(|| {
            StoreError::InvalidData("Project process generation must start at one".to_string())
        })?;
        let token = ChildLeaseToken::new();
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE project_sessions SET
                status=?2, status_reason=?3, status_at=?4,
                process_generation=?5, process_pid=?6, process_tmux_name=?7,
                process_started_at=?8, updated_at=?9,
                process_lease_token=?11, process_group_id=?12,
                process_agent=?13, process_provider=?14,
                process_provider_session_id=?15, process_lease_state='reserved',
                process_outcome_json=NULL, process_provenance_json=?17
             WHERE id=?1 AND status=?10
               AND COALESCE(process_generation, 0)=?16
               AND (process_lease_state IS NULL OR process_lease_state = 'finished')",
            params![
                session.id.as_str(),
                session.status.as_str(),
                session.status_reason,
                session.status_at.unix_timestamp(),
                i64::from(process.generation),
                process.pid.map(i64::from),
                process.tmux_name,
                process.started_at.unix_timestamp(),
                session.updated_at.unix_timestamp(),
                expected_status.as_str(),
                token.as_str(),
                process.process_group_id.map(i64::from),
                process.agent,
                process.provider,
                process.provider_session_id,
                i64::from(previous_generation),
                process
                    .provenance
                    .as_ref()
                    .map(|provenance| serde_json::to_string(provenance)
                        .expect("child body provenance must serialize")),
            ],
        )?;
        if changed == 0 {
            return Ok(None);
        }
        insert_project_event_in(
            &transaction,
            session,
            &ProjectEventKind::BodyLeaseChanged {
                process: process.clone(),
            },
        )?;
        insert_project_event_in(
            &transaction,
            session,
            &ProjectEventKind::StatusChanged {
                from: expected_status,
                to: session.status,
                reason: session.status_reason.clone(),
            },
        )?;
        transaction.commit()?;
        Ok(Some(ChildWriteLease {
            generation: process.generation,
            token,
        }))
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

    pub fn claim_project_commands_or_stop(
        &self,
        session_id: &ProjectSessionId,
        generation: u32,
        stopped_status: ProjectSessionStatus,
        reason: &str,
    ) -> StoreResult<BoundaryResult<ProjectSession>> {
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
        if session
            .latest_process
            .as_ref()
            .map(|process| process.generation)
            != Some(generation)
            || !session.status.is_process_active()
        {
            return Err(StoreError::InvalidData(format!(
                "Project Session {session_id} generation {generation} is not active"
            )));
        }
        let target = ChildRef::Project(session_id.clone());
        claim_child_commands_in(&transaction, &target, generation)?;
        let commands = read_claimed_child_commands(&transaction, &target, generation)?;
        if !commands.is_empty() {
            transaction.commit()?;
            return Ok(BoundaryResult::Commands(commands));
        }
        session.set_status(stopped_status, reason);
        let parameters = project_session_control_params(&session);
        transaction.execute(
            PROJECT_SESSION_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        transaction.commit()?;
        Ok(BoundaryResult::Stopped(session))
    }

    pub(crate) fn claim_project_commands_or_stop_for_lease(
        &self,
        session_id: &ProjectSessionId,
        lease: &ChildWriteLease,
        stopped_status: ProjectSessionStatus,
        reason: &str,
    ) -> StoreResult<BoundaryResult<ProjectSession>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let target = ChildRef::Project(session_id.clone());
        require_child_write_lease(&transaction, &target, lease)?;
        let mut session = transaction
            .query_row(
                PROJECT_SESSION_SELECT,
                params![session_id.as_str()],
                map_project_session_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        claim_child_commands_in(&transaction, &target, lease.generation)?;
        let commands = read_claimed_child_commands(&transaction, &target, lease.generation)?;
        if !commands.is_empty() {
            transaction.commit()?;
            return Ok(BoundaryResult::Commands(commands));
        }
        session.set_status(stopped_status, reason);
        if update_project_session_for_lease_in(
            &transaction,
            &session,
            lease,
            ChildLeaseState::Active,
        )? == 0
        {
            return Err(lease_revoked("Project Session", session_id.as_str(), lease));
        }
        transaction.commit()?;
        Ok(BoundaryResult::Stopped(session))
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

    pub(crate) fn append_project_event_for_lease(
        &self,
        session_id: &ProjectSessionId,
        lease: &ChildWriteLease,
        kind: &ProjectEventKind,
    ) -> StoreResult<ProjectEvent> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(&transaction, &ChildRef::Project(session_id.clone()), lease)?;
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
        self.consume_task_observation_for_project_with_lease(project_session_id, observation, None)
    }

    pub(crate) fn consume_task_observation_for_project_for_lease(
        &self,
        project_session_id: &ProjectSessionId,
        observation: &ObservationOutboxRow,
        lease: &ChildWriteLease,
    ) -> StoreResult<bool> {
        self.consume_task_observation_for_project_with_lease(
            project_session_id,
            observation,
            Some(lease),
        )
    }

    fn consume_task_observation_for_project_with_lease(
        &self,
        project_session_id: &ProjectSessionId,
        observation: &ObservationOutboxRow,
        lease: Option<&ChildWriteLease>,
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
        if let Some(lease) = lease {
            require_child_write_lease(
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

    pub fn child_directives(&self, target: &ChildRef) -> StoreResult<Vec<ChildDirective>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT id, target_kind, target_id, version, kind, text, source_json,
                    command_id, issued_at, applied_at, incorporated_at, incorporated_summary
             FROM child_directives
             WHERE target_kind=?1 AND target_id=?2
             ORDER BY version",
        )?;
        let rows = statement.query_map(
            params![target.target_kind(), target.target_id()],
            map_child_directive_row,
        )?;
        let mut directives = Vec::new();
        for row in rows {
            directives.push(row?);
        }
        Ok(directives)
    }

    pub fn child_directive_for_command(
        &self,
        command_id: &ChildCommandId,
    ) -> StoreResult<Option<ChildDirective>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT id, target_kind, target_id, version, kind, text, source_json,
                    command_id, issued_at, applied_at, incorporated_at, incorporated_summary
             FROM child_directives WHERE command_id=?1",
            params![command_id.as_str()],
            map_child_directive_row,
        )
        .optional()
        .map_err(StoreError::from)
    }

    pub fn mark_child_directive_applied(&self, target: &ChildRef, version: u32) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE child_directives SET applied_at=COALESCE(applied_at, ?1)
             WHERE target_kind=?2 AND target_id=?3 AND version=?4",
            params![
                OffsetDateTime::now_utc().unix_timestamp_nanos() as i64,
                target.target_kind(),
                target.target_id(),
                i64::from(version),
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub(crate) fn mark_child_directive_applied_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
        version: u32,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(&transaction, target, lease)?;
        let changed = transaction.execute(
            "UPDATE child_directives SET applied_at=COALESCE(applied_at, ?1)
             WHERE target_kind=?2 AND target_id=?3 AND version=?4",
            params![
                OffsetDateTime::now_utc().unix_timestamp_nanos() as i64,
                target.target_kind(),
                target.target_id(),
                i64::from(version),
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn incorporate_child_directive(
        &self,
        target: &ChildRef,
        version: u32,
        summary: &str,
    ) -> StoreResult<(ChildDirective, bool)> {
        self.incorporate_child_directive_with_lease(target, version, summary, None)
    }

    pub(crate) fn incorporate_child_directive_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
        version: u32,
        summary: &str,
    ) -> StoreResult<(ChildDirective, bool)> {
        self.incorporate_child_directive_with_lease(target, version, summary, Some(lease))
    }

    fn incorporate_child_directive_with_lease(
        &self,
        target: &ChildRef,
        version: u32,
        summary: &str,
        lease: Option<&ChildWriteLease>,
    ) -> StoreResult<(ChildDirective, bool)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(lease) = lease {
            require_child_write_lease(&transaction, target, lease)?;
        }
        let mut directive = transaction
            .query_row(
                "SELECT id, target_kind, target_id, version, kind, text, source_json,
                        command_id, issued_at, applied_at, incorporated_at, incorporated_summary
                 FROM child_directives
                 WHERE target_kind=?1 AND target_id=?2 AND version=?3",
                params![target.target_kind(), target.target_id(), i64::from(version)],
                map_child_directive_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        let (table, current): (&str, i64) = match target {
            ChildRef::Project(_) => (
                "project_sessions",
                transaction.query_row(
                    "SELECT current_directive_version FROM project_sessions WHERE id=?1",
                    params![target.target_id()],
                    |row| row.get(0),
                )?,
            ),
            ChildRef::Task(_) => (
                "task_sessions",
                transaction.query_row(
                    "SELECT current_directive_version FROM task_sessions WHERE id=?1",
                    params![target.target_id()],
                    |row| row.get(0),
                )?,
            ),
        };
        if current != i64::from(version) {
            return Err(StoreError::InvalidData(format!(
                "directive v{version} is not current; {table} {} is at v{current}",
                target.target_id()
            )));
        }
        if directive.incorporated_at.is_some() {
            return Ok((directive, false));
        }
        let now = OffsetDateTime::now_utc();
        transaction.execute(
            "UPDATE child_directives
             SET incorporated_at=?1, incorporated_summary=?2
             WHERE target_kind=?3 AND target_id=?4 AND version=?5",
            params![
                now.unix_timestamp_nanos() as i64,
                summary,
                target.target_kind(),
                target.target_id(),
                i64::from(version),
            ],
        )?;
        match target {
            ChildRef::Project(_) => transaction.execute(
                "UPDATE project_sessions
                 SET incorporated_directive_version=?2, updated_at=?3 WHERE id=?1",
                params![target.target_id(), i64::from(version), now.unix_timestamp()],
            )?,
            ChildRef::Task(_) => transaction.execute(
                "UPDATE task_sessions
                 SET incorporated_directive_version=?2, updated_at=?3 WHERE id=?1",
                params![target.target_id(), i64::from(version), now.unix_timestamp()],
            )?,
        };
        transaction.commit()?;
        directive.incorporated_at = Some(now);
        directive.incorporated_summary = Some(summary.to_string());
        Ok((directive, true))
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

fn insert_initial_task(conn: &Connection, session: &TaskSession, pr: &TaskPr) -> StoreResult<()> {
    validate_task_session(session)?;
    validate_initial_task_pr(session, pr)?;
    validate_task_project_session(conn, session)?;
    let parameters = task_session_params(session);
    conn.execute(
        TASK_SESSION_INSERT,
        rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
    )?;
    insert_task_pr(conn, pr)?;
    seed_task_linear_observation(conn, session)
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
    current_directive_version, incorporated_directive_version,
    lf_bin, db_path, lf_home, abandon_requested_at, abandon_reason,
    iterate_flow, iterate_interaction_policy, phase_cursor, phase_iteration,
    process_group_id, process_agent, process_provider,
    process_provider_session_id, process_lease_state, process_outcome_json,
    kickoff_flow, kickoff_interaction_policy, gate_flow, gate_interaction_policy,
    lifecycle_phase, phase_epoch, gate_cycle, gate_proposal_json,
    process_provenance_json
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
    ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27,
    ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44,
    ?45, ?46, ?47, ?48, ?49, ?50, ?51, ?52, ?53
)";
const TASK_SESSION_COLUMNS: &str = "SELECT
    id, issue_id, issue_identifier, issue_title, issue_description,
    project_id, project_slug, project_name, project_prompt_context, wave_id,
    status, status_reason, status_at, worktree, workspace_slug,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, created_at, updated_at,
    pm_snapshot_synced_at,
    pm_writeback_json, project_session_id,
    current_directive_version, incorporated_directive_version,
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
    current_directive_version, incorporated_directive_version,
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
    current_directive_version=MAX(current_directive_version, ?28),
    incorporated_directive_version=MAX(incorporated_directive_version, ?29),
    lf_bin=?30, db_path=?31, lf_home=?32,
    abandon_requested_at=?33, abandon_reason=?34
    WHERE id=?1";
const TASK_SESSION_LEASE_UPDATE: &str = "UPDATE task_sessions SET
    issue_id=?2, issue_identifier=?3, issue_title=?4, issue_description=?5,
    project_id=?6, project_slug=?7, project_name=?8, project_prompt_context=?9,
    wave_id=?10, status=?11, status_reason=?12, status_at=?13,
    worktree=?14, workspace_slug=?15, agent=?16, provider=?17,
    provider_session_id=?18, process_generation=?19, process_pid=?20,
    process_tmux_name=?21, process_started_at=?22,
    created_at=?23, updated_at=?24,
    pm_snapshot_synced_at=?25, pm_writeback_json=?26,
    project_session_id=?27,
    current_directive_version=MAX(current_directive_version, ?28),
    incorporated_directive_version=MAX(incorporated_directive_version, ?29),
    lf_bin=?30, db_path=?31, lf_home=?32,
    abandon_requested_at=?33, abandon_reason=?34,
    lifecycle_phase=CASE WHEN ?50>=phase_epoch THEN ?49 ELSE lifecycle_phase END,
    phase_cursor=CASE
        WHEN ?50>phase_epoch OR
             (?50=phase_epoch AND (?38>phase_iteration OR
                                   (?38=phase_iteration AND ?37>phase_cursor)))
        THEN ?37 ELSE phase_cursor
    END,
    phase_iteration=CASE
        WHEN ?50>phase_epoch THEN ?38
        WHEN ?50=phase_epoch THEN MAX(phase_iteration, ?38)
        ELSE phase_iteration
    END,
    phase_epoch=MAX(phase_epoch, ?50),
    gate_cycle=CASE WHEN ?50>=phase_epoch THEN ?51 ELSE gate_cycle END,
    gate_proposal_json=CASE WHEN ?50>=phase_epoch THEN ?52 ELSE gate_proposal_json END,
    process_group_id=?39, process_agent=?40, process_provider=?41,
    process_provider_session_id=?42, process_lease_state=?43,
    process_outcome_json=?44, process_provenance_json=?53
    WHERE id=?1 AND process_generation=?54 AND process_lease_token=?55
      AND process_lease_state=?56
      AND (
        status NOT IN ('completed', 'abandoned') OR status=?11 OR
        (status='completed' AND ?11='running' AND ?49='gate' AND ?50>phase_epoch)
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
const CHILD_COMMAND_COLUMNS: &str = "SELECT
    id, target_kind, session_id, source_json, kind_json, created_at,
    claimed_by_generation, accepted_at, state, effect, error
    FROM child_commands";

fn ensure_directive_target(
    directive: &ChildDirective,
    target_kind: &str,
    target_id: &str,
) -> StoreResult<()> {
    if directive.target.target_kind() != target_kind || directive.target.target_id() != target_id {
        return Err(StoreError::InvalidData(format!(
            "directive {} targets {} {}, expected {target_kind} {target_id}",
            directive.id,
            directive.target.target_kind(),
            directive.target.target_id()
        )));
    }
    Ok(())
}

fn insert_child_directive(conn: &Connection, directive: &ChildDirective) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO child_directives (
            id, target_kind, target_id, version, kind, text, source_json,
            command_id, issued_at, applied_at, incorporated_at, incorporated_summary
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            directive.id.as_str(),
            directive.target.target_kind(),
            directive.target.target_id(),
            i64::from(directive.version),
            directive.kind.as_str(),
            directive.text,
            serde_json::to_string(&directive.source)?,
            directive.command_id.as_ref().map(ChildCommandId::as_str),
            directive.issued_at.unix_timestamp_nanos() as i64,
            directive
                .applied_at
                .map(|at| at.unix_timestamp_nanos() as i64),
            directive
                .incorporated_at
                .map(|at| at.unix_timestamp_nanos() as i64),
            directive.incorporated_summary,
        ],
    )?;
    Ok(())
}

fn supersede_child_commands(
    conn: &Connection,
    target_kind: &str,
    target_id: &str,
) -> StoreResult<Vec<ChildCommandId>> {
    let superseded = {
        let mut statement = conn.prepare(
            "SELECT id FROM child_commands
             WHERE target_kind=?1 AND session_id=?2
               AND state IN ('persisted', 'claimed')
             ORDER BY created_at, id",
        )?;
        let rows = statement.query_map(params![target_kind, target_id], |row| {
            Ok(ChildCommandId::from_raw(row.get::<_, String>(0)?))
        })?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }
        ids
    };
    conn.execute(
        "UPDATE child_commands SET state='superseded', effect=NULL, error=NULL
         WHERE target_kind=?1 AND session_id=?2
           AND state IN ('persisted', 'claimed')",
        params![target_kind, target_id],
    )?;
    Ok(superseded)
}

fn map_child_directive_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChildDirective> {
    let target_kind: String = row.get(1)?;
    let target_id: String = row.get(2)?;
    let target = match target_kind.as_str() {
        "project" => ChildRef::Project(ProjectSessionId::from_raw(target_id)),
        "task" => ChildRef::Task(TaskSessionId::from_raw(target_id)),
        value => {
            return Err(invalid_column(
                1,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown directive target {value:?}"),
                ),
            ))
        }
    };
    let kind = match row.get::<_, String>(4)?.as_str() {
        "initial" => DirectiveKind::Initial,
        "replacement" => DirectiveKind::Replacement,
        "work_revised" => DirectiveKind::WorkRevised,
        value => {
            return Err(invalid_column(
                4,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown directive kind {value:?}"),
                ),
            ))
        }
    };
    let source_json: String = row.get(6)?;
    let applied_at = row
        .get::<_, Option<i64>>(9)?
        .map(|value| task_command_datetime(9, value))
        .transpose()?;
    let incorporated_at = row
        .get::<_, Option<i64>>(10)?
        .map(|value| task_command_datetime(10, value))
        .transpose()?;
    Ok(ChildDirective {
        id: ChildDirectiveId::from_raw(row.get::<_, String>(0)?),
        target,
        version: row.get::<_, i64>(3)? as u32,
        kind,
        text: row.get(5)?,
        source: serde_json::from_str(&source_json).map_err(|error| invalid_column(6, error))?,
        command_id: row
            .get::<_, Option<String>>(7)?
            .map(ChildCommandId::from_raw),
        issued_at: task_command_datetime(8, row.get(8)?)?,
        applied_at,
        incorporated_at,
        incorporated_summary: row.get(11)?,
    })
}

/// Persist one Linear comment as a follow-up command, exactly once. The insert
/// into `task_linear_ingested_comments` is the guard — the command is written
/// only when the comment id is new to the ledger, so a redelivered webhook or an
/// overlapping catch-up read cannot double-deliver. Shared by the snapshot apply
/// loop and the single-comment webhook path.
fn ingest_linear_comment(
    conn: &Connection,
    session_id: &str,
    comment_id: &str,
    command: &ChildCommand,
    observed_at: i64,
) -> StoreResult<Option<ChildCommandId>> {
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO task_linear_ingested_comments
            (session_id, comment_id, ingested_at) VALUES (?1, ?2, ?3)",
        params![session_id, comment_id, observed_at],
    )?;
    if inserted == 1 {
        insert_child_command(conn, command)?;
        Ok(Some(command.id.clone()))
    } else {
        Ok(None)
    }
}

pub(super) fn insert_child_command(conn: &Connection, command: &ChildCommand) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO child_commands (
            id, target_kind, session_id, source_json, kind_json, created_at,
            claimed_by_generation, accepted_at, state, effect, error
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            command.id.as_str(),
            command.target.target_kind(),
            command.target.target_id(),
            serde_json::to_string(&command.source)?,
            serde_json::to_string(&command.kind)?,
            command.created_at.unix_timestamp_nanos() as i64,
            command.claimed_by_generation.map(i64::from),
            command
                .accepted_at
                .map(|at| at.unix_timestamp_nanos() as i64),
            command.state.as_str(),
            command.effect.map(ChildCommandEffect::as_str),
            command.error.as_deref(),
        ],
    )?;
    Ok(())
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
        Box::new(i64::from(session.current_directive_version)),
        Box::new(i64::from(session.incorporated_directive_version)),
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

fn task_session_control_params(session: &TaskSession) -> Vec<Box<dyn ToSql>> {
    let mut parameters = task_session_params(session);
    parameters.truncate(34);
    parameters
}

fn update_task_session_for_lease_in(
    conn: &Connection,
    session: &TaskSession,
    lease: &ChildWriteLease,
    expected_state: ChildLeaseState,
) -> StoreResult<usize> {
    let mut parameters = task_session_params(session);
    parameters.push(Box::new(i64::from(lease.generation)));
    parameters.push(Box::new(lease.token.as_str().to_string()));
    parameters.push(Box::new(expected_state.as_str().to_string()));
    Ok(conn.execute(
        TASK_SESSION_LEASE_UPDATE,
        rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
    )?)
}

fn lease_revoked(kind: &str, id: &str, lease: &ChildWriteLease) -> StoreError {
    StoreError::LeaseRevoked {
        target: format!("{kind} {id}"),
        generation: lease.generation,
    }
}

pub(super) fn require_child_write_lease(
    conn: &Connection,
    target: &ChildRef,
    lease: &ChildWriteLease,
) -> StoreResult<()> {
    let table = match target {
        ChildRef::Project(_) => "project_sessions",
        ChildRef::Task(_) => "task_sessions",
    };
    let current: bool = conn.query_row(
        &format!(
            "SELECT EXISTS(
                SELECT 1 FROM {table}
                WHERE id=?1 AND process_generation=?2
                  AND process_lease_token=?3 AND process_lease_state='active'
             )"
        ),
        params![
            target.target_id(),
            i64::from(lease.generation),
            lease.token.as_str()
        ],
        |row| row.get(0),
    )?;
    if current {
        Ok(())
    } else {
        Err(lease_revoked(
            match target {
                ChildRef::Project(_) => "Project Session",
                ChildRef::Task(_) => "Task Session",
            },
            target.target_id(),
            lease,
        ))
    }
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
        && left.abandoned_at == right.abandoned_at
}

fn invalid_column(
    index: usize,
    error: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(error))
}

fn task_command_datetime(index: usize, value: i64) -> rusqlite::Result<time::OffsetDateTime> {
    time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(value))
        .map_err(|error| invalid_column(index, error))
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
            let state_text: String = row.get(42)?;
            let outcome_json: Option<String> = row.get(43)?;
            let provenance_json: Option<String> = row.get(52)?;
            Some(ChildProcessGeneration {
                generation: generation as u32,
                pid: row.get::<_, Option<i64>>(19)?.map(|pid| pid as u32),
                process_group_id: row.get::<_, Option<i64>>(38)?.map(|id| id as u32),
                tmux_name: row.get::<_, Option<String>>(20)?.unwrap_or_default(),
                agent: row.get(39)?,
                provider: row.get(40)?,
                provider_session_id: row.get(41)?,
                started_at: crate::store::rows::unix_to_datetime(started_at),
                state: ChildLeaseState::parse(&state_text)
                    .map_err(|error| invalid_column(42, error))?,
                outcome: outcome_json
                    .map(|json| serde_json::from_str(&json))
                    .transpose()
                    .map_err(|error| invalid_column(43, error))?,
                provenance: provenance_json
                    .map(|json| serde_json::from_str(&json))
                    .transpose()
                    .map_err(|error| invalid_column(52, error))?,
            })
        }
        _ => None,
    };
    // Columns 29/30/31 (lf_bin/db_path/lf_home) are legacy dead schema: a
    // Session no longer pins a binary, so they are not read into domain state.
    let abandon_intent = match (
        row.get::<_, Option<i64>>(32)?,
        row.get::<_, Option<String>>(33)?,
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
        current_directive_version: row.get::<_, i64>(27)? as u32,
        incorporated_directive_version: row.get::<_, i64>(28)? as u32,
        status,
        status_reason: row.get(11)?,
        status_at: crate::store::rows::unix_to_datetime(row.get(12)?),
        worktree: PathBuf::from(row.get::<_, String>(13)?),
        workspace_slug: row.get(14)?,
        lifecycle: TaskLifecyclePlan {
            kickoff: TaskPhasePlan {
                flow: row.get(44)?,
                interaction_policy: row
                    .get::<_, String>(45)?
                    .parse::<InteractionPolicy>()
                    .map_err(|error| invalid_column(45, error))?,
            },
            iterate: TaskPhasePlan {
                flow: row.get(34)?,
                interaction_policy: row
                    .get::<_, String>(35)?
                    .parse::<InteractionPolicy>()
                    .map_err(|error| invalid_column(35, error))?,
            },
            gate: TaskPhasePlan {
                flow: row.get(46)?,
                interaction_policy: row
                    .get::<_, String>(47)?
                    .parse::<InteractionPolicy>()
                    .map_err(|error| invalid_column(47, error))?,
            },
        },
        lifecycle_phase: row
            .get::<_, String>(48)?
            .parse::<TaskLifecyclePhase>()
            .map_err(|error| invalid_column(48, error))?,
        phase_epoch: row.get::<_, i64>(49)? as u32,
        phase_cursor: row.get::<_, i64>(36)? as u32,
        phase_iteration: row.get::<_, i64>(37)? as u32,
        gate_cycle: row.get::<_, i64>(50)? as u32,
        gate_proposal: row
            .get::<_, Option<String>>(51)?
            .map(|json| serde_json::from_str(&json))
            .transpose()
            .map_err(|error| invalid_column(51, error))?,
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

fn map_child_command_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChildCommand> {
    let target_id: String = row.get(2)?;
    let target = match row.get::<_, String>(1)?.as_str() {
        "project" => ChildRef::Project(ProjectSessionId::from_raw(target_id)),
        "task" => ChildRef::Task(TaskSessionId::from_raw(target_id)),
        value => {
            return Err(invalid_column(
                1,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown child command target {value:?}"),
                ),
            ))
        }
    };
    let source_json: String = row.get(3)?;
    let kind_json: String = row.get(4)?;
    let source: ChildCommandSource =
        serde_json::from_str(&source_json).map_err(|error| invalid_column(3, error))?;
    let kind: ChildCommandKind =
        serde_json::from_str(&kind_json).map_err(|error| invalid_column(4, error))?;
    let state = parse_command_state(row.get::<_, String>(8)?.as_str(), 8)?;
    let effect = parse_command_effect(row.get::<_, Option<String>>(9)?.as_deref(), 9)?;
    Ok(ChildCommand {
        id: ChildCommandId::from_raw(row.get::<_, String>(0)?),
        target,
        source,
        kind,
        state,
        effect,
        created_at: task_command_datetime(5, row.get(5)?)?,
        claimed_by_generation: row.get::<_, Option<i64>>(6)?.map(|value| value as u32),
        accepted_at: row
            .get::<_, Option<i64>>(7)?
            .map(|value| task_command_datetime(7, value))
            .transpose()?,
        error: row.get(10)?,
    })
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
    current_directive_version, incorporated_directive_version,
    lf_bin, db_path, lf_home, abandon_requested_at, abandon_reason,
    process_group_id, process_agent, process_provider,
    process_provider_session_id, process_lease_state, process_outcome_json,
    process_provenance_json
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
    ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22,
    ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36
)";
const PROJECT_SESSION_COLUMNS: &str = "SELECT
    id, project_id, project_slug, project_name, project_prompt_context,
    wave_id, pm_snapshot_synced_at, status,
    status_reason, status_at, iteration, observation_cursor, last_state_fingerprint,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, created_at, updated_at,
    current_directive_version, incorporated_directive_version,
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
    current_directive_version, incorporated_directive_version,
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
    current_directive_version=MAX(current_directive_version, ?23),
    incorporated_directive_version=MAX(incorporated_directive_version, ?24),
    lf_bin=?25, db_path=?26, lf_home=?27,
    abandon_requested_at=?28, abandon_reason=?29
    WHERE id=?1";
const PROJECT_SESSION_LEASE_UPDATE: &str = "UPDATE project_sessions SET
    project_id=?2, project_slug=?3, project_name=?4, project_prompt_context=?5,
    wave_id=?6, pm_snapshot_synced_at=?7, status=?8,
    status_reason=?9, status_at=?10, iteration=?11,
    observation_cursor=?12, last_state_fingerprint=?13, agent=?14, provider=?15,
    provider_session_id=?16, process_generation=?17, process_pid=?18,
    process_tmux_name=?19, process_started_at=?20, created_at=?21,
    updated_at=?22,
    current_directive_version=MAX(current_directive_version, ?23),
    incorporated_directive_version=MAX(incorporated_directive_version, ?24),
    lf_bin=?25, db_path=?26, lf_home=?27,
    abandon_requested_at=?28, abandon_reason=?29,
    process_group_id=?30, process_agent=?31, process_provider=?32,
    process_provider_session_id=?33, process_lease_state=?34,
    process_outcome_json=?35, process_provenance_json=?36
    WHERE id=?1 AND process_generation=?37 AND process_lease_token=?38
      AND process_lease_state=?39
      AND (status NOT IN ('completed', 'abandoned') OR status=?8)";
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
        Box::new(i64::from(session.current_directive_version)),
        Box::new(i64::from(session.incorporated_directive_version)),
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
    parameters.truncate(29);
    parameters
}

fn update_project_session_for_lease_in(
    conn: &Connection,
    session: &ProjectSession,
    lease: &ChildWriteLease,
    expected_state: ChildLeaseState,
) -> StoreResult<usize> {
    let mut parameters = project_session_params(session);
    parameters.push(Box::new(i64::from(lease.generation)));
    parameters.push(Box::new(lease.token.as_str().to_string()));
    parameters.push(Box::new(expected_state.as_str().to_string()));
    Ok(conn.execute(
        PROJECT_SESSION_LEASE_UPDATE,
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
            let state_text: String = row.get(33)?;
            let outcome_json: Option<String> = row.get(34)?;
            let provenance_json: Option<String> = row.get(35)?;
            Some(ChildProcessGeneration {
                generation: generation as u32,
                pid: row.get::<_, Option<i64>>(17)?.map(|pid| pid as u32),
                process_group_id: row.get::<_, Option<i64>>(29)?.map(|id| id as u32),
                tmux_name: row.get::<_, Option<String>>(18)?.unwrap_or_default(),
                agent: row.get(30)?,
                provider: row.get(31)?,
                provider_session_id: row.get(32)?,
                started_at: crate::store::rows::unix_to_datetime(started_at),
                state: ChildLeaseState::parse(&state_text)
                    .map_err(|error| invalid_column(33, error))?,
                outcome: outcome_json
                    .map(|json| serde_json::from_str(&json))
                    .transpose()
                    .map_err(|error| invalid_column(34, error))?,
                provenance: provenance_json
                    .map(|json| serde_json::from_str(&json))
                    .transpose()
                    .map_err(|error| invalid_column(35, error))?,
            })
        }
        _ => None,
    };
    // Columns 24/25/26 (lf_bin/db_path/lf_home) are legacy dead schema: a
    // Session no longer pins a binary, so they are not read into domain state.
    let abandon_intent = match (
        row.get::<_, Option<i64>>(27)?,
        row.get::<_, Option<String>>(28)?,
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
        current_directive_version: row.get::<_, i64>(22)? as u32,
        incorporated_directive_version: row.get::<_, i64>(23)? as u32,
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

fn claim_child_commands_in(
    conn: &Connection,
    target: &ChildRef,
    generation: u32,
) -> StoreResult<()> {
    conn.execute(
        "UPDATE child_commands SET claimed_by_generation=?1, state='claimed'
         WHERE target_kind=?2 AND session_id=?3
           AND state IN ('persisted', 'claimed')
           AND (claimed_by_generation IS NULL OR claimed_by_generation<>?1)",
        params![
            i64::from(generation),
            target.target_kind(),
            target.target_id()
        ],
    )?;
    Ok(())
}

fn read_claimed_child_commands(
    conn: &Connection,
    target: &ChildRef,
    generation: u32,
) -> StoreResult<Vec<ChildCommand>> {
    let mut statement = conn.prepare(&format!(
        "{CHILD_COMMAND_COLUMNS}
         WHERE target_kind=?1 AND session_id=?2
           AND claimed_by_generation=?3 AND state='claimed'
         ORDER BY created_at, id"
    ))?;
    let rows = statement.query_map(
        params![
            target.target_kind(),
            target.target_id(),
            i64::from(generation)
        ],
        map_child_command_row,
    )?;
    let mut commands = Vec::new();
    for row in rows {
        commands.push(row?);
    }
    Ok(commands)
}

fn parse_command_state(value: &str, index: usize) -> rusqlite::Result<ChildCommandState> {
    match value {
        "persisted" => Ok(ChildCommandState::Persisted),
        "claimed" => Ok(ChildCommandState::Claimed),
        "delivering" => Ok(ChildCommandState::Delivering),
        "accepted" => Ok(ChildCommandState::Accepted),
        "failed" => Ok(ChildCommandState::Failed),
        "superseded" => Ok(ChildCommandState::Superseded),
        "uncertain" => Ok(ChildCommandState::Uncertain),
        value => Err(invalid_column(
            index,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown command state {value:?}"),
            ),
        )),
    }
}

fn parse_command_effect(
    value: Option<&str>,
    index: usize,
) -> rusqlite::Result<Option<ChildCommandEffect>> {
    match value {
        None => Ok(None),
        Some("live_steer") => Ok(Some(ChildCommandEffect::LiveSteer)),
        Some("next_turn") => Ok(Some(ChildCommandEffect::NextTurn)),
        Some("replacement") => Ok(Some(ChildCommandEffect::Replacement)),
        Some("decision") => Ok(Some(ChildCommandEffect::Decision)),
        Some(value) => Err(invalid_column(
            index,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown command effect {value:?}"),
            ),
        )),
    }
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
