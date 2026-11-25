/// The caller's prompt and provider flags for the Task Graph it launches.
#[doc(hidden)]
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use loopflow_storage::work::selection::DropSide;

pub mod aliases;
mod desktop_inputs;
pub use desktop_inputs::{PortfolioArgs, ShellArgs};
pub mod commands;
pub mod navigation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BrowserMode {
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DiffContext {
    Files,
    Patch,
    Both,
    None,
}

/// `--select` on a read that takes `--subscribe`: the dotted paths its JSON
/// answer and frames keep. Only those reads declare it, so no other command
/// accepts a flag it would ignore.
#[derive(Debug, Clone, Default, clap::Args)]
pub struct Selection {
    /// Keep only these dotted paths of the JSON answer, comma-separated (`task.identifier,pr.url`)
    #[arg(long = "select", value_delimiter = ',', value_name = "PATHS")]
    pub select: Vec<String>,
}

/// The `--select` paths of the read `args` names, if it declares and sets them.
pub fn selected_paths(args: &[String]) -> Option<Vec<String>> {
    use clap::CommandFactory;
    let matches = Cli::command().try_get_matches_from(args).ok()?;
    let mut level = &matches;
    loop {
        if let Ok(Some(paths)) = level.try_get_many::<String>("select") {
            return Some(paths.cloned().collect());
        }
        level = level.subcommand()?.1;
    }
}

#[derive(Parser, Debug, Default)]
#[command(name = "lf", bin_name = "lf", disable_help_subcommand = true)]
#[command(about = "Open Loopflow or run its CLI")]
#[command(version = crate::build_info::VERSION_LINE)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Preview assembled agent input without launching or changing Work
    #[arg(long)]
    pub context: bool,

    /// Explain this invocation's Work selection without executing it
    #[arg(long)]
    pub explain: bool,

    /// Emit invocation previews as JSON
    #[arg(long)]
    pub json: bool,

    /// Docs paths, globs, or directories to include in context
    #[arg(long = "docs", value_delimiter = ',')]
    pub docs: Vec<String>,

    /// Include clipboard content in prompt
    #[arg(short = 'c', long = "clipboard")]
    pub clipboard: bool,

    /// Select this conversation or all agent steps of this Graph (harness[:model])
    #[arg(short = 'a', long = "agent", global = true)]
    pub agent: Option<String>,

    /// Override a typed preference for this invocation (KEY=YAML); repeatable
    #[arg(long = "setting", global = true)]
    pub settings: Vec<String>,

    /// Read the skill and its arguments from this captured invocation file, as a Graph step does
    #[arg(long, value_name = "PATH")]
    pub skill_input: Option<std::path::PathBuf>,
    /// Require the final answer to match this JSON schema, as a deciding Graph step does
    #[arg(long, value_name = "JSON", value_parser = |text: &str| serde_json::from_str::<serde_json::Value>(text))]
    pub output_schema: Option<serde_json::Value>,
    #[arg(skip)]
    pub resolved_invocation: Option<loopflow_execution::skills::invocation::SkillInvocation>,

    /// Skip permission prompts in this conversation, over a configured `yolo: false`
    #[arg(long)]
    pub yolo: bool,

    /// Run interactively
    #[arg(short = 'i', long = "interactive", conflicts_with = "batch")]
    pub interactive: bool,

    /// Run headless: print the output and return when the work ends
    #[arg(short = 'b', long = "batch")]
    pub batch: bool,

    /// Show context accounting and diagnostic logs
    #[arg(short = 'v', long, global = true)]
    pub verbose: bool,

    /// Override Chrome integration; omission inherits configuration
    #[arg(long, value_enum)]
    pub chrome: Option<BrowserMode>,

    /// Select changed-code context; omission inherits configuration
    #[arg(long, value_enum)]
    pub diff: Option<DiffContext>,

    /// Run the command on this saved machine in its repository
    #[arg(long, value_name = "LABEL_OR_ID")]
    pub machine: Option<String>,

    /// Select a repository by name under repo_root (default ~/src), or by explicit path
    #[arg(long, value_name = "NAME_OR_PATH", conflicts_with = "repository")]
    pub repo: Option<String>,

    /// Select a registered repository plan on this Machine, not a clone name
    #[arg(long = "repo-id", value_name = "ID")]
    pub repository: Option<loopflow_model::durable::RepositoryId>,

    /// Forward the SSH agent to the selected machine
    #[arg(long, requires = "machine")]
    pub forward_agent: bool,

    /// Add Wave context and identity without changing the working directory
    #[arg(long, value_name = "WAVE")]
    pub wave: Option<String>,

    /// Execute in this Task's checkout
    #[arg(long, value_name = "TASK", conflicts_with = "wt")]
    pub task: Option<String>,

    /// Execute in an existing worktree by name or branch
    #[arg(long, value_name = "NAME", conflicts_with = "task")]
    pub wt: Option<String>,

    /// Run in this directory; Work still comes from `--task` or `--wave`
    #[arg(long, value_name = "PATH", conflicts_with = "wt")]
    pub cwd: Option<PathBuf>,

    /// The conversation `lf -b steer SESSION TEXT` continues.
    #[arg(skip)]
    pub resume: Option<String>,

    /// The prepared capture a conversation's fresh launch publishes.
    #[arg(skip)]
    pub prepared_capture: Option<String>,

    /// The Session launch lock a conversation's fresh launch holds until its
    /// provider is recorded.
    #[arg(skip)]
    pub launch_lock: Option<loopflow_execution::ops::human_session::LaunchLock>,
}

impl Cli {
    pub fn setting_values(&self) -> anyhow::Result<serde_yaml_ng::Value> {
        let mut value = serde_yaml_ng::Value::Mapping(Default::default());
        for setting in &self.settings {
            let (key, text) = setting
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("--setting expects KEY=YAML"))?;
            let parts: Vec<_> = key.split('.').collect();
            if let Some(fields) = loopflow_execution::config::keys::tracker_path(&parts) {
                loopflow_execution::config::keys::edit_tracker(
                    &mut value,
                    fields,
                    Some(serde_yaml_ng::from_str(text)?),
                )?;
                continue;
            }
            let parsed = loopflow_execution::config::settings::parse_value(key, text)?;
            let typed = loopflow_execution::config::keys::typed_key(key);
            loopflow_execution::config::settings::insert(
                &mut value,
                &typed.split('.').collect::<Vec<_>>(),
                parsed,
            );
        }
        for (key, enabled) in [
            ("yolo", self.yolo.then_some(true)),
            ("context.paste", self.clipboard.then_some(true)),
            ("chrome", self.chrome.map(|mode| mode == BrowserMode::On)),
            (
                "context.diff",
                self.diff
                    .map(|mode| matches!(mode, DiffContext::Patch | DiffContext::Both)),
            ),
            (
                "context.diff_files",
                self.diff
                    .map(|mode| matches!(mode, DiffContext::Files | DiffContext::Both)),
            ),
        ] {
            if let Some(enabled) = enabled {
                loopflow_execution::config::settings::insert(
                    &mut value,
                    &key.split('.').collect::<Vec<_>>(),
                    enabled.into(),
                );
            }
        }
        if let Some(agent) = &self.agent {
            loopflow_execution::config::settings::insert(
                &mut value,
                &["agent"],
                agent.clone().into(),
            );
        }
        if !self.docs.is_empty() {
            loopflow_execution::config::settings::insert(
                &mut value,
                &["context", "docs"],
                serde_yaml_ng::to_value(&self.docs)?,
            );
        }
        Ok(value)
    }

    /// The global `--repo` or `--repository` selector names a repository.
    pub fn selects_repository(&self) -> bool {
        self.repo.is_some() || self.repository.is_some()
    }

    /// The command merges a PR, or asks GitHub to merge it.
    pub fn merges(&self) -> bool {
        matches!(
            &self.command,
            Some(Commands::Pr {
                cmd: Some(PrCommand::Land { .. } | PrCommand::Arm { .. })
            })
        )
    }

    /// `lf task auto on`: the Task's run, asked for auto.
    pub fn turns_auto_on(&self) -> bool {
        matches!(
            &self.command,
            Some(Commands::Task {
                cmd: TaskCommand::Auto {
                    cmd: AutoCommand::On { .. }
                }
            })
        )
    }

    pub fn may_launch_agent(&self) -> bool {
        match &self.command {
            None
            | Some(Commands::External(_) | Commands::Inline { .. })
            | Some(Commands::Discord { .. }) => true,
            Some(Commands::Skill { cmd }) => matches!(cmd, SkillCommand::External(_)),
            Some(Commands::Graph { cmd }) => {
                matches!(cmd, GraphCommand::Run | GraphCommand::External(_))
            }
            Some(Commands::Task { cmd }) => matches!(
                cmd,
                TaskCommand::Run { .. }
                    | TaskCommand::Auto {
                        cmd: AutoCommand::On { .. }
                    }
            ),
            Some(Commands::Session { cmd }) => matches!(
                cmd,
                SessionCommand::Resume {
                    connection: SessionConnectionArgs { prepare: false, .. },
                    ..
                } | SessionCommand::New {
                    connection: SessionConnectionArgs { prepare: false, .. },
                    ..
                } | SessionCommand::History {
                    cmd: Some(commands::history::HistoryCommand::Replay { .. }),
                    ..
                } | SessionCommand::Move { .. }
            ),
            Some(Commands::Commit { message, .. }) => message.is_none(),
            Some(Commands::Sync(args)) => !args.plan && !args.abort && !args.manual,
            Some(Commands::Pr { cmd }) => matches!(
                cmd,
                Some(
                    PrCommand::Open { .. }
                        | PrCommand::Publish { .. }
                        | PrCommand::Submit { .. }
                        | PrCommand::Arm { .. }
                        | PrCommand::Land { .. }
                )
            ),
            Some(Commands::Repo {
                cmd: Some(RepoCommand::Release { cmd }),
                ..
            }) => matches!(
                cmd,
                ReleaseCommand::Run { .. }
                    | ReleaseCommand::Bump { .. }
                    | ReleaseCommand::Notes { .. }
            ),
            Some(_) => false,
        }
    }

    /// A provider hook: the harness runs it on the agent's behalf, once per
    /// turn, event or tool call. Includes hooks an earlier lf left behind.
    pub fn harness_callback(&self) -> bool {
        match &self.command {
            Some(Commands::ContextBlock { .. }) => true,
            Some(Commands::External(parts)) => parts[0].starts_with("__"),
            _ => false,
        }
    }

    /// Reject argument combinations the derive cannot express, as the usage
    /// errors they are, before anything runs.
    pub fn checked(mut self) -> Result<Self, clap::Error> {
        if let Some(Commands::Open(open)) = &mut self.command {
            if let Some(machine) = self.machine.take() {
                if open.selection.machine.replace(machine).is_some() {
                    return Err(clap::Error::raw(
                        clap::error::ErrorKind::ArgumentConflict,
                        "select a Machine only once",
                    ));
                }
            }
            if open.selection.machine.is_some()
                && (open.selection.session.is_none()
                    || self.task.is_some()
                    || self.wave.is_some()
                    || open.selection.url.is_some())
            {
                return Err(clap::Error::raw(clap::error::ErrorKind::ArgumentConflict,
                    "use --machine with --session for a Session on that Machine; a Task selects its own Machine"));
            }
        }
        if let Some(Commands::Session { cmd }) = &self.command {
            let selected = match cmd {
                SessionCommand::Resume { id, connection, .. }
                | SessionCommand::New { id, connection, .. } => Some((id.as_deref(), connection)),
                _ => None,
            };
            if let Some((id, connection)) = selected {
                connection.target(&self, id).map_err(|error| {
                    clap::Error::raw(clap::error::ErrorKind::ArgumentConflict, error.to_string())
                })?;
            }
        }
        // `lf task auto on ISSUE` selects its Task as `--task` does: it runs it.
        if let Some(Commands::Task {
            cmd:
                TaskCommand::Auto {
                    cmd: AutoCommand::On { issue, .. },
                },
        }) = &mut self.command
        {
            if let Some(issue) = issue.take() {
                if self.task.as_ref().is_some_and(|task| task != &issue) {
                    return Err(clap::Error::raw(
                        clap::error::ErrorKind::ArgumentConflict,
                        format!(
                            "conflicting Task selections: {} and {issue}\n",
                            self.task.as_deref().unwrap_or_default()
                        ),
                    ));
                }
                self.task = Some(issue);
            }
        }
        // Work routing consumes the same explicit selector in either flag position.
        if let Some(Commands::Session {
            cmd: SessionCommand::Resume { connection, .. } | SessionCommand::New { connection, .. },
        }) = &mut self.command
        {
            if connection.task.is_some() {
                self.task = connection.task.take();
            }
            if connection.wave.is_some() {
                self.wave = connection.wave.take();
            }
        }
        if self.json && !self.context && !self.explain {
            return Err(clap::Error::raw(clap::error::ErrorKind::ArgumentConflict,
                "root --json requires --context or --explain; use the command's own --json when available\n"));
        }
        Ok(self)
    }

    /// Forward prompt and provider options to a captured step. Work and the
    /// definition remain captured; Work resolves from the declaration or checkout.
    #[doc(hidden)]
    pub fn step_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        for (flag, enabled) in [
            ("--batch", self.batch && !self.interactive),
            ("--interactive", self.interactive),
            ("--clipboard", self.clipboard),
            ("--yolo", self.yolo),
            ("--verbose", self.verbose),
        ] {
            if enabled {
                args.push(flag.to_string());
            }
        }
        for (flag, values) in [("--setting", &self.settings), ("--docs", &self.docs)] {
            for value in values {
                args.extend([flag.to_string(), value.clone()]);
            }
        }
        for (flag, value) in [
            (
                "--chrome",
                self.chrome.and_then(|value| value.to_possible_value()),
            ),
            (
                "--diff",
                self.diff.and_then(|value| value.to_possible_value()),
            ),
        ] {
            if let Some(value) = value {
                args.extend([flag.to_string(), value.get_name().to_string()]);
            }
        }
        if let Some(agent) = &self.agent {
            args.extend(["--agent".to_owned(), agent.clone()]);
        }
        if let Some(input) = &self.skill_input {
            args.extend([
                "--skill-input".to_string(),
                input.to_string_lossy().into_owned(),
            ]);
        }
        if let Some(schema) = &self.output_schema {
            args.extend(["--output-schema".to_string(), schema.to_string()]);
        }
        args
    }

    pub fn process_options(&self) -> Self {
        Self {
            command: None,
            context: self.context,
            explain: self.explain,
            json: self.json,
            docs: self.docs.clone(),
            clipboard: self.clipboard,
            agent: self.agent.clone(),
            settings: self.settings.clone(),
            skill_input: self.skill_input.clone(),
            output_schema: self.output_schema.clone(),
            resolved_invocation: self.resolved_invocation.clone(),
            yolo: self.yolo,
            interactive: self.interactive,
            batch: self.batch,
            verbose: self.verbose,
            chrome: self.chrome,
            diff: self.diff,
            machine: self.machine.clone(),
            repository: self.repository.clone(),
            repo: self.repo.clone(),
            forward_agent: self.forward_agent,
            wave: self.wave.clone(),
            task: self.task.clone(),
            wt: self.wt.clone(),
            cwd: self.cwd.clone(),
            resume: self.resume.clone(),
            prepared_capture: self.prepared_capture.clone(),
            launch_lock: self.launch_lock.clone(),
        }
    }

    /// The most specific Work selected for direct execution.
    pub fn work_subject_selector(&self) -> Option<String> {
        self.task
            .as_ref()
            .map(|task| format!("task:{task}"))
            .or_else(|| self.wave.as_ref().map(|wave| format!("wave:{wave}")))
    }
}

#[derive(Subcommand, Debug)]
pub enum ScratchCommand {
    /// Remove scratch files added in this branch's diff, keeping existing files and edits
    Clear,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Concrete work and Task lifecycle
    Task {
        #[command(subcommand)]
        cmd: TaskCommand,
    },
    /// Manage Wave identity and planning
    Wave {
        #[command(subcommand)]
        cmd: WaveCommand,
    },
    /// Direct agents: a Session, a Task's Sessions, or with `--wave` or `--repo` every Session there
    ///
    /// The steer is saved; each Session it applies to reads it after its next
    /// tool call, or when it is next woken or prompted. A stopped Session it
    /// names, or a Task's stopped conversation, is started by the Machine
    /// server to read it; with `-b`, a stopped Session reads it in one turn
    /// here, and the reply prints. A Task steer also joins the Task's thread.
    /// `lf session steers SESSION` shows what a Session has read.
    #[command(
        allow_missing_positional = true,
        after_help = "lf steer SESSION 'skip the vendored code'\nlf -b steer SESSION 'what changed?'\nlf steer LOO-12 'keep the public API'\nlf --wave parser steer 'prioritize the tokenizer'\nlf --repo . steer 'hold merges until the release is cut'"
    )]
    Steer {
        /// A Session or Task ID; omit it with `--wave` or `--repo`
        target: Option<String>,
        message: String,
        /// Print the saved steer or the Task delivery receipt
        #[arg(long = "json", id = "steer_json")]
        json: bool,
    },
    /// Inspect and continue Sessions
    Session {
        #[command(subcommand)]
        cmd: SessionCommand,
    },
    /// Open one conversation here, in a sibling tab, or a repository/Wave overview
    Open(commands::open::OpenArgs),
    /// Run an inline prompt
    #[command(name = ":")]
    Inline {
        #[arg(trailing_var_arg = true)]
        prompt: Vec<String>,
    },
    /// Run or inspect authored graphs
    Graph {
        #[command(subcommand)]
        cmd: GraphCommand,
    },
    /// Run a skill explicitly
    Skill {
        #[command(subcommand)]
        cmd: SkillCommand,
    },
    /// Select Work and control Loopflow Desktop panes
    #[command(
        after_help = "Targets: root --task/--wave set the base view; -p names a part of it (shell:2, a tile 3 or LOO-519, LOO-519/shell:2); --window picks the window. Each part resolves by full ID, case-insensitive label, 1-based position, then ID suffix. Without them a command acts on the terminal it runs in. Scripts should pass the IDs receipts return; positions are for people.\n\nExit status: 0 success, 1 failure, 2 bad arguments, expression or verb arguments, 3 missing or ambiguous Task, window, pane or tab (nothing changed), 4 Desktop unreachable (not running, not macOS, or not answering its socket), 130 interrupted."
    )]
    Desktop {
        #[command(subcommand)]
        cmd: DesktopCommand,
    },
    /// Plans across Waves: what Desktop shows of each Wave's Project and Tasks
    Plan {
        #[command(subcommand)]
        cmd: PlanCommand,
    },
    /// Pull request lifecycle
    Pr {
        #[command(subcommand)]
        cmd: Option<PrCommand>,
    },
    /// Commit changes and push unless on the default branch
    Commit {
        #[arg(short = 'm', long = "message")]
        message: Option<String>,
        /// Keep the commit local
        #[arg(long)]
        no_push: bool,
        #[arg(long = "no-add")]
        no_add: bool,
        /// Commit only these paths, preserving other staged and unstaged edits
        #[arg(value_name = "PATH", conflicts_with = "no_add")]
        paths: Vec<String>,
        /// Print what the commit left
        #[arg(long)]
        json: bool,
        /// Commit everything a finished skill step left, titled `lf <SKILL>: …`, and push
        #[arg(long, value_name = "SKILL", conflicts_with_all = ["message", "no_add", "no_push", "paths", "json"])]
        step: Option<String>,
    },
    /// Merge upstream into the current branch (default: main or stack parent)
    Sync(SyncArgs),
    /// Worktree operations
    Wt {
        #[command(subcommand)]
        cmd: WtCommand,
    },
    /// Serve Desktop and other clients on this host
    Server {
        #[command(subcommand)]
        cmd: ServerCommand,
    },
    /// Keep this build's Task and Wave terminals; started only by the server
    Holder {
        #[command(subcommand)]
        cmd: HolderCommand,
    },
    /// Name, connect, pool and rent Machines
    Machine {
        #[command(subcommand)]
        cmd: MachineCommand,
    },
    /// This repository: its identity and checkout, its tracker, releases, CI,
    /// people and size
    #[command(args_conflicts_with_subcommands = true)]
    Repo {
        /// Print the identity as JSON
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        cmd: Option<RepoCommand>,
    },
    /// Read and declare Loopflow settings
    Config {
        #[command(subcommand)]
        cmd: ConfigCommand,
    },
    /// Manage the installed Loopflow release and exported skills
    #[command(name = "self")]
    Self_ {
        #[command(subcommand)]
        cmd: SelfCommand,
    },
    /// Discover commands, skills, and graphs
    List {
        path: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Explain a command, skill, or graph without launching it
    Help {
        path: Vec<String>,
        #[arg(long)]
        all: bool,
        /// Describe installed command arguments and available result schemas
        #[arg(long)]
        json: bool,
    },
    /// Read embedded Loopflow user documentation offline
    Docs {
        /// Topic slug (omit to list all topics)
        topic: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show Wave memory size targets, their sources, and current usage
    Context {
        #[arg(long)]
        json: bool,
        /// Inspect a Wave's local authored context
        #[arg(long, conflicts_with = "task")]
        wave: Option<String>,
        /// Inspect a Task's checkout and Wave context
        #[arg(long)]
        task: Option<String>,
    },
    /// Manage working notes
    Scratch {
        #[command(subcommand)]
        cmd: ScratchCommand,
    },
    /// Bridge new Discord messages to finite Wave Sessions
    Discord {
        #[command(subcommand)]
        cmd: DiscordCommand,
    },
    /// Internal provider turn-boundary hook: context at startup and compaction,
    /// steers at tool and prompt boundaries, Linear fetch at turn end.
    // Running providers retain this older Stop spelling across lf upgrades.
    #[command(name = "__context-block", alias = "__planning-check", hide = true)]
    ContextBlock {
        #[arg(long)]
        delivery: Option<std::path::PathBuf>,
        #[arg(long, value_enum, default_value = "stop")]
        moment: loopflow_execution::context_block::ContextMoment,
        /// The harness whose provider sent the callback, when it is not in
        /// Claude's shape.
        #[arg(long, value_parser = parse_hook_harness)]
        harness: Option<loopflow_harness::HarnessKind>,
    },
    /// External: skill/graph name (when no subcommand matches)
    #[command(external_subcommand)]
    External(Vec<String>),
}

fn parse_hook_harness(name: &str) -> Result<loopflow_harness::HarnessKind, String> {
    loopflow_harness::HarnessKind::parse(name).ok_or_else(|| format!("unknown harness {name}"))
}

/// `repository.*`: one repository's planning exchange, releases, CI, people,
/// size and checkout.
#[derive(Subcommand, Debug)]
pub enum RepoCommand {
    /// Show how the repository's plan travels: its tracker and open and lost actions, or its Git remote and hub
    #[command(args_conflicts_with_subcommands = true)]
    Tracker {
        #[command(subcommand)]
        cmd: Option<RepoTrackerCommand>,
        #[arg(long)]
        json: bool,
        /// Print a JSON line now and each time the status changes, until stdin closes
        #[arg(short = 's', long, requires = "json")]
        subscribe: bool,
        #[command(flatten)]
        selection: crate::lf::Selection,
    },
    /// Print the selected repository's checkout on this Machine
    Path,
    /// Release operations (run, check, notes, bump, tag, status, issues, regress, promote)
    Release {
        #[command(subcommand)]
        cmd: ReleaseCommand,
    },
    /// Show how failed CI is detected, repaired, and landed across this Machine
    Ci {
        /// Relative window (7d, 24h, 30m) or RFC3339 start
        #[arg(long, default_value = "7d")]
        since: String,
        /// Scope to one Wave
        #[arg(long)]
        wave: Option<String>,
        /// Scope to one GitHub owner/repo
        #[arg(long)]
        repo: Option<String>,
        /// Emit the complete incident report as JSON
        #[arg(long)]
        json: bool,
    },
    /// List the people who own this repository's Tasks, with their GitHub identity
    People {
        #[arg(long)]
        json: bool,
    },
    /// Measure this codebase: lines and tokens per directory (tracked files only)
    Tokens {
        /// Emit as JSON
        #[arg(long)]
        json: bool,
        /// Walk git history instead: the codebase's size on each day it changed
        #[arg(long, value_name = "DAYS")]
        days: Option<u32>,
    },
}

#[derive(Subcommand, Debug)]
pub enum DesktopCommand {
    /// Desktop's Task shell viewer: the local server runs the shell wherever the Task runs; does not contact the GUI.
    #[command(hide = true)]
    Shell {
        #[command(flatten)]
        args: ShellArgs,
    },
    /// Show a Work selection; `list` and `show` read what views show without opening Desktop
    #[command(args_conflicts_with_subcommands = true)]
    View {
        /// wave/task/query/kind/state expression; omitted uses the repository default
        expression: Option<String>,
        /// Print the opening receipt as JSON
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        cmd: Option<DesktopViewCommand>,
    },
    /// Saved Dashboards: their queries, pins, names, slots and tiles, with revision checking
    Dashboard {
        #[command(subcommand)]
        cmd: DashboardCommand,
    },
    /// Select repositories for the Machine-local portfolio
    Portfolio {
        #[command(subcommand)]
        cmd: Option<PortfolioArgs>,
        #[arg(long, global = true, id = "portfolio_json")]
        json: bool,
    },
    /// Read a saved Dashboard (its arrangement and slots) as JSON; `lf desktop dashboard slots` replaces the slots
    Layout {
        /// query:ID
        #[arg(value_name = "QUERY")]
        work: String,
    },

    /// List the window's Tasks by position (as ⌘1–9 select them) and each Task's panes
    Ls {
        /// Desktop window: ID, position or ID suffix (default: this terminal's window)
        #[arg(long, value_name = "WINDOW")]
        window: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Read actual windows and panes without launching, focusing, or acquiring clients
    State {
        #[arg(long)]
        json: bool,
    },
    /// List the verbs Desktop performs, with their arguments and default keys
    Actions {
        #[arg(long)]
        json: bool,
    },
    /// Stream window, pane and destination changes; the first change is every window
    Events {
        /// One change per line as JSON
        #[arg(long)]
        json: bool,
    },
    /// Wait for a shell tab's command to finish (exits with its status), else for the pane to appear
    #[command(
        after_help = "Exit status: the command's status when waiting on a shell tab, 1 when --for elapses, otherwise as for lf desktop."
    )]
    Wait {
        /// Longest wait, e.g. 30s, 5m (default: no limit)
        #[arg(long = "for", value_name = "DURATION", value_parser = loopflow_execution::config::parse_duration)]
        timeout: Option<std::time::Duration>,
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Go back, as ⌘[ and the crumb chevron do
    Back {
        #[command(flatten)]
        target: DesktopWindowArgs,
    },
    /// Go forward, as ⌘] does
    Forward {
        #[command(flatten)]
        target: DesktopWindowArgs,
    },
    /// Show the window's Nth Working Task, as ⌘N does
    Goto {
        index: u32,
        #[command(flatten)]
        target: DesktopWindowArgs,
    },
    /// Show the next Working Task, as ⌃⇥ does
    Next {
        #[command(flatten)]
        target: DesktopWindowArgs,
    },
    /// Show the previous Working Task, as ⌃⇧⇥ does
    Previous {
        #[command(flatten)]
        target: DesktopWindowArgs,
    },
    /// Show the Working Tasks Dashboard
    WorkingTasks {
        #[command(flatten)]
        target: DesktopWindowArgs,
    },
    /// Enter a member of the shown Dashboard as full Work, as a tile click does
    Enter {
        /// Tile on the shown Dashboard or Wave view, as `-p` names it: position, ID, label or ID suffix
        tile: String,
        #[command(flatten)]
        target: DesktopWindowArgs,
    },
    /// Perform any registry action by name, as `lf desktop actions` lists them
    #[command(
        after_help = "Arguments are KEY=VALUE pairs. `-p` may name a view part \
(`backlog`, `session`, `tiles` on a Wave view), which must accept the action:\n  \
lf desktop do page -p tiles page=2\n  lf desktop do zoom -p backlog on=true"
    )]
    Do {
        /// Action name from `lf desktop actions`
        action: String,
        /// Action arguments as KEY=VALUE
        args: Vec<String>,
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Insert literal text at a terminal's cursor (stdin when omitted); never submits
    Send {
        /// Unicode text without control characters (use send-key for Enter/Tab)
        text: Option<String>,
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Send one key to a terminal, without following focus
    SendKey {
        #[arg(value_enum)]
        key: commands::desktop::DesktopKey,
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Print a terminal's text, without following focus
    Capture {
        #[arg(long, value_enum, default_value = "screen")]
        region: commands::desktop::DesktopTextRegion,
        /// Drop trailing spaces and blank lines
        #[arg(long)]
        trim: bool,
        #[arg(long, default_value_t = 65536)]
        max_bytes: usize,
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Add a shell tab at the Task's location and type LINE into it, as if typed; focus stays put
    Run {
        /// One shell line, e.g. 'cargo test -p loopflow'; omitted opens an empty shell
        line: Option<String>,
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Zoom a pane, or return it to the layout, without changing selection
    Zoom {
        #[arg(long, conflicts_with = "off", required_unless_present = "off")]
        on: bool,
        #[arg(long)]
        off: bool,
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Focus a pane, or the pane in a direction, opening its retained workspace when needed
    Focus {
        #[arg(long, value_enum)]
        direction: Option<commands::desktop::DesktopDirection>,
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Hide a pane without closing its Session or process; on a Dashboard, hide a tile from its shared record
    Hide {
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Show a hidden pane without selecting Work or acquiring a client; on a Dashboard, a hidden tile
    Show {
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Close a shell or browser tab, as ⌘W does; a session pane hides
    Close {
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Undo the Task's last close, as ⇧⌘T does
    Reopen {
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Move a pane beside another pane of the same Task; on a Dashboard or Wave view, a tile into another's place
    Move {
        /// Pane to place it beside, or tile whose place it takes, as for -p
        #[arg(long)]
        destination: String,
        /// Split axis for a pane; tiles take the destination's place
        #[arg(long, value_enum)]
        axis: Option<commands::desktop::DesktopSplitAxis>,
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Resize the divider between two panes or Dashboard tiles; ratio is the addressed one's share
    Resize {
        /// The other pane or tile, as for -p
        #[arg(long)]
        toward: String,
        #[arg(long)]
        ratio: f64,
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Show the Task's Files, optionally presenting a checkout-relative path
    Files {
        #[arg(value_name = "PATH[:LINE[-END]]")]
        path: Option<String>,
        #[arg(long, requires = "path", value_parser = ["read", "diff"])]
        mode: Option<String>,
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Show the Task's Graph runs in the browser
    GraphLog {
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Open an http(s) page in a browser tab
    Browse {
        url: String,
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Go back in a web page
    PageBack {
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Go forward in a web page
    PageForward {
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Reload a web page, as ⌘R does
    Reload {
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Show another of the Task's conversations in its session pane
    Switch {
        /// Session ID
        session: String,
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
}

impl DesktopCommand {
    /// Reads of saved intent and Work that need no Desktop and change nothing.
    pub fn is_read(&self) -> bool {
        matches!(
            self,
            Self::View { .. } | Self::Portfolio { cmd: None, .. } | Self::Layout { .. }
        )
    }
}

#[derive(Subcommand, Debug)]
pub enum DesktopViewCommand {
    /// Print the repository's default and saved Dashboard plans as JSON
    List {
        /// Apply one selection across every portfolio repository
        #[arg(long)]
        all: bool,
        /// Selection to apply with --all; omitted uses each repository's default
        #[arg(long, requires = "all")]
        expression: Option<String>,
        #[arg(long, required = true)]
        json: bool,
        /// Print a JSON line now and each time the plans change, until stdin closes
        #[arg(short = 's', long)]
        subscribe: bool,
        #[command(flatten)]
        selection: crate::lf::Selection,
    },
    /// Print what a view shows as JSON, without opening Desktop or starting a provider
    Show {
        /// wave/task/query/kind/state expression; omitted uses the repository default
        expression: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum DashboardCommand {
    /// Create or edit a Dashboard query with revision checking; prints the Dashboard as JSON
    Query {
        /// Dashboard name or query:ID
        container: String,
        /// Selection expression, as for `lf desktop view`
        expression: String,
        /// Last observed revision; omit only when creating a new Dashboard
        #[arg(long)]
        revision: Option<i64>,
    },
    /// Pin a Dashboard in repository discovery; prints the Dashboard as JSON
    Pin {
        /// Dashboard name or query:ID
        query: String,
        /// The Dashboard's last read revision; a stale one writes nothing
        #[arg(long)]
        revision: i64,
    },
    /// Unpin a Dashboard without changing its membership or arrangement; prints the Dashboard as JSON
    Unpin {
        /// Dashboard name or query:ID
        query: String,
        /// The Dashboard's last read revision; a stale one writes nothing
        #[arg(long)]
        revision: i64,
    },
    /// Rename a Dashboard, or a Task by task:<durable-id>, through its shared owner; prints the owner's record as JSON
    Rename {
        /// query:ID or task:<durable-id>
        target: String,
        /// The new name, written to the shared owner so labels and crumbs follow
        name: String,
        /// The target's last read revision; a stale one writes nothing
        #[arg(long)]
        revision: u64,
    },
    /// Replace a Dashboard's slots from JSON on stdin (null for the default); prints the Dashboard as JSON
    Slots {
        /// Dashboard name or query:ID
        query: String,
        /// The Dashboard's last read revision; a stale one writes nothing
        #[arg(long)]
        revision: i64,
    },
    /// Hide a tile from a saved board's record; prints the record
    Hide {
        /// query:ID/<tile> or working-tasks/<tile>
        tile: String,
    },
    /// Show a hidden tile on a saved board's record; prints the record
    Show {
        /// query:ID/<tile> or working-tasks/<tile>
        tile: String,
    },
    /// Move a tile into another's place on a saved board's record; prints the record
    Move {
        /// query:ID/<tile> or working-tasks/<tile>
        tile: String,
        /// Tile whose place it takes
        #[arg(long)]
        destination: String,
        /// Place the tile before or after the destination in the current order
        #[arg(long, value_enum)]
        side: Option<DropSide>,
    },
    /// Resize the divider between two tiles on a saved Dashboard; ratio is the addressed one's share
    Resize {
        /// query:ID/<tile>
        tile: String,
        /// The other tile
        #[arg(long)]
        toward: String,
        #[arg(long)]
        ratio: f64,
    },
}

// ── Which commands change state ─────────────────────────────────────────────
// Only a command that changes state writes a journal trace. A read (status, list,
// show, diff, file, every `--subscribe` reader) writes none, and it never wakes a
// Work reader or joins a Task's work. Every match below is exhaustive: a new
// command chooses here.

impl Commands {
    /// Whether running this command changes state, and so writes a journal trace.
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Task { cmd } => cmd.changes_work(),
            Self::Wave { cmd } => cmd.changes_work(),
            Self::Session { cmd } => cmd.changes_work(),
            Self::Graph { cmd } => cmd.changes_work(),
            Self::Skill { cmd } => cmd.changes_work(),
            Self::Desktop { cmd } => cmd.changes_work(),
            Self::Plan { cmd } => cmd.changes_work(),
            Self::Pr { cmd } => pr_changes_work(cmd.as_ref()),
            Self::Wt { cmd } => cmd.changes_work(),
            Self::Machine { cmd } => cmd.changes_work(),
            Self::Server { .. } | Self::Holder { .. } => false,
            Self::Commit { .. } | Self::Sync(_) | Self::Steer { .. } => true,
            Self::Repo { cmd, .. } => cmd.as_ref().is_some_and(|cmd| cmd.changes_work()),
            Self::Config { cmd } => cmd.changes_work(),
            Self::Self_ { cmd } => cmd.changes_work(),
            Self::List { .. } | Self::Help { .. } | Self::Docs { .. } | Self::Context { .. } => {
                false
            }
            Self::Open(_)
            | Self::Inline { .. }
            | Self::Scratch { .. }
            | Self::Discord { .. }
            | Self::ContextBlock { .. }
            | Self::External(_) => true,
        }
    }
}

impl TaskCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Detail { .. }
            | Self::List { .. }
            | Self::Show { .. }
            | Self::Location { .. }
            | Self::Status { .. }
            | Self::Diff { .. }
            | Self::Changes { .. }
            | Self::Context { .. }
            | Self::Runs { .. }
            | Self::Find { .. }
            | Self::Grep { .. }
            | Self::Symbols { .. }
            | Self::Files { .. }
            | Self::File { .. }
            | Self::Wait { .. } => false,
            Self::Retire { .. } => true,
            Self::Report { .. } => true,
            Self::Sync { .. }
            | Self::Checkout { .. }
            | Self::Run { .. }
            | Self::Complete { .. }
            | Self::Reopen { .. }
            | Self::Create { .. }
            | Self::Claim { .. }
            | Self::Unassign { .. }
            | Self::Save { .. }
            | Self::Abandon { .. }
            | Self::Delete { .. }
            | Self::Update { .. }
            | Self::Move { .. }
            | Self::Auto { .. }
            | Self::Interrupt { .. } => true,
        }
    }
}

impl WaveCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Show { .. } | Self::Graph { .. } | Self::List { .. } | Self::Status { .. } => {
                false
            }
            Self::Plan { cmd, .. } => cmd.is_some(),
            Self::Remove { preview, .. } => !preview,
            Self::Create { .. } | Self::Sync { .. } | Self::Rename { .. } => true,
        }
    }
}

impl PlanCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Board { .. } | Self::List { .. } => false,
        }
    }
}

impl SessionCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::ObserveStatus { .. }
            | Self::Card { .. }
            | Self::List { .. }
            | Self::Usage(_)
            | Self::Retention { .. } => false,
            Self::History { cmd, .. } => cmd.as_ref().is_some_and(|cmd| cmd.changes_work()),
            Self::Steers { .. } => false,
            Self::Bind { dry_run, .. } => !dry_run,
            Self::Resume { .. }
            | Self::New { .. }
            | Self::Rename { .. }
            | Self::Move { .. }
            | Self::Send { .. }
            | Self::Receive { .. } => true,
        }
    }
}

impl GraphCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            // Customizing copies a definition into the repository's files,
            // as an editor would; `lf` admits no Process for it.
            Self::List { .. } | Self::Show { .. } | Self::Customize { .. } => false,
            Self::Run { .. } | Self::External(_) => true,
        }
    }
}

impl SkillCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Customize { .. } => false,
            Self::External(_) => true,
        }
    }
}

impl DesktopCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Shell { .. } => true,
            Self::View { .. }
            | Self::Ls { .. }
            | Self::State { .. }
            | Self::Actions { .. }
            | Self::Events { .. }
            | Self::Wait { .. }
            | Self::Capture { .. }
            // Navigation, panes and Dashboard arrangement are not Work; their
            // saves still persist, without a Process.
            | Self::Layout { .. }
            | Self::Back { .. }
            | Self::Forward { .. }
            | Self::Goto { .. }
            | Self::Next { .. }
            | Self::Previous { .. }
            | Self::WorkingTasks { .. }
            | Self::Enter { .. }
            | Self::Zoom { .. }
            | Self::Focus { .. }
            | Self::Hide { .. }
            | Self::Show { .. }
            | Self::Move { .. }
            | Self::Resize { .. }
            | Self::Files { .. }
            | Self::GraphLog { .. }
            | Self::Browse { .. }
            | Self::PageBack { .. }
            | Self::PageForward { .. }
            | Self::Reload { .. }
            | Self::Switch { .. } => false,
            // Renaming a Task changes Work; renaming a Dashboard is arrangement.
            Self::Dashboard {
                cmd: DashboardCommand::Rename { target, .. },
            } => target.starts_with("task:"),
            Self::Dashboard { .. } => false,
            Self::Portfolio { cmd, .. } => cmd.is_some(),
            // Input reaches a Session or shell, closing ends one, and an
            // action named at runtime may do either.
            Self::Do { .. }
            | Self::Send { .. }
            | Self::SendKey { .. }
            | Self::Run { .. }
            | Self::Close { .. }
            | Self::Reopen { .. } => true,
        }
    }
}

impl RepoCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Tracker { cmd, .. } => match cmd {
                Some(RepoTrackerCommand::Answer { .. }) => true,
                None => false,
            },
            Self::Release { cmd } => cmd.changes_work(),
            Self::Path | Self::Ci { .. } | Self::People { .. } | Self::Tokens { .. } => false,
        }
    }
}

/// Bare `lf pr` acts on the current branch's PR.
fn pr_changes_work(cmd: Option<&PrCommand>) -> bool {
    match cmd {
        None => true,
        Some(PrCommand::Checks { .. } | PrCommand::Reviews { .. }) => false,
        Some(
            PrCommand::Reconcile { .. }
            | PrCommand::RequestReview { .. }
            | PrCommand::Publish { .. }
            | PrCommand::Open { .. }
            | PrCommand::Submit { .. }
            | PrCommand::Arm { .. }
            | PrCommand::Land { .. }
            | PrCommand::Abandon { .. },
        ) => true,
    }
}

impl ReleaseCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Check { .. } | Self::Status { .. } | Self::Issues { .. } => false,
            Self::Promote { dry_run, .. } => !dry_run,
            Self::Run { .. }
            | Self::Notes { .. }
            | Self::Bump { .. }
            | Self::Tag { .. }
            | Self::Publish { .. }
            | Self::Regress { .. } => true,
        }
    }
}

impl WtCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Switch { .. } | Self::List { .. } | Self::Timing { .. } => false,
            Self::Prune { dry_run, .. } => !dry_run,
            Self::Create { .. } | Self::Delete { .. } => true,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum HolderCommand {
    /// Hold PTYs and their agents until the last one ends.
    Run,
}

#[derive(Debug, Subcommand)]
pub enum ServerCommand {
    /// Look up a submitted creation without retrying it
    Receipt {
        request_id: String,
        /// The Task the creation was for, read where that Task runs
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Serve the private socket in the foreground (for the OS supervisor).
    Run,
    /// Pipe stdin and stdout to this Machine's server, starting it first if
    /// needed; another Machine runs it over SSH to reach the server.
    Connect,
    /// Report local readiness without registering or starting anything.
    Status,
    /// Configure private mobile access using this host's signed-in Tailscale installation.
    Mobile {
        /// Stop remote access without stopping local Sessions.
        #[arg(long, conflicts_with = "status")]
        off: bool,
        /// Report saved approval and live mobile readiness without enabling access.
        #[arg(long)]
        status: bool,
    },
    /// Summarize the protocol this build speaks; needs no server.
    Describe {
        /// Print its JSON Schema, generated from the wire types
        #[arg(long)]
        json: bool,
    },
}

impl MachineCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Self_ { .. }
            | Self::Doctor { .. }
            | Self::List { .. }
            | Self::Status { .. }
            | Self::Logins { .. } => false,
            Self::Credentials { .. } => true,
            Self::Pool { cmd } => match cmd {
                PoolCommand::List { .. } => false,
                PoolCommand::Add { .. }
                | PoolCommand::Off { .. }
                | PoolCommand::On { .. }
                | PoolCommand::Remove { .. } => true,
            },
            Self::Ondemand { cmd } => match cmd {
                OndemandCommand::List { .. } => false,
                OndemandCommand::Add { .. }
                | OndemandCommand::Max { .. }
                | OndemandCommand::Rent { .. }
                | OndemandCommand::Remove { .. } => true,
            },
            Self::Hub { machine, clear } => machine.is_some() || *clear,
            Self::Prune { .. }
            | Self::Forward { .. }
            | Self::Login { .. }
            | Self::Logout { .. }
            | Self::Public { .. }
            | Self::Add { .. }
            | Self::Rename { .. }
            | Self::Remove { .. }
            | Self::End { .. } => true,
        }
    }
}

impl ConfigCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::User { .. }
            | Self::Settings { .. }
            | Self::Catalog { .. }
            | Self::Scope { .. }
            | Self::Get { .. }
            | Self::Explain { .. } => false,
            Self::Set { .. } | Self::Unset { .. } => true,
            Self::Import { .. } => true,
            Self::Migrate { accept } => *accept,
            Self::Retained { replace, keep } => *replace || *keep,
            Self::Repo { bind, relocate, .. } => bind.is_some() || relocate.is_some(),
        }
    }
}

impl SelfCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Install { cmd, .. } => match cmd {
                Some(InstallCommand::Server { profile_stdin }) => *profile_stdin,
                Some(InstallCommand::Preflight { .. }) => false,
                None
                | Some(
                    InstallCommand::Schedule { .. }
                    | InstallCommand::Adopt
                    | InstallCommand::RecoverSwitch { .. }
                    | InstallCommand::AdvanceSwitch { .. }
                    | InstallCommand::Promote { .. }
                    | InstallCommand::Rollback { .. },
                ) => true,
            },
            Self::SyncSkills { .. } => true,
        }
    }
}

/// Which part of the view on screen a command acts on. `-p` is a path: a view part
/// (`backlog`, `session`, `tiles` on a Wave view), a pane (`session`, `shell`,
/// `browser`, `shell:2`), a Dashboard or Wave view tile (`3`, `LOO-519`), or `TILE/PANE`. `query:ID/TILE` and `working-tasks/TILE` name a
/// saved record's tile by Work key without Desktop. Root `--task`/`--wave` set the
/// base view; without either, a command acts on the terminal it runs in (`LF_TERMINAL_ID`).
#[derive(Args, Debug, Clone, Default)]
pub struct DesktopTargetArgs {
    /// Pane path: a view part (backlog), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix
    #[arg(short = 'p', long = "pane", value_name = "PATH")]
    pub pane: Option<String>,
    /// Desktop window: ID, position or ID suffix
    #[arg(long, value_name = "WINDOW")]
    pub window: Option<String>,
    #[arg(long)]
    pub json: bool,
}

/// Window-level verbs name no pane.
#[derive(Args, Debug, Clone, Default)]
pub struct DesktopWindowArgs {
    /// Desktop window: ID, position or ID suffix
    #[arg(long, value_name = "WINDOW")]
    pub window: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum SkillCommand {
    /// Print the repository file that defines a skill. When the repository
    /// has none, write one that extends the builtin with its own section
    Customize {
        name: String,
        /// Write the builtin's whole text instead, replacing it here
        #[arg(long)]
        full: bool,
    },
    #[command(external_subcommand)]
    External(Vec<String>),
}

const SETTING_KEYS: &str = "`lf config catalog` lists every preference and where it applies.
Values use YAML; false disables and [] clears an additive list.
The default destination is the main checkout's .lf/config.local.yaml, shared across Task worktrees.
--global edits ~/.lf/config.yaml; --shared explicitly edits committed configuration.
`lf --wave <name> config set` selects a Wave; unset removes only that declaration.
`lf config settings --skill <name> --json` explains effective values and sources.";

#[derive(Subcommand, Debug)]
pub enum WaveGraphCommand {
    /// Show the graph the Wave's Tasks take up by default; select one with
    /// `graph:` in `.lf/wave/<name>/config.yaml`
    Show {
        #[arg(conflicts_with = "no_wave")]
        wave: Option<String>,
        /// Show the default for Tasks filed in the repository, without a Wave
        #[arg(long)]
        no_wave: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum GraphCommand {
    /// List authored graphs, or Graphs that ran
    List {
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        inventory: commands::graph_inventory::GraphProcessInventoryArgs,
    },
    /// Inspect an authored graph, or a Graph run by its id
    Show {
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// Print the repository file that defines a Graph, creating
    /// it from the builtin when the repository has none
    Customize {
        name: String,
        /// Print `{"path": …}`
        #[arg(long)]
        json: bool,
    },
    /// Carry a Task's Graph from where its `lf task run` set out; that run starts it
    #[command(hide = true)]
    Run,
    #[command(external_subcommand)]
    External(Vec<String>),
}

/// Shared selection and preparation options for `resume` and `session new`.
#[derive(Args, Debug)]
pub struct SessionConnectionArgs {
    /// Attach without taking the terminal size until focus or input
    #[arg(long)]
    pub unfocused: bool,
    #[arg(long, conflicts_with = "wave")]
    pub task: Option<String>,
    #[arg(long)]
    pub wave: Option<String>,
    /// Return connection data without starting the provider
    #[arg(long)]
    pub prepare: bool,
    #[arg(long)]
    pub json: bool,
}

impl SessionConnectionArgs {
    pub fn target<'a>(
        &'a self,
        cli: &'a Cli,
        id: Option<&'a str>,
    ) -> anyhow::Result<loopflow_execution::ops::human_session::SessionTarget<'a>> {
        anyhow::ensure!(
            self.task.is_none() || cli.task.is_none(),
            "select a Task only once"
        );
        anyhow::ensure!(
            self.wave.is_none() || cli.wave.is_none(),
            "select a Wave only once"
        );
        use loopflow_execution::ops::human_session::SessionTarget;
        let target = SessionTarget::from_selectors(
            id,
            self.task.as_deref().or(cli.task.as_deref()),
            self.wave.as_deref().or(cli.wave.as_deref()),
        )?;
        Ok(
            if matches!(target, SessionTarget::Context) && cli.selects_repository() {
                SessionTarget::Repository
            } else {
                target
            },
        )
    }
}

/// Which unfinished Sessions `lf session list` reads.
#[derive(Args, Debug, Clone)]
pub struct SessionListArgs {
    #[command(flatten)]
    pub selector: SessionSelectorArgs,
    #[arg(long)]
    pub json: bool,
    /// Include Sessions from every repository on this machine
    #[arg(long)]
    pub all: bool,
    /// Include headless Sessions alongside interactive conversations
    #[arg(long)]
    pub headless: bool,
    /// Only conversations available for your input
    #[arg(long)]
    pub available: bool,
    /// Leave out raw conversations: a plain `claude` or `codex` lf never launched
    #[arg(long)]
    pub no_raw: bool,
    /// Maximum conversations; 0 reads the complete matching inventory
    #[arg(long, default_value_t = 100)]
    pub limit: usize,
    /// Return a bounded stable-ID page with a continuation cursor
    #[arg(long, requires = "json")]
    pub page: bool,
    /// Previous page's next identity; keep the same filters
    #[arg(long, requires = "page")]
    pub after: Option<String>,
    #[arg(long)]
    pub task: Option<String>,
    /// Conversations attributed to this Wave or to one of its Tasks
    #[arg(long, id = "session_wave", value_name = "W")]
    pub wave: Option<String>,
    /// Current primary conversations; omit the value to include all three scopes
    #[arg(long, value_enum, num_args = 0..=1, default_missing_value = "all", value_name = "task|wave|repository")]
    pub primary: Option<loopflow_execution::session::SessionPrimary>,
    /// Outside the work model: no Task association and no Wave
    #[arg(long, conflicts_with = "task")]
    pub orphan: bool,
    #[arg(long)]
    pub search: Option<String>,
    /// Print a JSON line now and each time the list changes, until stdin closes
    #[arg(short = 's', long)]
    pub subscribe: bool,
    #[command(flatten)]
    pub selection: crate::lf::Selection,
}

/// Which Sessions a list shows or an action moves; `lf session list` with the
/// same flags is the preview.
#[derive(Args, Debug, Clone, Default)]
pub struct SessionSelectorArgs {
    /// Sessions on this harness, optionally narrowed to a model
    #[arg(long, value_name = "AGENT")]
    pub on: Option<String>,
}

#[derive(Subcommand, Debug)]
// History carries its feed filters and its own commands; Clap owns the layout.
#[allow(clippy::large_enum_variant)]
pub enum SessionCommand {
    /// Move the selected open Sessions to another agent (-a harness[:model]);
    /// `lf session list` with the same flags previews them
    #[command(
        after_help = "Idle Sessions move now; a busy one moves after its turn.\n  lf session list --on claude\n  lf session move --on claude -a codex"
    )]
    Move {
        #[command(flatten)]
        selector: SessionSelectorArgs,
        /// Sessions from every repository on this machine
        #[arg(long)]
        all: bool,
        #[arg(long)]
        json: bool,
    },
    /// See which steers a Session has read and which wait
    Steers {
        id: String,
        #[arg(long)]
        json: bool,
        #[arg(long, default_value_t = 0)]
        after: i64,
        #[arg(long, default_value_t = 32)]
        limit: usize,
        /// Read the complete steer
        #[arg(long)]
        source: Option<String>,
    },
    /// Record one validated Program Status snapshot, read from stdin, from an active local terminal
    ObserveStatus {
        id: String,
        /// The terminal marker in the current provider client receipt
        #[arg(long)]
        terminal: String,
        /// AgentProcess from the Session reading; omit when it reported none
        #[arg(long)]
        agent_process: Option<String>,
        /// The observer this snapshot belongs to; a new one replaces the last
        #[arg(long)]
        stream: String,
        /// Position in the stream, from 1; an older snapshot never replaces a newer one
        #[arg(long)]
        sequence: i64,
        #[arg(long)]
        json: bool,
    },
    /// Read what Sessions did, tagged with their Work; with an ID, one
    /// conversation's native start, usage and completion receipts
    #[command(args_conflicts_with_subcommands = true)]
    History {
        /// Session ID or provider conversation ID
        id: Option<String>,
        #[command(flatten)]
        feed: commands::history::HistoryFeed,
        /// With an ID, continue after an observed event sequence
        #[arg(long, default_value_t = 0)]
        after: i64,
        #[command(subcommand)]
        cmd: Option<commands::history::HistoryCommand>,
    },
    /// Read a recorded Session's card without attaching or taking input control
    Card {
        id: String,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        selection: crate::lf::Selection,
    },
    /// List unfinished Sessions; use `lf history` for finished conversations
    List(SessionListArgs),
    /// Show direct provider-authored usage from recorded Session inputs
    Usage(commands::history::UsageArgs),
    /// Show how much Session history providers still keep, and what they remove next
    Retention {
        #[arg(long)]
        json: bool,
    },
    /// Get back to a conversation: this scope's, a Task's, or an exact one
    #[command(
        after_help = "Opens the conversation in this terminal. With none for this scope, it starts one and says so. A conversation running in an lf terminal gains one more view here, and every view types. An ended or crashed one continues its provider thread from history. One running in a client lf can't share is taken over: that client ends, unsent text there is lost, and resume says so. If attachment fails, resume reports the failure, leaves that work intact and resends no input.\n\nOther harness:\n  lf resume -a codex                       continue on Codex, seeded with what the last thread knew\nMoves:\n  lf session move --on claude -a codex      continue Claude Sessions on Codex\n  lf --machine <machine> resume <session>  move only this conversation to another Machine\n  lf task move <task> --machine <machine>  move a Task with its Workflow and Session"
    )]
    Resume {
        /// Session ID, Task identifier, or provider conversation ID; beside
        /// `--task`/`--wave` the ID selects and the Work makes it managed
        id: Option<String>,
        #[command(flatten)]
        connection: SessionConnectionArgs,
        /// Leave the other client running; continue in a new conversation forked from it
        #[arg(long)]
        fork: bool,
        /// The request this start answers: a new Task's setup steps are timed
        /// under it, and the server's recovery pass names its own starts
        #[arg(long)]
        request_id: Option<String>,
    },
    /// Restart this scope's conversation on a fresh provider thread, seeded with
    /// what the last thread knew; `-a` switches harness
    #[command(
        after_help = "The conversation keeps its name, Work and history; the old thread stays in `lf session history`.\n  lf session new -a claude      continue this scope's conversation on Claude"
    )]
    New {
        /// Retry a submitted new thread with the same identity and content
        #[arg(long)]
        request_id: Option<String>,
        /// Session ID, Task identifier, or provider conversation ID
        #[arg(conflicts_with_all = ["task", "wave"])]
        id: Option<String>,
        #[command(flatten)]
        connection: SessionConnectionArgs,
    },
    /// Rename a Session; a human name is never replaced by a suggestion
    Rename {
        /// Session ID or provider conversation ID
        id: String,
        #[arg(value_name = "NAME", required = true, num_args = 1..)]
        name: Vec<String>,
        /// Propose an agent-generated name; keeps a human-assigned name
        #[arg(long)]
        suggest: bool,
        #[arg(long)]
        json: bool,
    },
    /// Bind a Session as the Task's primary conversation without starting work
    #[command(
        after_help = "Select an unfinished interactive Session by its Loopflow ID or recorded provider ID. Binding is permanent; repeating it selects the same conversation again. The Task keeps its existing worktree and binding immediately sets the Session directory to it. Already-running clients may keep their original directory. Files and live input stay intact. An open Task without a worktree adopts the Session's unowned linked checkout; main and Wave checkouts stay shared. Done Tasks remain done; without a checkout, their conversation stays in place. Closed Sessions and headless Sessions cannot become primary. Remote calls use the target Machine's Sessions and paths."
    )]
    Bind {
        /// Session ID or provider conversation ID
        id: String,
        /// The Task, by its issue identifier (e.g. INF-123) or stable Task ID
        #[arg(long)]
        task: String,
        /// Resolve the exact target without assigning the Session
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Hand a Session to another Machine: commit and push its branch, stop it
    /// here and print the move for `lf session receive` there
    #[command(
        after_help = "Everyday spelling: `lf --machine <machine> resume <session>` sends, receives and resumes in one step. `lf task move <task> --machine <machine>` moves a Task with its Session; run it again to deliver a Task that was sent but did not arrive. Run send and receive by hand to retry a Session whose receive failed:\n  lf session send <session> --to <machine> | lf --machine <machine> session receive"
    )]
    Send {
        /// Session ID or provider conversation ID; with --task, the Task's
        /// conversation to carry
        #[arg(required_unless_present = "task")]
        id: Option<String>,
        /// Send this Task with its Session, as `lf task move --machine` does
        #[arg(long)]
        task: Option<String>,
        /// The Machine it moves to, by label or id
        #[arg(long)]
        to: String,
        /// Send nothing: record that the Task this Machine sent arrived there
        #[arg(long, hide = true, requires = "task")]
        arrived: bool,
    },
    /// Admit a Session `lf session send` printed, read from stdin: check out its
    /// branch and place its conversation so it resumes here
    Receive {
        /// Read no move; exit non-zero unless this Machine can receive one
        #[arg(long)]
        check: bool,
        /// With --check, also take in this Task from planning when this
        /// Machine has not saved it yet
        #[arg(long, requires_all = ["check", "issue", "repository"])]
        task: Option<String>,
        /// The Task's identifier in planning, such as LOO-12
        #[arg(long, requires = "task")]
        issue: Option<String>,
        /// The Task's repository
        #[arg(long, requires = "task")]
        repository: Option<loopflow_model::durable::RepositoryId>,
        /// Where a Machine without the repository clones it from
        #[arg(long, requires = "task")]
        origin: Option<String>,
        /// The Machine the Task moves from, which a retry here asks for it
        #[arg(long, requires = "task")]
        from: Option<loopflow_model::durable::MachineId>,
        /// Read no move; exit non-zero unless this Task's last move arrived
        /// here. A Machine that sent it asks before taking it back.
        #[arg(long, hide = true, value_name = "TASK", conflicts_with = "check")]
        arrived: Option<loopflow_model::durable::TaskId>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum WaveCommand {
    /// Read a Wave's Project and Tasks without starting work; an explicit Wave includes history
    Show {
        wave: Option<String>,
        /// Find an exact Task, including historical planning
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        json: bool,
        /// Read every repository on this Machine
        #[arg(long)]
        all: bool,
        /// Print a JSON line now and each time the plan changes, until stdin closes
        #[arg(short = 's', long, requires = "json")]
        subscribe: bool,
        #[command(flatten)]
        selection: crate::lf::Selection,
    },
    /// Show the graph the Wave's Tasks take up by default
    Graph {
        #[command(subcommand)]
        cmd: WaveGraphCommand,
    },
    /// Create a Wave: register it, then seed and commit .lf/wave/<name>/ on the main
    /// checkout. Creating an existing Wave changes nothing.
    Create {
        wave: String,
        /// Objective text for the new Wave's GOAL.md
        #[arg(long)]
        objective: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Remove a Wave and its child Waves. Open Tasks move to the parent Wave, or the
    /// repository; completed Tasks and Sessions stay readable in history.
    #[command(after_help = "Run again to finish an interrupted removal.")]
    Remove {
        wave: String,
        /// Show what removal does without changing anything
        #[arg(long)]
        preview: bool,
        /// Delete unstarted Tasks instead of moving them; started Tasks always move
        #[arg(long)]
        delete_backlog: bool,
        #[arg(long)]
        json: bool,
    },
    /// Print one Wave's Project and Tasks as Desktop shows them; `connect`
    /// and `reteam` change where its planning lives
    #[command(args_conflicts_with_subcommands = true)]
    Plan {
        wave: Option<String>,
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        cmd: Option<WavePlanCommand>,
    },
    /// Send one Wave's queued tracker changes and read its tracker Project,
    /// then show its status; with no tracker, exchange the repository's plan
    Sync {
        /// Wave name (default: the ambient wave)
        wave: Option<String>,
        /// Emit the status snapshot as JSON
        #[arg(long)]
        json: bool,
    },
    /// List this repository's Waves; removed Waves stay readable by `lf wave show`.
    List {
        /// Emit the wave snapshot as JSON (Loopflow's dashboard snapshot)
        #[arg(long)]
        json: bool,
        /// List Waves from every repository on this machine, not just the
        /// current repository (worktrees collapse to their main checkout).
        #[arg(long)]
        all: bool,
    },
    /// Show one Wave's Project, Tasks and execution evidence.
    Status {
        /// Wave name (default: the ambient wave)
        wave: Option<String>,
        /// Emit the status snapshot as JSON
        #[arg(long)]
        json: bool,
        /// Print a JSON line now and each time the status changes, until stdin closes
        #[arg(short = 's', long, requires = "json")]
        subscribe: bool,
        #[command(flatten)]
        selection: crate::lf::Selection,
    },
    /// Rename a Wave, moving .lf/wave/<name>/ on the main checkout; its Tasks,
    /// Project and Sessions keep their identity
    Rename {
        wave: String,
        /// The new name; omit to keep it while changing --repo
        new_name: Option<String>,
        /// Move the Wave to another repository
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum WavePlanCommand {
    /// Link a Wave's Project to a Linear Project in the repository's Team
    /// (Task prefix), or make one; a new Wave name is added first
    Connect {
        /// Wave name (auto-detected if omitted)
        wave: Option<String>,
        /// An existing Linear Project to link: its URL, id or name.
        /// Without one, the Wave's Project is made in Linear.
        #[arg(conflicts_with = "all")]
        project: Option<String>,
        /// Connect every Wave under .lf/wave/
        #[arg(long, conflicts_with = "wave")]
        all: bool,
        /// Repository Team key = Task prefix (e.g. LOO). Defaults from the repository name.
        #[arg(long = "team-key")]
        team_key: Option<String>,
        /// Repository Team display name. Defaults to the repository name.
        #[arg(long = "team-name")]
        team_name: Option<String>,
        /// Print one receipt per connected Wave
        #[arg(long)]
        json: bool,
    },
    /// Reconcile linked Waves to the repository's Linear Team
    Reteam {
        #[arg(long)]
        apply: bool,
        /// Print the plan, or what moved
        #[arg(long)]
        json: bool,
    },
}

#[derive(Args, Debug)]
pub struct SyncArgs {
    /// Print the planned sync strategy without mutating git
    #[arg(long, conflicts_with_all = ["manual", "continue_sync", "abort"])]
    pub plan: bool,
    /// Keep the sync and uncommitted edits local; leave conflicts for this process to resolve
    #[arg(long, conflicts_with_all = ["plan", "continue_sync", "abort"])]
    pub manual: bool,
    /// Stage resolved conflict paths and continue the local sync
    #[arg(long = "continue", conflicts_with_all = ["plan", "manual", "abort"])]
    pub continue_sync: bool,
    /// Abort the local sync in progress
    #[arg(long, conflicts_with_all = ["plan", "manual", "continue_sync"])]
    pub abort: bool,
    /// Explicitly claim a raw sync that has no Loopflow owner
    #[arg(long, conflicts_with_all = ["plan", "manual"])]
    pub adopt: bool,
    /// Branch or Task to sync onto; another open Task stacks this one on it, the default branch detaches
    pub onto: Option<String>,
    /// Print the plan, or what the sync did
    #[arg(long)]
    pub json: bool,
}

/// `lf task auto on|off`.
#[derive(Subcommand, Debug)]
pub enum AutoCommand {
    /// Run the Task with nobody waited for: stand in at each interactive step, and merge
    #[command(
        after_help = "Prints what `lf task run` prints and returns once the run registers (`-b` \
waits for it). A Task waiting at an interactive step is stood in for there; a Task already running \
carries on and stands in from its next one. With no ISSUE, the checkout's Task, or a new one for \
its branch."
    )]
    On {
        issue: Option<String>,
        /// Take up this graph in place of the Task's remaining steps
        #[arg(long)]
        graph: Option<String>,
        /// Direction for the run, published to the Task
        #[arg(long)]
        reason: Option<String>,
        /// Print the run it started, as `task.auto.on` answers
        #[arg(long)]
        json: bool,
    },
    /// Take the interactive steps and the merge back: the run waits at its next one
    #[command(
        after_help = "Nothing is killed: the run waits at its next interactive step, or before a \
step that merges, and auto-merge Loopflow armed is disarmed. With no ISSUE, the checkout's Task."
    )]
    Off {
        issue: Option<String>,
        /// Why, saved as a Task comment
        #[arg(long)]
        reason: Option<String>,
        /// Print the Task and whether auto was on
        #[arg(long)]
        json: bool,
    },
}

impl AutoCommand {
    fn issue(&self) -> &Option<String> {
        match self {
            Self::On { issue, .. } | Self::Off { issue, .. } => issue,
        }
    }

    fn issue_mut(&mut self) -> &mut Option<String> {
        match self {
            Self::On { issue, .. } | Self::Off { issue, .. } => issue,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum TaskCommand {
    /// Read one Task, its PR, checks, Graphs, comments and current Session
    Detail {
        issue: String,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        selection: crate::lf::Selection,
    },
    /// List the open Tasks in the selected repository
    List {
        /// Only Tasks whose title or identifier contains this text
        #[arg(conflicts_with = "closed")]
        query: Option<String>,
        #[arg(long)]
        wave: Option<String>,
        /// Project ID, provider ID, slug or name
        #[arg(long, conflicts_with = "closed")]
        project: Option<String>,
        /// Only Tasks in this state
        #[arg(long, conflicts_with = "closed", value_parser = ["unstarted", "idle", "active", "moving"])]
        state: Option<String>,
        #[arg(long)]
        json: bool,
        /// Page closed (done or abandoned) Tasks instead, most recently closed first
        #[arg(long)]
        closed: bool,
        /// Only Tasks closed since then: 7d, 24h, 30m or RFC3339
        #[arg(long, requires = "closed")]
        since: Option<String>,
        /// Previous page's `next` Task; keep the same filters
        #[arg(long, requires = "closed")]
        after: Option<String>,
        /// Closed Tasks per page
        #[arg(long, requires = "closed", default_value_t = commands::waves::CLOSED_TASK_PAGE)]
        limit: usize,
        /// Print a JSON line now and each time the list changes, until stdin closes
        #[arg(short = 's', long, conflicts_with = "closed")]
        subscribe: bool,
        #[command(flatten)]
        selection: crate::lf::Selection,
    },
    /// Print one Task as Desktop's Task pane shows it, as JSON
    Show {
        issue: String,
        #[arg(long)]
        json: bool,
        /// Print a JSON line now and each time the Task changes, until stdin closes
        #[arg(short = 's', long)]
        subscribe: bool,
        #[command(flatten)]
        selection: crate::lf::Selection,
    },

    /// Exchange a Task with its tracker now: look for what was sent, resend what it lacks
    #[command(after_help = "A lost line is a value the tracker kept over yours. \
--keep-theirs closes every one the Task has; --use-mine saves yours again and sends them. \
`lf repo tracker answer <id>` answers one.")]
    #[command(group(clap::ArgGroup::new("answer").args(["keep_theirs", "use_mine"])))]
    Sync {
        issue: String,
        /// Keep the tracker's value for each lost line
        #[arg(long)]
        keep_theirs: bool,
        /// Save each lost value again, which sends it
        #[arg(long)]
        use_mine: bool,
    },
    /// Ensure tracked Task Work and its worktree without launching a Graph
    Checkout {
        issue: Option<String>,
        #[arg(long)]
        name: Option<String>,
        /// Fork from another Task's branch; push if absent or an unpublished parent is ahead
        #[arg(long = "stack-on", value_name = "PARENT_TASK")]
        stack_on: Option<String>,
        #[arg(long)]
        directive: Option<String>,
        /// Hand off a child-specific design from the caller's checkout
        #[arg(long, value_name = "PATH")]
        design: Option<PathBuf>,
        /// Restore a missing Task branch from a deliberately recovered commit; never reset existing work
        #[arg(long, value_name = "COMMIT", conflicts_with_all = ["stack_on", "name", "directive", "design"])]
        recover_from: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Run the Task's Graph in its worktree until the next interactive step;
    /// returns once it runs (`-b` runs every step headless to `end` and waits)
    #[command(
        after_help = "A Task runs the graph it took up: its Wave's default, its repository's \
`graph:`, or `code`. A run stops before each interactive step, where the Task waits for its \
conversation; the next `lf task run` continues after it. A run that fails starts again at the \
step that failed; the steps before it stay done, and --from reruns them. `-i` runs every step \
with you in this terminal instead. A run that reaches a merging step, as `-b` does, may merge the PR \
unless someone turned auto off; then it waits at the landing step until `lf task auto on`. `-b` turns \
no auto on: a run carried again after it dies stops at its next interactive step. Add --open to open its primary conversation locally when the launch returns, \
using lf open's ordinary host selection. Preview and --quote open nothing."
    )]
    Run {
        issue: Option<String>,
        /// A graph to take up in place of the Task's own
        graph: Option<String>,
        /// Start at this step instead: to go back, skip ahead, or wait at an interactive step
        #[arg(long, value_name = "STEP")]
        from: Option<String>,
        #[arg(long)]
        name: Option<String>,
        /// Fork from another Task's branch; push if absent or an unpublished parent is ahead
        #[arg(long = "stack-on", value_name = "PARENT_TASK")]
        stack_on: Option<String>,
        #[arg(long)]
        directive: Option<String>,
        /// Direction for this run, published to the Task
        #[arg(long)]
        reason: Option<String>,
        /// Run on a free machine you own in this pool
        #[arg(long)]
        pool: Option<String>,
        /// Rent a machine from this on-demand (after `--pool`, only when none is free)
        #[arg(long)]
        rent: Option<String>,
        /// Print where it would run and the most it could cost; run nothing
        #[arg(long)]
        quote: bool,
        /// Open the Task's primary conversation locally after launch returns
        #[arg(long)]
        open: bool,
        /// Print what the run set out to do, as `task.run` returns it
        #[arg(long)]
        json: bool,
    },
    /// Complete a Task without moving its Graph; open children need --children
    #[command(
        after_help = "A Task with open children refuses to complete, writing nothing, and lists every open descendant. Pass --children rehome to move its direct open children up to the nearest open ancestor (or to the top level) where they stay open, or --children abandon to mark merged descendants done and abandon the rest, closing their PRs and deleting their branches. --children abandon refuses while a descendant it would abandon has a running Graph, headless run or active turn; a conversation idle at an interactive step is closed. `lf task reopen` reopens a done or abandoned Task."
    )]
    Complete {
        issue: String,
        /// What happens to open children: rehome keeps them open under the nearest open ancestor; abandon settles them
        #[arg(long, value_enum)]
        children: Option<loopflow_storage::sqlite::children::ChildPolicy>,
        /// Why, kept in the Task's completion
        #[arg(long)]
        reason: Option<String>,
        /// Print the completed Task
        #[arg(long)]
        json: bool,
    },
    /// Reopen a done or abandoned Task at its current Graph position
    Reopen {
        issue: String,
        #[arg(long)]
        reason: Option<String>,
        /// Print the reopened Task
        #[arg(long)]
        json: bool,
    },
    /// Create a Task; with --run, also start its Graph
    #[command(
        after_help = "Plain creation files new top-level work and never adopts the current checkout or invoking Session. Pass --parent ID to file a Task under another Task (inheriting its Wave unless --wave is set). --run starts the created Task; it does not select work to adopt. Pass --session SESSION or --worktree PATH to adopt existing work, with or without --run. Find Loopflow Session IDs with `lf session list --json`; an agent keeping its chat passes its own ID explicitly. Explicit-source retries return the same Task. Main and Wave checkouts stay shared. Adoption preserves files, branch, history and live attachment; creation alone starts no Graph and turns no auto on. Remote calls use sources on the selected Machine."
    )]
    Create {
        /// Retry a submitted creation with the same identity and content
        #[arg(long)]
        request_id: Option<String>,
        /// Existing conversation to promote without replaying its input
        #[arg(long, id = "create_session")]
        session: Option<String>,
        /// Existing linked checkout to adopt
        #[arg(long)]
        worktree: Option<PathBuf>,
        /// What you want, in your words: the Task's notes, its first line the title.
        /// With --run, also the first turn of its conversation
        #[arg(conflicts_with = "notes")]
        request: Option<String>,
        /// Create without a Wave (the default)
        #[arg(long, conflicts_with = "wave")]
        no_wave: bool,
        /// Explicit Wave membership; omitted creates a standalone Task
        #[arg(long)]
        wave: Option<String>,
        /// File this Task as a child of another Task (ID or identifier); omitted files top-level work
        #[arg(long, value_name = "ID")]
        parent: Option<String>,
        /// Task title; omitted when stdin supplies the report and first line
        #[arg(long)]
        title: Option<String>,
        /// Description; defaults to a report read from stdin
        #[arg(long)]
        notes: Option<String>,
        /// Start it as `lf task run` does, then open its conversation here
        #[arg(long)]
        run: bool,
        /// The graph it takes up in place of the default
        #[arg(long, requires = "run")]
        graph: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Claim the Task for you on this Machine
    Claim {
        issue: String,
        #[arg(long)]
        json: bool,
    },
    /// Unassign the Task: nobody owns it and its tracker shows no assignee; nothing
    /// moves it until someone claims or runs it. History stays intact
    Unassign {
        issue: String,
        #[arg(long)]
        json: bool,
    },
    /// Read recorded execution location without preparing or transferring Work
    Location {
        issue: String,
        /// Observe added Machines as well as this one
        #[arg(long)]
        peers: bool,
        #[arg(long, hide = true)]
        request: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Run one cleanup pass for a Task, or all pending Tasks on this Machine
    #[command(hide = true)]
    Retire { issue: Option<String> },
    /// Show durable Task facts and its recorded work
    Status {
        /// Task issue; defaults to the Task in this checkout
        issue: Option<String>,
        #[arg(long)]
        json: bool,
        /// Print a JSON line now and each time the status changes, until stdin closes
        #[arg(short = 's', long)]
        subscribe: bool,
        #[command(flatten)]
        selection: crate::lf::Selection,
    },
    /// Show this Task's patch
    Diff {
        issue: String,
        path: Option<String>,
        #[arg(long, default_value = "parent")]
        base: String,
        /// Compare to this resolved commit instead of the working tree
        #[arg(long)]
        head: Option<String>,
        /// Compare a UTF-8 draft read from stdin without writing the worktree
        #[arg(long, requires = "path")]
        draft: bool,
        #[arg(long)]
        json: bool,
        /// Open the comparison in Git's configured difftool (tracked files only)
        #[arg(long, conflicts_with_all = ["draft", "json"])]
        difftool: bool,
    },
    /// List this Task's changed paths and comparison revisions
    Changes {
        issue: String,
        #[arg(long, default_value = "parent")]
        base: String,
        /// Compare to this resolved commit instead of the working tree
        #[arg(long)]
        head: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// What each of this Task's Session inputs was made of
    Context {
        issue: String,
        #[arg(long)]
        json: bool,
    },
    /// Page the Graphs that ran for this Task
    Runs {
        issue: String,
        /// Previous page's next identity
        #[arg(long)]
        after: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Find declarations of a name in this Task's checkout
    Symbols {
        issue: String,
        name: String,
        /// Limit the search to this checkout-relative file
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Search for literal text in this Task's checkout
    Grep {
        issue: String,
        query: String,
        /// Match only at word boundaries
        #[arg(long)]
        whole_word: bool,
        #[arg(long)]
        json: bool,
    },
    /// Find files anywhere in this Task's checkout, including scratch
    Find {
        issue: String,
        query: Option<String>,
        /// Maximum paths to return
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    /// List one directory in this Task's worktree
    Files {
        issue: String,
        #[arg(default_value = ".")]
        directory: String,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        show_ignored: bool,
        #[arg(long)]
        json: bool,
    },
    /// Read one file from the Task checkout
    File {
        issue: String,
        path: String,
        /// Read the file at this commit (parent, head or a resolved SHA) instead of the working tree
        #[arg(long, conflicts_with = "recoveries")]
        at: Option<String>,
        /// Inspect retained versions, including late writes; omitted for fast content reads
        #[arg(long)]
        recoveries: bool,
        #[arg(long)]
        json: bool,
    },
    /// Save UTF-8 stdin with an expected revision and retained recovery files
    Save {
        issue: String,
        path: String,
        #[arg(long)]
        revision: String,
        #[arg(long)]
        json: bool,
    },
    /// Cancel the Task in its tracker and locally, close its PRs and delete its branches
    Abandon {
        /// Issue ID or branch; defaults to the Task in this checkout
        issue: Option<String>,
        #[arg(short = 'f', long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Cancel unfinished placed work, clean up delivery, then trash the tracker item
    Delete { issue: String },
    /// Update a Task's title, notes, rank, assignee or parent
    Update {
        issue: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        /// Zero-based position in the Project's Task order
        #[arg(long)]
        rank: Option<u32>,
        /// Assignee identity; tracker user ID in connected plans
        #[arg(long)]
        assignee: Option<String>,
        /// Parent Task ID or identifier, or `none` to clear
        #[arg(long, value_name = "ID")]
        parent: Option<String>,
        #[arg(short = 'w', long)]
        wave: Option<String>,
        /// Print the edited Task's ID and planning sync
        #[arg(long)]
        json: bool,
    },
    /// Change Task membership, or move the Task to another Machine, preserving its recorded work
    #[command(
        after_help = "`--machine` stops the Task's Graph, commits and pushes its worktree, and moves it with its Session, then continues the Graph on that Machine at the step that was running. The terminal showing the Session prints `lf resume --task <ID>` to open it there. A worktree mid-merge or mid-rebase refuses to move. A move that was sent but did not arrive shows in `lf task status`; the same command delivers it.\n  lf task move LOO-12 --machine mini"
    )]
    Move {
        issue: String,
        #[arg(
            short = 'w',
            long,
            required_unless_present_any = ["no_wave", "machine"],
            conflicts_with_all = ["no_wave", "machine"]
        )]
        wave: Option<String>,
        #[arg(long, conflicts_with = "machine")]
        no_wave: bool,
        /// Move the Task to this Machine, by label or id
        #[arg(long, value_name = "LABEL_OR_ID")]
        machine: Option<String>,
        #[arg(long, requires = "machine")]
        json: bool,
    },
    /// Post progress to the Task's thread; an agent's report carries its Session's provenance
    Report {
        issue: String,
        message: String,
        #[arg(short = 'w', long)]
        wave: Option<String>,
        /// Print the thread with the entry
        #[arg(long)]
        json: bool,
    },
    /// Turn the Task's auto setting on or off: who takes its interactive steps
    Auto {
        #[command(subcommand)]
        cmd: AutoCommand,
    },
    /// Stop the Task's running Graph or Task run and its scheduled automation
    Interrupt {
        issue: String,
        #[arg(long)]
        json: bool,
    },
    /// Wait without polling an LM
    Wait {
        issue: String,
        #[arg(long, default_value = "terminal", value_parser = ["submitted", "terminal"])]
        until: String,
        #[arg(long)]
        timeout: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

impl TaskCommand {
    pub fn selector(&self) -> Option<&str> {
        match self {
            Self::List { .. } | Self::Create { .. } => None,
            Self::Retire { issue }
            | Self::Status { issue, .. }
            | Self::Abandon { issue, .. }
            | Self::Run { issue, .. }
            | Self::Checkout { issue, .. } => issue.as_deref(),
            Self::Auto { cmd } => cmd.issue().as_deref(),
            Self::Detail { issue, .. }
            | Self::Show { issue, .. }
            | Self::Location { issue, .. }
            | Self::Claim { issue, .. }
            | Self::Unassign { issue, .. }
            | Self::Sync { issue, .. }
            | Self::Move { issue, .. }
            | Self::Complete { issue, .. }
            | Self::Reopen { issue, .. }
            | Self::Diff { issue, .. }
            | Self::Changes { issue, .. }
            | Self::Context { issue, .. }
            | Self::Runs { issue, .. }
            | Self::Find { issue, .. }
            | Self::Grep { issue, .. }
            | Self::Symbols { issue, .. }
            | Self::Files { issue, .. }
            | Self::File { issue, .. }
            | Self::Save { issue, .. }
            | Self::Delete { issue }
            | Self::Update { issue, .. }
            | Self::Report { issue, .. }
            | Self::Interrupt { issue, .. }
            | Self::Wait { issue, .. } => Some(issue),
        }
    }

    pub fn selector_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::List { .. } | Self::Create { .. } => None,
            Self::Retire { issue }
            | Self::Status { issue, .. }
            | Self::Abandon { issue, .. }
            | Self::Run { issue, .. }
            | Self::Checkout { issue, .. } => issue.as_mut(),
            Self::Auto { cmd } => cmd.issue_mut().as_mut(),
            Self::Detail { issue, .. }
            | Self::Show { issue, .. }
            | Self::Location { issue, .. }
            | Self::Claim { issue, .. }
            | Self::Unassign { issue, .. }
            | Self::Sync { issue, .. }
            | Self::Move { issue, .. }
            | Self::Complete { issue, .. }
            | Self::Reopen { issue, .. }
            | Self::Diff { issue, .. }
            | Self::Changes { issue, .. }
            | Self::Context { issue, .. }
            | Self::Runs { issue, .. }
            | Self::Find { issue, .. }
            | Self::Grep { issue, .. }
            | Self::Symbols { issue, .. }
            | Self::Files { issue, .. }
            | Self::File { issue, .. }
            | Self::Save { issue, .. }
            | Self::Delete { issue }
            | Self::Update { issue, .. }
            | Self::Report { issue, .. }
            | Self::Interrupt { issue, .. }
            | Self::Wait { issue, .. } => Some(issue),
        }
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum InstallFrequency {
    Weekly,
    Daily,
    Hourly,
    #[value(name = "5min")]
    FiveMinutes,
}

#[derive(Subcommand, Debug)]
pub enum InstallCommand {
    /// Reconcile the selected installation's local server for a client.
    #[command(hide = true)]
    Server {
        /// Retain a rental caller’s portable profile from stdin before starting.
        #[arg(long)]
        profile_stdin: bool,
    },
    /// Run `lf install` on your stream on a schedule (launchd, also at login; cron on Linux),
    Schedule {
        /// Weekly: Monday 09:00; daily: 09:00; otherwise on clock boundaries (local time).
        /// Defaults to the installed schedule's, else daily.
        #[arg(value_enum)]
        frequency: Option<InstallFrequency>,
    },
    /// Record this Loopflow.app, updated in place, as the install: migrate the
    /// store and link `lf` into it. Moves no files.
    #[command(hide = true)]
    Adopt,
    /// Continue one interrupted installation switch from its pinned candidate.
    #[command(hide = true)]
    RecoverSwitch {
        /// The fixed receipt to continue; omitted, recover the active switch with its pinned owner.
        #[arg(long)]
        switch: Option<String>,
    },
    /// Preview whether this build may replace the global lf (read-only).
    /// Reads the shared store's migration frontier and validates executable
    /// planning references against this binary without changing that frontier.
    /// Exits non-zero on refusal so a caller can gate on it.
    #[command(hide = true)]
    Preflight {
        /// Emit the structured PromotionPreview as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Advance the receipt-selected store with this exact candidate's registry.
    #[command(hide = true)]
    AdvanceSwitch {
        #[arg(long)]
        switch: String,
    },
    /// Promote this build to the global CLI: content-address it into ~/.lf/bin
    /// and atomically repoint the target symlink, under the exclusive promotion
    /// lock. Refuses — leaving every target unchanged — on incompatible
    /// schema or persisted executable evidence.
    #[command(hide = true)]
    Promote {
        /// The global CLI symlink to replace (e.g. ~/.local/bin/lf).
        #[arg(long)]
        cli_target: PathBuf,
        /// A staged Loopflow.app bundle to install alongside the CLI.
        #[arg(long)]
        app_source: Option<PathBuf>,
        /// The global Loopflow.app path to replace atomically.
        #[arg(long)]
        app_target: Option<PathBuf>,
        /// A retired app bundle to remove after the new app commits.
        #[arg(long)]
        legacy_app_target: Option<PathBuf>,
        /// Regenerate global skills after the promotion commits.
        #[arg(long)]
        sync_skills: bool,
        /// Validate and print the preview but change nothing.
        #[arg(long)]
        preview: bool,
    },
    /// Repoint a legacy global CLI at retained prior bytes only after that
    /// binary's own preflight proves it recognizes the current store frontier.
    /// Refuses receipt-managed installs.
    #[command(hide = true)]
    Rollback {
        /// The global CLI symlink to replace (e.g. ~/.local/bin/lf).
        #[arg(long)]
        cli_target: PathBuf,
        /// The immutable content-addressed prior executable to activate.
        #[arg(long)]
        candidate: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum PrCommand {
    /// Check recorded repository landings once, record CI failures, and settle verified merges.
    Reconcile {
        #[arg(long)]
        json: bool,
    },
    /// Show CI status for current branch
    Checks {
        #[arg(short = 'w', long = "watch")]
        watch: bool,
        #[arg(short = 'l', long = "logs")]
        logs: bool,
        /// Show only checks required by repository protection.
        #[arg(long)]
        required: bool,
        /// Print the report as JSON; not with --watch.
        #[arg(long, conflicts_with = "watch")]
        json: bool,
    },

    /// Request or re-request a teammate's review of an existing PR.
    RequestReview {
        /// PR number, URL or branch; defaults to the current branch's PR.
        pr: Option<String>,
        /// GitHub login or org/team. Repeat to request several reviewers.
        #[arg(short = 'r', long, required = true)]
        reviewer: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Read requested reviews and review submissions from GitHub.
    Reviews {
        /// PR number, URL or branch; defaults to the current branch's PR.
        pr: Option<String>,
        /// Emit GitHub CLI JSON, including each review's commit and body.
        #[arg(long)]
        json: bool,
    },

    /// Publish a ready PR headlessly: push, create or refresh, print state + URL.
    /// Opens no review surface.
    Publish {
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "body")]
        body: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Push and create or update a draft PR, then open its GitHub page.
    /// Existing ready PRs stay ready; opening a draft does not publish it.
    Open {
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "body")]
        body: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Prepare a PR to land: sync, clear committed scratch additions and mark ready.
    /// Leave it for manual merge without changing reviewers or assignees.
    Submit {
        #[arg(long)]
        strict: bool,
        #[arg(short = 'w', long = "worktree")]
        worktree: Option<String>,
        #[arg(short = 'm', long = "message")]
        message: Option<String>,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "body")]
        body: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Prepare a PR, request exact-head auto-merge, and return without subscribing.
    Arm {
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        local: bool,
        #[arg(short = 'w', long = "worktree")]
        worktree: Option<String>,
        #[arg(short = 'm', long = "message")]
        message: Option<String>,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "body")]
        body: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Request auto-merge, retain settlement intent, and return.
    Land {
        /// Wait for verified merge and repair failing CI until interrupted or blocked
        #[arg(long)]
        wait_and_fix: bool,
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        local: bool,
        #[arg(short = 'w', long = "worktree")]
        worktree: Option<String>,
        #[arg(short = 'm', long = "message")]
        message: Option<String>,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "body")]
        body: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Abandon branch: close PR, remove worktree, delete branch
    Abandon {
        /// Branch to abandon (default: current)
        branch: Option<String>,
        #[arg(short = 'f', long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum PlanCommand {
    /// Print compact Task rows for a board
    Board {
        wave: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        selection: crate::lf::Selection,
    },
    /// Print each Wave's plan as Desktop shows it: every Task's section, condition and controls
    List {
        /// One Wave; omitted reads the repository's Waves
        wave: Option<String>,
        /// Only this Task, open or closed
        #[arg(long)]
        task: Option<String>,
        /// Every repository's Waves
        #[arg(long)]
        all: bool,
        #[arg(long, required = true)]
        json: bool,
        /// Print a JSON line now and each time the plan changes, until stdin closes
        #[arg(short = 's', long)]
        subscribe: bool,
        #[command(flatten)]
        selection: crate::lf::Selection,
    },
}

#[derive(Subcommand, Debug)]
pub enum RepoTrackerCommand {
    /// Answer a lost line: keep the tracker's value, or save yours again and send it
    #[command(group(clap::ArgGroup::new("answer").args(["keep_theirs", "use_mine"]).required(true)))]
    Answer {
        /// The line's id, as `lf repo tracker` prints it
        id: String,
        /// Keep the tracker's value; the line closes
        #[arg(long)]
        keep_theirs: bool,
        /// Save the lost value again, which sends it
        #[arg(long)]
        use_mine: bool,
        #[arg(long)]
        json: bool,
    },
}

/// Manage the installed artifacts and exported skills.
#[derive(Debug, Subcommand)]
pub enum SelfCommand {
    /// Install the newest Loopflow on your stream from any directory
    #[command(
        after_help = "Without a stream flag, follows the stream recorded in ~/.lf/config.yaml \
(`stable` when none is recorded). A flag switches the stream and records it. \
A stream whose build cannot open this Machine's store keeps the installed lf until it can."
    )]
    Install {
        /// Follow the newest green main build (needs `gh` auth)
        #[arg(long, group = "stream")]
        dev: bool,
        /// Follow the newest release
        #[arg(long, group = "stream")]
        preview: bool,
        /// Follow the newest release soaked 7 days with no open regression
        #[arg(long, group = "stream")]
        stable: bool,
        #[command(subcommand)]
        cmd: Option<InstallCommand>,
    },
    /// Export skills and Graphs into native vendor Skills directories
    #[command(name = "sync-skills")]
    SyncSkills {
        /// Confirm writes under ~/ without prompting
        #[arg(short = 'y', long = "yes")]
        yes: bool,
        /// Keep stale loopflow-generated skills
        #[arg(long = "no-prune")]
        no_prune: bool,
        /// Export repository definitions into this checkout instead of home
        #[arg(long)]
        repo: bool,
    },
}

/// Read Loopflow configuration.
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Adopt a recovery mapping into this clone, or personal global configuration
    Import {
        path: PathBuf,
        /// JSON pointer to the authored mapping, for example /repo/owner~1project
        #[arg(long, default_value = "")]
        pointer: String,
        #[arg(long)]
        global: bool,
        /// Replace conflicting declarations explicitly
        #[arg(long)]
        replace: bool,
    },
    /// Inspect origin profile retention and explicitly keep or replace divergent files
    Retained {
        #[arg(long, conflicts_with = "keep")]
        replace: bool,
        #[arg(long)]
        keep: bool,
    },
    /// Inspect the preference cutover; explicitly accept changed file precedence
    Migrate {
        #[arg(long)]
        accept: bool,
    },
    /// Discover typed settings, destinations, combination rules and portability
    Catalog {
        #[arg(long)]
        json: bool,
    },
    /// Inspect effective settings and declarations for Global, a checkout or a Wave
    Scope(commands::setting::ScopeArgs),
    /// Show effective settings and their sources without launching
    Settings {
        #[arg(long)]
        skill: Option<String>,
        #[arg(long)]
        graph: Option<String>,
        #[arg(long, requires = "graph")]
        step: Option<String>,
        #[command(flatten)]
        output: commands::config::ShowOptions,
    },
    /// Read one file's declaration of a setting: yours for this repository,
    /// a Wave's or every repository's, or the committed one
    #[command(after_help = SETTING_KEYS)]
    Get {
        key: String,
        #[command(flatten)]
        destination: commands::setting::DestinationArgs,
        #[arg(long)]
        json: bool,
    },
    /// Set your own value of a setting for this repository, a Wave or every
    /// repository; it never edits committed config
    #[command(after_help = SETTING_KEYS)]
    Set {
        key: String,
        value: String,
        #[command(flatten)]
        destination: commands::setting::DestinationArgs,
        /// Print the saved declaration
        #[arg(long)]
        json: bool,
    },
    /// Clear your own value of a setting, so the committed config chooses
    #[command(after_help = SETTING_KEYS)]
    Unset {
        key: String,
        #[command(flatten)]
        destination: commands::setting::DestinationArgs,
        /// Print the removed declaration's file
        #[arg(long)]
        json: bool,
    },
    /// Explain how a setting resolves where it would be declared: every
    /// source's candidate and the value a launch there uses
    Explain {
        key: String,
        #[command(flatten)]
        destination: commands::setting::DestinationArgs,
        #[arg(long)]
        json: bool,
    },
    /// Print the configured participant display name.
    User {
        #[arg(long)]
        json: bool,
    },
    /// Establish or print this repository's selected planning identity
    Repo {
        /// Associate this checkout with a repository identity, retaining prior IDs
        #[arg(long)]
        bind: Option<loopflow_model::durable::RepositoryId>,
        /// Reconnect retained Work after a move predating its Git locator
        #[arg(long, conflicts_with = "bind")]
        relocate: Option<loopflow_model::durable::RepositoryId>,
        #[arg(long)]
        json: bool,
    },
}

/// Name and connect to machines.
#[derive(Debug, Subcommand)]
pub enum MachineCommand {
    /// Replace configured forward.files on the named Machine and optionally restart one conversation.
    Forward {
        label: String,
        /// Restart this Task's primary Session after all files arrive
        #[arg(long, conflicts_with = "session")]
        task: Option<String>,
        /// Restart this Session's provider after all files arrive
        #[arg(long)]
        session: Option<String>,
    },
    /// Sign Machines in through each provider's own login, approved in this
    /// Machine's browser; with no Machine named, this one
    Login {
        /// claude, codex, github, linear, asana, doppler or opencode
        provider: loopflow_execution::provider_auth::Provider,
        /// Added Machines to sign in; claude, asana and doppler sign
        /// in on this Machine only (Claude reaches others as a forwarded token)
        machines: Vec<String>,
        /// Open the approval page in this Chrome profile, by name or directory
        #[arg(long)]
        chrome_profile: Option<String>,
        /// Store an API key from the environment instead (OpenCode, this Machine)
        #[arg(long, conflicts_with = "machines")]
        api_key: bool,
        /// Every added Machine whose login reads signed out, or that refused a
        /// launch with none since
        #[arg(long, conflicts_with_all = ["machines", "api_key"])]
        refused: bool,
    },
    /// Sign Machines out through each provider's own logout; with no Machine
    /// named, this one
    Logout {
        /// claude, codex, github, linear, asana, doppler or opencode
        provider: loopflow_execution::provider_auth::Provider,
        /// Added Machines to sign out
        machines: Vec<String>,
    },
    /// What a Machine is signed in to, asked of its own clients; with no
    /// Machine named, this one
    Logins {
        /// An added Machine
        machine: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Receive service credentials (credential bytes use stdin only).
    Credentials {
        #[command(subcommand)]
        cmd: MachineCredentialCommand,
    },
    /// Name this Machine for teammates, or `off` to make it private again.
    /// A private Machine's ownership entries carry only its id.
    Public { name: String },
    /// Diagnose installation, storage and stray lf processes
    Doctor {
        /// Diagnose repository planning without changing it
        #[arg(long = "plan")]
        planning: bool,
        /// Emit the audit as JSON
        #[arg(long)]
        json: bool,
    },
    /// Apply store retention: end runs and agents nobody saw exit, expire
    /// Session history past 90 days and reclaim the space
    #[command(
        after_help = "The Machine server runs this every hour. Turns are kept; history an earlier \
                      lf wrote into loopflow.db moves to history.db."
    )]
    Prune {
        /// Emit the retention report as JSON
        #[arg(long)]
        json: bool,
    },
    /// Print this machine's stable local Machine identity.
    #[command(name = "self")]
    Self_ {
        #[arg(long)]
        json: bool,
    },
    /// Discover a remote machine and save its SSH destination
    Add {
        target: String,
        #[arg(long)]
        label: Option<String>,
        /// The pool of machines you own it joins
        #[arg(long)]
        pool: Option<String>,
        /// What it cost you, committed when added
        #[arg(long, value_name = "USD")]
        cost_usd: Option<loopflow_storage::rental::Usd>,
        /// After this RFC 3339 time it takes no new work
        #[arg(long, value_name = "RFC3339")]
        expires_at: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// List saved machines without connecting
    List {
        label: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Check reachability and version without prompting
    Status {
        label: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Exchange this Machine's plan with an added machine, live, for every
    /// repository without Linear or Asana; bare, show which
    Hub {
        machine: Option<String>,
        /// Stop exchanging with the hub; the plan stays on this Machine
        #[arg(long, conflicts_with = "machine")]
        clear: bool,
    },
    /// Change a saved machine's label
    Rename { label: String, name: String },
    /// Forget a connection without touching remote work
    Remove { label: String },
    /// Give a rented machine back now: copy its records, then delete it
    End {
        label: String,
        /// Delete even though it holds unpushed work, which is lost
        #[arg(long)]
        force: bool,
    },
    /// Group machines you own; a pool takes new work only on a free machine
    Pool {
        #[command(subcommand)]
        cmd: PoolCommand,
    },
    /// Rent machines from a provider within a limit and an hourly price
    Ondemand {
        #[command(subcommand)]
        cmd: OndemandCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum PoolCommand {
    /// Name a pool of machines you own
    Add { name: String },
    /// Each pool's machines, cost, expiry and what they hold
    List {
        #[arg(long)]
        json: bool,
    },
    /// Take no new work; what the pool holds stays
    Off { name: String },
    /// Take new work again
    On { name: String },
    /// Forget a pool; its machines stay added
    Remove { name: String },
}

#[derive(Debug, Subcommand)]
pub enum OndemandCommand {
    /// Name what to rent and the most it may cost
    Add {
        name: String,
        #[arg(long, value_enum)]
        provider: loopflow_storage::rental::Provider,
        #[arg(long)]
        vcpus: u32,
        #[arg(long)]
        memory_gb: u32,
        /// Modal sizes disk itself
        #[arg(long, default_value_t = 10)]
        disk_gb: u32,
        /// One GPU, on runpod (A40, A6000, L4, RTX4090, L40S, A100, H100),
        /// aws (L4, A10G) or modal (T4, L4, A10G, L40S, A100, H100)
        #[arg(long)]
        gpu: Option<String>,
        /// Interruptible capacity on aws or runpod, bid at the admitted price;
        /// the provider may end it early, losing unpushed work
        #[arg(long)]
        spot: bool,
        /// Reserve the full CPU and memory limits on Modal
        #[arg(long)]
        reserved: bool,
        /// Hard deadline for each machine it rents, at most 24
        #[arg(long, default_value_t = loopflow_storage::rental::DEFAULT_HOURS)]
        hours: f64,
        /// Machines rented at once
        #[arg(long)]
        max: u32,
        /// Above this hourly price nothing is rented
        #[arg(long, value_name = "USD")]
        max_usd_per_hour: loopflow_storage::rental::Usd,
        /// Providers each rental is signed in to as it starts (codex, github,
        /// linear or opencode); unset, each of those signed in here. Claude
        /// arrives with each launch as the token `forward.env` names
        #[arg(long, value_delimiter = ',', value_name = "PROVIDER")]
        logins: Vec<loopflow_execution::provider_auth::Provider>,
    },
    /// Templates, budgets and the machines each has rented
    List {
        #[arg(long)]
        json: bool,
    },
    /// Change how many machines it rents at once; current machines stay
    Max { name: String, max: u32 },
    /// Rent a machine by hand; it lasts until its hours, its cap or `lf machine end`
    Rent {
        name: String,
        #[arg(long)]
        hours: Option<f64>,
        /// Spend at most this much on it
        #[arg(long, value_name = "USD")]
        max_usd: Option<loopflow_storage::rental::Usd>,
        #[arg(long)]
        json: bool,
    },
    /// Forget it; its rentals' history stays
    Remove { name: String },
}

#[derive(Debug, Subcommand)]
pub enum MachineCredentialCommand {
    /// Resume the selected conversation with the newly delivered files.
    #[command(hide = true)]
    Restart {
        /// Select the Task's primary Session
        #[arg(long)]
        task: bool,
        id: String,
    },
    /// Receive a fresh login as JSON on stdin; preserve existing accounts.
    Receive,
}

#[derive(Subcommand, Debug)]
pub enum ReleaseCommand {
    /// Run the full release workflow end-to-end
    Run {
        /// Version to release: patch|minor|major|X.Y.Z (default: patch)
        version: Option<String>,
        #[arg(short = 't', long = "target")]
        target: Option<String>,
        /// Print the outcome
        #[arg(long)]
        json: bool,
    },
    /// Check if PRs have merged since the last tag
    Check {
        #[arg(short = 't', long = "target")]
        target: Option<String>,
    },
    /// Generate release notes for a version
    Notes {
        /// Version (e.g. 0.9.6)
        version: String,
        #[arg(long = "prev-tag")]
        prev_tag: Option<String>,
        /// Print notes without updating manifests or release archives
        #[arg(long)]
        preview: bool,
        #[arg(short = 't', long = "target")]
        target: Option<String>,
    },
    /// Bump version in manifest files
    Bump {
        /// Version to bump to (e.g. 0.9.6)
        version: String,
        #[arg(short = 't', long = "target")]
        target: Option<String>,
    },
    /// Create a git tag and push it
    Tag {
        /// Version to tag (e.g. 0.9.6)
        version: String,
        #[arg(short = 't', long = "target")]
        target: Option<String>,
    },
    /// Stage or publish a GitHub Release
    Publish {
        /// Release tag (for example v0.12.4)
        tag: String,
        /// Release notes used while creating or updating the draft
        #[arg(long)]
        notes: Option<PathBuf>,
        /// Asset to upload; repeat for multiple files
        #[arg(long = "asset")]
        assets: Vec<PathBuf>,
        /// Publish the existing draft and mark it latest
        #[arg(long)]
        finalize: bool,
    },
    /// Check release workflow status
    Status {
        #[arg(short = 't', long = "target")]
        target: Option<String>,
    },
    /// List the regressions reported against a release, and what they hold
    Issues {
        /// Release (e.g. v0.15.3); every open regression without one
        version: Option<String>,
        #[arg(long)]
        json: bool,
        #[arg(short = 't', long = "target")]
        target: Option<String>,
    },
    /// File a regression against a release
    Regress {
        /// Release it regressed in (e.g. v0.15.3)
        version: String,
        /// Issue title
        title: String,
    },
    /// Move stable to the newest soaked release no regression covers
    Promote {
        /// Say what would move or what holds it, changing nothing
        #[arg(long)]
        dry_run: bool,
        #[arg(short = 't', long = "target")]
        target: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum WtCommand {
    /// Create a low-level worktree following the repository's existing layout
    Create {
        /// Worktree name
        name: String,
        /// Print the placement plan without creating a worktree
        #[arg(long)]
        plan: bool,
        /// Keep this workspace after delivery
        #[arg(long)]
        persistent: bool,
        /// Start the branch from this checkout's branch instead of main, and
        /// keep following it: `lf sync` merges it and `lf pr` targets it
        #[arg(long)]
        stack: bool,
        /// Print the worktree, or the placement with --plan
        #[arg(long)]
        json: bool,
    },
    /// Switch to a worktree by name, identity leaf, or full branch
    Switch {
        /// Worktree name or full branch name to switch to
        name: String,
    },
    /// List worktrees (read-only; reflects the last-synced main)
    List {
        #[arg(long)]
        json: bool,
        /// Fetch origin and fast-forward main before listing (mutates the
        /// canonical checkout). Off by default so a list never touches it.
        #[arg(long)]
        sync: bool,
    },
    /// Report how long `lf wt list` has taken on this machine
    Timing {
        #[arg(long)]
        json: bool,
    },
    /// Prune worktrees, completed build output, or finished branches
    #[command(
        after_help = "Removes checkouts whose PR merged or closed, whose Task finished, or that \
                      have no Task or PR and sat untouched for a week, and names why each other \
                      checkout is kept. Changes under scratch/ and .lf/ never keep a checkout. \
                      Unpushed commits and source changes are saved under \
                      refs/lf/pruned/<branch>/ before removal."
    )]
    Prune {
        /// Remove every checkout no unfinished Task or running process holds
        #[arg(long, conflicts_with_all = ["builds", "branches"])]
        all: bool,
        /// Show what would be pruned without removing anything
        #[arg(long)]
        dry_run: bool,
        /// Reclaim old generated output from completed Tasks; retain checkouts
        #[arg(long)]
        builds: bool,
        /// Delete origin and local branches tied to merged PRs, finished
        /// Tasks, or published release candidates
        #[arg(long, conflicts_with = "builds")]
        branches: bool,
        /// Emit every checkout's verdict, or build cleanup receipts
        #[arg(long, conflicts_with = "branches")]
        json: bool,
        /// Keep output modified within this many hours
        #[arg(long, requires = "builds", default_value_t = 24)]
        retention_hours: u64,
        /// Maximum generated roots removed per invocation
        #[arg(long, requires = "builds", default_value_t = 8)]
        maximum_roots: usize,
        /// Maximum allocated bytes removed per invocation
        #[arg(long, requires = "builds", default_value_t = 64 * 1024 * 1024 * 1024)]
        maximum_bytes: u64,
    },
    /// Delete a worktree and its local and remote branch; retain PR and Task outcomes
    Delete {
        /// Worktree name to remove
        name: String,
        #[arg(short = 'f', long = "force")]
        force: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn every_provider_signs_in_and_out_through_machine_login() {
        for args in [
            vec!["lf", "--account", "claude=old@", "implement"],
            vec!["lf", "--only-account", "codex=old@", "implement"],
            vec!["lf", "session", "list", "--blocked"],
            vec!["lf", "machine", "credentials", "renew"],
            vec!["lf", "machine", "credentials", "inspect", "github", "jack"],
            vec!["lf", "machine", "login", "codex", "mini", "--refused"],
        ] {
            assert!(Cli::try_parse_from(&args).is_err(), "{args:?}");
        }
        for provider in [
            "claude", "codex", "github", "linear", "asana", "doppler", "opencode",
        ] {
            for verb in ["login", "logout"] {
                assert!(Cli::try_parse_from(["lf", "machine", verb, provider, "mini"]).is_ok());
            }
        }
        assert!(Cli::try_parse_from(["lf", "machine", "login", "claude", "--refused"]).is_ok());
        assert!(
            Cli::try_parse_from(["lf", "session", "move", "--on", "claude", "-a", "codex"]).is_ok()
        );
    }

    #[test]
    fn workspace_options_belong_to_open_and_exclude_one_conversation_modes() {
        for flags in [
            ["--here", "--tab"],
            ["--here", "--focus"],
            ["--tab", "--focus"],
            ["--workspace", "--here"],
            ["--workspace", "--tab"],
            ["--available", "--focus"],
            ["--available", "--workspace"],
        ] {
            assert!(Cli::try_parse_from(["lf", "open", flags[0], flags[1]]).is_err());
        }
        for option in ["--workspace", "--available", "--no-focus"] {
            assert!(Cli::try_parse_from(["lf", "open", option]).is_ok());
        }
    }

    #[test]
    fn desktop_shell_runtime_needs_no_gui_and_task_shell_is_removed() {
        assert!(Cli::try_parse_from(["lf", "task", "shell", "task-id"]).is_err());
        let cli = Cli::try_parse_from([
            "lf", "desktop", "shell", "task-id", "--id", "shell-id", "--create",
        ])
        .unwrap();
        // Only Close names the Machine that started the shell.
        assert!(Cli::try_parse_from([
            "lf",
            "desktop",
            "shell",
            "task-id",
            "--id",
            "s",
            "--started-on",
            "home_1",
        ])
        .is_err());
        let Some(Commands::Desktop { cmd }) = cli.command else {
            panic!("Desktop runtime command");
        };
        assert!(crate::lf::commands::desktop::runs_without_desktop(&cmd));
        assert!(cmd.changes_work());
        assert!(!cmd.is_read());
        let mut command = Cli::command();
        let task = command.find_subcommand_mut("task").unwrap();
        assert!(task.find_subcommand("shell").is_none());
        let desktop = command.find_subcommand_mut("desktop").unwrap();
        assert!(!desktop.render_help().to_string().contains("shell runtime"));
    }

    #[test]
    fn only_changes_to_work_record_a_process() {
        let records = |args: &[&str]| {
            Cli::try_parse_from(["lf"].iter().chain(args))
                .unwrap()
                .command
                .is_some_and(|command| command.changes_work())
        };
        assert!(!records(&["desktop", "focus"]));
        assert!(!records(&[
            "desktop",
            "dashboard",
            "rename",
            "query:q",
            "Mine",
            "--revision",
            "1"
        ]));
        assert!(records(&[
            "desktop",
            "dashboard",
            "rename",
            "task:t",
            "Mine",
            "--revision",
            "1"
        ]));
        assert!(records(&["desktop", "send", "hello"]));
        assert!(records(&["task", "create", "--title", "x"]));
        assert!(!records(&["task", "status", "x"]));
    }

    #[test]
    fn retired_surface_flags_are_rejected_for_a_valid_skill_command() {
        for flag in ["--ide", "--tui"] {
            let error = Cli::try_parse_from(["lf", flag, "skill", "debug"]).unwrap_err();
            assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
            assert!(error.to_string().contains(flag));
        }
    }

    #[test]
    fn select_belongs_to_the_reads_that_honour_it() {
        let words = |w: &[&str]| w.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        assert_eq!(
            selected_paths(&words(&[
                "lf", "session", "list", "--json", "--select", "id,title"
            ])),
            Some(vec!["id".to_string(), "title".to_string()])
        );
        assert_eq!(
            selected_paths(&words(&["lf", "session", "list", "--json"])),
            None
        );
        assert!(
            Cli::try_parse_from(["lf", "task", "changes", "A-1", "--json", "--select", "id"])
                .is_err()
        );
    }

    #[test]
    fn session_list_selects_wave_and_optional_primary_without_changing_launch_wave() {
        use loopflow_execution::session::SessionPrimary;
        for (words, expected) in [
            (vec!["--primary"], SessionPrimary::All),
            (vec!["--primary", "task"], SessionPrimary::Task),
            (vec!["--primary=wave"], SessionPrimary::Wave),
            (vec!["--primary", "repository"], SessionPrimary::Repository),
        ] {
            let cli = Cli::try_parse_from(
                ["lf", "session", "list", "--wave", "craft", "--all"]
                    .into_iter()
                    .chain(words),
            )
            .unwrap();
            assert!(cli.wave.is_none());
            let Some(Commands::Session {
                cmd: SessionCommand::List(args),
            }) = cli.command
            else {
                panic!("session list")
            };
            assert_eq!(args.primary, Some(expected));
            assert_eq!(args.wave.as_deref(), Some("craft"));
            assert!(!args.headless);
            assert!(args.all);
        }
        assert!(Cli::try_parse_from(["lf", "session", "list", "--offset", "1"]).is_err());
        assert!(Cli::try_parse_from(["lf", "session", "list", "--primary", "former"]).is_err());
    }

    #[test]
    fn session_list_adds_headless_and_rejects_history_and_mode_filters() {
        let cli = Cli::try_parse_from(["lf", "session", "list", "--headless"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Session {
                cmd: SessionCommand::List(SessionListArgs { headless: true, .. })
            })
        ));
        for flags in [
            vec!["--history"],
            vec!["--interactive", "true"],
            vec!["--interactive", "false"],
            vec!["--interactive", "all"],
        ] {
            assert!(
                Cli::try_parse_from(["lf", "session", "list"].into_iter().chain(flags)).is_err()
            );
        }
    }

    #[test]
    fn orphan_selects_inventory_and_cannot_opt_out_at_launch() {
        let cli = Cli::try_parse_from(["lf", "session", "list", "--orphan", "--json"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Session {
                cmd: SessionCommand::List(SessionListArgs { orphan: true, .. })
            })
        ));
        assert!(
            Cli::try_parse_from(["lf", "session", "list", "--orphan", "--task", "LOO-353"])
                .is_err()
        );
        assert!(Cli::try_parse_from(["lf", "--orphan", ":", "Start a conversation"]).is_err());
    }

    #[test]
    fn consolidated_commands_parse_without_old_namespaces() {
        use clap::CommandFactory;
        let command = Cli::command();
        assert!(command.find_subcommand("pm").is_none());
        assert!(command.find_subcommand("work").is_none());
        for verb in [
            "start", "stop", "pause", "resume", "catalog", "ls", "status",
        ] {
            assert!(
                command.find_subcommand(verb).is_none(),
                "removed root command {verb}"
            );
        }
        assert!(command.find_subcommand("op").is_none());
        let literal = crate::lf::navigation::normalize_args(
            ["lf", "wt", "create", "csv-export"]
                .map(String::from)
                .to_vec(),
        )
        .unwrap();
        assert_eq!(literal, ["lf", "wt", "create", "csv-export"]);
        assert!(Cli::try_parse_from(literal).is_ok());
        assert!(command
            .find_subcommand("task")
            .unwrap()
            .find_subcommand("worktree")
            .is_none());
        for verb in ["enable", "disable", "serve"] {
            assert!(Cli::try_parse_from(["lf", "wave", verb, "product"]).is_err());
        }
        for removed in ["start", "stop", "pause", "resume", "chat", "reply"] {
            assert!(command.find_subcommand(removed).is_none());
        }
        assert!(Cli::try_parse_from(["lf", "steer", "session_1", "m", "--repo"]).is_err());
        assert!(Cli::try_parse_from(["lf", "session", "steer", "session_1", "m"]).is_err());
        assert!(Cli::try_parse_from(["lf", "task", "steer", "LOO-1", "m"]).is_err());
        assert!(Cli::try_parse_from(["lf", "wave", "serve", "product"]).is_err());
        assert!(Cli::try_parse_from(["lf", "wave", "place", "product", "mini"]).is_err());
        assert!(Cli::try_parse_from(["lf", "task", "admission", "LOO-1"]).is_err());
        assert!(Cli::try_parse_from(["lf", "wave", "machine", "product", "mini"]).is_err());
        assert!(Cli::try_parse_from(["lf", "task", "handoff", "LOO-1", "mini"]).is_err());
        for args in [
            vec!["lf", "list"],
            vec!["lf", "wave", "list", "--json"],
            vec!["lf", "pr", "checks"],
            vec!["lf", "--wave", "product", "steer", "keep attribution"],
            vec!["lf", "--repo", "loopflow", "steer", "keep attribution"],
            vec!["lf", "steer", "session_1", "keep attribution"],
            vec!["lf", "steer", "LOO-1", "keep attribution"],
            vec!["lf", "wave", "rename", "product", "--repo", "../other"],
            vec!["lf", "task", "move", "LOO-1", "--machine", "mini"],
            vec![
                "lf", "--wave", "product", "config", "set", "machine", "mini",
            ],
            vec!["lf", "config", "unset", "agent", "--global"],
            vec!["lf", "discord", "serve", "product"],
            vec!["lf", "machine", "doctor", "--plan", "--json"],
            vec!["lf", "wave", "sync", "product"],
            vec![
                "lf",
                "task",
                "auto",
                "off",
                "LOO-1",
                "--reason",
                "waiting on review",
            ],
        ] {
            assert!(Cli::try_parse_from(args.clone()).is_ok(), "{args:?}");
        }
        assert!(matches!(
            Cli::try_parse_from(["lf", "task", "unassign", "LOO-1"]).unwrap().command,
            Some(Commands::Task { cmd: TaskCommand::Unassign { issue, .. } }) if issue == "LOO-1"
        ));
        for verb in [
            "recover", "enable", "disable", "prepare", "advance", "defer", "resume", "release",
            "unclaim",
        ] {
            assert!(Cli::try_parse_from(["lf", "task", verb, "LOO-1"]).is_err());
        }
        assert!(Cli::try_parse_from(["lf", "wave", "status", "--no-sync"]).is_err());
        assert!(Cli::try_parse_from(["lf", "wt", "list", "--full"]).is_err());
    }

    #[test]
    fn version_output_uses_the_embedded_build_identity() {
        let error = Cli::try_parse_from(["lf", "--version"]).expect_err("version exits");
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayVersion);
        assert_eq!(
            error.to_string(),
            format!("lf {}\n", crate::build_info::VERSION_LINE)
        );
    }

    #[test]
    fn server_access_is_separate_from_machine_routing_and_installation() {
        let args = crate::lf::navigation::normalize_args(
            ["lf", "server", "status"].map(str::to_owned).to_vec(),
        )
        .unwrap();
        assert_eq!(args, ["lf", "server", "status"]);
        let command = Cli::try_parse_from(args).unwrap().command.unwrap();
        assert!(!command.changes_work());
        assert!(matches!(
            command,
            Commands::Server {
                cmd: ServerCommand::Status
            }
        ));
        let describe = Cli::try_parse_from(["lf", "server", "describe", "--json"])
            .unwrap()
            .command
            .unwrap();
        assert!(!describe.changes_work());
        assert!(Cli::try_parse_from(["lf", "machine", "service", "run"]).is_err());
        assert!(Cli::try_parse_from(["lf", "server", "ensure"]).is_err());
        let tree = crate::lf::navigation::command_tree();
        let install = tree
            .find_subcommand("self")
            .unwrap()
            .find_subcommand("install")
            .unwrap();
        assert!(install.find_subcommand("server").unwrap().is_hide_set());
        let remote = crate::lf::navigation::machine_invocation(
            &["lf", "--machine", "worker", "server", "status"].map(str::to_owned),
        )
        .unwrap()
        .unwrap();
        assert_eq!(remote.1, ["server", "status"]);
    }

    #[test]
    fn install_exposes_refresh_and_schedule_but_hides_transaction_commands() {
        let mut command = Cli::command();
        let installation = command.find_subcommand_mut("self").unwrap();
        assert!(installation
            .render_long_help()
            .to_string()
            .contains("install"));
        let help = installation
            .find_subcommand_mut("install")
            .unwrap()
            .render_long_help()
            .to_string();
        assert!(help.contains("schedule"));
        assert!(!help.contains("preflight"));
        assert!(matches!(
            Cli::try_parse_from(["lf", "self", "install"])
                .unwrap()
                .command,
            Some(Commands::Self_ {
                cmd: crate::lf::SelfCommand::Install {
                    dev: false,
                    preview: false,
                    stable: false,
                    cmd: None
                }
            })
        ));
        assert!(Cli::try_parse_from(["lf", "machine", "install"]).is_err());
        assert!(Cli::try_parse_from(["lf", "self", "install", "schedule"]).is_ok());
        assert!(Cli::try_parse_from(["lf", "self", "install", "status"]).is_err());
        assert!(Cli::try_parse_from(["lf", "self", "install", "preflight"]).is_ok());
        assert!(matches!(
            Cli::try_parse_from(["lf", "self", "install", "--preview"])
                .unwrap()
                .command,
            Some(Commands::Self_ {
                cmd: crate::lf::SelfCommand::Install {
                    dev: false,
                    preview: true,
                    stable: false,
                    cmd: None
                }
            })
        ));
        assert!(Cli::try_parse_from(["lf", "self", "install", "--dev", "--stable"]).is_err());
        assert!(Cli::try_parse_from(["lf", "self", "install", "--main"]).is_err());
    }

    #[test]
    fn discovery_and_wave_reads_have_distinct_owners() {
        let command = Cli::try_parse_from(["lf", "list"]).unwrap();
        assert!(matches!(command.command, Some(Commands::List { .. })));
        assert!(Cli::try_parse_from(["lf", "--list"]).is_err());
        let waves = Cli::try_parse_from(["lf", "wave", "list", "--json"]).unwrap();
        assert!(matches!(
            waves.command,
            Some(Commands::Wave {
                cmd: WaveCommand::List {
                    json: true,
                    all: false
                }
            })
        ));
        assert!(Cli::try_parse_from(["lf", "wave", "probe", "product", "--json"]).is_err());
        assert!(Cli::try_parse_from(["lf", "pr", "checks", "--logs"]).is_ok());
        assert!(Cli::try_parse_from(["lf", "machine", "probe", "product"]).is_err());
        assert!(Cli::try_parse_from(["lf", "wt", "ci"]).is_err());
    }

    #[test]
    fn task_creation_needs_no_project_selector() {
        let task = Cli::try_parse_from([
            "lf",
            "task",
            "create",
            "--wave",
            "product",
            "--title",
            "Ship the outcome",
        ])
        .unwrap();
        assert!(
            matches!(task.command, Some(Commands::Task { cmd: TaskCommand::Create { wave: Some(wave), .. } }) if wave == "product")
        );
        assert!(Cli::try_parse_from(["lf", "--project", "old", "research"]).is_err());
    }

    #[test]
    fn direct_work_selector_accepts_an_inline_question() {
        let cli = Cli::try_parse_from([
            "lf",
            "--batch",
            "--wave",
            "product",
            ":",
            "Which Task matters?",
        ])
        .expect("parse bound inline prompt");

        assert_eq!(cli.wave.as_deref(), Some("product"));
        assert!(matches!(
            cli.command,
            Some(Commands::Inline { prompt }) if prompt == vec!["Which Task matters?"]
        ));
    }

    #[test]
    fn ci_report_accepts_machine_wide_filters() {
        let cli = Cli::try_parse_from([
            "lf",
            "repo",
            "ci",
            "--since",
            "24h",
            "--wave",
            "infrastructure",
            "--repo",
            "loopflowstudio/loopflow",
            "--json",
        ])
        .expect("parse CI report");
        assert!(matches!(
            cli.command,
            Some(Commands::Repo {
                cmd: Some(RepoCommand::Ci {
                    since,
                    wave: Some(wave),
                    repo: Some(repo),
                    json: true,
                }),
                ..
            }) if since == "24h" && wave == "infrastructure" && repo == "loopflowstudio/loopflow"
        ));
    }

    /// A provider running across an lf upgrade keeps the Stop hook an older lf
    /// declared, so that spelling still reaches the turn-end boundary.
    #[test]
    fn a_retained_stop_hook_spelling_reaches_the_turn_end() {
        let cli = Cli::try_parse_from(["lf", "__planning-check"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::ContextBlock {
                delivery: None,
                moment: loopflow_execution::context_block::ContextMoment::Stop,
                harness: None,
            })
        ));
    }

    #[test]
    fn only_provider_side_commands_are_hidden_and_the_root_hides_no_flag() {
        let root = Cli::command();
        let hidden = root
            .get_subcommands()
            .filter(|command| command.is_hide_set())
            .map(|command| command.get_name().to_string())
            .collect::<Vec<_>>();
        assert_eq!(hidden, ["__context-block"]);
        fn internal(command: &clap::Command) -> Vec<String> {
            command
                .get_subcommands()
                .flat_map(|child| {
                    let mut names = internal(child);
                    if child.get_name().starts_with("__") {
                        names.push(child.get_name().to_string());
                    }
                    names
                })
                .collect()
        }
        assert_eq!(internal(&root), ["__context-block"]);
        let hidden_flags = root
            .get_arguments()
            .filter(|arg| arg.is_hide_set())
            .map(|arg| arg.get_id().to_string())
            .collect::<Vec<_>>();
        assert!(hidden_flags.is_empty(), "{hidden_flags:?}");
    }

    #[test]
    fn pm_init_accepts_positional_wave() {
        let cli = Cli::try_parse_from(["lf", "wave", "plan", "connect", "pm"]).expect("parse");
        let Some(Commands::Wave {
            cmd:
                WaveCommand::Plan {
                    cmd: Some(WavePlanCommand::Connect { wave, all, .. }),
                    ..
                },
        }) = cli.command
        else {
            panic!("expected pm init command");
        };

        assert_eq!(wave.as_deref(), Some("pm"));
        assert!(!all);
    }

    #[test]
    fn task_checkout_accepts_worktree_options_without_starting_a_worker() {
        let cli = Cli::try_parse_from([
            "lf",
            "task",
            "checkout",
            "INF-123",
            "--name",
            "runtime-research",
            "--stack-on",
            "INF-122",
            "--directive",
            "collect both reports",
            "--design",
            "scratch/child.md",
            "--json",
        ])
        .expect("parse task checkout");
        let Some(Commands::Task {
            cmd:
                TaskCommand::Checkout {
                    issue,
                    name,
                    stack_on,
                    directive,
                    design,
                    json,
                    ..
                },
        }) = cli.command
        else {
            panic!("expected task checkout command");
        };
        assert_eq!(issue.as_deref(), Some("INF-123"));
        assert_eq!(name.as_deref(), Some("runtime-research"));
        assert_eq!(stack_on.as_deref(), Some("INF-122"));
        assert_eq!(directive.as_deref(), Some("collect both reports"));
        assert_eq!(
            design.as_deref(),
            Some(std::path::Path::new("scratch/child.md"))
        );
        assert!(json);
    }

    #[test]
    fn task_create_uses_wave_and_allows_piped_title_omission() {
        let cli = Cli::try_parse_from(["lf", "task", "create", "--wave", "product"])
            .expect("parse Task create with Wave");
        let Some(Commands::Task {
            cmd: TaskCommand::Create { wave, title, .. },
        }) = cli.command
        else {
            panic!("expected task create command");
        };
        assert_eq!(wave.as_deref(), Some("product"));
        assert_eq!(title, None);
        assert!(Cli::try_parse_from(["lf", "task", "create"]).is_ok());
        assert!(Cli::try_parse_from(["lf", "task", "start", "old entry"]).is_err());
        assert!(matches!(
            Cli::try_parse_from(["lf", "pm", "task", "create", "--title", "old entry"])
                .unwrap()
                .command,
            Some(Commands::External(_))
        ));
        assert!(Cli::try_parse_from(["lf", "task", "create", "--name", "placed"]).is_err());
        assert!(Cli::try_parse_from(["lf", "task", "create", "--graph", "proof"]).is_err());
        assert!(
            Cli::try_parse_from(["lf", "graph", "start", "proof"]).is_ok_and(|cli| !matches!(
                cli.command,
                Some(Commands::Graph {
                    cmd: GraphCommand::List { .. } | GraphCommand::Show { .. }
                })
            ))
        );
        let cli = Cli::try_parse_from(["lf", "-b", "task", "run", "INF-123", "proof"]).unwrap();
        assert!(cli.batch);
        assert!(matches!(
            cli.command,
            Some(Commands::Task {
                cmd: TaskCommand::Run { issue, graph: Some(graph), .. }
            }) if issue.as_deref() == Some("INF-123") && graph == "proof"
        ));
    }

    #[test]
    fn landing_wait_and_fix_is_explicit_and_serial_dispositions_are_rejected() {
        let land = Cli::try_parse_from(["lf", "pr", "land", "--wait-and-fix"])
            .expect("parse waited landing");
        assert!(matches!(
            land.command,
            Some(Commands::Pr {
                cmd: Some(PrCommand::Land {
                    wait_and_fix: true,
                    ..
                })
            })
        ));
        assert!(Cli::try_parse_from(["lf", "pr", "land", "--wait"]).is_err());
        for command in ["land", "arm", "submit"] {
            assert!(Cli::try_parse_from(["lf", "pr", command, "-c"]).is_err());
            assert!(Cli::try_parse_from(["lf", "pr", command, "--next", "follow-up"]).is_err());
        }
        assert!(Cli::try_parse_from(["lf", "pr", "next"]).is_err());
    }

    #[test]
    fn task_deletion_accepts_issue_identity() {
        let cli = Cli::try_parse_from(["lf", "task", "delete", "LOO-42"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Task {
            cmd: TaskCommand::Delete { issue }
        }) if issue == "LOO-42"));
        assert!(Cli::try_parse_from(["lf", "task", "delete"]).is_err());
    }

    #[test]
    fn task_run_open_is_optional_and_has_no_start_alias() {
        for (args, expected) in [
            (vec!["lf", "task", "run", "LOO-42"], false),
            (vec!["lf", "task", "run", "LOO-42", "--open"], true),
            (
                vec!["lf", "--task", "LOO-42", "task", "run", "--open"],
                true,
            ),
            (vec!["lf", "task", "run", "--open"], true),
        ] {
            let cli = Cli::try_parse_from(args).unwrap();
            assert!(matches!(cli.command, Some(Commands::Task {
                cmd: TaskCommand::Run { open, .. }
            }) if open == expected));
        }
        assert!(Cli::try_parse_from(["lf", "task", "start", "LOO-42"]).is_err());
    }

    #[test]
    fn task_run_takes_a_step_and_completion_is_its_own_command() {
        let cli =
            Cli::try_parse_from(["lf", "task", "run", "LOO-42", "--from", "implement"]).unwrap();
        assert!(matches!(cli.command,
            Some(Commands::Task { cmd: TaskCommand::Run { issue, graph: None, from, .. } })
                if issue.as_deref() == Some("LOO-42") && from.as_deref() == Some("implement")));
        assert!(Cli::try_parse_from(["lf", "task", "workflow", "move", "LOO-42", "end"]).is_err());
        let cli =
            Cli::try_parse_from(["lf", "task", "complete", "LOO-42", "--reason", "Delivered"])
                .unwrap();
        assert!(matches!(cli.command,
            Some(Commands::Task { cmd: TaskCommand::Complete { issue, reason, .. } })
                if issue == "LOO-42" && reason.as_deref() == Some("Delivered")));
        assert!(Cli::try_parse_from([
            "lf",
            "task",
            "complete",
            "LOO-42",
            "--summary",
            "Delivered"
        ])
        .is_err());
        assert!(matches!(
            Cli::try_parse_from(["lf", "pm", "task", "done", "--id", "LOO-42"])
                .unwrap()
                .command,
            Some(Commands::External(_))
        ));
    }

    #[test]
    fn task_interrupt_authors_no_direction() {
        let cli = Cli::try_parse_from(["lf", "task", "interrupt", "INF-123"])
            .expect("parse task interrupt");
        let Some(Commands::Task {
            cmd: TaskCommand::Interrupt { issue, json },
        }) = cli.command
        else {
            panic!("expected task interrupt command");
        };
        assert_eq!(issue, "INF-123");
        assert!(!json);
        assert!(Cli::try_parse_from([
            "lf",
            "task",
            "interrupt",
            "INF-123",
            "--message",
            "take the smaller approach",
        ])
        .is_err());
    }

    #[test]
    fn steer_and_task_report_take_a_message() {
        let steer = Cli::try_parse_from(["lf", "steer", "INF-123", "take the smaller approach"])
            .expect("parse steer");
        let Some(Commands::Steer {
            target,
            message,
            json,
        }) = steer.command
        else {
            panic!("expected steer command");
        };
        assert_eq!(target.as_deref(), Some("INF-123"));
        assert_eq!(message, "take the smaller approach");
        assert!(Cli::try_parse_from(["lf", "steer"]).is_err());
        assert!(matches!(
            Cli::try_parse_from(["lf", "task", "report", "INF-123", "landed"])
                .unwrap()
                .command,
            Some(Commands::Task {
                cmd: TaskCommand::Report { .. }
            })
        ));
        assert!(Cli::try_parse_from(["lf", "task", "comment", "INF-123", "landed"]).is_err());
        assert!(
            Cli::try_parse_from(["lf", "task", "update", "INF-123", "--notes", "revised"]).is_ok()
        );
        for removed in ["update", "comments"] {
            assert!(matches!(
                Cli::try_parse_from(["lf", "pm", "task", removed, "--id", "INF-123"])
                    .unwrap()
                    .command,
                Some(Commands::External(_))
            ));
        }
        assert!(!json);

        for removed in ["acknowledge", "decide", "request-decision"] {
            assert!(
                Cli::try_parse_from(["lf", "task", removed, "INF-123"]).is_err(),
                "{removed} must not remain as a compatibility command"
            );
        }
    }

    #[test]
    fn navigation_belongs_to_graph_decisions() {
        for args in [
            vec!["lf", "task", "run", "LOO-1", "--session", "review"],
            vec!["lf", "session", "advance", "review", "approved"],
            vec!["lf", "session", "iterate", "review", "revise"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
        assert!(Cli::try_parse_from(["lf", "session", "complete", "review"]).is_err());
    }

    #[test]
    fn session_work_flags_normalize_before_machine_routing() {
        let cli = Cli::try_parse_from(["lf", "session", "resume", "session-id"])
            .unwrap()
            .checked()
            .unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Session {
                cmd: SessionCommand::Resume { id: Some(id), .. }
            }) if id == "session-id"
        ));
        for command in ["resume", "new"] {
            for (flag, value) in [("--task", "LOO-1"), ("--wave", "infra")] {
                for argv in [
                    vec!["lf", flag, value, "session", command, "--prepare"],
                    vec!["lf", "session", command, flag, value, "--prepare"],
                ] {
                    let cli = Cli::try_parse_from(argv).unwrap().checked().unwrap();
                    assert_eq!(
                        if flag == "--task" {
                            cli.task.as_deref()
                        } else {
                            cli.wave.as_deref()
                        },
                        Some(value)
                    );
                    assert!(!cli.may_launch_agent());
                }
            }
        }
        // Auto on, and the Graph run `lf task run -a` starts, carry the
        // run's agent to the steps they start.
        let checked = |argv: &[&str]| Cli::try_parse_from(argv).unwrap().checked().unwrap();
        for tail in [&["task", "auto", "on"][..], &["graph", "run"]] {
            let argv = [
                &["lf", "--agent", "claude:opus", "--task", "LOO-1"][..],
                tail,
            ]
            .concat();
            assert!(checked(&argv).may_launch_agent(), "{argv:?}");
        }
        assert!(!checked(&["lf", "--task", "LOO-1", "task", "auto", "off"]).may_launch_agent());
        // `lf task auto on ISSUE` selects its Task as `--task` does.
        let on = checked(&[
            "lf", "task", "auto", "on", "LOO-1", "--graph", "code", "--reason", "go", "--json",
        ]);
        assert_eq!(on.task.as_deref(), Some("LOO-1"));
        assert!(matches!(
            &on.command,
            Some(Commands::Task { cmd: TaskCommand::Auto { cmd: AutoCommand::On {
                issue: None, graph: Some(graph), reason: Some(reason), json: true
            } } }) if reason == "go" && graph == "code"
        ));
        // `lf ship` is no command of its own: it would name a skill.
        assert!(matches!(
            Cli::try_parse_from(["lf", "ship", "LOO-1"])
                .unwrap()
                .command,
            Some(Commands::External(_))
        ));
        // A graph is taken up as auto goes on; off takes none.
        assert!(Cli::try_parse_from(["lf", "task", "auto", "off", "--graph", "code"]).is_err());
        // A Session id beside its Work selects the Session; Task and Wave
        // together stay one selection too many.
        for argv in [
            vec!["lf", "--task", "LOO-1", "session", "resume", "session-id"],
            vec!["lf", "session", "resume", "session-id", "--wave", "infra"],
        ] {
            let cli = checked(&argv);
            let Some(Commands::Session {
                cmd: SessionCommand::Resume { id, connection, .. },
            }) = &cli.command
            else {
                panic!("not a resume");
            };
            assert!(matches!(
                connection.target(&cli, id.as_deref()).unwrap(),
                loopflow_execution::ops::human_session::SessionTarget::Session("session-id")
            ));
        }
        assert!(Cli::try_parse_from([
            "lf", "--wave", "infra", "session", "resume", "--task", "LOO-1",
        ])
        .and_then(Cli::checked)
        .is_err());
    }

    #[test]
    fn cli_rejects_review_handshake_and_preserves_native_open() {
        assert!(Cli::try_parse_from(["lf", "session", "ask", "Help"]).is_err());
        for args in [
            vec!["lf", "session", "ready", "Ready for review"],
            vec!["lf", "session", "complete", "review"],
            vec!["lf", "session", "advance", "review"],
            vec!["lf", "session", "iterate", "review"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }

        let open = Cli::try_parse_from(["lf", "session", "resume", "run_123", "--fork"])
            .expect("parse forked open");
        assert!(matches!(
            open.command,
            Some(Commands::Session {
                cmd: SessionCommand::Resume {
                    id,
                    fork: true,
                    connection: SessionConnectionArgs { json: false, .. },
                    ..
                }
            }) if id.as_deref() == Some("run_123")
        ));
        for removed in [
            vec!["lf", "session", "connect", "run_123"],
            vec!["lf", "session", "attach", "run_123"],
            vec!["lf", "session", "resume", "run_123", "--takeover"],
            vec!["lf", "session", "resume", "run_123", "--try"],
            vec!["lf", "session", "resume", "--fresh"],
            vec!["lf", "session", "resume", "--all"],
        ] {
            assert!(Cli::try_parse_from(removed.clone()).is_err(), "{removed:?}");
        }

        let rename = Cli::try_parse_from([
            "lf",
            "session",
            "rename",
            "run_123",
            "Release",
            "notes",
            "--suggest",
        ])
        .expect("parse Session rename");
        assert!(matches!(
            rename.command,
            Some(Commands::Session {
                cmd: SessionCommand::Rename { id, name, suggest: true, json: false }
            }) if id == "run_123" && name == ["Release", "notes"]
        ));
        assert!(Cli::try_parse_from(["lf", "session", "rename", "run_123"]).is_err());
    }

    #[test]
    fn retired_loop_and_stack_surfaces_are_not_first_class_commands() {
        let loop_cli = Cli::try_parse_from(["lf", "loop", "infrastructure"])
            .expect("unknown names remain eligible for skill discovery");
        assert!(matches!(loop_cli.command, Some(Commands::External(_))));
        assert!(Cli::try_parse_from(["lf", "wt", "create", "child", "--child"]).is_err());
        assert!(Cli::try_parse_from(["lf", "wt", "up"]).is_err());
        assert!(Cli::try_parse_from(["lf", "wt", "down"]).is_err());
        assert!(Cli::try_parse_from(["lf", "pr", "stack"]).is_err());
    }

    #[test]
    fn sync_manual_recovery_modes_are_explicit_and_exclusive() {
        let manual = Cli::try_parse_from(["lf", "sync", "--manual", "origin/main"])
            .expect("parse manual sync");
        assert!(matches!(
            manual.command,
            Some(Commands::Sync(SyncArgs {
                manual: true,
                continue_sync: false,
                abort: false,
                onto: Some(ref onto),
                ..
            })) if onto == "origin/main"
        ));

        assert!(Cli::try_parse_from(["lf", "sync", "--continue", "--abort"]).is_err());
        assert!(Cli::try_parse_from(["lf", "sync", "--plan", "--manual"]).is_err());
    }

    #[test]
    fn pm_init_accepts_all_flag() {
        let cli = Cli::try_parse_from(["lf", "wave", "plan", "connect", "--all"]).expect("parse");
        let Some(Commands::Wave {
            cmd:
                WaveCommand::Plan {
                    cmd:
                        Some(WavePlanCommand::Connect {
                            wave,
                            all,
                            team_key,
                            team_name,
                            ..
                        }),
                    ..
                },
        }) = cli.command
        else {
            panic!("expected pm init command");
        };

        assert_eq!(wave, None);
        assert!(all);
        assert_eq!(team_key, None);
        assert_eq!(team_name, None);
    }

    #[test]
    fn pm_init_accepts_team_key_and_name() {
        let cli = Cli::try_parse_from([
            "lf",
            "wave",
            "plan",
            "connect",
            "product",
            "--team-key",
            "PRD",
            "--team-name",
            "Product",
        ])
        .expect("parse");
        let Some(Commands::Wave {
            cmd:
                WaveCommand::Plan {
                    cmd:
                        Some(WavePlanCommand::Connect {
                            team_key,
                            team_name,
                            ..
                        }),
                    ..
                },
        }) = cli.command
        else {
            panic!("expected pm init command");
        };

        assert_eq!(team_key.as_deref(), Some("PRD"));
        assert_eq!(team_name.as_deref(), Some("Product"));
    }

    #[test]
    fn pm_reteam_defaults_to_dry_run() {
        let cli = Cli::try_parse_from(["lf", "wave", "plan", "reteam"]).expect("parse");
        let Some(Commands::Wave {
            cmd:
                WaveCommand::Plan {
                    cmd: Some(WavePlanCommand::Reteam { apply, .. }),
                    ..
                },
        }) = cli.command
        else {
            panic!("expected pm reteam command");
        };
        assert!(!apply);

        let cli =
            Cli::try_parse_from(["lf", "wave", "plan", "reteam", "--apply"]).expect("parse apply");
        let Some(Commands::Wave {
            cmd:
                WaveCommand::Plan {
                    cmd: Some(WavePlanCommand::Reteam { apply, .. }),
                    ..
                },
        }) = cli.command
        else {
            panic!("expected pm reteam command");
        };
        assert!(apply);
    }

    #[test]
    fn radio_is_not_a_first_class_command() {
        assert!(Cli::command().find_subcommand("radio").is_none());
        assert!(matches!(
            Cli::try_parse_from(["lf", "radio", "pub", "status"])
                .expect("unknown names remain eligible for skill discovery")
                .command,
            Some(Commands::External(parts)) if parts[0] == "radio"
        ));
    }

    #[test]
    fn evidence_receipt_command_is_absent() {
        assert!(Cli::command().find_subcommand("receipt").is_none());
        let cli = Cli::try_parse_from(["lf", "receipt", "show", "chat_turn:turn-3"])
            .expect("unknown names remain eligible for skill discovery");
        assert!(matches!(cli.command, Some(Commands::External(_))));
    }
}

#[derive(Debug, Subcommand)]
pub enum DiscordCommand {
    /// Poll a configured channel and post each Session's final answer
    Serve { wave: String },
}
