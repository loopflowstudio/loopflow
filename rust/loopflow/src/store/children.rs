//! Durable Project and Task compatibility rows and their observation outbox.

use crate::child::{ChildBodyHandoffRequest, ObservationRecipient};
use crate::durable::{Author, Basis, Run, RunLease};
use crate::id::WaveId;
use crate::project::{ObservationOutboxRow, Project, ProjectEvent, ProjectEventKind, ProjectId};
use crate::task::{
    LinearObservationApply, LinearObservationOutcome, Task, TaskEvent, TaskEventKind, TaskId,
    TaskLinearObservation, TaskPr, TaskPrId,
};
use time::OffsetDateTime;

use super::{run_sqlite, Store, StoreError, StoreResult};

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct TestRunReservation {
    pub(crate) run_token: crate::durable::RunLeaseToken,
    lease: RunLease,
}

#[cfg(test)]
impl std::ops::Deref for TestRunReservation {
    type Target = RunLease;

    fn deref(&self) -> &Self::Target {
        &self.lease
    }
}

impl Store {
    pub async fn create_task(&self, task: &Task, pr: &TaskPr) -> StoreResult<()> {
        let task = task.clone();
        let pr = pr.clone();
        run_sqlite(&self.sqlite, move |store| store.insert_task(&task, &pr)).await
    }

    pub async fn create_task_run(
        &self,
        context: &crate::durable::ControlCtx<'_>,
        task: &Task,
        pr: &TaskPr,
        text: &str,
    ) -> StoreResult<(Run, RunLease)> {
        let _promotion_lock = crate::promotion_lock::acquire_shared()
            .await
            .map_err(|error| {
                StoreError::InvalidData(format!(
                    "acquire shared promotion lock before Task Run reservation: {error}"
                ))
            })?;
        let caller = match context {
            crate::durable::ControlCtx::User(_) => None,
            crate::durable::ControlCtx::Run(lease) => Some((*lease).clone()),
        };
        let task = task.clone();
        let pr = pr.clone();
        let text = text.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_task_run(&task, &pr, caller.as_ref(), &text)
        })
        .await
    }

    pub async fn reopen_task(
        &self,
        context: &crate::durable::ControlCtx<'_>,
        task: &Task,
        pr: Option<&TaskPr>,
        text: &str,
    ) -> StoreResult<()> {
        let caller = match context {
            crate::durable::ControlCtx::User(_) => None,
            crate::durable::ControlCtx::Run(lease) => Some((*lease).clone()),
        };
        let task = task.clone();
        let pr = pr.cloned();
        let text = text.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.reopen_task(&task, pr.as_ref(), caller.as_ref(), &text)
        })
        .await
    }

    pub async fn update_task(&self, task: &Task) -> StoreResult<()> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| store.update_task(&task)).await
    }

    pub(crate) async fn validate_task_run_route(
        &self,
        task: &Task,
        lease: &RunLease,
        current_external_project_id: &str,
    ) -> StoreResult<()> {
        let task = task.clone();
        let lease = lease.clone();
        let current_external_project_id = current_external_project_id.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.validate_task_run_route(&task, &lease, &current_external_project_id)
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

    #[cfg(test)]
    pub(crate) async fn activate_task_process(
        &self,
        task: &Task,
        lease: &RunLease,
    ) -> StoreResult<()> {
        self.update_task_for_run(task, lease).await
    }

    pub(crate) async fn update_task_for_run(
        &self,
        task: &Task,
        lease: &RunLease,
    ) -> StoreResult<()> {
        let task = task.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.update_task_for_run(&task, &lease)
        })
        .await
    }

    #[cfg(test)]
    pub(crate) async fn update_task_for_lease(
        &self,
        task: &Task,
        lease: &RunLease,
    ) -> StoreResult<()> {
        self.update_task_for_run(task, lease).await
    }

    pub(crate) async fn finish_task_run(
        &self,
        task: &Task,
        lease: &RunLease,
        outcome: crate::durable::BoundaryState,
    ) -> StoreResult<()> {
        let task = task.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.finish_task_run(&task, &lease, outcome)
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

    pub(crate) async fn complete_task_for_run(
        &self,
        task: &Task,
        skipped_pr: Option<&TaskPr>,
        lease: &RunLease,
    ) -> StoreResult<()> {
        let task = task.clone();
        let skipped_pr = skipped_pr.cloned();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_task_for_run(&task, skipped_pr.as_ref(), &lease)
        })
        .await
    }

    #[cfg(test)]
    pub(crate) async fn reserve_task_process(
        &self,
        task: &Task,
        _expected_status: crate::durable::WorkStatus,
    ) -> StoreResult<Option<TestRunReservation>> {
        self.update_task(task).await?;
        let work = self
            .work_for_child(&crate::child::ChildRef::Task(task.id.clone()))
            .await?;
        let (_run, lease) = match self
            .reserve_run(&work, crate::durable::RunTrigger::User)
            .await
        {
            Ok(reserved) => reserved,
            Err(super::StoreError::RunFenced { .. }) => return Ok(None),
            Err(error) => return Err(error),
        };
        self.advance_run(
            &lease,
            crate::durable::RunAdvance::RunStarting {
                containment: crate::durable::Containment::Tmux {
                    name: format!("test-task-{}", task.id),
                },
                cwd: task.worktree.clone(),
            },
        )
        .await?;
        self.advance_run(
            &lease,
            crate::durable::RunAdvance::InvocationStarting {
                route: crate::durable::InvocationRoute {
                    provider: task.provider.clone(),
                    model: None,
                    account_id: None,
                },
                surface: "headless".to_string(),
                resume_token: task.provider_session_id.clone(),
                answer_ask_id: None,
            },
        )
        .await?;
        let run_token = crate::durable::RunLeaseToken::parse(lease.env_value())
            .expect("a resolved Run lease has a valid token");
        Ok(Some(TestRunReservation { run_token, lease }))
    }

    pub async fn handoff_task_body(
        &self,
        task_id: &TaskId,
        request: &ChildBodyHandoffRequest,
    ) -> StoreResult<Task> {
        let task_id = task_id.clone();
        let request = request.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.handoff_task_body(&task_id, &request)
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

    pub async fn get_task_by_worktree(&self, worktree: &str) -> StoreResult<Option<Task>> {
        let worktree = worktree.to_string();
        run_sqlite(&self.sqlite, move |store| store.task_by_worktree(&worktree)).await
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

    pub(crate) async fn update_task_pr_for_run(
        &self,
        pr: &TaskPr,
        lease: &RunLease,
    ) -> StoreResult<()> {
        let pr = pr.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.update_task_pr_for_run(&pr, &lease)
        })
        .await
    }

    pub(crate) async fn record_task_pr_repair_incident(
        &self,
        pr_id: &TaskPrId,
        kind: crate::task::TaskPrRepairKind,
        occurred_at: OffsetDateTime,
    ) -> StoreResult<bool> {
        let pr_id = pr_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.record_task_pr_repair_incident(&pr_id, kind, occurred_at)
        })
        .await
    }

    pub(crate) async fn record_task_pr_repair_incident_for_run(
        &self,
        pr_id: &TaskPrId,
        kind: crate::task::TaskPrRepairKind,
        occurred_at: OffsetDateTime,
        lease: &RunLease,
    ) -> StoreResult<bool> {
        let pr_id = pr_id.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.record_task_pr_repair_incident_for_run(&pr_id, kind, occurred_at, &lease)
        })
        .await
    }

    pub async fn heal_task_pr_base(&self, pr: &TaskPr) -> StoreResult<()> {
        let pr = pr.clone();
        run_sqlite(&self.sqlite, move |store| store.heal_task_pr_base(&pr)).await
    }

    pub(crate) async fn heal_task_pr_base_for_run(
        &self,
        pr: &TaskPr,
        lease: &RunLease,
    ) -> StoreResult<()> {
        let pr = pr.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.heal_task_pr_base_for_run(&pr, &lease)
        })
        .await
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

    pub(crate) async fn settle_task_pr_for_run(
        &self,
        settled: &TaskPr,
        next: Option<&TaskPr>,
        lease: &RunLease,
    ) -> StoreResult<()> {
        let settled = settled.clone();
        let next = next.cloned();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.settle_task_pr_for_run(&settled, next.as_ref(), &lease)
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

    pub(crate) async fn settle_task_pr_merged_for_run(
        &self,
        settled: &TaskPr,
        merged_at: Option<OffsetDateTime>,
        lease: &RunLease,
    ) -> StoreResult<crate::store::TaskPrMergeEvidenceOutcome> {
        let settled = settled.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.settle_task_pr_merged_for_run(&settled, merged_at, &lease)
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

    pub(crate) async fn complete_task_after_pr_for_run(
        &self,
        task: &Task,
        pr: &TaskPr,
        lease: &RunLease,
    ) -> StoreResult<()> {
        let task = task.clone();
        let pr = pr.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_task_after_pr_for_run(&task, &pr, &lease)
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
    ) -> StoreResult<Option<crate::durable::SteerId>> {
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

    pub(crate) async fn append_task_event_for_run(
        &self,
        task_id: &TaskId,
        lease: &RunLease,
        kind: &TaskEventKind,
    ) -> StoreResult<TaskEvent> {
        let task_id = task_id.clone();
        let lease = lease.clone();
        let kind = kind.clone();
        let write_task_id = task_id.clone();
        let write_kind = kind.clone();
        let event = run_sqlite(&self.sqlite, move |store| {
            store.append_task_event_for_run(&write_task_id, &lease, &write_kind)
        })
        .await?;
        self.nudge_task_event(&task_id, &kind, &event).await?;
        Ok(event)
    }

    pub(crate) async fn fail_task_run(
        &self,
        task_id: &TaskId,
        lease: &RunLease,
        error: &str,
    ) -> StoreResult<TaskEvent> {
        let task_id = task_id.clone();
        let lease = lease.clone();
        let error = error.to_string();
        let write_task_id = task_id.clone();
        let write_error = error.clone();
        let event = run_sqlite(&self.sqlite, move |store| {
            store.fail_task_run(&write_task_id, &lease, &write_error)
        })
        .await?;
        let kind = TaskEventKind::Failed {
            error,
            resumable: true,
        };
        if let Err(error) = self.nudge_task_event(&task_id, &kind, &event).await {
            tracing::debug!(
                %error,
                %task_id,
                event_id = event.id,
                "terminal Task event persisted; live observer nudge will retry"
            );
        }
        Ok(event)
    }

    async fn nudge_task_event(
        &self,
        task_id: &TaskId,
        kind: &TaskEventKind,
        event: &TaskEvent,
    ) -> StoreResult<()> {
        if kind.is_project_observable() {
            if let Some(task) = self.get_task(task_id).await? {
                if let Err(error) = crate::ops::project::wake_task_project_route(self, &task).await
                {
                    tracing::debug!(
                        %error,
                        %task_id,
                        project_id = %task.project_id,
                        event_id = event.id,
                        "Task observation wake failed; Project lifecycle touch will retry"
                    );
                }
                if kind.is_root_wave_observable() {
                    match self.get_wave(&task.wave_id).await? {
                        Some(wave) => {
                            if let Err(error) =
                                crate::lf::commands::chat::nudge_child_observations(wave.name())
                                    .await
                            {
                                tracing::debug!(
                                    %error,
                                    %task_id,
                                    event_id = event.id,
                                    "live Task observation delivery failed; Wave observer will retry"
                                );
                            }
                        }
                        None => tracing::error!(
                            wave_id = %task.wave_id,
                            %task_id,
                            event_id = event.id,
                            "Task observation cannot nudge its missing owning Wave"
                        ),
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

    pub async fn create_project_with_steer(
        &self,
        project: &Project,
        author: Author,
        text: &str,
    ) -> StoreResult<()> {
        let project = project.clone();
        let text = text.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_project_with_steer(&project, &author, &text)
        })
        .await
    }

    pub async fn reopen_project(
        &self,
        project: &Project,
        author: Author,
        text: &str,
    ) -> StoreResult<()> {
        let project = project.clone();
        let text = text.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.reopen_project(&project, &author, &text)
        })
        .await
    }

    pub async fn update_project(&self, project: &Project) -> StoreResult<()> {
        let project = project.clone();
        run_sqlite(&self.sqlite, move |store| store.update_project(&project)).await
    }

    #[cfg(test)]
    pub(crate) async fn activate_project_process(
        &self,
        project: &Project,
        lease: &RunLease,
    ) -> StoreResult<()> {
        self.update_project_for_run(project, lease).await
    }

    pub(crate) async fn update_project_for_run(
        &self,
        project: &Project,
        lease: &RunLease,
    ) -> StoreResult<()> {
        let project = project.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.update_project_for_run(&project, &lease)
        })
        .await
    }

    pub(crate) async fn finish_project_run(
        &self,
        project: &Project,
        lease: &RunLease,
        outcome: crate::durable::BoundaryState,
    ) -> StoreResult<()> {
        let project = project.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.finish_project_run(&project, &lease, outcome)
        })
        .await
    }

    pub(crate) async fn fail_project_run(
        &self,
        project: &Project,
        lease: &RunLease,
        error: &str,
    ) -> StoreResult<ProjectEvent> {
        let project_id = project.id.clone();
        let wave_id = project.wave_id.clone();
        let project = project.clone();
        let lease = lease.clone();
        let error = error.to_string();
        let event = run_sqlite(&self.sqlite, move |store| {
            store.fail_project_run(&project, &lease, &error)
        })
        .await?;
        self.nudge_project_terminal_observation(&project_id, &wave_id, event.id, "failed")
            .await;
        Ok(event)
    }

    pub(crate) async fn complete_project_run(
        &self,
        project: &Project,
        lease: &RunLease,
        basis: &Basis,
        summary: &str,
    ) -> StoreResult<ProjectEvent> {
        let project_id = project.id.clone();
        let wave_id = project.wave_id.clone();
        let project = project.clone();
        let lease = lease.clone();
        let basis = basis.clone();
        let summary = summary.to_string();
        let event = run_sqlite(&self.sqlite, move |store| {
            store.complete_project_run(&project, &lease, &basis, &summary)
        })
        .await?;
        self.nudge_project_terminal_observation(&project_id, &wave_id, event.id, "completed")
            .await;
        Ok(event)
    }

    async fn nudge_project_terminal_observation(
        &self,
        project_id: &ProjectId,
        wave_id: &WaveId,
        event_id: i64,
        outcome: &'static str,
    ) {
        match self.get_wave(wave_id).await {
            Ok(Some(wave)) => {
                if let Err(error) =
                    crate::lf::commands::chat::nudge_child_observations(wave.name()).await
                {
                    tracing::debug!(%error, %project_id, event_id, outcome, "live Project terminal observation delivery failed; Wave observer will retry");
                }
            }
            Ok(None) => {
                tracing::error!(%wave_id, %project_id, event_id, outcome, "Project terminal event cannot nudge its missing owning Wave")
            }
            Err(error) => {
                tracing::debug!(%error, %wave_id, %project_id, event_id, outcome, "Project terminal observation lookup failed; Wave observer will retry")
            }
        }
    }

    #[cfg(test)]
    pub(crate) async fn reserve_project_process(
        &self,
        project: &Project,
        _expected_status: crate::durable::WorkStatus,
    ) -> StoreResult<Option<TestRunReservation>> {
        self.update_project(project).await?;
        let work = self
            .work_for_child(&crate::child::ChildRef::Project(project.id.clone()))
            .await?;
        let (_run, lease) = match self
            .reserve_run(&work, crate::durable::RunTrigger::User)
            .await
        {
            Ok(reserved) => reserved,
            Err(super::StoreError::RunFenced { .. }) => return Ok(None),
            Err(error) => return Err(error),
        };
        self.advance_run(
            &lease,
            crate::durable::RunAdvance::RunStarting {
                containment: crate::durable::Containment::Tmux {
                    name: format!("test-project-{}", project.id),
                },
                cwd: std::path::PathBuf::from("/tmp/project-test"),
            },
        )
        .await?;
        self.advance_run(
            &lease,
            crate::durable::RunAdvance::InvocationStarting {
                route: crate::durable::InvocationRoute {
                    provider: project.provider.clone(),
                    model: None,
                    account_id: None,
                },
                surface: "headless".to_string(),
                resume_token: project.provider_session_id.clone(),
                answer_ask_id: None,
            },
        )
        .await?;
        let run_token = crate::durable::RunLeaseToken::parse(lease.env_value())
            .expect("a resolved Run lease has a valid token");
        Ok(Some(TestRunReservation { run_token, lease }))
    }

    pub async fn handoff_project_body(
        &self,
        project_id: &ProjectId,
        request: &ChildBodyHandoffRequest,
    ) -> StoreResult<Project> {
        let project_id = project_id.clone();
        let request = request.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.handoff_project_body(&project_id, &request)
        })
        .await
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

    pub(crate) async fn append_project_event_for_run(
        &self,
        project_id: &ProjectId,
        lease: &RunLease,
        kind: &ProjectEventKind,
    ) -> StoreResult<ProjectEvent> {
        let project_id = project_id.clone();
        let lease = lease.clone();
        let kind = kind.clone();
        let write_project_id = project_id.clone();
        let write_kind = kind.clone();
        let event = run_sqlite(&self.sqlite, move |store| {
            store.append_project_event_for_run(&write_project_id, &lease, &write_kind)
        })
        .await?;
        if kind.is_wave_observable() {
            if let Some(project) = self.get_project(&project_id).await? {
                if let Some(wave) = self.get_wave(&project.wave_id).await? {
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

    pub async fn consume_task_observation_for_project(
        &self,
        project_id: &ProjectId,
        observation: &ObservationOutboxRow,
    ) -> StoreResult<bool> {
        let project_id = project_id.clone();
        let observation = observation.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.consume_task_observation_for_project(&project_id, &observation)
        })
        .await
    }

    pub(crate) async fn consume_task_observation_for_project_for_run(
        &self,
        project_id: &ProjectId,
        observation: &ObservationOutboxRow,
        lease: &RunLease,
    ) -> StoreResult<bool> {
        let project_id = project_id.clone();
        let observation = observation.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.consume_task_observation_for_project_for_run(&project_id, &observation, &lease)
        })
        .await
    }
}
