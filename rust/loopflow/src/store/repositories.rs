use super::{run_sqlite, Store, StoreResult};
use crate::durable::{RepositoryId, TaskExecutionRoute, TaskId};

impl Store {
    pub async fn task_execution_route(&self, task: &TaskId) -> StoreResult<TaskExecutionRoute> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| store.task_execution_route(&task)).await
    }

    pub async fn ensure_repository(&self, repo: &str) -> StoreResult<RepositoryId> {
        let repo = repo.to_string();
        run_sqlite(&self.sqlite, move |store| store.ensure_repository(&repo)).await
    }

    pub async fn repository_id(&self, repo: &str) -> StoreResult<Option<RepositoryId>> {
        let repo = repo.to_string();
        run_sqlite(&self.sqlite, move |store| store.repository_id(&repo)).await
    }
}
