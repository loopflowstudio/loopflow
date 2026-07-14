//! SQLite persistence for Project and Task Sessions.

use std::path::PathBuf;

use rusqlite::{params, Connection, OptionalExtension, ToSql};
use time::OffsetDateTime;

use crate::child_session::{
    BoundaryResult, ChildCommand, ChildCommandEffect, ChildCommandId, ChildCommandKind,
    ChildCommandSource, ChildCommandState, ChildDirective, ChildDirectiveId,
    ChildProcessGeneration, ChildRef, DirectiveKind, SessionSupervisor,
};
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
    PullRequestRef, TaskEvent, TaskEventKind, TaskSession, TaskSessionId, TaskSessionStatus,
};

use super::SqliteStore;

impl SqliteStore {
    // Durable task sessions: Linear identity, immutable placement, commands,
    // and lifecycle events share one sqlite transaction boundary.

    pub fn insert_task_session(&self, session: &TaskSession) -> StoreResult<()> {
        validate_task_session(session)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        let parameters = task_session_params(session);
        conn.execute(
            TASK_SESSION_INSERT,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        Ok(())
    }

    pub fn reserve_task_session(&self, session: &TaskSession) -> StoreResult<()> {
        validate_task_session(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction()?;
        let parameters = task_session_params(session);
        transaction.execute(
            TASK_SESSION_INSERT,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn reserve_task_session_with_directive(
        &self,
        session: &TaskSession,
        directive: &ChildDirective,
    ) -> StoreResult<()> {
        validate_task_session(session)?;
        ensure_directive_target(directive, "task", session.id.as_str())?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction()?;
        let parameters = task_session_params(session);
        transaction.execute(
            TASK_SESSION_INSERT,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        insert_child_directive(&transaction, directive)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn update_task_session(&self, session: &TaskSession) -> StoreResult<()> {
        validate_task_session(session)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        let parameters = task_session_params(session);
        let changed = conn.execute(
            TASK_SESSION_UPDATE,
            rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn reserve_task_process(
        &self,
        session: &TaskSession,
        expected_status: TaskSessionStatus,
    ) -> StoreResult<bool> {
        validate_task_session(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction()?;
        let current_status: String = transaction.query_row(
            "SELECT status FROM task_sessions WHERE id = ?1",
            params![session.id.as_str()],
            |row| row.get(0),
        )?;
        if current_status != expected_status.as_str() {
            return Ok(false);
        }
        let changed = transaction.execute(
            "UPDATE task_sessions SET
                status = ?2, status_reason = ?3, status_at = ?4,
                process_generation = ?5, process_pid = ?6,
                process_tmux_name = ?7, process_started_at = ?8,
                updated_at = ?9
             WHERE id = ?1 AND status = ?10",
            params![
                session.id.as_str(),
                session.status.as_str(),
                session.status_reason,
                session.status_at.unix_timestamp(),
                session
                    .latest_process
                    .as_ref()
                    .map(|process| i64::from(process.generation)),
                session
                    .latest_process
                    .as_ref()
                    .and_then(|process| process.pid.map(i64::from)),
                session
                    .latest_process
                    .as_ref()
                    .map(|process| &process.tmux_name),
                session
                    .latest_process
                    .as_ref()
                    .map(|process| process.started_at.unix_timestamp()),
                session.updated_at.unix_timestamp(),
                expected_status.as_str(),
            ],
        )?;
        if changed == 0 {
            return Ok(false);
        }
        transaction.commit()?;
        Ok(true)
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
        let query = format!(
            "{TASK_SESSION_COLUMNS} WHERE issue_id = ?1 OR issue_identifier = ?1 ORDER BY created_at"
        );
        let mut statement = conn.prepare(&query)?;
        let rows = statement.query_map(params![issue], map_task_session_row)?;
        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }
        match sessions.len() {
            0 => Ok(None),
            1 => Ok(sessions.pop()),
            count => Err(StoreError::InvalidData(format!(
                "issue {issue:?} resolves to {count} task sessions"
            ))),
        }
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

    pub fn insert_child_command(&self, command: &ChildCommand) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        insert_child_command(&conn, command)
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
        let transaction = conn.transaction()?;
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

    pub fn supersede_and_insert_child_command(
        &self,
        command: &ChildCommand,
    ) -> StoreResult<Vec<ChildCommandId>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction()?;
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
        let transaction = conn.transaction()?;
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
        let transaction = conn.transaction()?;
        claim_child_commands_in(&transaction, target, generation)?;
        let commands = read_claimed_child_commands(&transaction, target, generation)?;
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
        let transaction = conn.transaction()?;
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
        let parameters = task_session_params(&session);
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

    pub fn accept_child_command(
        &self,
        command_id: &ChildCommandId,
        effect: Option<ChildCommandEffect>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction()?;
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

    pub fn mark_stale_child_deliveries_uncertain(
        &self,
        target: &ChildRef,
        generation: u32,
    ) -> StoreResult<Vec<ChildCommand>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction()?;
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

    pub fn append_task_event(
        &self,
        session_id: &TaskSessionId,
        kind: &TaskEventKind,
    ) -> StoreResult<TaskEvent> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction()?;
        let created_at = now_unix();
        transaction.execute(
            "INSERT INTO task_events (session_id, kind_json, created_at) VALUES (?1, ?2, ?3)",
            params![
                session_id.as_str(),
                serde_json::to_string(kind)?,
                created_at
            ],
        )?;
        let event_id = transaction.last_insert_rowid();
        if kind.is_wave_observable() {
            let session = transaction.query_row(
                TASK_SESSION_SELECT,
                params![session_id.as_str()],
                map_task_session_row,
            )?;
            insert_observation(
                &transaction,
                &session.supervisor,
                &ChildRef::Task(session_id.clone()),
                event_id,
                &ChildEventPayload::Task {
                    event: kind.clone(),
                },
                created_at,
            )?;
            if matches!(session.supervisor, SessionSupervisor::Project { .. })
                && kind.is_root_wave_observable()
            {
                insert_observation(
                    &transaction,
                    &SessionSupervisor::Wave {
                        wave_id: session.wave_id,
                    },
                    &ChildRef::Task(session_id.clone()),
                    event_id,
                    &ChildEventPayload::Task {
                        event: kind.clone(),
                    },
                    created_at,
                )?;
            }
        }
        transaction.commit()?;
        Ok(TaskEvent {
            id: event_id,
            session_id: session_id.clone(),
            kind: kind.clone(),
            created_at: crate::store::rows::unix_to_datetime(created_at),
        })
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
        let transaction = conn.transaction()?;
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
        let changed = conn.execute(
            PROJECT_SESSION_UPDATE,
            rusqlite::params_from_iter(
                project_session_params(session)
                    .iter()
                    .map(|value| value.as_ref()),
            ),
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn reserve_project_process(
        &self,
        session: &ProjectSession,
        expected_status: ProjectSessionStatus,
    ) -> StoreResult<bool> {
        validate_project_session(session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction()?;
        let changed = transaction.execute(
            "UPDATE project_sessions SET
                status=?2, status_reason=?3, status_at=?4,
                process_generation=?5, process_pid=?6, process_tmux_name=?7,
                process_started_at=?8, updated_at=?9
             WHERE id=?1 AND status=?10",
            params![
                session.id.as_str(),
                session.status.as_str(),
                session.status_reason,
                session.status_at.unix_timestamp(),
                session
                    .latest_process
                    .as_ref()
                    .map(|process| i64::from(process.generation)),
                session
                    .latest_process
                    .as_ref()
                    .and_then(|process| process.pid.map(i64::from)),
                session
                    .latest_process
                    .as_ref()
                    .map(|process| &process.tmux_name),
                session
                    .latest_process
                    .as_ref()
                    .map(|process| process.started_at.unix_timestamp()),
                session.updated_at.unix_timestamp(),
                expected_status.as_str(),
            ],
        )?;
        if changed == 0 {
            return Ok(false);
        }
        transaction.commit()?;
        Ok(true)
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
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = format!(
            "{PROJECT_SESSION_COLUMNS} WHERE project_id=?1 OR project_slug=?1 ORDER BY created_at"
        );
        let mut statement = conn.prepare(&query)?;
        let rows = statement.query_map(params![project], map_project_session_row)?;
        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }
        match sessions.len() {
            0 => Ok(None),
            1 => Ok(sessions.pop()),
            count => Err(StoreError::InvalidData(format!(
                "project {project:?} resolves to {count} Project Sessions"
            ))),
        }
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
        let transaction = conn.transaction()?;
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
        transaction.execute(
            PROJECT_SESSION_UPDATE,
            rusqlite::params_from_iter(
                project_session_params(&session)
                    .iter()
                    .map(|value| value.as_ref()),
            ),
        )?;
        transaction.commit()?;
        Ok(BoundaryResult::Stopped(session))
    }

    pub fn append_project_event(
        &self,
        session_id: &ProjectSessionId,
        kind: &ProjectEventKind,
    ) -> StoreResult<ProjectEvent> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction()?;
        let created_at = now_unix();
        transaction.execute(
            "INSERT INTO project_events (session_id, kind_json, created_at)
             VALUES (?1, ?2, ?3)",
            params![
                session_id.as_str(),
                serde_json::to_string(kind)?,
                created_at
            ],
        )?;
        let event_id = transaction.last_insert_rowid();
        if kind.is_wave_observable() {
            let session = transaction.query_row(
                PROJECT_SESSION_SELECT,
                params![session_id.as_str()],
                map_project_session_row,
            )?;
            insert_observation(
                &transaction,
                &SessionSupervisor::Wave {
                    wave_id: session.wave_id,
                },
                &ChildRef::Project(session_id.clone()),
                event_id,
                &ChildEventPayload::Project {
                    event: kind.clone(),
                },
                created_at,
            )?;
        }
        transaction.commit()?;
        Ok(ProjectEvent {
            id: event_id,
            session_id: session_id.clone(),
            kind: kind.clone(),
            created_at: crate::store::rows::unix_to_datetime(created_at),
        })
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

    pub fn pending_observations(
        &self,
        supervisor: &SessionSupervisor,
    ) -> StoreResult<Vec<ObservationOutboxRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (kind, id) = supervisor_columns(supervisor);
        let mut statement = conn.prepare(
            "SELECT id, supervisor_kind, supervisor_id, source_kind, source_id,
                    event_id, payload_json, delivered_at
             FROM observation_outbox
             WHERE supervisor_kind=?1 AND supervisor_id=?2 AND delivered_at IS NULL
             ORDER BY id",
        )?;
        let rows = statement.query_map(params![kind, id], map_observation_row)?;
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
        let (
            SessionSupervisor::Project {
                session_id: supervisor_id,
            },
            ChildRef::Task(task_id),
            ChildEventPayload::Task { event },
        ) = (
            &observation.supervisor,
            &observation.source,
            &observation.payload,
        )
        else {
            return Err(StoreError::InvalidData(
                "Project Session can consume only supervised Task observations".to_string(),
            ));
        };
        if supervisor_id != project_session_id {
            return Err(StoreError::InvalidData(format!(
                "observation {} belongs to Project Session {supervisor_id}",
                observation.id
            )));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction()?;
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

    pub fn incorporate_child_directive(
        &self,
        target: &ChildRef,
        version: u32,
        summary: &str,
    ) -> StoreResult<(ChildDirective, bool)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction()?;
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

fn validate_project_session(session: &ProjectSession) -> StoreResult<()> {
    session
        .validate()
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

const TASK_SESSION_INSERT: &str = "INSERT INTO task_sessions (
    id, issue_id, issue_identifier, issue_title, issue_description,
    project_id, project_slug, project_name, project_prompt_context, wave_id,
    status, status_reason, status_at, worktree, branch, base_commit,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, pr_number, pr_url,
    created_at, updated_at, pm_snapshot_synced_at,
    pm_writeback_json, supervisor_kind, supervisor_id,
    current_directive_version, incorporated_directive_version
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
    ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27,
    ?28, ?29, ?30, ?31, ?32, ?33
)";
const TASK_SESSION_COLUMNS: &str = "SELECT
    id, issue_id, issue_identifier, issue_title, issue_description,
    project_id, project_slug, project_name, project_prompt_context, wave_id,
    status, status_reason, status_at, worktree, branch, base_commit,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, pr_number, pr_url,
    created_at, updated_at, pm_snapshot_synced_at,
    pm_writeback_json, supervisor_kind, supervisor_id,
    current_directive_version, incorporated_directive_version
    FROM task_sessions";
const TASK_SESSION_SELECT: &str = "SELECT
    id, issue_id, issue_identifier, issue_title, issue_description,
    project_id, project_slug, project_name, project_prompt_context, wave_id,
    status, status_reason, status_at, worktree, branch, base_commit,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, pr_number, pr_url,
    created_at, updated_at, pm_snapshot_synced_at,
    pm_writeback_json, supervisor_kind, supervisor_id,
    current_directive_version, incorporated_directive_version
    FROM task_sessions WHERE id = ?1";
const TASK_SESSION_UPDATE: &str = "UPDATE task_sessions SET
    issue_id=?2, issue_identifier=?3, issue_title=?4, issue_description=?5,
    project_id=?6, project_slug=?7, project_name=?8, project_prompt_context=?9,
    wave_id=?10, status=?11, status_reason=?12, status_at=?13,
    worktree=?14, branch=?15, base_commit=?16, agent=?17, provider=?18,
    provider_session_id=?19, process_generation=?20, process_pid=?21,
    process_tmux_name=?22, process_started_at=?23, pr_number=?24,
    pr_url=?25, created_at=?26, updated_at=?27,
    pm_snapshot_synced_at=?28, pm_writeback_json=?29,
    supervisor_kind=?30, supervisor_id=?31,
    current_directive_version=MAX(current_directive_version, ?32),
    incorporated_directive_version=MAX(incorporated_directive_version, ?33)
    WHERE id=?1";
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

fn insert_child_command(conn: &Connection, command: &ChildCommand) -> StoreResult<()> {
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
        Box::new(session.branch.clone()),
        Box::new(session.base_commit.clone()),
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
        Box::new(
            session
                .pull_request
                .as_ref()
                .map(|pull_request| i64::from(pull_request.number)),
        ),
        Box::new(
            session
                .pull_request
                .as_ref()
                .map(|pull_request| pull_request.url.clone()),
        ),
        Box::new(session.created_at.unix_timestamp()),
        Box::new(session.updated_at.unix_timestamp()),
        Box::new(session.launch.pm_snapshot_synced_at),
        Box::new(
            serde_json::to_string(&session.pm_writeback)
                .expect("Task Session PM writeback state must serialize"),
        ),
        Box::new(match &session.supervisor {
            SessionSupervisor::Wave { .. } => "wave".to_string(),
            SessionSupervisor::Project { .. } => "project".to_string(),
        }),
        Box::new(match &session.supervisor {
            SessionSupervisor::Wave { wave_id } => wave_id.as_str().to_string(),
            SessionSupervisor::Project { session_id } => session_id.as_str().to_string(),
        }),
        Box::new(i64::from(session.current_directive_version)),
        Box::new(i64::from(session.incorporated_directive_version)),
    ]
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

fn map_task_session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskSession> {
    let status_text: String = row.get(10)?;
    let status = status_text
        .parse()
        .map_err(|error| invalid_column(10, error))?;
    let process_generation: Option<i64> = row.get(19)?;
    let process_started_at: Option<i64> = row.get(22)?;
    let process = match (process_generation, process_started_at) {
        (Some(generation), Some(started_at)) => Some(ChildProcessGeneration {
            generation: generation as u32,
            pid: row.get::<_, Option<i64>>(20)?.map(|pid| pid as u32),
            tmux_name: row.get::<_, Option<String>>(21)?.unwrap_or_default(),
            started_at: crate::store::rows::unix_to_datetime(started_at),
        }),
        _ => None,
    };
    let pr_number: Option<i64> = row.get(23)?;
    let pr_url: Option<String> = row.get(24)?;
    let pull_request = match (pr_number, pr_url) {
        (Some(number), Some(url)) => Some(PullRequestRef {
            number: number as u32,
            url,
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
            pm_snapshot_synced_at: row.get(27)?,
        },
        pm_writeback: serde_json::from_str(&row.get::<_, String>(28)?)
            .map_err(|error| invalid_column(28, error))?,
        wave_id: row.get(9)?,
        supervisor: match row.get::<_, String>(29)?.as_str() {
            "wave" => SessionSupervisor::Wave {
                wave_id: row.get(30)?,
            },
            "project" => SessionSupervisor::Project {
                session_id: crate::project_session::ProjectSessionId::from_raw(
                    row.get::<_, String>(30)?,
                ),
            },
            value => {
                return Err(invalid_column(
                    29,
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("unknown Task Session supervisor kind {value:?}"),
                    ),
                ))
            }
        },
        current_directive_version: row.get::<_, i64>(31)? as u32,
        incorporated_directive_version: row.get::<_, i64>(32)? as u32,
        status,
        status_reason: row.get(11)?,
        status_at: crate::store::rows::unix_to_datetime(row.get(12)?),
        worktree: PathBuf::from(row.get::<_, String>(13)?),
        branch: row.get(14)?,
        base_commit: row.get(15)?,
        agent: row.get(16)?,
        provider: row.get(17)?,
        provider_session_id: row.get(18)?,
        latest_process: process,
        pull_request,
        created_at: crate::store::rows::unix_to_datetime(row.get(25)?),
        updated_at: crate::store::rows::unix_to_datetime(row.get(26)?),
    })
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
    current_directive_version, incorporated_directive_version
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
    ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22,
    ?23, ?24
)";
const PROJECT_SESSION_COLUMNS: &str = "SELECT
    id, project_id, project_slug, project_name, project_prompt_context,
    wave_id, pm_snapshot_synced_at, status,
    status_reason, status_at, iteration, observation_cursor, last_state_fingerprint,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, created_at, updated_at,
    current_directive_version, incorporated_directive_version
    FROM project_sessions";
const PROJECT_SESSION_SELECT: &str = "SELECT
    id, project_id, project_slug, project_name, project_prompt_context,
    wave_id, pm_snapshot_synced_at, status,
    status_reason, status_at, iteration, observation_cursor, last_state_fingerprint,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, created_at, updated_at,
    current_directive_version, incorporated_directive_version
    FROM project_sessions WHERE id=?1";
const PROJECT_SESSION_UPDATE: &str = "UPDATE project_sessions SET
    project_id=?2, project_slug=?3, project_name=?4, project_prompt_context=?5,
    wave_id=?6, pm_snapshot_synced_at=?7, status=?8,
    status_reason=?9, status_at=?10, iteration=?11,
    observation_cursor=?12, last_state_fingerprint=?13, agent=?14, provider=?15,
    provider_session_id=?16, process_generation=?17, process_pid=?18,
    process_tmux_name=?19, process_started_at=?20, created_at=?21,
    updated_at=?22,
    current_directive_version=MAX(current_directive_version, ?23),
    incorporated_directive_version=MAX(incorporated_directive_version, ?24)
    WHERE id=?1";
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
    ]
}

fn map_project_session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectSession> {
    let status_text: String = row.get(7)?;
    let status = status_text
        .parse()
        .map_err(|error| invalid_column(7, error))?;
    let process_generation: Option<i64> = row.get(16)?;
    let process_started_at: Option<i64> = row.get(19)?;
    let process = match (process_generation, process_started_at) {
        (Some(generation), Some(started_at)) => Some(ChildProcessGeneration {
            generation: generation as u32,
            pid: row.get::<_, Option<i64>>(17)?.map(|pid| pid as u32),
            tmux_name: row.get::<_, Option<String>>(18)?.unwrap_or_default(),
            started_at: crate::store::rows::unix_to_datetime(started_at),
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

fn supervisor_columns(supervisor: &SessionSupervisor) -> (&'static str, String) {
    match supervisor {
        SessionSupervisor::Wave { wave_id } => ("wave", wave_id.as_str().to_string()),
        SessionSupervisor::Project { session_id } => ("project", session_id.as_str().to_string()),
    }
}

fn child_columns(source: &ChildRef) -> (&'static str, String) {
    match source {
        ChildRef::Project(session_id) => ("project", session_id.as_str().to_string()),
        ChildRef::Task(session_id) => ("task", session_id.as_str().to_string()),
    }
}

fn insert_observation(
    conn: &Connection,
    supervisor: &SessionSupervisor,
    source: &ChildRef,
    event_id: i64,
    payload: &ChildEventPayload,
    created_at: i64,
) -> StoreResult<()> {
    let (supervisor_kind, supervisor_id) = supervisor_columns(supervisor);
    let (source_kind, source_id) = child_columns(source);
    conn.execute(
        "INSERT OR IGNORE INTO observation_outbox (
            supervisor_kind, supervisor_id, source_kind, source_id,
            event_id, payload_json, created_at, delivered_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
        params![
            supervisor_kind,
            supervisor_id,
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
    let supervisor_kind: String = row.get(1)?;
    let supervisor_id: String = row.get(2)?;
    let supervisor = match supervisor_kind.as_str() {
        "wave" => SessionSupervisor::Wave {
            wave_id: WaveId::parse(&supervisor_id).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
        },
        "project" => SessionSupervisor::Project {
            session_id: ProjectSessionId::from_raw(supervisor_id),
        },
        value => {
            return Err(invalid_column(
                1,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown observation supervisor {value:?}"),
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
        supervisor,
        source,
        event_id: row.get(5)?,
        payload,
        delivered_at: row
            .get::<_, Option<i64>>(7)?
            .map(crate::store::rows::unix_to_datetime),
    })
}
