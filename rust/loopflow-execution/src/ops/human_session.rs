use crate::store::session_projection;
pub use primary::admit_workspace;
mod resume_explanation;
pub use resume_explanation::explain_resume;
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

use crate::session::LfSession;
use crate::session_record::SessionTitleSource;
use crate::store::SharedStore;
use loopflow_model::durable::WorkRef;
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::task::Task;

pub mod conversation_log;
pub mod moves;
pub mod moves_bulk;
pub mod primary;
pub use primary::ensure_scope_worktree;
pub mod provider_conversation;
mod setup;

/// Launch exclusion is process state beside the Machine, not a Session record.
const LAUNCH_LOCK_DIRECTORY: &str = "human-sessions";
const SESSION_START_TIMEOUT: Duration = Duration::from_secs(30);

/// What opening a conversation leaves for this process to do. One is returned
/// per opening, so its variants' sizes do not matter.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Opening {
    /// It is shown, attached or resumed; nothing more runs here.
    Done(SessionRecord),
    /// Its prepared input starts fresh in this process.
    Launch(ConversationLaunch),
}

impl Opening {
    /// A preparation that starts no provider never yields a launch.
    ///
    /// # Errors
    /// When opening produced a launch after all.
    pub fn prepared(self) -> Result<SessionRecord> {
        match self {
            Self::Done(record) => Ok(record),
            Self::Launch(launch) => {
                bail!("preparing Session {} started its launch", launch.session_id)
            }
        }
    }
}

/// A conversation's fresh start, run by the `lf session resume` that opened it:
/// no exec, so that process is the provider's launcher and its one LfProcess.
#[derive(Debug)]
pub struct ConversationLaunch {
    pub session_id: String,
    pub agent: String,
    pub cwd: PathBuf,
    /// The conversation's Work, when launch can resolve it; the Session row
    /// keeps the attribution either way.
    pub work: Option<crate::ops::WorkBinding>,
    pub skill: Option<String>,
    pub first_turn: String,
    /// The input's prepared capture, which the launch publishes rather than a new one.
    pub prepared_capture: Option<String>,
    pub lock: LaunchLock,
    /// The kind delegation chose: a detached Work launch stays a work session.
    pub kind: crate::harness::LaunchKind,
}

/// A Session launch lock, held until the provider is recorded. Clones share
/// it: the first `take` releases it to its caller.
#[derive(Debug, Clone)]
pub struct LaunchLock(std::sync::Arc<std::sync::Mutex<Option<File>>>);

impl LaunchLock {
    fn new(file: File) -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(Some(file))))
    }

    pub fn take(&self) -> Option<File> {
        self.0.lock().ok()?.take()
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
pub async fn resolve_target<'a>(
    store: &SharedStore,
    target: SessionTarget<'a>,
) -> Result<SessionTarget<'a>> {
    let SessionTarget::Session(id) = target else {
        return Ok(target);
    };
    if store.session(id).await?.is_some() {
        return Ok(target);
    }
    Ok(
        match crate::store::children::resolve_task_id(&store.sqlite, id, None)? {
            Some(_) => SessionTarget::Task(id),
            None => target,
        },
    )
}

/// Resolve physical checkout ownership without admitting a conversation.
pub async fn connection_scope(
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
    let path = crate::repository::discover(cwd)?.to_string();
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
    let repo = crate::repository::discover(cwd)?;
    if matches!(target, SessionTarget::Repository) {
        return Ok(Some(PrimaryScope::Repository(repo)));
    }
    let root = crate::git::worktree_root(cwd)?;
    if let Some(task) = crate::ops::task::checkout_owner(store, &root).await? {
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
pub struct Selected {
    pub id: String,
    pub started: bool,
}

/// The scope `target` names from `cwd`, and its conversation if one exists.
/// Reading creates nothing: `lf open --focus` and `lf resume --explain` use it.
pub async fn existing_connection(
    store: &SharedStore,
    cwd: &Path,
    target: SessionTarget<'_>,
) -> Result<(Option<crate::session::PrimaryScope>, Option<String>)> {
    use crate::session::PrimaryScope;
    let target = resolve_target(store, target).await?;
    if let SessionTarget::Session(id) = target {
        return Ok((None, Some(id.to_string())));
    }
    let Some(scope) = checkout_scope(store, cwd, target).await? else {
        if let Some(id) = checkout_conversation(store, cwd).await? {
            return Ok((None, Some(id)));
        }
        return Err(unassociated_checkout(cwd)?);
    };
    let session = match &scope {
        PrimaryScope::Task(task) => primary::task_conversation(store, task).await?,
        _ => store.sqlite.primary_session(&scope)?,
    };
    Ok((Some(scope), session.map(|session| session.id.to_string())))
}

/// The conversation `lf resume` selects, created when its scope has none.
pub async fn select_connection(
    store: &SharedStore,
    cwd: &Path,
    target: SessionTarget<'_>,
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
    let (session, started) = match scope {
        PrimaryScope::Repository(repo) => primary::ensure(store, repo.as_path(), None).await?,
        PrimaryScope::Wave(wave) => primary::ensure(store, cwd, Some(wave.as_str())).await?,
        PrimaryScope::Task(task) => primary::ensure_task(store, cwd, task.as_str(), None).await?,
        _ => anyhow::bail!("Unsupported conversation scope: {scope:?}"),
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

/// What opening a Session does here, as a person reads it. Every opening is
/// `lf resume`: it adds a view, resumes saved history, or takes the
/// conversation over from a client lf can't share.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionAction {
    pub label: String,
    pub help: String,
}

/// What `lf resume` does with this Session here, before the choices one
/// invocation makes (another agent, a fork, preparation only).
pub(crate) fn resume_intent(session: &crate::session::SessionSummary) -> SessionResumeIntent {
    if session.held {
        SessionResumeIntent::View
    } else if session.agent_process_id.is_some()
        && session.agent_process_evidence != crate::store::ProcessIdentityEvidence::Dead
    {
        SessionResumeIntent::TakeOver
    } else if session.has_provider_history {
        SessionResumeIntent::Resume
    } else {
        SessionResumeIntent::Start
    }
}

/// How `intent` reads on this Session. A Run opens nowhere, since `lf resume`
/// takes only Sessions; another harness's new thread is chosen per invocation,
/// so no record reads as one.
pub(crate) fn opening(
    intent: SessionResumeIntent,
    session: &crate::session::SessionSummary,
) -> Option<SessionAction> {
    if !session.interactive {
        return None;
    }
    let (label, help) = match intent {
        SessionResumeIntent::View => (
            if session.unviewed {
                "Reconnect"
            } else {
                "Open here"
            },
            format!("View the running conversation here: lf resume {}", session.id),
        ),
        SessionResumeIntent::TakeOver => (
            "Take over",
            "Continue here; the client that has it ends, unsent text there is lost".into(),
        ),
        SessionResumeIntent::Resume => ("Open here", "Open this Session in a terminal".into()),
        SessionResumeIntent::Start if launch_consumed(session) => (
            "Try again",
            "The previous launch ended before the provider saved any history; this repeats its request".into(),
        ),
        SessionResumeIntent::Start => (
            "Start conversation",
            "No saved provider history; this starts it".into(),
        ),
        SessionResumeIntent::NewThread => return None,
    };
    Some(SessionAction {
        label: label.to_string(),
        help,
    })
}

/// Its first launch ended before the provider saved history. Listing reads the
/// marker alone: `capture_is_prepared` opens the store per Session.
fn launch_consumed(session: &crate::session::SessionSummary) -> bool {
    session.captured.is_some()
        && session
            .artifact_key
            .as_deref()
            .and_then(local_capture_dir)
            .is_some_and(|dir| !dir.join("prepared").is_file())
}

/// How opening this Session here reads.
pub(crate) fn session_action(session: &crate::session::SessionSummary) -> Option<SessionAction> {
    opening(resume_intent(session), session)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionPage<T = SessionRecord> {
    pub entries: Vec<T>,
    pub next: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionRecord {
    pub transition: Option<loopflow_storage::sqlite::provider_transitions::ProviderTransition>,
    pub preview: Option<crate::session::SessionPreview>,
    pub primary_scope: Option<String>,
    /// Available for a person, judged from current stream or terminal reports; absent when it is
    /// working or nothing current says.
    pub availability: Option<SessionAvailability>,
    pub program_status: Option<loopflow_model::program_status::Records>,
    /// The provider's current AgentProcess; absent when Loopflow never launched one.
    pub agent_process_id: Option<loopflow_model::id::AgentProcessId>,
    pub task_ids: Vec<loopflow_model::durable::TaskId>,
    pub id: loopflow_model::id::LfSessionId,
    pub workspace: Option<SessionWorkspace>,
    pub interactive: bool,
    pub work: Option<WorkRef>,
    pub wave_id: Option<loopflow_model::id::WaveId>,
    pub work_path: Option<String>,
    /// How opening it here reads; absent for a Run, which opens nowhere.
    #[serde(deserialize_with = "Option::deserialize")]
    pub action: Option<SessionAction>,
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
    pub cwd: String,
    pub state: SessionState,
    pub open_argv: Vec<String>,
    pub terminal_ids: Vec<String>,
    /// Its current or last turn while it is live; `null` when nothing says.
    #[serde(deserialize_with = "Option::deserialize")]
    pub turn: Option<loopflow_harness::transcript::live::SessionTurn>,
    /// A conversation open here that Loopflow never launched; `id` is the
    /// provider's conversation ID and no Session records it.
    pub raw: bool,
    /// How its last agent ended, while none runs; `null` for a headless
    /// Session, before a first start, or when nobody recorded the end.
    #[serde(deserialize_with = "Option::deserialize")]
    pub ended: Option<SessionEnd>,
    /// The server's starts of a Task's conversation since it last finished a
    /// turn; `null` when it made none, and once the person quit.
    #[serde(deserialize_with = "Option::deserialize")]
    pub start: Option<SessionStarts>,
    /// What happens next to a conversation that is not running, as this
    /// Machine reads it; `null` while it runs, and when nothing says why it
    /// is down.
    #[serde(deserialize_with = "Option::deserialize")]
    pub outlook: Option<SessionOutlook>,
    /// Whether it runs, how it ended and its outlook, as one sentence in this
    /// Machine's local time: "died 12:01 (exit 137) · starting again". Every
    /// client prints it as given; `null` for a headless Session.
    #[serde(deserialize_with = "Option::deserialize")]
    pub status_line: Option<String>,
}

/// A conversation's last agent end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionEnd {
    /// RFC 3339.
    pub at: String,
    /// The provider's exit; `null` when nobody saw it (the holder died, or
    /// the Machine restarted).
    #[serde(deserialize_with = "Option::deserialize")]
    pub exit_code: Option<i32>,
}

/// What the server's recovery pass did for a Task conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionStarts {
    /// Starts since it last finished a turn.
    pub attempts: u32,
    /// RFC 3339: the pass's last start.
    #[serde(deserialize_with = "Option::deserialize")]
    pub last_started_at: Option<String>,
}

/// Whether a conversation that is not running comes up, and what brings it.
/// The server works this out; no client does it again from `ended` and
/// `start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionOutlook {
    pub kind: SessionOutlookKind,
    /// RFC 3339: when the server starts it next; a time already past is a
    /// start under way. `null` when the server will not start it, and for
    /// one that is starting.
    #[serde(deserialize_with = "Option::deserialize")]
    pub at: Option<String>,
    /// Ready to print. Why it could not start; for one coming back, that its
    /// last start failed and why; for one waiting, why the server leaves it.
    #[serde(deserialize_with = "Option::deserialize")]
    pub reason: Option<String>,
    /// The command that opens it; `null` while the server brings it up.
    #[serde(deserialize_with = "Option::deserialize")]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionOutlookKind {
    /// No agent runs yet and one is on its way: a Task's conversation the
    /// server here is starting, or a start whose agent has no pid recorded.
    Starting,
    /// It died and the server here starts it again, at `at`.
    ComingBack,
    /// It is down and only its command brings it up: it is not a Task's
    /// conversation, or the server here does not or no longer starts it.
    WaitingForYou,
    /// The person quit it. It stays quit.
    Quit,
    /// No agent has run yet: its start failed, for `reason`. The server
    /// tries again at `at`, or left it to `command`.
    CouldNotStart,
}

/// What a Session's record says about its agent's end and what comes next.
#[derive(Debug, Default, PartialEq, Eq)]
struct SessionEnding {
    ended: Option<SessionEnd>,
    start: Option<SessionStarts>,
    outlook: Option<SessionOutlook>,
    status_line: Option<String>,
}

/// How an interactive Session's agent ended, what the recovery pass did for
/// a Task's conversation, what happens next to one that is not running, and
/// the sentence that says all of it. Whether the server starts it is
/// [`crate::ops::recovery::server_start`]'s answer, the one the pass acts on;
/// `claimed_here` and `server_runs` are this Machine's part of it.
fn session_ending(
    session: &crate::session::SessionSummary,
    now: i64,
    claimed_here: impl FnOnce() -> bool,
    server_runs: impl FnOnce() -> bool,
) -> SessionEnding {
    use crate::ops::recovery::{Left, ServerStart};
    use loopflow_storage::session::quit_by_person;
    if !session.interactive {
        return SessionEnding::default();
    }
    let state = session_state(session);
    let closed = state == SessionState::Closed;
    let end = session.agent_end.filter(|_| closed);
    let ended = end.and_then(|end| {
        Some(SessionEnd {
            at: loopflow_harness::transcript::rfc3339(end.at)?,
            exit_code: end.exit_code,
        })
    });
    let quit = end.is_some_and(|end| quit_by_person(end.exit_code));
    let task_primary = session.primary_scope.as_deref() == Some("task");
    let conversation = task_primary && !session.task_terminal && session.completed_at.is_none();
    let attempts = session.starts_since_turn;
    let start = (conversation && !quit && attempts > 0).then(|| SessionStarts {
        attempts,
        last_started_at: session
            .last_started_at
            .and_then(loopflow_harness::transcript::rfc3339),
    });
    if is_running(state) {
        let status_line = match session.last_started_at.filter(|_| start.is_some()) {
            Some(at) => format!("running (started again {}, attempt {attempts})", clock(at)),
            None => "running".to_string(),
        };
        return SessionEnding {
            ended,
            start,
            outlook: None,
            status_line: Some(status_line),
        };
    }
    if session.launching {
        // Between the old agent's end and the new one's pid.
        return SessionEnding {
            ended,
            start,
            outlook: Some(SessionOutlook {
                kind: SessionOutlookKind::Starting,
                at: None,
                reason: None,
                command: None,
            }),
            status_line: Some("starting".to_string()),
        };
    }
    let command = match (&session.task_identifier, task_primary) {
        (Some(task), true) => format!("lf resume --task {task}"),
        _ => format!("lf resume {}", session.id),
    };
    let failure = session.start_failure.as_deref().filter(|_| conversation);
    // One that never had an agent has no liveness to read: it is not Closed.
    let never_ran = session.agent_process_id.is_none() && ended.is_none();
    let ending = match end {
        Some(end) => format!(
            "died {} ({})",
            clock(end.at),
            end.exit_code
                .map_or("no exit recorded".to_string(), |code| format!(
                    "exit {code}"
                ))
        ),
        None if never_ran => "not started".to_string(),
        None => "closed".to_string(),
    };
    if ended.is_none() && failure.is_none() && !(never_ran && conversation) {
        // Nothing says why it is down: a recorded agent this Machine cannot
        // check, or one that went with no end recorded.
        let status_line = match state {
            SessionState::Unknown => "unknown".to_string(),
            _ => format!("{ending}; `{command}` opens it"),
        };
        return SessionEnding {
            ended,
            start,
            outlook: None,
            status_line: Some(status_line),
        };
    }
    let facts = crate::ops::recovery::DownConversation {
        here: conversation && session.in_task_checkout,
        exit_code: end.and_then(|end| end.exit_code),
        starts_since_turn: attempts,
        last_started_at: session.last_started_at,
    };
    let again = |later: &str, now_doing: &str, at: Option<i64>| match at {
        Some(at) if at > now => format!(" · {later} again {}", clock(at)),
        Some(_) => format!(" · {now_doing} again"),
        None => String::new(),
    };
    let printed = |reason: &Option<String>| {
        reason
            .as_deref()
            .map_or(String::new(), |reason| format!(" · {reason}"))
    };
    let verdict = crate::ops::recovery::server_start(&facts, claimed_here, server_runs);
    let at = match verdict {
        // A start counted after the agent ended is the one under way: it was
        // due then. The backoff names the start after the next death, or
        // after a start that failed before any agent ran.
        ServerStart::At(next) => session
            .last_started_at
            .zip(end)
            .filter(|(started, end)| *started >= end.at && failure.is_none())
            .map(|(started, _)| started)
            .or(next)
            .or(end.map(|end| end.at)),
        ServerStart::Left(_) => None,
    };
    let (kind, at, reason, mut line) = match (verdict, failure.filter(|_| never_ran)) {
        (ServerStart::Left(Left::Quit), _) => (
            SessionOutlookKind::Quit,
            None,
            None,
            end.map_or("quit".to_string(), |end| format!("quit {}", clock(end.at))),
        ),
        (ServerStart::At(_), None) if never_ran => (
            SessionOutlookKind::Starting,
            None,
            None,
            "starting".to_string(),
        ),
        (_, Some(failure)) => (
            SessionOutlookKind::CouldNotStart,
            at,
            Some(failure.to_string()),
            format!("could not start: {failure}{}", again("tries", "trying", at)),
        ),
        (ServerStart::Left(left), None) => {
            let left = left_reason(left, attempts, conversation);
            let reason = match (left, failure) {
                (Some(left), Some(failure)) => {
                    Some(format!("{left}; last start failed: {failure}"))
                }
                (left, _) => left,
            };
            let line = format!("{ending}{}", printed(&reason));
            (SessionOutlookKind::WaitingForYou, None, reason, line)
        }
        (ServerStart::At(_), None) => {
            let reason = failure.map(|failure| format!("last start failed: {failure}"));
            let line = format!(
                "{ending}{}{}",
                again("starts", "starting", at),
                printed(&reason)
            );
            (SessionOutlookKind::ComingBack, at, reason, line)
        }
    };
    // The server brings it up, or its command does.
    let command = matches!(verdict, ServerStart::Left(_)).then_some(command);
    if let Some(command) = &command {
        line.push_str(&format!("; `{command}` opens it"));
    }
    SessionEnding {
        ended,
        start,
        outlook: Some(SessionOutlook {
            kind,
            at: at.and_then(loopflow_harness::transcript::rfc3339),
            reason,
            command,
        }),
        status_line: Some(line),
    }
}

/// [`session_ending`] as this Machine answers it: whether it claims the
/// Session's Task and whether a server runs here.
fn ending_here(
    store: &SqliteStore,
    session: &crate::session::SessionSummary,
    now: i64,
) -> SessionEnding {
    let claimed = || {
        session
            .task_id
            .as_ref()
            .is_some_and(|task| crate::ops::recovery::claimed_here(store, task))
    };
    session_ending(session, now, claimed, crate::ops::recovery::server_runs)
}

/// Why the server leaves something that is down to a person, ready to
/// print; `None` when nothing here ever starts it. A Task's conversation and
/// a Task's run say it in the same words.
pub(crate) fn left_reason(
    left: crate::ops::recovery::Left,
    starts: u32,
    of_task: bool,
) -> Option<String> {
    use crate::ops::recovery::Left;
    match left {
        Left::OutOfStarts => Some(format!("started {starts} times")),
        Left::Elsewhere if of_task => Some("its Task is not checked out here".to_string()),
        // Not a Task's: nothing here ever starts it.
        Left::Elsewhere | Left::Quit => None,
        Left::NotClaimedHere => Some("its Task is not claimed on this Machine".to_string()),
        Left::NoServer => Some("no server runs here to start it".to_string()),
    }
}

/// This Machine's local `HH:MM` for a Unix time.
pub(crate) fn clock(at: i64) -> String {
    chrono::DateTime::from_timestamp(at, 0).map_or_else(
        || at.to_string(),
        |at| at.with_timezone(&chrono::Local).format("%H:%M").to_string(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionAvailability {
    Available,
}

pub(crate) fn session_state(session: &crate::session::SessionSummary) -> SessionState {
    match session.agent_process_evidence {
        crate::store::ProcessIdentityEvidence::Live if session.interactive && session.unviewed => {
            SessionState::Detached
        }
        crate::store::ProcessIdentityEvidence::Live => SessionState::Active,
        crate::store::ProcessIdentityEvidence::Dead => SessionState::Closed,
        crate::store::ProcessIdentityEvidence::Unknown => SessionState::Unknown,
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
        #[serde(deserialize_with = "loopflow_model::id::deserialize_uuid")]
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
    store: &loopflow_storage::sqlite::SqliteStore,
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

pub async fn list(
    store: &SharedStore,
    filter: &crate::session::SessionFilter,
) -> Result<Vec<SessionRecord>> {
    let on = match &filter.on {
        Some(raw) => Some(
            crate::config::Agent::parse(raw)
                .map_err(|_| anyhow!("--on requires a harness or harness:model"))?,
        ),
        None => None,
    };
    let mut selection = filter.clone();
    let task = match &filter.task {
        Some(selector) => {
            let Some(id) = crate::store::children::resolve_task_id(&store.sqlite, selector, None)?
            else {
                return Ok(Vec::new());
            };
            selection.task = Some(id.to_string());
            Some(id)
        }
        None => None,
    };
    // Harness selection uses the parsed agent below, before taking the page.
    if on.is_some() {
        selection.limit = 0;
    }
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut sessions = Vec::new();
    for summary in store.session_summaries(&selection, now).await? {
        if on.as_ref().is_some_and(|agent| !runs_on(agent, &summary)) {
            continue;
        }
        sessions.push(summary_surface(
            &summary,
            ending_here(&store.sqlite, &summary, now),
        ));
        if sessions.len() == filter.limit {
            break;
        }
    }
    workspace::associate(store, &mut sessions).await?;
    turns::read(&store.sqlite, &mut sessions)?;
    if lists_raw(filter) {
        let wave_tasks = match &filter.wave {
            Some(wave) => Some(store.sqlite.task_ids_in_wave(wave)?),
            None => None,
        };
        let raw = turns::raw(&store.sqlite, filter.repo.as_deref())?;
        sessions.extend(raw.into_iter().filter(|raw| {
            (!filter.available || raw.availability.is_some())
                && (!filter.orphan || raw.task_ids.is_empty())
                && task.as_ref().is_none_or(|id| raw.task_ids.contains(id))
                && wave_tasks
                    .as_ref()
                    .is_none_or(|ids| raw.task_ids.iter().any(|id| ids.contains(id)))
        }));
    }
    Ok(sessions)
}

/// Raw conversations list unless `--no-raw` asks otherwise. They have no
/// account or page position, so a selection by any of those leaves
/// them out. One open in a Task's checkout lists under that Task.
fn lists_raw(filter: &crate::session::SessionFilter) -> bool {
    filter.raw
        && filter.primary.is_none()
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
pub fn listed(store: &SharedStore, id: &str) -> Result<Option<SessionRecord>> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let Some(mut session) = session_projection::session_summary(&store.sqlite, id, now)?
        .map(|session| summary_surface(&session, ending_here(&store.sqlite, &session, now)))
    else {
        return Ok(turns::raw(&store.sqlite, None)?
            .into_iter()
            .find(|raw| raw.id.as_str() == id));
    };
    turns::read(&store.sqlite, std::slice::from_mut(&mut session))?;
    Ok(Some(session))
}

/// Whether `session` ran on the `--on` harness, and on its model when one is named.
fn runs_on(on: &crate::config::Agent, session: &crate::session::SessionSummary) -> bool {
    session.provider.as_deref() == Some(on.harness.as_str())
        && (on.model.is_none() || on.model == session.model)
}

/// Passive listing reads record metadata and exact local client receipts only.
/// Connect/complete still enter find_session and surface, with full validation.
fn summary_surface(
    session: &crate::session::SessionSummary,
    ending: SessionEnding,
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
    let SessionEnding {
        ended,
        start,
        outlook,
        status_line,
    } = ending;
    // Without a command that opens it, nothing here can open it.
    let (action, open_argv) = match human_session_argv(&session.id) {
        Ok(argv) => (session_action(session), argv),
        Err(error) => {
            tracing::warn!(%error, session = %session.id, "Session connection unavailable");
            (None, Vec::new())
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
        task_ids: session.member_task().cloned().into_iter().collect(),
        id: session.id.clone(),
        workspace: None,
        interactive: session.interactive,
        work,
        wave_id: session.wave_id.clone(),
        work_path,
        action,
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
        cwd: session.cwd.display().to_string(),
        state,
        open_argv,
        terminal_ids: session.terminal_id.clone().into_iter().collect(),
        turn: None,
        raw: false,
        ended,
        start,
        outlook,
        status_line,
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

/// The fresh launch of a conversation's prepared input, or of a saved Graph's
/// review, which the opening process runs itself while holding `launch_lock`.
async fn fresh_launch(
    store: &SharedStore,
    session: &LfSession,
    launch_lock: File,
    kind: crate::harness::LaunchKind,
) -> Result<ConversationLaunch> {
    let config = crate::agent::resolve_session_config(session, serde_yaml_ng::Value::Null, None)?;
    let work = match work_selector(session) {
        Some(selector) => crate::ops::resolve_work_binding(store, &session.cwd, &selector)
            .await
            .ok(),
        None => None,
    };
    let input = session.input_key()?;
    Ok(ConversationLaunch {
        session_id: session.id.to_string(),
        agent: config.agent.to_string(),
        cwd: session.cwd.clone(),
        work,
        skill: session.skill.clone(),
        first_turn: first_turn(session.request.as_deref()).to_owned(),
        prepared_capture: capture_is_prepared(input)?.then(|| input.clone()),
        lock: LaunchLock::new(launch_lock),
        kind,
    })
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
            source: crate::session_record::AttributionSource::Declared,
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

/// `lf resume`: open a Session here, doing what `resume_intent` says. A live
/// held terminal gains a view; a provider running in a client lf can't share
/// ends and continues here; an ended one resumes its provider from history.
/// A Task that never ran its Graph is set up first, unless another terminal
/// holds its conversation. With `fork`, the holder keeps it and a new Session
/// continues from its log. A Run is refused; it is never taken over or
/// resumed. Without `resume`, nothing launches.
#[allow(clippy::too_many_arguments)]
pub async fn open(
    store: &SharedStore,
    directory: &Path,
    session_id: &str,
    agent: Option<&str>,
    request_id: Option<&str>,
    fork: bool,
    resume: bool,
    viewer: crate::transport::ViewerOptions,
) -> Result<Opening> {
    use SessionResumeIntent::{NewThread, TakeOver, View};
    crate::agent_input_timing::record("connect_begin", session_id);
    let mut progress = resume.then(|| crate::startup::Startup::new("Preparing conversation"));
    let target = match find_session(store, session_id, true).await? {
        Some(target) => target,
        // Connecting is what brings a provider-started conversation in.
        None => provider_conversation::admit(store, session_id, directory)
            .await?
            .ok_or_else(|| session_not_found(session_id))?,
    };
    if let Some(refusal) = run_refusal(&store.sqlite, &target)? {
        bail!(refusal);
    }
    if fork {
        drop(progress);
        return self::fork(store, &target, resume, viewer).await;
    }
    if !resume {
        if invocation_intent(&target, &current_summary(store, &target.id)?, false, false)
            == NewThread
        {
            conversation_log::read(&store.sqlite, &target)?;
        }
        return surface(store, &target).await.map(Opening::Done);
    }
    // One launch lock covers the decision and the launch it chooses.
    let id = target.id.clone();
    let launch_lock =
        crate::journal::request::spawn_blocking(move || lock_session_process(&id)).await??;
    // A viewer of a held terminal attaches; the holder does the setup.
    let held_elsewhere = target.interactive
        && store
            .sqlite
            .session_terminal(&target.id)?
            .is_some_and(|terminal| crate::transport::current() != Some(&terminal));
    let session = match setup::owed(store, &target)? {
        Some(owed) if !held_elsewhere => {
            drop(progress.take());
            setup::finish(store, target, owed, request_id, agent).await?
        }
        _ => primary::admit_workspace(store, target).await?,
    };
    let summary = current_summary(store, &session.id)?;
    let intent = invocation_intent(&session, &summary, false, false);
    drop(progress);
    // Another harness starts a new thread from the log.
    if intent == NewThread {
        drop(launch_lock);
        let (_, launch) = new_thread(
            store,
            &session.id,
            handoff_agent(&session).as_deref(),
            true,
            viewer,
            &uuid::Uuid::new_v4().to_string(),
        )
        .await?;
        let session = store
            .session(&session.id)
            .await?
            .ok_or_else(|| session_not_found(&session.id))?;
        return Ok(match launch {
            Some(launch) => Opening::Launch(launch),
            None => Opening::Done(surface(store, &session).await?),
        });
    }
    // A held terminal is attached while its child runs, answering or
    // not: a stopped server never means a second agent for the Session.
    if intent == View {
        if let Some(terminal) = store.sqlite.session_terminal(&session.id)? {
            drop(launch_lock);
            // A delegated launch that lost the race to another terminal
            // ends here; its caller joins that terminal instead of
            // leaving a view nested inside a held terminal nobody sees.
            if crate::transport::current().is_none() {
                // The newest view shows where the conversation runs,
                // not where it was launched.
                if let Some(attachment) = store.sqlite.session_attachment(&session.id)? {
                    store.sqlite.record_session_terminal(
                        &session.id,
                        &attachment,
                        crate::session_record::current_terminal_id()
                            .or_else(crate::terminal_host::host_placement)
                            .as_deref(),
                    )?;
                }
                crate::transport::attach(&terminal, Some(&session.id), viewer)?;
            }
            return surface(store, &session).await.map(Opening::Done);
        }
    }
    let result = surface(store, &session).await?;
    let Some((kind, launch_lock)) =
        crate::transport::delegate_session(&session.id, viewer, session.interactive, launch_lock)?
    else {
        return Ok(Opening::Done(result));
    };
    // Opening takes over a client lf can't share, `lf <skill>`'s own
    // terminal included, and a held terminal whose view ended under it.
    if intent == TakeOver {
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
    Ok(
        match open_waiting_locked(store, session, launch_lock, kind).await? {
            Some(launch) => Opening::Launch(launch),
            None => Opening::Done(result),
        },
    )
}

/// The provider thread this input can resume: the one recorded for it, if the
/// provider saved its transcript. A launch that ended before the provider
/// wrote anything, or a transcript deleted since, leaves none, and the
/// conversation starts again from its first request.
fn saved_thread(
    store: &SharedStore,
    artifact_key: &str,
    provider: &str,
) -> Result<Option<loopflow_storage::session_record::ProviderSessionRef>> {
    let Some(thread) = store.sqlite.input_provider_session(artifact_key)? else {
        return Ok(None);
    };
    if !crate::transcript::thread_lost(&store.sqlite, provider, &thread.agent_session)? {
        return Ok(Some(thread));
    }
    crate::journal::request::diagnostic(format_args!(
        "This conversation has no saved {provider} transcript; starting it again from its first request."
    ));
    Ok(None)
}

fn current_summary(store: &SharedStore, id: &str) -> Result<crate::session::SessionSummary> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    session_projection::session_summary(&store.sqlite, id, now)?
        .ok_or_else(|| session_not_found(id))
}

/// What this invocation of `lf resume` does with `session`: another harness
/// starts a new thread from its log, and preparation alone launches nothing,
/// so it never views or takes over a running holder.
pub(super) fn invocation_intent(
    session: &LfSession,
    summary: &crate::session::SessionSummary,
    fork: bool,
    prepare_only: bool,
) -> SessionResumeIntent {
    if !fork && !prepare_only && summary.has_provider_history && handoff_agent(session).is_some() {
        return SessionResumeIntent::NewThread;
    }
    match resume_intent(summary) {
        SessionResumeIntent::View | SessionResumeIntent::TakeOver if prepare_only => {
            if summary.has_provider_history {
                SessionResumeIntent::Resume
            } else {
                SessionResumeIntent::Start
            }
        }
        intent => intent,
    }
}

/// A new conversation in the same Work, seeded with `source`'s log. The
/// source and whatever holds it are left alone.
async fn fork(
    store: &SharedStore,
    source: &LfSession,
    launch: bool,
    viewer: crate::transport::ViewerOptions,
) -> Result<Opening> {
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
            completed_at: None,
            created_at: loopflow_storage::rows::now_unix(),
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
    store.sqlite.record_provider_lineage(
        &next,
        &session_projection::provider_lineage(&store.sqlite, source)?,
    )?;
    let record = surface(store, &next).await?;
    if launch {
        if let Some(launch) = open_prepared(store, &next.id, viewer).await? {
            return Ok(Opening::Launch(launch));
        }
    }
    Ok(Opening::Done(record))
}

/// Preparation has already mutated the Session. Only its resulting identity
/// crosses to the launch; opening it reacquires the lock and reads current state.
/// Its new thread has no native history, so it launches with the Session's Work.
async fn open_prepared(
    store: &SharedStore,
    id: &str,
    viewer: crate::transport::ViewerOptions,
) -> Result<Option<ConversationLaunch>> {
    match crate::transport::delegate_prepared_session(id, viewer)? {
        None => Ok(None),
        Some(kind) => open_waiting(store, id, kind).await,
    }
}

/// Whether the Session's agent is running, so it reads a steer at its next
/// boundary.
pub fn agent_running(store: &SqliteStore, session: &LfSession) -> Result<bool> {
    let summary = session_projection::session_summary(
        store,
        &session.id,
        loopflow_storage::rows::now_unix(),
    )?
    .ok_or_else(|| session_not_found(&session.id))?;
    Ok(summary.completed_at.is_none()
        && summary.agent_process_evidence == crate::store::ProcessIdentityEvidence::Live)
}

/// What a headless Session is called: a worker runs a Graph step, a batch
/// runs alone (`lf -b`).
fn headless_kind(session: &LfSession) -> &'static str {
    if session.graph_lf_process_id.is_some() {
        "worker"
    } else {
        "batch"
    }
}

/// Why `lf resume` refuses a worker or a batch, running or finished: it opens
/// only work sessions and chats. None for an interactive Session.
pub fn run_refusal(store: &SqliteStore, session: &LfSession) -> Result<Option<String>> {
    if session.interactive {
        return Ok(None);
    }
    let summary = session_projection::session_summary(
        store,
        &session.id,
        loopflow_storage::rows::now_unix(),
    )?
    .ok_or_else(|| session_not_found(&session.id))?;
    let running = summary.completed_at.is_none()
        && summary.agent_process_evidence == crate::store::ProcessIdentityEvidence::Live;
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
        "{} is a {} ({}); `lf resume` opens only work sessions and chats. {pointer}.",
        session.id,
        headless_kind(session),
        what.join(" · ")
    )))
}

/// The terminal this process runs in, named for the one a move leaves.
fn departure_here() -> crate::process::Departure {
    let to = if crate::session_record::current_terminal_id().is_some() {
        Some("Desktop".to_string())
    } else if let Some(placement) = crate::terminal_host::host_placement() {
        Some(placement.replacen(':', " ", 1))
    } else {
        crate::session_record::stdin_tty().map(|tty| tty.trim_start_matches("/dev/").to_string())
    };
    crate::process::Departure::Terminal { to }
}

/// Open a conversation: resume its native history, else
/// launch its prepared input, else append another attempt to the Session.
async fn open_waiting(
    store: &SharedStore,
    id: &str,
    kind: crate::harness::LaunchKind,
) -> Result<Option<ConversationLaunch>> {
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
    let session = primary::admit_workspace(store, session).await?;
    open_waiting_locked(store, session, launch_lock, kind).await
}

/// `open_waiting` for a Session admitted to its workspace under `launch_lock`.
async fn open_waiting_locked(
    store: &SharedStore,
    session: LfSession,
    launch_lock: File,
    kind: crate::harness::LaunchKind,
) -> Result<Option<ConversationLaunch>> {
    let id = session.id.clone();
    let mut launch_lock = Some(launch_lock);
    let session = if session.captured.is_none() {
        append_input(store, session).await?
    } else {
        session
    };
    // An input launches on the provider it recorded, so another harness
    // selected here appends its own instead of reopening native history.
    let handoff = handoff_agent(&session).is_some();
    if !handoff && resume_native_session(store, session.input_key()?, &mut launch_lock, kind)? {
        return Ok(None);
    }
    crate::agent_input_timing::record("fresh_launch_selected", &id);
    // A consumed launch without native history gets another attempt under
    // the same Session; its title and feedback never leave the row.
    let session = if handoff {
        append_input(store, session).await?
    } else if capture_is_prepared(session.input_key()?)? {
        session
    } else if session.input_published {
        append_input(store, session).await?
    } else {
        publish_prepared_input(
            &store.sqlite,
            &session,
            crate::session_record::SessionGraphMembership::Independent,
        )?
    };
    fresh_launch(
        store,
        &session,
        launch_lock.expect("an unresumed Session retains its launch lock"),
        kind,
    )
    .await
    .map(Some)
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

fn switch_agent(session: &mut LfSession, agent: &str) {
    let (provider, model) = crate::config::parse_agent(agent);
    session.provider = Some(provider);
    session.model = model;
}

/// Start a fresh provider thread on the same Session, on `agent` or its
/// current provider, seeded with a log of the current thread, or its original
/// request when that thread's transcript is gone. The Session
/// keeps its name, Work and feedback; the old thread stays in its history.
/// Without `launch`, the new input waits for its first open.
pub async fn new_thread(
    store: &SharedStore,
    id: &str,
    agent: Option<&str>,
    launch: bool,
    viewer: crate::transport::ViewerOptions,
    request_id: &str,
) -> Result<(LfSession, Option<ConversationLaunch>)> {
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
        if receipt.disposition == loopflow_storage::sqlite::creation::RecoveryDisposition::Inspect {
            return Err(crate::engine::creation::unknown(&receipt).into());
        }
        let next = store
            .session(id)
            .await?
            .ok_or_else(|| session_not_found(id))?;
        if receipt.disposition == loopflow_storage::sqlite::creation::RecoveryDisposition::Continue
        {
            // The input and receipt committed together, before filesystem publication.
            // Only that exact input may finish publishing; never replace a newer one.
            if next.artifact_key != receipt.artifact_key {
                return Err(crate::engine::creation::unknown(&receipt).into());
            }
            return Ok((finish_thread_creation(store, next, request_id).await?, None));
        }
        return Ok((next, None));
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
        Some(key) => saved_thread(store, key, &from)?.is_some(),
        None => false,
    };
    // A conversation with no provider history, or whose transcript is gone,
    // carries its original first prompt.
    let request = if threaded {
        Some(conversation_log::continuation(&store.sqlite, &session)?)
    } else {
        session.request.clone()
    };
    let lineage = threaded
        .then(|| session_projection::provider_lineage(&store.sqlite, &session))
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
        switch_agent(&mut next, agent);
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
    let seed = match threaded {
        true => format!("what the {from} thread knew"),
        false => "its original request".to_owned(),
    };
    crate::journal::request::diagnostic(format_args!(
        "Started a new {} thread for {}, seeded with {seed}.",
        next.provider.as_deref().unwrap_or("provider"),
        next.title
    ));
    drop(launch_lock);
    let launch = match launch {
        true => open_prepared(store, &next.id, viewer).await?,
        false => None,
    };
    Ok((next, launch))
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
/// The input records the agent it launches: another harness this invocation
/// selected moves the Session to it.
pub async fn append_input(store: &SharedStore, mut next: LfSession) -> Result<LfSession> {
    let replaced = next.captured;
    if let Some(agent) = handoff_agent(&next) {
        switch_agent(&mut next, &agent);
    }
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

/// One Session's summary, with its viewers confirmed by its held terminal.
/// The server records the terminal's viewers, but a server from an earlier lf
/// cannot write a store a newer lf has advanced: after an upgrade and a
/// Desktop restart its record still says viewed. Its attached views are the
/// truth, and a terminal that does not answer has no viewer anyone can reach.
pub(crate) fn viewed_summary(
    store: &SqliteStore,
    id: &str,
) -> Result<Option<crate::session::SessionSummary>> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let Some(mut summary) = session_projection::session_summary(store, id, now)? else {
        return Ok(None);
    };
    let released = !summary.unviewed
        && summary.agent_process_evidence == crate::store::ProcessIdentityEvidence::Live
        && store
            .session_terminal(id)?
            .is_some_and(|terminal| !matches!(crate::transport::viewed(&terminal), Ok(true)));
    if released {
        summary.unviewed = true;
        summary.terminal_id = None;
    }
    Ok(Some(summary))
}

/// A Session as its row and its current input describe it.
pub async fn surface(store: &SharedStore, session: &LfSession) -> Result<SessionRecord> {
    let work = match (&session.task_id, &session.wave_id) {
        (Some(task), _) => Some(WorkRef::Task(task.clone())),
        (None, Some(wave)) => Some(WorkRef::Wave(wave.clone())),
        (None, None) => None,
    };
    let metadata = viewed_summary(&store.sqlite, &session.id)?
        .ok_or_else(|| session_not_found(&session.id))?;
    let state = session_state(&metadata);
    let SessionEnding {
        ended,
        start,
        outlook,
        status_line,
    } = ending_here(
        &store.sqlite,
        &metadata,
        time::OffsetDateTime::now_utc().unix_timestamp(),
    );
    let action = session_action(&metadata);
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
        task_ids: metadata.member_task().cloned().into_iter().collect(),
        id: session.id.clone(),
        workspace: None,
        interactive: session.interactive,
        wave_id: session.wave_id.clone(),
        work_path: session_work_path(store, session).await?,
        work,
        action,
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
        cwd: session.cwd.display().to_string(),
        state,
        terminal_ids: metadata.terminal_id.clone().into_iter().collect(),
        open_argv,
        turn: None,
        raw: false,
        ended,
        start,
        outlook,
        status_line,
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

pub fn local_capture_dir(artifact_key: &str) -> Option<PathBuf> {
    crate::session_record::record_dir(&crate::store::lf_home_dir(), artifact_key)
}

/// Rename a Session and return the authoritative record.
/// `Generated` is an agent suggestion; it never replaces a human-assigned name.
pub async fn rename(
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
pub async fn bind(store: &SharedStore, id: &str, task: &str) -> Result<SessionRecord> {
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
    pub session_id: loopflow_model::id::LfSessionId,
    pub task_id: String,
    pub identifier: String,
    pub title: String,
}

pub async fn preview_binding(
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
    let task = match loopflow_model::durable::TaskId::parse(task) {
        Ok(id) => store.get_task(&id).await?,
        Err(_) => store.get_task_by_issue(task).await?,
    }
    .ok_or_else(|| anyhow!("Task {task:?} is not registered"))?;
    Ok((session, task))
}

/// Resolve a Session in any state by its durable or native conversation id.
pub async fn session_by_id(store: &SharedStore, id: &str) -> Result<Option<LfSession>> {
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
    launch_lock: &mut Option<File>,
    kind: crate::harness::LaunchKind,
) -> Result<bool> {
    let Some(session) = store.sqlite.session_for_artifact(artifact_key)? else {
        return Ok(false);
    };
    if session.artifact_key.as_ref() != Some(artifact_key) {
        bail!("Session {} changed its input before resume", session.id);
    }
    let native = NativeSession::of(&session)?;
    let provider_session = saved_thread(store, artifact_key, native.provider)?;
    if provider_session.is_none() {
        if !loopflow_harness::HarnessKind::parse(native.provider).is_some_and(|kind| {
            crate::harness::launcher(kind)
                .capabilities()
                .initial_prompt_on_resume
        }) {
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
    crate::harness::native_session::require_provider_session_process(&native.dir)?;
    crate::harness::agent_process::stop_session_agent(
        &store.sqlite,
        &session.id,
        departure_here(),
    )?;
    crate::agent_input_timing::record("native_resume_selected", &session.id);
    crate::harness::native_session::resume_session_with_env(
        session.input_key()?,
        provider_session.as_ref(),
        &BTreeMap::new(),
        launch_lock.take(),
        kind,
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

pub fn lock_session_process(id: &str) -> Result<File> {
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

/// One Program Status reading from the terminal showing a Session. `stream`
/// names one observer; its first reading (`sequence` 1) fences any earlier
/// stream, even on the same provider. It owns no Session and no provider.
/// A held terminal's server records its Session's status; a view's reading
/// of one is ignored.
pub async fn record_program_status(
    store: &SharedStore,
    id: &str,
    terminal: &str,
    agent_process: Option<&loopflow_model::id::AgentProcessId>,
    stream: &str,
    sequence: i64,
    records: &loopflow_model::program_status::Records,
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
        let current = session_projection::session_summary(
            &store.sqlite,
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
pub mod tests;
