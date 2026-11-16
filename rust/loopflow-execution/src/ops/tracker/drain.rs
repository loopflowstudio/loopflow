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
use loopflow_storage::sqlite::tracker_effects::{
    EffectKind, EffectOwner, EffectPhase, Settlement, TrackerEffect,
};
use loopflow_storage::work::edit::WorkValue;

use super::{Sent, Tracker};
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
    pub settled: usize,
}

struct Budget {
    left: AtomicUsize,
    used: AtomicUsize,
    settled: AtomicUsize,
    until: Instant,
}

impl Budget {
    fn take(&self) -> bool {
        Instant::now() < self.until
            && self
                .left
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                    left.checked_sub(1)
                })
                .is_ok()
            && {
                self.used.fetch_add(1, Ordering::SeqCst);
                true
            }
    }
}

fn message(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
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
    if provider == loopflow_storage::provider::Provider::Linear {
        crate::ops::linear::sync_retained_relations(store, Path::new(repo)).await?;
    }
    store
        .sqlite
        .enqueue_tracker_creations(repo, provider)
        .map_err(message)?;
    let unsettled = store
        .sqlite
        .unsettled_tracker_effects(repo)
        .map_err(message)?;
    if unsettled.is_empty() {
        return Ok(Pass::default());
    }
    // Only a creation puts an item in the repository's Team, which Linear
    // validates once per process; edits address items that already name it.
    let tracker = if unsettled
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
            for effect in &unsettled {
                store
                    .sqlite
                    .tracker_effect_error(&effect.id, &error.to_string())
                    .map_err(message)?;
            }
            return Ok(Pass::default());
        }
    };
    drain(store, repo, &tracker, scope, bound).await
}

/// Drain `repo`'s queue to `tracker` within `bound`.
pub async fn drain(
    store: &Store,
    repo: &str,
    tracker: &Tracker,
    scope: Scope<'_>,
    bound: Bound,
) -> OpsResult<Pass> {
    store
        .sqlite
        .enqueue_tracker_creations(repo, tracker.provider())
        .map_err(message)?;
    let effects = store
        .sqlite
        .unsettled_tracker_effects(repo)
        .map_err(message)?;
    let creating: HashSet<String> = effects
        .iter()
        .filter(|effect| effect.kind == EffectKind::Create)
        .filter_map(|effect| match &effect.owner {
            EffectOwner::Project(id) => Some(id.to_string()),
            _ => None,
        })
        .collect();
    let budget = Budget {
        left: AtomicUsize::new(bound.requests),
        used: AtomicUsize::new(0),
        settled: AtomicUsize::new(0),
        until: Instant::now() + bound.time,
    };
    let window = if bound.windows {
        LOOK_WINDOW_SECS.max(crate::linear::delta_fresh_secs())
    } else {
        0
    };
    let mut owners = grouped(effects, scope);
    // Owners run concurrently, so a Task can wait on a Project created in the
    // same round; those get one more turn once the round has settled it.
    for round in 0..2 {
        if round == 1 {
            let effects = store
                .sqlite
                .unsettled_tracker_effects(repo)
                .map_err(message)?;
            owners = grouped(effects, scope);
            owners.retain(|(_, queued)| {
                queued.iter().any(|effect| {
                    parent_named(effect).is_some_and(|project| creating.contains(&project))
                        && !parent_pending(store, effect, &creating)
                })
            });
            if owners.is_empty() {
                break;
            }
        }
        let results: Vec<OpsResult<()>> = futures_util::stream::iter(std::mem::take(&mut owners))
            .map(|(owner, queued)| {
                drain_owner(
                    store,
                    Path::new(repo),
                    tracker,
                    owner,
                    queued,
                    &creating,
                    &budget,
                    window,
                )
            })
            .buffer_unordered(bound.concurrency.max(1))
            .collect()
            .await;
        for result in results {
            if let Err(error) = result {
                tracing::debug!(%error, "tracker effect pending");
            }
        }
    }
    Ok(Pass {
        requests: budget.used.load(Ordering::SeqCst),
        settled: budget.settled.load(Ordering::SeqCst),
    })
}

// Another lf on this store may be sending for the same object; the lock file
// keeps the two from interleaving requests.
fn lock(store: &Store, owner: &EffectOwner) -> OpsResult<Option<std::fs::File>> {
    let (kind, id) = match owner {
        EffectOwner::Task(id) => ("task", id.to_string()),
        EffectOwner::Project(id) => ("project", id.to_string()),
        EffectOwner::Wave(id) => ("wave", id.to_string()),
    };
    let path = store
        .sqlite
        .home_dir()
        .map_err(message)?
        .join("locks/planning-fields")
        .join(format!("{kind}-{id}.lock"));
    Ok(lock_delivery(&path)?)
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

async fn look(
    store: &Store,
    repo: &Path,
    tracker: &Tracker,
    effect: &TrackerEffect,
) -> Result<Option<Observed>, String> {
    tokio::time::timeout(REQUEST_DEADLINE, tracker.look(store, repo, effect))
        .await
        .unwrap_or_else(|_| Err("Look timed out; the effect waits for the next pass".into()))
}

async fn send(store: &Store, repo: &Path, tracker: &Tracker, effect: &TrackerEffect) -> Sent {
    tokio::time::timeout(REQUEST_DEADLINE, tracker.send(store, repo, effect))
        .await
        .unwrap_or_else(|_| Sent::Unknown("Request timed out; its reply is unknown".into()))
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
        EffectKind::Create => serde_json::from_value::<Vec<WorkValue>>(effect.value.clone())
            .ok()
            .and_then(|values| {
                values.into_iter().find_map(|value| match value {
                    WorkValue::TaskProject(project) => project,
                    _ => None,
                })
            }),
        EffectKind::Field => match serde_json::from_value(effect.value.clone()) {
            Ok(WorkValue::TaskProject(project)) => project,
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

/// Whether this effect names a Project being created that has no remote id yet.
fn parent_pending(store: &Store, effect: &TrackerEffect, creating: &HashSet<String>) -> bool {
    let Some(project) = parent_named(effect).filter(|project| creating.contains(project)) else {
        return false;
    };
    store
        .sqlite
        .project(&loopflow_model::durable::ProjectId::from_raw(project))
        .ok()
        .flatten()
        .is_none_or(|project| project.linear_id.is_none())
}

#[allow(clippy::too_many_arguments)]
async fn drain_owner(
    store: &Store,
    repo: &Path,
    tracker: &Tracker,
    owner: EffectOwner,
    queued: Vec<TrackerEffect>,
    creating: &HashSet<String>,
    budget: &Budget,
    window: i64,
) -> OpsResult<()> {
    let Some(_lock) = lock(store, &owner)? else {
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
        match drain_effect(store, repo, tracker, effect, creating, budget, window).await? {
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

/// What one effect left its owner's pass to do next.
enum Step {
    Done,
    /// This effect waits; later effects on its subject wait with it.
    Wait,
    /// The pass's budget is spent.
    Spent,
}

async fn drain_effect(
    store: &Store,
    repo: &Path,
    tracker: &Tracker,
    effect: TrackerEffect,
    creating: &HashSet<String>,
    budget: &Budget,
    window: i64,
) -> OpsResult<Step> {
    if effect.provider != tracker.provider() {
        let reason = format!(
            "queued for {}, but this repository uses {}",
            effect.provider.display_name(),
            tracker.provider().display_name()
        );
        settle(store, budget, &effect, &Settlement::Retire { reason })?;
        return Ok(Step::Done);
    }
    // An owner bound some other way (adopted, linked by hand) already has its
    // tracker object; sending the creation would make a second one.
    if effect.kind == EffectKind::Create {
        if let Some(remote) = bound(store, &effect.owner)? {
            settle(
                store,
                budget,
                &effect,
                &Settlement::Acknowledged {
                    revision: None,
                    remote_id: Some(remote),
                },
            )?;
            return Ok(Step::Done);
        }
    }
    let parent = parent_pending(store, &effect, creating);
    let decision = match effect.phase {
        EffectPhase::Attempted => {
            let now = loopflow_storage::rows::now_unix();
            match rules::lost_reply(&effect, tracker.idempotent(effect.kind), now, window) {
                LostReply::Wait => return Ok(Step::Wait),
                LostReply::Resend => Decision::Send,
                LostReply::Look => {
                    if !budget.take() {
                        return Ok(Step::Spent);
                    }
                    let observed = look(store, repo, tracker, &effect).await;
                    store
                        .sqlite
                        .look_tracker_effect(&effect.id, now)
                        .map_err(message)?;
                    match observed {
                        Ok(observed) => rules::decide(&effect, observed.as_ref(), parent),
                        Err(error) => {
                            store
                                .sqlite
                                .tracker_effect_error(&effect.id, &error)
                                .map_err(message)?;
                            return Ok(Step::Wait);
                        }
                    }
                }
            }
        }
        _ if parent => Decision::Hold,
        _ if effect.kind == EffectKind::Order => {
            // Moving Tasks needs the provider's complete list first.
            if !budget.take() {
                return Ok(Step::Spent);
            }
            match look(store, repo, tracker, &effect).await {
                Ok(observed) => rules::decide(&effect, observed.as_ref(), parent),
                Err(error) => {
                    store
                        .sqlite
                        .tracker_effect_error(&effect.id, &error)
                        .map_err(message)?;
                    return Ok(Step::Wait);
                }
            }
        }
        _ => Decision::Send,
    };
    match decision {
        Decision::Hold => return Ok(Step::Wait),
        Decision::Settle(settlement) => settle(store, budget, &effect, &settlement)?,
        Decision::Send => {
            if !budget.take() {
                return Ok(Step::Spent);
            }
            if !store
                .sqlite
                .attempt_tracker_effect(&effect.id)
                .map_err(message)?
            {
                return Ok(Step::Done);
            }
            match send(store, repo, tracker, &effect).await {
                Sent::Acknowledged {
                    revision,
                    remote_id,
                } => settle(
                    store,
                    budget,
                    &effect,
                    &Settlement::Acknowledged {
                        revision,
                        remote_id,
                    },
                )?,
                Sent::Rejected(error) => {
                    store
                        .sqlite
                        .reject_tracker_effect(&effect.id, &error)
                        .map_err(message)?;
                    return Ok(Step::Wait);
                }
                Sent::Unknown(error) => {
                    store
                        .sqlite
                        .tracker_effect_error(&effect.id, &error)
                        .map_err(message)?;
                    return Ok(Step::Wait);
                }
                Sent::Unsupported(reason) => {
                    settle(store, budget, &effect, &Settlement::Retire { reason })?
                }
            }
        }
    }
    Ok(Step::Done)
}

fn settle(
    store: &Store,
    budget: &Budget,
    effect: &TrackerEffect,
    settlement: &Settlement,
) -> OpsResult<()> {
    if store
        .sqlite
        .settle_tracker_effect(&effect.id, settlement)
        .map_err(message)?
    {
        budget.settled.fetch_add(1, Ordering::SeqCst);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use loopflow_model::durable::TaskId;
    use loopflow_storage::provider::Provider;
    use loopflow_storage::sqlite::tracker_effects::{EffectOwner, EffectPhase};
    use loopflow_storage::work::edit::WorkValue;
    use serde_json::json;

    use super::{drain, Bound, Pass, Scope};
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
            drain(&self.store, "/repo", &self.tracker, Scope::All, Bound::PASS)
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
