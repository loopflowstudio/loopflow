use crate::child::ChildRef;
use crate::durable::{
    AdvanceReceipt, AgentInvocation, AgentInvocationId, Answer, AnswerAttemptHistory,
    AnswerContext, AskExchange, AskId, Author, Basis, BoundarySeed, ContainmentObservation,
    ControlCtx, DoneProposal, EpochReceipt, FlowPosition, Home, HomeId, InterruptReceipt,
    InvocationSurface, Placement, Run, RunAdvance, RunControl, RunLease, RunTrigger, Send, SendId,
    SendState, SteerId, SteerReceipt, StopCause, StopReceipt, ToolResponseReceipt,
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
    Ask,
    Answer,
}

impl AskCommentTransition {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Answer => "answer",
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

    pub(crate) async fn open_ask(
        &self,
        lease: &RunLease,
        invocation_id: &AgentInvocationId,
        question: &str,
    ) -> StoreResult<AskExchange> {
        let lease = lease.clone();
        let invocation_id = invocation_id.clone();
        let question = question.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.open_ask(&lease, &invocation_id, &question)
        })
        .await
    }

    pub(crate) async fn current_ask(
        &self,
        lease: &RunLease,
        invocation_id: &AgentInvocationId,
        ask_id: Option<&AskId>,
    ) -> StoreResult<AskExchange> {
        let lease = lease.clone();
        let invocation_id = invocation_id.clone();
        let ask_id = ask_id.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.current_ask(&lease, &invocation_id, ask_id.as_ref())
        })
        .await
    }

    pub async fn answer_ask(
        &self,
        context: &ControlCtx<'_>,
        ask_id: &AskId,
        text: &str,
    ) -> StoreResult<Answer> {
        let lease = match context {
            ControlCtx::User(_) => None,
            ControlCtx::Run(lease) => Some((*lease).clone()),
        };
        let ask_id = ask_id.clone();
        let text = text.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.answer_ask(lease.as_ref(), &ask_id, &text)
        })
        .await
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

    pub async fn pending_asks_for_parent(&self, parent: &WorkRef) -> StoreResult<Vec<AskExchange>> {
        let parent = parent.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.pending_asks_for_parent(&parent)
        })
        .await
    }

    pub(crate) async fn oldest_answer_context(
        &self,
        parent: &WorkRef,
    ) -> StoreResult<Option<AnswerContext>> {
        let parent = parent.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.oldest_answer_context(&parent)
        })
        .await
    }

    pub(crate) async fn answer_attempt_history(
        &self,
        ask_id: &AskId,
    ) -> StoreResult<AnswerAttemptHistory> {
        let ask_id = ask_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.answer_attempt_history(&ask_id)
        })
        .await
    }

    pub async fn pending_user_asks(&self) -> StoreResult<Vec<AskExchange>> {
        run_sqlite(&self.sqlite, move |store| store.pending_user_asks()).await
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
        AuthenticatedRequest, BoundaryState, Containment, ContainmentObservation, ControlCtx,
        InvocationRoute, RunAdvance, RunState, RunTrigger, StopCause, WorkRef, WorkStatus,
    };
    use crate::id::WaveId;
    use crate::planning::{LinearProjectId, ProjectPlan};
    use crate::project::{Project, ProjectId};
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
    async fn parent_answer_wins_and_stale_run_cannot_write() {
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
            .open_ask(&child_lease, &child_invocation.id, "Which proof matters?")
            .unwrap();
        assert_eq!(ask.turn_id, turn.id);
        assert_eq!(
            ask.route,
            crate::durable::AnswerRoute::Parent(parent_work.clone())
        );
        let recovered = store
            .sqlite
            .open_ask(&child_lease, &child_invocation.id, "Which proof matters?")
            .unwrap();
        assert_eq!(recovered.id, ask.id);
        assert_eq!(
            store.pending_asks_for_parent(&parent_work).await.unwrap(),
            vec![ask.clone()]
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
            store
                .answer_ask(
                    &ControlCtx::Run(&parent_lease),
                    &ask.id,
                    "The live blocking exchange."
                )
                .await,
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
            store.pending_asks_for_parent(&parent_work).await.unwrap(),
            vec![ask.clone()],
            "runner loss must not erase its unanswered Ask"
        );

        let (replacement_lease, _replacement_invocation) =
            start_invocation(&store, &parent_work).await;
        let answer = store
            .answer_ask(
                &ControlCtx::Run(&replacement_lease),
                &ask.id,
                "The live blocking exchange.",
            )
            .await
            .unwrap();
        assert_eq!(answer.ask_id, ask.id);
        assert_eq!(
            store
                .answer_ask(
                    &ControlCtx::Run(&replacement_lease),
                    &ask.id,
                    "The live blocking exchange.",
                )
                .await
                .unwrap(),
            answer
        );
        assert!(matches!(
            store
                .answer_ask(
                    &ControlCtx::Run(&replacement_lease),
                    &ask.id,
                    "A different answer",
                )
                .await,
            Err(StoreError::InvalidAuthority(_))
        ));
        assert!(store
            .pending_asks_for_parent(&parent_work)
            .await
            .unwrap()
            .is_empty());

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
            .current_ask(&recovery_lease, &recovery_invocation.id, Some(&ask.id))
            .await
            .unwrap();
        assert_eq!(current.answer, Some(answer));
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
    async fn invocation_surface_reopens_without_owning_liveness() {
        let (store, work) = wave_work().await;
        let (_lease, invocation) = start_invocation(&store, &work).await;

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
    }
}
