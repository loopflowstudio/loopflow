use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use time::OffsetDateTime;

use crate::child::ChildRef;
use crate::durable::{
    AdvanceReceipt, AgentInvocation, AgentInvocationId, Answer, AnswerAttemptHistory,
    AnswerContext, AnswerRoute, AskExchange, AskId, Author, Basis, BoundarySeed, BoundaryState,
    Containment, ContainmentObservation, DoneProposal, DoneProposalId, Epoch, EpochId,
    EpochReceipt, EpochState, FlowPosition, Home, HomeId, InterruptReceipt, InvocationRoute,
    InvocationSurface, Placement, ProjectId, Run, RunAdvance, RunId, RunLease, RunLeaseToken,
    RunState, RunTrigger, Send, SendId, SendState, SendVia, Steer, SteerId, SteerReceipt,
    StopCause, StopReceipt, TaskId, ToolResponseId, ToolResponseReceipt, ToolResponseWrite, Turn,
    TurnId, Wait, WaitId, WaitOn, WorkRef, WorkStatus,
};
use crate::id::WaveId;
use crate::project::Project;
use crate::store::durable::{AskCommentTransition, AskCommentWrite};
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::task::Task;

use super::SqliteStore;

const HAS_PENDING_USER_ASK_FOR_WORK_SQL: &str = "SELECT EXISTS(
        SELECT 1 FROM ask_exchanges a
        JOIN agent_turns t ON t.id=a.turn_id
        JOIN epochs e ON e.id=t.epoch_id
        WHERE a.route_kind='user' AND a.answered_at IS NULL
          AND e.state='open'
          AND (e.wave_id=?1 OR e.project_id=?2 OR e.task_id=?3)
          AND t.status NOT IN ('completed', 'interrupted')
     )";

impl SqliteStore {
    pub(crate) fn task_writer_state(
        &self,
        external_issue_id: &str,
    ) -> StoreResult<Option<crate::store::durable::TaskWriterState>> {
        let task = {
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.query_row(
                "SELECT id, issue_identifier FROM tasks WHERE external_issue_id=?1",
                [external_issue_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        };
        let Some((id, identifier)) = task else {
            return Ok(None);
        };
        let work = WorkRef::Task(TaskId::parse(&id).map_err(invalid_durable)?);
        let run = self.current_run(&work)?;
        Ok(Some(crate::store::durable::TaskWriterState {
            work,
            identifier,
            run,
        }))
    }

    pub fn home_by_id(&self, home_id: &HomeId) -> StoreResult<Option<Home>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        map_home_by_id(&conn, home_id)
    }

    pub fn local_home(&self) -> StoreResult<Home> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        map_local_home(&conn)
    }

    pub fn observe_home(&self, home_id: &HomeId, route: &str) -> StoreResult<Home> {
        let route = route.trim();
        if route.is_empty() {
            return Err(StoreError::InvalidData(
                "Home route cannot be empty".to_string(),
            ));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        if map_home_by_id(&conn, home_id)?
            .is_some_and(|home| home.route == "local" && route != "local")
        {
            return Err(StoreError::InvalidData(format!(
                "cannot replace local Home {home_id} with remote route {route:?}"
            )));
        }
        let existing_id = conn
            .query_row("SELECT id FROM homes WHERE route=?1", [route], |row| {
                row.get::<_, String>(0)
            })
            .optional()?;
        if existing_id
            .as_deref()
            .is_some_and(|id| id != home_id.as_str())
        {
            return Err(StoreError::InvalidData(format!(
                "Home route {route:?} is already observed for {}",
                existing_id.expect("checked as present")
            )));
        }
        let now = now_unix();
        conn.execute(
            "INSERT INTO homes (id, route, created_at, observed_at)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(id) DO UPDATE SET
                route=excluded.route, observed_at=excluded.observed_at",
            params![home_id.as_str(), route, now],
        )?;
        map_home_by_id(&conn, home_id)?.ok_or(StoreError::NotFound)
    }

    pub fn placement(&self, work: &WorkRef) -> StoreResult<Placement> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        placement_in(&conn, work)
    }

    pub(crate) fn place_work(&self, work: &WorkRef, home_id: &HomeId) -> StoreResult<Placement> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        current_epoch_in(&tx, work)?;
        tx.query_row(
            "SELECT 1 FROM homes WHERE id=?1",
            [home_id.as_str()],
            |_| Ok(()),
        )?;
        if let Some(current) = find_placement_in(&tx, work)? {
            if current.home_id == *home_id {
                tx.commit()?;
                return Ok(current);
            }
        }
        if let Some(run) = current_run_for_work_in(&tx, work)? {
            return Err(StoreError::InvalidData(format!(
                "cannot move {} {} while Run {} is {:?}",
                work.kind(),
                work.id(),
                run.id,
                run.state
            )));
        }
        write_placement(&tx, work, home_id, now_unix())?;
        let placement = placement_in(&tx, work)?;
        tx.commit()?;
        Ok(placement)
    }

    pub fn reserve_run(
        &self,
        work: &WorkRef,
        trigger: &RunTrigger,
    ) -> StoreResult<(Run, RunLease)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let epoch = current_epoch_in(&tx, work)?;
        let home_id = reserving_home_in(&tx, work)?;
        resolve_wait_for_trigger(&tx, &epoch, trigger)?;
        let token = RunLeaseToken::new();
        let run = Run {
            id: RunId::new(),
            work: work.clone(),
            epoch_id: epoch.id.clone(),
            home_id,
            state: RunState::Reserved,
            trigger: trigger.clone(),
            retry_of: match trigger {
                RunTrigger::Recovery { prior_run_id } => Some(prior_run_id.clone()),
                _ => None,
            },
            containment: None,
            cwd: None,
            created_at: OffsetDateTime::now_utc(),
            started_at: None,
            ended_at: None,
        };
        tx.execute(
            "INSERT INTO runs (
                id, epoch_id, home_id, state, trigger_json, retry_of, lease_hash,
                lease_generation, source_kind, source_id, created_at, ended_at, stop_reason
             ) VALUES (?1, ?2, ?3, 'reserved', ?4, ?5, ?6, NULL, ?7, ?8, ?9, NULL, NULL)",
            params![
                run.id.as_str(),
                run.epoch_id.as_str(),
                run.home_id.as_str(),
                serde_json::to_string(trigger).expect("Run trigger must serialize"),
                run.retry_of.as_ref().map(RunId::as_str),
                token.hash(),
                work.kind(),
                work.id(),
                run.created_at.unix_timestamp(),
            ],
        )?;
        let lease = RunLease::new(run.id.clone(), work.clone(), epoch.current_basis, token);
        tx.commit()?;
        Ok((run, lease))
    }

    pub(crate) fn reserve_recovery_run(&self, lease: &RunLease) -> StoreResult<(Run, RunLease)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let prior = validate_run_lease(&tx, lease)?;
        if prior.state != RunState::Active {
            return Err(StoreError::InvalidData(format!(
                "Run {} cannot hand off recovery while {:?}",
                prior.id, prior.state
            )));
        }
        let now = now_unix();
        end_open_turns_for_run(&tx, &prior.id, now, "failed")?;
        tx.execute(
            "UPDATE agent_invocations
             SET ended_at=COALESCE(ended_at, ?2),
                 outcome=CASE WHEN outcome='running' THEN 'failed' ELSE outcome END,
                 handback_state=COALESCE(handback_state, 'unknown')
             WHERE supervising_run_id=?1 AND ended_at IS NULL",
            params![prior.id.as_str(), now],
        )?;
        let stop_reason =
            serde_json::to_string(&StopCause::Recovery).expect("Stop cause must serialize");
        tx.execute(
            "UPDATE runs SET state='ended', ended_at=?2, stop_reason=?3
             WHERE id=?1 AND state='active'",
            params![prior.id.as_str(), now, stop_reason],
        )?;

        let trigger = RunTrigger::Recovery {
            prior_run_id: prior.id.clone(),
        };
        let token = RunLeaseToken::new();
        let run = Run {
            id: RunId::new(),
            work: prior.work.clone(),
            epoch_id: prior.epoch_id.clone(),
            home_id: prior.home_id,
            state: RunState::Reserved,
            trigger: trigger.clone(),
            retry_of: Some(prior.id),
            containment: None,
            cwd: None,
            created_at: OffsetDateTime::now_utc(),
            started_at: None,
            ended_at: None,
        };
        tx.execute(
            "INSERT INTO runs (
                id, epoch_id, home_id, state, trigger_json, retry_of, lease_hash,
                lease_generation, source_kind, source_id, created_at, ended_at, stop_reason
             ) VALUES (?1, ?2, ?3, 'reserved', ?4, ?5, ?6, NULL, ?7, ?8, ?9, NULL, NULL)",
            params![
                run.id.as_str(),
                run.epoch_id.as_str(),
                run.home_id.as_str(),
                serde_json::to_string(&trigger).expect("Run trigger must serialize"),
                run.retry_of.as_ref().map(RunId::as_str),
                token.hash(),
                run.work.kind(),
                run.work.id(),
                run.created_at.unix_timestamp(),
            ],
        )?;
        let recovery_lease = RunLease::new(
            run.id.clone(),
            run.work.clone(),
            current_epoch_in(&tx, &run.work)?.current_basis,
            token,
        );
        tx.commit()?;
        Ok((run, recovery_lease))
    }

    pub fn current_run(&self, work: &WorkRef) -> StoreResult<Option<Run>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let Ok(epoch) = current_epoch_in(&conn, work) else {
            return Ok(None);
        };
        run_for_epoch_in(&conn, &epoch.id)
    }

    pub(crate) fn run_by_id(&self, run_id: &RunId) -> StoreResult<Run> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        run_by_id_in(&conn, run_id)
    }

    pub(crate) fn latest_run(&self, work: &WorkRef) -> StoreResult<Option<Run>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let epoch = current_epoch_in(&conn, work)?;
        latest_run_for_epoch_in(&conn, &epoch.id)
    }

    pub(crate) fn resolve_run_lease(&self, token: &RunLeaseToken) -> StoreResult<RunLease> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let id = conn
            .query_row(
                "SELECT id FROM runs
                 WHERE lease_hash=?1 AND state IN ('reserved', 'active')",
                [token.hash()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| {
                StoreError::InvalidAuthority(
                    "Run lease is malformed, stale, stopped, or unknown".to_string(),
                )
            })?;
        let run = run_by_id_in(&conn, &RunId::parse(&id).map_err(invalid_durable)?)?;
        let basis = current_epoch_in(&conn, &run.work)?.current_basis;
        Ok(RunLease::new(run.id, run.work, basis, token.clone()))
    }

    pub(crate) fn validate_run_lease(&self, lease: &RunLease) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        validate_run_lease(&conn, lease)?;
        Ok(())
    }

    pub fn advance_run(
        &self,
        lease: &RunLease,
        advance: &RunAdvance,
    ) -> StoreResult<AdvanceReceipt> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut run = validate_run_lease(&tx, lease)?;
        let receipt = match advance {
            RunAdvance::RunStarting { containment, cwd } => {
                if run.state != RunState::Reserved {
                    return Err(StoreError::InvalidData(format!(
                        "Run {} cannot start while {:?}",
                        run.id, run.state
                    )));
                }
                if !cwd.is_absolute() {
                    return Err(StoreError::InvalidData(
                        "Run cwd must be absolute".to_string(),
                    ));
                }
                let (kind, id) = containment.parts();
                if id.trim().is_empty() {
                    return Err(StoreError::InvalidData(
                        "Run containment identity cannot be empty".to_string(),
                    ));
                }
                let started_at = now_unix();
                tx.execute(
                    "UPDATE runs
                     SET state='active', containment_kind=?2, containment_id=?3,
                         cwd=?4, started_at=?5
                     WHERE id=?1 AND state='reserved'",
                    params![
                        run.id.as_str(),
                        kind,
                        id,
                        cwd.display().to_string(),
                        started_at,
                    ],
                )?;
                run = run_by_id_in(&tx, &run.id)?;
                AdvanceReceipt::Run(run.clone())
            }
            RunAdvance::InvocationStarting {
                route,
                surface,
                resume_token,
                answer_ask_id,
            } => {
                if run.state != RunState::Active {
                    return Err(StoreError::InvalidData(format!(
                        "Run {} cannot supervise an Invocation while {:?}",
                        run.id, run.state
                    )));
                }
                if route.provider.trim().is_empty() || surface.trim().is_empty() {
                    return Err(StoreError::InvalidData(
                        "Invocation provider and surface cannot be empty".to_string(),
                    ));
                }
                let invocation = AgentInvocation {
                    id: AgentInvocationId::new(),
                    supervising_run_id: Some(run.id.clone()),
                    answer_ask_id: answer_ask_id.clone(),
                    route: route.clone(),
                    surface: surface.clone(),
                    resume_token: resume_token.clone(),
                    started_at: OffsetDateTime::now_utc(),
                    ended_at: None,
                };
                insert_supervised_invocation(&tx, &run, &invocation)?;
                AdvanceReceipt::Invocation(invocation)
            }
            RunAdvance::InvocationEnded {
                invocation_id,
                outcome,
            } => {
                if !outcome.is_terminal() {
                    return Err(StoreError::InvalidData(
                        "Invocation outcome must be terminal".to_string(),
                    ));
                }
                require_invocation_for_run(&tx, invocation_id, &run.id)?;
                let now = now_unix();
                tx.execute(
                    "UPDATE agent_invocations
                     SET ended_at=COALESCE(ended_at, ?2), outcome=?3,
                         handback_state=CASE
                             WHEN surface IN ('tui', 'ide') AND answer_ask_id IS NULL
                             THEN handback_state
                             ELSE ?4
                         END
                     WHERE id=?1 AND ended_at IS NULL",
                    params![
                        invocation_id.as_str(),
                        now,
                        outcome.as_invocation_outcome(),
                        handback_state(*outcome),
                    ],
                )?;
                AdvanceReceipt::Invocation(supervised_invocation_in(&tx, invocation_id)?)
            }
            RunAdvance::TurnStarting { invocation_id } => {
                require_open_invocation_for_run(&tx, invocation_id, &run.id)?;
                let basis = current_epoch_in(&tx, &run.work)?.current_basis;
                let ordinal: i64 = tx.query_row(
                    "SELECT COALESCE(MAX(ordinal), 0) + 1 FROM agent_turns WHERE invocation_id=?1",
                    [invocation_id.as_str()],
                    |row| row.get(0),
                )?;
                let turn = Turn {
                    id: TurnId::new(),
                    invocation_id: invocation_id.clone(),
                    basis: basis.clone(),
                    state: BoundaryState::Starting,
                    provider_turn_id: None,
                    root_output: None,
                    started_at: OffsetDateTime::now_utc(),
                    ended_at: None,
                };
                tx.execute(
                    "INSERT INTO agent_turns (
                        id, invocation_id, ordinal, provider_turn_id, started_at, ended_at,
                        status, input_op, context_coverage, tokenizer, system_prompt_path,
                        task_prompt_path, system_tokens, task_tokens, supplied_context_tokens,
                        provider_input_tokens, provider_output_tokens, reasoning_tokens,
                        cache_read_tokens, cache_write_tokens, cost_usd, context_gather_ms,
                        context_render_ms, context_persist_ms, first_event_seq, last_event_seq,
                        provider_total_input_tokens, peak_input_tokens, context_window_tokens,
                        epoch_id, basis_rev
                     ) VALUES (
                        ?1, ?2, ?3, NULL, ?4, NULL, 'running', 'initial', 'unknown',
                        'cl100k_base', NULL, '', 0, 0, 0, NULL, NULL, NULL, NULL, NULL,
                        NULL, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, ?5, ?6
                     )",
                    params![
                        turn.id.as_str(),
                        turn.invocation_id.as_str(),
                        ordinal,
                        turn.started_at.unix_timestamp(),
                        basis.epoch_id.as_str(),
                        basis.revision as i64,
                    ],
                )?;
                insert_seed_sends_for_turn(&tx, turn.id.as_str(), &basis)?;
                AdvanceReceipt::Turn(turn)
            }
            RunAdvance::TurnActive {
                turn_id,
                provider_turn_id,
            } => {
                require_turn_for_run(&tx, turn_id, &run.id)?;
                tx.execute(
                    "UPDATE agent_turns SET provider_turn_id=?2
                     WHERE id=?1 AND status='running'",
                    params![turn_id.as_str(), provider_turn_id],
                )?;
                AdvanceReceipt::Turn(control_turn_in(&tx, turn_id)?)
            }
            RunAdvance::TurnEnded { turn_id, outcome } => {
                if !outcome.is_terminal() {
                    return Err(StoreError::InvalidData(
                        "Turn outcome must be terminal".to_string(),
                    ));
                }
                require_turn_for_run(&tx, turn_id, &run.id)?;
                tx.execute(
                    "UPDATE agent_turns SET status=?2, ended_at=COALESCE(ended_at, ?3)
                     WHERE id=?1 AND status='running'",
                    params![turn_id.as_str(), outcome.as_turn_status(), now_unix()],
                )?;
                AdvanceReceipt::Turn(control_turn_in(&tx, turn_id)?)
            }
            RunAdvance::Wait { on } => {
                let open: bool = tx.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM agent_invocations
                        WHERE supervising_run_id=?1 AND ended_at IS NULL
                    )",
                    [run.id.as_str()],
                    |row| row.get(0),
                )?;
                if open {
                    return Err(StoreError::InvalidData(
                        "Run cannot wait while an Invocation is open".to_string(),
                    ));
                }
                let wait = Wait {
                    id: WaitId::new(),
                    work: run.work.clone(),
                    epoch_id: run.epoch_id.clone(),
                    on: on.clone(),
                    created_at: OffsetDateTime::now_utc(),
                    resolved_at: None,
                };
                tx.execute(
                    "INSERT INTO waits (id, epoch_id, on_json, created_at, resolved_at)
                     VALUES (?1, ?2, ?3, ?4, NULL)",
                    params![
                        wait.id.as_str(),
                        wait.epoch_id.as_str(),
                        serde_json::to_string(on).expect("Wait must serialize"),
                        wait.created_at.unix_timestamp(),
                    ],
                )?;
                tx.execute(
                    "UPDATE runs SET state='ended', ended_at=?2 WHERE id=?1",
                    params![run.id.as_str(), now_unix()],
                )?;
                AdvanceReceipt::Wait(wait)
            }
        };
        run = run_by_id_in(&tx, &run.id)?;
        let _ = run;
        tx.commit()?;
        Ok(receipt)
    }

    pub fn stop_run(
        &self,
        lease: &RunLease,
        cause: &StopCause,
        containment: ContainmentObservation,
    ) -> StoreResult<StopReceipt> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = validate_stop_lease(&tx, lease)?;
        let cause_json = serde_json::to_string(cause).expect("Stop cause must serialize");
        match containment {
            ContainmentObservation::Absent => {
                let now = now_unix();
                end_open_turns_for_run(&tx, &run.id, now, "failed")?;
                tx.execute(
                    "UPDATE agent_invocations
                     SET ended_at=COALESCE(ended_at, ?2),
                         outcome=CASE WHEN outcome='running' THEN 'failed' ELSE outcome END,
                         handback_state=COALESCE(handback_state, 'unknown')
                     WHERE supervising_run_id=?1 AND ended_at IS NULL",
                    params![run.id.as_str(), now],
                )?;
                tx.execute(
                    "UPDATE runs SET state='ended', ended_at=?2, stop_reason=?3 WHERE id=?1",
                    params![run.id.as_str(), now, cause_json],
                )?;
            }
            ContainmentObservation::Present | ContainmentObservation::Unprovable => {
                tx.execute(
                    "UPDATE runs
                     SET state=CASE WHEN state='reserved' THEN 'reserved' ELSE 'stopping' END,
                         stop_reason=?2
                     WHERE id=?1",
                    params![run.id.as_str(), cause_json],
                )?;
            }
        }
        let run = run_by_id_in(&tx, &run.id)?;
        tx.commit()?;
        Ok(StopReceipt { run, containment })
    }

    pub fn run_control(
        &self,
        lease: &RunLease,
        active_turn_id: Option<&str>,
    ) -> StoreResult<Option<crate::durable::RunControl>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let run = validate_stop_lease(&conn, lease)?;
        let epoch_state: String = conn.query_row(
            "SELECT state FROM epochs WHERE id=?1",
            [run.epoch_id.as_str()],
            |row| row.get(0),
        )?;
        if epoch_state == "abandoned" {
            let reason = conn
                .query_row(
                    "SELECT stop_reason FROM runs WHERE id=?1",
                    [run.id.as_str()],
                    |row| row.get::<_, Option<String>>(0),
                )?
                .unwrap_or_else(|| "Work was abandoned".to_string());
            return Ok(Some(crate::durable::RunControl::Abandon { reason }));
        }
        if run.state == RunState::Stopping {
            return Ok(Some(crate::durable::RunControl::Interrupt));
        }
        let Some(turn_id) = active_turn_id else {
            return Ok(None);
        };
        let interrupted = conn
            .query_row(
                "SELECT t.status='interrupted'
                 FROM agent_turns t
                 JOIN agent_invocations l ON l.id=t.invocation_id
                 WHERE t.id=?1 AND l.supervising_run_id=?2",
                params![turn_id, run.id.as_str()],
                |row| row.get::<_, bool>(0),
            )
            .optional()?
            .unwrap_or(false);
        Ok(interrupted.then_some(crate::durable::RunControl::Interrupt))
    }

    pub fn set_flow_position(
        &self,
        lease: &RunLease,
        position: &FlowPosition,
    ) -> StoreResult<FlowPosition> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = validate_run_lease(&tx, lease)?;
        if position.work != run.work || position.epoch_id != run.epoch_id {
            return Err(StoreError::InvalidAuthority(
                "flow position does not belong to the active Run".to_string(),
            ));
        }
        if position.flow.trim().is_empty() || position.step.trim().is_empty() {
            return Err(StoreError::InvalidData(
                "flow and step cannot be empty".to_string(),
            ));
        }
        tx.execute(
            "INSERT INTO work_flow_positions (
                epoch_id, flow, step, step_index, iteration, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(epoch_id) DO UPDATE SET
                flow=excluded.flow, step=excluded.step, step_index=excluded.step_index,
                iteration=excluded.iteration,
                updated_at=excluded.updated_at",
            params![
                position.epoch_id.as_str(),
                position.flow,
                position.step,
                i64::from(position.step_index),
                i64::from(position.iteration),
                position.updated_at.unix_timestamp(),
            ],
        )?;
        tx.commit()?;
        Ok(position.clone())
    }

    pub fn open_ask(
        &self,
        lease: &RunLease,
        invocation_id: &AgentInvocationId,
        question: &str,
    ) -> StoreResult<AskExchange> {
        let question = question.trim();
        if question.is_empty() {
            return Err(StoreError::InvalidData(
                "Ask question cannot be empty".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = validate_run_lease(&tx, lease)?;
        require_open_invocation_for_run(&tx, invocation_id, &run.id)?;
        let turn_id = current_turn_for_invocation_in(&tx, invocation_id)?;
        if let Some(existing) = pending_ask_for_turn_in(&tx, &turn_id)? {
            if existing.question == question {
                tx.commit()?;
                return Ok(existing);
            }
            return Err(StoreError::InvalidData(format!(
                "Turn {turn_id} already has an unanswered Ask"
            )));
        }
        if let Some(existing) = latest_ask_for_turn_in(&tx, &turn_id)? {
            if existing.question == question {
                tx.commit()?;
                return Ok(existing);
            }
        }
        let route = match parent_work(&tx, &run.work)? {
            Some(parent) => AnswerRoute::Parent(parent),
            None => {
                let surface: String = tx.query_row(
                    "SELECT surface FROM agent_invocations WHERE id=?1",
                    [invocation_id.as_str()],
                    |row| row.get(0),
                )?;
                if surface == "headless" {
                    return Err(StoreError::InvalidData(format!(
                        "headless root {} {} has no parent or User answer route",
                        run.work.kind(),
                        run.work.id()
                    )));
                }
                AnswerRoute::User
            }
        };
        let asked_at = OffsetDateTime::from_unix_timestamp(now_unix()).map_err(invalid_durable)?;
        let ask = AskExchange {
            id: AskId::new(),
            turn_id,
            route,
            question: question.to_string(),
            asked_at,
            answer: None,
        };
        insert_ask(&tx, &ask)?;
        enqueue_ask_comment(&tx, &ask)?;
        tx.commit()?;
        Ok(ask)
    }

    pub fn current_ask(
        &self,
        lease: &RunLease,
        invocation_id: &AgentInvocationId,
        ask_id: Option<&AskId>,
    ) -> StoreResult<AskExchange> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let run = validate_run_lease(&conn, lease)?;
        require_invocation_for_run(&conn, invocation_id, &run.id)?;
        let ask = match ask_id {
            Some(ask_id) => {
                let ask = ask_by_id_in(&conn, ask_id)?;
                let (epoch_id, work) = ask_epoch_work_in(&conn, &ask.turn_id)?;
                if epoch_id != run.epoch_id || work != run.work {
                    return Err(StoreError::InvalidAuthority(
                        "Ask does not belong to this Run's Work Epoch".to_string(),
                    ));
                }
                ask
            }
            None => {
                let turn_id = current_or_latest_turn_for_invocation_in(&conn, invocation_id)?;
                latest_ask_for_turn_in(&conn, &turn_id)?.ok_or(StoreError::NotFound)?
            }
        };
        Ok(ask)
    }

    pub fn answer_ask(
        &self,
        caller: Option<&RunLease>,
        ask_id: &AskId,
        text: &str,
    ) -> StoreResult<Answer> {
        let text = text.trim();
        if text.is_empty() {
            return Err(StoreError::InvalidData(
                "Ask answer cannot be empty".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let ask = ask_by_id_in(&tx, ask_id)?;
        validate_answer_caller(&tx, caller, &ask.route)?;
        if let Some(answer) = ask.answer.as_ref() {
            if answer.text == text {
                enqueue_answer_comment(&tx, &ask, answer)?;
                tx.commit()?;
                return Ok(answer.clone());
            }
            return Err(StoreError::InvalidAuthority(format!(
                "Ask {ask_id} was already answered"
            )));
        }
        if !ask_is_answerable_in(&tx, ask_id)? {
            return Err(StoreError::InvalidAuthority(format!(
                "Ask {ask_id} is no longer answerable"
            )));
        }
        let (author, author_kind, author_id) = match caller {
            None => (Author::User, "user", None),
            Some(lease) => (
                Author::Run(lease.run_id.clone()),
                "run",
                Some(lease.run_id.as_str()),
            ),
        };
        let answered_at =
            OffsetDateTime::from_unix_timestamp(now_unix()).map_err(invalid_durable)?;
        if tx.execute(
            "UPDATE ask_exchanges SET answer_author_kind=?2, answer_author_id=?3,
                 answer_text=?4, answered_at=?5
             WHERE id=?1 AND answered_at IS NULL",
            params![
                ask_id.as_str(),
                author_kind,
                author_id,
                text,
                answered_at.unix_timestamp(),
            ],
        )? == 0
        {
            return Err(StoreError::InvalidAuthority(format!(
                "Ask {ask_id} was answered concurrently"
            )));
        }
        let answer = Answer {
            ask_id: ask_id.clone(),
            author,
            text: text.to_string(),
            answered_at,
        };
        enqueue_answer_comment(&tx, &ask, &answer)?;
        tx.commit()?;
        Ok(answer)
    }

    pub(crate) fn pending_ask_comment_writes(&self) -> StoreResult<Vec<AskCommentWrite>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT o.ask_id, o.transition, o.issue_id, o.body,
                    w.repo, w.name, o.attempt_count, o.attempt_started_at,
                    o.last_error, o.linear_comment_id, o.delivered_at
             FROM ask_linear_comment_outbox o
             JOIN tasks t ON t.id=o.task_id
             JOIN projects p ON p.id=t.project_id
             JOIN waves w ON w.id=p.wave_id
             WHERE o.delivered_at IS NULL
             ORDER BY o.created_at, o.ask_id,
                      CASE o.transition WHEN 'ask' THEN 0 ELSE 1 END",
        )?;
        let rows = statement
            .query_map([], read_ask_comment_write_row)?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(parse_ask_comment_write_row).collect()
    }

    pub(crate) fn claim_ask_comment_write(
        &self,
        ask_id: &AskId,
        transition: AskCommentTransition,
        attempted_at: i64,
        stale_before: i64,
    ) -> StoreResult<Option<AskCommentWrite>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE ask_linear_comment_outbox
             SET attempt_count=attempt_count + 1, attempt_started_at=?3
             WHERE ask_id=?1 AND transition=?2 AND delivered_at IS NULL
               AND (attempt_started_at IS NULL OR attempt_started_at <= ?4)",
            params![
                ask_id.as_str(),
                transition.as_str(),
                attempted_at,
                stale_before
            ],
        )?;
        let write = if changed == 1 {
            Some(ask_comment_write_in(&tx, ask_id, transition)?)
        } else {
            None
        };
        tx.commit()?;
        Ok(write)
    }

    pub(crate) fn complete_ask_comment_write(
        &self,
        ask_id: &AskId,
        transition: AskCommentTransition,
        comment_id: &str,
        delivered_at: i64,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE ask_linear_comment_outbox
             SET linear_comment_id=?3, delivered_at=?4,
                 attempt_started_at=NULL, last_error=NULL
             WHERE ask_id=?1 AND transition=?2 AND delivered_at IS NULL",
            params![
                ask_id.as_str(),
                transition.as_str(),
                comment_id,
                delivered_at
            ],
        )?;
        if changed == 0 {
            let existing = ask_comment_write_in(&conn, ask_id, transition)?;
            if existing.linear_comment_id.as_deref() == Some(comment_id) {
                return Ok(());
            }
            return Err(StoreError::InvalidData(format!(
                "Ask {ask_id} {} comment write completed concurrently",
                transition.as_str()
            )));
        }
        Ok(())
    }

    pub(crate) fn fail_ask_comment_write(
        &self,
        ask_id: &AskId,
        transition: AskCommentTransition,
        error: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute(
            "UPDATE ask_linear_comment_outbox
             SET attempt_started_at=NULL, last_error=?3
             WHERE ask_id=?1 AND transition=?2 AND delivered_at IS NULL",
            params![ask_id.as_str(), transition.as_str(), error],
        )? == 0
        {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn pending_asks_for_parent(&self, parent: &WorkRef) -> StoreResult<Vec<AskExchange>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        query_answerable_asks(
            &conn,
            "a.route_kind='parent' AND a.route_work_kind=?1 AND a.route_work_id=?2",
            params![parent.kind(), parent.id()],
        )
    }

    pub(crate) fn oldest_answer_context(
        &self,
        parent: &WorkRef,
    ) -> StoreResult<Option<AnswerContext>> {
        let Some(ask) = self.pending_asks_for_parent(parent)?.into_iter().next() else {
            return Ok(None);
        };
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (epoch_id, child) = ask_epoch_work_in(&conn, &ask.turn_id)?;
        let mut statement = conn.prepare(
            "SELECT a.id FROM ask_exchanges a
             JOIN agent_turns t ON t.id=a.turn_id
             WHERE t.epoch_id=?1 AND a.id!=?2 AND a.answered_at IS NOT NULL
             ORDER BY a.asked_at, a.rowid",
        )?;
        let ids = statement
            .query_map(params![epoch_id.as_str(), ask.id.as_str()], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let prior_exchanges = ids
            .into_iter()
            .map(|id| {
                let id = AskId::parse(&id).map_err(invalid_durable)?;
                ask_by_id_in(&conn, &id)
            })
            .collect::<StoreResult<Vec<_>>>()?;
        Ok(Some(AnswerContext {
            ask,
            child,
            epoch_id,
            prior_exchanges,
        }))
    }

    pub(crate) fn answer_attempt_history(
        &self,
        ask_id: &AskId,
    ) -> StoreResult<AnswerAttemptHistory> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (failed_attempts, last_failed_at) = conn.query_row(
            "SELECT COUNT(*), MAX(ended_at) FROM agent_invocations
             WHERE answer_ask_id=?1 AND ended_at IS NOT NULL
               AND COALESCE(handback_state, 'unknown') != 'succeeded'",
            [ask_id.as_str()],
            |row| Ok((row.get::<_, u32>(0)?, row.get::<_, Option<i64>>(1)?)),
        )?;
        Ok(AnswerAttemptHistory {
            failed_attempts,
            last_failed_at: last_failed_at
                .map(OffsetDateTime::from_unix_timestamp)
                .transpose()
                .map_err(invalid_durable)?,
        })
    }

    pub fn pending_user_asks(&self) -> StoreResult<Vec<AskExchange>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        query_answerable_asks(&conn, "a.route_kind='user'", [])
    }

    pub fn has_pending_user_ask_for_work(&self, work: &WorkRef) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (wave_id, project_id, task_id) = match work {
            WorkRef::Wave(id) => (Some(id.as_str()), None, None),
            WorkRef::Project(id) => (None, Some(id.as_str()), None),
            WorkRef::Task(id) => (None, None, Some(id.as_str())),
        };
        conn.query_row(
            HAS_PENDING_USER_ASK_FOR_WORK_SQL,
            params![wave_id, project_id, task_id],
            |row| row.get(0),
        )
        .map_err(StoreError::from)
    }

    pub fn invocation_surface(
        &self,
        invocation_id: &AgentInvocationId,
    ) -> StoreResult<Option<InvocationSurface>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        invocation_surface_in(&conn, invocation_id)
    }

    pub(crate) fn open_invocation_for_run(
        &self,
        run_id: &RunId,
    ) -> StoreResult<Option<AgentInvocation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        open_invocation_for_run_in(&conn, run_id)
    }

    pub(crate) fn open_invocation_for_run_by_id(
        &self,
        run_id: &RunId,
        invocation_id: &AgentInvocationId,
    ) -> StoreResult<Option<AgentInvocation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let exists = conn
            .query_row(
                "SELECT 1 FROM agent_invocations
                 WHERE id=?1 AND supervising_run_id=?2 AND ended_at IS NULL",
                params![invocation_id.as_str(), run_id.as_str()],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        exists
            .then(|| supervised_invocation_in(&conn, invocation_id))
            .transpose()
    }

    pub(crate) fn invocations_for_run(&self, run_id: &RunId) -> StoreResult<Vec<AgentInvocation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT id FROM agent_invocations
             WHERE supervising_run_id=?1 ORDER BY started_at, rowid",
        )?;
        let ids = statement
            .query_map([run_id.as_str()], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| {
                let id = AgentInvocationId::parse(&id).map_err(invalid_durable)?;
                supervised_invocation_in(&conn, &id)
            })
            .collect()
    }

    pub(crate) fn recover_run(
        &self,
        run_id: &RunId,
        containment: ContainmentObservation,
    ) -> StoreResult<StopReceipt> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = run_by_id_in(&tx, run_id)?;
        if run.state == RunState::Ended {
            tx.commit()?;
            return Ok(StopReceipt { run, containment });
        }
        let cause = serde_json::to_string(&StopCause::Recovery).expect("Stop cause must serialize");
        match containment {
            ContainmentObservation::Absent => {
                let now = now_unix();
                end_open_turns_for_run(&tx, run_id, now, "failed")?;
                tx.execute(
                    "UPDATE agent_invocations
                     SET ended_at=COALESCE(ended_at, ?2),
                         outcome=CASE WHEN outcome='running' THEN 'failed' ELSE outcome END,
                         handback_state=COALESCE(handback_state, 'unknown')
                     WHERE supervising_run_id=?1 AND ended_at IS NULL",
                    params![run_id.as_str(), now],
                )?;
                tx.execute(
                    "UPDATE runs SET state='ended', ended_at=?2, stop_reason=?3
                     WHERE id=?1 AND state != 'ended'",
                    params![run_id.as_str(), now, cause],
                )?;
            }
            ContainmentObservation::Present | ContainmentObservation::Unprovable => {
                tx.execute(
                    "UPDATE runs
                     SET state=CASE WHEN state='reserved' THEN 'reserved' ELSE 'stopping' END,
                         stop_reason=?2
                     WHERE id=?1 AND state IN ('reserved', 'active', 'stopping')",
                    params![run_id.as_str(), cause],
                )?;
            }
        }
        let run = run_by_id_in(&tx, run_id)?;
        tx.commit()?;
        Ok(StopReceipt { run, containment })
    }

    pub fn invocation_surfaces(&self, active_only: bool) -> StoreResult<Vec<InvocationSurface>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let sql = if active_only {
            "SELECT i.id FROM agent_invocations i
             JOIN runs r ON r.id=i.supervising_run_id
             WHERE i.ended_at IS NULL AND r.state IN ('active', 'stopping')
             ORDER BY i.started_at, i.id"
        } else {
            "SELECT id FROM agent_invocations
             WHERE supervising_run_id IS NOT NULL ORDER BY started_at, id"
        };
        let mut statement = conn.prepare(sql)?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| {
                let id = AgentInvocationId::parse(&id).map_err(invalid_durable)?;
                invocation_surface_in(&conn, &id)?.ok_or(StoreError::NotFound)
            })
            .collect()
    }

    /// Record what the provider actually turned out to be once the body is running.
    ///
    /// Route provider/model are known before spawn, but the selected account
    /// and resume token are observed only after the harness starts. The token
    /// can change mid-Run when a provider hands back a new session id.
    ///
    /// This records invocation metadata only. Containment is immutable on Run,
    /// and a fenced or ended writer cannot revive itself by reporting a provider
    /// observation.
    pub fn observe_invocation_provider(
        &self,
        lease: &RunLease,
        invocation_id: &AgentInvocationId,
        account_id: Option<&crate::store::ProviderAccountId>,
        resume_token: Option<&str>,
    ) -> StoreResult<AgentInvocation> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = validate_run_lease(&tx, lease)?;
        if !matches!(run.state, RunState::Reserved | RunState::Active) {
            return Err(StoreError::InvalidData(format!(
                "Run {} cannot observe a provider while {:?}",
                run.id, run.state
            )));
        }
        if tx.execute(
            "UPDATE agent_invocations
             SET account_id=COALESCE(account_id, ?3),
                 resume_token=COALESCE(?4, resume_token),
                 provider_session_id=COALESCE(?4, provider_session_id)
             WHERE id=?1 AND supervising_run_id=?2 AND ended_at IS NULL
               AND (?3 IS NULL OR account_id IS NULL OR account_id=?3)",
            params![
                invocation_id.as_str(),
                lease.run_id.as_str(),
                account_id.map(crate::store::ProviderAccountId::as_str),
                resume_token,
            ],
        )? == 0
        {
            return Err(StoreError::NotFound);
        }
        let invocation = supervised_invocation_in(&tx, invocation_id)?;
        tx.commit()?;
        Ok(invocation)
    }

    pub fn handback_invocation(
        &self,
        invocation_id: &AgentInvocationId,
        outcome: BoundaryState,
    ) -> StoreResult<InvocationSurface> {
        if !outcome.is_terminal() {
            return Err(StoreError::InvalidData(
                "Invocation handback outcome must be terminal".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute(
            "UPDATE agent_invocations
             SET ended_at=COALESCE(ended_at, ?2),
                 outcome=?3, handback_state=?4
             WHERE id=?1 AND supervising_run_id IS NOT NULL AND ended_at IS NULL
               AND surface IN ('tui', 'ide') AND answer_ask_id IS NULL",
            params![
                invocation_id.as_str(),
                now_unix(),
                outcome.as_invocation_outcome(),
                handback_state(outcome),
            ],
        )? == 0
        {
            return Err(StoreError::NotFound);
        }
        let surface = invocation_surface_in(&tx, invocation_id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(surface)
    }

    pub fn interrupt(
        &self,
        caller: Option<&RunLease>,
        work: &WorkRef,
        if_run: &RunId,
    ) -> StoreResult<InterruptReceipt> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_control_caller(&tx, caller, work)?;
        let run = current_run_for_work_in(&tx, work)?.ok_or(StoreError::NotFound)?;
        if &run.id != if_run {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {if_run} is not current for {}",
                work.id()
            )));
        }
        let mut statement = tx.prepare(
            "SELECT t.id FROM agent_turns t
             JOIN agent_invocations i ON i.id=t.invocation_id
             WHERE i.supervising_run_id=?1 AND t.status='running'
             ORDER BY t.started_at, t.ordinal",
        )?;
        let turn_ids = statement
            .query_map([run.id.as_str()], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let now = now_unix();
        tx.execute(
            "UPDATE agent_turns SET status='interrupted', ended_at=?2
             WHERE status='running' AND invocation_id IN (
                 SELECT id FROM agent_invocations WHERE supervising_run_id=?1
             )",
            params![run.id.as_str(), now],
        )?;
        let cause =
            serde_json::to_string(&StopCause::Interrupted).expect("interrupt cause must serialize");
        if run.state == RunState::Reserved {
            tx.execute(
                "UPDATE runs SET state='ended', ended_at=?2, stop_reason=?3 WHERE id=?1",
                params![run.id.as_str(), now, cause],
            )?;
        } else {
            tx.execute(
                "UPDATE runs SET state='stopping', stop_reason=?2 WHERE id=?1",
                params![run.id.as_str(), cause],
            )?;
        }
        let receipt = InterruptReceipt {
            run_id: run.id,
            turn_ids: turn_ids
                .into_iter()
                .map(|id| TurnId::parse(&id).map_err(invalid_durable))
                .collect::<StoreResult<Vec<_>>>()?,
        };
        tx.commit()?;
        Ok(receipt)
    }

    pub fn done(&self, lease: &RunLease, basis: &Basis) -> StoreResult<DoneProposal> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = validate_run_lease(&tx, lease)?;
        let epoch = current_epoch_in(&tx, &run.work)?;
        validate_basis(&epoch.current_basis, basis)?;
        let applied = applied_basis_in(&tx, &epoch.id)?.ok_or_else(|| {
            StoreError::InvalidData("no successful boundary can complete Work".to_string())
        })?;
        validate_basis(&applied, basis)?;
        let open: bool = tx.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM agent_invocations
                WHERE supervising_run_id=?1 AND ended_at IS NULL
             )",
            [run.id.as_str()],
            |row| row.get(0),
        )?;
        if open {
            return Err(StoreError::InvalidData(
                "Run has an open Invocation".to_string(),
            ));
        }
        let child_ask_open: bool = tx.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM ask_exchanges a
                JOIN agent_turns t ON t.id=a.turn_id
                JOIN epochs child_epoch ON child_epoch.id=t.epoch_id
                WHERE a.answered_at IS NULL AND child_epoch.state='open'
                  AND t.status NOT IN ('completed', 'interrupted')
                  AND a.route_kind='parent' AND a.route_work_kind=?1
                  AND a.route_work_id=?2
             )",
            params![run.work.kind(), run.work.id()],
            |row| row.get(0),
        )?;
        if child_ask_open {
            return Err(StoreError::InvalidData(
                "Run cannot complete while a child Ask is unanswered".to_string(),
            ));
        }
        let proposal = DoneProposal {
            id: DoneProposalId::new(),
            run_id: run.id.clone(),
            basis: basis.clone(),
            proposed_at: OffsetDateTime::now_utc(),
        };
        tx.execute(
            "INSERT INTO done_proposals (id, run_id, epoch_id, basis_rev, proposed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                proposal.id.as_str(),
                proposal.run_id.as_str(),
                proposal.basis.epoch_id.as_str(),
                proposal.basis.revision as i64,
                proposal.proposed_at.unix_timestamp(),
            ],
        )?;
        let now = now_unix();
        tx.execute(
            "UPDATE epochs SET state='done', terminal_at=?2
             WHERE id=?1 AND state='open' AND current_rev=?3",
            params![epoch.id.as_str(), now, basis.revision as i64],
        )?;
        tx.execute(
            "UPDATE runs SET state='ended', ended_at=?2 WHERE id=?1",
            params![run.id.as_str(), now],
        )?;
        tx.commit()?;
        Ok(proposal)
    }

    pub fn abandon(
        &self,
        work: &WorkRef,
        reason: &str,
        if_basis: &Basis,
    ) -> StoreResult<EpochReceipt> {
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(StoreError::InvalidData(
                "abandon reason cannot be empty".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut epoch = current_epoch_in(&tx, work)?;
        validate_basis(&epoch.current_basis, if_basis)?;
        let now = now_unix();
        tx.execute(
            "UPDATE runs
             SET state=CASE WHEN state='reserved' THEN 'ended' ELSE 'stopping' END,
                 ended_at=CASE WHEN state='reserved' THEN ?3 ELSE ended_at END,
                 stop_reason=?2
             WHERE epoch_id=?1 AND state != 'ended'",
            params![epoch.id.as_str(), reason, now],
        )?;
        tx.execute(
            "UPDATE epochs SET state='abandoned', terminal_at=?2 WHERE id=?1 AND state='open'",
            params![epoch.id.as_str(), now],
        )?;
        epoch.state = EpochState::Abandoned;
        epoch.terminal_at = Some(
            OffsetDateTime::from_unix_timestamp(now).expect("current Unix timestamp must be valid"),
        );
        tx.commit()?;
        Ok(EpochReceipt { epoch })
    }

    pub fn work_status(&self, work: &WorkRef) -> StoreResult<WorkStatus> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        work_status_in(&conn, work)
    }

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

    pub(crate) fn boundary_seed_for_child(&self, target: &ChildRef) -> StoreResult<BoundarySeed> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let work = work_for_child_in(&conn, target)?;
        let (column, id) = match &work {
            WorkRef::Project(id) => ("project_id", id.as_str()),
            WorkRef::Task(id) => ("task_id", id.as_str()),
            WorkRef::Wave(_) => unreachable!("a child is Project or Task Work"),
        };
        let (epoch_id, revision) = conn.query_row(
            &format!(
                "SELECT id, current_rev FROM epochs WHERE {column}=?1 ORDER BY number DESC LIMIT 1"
            ),
            [id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )?;
        let epoch_id = EpochId::parse(&epoch_id).map_err(|error| {
            StoreError::InvalidData(format!("invalid stored Epoch id: {error}"))
        })?;
        boundary_seed_for_epoch_in(&conn, &work, epoch_id, revision as u64)
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
        caller: Option<&RunLease>,
        work: &WorkRef,
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
        validate_control_caller(&tx, caller, work)?;
        let author = caller.map_or(Author::User, |lease| Author::Run(lease.run_id.clone()));
        let receipt = Self::append_steer_in(&tx, work, &author, text)?;
        tx.commit()?;
        Ok(receipt)
    }

    /// Every durable Steer recorded at or after `since`, newest first.
    ///
    /// This joins through historical Epochs so completed and reopened Work does
    /// not lose its authored direction from inspection surfaces.
    pub fn list_steers_since(&self, since: i64) -> StoreResult<Vec<Steer>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT s.id, s.epoch_id, s.rev, s.author_kind, s.author_run_id,
                    s.text, s.issued_at, e.wave_id, e.project_id, e.task_id
             FROM steers s
             JOIN epochs e ON e.id=s.epoch_id
             WHERE s.issued_at >= ?1
             ORDER BY s.issued_at DESC, s.id DESC",
        )?;
        let rows = statement.query_map([since], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
            ))
        })?;
        let mut steers = Vec::new();
        for row in rows {
            let (
                id,
                epoch_id,
                revision,
                author_kind,
                author_run_id,
                text,
                issued_at,
                wave_id,
                project_id,
                task_id,
            ) = row?;
            let work = parse_work_columns(wave_id, project_id, task_id)?;
            steers.push(decode_steer(
                (
                    id,
                    epoch_id,
                    revision,
                    author_kind,
                    author_run_id,
                    text,
                    issued_at,
                ),
                work,
            )?);
        }
        Ok(steers)
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
}

fn map_local_home(conn: &Connection) -> StoreResult<Home> {
    conn.query_row(
        "SELECT id, route, created_at, observed_at FROM homes WHERE route='local'",
        [],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    )
    .map_err(StoreError::from)
    .and_then(|(id, route, created_at, observed_at)| {
        Ok(Home {
            id: HomeId::parse(&id).map_err(invalid_durable)?,
            route,
            created_at: OffsetDateTime::from_unix_timestamp(created_at).map_err(invalid_durable)?,
            observed_at: OffsetDateTime::from_unix_timestamp(observed_at)
                .map_err(invalid_durable)?,
        })
    })
}

fn map_home_by_id(conn: &Connection, home_id: &HomeId) -> StoreResult<Option<Home>> {
    conn.query_row(
        "SELECT id, route, created_at, observed_at FROM homes WHERE id=?1",
        [home_id.as_str()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    )
    .optional()
    .map_err(StoreError::from)?
    .map(|(id, route, created_at, observed_at)| {
        Ok(Home {
            id: HomeId::parse(&id).map_err(invalid_durable)?,
            route,
            created_at: OffsetDateTime::from_unix_timestamp(created_at).map_err(invalid_durable)?,
            observed_at: OffsetDateTime::from_unix_timestamp(observed_at)
                .map_err(invalid_durable)?,
        })
    })
    .transpose()
}

fn placement_in(conn: &Connection, work: &WorkRef) -> StoreResult<Placement> {
    find_placement_in(conn, work)?.ok_or_else(|| {
        StoreError::InvalidData(format!(
            "{} {} has no Home placement",
            work.kind(),
            work.id()
        ))
    })
}

fn reserving_home_in(conn: &Connection, work: &WorkRef) -> StoreResult<HomeId> {
    let placed = placement_in(conn, work)?.home_id;
    let local = map_local_home(conn)?.id;
    if placed != local {
        return Err(StoreError::InvalidData(format!(
            "cannot reserve {} {} on local Home {local}; it is placed on {placed}",
            work.kind(),
            work.id()
        )));
    }
    Ok(local)
}

fn find_placement_in(conn: &Connection, work: &WorkRef) -> StoreResult<Option<Placement>> {
    let row = match work {
        WorkRef::Wave(id) => conn.query_row(
            "SELECT home_id, placed_at FROM work_placements WHERE wave_id=?1",
            [id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        ),
        WorkRef::Project(id) => conn.query_row(
            "SELECT home_id, placed_at FROM work_placements WHERE project_id=?1",
            [id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        ),
        WorkRef::Task(id) => conn.query_row(
            "SELECT home_id, placed_at FROM work_placements WHERE task_id=?1",
            [id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        ),
    }
    .optional()?;
    row.map(|(home_id, placed_at)| {
        Ok(Placement {
            work: work.clone(),
            home_id: HomeId::parse(&home_id).map_err(invalid_durable)?,
            placed_at: OffsetDateTime::from_unix_timestamp(placed_at).map_err(invalid_durable)?,
        })
    })
    .transpose()
}

fn write_placement(
    tx: &Transaction<'_>,
    work: &WorkRef,
    home_id: &HomeId,
    placed_at: i64,
) -> StoreResult<()> {
    match work {
        WorkRef::Wave(id) => tx.execute(
            "INSERT INTO work_placements (wave_id, home_id, placed_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(wave_id) DO UPDATE SET
                home_id=excluded.home_id, placed_at=excluded.placed_at",
            params![id.as_str(), home_id.as_str(), placed_at],
        )?,
        WorkRef::Project(id) => tx.execute(
            "INSERT INTO work_placements (project_id, home_id, placed_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(project_id) DO UPDATE SET
                home_id=excluded.home_id, placed_at=excluded.placed_at",
            params![id.as_str(), home_id.as_str(), placed_at],
        )?,
        WorkRef::Task(id) => tx.execute(
            "INSERT INTO work_placements (task_id, home_id, placed_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(task_id) DO UPDATE SET
                home_id=excluded.home_id, placed_at=excluded.placed_at",
            params![id.as_str(), home_id.as_str(), placed_at],
        )?,
    };
    Ok(())
}

fn inherit_placement(
    tx: &Transaction<'_>,
    work: &WorkRef,
    parent: Option<&WorkRef>,
    placed_at: i64,
) -> StoreResult<()> {
    if find_placement_in(tx, work)?.is_some() {
        return Ok(());
    }
    let home_id = match parent {
        Some(parent) => placement_in(tx, parent)?.home_id,
        None => map_local_home(tx)?.id,
    };
    write_placement(tx, work, &home_id, placed_at)
}

fn resolve_wait_for_trigger(
    tx: &Transaction<'_>,
    epoch: &Epoch,
    trigger: &RunTrigger,
) -> StoreResult<()> {
    let row = tx
        .query_row(
            "SELECT id, on_json FROM waits WHERE epoch_id=?1 AND resolved_at IS NULL",
            [epoch.id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let Some((wait_id, on_json)) = row else {
        return Ok(());
    };
    let wait_on: WaitOn = serde_json::from_str(&on_json)?;
    let resolves = match (&wait_on, trigger) {
        (WaitOn::Input { after }, RunTrigger::Input { basis }) => {
            basis.epoch_id == after.epoch_id && basis.revision > after.revision
        }
        (WaitOn::Input { .. }, RunTrigger::User) => true,
        (WaitOn::Time { not_before }, RunTrigger::Time { scheduled_at }) => {
            scheduled_at >= not_before
        }
        (WaitOn::Event { event }, RunTrigger::Event { event: observed }) => event == observed,
        (WaitOn::Child { work }, RunTrigger::Child { work: observed }) => work == observed,
        (WaitOn::Capability { .. }, RunTrigger::Recovery { .. })
        | (WaitOn::Effect { .. }, RunTrigger::Recovery { .. }) => true,
        _ => false,
    };
    if !resolves {
        return Err(StoreError::InvalidData(format!(
            "Run trigger does not resolve Wait {wait_id}"
        )));
    }
    tx.execute(
        "UPDATE waits SET resolved_at=?2 WHERE id=?1 AND resolved_at IS NULL",
        params![wait_id, now_unix()],
    )?;
    Ok(())
}

pub(crate) fn validate_run_lease(conn: &Connection, lease: &RunLease) -> StoreResult<Run> {
    let run = run_by_id_in(conn, &lease.run_id)?;
    if run.work != lease.work || !matches!(run.state, RunState::Reserved | RunState::Active) {
        return Err(StoreError::InvalidAuthority(format!(
            "Run {} no longer holds execution authority",
            lease.run_id
        )));
    }
    let stored_hash: String = conn.query_row(
        "SELECT lease_hash FROM runs WHERE id=?1",
        [lease.run_id.as_str()],
        |row| row.get(0),
    )?;
    if stored_hash != lease.token_hash() {
        return Err(StoreError::InvalidAuthority(format!(
            "Run {} lease token does not match",
            lease.run_id
        )));
    }
    Ok(run)
}

pub(crate) fn validate_stop_lease(conn: &Connection, lease: &RunLease) -> StoreResult<Run> {
    let run = run_by_id_in(conn, &lease.run_id)?;
    if run.work != lease.work
        || !matches!(
            run.state,
            RunState::Reserved | RunState::Active | RunState::Stopping
        )
    {
        return Err(StoreError::InvalidAuthority(format!(
            "Run {} no longer owns cleanup authority",
            lease.run_id
        )));
    }
    let stored_hash: String = conn.query_row(
        "SELECT lease_hash FROM runs WHERE id=?1",
        [lease.run_id.as_str()],
        |row| row.get(0),
    )?;
    if stored_hash != lease.token_hash() {
        return Err(StoreError::InvalidAuthority(format!(
            "Run {} lease token does not match",
            lease.run_id
        )));
    }
    Ok(run)
}

fn run_for_epoch_in(conn: &Connection, epoch_id: &EpochId) -> StoreResult<Option<Run>> {
    let id = conn
        .query_row(
            "SELECT id FROM runs WHERE epoch_id=?1 AND state != 'ended'",
            [epoch_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    id.map(|id| {
        RunId::parse(&id)
            .map_err(invalid_durable)
            .and_then(|id| run_by_id_in(conn, &id))
    })
    .transpose()
}

fn latest_run_for_epoch_in(conn: &Connection, epoch_id: &EpochId) -> StoreResult<Option<Run>> {
    let id = conn
        .query_row(
            "SELECT id FROM runs WHERE epoch_id=?1 ORDER BY created_at DESC, rowid DESC LIMIT 1",
            [epoch_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    id.map(|id| {
        RunId::parse(&id)
            .map_err(invalid_durable)
            .and_then(|id| run_by_id_in(conn, &id))
    })
    .transpose()
}

fn current_run_for_work_in(conn: &Connection, work: &WorkRef) -> StoreResult<Option<Run>> {
    let epoch = current_epoch_in(conn, work)?;
    run_for_epoch_in(conn, &epoch.id)
}

fn run_by_id_in(conn: &Connection, run_id: &RunId) -> StoreResult<Run> {
    let row = conn.query_row(
        "SELECT r.epoch_id, r.home_id, r.state, r.trigger_json, r.retry_of,
                r.created_at, r.ended_at, e.wave_id, e.project_id, e.task_id,
                r.containment_kind, r.containment_id, r.cwd, r.started_at
         FROM runs r JOIN epochs e ON e.id=r.epoch_id WHERE r.id=?1",
        [run_id.as_str()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<i64>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<i64>>(13)?,
            ))
        },
    )?;
    let work = work_from_parts((row.7, row.8, row.9))?;
    Ok(Run {
        id: run_id.clone(),
        work,
        epoch_id: EpochId::parse(&row.0).map_err(invalid_durable)?,
        home_id: HomeId::parse(&row.1).map_err(invalid_durable)?,
        state: RunState::parse(&row.2).map_err(invalid_durable)?,
        trigger: serde_json::from_str(&row.3)?,
        retry_of: row
            .4
            .map(|id| RunId::parse(&id).map_err(invalid_durable))
            .transpose()?,
        containment: match (row.10, row.11) {
            (Some(kind), Some(id)) => Some(Containment::parse(&kind, id).map_err(invalid_durable)?),
            (None, None) => None,
            _ => {
                return Err(StoreError::InvalidData(
                    "stored Run containment is incomplete".to_string(),
                ))
            }
        },
        cwd: row.12.map(Into::into),
        created_at: OffsetDateTime::from_unix_timestamp(row.5).map_err(invalid_durable)?,
        started_at: row
            .13
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()
            .map_err(invalid_durable)?,
        ended_at: row
            .6
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()
            .map_err(invalid_durable)?,
    })
}

fn insert_supervised_invocation(
    tx: &Transaction<'_>,
    run: &Run,
    invocation: &AgentInvocation,
) -> StoreResult<()> {
    if let Some(ask_id) = invocation.answer_ask_id.as_ref() {
        let ask = ask_by_id_in(tx, ask_id)?;
        if ask.route != AnswerRoute::Parent(run.work.clone()) {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {} does not own Ask {ask_id}'s answer route",
                run.id
            )));
        }
        if !ask_is_answerable_in(tx, ask_id)? {
            return Err(StoreError::InvalidAuthority(format!(
                "Ask {ask_id} is no longer answerable"
            )));
        }
    }
    let labels = work_labels(tx, &run.work)?;
    let cwd = run
        .cwd
        .as_ref()
        .expect("an active Run has cwd")
        .display()
        .to_string();
    let (_, containment_id) = run
        .containment
        .as_ref()
        .expect("an active Run has containment")
        .parts();
    tx.execute(
        "INSERT INTO agent_invocations (
            id, run_id, process_id, started_at, ended_at, repo, worktree, wave,
            flow, skill, project, task, provider, model, surface, capture_status,
            incomplete_reason, outcome, artifact_dir, conversation_path,
            provider_events_path, provider_session_id, provider_session_path,
            conversation_event_count, conversation_bytes, supervising_run_id,
            account_id, resume_token, answer_ask_id
         ) VALUES (
            ?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, NULL, NULL, ?8, ?9, ?10, ?11,
            ?12, 'prompt_only', NULL, 'running', '', '', NULL, NULL, NULL, 0, 0,
            ?2, ?13, ?14, ?15
         )",
        params![
            invocation.id.as_str(),
            run.id.as_str(),
            containment_id,
            invocation.started_at.unix_timestamp(),
            labels.repo,
            cwd,
            labels.wave,
            labels.project,
            labels.task,
            invocation.route.provider,
            invocation.route.model,
            invocation.surface,
            invocation.route.account_id,
            invocation.resume_token,
            invocation.answer_ask_id.as_ref().map(AskId::as_str),
        ],
    )?;
    Ok(())
}

struct WorkLabels {
    wave: Option<String>,
    project: Option<String>,
    task: Option<String>,
    repo: String,
}

fn work_labels(conn: &Connection, work: &WorkRef) -> StoreResult<WorkLabels> {
    match work {
        WorkRef::Wave(id) => conn
            .query_row(
                "SELECT name, repo FROM waves WHERE id=?1",
                [id.as_str()],
                |row| {
                    Ok(WorkLabels {
                        wave: Some(row.get(0)?),
                        project: None,
                        task: None,
                        repo: row.get(1)?,
                    })
                },
            )
            .map_err(StoreError::from),
        WorkRef::Project(id) => conn
            .query_row(
                "SELECT w.name, p.external_project_id, w.repo
                 FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
                [id.as_str()],
                |row| {
                    Ok(WorkLabels {
                        wave: Some(row.get(0)?),
                        project: Some(row.get(1)?),
                        task: None,
                        repo: row.get(2)?,
                    })
                },
            )
            .map_err(StoreError::from),
        WorkRef::Task(id) => conn
            .query_row(
                "SELECT w.name, p.external_project_id, t.issue_identifier, w.repo
                 FROM tasks t
                 JOIN projects p ON p.id=t.project_id
                 JOIN waves w ON w.id=p.wave_id
                 WHERE t.id=?1",
                [id.as_str()],
                |row| {
                    Ok(WorkLabels {
                        wave: Some(row.get(0)?),
                        project: Some(row.get(1)?),
                        task: Some(row.get(2)?),
                        repo: row.get(3)?,
                    })
                },
            )
            .map_err(StoreError::from),
    }
}

fn require_invocation_for_run(
    conn: &Connection,
    invocation_id: &AgentInvocationId,
    run_id: &RunId,
) -> StoreResult<()> {
    conn.query_row(
        "SELECT 1 FROM agent_invocations WHERE id=?1 AND supervising_run_id=?2",
        params![invocation_id.as_str(), run_id.as_str()],
        |_| Ok(()),
    )
    .map_err(StoreError::from)
}

fn require_open_invocation_for_run(
    conn: &Connection,
    invocation_id: &AgentInvocationId,
    run_id: &RunId,
) -> StoreResult<()> {
    conn.query_row(
        "SELECT 1 FROM agent_invocations
         WHERE id=?1 AND supervising_run_id=?2 AND ended_at IS NULL",
        params![invocation_id.as_str(), run_id.as_str()],
        |_| Ok(()),
    )
    .map_err(StoreError::from)
}

fn open_invocation_for_run_in(
    conn: &Connection,
    run_id: &RunId,
) -> StoreResult<Option<AgentInvocation>> {
    let invocation_id = conn
        .query_row(
            "SELECT id FROM agent_invocations
             WHERE supervising_run_id=?1 AND ended_at IS NULL
             ORDER BY started_at, rowid LIMIT 1",
            [run_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    invocation_id
        .map(|id| {
            let id = AgentInvocationId::parse(&id).map_err(invalid_durable)?;
            supervised_invocation_in(conn, &id)
        })
        .transpose()
}

fn supervised_invocation_in(
    conn: &Connection,
    invocation_id: &AgentInvocationId,
) -> StoreResult<AgentInvocation> {
    let row = conn.query_row(
        "SELECT supervising_run_id, provider, model, account_id, surface,
                resume_token, started_at, ended_at, answer_ask_id
         FROM agent_invocations WHERE id=?1 AND supervising_run_id IS NOT NULL",
        [invocation_id.as_str()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, Option<String>>(8)?,
            ))
        },
    )?;
    Ok(AgentInvocation {
        id: invocation_id.clone(),
        supervising_run_id: Some(RunId::parse(&row.0).map_err(invalid_durable)?),
        answer_ask_id: row
            .8
            .map(|id| AskId::parse(&id).map_err(invalid_durable))
            .transpose()?,
        route: InvocationRoute {
            provider: row.1,
            model: row.2,
            account_id: row.3,
        },
        surface: row.4,
        resume_token: row.5,
        started_at: OffsetDateTime::from_unix_timestamp(row.6).map_err(invalid_durable)?,
        ended_at: row
            .7
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()
            .map_err(invalid_durable)?,
    })
}

fn invocation_surface_in(
    conn: &Connection,
    invocation_id: &AgentInvocationId,
) -> StoreResult<Option<InvocationSurface>> {
    let row = conn
        .query_row(
            "SELECT r.id, e.wave_id, e.project_id, e.task_id, h.route,
                    l.handback_state
             FROM agent_invocations l
             JOIN runs r ON r.id=l.supervising_run_id
             JOIN epochs e ON e.id=r.epoch_id
             JOIN homes h ON h.id=r.home_id
             WHERE l.id=?1 AND l.supervising_run_id IS NOT NULL",
            [invocation_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .optional()?;
    let Some(row) = row else {
        return Ok(None);
    };
    let work = work_from_parts((row.1, row.2, row.3))?;
    let handback = row
        .5
        .as_deref()
        .map(BoundaryState::parse_handback)
        .transpose()
        .map_err(invalid_durable)?;
    let invocation = supervised_invocation_in(conn, invocation_id)?;
    let run_id = RunId::parse(&row.0).map_err(invalid_durable)?;
    let run = run_by_id_in(conn, &run_id)?;
    let attach_argv = match &run.containment {
        Some(Containment::Tmux { name }) => Some(vec![
            "tmux".to_string(),
            "attach-session".to_string(),
            "-t".to_string(),
            name.clone(),
        ]),
        Some(Containment::ProcessGroup { .. }) | None => None,
    };
    debug_assert_eq!(invocation.supervising_run_id.as_ref(), Some(&run.id));
    let wave_id = match &work {
        WorkRef::Wave(id) => id.clone(),
        WorkRef::Project(id) => {
            let value: String = conn.query_row(
                "SELECT wave_id FROM projects WHERE id=?1",
                [id.as_str()],
                |row| row.get(0),
            )?;
            WaveId::parse(&value).map_err(invalid_durable)?
        }
        WorkRef::Task(id) => {
            let value: String = conn.query_row(
                "SELECT p.wave_id FROM tasks t JOIN projects p ON p.id=t.project_id
                 WHERE t.id=?1",
                [id.as_str()],
                |row| row.get(0),
            )?;
            WaveId::parse(&value).map_err(invalid_durable)?
        }
    };
    Ok(Some(InvocationSurface {
        invocation,
        run,
        work,
        wave_id,
        home_route: row.4,
        handback,
        attach_argv,
    }))
}

fn require_turn_for_run(conn: &Connection, turn_id: &TurnId, run_id: &RunId) -> StoreResult<()> {
    conn.query_row(
        "SELECT 1 FROM agent_turns t
         JOIN agent_invocations l ON l.id=t.invocation_id
         WHERE t.id=?1 AND l.supervising_run_id=?2",
        params![turn_id.as_str(), run_id.as_str()],
        |_| Ok(()),
    )
    .map_err(StoreError::from)
}

fn control_turn_in(conn: &Connection, turn_id: &TurnId) -> StoreResult<Turn> {
    let row = conn.query_row(
        "SELECT invocation_id, epoch_id, basis_rev, status, provider_turn_id,
                root_output, started_at, ended_at FROM agent_turns WHERE id=?1",
        [turn_id.as_str()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<i64>>(7)?,
            ))
        },
    )?;
    Ok(Turn {
        id: turn_id.clone(),
        invocation_id: AgentInvocationId::parse(&row.0).map_err(invalid_durable)?,
        basis: Basis {
            epoch_id: EpochId::parse(&row.1).map_err(invalid_durable)?,
            revision: row.2 as u64,
        },
        state: BoundaryState::parse_turn(&row.3).map_err(invalid_durable)?,
        provider_turn_id: row.4,
        root_output: row.5,
        started_at: OffsetDateTime::from_unix_timestamp(row.6).map_err(invalid_durable)?,
        ended_at: row
            .7
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()
            .map_err(invalid_durable)?,
    })
}

fn handback_state(state: BoundaryState) -> &'static str {
    match state {
        BoundaryState::Succeeded => "succeeded",
        BoundaryState::Failed => "failed",
        BoundaryState::Interrupted => "interrupted",
        BoundaryState::Unknown => "unknown",
        BoundaryState::Starting | BoundaryState::Active => {
            unreachable!("terminal outcome validated before handback mapping")
        }
    }
}

fn parent_work(conn: &Connection, work: &WorkRef) -> StoreResult<Option<WorkRef>> {
    match work {
        WorkRef::Wave(_) => Ok(None),
        WorkRef::Project(id) => {
            let wave_id: String = conn.query_row(
                "SELECT wave_id FROM projects WHERE id=?1",
                [id.as_str()],
                |row| row.get(0),
            )?;
            Ok(Some(WorkRef::Wave(
                WaveId::parse(&wave_id).map_err(invalid_durable)?,
            )))
        }
        WorkRef::Task(id) => {
            let project_id: String = conn.query_row(
                "SELECT project_id FROM tasks WHERE id=?1",
                [id.as_str()],
                |row| row.get(0),
            )?;
            Ok(Some(WorkRef::Project(
                ProjectId::parse(&project_id).map_err(invalid_durable)?,
            )))
        }
    }
}

fn current_turn_for_invocation_in(
    conn: &Connection,
    invocation_id: &AgentInvocationId,
) -> StoreResult<TurnId> {
    let id = conn
        .query_row(
            "SELECT id FROM agent_turns
             WHERE invocation_id=?1 AND status='running'
             ORDER BY ordinal DESC LIMIT 1",
            [invocation_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| {
            StoreError::InvalidAuthority(format!(
                "AgentInvocation {invocation_id} has no active Turn"
            ))
        })?;
    TurnId::parse(&id).map_err(invalid_durable)
}

fn current_or_latest_turn_for_invocation_in(
    conn: &Connection,
    invocation_id: &AgentInvocationId,
) -> StoreResult<TurnId> {
    let id = conn
        .query_row(
            "SELECT id FROM agent_turns WHERE invocation_id=?1
             ORDER BY (status='running') DESC, ordinal DESC LIMIT 1",
            [invocation_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(StoreError::NotFound)?;
    TurnId::parse(&id).map_err(invalid_durable)
}

fn ask_epoch_work_in(conn: &Connection, turn_id: &TurnId) -> StoreResult<(EpochId, WorkRef)> {
    let (epoch_id, wave_id, project_id, task_id) = conn.query_row(
        "SELECT e.id, e.wave_id, e.project_id, e.task_id
         FROM agent_turns t JOIN epochs e ON e.id=t.epoch_id
         WHERE t.id=?1",
        [turn_id.as_str()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        },
    )?;
    Ok((
        EpochId::parse(&epoch_id).map_err(invalid_durable)?,
        work_from_parts((wave_id, project_id, task_id))?,
    ))
}

fn insert_ask(conn: &Connection, ask: &AskExchange) -> StoreResult<()> {
    let (route_kind, route_work_kind, route_work_id) = match &ask.route {
        AnswerRoute::User => ("user", None, None),
        AnswerRoute::Parent(work) => ("parent", Some(work.kind()), Some(work.id())),
    };
    conn.execute(
        "INSERT INTO ask_exchanges (
            id, turn_id, route_kind, route_work_kind, route_work_id,
            question, asked_at, answer_author_kind, answer_author_id,
            answer_text, answered_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, NULL, NULL)",
        params![
            ask.id.as_str(),
            ask.turn_id.as_str(),
            route_kind,
            route_work_kind,
            route_work_id,
            ask.question,
            ask.asked_at.unix_timestamp(),
        ],
    )?;
    Ok(())
}

fn enqueue_ask_comment(conn: &Connection, ask: &AskExchange) -> StoreResult<()> {
    let route = match &ask.route {
        AnswerRoute::User => "User".to_string(),
        AnswerRoute::Parent(work) => format!("{} `{}`", work.kind(), work.id()),
    };
    let transition = AskCommentTransition::Ask;
    let body = format!(
        "### Loopflow Ask\n\n**Route:** {route}\n\n{}\n\n{}",
        ask.question,
        transition.marker(&ask.id)
    );
    enqueue_ask_comment_write(
        conn,
        &ask.id,
        &ask.turn_id,
        transition,
        &body,
        ask.asked_at.unix_timestamp(),
    )
}

fn enqueue_answer_comment(
    conn: &Connection,
    ask: &AskExchange,
    answer: &Answer,
) -> StoreResult<()> {
    let author = match &answer.author {
        Author::User => "User".to_string(),
        Author::Run(run_id) => format!("Run `{run_id}`"),
    };
    let transition = AskCommentTransition::Answer;
    let body = format!(
        "### Loopflow Answer\n\n**Author:** {author}\n\n{}\n\n{}",
        answer.text,
        transition.marker(&ask.id)
    );
    enqueue_ask_comment_write(
        conn,
        &ask.id,
        &ask.turn_id,
        transition,
        &body,
        answer.answered_at.unix_timestamp(),
    )
}

fn enqueue_ask_comment_write(
    conn: &Connection,
    ask_id: &AskId,
    turn_id: &TurnId,
    transition: AskCommentTransition,
    body: &str,
    created_at: i64,
) -> StoreResult<()> {
    let (_, work) = ask_epoch_work_in(conn, turn_id)?;
    let WorkRef::Task(task_id) = work else {
        return Ok(());
    };
    let issue_id: String = conn.query_row(
        "SELECT external_issue_id FROM tasks WHERE id=?1",
        [task_id.as_str()],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO ask_linear_comment_outbox (
            ask_id, transition, task_id, issue_id, body, created_at,
            attempt_count, attempt_started_at, last_error,
            linear_comment_id, delivered_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, NULL, NULL, NULL)",
        params![
            ask_id.as_str(),
            transition.as_str(),
            task_id.as_str(),
            issue_id,
            body,
            created_at,
        ],
    )?;
    Ok(())
}

type AskCommentWriteRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    i64,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<i64>,
);

fn read_ask_comment_write_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AskCommentWriteRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
    ))
}

fn parse_ask_comment_write_row(row: AskCommentWriteRow) -> StoreResult<AskCommentWrite> {
    let transition = match row.1.as_str() {
        "ask" => AskCommentTransition::Ask,
        "answer" => AskCommentTransition::Answer,
        value => {
            return Err(StoreError::InvalidData(format!(
                "invalid Ask comment transition {value:?}"
            )))
        }
    };
    Ok(AskCommentWrite {
        ask_id: AskId::parse(&row.0).map_err(invalid_durable)?,
        transition,
        issue_id: row.2,
        body: row.3,
        repo: row.4,
        wave: row.5,
        attempt_count: u32::try_from(row.6).map_err(|_| {
            StoreError::InvalidData("invalid Ask comment attempt count".to_string())
        })?,
        attempt_started_at: row.7,
        last_error: row.8,
        linear_comment_id: row.9,
        delivered_at: row.10,
    })
}

fn ask_comment_write_in(
    conn: &Connection,
    ask_id: &AskId,
    transition: AskCommentTransition,
) -> StoreResult<AskCommentWrite> {
    let row = conn.query_row(
        "SELECT o.ask_id, o.transition, o.issue_id, o.body,
                w.repo, w.name, o.attempt_count, o.attempt_started_at,
                o.last_error, o.linear_comment_id, o.delivered_at
         FROM ask_linear_comment_outbox o
         JOIN tasks t ON t.id=o.task_id
         JOIN projects p ON p.id=t.project_id
         JOIN waves w ON w.id=p.wave_id
         WHERE o.ask_id=?1 AND o.transition=?2",
        params![ask_id.as_str(), transition.as_str()],
        read_ask_comment_write_row,
    )?;
    parse_ask_comment_write_row(row)
}

pub(super) fn ask_by_id_in(conn: &Connection, ask_id: &AskId) -> StoreResult<AskExchange> {
    conn.query_row(
        "SELECT turn_id, route_kind, route_work_kind, route_work_id,
                question, asked_at, answer_author_kind, answer_author_id,
                answer_text, answered_at
         FROM ask_exchanges WHERE id=?1",
        [ask_id.as_str()],
        |row| {
            map_ask_row(
                ask_id.clone(),
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
            )
            .map_err(to_sqlite_conversion_error)
        },
    )
    .map_err(StoreError::from)
}

#[allow(clippy::too_many_arguments)]
fn map_ask_row(
    id: AskId,
    turn_id: String,
    route_kind: String,
    route_work_kind: Option<String>,
    route_work_id: Option<String>,
    question: String,
    asked_at: i64,
    answer_author_kind: Option<String>,
    answer_author_id: Option<String>,
    answer_text: Option<String>,
    answered_at: Option<i64>,
) -> StoreResult<AskExchange> {
    let route = match (route_kind.as_str(), route_work_kind, route_work_id) {
        ("user", None, None) => AnswerRoute::User,
        ("parent", Some(kind), Some(id)) => AnswerRoute::Parent(parse_work_ref(&kind, &id)?),
        _ => {
            return Err(StoreError::InvalidData(
                "stored Ask route is inconsistent".to_string(),
            ))
        }
    };
    let answer = match (
        answer_author_kind.as_deref(),
        answer_author_id,
        answer_text,
        answered_at,
    ) {
        (None, None, None, None) => None,
        (Some("user"), None, Some(text), Some(answered_at)) => Some(Answer {
            ask_id: id.clone(),
            author: Author::User,
            text,
            answered_at: OffsetDateTime::from_unix_timestamp(answered_at)
                .map_err(invalid_durable)?,
        }),
        (Some("run"), Some(run_id), Some(text), Some(answered_at)) => Some(Answer {
            ask_id: id.clone(),
            author: Author::Run(RunId::parse(&run_id).map_err(invalid_durable)?),
            text,
            answered_at: OffsetDateTime::from_unix_timestamp(answered_at)
                .map_err(invalid_durable)?,
        }),
        _ => {
            return Err(StoreError::InvalidData(
                "stored Ask answer is inconsistent".to_string(),
            ))
        }
    };
    Ok(AskExchange {
        id,
        turn_id: TurnId::parse(&turn_id).map_err(invalid_durable)?,
        route,
        question,
        asked_at: OffsetDateTime::from_unix_timestamp(asked_at).map_err(invalid_durable)?,
        answer,
    })
}

fn pending_ask_for_turn_in(
    conn: &Connection,
    turn_id: &TurnId,
) -> StoreResult<Option<AskExchange>> {
    let id = conn
        .query_row(
            "SELECT id FROM ask_exchanges WHERE turn_id=?1 AND answered_at IS NULL",
            [turn_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    id.map(|id| AskId::parse(&id).map_err(invalid_durable))
        .transpose()?
        .map(|id| ask_by_id_in(conn, &id))
        .transpose()
}

fn latest_ask_for_turn_in(conn: &Connection, turn_id: &TurnId) -> StoreResult<Option<AskExchange>> {
    let id = conn
        .query_row(
            "SELECT id FROM ask_exchanges WHERE turn_id=?1
             ORDER BY asked_at DESC, rowid DESC LIMIT 1",
            [turn_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    id.map(|id| AskId::parse(&id).map_err(invalid_durable))
        .transpose()?
        .map(|id| ask_by_id_in(conn, &id))
        .transpose()
}

fn validate_answer_caller(
    conn: &Connection,
    caller: Option<&RunLease>,
    route: &AnswerRoute,
) -> StoreResult<()> {
    match (route, caller) {
        (AnswerRoute::User, None) => Ok(()),
        (AnswerRoute::Parent(parent), Some(lease)) => {
            let run = validate_run_lease(conn, lease)?;
            if &run.work == parent {
                Ok(())
            } else {
                Err(StoreError::InvalidAuthority(
                    "Run does not own this Ask answer route".to_string(),
                ))
            }
        }
        _ => Err(StoreError::InvalidAuthority(
            "caller does not own this Ask answer route".to_string(),
        )),
    }
}

fn ask_is_answerable_in(conn: &Connection, ask_id: &AskId) -> StoreResult<bool> {
    conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM ask_exchanges a
            JOIN agent_turns t ON t.id=a.turn_id
            JOIN epochs e ON e.id=t.epoch_id
            WHERE a.id=?1 AND a.answered_at IS NULL AND e.state='open'
              AND t.status NOT IN ('completed', 'interrupted')
         )",
        [ask_id.as_str()],
        |row| row.get(0),
    )
    .map_err(StoreError::from)
}

fn query_answerable_asks(
    conn: &Connection,
    route_predicate: &str,
    parameters: impl rusqlite::Params,
) -> StoreResult<Vec<AskExchange>> {
    let sql = format!(
        "SELECT a.id FROM ask_exchanges a
         JOIN agent_turns t ON t.id=a.turn_id
         JOIN epochs e ON e.id=t.epoch_id
         WHERE a.answered_at IS NULL AND e.state='open'
           AND t.status NOT IN ('completed', 'interrupted')
           AND {route_predicate}
         ORDER BY a.asked_at, a.rowid"
    );
    let mut statement = conn.prepare(&sql)?;
    let ids = statement
        .query_map(parameters, |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter()
        .map(|id| {
            let id = AskId::parse(&id).map_err(invalid_durable)?;
            ask_by_id_in(conn, &id)
        })
        .collect()
}

fn validate_control_caller(
    conn: &Connection,
    caller: Option<&RunLease>,
    target: &WorkRef,
) -> StoreResult<()> {
    let Some(lease) = caller else {
        return Ok(());
    };
    let run = validate_run_lease(conn, lease)?;
    if parent_work(conn, target)?.as_ref() == Some(&run.work) {
        Ok(())
    } else {
        Err(StoreError::InvalidAuthority(
            "Run may control only immediate child Work".to_string(),
        ))
    }
}

pub(crate) fn work_status_in(conn: &Connection, work: &WorkRef) -> StoreResult<WorkStatus> {
    let epoch = latest_epoch_in(conn, work)?;
    match epoch.state {
        EpochState::Done => return Ok(WorkStatus::Done),
        EpochState::Abandoned => return Ok(WorkStatus::Abandoned),
        EpochState::Open => {}
    }
    if let Some(run) = run_for_epoch_in(conn, &epoch.id)? {
        return Ok(WorkStatus::Running { run_id: run.id });
    }
    let wait = conn
        .query_row(
            "SELECT id, on_json, created_at, resolved_at FROM waits
             WHERE epoch_id=?1 AND resolved_at IS NULL",
            [epoch.id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            },
        )
        .optional()?;
    if let Some((id, on_json, created_at, resolved_at)) = wait {
        return Ok(WorkStatus::Waiting {
            wait: Wait {
                id: WaitId::parse(&id).map_err(invalid_durable)?,
                work: work.clone(),
                epoch_id: epoch.id,
                on: serde_json::from_str(&on_json)?,
                created_at: OffsetDateTime::from_unix_timestamp(created_at)
                    .map_err(invalid_durable)?,
                resolved_at: resolved_at
                    .map(OffsetDateTime::from_unix_timestamp)
                    .transpose()
                    .map_err(invalid_durable)?,
            },
        });
    }
    Ok(WorkStatus::Ready)
}

fn latest_epoch_in(conn: &Connection, work: &WorkRef) -> StoreResult<Epoch> {
    let (column, id) = match work {
        WorkRef::Wave(id) => ("wave_id", id.as_str()),
        WorkRef::Project(id) => ("project_id", id.as_str()),
        WorkRef::Task(id) => ("task_id", id.as_str()),
    };
    let sql = format!(
        "SELECT id, number, state, current_rev, created_at, terminal_at
         FROM epochs WHERE {column}=?1 ORDER BY number DESC LIMIT 1"
    );
    let row = conn.query_row(&sql, [id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, Option<i64>>(5)?,
        ))
    })?;
    let epoch_id = EpochId::parse(&row.0).map_err(invalid_durable)?;
    Ok(Epoch {
        id: epoch_id.clone(),
        work: work.clone(),
        number: row.1 as u32,
        state: EpochState::parse(&row.2).map_err(invalid_durable)?,
        current_basis: Basis {
            epoch_id,
            revision: row.3 as u64,
        },
        created_at: OffsetDateTime::from_unix_timestamp(row.4).map_err(invalid_durable)?,
        terminal_at: row
            .5
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()
            .map_err(invalid_durable)?,
    })
}

fn parse_work_ref(kind: &str, id: &str) -> StoreResult<WorkRef> {
    match kind {
        "wave" => WaveId::parse(id)
            .map(WorkRef::Wave)
            .map_err(invalid_durable),
        "project" => ProjectId::parse(id)
            .map(WorkRef::Project)
            .map_err(invalid_durable),
        "task" => TaskId::parse(id)
            .map(WorkRef::Task)
            .map_err(invalid_durable),
        value => Err(StoreError::InvalidData(format!(
            "invalid Work kind: {value}"
        ))),
    }
}

fn work_from_parts(
    parts: (Option<String>, Option<String>, Option<String>),
) -> StoreResult<WorkRef> {
    match parts {
        (Some(id), None, None) => parse_work_ref("wave", &id),
        (None, Some(id), None) => parse_work_ref("project", &id),
        (None, None, Some(id)) => parse_work_ref("task", &id),
        _ => Err(StoreError::InvalidData(
            "stored Epoch owns an invalid Work reference".to_string(),
        )),
    }
}

fn invalid_durable(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

fn to_sqlite_conversion_error(error: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    )
}

pub(crate) fn create_wave_spine(
    tx: &Transaction<'_>,
    wave_id: &WaveId,
    name: &str,
    repo: &str,
    created_at: i64,
) -> StoreResult<()> {
    let work = WorkRef::Wave(wave_id.clone());
    inherit_placement(tx, &work, None, created_at)?;
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM epochs WHERE wave_id=?1)",
        [wave_id.as_str()],
        |row| row.get(0),
    )?;
    if exists {
        return Ok(());
    }
    let epoch_id = EpochId::new();
    tx.execute(
        "INSERT INTO epochs (
            id, number, wave_id, project_id, task_id, state, current_rev,
            created_at, terminal_at
         ) VALUES (?1, 1, ?2, NULL, NULL, 'open', 0, ?3, NULL)",
        params![epoch_id.as_str(), wave_id.as_str(), created_at],
    )?;
    insert_truth(
        tx,
        &epoch_id,
        serde_json::json!({"name": name, "repo": repo}),
        OffsetDateTime::from_unix_timestamp(created_at).map_err(invalid_durable)?,
    )
}

pub(crate) fn create_project_spine(tx: &Transaction<'_>, project: &Project) -> StoreResult<()> {
    let project_id = tx
        .query_row(
            "SELECT id FROM projects WHERE external_project_id=?1",
            [project.plan.id.as_str()],
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
            project.wave_id.as_str(),
            project.plan.id.as_str(),
            project.created_at.unix_timestamp(),
        ],
    )?;
    let work = WorkRef::Project(ProjectId::parse(&project_id).map_err(invalid_durable)?);
    let parent = WorkRef::Wave(project.wave_id.clone());
    inherit_placement(
        tx,
        &work,
        Some(&parent),
        project.created_at.unix_timestamp(),
    )?;
    let epoch_id = EpochId::new();
    let number: i64 = tx.query_row(
        "SELECT COALESCE(MAX(number), 0) + 1 FROM epochs WHERE project_id=?1",
        [&project_id],
        |row| row.get(0),
    )?;
    tx.execute(
        "INSERT INTO epochs (
            id, number, wave_id, project_id, task_id, state, current_rev,
            created_at, terminal_at
         ) VALUES (?1, ?2, NULL, ?3, NULL, 'open', 0, ?4, NULL)",
        params![
            epoch_id.as_str(),
            number,
            project_id,
            project.updated_at.unix_timestamp(),
        ],
    )?;
    insert_truth(
        tx,
        &epoch_id,
        serde_json::json!({
            "external_project_id": project.plan.id.as_str(),
            "slug": project.plan.slug,
            "name": project.plan.name,
            "prompt_context": project.plan.prompt_context,
            "pm_snapshot_synced_at": project.plan.pm_snapshot_synced_at,
        }),
        project.updated_at,
    )?;
    Ok(())
}

pub(crate) fn create_task_spine(tx: &Transaction<'_>, task: &Task) -> StoreResult<()> {
    let project_id = task.project_id.as_str().to_string();
    let task_id = tx
        .query_row(
            "SELECT id FROM tasks WHERE external_issue_id=?1",
            [task.plan.id.as_str()],
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
            task.plan.id.as_str(),
            task.plan.identifier,
            task.created_at.unix_timestamp(),
        ],
    )?;
    let work = WorkRef::Task(TaskId::parse(&task_id).map_err(invalid_durable)?);
    let parent = WorkRef::Project(ProjectId::parse(&project_id).map_err(invalid_durable)?);
    inherit_placement(tx, &work, Some(&parent), task.created_at.unix_timestamp())?;
    let epoch_id = EpochId::new();
    let number: i64 = tx.query_row(
        "SELECT COALESCE(MAX(number), 0) + 1 FROM epochs WHERE task_id=?1",
        [&task_id],
        |row| row.get(0),
    )?;
    tx.execute(
        "INSERT INTO epochs (
            id, number, wave_id, project_id, task_id, state, current_rev,
            created_at, terminal_at
         ) VALUES (?1, ?2, NULL, NULL, ?3, 'open', 0, ?4, NULL)",
        params![
            epoch_id.as_str(),
            number,
            task_id,
            task.updated_at.unix_timestamp(),
        ],
    )?;
    insert_truth(
        tx,
        &epoch_id,
        serde_json::json!({
            "external_issue_id": task.plan.id.as_str(),
            "identifier": task.plan.identifier,
            "title": task.plan.title,
            "description": task.plan.description,
            "pm_snapshot_synced_at": task.plan.pm_snapshot_synced_at,
        }),
        task.updated_at,
    )?;
    Ok(())
}

pub(crate) fn end_run_for_lease(
    conn: &Connection,
    lease: &RunLease,
    outcome: BoundaryState,
) -> StoreResult<()> {
    if !outcome.is_terminal() {
        return Err(StoreError::InvalidData(
            "Run finish outcome must be terminal".to_string(),
        ));
    }
    let run = validate_stop_lease(conn, lease)?;
    let now = now_unix();
    let turn_outcome = if outcome == BoundaryState::Interrupted {
        "interrupted"
    } else {
        "failed"
    };
    end_open_turns_for_run(conn, &run.id, now, turn_outcome)?;
    conn.execute(
        "UPDATE agent_invocations SET
            ended_at=COALESCE(ended_at, ?2),
            outcome=?3, handback_state=?4
         WHERE supervising_run_id=?1 AND ended_at IS NULL",
        params![
            run.id.as_str(),
            now,
            outcome.as_invocation_outcome(),
            handback_state(outcome)
        ],
    )?;
    conn.execute(
        "UPDATE runs SET state='ended', ended_at=?2
         WHERE id=?1 AND state != 'ended'",
        params![run.id.as_str(), now],
    )?;
    Ok(())
}

fn end_open_turns_for_run(
    conn: &Connection,
    run_id: &RunId,
    ended_at: i64,
    outcome: &str,
) -> StoreResult<()> {
    conn.execute(
        "UPDATE agent_turns SET status=?3, ended_at=COALESCE(ended_at, ?2)
         WHERE status='running' AND invocation_id IN (
             SELECT id FROM agent_invocations WHERE supervising_run_id=?1
         )",
        params![run_id.as_str(), ended_at, outcome],
    )?;
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
             WHERE r.id=?1 AND r.state IN ('reserved', 'active')",
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
        .ok_or_else(|| {
            StoreError::InvalidAuthority("Run no longer holds execution authority".to_string())
        })?;
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
        ChildRef::Project(project_id) => {
            conn.query_row(
                "SELECT 1 FROM projects WHERE id=?1",
                [project_id.as_str()],
                |_| Ok(()),
            )?;
            Ok(WorkRef::Project(project_id.clone()))
        }
        ChildRef::Task(task_id) => {
            conn.query_row(
                "SELECT 1 FROM tasks WHERE id=?1",
                [task_id.as_str()],
                |_| Ok(()),
            )?;
            Ok(WorkRef::Task(task_id.clone()))
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
    boundary_seed_for_epoch_in(
        conn,
        work,
        epoch.current_basis.epoch_id,
        epoch.current_basis.revision,
    )
}

fn boundary_seed_for_epoch_in(
    conn: &Connection,
    work: &WorkRef,
    epoch_id: EpochId,
    revision: u64,
) -> StoreResult<BoundarySeed> {
    let applied = applied_basis_in(conn, &epoch_id)?
        .map(|basis| basis.revision as i64)
        .unwrap_or(-1);
    let mut statement = conn.prepare(
        "SELECT id, epoch_id, rev, author_kind, author_run_id, text, issued_at
         FROM steers WHERE epoch_id=?1 AND rev > ?2 AND rev <= ?3 ORDER BY rev",
    )?;
    let rows = statement.query_map(
        params![epoch_id.as_str(), applied, revision as i64],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
            ))
        },
    )?;
    let mut steers = Vec::new();
    for row in rows {
        steers.push(decode_steer(row?, work.clone())?);
    }
    Ok(BoundarySeed {
        basis: Basis { epoch_id, revision },
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
    parse_work_columns(row.0, row.1, row.2)
}

type SteerFields = (String, String, i64, String, Option<String>, String, i64);

fn decode_steer(fields: SteerFields, work: WorkRef) -> StoreResult<Steer> {
    let (id, epoch_id, revision, author_kind, author_run_id, text, issued_at) = fields;
    let author = match (author_kind.as_str(), author_run_id) {
        ("user", None) => Author::User,
        ("run", Some(id)) => Author::Run(RunId::parse(&id).map_err(invalid_durable)?),
        _ => {
            return Err(StoreError::InvalidData(
                "stored Steer author is inconsistent".to_string(),
            ))
        }
    };
    Ok(Steer {
        id: SteerId::parse(&id).map_err(invalid_durable)?,
        work,
        basis: Basis {
            epoch_id: EpochId::parse(&epoch_id).map_err(invalid_durable)?,
            revision: u64::try_from(revision).map_err(|_| {
                StoreError::InvalidData(format!("invalid stored Steer revision: {revision}"))
            })?,
        },
        author,
        text,
        issued_at: OffsetDateTime::from_unix_timestamp(issued_at).map_err(|error| {
            StoreError::InvalidData(format!("invalid Steer timestamp: {error}"))
        })?,
    })
}

fn parse_work_columns(
    wave_id: Option<String>,
    project_id: Option<String>,
    task_id: Option<String>,
) -> StoreResult<WorkRef> {
    match (wave_id, project_id, task_id) {
        (Some(id), None, None) => Ok(WorkRef::Wave(WaveId::parse(&id).map_err(invalid_durable)?)),
        (None, Some(id), None) => Ok(WorkRef::Project(
            ProjectId::parse(&id).map_err(invalid_durable)?,
        )),
        (None, None, Some(id)) => Ok(WorkRef::Task(TaskId::parse(&id).map_err(invalid_durable)?)),
        _ => Err(StoreError::InvalidData(
            "stored Epoch Work identity is inconsistent".to_string(),
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

#[cfg(test)]
mod durable_store_tests {
    use crate::durable::{
        AdvanceReceipt, AnswerRoute, AskExchange, AskId, Author, BoundaryState, Containment,
        ContainmentObservation, EpochId, InvocationRoute, RunAdvance, RunControl, RunState,
        RunTrigger, StopCause, WorkRef,
    };
    use crate::id::WaveId;
    use crate::project::ProjectId;
    use crate::store::sqlite::SqliteStore;
    use crate::store::StoreError;
    use crate::task::TaskId;
    use std::path::{Path, PathBuf};
    use time::OffsetDateTime;

    /// A registered Wave is the cheapest real Work: `upsert_wave` is the only
    /// public path that mints an Epoch, and Wave Work needs no PM binding.
    fn store_with_wave() -> (tempfile::TempDir, SqliteStore, WorkRef) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loopflow.db");
        let store = SqliteStore::new(&path).expect("open a fresh store");
        let wave_id = WaveId::new();
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute(
            "INSERT INTO waves (id, name, repo, created_at, parent_wave_id)
             VALUES (?1, 'probe', '/repo', 1700000000, NULL)",
            [wave_id.as_str()],
        )
        .unwrap();
        drop(conn);
        // Reach the crate-private spine builder the public upsert path calls.
        {
            let mut raw = rusqlite::Connection::open(&path).unwrap();
            let tx = raw.transaction().unwrap();
            super::create_wave_spine(&tx, &wave_id, "probe", "/repo", 1_700_000_000).unwrap();
            tx.commit().unwrap();
        }
        let work = WorkRef::Wave(wave_id);
        (dir, store, work)
    }

    #[test]
    fn activity_steers_span_historical_epochs() {
        let (dir, store, work) = store_with_wave();
        let first = store
            .append_steer(&work, &Author::User, "first direction", None)
            .unwrap();
        let second_epoch = EpochId::new();
        let conn = rusqlite::Connection::open(dir.path().join("loopflow.db")).unwrap();
        conn.execute(
            "UPDATE epochs SET state='done', terminal_at=1700000010
             WHERE id=?1",
            [first.steer.basis.epoch_id.as_str()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO epochs (
                id, number, wave_id, project_id, task_id, state, current_rev,
                created_at, terminal_at
             ) VALUES (?1, 2, ?2, NULL, NULL, 'open', 0, 1700000020, NULL)",
            rusqlite::params![second_epoch.as_str(), work.id()],
        )
        .unwrap();
        drop(conn);
        let second = store
            .append_steer(&work, &Author::User, "second direction", None)
            .unwrap();
        let conn = rusqlite::Connection::open(dir.path().join("loopflow.db")).unwrap();
        conn.execute(
            "UPDATE steers SET issued_at=1700000001 WHERE id=?1",
            [first.steer.id.as_str()],
        )
        .unwrap();
        conn.execute(
            "UPDATE steers SET issued_at=1700000021 WHERE id=?1",
            [second.steer.id.as_str()],
        )
        .unwrap();

        let steers = store.list_steers_since(0).unwrap();

        assert_eq!(
            steers
                .iter()
                .map(|steer| steer.text.as_str())
                .collect::<Vec<_>>(),
            ["second direction", "first direction"]
        );
        assert_eq!(steers[0].basis.epoch_id, second.steer.basis.epoch_id);
        assert_eq!(steers[1].basis.epoch_id, first.steer.basis.epoch_id);
        assert!(steers.iter().all(|steer| steer.work == work));
        assert_eq!(
            store
                .list_steers_since(1_700_000_010)
                .unwrap()
                .into_iter()
                .map(|steer| steer.id)
                .collect::<Vec<_>>(),
            [second.steer.id]
        );
    }

    fn start_invocation(
        store: &SqliteStore,
        work: &WorkRef,
    ) -> (crate::durable::RunLease, crate::durable::AgentInvocation) {
        let (_, lease) = store
            .reserve_run(work, &RunTrigger::User)
            .expect("reserve a Run");
        let cwd = PathBuf::from("/repo");
        store
            .advance_run(
                &lease,
                &RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: "probe".to_string(),
                    },
                    cwd: cwd.clone(),
                },
            )
            .expect("start a Run");
        let receipt = store
            .advance_run(
                &lease,
                &RunAdvance::InvocationStarting {
                    route: InvocationRoute {
                        provider: "codex".to_string(),
                        model: None,
                        account_id: None,
                    },
                    surface: "headless".to_string(),
                    resume_token: None,
                    answer_ask_id: None,
                },
            )
            .expect("start an AgentInvocation");
        let AdvanceReceipt::Invocation(invocation) = receipt else {
            panic!("expected AgentInvocation receipt")
        };
        (lease, invocation)
    }

    fn store_with_work_hierarchy() -> (tempfile::TempDir, SqliteStore, Vec<(WorkRef, WorkRef)>) {
        let (directory, store, wave) = store_with_wave();
        let path = directory.path().join("loopflow.db");
        let project_id = ProjectId::new();
        let project = WorkRef::Project(project_id.clone());
        let task_id = TaskId::new();
        let task = WorkRef::Task(task_id.clone());
        let mut conn = rusqlite::Connection::open(path).unwrap();
        let tx = conn.transaction().unwrap();
        tx.execute(
            "INSERT INTO projects (id, wave_id, external_project_id, created_at)
             VALUES (?1, ?2, 'linear-project', 1700000001)",
            rusqlite::params![project_id.as_str(), wave.id()],
        )
        .unwrap();
        super::inherit_placement(&tx, &project, Some(&wave), 1_700_000_001).unwrap();
        let project_epoch_id = EpochId::new();
        tx.execute(
            "INSERT INTO epochs (
                id, number, wave_id, project_id, task_id, state, current_rev,
                created_at, terminal_at
             ) VALUES (?1, 1, NULL, ?2, NULL, 'open', 0, 1700000001, NULL)",
            rusqlite::params![project_epoch_id.as_str(), project_id.as_str()],
        )
        .unwrap();
        super::insert_truth(
            &tx,
            &project_epoch_id,
            serde_json::json!({"external_project_id": "linear-project"}),
            OffsetDateTime::from_unix_timestamp(1_700_000_001).unwrap(),
        )
        .unwrap();
        tx.execute(
            "INSERT INTO tasks (
                id, project_id, external_issue_id, issue_identifier, created_at
             ) VALUES (?1, ?2, 'linear-issue', 'ENG-1', 1700000002)",
            rusqlite::params![task_id.as_str(), project_id.as_str()],
        )
        .unwrap();
        super::inherit_placement(&tx, &task, Some(&project), 1_700_000_002).unwrap();
        let task_epoch_id = EpochId::new();
        tx.execute(
            "INSERT INTO epochs (
                id, number, wave_id, project_id, task_id, state, current_rev,
                created_at, terminal_at
             ) VALUES (?1, 1, NULL, NULL, ?2, 'open', 0, 1700000002, NULL)",
            rusqlite::params![task_epoch_id.as_str(), task_id.as_str()],
        )
        .unwrap();
        super::insert_truth(
            &tx,
            &task_epoch_id,
            serde_json::json!({"issue_identifier": "ENG-1"}),
            OffsetDateTime::from_unix_timestamp(1_700_000_002).unwrap(),
        )
        .unwrap();
        tx.commit().unwrap();
        (
            directory,
            store,
            vec![
                (wave.clone(), wave.clone()),
                (project.clone(), wave),
                (task, project),
            ],
        )
    }

    fn start_turn(store: &SqliteStore, work: &WorkRef) -> crate::durable::TurnId {
        let (lease, invocation) = start_invocation(store, work);
        let receipt = store
            .advance_run(
                &lease,
                &RunAdvance::TurnStarting {
                    invocation_id: invocation.id,
                },
            )
            .unwrap();
        let AdvanceReceipt::Turn(turn) = receipt else {
            panic!("expected Turn receipt")
        };
        turn.id
    }

    #[test]
    fn status_ask_sql_prepares_against_the_migrated_schema() {
        let (directory, _, _) = store_with_wave();
        let conn = rusqlite::Connection::open(directory.path().join("loopflow.db")).unwrap();

        conn.prepare(super::HAS_PENDING_USER_ASK_FOR_WORK_SQL)
            .expect("runtime status SQL must prepare against the migration head");
    }

    #[test]
    fn pending_user_ask_status_resolves_every_work_kind_and_excludes_parent_routes() {
        let (directory, store, work_routes) = store_with_work_hierarchy();
        let path = directory.path().join("loopflow.db");

        for (work, parent) in &work_routes {
            let turn_id = start_turn(&store, work);
            let ask = AskExchange {
                id: AskId::new(),
                turn_id,
                route: AnswerRoute::User,
                question: format!("What blocks {}?", work.kind()),
                asked_at: OffsetDateTime::now_utc(),
                answer: None,
            };
            let conn = rusqlite::Connection::open(&path).unwrap();
            super::insert_ask(&conn, &ask).unwrap();

            for (candidate, _) in &work_routes {
                assert_eq!(
                    store.has_pending_user_ask_for_work(candidate).unwrap(),
                    candidate == work,
                    "the User Ask must belong only to its {} Epoch",
                    work.kind()
                );
            }

            conn.execute(
                "UPDATE ask_exchanges
                 SET route_kind='parent', route_work_kind=?2, route_work_id=?3
                 WHERE id=?1",
                rusqlite::params![ask.id.as_str(), parent.kind(), parent.id()],
            )
            .unwrap();
            assert!(!store.has_pending_user_ask_for_work(work).unwrap());
            let pending = store.pending_asks_for_parent(parent).unwrap();
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].route, AnswerRoute::Parent(parent.clone()));

            conn.execute("DELETE FROM ask_exchanges WHERE id=?1", [ask.id.as_str()])
                .unwrap();
        }
    }

    #[test]
    fn run_execution_shape_is_enforced_and_containment_is_immutable() {
        let (dir, store, work) = store_with_wave();
        let path = dir.path().join("loopflow.db");
        let (run, lease) = store
            .reserve_run(&work, &RunTrigger::User)
            .expect("reserve a Run");
        let conn = rusqlite::Connection::open(&path).unwrap();

        assert!(conn
            .execute(
                "UPDATE runs SET state='active' WHERE id=?1",
                [run.id.as_str()],
            )
            .is_err());

        store
            .advance_run(
                &lease,
                &RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: "probe".to_string(),
                    },
                    cwd: PathBuf::from("/repo"),
                },
            )
            .expect("start a Run with complete containment");

        assert!(conn
            .execute(
                "UPDATE runs SET containment_id='replacement' WHERE id=?1",
                [run.id.as_str()],
            )
            .is_err());
        assert!(conn
            .execute(
                "UPDATE runs SET containment_kind=NULL WHERE id=?1",
                [run.id.as_str()],
            )
            .is_err());

        store
            .stop_run(
                &lease,
                &StopCause::Requested,
                ContainmentObservation::Absent,
            )
            .expect("end the contained Run");
        let ended = store.run_by_id(&run.id).unwrap();
        assert_eq!(ended.state, RunState::Ended);
        assert_eq!(
            ended.containment,
            Some(Containment::Tmux {
                name: "probe".to_string()
            })
        );
        assert_eq!(ended.cwd, Some(PathBuf::from("/repo")));
        assert!(ended.started_at.is_some());
    }

    fn live_invocation_id(store: &SqliteStore, path: &Path) -> crate::durable::AgentInvocationId {
        let _ = store;
        let conn = rusqlite::Connection::open(path).unwrap();
        let id: String = conn
            .query_row("SELECT id FROM agent_invocations LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        crate::durable::AgentInvocationId::parse(&id).unwrap()
    }

    /// The provider account and resume token are observed after the harness
    /// starts, and the token can change mid-Run. This is the write path that
    /// makes AgentInvocation the durable owner of provider continuity.
    #[test]
    fn a_running_invocation_records_its_provider_continuity() {
        let (dir, store, work) = store_with_wave();
        let path = dir.path().join("loopflow.db");
        let (lease, _) = start_invocation(&store, &work);
        let invocation_id = live_invocation_id(&store, &path);

        let invocation = store
            .observe_invocation_provider(&lease, &invocation_id, None, Some("thread_abc"))
            .expect("record the observed provider");

        assert_eq!(invocation.resume_token.as_deref(), Some("thread_abc"));
        assert_eq!(
            store.current_run(&work).unwrap().unwrap().containment,
            Some(Containment::Tmux {
                name: "probe".to_string()
            }),
            "containment is spawn-time fencing evidence and must survive a provider observation"
        );
    }

    /// A later observation that carries no token must not erase the one already
    /// recorded — losing it would cost the Run its provider continuity across a
    /// relaunch, which is exactly what recovery depends on.
    #[test]
    fn an_empty_observation_never_erases_recorded_continuity() {
        let (dir, store, work) = store_with_wave();
        let path = dir.path().join("loopflow.db");
        let (lease, _) = start_invocation(&store, &work);
        let invocation_id = live_invocation_id(&store, &path);

        store
            .observe_invocation_provider(&lease, &invocation_id, None, Some("thread_abc"))
            .unwrap();
        let invocation = store
            .observe_invocation_provider(&lease, &invocation_id, None, None)
            .unwrap();

        assert_eq!(invocation.resume_token.as_deref(), Some("thread_abc"));
    }

    #[test]
    fn an_invocation_records_one_exact_account_route_and_rejects_route_drift() {
        let (dir, store, work) = store_with_wave();
        let path = dir.path().join("loopflow.db");
        let (lease, _) = start_invocation(&store, &work);
        let invocation_id = live_invocation_id(&store, &path);
        let work_account = crate::store::ProviderAccountId::parse("work").unwrap();
        let personal_account = crate::store::ProviderAccountId::parse("personal").unwrap();

        let invocation = store
            .observe_invocation_provider(&lease, &invocation_id, Some(&work_account), None)
            .unwrap();

        assert_eq!(invocation.route.account_id.as_deref(), Some("work"));
        assert!(store
            .observe_invocation_provider(&lease, &invocation_id, Some(&personal_account), None)
            .is_err());
    }

    /// Fail closed: once the Run is stopped it is no longer a writer, so it
    /// cannot report a provider observation. A dead process that wakes up and
    /// reports must be rejected, not allowed to touch its old AgentInvocation.
    #[test]
    fn a_stopped_run_cannot_record_a_provider_observation() {
        let (dir, store, work) = store_with_wave();
        let path = dir.path().join("loopflow.db");
        let (lease, _) = start_invocation(&store, &work);
        let invocation_id = live_invocation_id(&store, &path);
        store
            .stop_run(
                &lease,
                &StopCause::Requested,
                crate::durable::ContainmentObservation::Absent,
            )
            .expect("stop the Run with proven containment absence");

        let error = store
            .observe_invocation_provider(&lease, &invocation_id, None, Some("thread_zzz"))
            .expect_err("a stopped Run must not remain a writer");

        assert!(
            matches!(
                error,
                StoreError::InvalidAuthority(_) | StoreError::InvalidData(_)
            ),
            "expected an authority refusal, got {error:?}"
        );
    }

    #[test]
    fn an_interrupted_run_keeps_only_cleanup_authority() {
        let (dir, store, work) = store_with_wave();
        let (lease, _) = start_invocation(&store, &work);

        store
            .interrupt(None, &work, &lease.run_id)
            .expect("mark the Run for interruption");

        assert!(store.validate_run_lease(&lease).is_err());
        assert_eq!(
            store.run_control(&lease, None).unwrap(),
            Some(RunControl::Interrupt)
        );
        let conn = rusqlite::Connection::open(dir.path().join("loopflow.db")).unwrap();
        super::end_run_for_lease(&conn, &lease, BoundaryState::Interrupted)
            .expect("the stopped runner can finish cleanup");
        assert_eq!(
            store.run_by_id(&lease.run_id).unwrap().state,
            RunState::Ended
        );
    }

    #[test]
    fn proven_runner_loss_ends_its_incomplete_turns() {
        let (dir, store, work) = store_with_wave();
        let (lease, _) = start_invocation(&store, &work);
        let invocation_id = live_invocation_id(&store, &dir.path().join("loopflow.db"));
        let receipt = store
            .advance_run(
                &lease,
                &RunAdvance::TurnStarting {
                    invocation_id: invocation_id.clone(),
                },
            )
            .unwrap();
        let crate::durable::AdvanceReceipt::Turn(turn) = receipt else {
            panic!("expected Turn receipt")
        };

        store
            .recover_run(
                &lease.run_id,
                crate::durable::ContainmentObservation::Absent,
            )
            .expect("recover proven missing containment");

        let conn = rusqlite::Connection::open(dir.path().join("loopflow.db")).unwrap();
        let (status, ended_at): (String, Option<i64>) = conn
            .query_row(
                "SELECT status, ended_at FROM agent_turns WHERE id=?1",
                [turn.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "failed");
        assert!(ended_at.is_some());
    }
}
