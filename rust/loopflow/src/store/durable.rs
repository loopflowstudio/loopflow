use crate::child::ChildRef;
use crate::durable::{
    AbandonReceipt, Machine, MachineId, Placement, Steer, SteerComment, TaskId,
    ToolResponseReceipt, ToolResponseWrite, WorkRef, WorkStatus,
};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn begin_task_abandon(&self, task_id: &TaskId) -> StoreResult<()> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.begin_task_abandon(&task_id)
        })
        .await
    }

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

    pub async fn machine_by_id(&self, machine_id: &MachineId) -> StoreResult<Option<Machine>> {
        let machine_id = machine_id.clone();
        run_sqlite(&self.sqlite, move |store| store.machine_by_id(&machine_id)).await
    }

    pub async fn local_machine(&self) -> StoreResult<Machine> {
        run_sqlite(&self.sqlite, move |store| store.local_machine()).await
    }

    pub async fn add_machine(
        &self,
        machine_id: &MachineId,
        target: &str,
        label: &str,
        repo: &str,
    ) -> StoreResult<Machine> {
        let machine_id = machine_id.clone();
        let (target, label, repo) = (target.to_string(), label.to_string(), repo.to_string());
        run_sqlite(&self.sqlite, move |store| {
            store.add_machine(&machine_id, &target, &label, &repo)
        })
        .await
    }

    pub async fn machines(&self) -> StoreResult<Vec<Machine>> {
        run_sqlite(&self.sqlite, move |store| store.machines()).await
    }

    pub async fn rename_machine(&self, label: &str, name: &str) -> StoreResult<()> {
        let (label, name) = (label.to_string(), name.to_string());
        run_sqlite(&self.sqlite, move |store| {
            store.rename_machine(&label, &name)
        })
        .await
    }

    pub async fn remove_machine(&self, label: &str) -> StoreResult<()> {
        let label = label.to_string();
        run_sqlite(&self.sqlite, move |store| store.remove_machine(&label)).await
    }

    pub async fn placement(&self, work: &WorkRef) -> StoreResult<Placement> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.placement(&work)).await
    }

    pub(crate) async fn place_work(
        &self,
        work: &WorkRef,
        machine_id: &MachineId,
    ) -> StoreResult<Placement> {
        let work = work.clone();
        let machine_id = machine_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.place_work(&work, &machine_id)
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

    pub async fn task_state(&self, task: &TaskId) -> StoreResult<crate::durable::TaskState> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| store.task_state(&task)).await
    }

    pub async fn work_for_child(&self, target: &ChildRef) -> StoreResult<WorkRef> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| store.work_for_child(&target)).await
    }

    pub async fn task_steers(&self, task_id: &TaskId) -> StoreResult<Vec<Steer>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| store.task_steers(&task_id)).await
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

    #[cfg(test)]
    pub(crate) async fn append_steer(
        &self,
        work: &WorkRef,
        author: crate::durable::Author,
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
    async fn removing_connection_preserves_placement_and_identity() {
        let (store, work) = wave_work().await;
        let id = crate::durable::MachineId::new();
        let added = store
            .add_machine(&id, "mini", "builder", "/projects/repo")
            .await
            .unwrap();
        store.place_work(&work, &id).await.unwrap();
        assert!(store
            .add_machine(&crate::durable::MachineId::new(), "other", "builder", ".")
            .await
            .is_err());
        store.rename_machine("builder", "mini").await.unwrap();
        assert_eq!(store.machines().await.unwrap()[0].id, id);
        store.remove_machine("mini").await.unwrap();
        assert!(store.machines().await.unwrap().is_empty());
        assert_eq!(store.placement(&work).await.unwrap().machine_id, id);
        let retained = store.machine_by_id(&id).await.unwrap().unwrap();
        assert_eq!(retained.created_at, added.created_at);
        assert_eq!(retained.route, "mini");
        assert!(retained.label.is_none());
        assert!(retained.repo.is_none());
    }

    #[tokio::test]
    async fn placement_preserves_the_selected_home() {
        let (store, work) = wave_work().await;
        let local = store.local_machine().await.unwrap();

        let remote = store
            .add_machine(
                &crate::durable::MachineId::new(),
                "ssh://jack@buildbox",
                "ssh://jack@buildbox",
                ".",
            )
            .await
            .unwrap();
        assert_eq!(
            store
                .place_work(&work, &remote.id)
                .await
                .unwrap()
                .machine_id,
            remote.id
        );
        assert_eq!(
            store.place_work(&work, &local.id).await.unwrap().machine_id,
            local.id
        );
    }

    #[tokio::test]
    async fn local_machine_cannot_be_added_as_remote() {
        let (store, _) = wave_work().await;
        let local = store.local_machine().await.unwrap();

        assert!(matches!(
            store.add_machine(&local.id, "ssh://jack@elsewhere", "ssh://jack@elsewhere", ".").await,
            Err(StoreError::InvalidData(message)) if message.contains("cannot add the local machine")
        ));
        assert_eq!(store.local_machine().await.unwrap(), local);
    }
}
