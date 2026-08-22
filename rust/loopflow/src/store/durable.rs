use crate::child::ChildRef;
use crate::durable::{
    AdvanceReceipt, AgentInvocation, AgentInvocationId, Ask, AskBody, AskClaim, AskId, AskOrigin,
    AskResult, AskTarget, Author, Basis, BoundarySeed, ContainmentObservation, DoneProposal,
    EpochReceipt, FlowPosition, Home, HomeId, InterruptReceipt, InvocationRoute, InvocationSurface,
    Placement, Run, RunAdvance, RunContext, RunControl, RunLivenessEvidence, RunTrigger, Send,
    SendId, SendState, SteerId, SteerReceipt, StopCause, StopReceipt, ToolResponseReceipt,
    ToolResponseWrite, WorkRef, WorkStatus,
};

use super::{run_sqlite, Store, StoreError, StoreResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskWriterState {
    pub work: WorkRef,
    pub identifier: String,
    pub run: Option<Run>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AskCommentTransition {
    Requested,
    Result,
}

impl AskCommentTransition {
    // These are published outbox/marker values. Keep their bytes stable while
    // the Rust vocabulary follows requested-session and typed-result state.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "ask",
            Self::Result => "answer",
        }
    }

    pub(crate) fn marker(self, ask_id: &AskId) -> String {
        format!("<!-- loopflow:{ask_id}:{} -->", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AskCommentWrite {
    pub ask_id: AskId,
    pub transition: AskCommentTransition,
    pub issue_id: String,
    pub body: String,
    pub repo: String,
    pub wave: String,
    pub attempt_count: u32,
    pub attempt_started_at: Option<i64>,
    pub last_error: Option<String>,
    pub linear_comment_id: Option<String>,
    pub delivered_at: Option<i64>,
}

impl Store {
    pub(crate) async fn task_writer_state(
        &self,
        external_issue_id: &str,
    ) -> StoreResult<Option<TaskWriterState>> {
        let external_issue_id = external_issue_id.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.task_writer_state(&external_issue_id)
        })
        .await
    }

    pub async fn home_by_id(&self, home_id: &HomeId) -> StoreResult<Option<Home>> {
        let home_id = home_id.clone();
        run_sqlite(&self.sqlite, move |store| store.home_by_id(&home_id)).await
    }

    pub async fn local_home(&self) -> StoreResult<Home> {
        run_sqlite(&self.sqlite, move |store| store.local_home()).await
    }

    pub async fn observe_home(&self, home_id: &HomeId, route: &str) -> StoreResult<Home> {
        let home_id = home_id.clone();
        let route = route.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.observe_home(&home_id, &route)
        })
        .await
    }

    pub async fn placement(&self, work: &WorkRef) -> StoreResult<Placement> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.placement(&work)).await
    }

    pub async fn set_work_enabled(&self, work: &WorkRef, enabled: bool) -> StoreResult<Placement> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.set_work_enabled(&work, enabled)
        })
        .await
    }

    pub(crate) async fn place_work(
        &self,
        work: &WorkRef,
        home_id: &HomeId,
    ) -> StoreResult<Placement> {
        let work = work.clone();
        let home_id = home_id.clone();
        run_sqlite(&self.sqlite, move |store| store.place_work(&work, &home_id)).await
    }

    pub async fn reserve_run(
        &self,
        work: &WorkRef,
        trigger: RunTrigger,
    ) -> StoreResult<(Run, RunContext)> {
        let _promotion_lock = crate::promotion_lock::acquire_shared()
            .await
            .map_err(|error| {
                super::StoreError::InvalidData(format!(
                    "acquire shared promotion lock before Run reservation: {error}"
                ))
            })?;
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_run(&work, &trigger)
        })
        .await
    }

    /// Reserve replacement Work while the promotion coordinator still owns
    /// the exclusive machine fence. The unsettled switch is itself the public
    /// reservation fence; only its receipt-pinned candidate reaches this path.
    pub(crate) async fn reserve_run_for_install_switch(
        &self,
        work: &WorkRef,
        trigger: RunTrigger,
        switch_id: &str,
    ) -> StoreResult<(Run, RunContext)> {
        crate::promotion_lock::require_exclusive_holder().map_err(|error| {
            StoreError::InvalidData(format!(
                "verify promotion coordinator before switch Run reservation: {error}"
            ))
        })?;
        match crate::machine_install::read_state(&crate::machine_install::root().map_err(
            |error| StoreError::InvalidData(format!("resolve machine install authority: {error}")),
        )?)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?
        {
            crate::machine_install::MachineInstallState::Switching(receipt)
                if receipt.id == switch_id
                    && receipt.target_store_advanced
                    && receipt.phase == crate::machine_install::SwitchPhase::Reconciling => {}
            crate::machine_install::MachineInstallState::Switching(receipt) => {
                return Err(StoreError::InvalidData(format!(
                    "install switch {} is not reconciling as {switch_id}",
                    receipt.id
                )))
            }
            _ => {
                return Err(StoreError::InvalidData(format!(
                    "install switch {switch_id} is no longer active"
                )))
            }
        }
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_run(&work, &trigger)
        })
        .await
    }

    pub(crate) async fn reserve_recovery_run(
        &self,
        lease: &RunContext,
    ) -> StoreResult<(Run, RunContext)> {
        let _promotion_lock = crate::promotion_lock::acquire_shared()
            .await
            .map_err(|error| {
                StoreError::InvalidData(format!(
                    "acquire shared promotion lock before Recovery Run reservation: {error}"
                ))
            })?;
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_recovery_run(&lease)
        })
        .await
    }

    pub async fn current_run(&self, work: &WorkRef) -> StoreResult<Option<Run>> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.current_run(&work)).await
    }

    pub(crate) async fn run_by_id(&self, run_id: &crate::durable::RunId) -> StoreResult<Run> {
        let run_id = run_id.clone();
        run_sqlite(&self.sqlite, move |store| store.run_by_id(&run_id)).await
    }

    pub(crate) async fn nonterminal_runs_for_home(
        &self,
        home_id: &HomeId,
    ) -> StoreResult<Vec<Run>> {
        let home_id = home_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.nonterminal_runs_for_home(&home_id)
        })
        .await
    }

    pub(crate) async fn run_liveness_evidence(
        &self,
        run: &Run,
    ) -> StoreResult<RunLivenessEvidence> {
        let run = run.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.run_liveness_evidence(&run, time::OffsetDateTime::now_utc())
        })
        .await
    }

    pub(crate) async fn ensure_local_run(&self, run: &Run) -> StoreResult<()> {
        let run = run.clone();
        run_sqlite(&self.sqlite, move |store| store.ensure_local_run(&run)).await
    }

    pub(crate) async fn record_run_liveness(
        &self,
        run_id: &crate::durable::RunId,
        observation: ContainmentObservation,
    ) -> StoreResult<RunLivenessEvidence> {
        let run_id = run_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.record_run_liveness(&run_id, observation)
        })
        .await
    }

    pub(crate) async fn repair_stranded_asks(&self) -> StoreResult<usize> {
        run_sqlite(&self.sqlite, move |store| store.repair_stranded_asks()).await
    }

    pub(crate) async fn latest_run(&self, work: &WorkRef) -> StoreResult<Option<Run>> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.latest_run(&work)).await
    }

    pub(crate) async fn run_context(
        &self,
        run_id: &crate::durable::RunId,
    ) -> StoreResult<RunContext> {
        let run_id = run_id.clone();
        run_sqlite(&self.sqlite, move |store| store.run_context(&run_id)).await
    }

    pub(crate) async fn validate_run_context(&self, lease: &RunContext) -> StoreResult<()> {
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.validate_run_context(&lease)
        })
        .await
    }

    pub(crate) async fn record_first_material_at(
        &self,
        lease: &RunContext,
        observed_at: time::OffsetDateTime,
    ) -> StoreResult<time::OffsetDateTime> {
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.record_first_material_at(&lease, observed_at)
        })
        .await
    }

    pub async fn advance_run(
        &self,
        lease: &RunContext,
        advance: RunAdvance,
    ) -> StoreResult<AdvanceReceipt> {
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.advance_run(&lease, &advance)
        })
        .await
    }

    pub async fn stop_run(
        &self,
        lease: &RunContext,
        cause: StopCause,
        containment: ContainmentObservation,
    ) -> StoreResult<StopReceipt> {
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.stop_run(&lease, &cause, containment)
        })
        .await
    }

    pub(crate) async fn finish_run(
        &self,
        lease: &RunContext,
        outcome: crate::durable::BoundaryState,
    ) -> StoreResult<()> {
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| store.finish_run(&lease, outcome)).await
    }

    /// Release Wave Run authority from the process interrupt handler, which
    /// exits before Tokio can drive the listener's async shutdown path.
    pub(crate) fn stop_run_on_interrupt(&self, lease: &RunContext) -> StoreResult<StopReceipt> {
        self.sqlite
            .stop_run(lease, &StopCause::Requested, ContainmentObservation::Absent)
    }

    pub(crate) async fn run_control(
        &self,
        lease: &RunContext,
        active_turn_id: Option<&str>,
    ) -> StoreResult<Option<RunControl>> {
        let lease = lease.clone();
        let active_turn_id = active_turn_id.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.run_control(&lease, active_turn_id.as_deref())
        })
        .await
    }

    pub async fn set_flow_position(
        &self,
        lease: &RunContext,
        position: FlowPosition,
    ) -> StoreResult<FlowPosition> {
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.set_flow_position(&lease, &position)
        })
        .await
    }

    pub async fn create_ask(
        &self,
        lease: &RunContext,
        origin: AskOrigin,
        request: AskBody,
        target: AskTarget,
    ) -> StoreResult<Ask> {
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.create_ask(&lease, &origin, &request, &target)
        })
        .await
    }

    pub async fn ask_by_id(&self, ask_id: &AskId) -> StoreResult<Ask> {
        let ask_id = ask_id.clone();
        run_sqlite(&self.sqlite, move |store| store.ask_by_id(&ask_id)).await
    }

    pub async fn pending_asks(
        &self,
        actor: Option<&RunContext>,
        target: &AskTarget,
    ) -> StoreResult<Vec<Ask>> {
        let actor = actor.cloned();
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.pending_asks(actor.as_ref(), &target)
        })
        .await
    }

    pub(crate) async fn claim_ask(
        &self,
        actor: Option<&RunContext>,
        ask_id: &AskId,
        route: InvocationRoute,
        surface: &str,
    ) -> StoreResult<AskClaim> {
        let actor = actor.cloned();
        let ask_id = ask_id.clone();
        let surface = surface.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_ask(actor.as_ref(), &ask_id, &route, &surface)
        })
        .await
    }

    pub(crate) async fn claim_flow_step_run_context(
        &self,
        ask_id: &AskId,
        invocation_id: &AgentInvocationId,
    ) -> StoreResult<Option<RunContext>> {
        let ask_id = ask_id.clone();
        let invocation_id = invocation_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_flow_step_run_context(&ask_id, &invocation_id)
        })
        .await
    }

    pub async fn mark_ask_ready(
        &self,
        ask_id: &AskId,
        invocation_id: &AgentInvocationId,
    ) -> StoreResult<AgentInvocation> {
        let ask_id = ask_id.clone();
        let invocation_id = invocation_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_ask_ready(&ask_id, &invocation_id)
        })
        .await
    }

    pub async fn mark_ask_presented(
        &self,
        ask_id: &AskId,
        invocation_id: &AgentInvocationId,
    ) -> StoreResult<AgentInvocation> {
        let ask_id = ask_id.clone();
        let invocation_id = invocation_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_ask_presented(&ask_id, &invocation_id)
        })
        .await
    }

    pub async fn mark_presented_by_target(
        &self,
        actor: Option<&RunContext>,
        ask_id: &AskId,
        invocation_id: &AgentInvocationId,
    ) -> StoreResult<AgentInvocation> {
        let actor = actor.cloned();
        let ask_id = ask_id.clone();
        let invocation_id = invocation_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_presented_by_target(actor.as_ref(), &ask_id, &invocation_id)
        })
        .await
    }

    pub(crate) fn interrupt_ask_on_interrupt(
        &self,
        ask_id: &AskId,
        invocation_id: &AgentInvocationId,
    ) -> StoreResult<()> {
        self.sqlite
            .interrupt_ask_on_interrupt(ask_id, invocation_id)
            .map(|_| ())
    }

    pub async fn settle_ask(
        &self,
        ask_id: &AskId,
        invocation_id: &AgentInvocationId,
        result: AskResult,
    ) -> StoreResult<Ask> {
        let ask_id = ask_id.clone();
        let invocation_id = invocation_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.settle_ask(&ask_id, &invocation_id, &result)
        })
        .await
    }

    pub async fn release_ask(
        &self,
        ask_id: &AskId,
        invocation_id: &AgentInvocationId,
        reason: Option<&str>,
    ) -> StoreResult<Ask> {
        let ask_id = ask_id.clone();
        let invocation_id = invocation_id.clone();
        let reason = reason.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.release_ask(&ask_id, &invocation_id, reason.as_deref())
        })
        .await
    }

    pub(crate) async fn close_ask_invocation(
        &self,
        ask_id: &AskId,
        invocation_id: &AgentInvocationId,
        reason: Option<&str>,
        outcome: crate::durable::BoundaryState,
    ) -> StoreResult<Ask> {
        let ask_id = ask_id.clone();
        let invocation_id = invocation_id.clone();
        let reason = reason.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.close_ask_invocation(&ask_id, &invocation_id, reason.as_deref(), outcome)
        })
        .await
    }

    pub async fn escalate_ask(
        &self,
        ask_id: &AskId,
        invocation_id: &AgentInvocationId,
    ) -> StoreResult<Ask> {
        let ask_id = ask_id.clone();
        let invocation_id = invocation_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.escalate_ask(&ask_id, &invocation_id)
        })
        .await
    }

    pub async fn escalate_queued_ask(
        &self,
        actor: Option<&RunContext>,
        ask_id: &AskId,
    ) -> StoreResult<Ask> {
        let actor = actor.cloned();
        let ask_id = ask_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.escalate_queued_ask(actor.as_ref(), &ask_id)
        })
        .await
    }

    pub async fn cancel_ask(
        &self,
        actor: Option<&RunContext>,
        ask_id: &AskId,
        reason: &str,
    ) -> StoreResult<Ask> {
        let actor = actor.cloned();
        let ask_id = ask_id.clone();
        let reason = reason.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.cancel_ask(actor.as_ref(), &ask_id, &reason)
        })
        .await
    }

    pub async fn reconcile_ask(
        &self,
        invocation_id: &AgentInvocationId,
        observation: ContainmentObservation,
    ) -> StoreResult<Ask> {
        let invocation_id = invocation_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reconcile_ask(&invocation_id, observation)
        })
        .await
    }

    pub async fn ask_invocations(&self, ask_id: &AskId) -> StoreResult<Vec<AgentInvocation>> {
        let ask_id = ask_id.clone();
        run_sqlite(&self.sqlite, move |store| store.ask_invocations(&ask_id)).await
    }

    pub(crate) async fn ask_presentation(
        &self,
        invocation_id: &AgentInvocationId,
    ) -> StoreResult<(bool, bool)> {
        let invocation_id = invocation_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.ask_presentation(&invocation_id)
        })
        .await
    }

    pub(crate) async fn request_intervention(
        &self,
        lease: &RunContext,
        invocation_id: &AgentInvocationId,
        prompt: &str,
        user: bool,
    ) -> StoreResult<Ask> {
        let lease = lease.clone();
        let invocation_id = invocation_id.clone();
        let prompt = prompt.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.request_intervention(&lease, &invocation_id, &prompt, user)
        })
        .await
    }

    pub(crate) async fn asks_for_work_epoch(&self, lease: &RunContext) -> StoreResult<Vec<Ask>> {
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| store.asks_for_work_epoch(&lease)).await
    }

    pub(crate) async fn pending_ask_comment_writes(&self) -> StoreResult<Vec<AskCommentWrite>> {
        run_sqlite(&self.sqlite, move |store| {
            store.pending_ask_comment_writes()
        })
        .await
    }

    pub(crate) async fn claim_ask_comment_write(
        &self,
        ask_id: &AskId,
        transition: AskCommentTransition,
        attempted_at: i64,
        stale_before: i64,
    ) -> StoreResult<Option<AskCommentWrite>> {
        let ask_id = ask_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_ask_comment_write(&ask_id, transition, attempted_at, stale_before)
        })
        .await
    }

    pub(crate) async fn complete_ask_comment_write(
        &self,
        ask_id: &AskId,
        transition: AskCommentTransition,
        comment_id: &str,
        delivered_at: i64,
    ) -> StoreResult<()> {
        let ask_id = ask_id.clone();
        let comment_id = comment_id.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_ask_comment_write(&ask_id, transition, &comment_id, delivered_at)
        })
        .await
    }

    pub(crate) async fn fail_ask_comment_write(
        &self,
        ask_id: &AskId,
        transition: AskCommentTransition,
        error: &str,
    ) -> StoreResult<()> {
        let ask_id = ask_id.clone();
        let error = error.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.fail_ask_comment_write(&ask_id, transition, &error)
        })
        .await
    }

    pub async fn has_pending_user_ask_for_work(&self, work: &WorkRef) -> StoreResult<bool> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.has_pending_user_ask_for_work(&work)
        })
        .await
    }

    pub async fn invocation_surface(
        &self,
        invocation_id: &AgentInvocationId,
    ) -> StoreResult<Option<InvocationSurface>> {
        let invocation_id = invocation_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.invocation_surface(&invocation_id)
        })
        .await
    }

    pub(crate) async fn resolve_invocation_id(
        &self,
        selector: &str,
    ) -> StoreResult<Option<AgentInvocationId>> {
        let selector = selector.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.resolve_invocation_id(&selector)
        })
        .await
    }

    pub(crate) async fn open_invocation(
        &self,
        lease: &RunContext,
    ) -> StoreResult<Option<AgentInvocation>> {
        let lease = lease.clone();
        let invocation_id = std::env::var_os(crate::durable::AGENT_INVOCATION_ENV)
            .map(|value| {
                let value = value.into_string().map_err(|_| {
                    StoreError::InvalidData("LF_AGENT_INVOCATION_ID is not valid UTF-8".to_string())
                })?;
                AgentInvocationId::parse(&value)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))
            })
            .transpose()?;
        run_sqlite(&self.sqlite, move |store| {
            store.validate_run_context(&lease)?;
            match invocation_id {
                Some(invocation_id) => {
                    store.open_invocation_for_run_by_id(&lease.run_id, &invocation_id)
                }
                None => store.open_invocation_for_run(&lease.run_id),
            }
        })
        .await
    }

    pub(crate) async fn open_invocation_for_run(
        &self,
        run_id: &crate::durable::RunId,
    ) -> StoreResult<Option<AgentInvocation>> {
        let run_id = run_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.open_invocation_for_run(&run_id)
        })
        .await
    }

    pub(crate) async fn invocations_for_run(
        &self,
        run_id: &crate::durable::RunId,
    ) -> StoreResult<Vec<AgentInvocation>> {
        let run_id = run_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.invocations_for_run(&run_id)
        })
        .await
    }

    /// Reconcile an observed Run after its writer has disappeared.
    ///
    /// Only proven absence releases the exact Run slot; live or unprovable
    /// containment remains fenced for a later keeper pass.
    pub(crate) async fn recover_run(
        &self,
        run_id: &crate::durable::RunId,
        containment: ContainmentObservation,
    ) -> StoreResult<StopReceipt> {
        let run_id = run_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.recover_run(&run_id, containment)
        })
        .await
    }

    pub async fn invocation_surfaces(
        &self,
        active_only: bool,
    ) -> StoreResult<Vec<InvocationSurface>> {
        run_sqlite(&self.sqlite, move |store| {
            store.invocation_surfaces(active_only)
        })
        .await
    }

    pub async fn observe_invocation_provider(
        &self,
        lease: &RunContext,
        invocation_id: &AgentInvocationId,
        account_id: Option<crate::store::ProviderAccountId>,
        resume_token: Option<String>,
    ) -> StoreResult<AgentInvocation> {
        let lease = lease.clone();
        let invocation_id = invocation_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.observe_invocation_provider(
                &lease,
                &invocation_id,
                account_id.as_ref(),
                resume_token.as_deref(),
            )
        })
        .await
    }

    pub async fn handback_invocation(
        &self,
        invocation_id: &AgentInvocationId,
        outcome: crate::durable::BoundaryState,
    ) -> StoreResult<InvocationSurface> {
        let invocation_id = invocation_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.handback_invocation(&invocation_id, outcome)
        })
        .await
    }

    pub async fn interrupt(
        &self,
        actor: Option<&RunContext>,
        work: &WorkRef,
        if_run: &crate::durable::RunId,
    ) -> StoreResult<InterruptReceipt> {
        let actor = actor.cloned();
        let work = work.clone();
        let if_run = if_run.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.interrupt(actor.as_ref(), &work, &if_run)
        })
        .await
    }

    pub async fn done(&self, lease: &RunContext, basis: &Basis) -> StoreResult<DoneProposal> {
        let lease = lease.clone();
        let basis = basis.clone();
        run_sqlite(&self.sqlite, move |store| store.done(&lease, &basis)).await
    }

    pub async fn abandon(
        &self,
        work: &WorkRef,
        reason: &str,
        if_basis: &Basis,
    ) -> StoreResult<EpochReceipt> {
        let work = work.clone();
        let reason = reason.to_string();
        let if_basis = if_basis.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.abandon(&work, &reason, &if_basis)
        })
        .await
    }

    pub async fn work_status(&self, work: &WorkRef) -> StoreResult<WorkStatus> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.work_status(&work)).await
    }

    pub async fn work_for_child(&self, target: &ChildRef) -> StoreResult<WorkRef> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| store.work_for_child(&target)).await
    }

    pub async fn current_epoch(&self, work: &WorkRef) -> StoreResult<crate::durable::Epoch> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.current_epoch(&work)).await
    }

    pub async fn boundary_seed(&self, work: &WorkRef) -> StoreResult<BoundarySeed> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.boundary_seed(&work)).await
    }

    pub(crate) async fn boundary_seed_for_child(
        &self,
        target: &ChildRef,
    ) -> StoreResult<BoundarySeed> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.boundary_seed_for_child(&target)
        })
        .await
    }

    pub async fn steer(
        &self,
        actor: Option<&RunContext>,
        work: &WorkRef,
        text: &str,
        if_basis: Option<&Basis>,
    ) -> StoreResult<SteerReceipt> {
        let actor = actor.cloned();
        let work = work.clone();
        let text = text.to_string();
        let if_basis = if_basis.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.steer(actor.as_ref(), &work, &text, if_basis.as_ref())
        })
        .await
    }

    pub(crate) async fn append_steer(
        &self,
        work: &WorkRef,
        author: Author,
        text: &str,
        if_basis: Option<&Basis>,
    ) -> StoreResult<SteerReceipt> {
        let work = work.clone();
        let text = text.to_string();
        let if_basis = if_basis.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.append_steer(&work, &author, &text, if_basis.as_ref())
        })
        .await
    }

    pub async fn write_tool_response(
        &self,
        work: &WorkRef,
        write: ToolResponseWrite,
        if_basis: Option<&Basis>,
    ) -> StoreResult<(ToolResponseReceipt, bool)> {
        let work = work.clone();
        let if_basis = if_basis.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.write_tool_response(&work, &write, if_basis.as_ref())
        })
        .await
    }

    pub async fn tool_response(
        &self,
        work: &WorkRef,
        request_id: &str,
    ) -> StoreResult<Option<ToolResponseReceipt>> {
        let work = work.clone();
        let request_id = request_id.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.tool_response(&work, &request_id)
        })
        .await
    }

    pub async fn begin_live_send(
        &self,
        steer_id: &SteerId,
        turn_id: &str,
    ) -> StoreResult<Option<Send>> {
        let steer_id = steer_id.clone();
        let turn_id = turn_id.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.begin_live_send(&steer_id, &turn_id)
        })
        .await
    }

    pub async fn finish_send(
        &self,
        send_id: &SendId,
        state: SendState,
        provider_turn_id: Option<&str>,
        reason: Option<&str>,
    ) -> StoreResult<Send> {
        let send_id = send_id.clone();
        let provider_turn_id = provider_turn_id.map(ToString::to_string);
        let reason = reason.map(ToString::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.finish_send(
                &send_id,
                state,
                provider_turn_id.as_deref(),
                reason.as_deref(),
            )
        })
        .await
    }

    pub async fn validate_completion_basis(
        &self,
        work: &WorkRef,
        basis: &Basis,
    ) -> StoreResult<()> {
        let work = work.clone();
        let basis = basis.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.validate_completion_basis(&work, &basis)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use time::OffsetDateTime;

    use crate::durable::{
        AskBody, AskOrigin, AskResult, AskState, AskTarget, BoundaryState, Containment,
        ContainmentObservation, FlowPosition, InvocationRoute, RunAdvance, RunContext, RunState,
        RunTrigger, StopCause, WorkRef, WorkStatus,
    };
    use crate::id::WaveId;
    use crate::planning::{LinearProjectId, ProjectPlan};
    use crate::project::{Project, ProjectId};
    use crate::store::{open_store, StorageConfig, StoreError};
    use crate::wave::Wave;

    impl super::Store {
        pub(crate) async fn claim_test_ask(
            &self,
            actor: Option<&RunContext>,
            ask_id: &crate::durable::AskId,
        ) -> crate::store::StoreResult<crate::durable::AskClaim> {
            self.claim_ask(
                actor,
                ask_id,
                InvocationRoute {
                    provider: "codex".to_string(),
                    model: None,
                    account_id: None,
                },
                "ask_tui",
            )
            .await
        }
    }

    async fn _open_status_truth_store(database: PathBuf) -> super::Store {
        let store = open_store(&StorageConfig::sqlite(database.clone()))
            .await
            .unwrap();
        let connection = rusqlite::Connection::open(database).unwrap();
        let applied = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'run_liveness')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap();
        if !applied {
            connection
                .execute_batch(&crate::store::migrations::migration_sql_for_test(
                    "status_truth",
                ))
                .unwrap();
        }
        store
    }

    async fn wave_work() -> (super::Store, WorkRef) {
        let directory = tempfile::tempdir().unwrap().keep();
        let store = _open_status_truth_store(directory.join("registry.db")).await;
        let wave = Wave::new(
            WaveId::new(),
            "runtime".to_string(),
            directory.display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        (store, WorkRef::Wave(wave.id().clone()))
    }

    fn project_for(wave: &Wave) -> Project {
        let now = OffsetDateTime::now_utc();
        Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                id: LinearProjectId::new(format!("linear-{}", wave.id())).unwrap(),
                slug: "runtime-project".to_string(),
                name: "Runtime Project".to_string(),
                prompt_context: "Answer child questions.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        }
    }

    async fn start_invocation(
        store: &super::Store,
        work: &WorkRef,
    ) -> (crate::durable::RunContext, crate::durable::AgentInvocation) {
        let (_run, lease) = store.reserve_run(work, RunTrigger::User).await.unwrap();
        store
            .advance_run(
                &lease,
                RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: "lf-runtime".to_string(),
                    },
                    cwd: PathBuf::from("/tmp/runtime"),
                },
            )
            .await
            .unwrap();
        let receipt = store
            .advance_run(
                &lease,
                RunAdvance::InvocationStarting {
                    route: InvocationRoute {
                        provider: "codex".to_string(),
                        model: None,
                        account_id: None,
                    },
                    surface: "tui".to_string(),
                    resume_token: None,
                    answer_ask_id: None,
                },
            )
            .await
            .unwrap();
        let crate::durable::AdvanceReceipt::Invocation(invocation) = receipt else {
            panic!("expected Invocation receipt")
        };
        (lease, invocation)
    }

    struct AskFixture {
        store: super::Store,
        parent_work: WorkRef,
        parent_lease: crate::durable::RunContext,
        child_lease: crate::durable::RunContext,
        child_invocation: crate::durable::AgentInvocation,
        turn: crate::durable::Turn,
    }

    async fn ask_fixture() -> AskFixture {
        let directory = tempfile::tempdir().unwrap().keep();
        let store = _open_status_truth_store(directory.join("registry.db")).await;
        let wave = Wave::new(
            WaveId::new(),
            "runtime".to_string(),
            directory.display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let parent_work = WorkRef::Wave(wave.id().clone());
        let (parent_lease, _) = start_invocation(&store, &parent_work).await;
        let project = project_for(&wave);
        store.create_project(&project).await.unwrap();
        let child_work = WorkRef::Project(project.id.clone());
        let (child_lease, child_invocation) = start_invocation(&store, &child_work).await;
        let crate::durable::AdvanceReceipt::Turn(turn) = store
            .advance_run(
                &child_lease,
                RunAdvance::TurnStarting {
                    invocation_id: child_invocation.id.clone(),
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected Turn receipt")
        };
        AskFixture {
            store,
            parent_work,
            parent_lease,
            child_lease,
            child_invocation,
            turn,
        }
    }

    async fn create_parent_ask(fixture: &AskFixture, prompt: &str) -> crate::durable::Ask {
        let run = fixture
            .store
            .run_by_id(&fixture.child_lease.run_id)
            .await
            .unwrap();
        fixture
            .store
            .create_ask(
                &fixture.child_lease,
                AskOrigin {
                    work: fixture.child_lease.work.clone(),
                    run_id: run.id,
                    turn_id: Some(fixture.turn.id.clone()),
                    invocation_id: Some(fixture.child_invocation.id.clone()),
                    home_id: run.home_id,
                    cwd: run.cwd.unwrap(),
                },
                AskBody::Intervention {
                    prompt: prompt.to_string(),
                },
                AskTarget::Parent(fixture.parent_work.clone()),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn intervention_routes_to_parent_by_default_and_user_only_explicitly() {
        let fixture = ask_fixture().await;
        let parent = fixture
            .store
            .request_intervention(
                &fixture.child_lease,
                &fixture.child_invocation.id,
                "Choose the proof",
                false,
            )
            .await
            .unwrap();
        assert_eq!(
            parent.target,
            AskTarget::Parent(fixture.parent_work.clone())
        );
        fixture
            .store
            .cancel_ask(
                Some(&fixture.child_lease),
                &parent.id,
                "route proof complete",
            )
            .await
            .unwrap();

        let user = fixture
            .store
            .request_intervention(
                &fixture.child_lease,
                &fixture.child_invocation.id,
                "Connect the account",
                true,
            )
            .await
            .unwrap();
        assert_eq!(user.target, AskTarget::User);

        let (root_store, root_work) = wave_work().await;
        let (root_lease, root_invocation) = start_invocation(&root_store, &root_work).await;
        root_store
            .advance_run(
                &root_lease,
                RunAdvance::TurnStarting {
                    invocation_id: root_invocation.id.clone(),
                },
            )
            .await
            .unwrap();
        let missing_parent = root_store
            .request_intervention(
                &root_lease,
                &root_invocation.id,
                "Do not escalate silently",
                false,
            )
            .await
            .unwrap_err();
        assert!(missing_parent.to_string().contains("lf ask --user"));
        assert_eq!(
            root_store
                .request_intervention(
                    &root_lease,
                    &root_invocation.id,
                    "User intervention is genuinely required",
                    true,
                )
                .await
                .unwrap()
                .target,
            AskTarget::User
        );
    }

    #[tokio::test]
    async fn interrupt_ends_a_reserved_run_before_containment_exists() {
        let (store, work) = wave_work().await;
        let (run, _lease) = store.reserve_run(&work, RunTrigger::User).await.unwrap();

        let receipt = store.interrupt(None, &work, &run.id).await.unwrap();

        assert_eq!(receipt.run_id, run.id);
        assert!(receipt.turn_ids.is_empty());
        assert!(store.current_run(&work).await.unwrap().is_none());
        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Ready);
    }

    #[tokio::test]
    async fn listener_interrupt_releases_run_authority_for_restart() {
        let (store, work) = wave_work().await;
        let (_, lease) = store.reserve_run(&work, RunTrigger::User).await.unwrap();

        let stopped = store.stop_run_on_interrupt(&lease).unwrap();

        assert_eq!(stopped.run.state, RunState::Ended);
        assert!(store.current_run(&work).await.unwrap().is_none());
        assert!(store.reserve_run(&work, RunTrigger::User).await.is_ok());
    }

    #[tokio::test]
    async fn reserving_a_fenced_run_names_the_existing_authority() {
        let (store, work) = wave_work().await;
        let (run, _) = store.reserve_run(&work, RunTrigger::User).await.unwrap();

        let error = store
            .reserve_run(&work, RunTrigger::User)
            .await
            .expect_err("second Run must remain fenced");

        assert!(matches!(
            error,
            StoreError::RunFenced { run_id, state, .. }
                if run_id == run.id && state == RunState::Reserved
        ));
    }

    #[tokio::test]
    async fn one_run_can_supervise_overlapping_invocations_without_changing_containment() {
        let (store, work) = wave_work().await;
        let (lease, first) = start_invocation(&store, &work).await;
        let next = RunAdvance::InvocationStarting {
            route: InvocationRoute {
                provider: "claude".to_string(),
                model: None,
                account_id: None,
            },
            surface: "headless".to_string(),
            resume_token: None,
            answer_ask_id: None,
        };

        let crate::durable::AdvanceReceipt::Invocation(second) =
            store.advance_run(&lease, next).await.unwrap()
        else {
            panic!("expected second Invocation")
        };
        assert_eq!(first.supervising_run_id, Some(lease.run_id.clone()));
        assert_eq!(second.supervising_run_id, Some(lease.run_id.clone()));
        assert_eq!(second.route.provider, "claude");
        store
            .advance_run(
                &lease,
                RunAdvance::InvocationEnded {
                    invocation_id: first.id.clone(),
                    outcome: BoundaryState::Succeeded,
                },
            )
            .await
            .unwrap();
        let invocations = store.invocations_for_run(&lease.run_id).await.unwrap();
        assert!(invocations[0].ended_at.is_some());
        assert!(invocations[1].ended_at.is_none());
        let run = store.current_run(&work).await.unwrap().unwrap();
        assert_eq!(run.state, RunState::Active);
        assert_eq!(
            run.containment,
            Some(Containment::Tmux {
                name: "lf-runtime".into()
            })
        );
    }

    #[tokio::test]
    async fn parent_ask_session_wins_and_stale_run_cannot_claim() {
        let directory = tempfile::tempdir().unwrap().keep();
        let store = open_store(&StorageConfig::sqlite(directory.join("registry.db")))
            .await
            .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "runtime".to_string(),
            directory.display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let parent_work = WorkRef::Wave(wave.id().clone());
        let (parent_lease, _parent_invocation) = start_invocation(&store, &parent_work).await;
        let project = project_for(&wave);
        store.create_project(&project).await.unwrap();
        let child_work = WorkRef::Project(project.id.clone());
        let (child_lease, child_invocation) = start_invocation(&store, &child_work).await;
        let crate::durable::AdvanceReceipt::Turn(turn) = store
            .advance_run(
                &child_lease,
                RunAdvance::TurnStarting {
                    invocation_id: child_invocation.id.clone(),
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected Turn receipt")
        };

        let ask = store
            .sqlite
            .request_intervention(
                &child_lease,
                &child_invocation.id,
                "Which proof matters?",
                false,
            )
            .unwrap();
        assert_eq!(ask.origin.turn_id, Some(turn.id));
        assert_eq!(
            ask.target,
            crate::durable::AskTarget::Parent(parent_work.clone())
        );
        let duplicate = store
            .sqlite
            .request_intervention(
                &child_lease,
                &child_invocation.id,
                "Which proof matters?",
                false,
            )
            .unwrap();
        assert_ne!(
            duplicate.id, ask.id,
            "duplicate request text must not collapse Ask identity"
        );
        assert_eq!(
            store
                .pending_asks(Some(&parent_lease), &AskTarget::Parent(parent_work.clone()),)
                .await
                .unwrap(),
            vec![ask.clone(), duplicate]
        );

        store
            .stop_run(
                &parent_lease,
                StopCause::Requested,
                ContainmentObservation::Absent,
            )
            .await
            .unwrap();
        assert!(matches!(
            store.claim_test_ask(Some(&parent_lease), &ask.id).await,
            Err(StoreError::InvalidAuthority(_))
        ));

        store
            .stop_run(
                &child_lease,
                StopCause::Recovery,
                ContainmentObservation::Absent,
            )
            .await
            .unwrap();
        assert_eq!(
            store.ask_by_id(&ask.id).await.unwrap(),
            ask,
            "runner loss must not erase its unanswered Ask"
        );

        let (replacement_lease, _replacement_invocation) =
            start_invocation(&store, &parent_work).await;
        let claim = store
            .claim_test_ask(Some(&replacement_lease), &ask.id)
            .await
            .unwrap();
        store
            .mark_ask_ready(&ask.id, &claim.invocation_id)
            .await
            .unwrap();
        store
            .mark_ask_presented(&ask.id, &claim.invocation_id)
            .await
            .unwrap();
        store
            .stop_run(
                &replacement_lease,
                StopCause::Requested,
                ContainmentObservation::Absent,
            )
            .await
            .unwrap();
        let result = AskResult::Resolved {
            summary: "The live blocking exchange.".to_string(),
        };
        let settled = store
            .settle_ask(&ask.id, &claim.invocation_id, result.clone())
            .await
            .unwrap();
        assert_eq!(settled.id, ask.id);
        assert_eq!(
            settled.terminal_author,
            Some(crate::durable::Author::Run(
                replacement_lease.run_id.clone()
            ))
        );
        assert_eq!(
            store
                .settle_ask(&ask.id, &claim.invocation_id, result)
                .await
                .unwrap(),
            settled
        );
        assert!(matches!(
            store
                .settle_ask(
                    &ask.id,
                    &claim.invocation_id,
                    AskResult::Resolved {
                        summary: "A different answer".to_string()
                    }
                )
                .await,
            Err(StoreError::InvalidAuthority(_))
        ));
        let (recovery_lease, recovery_invocation) = start_invocation(&store, &child_work).await;
        store
            .advance_run(
                &recovery_lease,
                RunAdvance::TurnStarting {
                    invocation_id: recovery_invocation.id.clone(),
                },
            )
            .await
            .unwrap();
        let current = store
            .asks_for_work_epoch(&recovery_lease)
            .await
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == ask.id)
            .unwrap();
        assert_eq!(current.result, settled.result);
    }

    #[tokio::test]
    async fn ask_claim_is_idempotent_and_first_terminal_result_wins() {
        let fixture = ask_fixture().await;
        let ask = create_parent_ask(&fixture, "Which proof matters?").await;

        let first = fixture.store.claim_test_ask(None, &ask.id).await.unwrap();
        assert!(first.needs_launch);
        let reopened = fixture
            .store
            .claim_test_ask(Some(&fixture.parent_lease), &ask.id)
            .await
            .unwrap();
        assert_eq!(reopened.invocation_id, first.invocation_id);
        assert!(!reopened.needs_launch);

        let ask_invocation = fixture
            .store
            .ask_invocations(&ask.id)
            .await
            .unwrap()
            .into_iter()
            .find(|invocation| invocation.id == first.invocation_id)
            .unwrap();
        let surface = fixture
            .store
            .invocation_surface(&ask_invocation.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(surface.run.cwd, Some(ask.origin.cwd.clone()));
        assert_eq!(
            surface.attach_argv,
            Some(vec![
                "tmux".to_string(),
                "attach-session".to_string(),
                "-t".to_string(),
                crate::ops::ask::session_name(&ask_invocation.id),
            ])
        );
        assert!(matches!(
            fixture
                .store
                .mark_ask_presented(&ask.id, &first.invocation_id)
                .await,
            Err(StoreError::InvalidAuthority(_))
        ));
        fixture
            .store
            .mark_ask_ready(&ask.id, &first.invocation_id)
            .await
            .unwrap();
        fixture
            .store
            .mark_ask_presented(&ask.id, &first.invocation_id)
            .await
            .unwrap();
        let resolved = AskResult::Resolved {
            summary: "The durable blocking exchange.".to_string(),
        };
        let declined = AskResult::Declined {
            reason: "Different terminal result".to_string(),
        };
        let (resolved_write, declined_write) = tokio::join!(
            fixture
                .store
                .settle_ask(&ask.id, &first.invocation_id, resolved.clone()),
            fixture
                .store
                .settle_ask(&ask.id, &first.invocation_id, declined.clone()),
        );
        let (settled, committed, rejected) = match (resolved_write, declined_write) {
            (Ok(ask), Err(error)) => (ask, resolved, error),
            (Err(error), Ok(ask)) => (ask, declined, error),
            outcome => panic!("exactly one settlement must commit: {outcome:?}"),
        };
        assert!(matches!(rejected, StoreError::InvalidAuthority(_)));
        assert_eq!(settled.state, committed.state());
        assert_eq!(settled.active_invocation_id, None);
        assert_eq!(settled.result, Some(committed.clone()));
        assert_eq!(
            fixture
                .store
                .settle_ask(&ask.id, &first.invocation_id, committed)
                .await
                .unwrap(),
            settled,
            "an exact retry observes the first committed result"
        );
        assert!(fixture
            .store
            .pending_asks(
                Some(&fixture.parent_lease),
                &AskTarget::Parent(fixture.parent_work),
            )
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn headless_ask_surface_keeps_its_liveness_attach_route() {
        let fixture = ask_fixture().await;
        let ask = create_parent_ask(&fixture, "Supervise the detached Ask session").await;
        let claim = fixture
            .store
            .claim_ask(
                Some(&fixture.parent_lease),
                &ask.id,
                InvocationRoute {
                    provider: "claude".to_string(),
                    model: Some("sonnet".to_string()),
                    account_id: None,
                },
                "ask_headless",
            )
            .await
            .unwrap();
        assert!(claim.needs_launch);
        let invocation = fixture
            .store
            .ask_invocations(&ask.id)
            .await
            .unwrap()
            .into_iter()
            .find(|invocation| invocation.id == claim.invocation_id)
            .unwrap();

        let surface = fixture
            .store
            .invocation_surface(&invocation.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(surface.run.cwd, Some(ask.origin.cwd));
        assert_eq!(
            surface.attach_argv,
            Some(vec![
                "tmux".to_string(),
                "attach-session".to_string(),
                "-t".to_string(),
                crate::ops::ask::session_name(&invocation.id),
            ])
        );
    }

    #[tokio::test]
    async fn only_the_settling_attempt_can_replay_its_terminal_write() {
        let fixture = ask_fixture().await;
        let ask = create_parent_ask(&fixture, "Fence stale Ask authority").await;
        let first = fixture
            .store
            .claim_test_ask(Some(&fixture.parent_lease), &ask.id)
            .await
            .unwrap();
        fixture
            .store
            .release_ask(
                &ask.id,
                &first.invocation_id,
                Some("retry with a new attempt"),
            )
            .await
            .unwrap();

        let second = fixture
            .store
            .claim_test_ask(Some(&fixture.parent_lease), &ask.id)
            .await
            .unwrap();
        fixture
            .store
            .mark_ask_ready(&ask.id, &second.invocation_id)
            .await
            .unwrap();
        fixture
            .store
            .mark_ask_presented(&ask.id, &second.invocation_id)
            .await
            .unwrap();
        let result = AskResult::Resolved {
            summary: "The current attempt settled the Ask.".to_string(),
        };
        fixture
            .store
            .settle_ask(&ask.id, &second.invocation_id, result.clone())
            .await
            .unwrap();

        assert!(matches!(
            fixture
                .store
                .settle_ask(&ask.id, &first.invocation_id, result)
                .await,
            Err(StoreError::InvalidAuthority(_))
        ));
    }

    #[tokio::test]
    async fn escalation_keeps_ask_identity_and_cancellation_closes_the_attempt() {
        let fixture = ask_fixture().await;
        let ask = create_parent_ask(&fixture, "Need User judgment").await;
        let claim = fixture
            .store
            .claim_test_ask(Some(&fixture.parent_lease), &ask.id)
            .await
            .unwrap();
        let escalated = fixture
            .store
            .escalate_ask(&ask.id, &claim.invocation_id)
            .await
            .unwrap();
        assert_eq!(escalated.id, ask.id);
        assert_eq!(escalated.state, AskState::Queued);
        assert_eq!(escalated.target, AskTarget::User);
        assert!(matches!(
            fixture
                .store
                .mark_ask_ready(&ask.id, &claim.invocation_id)
                .await,
            Err(StoreError::InvalidAuthority(_))
        ));
        let retargeted = fixture
            .store
            .claim_test_ask(Some(&fixture.parent_lease), &ask.id)
            .await
            .unwrap();
        assert!(retargeted.needs_launch);
        let user_claim = fixture.store.claim_test_ask(None, &ask.id).await.unwrap();
        assert_eq!(user_claim.invocation_id, retargeted.invocation_id);
        assert!(!user_claim.needs_launch);
        let cancelled = fixture
            .store
            .cancel_ask(None, &ask.id, "No intervention needed")
            .await
            .unwrap();
        assert_eq!(cancelled.id, ask.id);
        assert_eq!(cancelled.state, AskState::Cancelled);
        assert_eq!(cancelled.active_invocation_id, None);
        assert_eq!(
            cancelled.result,
            Some(AskResult::Cancelled {
                reason: "No intervention needed".to_string()
            })
        );
        let invocations = fixture.store.ask_invocations(&ask.id).await.unwrap();
        assert_eq!(invocations.len(), 2);
        assert!(invocations[0].ended_at.is_some());
        assert_eq!(invocations[1].id, user_claim.invocation_id);
        assert!(invocations[1].ended_at.is_some());
        for invocation in invocations {
            assert_eq!(
                fixture
                    .store
                    .invocation_surface(&invocation.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .handback,
                Some(BoundaryState::Unknown)
            );
        }
    }

    #[tokio::test]
    async fn remote_unreachable_stays_claimed_but_local_absence_requeues() {
        let fixture = ask_fixture().await;
        let ask = create_parent_ask(&fixture, "Recover this Ask session").await;
        let claim = fixture
            .store
            .claim_test_ask(Some(&fixture.parent_lease), &ask.id)
            .await
            .unwrap();

        let remote_unreachable = fixture
            .store
            .reconcile_ask(&claim.invocation_id, ContainmentObservation::Unprovable)
            .await
            .unwrap();
        assert_eq!(remote_unreachable.state, AskState::Claimed);
        let still_present = fixture
            .store
            .reconcile_ask(&claim.invocation_id, ContainmentObservation::Present)
            .await
            .unwrap();
        assert_eq!(still_present.state, AskState::Claimed);

        let locally_absent = fixture
            .store
            .reconcile_ask(&claim.invocation_id, ContainmentObservation::Absent)
            .await
            .unwrap();
        assert_eq!(locally_absent.state, AskState::Queued);
        assert_eq!(locally_absent.active_invocation_id, None);
        let history = fixture.store.ask_invocations(&ask.id).await.unwrap();
        assert!(history[0].ended_at.is_some());
        assert_eq!(
            fixture
                .store
                .invocation_surface(&history[0].id)
                .await
                .unwrap()
                .unwrap()
                .handback,
            Some(BoundaryState::Unknown)
        );
        let retry = fixture
            .store
            .claim_test_ask(Some(&fixture.parent_lease), &ask.id)
            .await
            .unwrap();
        assert_ne!(retry.invocation_id, claim.invocation_id);
    }

    #[tokio::test]
    async fn ask_invocation_endings_requeue_without_settling() {
        let fixture = ask_fixture().await;
        let ask = create_parent_ask(&fixture, "Keep the Ask pending").await;
        let first = fixture
            .store
            .claim_test_ask(Some(&fixture.parent_lease), &ask.id)
            .await
            .unwrap();
        let released = fixture
            .store
            .release_ask(
                &ask.id,
                &first.invocation_id,
                Some("session exited cleanly"),
            )
            .await
            .unwrap();
        assert_eq!(released.state, AskState::Queued);
        assert_eq!(released.result, None);

        let second = fixture
            .store
            .claim_test_ask(Some(&fixture.parent_lease), &ask.id)
            .await
            .unwrap();
        let interrupted = fixture
            .store
            .release_ask(
                &ask.id,
                &second.invocation_id,
                Some("session received TERM"),
            )
            .await
            .unwrap();
        assert_eq!(interrupted.state, AskState::Queued);
        assert_eq!(interrupted.result, None);

        let invocations = fixture.store.ask_invocations(&ask.id).await.unwrap();
        assert!(invocations
            .iter()
            .all(|invocation| invocation.ended_at.is_some()));
        for invocation in invocations {
            assert_eq!(
                fixture
                    .store
                    .invocation_surface(&invocation.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .handback,
                Some(BoundaryState::Unknown)
            );
        }
    }

    #[tokio::test]
    async fn parent_lane_adopts_a_claimed_attempt_before_queued_work() {
        let fixture = ask_fixture().await;
        let claimed = create_parent_ask(&fixture, "Keep this Ask session attached").await;
        fixture
            .store
            .claim_test_ask(Some(&fixture.parent_lease), &claimed.id)
            .await
            .unwrap();
        let run = fixture
            .store
            .run_by_id(&fixture.child_lease.run_id)
            .await
            .unwrap();
        let queued = fixture
            .store
            .create_ask(
                &fixture.child_lease,
                AskOrigin {
                    work: run.work,
                    run_id: run.id,
                    turn_id: None,
                    invocation_id: None,
                    home_id: run.home_id,
                    cwd: run.cwd.unwrap(),
                },
                AskBody::FlowStep {
                    flow: "task-first".to_string(),
                    node_id: "queued-review".to_string(),
                    skill: "review".to_string(),
                    iteration: 0,
                },
                AskTarget::Parent(fixture.parent_work.clone()),
            )
            .await
            .unwrap();

        let mut lane = crate::ops::ask::AskLane::new(
            fixture.parent_work.clone(),
            fixture.parent_lease.clone(),
        );
        let store = Arc::new(fixture.store);
        assert!(lane.reconcile(&store).await.unwrap());
        assert_eq!(
            store.ask_by_id(&queued.id).await.unwrap().state,
            AskState::Queued
        );
        assert_eq!(
            store.ask_by_id(&claimed.id).await.unwrap().state,
            AskState::Claimed
        );
    }

    #[tokio::test]
    async fn flow_created_ask_needs_no_invocation_and_epoch_cleanup_is_historical() {
        let (store, work) = wave_work().await;
        let (run, lease) = store.reserve_run(&work, RunTrigger::User).await.unwrap();
        store
            .advance_run(
                &lease,
                RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: "lf-flow-ask".to_string(),
                    },
                    cwd: PathBuf::from("/tmp/flow-ask"),
                },
            )
            .await
            .unwrap();
        let origin = AskOrigin {
            work: work.clone(),
            run_id: run.id,
            turn_id: None,
            invocation_id: None,
            home_id: run.home_id,
            cwd: PathBuf::from("/tmp/flow-ask"),
        };
        let request = AskBody::FlowStep {
            flow: "task-design".to_string(),
            node_id: "review_kickoff".to_string(),
            skill: "review-design".to_string(),
            iteration: 0,
        };
        store
            .set_flow_position(
                &lease,
                FlowPosition {
                    work: work.clone(),
                    epoch_id: lease.basis.epoch_id.clone(),
                    flow: "task-design".to_string(),
                    step: "review-design".to_string(),
                    node_id: Some("review_kickoff".to_string()),
                    human: true,
                    step_index: 1,
                    iteration: 0,
                    updated_at: OffsetDateTime::now_utc(),
                },
            )
            .await
            .unwrap();
        let ask = store
            .create_ask(&lease, origin.clone(), request.clone(), AskTarget::User)
            .await
            .unwrap();
        let replay = store
            .create_ask(&lease, origin, request, AskTarget::User)
            .await
            .unwrap();
        assert_eq!(replay.id, ask.id);
        assert_eq!(ask.origin.invocation_id, None);
        let claim = store.claim_test_ask(None, &ask.id).await.unwrap();
        store
            .set_flow_position(
                &lease,
                FlowPosition {
                    work: work.clone(),
                    epoch_id: lease.basis.epoch_id.clone(),
                    flow: "task-design".to_string(),
                    step: "kickoff".to_string(),
                    node_id: None,
                    human: false,
                    step_index: 0,
                    iteration: 0,
                    updated_at: OffsetDateTime::now_utc(),
                },
            )
            .await
            .unwrap();
        assert!(matches!(
            store
                .claim_flow_step_run_context(&ask.id, &claim.invocation_id)
                .await,
            Err(StoreError::InvalidAuthority(_))
        ));
        store
            .set_flow_position(
                &lease,
                FlowPosition {
                    work: work.clone(),
                    epoch_id: lease.basis.epoch_id.clone(),
                    flow: "task-design".to_string(),
                    step: "review-design".to_string(),
                    node_id: Some("review_kickoff".to_string()),
                    human: true,
                    step_index: 1,
                    iteration: 0,
                    updated_at: OffsetDateTime::now_utc(),
                },
            )
            .await
            .unwrap();
        let flow_writer = store
            .claim_flow_step_run_context(&ask.id, &claim.invocation_id)
            .await
            .unwrap()
            .expect("flow-step Ask receives the current writer");
        store.validate_run_context(&flow_writer).await.unwrap();
        store.validate_run_context(&lease).await.unwrap();
        store
            .mark_ask_ready(&ask.id, &claim.invocation_id)
            .await
            .unwrap();
        store
            .mark_ask_presented(&ask.id, &claim.invocation_id)
            .await
            .unwrap();
        store
            .set_flow_position(
                &flow_writer,
                FlowPosition {
                    work: work.clone(),
                    epoch_id: flow_writer.basis.epoch_id.clone(),
                    flow: "task-design".to_string(),
                    step: "kickoff".to_string(),
                    node_id: None,
                    human: false,
                    step_index: 0,
                    iteration: 0,
                    updated_at: OffsetDateTime::now_utc(),
                },
            )
            .await
            .unwrap();
        assert!(matches!(
            store
                .settle_ask(
                    &ask.id,
                    &claim.invocation_id,
                    AskResult::Resolved {
                        summary: "stale acceptance".to_string()
                    }
                )
                .await,
            Err(StoreError::InvalidAuthority(_))
        ));
        store
            .release_ask(&ask.id, &claim.invocation_id, Some("flow position changed"))
            .await
            .unwrap();
        store
            .abandon(&work, "directive withdrawn", &lease.basis)
            .await
            .unwrap();
        let cancelled = store.ask_by_id(&ask.id).await.unwrap();
        assert_eq!(cancelled.state, AskState::Cancelled);
        assert_eq!(cancelled.active_invocation_id, None);
        assert_eq!(
            cancelled.result,
            Some(AskResult::Cancelled {
                reason: "owning Work Epoch abandoned".to_string()
            })
        );
        assert!(store.ask_invocations(&ask.id).await.unwrap()[0]
            .ended_at
            .is_some());
        assert!(matches!(
            store
                .mark_ask_presented(&ask.id, &claim.invocation_id)
                .await,
            Err(StoreError::InvalidAuthority(_))
        ));
    }

    #[tokio::test]
    async fn unprovable_containment_keeps_the_run_slot_fenced() {
        let (store, work) = wave_work().await;
        let (lease, _invocation) = start_invocation(&store, &work).await;
        let stopped = store
            .stop_run(
                &lease,
                StopCause::Recovery,
                ContainmentObservation::Unprovable,
            )
            .await
            .unwrap();
        assert_eq!(stopped.run.state, RunState::Stopping);
        assert!(store.reserve_run(&work, RunTrigger::User).await.is_err());

        let reaped = store
            .stop_run(&lease, StopCause::Recovery, ContainmentObservation::Absent)
            .await
            .unwrap();
        assert_eq!(reaped.run.state, RunState::Ended);
        let (recovery, _) = store
            .reserve_run(
                &work,
                RunTrigger::Recovery {
                    prior_run_id: reaped.run.id,
                },
            )
            .await
            .unwrap();
        assert_eq!(recovery.state, RunState::Reserved);
    }

    #[tokio::test]
    async fn keeper_recovery_releases_the_exact_absent_run() {
        let (store, work) = wave_work().await;
        let (lease, _invocation) = start_invocation(&store, &work).await;

        let recovered = store
            .recover_run(&lease.run_id, ContainmentObservation::Absent)
            .await
            .unwrap();
        assert_eq!(recovered.run.state, RunState::Ended);
        assert!(store.validate_run_context(&lease).await.is_err());

        let (next, _) = store
            .reserve_run(
                &work,
                RunTrigger::Recovery {
                    prior_run_id: recovered.run.id,
                },
            )
            .await
            .unwrap();
        assert_eq!(next.state, RunState::Reserved);
    }

    #[tokio::test]
    async fn invocation_order_does_not_fence_run_recovery() {
        let (store, work) = wave_work().await;
        let (lease, _first) = start_invocation(&store, &work).await;
        store
            .advance_run(
                &lease,
                RunAdvance::InvocationStarting {
                    route: InvocationRoute {
                        provider: "claude".to_string(),
                        model: None,
                        account_id: None,
                    },
                    surface: "headless".to_string(),
                    resume_token: None,
                    answer_ask_id: None,
                },
            )
            .await
            .unwrap();
        let recovered = store
            .recover_run(&lease.run_id, ContainmentObservation::Absent)
            .await
            .unwrap();
        assert_eq!(recovered.run.state, RunState::Ended);
    }

    #[tokio::test]
    async fn only_the_placed_home_can_reserve_and_live_work_cannot_move() {
        let (store, work) = wave_work().await;
        let local = store.placement(&work).await.unwrap();
        assert_eq!(local.home_id, store.local_home().await.unwrap().id);
        assert!(local.enabled);

        let remote = store
            .observe_home(&crate::durable::HomeId::new(), "ssh://jack@buildbox")
            .await
            .unwrap();
        let placed = store.place_work(&work, &remote.id).await.unwrap();
        assert_eq!(placed.home_id, remote.id);
        assert!(matches!(
            store.reserve_run(&work, RunTrigger::User).await,
            Err(StoreError::InvalidData(message)) if message.contains("it is placed on")
        ));

        let moved = store.place_work(&work, &local.home_id).await.unwrap();
        assert_eq!(moved.home_id, local.home_id);
        let (run, lease) = store.reserve_run(&work, RunTrigger::User).await.unwrap();
        assert_eq!(run.home_id, local.home_id);
        assert!(matches!(
            store.place_work(&work, &remote.id).await,
            Err(StoreError::InvalidData(message)) if message.contains("cannot move wave")
        ));

        store
            .stop_run(&lease, StopCause::Requested, ContainmentObservation::Absent)
            .await
            .unwrap();
        let moved = store.place_work(&work, &remote.id).await.unwrap();
        assert_eq!(moved.home_id, remote.id);
    }

    #[tokio::test]
    async fn disabled_work_cannot_reserve_a_run_and_remains_disabled_when_moved() {
        let (store, work) = wave_work().await;
        let local = store.local_home().await.unwrap();

        let disabled = store.set_work_enabled(&work, false).await.unwrap();
        assert!(!disabled.enabled);
        assert!(matches!(
            store.reserve_run(&work, RunTrigger::User).await,
            Err(StoreError::InvalidData(message)) if message.contains("is disabled")
        ));

        let remote = store
            .observe_home(&crate::durable::HomeId::new(), "ssh://jack@buildbox")
            .await
            .unwrap();
        assert!(!store.place_work(&work, &remote.id).await.unwrap().enabled);
        assert!(!store.place_work(&work, &local.id).await.unwrap().enabled);

        assert!(store.set_work_enabled(&work, true).await.unwrap().enabled);
        store.reserve_run(&work, RunTrigger::User).await.unwrap();
    }

    #[tokio::test]
    async fn local_home_route_cannot_be_observed_as_remote() {
        let (store, _) = wave_work().await;
        let local = store.local_home().await.unwrap();

        assert!(matches!(
            store.observe_home(&local.id, "ssh://jack@elsewhere").await,
            Err(StoreError::InvalidData(message)) if message.contains("cannot replace local Home")
        ));
        assert_eq!(store.local_home().await.unwrap(), local);
    }

    #[tokio::test]
    async fn successful_turn_is_a_completion_basis() {
        let (store, work) = wave_work().await;
        let (lease, invocation) = start_invocation(&store, &work).await;
        let receipt = store
            .advance_run(
                &lease,
                RunAdvance::TurnStarting {
                    invocation_id: invocation.id.clone(),
                },
            )
            .await
            .unwrap();
        let crate::durable::AdvanceReceipt::Turn(turn) = receipt else {
            panic!("expected Turn receipt")
        };
        let basis = turn.basis.clone();
        store
            .advance_run(
                &lease,
                RunAdvance::TurnEnded {
                    turn_id: turn.id,
                    outcome: BoundaryState::Succeeded,
                },
            )
            .await
            .unwrap();
        store
            .advance_run(
                &lease,
                RunAdvance::InvocationEnded {
                    invocation_id: invocation.id,
                    outcome: BoundaryState::Succeeded,
                },
            )
            .await
            .unwrap();
        store.done(&lease, &basis).await.unwrap();
        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Done);
    }

    #[tokio::test]
    async fn invocation_surface_reopens_without_owning_liveness() {
        let (store, work) = wave_work().await;
        let (lease, invocation) = start_invocation(&store, &work).await;

        let first = store
            .invocation_surface(&invocation.id)
            .await
            .unwrap()
            .unwrap();
        let reopened = store
            .invocation_surface(&invocation.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first, reopened);
        assert_eq!(first.attach_argv.as_ref().unwrap()[0], "tmux");

        let ended = store
            .handback_invocation(&invocation.id, BoundaryState::Unknown)
            .await
            .unwrap();
        assert!(ended.invocation.ended_at.is_some());
        assert_eq!(ended.handback, Some(BoundaryState::Unknown));

        let receipt = store
            .advance_run(
                &lease,
                RunAdvance::InvocationStarting {
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
            .await
            .unwrap();
        let crate::durable::AdvanceReceipt::Invocation(headless) = receipt else {
            panic!("expected Invocation receipt")
        };
        assert!(store
            .handback_invocation(&headless.id, BoundaryState::Succeeded)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn interactive_launch_and_late_handback_are_independent_evidence() {
        let (store, work) = wave_work().await;
        let (_lease, invocation) = start_invocation(&store, &work).await;

        let launched = store
            .invocation_surface(&invocation.id)
            .await
            .unwrap()
            .unwrap();
        assert!(launched.invocation.ended_at.is_none());
        assert_eq!(launched.handback, None);
        assert_eq!(store.invocation_surfaces(true).await.unwrap().len(), 1);

        let handed_back = store
            .handback_invocation(&invocation.id, BoundaryState::Failed)
            .await
            .unwrap();
        assert!(handed_back.invocation.ended_at.is_some());
        assert_eq!(handed_back.handback, Some(BoundaryState::Failed));
        assert!(store.invocation_surfaces(true).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn failed_interactive_launch_is_not_an_active_surface() {
        let (store, work) = wave_work().await;
        let (lease, invocation) = start_invocation(&store, &work).await;

        store
            .advance_run(
                &lease,
                RunAdvance::InvocationEnded {
                    invocation_id: invocation.id.clone(),
                    outcome: BoundaryState::Failed,
                },
            )
            .await
            .unwrap();

        let failed = store
            .invocation_surface(&invocation.id)
            .await
            .unwrap()
            .unwrap();
        assert!(failed.invocation.ended_at.is_some());
        assert_eq!(failed.handback, None);
        assert!(store.invocation_surfaces(true).await.unwrap().is_empty());
    }
}
