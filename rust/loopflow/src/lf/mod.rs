/// Prompt flags forwarded to a managed Task's ordinary skill commands.
#[doc(hidden)]
/// Explicit Work declaration inherited by descendants; checkout inference never writes it.
pub const WORK_DECLARATION_ENV: &str = "LF_AS";

pub const TASK_SKILL_OPTIONS_ENV: &str = "LF_TASK_SKILL_OPTIONS";

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

pub mod commands;
pub mod discovery;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LaunchMode {
    Interactive,
    Batch,
    Tui,
    Ide,
}

#[derive(Parser, Debug, Default)]
#[command(name = "lf", bin_name = "lf", disable_help_subcommand = true)]
#[command(about = "Open Loopflow or run its CLI")]
#[command(version = crate::build_info::BUILD_VERSION)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Docs paths, globs, or directories to include in context
    #[arg(long = "docs", value_delimiter = ',')]
    pub docs: Vec<String>,

    /// Include clipboard content in prompt
    #[arg(short = 'c', long = "clipboard")]
    pub clipboard: bool,

    /// Model to use (harness or harness:model)
    #[arg(short = 'm', long = "model")]
    pub model: Option<String>,

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

    /// Internal SSH compatibility and broker-connectivity probe.
    #[arg(long = "__account-lease-probe", hide = true)]
    pub account_lease_probe: bool,

    /// Skip permission prompts
    #[arg(long)]
    pub yolo: bool,

    /// Choose the provider surface; omission inherits configuration and terminal context
    #[arg(long, value_enum)]
    pub mode: Option<LaunchMode>,

    /// Override Chrome integration; omission inherits configuration
    #[arg(long, value_enum)]
    pub chrome: Option<BrowserMode>,

    /// Exact cron receipt attribution for mechanical release execution
    #[arg(long = "__cron-receipt", hide = true, requires = "cron_lock_fd")]
    pub cron_receipt: Option<String>,

    #[arg(long = "__cron-lock-fd", hide = true, requires = "cron_receipt")]
    pub cron_lock_fd: Option<i32>,

    /// Select changed-code context; omission inherits configuration
    #[arg(long, value_enum)]
    pub diff: Option<DiffContext>,

    /// Maximum agent turns for this invocation
    #[arg(long = "max-turns")]
    pub max_turns: Option<u32>,

    /// Add Wave context and identity without changing the working directory
    #[arg(long, value_name = "WAVE")]
    pub wave: Option<String>,

    /// Execute in this Task's checkout
    #[arg(long, value_name = "TASK", conflicts_with = "wt")]
    pub task: Option<String>,

    /// Execute in an existing worktree by name or branch
    #[arg(long, value_name = "NAME", conflicts_with = "task")]
    pub wt: Option<String>,

    /// Keep a Work-bound internal launch in this exact checkout.
    #[arg(long = "__cwd", value_name = "PATH", hide = true)]
    pub bound_cwd: Option<PathBuf>,

    /// Exclude loopflow operating guidance
    #[arg(long = "no-loopflow")]
    pub no_loopflow: bool,

    /// Execute a skill from this saved Flow boundary, without resolving its definition again.
    #[arg(long = "__flow-step", hide = true)]
    pub flow_step: Option<String>,
}

impl Cli {
    /// Reject argument combinations the derive cannot express, as the usage
    /// errors they are, before anything runs.
    pub fn checked(self) -> Result<Self, clap::Error> {
        if let Some(Commands::Account {
            cmd,
            provider,
            cached,
            details,
            json,
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
        let mut args = vec!["--mode".to_string(), "batch".to_string()];
        for (flag, enabled) in [
            ("--clipboard", self.clipboard),
            ("--yolo", self.yolo),
            ("--no-loopflow", self.no_loopflow),
            ("--isolate", self.isolate),
            ("--shared", self.shared),
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
        if let Some(model) = &self.model {
            args.extend(["--model".to_string(), model.clone()]);
        }
        if let Some(turns) = self.max_turns {
            args.extend(["--max-turns".to_string(), turns.to_string()]);
        }
        args
    }

    pub(crate) fn exec_options(&self) -> Self {
        Self {
            cron_receipt: self.cron_receipt.clone(),
            cron_lock_fd: self.cron_lock_fd,
            command: None,
            docs: self.docs.clone(),
            clipboard: self.clipboard,
            model: self.model.clone(),
            account: self.account.clone(),
            only_account: self.only_account.clone(),
            isolate: self.isolate,
            shared: self.shared,
            account_lease_probe: self.account_lease_probe,
            yolo: self.yolo,
            mode: self.mode,
            chrome: self.chrome,
            diff: self.diff,
            max_turns: self.max_turns,
            wave: self.wave.clone(),
            task: self.task.clone(),
            wt: self.wt.clone(),
            bound_cwd: self.bound_cwd.clone(),
            no_loopflow: self.no_loopflow,
            flow_step: self.flow_step.clone(),
        }
    }

    pub fn chrome_setting(&self) -> Option<bool> {
        self.chrome.map(|mode| mode == BrowserMode::On)
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

#[derive(Args, Debug, Clone)]
pub struct ScreenshotArgs {
    /// URL or local HTML file to capture
    pub source: String,

    /// PNG destination
    #[arg(short = 'o', long = "output")]
    pub output: PathBuf,

    /// Viewport width in pixels
    #[arg(long, default_value_t = 1440)]
    pub width: u32,

    /// Viewport height in pixels
    #[arg(long, default_value_t = 900)]
    pub height: u32,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Pull request lifecycle
    Pr {
        #[command(subcommand)]
        cmd: Option<PrCommand>,
    },
    /// Worktree operations
    Wt {
        #[command(subcommand)]
        cmd: WtCommand,
    },
    /// Merge upstream into the current branch (default: main or stack parent)
    Sync(SyncArgs),
    /// Commit changes
    Commit {
        #[arg(short = 'm', long = "message")]
        message: Option<String>,
        #[arg(long = "no-add")]
        no_add: bool,
        /// Commit only these paths, preserving other staged and unstaged edits
        #[arg(value_name = "PATH", conflicts_with = "no_add")]
        paths: Vec<String>,
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
    /// Run an inline prompt
    #[command(name = ":")]
    Inline {
        #[arg(trailing_var_arg = true)]
        prompt: Vec<String>,
    },
    /// Internal owner-loss supervisor for one browser capture.
    #[command(name = "__screenshot-supervisor", hide = true)]
    ScreenshotSupervisor {
        #[command(flatten)]
        screenshot: ScreenshotArgs,
    },
    /// Internal provider callback that records one native interactive session.
    #[command(name = "__provider-session", hide = true)]
    ProviderSession,
    /// Inspect and continue Sessions
    Session {
        #[command(subcommand)]
        cmd: SessionCommand,
    },
    /// Refresh account access and capacity, or manage logins and routing
    Account {
        #[command(subcommand)]
        cmd: Option<AccountCommand>,
        /// Limit observations to one provider
        provider: Option<crate::provider_auth::Provider>,
        /// Inspect cached evidence without contacting providers or the origin broker
        #[arg(long)]
        cached: bool,
        /// Include credential sources, browser choices, and timestamps
        #[arg(long)]
        details: bool,
        /// Emit the account overview as one JSON document
        #[arg(long)]
        json: bool,
    },
    /// Repository releases, source measurement, CI evidence, and provider administration
    Repo {
        #[command(subcommand)]
        cmd: RepoCommand,
    },
    /// Inspect this Home and observe routes to other Homes
    Home {
        #[command(subcommand)]
        cmd: HomeCommand,
    },
    /// Bridge new Discord messages to finite Wave Sessions
    Discord {
        #[command(subcommand)]
        cmd: DiscordCommand,
    },
    /// Manage Wave identity, placement and planning
    Wave {
        #[command(subcommand)]
        cmd: WaveCommand,
    },
    /// Concrete work and Task lifecycle
    Task {
        #[command(subcommand)]
        cmd: TaskCommand,
    },
    /// Show effective context budgets, their sources, and current source usage
    Context {
        #[arg(long)]
        json: bool,
        /// Inspect a Wave's local authored context
        #[arg(long, conflicts_with = "task")]
        wave: Option<String>,
        /// Inspect a Task's checkout and locally stored goal
        #[arg(long)]
        task: Option<String>,
        /// Skill to include in the launch preview
        #[arg(long, default_value = "realign")]
        skill: String,
    },
    /// Internal: render the repository maintainer scorecard for telemetry-daily
    #[command(name = "__telemetry-scorecard", hide = true)]
    TelemetryScorecard {
        /// Emit structured JSON for operator automation
        #[arg(long)]
        json: bool,
    },
    /// Discover commands, skills, and flows
    List {
        path: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Explain a command, skill, or flow without launching it
    Help {
        path: Vec<String>,
        #[arg(long)]
        all: bool,
    },
    /// Show the current repository's roadmap: every open Task across the repo's
    /// Waves, joined to live evidence and bucketed into Now / Waiting /
    /// Available / Later. `--wave` scopes it; `--all` spans every repository on
    /// this machine. Local-only, deterministic.
    Roadmap {
        /// Scope to one Wave (default: every Wave in the current repository)
        #[arg(long)]
        wave: Option<String>,
        /// Find an exact issue identifier, including retained historical Tasks.
        #[arg(long)]
        task: Option<String>,
        /// Emit the roadmap snapshot as JSON
        #[arg(long)]
        json: bool,
        /// Span every repository on this machine, not just the current one.
        #[arg(long)]
        all: bool,
    },
    /// Launch the immutable provider request retained for a captured input.
    Replay {
        /// Captured input identity or an unambiguous displayed prefix
        run: String,
    },
    /// Execute one captured Flow boundary in its own process.
    #[command(name = "__flow-step", hide = true)]
    FlowStep { id: String, version: u64 },
    /// Run a definition, preferring a flow over a same-named skill
    Run {
        name: String,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Run or inspect authored flows
    Flow {
        #[command(subcommand)]
        cmd: FlowCommand,
    },
    /// Run a skill explicitly
    Skill {
        #[command(subcommand)]
        cmd: SkillCommand,
    },
    /// External: skill/flow name (when no subcommand matches)
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand, Debug)]
pub enum SkillCommand {
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand, Debug)]
pub enum FlowCommand {
    /// Start or continue a Task through its saved Flow
    Start {
        /// Template for a new Task Flow; existing saved progress remains authoritative
        template: Option<String>,
        #[arg(long)]
        name: Option<String>,
        /// Fork this Task's worktree from another Task's active PR
        #[arg(long = "stack-on", value_name = "PARENT_TASK")]
        stack_on: Option<String>,
        #[arg(long)]
        directive: Option<String>,
        /// Explain what changed after an execution blocker
        #[arg(long)]
        reason: Option<String>,
        /// Retry uncertain native work after confirmed engine exit.
        #[arg(long)]
        retry: bool,
        #[arg(long)]
        json: bool,
    },

    /// List authored flows or saved FlowSessions
    List {
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        inventory: commands::flow_inventory::FlowInventoryArgs,
    },
    /// Inspect an authored flow or a saved FlowSession
    Show {
        name: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        sessions: bool,
    },
    /// Continue a saved Flow invocation
    Resume {
        invocation: String,
        #[arg(long)]
        retry: bool,
    },
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

#[derive(Subcommand, Debug)]
pub enum SessionCommand {
    /// Resume a conversation by ID, or the last interactive Session in this worktree
    Resume {
        /// Loopflow Session ID or Claude/Codex conversation ID
        id: Option<String>,
    },
    /// Read this conversation's native start, usage and completion receipts
    History {
        /// Session ID, one of its Run IDs, or the provider's own conversation ID
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
    List {
        #[arg(long)]
        json: bool,
        /// Include waiting steps from every repository on this machine
        #[arg(long)]
        all: bool,
        /// Select interactive (true), headless (false), or both (all)
        #[arg(long, value_enum, default_value = "true")]
        interactive: SessionMode,
        /// Include completed conversations and historical reviews
        #[arg(long)]
        history: bool,
        /// Only conversations waiting for review or a reply
        #[arg(long)]
        needs_me: bool,
        /// Maximum conversations; 0 reads the complete matching inventory
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long, default_value_t = 0)]
        offset: usize,
        /// Return a bounded stable-ID page with a continuation cursor
        #[arg(long, requires = "json", conflicts_with = "offset")]
        page: bool,
        /// Previous page's next identity; keep the same filters
        #[arg(long, requires = "page")]
        after: Option<String>,
        #[arg(long)]
        task: Option<String>,
        /// Only Sessions without a Task association
        #[arg(long, conflicts_with = "task")]
        orphan: bool,
        #[arg(long)]
        search: Option<String>,
    },
    /// Connect to the live conversation, or resume its saved history
    #[command(name = "connect")]
    Open {
        /// Session ID, one of its Run IDs, or the provider's own conversation ID
        id: String,
        #[arg(long)]
        json: bool,
        /// Stop Loopflow-owned clients before resuming here
        #[arg(long, conflicts_with = "try_open")]
        replace: bool,
        /// Ask the provider to resume even when another client is active
        #[arg(long = "try", conflicts_with = "replace")]
        try_open: bool,
    },
    /// Find or start the one ongoing conversation of this repository or a Wave
    Ensure {
        /// The Wave's conversation instead of the repository's
        #[arg(short = 'w', long)]
        wave: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Give a primary Session's scope a fresh conversation
    Replace {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Complete a review or interactive session
    Complete {
        /// Session ID, one of its Run IDs, or the provider's own conversation ID
        id: String,
    },
    /// Rename a Session; a human name is never replaced by a suggestion
    Rename {
        /// Session ID, one of its Run IDs, or the provider's own conversation ID
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
        /// Session ID, one of its Run IDs, or the provider's own conversation ID
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
    /// Mark the active session ready for your review
    Ready {
        #[arg(value_name = "SUMMARY", required = true, num_args = 1..)]
        summary: Vec<String>,
    },
    /// Run the exact review skill in its durable terminal
    #[command(name = "serve-flow", hide = true)]
    ServeFlow {
        task_id: crate::work::task::TaskId,
        invocation_id: String,
        flow: String,
        node_id: String,
        skill: String,
        iteration: u32,
    },
    /// Run one prepared conversation in its durable terminal
    #[command(name = "serve-conversation", hide = true)]
    ServeConversation { input: String },
    /// Stop one exact native provider client after its review completes
    #[command(name = "stop-client", hide = true)]
    StopClient { input: String },
}

#[derive(Subcommand, Debug)]
pub enum WaveCommand {
    /// Local launchd jobs that run lf commands on a schedule
    Cron {
        #[command(subcommand)]
        cmd: CronCommand,
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
        /// Refresh planning from Linear before reading
        #[arg(long)]
        sync: bool,
    },
    /// Set the Home for Wave schedules and newly created work
    Place {
        name: String,
        home_id: crate::durable::HomeId,
        #[arg(long)]
        json: bool,
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
    /// Replace the current chapter's KRs, targets, and Flow recommendation
    UpdatePlan {
        #[arg(short = 'w', long)]
        wave: Option<String>,
        #[arg(long)]
        plan: std::path::PathBuf,
    },
}

#[derive(Args, Debug)]
pub struct SyncArgs {
    /// Print the planned sync strategy without mutating git
    #[arg(long, conflicts_with_all = ["manual", "continue_sync", "abort"])]
    pub plan: bool,
    /// Keep the sync local and leave conflicts for this process to resolve
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
    /// Inspect repository scheduling and Task enrollment
    Automation {
        #[arg(long)]
        json: bool,
    },
    /// Check enrolled Tasks and authorized deliveries once, then exit
    Reconcile {
        #[arg(long)]
        json: bool,
    },
    /// Enroll or hold a Task without interrupting running work
    Automate {
        issue: String,
        #[arg(value_parser = ["on", "off"])]
        state: String,
    },
    /// Run a reserved CI repair in its own process
    #[command(name = "__repair", hide = true)]
    Repair { incident: String, launcher: String },
    /// Internal: drive a Task Flow from its claimed boundary
    #[command(name = "__worker", hide = true)]
    Worker { task_id: crate::work::task::TaskId },
    /// Ensure tracked Task Work and its worktree without starting a worker
    Checkout {
        issue: String,
        #[arg(long)]
        name: Option<String>,
        /// Fork this Task's worktree from another Task's active PR
        #[arg(long = "stack-on", value_name = "PARENT_TASK")]
        stack_on: Option<String>,
        #[arg(long)]
        directive: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// File a Task in the current chapter; optionally prepare and run it
    Create {
        /// Wave name; defaults to the bound Wave
        #[arg(long)]
        wave: Option<String>,
        /// Task title; omitted when stdin supplies the report and first line
        #[arg(long)]
        title: Option<String>,
        /// Description; defaults to a report read from stdin
        #[arg(long)]
        notes: Option<String>,
        /// Validate placement and execution before filing, then run the Task
        #[arg(long)]
        run: bool,
        #[arg(long, requires = "run")]
        name: Option<String>,
        /// Select a Flow for this Task worker; defaults to the chapter recommendation
        #[arg(long, value_name = "FLOW", requires = "run")]
        flow: Option<String>,
        /// Fork this Task's worktree from another Task's active PR
        #[arg(long = "stack-on", value_name = "PARENT_TASK", requires = "run")]
        stack_on: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show durable Task facts and current worker evidence
    Status {
        /// Task issue; defaults to the Task in this checkout
        issue: Option<String>,
        #[arg(long)]
        json: bool,
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
    /// Complete planning work, or a placed Task whose pull requests are settled
    Complete {
        issue: String,
        #[arg(long)]
        summary: String,
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
        #[arg(short = 'w', long)]
        wave: Option<String>,
    },
    /// Read the thread or publish a comment; agent comments default to progress
    Comment {
        issue: String,
        message: Option<String>,
        /// Deliver new direction even when publishing from an agent Session
        #[arg(long, requires = "message")]
        steer: bool,
        #[arg(short = 'w', long)]
        wave: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Interrupt the active provider turn
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
    /// Stop the pinned Flow and begin a new one in a fresh Task worker;
    /// uses valid cached planning, even offline. New advice requires Linear publication.
    Restart {
        issue: String,
        advice: Option<String>,
        /// Replacement Flow; validated before any checkpoint or stop
        #[arg(long)]
        flow: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

impl TaskCommand {
    pub fn selector(&self) -> Option<&str> {
        match self {
            Self::Worker { .. }
            | Self::Create { .. }
            | Self::Sweep { .. }
            | Self::Reconcile { .. }
            | Self::Repair { .. }
            | Self::Automation { .. } => None,
            Self::Automate { issue, .. } => Some(issue),
            Self::Status { issue, .. } | Self::Abandon { issue, .. } => issue.as_deref(),
            Self::Checkout { issue, .. }
            | Self::Diff { issue, .. }
            | Self::Files { issue, .. }
            | Self::File { issue, .. }
            | Self::Save { issue, .. }
            | Self::Complete { issue, .. }
            | Self::Delete { issue }
            | Self::Edit { issue, .. }
            | Self::Comment { issue, .. }
            | Self::Interrupt { issue, .. }
            | Self::Wait { issue, .. }
            | Self::Restart { issue, .. } => Some(issue),
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
    /// Continue one interrupted machine install switch from its pinned candidate.
    #[command(hide = true)]
    RecoverSwitch {
        /// The fixed machine switch receipt to continue.
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
    },

    /// After an out-of-band merge, rotate this Task to its next serial PR,
    /// carrying committed and uncommitted follow-up onto the new branch.
    Next {
        /// Name the next serial branch (defaults to the settled PR's next slug,
        /// then the sequence number).
        slug: Option<String>,
    },
    /// Publish a ready PR headlessly: push, create or refresh, print state + URL.
    /// Opens no review surface.
    Publish {
        #[arg(short = 'm', long = "model")]
        model: Option<String>,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "body")]
        body: Option<String>,
    },
    /// Push and create or update a draft PR, then open its GitHub page.
    /// Existing ready PRs stay ready; opening a draft does not publish it.
    Open {
        #[arg(short = 'm', long = "model")]
        model: Option<String>,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "body")]
        body: Option<String>,
    },
    /// Prepare a PR to land: sync, clear scratch, mark ready, and assign it
    /// to you. Nothing merges until you click merge on GitHub.
    Submit {
        #[arg(long)]
        strict: bool,
        #[arg(short = 'p', long = "create-pr")]
        create_pr: bool,
        #[arg(short = 'c', long)]
        complete: bool,
        #[arg(long = "next")]
        next: Option<String>,
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
        #[arg(short = 'c', long)]
        complete: bool,
        #[arg(long = "next")]
        next: Option<String>,
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
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        local: bool,
        #[arg(short = 'c', long)]
        complete: bool,
        #[arg(long = "next")]
        next: Option<String>,
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
pub enum CronCommand {
    /// Record repair ownership without changing the failed evidence
    Disposition {
        subject: String,
        #[arg(long)]
        wave: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        reason: String,
    },
    /// Install or replace a scheduled lf invocation
    Add {
        /// Wave name passed to `lf <flow> --wave <wave>` (ambient if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        /// Flow or skill name to run
        #[arg(long = "flow")]
        flow: String,
        /// Daily or every-minute cron expression, or a schedule alias
        #[arg(long = "schedule", default_value = "daily")]
        schedule: String,
    },
    /// List installed loopflow cron jobs
    List {
        /// Only jobs for this Wave
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        /// Emit machine-readable job state
        #[arg(long)]
        json: bool,
    },
    /// Validate Home authority and declared jobs without changing launchd
    Preflight {
        /// Wave whose GOAL.md `crons:` are validated
        #[arg(short = 'w', long = "wave")]
        wave: String,
    },
    /// Reconcile installed launchd jobs to match a wave's declared `crons:`
    Sync {
        #[arg(
            short = 'w',
            long,
            required_unless_present = "repo",
            conflicts_with = "repo"
        )]
        wave: Option<String>,
        /// Install the finite repository Task check on this Home
        #[arg(long)]
        repo: bool,
        /// Remove the repository schedule; running work retains its authority
        #[arg(long, requires = "repo")]
        disable: bool,
    },
    /// Execute one installed cron job and persist its terminal receipt
    #[command(hide = true)]
    Run {
        /// Wave whose installed declaration is executed
        #[arg(short = 'w', long = "wave")]
        wave: String,
        /// Flow or skill name to run
        #[arg(long = "flow")]
        flow: String,
        /// Mark a launchd-owned invocation
        #[arg(long, hide = true)]
        scheduled: bool,
    },
    /// Show durable cron receipts
    History {
        /// Wave whose receipts are shown
        #[arg(short = 'w', long = "wave")]
        wave: String,
        /// Only receipts for this flow or skill
        #[arg(long = "flow")]
        flow: Option<String>,
        /// Receipt window in days
        #[arg(long, default_value_t = 35)]
        days: u32,
        /// Emit machine-readable receipts
        #[arg(long)]
        json: bool,
    },
    /// Ask launchd to fire an installed job
    Trigger {
        /// Wave whose installed job is fired
        #[arg(short = 'w', long = "wave")]
        wave: String,
        /// Flow or skill name to run
        #[arg(long = "flow")]
        flow: String,
        /// Wait for and return the scheduled receipt
        #[arg(long)]
        wait: bool,
        /// Maximum wait for a receipt
        #[arg(long, default_value = "15m")]
        timeout: String,
    },
    /// Uninstall a scheduled lf invocation
    Remove {
        /// Wave name passed to `lf <flow> --wave <wave>`
        #[arg(short = 'w', long = "wave")]
        wave: String,
        /// Flow or skill name to remove
        #[arg(long = "flow")]
        flow: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum CiCommand {
    /// Watch this repository's PR checks and start a ci-fix when a recorded landing fails
    Watch {
        /// Check every open PR once and exit
        #[arg(long)]
        once: bool,
        /// Keep the watcher running in the background as a launchd service
        #[arg(long, conflicts_with_all = ["once", "uninstall", "status"])]
        install: bool,
        /// Remove the background service
        #[arg(long, conflicts_with_all = ["once", "status"])]
        uninstall: bool,
        /// Show whether a watcher is live, its last poll, and what it started
        #[arg(long, conflicts_with = "once")]
        status: bool,
        /// Emit the status as JSON
        #[arg(long, requires = "status")]
        json: bool,
        /// Stop when this process exits
        #[arg(long = "parent-pid", hide = true)]
        parent_pid: Option<u32>,
    },
}

#[derive(Subcommand, Debug)]
pub enum RepoCommand {
    /// Connect a Wave to its Initiative and the repository's Team (Task prefix)
    Connect {
        /// Wave name (auto-detected if omitted)
        wave: Option<String>,
        /// Recursively initialize every Wave under wave/
        #[arg(long, conflicts_with = "wave")]
        all: bool,
        /// Repository Team key = Task prefix (e.g. LOO). Defaults from the repository name.
        #[arg(long = "team-key")]
        team_key: Option<String>,
        /// Repository Team display name. Defaults to the repository name.
        #[arg(long = "team-name")]
        team_name: Option<String>,
    },
    /// Refresh shared planning from Linear
    Refresh {
        wave: Option<String>,
        #[arg(long, conflicts_with = "wave")]
        all: bool,
    },

    /// Advance every Wave to the named Project plan
    NewChapter {
        name: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Release operations (run, check, notes, bump, tag, status)
    Release {
        #[command(subcommand)]
        cmd: ReleaseCommand,
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
    /// Show how failed CI is detected, repaired, and landed across this Home
    Ci {
        #[command(subcommand)]
        cmd: Option<CiCommand>,
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

    /// Reconcile linked Waves to the repository's Linear Team
    Reteam {
        #[arg(long)]
        apply: bool,
    },
}

/// Inspect and observe durable Homes.
#[derive(Debug, Subcommand)]
pub enum HomeCommand {
    /// Open or focus Loopflow.app
    Desktop,
    /// Capture a URL or local HTML file without claiming the user's browser
    Screenshot {
        #[command(flatten)]
        screenshot: ScreenshotArgs,
    },
    /// Install the latest published Loopflow release from any directory
    Install {
        #[command(subcommand)]
        cmd: Option<InstallCommand>,
    },
    /// Compile loopflow skills into your home vendor Skills directories.
    #[command(name = "sync-skills", hide = true)]
    SyncSkills {
        /// Confirm writes under ~/ without prompting
        #[arg(short = 'y', long = "yes")]
        yes: bool,
        /// Keep stale loopflow-generated skills
        #[arg(long = "no-prune")]
        no_prune: bool,
    },
    /// Diagnose installation, storage, Exec integrity and scheduled receipts
    Doctor {
        /// Diagnose repository planning without changing it
        #[arg(long)]
        planning: bool,
        /// Emit the audit as JSON
        #[arg(long)]
        json: bool,
    },
    /// Run lf on a Home or SSH host carrying your local credentials.
    ///
    /// Resolves local credentials and forwards a foreground account lease over
    /// SSH; Loopflow writes no managed provider credential on the remote. The
    /// Doppler token is never forwarded — name specific secrets with `--secret`
    /// to resolve them locally. Example: `lf home ssh <home-id> pr open`.
    Ssh {
        /// Prefer this origin account when the remote lf chooses a provider.
        #[arg(
            id = "ssh_preferred_provider_account",
            long = "account",
            value_name = "SELECTOR",
            conflicts_with = "ssh_restricted_provider_account"
        )]
        origin_account: Vec<String>,
        /// Restrict remote provider launches to these origin accounts.
        #[arg(
            id = "ssh_restricted_provider_account",
            long = "only-account",
            value_name = "SELECTOR",
            conflicts_with = "ssh_preferred_provider_account"
        )]
        origin_only_account: Vec<String>,
        /// HomeId (preferred), SSH alias, or user@host
        target: String,
        /// Repository path on the remote, relative to $HOME
        #[arg(long = "repo")]
        repo: Option<String>,
        /// Doppler secret to resolve locally and forward as an env var
        /// (repeatable). The Doppler token itself is never forwarded.
        #[arg(long = "secret")]
        secret: Vec<String>,
        /// Forward the ssh-agent (`ssh -A`). Off by default: git pushes use the
        /// forwarded GH_TOKEN over HTTPS, so agent forwarding is unneeded risk.
        #[arg(long = "forward-agent")]
        forward_agent: bool,
        /// Arguments for the remote lf. The target is the boundary: every
        /// argument after it belongs to the remote invocation.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        lf_args: Vec<String>,
    },
    /// Print the configured participant display name.
    User {
        #[arg(long)]
        json: bool,
    },
    /// Print this machine's stable local Home identity.
    Id {
        #[arg(long)]
        json: bool,
    },
    /// Record the current route for a known Home identity.
    Observe {
        home_id: crate::durable::HomeId,
        route: String,
        #[arg(long)]
        json: bool,
    },
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
    /// Explain configured and automatic account selection, or replace a route
    #[command(args_conflicts_with_subcommands = true)]
    Route {
        #[command(subcommand)]
        cmd: Option<RouteCommand>,
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

#[derive(Debug, Subcommand)]
pub enum RouteCommand {
    /// Replace a provider's ordered route
    Set {
        provider: String,
        #[arg(required = true)]
        accounts: Vec<String>,
        #[arg(long, conflicts_with = "default")]
        repo: Option<String>,
        #[arg(long)]
        default: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ReleaseCommand {
    /// Show original due opportunities and their release evidence
    History {
        #[arg(short = 'w', long)]
        wave: String,
        #[arg(long, default_value_t = 35)]
        days: u32,
        #[arg(long)]
        json: bool,
    },
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
    /// Create a low-level sibling worktree
    Create {
        /// Worktree name
        name: String,
        /// Print the placement plan without creating a worktree
        #[arg(long)]
        plan: bool,
        /// Keep this workspace after delivery and keep scratch local
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
    /// Remove clean terminal or inactive worktrees
    Prune {
        /// Show what would be pruned without removing anything
        #[arg(long)]
        dry_run: bool,
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
    fn orphan_selects_inventory_and_cannot_opt_out_at_launch() {
        let cli = Cli::try_parse_from(["lf", "session", "list", "--orphan", "--json"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Session {
                cmd: SessionCommand::List { orphan: true, .. }
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
        assert!(Cli::try_parse_from(["lf", "repo", "webhook", "serve"]).is_err());
        assert!(Cli::try_parse_from(["lf", "wave", "serve", "product"]).is_err());
        for args in [
            vec!["lf", "list"],
            vec!["lf", "wave", "list", "--json"],
            vec!["lf", "pr", "checks"],
            vec!["lf", "repo", "refresh", "product"],
            vec!["lf", "repo", "refresh", "--all"],
            vec!["lf", "wave", "rename", "product", "--title", "Product"],
            vec![
                "lf",
                "wave",
                "place",
                "product",
                "home_00000000000000000000000000000001",
            ],
            vec!["lf", "discord", "serve", "product"],
            vec!["lf", "home", "doctor", "--planning", "--json"],
            vec!["lf", "wave", "status", "product", "--sync"],
        ] {
            assert!(Cli::try_parse_from(args.clone()).is_ok(), "{args:?}");
        }
        for verb in [
            "recover", "enable", "disable", "prepare", "resume", "advance",
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
            format!("lf {}\n", crate::build_info::BUILD_VERSION)
        );
    }

    #[test]
    fn screenshot_requires_an_output_and_accepts_a_viewport() {
        let cli = Cli::try_parse_from([
            "lf",
            "home",
            "screenshot",
            "page.html",
            "--output",
            "capture.png",
            "--width",
            "390",
            "--height",
            "844",
        ])
        .expect("parse screenshot");
        let Some(Commands::Home {
            cmd: crate::lf::HomeCommand::Screenshot { screenshot },
        }) = cli.command
        else {
            panic!("expected screenshot command");
        };
        assert_eq!(screenshot.source, "page.html");
        assert_eq!(screenshot.output, PathBuf::from("capture.png"));
        assert_eq!((screenshot.width, screenshot.height), (390, 844));
        assert!(Cli::try_parse_from(["lf", "home", "screenshot", "page.html"]).is_err());
    }

    #[test]
    fn install_exposes_refresh_and_schedule_but_hides_transaction_commands() {
        let mut command = Cli::command();
        let home = command.find_subcommand_mut("home").unwrap();
        assert!(home.render_long_help().to_string().contains("install"));
        let help = home
            .find_subcommand_mut("install")
            .unwrap()
            .render_long_help()
            .to_string();
        assert!(help.contains("schedule"));
        assert!(!help.contains("preflight"));
        assert!(matches!(
            Cli::try_parse_from(["lf", "home", "install"])
                .unwrap()
                .command,
            Some(Commands::Home {
                cmd: crate::lf::HomeCommand::Install { cmd: None }
            })
        ));
        assert!(Cli::try_parse_from(["lf", "home", "install", "schedule"]).is_ok());
        assert!(Cli::try_parse_from(["lf", "home", "install", "status"]).is_err());
        assert!(Cli::try_parse_from(["lf", "home", "install", "preflight"]).is_ok());
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
        assert!(Cli::try_parse_from(["lf", "pr", "checks", "--logs"]).is_ok());
        assert!(Cli::try_parse_from(["lf", "home", "probe", "product"]).is_err());
        assert!(Cli::try_parse_from(["lf", "wt", "ci"]).is_err());
    }

    #[test]
    fn chapter_and_task_commands_require_no_project_selector() {
        let preview =
            Cli::try_parse_from(["lf", "repo", "new-chapter", "two", "--dry-run", "--json"])
                .unwrap();
        assert!(matches!(
            preview.command,
            Some(Commands::Repo {
                cmd: RepoCommand::NewChapter { dry_run: true, .. }
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
            "--mode",
            "batch",
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
            Some(Commands::Repo { cmd: RepoCommand::Ci {
                cmd: None,
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
    fn account_lease_probe_is_parseable_but_hidden() {
        let cli = Cli::try_parse_from(["lf", "--__account-lease-probe"])
            .expect("parse internal account lease probe");
        assert!(cli.account_lease_probe);
        assert!(!Cli::command()
            .render_long_help()
            .to_string()
            .contains("__account-lease-probe"));
    }

    #[test]
    fn telemetry_scorecard_is_parseable_but_hidden() {
        let cli = Cli::try_parse_from(["lf", "__telemetry-scorecard", "--json"])
            .expect("parse internal telemetry scorecard");
        assert!(matches!(
            cli.command,
            Some(Commands::TelemetryScorecard { json: true })
        ));
        assert!(!Cli::command()
            .render_long_help()
            .to_string()
            .contains("__telemetry-scorecard"));
    }

    #[test]
    fn ssh_parser_respects_the_internal_target_boundary() {
        let cli = Cli::try_parse_from([
            "lf",
            "home",
            "ssh",
            "--account",
            "reserve",
            "mini",
            "--",
            "task",
            "pursue",
        ])
        .expect("parse origin SSH account preference");

        assert!(cli.account.is_empty());
        assert!(matches!(
            cli.command,
            Some(Commands::Home { cmd: crate::lf::HomeCommand::Ssh { origin_account, lf_args, .. } })
                if origin_account == vec!["reserve"]
                    && lf_args == vec!["task", "pursue"]
        ));

        let after_host = Cli::try_parse_from([
            "lf",
            "home",
            "ssh",
            "mini",
            "--",
            "--account",
            "reserve",
            "task",
            "pursue",
        ])
        .expect("parse remote account preference");
        assert!(after_host.account.is_empty());
        assert!(matches!(
            after_host.command,
            Some(Commands::Home { cmd: crate::lf::HomeCommand::Ssh { lf_args, .. } })
                if lf_args == vec!["--account", "reserve", "task", "pursue"]
        ));
    }

    #[test]
    fn account_use_takes_its_provider_before_the_verb() {
        let cli =
            Cli::try_parse_from(["lf", "account", "codex", "use", "jack@loopflow.studio"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Account {
            cmd: Some(AccountCommand::Use { email }),
            provider: Some(crate::provider_auth::Provider::Codex),
            ..
        }) if email == "jack@loopflow.studio"));
    }

    #[test]
    fn auth_set_accepts_ordered_service_profiles() {
        let cli = Cli::try_parse_from([
            "lf",
            "account",
            "set",
            "linear",
            "--chrome-profile",
            "Work",
            "--chrome-profile",
            "Personal",
        ])
        .unwrap();
        assert!(matches!(cli.command, Some(Commands::Account {
            cmd: Some(AccountCommand::Set { email: None, chrome_profile, .. }), ..
        }) if chrome_profile == ["Work", "Personal"]));
        assert!(
            Cli::try_parse_from(["lf", "account", "set", "linear", "--clear-cooldown"]).is_err()
        );
    }

    #[test]
    fn auth_connect_addresses_an_account_and_optional_bootstrap_venue() {
        let cli = Cli::try_parse_from([
            "lf",
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
            Some(Commands::Account {
                cmd: Some(AccountCommand::Connect {
                    provider,
                    email: Some(email),
                    chrome_profile: Some(chrome_profile),
                    ..
                }), ..
            }) if provider == "claude"
                && email == "operator@"
                && chrome_profile == "Profile 9"
        ));
    }

    #[test]
    fn service_auth_accepts_a_remembered_chrome_profile() {
        let cli = Cli::try_parse_from([
            "lf",
            "account",
            "connect",
            "linear",
            "--chrome-profile",
            "Work",
        ])
        .unwrap();
        assert!(
            matches!(cli.command, Some(Commands::Account { cmd: Some(AccountCommand::Connect {
            provider, email: None, chrome_profile: Some(profile), ..
        }), .. }) if provider == "linear" && profile == "Work")
        );
    }

    #[test]
    fn auth_connect_sources_are_exclusive() {
        assert!(Cli::try_parse_from([
            "lf",
            "account",
            "connect",
            "claude",
            "a@example.com",
            "--import"
        ])
        .is_ok());
        assert!(Cli::try_parse_from(["lf", "account", "connect", "codex", "--api-key"]).is_ok());
        for flags in [
            vec!["--import", "--api-key"],
            vec!["--import", "--chrome-profile", "Work"],
        ] {
            assert!(Cli::try_parse_from(
                ["lf", "account", "connect", "claude", "a@example.com"]
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
            Some(Commands::Account {
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
            }) if provider == "codex"
                && email == "loopflow-eng@"
                && login_email == "engineering@example.com"
                && routing == "automatic"
                && plan == "max"
                && paid_through == "2026-08-14"
        ));
    }

    #[test]
    fn pm_init_accepts_positional_wave() {
        let cli = Cli::try_parse_from(["lf", "repo", "connect", "pm"]).expect("parse");
        let Some(Commands::Repo {
            cmd: RepoCommand::Connect { wave, all, .. },
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
                    json,
                },
        }) = cli.command
        else {
            panic!("expected task checkout command");
        };
        assert_eq!(issue, "INF-123");
        assert_eq!(name.as_deref(), Some("runtime-research"));
        assert_eq!(stack_on.as_deref(), Some("INF-122"));
        assert_eq!(directive.as_deref(), Some("collect both reports"));
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
        assert!(Cli::try_parse_from([
            "lf",
            "task",
            "create",
            "--run",
            "--directive",
            "duplicate description"
        ])
        .is_err());
        let cli = Cli::try_parse_from([
            "lf",
            "task",
            "create",
            "--run",
            "--title",
            "New task",
            "--notes",
            "Full report",
            "--name",
            "placed",
        ])
        .unwrap();
        assert!(
            matches!(cli.command, Some(Commands::Task { cmd: TaskCommand::Create { run: true, title: Some(title), notes: Some(notes), .. } }) if title == "New task" && notes == "Full report")
        );
    }

    #[test]
    fn task_completion_and_pr_dispositions_parse() {
        let complete = Cli::try_parse_from([
            "lf",
            "task",
            "complete",
            "INF-123",
            "--summary",
            "Root cause recorded",
        ])
        .expect("parse task complete");
        assert!(matches!(
            complete.command,
            Some(Commands::Task {
                cmd: TaskCommand::Complete { issue, summary, .. }
            }) if issue == "INF-123" && summary == "Root cause recorded"
        ));

        let land = Cli::try_parse_from(["lf", "pr", "land", "-c"]).expect("parse completing land");
        assert!(matches!(
            land.command,
            Some(Commands::Pr {
                cmd: Some(PrCommand::Land {
                    complete: true,
                    next: None,
                    ..
                })
            })
        ));

        let submit =
            Cli::try_parse_from(["lf", "pr", "submit", "--next", "released-upgrade-proof"])
                .expect("parse continuation submit");
        assert!(matches!(
            submit.command,
            Some(Commands::Pr{
                cmd: Some(PrCommand::Submit {
                    complete: false,
                    next: Some(next),
                    ..
                })
            }) if next == "released-upgrade-proof"
        ));
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
    fn task_completion_has_one_public_command() {
        let cli = Cli::try_parse_from([
            "lf",
            "task",
            "complete",
            "LOO-42",
            "--summary",
            "Delivered",
            "--json",
        ])
        .unwrap();
        assert!(
            matches!(cli.command, Some(Commands::Task { cmd: TaskCommand::Complete { issue, summary, json: true } }) if issue == "LOO-42" && summary == "Delivered")
        );
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

        for removed in [
            "steer",
            "follow-up",
            "acknowledge",
            "decide",
            "request-decision",
        ] {
            assert!(
                Cli::try_parse_from(["lf", "task", removed, "INF-123"]).is_err(),
                "{removed} must not remain as a compatibility command"
            );
        }
    }

    #[test]
    fn task_restart_accepts_optional_advice() {
        let cli = Cli::try_parse_from([
            "lf",
            "task",
            "restart",
            "LOO-267",
            "replace the old runtime model",
            "--json",
        ])
        .expect("parse Task restart");
        assert!(matches!(
            cli.command,
            Some(Commands::Task {
                cmd: TaskCommand::Restart {
                    issue,
                    advice: Some(advice),
                    flow: None,
                    json: true,
                }
            }) if issue == "LOO-267" && advice == "replace the old runtime model"
        ));
    }

    #[test]
    fn navigation_belongs_to_flow_decisions() {
        for args in [
            vec![
                "lf",
                "--task",
                "LOO-1",
                "flow",
                "start",
                "--session",
                "review",
            ],
            vec![
                "lf",
                "--task",
                "LOO-1",
                "flow",
                "start",
                "--summary",
                "approved",
            ],
            vec!["lf", "session", "advance", "review", "approved"],
            vec!["lf", "session", "iterate", "review", "revise"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
        assert!(Cli::try_parse_from(["lf", "--task", "LOO-1", "flow", "start"]).is_ok());
        assert!(Cli::try_parse_from(["lf", "session", "complete", "review"]).is_ok());
    }

    #[test]
    fn cli_preserves_review_completion_and_rejects_ask() {
        assert!(Cli::try_parse_from(["lf", "session", "ask", "Help"]).is_err());
        let ready = Cli::try_parse_from(["lf", "session", "ready", "Ready for review"])
            .expect("parse session readiness");
        assert!(matches!(
            ready.command,
            Some(Commands::Session {
                cmd: SessionCommand::Ready { summary }
            }) if summary == ["Ready for review"]
        ));

        for args in [
            vec!["lf", "session", "ready"],
            vec!["lf", "session", "advance", "task_flow"],
            vec!["lf", "session", "iterate", "task_flow"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }

        let open = Cli::try_parse_from(["lf", "session", "connect", "run_123", "--replace"])
            .expect("parse replacement open");
        assert!(matches!(
            open.command,
            Some(Commands::Session {
                cmd: SessionCommand::Open {
                    id,
                    replace: true,
                    try_open: false,
                    json: false,
                }
            }) if id == "run_123"
        ));
        assert!(Cli::try_parse_from(
            ["lf", "session", "connect", "run_123", "--replace", "--try",]
        )
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

        let complete = Cli::try_parse_from(["lf", "session", "complete", "run_123"])
            .expect("parse interactive completion");
        assert!(matches!(
            complete.command,
            Some(Commands::Session {
                cmd: SessionCommand::Complete { id }
            }) if id == "run_123"
        ));
    }

    #[test]
    fn retired_loop_and_stack_surfaces_are_not_first_class_commands() {
        let loop_cli = Cli::try_parse_from(["lf", "loop", "infrastructure"])
            .expect("unknown names remain eligible for skill discovery");
        assert!(matches!(loop_cli.command, Some(Commands::External(_))));
        assert!(Cli::try_parse_from(["lf", "wt", "create", "child", "--stack"]).is_err());
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
        let cli = Cli::try_parse_from(["lf", "repo", "connect", "--all"]).expect("parse");
        let Some(Commands::Repo {
            cmd:
                RepoCommand::Connect {
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
            "repo",
            "connect",
            "product",
            "--team-key",
            "PRD",
            "--team-name",
            "Product",
        ])
        .expect("parse");
        let Some(Commands::Repo {
            cmd:
                RepoCommand::Connect {
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
        let cli = Cli::try_parse_from(["lf", "repo", "reteam"]).expect("parse");
        let Some(Commands::Repo {
            cmd: RepoCommand::Reteam { apply },
        }) = cli.command
        else {
            panic!("expected pm reteam command");
        };
        assert!(!apply);

        let cli = Cli::try_parse_from(["lf", "repo", "reteam", "--apply"]).expect("parse apply");
        let Some(Commands::Repo {
            cmd: RepoCommand::Reteam { apply },
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

    #[test]
    fn pr_open_accepts_model_override() {
        let cli = Cli::try_parse_from(["lf", "pr", "open", "-m", "codex"]).expect("parse");
        let Some(Commands::Pr {
            cmd: Some(PrCommand::Open { model, title, body }),
        }) = cli.command
        else {
            panic!("expected pr command");
        };

        assert_eq!(model.as_deref(), Some("codex"));
        assert_eq!(title, None);
        assert_eq!(body, None);
    }

    #[test]
    fn top_level_model_reaches_pr_open() {
        let cli = Cli::try_parse_from(["lf", "-m", "codex", "pr", "open"]).expect("parse");
        let Some(Commands::Pr {
            cmd: Some(PrCommand::Open { model, title, body }),
        }) = cli.command
        else {
            panic!("expected pr command");
        };

        assert_eq!(cli.model.as_deref(), Some("codex"));
        assert_eq!(model, None);
        assert_eq!(title, None);
        assert_eq!(body, None);
    }
}

#[derive(Debug, Subcommand)]
pub enum DiscordCommand {
    /// Poll a configured channel and post each Session's final answer
    Serve { wave: String },
}
