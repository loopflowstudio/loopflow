use crate::child_session::ChildRef;
use crate::durable::{
    AdvanceReceipt, AttentionRoute, Author, Basis, BoundarySeed, ContainmentObservation,
    ControlCtx, DoneProposal, EpochReceipt, FlowPosition, Home, HomeId, InterruptReceipt, LaunchId,
    LaunchSurface, Review, Run, RunAdvance, RunControl, RunLease, RunTrigger, Send, SendId,
    SendState, SteerId, SteerReceipt, StopCause, StopReceipt, ToolResponseReceipt,
    ToolResponseWrite, WorkRef, WorkStatus,
};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn home(&self, route: &str) -> StoreResult<Home> {
        let route = route.to_string();
        run_sqlite(&self.sqlite, move |store| store.home(&route)).await
    }

    pub async fn reserve_run(
        &self,
        work: &WorkRef,
        home_id: &HomeId,
        trigger: RunTrigger,
    ) -> StoreResult<(Run, RunLease)> {
        let work = work.clone();
        let home_id = home_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_run(&work, &home_id, &trigger)
        })
        .await
    }

    pub async fn current_run(&self, work: &WorkRef) -> StoreResult<Option<Run>> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.current_run(&work)).await
    }

    pub(crate) async fn run_lease_for_child(
        &self,
        target: &crate::child_session::ChildRef,
        lease: &crate::child_session::ChildWriteLease,
    ) -> StoreResult<RunLease> {
        let target = target.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.run_for_child_lease(&target, &lease)
        })
        .await
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

    pub async fn route_review(
        &self,
        lease: &RunLease,
        launch_id: &LaunchId,
        attention: AttentionRoute,
    ) -> StoreResult<Review> {
        let lease = lease.clone();
        let launch_id = launch_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.route_review(&lease, &launch_id, &attention)
        })
        .await
    }

    pub async fn review(&self, work: &WorkRef) -> StoreResult<Option<Review>> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.review(&work)).await
    }

    pub async fn launch_surface(&self, launch_id: &LaunchId) -> StoreResult<Option<LaunchSurface>> {
        let launch_id = launch_id.clone();
        run_sqlite(&self.sqlite, move |store| store.launch_surface(&launch_id)).await
    }

    pub async fn launch_surfaces(&self, active_only: bool) -> StoreResult<Vec<LaunchSurface>> {
        run_sqlite(&self.sqlite, move |store| {
            store.launch_surfaces(active_only)
        })
        .await
    }

    pub async fn handback_launch(
        &self,
        launch_id: &LaunchId,
        outcome: crate::durable::BoundaryState,
    ) -> StoreResult<LaunchSurface> {
        let launch_id = launch_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.handback_launch(&launch_id, outcome)
        })
        .await
    }

    pub async fn child_attention(&self, parent: &WorkRef) -> StoreResult<Vec<Review>> {
        let parent = parent.clone();
        run_sqlite(&self.sqlite, move |store| store.child_attention(&parent)).await
    }

    pub async fn close_review(
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
            store.close_review(context.as_ref(), &work, &if_basis)
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
        let author = match context {
            ControlCtx::User(_) => Author::User,
            ControlCtx::Run(lease) => Author::Run(lease.run_id.clone()),
        };
        self.append_steer(work, author, text, if_basis).await
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
        ControlCtx, FlowPosition, LaunchRoute, RunAdvance, RunState, RunTrigger, StopCause,
        WorkRef, WorkStatus,
    };
    use crate::id::WaveId;
    use crate::store::{open_store, StorageConfig, StoreError};
    use crate::wave::Wave;

    async fn wave_work() -> (super::Store, WorkRef, crate::durable::Home) {
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
        let home = store.home("test-home").await.unwrap();
        (store, WorkRef::Wave(wave.id().clone()), home)
    }

    async fn start_launch(
        store: &super::Store,
        work: &WorkRef,
        home: &crate::durable::Home,
        opaque: bool,
    ) -> (crate::durable::RunLease, crate::durable::Launch) {
        let (_run, lease) = store
            .reserve_run(work, &home.id, RunTrigger::User)
            .await
            .unwrap();
        let receipt = store
            .advance_run(
                &lease,
                RunAdvance::LaunchStarting {
                    route: LaunchRoute {
                        provider: "codex".to_string(),
                        model: None,
                        account_id: None,
                    },
                    containment: Containment::Tmux {
                        name: "lf-runtime".to_string(),
                    },
                    cwd: PathBuf::from("/tmp/runtime"),
                    surface: "tui".to_string(),
                    opaque,
                    resume_token: None,
                },
            )
            .await
            .unwrap();
        let crate::durable::AdvanceReceipt::Launch(launch) = receipt else {
            panic!("expected Launch receipt")
        };
        let receipt = store
            .advance_run(
                &lease,
                RunAdvance::LaunchLive {
                    launch_id: launch.id.clone(),
                },
            )
            .await
            .unwrap();
        let crate::durable::AdvanceReceipt::Launch(launch) = receipt else {
            panic!("expected Launch receipt")
        };
        (lease, launch)
    }

    #[tokio::test]
    async fn review_is_current_flow_launch_and_attention_not_a_stored_decision() {
        let (store, work, home) = wave_work().await;
        let (lease, launch) = start_launch(&store, &work, &home, false).await;
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
                    interactive: true,
                    updated_at: OffsetDateTime::now_utc(),
                },
            )
            .await
            .unwrap();
        let review = store
            .route_review(&lease, &launch.id, AttentionRoute::User)
            .await
            .unwrap();
        assert_eq!(review.work, work);
        assert_eq!(review.launch_id, launch.id);
        assert_eq!(review.position.step, "design");

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
        assert!(matches!(
            store
                .close_review(&ControlCtx::User(&request), &work, &initial)
                .await,
            Err(StoreError::StaleBasis { .. })
        ));
        assert!(matches!(
            store
                .close_review(&ControlCtx::User(&request), &work, &steer.steer.basis,)
                .await
                .unwrap(),
            WorkStatus::Running { .. }
        ));
        assert!(store.review(&work).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn unprovable_containment_keeps_the_run_slot_fenced() {
        let (store, work, home) = wave_work().await;
        let (lease, _launch) = start_launch(&store, &work, &home, false).await;
        let stopped = store
            .stop_run(
                &lease,
                StopCause::Recovery,
                ContainmentObservation::Unprovable,
            )
            .await
            .unwrap();
        assert_eq!(stopped.run.state, RunState::Stopping);
        assert!(store
            .reserve_run(&work, &home.id, RunTrigger::User)
            .await
            .is_err());

        let reaped = store
            .stop_run(&lease, StopCause::Recovery, ContainmentObservation::Absent)
            .await
            .unwrap();
        assert_eq!(reaped.run.state, RunState::Ended);
        let (recovery, _) = store
            .reserve_run(
                &work,
                &home.id,
                RunTrigger::Recovery {
                    prior_run_id: reaped.run.id,
                },
            )
            .await
            .unwrap();
        assert_eq!(recovery.state, RunState::Reserved);
    }

    #[tokio::test]
    async fn opaque_launch_success_is_a_completion_basis() {
        let (store, work, home) = wave_work().await;
        let (lease, launch) = start_launch(&store, &work, &home, true).await;
        let basis = launch.opaque_basis.clone().unwrap();
        store
            .advance_run(
                &lease,
                RunAdvance::LaunchEnded {
                    launch_id: launch.id,
                    outcome: BoundaryState::Succeeded,
                },
            )
            .await
            .unwrap();
        store.done(&lease, &basis).await.unwrap();
        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Done);
    }

    #[tokio::test]
    async fn launch_surface_reopens_without_owning_liveness_and_handback_clears_attention() {
        let (store, work, home) = wave_work().await;
        let (lease, launch) = start_launch(&store, &work, &home, true).await;
        let basis = launch.opaque_basis.clone().unwrap();
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
                    interactive: true,
                    updated_at: OffsetDateTime::now_utc(),
                },
            )
            .await
            .unwrap();
        store
            .route_review(&lease, &launch.id, AttentionRoute::User)
            .await
            .unwrap();

        let first = store.launch_surface(&launch.id).await.unwrap().unwrap();
        let reopened = store.launch_surface(&launch.id).await.unwrap().unwrap();
        assert_eq!(first, reopened);
        assert_eq!(first.attach_argv.as_ref().unwrap()[0], "tmux");
        assert!(store.review(&work).await.unwrap().is_some());

        let ended = store
            .handback_launch(&launch.id, BoundaryState::Unknown)
            .await
            .unwrap();
        assert_eq!(ended.launch.state, crate::durable::LaunchState::Ended);
        assert_eq!(ended.handback, Some(BoundaryState::Unknown));
        assert!(store.review(&work).await.unwrap().is_none());
    }
}
