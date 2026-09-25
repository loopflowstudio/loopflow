use crate::durable::TaskId;
use crate::id::WaveId;
use crate::work::chapter::{Chapter, ChapterId, TaskStartEvidence};
use crate::work::project::ProjectId;

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn chapter(
        &self,
        wave: &WaveId,
        id: Option<&ChapterId>,
    ) -> StoreResult<Option<Chapter>> {
        let wave = wave.clone();
        let id = id.cloned();
        run_sqlite(&self.sqlite, move |store| store.chapter(&wave, id.as_ref())).await
    }

    pub async fn chapters(&self, wave: &WaveId) -> StoreResult<Vec<Chapter>> {
        let wave = wave.clone();
        run_sqlite(&self.sqlite, move |store| store.chapters(&wave)).await
    }

    pub async fn save_chapter(&self, chapter: &Chapter, activate: bool) -> StoreResult<()> {
        let chapter = chapter.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.save_chapter(&chapter, activate)
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
