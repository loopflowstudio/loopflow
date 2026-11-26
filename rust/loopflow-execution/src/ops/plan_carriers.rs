//! A plan without Linear or Asana travels by a hub ([`super::plan_hub`]), a
//! Git remote ([`super::plan_git`]) or both. This is when the two run
//! together. [`send`] follows a save: the hub sends and reads, Git pushes
//! and reads only while something it brought was not taken.
//! [`sync`] reads and sends: when asked, before a claim or a new Wave, and
//! after a command the Machine server answers. [`read`] asks the hub alone,
//! for a read nobody asked for.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Result};

use loopflow_model::durable::{RepositoryId, TaskId};
use loopflow_storage::sqlite::plan_carriers::{Carrier, NotTaken};
use loopflow_storage::sqlite::planning_scope_checks::ScopeCheck;
use loopflow_storage::sqlite::SqliteStore;

use super::plan_hub::Why;

/// One exchange for `repo` by each carrier it has: the hub first, since it
/// answers in less time than a fetch and what it gives goes out by Git in the
/// same turn. Git runs whether or not the hub answered; the first failure is
/// returned. With `until`, both are over by then.
fn exchange(store: &SqliteStore, repo: &str, until: Option<Instant>) -> Result<()> {
    let why = until.map_or(Why::Asked, Why::Read);
    let by_hub = super::plan_hub::exchange(store, repo, why);
    super::plan_git::exchange(store, repo, until).and(by_hub)
}

/// Repositories with an open claim on this Machine, each checkout once.
pub(super) fn owned_repositories(store: &SqliteStore) -> Result<BTreeSet<String>> {
    let machine = store.saved_local_machine()?;
    Ok(store
        .held_planning_scope(&machine.id)?
        .repositories
        .into_values()
        .map(|(_, repo)| repo)
        .collect())
}

/// Background exchanges share an attempt across the server and all Graphs in
/// this checkout. Give Git its own deadline so an offline hub cannot starve it.
/// Returns `Ok(None)` without writing `planning_scope_checks` when the
/// repository directory is missing, connected to a tracker, has no carrier, or
/// its only carrier (Git) is currently backed off.
pub(super) async fn poll_owned(
    store: &crate::store::Store,
    repo: &str,
    window: i64,
) -> Result<Option<i64>> {
    let has_hub = match super::plan_hub::hub(&store.sqlite) {
        Ok(hub) => hub.is_some(),
        Err(error) => {
            tracing::debug!(%repo, error = %format!("{error:#}"), "owned plan hub lookup failed");
            false
        }
    };
    let now = SystemTime::now();
    poll_owned_with(
        store,
        repo,
        window,
        now,
        has_hub,
        || read(store, repo),
        |until| {
            on_thread(store, repo, move |sqlite, repo| {
                super::plan_git::exchange_background(sqlite, repo, Some(until))
            })
        },
    )
    .await
}

async fn poll_owned_with<FH, FG>(
    store: &crate::store::Store,
    repo: &str,
    window: i64,
    now: SystemTime,
    has_hub: bool,
    run_hub: impl FnOnce() -> FH,
    run_git: impl FnOnce(Instant) -> FG,
) -> Result<Option<i64>>
where
    FH: std::future::Future<Output = Result<()>>,
    FG: std::future::Future<Output = Result<()>>,
{
    if !Path::new(repo).is_dir() || crate::ops::tracker::connected(repo) {
        return Ok(None);
    }
    let git_due = super::plan_git::remote(Path::new(repo)).is_some()
        && !super::plan_git::is_backed_off(&store.sqlite, repo, now);
    if !has_hub && !git_due {
        return Ok(None);
    }
    let now_ts = now
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let scope = format!("plan:{repo}");
    if let ScopeCheck::Recent { attempted_at } =
        store.sqlite.begin_scope_check(&scope, now_ts, window)?
    {
        return Ok(Some(attempted_at));
    }
    let until = Instant::now() + WAIT;
    let hub = async {
        if has_hub {
            run_hub().await
        } else {
            Ok(())
        }
    };
    let git = async {
        if git_due {
            run_git(until).await
        } else {
            Ok(())
        }
    };
    let (hub, git) = tokio::join!(hub, git);
    for result in [hub, git] {
        if let Err(error) = result {
            tracing::debug!(%repo, error = %format!("{error:#}"), "owned plan exchange pending");
        }
    }
    Ok(Some(now_ts))
}

/// [`exchange`] for `repo`, when it has no Linear or Asana, for as long as
/// the carriers take.
pub async fn sync(store: &crate::store::Store, repo: &str) -> Result<()> {
    sync_until(store, repo, None).await
}

async fn sync_until(store: &crate::store::Store, repo: &str, until: Option<Instant>) -> Result<()> {
    on_thread(store, repo, move |store, repo| exchange(store, repo, until)).await
}

/// A read nobody asked for, riding on a provider exit: the hub only, for
/// [`WAIT`] at most. A fetch would cost the exit too long; Git is read by
/// [`sync`].
pub(crate) async fn read(store: &crate::store::Store, repo: &str) -> Result<()> {
    let until = Instant::now() + WAIT;
    on_thread(store, repo, move |store, repo| {
        super::plan_hub::exchange(store, repo, Why::Read(until))
    })
    .await
}

/// `carry` for `repo` on a blocking thread, when it has no Linear or Asana.
pub(super) async fn on_thread(
    store: &crate::store::Store,
    repo: &str,
    carry: impl FnOnce(&SqliteStore, &str) -> Result<()> + Send + 'static,
) -> Result<()> {
    if crate::ops::tracker::connected(repo) {
        return Ok(());
    }
    let (sqlite, repo) = (store.sqlite.clone(), repo.to_owned());
    crate::journal::request::spawn_blocking(move || carry(&sqlite, &repo)).await?
}

/// After a save in `repo`, which has no Linear or Asana: each carrier takes
/// the records past its cursor, the two at once and for [`WAIT`] at most.
/// The hub reads as it sends; Git reads only while something it brought was
/// not taken. A carrier that fails or runs out of time keeps its
/// records unsent for the next [`sync`]; the first failure is returned.
pub(crate) async fn send(store: &crate::store::Store, repo: &str) -> Result<()> {
    let until = Instant::now() + WAIT;
    let hub = {
        let (sqlite, repo) = (store.sqlite.clone(), repo.to_owned());
        crate::journal::request::spawn_blocking(move || {
            super::plan_hub::exchange(&sqlite, &repo, Why::Save(until))
        })
    };
    let git = {
        let (sqlite, repo) = (store.sqlite.clone(), repo.to_owned());
        crate::journal::request::spawn_blocking(move || {
            super::plan_git::send(&sqlite, &repo, until)
        })
    };
    let (hub, git) = tokio::time::timeout(WAIT, async { tokio::join!(hub, git) })
        .await
        .map_err(|_| anyhow!("the plan was still being sent after {} s", WAIT.as_secs()))?;
    git?.and(hub?)
}

/// How long a save, a claim, a first read or a read nobody asked for waits on
/// the carriers.
pub(super) const WAIT: Duration = Duration::from_secs(5);

/// Before this Machine claims `task`, which nobody owns as far as it has
/// seen: [`look_first`], so a claim already made elsewhere is read. Whichever
/// claim was first holds once they meet.
pub(crate) async fn before_claim(store: &crate::store::Store, repo: &Path, task: &TaskId) {
    if matches!(store.sqlite.task_owned(task), Ok(false)) {
        look_first(store, repo).await;
    }
}

/// Before this Machine makes something another may have made already (a
/// claim, a Wave of some name): one exchange by each carrier, so theirs is
/// read first. A carrier that fails or is stopped at [`WAIT`] leaves the
/// command to go ahead.
pub(crate) async fn look_first(store: &crate::store::Store, repo: &Path) {
    let repo = repo.to_string_lossy();
    went_on(
        &repo,
        sync_until(store, &repo, Some(Instant::now() + WAIT)).await,
    );
}

/// The carrier's own failure in `result`, for `lf repo tracker`. Records it
/// brought that this Machine could not write are not one: the carrier worked.
pub(super) fn carrier_error(result: &Result<()>) -> Option<String> {
    result
        .as_ref()
        .err()
        .filter(|error| !error.is::<NotTaken>())
        .map(|error| format!("{error:#}"))
}

/// How long an offline first read waits before trying again on a later read.
const FIRST_READ_COOLDOWN_SECS: i64 = 60;

/// A clone that has not read its plan from either current carrier reads it when
/// a non-subscribing CLI command first asks what the plan is (`lf task list`,
/// `lf repo tracker`, `lf plan list`, `lf plan board`, `lf wave list`,
/// `lf wave show`, `lf wave status`, `lf wave plan`), before the command opens
/// the store it reads. Served requests skip this and exchange after replying.
pub async fn before_first_read(directory: &Path) {
    if crate::journal::request::active() {
        return;
    }
    let Ok(Some((store, repo))) = carried(directory) else {
        return;
    };
    let until = Instant::now() + WAIT;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let exchanged = first_read_at(&store, &repo, until, now).await;
    went_on(&repo, exchanged.map(drop));
}

pub(super) async fn first_read_at(
    store: &SqliteStore,
    repo: &str,
    until: Instant,
    now: i64,
) -> Result<bool> {
    first_read_with(
        store,
        repo,
        until,
        now,
        |store, repo, until| {
            let (store, repo) = (store.clone(), repo.to_owned());
            async move {
                crate::journal::request::spawn_blocking(move || {
                    super::plan_git::exchange(&store, &repo, Some(until))
                })
                .await?
            }
        },
        |store, repo, until| {
            let (store, repo) = (store.clone(), repo.to_owned());
            async move {
                crate::journal::request::spawn_blocking(move || {
                    super::plan_hub::exchange(&store, &repo, Why::Read(until))
                })
                .await?
            }
        },
    )
    .await
}

pub(super) async fn first_read_with<FG, FH>(
    store: &SqliteStore,
    repo: &str,
    until: Instant,
    now: i64,
    run_git: impl FnOnce(&SqliteStore, &str, Instant) -> FG,
    run_hub: impl FnOnce(&SqliteStore, &str, Instant) -> FH,
) -> Result<bool>
where
    FG: std::future::Future<Output = Result<()>>,
    FH: std::future::Future<Output = Result<()>>,
{
    let remote = super::plan_git::remote(Path::new(repo));
    let git_row = match &remote {
        Some(peer) => store
            .plan_carrier(repo, Carrier::Git)?
            .filter(|held| held.peer == *peer),
        None => None,
    };
    let hub = match super::plan_hub::hub(store) {
        Ok(hub) => hub,
        Err(error) => {
            tracing::debug!(%repo, error = %format!("{error:#}"), "first read hub lookup failed");
            None
        }
    };
    let has_hub = hub.is_some() && store.repository_id(repo)?.is_some();
    let hub_row = match &hub {
        Some(hub) if has_hub => store
            .plan_carrier(repo, Carrier::Hub)?
            .filter(|held| held.peer == hub.id.as_str()),
        _ => None,
    };
    let has_git = remote.is_some();

    if (!has_git && !has_hub)
        || git_row.as_ref().is_some_and(|held| held.has_read())
        || hub_row.as_ref().is_some_and(|held| held.has_read())
    {
        return Ok(false);
    }

    let scope = format!("plan-first-read:{repo}");
    let errored = (!has_git || git_row.as_ref().is_some_and(|held| held.error.is_some()))
        && (!has_hub || hub_row.as_ref().is_some_and(|held| held.error.is_some()));
    let mut stamped = false;
    if errored {
        match store.begin_scope_check(&scope, now, FIRST_READ_COOLDOWN_SECS)? {
            ScopeCheck::Recent { .. } => return Ok(false),
            ScopeCheck::Started { .. } => stamped = true,
        }
    }

    let git = async {
        if has_git {
            Some(run_git(store, repo, until).await)
        } else {
            None
        }
    };
    let hub = async {
        if has_hub {
            Some(run_hub(store, repo, until).await)
        } else {
            None
        }
    };
    let (git, hub) = tokio::join!(git, hub);

    let mut succeeded = false;
    let mut carrier_failed = false;
    let mut first_error = None;
    for result in [git, hub].into_iter().flatten() {
        match result {
            Ok(()) => succeeded = true,
            Err(error) => {
                if error.is::<NotTaken>() {
                    succeeded = true;
                } else {
                    carrier_failed = true;
                }
                if first_error.is_none() {
                    first_error = Some(error);
                } else {
                    tracing::debug!(%repo, error = %format!("{error:#}"), "going on without the plan exchange");
                }
            }
        }
    }
    if succeeded {
        if let Some(error) = first_error {
            tracing::debug!(%repo, error = %format!("{error:#}"), "going on without the plan exchange");
        }
        return Ok(true);
    }
    if carrier_failed && !stamped {
        let _ = store.begin_scope_check(&scope, now, 0);
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(false),
    }
}

/// The store and repository of `directory` when the repository has no Linear
/// or Asana and something carries its plan: a Git remote, or a store that
/// may name a hub. A first Machine opens its store here.
fn carried(directory: &Path) -> Result<Option<(SqliteStore, String)>> {
    let repo = crate::repository::discover(directory)?.to_string();
    carried_store(&repo).map(|store| store.map(|store| (store, repo)))
}

fn carried_store(repo: &str) -> Result<Option<SqliteStore>> {
    let path = crate::store::default_db_path();
    if crate::ops::tracker::connected(repo)
        || (!path.exists() && super::plan_git::remote(Path::new(repo)).is_none())
    {
        return Ok(None);
    }
    Ok(Some(crate::store::open_sqlite(&path)?))
}

/// A command that read the plan first goes on without it.
fn went_on(repo: &str, exchanged: Result<()>) {
    if let Err(error) = exchanged {
        tracing::debug!(%repo, error = %format!("{error:#}"), "going on without the plan exchange");
    }
}

/// After a command on this Machine's server answers: one exchange for its
/// repository. Commands arriving while one runs, or within [`QUIET`] of the
/// last, are debounced *before* spawning a task or opening a store.
pub fn after_command(directory: PathBuf) {
    let Ok(repo) = crate::repository::discover(&directory).map(|r| r.to_string()) else {
        return;
    };
    {
        let mut turns = TURNS.lock().expect("plan turns are never poisoned");
        let turn = turns.entry(repo.clone()).or_default();
        if turn.running {
            turn.again = true;
            return;
        }
        if turn.finished.is_some_and(|at| at.elapsed() < QUIET) {
            return;
        }
        turn.running = true;
    }
    crate::runtime::handle().spawn_blocking(move || {
        if let Err(error) = exchange_after_command(&repo) {
            tracing::debug!(error = %format!("{error:#}"), "plan exchange after a command failed");
        }
    });
}

const QUIET: Duration = Duration::from_secs(5);

#[derive(Debug, Default)]
struct Turn {
    running: bool,
    again: bool,
    finished: Option<Instant>,
}

static TURNS: Mutex<BTreeMap<String, Turn>> = Mutex::new(BTreeMap::new());

fn exchange_after_command(repo: &str) -> Result<()> {
    struct FinishGuard<'a> {
        repo: &'a str,
        completed: bool,
    }
    impl Drop for FinishGuard<'_> {
        fn drop(&mut self) {
            if !self.completed {
                let mut turns = TURNS.lock().expect("plan turns are never poisoned");
                let turn = turns.entry(self.repo.to_owned()).or_default();
                turn.running = false;
                turn.again = false;
                turn.finished = Some(Instant::now());
            }
        }
    }
    let mut guard = FinishGuard {
        repo,
        completed: false,
    };
    let Some(store) = carried_store(repo)? else {
        return Ok(());
    };
    loop {
        let result = exchange(&store, repo, None);
        let mut turns = TURNS.lock().expect("plan turns are never poisoned");
        let turn = turns.entry(repo.to_owned()).or_default();
        if std::mem::take(&mut turn.again) && result.is_ok() {
            continue;
        }
        turn.running = false;
        turn.finished = Some(Instant::now());
        guard.completed = true;
        return result;
    }
}

/// One exchange per carrier (`plan-hub`, `plan-git`) and repository at a
/// time, across processes; the next reads what this one wrote. Waits for the
/// one running, until `until` when that is given.
pub(super) fn lock(
    store: &SqliteStore,
    carrier: &str,
    repository: &RepositoryId,
    until: Option<Instant>,
) -> Result<std::fs::File> {
    let path = store
        .home_dir()?
        .join("locks")
        .join(carrier)
        .join(format!("{repository}.lock"));
    std::fs::create_dir_all(path.parent().expect("a lock path has a parent"))?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;
    let Some(until) = until else {
        fs2::FileExt::lock_exclusive(&file)?;
        return Ok(file);
    };
    loop {
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => return Ok(file),
            Err(error) if error.kind() != std::io::ErrorKind::WouldBlock => {
                return Err(error.into())
            }
            Err(_) if Instant::now() >= until => {
                bail!("another exchange of this plan was still running")
            }
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{poll_owned, poll_owned_with};
    use loopflow_test_support::TestRepo;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn git(dir: &Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[tokio::test]
    async fn poll_owned_skips_local_only_repository_without_writing_scope_check() {
        let root = tempfile::tempdir().unwrap();
        let repo_dir = root.path().join("local-only");
        std::fs::create_dir_all(&repo_dir).unwrap();
        git(&repo_dir, &["init", "-q"]);
        let repo = repo_dir
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let sqlite =
            crate::store::opening::open_ephemeral(&root.path().join("loopflow.db")).unwrap();
        let store = crate::store::Store { sqlite };

        let spawns_before = crate::observe::thread_spawns();
        let attempted = poll_owned(&store, &repo, 30).await.unwrap();
        let spawns_after = crate::observe::thread_spawns();

        assert_eq!(attempted, None);
        assert_eq!(
            spawns_after - spawns_before,
            0,
            "local-only poll_owned must spawn nothing"
        );
        assert_eq!(
            store
                .sqlite
                .last_scope_attempt(&format!("plan:{repo}"))
                .unwrap(),
            None,
            "local-only poll_owned must not write planning_scope_checks"
        );

        // Missing directory also writes nothing.
        let missing = root
            .path()
            .join("deleted-repo")
            .to_string_lossy()
            .into_owned();
        assert_eq!(poll_owned(&store, &missing, 30).await.unwrap(), None);
        assert_eq!(
            store
                .sqlite
                .last_scope_attempt(&format!("plan:{missing}"))
                .unwrap(),
            None
        );

        // Adding a remote is observed immediately on the next poll.
        let origin = TestRepo::new();
        git(
            &repo_dir,
            &[
                "remote",
                "add",
                "origin",
                origin.bare_path().to_str().unwrap(),
            ],
        );
        let attempted = poll_owned(&store, &repo, 30).await.unwrap();
        assert!(attempted.is_some());
        assert_eq!(
            store
                .sqlite
                .last_scope_attempt(&format!("plan:{repo}"))
                .unwrap(),
            attempted
        );
    }

    #[tokio::test]
    async fn poll_owned_backs_off_unreachable_git_without_blocking_hub() {
        let root = tempfile::tempdir().unwrap();
        let origin = TestRepo::new();
        let clone = root.path().join("clone");
        git(
            root.path(),
            &[
                "clone",
                "-q",
                origin.bare_path().to_str().unwrap(),
                clone.to_str().unwrap(),
            ],
        );
        let repo = clone.canonicalize().unwrap().to_string_lossy().into_owned();
        let sqlite =
            crate::store::opening::open_ephemeral(&root.path().join("loopflow.db")).unwrap();
        let store = crate::store::Store { sqlite };

        // Make the remote unreachable.
        let away = origin.bare_path().with_extension("away");
        std::fs::rename(origin.bare_path(), &away).unwrap();

        let hub_calls = Arc::new(AtomicUsize::new(0));
        let git_calls = Arc::new(AtomicUsize::new(0));
        let t0 = UNIX_EPOCH + Duration::from_secs(1_000);

        let run_once = |now: SystemTime, has_hub: bool| {
            let store_ref = &store;
            let repo_str = repo.clone();
            let hub_calls = Arc::clone(&hub_calls);
            let git_calls = Arc::clone(&git_calls);
            async move {
                poll_owned_with(
                    store_ref,
                    &repo_str,
                    0,
                    now,
                    has_hub,
                    || {
                        hub_calls.fetch_add(1, Ordering::SeqCst);
                        async { Ok(()) }
                    },
                    |until| {
                        git_calls.fetch_add(1, Ordering::SeqCst);
                        let sqlite = store_ref.sqlite.clone();
                        let repo_inner = repo_str.clone();
                        async move {
                            super::super::plan_git::exchange_background_at(
                                &sqlite,
                                &repo_inner,
                                Some(until),
                                now,
                            )
                        }
                    },
                )
                .await
            }
        };

        // Tick 1: both hub and Git run; Git fails -> 2-minute backoff.
        assert!(run_once(t0, true).await.unwrap().is_some());
        assert_eq!(hub_calls.load(Ordering::SeqCst), 1);
        assert_eq!(git_calls.load(Ordering::SeqCst), 1);

        // Tick 2 (+30 s): Git is backed off, but hub still runs.
        assert!(run_once(t0 + Duration::from_secs(30), true)
            .await
            .unwrap()
            .is_some());
        assert_eq!(hub_calls.load(Ordering::SeqCst), 2);
        assert_eq!(git_calls.load(Ordering::SeqCst), 1);

        // Tick 3 (+60 s, no hub): Git backed off and no hub -> skipped completely,
        // no planning_scope_checks update.
        assert_eq!(
            run_once(t0 + Duration::from_secs(60), false).await.unwrap(),
            None
        );
        assert_eq!(
            store
                .sqlite
                .last_scope_attempt(&format!("plan:{repo}"))
                .unwrap(),
            Some(1_030)
        );
        assert_eq!(git_calls.load(Ordering::SeqCst), 1);

        // Tick 4 (+120 s): 2-minute backoff elapsed -> Git retries and fails -> 4-minute backoff.
        let t1 = t0 + Duration::from_secs(120);
        assert!(run_once(t1, true).await.unwrap().is_some());
        assert_eq!(hub_calls.load(Ordering::SeqCst), 3);
        assert_eq!(git_calls.load(Ordering::SeqCst), 2);

        // Tick 5 (+239 s after t1): still inside 4-minute backoff; hub still runs.
        assert!(run_once(t1 + Duration::from_secs(239), true)
            .await
            .unwrap()
            .is_some());
        assert_eq!(hub_calls.load(Ordering::SeqCst), 4);
        assert_eq!(git_calls.load(Ordering::SeqCst), 2);

        // Restoring the remote and running an explicit exchange bypasses and clears backoff.
        std::fs::rename(&away, origin.bare_path()).unwrap();
        super::super::plan_git::exchange(&store.sqlite, &repo, None).unwrap();

        // Next background poll even 1 s later sees backoff cleared and runs Git.
        assert!(
            run_once(t1 + Duration::from_secs(240) - Duration::from_secs(1), true)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(hub_calls.load(Ordering::SeqCst), 5);
        assert_eq!(git_calls.load(Ordering::SeqCst), 3);
    }
}
