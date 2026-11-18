use super::{run_sqlite, Store, StoreResult};
use crate::durable::TaskExecutionRoute;
use loopflow_model::durable::{RepositoryId, TaskId};

impl Store {
    pub async fn task_execution_route(&self, task: &TaskId) -> StoreResult<TaskExecutionRoute> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| {
            crate::store::ownership::task_execution_route(&store, &task)
        })
        .await
    }

    pub async fn ensure_repository(&self, repo: &str) -> StoreResult<RepositoryId> {
        let repo = repo.to_string();
        run_sqlite(&self.sqlite, move |store| {
            crate::repository::location::ensure_repository(&store, &repo)
        })
        .await
    }

    pub async fn repository_id(&self, repo: &str) -> StoreResult<Option<RepositoryId>> {
        let repo = repo.to_string();
        run_sqlite(&self.sqlite, move |store| store.repository_id(&repo)).await
    }
}
