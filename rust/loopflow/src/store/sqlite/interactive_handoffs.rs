use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};

use crate::id::WaveId;
use crate::interactive_handoff::{
    InteractiveHandoff, InteractiveHandoffDataError, InteractiveHandoffId,
    InteractiveHandoffOutcome, InteractiveHandoffParent, InteractiveHandoffStatus,
    OpenInteractiveHandoff,
};
use crate::project_session::ProjectSessionStatus;
use crate::store::rows::unix_to_datetime;
use crate::store::{StoreError, StoreResult};
use crate::task::TaskSessionStatus;

use super::SqliteStore;

const HANDOFF_COLUMNS: &str = "SELECT id, parent_kind, parent_id, wave_id, home, cwd, provider,
            provider_session_id, body_generation, reason, environment_json,
            attach_argv_json, status, outcome_json, created_at, updated_at,
            attached_at, terminal_at, wake_claimed_at,
            wake_claimed_by_generation
       FROM interactive_handoffs";

impl SqliteStore {
    pub(crate) fn open_interactive_handoff(
        &self,
        request: &OpenInteractiveHandoff,
    ) -> StoreResult<(InteractiveHandoff, bool)> {
        request.validate().map_err(invalid_handoff)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = active_for_parent(&transaction, &request.parent)? {
            transaction.commit()?;
            return Ok((existing, false));
        }

        let wave_id = validate_parent(&transaction, request)?;
        let now = time::OffsetDateTime::now_utc();
        let handoff = InteractiveHandoff {
            id: InteractiveHandoffId::new(),
            parent: request.parent.clone(),
            wave_id,
            home: request.home.clone(),
            cwd: request.cwd.clone(),
            provider: request.provider.clone(),
            provider_session_id: request.provider_session_id.clone(),
            body_generation: request.body_generation,
            reason: request.reason.clone(),
            environment: request.environment.clone(),
            attach_argv: request.attach_argv.clone(),
            status: InteractiveHandoffStatus::Waiting,
            outcome: None,
            created_at: now,
            updated_at: now,
            attached_at: None,
            terminal_at: None,
            wake_claimed_at: None,
            wake_claimed_by_generation: None,
        };
        handoff.validate().map_err(invalid_handoff)?;
        transaction.execute(
            "INSERT INTO interactive_handoffs (
                id, parent_kind, parent_id, wave_id, home, cwd, provider,
                provider_session_id, body_generation, reason, environment_json,
                attach_argv_json, status, outcome_json, created_at, updated_at,
                attached_at, terminal_at, wake_claimed_at,
                wake_claimed_by_generation
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, NULL, ?14, ?14, NULL, NULL, NULL, NULL
             )",
            params![
                handoff.id.as_str(),
                handoff.parent.kind(),
                handoff.parent.id(),
                handoff.wave_id.as_str(),
                handoff.home.to_string(),
                handoff.cwd.display().to_string(),
                handoff.provider,
                handoff.provider_session_id,
                i64::from(handoff.body_generation),
                handoff.reason,
                serde_json::to_string(&handoff.environment)?,
                serde_json::to_string(&handoff.attach_argv)?,
                handoff.status.as_str(),
                now.unix_timestamp(),
            ],
        )?;
        transaction.commit()?;
        Ok((handoff, true))
    }

    pub(crate) fn get_interactive_handoff(
        &self,
        session_id: &InteractiveHandoffId,
    ) -> StoreResult<Option<InteractiveHandoff>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        get_handoff(&conn, session_id)
    }

    pub(crate) fn list_interactive_handoffs(
        &self,
        parent: Option<&InteractiveHandoffParent>,
    ) -> StoreResult<Vec<InteractiveHandoff>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (query, values) = match parent {
            Some(parent) => (
                format!(
                    "{HANDOFF_COLUMNS} WHERE parent_kind=?1 AND parent_id=?2 ORDER BY created_at"
                ),
                Some((parent.kind(), parent.id())),
            ),
            None => (format!("{HANDOFF_COLUMNS} ORDER BY created_at"), None),
        };
        let mut statement = conn.prepare(&query)?;
        let mut handoffs = Vec::new();
        if let Some((kind, id)) = values {
            let rows = statement.query_map(params![kind, id], map_handoff_row)?;
            for row in rows {
                handoffs.push(row?);
            }
        } else {
            let rows = statement.query_map([], map_handoff_row)?;
            for row in rows {
                handoffs.push(row?);
            }
        }
        Ok(handoffs)
    }

    pub(crate) fn attach_interactive_handoff(
        &self,
        session_id: &InteractiveHandoffId,
    ) -> StoreResult<InteractiveHandoff> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let handoff = get_handoff(&transaction, session_id)?.ok_or(StoreError::NotFound)?;
        if handoff.status == InteractiveHandoffStatus::Waiting {
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            transaction.execute(
                "UPDATE interactive_handoffs
                    SET status='attached', attached_at=?2, updated_at=?2
                  WHERE id=?1 AND status='waiting' AND terminal_at IS NULL",
                params![session_id.as_str(), now],
            )?;
        }
        let handoff = get_handoff(&transaction, session_id)?.ok_or(StoreError::NotFound)?;
        transaction.commit()?;
        Ok(handoff)
    }

    pub(crate) fn finish_interactive_handoff(
        &self,
        session_id: &InteractiveHandoffId,
        outcome: &InteractiveHandoffOutcome,
    ) -> StoreResult<InteractiveHandoff> {
        outcome.validate().map_err(invalid_handoff)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = get_handoff(&transaction, session_id)?.ok_or(StoreError::NotFound)?;
        if let Some(stored_outcome) = &existing.outcome {
            if stored_outcome == outcome {
                transaction.commit()?;
                return Ok(existing);
            }
            return Err(StoreError::InvalidData(format!(
                "interactive handoff {session_id} already finished as {}",
                existing.status.as_str()
            )));
        }
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let changed = transaction.execute(
            "UPDATE interactive_handoffs
                SET status=?2, outcome_json=?3, terminal_at=?4, updated_at=?4
              WHERE id=?1 AND terminal_at IS NULL",
            params![
                session_id.as_str(),
                outcome.status().as_str(),
                serde_json::to_string(outcome)?,
                now,
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidData(format!(
                "interactive handoff {session_id} changed while finishing"
            )));
        }
        let handoff = get_handoff(&transaction, session_id)?.ok_or(StoreError::NotFound)?;
        transaction.commit()?;
        Ok(handoff)
    }

    pub(crate) fn claim_interactive_handoff_wake(
        &self,
        session_id: &InteractiveHandoffId,
        parent_generation: u32,
    ) -> StoreResult<bool> {
        if parent_generation == 0 {
            return Err(StoreError::InvalidData(
                "parent generation must be positive".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let handoff = get_handoff(&transaction, session_id)?.ok_or(StoreError::NotFound)?;
        if !handoff.status.is_terminal() {
            return Err(StoreError::InvalidData(format!(
                "interactive handoff {session_id} is not terminal"
            )));
        }
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let changed = transaction.execute(
            "UPDATE interactive_handoffs
                SET wake_claimed_at=?2, wake_claimed_by_generation=?3, updated_at=?2
              WHERE id=?1 AND terminal_at IS NOT NULL AND wake_claimed_at IS NULL",
            params![session_id.as_str(), now, i64::from(parent_generation)],
        )?;
        transaction.commit()?;
        Ok(changed == 1)
    }
}

fn active_for_parent(
    transaction: &Transaction<'_>,
    parent: &InteractiveHandoffParent,
) -> StoreResult<Option<InteractiveHandoff>> {
    let query =
        format!("{HANDOFF_COLUMNS} WHERE parent_kind=?1 AND parent_id=?2 AND terminal_at IS NULL");
    transaction
        .query_row(&query, params![parent.kind(), parent.id()], map_handoff_row)
        .optional()
        .map_err(StoreError::from)
}

fn validate_parent(
    transaction: &Transaction<'_>,
    request: &OpenInteractiveHandoff,
) -> StoreResult<WaveId> {
    match &request.parent {
        InteractiveHandoffParent::Wave(wave_id) => {
            let found = transaction
                .query_row(
                    "SELECT id FROM waves WHERE id=?1",
                    [wave_id.as_str()],
                    |row| row.get::<_, WaveId>(0),
                )
                .optional()?;
            found.ok_or(StoreError::NotFound)
        }
        InteractiveHandoffParent::Project(session_id) => {
            let parent = transaction
                .query_row(
                    "SELECT wave_id, status, process_generation,
                            COALESCE(process_provider, provider),
                            COALESCE(process_provider_session_id, provider_session_id)
                       FROM project_sessions WHERE id=?1",
                    [session_id.as_str()],
                    |row| {
                        Ok((
                            row.get::<_, WaveId>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<i64>>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, Option<String>>(4)?,
                        ))
                    },
                )
                .optional()?
                .ok_or(StoreError::NotFound)?;
            let status = parent
                .1
                .parse::<ProjectSessionStatus>()
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            validate_body_reference(
                "Project",
                session_id.as_str(),
                status.is_terminal(),
                parent.2,
                &parent.3,
                parent.4.as_deref(),
                None,
                request,
            )?;
            Ok(parent.0)
        }
        InteractiveHandoffParent::Task(session_id) => {
            let parent = transaction
                .query_row(
                    "SELECT wave_id, status, process_generation,
                            COALESCE(process_provider, provider),
                            COALESCE(process_provider_session_id, provider_session_id),
                            worktree
                       FROM task_sessions WHERE id=?1",
                    [session_id.as_str()],
                    |row| {
                        Ok((
                            row.get::<_, WaveId>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<i64>>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, Option<String>>(4)?,
                            row.get::<_, String>(5)?,
                        ))
                    },
                )
                .optional()?
                .ok_or(StoreError::NotFound)?;
            let status = parent
                .1
                .parse::<TaskSessionStatus>()
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            validate_body_reference(
                "Task",
                session_id.as_str(),
                status.is_terminal(),
                parent.2,
                &parent.3,
                parent.4.as_deref(),
                Some(&parent.5),
                request,
            )?;
            Ok(parent.0)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_body_reference(
    kind: &str,
    parent_id: &str,
    terminal: bool,
    generation: Option<i64>,
    provider: &str,
    provider_session_id: Option<&str>,
    cwd: Option<&str>,
    request: &OpenInteractiveHandoff,
) -> StoreResult<()> {
    if terminal {
        return Err(StoreError::InvalidData(format!(
            "terminal {kind} Session {parent_id} cannot open an interactive handoff"
        )));
    }
    if generation != Some(i64::from(request.body_generation)) {
        return Err(StoreError::InvalidData(format!(
            "{kind} Session {parent_id} does not own body generation {}",
            request.body_generation
        )));
    }
    if provider != request.provider {
        return Err(StoreError::InvalidData(format!(
            "{kind} Session {parent_id} body belongs to provider {provider}, not {}",
            request.provider
        )));
    }
    if let Some(provider_session_id) = provider_session_id {
        if request.provider_session_id.as_deref() != Some(provider_session_id) {
            return Err(StoreError::InvalidData(format!(
                "{kind} Session {parent_id} handoff does not preserve provider session history"
            )));
        }
    }
    if cwd.is_some_and(|cwd| Path::new(cwd) != request.cwd) {
        return Err(StoreError::InvalidData(format!(
            "{kind} Session {parent_id} handoff cwd does not match its worktree"
        )));
    }
    Ok(())
}

fn get_handoff(
    conn: &rusqlite::Connection,
    session_id: &InteractiveHandoffId,
) -> StoreResult<Option<InteractiveHandoff>> {
    let query = format!("{HANDOFF_COLUMNS} WHERE id=?1");
    conn.query_row(&query, [session_id.as_str()], map_handoff_row)
        .optional()
        .map_err(StoreError::from)
}

fn map_handoff_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<InteractiveHandoff> {
    let parent_kind = row.get::<_, String>(1)?;
    let parent_id = row.get::<_, String>(2)?;
    let parent = InteractiveHandoffParent::parse(&format!("{parent_kind}:{parent_id}"))
        .map_err(|error| invalid_column(2, error))?;
    let home = row
        .get::<_, String>(4)?
        .parse()
        .map_err(|error: String| invalid_column(4, std::io::Error::other(error)))?;
    let environment = serde_json::from_str::<BTreeMap<String, String>>(&row.get::<_, String>(10)?)
        .map_err(|error| invalid_column(10, error))?;
    let attach_argv = serde_json::from_str::<Vec<String>>(&row.get::<_, String>(11)?)
        .map_err(|error| invalid_column(11, error))?;
    let status = row
        .get::<_, String>(12)?
        .parse()
        .map_err(|error| invalid_column(12, error))?;
    let outcome = row
        .get::<_, Option<String>>(13)?
        .map(|json| serde_json::from_str(&json))
        .transpose()
        .map_err(|error| invalid_column(13, error))?;
    let handoff = InteractiveHandoff {
        id: InteractiveHandoffId::parse(&row.get::<_, String>(0)?)
            .map_err(|error| invalid_column(0, error))?,
        parent,
        wave_id: row.get(3)?,
        home,
        cwd: PathBuf::from(row.get::<_, String>(5)?),
        provider: row.get(6)?,
        provider_session_id: row.get(7)?,
        body_generation: row.get::<_, i64>(8)? as u32,
        reason: row.get(9)?,
        environment,
        attach_argv,
        status,
        outcome,
        created_at: unix_to_datetime(row.get(14)?),
        updated_at: unix_to_datetime(row.get(15)?),
        attached_at: row.get::<_, Option<i64>>(16)?.map(unix_to_datetime),
        terminal_at: row.get::<_, Option<i64>>(17)?.map(unix_to_datetime),
        wake_claimed_at: row.get::<_, Option<i64>>(18)?.map(unix_to_datetime),
        wake_claimed_by_generation: row.get::<_, Option<i64>>(19)?.map(|value| value as u32),
    };
    handoff
        .validate()
        .map_err(|error| invalid_column(0, error))?;
    Ok(handoff)
}

fn invalid_handoff(error: InteractiveHandoffDataError) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

fn invalid_column(
    index: usize,
    error: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(error))
}
