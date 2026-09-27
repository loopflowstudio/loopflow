use crate::durable::{FlowInvocation, RunId};
use crate::engine::transitions::FlowVerdict;
use crate::engine::ExecutionCursor;

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn create_flow(&self, flow: FlowInvocation) -> StoreResult<FlowInvocation> {
        run_sqlite(&self.sqlite, move |store| store.create_flow(&flow)).await
    }

    pub async fn flow(&self, id: &str) -> StoreResult<Option<FlowInvocation>> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.flow(&id)).await
    }

    pub async fn recover_flow(&self, id: &str) -> StoreResult<FlowInvocation> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.recover_flow(&id)).await
    }

    pub async fn retry_flow(&self, id: &str) -> StoreResult<FlowInvocation> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.retry_flow(&id)).await
    }

    pub async fn checkpoint_flow(
        &self,
        id: &str,
        version: u64,
        cursor: &ExecutionCursor,
    ) -> StoreResult<u64> {
        let id = id.to_string();
        let cursor = cursor.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.checkpoint_flow(&id, version, &cursor)
        })
        .await
    }

    pub async fn fail_flow(&self, id: &str, version: u64, reason: &str) -> StoreResult<()> {
        let id = id.to_string();
        let reason = reason.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.fail_flow(&id, version, &reason)
        })
        .await
    }

    pub async fn record_flow_decision(
        &self,
        id: &str,
        version: u64,
        run: &RunId,
        verdict: &FlowVerdict,
    ) -> StoreResult<()> {
        let id = id.to_string();
        let run = run.clone();
        let verdict = verdict.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.record_flow_decision(&id, version, &run, &verdict)
        })
        .await
    }

    pub async fn record_flow_path(
        &self,
        id: &str,
        version: u64,
        run: &RunId,
        path: &str,
    ) -> StoreResult<()> {
        let id = id.to_string();
        let run = run.clone();
        let path = path.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.record_flow_path(&id, version, &run, &path)
        })
        .await
    }

    pub async fn flow_blocker_key(
        &self,
        id: &str,
        version: u64,
        run: &RunId,
    ) -> StoreResult<String> {
        let id = id.to_string();
        let run = run.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.flow_blocker_key(&id, version, &run)
        })
        .await
    }

    pub async fn waiting_review(&self, session_id: &str) -> StoreResult<FlowInvocation> {
        let session_id = session_id.to_string();
        run_sqlite(&self.sqlite, move |store| store.waiting_review(&session_id)).await
    }
}
