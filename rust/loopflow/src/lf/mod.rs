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

    /// Override agents named in Tasks, Graphs and skills, including child commands (claude, codex, opencode, pi; harness[:model])
    #[arg(short = 'a', long = "agent", global = true)]
    pub agent: Option<String>,

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

    /// The conversation `lf -b connect ID --message TEXT` continues.
    #[arg(skip)]
    pub resume: Option<String>,
}

impl Cli {
    /// The global `--repo` or `--repository` selector names a repository.
    pub fn selects_repository(&self) -> bool {
        self.repo.is_some() || self.repository.is_some()
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
                SessionCommand::Connect {
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
        if let Some(Commands::Session {
            cmd: SessionCommand::Connect { id, connection, .. },
        }) = &self.command
        {
            connection.target(&self, id.as_deref()).map_err(|error| {
                clap::Error::raw(clap::error::ErrorKind::ArgumentConflict, error.to_string())
            })?;
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
            cmd: SessionCommand::Connect { connection, .. },
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

    pub fn chrome_setting(&self) -> Option<bool> {
        self.chrome.map(|mode| mode == BrowserMode::On).or_else(|| {
            std::env::var(crate::config::CHROME_OVERRIDE_ENV)
                .ok()
                .map(|value| value == "on")
        })
    }

    pub fn diff_files_setting(&self) -> Option<bool> {
        self.diff
            .map(|mode| matches!(mode, DiffContext::Files | DiffContext::Both))
    }

    pub fn diff_setting(&self) -> Option<bool> {
        self.diff
            .map(|mode| matches!(mode, DiffContext::Patch | DiffContext::Both))
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
    /// `--stop` withdraws the approval and takes back its merge request.
    Ship {
        issue: Option<String>,
        /// Take up this graph in place of the Task's remaining steps
        #[arg(long, conflicts_with = "stop")]
        graph: Option<String>,
        /// Why, kept in the Task's history
        #[arg(long)]
        reason: Option<String>,
        /// Stop Auto-Ship: end the ship process and disarm auto-merge it armed
        #[arg(long)]
        stop: bool,
    },
    /// Inspect and continue Sessions
    Session {
        #[command(subcommand)]
        cmd: SessionCommand,
    },
    /// Open Work's conversation in Desktop, this cmux/Herdr host or a terminal
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
        /// Apply on this Machine only; set by the copy to your other Machines
        #[arg(long, hide = true)]
        here: bool,
    },
    /// Clear your own value of a setting, so the committed config chooses
    #[command(after_help = SETTING_KEYS)]
    Unset {
        key: String,
        /// Every repository
        #[arg(long)]
        global: bool,
        /// Apply on this Machine only; set by the copy to your other Machines
        #[arg(long, hide = true)]
        here: bool,
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
    /// Create and use an offline, isolated development data clone (macOS)
    Clone {
        #[command(subcommand)]
        cmd: crate::data_clone::CloneCommand,
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
    #[command(name = "__context-block", hide = true)]
    ContextBlock {
        #[arg(long)]
        delivery: Option<std::path::PathBuf>,
        #[arg(long, value_enum)]
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
    /// Release operations (run, check, notes, bump, tag, status)
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
        #[arg(long, default_value_t = 50)]
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

const SETTING_KEYS: &str = "Keys:
  graph                 the graph new Tasks take up
  agent                 the agent work launches on, such as claude:opus
  skill_agents.<skill>  the agent one skill launches on
  machine               where new Tasks and the Session run; copied to your other
                        Machines. A pool or on-demand sends new Tasks there.
  machine.tasks         where new Tasks run
  machine.session       where the Session runs; it moves there
  account.<provider>    the logins that pay, in order: work@,personal@

Scope: this repository; `lf --wave <wave> set` a Wave; --global every repository.
Yours wins over the committed .lf/config.yaml and .lf/wave/<name>/config.yaml,
for you only; `lf wave status` shows each value and its source.";

#[derive(Subcommand, Debug)]
pub enum WaveGraphCommand {
    /// Show the graph the Wave's Tasks take up by default; select one with
    /// `graph:` in `.lf/wave/<name>/config.yaml`
    Show {
        wave: Option<String>,
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

/// Shared selection, preparation and next-turn options for resume and connect.
#[derive(Args, Debug)]
pub struct SessionConnectionArgs {
    #[arg(long, conflicts_with = "wave")]
    pub task: Option<String>,
    #[arg(long)]
    pub wave: Option<String>,
    /// Create a fresh primary in this scope, keeping the previous client and history
    #[arg(long, conflicts_with = "id")]
    pub fresh: bool,
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
    /// Run the process a Session lives in while terminals come and go
    Serve {
        /// Relay the execution to an attached terminal
        #[arg(long)]
        terminal: bool,
        /// The command this process executes
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        argv: Vec<String>,
    },
    /// Attach to the current execution without restarting or submitting input
    Attach {
        id: String,
        #[arg(long)]
        observe: bool,
        /// Refuse to replace another viewer's input control
        #[arg(long, conflicts_with = "observe")]
        if_free: bool,
        /// Return an attach-only terminal command without acquiring control
        #[arg(long)]
        prepare: bool,
        #[arg(long, requires = "prepare")]
        json: bool,
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
    /// Open the ongoing conversation of this scope, or an exact Session
    #[command(
        visible_alias = "resume",
        after_help = "Moves:\n  lf resume --account <email> -a claude    keep this conversation on another login\n  lf --machine <machine> resume <session>  move only this conversation to another Machine\n  lf task move <task> --machine <machine>  move a Task with its Workflow and Session"
    )]
    Connect {
        /// Session ID, Task identifier, or provider conversation ID
        #[arg(conflicts_with_all = ["task", "wave"])]
        id: Option<String>,
        #[command(flatten)]
        connection: SessionConnectionArgs,
        /// Stop Loopflow-owned clients before resuming here
        #[arg(long, conflicts_with = "try_open")]
        takeover: bool,
        /// Ask the provider to resume even when another client is active
        #[arg(long = "try", conflicts_with = "takeover")]
        try_open: bool,
        /// Resume every conversation a restart or crash interrupted; never one
        /// someone left. Terminals attach when they open them.
        #[arg(long, conflicts_with_all = ["id", "takeover", "try_open", "fresh", "message", "prepare"])]
        all: bool,
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
    /// Assign a Task to a Session that has none; the Task never changes after
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
    /// Ensure the configured Project is active, or create one with a durable identity
    Ensure {
        wave: String,
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
    /// List authored Waves and retained planning identities without starting work.
    List {
        /// Emit the wave snapshot as JSON (Loopflow's dashboard snapshot)
        #[arg(long)]
        json: bool,
        /// List Waves from every repository on this machine, not just the
        /// current repository (worktrees collapse to their main checkout).
        #[arg(long)]
        all: bool,
        /// Exclude abandoned and retired registrations from current navigation.
        #[arg(long)]
        current: bool,
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
    /// Rename or relocate an authored Wave and its provider mapping
    Rename {
        wave: String,
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long)]
        name: Option<String>,
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
    /// Connect a Wave to its Initiative and the repository's Team (Task prefix)
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
    /// Branch to sync onto
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
        #[arg(long, conflicts_with = "done", value_parser = ["deferred", "unstarted", "idle", "active", "moving", "shipping"])]
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

    /// Defer future Task execution without stopping live processes
    Defer { issue: String },
    /// Resume a deferred Task without starting execution
    Resume { issue: String },
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
    /// returns once it runs (`-b` runs every step headless and waits for the end)
    #[command(
        after_help = "A Task runs the graph it took up: its Wave's default, its repository's \
`graph:`, or `code`. A run stops before each interactive step, where the Task waits for its \
conversation; the next `lf task run` continues after it. `-i` runs every step with you in this \
terminal instead."
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
    /// Create a planning Task without allocating a checkout or starting work
    Create {
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
        #[arg(long)]
        json: bool,
    },
    /// Claim the Task for you on this Machine
    Claim {
        issue: String,
        #[arg(long)]
        json: bool,
    },
    /// Unclaim the Task; it is unowned again, with its history intact
    Unclaim {
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
    },
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
    /// Show this Task's patch or list its changed files
    Diff {
        issue: String,
        path: Option<String>,
        /// List changed paths and comparison revisions instead of a patch
        #[arg(long, conflicts_with_all = ["path", "draft"])]
        files: bool,
        #[arg(long, default_value = "parent")]
        base: String,
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
        #[arg(long, conflicts_with = "unassign")]
        assignee: Option<String>,
        #[arg(long)]
        unassign: bool,
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
            | Self::Abandon { issue, .. }
            | Self::Run { issue, .. }
            | Self::Checkout { issue, .. } => issue.as_deref(),
            Self::Shell { issue, .. }
            | Self::WatchFiles { issue, .. }
            | Self::Location { issue, .. }
            | Self::Claim { issue, .. }
            | Self::Unclaim { issue, .. }
            | Self::Sync { issue, .. }
            | Self::Move { issue, .. }
            | Self::Complete { issue, .. }
            | Self::Defer { issue }
            | Self::Resume { issue }
            | Self::Reopen { issue, .. }
            | Self::Diff { issue, .. }
            | Self::Files { issue, .. }
            | Self::File { issue, .. }
            | Self::Save { issue, .. }
            | Self::Delete { issue }
            | Self::Edit { issue, .. }
            | Self::Comment { issue, .. }
            | Self::Interrupt { issue, .. }
            | Self::Wait { issue, .. } => Some(issue),
        }
    }

    pub fn selector_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::List { .. } | Self::Create { .. } | Self::Sweep { .. } => None,
            Self::Status { issue, .. }
            | Self::Abandon { issue, .. }
            | Self::Run { issue, .. }
            | Self::Checkout { issue, .. } => issue.as_mut(),
            Self::Shell { issue, .. }
            | Self::WatchFiles { issue, .. }
            | Self::Location { issue, .. }
            | Self::Claim { issue, .. }
            | Self::Unclaim { issue, .. }
            | Self::Sync { issue, .. }
            | Self::Move { issue, .. }
            | Self::Complete { issue, .. }
            | Self::Defer { issue }
            | Self::Resume { issue }
            | Self::Reopen { issue, .. }
            | Self::Diff { issue, .. }
            | Self::Files { issue, .. }
            | Self::File { issue, .. }
            | Self::Save { issue, .. }
            | Self::Delete { issue }
            | Self::Edit { issue, .. }
            | Self::Comment { issue, .. }
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
    /// Install the latest Loopflow at login and weekly by default (macOS launchd)
    Schedule {
        /// Weekly: Monday 09:00; daily: 09:00; otherwise on clock boundaries (local time)
        #[arg(value_enum, default_value = "weekly")]
        frequency: InstallFrequency,
    },
    /// Continue one interrupted installation switch from its pinned candidate.
    #[command(hide = true)]
    RecoverSwitch {
        /// The fixed installation switch receipt to continue.
        #[arg(long)]
        switch: String,
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
    /// Repoint the global CLI at retained prior bytes only after that binary's
    /// own preflight proves it recognizes the current store frontier.
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
    Reconcile,
    /// Show CI status for current branch
    Checks {
        #[arg(short = 'w', long = "watch")]
        watch: bool,
        #[arg(short = 'l', long = "logs")]
        logs: bool,
        /// Show only checks required by repository protection.
        #[arg(long)]
        required: bool,
    },

    /// Request or re-request a teammate's review of an existing PR.
    RequestReview {
        /// PR number, URL or branch; defaults to the current branch's PR.
        pr: Option<String>,
        /// GitHub login or org/team. Repeat to request several reviewers.
        #[arg(short = 'r', long, required = true)]
        reviewer: Vec<String>,
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
    },
    /// Push and create or update a draft PR, then open its GitHub page.
    /// Existing ready PRs stay ready; opening a draft does not publish it.
    Open {
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "body")]
        body: Option<String>,
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
    },
    /// Prepare a PR, request exact-head auto-merge, and return without watching.
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
    },
    /// Abandon branch: close PR, remove worktree, delete branch
    Abandon {
        /// Branch to abandon (default: current)
        branch: Option<String>,
        #[arg(short = 'f', long)]
        force: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum PlanningCommand {
    /// Show, create, or recover the planning user key that names a `personal: true` ref
    Key {
        #[arg(long, conflicts_with = "recover")]
        new: bool,
        #[arg(long, value_name = "UUID")]
        recover: Option<String>,
    },
    /// Show selection, retained imports and held records without contacting Git
    Status {
        #[arg(long)]
        json: bool,
        /// Keep the repository's planning exchanged, printing a JSON line now and each time the status changes, until stdin closes
        #[arg(long, requires = "json")]
        watch: bool,
    },
    /// Show each destination's retained changes, including losing values, without contacting Git
    Journal {
        #[arg(long)]
        json: bool,
    },
}

/// Manage the installed artifacts and exported skills.
#[derive(Debug, Subcommand)]
pub enum SelfCommand {
    /// Diagnose installation, storage and Process integrity
    Doctor {
        /// Diagnose repository planning without changing it
        #[arg(long)]
        planning: bool,
        /// Emit the audit as JSON
        #[arg(long)]
        json: bool,
    },
    /// Install the latest published Loopflow release from any directory
    Install {
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
    /// Print the configured participant display name.
    User {
        #[arg(long)]
        json: bool,
    },
    /// Establish or print this repository's selected planning identity
    Repository {
        /// Associate this checkout with a repository identity, retaining prior IDs
        #[arg(long)]
        bind: Option<crate::durable::RepositoryId>,
        /// Reconnect retained Work after a move predating its Git locator
        #[arg(long, conflicts_with = "bind")]
        relocate: Option<crate::durable::RepositoryId>,
        /// Print the selected repository's current local directory
        #[arg(long, conflicts_with = "json")]
        path: bool,
        #[arg(long)]
        json: bool,
    },
    /// Inspect planning synchronization and personal identity
    Planning {
        #[command(subcommand)]
        cmd: PlanningCommand,
    },
}

/// Name and connect to machines.
#[derive(Debug, Subcommand)]
pub enum MachineCommand {
    /// Install a separate login on an added machine using this laptop's browser.
    Connect {
        target: String,
        /// github, claude, codex, opencode or linear
        provider: crate::provider_auth::Provider,
        email: Option<String>,
        #[arg(long)]
        chrome_profile: Option<String>,
    },
    /// Inspect or receive a machine credential (credential bytes use stdin only).
    Credentials {
        #[command(subcommand)]
        cmd: MachineCredentialCommand,
    },
    /// Name this Machine for teammates, or `off` to make it private again.
    /// A private Machine's ownership entries carry only its id.
    Public { name: String },
    /// Print this machine's stable local Machine identity.
    Id {
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
        cost_usd: Option<crate::rental::Usd>,
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
        provider: crate::rental::Provider,
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
        /// Hard deadline for each machine it rents, at most 24
        #[arg(long, default_value_t = crate::rental::DEFAULT_HOURS)]
        hours: f64,
        /// Machines rented at once
        #[arg(long)]
        max: u32,
        /// Above this hourly price nothing is rented
        #[arg(long, value_name = "USD")]
        max_usd_per_hour: crate::rental::Usd,
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
        max_usd: Option<crate::rental::Usd>,
        #[arg(long)]
        json: bool,
    },
    /// Forget it; its rentals' history stays
    Remove { name: String },
}

#[derive(Debug, Subcommand)]
pub enum MachineCredentialCommand {
    /// Report whether this account is installed, without logging in.
    Inspect {
        provider: crate::provider_auth::Provider,
        login: String,
    },
    /// Receive a fresh login as JSON on stdin; preserve existing accounts.
    Receive,
}

#[derive(Debug, Subcommand)]
pub enum AccountCommand {
    /// Disconnect local credentials or one managed login
    Disconnect {
        provider: String,
        email: Option<String>,
    },
    /// Connect local credentials or a managed login using a remembered browser
    Connect {
        provider: String,
        email: Option<String>,
        #[arg(long, conflicts_with_all = ["import", "api_key"])]
        chrome_profile: Option<String>,
        /// Adopt an existing Claude login
        #[arg(long = "import", requires = "email", conflicts_with = "api_key")]
        import: bool,
        /// Read the provider's API key environment variable
        #[arg(long, conflicts_with = "email")]
        api_key: bool,
    },
    /// Edit account configuration or remembered browser choices
    Set {
        provider: String,
        email: Option<String>,
        #[arg(long, requires = "email")]
        login_email: Option<String>,
        #[arg(long, requires = "email")]
        routing: Option<String>,
        #[arg(long, requires = "email", conflicts_with = "clear_plan")]
        plan: Option<String>,
        #[arg(long, requires = "email")]
        clear_plan: bool,
        #[arg(long, requires = "email", conflicts_with = "clear_paid_through")]
        paid_through: Option<String>,
        #[arg(long, requires = "email")]
        clear_paid_through: bool,
        #[arg(long, requires = "email")]
        clear_cooldown: bool,
        /// Replace the ordered browser choices (repeat for fallback profiles)
        #[arg(long, conflicts_with = "clear_chrome_profiles")]
        chrome_profile: Vec<String>,
        #[arg(long)]
        clear_chrome_profiles: bool,
    },
    /// Store a long-lived Claude token for rented machines: mint it with
    /// `claude setup-token` in the account's Chrome profile, or read one minted
    /// by hand from --file (`-` for stdin)
    Token {
        provider: String,
        email: String,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long, conflicts_with = "file")]
        chrome_profile: Option<String>,
    },
    /// Spend one banked Codex reset for this named login
    RedeemReset {
        provider: String,
        email: String,
        /// Reuse this key when retrying the same redemption
        #[arg(long)]
        idempotency_key: Option<String>,
        /// Opaque credit ID returned by live status (otherwise the service chooses)
        #[arg(long)]
        credit_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Sign the provider's ordinary home in as a stored login:
    /// `lf account <provider> use <email>`
    #[command(override_usage = "lf account <PROVIDER> use <EMAIL>")]
    Use { email: String },
    /// Explain configured and automatic account selection; `lf set
    /// account.<provider>` replaces a route
    Route {
        #[arg(long, conflicts_with = "default")]
        repo: Option<String>,
        #[arg(long)]
        default: bool,
        #[arg(long)]
        json: bool,
    },
}

impl AccountCommand {
    /// `use` is the one verb that takes its provider first; a provider or an
    /// overview flag before any other verb belongs to bare `lf account`.
    pub fn misuse(
        cmd: Option<&Self>,
        provider: Option<crate::provider_auth::Provider>,
        overview_flags: bool,
    ) -> Option<&'static str> {
        match (cmd, provider) {
            (Some(Self::Use { .. }), None) => {
                Some("name the provider: lf account <provider> use <email>")
            }
            (Some(Self::Use { .. }), Some(_)) | (None, _) => None,
            (Some(_), None) if !overview_flags => None,
            (Some(_), _) => Some(
                "the provider and --cached, --details and --json before a subcommand apply to `lf account` alone",
            ),
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum ReleaseCommand {
    /// Run the full release workflow end-to-end
    Run {
        /// Version to release: patch|minor|major|X.Y.Z (default: patch)
        version: Option<String>,
        #[arg(short = 't', long = "target")]
        target: Option<String>,
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
    Prune {
        /// Show what would be pruned without removing anything
        #[arg(long)]
        dry_run: bool,
        /// Reclaim old generated output from completed Tasks; retain checkouts
        #[arg(long)]
        builds: bool,
        /// Delete origin and local branches tied to merged or closed PRs,
        /// finished Tasks, or published release candidates
        #[arg(long, conflicts_with = "builds")]
        branches: bool,
        /// Emit cleanup entries and allocated-byte receipts
        #[arg(long, requires = "builds")]
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
    fn retired_surface_flags_are_rejected_for_a_valid_skill_command() {
        for flag in ["--ide", "--tui"] {
            let error = Cli::try_parse_from(["lf", flag, "skill", "debug"]).unwrap_err();
            assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
            assert!(error.to_string().contains(flag));
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
            ["lf", "ops", "wt", "create", "csv-export"]
                .map(String::from)
                .to_vec(),
        )
        .unwrap();
        assert_eq!(literal, ["lf", "ops", "wt", "create", "csv-export"]);
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
        assert!(command.find_subcommand("repo").is_none());
        assert!(
            Cli::try_parse_from(["lf", "wave", "new-chapter", "October", "--plan", "p.json"])
                .is_err()
        );
        assert!(
            Cli::try_parse_from(["lf", "session", "steer", "session_1", "m", "--repo"]).is_err()
        );
        assert!(Cli::try_parse_from(["lf", "wave", "serve", "product"]).is_err());
        assert!(Cli::try_parse_from(["lf", "wave", "place", "product", "mini"]).is_err());
        assert!(Cli::try_parse_from(["lf", "task", "admission", "LOO-1"]).is_err());
        assert!(Cli::try_parse_from(["lf", "wave", "machine", "product", "mini"]).is_err());
        assert!(Cli::try_parse_from(["lf", "task", "handoff", "LOO-1", "mini"]).is_err());
        for args in [
            vec!["lf", "list"],
            vec!["lf", "wave", "list", "--json"],
            vec!["lf", "ops", "pr", "checks"],
            vec![
                "lf",
                "wave",
                "new-chapter",
                "--all",
                "October",
                "--plan",
                "p.json",
            ],
            vec![
                "lf",
                "wave",
                "new-chapter",
                "product",
                "October",
                "--plan",
                "p.json",
            ],
            vec!["lf", "session", "steer", "keep attribution"],
            vec![
                "lf",
                "--repo",
                "loopflow",
                "session",
                "steer",
                "keep attribution",
            ],
            vec!["lf", "session", "steer", "session_1", "keep attribution"],
            vec!["lf", "wave", "rename", "product", "--title", "Product"],
            vec!["lf", "task", "move", "LOO-1", "--machine", "mini"],
            vec!["lf", "--wave", "product", "set", "machine", "mini"],
            vec!["lf", "unset", "account.claude", "--global"],
            vec!["lf", "discord", "serve", "product"],
            vec!["lf", "self", "doctor", "--planning", "--json"],
            vec!["lf", "wave", "status", "product", "--sync"],
        ] {
            assert!(Cli::try_parse_from(args.clone()).is_ok(), "{args:?}");
        }
        assert!(matches!(
            Cli::try_parse_from(["lf", "task", "defer", "LOO-1"]).unwrap().command,
            Some(Commands::Task { cmd: TaskCommand::Defer { issue } }) if issue == "LOO-1"
        ));
        assert!(matches!(
            Cli::try_parse_from(["lf", "task", "resume", "LOO-1"]).unwrap().command,
            Some(Commands::Task { cmd: TaskCommand::Resume { issue } }) if issue == "LOO-1"
        ));
        for verb in ["recover", "enable", "disable", "prepare", "advance"] {
            assert!(Cli::try_parse_from(["lf", "task", verb, "LOO-1"]).is_err());
        }
        assert!(Cli::try_parse_from(["lf", "wave", "status", "--no-sync"]).is_err());
        assert!(Cli::try_parse_from(["lf", "ops", "wt", "list", "--full"]).is_err());
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
                cmd: crate::lf::SelfCommand::Install { cmd: None }
            })
        ));
        assert!(Cli::try_parse_from(["lf", "ops", "machine", "install"]).is_err());
        assert!(Cli::try_parse_from(["lf", "self", "install", "schedule"]).is_ok());
        assert!(Cli::try_parse_from(["lf", "self", "install", "status"]).is_err());
        assert!(Cli::try_parse_from(["lf", "self", "install", "preflight"]).is_ok());
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
                    all: false,
                    current: false
                }
            })
        ));
        assert!(Cli::try_parse_from(["lf", "wave", "probe", "product", "--json"]).is_err());
        assert!(Cli::try_parse_from(["lf", "ops", "pr", "checks", "--logs"]).is_ok());
        assert!(Cli::try_parse_from(["lf", "ops", "machine", "probe", "product"]).is_err());
        assert!(Cli::try_parse_from(["lf", "ops", "wt", "ci"]).is_err());
    }

    #[test]
    fn chapter_commands_require_retained_plan_and_task_creation_needs_no_project_selector() {
        let preview = Cli::try_parse_from([
            "lf",
            "wave",
            "new-chapter",
            "two",
            "--all",
            "--plan",
            "chapter.json",
            "--dry-run",
            "--json",
        ])
        .unwrap();
        assert!(matches!(
            preview.command,
            Some(Commands::Wave {
                cmd: WaveCommand::NewChapter { dry_run: true, .. }
            })
        ));
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
            "Which KR matters?",
        ])
        .expect("parse bound inline prompt");

        assert_eq!(cli.wave.as_deref(), Some("product"));
        assert!(matches!(
            cli.command,
            Some(Commands::Inline { prompt }) if prompt == vec!["Which KR matters?"]
        ));
    }

    #[test]
    fn ci_report_accepts_machine_wide_filters() {
        let cli = Cli::try_parse_from([
            "lf",
            "ops",
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
            Some(Commands::Ops { cmd: OpsCommand::Ci {
                since,
                wave: Some(wave),
                repo: Some(repo),
                json: true,
            } }) if since == "24h" && wave == "infrastructure" && repo == "loopflowstudio/loopflow"
        ));
    }

    #[test]
    fn account_preference_and_restriction_are_distinct_repeatable_flags() {
        let preferred = Cli::try_parse_from([
            "lf",
            "--account",
            "claude=personal",
            "--account",
            "codex=reserve",
            "skill",
            "implement",
        ])
        .expect("parse account preferences");
        assert_eq!(preferred.account, vec!["claude=personal", "codex=reserve"]);
        assert!(preferred.only_account.is_empty());

        let restricted = Cli::try_parse_from([
            "lf",
            "--only-account",
            "claude=personal",
            "--only-account",
            "codex=reserve",
            "skill",
            "implement",
        ])
        .expect("parse account restrictions");
        assert_eq!(
            restricted.only_account,
            vec!["claude=personal", "codex=reserve"]
        );

        assert!(Cli::try_parse_from([
            "lf",
            "--account",
            "reserve",
            "--only-account",
            "reserve",
            "skill",
            "implement",
        ])
        .is_err());
    }

    #[test]
    fn the_provider_hook_is_the_only_hidden_command_and_the_root_hides_no_flag() {
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
    fn account_use_takes_its_provider_before_the_verb() {
        let cli = Cli::try_parse_from([
            "lf",
            "ops",
            "account",
            "codex",
            "use",
            "jack@loopflow.studio",
        ])
        .unwrap();
        assert!(
            matches!(cli.command, Some(Commands::Ops { cmd: OpsCommand::Account {
            cmd: Some(AccountCommand::Use { email }),
            provider: Some(crate::provider_auth::Provider::Codex),
            ..
        } }) if email == "jack@loopflow.studio")
        );
    }

    #[test]
    fn auth_set_accepts_ordered_service_profiles() {
        let cli = Cli::try_parse_from([
            "lf",
            "ops",
            "account",
            "set",
            "linear",
            "--chrome-profile",
            "Work",
            "--chrome-profile",
            "Personal",
        ])
        .unwrap();
        assert!(
            matches!(cli.command, Some(Commands::Ops { cmd: OpsCommand::Account {
            cmd: Some(AccountCommand::Set { email: None, chrome_profile, .. }), ..
        } }) if chrome_profile == ["Work", "Personal"])
        );
        assert!(
            Cli::try_parse_from(["lf", "ops", "account", "set", "linear", "--clear-cooldown"])
                .is_err()
        );
    }

    #[test]
    fn auth_connect_addresses_an_account_and_optional_bootstrap_venue() {
        let cli = Cli::try_parse_from([
            "lf",
            "ops",
            "account",
            "connect",
            "claude",
            "operator@",
            "--chrome-profile",
            "Profile 9",
        ])
        .expect("parse account connection");

        assert!(cli.account.is_empty());
        assert!(matches!(
            cli.command,
            Some(Commands::Ops { cmd: OpsCommand::Account {
                cmd: Some(AccountCommand::Connect {
                    provider,
                    email: Some(email),
                    chrome_profile: Some(chrome_profile),
                    ..
                }), ..
            } }) if provider == "claude"
                && email == "operator@"
                && chrome_profile == "Profile 9"
        ));
    }

    #[test]
    fn service_auth_accepts_a_remembered_chrome_profile() {
        let cli = Cli::try_parse_from([
            "lf",
            "ops",
            "account",
            "connect",
            "linear",
            "--chrome-profile",
            "Work",
        ])
        .unwrap();
        assert!(
            matches!(cli.command, Some(Commands::Ops { cmd: OpsCommand::Account { cmd: Some(AccountCommand::Connect {
            provider, email: None, chrome_profile: Some(profile), ..
        }), .. } }) if provider == "linear" && profile == "Work")
        );
    }

    #[test]
    fn auth_connect_sources_are_exclusive() {
        assert!(Cli::try_parse_from([
            "lf",
            "ops",
            "account",
            "connect",
            "claude",
            "a@example.com",
            "--import"
        ])
        .is_ok());
        assert!(
            Cli::try_parse_from(["lf", "ops", "account", "connect", "codex", "--api-key"]).is_ok()
        );
        for flags in [
            vec!["--import", "--api-key"],
            vec!["--import", "--chrome-profile", "Work"],
        ] {
            assert!(Cli::try_parse_from(
                ["lf", "ops", "account", "connect", "claude", "a@example.com"]
                    .into_iter()
                    .chain(flags)
            )
            .is_err());
        }
    }

    #[test]
    fn auth_set_accepts_provider_specific_billing_and_routing_state() {
        let cli = Cli::try_parse_from([
            "lf",
            "ops",
            "account",
            "set",
            "codex",
            "loopflow-eng@",
            "--login-email",
            "engineering@example.com",
            "--routing",
            "automatic",
            "--plan",
            "max",
            "--paid-through",
            "2026-08-14",
        ])
        .expect("parse provider account lifecycle");

        assert!(cli.account.is_empty());
        assert!(matches!(
            cli.command,
            Some(Commands::Ops { cmd: OpsCommand::Account {
                cmd: Some(AccountCommand::Set {
                    provider,
                    email: Some(email),
                    login_email: Some(login_email),
                    routing: Some(routing),
                    plan: Some(plan),
                    paid_through: Some(paid_through),
                    clear_plan: false,
                    clear_paid_through: false,
                    ..
                }), ..
            } }) if provider == "codex"
                && email == "loopflow-eng@"
                && login_email == "engineering@example.com"
                && routing == "automatic"
                && plan == "max"
                && paid_through == "2026-08-14"
        ));
    }

    #[test]
    fn pm_init_accepts_positional_wave() {
        let cli = Cli::try_parse_from(["lf", "wave", "connect", "pm"]).expect("parse");
        let Some(Commands::Wave {
            cmd: WaveCommand::Connect { wave, all, .. },
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
        assert!(Cli::try_parse_from(["lf", "task", "create", "--run"]).is_err());
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
        let land = Cli::try_parse_from(["lf", "ops", "pr", "land", "--wait-and-fix"])
            .expect("parse waited landing");
        assert!(matches!(
            land.command,
            Some(Commands::Ops {
                cmd: OpsCommand::Pr {
                    cmd: Some(PrCommand::Land {
                        wait_and_fix: true,
                        ..
                    })
                }
            })
        ));
        assert!(Cli::try_parse_from(["lf", "ops", "pr", "land", "--wait"]).is_err());
        for command in ["land", "arm", "submit"] {
            assert!(Cli::try_parse_from(["lf", "ops", "pr", command, "-c"]).is_err());
            assert!(
                Cli::try_parse_from(["lf", "ops", "pr", command, "--next", "follow-up"]).is_err()
            );
        }
        assert!(Cli::try_parse_from(["lf", "ops", "pr", "next"]).is_err());
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
            Some(Commands::Task { cmd: TaskCommand::Complete { issue, reason } })
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
    fn task_comment_reads_and_writes_without_retired_commands() {
        let steer = Cli::try_parse_from([
            "lf",
            "task",
            "comment",
            "INF-123",
            "take the smaller approach",
        ])
        .expect("parse task comment");
        let Some(Commands::Task {
            cmd:
                TaskCommand::Comment {
                    issue,
                    message,
                    json,
                    wave: _,
                    steer: _,
                },
        }) = steer.command
        else {
            panic!("expected task comment command");
        };
        assert_eq!(issue, "INF-123");
        assert_eq!(message.as_deref(), Some("take the smaller approach"));
        assert!(Cli::try_parse_from(["lf", "task", "comment", "INF-123"]).is_ok());
        assert!(matches!(
            Cli::try_parse_from([
                "lf",
                "task",
                "comment",
                "INF-123",
                "--steer",
                "keep the API"
            ])
            .unwrap()
            .command,
            Some(Commands::Task {
                cmd: TaskCommand::Comment { steer: true, .. }
            })
        ));
        assert!(
            Cli::try_parse_from(["lf", "task", "edit", "INF-123", "--notes", "revised"]).is_ok()
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

        for removed in ["steer", "acknowledge", "decide", "request-decision"] {
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
        for command in ["resume", "connect"] {
            let cli = Cli::try_parse_from(["lf", "session", command, "session-id"])
                .unwrap()
                .checked()
                .unwrap();
            assert!(matches!(
                cli.command,
                Some(Commands::Session {
                    cmd: SessionCommand::Connect { id: Some(id), .. }
                }) if id == "session-id"
            ));
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
        for argv in [
            vec!["lf", "--task", "LOO-1", "session", "connect", "session-id"],
            vec![
                "lf", "--wave", "infra", "session", "resume", "--task", "LOO-1",
            ],
        ] {
            assert!(Cli::try_parse_from(argv).and_then(Cli::checked).is_err());
        }
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

        let open = Cli::try_parse_from(["lf", "session", "connect", "run_123", "--takeover"])
            .expect("parse replacement open");
        assert!(matches!(
            open.command,
            Some(Commands::Session {
                cmd: SessionCommand::Connect {
                    id,
                    takeover: true,
                    try_open: false,
                    connection: SessionConnectionArgs { json: false, .. },
                    ..
                }
            }) if id.as_deref() == Some("run_123")
        ));
        assert!(Cli::try_parse_from([
            "lf",
            "session",
            "connect",
            "run_123",
            "--takeover",
            "--try",
        ])
        .is_err());

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
        assert!(Cli::try_parse_from(["lf", "ops", "wt", "create", "child", "--stack"]).is_err());
        assert!(Cli::try_parse_from(["lf", "ops", "wt", "create", "child", "--child"]).is_err());
        assert!(Cli::try_parse_from(["lf", "ops", "wt", "up"]).is_err());
        assert!(Cli::try_parse_from(["lf", "ops", "wt", "down"]).is_err());
        assert!(Cli::try_parse_from(["lf", "ops", "pr", "stack"]).is_err());
    }

    #[test]
    fn sync_manual_recovery_modes_are_explicit_and_exclusive() {
        let manual = Cli::try_parse_from(["lf", "ops", "sync", "--manual", "origin/main"])
            .expect("parse manual sync");
        assert!(matches!(
            manual.command,
            Some(Commands::Ops { cmd: OpsCommand::Sync(SyncArgs {
                manual: true,
                continue_sync: false,
                abort: false,
                onto: Some(ref onto),
                ..
            }) }) if onto == "origin/main"
        ));

        assert!(Cli::try_parse_from(["lf", "ops", "sync", "--continue", "--abort"]).is_err());
        assert!(Cli::try_parse_from(["lf", "ops", "sync", "--plan", "--manual"]).is_err());
    }

    #[test]
    fn pm_init_accepts_all_flag() {
        let cli = Cli::try_parse_from(["lf", "wave", "connect", "--all"]).expect("parse");
        let Some(Commands::Wave {
            cmd:
                WaveCommand::Connect {
                    wave,
                    all,
                    team_key,
                    team_name,
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
                WaveCommand::Connect {
                    team_key,
                    team_name,
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
        let cli = Cli::try_parse_from(["lf", "wave", "reteam"]).expect("parse");
        let Some(Commands::Wave {
            cmd: WaveCommand::Reteam { apply },
        }) = cli.command
        else {
            panic!("expected pm reteam command");
        };
        assert!(!apply);

        let cli = Cli::try_parse_from(["lf", "wave", "reteam", "--apply"]).expect("parse apply");
        let Some(Commands::Wave {
            cmd: WaveCommand::Reteam { apply },
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

/// Pushed code requirements for one remote Task invocation.
pub const TASK_SOURCE_ENV: &str = "LF_TASK_SOURCE";
