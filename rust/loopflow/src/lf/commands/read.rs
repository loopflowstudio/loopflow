//! Shared reads served by the client API server. Each [`Read`] is an `lf`
//! command: [`Read::argv`] names it, [`Read::read`] returns what it prints,
//! and for a command that takes `--subscribe`, [`Read::subscription`] goes through the
//! same function as `lf <argv> --subscribe`, so clients receive exactly the frames
//! that command prints.
//!
//! A read the command scopes by its working directory names that directory
//! instead: `--cwd <directory>` in its argv, and an explicit path in the server,
//! where concurrent subscriptions cannot change the working directory.
//!
//! Reads only read: they change no Work and leave no journal trace. The app's
//! writes are [`super::write::Write`]s.

use std::path::{Path, PathBuf};

use clap::{Args, FromArgMatches};
use serde::{Deserialize, Serialize};

use super::activity::HistoryWindow;
use super::graph_inventory;
use super::machine::machines;
use super::usage;
use super::waves::list as list_waves;
use crate::lf::SessionListArgs;
use loopflow_execution::engine::{Error, ErrorCode};
use loopflow_execution::machine::client::{Connection, Response};
use loopflow_execution::machine::connection::local_machine;
use loopflow_execution::machine::wire::{Secrets, Selection, Sender};
use loopflow_execution::ops::task::{task_changes, task_diff, task_file, task_files};
use loopflow_execution::read_stream::{Subscription, Transport};
use loopflow_execution::session::SessionFilter;
use loopflow_model::durable::{Machine, RepositoryId};

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
    destination: &loopflow_execution::config::files::Destination,
    command: &[&str],
) -> Vec<String> {
    let repository = destination.repository();
    let wave = destination.wave();
    let flag = if destination.is_shared() {
        Some("--shared")
    } else if repository.is_none() {
        Some("--global")
    } else {
        None
    };
    let mut argv = match repository {
        Some(repository) => at(repository, command),
        None => words(command),
    };
    if let Some(wave) = wave {
        argv.splice(0..0, ["--wave".to_owned(), wave.to_owned()]);
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// On the wire each read is a resource-named `kind` (`task.get`); `desktop.*`
/// reads are Desktop's own arrangement state.
#[serde(tag = "kind", deny_unknown_fields)]
// Exhaustive on purpose: the equivalence test matches every variant.
pub enum Read {
    /// `lf session history show <id>`: one Session's history, as display text.
    #[serde(rename = "session.history.get")]
    #[schemars(extend("x-result" = generator.subschema_for::<String>()))]
    SessionHistoryGet { id: String },
    /// Look up a creation without starting or retrying it. Null confirms absence.
    #[serde(rename = "creation.get")]
    #[schemars(extend("x-result" = generator.subschema_for::<Option<loopflow_storage::sqlite::creation::CreationReceipt>>()))]
    CreationGet {
        request_id: String,
        /// The Task the creation was for: a Task another Machine runs keeps
        /// its receipt there, as `--task <task>` reads it.
        #[serde(deserialize_with = "Option::deserialize")]
        task: Option<String>,
    },
    /// `lf --cwd <directory> desktop view list`: the plans a repository window shows.
    #[serde(rename = "desktop.view.list")]
    #[schemars(extend("x-result" = generator.subschema_for::<Vec<loopflow_storage::work::selection::ViewPlan>>()))]
    DesktopViewList { directory: PathBuf },
    /// `lf desktop view list --all [--expression <expression>]`: one selection
    /// across the portfolio repositories, as the phone lists open Tasks.
    #[serde(rename = "view.select")]
    #[schemars(extend("x-result" = generator.subschema_for::<Vec<loopflow_storage::work::selection::ViewPlan>>()))]
    ViewSelect { expression: Option<String> },
    /// `lf --cwd <directory> session list`, with `--headless`
    /// when `headless` Sessions are shown too: every row, or with a `limit`
    /// one page (`--page --limit <limit> [--after <after>]`). Only the whole
    /// list has a subscription.
    #[serde(rename = "session.list")]
    #[schemars(extend("x-result" = (crate::server::schema::either::<Vec<loopflow_execution::ops::human_session::SessionRecord>, loopflow_execution::ops::human_session::SessionPage>(generator))))]
    SessionList {
        directory: PathBuf,
        headless: bool,
        after: Option<String>,
        limit: Option<u32>,
        /// Only the Sessions of this Task, as `--task <task>` filters them.
        task: Option<String>,
    },
    /// `lf plan board`: compact Task rows for a board.
    #[serde(rename = "plan.board")]
    #[schemars(extend("x-result" = generator.subschema_for::<super::screens::PlanBoard>()))]
    PlanBoard {
        directory: PathBuf,
        wave: Option<String>,
        task: Option<String>,
        all: bool,
    },
    /// `lf task detail <issue>`: one Task and its supporting detail.
    #[serde(rename = "task.detail")]
    #[schemars(extend("x-result" = generator.subschema_for::<super::screens::TaskDetail>()))]
    TaskDetail { directory: PathBuf, issue: String },
    /// `lf session card <id>`: a passive reading of one recorded Session.
    #[serde(rename = "session.card")]
    #[schemars(extend("x-result" = generator.subschema_for::<super::screens::SessionCard>()))]
    SessionCard { id: String },
    /// `lf --cwd <directory> task show <issue>`: the Task pane.
    #[serde(rename = "task.get")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::lf::commands::waves::DesktopTaskPane>()))]
    TaskGet { directory: PathBuf, issue: String },
    /// `lf --cwd <directory> task status <issue> --json`: one Task's status
    /// and its planning.
    #[serde(rename = "task.status")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::machine::schema::Untyped>()))]
    TaskStatus { directory: PathBuf, issue: String },
    /// `lf task changes <issue> --base <base> [--head <head>]`: a Task's changed files.
    #[serde(rename = "task.changes")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::ops::task::TaskChangesSnapshot>()))]
    TaskChanges {
        issue: String,
        base: String,
        head: Option<String>,
    },
    /// `lf task diff <issue> [path] --base <base>`, or with `--draft` and the
    /// draft on stdin: one Task's patch.
    #[serde(rename = "task.diff")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::ops::task::TaskDiffSnapshot>()))]
    TaskDiff {
        issue: String,
        path: Option<String>,
        base: String,
        draft: Option<String>,
        head: Option<String>,
    },
    /// `lf task file <issue> <path> [--at <rev>]`: one file of a Task's
    /// checkout, or its version at a commit.
    #[serde(rename = "task.file")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::ops::task::TaskFileSnapshot>()))]
    TaskFile {
        issue: String,
        path: String,
        at: Option<String>,
    },
    /// `lf task files <issue> <path> [--cursor <cursor>] [--show-ignored]`:
    /// one page of a Task checkout directory's immediate entries.
    #[serde(rename = "task.files")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::ops::task::TaskDirectory>()))]
    TaskFiles {
        issue: String,
        path: String,
        cursor: Option<String>,
        show_ignored: bool,
    },
    /// Subscribe to changes in a Task checkout. A new revision invalidates the file tree.
    #[serde(rename = "task.files.watch")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::ops::task::TaskFilesRevision>()))]
    TaskFilesWatch { issue: String },
    /// `lf wave list --all`: every Wave on this Machine that is not removed.
    #[serde(rename = "wave.list")]
    #[schemars(extend("x-result" = generator.subschema_for::<Vec<crate::lf::commands::waves::WaveSnapshot>>()))]
    WaveList,
    /// `lf machine id`: this Machine's identity.
    #[serde(rename = "machine.self")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_model::durable::Machine>()))]
    MachineSelf,
    /// `lf desktop portfolio`: the repositories this Machine's windows show.
    #[serde(rename = "desktop.portfolio")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::portfolio::Portfolio>()))]
    DesktopPortfolio,
    /// `lf config catalog --json`: every typed setting and its policies.
    #[serde(rename = "config.catalog")]
    #[schemars(extend("x-result" = generator.subschema_for::<Vec<loopflow_execution::config::catalog::Definition>>()))]
    ConfigCatalog,
    /// `lf --cwd <directory> [--wave <wave>] config settings [--skill|--graph
    /// [--step]] --json`: effective settings and their sources.
    #[serde(rename = "config.settings")]
    #[schemars(extend("x-result" = generator.subschema_for::<Vec<loopflow_execution::engine::config::Inspection>>()))]
    ConfigSettings {
        directory: PathBuf,
        wave: Option<String>,
        skill: Option<String>,
        graph: Option<String>,
        step: Option<String>,
    },
    /// Effective settings and editable declarations for Global, a checkout or a Wave.
    #[serde(rename = "config.scope")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::engine::config::Scope>()))]
    ConfigScope {
        target: loopflow_execution::engine::config::ScopeTarget,
        #[serde(deserialize_with = "Option::deserialize")]
        directory: Option<PathBuf>,
        #[serde(deserialize_with = "Option::deserialize")]
        wave: Option<String>,
    },
    /// `lf config get <key> <destination flags> --json`: one file's declaration.
    #[serde(rename = "config.get")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::engine::config::Declaration>()))]
    ConfigGet {
        destination: loopflow_execution::config::files::Destination,
        key: String,
    },
    /// `lf config explain <key> <destination flags> --json`: every source's
    /// candidate and the value a launch there resolves.
    #[serde(rename = "config.explain")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::engine::config::Explanation>()))]
    ConfigExplain {
        destination: loopflow_execution::config::files::Destination,
        key: String,
    },
    /// `lf --cwd <directory> desktop layout query:<id>`: a Dashboard's slots.
    #[serde(rename = "desktop.layout")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_storage::work::selection::Dashboard>()))]
    DesktopLayout { directory: PathBuf, work: String },
    /// `lf --cwd <directory> repo people`: who owns the repository's Tasks.
    #[serde(rename = "repo.people")]
    #[schemars(extend("x-result" = generator.subschema_for::<Vec<loopflow_execution::ops::people::RepoPerson>>()))]
    RepoPeople { directory: PathBuf },
    /// `lf --cwd <directory> graph list`: every Graph a Task there can take up.
    #[serde(rename = "graph.list")]
    #[schemars(extend("x-result" = generator.subschema_for::<Vec<loopflow_execution::graph::compiled::GraphCatalogEntry>>()))]
    GraphList { directory: PathBuf },
    /// `lf --cwd <directory> desktop view show [expression]`: what a view
    /// shows; no expression is the repository's own.
    #[serde(rename = "desktop.view.get")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_storage::work::selection::ViewPlan>()))]
    DesktopViewGet {
        directory: PathBuf,
        expression: Option<String>,
    },
    /// `lf --cwd <directory> task runs <task> [--after <after>]`: one page of
    /// the Graphs that ran for a Task.
    #[serde(rename = "task.runs")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::durable::GraphProcessPage>()))]
    TaskRuns {
        directory: PathBuf,
        task: String,
        after: Option<String>,
    },
    /// `lf graph show <id>`: one Graph that ran, with its steps.
    #[serde(rename = "graph.run.get")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::durable::GraphProcessDetail>()))]
    GraphRunGet { id: String },
    /// `lf usage --days <days> [--wave|--project|--task …]`: the Session
    /// inputs recorded in that window, by Work.
    #[serde(rename = "session.usage")]
    #[schemars(extend("x-result" = generator.subschema_for::<Vec<loopflow_execution::session_record::SessionHistory>>()))]
    SessionUsage {
        days: u32,
        wave: Option<String>,
        project: Option<String>,
        task: Option<String>,
    },
    /// `lf task context <task>`: what each of that Task's Session inputs was
    /// made of.
    #[serde(rename = "task.context")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::context_usage::ContextReport>()))]
    TaskContext { task: String },
    /// `lf --cwd <directory> wave plan <wave>`: one Wave's Projects and
    /// Tasks, after syncing its repository with the tracker.
    #[serde(rename = "wave.plan")]
    #[schemars(extend("x-result" = generator.subschema_for::<Option<crate::lf::commands::waves::WaveDetailSnapshot>>()))]
    WavePlan { directory: PathBuf, wave: String },
    /// `lf --cwd <directory> task list --closed --wave <wave> [--after
    /// <after>]`: one page of a Wave's closed Tasks. Only `closed` lists
    /// exist; open Tasks are `plan.list`'s.
    #[serde(rename = "task.list")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::lf::commands::waves::ClosedTaskPageSnapshot>()))]
    TaskList {
        directory: PathBuf,
        wave: String,
        after: Option<String>,
        closed: bool,
    },
    /// `lf machine list`: your registered Machines.
    #[serde(rename = "machine.list")]
    #[schemars(extend("x-result" = generator.subschema_for::<Vec<crate::lf::commands::machine::MachineListing>>()))]
    MachineList,
    /// Check one registered connection by its durable identity.
    #[serde(rename = "machine.status")]
    #[schemars(extend("x-result" = generator.subschema_for::<Vec<super::machine::MachineStatus>>()))]
    MachineStatus {
        machine: loopflow_model::durable::MachineId,
    },
    /// `lf session history`: what Sessions did, by Work.
    #[serde(rename = "session.history")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::lf::commands::activity::WorkActivitySnapshot>()))]
    SessionHistory(HistoryWindow),
    /// `lf --cwd <directory> plan list [wave] [--task <task>] [--all]`:
    /// Waves and Tasks. Read once, it first asks the tracker about each
    /// repository read; its subscription reads the store.
    #[serde(rename = "plan.list")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::lf::commands::waves::RoadmapSnapshot>()))]
    PlanList {
        directory: PathBuf,
        wave: Option<String>,
        task: Option<String>,
        all: bool,
    },
    /// `lf --cwd <directory> repo tracker`: where the repository's
    /// planning is exchanged. Its subscription exchanges it while it runs.
    #[serde(rename = "repo.tracker")]
    #[schemars(extend("x-result" = generator.subschema_for::<super::tracker::TrackerStatus>()))]
    RepoTracker { directory: PathBuf },
    /// `lf --cwd <directory> open <url> --link`: what a Work address opens.
    #[serde(rename = "work.resolve")]
    #[schemars(extend("x-result" = generator.subschema_for::<Vec<crate::lf::commands::open::WorkLink>>()))]
    WorkResolve { directory: PathBuf, url: String },
    /// `lf --repo-id <id> repo path`: a repository plan's
    /// checkout on this Machine.
    #[serde(rename = "repo.path")]
    #[schemars(extend("x-result" = generator.subschema_for::<String>()))]
    RepoPath { id: RepositoryId },
    /// `lf --cwd <directory> wave graph show <wave>|--no-wave`: the graph a
    /// new Task there would take up.
    #[serde(rename = "wave.graph")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::ops::project::WaveGraph>()))]
    WaveGraph {
        directory: PathBuf,
        wave: Option<String>,
    },
    /// `lf --cwd <directory> repo tokens [--days <days>]`: the codebase's size.
    #[serde(rename = "repo.tokens")]
    #[schemars(extend("x-result" = (crate::server::schema::either::<crate::lf::commands::tokens::CodeNode, Vec<crate::lf::commands::tokens::CodeSnapshot>>(generator))))]
    RepoTokens {
        directory: PathBuf,
        days: Option<u32>,
    },
    /// `lf session retention`: how much Session history providers keep.
    #[serde(rename = "session.retention")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::transcript::summary::RetentionSummary>()))]
    SessionRetention,
    /// `lf machine doctor`: the ledger's self-audit.
    #[serde(rename = "machine.doctor")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::lf::commands::doctor::DoctorReport>()))]
    MachineDoctor,
    /// `lf repo ci --since <since> [--wave <wave>] [--repo <repo>]`:
    /// how failed CI was detected, repaired and landed on this Machine.
    #[serde(rename = "repo.ci")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::ops::ci::CiReportDto>()))]
    RepoCi {
        since: String,
        wave: Option<String>,
        repo: Option<String>,
    },
    /// `lf account [<provider>] --json`: saved service authentication.
    #[serde(rename = "account.list")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::provider_auth::operations::account_status::AccountReport>()))]
    AccountList { target: Option<String> },
    /// `lf --cwd <directory> list [<path>…] --json`: the commands, skills,
    /// graphs and Waves under `path`.
    #[serde(rename = "skill.list")]
    #[schemars(extend("x-result" = generator.subschema_for::<Vec<loopflow_execution::skills::listing::Entry>>()))]
    SkillList {
        directory: PathBuf,
        path: Vec<String>,
    },
    /// `lf --cwd <directory> pr checks [--required] --json`: the branch's
    /// check report as the provider prints it, with its state.
    #[serde(rename = "pr.checks")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::engine::pr::Checks>()))]
    PrChecks { directory: PathBuf, required: bool },
    /// `lf --cwd <directory> pr reviews [<pr>] --json`: requested reviews
    /// and review submissions, each with the commit it reviewed.
    #[serde(rename = "pr.reviews")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::engine::pr::Reviews>()))]
    PrReviews {
        directory: PathBuf,
        pr: Option<String>,
    },
    /// `lf task wait <issue> --until <until> --timeout 0s --json`: the Task now
    /// and whether it reached `until`. Wake, it reads on each change and
    /// ends after the reading that reached it.
    #[serde(rename = "task.wait")]
    #[schemars(extend("x-result" = generator.subschema_for::<loopflow_execution::engine::task::Waited>()))]
    TaskWait {
        issue: String,
        until: loopflow_execution::ops::task::TaskWaitUntil,
    },
}

impl Read {
    /// The `lf` command that prints this read, `--json` included where the
    /// command takes it. Its subscription is this argv with `--subscribe`.
    pub fn argv(&self) -> Vec<String> {
        match self {
            Self::SessionHistoryGet { id } => words(&["session", "history", "show", id]),
            Self::CreationGet { request_id, task } => {
                let mut argv = words(&["server", "receipt", request_id]);
                argv.extend(optional("--task", task.as_ref()));
                argv.push("--json".into());
                argv
            }
            Self::DesktopViewList { directory } => {
                at(directory, &["desktop", "view", "list", "--json"])
            }
            Self::ViewSelect { expression } => {
                let mut argv = words(&["desktop", "view", "list", "--all", "--json"]);
                argv.extend(optional("--expression", expression.as_ref()));
                argv
            }
            Self::SessionList {
                directory,
                headless,
                after,
                limit,
                task,
            } => {
                let mut argv = at(directory, &["session", "list"]);
                argv.extend(words(session_flags(*headless)));
                argv.push("--json".into());
                if let Some(limit) = limit {
                    argv.extend(words(&["--page", "--limit", &limit.to_string()]));
                    argv.extend(optional("--after", after.as_ref()));
                } else {
                    argv.extend(words(&["--limit", "0"]));
                }
                argv.extend(optional("--task", task.as_ref()));
                argv
            }
            Self::TaskDetail { directory, issue } => {
                at(directory, &["task", "detail", issue, "--json"])
            }
            Self::SessionCard { id } => words(&["session", "card", id, "--json"]),
            Self::PlanBoard {
                directory,
                wave,
                task,
                all,
            } => {
                let mut argv = at(directory, &["plan", "board"]);
                argv.extend(wave.clone());
                argv.extend(optional("--task", task.as_ref()));
                if *all {
                    argv.push("--all".into());
                }
                argv.push("--json".into());
                argv
            }
            Self::TaskGet { directory, issue } => at(directory, &["task", "show", issue, "--json"]),
            Self::TaskStatus { directory, issue } => {
                at(directory, &["task", "status", issue, "--json"])
            }
            Self::TaskChanges { issue, base, head } => {
                let mut argv = words(&["task", "changes", issue, "--base", base, "--json"]);
                argv.extend(optional("--head", head.as_ref()));
                argv
            }
            Self::TaskDiff {
                issue,
                path,
                base,
                head,
                draft,
            } => {
                let mut argv: Vec<String> = words(&["task", "diff", issue]);
                argv.extend(path.clone());
                argv.extend(["--base".into(), base.clone(), "--json".into()]);
                argv.extend(optional("--head", head.as_ref()));
                if draft.is_some() {
                    argv.push("--draft".into());
                }
                argv
            }
            Self::TaskFile { issue, path, at } => {
                let mut argv = words(&["task", "file", issue, path, "--json"]);
                argv.extend(optional("--at", at.as_ref()));
                argv
            }
            Self::TaskFiles {
                issue,
                path,
                cursor,
                show_ignored,
            } => {
                let mut argv = words(&["task", "files", issue, path, "--json"]);
                argv.extend(optional("--cursor", cursor.as_ref()));
                if *show_ignored {
                    argv.push("--show-ignored".into());
                }
                argv
            }
            // No command streams it; this is the read each revision repeats.
            Self::TaskFilesWatch { issue } => words(&["task", "files", issue, ".", "--json"]),
            Self::WaveList => words(&["wave", "list", "--all", "--json"]),
            Self::MachineSelf => words(&["machine", "self", "--json"]),
            Self::DesktopPortfolio => words(&["desktop", "portfolio", "--json"]),
            Self::ConfigCatalog => words(&["config", "catalog", "--json"]),
            Self::ConfigScope {
                directory,
                wave,
                target,
            } => {
                let mut argv = directory.as_ref().map_or_else(
                    || words(&["config", "scope", "--global", "--json"]),
                    |dir| at(dir, &["config", "scope", "--json"]),
                );
                argv.extend(optional("--wave", wave.as_ref()));
                use loopflow_execution::engine::config::ScopeTarget;
                match target {
                    ScopeTarget::General => {}
                    ScopeTarget::Skill { name } => argv.extend(words(&["--skill", name])),
                    ScopeTarget::Step { graph, step } => {
                        argv.extend(words(&["--graph", graph, "--step", step]))
                    }
                }
                argv
            }
            Self::ConfigGet { destination, key } => {
                destination_argv(destination, &["config", "get", key, "--json"])
            }
            Self::ConfigExplain { destination, key } => {
                destination_argv(destination, &["config", "explain", key, "--json"])
            }
            Self::ConfigSettings {
                directory,
                wave,
                skill,
                graph,
                step,
            } => {
                let mut argv = at(directory, &["config", "settings", "--json"]);
                argv.extend(optional("--wave", wave.as_ref()));
                argv.extend(optional("--skill", skill.as_ref()));
                argv.extend(optional("--graph", graph.as_ref()));
                argv.extend(optional("--step", step.as_ref()));
                argv
            }
            // `desktop layout` always prints JSON.
            Self::DesktopLayout { directory, work } => at(directory, &["desktop", "layout", work]),
            Self::RepoPeople { directory } => at(directory, &["repo", "people", "--json"]),
            Self::GraphList { directory } => at(directory, &["graph", "list", "--json"]),
            Self::DesktopViewGet {
                directory,
                expression,
            } => {
                let mut argv = at(directory, &["desktop", "view", "show"]);
                argv.extend(expression.clone());
                argv.push("--json".into());
                argv
            }
            Self::TaskRuns {
                directory,
                task,
                after,
            } => {
                let mut argv = at(directory, &["task", "runs", task]);
                argv.extend(optional("--after", after.as_ref()));
                argv.push("--json".into());
                argv
            }
            Self::GraphRunGet { id } => words(&["graph", "show", id, "--json"]),
            Self::SessionUsage {
                days,
                wave,
                project,
                task,
            } => {
                let mut argv = words(&["session", "usage", "--days", &days.to_string()]);
                argv.extend(optional("--wave", wave.as_ref()));
                argv.extend(optional("--project", project.as_ref()));
                argv.extend(optional("--task", task.as_ref()));
                argv.push("--json".into());
                argv
            }
            Self::TaskContext { task } => words(&["task", "context", task, "--json"]),
            Self::WavePlan { directory, wave } => at(directory, &["wave", "plan", wave, "--json"]),
            Self::TaskList {
                directory,
                wave,
                after,
                ..
            } => {
                let mut argv = at(
                    directory,
                    &["task", "list", "--closed", "--wave", wave, "--json"],
                );
                argv.extend(optional("--after", after.as_ref()));
                argv
            }
            Self::MachineStatus { machine } => {
                words(&["machine", "status", machine.as_str(), "--json"])
            }
            Self::MachineList => words(&["machine", "list", "--json"]),
            Self::SessionHistory(window) => {
                let mut argv = window.argv();
                argv.push("--json".into());
                argv
            }
            Self::PlanList {
                directory,
                wave,
                task,
                all,
            } => {
                let mut argv = at(directory, &["plan", "list"]);
                argv.extend(wave.clone());
                argv.extend(optional("--task", task.as_ref()));
                if *all {
                    argv.push("--all".into());
                }
                argv.push("--json".into());
                argv
            }
            Self::RepoTracker { directory } => at(directory, &["repo", "tracker", "--json"]),
            Self::WorkResolve { directory, url } => {
                at(directory, &["open", url, "--link", "--json"])
            }
            Self::RepoPath { id } => words(&["--repo-id", &id.to_string(), "repo", "path"]),
            Self::WaveGraph { directory, wave } => {
                let mut argv = at(directory, &["wave", "graph", "show"]);
                argv.push(wave.clone().unwrap_or_else(|| "--no-wave".into()));
                argv.push("--json".into());
                argv
            }
            Self::RepoTokens { directory, days } => {
                let mut argv = at(directory, &["repo", "tokens", "--json"]);
                argv.extend(optional("--days", *days));
                argv
            }
            Self::SessionRetention => words(&["session", "retention", "--json"]),
            Self::MachineDoctor => words(&["machine", "doctor", "--json"]),
            Self::RepoCi { since, wave, repo } => {
                let mut argv = words(&["repo", "ci", "--since", since]);
                if let Some(wave) = wave {
                    argv.extend(words(&["--wave", wave]));
                }
                if let Some(repo) = repo {
                    argv.extend(words(&["--repo", repo]));
                }
                argv.push("--json".into());
                argv
            }
            Self::AccountList { target } => {
                let mut argv = words(&["account"]);
                argv.extend(target.iter().cloned());
                argv.push("--json".into());
                argv
            }

            Self::SkillList { directory, path } => {
                let mut argv = at(directory, &["list"]);
                argv.extend(path.iter().cloned());
                argv.push("--json".into());
                argv
            }
            Self::PrChecks {
                directory,
                required,
            } => {
                let mut argv = at(directory, &["pr", "checks"]);
                if *required {
                    argv.push("--required".into());
                }
                argv.push("--json".into());
                argv
            }
            Self::PrReviews { directory, pr } => {
                let mut argv = at(directory, &["pr", "reviews"]);
                argv.extend(pr.clone());
                argv.push("--json".into());
                argv
            }
            Self::TaskWait { issue, until } => {
                let until = match until {
                    loopflow_execution::ops::task::TaskWaitUntil::Open => "submitted",
                    loopflow_execution::ops::task::TaskWaitUntil::Terminal => "terminal",
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
    /// A Task another Machine runs is read by that Machine's server.
    pub fn read(self) -> anyhow::Result<serde_json::Value> {
        self.read_with(|machine, sender, read| {
            Connection::open_on(machine, sender, None)?.query(read)
        })
    }

    pub(crate) fn read_with(
        mut self,
        query: impl FnOnce(&Machine, Sender, &Read) -> anyhow::Result<Response>,
    ) -> anyhow::Result<serde_json::Value> {
        if let Some(route) = self.peer_route()? {
            let (machine, sender) = self.address_peer(route.clone())?;
            let mut result = query(&machine, sender, &self)?.result;
            self.localize_peer_reading(&mut result, &machine, &route)?;
            return Ok(result);
        }
        self.read_here()
    }

    /// What `machine` answered for this read, as this Machine names it: the
    /// Task by this Machine's ID and repository checkout, the owner by this
    /// Machine's label for it, and Sessions opened with this Machine's
    /// `lf open`. The owner's checkout path stays: it is where the files are.
    /// `reading` may be a selected one; keys it lacks stay absent.
    pub(crate) fn localize_peer_reading(
        &self,
        reading: &mut serde_json::Value,
        machine: &Machine,
        route: &loopflow_execution::durable::TaskExecutionRoute,
    ) -> anyhow::Result<()> {
        let task = serde_json::Value::from(route.task_id.to_string());
        let label = serde_json::Value::from(machine.label.clone());
        match self {
            Self::SessionList { .. } => {
                super::ssh::localize_remote_sessions(reading, machine, route)?;
            }
            Self::TaskGet { .. } | Self::TaskStatus { .. } | Self::TaskDetail { .. } => {
                for pointer in ["/task/id", "/row/task/id", "/row/runtime/work_id"] {
                    replace_present(reading, pointer, &task);
                }
                // Unbound here, the owner's path is the only one there is.
                if let Ok(repo) = super::work_route::repository_path(&route.repository_id) {
                    let repo = serde_json::Value::from(repo.display().to_string());
                    for pointer in ["/task/repo", "/row/task/repo"] {
                        replace_present(reading, pointer, &repo);
                    }
                }
                replace_present(reading, "/row/reference/workspace/machine_label", &label);
                if let Some(session) = reading.pointer_mut("/current_session") {
                    super::ssh::localize_remote_session(session, machine, Some(route))?;
                }
            }
            Self::CreationGet { .. } => {
                if let Some(id) = reading.get_mut("task_id").filter(|id| !id.is_null()) {
                    *id = task;
                }
            }
            Self::TaskFilesWatch { .. } => {
                replace_present(reading, "/task_id", &task);
                replace_present(reading, "/machine_label", &label);
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn follows_task(&self) -> bool {
        matches!(
            self,
            Self::TaskGet { .. } | Self::TaskStatus { .. } | Self::TaskFilesWatch { .. }
        )
    }

    pub(crate) fn workspace_task_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::TaskGet { issue, .. }
            | Self::TaskDetail { issue, .. }
            | Self::TaskRuns { task: issue, .. }
            | Self::SessionList {
                task: Some(issue), ..
            }
            | Self::CreationGet {
                task: Some(issue), ..
            }
            | Self::TaskChanges { issue, .. }
            | Self::TaskDiff { issue, .. }
            | Self::TaskFile { issue, .. }
            | Self::TaskFilesWatch { issue }
            | Self::TaskFiles { issue, .. }
            | Self::TaskStatus { issue, .. } => Some(issue),
            _ => None,
        }
    }

    /// Address the read as the owner's repository knows the Task.
    pub(super) fn peer(&mut self) -> anyhow::Result<Option<(Machine, Sender)>> {
        let Some(route) = self.peer_route()? else {
            return Ok(None);
        };
        self.address_peer(route).map(Some)
    }

    pub(crate) fn peer_route(
        &mut self,
    ) -> anyhow::Result<Option<loopflow_execution::durable::TaskExecutionRoute>> {
        use loopflow_execution::machine::task;
        let status = matches!(self, Self::TaskStatus { .. });
        let directory = self.directory_mut().cloned();
        let Some(issue) = self.workspace_task_mut() else {
            return Ok(None);
        };
        // While a move is in flight the source still reports its progress. File
        // operations always follow the execution route, even if that peer is down.
        if status {
            task::remote_ownership_writer(issue, directory.as_deref())
        } else {
            task::remote_task(issue, directory.as_deref())
        }
    }

    pub(crate) fn address_peer(
        &mut self,
        route: loopflow_execution::durable::TaskExecutionRoute,
    ) -> anyhow::Result<(Machine, Sender)> {
        let (machine, sender) = peer_of(&route)?;
        *self
            .workspace_task_mut()
            .expect("only Task reads have peer routes") = route.peer_selector;
        Ok((machine, sender))
    }

    /// `read` on this Machine, never sent on: what a request another Machine
    /// sent runs. A command that prints bare text (`config repo --path`) is a
    /// string.
    pub fn read_here(&self) -> anyhow::Result<serde_json::Value> {
        match self {
            Self::SessionHistoryGet { id } => value(&super::history::render(
                id,
                false,
                None,
                super::session_history::SessionView::Summary,
            )?),
            Self::CreationGet { request_id, .. } => {
                value(&loopflow_execution::engine::creation::lookup(request_id)?)
            }
            Self::PlanBoard {
                directory,
                wave,
                task,
                all,
            } => value(&super::screens::board(
                directory,
                wave.as_deref(),
                task.as_deref(),
                *all,
            )?),
            Self::TaskDetail { directory, issue } => {
                value(&super::screens::task_detail(directory, issue)?)
            }
            Self::SessionCard { id } => value(&super::screens::session_card(id)?),
            Self::DesktopViewList { directory } => {
                value(&super::desktop::read_plans(None, directory)?)
            }
            Self::ViewSelect { expression } => {
                value(&super::desktop::read_all_view_plans(expression.as_deref())?)
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
                )?)
            }
            Self::TaskGet { directory, issue } => {
                value(&super::desktop::read_task_pane(directory, issue)?)
            }
            Self::TaskStatus { directory, issue } => {
                value(&super::waves::read_task_detail(directory, issue)?)
            }
            Self::TaskChanges { issue, base, head } => {
                value(&task_changes(issue, base, head.as_deref())?)
            }
            Self::TaskDiff {
                issue,
                path,
                base,
                head,
                draft,
            } => value(&task_diff(
                issue,
                path.as_deref(),
                base,
                head.as_deref(),
                draft.as_deref(),
            )?),
            Self::TaskFile { issue, path, at } => {
                value(&task_file(issue, path, at.as_deref(), false)?)
            }
            Self::TaskFiles {
                issue,
                path,
                cursor,
                show_ignored,
            } => value(&task_files(issue, path, cursor.as_deref(), *show_ignored)?),
            Self::TaskFilesWatch { .. } => {
                Err(invalid("task.files.watch requires a subscription").into())
            }
            Self::WaveList => {
                let waves = block_on(list_waves(true))??;
                value(&waves.unwrap_or_default())
            }
            Self::MachineSelf => value(&block_on(local_machine())??),
            Self::DesktopPortfolio => value(&loopflow_execution::portfolio::read()?),
            Self::ConfigSettings {
                directory,
                wave,
                skill,
                graph,
                step,
            } => value(&loopflow_execution::engine::config::settings(
                directory,
                wave.as_deref(),
                skill.as_deref(),
                graph.as_deref(),
                step.as_deref(),
            )?),
            Self::ConfigScope {
                directory,
                wave,
                target,
            } => value(&loopflow_execution::engine::config::scope(
                directory.as_deref(),
                wave.as_deref(),
                target,
            )?),
            Self::ConfigCatalog => value(&loopflow_execution::engine::config::catalog()),
            Self::ConfigGet { destination, key } => {
                value(&loopflow_execution::engine::config::get(destination, key)?)
            }
            Self::ConfigExplain { destination, key } => value(
                &loopflow_execution::engine::config::explain(destination, key)?,
            ),
            Self::DesktopLayout { directory, work } => {
                super::desktop::saved_layout(None, directory, work)
            }
            Self::RepoPeople { directory } => value(&super::people::people(directory)?.0),
            Self::GraphList { directory } => {
                value(&loopflow_execution::graph::compiled::graph_catalog(
                    &loopflow_execution::repo::working_directory_in(directory)?,
                )?)
            }
            Self::DesktopViewGet {
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
            Self::TaskRuns {
                directory,
                task,
                after,
            } => value(&graph_inventory::page(
                directory,
                &graph_inventory::GraphProcessInventoryArgs {
                    runs: true,
                    for_task: Some(task.clone()),
                    after: after.clone(),
                    ..Default::default()
                },
            )?),
            Self::GraphRunGet { id } => value(&graph_inventory::detail(id)?),
            Self::SessionUsage {
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
            Self::WavePlan { directory, wave } => {
                value(&super::waves::desktop_status(directory, wave)?)
            }
            Self::TaskList { closed: false, .. } => {
                Err(invalid("only closed Tasks are listed; read plan.list for open ones").into())
            }
            Self::TaskList {
                directory,
                wave,
                after,
                closed: true,
            } => value(&super::waves::desktop_closed_tasks(
                directory,
                wave,
                after.as_deref(),
            )?),
            Self::MachineStatus { machine } => {
                value(&block_on(super::machine::status_by_id(machine))??)
            }
            Self::MachineList => value(&block_on(machines())??),
            Self::SessionHistory(window) => value(&super::activity::snapshot(window)?),
            Self::PlanList {
                directory,
                wave,
                task,
                all,
            } => value(&super::waves::show_snapshot(
                directory,
                wave.as_deref(),
                task.as_deref(),
                *all,
                false,
            )?),
            Self::RepoTracker { directory } => value(&super::tracker::read_status(directory)?),
            Self::WorkResolve { directory, url } => value(&super::open::url_links(directory, url)?),
            // The command prints the path alone.
            Self::RepoPath { id } => {
                let checkout = super::work_route::repository_path(id)?;
                let repo =
                    loopflow_execution::repository::containing(&checkout)?.ok_or_else(|| {
                        not_found(
                            format!("repository plan {id} has no local checkout"),
                            serde_json::json!({ "repo_id": id }),
                        )
                    })?;
                Ok(repo.to_string().into())
            }
            Self::WaveGraph { directory, wave } => {
                let repo = loopflow_execution::repo::working_directory_in(directory)?;
                value(&block_on(loopflow_execution::ops::project::wave_graph(
                    &repo,
                    wave.as_deref(),
                    wave.is_some(),
                ))??)
            }
            Self::RepoTokens { directory, days } => super::tokens::codebase(directory, *days),
            Self::SessionRetention => value(
                &super::history_transcript::retention_summary()?.ok_or_else(|| {
                    not_found(
                        "No Sessions recorded.",
                        serde_json::json!({ "resource": "sessions" }),
                    )
                })?,
            ),
            Self::MachineDoctor => value(&super::doctor::report()?),
            Self::RepoCi { since, wave, repo } => value(&loopflow_execution::engine::repo::ci(
                since,
                wave.as_deref(),
                repo.as_deref(),
            )?),
            Self::AccountList { target } => value(&loopflow_execution::engine::account::list(
                target.as_deref(),
            )?),
            Self::SkillList { directory, path } => {
                let repo = loopflow_execution::repo::discover_repo_root(directory)
                    .map_err(|error| Error::refused(&error, ErrorCode::Invalid))?
                    .unwrap_or_else(|| directory.to_owned());
                let entries = super::list::list_children(path, &repo)
                    .map_err(|error| Error::refused(&error, ErrorCode::Unavailable))?;
                value(&entries)
            }
            Self::PrChecks {
                directory,
                required,
            } => value(&loopflow_execution::engine::pr::checks(
                directory, *required,
            )?),
            Self::PrReviews { directory, pr } => value(&loopflow_execution::engine::pr::reviews(
                directory,
                pr.as_deref(),
            )?),
            Self::TaskWait { issue, until } => {
                value(&loopflow_execution::engine::task::waited(issue, *until)?)
            }
        }
    }

    /// The directory this read is scoped by, which a read another Machine
    /// sent names on that Machine.
    pub(crate) fn directory_mut(&mut self) -> Option<&mut PathBuf> {
        match self {
            Self::DesktopViewList { directory }
            | Self::SessionList { directory, .. }
            | Self::PlanBoard { directory, .. }
            | Self::TaskDetail { directory, .. }
            | Self::TaskGet { directory, .. }
            | Self::TaskStatus { directory, .. }
            | Self::ConfigSettings { directory, .. }
            | Self::DesktopLayout { directory, .. }
            | Self::RepoPeople { directory }
            | Self::GraphList { directory }
            | Self::DesktopViewGet { directory, .. }
            | Self::TaskRuns { directory, .. }
            | Self::WavePlan { directory, .. }
            | Self::TaskList { directory, .. }
            | Self::PlanList { directory, .. }
            | Self::RepoTracker { directory }
            | Self::WorkResolve { directory, .. }
            | Self::WaveGraph { directory, .. }
            | Self::RepoTokens { directory, .. }
            | Self::SkillList { directory, .. }
            | Self::PrChecks { directory, .. }
            | Self::PrReviews { directory, .. } => Some(directory),
            Self::ConfigScope { directory, .. } => directory.as_mut(),
            Self::SessionHistoryGet { .. }
            | Self::CreationGet { .. }
            | Self::ViewSelect { .. }
            | Self::SessionCard { .. }
            | Self::TaskChanges { .. }
            | Self::TaskDiff { .. }
            | Self::TaskFile { .. }
            | Self::TaskFilesWatch { .. }
            | Self::TaskFiles { .. }
            | Self::WaveList
            | Self::MachineSelf
            | Self::DesktopPortfolio
            | Self::ConfigCatalog
            | Self::ConfigGet { .. }
            | Self::ConfigExplain { .. }
            | Self::GraphRunGet { .. }
            | Self::SessionUsage { .. }
            | Self::TaskContext { .. }
            | Self::MachineList
            | Self::MachineStatus { .. }
            | Self::SessionHistory(_)
            | Self::RepoPath { .. }
            | Self::SessionRetention
            | Self::MachineDoctor
            | Self::RepoCi { .. }
            | Self::AccountList { .. }
            | Self::TaskWait { .. } => None,
        }
    }

    /// Serve this read's subscription inside this process, as `lf <argv> --subscribe`
    /// prints it, keeping only `select`'s dotted paths of each body. `wake` is called from any thread when a
    /// [`Subscription::pull`] may find a reading or occurrences; dropping the subscription ends
    /// it. A read whose command takes no `--subscribe` is an error.
    pub fn subscribe(
        self,
        select: Option<Vec<String>>,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> anyhow::Result<Subscription> {
        // What ends a subscription stays in its frames whatever is selected.
        let select = select.map(|mut paths| {
            paths.extend(self.ending_paths().iter().map(|path| path.to_string()));
            paths
        });
        let events = matches!(
            self,
            Self::TaskGet { .. }
                | Self::TaskStatus { .. }
                | Self::TaskWait { .. }
                | Self::PlanList { .. }
                | Self::DesktopViewList { .. }
        );
        let transport = Transport::Pull {
            wake: Box::new(wake),
            select,
            events,
        };
        let subscription = match self {
            Self::ConfigScope {
                directory,
                wave,
                target,
            } => super::setting::subscribe_scope(directory, wave, target, transport),
            Self::DesktopViewList { directory } => {
                super::desktop::subscribe_plans(None, directory, transport)
            }
            Self::DesktopViewGet {
                directory,
                expression,
            } => super::desktop::subscribe_view_plan(directory, expression, transport),
            Self::ViewSelect { expression } => {
                super::desktop::subscribe_all_view_plans(expression, transport)
            }
            Self::SessionList {
                ref directory,
                limit: None,
                after: None,
                ..
            } => super::session::subscribe_list(self.session_filter(directory)?, transport),
            Self::TaskGet { directory, issue } => {
                super::desktop::subscribe_task_pane(&directory, &issue, transport)
            }
            Self::TaskStatus { directory, issue } => {
                super::waves::subscribe_task_status(&directory, issue, transport)
            }
            Self::SessionHistory(window) => super::activity::subscribe(window, transport),
            Self::PlanList {
                directory,
                wave,
                task,
                all,
            } => super::waves::subscribe_show(directory, wave, task, all, transport),
            Self::RepoTracker { directory } => super::tracker::subscribe_status(
                &super::tracker::status_repository(&directory)?,
                transport,
            ),
            Self::TaskWait { issue, until } => {
                loopflow_execution::engine::task::subscribe_wait(issue, until, transport)
            }
            other => {
                return Err(loopflow_execution::engine::Error::new(
                    loopflow_execution::engine::ErrorCode::Invalid,
                    format!("`lf {}` has no --subscribe", other.argv().join(" ")),
                )
                .into())
            }
        }?;
        Ok(subscription.expect("a pulled subscription returns its handle"))
    }

    /// Whether a subscription to this read ends after `body`, a reading's
    /// body: `task.wait` ends once the Task reached what it waits for.
    pub fn ends_after(&self, body: &serde_json::Value) -> bool {
        matches!(self, Self::TaskWait { .. }) && body["reached"] == serde_json::Value::Bool(true)
    }

    /// The body paths [`Self::ends_after`] reads.
    fn ending_paths(&self) -> &'static [&'static str] {
        match self {
            Self::TaskWait { .. } => &["reached"],
            _ => &[],
        }
    }

    /// The Sessions a `session list` read lists, its argv's flags parsed
    /// as the command parses them, defaults included.
    fn session_filter(&self, directory: &Path) -> anyhow::Result<SessionFilter> {
        // Past `--cwd <directory> session list`.
        let flags = std::iter::once("sessions".to_owned()).chain(self.argv().into_iter().skip(4));
        let command = SessionListArgs::augment_args(clap::Command::new("sessions"));
        let args = SessionListArgs::from_arg_matches(&command.try_get_matches_from(flags)?)?;
        super::session::list_filter(&args, directory)
    }
}

fn session_flags(headless: bool) -> &'static [&'static str] {
    if headless {
        &["--headless"]
    } else {
        &[]
    }
}

/// `future` run to completion on a runtime of its own, as its command does.
fn block_on<F: std::future::Future>(future: F) -> anyhow::Result<F::Output> {
    Ok(tokio::runtime::Runtime::new()?.block_on(future))
}

fn invalid(message: &str) -> loopflow_execution::engine::Error {
    loopflow_execution::engine::Error::new(loopflow_execution::engine::ErrorCode::Invalid, message)
}

/// A served read's named absence; the CLI prints its message.
fn not_found(
    message: impl Into<String>,
    data: serde_json::Value,
) -> loopflow_execution::engine::Error {
    loopflow_execution::engine::Error::new(loopflow_execution::engine::ErrorCode::NotFound, message)
        .with(data)
}

fn value(value: &impl Serialize) -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::to_value(value)?)
}

/// Set `pointer` in `reading` to `value` when `reading` carries it.
fn replace_present(reading: &mut serde_json::Value, pointer: &str, value: &serde_json::Value) {
    if let Some(slot) = reading.pointer_mut(pointer) {
        *slot = value.clone();
    }
}

/// The Machine a Task route names and the sender its repository sends as.
pub(crate) fn peer_of(
    route: &loopflow_execution::durable::TaskExecutionRoute,
) -> anyhow::Result<(Machine, Sender)> {
    let machine = super::write::find_machine(route.machine_id.as_str())?;
    let directory = super::work_route::repository_path(&route.repository_id)?;
    let sender = Sender {
        repository: route.repository_id.clone(),
        selection: Selection::capture(&directory, None)?,
        secrets: Secrets::default(),
    };
    Ok((machine, sender))
}

#[cfg(test)]
mod tests {
    use super::Read;

    #[test]
    fn machine_status_shares_the_desktop_wire_shape() {
        use super::super::machine::MachineStatus;
        use serde_json::{from_value, to_value, Value};

        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/machine_status.json"
        ))
        .unwrap();
        let read: Read = from_value(fixture["read"].clone()).unwrap();
        assert_eq!(to_value(read).unwrap(), fixture["read"]);
        let status: Vec<MachineStatus> = from_value(fixture["status"].clone()).unwrap();
        assert_eq!(to_value(status).unwrap(), fixture["status"]);
    }

    #[test]
    fn checkout_scope_and_checked_edit_share_the_desktop_wire_shape() {
        use loopflow_execution::{config::files::edit::Applied, engine::config::Scope};
        use serde_json::{from_value, to_value, Value};
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/configuration_scope.json"
        ))
        .unwrap();
        let check = |key: &str, value: Value| assert_eq!(value, fixture[key]);
        check(
            "read",
            to_value(from_value::<Read>(fixture["read"].clone()).unwrap()).unwrap(),
        );
        for name in ["write", "preview", "promote", "dismiss"] {
            check(
                name,
                to_value(
                    from_value::<crate::lf::commands::write::Write>(fixture[name].clone()).unwrap(),
                )
                .unwrap(),
            );
        }
        check(
            "scope",
            to_value(from_value::<Scope>(fixture["scope"].clone()).unwrap()).unwrap(),
        );
        for name in ["move_result", "dismissed"] {
            let outcome = from_value::<loopflow_execution::engine::config::promotion::Outcome>(
                fixture[name].clone(),
            )
            .unwrap();
            check(name, to_value(outcome).unwrap());
        }
        check(
            "receipt",
            to_value(from_value::<Applied>(fixture["receipt"].clone()).unwrap()).unwrap(),
        );
    }

    /// The JSON Desktop's `ServedRead` encodes.
    #[test]
    fn desktop_encodings_name_their_reads() {
        let read = |json: &str| serde_json::from_str::<Read>(json).unwrap().argv();
        assert_eq!(
            read(
                r#"{"kind":"session.history","since":"7d","limit":50,"wave":null,"project":"p","task":null}"#
            ),
            [
                "session",
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
                "session",
                "list",
                "--headless",
                "--json",
                "--limit",
                "0"
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
                "show",
                "state:backlog",
                "--json"
            ]
        );
        assert_eq!(
            read(r#"{"kind":"creation.get","request_id":"reset","task":"LOO-1"}"#),
            ["server", "receipt", "reset", "--task", "LOO-1", "--json"]
        );
    }

    #[test]
    fn session_subscription_keeps_conversations_beyond_the_first_hundred() {
        let home = tempfile::tempdir().unwrap();
        let store =
            loopflow_execution::store::opening::open_ephemeral(&home.path().join("store.db"))
                .unwrap();
        for index in 0..101 {
            store.test_session(
                &format!("session-{index:03}"),
                &loopflow_execution::session_record::new_artifact_key(),
            );
        }
        let read = Read::SessionList {
            directory: home.path().into(),
            headless: false,
            after: None,
            limit: None,
            task: None,
        };
        let filter = read.session_filter(home.path()).unwrap();
        let sessions =
            loopflow_execution::store::session_projection::session_summaries(&store, &filter, 0)
                .unwrap();
        assert_eq!(sessions.len(), 101);
        assert!(sessions.iter().any(|session| session.id == "session-100"));
    }

    #[test]
    fn served_session_lists_include_headless_but_never_completed() {
        let home = tempfile::tempdir().unwrap();
        let store =
            loopflow_execution::store::opening::open_ephemeral(&home.path().join("store.db"))
                .unwrap();
        for id in ["interactive", "headless", "closed", "closed-headless"] {
            store.test_session(id, &loopflow_execution::session_record::new_artifact_key());
        }
        store.conn.lock().unwrap().execute_batch(
            "UPDATE agent_sessions SET interactive=0 WHERE id IN ('headless','closed-headless');
             UPDATE agent_sessions SET completed_at=2 WHERE id IN ('closed','closed-headless');"
        ).unwrap();
        for limit in [None, Some(100)] {
            for headless in [false, true] {
                let read = Read::SessionList {
                    directory: home.path().into(),
                    headless,
                    after: None,
                    limit,
                    task: None,
                };
                let filter = read.session_filter(home.path()).unwrap();
                let sessions = loopflow_execution::store::session_projection::session_summaries(
                    &store, &filter, 0,
                )
                .unwrap();
                let mut ids = sessions
                    .iter()
                    .map(|row| row.id.as_str())
                    .collect::<Vec<_>>();
                ids.sort_unstable();
                assert_eq!(
                    ids,
                    if headless {
                        vec!["headless", "interactive"]
                    } else {
                        vec!["interactive"]
                    }
                );
            }
        }
    }

    #[test]
    fn a_read_without_a_subscription_cannot_be_subscribed() {
        let error = Read::MachineDoctor.subscribe(None, || {}).unwrap_err();
        assert!(error.to_string().contains("--subscribe"), "{error}");
    }

    mod peer_readings {
        use super::Read;
        use loopflow_execution::durable::{TaskExecutionRoute, TaskExecutionSource};
        use loopflow_model::durable::{Machine, MachineId, RepositoryId, TaskId};
        use serde_json::json;
        use std::path::PathBuf;

        fn mini() -> Machine {
            Machine {
                label: Some("mini".into()),
                id: MachineId::new(),
                route: "mini.local".into(),
                created_at: time::OffsetDateTime::UNIX_EPOCH,
                observed_at: time::OffsetDateTime::UNIX_EPOCH,
            }
        }

        fn route_on(machine: &Machine) -> TaskExecutionRoute {
            TaskExecutionRoute {
                task_id: TaskId::new(),
                peer_selector: "LOO-7".into(),
                checkout: None,
                // Bound on no Machine this test can see.
                repository_id: RepositoryId::from_raw("github.com/acme/unbound"),
                machine_id: machine.id.clone(),
                source: TaskExecutionSource::Owner,
            }
        }

        fn task_get() -> Read {
            Read::TaskGet {
                directory: PathBuf::from("/src/app"),
                issue: "LOO-7".into(),
            }
        }

        #[test]
        fn a_task_pane_names_the_task_and_owner_as_this_machine_does() {
            let machine = mini();
            let route = route_on(&machine);
            let mut pane = json!({
                "task": { "id": "TASK_PEER", "identifier": "LOO-7" },
                "row": {
                    "task": { "id": "TASK_PEER", "repo": "/workspace/app" },
                    "runtime": { "work_id": "TASK_PEER" },
                    "reference": { "workspace": { "machine_label": "self", "worktree": "/workspace/app.loo-7" } },
                },
            });
            task_get()
                .localize_peer_reading(&mut pane, &machine, &route)
                .unwrap();
            let local = route.task_id.to_string();
            assert_eq!(pane["task"]["id"], local.as_str());
            assert_eq!(pane["row"]["task"]["id"], local.as_str());
            assert_eq!(pane["row"]["runtime"]["work_id"], local.as_str());
            assert_eq!(
                pane["row"]["reference"]["workspace"]["machine_label"],
                "mini"
            );
            // The files live in the owner's checkout; an unbound repository
            // has no path here but the owner's.
            assert_eq!(
                pane["row"]["reference"]["workspace"]["worktree"],
                "/workspace/app.loo-7"
            );
            assert_eq!(pane["row"]["task"]["repo"], "/workspace/app");
        }

        #[test]
        fn a_selected_reading_gains_no_keys() {
            let machine = mini();
            let route = route_on(&machine);
            let mut selected =
                json!({ "row": { "task": { "identifier": "LOO-7" } }, "task": null });
            task_get()
                .localize_peer_reading(&mut selected, &machine, &route)
                .unwrap();
            assert_eq!(
                selected,
                json!({ "row": { "task": { "identifier": "LOO-7" } }, "task": null })
            );
        }

        #[test]
        fn a_task_detail_opens_its_current_session_with_this_machines_lf_open() {
            let machine = mini();
            let route = route_on(&machine);
            let mut detail = json!({
                "task": { "id": "TASK_PEER" },
                "current_session": {
                    "id": "session_1",
                    "task_ids": ["TASK_PEER"],
                    "outlook": { "command": "lf resume --task LOO-7" },
                },
            });
            Read::TaskDetail {
                directory: PathBuf::from("/src/app"),
                issue: "LOO-7".into(),
            }
            .localize_peer_reading(&mut detail, &machine, &route)
            .unwrap();
            let local = route.task_id.to_string();
            assert_eq!(detail["task"]["id"], local.as_str());
            assert_eq!(
                detail["current_session"]["outlook"]["command"],
                "lf open --task LOO-7"
            );
            assert_eq!(detail["current_session"]["task_ids"][0], local.as_str());
        }

        #[test]
        fn a_files_watch_names_the_owner_by_this_machines_label() {
            let machine = mini();
            let route = route_on(&machine);
            let mut files = json!({ "task_id": "TASK_PEER", "machine_label": null, "root": "/workspace/app.loo-7" });
            Read::TaskFilesWatch {
                issue: "LOO-7".into(),
            }
            .localize_peer_reading(&mut files, &machine, &route)
            .unwrap();
            assert_eq!(
                files,
                json!({ "task_id": route.task_id.to_string(), "machine_label": "mini", "root": "/workspace/app.loo-7" })
            );
        }
    }
}
