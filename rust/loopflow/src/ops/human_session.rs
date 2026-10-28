pub(crate) use primary::admit_workspace;
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

pub(crate) mod conversation_log;
pub(crate) mod moves;
pub(crate) mod primary;
pub(crate) mod restore;
pub(crate) use primary::ensure_scope_worktree;
pub(crate) mod provider_conversation;

pub(crate) const HUMAN_SESSION_ENV: &str = "LF_HUMAN_SESSION";
pub(crate) const PREPARED_CAPTURE_ENV: &str = "LF_PREPARED_CAPTURE";
/// Launch exclusion is process state beside the Machine, not a Session record.
const LAUNCH_LOCK_DIRECTORY: &str = "human-sessions";
const SESSION_START_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum HumanSessionToken {
    /// A conversation; no caller or Graph waits on it.
    Primary { id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenMode {
    Refuse,
    Takeover,
    Try,
}

impl OpenMode {
    pub(crate) fn from_flags(takeover: bool, try_open: bool) -> Self {
        if takeover {
            Self::Takeover
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

/// An exact conversation or the scope whose primary conversation should open.
#[derive(Debug, Clone, Copy)]
pub enum SessionTarget<'a> {
    Context,
    Repository,
    Session(&'a str),
    Task(&'a str),
    Wave(&'a str),
}

impl<'a> SessionTarget<'a> {
    pub fn from_selectors(
        id: Option<&'a str>,
        task: Option<&'a str>,
        wave: Option<&'a str>,
    ) -> Result<Self> {
        match (id, task, wave) {
            (None, None, None) => Ok(Self::Context),
            (Some(id), None, None) => Ok(Self::Session(id)),
            (None, Some(task), None) => Ok(Self::Task(task)),
            (None, None, Some(wave)) => Ok(Self::Wave(wave)),
            _ => bail!("select one Session, Task or Wave target"),
        }
    }
}

/// A Task identifier where a Session id belongs names that Task's ongoing
/// conversation, as `--task` does. An exact Session id wins.
pub(super) async fn resolve_target<'a>(
    store: &SharedStore,
    target: SessionTarget<'a>,
) -> Result<SessionTarget<'a>> {
    let SessionTarget::Session(id) = target else {
        return Ok(target);
    };
    if store.session(id).await?.is_some() {
        return Ok(target);
    }
    Ok(match store.sqlite.resolve_task_id(id, None)? {
        Some(_) => SessionTarget::Task(id),
        None => target,
    })
}

/// Resolve physical checkout ownership without admitting a conversation.
pub(crate) async fn connection_scope(
    store: &SharedStore,
    cwd: &Path,
    target: SessionTarget<'_>,
) -> Result<crate::session::PrimaryScope> {
    match checkout_scope(store, cwd, target).await? {
        Some(scope) => Ok(scope),
        None => Err(unassociated_checkout(cwd)?),
    }
}

/// The conversation last opened in a linked checkout no Task, Wave or
/// repository owns. Such a checkout has no primary to resolve or create.
pub(super) async fn checkout_conversation(
    store: &SharedStore,
    cwd: &Path,
) -> Result<Option<String>> {
    let root = crate::git::worktree_root(cwd)?;
    Ok(store
        .sqlite
        .checkout_conversation(&root)?
        .map(|session| session.id.into()))
}

pub(super) fn unassociated_checkout(cwd: &Path) -> Result<anyhow::Error> {
    let path = crate::repository::CanonicalRepo::discover(cwd)?.to_string();
    let command = shlex::try_join(["lf", "--repo", path.as_str(), "connect"])?;
    Ok(anyhow!("This worktree has no associated work conversation.\n  lf                         Start a new conversation in this worktree\n  {command}  Open the repository's ongoing conversation\n  lf connect SESSION         Continue a particular conversation"))
}

/// The scope that owns this checkout, or `None` for an unassociated linked one.
pub(super) async fn checkout_scope(
    store: &SharedStore,
    cwd: &Path,
    target: SessionTarget<'_>,
) -> Result<Option<crate::session::PrimaryScope>> {
    use crate::session::PrimaryScope;
    let selection = match target {
        SessionTarget::Session(_) => bail!("an exact Session has no selected scope"),
        SessionTarget::Task(task) => Some(crate::ops::WorkSelection {
            task: Some(task),
            wave: None,
        }),
        SessionTarget::Wave(wave) => Some(crate::ops::WorkSelection {
            task: None,
            wave: Some(wave),
        }),
        SessionTarget::Context | SessionTarget::Repository => None,
    };
    if let Some(selection) = selection {
        let work = crate::ops::run::select_work(store, cwd, selection)
            .await?
            .context("selected Work is unavailable")?;
        return Ok(Some(match work.task {
            Some(task) => PrimaryScope::Task(task.id),
            None => PrimaryScope::Wave(
                work.wave
                    .expect("selection without a Task has a Wave")
                    .id()
                    .clone(),
            ),
        }));
    }
    let repo = crate::repository::CanonicalRepo::discover(cwd)?;
    if matches!(target, SessionTarget::Repository) {
        return Ok(Some(PrimaryScope::Repository(repo)));
    }
    let root = crate::git::worktree_root(cwd)?;
    if let Some(task) = crate::ops::task::task_for_checkout(store, &root).await? {
        return Ok(Some(PrimaryScope::Task(task.id)));
    }
    if root == repo.as_path() {
        return Ok(Some(PrimaryScope::Repository(repo)));
    }
    if crate::git::worktrees::is_persistent_worktree(&root)? {
        let matches = |scope: &PrimaryScope| -> Result<bool> {
            let plan = primary::scope_worktree(repo.as_path(), scope)?;
            Ok(
                plan.strategy == crate::git::worktrees::PlacementStrategy::UseExistingWorktree
                    && plan.worktree_path.canonicalize().ok().as_ref() == Some(&root),
            )
        };
        if matches(&PrimaryScope::Repository(repo.clone()))? {
            return Ok(Some(PrimaryScope::Repository(repo)));
        }
        for wave in store.list_waves(Some(&repo.to_string())).await? {
            if matches(&PrimaryScope::Wave(wave.id().clone()))? {
                return Ok(Some(PrimaryScope::Wave(wave.id().clone())));
            }
        }
    }
    Ok(None)
}

pub(crate) async fn select_connection(
    store: &SharedStore,
    cwd: &Path,
    target: SessionTarget<'_>,
    fresh: bool,
) -> Result<String> {
    use crate::session::PrimaryScope;
    let target = resolve_target(store, target).await?;
    if let SessionTarget::Session(id) = target {
        return Ok(id.to_owned());
    }
    let Some(scope) = checkout_scope(store, cwd, target).await? else {
        if !fresh {
            if let Some(id) = checkout_conversation(store, cwd).await? {
                return Ok(id);
            }
        }
        return Err(unassociated_checkout(cwd)?);
    };
    let session = match scope {
        PrimaryScope::Repository(repo) => {
            primary::ensure(store, repo.as_path(), None, fresh).await?
        }
        PrimaryScope::Wave(wave) => primary::ensure(store, cwd, Some(wave.as_str()), fresh).await?,
        PrimaryScope::Task(task) => {
            primary::ensure_task(store, cwd, task.as_str(), fresh, None).await?
        }
    };
    Ok(session.id.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Passive metadata has no trustworthy live/closed observation.
    Unknown,
    Active,
    /// Its agent runs, but no terminal shows it.
    Detached,
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
    let (label, help) = match state {
        SessionState::Detached => ("Reconnect", "Show the running Session in this terminal"),
        _ => ("Open here", "Open this Session in a terminal"),
    };
    let mut actions = vec![(
        Open,
        label,
        help,
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
pub struct SessionPage<T = SessionRecord> {
    pub entries: Vec<T>,
    pub next: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub preview: Option<crate::session::SessionPreview>,
    pub primary_scope: Option<String>,
    /// Available for a person, judged from current stream or terminal reports; absent when it is
    /// working or nothing current says.
    pub availability: Option<SessionAvailability>,
    pub program_status: Option<crate::program_status::Records>,
    /// The provider's current AgentProcess; absent when Loopflow never launched one.
    pub agent_process_id: Option<crate::id::LfProcessId>,
    /// Its Task names it as the Task's primary conversation.
    pub task_primary: bool,
    pub task_ids: Vec<crate::durable::TaskId>,
    pub id: crate::id::LfSessionId,
    pub workspace: Option<SessionWorkspace>,
    pub interactive: bool,
    pub work: Option<WorkRef>,
    pub wave_id: Option<crate::id::WaveId>,
    pub work_path: Option<String>,
    pub actions: Vec<SessionAction>,
    pub title: String,
    pub title_source: SessionTitleSource,
    pub graph_membership: SessionGraphMembership,
    pub detail: String,
    /// The provider its current input launched with; absent until a review's
    /// first launch records one.
    pub provider: Option<String>,
    /// The login its conversation is pinned to; absent for a shared or
    /// unrouted conversation.
    pub account: Option<crate::store::ProviderAccountId>,
    pub cwd: String,
    pub state: SessionState,
    pub ready_summary: Option<String>,
    pub open_argv: Vec<String>,
    pub terminal_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionAvailability {
    Available,
}

fn session_state(session: &crate::session::SessionSummary) -> SessionState {
    match session.agent_process_evidence {
        crate::journal::ProcessIdentityEvidence::Live
            if session.interactive && session.unviewed =>
        {
            SessionState::Detached
        }
        crate::journal::ProcessIdentityEvidence::Live => SessionState::Active,
        crate::journal::ProcessIdentityEvidence::Dead => SessionState::Closed,
        crate::journal::ProcessIdentityEvidence::Unknown => SessionState::Unknown,
    }
}

fn session_availability(session: &crate::session::SessionSummary) -> Option<SessionAvailability> {
    session.available.then_some(SessionAvailability::Available)
}

/// Whether a Session's conversation is an occurrence of a Graph. Membership
/// comes only from its recorded Graph invocation, never from matching Task, checkout,
/// provider, or skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionGraphMembership {
    Step {
        graph: String,
        #[serde(deserialize_with = "crate::id::deserialize_uuid")]
        graph_lf_process_id: String,
        step: String,
        /// Exact graph occurrence, unavailable for older capture manifests.
        node: Option<u32>,
        iterations: Option<Vec<Vec<u32>>>,
        occurrence: SessionGraphOccurrence,
    },
    Independent,
    Unknown {
        reason: String,
    },
}

/// Where a Graph occurrence sits relative to its Graph's current position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionGraphOccurrence {
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
    graph: crate::session_record::SessionGraphMembership,
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
            graph,
            work: None,
        },
        session.caller_artifact_key.clone(),
        session.input_key()?.clone(),
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

/// Where a Session's step stands in its Graph: the last one a still-open Graph
/// process launched, an earlier one, or part of a Graph whose process has exited.
fn graph_occurrence(
    state: crate::session::GraphProcessSummaryState,
    latest_step: bool,
) -> SessionGraphOccurrence {
    match (state, latest_step) {
        (crate::session::GraphProcessSummaryState::Current, true) => {
            SessionGraphOccurrence::Current
        }
        (crate::session::GraphProcessSummaryState::Current, false) => {
            SessionGraphOccurrence::Earlier
        }
        _ => SessionGraphOccurrence::Past,
    }
}

/// One Session as `list` reads it; `None` once it is gone.
pub(crate) fn listed(store: &SharedStore, id: &str) -> Result<Option<SessionRecord>> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    Ok(store
        .sqlite
        .session_summary(id, now)?
        .map(|session| summary_surface(&session)))
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
    let graph_membership = match (&session.graph_lf_process_id, &session.graph) {
        (None, _) if session.independent => SessionGraphMembership::Independent,
        (None, _) => SessionGraphMembership::Unknown {
            reason: "Graph membership was not recorded".into(),
        },
        (Some(id), Some(graph)) => SessionGraphMembership::Step {
            graph: graph.name.clone(),
            graph_lf_process_id: id.clone(),
            step: session.skill.clone().unwrap_or_default(),
            node: session.node,
            iterations: session.iterations.clone(),
            occurrence: graph_occurrence(graph.state, session.graph_step_latest),
        },
        (Some(id), _) => SessionGraphMembership::Unknown {
            reason: format!("Graph {id} metadata is unavailable"),
        },
    };
    let state = session_state(session);
    let mut actions = session_actions(state);
    let open_argv = match human_session_argv(&session.id, "connect") {
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
        preview: session.preview.clone(),
        primary_scope: session.primary_scope.clone(),
        availability: session_availability(session),
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
        graph_membership,
        detail: session
            .model
            .as_ref()
            .map_or(provider.clone(), |model| format!("{provider}:{model}")),
        provider: session.provider.clone(),
        account: session.account.clone(),
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
            dir: local_capture_dir(session.input_key()?).ok_or_else(|| {
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
        id: session.id.to_string(),
    }
}

/// Launch the prepared input of a conversation or of a saved Graph's review.
async fn serve_locked(store: &SharedStore, session: &LfSession, launch_lock: File) -> Result<()> {
    let lf = crate::os_process::resolve_lf_binary()?;
    let mut command = tokio::process::Command::new(lf);
    let token = session_token(session);
    command
        .current_dir(&session.cwd)
        .env(HUMAN_SESSION_ENV, serde_json::to_string(&token)?);
    command.args(conversation_launch_args(store, session).await?);
    // The launch runs inside this execution's terminal owner, not a nested one.
    crate::transport::configure_child(command.as_std_mut());
    let _output = crate::terminal_output::TerminalOutput::for_child(command.as_std_mut())?;
    let mut child = spawn_session_process(&mut command, session.input_key()?).await?;
    drop(launch_lock);
    // Provider termination never completes a Session.
    let status = child.wait().await.context("wait for session agent")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("session agent exited with {status}"))
    }
}

async fn conversation_launch_args(store: &SharedStore, session: &LfSession) -> Result<Vec<String>> {
    let config = crate::agent::resolve_session_config(session, &crate::lf::Cli::default())?;
    let mut args = vec![
        "-i".to_string(),
        "--agent".to_string(),
        config.agent().to_owned(),
        "--cwd".to_string(),
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
    Ok(args)
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
    crate::agent_input_timing::record("connect_begin", session_id);
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
    let provider_session = match session.artifact_key.as_deref() {
        Some(key) => store.sqlite.input_provider_session(key)?,
        None => None,
    };
    if provider_session.is_some() {
        if let Some(provider) = session.provider.as_deref() {
            if let crate::config::Resume::Import { agent } = crate::config::resume_choice(
                provider,
                session.model.as_deref(),
                crate::config::agent_override().as_deref(),
            ) {
                if !resume {
                    conversation_log::read(&store.sqlite, session)?;
                } else {
                    mode.require_action(surface(store, session).await?.state)?;
                    drop(progress);
                    import(store, &session.id, &agent).await?;
                    let session = store
                        .session(&session.id)
                        .await?
                        .ok_or_else(|| session_not_found(&session.id))?;
                    return surface(store, &session).await;
                }
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
    drop(progress);
    // Naming another login moves the conversation: its agent on the old login
    // stops, and it relaunches here rather than attaching to that one.
    let moving = match (&provider_session, session.provider.as_deref()) {
        (Some(native), Some(provider)) if resume => match provider.parse() {
            Ok(provider) => {
                crate::provider_account::names_another_account(
                    store,
                    provider,
                    &native.agent_session,
                )
                .await?
            }
            Err(_) => None,
        },
        _ => None,
    };
    // An agent that already exited, whose identity was never recorded, has
    // nothing to stop.
    let running = || -> Result<bool> {
        Ok(store
            .sqlite
            .session_attachment(&session.id)?
            .is_some_and(|attachment| {
                crate::journal::process_evidence(&store.sqlite, &attachment.agent_process_id)
                    == crate::journal::ProcessIdentityEvidence::Live
            }))
    };
    if let Some(account) = &moving {
        if running()? {
            crate::harness::agent_process::stop_session_agent(
                &store.sqlite,
                &session.id,
                crate::process::Departure::Account {
                    account: account.clone(),
                },
            )?;
        }
    }
    if resume {
        if session.interactive && moving.is_none() {
            if let Some(owner) = store.sqlite.session_transport(&session.id)? {
                if crate::transport::current() != Some(&owner)
                    && owner_running(&store.sqlite, &owner)?
                {
                    // This viewer takes control, so the conversation is now
                    // shown where it runs, not where it was launched.
                    if let Some(attachment) = store.sqlite.session_attachment(&session.id)? {
                        store.sqlite.record_session_terminal(
                            &session.id,
                            &attachment,
                            crate::session_record::current_terminal_id()
                                .or_else(crate::lf::commands::open::host_placement)
                                .as_deref(),
                        )?;
                    }
                    crate::transport::attach(&owner, false, true)?;
                    return surface(store, session).await;
                }
            }
        }
        if crate::transport::delegate_current(false)? {
            return surface(store, session).await;
        }
    }
    if session.captured.is_none() {
        if resume {
            open_waiting(store, &session.id).await?;
        }
        return surface(store, session).await;
    }
    let native = NativeSession::of(session)?;
    if resume {
        crate::lf::commands::util::require_provider_session_process(&native.dir)?;
        transition_headless_to_native(store, session, provider_session.as_ref()).await?;
    }
    let Some(provider_session) = provider_session else {
        let surface = surface(store, session).await?;
        if resume {
            open_waiting(store, &session.id).await?;
        }
        return Ok(surface);
    };
    if resume && mode == OpenMode::Takeover {
        crate::harness::agent_process::stop_session_agent(
            &store.sqlite,
            &session.id,
            crate::process::Departure::Terminal,
        )?;
    }
    let mut result = surface(store, session).await?;
    if resume {
        mode.require_action(result.state)?;
        crate::lf::commands::util::resume_session(session.input_key()?, &provider_session)?;
    } else {
        match mode {
            OpenMode::Takeover => result.open_argv.push("--takeover".to_string()),
            OpenMode::Try => result.open_argv.push("--try".to_string()),
            OpenMode::Refuse => {}
        }
    }
    Ok(result)
}

/// Explicit resume retires a structured Run before opening its native history.
/// Require native identity before stopping useful work; viewer attachment never enters here.
async fn transition_headless_to_native(
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
    // A private server can die before creating a native conversation. Retire
    // its stale attachment so the prepared input can start normally.
    if crate::journal::process_evidence(&store.sqlite, &agent.process.id)
        == crate::journal::ProcessIdentityEvidence::Dead
    {
        anyhow::ensure!(
            store.sqlite.settle_agent_process(&agent, || {
                if crate::journal::process_evidence(&store.sqlite, &agent.process.id)
                    != crate::journal::ProcessIdentityEvidence::Dead
                {
                    return Err(crate::store::StoreError::InvalidAuthority(
                        "AgentProcess death changed".into(),
                    ));
                }
                Ok(())
            })?,
            "Session attachment changed before observing death"
        );
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
        sqlite.finish_session_attachment(&id, &attachment, "interrupted", None, || {
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

/// Open a conversation: resume its native history, else
/// launch its prepared input, else append another attempt to the Session.
async fn open_waiting(store: &SharedStore, id: &str) -> Result<()> {
    crate::agent_input_timing::record("launch_lock_begin", id);
    let lock_id = id.to_string();
    let launch_lock = tokio::task::spawn_blocking(move || lock_session_process(&lock_id)).await??;
    crate::agent_input_timing::record("launch_lock_acquired", id);
    // Resolve the current input under the launch lock, including completion
    // or replacement that happened while this opener waited.
    let session = store
        .session(id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    let session = primary::admit_workspace(store, session).await?;
    let mut launch_lock = Some(launch_lock);
    let session = if session.captured.is_none() {
        append_input(store, session).await?
    } else {
        session
    };
    let token = session_token(&session);
    if resume_native_session(store, session.input_key()?, &token, &mut launch_lock)? {
        return Ok(());
    }
    crate::agent_input_timing::record("fresh_launch_selected", id);
    // A consumed launch without native history gets another attempt under
    // the same Session; its title and feedback never leave the row.
    let session = if capture_is_prepared(session.input_key()?)? {
        session
    } else {
        if session.input_published {
            append_input(store, session).await?
        } else {
            publish_prepared_input(
                &store.sqlite,
                &session,
                crate::session_record::SessionGraphMembership::Independent,
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
    crate::harness::agent_process::stop_session_agent(
        &store.sqlite,
        &session.id,
        crate::process::Departure::Terminal,
    )?;
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
pub(crate) async fn append_input(store: &SharedStore, mut next: LfSession) -> Result<LfSession> {
    let replaced = next.captured;
    next.artifact_key = Some(crate::session_record::new_artifact_key());
    next.input_published = false;
    let next = store.replace_session_input(replaced, next).await?;
    publish_prepared_input(
        &store.sqlite,
        &next,
        crate::session_record::SessionGraphMembership::Independent,
    )
}

pub(crate) fn capture_is_prepared(run_id: &str) -> Result<bool> {
    match crate::session_record::resolve_manifest(&crate::store::lf_home_dir(), run_id) {
        Ok((dir, _)) => Ok(dir.join("prepared").is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).context("resolve prepared Session input"),
    }
}

/// Read an existing execution without preparing input or resuming provider history.
/// None when nothing is running to attach to; only an explicit resume reopens history.
pub(crate) async fn prepare_attachment(
    store: &SharedStore,
    session: &LfSession,
    observe: bool,
    if_free: bool,
) -> Result<Option<SessionRecord>> {
    crate::data_clone::require_writable_session(&session.id)?;
    let Some(owner) = store.sqlite.session_transport(&session.id)? else {
        return Ok(None);
    };
    if !owner_running(&store.sqlite, &owner)? {
        return Ok(None);
    }
    let mut record = surface(store, session).await?;
    record.open_argv = human_session_argv(&session.id, "attach")?;
    if observe {
        record.open_argv.push("--observe".into());
    }
    if if_free {
        record.open_argv.push("--if-free".into());
    }
    Ok(Some(record))
}

/// An execution owner runs until it records its exit or the OS shows it gone,
/// as after a restart. Unknown evidence is not exit.
pub(crate) fn owner_running(store: &SqliteStore, owner: &crate::id::LfProcessId) -> Result<bool> {
    Ok(store
        .process(owner)?
        .is_some_and(|process| process.completed_at.is_none())
        && crate::journal::process_evidence(store, owner)
            != crate::journal::ProcessIdentityEvidence::Dead)
}

/// A Session as its row and its current input describe it.
pub(crate) async fn surface(store: &SharedStore, session: &LfSession) -> Result<SessionRecord> {
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
        human_session_argv(&session.id, "connect")?
    };
    let graph_membership = match (&session.graph_lf_process_id, &metadata.graph) {
        (None, _) if metadata.independent => SessionGraphMembership::Independent,
        (None, _) => SessionGraphMembership::Unknown {
            reason: "Graph membership was not recorded".into(),
        },
        (Some(id), Some(graph)) => SessionGraphMembership::Step {
            graph: graph.name.clone(),
            graph_lf_process_id: id.clone(),
            step: session.skill.clone().unwrap_or_default(),
            node: session.node,
            iterations: session.iterations.clone(),
            occurrence: graph_occurrence(graph.state, metadata.graph_step_latest),
        },
        (Some(id), None) => SessionGraphMembership::Unknown {
            reason: format!("Graph {id} is unavailable"),
        },
    };
    let mut reading = SessionRecord {
        preview: metadata.preview.clone(),
        primary_scope: metadata.primary_scope.clone(),
        availability: session_availability(&metadata),
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
        graph_membership,
        detail: launch_model(session),
        provider: session.provider.clone(),
        account: metadata.account.clone(),
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
    pub session_id: crate::id::LfSessionId,
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
        session_id: session.id.clone(),
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
    // never start; the opening command's error must carry enough evidence to
    // diagnose it. Arguments and environment values can contain prompts or credentials.
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
    // This child owns the visible foreground terminal. First-use trust and
    // authentication can wait for a person before native history exists; an
    // elapsed startup deadline must not kill that conversation. Keep the launch
    // lock until history is resumable or the child exits.
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
    Ok(summary.agent_process_evidence == crate::journal::ProcessIdentityEvidence::Live)
}

pub(crate) fn resume_native_session(
    store: &SharedStore,
    artifact_key: &String,
    token: &HumanSessionToken,
    launch_lock: &mut Option<File>,
) -> Result<bool> {
    let Some(session) = store.sqlite.session_for_artifact(artifact_key)? else {
        return Ok(false);
    };
    crate::data_clone::require_writable_session(&session.id)?;
    if session.artifact_key.as_ref() != Some(artifact_key) {
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
    crate::harness::agent_process::stop_session_agent(
        &store.sqlite,
        &session.id,
        crate::process::Departure::Terminal,
    )?;
    let environment =
        BTreeMap::from([(HUMAN_SESSION_ENV.to_string(), serde_json::to_string(token)?)]);
    crate::agent_input_timing::record("native_resume_selected", &session.id);
    crate::lf::commands::util::resume_session_with_env(
        session.input_key()?,
        provider_session.as_ref(),
        &environment,
        launch_lock.take(),
    )?;
    Ok(true)
}

pub(crate) fn human_session_argv(id: &str, command: &str) -> Result<Vec<String>> {
    crate::data_clone::require_writable_session(id)?;
    let context = crate::os_process::execution_context()?;
    // A fresh terminal does not inherit the listing process's data selection.
    // Carry its Machine and stable entry so opening picks up the current install.
    Ok(vec![
        "/usr/bin/env".to_string(),
        format!("LF_BIN={}", context.lf_bin.display()),
        format!("LF_HOME={}", context.lf_home.display()),
        context.lf_bin.display().to_string(),
        "session".to_string(),
        command.to_string(),
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
mod tests;
