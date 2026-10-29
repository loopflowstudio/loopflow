//! The writes an app makes inside its own process. Each [`Write`] is an `lf`
//! command that changes Work: [`Write::argv`] names it, and [`Write::write`]
//! calls the function that command calls, returns what it prints, and records
//! the Process `lf <argv>` would, with the app's process as its OS process.
//!
//! Every write names the directory the app ran it from: `--cwd <directory>`
//! in its argv, and the same path in-process.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::ops::task::{
    task_complete, task_defer, task_edit, task_move_membership, task_reopen, task_repository,
    task_save,
};
use crate::work::task::TaskUpdate;

/// A write an app makes inside its own process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "write", rename_all = "snake_case", deny_unknown_fields)]
// Exhaustive on purpose: the equivalence test matches every variant.
pub enum Write {
    /// `lf task complete <issue>`.
    TaskComplete { directory: PathBuf, issue: String },
    /// `lf task reopen <issue>`.
    TaskReopen { directory: PathBuf, issue: String },
    /// `lf task defer <issue>`, or `lf task resume <issue>` when not `deferred`.
    TaskDefer {
        directory: PathBuf,
        issue: String,
        deferred: bool,
    },
    /// `lf task edit <issue> --notes=<notes>`.
    TaskNotes {
        directory: PathBuf,
        issue: String,
        notes: String,
    },
    /// `lf task move <issue> --wave <wave>`, or `--no-wave` without one.
    TaskMove {
        directory: PathBuf,
        issue: String,
        wave: Option<String>,
    },
    /// `lf task save <issue> <path> --revision <revision> --json`, with
    /// `content` on stdin.
    TaskSave {
        directory: PathBuf,
        issue: String,
        path: String,
        revision: String,
        content: String,
    },
    /// `lf session rename --json -- <id> <name>`.
    SessionRename {
        directory: PathBuf,
        id: String,
        name: String,
    },
    /// `lf session bind --json --task <task> -- <id>`.
    SessionBind {
        directory: PathBuf,
        id: String,
        task: String,
    },
}

/// In-app writes record one Process at a time: a Process's identity reaches
/// what it runs through this process's environment.
static WRITING: Mutex<()> = Mutex::new(());

impl Write {
    /// The `lf` command this write equals, without the binary.
    pub fn argv(&self) -> Vec<String> {
        let mut argv = vec!["--cwd".to_owned(), self.directory().display().to_string()];
        let notes_flag;
        let words: Vec<&str> = match self {
            Self::TaskComplete { issue, .. } => vec!["task", "complete", issue],
            Self::TaskReopen { issue, .. } => vec!["task", "reopen", issue],
            Self::TaskDefer {
                issue, deferred, ..
            } => vec!["task", if *deferred { "defer" } else { "resume" }, issue],
            Self::TaskNotes { issue, notes, .. } => {
                notes_flag = format!("--notes={notes}");
                vec!["task", "edit", issue, &notes_flag]
            }
            Self::TaskMove { issue, wave, .. } => match wave {
                Some(wave) => vec!["task", "move", issue, "--wave", wave],
                None => vec!["task", "move", issue, "--no-wave"],
            },
            Self::TaskSave {
                issue,
                path,
                revision,
                ..
            } => vec![
                "task",
                "save",
                issue,
                path,
                "--revision",
                revision,
                "--json",
            ],
            Self::SessionRename { id, name, .. } => {
                vec!["session", "rename", "--json", "--", id, name]
            }
            Self::SessionBind { id, task, .. } => {
                vec!["session", "bind", "--json", "--task", task, "--", id]
            }
        };
        argv.extend(words.into_iter().map(str::to_owned));
        argv
    }

    /// What the command reads from stdin.
    pub fn input(&self) -> Option<&str> {
        match self {
            Self::TaskSave { content, .. } => Some(content),
            _ => None,
        }
    }

    fn directory(&self) -> &Path {
        match self {
            Self::TaskComplete { directory, .. }
            | Self::TaskReopen { directory, .. }
            | Self::TaskDefer { directory, .. }
            | Self::TaskNotes { directory, .. }
            | Self::TaskMove { directory, .. }
            | Self::TaskSave { directory, .. }
            | Self::SessionRename { directory, .. }
            | Self::SessionBind { directory, .. } => directory,
        }
    }

    /// The Task a write names, which places its Process as `lf` places it.
    fn issue(&self) -> Option<&str> {
        match self {
            Self::TaskComplete { issue, .. }
            | Self::TaskReopen { issue, .. }
            | Self::TaskDefer { issue, .. }
            | Self::TaskNotes { issue, .. }
            | Self::TaskMove { issue, .. }
            | Self::TaskSave { issue, .. } => Some(issue),
            Self::SessionRename { .. } | Self::SessionBind { .. } => None,
        }
    }

    /// Make the write on the caller's thread and return what `lf <argv>`
    /// prints to stdout, recording its Process around it as `lf` would.
    pub fn write(&self) -> anyhow::Result<String> {
        let argv: Vec<String> = std::iter::once("lf".to_owned())
            .chain(self.argv())
            .collect();
        let _writing = WRITING.lock().unwrap_or_else(|poison| poison.into_inner());
        // `lf` runs a Task command from the Task's repository.
        let repo = match self.issue() {
            Some(issue) => task_repository(self.directory(), Some(issue))?,
            None => self.directory().to_owned(),
        };
        crate::journal::with_runtime(&repo, &argv, || self.perform(&repo))
    }

    fn perform(&self, repo: &Path) -> anyhow::Result<String> {
        Ok(match self {
            Self::TaskComplete { issue, .. } => {
                task_complete(repo, issue, None)?;
                format!("{issue}: completed\n")
            }
            Self::TaskReopen { issue, .. } => {
                task_reopen(issue, None)?;
                format!("{issue}: reopened; its Graph unchanged\n")
            }
            Self::TaskDefer {
                issue, deferred, ..
            } => {
                task_defer(issue, *deferred)?;
                let state = if *deferred { "deferred" } else { "resumed" };
                format!("{issue}: {state}; execution unchanged\n")
            }
            Self::TaskNotes { issue, notes, .. } => {
                let update = TaskUpdate {
                    description: Some(notes.clone()),
                    ..TaskUpdate::default()
                };
                // Planning delivery prints to stderr in `lf`; a pending one
                // stays pending for the app's planning refresh to retry.
                format!("{}: updated\n", task_edit(repo, issue, None, update)?.id)
            }
            Self::TaskMove { issue, wave, .. } => {
                let moved = task_move_membership(repo, issue, wave.as_deref())?;
                format!("{}: membership saved\n", moved.id)
            }
            Self::TaskSave {
                issue,
                path,
                revision,
                content,
                ..
            } => pretty(&task_save(issue, path, revision, content)?)?,
            Self::SessionRename { id, name, .. } => pretty(&super::session::renamed(id, name)?)?,
            Self::SessionBind { id, task, .. } => pretty(&super::session::bound(id, task)?)?,
        })
    }
}

fn pretty(value: &impl Serialize) -> anyhow::Result<String> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lf::Cli;
    use clap::Parser;

    /// The JSON `RegistryQueryTests.servedWriteEncodings` encodes in Swift.
    #[test]
    fn desktop_encodings_name_their_writes() {
        let write = |json: &str| serde_json::from_str::<Write>(json).unwrap().argv();
        assert_eq!(
            write(r#"{"write":"task_move","directory":"/r","issue":"FIX-1","wave":null}"#),
            ["--cwd", "/r", "task", "move", "FIX-1", "--no-wave"]
        );
        assert_eq!(
            write(r#"{"write":"task_defer","directory":"/r","issue":"FIX-1","deferred":false}"#),
            ["--cwd", "/r", "task", "resume", "FIX-1"]
        );
        let save = serde_json::from_str::<Write>(
            r#"{"write":"task_save","directory":"/","issue":"FIX-1","path":"a.txt","revision":"r","content":"text"}"#,
        )
        .unwrap();
        assert_eq!(save.input(), Some("text"));
    }

    #[test]
    fn every_write_parses_as_the_lf_command_it_names() {
        let directory = PathBuf::from("/tmp/repo");
        let writes = [
            Write::TaskComplete {
                directory: directory.clone(),
                issue: "FIX-1".into(),
            },
            Write::TaskReopen {
                directory: directory.clone(),
                issue: "FIX-1".into(),
            },
            Write::TaskDefer {
                directory: directory.clone(),
                issue: "FIX-1".into(),
                deferred: false,
            },
            Write::TaskNotes {
                directory: directory.clone(),
                issue: "FIX-1".into(),
                notes: "--not a flag".into(),
            },
            Write::TaskMove {
                directory: directory.clone(),
                issue: "FIX-1".into(),
                wave: None,
            },
            Write::TaskSave {
                directory: directory.clone(),
                issue: "FIX-1".into(),
                path: "a.txt".into(),
                revision: "r".into(),
                content: "text".into(),
            },
            Write::SessionRename {
                directory: directory.clone(),
                id: "session_1".into(),
                name: "-n".into(),
            },
            Write::SessionBind {
                directory,
                id: "session_1".into(),
                task: "FIX-1".into(),
            },
        ];
        for write in writes {
            let argv = std::iter::once("lf".to_owned()).chain(write.argv());
            let command = Cli::try_parse_from(argv)
                .unwrap_or_else(|error| panic!("{write:?}: {error}"))
                .command
                .unwrap();
            assert!(command.changes_work(), "{write:?}");
        }
    }
}
