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

use crate::durable::{FlowSession, WorkRef};
use crate::engine::Skill;
use crate::session::{AgentSession, WorkSource};
use crate::session_record::{SessionCaptureManifest, SessionTitleSource};
use crate::store::SharedStore;
use crate::work::task::{Task, TaskId};

#[cfg(test)]
pub(crate) mod action_test;
pub(crate) mod primary;
pub(crate) use primary::ensure_scope_worktree;
pub(crate) mod provider_conversation;

/// Choose by native human input, falling back to each Session's last opening.
pub async fn latest_interactive_session(
    store: &SharedStore,
    cwd: &Path,
) -> Result<Option<AgentSession>> {
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
pub(crate) const PREPARED_CAPTURE_ENV: &str = "LF_HUMAN_SESSION_RUN";
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
    StandaloneFlow {
        id: String,
    },
    /// A conversation; no caller or Flow waits on it.
    Primary {
        id: String,
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
    /// Passive metadata has no trustworthy live/closed observation.
    Unknown,
    Waiting,
    Active,
    Closed,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Flow,
    Conversation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionActionKind {
    Open,
    MoveHere,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAction {
    pub kind: SessionActionKind,
    pub label: String,
    pub help: String,
    pub unavailable_reason: Option<String>,
}

pub(crate) fn session_actions(kind: SessionKind, state: SessionState) -> Vec<SessionAction> {
    use SessionActionKind::{MoveHere, Open};
    let active_client = kind == SessionKind::Conversation && state == SessionState::Active;
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
            "Stop the other client and resume here; unsent text there is lost",
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
pub struct SessionPage {
    pub entries: Vec<SessionRecord>,
    pub next: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub primary_scope: Option<String>,
    /// A current obligation, independent of process liveness.
    pub attention: Option<SessionAttention>,
    pub task_ids: Vec<crate::durable::TaskId>,
    pub id: String,
    pub workspace: Option<SessionWorkspace>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionAttention {
    Review,
    Reply,
}

fn session_state(session: &crate::session::SessionSummary, has_clients: bool) -> SessionState {
    if session.completed_at.is_some() {
        SessionState::Closed
    } else if has_clients {
        SessionState::Active
    } else if session.driver_outcome.as_deref() == Some("interrupted") {
        SessionState::Interrupted
    } else if session.kind != crate::session::SessionKind::Conversation {
        SessionState::Waiting
    } else {
        SessionState::Unknown
    }
}

fn session_attention(session: &crate::session::SessionSummary) -> Option<SessionAttention> {
    if session.completed_at.is_some() {
        return None;
    }
    let current_review = session.kind == crate::session::SessionKind::FlowReview
        && !session.task_terminal
        && session.flow.as_ref().is_some_and(|flow| {
            flow.state == crate::session::FlowSummaryState::Current
                && flow.pending_session.as_deref() == Some(&session.id)
        });
    if current_review {
        Some(SessionAttention::Review)
    } else if session.interactive
        && session.latest_turn.as_deref() == Some("completed")
        && session.driver_outcome.is_none()
    {
        Some(SessionAttention::Reply)
    } else {
        None
    }
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
    /// An invocation that has finished or been replaced by a restart.
    Past,
}

/// One resolved Session. Every operation looks a Session up by its id, or by
/// its linked Run id, through `find_session`.
#[derive(Debug)]
enum SessionTarget {
    /// An interactive Session or a saved Flow's review.
    Row { session: Box<AgentSession> },
    /// A Task's review also settles its Task's invocation.
    Flow {
        task: Box<Task>,
        position: Box<FlowSession>,
        run_id: String,
    },
}

pub(crate) fn publish_prepared_input(
    store: &SharedStore,
    session: &AgentSession,
    flow: crate::session_record::SessionFlowMembership,
) -> Result<AgentSession> {
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
            subjects: work_selector(session)
                .map(|selector| crate::session_record::SubjectAttribution {
                    selector,
                    source: if session.work_source == Some(WorkSource::Inherited) {
                        crate::session_record::AttributionSource::Inherited
                    } else {
                        crate::session_record::AttributionSource::Declared
                    },
                })
                .into_iter()
                .collect(),
            flow,
            work: None,
        },
        session.caller_artifact_key.clone(),
        session.artifact_key.clone(),
    )?;
    store
        .sqlite
        .publish_capture(&session.id, session.captured)?;
    store
        .sqlite
        .session(&session.id)?
        .ok_or_else(|| session_not_found(&session.id))
}

/// The Task whose own Flow waits at this Run's review: the Run's invocation is
/// the one the Task points at. Any other review naming a Task is a Flow about it.
async fn managed_review(
    store: &SharedStore,
    session: &AgentSession,
) -> crate::store::StoreResult<Option<(crate::durable::TaskId, FlowSession)>> {
    if session.kind != crate::session::SessionKind::FlowReview {
        return Ok(None);
    }
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
    for session in store.session_summaries(&selection).await? {
        sessions.push(summary_surface(&session));
    }
    workspace::associate(store, &mut sessions).await?;
    if association_filter {
        let task = if let Some(selector) = &filter.task {
            store
                .task_checkouts()
                .await?
                .into_iter()
                .find(|task| {
                    task.task_id.as_str() == selector
                        || task.issue_id == *selector
                        || task.issue_identifier == *selector
                })
                .map(|task| task.task_id)
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

/// Filter before applying the caller's page size, using the same attention
/// metadata reading. Scan bounded metadata pages, never provider transcripts.
pub(crate) async fn list_attention(
    store: &SharedStore,
    filter: &crate::session::SessionFilter,
) -> Result<Vec<SessionRecord>> {
    let mut page = filter.clone();
    page.after = Some(filter.after.clone().unwrap_or_default());
    page.offset = 0;
    page.limit = 100;
    let mut skip = filter.offset;
    let mut matches = Vec::new();
    loop {
        let rows = list(store, &page).await?;
        let exhausted = rows.len() < page.limit;
        page.after = rows.last().map(|row| row.id.clone());
        for row in rows.into_iter().filter(|row| row.attention.is_some()) {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            matches.push(row);
            if filter.limit > 0 && matches.len() == filter.limit {
                return Ok(matches);
            }
        }
        if exhausted {
            return Ok(matches);
        }
    }
}

/// Passive listing reads record metadata and exact local client receipts only.
/// Connect/complete still enter owned_target and surface, with full validation.
fn summary_surface(session: &crate::session::SessionSummary) -> SessionRecord {
    use crate::session::FlowSummaryState;
    let kind = match session.kind {
        crate::session::SessionKind::Conversation => SessionKind::Conversation,
        crate::session::SessionKind::FlowReview => SessionKind::Flow,
    };
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
    let flow_membership = match (&session.flow_session_id, &session.flow) {
        (None, _) if session.independent => SessionFlowMembership::Independent,
        (None, _) => SessionFlowMembership::Unknown {
            reason: "Flow membership was not recorded".into(),
        },
        (Some(id), Some(flow)) if flow.name.is_some() => SessionFlowMembership::Step {
            flow: flow.name.clone().expect("matched a recorded Flow name"),
            invocation_id: id.clone(),
            step: session.skill.clone().unwrap_or_default(),
            // Recorded membership is not revalidated by opening the capture.
            // Detail/actions validate that exact occurrence before using it.
            node: session.node,
            iterations: session.iterations.clone(),
            occurrence: if flow.state != FlowSummaryState::Current {
                SessionFlowOccurrence::Past
            } else if flow.current_capture == session.captured {
                SessionFlowOccurrence::Current
            } else if flow.current_capture.is_some() {
                SessionFlowOccurrence::Earlier
            } else if flow.pending_session.as_deref() == Some(session.id.as_str()) {
                SessionFlowOccurrence::Current
            } else {
                SessionFlowOccurrence::Unknown
            },
        },
        (Some(id), _) => SessionFlowMembership::Unknown {
            reason: format!("Flow {id} metadata is unavailable"),
        },
    };
    let mut unavailable = None;
    let remote = if session.managed {
        match (&session.home_id, &session.home_route) {
            (Some(id), Some(route)) => (route != "local").then_some(id),
            _ => {
                unavailable = Some("Session Home placement is unavailable".to_string());
                None
            }
        }
    } else {
        None
    };
    let clients = if remote.is_none() && unavailable.is_none() {
        match (
            &session.provider,
            local_session_run_dir(&session.artifact_key),
        ) {
            (Some(provider), Some(dir)) => {
                match crate::lf::commands::util::active_provider_clients(&dir, provider) {
                    Ok(clients) => clients,
                    Err(error) => {
                        unavailable =
                            Some(format!("Session client observation unavailable: {error}"));
                        Vec::new()
                    }
                }
            }
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };
    let state = session_state(session, !clients.is_empty());
    let mut actions = session_actions(kind, state);
    let open_argv = if unavailable.is_none() {
        match human_open_argv(remote, Some(&session.cwd), &session.id) {
            Ok(argv) => argv,
            Err(error) => {
                unavailable = Some(format!("Session connection unavailable: {error}"));
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    if let Some(reason) = unavailable {
        for action in &mut actions {
            action.unavailable_reason = Some(reason.clone());
        }
    }
    let provider = session.provider.clone().unwrap_or_default();
    SessionRecord {
        primary_scope: session.primary_scope.clone(),
        attention: session_attention(session),
        task_ids: session.task_ids.clone(),
        id: session.id.clone(),
        workspace: remote.map(|home| SessionWorkspace {
            home_id: home.clone(),
            worktree: session.cwd.clone(),
            task_id: session.task_id.clone(),
            unavailable: Some("Checkout resolution is unavailable on this remote Home".into()),
        }),
        kind,
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
        detail: match (kind, &session.skill) {
            (SessionKind::Conversation, _) => session
                .model
                .as_ref()
                .map_or(provider.clone(), |model| format!("{provider}:{model}")),
            (_, Some(skill)) => skill.clone(),
            (_, None) => "Request for input".into(),
        },
        provider: session.provider.clone(),
        cwd: session.cwd.display().to_string(),
        state,
        ready_summary: session.ready_summary.clone(),
        open_argv,
        terminal_ids: clients
            .into_iter()
            .filter_map(|client| client.terminal_id)
            .collect(),
    }
}

/// Resolve a Session id, the Run id linked to it, or the provider's own id for
/// its conversation. A conversation or Flow Run names its waiting boundary, so
/// `$LF_RUN_ID` inside a review targets it.
async fn find_session(
    store: &SharedStore,
    session_id: &str,
    open_completed: bool,
) -> Result<Option<SessionTarget>> {
    // Membership outlives a pending boundary. Never reinterpret a retained
    // attempt's manifest as an independent conversation or current actor.
    let owned = match crate::session_record::parse_artifact_key(session_id) {
        Ok(run_id) => store.session_for_artifact(&run_id).await?,
        Err(_) => store.session(session_id).await?,
    };
    if let Some(session) = owned {
        return owned_target(store, session_id, session, open_completed)
            .await
            .map(Some);
    }
    let selected = match store.sqlite.resolve_history_input(session_id) {
        Ok(selected) => selected,
        Err(crate::store::StoreError::NotFound) => {
            let Some(session) = provider_conversation::recorded(store, session_id).await? else {
                return Ok(None);
            };
            let id = session.id.clone();
            return owned_target(store, &id, session, open_completed)
                .await
                .map(Some);
        }
        Err(error) => return Err(error.into()),
    };
    let input = crate::session_record::parse_artifact_key(&selected)?;
    let Some(session) = store.session_for_artifact(&input).await? else {
        bail!("Input {input} does not belong to a Session");
    };
    owned_target(store, input.as_str(), session, open_completed)
        .await
        .map(Some)
}

async fn owned_target(
    store: &SharedStore,
    selector: &str,
    session: crate::session::AgentSession,
    open_completed: bool,
) -> Result<SessionTarget> {
    if selector != session.id && selector != session.artifact_key.as_str() {
        bail!(
            "Run {selector} is a historical attempt of Session {}; current Run is {}",
            session.id,
            session.artifact_key
        );
    }
    if session.completed_at.is_some()
        && !(open_completed && session.kind == crate::session::SessionKind::Conversation)
    {
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
    if latest.artifact_key != session.artifact_key
        || (session.input_published
            && position.review_artifact_key() != Some(&session.artifact_key))
    {
        bail!(
            "Session {} changed its current Run during lookup; open the Session again",
            session.id
        );
    }
    Ok(SessionTarget::Flow {
        run_id: session.artifact_key.clone(),
        task: Box::new(task),
        position: Box::new(position),
    })
}

/// The provider and local client receipts for a conversation's current input.
struct NativeSession<'a> {
    dir: PathBuf,
    provider: &'a str,
}

impl<'a> NativeSession<'a> {
    fn of(session: &'a AgentSession) -> Result<Self> {
        Ok(Self {
            dir: local_session_run_dir(&session.artifact_key)
                .ok_or_else(|| anyhow!("Session {} has an invalid Run reference", session.id))?,
            provider: session
                .provider
                .as_deref()
                .ok_or_else(|| anyhow!("Session {} Run has no recorded provider", session.id))?,
        })
    }

    fn clients(&self) -> Result<Vec<crate::session_record::ProviderClientRef>> {
        crate::lf::commands::util::active_provider_clients(&self.dir, self.provider)
    }

    fn stop_clients(&self, reason: crate::session_record::ProviderClientStopReason) -> Result<()> {
        crate::lf::commands::util::replace_provider_clients(
            &self.dir,
            self.provider,
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

pub(crate) async fn serve_conversation(store: &SharedStore, run_id: &String) -> Result<()> {
    let session = store
        .session_for_artifact(run_id)
        .await?
        .ok_or_else(|| anyhow!("Conversation Run {run_id} no longer exists"))?;
    let launch_lock = lock_session_exec(&session.id)?;
    let session = store
        .session(&session.id)
        .await?
        .ok_or_else(|| anyhow!("Conversation Run {run_id} no longer exists"))?;
    if session.completed_at.is_some() {
        bail!("session {:?} is already resolved", session.id);
    }
    if session.artifact_key != *run_id || !capture_is_prepared(&session.artifact_key)? {
        // Another launcher already published this Session. Do not replace or
        // infer death of its native client; explicit open owns native resume.
        return Ok(());
    }
    serve_locked(store, &session, launch_lock).await
}

/// What a waiting Session's agent carries to act on its own Session.
fn session_token(session: &AgentSession) -> HumanSessionToken {
    match session.kind {
        crate::session::SessionKind::FlowReview => HumanSessionToken::StandaloneFlow {
            id: session.id.clone(),
        },
        // Prepared conversations have no Flow settlement authority.
        crate::session::SessionKind::Conversation => HumanSessionToken::Primary {
            id: session.id.clone(),
        },
    }
}

/// Launch the prepared Run of a conversation or of a saved Flow's review.
async fn serve_locked(
    store: &SharedStore,
    session: &AgentSession,
    launch_lock: File,
) -> Result<()> {
    let lf = crate::engine::process::resolve_pinned_lf_binary()?;
    let mut command = tokio::process::Command::new(lf);
    let token = session_token(session);
    command
        .current_dir(&session.cwd)
        .env(HUMAN_SESSION_ENV, serde_json::to_string(&token)?);
    command.args(conversation_launch_args(store, session).await);
    let mut child = spawn_session_exec(&mut command, &session.artifact_key).await?;
    drop(launch_lock);
    // Provider termination never completes a Session.
    let status = child.wait().await.context("wait for session agent")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("session agent exited with {status}"))
    }
}

async fn conversation_launch_args(store: &SharedStore, session: &AgentSession) -> Vec<String> {
    let mut args = vec![
        "--mode".to_string(),
        "tui".to_string(),
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

fn work_selector(session: &AgentSession) -> Option<String> {
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
            .map_err(|_| anyhow!("prepared Run id is not UTF-8"))?,
    )?))
}

pub(crate) async fn open(
    store: &SharedStore,
    session_id: &str,
    mode: OpenMode,
    resume: bool,
) -> Result<SessionRecord> {
    let target = match find_session(store, session_id, true).await? {
        Some(target) => target,
        // Connecting is what brings a provider-started conversation in.
        None => SessionTarget::Row {
            session: Box::new(
                provider_conversation::admit(store, session_id)
                    .await?
                    .ok_or_else(|| session_not_found(session_id))?,
            ),
        },
    };
    match &target {
        SessionTarget::Row { session }
            if session.kind == crate::session::SessionKind::Conversation =>
        {
            let admitted;
            let session = if resume {
                let id = session.id.clone();
                let _launch = tokio::task::spawn_blocking(move || lock_session_exec(&id)).await??;
                admitted = primary::admit_workspace(store, session.as_ref().clone()).await?;
                &admitted
            } else {
                session
            };
            let native = NativeSession::of(session)?;
            let Some(provider_session) =
                store.sqlite.input_provider_session(&session.artifact_key)?
            else {
                let surface = surface(store, session).await?;
                if resume {
                    open_waiting(store, &session.id).await?;
                }
                return Ok(surface);
            };
            if resume {
                crate::lf::commands::util::require_provider_session_exec(&native.dir)?;
            }
            if resume
                && native.provider == "codex"
                && connect_live_codex(
                    store,
                    session,
                    &native.dir,
                    &provider_session,
                    mode == OpenMode::Replace,
                )
                .await?
            {
                let session = store
                    .sqlite
                    .session(&session.id)?
                    .ok_or_else(|| session_not_found(&session.id))?;
                return surface(store, &session).await;
            }
            if resume && mode == OpenMode::Replace {
                native.stop_clients(crate::session_record::ProviderClientStopReason::Moved)?;
            }
            if mode == OpenMode::Refuse && !native.clients()?.is_empty() {
                require_session_action(
                    SessionKind::Conversation,
                    SessionState::Active,
                    SessionActionKind::Open,
                )?;
            }
            let mut result = surface(store, session).await?;
            if resume {
                crate::lf::commands::util::resume_session(
                    native.provider,
                    session.model.as_deref(),
                    &session.cwd,
                    &session.artifact_key,
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
            let id = flow_id(position)?;
            let lock_id = id.clone();
            let launch_lock =
                tokio::task::spawn_blocking(move || lock_session_exec(&lock_id)).await??;
            let current = find_session(store, session_id, false)
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
            let session = session_surface(store, &target).await?;
            if resume {
                open_flow_locked(store, task, position, launch_lock).await?;
            }
            Ok(session)
        }
        SessionTarget::Row { session } => {
            if mode != OpenMode::Refuse {
                bail!("--replace and --try apply only to interactive provider sessions");
            }
            let surface = surface(store, session).await?;
            if resume {
                open_waiting(store, &session.id).await?;
            }
            Ok(surface)
        }
    }
}

/// Connect to one existing provider thread. Native UI traffic crosses the same
/// driver fence as the headless writer; closing the current UI closes the runtime.
#[cfg(unix)]
async fn connect_live_codex(
    store: &SharedStore,
    session: &AgentSession,
    dir: &Path,
    provider: &crate::session_record::ProviderSessionRef,
    replace_clients: bool,
) -> Result<bool> {
    let expected = store.sqlite.session_driver(&session.id)?;
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
    let driver =
        match store
            .sqlite
            .claim_session_driver(&session.id, expected.as_ref(), &exec, false)
        {
            Ok(driver) => driver,
            Err(crate::store::StoreError::InvalidAuthority(_))
                if store.sqlite.session_connection(&session.id)?.is_none() =>
            {
                // Close won the race. The ordinary open path resumes saved history.
                return Ok(false);
            }
            Err(error) => return Err(error.into()),
        };
    crate::session_record::register_session_driver_interrupt(
        &store.sqlite,
        session.id.clone(),
        driver.clone(),
    );
    let connected = async {
        if replace_clients {
            NativeSession::of(session)?.stop_clients(crate::session_record::ProviderClientStopReason::Moved)?;
        }
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
                "codex", session.model.as_deref(), &session.cwd, &session.artifact_key, &dir, &provider,
                &BTreeMap::new(), None, Some(&remote),
            )
        }).await;
        relay.abort();
        let _ = relay.await;
        result??;
        Ok::<_, anyhow::Error>(true)
    }.await;
    match crate::session_record::finish_session_driver(
        &store.sqlite,
        &session.id,
        &driver,
        "completed",
    ) {
        Ok(_) | Err(crate::store::StoreError::InvalidAuthority(_)) => {}
        Err(error) => return Err(error.into()),
    }
    connected
}

/// Open a conversation or a saved Flow's review: resume its native history, else
/// launch its prepared Run, else append another attempt to the Session.
async fn open_waiting(store: &SharedStore, id: &str) -> Result<()> {
    let lock_id = id.to_string();
    let launch_lock = tokio::task::spawn_blocking(move || lock_session_exec(&lock_id)).await??;
    // Resolve the current input under the launch lock, including completion
    // or replacement that happened while this opener waited.
    let session = store
        .session(id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    if session.completed_at.is_some() {
        bail!("session {id:?} is already complete");
    }
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
        let flow = match &token {
            HumanSessionToken::StandaloneFlow { id } => {
                crate::ops::flow_session::membership(store, id).await?
            }
            _ => crate::session_record::SessionFlowMembership::Independent,
        };
        let replaced = session.captured;
        let mut next = session;
        if next.input_published {
            next.artifact_key = crate::session_record::new_artifact_key();
            next.input_published = false;
            next = store.replace_session_input(replaced, next).await?;
        }
        publish_prepared_input(store, &next, flow)?
    };
    serve_locked(
        store,
        &session,
        launch_lock.expect("an unresumed Session retains its launch lock"),
    )
    .await
}

async fn open_flow_locked(
    store: &SharedStore,
    task: &Task,
    position: &FlowSession,
    launch_lock: File,
) -> Result<String> {
    let previous = position.review_artifact_key().cloned();
    let token = flow_token(task, position)?;
    let mut launch_lock = Some(launch_lock);
    if let Some(run_id) = &previous {
        if resume_native_session(
            store,
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
        let input = store
            .sqlite
            .input_history(run_id.as_str())
            .context("cannot replace a review input without its retained launch evidence")?;
        let dir = local_session_run_dir(run_id)
            .ok_or_else(|| anyhow!("invalid Session input {run_id}"))?;
        if store.sqlite.input_provider_session(run_id)?.is_some()
            || !crate::lf::commands::util::active_provider_clients(&dir, &input.harness)?.is_empty()
        {
            bail!("review input still has native history or an active client");
        }
        if input.recorded_outcome.is_none() {
            bail!("review Run {run_id} has no terminal outcome; launch status is unresolved, so Open cannot authorize a replacement");
        }
    }
    let session = store
        .session(&flow_id(position)?)
        .await?
        .context("historical review conversation is missing")?;
    serve_locked(
        store,
        &session,
        launch_lock.expect("unresumed conversation retains its launch lock"),
    )
    .await?;
    Ok(session.artifact_key)
}

pub(crate) fn capture_is_prepared(run_id: &str) -> Result<bool> {
    match crate::session_record::resolve_manifest(&crate::store::lf_home_dir(), run_id) {
        Ok((dir, _)) => Ok(dir.join("prepared").is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).context("resolve prepared Session Run"),
    }
}

/// A Session as its row and its current Run describe it.
async fn surface(store: &SharedStore, session: &AgentSession) -> Result<SessionRecord> {
    let kind = match session.kind {
        crate::session::SessionKind::Conversation => SessionKind::Conversation,
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
    let dir = local_session_run_dir(&session.artifact_key)
        .ok_or_else(|| anyhow!("Session {} has an invalid Run reference", session.id))?;
    let clients = match &session.provider {
        Some(provider) if remote.is_none() => {
            crate::lf::commands::util::active_provider_clients(&dir, provider)?
        }
        _ => Vec::new(),
    };
    let metadata = store
        .sqlite
        .session_summary(&session.id)?
        .ok_or_else(|| session_not_found(&session.id))?;
    let state = session_state(&metadata, !clients.is_empty());
    let mut actions = session_actions(kind, state);
    let flow_membership = match &session.flow_session_id {
        None => match store.sqlite.session_summary(&session.id)? {
            Some(metadata) if metadata.independent => SessionFlowMembership::Independent,
            _ => SessionFlowMembership::Unknown {
                reason: "Flow membership was not recorded".into(),
            },
        },
        Some(id) => match store.flow(id).await {
            Ok(Some(flow)) => {
                let graph = crate::engine::flow_graph::FlowGraph::new(
                    &flow.invocation.flow,
                    &flow.invocation.steps,
                );
                let node = session
                    .node
                    .map(|id| {
                        graph.node_at(id).map(|node| node.key).ok_or_else(|| {
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
                        .is_some_and(|attempt| attempt.run_id == session.artifact_key)
                    {
                        SessionFlowOccurrence::Current
                    } else if flow.current_attempt.is_some() {
                        SessionFlowOccurrence::Earlier
                    } else if flow.pending_session_id.as_deref() == Some(session.id.as_str()) {
                        SessionFlowOccurrence::Current
                    } else {
                        SessionFlowOccurrence::Unknown
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
    let mut reading = SessionRecord {
        primary_scope: metadata.primary_scope.clone(),
        attention: session_attention(&metadata),
        task_ids: store.sqlite.session_task_ids(&session.id)?,
        id: session.id.clone(),
        workspace: remote.as_ref().map(|home| SessionWorkspace {
            home_id: home.clone(),
            worktree: session.cwd.clone(),
            task_id: session.task_id.clone(),
            unavailable: Some("Checkout resolution is unavailable on this remote Home".into()),
        }),
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
    };
    workspace::associate(store, std::slice::from_mut(&mut reading)).await?;
    Ok(reading)
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
        Some(wave) => wave.slug().to_string(),
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

pub(crate) fn local_session_run_dir(run_id: &str) -> Option<PathBuf> {
    crate::session_record::record_dir(&crate::store::lf_home_dir(), run_id)
}

/// Rename a Session through its Run and return the authoritative record.
/// `Generated` is an agent suggestion; it never replaces a human-assigned name.
/// The id may be the Session's own `$LF_RUN_ID`: a conversation or Flow Run names its
/// boundary, and naming needs no provider history.
pub(crate) async fn rename(
    store: &SharedStore,
    session_id: &str,
    title: &str,
    source: SessionTitleSource,
) -> Result<SessionRecord> {
    let mut target = find_session(store, session_id, false)
        .await?
        .ok_or_else(|| session_not_found(session_id))?;
    let title = crate::session_record::validate_session_title(title)
        .map_err(|error| anyhow!("cannot rename Session {session_id}: {error}"))?;
    let title_source = match source {
        SessionTitleSource::Human => crate::session::TitleSource::Human,
        SessionTitleSource::Generated => crate::session::TitleSource::Generated,
        SessionTitleSource::Unavailable => bail!("unavailable is not a title source"),
    };
    let (run_id, position) = match &target {
        SessionTarget::Row { session } => {
            // A Run selector names that attempt; the Session id names the conversation.
            let expected_run = (session_id != session.id)
                .then_some(session.captured)
                .flatten();
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
    let expected_run = if session_id != id {
        store.sqlite.captured_sequence(run_id)?
    } else {
        None
    };
    // Retain the caller's exact/prefix selector while waiting for replacement.
    let pending_lock = tokio::task::spawn_blocking(move || lock_session_exec(&id));
    let _launch_lock = pending_lock.await??;
    target = find_session(store, session_id, false)
        .await?
        .ok_or_else(|| session_not_found(session_id))?;
    if let SessionTarget::Flow { position, .. } = &target {
        store
            .rename_session(&flow_id(position)?, expected_run, title, title_source)
            .await?;
    }
    session_surface(store, &target).await
}

/// Assign a Task to future Session work. The id is the Session's
/// or any of its Runs'; a closed Session binds like an open one.
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
        session.id, task.plan.identifier, task.plan.title
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
        identifier: task.plan.identifier,
        title: task.plan.title,
    })
}

async fn binding_target(store: &SharedStore, id: &str, task: &str) -> Result<(AgentSession, Task)> {
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

/// The Session an id names whatever its state: its own id, any of its Runs',
/// or the provider's id for its conversation.
pub(crate) async fn session_by_id(store: &SharedStore, id: &str) -> Result<Option<AgentSession>> {
    if let Some(session) = store.session(id).await? {
        return Ok(Some(session));
    }
    if let Some(session) = store.session_for_artifact(id).await? {
        return Ok(Some(session));
    }
    provider_conversation::recorded(store, id).await
}

fn session_not_found(id: &str) -> anyhow::Error {
    anyhow!("Session {id} was not found")
}

pub(crate) async fn spawn_session_exec(
    command: &mut tokio::process::Command,
    run_id: &String,
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
    // never start; the opening Run must retain enough evidence to diagnose it.
    // Arguments and environment values can contain prompts or credentials.
    let launch = format!(
        "Session Run {run_id}: executable {} (sha256 {digest}), cwd {}, Home {}, database {}",
        executable.display(),
        cwd.display(),
        home.display(),
        database.display()
    );
    if capture_is_prepared(run_id)? {
        command.env(PREPARED_CAPTURE_ENV, run_id.as_str());
    }
    let mut child = command
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("could not start {launch}"))?;
    let deadline = tokio::time::Instant::now() + SESSION_START_TIMEOUT;
    loop {
        match crate::session_record::resolve_manifest(&home, run_id.as_str()) {
            Ok((dir, manifest)) => {
                if session_is_resumable(&dir, &manifest)? {
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

fn session_is_resumable(dir: &Path, manifest: &SessionCaptureManifest) -> Result<bool> {
    if crate::session_record::read_provider_session(dir)?.is_none() {
        return Ok(false);
    }
    Ok(!crate::lf::commands::util::active_provider_clients(dir, &manifest.harness)?.is_empty())
}

pub(crate) fn resume_native_session(
    store: &SharedStore,
    run_id: &String,
    token: &HumanSessionToken,
    launch_lock: &mut Option<File>,
) -> Result<bool> {
    #[cfg(test)]
    if let Some(result) = action_test::resume(run_id, launch_lock) {
        return result;
    }
    let Some(session) = store.sqlite.session_for_artifact(run_id)? else {
        return Ok(false);
    };
    if session.artifact_key != *run_id {
        bail!("Session {} changed its input before resume", session.id);
    }
    let native = NativeSession::of(&session)?;
    let Some(provider_session) = store.sqlite.input_provider_session(run_id)? else {
        return Ok(false);
    };
    crate::lf::commands::util::require_provider_session_exec(&native.dir)?;
    native.stop_clients(crate::session_record::ProviderClientStopReason::Moved)?;
    let environment =
        BTreeMap::from([(HUMAN_SESSION_ENV.to_string(), serde_json::to_string(token)?)]);
    crate::lf::commands::util::resume_session_with_env(
        native.provider,
        session.model.as_deref(),
        &session.cwd,
        &session.artifact_key,
        &native.dir,
        &provider_session,
        &environment,
        launch_lock.take(),
        None,
    )?;
    Ok(true)
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
    let context = crate::engine::process::execution_context()?;
    // A fresh terminal does not inherit the listing process's data selection.
    // Carry the executable and its data together, including when a different
    // installation becomes current between listing and opening.
    let mut argv = vec![
        "/usr/bin/env".to_string(),
        format!("LF_BIN={}", context.lf_bin.display()),
        format!("LF_HOME={}", context.lf_home.display()),
        context.lf_bin.display().to_string(),
    ];
    if let Some(home_id) = remote_home {
        argv.extend(["home".to_string(), "ssh".to_string()]);
        if let Some(worktree) = worktree {
            let repo = crate::engine::wave_home::resolve_home_relative_repo(worktree)
                .map_err(anyhow::Error::msg)?;
            argv.extend(["--repo".to_string(), repo]);
        }
        argv.push(home_id.to_string());
    }
    argv.extend(["session".to_string(), "connect".to_string(), id.to_string()]);
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

const PRIMARY_MESSAGE: &str = "<lf:primary-session>\nThis is the one ongoing primary conversation of its repository or Wave. Reconcile current evidence, then work with the user.\n</lf:primary-session>";

#[cfg(not(test))]
async fn conversation_exec_is_running(id: &str) -> Result<bool> {
    let status = tokio::process::Command::new("tmux")
        .args([
            "has-session",
            "-t",
            &format!("={}", conversation_background_name(id)),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .context("inspect conversation launcher")?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => bail!("inspect conversation launcher: tmux exited with {status}"),
    }
}

#[cfg(test)]
async fn conversation_exec_is_running(id: &str) -> Result<bool> {
    Ok(tests::CONVERSATION_LAUNCHERS
        .lock()
        .unwrap()
        .contains(&conversation_background_name(id)))
}

#[cfg(not(test))]
async fn start_durable_session(name: &str, cwd: &Path, argv: &[String]) -> Result<()> {
    crate::engine::process::start_home_session(name, cwd, argv).await
}

#[cfg(test)]
async fn start_durable_session(name: &str, _cwd: &Path, argv: &[String]) -> Result<()> {
    if !argv.iter().any(|arg| arg == "serve-conversation") {
        return Ok(());
    }
    if tests::FAILED_CONVERSATION_LAUNCHERS
        .lock()
        .unwrap()
        .contains(name)
    {
        bail!("simulated Session launch failure");
    }
    if !tests::CONVERSATION_LAUNCHERS
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

pub(crate) fn lock_session_exec(id: &str) -> Result<File> {
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

fn conversation_background_name(id: &str) -> String {
    format!("lf-human-{}", id.chars().take(12).collect::<String>())
}

fn active_session_token() -> Result<HumanSessionToken> {
    let raw =
        std::env::var(HUMAN_SESSION_ENV).context("this command requires an active session")?;
    serde_json::from_str(&raw).context("active session token is invalid")
}

#[doc(hidden)]
pub fn active_flow_skill(requested: &str) -> Result<Option<Skill>> {
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
        HumanSessionToken::Primary { .. } => Ok(None),
    }
}

#[cfg(test)]
mod tests {
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
    fn session_metadata_keeps_unknown_position_and_unavailable_placement_visible() {
        let task = TaskId::new();
        let wave = crate::id::WaveId::new();
        let mut summary = crate::session::SessionSummary {
            primary_scope: None,
            driver_outcome: None,
            latest_turn: None,
            task_terminal: false,
            task_ids: vec![task.clone()],
            captured: Some(1),
            id: "metadata".into(),
            artifact_key: crate::session_record::new_artifact_key(),
            title: "Retained conversation".into(),
            title_source: crate::session::TitleSource::Human,
            ready_summary: None,
            completed_at: None,
            kind: crate::session::SessionKind::Conversation,
            interactive: true,
            task_id: Some(task.clone()),
            wave_id: Some(wave.clone()),
            flow_session_id: Some("flow".into()),
            cwd: "/unavailable".into(),
            skill: Some("review".into()),
            provider: None,
            model: None,
            node: Some(2),
            iterations: None,
            flow: Some(crate::session::FlowSummary {
                id: "flow".into(),
                name: Some("retained".into()),
                state: crate::session::FlowSummaryState::Current,
                current_capture: None,
                pending_session: None,
                task_id: Some(task.clone()),
                wave_id: Some(wave.clone()),
                updated_at: 1,
            }),
            independent: false,
            wave_name: Some("Infrastructure".into()),
            task_identifier: Some("INF-123".into()),
            // Missing Home must never fall back to a local launch/observation.
            managed: true,
            home_id: None,
            home_route: None,
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
                occurrence: super::SessionFlowOccurrence::Unknown,
                ..
            }
        ));
        assert!(row.open_argv.is_empty());
        assert!(row
            .actions
            .iter()
            .all(|action| action.unavailable_reason.as_deref()
                == Some("Session Home placement is unavailable")));
        summary.flow.as_mut().unwrap().state = crate::session::FlowSummaryState::Completed;
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
        summary.flow_session_id = None;
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
        summary.driver_outcome = Some("interrupted".into());
        assert_eq!(
            super::summary_surface(&summary).state,
            super::SessionState::Interrupted
        );
        summary.kind = crate::session::SessionKind::FlowReview;
        summary.flow.as_mut().unwrap().state = crate::session::FlowSummaryState::Current;
        summary.flow.as_mut().unwrap().pending_session = Some(summary.id.clone());
        assert_eq!(
            super::summary_surface(&summary).attention,
            Some(super::SessionAttention::Review)
        );
        summary.task_terminal = true;
        assert_eq!(super::summary_surface(&summary).attention, None);
        summary.task_terminal = false;
        summary.completed_at = Some(1);
        assert_eq!(super::summary_surface(&summary).attention, None);
    }

    use std::collections::HashSet;
    use std::ffi::OsString;
    use std::sync::{LazyLock, Mutex};

    use super::{
        human_open_argv, session_is_resumable, FlowSessionToken, HumanSessionToken,
        HUMAN_SESSION_ENV,
    };
    use crate::durable::FlowSession;
    use crate::session::AgentSession;
    use crate::store::{open_ephemeral_store, SharedStore, StorageConfig};
    use crate::work::task::TaskId;

    pub(super) static CONVERSATION_LAUNCHERS: LazyLock<Mutex<HashSet<String>>> =
        LazyLock::new(|| Mutex::new(HashSet::new()));
    pub(super) static FAILED_CONVERSATION_LAUNCHERS: LazyLock<Mutex<HashSet<String>>> =
        LazyLock::new(|| Mutex::new(HashSet::new()));

    pub(super) struct SessionHome {
        home: tempfile::TempDir,
        previous: Vec<(&'static str, Option<OsString>)>,
        _ambient: crate::test_ambient::EnvGuard,
    }

    impl SessionHome {
        pub(super) fn new() -> Self {
            CONVERSATION_LAUNCHERS.lock().unwrap().clear();
            FAILED_CONVERSATION_LAUNCHERS.lock().unwrap().clear();
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
            CONVERSATION_LAUNCHERS.lock().unwrap().clear();
            FAILED_CONVERSATION_LAUNCHERS.lock().unwrap().clear();
        }
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
        let session = |task_id: Option<crate::work::task::TaskId>| AgentSession {
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
                sources: Vec::new(),
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
        use crate::session_record::SessionFlowStep;

        let position = nested_position();
        let graph = FlowGraph::new("review", &position.invocation.steps);
        let expected = &graph.steps[1].paths[0].steps[1].key;
        assert_eq!(*expected, 3);
        let captured = SessionFlowStep::of(&position).unwrap();
        // Immutable legacy input retains its runtime path; the public wire uses its captured ID.
        assert_eq!(captured.node.as_deref(), Some("1/fix/1"));
        assert_eq!(
            position.invocation.node_id(&position.cursor).unwrap(),
            *expected
        );
        assert_eq!(captured.iterations, Some(vec![vec![5], vec![2]]));

        // Old captures keep their known Flow provenance without fabricating a
        // structural node from the former leaf index.
        let mut old = serde_json::to_value(&captured).unwrap();
        old.as_object_mut().unwrap().remove("node");
        old.as_object_mut().unwrap().remove("iterations");
        old["iteration"] = serde_json::json!(3);
        old["step_index"] = serde_json::json!(1);
        let old: SessionFlowStep = serde_json::from_value(old).unwrap();
        assert_eq!(old.node, None);
        assert_eq!(old.iterations, None);
        assert_eq!(old.invocation_id, captured.invocation_id);
    }

    #[test]
    fn stored_session_membership_resolves_nested_and_post_xor_graph_nodes() {
        let _lock = crate::journal::test_env_lock();
        let home = SessionHome::new();
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
                        store.publish_attempt(
                            flow.id(), flow.version,
                flow.current_attempt.as_ref().unwrap().captured,
                            None, "codex", None,
                        ).await.unwrap();
                        let actor = store.sqlite.test_flow_turn(
                            &flow.current_attempt.as_ref().unwrap().run_id,
                        );
                        store.sqlite.test_output(
                            &actor,
                            &serde_json::json!({"decision":"advance","summary":"Membership fixture completed"}),
                        ).unwrap();
                        store.sqlite.test_finish_flow_turn(&actor, "completed");
                        flow = store
                            .checkpoint_flow(flow.id(), flow.version, &cursor, None, None)
                            .await
                            .unwrap();
                    }
                    flow = store
                        .reserve_attempt(flow.id(), flow.version, None, None)
                        .await
                        .unwrap();
                    let run_id = &flow.current_attempt.as_ref().unwrap().run_id;
                    let session = store.session_for_artifact(run_id).await.unwrap().unwrap();
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
                        assert_eq!(node, Some(expected[previous].key));
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
    fn human_sessions_open_through_the_public_session_command() {
        let _lock = crate::journal::test_env_lock();
        let home = SessionHome::new();
        let argv = human_open_argv(None, None, "session_123").unwrap();

        assert_eq!(
            &argv[argv.len() - 3..],
            ["session", "connect", "session_123"]
        );
        assert!(argv.contains(&format!("LF_HOME={}", home.home.path().display())));
        assert!(!argv.iter().any(|argument| argument == "tmux"));
    }

    #[test]
    fn initial_session_publication_requires_history_and_an_owned_client() {
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
        let manifest = crate::session_record::read_manifest(&dir).unwrap();
        assert!(!session_is_resumable(&dir, &manifest).unwrap());
        crate::session_record::write_provider_session(&dir, "provider-session", None).unwrap();
        assert!(!session_is_resumable(&dir, &manifest).unwrap());
        let mut client = std::process::Command::new("/bin/sleep")
            .arg("60")
            .spawn()
            .unwrap();
        let publication = crate::session_record::write_provider_client(&dir, client.id());
        let resumable = publication
            .map_err(anyhow::Error::from)
            .and_then(|()| session_is_resumable(&dir, &manifest));
        client.kill().unwrap();
        client.wait().unwrap();
        assert!(resumable.unwrap());
    }

    #[test]
    fn membership_wire_ids_are_derived_from_their_captures() {
        use crate::engine::flow::{ConcretePath, ConcreteSkill, ConcreteStep, ConcreteXor, Skill};
        use crate::engine::flow_graph::FlowGraph;
        fn skill(name: &str, human: bool) -> ConcreteStep {
            ConcreteStep::Skill(ConcreteSkill {
                skill: Skill::named(name),

                human,
                id: None,
                repeat: None,
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
            skill("loop-decide", false),
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
                invocation_id,
                step,
                node: Some(node),
                ..
            } = session.flow_membership
            {
                assert_eq!(graphs[&invocation_id].node_at(node).unwrap().label, step);
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
