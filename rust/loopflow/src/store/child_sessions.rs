//! Durable Project and Task Sessions: commands, directives, events, and the
//! observation outbox that links each event to its next responsible recipient.

use crate::child_session::{
    AbandonIntent, BoundaryResult, ChildCommand, ChildCommandEffect, ChildCommandId,
    ChildDirective, ChildRef, ObservationRecipient,
};
use crate::id::WaveId;
use crate::project_session::{
    ObservationOutboxRow, ProjectEvent, ProjectEventKind, ProjectSession, ProjectSessionId,
    ProjectSessionStatus,
};
use crate::task::{
    TaskDelivery, TaskDeliveryId, TaskEvent, TaskEventKind, TaskSession, TaskSessionId,
    TaskSessionStatus,
};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn create_task_session(
        &self,
        session: &TaskSession,
        delivery: &TaskDelivery,
    ) -> StoreResult<()> {
        let session = session.clone();
        let delivery = delivery.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_task_session(&session, &delivery)
        })
        .await
    }

    pub async fn reserve_task_session_with_directive(
        &self,
        session: &TaskSession,
        delivery: &TaskDelivery,
        directive: &ChildDirective,
    ) -> StoreResult<()> {
        let session = session.clone();
        let delivery = delivery.clone();
        let directive = directive.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_task_session_with_directive(&session, &delivery, &directive)
        })
        .await
    }

    pub async fn update_task_session(&self, session: &TaskSession) -> StoreResult<()> {
        let session = session.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.update_task_session(&session)
        })
        .await
    }

    pub async fn complete_task_session(
        &self,
        session: &TaskSession,
        empty_delivery: Option<&TaskDelivery>,
    ) -> StoreResult<()> {
        let session = session.clone();
        let empty_delivery = empty_delivery.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_task_session(&session, empty_delivery.as_ref())
        })
        .await
    }

    pub async fn reserve_task_process(
        &self,
        session: &TaskSession,
        expected_status: TaskSessionStatus,
    ) -> StoreResult<bool> {
        let session = session.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_task_process(&session, expected_status)
        })
        .await
    }

    pub async fn get_task_session(
        &self,
        session_id: &TaskSessionId,
    ) -> StoreResult<Option<TaskSession>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| store.task_session(&session_id)).await
    }

    pub async fn get_task_session_by_issue(&self, issue: &str) -> StoreResult<Option<TaskSession>> {
        let issue = issue.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.task_session_by_issue(&issue)
        })
        .await
    }

    pub async fn list_task_sessions(
        &self,
        wave_id: Option<&WaveId>,
    ) -> StoreResult<Vec<TaskSession>> {
        let wave_id = wave_id.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.list_task_sessions(wave_id.as_ref())
        })
        .await
    }

    pub async fn update_task_delivery(&self, delivery: &TaskDelivery) -> StoreResult<()> {
        let delivery = delivery.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.update_task_delivery(&delivery)
        })
        .await
    }

    pub async fn get_task_delivery(
        &self,
        delivery_id: &TaskDeliveryId,
    ) -> StoreResult<Option<TaskDelivery>> {
        let delivery_id = delivery_id.clone();
        run_sqlite(&self.sqlite, move |store| store.task_delivery(&delivery_id)).await
    }

    pub async fn task_deliveries(
        &self,
        session_id: &TaskSessionId,
    ) -> StoreResult<Vec<TaskDelivery>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.task_deliveries(&session_id)
        })
        .await
    }

    pub async fn active_task_delivery(
        &self,
        session_id: &TaskSessionId,
    ) -> StoreResult<Option<TaskDelivery>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.active_task_delivery(&session_id)
        })
        .await
    }

    pub async fn settle_task_delivery(
        &self,
        settled: &TaskDelivery,
        next: Option<&TaskDelivery>,
    ) -> StoreResult<()> {
        let settled = settled.clone();
        let next = next.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.settle_task_delivery(&settled, next.as_ref())
        })
        .await
    }

    pub async fn create_child_command(&self, command: &ChildCommand) -> StoreResult<()> {
        let command = command.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_child_command(&command)
        })
        .await
    }

    pub async fn create_child_abandon_command(
        &self,
        command: &ChildCommand,
        intent: &AbandonIntent,
    ) -> StoreResult<()> {
        let command = command.clone();
        let intent = intent.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_child_abandon_command(&command, &intent)
        })
        .await
    }

    pub async fn ensure_child_decision_command(
        &self,
        command: &ChildCommand,
    ) -> StoreResult<(ChildCommand, bool)> {
        let command = command.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.ensure_child_decision_command(&command)
        })
        .await
    }

    pub async fn supersede_and_create_child_command(
        &self,
        command: &ChildCommand,
    ) -> StoreResult<Vec<ChildCommandId>> {
        let command = command.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.supersede_and_insert_child_command(&command)
        })
        .await
    }

    pub async fn create_child_command_with_directive(
        &self,
        command: &ChildCommand,
        directive: &ChildDirective,
    ) -> StoreResult<Vec<ChildCommandId>> {
        let command = command.clone();
        let directive = directive.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_child_command_with_directive(&command, &directive)
        })
        .await
    }

    pub async fn get_child_command(
        &self,
        command_id: &ChildCommandId,
    ) -> StoreResult<Option<ChildCommand>> {
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| store.child_command(&command_id)).await
    }

    pub async fn list_child_commands(&self, target: &ChildRef) -> StoreResult<Vec<ChildCommand>> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| store.child_commands(&target)).await
    }

    pub async fn claim_child_commands(
        &self,
        target: &ChildRef,
        generation: u32,
    ) -> StoreResult<Vec<ChildCommand>> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_child_commands(&target, generation)
        })
        .await
    }

    pub async fn claim_task_commands_or_stop(
        &self,
        session_id: &TaskSessionId,
        generation: u32,
        stopped_status: TaskSessionStatus,
        reason: &str,
    ) -> StoreResult<BoundaryResult<TaskSession>> {
        let session_id = session_id.clone();
        let reason = reason.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_task_commands_or_stop(&session_id, generation, stopped_status, &reason)
        })
        .await
    }

    pub async fn accept_child_command(
        &self,
        command_id: &ChildCommandId,
        effect: Option<ChildCommandEffect>,
    ) -> StoreResult<()> {
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.accept_child_command(&command_id, effect)
        })
        .await
    }

    pub async fn mark_child_command_delivering(
        &self,
        command_id: &ChildCommandId,
        effect: ChildCommandEffect,
    ) -> StoreResult<()> {
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_child_command_delivering(&command_id, effect)
        })
        .await
    }

    pub async fn mark_stale_child_deliveries_uncertain(
        &self,
        target: &ChildRef,
        generation: u32,
    ) -> StoreResult<Vec<ChildCommand>> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_stale_child_deliveries_uncertain(&target, generation)
        })
        .await
    }

    pub async fn set_child_command_effect(
        &self,
        command_id: &ChildCommandId,
        effect: ChildCommandEffect,
    ) -> StoreResult<()> {
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.set_child_command_effect(&command_id, effect)
        })
        .await
    }

    pub async fn fail_child_command(
        &self,
        command_id: &ChildCommandId,
        effect: Option<ChildCommandEffect>,
        error: String,
    ) -> StoreResult<()> {
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.fail_child_command(&command_id, effect, &error)
        })
        .await
    }

    pub async fn append_task_event(
        &self,
        session_id: &TaskSessionId,
        kind: &TaskEventKind,
    ) -> StoreResult<TaskEvent> {
        let session_id = session_id.clone();
        let kind = kind.clone();
        let write_session_id = session_id.clone();
        let write_kind = kind.clone();
        let event = run_sqlite(&self.sqlite, move |store| {
            store.append_task_event(&write_session_id, &write_kind)
        })
        .await?;
        if kind.is_project_observable() {
            if let Some(session) = self.get_task_session(&session_id).await? {
                if let Err(error) =
                    crate::ops::project::wake_project_session(&session.project_session_id).await
                {
                    tracing::debug!(
                        %error,
                        %session_id,
                        project_session_id = %session.project_session_id,
                        event_id = event.id,
                        "Task observation wake failed; Project lifecycle touch will retry"
                    );
                }
                if kind.is_root_wave_observable() {
                    match self.get_wave(&session.wave_id).await? {
                        Some(wave) => {
                            if let Err(error) =
                                crate::lf::commands::chat::nudge_child_observations(wave.name())
                                    .await
                            {
                                tracing::debug!(
                                    %error,
                                    %session_id,
                                    event_id = event.id,
                                    "live Task observation delivery failed; Wave observer will retry"
                                );
                            }
                        }
                        None => tracing::error!(
                            wave_id = %session.wave_id,
                            %session_id,
                            event_id = event.id,
                            "Task observation cannot nudge its missing owning Wave"
                        ),
                    }
                }
            }
        }
        Ok(event)
    }

    pub async fn task_events_after(
        &self,
        session_id: &TaskSessionId,
        cursor: i64,
    ) -> StoreResult<Vec<TaskEvent>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.task_events_after(&session_id, cursor)
        })
        .await
    }

    pub async fn get_task_event(
        &self,
        session_id: &TaskSessionId,
        event_id: i64,
    ) -> StoreResult<Option<TaskEvent>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.task_event(&session_id, event_id)
        })
        .await
    }

    pub async fn create_project_session(&self, session: &ProjectSession) -> StoreResult<()> {
        let session = session.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_project_session(&session)
        })
        .await
    }

    pub async fn create_project_session_with_directive(
        &self,
        session: &ProjectSession,
        directive: &ChildDirective,
    ) -> StoreResult<()> {
        let session = session.clone();
        let directive = directive.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_project_session_with_directive(&session, &directive)
        })
        .await
    }

    pub async fn update_project_session(&self, session: &ProjectSession) -> StoreResult<()> {
        let session = session.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.update_project_session(&session)
        })
        .await
    }

    pub async fn reserve_project_process(
        &self,
        session: &ProjectSession,
        expected_status: ProjectSessionStatus,
    ) -> StoreResult<bool> {
        let session = session.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_project_process(&session, expected_status)
        })
        .await
    }

    pub async fn get_project_session(
        &self,
        session_id: &ProjectSessionId,
    ) -> StoreResult<Option<ProjectSession>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.project_session(&session_id)
        })
        .await
    }

    pub async fn get_project_session_by_project(
        &self,
        project: &str,
    ) -> StoreResult<Option<ProjectSession>> {
        let project = project.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.project_session_by_project(&project)
        })
        .await
    }

    pub async fn list_project_sessions(
        &self,
        wave_id: Option<&WaveId>,
    ) -> StoreResult<Vec<ProjectSession>> {
        let wave_id = wave_id.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.list_project_sessions(wave_id.as_ref())
        })
        .await
    }

    pub async fn claim_project_commands_or_stop(
        &self,
        session_id: &ProjectSessionId,
        generation: u32,
        stopped_status: ProjectSessionStatus,
        reason: String,
    ) -> StoreResult<BoundaryResult<ProjectSession>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_project_commands_or_stop(&session_id, generation, stopped_status, &reason)
        })
        .await
    }

    pub async fn append_project_event(
        &self,
        session_id: &ProjectSessionId,
        kind: &ProjectEventKind,
    ) -> StoreResult<ProjectEvent> {
        let session_id = session_id.clone();
        let kind = kind.clone();
        let write_session_id = session_id.clone();
        let write_kind = kind.clone();
        let event = run_sqlite(&self.sqlite, move |store| {
            store.append_project_event(&write_session_id, &write_kind)
        })
        .await?;
        if kind.is_wave_observable() {
            if let Some(session) = self.get_project_session(&session_id).await? {
                match self.get_wave(&session.wave_id).await? {
                    Some(wave) => {
                        if let Err(error) =
                            crate::lf::commands::chat::nudge_child_observations(wave.name()).await
                        {
                            tracing::debug!(
                                %error,
                                %session_id,
                                event_id = event.id,
                                "live Project observation delivery failed; Wave observer will retry"
                            );
                        }
                    }
                    None => tracing::error!(
                        wave_id = %session.wave_id,
                        %session_id,
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
        session_id: &ProjectSessionId,
        cursor: i64,
    ) -> StoreResult<Vec<ProjectEvent>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.project_events_after(&session_id, cursor)
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

    pub async fn mark_observation_delivered(&self, id: i64) -> StoreResult<()> {
        run_sqlite(&self.sqlite, move |store| {
            store.mark_observation_delivered(id)
        })
        .await
    }

    pub async fn consume_task_observation_for_project(
        &self,
        project_session_id: &ProjectSessionId,
        observation: &ObservationOutboxRow,
    ) -> StoreResult<bool> {
        let project_session_id = project_session_id.clone();
        let observation = observation.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.consume_task_observation_for_project(&project_session_id, &observation)
        })
        .await
    }

    pub async fn child_directives(&self, target: &ChildRef) -> StoreResult<Vec<ChildDirective>> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| store.child_directives(&target)).await
    }

    pub async fn child_directive_for_command(
        &self,
        command_id: &ChildCommandId,
    ) -> StoreResult<Option<ChildDirective>> {
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.child_directive_for_command(&command_id)
        })
        .await
    }

    pub async fn mark_child_directive_applied(
        &self,
        target: &ChildRef,
        version: u32,
    ) -> StoreResult<()> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_child_directive_applied(&target, version)
        })
        .await
    }

    pub async fn incorporate_child_directive(
        &self,
        target: &ChildRef,
        version: u32,
        summary: &str,
    ) -> StoreResult<(ChildDirective, bool)> {
        let target = target.clone();
        let summary = summary.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.incorporate_child_directive(&target, version, &summary)
        })
        .await
    }
}
