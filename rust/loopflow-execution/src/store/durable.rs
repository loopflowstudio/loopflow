use crate::durable::AbandonReceipt;
use crate::steer::Steer;
use loopflow_model::durable::{Machine, MachineId, TaskId, WorkRef, WorkStatus};
use loopflow_storage::child::ChildRef;

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

    pub async fn machine_by_id(&self, machine_id: &MachineId) -> StoreResult<Option<Machine>> {
        let machine_id = machine_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            crate::store::machines::machine_by_id(&store, &machine_id)
        })
        .await
    }

    pub async fn local_machine(&self) -> StoreResult<Machine> {
        run_sqlite(&self.sqlite, move |store| {
            crate::store::machines::local_machine(&store)
        })
        .await
    }

    pub async fn add_machine(
        &self,
        machine_id: &MachineId,
        target: &str,
        label: &str,
    ) -> StoreResult<Machine> {
        let machine_id = machine_id.clone();
        let (target, label) = (target.to_string(), label.to_string());
        run_sqlite(&self.sqlite, move |store| {
            crate::store::machines::add_machine(&store, &machine_id, &target, &label)
        })
        .await
    }

    pub async fn machines(&self) -> StoreResult<Vec<Machine>> {
        run_sqlite(&self.sqlite, move |store| {
            crate::store::machines::machines(&store)
        })
        .await
    }

    pub async fn rename_machine(&self, label: &str, name: &str) -> StoreResult<()> {
        let (label, name) = (label.to_string(), name.to_string());
        run_sqlite(&self.sqlite, move |store| {
            crate::store::machines::rename_machine(&store, &label, &name)
        })
        .await
    }

    pub async fn remove_machine(&self, label: &str) -> StoreResult<()> {
        let label = label.to_string();
        run_sqlite(&self.sqlite, move |store| {
            crate::store::machines::remove_machine(&store, &label)
        })
        .await
    }

    pub async fn wave_owning_machine(
        &self,
        wave: &loopflow_model::id::WaveId,
        setting: loopflow_model::durable::MachineSetting,
    ) -> StoreResult<(MachineId, crate::durable::OwningMachineSource)> {
        let wave = wave.clone();
        run_sqlite(&self.sqlite, move |store| {
            crate::store::placement::wave_owning_machine(&store, &wave, setting)
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

    pub async fn append_interrupt(&self, work: &WorkRef) -> StoreResult<i64> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.append_interrupt(&work)).await
    }

    pub async fn latest_interrupt_id(&self, work: &WorkRef) -> StoreResult<i64> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.latest_interrupt_id(&work)).await
    }

    #[cfg(any(test, feature = "test-support"))]
    pub async fn save_steer(
        &self,
        work: &WorkRef,
        author: loopflow_model::durable::Author,
        text: &str,
    ) -> StoreResult<Steer> {
        let work = work.clone();
        let text = text.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.save_steer(&work, &author, &text)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use crate::store::{StorageConfig, StoreError};
    use crate::work::wave::Wave;
    use loopflow_model::durable::WorkRef;
    use loopflow_model::id::WaveId;

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
    async fn removing_connection_preserves_identity() {
        let (store, _) = wave_work().await;
        let id = loopflow_model::durable::MachineId::new();
        let added = store.add_machine(&id, "mini", "builder").await.unwrap();
        assert!(store
            .add_machine(
                &loopflow_model::durable::MachineId::new(),
                "other",
                "builder"
            )
            .await
            .is_err());
        store.rename_machine("builder", "mini").await.unwrap();
        assert_eq!(store.machines().await.unwrap()[0].id, id);
        store.remove_machine("mini").await.unwrap();
        assert!(store.machines().await.unwrap().is_empty());
        let retained = store.machine_by_id(&id).await.unwrap().unwrap();
        assert_eq!(retained.created_at, added.created_at);
        assert_eq!(retained.route, "mini");
        assert!(retained.label.is_none());
    }

    #[tokio::test]
    async fn local_machine_cannot_be_added_as_remote() {
        let (store, _) = wave_work().await;
        let local = store.local_machine().await.unwrap();

        assert!(matches!(
            store.add_machine(&local.id, "ssh://jack@elsewhere", "ssh://jack@elsewhere").await,
            Err(StoreError::InvalidData(message)) if message.contains("cannot add the local machine")
        ));
        assert_eq!(store.local_machine().await.unwrap(), local);
    }
}
