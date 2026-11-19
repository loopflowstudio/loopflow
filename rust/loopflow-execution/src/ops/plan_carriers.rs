//! A plan without Linear or Asana travels by a hub ([`super::plan_hub`]), a
//! Git remote ([`super::plan_git`]) or both. This is when the two exchange
//! together: when asked, before a claim, and after a command the Machine
//! server answers.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;

use loopflow_model::durable::{RepositoryId, TaskId};
use loopflow_storage::sqlite::SqliteStore;

/// One exchange for `repo` by each carrier it has: the hub first, since it
/// answers in less time than a fetch and what it gives goes out by Git in the
/// same turn. Git runs whether or not the hub answered; the first failure is
/// returned.
fn exchange(store: &SqliteStore, repo: &str) -> Result<()> {
    let by_hub = super::plan_hub::hub(store).and_then(|hub| match hub {
        Some(hub) => super::plan_hub::exchange(store, repo, &hub, false),
        None => Ok(()),
    });
    super::plan_git::exchange(store, repo).and(by_hub)
}

/// [`exchange`] for `repo`, when it has no Linear or Asana.
pub async fn sync(store: &crate::store::Store, repo: &str) -> Result<()> {
    if crate::ops::tracker::connected(repo) {
        return Ok(());
    }
    let (sqlite, repo) = (store.sqlite.clone(), repo.to_owned());
    crate::journal::request::spawn_blocking(move || exchange(&sqlite, &repo)).await?
}

/// How long a claim waits to see the claims other Machines have made.
const BEFORE_CLAIM: Duration = Duration::from_secs(5);

/// Before this Machine claims `task`, which nobody owns as far as it has
/// seen: one exchange by each carrier, so a claim already made elsewhere is
/// read first. A carrier that fails or outlasts [`BEFORE_CLAIM`] leaves the
/// claim to go ahead; whichever claim was first holds once they meet.
pub(crate) async fn before_claim(store: &crate::store::Store, repo: &Path, task: &TaskId) {
    if !matches!(store.sqlite.task_owned(task), Ok(false)) {
        return;
    }
    let repo = repo.to_string_lossy();
    match tokio::time::timeout(BEFORE_CLAIM, sync(store, &repo)).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            tracing::debug!(%task, error = %format!("{error:#}"), "claiming without the plan exchange");
        }
        Err(_) => tracing::debug!(%task, "claiming before the plan exchange finished"),
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
    let repo = crate::repository::discover(directory)?.to_string();
    let path = crate::store::default_db_path();
    if crate::ops::tracker::connected(&repo) || !path.exists() {
        return Ok(());
    }
    let store = crate::store::open_sqlite(&path)?;
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
        let result = exchange(&store, &repo);
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
/// time, across processes; the next reads what this one wrote.
pub(super) fn lock(
    store: &SqliteStore,
    carrier: &str,
    repository: &RepositoryId,
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
    fs2::FileExt::lock_exclusive(&file)?;
    Ok(file)
}
