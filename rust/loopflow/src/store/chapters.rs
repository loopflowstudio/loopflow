use crate::durable::TaskId;
use crate::id::WaveId;
use crate::ops::chapter::TaskStartEvidence;
use crate::work::project::ProjectId;

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn projects_pending_adoption(
        &self,
        wave: &WaveId,
    ) -> StoreResult<Vec<(String, i64)>> {
        let wave = wave.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.projects_pending_adoption(&wave)
        })
        .await
    }

    pub async fn finish_project_adoption(&self, wave: &WaveId, project: &str) -> StoreResult<()> {
        let wave = wave.clone();
        let project = project.to_owned();
        run_sqlite(&self.sqlite, move |store| {
            store.finish_project_adoption(&wave, &project)
        })
        .await
    }

    pub async fn move_chapter_task(&self, task: &TaskId, project: &ProjectId) -> StoreResult<()> {
        let task = task.clone();
        let project = project.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.move_chapter_task(&task, &project)
        })
        .await
    }

    pub async fn task_started(&self, task: &TaskId) -> StoreResult<bool> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| store.task_started(&task)).await
    }

    pub async fn chapter_task_evidence(&self, task: &TaskId) -> StoreResult<TaskStartEvidence> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.chapter_task_evidence(&task)
        })
        .await
    }

    pub async fn retire_chapter_backlog(&self, task: &TaskId) -> StoreResult<bool> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.retire_chapter_backlog(&task)
        })
        .await
    }
}
