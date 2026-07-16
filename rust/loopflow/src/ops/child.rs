//! Shared durable control submission for Project and Task Sessions.
//!
//! The public nouns stay explicit. This module owns only the protocol that is
//! already common at both edges: persist one command, supersede or version its
//! directive atomically, wake the right child, and report the durable receipt.

use std::time::Duration;

use crate::child_session::{
    AbandonIntent, ChildBodyHandoffRequest, ChildBodyOutcome, ChildCommand, ChildCommandEffect,
    ChildCommandId, ChildCommandKind, ChildCommandSource, ChildCommandState, ChildDirective,
    ChildProcessGeneration, ChildRef,
};
use crate::project_session::{ProjectEventKind, ProjectSession, ProjectSessionStatus};
use crate::store::SharedStore;
use crate::task::{TaskEventKind, TaskSession, TaskSessionStatus};

use super::{OpsError, OpsResult};

const CHILD_STARTUP_GRACE_SECONDS: i64 = 10;
pub(crate) const CHILD_STARTUP_GRACE: Duration = Duration::from_secs(10);

pub(crate) fn child_body_reservation_is_fresh(process: &ChildProcessGeneration) -> bool {
    if process.state != crate::child_session::ChildLeaseState::Reserved {
        return false;
    }
    process
        .started_at
        .checked_add(time::Duration::seconds(CHILD_STARTUP_GRACE_SECONDS))
        .is_some_and(|deadline| time::OffsetDateTime::now_utc() < deadline)
}

pub(crate) fn lost_child_body_outcome(
    process: &ChildProcessGeneration,
    reason: &str,
) -> ChildBodyOutcome {
    if process.state == crate::child_session::ChildLeaseState::Legacy {
        ChildBodyOutcome::LegacyStopped {
            reason: reason.to_string(),
        }
    } else {
        ChildBodyOutcome::Lost {
            reason: reason.to_string(),
        }
    }
}

pub(crate) async fn revoke_and_reap_child_body(
    store: &SharedStore,
    target: &ChildRef,
    outcome: ChildBodyOutcome,
) -> OpsResult<ChildProcessGeneration> {
    let revoked = match target {
        ChildRef::Project(session_id) => store
            .revoke_project_process(session_id, &outcome)
            .await
            .map_err(child_error)?,
        ChildRef::Task(session_id) => store
            .revoke_task_process(session_id, &outcome)
            .await
            .map_err(child_error)?,
    };
    reap_revoked_child_body(store, target, revoked).await
}

pub(crate) async fn reap_revoked_child_body(
    store: &SharedStore,
    target: &ChildRef,
    revoked: ChildProcessGeneration,
) -> OpsResult<ChildProcessGeneration> {
    crate::engine::process::reap_child_process(&revoked, Duration::from_secs(2))
        .await
        .map_err(child_error)?;
    match target {
        ChildRef::Project(session_id) => store
            .finish_revoked_project_process(session_id, revoked.generation)
            .await
            .map_err(child_error),
        ChildRef::Task(session_id) => store
            .finish_revoked_task_process(session_id, revoked.generation)
            .await
            .map_err(child_error),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ChildReceiptUntil {
    Applied,
    Incorporated,
}

#[derive(Debug)]
pub(crate) enum ChildSession {
    Project(Box<ProjectSession>),
    Task(Box<TaskSession>),
}

impl ChildSession {
    fn target(&self) -> ChildRef {
        match self {
            Self::Project(session) => ChildRef::Project(session.id.clone()),
            Self::Task(session) => ChildRef::Task(session.id.clone()),
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Project(session) => format!("Project {}", session.launch.project.slug),
            Self::Task(session) => format!("Task {}", session.launch.issue.identifier),
        }
    }

    fn status(&self) -> &'static str {
        match self {
            Self::Project(session) => session.status.as_str(),
            Self::Task(session) => session.status.as_str(),
        }
    }

    fn is_terminal(&self) -> bool {
        match self {
            Self::Project(session) => session.status.is_terminal(),
            Self::Task(session) => session.status.is_terminal(),
        }
    }

    fn is_process_active(&self) -> bool {
        match self {
            Self::Project(session) => session.status.is_process_active(),
            Self::Task(session) => session.status.is_process_active(),
        }
    }

    fn agent(&self) -> &str {
        match self {
            Self::Project(session) => &session.agent,
            Self::Task(session) => &session.agent,
        }
    }

    async fn handoff(
        &mut self,
        store: &SharedStore,
        request: &ChildBodyHandoffRequest,
    ) -> OpsResult<()> {
        *self = match self {
            Self::Project(session) => Self::Project(Box::new(
                store
                    .handoff_project_body(&session.id, request)
                    .await
                    .map_err(child_error)?,
            )),
            Self::Task(session) => Self::Task(Box::new(
                store
                    .handoff_task_body(&session.id, request)
                    .await
                    .map_err(child_error)?,
            )),
        };
        Ok(())
    }

    async fn supervisor_restart_bar(&self, store: &SharedStore) -> OpsResult<Option<String>> {
        match self {
            Self::Project(session) => Ok(session.supervisor_restart_bar()),
            Self::Task(session) => {
                let pr = store
                    .active_task_pr(&session.id)
                    .await
                    .map_err(child_error)?;
                Ok(session.supervisor_restart_bar(pr.as_ref()))
            }
        }
    }

    /// The `ci-fix` wake bar. Task-only: it permits the open-PR restart the
    /// supervisor bar forbids, but only on a warranted current-head failure. A
    /// Project is never woken this way, so it falls back to the supervisor bar.
    async fn ci_fix_restart_bar(&self, store: &SharedStore) -> OpsResult<Option<String>> {
        match self {
            Self::Project(session) => Ok(session.supervisor_restart_bar()),
            Self::Task(session) => {
                let pr = store
                    .active_task_pr(&session.id)
                    .await
                    .map_err(child_error)?;
                Ok(session.ci_fix_restart_bar(pr.as_ref()))
            }
        }
    }

    /// The automatic-recovery bar: terminal and abandoning work only.
    ///
    /// Unlike the supervisor bar, this does **not** consult the PR phase. A
    /// strand is decided by durable status — `plan_stranded_recovery` only ever
    /// reaches a launch for a Session whose status claims a body that does not
    /// exist, and delivered work parks at `Waiting`, which it never selects. So
    /// the open-PR/W2-129 bar is already honoured upstream, and re-applying it
    /// here would refuse exactly the crashed-mid-publication strands this path
    /// exists to recover (their PR phase moves to `Open` after the fact, when
    /// the receipt is finally observed).
    fn recovery_restart_bar(&self) -> Option<String> {
        match self {
            Self::Project(session) => session.supervisor_restart_bar(),
            Self::Task(session) => session.terminal_or_abandon_bar(),
        }
    }

    fn current_directive_version(&self) -> u32 {
        match self {
            Self::Project(session) => session.current_directive_version,
            Self::Task(session) => session.current_directive_version,
        }
    }

    fn set_current_directive_version(&mut self, version: u32) {
        match self {
            Self::Project(session) => session.current_directive_version = version,
            Self::Task(session) => session.current_directive_version = version,
        }
    }

    async fn refresh(&mut self, store: &SharedStore) -> OpsResult<()> {
        *self = match self {
            Self::Project(session) => Self::Project(Box::new(
                store
                    .get_project_session(&session.id)
                    .await
                    .map_err(child_error)?
                    .ok_or_else(|| child_error("Project Session disappeared"))?,
            )),
            Self::Task(session) => Self::Task(Box::new(
                store
                    .get_task_session(&session.id)
                    .await
                    .map_err(child_error)?
                    .ok_or_else(|| child_error("Task Session disappeared"))?,
            )),
        };
        Ok(())
    }

    fn record_abandon_intent(&mut self, intent: AbandonIntent) {
        match self {
            Self::Project(session) => session.abandon_intent = Some(intent),
            Self::Task(session) => session.abandon_intent = Some(intent),
        }
    }

    fn abandon_intent_reason(&self) -> Option<String> {
        let intent = match self {
            Self::Project(session) => session.abandon_intent.as_ref(),
            Self::Task(session) => session.abandon_intent.as_ref(),
        };
        intent.map(|intent| {
            format!(
                "{} is being abandoned: {}",
                self.label(),
                intent.reason.clone()
            )
        })
    }

    /// Start a process generation, unless this Session's own state forbids it.
    ///
    /// `intent` decides how much is forbidden. A supervisor — a project waking on
    /// a task observation, a queued steer, an internal retry — may not restart
    /// delivered or abandoning work. An operator typing `lf task resume` may
    /// restart delivered work (that is how review feedback gets answered), but
    /// not abandoning work.
    async fn launch(&mut self, store: &SharedStore, intent: LaunchIntent) -> OpsResult<()> {
        let bar = match intent {
            LaunchIntent::Supervisor => self.supervisor_restart_bar(store).await?,
            LaunchIntent::ExplicitResume => self.abandon_intent_reason(),
            LaunchIntent::CiFix => self.ci_fix_restart_bar(store).await?,
            LaunchIntent::Recovery => self.recovery_restart_bar(),
        };
        if let Some(bar) = bar {
            return Err(child_error(bar));
        }
        match self {
            Self::Project(session) => super::project::launch_project_process(store, session).await,
            Self::Task(session) => super::task::relaunch_inactive_process(store, session).await,
        }
    }

    async fn append_command_event(
        &self,
        store: &SharedStore,
        command_id: ChildCommandId,
        state: ChildCommandState,
        effect: Option<ChildCommandEffect>,
    ) -> OpsResult<()> {
        match self {
            Self::Project(session) => store
                .append_project_event(
                    &session.id,
                    &ProjectEventKind::CommandChanged {
                        command_id,
                        state,
                        effect,
                        error: None,
                    },
                )
                .await
                .map(|_| ())
                .map_err(child_error),
            Self::Task(session) => store
                .append_task_event(
                    &session.id,
                    &TaskEventKind::CommandChanged {
                        command_id,
                        state,
                        effect,
                        error: None,
                    },
                )
                .await
                .map(|_| ())
                .map_err(child_error),
        }
    }

    async fn append_directive_event(
        &self,
        store: &SharedStore,
        directive: &ChildDirective,
    ) -> OpsResult<()> {
        match self {
            Self::Project(session) => store
                .append_project_event(
                    &session.id,
                    &ProjectEventKind::DirectiveChanged {
                        directive_id: directive.id.clone(),
                        version: directive.version,
                        directive_kind: directive.kind,
                    },
                )
                .await
                .map(|_| ())
                .map_err(child_error),
            Self::Task(session) => store
                .append_task_event(
                    &session.id,
                    &TaskEventKind::DirectiveChanged {
                        directive_id: directive.id.clone(),
                        version: directive.version,
                        directive_kind: directive.kind,
                    },
                )
                .await
                .map(|_| ())
                .map_err(child_error),
        }
    }

    /// Record that an interrupt landed on a Session whose process was already gone.
    ///
    /// Without this the command is accepted silently: the receipt says `accepted`
    /// while `lf task show` still reads whatever it read before, so an operator
    /// cannot tell an interrupt that landed from one that evaporated. A delivered
    /// (`Open`) Session keeps its status — interrupting it must not erase the
    /// fact that its PR is open.
    async fn record_interrupt_of_inactive_process(&mut self, store: &SharedStore) -> OpsResult<()> {
        let reason = "interrupted while no process was running; \
                      resume the Session to start one"
            .to_string();
        match self {
            Self::Project(session) => {
                let from = session.status;
                session.set_status(ProjectSessionStatus::Waiting, reason);
                store
                    .update_project_session(session)
                    .await
                    .map_err(child_error)?;
                store
                    .append_project_event(
                        &session.id,
                        &ProjectEventKind::StatusChanged {
                            from,
                            to: ProjectSessionStatus::Waiting,
                            reason: session.status_reason.clone(),
                        },
                    )
                    .await
                    .map_err(child_error)?;
            }
            Self::Task(session) => {
                let submitted = store
                    .active_task_pr(&session.id)
                    .await
                    .map_err(child_error)?
                    .is_some_and(|pr| pr.phase() == crate::task::PrPhase::Open);
                if submitted {
                    return Ok(());
                }
                let from = session.status;
                session.set_status(TaskSessionStatus::Waiting, reason);
                store
                    .update_task_session(session)
                    .await
                    .map_err(child_error)?;
                store
                    .append_task_event(
                        &session.id,
                        &TaskEventKind::StatusChanged {
                            from,
                            to: TaskSessionStatus::Waiting,
                            reason: session.status_reason.clone(),
                        },
                    )
                    .await
                    .map_err(child_error)?;
            }
        }
        Ok(())
    }

    async fn abandon(&mut self, store: &SharedStore, reason: &str) -> OpsResult<()> {
        match self {
            Self::Project(session) => {
                let from = session.status;
                session.set_status(
                    ProjectSessionStatus::Abandoned,
                    format!("Project Session explicitly abandoned: {reason}"),
                );
                store
                    .update_project_session(session)
                    .await
                    .map_err(child_error)?;
                store
                    .append_project_event(
                        &session.id,
                        &ProjectEventKind::StatusChanged {
                            from,
                            to: ProjectSessionStatus::Abandoned,
                            reason: session.status_reason.clone(),
                        },
                    )
                    .await
                    .map_err(child_error)?;
            }
            Self::Task(session) => {
                let from = session.status;
                session.set_status(
                    TaskSessionStatus::Abandoned,
                    format!("Task Session explicitly abandoned: {reason}"),
                );
                store
                    .update_task_session(session)
                    .await
                    .map_err(child_error)?;
                store
                    .append_task_event(
                        &session.id,
                        &TaskEventKind::StatusChanged {
                            from,
                            to: TaskSessionStatus::Abandoned,
                            reason: session.status_reason.clone(),
                        },
                    )
                    .await
                    .map_err(child_error)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChildControlResult {
    pub session_id: String,
    pub command_id: String,
    pub directive_version: Option<u32>,
    pub state: ChildCommandState,
    pub effect: Option<ChildCommandEffect>,
    pub incorporated: bool,
    pub generation: Option<u32>,
    pub accepted_at: Option<time::OffsetDateTime>,
    pub incorporated_at: Option<time::OffsetDateTime>,
    pub error: Option<String>,
}

/// Who is asking for a process, which decides what state may refuse them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LaunchIntent {
    Supervisor,
    ExplicitResume,
    /// An automated `ci-fix` wake: allowed past the open-PR bar, but only when
    /// the active PR carries a warranted current-head required-check failure
    /// (`TaskSession::ci_fix_restart_bar`).
    CiFix,
    /// Automatic recovery of a Session whose body died while its status still
    /// claimed one. Barred only by terminal and abandoning work; the strand
    /// predicate upstream (`plan_stranded_recovery`) is what keeps it off
    /// delivered work.
    Recovery,
}

/// Wake a Task that is sleeping on an open PR into a bounded `ci-fix` turn.
///
/// Launches a fresh generation under [`LaunchIntent::CiFix`], which the
/// `ci_fix_restart_bar` permits past the open-PR/W2-129 bar only because the
/// active PR carries a warranted current-head required-check failure. A no-op if
/// a process is already active — the caller may poll every supervision pass, and
/// the lease CAS in `reserve_task_process` is the second guard against a double
/// body. Returns whether a generation was launched.
pub(crate) async fn wake_task_ci_fix(
    store: &SharedStore,
    session: &mut TaskSession,
) -> OpsResult<bool> {
    if session.status.is_process_active() {
        return Ok(false);
    }
    let mut child = ChildSession::Task(Box::new(session.clone()));
    child.launch(store, LaunchIntent::CiFix).await?;
    if let ChildSession::Task(relaunched) = child {
        *session = *relaunched;
    }
    Ok(true)
}

/// Re-dispatch a Task Session whose body died, with no human asking.
///
/// The one automatic path for a strand. Barred only by terminal and abandoning
/// work — see [`ChildSession::recovery_restart_bar`] for why the PR phase is
/// deliberately not consulted here.
pub(crate) async fn redispatch_task_body(
    store: &SharedStore,
    session: &mut TaskSession,
) -> OpsResult<()> {
    let mut child = ChildSession::Task(Box::new(session.clone()));
    child.launch(store, LaunchIntent::Recovery).await?;
    if let ChildSession::Task(relaunched) = child {
        *session = *relaunched;
    }
    Ok(())
}

pub(crate) async fn resume_session(
    store: &SharedStore,
    mut session: ChildSession,
    source: ChildCommandSource,
    message: Option<String>,
    model: Option<String>,
    reason: Option<String>,
) -> OpsResult<ChildControlResult> {
    if let Some(model) = model {
        let request = handoff_request(&model, reason.as_deref())?;
        if session.agent() != request.agent {
            if !matches!(&source, ChildCommandSource::Human) {
                if let Some(bar) = session.supervisor_restart_bar(store).await? {
                    return Err(child_error(bar));
                }
            }
            session.handoff(store, &request).await?;
        }
    }
    queue_command(store, session, source, ChildCommandKind::Resume { message }).await
}

fn handoff_request(model: &str, reason: Option<&str>) -> OpsResult<ChildBodyHandoffRequest> {
    let agent = model.trim();
    if agent.is_empty() {
        return Err(child_error("handoff model cannot be empty"));
    }
    let (provider, model_name) = agent
        .split_once(':')
        .map_or((agent, None), |(provider, model_name)| {
            (provider, Some(model_name))
        });
    let provider = crate::harness::canonical_harness(provider)
        .ok_or_else(|| child_error(format!("unsupported session harness: {provider}")))?;
    let agent = match model_name {
        Some(model_name) if model_name.trim().is_empty() => {
            return Err(child_error("handoff model name cannot be empty"));
        }
        Some(model_name) => format!("{provider}:{}", model_name.trim()),
        None => provider.to_string(),
    };
    let reason = reason
        .unwrap_or("operator requested provider handoff")
        .trim();
    if reason.is_empty() {
        return Err(child_error("handoff reason cannot be empty"));
    }
    Ok(ChildBodyHandoffRequest {
        agent,
        provider: provider.to_string(),
        reason: reason.to_string(),
    })
}

pub(crate) async fn queue_command(
    store: &SharedStore,
    mut session: ChildSession,
    source: ChildCommandSource,
    kind: ChildCommandKind,
) -> OpsResult<ChildControlResult> {
    if session.is_terminal() {
        return Err(child_error(format!(
            "{} is {}; terminal Sessions cannot accept commands",
            session.label(),
            session.status()
        )));
    }
    // Abandonment is decided the moment it is requested. Between that moment and
    // the runner consuming the command, the Session must accept nothing else —
    // otherwise a steer arriving in the gap relaunches work someone just ended.
    if !matches!(kind, ChildCommandKind::Abandon { .. }) {
        if let Some(bar) = session.abandon_intent_reason() {
            return Err(child_error(bar));
        }
    }

    // Only an operator's explicit resume may restart delivered work.
    let launch_intent = match (&kind, &source) {
        (ChildCommandKind::Resume { .. }, ChildCommandSource::Human) => {
            LaunchIntent::ExplicitResume
        }
        _ => LaunchIntent::Supervisor,
    };

    let command = ChildCommand::new(session.target(), source, kind);
    let wait_for_resolution = !matches!(&command.kind, ChildCommandKind::FollowUp { .. });
    let replacement = match &command.kind {
        ChildCommandKind::Steer { text } => Some(text.clone()),
        ChildCommandKind::Interrupt {
            replacement: Some(text),
        } => Some(text.clone()),
        _ => None,
    };

    let (command, created, superseded, directive) = if let Some(text) = replacement {
        let directive = ChildDirective::replacement(
            session.target(),
            session.current_directive_version() + 1,
            text,
            command.source.clone(),
            command.id.clone(),
        );
        let superseded = store
            .create_child_command_with_directive(&command, &directive)
            .await
            .map_err(child_error)?;
        session.set_current_directive_version(directive.version);
        (command, true, superseded, Some(directive))
    } else if matches!(&command.kind, ChildCommandKind::Decide { .. }) {
        let (command, created) = store
            .ensure_child_decision_command(&command)
            .await
            .map_err(child_error)?;
        (command, created, Vec::new(), None)
    } else if matches!(&command.kind, ChildCommandKind::Interrupt { .. }) {
        let superseded = store
            .supersede_and_create_child_command(&command)
            .await
            .map_err(child_error)?;
        (command, true, superseded, None)
    } else if let ChildCommandKind::Abandon { reason } = &command.kind {
        // The intent lands with the command, in one transaction. From here on
        // every launch path reads it and refuses to start a process.
        let intent = AbandonIntent {
            requested_at: time::OffsetDateTime::now_utc(),
            reason: reason.clone(),
        };
        store
            .create_child_abandon_command(&command, &intent)
            .await
            .map_err(child_error)?;
        session.record_abandon_intent(intent);
        (command, true, Vec::new(), None)
    } else {
        store
            .create_child_command(&command)
            .await
            .map_err(child_error)?;
        (command, true, Vec::new(), None)
    };

    if !created {
        if !command.state.is_terminal() && !session.is_process_active() {
            session.launch(store, launch_intent).await?;
        }
        let receipt = resolve_receipt(store, &command.id, wait_for_resolution).await?;
        return control_result(store, &command, receipt).await;
    }

    for command_id in superseded {
        session
            .append_command_event(store, command_id, ChildCommandState::Superseded, None)
            .await?;
    }
    if let Some(directive) = &directive {
        session.append_directive_event(store, directive).await?;
    }
    session
        .append_command_event(
            store,
            command.id.clone(),
            ChildCommandState::Persisted,
            command.effect,
        )
        .await?;

    if let ChildCommandKind::Abandon { reason } = &command.kind {
        if !session.is_process_active() {
            store
                .accept_child_command(&command.id, None)
                .await
                .map_err(child_error)?;
            session
                .append_command_event(store, command.id.clone(), ChildCommandState::Accepted, None)
                .await?;
            session.abandon(store, reason).await?;
            let receipt = read_receipt(store, &command.id).await?;
            return control_result(store, &command, receipt).await;
        }
    }

    // Interrupt never starts a process. It ends the current turn; it does not
    // begin one. Interrupting an inactive Session used to relaunch it whenever a
    // replacement rode along — the exact path that respawned a Session under
    // whatever binary the interrupting shell happened to be using. The
    // replacement lands as the pending directive, and `resume` is the one verb
    // that starts a process.
    if matches!(&command.kind, ChildCommandKind::Interrupt { .. }) && !session.is_process_active() {
        store
            .accept_child_command(&command.id, None)
            .await
            .map_err(child_error)?;
        session
            .append_command_event(store, command.id.clone(), ChildCommandState::Accepted, None)
            .await?;
        session.record_interrupt_of_inactive_process(store).await?;
        let receipt = read_receipt(store, &command.id).await?;
        return control_result(store, &command, receipt).await;
    }

    if !session.is_process_active() {
        session.launch(store, launch_intent).await?;
    }
    let mut receipt = resolve_receipt(store, &command.id, wait_for_resolution).await?;
    if matches!(
        receipt.state,
        ChildCommandState::Persisted | ChildCommandState::Claimed | ChildCommandState::Delivering
    ) {
        session.refresh(store).await?;
        if !session.is_process_active() && !session.is_terminal() {
            session.launch(store, launch_intent).await?;
            receipt = resolve_receipt(store, &command.id, wait_for_resolution).await?;
        }
    }
    control_result(store, &command, receipt).await
}

pub(crate) async fn control_result(
    store: &SharedStore,
    command: &ChildCommand,
    receipt: ChildCommand,
) -> OpsResult<ChildControlResult> {
    let directive = store
        .child_directive_for_command(&command.id)
        .await
        .map_err(child_error)?;
    Ok(ChildControlResult {
        session_id: command.target.target_id().to_string(),
        command_id: command.id.to_string(),
        directive_version: directive.as_ref().map(|directive| directive.version),
        state: receipt.state,
        effect: receipt.effect,
        incorporated: directive
            .as_ref()
            .is_some_and(|directive| directive.incorporated_at.is_some()),
        generation: receipt.claimed_by_generation,
        accepted_at: receipt.accepted_at,
        incorporated_at: directive.and_then(|directive| directive.incorporated_at),
        error: receipt.error,
    })
}

async fn resolve_receipt(
    store: &SharedStore,
    command_id: &ChildCommandId,
    wait: bool,
) -> OpsResult<ChildCommand> {
    if wait {
        Ok(wait_for_receipt(store, command_id, Duration::from_secs(2))
            .await?
            .0)
    } else {
        read_receipt(store, command_id).await
    }
}

pub(crate) async fn read_receipt(
    store: &SharedStore,
    command_id: &ChildCommandId,
) -> OpsResult<ChildCommand> {
    store
        .get_child_command(command_id)
        .await
        .map_err(child_error)?
        .ok_or_else(|| child_error(format!("child command {command_id} disappeared")))
}

pub(crate) async fn wait_for_receipt(
    store: &SharedStore,
    command_id: &ChildCommandId,
    timeout: Duration,
) -> OpsResult<(ChildCommand, bool)> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let command = read_receipt(store, command_id).await?;
        if command.state.is_terminal() {
            return Ok((command, false));
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok((command, true));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

pub(crate) async fn wait_for_receipt_condition(
    store: &SharedStore,
    command_id: &ChildCommandId,
    until: ChildReceiptUntil,
    timeout: Duration,
) -> OpsResult<(ChildCommand, bool)> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let command = read_receipt(store, command_id).await?;
        if matches!(
            command.state,
            ChildCommandState::Failed
                | ChildCommandState::Superseded
                | ChildCommandState::Uncertain
        ) {
            return Ok((command, false));
        }
        let settled = match until {
            ChildReceiptUntil::Applied => command.state.is_terminal(),
            ChildReceiptUntil::Incorporated => store
                .child_directive_for_command(command_id)
                .await
                .map_err(child_error)?
                .ok_or_else(|| {
                    child_error(format!(
                        "child command {command_id} does not carry a directive to incorporate"
                    ))
                })?
                .incorporated_at
                .is_some(),
        };
        if settled {
            return Ok((command, false));
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok((command, true));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn child_error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::os::unix::process::CommandExt;
    use std::sync::Arc;

    use time::OffsetDateTime;

    use crate::child_session::{
        ChildBodyOutcome, ChildCommandKind, ChildCommandSource, ChildCommandState, ChildLeaseState,
        ChildProcessGeneration, ChildRef,
    };
    use crate::id::WaveId;
    use crate::project_session::{
        ProjectEventKind, ProjectSession, ProjectSessionId, ProjectSessionStatus,
    };
    use crate::session_context::{
        LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot,
        ProjectLaunchReceipt, TaskLaunchReceipt,
    };
    use crate::store::{open_store, StorageConfig};
    use crate::task::{GithubPr, PrPublication, TaskPr, TaskPrId, TaskSessionStatus};
    use crate::wave::Wave;

    use super::{
        child_body_reservation_is_fresh, handoff_request, queue_command, resume_session,
        revoke_and_reap_child_body, ChildSession,
    };

    #[test]
    fn a_fresh_reservation_is_adopted_while_a_stale_one_can_be_reaped() {
        let mut process = ChildProcessGeneration {
            generation: 1,
            pid: None,
            process_group_id: None,
            tmux_name: "starting".to_string(),
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            started_at: OffsetDateTime::now_utc(),
            state: crate::child_session::ChildLeaseState::Reserved,
            outcome: None,
            provenance: None,
        };
        assert!(child_body_reservation_is_fresh(&process));
        process.started_at -= time::Duration::seconds(11);
        assert!(!child_body_reservation_is_fresh(&process));
    }

    #[tokio::test]
    async fn recovery_reaps_the_old_process_group_before_reserving_its_successor() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = make_wave(dir.path().to_str().unwrap());
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave, ProjectSessionStatus::Running);
        store.create_project_session(&project).await.unwrap();
        let mut task = make_task(&wave, &project, TaskSessionStatus::Waiting);
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();
        task.begin_generation("recovery-body".to_string());
        let lease = store
            .reserve_task_process(&task, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .expect("reserve generation one");

        let mut child = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("sleep 60 & echo $!; wait")
            .process_group(0)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("spawn isolated body process group");
        let group = child.id();
        let stdout = child.stdout.take().expect("capture grandchild pid");
        let mut line = String::new();
        BufReader::new(stdout)
            .read_line(&mut line)
            .expect("read grandchild pid");
        let grandchild: u32 = line.trim().parse().expect("grandchild pid");
        let waiter = std::thread::spawn(move || child.wait().expect("reap shell"));

        let process = task.latest_process.as_mut().expect("reserved process");
        process.pid = Some(group);
        process.process_group_id = Some(group);
        process.state = ChildLeaseState::Active;
        task.set_status(TaskSessionStatus::Running, "fake provider is alive");
        store.activate_task_process(&task, &lease).await.unwrap();

        let finished = revoke_and_reap_child_body(
            &store,
            &ChildRef::Task(task.id.clone()),
            ChildBodyOutcome::Superseded {
                reason: "deterministic recovery".to_string(),
            },
        )
        .await
        .unwrap();
        waiter.join().unwrap();
        assert_eq!(finished.state, ChildLeaseState::Finished);
        // SAFETY: signal 0 is an existence probe and uses no pointers.
        assert_ne!(unsafe { libc::kill(group as i32, 0) }, 0);
        // SAFETY: signal 0 is an existence probe and uses no pointers.
        assert_ne!(unsafe { libc::kill(grandchild as i32, 0) }, 0);

        let mut stopped = store.get_task_session(&task.id).await.unwrap().unwrap();
        stopped.set_status(TaskSessionStatus::Waiting, "old body fully reaped");
        store.update_task_session(&stopped).await.unwrap();
        let mut successor = store.get_task_session(&task.id).await.unwrap().unwrap();
        assert_eq!(
            successor.begin_generation("recovery-successor".to_string()),
            2
        );
        assert!(store
            .reserve_task_process(&successor, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .is_some());
    }

    fn make_wave(repo: &str) -> Wave {
        let id = WaveId::new();
        Wave::new(id.clone(), format!("wave-{id}"), repo.to_string())
    }

    fn make_project(wave: &Wave, status: ProjectSessionStatus) -> ProjectSession {
        let now = OffsetDateTime::now_utc();
        let active = status.is_process_active();
        ProjectSession {
            id: ProjectSessionId::new(),
            launch: ProjectLaunchReceipt {
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new(format!("project-{}", WaveId::new())).unwrap(),
                    slug: format!("project-{}", WaveId::new()),
                    name: "Child control".to_string(),
                    prompt_context: "Keep one control protocol.".to_string(),
                },
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status,
            status_reason: "test project session".to_string(),
            status_at: now,
            iteration: 1,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: active.then(|| "thread-project".to_string()),
            latest_process: active.then_some(ChildProcessGeneration {
                generation: 1,
                pid: None,
                process_group_id: None,
                tmux_name: "lf-project-test".to_string(),
                agent: "codex".to_string(),
                provider: "codex".to_string(),
                provider_session_id: active.then(|| "thread-project".to_string()),
                started_at: now,
                state: crate::child_session::ChildLeaseState::Active,
                outcome: None,
                provenance: None,
            }),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn make_task(
        wave: &Wave,
        project: &ProjectSession,
        status: TaskSessionStatus,
    ) -> crate::task::TaskSession {
        let now = OffsetDateTime::now_utc();
        let active = status.is_process_active();
        let id = WaveId::new();
        crate::task::TaskSession {
            id: crate::task::TaskSessionId::new(),
            launch: TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new(format!("issue-{id}")).unwrap(),
                    identifier: "W2-129".to_string(),
                    title: "Terminal intent dominates process liveness".to_string(),
                    description: "Delivered work must not restart itself.".to_string(),
                },
                project: project.launch.project.clone(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: crate::task::PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_session_id: project.id.clone(),
            current_directive_version: 1,
            incorporated_directive_version: 0,
            status,
            status_reason: "test task session".to_string(),
            status_at: now,
            worktree: std::path::PathBuf::from(format!("/tmp/loopflow.{id}")),
            workspace_slug: format!("test-{id}"),
            lifecycle: crate::task::TaskLifecyclePlan::standard("task"),
            lifecycle_phase: crate::task::TaskLifecyclePhase::Iterate,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "claude".to_string(),
            provider: "claude".to_string(),
            provider_session_id: active.then(|| "thread-task".to_string()),
            observation: crate::task::Observation::NotRequired,
            latest_process: active.then_some(ChildProcessGeneration {
                generation: 1,
                pid: None,
                process_group_id: None,
                tmux_name: "lf-task-test".to_string(),
                agent: "claude".to_string(),
                provider: "claude".to_string(),
                provider_session_id: active.then(|| "thread-task".to_string()),
                started_at: now,
                state: crate::child_session::ChildLeaseState::Active,
                outcome: None,
                provenance: None,
            }),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn make_task_pr(task: &crate::task::TaskSession) -> TaskPr {
        TaskPr {
            id: TaskPrId::new(),
            task_session_id: task.id.clone(),
            sequence: 1,
            slug: task.workspace_slug.clone(),
            branch: format!("test/{}", task.workspace_slug),
            base_commit: "0".repeat(40),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: task.created_at,
            updated_at: task.updated_at,
        }
    }

    #[tokio::test]
    async fn project_follow_up_returns_once_durable() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = make_wave(dir.path().to_str().unwrap());
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave, ProjectSessionStatus::Running);
        store.create_project_session(&project).await.unwrap();

        let result = queue_command(
            &store,
            ChildSession::Project(Box::new(project)),
            ChildCommandSource::Human,
            ChildCommandKind::FollowUp {
                text: "Inspect the boundary next".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(result.state, ChildCommandState::Persisted);
        assert_eq!(result.accepted_at, None);
    }

    #[tokio::test]
    async fn inactive_project_abandonment_does_not_launch_a_process() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = make_wave(dir.path().to_str().unwrap());
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave, ProjectSessionStatus::Created);
        let project_id = project.id.clone();
        store.create_project_session(&project).await.unwrap();

        let result = queue_command(
            &store,
            ChildSession::Project(Box::new(project)),
            ChildCommandSource::Human,
            ChildCommandKind::Abandon {
                reason: "The measured bet no longer matters".to_string(),
            },
        )
        .await
        .unwrap();
        let persisted = store
            .get_project_session(&project_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(result.state, ChildCommandState::Accepted);
        assert_eq!(persisted.status, ProjectSessionStatus::Abandoned);
        assert_eq!(persisted.latest_process, None);
    }

    #[tokio::test]
    async fn interrupting_an_inactive_project_does_not_launch_a_process() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = make_wave(dir.path().to_str().unwrap());
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave, ProjectSessionStatus::Waiting);
        let project_id = project.id.clone();
        store.create_project_session(&project).await.unwrap();

        let result = queue_command(
            &store,
            ChildSession::Project(Box::new(project)),
            ChildCommandSource::Human,
            ChildCommandKind::Interrupt { replacement: None },
        )
        .await
        .unwrap();
        let persisted = store
            .get_project_session(&project_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(result.state, ChildCommandState::Accepted);
        assert_eq!(persisted.status, ProjectSessionStatus::Waiting);
        assert_eq!(persisted.latest_process, None);
    }

    /// The 2026-07-14 W2-129 sequence, preserved as a regression.
    ///
    /// Task Session `ts_c33d8dc7…` emitted `pull_request_opened` for #878 and went
    /// `submitted` with no live process. Supervision then launched generation 2
    /// and drove it back to `running` at `task_clarify` — re-deriving a design
    /// whose PR was already open for review, because `Open` is neither
    /// terminal nor process-active and so reads exactly like a Session that
    /// merely stopped.
    ///
    /// Terminal intent dominates process liveness: delivered work is not a
    /// restart candidate, whatever the liveness fields say.
    #[tokio::test]
    async fn a_submitted_task_with_an_open_pr_is_never_restarted_by_supervision() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = make_wave(dir.path().to_str().unwrap());
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave, ProjectSessionStatus::Running);
        store.create_project_session(&project).await.unwrap();

        let task = make_task(&wave, &project, TaskSessionStatus::Waiting);
        let mut pr = make_task_pr(&task);
        store.create_task_session(&task, &pr).await.unwrap();
        pr.publication = Some(PrPublication {
            requested_at: pr.updated_at,
            after_merge: crate::task::AfterMerge::Review,
            next_slug: None,
            github: Some(GithubPr {
                number: 878,
                url: "https://github.com/loopflow/loopflow/pull/878".to_string(),
                head_sha: None,
            }),
        });
        store.update_task_pr(&pr).await.unwrap();
        let task_id = task.id.clone();

        // The supervisor's steer: exactly the path the Project wake took.
        let error = queue_command(
            &store,
            ChildSession::Task(Box::new(task)),
            ChildCommandSource::Project(project.id.clone()),
            ChildCommandKind::Steer {
                text: "Keep going on the design".to_string(),
            },
        )
        .await
        .expect_err("supervision must not restart a submitted Task");

        assert!(
            error.to_string().contains("#878"),
            "the refusal should name the open PR, got: {error}"
        );

        // No generation 2, and the Session still reads as delivered.
        let persisted = store.get_task_session(&task_id).await.unwrap().unwrap();
        assert_eq!(persisted.status, TaskSessionStatus::Waiting);
        assert_eq!(persisted.latest_process, None);
    }

    #[tokio::test]
    async fn ci_fix_wake_refuses_an_open_pr_without_a_warranted_failure() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = make_wave(dir.path().to_str().unwrap());
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave, ProjectSessionStatus::Running);
        store.create_project_session(&project).await.unwrap();

        let mut task = make_task(&wave, &project, TaskSessionStatus::Waiting);
        let mut pr = make_task_pr(&task);
        store.create_task_session(&task, &pr).await.unwrap();
        // Open PR, head observed green: no failure warrants a ci-fix wake.
        pr.publication = Some(PrPublication {
            requested_at: pr.updated_at,
            after_merge: crate::task::AfterMerge::Review,
            next_slug: None,
            github: Some(GithubPr {
                number: 878,
                url: "https://github.com/loopflow/loopflow/pull/878".to_string(),
                head_sha: Some("h1".to_string()),
            }),
        });
        pr.ci_observation = Some(crate::task::CiObservation {
            head_sha: "h1".to_string(),
            state: crate::task::CiState::Passing,
            failing_checks: vec![],
            observed_at: pr.updated_at,
            woken_failure_set: None,
        });
        store.update_task_pr(&pr).await.unwrap();

        let error = super::wake_task_ci_fix(&store, &mut task)
            .await
            .expect_err("a green open PR must not wake a ci-fix turn");
        assert!(
            error.to_string().contains("#878"),
            "the refusal names the open PR, got: {error}"
        );
        let persisted = store.get_task_session(&task.id).await.unwrap().unwrap();
        assert_eq!(persisted.status, TaskSessionStatus::Waiting);
        assert_eq!(persisted.latest_process, None);
    }

    /// A human may still answer review on a submitted Task. The bar is on the
    /// supervisor, not the operator — otherwise delivered work becomes unreachable.
    #[tokio::test]
    async fn an_operator_may_still_resume_a_submitted_task_to_answer_review() {
        let (task, pr) = {
            let wave = make_wave("/tmp");
            let project = make_project(&wave, ProjectSessionStatus::Running);
            let task = make_task(&wave, &project, TaskSessionStatus::Waiting);
            let mut pr = make_task_pr(&task);
            pr.publication = Some(PrPublication {
                requested_at: pr.updated_at,
                after_merge: crate::task::AfterMerge::Review,
                next_slug: None,
                github: Some(GithubPr {
                    number: 878,
                    url: "https://github.com/loopflow/loopflow/pull/878".to_string(),
                    head_sha: None,
                }),
            });
            (task, pr)
        };

        // The supervisor is barred...
        assert!(task.supervisor_restart_bar(Some(&pr)).is_some());
        // ...but nothing about the Session itself is terminal, so an explicit
        // `lf task resume` still has a Session to resume.
        assert!(!task.status.is_terminal());
        assert!(task.abandon_intent.is_none());
    }

    #[tokio::test]
    async fn project_handoff_reuses_compatible_history_and_records_the_transition() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = make_wave(dir.path().to_str().unwrap());
        store.create_wave(&wave).await.unwrap();
        let mut project = make_project(&wave, ProjectSessionStatus::Waiting);
        project.provider_session_id = Some("claude-thread".to_string());
        project.latest_process = Some(ChildProcessGeneration {
            generation: 4,
            pid: None,
            process_group_id: None,
            tmux_name: "lf-project-old".to_string(),
            agent: "claude:sonnet".to_string(),
            provider: "claude".to_string(),
            provider_session_id: Some("claude-thread".to_string()),
            started_at: project.updated_at,
            state: crate::child_session::ChildLeaseState::Finished,
            outcome: Some(crate::child_session::ChildBodyOutcome::LegacyStopped {
                reason: "test body stopped".to_string(),
            }),
            provenance: None,
        });
        project.agent = "claude:sonnet".to_string();
        project.provider = "claude".to_string();
        let project_id = project.id.clone();
        store.create_project_session(&project).await.unwrap();

        let request = handoff_request("claude:opus", Some("use the larger context window"))
            .expect("valid same-provider handoff");
        let mut child = ChildSession::Project(Box::new(project));
        child.handoff(&store, &request).await.unwrap();

        let persisted = store
            .get_project_session(&project_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(persisted.agent, "claude:opus");
        assert_eq!(persisted.provider, "claude");
        assert_eq!(
            persisted.provider_session_id.as_deref(),
            Some("claude-thread")
        );
        let events = store.project_events_after(&project_id, 0).await.unwrap();
        assert!(matches!(
            events.last().map(|event| &event.kind),
            Some(ProjectEventKind::BodyHandedOff { handoff })
                if handoff.from_agent == "claude:sonnet"
                    && handoff.to_agent == "claude:opus"
                    && handoff.reason == "use the larger context window"
        ));
    }

    #[tokio::test]
    async fn live_task_writer_rejects_provider_handoff_without_mutation() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = make_wave(dir.path().to_str().unwrap());
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave, ProjectSessionStatus::Running);
        store.create_project_session(&project).await.unwrap();
        let task = make_task(&wave, &project, TaskSessionStatus::Running);
        let task_id = task.id.clone();
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();

        let request = handoff_request("codex", Some("Claude quota exhausted")).unwrap();
        let error = store
            .handoff_task_body(&task_id, &request)
            .await
            .expect_err("a live writer must be interrupted before handoff");
        assert!(error.to_string().contains("active writer"));

        let persisted = store.get_task_session(&task_id).await.unwrap().unwrap();
        assert_eq!(persisted.agent, "claude");
        assert_eq!(
            persisted.provider_session_id.as_deref(),
            Some("thread-task")
        );
        assert!(store
            .task_events_after(&task_id, 0)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn task_handoff_preserves_an_open_pr_and_its_supervisor_bar() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = make_wave(dir.path().to_str().unwrap());
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave, ProjectSessionStatus::Running);
        store.create_project_session(&project).await.unwrap();
        let mut task = make_task(&wave, &project, TaskSessionStatus::Waiting);
        task.provider_session_id = Some("claude-review-thread".to_string());
        let mut pr = make_task_pr(&task);
        let task_id = task.id.clone();
        let pr_id = pr.id.clone();
        store.create_task_session(&task, &pr).await.unwrap();
        pr.publication = Some(PrPublication {
            requested_at: pr.updated_at,
            after_merge: crate::task::AfterMerge::Review,
            next_slug: None,
            github: Some(GithubPr {
                number: 898,
                url: "https://github.com/loopflow/loopflow/pull/898".to_string(),
                head_sha: None,
            }),
        });
        store.update_task_pr(&pr).await.unwrap();

        let error = resume_session(
            &store,
            ChildSession::Task(Box::new(task)),
            ChildCommandSource::Project(project.id.clone()),
            None,
            Some("codex".to_string()),
            Some("supervisor tried to answer review".to_string()),
        )
        .await
        .expect_err("supervision must not hand off an open PR");
        assert!(error.to_string().contains("#898"));
        assert_eq!(
            store
                .get_task_session(&task_id)
                .await
                .unwrap()
                .unwrap()
                .agent,
            "claude"
        );

        let request = handoff_request("codex", Some("answer review on Codex")).unwrap();
        let persisted = store.handoff_task_body(&task_id, &request).await.unwrap();
        let active_pr = store.active_task_pr(&task_id).await.unwrap().unwrap();

        assert_eq!(active_pr.id, pr_id);
        assert!(persisted.supervisor_restart_bar(Some(&active_pr)).is_some());
        assert_eq!(persisted.agent, "codex");
        assert_eq!(persisted.provider_session_id, None);
    }

    #[tokio::test]
    async fn terminal_task_rejects_handoff() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = make_wave(dir.path().to_str().unwrap());
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave, ProjectSessionStatus::Running);
        store.create_project_session(&project).await.unwrap();
        let task = make_task(&wave, &project, TaskSessionStatus::Completed);
        let task_id = task.id.clone();
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();

        let request = handoff_request("codex", Some("should not restart")).unwrap();
        let error = store
            .handoff_task_body(&task_id, &request)
            .await
            .expect_err("terminal Sessions never hand off");
        assert!(error.to_string().contains("terminal Sessions"));
    }

    /// Abandonment is decided when it is *queued*, not when a runner consumes it.
    /// Until this landed, the intent lived only in the command row while the
    /// Session still read `Running` — so anything that launched in that window
    /// revived work someone had already ended.
    #[tokio::test]
    async fn queueing_abandon_stamps_intent_and_bars_every_later_command() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = make_wave(dir.path().to_str().unwrap());
        store.create_wave(&wave).await.unwrap();
        // Active: the abandon command stays pending rather than being applied,
        // which is exactly the window the race used to live in.
        let project = make_project(&wave, ProjectSessionStatus::Running);
        let project_id = project.id.clone();
        store.create_project_session(&project).await.unwrap();

        queue_command(
            &store,
            ChildSession::Project(Box::new(project)),
            ChildCommandSource::Human,
            ChildCommandKind::Abandon {
                reason: "Superseded by a different bet".to_string(),
            },
        )
        .await
        .unwrap();

        let persisted = store
            .get_project_session(&project_id)
            .await
            .unwrap()
            .unwrap();
        // The runner has not consumed the command, so the Session is not yet
        // Abandoned — but the intent is durable and already dominates.
        assert_eq!(persisted.status, ProjectSessionStatus::Running);
        let intent = persisted
            .abandon_intent
            .as_ref()
            .expect("abandon intent is stamped when the command is queued");
        assert_eq!(intent.reason, "Superseded by a different bet");
        assert!(persisted.supervisor_restart_bar().is_some());

        let error = queue_command(
            &store,
            ChildSession::Project(Box::new(persisted)),
            ChildCommandSource::Human,
            ChildCommandKind::FollowUp {
                text: "One more thing".to_string(),
            },
        )
        .await
        .expect_err("a Session being abandoned accepts nothing else");
        assert!(
            error.to_string().contains("being abandoned"),
            "got: {error}"
        );
    }
}
