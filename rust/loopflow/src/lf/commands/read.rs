//! Shared reads served by the client API server. Each [`Read`] is an `lf`
//! command: [`Read::argv`] names it, [`Read::read`] returns what it prints,
//! and for a command that takes `--watch`, [`Read::subscribe`] goes through the
//! same function as `lf <argv> --watch`, so clients receive exactly the frames
//! that command prints.
//!
//! A read the command scopes by its working directory names that directory
//! instead: `--cwd <directory>` in its argv, and an explicit path in the server,
//! where concurrent subscriptions cannot change the working directory.
//!
//! Reads only read: they record no Process and change no Work. The app's
//! writes are [`super::write::Write`]s.

use std::path::{Path, PathBuf};

use clap::{Args, FromArgMatches};
use serde::{Deserialize, Serialize};

use super::activity::HistoryWindow;
use super::graph_inventory;
use super::machine::{local_machine, machines};
use super::usage;
use super::watch::{Subscription, Transport};
use super::waves::{list as list_waves, Shape};
use crate::durable::RepositoryId;
use crate::lf::SessionListArgs;
use crate::ops::task::{task_changes, task_diff, task_file};
use crate::session::SessionFilter;

fn words(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| word.to_string()).collect()
}

/// `[flag, value]`, or nothing without a value.
fn optional(flag: &str, value: Option<impl ToString>) -> Vec<String> {
    value
        .map(|value| vec![flag.to_owned(), value.to_string()])
        .unwrap_or_default()
}

/// `command` as `lf` spells it for `destination`: the flags
/// `Destination::from_cli` reads back.
pub(crate) fn destination_argv(
    destination: &crate::config::files::Destination,
    command: &[&str],
) -> Vec<String> {
    use crate::config::files::Destination;
    let (repository, wave, flag) = match destination {
        Destination::PersonalGlobal {} => (None, None, Some("--global")),
        Destination::PersonalRepository { repository } => (Some(repository), None, None),
        Destination::SharedRepository { repository } => (Some(repository), None, Some("--shared")),
        Destination::PersonalWave { repository, wave } => (Some(repository), Some(wave), None),
        Destination::SharedWave { repository, wave } => {
            (Some(repository), Some(wave), Some("--shared"))
        }
    };
    let mut argv = match repository {
        Some(repository) => at(repository, command),
        None => words(command),
    };
    if let Some(wave) = wave {
        argv.splice(0..0, ["--wave".to_owned(), wave.clone()]);
    }
    argv.extend(flag.map(str::to_owned));
    argv
}

/// `command` as `lf --cwd <directory> …` runs it.
fn at(directory: &Path, command: &[&str]) -> Vec<String> {
    let mut argv = vec!["--cwd".to_owned(), directory.display().to_string()];
    argv.extend(words(command));
    argv
}

/// A read an app serves inside its own process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// On the wire each read is a resource-named `kind` (`task.get`); `desktop.*`
/// reads are Desktop's own arrangement state.
#[serde(tag = "kind", deny_unknown_fields)]
// Exhaustive on purpose: the equivalence test matches every variant.
pub enum Read {
    /// `lf monitor ps`: live Loopflow call trees.
    #[serde(rename = "process.list")]
    MonitorPs,
    /// `lf --cwd <directory> desktop plans`: the plans a repository window shows.
    #[serde(rename = "desktop.view.list")]
    DesktopPlans { directory: PathBuf },
    /// `lf --cwd <directory> desktop sessions`, with `--interactive all`
    /// when `headless` Sessions are shown too: every row, or with a `limit`
    /// one page (`--page --limit <limit> [--after <after>]`). Only the whole
    /// list has a watch.
    #[serde(rename = "session.list")]
    SessionList {
        directory: PathBuf,
        headless: bool,
        after: Option<String>,
        limit: Option<u32>,
    },
    /// `lf --cwd <directory> desktop task <issue>`: the Task pane.
    #[serde(rename = "task.get")]
    DesktopTask { directory: PathBuf, issue: String },
    /// `lf task diff <issue> --files --base <base>`: a Task's changed files.
    #[serde(rename = "task.changes")]
    TaskChanges { issue: String, base: String },
    /// `lf task diff <issue> [path] --base <base>`, or with `--draft` and the
    /// draft on stdin: one Task's patch.
    #[serde(rename = "task.diff")]
    TaskDiff {
        issue: String,
        path: Option<String>,
        base: String,
        draft: Option<String>,
    },
    /// `lf task file <issue> <path>`: one file of a Task's checkout.
    #[serde(rename = "task.file")]
    TaskFile { issue: String, path: String },
    /// `lf wave list --all`: every Wave on this Machine that is not removed.
    #[serde(rename = "wave.list")]
    Waves,
    /// `lf machine id`: this Machine's identity.
    #[serde(rename = "machine.self")]
    MachineId,
    /// `lf portfolio`: the repositories this Machine's windows show.
    #[serde(rename = "desktop.portfolio")]
    Portfolio,
    /// `lf config catalog --json`: every typed setting and its policies.
    #[serde(rename = "config.catalog")]
    ConfigCatalog,
    /// `lf --cwd <directory> [--wave <wave>] config settings [--skill|--graph
    /// [--step]] --json`: effective settings and their sources.
    #[serde(rename = "config.settings")]
    Settings {
        directory: PathBuf,
        wave: Option<String>,
        skill: Option<String>,
        graph: Option<String>,
        step: Option<String>,
    },
    /// `lf config get <key> <destination flags> --json`: one file's declaration.
    #[serde(rename = "config.get")]
    ConfigGet {
        destination: crate::config::files::Destination,
        key: String,
    },
    /// `lf config explain <key> <destination flags> --json`: every source's
    /// candidate and the value a launch there resolves.
    #[serde(rename = "config.explain")]
    ConfigExplain {
        destination: crate::config::files::Destination,
        key: String,
    },
    /// `lf --cwd <directory> desktop layout <work>`: a Work's saved panes.
    #[serde(rename = "desktop.layout")]
    PaneLayout { directory: PathBuf, work: String },
    /// `lf --cwd <directory> ops people`: who owns the repository's Tasks.
    #[serde(rename = "repository.people")]
    People { directory: PathBuf },
    /// `lf --cwd <directory> graph list`: every Graph a Task there can take up.
    #[serde(rename = "graph.list")]
    Graphs { directory: PathBuf },
    /// `lf --cwd <directory> desktop view [expression] --plan`: what a view
    /// shows; no expression is the repository's own.
    #[serde(rename = "desktop.view.get")]
    ViewPlan {
        directory: PathBuf,
        expression: Option<String>,
    },
    /// `lf --cwd <directory> graph list --processes --for-task <task>
    /// [--after <after>]`: one page of the Graphs that ran for a Task.
    #[serde(rename = "task.runs")]
    GraphProcesses {
        directory: PathBuf,
        task: String,
        after: Option<String>,
    },
    /// `lf graph show <id> --processes`: one Graph that ran, with its steps.
    #[serde(rename = "graph.run")]
    GraphProcess { id: String },
    /// `lf usage --days <days> [--wave|--project|--task …]`: the Session
    /// inputs recorded in that window, by Work.
    #[serde(rename = "session.usage")]
    Usage {
        days: u32,
        wave: Option<String>,
        project: Option<String>,
        task: Option<String>,
    },
    /// `lf usage --days 0 --task <task> --context`: what each of those
    /// inputs was made of.
    #[serde(rename = "task.context")]
    TaskContext { task: String },
    /// `lf --cwd <directory> desktop wave <wave>`: one Wave's Projects and
    /// Tasks, after syncing its repository with the tracker.
    #[serde(rename = "wave.planning")]
    WaveDetail { directory: PathBuf, wave: String },
    /// `lf --cwd <directory> desktop done --wave <wave> [--after <after>]`:
    /// one page of a Wave's closed Tasks. Only `closed` lists exist; open
    /// Tasks are `planning.list`'s.
    #[serde(rename = "task.list")]
    ClosedTasks {
        directory: PathBuf,
        wave: String,
        after: Option<String>,
        closed: bool,
    },
    /// `lf machine list`: your registered Machines.
    #[serde(rename = "machine.list")]
    Machines,
    /// `lf history`: what Sessions did, by Work.
    #[serde(rename = "session.history")]
    History(HistoryWindow),
    /// `lf --cwd <directory> desktop plan [wave] [--task <task>] [--all]`:
    /// Waves and Tasks. Read once, it first asks the tracker about each
    /// repository read; its watch reads the store.
    #[serde(rename = "planning.list")]
    Roadmap {
        directory: PathBuf,
        wave: Option<String>,
        task: Option<String>,
        all: bool,
    },
    /// `lf --cwd <directory> config planning status`: where the repository's
    /// planning is exchanged. Its watch exchanges it while it runs.
    #[serde(rename = "repository.planning")]
    PlanningStatus { directory: PathBuf },
    /// `lf --cwd <directory> open <url> --link`: what a Work address opens.
    #[serde(rename = "work.resolve")]
    WorkLink { directory: PathBuf, url: String },
    /// `lf --repository <id> config repository --path`: a repository plan's
    /// checkout on this Machine.
    #[serde(rename = "repository.path")]
    RepositoryPath { id: RepositoryId },
    /// `lf --cwd <directory> wave graph show <wave>|--no-wave`: the graph a
    /// new Task there would take up.
    #[serde(rename = "wave.graph")]
    WaveGraph {
        directory: PathBuf,
        wave: Option<String>,
    },
    /// `lf --cwd <directory> ops tokens [--days <days>]`: the codebase's size.
    #[serde(rename = "repository.tokens")]
    Tokens {
        directory: PathBuf,
        days: Option<u32>,
    },
    /// `lf history retention`: how much Session history providers keep.
    #[serde(rename = "session.retention")]
    Retention,
    /// `lf doctor`: the ledger's self-audit.
    #[serde(rename = "machine.doctor")]
    Doctor,
    /// `lf task wait <issue> --until <until> --timeout 0s --json`: the Task now
    /// and whether it reached `until`. Subscribed, it reads on each change and
    /// ends after the reading that reached it.
    #[serde(rename = "task.wait")]
    TaskWait {
        issue: String,
        until: crate::ops::task::TaskWaitUntil,
    },
}

impl Read {
    /// The `lf` command that prints this read, `--json` included where the
    /// command takes it. Its watch is this argv with `--watch`.
    pub fn argv(&self) -> Vec<String> {
        match self {
            Self::MonitorPs => words(&["monitor", "ps", "--json"]),
            Self::DesktopPlans { directory } => at(directory, &["desktop", "plans", "--json"]),
            Self::SessionList {
                directory,
                headless,
                after,
                limit,
            } => {
                let mut argv = at(directory, &["desktop", "sessions"]);
                argv.extend(words(session_flags(*headless)));
                argv.push("--json".into());
                if let Some(limit) = limit {
                    argv.extend(words(&["--page", "--limit", &limit.to_string()]));
                    argv.extend(optional("--after", after.as_ref()));
                }
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
            Self::Waves => words(&["wave", "list", "--all", "--json"]),
            Self::MachineId => words(&["machine", "id", "--json"]),
            Self::Portfolio => words(&["portfolio", "--json"]),
            Self::ConfigCatalog => words(&["config", "catalog", "--json"]),
            Self::ConfigGet { destination, key } => {
                destination_argv(destination, &["config", "get", key, "--json"])
            }
            Self::ConfigExplain { destination, key } => {
                destination_argv(destination, &["config", "explain", key, "--json"])
            }
            Self::Settings {
                directory,
                wave,
                skill,
                graph,
                step,
            } => {
                let mut argv = at(directory, &["config", "show", "--json"]);
                argv.extend(optional("--wave", wave.as_ref()));
                argv.extend(optional("--skill", skill.as_ref()));
                argv.extend(optional("--graph", graph.as_ref()));
                argv.extend(optional("--step", step.as_ref()));
                argv
            }
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
                ..
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
            Self::Tokens { directory, days } => {
                let mut argv = at(directory, &["ops", "tokens", "--json"]);
                argv.extend(optional("--days", *days));
                argv
            }
            Self::Retention => words(&["history", "retention", "--json"]),
            Self::Doctor => words(&["doctor", "--json"]),
            Self::TaskWait { issue, until } => {
                let until = match until {
                    crate::ops::task::TaskWaitUntil::Open => "submitted",
                    crate::ops::task::TaskWaitUntil::Terminal => "terminal",
                };
                words(&[
                    "task",
                    "wait",
                    issue,
                    "--until",
                    until,
                    "--timeout",
                    "0s",
                    "--json",
                ])
            }
        }
    }

    /// What the command reads from stdin.
    pub fn input(&self) -> Option<&str> {
        match self {
            Self::TaskDiff { draft, .. } => draft.as_deref(),
            _ => None,
        }
    }

    /// What `lf <argv>` prints, as a JSON value, read on the caller's thread.
    /// A command that prints bare text (`config repository --path`) is a string.
    pub fn read(&self) -> anyhow::Result<serde_json::Value> {
        match self {
            Self::MonitorPs => value(&super::top::load_snapshot()?),
            Self::DesktopPlans { directory } => {
                value(&super::desktop::read_plans(None, directory)?)
            }
            Self::SessionList {
                directory,
                after,
                limit,
                ..
            } => {
                if after.is_some() && limit.is_none() {
                    return Err(invalid("`after` pages a list: name its `limit` too").into());
                }
                value(&super::session::read_once(
                    &self.session_filter(directory)?,
                    Shape::Desktop,
                )?)
            }
            Self::DesktopTask { directory, issue } => {
                value(&super::desktop::read_task_pane(directory, issue)?)
            }
            Self::TaskChanges { issue, base } => value(&task_changes(issue, base)?),
            Self::TaskDiff {
                issue,
                path,
                base,
                draft,
            } => value(&task_diff(issue, path.as_deref(), base, draft.as_deref())?),
            Self::TaskFile { issue, path } => value(&task_file(issue, path, false)?),
            Self::Waves => {
                let waves = block_on(list_waves(true))??;
                value(&waves.unwrap_or_default())
            }
            Self::MachineId => value(&block_on(local_machine())??),
            Self::Portfolio => value(&crate::portfolio::read()?),
            Self::Settings {
                directory,
                wave,
                skill,
                graph,
                step,
            } => value(&crate::engine::config::settings(
                directory,
                wave.as_deref(),
                skill.as_deref(),
                graph.as_deref(),
                step.as_deref(),
            )?),
            Self::ConfigCatalog => value(&crate::engine::config::catalog()),
            Self::ConfigGet { destination, key } => {
                value(&crate::engine::config::get(destination, key)?)
            }
            Self::ConfigExplain { destination, key } => {
                value(&crate::engine::config::explain(destination, key)?)
            }
            Self::PaneLayout { directory, work } => {
                super::desktop::saved_layout(None, directory, work)
            }
            Self::People { directory } => value(&super::people::people(directory)?.0),
            Self::Graphs { directory } => value(&crate::graph::compiled::graph_catalog(
                &crate::repo::working_directory_in(directory)?,
            )?),
            Self::ViewPlan {
                directory,
                expression,
            } => {
                let plan = super::desktop::plan_view(None, directory, expression.as_deref())?;
                // The command prints this plan, then exits 3.
                if plan.resolution.members.is_empty() && !plan.resolution.unavailable.is_empty() {
                    return Err(not_found(
                        plan.resolution.unavailable.join("; "),
                        serde_json::json!({ "expression": expression }),
                    )
                    .into());
                }
                value(&plan)
            }
            Self::GraphProcesses {
                directory,
                task,
                after,
            } => value(&graph_inventory::page(
                directory,
                &graph_inventory::GraphProcessInventoryArgs {
                    processes: true,
                    for_task: Some(task.clone()),
                    after: after.clone(),
                    ..Default::default()
                },
            )?),
            Self::GraphProcess { id } => value(&graph_inventory::detail(id)?),
            Self::Usage {
                days,
                wave,
                project,
                task,
            } => value(&usage::sessions(
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
                value(&usage::context_report(&usage::sessions(0, filter, None)?)?)
            }
            Self::WaveDetail { directory, wave } => {
                value(&super::waves::desktop_status(directory, wave)?)
            }
            Self::ClosedTasks { closed: false, .. } => Err(invalid(
                "only closed Tasks are listed; read planning.list for open ones",
            )
            .into()),
            Self::ClosedTasks {
                directory,
                wave,
                after,
                closed: true,
            } => value(&super::waves::desktop_closed_tasks(
                directory,
                wave,
                after.as_deref(),
            )?),
            Self::Machines => value(&block_on(machines())??),
            Self::History(window) => value(&super::activity::snapshot(window)?),
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
            Self::WorkLink { directory, url } => value(&super::open::url_links(directory, url)?),
            // The command prints the path alone.
            Self::RepositoryPath { id } => {
                let checkout = super::work_route::repository_path(id)?;
                let repo =
                    crate::repository::CanonicalRepo::containing(&checkout)?.ok_or_else(|| {
                        not_found(
                            format!("repository plan {id} has no local checkout"),
                            serde_json::json!({ "repository": id }),
                        )
                    })?;
                Ok(repo.to_string().into())
            }
            Self::WaveGraph { directory, wave } => {
                let repo = crate::repo::working_directory_in(directory)?;
                value(&block_on(crate::ops::project::wave_graph(
                    &repo,
                    wave.as_deref(),
                    wave.is_some(),
                ))??)
            }
            Self::Tokens { directory, days } => super::tokens::codebase(directory, *days),
            Self::Retention => value(&super::history_transcript::retention_summary()?.ok_or_else(
                || {
                    not_found(
                        "No Sessions recorded.",
                        serde_json::json!({ "resource": "sessions" }),
                    )
                },
            )?),
            Self::Doctor => value(&super::doctor::report()?),
            Self::TaskWait { issue, until } => value(&crate::engine::task::waited(issue, *until)?),
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
            Self::SessionList {
                ref directory,
                limit: None,
                after: None,
                ..
            } => super::session::watch_list(
                self.session_filter(directory)?,
                Shape::Desktop,
                transport,
            ),
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
            Self::TaskWait { issue, until } => {
                crate::engine::task::watch_wait(issue, until, transport)
            }
            other => {
                return Err(crate::engine::Error::new(
                    crate::engine::ErrorCode::Invalid,
                    format!("`lf {}` has no --watch", other.argv().join(" ")),
                )
                .into())
            }
        }?;
        Ok(subscription.expect("a pulled watch returns its subscription"))
    }

    /// Whether a subscription to this read ends after `body`, a reading's
    /// body: `task.wait` ends once the Task reached what it waits for.
    pub fn ends_after(&self, body: &serde_json::Value) -> bool {
        matches!(self, Self::TaskWait { .. }) && body["reached"] == serde_json::Value::Bool(true)
    }

    /// The Sessions a `desktop sessions` read lists, its argv's flags parsed
    /// as the command parses them, defaults included.
    fn session_filter(&self, directory: &Path) -> anyhow::Result<SessionFilter> {
        // Past `--cwd <directory> desktop sessions`.
        let flags = std::iter::once("sessions".to_owned()).chain(self.argv().into_iter().skip(4));
        let command = SessionListArgs::augment_args(clap::Command::new("sessions"));
        let args = SessionListArgs::from_arg_matches(&command.try_get_matches_from(flags)?)?;
        super::session::list_filter(&args, directory)
    }
}

fn session_flags(headless: bool) -> &'static [&'static str] {
    if headless {
        &["--interactive", "all"]
    } else {
        &[]
    }
}

/// `future` run to completion on a runtime of its own, as its command does.
fn block_on<F: std::future::Future>(future: F) -> anyhow::Result<F::Output> {
    Ok(tokio::runtime::Runtime::new()?.block_on(future))
}

fn invalid(message: &str) -> crate::engine::Error {
    crate::engine::Error::new(crate::engine::ErrorCode::Invalid, message)
}

/// A served read's named absence; the CLI prints its message.
fn not_found(message: impl Into<String>, data: serde_json::Value) -> crate::engine::Error {
    crate::engine::Error::new(crate::engine::ErrorCode::NotFound, message).with(data)
}

fn value(value: &impl Serialize) -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::to_value(value)?)
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
                r#"{"kind":"session.history","since":"7d","limit":50,"wave":null,"project":"p","task":null}"#
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
            read(
                r#"{"kind":"session.list","directory":"/r","headless":true,"after":null,"limit":null}"#
            ),
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
                r#"{"kind":"task.diff","issue":"FIX-1","path":null,"base":"parent","draft":null}"#
            ),
            ["task", "diff", "FIX-1", "--base", "parent", "--json"]
        );
        assert_eq!(
            read(r#"{"kind":"desktop.view.get","directory":"/r","expression":"state:backlog"}"#),
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
