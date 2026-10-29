//! The reads an app serves inside its own process. Each [`Read`] is an `lf`
//! command: [`Read::argv`] names it, [`Read::read`] returns what it prints,
//! and for a command that takes `--watch`, [`Read::subscribe`] goes through the
//! same function as `lf <argv> --watch`, so the app pulls exactly the frames
//! that command prints.
//!
//! A read the command scopes by its working directory names that directory
//! instead: `--cwd <directory>` in its argv, and the same path in-process,
//! where every subscription shares one working directory.

use std::path::{Path, PathBuf};

use clap::{Args, FromArgMatches};
use serde::{Deserialize, Serialize};

use super::activity::HistoryWindow;
use super::graph_inventory;
use super::machine::{local_machine, machines};
use super::usage;
use super::watch::{Subscription, Transport};
use super::waves::{list as list_waves, Shape};
use super::work_route::repository_identity;
use crate::durable::RepositoryId;
use crate::lf::SessionListArgs;
use crate::ops::task::{task_changes, task_diff, task_file};

fn words(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| word.to_string()).collect()
}

/// `[flag, value]`, or nothing without a value.
fn optional(flag: &str, value: Option<impl ToString>) -> Vec<String> {
    value
        .map(|value| vec![flag.to_owned(), value.to_string()])
        .unwrap_or_default()
}

/// `command` as `lf --cwd <directory> …` runs it.
fn at(directory: &Path, command: &[&str]) -> Vec<String> {
    let mut argv = vec!["--cwd".to_owned(), directory.display().to_string()];
    argv.extend(words(command));
    argv
}

/// A read an app serves inside its own process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "read", rename_all = "snake_case", deny_unknown_fields)]
// Exhaustive on purpose: the equivalence test matches every variant.
pub enum Read {
    /// `lf monitor ps`: live Loopflow call trees.
    MonitorPs,
    /// `lf --cwd <directory> desktop plans`: the plans a repository window shows.
    DesktopPlans { directory: PathBuf },
    /// `lf --cwd <directory> desktop sessions`, with `--interactive all`
    /// when `headless` Sessions are shown too.
    DesktopSessions { directory: PathBuf, headless: bool },
    /// `lf --cwd <directory> desktop task <issue>`: the Task pane.
    DesktopTask { directory: PathBuf, issue: String },
    /// `lf task diff <issue> --files --base <base>`: a Task's changed files.
    TaskChanges { issue: String, base: String },
    /// `lf task diff <issue> [path] --base <base>`, or with `--draft` and the
    /// draft on stdin: one Task's patch.
    TaskDiff {
        issue: String,
        path: Option<String>,
        base: String,
        draft: Option<String>,
    },
    /// `lf task file <issue> <path>`: one file of a Task's checkout.
    TaskFile { issue: String, path: String },
    /// `lf --cwd <directory> config repository`: the repository's planning
    /// identity, registered on first sight.
    RepositoryIdentity { directory: PathBuf },
    /// `lf wave list --all --current`: every current Wave on this Machine.
    Waves,
    /// `lf machine id`: this Machine's identity.
    MachineId,
    /// `lf portfolio`: the repositories this Machine's windows show.
    Portfolio,
    /// `lf --cwd <directory> desktop layout <work>`: a Work's saved panes.
    PaneLayout { directory: PathBuf, work: String },
    /// `lf --cwd <directory> ops people`: who owns the repository's Tasks.
    People { directory: PathBuf },
    /// `lf --cwd <directory> graph list`: every Graph a Task there can take up.
    Graphs { directory: PathBuf },
    /// `lf --cwd <directory> desktop view [expression] --plan`: what a view
    /// shows; no expression is the repository's own.
    ViewPlan {
        directory: PathBuf,
        expression: Option<String>,
    },
    /// `lf --cwd <directory> graph list --processes --for-task <task>
    /// [--after <after>]`: one page of the Graphs that ran for a Task.
    GraphProcesses {
        directory: PathBuf,
        task: String,
        after: Option<String>,
    },
    /// `lf graph show <id> --processes`: one Graph that ran, with its steps.
    GraphProcess { id: String },
    /// `lf usage --days <days> [--wave|--project|--task …]`: the Session
    /// inputs recorded in that window, by Work.
    Usage {
        days: u32,
        wave: Option<String>,
        project: Option<String>,
        task: Option<String>,
    },
    /// `lf usage --days 0 --task <task> --context`: what each of those
    /// inputs was made of.
    TaskContext { task: String },
    /// `lf --cwd <directory> desktop wave <wave>`: one Wave's Projects and
    /// Tasks, after syncing its repository with the tracker.
    WaveDetail { directory: PathBuf, wave: String },
    /// `lf --cwd <directory> desktop done --wave <wave> [--after <after>]`:
    /// one page of a Wave's closed Tasks.
    ClosedTasks {
        directory: PathBuf,
        wave: String,
        after: Option<String>,
    },
    /// `lf machine list`: your registered Machines.
    Machines,
    /// `lf history`: what Sessions did, by Work.
    History(HistoryWindow),
    /// `lf --cwd <directory> desktop plan [wave] [--task <task>] [--all]`:
    /// Waves and Tasks. Read once, it first asks the tracker about each
    /// repository read; its watch reads the store.
    Roadmap {
        directory: PathBuf,
        wave: Option<String>,
        task: Option<String>,
        all: bool,
    },
    /// `lf --cwd <directory> config planning status`: where the repository's
    /// planning is exchanged. Its watch exchanges it while it runs.
    PlanningStatus { directory: PathBuf },
    /// `lf --cwd <directory> open <url> --link`: what a Work address opens.
    WorkLink { directory: PathBuf, url: String },
    /// `lf --repository <id> config repository --path`: a repository plan's
    /// checkout on this Machine.
    RepositoryPath { id: RepositoryId },
    /// `lf --cwd <directory> wave graph show <wave>|--no-wave`: the graph a
    /// new Task there would take up.
    WaveGraph {
        directory: PathBuf,
        wave: Option<String>,
    },
    /// `lf --cwd <directory> desktop sessions --page --limit 100 [--after …]`,
    /// with `--interactive all` when `headless` Sessions are shown too.
    SessionPage {
        directory: PathBuf,
        headless: bool,
        after: Option<String>,
    },
    /// `lf --cwd <directory> ops tokens [--days <days>]`: the codebase's size.
    Tokens {
        directory: PathBuf,
        days: Option<u32>,
    },
    /// `lf history retention`: how much Session history providers keep.
    Retention,
    /// `lf doctor`: the ledger's self-audit.
    Doctor,
    /// `lf session bind --dry-run --task <task> -- <session>`: the Task a
    /// bind would assign, without assigning it.
    BindPreview { session: String, task: String },
}

impl Read {
    /// The `lf` command that prints this read, `--json` included where the
    /// command takes it. Its watch is this argv with `--watch`.
    pub fn argv(&self) -> Vec<String> {
        match self {
            Self::MonitorPs => words(&["monitor", "ps", "--json"]),
            Self::DesktopPlans { directory } => at(directory, &["desktop", "plans", "--json"]),
            Self::DesktopSessions {
                directory,
                headless,
            } => {
                let mut argv = at(directory, &["desktop", "sessions"]);
                argv.extend(words(session_flags(*headless)));
                argv.push("--json".into());
                argv
            }
            Self::DesktopTask { directory, issue } => {
                at(directory, &["desktop", "task", issue, "--json"])
            }
            Self::TaskChanges { issue, base } => {
                words(&["task", "diff", issue, "--files", "--base", base, "--json"])
            }
            Self::TaskDiff {
                issue,
                path,
                base,
                draft,
            } => {
                let mut argv: Vec<String> = words(&["task", "diff", issue]);
                argv.extend(path.clone());
                argv.extend(["--base".into(), base.clone(), "--json".into()]);
                if draft.is_some() {
                    argv.push("--draft".into());
                }
                argv
            }
            Self::TaskFile { issue, path } => words(&["task", "file", issue, path, "--json"]),
            Self::RepositoryIdentity { directory } => {
                at(directory, &["config", "repository", "--json"])
            }
            Self::Waves => words(&["wave", "list", "--all", "--current", "--json"]),
            Self::MachineId => words(&["machine", "id", "--json"]),
            Self::Portfolio => words(&["portfolio", "--json"]),
            // `desktop layout` always prints JSON.
            Self::PaneLayout { directory, work } => at(directory, &["desktop", "layout", work]),
            Self::People { directory } => at(directory, &["ops", "people", "--json"]),
            Self::Graphs { directory } => at(directory, &["graph", "list", "--json"]),
            Self::ViewPlan {
                directory,
                expression,
            } => {
                let mut argv = at(directory, &["desktop", "view"]);
                argv.extend(expression.clone());
                argv.extend(words(&["--plan", "--json"]));
                argv
            }
            Self::GraphProcesses {
                directory,
                task,
                after,
            } => {
                let mut argv = at(
                    directory,
                    &["graph", "list", "--processes", "--for-task", task],
                );
                argv.extend(optional("--after", after.as_ref()));
                argv.push("--json".into());
                argv
            }
            Self::GraphProcess { id } => words(&["graph", "show", id, "--processes", "--json"]),
            Self::Usage {
                days,
                wave,
                project,
                task,
            } => {
                let mut argv = words(&["usage", "--days", &days.to_string()]);
                argv.extend(optional("--wave", wave.as_ref()));
                argv.extend(optional("--project", project.as_ref()));
                argv.extend(optional("--task", task.as_ref()));
                argv.push("--json".into());
                argv
            }
            Self::TaskContext { task } => words(&[
                "usage",
                "--days",
                "0",
                "--task",
                task,
                "--context",
                "--json",
            ]),
            Self::WaveDetail { directory, wave } => {
                at(directory, &["desktop", "wave", wave, "--json"])
            }
            Self::ClosedTasks {
                directory,
                wave,
                after,
            } => {
                let mut argv = at(directory, &["desktop", "done", "--wave", wave, "--json"]);
                argv.extend(optional("--after", after.as_ref()));
                argv
            }
            Self::Machines => words(&["machine", "list", "--json"]),
            Self::History(window) => {
                let mut argv = window.argv();
                argv.push("--json".into());
                argv
            }
            Self::Roadmap {
                directory,
                wave,
                task,
                all,
            } => {
                let mut argv = at(directory, &["desktop", "plan"]);
                argv.extend(wave.clone());
                argv.extend(optional("--task", task.as_ref()));
                if *all {
                    argv.push("--all".into());
                }
                argv.push("--json".into());
                argv
            }
            Self::PlanningStatus { directory } => {
                at(directory, &["config", "planning", "status", "--json"])
            }
            Self::WorkLink { directory, url } => at(directory, &["open", url, "--link", "--json"]),
            Self::RepositoryPath { id } => words(&[
                "--repository",
                &id.to_string(),
                "config",
                "repository",
                "--path",
            ]),
            Self::WaveGraph { directory, wave } => {
                let mut argv = at(directory, &["wave", "graph", "show"]);
                argv.push(wave.clone().unwrap_or_else(|| "--no-wave".into()));
                argv.push("--json".into());
                argv
            }
            Self::SessionPage {
                directory,
                headless,
                after,
            } => {
                let mut argv = at(directory, &["desktop", "sessions"]);
                argv.extend(page_flags(*headless, after.as_deref()));
                argv
            }
            Self::Tokens { directory, days } => {
                let mut argv = at(directory, &["ops", "tokens", "--json"]);
                argv.extend(optional("--days", *days));
                argv
            }
            Self::Retention => words(&["history", "retention", "--json"]),
            Self::Doctor => words(&["doctor", "--json"]),
            Self::BindPreview { session, task } => words(&[
                "session",
                "bind",
                "--dry-run",
                "--json",
                "--task",
                task,
                "--",
                session,
            ]),
        }
    }

    /// What the command reads from stdin.
    pub fn input(&self) -> Option<&str> {
        match self {
            Self::TaskDiff { draft, .. } => draft.as_deref(),
            _ => None,
        }
    }

    /// What `lf <argv>` prints, read on the caller's thread.
    pub fn read(&self) -> anyhow::Result<String> {
        match self {
            Self::MonitorPs => pretty(&super::top::load_snapshot()?),
            Self::DesktopPlans { directory } => {
                pretty(&super::desktop::read_plans(None, directory)?)
            }
            Self::DesktopSessions {
                directory,
                headless,
            } => {
                let filter = super::session::list_filter(&session_args(*headless)?, directory)?;
                pretty(&super::session::read_once(&filter, Shape::Desktop)?)
            }
            Self::DesktopTask { directory, issue } => {
                pretty(&super::desktop::read_task_pane(directory, issue)?)
            }
            Self::TaskChanges { issue, base } => pretty(&task_changes(issue, base)?),
            Self::TaskDiff {
                issue,
                path,
                base,
                draft,
            } => pretty(&task_diff(issue, path.as_deref(), base, draft.as_deref())?),
            Self::TaskFile { issue, path } => pretty(&task_file(issue, path, false)?),
            Self::RepositoryIdentity { directory } => {
                let (_, identity) = repository_identity(directory, None, None)?;
                pretty(&identity)
            }
            Self::Waves => {
                let waves = block_on(list_waves(true, true))??;
                pretty(&waves.unwrap_or_default())
            }
            Self::MachineId => pretty(&block_on(local_machine())??),
            Self::Portfolio => pretty(&crate::portfolio::read()?),
            Self::PaneLayout { directory, work } => {
                super::desktop::saved_layout(None, directory, work)
            }
            Self::People { directory } => pretty(&super::people::people(directory)?.0),
            Self::Graphs { directory } => pretty(&crate::graph::compiled::graph_catalog(
                &crate::repo::working_directory_in(directory)?,
            )?),
            Self::ViewPlan {
                directory,
                expression,
            } => {
                let plan = super::desktop::plan_view(None, directory, expression.as_deref())?;
                // The command prints this plan, then exits 3.
                if plan.resolution.members.is_empty() && !plan.resolution.unavailable.is_empty() {
                    anyhow::bail!("{}", plan.resolution.unavailable.join("; "));
                }
                pretty(&plan)
            }
            Self::GraphProcesses {
                directory,
                task,
                after,
            } => pretty(&graph_inventory::page(
                directory,
                &graph_inventory::GraphProcessInventoryArgs {
                    processes: true,
                    for_task: Some(task.clone()),
                    after: after.clone(),
                    ..Default::default()
                },
            )?),
            Self::GraphProcess { id } => pretty(&graph_inventory::detail(id)?),
            Self::Usage {
                days,
                wave,
                project,
                task,
            } => pretty(&usage::sessions(
                *days,
                super::WorkFilter {
                    wave: wave.as_deref(),
                    project: project.as_deref(),
                    task: task.as_deref(),
                },
                None,
            )?),
            Self::TaskContext { task } => {
                let filter = super::WorkFilter {
                    wave: None,
                    project: None,
                    task: Some(task),
                };
                pretty(&usage::context_report(&usage::sessions(0, filter, None)?)?)
            }
            Self::WaveDetail { directory, wave } => {
                pretty(&super::waves::desktop_status(directory, wave)?)
            }
            Self::ClosedTasks {
                directory,
                wave,
                after,
            } => pretty(&super::waves::desktop_closed_tasks(
                directory,
                wave,
                after.as_deref(),
            )?),
            Self::Machines => pretty(&block_on(machines())??),
            Self::History(window) => pretty(&super::activity::snapshot(window)?),
            Self::Roadmap {
                directory,
                wave,
                task,
                all,
            } => super::waves::show_json(
                directory,
                wave.as_deref(),
                task.as_deref(),
                *all,
                Shape::Desktop,
            ),
            Self::PlanningStatus { directory } => super::planning::status_json(directory),
            Self::WorkLink { directory, url } => pretty(&super::open::url_links(directory, url)?),
            // The command prints the path alone.
            Self::RepositoryPath { id } => {
                Ok(
                    repository_identity(&super::work_route::repository_path(id)?, None, None)?
                        .0
                        .to_string(),
                )
            }
            Self::WaveGraph { directory, wave } => {
                let repo = crate::repo::working_directory_in(directory)?;
                pretty(&block_on(crate::ops::project::wave_graph(
                    &repo,
                    wave.as_deref(),
                    wave.is_some(),
                ))??)
            }
            Self::SessionPage {
                directory,
                headless,
                after,
            } => {
                let args = parse_session_args(page_flags(*headless, after.as_deref()))?;
                let filter = super::session::list_filter(&args, directory)?;
                pretty(&super::session::read_once(&filter, Shape::Desktop)?)
            }
            Self::Tokens { directory, days } => super::tokens::codebase(directory, *days),
            Self::Retention => pretty(
                &super::history_transcript::retention_summary()?
                    .ok_or_else(|| anyhow::anyhow!("No Sessions recorded."))?,
            ),
            Self::Doctor => pretty(&super::doctor::report()?),
            Self::BindPreview { session, task } => {
                pretty(&super::session::binding_preview(session, task)?)
            }
        }
    }

    /// Serve this read's watch inside this process, as `lf <argv> --watch`
    /// prints it. `wake` is called from any thread when a
    /// [`Subscription::pull`] may find a frame; dropping the subscription ends
    /// it. A read whose command takes no `--watch` is an error.
    pub fn subscribe(
        self,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> anyhow::Result<Subscription> {
        let transport = Transport::Pull(Box::new(wake));
        let subscription = match self {
            Self::MonitorPs => super::top::watch_ps(transport),
            Self::DesktopPlans { directory } => {
                super::desktop::watch_plans(None, directory, transport)
            }
            Self::DesktopSessions {
                directory,
                headless,
            } => {
                let filter = super::session::list_filter(&session_args(headless)?, &directory)?;
                super::session::watch_list(filter, Shape::Desktop, transport)
            }
            Self::DesktopTask { directory, issue } => {
                super::desktop::watch_task_pane(&directory, &issue, transport)
            }
            Self::History(window) => super::activity::watch(window, transport),
            Self::Roadmap {
                directory,
                wave,
                task,
                all,
            } => super::waves::watch_show(directory, wave, task, all, Shape::Desktop, transport),
            Self::PlanningStatus { directory } => super::planning::watch_status(
                &super::planning::status_repository(&directory)?,
                transport,
            ),
            other => anyhow::bail!("`lf {}` has no --watch", other.argv().join(" ")),
        }?;
        Ok(subscription.expect("a pulled watch returns its subscription"))
    }
}

fn session_flags(headless: bool) -> &'static [&'static str] {
    if headless {
        &["--interactive", "all"]
    } else {
        &[]
    }
}

/// `lf desktop sessions`' arguments as its flags parse, defaults included.
fn session_args(headless: bool) -> anyhow::Result<SessionListArgs> {
    parse_session_args(session_flags(headless).iter().map(|flag| flag.to_string()))
}

fn parse_session_args(flags: impl IntoIterator<Item = String>) -> anyhow::Result<SessionListArgs> {
    let command = SessionListArgs::augment_args(clap::Command::new("sessions"));
    let flags = std::iter::once("sessions".to_owned()).chain(flags);
    Ok(SessionListArgs::from_arg_matches(
        &command.try_get_matches_from(flags)?,
    )?)
}

/// The flags of one page of Desktop's Session inventory.
fn page_flags(headless: bool, after: Option<&str>) -> Vec<String> {
    let mut flags = words(&["--json", "--page"]);
    flags.extend(words(session_flags(headless)));
    flags.extend(words(&["--limit", "100"]));
    flags.extend(optional("--after", after));
    flags
}

/// `future` run to completion on a runtime of its own, as its command does.
fn block_on<F: std::future::Future>(future: F) -> anyhow::Result<F::Output> {
    Ok(tokio::runtime::Runtime::new()?.block_on(future))
}

fn pretty(value: &impl Serialize) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(value)?)
}

#[cfg(test)]
mod tests {
    use super::Read;

    /// The JSON Desktop's `ServedRead` encodes.
    #[test]
    fn desktop_encodings_name_their_reads() {
        let read = |json: &str| serde_json::from_str::<Read>(json).unwrap().argv();
        assert_eq!(
            read(
                r#"{"read":"history","since":"7d","limit":50,"wave":null,"project":"p","task":null}"#
            ),
            [
                "history",
                "--since",
                "7d",
                "--limit",
                "50",
                "--project",
                "p",
                "--json"
            ]
        );
        assert_eq!(
            read(r#"{"read":"desktop_sessions","directory":"/r","headless":true}"#),
            [
                "--cwd",
                "/r",
                "desktop",
                "sessions",
                "--interactive",
                "all",
                "--json"
            ]
        );
        assert_eq!(
            read(
                r#"{"read":"task_diff","issue":"FIX-1","path":null,"base":"parent","draft":null}"#
            ),
            ["task", "diff", "FIX-1", "--base", "parent", "--json"]
        );
        assert_eq!(
            read(r#"{"read":"view_plan","directory":"/r","expression":"state:backlog"}"#),
            [
                "--cwd",
                "/r",
                "desktop",
                "view",
                "state:backlog",
                "--plan",
                "--json"
            ]
        );
    }

    #[test]
    fn a_read_without_a_watch_cannot_be_subscribed() {
        let error = Read::Doctor.subscribe(|| {}).unwrap_err();
        assert!(error.to_string().contains("--watch"), "{error}");
    }
}
