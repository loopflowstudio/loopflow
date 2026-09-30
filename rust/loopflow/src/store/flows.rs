use time::OffsetDateTime;

use crate::durable::{
    FlowSession, TaskFlowBlocker, TaskId, TaskWorkerClaim, TaskWorkerClaimOutcome, TaskWorkerOwner,
};
use crate::engine::ExecutionCursor;

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn flow_inventory(
        &self,
        filter: &crate::durable::FlowFilter,
        after: Option<&str>,
        limit: std::num::NonZeroU32,
    ) -> StoreResult<crate::durable::FlowPage> {
        let filter = filter.clone();
        let after = after.map(str::to_owned);
        run_sqlite(&self.sqlite, move |store| {
            store.flow_inventory(&filter, after.as_deref(), limit)
        })
        .await
    }

    pub async fn flow_detail(
        &self,
        selector: &str,
    ) -> StoreResult<Option<crate::durable::FlowDetail>> {
        let selector = selector.to_string();
        run_sqlite(&self.sqlite, move |store| store.flow_detail(&selector)).await
    }

    /// Restore an old captured Flow without selecting or executing it.
    pub(crate) async fn import_flow(&self, flow: FlowSession, dry_run: bool) -> StoreResult<bool> {
        run_sqlite(&self.sqlite, move |store| store.import_flow(&flow, dry_run)).await
    }
    pub async fn create_flow(&self, flow: FlowSession) -> StoreResult<FlowSession> {
        run_sqlite(&self.sqlite, move |store| store.create_flow(&flow)).await
    }

    pub async fn start_task_flow(
        &self,
        task_id: &TaskId,
        flow: FlowSession,
    ) -> StoreResult<FlowSession> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.start_task_flow(&task_id, &flow)
        })
        .await
    }

    pub async fn flow(&self, id: &str) -> StoreResult<Option<FlowSession>> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.flow(&id)).await
    }

    pub async fn task_flow(&self, task_id: &TaskId) -> StoreResult<Option<FlowSession>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| store.task_flow(&task_id)).await
    }

    pub async fn recover_flow(
        &self,
        id: &str,
        claim: Option<&TaskWorkerClaim>,
    ) -> StoreResult<FlowSession> {
        let id = id.to_string();
        let claim = claim.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.recover_flow(&id, claim.as_ref())
        })
        .await
    }

    pub async fn reserve_attempt(
        &self,
        id: &str,
        version: u64,
        claim: Option<&TaskWorkerClaim>,
        exec: Option<&crate::id::ExecId>,
    ) -> StoreResult<FlowSession> {
        let id = id.to_string();
        let claim = claim.cloned();
        let exec = exec.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_attempt(&id, version, claim.as_ref(), exec.as_ref())
        })
        .await
    }

    pub async fn publish_attempt(
        &self,
        id: &str,
        version: u64,
        captured: i64,
        claim: Option<&TaskWorkerClaim>,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let id = id.to_string();
        let claim = claim.cloned();
        let provider = provider.to_string();
        let model = model.map(str::to_owned);
        run_sqlite(&self.sqlite, move |store| {
            store.publish_attempt(
                &id,
                version,
                captured,
                claim.as_ref(),
                &provider,
                model.as_deref(),
            )
        })
        .await
    }

    pub async fn retry_flow(&self, id: &str, direction: Option<&str>) -> StoreResult<FlowSession> {
        let id = id.to_string();
        let direction = direction.map(str::to_owned);
        run_sqlite(&self.sqlite, move |store| {
            store.retry_flow(&id, direction.as_deref())
        })
        .await
    }

    pub async fn release_flow(
        &self,
        id: &str,
        version: u64,
        claim: Option<&TaskWorkerClaim>,
    ) -> StoreResult<FlowSession> {
        let id = id.to_string();
        let claim = claim.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.release_flow(&id, version, claim.as_ref())
        })
        .await
    }

    pub async fn checkpoint_flow(
        &self,
        id: &str,
        version: u64,
        cursor: &ExecutionCursor,
        claim: Option<&TaskWorkerClaim>,
        progress: Option<&str>,
    ) -> StoreResult<FlowSession> {
        let id = id.to_string();
        let cursor = cursor.clone();
        let claim = claim.cloned();
        let progress = progress.map(str::to_owned);
        run_sqlite(&self.sqlite, move |store| {
            store.checkpoint_flow(&id, version, &cursor, claim.as_ref(), progress.as_deref())
        })
        .await
    }

    pub async fn fail_flow(
        &self,
        id: &str,
        version: u64,
        claim: Option<&TaskWorkerClaim>,
        failure: &TaskFlowBlocker,
    ) -> StoreResult<FlowSession> {
        let id = id.to_string();
        let claim = claim.cloned();
        let failure = failure.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.fail_flow(&id, version, claim.as_ref(), &failure)
        })
        .await
    }

    pub async fn end_flow(
        &self,
        id: &str,
        version: u64,
        claim: Option<&TaskWorkerClaim>,
        summary: &str,
    ) -> StoreResult<()> {
        let id = id.to_string();
        let claim = claim.cloned();
        let summary = summary.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.end_flow(&id, version, claim.as_ref(), &summary)
        })
        .await
    }

    pub async fn reserve_task_review(&self, id: &str, version: u64) -> StoreResult<FlowSession> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_task_review(&id, version)
        })
        .await
    }

    pub async fn complete_task_review(
        &self,
        task_id: &TaskId,
        expected: &FlowSession,
        summary: &str,
    ) -> StoreResult<()> {
        let task_id = task_id.clone();
        let expected = expected.clone();
        let summary = summary.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_task_review(&task_id, &expected, &summary)
        })
        .await
    }

    pub async fn claim_task_worker(
        &self,
        task_id: &TaskId,
        expected_invocation: &str,
        expected_version: u64,
        owner: &TaskWorkerOwner,
        claimed_at: OffsetDateTime,
    ) -> StoreResult<TaskWorkerClaimOutcome> {
        let task_id = task_id.clone();
        let expected_invocation = expected_invocation.to_string();
        let owner = owner.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_task_worker(
                &task_id,
                &expected_invocation,
                expected_version,
                &owner,
                claimed_at,
            )
        })
        .await
    }

    pub async fn reclaim_task_worker(
        &self,
        task_id: &TaskId,
        expected: &TaskWorkerClaim,
        owner: &TaskWorkerOwner,
        claimed_at: OffsetDateTime,
    ) -> StoreResult<TaskWorkerClaim> {
        let task_id = task_id.clone();
        let expected = expected.clone();
        let owner = owner.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reclaim_task_worker(&task_id, &expected, &owner, claimed_at)
        })
        .await
    }

    pub async fn flow_blocker_key(
        &self,
        id: &str,
        version: u64,
        actor: &crate::id::ExecId,
    ) -> StoreResult<String> {
        let id = id.to_string();
        let actor = actor.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.flow_blocker_key(&id, version, &actor)
        })
        .await
    }

    pub async fn waiting_review(&self, session_id: &str) -> StoreResult<FlowSession> {
        let session_id = session_id.to_string();
        run_sqlite(&self.sqlite, move |store| store.waiting_review(&session_id)).await
    }
}
