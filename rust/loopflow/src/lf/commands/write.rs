//! The writes an app makes inside its own process. Each [`Write`] is an `lf`
//! command: [`Write::argv`] names it, and [`Write::write`] calls the function
//! that command calls, returns what it prints, and records the Process
//! `lf <argv>` would, with the app's process as its OS process. Arrangement
//! (Dashboards, the Task order, pane layouts) persists without a Process, as
//! its command does.
//!
//! Every write names the directory the app ran it from: `--cwd <directory>`
//! in its argv, and the same path in-process. A write `lf` would send to
//! another Machine is not made here: [`Write::write`] returns `None`, and the
//! app runs its command, which sends it.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Context;
use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::lf::{Cli, Commands};
use crate::ops::task::{
    task_complete, task_edit, task_move_membership, task_reopen, task_repository, task_save,
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
    /// `lf task edit <issue> --notes=<notes>`.
    TaskNotes {
        directory: PathBuf,
        issue: String,
        notes: String,
    },
    /// `lf task abandon <issue>`: cancels the Task and removes its PRs and
    /// branches.
    TaskAbandon { directory: PathBuf, issue: String },
    /// `lf task claim <issue> --json`, or `lf task unassign <issue> --json`
    /// when not `claimed`.
    TaskOwnership {
        directory: PathBuf,
        issue: String,
        claimed: bool,
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
    /// `lf [--agent <agent>] session resume [<id>] [--task <task>] [--wave <wave>]
    /// --prepare --json`, with `--repo <directory>` when it names none of them:
    /// the conversation and the command that opens it, no provider started.
    SessionResume {
        directory: PathBuf,
        id: Option<String>,
        task: Option<String>,
        wave: Option<String>,
        agent: Option<String>,
    },
    /// `lf [--agent <agent>] session new <id> --prepare --json`: the
    /// conversation on a fresh provider thread, not yet started.
    SessionNew {
        directory: PathBuf,
        id: String,
        agent: Option<String>,
    },
    /// `lf wave ensure <wave> --json`: the Wave's active Project.
    WaveEnsure { directory: PathBuf, wave: String },
    /// `lf wave status <wave> --sync --json`: the planning refresh.
    PlanningRefresh { directory: PathBuf, wave: String },
    /// `lf wave new-chapter --all <name> --plan /dev/stdin --json`, with
    /// `plan` (JSON) on stdin; `--dry-run` when `preview`, which records nothing.
    NewChapter {
        directory: PathBuf,
        name: String,
        plan: String,
        preview: bool,
    },
    /// `lf graph customize <name>`: copies a builtin Graph into the repository,
    /// recording nothing.
    GraphCustomize { directory: PathBuf, name: String },
    /// `lf portfolio <change> --json`, run from `/`: which repositories the
    /// app shows.
    Portfolio { change: Vec<String> },
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
            Self::TaskAbandon { issue, .. } => vec!["task", "abandon", issue],
            Self::TaskNotes { issue, notes, .. } => {
                notes_flag = format!("--notes={notes}");
                vec!["task", "edit", issue, &notes_flag]
            }
            Self::TaskOwnership { issue, claimed, .. } => {
                vec![
                    "task",
                    if *claimed { "claim" } else { "unassign" },
                    issue,
                    "--json",
                ]
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
            Self::SessionResume {
                directory,
                id,
                task,
                wave,
                agent,
            } => {
                let mut words = Vec::new();
                if id.is_none() && task.is_none() && wave.is_none() {
                    path = directory.display().to_string();
                    words.extend(["--repo", path.as_str()]);
                }
                if let Some(agent) = agent {
                    words.extend(["--agent", agent]);
                }
                words.extend(["session", "resume"]);
                words.extend(id.as_deref());
                if let Some(task) = task {
                    words.extend(["--task", task]);
                }
                if let Some(wave) = wave {
                    words.extend(["--wave", wave]);
                }
                words.extend(["--prepare", "--json"]);
                words
            }
            Self::SessionNew { id, agent, .. } => {
                let mut words = Vec::new();
                if let Some(agent) = agent {
                    words.extend(["--agent", agent]);
                }
                words.extend(["session", "new", id, "--prepare", "--json"]);
                words
            }
            Self::WaveEnsure { wave, .. } => vec!["wave", "ensure", wave, "--json"],
            Self::PlanningRefresh { wave, .. } => {
                vec!["wave", "status", wave, "--sync", "--json"]
            }
            Self::NewChapter { name, preview, .. } => {
                let mut words = vec![
                    "wave",
                    "new-chapter",
                    "--all",
                    name,
                    "--plan",
                    "/dev/stdin",
                    "--json",
                ];
                if *preview {
                    words.push("--dry-run");
                }
                words
            }
            Self::GraphCustomize { name, .. } => vec!["graph", "customize", name],
            Self::Portfolio { change } => std::iter::once("portfolio")
                .chain(change.iter().map(String::as_str))
                .chain(["--json"])
                .collect(),
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
            Self::NewChapter { plan, .. } => Some(plan),
            Self::DashboardSlots { slots, .. } => Some(slots),
            Self::PaneLayout { layout, .. } => Some(layout),
            _ => None,
        }
    }

    fn directory(&self) -> &Path {
        match self {
            Self::TaskComplete { directory, .. }
            | Self::TaskReopen { directory, .. }
            | Self::TaskAbandon { directory, .. }
            | Self::TaskNotes { directory, .. }
            | Self::TaskOwnership { directory, .. }
            | Self::TaskMove { directory, .. }
            | Self::TaskSave { directory, .. }
            | Self::SessionRename { directory, .. }
            | Self::SessionBind { directory, .. }
            | Self::SessionResume { directory, .. }
            | Self::SessionNew { directory, .. }
            | Self::WaveEnsure { directory, .. }
            | Self::PlanningRefresh { directory, .. }
            | Self::NewChapter { directory, .. }
            | Self::GraphCustomize { directory, .. }
            | Self::DashboardQuery { directory, .. }
            | Self::DashboardPin { directory, .. }
            | Self::DashboardRename { directory, .. }
            | Self::DashboardTile { directory, .. }
            | Self::DashboardSlots { directory, .. }
            | Self::TileMove { directory, .. }
            | Self::PaneLayout { directory, .. } => directory,
            Self::Portfolio { .. } => Path::new("/"),
        }
    }

    /// Where the write runs and `lf` records its Process: a Task command runs
    /// from the Task's repository, a Wave command from the repository it is in.
    fn repo(&self) -> anyhow::Result<PathBuf> {
        let directory = self.directory();
        match self {
            Self::TaskComplete { issue, .. }
            | Self::TaskReopen { issue, .. }
            | Self::TaskAbandon { issue, .. }
            | Self::TaskNotes { issue, .. }
            | Self::TaskOwnership { issue, .. }
            | Self::TaskMove { issue, .. }
            | Self::TaskSave { issue, .. } => Ok(task_repository(directory, Some(issue))?),
            Self::WaveEnsure { .. } | Self::NewChapter { .. } => {
                crate::repo::require_repo_root(directory, "this command")
            }
            _ => crate::repo::working_directory_in(directory),
        }
    }

    /// Make the write on the caller's thread and return what `lf <argv>`
    /// prints to stdout, recording its Process around it when `lf` would.
    /// `None` when `lf` would send it to another Machine: nothing was written.
    pub fn write(&self) -> anyhow::Result<Option<String>> {
        let argv: Vec<String> = std::iter::once("lf".to_owned())
            .chain(self.argv())
            .collect();
        let cli = Cli::try_parse_from(&argv)?;
        if super::work_route::forward(&cli, &argv, self.directory())?.is_some() {
            return Ok(None);
        }
        let repo = self.repo()?;
        if !cli.command.as_ref().is_some_and(Commands::changes_work) {
            return self.perform(&repo, &cli).map(Some);
        }
        let _writing = WRITING.lock().unwrap_or_else(|poison| poison.into_inner());
        // `lf` hands its `--agent` to what it runs through the environment.
        let _agent = cli
            .agent
            .as_deref()
            .map(|agent| super::graph::EnvVarGuard::set(crate::config::AGENT_OVERRIDE_ENV, agent));
        crate::journal::with_runtime(&repo, &argv, || self.perform(&repo, &cli)).map(Some)
    }

    fn perform(&self, repo: &Path, cli: &Cli) -> anyhow::Result<String> {
        let command = cli.command.as_ref().context("a write names its command")?;
        Ok(match self {
            Self::TaskComplete { issue, .. } => {
                task_complete(repo, issue, None)?;
                format!("{issue}: completed\n")
            }
            Self::TaskReopen { issue, .. } => {
                task_reopen(issue, None)?;
                format!("{issue}: reopened; its Graph unchanged\n")
            }
            Self::TaskAbandon { issue, .. } => {
                let identifier = crate::ops::task::task_abandon(repo, Some(issue), false)?;
                format!("{identifier}: canceled; PRs and branches removed\n")
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
            Self::TaskOwnership { issue, claimed, .. } => {
                use crate::ops::task_ownership::{change, OwnershipChange};
                let requested = if *claimed {
                    OwnershipChange::Claim
                } else {
                    OwnershipChange::Unassign
                };
                format!(
                    "{}\n",
                    serde_json::to_string(&change(repo, issue, requested)?)?
                )
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
            Self::SessionResume { directory, .. } | Self::SessionNew { directory, .. } => {
                pretty(&super::session::prepared(cli, directory)?)?
            }
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
            Self::NewChapter {
                name,
                plan,
                preview,
                ..
            } => pretty(&crate::ops::chapter::new_chapter(
                repo,
                name,
                plan.as_bytes(),
                None,
                *preview,
            )?)?,
            Self::GraphCustomize { name, .. } => {
                format!("{}\n", crate::graph::customize(name, repo)?.display())
            }
            Self::Portfolio { .. } => {
                let Commands::Portfolio { cmd: Some(cmd), .. } = command else {
                    unreachable!("a portfolio write names its change")
                };
                format!(
                    "{}\n",
                    serde_json::to_string(&crate::portfolio::change(cmd)?)?
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
                    super::desktop::save_arrangement(None, directory, cmd, &mut input.as_bytes())?;
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
        assert_eq!(
            write(r#"{"write":"portfolio","change":["remove","/r"]}"#),
            ["--cwd", "/", "portfolio", "remove", "/r", "--json"]
        );
        let chapter = serde_json::from_str::<Write>(
            r#"{"write":"new_chapter","directory":"/r","name":"October","plan":"{}","preview":true}"#,
        )
        .unwrap();
        assert_eq!(chapter.input(), Some("{}"));
        assert_eq!(chapter.argv().last().map(String::as_str), Some("--dry-run"));
        assert_eq!(
            write(
                r#"{"write":"session_resume","directory":"/r","id":null,"task":null,"wave":null,"agent":null}"#
            ),
            [
                "--cwd",
                "/r",
                "--repo",
                "/r",
                "session",
                "resume",
                "--prepare",
                "--json"
            ]
        );
        assert_eq!(
            write(r#"{"write":"session_new","directory":"/r","id":"session_1","agent":"codex"}"#),
            [
                "--cwd",
                "/r",
                "--agent",
                "codex",
                "session",
                "new",
                "session_1",
                "--prepare",
                "--json"
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
            Write::TaskAbandon {
                directory: directory.clone(),
                issue: "FIX-1".into(),
            },
            Write::NewChapter {
                directory: directory.clone(),
                name: "October".into(),
                plan: "{}".into(),
                preview: false,
            },
            Write::Portfolio {
                change: vec!["add".into(), "/tmp/repo".into()],
            },
            Write::TaskOwnership {
                directory: directory.clone(),
                issue: "FIX-1".into(),
                claimed: false,
            },
            Write::SessionResume {
                directory: directory.clone(),
                id: None,
                task: Some("FIX-1".into()),
                wave: None,
                agent: Some("codex".into()),
            },
            Write::SessionNew {
                directory: directory.clone(),
                id: "session_1".into(),
                agent: None,
            },
        ];
        // Arrangement, a customized Graph and a chapter preview record nothing.
        let arrangement = [
            Write::GraphCustomize {
                directory: directory.clone(),
                name: "code".into(),
            },
            Write::NewChapter {
                directory: directory.clone(),
                name: "October".into(),
                plan: "{}".into(),
                preview: true,
            },
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
