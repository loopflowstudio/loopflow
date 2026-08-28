use crate::child::ChildRef;
use crate::durable::{
    AbandonReceipt, Ask, AskBody, AskClaim, AskId, AskOrigin, AskResult, AskTarget, Author,
    FlowPosition, Home, HomeId, Placement, ProjectChildControlBasis, ProjectChildControlToken,
    ProjectId, RunId, Steer, SteerReceipt, TaskId, ToolResponseReceipt, ToolResponseWrite, WorkRef,
    WorkStatus,
};

use super::{run_sqlite, Store, StoreResult};

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
    pub(crate) async fn task_issue_identifier(
        &self,
        external_issue_id: &str,
    ) -> StoreResult<Option<String>> {
        let external_issue_id = external_issue_id.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.task_issue_identifier(&external_issue_id)
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

    pub async fn create_ask(
        &self,
        origin: AskOrigin,
        request: AskBody,
        target: AskTarget,
    ) -> StoreResult<Ask> {
        run_sqlite(&self.sqlite, move |store| {
            store.create_ask(&origin, &request, &target)
        })
        .await
    }

    pub async fn ask_by_id(&self, ask_id: &AskId) -> StoreResult<Ask> {
        let ask_id = ask_id.clone();
        run_sqlite(&self.sqlite, move |store| store.ask_by_id(&ask_id)).await
    }

    pub async fn pending_asks(&self, target: &AskTarget) -> StoreResult<Vec<Ask>> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| store.pending_asks(&target)).await
    }

    pub(crate) async fn claim_ask(&self, ask_id: &AskId) -> StoreResult<AskClaim> {
        let ask_id = ask_id.clone();
        run_sqlite(&self.sqlite, move |store| store.claim_ask(&ask_id)).await
    }

    pub async fn mark_ask_presented(&self, ask_id: &AskId, run_id: &RunId) -> StoreResult<Ask> {
        let ask_id = ask_id.clone();
        let run_id = run_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_ask_presented(&ask_id, &run_id)
        })
        .await
    }

    pub(crate) fn interrupt_ask_on_interrupt(
        &self,
        ask_id: &AskId,
        run_id: &RunId,
    ) -> StoreResult<()> {
        self.sqlite
            .interrupt_ask_on_interrupt(ask_id, run_id)
            .map(|_| ())
    }

    pub async fn settle_ask(
        &self,
        ask_id: &AskId,
        run_id: &RunId,
        result: AskResult,
    ) -> StoreResult<Ask> {
        let ask_id = ask_id.clone();
        let run_id = run_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.settle_ask(&ask_id, &run_id, &result)
        })
        .await
    }

    pub async fn release_ask(
        &self,
        ask_id: &AskId,
        run_id: &RunId,
        reason: Option<&str>,
    ) -> StoreResult<Ask> {
        let ask_id = ask_id.clone();
        let run_id = run_id.clone();
        let reason = reason.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.release_ask(&ask_id, &run_id, reason.as_deref())
        })
        .await
    }

    pub async fn escalate_ask(&self, ask_id: &AskId, run_id: &RunId) -> StoreResult<Ask> {
        let ask_id = ask_id.clone();
        let run_id = run_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.escalate_ask(&ask_id, &run_id)
        })
        .await
    }

    pub async fn escalate_queued_ask(&self, ask_id: &AskId) -> StoreResult<Ask> {
        let ask_id = ask_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.escalate_queued_ask(&ask_id)
        })
        .await
    }

    pub async fn cancel_ask(&self, ask_id: &AskId, reason: &str) -> StoreResult<Ask> {
        let ask_id = ask_id.clone();
        let reason = reason.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.cancel_ask(&ask_id, &reason)
        })
        .await
    }

    pub(crate) async fn request_intervention(
        &self,
        origin: AskOrigin,
        prompt: &str,
        user: bool,
    ) -> StoreResult<Ask> {
        let prompt = prompt.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.request_intervention(&origin, &prompt, user)
        })
        .await
    }

    pub(crate) async fn asks_for_work(&self, work: &WorkRef) -> StoreResult<Vec<Ask>> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.asks_for_work(&work)).await
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

    pub async fn abandon(&self, work: &WorkRef, reason: &str) -> StoreResult<AbandonReceipt> {
        let work = work.clone();
        let reason = reason.to_string();
        run_sqlite(&self.sqlite, move |store| store.abandon(&work, &reason)).await
    }

    pub async fn work_status(&self, work: &WorkRef) -> StoreResult<WorkStatus> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.work_status(&work)).await
    }

    pub async fn work_for_child(&self, target: &ChildRef) -> StoreResult<WorkRef> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| store.work_for_child(&target)).await
    }

    pub async fn work_steers(&self, work: &WorkRef) -> StoreResult<Vec<Steer>> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.work_steers(&work)).await
    }

    pub(crate) async fn work_steers_for_child(&self, target: &ChildRef) -> StoreResult<Vec<Steer>> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.work_steers_for_child(&target)
        })
        .await
    }

    pub(crate) async fn begin_project_child_control(
        &self,
        project_id: &ProjectId,
        run_id: &RunId,
        basis: &ProjectChildControlBasis,
    ) -> StoreResult<ProjectChildControlToken> {
        let project_id = project_id.clone();
        let run_id = run_id.clone();
        let basis = basis.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.begin_project_child_control(&project_id, &run_id, &basis)
        })
        .await
    }

    pub(crate) async fn advance_project_child_control(
        &self,
        project_id: &ProjectId,
        run_id: &RunId,
        token: &ProjectChildControlToken,
        basis: &ProjectChildControlBasis,
    ) -> StoreResult<()> {
        let project_id = project_id.clone();
        let run_id = run_id.clone();
        let token = token.clone();
        let basis = basis.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.advance_project_child_control(&project_id, &run_id, &token, &basis)
        })
        .await
    }

    pub(crate) async fn authorize_project_child_control(
        &self,
        task_id: &TaskId,
        run_id: &RunId,
        token: &ProjectChildControlToken,
    ) -> StoreResult<()> {
        let task_id = task_id.clone();
        let run_id = run_id.clone();
        let token = token.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.authorize_project_child_control(&task_id, &run_id, &token)
        })
        .await
    }

    pub(crate) async fn release_project_child_control(
        &self,
        project_id: &ProjectId,
        run_id: &RunId,
        token: &ProjectChildControlToken,
    ) -> StoreResult<()> {
        let project_id = project_id.clone();
        let run_id = run_id.clone();
        let token = token.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.release_project_child_control(&project_id, &run_id, &token)
        })
        .await
    }

    pub(crate) async fn append_steer(
        &self,
        work: &WorkRef,
        author: Author,
        text: &str,
    ) -> StoreResult<SteerReceipt> {
        let work = work.clone();
        let text = text.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.append_steer(&work, &author, &text)
        })
        .await
    }

    pub async fn write_tool_response(
        &self,
        work: &WorkRef,
        write: ToolResponseWrite,
    ) -> StoreResult<(ToolResponseReceipt, bool)> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.write_tool_response(&work, &write)
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
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use crate::durable::{AskBody, AskOrigin, AskResult, AskState, AskTarget, WorkRef};
    use crate::id::WaveId;
    use crate::planning::{LinearProjectId, ProjectPlan};
    use crate::store::{open_store, StorageConfig, StoreError};
    use crate::work::project::{Project, ProjectId};
    use crate::work::wave::Wave;

    async fn wave_work() -> (super::Store, WorkRef) {
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
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        }
    }

    async fn planning_ask_fixture() -> (super::Store, WorkRef, WorkRef, crate::durable::HomeId) {
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
        let parent = WorkRef::Wave(wave.id().clone());
        let project = project_for(&wave);
        store.create_project(&project).await.unwrap();
        let child = WorkRef::Project(project.id.clone());
        let home_id = store.placement(&child).await.unwrap().home_id;
        (store, parent, child, home_id)
    }

    #[tokio::test]
    async fn ask_claim_uses_a_generic_run_identity() {
        let (store, parent, child, home_id) = planning_ask_fixture().await;
        let source_run_id = crate::durable::RunId::new();
        let ask = store
            .create_ask(
                AskOrigin {
                    work: child,
                    source_run_id: Some(source_run_id.clone()),
                    home_id,
                    cwd: "/tmp/runtime".into(),
                },
                AskBody::Intervention {
                    prompt: "Which proof matters?".to_string(),
                },
                AskTarget::Parent(parent),
            )
            .await
            .unwrap();

        let claim = store.claim_ask(&ask.id).await.unwrap();
        assert!(claim.needs_launch);
        let claimed = store.ask_by_id(&ask.id).await.unwrap();
        assert_eq!(claimed.origin.source_run_id, Some(source_run_id));
        assert_eq!(claimed.active_run_id, Some(claim.run_id.clone()));
        assert!(claimed.ready_at.is_some());

        store
            .mark_ask_presented(&ask.id, &claim.run_id)
            .await
            .unwrap();
        let result = AskResult::Resolved {
            summary: "Use the observable boundary".to_string(),
        };
        let settled = store
            .settle_ask(&ask.id, &claim.run_id, result.clone())
            .await
            .unwrap();
        assert_eq!(settled.state, AskState::Resolved);
        assert_eq!(settled.result, Some(result));
        assert!(settled.active_run_id.is_none());
        assert!(settled.ready_at.is_some());
        assert!(settled.presented_at.is_some());
    }

    #[tokio::test]
    async fn released_ask_claim_requeues_with_a_fresh_generic_run() {
        let (store, parent, child, home_id) = planning_ask_fixture().await;
        let ask = store
            .create_ask(
                AskOrigin {
                    work: child,
                    source_run_id: None,
                    home_id,
                    cwd: "/tmp/runtime".into(),
                },
                AskBody::Intervention {
                    prompt: "Try again safely".to_string(),
                },
                AskTarget::Parent(parent),
            )
            .await
            .unwrap();
        let first = store.claim_ask(&ask.id).await.unwrap();
        store
            .release_ask(&ask.id, &first.run_id, Some("provider exited"))
            .await
            .unwrap();
        let second = store.claim_ask(&ask.id).await.unwrap();
        assert_ne!(first.run_id, second.run_id);
        assert!(second.needs_launch);
    }

    #[tokio::test]
    async fn disabled_work_remains_disabled_when_moved() {
        let (store, work) = wave_work().await;
        let local = store.local_home().await.unwrap();

        let disabled = store.set_work_enabled(&work, false).await.unwrap();
        assert!(!disabled.enabled);

        let remote = store
            .observe_home(&crate::durable::HomeId::new(), "ssh://jack@buildbox")
            .await
            .unwrap();
        assert!(!store.place_work(&work, &remote.id).await.unwrap().enabled);
        assert!(!store.place_work(&work, &local.id).await.unwrap().enabled);

        assert!(store.set_work_enabled(&work, true).await.unwrap().enabled);
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
}
