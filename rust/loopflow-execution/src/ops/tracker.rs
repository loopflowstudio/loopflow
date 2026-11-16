//! The repository's tracker, selected in `.lf/config.yaml` the way an agent
//! harness is. A bridge carries the store's queued effects out (`send`), reads
//! an owner back after a lost reply (`look`), and answers what people changed
//! there (`changes`, `observe_issue`), always in store words. Linear GraphQL
//! lives in `linear/effects.rs`, Asana REST in `asana/effects.rs`. The drain
//! owns pacing; `planning::rules` owns settlement.

pub mod drain;

use std::path::Path;

use crate::asana::AsanaClient;
use crate::config::PlanningConfig;
use crate::linear::LinearClient;
use crate::store::Store;
use loopflow_storage::planning::rules::Observed;
use loopflow_storage::planning::{TrackerItem, TrackerObservation, TrackerProject};
use loopflow_storage::provider::Provider;
use loopflow_storage::sqlite::tracker_effects::{EffectKind, TrackerEffect};
use loopflow_storage::work::edit::{Disposition, WorkValue};

use super::{OpsError, OpsResult};

/// What one send did.
#[derive(Debug, Clone, PartialEq)]
pub enum Sent {
    Acknowledged {
        revision: Option<String>,
        remote_id: Option<String>,
    },
    /// Refused before acting (validation, rate limit): safe to send again.
    Rejected(String),
    /// No reply: the provider may or may not have acted.
    Unknown(String),
    /// The bridge has no form for this effect; it retires with the reason.
    Unsupported(String),
}

impl Sent {
    /// A failed request: a rate limit refused it; anything else may have landed.
    pub(crate) fn failed(error: impl Into<BridgeError>) -> Self {
        match error.into() {
            error @ BridgeError::RateLimited { .. } => Self::Rejected(error.to_string()),
            error => Self::Unknown(error.to_string()),
        }
    }
}

/// A bridge request that failed, in the tracker's terms. Saved delivery errors
/// show this text, so it stays the same across retries.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BridgeError {
    /// Refused before acting; the reset time is logged where it is first seen.
    #[error("{tracker} rate limit reached; saved changes retry after it resets")]
    RateLimited {
        tracker: &'static str,
        reset_at_ms: i64,
    },
    #[error("{0}")]
    Message(String),
}

pub type BridgeResult<T> = Result<T, BridgeError>;

impl From<crate::linear::LinearError> for BridgeError {
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
fn transport(repo: &Path) -> Option<PlanningConfig> {
    crate::config::load_planning_transport(repo).ok()
}

/// The configured bridge; Git planning has none.
pub fn configured(repo: &Path) -> Option<Provider> {
    match transport(repo)? {
        PlanningConfig::Linear { .. } => Some(Provider::Linear),
        PlanningConfig::Asana { .. } => Some(Provider::Asana),
        _ => None,
    }
}

pub fn connected(repo: &str) -> bool {
    configured(Path::new(repo)).is_some()
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
        Some(PlanningConfig::Asana { team }) => Ok(crate::asana::team_gid(&team)),
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

/// The configured bridge for one repository.
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
            Some(PlanningConfig::Asana { team }) => asana(repo, &team).await,
            _ => Ok(Self::Linear(super::linear::linear_client(repo).await?)),
        }
    }

    /// Exact-item access for a Task that already names its planning identity.
    pub(crate) async fn for_items(repo: &Path) -> OpsResult<Self> {
        match transport(repo) {
            Some(PlanningConfig::Asana { team }) => asana(repo, &team).await,
            _ => Ok(Self::Linear(
                super::linear::issue_client().await?.for_repository(repo),
            )),
        }
    }

    pub fn provider(&self) -> Provider {
        match self {
            Self::Linear(_) => Provider::Linear,
            Self::Asana(_) => Provider::Asana,
            #[cfg(test)]
            Self::Fake(fake) => fake.provider,
        }
    }

    /// A send carrying our own id cannot duplicate, so a lost reply repeats
    /// it: Linear comments take the id we give them, and every link reads
    /// what the provider holds before it adds anything.
    pub fn idempotent(&self, kind: EffectKind) -> bool {
        match self {
            Self::Linear(_) => matches!(kind, EffectKind::Comment | EffectKind::Link),
            Self::Asana(_) => kind == EffectKind::Link,
            #[cfg(test)]
            Self::Fake(fake) => fake.idempotent.contains(&kind),
        }
    }

    pub async fn send(&self, store: &Store, repo: &Path, effect: &TrackerEffect) -> Sent {
        match self {
            Self::Linear(client) => client.send_effect(store, repo, effect).await,
            Self::Asana(client) => client.send_effect(store, repo, effect).await,
            #[cfg(test)]
            Self::Fake(fake) => fake.send(effect),
        }
    }

    /// Read back the owner an effect addresses, accepting what the provider
    /// holds into the store. `Ok(None)`: the provider has no such object.
    pub async fn look(
        &self,
        store: &Store,
        repo: &Path,
        effect: &TrackerEffect,
    ) -> Result<Option<Observed>, String> {
        match self {
            Self::Linear(client) => client.look_effect(store, repo, effect).await,
            Self::Asana(client) => client.look_effect(store, repo, effect).await,
            #[cfg(test)]
            Self::Fake(fake) => fake.look(effect),
        }
    }

    /// Asana comments carry no caller identity, so a Loopflow comment is its
    /// visible text as the provider reads it back. Linear comments keep their
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
    ) -> BridgeResult<Option<(TrackerItem, Option<TrackerProject>)>> {
        match self {
            Self::Linear(client) => Ok(client.issue_ownership(id).await?),
            Self::Asana(client) => Ok(client.task(id).await?.map(|task| (task, None))),
            #[cfg(test)]
            Self::Fake(_) => Ok(None),
        }
    }

    /// Whether the provider positively reports the item deleted: Linear's trash,
    /// Asana's missing task. A lookup that merely finds nothing is not evidence.
    pub async fn removed(&self, id: &str) -> BridgeResult<bool> {
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
    /// people created there and edits to known ones, with their observations.
    pub async fn changes(
        &self,
        scope: &str,
        since: Option<&str>,
        through: &str,
        live: &[String],
        finished: &[String],
    ) -> BridgeResult<Vec<(TrackerItem, Option<TrackerProject>, TrackerObservation)>> {
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
            .map(|(issue, observation)| (issue, None, observation))
            .collect())
    }

    pub(crate) async fn observe_issue(&self, id: &str) -> BridgeResult<TrackerObservation> {
        match self {
            Self::Linear(client) => Ok(client.observe_issue(id).await?),
            Self::Asana(client) => client.observe(id).await,
            #[cfg(test)]
            Self::Fake(_) => Err(BridgeError::Message("fake tracker".into())),
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
/// providers without comment IDs would otherwise show.
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

// ---- Shared by both bridges: owners and observations in store words ----

pub(crate) fn message(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

/// The value a `field` or `state` effect sets.
pub(crate) fn value(effect: &TrackerEffect) -> OpsResult<WorkValue> {
    serde_json::from_value(effect.value.clone()).map_err(message)
}

/// The values a `create` effect starts its tracker object with.
pub(crate) fn values(effect: &TrackerEffect) -> OpsResult<Vec<WorkValue>> {
    serde_json::from_value(effect.value.clone()).map_err(message)
}

/// The uuid a creation carries as the provider id: the owner's own.
pub(crate) fn creation_id(local: &str) -> OpsResult<String> {
    let raw = local.split_once('_').map_or(local, |(_, id)| id);
    Ok(uuid::Uuid::parse_str(raw).map_err(message)?.to_string())
}

/// The provider id of a bound Task.
pub(crate) async fn remote_task(
    store: &Store,
    task: &loopflow_model::durable::TaskId,
) -> OpsResult<String> {
    let task = store
        .get_task(task)
        .await
        .map_err(message)?
        .ok_or_else(|| message("Task is missing"))?;
    Ok(task.linear_id()?.as_str().to_owned())
}

/// The provider id of a bound Project.
pub(crate) async fn remote_project(
    store: &Store,
    project: &loopflow_model::durable::ProjectId,
) -> OpsResult<String> {
    let project = store
        .get_project(project)
        .await
        .map_err(message)?
        .ok_or_else(|| message("Project is missing"))?;
    Ok(project.linear_id()?.as_str().to_owned())
}

/// What the provider holds for a Task, as the values a Task effect compares.
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
    Observed {
        remote_id: Some(item.id.clone()),
        revision: item.revision.clone(),
        values: vec![
            WorkValue::TaskTitle(Some(item.name.clone())),
            WorkValue::TaskDescription(Some(item.description.clone())),
            WorkValue::TaskAssignee(item.assignee.clone()),
            WorkValue::TaskProject(project),
            WorkValue::Disposition(Disposition {
                planning_state: item.state.clone(),
                planning_completed: i64::from(item.completed),
                planning_completed_at: item.completed_at.clone(),
            }),
        ],
        ..Observed::default()
    }
}

/// What the provider holds for a Project.
pub(crate) fn observed_project(project: &TrackerProject) -> Observed {
    Observed {
        remote_id: Some(project.id.clone()),
        revision: project.revision.clone(),
        values: vec![
            WorkValue::ProjectName(Some(project.name.clone())),
            WorkValue::ProjectSummary(project.summary.clone()),
            WorkValue::ProjectDescription(project.description.clone()),
            WorkValue::ProjectStatus(project.status),
            WorkValue::Krs(project.krs.clone()),
            WorkValue::MetricTargets(project.metric_targets.clone()),
        ],
        initiatives: project.initiative_ids.clone(),
        ..Observed::default()
    }
}

/// Accept a provider's copy of one item into Work. Exact detail can establish
/// the saved Project's association without claiming a complete inventory
/// refresh; creation, look and acquisition share this path.
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
    let mut confirmed = None;
    if let Some(project) = project {
        if let Some(local) = store
            .get_project_by_project(&project.id)
            .await
            .map_err(message)?
            .filter(|p| {
                p.linear_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == project.id)
            })
        {
            let wave = store
                .get_wave(&local.wave_id)
                .await
                .map_err(message)?
                .ok_or_else(|| message("Project Wave is missing"))?;
            if let Some(initiative) = super::linear::read_initiative(repo, wave.slug()) {
                confirmed = Some((local.wave_id, initiative));
            }
        }
    }
    if confirmed.is_none() {
        if let Some(project) = project {
            for wave in store
                .list_waves(Some(&repo.to_string_lossy()))
                .await
                .map_err(message)?
            {
                if let Some(initiative) = super::linear::read_initiative(repo, wave.slug()) {
                    if project.initiative_ids.contains(&initiative) {
                        if confirmed.is_some() {
                            return Err(message("Project matches multiple Wave Initiatives"));
                        }
                        confirmed = Some((wave.id().clone(), initiative));
                    }
                }
            }
        }
    }
    loopflow_storage::sqlite::linear_acceptance::accept_linear_issue_in(
        &store.sqlite,
        &repo.to_string_lossy(),
        issue,
        project,
        observed_at,
        confirmed
            .as_ref()
            .map(|(wave, initiative)| (wave, initiative.as_str())),
        configured(repo).unwrap_or(Provider::Linear),
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
    use loopflow_storage::provider::Provider;
    use loopflow_storage::sqlite::tracker_effects::{EffectKind, TrackerEffect};

    use super::Sent;

    /// A provider that acknowledges every send and answers looks from `held`.
    #[derive(Debug)]
    pub struct Fake {
        pub provider: Provider,
        pub idempotent: Vec<EffectKind>,
        pub held: Mutex<Option<Observed>>,
        pub requests: AtomicUsize,
    }

    impl Fake {
        pub fn new(provider: Provider) -> Self {
            Self {
                provider,
                idempotent: Vec::new(),
                held: Mutex::new(None),
                requests: AtomicUsize::new(0),
            }
        }

        pub fn requests(&self) -> usize {
            self.requests.load(Ordering::SeqCst)
        }

        pub(super) fn send(&self, effect: &TrackerEffect) -> Sent {
            self.requests.fetch_add(1, Ordering::SeqCst);
            Sent::Acknowledged {
                revision: Some("2026-11-16T00:00:00Z".into()),
                remote_id: Some(format!("remote-{}", effect.id)),
            }
        }

        pub(super) fn look(&self, _effect: &TrackerEffect) -> Result<Option<Observed>, String> {
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
