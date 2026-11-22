//! A Git common directory carries a locator for its existing planning identity.
//! Ordinary clones do not copy it; remotes and directory names never identify Work.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use crate::repository::CanonicalRepo;
use loopflow_model::durable::{MachineId, RepositoryId};
use loopflow_storage::sqlite::SqliteStore;

const LOCATOR: &str = "loopflow-repository";

pub(crate) fn locator_directory(common: &Path, machine: &MachineId) -> PathBuf {
    common.join("loopflow").join(machine.as_str())
}

pub(crate) fn read_locator(common: &Path) -> Result<Option<RepositoryId>> {
    match fs::read_to_string(common.join(LOCATOR)) {
        Ok(value) => Ok(Some(RepositoryId::parse(value.trim())?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("read repository locator"),
    }
}

pub(crate) fn write_locator(common: &Path, id: &RepositoryId) -> Result<()> {
    fs::create_dir_all(common)?;
    let mut file = tempfile::NamedTempFile::new_in(common)?;
    writeln!(file, "{id}")?;
    file.as_file().sync_all()?;
    file.persist(common.join(LOCATOR))?;
    Ok(())
}

fn invalid(error: impl std::fmt::Display) -> crate::store::StoreError {
    crate::store::StoreError::InvalidData(error.to_string())
}

fn git_locator(repo: &str, machine: &MachineId) -> crate::store::StoreResult<Option<PathBuf>> {
    let root = Path::new(repo);
    // Historical symlink spellings never replace the canonical Git locator.
    if root.canonicalize().ok().as_deref() != Some(root) {
        return Ok(None);
    }
    let common = root.join(".git");
    let common = if common.is_dir() {
        common
    } else if common.is_file() {
        let canonical = crate::repository::discover(root).map_err(invalid)?;
        if canonical.as_path() != root {
            return Ok(None);
        }
        crate::git::worktrees::git_common_dir(root).map_err(invalid)?
    } else {
        return Ok(None);
    };
    Ok(Some(locator_directory(&common, machine)))
}

fn retained_checkout_matches(
    repo: &str,
    retained: &[(String, String)],
) -> crate::store::StoreResult<bool> {
    if retained.is_empty() {
        return Ok(false);
    }
    let Ok(observed) = crate::git::worktrees::list_porcelain(Path::new(repo)) else {
        return Ok(false);
    };
    let canonical = crate::repository::discover(Path::new(repo)).map_err(invalid)?;
    Ok(retained.iter().any(|(path, branch)| {
        let Ok(path) = Path::new(path).canonicalize() else {
            return false;
        };
        path.join(".git").is_file()
            && crate::repository::discover(&path).is_ok_and(|other| other == canonical)
            && observed.iter().any(|(checkout, observed_branch)| {
                checkout.canonicalize().ok().as_ref() == Some(&path)
                    && observed_branch.as_deref() == Some(branch.as_str())
            })
    }))
}

fn lock_locator(common: &Path) -> Result<fs::File> {
    fs::create_dir_all(common)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(common.join("loopflow-repository.lock"))?;
    fs2::FileExt::lock_exclusive(&lock)?;
    Ok(lock)
}

/// Discover and publish the Git locator outside SQL, then conditionally register it.
/// A locator survives a failed commit so retry retains the same identity.
pub fn ensure_repository(
    store: &SqliteStore,
    repo: &str,
) -> crate::store::StoreResult<RepositoryId> {
    register(store, repo, None, false)
}

/// Explicitly select a repository identity, retaining prior IDs as local locators.
pub fn bind_repository(
    store: &SqliteStore,
    repo: &str,
    id: &RepositoryId,
) -> crate::store::StoreResult<()> {
    register(store, repo, Some(id), false).map(|_| ())
}

#[derive(Debug)]
pub(crate) struct PreparedRegistration {
    pub inputs: loopflow_storage::sqlite::repositories::RepositoryRegistration,
    pub id: RepositoryId,
    _lock: Option<fs::File>,
}

fn register(
    store: &SqliteStore,
    repo: &str,
    bind: Option<&RepositoryId>,
    locked: bool,
) -> crate::store::StoreResult<RepositoryId> {
    loop {
        let prepared = prepare_registration(store, repo, bind, locked)?;
        if store.register_repository(repo, &prepared.inputs, &prepared.id, bind.is_some())? {
            return Ok(prepared.id);
        }
    }
}

/// Acquire the locator without committing a row: Wave creation keeps registration
/// in its own transaction. The locator and its lock survive until that write ends.
pub(crate) fn prepare_registration(
    store: &SqliteStore,
    repo: &str,
    bind: Option<&RepositoryId>,
    locked: bool,
) -> crate::store::StoreResult<PreparedRegistration> {
    loop {
        let inputs = store.repository_registration(repo)?;
        let common = git_locator(repo, &inputs.machine)?;
        let _lock = common
            .as_deref()
            .filter(|_| !locked)
            .map(lock_locator)
            .transpose()
            .map_err(invalid)?;
        if store.repository_registration(repo)? != inputs {
            continue;
        }
        let locator = common
            .as_deref()
            .map(read_locator)
            .transpose()
            .map_err(invalid)?
            .flatten();
        let id = match bind {
            Some(id) => id.clone(),
            None => {
                match &inputs.selected {
                    Some(id) => {
                        if common.is_some()
                            && locator.is_none()
                            && !retained_checkout_matches(repo, &inputs.checkouts)?
                        {
                            return Err(invalid(format!(
                            "{repo} retains repository {id} but has no Git locator; establish the original checkout with `lf config repo --bind {id}`, or recover the moved checkout with `lf config repo --relocate {id}`"
                        )));
                        }
                        if locator
                            .as_ref()
                            .is_some_and(|id| !inputs.identities.contains(id))
                        {
                            return Err(invalid("Git locator conflicts with retained Work; reconcile with `lf repo`"));
                        }
                        id.clone()
                    }
                    None => locator.clone().unwrap_or_else(RepositoryId::new),
                }
            }
        };
        // Check a locator registered at another address before publishing anything.
        if store.repository_path(&id)?.is_some_and(|path| path != repo) {
            return Err(invalid(format!("repository identity {id} already locates another local checkout; reconcile with `lf config repo`")));
        }
        if let Some(common) = &common {
            // A selected identity can retain an older, valid local locator.
            if bind.is_some() || locator.is_none() {
                write_locator(common, &id).map_err(invalid)?;
            }
        }
        return Ok(PreparedRegistration { inputs, id, _lock });
    }
}

/// Register or reconnect a repository. `relocate` is explicit recovery for a
/// directory moved before its locator was recorded.
pub fn reconcile(
    store: &SqliteStore,
    repo: &CanonicalRepo,
    bind: Option<&RepositoryId>,
    relocate: Option<&RepositoryId>,
) -> Result<RepositoryId> {
    let common = crate::git::worktrees::git_common_dir(repo.as_path())?;
    let machine = crate::store::machines::local_machine(store)?.id;
    let common = locator_directory(&common, &machine);
    let _lock = lock_locator(&common)?;
    let locator = read_locator(&common)?;
    let destination = repo.to_string();
    if locator.is_none()
        && bind.is_none()
        && relocate.is_none()
        && store.repository_id(&destination)?.is_some()
    {
        return Ok(register(store, &destination, None, true)?);
    }
    if let Some(id) = relocate.or(locator.as_ref()).filter(|_| bind.is_none()) {
        if relocate.is_some() && locator.as_ref().is_some_and(|other| other != id) {
            return Err(anyhow!(
                "this Git directory already retains {}; no Work was moved",
                locator.expect("checked locator")
            ));
        }
        if let Some(source) = store.repository_path(id)? {
            if source != destination {
                if let Ok(current) = crate::repository::discover(Path::new(&source)) {
                    if current != *repo {
                        let source_locator =
                            crate::git::worktrees::git_common_dir(current.as_path())
                                .ok()
                                .map(|common| read_locator(&locator_directory(&common, &machine)))
                                .transpose()?
                                .flatten();
                        if source_locator.as_ref() == Some(id) {
                            return Err(anyhow!("repository {id} still locates the Git directory at {source}; a copy cannot replace its retained Work"));
                        }
                        if source_locator.is_none() && relocate.is_none() {
                            return Err(anyhow!("the old address {source} is occupied without a Git locator; for a verified move, run `lf config repo --relocate {id}` from its new location"));
                        }
                    }
                }
                // Repair Git first. If SQLite cannot commit, retry repeats this
                // idempotent operation against the same common directory.
                let nested = crate::git::worktrees::list_porcelain(repo.as_path())?
                    .into_iter()
                    .filter_map(|(path, _)| {
                        path.strip_prefix(&source)
                            .ok()
                            .map(|relative| repo.as_path().join(relative))
                    })
                    .filter(|path| path != repo.as_path() && path.join(".git").is_file())
                    .collect::<Vec<_>>();
                let repaired = Command::new("git")
                    .current_dir(repo.as_path())
                    .args(["worktree", "repair"])
                    .args(nested)
                    .output()?;
                anyhow::ensure!(
                    repaired.status.success(),
                    "repair moved repository worktrees: {}",
                    String::from_utf8_lossy(&repaired.stderr)
                );
                store.relocate_repository(id, &destination)?;
            }
        } else if relocate.is_some() {
            return Err(anyhow!(
                "repository {id} has no retained local Work to relocate"
            ));
        } else {
            if let Some(existing) = store.repository_id(&destination)? {
                return Err(anyhow!("Git locator {id} conflicts with retained repository {existing}; use `lf config repo --bind {existing}` only after checking this checkout"));
            }
            register(store, &destination, Some(id), true)?;
        }
    }
    if let Some(id) = bind {
        register(store, &destination, Some(id), true)?;
    }
    let selected = match store.repository_id(&destination)? {
        Some(id) => id,
        None => register(store, &destination, None, true)?,
    };
    if locator.as_ref() != Some(&selected) {
        write_locator(&common, &selected)?;
    }
    Ok(selected)
}

/// Ordinary reads reconnect enrolled directories and enroll retained identities
/// witnessed by linked Task checkouts. They never invent an identity.
pub fn reconnect_current() -> Result<()> {
    // Most commands are unrelated to relocation. Inspect only the enclosing
    // Git locator before adding any Git subprocesses or database reads.
    let cwd = std::env::current_dir()?;
    let Some(git) = cwd
        .ancestors()
        .map(|path| path.join(".git"))
        .find(|path| path.exists())
    else {
        return Ok(());
    };
    let common = if git.is_dir() {
        git
    } else {
        let text = fs::read_to_string(&git)?;
        let Some(directory) = text.trim().strip_prefix("gitdir: ") else {
            return Ok(());
        };
        let directory = git.parent().expect("Git file has parent").join(directory);
        match fs::read_to_string(directory.join("commondir")) {
            Ok(relative) => directory.join(relative.trim()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => directory,
            Err(error) => return Err(error).context("read Git common directory"),
        }
    };
    let Some(store) = crate::store::read_existing_registry()? else {
        return Ok(());
    };
    let root = common.canonicalize()?.parent().map(Path::to_path_buf);
    let common = locator_directory(
        &common,
        &crate::store::machines::local_machine(&store.sqlite)?.id,
    );
    let locator = read_locator(&common)?;
    let registered = locator
        .as_ref()
        .map(|id| store.sqlite.repository_path(id))
        .transpose()?
        .flatten();
    if registered.as_deref().map(Path::new) == root.as_deref() && registered.is_some() {
        return Ok(());
    }
    let Some(repo) = crate::repository::current()? else {
        return Ok(());
    };
    let needs_enrollment =
        locator.is_none() && store.sqlite.repository_id(&repo.to_string())?.is_some();
    if needs_enrollment || registered.is_some_and(|path| path != repo.to_string()) {
        let runtime = tokio::runtime::Runtime::new()?;
        let store = runtime.block_on(crate::store::open_store(
            &crate::store::storage_config_from_env()?,
        ))?;
        reconcile(&store.sqlite, &repo, None, None)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{bind_repository, ensure_repository, git_locator, read_locator};

    use loopflow_model::durable::RepositoryId;
    use loopflow_test_support::TestRepo;

    #[test]
    fn registration_retry_reuses_the_locator_published_before_a_failed_commit() {
        let home = tempfile::tempdir().unwrap();
        let repo = TestRepo::new();
        let path = repo.path().canonicalize().unwrap();
        let path = path.to_str().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TEMP TRIGGER fail_registration BEFORE INSERT ON repository_plans
             BEGIN SELECT RAISE(ABORT,'registration failed'); END;",
            )
            .unwrap();
        assert!(ensure_repository(&store, path).is_err());
        assert!(store.repository_id(path).unwrap().is_none());
        let directory = git_locator(path, &store.repository_registration(path).unwrap().machine)
            .unwrap()
            .unwrap();
        let original = read_locator(&directory).unwrap().unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_registration")
            .unwrap();
        assert_eq!(ensure_repository(&store, path).unwrap(), original);
        assert_eq!(read_locator(&directory).unwrap(), Some(original));
    }

    #[test]
    fn failed_wave_creation_keeps_registration_in_the_wave_transaction() {
        use crate::work::wave::Wave;
        use loopflow_model::id::WaveId;

        let home = tempfile::tempdir().unwrap();
        let repo = TestRepo::new();
        let path = repo.path().canonicalize().unwrap();
        let path = path.to_str().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let invalid = Wave::new(WaveId::new(), "invalid/name".into(), path.into());
        assert!(crate::work::wave::bootstrap::upsert_wave(&store, &invalid).is_err());
        assert!(store.repository_id(path).unwrap().is_none());
        let directory = git_locator(path, &store.repository_registration(path).unwrap().machine)
            .unwrap()
            .unwrap();
        let original = read_locator(&directory).unwrap().unwrap();
        crate::work::wave::bootstrap::upsert_wave(
            &store,
            &Wave::new(WaveId::new(), "valid".into(), path.into()),
        )
        .unwrap();
        assert_eq!(store.repository_id(path).unwrap(), Some(original));
    }

    #[test]
    fn changed_registration_inputs_cannot_replace_a_later_binding() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let inputs = store.repository_registration("/repo").unwrap();
        let later = RepositoryId::new();
        bind_repository(&store, "/repo", &later).unwrap();
        assert!(!store
            .register_repository("/repo", &inputs, &RepositoryId::new(), false)
            .unwrap());
        assert_eq!(ensure_repository(&store, "/repo").unwrap(), later);
    }

    #[test]
    fn registration_uses_indexed_checkout_evidence_and_an_unchanged_identity_only_reads() {
        use loopflow_storage::query_plans::{assert_no_empty_writes, growing_scans, statements};

        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let id = ensure_repository(&store, "/repo").unwrap();
        let history = ensure_repository(&store, "/history").unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(&format!(
                "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<10000)
             INSERT INTO tasks(id,repository_id,issue_identifier,created_at)
             SELECT printf('task_%032x',x),'{history}',printf('H-%d',x),1 FROM n;
             INSERT INTO task_checkouts(task_id,worktree,branch)
             SELECT id,'/history/' || id,'main' FROM tasks;"
            ))
            .unwrap();
        let traced = statements(&store, || {
            assert_no_empty_writes(&store, || {
                assert_eq!(ensure_repository(&store, "/repo").unwrap(), id);
            });
        });
        let scans = growing_scans(&store, &traced);
        assert!(scans.is_empty(), "{}", scans.join("\n"));
    }

    #[test]
    fn concurrent_registration_retains_one_locator_and_one_identity() {
        let home = tempfile::tempdir().unwrap();
        let repo = TestRepo::new();
        let path = repo.path().canonicalize().unwrap();
        let path = path.to_str().unwrap();
        let first = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let second = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let start = std::sync::Barrier::new(2);
        let ids = std::thread::scope(|scope| {
            let other = scope.spawn(|| {
                start.wait();
                ensure_repository(&second, path).unwrap()
            });
            start.wait();
            let id = ensure_repository(&first, path).unwrap();
            (id, other.join().unwrap())
        });
        assert_eq!(ids.0, ids.1);
        let directory = git_locator(path, &first.repository_registration(path).unwrap().machine)
            .unwrap()
            .unwrap();
        assert_eq!(read_locator(&directory).unwrap(), Some(ids.0));
    }
}
