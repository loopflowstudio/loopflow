//! Shared recovery and execution helpers for Project and Task Work.

use std::time::Duration;

use crate::child_session::{
    ChildBodyHandoffRequest, ChildBodyOutcome, ChildProcessGeneration, ChildRef,
};
use crate::durable::{
    AuthenticatedRequest, ControlCtx, EpochReceipt, InterruptReceipt, Run, RunLease, RunLeaseToken,
    SteerReceipt, RUN_CONTEXT_ENV, RUN_LEASE_ENV,
};
use crate::project_session::ProjectSession;
use crate::store::{SharedStore, Store};
use crate::task::TaskSession;

use super::{OpsError, OpsResult};

const CHILD_STARTUP_GRACE_SECONDS: i64 = 10;
pub(crate) const CHILD_STARTUP_GRACE: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkControlReceipt {
    Steer { receipt: SteerReceipt },
    Interrupt { receipt: InterruptReceipt },
    Resume { run: Run },
    Abandon { receipt: EpochReceipt },
}

impl WorkControlReceipt {
    pub fn label(&self) -> String {
        match self {
            Self::Steer { receipt } => receipt.steer.id.to_string(),
            Self::Interrupt { receipt } => receipt.run_id.to_string(),
            Self::Resume { run } => run.id.to_string(),
            Self::Abandon { receipt } => receipt.epoch.id.to_string(),
        }
    }

    pub fn action(&self) -> &'static str {
        match self {
            Self::Steer { .. } => "steered",
            Self::Interrupt { .. } => "interrupted",
            Self::Resume { .. } => "resumed",
            Self::Abandon { .. } => "abandoned",
        }
    }
}

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
    if let Err(error) = crate::engine::process::reap_child_process(&revoked, Duration::from_secs(2))
        .await
        .map_err(child_error)
    {
        // The kill failed, and this used to return here — leaving the lease at
        // `revoked` forever, because nothing ever re-examines it and the reserve
        // CAS accepts only NULL or `finished`. But a lease exists to bar a second
        // body, and a body we can prove is gone cannot run anything, so it has no
        // claim on the lease whatever happened during the reap. Release on proof;
        // anything short of proof keeps the lease and surfaces the real failure.
        // ("refusing to reap current process group" self-classifies as `Present`:
        // that group is us.)
        return match release_dead_revoked_child_body(store, target, &revoked).await? {
            Some(finished) => Ok(finished),
            None => Err(error),
        };
    }
    finish_revoked_child_body(store, target, revoked.generation).await
}

/// Release a lease stuck at `revoked` whose body is provably gone.
///
/// `Some(finished)` only when the body probes [`Presence::Absent`]; `None` when
/// it is present or unprovable, which leaves the lease exactly as it was.
///
/// This never re-signals. A stuck lease is old by construction and a pid is a
/// recycled resource, so retrying the kill would mean sending TERM to whatever
/// inherited the number — the release path would become a way to end a
/// stranger's work. And if the body is provably gone there is nothing left to
/// kill anyway.
///
/// The store's CAS pins both the generation and `process_lease_state='revoked'`,
/// so only the same generation still awaiting reap can become `finished`.
pub(crate) async fn release_dead_revoked_child_body(
    store: &SharedStore,
    target: &ChildRef,
    revoked: &ChildProcessGeneration,
) -> OpsResult<Option<ChildProcessGeneration>> {
    if revoked.state != crate::child_session::ChildLeaseState::Revoked {
        return Ok(None);
    }
    if crate::engine::process::probe_child_body_presence(revoked).await
        != crate::engine::process::Presence::Absent
    {
        return Ok(None);
    }
    finish_revoked_child_body(store, target, revoked.generation)
        .await
        .map(Some)
}

async fn finish_revoked_child_body(
    store: &SharedStore,
    target: &ChildRef,
    generation: u32,
) -> OpsResult<ChildProcessGeneration> {
    match target {
        ChildRef::Project(session_id) => store
            .finish_revoked_project_process(session_id, generation)
            .await
            .map_err(child_error),
        ChildRef::Task(session_id) => store
            .finish_revoked_task_process(session_id, generation)
            .await
            .map_err(child_error),
    }
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
            LaunchIntent::ExplicitResume => self.abandon_intent_reason(),
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
}

/// Who is asking for a process, which decides what state may refuse them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LaunchIntent {
    ExplicitResume,
    /// Automatic recovery of a Session whose body died while its status still
    /// claimed one. Barred only by terminal and abandoning work; the strand
    /// predicate upstream (`plan_stranded_recovery`) is what keeps it off
    /// delivered work.
    Recovery,
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
    model: Option<String>,
    reason: Option<String>,
) -> OpsResult<Run> {
    if let Some(model) = model {
        let request = handoff_request(&model, reason.as_deref())?;
        if session.agent() != request.agent {
            session.handoff(store, &request).await?;
        }
    }
    if !session.is_process_active() {
        session.launch(store, LaunchIntent::ExplicitResume).await?;
    }
    let work = store
        .work_for_child(&session.target())
        .await
        .map_err(child_error)?;
    store
        .current_run(&work)
        .await
        .map_err(child_error)?
        .ok_or_else(|| {
            child_error(format!(
                "{} has no active Run after resume",
                session.label()
            ))
        })
}

pub(crate) async fn append_steer(
    store: &SharedStore,
    target: ChildRef,
    text: &str,
) -> OpsResult<SteerReceipt> {
    let work = store.work_for_child(&target).await.map_err(child_error)?;
    if let Some(lease) = ambient_run_lease(store).await? {
        store
            .steer(&ControlCtx::Run(&lease), &work, text, None)
            .await
            .map_err(child_error)
    } else {
        let request = AuthenticatedRequest::cli();
        store
            .steer(&ControlCtx::User(&request), &work, text, None)
            .await
            .map_err(child_error)
    }
}

pub(crate) async fn ambient_run_lease(store: &Store) -> OpsResult<Option<RunLease>> {
    if let Some(value) = std::env::var_os(RUN_LEASE_ENV) {
        let value = value
            .into_string()
            .map_err(|_| child_error("LF_RUN_LEASE is not valid UTF-8"))?;
        let token =
            RunLeaseToken::parse(&value).map_err(|_| child_error("LF_RUN_LEASE is malformed"))?;
        return store
            .resolve_run_lease(token)
            .await
            .map(Some)
            .map_err(child_error);
    }

    // `LF_RUN_CONTEXT` is the sole positive marker that this process is an
    // agent body rather than a person at a shell. It matters because User
    // authority is the ambient fallback: `AuthenticatedRequest::cli()` treats
    // local shell presence as the user, which is right for a local-first
    // product but means a body that lost its lease would otherwise inherit
    // full User rights. Every Launch sets this var, so its presence without a
    // resolvable lease is a fenced or stale writer and must fail closed.
    //
    // This deliberately does not consult the legacy Session env vars. They
    // served as this sentinel before Run owned authority; keying on them now
    // would make deleting them a silent privilege escalation.
    if std::env::var_os(RUN_CONTEXT_ENV).is_some() {
        return Err(child_error(
            "in-Run agent process has no usable LF_RUN_LEASE; refusing User authority",
        ));
    }
    Ok(None)
}

pub(crate) async fn required_run_lease(store: &Store) -> OpsResult<RunLease> {
    ambient_run_lease(store)
        .await?
        .ok_or_else(|| child_error("in-Run entrypoint requires LF_RUN_LEASE"))
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

fn child_error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}
