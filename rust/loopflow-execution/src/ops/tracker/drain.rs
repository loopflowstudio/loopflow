//! One bounded pass over the outbound queue. Owners go in order of their
//! oldest unsettled effect; within an owner effects go strictly in `seq`
//! order, so an owner whose head waits holds its later effects and no one
//! else's. A pass stops after its request or time budget, whichever is first.

use std::collections::HashSet;
use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use loopflow_storage::planning::rules::{self, Decision, LostReply, Observed};
use loopflow_storage::provider::Provider;
use loopflow_storage::sqlite::tracker_effects::{
    EffectKind, EffectOwner, EffectPhase, Settlement, TaskLink, TrackerEffect,
};
use loopflow_storage::work::edit::WorkValue;

use super::{message, value, values, Sent, Tracker};
use crate::ops::OpsResult;
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
    pub settled: usize,
}

/// Which owners a pass drains.
#[derive(Debug, Clone, Copy)]
pub enum Scope<'a> {
    All,
    /// The owner just saved goes ahead of older work.
    First(&'a EffectOwner),
    /// `lf task sync <id>`: this owner only.
    Only(&'a EffectOwner),
}

/// One bounded pass for `repo` when it has a tracker. With nothing unsettled
/// it asks the provider nothing, not even to build the bridge.
pub(crate) async fn drain_repository(
    store: &Store,
    repo: &str,
    scope: Scope<'_>,
    bound: Bound,
) -> OpsResult<Pass> {
    let Some(provider) = super::configured(Path::new(repo)) else {
        return Ok(Pass::default());
    };
    let effects = queued(store, repo, provider)?;
    if effects.is_empty() {
        return Ok(Pass::default());
    }
    // Only a creation puts an item in the repository's Team, which Linear
    // validates once per process; edits address items that already name it.
    let tracker = if effects
        .iter()
        .any(|effect| effect.kind == EffectKind::Create)
    {
        Tracker::for_repository(Path::new(repo)).await
    } else {
        Tracker::for_items(Path::new(repo)).await
    };
    let tracker = match tracker {
        Ok(tracker) => tracker,
        // Offline or signed out: the queue keeps every effect and says why.
        Err(error) => {
            for effect in &effects {
                store
                    .sqlite
                    .tracker_effect_error(&effect.id, &error.to_string())
                    .map_err(message)?;
            }
            return Ok(Pass::default());
        }
    };
    drain(store, repo, &tracker, effects, scope, bound).await
}

/// The repository's unsettled effects, after queueing a creation for each
/// owner made before the tracker was connected.
fn queued(store: &Store, repo: &str, provider: Provider) -> OpsResult<Vec<TrackerEffect>> {
    store
        .sqlite
        .enqueue_tracker_creations(repo, provider)
        .map_err(message)?;
    store
        .sqlite
        .unsettled_tracker_effects(repo)
        .map_err(message)
}

/// Drain `effects` to `tracker` within `bound`.
async fn drain(
    store: &Store,
    repo: &str,
    tracker: &Tracker,
    effects: Vec<TrackerEffect>,
    scope: Scope<'_>,
    bound: Bound,
) -> OpsResult<Pass> {
    let creating: HashSet<String> = effects
        .iter()
        .filter(|effect| effect.kind == EffectKind::Create)
        .filter_map(|effect| match &effect.owner {
            EffectOwner::Project(id) => Some(id.to_string()),
            _ => None,
        })
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
        settled: AtomicUsize::new(0),
    };
    pass.round(grouped(effects, scope), bound.concurrency).await;
    // Owners run concurrently, so a Task can wait on a Project created in the
    // same round; those get one more turn once the round has settled it.
    if !pass.creating.is_empty() {
        let mut owners = grouped(
            store
                .sqlite
                .unsettled_tracker_effects(repo)
                .map_err(message)?,
            scope,
        );
        owners.retain(|(_, queued)| {
            queued.iter().any(|effect| {
                parent_named(effect).is_some_and(|project| pass.creating.contains(&project))
                    && !pass.parent_pending(effect)
            })
        });
        pass.round(owners, bound.concurrency).await;
    }
    Ok(Pass {
        requests: pass.used.load(Ordering::SeqCst),
        settled: pass.settled.load(Ordering::SeqCst),
    })
}

/// Effects by owner, owners in order of their oldest effect, narrowed to `scope`.
fn grouped(
    effects: Vec<TrackerEffect>,
    scope: Scope<'_>,
) -> Vec<(EffectOwner, Vec<TrackerEffect>)> {
    let mut owners: Vec<(EffectOwner, Vec<TrackerEffect>)> = Vec::new();
    for effect in effects {
        match owners.iter_mut().find(|(owner, _)| *owner == effect.owner) {
            Some((_, queued)) => queued.push(effect),
            None => owners.push((effect.owner.clone(), vec![effect])),
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

/// The Project a Task creation or membership edit names.
fn parent_named(effect: &TrackerEffect) -> Option<String> {
    match effect.kind {
        EffectKind::Create => values(effect)
            .ok()?
            .into_iter()
            .find_map(|value| match value {
                WorkValue::TaskProject(project) => project,
                _ => None,
            }),
        EffectKind::Field => match value(effect).ok()? {
            WorkValue::TaskProject(project) => project,
            _ => None,
        },
        _ => None,
    }
}

/// The tracker id an owner is bound to.
fn bound(store: &Store, owner: &EffectOwner) -> OpsResult<Option<String>> {
    Ok(match owner {
        EffectOwner::Task(id) => store
            .sqlite
            .task(id)
            .map_err(message)?
            .and_then(|task| task.linear_id)
            .map(|id| id.as_str().to_owned()),
        EffectOwner::Project(id) => store
            .sqlite
            .project(id)
            .map_err(message)?
            .and_then(|project| project.linear_id)
            .map(|id| id.as_str().to_owned()),
        EffectOwner::Wave(_) => None,
    })
}

/// Serialize provider effects only; saves and acquisition never take this lock.
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

/// What one effect left its owner's pass to do next.
enum Step {
    Done,
    /// This effect waits; later effects on its subject wait with it.
    Wait,
    /// The pass's budget is spent.
    Spent,
}

/// One pass: where it sends, what it may spend, and what it has done.
struct Drain<'a> {
    store: &'a Store,
    repo: &'a Path,
    tracker: &'a Tracker,
    /// Projects whose creation is queued.
    creating: HashSet<String>,
    limit: usize,
    until: Instant,
    /// Seconds between looks for one lost reply; 0 looks now.
    window: i64,
    used: AtomicUsize,
    settled: AtomicUsize,
}

impl Drain<'_> {
    async fn round(&self, owners: Vec<(EffectOwner, Vec<TrackerEffect>)>, concurrency: usize) {
        let results: Vec<OpsResult<()>> = futures_util::stream::iter(owners)
            .map(|(owner, queued)| self.owner(owner, queued))
            .buffer_unordered(concurrency.max(1))
            .collect()
            .await;
        for error in results.into_iter().filter_map(Result::err) {
            tracing::debug!(%error, "tracker effect pending");
        }
    }

    /// Spend one request of the pass, if any is left.
    fn take(&self) -> bool {
        Instant::now() < self.until
            && self
                .used
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
                    (used < self.limit).then_some(used + 1)
                })
                .is_ok()
    }

    /// Whether this effect names a Project being created that has no remote id yet.
    fn parent_pending(&self, effect: &TrackerEffect) -> bool {
        let Some(project) = parent_named(effect).filter(|project| self.creating.contains(project))
        else {
            return false;
        };
        self.store
            .sqlite
            .project(&loopflow_model::durable::ProjectId::from_raw(project))
            .ok()
            .flatten()
            .is_none_or(|project| project.linear_id.is_none())
    }

    /// Whether this effect relates its Task to one with no tracker object yet.
    fn target_pending(&self, effect: &TrackerEffect) -> bool {
        let Some(TaskLink::Relation { target, .. }) = effect.task_link() else {
            return false;
        };
        !matches!(bound(self.store, &EffectOwner::Task(target)), Ok(Some(_)))
    }

    // Another lf on this store may be sending for the same object; the lock
    // file keeps the two from interleaving requests.
    fn lock(&self, owner: &EffectOwner) -> OpsResult<Option<File>> {
        let (kind, id) = match owner {
            EffectOwner::Task(id) => ("task", id.to_string()),
            EffectOwner::Project(id) => ("project", id.to_string()),
            EffectOwner::Wave(id) => ("wave", id.to_string()),
        };
        let path = self
            .store
            .sqlite
            .home_dir()
            .map_err(message)?
            .join("locks/planning-fields")
            .join(format!("{kind}-{id}.lock"));
        Ok(lock_delivery(&path)?)
    }

    async fn owner(&self, owner: EffectOwner, queued: Vec<TrackerEffect>) -> OpsResult<()> {
        let Some(_lock) = self.lock(&owner)? else {
            return Ok(());
        };
        // Everything on an owner waits behind its unsettled creation; otherwise
        // only effects on the same subject wait behind each other.
        let mut waiting = HashSet::new();
        for effect in queued {
            let key = (effect.kind, effect.subject.clone());
            if waiting.contains(&key) {
                continue;
            }
            match self.effect(effect).await? {
                Step::Done => {}
                Step::Wait if key.0 == EffectKind::Create => return Ok(()),
                Step::Wait => {
                    waiting.insert(key);
                }
                Step::Spent => return Ok(()),
            }
        }
        Ok(())
    }

    async fn effect(&self, effect: TrackerEffect) -> OpsResult<Step> {
        let provider = self.tracker.provider();
        if effect.provider != provider {
            let reason = format!(
                "queued for {}, but this repository uses {}",
                effect.provider.display_name(),
                provider.display_name()
            );
            self.settle(&effect, &Settlement::Retire { reason })?;
            return Ok(Step::Done);
        }
        // An owner bound some other way (adopted, linked by hand) already has its
        // tracker object; sending the creation would make a second one.
        if effect.kind == EffectKind::Create {
            if let Some(remote) = bound(self.store, &effect.owner)? {
                let settlement = Settlement::Acknowledged {
                    revision: None,
                    remote_id: Some(remote),
                };
                self.settle(&effect, &settlement)?;
                return Ok(Step::Done);
            }
        }
        let parent = self.parent_pending(&effect) || self.target_pending(&effect);
        let looked = match effect.phase {
            EffectPhase::Attempted => {
                let now = loopflow_storage::rows::now_unix();
                let idempotent = self.tracker.idempotent(effect.kind);
                match rules::lost_reply(&effect, idempotent, now, self.window) {
                    LostReply::Wait => return Ok(Step::Wait),
                    LostReply::Resend => None,
                    LostReply::Look => Some(self.look(&effect).await?),
                }
            }
            _ if parent => return Ok(Step::Wait),
            // Moving Tasks needs the provider's complete list first.
            _ if effect.kind == EffectKind::Order => Some(self.look(&effect).await?),
            _ => None,
        };
        let decision = match looked {
            None => Decision::Send,
            Some(Ok(observed)) => rules::decide(&effect, observed.as_ref(), parent),
            Some(Err(step)) => return Ok(step),
        };
        match decision {
            Decision::Hold => return Ok(Step::Wait),
            Decision::Settle(settlement) => self.settle(&effect, &settlement)?,
            Decision::Send => return self.send(&effect).await,
        }
        Ok(Step::Done)
    }

    /// Read back the owner `effect` addresses. A look at a lost reply starts
    /// its window; a failed look saves its error and the effect waits.
    async fn look(&self, effect: &TrackerEffect) -> OpsResult<Result<Option<Observed>, Step>> {
        if !self.take() {
            return Ok(Err(Step::Spent));
        }
        let observed = tokio::time::timeout(
            REQUEST_DEADLINE,
            self.tracker.look(self.store, self.repo, effect),
        )
        .await
        .unwrap_or_else(|_| Err("Look timed out; the effect waits for the next pass".into()));
        if effect.phase == EffectPhase::Attempted {
            self.store
                .sqlite
                .look_tracker_effect(&effect.id, loopflow_storage::rows::now_unix())
                .map_err(message)?;
        }
        match observed {
            Ok(observed) => Ok(Ok(observed)),
            Err(error) => {
                self.error(effect, &error)?;
                Ok(Err(Step::Wait))
            }
        }
    }

    async fn send(&self, effect: &TrackerEffect) -> OpsResult<Step> {
        if !self.take() {
            return Ok(Step::Spent);
        }
        if !self
            .store
            .sqlite
            .attempt_tracker_effect(&effect.id)
            .map_err(message)?
        {
            return Ok(Step::Done);
        }
        let sent = tokio::time::timeout(
            REQUEST_DEADLINE,
            self.tracker.send(self.store, self.repo, effect),
        )
        .await
        .unwrap_or_else(|_| Sent::Unknown("Request timed out; its reply is unknown".into()));
        match sent {
            Sent::Acknowledged {
                revision,
                remote_id,
            } => self.settle(
                effect,
                &Settlement::Acknowledged {
                    revision,
                    remote_id,
                },
            )?,
            Sent::Rejected(error) => {
                self.store
                    .sqlite
                    .reject_tracker_effect(&effect.id, &error)
                    .map_err(message)?;
                return Ok(Step::Wait);
            }
            Sent::Unknown(error) => {
                self.error(effect, &error)?;
                return Ok(Step::Wait);
            }
            Sent::Unsupported(reason) => self.settle(effect, &Settlement::Retire { reason })?,
        }
        Ok(Step::Done)
    }

    fn error(&self, effect: &TrackerEffect, error: &str) -> OpsResult<()> {
        self.store
            .sqlite
            .tracker_effect_error(&effect.id, error)
            .map_err(message)
    }

    fn settle(&self, effect: &TrackerEffect, settlement: &Settlement) -> OpsResult<()> {
        if self
            .store
            .sqlite
            .settle_tracker_effect(&effect.id, settlement)
            .map_err(message)?
        {
            self.settled.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use loopflow_model::durable::TaskId;
    use loopflow_storage::provider::Provider;
    use loopflow_storage::sqlite::tracker_effects::{EffectOwner, EffectPhase};
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
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = Store {
            sqlite: crate::store::opening::open_ephemeral(&path).unwrap(),
        };
        let fake = Arc::new(Fake::new(Provider::Linear));
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
                "INSERT INTO tracker_effects(id,task_id,subject,kind,value_json,base_json,phase,provider,looked_at)
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
            let effects = queued(&self.store, "/repo", Provider::Linear).unwrap();
            drain(
                &self.store,
                "/repo",
                &self.tracker,
                effects,
                scope,
                Bound::PASS,
            )
            .await
            .unwrap()
        }

        fn phases(&self, task: &TaskId) -> Vec<EffectPhase> {
            self.store
                .sqlite
                .tracker_effects_for(&EffectOwner::Task(task.clone()))
                .unwrap()
                .into_iter()
                .map(|effect| effect.phase)
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
    async fn a_pass_with_nothing_unsettled_sends_no_request() {
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
            f.title(&task, "Renamed", "pending", None);
        }
        let pass = f.pass().await;
        assert_eq!(f.fake.requests(), 20);
        assert_eq!(
            pass,
            Pass {
                requests: 20,
                settled: 20
            }
        );
        assert_eq!(
            f.store
                .sqlite
                .unsettled_tracker_effects("/repo")
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
            f.title(&task, "Older", "pending", None);
        }
        let saved = f.bound_task();
        f.title(&saved, "Just saved", "pending", None);
        let owner = EffectOwner::Task(saved.clone());
        f.drain(Scope::First(&owner)).await;
        assert_eq!(f.fake.requests(), 20);
        assert_eq!(f.phases(&saved), [EffectPhase::Acknowledged]);
    }

    #[tokio::test]
    async fn a_lost_reply_waits_for_its_window_and_holds_its_owner() {
        let f = fixture();
        let waiting = f.bound_task();
        let now = loopflow_storage::rows::now_unix();
        f.title(&waiting, "Sent", "attempted", Some(now));
        f.title(&waiting, "Later", "pending", None);
        let other = f.bound_task();
        f.title(&other, "Other", "pending", None);
        f.pass().await;
        assert_eq!(f.fake.requests(), 1, "only the other owner sends");
        assert_eq!(
            f.phases(&waiting),
            [EffectPhase::Attempted, EffectPhase::Pending]
        );
        assert_eq!(f.phases(&other), [EffectPhase::Acknowledged]);
    }

    #[tokio::test]
    async fn a_lost_reply_found_in_the_tracker_settles_with_one_look() {
        let f = fixture();
        let task = f.bound_task();
        f.title(&task, "Sent", "attempted", None);
        f.hold("Sent");
        f.pass().await;
        assert_eq!(f.fake.requests(), 1);
        assert_eq!(f.phases(&task), [EffectPhase::Acknowledged]);
    }

    #[tokio::test]
    async fn a_conflict_adopts_the_tracker_and_keeps_the_losing_value() {
        let f = fixture();
        let task = f.bound_task();
        f.title(&task, "Mine", "attempted", None);
        f.hold("Theirs");
        f.pass().await;
        let effect = &f
            .store
            .sqlite
            .tracker_effects_for(&EffectOwner::Task(task))
            .unwrap()[0];
        assert_eq!(effect.phase, EffectPhase::Acknowledged);
        assert_eq!(effect.value, json!({"field":"task_title","value":"Mine"}));
        assert_eq!(
            effect.conflict,
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
            "INSERT INTO tracker_effects(id,task_id,subject,kind,value_json,phase,provider)
             VALUES(?1,?2,'relation:'||?1,'link',json_object('target',?3),'pending','linear')",
            &[&relation, source.as_str(), target.as_str()],
        );
        f.title(&source, "Renamed", "pending", None);
        f.pass().await;
        assert_eq!(
            f.phases(&source),
            [EffectPhase::Pending, EffectPhase::Acknowledged]
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
            [EffectPhase::Acknowledged, EffectPhase::Acknowledged]
        );
    }

    #[tokio::test]
    async fn an_effect_for_another_tracker_retires_without_a_request() {
        let f = fixture();
        let task = f.bound_task();
        f.title(&task, "Renamed", "pending", None);
        f.sql("UPDATE tracker_effects SET provider='asana'", &[]);
        f.pass().await;
        assert_eq!(f.fake.requests(), 0);
        assert_eq!(f.phases(&task), [EffectPhase::Retired]);
    }
}
