use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::durable::{FlowSession, RunId, WorkRef};
use crate::engine::Skill;
use crate::run_record::{ProviderSessionRef, RunManifest, SessionTitleSource, RUN_DIR_ENV};
use crate::session::{AgentSession, WorkSource};
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
    let session = store
        .session(&reservation.session_id)?
        .ok_or_else(|| anyhow!("review reservation disappeared"))?;
    let task_id = session
        .task_id
        .as_ref()
        .ok_or_else(|| anyhow!("review reservation has no Task"))?;
    let position = store
        .task_flow(task_id)?
        .ok_or_else(|| anyhow!("review invocation is no longer current"))?;
    if session.input_id != reservation.run_id
        || session.completed_at.is_some()
        || session.input_published
        || position.version != reservation.version
        || flow_id(&position)? != session.id
    {
        bail!("review Run reservation is stale");
    }
    let mut membership = crate::run_record::RunFlowStep::of(&position)?;
    membership.task_pr_id = store.active_task_pr(task_id)?.map(|pr| pr.id);
    Ok(Some((
        reservation.run_id,
        crate::run_record::RunFlowMembership::Step(membership),
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
    Flow { token: Box<FlowSessionToken> },
    Ask { id: String },
    StandaloneFlow { id: String },
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
    Conversation,
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
    let active_client = kind == SessionKind::Conversation && state == SessionState::Active;
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
        SessionKind::Conversation => {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub run_id: RunId,
    pub kind: SessionKind,
    pub interactive: bool,
    pub work: Option<WorkRef>,
    pub wave_id: Option<crate::id::WaveId>,
    pub work_path: Option<String>,
    pub actions: Vec<SessionAction>,
    pub title: String,
    pub title_source: SessionTitleSource,
    pub flow_membership: SessionFlowMembership,
    pub detail: String,
    /// The provider its current Run launched with; absent until a review's
    /// first launch records one.
    pub provider: Option<String>,
    pub cwd: String,
    pub state: SessionState,
    pub ready_summary: Option<String>,
    pub open_argv: Vec<String>,
    pub terminal_ids: Vec<String>,
}

/// Whether a Session's conversation is an occurrence of a Flow. Membership
/// comes only from its Run's invocation, never from matching Task, checkout,
/// provider, or skill.
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

/// One resolved Session. Every operation looks a Session up by its id, or by
/// its linked Run id, through `find_session`.
#[derive(Debug)]
enum SessionTarget {
    /// An interactive Session, an Ask, or a saved Flow's review.
    Row { session: Box<AgentSession> },
    /// A Task's review also settles its Task's invocation.
    Flow {
        task: Box<Task>,
        position: Box<FlowSession>,
        run_id: RunId,
    },
}

pub(crate) async fn ask(
    store: &SharedStore,
    question: &str,
    skill: Option<&str>,
) -> Result<String> {
    let id = format!("ask_{}", uuid::Uuid::new_v4().simple());
    let session = reserve_ask(store, id, question, skill).await?;
    launch_ask(&session).await?;
    report_ask_wait(&session.id);
    wait_for_ask(store, &session.id).await
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
    let id = keyed_ask_id(key)?;
    let session = launch_keyed_ask(store, id.clone(), |id| {
        reserve_ask(store, id, question, skill)
    })
    .await?;
    if session.completed_at.is_none() {
        report_ask_wait(&id);
    }
    wait_for_ask(store, &id).await
}

/// Open the keyed Session once: the first caller stores it, every caller
/// launches it if no launcher holds it, and a completed one is returned as is.
async fn launch_keyed_ask<F: Future<Output = Result<AgentSession>>>(
    store: &SharedStore,
    id: String,
    reserve: impl FnOnce(String) -> F,
) -> Result<AgentSession> {
    // The server holds this lock through provider publication. Wait off the
    // async runtime so concurrent callers can still finish their launches.
    let lock_id = id.clone();
    let launch_lock = tokio::task::spawn_blocking(move || lock_session_launch(&lock_id)).await??;
    let session = match store.session(&id).await? {
        Some(found) => found,
        None => reserve(id.clone()).await?,
    };
    if session.completed_at.is_none()
        && run_is_prepared(&session.input_id)?
        && !ask_launcher_is_running(&id).await?
    {
        // Keep the Session on failure. A retry can start the same Session;
        // a published native Run is reopened only through `lf session open`.
        launch_ask(&session).await.with_context(|| {
            format!("launch human Ask {id}; retry the same boundary to recover")
        })?;
    }
    drop(launch_lock);
    Ok(session)
}

fn keyed_ask_id(key: &str) -> Result<String> {
    anyhow::ensure!(
        !key.trim().is_empty(),
        "Ask continuation key cannot be empty"
    );
    Ok(format!(
        "ask_once_{}",
        hex::encode(Sha256::digest(key.as_bytes()))
    ))
}

/// The unblock Session of a Task decision that failed: the same Session the
/// deciding Run opens with `lf flow blocked`, keyed by the position. Returns
/// its id and, once the human completed it, the feedback.
pub(crate) async fn task_unblock(
    store: &SharedStore,
    task: &Task,
    position: &FlowSession,
) -> Result<(String, Option<String>)> {
    let failure = position
        .failure
        .as_ref()
        .ok_or_else(|| anyhow!("Task is not blocked"))?;
    let failed = failure
        .run_id
        .clone()
        .ok_or_else(|| anyhow!("Task blocker has no Run"))?;
    // A replacement invocation must not acquire an obsolete unblock Session.
    anyhow::ensure!(
        store.task_flow(&task.id).await?.as_ref() == Some(position),
        "Task failure changed before opening unblock"
    );
    let id = keyed_ask_id(&task_unblock_key(position)?)?;
    let session = launch_keyed_ask(store, id, |id| async move {
        let agent = crate::ops::task::resolve_task_agent(&task.worktree, task.agent.as_deref(), None);
        let (provider, model) = crate::engine::config::parse_agent(&agent);
        let request = format!("{}\n\nFailed Run: {failed}. Resolve the blocker and leave feedback for reassessment. Completion does not choose Advance or Iterate.", failure.reason);
        let session = AgentSession {
            caller_input_id: None,
            id, input_id: RunId::new(), input_published: true,
            cwd: task.worktree.clone(), skill: Some("unblock".into()), provider: Some(provider), model,
            node: None, iterations: None, task_id: Some(task.id.clone()), wave_id: Some(task.wave_id.clone()),
            flow_session_id: None, work_source: Some(WorkSource::Inherited), bound_at: None,
            kind: crate::session::SessionKind::Ask, interactive: true, repo: None,
            title: question_title(&failure.reason), title_source: crate::session::TitleSource::Generated,
            request: Some(request), ready_summary: None, completed_at: None, created_at: crate::store::rows::now_unix(),
        };
        let session = prepare_input(session, crate::run_record::RunFlowMembership::Independent, Some(failed.clone()))?;
        Ok(store.create_session(session, None).await?)
    })
    .await?;
    let feedback = session
        .completed_at
        .is_some()
        .then_some(session.ready_summary)
        .flatten();
    Ok((session.id, feedback))
}

pub(crate) fn task_unblock_key(position: &FlowSession) -> Result<String> {
    position.blocker_key()
}

/// The unblock Session the current decision Run waits on, if one is open.
pub(crate) async fn task_waiting_unblock(
    store: &SharedStore,
    position: &FlowSession,
) -> Result<Option<String>> {
    if !position.is_decision() {
        return Ok(None);
    }
    let Some(run) = position
        .claim
        .as_ref()
        .and_then(|_| position.session_run_id())
    else {
        return Ok(None);
    };
    let Some(session) = store
        .session(&keyed_ask_id(&task_unblock_key(position)?)?)
        .await?
    else {
        return Ok(None);
    };
    Ok((session.caller_input_id.as_ref() == Some(run) && session.completed_at.is_none()).then(|| {
        format!(
            "Run {run} is waiting for unblock Session {}: {}. Complete the Session; the same decision Run receives feedback and reassesses without Task resume.",
            session.id, session.title
        )
    }))
}

fn report_ask_wait(id: &str) {
    eprintln!(
        "Waiting for human session {id}. Open it in Loopflow or with `lf session open {id}`."
    );
}

/// Store an Ask's Session with its first Run prepared. Work and the caller
/// come from the asking Run; its invocation never does.
async fn reserve_ask(
    store: &SharedStore,
    id: String,
    question: &str,
    skill: Option<&str>,
) -> Result<AgentSession> {
    let question = question.trim();
    if question.is_empty() {
        bail!("question cannot be empty");
    }
    let caller = active_run_manifest()?;
    let cwd = std::env::current_dir().context("resolve Ask working directory")?;
    if let Some(skill) = skill {
        crate::engine::load_skill(skill, &cwd).context("load Ask skill")?;
    }
    let current_work = match crate::journal::current_exec_id() {
        Some(exec) => store.agent_work(&exec).await?,
        None => None,
    };
    // A continuing provider inherits its Session's present assignment. Old
    // launch evidence alone still describes only its historical assignment.
    let (task_id, wave_id) = match current_work {
        Some(work) => (work.task_id, work.wave_id),
        None => match store.session_for_run(&caller.run_id).await? {
            Some(run) => (run.task_id, run.wave_id),
            None => (None, None),
        },
    };
    if let Some(task_id) = &task_id {
        let task = store
            .get_task(task_id)
            .await?
            .ok_or_else(|| anyhow!("Task {task_id} is not registered"))?;
        if store
            .task_deletion(&task.wave_id, task.plan.id.as_str())
            .await?
            .is_some()
        {
            bail!(
                "Task {} was deleted and cannot ask for new execution",
                task.plan.identifier
            );
        }
    }
    let session = AgentSession {
        caller_input_id: None,
        id,
        input_id: RunId::new(),
        input_published: true,
        cwd,
        skill: skill.map(str::to_string),
        provider: Some(caller.harness),
        model: caller.model,
        node: None,
        iterations: None,
        work_source: wave_id.as_ref().map(|_| WorkSource::Inherited),
        task_id,
        wave_id,
        flow_session_id: None,
        bound_at: None,
        kind: crate::session::SessionKind::Ask,
        interactive: true,
        repo: None,
        title: question_title(question),
        title_source: crate::session::TitleSource::Generated,
        request: Some(question.to_owned()),
        ready_summary: None,
        completed_at: None,
        created_at: crate::store::rows::now_unix(),
    };
    let session = prepare_input(
        session,
        crate::run_record::RunFlowMembership::Independent,
        Some(caller.run_id),
    )?;
    Ok(store.create_session(session, None).await?)
}

/// Capture immutable prompt input. Retries preserve earlier artifact references.
pub(crate) fn prepare_input(
    mut session: AgentSession,
    flow: crate::run_record::RunFlowMembership,
    caller: Option<RunId>,
) -> Result<AgentSession> {
    let caller = caller.or_else(|| session.caller_input_id.clone());
    session.caller_input_id = caller.clone();
    session.input_id = crate::run_record::CaptureHandle::prepare(
        crate::run_record::RunSpec {
            harness: session.provider.clone().unwrap_or_default(),
            model: session.model.clone(),
            surface: "tui".to_owned(),
            cwd: session.cwd.clone(),
            repo: None,
            worktree: Some(session.cwd.clone()),
            skill: session.skill.clone(),
            subjects: work_selector(&session)
                .map(crate::run_record::SubjectAttribution::declared)
                .into_iter()
                .collect(),
            flow,
            work: None,
        },
        caller,
    )?;
    Ok(session)
}

pub(crate) async fn prepare(
    store: &SharedStore,
    task: &Task,
    position: &FlowSession,
) -> Result<SessionRecord> {
    validate_task_position(task, position)?;
    let placement = store.placement(&WorkRef::Task(task.id.clone())).await?;
    let home = store
        .home_by_id(&placement.home_id)
        .await?
        .ok_or_else(|| anyhow!("Task {} Home {} disappeared", task.id, placement.home_id))?;
    if home.route != "local" {
        bail!("session starts on its placed Home; resume the Task there");
    }
    if position.session_run_id().is_none() {
        select_review_agent(store, task, position).await?;
        launch_flow(task, position).await?;
    }
    flow_surface(store, task, position).await
}

/// Choose the reserved review Run's agent from the Task's current choice,
/// then the step's, then the checkout's config. A published Run keeps what
/// it launched with; the choice is read back at launch.
pub(crate) async fn select_review_agent(
    store: &SharedStore,
    task: &Task,
    position: &FlowSession,
) -> Result<String> {
    let task = store.get_task(&task.id).await?.ok_or_else(|| {
        anyhow!(
            "Task {} disappeared while selecting its review agent",
            task.id
        )
    })?;
    let skill = crate::engine::current_skill(&position.invocation.steps, &position.cursor);
    let agent = crate::ops::task::resolve_task_agent(
        &task.worktree,
        task.agent.as_deref(),
        skill.as_ref().map(|step| &step.skill),
    );
    if let Some(session) = store.session(&flow_id(position)?).await? {
        let (provider, model) = crate::engine::config::parse_agent(&agent);
        store
            .retarget_unpublished_run(&session.input_id, &provider, model.as_deref())
            .await?;
    }
    Ok(agent)
}

/// A changed Task choice applies to a review that has not launched.
pub(crate) async fn retarget_prepared_task_review(store: &SharedStore, task: &Task) -> Result<()> {
    if let Some(position) = store
        .task_flow(&task.id)
        .await?
        .filter(FlowSession::is_human)
    {
        select_review_agent(store, task, &position).await?;
    }
    Ok(())
}

/// The Task whose own Flow waits at this Run's review: the Run's invocation is
/// the one the Task points at. Any other review naming a Task is a Flow about it.
async fn managed_review(
    store: &SharedStore,
    session: &AgentSession,
) -> crate::store::StoreResult<Option<(crate::durable::TaskId, FlowSession)>> {
    let (Some(task_id), Some(invocation)) = (&session.task_id, &session.flow_session_id) else {
        return Ok(None);
    };
    Ok(store
        .task_flow(task_id)
        .await?
        .filter(|position| position.invocation.id == *invocation)
        .map(|position| (task_id.clone(), position)))
}

pub(crate) async fn list(
    store: &SharedStore,
    filter: &crate::session::SessionFilter,
) -> Result<Vec<SessionRecord>> {
    let mut sessions = Vec::new();
    for session in store.sessions(filter).await? {
        sessions.push(surface(store, &session).await?);
    }
    Ok(sessions)
}

/// Resolve a Session id, or the Run id linked to it. An Ask or Flow Run
/// names its waiting boundary, so `$LF_RUN_ID` inside a review targets it.
async fn find_session(store: &SharedStore, session_id: &str) -> Result<Option<SessionTarget>> {
    // Membership outlives a pending boundary. Never reinterpret a retained
    // attempt's manifest as an independent conversation or current actor.
    let owned = match RunId::parse(session_id) {
        Ok(run_id) => store.session_for_run(&run_id).await?,
        Err(_) => store.session(session_id).await?,
    };
    if let Some(session) = owned {
        return owned_target(store, session_id, session).await.map(Some);
    }
    match crate::run_record::resolve_manifest(&crate::store::observability_home_dir(), session_id) {
        Ok((_, manifest)) => {
            // Prefix selectors also resolve through the canonical Run's owner.
            let Some(session) = store.session_for_run(&manifest.run_id).await? else {
                bail!("Run {} does not belong to a Session", manifest.run_id);
            };
            owned_target(store, manifest.run_id.as_str(), session)
                .await
                .map(Some)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(anyhow!("Session record unavailable: {error}")),
    }
}

async fn owned_target(
    store: &SharedStore,
    selector: &str,
    session: crate::session::AgentSession,
) -> Result<SessionTarget> {
    if selector != session.id && selector != session.input_id.as_str() {
        bail!(
            "Run {selector} is a historical attempt of Session {}; current Run is {}",
            session.id,
            session.input_id
        );
    }
    if session.completed_at.is_some() {
        bail!("Session {} is already complete", session.id);
    }
    let Some((task_id, position)) = managed_review(store, &session).await? else {
        if session.kind == crate::session::SessionKind::FlowReview
            && store.waiting_flow(&session.id).await?.is_none()
        {
            bail!("Session {} is no longer waiting", session.id);
        }
        return Ok(SessionTarget::Row {
            session: Box::new(session),
        });
    };
    if flow_id(&position)? != session.id {
        bail!("Session {} is no longer waiting", session.id);
    }
    let task = store
        .get_task(&task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {task_id} disappeared"))?;
    let latest = store
        .session(&session.id)
        .await?
        .ok_or_else(|| session_not_found(&session.id))?;
    if latest.input_id != session.input_id
        || (session.input_published && position.session_run_id() != Some(&session.input_id))
    {
        bail!(
            "Session {} changed its current Run during lookup; open the Session again",
            session.id
        );
    }
    Ok(SessionTarget::Flow {
        task: Box::new(task),
        position: Box::new(position),
        run_id: session.input_id.clone(),
    })
}

/// Where an interactive Run's provider keeps its history and client receipts.
struct NativeRun {
    dir: PathBuf,
    provider: String,
}

impl NativeRun {
    fn of(session: &crate::session::AgentSession) -> Result<Self> {
        Ok(Self {
            dir: local_session_run_dir(&session.input_id)
                .ok_or_else(|| anyhow!("Session {} has an invalid Run reference", session.id))?,
            provider: session
                .provider
                .clone()
                .ok_or_else(|| anyhow!("Session {} Run has no recorded provider", session.id))?,
        })
    }

    fn clients(&self) -> Result<Vec<crate::run_record::ProviderClientRef>> {
        crate::lf::commands::util::active_provider_clients(&self.dir, &self.provider)
    }

    fn history(&self, session: &crate::session::AgentSession) -> Result<ProviderSessionRef> {
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
        SessionTarget::Row { session } => surface(store, session).await,
        SessionTarget::Flow { task, position, .. } => flow_surface(store, task, position).await,
    }
}

pub(crate) async fn mark_ready(store: &SharedStore, summary: &str) -> Result<()> {
    let summary = summary.trim();
    if summary.is_empty() {
        bail!("ready summary cannot be empty");
    }
    let run_id = active_run_id()?;
    let id = match active_session_token()? {
        HumanSessionToken::Flow { token } => flow_token_id(&token),
        HumanSessionToken::StandaloneFlow { id } | HumanSessionToken::Ask { id } => id,
    };
    // The store fences readiness on the Session's current Run.
    store.ready_session(&id, &run_id, summary).await?;
    Ok(())
}

async fn complete_flow(store: &SharedStore, task: &Task, position: &FlowSession) -> Result<()> {
    let token = flow_token(task, position)?;
    let lock_id = flow_id(position)?;
    let launch_lock = tokio::task::spawn_blocking(move || lock_session_launch(&lock_id)).await??;
    crate::controller::task::complete_human_flow_step(store, &token, position).await?;
    let mut task = store
        .get_task(&token.task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {} disappeared after review completion", token.task_id))?;
    let launch = if store.task_flow(&task.id).await?.is_some() {
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
    position: &FlowSession,
) -> Result<()> {
    let Ok(active) = active_run_id() else {
        return Ok(());
    };
    let id = flow_id(position)?;
    let owner = store.session_for_run(&active).await?;
    if owner.is_some_and(|session| session.id == id) && position.session_run_id() != Some(&active) {
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
        SessionTarget::Row { session } => {
            Ok((session.kind == crate::session::SessionKind::FlowReview).then_some(session.cwd))
        }
    }
}

async fn stop_flow_run(store: &SharedStore, task: &Task, position: &FlowSession) {
    let Some(run_id) = position.session_run_id().cloned() else {
        return;
    };
    let result = async {
        let placement = store.placement(&WorkRef::Task(task.id.clone())).await?;
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
        .task_flow(&task_id)
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
        .task_flow(&task_id)
        .await?
        .ok_or_else(|| anyhow!("review is no longer waiting"))?;
    if current.session_run_id().is_some() {
        return Ok(());
    }
    serve_flow_locked(store, token, &current, launch_lock)
        .await
        .map(|_| ())
}

async fn serve_flow_locked(
    store: SharedStore,
    token: FlowSessionToken,
    position: &FlowSession,
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
    let agent = select_review_agent(&store, &task, &position).await?;
    let reservation = ReviewRunReservation {
        session_id: flow_token_id(&token),
        run_id: reserved.input_id.clone(),
        version: position.version,
    };
    let mut command = tokio::process::Command::new(lf);
    command
        .args(["--tui", "--model", &agent, "--as", &selector])
        .args(["skill", "--", &token.skill.name, &message])
        .current_dir(&task.worktree)
        .env(HUMAN_SESSION_ENV, serialized)
        .env(REVIEW_RUN_ENV, serde_json::to_string(&reservation)?);
    let mut child = spawn_session_run(&mut command, &reserved.input_id).await?;
    drop(launch_lock);
    let status = child.wait().await.context("wait for review skill")?;
    if !token_is_current(&store, &token).await? {
        let execution = crate::ops::task_execution::task_execution(&store, &token.task_id).await?;
        println!("Review session finished. {}", execution.reason);
        return Ok(reserved.input_id);
    }
    if status.success() {
        Ok(reserved.input_id)
    } else {
        Err(anyhow!("review skill exited with {status}"))
    }
}

pub(crate) async fn serve_ask(store: &SharedStore, run_id: &RunId) -> Result<()> {
    let session = store
        .session_for_run(run_id)
        .await?
        .ok_or_else(|| anyhow!("Ask Run {run_id} no longer exists"))?;
    let launch_lock = lock_session_launch(&session.id)?;
    let session = store
        .session(&session.id)
        .await?
        .ok_or_else(|| anyhow!("Ask Run {run_id} no longer exists"))?;
    if session.completed_at.is_some() {
        bail!("session {:?} is already resolved", session.id);
    }
    if session.input_id != *run_id || !run_is_prepared(&session.input_id)? {
        // Another launcher already published this Session. Do not replace or
        // infer death of its native client; explicit open owns native resume.
        return Ok(());
    }
    serve_locked(store, &session, launch_lock).await
}

/// What a waiting Session's agent carries to act on its own Session.
fn session_token(session: &AgentSession) -> Result<HumanSessionToken> {
    Ok(match session.kind {
        crate::session::SessionKind::FlowReview => HumanSessionToken::StandaloneFlow {
            id: session.id.clone(),
        },
        _ => HumanSessionToken::Ask {
            id: session.id.clone(),
        },
    })
}

/// Launch the prepared Run of an Ask or of a saved Flow's review.
async fn serve_locked(
    store: &SharedStore,
    session: &AgentSession,
    launch_lock: File,
) -> Result<()> {
    let lf = crate::engine::process::resolve_current_home_lf_binary_checked()?;
    let mut command = tokio::process::Command::new(lf);
    let token = session_token(session)?;
    command
        .current_dir(&session.cwd)
        .env(HUMAN_SESSION_ENV, serde_json::to_string(&token)?);
    match &token {
        HumanSessionToken::StandaloneFlow { id } => {
            crate::ops::flow_session::launch(store, &mut command, id).await?
        }
        _ => {
            command.args(ask_launch_args(store, session).await);
        }
    }
    let mut child = spawn_session_run(&mut command, &session.input_id).await?;
    drop(launch_lock);
    // Provider termination never completes a Session.
    let status = child.wait().await.context("wait for session agent")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("session agent exited with {status}"))
    }
}

async fn ask_launch_args(store: &SharedStore, session: &AgentSession) -> Vec<String> {
    let mut args = vec![
        "--tui".to_string(),
        "--model".to_string(),
        launch_model(session),
        "--__cwd".to_string(),
        session.cwd.display().to_string(),
    ];
    // The conversation's prompt carries its Work only while launch can resolve
    // it; the Run row keeps the attribution either way.
    if let Some(selector) = work_selector(session) {
        if crate::ops::resolve_work_binding(store, &session.cwd, &selector)
            .await
            .is_ok()
        {
            args.extend(["--as".to_string(), selector]);
        }
    }
    match &session.skill {
        Some(skill) => args.extend(["skill".to_string(), "--".to_string(), skill.clone()]),
        None => args.push(":".to_string()),
    }
    args.push(ask_message(session.request.as_deref().unwrap_or_default()));
    args
}

fn work_selector(session: &AgentSession) -> Option<String> {
    match (&session.task_id, &session.wave_id) {
        (Some(task), _) => Some(format!("task:{task}")),
        (None, Some(wave)) => Some(format!("wave:{wave}")),
        (None, None) => None,
    }
}

pub(crate) fn publish_run_binding(
    run_id: &RunId,
    provider: &str,
    model: Option<&str>,
) -> Result<()> {
    let raw = std::env::var_os(REVIEW_RUN_ENV)
        .ok_or_else(|| anyhow!("review Run has no launch reservation"))?;
    std::env::remove_var(REVIEW_RUN_ENV);
    let reservation: ReviewRunReservation = serde_json::from_str(&raw.to_string_lossy())?;
    if reservation.run_id != *run_id {
        bail!("review Run differs from its reservation");
    }
    let store =
        crate::store::sqlite::SqliteStore::new(&crate::store::observability_database_path()?)?;
    store.publish_review_run(
        &reservation.session_id,
        run_id,
        reservation.version,
        provider,
        model,
    )?;
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
        SessionTarget::Row { session }
            if session.kind == crate::session::SessionKind::Conversation =>
        {
            let native = NativeRun::of(session)?;
            let provider_session = native.history(session)?;
            if resume
                && mode != OpenMode::Replace
                && native.provider == "codex"
                && connect_live_codex(store, session, &native.dir, &provider_session).await?
            {
                let session = store
                    .sqlite
                    .session(&session.id)?
                    .ok_or_else(|| session_not_found(&session.id))?;
                return surface(store, &session).await;
            }
            if resume {
                crate::lf::commands::util::require_provider_session_launch(&native.dir)?;
            }
            match mode {
                OpenMode::Refuse if !native.clients()?.is_empty() => {
                    require_session_action(
                        SessionKind::Conversation,
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
            let mut result = surface(store, session).await?;
            if resume {
                crate::lf::commands::util::resume_session(
                    &native.provider,
                    session.model.as_deref(),
                    &session.cwd,
                    &session.input_id,
                    &native.dir,
                    &provider_session,
                )?;
            } else {
                match mode {
                    OpenMode::Replace => result.open_argv.push("--replace".to_string()),
                    OpenMode::Try => result.open_argv.push("--try".to_string()),
                    OpenMode::Refuse => {}
                }
            }
            Ok(result)
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
        SessionTarget::Row { session } => {
            if mode != OpenMode::Refuse {
                bail!("--replace and --try apply only to interactive provider sessions");
            }
            let mut surface = surface(store, session).await?;
            if resume {
                surface.run_id = open_waiting(store, &session.id).await?;
            }
            Ok(surface)
        }
    }
}

/// Connect to one existing provider thread. Native UI traffic crosses the same
/// driver fence as the headless writer; closing the UI releases only its claim.
#[cfg(unix)]
async fn connect_live_codex(
    store: &SharedStore,
    session: &AgentSession,
    dir: &Path,
    provider: &crate::run_record::ProviderSessionRef,
) -> Result<bool> {
    let Some((endpoint, thread)) = store.sqlite.session_connection(&session.id)? else {
        return Ok(false);
    };
    match tokio::net::UnixStream::connect(&endpoint).await {
        Ok(socket) => drop(socket),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ) =>
        {
            return Ok(false)
        }
        Err(error) => return Err(error.into()),
    }
    if thread != provider.provider_session_id {
        bail!("Recorded conversation differs from the live provider thread");
    }
    let exec = crate::journal::current_exec_id()
        .ok_or_else(|| anyhow!("Connecting requires the current lf Exec"))?;
    let expected = store.sqlite.session_driver(&session.id)?;
    let driver = store
        .sqlite
        .claim_session_driver(&session.id, expected.as_ref(), &exec, false)?;
    let connected = async {
        store.sqlite.make_session_interactive(&session.id, &driver)?;
        let directory = tempfile::Builder::new().prefix("lf-connect-").tempdir_in("/tmp")?;
        let remote = directory.path().join("client.sock");
        let listener = tokio::net::UnixListener::bind(&remote)?;
        let connection = crate::harness::codex_connection::CodexConnection {
            store: store.sqlite.clone(), session_id: session.id.clone(), thread_id: thread, driver: Some(driver.clone()),
        };
        connection.recover_history(Path::new(&endpoint)).await?;
        let relay = tokio::spawn(async move {
            let mut clients = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((client, _)) = accepted else { break };
                        let connection = connection.clone();
                        let endpoint = endpoint.clone();
                        clients.spawn(async move { connection.serve(client, Path::new(&endpoint)).await });
                    }
                    result = clients.join_next(), if !clients.is_empty() => {
                        if let Some(Ok(Err(error))) = result { tracing::warn!(%error, "native conversation connection ended"); }
                    }
                }
            }
        });
        let session = session.clone();
        let dir = dir.to_path_buf();
        let provider = provider.clone();
        let result = tokio::task::spawn_blocking(move || {
            crate::lf::commands::util::resume_session_with_env(
                "codex", session.model.as_deref(), &session.cwd, &session.input_id, &dir, &provider,
                &BTreeMap::new(), None, Some(&remote),
            )
        }).await;
        relay.abort();
        let _ = relay.await;
        result??;
        Ok::<_, anyhow::Error>(true)
    }.await;
    match store.sqlite.release_session_driver(&session.id, &driver) {
        Ok(_) | Err(crate::store::StoreError::InvalidAuthority(_)) => {}
        Err(error) => return Err(error.into()),
    }
    connected
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
        SessionTarget::Row { session }
            if session.kind == crate::session::SessionKind::Conversation =>
        {
            let native = NativeRun::of(session)?;
            crate::lf::commands::util::stop_provider_session(&native.dir, &native.provider)?;
            store
                .complete_session(&session.id, &session.input_id)
                .await?;
        }
        SessionTarget::Row { session } => {
            let lock_id = session.id.clone();
            let _lock =
                tokio::task::spawn_blocking(move || lock_session_launch(&lock_id)).await??;
            if session.kind == crate::session::SessionKind::FlowReview {
                crate::ops::flow_session::complete(store, session).await?;
            } else {
                store
                    .complete_session(&session.id, &session.input_id)
                    .await?;
                // The answer is durable before teardown can interrupt this caller
                // or fail. A cleanup failure must not strand the waiting caller.
                if let Err(error) = stop_native_run(&session.input_id) {
                    tracing::warn!(session_id = %session.id, run_id = %session.input_id,
                        error = %format!("{error:#}"),
                        "Ask completion saved, but its native Session could not be stopped");
                }
            }
        }
        SessionTarget::Flow { task, position, .. } => {
            complete_flow(store, task, position).await?;
        }
    }
    Ok(session)
}

/// Open an Ask or a saved Flow's review: resume its native history, else
/// launch its prepared Run, else append another attempt to the Session.
async fn open_waiting(store: &SharedStore, id: &str) -> Result<RunId> {
    loop {
        let observed = store
            .session(id)
            .await?
            .ok_or_else(|| session_not_found(id))?;
        let lock_id = id.to_string();
        let launch_lock =
            tokio::task::spawn_blocking(move || lock_session_launch(&lock_id)).await??;
        let session = store
            .session(id)
            .await?
            .ok_or_else(|| session_not_found(id))?;
        if session.completed_at.is_some() {
            bail!("session {id:?} is already complete");
        }
        if session.input_id != observed.input_id {
            drop(launch_lock);
            continue;
        }
        let mut launch_lock = Some(launch_lock);
        let token = session_token(&session)?;
        if resume_native_run(&session.input_id, &token, &mut launch_lock)? {
            return Ok(session.input_id);
        }
        // A consumed launch without native history gets another attempt under
        // the same Session; its title and feedback never leave the row.
        let session = if run_is_prepared(&session.input_id)? {
            session
        } else {
            let flow = match &token {
                HumanSessionToken::StandaloneFlow { id } => {
                    crate::ops::flow_session::membership(store, id).await?
                }
                _ => crate::run_record::RunFlowMembership::Independent,
            };
            let replaced = session.input_id.clone();
            store
                .replace_session_input(&replaced, prepare_input(session, flow, None)?)
                .await?
        };
        serve_locked(
            store,
            &session,
            launch_lock.expect("an unresumed Session retains its launch lock"),
        )
        .await?;
        return Ok(session.input_id);
    }
}

async fn open_flow_locked(
    store: &SharedStore,
    task: &Task,
    position: &FlowSession,
    launch_lock: File,
) -> Result<RunId> {
    let previous = position.session_run_id().cloned();
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

/// A Session as its row and its current Run describe it.
async fn surface(store: &SharedStore, session: &AgentSession) -> Result<SessionRecord> {
    let kind = match session.kind {
        crate::session::SessionKind::Conversation => SessionKind::Conversation,
        crate::session::SessionKind::Ask => SessionKind::Ask,
        crate::session::SessionKind::FlowReview => SessionKind::Flow,
    };
    let work = match (&session.task_id, &session.wave_id) {
        (Some(task), _) => Some(WorkRef::Task(task.clone())),
        (None, Some(wave)) => Some(WorkRef::Wave(wave.clone())),
        (None, None) => None,
    };
    let managed = match managed_review(store, session).await {
        Ok(managed) => Ok(managed),
        Err(crate::store::StoreError::InvalidData(reason)) => Err(reason),
        Err(error) => return Err(error.into()),
    };
    // Only a Task's own review can wait on another Home.
    let remote = match (&work, &managed) {
        (Some(work @ WorkRef::Task(_)), Ok(Some(_))) => {
            let placement = store.placement(work).await?;
            let home = store
                .home_by_id(&placement.home_id)
                .await?
                .ok_or_else(|| anyhow!("Session {} Home disappeared", session.id))?;
            (home.route != "local").then_some(home.id)
        }
        _ => None,
    };
    let dir = local_session_run_dir(&session.input_id)
        .ok_or_else(|| anyhow!("Session {} has an invalid Run reference", session.id))?;
    let clients = match &session.provider {
        Some(provider) if remote.is_none() => {
            crate::lf::commands::util::active_provider_clients(&dir, provider)?
        }
        _ => Vec::new(),
    };
    let launched = match remote {
        Some(_) => session.input_published,
        None => crate::run_record::read_provider_session(&dir)?.is_some(),
    };
    let state = if session.completed_at.is_some() {
        SessionState::Closed
    } else if session.ready_summary.is_some() {
        SessionState::Ready
    } else if !clients.is_empty() {
        SessionState::Active
    } else if launched {
        SessionState::Closed
    } else {
        SessionState::Waiting
    };
    let mut actions = session_actions(kind, state);
    let flow_membership = match &session.flow_session_id {
        None => SessionFlowMembership::Independent,
        Some(id) => match store.flow(id).await {
            Ok(Some(flow)) => {
                let graph = crate::engine::flow_graph::FlowGraph::new(
                    &flow.invocation.flow,
                    &flow.invocation.steps,
                );
                let node = session
                    .node
                    .map(|id| {
                        graph
                            .node_at(id)
                            .map(|node| node.key.clone())
                            .ok_or_else(|| {
                                anyhow!(
                                    "Session {} names an absent captured Flow node {id}",
                                    session.id
                                )
                            })
                    })
                    .transpose()?;
                SessionFlowMembership::Step {
                    flow: flow.invocation.flow.clone(),
                    invocation_id: id.clone(),
                    step: session.skill.clone().unwrap_or_default(),
                    node,
                    iterations: session.iterations.clone(),
                    occurrence: if flow.finished {
                        SessionFlowOccurrence::Past
                    } else if flow
                        .current_attempt
                        .as_ref()
                        .is_some_and(|attempt| attempt.run_id == session.input_id)
                    {
                        SessionFlowOccurrence::Current
                    } else {
                        SessionFlowOccurrence::Earlier
                    },
                }
            }
            Ok(None) => SessionFlowMembership::Unknown {
                reason: format!("Flow {id} is unavailable"),
            },
            Err(crate::store::StoreError::InvalidData(reason)) => {
                for action in &mut actions {
                    action.unavailable_reason = Some(reason.clone());
                }
                SessionFlowMembership::Unknown { reason }
            }
            Err(error) => return Err(error.into()),
        },
    };
    Ok(SessionRecord {
        id: session.id.clone(),
        run_id: session.input_id.clone(),
        kind,
        interactive: session.interactive,
        wave_id: session.wave_id.clone(),
        work_path: session_work_path(store, session).await?,
        work,
        actions,
        title: session.title.clone(),
        title_source: match session.title_source {
            crate::session::TitleSource::Human => SessionTitleSource::Human,
            crate::session::TitleSource::Generated => SessionTitleSource::Generated,
        },
        flow_membership,
        detail: match (kind, &session.skill) {
            (SessionKind::Conversation, _) => launch_model(session),
            (_, Some(skill)) => skill.clone(),
            (_, None) => "Request for input".to_string(),
        },
        provider: session.provider.clone(),
        cwd: session.cwd.display().to_string(),
        state,
        ready_summary: session.ready_summary.clone(),
        terminal_ids: clients
            .into_iter()
            .filter_map(|client| client.terminal_id)
            .collect(),
        open_argv: human_open_argv(remote.as_ref(), Some(&session.cwd), &session.id)?,
    })
}

/// The `provider[:model]` a Run launched with.
fn launch_model(session: &AgentSession) -> String {
    let provider = session.provider.clone().unwrap_or_default();
    match &session.model {
        Some(model) => format!("{provider}:{model}"),
        None => provider,
    }
}

async fn session_work_path(store: &SharedStore, session: &AgentSession) -> Result<Option<String>> {
    let Some(wave_id) = &session.wave_id else {
        return Ok(None);
    };
    let wave = match store.get_wave(wave_id).await? {
        Some(wave) => wave.name().to_string(),
        None => format!("Wave {wave_id} (unavailable)"),
    };
    Ok(Some(match &session.task_id {
        None => wave,
        Some(id) => match store.get_task(id).await? {
            Some(task) => format!("{wave} / {}", task.plan.identifier),
            None => format!("Task {id} (unavailable)"),
        },
    }))
}

fn local_session_run_dir(run_id: &RunId) -> Option<PathBuf> {
    crate::run_record::record_dir(&crate::store::observability_home_dir(), run_id)
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
    let (run_id, position) = match &target {
        SessionTarget::Row { session } => {
            // A Run selector names that attempt; the Session id names the conversation.
            let expected_run = (session_id != session.id).then_some(&session.input_id);
            store
                .rename_session(&session.id, expected_run, title, title_source)
                .await?;
            let session = store
                .session(&session.id)
                .await?
                .ok_or_else(|| session_not_found(session_id))?;
            return surface(store, &session).await;
        }
        SessionTarget::Flow {
            run_id, position, ..
        } => (run_id, position),
    };
    let id = flow_id(position)?;
    let expected_run = (session_id != id).then(|| run_id.clone());
    // Retain the caller's exact/prefix selector while waiting for replacement.
    let pending_lock = tokio::task::spawn_blocking(move || lock_session_launch(&id));
    #[cfg(test)]
    action_test::after_lookup("rename", session_id).await;
    let _launch_lock = pending_lock.await??;
    target = find_session(store, session_id)
        .await?
        .ok_or_else(|| session_not_found(session_id))?;
    if let SessionTarget::Flow { position, .. } = &target {
        store
            .rename_session(
                &flow_id(position)?,
                expected_run.as_ref(),
                title,
                title_source,
            )
            .await?;
    }
    session_surface(store, &target).await
}

/// Assign a Task to future Session work. The id is the Session's
/// or any of its Runs'; a closed Session binds like an open one.
pub(crate) async fn bind(store: &SharedStore, id: &str, task: &str) -> Result<SessionRecord> {
    let owned = match RunId::parse(id) {
        Ok(run_id) => store.session_for_run(&run_id).await?,
        Err(_) => store.session(id).await?,
    };
    let session = owned.ok_or_else(|| session_not_found(id))?;
    let task = match crate::durable::TaskId::parse(task) {
        Ok(id) => store.get_task(&id).await?,
        Err(_) => store.get_task_by_issue(task).await?,
    }
    .ok_or_else(|| anyhow!("Task {task:?} is not registered"))?;
    let session = store
        .bind_session(&session.id, &session.input_id, &task.id)
        .await
        .map_err(|error| match error {
            crate::store::StoreError::InvalidAuthority(reason) => anyhow!(reason),
            error => anyhow!(error),
        })?;
    eprintln!(
        "Binding {} to {} ({}). Permanent.",
        session.id, task.plan.identifier, task.plan.title
    );
    surface(store, &session).await
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
        None,
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

pub(crate) async fn token_is_current(
    store: &SharedStore,
    token: &FlowSessionToken,
) -> Result<bool> {
    Ok(store
        .task_flow(&token.task_id)
        .await?
        .as_ref()
        .is_some_and(|position| token_matches(token, position)))
}

async fn flow_surface(
    store: &SharedStore,
    task: &Task,
    position: &FlowSession,
) -> Result<SessionRecord> {
    validate_task_position(task, position)?;
    let id = flow_id(position)?;
    let session = store
        .session(&id)
        .await?
        .ok_or_else(|| session_not_found(&id))?;
    surface(store, &session).await
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

fn validate_task_position(task: &Task, position: &FlowSession) -> Result<()> {
    if position.task_id.as_ref() != Some(&task.id) || !position.is_human() {
        return Err(anyhow!("session does not belong to Task {}", task.id));
    }
    let step = position.current();
    let node_id = step
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

fn flow_token(task: &Task, position: &FlowSession) -> Result<FlowSessionToken> {
    validate_task_position(task, position)?;
    let step = position.current();
    let crate::engine::ConcreteStep::Skill(planned) = position.current_plan() else {
        bail!("review flow position does not select a Skill");
    };
    Ok(FlowSessionToken {
        task_id: task.id.clone(),
        invocation_id: position.invocation.id.clone(),
        flow: step.flow,
        node_id: step.id.expect("validated human position has a node id"),
        skill: planned.skill.clone(),
        iteration: position.cursor.iteration,
    })
}

fn token_matches(token: &FlowSessionToken, position: &FlowSession) -> bool {
    let step = position.current();
    position.task_id.as_ref() == Some(&token.task_id)
        && position.invocation.id == token.invocation_id
        && step.human
        && step.flow == token.flow
        && step.id.as_deref() == Some(token.node_id.as_str())
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

async fn launch_flow(task: &Task, position: &FlowSession) -> Result<()> {
    let step = position.current();
    let node_id = step
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
async fn launch_ask(session: &AgentSession) -> Result<()> {
    let lf = crate::engine::process::resolve_current_home_lf_binary();
    let argv = vec![
        lf.to_string_lossy().to_string(),
        "session".to_string(),
        "serve-ask".to_string(),
        session.input_id.to_string(),
    ];
    let caller = session
        .caller_input_id
        .as_ref()
        .ok_or_else(|| anyhow!("Ask {} has no caller input", session.id))?
        .to_string();
    let caller_dir =
        crate::run_record::resolve_manifest(&crate::store::observability_home_dir(), &caller)
            .ok()
            .map(|(directory, _)| directory.to_string_lossy().to_string());
    let mut env = vec![(crate::durable::RUN_ID_ENV, caller.as_str())];
    if let Some(directory) = &caller_dir {
        env.push((RUN_DIR_ENV, directory.as_str()));
    }
    start_durable_session(&ask_background_name(&session.id), &session.cwd, &argv, &env).await
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

pub(crate) fn flow_id(position: &FlowSession) -> Result<String> {
    let step = position.current();
    let node_id = step
        .id
        .as_deref()
        .ok_or_else(|| anyhow!("review flow position has no node id"))?;
    let task = position
        .task_id
        .as_ref()
        .ok_or_else(|| anyhow!("review flow position belongs to no Task"))?;
    Ok(format!(
        "{}:{}:{}:{}:{}",
        task, position.invocation.id, step.flow, node_id, position.cursor.iteration
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

fn flow_background_name(position: &FlowSession) -> Result<String> {
    let step = position.current();
    Ok(flow_token_background_name(&FlowSessionToken {
        task_id: position
            .task_id
            .clone()
            .ok_or_else(|| anyhow!("review flow position belongs to no Task"))?,
        invocation_id: position.invocation.id.clone(),
        flow: step.flow,
        node_id: step
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

async fn wait_for_ask(store: &SharedStore, id: &str) -> Result<String> {
    loop {
        let session = store
            .session(id)
            .await?
            .ok_or_else(|| anyhow!("session {id:?} disappeared before resolution"))?;
        if session.completed_at.is_some() {
            return session
                .ready_summary
                .ok_or_else(|| anyhow!("Ask {id} completed without an answer"));
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
        HumanSessionToken::StandaloneFlow { id } => {
            crate::ops::flow_session::pinned_skill(&id, requested).map(Some)
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
    use crate::durable::{FlowSession, RunId};
    use crate::engine::{prepare_launch_prompt, Config, LaunchPromptInput, Surface};
    use crate::lf::{Cli, Commands};
    use crate::run_record::RunManifest;
    use crate::session::{AgentSession, SessionKind};
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

    async fn open_asks(store: &SharedStore) -> Vec<AgentSession> {
        let mut open = store
            .sessions(&crate::session::SessionFilter::default())
            .await
            .unwrap();
        open.retain(|session| session.kind == SessionKind::Ask);
        open
    }

    async fn wait_until_asks(store: &SharedStore, count: usize) -> Vec<AgentSession> {
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

    async fn answer(store: &SharedStore, session: &AgentSession, summary: &str) {
        store
            .ready_session(&session.id, &session.input_id, summary)
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
    async fn consumed_ask(store: &SharedStore, id: String, question: &str) -> AgentSession {
        let stored = reserve_ask(store, id, question, None).await.unwrap();
        std::fs::remove_file(run_dir(&stored).join("prepared")).unwrap();
        stored
    }

    fn run_dir(run: &AgentSession) -> std::path::PathBuf {
        super::local_session_run_dir(&run.input_id).unwrap()
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
            // Concurrent callers race for the launch lock, so the retry
            // starts once the first request's Session is stored.
            let duplicate = async {
                while store.session(&keyed_id(key)).await.unwrap().is_none() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                ask_once(&store, key, "Changed retry text", Some("unblock")).await
            };
            let independent =
                ask_once(&store, "invocation/decision/visit-2", "Second choice", None);
            let human = async {
                for session in wait_until_asks(&store, 2).await {
                    assert!(session.id.starts_with("ask_once_"));
                    assert!(!session.id.contains("checkout"));
                    let manifest: RunManifest = serde_json::from_slice(
                        &std::fs::read(run_dir(&session).join("manifest.json")).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(manifest.parent_run_id.as_ref().unwrap().as_str(), caller);
                    assert_eq!(session.flow_session_id, None);
                    assert_eq!(
                        (session.provider.as_deref(), session.model.as_deref()),
                        (Some("codex"), Some("test"))
                    );
                    let summary = if session.request.as_deref() == Some("Second choice") {
                        "second"
                    } else {
                        assert_eq!(session.request.as_deref(), Some("Choose a policy"));
                        assert_eq!(session.skill.as_deref(), Some("unblock"));
                        "first"
                    };
                    answer(&store, &session, summary).await;
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
            assert_eq!(store.session_inputs(&keyed_id(key)).await.unwrap().len(), 1);
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
            let preserved = store.session(&id).await.unwrap().unwrap();
            let first_input = preserved.input_id.clone();
            assert_eq!(preserved.request.as_deref(), Some("Preserved question"));
            FAILED_ASK_LAUNCHERS.lock().unwrap().clear();
            let human = async {
                let sessions = wait_until_asks(&store, 1).await;
                assert_eq!(sessions[0].id, id);
                assert_eq!(sessions[0].input_id, first_input);
                answer(&store, &sessions[0], "Recovered").await;
            };
            let (result, ()) = tokio::join!(ask_once(&store, key, "replacement", None), human);
            assert_eq!(result.unwrap(), "Recovered");
            assert_eq!(wait_for_ask(&store, &id).await.unwrap(), "Recovered");
        });
    }

    #[test]
    fn keyed_ask_does_not_replace_a_published_native_run() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let key = "published-run-boundary";
            let session = consumed_ask(&store, keyed_id(key), "Keep native ownership").await;
            // No tmux launcher and no native history do not grant replacement
            // authority. Even a delayed serve command must join.
            serve_ask(&store, &session.input_id).await.unwrap();
            assert!(tokio::time::timeout(
                Duration::from_millis(100),
                ask_once(&store, key, "retry", None)
            )
            .await
            .is_err());
            let current = store.session(&session.id).await.unwrap().unwrap();
            assert_eq!(current.input_id, session.input_id);
            assert!(ASK_LAUNCHERS.lock().unwrap().is_empty());
            std::env::set_var("LF_RUN_ID", session.input_id.as_str());
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
            assert_eq!(wait_for_ask(&store, &session.id).await.unwrap(), "Resolved");
            assert!(open_asks(&store).await.is_empty());
        });
    }

    #[test]
    fn reopening_ask_cannot_overwrite_a_concurrent_completion() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let session =
                consumed_ask(&store, "ask_reopening".to_string(), "Keep the answer").await;
            store
                .ready_session(&session.id, &session.input_id, "Accepted direction")
                .await
                .unwrap();
            let lock = super::lock_session_launch(&session.id).unwrap();
            let mut reopening = Box::pin(super::open_waiting(&store, &session.id));
            assert!(
                tokio::time::timeout(Duration::from_millis(200), reopening.as_mut())
                    .await
                    .is_err()
            );
            let untouched = store.session(&session.id).await.unwrap().unwrap();
            let current = untouched.clone();
            assert_eq!(current.input_id, session.input_id);
            assert_eq!(
                untouched.ready_summary.as_deref(),
                Some("Accepted direction")
            );

            store
                .complete_session(&session.id, &session.input_id)
                .await
                .unwrap();
            drop(lock);

            assert!(reopening
                .await
                .unwrap_err()
                .to_string()
                .contains("already complete"));
            assert_eq!(
                wait_for_ask(&store, &session.id).await.unwrap(),
                "Accepted direction"
            );
            assert_eq!(store.session_inputs(&session.id).await.unwrap().len(), 1);
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
            let session = consumed_ask(
                &store,
                "ask_cleanup".to_string(),
                "Resolve stalled progress",
            )
            .await;
            let manifest = run_dir(&session).join("manifest.json");
            std::fs::write(&manifest, b"invalid manifest").unwrap();

            answer(&store, &session, "Try the narrower proof").await;

            assert_eq!(
                wait_for_ask(&store, &session.id).await.unwrap(),
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
                let session = wait_until_asks(&store, 1).await.remove(0);
                answer(&store, &session, "Ordinary answer").await;
                session.id
            };
            let (result, id) = tokio::join!(ask(&store, "Plain question", None), human);
            assert_eq!(result.unwrap(), "Ordinary answer");
            let closed = store.session(&id).await.unwrap().unwrap();
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
        for kind in ["conversation", "ask", "flow"] {
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
        let session = |task_id: Option<crate::work::task::TaskId>| AgentSession {
            caller_input_id: None,
            id: "conversation".into(),
            input_id: RunId::new(),
            input_published: true,
            cwd: directory.path().into(),
            skill: None,
            provider: None,
            model: None,
            node: None,
            iterations: None,
            task_id,
            wave_id: Some(wave.id().clone()),
            flow_session_id: None,
            work_source: None,
            bound_at: None,
            kind: crate::session::SessionKind::Conversation,
            interactive: true,
            repo: None,
            title: "Conversation".into(),
            title_source: crate::session::TitleSource::Generated,
            request: None,
            ready_summary: None,
            completed_at: None,
            created_at: 1,
        };
        assert_eq!(
            super::session_work_path(&store, &session(None))
                .await
                .unwrap()
                .as_deref(),
            Some("product")
        );
        let unbound = AgentSession {
            wave_id: None,
            ..session(None)
        };
        assert_eq!(
            super::session_work_path(&store, &unbound).await.unwrap(),
            None
        );
        let missing = crate::work::task::TaskId::new();
        assert_eq!(
            super::session_work_path(&store, &session(Some(missing.clone())))
                .await
                .unwrap(),
            Some(format!("Task {missing} (unavailable)"))
        );
    }

    fn position() -> FlowSession {
        FlowSession {
            invocation: crate::durable::test_flow_invocation(
                "review",
                1,
                "review-design",
                Some("review_kickoff"),
                true,
            ),
            cursor: crate::engine::ExecutionCursor {
                index: 1,
                iteration: 3,
                ..Default::default()
            },
            version: 0,
            task_id: Some(TaskId::new()),
            wave_id: None,
            cwd: "/repo".into(),
            message: None,
            model: None,
            current_attempt: None,
            pending_session_id: None,
            ready_summary: None,
            worker_generation: 0,
            claim: None,
            failure: None,
            finished: false,
            updated_at: time::OffsetDateTime::now_utc(),
        }
    }

    fn nested_position() -> FlowSession {
        use crate::engine::flow::{ConcretePath, ConcreteStep, ConcreteXor, RepeatPolicy, Skill};
        use crate::engine::{ExecutionCursor, NestedCursor};

        let mut position = position();
        let mut review = position.invocation.steps[1].clone();
        let ConcreteStep::Skill(start) = &mut review else {
            panic!("skill")
        };
        start.id = Some("begin".into());
        start.human = false;
        let mut decide = review.clone();
        let ConcreteStep::Skill(end) = &mut decide else {
            panic!("skill")
        };
        end.id = Some("decide".into());
        end.repeat = Some(RepeatPolicy {
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
        position
    }

    #[test]
    fn captured_nested_membership_retains_its_exact_graph_occurrence() {
        use crate::engine::flow_graph::FlowGraph;
        use crate::run_record::RunFlowStep;

        let position = nested_position();
        let graph = FlowGraph::new("review", &position.invocation.steps);
        let expected = &graph.steps[1].paths[0].steps[1].key;
        assert_eq!(expected, "1/fix/1");
        let captured = RunFlowStep::of(&position).unwrap();
        assert_eq!(captured.node.as_ref(), Some(expected));
        assert_eq!(captured.iterations, Some(vec![vec![5], vec![2]]));

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
    fn stored_session_membership_resolves_nested_and_post_xor_graph_nodes() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let store = home.store().await;
                let mut flow = nested_position();
                flow.task_id = None;
                flow.cwd = home.home.path().into();
                let nested = flow.cursor.clone();
                let graph =
                    crate::engine::flow_graph::FlowGraph::new("review", &flow.invocation.steps);
                let expected = [
                    graph.steps[1].paths[0].steps[1].clone(),
                    graph.steps[2].clone(),
                    graph.steps[0].clone(),
                ];
                let mut flow = store.create_flow(flow).await.unwrap();
                let mut sessions = Vec::new();
                for (index, cursor) in [
                    nested,
                    crate::engine::ExecutionCursor {
                        index: 2,
                        ..Default::default()
                    },
                    crate::engine::ExecutionCursor::default(),
                ]
                .into_iter()
                .enumerate()
                {
                    if index != 0 {
                        flow = store
                            .checkpoint_flow(flow.id(), flow.version, &cursor, None, None)
                            .await
                            .unwrap();
                    }
                    flow = store
                        .reserve_attempt(flow.id(), flow.version, None)
                        .await
                        .unwrap();
                    let run_id = &flow.current_attempt.as_ref().unwrap().run_id;
                    let session = store.session_for_run(run_id).await.unwrap().unwrap();
                    let run = session.clone();
                    assert_eq!(run.node, Some([3, 4, 0][index]));
                    sessions.push((session.id, run.iterations.clone()));
                    for (previous, (id, iterations)) in sessions.iter().enumerate() {
                        let session = store.session(id).await.unwrap().unwrap();
                        let projected = super::surface(&store, &session).await.unwrap();
                        let super::SessionFlowMembership::Step {
                            node,
                            step,
                            iterations: actual_iterations,
                            occurrence,
                            ..
                        } = projected.flow_membership
                        else {
                            panic!("stored Session must name its captured node")
                        };
                        assert_eq!(node.as_deref(), Some(expected[previous].key.as_str()));
                        assert_eq!(step, expected[previous].label);
                        assert_eq!(&actual_iterations, iterations);
                        assert_eq!(
                            occurrence,
                            if previous == index {
                                super::SessionFlowOccurrence::Current
                            } else {
                                super::SessionFlowOccurrence::Earlier
                            }
                        );
                    }
                }
            });
    }

    #[test]
    fn session_identity_is_the_exact_human_flow_position() {
        let position = position();
        let step = position.current();
        let token = FlowSessionToken {
            task_id: position.task_id.clone().unwrap(),
            invocation_id: position.invocation.id.clone(),
            flow: step.flow,
            node_id: step.id.unwrap(),
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
        planned.skill.name = "retained-review".to_string();
        let planned_skill = planned.skill.clone();
        let step = position.current();
        let token = FlowSessionToken {
            task_id: position.task_id.clone().unwrap(),
            invocation_id: position.invocation.id.clone(),
            flow: step.flow,
            node_id: step.id.unwrap(),
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

        let repo = tempfile::tempdir().unwrap();
        let selected = crate::lf::discovery::resolve_definition(
            repo.path(),
            "retained-review",
            Some(crate::engine::target::DefinitionKind::Skill),
        );

        match previous {
            Some(value) => std::env::set_var(HUMAN_SESSION_ENV, value),
            None => std::env::remove_var(HUMAN_SESSION_ENV),
        }
        let crate::engine::target::Target::Skill(skill) = selected.unwrap() else {
            panic!("review must retain its captured Skill")
        };
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
            checkout.path().join(".lf/skills/list.md"),
            "Resolve the blocker with the human using the preserved evidence.",
        )
        .unwrap();
        let session = AgentSession {
            caller_input_id: None,
            task_id: None,
            wave_id: None,
            flow_session_id: None,
            work_source: None,
            bound_at: None,
            id: "ask_skill_proof".to_string(),
            input_id: RunId::new(),
            input_published: true,
            cwd: checkout.path().into(),
            skill: Some("unblock".into()),
            provider: Some("claude".into()),
            model: Some("opus".into()),
            node: None,
            iterations: None,
            kind: SessionKind::Ask,
            interactive: true,
            repo: None,
            title: "Choose the delivery policy".to_string(),
            title_source: crate::session::TitleSource::Generated,
            request: Some("Choose the delivery policy".to_string()),
            ready_summary: Some("Discussed the policy".to_string()),
            completed_at: None,
            created_at: 1,
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let store: SharedStore = std::sync::Arc::new(
            runtime
                .block_on(open_ephemeral_store(&StorageConfig::sqlite(
                    checkout.path().join("registry.db"),
                )))
                .unwrap(),
        );

        let args = runtime.block_on(ask_launch_args(&store, &session));
        let cli = Cli::try_parse_from(std::iter::once("lf".to_string()).chain(args)).unwrap();
        let Some(Commands::Skill {
            cmd: crate::lf::SkillCommand::External(mut args),
        }) = cli.command
        else {
            panic!("selected Ask must use the ordinary skill command")
        };
        let name = args.remove(0);
        let prepared = prepare_launch_prompt(
            &Config {
                diff: false,
                diff_files: false,
                paste: false,
                ..Config::default()
            },
            LaunchPromptInput {
                repo_root: session.cwd.clone(),
                cwd: Some(session.cwd.clone()),
                skill: Some(name),
                message: Some(args.join(" ")),
                agent: Some(super::launch_model(&session)),
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

        let inline = AgentSession {
            skill: None,
            ..session
        };
        let args = runtime.block_on(ask_launch_args(&store, &inline));
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
    fn initial_session_publication_requires_history_and_an_owned_client() {
        let dir = tempfile::tempdir().unwrap();
        let harness = "sleep".to_string();
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
        let mut client = std::process::Command::new("/bin/sleep")
            .arg("60")
            .spawn()
            .unwrap();
        crate::run_record::write_provider_client(dir.path(), client.id()).unwrap();
        let resumable = session_run_is_resumable(dir.path(), &manifest);
        client.kill().unwrap();
        client.wait().unwrap();
        assert!(resumable.unwrap());
    }
}
