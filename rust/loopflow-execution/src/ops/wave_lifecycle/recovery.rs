//! Retained intent for non-transactional provider and authored-file effects.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use crate::work::wave::lifecycle::{FileChange, Identity, Image, Operation};

use super::{authored_path, lifecycle_error, resolve_wave, AuthoredFiles};
use crate::ops::OpsResult;
use crate::store::Store;

pub fn save(operation: &Operation, store: &Store) -> OpsResult<()> {
    store
        .sqlite
        .save_wave_operation(operation)
        .map_err(lifecycle_error)
}

/// Commit saved content, not whatever happens to occupy these paths on retry.
pub(super) fn finish_files(operation: &mut Operation, store: &Store, main: &Path) -> AuthoredFiles {
    if operation.files_done {
        return AuthoredFiles::Unchanged;
    }
    match apply_and_commit(operation, store, main) {
        Ok(files) => files,
        Err(error) => AuthoredFiles::Uncommitted {
            reason: error.to_string(),
        },
    }
}

fn apply_and_commit(
    operation: &mut Operation,
    store: &Store,
    main: &Path,
) -> OpsResult<AuthoredFiles> {
    if operation.source_repo != operation.target_repo {
        return Err(lifecycle_error(format!(
            "move .lf/wave/{} into the target repository; files do not cross repositories",
            operation.from
        )));
    }
    let mut replaced_roots = Vec::new();
    for slug in [&operation.from, &operation.to] {
        if !slug.is_empty()
            && resolve_wave(store, main, slug)?
                .is_some_and(|wave| wave.id() != &operation.waves[0].id)
        {
            replaced_roots.push(authored_path(slug) + "/");
        }
    }
    let mut selected = Vec::new();
    for change in &operation.changes {
        if replaced_roots
            .iter()
            .any(|root| change.path.starts_with(root))
            || is_replaced(change, store, main)?
        {
            continue;
        }
        selected.push(change);
    }
    if !operation.applied {
        for change in &selected {
            let current = read_image(&main.join(&change.path))?;
            if current == change.after {
                continue;
            }
            if current != change.before {
                return Err(lifecycle_error(format!(
                    "{} changed after this operation began; its pending change is retained",
                    change.path
                )));
            }
            write_image(change.after.as_ref(), &main.join(&change.path))?;
        }
        // The last write may have preceded an interruption. The images above
        // recognize it; recording this before committing preserves subsequent edits.
        operation.applied = true;
        save(operation, store)?;
    }
    let _mutation = crate::ops::task::lock_task_pr_mutation(main)?;
    let mut pending = Vec::new();
    for &change in &selected {
        let head = image_at_head(main, &change.path)?;
        if head == change.after {
            continue;
        }
        if head != change.base {
            return Err(lifecycle_error(format!(
                "{} has a later committed change; the lifecycle commit is still pending",
                change.path
            )));
        }
        pending.push(change);
    }
    let files = if pending.is_empty() {
        AuthoredFiles::Unchanged
    } else {
        commit_images(
            main,
            &pending,
            &format!("lf wave {}: {}", operation.kind, operation.from),
        )?
    };
    // A commit can succeed before index reconciliation fails. Retained
    // images still identify its residue even when HEAD needs no new commit.
    reconcile_index(main, &selected)?;
    let source_replaced = replaced_roots.contains(&(authored_path(&operation.from) + "/"));
    if (operation.kind == "remove" || operation.from != operation.to) && !source_replaced {
        let source = main.join(authored_path(&operation.from));
        prune_empty(&source)?;
        if source.exists() {
            return Err(lifecycle_error(format!(
                    "{} contains later files; saved changes are committed, but directory cleanup remains pending",
                    source.display()
                )));
        }
    }
    operation.files_done = true;
    save(operation, store)?;
    Ok(files)
}

fn lock_file(main: &Path) -> OpsResult<File> {
    let git = crate::git::git_stdout(main, &["rev-parse", "--absolute-git-dir"])
        .map_err(lifecycle_error)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(Path::new(git.trim()).join("lf-wave-lifecycle.lock"))?;
    Ok(file)
}

pub fn lock(main: &Path) -> OpsResult<File> {
    let file = lock_file(main)?;
    fs2::FileExt::lock_exclusive(&file)?;
    Ok(file)
}

/// Keep an existing Wave's address live through Task admission.
pub(crate) fn read_lock(main: &Path) -> OpsResult<File> {
    let file = lock_file(main)?;
    fs2::FileExt::lock_shared(&file)?;
    Ok(file)
}

fn read_image(path: &Path) -> OpsResult<Option<Image>> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let (bytes, mode) = if metadata.file_type().is_symlink() {
        (
            std::fs::read_link(path)?
                .as_os_str()
                .as_encoded_bytes()
                .to_vec(),
            0o120000,
        )
    } else {
        (
            std::fs::read(path)?,
            if metadata.permissions().mode() & 0o111 != 0 {
                0o100755
            } else {
                0o100644
            },
        )
    };
    Ok(Some(Image { bytes, mode }))
}

fn write_image(image: Option<&Image>, path: &Path) -> OpsResult<()> {
    if let Some(image) = image {
        let parent = path.parent().expect("authored file has a parent");
        std::fs::create_dir_all(parent)?;
        let temporary = tempfile::tempdir_in(parent)?;
        let prepared = temporary.path().join("file");
        if image.mode == 0o120000 {
            use std::os::unix::ffi::OsStrExt;
            std::os::unix::fs::symlink(std::ffi::OsStr::from_bytes(&image.bytes), &prepared)?;
        } else {
            std::fs::write(&prepared, &image.bytes)?;
            std::fs::set_permissions(
                &prepared,
                std::fs::Permissions::from_mode(image.mode & 0o777),
            )?;
        }
        std::fs::rename(prepared, path)?;
    } else {
        match std::fs::remove_file(path) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn image_at_head(main: &Path, path: &str) -> OpsResult<Option<Image>> {
    let entry = crate::git::git_stdout(
        main,
        &["ls-tree", "HEAD", "--", &format!(":(literal){path}")],
    )
    .map_err(lifecycle_error)?;
    if entry.is_empty() {
        return Ok(None);
    }
    let mut fields = entry.split_whitespace();
    let mode = fields.next().expect("tree entry has mode");
    let hash = fields.nth(1).expect("tree entry has object");
    image_from_object(main, mode, hash).map(Some)
}

fn image_in_index(main: &Path, path: &str) -> OpsResult<Option<Image>> {
    let entry = crate::git::git_stdout(
        main,
        &["ls-files", "--stage", "--", &format!(":(literal){path}")],
    )
    .map_err(lifecycle_error)?;
    if entry.is_empty() {
        return Ok(None);
    }
    let mut fields = entry.split_whitespace();
    let mode = fields.next().expect("index entry has mode");
    let hash = fields.next().expect("index entry has object");
    if fields.next() != Some("0") {
        return Err(lifecycle_error(format!(
            "{path} has an unresolved index conflict"
        )));
    }
    image_from_object(main, mode, hash).map(Some)
}

fn image_from_object(main: &Path, mode: &str, hash: &str) -> OpsResult<Image> {
    let blob = Command::new("git")
        .current_dir(main)
        .args(["cat-file", "blob", hash])
        .output()?;
    if !blob.status.success() {
        return Err(lifecycle_error(String::from_utf8_lossy(&blob.stderr)));
    }
    Ok(Image {
        bytes: blob.stdout,
        mode: u32::from_str_radix(mode, 8).map_err(lifecycle_error)?,
    })
}

fn is_replaced(change: &FileChange, store: &Store, main: &Path) -> OpsResult<bool> {
    let Some(owner) = &change.owner else {
        return Ok(false);
    };
    if resolve_wave(store, main, &owner.slug)?.is_some_and(|wave| wave.id() != &owner.id) {
        return Ok(true);
    }
    let config = crate::work::wave::config::try_read_wave_config(main, &owner.slug)
        .map_err(lifecycle_error)?;
    Ok(config
        .and_then(|config| config.id)
        .is_some_and(|id| id != owner.id))
}
/// Run the authored transformation on a private copy, retaining exact images
/// before any planning/provider/filesystem mutation can make them undiscoverable.
pub(super) fn prepare_files(
    main: &Path,
    owners: &[Identity],
    transform: impl FnOnce(&Path) -> OpsResult<()>,
) -> OpsResult<Vec<FileChange>> {
    let before = snapshot(main)?;
    let temporary = tempfile::tempdir()?;
    for (path, image) in &before {
        write_image(Some(image), &temporary.path().join(path))?;
    }
    transform(temporary.path())?;
    let after = snapshot(temporary.path())?;
    let paths: BTreeSet<_> = before.keys().chain(after.keys()).collect();
    paths
        .into_iter()
        .filter(|path| before.get(*path) != after.get(*path))
        .map(|path| {
            let owner = owners
                .iter()
                .filter(|owner| path.starts_with(&(authored_path(&owner.slug) + "/")))
                .max_by_key(|owner| owner.slug.len())
                .cloned();
            Ok(FileChange {
                base: image_at_head(main, path)?,
                index: image_in_index(main, path)?,
                before: before.get(path).cloned(),
                after: after.get(path).cloned(),
                path: path.clone(),
                owner,
            })
        })
        .collect()
}

fn snapshot(main: &Path) -> OpsResult<BTreeMap<String, Image>> {
    let mut images = BTreeMap::new();
    let root = main.join(".lf/wave");
    if !root.exists() {
        return Ok(images);
    }
    let mut pending = vec![root];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
            } else if let Some(image) = read_image(&entry.path())? {
                images.insert(
                    entry
                        .path()
                        .strip_prefix(main)
                        .expect("file under main")
                        .to_string_lossy()
                        .to_string(),
                    image,
                );
            }
        }
    }
    Ok(images)
}

fn prune_empty(path: &Path) -> OpsResult<()> {
    if !path.is_dir() || path.is_symlink() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            prune_empty(&entry.path())?;
        }
    }
    if std::fs::read_dir(path)?.next().is_none() {
        std::fs::remove_dir(path)?;
    }
    Ok(())
}

fn commit_images(main: &Path, changes: &[&FileChange], message: &str) -> OpsResult<AuthoredFiles> {
    if crate::git::git_stdout(main, &["symbolic-ref", "-q", "HEAD"]).is_err() {
        return Err(lifecycle_error(format!(
            "{} is not on a branch",
            main.display()
        )));
    }
    let temporary = tempfile::tempdir()?;
    let index = temporary.path().join("index");
    let worktree = temporary.path().join("files");
    std::fs::create_dir(&worktree)?;
    let inherit = |command: &mut Command| {
        command
            .env("GIT_INDEX_FILE", &index)
            .env("GIT_WORK_TREE", &worktree);
    };
    let run = |args: &[&str]| {
        crate::git::git_stdout_inheriting(main, args, &inherit).map_err(lifecycle_error)
    };
    run(&["read-tree", "HEAD"])?;
    for change in changes {
        write_image(change.after.as_ref(), &worktree.join(&change.path))?;
        if change.after.is_some() {
            run(&["add", "-f", "--", &format!(":(literal){}", change.path)])?;
        } else {
            run(&["update-index", "--force-remove", "--", &change.path])?;
        }
    }
    crate::git::commit(main, message, &inherit).map_err(lifecycle_error)?;
    let commit = crate::git::git_stdout(main, &["rev-parse", "HEAD"]).map_err(lifecycle_error)?;
    Ok(AuthoredFiles::Committed {
        commit: commit.trim().into(),
    })
}

fn reconcile_index(main: &Path, changes: &[&FileChange]) -> OpsResult<()> {
    let mut refresh = Vec::new();
    for change in changes {
        // Preserve a person's later staged edit as well as the working file.
        let staged = image_in_index(main, &change.path)?;
        if staged != change.after && staged == change.index {
            refresh.push(format!(":(literal){}", change.path));
        }
    }
    if !refresh.is_empty() {
        let mut args = vec!["reset", "-q", "HEAD", "--"];
        args.extend(refresh.iter().map(String::as_str));
        crate::git::git_stdout(main, &args).map_err(lifecycle_error)?;
    }
    Ok(())
}
