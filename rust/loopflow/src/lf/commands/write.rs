//! Writes submitted by native clients to the Loopflow server. [`Write::run`]
//! calls the shared functions the CLI handlers call and returns the value the
//! CLI prints with `--json`; the carrier owns its trace.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::task;

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
    /// `lf [-b] [-a <agent>] task run <issue> [--from <step>] --json`, started
    /// detached: the Graph process it started, or where the Task now waits.
    /// `approve` is `-b`: every step headless to `end`, approving Auto-Ship.
    #[serde(rename = "task.run")]
    TaskRun {
        directory: PathBuf,
        issue: String,
        from: Option<String>,
        agent: Option<String>,
        approve: bool,
    },
    /// `lf --task <issue> ship --json`, started detached: approves Auto-Ship.
    #[serde(rename = "task.ship")]
    TaskShip { directory: PathBuf, issue: String },
    /// `lf --task <issue> ship --stop --json`: withdraws the Task's Auto-Ship approval.
    #[serde(rename = "task.hold")]
    TaskHold { directory: PathBuf, issue: String },
    /// `lf task comment <issue> --steer <message> --json`: direction for the
    /// Task's runs, whoever writes it.
    #[serde(rename = "task.steer")]
    TaskSteer {
        directory: PathBuf,
        issue: String,
        message: String,
    },
    /// `lf task comment <issue> <message> --json`; an agent's is progress.
    #[serde(rename = "task.comment")]
    TaskComment {
        directory: PathBuf,
        issue: String,
        message: String,
    },
    /// `lf task interrupt <issue> --json`.
    #[serde(rename = "task.interrupt")]
    TaskInterrupt { directory: PathBuf, issue: String },
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
    /// `lf session answer <session> <text> [--submit] --json`: type into the
    /// Session's terminal through its owner, Enter after with `submit`.
    #[serde(rename = "session.answer")]
    SessionAnswer {
        session: crate::id::LfSessionId,
        text: String,
        submit: bool,
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
    Portfolio {
        change: crate::portfolio::PortfolioCommand,
    },
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
    /// Make this write on the caller's thread and return what it wrote: the
    /// value the CLI spelling prints with `--json`. CLI handlers call the same
    /// shared functions and print that value.
    pub fn run(&self) -> anyhow::Result<serde_json::Value> {
        if let Some(value) = self.routed()? {
            return Ok(value);
        }
        let directory = self.directory();
        let task_repo = |issue: &str| crate::ops::task::task_repository(directory, Some(issue));
        match self {
            Self::TaskFromSession { id, title, .. } => value(&crate::ops::task::task_create(
                &crate::ops::task::task_repository(directory, None)?,
                None,
                Some(title.clone()),
                None,
                crate::ops::task::TaskAdoptionOptions {
                    session: Some(id.clone()),
                    worktree: None,
                },
            )?),
            Self::TaskComplete { issue, .. } => value(&crate::ops::task::task_complete(
                &task_repo(issue)?,
                issue,
                None,
            )?),
            Self::TaskReopen { issue, .. } => value(&crate::ops::task::task_reopen(issue, None)?),
            Self::TaskNotes { issue, notes, .. } => value(&crate::ops::task::task_edit(
                &task_repo(issue)?,
                issue,
                None,
                super::mutation::task_edit_update(&None, &Some(notes.clone()), None, &None),
            )?),
            Self::TaskAbandon { issue, .. } => value(&crate::ops::task::task_abandon(
                &task_repo(issue)?,
                Some(issue),
                false,
            )?),
            Self::TaskClaim { issue, .. } => value(&task::ownership(
                directory,
                issue,
                crate::ops::task_ownership::OwnershipChange::Claim,
            )?),
            Self::TaskUnassign { issue, .. } => value(&task::ownership(
                directory,
                issue,
                crate::ops::task_ownership::OwnershipChange::Unassign,
            )?),
            Self::TaskRun {
                issue,
                from,
                agent,
                approve,
                ..
            } => value(&task::run(
                directory,
                issue,
                from.as_deref(),
                agent.as_deref(),
                *approve,
            )?),
            Self::TaskShip { issue, .. } => value(&task::ship(directory, issue)?),
            Self::TaskHold { issue, .. } => value(&task::hold(directory, issue, None)?),
            Self::TaskSteer { issue, message, .. } => {
                value(&task::comment(directory, issue, None, message, true)?)
            }
            Self::TaskComment { issue, message, .. } => {
                value(&task::comment(directory, issue, None, message, false)?)
            }
            Self::TaskInterrupt { issue, .. } => value(&task::interrupt(issue)?),
            Self::TaskMove { issue, wave, .. } => value(&crate::ops::task::task_move_membership(
                &task_repo(issue)?,
                issue,
                wave.as_deref(),
            )?),
            Self::TaskSave {
                issue,
                path,
                revision,
                content,
                ..
            } => {
                let content = super::mutation::read_task_draft(&mut content.as_bytes())?;
                value(&crate::ops::task::task_save(
                    issue, path, revision, &content,
                )?)
            }
            Self::SessionRename { id, name, .. } => value(&super::session::renamed(id, name)?),
            Self::SessionBind {
                id, task, preview, ..
            } => super::session::bound(id, task, *preview),
            Self::SessionAnswer {
                session,
                text,
                submit,
            } => value(&crate::engine::session::answer(session, text, *submit)?),
            Self::SessionResume { .. } | Self::SessionNew { .. } => {
                let (cli, _) = self
                    .cli()
                    .expect("a Session preparation has a CLI spelling")?;
                value(&super::session::prepared(&cli, directory)?)
            }
            Self::WaveAdd {
                wave, objective, ..
            } => {
                let repo = crate::repo::require_repo_root(directory, "this command")?;
                value(&tokio::runtime::Runtime::new()?.block_on(
                    crate::ops::wave_lifecycle::add(&repo, wave, objective.as_deref()),
                )?)
            }
            Self::PlanningRefresh { wave, .. } => {
                let refreshed = super::ops::refresh_status(directory, Some(wave))?;
                Ok(serde_json::from_str(&super::waves::status_json(
                    directory,
                    Some(refreshed.as_deref().unwrap_or(wave)),
                    super::waves::Shape::Tasks,
                )?)?)
            }
            Self::NewChapter {
                name,
                plan,
                preview,
                ..
            } => {
                let repo = crate::repo::require_repo_root(directory, "this command")?;
                value(&crate::ops::chapter::new_chapter(
                    &repo,
                    name,
                    plan.as_bytes(),
                    None,
                    *preview,
                )?)
            }
            Self::GraphCustomize { name, .. } => value(&super::graph::Customized {
                path: crate::graph::customize(
                    name,
                    &crate::repo::working_directory_in(directory)?,
                )?,
            }),
            Self::Portfolio { change } => value(&crate::portfolio::change(change, directory)?),
            Self::DashboardQuery { .. }
            | Self::DashboardPin { .. }
            | Self::DashboardUnpin { .. }
            | Self::DashboardRename { .. }
            | Self::DashboardSlots { .. }
            | Self::PaneLayout { .. } => {
                use crate::lf::DesktopCommand;
                let command = match self {
                    Self::DashboardQuery {
                        id,
                        expression,
                        revision,
                        ..
                    } => DesktopCommand::Query {
                        container: id.clone(),
                        expression: expression.clone(),
                        revision: *revision,
                    },
                    Self::DashboardPin { id, revision, .. } => DesktopCommand::Pin {
                        query: id.clone(),
                        revision: *revision,
                    },
                    Self::DashboardUnpin { id, revision, .. } => DesktopCommand::Unpin {
                        query: id.clone(),
                        revision: *revision,
                    },
                    Self::DashboardRename {
                        id, name, revision, ..
                    } => DesktopCommand::Rename {
                        target: format!("query:{id}"),
                        name: name.clone(),
                        revision: u64::try_from(*revision)?,
                    },
                    Self::DashboardSlots { id, revision, .. } => DesktopCommand::Layout {
                        work: format!("query:{id}"),
                        save: true,
                        revision: Some(*revision),
                    },
                    Self::PaneLayout { work, .. } => DesktopCommand::Layout {
                        work: work.clone(),
                        save: true,
                        revision: None,
                    },
                    _ => unreachable!("matched above"),
                };
                let mut input = self.input().unwrap_or_default().as_bytes();
                super::desktop::save_arrangement(None, directory, &command, &mut input)
            }
            Self::DashboardHide { id, member, .. } | Self::DashboardShow { id, member, .. } => {
                let verb = if matches!(self, Self::DashboardHide { .. }) {
                    "hide"
                } else {
                    "show"
                };
                super::desktop::save_tile(
                    None,
                    directory,
                    &format!("query:{id}"),
                    member,
                    verb,
                    &Default::default(),
                )
            }
            Self::TileMove {
                board,
                member,
                destination,
                ..
            } => super::desktop::save_tile(
                None,
                directory,
                board,
                member,
                "move",
                &[("destination".to_owned(), destination.clone())].into(),
            ),
        }
    }

    /// The Session writes whose Work can live on another Machine, and whose
    /// preparation reads the CLI's selectors, as `lf` spells them. SSH runs
    /// this spelling on the destination; nothing else parses it. Task writes
    /// route through [`crate::engine::task`].
    fn cli(&self) -> Option<anyhow::Result<(crate::lf::Cli, Vec<String>)>> {
        let mut args = vec![
            "lf".to_owned(),
            "--cwd".to_owned(),
            self.directory().display().to_string(),
        ];
        let words: Vec<&str> = match self {
            Self::SessionResume {
                id,
                task,
                wave,
                agent,
                ..
            } => {
                let mut words = Vec::new();
                if id.is_none() && task.is_none() && wave.is_none() {
                    args.extend(["--repo".to_owned(), self.directory().display().to_string()]);
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
            _ => return None,
        };
        args.extend(words.into_iter().map(str::to_owned));
        Some(
            <crate::lf::Cli as clap::Parser>::try_parse_from(&args)
                .and_then(crate::lf::Cli::checked)
                .map_err(anyhow::Error::from)
                .map(|cli| (cli, args)),
        )
    }

    /// What the destination printed, when this write's Work lives on another
    /// Machine: its `lf … --json` prints what its API returns.
    fn routed(&self) -> anyhow::Result<Option<serde_json::Value>> {
        let Some(spelled) = self.cli() else {
            return Ok(None);
        };
        let (cli, args) = spelled?;
        let Some(forward) = super::work_route::forward(&cli, &args, self.directory())? else {
            return Ok(None);
        };
        let (mut output, mut error) = (Vec::new(), Vec::new());
        let ran = forward.run(
            &cli,
            self.directory(),
            &mut super::mutation::CommandIo {
                input: &mut std::io::empty(),
                output: &mut output,
                error: &mut error,
            },
        );
        let diagnostics = String::from_utf8_lossy(&error);
        if let Err(failed) = ran {
            return Err(match diagnostics.trim() {
                "" => failed,
                said => failed.context(format!("the destination said: {said}")),
            });
        }
        for line in diagnostics.lines() {
            crate::journal::request::diagnostic(format_args!("{line}"));
        }
        Ok(Some(serde_json::from_slice(&output)?))
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
            | Self::TaskRun { directory, .. }
            | Self::TaskShip { directory, .. }
            | Self::TaskHold { directory, .. }
            | Self::TaskSteer { directory, .. }
            | Self::TaskComment { directory, .. }
            | Self::TaskInterrupt { directory, .. }
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
            Self::Portfolio { .. } | Self::SessionAnswer { .. } => Path::new("/"),
        }
    }
}

fn value(value: &impl Serialize) -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::to_value(value)?)
}

#[cfg(test)]
mod tests {
    use super::Write;

    /// The JSON `RegistryQueryTests.servedWriteEncodings` encodes in Swift.
    #[test]
    fn desktop_encodings_name_their_writes() {
        for json in [
            r#"{"kind":"task.move","directory":"/r","issue":"FIX-1","wave":null}"#,
            r#"{"kind":"desktop.dashboard.unpin","directory":"/r","id":"picked","revision":3}"#,
            r#"{"kind":"desktop.dashboard.query","directory":"/r","id":"picked","expression":"-state:done","revision":null}"#,
            r#"{"kind":"task.hold","directory":"/r","issue":"FIX-1"}"#,
            r#"{"kind":"desktop.portfolio.set","change":{"action":"remove","path":"/r"}}"#,
            r#"{"kind":"chapter.start","directory":"/r","name":"October","plan":"{}","preview":true}"#,
            r#"{"kind":"session.resume","directory":"/r","id":null,"task":null,"wave":null,"agent":null}"#,
            r#"{"kind":"session.new","directory":"/r","id":"session_1","agent":"codex"}"#,
            r#"{"kind":"task.save","directory":"/","issue":"FIX-1","path":"a.txt","revision":"r","content":"text"}"#,
            r#"{"kind":"task.run","directory":"/r","issue":"FIX-1","from":"implement","agent":"claude","approve":false}"#,
            r#"{"kind":"task.ship","directory":"/r","issue":"FIX-1"}"#,
            r#"{"kind":"task.steer","directory":"/r","issue":"FIX-1","message":"keep the API"}"#,
            r#"{"kind":"task.comment","directory":"/r","issue":"FIX-1","message":"landed"}"#,
            r#"{"kind":"task.interrupt","directory":"/r","issue":"FIX-1"}"#,
        ] {
            let write: Write = serde_json::from_str(json).unwrap();
            assert_eq!(
                serde_json::to_value(&write).unwrap(),
                serde_json::from_str::<serde_json::Value>(json).unwrap()
            );
        }
        let chapter: Write = serde_json::from_str(
            r#"{"kind":"chapter.start","directory":"/r","name":"October","plan":"{}","preview":true}"#,
        )
        .unwrap();
        assert_eq!(chapter.input(), Some("{}"));
    }

    #[test]
    fn routed_writes_spell_the_command_ssh_runs() {
        let spelled = |json: &str| {
            let (_, args) = serde_json::from_str::<Write>(json)
                .unwrap()
                .cli()
                .expect("a routed write")
                .unwrap();
            args
        };
        assert_eq!(
            spelled(
                r#"{"kind":"session.resume","directory":"/r","id":null,"task":null,"wave":null,"agent":null}"#
            ),
            [
                "lf",
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
            spelled(r#"{"kind":"session.new","directory":"/r","id":"session_1","agent":"codex"}"#),
            [
                "lf",
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
        for json in [
            r#"{"kind":"task.complete","directory":"/r","issue":"FIX-1"}"#,
            r#"{"kind":"task.hold","directory":"/r","issue":"FIX-1"}"#,
            r#"{"kind":"task.claim","directory":"/r","issue":"FIX-1"}"#,
        ] {
            assert!(serde_json::from_str::<Write>(json).unwrap().cli().is_none());
        }
    }
}
