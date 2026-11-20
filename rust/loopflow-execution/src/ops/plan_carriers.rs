//! A plan without Linear or Asana travels by a hub ([`super::plan_hub`]), a
//! Git remote ([`super::plan_git`]) or both. This is when the two run
//! together. [`send`] follows a save and reads only what was not taken.
//! [`sync`] reads and sends: when asked, before a claim or a new Wave, and
//! after a command the Machine server answers. [`read`] asks the hub alone,
//! for a read nobody asked for.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};

use loopflow_model::durable::{RepositoryId, TaskId};
use loopflow_storage::sqlite::plan_carriers::NotTaken;
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
/// the records past its cursor and reads only while something it brought was
/// not taken, the two at once and for
/// [`WAIT`] at most. A carrier that fails or runs out of time keeps its
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

/// A clone this Machine has not met reads its remote's plan when first
/// asked what the plan is (`lf task list`, `lf repo tracker`), before the
/// command opens the store it reads. See [`super::plan_git::first_exchange`].
pub async fn before_first_read(directory: &Path) {
    let Ok(Some((store, repo))) = carried(directory) else {
        return;
    };
    let until = Instant::now() + WAIT;
    let exchanged = {
        let repo = repo.clone();
        crate::journal::request::spawn_blocking(move || {
            super::plan_git::first_exchange(&store, &repo, until).map(drop)
        })
    };
    went_on(
        &repo,
        exchanged.await.unwrap_or_else(|error| Err(error.into())),
    );
}

/// The store and repository of `directory`, when this Machine has a store
/// and the repository has no Linear or Asana.
fn carried(directory: &Path) -> Result<Option<(SqliteStore, String)>> {
    let repo = crate::repository::discover(directory)?.to_string();
    let path = crate::store::default_db_path();
    if !path.exists() || crate::ops::tracker::connected(&repo) {
        return Ok(None);
    }
    Ok(Some((crate::store::open_sqlite(&path)?, repo)))
}

/// A command that read the plan first goes on without it.
fn went_on(repo: &str, exchanged: Result<()>) {
    if let Err(error) = exchanged {
        tracing::debug!(%repo, error = %format!("{error:#}"), "going on without the plan exchange");
    }
}

/// After a command on this Machine's server answers: one exchange for its
/// repository, on a thread of its own. Commands arriving while one runs, or
/// within [`QUIET`] of the last, share it.
pub fn after_command(directory: PathBuf) {
    std::thread::spawn(move || {
        if let Err(error) = exchange_after_command(&directory) {
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

fn exchange_after_command(directory: &Path) -> Result<()> {
    let Some((store, repo)) = carried(directory)? else {
        return Ok(());
    };
    {
        let mut turns = TURNS.lock().expect("plan turns are never poisoned");
        let turn = turns.entry(repo.clone()).or_default();
        if turn.running {
            turn.again = true;
            return Ok(());
        }
        if turn.finished.is_some_and(|at| at.elapsed() < QUIET) {
            return Ok(());
        }
        turn.running = true;
    }
    loop {
        let result = exchange(&store, &repo, None);
        let mut turns = TURNS.lock().expect("plan turns are never poisoned");
        let turn = turns.entry(repo.clone()).or_default();
        if std::mem::take(&mut turn.again) && result.is_ok() {
            continue;
        }
        turn.running = false;
        turn.finished = Some(Instant::now());
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
