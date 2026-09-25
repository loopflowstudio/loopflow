//! Durable Project and Task compatibility rows and their observation outbox.

use crate::child::ObservationRecipient;
use crate::durable::{FlowPosition, TaskFlowBlocker, TaskWorkerClaim};
use crate::id::WaveId;
use crate::work::project::{
    ObservationOutboxRow, Project, ProjectEvent, ProjectEventKind, ProjectId,
};
use crate::work::task::{
    LinearObservationApply, LinearObservationOutcome, Task, TaskEvent, TaskEventKind, TaskId,
    TaskLinearObservation, TaskPr, TaskPrId,
};
use time::OffsetDateTime;

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn create_task(&self, task: &Task, pr: &TaskPr) -> StoreResult<()> {
        let task = task.clone();
        let pr = pr.clone();
        run_sqlite(&self.sqlite, move |store| store.insert_task(&task, &pr)).await
    }

    pub async fn create_task_with_worktree(&self, task: &Task, pr: &TaskPr) -> StoreResult<()> {
        let task = task.clone();
        let pr = pr.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_task_with_worktree(&task, &pr)
        })
        .await
    }

    pub async fn reopen_task(&self, task: &Task, pr: Option<&TaskPr>) -> StoreResult<()> {
        let task = task.clone();
        let pr = pr.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.reopen_task(&task, pr.as_ref())
        })
        .await
    }

    pub async fn update_task(&self, task: &Task) -> StoreResult<()> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| store.update_task(&task)).await
    }

    pub async fn settle_task_worker(
        &self,
        task: &Task,
        expected: &TaskWorkerClaim,
        next: &FlowPosition,
        progress: Option<&str>,
    ) -> StoreResult<FlowPosition> {
        let task = task.clone();
        let expected = expected.clone();
        let next = next.clone();
        let progress = progress.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.settle_task_worker(&task, &expected, &next, progress.as_deref())
        })
        .await
    }

    pub async fn finish_task_flow(
        &self,
        task: &Task,
        expected: &TaskWorkerClaim,
        progress: Option<&str>,
    ) -> StoreResult<()> {
        let task = task.clone();
        let expected = expected.clone();
        let progress = progress.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.finish_task_flow(&task, &expected, progress.as_deref())
        })
        .await
    }

    pub async fn block_task_flow(
        &self,
        task_id: &TaskId,
        expected: &TaskWorkerClaim,
        failure: &TaskFlowBlocker,
    ) -> StoreResult<FlowPosition> {
        let task_id = task_id.clone();
        let expected = expected.clone();
        let failure = failure.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.block_task_flow(&task_id, &expected, &failure)
        })
        .await
    }

    pub async fn release_task_worker(
        &self,
        task_id: &TaskId,
        expected: &TaskWorkerClaim,
    ) -> StoreResult<FlowPosition> {
        let task_id = task_id.clone();
        let expected = expected.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.release_task_worker(&task_id, &expected)
        })
        .await
    }

    pub async fn approve_human_task_boundary(
        &self,
        task: &Task,
        expected: &FlowPosition,
        next: &FlowPosition,
        summary: &str,
    ) -> StoreResult<FlowPosition> {
        let task = task.clone();
        let expected = expected.clone();
        let next = next.clone();
        let summary = summary.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.approve_human_task_boundary(&task, &expected, &next, &summary)
        })
        .await
    }

    pub async fn finish_human_task_boundary(
        &self,
        task: &Task,
        expected: &FlowPosition,
        summary: &str,
    ) -> StoreResult<()> {
        let task = task.clone();
        let expected = expected.clone();
        let summary = summary.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.finish_human_task_boundary(&task, &expected, &summary)
        })
        .await
    }

    pub async fn iterate_human_task_boundary(
        &self,
        task: &Task,
        expected: &FlowPosition,
        next: &FlowPosition,
    ) -> StoreResult<FlowPosition> {
        let task = task.clone();
        let expected = expected.clone();
        let next = next.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.iterate_human_task_boundary(&task, &expected, &next)
        })
        .await
    }

    pub async fn retry_task_flow(
        &self,
        task_id: &TaskId,
        expected: &FlowPosition,
    ) -> StoreResult<FlowPosition> {
        let task_id = task_id.clone();
        let expected = expected.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.retry_task_flow(&task_id, &expected)
        })
        .await
    }

    pub(crate) async fn restart_task_flow(
        &self,
        task: &Task,
        checkpoint_head: &str,
    ) -> StoreResult<()> {
        let task = task.clone();
        let checkpoint_head = checkpoint_head.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.restart_task_flow(&task, &checkpoint_head)
        })
        .await
    }

    pub async fn rebind_task_issue_identifier(
        &self,
        issue_id: &str,
        old_identifier: &str,
        new_identifier: &str,
    ) -> StoreResult<bool> {
        let issue_id = issue_id.to_string();
        let old_identifier = old_identifier.to_string();
        let new_identifier = new_identifier.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.rebind_task_issue_identifier(&issue_id, &old_identifier, &new_identifier)
        })
        .await
    }

    pub async fn complete_task(&self, task: &Task, skipped_pr: Option<&TaskPr>) -> StoreResult<()> {
        let task = task.clone();
        let skipped_pr = skipped_pr.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_task(&task, skipped_pr.as_ref())
        })
        .await
    }

    pub async fn get_task(&self, task_id: &TaskId) -> StoreResult<Option<Task>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| store.task(&task_id)).await
    }

    pub async fn get_task_by_issue(&self, issue: &str) -> StoreResult<Option<Task>> {
        let issue = issue.to_string();
        run_sqlite(&self.sqlite, move |store| store.task_by_issue(&issue)).await
    }

    pub async fn get_task_by_branch(&self, branch: &str) -> StoreResult<Option<Task>> {
        let branch = branch.to_string();
        run_sqlite(&self.sqlite, move |store| store.task_by_branch(&branch)).await
    }

    pub async fn list_tasks(&self, wave_id: Option<&WaveId>) -> StoreResult<Vec<Task>> {
        let wave_id = wave_id.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.list_tasks(wave_id.as_ref())
        })
        .await
    }

    pub async fn update_task_pr(&self, pr: &TaskPr) -> StoreResult<()> {
        let pr = pr.clone();
        run_sqlite(&self.sqlite, move |store| store.update_task_pr(&pr)).await
    }

    pub(crate) async fn record_task_pr_repair_incident(
        &self,
        pr_id: &TaskPrId,
        kind: crate::work::task::TaskPrRepairKind,
        occurred_at: OffsetDateTime,
    ) -> StoreResult<bool> {
        let pr_id = pr_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.record_task_pr_repair_incident(&pr_id, kind, occurred_at)
        })
        .await
    }

    pub async fn heal_task_pr_base(&self, pr: &TaskPr) -> StoreResult<()> {
        let pr = pr.clone();
        run_sqlite(&self.sqlite, move |store| store.heal_task_pr_base(&pr)).await
    }

    pub async fn task_prs(&self, task_id: &TaskId) -> StoreResult<Vec<TaskPr>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| store.task_prs(&task_id)).await
    }

    pub async fn latest_task_event_at(
        &self,
        task_id: &TaskId,
    ) -> StoreResult<Option<OffsetDateTime>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.latest_task_event_at(&task_id)
        })
        .await
    }

    pub async fn recent_task_events(
        &self,
        task_id: &TaskId,
        limit: u32,
    ) -> StoreResult<Vec<TaskEvent>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.recent_task_events(&task_id, limit)
        })
        .await
    }

    pub async fn latest_task_event(&self, task_id: &TaskId) -> StoreResult<Option<TaskEvent>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| store.latest_task_event(&task_id)).await
    }

    pub async fn latest_project_event_at(
        &self,
        project_id: &ProjectId,
    ) -> StoreResult<Option<OffsetDateTime>> {
        let project_id = project_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.latest_project_event_at(&project_id)
        })
        .await
    }

    pub async fn latest_project_event(
        &self,
        project_id: &ProjectId,
    ) -> StoreResult<Option<ProjectEvent>> {
        let project_id = project_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.latest_project_event(&project_id)
        })
        .await
    }

    pub async fn latest_project_failure(
        &self,
        project_id: &ProjectId,
    ) -> StoreResult<Option<crate::work::project::HistoricalFailure>> {
        let project_id = project_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.latest_project_failure(&project_id)
        })
        .await
    }

    pub async fn get_task_pr(&self, pr_id: &TaskPrId) -> StoreResult<Option<TaskPr>> {
        let pr_id = pr_id.clone();
        run_sqlite(&self.sqlite, move |store| store.task_pr(&pr_id)).await
    }

    pub async fn active_task_pr(&self, task_id: &TaskId) -> StoreResult<Option<TaskPr>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| store.active_task_pr(&task_id)).await
    }

    pub async fn rebase_task_pr(
        &self,
        pr_id: &TaskPrId,
        new_base: &str,
        clear_parent: bool,
        updated_at: OffsetDateTime,
    ) -> StoreResult<()> {
        let pr_id = pr_id.clone();
        let new_base = new_base.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.rebase_task_pr(&pr_id, &new_base, clear_parent, updated_at)
        })
        .await
    }

    pub async fn settle_task_pr(&self, settled: &TaskPr, next: Option<&TaskPr>) -> StoreResult<()> {
        let settled = settled.clone();
        let next = next.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.settle_task_pr(&settled, next.as_ref())
        })
        .await
    }

    pub(crate) async fn settle_task_pr_merged(
        &self,
        settled: &TaskPr,
        merged_at: Option<OffsetDateTime>,
    ) -> StoreResult<crate::store::TaskPrMergeEvidenceOutcome> {
        let settled = settled.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.settle_task_pr_merged(&settled, merged_at)
        })
        .await
    }

    pub async fn complete_task_after_pr(&self, task: &Task, pr: &TaskPr) -> StoreResult<()> {
        let task = task.clone();
        let pr = pr.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_task_after_pr(&task, &pr)
        })
        .await
    }

    pub async fn task_linear_observation(
        &self,
        task_id: &TaskId,
    ) -> StoreResult<Option<TaskLinearObservation>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.task_linear_observation(&task_id)
        })
        .await
    }

    pub async fn apply_linear_observation(
        &self,
        apply: LinearObservationApply,
    ) -> StoreResult<LinearObservationOutcome> {
        run_sqlite(&self.sqlite, move |store| {
            store.apply_linear_observation(&apply)
        })
        .await
    }

    pub async fn apply_linear_comment(
        &self,
        task_id: &TaskId,
        comment_id: String,
        text: String,
        observed_at: OffsetDateTime,
    ) -> StoreResult<Option<i64>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.apply_linear_comment(&task_id, &comment_id, &text, observed_at)
        })
        .await
    }

    pub async fn mark_task_linear_degraded(
        &self,
        task_id: &TaskId,
        reason: String,
    ) -> StoreResult<()> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_task_linear_degraded(&task_id, &reason)
        })
        .await
    }

    pub async fn append_task_event(
        &self,
        task_id: &TaskId,
        kind: &TaskEventKind,
    ) -> StoreResult<TaskEvent> {
        let task_id = task_id.clone();
        let kind = kind.clone();
        let write_task_id = task_id.clone();
        let write_kind = kind.clone();
        let event = run_sqlite(&self.sqlite, move |store| {
            store.append_task_event(&write_task_id, &write_kind)
        })
        .await?;
        self.nudge_task_event(&task_id, &kind, &event).await?;
        Ok(event)
    }

    async fn nudge_task_event(
        &self,
        task_id: &TaskId,
        kind: &TaskEventKind,
        event: &TaskEvent,
    ) -> StoreResult<()> {
        if kind.is_wave_observable() {
            if let Some(task) = self.get_task(task_id).await? {
                if let Some(wave) = self.get_wave(&task.wave_id).await? {
                    if let Err(error) =
                        crate::lf::commands::chat::nudge_child_observations(wave.name()).await
                    {
                        tracing::debug!(%error, %task_id, event_id = event.id,
                            "Task delivery failed; the Wave observer will retry its durable outbox");
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn task_events_after(
        &self,
        task_id: &TaskId,
        cursor: i64,
    ) -> StoreResult<Vec<TaskEvent>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.task_events_after(&task_id, cursor)
        })
        .await
    }

    pub async fn get_task_event(
        &self,
        task_id: &TaskId,
        event_id: i64,
    ) -> StoreResult<Option<TaskEvent>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.task_event(&task_id, event_id)
        })
        .await
    }

    pub async fn create_project(&self, project: &Project) -> StoreResult<()> {
        let project = project.clone();
        run_sqlite(&self.sqlite, move |store| store.insert_project(&project)).await
    }

    pub async fn reopen_project(&self, project: &Project) -> StoreResult<()> {
        let project = project.clone();
        run_sqlite(&self.sqlite, move |store| store.reopen_project(&project)).await
    }

    pub async fn update_project(&self, project: &Project) -> StoreResult<()> {
        let project = project.clone();
        run_sqlite(&self.sqlite, move |store| store.update_project(&project)).await
    }

    pub async fn get_project(&self, project_id: &ProjectId) -> StoreResult<Option<Project>> {
        let project_id = project_id.clone();
        run_sqlite(&self.sqlite, move |store| store.project(&project_id)).await
    }

    pub async fn get_project_by_project(&self, project: &str) -> StoreResult<Option<Project>> {
        let project = project.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.project_by_project(&project)
        })
        .await
    }

    pub async fn list_projects(&self, wave_id: Option<&WaveId>) -> StoreResult<Vec<Project>> {
        let wave_id = wave_id.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.list_projects(wave_id.as_ref())
        })
        .await
    }

    pub async fn append_project_event(
        &self,
        project_id: &ProjectId,
        kind: &ProjectEventKind,
    ) -> StoreResult<ProjectEvent> {
        let project_id = project_id.clone();
        let kind = kind.clone();
        let write_project_id = project_id.clone();
        let write_kind = kind.clone();
        let event = run_sqlite(&self.sqlite, move |store| {
            store.append_project_event(&write_project_id, &write_kind)
        })
        .await?;
        if kind.is_wave_observable() {
            if let Some(project) = self.get_project(&project_id).await? {
                match self.get_wave(&project.wave_id).await? {
                    Some(wave) => {
                        if let Err(error) =
                            crate::lf::commands::chat::nudge_child_observations(wave.name()).await
                        {
                            tracing::debug!(
                                %error,
                                %project_id,
                                event_id = event.id,
                                "live Project observation delivery failed; Wave observer will retry"
                            );
                        }
                    }
                    None => tracing::error!(
                        wave_id = %project.wave_id,
                        %project_id,
                        event_id = event.id,
                        "Project observation cannot nudge its missing owning Wave"
                    ),
                }
            }
        }
        Ok(event)
    }

    pub async fn project_events_after(
        &self,
        project_id: &ProjectId,
        cursor: i64,
    ) -> StoreResult<Vec<ProjectEvent>> {
        let project_id = project_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.project_events_after(&project_id, cursor)
        })
        .await
    }

    pub async fn pending_observations(
        &self,
        recipient: &ObservationRecipient,
    ) -> StoreResult<Vec<ObservationOutboxRow>> {
        let recipient = recipient.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.pending_observations(&recipient)
        })
        .await
    }

    pub async fn pending_project_observations(
        &self,
        project_id: &ProjectId,
    ) -> StoreResult<Vec<ObservationOutboxRow>> {
        let project_id = project_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.pending_project_observations(&project_id)
        })
        .await
    }

    pub async fn mark_observation_delivered(&self, id: i64) -> StoreResult<()> {
        run_sqlite(&self.sqlite, move |store| {
            store.mark_observation_delivered(id)
        })
        .await
    }
}
