use loopflow_model::durable::TaskId;

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn task_owned(&self, task: &TaskId) -> StoreResult<bool> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| store.task_owned(&task)).await
    }

    pub async fn task_begun(&self, task: &TaskId) -> StoreResult<bool> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| store.task_begun(&task)).await
    }
}
