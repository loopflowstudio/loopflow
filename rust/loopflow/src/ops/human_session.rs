use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::durable::{FlowPosition, RunId, WorkRef, WorkStatus};
use crate::engine::Skill;
use crate::run_record::{
    preferred_work_selector, ProviderSessionRef, RunManifest, SessionName, SessionTitleSource,
    RUN_DIR_ENV,
};
use crate::store::SharedStore;
use crate::work::task::{Task, TaskId};

pub(crate) const HUMAN_SESSION_ENV: &str = "LF_HUMAN_SESSION";
pub(crate) const PREPARED_RUN_ENV: &str = "LF_HUMAN_SESSION_RUN";

const ASK_SESSION_DIRECTORY: &str = "human-sessions";
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

fn session_actions(kind: SessionKind, state: SessionState) -> Vec<SessionAction> {
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
    /// `current` is false once the Flow has moved past this occurrence.
    Step {
        flow: String,
        invocation_id: String,
        step: String,
        step_index: u32,
        iteration: u32,
        current: bool,
    },
    Independent,
    Unknown {
        reason: String,
    },
}

impl SessionFlowMembership {
    fn of_position(position: &FlowPosition) -> Self {
        Self::Step {
            flow: position.invocation.flow.clone(),
            invocation_id: position.invocation.id.clone(),
            step: position.current().step,
            step_index: position.step_index,
            iteration: position.iteration,
            current: true,
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
            match store.flow_position(&step.task_id).await {
                Ok(position) => SessionFlowMembership::Step {
                    flow: step.flow.clone(),
                    invocation_id: step.invocation_id.clone(),
                    step: step.step.clone(),
                    step_index: step.step_index,
                    iteration: step.iteration,
                    current: step.is_current(position.as_ref()),
                },
                Err(error) => SessionFlowMembership::Unknown {
                    reason: format!(
                        "Flow position for Task {} is unavailable: {error}",
                        step.task_id
                    ),
                },
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AskSessionStatus {
    Waiting,
    Completed { summary: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AskSessionRecord {
    id: String,
    parent_run_id: RunId,
    parent_run_dir: PathBuf,
    work: Option<WorkRef>,
    work_selector: Option<String>,
    title: String,
    detail: String,
    prompt: String,
    skill: Option<String>,
    cwd: PathBuf,
    model: String,
    session_run_id: Option<RunId>,
    ready_summary: Option<String>,
    status: AskSessionStatus,
    #[serde(default)]
    retain_completed: bool,
}

#[derive(Debug)]
enum SessionTarget {
    Interactive {
        dir: PathBuf,
        manifest: Box<RunManifest>,
        provider_session: ProviderSessionRef,
    },
    Ask(AskSessionRecord),
    Flow {
        task: Box<Task>,
        position: FlowPosition,
    },
}

pub(crate) async fn ask(
    store: &SharedStore,
    question: &str,
    skill: Option<&str>,
) -> Result<String> {
    let record = prepare_ask_record(store, question, skill).await?;
    write_ask_record(&record)?;
    if let Err(error) = launch_ask(&record).await {
        let _ = fs::remove_file(ask_record_path(&record.id));
        return Err(error);
    }
    report_ask_wait(&record.id);
    wait_for_ask(&record.id).await
}

/// Reuse one Ask for an exact Flow boundary, including its completed result.
pub(crate) async fn ask_once(
    store: &SharedStore,
    key: &str,
    question: &str,
    skill: Option<&str>,
) -> Result<String> {
    if key.trim().is_empty() {
        bail!("Ask continuation key cannot be empty");
    }
    active_run_manifest()?;
    let id = format!("ask_once_{}", hex::encode(Sha256::digest(key.as_bytes())));
    let candidate = if read_ask_record(&id)?.is_none() {
        let mut record = prepare_ask_record(store, question, skill).await?;
        record.id = id.clone();
        record.retain_completed = true;
        Some(record)
    } else {
        None
    };
    // The server holds this lock through provider publication. Wait off the
    // async runtime so concurrent callers can still finish their launches.
    let lock_id = id.clone();
    let launch_lock = tokio::task::spawn_blocking(move || lock_session_launch(&lock_id)).await??;
    let record = match read_ask_record(&id)? {
        Some(record) => record,
        None => {
            let record = candidate.ok_or_else(|| anyhow!("Ask {id} disappeared"))?;
            write_ask_record(&record)?;
            record
        }
    };
    if matches!(record.status, AskSessionStatus::Completed { .. }) {
        drop(launch_lock);
        return wait_for_ask(&id).await;
    }
    if record.session_run_id.is_none() && !ask_launcher_is_running(&record.id).await? {
        // Preserve the record on failure. A retry can start the same Session;
        // a published native Run is reopened only through `lf session open`.
        launch_ask(&record).await.with_context(|| {
            format!("launch human Ask {id}; retry the same boundary to recover")
        })?;
    }
    drop(launch_lock);
    report_ask_wait(&id);
    wait_for_ask(&id).await
}

fn report_ask_wait(id: &str) {
    eprintln!(
        "Waiting for human session {id}. Open it in Loopflow or with `lf session open {id}`."
    );
}

async fn prepare_ask_record(
    store: &SharedStore,
    question: &str,
    skill: Option<&str>,
) -> Result<AskSessionRecord> {
    let question = question.trim();
    if question.is_empty() {
        bail!("question cannot be empty");
    }
    let manifest = active_run_manifest()?;
    let cwd = std::env::current_dir().context("resolve Ask working directory")?;
    if let Some(skill) = skill {
        crate::engine::load_skill(skill, &cwd).context("load Ask skill")?;
    }
    let (work_selector, work) = match preferred_work_selector(&manifest) {
        Some(selector) => match crate::ops::resolve_work_binding(store, &cwd, &selector).await {
            Ok(binding) => (Some(selector), Some(binding.work)),
            Err(_) => (None, None),
        },
        None => (None, None),
    };
    let model = match &manifest.model {
        Some(model) => format!("{}:{model}", manifest.harness),
        None => manifest.harness.clone(),
    };
    let mut record = AskSessionRecord {
        id: format!("ask_{}", uuid::Uuid::new_v4().simple()),
        parent_run_id: manifest.run_id,
        parent_run_dir: PathBuf::from(
            std::env::var_os(RUN_DIR_ENV).expect("active Run manifest requires LF_RUN_DIR"),
        ),
        work,
        work_selector,
        title: question_title(question),
        detail: manifest
            .skill
            .unwrap_or_else(|| "Request for input".to_string()),
        prompt: question.to_string(),
        skill: skill.map(str::to_string),
        cwd,
        model,
        session_run_id: None,
        ready_summary: None,
        status: AskSessionStatus::Waiting,
        retain_completed: false,
    };
    prepare_ask_run(&mut record)?;
    Ok(record)
}

/// Prepare the Run before a human Flow position is committed or presented.
/// Autonomous positions have no Session and keep their optional internal link.
pub(crate) fn prepare_flow_run(task: &Task, position: &mut FlowPosition) -> Result<()> {
    if !position.is_human() || position.session_run_id.is_some() {
        return Ok(());
    }
    let config = crate::engine::config::load_config(Some(&task.worktree))?.unwrap_or_default();
    let (harness, model) = crate::engine::config::parse_agent(config.agent());
    position.session_run_id = Some(crate::run_record::CaptureHandle::prepare(
        crate::run_record::RunSpec {
            harness: harness.to_string(),
            model,
            surface: "tui".to_string(),
            cwd: task.worktree.clone(),
            repo: None,
            worktree: Some(task.worktree.clone()),
            skill: Some(position.current().step),
            subjects: vec![crate::run_record::SubjectAttribution::declared(format!(
                "task:{}",
                task.id
            ))],
            flow: crate::run_record::RunFlowMembership::Step(crate::run_record::RunFlowStep::of(
                position,
            )),
        },
        None,
    )?);
    Ok(())
}

fn prepare_ask_run(record: &mut AskSessionRecord) -> Result<()> {
    if record.session_run_id.is_some() {
        return Ok(());
    }
    let (harness, model) = crate::engine::config::parse_agent(&record.model);
    record.session_run_id = Some(crate::run_record::CaptureHandle::prepare(
        crate::run_record::RunSpec {
            harness: harness.to_string(),
            model,
            surface: "tui".to_string(),
            cwd: record.cwd.clone(),
            repo: None,
            worktree: Some(record.cwd.clone()),
            skill: None,
            subjects: record
                .work_selector
                .clone()
                .map(crate::run_record::SubjectAttribution::declared)
                .into_iter()
                .collect(),
            // An Ask is a human boundary requested by a Run, never a Flow step.
            flow: crate::run_record::RunFlowMembership::Independent,
        },
        Some(record.parent_run_id.clone()),
    )?);
    Ok(())
}

fn session_run_id(id: &str, run: Option<&RunId>) -> Result<RunId> {
    run.cloned().ok_or_else(|| anyhow!(
        "Session {id} predates prepared Runs; run `lf session open '{id}' --json` to prepare its Run without starting a provider"
    ))
}

/// Upgrade an old unbound boundary only through the existing opening mutation.
/// Listing never allocates a Run or changes persisted Session state.
async fn prepare_boundary(store: &SharedStore, id: &str) -> Result<()> {
    let _lock = lock_session_launch(id)?;
    if let Some(mut record) = read_ask_record(id)? {
        if record.session_run_id.is_none() {
            prepare_ask_run(&mut record)?;
            carry_session_name(session_id, observed_run.as_ref(), record.session_run_id.as_ref())?;
            write_ask_record(&record)?;
        }
    } else if let Some((task, mut position)) = find_flow_session_optional(store, id).await? {
        if position.session_run_id.is_none() {
            let placement = store.placement(&position.work()).await?;
            let home = store
                .home_by_id(&placement.home_id)
                .await?
                .ok_or_else(|| anyhow!("Session Home is unavailable"))?;
            if home.route != "local" {
                bail!(
                    "Prepare this Session on its Home with `lf ssh {} session open '{id}' --json`",
                    home.id
                );
            }
            prepare_flow_run(&task, &mut position)?;
            store.set_flow_position(&task.id, position).await?;
        }
    }
    Ok(())
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
    prepare_boundary(store, &flow_id(position)?).await?;
    let position = store
        .flow_position(&task.id)
        .await?
        .ok_or_else(|| anyhow!("human Session disappeared"))?;
    let run_id = session_run_id(&flow_id(&position)?, position.session_run_id.as_ref())?;
    let (dir, _) = crate::run_record::resolve_manifest(
        &crate::store::observability_home_dir(),
        run_id.as_str(),
    )?;
    if dir.join("prepared").exists() {
        launch_flow(task, &position).await?;
    }
    flow_surface(store, task, &position).await
}

pub(crate) async fn list(store: &SharedStore) -> Result<Vec<SessionRecord>> {
    let mut sessions = list_flow_sessions(store).await?;
    sessions.extend(list_ask_sessions(store).await?);
    sessions.extend(crate::ops::flow_session::list()?);
    let boundary_runs = boundary_run_ids(store).await?;
    sessions.extend(list_interactive_sessions(store, &boundary_runs).await?);
    sessions.sort_by(|left, right| left.title.cmp(&right.title).then(left.id.cmp(&right.id)));
    Ok(sessions)
}

async fn find_session(store: &SharedStore, session_id: &str) -> Result<Option<SessionTarget>> {
    let home = crate::store::observability_home_dir();
    match crate::run_record::resolve_manifest(&home, session_id) {
        Ok((dir, manifest)) => {
            if !crate::run_record::has_interactive_history(&dir, &manifest)? {
                bail!("Run {} is not an interactive Session", manifest.run_id);
            }
            if crate::run_record::provider_session_is_resolved(&dir)? {
                bail!("Session {} is resolved", manifest.run_id);
            }
            let provider_session =
                crate::run_record::read_provider_session(&dir)?.ok_or_else(|| {
                    anyhow!("Session {} has no provider history yet", manifest.run_id)
                })?;
            return Ok(Some(SessionTarget::Interactive {
                dir,
                manifest: Box::new(manifest),
                provider_session,
            }));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(anyhow!("Session record unavailable: {error}")),
    }

    if let Some(record) = read_ask_record(session_id)? {
        if !matches!(record.status, AskSessionStatus::Waiting) {
            bail!("session {:?} is already resolved", record.id);
        }
        return Ok(Some(SessionTarget::Ask(record)));
    }
    let Some((task, position)) = find_flow_session_optional(store, session_id).await? else {
        return Ok(None);
    };
    Ok(Some(SessionTarget::Flow {
        task: Box::new(task),
        position,
    }))
}

async fn session_surface(store: &SharedStore, target: &SessionTarget) -> Result<SessionRecord> {
    match target {
        SessionTarget::Interactive { dir, manifest, .. } => {
            interactive_surface(store, dir, manifest).await
        }
        SessionTarget::Ask(record) => ask_surface(store, record).await,
        SessionTarget::Flow { task, position } => flow_surface(store, task, position).await,
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
            let mut position = store
                .flow_position(&token.task_id)
                .await?
                .ok_or_else(|| anyhow!("review session is no longer waiting"))?;
            if !token_matches(&token, &position)
                || position.session_run_id.as_ref() != Some(&run_id)
            {
                bail!("review session is stale");
            }
            position.ready_summary = Some(summary.to_string());
            position.updated_at = time::OffsetDateTime::now_utc();
            store.set_flow_position(&token.task_id, position).await?;
        }
        HumanSessionToken::Ask { id } => {
            let lock_id = id.clone();
            let _lock =
                tokio::task::spawn_blocking(move || lock_session_launch(&lock_id)).await??;
            let mut record =
                read_ask_record(&id)?.ok_or_else(|| anyhow!("session {id:?} no longer exists"))?;
            if !matches!(record.status, AskSessionStatus::Waiting)
                || record.session_run_id.as_ref() != Some(&run_id)
            {
                bail!("session is stale");
            }
            record.ready_summary = Some(summary.to_string());
            write_ask_record(&record)?;
        }
    }
    Ok(())
}

async fn complete_flow(store: &SharedStore, task: &Task, position: &FlowPosition) -> Result<()> {
    let token = flow_token(task, position)?;
    crate::controller::task::complete_human_flow_step(store, &token).await?;
    let mut task = store
        .get_task(&token.task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {} disappeared after review completion", token.task_id))?;
    let launch = if store.flow_position(&task.id).await?.is_some() {
        crate::ops::task::relaunch_inactive_process(store, &mut task)
            .await
            .map_err(|error| anyhow!(error.to_string()))
    } else {
        Ok(())
    };
    stop_flow_run(store, &task, position).await;
    launch.with_context(|| {
        format!(
            "Review feedback saved; continue with `lf task advance {}`",
            task.plan.identifier
        )
    })
}

pub(crate) async fn completion_worktree(
    store: &SharedStore,
    session_id: &str,
) -> Result<Option<PathBuf>> {
    if let Some(token) = crate::ops::flow_session::parse_id(session_id)? {
        return crate::ops::flow_session::worktree(&token).map(Some);
    }
    match find_session(store, session_id)
        .await?
        .ok_or_else(|| session_not_found(session_id))?
    {
        SessionTarget::Flow { task, .. } => Ok(Some(task.worktree.clone())),
        SessionTarget::Interactive { .. } | SessionTarget::Ask(_) => Ok(None),
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

async fn complete_ask(session_id: &str) -> Result<()> {
    let lock_id = session_id.to_string();
    let _lock = tokio::task::spawn_blocking(move || lock_session_launch(&lock_id)).await??;
    let Some(mut record) = read_ask_record(session_id)? else {
        return Err(session_not_found(session_id));
    };
    if !matches!(record.status, AskSessionStatus::Waiting) {
        bail!("Ask session {session_id:?} is already complete");
    }
    require_session_action(
        SessionKind::Ask,
        if record.ready_summary.is_some() {
            SessionState::Ready
        } else {
            SessionState::Waiting
        },
        SessionActionKind::Complete,
    )?;
    let summary = record.ready_summary.clone().ok_or_else(|| {
        anyhow!(
            "Ask session {session_id:?} is not ready; its agent must run `lf session ready \"<summary>\"` first"
        )
    })?;
    record.status = AskSessionStatus::Completed { summary };
    write_ask_record(&record)?;
    // The human's answer is durable before teardown can interrupt this caller
    // or fail. A cleanup failure must not strand the waiting decision agent.
    if let Some(run_id) = &record.session_run_id {
        if let Err(error) = stop_native_run(run_id) {
            tracing::warn!(session_id, %run_id, error = %format!("{error:#}"),
                "Ask completion saved, but its native Session could not be stopped");
        }
    }
    Ok(())
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
    serve_flow_locked(store, token, launch_lock).await
}

async fn serve_flow_locked(
    store: SharedStore,
    token: FlowSessionToken,
    launch_lock: File,
) -> Result<()> {
    let task = store
        .get_task(&token.task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {} disappeared", token.task_id))?;
    validate_token(&store, &token).await?;
    let position = store
        .flow_position(&token.task_id)
        .await?
        .ok_or_else(|| anyhow!("review session is no longer waiting"))?;
    if let Some(failure) = &position.failure {
        bail!("review session cannot start: {}", failure.reason);
    }
    let message = flow_message(&task, &token);
    let lf = crate::engine::process::resolve_current_home_lf_binary_checked()?;
    let selector = format!("task:{}", token.task_id);
    let serialized = serde_json::to_string(&HumanSessionToken::Flow {
        token: Box::new(token.clone()),
    })?;
    let mut command = tokio::process::Command::new(lf);
    command
        .args(["--tui", "--as", &selector, &token.skill.name, &message])
        .current_dir(&task.worktree)
        .env(HUMAN_SESSION_ENV, serialized);
    let run_id = session_run_id(&flow_token_id(&token), position.session_run_id.as_ref())?;
    let mut child = spawn_session_run(&mut command, &run_id).await?;
    let position = store
        .flow_position(&token.task_id)
        .await?
        .ok_or_else(|| anyhow!("review session is no longer waiting"))?;
    if !token_matches(&token, &position) {
        let _ = child.kill().await;
        bail!("review session is stale");
    }
    drop(launch_lock);
    let status = child.wait().await.context("wait for review skill")?;
    if !token_is_current(&store, &token).await? {
        let execution = crate::ops::task_execution::task_execution(&store, &token.task_id).await?;
        println!("Review session finished. {}", execution.reason);
        return Ok(());
    }
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("review skill exited with {status}"))
    }
}

pub(crate) async fn serve_ask(id: &str) -> Result<()> {
    let launch_lock = lock_session_launch(id)?;
    serve_ask_locked(id, launch_lock).await
}

async fn serve_ask_locked(id: &str, launch_lock: File) -> Result<()> {
    let mut record =
        read_ask_record(id)?.ok_or_else(|| anyhow!("session {id:?} no longer exists"))?;
    if !matches!(record.status, AskSessionStatus::Waiting) {
        bail!("session {id:?} is already resolved");
    }
    if record.session_run_id.as_ref().is_some_and(|id| !run_is_prepared(id).unwrap_or(false)) {
        // Another launcher already published this Session. Do not replace or
        // infer death of its native client; explicit open owns native resume.
        return Ok(());
    }
    let lf = crate::engine::process::resolve_current_home_lf_binary_checked()?;
    let mut command = tokio::process::Command::new(lf);
    command.args(ask_launch_args(&record)).current_dir(&record.cwd).env(
        HUMAN_SESSION_ENV,
        serde_json::to_string(&HumanSessionToken::Ask {
            id: record.id.clone(),
        })?,
    );
    if record.session_run_id.is_none() {
        prepare_ask_run(&mut record)?;
        write_ask_record(&record)?;
    }
    let run_id = session_run_id(id, record.session_run_id.as_ref())?;
    let mut child = spawn_session_run(&mut command, &run_id).await?;
    drop(launch_lock);
    let status = child.wait().await.context("wait for session agent")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("session agent exited with {status}"))
    }
}

fn ask_launch_args(record: &AskSessionRecord) -> Vec<String> {
    let mut args = vec![
        "--tui".to_string(),
        "--model".to_string(),
        record.model.clone(),
        "--__cwd".to_string(),
        record.cwd.display().to_string(),
    ];
    if let Some(selector) = &record.work_selector {
        args.extend(["--as".to_string(), selector.clone()]);
    }
    match &record.skill {
        Some(skill) => args.extend(["skill".to_string(), skill.clone()]),
        None => args.push(":".to_string()),
    }
    args.push(ask_message(record));
    args
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
    if let Some(token) = crate::ops::flow_session::parse_id(session_id)? {
        return crate::ops::flow_session::open(&token, mode, resume).await;
    }
    let target = find_session(store, session_id)
        .await?
        .ok_or_else(|| session_not_found(session_id))?;
    match &target {
        SessionTarget::Interactive {
            dir,
            manifest,
            provider_session,
        } => {
            let active_clients =
                crate::lf::commands::util::active_provider_clients(dir, &manifest.harness)?;
            match mode {
                OpenMode::Refuse if !active_clients.is_empty() => {
                    require_session_action(
                        SessionKind::Interactive,
                        SessionState::Active,
                        SessionActionKind::Open,
                    )?;
                }
                OpenMode::Replace if resume => crate::lf::commands::util::replace_provider_clients(
                    dir,
                    &manifest.harness,
                    &active_clients,
                    crate::run_record::ProviderClientStopReason::Moved,
                )?,
                OpenMode::Replace => {}
                OpenMode::Refuse | OpenMode::Try => {}
            }
            let mut session = interactive_surface(store, dir, manifest).await?;
            if resume {
                crate::lf::commands::util::resume_session(
                    &manifest.harness,
                    manifest.model.as_deref(),
                    &manifest.cwd,
                    &manifest.run_id,
                    dir,
                    provider_session,
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
        SessionTarget::Ask(_) | SessionTarget::Flow { .. } => {
            if mode != OpenMode::Refuse {
                bail!("--replace and --try apply only to interactive provider sessions");
            }
            prepare_boundary(store, session_id).await?;
            let target = find_session(store, session_id)
                .await?
                .ok_or_else(|| session_not_found(session_id))?;
            let mut session = session_surface(store, &target).await?;
            if resume {
                session.run_id = open_boundary(store, session_id).await?;
            }
            Ok(session)
        }
    }
}

pub(crate) async fn complete(store: &SharedStore, session_id: &str) -> Result<SessionRecord> {
    if let Some(token) = crate::ops::flow_session::parse_id(session_id)? {
        let session = crate::ops::flow_session::surface(&token)?;
        crate::ops::flow_session::complete(&token).await?;
        return Ok(session);
    }
    let target = find_session(store, session_id)
        .await?
        .ok_or_else(|| session_not_found(session_id))?;
    let session = session_surface(store, &target).await?;
    require_session_action(session.kind, session.state, SessionActionKind::Complete)?;
    match &target {
        SessionTarget::Interactive { dir, manifest, .. } => {
            let active_clients =
                crate::lf::commands::util::active_provider_clients(dir, &manifest.harness)?;
            crate::lf::commands::util::replace_provider_clients(
                dir,
                &manifest.harness,
                &active_clients,
                crate::run_record::ProviderClientStopReason::Completed,
            )?;
            crate::run_record::resolve_provider_session(dir)
                .map_err(|error| anyhow!("cannot complete Session {}: {error}", manifest.run_id))?;
        }
        SessionTarget::Ask(_) => {
            complete_ask(session_id).await?;
        }
        SessionTarget::Flow { task, position } => {
            complete_flow(store, task, position).await?;
        }
    }
    Ok(session)
}

async fn open_boundary(store: &SharedStore, session_id: &str) -> Result<RunId> {
    if let Some(mut record) = read_ask_record(session_id)? {
        loop {
            if !matches!(record.status, AskSessionStatus::Waiting) {
                bail!("session {session_id:?} is already complete");
            }
            let observed_run = record.session_run_id.clone();
            if let Some(run_id) = &observed_run {
                if resume_native_run(run_id, &HumanSessionToken::Ask { id: record.id.clone() })? {
                    return Ok(run_id.clone());
                }
            }
            let lock_id = session_id.to_string();
            let launch_lock = tokio::task::spawn_blocking(move || lock_session_launch(&lock_id)).await??;
            record = read_ask_record(session_id)?
                .ok_or_else(|| anyhow!("session {session_id:?} no longer exists"))?;
            if !matches!(record.status, AskSessionStatus::Waiting) {
                bail!("session {session_id:?} is already complete");
            }
            if record.session_run_id != observed_run {
                drop(launch_lock);
                continue;
            }
            if let Some(run_id) = &record.session_run_id {
                if !run_is_prepared(run_id)? {
                    record.session_run_id = None;
                    record.ready_summary = None;
                }
            }
            prepare_ask_run(&mut record)?;
            write_ask_record(&record)?;
            let run_id = session_run_id(session_id, record.session_run_id.as_ref())?;
            serve_ask_locked(session_id, launch_lock).await?;
            return Ok(run_id);
        }
    }
    let launch_lock = lock_session_launch(session_id)?;
    let (task, mut position) = find_flow_session(store, session_id).await?;
    let token = flow_token(&task, &position)?;
    let previous = position.session_run_id.clone();
    if let Some(run_id) = &previous {
        if resume_native_run(
            run_id,
            &HumanSessionToken::Flow {
                token: Box::new(token.clone()),
            },
        )? {
            return Ok(run_id.clone());
        }
        if !run_is_prepared(run_id)? {
            position.session_run_id = None;
            position.ready_summary = None;
        }
    }
    prepare_flow_run(&task, &mut position)?;
    carry_session_name(
        session_id,
        previous.as_ref(),
        position.session_run_id.as_ref(),
    )?;
    let run_id = session_run_id(session_id, position.session_run_id.as_ref())?;
    store.set_flow_position(&task.id, position).await?;
    serve_flow_locked(store.clone(), token, launch_lock).await?;
    Ok(run_id)
}

/// A replacement Run continues the same Session, so it keeps that Session's
/// name. Copy it before the boundary publishes the new Run.
fn carry_session_name(
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

fn run_is_prepared(run_id: &RunId) -> Result<bool> {
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

async fn boundary_run_ids(store: &SharedStore) -> Result<HashSet<RunId>> {
    let mut run_ids = store
        .human_task_flow_positions()
        .await?
        .into_iter()
        .filter_map(|position| position.session_run_id)
        .collect::<HashSet<_>>();
    for session in crate::ops::flow_session::list()? {
        if let Some(token) = crate::ops::flow_session::parse_id(&session.id)? {
            if let Some(id) = crate::ops::flow_run::read(&token.invocation)?
                .active
                .and_then(|b| b.run_id)
            {
                run_ids.insert(id);
            }
        }
    }
    run_ids.extend(
        ask_records()?
            .into_iter()
            .filter_map(|record| record.session_run_id),
    );
    Ok(run_ids)
}

fn ask_records() -> Result<Vec<AskSessionRecord>> {
    let entries = match fs::read_dir(ask_session_directory()) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).context("read session directory"),
    };
    let mut records = Vec::new();
    for entry in entries {
        let entry = entry.context("read session entry")?;
        let Some(id) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
            .map(str::to_string)
        else {
            continue;
        };
        records.extend(read_ask_record(&id)?);
    }
    Ok(records)
}

async fn list_interactive_sessions(
    store: &SharedStore,
    human_runs: &HashSet<RunId>,
) -> Result<Vec<SessionRecord>> {
    let home = crate::store::observability_home_dir();
    let runs = crate::run_record::scan_unresolved_provider_runs(&home)
        .map_err(|error| anyhow!("Session records unavailable: {error}"))?;
    let mut sessions = Vec::new();
    for (dir, manifest) in runs {
        if human_runs.contains(&manifest.run_id) {
            continue;
        }
        sessions.push(interactive_surface(store, &dir, &manifest).await?);
    }
    Ok(sessions)
}

async fn interactive_surface(
    store: &SharedStore,
    dir: &Path,
    manifest: &RunManifest,
) -> Result<SessionRecord> {
    let clients = crate::lf::commands::util::active_provider_clients(dir, &manifest.harness)?;
    let state = if clients.is_empty() {
        SessionState::Closed
    } else {
        SessionState::Active
    };
    let lf = crate::engine::process::resolve_current_home_lf_binary_checked()?;
    let work = crate::run_record::attributed_work(store, manifest).await;
    let name = session_name(
        Some(dir),
        manifest
            .skill
            .clone()
            .unwrap_or_else(|| crate::engine::naming::word_pair(manifest.run_id.as_str())),
    )?;
    Ok(SessionRecord {
        id: manifest.run_id.to_string(),
        run_id: manifest.run_id.clone(),
        kind: SessionKind::Interactive,
        wave_id: session_wave_id(store, work.as_ref()).await?,
        work_path: session_work_path(store, work.as_ref()).await?,
        work,
        actions: session_actions(SessionKind::Interactive, state),
        title: name.title,
        title_source: name.source,
        flow_membership: run_flow_membership(store, Some(manifest), &manifest.run_id).await,
        detail: match &manifest.model {
            Some(model) => format!("{}:{model}", manifest.harness),
            None => manifest.harness.clone(),
        },
        cwd: manifest.cwd.display().to_string(),
        state,
        ready_summary: None,
        terminal_ids: clients
            .into_iter()
            .filter_map(|client| client.terminal_id)
            .collect(),
        open_argv: vec![
            lf.display().to_string(),
            "session".to_string(),
            "open".to_string(),
            manifest.run_id.to_string(),
        ],
    })
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
fn session_name(dir: Option<&Path>, seed: String) -> Result<SessionName> {
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
    let mut target = find_named_session(store, session_id)
        .await?
        .ok_or_else(|| session_not_found(session_id))?;
    // A boundary may replace its Run while opening. Serialize with that
    // preparation and name the Run it publishes, not the one it replaced.
    let _launch_lock = if matches!(target, NamedSession::Boundary(_)) {
        let id = named_surface(store, &target).await?.id;
        let lock = lock_session_launch(&id)?;
        target = NamedSession::Boundary(Box::new(
            find_session(store, &id)
                .await?
                .ok_or_else(|| session_not_found(&id))?,
        ));
        Some(lock)
    } else {
        None
    };
    let session = named_surface(store, &target).await?;
    if let NamedSession::Boundary(boundary) = &target {
        if let SessionTarget::Flow { position, .. } = boundary.as_ref() {
            let placement = store.placement(&position.work()).await?;
            let home = store
                .home_by_id(&placement.home_id)
                .await?
                .ok_or_else(|| anyhow!("Session Home {} is unavailable", placement.home_id))?;
            if home.route != "local" {
                let prefix = &session.open_argv[..session.open_argv.len().saturating_sub(3)];
                bail!(
                "Session {} runs on Home {}; rename it there with `{} session rename '{}' <name>`",
                session.id,
                placement.home_id,
                prefix.join(" "),
                session.id
            );
            }
        }
    }
    let dir = local_session_run_dir(&session.run_id)
        .ok_or_else(|| anyhow!("Session {} has an invalid Run reference", session.id))?;
    crate::run_record::write_session_name(&dir, title, source)
        .map_err(|error| anyhow!("cannot rename Session {}: {error}", session.id))?;
    named_surface(store, &target).await
}

enum NamedSession {
    Interactive {
        dir: PathBuf,
        manifest: Box<RunManifest>,
    },
    Boundary(Box<SessionTarget>),
}

async fn named_surface(store: &SharedStore, target: &NamedSession) -> Result<SessionRecord> {
    match target {
        NamedSession::Interactive { dir, manifest } => {
            interactive_surface(store, dir, manifest).await
        }
        NamedSession::Boundary(target) => session_surface(store, target).await,
    }
}

async fn find_named_session(store: &SharedStore, id: &str) -> Result<Option<NamedSession>> {
    if let Some(target) = find_boundary_for_run(store, id).await? {
        return Ok(Some(NamedSession::Boundary(Box::new(target))));
    }
    match crate::run_record::resolve_manifest(&crate::store::observability_home_dir(), id) {
        Ok((dir, manifest)) => {
            if !crate::run_record::has_interactive_history(&dir, &manifest)? {
                bail!("Run {} is not an interactive Session", manifest.run_id);
            }
            if crate::run_record::provider_session_is_resolved(&dir)? {
                bail!("Session {} is resolved", manifest.run_id);
            }
            return Ok(Some(NamedSession::Interactive {
                dir,
                manifest: Box::new(manifest),
            }));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(anyhow!("Session record unavailable: {error}")),
    }
    Ok(find_session(store, id)
        .await?
        .map(|target| NamedSession::Boundary(Box::new(target))))
}

/// The waiting Ask or Flow boundary whose linked Run is `run_id`.
async fn find_boundary_for_run(store: &SharedStore, run_id: &str) -> Result<Option<SessionTarget>> {
    for record in ask_records()? {
        if record.session_run_id.as_ref().map(RunId::as_str) == Some(run_id)
            && matches!(record.status, AskSessionStatus::Waiting)
        {
            return Ok(Some(SessionTarget::Ask(record)));
        }
    }
    for position in store.human_task_flow_positions().await? {
        if position.session_run_id.as_ref().map(RunId::as_str) != Some(run_id) {
            continue;
        }
        let task = store
            .get_task(&position.task_id)
            .await?
            .ok_or_else(|| anyhow!("Task {} disappeared", position.task_id))?;
        return Ok(Some(SessionTarget::Flow {
            task: Box::new(task),
            position,
        }));
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
    let (dir, _) = crate::run_record::resolve_manifest(&home, run_id.as_str())?;
    let mut child = command
        .env(PREPARED_RUN_ENV, run_id.as_str())
        .kill_on_drop(true)
        .spawn()
        .context("launch Session Run")?;
    let deadline = tokio::time::Instant::now() + SESSION_START_TIMEOUT;
    loop {
        let manifest = crate::run_record::read_manifest(&dir)?;
        if session_run_is_resumable(&dir, &manifest)? {
            return Ok(child);
        }
        if let Some(status) = child.try_wait().context("probe human Session Run")? {
            bail!("human Session Run {run_id} exited with {status} before becoming resumable");
        }
        if tokio::time::Instant::now() >= deadline {
            bail!("human Session Run {run_id} did not become resumable within 30s");
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

pub(crate) fn resume_native_run(run_id: &RunId, token: &HumanSessionToken) -> Result<bool> {
    let home = crate::store::observability_home_dir();
    let (dir, manifest) = match crate::run_record::resolve_manifest(&home, run_id.as_str()) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).context("resolve Session Run"),
    };
    let clients = crate::lf::commands::util::active_provider_clients(&dir, &manifest.harness)?;
    crate::lf::commands::util::replace_provider_clients(
        &dir,
        &manifest.harness,
        &clients,
        crate::run_record::ProviderClientStopReason::Moved,
    )?;
    let Some(provider_session) = crate::run_record::read_provider_session(&dir)? else {
        return Ok(false);
    };
    let environment =
        BTreeMap::from([(HUMAN_SESSION_ENV.to_string(), serde_json::to_string(token)?)]);
    // Native resume waits until the conversation exits. Preparation is finished;
    // keeping its lock here would prevent another reader/opener from returning.
    FileExt::unlock(launch_lock).context("release human Session preparation lock")?;
    crate::lf::commands::util::resume_session_with_env(
        &manifest.harness,
        manifest.model.as_deref(),
        &manifest.cwd,
        &manifest.run_id,
        &dir,
        &provider_session,
        &environment,
    )?;
    Ok(true)
}

fn stop_native_run(run_id: &RunId) -> Result<()> {
    let home = crate::store::observability_home_dir();
    let (dir, manifest) = match crate::run_record::resolve_manifest(&home, run_id.as_str()) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).context("resolve Session Run"),
    };
    let clients = crate::lf::commands::util::active_provider_clients(&dir, &manifest.harness)?;
    crate::lf::commands::util::replace_provider_clients(
        &dir,
        &manifest.harness,
        &clients,
        crate::run_record::ProviderClientStopReason::Completed,
    )?;
    if crate::run_record::read_provider_session(&dir)?.is_some() {
        crate::run_record::resolve_provider_session(&dir)?;
    }
    Ok(())
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
    let mut sessions = Vec::new();
    for position in store.human_task_flow_positions().await? {
        let Some(task) = store.get_task(&position.task_id).await? else {
            continue;
        };
        if store.work_status(&position.work()).await? != WorkStatus::Ready {
            continue;
        }
        sessions.push(flow_surface(store, &task, &position).await?);
    }
    Ok(sessions)
}

async fn list_ask_sessions(store: &SharedStore) -> Result<Vec<SessionRecord>> {
    let directory = ask_session_directory();
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).context("read session directory"),
    };
    let mut sessions = Vec::new();
    for entry in entries {
        let entry = entry.context("read session entry")?;
        let file_name = entry.file_name();
        let Some(id) = file_name
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
        else {
            continue;
        };
        let Some(record) = read_ask_record(id)? else {
            continue;
        };
        if matches!(record.status, AskSessionStatus::Waiting) {
            sessions.push(ask_surface(store, &record).await?);
        }
    }
    Ok(sessions)
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
    for position in store.human_task_flow_positions().await? {
        if flow_id(&position)? != session_id {
            continue;
        }
        let task = store
            .get_task(&position.task_id)
            .await?
            .ok_or_else(|| anyhow!("Task {} disappeared", position.task_id))?;
        return Ok(Some((task, position)));
    }
    Ok(None)
}

async fn flow_surface(
    store: &SharedStore,
    task: &Task,
    position: &FlowPosition,
) -> Result<SessionRecord> {
    validate_task_position(task, position)?;
    let step = position.current();
    let placement = store.placement(&position.work()).await?;
    let home = store
        .home_by_id(&placement.home_id)
        .await?
        .ok_or_else(|| anyhow!("Task {} Home {} disappeared", task.id, placement.home_id))?;
    let id = flow_id(position)?;
    let run_id = session_run_id(&id, position.session_run_id.as_ref())?;
    // A remote Session's canonical name lives with its Run on that Home.
    // Show the step as a label without claiming it is the generated name.
    let name = if home.route == "local" {
        session_name(local_session_run_dir(&run_id).as_deref(), step.step.clone())?
    } else {
        SessionName {
            title: step.step.clone(),
            source: SessionTitleSource::Unavailable,
        }
    };
    let open_argv = human_open_argv(
        (home.route != "local").then_some(&home.id),
        Some(&task.worktree),
        &id,
    )?;
    let runtime = if home.route == "local" {
        native_session_state(
            position.session_run_id.as_ref(),
            position.ready_summary.as_deref(),
        )?
    } else if position.ready_summary.is_some() {
        SessionState::Ready
    } else if position.session_run_id.is_some() {
        SessionState::Closed
    } else {
        SessionState::Waiting
    };
    Ok(SessionRecord {
        run_id,
        id,
        kind: SessionKind::Flow,
        wave_id: Some(task.wave_id.clone()),
        work: Some(position.work()),
        work_path: session_work_path(store, Some(&position.work())).await?,
        actions: flow_actions(position),
        title: name.title,
        title_source: name.source,
        flow_membership: SessionFlowMembership::of_position(position),
        detail: step.step,
        cwd: task.worktree.display().to_string(),
        state: runtime,
        ready_summary: position.ready_summary.clone(),
        terminal_ids: Vec::new(),
        open_argv,
    })
}

async fn ask_surface(store: &SharedStore, record: &AskSessionRecord) -> Result<SessionRecord> {
    let open_argv = human_open_argv(None, None, &record.id)?;
    let runtime = native_session_state(
        record.session_run_id.as_ref(),
        record.ready_summary.as_deref(),
    )?;
    let run_id = session_run_id(&record.id, record.session_run_id.as_ref())?;
    let dir = local_session_run_dir(&run_id);
    let name = session_name(dir.as_deref(), record.title.clone())?;
    let manifest = dir
        .as_deref()
        .map(crate::run_record::read_manifest)
        .transpose()
        .ok()
        .flatten();
    let flow_membership = run_flow_membership(store, manifest.as_ref(), &run_id).await;
    Ok(SessionRecord {
        id: record.id.clone(),
        run_id,
        kind: SessionKind::Ask,
        wave_id: session_wave_id(store, record.work.as_ref()).await?,
        work: record.work.clone(),
        work_path: session_work_path(store, record.work.as_ref()).await?,
        actions: session_actions(SessionKind::Ask, runtime),
        title: name.title,
        title_source: name.source,
        flow_membership,
        detail: record.detail.clone(),
        cwd: record.cwd.display().to_string(),
        state: runtime,
        ready_summary: record.ready_summary.clone(),
        terminal_ids: Vec::new(),
        open_argv,
    })
}

pub(crate) fn human_open_argv(
    remote_home: Option<&crate::durable::HomeId>,
    worktree: Option<&Path>,
    id: &str,
) -> Result<Vec<String>> {
    let lf = crate::engine::process::resolve_current_home_lf_binary_checked()?
        .display()
        .to_string();
    let mut argv = vec![lf];
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

fn ask_message(record: &AskSessionRecord) -> String {
    format!(
        "{}\n\n<lf:human-session>\nThe originating Loopflow Run is blocked while you work with the user in this terminal. You are in the caller's checkout and may inspect or edit it. Save useful findings and user decisions in self-contained topic notes under scratch/; include their exact paths and a short takeaway in the ready summary. When the work is ready, run `lf session ready \"<concise summary>\"`. Ready keeps this session visible and does not resume the caller; the user completes it when the conversation is finished.\n</lf:human-session>",
        record.prompt
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

async fn launch_ask(record: &AskSessionRecord) -> Result<()> {
    let lf = crate::engine::process::resolve_current_home_lf_binary();
    let argv = vec![
        lf.to_string_lossy().to_string(),
        "session".to_string(),
        "serve-ask".to_string(),
        record.id.clone(),
    ];
    let run_id = record.parent_run_id.to_string();
    let run_dir = record.parent_run_dir.to_string_lossy().to_string();
    start_durable_session(
        &ask_background_name(&record.id),
        &record.cwd,
        &argv,
        &[
            (crate::durable::RUN_ID_ENV, run_id.as_str()),
            (RUN_DIR_ENV, run_dir.as_str()),
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
    let directory = ask_session_directory();
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

fn ask_session_directory() -> PathBuf {
    crate::store::current_home_lf_home_dir().join(ASK_SESSION_DIRECTORY)
}

fn ask_record_path(id: &str) -> PathBuf {
    ask_session_directory().join(format!("{id}.json"))
}

fn write_ask_record(record: &AskSessionRecord) -> Result<()> {
    let directory = ask_session_directory();
    fs::create_dir_all(&directory).context("create session directory")?;
    #[cfg(unix)]
    fs::set_permissions(
        &directory,
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .context("protect session directory")?;
    let path = ask_record_path(&record.id);
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
    let bytes = serde_json::to_vec_pretty(record)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&temporary).context("stage session record")?;
    file.write_all(&bytes).context("write session record")?;
    file.sync_all().context("sync session record")?;
    fs::rename(&temporary, &path).context("publish session record")
}

fn read_ask_record(id: &str) -> Result<Option<AskSessionRecord>> {
    let path = ask_record_path(id);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("read session record"),
    };
    let record: AskSessionRecord =
        serde_json::from_slice(&bytes).context("parse session record")?;
    if record.id != id {
        bail!("session record {id:?} is invalid");
    }
    Ok(Some(record))
}

async fn wait_for_ask(id: &str) -> Result<String> {
    loop {
        let record = read_ask_record(id)?
            .ok_or_else(|| anyhow!("session {id:?} disappeared before resolution"))?;
        match record.status {
            AskSessionStatus::Waiting => tokio::time::sleep(Duration::from_millis(250)).await,
            AskSessionStatus::Completed { summary } => {
                if !record.retain_completed {
                    fs::remove_file(ask_record_path(id)).context("remove resolved session")?;
                }
                return Ok(summary);
            }
        }
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
        active_flow_skill, ask, ask_background_name, ask_launch_args, ask_once, ask_record_path,
        complete_ask, flow_background_name, flow_id, flow_token_id, human_open_argv,
        list_ask_sessions, preferred_work_selector, question_title, read_ask_record, serve_ask,
        session_run_is_resumable, token_matches, wait_for_ask, write_ask_record, AskSessionRecord,
        AskSessionStatus, FlowSessionToken, HumanSessionToken, HUMAN_SESSION_ENV,
    };
    use crate::durable::{FlowPosition, RunId};
    use crate::engine::{prepare_launch_prompt, Config, LaunchPromptInput, Surface};
    use crate::lf::{Cli, Commands};
    use crate::run_record::{AttributionSource, RunManifest, SubjectAttribution};
    use crate::store::{open_ephemeral_store, SharedStore, StorageConfig};
    use crate::work::task::TaskId;

    pub(super) static ASK_LAUNCHERS: LazyLock<Mutex<HashSet<String>>> =
        LazyLock::new(|| Mutex::new(HashSet::new()));
    pub(super) static FAILED_ASK_LAUNCHERS: LazyLock<Mutex<HashSet<String>>> =
        LazyLock::new(|| Mutex::new(HashSet::new()));

    struct AskHome {
        home: tempfile::TempDir,
        previous: Vec<(&'static str, Option<OsString>)>,
    }

    impl AskHome {
        fn new() -> Self {
            let home = tempfile::tempdir().unwrap();
            let previous = [
                "LF_HOME",
                "LF_BIN",
                "LF_DB_PATH",
                "LF_CONTROL_HOME",
                "LF_CONTROL_DB_PATH",
                "LF_RUN_ID",
                "LF_RUN_DIR",
                "LF_RUN_CONTEXT",
                "LF_HUMAN_SESSION",
            ]
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
            Self { home, previous }
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

    async fn wait_until_asks(count: usize) -> Vec<super::SessionRecord> {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let sessions = list_ask_sessions().await.unwrap();
                if sessions.len() == count && ASK_LAUNCHERS.lock().unwrap().len() == count {
                    return sessions;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }

    async fn finish_simulated_ask(id: &str, summary: &str) {
        let mut record = read_ask_record(id).unwrap().unwrap();
        record.ready_summary = Some(summary.to_string());
        write_ask_record(&record).unwrap();
        complete_ask(id).await.unwrap();
    }

    #[test]
    fn keyed_asks_join_recover_completion_and_keep_boundaries_independent() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let key = "/private/checkout/invocation/decision/visit-1";
            let first = ask_once(&store, key, "Choose a policy", Some("unblock"));
            let duplicate = ask_once(&store, key, "Changed retry text", Some("unblock"));
            let independent =
                ask_once(&store, "invocation/decision/visit-2", "Second choice", None);
            let human = async {
                let sessions = wait_until_asks(2).await;
                for session in sessions {
                    assert!(session.id.starts_with("ask_once_"));
                    assert!(!session.id.contains("checkout"));
                    let record = read_ask_record(&session.id).unwrap().unwrap();
                    assert_eq!(record.parent_run_dir, home.home.path());
                    assert_eq!(record.model, "codex:test");
                    let summary = if record.prompt == "Second choice" {
                        "second"
                    } else {
                        assert_eq!(record.skill.as_deref(), Some("unblock"));
                        "first"
                    };
                    finish_simulated_ask(&session.id, summary).await;
                }
            };
            let (first, duplicate, independent, ()) =
                tokio::join!(first, duplicate, independent, human);
            assert_eq!(first.unwrap(), "first");
            assert_eq!(duplicate.unwrap(), "first");
            assert_eq!(independent.unwrap(), "second");
            assert!(list_ask_sessions().await.unwrap().is_empty());
            // Completed recovery must not reload a now-missing selected skill.
            assert_eq!(
                ask_once(&store, key, "", Some("missing-skill"))
                    .await
                    .unwrap(),
                "first"
            );
            assert_eq!(ASK_LAUNCHERS.lock().unwrap().len(), 2);
        });
    }

    #[test]
    fn keyed_ask_failed_launch_can_retry_the_preserved_record() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let key = "failed-launch-boundary";
            let id = format!(
                "ask_once_{}",
                hex::encode(super::Sha256::digest(key.as_bytes()))
            );
            FAILED_ASK_LAUNCHERS
                .lock()
                .unwrap()
                .insert(ask_background_name(&id));
            let error = ask_once(&store, key, "Preserved question", None)
                .await
                .unwrap_err();
            assert!(error.to_string().contains(&id));
            assert_eq!(
                read_ask_record(&id).unwrap().unwrap().prompt,
                "Preserved question"
            );
            FAILED_ASK_LAUNCHERS.lock().unwrap().clear();
            let human = async {
                let sessions = wait_until_asks(1).await;
                assert_eq!(sessions[0].id, id);
                finish_simulated_ask(&id, "Recovered").await;
            };
            let (result, ()) = tokio::join!(ask_once(&store, key, "replacement", None), human);
            assert_eq!(result.unwrap(), "Recovered");
            assert_eq!(wait_for_ask(&id).await.unwrap(), "Recovered");
        });
    }

    #[test]
    fn keyed_ask_does_not_replace_a_published_native_run() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let mut record = super::prepare_ask_record(&store, "Keep native ownership", None)
                .await
                .unwrap();
            let key = "published-run-boundary";
            record.id = format!(
                "ask_once_{}",
                hex::encode(super::Sha256::digest(key.as_bytes()))
            );
            record.retain_completed = true;
            record.session_run_id = Some(RunId::new());
            write_ask_record(&record).unwrap();
            // No tmux launcher and no readable native manifest do not grant
            // replacement authority. Even a delayed serve command must join.
            serve_ask(&record.id).await.unwrap();
            assert!(tokio::time::timeout(
                Duration::from_millis(100),
                ask_once(&store, key, "retry", None)
            )
            .await
            .is_err());
            let saved = read_ask_record(&record.id).unwrap().unwrap();
            assert_eq!(saved.session_run_id, record.session_run_id);
            assert!(ASK_LAUNCHERS.lock().unwrap().is_empty());
            std::env::set_var("LF_RUN_ID", saved.session_run_id.as_ref().unwrap().as_str());
            std::env::set_var(
                HUMAN_SESSION_ENV,
                serde_json::to_string(&HumanSessionToken::Ask {
                    id: record.id.clone(),
                })
                .unwrap(),
            );
            super::mark_ready(&store, "Resolved").await.unwrap();
            complete_ask(&record.id).await.unwrap();
            assert!(super::mark_ready(&store, "Late readiness").await.is_err());
            assert_eq!(wait_for_ask(&record.id).await.unwrap(), "Resolved");
            assert!(list_ask_sessions().await.unwrap().is_empty());
        });
    }

    #[test]
    fn reopening_ask_cannot_overwrite_a_concurrent_completion() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let mut record = super::prepare_ask_record(&store, "Keep the answer", None)
                .await
                .unwrap();
            record.session_run_id = Some(RunId::new()); // Native history is absent.
            record.ready_summary = Some("Accepted direction".into());
            record.retain_completed = true;
            write_ask_record(&record).unwrap();
            let lock = super::lock_session_launch(&record.id).unwrap();
            let mut reopening = Box::pin(super::open_boundary(&store, &record.id));
            let progress = std::future::poll_fn(|cx| {
                std::task::Poll::Ready(std::future::Future::poll(reopening.as_mut(), cx))
            })
            .await;
            assert!(progress.is_pending());
            let untouched = read_ask_record(&record.id).unwrap().unwrap();
            assert_eq!(untouched.session_run_id, record.session_run_id);
            assert_eq!(untouched.ready_summary, record.ready_summary);

            record.status = AskSessionStatus::Completed {
                summary: "Accepted direction".into(),
            };
            write_ask_record(&record).unwrap();
            drop(lock);

            assert!(reopening
                .await
                .unwrap_err()
                .to_string()
                .contains("already complete"));
            assert_eq!(
                wait_for_ask(&record.id).await.unwrap(),
                "Accepted direction"
            );
            assert!(ASK_LAUNCHERS.lock().unwrap().is_empty());
        });
    }

    #[test]
    fn ask_completion_survives_native_cleanup_failure() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let mut record = super::prepare_ask_record(&store, "Resolve stalled progress", None)
                .await
                .unwrap();
            let run_id = RunId::new();
            let run_dir = home
                .home
                .path()
                .join("runs")
                .join("broken")
                .join(run_id.as_str());
            std::fs::create_dir_all(&run_dir).unwrap();
            std::fs::write(run_dir.join("manifest.json"), b"invalid manifest").unwrap();
            record.session_run_id = Some(run_id);
            record.ready_summary = Some("Try the narrower proof".into());
            record.retain_completed = true;
            write_ask_record(&record).unwrap();

            complete_ask(&record.id).await.unwrap();

            let saved = read_ask_record(&record.id).unwrap().unwrap();
            assert!(matches!(saved.status, AskSessionStatus::Completed { .. }));
            assert_eq!(
                wait_for_ask(&record.id).await.unwrap(),
                "Try the narrower proof"
            );
            assert!(list_ask_sessions().await.unwrap().is_empty());
            assert_eq!(
                std::fs::read(run_dir.join("manifest.json")).unwrap(),
                b"invalid manifest"
            );
        });
    }

    #[test]
    fn ordinary_and_legacy_prompt_only_asks_still_remove_completed_records() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let human = async {
                let sessions = wait_until_asks(1).await;
                let id = &sessions[0].id;
                let record = read_ask_record(id).unwrap().unwrap();
                let mut legacy = serde_json::to_value(&record).unwrap();
                legacy.as_object_mut().unwrap().remove("skill");
                legacy.as_object_mut().unwrap().remove("retain_completed");
                std::fs::write(ask_record_path(id), serde_json::to_vec(&legacy).unwrap()).unwrap();
                let reopened = read_ask_record(id).unwrap().unwrap();
                assert!(reopened.skill.is_none());
                assert!(!reopened.retain_completed);
                finish_simulated_ask(id, "Ordinary answer").await;
                id.clone()
            };
            let (result, id) = tokio::join!(ask(&store, "Plain question", None), human);
            assert_eq!(result.unwrap(), "Ordinary answer");
            assert!(read_ask_record(&id).unwrap().is_none());
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
                super::SessionFlowMembership::Step { current: true, .. } => "current",
                super::SessionFlowMembership::Step { current: false, .. } => "historical",
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
            ["current", "historical", "independent", "unknown", "current"]
        );
        assert_eq!(
            sessions.last().unwrap().title_source,
            super::SessionTitleSource::Unavailable
        );
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

    #[test]
    fn ready_flow_without_an_autonomous_predecessor_cannot_iterate() {
        let mut position = position();
        position.ready_summary = Some("Reviewed".to_string());
        position.invocation.steps.remove(0);
        position.step_index = 0;
        let actions = super::flow_actions(&position);
        let reason = actions
            .iter()
            .find(|action| action.kind == super::SessionActionKind::Iterate)
            .unwrap()
            .unavailable_reason
            .as_deref()
            .unwrap();
        assert!(reason.contains("no preceding autonomous skill"));
        assert_eq!(
            super::require_flow_decision(&position, super::FlowDecision::Iterate)
                .unwrap_err()
                .to_string(),
            reason
        );
        assert!(super::require_flow_decision(&position, super::FlowDecision::Approve).is_ok());
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
    fn ask_skill_and_question_survive_reopening_and_reach_the_launch_prompt() {
        let _lock = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let checkout = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(checkout.path().join(".lf/skills")).unwrap();
        std::fs::write(
            checkout.path().join(".lf/skills/unblock.md"),
            "Resolve the blocker with the human using the preserved evidence.",
        )
        .unwrap();
        let record = AskSessionRecord {
            id: "ask_skill_proof".to_string(),
            parent_run_id: RunId::new(),
            parent_run_dir: home.path().join("parent"),
            work: None,
            work_selector: None,
            title: "Choose the delivery policy".to_string(),
            detail: "concept-review".to_string(),
            prompt: "Choose the delivery policy".to_string(),
            skill: Some("unblock".to_string()),
            cwd: checkout.path().to_path_buf(),
            model: "claude:opus".to_string(),
            session_run_id: None,
            ready_summary: None,
            status: AskSessionStatus::Waiting,
            retain_completed: false,
        };

        let previous_home = std::env::var_os("LF_HOME");
        std::env::set_var("LF_HOME", home.path());
        assert!(ask_record_path(&record.id).starts_with(home.path()));
        write_ask_record(&record).unwrap();
        let mut reopened = read_ask_record(&record.id).unwrap().unwrap();
        reopened.ready_summary = Some("Discussed the policy".to_string());
        write_ask_record(&reopened).unwrap();
        let reopened = read_ask_record(&record.id).unwrap().unwrap();

        // Existing records omit skill entirely; they still launch an inline Ask.
        let mut legacy = serde_json::to_value(&record).unwrap();
        legacy.as_object_mut().unwrap().remove("skill");
        std::fs::write(
            ask_record_path(&record.id),
            serde_json::to_vec(&legacy).unwrap(),
        )
        .unwrap();
        let legacy = read_ask_record(&record.id).unwrap().unwrap();
        match previous_home {
            Some(value) => std::env::set_var("LF_HOME", value),
            None => std::env::remove_var("LF_HOME"),
        }

        assert_eq!(reopened.skill.as_deref(), Some("unblock"));
        assert_eq!(reopened.cwd, checkout.path());
        let args = ask_launch_args(&reopened);
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
                repo_root: reopened.cwd.clone(),
                cwd: Some(reopened.cwd),
                skill: Some(name),
                message: Some(args.join(" ")),
                agent: Some(reopened.model),
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

        assert!(legacy.skill.is_none());
        let args = ask_launch_args(&legacy);
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
