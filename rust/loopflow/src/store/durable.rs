use crate::child::ChildRef;
use crate::durable::{
    AdvanceReceipt, AgentInvocation, AgentInvocationId, AttentionRoute, Author, Basis,
    BoundarySeed, ChildFeedback, ContainmentObservation, ControlCtx, DoneProposal, EpochReceipt,
    Feedback, FlowPosition, Home, HomeId, InterruptReceipt, InvocationSurface, Placement, Run,
    RunAdvance, RunControl, RunLease, RunTrigger, Send, SendId, SendState, SteerId, SteerReceipt,
    StopCause, StopReceipt, ToolResponseReceipt, ToolResponseWrite, UserFeedback, WorkRef,
    WorkStatus,
};

use super::{run_sqlite, Store, StoreError, StoreResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskWriterState {
    pub work: WorkRef,
    pub identifier: String,
    pub run: Option<Run>,
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
    ) -> StoreResult<(Run, RunLease)> {
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

    pub(crate) async fn reserve_recovery_run(
        &self,
        lease: &RunLease,
    ) -> StoreResult<(Run, RunLease)> {
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

    pub(crate) async fn resolve_run_lease(
        &self,
        token: crate::durable::RunLeaseToken,
    ) -> StoreResult<RunLease> {
        run_sqlite(&self.sqlite, move |store| store.resolve_run_lease(&token)).await
    }

    pub(crate) async fn validate_run_lease(&self, lease: &RunLease) -> StoreResult<()> {
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| store.validate_run_lease(&lease)).await
    }

    pub async fn advance_run(
        &self,
        lease: &RunLease,
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
        lease: &RunLease,
        cause: StopCause,
        containment: ContainmentObservation,
    ) -> StoreResult<StopReceipt> {
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.stop_run(&lease, &cause, containment)
        })
        .await
    }

    pub(crate) async fn run_control(
        &self,
        lease: &RunLease,
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
        lease: &RunLease,
        position: FlowPosition,
    ) -> StoreResult<FlowPosition> {
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.set_flow_position(&lease, &position)
        })
        .await
    }

    pub async fn route_feedback(
        &self,
        lease: &RunLease,
        invocation_id: &AgentInvocationId,
        attention: AttentionRoute,
    ) -> StoreResult<Feedback> {
        let lease = lease.clone();
        let invocation_id = invocation_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.route_feedback(&lease, &invocation_id, &attention)
        })
        .await
    }

    pub async fn feedback(&self, work: &WorkRef) -> StoreResult<Option<Feedback>> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.feedback(&work)).await
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

    pub(crate) async fn open_invocation(
        &self,
        lease: &RunLease,
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
            store.validate_run_lease(&lease)?;
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
        lease: &RunLease,
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

    pub async fn child_attention(&self, parent: &WorkRef) -> StoreResult<Vec<ChildFeedback>> {
        let parent = parent.clone();
        run_sqlite(&self.sqlite, move |store| store.child_attention(&parent)).await
    }

    pub async fn user_attention(&self) -> StoreResult<Vec<UserFeedback>> {
        run_sqlite(&self.sqlite, move |store| store.user_attention()).await
    }

    pub async fn continue_feedback(
        &self,
        context: &ControlCtx<'_>,
        work: &WorkRef,
        if_basis: &Basis,
    ) -> StoreResult<WorkStatus> {
        let context = match context {
            ControlCtx::User(_) => None,
            ControlCtx::Run(lease) => Some((*lease).clone()),
        };
        let work = work.clone();
        let if_basis = if_basis.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.continue_feedback(context.as_ref(), &work, &if_basis)
        })
        .await
    }

    pub async fn interrupt(
        &self,
        context: &ControlCtx<'_>,
        work: &WorkRef,
        if_run: &crate::durable::RunId,
    ) -> StoreResult<InterruptReceipt> {
        let context = match context {
            ControlCtx::User(_) => None,
            ControlCtx::Run(lease) => Some((*lease).clone()),
        };
        let work = work.clone();
        let if_run = if_run.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.interrupt(context.as_ref(), &work, &if_run)
        })
        .await
    }

    pub async fn done(&self, lease: &RunLease, basis: &Basis) -> StoreResult<DoneProposal> {
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
        context: &ControlCtx<'_>,
        work: &WorkRef,
        text: &str,
        if_basis: Option<&Basis>,
    ) -> StoreResult<SteerReceipt> {
        let caller = match context {
            ControlCtx::User(_) => None,
            ControlCtx::Run(lease) => Some((*lease).clone()),
        };
        let work = work.clone();
        let text = text.to_string();
        let if_basis = if_basis.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.steer(caller.as_ref(), &work, &text, if_basis.as_ref())
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

    use time::OffsetDateTime;

    use crate::durable::{
        AttentionRoute, AuthenticatedRequest, BoundaryState, Containment, ContainmentObservation,
        ControlCtx, FlowPosition, InvocationRoute, RunAdvance, RunState, RunTrigger, StopCause,
        WorkRef, WorkStatus,
    };
    use crate::id::WaveId;
    use crate::store::{open_store, StorageConfig, StoreError};
    use crate::wave::Wave;

    async fn wave_work() -> (super::Store, WorkRef) {
        let directory = tempfile::tempdir().unwrap().keep();
        let database = directory.join("registry.db");
        let store = open_store(&StorageConfig::sqlite(database.clone()))
            .await
            .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "runtime".to_string(),
            directory.display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        (store, WorkRef::Wave(wave.id().clone()))
    }

    async fn start_invocation(
        store: &super::Store,
        work: &WorkRef,
    ) -> (crate::durable::RunLease, crate::durable::AgentInvocation) {
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
                },
            )
            .await
            .unwrap();
        let crate::durable::AdvanceReceipt::Invocation(invocation) = receipt else {
            panic!("expected Invocation receipt")
        };
        (lease, invocation)
    }

    #[tokio::test]
    async fn interrupt_ends_a_reserved_run_before_containment_exists() {
        let (store, work) = wave_work().await;
        let (run, _lease) = store.reserve_run(&work, RunTrigger::User).await.unwrap();
        let request = AuthenticatedRequest::cli();

        let receipt = store
            .interrupt(&ControlCtx::User(&request), &work, &run.id)
            .await
            .unwrap();

        assert_eq!(receipt.run_id, run.id);
        assert!(receipt.turn_ids.is_empty());
        assert!(store.current_run(&work).await.unwrap().is_none());
        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Ready);
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
    async fn feedback_stays_open_until_explicit_continue() {
        let (store, work) = wave_work().await;
        let (lease, invocation) = start_invocation(&store, &work).await;
        let initial = store.current_epoch(&work).await.unwrap().current_basis;
        store
            .set_flow_position(
                &lease,
                FlowPosition {
                    work: work.clone(),
                    epoch_id: initial.epoch_id.clone(),
                    flow: "wave-pursue".to_string(),
                    step: "design".to_string(),
                    step_index: 2,
                    iteration: 1,
                    feedback: true,
                    updated_at: OffsetDateTime::now_utc(),
                },
            )
            .await
            .unwrap();
        let feedback = store
            .route_feedback(&lease, &invocation.id, AttentionRoute::User)
            .await
            .unwrap();
        assert_eq!(feedback.work, work);
        assert_eq!(feedback.invocation_id, invocation.id);
        assert_eq!(feedback.position.step, "design");
        let queued = store.user_attention().await.unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].feedback, feedback);
        assert_eq!(queued[0].surface.invocation.id, invocation.id);

        let request = AuthenticatedRequest::cli();
        let steer = store
            .steer(
                &ControlCtx::User(&request),
                &work,
                "show the failure path",
                Some(&initial),
            )
            .await
            .unwrap();
        let parked = store
            .feedback(&work)
            .await
            .unwrap()
            .expect("a User response does not close the Feedback");
        assert!(parked.attention_at.is_none());
        assert!(matches!(
            store
                .continue_feedback(&ControlCtx::User(&request), &work, &initial)
                .await,
            Err(StoreError::StaleBasis { .. })
        ));
        assert!(matches!(
            store
                .continue_feedback(&ControlCtx::User(&request), &work, &steer.steer.basis,)
                .await
                .unwrap(),
            WorkStatus::Running { .. }
        ));
        assert!(store.feedback(&work).await.unwrap().is_none());
        assert!(store.user_attention().await.unwrap().is_empty());
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
        assert!(store.validate_run_lease(&lease).await.is_err());

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
    async fn invocation_surface_reopens_without_owning_liveness_and_handback_clears_attention() {
        let (store, work) = wave_work().await;
        let (lease, invocation) = start_invocation(&store, &work).await;
        let basis = store.current_epoch(&work).await.unwrap().current_basis;
        store
            .set_flow_position(
                &lease,
                FlowPosition {
                    work: work.clone(),
                    epoch_id: basis.epoch_id,
                    flow: "wave-pursue".to_string(),
                    step: "demo".to_string(),
                    step_index: 0,
                    iteration: 0,
                    feedback: true,
                    updated_at: OffsetDateTime::now_utc(),
                },
            )
            .await
            .unwrap();
        store
            .route_feedback(&lease, &invocation.id, AttentionRoute::User)
            .await
            .unwrap();

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
        assert!(store.feedback(&work).await.unwrap().is_some());

        let ended = store
            .handback_invocation(&invocation.id, BoundaryState::Unknown)
            .await
            .unwrap();
        assert!(ended.invocation.ended_at.is_some());
        assert_eq!(ended.handback, Some(BoundaryState::Unknown));
        assert!(store.feedback(&work).await.unwrap().is_none());
    }
}
