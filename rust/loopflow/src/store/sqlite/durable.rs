use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use time::OffsetDateTime;

use crate::child::ChildRef;
use crate::durable::{
    AbandonReceipt, Ask, AskBody, AskClaim, AskId, AskOrigin, AskResult, AskState, AskTarget,
    Author, Home, HomeId, Placement, ProjectChildControlBasis, ProjectChildControlToken, ProjectId,
    RunId, Steer, SteerId, SteerReceipt, TaskId, ToolResponseId, ToolResponseReceipt,
    ToolResponseWrite, WorkRef, WorkStatus,
};
use crate::id::WaveId;
use crate::store::durable::{AskCommentTransition, AskCommentWrite};
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::work::project::Project;
use crate::work::task::Task;

use super::SqliteStore;

const HAS_PENDING_USER_ASK_FOR_WORK_SQL: &str = "SELECT EXISTS(
        SELECT 1 FROM ask_exchanges a
        WHERE a.target_kind='user' AND a.state IN ('queued', 'claimed')
          AND a.origin_work_kind=?1 AND a.origin_work_id=?2
     )";

impl SqliteStore {
    pub(crate) fn task_issue_identifier(
        &self,
        external_issue_id: &str,
    ) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT issue_identifier FROM tasks WHERE external_issue_id=?1",
            [external_issue_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
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

    pub fn set_work_enabled(&self, work: &WorkRef, enabled: bool) -> StoreResult<Placement> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        placement_in(&tx, work)?;
        let enabled = if enabled { 1_i64 } else { 0_i64 };
        match work {
            WorkRef::Wave(id) => tx.execute(
                "UPDATE work_placements SET enabled=?2 WHERE wave_id=?1",
                params![id.as_str(), enabled],
            )?,
            WorkRef::Project(id) => tx.execute(
                "UPDATE work_placements SET enabled=?2 WHERE project_id=?1",
                params![id.as_str(), enabled],
            )?,
            WorkRef::Task(id) => tx.execute(
                "UPDATE work_placements SET enabled=?2 WHERE task_id=?1",
                params![id.as_str(), enabled],
            )?,
        };
        let placement = placement_in(&tx, work)?;
        tx.commit()?;
        Ok(placement)
    }

    pub(crate) fn place_work(&self, work: &WorkRef, home_id: &HomeId) -> StoreResult<Placement> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_ready_work(&tx, work)?;
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
        write_placement(&tx, work, home_id, now_unix())?;
        let placement = placement_in(&tx, work)?;
        tx.commit()?;
        Ok(placement)
    }

    pub fn create_ask(
        &self,
        origin: &AskOrigin,
        request: &AskBody,
        target: &AskTarget,
    ) -> StoreResult<Ask> {
        validate_ask_body(request)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_ask_origin(&tx, origin)?;
        validate_requested_target(&tx, &origin.work, target)?;
        if let AskBody::FlowStep {
            flow,
            node_id,
            skill,
            iteration,
        } = request
        {
            if let Some(existing) =
                flow_ask_in(&tx, &origin.work, flow, node_id, skill, *iteration, target)?
            {
                tx.commit()?;
                return Ok(existing);
            }
        }
        let ask = Ask {
            id: AskId::new(),
            origin: origin.clone(),
            target: target.clone(),
            request: request.clone(),
            state: AskState::Queued,
            active_run_id: None,
            ready_at: None,
            presented_at: None,
            result: None,
            terminal_author: None,
            asked_at: OffsetDateTime::from_unix_timestamp(now_unix()).map_err(invalid_durable)?,
            terminal_at: None,
        };
        insert_ask(&tx, &ask)?;
        enqueue_ask_comment(&tx, &ask)?;
        tx.commit()?;
        Ok(ask)
    }

    pub fn ask_by_id(&self, ask_id: &AskId) -> StoreResult<Ask> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        ask_by_id_in(&conn, ask_id)
    }

    pub fn pending_asks(&self, target: &AskTarget) -> StoreResult<Vec<Ask>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        query_asks(&conn, AskScope::Target(target))
    }

    pub fn claim_ask(&self, ask_id: &AskId) -> StoreResult<AskClaim> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let ask = ask_by_id_in(&tx, ask_id)?;
        require_ready_ask_work(&tx, ask_id)?;
        if ask.state.is_terminal() {
            return Err(StoreError::InvalidAuthority(format!(
                "Ask {ask_id} is already terminal"
            )));
        }
        if ask.state == AskState::Claimed {
            let run_id = ask.active_run_id.as_ref().ok_or_else(|| {
                StoreError::InvalidData(format!("claimed Ask {ask_id} has no active session"))
            })?;
            tx.commit()?;
            return Ok(AskClaim {
                run_id: run_id.clone(),
                needs_launch: false,
            });
        }

        let run_id = RunId::new();
        if tx.execute(
            "UPDATE ask_exchanges
             SET state='claimed', active_run_id=?2, ready_at=?3
             WHERE id=?1 AND state='queued' AND active_run_id IS NULL",
            params![ask_id.as_str(), run_id.as_str(), now_unix()],
        )? != 1
        {
            return Err(StoreError::InvalidAuthority(format!(
                "Ask {ask_id} was claimed concurrently"
            )));
        }
        tx.commit()?;
        Ok(AskClaim {
            run_id,
            needs_launch: true,
        })
    }

    pub fn mark_ask_presented(&self, ask_id: &AskId, run_id: &RunId) -> StoreResult<Ask> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute(
            "UPDATE ask_exchanges SET presented_at=COALESCE(presented_at, ?3)
             WHERE id=?1 AND state='claimed' AND active_run_id=?2
               AND ready_at IS NOT NULL",
            params![ask_id.as_str(), run_id.as_str(), now_unix()],
        )? != 1
        {
            return Err(StoreError::InvalidAuthority(format!(
                "Ask Run {run_id} is not attachable"
            )));
        }
        ask_by_id_in(&conn, ask_id)
    }

    pub fn interrupt_ask_on_interrupt(&self, ask_id: &AskId, run_id: &RunId) -> StoreResult<Ask> {
        self.close_ask_run(ask_id, run_id, Some("Ask process interrupted"))
    }

    pub fn settle_ask(
        &self,
        ask_id: &AskId,
        run_id: &RunId,
        result: &AskResult,
    ) -> StoreResult<Ask> {
        if matches!(result, AskResult::Cancelled { .. }) {
            return Err(StoreError::InvalidAuthority(
                "an Ask Run cannot cancel its Ask".to_string(),
            ));
        }
        validate_ask_result(result)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let ask = ask_by_id_in(&tx, ask_id)?;
        if ask.state.is_terminal() {
            if ask.terminal_author == Some(Author::Run(run_id.clone()))
                && ask.result.as_ref() == Some(result)
            {
                tx.commit()?;
                return Ok(ask);
            }
            return Err(StoreError::InvalidAuthority(format!(
                "Ask {ask_id} was already settled"
            )));
        }
        if ask.state != AskState::Claimed || ask.active_run_id.as_ref() != Some(run_id) {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {run_id} no longer owns Ask {ask_id}"
            )));
        }
        if ask.presented_at.is_none() {
            return Err(StoreError::InvalidAuthority(format!(
                "Ask Run {run_id} was not presented"
            )));
        }
        let now = now_unix();
        let author = match ask.target {
            AskTarget::User => Author::User,
            AskTarget::Parent(_) => Author::Run(run_id.clone()),
        };
        let settled = write_terminal_ask_in(&tx, &ask, result, &author, now)?;
        enqueue_ask_result_comment(&tx, &settled)?;
        tx.commit()?;
        Ok(settled)
    }

    pub fn release_ask(
        &self,
        ask_id: &AskId,
        run_id: &RunId,
        reason: Option<&str>,
    ) -> StoreResult<Ask> {
        self.close_ask_run(ask_id, run_id, reason)
    }

    fn close_ask_run(
        &self,
        ask_id: &AskId,
        run_id: &RunId,
        reason: Option<&str>,
    ) -> StoreResult<Ask> {
        let _reason = normalize_optional_reason(reason)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let ask = ask_by_id_in(&tx, ask_id)?;
        if ask.state != AskState::Claimed || ask.active_run_id.as_ref() != Some(run_id) {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {run_id} no longer owns Ask {ask_id}"
            )));
        }
        requeue_ask_in(&tx, &ask, run_id)?;
        let ask = ask_by_id_in(&tx, &ask.id)?;
        tx.commit()?;
        Ok(ask)
    }

    pub fn escalate_ask(&self, ask_id: &AskId, run_id: &RunId) -> StoreResult<Ask> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let ask = ask_by_id_in(&tx, ask_id)?;
        if !matches!(ask.target, AskTarget::Parent(_)) {
            return Err(StoreError::InvalidData(format!(
                "Ask {} already targets the User",
                ask.id
            )));
        }
        if ask.state != AskState::Claimed || ask.active_run_id.as_ref() != Some(run_id) {
            return Err(StoreError::InvalidAuthority(format!(
                "Run {run_id} no longer owns Ask {}",
                ask.id
            )));
        }
        tx.execute(
            "UPDATE ask_exchanges
             SET target_kind='user', target_work_kind=NULL, target_work_id=NULL,
                 state='queued', active_run_id=NULL, ready_at=NULL, presented_at=NULL
             WHERE id=?1 AND state='claimed' AND active_run_id=?2",
            params![ask.id.as_str(), run_id.as_str()],
        )?;
        let ask = ask_by_id_in(&tx, &ask.id)?;
        tx.commit()?;
        Ok(ask)
    }

    pub fn escalate_queued_ask(&self, ask_id: &AskId) -> StoreResult<Ask> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let ask = ask_by_id_in(&tx, ask_id)?;
        if !matches!(ask.target, AskTarget::Parent(_)) || ask.state != AskState::Queued {
            return Err(StoreError::InvalidAuthority(format!(
                "Ask {ask_id} is not a queued parent request"
            )));
        }
        tx.execute(
            "UPDATE ask_exchanges
             SET target_kind='user', target_work_kind=NULL, target_work_id=NULL
             WHERE id=?1 AND state='queued'",
            [ask_id.as_str()],
        )?;
        let ask = ask_by_id_in(&tx, ask_id)?;
        tx.commit()?;
        Ok(ask)
    }

    pub fn cancel_ask(&self, ask_id: &AskId, reason: &str) -> StoreResult<Ask> {
        let reason = normalize_reason(reason, "Ask cancellation reason")?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let ask = ask_by_id_in(&tx, ask_id)?;
        let author = Author::User;
        if ask.state.is_terminal() {
            if ask.result
                == Some(AskResult::Cancelled {
                    reason: reason.clone(),
                })
            {
                tx.commit()?;
                return Ok(ask);
            }
            return Err(StoreError::InvalidAuthority(format!(
                "Ask {ask_id} is already terminal"
            )));
        }
        let now = now_unix();
        let ask = write_terminal_ask_in(&tx, &ask, &AskResult::Cancelled { reason }, &author, now)?;
        enqueue_ask_result_comment(&tx, &ask)?;
        tx.commit()?;
        Ok(ask)
    }

    pub fn request_intervention(
        &self,
        origin: &AskOrigin,
        prompt: &str,
        user: bool,
    ) -> StoreResult<Ask> {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Err(StoreError::InvalidData(
                "Ask request cannot be empty".to_string(),
            ));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let request = AskBody::Intervention {
            prompt: prompt.to_string(),
        };
        let target = if user {
            AskTarget::User
        } else {
            let Some(parent) = parent_work(&conn, &origin.work)? else {
                return Err(StoreError::InvalidData(format!(
                    "root {} {} has no parent; use `lf ask --user` for genuine User intervention",
                    origin.work.kind(),
                    origin.work.id()
                )));
            };
            AskTarget::Parent(parent)
        };
        drop(conn);
        self.create_ask(origin, &request, &target)
    }

    pub fn asks_for_work(&self, work: &WorkRef) -> StoreResult<Vec<Ask>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        query_asks(&conn, AskScope::OriginWork(work))
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

    pub fn has_pending_user_ask_for_work(&self, work: &WorkRef) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            HAS_PENDING_USER_ASK_FOR_WORK_SQL,
            params![work.kind(), work.id()],
            |row| row.get(0),
        )
        .map_err(StoreError::from)
    }

    pub fn abandon(&self, work: &WorkRef, reason: &str) -> StoreResult<AbandonReceipt> {
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(StoreError::InvalidData(
                "abandon reason cannot be empty".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = now_unix();
        cancel_pending_asks_for_work(&tx, work, "owning Work abandoned", now)?;
        let (table, id) = work_table(work);
        if tx.execute(
            &format!(
                "UPDATE {table} SET work_state='abandoned', work_terminal_at=?2
                 WHERE id=?1 AND work_state='ready'"
            ),
            params![id, now],
        )? != 1
        {
            return Err(StoreError::InvalidAuthority(format!(
                "{} {} is not ready",
                work.kind(),
                work.id()
            )));
        }
        tx.commit()?;
        Ok(AbandonReceipt {
            work: work.clone(),
            reason: reason.to_string(),
            abandoned_at: OffsetDateTime::from_unix_timestamp(now)
                .expect("current Unix timestamp must be valid"),
        })
    }

    pub fn work_status(&self, work: &WorkRef) -> StoreResult<WorkStatus> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        work_status_in(&conn, work)
    }

    pub fn work_for_child(&self, target: &ChildRef) -> StoreResult<WorkRef> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        work_for_child_in(&conn, target)
    }

    pub fn work_steers(&self, work: &WorkRef) -> StoreResult<Vec<Steer>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        work_steers_in(&conn, work)
    }

    pub(crate) fn work_steers_for_child(&self, target: &ChildRef) -> StoreResult<Vec<Steer>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let work = work_for_child_in(&conn, target)?;
        work_steers_in(&conn, &work)
    }

    pub(crate) fn begin_project_child_control(
        &self,
        project_id: &ProjectId,
        run_id: &RunId,
        basis: &ProjectChildControlBasis,
    ) -> StoreResult<ProjectChildControlToken> {
        let token = ProjectChildControlToken::new();
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_project_child_control_basis(&tx, project_id, basis)?;
        tx.execute(
            "INSERT INTO project_child_controls (
                project_id, run_id, token_hash, flow, step, step_index, iteration, steer_sequence
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(project_id) DO UPDATE SET
                run_id=excluded.run_id,
                token_hash=excluded.token_hash,
                flow=excluded.flow,
                step=excluded.step,
                step_index=excluded.step_index,
                iteration=excluded.iteration,
                steer_sequence=excluded.steer_sequence",
            params![
                project_id.as_str(),
                run_id.as_str(),
                token.hash(),
                basis.flow,
                basis.step,
                i64::from(basis.step_index),
                i64::from(basis.iteration),
                control_steer_sequence(basis)?,
            ],
        )?;
        tx.commit()?;
        Ok(token)
    }

    pub(crate) fn advance_project_child_control(
        &self,
        project_id: &ProjectId,
        run_id: &RunId,
        token: &ProjectChildControlToken,
        basis: &ProjectChildControlBasis,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_project_child_control_holder(&tx, project_id, run_id, token)?;
        validate_project_child_control_basis(&tx, project_id, basis)?;
        tx.execute(
            "UPDATE project_child_controls
             SET flow=?4, step=?5, step_index=?6, iteration=?7, steer_sequence=?8
             WHERE project_id=?1 AND run_id=?2 AND token_hash=?3",
            params![
                project_id.as_str(),
                run_id.as_str(),
                token.hash(),
                basis.flow,
                basis.step,
                i64::from(basis.step_index),
                i64::from(basis.iteration),
                control_steer_sequence(basis)?,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn authorize_project_child_control(
        &self,
        task_id: &TaskId,
        run_id: &RunId,
        token: &ProjectChildControlToken,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let project_id = conn.query_row(
            "SELECT project_id FROM tasks WHERE id=?1",
            [task_id.as_str()],
            |row| row.get::<_, String>(0),
        )?;
        let project_id = ProjectId::parse(&project_id).map_err(invalid_durable)?;
        validate_project_child_control_holder(&conn, &project_id, run_id, token)?;
        require_ready_work(&conn, &WorkRef::Project(project_id.clone()))?;
        let steer_sequence = conn.query_row(
            "SELECT steer_sequence FROM project_child_controls WHERE project_id=?1",
            [project_id.as_str()],
            |row| row.get::<_, i64>(0),
        )?;
        let current_sequence = project_steer_sequence(&conn, &project_id)?;
        if steer_sequence != current_sequence {
            return Err(StoreError::InvalidAuthority(format!(
                "Project {project_id} child-control basis is stale after new direction; continue Project Work to its next phase before resuming child Tasks"
            )));
        }
        Ok(())
    }

    pub(crate) fn release_project_child_control(
        &self,
        project_id: &ProjectId,
        run_id: &RunId,
        token: &ProjectChildControlToken,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "DELETE FROM project_child_controls
             WHERE project_id=?1 AND run_id=?2 AND token_hash=?3",
            params![project_id.as_str(), run_id.as_str(), token.hash()],
        )?;
        Ok(())
    }

    pub fn append_steer(
        &self,
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
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let receipt = Self::append_steer_in(&tx, work, author, text)?;
        tx.commit()?;
        Ok(receipt)
    }

    /// Every durable Steer recorded at or after `since`, newest first.
    ///
    pub fn list_steers_since(&self, since: i64) -> StoreResult<Vec<Steer>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT id, work_kind, work_id, author_kind, author_run_id, text, issued_at
             FROM steers WHERE issued_at >= ?1 ORDER BY issued_at DESC, id DESC",
        )?;
        let rows = statement.query_map([since], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })?;
        let mut steers = Vec::new();
        for row in rows {
            let (id, work_kind, work_id, author_kind, author_run_id, text, issued_at) = row?;
            let work = parse_work_ref(&work_kind, &work_id)?;
            steers.push(decode_steer(
                (id, author_kind, author_run_id, text, issued_at),
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
        require_ready_work(tx, work)?;
        let sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM steers
             WHERE work_kind=?1 AND work_id=?2",
            params![work.kind(), work.id()],
            |row| row.get(0),
        )?;
        let steer = Steer {
            id: SteerId::new(),
            work: work.clone(),
            author: author.clone(),
            text: text.to_string(),
            issued_at: OffsetDateTime::now_utc(),
        };
        let (author_kind, author_run_id) = match &steer.author {
            Author::User => ("user", None),
            Author::Run(run_id) => ("run", Some(run_id.as_str())),
        };
        tx.execute(
            "INSERT INTO steers (
                id, work_kind, work_id, sequence, author_kind, author_run_id, text, issued_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                steer.id.as_str(),
                work.kind(),
                work.id(),
                sequence,
                author_kind,
                author_run_id,
                steer.text,
                steer.issued_at.unix_timestamp(),
            ],
        )?;
        Ok(SteerReceipt { steer })
    }

    pub fn write_tool_response(
        &self,
        work: &WorkRef,
        write: &ToolResponseWrite,
    ) -> StoreResult<(ToolResponseReceipt, bool)> {
        let choice = write.choice.trim();
        if choice.is_empty() {
            return Err(StoreError::InvalidData(
                "tool response choice cannot be empty".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_ready_work(&tx, work)?;
        if let Some(existing) = tool_response_in(&tx, work, &write.request_id)? {
            if existing.choice != choice {
                return Err(StoreError::InvalidData(format!(
                    "tool response {} is already resolved as {:?}",
                    write.request_id, existing.choice
                )));
            }
            return Ok((existing, false));
        }
        let receipt = ToolResponseReceipt {
            id: ToolResponseId::new(),
            work: work.clone(),
            request_id: write.request_id.clone(),
            choice: choice.to_string(),
            responded_at: OffsetDateTime::now_utc(),
        };
        tx.execute(
            "INSERT INTO tool_responses (
                id, work_kind, work_id, request_id, choice, responded_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                receipt.id.as_str(),
                work.kind(),
                work.id(),
                receipt.request_id,
                receipt.choice,
                receipt.responded_at.unix_timestamp(),
            ],
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
        tool_response_in(&conn, work, request_id)
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

fn find_placement_in(conn: &Connection, work: &WorkRef) -> StoreResult<Option<Placement>> {
    let row = match work {
        WorkRef::Wave(id) => conn.query_row(
            "SELECT home_id, enabled, placed_at FROM work_placements WHERE wave_id=?1",
            [id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        ),
        WorkRef::Project(id) => conn.query_row(
            "SELECT home_id, enabled, placed_at FROM work_placements WHERE project_id=?1",
            [id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        ),
        WorkRef::Task(id) => conn.query_row(
            "SELECT home_id, enabled, placed_at FROM work_placements WHERE task_id=?1",
            [id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        ),
    }
    .optional()?;
    row.map(|(home_id, enabled, placed_at)| {
        Ok(Placement {
            work: work.clone(),
            home_id: HomeId::parse(&home_id).map_err(invalid_durable)?,
            enabled,
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
            "INSERT INTO work_placements (wave_id, home_id, enabled, placed_at)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(wave_id) DO UPDATE SET
                home_id=excluded.home_id, placed_at=excluded.placed_at",
            params![id.as_str(), home_id.as_str(), placed_at],
        )?,
        WorkRef::Project(id) => tx.execute(
            "INSERT INTO work_placements (project_id, home_id, enabled, placed_at)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(project_id) DO UPDATE SET
                home_id=excluded.home_id, placed_at=excluded.placed_at",
            params![id.as_str(), home_id.as_str(), placed_at],
        )?,
        WorkRef::Task(id) => tx.execute(
            "INSERT INTO work_placements (task_id, home_id, enabled, placed_at)
             VALUES (?1, ?2, 1, ?3)
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

fn insert_ask(conn: &Connection, ask: &Ask) -> StoreResult<()> {
    let (target_kind, target_work_kind, target_work_id) = match &ask.target {
        AskTarget::User => ("user", None, None),
        AskTarget::Parent(work) => ("parent", Some(work.kind()), Some(work.id())),
    };
    let (request_kind, prompt, flow, node_id, skill, iteration) = match &ask.request {
        AskBody::Intervention { prompt } => (
            "intervention",
            Some(prompt.as_str()),
            None,
            None,
            None,
            None,
        ),
        AskBody::FlowStep {
            flow,
            node_id,
            skill,
            iteration,
        } => (
            "flow_step",
            None,
            Some(flow.as_str()),
            Some(node_id.as_str()),
            Some(skill.as_str()),
            Some(i64::from(*iteration)),
        ),
    };
    let (result_kind, result_text) = ask
        .result
        .as_ref()
        .map(|result| (Some(result.state().as_str()), Some(result.text())))
        .unwrap_or((None, None));
    let (author_kind, author_id) = ask
        .terminal_author
        .as_ref()
        .map(author_parts)
        .unwrap_or((None, None));
    conn.execute(
        "INSERT INTO ask_exchanges (
            id, origin_work_kind, origin_work_id, source_run_id,
            origin_home_id, origin_cwd,
            target_kind, target_work_kind, target_work_id,
            request_kind, request_prompt, request_flow, request_node_id,
            request_skill, request_iteration, state, active_run_id,
            ready_at, presented_at,
            result_kind, result_text, terminal_author_kind, terminal_author_id,
            asked_at, terminal_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
            ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25
         )",
        params![
            ask.id.as_str(),
            ask.origin.work.kind(),
            ask.origin.work.id(),
            ask.origin.source_run_id.as_ref().map(RunId::as_str),
            ask.origin.home_id.as_str(),
            ask.origin.cwd.display().to_string(),
            target_kind,
            target_work_kind,
            target_work_id,
            request_kind,
            prompt,
            flow,
            node_id,
            skill,
            iteration,
            ask.state.as_str(),
            ask.active_run_id.as_ref().map(RunId::as_str),
            ask.ready_at.map(OffsetDateTime::unix_timestamp),
            ask.presented_at.map(OffsetDateTime::unix_timestamp),
            result_kind,
            result_text,
            author_kind,
            author_id,
            ask.asked_at.unix_timestamp(),
            ask.terminal_at.map(OffsetDateTime::unix_timestamp),
        ],
    )?;
    Ok(())
}

fn enqueue_ask_comment(conn: &Connection, ask: &Ask) -> StoreResult<()> {
    let route = match &ask.target {
        AskTarget::User => "User".to_string(),
        AskTarget::Parent(work) => format!("{} `{}`", work.kind(), work.id()),
    };
    let request = match &ask.request {
        AskBody::Intervention { prompt } => prompt.clone(),
        AskBody::FlowStep {
            flow,
            node_id,
            skill,
            ..
        } => format!("Run `{flow}` node `{node_id}` with `{skill}`."),
    };
    let transition = AskCommentTransition::Requested;
    let body = format!(
        "### Loopflow Ask\n\n**Route:** {route}\n\n{}\n\n{}",
        request,
        transition.marker(&ask.id)
    );
    enqueue_ask_comment_write(
        conn,
        &ask.id,
        &ask.origin.work,
        transition,
        &body,
        ask.asked_at.unix_timestamp(),
    )
}

fn enqueue_ask_result_comment(conn: &Connection, ask: &Ask) -> StoreResult<()> {
    let result = ask
        .result
        .as_ref()
        .ok_or_else(|| StoreError::InvalidData(format!("terminal Ask {} has no result", ask.id)))?;
    let author = match ask.terminal_author.as_ref() {
        Some(Author::User) => "User".to_string(),
        Some(Author::Run(run_id)) => format!("Run `{run_id}`"),
        None => "Loopflow".to_string(),
    };
    let heading = match result {
        AskResult::Resolved { .. } => "Loopflow Ask Resolved",
        AskResult::Declined { .. } => "Loopflow Ask Declined",
        AskResult::Cancelled { .. } => "Loopflow Ask Cancelled",
    };
    let transition = AskCommentTransition::Result;
    let body = format!(
        "### {heading}\n\n**Author:** {author}\n\n{}\n\n{}",
        result.text(),
        transition.marker(&ask.id)
    );
    enqueue_ask_comment_write(
        conn,
        &ask.id,
        &ask.origin.work,
        transition,
        &body,
        ask.terminal_at
            .expect("a terminal Ask has terminal_at")
            .unix_timestamp(),
    )
}

fn enqueue_ask_comment_write(
    conn: &Connection,
    ask_id: &AskId,
    work: &WorkRef,
    transition: AskCommentTransition,
    body: &str,
    created_at: i64,
) -> StoreResult<()> {
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
        "ask" => AskCommentTransition::Requested,
        "answer" => AskCommentTransition::Result,
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

pub(super) fn ask_by_id_in(conn: &Connection, ask_id: &AskId) -> StoreResult<Ask> {
    conn.query_row(
        "SELECT origin_work_kind, origin_work_id, source_run_id,
                origin_home_id, origin_cwd,
                target_kind, target_work_kind, target_work_id,
                request_kind, request_prompt, request_flow, request_node_id,
                request_skill, request_iteration, state, active_run_id,
                ready_at, presented_at,
                result_kind, result_text, terminal_author_kind, terminal_author_id,
                asked_at, terminal_at
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
                row.get(10)?,
                row.get(11)?,
                row.get(12)?,
                row.get(13)?,
                row.get(14)?,
                row.get(15)?,
                row.get(16)?,
                row.get(17)?,
                row.get(18)?,
                row.get(19)?,
                row.get(20)?,
                row.get(21)?,
                row.get(22)?,
                row.get(23)?,
            )
            .map_err(to_sqlite_conversion_error)
        },
    )
    .map_err(StoreError::from)
}

#[allow(clippy::too_many_arguments)]
fn map_ask_row(
    id: AskId,
    origin_work_kind: String,
    origin_work_id: String,
    source_run_id: Option<String>,
    origin_home_id: String,
    origin_cwd: String,
    target_kind: String,
    target_work_kind: Option<String>,
    target_work_id: Option<String>,
    request_kind: String,
    request_prompt: Option<String>,
    request_flow: Option<String>,
    request_node_id: Option<String>,
    request_skill: Option<String>,
    request_iteration: Option<i64>,
    state: String,
    active_run_id: Option<String>,
    ready_at: Option<i64>,
    presented_at: Option<i64>,
    result_kind: Option<String>,
    result_text: Option<String>,
    terminal_author_kind: Option<String>,
    terminal_author_id: Option<String>,
    asked_at: i64,
    terminal_at: Option<i64>,
) -> StoreResult<Ask> {
    let target = match (target_kind.as_str(), target_work_kind, target_work_id) {
        ("user", None, None) => AskTarget::User,
        ("parent", Some(kind), Some(id)) => AskTarget::Parent(parse_work_ref(&kind, &id)?),
        _ => {
            return Err(StoreError::InvalidData(
                "stored Ask route is inconsistent".to_string(),
            ))
        }
    };
    let request = match (
        request_kind.as_str(),
        request_prompt,
        request_flow,
        request_node_id,
        request_skill,
        request_iteration,
    ) {
        ("intervention", Some(prompt), None, None, None, None) => AskBody::Intervention { prompt },
        ("flow_step", None, Some(flow), Some(node_id), Some(skill), Some(iteration)) => {
            AskBody::FlowStep {
                flow,
                node_id,
                skill,
                iteration: u32::try_from(iteration).map_err(|_| {
                    StoreError::InvalidData(
                        "stored flow-step iteration is outside the u32 range".to_string(),
                    )
                })?,
            }
        }
        _ => {
            return Err(StoreError::InvalidData(
                "stored Ask body is inconsistent".to_string(),
            ))
        }
    };
    let result = match (result_kind.as_deref(), result_text) {
        (None, None) => None,
        (Some("resolved"), Some(summary)) => Some(AskResult::Resolved { summary }),
        (Some("declined"), Some(reason)) => Some(AskResult::Declined { reason }),
        (Some("cancelled"), Some(reason)) => Some(AskResult::Cancelled { reason }),
        _ => {
            return Err(StoreError::InvalidData(
                "stored Ask result is inconsistent".to_string(),
            ))
        }
    };
    let terminal_author = match (terminal_author_kind.as_deref(), terminal_author_id) {
        (None, None) => None,
        (Some("user"), None) => Some(Author::User),
        (Some("run"), Some(run_id)) => {
            Some(Author::Run(RunId::parse(&run_id).map_err(invalid_durable)?))
        }
        _ => {
            return Err(StoreError::InvalidData(
                "stored Ask terminal author is inconsistent".to_string(),
            ))
        }
    };
    Ok(Ask {
        id,
        origin: AskOrigin {
            work: parse_work_ref(&origin_work_kind, &origin_work_id)?,
            source_run_id: source_run_id
                .map(|run_id| RunId::parse(&run_id).map_err(invalid_durable))
                .transpose()?,
            home_id: HomeId::parse(&origin_home_id).map_err(invalid_durable)?,
            cwd: origin_cwd.into(),
        },
        target,
        request,
        state: AskState::parse(&state).map_err(invalid_durable)?,
        active_run_id: active_run_id
            .map(|run_id| RunId::parse(&run_id).map_err(invalid_durable))
            .transpose()?,
        ready_at: ready_at
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()
            .map_err(invalid_durable)?,
        presented_at: presented_at
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()
            .map_err(invalid_durable)?,
        result,
        terminal_author,
        asked_at: OffsetDateTime::from_unix_timestamp(asked_at).map_err(invalid_durable)?,
        terminal_at: terminal_at
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()
            .map_err(invalid_durable)?,
    })
}

enum AskScope<'a> {
    Target(&'a AskTarget),
    OriginWork(&'a WorkRef),
}

fn query_asks(conn: &Connection, scope: AskScope<'_>) -> StoreResult<Vec<Ask>> {
    let ids = match scope {
        AskScope::Target(target) => {
            let (predicate, kind, id) = match target {
                AskTarget::User => ("a.target_kind='user'", None, None),
                AskTarget::Parent(work) => (
                    "a.target_kind='parent' AND a.target_work_kind=?1 AND a.target_work_id=?2",
                    Some(work.kind()),
                    Some(work.id()),
                ),
            };
            let sql = format!(
                "SELECT a.id FROM ask_exchanges a
                 WHERE a.state IN ('queued', 'claimed') AND {predicate}
                 ORDER BY a.asked_at, a.rowid"
            );
            let mut statement = conn.prepare(&sql)?;
            match (kind, id) {
                (Some(kind), Some(id)) => statement
                    .query_map(params![kind, id], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?,
                (None, None) => statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?,
                _ => unreachable!("Ask target parts are complete"),
            }
        }
        AskScope::OriginWork(work) => {
            let mut statement = conn.prepare(
                "SELECT id FROM ask_exchanges
                 WHERE origin_work_kind=?1 AND origin_work_id=?2
                 ORDER BY asked_at DESC, rowid DESC",
            )?;
            let ids = statement
                .query_map(params![work.kind(), work.id()], |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?;
            ids
        }
    };
    ids.into_iter()
        .map(|id| {
            let id = AskId::parse(&id).map_err(invalid_durable)?;
            ask_by_id_in(conn, &id)
        })
        .filter(|ask| {
            ask.as_ref().map_or(true, |ask| {
                ask.state.is_terminal()
                    || matches!(
                        work_status_in(conn, &ask.origin.work),
                        Ok(WorkStatus::Ready)
                    )
            })
        })
        .collect()
}

fn validate_ask_origin(conn: &Connection, origin: &AskOrigin) -> StoreResult<()> {
    require_ready_work(conn, &origin.work)?;
    let placement = placement_in(conn, &origin.work)?;
    if origin.home_id != placement.home_id {
        return Err(StoreError::InvalidAuthority(
            "Ask origin does not match the Work placement".to_string(),
        ));
    }
    if origin.cwd.as_os_str().is_empty() {
        return Err(StoreError::InvalidData(
            "Ask origin cwd cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_requested_target(
    conn: &Connection,
    origin: &WorkRef,
    target: &AskTarget,
) -> StoreResult<()> {
    if let AskTarget::Parent(parent) = target {
        if parent_work(conn, origin)?.as_ref() != Some(parent) {
            return Err(StoreError::InvalidAuthority(
                "Ask parent target is not the origin Work's immediate parent".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_ask_body(request: &AskBody) -> StoreResult<()> {
    let empty = |value: &str| value.trim().is_empty();
    match request {
        AskBody::Intervention { prompt } if empty(prompt) => Err(StoreError::InvalidData(
            "Ask intervention prompt cannot be empty".to_string(),
        )),
        AskBody::FlowStep {
            flow,
            node_id,
            skill,
            ..
        } if [flow, node_id, skill].into_iter().any(|value| empty(value)) => Err(
            StoreError::InvalidData("Ask flow, node, and skill cannot be empty".to_string()),
        ),
        _ => Ok(()),
    }
}

fn validate_ask_result(result: &AskResult) -> StoreResult<()> {
    if result.text().trim().is_empty() {
        return Err(StoreError::InvalidData(
            "Ask result cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn flow_ask_in(
    conn: &Connection,
    work: &WorkRef,
    flow: &str,
    node_id: &str,
    skill: &str,
    iteration: u32,
    target: &AskTarget,
) -> StoreResult<Option<Ask>> {
    let mut statement = conn.prepare(
        "SELECT id FROM ask_exchanges
         WHERE origin_work_kind=?1 AND origin_work_id=?2
           AND request_kind='flow_step'
           AND request_flow=?3 AND request_node_id=?4
           AND request_skill=?5 AND request_iteration=?6
         ORDER BY asked_at DESC, rowid DESC",
    )?;
    let ids = statement
        .query_map(
            params![work.kind(), work.id(), flow, node_id, skill, iteration],
            |row| row.get::<_, String>(0),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    for id in ids {
        let id = AskId::parse(&id).map_err(invalid_durable)?;
        let ask = ask_by_id_in(conn, &id)?;
        if &ask.target == target {
            return Ok(Some(ask));
        }
    }
    Ok(None)
}

fn require_ready_ask_work(conn: &Connection, ask_id: &AskId) -> StoreResult<()> {
    let ask = ask_by_id_in(conn, ask_id)?;
    if work_status_in(conn, &ask.origin.work)? == WorkStatus::Ready {
        Ok(())
    } else {
        Err(StoreError::InvalidAuthority(format!(
            "Ask {ask_id} no longer belongs to ready Work"
        )))
    }
}

fn requeue_ask_in(conn: &Connection, ask: &Ask, run_id: &RunId) -> StoreResult<()> {
    if conn.execute(
        "UPDATE ask_exchanges
         SET state='queued', active_run_id=NULL, ready_at=NULL, presented_at=NULL
         WHERE id=?1 AND state='claimed' AND active_run_id=?2",
        params![ask.id.as_str(), run_id.as_str()],
    )? != 1
    {
        return Err(StoreError::InvalidAuthority(format!(
            "Run {run_id} no longer owns Ask {}",
            ask.id
        )));
    }
    Ok(())
}

fn write_terminal_ask_in(
    conn: &Connection,
    ask: &Ask,
    result: &AskResult,
    author: &Author,
    terminal_at: i64,
) -> StoreResult<Ask> {
    let (author_kind, author_id) = author_parts(author);
    if conn.execute(
        "UPDATE ask_exchanges
         SET state=?2, active_run_id=NULL,
             result_kind=?2, result_text=?3,
             terminal_author_kind=?4, terminal_author_id=?5, terminal_at=?6
         WHERE id=?1 AND state=?7 AND active_run_id IS ?8",
        params![
            ask.id.as_str(),
            result.state().as_str(),
            result.text(),
            author_kind,
            author_id,
            terminal_at,
            ask.state.as_str(),
            ask.active_run_id.as_ref().map(RunId::as_str),
        ],
    )? != 1
    {
        return Err(StoreError::InvalidAuthority(format!(
            "Ask {} changed before terminal settlement",
            ask.id
        )));
    }
    ask_by_id_in(conn, &ask.id)
}

fn author_parts(author: &Author) -> (Option<&'static str>, Option<&str>) {
    match author {
        Author::User => (Some("user"), None),
        Author::Run(run_id) => (Some("run"), Some(run_id.as_str())),
    }
}

fn normalize_reason(value: &str, label: &str) -> StoreResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(StoreError::InvalidData(format!("{label} cannot be empty")));
    }
    Ok(value.to_string())
}

fn normalize_optional_reason(value: Option<&str>) -> StoreResult<Option<String>> {
    value
        .map(|value| normalize_reason(value, "Ask release reason"))
        .transpose()
}

pub(super) fn cancel_pending_asks_for_work(
    conn: &Connection,
    work: &WorkRef,
    reason: &str,
    terminal_at: i64,
) -> StoreResult<()> {
    cancel_pending_asks(conn, work, reason, terminal_at, false)
}

pub(super) fn cancel_pending_flow_asks_for_work(
    conn: &Connection,
    work: &WorkRef,
    reason: &str,
    terminal_at: i64,
) -> StoreResult<()> {
    cancel_pending_asks(conn, work, reason, terminal_at, true)
}

fn cancel_pending_asks(
    conn: &Connection,
    work: &WorkRef,
    reason: &str,
    terminal_at: i64,
    flow_only: bool,
) -> StoreResult<()> {
    let mut statement = conn.prepare(
        "SELECT id FROM ask_exchanges
         WHERE origin_work_kind=?1 AND origin_work_id=?2
           AND state IN ('queued', 'claimed')
           AND (?3=0 OR request_kind='flow_step')
         ORDER BY asked_at, rowid",
    )?;
    let ids = statement
        .query_map(params![work.kind(), work.id(), flow_only], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    conn.execute(
        "UPDATE ask_exchanges
         SET state='cancelled', active_run_id=NULL,
             result_kind='cancelled', result_text=?4,
             terminal_author_kind=NULL, terminal_author_id=NULL, terminal_at=?5
         WHERE origin_work_kind=?1 AND origin_work_id=?2
           AND state IN ('queued', 'claimed')
           AND (?3=0 OR request_kind='flow_step')",
        params![work.kind(), work.id(), flow_only, reason, terminal_at],
    )?;
    for id in ids {
        let id = AskId::parse(&id).map_err(invalid_durable)?;
        let ask = ask_by_id_in(conn, &id)?;
        enqueue_ask_result_comment(conn, &ask)?;
    }
    Ok(())
}

pub(crate) fn work_status_in(conn: &Connection, work: &WorkRef) -> StoreResult<WorkStatus> {
    let (table, id) = work_table(work);
    let state: String = conn.query_row(
        &format!("SELECT work_state FROM {table} WHERE id=?1"),
        [id],
        |row| row.get(0),
    )?;
    match state.as_str() {
        "ready" => Ok(WorkStatus::Ready),
        "done" => Ok(WorkStatus::Done),
        "abandoned" => Ok(WorkStatus::Abandoned),
        other => Err(StoreError::InvalidData(format!(
            "invalid {} Work state {other:?}",
            work.kind()
        ))),
    }
}

fn require_ready_work(conn: &Connection, work: &WorkRef) -> StoreResult<()> {
    match work_status_in(conn, work)? {
        WorkStatus::Ready => Ok(()),
        status => Err(StoreError::InvalidAuthority(format!(
            "{} {} is {status}",
            work.kind(),
            work.id()
        ))),
    }
}

pub(crate) fn reopen_work_in(conn: &Connection, work: &WorkRef) -> StoreResult<()> {
    let now = now_unix();
    cancel_pending_asks_for_work(conn, work, "owning Work reopened", now)?;
    let (table, id) = work_table(work);
    if conn.execute(
        &format!(
            "UPDATE {table} SET work_state='ready', work_terminal_at=NULL
             WHERE id=?1 AND work_state IN ('done', 'abandoned')"
        ),
        [id],
    )? != 1
    {
        return Err(StoreError::InvalidAuthority(format!(
            "{} {} is not terminal",
            work.kind(),
            work.id()
        )));
    }
    Ok(())
}

fn work_table(work: &WorkRef) -> (&'static str, &str) {
    match work {
        WorkRef::Wave(id) => ("waves", id.as_str()),
        WorkRef::Project(id) => ("projects", id.as_str()),
        WorkRef::Task(id) => ("tasks", id.as_str()),
    }
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

pub(crate) fn create_wave_work(
    tx: &Transaction<'_>,
    wave_id: &WaveId,
    created_at: i64,
) -> StoreResult<()> {
    let work = WorkRef::Wave(wave_id.clone());
    inherit_placement(tx, &work, None, created_at)
}

pub(crate) fn create_project_work(tx: &Transaction<'_>, project: &Project) -> StoreResult<()> {
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
    Ok(())
}

pub(crate) fn create_task_work(tx: &Transaction<'_>, task: &Task) -> StoreResult<()> {
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
    Ok(())
}

pub(crate) fn validate_work_completion_readiness_in(
    conn: &Connection,
    work: &WorkRef,
) -> StoreResult<()> {
    let child_ask_open: bool = conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM ask_exchanges a
            WHERE a.state IN ('queued', 'claimed')
              AND a.target_kind='parent' AND a.target_work_kind=?1
              AND a.target_work_id=?2
         )",
        params![work.kind(), work.id()],
        |row| row.get(0),
    )?;
    if child_ask_open {
        return Err(StoreError::InvalidData(
            "Run cannot complete while a child Ask is unanswered".to_string(),
        ));
    }
    Ok(())
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

fn work_steers_in(conn: &Connection, work: &WorkRef) -> StoreResult<Vec<Steer>> {
    require_ready_work(conn, work)?;
    let mut statement = conn.prepare(
        "SELECT id, author_kind, author_run_id, text, issued_at
         FROM steers WHERE work_kind=?1 AND work_id=?2 ORDER BY sequence",
    )?;
    let rows = statement.query_map(params![work.kind(), work.id()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
        ))
    })?;
    let mut steers = Vec::new();
    for row in rows {
        steers.push(decode_steer(row?, work.clone())?);
    }
    Ok(steers)
}

fn validate_project_child_control_basis(
    conn: &Connection,
    project_id: &ProjectId,
    basis: &ProjectChildControlBasis,
) -> StoreResult<()> {
    let expected_work = WorkRef::Project(project_id.clone());
    if basis.flow.trim().is_empty() || basis.step.trim().is_empty() {
        return Err(StoreError::InvalidData(
            "Project child-control flow and step cannot be empty".to_string(),
        ));
    }
    require_ready_work(conn, &expected_work)?;
    let expected_sequence = control_steer_sequence(basis)?;
    let current_sequence = project_steer_sequence(conn, project_id)?;
    if expected_sequence != current_sequence {
        return Err(StoreError::InvalidAuthority(format!(
            "Project {project_id} direction changed while its child-control basis was being prepared"
        )));
    }
    Ok(())
}

fn validate_project_child_control_holder(
    conn: &Connection,
    project_id: &ProjectId,
    run_id: &RunId,
    token: &ProjectChildControlToken,
) -> StoreResult<()> {
    let holder = conn
        .query_row(
            "SELECT run_id, token_hash FROM project_child_controls WHERE project_id=?1",
            [project_id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let Some((holder_run_id, holder_token_hash)) = holder else {
        return Err(StoreError::InvalidAuthority(format!(
            "Project {project_id} has no active child-control basis; restart Project Work before pursuit"
        )));
    };
    if holder_run_id != run_id.as_str() || holder_token_hash != token.hash() {
        return Err(StoreError::InvalidAuthority(format!(
            "Project {project_id} child-control authority was superseded; restart Project Work"
        )));
    }
    Ok(())
}

fn project_steer_sequence(conn: &Connection, project_id: &ProjectId) -> StoreResult<i64> {
    conn.query_row(
        "SELECT COALESCE(MAX(sequence), 0) FROM steers
         WHERE work_kind='project' AND work_id=?1",
        [project_id.as_str()],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn control_steer_sequence(basis: &ProjectChildControlBasis) -> StoreResult<i64> {
    i64::try_from(basis.steer_sequence).map_err(|_| {
        StoreError::InvalidData("Project child-control Steer sequence is too large".to_string())
    })
}

fn tool_response_in(
    conn: &Connection,
    work: &WorkRef,
    request_id: &str,
) -> StoreResult<Option<ToolResponseReceipt>> {
    let row = conn
        .query_row(
            "SELECT id, choice, responded_at FROM tool_responses
             WHERE work_kind=?1 AND work_id=?2 AND request_id=?3",
            params![work.kind(), work.id(), request_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((id, choice, responded_at)) = row else {
        return Ok(None);
    };
    Ok(Some(ToolResponseReceipt {
        id: ToolResponseId::parse(&id).map_err(|error| {
            StoreError::InvalidData(format!("invalid stored ToolResponse id: {error}"))
        })?,
        work: work.clone(),
        request_id: request_id.to_string(),
        choice,
        responded_at: OffsetDateTime::from_unix_timestamp(responded_at).map_err(|error| {
            StoreError::InvalidData(format!("invalid Decision timestamp: {error}"))
        })?,
    }))
}

type SteerFields = (String, String, Option<String>, String, i64);

fn decode_steer(fields: SteerFields, work: WorkRef) -> StoreResult<Steer> {
    let (id, author_kind, author_run_id, text, issued_at) = fields;
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
        author,
        text,
        issued_at: OffsetDateTime::from_unix_timestamp(issued_at).map_err(|error| {
            StoreError::InvalidData(format!("invalid Steer timestamp: {error}"))
        })?,
    })
}

#[cfg(test)]
mod durable_store_tests {
    use crate::durable::{AskBody, AskOrigin, AskTarget, Author, WorkRef};
    use crate::id::WaveId;
    use crate::store::sqlite::SqliteStore;
    use crate::work::project::ProjectId;
    use crate::work::task::TaskId;

    /// A registered Wave is the cheapest real Work and needs no PM binding.
    fn _store_with_wave() -> (tempfile::TempDir, SqliteStore, WorkRef) {
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
            super::create_wave_work(&tx, &wave_id, 1_700_000_000).unwrap();
            tx.commit().unwrap();
        }
        let work = WorkRef::Wave(wave_id);
        (dir, store, work)
    }

    fn store_with_wave() -> (tempfile::TempDir, SqliteStore, WorkRef) {
        _store_with_wave()
    }

    #[test]
    fn activity_steers_are_ordered_work_facts() {
        let (dir, store, work) = store_with_wave();
        let first = store
            .append_steer(&work, &Author::User, "first direction")
            .unwrap();
        let second = store
            .append_steer(&work, &Author::User, "second direction")
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
        tx.execute(
            "INSERT INTO tasks (
                id, project_id, external_issue_id, issue_identifier, created_at
             ) VALUES (?1, ?2, 'linear-issue', 'ENG-1', 1700000002)",
            rusqlite::params![task_id.as_str(), project_id.as_str()],
        )
        .unwrap();
        super::inherit_placement(&tx, &task, Some(&project), 1_700_000_002).unwrap();
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

    #[test]
    fn status_ask_sql_prepares_against_the_migrated_schema() {
        let (directory, _, _) = store_with_wave();
        let conn = rusqlite::Connection::open(directory.path().join("loopflow.db")).unwrap();

        conn.prepare(super::HAS_PENDING_USER_ASK_FOR_WORK_SQL)
            .expect("runtime status SQL must prepare against the migration head");
    }

    #[test]
    fn pending_user_ask_status_resolves_every_work_kind_and_excludes_parent_routes() {
        for index in 0..3 {
            let (directory, store, work_routes) = store_with_work_hierarchy();
            let path = directory.path().join("loopflow.db");
            let (work, parent) = &work_routes[index];
            let placement = store.placement(work).unwrap();
            let ask = store
                .create_ask(
                    &AskOrigin {
                        work: work.clone(),
                        source_run_id: None,
                        home_id: placement.home_id,
                        cwd: "/tmp/runtime".into(),
                    },
                    &AskBody::Intervention {
                        prompt: format!("What blocks {}?", work.kind()),
                    },
                    &AskTarget::User,
                )
                .unwrap();
            let conn = rusqlite::Connection::open(&path).unwrap();

            for (candidate, _) in &work_routes {
                assert_eq!(
                    store.has_pending_user_ask_for_work(candidate).unwrap(),
                    candidate == work,
                    "the User Ask must belong only to its {} Work",
                    work.kind()
                );
            }

            conn.execute(
                "UPDATE ask_exchanges
                 SET target_kind='parent', target_work_kind=?2, target_work_id=?3
                 WHERE id=?1",
                rusqlite::params![ask.id.as_str(), parent.kind(), parent.id()],
            )
            .unwrap();
            assert!(!store.has_pending_user_ask_for_work(work).unwrap());
            let conn = store.conn.lock().expect("store mutex poisoned");
            let pending = super::query_asks(
                &conn,
                super::AskScope::Target(&AskTarget::Parent(parent.clone())),
            )
            .unwrap();
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].target, AskTarget::Parent(parent.clone()));
        }
    }
}
