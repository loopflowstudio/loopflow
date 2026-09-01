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

use crate::durable::{FlowPosition, ProjectId, RunId, WorkRef, WorkStatus};
use crate::id::WaveId;
use crate::run_record::{ProviderSessionRef, RunManifest, RUN_DIR_ENV};
use crate::store::SharedStore;
use crate::work::task::{Task, TaskId};

pub(crate) const HUMAN_SESSION_ENV: &str = "LF_HUMAN_SESSION";
pub(crate) const RUN_BIND_PATH_ENV: &str = "LF_HUMAN_SESSION_RUN_BIND";

const ASK_SESSION_DIRECTORY: &str = "human-sessions";
const SESSION_START_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FlowSessionToken {
    pub(crate) task_id: TaskId,
    pub(crate) flow: String,
    pub(crate) node_id: String,
    pub(crate) skill: String,
    pub(crate) iteration: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum HumanSessionToken {
    Flow { token: FlowSessionToken },
    Ask { id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowDecision {
    Approve,
    Iterate,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionRecord {
    pub id: String,
    pub kind: SessionKind,
    pub work: Option<WorkRef>,
    pub title: String,
    pub detail: String,
    pub cwd: String,
    pub state: SessionState,
    pub ready_summary: Option<String>,
    pub open_argv: Vec<String>,
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
    cwd: PathBuf,
    model: String,
    session_run_id: Option<RunId>,
    ready_summary: Option<String>,
    status: AskSessionStatus,
}

#[derive(Debug)]
enum SessionTarget {
    Interactive {
        dir: PathBuf,
        manifest: RunManifest,
        provider_session: ProviderSessionRef,
    },
    Ask(AskSessionRecord),
    Flow {
        task: Task,
        position: FlowPosition,
    },
}

pub(crate) async fn ask(store: &SharedStore, question: &str) -> Result<String> {
    let question = question.trim();
    if question.is_empty() {
        bail!("question cannot be empty");
    }
    let manifest = active_run_manifest()?;
    let cwd = std::env::current_dir().context("resolve Ask working directory")?;
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
    let record = AskSessionRecord {
        id: format!("ask_{}", uuid::Uuid::new_v4().simple()),
        parent_run_id: manifest.run_id,
        parent_run_dir: PathBuf::from(
            std::env::var_os(RUN_DIR_ENV).expect("active Run manifest requires LF_RUN_DIR"),
        ),
        work,
        work_selector,
        title: question_title(question),
        detail: manifest.skill.unwrap_or_else(|| "Human ask".to_string()),
        prompt: question.to_string(),
        cwd,
        model,
        session_run_id: None,
        ready_summary: None,
        status: AskSessionStatus::Waiting,
    };
    write_ask_record(&record)?;
    if let Err(error) = launch_ask(&record).await {
        let _ = fs::remove_file(ask_record_path(&record.id));
        return Err(error);
    }
    eprintln!(
        "Waiting for human session {}. Open it in Loopflow or with `lf session open {}`.",
        record.id, record.id
    );
    wait_for_ask(&record.id).await
}

pub(crate) async fn prepare(
    store: &SharedStore,
    task: &Task,
    position: &FlowPosition,
) -> Result<SessionRecord> {
    validate_task_position(task, position)?;
    let surface = flow_surface(store, task, position).await?;
    if position.session_run_id.is_some() {
        return Ok(surface);
    }
    let placement = store.placement(&position.work).await?;
    let home = store
        .home_by_id(&placement.home_id)
        .await?
        .ok_or_else(|| anyhow!("Task {} Home {} disappeared", task.id, placement.home_id))?;
    if home.route != "local" {
        bail!("human session starts on its placed Home; resume the Task there");
    }
    launch_flow(task, position).await?;
    flow_surface(store, task, position).await
}

pub(crate) async fn list(store: &SharedStore) -> Result<Vec<SessionRecord>> {
    let mut sessions = list_flow_sessions(store).await?;
    sessions.extend(list_ask_sessions().await?);
    let boundary_runs = boundary_run_ids(store).await?;
    sessions.extend(list_interactive_sessions(&boundary_runs)?);
    sessions.sort_by(|left, right| left.title.cmp(&right.title).then(left.id.cmp(&right.id)));
    Ok(sessions)
}

async fn find_session(store: &SharedStore, session_id: &str) -> Result<Option<SessionTarget>> {
    let home = crate::store::observability_home_dir();
    match crate::run_record::resolve_manifest(&home, session_id) {
        Ok((dir, manifest)) => {
            if manifest.surface != "tui" {
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
                manifest,
                provider_session,
            }));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(anyhow!("Session record unavailable: {error}")),
    }

    if let Some(record) = read_ask_record(session_id)? {
        if !matches!(record.status, AskSessionStatus::Waiting) {
            bail!("human session {:?} is already resolved", record.id);
        }
        return Ok(Some(SessionTarget::Ask(record)));
    }
    let Some((task, position)) = find_flow_session_optional(store, session_id).await? else {
        return Ok(None);
    };
    Ok(Some(SessionTarget::Flow { task, position }))
}

async fn session_surface(store: &SharedStore, target: &SessionTarget) -> Result<SessionRecord> {
    match target {
        SessionTarget::Interactive { dir, manifest, .. } => interactive_surface(dir, manifest),
        SessionTarget::Ask(record) => ask_surface(record),
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
        HumanSessionToken::Flow { token } => {
            let work = WorkRef::Task(token.task_id.clone());
            let mut position = store
                .flow_position(&work)
                .await?
                .ok_or_else(|| anyhow!("human flow session is no longer waiting"))?;
            if !token_matches(&token, &position)
                || position.session_run_id.as_ref() != Some(&run_id)
            {
                bail!("human flow session is stale");
            }
            position.ready_summary = Some(summary.to_string());
            position.updated_at = time::OffsetDateTime::now_utc();
            store.set_flow_position(&work, position).await?;
        }
        HumanSessionToken::Ask { id } => {
            let mut record = read_ask_record(&id)?
                .ok_or_else(|| anyhow!("human Ask session {id:?} no longer exists"))?;
            if record.session_run_id.as_ref() != Some(&run_id) {
                bail!("human Ask session is stale");
            }
            record.ready_summary = Some(summary.to_string());
            write_ask_record(&record)?;
        }
    }
    Ok(())
}

pub(crate) async fn decide(
    store: &SharedStore,
    session_id: &str,
    decision: FlowDecision,
    text: &str,
) -> Result<()> {
    let text = text.trim();
    if text.is_empty() {
        bail!("FlowStep decision cannot be empty");
    }
    let target = find_session(store, session_id)
        .await?
        .ok_or_else(|| session_not_found(session_id))?;
    let (task, position) = match target {
        SessionTarget::Interactive { .. } => {
            bail!("Interactive Sessions use `lf session complete`")
        }
        SessionTarget::Ask(_) => bail!("Ad-hoc Ask Sessions use `lf session complete`"),
        SessionTarget::Flow { task, position } => (task, position),
    };
    let token = flow_token(&task, &position)?;
    stop_flow_run(store, &task, &position).await;
    crate::controller::task::decide_human_flow_step(store, &token, decision, text).await?;
    let mut task = store.get_task(&token.task_id).await?.ok_or_else(|| {
        anyhow!(
            "Task {} disappeared after its FlowStep decision",
            token.task_id
        )
    })?;
    crate::ops::task::relaunch_inactive_process(store, &mut task)
        .await
        .map_err(|error| anyhow!(error.to_string()))
}

async fn stop_flow_run(store: &SharedStore, task: &Task, position: &FlowPosition) {
    let Some(run_id) = position.session_run_id.clone() else {
        return;
    };
    let result = async {
        let placement = store.placement(&position.work).await?;
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
        .context("join remote human Session stop")?
        .map(|_| ())
        .map_err(|error| anyhow!(error.to_string()))
    }
    .await;
    if let Err(error) = result {
        eprintln!("warning: FlowStep decided but its provider client could not stop: {error:#}");
    }
}

async fn complete_ask(session_id: &str) -> Result<()> {
    let Some(mut record) = read_ask_record(session_id)? else {
        return Err(session_not_found(session_id));
    };
    if !matches!(record.status, AskSessionStatus::Waiting) {
        bail!("Ask session {session_id:?} is already complete");
    }
    let summary = record.ready_summary.clone().ok_or_else(|| {
        anyhow!(
            "Ask session {session_id:?} is not ready; its agent must run `lf session ready \"<summary>\"` first"
        )
    })?;
    if let Some(run_id) = &record.session_run_id {
        stop_native_run(run_id)?;
    }
    record.status = AskSessionStatus::Completed { summary };
    write_ask_record(&record)?;
    Ok(())
}

pub(crate) async fn serve_flow(
    store: SharedStore,
    task_id: TaskId,
    flow: String,
    node_id: String,
    skill: String,
    iteration: u32,
) -> Result<()> {
    let token = FlowSessionToken {
        task_id,
        flow,
        node_id,
        skill,
        iteration,
    };
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
    let message = flow_message(&task, &token);
    let lf = crate::engine::process::resolve_current_home_lf_binary_checked()?;
    let selector = format!("task:{}", token.task_id);
    let serialized = serde_json::to_string(&HumanSessionToken::Flow {
        token: token.clone(),
    })?;
    let mut command = tokio::process::Command::new(lf);
    command
        .args(["--tui", "--as", &selector, &token.skill, &message])
        .current_dir(&task.worktree)
        .env(HUMAN_SESSION_ENV, serialized);
    let (mut child, run_id) = spawn_session_run(&mut command).await?;
    let work = WorkRef::Task(token.task_id.clone());
    let mut position = store
        .flow_position(&work)
        .await?
        .ok_or_else(|| anyhow!("human flow session is no longer waiting"))?;
    if !token_matches(&token, &position) {
        let _ = child.kill().await;
        bail!("human flow session is stale");
    }
    position.session_run_id = Some(run_id);
    position.ready_summary = None;
    position.updated_at = time::OffsetDateTime::now_utc();
    store.set_flow_position(&work, position).await?;
    drop(launch_lock);
    let status = child.wait().await.context("wait for human flow skill")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("human flow skill exited with {status}"))
    }
}

pub(crate) async fn serve_ask(id: &str) -> Result<()> {
    let launch_lock = lock_session_launch(id)?;
    serve_ask_locked(id, launch_lock).await
}

async fn serve_ask_locked(id: &str, launch_lock: File) -> Result<()> {
    let mut record =
        read_ask_record(id)?.ok_or_else(|| anyhow!("human Ask session {id:?} no longer exists"))?;
    if !matches!(record.status, AskSessionStatus::Waiting) {
        bail!("human Ask session {id:?} is already resolved");
    }
    let message = ask_message(&record);
    let lf = crate::engine::process::resolve_current_home_lf_binary_checked()?;
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
    args.extend([":".to_string(), message]);
    let mut command = tokio::process::Command::new(lf);
    command.args(args).current_dir(&record.cwd).env(
        HUMAN_SESSION_ENV,
        serde_json::to_string(&HumanSessionToken::Ask {
            id: record.id.clone(),
        })?,
    );
    let (mut child, run_id) = spawn_session_run(&mut command).await?;
    record.session_run_id = Some(run_id);
    record.ready_summary = None;
    write_ask_record(&record)?;
    drop(launch_lock);
    let status = child.wait().await.context("wait for human Ask agent")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("human Ask agent exited with {status}"))
    }
}

pub(crate) fn publish_run_binding(run_id: &RunId) -> Result<()> {
    let Some(path) = std::env::var_os(RUN_BIND_PATH_ENV).map(PathBuf::from) else {
        return Ok(());
    };
    std::env::remove_var(RUN_BIND_PATH_ENV);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&path)
        .context("publish human Session Run binding")?;
    file.write_all(run_id.as_str().as_bytes())
        .context("write human Session Run binding")?;
    file.sync_all().context("sync human Session Run binding")
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
        SessionTarget::Interactive {
            dir,
            manifest,
            provider_session,
        } => {
            let active_clients =
                crate::lf::commands::util::active_provider_clients(dir, &manifest.harness)?;
            match mode {
                OpenMode::Refuse if !active_clients.is_empty() => {
                    bail!(
                        "Session {} is active in another terminal. Close it first, use `--replace` to stop the Loopflow-owned client, or `--try` to let {} decide.",
                        manifest.run_id,
                        manifest.harness
                    );
                }
                OpenMode::Replace => crate::lf::commands::util::replace_provider_clients(
                    dir,
                    &manifest.harness,
                    &active_clients,
                )?,
                OpenMode::Refuse | OpenMode::Try => {}
            }
            let session = interactive_surface(dir, manifest)?;
            if resume {
                crate::lf::commands::util::resume_session(
                    &manifest.harness,
                    manifest.model.as_deref(),
                    &manifest.cwd,
                    &manifest.run_id,
                    dir,
                    provider_session,
                )?;
            }
            Ok(session)
        }
        SessionTarget::Ask(_) | SessionTarget::Flow { .. } => {
            if mode != OpenMode::Refuse {
                bail!("--replace and --try apply only to interactive provider sessions");
            }
            let session = session_surface(store, &target).await?;
            if resume {
                open_boundary(store, session_id).await?;
            }
            Ok(session)
        }
    }
}

pub(crate) async fn complete(store: &SharedStore, session_id: &str) -> Result<SessionRecord> {
    let target = find_session(store, session_id)
        .await?
        .ok_or_else(|| session_not_found(session_id))?;
    match &target {
        SessionTarget::Interactive { dir, manifest, .. } => {
            let session = interactive_surface(dir, manifest)?;
            let active_clients =
                crate::lf::commands::util::active_provider_clients(dir, &manifest.harness)?;
            crate::lf::commands::util::replace_provider_clients(
                dir,
                &manifest.harness,
                &active_clients,
            )?;
            crate::run_record::resolve_provider_session(dir)
                .map_err(|error| anyhow!("cannot complete Session {}: {error}", manifest.run_id))?;
            Ok(session)
        }
        SessionTarget::Ask(_) => {
            let session = session_surface(store, &target).await?;
            complete_ask(session_id).await?;
            Ok(session)
        }
        SessionTarget::Flow { .. } => {
            bail!("Task FlowStep Sessions use `lf session approve` or `lf session iterate`")
        }
    }
}

async fn open_boundary(store: &SharedStore, session_id: &str) -> Result<()> {
    if let Some(mut record) = read_ask_record(session_id)? {
        if !matches!(record.status, AskSessionStatus::Waiting) {
            bail!("human Ask session {session_id:?} is already complete");
        }
        if let Some(run_id) = &record.session_run_id {
            if resume_native_run(
                run_id,
                &HumanSessionToken::Ask {
                    id: record.id.clone(),
                },
            )? {
                return Ok(());
            }
            record.session_run_id = None;
            record.ready_summary = None;
            write_ask_record(&record)?;
        }
        let launch_lock = lock_session_launch(session_id)?;
        record = read_ask_record(session_id)?
            .ok_or_else(|| anyhow!("human Ask session {session_id:?} no longer exists"))?;
        if let Some(run_id) = &record.session_run_id {
            if resume_native_run(
                run_id,
                &HumanSessionToken::Ask {
                    id: record.id.clone(),
                },
            )? {
                return Ok(());
            }
            record.session_run_id = None;
            record.ready_summary = None;
            write_ask_record(&record)?;
        }
        return serve_ask_locked(session_id, launch_lock).await;
    }

    let (task, mut position) = find_flow_session(store, session_id).await?;
    let token = flow_token(&task, &position)?;
    if let Some(run_id) = &position.session_run_id {
        if resume_native_run(
            run_id,
            &HumanSessionToken::Flow {
                token: token.clone(),
            },
        )? {
            return Ok(());
        }
        position.session_run_id = None;
        position.ready_summary = None;
        position.updated_at = time::OffsetDateTime::now_utc();
        let work = position.work.clone();
        store.set_flow_position(&work, position).await?;
    }
    let launch_lock = lock_session_launch(session_id)?;
    let (_, current) = find_flow_session(store, session_id).await?;
    if let Some(run_id) = &current.session_run_id {
        if resume_native_run(
            run_id,
            &HumanSessionToken::Flow {
                token: token.clone(),
            },
        )? {
            return Ok(());
        }
    }
    serve_flow_locked(store.clone(), token, launch_lock).await
}

pub(crate) fn stop_run(run_id: &RunId) -> Result<()> {
    stop_native_run(run_id)
}

async fn boundary_run_ids(store: &SharedStore) -> Result<HashSet<RunId>> {
    let mut run_ids = store
        .human_flow_positions()
        .await?
        .into_iter()
        .filter_map(|position| position.session_run_id)
        .collect::<HashSet<_>>();
    let directory = ask_session_directory();
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(run_ids),
        Err(error) => return Err(error).context("read human session directory"),
    };
    for entry in entries {
        let entry = entry.context("read human session entry")?;
        let Some(id) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
            .map(str::to_string)
        else {
            continue;
        };
        if let Some(run_id) = read_ask_record(&id)?.and_then(|record| record.session_run_id) {
            run_ids.insert(run_id);
        }
    }
    Ok(run_ids)
}

fn list_interactive_sessions(human_runs: &HashSet<RunId>) -> Result<Vec<SessionRecord>> {
    let home = crate::store::observability_home_dir();
    let runs = crate::run_record::scan_unresolved_provider_runs(&home)
        .map_err(|error| anyhow!("Session records unavailable: {error}"))?;
    let mut sessions = Vec::new();
    for (dir, manifest) in runs {
        if human_runs.contains(&manifest.run_id) {
            continue;
        }
        sessions.push(interactive_surface(&dir, &manifest)?);
    }
    Ok(sessions)
}

fn interactive_surface(dir: &Path, manifest: &RunManifest) -> Result<SessionRecord> {
    let state =
        if crate::lf::commands::util::active_provider_clients(dir, &manifest.harness)?.is_empty() {
            SessionState::Closed
        } else {
            SessionState::Active
        };
    let lf = crate::engine::process::resolve_current_home_lf_binary_checked()?;
    Ok(SessionRecord {
        id: manifest.run_id.to_string(),
        kind: SessionKind::Interactive,
        work: attributed_work(manifest),
        title: session_title(dir, manifest),
        detail: match &manifest.model {
            Some(model) => format!("{}:{model}", manifest.harness),
            None => manifest.harness.clone(),
        },
        cwd: manifest.cwd.display().to_string(),
        state,
        ready_summary: None,
        open_argv: vec![
            lf.display().to_string(),
            "session".to_string(),
            "open".to_string(),
            manifest.run_id.to_string(),
        ],
    })
}

fn attributed_work(manifest: &RunManifest) -> Option<WorkRef> {
    for prefix in ["task:", "project:", "wave:"] {
        let Some(id) = manifest
            .subjects
            .iter()
            .find_map(|subject| subject.selector.strip_prefix(prefix))
        else {
            continue;
        };
        let work = match prefix {
            "task:" => TaskId::parse(id).ok().map(WorkRef::Task),
            "project:" => ProjectId::parse(id).ok().map(WorkRef::Project),
            "wave:" => WaveId::parse(id).ok().map(WorkRef::Wave),
            _ => None,
        };
        if work.is_some() {
            return work;
        }
    }
    None
}

fn session_title(dir: &Path, manifest: &RunManifest) -> String {
    let context = fs::read(dir.join("context.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let task = context
        .as_ref()
        .and_then(|value| value.pointer("/context/task"));
    let text = task
        .and_then(|task| task.get("text"))
        .and_then(serde_json::Value::as_str);
    let user_message = task
        .and_then(|task| task.get("assets"))
        .and_then(serde_json::Value::as_array)
        .and_then(|assets| {
            assets.iter().rev().find(|asset| {
                asset.get("kind").and_then(serde_json::Value::as_str) == Some("user_message")
            })
        })
        .and_then(|asset| {
            let start = asset.get("byte_start")?.as_u64()? as usize;
            let end = asset.get("byte_end")?.as_u64()? as usize;
            text?.get(start..end)
        });
    concise_title(user_message.or(text))
        .or_else(|| manifest.skill.clone())
        .unwrap_or_else(|| manifest.harness.clone())
}

fn concise_title(text: Option<&str>) -> Option<String> {
    let title = text?.lines().map(str::trim).find(|line| !line.is_empty())?;
    let mut chars = title.chars();
    let truncated = chars.by_ref().take(80).collect::<String>();
    Some(if chars.next().is_some() {
        format!("{}…", truncated.trim_end())
    } else {
        truncated
    })
}

fn session_not_found(id: &str) -> anyhow::Error {
    anyhow!("Session {id} was not found")
}

async fn spawn_session_run(
    command: &mut tokio::process::Command,
) -> Result<(tokio::process::Child, RunId)> {
    let directory = ask_session_directory();
    fs::create_dir_all(&directory).context("create human Session directory")?;
    let binding = directory.join(format!(".run-bind-{}", uuid::Uuid::new_v4().simple()));
    let mut child = command
        .env(RUN_BIND_PATH_ENV, &binding)
        .kill_on_drop(true)
        .spawn()
        .context("launch human Session Run")?;
    let deadline = tokio::time::Instant::now() + SESSION_START_TIMEOUT;
    let mut run = None;
    loop {
        if run.is_none() {
            match fs::read_to_string(&binding) {
                Ok(value) => {
                    let run_id = RunId::parse(value.trim())?;
                    let home = crate::store::observability_home_dir();
                    let (dir, manifest) =
                        crate::run_record::resolve_manifest(&home, run_id.as_str())
                            .context("resolve published human Session Run")?;
                    run = Some((run_id, dir, manifest));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("read human Session Run binding"),
            }
        }
        if let Some((run_id, dir, manifest)) = &run {
            if session_run_is_resumable(dir, manifest)? {
                let _ = fs::remove_file(&binding);
                return Ok((child, run_id.clone()));
            }
        }
        if let Some(status) = child.try_wait().context("probe human Session Run")? {
            let _ = fs::remove_file(&binding);
            bail!("human Session Run exited with {status} before becoming resumable");
        }
        if tokio::time::Instant::now() >= deadline {
            let _ = fs::remove_file(&binding);
            bail!("human Session Run did not become resumable within 30s");
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

fn resume_native_run(run_id: &RunId, token: &HumanSessionToken) -> Result<bool> {
    let home = crate::store::observability_home_dir();
    let (dir, manifest) = match crate::run_record::resolve_manifest(&home, run_id.as_str()) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).context("resolve human Session Run"),
    };
    let clients = crate::lf::commands::util::active_provider_clients(&dir, &manifest.harness)?;
    crate::lf::commands::util::replace_provider_clients(&dir, &manifest.harness, &clients)?;
    let Some(provider_session) = crate::run_record::read_provider_session(&dir)? else {
        return Ok(false);
    };
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
    )?;
    Ok(true)
}

fn stop_native_run(run_id: &RunId) -> Result<()> {
    let home = crate::store::observability_home_dir();
    let (dir, manifest) = match crate::run_record::resolve_manifest(&home, run_id.as_str()) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).context("resolve human Session Run"),
    };
    let clients = crate::lf::commands::util::active_provider_clients(&dir, &manifest.harness)?;
    crate::lf::commands::util::replace_provider_clients(&dir, &manifest.harness, &clients)?;
    if crate::run_record::read_provider_session(&dir)?.is_some() {
        crate::run_record::resolve_provider_session(&dir)?;
    }
    Ok(())
}

fn native_session_state(
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
        Err(error) => return Err(error).context("resolve human Session Run"),
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
    let work = WorkRef::Task(token.task_id.clone());
    Ok(store
        .flow_position(&work)
        .await?
        .as_ref()
        .is_some_and(|position| token_matches(token, position)))
}

async fn list_flow_sessions(store: &SharedStore) -> Result<Vec<SessionRecord>> {
    let mut sessions = Vec::new();
    for position in store.human_flow_positions().await? {
        let WorkRef::Task(task_id) = &position.work else {
            continue;
        };
        let Some(task) = store.get_task(task_id).await? else {
            continue;
        };
        if store.work_status(&position.work).await? != WorkStatus::Ready {
            continue;
        }
        sessions.push(flow_surface(store, &task, &position).await?);
    }
    Ok(sessions)
}

async fn list_ask_sessions() -> Result<Vec<SessionRecord>> {
    let directory = ask_session_directory();
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).context("read human session directory"),
    };
    let mut sessions = Vec::new();
    for entry in entries {
        let entry = entry.context("read human session entry")?;
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
            sessions.push(ask_surface(&record)?);
        }
    }
    Ok(sessions)
}

async fn find_flow_session(store: &SharedStore, session_id: &str) -> Result<(Task, FlowPosition)> {
    find_flow_session_optional(store, session_id)
        .await?
        .ok_or_else(|| anyhow!("human session {session_id:?} is no longer waiting"))
}

async fn find_flow_session_optional(
    store: &SharedStore,
    session_id: &str,
) -> Result<Option<(Task, FlowPosition)>> {
    for position in store.human_flow_positions().await? {
        if flow_id(&position)? != session_id {
            continue;
        }
        let WorkRef::Task(task_id) = &position.work else {
            break;
        };
        let task = store
            .get_task(task_id)
            .await?
            .ok_or_else(|| anyhow!("Task {task_id} disappeared"))?;
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
    let placement = store.placement(&position.work).await?;
    let home = store
        .home_by_id(&placement.home_id)
        .await?
        .ok_or_else(|| anyhow!("Task {} Home {} disappeared", task.id, placement.home_id))?;
    let id = flow_id(position)?;
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
        id,
        kind: SessionKind::Flow,
        work: Some(position.work.clone()),
        title: task.plan.title.clone(),
        detail: position.step.clone(),
        cwd: task.worktree.display().to_string(),
        state: runtime,
        ready_summary: position.ready_summary.clone(),
        open_argv,
    })
}

fn ask_surface(record: &AskSessionRecord) -> Result<SessionRecord> {
    let open_argv = human_open_argv(None, None, &record.id)?;
    let runtime = native_session_state(
        record.session_run_id.as_ref(),
        record.ready_summary.as_deref(),
    )?;
    Ok(SessionRecord {
        id: record.id.clone(),
        kind: SessionKind::Ask,
        work: record.work.clone(),
        title: record.title.clone(),
        detail: record.detail.clone(),
        cwd: record.cwd.display().to_string(),
        state: runtime,
        ready_summary: record.ready_summary.clone(),
        open_argv,
    })
}

fn human_open_argv(
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
    if position.work != WorkRef::Task(task.id.clone()) || !position.human {
        return Err(anyhow!("human session does not belong to Task {}", task.id));
    }
    let node_id = position
        .node_id
        .as_deref()
        .ok_or_else(|| anyhow!("human flow position has no node id"))?;
    if node_id.trim().is_empty() {
        return Err(anyhow!("human flow position has an empty node id"));
    }
    Ok(())
}

async fn validate_token(store: &SharedStore, token: &FlowSessionToken) -> Result<()> {
    if token_is_current(store, token).await? {
        Ok(())
    } else {
        Err(anyhow!("human flow session is stale"))
    }
}

fn flow_token(task: &Task, position: &FlowPosition) -> Result<FlowSessionToken> {
    validate_task_position(task, position)?;
    Ok(FlowSessionToken {
        task_id: task.id.clone(),
        flow: position.flow.clone(),
        node_id: position
            .node_id
            .clone()
            .expect("validated human position has a node id"),
        skill: position.step.clone(),
        iteration: position.iteration,
    })
}

fn token_matches(token: &FlowSessionToken, position: &FlowPosition) -> bool {
    position.work == WorkRef::Task(token.task_id.clone())
        && position.human
        && position.flow == token.flow
        && position.node_id.as_deref() == Some(token.node_id.as_str())
        && position.step == token.skill
        && position.iteration == token.iteration
}

fn flow_message(task: &Task, token: &FlowSessionToken) -> String {
    format!(
        "<lf:human-session>\nThis `{skill}` Run is the writable human session for Task {identifier} at `{node}`. Work with the human in this terminal. When your work is ready for their decision, run `lf session ready \"<concise summary>\"`. Ready does not approve, iterate, close, or advance the Task; only the human can approve or iterate on the session.\n</lf:human-session>",
        skill = token.skill,
        identifier = task.plan.identifier,
        node = token.node_id,
    )
}

fn ask_message(record: &AskSessionRecord) -> String {
    format!(
        "{}\n\n<lf:human-session>\nThe originating Loopflow Run is blocked while you work with the human in this terminal. You are in the caller's checkout and may inspect or edit it. When the work is ready, run `lf session ready \"<concise summary>\"`. Ready keeps this session visible and does not resume the caller; the human completes it when the conversation is finished.\n</lf:human-session>",
        record.prompt
    )
}

async fn launch_flow(task: &Task, position: &FlowPosition) -> Result<()> {
    let node_id = position
        .node_id
        .as_deref()
        .ok_or_else(|| anyhow!("human flow position has no node id"))?;
    let lf = crate::engine::process::resolve_current_home_lf_binary();
    let argv = vec![
        lf.to_string_lossy().to_string(),
        "session".to_string(),
        "serve-flow".to_string(),
        task.id.to_string(),
        position.flow.clone(),
        node_id.to_string(),
        position.step.clone(),
        position.iteration.to_string(),
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
    _name: &str,
    _cwd: &Path,
    _argv: &[String],
    _env: &[(&str, &str)],
) -> Result<()> {
    Ok(())
}

fn flow_id(position: &FlowPosition) -> Result<String> {
    let node_id = position
        .node_id
        .as_deref()
        .ok_or_else(|| anyhow!("human flow position has no node id"))?;
    Ok(format!(
        "{}:{}:{}:{}",
        position.work.id(),
        position.flow,
        node_id,
        position.iteration
    ))
}

fn flow_token_id(token: &FlowSessionToken) -> String {
    format!(
        "{}:{}:{}:{}",
        token.task_id, token.flow, token.node_id, token.iteration
    )
}

fn lock_session_launch(id: &str) -> Result<File> {
    let directory = ask_session_directory();
    fs::create_dir_all(&directory).context("create human Session directory")?;
    let name = hex::encode(&Sha256::digest(id.as_bytes())[..16]);
    let path = directory.join(format!(".{name}.launch.lock"));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .context("open human Session launch lock")?;
    FileExt::lock_exclusive(&file).context("lock human Session launch")?;
    Ok(file)
}

fn flow_background_name(position: &FlowPosition) -> Result<String> {
    Ok(flow_token_background_name(&FlowSessionToken {
        task_id: match &position.work {
            WorkRef::Task(task_id) => task_id.clone(),
            _ => return Err(anyhow!("human flow position is not Task Work")),
        },
        flow: position.flow.clone(),
        node_id: position
            .node_id
            .clone()
            .ok_or_else(|| anyhow!("human flow position has no node id"))?,
        skill: position.step.clone(),
        iteration: position.iteration,
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
    format!("lf-human-{task}-{node}-{}", token.iteration)
}

fn ask_background_name(id: &str) -> String {
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

fn preferred_work_selector(manifest: &RunManifest) -> Option<String> {
    manifest
        .subjects
        .iter()
        .filter_map(|subject| {
            let rank = if subject.selector.starts_with("task:") {
                3
            } else if subject.selector.starts_with("project:") {
                2
            } else if subject.selector.starts_with("wave:") {
                1
            } else {
                return None;
            };
            Some((rank, subject.selector.clone()))
        })
        .max_by_key(|(rank, _)| *rank)
        .map(|(_, selector)| selector)
}

fn question_title(question: &str) -> String {
    let first = question
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("Human ask");
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
    fs::create_dir_all(&directory).context("create human session directory")?;
    #[cfg(unix)]
    fs::set_permissions(
        &directory,
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .context("protect human session directory")?;
    let path = ask_record_path(&record.id);
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
    let bytes = serde_json::to_vec_pretty(record)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&temporary)
        .context("stage human session record")?;
    file.write_all(&bytes)
        .context("write human session record")?;
    file.sync_all().context("sync human session record")?;
    fs::rename(&temporary, &path).context("publish human session record")
}

fn read_ask_record(id: &str) -> Result<Option<AskSessionRecord>> {
    let path = ask_record_path(id);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("read human session record"),
    };
    let record: AskSessionRecord =
        serde_json::from_slice(&bytes).context("parse human session record")?;
    if record.id != id {
        bail!("human session record {id:?} is invalid");
    }
    Ok(Some(record))
}

async fn wait_for_ask(id: &str) -> Result<String> {
    loop {
        let record = read_ask_record(id)?
            .ok_or_else(|| anyhow!("human Ask session {id:?} disappeared before resolution"))?;
        match record.status {
            AskSessionStatus::Waiting => tokio::time::sleep(Duration::from_millis(250)).await,
            AskSessionStatus::Completed { summary } => {
                fs::remove_file(ask_record_path(id)).context("remove resolved human session")?;
                return Ok(summary);
            }
        }
    }
}

fn active_session_token() -> Result<HumanSessionToken> {
    let raw = std::env::var(HUMAN_SESSION_ENV)
        .context("this command requires an active human session")?;
    serde_json::from_str(&raw).context("active human session token is invalid")
}

fn active_run_id() -> Result<RunId> {
    let value = std::env::var(crate::durable::RUN_ID_ENV)
        .context("this command requires an active Loopflow Run")?;
    RunId::parse(&value).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::{
        concise_title, flow_background_name, flow_id, flow_token_id, human_open_argv,
        preferred_work_selector, question_title, session_run_is_resumable, token_matches,
        FlowSessionToken,
    };
    use crate::durable::{FlowPosition, WorkRef};
    use crate::run_record::{AttributionSource, RunManifest, SubjectAttribution};
    use crate::work::task::TaskId;

    fn position() -> FlowPosition {
        FlowPosition {
            work: WorkRef::Task(TaskId::new()),
            flow: "review".to_string(),
            step: "review-design".to_string(),
            node_id: Some("review_kickoff".to_string()),
            human: true,
            session_run_id: None,
            ready_summary: None,
            step_index: 1,
            iteration: 3,
            updated_at: time::OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn session_identity_is_the_exact_human_flow_position() {
        let position = position();
        let token = FlowSessionToken {
            task_id: match &position.work {
                WorkRef::Task(task_id) => task_id.clone(),
                _ => unreachable!(),
            },
            flow: position.flow.clone(),
            node_id: position.node_id.clone().unwrap(),
            skill: position.step.clone(),
            iteration: position.iteration,
        };

        assert!(token_matches(&token, &position));
        assert!(flow_id(&position).unwrap().contains("review_kickoff"));
        assert_eq!(flow_id(&position).unwrap(), flow_token_id(&token));
        assert!(flow_background_name(&position)
            .unwrap()
            .starts_with("lf-human-"));
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
    fn interactive_titles_use_one_bounded_line() {
        assert_eq!(
            concise_title(Some("\n  Fix session restore\nmore")),
            Some("Fix session restore".to_string())
        );
        assert_eq!(
            concise_title(Some(&"x".repeat(81))),
            Some(format!("{}…", "x".repeat(80)))
        );
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
