//! The writes an app makes inside its own process. Each [`Write`] is an `lf`
//! command: [`Write::argv`] names it, and [`Write::write`] calls the function
//! that command calls, returns what it prints, and records the Process
//! `lf <argv>` would, with the app's process as its OS process. Arrangement
//! (Dashboards, the Task order, pane layouts) persists without a Process, as
//! its command does.
//!
//! Every write names the directory the app ran it from: `--cwd <directory>`
//! in its argv, and the same path in-process.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Context;
use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::lf::{Cli, Commands};
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
    /// `lf wave ensure <wave> --json`: the Wave's active Project.
    WaveEnsure { directory: PathBuf, wave: String },
    /// `lf wave status <wave> --sync --json`: the planning refresh.
    PlanningRefresh { directory: PathBuf, wave: String },
    /// `lf desktop query [--revision <revision>] -- <id> <expression>`.
    DashboardQuery {
        directory: PathBuf,
        id: String,
        expression: String,
        revision: Option<i64>,
    },
    /// `lf desktop pin|unpin <id> --revision <revision>`.
    DashboardPin {
        directory: PathBuf,
        id: String,
        pinned: bool,
        revision: i64,
    },
    /// `lf desktop rename --revision <revision> -- query:<id> <name>`.
    DashboardRename {
        directory: PathBuf,
        id: String,
        name: String,
        revision: i64,
    },
    /// `lf desktop hide|show -p query:<id>/<member>`.
    DashboardTile {
        directory: PathBuf,
        id: String,
        member: String,
        hidden: bool,
    },
    /// `lf desktop layout query:<id> --save --revision <revision>`, with
    /// `slots` (JSON) on stdin.
    DashboardSlots {
        directory: PathBuf,
        id: String,
        revision: i64,
        slots: String,
    },
    /// `lf desktop move -p <board>/<member> --destination <destination>`: a
    /// Dashboard's tile (`query:<id>`) or the shared Task order (`working-tasks`).
    TileMove {
        directory: PathBuf,
        board: String,
        member: String,
        destination: String,
    },
    /// `lf desktop layout <work> --save`, with `layout` (JSON) on stdin.
    PaneLayout {
        directory: PathBuf,
        work: String,
        layout: String,
    },
}

/// In-app writes record one Process at a time: a Process's identity reaches
/// what it runs through this process's environment.
static WRITING: Mutex<()> = Mutex::new(());

impl Write {
    /// The `lf` command this write equals, without the binary.
    pub fn argv(&self) -> Vec<String> {
        let mut argv = vec!["--cwd".to_owned(), self.directory().display().to_string()];
        let (notes_flag, revision_flag, query, path);
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
            Self::WaveEnsure { wave, .. } => vec!["wave", "ensure", wave, "--json"],
            Self::PlanningRefresh { wave, .. } => {
                vec!["wave", "status", wave, "--sync", "--json"]
            }
            Self::DashboardQuery {
                id,
                expression,
                revision,
                ..
            } => {
                let mut words = vec!["desktop", "query"];
                if let Some(revision) = revision {
                    revision_flag = format!("--revision={revision}");
                    words.push(&revision_flag);
                }
                words.extend(["--", id, expression]);
                words
            }
            Self::DashboardPin {
                id,
                pinned,
                revision,
                ..
            } => {
                revision_flag = format!("--revision={revision}");
                let verb = if *pinned { "pin" } else { "unpin" };
                vec!["desktop", verb, &revision_flag, "--", id]
            }
            Self::DashboardRename {
                id, name, revision, ..
            } => {
                revision_flag = format!("--revision={revision}");
                query = format!("query:{id}");
                vec!["desktop", "rename", &revision_flag, "--", &query, name]
            }
            Self::DashboardTile {
                id, member, hidden, ..
            } => {
                path = format!("query:{id}/{member}");
                let verb = if *hidden { "hide" } else { "show" };
                vec!["desktop", verb, "-p", &path]
            }
            Self::DashboardSlots { id, revision, .. } => {
                revision_flag = format!("--revision={revision}");
                query = format!("query:{id}");
                vec!["desktop", "layout", &query, "--save", &revision_flag]
            }
            Self::TileMove {
                board,
                member,
                destination,
                ..
            } => {
                path = format!("{board}/{member}");
                vec!["desktop", "move", "-p", &path, "--destination", destination]
            }
            Self::PaneLayout { work, .. } => vec!["desktop", "layout", work, "--save"],
        };
        argv.extend(words.into_iter().map(str::to_owned));
        argv
    }

    /// What the command reads from stdin.
    pub fn input(&self) -> Option<&str> {
        match self {
            Self::TaskSave { content, .. } => Some(content),
            Self::DashboardSlots { slots, .. } => Some(slots),
            Self::PaneLayout { layout, .. } => Some(layout),
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
            | Self::SessionBind { directory, .. }
            | Self::WaveEnsure { directory, .. }
            | Self::PlanningRefresh { directory, .. }
            | Self::DashboardQuery { directory, .. }
            | Self::DashboardPin { directory, .. }
            | Self::DashboardRename { directory, .. }
            | Self::DashboardTile { directory, .. }
            | Self::DashboardSlots { directory, .. }
            | Self::TileMove { directory, .. }
            | Self::PaneLayout { directory, .. } => directory,
        }
    }

    /// Where `lf` records the write's Process: a Task command runs from the
    /// Task's repository, a Wave command from the repository it is in.
    fn repo(&self) -> anyhow::Result<PathBuf> {
        let directory = self.directory();
        match self {
            Self::TaskComplete { issue, .. }
            | Self::TaskReopen { issue, .. }
            | Self::TaskDefer { issue, .. }
            | Self::TaskNotes { issue, .. }
            | Self::TaskMove { issue, .. }
            | Self::TaskSave { issue, .. } => Ok(task_repository(directory, Some(issue))?),
            Self::WaveEnsure { .. } => crate::repo::require_repo_root(directory, "this command"),
            _ => crate::repo::working_directory_in(directory),
        }
    }

    /// Make the write on the caller's thread and return what `lf <argv>`
    /// prints to stdout, recording its Process around it when `lf` would.
    pub fn write(&self) -> anyhow::Result<String> {
        let argv: Vec<String> = std::iter::once("lf".to_owned())
            .chain(self.argv())
            .collect();
        let command = Cli::try_parse_from(&argv)?
            .command
            .context("a write names its command")?;
        if !command.changes_work() {
            return self.perform(self.directory(), command);
        }
        let _writing = WRITING.lock().unwrap_or_else(|poison| poison.into_inner());
        let repo = self.repo()?;
        crate::journal::with_runtime(&repo, &argv, || self.perform(&repo, command))
    }

    fn perform(&self, repo: &Path, command: Commands) -> anyhow::Result<String> {
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
                // Planning delivery starts right after the local commit and
                // prints to stderr in `lf`; one that fails stays pending for
                // the app's planning refresh to retry.
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
            Self::WaveEnsure { wave, .. } => pretty(
                &tokio::runtime::Runtime::new()?
                    .block_on(crate::ops::project::ensure(repo, wave))?,
            )?,
            Self::PlanningRefresh { directory, wave } => {
                format!(
                    "{}\n",
                    super::waves::refreshed_status(directory, Some(wave))?
                )
            }
            // Arrangement parses its own argv, so the app and `lf` reach the
            // command's one function with the same arguments.
            Self::DashboardQuery { directory, .. }
            | Self::DashboardPin { directory, .. }
            | Self::DashboardRename { directory, .. }
            | Self::DashboardTile { directory, .. }
            | Self::DashboardSlots { directory, .. }
            | Self::TileMove { directory, .. }
            | Self::PaneLayout { directory, .. } => {
                let Commands::Desktop { cmd } = command else {
                    unreachable!("arrangement is a desktop command")
                };
                let input = self.input().unwrap_or_default();
                let saved =
                    super::desktop::save_arrangement(None, directory, &cmd, &mut input.as_bytes())?;
                format!("{saved}\n")
            }
        })
    }
}

fn pretty(value: &impl Serialize) -> anyhow::Result<String> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(
            write(
                r#"{"write":"dashboard_pin","directory":"/r","id":"picked","pinned":false,"revision":3}"#
            ),
            [
                "--cwd",
                "/r",
                "desktop",
                "unpin",
                "--revision=3",
                "--",
                "picked"
            ]
        );
        assert_eq!(
            write(
                r#"{"write":"dashboard_query","directory":"/r","id":"picked","expression":"-state:done","revision":null}"#
            ),
            [
                "--cwd",
                "/r",
                "desktop",
                "query",
                "--",
                "picked",
                "-state:done"
            ]
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
                directory: directory.clone(),
                id: "session_1".into(),
                task: "FIX-1".into(),
            },
            Write::WaveEnsure {
                directory: directory.clone(),
                wave: "product".into(),
            },
            Write::PlanningRefresh {
                directory: directory.clone(),
                wave: "product".into(),
            },
        ];
        let arrangement = [
            Write::DashboardQuery {
                directory: directory.clone(),
                id: "picked".into(),
                expression: "-state:done".into(),
                revision: Some(3),
            },
            Write::DashboardPin {
                directory: directory.clone(),
                id: "picked".into(),
                pinned: false,
                revision: 3,
            },
            Write::DashboardRename {
                directory: directory.clone(),
                id: "picked".into(),
                name: "-n".into(),
                revision: 3,
            },
            Write::DashboardTile {
                directory: directory.clone(),
                id: "picked".into(),
                member: "task:task_1".into(),
                hidden: true,
            },
            Write::DashboardSlots {
                directory: directory.clone(),
                id: "picked".into(),
                revision: 3,
                slots: "null".into(),
            },
            Write::TileMove {
                directory: directory.clone(),
                board: "working-tasks".into(),
                member: "task:task_1".into(),
                destination: "task:task_2".into(),
            },
            Write::PaneLayout {
                directory,
                work: "task:task_1".into(),
                layout: "{}".into(),
            },
        ];
        let command = |write: &Write| {
            let argv = std::iter::once("lf".to_owned()).chain(write.argv());
            Cli::try_parse_from(argv)
                .unwrap_or_else(|error| panic!("{write:?}: {error}"))
                .command
                .unwrap()
        };
        for write in writes {
            assert!(command(&write).changes_work(), "{write:?}");
        }
        for write in arrangement {
            assert!(!command(&write).changes_work(), "{write:?}");
        }
    }
}
