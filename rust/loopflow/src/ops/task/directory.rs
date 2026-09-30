use std::collections::HashSet;
use std::io::{Seek, Write};
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::{file_context, task_error, validate_task_relative_path, TaskWorkspace};
use crate::ops::error::OpsResult;

const PAGE_SIZE: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskFileKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFileEntry {
    pub path: String,
    pub kind: TaskFileKind,
}

impl TaskFileEntry {
    fn sort_key(&self) -> (bool, &str) {
        (self.kind != TaskFileKind::Directory, &self.path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    let checkout = file_context(issue)?;
    directory_snapshot(
        TaskWorkspace::from(&checkout),
        directory,
        cursor,
        show_ignored,
    )
}

fn directory_snapshot(
    workspace: TaskWorkspace<'_>,
    directory: &str,
    cursor: Option<&str>,
    show_ignored: bool,
) -> OpsResult<TaskDirectory> {
    let directory = if directory == "." || directory.is_empty() {
        String::new()
    } else {
        validate_task_relative_path(directory)?
    };
    if Path::new(&directory)
        .components()
        .any(|part| part.as_os_str() == ".git")
    {
        return Err(task_error("Git metadata is not a Task directory"));
    }
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
    let root = workspace.worktree.canonicalize()?;
    let absolute = root.join(&directory).canonicalize()?;
    if !absolute.starts_with(&root) || !absolute.is_dir() {
        return Err(task_error("Directory must remain inside the Task worktree"));
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
        // File-backed input avoids a pipe deadlock for directories larger than a pipe buffer.
        let mut input = tempfile::tempfile()?;
        for entry in &entries {
            input.write_all(entry.path.as_bytes())?;
            input.write_all(b"\0")?;
        }
        input.rewind()?;
        let output = Command::new("git")
            .current_dir(&root)
            .args(["check-ignore", "--stdin", "-z"])
            .stdin(input)
            .output()?;
        if !output.status.success() && output.status.code() != Some(1) {
            return Err(task_error(format!(
                "cannot read ignored paths: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let ignored: HashSet<&[u8]> = output.stdout.split(|byte| *byte == 0).collect();
        entries.retain(|entry| !ignored.contains(entry.path.as_bytes()));
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
    use crate::ops::task::TaskWorkspace;
    use crate::work::task::TaskId;

    #[test]
    fn task_files_pages_include_unchanged_untracked_and_bounded_links() {
        let repo = loopflow_test_support::TestRepo::new();
        let id = TaskId::new();
        let workspace = TaskWorkspace {
            issue_identifier: "FILES-1",
            task_id: &id,
            worktree: repo.path(),
        };
        std::fs::write(repo.path().join(".gitignore"), "ignored/\n").unwrap();
        for directory in ["nested", "ignored"] {
            std::fs::create_dir(repo.path().join(directory)).unwrap();
        }
        for index in 0..505 {
            std::fs::write(repo.path().join(format!("file-{index:03}")), "").unwrap();
        }
        std::os::unix::fs::symlink("nested", repo.path().join("link")).unwrap();
        let first = directory_snapshot(workspace, "", None, false).unwrap();
        assert_eq!(first.entries.len(), 500);
        assert_eq!(first.entries[0].path, "nested");
        assert!(!first
            .entries
            .iter()
            .any(|entry| entry.path == ".git" || entry.path == "ignored"));
        let second =
            directory_snapshot(workspace, "", first.next_cursor.as_deref(), false).unwrap();
        assert!(second.next_cursor.is_none());
        assert!(second
            .entries
            .iter()
            .any(|entry| entry.path == "link" && entry.kind == TaskFileKind::Symlink));
        assert!(first
            .entries
            .iter()
            .all(|entry| !second.entries.contains(entry)));
        let visible = directory_snapshot(workspace, "", None, true).unwrap();
        assert!(visible.entries.iter().any(|entry| entry.path == "ignored"));
        assert!(
            directory_snapshot(workspace, "nested", first.next_cursor.as_deref(), false).is_err()
        );
        assert!(directory_snapshot(workspace, "", first.next_cursor.as_deref(), true).is_err());
        assert!(directory_snapshot(workspace, ".git", None, true).is_err());
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), repo.path().join("outside")).unwrap();
        assert!(directory_snapshot(workspace, "outside", None, true).is_err());
    }
}
