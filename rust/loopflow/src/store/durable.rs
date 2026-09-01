use crate::child::ChildRef;
use crate::durable::{
    AbandonReceipt, Author, FlowPosition, Home, HomeId, Placement, Steer, SteerComment,
    ToolResponseReceipt, ToolResponseWrite, WorkRef, WorkStatus,
};

use super::{run_sqlite, Store, StoreResult};

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

    pub async fn set_flow_position(
        &self,
        work: &WorkRef,
        position: FlowPosition,
    ) -> StoreResult<FlowPosition> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.set_flow_position(&work, &position)
        })
        .await
    }

    pub async fn flow_position(&self, work: &WorkRef) -> StoreResult<Option<FlowPosition>> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.flow_position(&work)).await
    }

    pub async fn human_flow_positions(&self) -> StoreResult<Vec<FlowPosition>> {
        run_sqlite(&self.sqlite, |store| store.human_flow_positions()).await
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

    pub async fn steers_since(&self, since: i64) -> StoreResult<Vec<SteerComment>> {
        run_sqlite(&self.sqlite, move |store| store.steers_since(since)).await
    }

    pub async fn append_interrupt(&self, work: &WorkRef) -> StoreResult<i64> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.append_interrupt(&work)).await
    }

    pub async fn latest_interrupt_id(&self, work: &WorkRef) -> StoreResult<i64> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.latest_interrupt_id(&work)).await
    }

    pub(crate) async fn work_steers_for_child(&self, target: &ChildRef) -> StoreResult<Vec<Steer>> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.work_steers_for_child(&target)
        })
        .await
    }

    pub(crate) async fn append_steer(
        &self,
        work: &WorkRef,
        author: Author,
        text: &str,
    ) -> StoreResult<Steer> {
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
    use crate::durable::WorkRef;
    use crate::id::WaveId;
    use crate::store::{StorageConfig, StoreError};
    use crate::work::wave::Wave;

    async fn wave_work() -> (super::Store, WorkRef) {
        let directory = tempfile::tempdir().unwrap().keep();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            directory.join("registry.db"),
        ))
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
