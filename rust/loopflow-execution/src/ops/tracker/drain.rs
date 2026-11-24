//! One bounded pass over the outbound queue. Owners go in order of their
//! oldest unconfirmed action; within an owner actions go strictly in `seq`
//! order, so an owner whose head waits holds its later actions and no one
//! else's. A pass stops after its request or time budget, whichever is first;
//! the request budget counts only requests that leave for the tracker.

use std::collections::HashSet;
use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use loopflow_storage::planning::rules::{self, Decision, LostReply, Observed};
use loopflow_storage::planning_sync::TrackerKind;
use loopflow_storage::sqlite::tracker_actions::{
    ActionKind, ActionOwner, ActionPhase, Resolution, TaskLink, TrackerAction,
};
use loopflow_storage::work::edit::WorkValue;

use super::{message, value, values, Sent, Tracker, TrackerError};
use crate::ops::{OpsError, OpsResult};
use crate::store::Store;

/// The shortest wait between looks for one lost reply.
const LOOK_WINDOW_SECS: i64 = 600;

/// Each request keeps its own deadline: a shared one once let a slow first
/// creation starve every later Task, so the pass bound counts requests.
const REQUEST_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy)]
pub struct Bound {
    pub requests: usize,
    pub time: Duration,
    pub concurrency: usize,
    /// `lf task sync` looks now instead of waiting for a lost reply's window.
    pub windows: bool,
}

impl Bound {
    /// One pass after a save or at a planning boundary.
    pub const PASS: Self = Self {
        requests: 20,
        time: Duration::from_secs(5),
        concurrency: 4,
        windows: true,
    };
    /// `lf task sync <id>`: one owner, now.
    pub const NOW: Self = Self {
        requests: 20,
        time: Duration::from_secs(30),
        concurrency: 1,
        windows: false,
    };
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pass {
    pub requests: usize,
    pub resolved: usize,
}

/// Which owners a pass drains.
#[derive(Debug, Clone, Copy)]
pub enum Scope<'a> {
    All,
    /// The owner just saved goes ahead of older work.
    First(&'a ActionOwner),
    /// `lf task sync <id>`: this owner only.
    Only(&'a ActionOwner),
}

/// One bounded pass for `repo` when it has a tracker. With nothing unconfirmed
/// it asks the tracker nothing, not even to build the tracker.
pub(crate) async fn drain_repository(
    store: &Store,
    repo: &str,
    scope: Scope<'_>,
    bound: Bound,
) -> OpsResult<Pass> {
    let Some(tracker) = super::queued_tracker(Path::new(repo)) else {
        return Ok(Pass::default());
    };
    let actions = queued(store, repo, tracker)?;
    if actions.is_empty() {
        return Ok(Pass::default());
    }
    // Only a creation puts an item in the repository's Team, which Linear
    // validates once per process; edits address items that already name it.
    let tracker = if actions
        .iter()
        .any(|action| action.kind == ActionKind::Create)
    {
        Tracker::for_repository(Path::new(repo)).await
    } else {
        Tracker::for_items(Path::new(repo)).await
    };
    let tracker = match tracker {
        Ok(tracker) => tracker,
        // A pause belongs to the Home, not to each action: their own errors stay.
        Err(OpsError::Tracker(TrackerError::RateLimited { .. })) => return Ok(Pass::default()),
        // Offline or signed out: the queue keeps every action and says why.
        Err(error) => {
            for action in &actions {
                store
                    .sqlite
                    .tracker_action_error(&action.id, &error.to_string())
                    .map_err(message)?;
            }
            return Ok(Pass::default());
        }
    };
    create_named_projects(store, Path::new(repo), &tracker, &actions).await;
    drain(store, repo, &tracker, actions, scope, bound).await
}

/// A live Wave's Project the tracker does not hold yet (its Wave came from
/// another checkout's files, or from an upgrade) is created here, before the
/// Tasks that name it are sent. A failure leaves those Tasks to say why.
async fn create_named_projects(
    store: &Store,
    repo: &Path,
    tracker: &Tracker,
    actions: &[TrackerAction],
) {
    let mut named: Vec<loopflow_model::durable::ProjectId> = Vec::new();
    for parent in actions.iter().flat_map(parents_named) {
        if let ActionOwner::Project(project) = parent {
            if !named.contains(&project) {
                named.push(project);
            }
        }
    }
    for id in named {
        let Ok(Some(project)) = store.sqlite.project(&id) else {
            continue;
        };
        if project.tracker_id.is_some()
            || !matches!(store.sqlite.get_wave(&project.wave_id), Ok(Some(wave)) if !wave.is_retired())
        {
            continue;
        }
        if let Err(error) = tracker.create_project(store, repo, &project).await {
            tracing::warn!(project = %id, %error, "Wave Project not created in the tracker");
        }
    }
}

/// The repository's unconfirmed actions for `tracker`, after queueing a
/// creation for each owner made before the tracker was connected. An action
/// queued for another tracker retires here, before any tracker is built, so a
/// switched repository that is offline or signed out does not keep it.
fn queued(store: &Store, repo: &str, tracker: TrackerKind) -> OpsResult<Vec<TrackerAction>> {
    store
        .sqlite
        .enqueue_tracker_creations(repo, tracker)
        .map_err(message)?;
    let (actions, elsewhere): (Vec<_>, Vec<_>) = store
        .sqlite
        .unconfirmed_tracker_actions(repo)
        .map_err(message)?
        .into_iter()
        .partition(|action| action.tracker == tracker);
    for action in elsewhere {
        let reason = format!(
            "queued for {}, but this repository uses {}",
            action.tracker.display_name(),
            tracker.display_name()
        );
        store
            .sqlite
            .resolve_tracker_action(&action.id, &Resolution::Retire { reason })
            .map_err(message)?;
    }
    Ok(actions)
}

/// Drain `actions` to `tracker` within `bound`.
async fn drain(
    store: &Store,
    repo: &str,
    tracker: &Tracker,
    actions: Vec<TrackerAction>,
    scope: Scope<'_>,
    bound: Bound,
) -> OpsResult<Pass> {
    let creating: HashSet<ActionOwner> = actions
        .iter()
        .filter(|action| action.kind == ActionKind::Create)
        .map(|action| action.owner.clone())
        .collect();
    let pass = Drain {
        store,
        repo: Path::new(repo),
        tracker,
        creating,
        limit: bound.requests,
        until: Instant::now() + bound.time,
        window: if bound.windows {
            LOOK_WINDOW_SECS.max(crate::linear::delta_fresh_secs())
        } else {
            0
        },
        used: AtomicUsize::new(0),
        resolved: AtomicUsize::new(0),
    };
    pass.round(grouped(actions, scope), bound.concurrency).await;
    // Owners run concurrently, so a Task can wait on a parent created in the
    // same round; those get one more turn once it has resolved.
    if !pass.creating.is_empty() {
        let mut owners = grouped(
            store
                .sqlite
                .unconfirmed_tracker_actions(repo)
                .map_err(message)?,
            scope,
        );
        owners.retain(|(_, queued)| {
            queued.iter().any(|action| {
                parents_named(action)
                    .iter()
                    .any(|parent| pass.creating.contains(parent))
                    && !pass.parent_uncreated(action)
            })
        });
        pass.round(owners, bound.concurrency).await;
    }
    Ok(Pass {
        requests: pass.used.load(Ordering::SeqCst),
        resolved: pass.resolved.load(Ordering::SeqCst),
    })
}

/// Actions by owner, owners in order of their oldest action, narrowed to `scope`.
fn grouped(
    actions: Vec<TrackerAction>,
    scope: Scope<'_>,
) -> Vec<(ActionOwner, Vec<TrackerAction>)> {
    let mut owners: Vec<(ActionOwner, Vec<TrackerAction>)> = Vec::new();
    for action in actions {
        match owners.iter_mut().find(|(owner, _)| *owner == action.owner) {
            Some((_, queued)) => queued.push(action),
            None => owners.push((action.owner.clone(), vec![action])),
        }
    }
    match scope {
        Scope::All => {}
        Scope::First(first) => {
            if let Some(at) = owners.iter().position(|(owner, _)| owner == first) {
                let owner = owners.remove(at);
                owners.insert(0, owner);
            }
        }
        Scope::Only(only) => owners.retain(|(owner, _)| owner == only),
    }
    owners
}

/// The Project and the parent Task a Task creation or edit names.
fn parents_named(action: &TrackerAction) -> Vec<ActionOwner> {
    let values = match action.kind {
        ActionKind::Create => values(action).unwrap_or_default(),
        ActionKind::Field => value(action).into_iter().collect(),
        _ => Vec::new(),
    };
    values
        .into_iter()
        .filter_map(|value| match value {
            WorkValue::TaskProject(Some(project)) => Some(ActionOwner::Project(
                loopflow_model::durable::ProjectId::from_raw(project),
            )),
            WorkValue::TaskParent(Some(parent)) => Some(ActionOwner::Task(
                loopflow_model::durable::TaskId::from_raw(parent),
            )),
            _ => None,
        })
        .collect()
}

/// The tracker id an owner is bound to.
fn bound(store: &Store, owner: &ActionOwner) -> OpsResult<Option<String>> {
    Ok(match owner {
        ActionOwner::Task(id) => store
            .sqlite
            .task(id)
            .map_err(message)?
            .and_then(|task| task.tracker_id)
            .map(|id| id.as_str().to_owned()),
        ActionOwner::Project(id) => store
            .sqlite
            .project(id)
            .map_err(message)?
            .and_then(|project| project.tracker_id)
            .map(|id| id.as_str().to_owned()),
        ActionOwner::Wave(_) => None,
    })
}

/// Serialize tracker actions only; saves and acquisition never take this lock.
/// The file stays in place so another connection always locks the same inode.
pub(crate) fn lock_delivery(path: &Path) -> std::io::Result<Option<File>> {
    std::fs::create_dir_all(path.parent().expect("delivery lock has a parent directory"))?;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    match fs2::FileExt::try_lock_exclusive(&lock) {
        Ok(()) => Ok(Some(lock)),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(error),
    }
}

/// What one action left its owner's pass to do next.
enum Step {
    Done,
    /// This action waits; later actions on its subject wait with it.
    Wait,
    /// The pass's budget is spent.
    Spent,
}

/// One pass: where it sends, what it may spend, and what it has done.
struct Drain<'a> {
    store: &'a Store,
    repo: &'a Path,
    tracker: &'a Tracker,
    /// Projects and Tasks whose creation is queued.
    creating: HashSet<ActionOwner>,
    limit: usize,
    until: Instant,
    /// Seconds between looks for one lost reply; 0 looks now.
    window: i64,
    used: AtomicUsize,
    resolved: AtomicUsize,
}

impl Drain<'_> {
    async fn round(&self, owners: Vec<(ActionOwner, Vec<TrackerAction>)>, concurrency: usize) {
        let results: Vec<OpsResult<()>> = futures_util::stream::iter(owners)
            .map(|(owner, queued)| self.owner(owner, queued))
            .buffer_unordered(concurrency.max(1))
            .collect()
            .await;
        for error in results.into_iter().filter_map(Result::err) {
            tracing::debug!(%error, "tracker action unconfirmed");
        }
    }

    /// Spend one request of the pass, if any is left.
    fn take(&self) -> bool {
        if Instant::now() >= self.until {
            return false;
        }
        let mut used = self.used.load(Ordering::SeqCst);
        while used < self.limit {
            match self.used.compare_exchange_weak(
                used,
                used + 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return true,
                Err(current) => used = current,
            }
        }
        false
    }

    /// Whether this action names a Project or a parent Task being created
    /// that has no remote id yet.
    fn parent_uncreated(&self, action: &TrackerAction) -> bool {
        parents_named(action)
            .iter()
            .filter(|parent| self.creating.contains(parent))
            .any(|parent| !matches!(bound(self.store, parent), Ok(Some(_))))
    }

    /// Whether this action relates its Task to one with no tracker object yet.
    fn target_unbound(&self, action: &TrackerAction) -> bool {
        let Some(TaskLink::Relation { target, .. }) = action.task_link() else {
            return false;
        };
        !matches!(bound(self.store, &ActionOwner::Task(target)), Ok(Some(_)))
    }

    // Another lf on this store may be sending for the same object; the lock
    // file keeps the two from interleaving requests.
    fn lock(&self, owner: &ActionOwner) -> OpsResult<Option<File>> {
        let path = self
            .store
            .sqlite
            .home_dir()
            .map_err(message)?
            .join("locks/planning-fields")
            .join(format!("{}-{}.lock", owner.kind().as_str(), owner.id()));
        Ok(lock_delivery(&path)?)
    }

    async fn owner(&self, owner: ActionOwner, queued: Vec<TrackerAction>) -> OpsResult<()> {
        let Some(_lock) = self.lock(&owner)? else {
            return Ok(());
        };
        // Everything on an owner waits behind its unconfirmed creation; otherwise
        // only actions on the same subject wait behind each other.
        let mut waiting = HashSet::new();
        for read in queued {
            let key = (read.kind, read.subject.clone());
            if waiting.contains(&key) {
                continue;
            }
            // Another lf may have sent or resolved it between this pass's read
            // and the lock; acting on the old phase would send it twice.
            let Some(action) = self
                .store
                .sqlite
                .tracker_action(&read.id)
                .map_err(message)?
                .filter(|action| matches!(action.phase, ActionPhase::Queued | ActionPhase::Sent))
            else {
                continue;
            };
            match self.action(action).await? {
                Step::Done => {}
                Step::Wait if key.0 == ActionKind::Create => return Ok(()),
                Step::Wait => {
                    waiting.insert(key);
                }
                Step::Spent => return Ok(()),
            }
        }
        Ok(())
    }

    async fn action(&self, action: TrackerAction) -> OpsResult<Step> {
        // An owner bound some other way (adopted, linked by hand) already has its
        // tracker object; sending the creation would make a second one.
        if action.kind == ActionKind::Create {
            if let Some(remote) = bound(self.store, &action.owner)? {
                let resolution = Resolution::Confirmed {
                    revision: None,
                    remote_id: Some(remote),
                };
                self.resolve(&action, &resolution)?;
                return Ok(Step::Done);
            }
        }
        let parent = self.parent_uncreated(&action) || self.target_unbound(&action);
        let looked = match action.phase {
            ActionPhase::Sent => {
                let now = loopflow_storage::rows::now_unix();
                let idempotent = self.tracker.idempotent(action.kind);
                match rules::lost_reply(&action, idempotent, now, self.window) {
                    LostReply::Wait => return Ok(Step::Wait),
                    LostReply::Resend => None,
                    LostReply::Look => Some(self.look(&action).await?),
                }
            }
            _ if parent => return Ok(Step::Wait),
            // Moving Tasks needs the tracker's complete list first.
            _ if action.kind == ActionKind::Order => Some(self.look(&action).await?),
            _ => None,
        };
        let decision = match looked {
            None => Decision::Send,
            Some(Ok(observed)) => rules::decide(&action, observed.as_ref(), parent),
            Some(Err(step)) => return Ok(step),
        };
        match decision {
            Decision::Hold => return Ok(Step::Wait),
            Decision::Resolve(resolution) => self.resolve(&action, &resolution)?,
            Decision::Send => return self.send(&action).await,
        }
        Ok(Step::Done)
    }

    /// Read back the owner `action` addresses. A look at a lost reply starts
    /// its window; a failed look saves its error and the action waits.
    async fn look(&self, action: &TrackerAction) -> OpsResult<Result<Option<Observed>, Step>> {
        if !self.take() {
            return Ok(Err(Step::Spent));
        }
        let observed = tokio::time::timeout(
            REQUEST_DEADLINE,
            self.tracker.look(self.store, self.repo, action),
        )
        .await
        .unwrap_or_else(|_| Err("Look timed out; the action waits for the next pass".into()));
        if action.phase == ActionPhase::Sent {
            self.store
                .sqlite
                .look_tracker_action(&action.id, loopflow_storage::rows::now_unix())
                .map_err(message)?;
        }
        match observed {
            Ok(observed) => Ok(Ok(observed)),
            Err(error) => {
                self.error(action, &error)?;
                Ok(Err(Step::Wait))
            }
        }
    }

    async fn send(&self, action: &TrackerAction) -> OpsResult<Step> {
        if !self.take() {
            return Ok(Step::Spent);
        }
        if !self
            .store
            .sqlite
            .attempt_tracker_action(action)
            .map_err(message)?
        {
            return Ok(Step::Done);
        }
        let sent = tokio::time::timeout(
            REQUEST_DEADLINE,
            self.tracker.send(self.store, self.repo, action),
        )
        .await
        .unwrap_or_else(|_| Sent::Unknown("Request timed out; its reply is unknown".into()));
        match sent {
            Sent::Acknowledged {
                revision,
                remote_id,
            } => self.resolve(
                action,
                &Resolution::Confirmed {
                    revision,
                    remote_id,
                },
            )?,
            Sent::Rejected(error) => {
                self.store
                    .sqlite
                    .reject_tracker_action(&action.id, &error)
                    .map_err(message)?;
                return Ok(Step::Wait);
            }
            Sent::Unknown(error) => {
                self.error(action, &error)?;
                return Ok(Step::Wait);
            }
            Sent::Unsupported(reason) => self.resolve(action, &Resolution::Retire { reason })?,
        }
        Ok(Step::Done)
    }

    fn error(&self, action: &TrackerAction, error: &str) -> OpsResult<()> {
        self.store
            .sqlite
            .tracker_action_error(&action.id, error)
            .map_err(message)
    }

    fn resolve(&self, action: &TrackerAction, resolution: &Resolution) -> OpsResult<()> {
        if self
            .store
            .sqlite
            .resolve_tracker_action(&action.id, resolution)
            .map_err(message)?
        {
            self.resolved.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use loopflow_model::durable::TaskId;
    use loopflow_storage::planning_sync::TrackerKind;
    use loopflow_storage::sqlite::tracker_actions::{ActionKind, ActionOwner, ActionPhase};
    use loopflow_storage::work::edit::WorkValue;
    use serde_json::json;

    use super::{drain, queued, Bound, Pass, Scope};
    use crate::ops::tracker::fake::Fake;
    use crate::ops::tracker::Tracker;
    use crate::store::Store;
    use loopflow_storage::planning::rules::Observed;

    struct Fixture {
        _home: tempfile::TempDir,
        path: std::path::PathBuf,
        store: Store,
        fake: Arc<Fake>,
        tracker: Tracker,
    }

    fn fixture() -> Fixture {
        fixture_with(Fake::new())
    }

    fn fixture_with(fake: Fake) -> Fixture {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = Store {
            sqlite: crate::store::opening::open_ephemeral(&path).unwrap(),
        };
        let fake = Arc::new(fake);
        Fixture {
            _home: home,
            path,
            store,
            tracker: Tracker::Fake(fake.clone()),
            fake,
        }
    }

    impl Fixture {
        fn bound_task(&self) -> TaskId {
            let id = TaskId::new();
            self.store
                .sqlite
                .create_task(&loopflow_storage::work::task::NewTask {
                    parent_task_id: None,
                    repository_id: loopflow_storage::test_fixture::repository(
                        &self.store.sqlite,
                        "/repo",
                    ),
                    id: id.clone(),
                    project_id: None,
                    title: "Title".into(),
                    description: String::new(),
                    due_date: None,
                })
                .unwrap();
            self.sql(
                "UPDATE tasks SET external_issue_id=?2 WHERE id=?1",
                &[id.as_str(), &uuid::Uuid::new_v4().to_string()],
            );
            id
        }

        fn sql(&self, sql: &str, values: &[&str]) {
            rusqlite::Connection::open(&self.path)
                .unwrap()
                .execute(sql, rusqlite::params_from_iter(values))
                .unwrap();
        }

        /// Queue a title edit; `phase` and `looked_at` set its delivery history.
        fn title(&self, task: &TaskId, value: &str, phase: &str, looked_at: Option<i64>) {
            let looked = looked_at.map(|at| at.to_string()).unwrap_or_default();
            self.sql(
                "INSERT INTO tracker_actions(id,task_id,subject,kind,value_json,base_json,phase,tracker,looked_at)
                 VALUES(?1,?2,'task_title','field',?3,?4,?5,'linear',NULLIF(?6,''))",
                &[
                    &uuid::Uuid::new_v4().to_string(),
                    task.as_str(),
                    &json!({"field":"task_title","value":value}).to_string(),
                    &json!({"revision":"2026-11-01T00:00:00Z","value":{"field":"task_title","value":"Before"}}).to_string(),
                    phase,
                    &looked,
                ],
            );
        }

        async fn pass(&self) -> Pass {
            self.drain(Scope::All).await
        }

        async fn drain(&self, scope: Scope<'_>) -> Pass {
            let actions = queued(&self.store, "/repo", TrackerKind::Linear).unwrap();
            drain(
                &self.store,
                "/repo",
                &self.tracker,
                actions,
                scope,
                Bound::PASS,
            )
            .await
            .unwrap()
        }

        fn phases(&self, task: &TaskId) -> Vec<ActionPhase> {
            self.store
                .sqlite
                .tracker_actions_for(&ActionOwner::Task(task.clone()))
                .unwrap()
                .into_iter()
                .map(|action| action.phase)
                .collect()
        }

        fn hold(&self, value: &str) {
            *self.fake.held.lock().unwrap() = Some(Observed {
                revision: Some("2026-11-02T00:00:00Z".into()),
                values: vec![WorkValue::TaskTitle(Some(value.into()))],
                ..Observed::default()
            });
        }
    }

    #[tokio::test]
    async fn a_pass_with_nothing_unconfirmed_sends_no_request() {
        let f = fixture();
        f.bound_task();
        assert_eq!(f.pass().await, Pass::default());
        assert_eq!(f.fake.requests(), 0);
    }

    #[tokio::test]
    async fn a_pass_sends_at_most_twenty_requests() {
        let f = fixture();
        for _ in 0..100 {
            let task = f.bound_task();
            f.title(&task, "Renamed", "queued", None);
        }
        let pass = f.pass().await;
        assert_eq!(f.fake.requests(), 20);
        assert_eq!(
            pass,
            Pass {
                requests: 20,
                resolved: 20
            }
        );
        assert_eq!(
            f.store
                .sqlite
                .unconfirmed_tracker_actions("/repo")
                .unwrap()
                .len(),
            80
        );
    }

    #[tokio::test]
    async fn a_save_goes_ahead_of_a_backlog_larger_than_one_pass() {
        let f = fixture();
        for _ in 0..30 {
            let task = f.bound_task();
            f.title(&task, "Older", "queued", None);
        }
        let saved = f.bound_task();
        f.title(&saved, "Just saved", "queued", None);
        let owner = ActionOwner::Task(saved.clone());
        f.drain(Scope::First(&owner)).await;
        assert_eq!(f.fake.requests(), 20);
        assert_eq!(f.phases(&saved), [ActionPhase::Confirmed]);
    }

    #[tokio::test]
    async fn a_lost_reply_waits_for_its_window_and_holds_its_owner() {
        let f = fixture();
        let waiting = f.bound_task();
        let now = loopflow_storage::rows::now_unix();
        f.title(&waiting, "Sent", "sent", Some(now));
        f.title(&waiting, "Later", "queued", None);
        let other = f.bound_task();
        f.title(&other, "Other", "queued", None);
        f.pass().await;
        assert_eq!(f.fake.requests(), 1, "only the other owner sends");
        assert_eq!(f.phases(&waiting), [ActionPhase::Sent, ActionPhase::Queued]);
        assert_eq!(f.phases(&other), [ActionPhase::Confirmed]);
    }

    #[tokio::test]
    async fn a_failing_idempotent_send_waits_its_window_unless_synced_now() {
        let mut fake = Fake::new();
        fake.idempotent = vec![ActionKind::Field];
        let f = fixture_with(fake);
        let task = f.bound_task();
        f.title(&task, "Renamed", "sent", None);
        let now = loopflow_storage::rows::now_unix().to_string();
        f.sql("UPDATE tracker_actions SET sent_at=?1", &[&now]);
        f.pass().await;
        f.pass().await;
        assert_eq!(f.fake.requests(), 0, "no pass resends inside the window");
        assert_eq!(f.phases(&task), [ActionPhase::Sent]);

        let owner = ActionOwner::Task(task.clone());
        let actions = queued(&f.store, "/repo", TrackerKind::Linear).unwrap();
        drain(
            &f.store,
            "/repo",
            &f.tracker,
            actions,
            Scope::Only(&owner),
            Bound::NOW,
        )
        .await
        .unwrap();
        assert_eq!(f.fake.requests(), 1);
        assert_eq!(f.phases(&task), [ActionPhase::Confirmed]);
    }

    #[tokio::test]
    async fn a_reply_another_lf_lost_after_this_pass_read_is_not_sent_again() {
        let f = fixture();
        let task = f.bound_task();
        f.title(&task, "Renamed", "queued", None);
        let read = queued(&f.store, "/repo", TrackerKind::Linear).unwrap();
        let now = loopflow_storage::rows::now_unix().to_string();
        f.sql(
            "UPDATE tracker_actions SET phase='sent',sent_at=?1,looked_at=?1",
            &[&now],
        );
        drain(&f.store, "/repo", &f.tracker, read, Scope::All, Bound::PASS)
            .await
            .unwrap();
        assert_eq!(f.fake.requests(), 0);
        assert_eq!(f.phases(&task), [ActionPhase::Sent]);
    }

    #[tokio::test]
    async fn a_lost_reply_found_in_the_tracker_resolves_with_one_look() {
        let f = fixture();
        let task = f.bound_task();
        f.title(&task, "Sent", "sent", None);
        f.hold("Sent");
        f.pass().await;
        assert_eq!(f.fake.requests(), 1);
        assert_eq!(f.phases(&task), [ActionPhase::Confirmed]);
    }

    #[tokio::test]
    async fn a_conflict_adopts_the_tracker_and_keeps_the_losing_value() {
        let f = fixture();
        let task = f.bound_task();
        f.title(&task, "Mine", "sent", None);
        f.hold("Theirs");
        f.pass().await;
        let action = &f
            .store
            .sqlite
            .tracker_actions_for(&ActionOwner::Task(task))
            .unwrap()[0];
        assert_eq!(action.phase, ActionPhase::Conflicted);
        assert_eq!(action.value, json!({"field":"task_title","value":"Mine"}));
        assert_eq!(
            action.conflict,
            Some(
                json!({"revision":"2026-11-02T00:00:00Z","value":{"field":"task_title","value":"Theirs"}})
            )
        );
    }

    #[tokio::test]
    async fn a_relation_waits_until_its_target_has_a_tracker_object() {
        let f = fixture();
        let source = f.bound_task();
        let target = f.bound_task();
        f.sql(
            "UPDATE tasks SET external_issue_id=NULL WHERE id=?1",
            &[target.as_str()],
        );
        let relation = uuid::Uuid::new_v4().to_string();
        f.sql(
            "INSERT INTO tracker_actions(id,task_id,subject,kind,value_json,phase,tracker)
             VALUES(?1,?2,'relation:'||?1,'link',json_object('target',?3),'queued','linear')",
            &[&relation, source.as_str(), target.as_str()],
        );
        f.title(&source, "Renamed", "queued", None);
        f.pass().await;
        assert_eq!(
            f.phases(&source),
            [ActionPhase::Queued, ActionPhase::Confirmed]
        );
        f.sql(
            "UPDATE tasks SET external_issue_id=?2 WHERE id=?1",
            &[target.as_str(), &uuid::Uuid::new_v4().to_string()],
        );
        let before = f.fake.requests();
        f.pass().await;
        assert_eq!(f.fake.requests(), before + 1);
        assert_eq!(
            f.phases(&source),
            [ActionPhase::Confirmed, ActionPhase::Confirmed]
        );
    }

    #[tokio::test]
    async fn an_action_for_another_tracker_retires_without_a_request() {
        let f = fixture();
        let task = f.bound_task();
        f.title(&task, "Renamed", "queued", None);
        f.sql("UPDATE tracker_actions SET tracker='asana'", &[]);
        f.pass().await;
        assert_eq!(f.fake.requests(), 0);
        assert_eq!(f.phases(&task), [ActionPhase::Retired]);
    }
}
