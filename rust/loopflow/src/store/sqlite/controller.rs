use rusqlite::{params, OptionalExtension, TransactionBehavior};
use time::OffsetDateTime;

use crate::child::{ChildBodyHandoffRequest, ChildRef};
use crate::controller::{project, task};
use crate::durable::Author;
use crate::store::{StoreError, StoreResult};
use crate::work::project::ProjectId;
use crate::work::task::TaskId;

use super::SqliteStore;

impl SqliteStore {
    pub(crate) fn task_controller_state(
        &self,
        task_id: &TaskId,
    ) -> StoreResult<Option<task::State>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT task_id, kickoff_flow, iterate_flow, gate_flow, lifecycle_phase,
                    phase_cursor, phase_iteration, gate_cycle, gate_proposal_json,
                    agent, provider, provider_session_id, updated_at
             FROM task_controller_state WHERE task_id=?1",
            params![task_id.as_str()],
            map_task_state,
        )
        .optional()
        .map_err(StoreError::from)
    }

    pub(crate) fn put_task_controller_state(&self, state: &task::State) -> StoreResult<()> {
        state
            .validate()
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        put_task_state_on(&conn, state)
    }

    pub(crate) fn project_controller_state(
        &self,
        project_id: &ProjectId,
    ) -> StoreResult<Option<project::State>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT project_id, iteration, observation_cursor, last_state_fingerprint,
                    agent, provider, provider_session_id, updated_at
             FROM project_controller_state WHERE project_id=?1",
            params![project_id.as_str()],
            |row| {
                Ok(project::State {
                    project_id: ProjectId::from_raw(row.get::<_, String>(0)?),
                    iteration: row.get::<_, i64>(1)? as u32,
                    observation_cursor: row.get(2)?,
                    last_state_fingerprint: row.get(3)?,
                    agent: row.get(4)?,
                    provider: row.get(5)?,
                    provider_session_id: row.get(6)?,
                    updated_at: crate::store::rows::unix_to_datetime(row.get(7)?),
                })
            },
        )
        .optional()
        .map_err(StoreError::from)
    }

    pub(crate) fn put_project_controller_state(&self, state: &project::State) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        put_project_state_on(&conn, state)
    }

    pub(crate) fn handoff_task_controller(
        &self,
        task_id: &TaskId,
        request: &ChildBodyHandoffRequest,
    ) -> StoreResult<task::State> {
        super::children::validate_handoff_request(request)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let work =
            super::durable::work_for_child_in(&transaction, &ChildRef::Task(task_id.clone()))?;
        let task_work = transaction
            .query_row(
                super::children::TASK_SELECT,
                params![task_id.as_str()],
                super::children::map_task_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        super::children::validate_handoff_state(
            "Task controller",
            &task_work.plan.identifier,
            &super::durable::work_status_in(&transaction, &work)?,
            task_work.abandon_intent.as_ref(),
        )?;
        let mut state = transaction
            .query_row(
                "SELECT task_id, kickoff_flow, iterate_flow, gate_flow, lifecycle_phase,
                        phase_cursor, phase_iteration, gate_cycle, gate_proposal_json,
                        agent, provider, provider_session_id, updated_at
                 FROM task_controller_state WHERE task_id=?1",
                params![task_id.as_str()],
                map_task_state,
            )
            .optional()?
            .ok_or_else(|| {
                StoreError::InvalidData(format!("Task {task_id} has no controller state"))
            })?;
        let handoff = super::children::apply_handoff(
            &mut state.agent,
            &mut state.provider,
            &mut state.provider_session_id,
            request,
        );
        state.updated_at = OffsetDateTime::now_utc();
        put_task_state_on(&transaction, &state)?;
        super::children::insert_task_event_in(
            &transaction,
            &task_work,
            &crate::work::task::TaskEventKind::BodyHandedOff { handoff },
        )?;
        transaction.commit()?;
        Ok(state)
    }

    pub(crate) fn restart_task_controller(
        &self,
        state: &task::State,
        author: &Author,
        direction: &str,
        checkpoint_head: &str,
    ) -> StoreResult<()> {
        state
            .validate()
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        if direction.trim().is_empty() || checkpoint_head.trim().is_empty() {
            return Err(StoreError::InvalidData(
                "Task controller restart requires direction and a checkpoint head".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task_work = transaction
            .query_row(
                super::children::TASK_SELECT,
                params![state.task_id.as_str()],
                super::children::map_task_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        let work = super::durable::work_for_child_in(
            &transaction,
            &ChildRef::Task(state.task_id.clone()),
        )?;
        transaction.execute(
            "DELETE FROM work_flow_positions WHERE work_kind=?1 AND work_id=?2",
            params![work.kind(), work.id()],
        )?;
        put_task_state_on(&transaction, state)?;
        SqliteStore::append_steer_in(&transaction, &work, author, direction)?;
        super::children::insert_task_event_in(
            &transaction,
            &task_work,
            &crate::work::task::TaskEventKind::Progress {
                summary: format!("Controller restarted from checkpoint {checkpoint_head}"),
            },
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn handoff_project_controller(
        &self,
        project_id: &ProjectId,
        request: &ChildBodyHandoffRequest,
    ) -> StoreResult<project::State> {
        super::children::validate_handoff_request(request)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let work = super::durable::work_for_child_in(
            &transaction,
            &ChildRef::Project(project_id.clone()),
        )?;
        let project_work = transaction
            .query_row(
                super::children::PROJECT_SELECT,
                params![project_id.as_str()],
                super::children::map_project_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        super::children::validate_handoff_state(
            "Project controller",
            &project_work.plan.slug,
            &super::durable::work_status_in(&transaction, &work)?,
            project_work.abandon_intent.as_ref(),
        )?;
        let mut state = transaction
            .query_row(
                "SELECT project_id, iteration, observation_cursor, last_state_fingerprint,
                        agent, provider, provider_session_id, updated_at
                 FROM project_controller_state WHERE project_id=?1",
                params![project_id.as_str()],
                map_project_state,
            )
            .optional()?
            .ok_or_else(|| {
                StoreError::InvalidData(format!("Project {project_id} has no controller state"))
            })?;
        let handoff = super::children::apply_handoff(
            &mut state.agent,
            &mut state.provider,
            &mut state.provider_session_id,
            request,
        );
        state.updated_at = OffsetDateTime::now_utc();
        put_project_state_on(&transaction, &state)?;
        super::children::insert_project_event_in(
            &transaction,
            &project_work,
            &crate::work::project::ProjectEventKind::BodyHandedOff { handoff },
        )?;
        transaction.commit()?;
        Ok(state)
    }
}

fn put_task_state_on(conn: &rusqlite::Connection, state: &task::State) -> StoreResult<()> {
    let gate_proposal = state.gate_proposal.as_ref().map(|proposal| {
        serde_json::to_string(proposal).expect("Task gate proposal must serialize")
    });
    conn.execute(
        "INSERT INTO task_controller_state (
            task_id, kickoff_flow, iterate_flow, gate_flow, lifecycle_phase,
            phase_cursor, phase_iteration, gate_cycle, gate_proposal_json,
            agent, provider, provider_session_id, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(task_id) DO UPDATE SET
            kickoff_flow=excluded.kickoff_flow,
            iterate_flow=excluded.iterate_flow,
            gate_flow=excluded.gate_flow,
            lifecycle_phase=excluded.lifecycle_phase,
            phase_cursor=excluded.phase_cursor,
            phase_iteration=excluded.phase_iteration,
            gate_cycle=excluded.gate_cycle,
            gate_proposal_json=excluded.gate_proposal_json,
            agent=excluded.agent,
            provider=excluded.provider,
            provider_session_id=excluded.provider_session_id,
            updated_at=excluded.updated_at",
        params![
            state.task_id.as_str(),
            state.lifecycle.first.flow,
            state.lifecycle.loop_.flow,
            state.lifecycle.finally.flow,
            state.lifecycle_phase.storage_str(),
            state.phase_cursor,
            state.phase_iteration,
            state.gate_cycle,
            gate_proposal,
            state.agent,
            state.provider,
            state.provider_session_id,
            state.updated_at.unix_timestamp(),
        ],
    )?;
    Ok(())
}

fn put_project_state_on(conn: &rusqlite::Connection, state: &project::State) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO project_controller_state (
            project_id, iteration, observation_cursor, last_state_fingerprint,
            agent, provider, provider_session_id, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(project_id) DO UPDATE SET
            iteration=excluded.iteration,
            observation_cursor=excluded.observation_cursor,
            last_state_fingerprint=excluded.last_state_fingerprint,
            agent=excluded.agent,
            provider=excluded.provider,
            provider_session_id=excluded.provider_session_id,
            updated_at=excluded.updated_at",
        params![
            state.project_id.as_str(),
            state.iteration,
            state.observation_cursor,
            state.last_state_fingerprint,
            state.agent,
            state.provider,
            state.provider_session_id,
            state.updated_at.unix_timestamp(),
        ],
    )?;
    Ok(())
}

fn map_task_state(row: &rusqlite::Row<'_>) -> rusqlite::Result<task::State> {
    Ok(task::State {
        task_id: TaskId::from_raw(row.get::<_, String>(0)?),
        lifecycle: task::TaskLifecyclePlan {
            first: task::TaskPhasePlan { flow: row.get(1)? },
            loop_: task::TaskPhasePlan { flow: row.get(2)? },
            finally: task::TaskPhasePlan { flow: row.get(3)? },
        },
        lifecycle_phase: task::TaskLifecyclePhase::from_storage_str(&row.get::<_, String>(4)?)
            .map_err(|error| invalid_column(4, error))?,
        phase_cursor: row.get::<_, i64>(5)? as u32,
        phase_iteration: row.get::<_, i64>(6)? as u32,
        gate_cycle: row.get::<_, i64>(7)? as u32,
        gate_proposal: row
            .get::<_, Option<String>>(8)?
            .map(|json| serde_json::from_str(&json))
            .transpose()
            .map_err(|error| invalid_column(8, error))?,
        agent: row.get(9)?,
        provider: row.get(10)?,
        provider_session_id: row.get(11)?,
        updated_at: crate::store::rows::unix_to_datetime(row.get(12)?),
    })
}

fn map_project_state(row: &rusqlite::Row<'_>) -> rusqlite::Result<project::State> {
    Ok(project::State {
        project_id: ProjectId::from_raw(row.get::<_, String>(0)?),
        iteration: row.get::<_, i64>(1)? as u32,
        observation_cursor: row.get(2)?,
        last_state_fingerprint: row.get(3)?,
        agent: row.get(4)?,
        provider: row.get(5)?,
        provider_session_id: row.get(6)?,
        updated_at: crate::store::rows::unix_to_datetime(row.get(7)?),
    })
}

fn invalid_column(
    index: usize,
    error: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(error))
}
