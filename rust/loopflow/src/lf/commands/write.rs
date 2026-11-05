//! Commands submitted by native clients to the Loopflow server.
//! CLI and server share mutation dispatch; the carrier owns its I/O and trace.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A typed mutation with the same defaults and effects as its CLI spelling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// On the wire each write is a resource-named `kind` (`task.complete`);
/// `desktop.*` writes are Desktop's own arrangement state.
#[serde(tag = "kind", deny_unknown_fields)]
// Exhaustive on purpose: the equivalence test matches every variant.
pub enum Write {
    /// `lf task create --session <id> --title <title> --json`, without launching.
    #[serde(rename = "task.create")]
    TaskFromSession {
        directory: PathBuf,
        id: String,
        title: String,
    },
    /// `lf task complete <issue>`.
    #[serde(rename = "task.complete")]
    TaskComplete { directory: PathBuf, issue: String },
    /// `lf task reopen <issue>`.
    #[serde(rename = "task.reopen")]
    TaskReopen { directory: PathBuf, issue: String },
    /// `lf task edit <issue> --notes=<notes>`.
    #[serde(rename = "task.update")]
    TaskNotes {
        directory: PathBuf,
        issue: String,
        notes: String,
    },
    /// `lf task abandon <issue>`: cancels the Task and removes its PRs and
    /// branches.
    #[serde(rename = "task.abandon")]
    TaskAbandon { directory: PathBuf, issue: String },
    /// `lf task claim <issue> --json`.
    #[serde(rename = "task.claim")]
    TaskClaim { directory: PathBuf, issue: String },
    /// `lf task unassign <issue> --json`.
    #[serde(rename = "task.unassign")]
    TaskUnassign { directory: PathBuf, issue: String },
    /// `lf --task <issue> ship --stop`: withdraws the Task's Auto-Ship approval.
    #[serde(rename = "task.hold")]
    ShipStop { directory: PathBuf, issue: String },
    /// `lf task move <issue> --wave <wave>`, or `--no-wave` without one.
    #[serde(rename = "task.move")]
    TaskMove {
        directory: PathBuf,
        issue: String,
        wave: Option<String>,
    },
    /// `lf task save <issue> <path> --revision <revision> --json`, with
    /// `content` on stdin.
    #[serde(rename = "task.save")]
    TaskSave {
        directory: PathBuf,
        issue: String,
        path: String,
        revision: String,
        content: String,
    },
    /// `lf session rename --json -- <id> <name>`.
    #[serde(rename = "session.rename")]
    SessionRename {
        directory: PathBuf,
        id: String,
        name: String,
    },
    /// `lf session bind --json --task <task> -- <id>`; with `preview`,
    /// `--dry-run`: the Task a bind would assign, assigning nothing.
    #[serde(rename = "session.bind")]
    SessionBind {
        directory: PathBuf,
        id: String,
        task: String,
        preview: bool,
    },
    /// `lf [--agent <agent>] session resume [<id>] [--task <task>] [--wave <wave>]
    /// --prepare --json`, with `--repo <directory>` when it names none of them:
    /// the conversation and the command that opens it, no provider started.
    #[serde(rename = "session.resume")]
    SessionResume {
        directory: PathBuf,
        id: Option<String>,
        task: Option<String>,
        wave: Option<String>,
        agent: Option<String>,
    },
    /// `lf [--agent <agent>] session new <id> --prepare --json`: the
    /// conversation on a fresh provider thread, not yet started.
    #[serde(rename = "session.new")]
    SessionNew {
        directory: PathBuf,
        id: String,
        agent: Option<String>,
    },
    /// `lf wave add <wave> [--objective <objective>] --json`: the Wave, its
    /// authored files and first Project; an existing Wave is unchanged.
    #[serde(rename = "wave.create")]
    WaveAdd {
        directory: PathBuf,
        wave: String,
        objective: Option<String>,
    },
    /// `lf wave status <wave> --sync --json`: the planning refresh.
    #[serde(rename = "wave.sync")]
    PlanningRefresh { directory: PathBuf, wave: String },
    /// `lf wave new-chapter --all <name> --plan /dev/stdin --json`, with
    /// `plan` (JSON) on stdin; `--dry-run` when `preview`, which records nothing.
    #[serde(rename = "chapter.start")]
    NewChapter {
        directory: PathBuf,
        name: String,
        plan: String,
        preview: bool,
    },
    /// `lf graph customize <name>`: copies a builtin Graph into the repository,
    /// recording nothing.
    #[serde(rename = "graph.customize")]
    GraphCustomize { directory: PathBuf, name: String },
    /// `lf portfolio <change> --json`, run from `/`: which repositories the
    /// app shows.
    #[serde(rename = "desktop.portfolio.set")]
    Portfolio { change: Vec<String> },
    /// `lf desktop query [--revision <revision>] -- <id> <expression>`.
    #[serde(rename = "desktop.dashboard.query")]
    DashboardQuery {
        directory: PathBuf,
        id: String,
        expression: String,
        revision: Option<i64>,
    },
    /// `lf desktop pin <id> --revision <revision>`.
    #[serde(rename = "desktop.dashboard.pin")]
    DashboardPin {
        directory: PathBuf,
        id: String,
        revision: i64,
    },
    /// `lf desktop unpin <id> --revision <revision>`.
    #[serde(rename = "desktop.dashboard.unpin")]
    DashboardUnpin {
        directory: PathBuf,
        id: String,
        revision: i64,
    },
    /// `lf desktop rename --revision <revision> -- query:<id> <name>`.
    #[serde(rename = "desktop.dashboard.rename")]
    DashboardRename {
        directory: PathBuf,
        id: String,
        name: String,
        revision: i64,
    },
    /// `lf desktop hide -p query:<id>/<member>`.
    #[serde(rename = "desktop.dashboard.hide")]
    DashboardHide {
        directory: PathBuf,
        id: String,
        member: String,
    },
    /// `lf desktop show -p query:<id>/<member>`.
    #[serde(rename = "desktop.dashboard.show")]
    DashboardShow {
        directory: PathBuf,
        id: String,
        member: String,
    },
    /// `lf desktop layout query:<id> --save --revision <revision>`, with
    /// `slots` (JSON) on stdin.
    #[serde(rename = "desktop.dashboard.slots")]
    DashboardSlots {
        directory: PathBuf,
        id: String,
        revision: i64,
        slots: String,
    },
    /// `lf desktop move -p <board>/<member> --destination <destination>`: a
    /// Dashboard's tile (`query:<id>`) or the shared Task order (`working-tasks`).
    #[serde(rename = "desktop.dashboard.move")]
    TileMove {
        directory: PathBuf,
        board: String,
        member: String,
        destination: String,
    },
    /// `lf desktop layout <work> --save`, with `layout` (JSON) on stdin.
    #[serde(rename = "desktop.layout.save")]
    PaneLayout {
        directory: PathBuf,
        work: String,
        layout: String,
    },
}

impl Write {
    /// The `lf` command this write equals, without the binary.
    pub fn argv(&self) -> Vec<String> {
        let mut argv = vec!["--cwd".to_owned(), self.directory().display().to_string()];
        let (notes_flag, revision_flag, query, path);
        let words: Vec<&str> = match self {
            Self::TaskFromSession { id, title, .. } => {
                vec![
                    "task",
                    "create",
                    "--session",
                    id,
                    "--title",
                    title,
                    "--json",
                ]
            }
            Self::TaskComplete { issue, .. } => vec!["task", "complete", issue],
            Self::TaskReopen { issue, .. } => vec!["task", "reopen", issue],
            Self::TaskAbandon { issue, .. } => vec!["task", "abandon", issue],
            Self::TaskNotes { issue, notes, .. } => {
                notes_flag = format!("--notes={notes}");
                vec!["task", "edit", issue, &notes_flag]
            }
            Self::TaskClaim { issue, .. } => vec!["task", "claim", issue, "--json"],
            Self::TaskUnassign { issue, .. } => vec!["task", "unassign", issue, "--json"],
            Self::ShipStop { issue, .. } => vec!["--task", issue, "ship", "--stop"],
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
            Self::SessionBind {
                id, task, preview, ..
            } => {
                let mut words = vec!["session", "bind", "--json", "--task", task];
                if *preview {
                    words.push("--dry-run");
                }
                words.extend(["--", id]);
                words
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
            Self::WaveAdd {
                wave, objective, ..
            } => {
                let mut words = vec!["wave", "add", wave.as_str()];
                if let Some(objective) = objective {
                    words.extend(["--objective", objective]);
                }
                words.push("--json");
                words
            }
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
            Self::DashboardPin { id, revision, .. } | Self::DashboardUnpin { id, revision, .. } => {
                revision_flag = format!("--revision={revision}");
                let verb = if matches!(self, Self::DashboardPin { .. }) {
                    "pin"
                } else {
                    "unpin"
                };
                vec!["desktop", verb, &revision_flag, "--", id]
            }
            Self::DashboardRename {
                id, name, revision, ..
            } => {
                revision_flag = format!("--revision={revision}");
                query = format!("query:{id}");
                vec!["desktop", "rename", &revision_flag, "--", &query, name]
            }
            Self::DashboardHide { id, member, .. } | Self::DashboardShow { id, member, .. } => {
                path = format!("query:{id}/{member}");
                let verb = if matches!(self, Self::DashboardHide { .. }) {
                    "hide"
                } else {
                    "show"
                };
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

    pub(crate) fn directory(&self) -> &Path {
        match self {
            Self::TaskFromSession { directory, .. }
            | Self::TaskComplete { directory, .. }
            | Self::TaskReopen { directory, .. }
            | Self::TaskAbandon { directory, .. }
            | Self::TaskNotes { directory, .. }
            | Self::TaskClaim { directory, .. }
            | Self::TaskUnassign { directory, .. }
            | Self::ShipStop { directory, .. }
            | Self::TaskMove { directory, .. }
            | Self::TaskSave { directory, .. }
            | Self::SessionRename { directory, .. }
            | Self::SessionBind { directory, .. }
            | Self::SessionResume { directory, .. }
            | Self::SessionNew { directory, .. }
            | Self::WaveAdd { directory, .. }
            | Self::PlanningRefresh { directory, .. }
            | Self::NewChapter { directory, .. }
            | Self::GraphCustomize { directory, .. }
            | Self::DashboardQuery { directory, .. }
            | Self::DashboardPin { directory, .. }
            | Self::DashboardUnpin { directory, .. }
            | Self::DashboardRename { directory, .. }
            | Self::DashboardHide { directory, .. }
            | Self::DashboardShow { directory, .. }
            | Self::DashboardSlots { directory, .. }
            | Self::TileMove { directory, .. }
            | Self::PaneLayout { directory, .. } => directory,
            Self::Portfolio { .. } => Path::new("/"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Write;
    use crate::lf::Cli;
    use clap::Parser;
    use std::path::PathBuf;

    /// The JSON `RegistryQueryTests.servedWriteEncodings` encodes in Swift.
    #[test]
    fn desktop_encodings_name_their_writes() {
        let write = |json: &str| serde_json::from_str::<Write>(json).unwrap().argv();
        assert_eq!(
            write(r#"{"kind":"task.move","directory":"/r","issue":"FIX-1","wave":null}"#),
            ["--cwd", "/r", "task", "move", "FIX-1", "--no-wave"]
        );
        assert_eq!(
            write(
                r#"{"kind":"desktop.dashboard.unpin","directory":"/r","id":"picked","revision":3}"#
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
                r#"{"kind":"desktop.dashboard.query","directory":"/r","id":"picked","expression":"-state:done","revision":null}"#
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
            write(r#"{"kind":"task.hold","directory":"/r","issue":"FIX-1"}"#),
            ["--cwd", "/r", "--task", "FIX-1", "ship", "--stop"]
        );
        assert_eq!(
            write(r#"{"kind":"desktop.portfolio.set","change":["remove","/r"]}"#),
            ["--cwd", "/", "portfolio", "remove", "/r", "--json"]
        );
        let chapter = serde_json::from_str::<Write>(
            r#"{"kind":"chapter.start","directory":"/r","name":"October","plan":"{}","preview":true}"#,
        )
        .unwrap();
        assert_eq!(chapter.input(), Some("{}"));
        assert_eq!(chapter.argv().last().map(String::as_str), Some("--dry-run"));
        assert_eq!(
            write(
                r#"{"kind":"session.resume","directory":"/r","id":null,"task":null,"wave":null,"agent":null}"#
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
            write(r#"{"kind":"session.new","directory":"/r","id":"session_1","agent":"codex"}"#),
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
            r#"{"kind":"task.save","directory":"/","issue":"FIX-1","path":"a.txt","revision":"r","content":"text"}"#,
        )
        .unwrap();
        assert_eq!(save.input(), Some("text"));
    }

    #[test]
    fn every_write_parses_as_the_lf_command_it_names() {
        let directory = PathBuf::from("/tmp/repo");
        let writes = [
            Write::TaskFromSession {
                directory: directory.clone(),
                id: "session_1".into(),
                title: "Adopt work".into(),
            },
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
                preview: false,
            },
            Write::WaveAdd {
                directory: directory.clone(),
                wave: "product".into(),
                objective: None,
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
            Write::TaskUnassign {
                directory: directory.clone(),
                issue: "FIX-1".into(),
            },
            Write::ShipStop {
                directory: directory.clone(),
                issue: "FIX-1".into(),
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
            Write::DashboardUnpin {
                directory: directory.clone(),
                id: "picked".into(),
                revision: 3,
            },
            Write::DashboardRename {
                directory: directory.clone(),
                id: "picked".into(),
                name: "-n".into(),
                revision: 3,
            },
            Write::DashboardHide {
                directory: directory.clone(),
                id: "picked".into(),
                member: "task:task_1".into(),
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
