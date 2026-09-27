use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::durable::{FlowPosition, RunId, WorkRef};
use crate::engine::Skill;
use crate::ops::unrecorded_session::{self, Reservation};
use crate::run_record::{
    ProviderSessionRef, RunManifest, SessionName, SessionTitleSource, RUN_DIR_ENV,
};
use crate::session::{Run, Session, WorkSource};
use crate::store::SharedStore;
use crate::work::task::{Task, TaskId};

#[cfg(test)]
pub(crate) mod action_test;

pub(crate) const HUMAN_SESSION_ENV: &str = "LF_HUMAN_SESSION";
pub(crate) const PREPARED_RUN_ENV: &str = "LF_HUMAN_SESSION_RUN";
const REVIEW_RUN_ENV: &str = "LF_REVIEW_RUN_RESERVATION";

#[derive(Debug, Serialize, Deserialize)]
struct ReviewRunReservation {
    session_id: String,
    run_id: RunId,
    version: u64,
}

pub(crate) fn reserved_run() -> Result<Option<(RunId, crate::run_record::RunFlowMembership)>> {
    let Some(raw) = std::env::var_os(REVIEW_RUN_ENV) else {
        return Ok(None);
    };
    let reservation: ReviewRunReservation = serde_json::from_str(&raw.to_string_lossy())?;
    let store =
        crate::store::sqlite::SqliteStore::new(&crate::store::observability_database_path()?)?;
    let (session, run) = store
        .session(&reservation.session_id)?
        .ok_or_else(|| anyhow!("review reservation disappeared"))?;
    let task_id = run
        .task_id
        .as_ref()
        .ok_or_else(|| anyhow!("review reservation has no Task"))?;
    let position = store
        .flow_position(task_id)?
        .ok_or_else(|| anyhow!("review invocation is no longer current"))?;
    if session.current_run_id != reservation.run_id
        || session.completed_at.is_some()
        || run.published
        || position.version != reservation.version
        || flow_id(&position)? != session.id
    {
        bail!("review Run reservation is stale");
    }
    let task_pr_id = store.active_task_pr(task_id)?.map(|pr| pr.id);
    Ok(Some((
        reservation.run_id,
        crate::run_record::RunFlowMembership::Step(crate::run_record::RunFlowStep::of(
            &position, task_pr_id,
        )),
    )))
}

/// Launch exclusion is process state beside the Home, not a Session record.
const LAUNCH_LOCK_DIRECTORY: &str = "human-sessions";
const SESSION_START_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FlowSessionToken {
    pub(crate) task_id: TaskId,
    pub(crate) invocation_id: String,
    pub(crate) flow: String,
    pub(crate) node_id: String,
    pub(crate) skill: Skill,
    pub(crate) iteration: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum HumanSessionToken {
    Flow {
        token: Box<FlowSessionToken>,
    },
    Ask {
        id: String,
    },
    StandaloneFlow {
        token: crate::ops::flow_run::StepToken,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenMode {
    Refuse,
    Replace,
    Try,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Waiting,
    Active,
    Ready,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Ask,
    Flow,
    Interactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionActionKind {
    Open,
    MoveHere,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAction {
    pub kind: SessionActionKind,
    pub label: String,
    pub help: String,
    pub unavailable_reason: Option<String>,
}

pub(crate) fn session_actions(kind: SessionKind, state: SessionState) -> Vec<SessionAction> {
    use SessionActionKind::{Complete, MoveHere, Open};
    let active_client = kind == SessionKind::Interactive && state == SessionState::Active;
    let not_ready =
        (state != SessionState::Ready).then_some("The session agent has not marked this ready");
    let mut actions = vec![(
        Open,
        "Open here",
        "Open this Session in a terminal",
        active_client
            .then_some("This Session is active in another terminal; use Move here to transfer it"),
    )];
    match kind {
        SessionKind::Interactive => {
            if active_client {
                actions.push((
                    MoveHere,
                    "Move here",
                    "Stop the other client and resume here; unsent text there is lost",
                    None,
                ));
            }
            actions.push((
                Complete,
                "Complete",
                "Stop the provider and remove this Session; native history remains resumable",
                None,
            ));
        }
        SessionKind::Ask => actions.push((
            Complete,
            "Complete",
            "Complete the conversation and resume its blocked caller",
            not_ready,
        )),
        SessionKind::Flow => actions.push((
            Complete,
            "Complete",
            "Complete the review and return feedback to the next Flow step",
            not_ready,
        )),
    }
    actions
        .into_iter()
        .map(|(kind, label, help, reason)| SessionAction {
            kind,
            label: label.to_string(),
            help: help.to_string(),
            unavailable_reason: reason.map(str::to_string),
        })
        .collect()
}

fn require_session_action(
    kind: SessionKind,
    state: SessionState,
    action: SessionActionKind,
) -> Result<()> {
    let projected = session_actions(kind, state)
        .into_iter()
        .find(|item| item.kind == action)
        .ok_or_else(|| anyhow!("This action does not apply to this Session"))?;
    if let Some(reason) = projected.unavailable_reason {
        bail!("{reason}");
    }
    Ok(())
}

fn flow_actions(position: &FlowPosition) -> Vec<SessionAction> {
    let state = if position.ready_summary.is_some() {
        SessionState::Ready
    } else {
        SessionState::Waiting
    };
    session_actions(SessionKind::Flow, state)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub run_id: RunId,
    pub kind: SessionKind,
    pub work: Option<WorkRef>,
    pub wave_id: Option<crate::id::WaveId>,
    pub work_path: Option<String>,
    pub actions: Vec<SessionAction>,
    pub title: String,
    pub title_source: SessionTitleSource,
    pub flow_membership: SessionFlowMembership,
    pub detail: String,
    /// The provider harness recorded on the Session's Run manifest; absent
    /// when that Run is on another Home or its manifest cannot be read.
    pub provider: Option<String>,
    pub cwd: String,
    pub state: SessionState,
    pub ready_summary: Option<String>,
    pub open_argv: Vec<String>,
    pub terminal_ids: Vec<String>,
}

/// Whether a Session's conversation is an occurrence of its Task's managed
/// Flow. Membership comes only from the Flow position or the Run's recorded
/// capture, never from matching Task, checkout, provider, or skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionFlowMembership {
    Step {
        flow: String,
        invocation_id: String,
        step: String,
        /// Exact graph occurrence, unavailable for older capture manifests.
        node: Option<String>,
        iterations: Option<Vec<Vec<u32>>>,
        occurrence: SessionFlowOccurrence,
    },
    Independent,
    Unknown {
        reason: String,
    },
}

/// Where a Flow occurrence sits relative to its Flow's current position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionFlowOccurrence {
    /// The invocation's current position.
    Current,
    /// An earlier position of the invocation that is still active.
    Earlier,
    /// An invocation that has finished or been replaced by a restart.
    Past,
}

impl SessionFlowMembership {
    fn of_position(position: &FlowPosition) -> Self {
        Self::of_step(
            crate::run_record::RunFlowStep::of(position, None),
            SessionFlowOccurrence::Current,
        )
    }

    /// Membership in the exact captured occurrence, placed relative to its Flow.
    pub(crate) fn of_step(
        step: crate::run_record::RunFlowStep,
        occurrence: SessionFlowOccurrence,
    ) -> Self {
        Self::Step {
            flow: step.flow,
            invocation_id: step.invocation_id,
            step: step.step,
            node: step.node,
            iterations: step.iterations,
            occurrence,
        }
    }
}

/// Project a Run's recorded membership, checking whether its Flow has moved on.
async fn run_flow_membership(
    store: &SharedStore,
    manifest: Option<&RunManifest>,
    run_id: &RunId,
) -> SessionFlowMembership {
    let Some(manifest) = manifest else {
        return SessionFlowMembership::Unknown {
            reason: format!("Run {run_id} is not recorded on this Home"),
        };
    };
    match &manifest.flow {
        None => SessionFlowMembership::Unknown {
            reason: format!("Run {run_id} predates recorded Flow membership"),
        },
        Some(crate::run_record::RunFlowMembership::Independent) => {
            SessionFlowMembership::Independent
        }
        Some(crate::run_record::RunFlowMembership::Step(step)) => {
            let occurrence = match &step.task_id {
                Some(task_id) => store
                    .flow_position(task_id)
                    .await
                    .map(|position| step.occurrence(position.as_ref()))
                    .map_err(|error| error.to_string()),
                None => crate::ops::flow_run::read(&step.invocation_id)
                    .map(|run| {
                        if run.finished {
                            SessionFlowOccurrence::Past
                        } else if run.cursor.boundary_key() == step.boundary_key {
                            SessionFlowOccurrence::Current
                        } else {
                            SessionFlowOccurrence::Earlier
                        }
                    })
                    .map_err(|error| error.to_string()),
            };
            match occurrence {
                Ok(occurrence) => SessionFlowMembership::of_step(step.clone(), occurrence),
                Err(error) => SessionFlowMembership::Unknown {
                    reason: format!("Flow {} is unavailable: {error}", step.invocation_id),
                },
            }
        }
    }
}

/// One resolved Session. Every operation looks a Session up by its id, or by
/// its linked Run id, through `find_session`.
#[derive(Debug)]
enum SessionTarget {
    Interactive {
        session: Session,
        run: Run,
    },
    Ask {
        session: Session,
        run: Run,
    },
    Flow {
        task: Box<Task>,
        position: FlowPosition,
        run_id: RunId,
    },
    StandaloneFlow(crate::ops::flow_run::StepToken),
}

pub(crate) async fn ask(
    store: Option<SharedStore>,
    question: &str,
    skill: Option<&str>,
) -> Result<String> {
    let id = format!("ask_{}", uuid::Uuid::new_v4().simple());
    let Reservation { session, run } = reserve_ask(store.as_ref(), id, question, skill).await?;
    if let Err(error) = launch_ask(&session, &run).await {
        // Nothing was stored for a conversation that never opened.
        let _ = unrecorded_session::forget(&run.id);
        return Err(error);
    }
    record_ask(store.as_ref(), &run.id).await;
    report_ask_wait(&session.id);
    wait_for_ask(store, &session.id, &run.id).await
}

/// Reuse one Ask for an exact Flow boundary, including its completed result.
/// The key names the Session, so a retry finds the first request's row.
pub(crate) async fn ask_once(
    store: &SharedStore,
    key: &str,
    question: &str,
    skill: Option<&str>,
) -> Result<String> {
    active_run_manifest()?;
    let id = format!("ask_once_{}", hex::encode(Sha256::digest(key.as_bytes())));
    // The server holds this lock through provider publication. Wait off the
    // async runtime so concurrent callers can still finish their launches.
    let lock_id = id.clone();
    let launch_lock = tokio::task::spawn_blocking(move || lock_session_launch(&lock_id)).await??;
    let (session, run) = match store.session(&id).await? {
        Some(found) => found,
        None => {
            let reserved = reserve_ask(Some(store), id.clone(), question, skill).await?;
            record_ask(Some(store), &reserved.run.id).await;
            (reserved.session, reserved.run)
        }
    };
    if session.completed_at.is_none()
        && run_is_prepared(&run.id)?
        && !ask_launcher_is_running(&id).await?
    {
        // Keep the Session on failure. A retry can start the same Session;
        // a published native Run is reopened only through `lf session open`.
        launch_ask(&session, &run).await.with_context(|| {
            format!("launch human Ask {id}; retry the same boundary to recover")
        })?;
    }
    drop(launch_lock);
    if session.completed_at.is_none() {
        report_ask_wait(&id);
    }
    wait_for_ask(Some(store.clone()), &id, &run.id).await
}

fn report_ask_wait(id: &str) {
    eprintln!(
        "Waiting for human session {id}. Open it in Loopflow or with `lf session open {id}`."
    );
}

/// Prepare an Ask's Session and first Run, kept beside the Run until stored.
/// Work and the caller come from the asking Run; its invocation never does.
async fn reserve_ask(
    store: Option<&SharedStore>,
    id: String,
    question: &str,
    skill: Option<&str>,
) -> Result<Reservation> {
    let question = question.trim();
    if question.is_empty() {
        bail!("question cannot be empty");
    }
    let caller = active_run_manifest()?;
    let cwd = std::env::current_dir().context("resolve Ask working directory")?;
    if let Some(skill) = skill {
        crate::engine::load_skill(skill, &cwd).context("load Ask skill")?;
    }
    let (task_id, wave_id) = match store {
        Some(store) => caller_work(store, &caller).await,
        None => (None, None),
    };
    let run = prepare_ask_run(Run {
        id: caller.run_id.clone(),
        session_id: Some(id.clone()),
        invocation_id: None,
        node: None,
        iterations: None,
        attempt: None,
        work_source: wave_id.as_ref().map(|_| WorkSource::Inherited),
        task_id,
        wave_id,
        created_at: 0,
        published: true,
        cwd,
        skill: skill.map(str::to_string),
        provider: Some(caller.harness),
        model: caller.model,
        caller_run_id: Some(caller.run_id),
    })?;
    let reservation = Reservation {
        session: Session {
            id,
            current_run_id: run.id.clone(),
            kind: crate::session::SessionKind::Ask,
            title: question_title(question),
            title_source: crate::session::TitleSource::Generated,
            request: Some(question.to_string()),
            ready_summary: None,
            completed_at: None,
            created_at: run.created_at,
        },
        run,
    };
    unrecorded_session::defer(&reservation)?;
    Ok(reservation)
}

/// Store the Ask now when the store can take it; otherwise the next
/// operation that touches its Run does.
async fn record_ask(store: Option<&SharedStore>, run_id: &RunId) {
    let recorded = match store {
        Some(store) => unrecorded_session::record(store, run_id).await.map(|_| ()),
        None => Err(anyhow!("the store is unavailable")),
    };
    if let Err(error) = recorded {
        tracing::warn!(%run_id, error = %format!("{error:#}"),
            "Ask Session is not stored yet; it will not list until it is");
    }
}

/// The asking Run's Task and Wave. A headless caller has no row yet, so its
/// launch attribution is its manifest.
async fn caller_work(
    store: &SharedStore,
    caller: &RunManifest,
) -> (Option<TaskId>, Option<crate::id::WaveId>) {
    if let Ok(Some(run)) = store.run(&caller.run_id).await {
        return (run.task_id, run.wave_id);
    }
    match crate::run_record::attributed_work(store, caller).await {
        Some(WorkRef::Task(id)) => match store.get_task(&id).await {
            Ok(Some(task)) => (Some(id), Some(task.wave_id)),
            _ => (None, None),
        },
        Some(WorkRef::Wave(id)) => match store.get_wave(&id).await {
            Ok(Some(_)) => (None, Some(id)),
            _ => (None, None),
        },
        _ => (None, None),
    }
}

/// Prepare another attempt of an Ask; `run` supplies everything but identity.
fn prepare_ask_run(run: Run) -> Result<Run> {
    let id = crate::run_record::CaptureHandle::prepare(
        crate::run_record::RunSpec {
            harness: run.provider.clone().unwrap_or_default(),
            model: run.model.clone(),
            surface: "tui".to_string(),
            cwd: run.cwd.clone(),
            repo: None,
            worktree: Some(run.cwd.clone()),
            skill: None,
            subjects: work_selector(&run)
                .map(crate::run_record::SubjectAttribution::declared)
                .into_iter()
                .collect(),
            // An Ask is a human boundary requested by a Run, never a Flow step.
            flow: crate::run_record::RunFlowMembership::Independent,
        },
        run.caller_run_id.clone(),
    )?;
    Ok(Run {
        id,
        created_at: crate::store::rows::now_unix(),
        ..run
    })
}

pub(crate) async fn prepare(
    store: &SharedStore,
    task: &Task,
    position: &FlowPosition,
) -> Result<SessionRecord> {
    validate_task_position(task, position)?;
    let placement = store.placement(&position.work()).await?;
    let home = store
        .home_by_id(&placement.home_id)
        .await?
        .ok_or_else(|| anyhow!("Task {} Home {} disappeared", task.id, placement.home_id))?;
    if home.route != "local" {
        bail!("session starts on its placed Home; resume the Task there");
    }
    if position.session_run_id.is_none() {
        launch_flow(task, position).await?;
    }
    flow_surface(store, task, position).await
}

pub(crate) async fn list(store: &SharedStore) -> Result<Vec<SessionRecord>> {
    let mut sessions = list_flow_sessions(store).await?;
    for session in crate::ops::flow_session::list()? {
        sessions.push(attribute_standalone_session(store, session).await?);
    }
    for (session, run) in store.open_conversations().await? {
        sessions.push(conversation_surface(store, &session, &run).await?);
    }
    sessions.sort_by(|left, right| left.title.cmp(&right.title).then(left.id.cmp(&right.id)));
    Ok(sessions)
}

/// Resolve a Session id, or the Run id linked to it. An Ask or Flow Run
/// names its waiting boundary, so `$LF_RUN_ID` inside a review targets it.
async fn find_session(store: &SharedStore, session_id: &str) -> Result<Option<SessionTarget>> {
    // Membership outlives a pending boundary. Never reinterpret a retained
    // attempt's manifest as an independent conversation or current actor.
    let owned = match RunId::parse(session_id) {
        Ok(run_id) => session_of_run(store, &run_id).await?,
        Err(_) => store.session(session_id).await?,
    };
    if let Some((session, current)) = owned {
        return owned_target(store, session_id, session, current)
            .await
            .map(Some);
    }
    if let Some(token) = crate::ops::flow_session::parse_id(session_id)? {
        return Ok(Some(SessionTarget::StandaloneFlow(token)));
    }
    if let Some(target) = find_boundary_for_run(session_id)? {
        return Ok(Some(target));
    }
    match crate::run_record::resolve_manifest(&crate::store::observability_home_dir(), session_id) {
        Ok((_, manifest)) => {
            // Prefix selectors also resolve through the canonical Run's owner.
            let Some((session, current)) = session_of_run(store, &manifest.run_id).await? else {
                bail!("Run {} does not belong to a Session", manifest.run_id);
            };
            owned_target(store, manifest.run_id.as_str(), session, current)
                .await
                .map(Some)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(anyhow!("Session record unavailable: {error}")),
    }
}

/// The Session owning `run_id`, recording its waiting reservation first.
async fn session_of_run(store: &SharedStore, run_id: &RunId) -> Result<Option<(Session, Run)>> {
    if let Some(found) = store.session_for_run(run_id).await? {
        return Ok(Some(found));
    }
    unrecorded_session::record(store, run_id).await
}

async fn owned_target(
    store: &SharedStore,
    selector: &str,
    session: crate::session::Session,
    current: crate::session::Run,
) -> Result<SessionTarget> {
    if selector != session.id && selector != current.id.as_str() {
        bail!(
            "Run {selector} is a historical attempt of Session {}; current Run is {}",
            session.id,
            current.id
        );
    }
    match session.kind {
        crate::session::SessionKind::FlowReview => {}
        crate::session::SessionKind::Interactive => {
            if session.completed_at.is_some() {
                bail!("Session {} is already complete", session.id);
            }
            return Ok(SessionTarget::Interactive {
                session,
                run: current,
            });
        }
        crate::session::SessionKind::Ask => {
            if session.completed_at.is_some() {
                bail!("Ask session {:?} is already complete", session.id);
            }
            return Ok(SessionTarget::Ask {
                session,
                run: current,
            });
        }
    }
    let (task, position) = find_flow_session(store, &session.id).await?;
    let (latest, _) = store
        .session(&session.id)
        .await?
        .ok_or_else(|| session_not_found(&session.id))?;
    if latest.current_run_id != current.id
        || (current.published && position.session_run_id.as_ref() != Some(&current.id))
    {
        bail!(
            "Session {} changed its current Run during lookup; open the Session again",
            session.id
        );
    }
    Ok(SessionTarget::Flow {
        task: Box::new(task),
        position,
        run_id: current.id.clone(),
    })
}

/// Where an interactive Run's provider keeps its history and client receipts.
struct NativeRun {
    dir: PathBuf,
    provider: String,
}

impl NativeRun {
    fn of(session: &crate::session::Session, run: &crate::session::Run) -> Result<Self> {
        Ok(Self {
            dir: local_session_run_dir(&run.id)
                .ok_or_else(|| anyhow!("Session {} has an invalid Run reference", session.id))?,
            provider: run
                .provider
                .clone()
                .ok_or_else(|| anyhow!("Session {} Run has no recorded provider", session.id))?,
        })
    }

    fn clients(&self) -> Result<Vec<crate::run_record::ProviderClientRef>> {
        crate::lf::commands::util::active_provider_clients(&self.dir, &self.provider)
    }

    fn history(&self, session: &crate::session::Session) -> Result<ProviderSessionRef> {
        crate::run_record::read_provider_session(&self.dir)?
            .ok_or_else(|| anyhow!("Session {} has no provider history yet", session.id))
    }

    fn stop_clients(&self, reason: crate::run_record::ProviderClientStopReason) -> Result<()> {
        crate::lf::commands::util::replace_provider_clients(
            &self.dir,
            &self.provider,
            &self.clients()?,
            reason,
        )
    }
}

async fn session_surface(store: &SharedStore, target: &SessionTarget) -> Result<SessionRecord> {
    match target {
        SessionTarget::Interactive { session, run } | SessionTarget::Ask { session, run } => {
            conversation_surface(store, session, run).await
        }
        SessionTarget::Flow { task, position, .. } => flow_surface(store, task, position).await,
        SessionTarget::StandaloneFlow(token) => {
            attribute_standalone_session(store, crate::ops::flow_session::surface(token)?).await
        }
    }
}

pub(crate) async fn mark_ready(store: &SharedStore, summary: &str) -> Result<()> {
    let summary = summary.trim();
    if summary.is_empty() {
        bail!("ready summary cannot be empty");
    }
    let run_id = active_run_id()?;
    let token = active_session_token()?;
    match token {
        HumanSessionToken::StandaloneFlow { token } => {
            crate::ops::flow_session::mark_ready(&token, &run_id, summary)?
        }
        HumanSessionToken::Flow { token } => {
            store
                .ready_session(&flow_token_id(&token), &run_id, summary)
                .await?;
        }
        HumanSessionToken::Ask { id } => {
            let lock_id = id.clone();
            let _lock =
                tokio::task::spawn_blocking(move || lock_session_launch(&lock_id)).await??;
            session_of_run(store, &run_id).await?;
            store.ready_session(&id, &run_id, summary).await?;
        }
    }
    Ok(())
}

async fn complete_flow(store: &SharedStore, task: &Task, position: &FlowPosition) -> Result<()> {
    let token = flow_token(task, position)?;
    let lock_id = flow_id(position)?;
    let launch_lock = tokio::task::spawn_blocking(move || lock_session_launch(&lock_id)).await??;
    crate::controller::task::complete_human_flow_step(store, &token, position).await?;
    let mut task = store
        .get_task(&token.task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {} disappeared after review completion", token.task_id))?;
    let launch = if store.flow_position(&task.id).await?.is_some() {
        crate::ops::task::launch_task_process(store, &mut task, None)
            .await
            .map_err(|error| anyhow!(error.to_string()))
    } else {
        Ok(())
    };
    stop_flow_run(store, &task, position).await;
    drop(launch_lock);
    launch.with_context(|| {
        format!(
            "Review feedback saved; continue with `lf task run {}`",
            task.plan.identifier
        )
    })
}

pub(crate) async fn require_current_review_actor(
    store: &SharedStore,
    position: &FlowPosition,
) -> Result<()> {
    let Ok(active) = active_run_id() else {
        return Ok(());
    };
    let id = flow_id(position)?;
    let history = store.session_runs(&id).await?;
    if history.iter().any(|run| run.id == active)
        && position.session_run_id.as_ref() != Some(&active)
    {
        bail!("superseded review Run cannot complete this Session");
    }
    Ok(())
}

pub(crate) async fn completion_worktree(
    store: &SharedStore,
    session_id: &str,
) -> Result<Option<PathBuf>> {
    match find_session(store, session_id)
        .await?
        .ok_or_else(|| session_not_found(session_id))?
    {
        SessionTarget::Flow { task, .. } => Ok(Some(task.worktree.clone())),
        SessionTarget::StandaloneFlow(token) => {
            crate::ops::flow_session::worktree(&token).map(Some)
        }
        SessionTarget::Interactive { .. } | SessionTarget::Ask { .. } => Ok(None),
    }
}

async fn stop_flow_run(store: &SharedStore, task: &Task, position: &FlowPosition) {
    let Some(run_id) = position.session_run_id.clone() else {
        return;
    };
    let result = async {
        let placement = store.placement(&position.work()).await?;
        let home = store
            .home_by_id(&placement.home_id)
            .await?
            .ok_or_else(|| anyhow!("Task {} Home {} disappeared", task.id, placement.home_id))?;
        if home.route == "local" {
            return stop_native_run(&run_id);
        }
        let repo = crate::engine::wave_home::resolve_home_relative_repo(&task.worktree)
            .map_err(anyhow::Error::msg)?;
        let command = vec![
            "lf".to_string(),
            "session".to_string(),
            "stop-run".to_string(),
            run_id.to_string(),
        ];
        tokio::task::spawn_blocking(move || {
            crate::lf::commands::ssh::capture_home_command(&home.id, &repo, &command)
        })
        .await
        .context("join remote Session stop")?
        .map(|_| ())
        .map_err(|error| anyhow!(error.to_string()))
    }
    .await;
    if let Err(error) = result {
        eprintln!("warning: Review completed but its provider client could not stop: {error:#}");
    }
}

pub(crate) async fn serve_flow(
    store: SharedStore,
    task_id: TaskId,
    invocation_id: String,
    flow: String,
    node_id: String,
    skill: String,
    iteration: u32,
) -> Result<()> {
    let task = store
        .get_task(&task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {task_id} disappeared"))?;
    let position = store
        .flow_position(&task_id)
        .await?
        .ok_or_else(|| anyhow!("review session is no longer waiting"))?;
    let token = flow_token(&task, &position)?;
    if token.invocation_id != invocation_id
        || token.flow != flow
        || token.node_id != node_id
        || token.skill.name != skill
        || token.iteration != iteration
    {
        bail!("review session is stale");
    }
    let launch_lock = lock_session_launch(&flow_token_id(&token))?;
    let current = store
        .flow_position(&task_id)
        .await?
        .ok_or_else(|| anyhow!("review is no longer waiting"))?;
    if current.session_run_id.is_some() {
        return Ok(());
    }
    serve_flow_locked(store, token, &current, launch_lock)
        .await
        .map(|_| ())
}

async fn serve_flow_locked(
    store: SharedStore,
    token: FlowSessionToken,
    position: &FlowPosition,
    launch_lock: File,
) -> Result<RunId> {
    let task = store
        .get_task(&token.task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {} disappeared", token.task_id))?;
    validate_token(&store, &token).await?;
    if let Some(failure) = &position.failure {
        bail!("review session cannot start: {}", failure.reason);
    }
    let message = flow_message(&task, &token);
    let lf = crate::engine::process::resolve_current_home_lf_binary_checked()?;
    let selector = format!("task:{}", token.task_id);
    let serialized = serde_json::to_string(&HumanSessionToken::Flow {
        token: Box::new(token.clone()),
    })?;
    let (position, reserved) = store.reserve_review_run(position).await?;
    let reservation = ReviewRunReservation {
        session_id: flow_token_id(&token),
        run_id: reserved.id.clone(),
        version: position.version,
    };
    let mut command = tokio::process::Command::new(lf);
    command
        .args(["--tui", "--as", &selector, &token.skill.name, &message])
        .current_dir(&task.worktree)
        .env(HUMAN_SESSION_ENV, serialized)
        .env(REVIEW_RUN_ENV, serde_json::to_string(&reservation)?);
    let mut child = spawn_session_run(&mut command, &reserved.id).await?;
    drop(launch_lock);
    let status = child.wait().await.context("wait for review skill")?;
    if !token_is_current(&store, &token).await? {
        let execution = crate::ops::task_execution::task_execution(&store, &token.task_id).await?;
        println!("Review session finished. {}", execution.reason);
        return Ok(reserved.id);
    }
    if status.success() {
        Ok(reserved.id)
    } else {
        Err(anyhow!("review skill exited with {status}"))
    }
}

pub(crate) async fn serve_ask(store: Option<SharedStore>, run_id: &RunId) -> Result<()> {
    let store = store.as_ref();
    let (session, _) = ask_attempt(store, run_id)
        .await?
        .ok_or_else(|| anyhow!("Ask Run {run_id} no longer exists"))?;
    let launch_lock = lock_session_launch(&session.id)?;
    let (session, run) = ask_attempt(store, run_id)
        .await?
        .ok_or_else(|| anyhow!("Ask Run {run_id} no longer exists"))?;
    if session.completed_at.is_some() {
        bail!("session {:?} is already resolved", session.id);
    }
    if run.id != *run_id || !run_is_prepared(&run.id)? {
        // Another launcher already published this Session. Do not replace or
        // infer death of its native client; explicit open owns native resume.
        return Ok(());
    }
    serve_ask_locked(store, &session, &run, launch_lock).await
}

/// An Ask attempt's rows, or its reservation while no store can take them.
async fn ask_attempt(
    store: Option<&SharedStore>,
    run_id: &RunId,
) -> Result<Option<(Session, Run)>> {
    if let Some(store) = store {
        if let Ok(Some(found)) = session_of_run(store, run_id).await {
            return Ok(Some(found));
        }
    }
    Ok(unrecorded_session::read(run_id)?.map(|reserved| (reserved.session, reserved.run)))
}

async fn serve_ask_locked(
    store: Option<&SharedStore>,
    session: &Session,
    run: &Run,
    launch_lock: File,
) -> Result<()> {
    let lf = crate::engine::process::resolve_current_home_lf_binary_checked()?;
    let mut command = tokio::process::Command::new(lf);
    command
        .args(ask_launch_args(store, session, run).await)
        .current_dir(&run.cwd)
        .env(
            HUMAN_SESSION_ENV,
            serde_json::to_string(&HumanSessionToken::Ask {
                id: session.id.clone(),
            })?,
        );
    let mut child = spawn_session_run(&mut command, &run.id).await?;
    drop(launch_lock);
    let status = child.wait().await.context("wait for session agent")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("session agent exited with {status}"))
    }
}

async fn ask_launch_args(store: Option<&SharedStore>, session: &Session, run: &Run) -> Vec<String> {
    let mut args = vec![
        "--tui".to_string(),
        "--model".to_string(),
        launch_model(run),
        "--__cwd".to_string(),
        run.cwd.display().to_string(),
    ];
    // The conversation's prompt carries its Work only while launch can resolve
    // it; the Run row keeps the attribution either way.
    if let (Some(store), Some(selector)) = (store, work_selector(run)) {
        if crate::ops::resolve_work_binding(store, &run.cwd, &selector)
            .await
            .is_ok()
        {
            args.extend(["--as".to_string(), selector]);
        }
    }
    match &run.skill {
        Some(skill) => args.extend(["skill".to_string(), skill.clone()]),
        None => args.push(":".to_string()),
    }
    args.push(ask_message(session.request.as_deref().unwrap_or_default()));
    args
}

fn work_selector(run: &Run) -> Option<String> {
    match (&run.task_id, &run.wave_id) {
        (Some(task), _) => Some(format!("task:{task}")),
        (None, Some(wave)) => Some(format!("wave:{wave}")),
        (None, None) => None,
    }
}

pub(crate) fn publish_run_binding(run_id: &RunId) -> Result<()> {
    let raw = std::env::var_os(REVIEW_RUN_ENV)
        .ok_or_else(|| anyhow!("review Run has no launch reservation"))?;
    std::env::remove_var(REVIEW_RUN_ENV);
    let reservation: ReviewRunReservation = serde_json::from_str(&raw.to_string_lossy())?;
    if reservation.run_id != *run_id {
        bail!("review Run differs from its reservation");
    }
    let store =
        crate::store::sqlite::SqliteStore::new(&crate::store::observability_database_path()?)?;
    store.publish_review_run(&reservation.session_id, run_id, reservation.version)?;
    Ok(())
}

pub(crate) fn prepared_run_id() -> Result<Option<RunId>> {
    let Some(value) = std::env::var_os(PREPARED_RUN_ENV) else {
        return Ok(None);
    };
    std::env::remove_var(PREPARED_RUN_ENV);
    active_session_token()?;
    Ok(Some(RunId::parse(
        &value
            .into_string()
            .map_err(|_| anyhow!("prepared Run id is not UTF-8"))?,
    )?))
}

pub(crate) async fn open(
    store: &SharedStore,
    session_id: &str,
    mode: OpenMode,
    resume: bool,
) -> Result<SessionRecord> {
    let target = find_session(store, session_id)
        .await?
        .ok_or_else(|| session_not_found(session_id))?;
    match &target {
        SessionTarget::StandaloneFlow(token) => {
            let session = crate::ops::flow_session::open(token, mode, resume).await?;
            attribute_standalone_session(store, session).await
        }
        SessionTarget::Interactive { session, run } => {
            let native = NativeRun::of(session, run)?;
            let provider_session = native.history(session)?;
            if resume {
                crate::lf::commands::util::require_provider_session_launch(&native.dir)?;
            }
            match mode {
                OpenMode::Refuse if !native.clients()?.is_empty() => {
                    require_session_action(
                        SessionKind::Interactive,
                        SessionState::Active,
                        SessionActionKind::Open,
                    )?;
                }
                OpenMode::Replace if resume => {
                    native.stop_clients(crate::run_record::ProviderClientStopReason::Moved)?
                }
                OpenMode::Replace => {}
                OpenMode::Refuse | OpenMode::Try => {}
            }
            let mut session = conversation_surface(store, session, run).await?;
            if resume {
                crate::lf::commands::util::resume_session(
                    &native.provider,
                    run.model.as_deref(),
                    &run.cwd,
                    &run.id,
                    &native.dir,
                    &provider_session,
                )?;
            } else {
                match mode {
                    OpenMode::Replace => session.open_argv.push("--replace".to_string()),
                    OpenMode::Try => session.open_argv.push("--try".to_string()),
                    OpenMode::Refuse => {}
                }
            }
            Ok(session)
        }
        SessionTarget::Flow { task, position, .. } => {
            if mode != OpenMode::Refuse {
                bail!("--replace and --try apply only to interactive provider sessions");
            }
            #[cfg(test)]
            action_test::after_lookup("open", session_id).await;
            let id = flow_id(position)?;
            let lock_id = id.clone();
            let launch_lock =
                tokio::task::spawn_blocking(move || lock_session_launch(&lock_id)).await??;
            let current = find_session(store, session_id)
                .await?
                .ok_or_else(|| session_not_found(session_id))?;
            let SessionTarget::Flow {
                position: current, ..
            } = current
            else {
                bail!("review Session changed while opening");
            };
            if current != *position {
                bail!("review Run changed while opening; open the Session again");
            }
            let mut session = session_surface(store, &target).await?;
            if resume {
                session.run_id = open_flow_locked(store, task, position, launch_lock).await?;
            }
            Ok(session)
        }
        SessionTarget::Ask { session, run } => {
            if mode != OpenMode::Refuse {
                bail!("--replace and --try apply only to interactive provider sessions");
            }
            let mut surface = conversation_surface(store, session, run).await?;
            if resume {
                surface.run_id = open_ask(store, &session.id).await?;
            }
            Ok(surface)
        }
    }
}

pub(crate) async fn complete(store: &SharedStore, session_id: &str) -> Result<SessionRecord> {
    let target = find_session(store, session_id)
        .await?
        .ok_or_else(|| session_not_found(session_id))?;
    #[cfg(test)]
    action_test::after_lookup("complete", session_id).await;
    let session = session_surface(store, &target).await?;
    if matches!(&target, SessionTarget::Flow { .. }) {
        if let Some(destination) = super::task_destination::destination()? {
            // The full boundary id includes Task, invocation, node and iteration.
            // The installed operation reads its own readiness/feedback; none is copied.
            super::task_destination::execute(
                &destination,
                &std::env::current_dir()?,
                &["session".into(), "complete".into(), session.id.clone()],
                None,
            )?;
            return Ok(session);
        }
    }
    require_session_action(session.kind, session.state, SessionActionKind::Complete)?;
    match &target {
        SessionTarget::Interactive { session, run } => {
            let native = NativeRun::of(session, run)?;
            crate::lf::commands::util::stop_provider_session(&native.dir, &native.provider)?;
            store
                .complete_session(&session.id, &run.id)
                .await?;
        }
        SessionTarget::Ask { session, run } => {
            let lock_id = session.id.clone();
            let _lock =
                tokio::task::spawn_blocking(move || lock_session_launch(&lock_id)).await??;
            store.complete_session(&session.id, &run.id).await?;
            // The answer is durable before teardown can interrupt this caller
            // or fail. A cleanup failure must not strand the waiting caller.
            if let Err(error) = stop_native_run(&run.id) {
                tracing::warn!(session_id = %session.id, run_id = %run.id,
                    error = %format!("{error:#}"),
                    "Ask completion saved, but its native Session could not be stopped");
            }
        }
        SessionTarget::Flow { task, position, .. } => {
            complete_flow(store, task, position).await?;
        }
        SessionTarget::StandaloneFlow(token) => {
            crate::ops::flow_session::complete(token).await?;
        }
    }
    Ok(session)
}

async fn open_ask(store: &SharedStore, id: &str) -> Result<RunId> {
    loop {
        let (_, observed) = store
            .session(id)
            .await?
            .ok_or_else(|| session_not_found(id))?;
        let lock_id = id.to_string();
        let launch_lock =
            tokio::task::spawn_blocking(move || lock_session_launch(&lock_id)).await??;
        let (session, run) = store
            .session(id)
            .await?
            .ok_or_else(|| session_not_found(id))?;
        if session.completed_at.is_some() {
            bail!("session {id:?} is already complete");
        }
        if run.id != observed.id {
            drop(launch_lock);
            continue;
        }
        let mut launch_lock = Some(launch_lock);
        let token = HumanSessionToken::Ask { id: id.to_string() };
        if resume_native_run(&run.id, &token, &mut launch_lock)? {
            return Ok(run.id);
        }
        // A consumed launch without native history gets another attempt under
        // the same Session; its title and feedback never leave the row.
        let run = if run_is_prepared(&run.id)? {
            run
        } else {
            let replaced = run.id.clone();
            store
                .replace_session_run(&replaced, prepare_ask_run(run)?)
                .await?
        };
        serve_ask_locked(
            Some(store),
            &session,
            &run,
            launch_lock.expect("unresumed Ask retains its launch lock"),
        )
        .await?;
        return Ok(run.id);
    }
}

async fn open_flow_locked(
    store: &SharedStore,
    task: &Task,
    position: &FlowPosition,
    launch_lock: File,
) -> Result<RunId> {
    let previous = position.session_run_id.clone();
    let token = flow_token(task, position)?;
    let mut launch_lock = Some(launch_lock);
    if let Some(run_id) = &previous {
        if resume_native_run(
            run_id,
            &HumanSessionToken::Flow {
                token: Box::new(token.clone()),
            },
            &mut launch_lock,
        )? {
            return Ok(run_id.clone());
        }
    }
    if let Some(run_id) = &previous {
        let (dir, manifest) = crate::run_record::resolve_manifest(
            &crate::store::observability_home_dir(),
            run_id.as_str(),
        )
        .context("cannot replace a review Run without its launch evidence")?;
        if crate::run_record::read_provider_session(&dir)?.is_some()
            || !crate::lf::commands::util::active_provider_clients(&dir, &manifest.harness)?
                .is_empty()
        {
            bail!("review Run still has native history or an active client");
        }
        if crate::run_record::read_run_snapshot(&dir)?
            .outcome
            .is_none()
        {
            bail!("review Run {run_id} has no terminal outcome; launch status is unresolved, so Open cannot authorize a replacement");
        }
    }
    serve_flow_locked(
        store.clone(),
        token,
        position,
        launch_lock.expect("unresumed review retains its launch lock"),
    )
    .await
}

/// A replacement Run continues the same Session, so it keeps that Session's
/// name. Copy it before the boundary publishes the new Run.
pub(crate) fn carry_session_name(
    session_id: &str,
    previous: Option<&RunId>,
    next: Option<&RunId>,
) -> Result<()> {
    let (Some(previous), Some(next)) = (previous, next) else {
        return Ok(());
    };
    if previous == next {
        return Ok(());
    }
    let (Some(from), Some(to)) = (local_session_run_dir(previous), local_session_run_dir(next))
    else {
        return Ok(());
    };
    let Some(name) =
        crate::run_record::read_session_name(&from).context("read replaced Session name")?
    else {
        return Ok(());
    };
    crate::run_record::write_session_name(&to, &name.title, name.source)
        .with_context(|| format!("carry Session {session_id} name to its replacement Run"))?;
    Ok(())
}

pub(crate) fn run_is_prepared(run_id: &RunId) -> Result<bool> {
    match crate::run_record::resolve_manifest(
        &crate::store::observability_home_dir(),
        run_id.as_str(),
    ) {
        Ok((dir, _)) => Ok(dir.join("prepared").is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).context("resolve prepared Session Run"),
    }
}

pub(crate) fn stop_run(run_id: &RunId) -> Result<()> {
    stop_native_run(run_id)
}

/// An interactive or Ask Session, read from its row and its current Run.
async fn conversation_surface(
    store: &SharedStore,
    session: &Session,
    run: &Run,
) -> Result<SessionRecord> {
    let asked = session.kind == crate::session::SessionKind::Ask;
    let kind = if asked {
        SessionKind::Ask
    } else {
        SessionKind::Interactive
    };
    let native = NativeRun::of(session, run)?;
    let clients = native.clients()?;
    let state = if asked && session.completed_at.is_some() {
        SessionState::Closed
    } else if asked && session.ready_summary.is_some() {
        SessionState::Ready
    } else if !clients.is_empty() {
        SessionState::Active
    } else if session.completed_at.is_some()
        || crate::run_record::read_provider_session(&native.dir)?.is_some()
    {
        SessionState::Closed
    } else {
        SessionState::Waiting
    };
    let work = match (&run.task_id, &run.wave_id) {
        (Some(task), _) => Some(WorkRef::Task(task.clone())),
        (None, Some(wave)) => Some(WorkRef::Wave(wave.clone())),
        (None, None) => None,
    };
    Ok(SessionRecord {
        id: session.id.clone(),
        run_id: run.id.clone(),
        kind,
        wave_id: run.wave_id.clone(),
        work_path: session_work_path(store, work.as_ref()).await?,
        work,
        actions: session_actions(kind, state),
        title: session.title.clone(),
        title_source: wire_title_source(session.title_source),
        // An Ask is a human boundary requested by a Run, never a Flow step.
        flow_membership: SessionFlowMembership::Independent,
        detail: if asked {
            run.skill
                .clone()
                .unwrap_or_else(|| "Request for input".to_string())
        } else {
            launch_model(run)
        },
        provider: Some(native.provider),
        cwd: run.cwd.display().to_string(),
        state,
        ready_summary: session.ready_summary.clone(),
        terminal_ids: clients
            .into_iter()
            .filter_map(|client| client.terminal_id)
            .collect(),
        open_argv: human_open_argv(None, None, &session.id)?,
    })
}

/// The `provider[:model]` a Run launched with.
fn launch_model(run: &Run) -> String {
    let provider = run.provider.clone().unwrap_or_default();
    match &run.model {
        Some(model) => format!("{provider}:{model}"),
        None => provider,
    }
}

fn wire_title_source(source: crate::session::TitleSource) -> SessionTitleSource {
    match source {
        crate::session::TitleSource::Human => SessionTitleSource::Human,
        crate::session::TitleSource::Generated => SessionTitleSource::Generated,
    }
}

async fn attribute_standalone_session(
    store: &SharedStore,
    mut session: SessionRecord,
) -> Result<SessionRecord> {
    let (dir, manifest) = crate::run_record::resolve_manifest(
        &crate::store::observability_home_dir(),
        session.run_id.as_str(),
    )?;
    session.work = crate::run_record::attributed_work(store, &manifest).await;
    session.wave_id = session_wave_id(store, session.work.as_ref()).await?;
    session.work_path = session_work_path(store, session.work.as_ref()).await?;
    session.terminal_ids =
        crate::lf::commands::util::active_provider_clients(&dir, &manifest.harness)?
            .into_iter()
            .filter_map(|client| client.terminal_id)
            .collect();
    Ok(session)
}

async fn session_wave_id(
    store: &SharedStore,
    work: Option<&WorkRef>,
) -> Result<Option<crate::id::WaveId>> {
    Ok(match work {
        Some(WorkRef::Wave(id)) => Some(id.clone()),
        Some(WorkRef::Task(id)) => store.get_task(id).await?.map(|task| task.wave_id),
        Some(WorkRef::Project(id)) => store.get_project(id).await?.map(|project| project.wave_id),
        None => None,
    })
}

async fn session_work_path(store: &SharedStore, work: Option<&WorkRef>) -> Result<Option<String>> {
    let Some(work) = work else { return Ok(None) };
    let mut labels = Vec::new();
    if let WorkRef::Task(id) = work {
        match store.get_task(id).await? {
            Some(task) => labels.push(task.plan.identifier),
            None => return Ok(Some(format!("Task {id} (unavailable)"))),
        }
    }
    if let Some(id) = session_wave_id(store, Some(work)).await? {
        labels.push(match store.get_wave(&id).await? {
            Some(wave) => wave.name().to_string(),
            None => format!("Wave {id} (unavailable)"),
        });
    } else if let WorkRef::Project(id) = work {
        return Ok(Some(format!("Historical Work {id} (unavailable)")));
    }
    labels.reverse();
    Ok(Some(labels.join(" / ")))
}

/// The stored name, else the seed: the invoked skill for skill Sessions, the
/// question for Asks, or a stable `magical-musical` pair for raw Sessions.
pub(crate) fn session_name(dir: Option<&Path>, seed: String) -> Result<SessionName> {
    let stored = match dir {
        Some(dir) => crate::run_record::read_session_name(dir).context("read Session name")?,
        None => None,
    };
    Ok(stored.unwrap_or(SessionName {
        title: seed,
        source: SessionTitleSource::Generated,
    }))
}

fn local_session_run_dir(run_id: &RunId) -> Option<PathBuf> {
    crate::run_record::record_dir(&crate::store::observability_home_dir(), run_id)
}

/// The provider recorded on a local Run's manifest. Never guessed from
/// configuration: an unreadable or remote Run has no recorded provider here.
pub(crate) fn recorded_provider(dir: Option<&Path>) -> Option<String> {
    let manifest = crate::run_record::read_manifest(dir?).ok()?;
    Some(manifest.harness)
}

/// Rename a Session through its Run and return the authoritative record.
/// `Generated` is an agent suggestion; it never replaces a human-assigned name.
/// The id may be the Session's own `$LF_RUN_ID`: an Ask or Flow Run names its
/// boundary, and naming needs no provider history.
pub(crate) async fn rename(
    store: &SharedStore,
    session_id: &str,
    title: &str,
    source: SessionTitleSource,
) -> Result<SessionRecord> {
    let mut target = find_session(store, session_id)
        .await?
        .ok_or_else(|| session_not_found(session_id))?;
    let title = crate::run_record::validate_session_title(title)
        .map_err(|error| anyhow!("cannot rename Session {session_id}: {error}"))?;
    let title_source = match source {
        SessionTitleSource::Human => crate::session::TitleSource::Human,
        SessionTitleSource::Generated => crate::session::TitleSource::Generated,
        SessionTitleSource::Unavailable => bail!("unavailable is not a title source"),
    };
    if let SessionTarget::Interactive { session, run } | SessionTarget::Ask { session, run } =
        &target
    {
        // A Run selector names that attempt; the Session id names the conversation.
        let expected_run = (session_id != session.id).then_some(&run.id);
        store
            .rename_session(&session.id, expected_run, title, title_source)
            .await?;
        let (session, run) = store
            .session(&session.id)
            .await?
            .ok_or_else(|| session_not_found(session_id))?;
        return conversation_surface(store, &session, &run).await;
    }
    let expected_run = match &target {
        SessionTarget::Flow {
            run_id, position, ..
        } if session_id != flow_id(position)? => Some(run_id.clone()),
        _ => None,
    };
    // Retain the caller's exact/prefix selector while waiting for replacement.
    let id = boundary_id(&target)?;
    let pending_lock = tokio::task::spawn_blocking(move || lock_session_launch(&id));
    #[cfg(test)]
    action_test::after_lookup("rename", session_id).await;
    let _launch_lock = pending_lock.await??;
    target = find_session(store, session_id)
        .await?
        .ok_or_else(|| session_not_found(session_id))?;
    if let SessionTarget::Flow { position, .. } = &target {
        let id = flow_id(position)?;
        store
            .rename_session(&id, expected_run.as_ref(), title, title_source)
            .await?;
        return session_surface(store, &target).await;
    }
    let session = session_surface(store, &target).await?;
    let dir = local_session_run_dir(&session.run_id)
        .ok_or_else(|| anyhow!("Session {} has an invalid Run reference", session.id))?;
    crate::run_record::write_session_name(&dir, title, source)
        .map_err(|error| anyhow!("cannot rename Session {}: {error}", session.id))?;
    session_surface(store, &target).await
}

/// The durable id of a boundary Session, whichever id resolved it.
fn boundary_id(target: &SessionTarget) -> Result<String> {
    Ok(match target {
        SessionTarget::Interactive { session, .. } | SessionTarget::Ask { session, .. } => {
            session.id.clone()
        }
        SessionTarget::Flow { position, .. } => flow_id(position)?,
        SessionTarget::StandaloneFlow(token) => crate::ops::flow_session::session_id(token),
    })
}

/// The waiting standalone Flow boundary whose linked Run is `run_id`.
fn find_boundary_for_run(run_id: &str) -> Result<Option<SessionTarget>> {
    if RunId::parse(run_id).is_err() {
        return Ok(None);
    }
    for (run, token) in crate::ops::flow_session::reviews()? {
        if run
            .active
            .and_then(|boundary| boundary.run_id)
            .as_ref()
            .map(RunId::as_str)
            == Some(run_id)
        {
            return Ok(Some(SessionTarget::StandaloneFlow(token)));
        }
    }
    Ok(None)
}

fn session_not_found(id: &str) -> anyhow::Error {
    anyhow!("Session {id} was not found")
}

pub(crate) async fn spawn_session_run(
    command: &mut tokio::process::Command,
    run_id: &RunId,
) -> Result<tokio::process::Child> {
    let home = crate::store::observability_home_dir();
    let executable = Path::new(command.as_std().get_program());
    let digest = fs::read(executable)
        .ok()
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .unwrap_or_else(|| "unavailable".to_string());
    let cwd = match command.as_std().get_current_dir() {
        Some(cwd) => cwd.to_path_buf(),
        None => std::env::current_dir()?,
    };
    let database = crate::store::observability_database_path()?;
    // Capture the child before it parses arguments. Its own Run recorder may
    // never start; the opening Run must retain enough evidence to diagnose it.
    // Arguments and environment values can contain prompts or credentials.
    let launch = format!(
        "Session Run {run_id}: executable {} (sha256 {digest}), cwd {}, Home {}, database {}",
        executable.display(),
        cwd.display(),
        home.display(),
        database.display()
    );
    if run_is_prepared(run_id)? {
        command.env(PREPARED_RUN_ENV, run_id.as_str());
    }
    let mut child = command
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("could not start {launch}"))?;
    let deadline = tokio::time::Instant::now() + SESSION_START_TIMEOUT;
    loop {
        match crate::run_record::resolve_manifest(&home, run_id.as_str()) {
            Ok((dir, manifest)) => {
                if session_run_is_resumable(&dir, &manifest)? {
                    return Ok(child);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        if let Some(status) = child.try_wait().context("probe human Session Run")? {
            bail!("{launch}: exited with {status} before becoming resumable");
        }
        if tokio::time::Instant::now() >= deadline {
            bail!("{launch}: did not become resumable within 30s");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn session_run_is_resumable(dir: &Path, manifest: &RunManifest) -> Result<bool> {
    if crate::run_record::read_provider_session(dir)?.is_none() {
        return Ok(false);
    }
    Ok(!crate::lf::commands::util::active_provider_clients(dir, &manifest.harness)?.is_empty())
}

pub(crate) fn resume_native_run(
    run_id: &RunId,
    token: &HumanSessionToken,
    launch_lock: &mut Option<File>,
) -> Result<bool> {
    #[cfg(test)]
    if let Some(result) = action_test::resume(run_id, launch_lock) {
        return result;
    }
    let home = crate::store::observability_home_dir();
    let (dir, manifest) = match crate::run_record::resolve_manifest(&home, run_id.as_str()) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).context("resolve Session Run"),
    };
    let Some(provider_session) = crate::run_record::read_provider_session(&dir)? else {
        return Ok(false);
    };
    crate::lf::commands::util::require_provider_session_launch(&dir)?;
    let clients = crate::lf::commands::util::active_provider_clients(&dir, &manifest.harness)?;
    crate::lf::commands::util::replace_provider_clients(
        &dir,
        &manifest.harness,
        &clients,
        crate::run_record::ProviderClientStopReason::Moved,
    )?;
    let environment =
        BTreeMap::from([(HUMAN_SESSION_ENV.to_string(), serde_json::to_string(token)?)]);
    crate::lf::commands::util::resume_session_with_env(
        &manifest.harness,
        manifest.model.as_deref(),
        &manifest.cwd,
        &manifest.run_id,
        &dir,
        &provider_session,
        &environment,
        launch_lock.take(),
    )?;
    Ok(true)
}

fn stop_native_run(run_id: &RunId) -> Result<()> {
    #[cfg(test)]
    if action_test::stop(run_id) {
        return Ok(());
    }
    let home = crate::store::observability_home_dir();
    let (dir, manifest) = crate::run_record::resolve_manifest(&home, run_id.as_str())
        .with_context(|| {
            format!("cannot confirm Session Run {run_id} stopped: manifest unavailable")
        })?;
    crate::lf::commands::util::stop_provider_session(&dir, &manifest.harness)
}

pub(crate) fn native_session_state(
    run_id: Option<&RunId>,
    ready_summary: Option<&str>,
) -> Result<SessionState> {
    if ready_summary.is_some() {
        return Ok(SessionState::Ready);
    }
    let Some(run_id) = run_id else {
        return Ok(SessionState::Waiting);
    };
    let home = crate::store::observability_home_dir();
    let (dir, manifest) = match crate::run_record::resolve_manifest(&home, run_id.as_str()) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SessionState::Waiting);
        }
        Err(error) => return Err(error).context("resolve Session Run"),
    };
    if !crate::lf::commands::util::active_provider_clients(&dir, &manifest.harness)?.is_empty() {
        return Ok(SessionState::Active);
    }
    if crate::run_record::read_provider_session(&dir)?.is_some() {
        Ok(SessionState::Closed)
    } else {
        Ok(SessionState::Waiting)
    }
}

pub(crate) async fn token_is_current(
    store: &SharedStore,
    token: &FlowSessionToken,
) -> Result<bool> {
    Ok(store
        .flow_position(&token.task_id)
        .await?
        .as_ref()
        .is_some_and(|position| token_matches(token, position)))
}

async fn list_flow_sessions(store: &SharedStore) -> Result<Vec<SessionRecord>> {
    let mut records = Vec::new();
    for (session, run) in store.open_review_sessions().await? {
        records.push(review_surface(store, &session, &run).await?);
    }
    Ok(records)
}

async fn review_surface(
    store: &SharedStore,
    session: &crate::session::Session,
    run: &crate::session::Run,
) -> Result<SessionRecord> {
    let task_id = run
        .task_id
        .as_ref()
        .ok_or_else(|| anyhow!("Task review lacks its Task"))?;
    let work = WorkRef::Task(task_id.clone());
    let placement = store.placement(&work).await?;
    let home = store
        .home_by_id(&placement.home_id)
        .await?
        .ok_or_else(|| anyhow!("Session {} Home disappeared", session.id))?;
    let runtime = if session.completed_at.is_some() {
        SessionState::Closed
    } else if home.route == "local" {
        native_session_state(
            run.published.then_some(&run.id),
            session.ready_summary.as_deref(),
        )?
    } else if session.ready_summary.is_some() {
        SessionState::Ready
    } else if run.published {
        SessionState::Closed
    } else {
        SessionState::Waiting
    };
    let (position, execution_error) = match store.flow_position(task_id).await {
        Ok(position) => (position, None),
        Err(crate::store::StoreError::InvalidData(reason)) => (None, Some(reason)),
        Err(error) => return Err(error.into()),
    };
    let dir = local_session_run_dir(&run.id);
    let flow_membership = if let Some(reason) = &execution_error {
        SessionFlowMembership::Unknown {
            reason: reason.clone(),
        }
    } else if let Some(manifest) = dir
        .as_deref()
        .and_then(|dir| crate::run_record::read_manifest(dir).ok())
    {
        run_flow_membership(store, Some(&manifest), &run.id).await
    } else if let Some(position) = position
        .as_ref()
        .filter(|position| flow_id(position).ok().as_deref() == Some(session.id.as_str()))
    {
        SessionFlowMembership::of_position(position)
    } else {
        SessionFlowMembership::Unknown {
            reason: "Run membership evidence is unavailable".into(),
        }
    };
    let mut actions = position
        .as_ref()
        .filter(|position| flow_id(position).ok().as_deref() == Some(session.id.as_str()))
        .map(flow_actions)
        .unwrap_or_else(|| session_actions(SessionKind::Flow, runtime));
    if let Some(reason) = &execution_error {
        for action in &mut actions {
            action.unavailable_reason = Some(reason.clone());
        }
    }
    Ok(SessionRecord {
        run_id: run.id.clone(),
        title_source: wire_title_source(session.title_source),
        work_path: session_work_path(store, Some(&work)).await?,
        actions,
        flow_membership,
        provider: recorded_provider(dir.as_deref()),
        terminal_ids: Vec::new(),
        id: session.id.clone(),
        kind: SessionKind::Flow,
        work: Some(work),
        wave_id: run.wave_id.clone(),
        title: session.title.clone(),
        detail: run.skill.clone().unwrap_or_default(),
        cwd: run.cwd.display().to_string(),
        state: runtime,
        ready_summary: session.ready_summary.clone(),
        open_argv: human_open_argv(
            (home.route != "local").then_some(&home.id),
            Some(&run.cwd),
            &session.id,
        )?,
    })
}

async fn find_flow_session(store: &SharedStore, session_id: &str) -> Result<(Task, FlowPosition)> {
    find_flow_session_optional(store, session_id)
        .await?
        .ok_or_else(|| anyhow!("session {session_id:?} is no longer waiting"))
}

async fn find_flow_session_optional(
    store: &SharedStore,
    session_id: &str,
) -> Result<Option<(Task, FlowPosition)>> {
    let Some((session, run)) = store.session(session_id).await? else {
        return Ok(None);
    };
    if session.completed_at.is_some() {
        bail!("Session {session_id} is already complete");
    }
    let task_id = run
        .task_id
        .ok_or_else(|| anyhow!("review Session has no Task"))?;
    let position = store
        .flow_position(&task_id)
        .await?
        .ok_or_else(|| anyhow!("Session {session_id} invocation is no longer current"))?;
    if flow_id(&position)? != session.id {
        bail!("Session {session_id} is no longer waiting");
    }
    let task = store
        .get_task(&task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {task_id} disappeared"))?;
    Ok(Some((task, position)))
}

async fn flow_surface(
    store: &SharedStore,
    task: &Task,
    position: &FlowPosition,
) -> Result<SessionRecord> {
    validate_task_position(task, position)?;
    let id = flow_id(position)?;
    let (session, run) = store
        .session(&id)
        .await?
        .ok_or_else(|| session_not_found(&id))?;
    review_surface(store, &session, &run).await
}

pub(crate) fn human_open_argv(
    remote_home: Option<&crate::durable::HomeId>,
    worktree: Option<&Path>,
    id: &str,
) -> Result<Vec<String>> {
    let context = crate::engine::process::current_home_execution_context()?;
    // A fresh terminal does not inherit the listing process's data selection.
    // Carry the executable and its data together, including when a different
    // installation becomes current between listing and opening.
    let mut argv = vec![
        "/usr/bin/env".to_string(),
        format!("LF_BIN={}", context.lf_bin.display()),
        format!("LF_HOME={}", context.lf_home.display()),
        format!("LF_DB_PATH={}", context.db_path.display()),
        context.lf_bin.display().to_string(),
    ];
    if let Some(home_id) = remote_home {
        argv.push("ssh".to_string());
        if let Some(worktree) = worktree {
            let repo = crate::engine::wave_home::resolve_home_relative_repo(worktree)
                .map_err(anyhow::Error::msg)?;
            argv.extend(["--repo".to_string(), repo]);
        }
        argv.push(home_id.to_string());
    }
    argv.extend(["session".to_string(), "open".to_string(), id.to_string()]);
    Ok(argv)
}

fn validate_task_position(task: &Task, position: &FlowPosition) -> Result<()> {
    if position.task_id != task.id || !position.is_human() {
        return Err(anyhow!("session does not belong to Task {}", task.id));
    }
    let step = position.current();
    let node_id = step
        .policy
        .id
        .as_deref()
        .ok_or_else(|| anyhow!("review flow position has no node id"))?;
    if node_id.trim().is_empty() {
        return Err(anyhow!("review flow position has an empty node id"));
    }
    Ok(())
}

async fn validate_token(store: &SharedStore, token: &FlowSessionToken) -> Result<()> {
    if token_is_current(store, token).await? {
        Ok(())
    } else {
        Err(anyhow!("review session is stale"))
    }
}

fn flow_token(task: &Task, position: &FlowPosition) -> Result<FlowSessionToken> {
    validate_task_position(task, position)?;
    let step = position.current();
    let crate::engine::ConcreteStep::Skill(planned) = position.current_plan() else {
        bail!("review flow position does not select a Skill");
    };
    Ok(FlowSessionToken {
        task_id: task.id.clone(),
        invocation_id: position.invocation.id.clone(),
        flow: step.flow,
        node_id: step
            .policy
            .id
            .expect("validated human position has a node id"),
        skill: planned.skill.clone(),
        iteration: position.cursor.iteration,
    })
}

fn token_matches(token: &FlowSessionToken, position: &FlowPosition) -> bool {
    let step = position.current();
    position.task_id == token.task_id
        && position.invocation.id == token.invocation_id
        && step.policy.human
        && step.flow == token.flow
        && step.policy.id.as_deref() == Some(token.node_id.as_str())
        && step.step == token.skill.name
        && position.cursor.iteration == token.iteration
}

fn flow_message(task: &Task, token: &FlowSessionToken) -> String {
    format!(
        "<lf:human-session>\nThis `{skill}` Run is the interactive review for Task {identifier}. Work with the user and save self-contained, topic-named notes under scratch/: feedback, agreed design changes, unresolved questions, and next useful action, with links to the current design and evidence. Put the exact note paths and a short takeaway in the ready summary. Run `lf session ready \"feedback, design changes, and remaining work\"` when the review is ready to end. The user completes it with `lf session complete {session}`. Completion returns feedback to the next Flow step; a following loop-decide owns Advance or Iterate. Do not record a navigation decision from this review.\n</lf:human-session>",
        skill = token.skill.name,
        identifier = task.plan.identifier,
        session = flow_token_id(token),
    )
}

fn ask_message(request: &str) -> String {
    format!(
        "{request}\n\n<lf:human-session>\nThe originating Loopflow Run is blocked while you work with the user in this terminal. You are in the caller's checkout and may inspect or edit it. Save useful findings and user decisions in self-contained topic notes under scratch/; include their exact paths and a short takeaway in the ready summary. When the work is ready, run `lf session ready \"<concise summary>\"`. Ready keeps this session visible and does not resume the caller; the user completes it when the conversation is finished.\n</lf:human-session>"
    )
}

async fn launch_flow(task: &Task, position: &FlowPosition) -> Result<()> {
    let step = position.current();
    let node_id = step
        .policy
        .id
        .as_deref()
        .ok_or_else(|| anyhow!("review flow position has no node id"))?;
    let lf = crate::engine::process::resolve_current_home_lf_binary();
    let argv = vec![
        lf.to_string_lossy().to_string(),
        "session".to_string(),
        "serve-flow".to_string(),
        task.id.to_string(),
        position.invocation.id.clone(),
        step.flow,
        node_id.to_string(),
        step.step,
        position.cursor.iteration.to_string(),
    ];
    start_durable_session(&flow_background_name(position)?, &task.worktree, &argv, &[]).await
}

/// Hand the conversation to a durable terminal, as a child of the asking Run.
async fn launch_ask(session: &Session, run: &Run) -> Result<()> {
    let lf = crate::engine::process::resolve_current_home_lf_binary();
    let argv = vec![
        lf.to_string_lossy().to_string(),
        "session".to_string(),
        "serve-ask".to_string(),
        run.id.to_string(),
    ];
    let caller = std::env::var(crate::durable::RUN_ID_ENV).context("read the asking Run")?;
    let caller_dir = std::env::var(RUN_DIR_ENV).context("read the asking Run directory")?;
    start_durable_session(
        &ask_background_name(&session.id),
        &run.cwd,
        &argv,
        &[
            (crate::durable::RUN_ID_ENV, caller.as_str()),
            (RUN_DIR_ENV, caller_dir.as_str()),
        ],
    )
    .await
}

#[cfg(not(test))]
async fn ask_launcher_is_running(id: &str) -> Result<bool> {
    let status = tokio::process::Command::new("tmux")
        .args([
            "has-session",
            "-t",
            &format!("={}", ask_background_name(id)),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .context("inspect human Ask launcher")?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => bail!("inspect human Ask launcher: tmux exited with {status}"),
    }
}

#[cfg(test)]
async fn ask_launcher_is_running(id: &str) -> Result<bool> {
    Ok(tests::ASK_LAUNCHERS
        .lock()
        .unwrap()
        .contains(&ask_background_name(id)))
}

#[cfg(not(test))]
async fn start_durable_session(
    name: &str,
    cwd: &Path,
    argv: &[String],
    env: &[(&str, &str)],
) -> Result<()> {
    crate::engine::process::start_home_session_with_env(name, cwd, argv, env).await
}

#[cfg(test)]
async fn start_durable_session(
    name: &str,
    _cwd: &Path,
    argv: &[String],
    _env: &[(&str, &str)],
) -> Result<()> {
    if !argv.iter().any(|arg| arg == "serve-ask") {
        return Ok(());
    }
    if tests::FAILED_ASK_LAUNCHERS.lock().unwrap().contains(name) {
        bail!("simulated Session launch failure");
    }
    if !tests::ASK_LAUNCHERS
        .lock()
        .unwrap()
        .insert(name.to_string())
    {
        bail!("simulated duplicate Session launcher");
    }
    Ok(())
}

pub(crate) fn flow_id(position: &FlowPosition) -> Result<String> {
    let step = position.current();
    let node_id = step
        .policy
        .id
        .as_deref()
        .ok_or_else(|| anyhow!("review flow position has no node id"))?;
    Ok(format!(
        "{}:{}:{}:{}:{}",
        position.task_id, position.invocation.id, step.flow, node_id, position.cursor.iteration
    ))
}

fn flow_token_id(token: &FlowSessionToken) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        token.task_id, token.invocation_id, token.flow, token.node_id, token.iteration
    )
}

pub(crate) fn lock_session_launch(id: &str) -> Result<File> {
    let directory = crate::store::current_home_lf_home_dir().join(LAUNCH_LOCK_DIRECTORY);
    fs::create_dir_all(&directory).context("create Session directory")?;
    let name = hex::encode(&Sha256::digest(id.as_bytes())[..16]);
    let path = directory.join(format!(".{name}.launch.lock"));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .context("open Session launch lock")?;
    FileExt::lock_exclusive(&file).context("lock Session launch")?;
    Ok(file)
}

fn flow_background_name(position: &FlowPosition) -> Result<String> {
    let step = position.current();
    Ok(flow_token_background_name(&FlowSessionToken {
        task_id: position.task_id.clone(),
        invocation_id: position.invocation.id.clone(),
        flow: step.flow,
        node_id: step
            .policy
            .id
            .ok_or_else(|| anyhow!("review flow position has no node id"))?,
        skill: match position.current_plan() {
            crate::engine::ConcreteStep::Skill(planned) => planned.skill.clone(),
            _ => bail!("review flow position does not select a Skill"),
        },
        iteration: position.cursor.iteration,
    }))
}

fn flow_token_background_name(token: &FlowSessionToken) -> String {
    let task = token
        .task_id
        .to_string()
        .chars()
        .skip(5)
        .take(8)
        .collect::<String>();
    let node = crate::engine::process::tmux_session_slug(&token.node_id)
        .chars()
        .take(24)
        .collect::<String>();
    let invocation = token.invocation_id.chars().take(8).collect::<String>();
    format!("lf-human-{task}-{node}-{invocation}-{}", token.iteration)
}

fn ask_background_name(id: &str) -> String {
    if let Some(key) = id.strip_prefix("ask_once_") {
        return format!("lf-human-once-{key}");
    }
    format!(
        "lf-human-{}",
        id.trim_start_matches("ask_")
            .chars()
            .take(12)
            .collect::<String>()
    )
}

fn active_run_manifest() -> Result<RunManifest> {
    let run_id = std::env::var(crate::durable::RUN_ID_ENV)
        .context("lf ask can only be called from a Loopflow Run")?;
    let run_dir = PathBuf::from(
        std::env::var_os(RUN_DIR_ENV).context("active Loopflow Run has no LF_RUN_DIR")?,
    );
    let bytes = fs::read(run_dir.join("manifest.json")).context("read active Run manifest")?;
    let manifest: RunManifest =
        serde_json::from_slice(&bytes).context("parse active Run manifest")?;
    if manifest.run_id.as_str() != run_id {
        bail!("active Run identity does not match its manifest");
    }
    Ok(manifest)
}

fn question_title(question: &str) -> String {
    let first = question
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("Request for input");
    let mut chars = first.trim().chars();
    let title = chars.by_ref().take(80).collect::<String>();
    if chars.next().is_some() {
        format!("{title}…")
    } else {
        title
    }
}

async fn wait_for_ask(mut store: Option<SharedStore>, id: &str, run_id: &RunId) -> Result<String> {
    loop {
        // While the rows wait beside the Run there is nothing to read, and a
        // waiting caller must not be the one to initialize a store.
        let reserved = unrecorded_session::read(run_id)?.is_some();
        if store.is_none() && !reserved {
            store = crate::store::open_existing_store()
                .await
                .map(std::sync::Arc::new);
        }
        if let Some(store) = store.as_ref().filter(|_| !reserved) {
            let (session, _) = store
                .session(id)
                .await?
                .ok_or_else(|| anyhow!("session {id:?} disappeared before resolution"))?;
            if session.completed_at.is_some() {
                return session
                    .ready_summary
                    .ok_or_else(|| anyhow!("Ask {id} completed without an answer"));
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

fn active_session_token() -> Result<HumanSessionToken> {
    let raw =
        std::env::var(HUMAN_SESSION_ENV).context("this command requires an active session")?;
    serde_json::from_str(&raw).context("active session token is invalid")
}

pub(crate) fn active_flow_skill(requested: &str) -> Result<Option<Skill>> {
    let Some(raw) = std::env::var_os(HUMAN_SESSION_ENV) else {
        return Ok(None);
    };
    let raw = raw
        .into_string()
        .map_err(|_| anyhow!("active session token is not valid UTF-8"))?;
    let token: HumanSessionToken =
        serde_json::from_str(&raw).context("active session token is invalid")?;
    match token {
        HumanSessionToken::StandaloneFlow { token } => {
            crate::ops::flow_session::pinned_skill(&token, requested).map(Some)
        }
        HumanSessionToken::Flow { token } => {
            if token.skill.name != requested {
                bail!("review session Skill does not match the requested Skill");
            }
            Ok(Some(token.skill))
        }
        HumanSessionToken::Ask { .. } => Ok(None),
    }
}

fn active_run_id() -> Result<RunId> {
    let value = std::env::var(crate::durable::RUN_ID_ENV)
        .context("this command requires an active Loopflow Run")?;
    RunId::parse(&value).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::ffi::OsString;
    use std::sync::{LazyLock, Mutex};
    use std::time::Duration;

    use clap::Parser;
    use sha2::Digest;

    use super::{
        active_flow_skill, ask, ask_background_name, ask_launch_args, ask_once,
        flow_background_name, flow_id, flow_token_id, human_open_argv, question_title, reserve_ask,
        serve_ask, session_run_is_resumable, token_matches, wait_for_ask, FlowSessionToken,
        HumanSessionToken, HUMAN_SESSION_ENV,
    };
    use crate::durable::{FlowPosition, RunId};
    use crate::engine::{prepare_launch_prompt, Config, LaunchPromptInput, Surface};
    use crate::lf::{Cli, Commands};
    use crate::ops::unrecorded_session;
    use crate::run_record::{
        preferred_work_selector, AttributionSource, RunManifest, SubjectAttribution,
    };
    use crate::session::{Run, Session, SessionKind};
    use crate::store::{open_ephemeral_store, SharedStore, StorageConfig};
    use crate::work::task::TaskId;

    pub(super) static ASK_LAUNCHERS: LazyLock<Mutex<HashSet<String>>> =
        LazyLock::new(|| Mutex::new(HashSet::new()));
    pub(super) static FAILED_ASK_LAUNCHERS: LazyLock<Mutex<HashSet<String>>> =
        LazyLock::new(|| Mutex::new(HashSet::new()));

    struct AskHome {
        home: tempfile::TempDir,
        previous: Vec<(&'static str, Option<OsString>)>,
        _ambient: crate::test_ambient::EnvGuard,
    }

    impl AskHome {
        fn new() -> Self {
            ASK_LAUNCHERS.lock().unwrap().clear();
            FAILED_ASK_LAUNCHERS.lock().unwrap().clear();
            let ambient = crate::test_ambient::EnvGuard::new();
            let home = tempfile::tempdir().unwrap();
            let previous = ["LF_HOME", "LF_BIN", "LF_DB_PATH"]
                .into_iter()
                .map(|name| {
                    let value = std::env::var_os(name);
                    std::env::remove_var(name);
                    (name, value)
                })
                .collect();
            std::env::set_var("LF_HOME", home.path());
            std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
            let manifest = RunManifest {
                schema_version: 1,
                run_id: RunId::new(),
                parent_run_id: None,
                created_at: time::OffsetDateTime::now_utc(),
                harness: "codex".to_string(),
                model: Some("test".to_string()),
                surface: "headless".to_string(),
                cwd: std::env::current_dir().unwrap(),
                repo: None,
                worktree: None,
                skill: Some("loop-decide".to_string()),
                subjects: Vec::new(),
                flow: None,
                launch: None,
                context: None,
                runtime_path: None,
                runtime_digest: None,
                host: "test".to_string(),
                boot_id: None,
            };
            std::fs::write(
                home.path().join("manifest.json"),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
            std::env::set_var("LF_RUN_DIR", home.path());
            std::env::set_var("LF_RUN_ID", manifest.run_id.as_str());
            Self {
                home,
                previous,
                _ambient: ambient,
            }
        }

        async fn store(&self) -> SharedStore {
            std::sync::Arc::new(
                open_ephemeral_store(&StorageConfig::sqlite(self.home.path().join("registry.db")))
                    .await
                    .unwrap(),
            )
        }
    }

    impl Drop for AskHome {
        fn drop(&mut self) {
            for (key, value) in self.previous.drain(..) {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
            ASK_LAUNCHERS.lock().unwrap().clear();
            FAILED_ASK_LAUNCHERS.lock().unwrap().clear();
        }
    }

    async fn open_asks(store: &SharedStore) -> Vec<(Session, Run)> {
        let mut open = store.open_conversations().await.unwrap();
        open.retain(|(session, _)| session.kind == SessionKind::Ask);
        open
    }

    async fn wait_until_asks(store: &SharedStore, count: usize) -> Vec<(Session, Run)> {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let sessions = open_asks(store).await;
                if sessions.len() == count && ASK_LAUNCHERS.lock().unwrap().len() == count {
                    return sessions;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }

    async fn answer(store: &SharedStore, session: &Session, run: &Run, summary: &str) {
        store
            .ready_session(&session.id, &run.id, summary)
            .await
            .unwrap();
        super::complete(store, &session.id).await.unwrap();
    }

    fn keyed_id(key: &str) -> String {
        format!(
            "ask_once_{}",
            hex::encode(super::Sha256::digest(key.as_bytes()))
        )
    }

    /// A stored Ask whose launch was consumed without provider history.
    async fn consumed_ask(store: &SharedStore, id: String, question: &str) -> (Session, Run) {
        let reserved = reserve_ask(Some(store), id, question, None).await.unwrap();
        let stored = unrecorded_session::record(store, &reserved.run.id)
            .await
            .unwrap()
            .unwrap();
        std::fs::remove_file(run_dir(&stored.1).join("prepared")).unwrap();
        stored
    }

    fn run_dir(run: &Run) -> std::path::PathBuf {
        super::local_session_run_dir(&run.id).unwrap()
    }

    #[test]
    fn keyed_asks_join_recover_completion_and_keep_boundaries_independent() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let caller = std::env::var("LF_RUN_ID").unwrap();
            let key = "/private/checkout/invocation/decision/visit-1";
            let first = ask_once(&store, key, "Choose a policy", Some("unblock"));
            let duplicate = ask_once(&store, key, "Changed retry text", Some("unblock"));
            let independent =
                ask_once(&store, "invocation/decision/visit-2", "Second choice", None);
            let human = async {
                for (session, run) in wait_until_asks(&store, 2).await {
                    assert!(session.id.starts_with("ask_once_"));
                    assert!(!session.id.contains("checkout"));
                    assert_eq!(run.caller_run_id.as_ref().unwrap().as_str(), caller);
                    assert_eq!(run.invocation_id, None);
                    assert_eq!(
                        (run.provider.as_deref(), run.model.as_deref()),
                        (Some("codex"), Some("test"))
                    );
                    let summary = if session.request.as_deref() == Some("Second choice") {
                        "second"
                    } else {
                        assert_eq!(session.request.as_deref(), Some("Choose a policy"));
                        assert_eq!(run.skill.as_deref(), Some("unblock"));
                        "first"
                    };
                    answer(&store, &session, &run, summary).await;
                }
            };
            let (first, duplicate, independent, ()) =
                tokio::join!(first, duplicate, independent, human);
            assert_eq!(first.unwrap(), "first");
            assert_eq!(duplicate.unwrap(), "first");
            assert_eq!(independent.unwrap(), "second");
            assert!(open_asks(&store).await.is_empty());
            // Completed recovery must not reload a now-missing selected skill.
            assert_eq!(
                ask_once(&store, key, "", Some("missing-skill"))
                    .await
                    .unwrap(),
                "first"
            );
            assert_eq!(ASK_LAUNCHERS.lock().unwrap().len(), 2);
            assert_eq!(store.session_runs(&keyed_id(key)).await.unwrap().len(), 1);
        });
    }

    #[test]
    fn keyed_ask_failed_launch_can_retry_the_same_session() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let key = "failed-launch-boundary";
            let id = keyed_id(key);
            FAILED_ASK_LAUNCHERS
                .lock()
                .unwrap()
                .insert(ask_background_name(&id));
            let error = ask_once(&store, key, "Preserved question", None)
                .await
                .unwrap_err();
            assert!(error.to_string().contains(&id));
            let (preserved, first_run) = store.session(&id).await.unwrap().unwrap();
            assert_eq!(preserved.request.as_deref(), Some("Preserved question"));
            FAILED_ASK_LAUNCHERS.lock().unwrap().clear();
            let human = async {
                let sessions = wait_until_asks(&store, 1).await;
                assert_eq!(sessions[0].0.id, id);
                assert_eq!(sessions[0].1.id, first_run.id);
                answer(&store, &sessions[0].0, &sessions[0].1, "Recovered").await;
            };
            let (result, ()) = tokio::join!(ask_once(&store, key, "replacement", None), human);
            assert_eq!(result.unwrap(), "Recovered");
            assert_eq!(
                wait_for_ask(Some(store.clone()), &id, &first_run.id)
                    .await
                    .unwrap(),
                "Recovered"
            );
        });
    }

    #[test]
    fn keyed_ask_does_not_replace_a_published_native_run() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let key = "published-run-boundary";
            let (session, run) = consumed_ask(&store, keyed_id(key), "Keep native ownership").await;
            // No tmux launcher and no native history do not grant replacement
            // authority. Even a delayed serve command must join.
            serve_ask(Some(store.clone()), &run.id).await.unwrap();
            assert!(tokio::time::timeout(
                Duration::from_millis(100),
                ask_once(&store, key, "retry", None)
            )
            .await
            .is_err());
            let (_, current) = store.session(&session.id).await.unwrap().unwrap();
            assert_eq!(current.id, run.id);
            assert!(ASK_LAUNCHERS.lock().unwrap().is_empty());
            std::env::set_var("LF_RUN_ID", run.id.as_str());
            std::env::set_var(
                HUMAN_SESSION_ENV,
                serde_json::to_string(&HumanSessionToken::Ask {
                    id: session.id.clone(),
                })
                .unwrap(),
            );
            super::mark_ready(&store, "Resolved").await.unwrap();
            super::complete(&store, &session.id).await.unwrap();
            assert!(super::mark_ready(&store, "Late readiness").await.is_err());
            assert_eq!(
                wait_for_ask(Some(store.clone()), &session.id, &run.id)
                    .await
                    .unwrap(),
                "Resolved"
            );
            assert!(open_asks(&store).await.is_empty());
        });
    }

    #[test]
    fn reopening_ask_cannot_overwrite_a_concurrent_completion() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let (session, run) =
                consumed_ask(&store, "ask_reopening".to_string(), "Keep the answer").await;
            store
                .ready_session(&session.id, &run.id, "Accepted direction")
                .await
                .unwrap();
            let lock = super::lock_session_launch(&session.id).unwrap();
            let mut reopening = Box::pin(super::open_ask(&store, &session.id));
            assert!(
                tokio::time::timeout(Duration::from_millis(200), reopening.as_mut())
                    .await
                    .is_err()
            );
            let (untouched, current) = store.session(&session.id).await.unwrap().unwrap();
            assert_eq!(current.id, run.id);
            assert_eq!(
                untouched.ready_summary.as_deref(),
                Some("Accepted direction")
            );

            store.complete_session(&session.id, &run.id).await.unwrap();
            drop(lock);

            assert!(reopening
                .await
                .unwrap_err()
                .to_string()
                .contains("already complete"));
            assert_eq!(
                wait_for_ask(Some(store.clone()), &session.id, &run.id)
                    .await
                    .unwrap(),
                "Accepted direction"
            );
            assert_eq!(store.session_runs(&session.id).await.unwrap().len(), 1);
            assert!(ASK_LAUNCHERS.lock().unwrap().is_empty());
        });
    }

    #[test]
    fn session_stop_requires_a_readable_run_manifest() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        let run_id = RunId::new();
        assert!(super::stop_run(&run_id)
            .unwrap_err()
            .to_string()
            .contains("manifest unavailable"));
        let dir = crate::run_record::record_dir(home.home.path(), &run_id).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("manifest.json"), b"invalid manifest").unwrap();
        assert!(super::stop_run(&run_id)
            .unwrap_err()
            .to_string()
            .contains("manifest unavailable"));
        assert_eq!(
            std::fs::read(dir.join("manifest.json")).unwrap(),
            b"invalid manifest"
        );
    }

    #[test]
    fn ask_completion_survives_native_cleanup_failure() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let (session, run) = consumed_ask(
                &store,
                "ask_cleanup".to_string(),
                "Resolve stalled progress",
            )
            .await;
            let manifest = run_dir(&run).join("manifest.json");
            std::fs::write(&manifest, b"invalid manifest").unwrap();

            answer(&store, &session, &run, "Try the narrower proof").await;

            assert_eq!(
                wait_for_ask(Some(store.clone()), &session.id, &run.id)
                    .await
                    .unwrap(),
                "Try the narrower proof"
            );
            assert!(open_asks(&store).await.is_empty());
            assert_eq!(std::fs::read(manifest).unwrap(), b"invalid manifest");
        });
    }

    #[test]
    fn ordinary_ask_returns_its_answer_and_stays_a_closed_session() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let human = async {
                let (session, run) = wait_until_asks(&store, 1).await.remove(0);
                assert!(unrecorded_session::read(&run.id).unwrap().is_none());
                answer(&store, &session, &run, "Ordinary answer").await;
                session.id
            };
            let (result, id) =
                tokio::join!(ask(Some(store.clone()), "Plain question", None), human);
            assert_eq!(result.unwrap(), "Ordinary answer");
            let (closed, _) = store.session(&id).await.unwrap().unwrap();
            assert!(closed.completed_at.is_some());
            assert_eq!(closed.ready_summary.as_deref(), Some("Ordinary answer"));
            assert!(!home
                .home
                .path()
                .join("human-sessions")
                .join(format!("{id}.json"))
                .exists());
        });
    }

    #[test]
    fn session_action_fixtures_match_the_shared_boundary() {
        #[derive(serde::Deserialize)]
        struct Case {
            kind: super::SessionKind,
            state: super::SessionState,
            actions: Vec<super::SessionAction>,
        }
        let cases: Vec<Case> = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/session_actions.json"
        ))
        .unwrap();
        for case in cases {
            assert_eq!(super::session_actions(case.kind, case.state), case.actions);
            for action in case.actions {
                let outcome = super::require_session_action(case.kind, case.state, action.kind);
                assert_eq!(
                    outcome.err().map(|error| error.to_string()),
                    action.unavailable_reason
                );
            }
        }
        for fixture in [
            include_str!("../../../../tests/fixtures/dto/session.json"),
            &serde_json::from_str::<serde_json::Value>(include_str!(
                "../../../../tests/fixtures/dto/sessions.json"
            ))
            .unwrap()[0]
                .to_string(),
        ] {
            let session: super::SessionRecord = serde_json::from_str(fixture).unwrap();
            assert_eq!(
                session.actions,
                super::session_actions(session.kind, session.state)
            );
            assert_eq!(session.work_path.as_deref(), Some("product / LOO-291"));
            let mut untitled = serde_json::to_value(&session).unwrap();
            untitled.as_object_mut().unwrap().remove("title_source");
            assert!(serde_json::from_value::<super::SessionRecord>(untitled).is_err());
            assert_eq!(
                serde_json::from_value::<super::SessionRecord>(
                    serde_json::to_value(&session).unwrap()
                )
                .unwrap(),
                session
            );
        }
    }

    #[test]
    fn flow_membership_fixtures_cover_every_projection_and_are_required() {
        let sessions: Vec<super::SessionRecord> = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/session_memberships.json"
        ))
        .unwrap();
        let mut kinds = Vec::new();
        for session in &sessions {
            assert_eq!(
                session.actions,
                super::session_actions(session.kind, session.state)
            );
            kinds.push(match &session.flow_membership {
                super::SessionFlowMembership::Step { occurrence, .. } => match occurrence {
                    super::SessionFlowOccurrence::Current => "current",
                    super::SessionFlowOccurrence::Earlier => "earlier",
                    super::SessionFlowOccurrence::Past => "past",
                },
                super::SessionFlowMembership::Independent => "independent",
                super::SessionFlowMembership::Unknown { .. } => "unknown",
            });
            let mut value = serde_json::to_value(session).unwrap();
            assert_eq!(
                serde_json::from_value::<super::SessionRecord>(value.clone()).unwrap(),
                *session
            );
            value.as_object_mut().unwrap().remove("flow_membership");
            assert!(serde_json::from_value::<super::SessionRecord>(value).is_err());
        }
        assert_eq!(
            kinds,
            [
                "current",
                "earlier",
                "independent",
                "unknown",
                "past",
                "past",
                "current"
            ]
        );
        // An older capture keeps its Flow provenance with no node or tuple.
        assert!(matches!(
            &sessions[5].flow_membership,
            super::SessionFlowMembership::Step {
                node: None,
                iterations: None,
                ..
            }
        ));
        assert_eq!(
            sessions.last().unwrap().title_source,
            super::SessionTitleSource::Unavailable
        );
        // A remote Session's Run manifest is not here: no provider is claimed.
        assert_eq!(sessions.last().unwrap().provider, None);
        assert_eq!(sessions[0].provider.as_deref(), Some("codex"));
    }

    #[test]
    fn every_session_kind_requires_its_own_run_reference() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../../tests/fixtures/dto/session.json"))
                .unwrap();
        for kind in ["interactive", "ask", "flow"] {
            let mut value = fixture.clone();
            value["kind"] = kind.into();
            let session: super::SessionRecord = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(
                session.run_id.as_str(),
                "run_00000000000000000000000000000001"
            );
            value.as_object_mut().unwrap().remove("run_id");
            assert!(serde_json::from_value::<super::SessionRecord>(value.clone()).is_err());
            value["run_id"] = serde_json::Value::Null;
            assert!(serde_json::from_value::<super::SessionRecord>(value).is_err());
        }
    }

    #[tokio::test]
    async fn session_work_labels_follow_stable_ancestry_without_a_roadmap() {
        let directory = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                directory.path().join("test.db"),
            ))
            .await
            .unwrap(),
        );
        let wave = crate::work::wave::Wave::new(
            crate::id::WaveId::new(),
            "product".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        assert_eq!(
            super::session_work_path(
                &store,
                Some(&crate::durable::WorkRef::Wave(wave.id().clone()))
            )
            .await
            .unwrap()
            .as_deref(),
            Some("product")
        );
        let now = time::OffsetDateTime::now_utc();
        let project = crate::work::project::Project {
            id: crate::durable::ProjectId::new(),
            plan: crate::planning::ProjectPlan {
                id: crate::planning::LinearProjectId::new("historical-project").unwrap(),
                slug: "previous-chapter".to_string(),
                name: "Obsolete public tier".to_string(),
                prompt_context: String::new(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            iteration: 0,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store.create_project(&project).await.unwrap();
        let subject = crate::durable::WorkRef::Project(project.id.clone());
        let mut manifest = RunManifest {
            schema_version: 1,
            run_id: crate::durable::RunId::new(),
            parent_run_id: None,
            created_at: now,
            harness: "fixture".to_string(),
            model: None,
            surface: "tui".to_string(),
            cwd: directory.path().to_path_buf(),
            repo: None,
            worktree: None,
            skill: None,
            subjects: Vec::new(),
            launch: None,
            context: None,
            runtime_path: None,
            runtime_digest: None,
            host: "fixture".to_string(),
            boot_id: None,
            flow: None,
        };
        for selector in [&project.plan.slug, project.plan.id.as_str()] {
            manifest.subjects = vec![SubjectAttribution::declared(format!("project:{selector}"))];
            assert_eq!(
                crate::run_record::attributed_work(&store, &manifest).await,
                Some(subject.clone())
            );
        }
        assert_eq!(
            super::session_wave_id(&store, Some(&subject))
                .await
                .unwrap(),
            Some(wave.id().clone())
        );
        assert_eq!(
            super::session_work_path(&store, Some(&subject))
                .await
                .unwrap()
                .as_deref(),
            Some("product")
        );
        assert_eq!(super::session_work_path(&store, None).await.unwrap(), None);
        let missing = crate::work::task::TaskId::new();
        assert_eq!(
            super::session_work_path(
                &store,
                Some(&crate::durable::WorkRef::Task(missing.clone()))
            )
            .await
            .unwrap(),
            Some(format!("Task {missing} (unavailable)"))
        );
    }

    fn position() -> FlowPosition {
        FlowPosition {
            task_id: TaskId::new(),
            invocation: crate::durable::test_flow_invocation(
                "review",
                1,
                "review-design",
                Some("review_kickoff"),
                true,
            ),
            session_run_id: None,
            ready_summary: None,
            cursor: crate::engine::ExecutionCursor {
                index: 1,
                iteration: 3,
                ..Default::default()
            },
            version: 0,
            worker_generation: 0,
            claim: None,
            failure: None,

            updated_at: time::OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn nested_session_membership_joins_its_exact_graph_occurrence() {
        use crate::engine::flow::{ConcretePath, ConcreteStep, ConcreteXor, RepeatPolicy, Skill};
        use crate::engine::flow_graph::FlowGraph;
        use crate::engine::{ExecutionCursor, NestedCursor};
        use crate::run_record::RunFlowStep;

        let mut position = position();
        let mut review = position.invocation.steps[1].clone();
        let ConcreteStep::Skill(start) = &mut review else {
            panic!("skill")
        };
        start.policy.id = Some("begin".into());
        let mut decide = review.clone();
        let ConcreteStep::Skill(end) = &mut decide else {
            panic!("skill")
        };
        end.policy.id = Some("decide".into());
        end.policy.repeat = Some(RepeatPolicy {
            from: "begin".into(),
        });
        position.invocation.steps = vec![
            review.clone(),
            ConcreteStep::Xor(ConcreteXor {
                router: Skill::named("route"),
                paths: std::collections::HashMap::from([(
                    "fix".into(),
                    ConcretePath {
                        description: "Revision".into(),
                        steps: vec![review.clone(), decide.clone()],
                    },
                )]),
                flow_parents: Vec::new(),
            }),
            decide,
        ];
        position.cursor.progress.repeats.insert("decide".into(), 5);
        position.cursor.child = Some(Box::new(NestedCursor::Xor {
            selected: "fix".into(),
            cursor: ExecutionCursor {
                index: 1,
                iteration: 99,
                progress: crate::engine::transitions::FlowProgress {
                    repeats: std::collections::BTreeMap::from([("decide".into(), 2)]),
                    ..Default::default()
                },
                ..Default::default()
            },
        }));
        let graph = FlowGraph::new("review", &position.invocation.steps);
        let expected = &graph.steps[1].paths[0].steps[1].key;
        assert_eq!(expected, "1/fix/1");
        let super::SessionFlowMembership::Step {
            node, iterations, ..
        } = super::SessionFlowMembership::of_position(&position)
        else {
            panic!("step")
        };
        assert_eq!(node.as_ref(), Some(expected));
        assert_eq!(iterations, Some(vec![vec![5], vec![2]]));
        let captured = RunFlowStep::of(&position, None);
        assert_eq!(captured.node.as_ref(), Some(expected));
        assert_eq!(captured.iterations, iterations);

        // Old captures keep their known Flow provenance without fabricating a
        // structural node from the former leaf index.
        let mut old = serde_json::to_value(&captured).unwrap();
        old.as_object_mut().unwrap().remove("node");
        old.as_object_mut().unwrap().remove("iterations");
        old["iteration"] = serde_json::json!(3);
        old["step_index"] = serde_json::json!(1);
        let old: RunFlowStep = serde_json::from_value(old).unwrap();
        assert_eq!(old.node, None);
        assert_eq!(old.iterations, None);
        assert_eq!(old.invocation_id, captured.invocation_id);
    }

    #[test]
    fn session_identity_is_the_exact_human_flow_position() {
        let position = position();
        let step = position.current();
        let token = FlowSessionToken {
            task_id: position.task_id.clone(),
            invocation_id: position.invocation.id.clone(),
            flow: step.flow,
            node_id: step.policy.id.unwrap(),
            skill: match position.current_plan() {
                crate::engine::ConcreteStep::Skill(planned) => planned.skill.clone(),
                _ => panic!("human position must select a Skill"),
            },
            iteration: position.cursor.iteration,
        };

        assert!(token_matches(&token, &position));
        assert!(flow_id(&position).unwrap().contains("review_kickoff"));
        assert_eq!(flow_id(&position).unwrap(), flow_token_id(&token));
        assert!(flow_background_name(&position)
            .unwrap()
            .starts_with("lf-human-"));
    }

    #[test]
    fn human_session_carries_the_started_skill_definition() {
        let _lock = crate::journal::test_env_lock();
        let mut position = position();
        let crate::engine::ConcreteStep::Skill(planned) =
            &mut position.invocation.steps[position.cursor.index]
        else {
            panic!("human position must select a Skill")
        };
        planned.skill.content = Some("started instructions".to_string());
        let planned_skill = planned.skill.clone();
        let step = position.current();
        let token = FlowSessionToken {
            task_id: position.task_id.clone(),
            invocation_id: position.invocation.id.clone(),
            flow: step.flow,
            node_id: step.policy.id.unwrap(),
            skill: planned_skill,
            iteration: position.cursor.iteration,
        };
        let previous = std::env::var_os(HUMAN_SESSION_ENV);
        std::env::set_var(
            HUMAN_SESSION_ENV,
            serde_json::to_string(&HumanSessionToken::Flow {
                token: Box::new(token),
            })
            .unwrap(),
        );

        let skill = active_flow_skill("review-design").unwrap().unwrap();

        match previous {
            Some(value) => std::env::set_var(HUMAN_SESSION_ENV, value),
            None => std::env::remove_var(HUMAN_SESSION_ENV),
        }
        assert_eq!(skill.content.as_deref(), Some("started instructions"));
    }

    #[test]
    fn ask_titles_are_meaningful_and_bounded() {
        assert_eq!(
            question_title("\nReview this branch\nmore"),
            "Review this branch"
        );
        assert!(question_title(&"x".repeat(100)).ends_with('…'));
    }

    #[test]
    fn ask_skill_and_question_reach_the_launch_prompt() {
        let _lock = crate::journal::test_env_lock();
        let checkout = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(checkout.path().join(".lf/skills")).unwrap();
        std::fs::write(
            checkout.path().join(".lf/skills/unblock.md"),
            "Resolve the blocker with the human using the preserved evidence.",
        )
        .unwrap();
        let run = Run {
            id: RunId::new(),
            session_id: Some("ask_skill_proof".to_string()),
            invocation_id: None,
            node: None,
            iterations: None,
            attempt: None,
            task_id: None,
            wave_id: None,
            work_source: None,
            created_at: 1,
            published: true,
            cwd: checkout.path().to_path_buf(),
            skill: Some("unblock".to_string()),
            provider: Some("claude".to_string()),
            model: Some("opus".to_string()),
            caller_run_id: Some(RunId::new()),
        };
        let session = Session {
            id: "ask_skill_proof".to_string(),
            current_run_id: run.id.clone(),
            kind: SessionKind::Ask,
            title: "Choose the delivery policy".to_string(),
            title_source: crate::session::TitleSource::Generated,
            request: Some("Choose the delivery policy".to_string()),
            ready_summary: Some("Discussed the policy".to_string()),
            completed_at: None,
            created_at: 1,
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();

        let args = runtime.block_on(ask_launch_args(None, &session, &run));
        let cli = Cli::try_parse_from(std::iter::once("lf".to_string()).chain(args)).unwrap();
        let Some(Commands::Skill { name, args }) = cli.command else {
            panic!("selected Ask must use the ordinary skill command")
        };
        let prepared = prepare_launch_prompt(
            &Config {
                diff: false,
                diff_files: false,
                paste: false,
                ..Config::default()
            },
            LaunchPromptInput {
                repo_root: run.cwd.clone(),
                cwd: Some(run.cwd.clone()),
                skill: Some(name),
                message: Some(args.join(" ")),
                agent: Some(super::launch_model(&run)),
                surface: Surface::Cli,
                ..LaunchPromptInput::default()
            },
        )
        .unwrap();
        assert!(prepared
            .prompt
            .contains("Resolve the blocker with the human using the preserved evidence."));
        assert!(prepared.prompt.contains("Choose the delivery policy"));
        assert!(prepared
            .prompt
            .contains("Ready keeps this session visible and does not resume the caller"));
        assert_eq!(prepared.config.cwd.as_deref(), Some(checkout.path()));

        let inline = Run { skill: None, ..run };
        let args = runtime.block_on(ask_launch_args(None, &session, &inline));
        let cli = Cli::try_parse_from(std::iter::once("lf".to_string()).chain(args)).unwrap();
        assert!(matches!(cli.command, Some(Commands::Inline { .. })));
    }

    #[test]
    fn human_sessions_open_through_the_public_session_command() {
        let _lock = crate::journal::test_env_lock();
        let previous_lf_bin = std::env::var_os("LF_BIN");
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let argv = human_open_argv(None, None, "ask_123");
        match previous_lf_bin {
            Some(value) => std::env::set_var("LF_BIN", value),
            None => std::env::remove_var("LF_BIN"),
        }
        let argv = argv.unwrap();

        assert_eq!(&argv[argv.len() - 3..], ["session", "open", "ask_123"]);
        assert!(!argv.iter().any(|argument| argument == "tmux"));
    }

    #[test]
    fn task_is_the_most_specific_parent_run_subject() {
        let manifest = RunManifest {
            schema_version: 1,
            run_id: crate::durable::RunId::new(),
            parent_run_id: None,
            created_at: time::OffsetDateTime::now_utc(),
            harness: "codex".to_string(),
            model: None,
            surface: "headless".to_string(),
            cwd: "/tmp/worktree".into(),
            repo: None,
            worktree: None,
            skill: Some("implement".to_string()),
            subjects: vec![
                SubjectAttribution {
                    selector: "wave:product".to_string(),
                    source: AttributionSource::Declared,
                },
                SubjectAttribution {
                    selector: "task:task_123".to_string(),
                    source: AttributionSource::Declared,
                },
            ],
            launch: None,
            context: None,
            runtime_path: None,
            runtime_digest: None,
            host: "test".to_string(),
            boot_id: None,
            flow: None,
        };
        assert_eq!(
            preferred_work_selector(&manifest).as_deref(),
            Some("task:task_123")
        );
    }

    #[test]
    fn initial_session_publication_requires_history_and_an_owned_client() {
        let dir = tempfile::tempdir().unwrap();
        let harness = std::env::current_exe()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let manifest = RunManifest {
            schema_version: 1,
            run_id: crate::durable::RunId::new(),
            parent_run_id: None,
            created_at: time::OffsetDateTime::now_utc(),
            harness,
            model: None,
            surface: "tui".to_string(),
            cwd: dir.path().into(),
            repo: None,
            worktree: None,
            skill: Some("review-design".to_string()),
            subjects: Vec::new(),
            launch: None,
            context: None,
            runtime_path: None,
            runtime_digest: None,
            host: "test".to_string(),
            boot_id: None,
            flow: None,
        };
        std::fs::write(
            dir.path().join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();

        assert!(!session_run_is_resumable(dir.path(), &manifest).unwrap());
        crate::run_record::write_provider_session(dir.path(), "provider-session", None).unwrap();
        assert!(!session_run_is_resumable(dir.path(), &manifest).unwrap());
        crate::run_record::write_provider_client(dir.path(), std::process::id()).unwrap();
        assert!(session_run_is_resumable(dir.path(), &manifest).unwrap());
    }
}
