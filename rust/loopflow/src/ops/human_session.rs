mod connection;
pub(crate) use connection::explain_connect;
pub use connection::{SessionConnectAction, SessionConnectExplanation, SessionConnectIntent};

mod workspace;
pub use workspace::SessionWorkspace;

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::durable::WorkRef;
use crate::session::{LfSession, WorkSource};
use crate::session_record::SessionTitleSource;
use crate::store::sqlite::SqliteStore;
use crate::store::SharedStore;
use crate::work::task::Task;

#[cfg(test)]
pub(crate) mod action_test;
pub(crate) mod conversation_log;
pub(crate) mod primary;
pub(crate) use primary::ensure_scope_worktree;
pub(crate) mod provider_conversation;

/// Choose by native human input, falling back to each Session's last opening.
pub async fn latest_interactive_session(
    store: &SharedStore,
    cwd: &Path,
) -> Result<Option<LfSession>> {
    fn checkout(path: &Path) -> Option<PathBuf> {
        let path = fs::canonicalize(path).ok()?;
        let root = crate::repo::discover_repo_root(&path).ok()?.unwrap_or(path);
        fs::canonicalize(root).ok()
    }
    let Some(current) = checkout(cwd) else {
        return Ok(None);
    };
    let mut roots = BTreeMap::new();
    let candidates = store
        .resume_candidates()
        .await?
        .into_iter()
        .filter(|(session, _)| {
            roots
                .entry(session.cwd.clone())
                .or_insert_with(|| checkout(&session.cwd))
                .as_ref()
                == Some(&current)
        })
        .collect::<Vec<_>>();
    most_recent(store, candidates).await
}

/// Rank by native human input, then each conversation's last interactive
/// opening, then its creation. Assistant output never changes the choice.
async fn most_recent(
    store: &SharedStore,
    candidates: Vec<(LfSession, Option<i64>)>,
) -> Result<Option<LfSession>> {
    let human = provider_conversation::human_input_times(
        store,
        candidates.iter().map(|(session, _)| session),
    )
    .await?;
    Ok(candidates
        .into_iter()
        .max_by_key(|(session, opened)| {
            (
                human
                    .get(&session.id)
                    .copied()
                    .or(*opened)
                    .unwrap_or(session.created_at.saturating_mul(1000)),
                session.id.clone(),
            )
        })
        .map(|(session, _)| session))
}

pub(crate) const HUMAN_SESSION_ENV: &str = "LF_HUMAN_SESSION";
pub(crate) const PREPARED_CAPTURE_ENV: &str = "LF_PREPARED_CAPTURE";
/// Launch exclusion is process state beside the Machine, not a Session record.
const LAUNCH_LOCK_DIRECTORY: &str = "human-sessions";
const SESSION_START_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum HumanSessionToken {
    /// A conversation; no caller or Flow waits on it.
    Primary { id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenMode {
    Refuse,
    Replace,
    Try,
}

impl OpenMode {
    pub(crate) fn from_flags(replace: bool, try_open: bool) -> Self {
        if replace {
            Self::Replace
        } else if try_open {
            Self::Try
        } else {
            Self::Refuse
        }
    }

    fn require_action(self, state: SessionState) -> Result<()> {
        if self == Self::Refuse {
            require_session_action(state, SessionActionKind::Open)?;
        }
        Ok(())
    }
}

/// The ordinary resume command and its preview choose the same conversation.
pub(crate) async fn select_connection(
    store: &SharedStore,
    cwd: &Path,
    id: Option<&str>,
) -> Result<String> {
    match id {
        Some(id) => Ok(id.to_owned()),
        None => latest_interactive_session(store, cwd)
            .await?
            .map(|session| session.id)
            .context("No interactive session found in this worktree"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Passive metadata has no trustworthy live/closed observation.
    Unknown,
    Active,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionActionKind {
    Open,
    MoveHere,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionAction {
    pub kind: SessionActionKind,
    pub label: String,
    pub help: String,
    pub unavailable_reason: Option<String>,
}

pub(crate) fn session_actions(state: SessionState) -> Vec<SessionAction> {
    use SessionActionKind::{MoveHere, Open};
    let active_client = state == SessionState::Active;
    let mut actions = vec![(
        Open,
        "Open here",
        "Open this Session in a terminal",
        active_client
            .then_some("This Session is active in another terminal; use Move here to transfer it"),
    )];
    if active_client {
        actions.push((
            MoveHere,
            "Move here",
            "Transfer control here; unsent text in the previous terminal is not transferred",
            None,
        ));
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

fn require_session_action(state: SessionState, action: SessionActionKind) -> Result<()> {
    let projected = session_actions(state)
        .into_iter()
        .find(|item| item.kind == action)
        .ok_or_else(|| anyhow!("This action does not apply to this Session"))?;
    if let Some(reason) = projected.unavailable_reason {
        bail!("{reason}");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionPage {
    pub entries: Vec<SessionRecord>,
    pub next: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub primary_scope: Option<String>,
    /// Waiting on a person, judged from current stream or terminal reports; absent when it is
    /// working or nothing current says.
    pub attention: Option<SessionAttention>,
    pub program_status: Option<crate::program_status::Records>,
    /// The provider's current AgentProcess; absent when Loopflow never launched one.
    pub agent_process_id: Option<crate::id::LfProcessId>,
    /// Its Task names it as the Task's primary conversation.
    pub task_primary: bool,
    pub task_ids: Vec<crate::durable::TaskId>,
    pub id: String,
    pub workspace: Option<SessionWorkspace>,
    pub interactive: bool,
    pub work: Option<WorkRef>,
    pub wave_id: Option<crate::id::WaveId>,
    pub work_path: Option<String>,
    pub actions: Vec<SessionAction>,
    pub title: String,
    pub title_source: SessionTitleSource,
    pub flow_membership: SessionFlowMembership,
    pub detail: String,
    /// The provider its current input launched with; absent until a review's
    /// first launch records one.
    pub provider: Option<String>,
    pub cwd: String,
    pub state: SessionState,
    pub ready_summary: Option<String>,
    pub open_argv: Vec<String>,
    pub terminal_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionAttention {
    Waiting,
}

fn session_state(session: &crate::session::SessionSummary) -> SessionState {
    match session.agent_process_evidence {
        crate::journal::ProcessIdentityEvidence::Live => SessionState::Active,
        crate::journal::ProcessIdentityEvidence::Dead => SessionState::Closed,
        crate::journal::ProcessIdentityEvidence::Unknown => SessionState::Unknown,
    }
}

fn session_attention(session: &crate::session::SessionSummary) -> Option<SessionAttention> {
    session.waiting.then_some(SessionAttention::Waiting)
}

/// Whether a Session's conversation is an occurrence of a Flow. Membership
/// comes only from its recorded Flow invocation, never from matching Task, checkout,
/// provider, or skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionFlowMembership {
    Step {
        flow: String,
        flow_lf_process_id: String,
        step: String,
        /// Exact graph occurrence, unavailable for older capture manifests.
        node: Option<u32>,
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
    /// The row retains membership but no selected occurrence.
    Unknown,
    /// The invocation's current position.
    Current,
    /// An earlier position of the invocation that is still active.
    Earlier,
    /// An invocation that has finished.
    Past,
}

pub(crate) fn publish_prepared_input(
    store: &crate::store::sqlite::SqliteStore,
    session: &LfSession,
    flow: crate::session_record::SessionFlowMembership,
) -> Result<LfSession> {
    anyhow::ensure!(
        session.captured.is_some(),
        "Session input must be reserved before artifact publication"
    );
    crate::session_record::CaptureHandle::prepare(
        crate::session_record::SessionCaptureSpec {
            harness: session.provider.clone().unwrap_or_default(),
            model: session.model.clone(),
            surface: "tui".to_owned(),
            cwd: session.cwd.clone(),
            repo: None,
            worktree: Some(session.cwd.clone()),
            skill: session.skill.clone(),
            subjects: capture_subjects(session),
            flow,
            work: None,
        },
        session.caller_artifact_key.clone(),
        session.artifact_key.clone(),
    )?;
    store.publish_capture(&session.id, session.captured)?;
    store
        .session(&session.id)?
        .ok_or_else(|| session_not_found(&session.id))
}

pub(crate) async fn list(
    store: &SharedStore,
    filter: &crate::session::SessionFilter,
) -> Result<Vec<SessionRecord>> {
    // Checkout resolution must precede Task filtering and pagination: SQL path
    // prefixes cannot recognize symlink aliases or nested repositories.
    let association_filter = filter.orphan || filter.task.is_some();
    let mut selection = filter.clone();
    if association_filter {
        selection.task = None;
        selection.orphan = false;
        selection.limit = 0;
        selection.offset = 0;
    }
    let mut sessions = Vec::new();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    for session in store.session_summaries(&selection, now).await? {
        sessions.push(summary_surface(&session));
    }
    workspace::associate(store, &mut sessions).await?;
    if association_filter {
        let task = if let Some(selector) = &filter.task {
            store.sqlite.resolve_task_id(selector, None)?
        } else {
            None
        };
        sessions.retain(|session| {
            (!filter.orphan || session.task_ids.is_empty())
                && (filter.task.is_none()
                    || task
                        .as_ref()
                        .is_some_and(|task| session.task_ids.contains(task)))
        });
        let offset = if filter.after.is_some() {
            0
        } else {
            filter.offset
        };
        sessions = sessions
            .into_iter()
            .skip(offset)
            .take(if filter.limit == 0 {
                usize::MAX
            } else {
                filter.limit
            })
            .collect();
    }
    Ok(sessions)
}

/// Where a Session's step stands in its Flow: the last one a still-open Flow
/// process launched, an earlier one, or part of a Flow whose process has exited.
fn flow_occurrence(
    state: crate::session::FlowProcessSummaryState,
    latest_step: bool,
) -> SessionFlowOccurrence {
    match (state, latest_step) {
        (crate::session::FlowProcessSummaryState::Current, true) => SessionFlowOccurrence::Current,
        (crate::session::FlowProcessSummaryState::Current, false) => SessionFlowOccurrence::Earlier,
        _ => SessionFlowOccurrence::Past,
    }
}

/// Passive listing reads record metadata and exact local client receipts only.
/// Connect/complete still enter find_session and surface, with full validation.
fn summary_surface(session: &crate::session::SessionSummary) -> SessionRecord {
    let work = match (&session.task_id, &session.wave_id) {
        (Some(task), _) => Some(WorkRef::Task(task.clone())),
        (None, Some(wave)) => Some(WorkRef::Wave(wave.clone())),
        _ => None,
    };
    let work_path = session.wave_id.as_ref().map(|wave| {
        let wave = session
            .wave_name
            .clone()
            .unwrap_or_else(|| format!("Wave {wave} (unavailable)"));
        match &session.task_id {
            None => wave,
            Some(task) => match &session.task_identifier {
                Some(label) => format!("{wave} / {label}"),
                None => format!("Task {task} (unavailable)"),
            },
        }
    });
    let flow_membership = match (&session.flow_lf_process_id, &session.flow) {
        (None, _) if session.independent => SessionFlowMembership::Independent,
        (None, _) => SessionFlowMembership::Unknown {
            reason: "Flow membership was not recorded".into(),
        },
        (Some(id), Some(flow)) => SessionFlowMembership::Step {
            flow: flow.name.clone(),
            flow_lf_process_id: id.clone(),
            step: session.skill.clone().unwrap_or_default(),
            node: session.node,
            iterations: session.iterations.clone(),
            occurrence: flow_occurrence(flow.state, session.flow_step_latest),
        },
        (Some(id), _) => SessionFlowMembership::Unknown {
            reason: format!("Flow {id} metadata is unavailable"),
        },
    };
    let state = session_state(session);
    let mut actions = session_actions(state);
    let open_argv = match human_open_argv(&session.id) {
        Ok(argv) => argv,
        Err(error) => {
            let reason = format!("Session connection unavailable: {error}");
            for action in &mut actions {
                action.unavailable_reason = Some(reason.clone());
            }
            Vec::new()
        }
    };
    let provider = session.provider.clone().unwrap_or_default();
    SessionRecord {
        primary_scope: session.primary_scope.clone(),
        attention: session_attention(session),
        program_status: session.program_status.clone(),
        agent_process_id: session.agent_process_id.clone(),
        task_primary: session.task_primary,
        task_ids: session.task_ids.clone(),
        id: session.id.clone(),
        workspace: None,
        interactive: session.interactive,
        work,
        wave_id: session.wave_id.clone(),
        work_path,
        actions,
        title: session.title.clone(),
        title_source: match session.title_source {
            crate::session::TitleSource::Human => SessionTitleSource::Human,
            crate::session::TitleSource::Generated => SessionTitleSource::Generated,
        },
        flow_membership,
        detail: session
            .model
            .as_ref()
            .map_or(provider.clone(), |model| format!("{provider}:{model}")),
        provider: session.provider.clone(),
        cwd: session.cwd.display().to_string(),
        state,
        ready_summary: session.ready_summary.clone(),
        open_argv,
        terminal_ids: session.terminal_id.clone().into_iter().collect(),
    }
}

/// Resolve a durable Session or recorded native conversation, never a capture selector.
async fn find_session(
    store: &SharedStore,
    session_id: &str,
    open_completed: bool,
) -> Result<Option<LfSession>> {
    let Some(session) = session_by_id(store, session_id).await? else {
        return Ok(None);
    };
    if session.completed_at.is_some() && !open_completed {
        bail!("Session {} is already complete", session.id);
    }
    Ok(Some(session))
}

/// The provider and capture location for a conversation's current input.
struct NativeSession<'a> {
    dir: PathBuf,
    provider: &'a str,
}

impl<'a> NativeSession<'a> {
    fn of(session: &'a LfSession) -> Result<Self> {
        Ok(Self {
            dir: local_capture_dir(&session.artifact_key).ok_or_else(|| {
                anyhow!("Session {} has an invalid capture reference", session.id)
            })?,
            provider: session
                .provider
                .as_deref()
                .ok_or_else(|| anyhow!("Session {} input has no recorded provider", session.id))?,
        })
    }
}

fn session_token(session: &LfSession) -> HumanSessionToken {
    HumanSessionToken::Primary {
        id: session.id.clone(),
    }
}

/// Launch the prepared input of a conversation or of a saved Flow's review.
async fn serve_locked(store: &SharedStore, session: &LfSession, launch_lock: File) -> Result<()> {
    let lf = crate::os_process::resolve_pinned_lf_binary()?;
    let mut command = tokio::process::Command::new(lf);
    let token = session_token(session);
    command
        .current_dir(&session.cwd)
        .env(HUMAN_SESSION_ENV, serde_json::to_string(&token)?);
    command.args(conversation_launch_args(store, session).await);
    let mut child = spawn_session_process(&mut command, &session.artifact_key).await?;
    drop(launch_lock);
    // Provider termination never completes a Session.
    let status = child.wait().await.context("wait for session agent")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("session agent exited with {status}"))
    }
}

async fn conversation_launch_args(store: &SharedStore, session: &LfSession) -> Vec<String> {
    let mut args = vec![
        "-i".to_string(),
        "--agent".to_string(),
        launch_model(session),
        "--__cwd".to_string(),
        session.cwd.display().to_string(),
    ];
    // The conversation's prompt carries its Work only while launch can resolve
    // it; the Session row keeps the attribution either way.
    if let Some(selector) = work_selector(session) {
        if crate::ops::resolve_work_binding(store, &session.cwd, &selector)
            .await
            .is_ok()
        {
            let (kind, value) = selector.split_once(':').expect("Work selector has a kind");
            args.extend([format!("--{kind}"), value.to_string()]);
        }
    }
    match &session.skill {
        Some(skill) => args.extend(["skill".to_string(), "--".to_string(), skill.clone()]),
        None => args.push(":".to_string()),
    }
    args.push(
        session
            .request
            .clone()
            .unwrap_or_else(|| PRIMARY_MESSAGE.to_string()),
    );
    args
}

pub(crate) fn capture_subjects(
    session: &LfSession,
) -> Vec<crate::session_record::SubjectAttribution> {
    work_selector(session)
        .map(|selector| crate::session_record::SubjectAttribution {
            selector,
            source: if session.work_source == Some(WorkSource::Inherited) {
                crate::session_record::AttributionSource::Inherited
            } else {
                crate::session_record::AttributionSource::Declared
            },
        })
        .into_iter()
        .collect()
}

fn work_selector(session: &LfSession) -> Option<String> {
    match (&session.task_id, &session.wave_id) {
        (Some(task), _) => Some(format!("task:{task}")),
        (None, Some(wave)) => Some(format!("wave:{wave}")),
        (None, None) => None,
    }
}

pub(crate) fn prepared_artifact_key() -> Result<Option<String>> {
    let Some(value) = std::env::var_os(PREPARED_CAPTURE_ENV) else {
        return Ok(None);
    };
    std::env::remove_var(PREPARED_CAPTURE_ENV);
    active_session_token()?;
    Ok(Some(crate::session_record::parse_artifact_key(
        &value
            .into_string()
            .map_err(|_| anyhow!("prepared input id is not UTF-8"))?,
    )?))
}

pub(crate) async fn open(
    store: &SharedStore,
    session_id: &str,
    mode: OpenMode,
    resume: bool,
) -> Result<SessionRecord> {
    crate::data_clone::require_writable_session(session_id)?;
    let progress = resume.then(|| crate::startup::Startup::new("Preparing conversation"));
    let target = match find_session(store, session_id, true).await? {
        Some(target) => target,
        // Connecting is what brings a provider-started conversation in.
        None => provider_conversation::admit(store, session_id)
            .await?
            .ok_or_else(|| session_not_found(session_id))?,
    };
    let session = &target;
    crate::data_clone::require_writable_session(&session.id)?;
    if resume
        && store
            .sqlite
            .input_provider_session(&session.artifact_key)?
            .is_some()
    {
        if let Some(provider) = session.provider.as_deref() {
            if let crate::config::Resume::Import { agent } = crate::config::resume_choice(
                provider,
                session.model.as_deref(),
                crate::config::agent_override().as_deref(),
            ) {
                mode.require_action(surface(store, session).await?.state)?;
                import(store, &session.id, &agent).await?;
                let session = store
                    .session(&session.id)
                    .await?
                    .ok_or_else(|| session_not_found(&session.id))?;
                return surface(store, &session).await;
            }
        }
    }
    let admitted;
    let session = if resume {
        let id = session.id.clone();
        let _launch = tokio::task::spawn_blocking(move || lock_session_process(&id)).await??;
        admitted = primary::admit_workspace(store, session.clone()).await?;
        &admitted
    } else {
        session
    };
    let native = NativeSession::of(session)?;
    let provider_session = store.sqlite.input_provider_session(&session.artifact_key)?;
    if resume && matches!(native.provider, "claude" | "opencode") {
        crate::lf::commands::util::require_provider_session_process(&native.dir)?;
        stop_headless_for_resume(store, session, provider_session.as_ref()).await?;
    }
    drop(progress);
    let Some(provider_session) = provider_session else {
        let surface = surface(store, session).await?;
        if resume {
            open_waiting(store, &session.id).await?;
        }
        return Ok(surface);
    };
    if resume {
        crate::lf::commands::util::require_provider_session_process(&native.dir)?;
    }
    if resume
        && native.provider == "codex"
        && connect_live_codex(store, session, &provider_session).await?
    {
        let session = store
            .sqlite
            .session(&session.id)?
            .ok_or_else(|| session_not_found(&session.id))?;
        return surface(store, &session).await;
    }
    if resume && mode == OpenMode::Replace {
        crate::harness::agent_process::stop_session_agent(&store.sqlite, &session.id)?;
    }
    let mut result = surface(store, session).await?;
    mode.require_action(result.state)?;
    if resume {
        crate::lf::commands::util::resume_session(
            native.provider,
            session.model.as_deref(),
            &session.cwd,
            &session.artifact_key,
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
/// End the recorded headless group before reopening its native conversation.
/// Missing native identity must refuse before stopping otherwise useful work.
async fn stop_headless_for_resume(
    store: &SharedStore,
    session: &LfSession,
    provider: Option<&crate::session_record::ProviderSessionRef>,
) -> Result<()> {
    let Some(expected) = store.sqlite.session_attachment(&session.id)? else {
        return Ok(());
    };
    let Some(agent) = store
        .sqlite
        .agent_processes()?
        .into_iter()
        .find(|agent| agent.process.id == expected.agent_process_id)
    else {
        return Ok(());
    };
    if agent.interactive || agent.process.completed_at.is_some() {
        return Ok(());
    }
    let provider = provider.ok_or_else(|| {
        anyhow!(
            "Native conversation identity is unavailable; cannot stop and resume this headless run"
        )
    })?;
    if let Some(thread) = store.sqlite.session_thread(&session.id)? {
        anyhow::ensure!(
            thread == provider.agent_session,
            "Recorded conversation differs from the headless conversation"
        );
    }
    let process = crate::journal::current_lf_process_id()
        .ok_or_else(|| anyhow!("Connecting requires the current lf Process"))?;
    let sqlite = store.sqlite.clone();
    let id = session.id.clone();
    tokio::task::spawn_blocking(move || {
        let attachment = sqlite.claim_session_attachment(&id, Some(&expected), &process, false)?;
        sqlite.finish_session_attachment(&id, &attachment, "interrupted", || {
            if !crate::harness::agent_process::close_session_agent_process(&sqlite, &id)? {
                return Err(crate::store::StoreError::InvalidAuthority(
                    "Headless AgentProcess identity is unavailable".into(),
                ));
            }
            Ok(true)
        })
    })
    .await??;
    eprintln!(
        "Stopped headless {}; resuming the same native conversation (not live takeover).",
        session.provider.as_deref().unwrap_or("provider")
    );
    Ok(())
}

/// Connect to one existing provider thread. Native UI traffic crosses the same
/// attachment fence as the headless writer; client exit leaves the provider alive.
#[cfg(unix)]
async fn connect_live_codex(
    store: &SharedStore,
    session: &LfSession,
    provider: &crate::session_record::ProviderSessionRef,
) -> Result<bool> {
    let expected = store.sqlite.session_attachment(&session.id)?;
    let Some((endpoint, thread)) = store.sqlite.session_connection(&session.id)? else {
        return Ok(false);
    };
    if thread != provider.agent_session {
        bail!("Recorded conversation differs from the live provider thread");
    }
    let process = crate::journal::current_lf_process_id()
        .ok_or_else(|| anyhow!("Connecting requires the current lf Process"))?;
    let attachment =
        match store
            .sqlite
            .claim_session_attachment(&session.id, expected.as_ref(), &process, false)
        {
            Ok(attachment) => attachment,
            Err(crate::store::StoreError::InvalidAuthority(_))
                if store.sqlite.session_connection(&session.id)?.is_none() =>
            {
                // Close won the race. The ordinary open path resumes saved history.
                return Ok(false);
            }
            Err(error) => return Err(error.into()),
        };
    let interrupted_store = store.sqlite.clone();
    let interrupted_session = session.id.clone();
    let interrupted_attachment = attachment.clone();
    crate::agent::register_interrupt_cleanup(move || {
        // A native client owns its attachment, never the surviving provider's exit.
        match interrupted_store.finish_session_attachment(
            &interrupted_session,
            &interrupted_attachment,
            "interrupted",
            || Ok(false),
        ) {
            Ok(()) | Err(crate::store::StoreError::InvalidAuthority(_)) => {}
            Err(error) => tracing::warn!(%error, "record interrupted Session connection"),
        }
    });
    let connected = async {
        // Retain the AgentProcess while this lf invocation is attached.
        crate::os_process::hold_agent_process_lifeline(
            &crate::os_process::agent_process_lifeline_path(Path::new(&endpoint)),
        )
        .context("Codex AgentProcess is stopping after its attached lf exited")?;
        store.sqlite.make_session_interactive(&session.id, &attachment)?;
        let directory = tempfile::tempdir()?;
        let remote = directory.path().join("client.sock");
        let listener = tokio::net::UnixListener::bind(&remote)?;
        let connection = crate::harness::codex_connection::CodexConnection {
            store: store.sqlite.clone(), session_id: session.id.clone(), thread_id: Some(thread), attachment: Some(attachment.clone()), account_id: provider.account_id.clone(),
        };
        let empty = connection.recover_history(Path::new(&endpoint)).await?;
        let initial = if empty {
            let (_, manifest) = crate::session_record::resolve_manifest(
                &crate::store::lf_home_dir(), &session.artifact_key,
            )?;
            manifest.process.map(|request| request.codex_initial_input()).filter(|input| !input.is_empty())
        } else { None };
        let initial = std::sync::Arc::new(tokio::sync::Mutex::new(crate::harness::codex_connection::NativeInput { params: None, text: initial }));
        let upstream = PathBuf::from(&endpoint);
        let relay = tokio::spawn(async move {
            let mut clients = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((client, _)) = accepted else { break };
                        let connection = connection.clone();
                        let endpoint = endpoint.clone();
                        let initial = initial.clone();
                        clients.spawn(async move { connection.serve_with_input(client, Path::new(&endpoint), Some(initial)).await });
                    }
                    result = clients.join_next(), if !clients.is_empty() => {
                        if let Some(Ok(Err(error))) = result { tracing::warn!(%error, "native conversation connection ended"); }
                    }
                }
            }
        });
        let session = session.clone();
        let provider = provider.clone();
        let launch_attachment = attachment.clone();
        let environment = BTreeMap::from([(
            crate::process::AGENT_CALLER_ENV.into(),
            serde_json::to_string(&launch_attachment.caller(session.id.clone()))?,
        )]);
        let result = tokio::task::spawn_blocking(move || {
            crate::lf::commands::util::open_saved_session_with_env(
                "codex", session.model.as_deref(), &session.cwd, &session.artifact_key, Some(&provider),
                &environment, None, Some(crate::lf::commands::util::NativeConnection { relay: remote, upstream }),
                Some((session.id.clone(), launch_attachment)),
            )
        }).await;
        relay.abort();
        let _ = relay.await;
        result??;
        Ok::<_, anyhow::Error>(true)
    }.await;
    match store.sqlite.finish_session_attachment(
        &session.id,
        &attachment,
        if connected.is_ok() {
            "completed"
        } else {
            "failed"
        },
        || Ok(false),
    ) {
        Ok(_) | Err(crate::store::StoreError::InvalidAuthority(_)) => {}
        Err(error) => return Err(error.into()),
    }
    connected
}

/// Open a conversation: resume its native history, else
/// launch its prepared input, else append another attempt to the Session.
async fn open_waiting(store: &SharedStore, id: &str) -> Result<()> {
    let lock_id = id.to_string();
    let launch_lock = tokio::task::spawn_blocking(move || lock_session_process(&lock_id)).await??;
    // Resolve the current input under the launch lock, including completion
    // or replacement that happened while this opener waited.
    let session = store
        .session(id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    let session = primary::admit_workspace(store, session).await?;
    let mut launch_lock = Some(launch_lock);
    let token = session_token(&session);
    if resume_native_session(store, &session.artifact_key, &token, &mut launch_lock)? {
        return Ok(());
    }
    // A consumed launch without native history gets another attempt under
    // the same Session; its title and feedback never leave the row.
    let session = if capture_is_prepared(&session.artifact_key)? {
        session
    } else {
        if session.input_published {
            append_input(store, session).await?
        } else {
            publish_prepared_input(
                &store.sqlite,
                &session,
                crate::session_record::SessionFlowMembership::Independent,
            )?
        }
    };
    serve_locked(
        store,
        &session,
        launch_lock.expect("an unresumed Session retains its launch lock"),
    )
    .await
}

/// Continue a conversation on another provider. The Session keeps its name,
/// Work and feedback; a fresh AgentSession starts from a log of the current one.
async fn import(store: &SharedStore, id: &str, agent: &str) -> Result<()> {
    let lock_id = id.to_string();
    let launch_lock = tokio::task::spawn_blocking(move || lock_session_process(&lock_id)).await??;
    let session = store
        .session(id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    let session = primary::admit_workspace(store, session).await?;
    let request = conversation_log::continuation(&store.sqlite, &session)?;
    crate::harness::agent_process::stop_session_agent(&store.sqlite, &session.id)?;
    let mut next = session;
    let (provider, model) = crate::config::parse_agent(agent);
    next.provider = Some(provider);
    next.model = model;
    // The skill ran in the earlier conversation and is in its log. The
    // continuation is stored so a relaunch after a failed start still carries it.
    next.skill = None;
    next.request = Some(request);
    let next = append_input(store, next).await?;
    serve_locked(store, &next, launch_lock).await
}

/// Append a fresh, published input to the Session; earlier inputs stay in its history.
async fn append_input(store: &SharedStore, mut next: LfSession) -> Result<LfSession> {
    let replaced = next.captured;
    next.artifact_key = crate::session_record::new_artifact_key();
    next.input_published = false;
    let next = store.replace_session_input(replaced, next).await?;
    publish_prepared_input(
        &store.sqlite,
        &next,
        crate::session_record::SessionFlowMembership::Independent,
    )
}

pub(crate) fn capture_is_prepared(run_id: &str) -> Result<bool> {
    match crate::session_record::resolve_manifest(&crate::store::lf_home_dir(), run_id) {
        Ok((dir, _)) => Ok(dir.join("prepared").is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).context("resolve prepared Session input"),
    }
}

/// A Session as its row and its current input describe it.
async fn surface(store: &SharedStore, session: &LfSession) -> Result<SessionRecord> {
    let work = match (&session.task_id, &session.wave_id) {
        (Some(task), _) => Some(WorkRef::Task(task.clone())),
        (None, Some(wave)) => Some(WorkRef::Wave(wave.clone())),
        (None, None) => None,
    };
    let metadata = store
        .sqlite
        .session_summary(
            &session.id,
            time::OffsetDateTime::now_utc().unix_timestamp(),
        )?
        .ok_or_else(|| session_not_found(&session.id))?;
    let state = session_state(&metadata);
    let mut actions = session_actions(state);
    let open_argv = if let Some(reason) = crate::data_clone::session_unavailable(&session.id)? {
        for action in &mut actions {
            action.unavailable_reason = Some(reason.clone());
        }
        Vec::new()
    } else {
        human_open_argv(&session.id)?
    };
    let flow_membership = match (&session.flow_lf_process_id, &metadata.flow) {
        (None, _) if metadata.independent => SessionFlowMembership::Independent,
        (None, _) => SessionFlowMembership::Unknown {
            reason: "Flow membership was not recorded".into(),
        },
        (Some(id), Some(flow)) => SessionFlowMembership::Step {
            flow: flow.name.clone(),
            flow_lf_process_id: id.clone(),
            step: session.skill.clone().unwrap_or_default(),
            node: session.node,
            iterations: session.iterations.clone(),
            occurrence: flow_occurrence(flow.state, metadata.flow_step_latest),
        },
        (Some(id), None) => SessionFlowMembership::Unknown {
            reason: format!("Flow {id} is unavailable"),
        },
    };
    let mut reading = SessionRecord {
        primary_scope: metadata.primary_scope.clone(),
        attention: session_attention(&metadata),
        program_status: metadata.program_status.clone(),
        agent_process_id: metadata.agent_process_id.clone(),
        task_primary: metadata.task_primary,
        task_ids: store.sqlite.session_task_ids(&session.id)?,
        id: session.id.clone(),
        workspace: None,
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
        detail: launch_model(session),
        provider: session.provider.clone(),
        cwd: session.cwd.display().to_string(),
        state,
        ready_summary: session.ready_summary.clone(),
        terminal_ids: metadata.terminal_id.clone().into_iter().collect(),
        open_argv,
    };
    workspace::associate(store, std::slice::from_mut(&mut reading)).await?;
    Ok(reading)
}

/// The `provider[:model]` a captured input launched with.
fn launch_model(session: &LfSession) -> String {
    let provider = session.provider.clone().unwrap_or_default();
    match &session.model {
        Some(model) => format!("{provider}:{model}"),
        None => provider,
    }
}

async fn session_work_path(store: &SharedStore, session: &LfSession) -> Result<Option<String>> {
    let Some(wave_id) = &session.wave_id else {
        return Ok(None);
    };
    let wave = match store.get_wave(wave_id).await? {
        Some(wave) => wave.slug().to_string(),
        None => format!("Wave {wave_id} (unavailable)"),
    };
    Ok(Some(match &session.task_id {
        None => wave,
        Some(id) => match store.get_task(id).await? {
            Some(task) => format!("{wave} / {}", task.identifier),
            None => format!("Task {id} (unavailable)"),
        },
    }))
}

pub(crate) fn local_capture_dir(artifact_key: &str) -> Option<PathBuf> {
    crate::session_record::record_dir(&crate::store::lf_home_dir(), artifact_key)
}

/// Rename a Session and return the authoritative record.
/// `Generated` is an agent suggestion; it never replaces a human-assigned name.
pub(crate) async fn rename(
    store: &SharedStore,
    session_id: &str,
    title: &str,
    source: SessionTitleSource,
) -> Result<SessionRecord> {
    let session = find_session(store, session_id, false)
        .await?
        .ok_or_else(|| session_not_found(session_id))?;
    let title = crate::naming::validate_session_title(title)
        .map_err(|error| anyhow!("cannot rename Session {session_id}: {error}"))?;
    let title_source = match source {
        SessionTitleSource::Human => crate::session::TitleSource::Human,
        SessionTitleSource::Generated => crate::session::TitleSource::Generated,
        SessionTitleSource::Unavailable => bail!("unavailable is not a title source"),
    };
    store
        .rename_session(&session.id, title, title_source)
        .await?;
    let session = store
        .session(&session.id)
        .await?
        .ok_or_else(|| session_not_found(session_id))?;
    surface(store, &session).await
}

/// Assign a Task to future Session work by its durable ID, including closed Sessions.
pub(crate) async fn bind(store: &SharedStore, id: &str, task: &str) -> Result<SessionRecord> {
    let (session, task) = binding_target(store, id, task).await?;
    let session = store
        .bind_session(&session.id, session.captured, &task.id)
        .await
        .map_err(|error| match error {
            crate::store::StoreError::InvalidAuthority(reason) => anyhow!(reason),
            error => anyhow!(error),
        })?;
    eprintln!(
        "Binding {} to {} ({}). Permanent.",
        session.id, task.identifier, task.title
    );
    surface(store, &session).await
}

/// Resolved identity for confirmation, not a reservation or permission to bind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBindingPreview {
    pub session_id: String,
    pub task_id: String,
    pub identifier: String,
    pub title: String,
}

pub(crate) async fn preview_binding(
    store: &SharedStore,
    id: &str,
    task: &str,
) -> Result<SessionBindingPreview> {
    let (session, task) = binding_target(store, id, task).await?;
    Ok(SessionBindingPreview {
        session_id: session.id,
        task_id: task.id.to_string(),
        identifier: task.identifier,
        title: task.title,
    })
}

async fn binding_target(store: &SharedStore, id: &str, task: &str) -> Result<(LfSession, Task)> {
    let session = session_by_id(store, id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    let task = match crate::durable::TaskId::parse(task) {
        Ok(id) => store.get_task(&id).await?,
        Err(_) => store.get_task_by_issue(task).await?,
    }
    .ok_or_else(|| anyhow!("Task {task:?} is not registered"))?;
    Ok((session, task))
}

/// Resolve a Session in any state by its durable or native conversation id.
pub(crate) async fn session_by_id(store: &SharedStore, id: &str) -> Result<Option<LfSession>> {
    if let Some(session) = store.session(id).await? {
        return Ok(Some(session));
    }
    provider_conversation::recorded(store, id).await
}

fn session_not_found(id: &str) -> anyhow::Error {
    anyhow!("Session {id} was not found")
}

pub(crate) async fn spawn_session_process(
    command: &mut tokio::process::Command,
    artifact_key: &String,
) -> Result<tokio::process::Child> {
    let home = crate::store::lf_home_dir();
    let executable = Path::new(command.as_std().get_program());
    let digest = fs::read(executable)
        .ok()
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .unwrap_or_else(|| "unavailable".to_string());
    let cwd = match command.as_std().get_current_dir() {
        Some(cwd) => cwd.to_path_buf(),
        None => std::env::current_dir()?,
    };
    let database = crate::store::database_path_from_env()?;
    // Capture the child before it parses arguments. Its own Session recorder may
    // never start; the opening Process must retain enough evidence to diagnose it.
    // Arguments and environment values can contain prompts or credentials.
    let launch = format!(
        "Session input {artifact_key}: executable {} (sha256 {digest}), cwd {}, Machine {}, database {}",
        executable.display(),
        cwd.display(),
        home.display(),
        database.display()
    );
    if capture_is_prepared(artifact_key)? {
        command.env(PREPARED_CAPTURE_ENV, artifact_key.as_str());
    }
    let mut child = command
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("could not start {launch}"))?;
    let deadline = tokio::time::Instant::now() + SESSION_START_TIMEOUT;
    loop {
        match crate::session_record::resolve_manifest(&home, artifact_key.as_str()) {
            Ok((dir, _manifest)) => {
                if session_is_resumable(&SqliteStore::open_processes_read_only(&database)?, &dir)? {
                    return Ok(child);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        if let Some(status) = child.try_wait().context("probe human Session input")? {
            // A finite terminal can publish native history and exit between
            // probes. Its successful exit does not make that history unusable.
            if status.success() {
                if let Ok((dir, _)) = crate::session_record::resolve_manifest(&home, artifact_key) {
                    if crate::session_record::read_provider_session(&dir)?.is_some() {
                        return Ok(child);
                    }
                }
            }
            bail!("{launch}: exited with {status} before becoming resumable");
        }
        if tokio::time::Instant::now() >= deadline {
            // Reap the launcher before returning. Protocol providers are tied to
            // its lifeline, including wrappers that never finish starting.
            child
                .kill()
                .await
                .context("stop timed-out Session launcher")?;
            bail!("{launch}: did not become resumable within 30s");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn session_is_resumable(store: &SqliteStore, dir: &Path) -> Result<bool> {
    if crate::session_record::read_provider_session(dir)?.is_none() {
        return Ok(false);
    }
    let input = crate::session_record::input_id_from_dir(dir)?;
    let session = store
        .session_for_artifact(&input)?
        .context("Session disappeared")?;
    let summary = store
        .session_summary(
            &session.id,
            time::OffsetDateTime::now_utc().unix_timestamp(),
        )?
        .context("Session disappeared")?;
    Ok(session_state(&summary) == SessionState::Active)
}

pub(crate) fn resume_native_session(
    store: &SharedStore,
    artifact_key: &String,
    token: &HumanSessionToken,
    launch_lock: &mut Option<File>,
) -> Result<bool> {
    #[cfg(test)]
    if let Some(result) = action_test::resume(artifact_key, launch_lock) {
        return result;
    }
    let Some(session) = store.sqlite.session_for_artifact(artifact_key)? else {
        return Ok(false);
    };
    crate::data_clone::require_writable_session(&session.id)?;
    if session.artifact_key != *artifact_key {
        bail!("Session {} changed its input before resume", session.id);
    }
    let native = NativeSession::of(&session)?;
    let provider_session = store.sqlite.input_provider_session(artifact_key)?;
    if provider_session.is_none() {
        if native.provider != "codex" {
            return Ok(false);
        }
        // A failed Codex bootstrap may have no provider history yet. Reopen its
        // captured request, not the Session's optional summary/request label.
        let saved =
            crate::session_record::resolve_manifest(&crate::store::lf_home_dir(), artifact_key)?;
        if saved.1.process.is_none() {
            return Ok(false);
        }
    }
    crate::lf::commands::util::require_provider_session_process(&native.dir)?;
    crate::harness::agent_process::stop_session_agent(&store.sqlite, &session.id)?;
    let environment =
        BTreeMap::from([(HUMAN_SESSION_ENV.to_string(), serde_json::to_string(token)?)]);
    crate::lf::commands::util::open_saved_session_with_env(
        native.provider,
        session.model.as_deref(),
        &session.cwd,
        &session.artifact_key,
        provider_session.as_ref(),
        &environment,
        launch_lock.take(),
        None,
        None,
    )?;
    Ok(true)
}

fn human_open_argv(id: &str) -> Result<Vec<String>> {
    crate::data_clone::require_writable_session(id)?;
    let context = crate::os_process::execution_context()?;
    // A fresh terminal does not inherit the listing process's data selection.
    // Carry the executable and its data together, including when a different
    // installation becomes current between listing and opening.
    Ok(vec![
        "/usr/bin/env".to_string(),
        format!("LF_BIN={}", context.lf_bin.display()),
        format!("LF_HOME={}", context.lf_home.display()),
        context.lf_bin.display().to_string(),
        "session".to_string(),
        "connect".to_string(),
        id.to_string(),
    ])
}

const PRIMARY_MESSAGE: &str = "<lf:primary-session>\nThis is the one ongoing primary conversation of its repository, Wave or Task. Reconcile current evidence, then work with the user.\n</lf:primary-session>";

pub(crate) fn lock_session_process(id: &str) -> Result<File> {
    let directory = crate::store::lf_home_dir().join(LAUNCH_LOCK_DIRECTORY);
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
    let deadline = std::time::Instant::now() + SESSION_START_TIMEOUT;
    loop {
        match FileExt::try_lock_exclusive(&file) {
            Ok(()) => break,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() >= deadline {
                    bail!("Session launch is still busy; retry after its current launch settles");
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => return Err(error).context("lock Session launch"),
        }
    }
    Ok(file)
}

fn active_session_token() -> Result<HumanSessionToken> {
    let raw =
        std::env::var(HUMAN_SESSION_ENV).context("this command requires an active session")?;
    serde_json::from_str(&raw).context("active session token is invalid")
}

/// The observer owns only this input stream, never the Session or its provider.
/// A replacement observer fences the previous stream even on the same provider.
pub(crate) async fn observe_program_status(
    store: &SharedStore,
    id: &str,
    terminal: &str,
    agent_process: Option<&crate::id::LfProcessId>,
) -> Result<()> {
    let session = find_session(store, id, false)
        .await?
        .context("Session not found")?;
    let current = store
        .sqlite
        .session_summary(
            &session.id,
            time::OffsetDateTime::now_utc().unix_timestamp(),
        )?
        .context("Session disappeared")?;
    anyhow::ensure!(
        current.agent_process_id.as_ref() == agent_process,
        "Session provider changed"
    );
    anyhow::ensure!(
        current.terminal_id.as_deref() == Some(terminal),
        "terminal has no active Session attachment"
    );
    let stream = uuid::Uuid::new_v4().to_string();
    anyhow::ensure!(
        store
            .sqlite
            .begin_program_status(&session.id, agent_process, &stream)?,
        "Session provider changed"
    );
    let mut last = None;
    let mut sequence = 0;
    let mut pending = None;
    let mut next_write = std::time::Instant::now();
    let mut bytes = Vec::new();
    let mut chunk = [0; 8192];
    loop {
        let count = read_observation_chunk(&mut chunk)?;
        if count == Some(0) {
            anyhow::ensure!(bytes.is_empty(), "incomplete Program Status snapshot");
            break;
        }
        for byte in &chunk[..count.unwrap_or(0)] {
            if *byte == b'\n' {
                let records: crate::program_status::Records = serde_json::from_slice(&bytes)?;
                bytes.clear();
                anyhow::ensure!(
                    records.seen && records.validate(),
                    "invalid Program Status snapshot"
                );
                pending = Some(records);
            } else {
                anyhow::ensure!(
                    bytes.len() < 512 * 1024,
                    "Program Status snapshot exceeds limit"
                );
                bytes.push(*byte);
            }
        }
        if std::time::Instant::now() >= next_write {
            if let Some(records) = pending.take().filter(|r| last.as_ref() != Some(r)) {
                sequence += 1;
                anyhow::ensure!(
                    store.sqlite.record_program_status(
                        &session.id,
                        agent_process,
                        &stream,
                        sequence,
                        &records
                    )?,
                    "Program Status stream replaced"
                );
                last = Some(records);
                next_write = std::time::Instant::now() + std::time::Duration::from_millis(250);
            }
        }
    }
    if let Some(records) = pending.filter(|r| last.as_ref() != Some(r)) {
        anyhow::ensure!(
            store.sqlite.record_program_status(
                &session.id,
                agent_process,
                &stream,
                sequence + 1,
                &records
            )?,
            "Program Status stream replaced"
        );
    }
    Ok(())
}

// This command is the sole stdin reader. Polling avoids a blocking stdin worker
// that could survive a replaced observer and keep the process alive indefinitely.
#[cfg(unix)]
fn read_observation_chunk(bytes: &mut [u8]) -> std::io::Result<Option<usize>> {
    let mut fd = libc::pollfd {
        fd: libc::STDIN_FILENO,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: fd points to one initialized pollfd for the process's stdin.
    let ready = unsafe { libc::poll(&mut fd, 1, 250) };
    if ready < 0 {
        return Err(std::io::Error::last_os_error());
    }
    if ready == 0 {
        return Ok(None);
    }
    // SAFETY: bytes is writable for its stated length; this is the only reader.
    let count = unsafe { libc::read(fd.fd, bytes.as_mut_ptr().cast(), bytes.len()) };
    if count < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(Some(count as usize))
    }
}
#[cfg(not(unix))]
fn read_observation_chunk(_: &mut [u8]) -> std::io::Result<Option<usize>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "terminal observation requires Unix",
    ))
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[test]
    fn public_open_stops_headless_group_and_resumes_saved_conversation() {
        use std::os::unix::fs::PermissionsExt;
        const CHILD: &str = "LOOPFLOW_RESUME_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            for provider in ["claude", "opencode"] {
                let mut child = std::process::Command::new(std::env::current_exe().unwrap());
                child.args(["--exact", "ops::human_session::tests::public_open_stops_headless_group_and_resumes_saved_conversation", "--nocapture"]);
                for (key, _) in std::env::vars_os() {
                    if key.to_string_lossy().starts_with("LF_") {
                        child.env_remove(key);
                    }
                }
                let output = child.env(CHILD, provider).output().unwrap();
                assert!(
                    output.status.success(),
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            return;
        }
        let provider_name = std::env::var(CHILD).unwrap();
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let home = ledger.home();
        let bin = home.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let lf = bin.join("lf");
        std::fs::write(&lf, "#!/bin/sh\nexit 95\n").unwrap();
        std::fs::set_permissions(&lf, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("LF_BIN", &lf);
        let native = bin.join(&provider_name);
        std::fs::write(&native, "#!/bin/sh\nstate=$(ps -o stat= -p \"$(cat provider-pid)\" | tr -d ' ')\ncase \"$state\" in \"\"|Z*) ;; *) exit 96;; esac\nprintf '%s\\n' \"$@\" > client-args\n").unwrap();
        std::fs::set_permissions(&native, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _path = crate::test_ambient::EnvGuard::clear(&["PATH"]);
        std::env::set_var("PATH", format!("{}:/usr/bin:/bin", bin.display()));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        crate::journal::with_process(|| {
            crate::journal::admit_process(home, &["lf".into(), "session".into()]);
            runtime.block_on(async {
                let store = std::sync::Arc::new(
                    crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                        home.join("loopflow.db"),
                    ))
                    .await?,
                );
                let capture = crate::session_record::CaptureHandle::begin_at(
                    home,
                    crate::session_record::SessionCaptureSpec {
                        harness: provider_name.clone(),
                        model: None,
                        surface: "headless".into(),
                        cwd: home.into(),
                        repo: None,
                        worktree: None,
                        skill: None,
                        subjects: vec![],
                        flow: crate::session_record::SessionFlowMembership::Independent,
                        work: None,
                    },
                )?;
                let session = store
                    .sqlite
                    .session_for_artifact(&capture.artifact_key())?
                    .unwrap();
                let launcher = crate::id::LfProcessId::new();
                rusqlite::Connection::open(home.join("loopflow.db"))?.execute(
                    "INSERT INTO processes(id,trace_id,started_at) SELECT ?1,trace_id,started_at FROM processes WHERE id=?2",
                    rusqlite::params![launcher, crate::journal::current_lf_process_id().unwrap()],
                )?;
                let first = store.sqlite.claim_session_attachment(&session.id, None, &launcher, true)?;
                let mut command = tokio::process::Command::new("/bin/sleep");
                command
                    .env_clear()
                    .arg("60")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                let mut provider = crate::harness::agent_process::spawn(
                    command,
                    None,
                    &(store.sqlite.clone(), session.id.clone(), first.clone()),
                )?;
                std::fs::write(home.join("provider-pid"), provider.id().unwrap().to_string())?;
                // Without native identity, connect must leave the running provider alone.
                let refused = super::open(&store, &session.id, super::OpenMode::Try, true).await;
                assert!(refused.unwrap_err().to_string().contains("identity is unavailable"));
                assert!(provider.try_wait()?.is_none());
                assert_eq!(store.sqlite.session_attachment(&session.id)?, Some(first.clone()));
                crate::session_record::write_provider_session(
                    &capture.artifact_dir(), &"saved-thread".into(), None,
                )?;
                let origin = store.sqlite.session_turn_origin(&session.id, &first)?;
                store.sqlite.record_session_turn_origin(&"saved-thread".into(), "pending", &origin)?;
                let result = super::open(&store, &session.id, super::OpenMode::Try, true).await;
                let alive = crate::journal::OsProcess::group_is_alive(provider.id().unwrap())?;
                if alive {
                    provider.kill().await?;
                }
                result?;
                assert!(!alive, "native resume left the headless provider running");
                let row = store.sqlite.process(&first.agent_process_id)?.unwrap();
                assert!(row.completed_at.is_some());
                assert!(store.sqlite.with_session_attachment(&session.id, &first, || Ok(())).is_err());
                let history = store.sqlite.session_history(&session.id, 0, 0)?;
                assert!(history.iter().any(|event| event.provider_turn.as_deref() == Some("pending")
                    && event.kind == crate::session::SessionEventKind::Started));
                assert!(!history.iter().any(|event| event.provider_turn.as_deref() == Some("pending")
                    && event.kind == crate::session::SessionEventKind::Completed));
                let args = std::fs::read_to_string(home.join("client-args"))?;
                let flag = if provider_name == "claude" { "--resume" } else { "--session" };
                assert!(args.contains(&format!("{flag}\nsaved-thread\n")), "{args}");
                assert_eq!(store.sqlite.input_provider_session(&session.artifact_key)?.unwrap()
                    .agent_session.as_str(), "saved-thread");
                Ok::<_, anyhow::Error>(())
            })
        })
        .unwrap();
    }

    #[test]
    fn session_states_fixture_has_only_process_or_unknown_states() {
        let states: Vec<super::SessionState> = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/session_states.json"
        ))
        .unwrap();
        assert_eq!(
            states,
            vec![
                super::SessionState::Unknown,
                super::SessionState::Active,
                super::SessionState::Closed
            ]
        );
        assert!(serde_json::from_str::<super::SessionState>("\"interrupted\"").is_err());
    }

    #[test]
    fn codex_connection_launch_preserves_provider_and_rejects_replaced_attachment() {
        use futures_util::{SinkExt, StreamExt};
        use std::os::unix::fs::PermissionsExt;
        use tokio_tungstenite::{accept_async, tungstenite::Message};

        // Exercise the actual process-wide invocation across the blocking native launcher.
        // Keep its one-shot runtime initialization out of the other library tests.
        const CHILD: &str = "LOOPFLOW_CONNECTION_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child.args(["--exact", "ops::human_session::tests::codex_connection_launch_preserves_provider_and_rejects_replaced_attachment", "--nocapture"]);
            for (key, _) in std::env::vars_os() {
                if key.to_string_lossy().starts_with("LF_") {
                    child.env_remove(key);
                }
            }
            let output = child.env(CHILD, "1").output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let home = ledger.home();
        let bin = home.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let codex = bin.join("codex");
        std::fs::write(&codex, "#!/bin/sh\nprintf '%s\\n' \"$@\" > client-args\n").unwrap();
        std::fs::set_permissions(&codex, std::fs::Permissions::from_mode(0o755)).unwrap();
        // No installed provider or account can win PATH discovery.
        let _path = crate::test_ambient::EnvGuard::clear(&["PATH"]);
        std::env::set_var("PATH", format!("{}:/usr/bin:/bin", bin.display()));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        crate::journal::with_process(|| {
            crate::journal::admit_process(home, &["lf".into(), "session".into()]);
            runtime.block_on(async {
                let store = std::sync::Arc::new(
                    crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                        home.join("loopflow.db"),
                    ))
                    .await?,
                );
                for transfer in ["none", "takeover", "reattach"] {
                    let capture = crate::session_record::CaptureHandle::begin_at(
                        home,
                        crate::session_record::SessionCaptureSpec {
                            harness: "codex".into(),
                            model: None,
                            surface: "headless".into(),
                            cwd: home.into(),
                            repo: None,
                            worktree: None,
                            skill: None,
                            subjects: Vec::new(),
                            flow: crate::session_record::SessionFlowMembership::Independent,
                            work: None,
                        },
                    )?;
                    let session = store
                        .sqlite
                        .session_for_artifact(&capture.artifact_key())?
                        .unwrap();
                    let original = store.sqlite.claim_session_attachment(
                        &session.id,
                        None,
                        &crate::id::LfProcessId::new(),
                        true,
                    )?;
                    let mut command = std::process::Command::new("/bin/sleep");
                    command
                        .env_clear()
                        .arg("60")
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null());
                    let mut provider = crate::harness::agent_process::spawn_native(
                        command,
                        &(store.sqlite.clone(), session.id.clone(), original.clone()),
                    )?;
                    let endpoint = home.join(format!("{transfer}.sock"));
                    store.sqlite.record_session_connection(
                        &session.id,
                        &original,
                        endpoint.to_str().unwrap(),
                        &"saved-thread".into(),
                    )?;
                    let listener = tokio::net::UnixListener::bind(&endpoint)?;
                    let sqlite = store.sqlite.clone();
                    let session_id = session.id.clone();
                    let server = tokio::spawn(tokio::time::timeout(
                        std::time::Duration::from_secs(5),
                        async move {
                            let mut socket = accept_async(listener.accept().await?.0).await?;
                            while let Some(message) = socket.next().await {
                                let message = message?;
                                if matches!(message, Message::Close(_)) {
                                    break;
                                }
                                let Message::Text(text) = message else {
                                    continue;
                                };
                                let rpc: serde_json::Value = serde_json::from_str(&text)?;
                                let Some(id) = rpc.get("id") else { continue };
                                let history = rpc["method"] == "thread/turns/list";
                                if history && transfer != "none" {
                                    let first = sqlite.session_attachment(&session_id)?.unwrap();
                                    let second = sqlite.claim_session_attachment(
                                        &session_id,
                                        Some(&first),
                                        &crate::id::LfProcessId::new(),
                                        false,
                                    )?;
                                    if transfer == "reattach" {
                                        let third = sqlite.claim_session_attachment(
                                            &session_id,
                                            Some(&second),
                                            first.lf_process_id.as_ref().unwrap(),
                                            false,
                                        )?;
                                        assert_eq!(
                                            first.caller(session_id.clone()),
                                            third.caller(session_id.clone())
                                        );
                                        assert_ne!(first.token, third.token);
                                    }
                                }
                                let result = if history {
                                    serde_json::json!({"data":[], "nextCursor":null})
                                } else {
                                    serde_json::json!({})
                                };
                                socket
                                    .send(Message::Text(
                                        serde_json::json!({"id":id, "result":result})
                                            .to_string()
                                            .into(),
                                    ))
                                    .await?;
                            }
                            Ok::<_, anyhow::Error>(())
                        },
                    ));
                    crate::session_record::write_provider_session(
                        &capture.artifact_dir(),
                        &"saved-thread".into(),
                        None,
                    )?;
                    let native =
                        crate::session_record::read_provider_session(&capture.artifact_dir())?
                            .unwrap();
                    let result = super::connect_live_codex(&store, &session, &native).await;
                    let history = server.await;
                    let alive = provider.try_wait()?.is_none();
                    let row = store.sqlite.process(&original.agent_process_id)?.unwrap();
                    let attachment = store.sqlite.session_attachment(&session.id)?.unwrap();
                    let retained = store.sqlite.session_connection(&session.id)?;
                    // Only the test's throwaway child is stopped, after observing client effects.
                    provider.kill()?;
                    provider.wait()?;
                    history???;
                    assert!(alive && row.completed_at.is_none());
                    assert_eq!(
                        retained,
                        Some((endpoint.to_str().unwrap().into(), "saved-thread".into()))
                    );
                    let args_path = home.join("client-args");
                    if transfer == "none" {
                        assert!(result?, "composed connection did not launch");
                        let args = std::fs::read_to_string(&args_path)?;
                        assert!(args.contains("--remote\nunix://"), "{args}");
                        assert!(args.contains("client.sock") && args.contains("saved-thread"));
                        assert!(!args.contains(endpoint.to_str().unwrap()));
                        assert!(attachment.lf_process_id.is_none());
                        std::fs::remove_file(args_path)?;
                    } else {
                        assert!(result.is_err(), "stale {transfer} connection launched");
                        assert!(!args_path.exists());
                        assert!(attachment.lf_process_id.is_some());
                    }
                }
                Ok(())
            })
        })
        .unwrap();
    }

    #[test]
    fn session_metadata_wire_preserves_unknown_observation_and_occurrence() {
        let session: super::SessionRecord = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/session_metadata.json"
        ))
        .unwrap();
        assert_eq!(session.state, super::SessionState::Unknown);
        assert!(matches!(
            session.flow_membership,
            super::SessionFlowMembership::Step {
                node: None,
                iterations: None,
                occurrence: super::SessionFlowOccurrence::Unknown,
                ..
            }
        ));
        assert!(session.terminal_ids.is_empty());
        assert_eq!(
            serde_json::from_value::<super::SessionRecord>(serde_json::to_value(&session).unwrap())
                .unwrap(),
            session
        );
    }

    #[test]
    fn session_metadata_keeps_unknown_position_visible() {
        let task = TaskId::new();
        let wave = crate::id::WaveId::new();
        let mut summary = crate::session::SessionSummary {
            program_status: None,
            agent_process_id: None,
            primary_scope: None,
            agent_process_evidence: crate::journal::ProcessIdentityEvidence::Unknown,
            waiting: false,
            terminal_id: None,
            task_terminal: false,
            task_primary: false,
            task_ids: vec![task.clone()],
            captured: Some(1),
            id: "metadata".into(),
            artifact_key: crate::session_record::new_artifact_key(),
            title: "Retained conversation".into(),
            title_source: crate::session::TitleSource::Human,
            ready_summary: None,
            completed_at: None,
            interactive: true,
            task_id: Some(task.clone()),
            wave_id: Some(wave.clone()),
            flow_lf_process_id: Some("flow".into()),
            cwd: "/unavailable".into(),
            skill: Some("review".into()),
            provider: None,
            model: None,
            node: Some(2),
            iterations: None,
            flow_step_latest: false,
            flow: Some(crate::session::FlowProcessSummary {
                id: "flow".into(),
                name: "retained".into(),
                state: crate::session::FlowProcessSummaryState::Current,
                task_id: Some(task.clone()),
                wave_id: Some(wave.clone()),
                updated_at: 1,
            }),
            independent: false,
            wave_name: Some("Infrastructure".into()),
            task_identifier: Some("INF-123".into()),
        };
        let row = super::summary_surface(&summary);
        assert_eq!(row.id, summary.id);
        assert_eq!(row.work, Some(crate::durable::WorkRef::Task(task)));
        assert_eq!(row.wave_id, Some(wave));
        assert_eq!(row.work_path.as_deref(), Some("Infrastructure / INF-123"));
        assert_eq!(row.state, super::SessionState::Unknown);
        assert!(matches!(
            row.flow_membership,
            super::SessionFlowMembership::Step {
                node: Some(2),
                occurrence: super::SessionFlowOccurrence::Earlier,
                ..
            }
        ));
        summary.flow_step_latest = true;
        assert!(matches!(
            super::summary_surface(&summary).flow_membership,
            super::SessionFlowMembership::Step {
                occurrence: super::SessionFlowOccurrence::Current,
                ..
            }
        ));
        summary.flow.as_mut().unwrap().state = crate::session::FlowProcessSummaryState::Stopped;
        let row = super::summary_surface(&summary);
        assert!(matches!(
            row.flow_membership,
            super::SessionFlowMembership::Step {
                occurrence: super::SessionFlowOccurrence::Past,
                ..
            }
        ));
        assert_eq!(
            row.state,
            super::SessionState::Unknown,
            "Flow completion is not Session/process completion"
        );
        summary.flow_lf_process_id = None;
        assert!(matches!(
            super::summary_surface(&summary).flow_membership,
            super::SessionFlowMembership::Unknown { .. }
        ));
        summary.independent = true;
        assert_eq!(
            super::summary_surface(&summary).flow_membership,
            super::SessionFlowMembership::Independent
        );
        summary.ready_summary = Some("Exact retained feedback".into());
        let row = super::summary_surface(&summary);
        assert_eq!(row.state, super::SessionState::Unknown);
        assert_eq!(row.ready_summary, summary.ready_summary);
        assert_eq!(row.attention, None);
        summary.ready_summary = None;
        summary.completed_at = Some(1);
        assert_eq!(
            super::session_state(&summary),
            super::SessionState::Unknown,
            "legacy Session rows supply no process evidence"
        );
        // Session completion does not override its recorded provider's life.
        for (evidence, expected) in [
            (
                crate::journal::ProcessIdentityEvidence::Live,
                super::SessionState::Active,
            ),
            (
                crate::journal::ProcessIdentityEvidence::Dead,
                super::SessionState::Closed,
            ),
            (
                crate::journal::ProcessIdentityEvidence::Unknown,
                super::SessionState::Unknown,
            ),
        ] {
            summary.agent_process_evidence = evidence;
            assert_eq!(super::session_state(&summary), expected);
        }
    }

    use std::ffi::OsString;

    use super::{human_open_argv, session_is_resumable};
    use crate::session::LfSession;
    use crate::store::{open_ephemeral_store, SharedStore, StorageConfig};
    use crate::work::task::TaskId;

    pub(super) struct SessionHome {
        home: tempfile::TempDir,
        previous: Vec<(&'static str, Option<OsString>)>,
        _ambient: crate::test_ambient::EnvGuard,
    }

    impl SessionHome {
        pub(super) fn new() -> Self {
            let ambient = crate::test_ambient::EnvGuard::new();
            let home = tempfile::tempdir().unwrap();
            let previous = ["LF_HOME", "LF_BIN"]
                .into_iter()
                .map(|name| {
                    let value = std::env::var_os(name);
                    std::env::remove_var(name);
                    (name, value)
                })
                .collect();
            std::env::set_var("LF_HOME", home.path());
            std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
            Self {
                home,
                previous,
                _ambient: ambient,
            }
        }

        pub(super) async fn store(&self) -> SharedStore {
            std::sync::Arc::new(
                open_ephemeral_store(&StorageConfig::sqlite(self.home.path().join("loopflow.db")))
                    .await
                    .unwrap(),
            )
        }
    }

    impl Drop for SessionHome {
        fn drop(&mut self) {
            for (key, value) in self.previous.drain(..) {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    #[test]
    fn session_action_fixtures_match_the_shared_boundary() {
        #[derive(serde::Deserialize)]
        struct Case {
            state: super::SessionState,
            actions: Vec<super::SessionAction>,
        }
        let cases: Vec<Case> = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/session_actions.json"
        ))
        .unwrap();
        for case in cases {
            assert_eq!(super::session_actions(case.state), case.actions);
            for action in case.actions {
                let outcome = super::require_session_action(case.state, action.kind);
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
            assert_eq!(session.actions, super::session_actions(session.state));
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
            assert_eq!(session.actions, super::session_actions(session.state));
            kinds.push(match &session.flow_membership {
                super::SessionFlowMembership::Step { occurrence, .. } => match occurrence {
                    super::SessionFlowOccurrence::Unknown => "unknown_occurrence",
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
        // A remote Session's Session capture manifest is not here: no provider is claimed.
        assert_eq!(sessions.last().unwrap().provider, None);
        assert_eq!(sessions[0].provider.as_deref(), Some("codex"));
    }

    #[test]
    fn session_wire_names_the_conversation_without_a_run_reference() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../../tests/fixtures/dto/session.json"))
                .unwrap();
        for kind in ["conversation", "flow"] {
            let mut value = fixture.clone();
            value["kind"] = kind.into();
            let session: super::SessionRecord = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(session.id, fixture["id"].as_str().unwrap());
            assert!(serde_json::to_value(session)
                .unwrap()
                .get("run_id")
                .is_none());
            value.as_object_mut().unwrap().remove("id");
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
        let session = |task_id: Option<crate::work::task::TaskId>| LfSession {
            captured: None,
            caller_artifact_key: None,
            id: "conversation".into(),
            artifact_key: crate::session_record::new_artifact_key(),
            input_published: true,
            cwd: directory.path().into(),
            skill: None,
            provider: None,
            model: None,
            node: None,
            iterations: None,
            task_id,
            wave_id: Some(wave.id().clone()),
            flow_lf_process_id: None,
            work_source: None,
            bound_at: None,
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
        let unbound = LfSession {
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

    #[test]
    fn old_captured_membership_keeps_its_provenance_without_a_fabricated_node() {
        use crate::session_record::SessionFlowStep;
        // Captures from before node keys carried a leaf index and scalar visit.
        let old: SessionFlowStep = serde_json::from_value(serde_json::json!({
            "task_id": null, "task_pr_id": null, "invocation_id": "process_old",
            "flow": "review", "step": "review-design", "iteration": 3, "step_index": 1
        }))
        .unwrap();
        assert_eq!((old.node, old.key, old.iterations), (None, None, None));
        assert_eq!(old.invocation_id, "process_old");
    }

    #[test]
    fn human_sessions_open_through_the_public_session_command() {
        let _lock = crate::journal::test_env_lock();
        let home = SessionHome::new();
        let argv = human_open_argv("session_123").unwrap();

        assert_eq!(
            &argv[argv.len() - 3..],
            ["session", "connect", "session_123"]
        );
        assert!(argv.contains(&format!("LF_HOME={}", home.home.path().display())));
        assert!(!argv.iter().any(|argument| argument == "tmux"));
    }

    #[test]
    fn initial_session_publication_requires_history_and_a_live_agent() {
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        let capture = crate::session_record::CaptureHandle::begin_at(
            home.path(),
            crate::session_record::SessionCaptureSpec {
                harness: "sleep".into(),
                model: None,
                surface: "tui".into(),
                cwd: home.path().into(),
                repo: None,
                worktree: None,
                skill: Some("review-design".into()),
                subjects: Vec::new(),
                flow: crate::session_record::SessionFlowMembership::Independent,
                work: None,
            },
        )
        .unwrap();
        let dir = capture.artifact_dir();
        let store =
            crate::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
        assert!(!session_is_resumable(&store, &dir).unwrap());
        crate::session_record::write_provider_session(&dir, &"provider-session".into(), None)
            .unwrap();
        assert!(!session_is_resumable(&store, &dir).unwrap());
        let mut client = std::process::Command::new("/bin/sleep")
            .arg("60")
            .spawn()
            .unwrap();
        let session = store
            .session_for_artifact(&capture.artifact_key())
            .unwrap()
            .unwrap();
        let attachment = store
            .claim_session_attachment(&session.id, None, &crate::id::LfProcessId::new(), true)
            .unwrap();
        let publication = store.record_agent_process_identity(
            &session.id,
            &attachment,
            client.id(),
            crate::journal::process_started_at(client.id())
                .unwrap()
                .unwrap(),
        );
        let resumable = publication
            .map_err(anyhow::Error::from)
            .and_then(|()| session_is_resumable(&store, &dir));
        client.kill().unwrap();
        client.wait().unwrap();
        assert!(resumable.unwrap());
    }

    #[test]
    fn membership_wire_ids_are_derived_from_their_captures() {
        use crate::flow::graph::FlowGraph;
        use crate::flow::{ConcretePath, ConcreteSkill, ConcreteStep, ConcreteXor, Skill};
        fn skill(name: &str, human: bool) -> ConcreteStep {
            ConcreteStep::Skill(ConcreteSkill {
                skill: Skill::named(name),

                human,
                id: None,
                returns: None,
                sources: Vec::new(),
            })
        }
        let opening = || vec![skill("kickoff", false), skill("review-design", true)];
        let mut past = opening();
        past.push(skill("implement", false));
        let mut current = past.clone();
        current.push(skill("compress", false));
        current.push(ConcreteStep::Xor(ConcreteXor {
            router: Skill::named("route"),
            sources: Vec::new(),
            paths: std::collections::HashMap::from([
                (
                    "fix".into(),
                    ConcretePath {
                        description: "Repair".into(),
                        steps: vec![skill("patch", false), skill("implement", false)],
                    },
                ),
                (
                    "skip".into(),
                    ConcretePath {
                        description: "Skip".into(),
                        steps: vec![],
                    },
                ),
            ]),
        }));
        current.extend([
            skill("loop-or-next", false),
            skill("pr-publish", false),
            skill("demo", true),
        ]);
        let expected = std::collections::BTreeMap::from([
            (
                "00000000-0000-0000-0000-00000000f10w".to_string(),
                FlowGraph::new("task-design", &opening()),
            ),
            (
                "00000000-0000-0000-0000-0000000000f1".to_string(),
                FlowGraph::new("feature", &past),
            ),
            (
                "00000000-0000-0000-0000-0000000000f2".to_string(),
                FlowGraph::new("feature", &current),
            ),
        ]);
        let graphs: std::collections::BTreeMap<String, FlowGraph> = serde_json::from_str(
            include_str!("../../../../tests/fixtures/dto/session_membership_graphs.json"),
        )
        .unwrap();
        assert_eq!(graphs, expected);
        let sessions: Vec<super::SessionRecord> = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/session_memberships.json"
        ))
        .unwrap();
        for session in sessions {
            if let super::SessionFlowMembership::Step {
                flow_lf_process_id,
                step,
                node: Some(node),
                ..
            } = session.flow_membership
            {
                assert_eq!(
                    graphs[&flow_lf_process_id].node_at(node).unwrap().label,
                    step
                );
            }
        }
    }
}

#[cfg(test)]
mod binding_preview_tests {
    use super::SessionBindingPreview;

    #[test]
    fn binding_preview_requires_exact_identity_and_label() {
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/session_binding_preview.json"
        ))
        .unwrap();
        let preview: SessionBindingPreview = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(preview).unwrap(), value);
        for key in ["session_id", "task_id", "identifier", "title"] {
            let mut missing = value.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(serde_json::from_value::<SessionBindingPreview>(missing).is_err());
        }
    }
}

#[cfg(test)]
mod page_tests {
    use super::SessionPage;

    #[test]
    fn session_page_preserves_the_continuation_and_required_entries() {
        let json = include_str!("../../../../tests/fixtures/dto/session_page.json");
        let page: SessionPage = serde_json::from_str(json).unwrap();
        assert_eq!(
            serde_json::to_value(page).unwrap(),
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
        assert!(serde_json::from_str::<SessionPage>(r#"{"next":null}"#).is_err());
    }
}
