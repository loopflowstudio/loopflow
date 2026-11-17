//! Compose saved planning evidence with observed repository configuration.
use crate::store::{Store, StoreResult};
use loopflow_model::durable::{ProjectId, TaskId};
use loopflow_storage::planning_sync::TrackerSync;

impl Store {
    pub fn task_comments(
        &self,
        task: &TaskId,
    ) -> StoreResult<loopflow_model::comment::TaskComments> {
        let saved = self.sqlite.task_comment_thread(task)?;
        let mut thread = saved.thread;
        if !crate::ops::tracker::connected(&saved.repo) {
            thread.pending_sync.clear();
        }
        Ok(thread)
    }

    pub fn task_tracker_sync(&self, task: &TaskId) -> StoreResult<TrackerSync> {
        let receipts = self.sqlite.task_tracker_receipts(task)?;
        let connected = crate::ops::tracker::connected(&receipts.repo);
        Ok(receipts.status(connected))
    }

    pub fn project_tracker_sync(&self, project: &ProjectId) -> StoreResult<TrackerSync> {
        let receipts = self.sqlite.project_tracker_receipts(project)?;
        let connected = crate::ops::tracker::connected(&receipts.repo);
        Ok(receipts.status(connected))
    }
}
