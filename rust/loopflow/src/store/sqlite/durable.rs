use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use time::OffsetDateTime;

use crate::child_session::ChildRef;
use crate::durable::{
    AdvanceReceipt, AttentionRoute, Author, Basis, BoundarySeed, BoundaryState, ChildReview,
    Containment, ContainmentObservation, DoneProposal, DoneProposalId, Epoch, EpochId,
    EpochReceipt, EpochState, FlowPosition, Home, HomeId, InterruptReceipt, Launch, LaunchId,
    LaunchRoute, LaunchState, LaunchSurface, ProjectId, Review, Run, RunAdvance, RunId, RunLease,
    RunLeaseToken, RunState, RunTrigger, Send, SendId, SendState, SendVia, Steer, SteerId,
    SteerReceipt, StopCause, StopReceipt, TaskId, ToolResponseId, ToolResponseReceipt,
    ToolResponseWrite, Turn, TurnId, Wait, WaitId, WaitOn, WorkRef, WorkStatus,
};
use crate::id::WaveId;
use crate::project_session::ProjectSession;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::task::TaskSession;

use super::SqliteStore;

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

    pub fn home(&self, route: &str) -> StoreResult<Home> {
        let route = route.trim();
        if route.is_empty() {
            return Err(StoreError::InvalidData(
                "Home route cannot be empty".to_string(),
            ));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let now = now_unix();
        conn.execute(
            "INSERT INTO homes (id, route, created_at, observed_at)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(route) DO UPDATE SET observed_at=excluded.observed_at",
            params![HomeId::new().as_str(), route, now],
        )?;
        map_home(&conn, route)
    }

    pub fn reserve_run(
        &self,
        work: &WorkRef,
        home_id: &HomeId,
        trigger: &RunTrigger,
    ) -> StoreResult<(Run, RunLease)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let epoch = current_epoch_in(&tx, work)?;
        tx.query_row(
            "SELECT 1 FROM homes WHERE id=?1",
            [home_id.as_str()],
            |_| Ok(()),
        )?;
        resolve_wait_for_trigger(&tx, &epoch, trigger)?;
        let token = RunLeaseToken::new();
        let run = Run {
            id: RunId::new(),
            work: work.clone(),
            epoch_id: epoch.id.clone(),
            home_id: home_id.clone(),
            state: RunState::Reserved,
            trigger: trigger.clone(),
            retry_of: match trigger {
                RunTrigger::Recovery { prior_run_id } => Some(prior_run_id.clone()),
                _ => None,
            },
            created_at: OffsetDateTime::now_utc(),
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

    pub fn current_run(&self, work: &WorkRef) -> StoreResult<Option<Run>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let Ok(epoch) = current_epoch_in(&conn, work) else {
            return Ok(None);
        };
        run_for_epoch_in(&conn, &epoch.id)
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

    pub fn advance_run(
        &self,
        lease: &RunLease,
        advance: &RunAdvance,
    ) -> StoreResult<AdvanceReceipt> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut run = validate_run_lease(&tx, lease)?;
        let receipt = match advance {
            RunAdvance::LaunchStarting {
                route,
                containment,
                cwd,
                surface,
                opaque,
                resume_token,
            } => {
                if !matches!(run.state, RunState::Reserved | RunState::Active) {
                    return Err(StoreError::InvalidData(format!(
                        "Run {} cannot start a Launch while {:?}",
                        run.id, run.state
                    )));
                }
                if !cwd.is_absolute() {
                    return Err(StoreError::InvalidData(
                        "Launch cwd must be absolute".to_string(),
                    ));
                }
                if route.provider.trim().is_empty() || surface.trim().is_empty() {
                    return Err(StoreError::InvalidData(
                        "Launch provider and surface cannot be empty".to_string(),
                    ));
                }
                let basis = current_epoch_in(&tx, &run.work)?.current_basis;
                let launch = Launch {
                    id: LaunchId::new(),
                    run_id: run.id.clone(),
                    home_id: run.home_id.clone(),
                    route: route.clone(),
                    cwd: cwd.clone(),
                    surface: surface.clone(),
                    state: LaunchState::Starting,
                    containment: containment.clone(),
                    opaque_basis: opaque.then_some(basis),
                    resume_token: resume_token.clone(),
                    started_at: OffsetDateTime::now_utc(),
                    ended_at: None,
                };
                insert_control_launch(&tx, &run, &launch)?;
                tx.execute(
                    "UPDATE runs SET state='active' WHERE id=?1 AND state='reserved'",
                    [run.id.as_str()],
                )?;
                AdvanceReceipt::Launch(launch)
            }
            RunAdvance::LaunchLive { launch_id } => {
                require_launch_for_run(&tx, launch_id, &run.id)?;
                if tx.execute(
                    "UPDATE agent_launches SET launch_state='live'
                     WHERE id=?1 AND launch_state='starting'",
                    [launch_id.as_str()],
                )? == 0
                {
                    return Err(StoreError::InvalidData(format!(
                        "Launch {launch_id} is not starting"
                    )));
                }
                AdvanceReceipt::Launch(control_launch_in(&tx, launch_id)?)
            }
            RunAdvance::LaunchEnded { launch_id, outcome } => {
                if !outcome.is_terminal() {
                    return Err(StoreError::InvalidData(
                        "Launch handback must be terminal".to_string(),
                    ));
                }
                require_launch_for_run(&tx, launch_id, &run.id)?;
                let now = now_unix();
                tx.execute(
                    "UPDATE agent_launches
                     SET launch_state='ended', ended_at=COALESCE(ended_at, ?2),
                         outcome=?3, handback_state=?4,
                         attention_kind=NULL, attention_work_kind=NULL,
                         attention_work_id=NULL, attention_at=NULL
                     WHERE id=?1 AND launch_state != 'ended'",
                    params![
                        launch_id.as_str(),
                        now,
                        outcome.as_launch_outcome(),
                        handback_state(*outcome),
                    ],
                )?;
                AdvanceReceipt::Launch(control_launch_in(&tx, launch_id)?)
            }
            RunAdvance::TurnStarting { launch_id } => {
                require_live_launch_for_run(&tx, launch_id, &run.id)?;
                let basis = current_epoch_in(&tx, &run.work)?.current_basis;
                let ordinal: i64 = tx.query_row(
                    "SELECT COALESCE(MAX(ordinal), 0) + 1 FROM agent_turns WHERE launch_id=?1",
                    [launch_id.as_str()],
                    |row| row.get(0),
                )?;
                let turn = Turn {
                    id: TurnId::new(),
                    launch_id: launch_id.clone(),
                    basis: basis.clone(),
                    state: BoundaryState::Starting,
                    provider_turn_id: None,
                    root_output: None,
                    started_at: OffsetDateTime::now_utc(),
                    ended_at: None,
                };
                tx.execute(
                    "INSERT INTO agent_turns (
                        id, launch_id, ordinal, provider_turn_id, started_at, ended_at,
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
                        turn.launch_id.as_str(),
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
                let live: bool = tx.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM agent_launches
                        WHERE product_run_id=?1 AND launch_state != 'ended'
                     )",
                    [run.id.as_str()],
                    |row| row.get(0),
                )?;
                if live {
                    return Err(StoreError::InvalidData(
                        "Run cannot wait while owned containment is live".to_string(),
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
                tx.execute(
                    "UPDATE agent_launches
                     SET launch_state='ended', ended_at=COALESCE(ended_at, ?2),
                         outcome=CASE WHEN outcome='running' THEN 'failed' ELSE outcome END,
                         handback_state=COALESCE(handback_state, 'unknown'),
                         attention_kind=NULL, attention_work_kind=NULL,
                         attention_work_id=NULL, attention_at=NULL
                     WHERE product_run_id=?1 AND launch_state != 'ended'",
                    params![run.id.as_str(), now],
                )?;
                tx.execute(
                    "UPDATE runs SET state='ended', ended_at=?2, stop_reason=?3 WHERE id=?1",
                    params![run.id.as_str(), now, cause_json],
                )?;
            }
            ContainmentObservation::Present | ContainmentObservation::Unprovable => {
                tx.execute(
                    "UPDATE runs SET state='stopping', stop_reason=?2 WHERE id=?1",
                    params![run.id.as_str(), cause_json],
                )?;
                tx.execute(
                    "UPDATE agent_launches SET launch_state='stopping'
                     WHERE product_run_id=?1 AND launch_state IN ('starting', 'live')",
                    [run.id.as_str()],
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
        let Some(turn_id) = active_turn_id else {
            return Ok(None);
        };
        let interrupted = conn
            .query_row(
                "SELECT t.status='interrupted'
                 FROM agent_turns t
                 JOIN agent_launches l ON l.id=t.launch_id
                 WHERE t.id=?1 AND l.product_run_id=?2",
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
                epoch_id, flow, step, step_index, iteration, interactive, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(epoch_id) DO UPDATE SET
                flow=excluded.flow, step=excluded.step, step_index=excluded.step_index,
                iteration=excluded.iteration, interactive=excluded.interactive,
                updated_at=excluded.updated_at",
            params![
                position.epoch_id.as_str(),
                position.flow,
                position.step,
                i64::from(position.step_index),
                i64::from(position.iteration),
                position.interactive,
                position.updated_at.unix_timestamp(),
            ],
        )?;
        tx.commit()?;
        Ok(position.clone())
    }

    pub fn route_review(
        &self,
        lease: &RunLease,
        launch_id: &LaunchId,
        attention: &AttentionRoute,
    ) -> StoreResult<Review> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = validate_run_lease(&tx, lease)?;
        require_live_launch_for_run(&tx, launch_id, &run.id)?;
        let position = flow_position_in(&tx, &run.work, &run.epoch_id)?;
        if !position.interactive {
            return Err(StoreError::InvalidData(
                "current flow step is not interactive".to_string(),
            ));
        }
        validate_attention_route(&tx, &run.work, attention)?;
        let (attention_kind, attention_work_kind, attention_work_id) = match attention {
            AttentionRoute::User => ("user", None, None),
            AttentionRoute::Parent(work) => ("parent", Some(work.kind()), Some(work.id())),
        };
        tx.execute(
            "UPDATE agent_launches SET
                attention_kind=?2, attention_work_kind=?3,
                attention_work_id=?4, attention_at=COALESCE(attention_at, ?5)
             WHERE id=?1 AND launch_state='live'",
            params![
                launch_id.as_str(),
                attention_kind,
                attention_work_kind,
                attention_work_id,
                now_unix(),
            ],
        )?;
        let review = review_in(&tx, &run.work)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(review)
    }

    pub fn review(&self, work: &WorkRef) -> StoreResult<Option<Review>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        review_in(&conn, work)
    }

    pub fn launch_surface(&self, launch_id: &LaunchId) -> StoreResult<Option<LaunchSurface>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        launch_surface_in(&conn, launch_id)
    }

    pub fn launch_surfaces(&self, active_only: bool) -> StoreResult<Vec<LaunchSurface>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let sql = if active_only {
            "SELECT id FROM agent_launches
             WHERE product_run_id IS NOT NULL AND launch_state != 'ended'
             ORDER BY started_at, id"
        } else {
            "SELECT id FROM agent_launches
             WHERE product_run_id IS NOT NULL ORDER BY started_at, id"
        };
        let mut statement = conn.prepare(sql)?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| {
                let id = LaunchId::parse(&id).map_err(invalid_durable)?;
                launch_surface_in(&conn, &id)?.ok_or(StoreError::NotFound)
            })
            .collect()
    }

    pub fn handback_launch(
        &self,
        launch_id: &LaunchId,
        outcome: BoundaryState,
    ) -> StoreResult<LaunchSurface> {
        if !outcome.is_terminal() {
            return Err(StoreError::InvalidData(
                "Launch handback outcome must be terminal".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute(
            "UPDATE agent_launches
             SET launch_state='ended', ended_at=COALESCE(ended_at, ?2),
                 outcome=?3, handback_state=?4,
                 attention_kind=NULL, attention_work_kind=NULL,
                 attention_work_id=NULL, attention_at=NULL
             WHERE id=?1 AND product_run_id IS NOT NULL AND launch_state != 'ended'",
            params![
                launch_id.as_str(),
                now_unix(),
                outcome.as_launch_outcome(),
                handback_state(outcome),
            ],
        )? == 0
        {
            return Err(StoreError::NotFound);
        }
        let surface = launch_surface_in(&tx, launch_id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(surface)
    }

    pub fn child_attention(&self, parent: &WorkRef) -> StoreResult<Vec<ChildReview>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT e.wave_id, e.project_id, e.task_id
             FROM agent_launches l
             JOIN runs r ON r.id=l.product_run_id
             JOIN epochs e ON e.id=r.epoch_id
             WHERE l.launch_state='live' AND l.attention_kind='parent'
               AND l.attention_work_kind=?1 AND l.attention_work_id=?2
             ORDER BY l.attention_at, l.id",
        )?;
        let rows = statement.query_map(params![parent.kind(), parent.id()], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        let mut reviews = Vec::new();
        for row in rows {
            let work = work_from_parts(row?)?;
            if let Some(review) = review_in(&conn, &work)? {
                let latest_output = conn
                    .query_row(
                        "SELECT root_output FROM agent_turns
                         WHERE launch_id=?1 AND root_output IS NOT NULL
                         ORDER BY ordinal DESC LIMIT 1",
                        [review.launch_id.as_str()],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?;
                let evidence_json: String = conn.query_row(
                    "SELECT wt.payload_json FROM work_truth wt
                     WHERE wt.epoch_id=?1 ORDER BY wt.rev DESC LIMIT 1",
                    [review.basis.epoch_id.as_str()],
                    |row| row.get(0),
                )?;
                let status = work_status_in(&conn, &work)?;
                reviews.push(ChildReview {
                    review,
                    latest_output,
                    evidence: serde_json::json!({
                        "work": serde_json::from_str::<serde_json::Value>(&evidence_json)
                            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
                        "status": status,
                    }),
                });
            }
        }
        Ok(reviews)
    }

    pub fn close_review(
        &self,
        caller: Option<&RunLease>,
        work: &WorkRef,
        if_basis: &Basis,
    ) -> StoreResult<WorkStatus> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let review = review_in(&tx, work)?.ok_or(StoreError::NotFound)?;
        validate_basis(&review.basis, if_basis)?;
        validate_review_caller(&tx, caller, &review)?;
        tx.execute(
            "UPDATE work_flow_positions
             SET step_index=step_index+1, interactive=0, updated_at=?2
             WHERE epoch_id=?1 AND flow=?3 AND step=?4 AND step_index=?5 AND interactive=1",
            params![
                review.position.epoch_id.as_str(),
                now_unix(),
                review.position.flow,
                review.position.step,
                i64::from(review.position.step_index),
            ],
        )?;
        tx.execute(
            "UPDATE agent_launches SET attention_kind=NULL, attention_work_kind=NULL,
                attention_work_id=NULL, attention_at=NULL
             WHERE id=?1 AND attention_at=?2",
            params![review.launch_id.as_str(), review.opened_at.unix_timestamp()],
        )?;
        let status = work_status_in(&tx, work)?;
        tx.commit()?;
        Ok(status)
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
        let launch_id: String = tx.query_row(
            "SELECT id FROM agent_launches
             WHERE product_run_id=?1 AND launch_state IN ('starting', 'live')
             ORDER BY started_at DESC LIMIT 1",
            [run.id.as_str()],
            |row| row.get(0),
        )?;
        let turn_id = tx
            .query_row(
                "SELECT t.id FROM agent_turns t
                 WHERE t.launch_id=?1 AND t.status='running'
                 ORDER BY t.started_at DESC, t.ordinal DESC LIMIT 1",
                [&launch_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(turn_id) = &turn_id {
            tx.execute(
                "UPDATE agent_turns SET status='interrupted', ended_at=?2
                 WHERE id=?1 AND status='running'",
                params![turn_id, now_unix()],
            )?;
        } else {
            tx.execute(
                "UPDATE agent_launches SET launch_state='stopping', outcome='interrupted',
                    handback_state='interrupted'
                 WHERE id=?1 AND launch_state IN ('starting', 'live')",
                [&launch_id],
            )?;
        }
        let receipt = InterruptReceipt {
            run_id: run.id,
            launch_id: LaunchId::parse(&launch_id).map_err(invalid_durable)?,
            turn_id: turn_id
                .map(|id| TurnId::parse(&id).map_err(invalid_durable))
                .transpose()?,
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
        let live: bool = tx.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM agent_launches
                WHERE product_run_id=?1 AND launch_state != 'ended'
             )",
            [run.id.as_str()],
            |row| row.get(0),
        )?;
        if live {
            return Err(StoreError::InvalidData(
                "Run containment is not absent".to_string(),
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
            "UPDATE runs SET state='stopping', stop_reason=?2
             WHERE epoch_id=?1 AND state != 'ended'",
            params![epoch.id.as_str(), reason],
        )?;
        tx.execute(
            "UPDATE epochs SET state='abandoned', terminal_at=?2 WHERE id=?1 AND state='open'",
            params![epoch.id.as_str(), now],
        )?;
        tx.execute(
            "UPDATE agent_launches SET attention_kind=NULL, attention_work_kind=NULL,
                attention_work_id=NULL, attention_at=NULL
             WHERE product_run_id IN (SELECT id FROM runs WHERE epoch_id=?1)",
            [epoch.id.as_str()],
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
        let (epoch_id, revision) = match target {
            ChildRef::Project(session_id) => conn.query_row(
                "SELECT epoch_id, e.current_rev
                 FROM project_sessions s JOIN epochs e ON e.id=s.epoch_id
                 WHERE s.id=?1",
                [session_id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )?,
            ChildRef::Task(session_id) => conn.query_row(
                "SELECT epoch_id, e.current_rev
                 FROM task_sessions s JOIN epochs e ON e.id=s.epoch_id
                 WHERE s.id=?1",
                [session_id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )?,
        };
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

fn map_home(conn: &Connection, route: &str) -> StoreResult<Home> {
    conn.query_row(
        "SELECT id, route, created_at, observed_at FROM homes WHERE route=?1",
        [route],
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

fn validate_run_lease(conn: &Connection, lease: &RunLease) -> StoreResult<Run> {
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

fn validate_stop_lease(conn: &Connection, lease: &RunLease) -> StoreResult<Run> {
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

fn current_run_for_work_in(conn: &Connection, work: &WorkRef) -> StoreResult<Option<Run>> {
    let epoch = current_epoch_in(conn, work)?;
    run_for_epoch_in(conn, &epoch.id)
}

fn run_by_id_in(conn: &Connection, run_id: &RunId) -> StoreResult<Run> {
    let row = conn.query_row(
        "SELECT r.epoch_id, r.home_id, r.state, r.trigger_json, r.retry_of,
                r.created_at, r.ended_at, e.wave_id, e.project_id, e.task_id
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
        created_at: OffsetDateTime::from_unix_timestamp(row.5).map_err(invalid_durable)?,
        ended_at: row
            .6
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()
            .map_err(invalid_durable)?,
    })
}

fn insert_control_launch(tx: &Transaction<'_>, run: &Run, launch: &Launch) -> StoreResult<()> {
    let labels = work_labels(tx, &run.work)?;
    let cwd = launch.cwd.display().to_string();
    let (containment_kind, containment_id) = launch.containment.parts();
    let (opaque_epoch_id, opaque_basis_rev) = launch
        .opaque_basis
        .as_ref()
        .map(|basis| (Some(basis.epoch_id.as_str()), Some(basis.revision as i64)))
        .unwrap_or((None, None));
    tx.execute(
        "INSERT INTO agent_launches (
            id, run_id, process_id, started_at, ended_at, repo, worktree, wave,
            flow, skill, project, task, provider, model, surface, capture_status,
            incomplete_reason, outcome, artifact_dir, conversation_path,
            provider_events_path, provider_session_id, provider_session_path,
            conversation_event_count, conversation_bytes, product_run_id, home_id,
            account_id, launch_state, containment_kind, containment_id, resume_token,
            opaque_epoch_id, opaque_basis_rev
         ) VALUES (
            ?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, NULL, NULL, ?8, ?9, ?10, ?11,
            ?12, 'prompt_only', NULL, 'running', '', '', NULL, NULL, NULL, 0, 0,
            ?2, ?13, ?14, 'starting', ?15, ?16, ?17, ?18, ?19
         )",
        params![
            launch.id.as_str(),
            run.id.as_str(),
            containment_id,
            launch.started_at.unix_timestamp(),
            labels.repo,
            cwd,
            labels.wave,
            labels.project,
            labels.task,
            launch.route.provider,
            launch.route.model,
            launch.surface,
            launch.home_id.as_str(),
            launch.route.account_id,
            containment_kind,
            containment_id,
            launch.resume_token,
            opaque_epoch_id,
            opaque_basis_rev,
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

fn require_launch_for_run(
    conn: &Connection,
    launch_id: &LaunchId,
    run_id: &RunId,
) -> StoreResult<()> {
    conn.query_row(
        "SELECT 1 FROM agent_launches WHERE id=?1 AND product_run_id=?2",
        params![launch_id.as_str(), run_id.as_str()],
        |_| Ok(()),
    )
    .map_err(StoreError::from)
}

fn require_live_launch_for_run(
    conn: &Connection,
    launch_id: &LaunchId,
    run_id: &RunId,
) -> StoreResult<()> {
    conn.query_row(
        "SELECT 1 FROM agent_launches
         WHERE id=?1 AND product_run_id=?2 AND launch_state='live'",
        params![launch_id.as_str(), run_id.as_str()],
        |_| Ok(()),
    )
    .map_err(StoreError::from)
}

fn control_launch_in(conn: &Connection, launch_id: &LaunchId) -> StoreResult<Launch> {
    let row = conn.query_row(
        "SELECT product_run_id, home_id, provider, model, account_id, worktree,
                surface, launch_state, containment_kind, containment_id,
                opaque_epoch_id, opaque_basis_rev, resume_token, started_at, ended_at
         FROM agent_launches WHERE id=?1 AND product_run_id IS NOT NULL",
        [launch_id.as_str()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<i64>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, i64>(13)?,
                row.get::<_, Option<i64>>(14)?,
            ))
        },
    )?;
    let opaque_basis = match (row.10, row.11) {
        (Some(epoch_id), Some(revision)) => Some(Basis {
            epoch_id: EpochId::parse(&epoch_id).map_err(invalid_durable)?,
            revision: revision as u64,
        }),
        (None, None) => None,
        _ => {
            return Err(StoreError::InvalidData(
                "opaque Launch Basis is incomplete".to_string(),
            ))
        }
    };
    Ok(Launch {
        id: launch_id.clone(),
        run_id: RunId::parse(&row.0).map_err(invalid_durable)?,
        home_id: HomeId::parse(&row.1).map_err(invalid_durable)?,
        route: LaunchRoute {
            provider: row.2,
            model: row.3,
            account_id: row.4,
        },
        cwd: row.5.into(),
        surface: row.6,
        state: LaunchState::parse(&row.7).map_err(invalid_durable)?,
        containment: Containment::parse(&row.8, row.9).map_err(invalid_durable)?,
        opaque_basis,
        resume_token: row.12,
        started_at: OffsetDateTime::from_unix_timestamp(row.13).map_err(invalid_durable)?,
        ended_at: row
            .14
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()
            .map_err(invalid_durable)?,
    })
}

fn launch_surface_in(
    conn: &Connection,
    launch_id: &LaunchId,
) -> StoreResult<Option<LaunchSurface>> {
    let row = conn
        .query_row(
            "SELECT r.id, e.wave_id, e.project_id, e.task_id, h.route,
                    l.attention_kind, l.attention_work_kind, l.attention_work_id,
                    l.handback_state
             FROM agent_launches l
             JOIN runs r ON r.id=l.product_run_id
             JOIN epochs e ON e.id=r.epoch_id
             JOIN homes h ON h.id=l.home_id
             WHERE l.id=?1 AND l.product_run_id IS NOT NULL",
            [launch_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            },
        )
        .optional()?;
    let Some(row) = row else {
        return Ok(None);
    };
    let work = work_from_parts((row.1, row.2, row.3))?;
    let attention = match (row.5.as_deref(), row.6, row.7) {
        (None, None, None) => None,
        (Some("user"), None, None) => Some(AttentionRoute::User),
        (Some("parent"), Some(kind), Some(id)) => {
            Some(AttentionRoute::Parent(parse_work_ref(&kind, &id)?))
        }
        _ => {
            return Err(StoreError::InvalidData(
                "stored Launch attention route is inconsistent".to_string(),
            ))
        }
    };
    let handback = row
        .8
        .as_deref()
        .map(BoundaryState::parse_handback)
        .transpose()
        .map_err(invalid_durable)?;
    let launch = control_launch_in(conn, launch_id)?;
    let attach_argv = match &launch.containment {
        Containment::Tmux { name } => Some(vec![
            "tmux".to_string(),
            "attach-session".to_string(),
            "-t".to_string(),
            name.clone(),
        ]),
        Containment::ProcessGroup { .. } => None,
    };
    debug_assert_eq!(launch.run_id.as_str(), row.0);
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
    Ok(Some(LaunchSurface {
        launch,
        work,
        wave_id,
        home_route: row.4,
        attention,
        handback,
        attach_argv,
    }))
}

fn require_turn_for_run(conn: &Connection, turn_id: &TurnId, run_id: &RunId) -> StoreResult<()> {
    conn.query_row(
        "SELECT 1 FROM agent_turns t
         JOIN agent_launches l ON l.id=t.launch_id
         WHERE t.id=?1 AND l.product_run_id=?2",
        params![turn_id.as_str(), run_id.as_str()],
        |_| Ok(()),
    )
    .map_err(StoreError::from)
}

fn control_turn_in(conn: &Connection, turn_id: &TurnId) -> StoreResult<Turn> {
    let row = conn.query_row(
        "SELECT launch_id, epoch_id, basis_rev, status, provider_turn_id,
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
        launch_id: LaunchId::parse(&row.0).map_err(invalid_durable)?,
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

fn flow_position_in(
    conn: &Connection,
    work: &WorkRef,
    epoch_id: &EpochId,
) -> StoreResult<FlowPosition> {
    conn.query_row(
        "SELECT flow, step, step_index, iteration, interactive, updated_at
         FROM work_flow_positions WHERE epoch_id=?1",
        [epoch_id.as_str()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, bool>(4)?,
                row.get::<_, i64>(5)?,
            ))
        },
    )
    .map_err(StoreError::from)
    .and_then(|row| {
        Ok(FlowPosition {
            work: work.clone(),
            epoch_id: epoch_id.clone(),
            flow: row.0,
            step: row.1,
            step_index: row.2 as u32,
            iteration: row.3 as u32,
            interactive: row.4,
            updated_at: OffsetDateTime::from_unix_timestamp(row.5).map_err(invalid_durable)?,
        })
    })
}

fn validate_attention_route(
    conn: &Connection,
    work: &WorkRef,
    attention: &AttentionRoute,
) -> StoreResult<()> {
    match attention {
        AttentionRoute::User => Ok(()),
        AttentionRoute::Parent(parent) if parent_work(conn, work)?.as_ref() == Some(parent) => {
            Ok(())
        }
        AttentionRoute::Parent(_) => Err(StoreError::InvalidAuthority(
            "attention may route only to immediate parent Work".to_string(),
        )),
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

fn review_in(conn: &Connection, work: &WorkRef) -> StoreResult<Option<Review>> {
    let Ok(epoch) = current_epoch_in(conn, work) else {
        return Ok(None);
    };
    let row = conn
        .query_row(
            "SELECT l.id, l.attention_kind, l.attention_work_kind,
                    l.attention_work_id, l.attention_at
             FROM runs r JOIN agent_launches l ON l.product_run_id=r.id
             WHERE r.epoch_id=?1 AND r.state='active' AND l.launch_state='live'
               AND l.attention_kind IS NOT NULL
             ORDER BY l.attention_at LIMIT 1",
            [epoch.id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((launch_id, kind, parent_kind, parent_id, opened_at)) = row else {
        return Ok(None);
    };
    let position = flow_position_in(conn, work, &epoch.id)?;
    if !position.interactive {
        return Ok(None);
    }
    let attention = match (kind.as_str(), parent_kind, parent_id) {
        ("user", None, None) => AttentionRoute::User,
        ("parent", Some(kind), Some(id)) => AttentionRoute::Parent(parse_work_ref(&kind, &id)?),
        _ => {
            return Err(StoreError::InvalidData(
                "stored attention route is inconsistent".to_string(),
            ))
        }
    };
    Ok(Some(Review {
        work: work.clone(),
        launch_id: LaunchId::parse(&launch_id).map_err(invalid_durable)?,
        basis: epoch.current_basis,
        position,
        attention,
        opened_at: OffsetDateTime::from_unix_timestamp(opened_at).map_err(invalid_durable)?,
    }))
}

fn validate_review_caller(
    conn: &Connection,
    caller: Option<&RunLease>,
    review: &Review,
) -> StoreResult<()> {
    match (&review.attention, caller) {
        (AttentionRoute::User, None) => Ok(()),
        (AttentionRoute::Parent(parent), Some(lease)) => {
            let run = validate_run_lease(conn, lease)?;
            if &run.work == parent {
                Ok(())
            } else {
                Err(StoreError::InvalidAuthority(
                    "Run does not own this child attention route".to_string(),
                ))
            }
        }
        _ => Err(StoreError::InvalidAuthority(
            "caller does not own this attention route".to_string(),
        )),
    }
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

fn work_status_in(conn: &Connection, work: &WorkRef) -> StoreResult<WorkStatus> {
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

pub(crate) fn create_wave_spine(
    tx: &Transaction<'_>,
    wave_id: &WaveId,
    name: &str,
    repo: &str,
    created_at: i64,
) -> StoreResult<()> {
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

pub(crate) fn create_project_spine(
    tx: &Transaction<'_>,
    session: &ProjectSession,
) -> StoreResult<()> {
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
    import_project_run(tx, session)?;
    Ok(())
}

pub(crate) fn create_task_spine(tx: &Transaction<'_>, session: &TaskSession) -> StoreResult<()> {
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
    import_task_run(tx, session)?;
    Ok(())
}

fn import_project_run(tx: &Transaction<'_>, session: &ProjectSession) -> StoreResult<()> {
    let Some(process) = session.latest_process.as_ref() else {
        return Ok(());
    };
    import_run_for_child(
        tx,
        &ChildRef::Project(session.id.clone()),
        process.generation,
        process.state,
    )
}

fn import_task_run(tx: &Transaction<'_>, session: &TaskSession) -> StoreResult<()> {
    let Some(process) = session.latest_process.as_ref() else {
        return Ok(());
    };
    import_run_for_child(
        tx,
        &ChildRef::Task(session.id.clone()),
        process.generation,
        process.state,
    )
}

fn import_run_for_child(
    tx: &Transaction<'_>,
    target: &ChildRef,
    generation: u32,
    lease_state: crate::child_session::ChildLeaseState,
) -> StoreResult<()> {
    use crate::child_session::ChildLeaseState;

    let state = match lease_state {
        ChildLeaseState::Legacy | ChildLeaseState::Active => "active",
        ChildLeaseState::Reserved => "reserved",
        ChildLeaseState::Revoked => "stopping",
        ChildLeaseState::Finished => return Ok(()),
    };
    let work = work_for_child_in(tx, target)?;
    let epoch = current_epoch_in(tx, &work)?;
    let home_id: String = tx.query_row(
        "SELECT id FROM homes ORDER BY created_at LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    let trigger_json = serde_json::to_string(&RunTrigger::Input {
        basis: epoch.current_basis,
    })
    .expect("run trigger must serialize");
    // A legacy body never receives product Run authority by derivation. The
    // imported slot stays fenced until containment is reconciled, and only a
    // newly reserved Run gets a capability its process can inherit.
    let lease_hash = crate::durable::RunLeaseToken::new().hash();
    tx.execute(
        "INSERT INTO runs (
            id, epoch_id, home_id, state, trigger_json, lease_hash,
            lease_generation, source_kind, source_id, created_at, ended_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL)",
        params![
            RunId::new().as_str(),
            epoch.id.as_str(),
            home_id,
            state,
            trigger_json,
            lease_hash,
            i64::from(generation),
            target.target_kind(),
            target.target_id(),
            now_unix(),
        ],
    )?;
    Ok(())
}

pub(crate) fn reserve_run_for_child(
    tx: &Transaction<'_>,
    target: &ChildRef,
    generation: u32,
    trigger: Option<&RunTrigger>,
) -> StoreResult<crate::durable::RunLeaseToken> {
    let work = work_for_child_in(tx, target)?;
    let epoch = current_epoch_in(tx, &work)?;
    let home_id: String = tx.query_row(
        "SELECT id FROM homes ORDER BY created_at LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    let default_trigger = RunTrigger::Input {
        basis: epoch.current_basis.clone(),
    };
    let trigger_json = serde_json::to_string(trigger.unwrap_or(&default_trigger))
        .expect("run trigger must serialize");
    let token = crate::durable::RunLeaseToken::new();
    let lease_hash = token.hash();
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
    Ok(token)
}

pub(crate) fn activate_run_for_child(
    tx: &Transaction<'_>,
    target: &ChildRef,
    generation: u32,
) -> StoreResult<()> {
    if tx.execute(
        "UPDATE runs SET state='active'
         WHERE source_kind=?1 AND source_id=?2 AND lease_generation=?3
           AND state='reserved'",
        params![
            target.target_kind(),
            target.target_id(),
            i64::from(generation)
        ],
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
    conn.execute(
        "UPDATE runs SET state='ended', ended_at=?4
         WHERE source_kind=?1 AND source_id=?2 AND lease_generation=?3
           AND state != 'ended'",
        params![
            target.target_kind(),
            target.target_id(),
            i64::from(generation),
            now_unix()
        ],
    )?;
    Ok(())
}

pub(crate) fn fence_run_for_child(
    conn: &Connection,
    target: &ChildRef,
    generation: u32,
) -> StoreResult<()> {
    if conn.execute(
        "UPDATE runs SET state='stopping'
         WHERE source_kind=?1 AND source_id=?2 AND lease_generation=?3
           AND state IN ('reserved', 'active')",
        params![
            target.target_kind(),
            target.target_id(),
            i64::from(generation)
        ],
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
        "SELECT MAX(revision) FROM (
            SELECT basis_rev AS revision FROM agent_turns
            WHERE epoch_id=?1 AND status='completed'
            UNION ALL
            SELECT opaque_basis_rev AS revision FROM agent_launches
            WHERE opaque_epoch_id=?1 AND handback_state='succeeded'
         )",
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
        "SELECT id, rev, author_kind, author_run_id, text, issued_at
         FROM steers WHERE epoch_id=?1 AND rev > ?2 AND rev <= ?3 ORDER BY rev",
    )?;
    let rows = statement.query_map(
        params![epoch_id.as_str(), applied, revision as i64],
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
                epoch_id: epoch_id.clone(),
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
