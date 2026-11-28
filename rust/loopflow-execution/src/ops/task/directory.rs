use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{file_context, file_store, task_error, validate_task_relative_path};
use crate::engine::{Error, ErrorCode};
use crate::ops::error::OpsResult;

const PAGE_SIZE: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskFileKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskFileEntry {
    pub path: String,
    pub kind: TaskFileKind,
}

impl TaskFileEntry {
    fn sort_key(&self) -> (bool, &str) {
        (self.kind != TaskFileKind::Directory, &self.path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskDirectory {
    pub path: String,
    pub entries: Vec<TaskFileEntry>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Cursor {
    directory: String,
    show_ignored: bool,
    last: TaskFileEntry,
}

pub fn task_files(
    issue: &str,
    directory: &str,
    cursor: Option<&str>,
    show_ignored: bool,
) -> OpsResult<TaskDirectory> {
    let checkout = file_context(&file_store()?, issue)?;
    directory_snapshot(&checkout.worktree, directory, cursor, show_ignored)
}

fn directory_snapshot(
    worktree: &Path,
    directory: &str,
    cursor: Option<&str>,
    show_ignored: bool,
) -> OpsResult<TaskDirectory> {
    let directory = if directory == "." || directory.is_empty() {
        String::new()
    } else {
        validate_task_relative_path(directory)?
    };
    let cursor: Option<Cursor> = cursor
        .map(|value| {
            let bytes = hex::decode(value).map_err(|_| task_error("Invalid directory cursor"))?;
            serde_json::from_slice(&bytes).map_err(|_| task_error("Invalid directory cursor"))
        })
        .transpose()?;
    if cursor
        .as_ref()
        .is_some_and(|cursor| cursor.directory != directory || cursor.show_ignored != show_ignored)
    {
        return Err(task_error(
            "Directory cursor belongs to another directory or ignored-file selection",
        ));
    }
    let root = worktree.canonicalize()?;
    let absolute = root.join(&directory).canonicalize()?;
    if !absolute.starts_with(&root) {
        return Err(task_error("Directory must remain inside the Task worktree"));
    }
    let metadata = absolute.metadata()?;
    if metadata.is_file() {
        return Err(
            Error::new(ErrorCode::Invalid, "Task directory read needs a folder.")
                .with(serde_json::json!({ "reason": "file" }))
                .into(),
        );
    }
    if !metadata.is_dir() {
        return Err(task_error("Task directory read needs a folder"));
    }
    if Path::new(&directory)
        .components()
        .any(|part| part.as_os_str() == ".git")
    {
        return Err(task_error("Git metadata is not a Task directory"));
    }
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&absolute)? {
        let entry = entry?;
        if entry.file_name() == ".git" {
            continue;
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| task_error("Task directory contains a non-UTF-8 filename"))?;
        let kind = entry.file_type()?;
        let kind = if kind.is_symlink() {
            TaskFileKind::Symlink
        } else if kind.is_dir() {
            TaskFileKind::Directory
        } else if kind.is_file() {
            TaskFileKind::File
        } else {
            continue;
        };
        let path = if directory.is_empty() {
            name
        } else {
            format!("{directory}/{name}")
        };
        entries.push(TaskFileEntry { path, kind });
    }
    if !show_ignored && !entries.is_empty() {
        let ignored =
            crate::git::ignored_paths(&root, entries.iter().map(|entry| entry.path.as_str()))
                .map_err(task_error)?;
        entries.retain(|entry| !ignored.contains(&entry.path));
    }
    entries.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
    if let Some(cursor) = &cursor {
        entries.retain(|entry| entry.sort_key() > cursor.last.sort_key());
    }
    let has_more = entries.len() > PAGE_SIZE;
    entries.truncate(PAGE_SIZE);
    let next_cursor = if has_more {
        Some(hex::encode(
            serde_json::to_vec(&Cursor {
                directory: directory.clone(),
                show_ignored,
                last: entries.last().expect("full page has entries").clone(),
            })
            .map_err(task_error)?,
        ))
    } else {
        None
    };
    Ok(TaskDirectory {
        path: directory,
        entries,
        next_cursor,
    })
}

#[cfg(test)]
mod tests {
    use super::{directory_snapshot, TaskFileKind};
    use crate::engine::{Error, ErrorCode};
    use crate::ops::error::OpsError;

    #[test]
    fn a_directory_read_identifies_files_inside_the_checkout() {
        let repo = loopflow_test_support::TestRepo::new();
        let root = repo.path();
        std::fs::write(root.join("file"), "").unwrap();
        std::os::unix::fs::symlink("file", root.join("file-link")).unwrap();
        std::fs::create_dir(root.join("folder")).unwrap();
        std::os::unix::fs::symlink("folder", root.join("folder-link")).unwrap();
        std::os::unix::fs::symlink(root.parent().unwrap(), root.join("outside")).unwrap();

        for path in ["file", "file-link"] {
            let error = directory_snapshot(root, path, None, true).unwrap_err();
            assert!(
                matches!(error, OpsError::Refused(Error { code: ErrorCode::Invalid, data: Some(data), .. })
                if data == serde_json::json!({ "reason": "file" }))
            );
        }
        assert!(directory_snapshot(root, "folder-link", None, true).is_ok());
        for path in ["outside", "missing"] {
            assert!(!matches!(
                directory_snapshot(root, path, None, true),
                Err(OpsError::Refused(_))
            ));
        }
    }

    #[test]
    fn task_files_pages_include_unchanged_untracked_and_bounded_links() {
        let repo = loopflow_test_support::TestRepo::new();
        let worktree = repo.path();
        std::fs::write(repo.path().join(".gitignore"), "ignored/\n").unwrap();
        for directory in ["nested", "ignored"] {
            std::fs::create_dir(repo.path().join(directory)).unwrap();
        }
        for index in 0..505 {
            std::fs::write(repo.path().join(format!("file-{index:03}")), "").unwrap();
        }
        std::os::unix::fs::symlink("nested", repo.path().join("link")).unwrap();
        let first = directory_snapshot(worktree, "", None, false).unwrap();
        assert_eq!(first.entries.len(), 500);
        assert_eq!(first.entries[0].path, "nested");
        assert!(!first
            .entries
            .iter()
            .any(|entry| entry.path == ".git" || entry.path == "ignored"));
        let second = directory_snapshot(worktree, "", first.next_cursor.as_deref(), false).unwrap();
        assert!(second.next_cursor.is_none());
        assert!(second
            .entries
            .iter()
            .any(|entry| entry.path == "link" && entry.kind == TaskFileKind::Symlink));
        assert!(first
            .entries
            .iter()
            .all(|entry| !second.entries.contains(entry)));
        let visible = directory_snapshot(worktree, "", None, true).unwrap();
        assert!(visible.entries.iter().any(|entry| entry.path == "ignored"));
        assert!(
            directory_snapshot(worktree, "nested", first.next_cursor.as_deref(), false).is_err()
        );
        assert!(directory_snapshot(worktree, "", first.next_cursor.as_deref(), true).is_err());
        assert!(directory_snapshot(worktree, ".git", None, true).is_err());
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), repo.path().join("outside")).unwrap();
        assert!(directory_snapshot(worktree, "outside", None, true).is_err());
    }
}
