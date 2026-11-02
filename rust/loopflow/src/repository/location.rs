//! A Git common directory carries a locator for its existing planning identity.
//! Ordinary clones do not copy it; remotes and directory names never identify Work.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use crate::durable::{MachineId, RepositoryId};
use crate::repository::CanonicalRepo;
use crate::store::sqlite::SqliteStore;

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

/// Register or reconnect a repository. `relocate` is explicit recovery for a
/// directory moved before its locator was recorded.
pub fn reconcile(
    store: &SqliteStore,
    repo: &CanonicalRepo,
    bind: Option<&RepositoryId>,
    relocate: Option<&RepositoryId>,
) -> Result<RepositoryId> {
    let common = crate::git::worktrees::git_common_dir(repo.as_path())?;
    let machine = store.local_machine()?.id;
    let common = locator_directory(&common, &machine);
    fs::create_dir_all(&common)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(common.join("loopflow-repository.lock"))?;
    fs2::FileExt::lock_exclusive(&lock)?;
    let locator = read_locator(&common)?;
    let destination = repo.to_string();
    if locator.is_none()
        && bind.is_none()
        && relocate.is_none()
        && store.repository_id(&destination)?.is_some()
    {
        return Ok(store.ensure_repository(&destination)?);
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
                if let Ok(current) = CanonicalRepo::discover(Path::new(&source)) {
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
                            return Err(anyhow!("the old address {source} is occupied without a Git locator; for a verified move, run `lf repository --relocate {id}` from its new location"));
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
                return Err(anyhow!("Git locator {id} conflicts with retained repository {existing}; use `lf repository --bind {existing}` only after checking this checkout"));
            }
            store.bind_repository(&destination, id)?;
        }
    }
    if let Some(id) = bind {
        store.bind_repository(&destination, id)?;
    }
    let selected = match store.repository_id(&destination)? {
        Some(id) => id,
        None => store.ensure_repository(&destination)?,
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
    let common = locator_directory(&common, &store.sqlite.local_machine()?.id);
    let locator = read_locator(&common)?;
    let registered = locator
        .as_ref()
        .map(|id| store.sqlite.repository_path(id))
        .transpose()?
        .flatten();
    if registered.as_deref().map(Path::new) == root.as_deref() && registered.is_some() {
        return Ok(());
    }
    let Some(repo) = CanonicalRepo::current()? else {
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
