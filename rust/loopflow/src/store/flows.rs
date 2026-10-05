use crate::durable::{FlowSession, TaskFlowBlocker, TaskId};
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

    pub async fn create_flow(&self, flow: FlowSession) -> StoreResult<FlowSession> {
        run_sqlite(&self.sqlite, move |store| store.create_flow(&flow)).await
    }

    pub async fn flow(&self, id: &str) -> StoreResult<Option<FlowSession>> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.flow(&id)).await
    }

    pub async fn latest_task_flow(&self, task_id: &TaskId) -> StoreResult<Option<FlowSession>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| store.latest_task_flow(&task_id)).await
    }

    pub async fn settle_flow_step(&self, id: &str) -> StoreResult<FlowSession> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.settle_flow_step(&id)).await
    }

    pub async fn reserve_attempt(
        &self,
        id: &str,
        version: u64,
        exec: Option<&crate::id::ExecId>,
    ) -> StoreResult<FlowSession> {
        let id = id.to_string();
        let exec = exec.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_attempt(&id, version, exec.as_ref())
        })
        .await
    }

    pub async fn publish_attempt(
        &self,
        id: &str,
        version: u64,
        captured: i64,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let id = id.to_string();
        let provider = provider.to_string();
        let model = model.map(str::to_owned);
        run_sqlite(&self.sqlite, move |store| {
            store.publish_attempt(&id, version, captured, &provider, model.as_deref())
        })
        .await
    }

    pub async fn record_flow_cursor(
        &self,
        id: &str,
        version: u64,
        cursor: &ExecutionCursor,
        progress: Option<&str>,
    ) -> StoreResult<FlowSession> {
        let id = id.to_string();
        let cursor = cursor.clone();
        let progress = progress.map(str::to_owned);
        run_sqlite(&self.sqlite, move |store| {
            store.record_flow_cursor(&id, version, &cursor, progress.as_deref())
        })
        .await
    }

    pub async fn fail_flow(
        &self,
        id: &str,
        version: u64,
        failure: &TaskFlowBlocker,
    ) -> StoreResult<FlowSession> {
        let id = id.to_string();
        let failure = failure.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.fail_flow(&id, version, &failure)
        })
        .await
    }

    pub async fn end_flow(&self, id: &str, version: u64, summary: &str) -> StoreResult<()> {
        let id = id.to_string();
        let summary = summary.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.end_flow(&id, version, &summary)
        })
        .await
    }
}
