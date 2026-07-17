use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use time::OffsetDateTime;

use crate::child_session::{ChildRef, ChildWriteLease};
use crate::durable::{
    Author, Basis, BoundarySeed, Epoch, EpochId, EpochState, ProjectId, RunId, RunTrigger, Send,
    SendId, SendState, SendVia, Steer, SteerId, SteerReceipt, TaskId, ToolResponseId,
    ToolResponseReceipt, ToolResponseWrite, WorkRef,
};
use crate::id::WaveId;
use crate::project_session::ProjectSession;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::task::TaskSession;

use super::child_sessions::require_child_write_lease;
use super::SqliteStore;

impl SqliteStore {
    pub fn work_for_child(&self, target: &ChildRef) -> StoreResult<WorkRef> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        work_for_child_in(&conn, target)
    }

    pub fn current_epoch(&self, work: &WorkRef) -> StoreResult<Epoch> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        current_epoch_in(&conn, work)
    }

    pub fn boundary_seed(&self, work: &WorkRef) -> StoreResult<BoundarySeed> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        boundary_seed_in(&conn, work)
    }

    pub fn append_steer(
        &self,
        work: &WorkRef,
        author: &Author,
        text: &str,
        if_basis: Option<&Basis>,
    ) -> StoreResult<SteerReceipt> {
        let text = text.trim();
        if text.is_empty() {
            return Err(StoreError::InvalidData(
                "Steer text cannot be empty".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let epoch = current_epoch_in(&tx, work)?;
        if let Some(expected) = if_basis {
            validate_basis(&epoch.current_basis, expected)?;
        }
        let receipt = Self::append_steer_in(&tx, work, author, text)?;
        tx.commit()?;
        Ok(receipt)
    }

    pub fn steer(
        &self,
        context: &crate::durable::ControlCtx<'_>,
        work: &WorkRef,
        text: &str,
        if_basis: Option<&Basis>,
    ) -> StoreResult<SteerReceipt> {
        let author = match context {
            crate::durable::ControlCtx::User(_) => Author::User,
            crate::durable::ControlCtx::Run(lease) => Author::Run(lease.run_id.clone()),
        };
        self.append_steer(work, &author, text, if_basis)
    }

    pub(crate) fn append_steer_in(
        tx: &Transaction<'_>,
        work: &WorkRef,
        author: &Author,
        text: &str,
    ) -> StoreResult<SteerReceipt> {
        let text = text.trim();
        if text.is_empty() {
            return Err(StoreError::InvalidData(
                "Steer text cannot be empty".to_string(),
            ));
        }
        let epoch = current_epoch_in(tx, work)?;
        validate_author(tx, work, author)?;
        let revision = epoch.current_basis.revision + 1;
        let steer = Steer {
            id: SteerId::new(),
            work: work.clone(),
            basis: Basis {
                epoch_id: epoch.id.clone(),
                revision,
            },
            author: author.clone(),
            text: text.to_string(),
            issued_at: OffsetDateTime::now_utc(),
        };
        tx.execute(
            "INSERT INTO epoch_revisions (epoch_id, rev, kind, source_id, created_at)
             VALUES (?1, ?2, 'steer', ?3, ?4)",
            params![
                steer.basis.epoch_id.as_str(),
                steer.basis.revision as i64,
                steer.id.as_str(),
                steer.issued_at.unix_timestamp(),
            ],
        )?;
        let (author_kind, author_run_id) = match &steer.author {
            Author::User => ("user", None),
            Author::Run(run_id) => ("run", Some(run_id.as_str())),
        };
        tx.execute(
            "INSERT INTO steers (
                id, epoch_id, rev, author_kind, author_run_id, text, issued_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                steer.id.as_str(),
                steer.basis.epoch_id.as_str(),
                steer.basis.revision as i64,
                author_kind,
                author_run_id,
                steer.text,
                steer.issued_at.unix_timestamp(),
            ],
        )?;
        tx.execute(
            "UPDATE epochs SET current_rev=?2 WHERE id=?1 AND state='open'",
            params![steer.basis.epoch_id.as_str(), revision as i64],
        )?;
        Ok(SteerReceipt {
            steer,
            sends: Vec::new(),
            applied_by: None,
        })
    }

    pub fn write_tool_response(
        &self,
        work: &WorkRef,
        write: &ToolResponseWrite,
        if_basis: Option<&Basis>,
    ) -> StoreResult<(ToolResponseReceipt, bool)> {
        let choice = write.choice.trim();
        if choice.is_empty() {
            return Err(StoreError::InvalidData(
                "tool response choice cannot be empty".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let epoch = current_epoch_in(&tx, work)?;
        if let Some(expected) = if_basis {
            validate_basis(&epoch.current_basis, expected)?;
        }
        if let Some(existing) = tool_response_in(&tx, &epoch.id, &write.request_id)? {
            if existing.choice != choice {
                return Err(StoreError::InvalidData(format!(
                    "tool response {} is already resolved as {:?}",
                    write.request_id, existing.choice
                )));
            }
            return Ok((existing, false));
        }
        let revision = epoch.current_basis.revision + 1;
        let receipt = ToolResponseReceipt {
            id: ToolResponseId::new(),
            work: work.clone(),
            basis: Basis {
                epoch_id: epoch.id.clone(),
                revision,
            },
            request_id: write.request_id.clone(),
            choice: choice.to_string(),
            responded_at: OffsetDateTime::now_utc(),
        };
        tx.execute(
            "INSERT INTO epoch_revisions (epoch_id, rev, kind, source_id, created_at)
             VALUES (?1, ?2, 'tool_response', ?3, ?4)",
            params![
                receipt.basis.epoch_id.as_str(),
                receipt.basis.revision as i64,
                receipt.id.as_str(),
                receipt.responded_at.unix_timestamp(),
            ],
        )?;
        tx.execute(
            "INSERT INTO tool_responses (id, epoch_id, rev, request_id, choice, responded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                receipt.id.as_str(),
                receipt.basis.epoch_id.as_str(),
                receipt.basis.revision as i64,
                receipt.request_id,
                receipt.choice,
                receipt.responded_at.unix_timestamp(),
            ],
        )?;
        tx.execute(
            "UPDATE epochs SET current_rev=?2 WHERE id=?1 AND state='open'",
            params![receipt.basis.epoch_id.as_str(), revision as i64],
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
        let epoch = current_epoch_in(&conn, work)?;
        tool_response_in(&conn, &epoch.id, request_id)
    }

    pub fn begin_live_send(&self, steer_id: &SteerId, turn_id: &str) -> StoreResult<Option<Send>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = send_for(&tx, steer_id, turn_id, SendVia::Live)? {
            return Ok(Some(existing));
        }
        let eligible: bool = tx.query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM steers s
                JOIN agent_turns t ON t.id=?2
                WHERE s.id=?1
                  AND s.epoch_id=t.epoch_id
                  AND t.status='running'
                  AND s.rev > COALESCE((
                      SELECT MAX(done.basis_rev)
                      FROM agent_turns done
                      WHERE done.epoch_id=s.epoch_id AND done.status='completed'
                  ), -1)
             )",
            params![steer_id.as_str(), turn_id],
            |row| row.get(0),
        )?;
        if !eligible {
            tx.commit()?;
            return Ok(None);
        }
        let send = Send {
            id: SendId::new(),
            steer_id: steer_id.clone(),
            turn_id: turn_id.to_string(),
            via: SendVia::Live,
            state: SendState::Sending,
            provider_turn_id: None,
            reason: None,
            attempted_at: OffsetDateTime::now_utc(),
            finished_at: None,
        };
        insert_send(&tx, &send)?;
        tx.commit()?;
        Ok(Some(send))
    }

    pub fn finish_send(
        &self,
        send_id: &SendId,
        state: SendState,
        provider_turn_id: Option<&str>,
        reason: Option<&str>,
    ) -> StoreResult<Send> {
        if state == SendState::Sending {
            return Err(StoreError::InvalidData(
                "a Send cannot finish as sending".to_string(),
            ));
        }
        let now = OffsetDateTime::now_utc();
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE sends
             SET state=?2, provider_turn_id=?3, reason=?4, finished_at=?5
             WHERE id=?1 AND state='sending'",
            params![
                send_id.as_str(),
                state.as_str(),
                provider_turn_id,
                reason,
                now.unix_timestamp(),
            ],
        )?;
        if changed == 0 {
            return send_by_id(&conn, send_id)?.ok_or(StoreError::NotFound);
        }
        send_by_id(&conn, send_id)?.ok_or(StoreError::NotFound)
    }

    pub fn validate_completion_basis(&self, work: &WorkRef, proposed: &Basis) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let epoch = current_epoch_in(&conn, work)?;
        validate_basis(&epoch.current_basis, proposed)?;
        let applied = applied_basis_in(&conn, &epoch.id)?.ok_or_else(|| {
            StoreError::InvalidData("no successful root boundary can complete Work".to_string())
        })?;
        validate_basis(&applied, proposed)
    }

    pub(crate) fn run_for_child_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
    ) -> StoreResult<crate::durable::RunLease> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        require_child_write_lease(&conn, target, lease)?;
        let work = work_for_child_in(&conn, target)?;
        let epoch = current_epoch_in(&conn, &work)?;
        let run_id = conn
            .query_row(
                "SELECT id FROM runs
                 WHERE epoch_id=?1 AND lease_generation=?2 AND state IN ('reserved', 'active')
                 ORDER BY created_at DESC LIMIT 1",
                params![epoch.id.as_str(), i64::from(lease.generation)],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| {
                StoreError::InvalidData(format!(
                    "{} generation {} has no active Run",
                    target.target_id(),
                    lease.generation
                ))
            })?;
        Ok(crate::durable::RunLease::new(
            RunId::parse(&run_id).map_err(|error| {
                StoreError::InvalidData(format!("invalid stored Run id: {error}"))
            })?,
            work,
            epoch.current_basis,
            crate::durable::RunLeaseToken::from_child(lease.token.as_str()),
        ))
    }
}

pub(crate) fn create_project_spine(tx: &Connection, session: &ProjectSession) -> StoreResult<()> {
    let project_id = tx
        .query_row(
            "SELECT id FROM projects WHERE external_project_id=?1",
            [session.launch.project.id.as_str()],
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
            session.wave_id.as_str(),
            session.launch.project.id.as_str(),
            session.created_at.unix_timestamp(),
        ],
    )?;
    let epoch_id = EpochId::new();
    let number: i64 = tx.query_row(
        "SELECT COALESCE(MAX(number), 0) + 1 FROM epochs WHERE project_id=?1",
        [&project_id],
        |row| row.get(0),
    )?;
    let (state, terminal_at) = epoch_state_for_project(session);
    tx.execute(
        "INSERT INTO epochs (
            id, number, wave_id, project_id, task_id, state, current_rev,
            created_at, terminal_at
         ) VALUES (?1, ?2, NULL, ?3, NULL, ?4, 0, ?5, ?6)",
        params![
            epoch_id.as_str(),
            number,
            project_id,
            state.as_str(),
            session.created_at.unix_timestamp(),
            terminal_at,
        ],
    )?;
    insert_truth(
        tx,
        &epoch_id,
        serde_json::json!({
            "external_project_id": session.launch.project.id.as_str(),
            "slug": session.launch.project.slug,
            "name": session.launch.project.name,
            "prompt_context": session.launch.project.prompt_context,
            "pm_snapshot_synced_at": session.launch.pm_snapshot_synced_at,
        }),
        session.created_at,
    )?;
    tx.execute(
        "UPDATE project_sessions SET epoch_id=?2 WHERE id=?1",
        params![session.id.as_str(), epoch_id.as_str()],
    )?;
    Ok(())
}

pub(crate) fn create_task_spine(tx: &Connection, session: &TaskSession) -> StoreResult<()> {
    let project_id: String = tx.query_row(
        "SELECT id FROM projects WHERE external_project_id=?1",
        [session.launch.project.id.as_str()],
        |row| row.get(0),
    )?;
    let task_id = tx
        .query_row(
            "SELECT id FROM tasks WHERE external_issue_id=?1",
            [session.launch.issue.id.as_str()],
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
            session.launch.issue.id.as_str(),
            session.launch.issue.identifier,
            session.created_at.unix_timestamp(),
        ],
    )?;
    let epoch_id = EpochId::new();
    let number: i64 = tx.query_row(
        "SELECT COALESCE(MAX(number), 0) + 1 FROM epochs WHERE task_id=?1",
        [&task_id],
        |row| row.get(0),
    )?;
    let (state, terminal_at) = epoch_state_for_task(session);
    tx.execute(
        "INSERT INTO epochs (
            id, number, wave_id, project_id, task_id, state, current_rev,
            created_at, terminal_at
         ) VALUES (?1, ?2, NULL, NULL, ?3, ?4, 0, ?5, ?6)",
        params![
            epoch_id.as_str(),
            number,
            task_id,
            state.as_str(),
            session.created_at.unix_timestamp(),
            terminal_at,
        ],
    )?;
    insert_truth(
        tx,
        &epoch_id,
        serde_json::json!({
            "external_issue_id": session.launch.issue.id.as_str(),
            "identifier": session.launch.issue.identifier,
            "title": session.launch.issue.title,
            "description": session.launch.issue.description,
            "pm_snapshot_synced_at": session.launch.pm_snapshot_synced_at,
        }),
        session.created_at,
    )?;
    tx.execute(
        "UPDATE task_sessions SET epoch_id=?2 WHERE id=?1",
        params![session.id.as_str(), epoch_id.as_str()],
    )?;
    Ok(())
}

pub(crate) fn reserve_run_for_child(
    tx: &Transaction<'_>,
    target: &ChildRef,
    generation: u32,
    lease_token: &str,
) -> StoreResult<RunId> {
    let work = work_for_child_in(tx, target)?;
    let epoch = current_epoch_in(tx, &work)?;
    let home_id: String = tx.query_row(
        "SELECT id FROM homes ORDER BY created_at LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    let trigger_json = serde_json::to_string(&RunTrigger::Input {
        basis: epoch.current_basis.clone(),
    })
    .expect("run trigger must serialize");
    let lease_hash = crate::durable::RunLeaseToken::from_child(lease_token).hash();
    let run_id = RunId::new();
    tx.execute(
        "INSERT INTO runs (
            id, epoch_id, home_id, state, trigger_json, lease_hash,
            lease_generation, source_kind, source_id, created_at, ended_at
         ) VALUES (?1, ?2, ?3, 'reserved', ?4, ?5, ?6, ?7, ?8, ?9, NULL)",
        params![
            run_id.as_str(),
            epoch.id.as_str(),
            home_id,
            trigger_json,
            lease_hash,
            i64::from(generation),
            target.target_kind(),
            target.target_id(),
            now_unix(),
        ],
    )?;
    Ok(run_id)
}

pub(crate) fn activate_run_for_child(
    tx: &Transaction<'_>,
    target: &ChildRef,
    generation: u32,
) -> StoreResult<()> {
    let work = work_for_child_in(tx, target)?;
    let epoch = current_epoch_in(tx, &work)?;
    if tx.execute(
        "UPDATE runs SET state='active'
         WHERE epoch_id=?1 AND lease_generation=?2 AND state='reserved'",
        params![epoch.id.as_str(), i64::from(generation)],
    )? == 0
    {
        return Err(StoreError::InvalidData(format!(
            "{} generation {generation} has no reserved Run",
            target.target_id()
        )));
    }
    Ok(())
}

pub(crate) fn end_run_for_child(
    conn: &Connection,
    target: &ChildRef,
    generation: u32,
) -> StoreResult<()> {
    let work = work_for_child_in(conn, target)?;
    let epoch = current_epoch_in(conn, &work)?;
    conn.execute(
        "UPDATE runs SET state='ended', ended_at=?3
         WHERE epoch_id=?1 AND lease_generation=?2 AND state != 'ended'",
        params![epoch.id.as_str(), i64::from(generation), now_unix()],
    )?;
    Ok(())
}

pub(crate) fn fence_run_for_child(
    conn: &Connection,
    target: &ChildRef,
    generation: u32,
) -> StoreResult<()> {
    let work = work_for_child_in(conn, target)?;
    let epoch = current_epoch_in(conn, &work)?;
    if conn.execute(
        "UPDATE runs SET state='stopping'
         WHERE epoch_id=?1 AND lease_generation=?2
           AND state IN ('reserved', 'active')",
        params![epoch.id.as_str(), i64::from(generation)],
    )? == 0
    {
        return Err(StoreError::InvalidData(format!(
            "{} generation {generation} has no Run to stop",
            target.target_id()
        )));
    }
    Ok(())
}

pub(crate) fn insert_seed_sends_for_turn(
    tx: &Transaction<'_>,
    turn_id: &str,
    basis: &Basis,
) -> StoreResult<()> {
    let current: i64 = tx.query_row(
        "SELECT current_rev FROM epochs WHERE id=?1 AND state='open'",
        [basis.epoch_id.as_str()],
        |row| row.get(0),
    )?;
    if current != basis.revision as i64 {
        return Err(StoreError::StaleBasis {
            expected: format!("{}:{}", basis.epoch_id, basis.revision),
            current: format!("{}:{current}", basis.epoch_id),
        });
    }
    let applied: i64 = tx.query_row(
        "SELECT COALESCE(MAX(basis_rev), -1)
         FROM agent_turns WHERE epoch_id=?1 AND status='completed'",
        [basis.epoch_id.as_str()],
        |row| row.get(0),
    )?;
    let mut statement = tx.prepare(
        "SELECT id FROM steers
         WHERE epoch_id=?1 AND rev > ?2 AND rev <= ?3
         ORDER BY rev",
    )?;
    let steer_ids = statement
        .query_map(
            params![basis.epoch_id.as_str(), applied, basis.revision as i64],
            |row| row.get::<_, String>(0),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    let now = now_unix();
    for steer_id in steer_ids {
        tx.execute(
            "INSERT INTO sends (
                id, steer_id, turn_id, via, state, provider_turn_id,
                reason, attempted_at, finished_at
             ) VALUES (?1, ?2, ?3, 'seed', 'sent', NULL, NULL, ?4, ?4)",
            params![SendId::new().as_str(), steer_id, turn_id, now],
        )?;
    }
    Ok(())
}

fn insert_truth(
    tx: &Connection,
    epoch_id: &EpochId,
    payload: serde_json::Value,
    at: OffsetDateTime,
) -> StoreResult<()> {
    let source_id = format!("truth:{}:0", epoch_id);
    tx.execute(
        "INSERT INTO epoch_revisions (epoch_id, rev, kind, source_id, created_at)
         VALUES (?1, 0, 'truth', ?2, ?3)",
        params![epoch_id.as_str(), source_id, at.unix_timestamp()],
    )?;
    tx.execute(
        "INSERT INTO work_truth (epoch_id, rev, payload_json, created_at)
         VALUES (?1, 0, ?2, ?3)",
        params![epoch_id.as_str(), payload.to_string(), at.unix_timestamp()],
    )?;
    Ok(())
}

fn epoch_state_for_project(session: &ProjectSession) -> (EpochState, Option<i64>) {
    use crate::project_session::ProjectSessionStatus;
    match session.status {
        ProjectSessionStatus::Completed => {
            (EpochState::Done, Some(session.updated_at.unix_timestamp()))
        }
        ProjectSessionStatus::Abandoned => (
            EpochState::Abandoned,
            Some(session.updated_at.unix_timestamp()),
        ),
        _ => (EpochState::Open, None),
    }
}

fn epoch_state_for_task(session: &TaskSession) -> (EpochState, Option<i64>) {
    use crate::task::TaskSessionStatus;
    match session.status {
        TaskSessionStatus::Completed => {
            (EpochState::Done, Some(session.updated_at.unix_timestamp()))
        }
        TaskSessionStatus::Abandoned => (
            EpochState::Abandoned,
            Some(session.updated_at.unix_timestamp()),
        ),
        _ => (EpochState::Open, None),
    }
}

fn validate_basis(current: &Basis, expected: &Basis) -> StoreResult<()> {
    if current == expected {
        return Ok(());
    }
    Err(StoreError::StaleBasis {
        expected: format!("{}:{}", expected.epoch_id, expected.revision),
        current: format!("{}:{}", current.epoch_id, current.revision),
    })
}

fn validate_author(tx: &Transaction<'_>, target: &WorkRef, author: &Author) -> StoreResult<()> {
    let Author::Run(run_id) = author else {
        return Ok(());
    };
    let source = tx
        .query_row(
            "SELECT e.wave_id, e.project_id, e.task_id
             FROM runs r JOIN epochs e ON e.id=r.epoch_id
             WHERE r.id=?1 AND r.state='active'",
            [run_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::InvalidAuthority("Run lease is not active".to_string()))?;
    let allowed = match target {
        WorkRef::Project(project_id) => {
            let parent: String = tx.query_row(
                "SELECT wave_id FROM projects WHERE id=?1",
                [project_id.as_str()],
                |row| row.get(0),
            )?;
            source.0.as_deref() == Some(parent.as_str())
        }
        WorkRef::Task(task_id) => {
            let parent: String = tx.query_row(
                "SELECT project_id FROM tasks WHERE id=?1",
                [task_id.as_str()],
                |row| row.get(0),
            )?;
            source.1.as_deref() == Some(parent.as_str())
        }
        WorkRef::Wave(_) => false,
    };
    if allowed {
        Ok(())
    } else {
        Err(StoreError::InvalidAuthority(format!(
            "Run {run_id} may steer only immediate child Work"
        )))
    }
}

pub(crate) fn work_for_child_in(conn: &Connection, target: &ChildRef) -> StoreResult<WorkRef> {
    match target {
        ChildRef::Project(session_id) => {
            let id: String = conn.query_row(
                "SELECT e.project_id
                 FROM project_sessions s JOIN epochs e ON e.id=s.epoch_id
                 WHERE s.id=?1",
                [session_id.as_str()],
                |row| row.get(0),
            )?;
            Ok(WorkRef::Project(ProjectId::parse(&id).map_err(
                |error| StoreError::InvalidData(format!("invalid stored Project id: {error}")),
            )?))
        }
        ChildRef::Task(session_id) => {
            let id: String = conn.query_row(
                "SELECT e.task_id
                 FROM task_sessions s JOIN epochs e ON e.id=s.epoch_id
                 WHERE s.id=?1",
                [session_id.as_str()],
                |row| row.get(0),
            )?;
            Ok(WorkRef::Task(TaskId::parse(&id).map_err(|error| {
                StoreError::InvalidData(format!("invalid stored Task id: {error}"))
            })?))
        }
    }
}

fn current_epoch_in(conn: &Connection, work: &WorkRef) -> StoreResult<Epoch> {
    let (column, id) = match work {
        WorkRef::Wave(id) => ("wave_id", id.as_str()),
        WorkRef::Project(id) => ("project_id", id.as_str()),
        WorkRef::Task(id) => ("task_id", id.as_str()),
    };
    let sql = format!(
        "SELECT id, number, state, current_rev, created_at, terminal_at
         FROM epochs WHERE {column}=?1 AND state='open'"
    );
    conn.query_row(&sql, [id], |row| {
        let epoch_id = row.get::<_, String>(0)?;
        let state = row.get::<_, String>(2)?;
        Ok((
            epoch_id,
            row.get::<_, i64>(1)?,
            state,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, Option<i64>>(5)?,
        ))
    })
    .optional()?
    .map(
        |(epoch_id, number, state, revision, created_at, terminal_at)| -> StoreResult<Epoch> {
            let id = EpochId::parse(&epoch_id).map_err(|error| {
                StoreError::InvalidData(format!("invalid stored Epoch id: {error}"))
            })?;
            Ok(Epoch {
                id: id.clone(),
                work: work.clone(),
                number: number as u32,
                state: EpochState::parse(&state)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?,
                current_basis: Basis {
                    epoch_id: id,
                    revision: revision as u64,
                },
                created_at: OffsetDateTime::from_unix_timestamp(created_at).map_err(|error| {
                    StoreError::InvalidData(format!("invalid Epoch timestamp: {error}"))
                })?,
                terminal_at: terminal_at
                    .map(OffsetDateTime::from_unix_timestamp)
                    .transpose()
                    .map_err(|error| {
                        StoreError::InvalidData(format!(
                            "invalid Epoch terminal timestamp: {error}"
                        ))
                    })?,
            })
        },
    )
    .transpose()?
    .ok_or(StoreError::NotFound)
}

fn applied_basis_in(conn: &Connection, epoch_id: &EpochId) -> StoreResult<Option<Basis>> {
    let revision = conn.query_row(
        "SELECT MAX(basis_rev) FROM agent_turns
         WHERE epoch_id=?1 AND status='completed'",
        [epoch_id.as_str()],
        |row| row.get::<_, Option<i64>>(0),
    )?;
    Ok(revision.map(|revision| Basis {
        epoch_id: epoch_id.clone(),
        revision: revision as u64,
    }))
}

fn boundary_seed_in(conn: &Connection, work: &WorkRef) -> StoreResult<BoundarySeed> {
    let epoch = current_epoch_in(conn, work)?;
    let applied = applied_basis_in(conn, &epoch.id)?
        .map(|basis| basis.revision as i64)
        .unwrap_or(-1);
    let mut statement = conn.prepare(
        "SELECT id, rev, author_kind, author_run_id, text, issued_at
         FROM steers WHERE epoch_id=?1 AND rev > ?2 AND rev <= ?3 ORDER BY rev",
    )?;
    let rows = statement.query_map(
        params![
            epoch.id.as_str(),
            applied,
            epoch.current_basis.revision as i64
        ],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
            ))
        },
    )?;
    let mut steers = Vec::new();
    for row in rows {
        let (id, revision, author_kind, author_run_id, text, issued_at) = row?;
        let author = match (author_kind.as_str(), author_run_id) {
            ("user", None) => Author::User,
            ("run", Some(id)) => Author::Run(RunId::parse(&id).map_err(|error| {
                StoreError::InvalidData(format!("invalid stored Run id: {error}"))
            })?),
            _ => {
                return Err(StoreError::InvalidData(
                    "stored Steer author is inconsistent".to_string(),
                ))
            }
        };
        steers.push(Steer {
            id: SteerId::parse(&id).map_err(|error| {
                StoreError::InvalidData(format!("invalid stored Steer id: {error}"))
            })?,
            work: work.clone(),
            basis: Basis {
                epoch_id: epoch.id.clone(),
                revision: revision as u64,
            },
            author,
            text,
            issued_at: OffsetDateTime::from_unix_timestamp(issued_at).map_err(|error| {
                StoreError::InvalidData(format!("invalid Steer timestamp: {error}"))
            })?,
        });
    }
    Ok(BoundarySeed {
        basis: epoch.current_basis,
        steers,
    })
}

fn tool_response_in(
    conn: &Connection,
    epoch_id: &EpochId,
    request_id: &str,
) -> StoreResult<Option<ToolResponseReceipt>> {
    let row = conn
        .query_row(
            "SELECT id, rev, choice, responded_at FROM tool_responses
             WHERE epoch_id=?1 AND request_id=?2",
            params![epoch_id.as_str(), request_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((id, revision, choice, responded_at)) = row else {
        return Ok(None);
    };
    let work = work_for_epoch(conn, epoch_id)?;
    Ok(Some(ToolResponseReceipt {
        id: ToolResponseId::parse(&id).map_err(|error| {
            StoreError::InvalidData(format!("invalid stored ToolResponse id: {error}"))
        })?,
        work,
        basis: Basis {
            epoch_id: epoch_id.clone(),
            revision: revision as u64,
        },
        request_id: request_id.to_string(),
        choice,
        responded_at: OffsetDateTime::from_unix_timestamp(responded_at).map_err(|error| {
            StoreError::InvalidData(format!("invalid Decision timestamp: {error}"))
        })?,
    }))
}

fn work_for_epoch(conn: &Connection, epoch_id: &EpochId) -> StoreResult<WorkRef> {
    let row = conn.query_row(
        "SELECT wave_id, project_id, task_id FROM epochs WHERE id=?1",
        [epoch_id.as_str()],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        },
    )?;
    match row {
        (Some(id), None, None) => Ok(WorkRef::Wave(WaveId::parse(&id).map_err(|error| {
            StoreError::InvalidData(format!("invalid stored Wave id: {error}"))
        })?)),
        (None, Some(id), None) => Ok(WorkRef::Project(ProjectId::parse(&id).map_err(
            |error| StoreError::InvalidData(format!("invalid stored Project id: {error}")),
        )?)),
        (None, None, Some(id)) => Ok(WorkRef::Task(TaskId::parse(&id).map_err(|error| {
            StoreError::InvalidData(format!("invalid stored Task id: {error}"))
        })?)),
        _ => Err(StoreError::InvalidData(
            "stored Epoch owns an invalid Work reference".to_string(),
        )),
    }
}

fn insert_send(conn: &Connection, send: &Send) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO sends (
            id, steer_id, turn_id, via, state, provider_turn_id, reason,
            attempted_at, finished_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            send.id.as_str(),
            send.steer_id.as_str(),
            send.turn_id.as_str(),
            send.via.as_str(),
            send.state.as_str(),
            send.provider_turn_id,
            send.reason,
            send.attempted_at.unix_timestamp(),
            send.finished_at.map(|at| at.unix_timestamp()),
        ],
    )?;
    Ok(())
}

fn send_for(
    conn: &Connection,
    steer_id: &SteerId,
    turn_id: &str,
    via: SendVia,
) -> StoreResult<Option<Send>> {
    conn.query_row(
        "SELECT id, steer_id, turn_id, via, state, provider_turn_id, reason,
                attempted_at, finished_at
         FROM sends WHERE steer_id=?1 AND turn_id=?2 AND via=?3",
        params![steer_id.as_str(), turn_id, via.as_str()],
        map_send,
    )
    .optional()
    .map_err(StoreError::from)
}

fn send_by_id(conn: &Connection, send_id: &SendId) -> StoreResult<Option<Send>> {
    conn.query_row(
        "SELECT id, steer_id, turn_id, via, state, provider_turn_id, reason,
                attempted_at, finished_at
         FROM sends WHERE id=?1",
        [send_id.as_str()],
        map_send,
    )
    .optional()
    .map_err(StoreError::from)
}

fn map_send(row: &rusqlite::Row<'_>) -> rusqlite::Result<Send> {
    let id = row.get::<_, String>(0)?;
    let steer_id = row.get::<_, String>(1)?;
    let via = row.get::<_, String>(3)?;
    let state = row.get::<_, String>(4)?;
    let attempted_at = row.get::<_, i64>(7)?;
    let finished_at = row.get::<_, Option<i64>>(8)?;
    Ok(Send {
        id: SendId::parse(&id).map_err(to_sql_error)?,
        steer_id: SteerId::parse(&steer_id).map_err(to_sql_error)?,
        turn_id: row.get(2)?,
        via: match via.as_str() {
            "live" => SendVia::Live,
            "seed" => SendVia::Seed,
            _ => return Err(to_sql_error(format!("invalid send via: {via}"))),
        },
        state: SendState::parse(&state).map_err(to_sql_error)?,
        provider_turn_id: row.get(5)?,
        reason: row.get(6)?,
        attempted_at: OffsetDateTime::from_unix_timestamp(attempted_at).map_err(to_sql_error)?,
        finished_at: finished_at
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()
            .map_err(to_sql_error)?,
    })
}

fn to_sql_error(error: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    )
}
