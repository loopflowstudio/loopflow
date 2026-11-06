/// Explicit Work declaration inherited by descendants; checkout inference never writes it.
pub const WORK_DECLARATION_ENV: &str = "LF_AS";

/// The caller's prompt and provider flags for the Task Graph it launches.
#[doc(hidden)]
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

pub mod commands;
pub mod navigation;
pub mod output;

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
    pub resolved_invocation: Option<crate::skills::invocation::SkillInvocation>,

    /// Prefer this managed provider login before the normal route. Repeat to
    /// select provider-qualified preferences such as `claude=jack@`.
    /// Logins spend; a profile is only the Chrome venue accounts log in
    /// through, so it is never a run-time selector.
    #[arg(
        id = "preferred_provider_account",
        long = "account",
        value_name = "SELECTOR",
        conflicts_with = "restricted_provider_account"
    )]
    pub account: Vec<String>,

    /// Restrict this invocation and its children to exactly these managed
    /// provider logins. Providers without a selection are unavailable.
    #[arg(
        id = "restricted_provider_account",
        long = "only-account",
        value_name = "SELECTOR",
        conflicts_with = "preferred_provider_account"
    )]
    pub only_account: Vec<String>,

    /// Run in the selected account's own provider home, unmoved by account
    /// switches. Applies to this invocation and its children.
    #[arg(long, conflicts_with = "shared")]
    pub isolate: bool,

    /// Run in the provider's ordinary home despite an `isolate: true` default
    #[arg(long, conflicts_with = "isolate")]
    pub shared: bool,

    /// Skip permission prompts
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
    #[arg(long, value_name = "ID")]
    pub repository: Option<crate::durable::RepositoryId>,

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

    /// The conversation `lf -b resume ID --message TEXT` continues.
    #[arg(skip)]
    pub resume: Option<String>,
}

impl Cli {
    pub fn setting_values(&self) -> anyhow::Result<serde_yaml_ng::Value> {
        let mut value = serde_yaml_ng::Value::Mapping(Default::default());
        for setting in &self.settings {
            let (key, text) = setting
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("--setting expects KEY=YAML"))?;
            let parsed = crate::config::settings::parse_value(key, text)?;
            crate::config::settings::insert(
                &mut value,
                &key.split('.').collect::<Vec<_>>(),
                parsed,
            );
        }
        for (key, enabled) in [
            ("yolo", self.yolo.then_some(true)),
            ("context.paste", self.clipboard.then_some(true)),
            (
                "isolate",
                self.shared
                    .then_some(false)
                    .or(self.isolate.then_some(true)),
            ),
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
                crate::config::settings::insert(
                    &mut value,
                    &key.split('.').collect::<Vec<_>>(),
                    enabled.into(),
                );
            }
        }
        if let Some(agent) = &self.agent {
            crate::config::settings::insert(&mut value, &["agent"], agent.clone().into());
        }
        if !self.docs.is_empty() {
            crate::config::settings::insert(
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
            Some(Commands::Ops {
                cmd: OpsCommand::Pr {
                    cmd: Some(PrCommand::Land { .. } | PrCommand::Arm { .. })
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
            Some(Commands::Graph { cmd }) => matches!(cmd, GraphCommand::External(_)),
            Some(Commands::History {
                cmd: Some(commands::history::HistoryCommand::Replay { .. }),
                ..
            }) => true,
            Some(Commands::Task { cmd }) => matches!(cmd, TaskCommand::Run { .. }),
            Some(Commands::Session { cmd }) => matches!(
                cmd,
                SessionCommand::Resume {
                    connection: SessionConnectionArgs { prepare: false, .. },
                    ..
                } | SessionCommand::New {
                    connection: SessionConnectionArgs { prepare: false, .. },
                    ..
                }
            ),
            Some(Commands::Ops { cmd }) => match cmd {
                OpsCommand::Commit { message, .. } => message.is_none(),
                OpsCommand::Sync(args) => !args.plan && !args.abort && !args.manual,
                OpsCommand::Pr { cmd } => matches!(
                    cmd,
                    Some(
                        PrCommand::Open { .. }
                            | PrCommand::Publish { .. }
                            | PrCommand::Submit { .. }
                            | PrCommand::Arm { .. }
                            | PrCommand::Land { .. }
                    )
                ),
                OpsCommand::Release { cmd } => matches!(
                    cmd,
                    ReleaseCommand::Run { .. }
                        | ReleaseCommand::Bump { .. }
                        | ReleaseCommand::Notes { .. }
                ),
                _ => false,
            },
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
        if let Some(Commands::Session { cmd }) = &self.command {
            let selected = match cmd {
                SessionCommand::Resume { id, connection, .. }
                | SessionCommand::New { id, connection } => Some((id.as_deref(), connection)),
                _ => None,
            };
            if let Some((id, connection)) = selected {
                connection.target(&self, id).map_err(|error| {
                    clap::Error::raw(clap::error::ErrorKind::ArgumentConflict, error.to_string())
                })?;
            }
        }
        // `lf ship ISSUE` selects its Task as `--task` does.
        if let Some(Commands::Ship { issue, .. }) = &mut self.command {
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
        if let Some(Commands::Ops {
            cmd:
                OpsCommand::Account {
                    cmd,
                    provider,
                    cached,
                    details,
                    json,
                },
        }) = &self.command
        {
            if let Some(misuse) =
                AccountCommand::misuse(cmd.as_ref(), *provider, *cached || *details || *json)
            {
                return Err(clap::Error::raw(
                    clap::error::ErrorKind::ArgumentConflict,
                    format!("{misuse}\n"),
                ));
            }
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
            ("--isolate", self.isolate),
            ("--shared", self.shared),
            ("--verbose", self.verbose),
        ] {
            if enabled {
                args.push(flag.to_string());
            }
        }
        for (flag, values) in [
            ("--setting", &self.settings),
            ("--docs", &self.docs),
            ("--account", &self.account),
            ("--only-account", &self.only_account),
        ] {
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
            account: self.account.clone(),
            only_account: self.only_account.clone(),
            isolate: self.isolate,
            shared: self.shared,
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
    /// Ship the Task: run every remaining step to merge and completion, standing in at each interactive step
    ///
    /// Returns once the Task is shipping and runs on to its end (`-b` waits for
    /// it); `lf task status` follows it. A second `lf ship` while one runs exits 0.
    /// With no ISSUE, ships the checkout's Task, or a new one for its branch.
    /// `--stop` withdraws the approval and takes back its merge request; the
    /// stop stands, so no run ships the Task again until `lf ship`.
    Ship {
        issue: Option<String>,
        /// Take up this graph in place of the Task's remaining steps
        #[arg(long, conflicts_with = "stop")]
        graph: Option<String>,
        /// Why, kept in the Task's history
        #[arg(long)]
        reason: Option<String>,
        /// Stop Auto-Ship until the next `lf ship`: end the ship process and disarm auto-merge it armed
        #[arg(long)]
        stop: bool,
    },
    /// Inspect and continue Sessions
    Session {
        #[command(subcommand)]
        cmd: SessionCommand,
    },
    /// Open Work in Desktop, this Rex/Herdr/cmux/tmux host or a terminal
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
    /// Read what Sessions did, tagged with their Work: turns, commands and provider usage
    #[command(args_conflicts_with_subcommands = true)]
    History {
        #[command(flatten)]
        feed: commands::history::HistoryFeed,
        #[command(subcommand)]
        cmd: Option<commands::history::HistoryCommand>,
    },
    /// Select Work and control Loopflow Desktop panes
    #[command(
        after_help = "Targets: root --task/--wave set the base view; -p names a part of it (shell:2, a tile 3 or LOO-519, LOO-519/shell:2); --window picks the window. Each part resolves by full ID, case-insensitive label, 1-based position, then ID suffix. Without them a command acts on the terminal it runs in. Scripts should pass the IDs receipts return; positions are for people.\n\nExit status: 0 success, 1 failure, 2 bad arguments, expression or verb arguments, 3 missing or ambiguous Task, window, pane or tab (nothing changed), 4 Desktop unreachable (not running, not macOS, or not answering its socket), 130 interrupted."
    )]
    Desktop {
        #[command(subcommand)]
        cmd: DesktopCommand,
    },
    /// Show waiting, blocked, active, and finished work with next actions
    #[command(args_conflicts_with_subcommands = true)]
    Monitor {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        all: bool,
        #[command(subcommand)]
        cmd: Option<commands::monitor::MonitorCommand>,
    },
    /// Software delivery and the environment work runs in: PRs, CI, releases,
    /// commits, worktrees, Machines and provider accounts
    Ops {
        #[command(subcommand)]
        cmd: OpsCommand,
    },
    /// Set your own value of a setting for this repository, a Wave or every
    /// repository; it never edits committed config
    #[command(after_help = SETTING_KEYS)]
    Set {
        key: String,
        value: String,
        /// Every repository
        #[arg(long)]
        global: bool,
        /// Edit the committed repository or Wave configuration explicitly
        #[arg(long, id = "set_shared", conflicts_with = "global")]
        shared: bool,
    },
    /// Clear your own value of a setting, so the committed config chooses
    #[command(after_help = SETTING_KEYS)]
    Unset {
        key: String,
        /// Every repository
        #[arg(long)]
        global: bool,
        /// Edit the committed repository or Wave configuration explicitly
        #[arg(long, id = "unset_shared", conflicts_with = "global")]
        shared: bool,
    },
    /// Read Loopflow configuration
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
    /// Show memory and scratch size targets, their sources, and current usage
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
    /// Select repositories for the Machine-local portfolio
    Portfolio {
        #[command(subcommand)]
        cmd: Option<crate::portfolio::PortfolioCommand>,
        #[arg(long, global = true, id = "portfolio_json")]
        json: bool,
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
    /// steers at tool and prompt boundaries, Linear acquisition at turn end,
    /// Pi's native lifecycle events and its bash tool groups.
    // Running providers retain this older Stop spelling across lf upgrades.
    #[command(name = "__context-block", alias = "__planning-check", hide = true)]
    ContextBlock {
        #[arg(long)]
        delivery: Option<std::path::PathBuf>,
        #[arg(long, value_enum, default_value = "stop")]
        moment: crate::context_block::ContextMoment,
        /// The FIFO Pi holds open while a `tool-group` command runs
        #[arg(
            long,
            required_if_eq("moment", "tool-group"),
            requires = "provider_pid"
        )]
        lifeline: Option<std::path::PathBuf>,
        #[arg(long, requires = "lifeline")]
        provider_pid: Option<u32>,
    },
    /// Watch a Codex conversation beside its native client. Started by the
    /// launcher, which sends the launch on stdin.
    #[command(name = "__codex-observe", hide = true)]
    CodexObserve,
    /// External: skill/graph name (when no subcommand matches)
    #[command(external_subcommand)]
    External(Vec<String>),
}

/// Software delivery and the environment work runs in.
#[derive(Subcommand, Debug)]
pub enum OpsCommand {
    /// Pull request lifecycle
    Pr {
        #[command(subcommand)]
        cmd: Option<PrCommand>,
    },
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
    /// Commit changes
    Commit {
        #[arg(short = 'm', long = "message")]
        message: Option<String>,
        /// Push the branch after committing
        #[arg(short = 'p', long = "push")]
        push: bool,
        #[arg(long = "no-add")]
        no_add: bool,
        /// Commit only these paths, preserving other staged and unstaged edits
        #[arg(value_name = "PATH", conflicts_with_all = ["no_add", "push"])]
        paths: Vec<String>,
    },
    /// Merge upstream into the current branch (default: main or stack parent)
    Sync(SyncArgs),
    /// Worktree operations
    Wt {
        #[command(subcommand)]
        cmd: WtCommand,
    },
    /// Name, connect, pool and rent Machines
    Machine {
        #[command(subcommand)]
        cmd: MachineCommand,
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
    /// Refresh account access and capacity, or manage logins and routing
    Account {
        #[command(subcommand)]
        cmd: Option<AccountCommand>,
        /// Limit observations to one provider
        provider: Option<crate::provider_auth::Provider>,
        /// Inspect cached evidence without contacting providers
        #[arg(long)]
        cached: bool,
        /// Include credential sources, browser choices, and timestamps
        #[arg(long)]
        details: bool,
        /// Emit the account overview as one JSON document
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum DesktopCommand {
    /// Show a Work selection, or derive a passive plan for external hosts
    View {
        /// wave/task/query/kind/state expression; omitted uses the repository default
        expression: Option<String>,
        /// Print the derived plan as JSON without opening Desktop or starting a provider
        #[arg(long)]
        plan: bool,
        /// Print the opening receipt as JSON
        #[arg(long)]
        json: bool,
    },
    /// Print the repository's default and saved Dashboard plans as JSON
    Plans {
        #[arg(long, required = true)]
        json: bool,
        /// Print a JSON line now and each time the plans change, until stdin closes
        #[arg(long)]
        watch: bool,
    },
    /// Print the plan as Desktop shows it: each Task's section, condition and controls
    Plan {
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
        #[arg(long)]
        watch: bool,
    },
    /// Print one page of closed Tasks as Desktop shows them, most recently closed first
    Done {
        #[arg(long)]
        wave: Option<String>,
        /// Only Tasks closed since then: 7d, 24h, 30m or RFC3339
        #[arg(long)]
        since: Option<String>,
        /// Previous page's `next` Task; keep the same filters
        #[arg(long)]
        after: Option<String>,
        /// Closed Tasks per page
        #[arg(long, default_value_t = commands::waves::CLOSED_TASK_PAGE)]
        limit: usize,
        #[arg(long, required = true)]
        json: bool,
    },
    /// Print one Wave's Projects and Tasks as Desktop shows them
    Wave {
        wave: Option<String>,
        #[arg(long, required = true)]
        json: bool,
    },
    /// Print Sessions as Desktop's Session list shows them
    Sessions {
        #[command(flatten)]
        list: SessionListArgs,
    },
    /// Print one Task as Desktop's Task pane shows it
    Task {
        issue: String,
        #[arg(long, required = true)]
        json: bool,
        /// Print a JSON line now and each time the Task changes, until stdin closes
        #[arg(long)]
        watch: bool,
    },
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
    /// Rename a Dashboard or Task through its shared owner; prints the owner's record as JSON
    Rename {
        /// query:ID or task:<durable-id>
        target: String,
        /// The new name, written to the shared owner so labels and crumbs follow
        name: String,
        /// The target's last read revision; a stale one writes nothing
        #[arg(long)]
        revision: u64,
    },
    /// Read a Work's pane intent or a Dashboard's slots as JSON, or save a complete revision-checked record from stdin
    Layout {
        /// repository, wave:<durable-id>, task:<durable-id> or query:ID
        work: String,
        /// Replace the record from JSON on stdin instead of reading it (a Dashboard's: slots, null for the default)
        #[arg(long)]
        save: bool,
        /// A Dashboard's last read revision; a stale one writes nothing
        #[arg(long, requires = "save")]
        revision: Option<i64>,
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
        #[arg(long = "for", value_name = "DURATION", value_parser = crate::config::parse_duration)]
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
(`board`, `session`, `tiles` on a Wave view), which must accept the action:\n  \
lf desktop do page -p tiles page=2\n  lf desktop do filter -p board days=14"
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
    /// Show the Task's Files in the browser
    Files {
        #[command(flatten)]
        target: DesktopTargetArgs,
    },
    /// Show the Task's Graph processes in the browser
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
            Self::View { .. }
                | Self::Plans { .. }
                | Self::Plan { .. }
                | Self::Done { .. }
                | Self::Wave { .. }
                | Self::Sessions { .. }
                | Self::Task { .. }
                | Self::Layout { save: false, .. }
        )
    }
}

// ── Which commands change state ─────────────────────────────────────────────
// Only a command that changes state records a Process. A read (status, list,
// show, diff, file, every `--watch` reader) records none, so it never wakes a
// Work reader or joins a Task's work. Every match below is exhaustive: a new
// command chooses here.

impl Commands {
    /// Whether running this command changes state, and so records a Process.
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Task { cmd } => cmd.changes_work(),
            Self::Wave { cmd } => cmd.changes_work(),
            Self::Session { cmd } => cmd.changes_work(),
            Self::Graph { cmd } => cmd.changes_work(),
            Self::Skill { cmd } => cmd.changes_work(),
            Self::History { cmd, .. } => cmd.as_ref().is_some_and(|cmd| cmd.changes_work()),
            Self::Desktop { cmd } => cmd.changes_work(),
            Self::Monitor { cmd, .. } => cmd.as_ref().is_some_and(|cmd| cmd.changes_work()),
            Self::Ops { cmd } => cmd.changes_work(),
            Self::Config { cmd } => cmd.changes_work(),
            Self::Self_ { cmd } => cmd.changes_work(),
            Self::Portfolio { cmd, .. } => cmd.is_some(),
            Self::List { .. } | Self::Help { .. } | Self::Context { .. } | Self::CodexObserve => {
                false
            }
            Self::Ship { .. }
            | Self::Open(_)
            | Self::Inline { .. }
            | Self::Set { .. }
            | Self::Unset { .. }
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
            Self::List { .. }
            | Self::Location { .. }
            | Self::WatchFiles { .. }
            | Self::Status { .. }
            | Self::View { .. }
            | Self::Tool { .. }
            | Self::Diff { .. }
            | Self::Files { .. }
            | Self::File { .. }
            | Self::Wait { .. } => false,
            Self::Shell { list, .. } => !list,
            Self::Retire { .. } => true,
            Self::Comment { message, .. } => message.is_some(),
            Self::Sweep { apply, .. } => *apply,
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
            | Self::Edit { .. }
            | Self::Move { .. }
            | Self::Interrupt { .. } => true,
        }
    }
}

impl WaveCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Show { .. } | Self::Graph { .. } | Self::List { .. } => false,
            Self::Status { sync, .. } => *sync,
            Self::NewChapter { dry_run, .. } => !dry_run,
            Self::Remove { preview, .. } => !preview,
            Self::EditPlan { .. }
            | Self::Add { .. }
            | Self::BindProject { .. }
            | Self::Rename { .. }
            | Self::UpdatePlan { .. }
            | Self::Connect { .. }
            | Self::Reteam { .. } => true,
        }
    }
}

impl SessionCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::ObserveStatus { .. }
            | Self::History { .. }
            | Self::List { .. }
            | Self::Remote { .. } => false,
            Self::Steers { retry, .. } => retry.is_some(),
            Self::Bind { dry_run, .. } => !dry_run,
            Self::Serve { .. }
            | Self::Resume { .. }
            | Self::New { .. }
            | Self::Steer { .. }
            | Self::Rename { .. }
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
            Self::View { .. }
            | Self::Plans { .. }
            | Self::Plan { .. }
            | Self::Done { .. }
            | Self::Wave { .. }
            | Self::Sessions { .. }
            | Self::Task { .. }
            | Self::Ls { .. }
            | Self::State { .. }
            | Self::Actions { .. }
            | Self::Events { .. }
            | Self::Wait { .. }
            | Self::Capture { .. }
            // Navigation, panes and Dashboard arrangement are not Work; their
            // saves still persist, without a Process.
            | Self::Layout { .. }
            | Self::Query { .. }
            | Self::Pin { .. }
            | Self::Unpin { .. }
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
            Self::Rename { target, .. } => target.starts_with("task:"),
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

impl OpsCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Pr { cmd } => pr_changes_work(cmd.as_ref()),
            Self::Release { cmd } => cmd.changes_work(),
            Self::Wt { cmd } => cmd.changes_work(),
            Self::Machine { cmd } => cmd.changes_work(),
            Self::Account { cmd, cached, .. } => account_changes_work(cmd.as_ref(), *cached),
            Self::Ci { .. } | Self::People { .. } | Self::Tokens { .. } => false,
            Self::Commit { .. } | Self::Sync(_) => true,
        }
    }
}

/// Bare `lf pr` acts on the current branch's PR.
fn pr_changes_work(cmd: Option<&PrCommand>) -> bool {
    match cmd {
        None => true,
        Some(PrCommand::Checks { .. } | PrCommand::Reviews { .. }) => false,
        Some(
            PrCommand::Reconcile
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

/// Bare `lf account` refreshes access and capacity unless it reads the cache.
fn account_changes_work(cmd: Option<&AccountCommand>, cached: bool) -> bool {
    match cmd {
        None => !cached,
        Some(AccountCommand::Route { .. }) => false,
        Some(
            AccountCommand::Disconnect { .. }
            | AccountCommand::Connect { .. }
            | AccountCommand::Set { .. }
            | AccountCommand::Token { .. }
            | AccountCommand::RedeemReset { .. }
            | AccountCommand::Use { .. },
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

impl MachineCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Id { .. } | Self::List { .. } | Self::Status { .. } => false,
            Self::Credentials { cmd } => match cmd {
                MachineCredentialCommand::Inspect { .. } => false,
                MachineCredentialCommand::Receive => true,
            },
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
            Self::Connect { .. }
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
            Self::User { .. } | Self::Show { .. } | Self::Catalog { .. } => false,
            Self::Import { .. } => true,
            Self::Migrate { accept } => *accept,
            Self::Retained { replace, keep } => *replace || *keep,
            Self::Repository { bind, relocate, .. } => bind.is_some() || relocate.is_some(),
            Self::Planning { cmd } => match cmd {
                PlanningCommand::Key { new, recover } => *new || recover.is_some(),
                PlanningCommand::Status { .. } | PlanningCommand::Journal { .. } => false,
            },
        }
    }
}

impl SelfCommand {
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Doctor { .. } => false,
            Self::Install { cmd, .. } => match cmd {
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
/// (`board`, `session`, `tiles` on a Wave view), a pane (`session`, `shell`,
/// `browser`, `shell:2`), a Dashboard or Wave view tile (`3`, `LOO-519`), or `TILE/PANE`. `query:ID/TILE` and `working-tasks/TILE` name a
/// saved record's tile by Work key without Desktop. Root `--task`/`--wave` set the
/// base view; without either, a command acts on the terminal it runs in (`LF_TERMINAL_ID`).
#[derive(Args, Debug, Clone, Default)]
pub struct DesktopTargetArgs {
    /// Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix
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
    /// Print the repository file that defines a skill, creating
    /// it from the builtin when the repository has none
    Customize { name: String },
    #[command(external_subcommand)]
    External(Vec<String>),
}

/// Select a Wave's current chapter, or an exact retained historical Project.
#[derive(Args, Debug)]
pub struct WavePlanTarget {
    /// Wave name or ID (default: the ambient Wave)
    #[arg(conflicts_with = "project")]
    pub wave: Option<String>,
    /// Exact retained Project ID, provider ID or unique name instead of current selection
    #[arg(long)]
    pub project: Option<String>,
}

impl WavePlanTarget {
    pub fn as_plan_target(&self) -> crate::ops::project::PlanTarget<'_> {
        match self.project.as_deref() {
            Some(project) => crate::ops::project::PlanTarget::Project(project),
            None => crate::ops::project::PlanTarget::Wave(self.wave.as_deref()),
        }
    }
}

const SETTING_KEYS: &str = "`lf config catalog` lists every preference and where it applies.
Values use YAML; false disables and [] clears an additive list.
The default destination is root lf.config.local.yaml, shared across Task worktrees.
--global edits ~/.lf/config.yaml; --shared explicitly edits committed configuration.
`lf --wave <name> set` selects a Wave; unset removes only that declaration.
`lf config show --skill <name> --json` explains effective values and sources.";

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
    /// Inspect an authored graph, or one that ran
    Show {
        name: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        processes: bool,
    },
    /// Print the repository file that defines a Graph, creating
    /// it from the builtin when the repository has none
    Customize { name: String },
    /// Carry a Task's Graph from where its `lf task run` set out; that run starts it
    #[command(hide = true)]
    Run,
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum SessionMode {
    #[value(name = "true")]
    Interactive,
    #[value(name = "false")]
    Headless,
    All,
}

impl SessionMode {
    pub fn interactive(self) -> Option<bool> {
        match self {
            Self::Interactive => Some(true),
            Self::Headless => Some(false),
            Self::All => None,
        }
    }
}

/// Shared selection, preparation and next-turn options for `resume` and `session new`.
#[derive(Args, Debug)]
pub struct SessionConnectionArgs {
    #[arg(long, conflicts_with = "wave")]
    pub task: Option<String>,
    #[arg(long)]
    pub wave: Option<String>,
    /// With `-b`: send the conversation its next turn
    #[arg(long)]
    pub message: Option<String>,
    /// Return connection data without starting the provider
    #[arg(long, conflicts_with = "message")]
    pub prepare: bool,
    #[arg(long)]
    pub json: bool,
}

impl SessionConnectionArgs {
    pub fn target<'a>(
        &'a self,
        cli: &'a Cli,
        id: Option<&'a str>,
    ) -> anyhow::Result<crate::ops::human_session::SessionTarget<'a>> {
        anyhow::ensure!(
            self.task.is_none() || cli.task.is_none(),
            "select a Task only once"
        );
        anyhow::ensure!(
            self.wave.is_none() || cli.wave.is_none(),
            "select a Wave only once"
        );
        use crate::ops::human_session::SessionTarget;
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

/// Which Sessions `lf session list` and `lf desktop sessions` read.
#[derive(Args, Debug, Clone)]
pub struct SessionListArgs {
    #[arg(long)]
    pub json: bool,
    /// Include Sessions from every repository on this machine
    #[arg(long)]
    pub all: bool,
    /// Select interactive (true), headless (false), or both (all)
    #[arg(long, value_enum, default_value = "true")]
    pub interactive: SessionMode,
    /// Include completed conversations and historical reviews
    #[arg(long)]
    pub history: bool,
    /// Only conversations available for your input
    #[arg(long)]
    pub available: bool,
    /// Maximum conversations; 0 reads the complete matching inventory
    #[arg(long, default_value_t = 100)]
    pub limit: usize,
    #[arg(long, default_value_t = 0)]
    pub offset: usize,
    /// Return a bounded stable-ID page with a continuation cursor
    #[arg(long, requires = "json", conflicts_with = "offset")]
    pub page: bool,
    /// Previous page's next identity; keep the same filters
    #[arg(long, requires = "page")]
    pub after: Option<String>,
    #[arg(long)]
    pub task: Option<String>,
    /// Only Sessions without a Task association
    #[arg(long, conflicts_with = "task")]
    pub orphan: bool,
    #[arg(long)]
    pub search: Option<String>,
    /// Print a JSON line now and each time the list changes, until stdin closes
    #[arg(long)]
    pub watch: bool,
}

#[derive(Subcommand, Debug)]
pub enum SessionCommand {
    /// Carry a viewer through SSH with end-to-end liveness.
    #[command(hide = true)]
    Remote {
        #[arg(long)]
        serve: bool,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        argv: Vec<String>,
    },
    /// Run the process a Session lives in while terminals come and go
    Serve {
        /// Relay the execution to an attached terminal
        #[arg(long)]
        terminal: bool,
        /// The command this process executes
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        argv: Vec<String>,
    },
    /// Save a steer for this Session, or without one for the current Wave's or
    /// repository's Sessions, without starting a turn
    #[command(allow_missing_positional = true)]
    Steer { id: Option<String>, message: String },
    /// Inspect steer delivery or retry delivery
    Steers {
        id: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        retry: Option<String>,
        #[arg(long, default_value_t = 0)]
        after: i64,
        #[arg(long, default_value_t = 32)]
        limit: usize,
        /// Read the complete steer
        #[arg(long)]
        source: Option<String>,
    },
    /// Observe validated Program Status snapshots from an active local terminal
    ObserveStatus {
        id: String,
        /// The terminal marker in the current provider client receipt
        #[arg(long)]
        terminal: String,
        /// AgentProcess from the Session reading; omit when it reported none
        #[arg(long)]
        agent_process: Option<String>,
    },
    /// Read this conversation's native start, usage and completion receipts
    History {
        /// Session ID or provider conversation ID
        id: String,
        #[arg(long)]
        json: bool,
        /// Continue after an observed event sequence
        #[arg(long, default_value_t = 0)]
        after: i64,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// List Sessions
    List(SessionListArgs),
    /// Get back to a conversation: this scope's, a Task's, or an exact one
    #[command(
        after_help = "Reconnects to the running provider or continues its saved thread after safe replacement: a terminal showing it hands it over and says where it went; a dead provider resumes from its history. A live turn, draft or pending request keeps its provider; if attachment fails, resume reports the failure, leaves that work intact and resends no input. With nothing to resume, it starts one.\n\nOther harness:\n  lf resume -a codex                       continue on Codex, seeded with what the last thread knew\nMoves:\n  lf resume --account <email> -a claude    keep this conversation on another login\n  lf --machine <machine> resume <session>  move only this conversation to another Machine\n  lf task move <task> --machine <machine>  move a Task with its Workflow and Session"
    )]
    Resume {
        /// Session ID, Task identifier, or provider conversation ID
        #[arg(conflicts_with_all = ["task", "wave"])]
        id: Option<String>,
        #[command(flatten)]
        connection: SessionConnectionArgs,
        /// Leave the other client running; continue in a new conversation forked from it
        #[arg(long, conflicts_with = "message")]
        fork: bool,
    },
    /// Restart this scope's conversation on a fresh provider thread, seeded with
    /// what the last thread knew; `-a` switches harness
    #[command(
        after_help = "The conversation keeps its name, Work and history; the old thread stays in `lf session history`.\n  lf session new -a claude      continue this scope's conversation on Claude"
    )]
    New {
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
        after_help = "Select an unfinished interactive Session by its Loopflow ID or recorded provider ID. Binding is permanent; repeating it selects the same conversation again. The Task keeps its existing worktree and binding immediately sets the Session directory to it. Already-running clients may keep their original directory. Files and live input stay intact. An open Task without a worktree adopts the Session's unowned linked checkout; main and Wave checkouts stay shared. Done Tasks remain done; without a checkout, their conversation stays in place. Closed Sessions and headless Runs cannot become primary. Remote calls use the target Machine's Sessions and paths."
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
        after_help = "Everyday spelling: `lf --machine <machine> resume <session>` sends, receives and resumes in one step. `lf task move <task> --machine <machine>` moves a Task with its Session. Run send and receive by hand to retry a move whose receive failed:\n  lf session send <session|--task <task>> --to <machine> | lf --machine <machine> session receive"
    )]
    Send {
        /// Session ID or provider conversation ID
        #[arg(required_unless_present = "task", conflicts_with = "task")]
        id: Option<String>,
        /// Send this Task with its Session, as `lf task move --machine` does
        #[arg(long)]
        task: Option<String>,
        /// The Machine it moves to, by label or id
        #[arg(long)]
        to: String,
    },
    /// Admit a Session `lf session send` printed, read from stdin: check out its
    /// branch and place its conversation so it resumes here
    Receive {
        /// Read no move; exit non-zero unless this Machine can receive one
        #[arg(long)]
        check: bool,
        /// With --check, also require that this Machine knows the Task
        #[arg(long, requires = "check")]
        task: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum WaveCommand {
    /// Read chapter plans and Tasks without starting work; an explicit Wave includes history
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
        #[arg(long, requires = "json")]
        watch: bool,
    },
    /// Show the graph the Wave's Tasks take up by default
    Graph {
        #[command(subcommand)]
        cmd: WaveGraphCommand,
    },
    /// Edit the selected chapter's name or summary
    EditPlan {
        #[command(flatten)]
        target: WavePlanTarget,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        summary: Option<String>,
        /// Authored chapter prose
        #[arg(long)]
        description: Option<String>,
    },

    /// Rotate this Wave, or every Wave with --all, to the named Project plan
    #[command(allow_missing_positional = true)]
    NewChapter {
        #[arg(required_unless_present = "all", conflicts_with = "all")]
        wave: Option<String>,
        name: String,
        /// Advance every Wave in this repository
        #[arg(long)]
        all: bool,
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Add a Wave: register it, seed and commit .lf/wave/<name>/ on the main checkout,
    /// and start its first chapter. Adding an existing Wave changes nothing.
    Add {
        wave: String,
        /// Objective text for the new Wave's GOAL.md
        #[arg(long)]
        objective: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Remove a Wave and its child Waves. Open Tasks move to the parent Wave, or the
    /// repository; chapters, completed Tasks and Sessions stay readable in history.
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
    /// Bind an existing Project UUID as this Wave's shared current selection
    BindProject {
        wave: String,
        project: String,
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
    /// Show one Wave's current plan, Task details, and execution evidence.
    Status {
        /// Wave name (default: the ambient wave)
        wave: Option<String>,
        /// Emit the status snapshot as JSON
        #[arg(long)]
        json: bool,
        /// Refresh planning from its provider (Git or Linear) before reading
        #[arg(long, conflicts_with = "watch")]
        sync: bool,
        /// Print a JSON line now and each time the status changes, until stdin closes
        #[arg(long, requires = "json")]
        watch: bool,
    },
    /// Rename a Wave, moving .lf/wave/<name>/ on the main checkout; its Tasks,
    /// chapters and Sessions keep their identity
    Rename {
        wave: String,
        /// The new name; omit to keep it while changing --repo or --title
        new_name: Option<String>,
        /// Move the Wave to another repository
        #[arg(long)]
        repo: Option<PathBuf>,
        /// Change the linked Initiative display title
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Replace the current chapter's KRs and targets
    UpdatePlan {
        #[arg(short = 'w', long)]
        wave: Option<String>,
        /// The complete plan as JSON
        #[arg(long)]
        plan: std::path::PathBuf,
    },
    /// Connect a Wave to its Initiative and the repository's Team (Task prefix); a new name is added first
    Connect {
        /// Wave name (auto-detected if omitted)
        wave: Option<String>,
        /// Recursively initialize every Wave under .lf/wave/
        #[arg(long, conflicts_with = "wave")]
        all: bool,
        /// Repository Team key = Task prefix (e.g. LOO). Defaults from the repository name.
        #[arg(long = "team-key")]
        team_key: Option<String>,
        /// Repository Team display name. Defaults to the repository name.
        #[arg(long = "team-name")]
        team_name: Option<String>,
    },
    /// Reconcile linked Waves to the repository's Linear Team
    Reteam {
        #[arg(long)]
        apply: bool,
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
    /// Branch to sync onto; naming the default branch detaches a stacked Task
    pub onto: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum TaskCommand {
    /// List the open Tasks in the selected repository
    List {
        /// Only Tasks whose title or identifier contains this text
        #[arg(conflicts_with = "done")]
        query: Option<String>,
        #[arg(long)]
        wave: Option<String>,
        /// Project ID, provider ID, slug or name
        #[arg(long, conflicts_with = "done")]
        project: Option<String>,
        /// Only Tasks in this state
        #[arg(long, conflicts_with = "done", value_parser = ["unstarted", "idle", "active", "moving", "shipping"])]
        state: Option<String>,
        #[arg(long)]
        json: bool,
        /// Page closed (done or abandoned) Tasks instead, most recently closed first
        #[arg(long)]
        done: bool,
        /// Only Tasks closed since then: 7d, 24h, 30m or RFC3339
        #[arg(long, requires = "done")]
        since: Option<String>,
        /// Previous page's `next` Task; keep the same filters
        #[arg(long, requires = "done")]
        after: Option<String>,
        /// Closed Tasks per page
        #[arg(long, requires = "done", default_value_t = 50)]
        limit: usize,
        /// Print a JSON line now and each time the list changes, until stdin closes
        #[arg(long, conflicts_with = "done")]
        watch: bool,
    },

    /// Synchronize a Task with its tracker; the change sent last wins
    Sync {
        issue: String,
        /// Retry an uncertain issue creation with its saved ID; may recreate a deleted issue
        #[arg(long)]
        retry_creation: bool,
        /// Retry uncertain saved field/state updates after observing current Linear values
        #[arg(long)]
        retry_updates: bool,
    },
    /// Ensure tracked Task Work and its worktree without launching a Graph
    Checkout {
        issue: Option<String>,
        #[arg(long)]
        name: Option<String>,
        /// Fork this Task's worktree from another Task's active PR
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
    /// returns once it runs (`-b` runs every step headless to `end`, approving Auto-Ship)
    #[command(
        after_help = "A Task runs the graph it took up: its Wave's default, its repository's \
`graph:`, or `code`. A run stops before each interactive step, where the Task waits for its \
conversation; the next `lf task run` continues after it. `-i` runs every step with you in this \
terminal instead. A run that reaches `end`, as `-b` does, approves Auto-Ship like `lf ship`, \
which may merge the PR, unless someone stopped shipping; then it waits at the landing step \
until `lf ship`."
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
        /// Fork this Task's worktree from another Task's active PR
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
    },
    /// Complete a Task without moving its Graph
    Complete {
        issue: String,
        /// Why, kept in the Task's completion request
        #[arg(long)]
        reason: Option<String>,
    },
    /// Reopen local planning without moving its Graph or replacing the PR
    Reopen {
        issue: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Create a Task; with --run, also start its Graph
    #[command(
        after_help = "Plain creation files new work, regardless of the current checkout or invoking Session. --run starts the created Task; it does not select work to adopt. Pass --session SESSION or --worktree PATH to adopt existing work, with or without --run. Find Loopflow Session IDs with `lf session list --json`; an agent keeping its chat passes its own ID explicitly. Explicit-source retries return the same Task. Main and Wave checkouts stay shared. Adoption preserves files, branch, history and live attachment; creation alone starts no Graph and approves no Auto-Ship. Remote calls use sources on the selected Machine."
    )]
    Create {
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
    /// Unassign the Task: nobody owns it and Linear shows no assignee; nothing
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
    /// Open a shell in an existing Task checkout, without preparing work
    Shell {
        issue: String,
        #[arg(long)]
        checkout: std::path::PathBuf,
        /// Stable Desktop shell identity; attachment never creates execution
        #[arg(long, required_unless_present = "list")]
        id: Option<String>,
        /// List retained shell identities, including ended shells
        #[arg(long, conflicts_with_all = ["id", "create", "close", "command"])]
        list: bool,
        /// Create once, then attach; retrying the same identity never replays it
        #[arg(long, conflicts_with = "close")]
        create: bool,
        /// End this shell and its ordinary jobs
        #[arg(long)]
        close: bool,
        /// Initial command, followed by the destination login shell
        #[arg(last = true)]
        command: Vec<String>,
    },
    /// Run the Task's recorded workspace retirement
    #[command(hide = true)]
    Retire { issue: String },
    /// Stream invalidations from an existing Task checkout (JSON lines)
    WatchFiles {
        issue: String,
        #[arg(long)]
        checkout: std::path::PathBuf,
        #[arg(long)]
        request: String,
    },
    /// Show durable Task facts and its recorded work
    Status {
        /// Task issue; defaults to the Task in this checkout
        issue: Option<String>,
        #[arg(long)]
        json: bool,
        /// Print a JSON line now and each time the status changes, until stdin closes
        #[arg(long)]
        watch: bool,
    },
    /// Render a workspace tool in the Task's recorded checkout
    Tool {
        issue: String,
        #[arg(value_enum)]
        tool: crate::lf::commands::task_tool::Tool,
        #[arg(long)]
        checkout: std::path::PathBuf,
        #[arg(long)]
        path: Option<std::path::PathBuf>,
        #[arg(long, default_value = "parent")]
        base: String,
        /// Compare to this resolved commit instead of the working tree
        #[arg(long)]
        head: Option<String>,
        /// Delegate comparison display to the configured Git difftool
        #[arg(long)]
        difftool: bool,
    },
    /// Browse the Task brief, workflow and recorded runs without starting work
    View {
        issue: Option<String>,
        /// Print once, including when stdout is a terminal
        #[arg(long)]
        once: bool,
    },
    /// Show this Task's patch or list its changed files
    Diff {
        issue: String,
        path: Option<String>,
        /// List changed paths and comparison revisions instead of a patch
        #[arg(long, conflicts_with_all = ["path", "draft"])]
        files: bool,
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
    /// Cancel the Task in Linear and locally, close its PRs and delete its branches
    Abandon {
        /// Issue ID or branch; defaults to the Task in this checkout
        issue: Option<String>,
        #[arg(short = 'f', long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Preview open issues outside current chapters; apply safe cancellations explicitly
    Sweep {
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Cancel unfinished placed work, clean up delivery, then trash the Linear issue
    Delete { issue: String },
    /// Edit a Task's title or notes, before or after placement
    Edit {
        issue: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        /// Zero-based position in the Project's Task order
        #[arg(long)]
        rank: Option<u32>,
        /// Assignee identity; provider user ID in connected plans
        #[arg(long)]
        assignee: Option<String>,
        #[arg(short = 'w', long)]
        wave: Option<String>,
    },
    /// Change Task membership, or move the Task to another Machine, preserving its recorded work
    #[command(
        after_help = "`--machine` stops the Task's Graph, commits and pushes its worktree, and hands it off with its Workflow position and Session; `lf task run` there runs its route again. A worktree mid-merge or mid-rebase refuses to move.\n  lf task move LOO-12 --machine mini"
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
    /// Read the thread or publish a comment; agent comments default to progress
    Comment {
        issue: String,
        message: Option<String>,
        /// Save a steer even when commenting from an agent Session
        #[arg(long, requires = "message")]
        steer: bool,
        #[arg(short = 'w', long)]
        wave: Option<String>,
        #[arg(long)]
        json: bool,
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
            Self::List { .. } | Self::Create { .. } | Self::Sweep { .. } => None,
            Self::Status { issue, .. }
            | Self::View { issue, .. }
            | Self::Abandon { issue, .. }
            | Self::Run { issue, .. }
            | Self::Checkout { issue, .. } => issue.as_deref(),
            Self::Retire { issue }
            | Self::Shell { issue, .. }
            | Self::Tool { issue, .. }
