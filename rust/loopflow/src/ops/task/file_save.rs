use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{
    file_context, file_snapshot, git_output, task_error, validate_task_relative_path,
    TaskFileSnapshot, TaskFileState, TaskWorkspace, MAX_FILE_BYTES,
};
use crate::ops::error::OpsResult;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskFileRecovery {
    pub directory: String,
    pub changed: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskFileSave {
    pub file: TaskFileSnapshot,
    pub published: bool,
    pub message: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Receipt {
    path: String,
    expected_revision: String,
    draft_revision: String,
}

pub fn task_save(
    issue: &str,
    path: &str,
    revision: &str,
    content: &str,
) -> OpsResult<TaskFileSave> {
    let (task, pr) = file_context(issue)?;
    save(TaskWorkspace::new(&task, &pr), path, revision, content)
}

pub(super) fn recovery_directory(workspace: TaskWorkspace<'_>) -> OpsResult<PathBuf> {
    let git_dir = git_output(workspace.worktree, &["rev-parse", "--absolute-git-dir"])?;
    Ok(Path::new(git_dir.trim()).join("loopflow-file-recovery"))
}

fn recovery_root(workspace: TaskWorkspace<'_>, path: &str) -> OpsResult<PathBuf> {
    Ok(recovery_directory(workspace)?.join(hex::encode(Sha256::digest(path.as_bytes()))))
}

pub(super) fn recoveries(
    workspace: TaskWorkspace<'_>,
    path: &str,
) -> OpsResult<Vec<TaskFileRecovery>> {
    let root = recovery_root(workspace, path)?;
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut results = Vec::new();
    for entry in entries {
        let entry = entry?;
        // Finder leaves `.DS_Store` beside saves; only directories are saves.
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let directory = entry.path();
        results.push(inspect_recovery(&directory, path));
    }
    results.sort_by(|a, b| b.directory.cmp(&a.directory));
    Ok(results)
}

fn inspect_recovery(directory: &Path, path: &str) -> TaskFileRecovery {
    let receipt = fs::read(directory.join("receipt.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Receipt>(&bytes).ok());
    let (changed, message) = match receipt {
        Some(receipt) if receipt.path == path => match revision_at(&directory.join("displaced")) {
            Ok(revision) if revision == receipt.expected_revision => (
                false,
                "Previous file retained; late writes remain recoverable.",
            ),
            Ok(revision) if revision == receipt.draft_revision => (
                true,
                "Save may not have published. Inspect the retained draft and file.",
            ),
            _ => (
                true,
                "Recovered file changed or has a different kind. Inspect before continuing.",
            ),
        },
        _ => (
            true,
            "Incomplete save. Inspect retained files before continuing.",
        ),
    };
    TaskFileRecovery {
        directory: directory.to_string_lossy().into_owned(),
        changed,
        message: message.into(),
    }
}

fn revision_at(path: &Path) -> OpsResult<String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    revision_of(file)
}

fn revision_of(file: File) -> OpsResult<String> {
    if !file.metadata()?.is_file() {
        return Err(task_error("Expected a regular file"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(task_error("File exceeds 1 MB"));
    }
    Ok(hex::encode(Sha256::digest(bytes)))
}

// Pin each directory before opening the next component. A symlink retarget cannot
// redirect the save outside this checkout. A concurrently moved directory remains
// the opened directory; the readback reports the current named path separately.
fn parent_directory(root: &Path, relative: &str) -> OpsResult<(File, CString)> {
    let mut directory = File::open(root.canonicalize()?)?;
    let path = Path::new(relative);
    for component in path
        .parent()
        .expect("relative file has a parent")
        .components()
    {
        let name = CString::new(component.as_os_str().as_encoded_bytes())
            .map_err(|_| task_error("Invalid file path"))?;
        // SAFETY: directory and name are live; openat returns a fresh owned descriptor.
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: successful openat returned a fresh descriptor owned by this File.
        directory = unsafe { File::from_raw_fd(fd) };
    }
    let name = CString::new(
        path.file_name()
            .expect("validated file name")
            .as_encoded_bytes(),
    )
    .map_err(|_| task_error("Invalid file path"))?;
    Ok((directory, name))
}

fn save(
    workspace: TaskWorkspace<'_>,
    path: &str,
    revision: &str,
    content: &str,
) -> OpsResult<TaskFileSave> {
    let path = validate_task_relative_path(path)?;
    if Path::new(&path)
        .components()
        .any(|part| part.as_os_str() == ".git")
    {
        return Err(task_error("Git metadata is not an editable Task file"));
    }
    if content.len() > MAX_FILE_BYTES || content.contains('\0') {
        return Err(task_error("Save requires complete UTF-8 text at most 1 MB"));
    }
    let current = file_snapshot(workspace, &path)?;
    if current.state != TaskFileState::Text || current.revision.as_deref() != Some(revision) {
        return Ok(TaskFileSave {
            file: current,
            published: false,
            message: "File changed on disk. Draft retained; reload or copy it before saving again."
                .into(),
        });
    }
    let (parent, name) = parent_directory(workspace.worktree, &path)?;
    // SAFETY: parent and name remain alive and openat creates an owned descriptor.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: fd was returned by successful openat and has no other owner.
    let original = unsafe { File::from_raw_fd(fd) };
    let permissions = original.metadata()?.permissions();
    if revision_of(original)? != revision {
        return Ok(TaskFileSave {
            file: file_snapshot(workspace, &path)?,
            published: false,
            message: "File changed before Save. Draft retained.".into(),
        });
    }

    let root = recovery_root(workspace, &path)?;
    fs::create_dir_all(&root)?;
    let directory = root.join(format!(
        "{}-{}",
        time::OffsetDateTime::now_utc().unix_timestamp_nanos(),
        uuid::Uuid::new_v4()
    ));
    fs::create_dir(&directory)?;
    let receipt = Receipt {
        path: path.clone(),
        expected_revision: revision.into(),
        draft_revision: hex::encode(Sha256::digest(content.as_bytes())),
    };
    // Persist intent and an independent draft before exchange. Never unlink the
    // displaced inode: an arbitrary writer can still hold an open descriptor.
    write_new(
        &directory.join("receipt.json"),
        &serde_json::to_vec_pretty(&receipt).map_err(|error| task_error(error.to_string()))?,
    )?;
    write_new(&directory.join("draft"), content.as_bytes())?;
    let displaced = directory.join("displaced");
    write_new(&displaced, content.as_bytes())?;
    fs::set_permissions(&displaced, permissions)?;
    File::open(&displaced)?.sync_all()?;
    let recovery = File::open(&directory)?;
    recovery.sync_all()?;
    File::open(&root)?.sync_all()?;
    #[cfg(test)]
    BEFORE_EXCHANGE.with(|hook| {
        if let Some(hook) = hook.borrow_mut().take() {
            hook();
        }
    });
    if let Err(error) = exchange(&parent, &name, &recovery) {
        return Err(task_error(format!(
            "Save failed: {error}. Draft and recovery retained at {}",
            directory.display()
        )));
    }
    // Any failure after exchange must leave a discoverable receipt and both files.
    let after_exchange_error = |error| {
        task_error(format!(
        "Save exchanged files but could not confirm completion: {error}. Inspect recovery at {}", directory.display()))
    };
    parent.sync_all().map_err(after_exchange_error)?;
    recovery.sync_all().map_err(after_exchange_error)?;
    // Saving inspects only this exchange, independent of retained history size.
    let retained = inspect_recovery(&directory, &path);
    let mut file = file_snapshot(workspace, &path).map_err(|error| {
        task_error(format!(
            "Save exchanged files but readback failed: {error}. Inspect recovery at {}",
            directory.display()
        ))
    })?;
    let published = file.revision.as_deref() == Some(&receipt.draft_revision);
    let message = if retained.changed {
        "Concurrent change retained in recovery. Inspect recovery; no automatic restore was attempted."
    } else if !published {
        "File changed again during Save. Submitted draft and previous file remain in recovery."
    } else {
        "Saved. Previous file retained in recovery."
    };
    file.recoveries.push(retained);
    Ok(TaskFileSave {
        file,
        published,
        message: message.into(),
    })
}

fn write_new(path: &Path, bytes: &[u8]) -> OpsResult<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn exchange(parent: &File, name: &CString, recovery: &File) -> std::io::Result<()> {
    // SAFETY: both directory descriptors and nul-terminated names are live.
    #[cfg(target_os = "macos")]
    let result = unsafe {
        libc::renameatx_np(
            parent.as_raw_fd(),
            name.as_ptr(),
            recovery.as_raw_fd(),
            c"displaced".as_ptr(),
            libc::RENAME_SWAP,
        )
    };
    // SAFETY: both directory descriptors and nul-terminated names are live.
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::renameat2(
            parent.as_raw_fd(),
            name.as_ptr(),
            recovery.as_raw_fd(),
            c"displaced".as_ptr(),
            libc::RENAME_EXCHANGE,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
thread_local! {
    static BEFORE_EXCHANGE: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::path::Path;

    use super::{recoveries, save, BEFORE_EXCHANGE};
    use crate::ops::task::{file_snapshot, git_output, TaskWorkspace};
    use crate::work::task::TaskId;

    #[test]
    fn task_files_save_retains_late_descriptor_writes_and_refuses_stale_revision() {
        let repo = tempfile::tempdir().unwrap();
        git_output(repo.path(), &["init", "-q"]).unwrap();
        let id = TaskId::new();
        let workspace = TaskWorkspace {
            issue_identifier: "TEST-1",
            task_id: &id,
            worktree: repo.path(),
            base_commit: "",
        };
        let path = repo.path().join("notes.txt");
        fs::write(&path, "\u{feff}notes\r\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let original = file_snapshot(workspace, "notes.txt").unwrap();
        let mut writer = OpenOptions::new().write(true).open(&path).unwrap();
        let saved = save(
            workspace,
            "notes.txt",
            original.revision.as_ref().unwrap(),
            "\u{feff}draft\r\n",
        )
        .unwrap();
        assert!(saved.published);
        assert_eq!(saved.file.content.as_deref(), Some("\u{feff}draft\r\n"));
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        let directory = Path::new(&saved.file.recoveries[0].directory);
        assert_eq!(
            fs::read_to_string(directory.join("displaced")).unwrap(),
            "\u{feff}notes\r\n"
        );
        writer.set_len(0).unwrap();
        writer.write_all(b"late writer\n").unwrap();
        writer.sync_all().unwrap();
        drop(writer);
        assert_eq!(
            fs::read_to_string(directory.join("displaced")).unwrap(),
            "late writer\n"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "\u{feff}draft\r\n");
        assert!(recoveries(workspace, "notes.txt").unwrap()[0].changed);
        fs::write(directory.parent().unwrap().join(".DS_Store"), "finder").unwrap();
        assert_eq!(recoveries(workspace, "notes.txt").unwrap().len(), 1);
        let refused = save(
            workspace,
            "notes.txt",
            original.revision.as_ref().unwrap(),
            "stale",
        )
        .unwrap();
        assert!(!refused.published);
        assert_eq!(fs::read_to_string(&path).unwrap(), "\u{feff}draft\r\n");
        assert_eq!(
            fs::read_to_string(directory.join("draft")).unwrap(),
            "\u{feff}draft\r\n"
        );
    }

    #[test]
    fn task_files_save_preserves_racing_replacements_deletion_rename_and_symlink() {
        for schedule in ["in_place", "atomic", "delete", "rename", "symlink"] {
            let repo = tempfile::tempdir().unwrap();
            git_output(repo.path(), &["init", "-q"]).unwrap();
            let id = TaskId::new();
            let workspace = TaskWorkspace {
                issue_identifier: "TEST-1",
                task_id: &id,
                worktree: repo.path(),
                base_commit: "",
            };
            let path = repo.path().join("notes.txt");
            fs::write(&path, "original").unwrap();
            let original = file_snapshot(workspace, "notes.txt").unwrap();
            let root = repo.path().to_owned();
            BEFORE_EXCHANGE.with(|hook| {
                *hook.borrow_mut() = Some(Box::new(move || {
                    let path = root.join("notes.txt");
                    match schedule {
                        "in_place" => fs::write(path, "external").unwrap(),
                        "atomic" => {
                            fs::write(root.join("new"), "external").unwrap();
                            fs::rename(root.join("new"), path).unwrap();
                        }
                        "delete" => fs::remove_file(path).unwrap(),
                        "rename" => fs::rename(path, root.join("moved")).unwrap(),
                        "symlink" => {
                            fs::write(root.join("destination"), "external").unwrap();
                            fs::remove_file(&path).unwrap();
                            symlink(root.join("destination"), path).unwrap();
                        }
                        _ => unreachable!(),
                    }
                }))
            });
            let result = save(
                workspace,
                "notes.txt",
                original.revision.as_ref().unwrap(),
                "draft",
            );
            let recovered = recoveries(workspace, "notes.txt").unwrap();
            assert_eq!(recovered.len(), 1);
            assert!(recovered[0].changed);
            let directory = Path::new(&recovered[0].directory);
            assert_eq!(
                fs::read_to_string(directory.join("draft")).unwrap(),
                "draft"
            );
            match schedule {
                "delete" | "rename" => {
                    assert!(result.is_err());
                    assert!(!path.exists());
                    if schedule == "rename" {
                        assert_eq!(
                            fs::read_to_string(repo.path().join("moved")).unwrap(),
                            "original"
                        );
                    }
                }
                "symlink" => {
                    assert!(result.unwrap().published);
                    assert!(fs::symlink_metadata(directory.join("displaced"))
                        .unwrap()
                        .is_symlink());
                    assert_eq!(
                        fs::read_to_string(repo.path().join("destination")).unwrap(),
                        "external"
                    );
                }
                _ => {
                    assert!(result.unwrap().published);
                    assert_eq!(
                        fs::read_to_string(directory.join("displaced")).unwrap(),
                        "external"
                    );
                }
            }
        }
    }

    #[test]
    fn task_files_save_rejects_symlink_parents_and_nontext_without_writing() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        git_output(repo.path(), &["init", "-q"]).unwrap();
        let id = TaskId::new();
        let workspace = TaskWorkspace {
            issue_identifier: "TEST-1",
            task_id: &id,
            worktree: repo.path(),
            base_commit: "",
        };
        fs::write(outside.path().join("notes"), "external").unwrap();
        symlink(outside.path(), repo.path().join("link")).unwrap();
        assert!(save(workspace, "link/notes", "revision", "draft").is_err());
        assert_eq!(
            fs::read_to_string(outside.path().join("notes")).unwrap(),
            "external"
        );
        fs::write(repo.path().join("binary"), [0, 255]).unwrap();
        assert!(
            !save(workspace, "binary", "revision", "draft")
                .unwrap()
                .published
        );
        assert_eq!(fs::read(repo.path().join("binary")).unwrap(), [0, 255]);
        assert!(save(workspace, ".git/config", "revision", "draft").is_err());
    }
}
