//! The repository's tracker, selected in `.lf/config.yaml` the way an agent
//! harness is. A tracker carries the store's queued actions out (`send`), reads
//! an owner back after a lost reply (`look`), and answers what people changed
//! there (`changes`, `fetch_issue`), always in store words. Linear GraphQL
//! lives in `linear/actions.rs`, Asana REST in `asana/actions.rs`. The drain
//! owns pacing; `planning::rules` owns resolution.

pub mod drain;

use std::path::Path;

use crate::asana::AsanaClient;
use crate::config::TrackerConfig;
use crate::linear::LinearClient;
use crate::store::Store;
use loopflow_storage::planning::rules::Observed;
use loopflow_storage::planning::{TrackerEvent, TrackerItem, TrackerProject};
use loopflow_storage::planning_sync::TrackerKind;
use loopflow_storage::sqlite::tracker_actions::{ActionKind, TrackerAction};
use loopflow_storage::work::edit::{Disposition, WorkValue};

use super::{OpsError, OpsResult};

/// What one send did.
#[derive(Debug, Clone, PartialEq)]
pub enum Sent {
    Acknowledged {
        revision: Option<String>,
        remote_id: Option<String>,
    },
    /// The tracker refused before acting (validation, rate limit): safe to
    /// send again.
    Rejected(String),
    /// No reply: the tracker may or may not have acted.
    Unknown(String),
    /// The tracker has no form for this action; it retires with the reason.
    Unsupported(String),
}

impl Sent {
    /// A failed request: a rate limit refused it; anything else may have landed.
    pub(crate) fn failed(error: impl Into<TrackerError>) -> Self {
        match error.into() {
            error @ TrackerError::RateLimited { .. } => Self::Rejected(error.to_string()),
            error => Self::Unknown(error.to_string()),
        }
    }
}

/// A tracker request that failed, in the tracker's terms. Saved delivery errors
/// show this text, so it stays the same across retries.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TrackerError {
    /// Refused before acting; the reset time is logged where it is first seen.
    #[error("{tracker} rate limit reached; saved changes retry after it resets")]
    RateLimited {
        tracker: &'static str,
        reset_at_ms: i64,
    },
    #[error("{0}")]
    Message(String),
}

pub type TrackerResult<T> = Result<T, TrackerError>;

impl From<crate::linear::LinearError> for TrackerError {
    fn from(error: crate::linear::LinearError) -> Self {
        match error {
            crate::linear::LinearError::RateLimited { reset_at_ms } => Self::RateLimited {
                tracker: "Linear",
                reset_at_ms,
            },
            crate::linear::LinearError::Message(text) => Self::Message(text),
        }
    }
}

/// The repository's planning selection. Unreadable configuration selects none:
/// routing is configuration, never availability.
fn transport(repo: &Path) -> Option<TrackerConfig> {
    crate::config::load_planning_transport(repo).ok().flatten()
}

/// The tracker that sends through the action queue: Linear or Asana.
pub fn queued_tracker(repo: &Path) -> Option<TrackerKind> {
    match transport(repo)? {
        TrackerConfig::Linear { .. } => Some(TrackerKind::Linear),
        TrackerConfig::Asana { .. } => Some(TrackerKind::Asana),
        _ => None,
    }
}

pub fn connected(repo: &str) -> bool {
    queued_tracker(Path::new(repo)).is_some()
}

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    pub(crate) static SCOPE_READS: std::cell::RefCell<std::collections::BTreeMap<std::path::PathBuf, usize>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

/// The team every repository Task belongs to: a Linear Team or an Asana team.
pub fn repository_scope(repo: &Path) -> OpsResult<String> {
    #[cfg(any(test, feature = "test-support"))]
    SCOPE_READS.with_borrow_mut(|reads| *reads.entry(repo.to_path_buf()).or_default() += 1);
    match transport(repo) {
        Some(TrackerConfig::Asana { team }) => Ok(crate::asana::team_gid(&team)),
        _ => super::linear::repository_team_id(repo),
    }
}

/// The repository's name, which names its Asana team project: the origin's
/// repository name, else the main checkout's directory.
fn repository_name(repo: &Path) -> String {
    if let Ok(id) = crate::repository::RepoId::discover(repo) {
        return id.name().to_string();
    }
    let main = crate::git::worktrees::main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    main.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "loopflow".into())
}

/// The configured Linear or Asana tracker for one repository.
#[derive(Debug)]
#[non_exhaustive]
pub enum Tracker {
    Linear(LinearClient),
    Asana(AsanaClient),
    #[cfg(test)]
    Fake(std::sync::Arc<fake::Fake>),
}

impl Tracker {
    /// Repository-scoped access; Linear validates the repository's Team claim.
    pub(crate) async fn for_repository(repo: &Path) -> OpsResult<Self> {
        match transport(repo) {
            Some(TrackerConfig::Asana { team }) => asana(repo, &team).await,
            _ => Ok(Self::Linear(super::linear::linear_client(repo).await?)),
        }
    }

    /// Exact-item access for a Task that already names its planning identity.
    pub(crate) async fn for_items(repo: &Path) -> OpsResult<Self> {
        match transport(repo) {
            Some(TrackerConfig::Asana { team }) => asana(repo, &team).await,
            _ => Ok(Self::Linear(
                super::linear::issue_client().await?.for_repository(repo),
            )),
        }
    }

    /// A send carrying our own id cannot duplicate, so a lost reply repeats
    /// it: Linear comments take the id we give them, a relation reads what
    /// the tracker holds before adding one, and a PR link updates the
    /// attachment it recorded or finds the one Linear already has.
    pub fn idempotent(&self, kind: ActionKind) -> bool {
        match self {
            Self::Linear(_) => matches!(kind, ActionKind::Comment | ActionKind::Link),
            Self::Asana(_) => kind == ActionKind::Link,
            #[cfg(test)]
            Self::Fake(fake) => fake.idempotent.contains(&kind),
        }
    }

    pub async fn send(&self, store: &Store, repo: &Path, action: &TrackerAction) -> Sent {
        match self {
            Self::Linear(client) => client.send_action(store, repo, action).await,
            Self::Asana(client) => client.send_action(store, repo, action).await,
            #[cfg(test)]
            Self::Fake(fake) => fake.send(action),
        }
    }

    /// Read back the owner an action addresses, accepting what the tracker
    /// holds into the store. `Ok(None)`: the tracker has no such object.
    pub async fn look(
        &self,
        store: &Store,
        repo: &Path,
        action: &TrackerAction,
    ) -> Result<Option<Observed>, String> {
        match self {
            Self::Linear(client) => client.look_action(store, repo, action).await,
            Self::Asana(client) => client.look_action(store, repo, action).await,
            #[cfg(test)]
            Self::Fake(fake) => fake.look(action),
        }
    }

    /// Asana comments carry no caller identity, so a Loopflow comment is its
    /// visible text as the tracker reads it back. Linear comments keep their
    /// Loopflow ids.
    pub(crate) fn claimed_comment(&self, body: &str) -> Option<String> {
        match self {
            Self::Asana(_) => Some(crate::asana::comment_markdown(&visible_text(body))),
            _ => None,
        }
    }

    pub(crate) async fn issue_ownership(
        &self,
        id: &str,
    ) -> TrackerResult<Option<(TrackerItem, Option<TrackerProject>)>> {
        match self {
            Self::Linear(client) => Ok(client.issue_ownership(id).await?),
            Self::Asana(client) => Ok(client.task(id).await?.map(|task| (task, None))),
            #[cfg(test)]
            Self::Fake(_) => Ok(None),
        }
    }

    /// Whether the tracker positively reports the item deleted: Linear's trash,
    /// Asana's missing task. A lookup that merely finds nothing is not evidence.
    pub async fn removed(&self, id: &str) -> TrackerResult<bool> {
        match self {
            Self::Linear(client) => Ok(client
                .issue_trash(id)
                .await?
                .is_some_and(|(trashed, _)| trashed)),
            Self::Asana(client) => Ok(client.task(id).await?.is_none()),
            #[cfg(test)]
            Self::Fake(_) => Ok(false),
        }
    }

    /// Items in the repository's planning scope changed since `since`: new items
    /// people created there and edits to known ones, with their events.
    pub async fn changes(
        &self,
        scope: &str,
        since: Option<&str>,
        through: &str,
        live: &[String],
        finished: &[String],
    ) -> TrackerResult<Vec<(TrackerItem, Option<TrackerProject>, TrackerEvent)>> {
        let changes = match self {
            Self::Linear(client) => {
                return Ok(client
                    .team_changes(scope, since, through, live, finished)
                    .await?)
            }
            Self::Asana(client) => client.changes(since, live, finished).await?,
            #[cfg(test)]
            Self::Fake(_) => Vec::new(),
        };
        Ok(changes
            .into_iter()
            .map(|(issue, event)| (issue, None, event))
            .collect())
    }

    pub(crate) async fn fetch_issue(&self, id: &str) -> TrackerResult<TrackerEvent> {
        match self {
            Self::Linear(client) => Ok(client.fetch_issue(id).await?),
            Self::Asana(client) => client.fetch_issue(id).await,
            #[cfg(test)]
            Self::Fake(_) => Err(TrackerError::Message("fake tracker".into())),
        }
    }
}

async fn asana(repo: &Path, team: &str) -> OpsResult<Tracker> {
    let token =
        super::linear::resolve_planning_token(crate::provider_auth::Provider::Asana).await?;
    let repository = repository_name(repo);
    #[cfg(any(test, feature = "test-support"))]
    if let Ok(url) =
        super::linear::LINEAR_TEST_CONTEXT.try_with(|context| context.graphql_url.clone())
    {
        return Ok(Tracker::Asana(AsanaClient::with_base_url(
            token, team, repository, url,
        )));
    }
    Ok(Tracker::Asana(AsanaClient::new(token, team, repository)))
}

/// A comment without Loopflow's hidden `<!-- loopflow-… -->` markers, which
/// trackers without comment IDs would otherwise show.
pub(crate) fn visible_text(body: &str) -> String {
    let mut text = String::new();
    let mut rest = body;
    while let Some(start) = rest.find("<!-- loopflow-") {
        text.push_str(&rest[..start]);
        rest = match rest[start..].find("-->") {
            Some(end) => &rest[start + end + 3..],
            None => "",
        };
    }
    text.push_str(rest);
    text.trim().to_string()
}

// ---- Shared by both trackers: owners and events in store words ----

pub(crate) fn message(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

/// The value a `field` or `state` action sets.
pub(crate) fn value(action: &TrackerAction) -> OpsResult<WorkValue> {
    serde_json::from_value(action.value.clone()).map_err(message)
}

/// The values a `create` action starts its tracker object with.
pub(crate) fn values(action: &TrackerAction) -> OpsResult<Vec<WorkValue>> {
    loopflow_storage::work::edit::known_values(action.value.clone()).map_err(message)
}

/// The state a `state` action moves its Task to.
pub(crate) fn target_state(action: &TrackerAction) -> OpsResult<String> {
    let WorkValue::Disposition(disposition) = value(action)? else {
        return Err(message("state action holds no Disposition"));
    };
    disposition
        .planning_state
        .ok_or_else(|| message("state action names no state"))
}

/// The state a Task's tracker item is created in: the Task's own, else open.
pub(crate) async fn opening_state(
    store: &Store,
    task: &loopflow_model::durable::TaskId,
) -> OpsResult<String> {
    Ok(store
        .get_task(task)
        .await
        .map_err(message)?
        .and_then(|task| task.state)
        .unwrap_or_else(|| "unstarted".into()))
}

/// The uuid a creation carries as the tracker id: the owner's own.
pub(crate) fn creation_id(local: &str) -> OpsResult<String> {
    let raw = local.split_once('_').map_or(local, |(_, id)| id);
    Ok(uuid::Uuid::parse_str(raw).map_err(message)?.to_string())
}

/// The tracker id of a bound Task.
pub(crate) async fn remote_task(
    store: &Store,
    task: &loopflow_model::durable::TaskId,
) -> OpsResult<String> {
    let task = store
        .get_task(task)
        .await
        .map_err(message)?
        .ok_or_else(|| message("Task is missing"))?;
    Ok(task.tracker_id()?.as_str().to_owned())
}

/// The tracker id of the Task a creation or a parent edit names as parent.
/// `None` when it names none, or one the tracker does not hold: another
/// repository's, or one no tracker was ever sent.
pub(crate) async fn remote_parent(
    store: &Store,
    parent: Option<&str>,
) -> OpsResult<Option<String>> {
    let Some(parent) = parent else {
        return Ok(None);
    };
    let parent = loopflow_model::durable::TaskId::from_raw(parent);
    Ok(store
        .get_task(&parent)
        .await
        .map_err(message)?
        .and_then(|task| task.tracker_id)
        .map(|id| id.as_str().to_owned()))
}

/// The tracker id of a bound Project.
pub(crate) async fn remote_project(
    store: &Store,
    project: &loopflow_model::durable::ProjectId,
) -> OpsResult<String> {
    let project = store
        .get_project(project)
        .await
        .map_err(message)?
        .ok_or_else(|| message("Project is missing"))?;
    Ok(project.tracker_id()?.as_str().to_owned())
}

/// What the tracker holds for a Task, as the values a Task action compares.
pub(crate) async fn observed_task(store: &Store, item: &TrackerItem) -> Observed {
    let project = match &item.project_id {
        Some(external) => store
            .get_project_by_project(external)
            .await
            .ok()
            .flatten()
            .map(|project| project.id.to_string()),
        None => None,
    };
    let parent = match &item.parent_id {
        Some(external) => store
            .get_task_by_issue(external)
            .await
            .ok()
            .flatten()
            .map(|task| task.id.to_string()),
        None => None,
    };
    let mut values = vec![
        WorkValue::TaskTitle(Some(item.name.clone())),
        WorkValue::TaskDescription(Some(item.description.clone())),
        WorkValue::TaskAssignee(item.assignee.clone()),
        WorkValue::TaskProject(project),
        WorkValue::Disposition(Disposition {
            planning_state: item.state.clone(),
            planning_completed: i64::from(item.completed),
            planning_completed_at: item.completed_at.clone(),
        }),
    ];
    if item.parent_observed {
        values.push(WorkValue::TaskParent(parent));
    }
    Observed {
        remote_id: Some(item.id.clone()),
        revision: item.revision.clone(),
        values,
        ..Observed::default()
    }
}

/// What the tracker holds for a Project.
pub(crate) fn observed_project(project: &TrackerProject) -> Observed {
    Observed {
        remote_id: Some(project.id.clone()),
        revision: project.revision.clone(),
        values: vec![
            WorkValue::ProjectName(Some(project.name.clone())),
            WorkValue::ProjectSummary(project.summary.clone()),
            WorkValue::ProjectDescription(project.description.clone()),
            WorkValue::ProjectStatus(project.status),
        ],
        ..Observed::default()
    }
}

/// Accept a tracker's copy of one item into Work. Exact detail can establish
/// the saved Project's association without claiming a complete inventory
/// refresh; creation, readback and refresh share this path.
pub(crate) async fn ingest_task(
    store: &Store,
    repo: &Path,
    issue: &TrackerItem,
    project: Option<&TrackerProject>,
    observed_at: i64,
) -> OpsResult<()> {
    let team = repository_scope(repo)?;
    if issue.team_id.as_deref() != Some(team.as_str()) {
        return Err(message(format!(
            "Task {} must belong to repository Team {team}",
            issue.identifier
        )));
    }
    loopflow_storage::sqlite::linear_events::accept_linear_issue_in(
        &store.sqlite,
        &repo.to_string_lossy(),
        issue,
        project,
        observed_at,
        queued_tracker(repo).unwrap_or(TrackerKind::Linear),
    )
    .map_err(message)
}

pub(crate) fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

#[cfg(test)]
pub(crate) mod fake {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    use loopflow_storage::planning::rules::Observed;
    use loopflow_storage::sqlite::tracker_actions::{ActionKind, TrackerAction};

    use super::Sent;

    /// A tracker that confirms every send and answers looks from `held`.
    #[derive(Debug)]
    pub struct Fake {
        pub idempotent: Vec<ActionKind>,
        pub held: Mutex<Option<Observed>>,
        pub requests: AtomicUsize,
    }

    impl Fake {
        pub fn new() -> Self {
            Self {
                idempotent: Vec::new(),
                held: Mutex::new(None),
                requests: AtomicUsize::new(0),
            }
        }

        pub fn requests(&self) -> usize {
            self.requests.load(Ordering::SeqCst)
        }

        pub(super) fn send(&self, action: &TrackerAction) -> Sent {
            self.requests.fetch_add(1, Ordering::SeqCst);
            Sent::Acknowledged {
                revision: Some("2026-11-16T00:00:00Z".into()),
                remote_id: Some(format!("remote-{}", action.id)),
            }
        }

        pub(super) fn look(&self, _action: &TrackerAction) -> Result<Option<Observed>, String> {
            self.requests.fetch_add(1, Ordering::SeqCst);
            Ok(self.held.lock().unwrap().clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::visible_text;

    #[test]
    fn visible_text_hides_loopflow_markers() {
        assert_eq!(
            visible_text(
                "Ship it\n\n<!-- loopflow-requester:\"Jack\" -->\n\n<!-- loopflow-steer:1 -->"
            ),
            "Ship it"
        );
        assert_eq!(visible_text("<!-- other --> kept"), "<!-- other --> kept");
    }

    #[test]
    fn local_identifier_matches_the_task_identifier() {
        let task = loopflow_model::durable::TaskId::new();
        let uuid = uuid::Uuid::parse_str(task.as_str().trim_start_matches("task_")).unwrap();
        assert_eq!(
            crate::linear::local_identifier(&uuid.to_string()),
            format!("lf-{}", task.as_str().trim_start_matches("task_"))
        );
    }
}
