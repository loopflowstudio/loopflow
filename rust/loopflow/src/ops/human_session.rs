pub(crate) use primary::admit_workspace;
mod resume_explanation;
pub(crate) use resume_explanation::explain_resume;
pub use resume_explanation::{SessionResumeAction, SessionResumeExplanation, SessionResumeIntent};

mod turns;
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
use crate::lf::commands::write::SessionOpenIntent;
use crate::provider_account::LaunchCapacity;
use crate::session::{LfSession, WorkSource};
use crate::session_record::SessionTitleSource;
use crate::store::sqlite::SqliteStore;
use crate::store::SharedStore;
use crate::work::task::Task;

pub(crate) mod conversation_log;
pub(crate) mod moves;
pub(crate) mod moves_bulk;
pub(crate) mod primary;
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
    /// A Session id selects that conversation even beside `--task` or
    /// `--wave`; naming the Work there only makes the command managed.
    pub fn from_selectors(
        id: Option<&'a str>,
        task: Option<&'a str>,
        wave: Option<&'a str>,
    ) -> Result<Self> {
        match (id, task, wave) {
            (_, Some(_), Some(_)) => bail!("select one Task or Wave"),
            (Some(id), _, _) => Ok(Self::Session(id)),
            (None, Some(task), None) => Ok(Self::Task(task)),
            (None, None, Some(wave)) => Ok(Self::Wave(wave)),
            (None, None, None) => Ok(Self::Context),
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
    let command = shlex::try_join(["lf", "--repo", path.as_str(), "resume"])?;
    Ok(anyhow!("This worktree has no associated work conversation.\n  lf                         Start a new conversation in this worktree\n  {command}  Open the repository's ongoing conversation\n  lf resume SESSION          Continue a particular conversation"))
}

/// The scope that owns this checkout, or `None` for an unassociated linked one.
pub(crate) async fn checkout_scope(
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

/// The conversation `lf resume` selected, and whether selecting it started one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Selected {
    pub id: String,
    pub started: bool,
}

pub(crate) async fn select_connection(
    store: &SharedStore,
    cwd: &Path,
    target: SessionTarget<'_>,
    intent: SessionOpenIntent,
) -> Result<Selected> {
    use crate::session::PrimaryScope;
    let target = resolve_target(store, target).await?;
    let existing = |id: String| Selected { id, started: false };
    if let SessionTarget::Session(id) = target {
        return Ok(existing(id.to_owned()));
    }
    let Some(scope) = checkout_scope(store, cwd, target).await? else {
        if let Some(id) = checkout_conversation(store, cwd).await? {
            return Ok(existing(id));
        }
        return Err(unassociated_checkout(cwd)?);
    };
    if intent == SessionOpenIntent::Reveal {
        let session = match &scope {
            PrimaryScope::Task(task) => primary::task_conversation(store, task).await?,
            _ => store.sqlite.primary_session(&scope)?,
        }
        .context("No existing conversation; start one explicitly")?;
        return Ok(existing(session.id.into()));
    }
    let (session, started) = match scope {
        PrimaryScope::Repository(repo) => primary::ensure(store, repo.as_path(), None).await?,
        PrimaryScope::Wave(wave) => primary::ensure(store, cwd, Some(wave.as_str())).await?,
        PrimaryScope::Task(task) => primary::ensure_task(store, cwd, task.as_str(), None).await?,
    };
    Ok(Selected {
        id: session.id.into(),
        started,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
    Reveal,
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

const CONTROLLED_ELSEWHERE: &str =
    "This conversation is controlled elsewhere; use Take back to continue here";
const NO_PROVIDER_HISTORY: &str = "No saved provider history; start this conversation explicitly";
const LAUNCH_ENDED: &str =
    "The previous launch ended before the provider saved any history; Try again repeats its request";

/// A Run opens nowhere: `lf resume` takes only Sessions. A consumed first launch
/// has nothing to open: MoveHere, without Open, is the click that repeats it.
pub(crate) fn session_actions(
    state: SessionState,
    interactive: bool,
    has_provider_history: bool,
    occupied: bool,
    launch_consumed: bool,
) -> Vec<SessionAction> {
    use SessionActionKind::{MoveHere, Open, Reveal};
    if !interactive {
        return Vec::new();
    }
    let active_client = occupied && state != SessionState::Detached;
    let unstarted = !occupied && !has_provider_history;
    let (label, help) = match state {
        SessionState::Detached => ("Reconnect", "Show the running Session in this terminal"),
        _ if unstarted => ("Start conversation", NO_PROVIDER_HISTORY),
        _ => ("Open here", "Open this Session in a terminal"),
    };
    let mut actions = vec![if unstarted && launch_consumed {
        (MoveHere, "Try again", LAUNCH_ENDED, None)
    } else {
        (
            Open,
            label,
            help,
            active_client.then_some(
                "This Session is active in another terminal; use Take back to transfer it",
            ),
        )
    }];
    if active_client {
        actions.push((
            MoveHere,
            "Take back",
            "Transfer control here; unsent text in the previous terminal is not transferred",
            None,
        ));
    }
    actions.push((
        Reveal,
        "Show conversation",
        "Continue existing history without taking another terminal's input",
        if active_client {
            Some(CONTROLLED_ELSEWHERE)
        } else {
            unstarted.then_some(NO_PROVIDER_HISTORY)
        },
    ));
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

fn projected_actions(session: &crate::session::SessionSummary) -> Vec<SessionAction> {
    if session.interactive && session.held {
        return held_actions(session);
    }
    let occupied = session.agent_process_id.is_some()
        && session.agent_process_evidence != crate::journal::ProcessIdentityEvidence::Dead;
    // Listing reads the marker alone: `capture_is_prepared` opens the store per Session.
    let launch_consumed = session.interactive
        && session.captured.is_some()
        && !occupied
        && !session.has_provider_history
        && session
            .artifact_key
            .as_deref()
            .and_then(local_capture_dir)
            .is_some_and(|dir| !dir.join("prepared").is_file());
    let mut actions = session_actions(
        session_state(session),
        session.interactive,
        session.has_provider_history,
        occupied,
        launch_consumed,
    );
    if occupied {
        name_session_holder(&mut actions, session.terminal_id.as_deref());
    }
    actions
}

/// A held terminal takes any number of views, so opening one takes no other
/// terminal's input.
fn held_actions(session: &crate::session::SessionSummary) -> Vec<SessionAction> {
    vec![SessionAction {
        kind: SessionActionKind::Open,
        label: if session.unviewed {
            "Reconnect"
        } else {
            "Open here"
        }
        .to_string(),
        help: format!(
            "View the running conversation here: lf resume {}",
            session.id
        ),
        unavailable_reason: None,
    }]
}

fn name_session_holder(actions: &mut [SessionAction], terminal: Option<&str>) {
    if !actions
        .iter()
        .any(|action| action.kind == SessionActionKind::MoveHere)
    {
        return;
    }
    let holder = match terminal {
        Some(id) if id.starts_with("rex:") => format!("Rex ({})", &id[4..]),
        Some(id) if id.starts_with("cmux:") => format!("cmux ({})", &id[5..]),
        Some(_) => "another client on this Machine".to_string(),
        None => "another terminal on this Machine".to_string(),
    };
    for action in actions {
        match action.kind {
            SessionActionKind::Open => {
                action.unavailable_reason = Some(format!(
                    "This conversation is open in {holder}; Take back transfers input here"
                ))
            }
            SessionActionKind::MoveHere => {
                action.help =
                    format!("Continue here from {holder}; the previous terminal reports the move")
            }
            SessionActionKind::Reveal => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionPage<T = SessionRecord> {
    pub entries: Vec<T>,
    pub next: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionRecord {
    pub transition: Option<crate::store::sqlite::provider_transitions::ProviderTransition>,
    pub preview: Option<crate::session::SessionPreview>,
    pub primary_scope: Option<String>,
    /// Available for a person, judged from current stream or terminal reports; absent when it is
    /// working or nothing current says.
    pub availability: Option<SessionAvailability>,
    pub program_status: Option<crate::program_status::Records>,
    /// The provider's current AgentProcess; absent when Loopflow never launched one.
    pub agent_process_id: Option<crate::id::AgentProcessId>,
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
    /// The login its conversation last ran on; absent for an unrouted one.
    pub account: Option<crate::store::ProviderAccountId>,
    /// The model its current input launched with, when one was named.
    pub model: Option<String>,
    /// Its next message would fail: the login it would use cannot take its
    /// model now. `lf session move --blocked` moves these.
    pub blocked: bool,
    pub cwd: String,
    pub state: SessionState,
    pub ready_summary: Option<String>,
    pub open_argv: Vec<String>,
    pub terminal_ids: Vec<String>,
    /// Its current or last turn while it is live; `null` when nothing says.
    #[serde(deserialize_with = "Option::deserialize")]
    pub turn: Option<crate::transcript::live::SessionTurn>,
    /// A conversation open here that Loopflow never launched; `id` is the
    /// provider's conversation ID and no Session records it.
    pub raw: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionAvailability {
    Available,
}

pub(crate) fn session_state(session: &crate::session::SessionSummary) -> SessionState {
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

fn is_running(state: SessionState) -> bool {
    matches!(state, SessionState::Active | SessionState::Detached)
}

fn session_availability(session: &crate::session::SessionSummary) -> Option<SessionAvailability> {
    session.available.then_some(SessionAvailability::Available)
}

/// Whether a Session's conversation is an occurrence of a Graph. Membership
/// comes only from its recorded Graph invocation, never from matching Task, checkout,
/// provider, or skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
    crate::session_record::SessionRecorder::prepare(
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
    // prefixes cannot recognize symlink aliases or nested repositories. Neither
    // can they judge capacity.
    let association_filter = filter.orphan
        || filter.task.is_some()
        || filter.wave.is_some()
        || filter.blocked
        || filter.on.is_some();
    let mut selection = filter.clone();
    if association_filter {
        selection.task = None;
        selection.wave = None;
        selection.orphan = false;
        selection.blocked = false;
        selection.on = None;
        selection.limit = 0;
    }
    let on = match &filter.on {
        Some(raw) => Some(SessionOn::resolve(store, raw).await?),
        None => None,
    };
    let mut sessions = Vec::new();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let capacity = LaunchCapacity::read(&store.sqlite)?;
    for session in store.session_summaries(&selection, now).await? {
        sessions.push(summary_surface(&session, &capacity));
    }
    let mut workspaces = workspace::associate(store, &mut sessions).await?;
    turns::read(&store.sqlite, &mut sessions)?;
    let task = match &filter.task {
        Some(selector) => store.sqlite.resolve_task_id(selector, None)?,
        None => None,
    };
    let in_task = |session: &SessionRecord| {
        filter.task.is_none()
            || task
                .as_ref()
                .is_some_and(|task| session.task_ids.contains(task))
    };
    let wave_tasks = match &filter.wave {
        Some(wave) => store.sqlite.task_ids_in_wave(wave)?,
        None => Default::default(),
    };
    let mut in_work = |session: &SessionRecord| {
        (!filter.orphan || (session.task_ids.is_empty() && session.wave_id.is_none()))
            && in_task(session)
            && (filter.wave.is_none()
                || session.wave_id == filter.wave
                || session
                    .task_ids
                    .iter()
                    .any(|task| wave_tasks.contains(task))
                || workspaces.in_task_checkout(Path::new(&session.cwd), &wave_tasks))
    };
    if association_filter {
        sessions.retain(|session| {
            in_work(session)
                && (!filter.blocked || session.blocked)
                && on.as_ref().is_none_or(|on| on.selects(session, &capacity))
        });
        sessions = sessions
            .into_iter()
            .take(if filter.limit == 0 {
                usize::MAX
            } else {
                filter.limit
            })
            .collect();
    }
    if lists_raw(filter) {
        let raw = turns::raw(&store.sqlite, filter.repo.as_deref())?;
        sessions.extend(
            raw.into_iter()
                .filter(|raw| (!filter.available || raw.availability.is_some()) && in_work(raw)),
        );
    }
    Ok(sessions)
}

/// Raw conversations list unless `--no-raw` asks otherwise. They have no
/// account or page position, so a selection by any of those leaves
/// them out. One open in a Task's checkout lists under that Task.
fn lists_raw(filter: &crate::session::SessionFilter) -> bool {
    filter.raw
        && filter.primary.is_none()
        && !filter.blocked
        && filter.on.is_none()
        && filter.search.is_none()
        && filter.after.is_none()
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

/// One conversation as `list` reads it: the Session with this ID, else the
/// open raw conversation whose provider ID it is. `None` once it is gone.
pub(crate) fn listed(
    store: &SharedStore,
    id: &str,
    capacity: &LaunchCapacity,
) -> Result<Option<SessionRecord>> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let Some(mut session) = store
        .sqlite
        .session_summary(id, now)?
        .map(|session| summary_surface(&session, capacity))
    else {
        return Ok(turns::raw(&store.sqlite, None)?
            .into_iter()
            .find(|raw| raw.id.as_str() == id));
    };
    turns::read(&store.sqlite, std::slice::from_mut(&mut session))?;
    Ok(Some(session))
}

/// What `--on <account|harness[:model]>` names, resolved once before filtering.
enum SessionOn {
    /// A bare harness selects every model on it.
    Agent {
        harness: crate::harness::HarnessKind,
        model: Option<String>,
    },
    Login {
        provider: String,
        account_id: crate::store::ProviderAccountId,
    },
}

impl SessionOn {
    async fn resolve(store: &SharedStore, raw: &str) -> Result<Self> {
        let (harness, model) = crate::config::parse_agent(raw);
        match crate::harness::HarnessKind::parse(&harness) {
            Some(harness) if !raw.contains('@') => Ok(Self::Agent { harness, model }),
            _ => {
                let login = crate::lf::commands::account::resolve_on(store, raw).await?;
                Ok(Self::Login {
                    provider: login.provider,
                    account_id: login.account_id,
                })
            }
        }
    }

    fn selects(&self, session: &SessionRecord, capacity: &LaunchCapacity) -> bool {
        let Some(provider) = session.provider.as_deref() else {
            return false;
        };
        match self {
            Self::Agent { harness, model } => {
                provider == harness.as_str() && (model.is_none() || *model == session.model)
            }
            Self::Login {
                provider: selected,
                account_id,
            } => capacity
                .login(
                    provider,
                    session.account.as_ref(),
                    is_running(session.state),
                )
                .is_some_and(|login| {
                    login.provider == *selected && login.account_id == *account_id
                }),
        }
    }
}

fn is_blocked(
    capacity: &LaunchCapacity,
    provider: Option<&str>,
    account: Option<&crate::store::ProviderAccountId>,
    state: SessionState,
    model: Option<&str>,
) -> bool {
    provider
        .and_then(|provider| capacity.login(provider, account, is_running(state)))
        .is_some_and(|login| capacity.blocks(login, model))
}

/// Passive listing reads record metadata and exact local client receipts only.
/// Connect/complete still enter find_session and surface, with full validation.
fn summary_surface(
    session: &crate::session::SessionSummary,
    capacity: &LaunchCapacity,
) -> SessionRecord {
    let work = match (&session.task_id, &session.wave_id) {
        (Some(task), _) => Some(WorkRef::Task(task.clone())),
        (None, Some(wave)) => Some(WorkRef::Wave(wave.clone())),
        _ => None,
    };
    let wave = session.wave_id.as_ref().map(|wave| {
        session
            .wave_name
            .clone()
            .unwrap_or_else(|| format!("Wave {wave} (unavailable)"))
    });
    let task = session.task_id.as_ref().map(|task| {
        session
            .task_identifier
            .clone()
            .unwrap_or_else(|| format!("Task {task} (unavailable)"))
    });
    let work_path = join_work_path(wave, task);
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
    let mut actions = projected_actions(session);
    let open_argv = match human_session_argv(&session.id) {
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
        transition: session.transition.clone(),
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
        model: session.model.clone(),
        blocked: is_blocked(
            capacity,
            session.provider.as_deref(),
            session.account.as_ref(),
            state,
            session.model.as_deref(),
        ),
        cwd: session.cwd.display().to_string(),
        state,
        ready_summary: session.ready_summary.clone(),
        open_argv,
        terminal_ids: session.terminal_id.clone().into_iter().collect(),
        turn: None,
        raw: false,
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
///
/// This process becomes the launcher: it execs `lf -i` rather than waiting on
/// it, so no connect sits beside the conversation. The Session launch lock
/// crosses the exec and is released once the provider is recorded.
async fn serve_locked(store: &SharedStore, session: &LfSession, launch_lock: File) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let lf = crate::os_process::resolve_lf_binary()?;
    let mut command = std::process::Command::new(lf);
    let token = session_token(session);
    command
        .current_dir(&session.cwd)
        .env(HUMAN_SESSION_ENV, serde_json::to_string(&token)?);
    command.args(conversation_launch_args(store, session).await?);
    let input = session.input_key()?;
    if capture_is_prepared(input)? {
        command.env(PREPARED_CAPTURE_ENV, input.as_str());
    }
    // The launch runs inside this execution's held terminal, not a nested one.
    crate::transport::carry_across_exec(&mut command);
    pass_launch_lock(&mut command, &launch_lock)?;
    crate::journal::finish_before_exec();
    let error = command.exec();
    Err(error).with_context(|| format!("could not start Session input {input}"))
}

/// Environment naming the inherited descriptor that holds a Session launch lock.
pub(crate) const LAUNCH_LOCK_ENV: &str = "LF_LAUNCH_LOCK";

fn pass_launch_lock(command: &mut std::process::Command, lock: &File) -> Result<()> {
    use std::os::fd::AsRawFd;
    let fd = lock.as_raw_fd();
    // SAFETY: fcntl on a live descriptor this process owns; clearing
    // close-on-exec lets the exec'd launcher inherit the lock.
    if unsafe { libc::fcntl(fd, libc::F_SETFD, 0) } < 0 {
        return Err(std::io::Error::last_os_error()).context("pass Session launch lock");
    }
    command.env(LAUNCH_LOCK_ENV, fd.to_string());
    Ok(())
}

/// The Session launch lock a connect passed across its exec, if any.
pub(crate) fn inherited_launch_lock() -> Option<File> {
    use std::os::fd::FromRawFd;
    let fd: i32 = std::env::var(LAUNCH_LOCK_ENV).ok()?.parse().ok()?;
    std::env::remove_var(LAUNCH_LOCK_ENV);
    // SAFETY: fcntl on the inherited descriptor; close-on-exec keeps it from
    // reaching the provider.
    if fd < 3 || unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return None;
    }
    // SAFETY: the descriptor was opened by the connect that exec'd into this
    // process and is referenced nowhere else here.
    Some(unsafe { File::from_raw_fd(fd) })
}

async fn conversation_launch_args(store: &SharedStore, session: &LfSession) -> Result<Vec<String>> {
    let config = crate::agent::resolve_session_config(session, &crate::lf::Cli::default())?;
    let mut args = vec![
        "-i".to_string(),
        "--agent".to_string(),
        config.agent()?.to_owned(),
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
    args.push(first_turn(session.request.as_deref()).to_string());
    Ok(args)
}

/// A stored request, the person's own or a continuation `new_thread` seeded, is
/// the first turn; without one, opening a primary asks for nothing, so its
/// first turn only reads. Desktop opens every primary on load this way.
fn first_turn(request: Option<&str>) -> &str {
    request.unwrap_or(PRIMARY_MESSAGE)
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
    if crate::journal::request::active() {
        return Ok(None);
    }
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

/// Get back to a Session. A terminal showing it hands over control; a provider
/// client lf started stops only when no live turn or request needs it.
/// With `fork`, the holder keeps it and a new Session continues from its log. A
/// Run is refused; it is never taken over or resumed.
pub(crate) async fn open(
    store: &SharedStore,
    directory: &Path,
    session_id: &str,
    fork: bool,
    resume: bool,
    json: bool,
    named_work: bool,
) -> Result<SessionRecord> {
    crate::agent_input_timing::record("connect_begin", session_id);
    let progress = resume.then(|| crate::startup::Startup::new("Preparing conversation"));
    let target = match find_session(store, session_id, true).await? {
        Some(target) => target,
        // Connecting is what brings a provider-started conversation in.
        None => provider_conversation::admit(store, session_id, directory)
            .await?
            .ok_or_else(|| session_not_found(session_id))?,
    };
    let session = &target;
    if let Some(refusal) = run_refusal(&store.sqlite, session)? {
        bail!(refusal);
    }
    if fork {
        drop(progress);
        return self::fork(store, session, resume, json).await;
    }
    if resume {
        end_earlier_owner(&store.sqlite, &session.id, progress.as_ref())?;
    }
    let provider_session = match session.artifact_key.as_deref() {
        Some(key) => store.sqlite.input_provider_session(key)?,
        None => None,
    };
    if let Some(agent) = provider_session.as_ref().and(handoff_agent(session)) {
        if !resume {
            conversation_log::read(&store.sqlite, session)?;
        } else {
            drop(progress);
            new_thread(
                store,
                &session.id,
                Some(&agent),
                true,
                json,
                &uuid::Uuid::new_v4().to_string(),
            )
            .await?;
            let session = store
                .session(&session.id)
                .await?
                .ok_or_else(|| session_not_found(&session.id))?;
            return surface(store, &session).await;
        }
    }
    let admitted;
    let session = if resume {
        let id = session.id.clone();
        let _launch =
            crate::journal::request::spawn_blocking(move || lock_session_process(&id)).await??;
        admitted = primary::admit_workspace(store, session.clone()).await?;
        &admitted
    } else {
        session
    };
    drop(progress);
    // A running Codex keeps the login it started with, so naming another one
    // stops it and relaunches here. A running Claude follows the Machine's
    // login, so that moves below and the resume attaches.
    let moving = match (&provider_session, session.provider.as_deref()) {
        (Some(native), Some("codex")) if resume => {
            let provider = crate::provider_auth::Provider::Codex;
            let ran_on = store
                .provider_session_account(provider, &native.agent_session)
                .await?;
            crate::provider_account::names_another_account(store, provider, ran_on).await?
        }
        _ => None,
    };
    // An agent that already exited, whose identity was never recorded, has
    // nothing to stop.
    let running = || -> Result<bool> {
        Ok(store
            .sqlite
            .session_attachment(&session.id)?
            .is_some_and(|attachment| {
                crate::journal::agent_evidence(&store.sqlite, &attachment.agent_process_id)
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
    if resume && session.provider.as_deref() == Some("claude") && running()? {
        crate::lf::commands::account::switch_to_named_login(
            store,
            crate::provider_auth::Provider::Claude,
        )
        .await?;
    }
    if resume {
        if session.interactive && moving.is_none() {
            // A held terminal is attached while its child runs, answering or
            // not: a stopped server never means a second agent for the Session.
            if let Some(terminal) = store.sqlite.session_terminal(&session.id)? {
                if crate::transport::current() != Some(&terminal) {
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
                    let moved = crate::lf::commands::util::departure_notice(
                        &store.sqlite,
                        &session.id,
                        departure_here(),
                    )?;
                    crate::transport::attach(
                        &terminal,
                        Some(&session.id),
                        Some(moved),
                        json.then_some(session.id.as_str()),
                    )?;
                    return surface(store, session).await;
                }
            }
        }
        if crate::transport::delegate_session(
            json.then_some(session.id.as_str()),
            session.interactive.then_some(session.id.as_str()),
            true,
            named_work,
        )? {
            return surface(store, session).await;
        }
    }
    if session.captured.is_none() {
        if resume {
            open_waiting(store, &session.id, named_work).await?;
        }
        return surface(store, session).await;
    }
    let native = NativeSession::of(session)?;
    if resume {
        crate::lf::commands::util::require_provider_session_process(&native.dir)?;
    }
    let Some(provider_session) = provider_session else {
        let surface = surface(store, session).await?;
        if resume {
            open_waiting(store, &session.id, named_work).await?;
        }
        return Ok(surface);
    };
    let result = surface(store, session).await?;
    if resume {
        // A conversation started with `lf <skill>` belongs to its terminal;
        // this one runs a second agent on the same thread beside it.
        if running()? && !store.sqlite.runs_in_its_terminal(&session.id)? {
            crate::harness::agent_process::stop_session_agent(
                &store.sqlite,
                &session.id,
                departure_here(),
            )?;
            crate::journal::request::diagnostic(format_args!(
                "Stopped the {} client that had this conversation; continuing here.",
                session.provider.as_deref().unwrap_or("provider")
            ));
        }
        crate::lf::commands::util::resume_session(
            session.input_key()?,
            &provider_session,
            named_work,
        )?;
    }
    Ok(result)
}

/// Open without replacing a client. Start may initialize a first thread, but
/// never replay consumed input; reveal requires history. The launch lock and
/// transport handshake recheck the facts after preparation, which carries no
/// takeover authority.
pub(crate) async fn open_without_replacing(
    store: &SharedStore,
    id: &str,
    intent: SessionOpenIntent,
    launch: bool,
    json: bool,
    named_work: bool,
) -> Result<SessionRecord> {
    let start = intent == SessionOpenIntent::Start;
    let lock_id = id.to_owned();
    let lock = tokio::task::spawn_blocking(move || lock_session_process(&lock_id)).await??;
    let session = store
        .session(id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    if let Some(reason) = run_refusal(&store.sqlite, &session)? {
        bail!(reason);
    }
    // Opening restarts what an earlier build's owner still runs; reading what
    // opening would do does not, and counts that owner as no client.
    let earlier_owner = if launch {
        end_earlier_owner(&store.sqlite, id, None)?;
        false
    } else {
        store.sqlite.session_owner(id)?.is_some()
    };
    let mut record = surface(store, &session).await?;
    let attachable = store
        .sqlite
        .session_terminal(id)?
        .filter(|terminal| crate::transport::current() != Some(terminal) && answering(terminal));
    let native = session
        .artifact_key
        .as_deref()
        .map(|key| store.sqlite.input_provider_session(key))
        .transpose()?
        .flatten();
    let attachment = store.sqlite.session_attachment(id)?;
    let occupied = !earlier_owner
        && attachment.as_ref().is_some_and(|attachment| {
            crate::journal::agent_evidence(&store.sqlite, &attachment.agent_process_id)
                != crate::journal::ProcessIdentityEvidence::Dead
        });
    let first_thread = attachable.is_none() && native.is_none();
    let reason = if record.state == SessionState::Active || (occupied && attachable.is_none()) {
        Some(CONTROLLED_ELSEWHERE.to_string())
    } else if !session.cwd.is_dir() {
        Some("The recorded checkout is unavailable; it has not been recreated".to_string())
    } else if first_thread && !start {
        Some(NO_PROVIDER_HISTORY.to_string())
    } else if first_thread
        && session.captured.is_some()
        && !capture_is_prepared(session.input_key()?)?
    {
        Some(format!(
            "The previous launch has no saved provider history; use `lf resume {id}` to retry it explicitly"
        ))
    } else if handoff_agent(&session).is_some() {
        Some("Changing provider requires an explicit open".to_string())
    } else {
        None
    };
    let kind = if start {
        SessionActionKind::Open
    } else {
        SessionActionKind::Reveal
    };
    if let Some(action) = record.actions.iter_mut().find(|action| action.kind == kind) {
        action.unavailable_reason = reason.clone();
    }
    record.open_argv.extend(intent.flag().map(str::to_string));
    if !launch {
        return Ok(record);
    }
    if let Some(reason) = reason {
        bail!(reason);
    }
    if let Some(terminal) = attachable {
        drop(lock);
        crate::transport::attach_existing(&terminal, id, json)?;
        return Ok(record);
    }
    // A held terminal re-executes this same intent and acquires the lock
    // again; a bare conversation opens here, in this terminal.
    let lock = if crate::transport::current().is_none() {
        drop(lock);
        if crate::transport::delegate_session(json.then_some(id), Some(id), false, named_work)? {
            return Ok(record);
        }
        let lock_id = id.to_owned();
        tokio::task::spawn_blocking(move || lock_session_process(&lock_id)).await??
    } else {
        lock
    };
    let Some(native) = native else {
        // Keep the ownership read and first-thread admission under one lock.
        return open_waiting_locked(store, session, lock, named_work)
            .await
            .map(|()| record);
    };
    if let Some(provider) = session.provider.as_deref() {
        let provider = provider.parse()?;
        let ran_on = store
            .provider_session_account(provider, &native.agent_session)
            .await?;
        if crate::provider_account::names_another_account(store, provider, ran_on)
            .await?
            .is_some()
        {
            bail!("Changing account requires an explicit open");
        }
    }
    let environment = BTreeMap::from([(
        HUMAN_SESSION_ENV.to_string(),
        serde_json::to_string(&session_token(&session))?,
    )]);
    crate::lf::commands::util::resume_session_with_env(
        session.input_key()?,
        Some(&native),
        &environment,
        Some(lock),
        named_work,
    )?;
    Ok(record)
}

/// A new conversation in the same Work, seeded with `source`'s log. The
/// source and whatever holds it are left alone.
async fn fork(
    store: &SharedStore,
    source: &LfSession,
    launch: bool,
    json: bool,
) -> Result<SessionRecord> {
    let request = conversation_log::continuation(&store.sqlite, source)?;
    let (provider, model) = match crate::config::agent_override() {
        Some(agent) => {
            let (provider, model) = crate::config::parse_agent(&agent);
            (Some(provider), model)
        }
        None => (source.provider.clone(), source.model.clone()),
    };
    let next = store
        .create_session(LfSession {
            id: uuid::Uuid::new_v4().simple().to_string().into(),
            captured: None,
            artifact_key: Some(crate::session_record::new_artifact_key()),
            caller_artifact_key: None,
            input_published: false,
            skill: None,
            provider,
            model,
            node: None,
            iterations: None,
            graph_lf_process_id: None,
            interactive: true,
            title: format!("{} (fork)", source.title),
            title_source: crate::session::TitleSource::Generated,
            request: Some(request),
            ready_summary: None,
            completed_at: None,
            created_at: crate::store::rows::now_unix(),
            ..source.clone()
        })
        .await?;
    let next = publish_prepared_input(
        &store.sqlite,
        &next,
        crate::session_record::SessionGraphMembership::Independent,
    )?;
    crate::journal::request::diagnostic(format_args!(
        "Forked {} into {}; the original keeps running.",
        source.id, next.id
    ));
    store
        .sqlite
        .record_provider_lineage(&next, &store.sqlite.provider_lineage(source)?)?;
    let record = surface(store, &next).await?;
    if launch {
        open_prepared(store, &next.id, json).await?;
    }
    Ok(record)
}

/// Preparation has already mutated the Session. Only its resulting identity
/// crosses to the launch; opening it reacquires the lock and reads current state.
/// Its new thread has no native history, so it launches with the Session's Work.
async fn open_prepared(store: &SharedStore, id: &str, json: bool) -> Result<()> {
    if !crate::transport::delegate_prepared_session(id, json)? {
        open_waiting(store, id, false).await?;
    }
    Ok(())
}

/// Why `lf resume` refuses a Run (a headless conversation, alone or a Graph
/// step), running or finished: it takes only Sessions. None for a Session.
pub(crate) fn run_refusal(store: &SqliteStore, session: &LfSession) -> Result<Option<String>> {
    if session.interactive {
        return Ok(None);
    }
    let summary = store
        .session_summary(&session.id, crate::store::rows::now_unix())?
        .ok_or_else(|| session_not_found(&session.id))?;
    let running = summary.completed_at.is_none()
        && summary.agent_process_evidence == crate::journal::ProcessIdentityEvidence::Live;
    let mut what: Vec<&str> = Vec::new();
    what.extend(summary.task_identifier.as_deref());
    what.extend(summary.graph.as_ref().map(|graph| graph.name.as_str()));
    what.extend(summary.skill.as_deref());
    what.push(if running { "running" } else { "finished" });
    let pointer = match &summary.task_identifier {
        Some(task) => format!("`lf resume --task {task}` opens its Task's Session"),
        None => "`lf resume` opens this checkout's Session".to_string(),
    };
    Ok(Some(format!(
        "{} is a Run ({}); `lf resume` takes only Sessions. {pointer}.",
        session.id,
        what.join(" · ")
    )))
}

/// The terminal this process runs in, named for the one a move leaves.
fn departure_here() -> crate::process::Departure {
    let to = if crate::session_record::current_terminal_id().is_some() {
        Some("Desktop".to_string())
    } else if let Some(placement) = crate::lf::commands::open::host_placement() {
        Some(placement.replacen(':', " ", 1))
    } else {
        crate::session_record::stdin_tty().map(|tty| tty.trim_start_matches("/dev/").to_string())
    };
    crate::process::Departure::Terminal { to }
}

/// Open a conversation: resume its native history, else
/// launch its prepared input, else append another attempt to the Session.
async fn open_waiting(store: &SharedStore, id: &str, named_work: bool) -> Result<()> {
    crate::agent_input_timing::record("launch_lock_begin", id);
    let lock_id = id.to_string();
    let launch_lock =
        crate::journal::request::spawn_blocking(move || lock_session_process(&lock_id)).await??;
    crate::agent_input_timing::record("launch_lock_acquired", id);
    // Resolve the current input under the launch lock, including completion
    // or replacement that happened while this opener waited.
    let session = store
        .session(id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    open_waiting_locked(store, session, launch_lock, named_work).await
}

async fn open_waiting_locked(
    store: &SharedStore,
    session: LfSession,
    launch_lock: File,
    named_work: bool,
) -> Result<()> {
    let id = session.id.clone();
    let session = primary::admit_workspace(store, session).await?;
    let mut launch_lock = Some(launch_lock);
    let session = if session.captured.is_none() {
        append_input(store, session).await?
    } else {
        session
    };
    let token = session_token(&session);
    if resume_native_session(
        store,
        session.input_key()?,
        &token,
        &mut launch_lock,
        named_work,
    )? {
        return Ok(());
    }
    crate::agent_input_timing::record("fresh_launch_selected", &id);
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

/// The other harness the invocation selected for a recorded conversation; it
/// starts a new thread from the log rather than resuming native history.
pub(super) fn handoff_agent(session: &LfSession) -> Option<String> {
    match crate::config::resume_choice(
        session.provider.as_deref()?,
        session.model.as_deref(),
        crate::config::agent_override().as_deref(),
    ) {
        crate::config::Resume::Import { agent } => Some(agent),
        crate::config::Resume::Native(_) => None,
    }
}

/// Start a fresh provider thread on the same Session, on `agent` or its
/// current provider, seeded with a log of the current thread. The Session
/// keeps its name, Work and feedback; the old thread stays in its history.
/// Without `launch`, the new input waits for its first open.
pub(crate) async fn new_thread(
    store: &SharedStore,
    id: &str,
    agent: Option<&str>,
    launch: bool,
    json: bool,
    request_id: &str,
) -> Result<LfSession> {
    let identity = crate::engine::creation::identity(
        "session.new",
        request_id,
        &serde_json::json!({"id":id,"agent":agent}),
        false,
    )?;
    let request = request_id.to_owned();
    let _creation_lock =
        crate::journal::request::spawn_blocking(move || crate::engine::creation::lock(&request))
            .await??;
    store.sqlite.check_creation(&identity)?;
    // Serialize with other Session launches and new-thread requests.
    let lock_id = id.to_string();
    let launch_lock =
        crate::journal::request::spawn_blocking(move || lock_session_process(&lock_id)).await??;
    if let Some(receipt) = store.sqlite.creation_receipt(request_id)? {
        if receipt.disposition == crate::store::sqlite::creation::RecoveryDisposition::Inspect {
            return Err(crate::engine::creation::unknown(&receipt).into());
        }
        let next = store
            .session(id)
            .await?
            .ok_or_else(|| session_not_found(id))?;
        if receipt.disposition == crate::store::sqlite::creation::RecoveryDisposition::Continue {
            // The input and receipt committed together, before filesystem publication.
            // Only that exact input may finish publishing; never replace a newer one.
            if next.artifact_key != receipt.artifact_key {
                return Err(crate::engine::creation::unknown(&receipt).into());
            }
            return finish_thread_creation(store, next, request_id).await;
        }
        return Ok(next);
    }
    let session = store
        .session(id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    let session = primary::admit_workspace(store, session).await?;
    let from = session.provider.clone().unwrap_or_default();
    if let Some(refusal) = run_refusal(&store.sqlite, &session)? {
        bail!(refusal);
    }
    let threaded = match session.artifact_key.as_deref() {
        Some(key) => store.sqlite.input_provider_session(key)?.is_some(),
        None => false,
    };
    // A conversation with no provider history carries its original first prompt.
    let request = if threaded {
        Some(conversation_log::continuation(&store.sqlite, &session)?)
    } else {
        session.request.clone()
    };
    let lineage = threaded
        .then(|| store.sqlite.provider_lineage(&session))
        .transpose()?;
    let artifact = crate::session_record::new_artifact_key();
    store
        .sqlite
        .begin_thread_creation(&identity, id, &artifact)?;
    crate::harness::agent_process::stop_session_agent(
        &store.sqlite,
        &session.id,
        departure_here(),
    )?;
    let mut next = session;
    if let Some(agent) = agent {
        let (provider, model) = crate::config::parse_agent(agent);
        next.provider = Some(provider);
        next.model = model;
    }
    // The skill ran in the earlier conversation and is in its log. The
    // continuation is stored so a relaunch after a failed start still carries it.
    if threaded {
        next.skill = None;
    }
    next.request = request;
    next.artifact_key = Some(artifact);
    next.input_published = false;
    let next = store
        .replace_session_input(next.captured, next, Some(request_id.into()))
        .await?;
    if let Some(lineage) = lineage {
        store.sqlite.record_provider_lineage(&next, &lineage)?;
    }
    let next = finish_thread_creation(store, next, request_id).await?;
    crate::journal::request::diagnostic(format_args!(
        "Started a new {} thread for {}, seeded with what the {from} thread knew.",
        next.provider.as_deref().unwrap_or("provider"),
        next.title
    ));
    if launch {
        drop(launch_lock);
        open_prepared(store, &next.id, json).await?;
    }
    Ok(next)
}

// First submission and recovery finish the same input. Publication may already
// have succeeded when the process exited before recording its result.
async fn finish_thread_creation(
    store: &SharedStore,
    next: LfSession,
    request_id: &str,
) -> Result<LfSession> {
    let next = if next.input_published {
        next
    } else {
        publish_prepared_input(
            &store.sqlite,
            &next,
            crate::session_record::SessionGraphMembership::Independent,
        )?
    };
    let result = surface(store, &next).await?;
    store
        .sqlite
        .advance_creation(request_id, "admitted", Some(&serde_json::to_value(result)?))?;
    Ok(next)
}

/// Append a fresh, published input to the Session; earlier inputs stay in its history.
pub(crate) async fn append_input(store: &SharedStore, mut next: LfSession) -> Result<LfSession> {
    let replaced = next.captured;
    next.artifact_key = Some(crate::session_record::new_artifact_key());
    next.input_published = false;
    let next = store.replace_session_input(replaced, next, None).await?;
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

/// How long an earlier build's owner has to exit once signalled.
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// An earlier build's `lf session serve` owner runs until it records its exit
/// or the OS shows it gone, as after a restart. Unknown evidence is not exit.
fn owner_running(store: &SqliteStore, owner: &crate::id::LfProcessId) -> Result<bool> {
    Ok(store
        .lf_process(owner)?
        .is_some_and(|process| process.completed_at.is_none())
        && crate::journal::lf_evidence(store, owner)
            != crate::journal::ProcessIdentityEvidence::Dead)
}

/// End the `lf session serve` owner an earlier build left running `session`,
/// so the conversation restarts from provider history. The owner is
/// signalled only by its verified pid and birth, and this returns once it has
/// exited. One whose identity cannot be verified, or that outlives
/// `STOP_TIMEOUT`, is left running and nothing starts beside it. Nothing ends
/// these owners but a resume. The wait can take the whole timeout, so a
/// terminal's `progress` says what it waits for.
pub(crate) fn end_earlier_owner(
    store: &SqliteStore,
    session: &str,
    progress: Option<&crate::startup::Startup>,
) -> Result<()> {
    let Some(owner) = store.session_owner(session)? else {
        return Ok(());
    };
    if !owner_running(store, &owner)? {
        return Ok(());
    }
    let still_running = |pid: Option<u32>| {
        let pid = pid.map_or_else(|| "unknown".to_owned(), |pid| pid.to_string());
        anyhow!(
            "This conversation is still running under an earlier lf (pid {pid}); quit it there, then resume"
        )
    };
    let Some(pid) = crate::journal::live_process_pid(store, &owner) else {
        let recorded = store.lf_process(&owner)?.and_then(|process| process.pid);
        return Err(still_running(recorded));
    };
    let ending = format!("Ending the earlier lf (pid {pid}) that runs this conversation");
    match progress {
        Some(progress) => progress.stage(ending),
        None => crate::journal::request::diagnostic(format_args!("{ending}")),
    }
    crate::platform::kill_process(pid);
    let deadline = std::time::Instant::now() + STOP_TIMEOUT;
    while owner_running(store, &owner)? {
        if std::time::Instant::now() >= deadline {
            return Err(still_running(Some(pid)));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// Whether a held terminal answers the server. This observes reachability,
/// not whether its provider can safely be replaced.
fn answering(terminal: &crate::id::TerminalId) -> bool {
    match crate::transport::controlled(terminal) {
        Ok(_) => true,
        Err(error) => {
            tracing::warn!(%error, %terminal, "Session terminal does not answer");
            false
        }
    }
}

/// One Session's summary, with its viewers confirmed by its held terminal.
/// The server records the terminal's viewers, but a server from an earlier lf
/// cannot write a store a newer lf has advanced: after an upgrade and a
/// Desktop restart its record still says viewed. Its controlling viewer is the
/// truth, and a terminal that does not answer has no viewer anyone can reach.
pub(crate) fn viewed_summary(
    store: &SqliteStore,
    id: &str,
) -> Result<Option<crate::session::SessionSummary>> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let Some(mut summary) = store.session_summary(id, now)? else {
        return Ok(None);
    };
    let released = !summary.unviewed
        && summary.agent_process_evidence == crate::journal::ProcessIdentityEvidence::Live
        && store
            .session_terminal(id)?
            .is_some_and(|terminal| !matches!(crate::transport::controlled(&terminal), Ok(true)));
    if released {
        summary.unviewed = true;
        summary.terminal_id = None;
    }
    Ok(Some(summary))
}

/// A Session as its row and its current input describe it.
pub(crate) async fn surface(store: &SharedStore, session: &LfSession) -> Result<SessionRecord> {
    let work = match (&session.task_id, &session.wave_id) {
        (Some(task), _) => Some(WorkRef::Task(task.clone())),
        (None, Some(wave)) => Some(WorkRef::Wave(wave.clone())),
        (None, None) => None,
    };
    let metadata = viewed_summary(&store.sqlite, &session.id)?
        .ok_or_else(|| session_not_found(&session.id))?;
    let state = session_state(&metadata);
    let actions = projected_actions(&metadata);
    let open_argv = human_session_argv(&session.id)?;
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
        transition: store.sqlite.provider_transition(&session.id)?,
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
        model: session.model.clone(),
        blocked: LaunchCapacity::read(&store.sqlite).is_ok_and(|capacity| {
            is_blocked(
                &capacity,
                session.provider.as_deref(),
                metadata.account.as_ref(),
                state,
                session.model.as_deref(),
            )
        }),
        cwd: session.cwd.display().to_string(),
        state,
        ready_summary: session.ready_summary.clone(),
        terminal_ids: metadata.terminal_id.clone().into_iter().collect(),
        open_argv,
        turn: None,
        raw: false,
    };
    workspace::associate(store, std::slice::from_mut(&mut reading)).await?;
    turns::read(&store.sqlite, std::slice::from_mut(&mut reading))?;
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
    let wave = match &session.wave_id {
        Some(id) => Some(match store.get_wave(id).await? {
            Some(wave) => wave.slug().to_string(),
            None => format!("Wave {id} (unavailable)"),
        }),
        None => None,
    };
    let task = match &session.task_id {
        Some(id) => Some(match store.get_task(id).await? {
            Some(task) => task.identifier,
            None => format!("Task {id} (unavailable)"),
        }),
        None => None,
    };
    Ok(join_work_path(wave, task))
}

/// "Wave / Task", either alone, or nothing for a repository conversation.
fn join_work_path(wave: Option<String>, task: Option<String>) -> Option<String> {
    match (wave, task) {
        (Some(wave), Some(task)) => Some(format!("{wave} / {task}")),
        (wave, task) => wave.or(task),
    }
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

/// Bind and select the Task conversation without changing its live attachment.
pub(crate) async fn bind(store: &SharedStore, id: &str, task: &str) -> Result<SessionRecord> {
    let (session, task) = binding_target(store, id, task).await?;
    crate::ops::task::adoption::bind_session(store, &session, &task).await?;
    let session = store
        .session(&session.id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    crate::journal::request::diagnostic(format_args!(
        "Binding {} to {} ({}). Permanent.",
        session.id, task.identifier, task.title
    ));
    surface(store, &session).await
}

/// Resolved identity for confirmation, not a reservation or permission to bind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
    anyhow::ensure!(
        session.interactive && session.completed_at.is_none(),
        "Select an unfinished interactive Session to become the Task conversation"
    );
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

pub(crate) fn resume_native_session(
    store: &SharedStore,
    artifact_key: &String,
    token: &HumanSessionToken,
    launch_lock: &mut Option<File>,
    named_work: bool,
) -> Result<bool> {
    let Some(session) = store.sqlite.session_for_artifact(artifact_key)? else {
        return Ok(false);
    };
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
        departure_here(),
    )?;
    let environment =
        BTreeMap::from([(HUMAN_SESSION_ENV.to_string(), serde_json::to_string(token)?)]);
    crate::agent_input_timing::record("native_resume_selected", &session.id);
    crate::lf::commands::util::resume_session_with_env(
        session.input_key()?,
        provider_session.as_ref(),
        &environment,
        launch_lock.take(),
        named_work,
    )?;
    Ok(true)
}

/// The `lf session resume` command that opens this Session in a new terminal.
pub(crate) fn human_session_argv(id: &str) -> Result<Vec<String>> {
    let context = crate::os_process::execution_context()?;
    // A fresh terminal does not inherit the listing process's data selection.
    // Carry its Machine and stable entry so opening picks up the current install.
    Ok(vec![
        "/usr/bin/env".to_string(),
        format!("LF_BIN={}", context.lf_bin.display()),
        format!("LF_HOME={}", context.lf_home.display()),
        context.lf_bin.display().to_string(),
        "session".to_string(),
        "resume".to_string(),
        id.to_string(),
    ])
}

const PRIMARY_MESSAGE: &str = "<lf:primary-session>\nThis is the one ongoing primary conversation of its repository, Wave or Task, and nobody has asked for anything yet. Read its current state, report where it stands and change no Work: launch no Graph, claim or start no Task and replay no earlier input, even for a stopped run. Steers delivered with this turn are recorded direction: report them and act when the user asks here. Operate once the user asks.\n</lf:primary-session>";

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

/// One Program Status reading from the terminal showing a Session. `stream`
/// names one observer; its first reading (`sequence` 1) fences any earlier
/// stream, even on the same provider. It owns no Session and no provider.
/// A held terminal's server records its Session's status; a view's reading
/// of one is ignored.
pub(crate) async fn record_program_status(
    store: &SharedStore,
    id: &str,
    terminal: &str,
    agent_process: Option<&crate::id::AgentProcessId>,
    stream: &str,
    sequence: i64,
    records: &crate::program_status::Records,
) -> Result<()> {
    anyhow::ensure!(
        sequence > 0 && records.seen && records.validate(),
        "invalid Program Status snapshot"
    );
    let session = find_session(store, id, false)
        .await?
        .context("Session not found")?;
    // A held terminal's server records its status; a view's would race it.
    if store.sqlite.session_is_held(&session.id)? {
        return Ok(());
    }
    if sequence == 1 {
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
        anyhow::ensure!(
            store
                .sqlite
                .begin_program_status(&session.id, agent_process, stream)?,
            "Session provider changed"
        );
    }
    anyhow::ensure!(
        store.sqlite.record_program_status(
            &session.id,
            agent_process,
            stream,
            sequence,
            records
        )?,
        "Program Status stream replaced"
    );
    Ok(())
}

#[cfg(test)]
mod tests;
