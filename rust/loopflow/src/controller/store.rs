use crate::child::ChildBodyHandoffRequest;
use crate::durable::Author;
use crate::store::{run_sqlite, Store, StoreResult};
use crate::work::project::ProjectId;
use crate::work::task::TaskId;

use super::{project, task};

impl Store {
    pub(crate) async fn task_controller_state(
        &self,
        task_id: &TaskId,
    ) -> StoreResult<Option<task::State>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.task_controller_state(&task_id)
        })
        .await
    }

    pub(crate) async fn put_task_controller_state(&self, state: &task::State) -> StoreResult<()> {
        let state = state.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.put_task_controller_state(&state)
        })
        .await
    }

    pub(crate) async fn project_controller_state(
        &self,
        project_id: &ProjectId,
    ) -> StoreResult<Option<project::State>> {
        let project_id = project_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.project_controller_state(&project_id)
        })
        .await
    }

    pub(crate) async fn put_project_controller_state(
        &self,
        state: &project::State,
    ) -> StoreResult<()> {
        let state = state.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.put_project_controller_state(&state)
        })
        .await
    }

    pub(crate) async fn handoff_task_controller(
        &self,
        task_id: &TaskId,
        request: &ChildBodyHandoffRequest,
    ) -> StoreResult<task::State> {
        let task_id = task_id.clone();
        let request = request.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.handoff_task_controller(&task_id, &request)
        })
        .await
    }

    pub(crate) async fn restart_task_controller(
        &self,
        state: &task::State,
        author: &Author,
        direction: &str,
        checkpoint_head: &str,
    ) -> StoreResult<()> {
        let state = state.clone();
        let author = author.clone();
        let direction = direction.to_string();
        let checkpoint_head = checkpoint_head.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.restart_task_controller(&state, &author, &direction, &checkpoint_head)
        })
        .await
    }

    pub(crate) async fn handoff_project_controller(
        &self,
        project_id: &ProjectId,
        request: &ChildBodyHandoffRequest,
    ) -> StoreResult<project::State> {
        let project_id = project_id.clone();
        let request = request.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.handoff_project_controller(&project_id, &request)
        })
        .await
    }
}
