//! Durable Project and Task Sessions: commands, directives, events, and the
//! observation outbox that links each event to its next responsible recipient.

use crate::child_session::{
    AbandonIntent, BoundaryResult, ChildBodyHandoffRequest, ChildBodyOutcome, ChildCommand,
    ChildCommandEffect, ChildCommandId, ChildDirective, ChildProcessGeneration, ChildRef,
    ChildWriteLease, ObservationRecipient,
};
use crate::id::WaveId;
use crate::project_session::{
    ObservationOutboxRow, ProjectEvent, ProjectEventKind, ProjectSession, ProjectSessionId,
    ProjectSessionStatus,
};
use crate::task::{
    LinearObservationApply, LinearObservationOutcome, TaskEvent, TaskEventKind,
    TaskLinearObservation, TaskPr, TaskPrId, TaskSession, TaskSessionId, TaskSessionStatus,
    TaskSessionSuccession,
};
use time::OffsetDateTime;

use super::{run_sqlite, Store, StoreResult};

async fn _acquire_promotion_reservation_lock() -> StoreResult<crate::promotion_lock::PromotionLock>
{
    crate::promotion_lock::acquire_shared()
        .await
        .map_err(|error| {
            super::StoreError::InvalidData(format!(
                "acquire shared promotion lock before body reservation: {error}"
            ))
        })
}

impl Store {
    pub async fn create_task_session(&self, session: &TaskSession, pr: &TaskPr) -> StoreResult<()> {
        let session = session.clone();
        let pr = pr.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_task_session(&session, &pr)
        })
        .await
    }

    pub async fn reserve_task_session_with_directive(
        &self,
        session: &TaskSession,
        pr: &TaskPr,
        directive: &ChildDirective,
    ) -> StoreResult<()> {
        let session = session.clone();
        let pr = pr.clone();
        let directive = directive.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_task_session_with_directive(&session, &pr, &directive)
        })
        .await
    }

    /// Carry a terminal Task Session's direction onto a successor. See
    /// [`crate::store::sqlite::SqliteStore::reserve_task_session_successor`].
    pub async fn reserve_task_session_successor(
        &self,
        predecessor: &TaskSession,
        successor: &TaskSession,
        pr: &TaskPr,
        directive: &ChildDirective,
    ) -> StoreResult<TaskSessionSuccession> {
        let predecessor = predecessor.clone();
        let successor = successor.clone();
        let pr = pr.clone();
        let directive = directive.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_task_session_successor(&predecessor, &successor, &pr, &directive)
        })
        .await
    }

    /// Recover an abandoned Task by atomically adopting its worktree and PR
    /// history onto one linked successor.
    pub async fn recover_task_session_successor(
        &self,
        predecessor: &TaskSession,
        successor: &TaskSession,
        directive: &ChildDirective,
    ) -> StoreResult<TaskSessionSuccession> {
        let predecessor = predecessor.clone();
        let successor = successor.clone();
        let directive = directive.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.recover_task_session_successor(&predecessor, &successor, &directive)
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

    pub(crate) async fn activate_task_process(
        &self,
        session: &TaskSession,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        let session = session.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.activate_task_process(&session, &lease)
        })
        .await
    }

    pub(crate) async fn update_task_session_for_lease(
        &self,
        session: &TaskSession,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        let session = session.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.update_task_session_for_lease(&session, &lease)
        })
        .await
    }

    pub(crate) async fn finish_task_process(
        &self,
        session: &TaskSession,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        let session = session.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.finish_task_process(&session, &lease)
        })
        .await
    }

    pub(crate) async fn revoke_task_process(
        &self,
        session_id: &TaskSessionId,
        outcome: &ChildBodyOutcome,
    ) -> StoreResult<ChildProcessGeneration> {
        let session_id = session_id.clone();
        let outcome = outcome.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.revoke_task_process(&session_id, &outcome)
        })
        .await
    }

    pub(crate) async fn revoke_task_process_if_unchanged(
        &self,
        session_id: &TaskSessionId,
        generation: u32,
        status_at: OffsetDateTime,
        latest_event_id: Option<i64>,
        outcome: &ChildBodyOutcome,
    ) -> StoreResult<Option<ChildProcessGeneration>> {
        let session_id = session_id.clone();
        let outcome = outcome.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.revoke_task_process_if_unchanged(
                &session_id,
                generation,
                status_at,
                latest_event_id,
                &outcome,
            )
        })
        .await
    }

    pub(crate) async fn finish_revoked_task_process(
        &self,
        session_id: &TaskSessionId,
        generation: u32,
    ) -> StoreResult<ChildProcessGeneration> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.finish_revoked_task_process(&session_id, generation)
        })
        .await
    }

    pub async fn complete_task_session(
        &self,
        session: &TaskSession,
        skipped_pr: Option<&TaskPr>,
    ) -> StoreResult<()> {
        let session = session.clone();
        let skipped_pr = skipped_pr.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_task_session(&session, skipped_pr.as_ref())
        })
        .await
    }

    pub(crate) async fn complete_task_session_for_lease(
        &self,
        session: &TaskSession,
        skipped_pr: Option<&TaskPr>,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        let session = session.clone();
        let skipped_pr = skipped_pr.cloned();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_task_session_for_lease(&session, skipped_pr.as_ref(), &lease)
        })
        .await
    }

    pub(crate) async fn reserve_task_process(
        &self,
        session: &TaskSession,
        expected_status: TaskSessionStatus,
    ) -> StoreResult<Option<ChildWriteLease>> {
        let _promotion_lock = _acquire_promotion_reservation_lock().await?;
        let session = session.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_task_process(&session, expected_status)
        })
        .await
    }

    pub async fn handoff_task_body(
        &self,
        session_id: &TaskSessionId,
        request: &ChildBodyHandoffRequest,
    ) -> StoreResult<TaskSession> {
        let session_id = session_id.clone();
        let request = request.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.handoff_task_body(&session_id, &request)
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

    pub async fn task_session_chain_neighbors(
        &self,
        session_id: &TaskSessionId,
    ) -> StoreResult<(Option<String>, Option<String>)> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.task_session_chain_neighbors(&session_id)
        })
        .await
    }

    pub async fn get_task_session_by_issue(&self, issue: &str) -> StoreResult<Option<TaskSession>> {
        let issue = issue.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.task_session_by_issue(&issue)
        })
        .await
    }

    pub async fn get_task_session_by_worktree(
        &self,
        worktree: &str,
    ) -> StoreResult<Option<TaskSession>> {
        let worktree = worktree.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.task_session_by_worktree(&worktree)
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

    pub async fn update_task_pr(&self, pr: &TaskPr) -> StoreResult<()> {
        let pr = pr.clone();
        run_sqlite(&self.sqlite, move |store| store.update_task_pr(&pr)).await
    }

    pub(crate) async fn update_task_pr_for_lease(
        &self,
        pr: &TaskPr,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        let pr = pr.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.update_task_pr_for_lease(&pr, &lease)
        })
        .await
    }

    pub async fn heal_task_pr_base(&self, pr: &TaskPr) -> StoreResult<()> {
        let pr = pr.clone();
        run_sqlite(&self.sqlite, move |store| store.heal_task_pr_base(&pr)).await
    }

    pub(crate) async fn heal_task_pr_base_for_lease(
        &self,
        pr: &TaskPr,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        let pr = pr.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.heal_task_pr_base_for_lease(&pr, &lease)
        })
        .await
    }

    pub async fn task_prs(&self, session_id: &TaskSessionId) -> StoreResult<Vec<TaskPr>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| store.task_prs(&session_id)).await
    }

    pub async fn latest_task_event_at(
        &self,
        session_id: &TaskSessionId,
    ) -> StoreResult<Option<OffsetDateTime>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.latest_task_event_at(&session_id)
        })
        .await
    }

    pub async fn recent_task_events(
        &self,
        session_id: &TaskSessionId,
        limit: u32,
    ) -> StoreResult<Vec<TaskEvent>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.recent_task_events(&session_id, limit)
        })
        .await
    }

    pub async fn latest_task_event(
        &self,
        session_id: &TaskSessionId,
    ) -> StoreResult<Option<TaskEvent>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.latest_task_event(&session_id)
        })
        .await
    }

    pub async fn latest_project_event_at(
        &self,
        session_id: &ProjectSessionId,
    ) -> StoreResult<Option<OffsetDateTime>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.latest_project_event_at(&session_id)
        })
        .await
    }

    pub async fn get_task_pr(&self, pr_id: &TaskPrId) -> StoreResult<Option<TaskPr>> {
        let pr_id = pr_id.clone();
        run_sqlite(&self.sqlite, move |store| store.task_pr(&pr_id)).await
    }

    pub async fn active_task_pr(&self, session_id: &TaskSessionId) -> StoreResult<Option<TaskPr>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| store.active_task_pr(&session_id)).await
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

    pub(crate) async fn settle_task_pr_for_lease(
        &self,
        settled: &TaskPr,
        next: Option<&TaskPr>,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        let settled = settled.clone();
        let next = next.cloned();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.settle_task_pr_for_lease(&settled, next.as_ref(), &lease)
        })
        .await
    }

    pub async fn complete_task_session_after_pr(
        &self,
        session: &TaskSession,
        pr: &TaskPr,
    ) -> StoreResult<()> {
        let session = session.clone();
        let pr = pr.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_task_session_after_pr(&session, &pr)
        })
        .await
    }

    pub(crate) async fn complete_task_session_after_pr_for_lease(
        &self,
        session: &TaskSession,
        pr: &TaskPr,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        let session = session.clone();
        let pr = pr.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_task_session_after_pr_for_lease(&session, &pr, &lease)
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

    pub async fn task_linear_observation(
        &self,
        session_id: &TaskSessionId,
    ) -> StoreResult<Option<TaskLinearObservation>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.task_linear_observation(&session_id)
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
        session_id: &TaskSessionId,
        comment_id: String,
        command: ChildCommand,
        observed_at: OffsetDateTime,
    ) -> StoreResult<Option<ChildCommandId>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.apply_linear_comment(&session_id, &comment_id, &command, observed_at)
        })
        .await
    }

    pub async fn mark_task_linear_degraded(
        &self,
        session_id: &TaskSessionId,
        reason: String,
    ) -> StoreResult<()> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_task_linear_degraded(&session_id, &reason)
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

    /// Create a `ci-fix` wake unless this incident identity already has one.
    /// Returns the surviving command and whether it was created.
    pub async fn ensure_child_ci_fix_command(
        &self,
        command: &ChildCommand,
    ) -> StoreResult<(ChildCommand, bool)> {
        let command = command.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.ensure_child_ci_fix_command(&command)
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

    pub(crate) async fn claim_child_commands_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
    ) -> StoreResult<Vec<ChildCommand>> {
        let target = target.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_child_commands_for_lease(&target, &lease)
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

    pub(crate) async fn claim_task_commands_or_stop_for_lease(
        &self,
        session_id: &TaskSessionId,
        lease: &ChildWriteLease,
        stopped_status: TaskSessionStatus,
        reason: &str,
    ) -> StoreResult<BoundaryResult<TaskSession>> {
        let session_id = session_id.clone();
        let lease = lease.clone();
        let reason = reason.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_task_commands_or_stop_for_lease(
                &session_id,
                &lease,
                stopped_status,
                &reason,
            )
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

    pub(crate) async fn accept_child_command_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
        command_id: &ChildCommandId,
        effect: Option<ChildCommandEffect>,
    ) -> StoreResult<()> {
        let target = target.clone();
        let lease = lease.clone();
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.accept_child_command_for_lease(&target, &lease, &command_id, effect)
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

    pub(crate) async fn mark_child_command_delivering_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
        command_id: &ChildCommandId,
        effect: ChildCommandEffect,
    ) -> StoreResult<()> {
        let target = target.clone();
        let lease = lease.clone();
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_child_command_delivering_for_lease(&target, &lease, &command_id, effect)
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

    pub(crate) async fn mark_stale_child_deliveries_uncertain_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
    ) -> StoreResult<Vec<ChildCommand>> {
        let target = target.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_stale_child_deliveries_uncertain_for_lease(&target, &lease)
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

    /// Pre-delivery rejection of a still-`persisted` command (never claimed),
    /// recording `error`. The supervisor-`Resume` open-PR race uses this to
    /// terminalize the command it just created without racing a body: unlike
    /// [`fail_child_command`], whose UPDATE matches only `claimed`/`delivering`,
    /// this matches only `persisted`.
    pub async fn reject_persisted_child_command(
        &self,
        command_id: &ChildCommandId,
        error: String,
    ) -> StoreResult<()> {
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reject_persisted_child_command(&command_id, &error)
        })
        .await
    }

    /// Mark one claimed command superseded because circumstances made it moot.
    /// Unlike failing it, this records no error — nothing went wrong.
    pub(crate) async fn supersede_child_command_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
        command_id: &ChildCommandId,
    ) -> StoreResult<()> {
        let target = target.clone();
        let lease = lease.clone();
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.supersede_child_command_for_lease(&target, &lease, &command_id)
        })
        .await
    }

    pub(crate) async fn fail_child_command_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
        command_id: &ChildCommandId,
        effect: Option<ChildCommandEffect>,
        error: String,
    ) -> StoreResult<()> {
        let target = target.clone();
        let lease = lease.clone();
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.fail_child_command_for_lease(&target, &lease, &command_id, effect, &error)
        })
        .await
    }

    pub(crate) async fn validate_child_write_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        let target = target.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.validate_child_write_lease(&target, &lease)
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
                    crate::ops::project::wake_task_project_route(self, &session).await
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

    pub(crate) async fn append_task_event_for_lease(
        &self,
        session_id: &TaskSessionId,
        lease: &ChildWriteLease,
        kind: &TaskEventKind,
    ) -> StoreResult<TaskEvent> {
        let session_id = session_id.clone();
        let lease = lease.clone();
        let kind = kind.clone();
        let write_session_id = session_id.clone();
        let write_kind = kind.clone();
        let event = run_sqlite(&self.sqlite, move |store| {
            store.append_task_event_for_lease(&write_session_id, &lease, &write_kind)
        })
        .await?;
        if kind.is_project_observable() {
            if let Some(session) = self.get_task_session(&session_id).await? {
                if let Err(error) =
                    crate::ops::project::wake_task_project_route(self, &session).await
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

    pub(crate) async fn activate_project_process(
        &self,
        session: &ProjectSession,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        let session = session.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.activate_project_process(&session, &lease)
        })
        .await
    }

    pub(crate) async fn update_project_session_for_lease(
        &self,
        session: &ProjectSession,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        let session = session.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.update_project_session_for_lease(&session, &lease)
        })
        .await
    }

    pub(crate) async fn finish_project_process(
        &self,
        session: &ProjectSession,
        lease: &ChildWriteLease,
    ) -> StoreResult<()> {
        let session = session.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.finish_project_process(&session, &lease)
        })
        .await
    }

    pub(crate) async fn revoke_project_process(
        &self,
        session_id: &ProjectSessionId,
        outcome: &ChildBodyOutcome,
    ) -> StoreResult<ChildProcessGeneration> {
        let session_id = session_id.clone();
        let outcome = outcome.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.revoke_project_process(&session_id, &outcome)
        })
        .await
    }

    pub(crate) async fn finish_revoked_project_process(
        &self,
        session_id: &ProjectSessionId,
        generation: u32,
    ) -> StoreResult<ChildProcessGeneration> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.finish_revoked_project_process(&session_id, generation)
        })
        .await
    }

    pub(crate) async fn reserve_project_process(
        &self,
        session: &ProjectSession,
        expected_status: ProjectSessionStatus,
    ) -> StoreResult<Option<ChildWriteLease>> {
        let _promotion_lock = _acquire_promotion_reservation_lock().await?;
        let session = session.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_project_process(&session, expected_status)
        })
        .await
    }

    pub async fn handoff_project_body(
        &self,
        session_id: &ProjectSessionId,
        request: &ChildBodyHandoffRequest,
    ) -> StoreResult<ProjectSession> {
        let session_id = session_id.clone();
        let request = request.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.handoff_project_body(&session_id, &request)
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

    pub(crate) async fn claim_project_commands_or_stop_for_lease(
        &self,
        session_id: &ProjectSessionId,
        lease: &ChildWriteLease,
        stopped_status: ProjectSessionStatus,
        reason: String,
    ) -> StoreResult<BoundaryResult<ProjectSession>> {
        let session_id = session_id.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_project_commands_or_stop_for_lease(
                &session_id,
                &lease,
                stopped_status,
                &reason,
            )
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

    pub(crate) async fn append_project_event_for_lease(
        &self,
        session_id: &ProjectSessionId,
        lease: &ChildWriteLease,
        kind: &ProjectEventKind,
    ) -> StoreResult<ProjectEvent> {
        let session_id = session_id.clone();
        let lease = lease.clone();
        let kind = kind.clone();
        let write_session_id = session_id.clone();
        let write_kind = kind.clone();
        let event = run_sqlite(&self.sqlite, move |store| {
            store.append_project_event_for_lease(&write_session_id, &lease, &write_kind)
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

    /// Undelivered Task observations addressed to any Project Session in the
    /// chain for `project_id` — the successor plus its terminal predecessors.
    /// The live successor consumes the whole chain; the outbox `recipient_id`
    /// stays the historical owner, so this is routing, not rewriting.
    pub async fn pending_project_observations_for_chain(
        &self,
        project_id: &str,
    ) -> StoreResult<Vec<ObservationOutboxRow>> {
        let project_id = project_id.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.pending_project_observations_for_chain(&project_id)
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

    pub(crate) async fn consume_task_observation_for_project_for_lease(
        &self,
        project_session_id: &ProjectSessionId,
        observation: &ObservationOutboxRow,
        lease: &ChildWriteLease,
    ) -> StoreResult<bool> {
        let project_session_id = project_session_id.clone();
        let observation = observation.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.consume_task_observation_for_project_for_lease(
                &project_session_id,
                &observation,
                &lease,
            )
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

    pub(crate) async fn mark_child_directive_applied_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
        version: u32,
    ) -> StoreResult<()> {
        let target = target.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_child_directive_applied_for_lease(&target, &lease, version)
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

    pub(crate) async fn incorporate_child_directive_for_lease(
        &self,
        target: &ChildRef,
        lease: &ChildWriteLease,
        version: u32,
        summary: &str,
    ) -> StoreResult<(ChildDirective, bool)> {
        let target = target.clone();
        let lease = lease.clone();
        let summary = summary.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.incorporate_child_directive_for_lease(&target, &lease, version, &summary)
        })
        .await
    }
}
