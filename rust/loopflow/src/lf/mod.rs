use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

pub mod commands;
pub mod discovery;
pub mod navigation;
pub mod output;

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
    #[arg(short = 'c', long = "clipboard", short_alias = 'C')]
    pub clipboard: bool,

    /// Model to use (harness or harness:model)
    #[arg(short = 'm', long = "model", short_alias = 'M')]
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

    /// Internal SSH compatibility and broker-connectivity probe.
    #[arg(long = "__account-lease-probe", hide = true)]
    pub account_lease_probe: bool,

    /// Skip permission prompts
    #[arg(long)]
    pub yolo: bool,

    /// Run interactively
    #[arg(short = 'i', long = "interactive", short_alias = 'I')]
    pub interactive: bool,

    /// Run in batch/headless mode
    #[arg(short = 'b', long = "batch", short_alias = 'B')]
    pub batch: bool,

    /// Hand off Claude, Codex, or OpenCode to the terminal (overrides session.launch)
    #[arg(long, conflicts_with = "ide")]
    pub tui: bool,

    /// Hand off Claude or Codex to the vendor app (overrides session.launch)
    #[arg(long)]
    pub ide: bool,

    /// Enable Chrome integration (Claude)
    #[arg(long)]
    pub chrome: bool,

    /// Disable Chrome integration (Claude)
    #[arg(long = "no-chrome", overrides_with = "chrome")]
    pub no_chrome: bool,

    /// Include files changed on branch
    #[arg(long = "diff-files")]
    pub diff_files: bool,

    /// Exclude files changed on branch
    #[arg(long = "no-diff-files", overrides_with = "diff_files")]
    pub no_diff_files: bool,

    /// Include raw git diff
    #[arg(long = "diff")]
    pub diff: bool,

    /// Exclude raw git diff
    #[arg(long = "no-diff", overrides_with = "diff")]
    pub no_diff: bool,

    /// Maximum agent turns for this invocation
    #[arg(long = "max-turns")]
    pub max_turns: Option<u32>,

    /// Select Wave Work, or qualify a selected Task
    #[arg(short = 'w', long = "wave", short_alias = 'W')]
    pub wave: Option<String>,

    /// Select Task Work
    #[arg(long = "task", value_name = "ISSUE")]
    pub task: Option<String>,

    /// Select one Work for a direct skill, flow or inline prompt
    #[arg(
        long = "as",
        value_name = "WORK",
        conflicts_with_all = ["wave", "task"]
    )]
    pub as_work: Option<String>,

    /// Keep a Work-bound internal launch in this exact checkout.
    #[arg(long = "__cwd", value_name = "PATH", hide = true)]
    pub bound_cwd: Option<PathBuf>,

    /// Exclude loopflow operating guidance
    #[arg(long = "no-loopflow")]
    pub no_loopflow: bool,
}

impl Cli {
    pub(crate) fn launch_options(&self) -> Self {
        Self {
            command: None,
            docs: self.docs.clone(),
            clipboard: self.clipboard,
            model: self.model.clone(),
            account: self.account.clone(),
            only_account: self.only_account.clone(),
            account_lease_probe: self.account_lease_probe,
            yolo: self.yolo,
            interactive: self.interactive,
            batch: self.batch,
            tui: self.tui,
            ide: self.ide,
            chrome: self.chrome,
            no_chrome: self.no_chrome,
            diff_files: self.diff_files,
            no_diff_files: self.no_diff_files,
            diff: self.diff,
            no_diff: self.no_diff,
            max_turns: self.max_turns,
            wave: self.wave.clone(),
            task: self.task.clone(),
            as_work: self.as_work.clone(),
            bound_cwd: self.bound_cwd.clone(),
            no_loopflow: self.no_loopflow,
        }
    }

    fn toggle_setting(enabled: bool, disabled: bool) -> Option<bool> {
        if enabled {
            Some(true)
        } else if disabled {
            Some(false)
        } else {
            None
        }
    }

    /// Get chrome setting: Some(true) if --chrome, Some(false) if --no-chrome, None if neither.
    pub fn chrome_setting(&self) -> Option<bool> {
        Self::toggle_setting(self.chrome, self.no_chrome)
    }

    /// Get diff_files setting: Some(true) if --diff-files, Some(false) if --no-diff-files, None if neither.
    pub fn diff_files_setting(&self) -> Option<bool> {
        Self::toggle_setting(self.diff_files, self.no_diff_files)
    }

    /// Get diff setting: Some(true) if --diff, Some(false) if --no-diff, None if neither.
    pub fn diff_setting(&self) -> Option<bool> {
        Self::toggle_setting(self.diff, self.no_diff)
    }

    /// The most specific Work selected for direct execution.
    pub fn work_subject_selector(&self) -> Option<String> {
        self.as_work.clone().or_else(|| {
            self.task
                .as_ref()
                .map(|task| format!("task:{task}"))
                .or_else(|| self.wave.as_ref().map(|wave| format!("wave:{wave}")))
        })
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
pub enum UserCommand {
    /// Show the display name from personal Loopflow configuration or Git
    Name {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Inspect the current user
    User {
        #[command(subcommand)]
        cmd: UserCommand,
    },
    /// Run an inline prompt
    #[command(name = ":")]
    Inline {
        #[arg(trailing_var_arg = true)]
        prompt: Vec<String>,
    },
    /// Open or focus Loopflow.app
    Desktop,
    /// Capture a URL or local HTML file without claiming the user's browser
    Screenshot {
        #[command(flatten)]
        screenshot: ScreenshotArgs,
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
    /// Open a durable session and wait for the user to complete it
    Ask {
        #[command(flatten)]
        ask: AskArgs,
    },
    /// Inspect and continue Sessions
    Session {
        #[command(subcommand)]
        cmd: SessionCommand,
    },
    /// Install the latest published Loopflow release from any directory
    Install {
        #[command(subcommand)]
        cmd: Option<InstallCommand>,
    },
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
    /// Rebase current branch onto target (default: main)
    Rebase {
        /// Print the planned rebase strategy without mutating git
        #[arg(long, conflicts_with_all = ["manual", "continue_rebase", "abort"])]
        plan: bool,
        /// Keep the rebase local and leave conflicts for this process to resolve
        #[arg(long, conflicts_with_all = ["plan", "continue_rebase", "abort"])]
        manual: bool,
        /// Stage resolved conflict paths and continue the local rebase
        #[arg(long = "continue", conflicts_with_all = ["plan", "manual", "abort"])]
        continue_rebase: bool,
        /// Abort the local rebase in progress
        #[arg(long, conflicts_with_all = ["plan", "manual", "continue_rebase"])]
        abort: bool,
        /// Explicitly claim a raw rebase that has no Loopflow owner
        #[arg(long, conflicts_with_all = ["plan", "manual"])]
        adopt: bool,
        /// Branch to rebase onto
        onto: Option<String>,
    },
    /// Commit changes
    Commit {
        #[arg(short = 'm', long = "message", short_alias = 'M')]
        message: Option<String>,
        #[arg(short = 'p', long = "push", short_alias = 'P')]
        push: bool,
        #[arg(long = "no-add")]
        no_add: bool,
    },
    /// Provider authentication for local lf skills and ops
    Auth {
        #[command(subcommand)]
        cmd: AuthCommand,
    },
    /// Release operations (run, check, notes, bump, tag, status)
    Release {
        #[command(subcommand)]
        cmd: ReleaseCommand,
    },
    /// Repository provider administration
    Repo {
        #[command(subcommand)]
        cmd: RepoCommand,
    },
    /// Inspect this Home and observe routes to other Homes
    Home {
        #[command(subcommand)]
        cmd: HomeCommand,
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
    /// Bridge new Discord messages to finite Wave Runs
    Discord {
        #[command(subcommand)]
        cmd: DiscordCommand,
    },
    /// Local launchd jobs that run lf commands on a schedule
    Cron {
        #[command(subcommand)]
        cmd: CronCommand,
    },
    /// Manage Wave identity, placement and planning
    Wave {
        #[command(subcommand)]
        cmd: WaveCommand,
    },
    /// Linear-backed Task work and bounded workers
    Task {
        #[command(subcommand)]
        cmd: TaskCommand,
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
    /// Show direct provider-authored usage from Home-local Run records
    Usage {
        /// Emit Run usage evidence as JSON
        #[arg(long)]
        json: bool,
        /// Run window, in days (zero means all time)
        #[arg(long, default_value_t = 30)]
        days: u32,
        /// Limit to Runs attributed to one Wave
        #[arg(long)]
        wave: Option<String>,
        /// Limit to Runs attributed to one Project
        #[arg(long)]
        project: Option<String>,
        /// Limit to Runs attributed to one Task
        #[arg(long)]
        task: Option<String>,
    },
    /// Internal: render the repository maintainer scorecard for telemetry-daily
    #[command(name = "__telemetry-scorecard", hide = true)]
    TelemetryScorecard {
        /// Emit structured JSON for operator automation
        #[arg(long)]
        json: bool,
    },
    /// Show how failed CI is detected, repaired, and landed across this Home
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
    /// Print one parseable snapshot of live Loopflow call trees
    Ps {
        /// Emit the versioned activity snapshot as JSON
        #[arg(long)]
        json: bool,
    },
    /// Refresh live Loopflow call trees on a terminal; print once when redirected
    Top {
        /// Emit one versioned activity snapshot as JSON
        #[arg(long)]
        json: bool,
    },
    /// Reap registered orphan providers and remove dead process receipts
    Prune {
        /// Show exact targets without changing process or receipt state
        #[arg(long)]
        dry_run: bool,
        /// Emit the versioned prune report as JSON
        #[arg(long)]
        json: bool,
    },
    /// Audit the local run ledger: continuity, vocabulary, attribution, identity, lineage, coverage
    Doctor {
        /// Diagnose repository planning without changing it
        #[arg(long)]
        planning: bool,
        /// Emit the audit as JSON
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
        /// Emit the roadmap snapshot as JSON
        #[arg(long)]
        json: bool,
        /// Span every repository on this machine, not just the current one.
        #[arg(long)]
        all: bool,
    },
    /// Show one ordered record of durable Work, Run, PR, and Steer facts
    Activity {
        /// Relative window (7d, 24h, 30m) or RFC3339 start
        #[arg(long, default_value = "7d")]
        since: String,
        /// Maximum rows after Work filters (1-200)
        #[arg(long, default_value_t = 50)]
        limit: usize,
        /// Scope to one Wave by name
        #[arg(long)]
        wave: Option<String>,
        /// Scope to one Project by slug
        #[arg(long)]
        project: Option<String>,
        /// Scope to one Task by Linear identifier
        #[arg(long)]
        task: Option<String>,
        /// Emit the typed activity snapshot as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show recent agent-backed skill runs with context and token evidence
    Runs {
        /// Observe current provider-backed Runs without the history window or cap
        #[arg(long, conflicts_with_all = ["run", "parent", "wave", "project"])]
        active: bool,
        /// Retain discovery and stream active snapshots until stdin closes
        #[arg(long, requires_all = ["active", "json"])]
        watch: bool,
        /// Inspect one Run by full id or unambiguous displayed prefix
        #[arg(conflicts_with_all = ["parent", "task", "project", "wave"])]
        run: Option<String>,
        /// List every direct child of one Run, without the recent-history cap
        #[arg(long, conflicts_with_all = ["run", "task", "project", "wave"])]
        parent: Option<String>,
        /// Print the Run's append-only event stream verbatim
        #[arg(
            long,
            requires = "run",
            conflicts_with_all = ["final_answer", "json", "resume"]
        )]
        events: bool,
        /// Print the Run's last durable provider conclusion
        #[arg(
            long = "final",
            requires = "run",
            conflicts_with_all = ["events", "json", "resume"]
        )]
        final_answer: bool,
        /// Resume the Run's provider-native interactive session
        #[arg(
            long,
            requires = "run",
            conflicts_with_all = ["events", "final_answer", "json"]
        )]
        resume: bool,
        /// Drill to one roadmap Task by its Linear issue identifier (e.g. W2-122)
        #[arg(long)]
        task: Option<String>,
        /// Drill to one roadmap Project by slug
        #[arg(long)]
        project: Option<String>,
        /// Scope to one Wave by name
        #[arg(long)]
        wave: Option<String>,
        /// Emit the run history as JSON
        #[arg(long)]
        json: bool,
    },
    /// Launch the exact provider request recorded by a prior Run as a child Run.
    Replay {
        /// Full Run id or an unambiguous displayed prefix
        run: String,
    },
    // architecture-shim: retired-op
    // Same reservation for the retired `lf op` namespace, which held every
    // operation before the runtime collapsed to waves, projects, and tasks.
    // Without it, `lf op land` reports a missing skill named `op` instead of
    // naming the command that replaced it.
    #[command(
        name = "op",
        hide = true,
        about = "Removed; the operations are top-level (`lf pr`, `lf rebase`, `lf wt`, `lf task`)",
        arg_required_else_help = true
    )]
    RetiredOp {
        #[arg(required = true, value_name = "COMMAND", value_parser = reject_retired_op)]
        removed: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        rest: Vec<String>,
    },
    /// Run lf on a Home or SSH host carrying your local credentials.
    ///
    /// Resolves local credentials and forwards a foreground account lease over
    /// SSH; Loopflow writes no managed provider credential on the remote. The
    /// Doppler token is never forwarded — name specific secrets with `--secret`
    /// to resolve them locally. Example: `lf ssh <home-id> pr open`.
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
    /// Run or inspect skills
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
    /// List skills, optionally inside a namespace
    List {
        namespace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Inspect a skill without launching it
    Show { name: String },
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand, Debug)]
pub enum FlowCommand {
    /// List authored flows
    List {
        #[arg(long)]
        json: bool,
    },
    /// Inspect the expanded steps of a flow
    Show { name: String },
    /// Validate a flow and its review points
    Validate { name: String },
    /// Record a decision for the current Flow boundary
    Decide {
        #[arg(value_parser = ["advance", "iterate"])]
        decision: String,
        #[arg(required = true, num_args = 1..)]
        summary: Vec<String>,
    },
    /// Select an authored branch
    Route { path: String },
    /// Open a Session to resolve a blocked decision
    Blocked {
        #[arg(required = true, num_args = 1..)]
        reason: Vec<String>,
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

#[derive(Args, Debug, Default)]
pub struct AskArgs {
    /// Named skill for the session
    #[arg(long)]
    pub skill: Option<String>,
    /// What the session should work through
    #[arg(trailing_var_arg = true, value_name = "QUESTION")]
    pub question: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum SessionCommand {
    /// List Sessions
    List {
        #[arg(long)]
        json: bool,
        /// Include waiting steps from every repository on this machine
        #[arg(long)]
        all: bool,
    },
    /// Open or resume one session in this terminal
    Open {
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
    /// Complete a review, blocked Ask, or interactive session
    Complete { id: String },
    /// Rename a Session; a human name is never replaced by a suggestion
    Rename {
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
        id: String,
        /// The Task, by its issue identifier (e.g. INF-123)
        #[arg(long)]
        task: String,
        #[arg(long)]
        json: bool,
    },
    /// Store the Session files an older Home kept beside its Runs; run once
    Import {
        /// Report what would be stored and store nothing
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
    /// Run one ad-hoc request in its durable terminal
    #[command(name = "serve-ask", hide = true)]
    ServeAsk { run_id: crate::durable::RunId },
    /// Stop one exact native provider Run after its review completes
    #[command(name = "stop-run", hide = true)]
    StopRun { run_id: crate::durable::RunId },
}

/// Name the surviving spelling for each retired `lf op` verb. Prompts, `.lf/`
/// adaptations, and older installed binaries still say `lf op …`; a caller who
/// types it should learn where the operation went, not that a skill named `op`
/// is missing. Nothing here executes — it only fails with a memory.
fn reject_retired_op(sub: &str) -> Result<String, String> {
    let hint = match sub {
        // Ephemeral rotation is gone, not renamed: a worker forks from and
        // targets its parent branch, so no branch rotates through a worktree.
        "next" | "advance" => {
            "it has no replacement — dispatch work with `lf task run <issue-id>`, \
             and the worker forks from and targets its parent branch"
                .to_string()
        }
        "pr" => "use `lf pr open`".to_string(),
        "submit" => "use `lf pr submit`".to_string(),
        "land" => "use `lf pr land`".to_string(),
        "dispatch" => "use `lf task run <issue-id>`".to_string(),
        "auth" | "commit" | "cron" | "doctor" | "rebase" | "release" | "sync-skills" | "wt" => {
            format!("use `lf {sub}`")
        }
        _ => "the operations are top-level now — see `lf --help`".to_string(),
    };
    Err(format!("`lf op {sub}` was removed; {hint}"))
}

#[derive(Subcommand, Debug)]
pub enum WaveCommand {
    /// List every wave in the registry (running and stopped), marking which
    /// have a live server. Local-only query over the shared ledger.
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
    /// Show one Wave's chapter, Tasks, Runs, and live loop
    /// state from the registry. Defaults to the ambient wave (`LF_WAVE_ID`).
    Status {
        /// Wave name (default: the ambient wave)
        wave: Option<String>,
        /// Read a dated chapter snapshot instead of current execution
        #[arg(long)]
        chapter: Option<String>,
        /// Emit the status snapshot as JSON
        #[arg(long)]
        json: bool,
        /// Refresh planning from Linear before reading
        #[arg(long, conflicts_with = "no_sync")]
        sync: bool,
        /// Read cached planning
        #[arg(long = "no-sync")]
        no_sync: bool,
    },
    /// Connect a Wave to its Initiative and the repository's Team (Task prefix)
    Connect {
        /// Wave name (auto-detected if omitted)
        wave: Option<String>,
        /// Wave name (flag form; same as positional wave)
        #[arg(short = 'w', long = "wave", conflicts_with_all = ["wave", "all"])]
        wave_flag: Option<String>,
        /// Recursively initialize every Wave under wave/
        #[arg(long, conflicts_with_all = ["wave", "wave_flag"])]
        all: bool,
        /// Repository Team key = Task prefix (e.g. LOO). Defaults from the repository name.
        #[arg(long = "team-key")]
        team_key: Option<String>,
        /// Repository Team display name. Defaults to the repository name.
        #[arg(long = "team-name")]
        team_name: Option<String>,
    },
    /// Refresh shared planning from Linear
    Sync {
        wave: Option<String>,
        #[arg(short = 'w', long = "wave", conflicts_with_all = ["wave", "all"])]
        wave_flag: Option<String>,
        #[arg(long, conflicts_with_all = ["wave", "wave_flag"])]
        all: bool,
    },
    /// Rename the provider Initiative
    Rename {
        wave: String,
        #[arg(long)]
        title: String,
    },
    /// Forget an empty Wave registration, preserving authored files
    Forget {
        name: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Place a Wave on a Home
    Place {
        name: String,
        home_id: crate::durable::HomeId,
        #[arg(long)]
        json: bool,
    },
    /// Rename or rehome a stopped Wave
    Relocate {
        wave: String,
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Retire the Wave, retaining history
    Retire {
        name: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },

    /// Replace the plan, carry started Tasks, and retire unopened backlog
    NewChapter {
        #[arg(short = 'w', long)]
        wave: Option<String>,
        #[arg(long)]
        chapter: String,
        /// Fresh chapter content as JSON: metric_targets, flows, and krs
        #[arg(long)]
        plan: Option<std::path::PathBuf>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// List chapter boundary receipts
    History {
        #[arg(short = 'w', long)]
        wave: Option<String>,
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

#[derive(Subcommand, Debug)]
pub enum TaskCommand {
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
    /// Start or continue a Task through its saved Flow
    Run {
        issue: String,
        #[arg(long)]
        name: Option<String>,
        /// Select a Flow for this Task worker; defaults to the chapter recommendation
        #[arg(long, value_name = "FLOW")]
        flow: Option<String>,
        /// Fork this Task's worktree from another Task's active PR
        #[arg(long = "stack-on", value_name = "PARENT_TASK")]
        stack_on: Option<String>,
        #[arg(long)]
        directive: Option<String>,
        /// Explain what changed after an execution blocker
        #[arg(long)]
        reason: Option<String>,
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
    /// List files changed from this Task's recorded base commit
    Changes {
        issue: String,
        #[arg(long, default_value = "parent")]
        base: String,
        #[arg(long)]
        json: bool,
    },
    /// Show this Task's patch, optionally limited to one changed file
    Diff {
        issue: String,
        path: Option<String>,
        #[arg(long, default_value = "parent")]
        base: String,
        /// Compare a UTF-8 draft read from stdin without writing the worktree
        #[arg(long, requires = "path")]
        draft: bool,
        #[arg(long)]
        json: bool,
    },
    /// Read one file from this Task's worktree
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
    /// Delete a Task from Linear and reconcile its local record
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
    /// Read the comment thread, or append direction without starting execution
    Comment {
        issue: String,
        message: Option<String>,
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
    /// defaults to the chapter's currently recommended Flow
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
    /// planning references against this binary; mutates nothing and exits
    /// non-zero on refusal so a caller can gate on it.
    #[command(hide = true)]
    Preflight {
        /// Emit the structured PromotionPreview as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Validate this exact local candidate against one receipt-selected store.
    #[command(hide = true)]
    LocalPreflight {
        #[arg(long)]
        store: PathBuf,
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
        /// Promote this exact unpublished local lf into a disposable installed Home.
        #[arg(long)]
        from_build: Option<PathBuf>,
        /// Candidate delegated to the receipt-pinned active coordinator.
        #[arg(long, hide = true)]
        coordinated_build: Option<PathBuf>,
        /// Abandon an incompatible disposable Home and fork published data again.
        #[arg(long, requires = "from_build")]
        fresh: bool,
        /// Reuse a retained development installation and its existing Home data.
        #[arg(long, requires = "from_build", conflicts_with = "fresh")]
        reuse_home: Option<String>,
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
    /// Show CI status for current branch
    Checks {
        #[arg(short = 'w', long = "watch")]
        watch: bool,
        #[arg(short = 'l', long = "logs")]
        logs: bool,
    },

    /// Show current branch's PR state
    Status,
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
        #[arg(short = 'm', long = "model", short_alias = 'M')]
        model: Option<String>,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "body")]
        body: Option<String>,
    },
    /// Push and create or update a draft PR, then open its GitHub page.
    /// Existing ready PRs stay ready; opening a draft does not publish it.
    Open {
        #[arg(short = 'm', long = "model", short_alias = 'M')]
        model: Option<String>,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "body")]
        body: Option<String>,
    },
    /// Prepare a PR to land: rebase, clear scratch, mark ready, and assign it
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
    /// Arm and watch a PR through CI repair and authoritative merge.
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
    /// Install or replace a scheduled lf invocation
    Add {
        /// Wave name passed to `lf <flow> --wave <wave>` (ambient if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        /// Flow or skill name to run
        #[arg(long = "flow")]
        flow: String,
        /// Fixed-daily cron expression, or the `daily` alias
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
        /// Wave whose GOAL.md `crons:` drive the installed jobs
        #[arg(short = 'w', long = "wave")]
        wave: String,
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
pub enum RepoCommand {
    /// Reconcile linked Waves to the repository's Linear Team
    Reteam {
        #[arg(long)]
        apply: bool,
    },
}

/// Inspect and observe durable Homes.
#[derive(Debug, Subcommand)]
pub enum HomeCommand {
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
pub enum AuthCommand {
    /// Inspect cached credentials and subscription windows; verify explicitly
    Status {
        provider: Option<String>,
        #[arg(long)]
        verify: bool,
        #[arg(long)]
        details: bool,
        #[arg(long)]
        json: bool,
    },
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
    /// Configure and inspect managed account routing
    Route {
        #[command(subcommand)]
        cmd: RouteCommand,
    },
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
    /// Explain configured and automatic account selection
    Show {
        #[arg(long, conflicts_with = "default")]
        repo: Option<String>,
        #[arg(long)]
        default: bool,
        #[arg(long)]
        json: bool,
    },
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
    /// Create a low-level sibling worktree
    Create {
        /// Worktree name
        name: String,
        /// Print the placement plan without creating a worktree
        #[arg(long)]
        plan: bool,
    },
    /// Switch to a worktree by name, identity leaf, or full branch
    Switch {
        /// Worktree name or full branch name to switch to
        name: String,
    },
    /// List worktrees (read-only; reflects the last-synced main)
    List {
        #[arg(long)]
        format: Option<String>,
        #[arg(long)]
        full: bool,
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
    /// Remove a worktree
    #[command(alias = "rm")]
    Remove {
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
        for verb in ["enable", "disable", "serve"] {
            assert!(Cli::try_parse_from(["lf", "wave", verb, "product"]).is_err());
        }
        for removed in [
            "start",
            "stop",
            "pause",
            "resume",
            "chat",
            "reply",
            "__resident",
        ] {
            assert!(command.find_subcommand(removed).is_none());
        }
        assert!(Cli::try_parse_from(["lf", "repo", "webhook", "serve"]).is_err());
        assert!(Cli::try_parse_from(["lf", "wave", "serve", "product"]).is_err());
        for args in [
            vec!["lf", "list"],
            vec!["lf", "wave", "list", "--json"],
            vec!["lf", "pr", "checks"],
            vec!["lf", "wave", "sync", "product"],
            vec!["lf", "wave", "sync", "--all"],
            vec!["lf", "wave", "rename", "product", "--title", "Product"],
            vec![
                "lf",
                "wave",
                "place",
                "product",
                "home_00000000000000000000000000000001",
            ],
            vec!["lf", "discord", "serve", "product"],
            vec!["lf", "doctor", "--planning", "--json"],
            vec!["lf", "wave", "status", "product", "--sync"],
            vec!["lf", "wave", "status", "product", "--no-sync"],
        ] {
            assert!(Cli::try_parse_from(args.clone()).is_ok(), "{args:?}");
        }
        for verb in [
            "abandon", "recover", "enable", "disable", "prepare", "resume", "advance",
        ] {
            assert!(Cli::try_parse_from(["lf", "task", verb, "LOO-1"]).is_err());
        }
        assert!(Cli::try_parse_from(["lf", "wave", "status", "--sync", "--no-sync"]).is_err());
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
        let Some(Commands::Screenshot { screenshot }) = cli.command else {
            panic!("expected screenshot command");
        };
        assert_eq!(screenshot.source, "page.html");
        assert_eq!(screenshot.output, PathBuf::from("capture.png"));
        assert_eq!((screenshot.width, screenshot.height), (390, 844));
        assert!(Cli::try_parse_from(["lf", "screenshot", "page.html"]).is_err());
    }

    #[test]
    fn install_exposes_refresh_and_schedule_but_hides_transaction_commands() {
        let mut command = Cli::command();
        assert!(command.render_long_help().to_string().contains("install"));
        let help = command
            .find_subcommand_mut("install")
            .unwrap()
            .render_long_help()
            .to_string();
        assert!(help.contains("schedule"));
        assert!(!help.contains("preflight"));
        assert!(matches!(
            Cli::try_parse_from(["lf", "install"]).unwrap().command,
            Some(Commands::Install { cmd: None })
        ));
        assert!(Cli::try_parse_from(["lf", "install", "schedule"]).is_ok());
        assert!(Cli::try_parse_from(["lf", "install", "status"]).is_err());
        assert!(Cli::try_parse_from(["lf", "install", "preflight"]).is_ok());
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
        let preview = Cli::try_parse_from([
            "lf",
            "wave",
            "new-chapter",
            "--wave",
            "product",
            "--chapter",
            "two",
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
            "--as",
            "wave:product",
            ":",
            "Which KR matters?",
        ])
        .expect("parse bound inline prompt");

        assert_eq!(cli.as_work.as_deref(), Some("wave:product"));
        assert!(matches!(
            cli.command,
            Some(Commands::Inline { prompt }) if prompt == vec!["Which KR matters?"]
        ));
    }

    #[test]
    fn ci_report_accepts_machine_wide_filters() {
        let cli = Cli::try_parse_from([
            "lf",
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
            Some(Commands::Ci {
                since,
                wave: Some(wave),
                repo: Some(repo),
                json: true,
            }) if since == "24h" && wave == "infrastructure" && repo == "loopflowstudio/loopflow"
        ));
    }

    #[test]
    fn activity_accepts_composed_work_filters() {
        let cli = Cli::try_parse_from([
            "lf",
            "activity",
            "--since",
            "24h",
            "--limit",
            "100",
            "--wave",
            "live",
            "--project",
            "control-room",
            "--task",
            "W2-140",
            "--json",
        ])
        .expect("parse Activity query");
        assert!(matches!(
            cli.command,
            Some(Commands::Activity {
                since,
                limit: 100,
                wave: Some(wave),
                project: Some(project),
                task: Some(task),
                json: true,
            }) if since == "24h"
                && wave == "live"
                && project == "control-room"
                && task == "W2-140"
        ));
    }

    #[test]
    fn runs_resume_requires_one_run_and_excludes_record_output() {
        let cli = Cli::try_parse_from(["lf", "runs", "abc123", "--resume"])
            .expect("parse provider session resume");
        assert!(matches!(
            cli.command,
            Some(Commands::Runs {
                run: Some(run),
                parent: None,
                resume: true,
                events: false,
                final_answer: false,
                json: false,
                ..
            }) if run == "abc123"
        ));
        assert!(Cli::try_parse_from(["lf", "runs", "--resume"]).is_err());
        assert!(Cli::try_parse_from(["lf", "runs", "abc123", "--resume", "--events"]).is_err());
    }

    #[test]
    fn runs_exposes_direct_children_and_final_answers() {
        let children = Cli::try_parse_from(["lf", "runs", "--parent", "abc123", "--json"])
            .expect("parse direct child query");
        assert!(matches!(
            children.command,
            Some(Commands::Runs {
                run: None,
                parent: Some(parent),
                json: true,
                ..
            }) if parent == "abc123"
        ));

        let final_answer = Cli::try_parse_from(["lf", "runs", "abc123", "--final"])
            .expect("parse final answer read");
        assert!(matches!(
            final_answer.command,
            Some(Commands::Runs {
                run: Some(run),
                final_answer: true,
                ..
            }) if run == "abc123"
        ));
    }

    #[test]
    fn auth_has_six_leaves_and_rejects_retired_paths() {
        let command = Cli::command();
        let auth = command.find_subcommand("auth").unwrap();
        for name in ["status", "connect", "disconnect", "set", "route"] {
            assert!(auth.find_subcommand(name).is_some());
        }
        for args in [
            vec!["auth", "accounts"],
            vec!["auth", "import", "claude"],
            vec!["auth", "configure", "codex"],
            vec!["auth", "reset", "claude", "a"],
            vec!["auth", "access", "set"],
            vec!["auth", "linear"],
        ] {
            assert!(Cli::try_parse_from(std::iter::once("lf").chain(args)).is_err());
        }
        assert!(command.find_subcommand("profile").is_none());
        assert!(command.find_subcommand("route").is_none());
        assert!(
            Cli::try_parse_from(["lf", "auth", "route", "set", "claude", "a", "--default"]).is_ok()
        );
        assert!(Cli::try_parse_from([
            "lf",
            "auth",
            "route",
            "set",
            "claude",
            "a",
            "--default",
            "--repo",
            "a/b"
        ])
        .is_err());
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
            Some(Commands::Ssh { origin_account, lf_args, .. })
                if origin_account == vec!["reserve"]
                    && lf_args == vec!["task", "pursue"]
        ));

        let after_host = Cli::try_parse_from([
            "lf",
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
            Some(Commands::Ssh { lf_args, .. })
                if lf_args == vec!["--account", "reserve", "task", "pursue"]
        ));
    }

    #[test]
    fn auth_set_accepts_ordered_service_profiles() {
        let cli = Cli::try_parse_from([
            "lf",
            "auth",
            "set",
            "linear",
            "--chrome-profile",
            "Work",
            "--chrome-profile",
            "Personal",
        ])
        .unwrap();
        assert!(matches!(cli.command, Some(Commands::Auth {
            cmd: AuthCommand::Set { email: None, chrome_profile, .. }
        }) if chrome_profile == ["Work", "Personal"]));
        assert!(Cli::try_parse_from(["lf", "auth", "set", "linear", "--clear-cooldown"]).is_err());
    }

    #[test]
    fn auth_connect_addresses_an_account_and_optional_bootstrap_venue() {
        let cli = Cli::try_parse_from([
            "lf",
            "auth",
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
            Some(Commands::Auth {
                cmd: AuthCommand::Connect {
                    provider,
                    email: Some(email),
                    chrome_profile: Some(chrome_profile),
                    ..
                }
            }) if provider == "claude"
                && email == "operator@"
                && chrome_profile == "Profile 9"
        ));
    }

    #[test]
    fn service_auth_accepts_a_remembered_chrome_profile() {
        let cli = Cli::try_parse_from([
            "lf",
            "auth",
            "connect",
            "linear",
            "--chrome-profile",
            "Work",
        ])
        .unwrap();
        assert!(
            matches!(cli.command, Some(Commands::Auth { cmd: AuthCommand::Connect {
            provider, email: None, chrome_profile: Some(profile), ..
        } }) if provider == "linear" && profile == "Work")
        );
    }

    #[test]
    fn auth_connect_sources_are_exclusive() {
        assert!(Cli::try_parse_from([
            "lf",
            "auth",
            "connect",
            "claude",
            "a@example.com",
            "--import"
        ])
        .is_ok());
        assert!(Cli::try_parse_from(["lf", "auth", "connect", "codex", "--api-key"]).is_ok());
        for flags in [
            vec!["--import", "--api-key"],
            vec!["--import", "--chrome-profile", "Work"],
        ] {
            assert!(Cli::try_parse_from(
                ["lf", "auth", "connect", "claude", "a@example.com"]
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
            "auth",
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
            Some(Commands::Auth {
                cmd: AuthCommand::Set {
                    provider,
                    email: Some(email),
                    login_email: Some(login_email),
                    routing: Some(routing),
                    plan: Some(plan),
                    paid_through: Some(paid_through),
                    clear_plan: false,
                    clear_paid_through: false,
                    ..
                }
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
        let cli = Cli::try_parse_from(["lf", "wave", "connect", "pm"]).expect("parse");
        let Some(Commands::Wave {
            cmd:
                WaveCommand::Connect {
                    wave,
                    wave_flag,
                    all,
                    ..
                },
        }) = cli.command
        else {
            panic!("expected pm init command");
        };

        assert_eq!(wave.as_deref(), Some("pm"));
        assert_eq!(wave_flag, None);
        assert!(!all);
    }

    #[test]
    fn task_run_accepts_linear_identifier_and_json() {
        let cli = Cli::try_parse_from([
            "lf",
            "task",
            "run",
            "INF-123",
            "--name",
            "release-scoped-migrations",
            "--stack-on",
            "INF-122",
            "--json",
        ])
        .expect("parse task run");
        let Some(Commands::Task {
            cmd:
                TaskCommand::Run {
                    issue,
                    name,
                    stack_on,
                    json,
                    ..
                },
        }) = cli.command
        else {
            panic!("expected task run command");
        };
        assert_eq!(issue, "INF-123");
        assert_eq!(name.as_deref(), Some("release-scoped-migrations"));
        assert_eq!(stack_on.as_deref(), Some("INF-122"));
        assert!(json);
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
    fn task_run_accepts_flow_selection() {
        let cli = Cli::try_parse_from(["lf", "task", "run", "INF-123", "--flow", "incident"])
            .expect("parse task lifecycle overrides");
        let Some(Commands::Task {
            cmd: TaskCommand::Run { flow, .. },
        }) = cli.command
        else {
            panic!("expected task run command");
        };
        assert_eq!(flow.as_deref(), Some("incident"));
    }

    #[test]
    fn task_run_rejects_design_only_outcome() {
        assert!(Cli::try_parse_from(["lf", "task", "run", "INF-123", "--design-only"]).is_err());
    }

    #[test]
    fn task_run_rejects_retired_reviewer_flag() {
        assert!(
            Cli::try_parse_from(["lf", "task", "run", "INF-123", "--reviewer", "parent"]).is_err()
        );
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
    fn task_run_rejects_retired_headless_flag() {
        let error = Cli::try_parse_from(["lf", "task", "run", "INF-123", "--headless"])
            .expect_err("--headless must not remain as an alias");
        assert!(error
            .to_string()
            .contains("unexpected argument '--headless'"));
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
            Some(Commands::Pr {
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
    fn top_is_a_first_class_machine_dashboard() {
        let cli = Cli::try_parse_from(["lf", "top"]).expect("parse top");
        assert!(matches!(cli.command, Some(Commands::Top { json: false })));

        let cli = Cli::try_parse_from(["lf", "ps", "--json"]).expect("parse ps");
        assert!(matches!(cli.command, Some(Commands::Ps { json: true })));
        assert!(Cli::try_parse_from(["lf", "ps", "--sort", "tokens"]).is_err());

        let cli = Cli::try_parse_from(["lf", "prune", "--dry-run", "--json"])
            .expect("parse process prune");
        assert!(matches!(
            cli.command,
            Some(Commands::Prune {
                dry_run: true,
                json: true,
            })
        ));
    }

    #[test]
    fn usage_exposes_the_direct_run_window_and_work_drill() {
        let cli =
            Cli::try_parse_from(["lf", "usage", "--days", "7", "--task", "LOO-265", "--json"])
                .expect("parse direct usage");
        assert!(matches!(
            cli.command,
            Some(Commands::Usage {
                json: true,
                days: 7,
                wave: None,
                project: None,
                task: Some(task),
            })
                if task == "LOO-265"
        ));
        assert!(Cli::try_parse_from(["lf", "usage", "--refresh"]).is_err());
        assert!(Cli::try_parse_from(["lf", "usage", "--cached"]).is_err());
    }

    #[test]
    fn task_workspace_commands_address_the_task_then_optional_file() {
        let changes = Cli::try_parse_from(["lf", "task", "changes", "INF-123", "--json"])
            .expect("parse task changes");
        assert!(matches!(
            changes.command,
            Some(Commands::Task {
                cmd: TaskCommand::Changes { issue, json: true, .. }
            }) if issue == "INF-123"
        ));

        let diff =
            Cli::try_parse_from(["lf", "task", "diff", "INF-123", "src/parser.rs", "--json"])
                .expect("parse task diff");
        assert!(matches!(
            diff.command,
            Some(Commands::Task {
                cmd: TaskCommand::Diff {
                    issue,
                    path: Some(path),
                    json: true,
                    ..
                }
            }) if issue == "INF-123" && path == "src/parser.rs"
        ));

        let file =
            Cli::try_parse_from(["lf", "task", "file", "INF-123", "src/parser.rs", "--json"])
                .expect("parse task file");
        assert!(matches!(
            file.command,
            Some(Commands::Task {
                cmd: TaskCommand::File { issue, path, json: true, recoveries: false }
            }) if issue == "INF-123" && path == "src/parser.rs"
        ));
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
                },
        }) = steer.command
        else {
            panic!("expected task comment command");
        };
        assert_eq!(issue, "INF-123");
        assert_eq!(message.as_deref(), Some("take the smaller approach"));
        assert!(Cli::try_parse_from(["lf", "task", "comment", "INF-123"]).is_ok());
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
    fn task_run_accepts_blocker_feedback() {
        let task = Cli::try_parse_from([
            "lf",
            "task",
            "run",
            "W2-135",
            "--reason",
            "credential repaired",
            "--json",
        ])
        .expect("parse Task advancement retry");
        assert!(matches!(
            task.command,
            Some(Commands::Task {
                cmd: TaskCommand::Run {
                    issue,
                    reason: Some(reason),
                    json: true,
                    ..
                }
            }) if issue == "W2-135" && reason == "credential repaired"
        ));

        assert!(Cli::try_parse_from(["lf", "task", "run", "W2-135", "--model", "codex",]).is_err());
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
            vec!["lf", "task", "run", "LOO-1", "--session", "review"],
            vec!["lf", "task", "run", "LOO-1", "--summary", "approved"],
            vec!["lf", "session", "advance", "review", "approved"],
            vec!["lf", "session", "iterate", "review", "revise"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
        assert!(Cli::try_parse_from(["lf", "task", "run", "LOO-1"]).is_ok());
        assert!(
            Cli::try_parse_from(["lf", "flow", "decide", "iterate", "revise implementation"])
                .is_ok()
        );
        assert!(Cli::try_parse_from(["lf", "session", "complete", "review"]).is_ok());
    }

    #[test]
    fn cli_separates_ask_completion_from_flow_decisions() {
        let ask = Cli::try_parse_from(["lf", "ask", "Review", "this", "branch"])
            .expect("parse human Ask");
        assert!(matches!(
            ask.command,
            Some(Commands::Ask { ask }) if ask.question == ["Review", "this", "branch"] && ask.skill.is_none()
        ));

        let ask =
            Cli::try_parse_from(["lf", "ask", "--skill", "unblock", "Resolve", "this blocker"])
                .expect("parse skill-selected Ask");
        assert!(matches!(
            ask.command,
            Some(Commands::Ask { ask }) if ask.skill.as_deref() == Some("unblock")
                && ask.question == ["Resolve", "this blocker"]
        ));

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

        let open = Cli::try_parse_from(["lf", "session", "open", "run_123", "--replace"])
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
        assert!(
            Cli::try_parse_from(["lf", "session", "open", "run_123", "--replace", "--try",])
                .is_err()
        );

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
    fn rebase_manual_recovery_modes_are_explicit_and_exclusive() {
        let manual = Cli::try_parse_from(["lf", "rebase", "--manual", "origin/main"])
            .expect("parse manual rebase");
        assert!(matches!(
            manual.command,
            Some(Commands::Rebase {
                manual: true,
                continue_rebase: false,
                abort: false,
                onto: Some(ref onto),
                ..
            }) if onto == "origin/main"
        ));

        assert!(Cli::try_parse_from(["lf", "rebase", "--continue", "--abort"]).is_err());
        assert!(Cli::try_parse_from(["lf", "rebase", "--plan", "--manual"]).is_err());
    }

    #[test]
    fn install_promote_requires_a_local_build_for_fresh_forks() {
        let cli = Cli::try_parse_from([
            "lf",
            "install",
            "promote",
            "--from-build",
            "/tmp/lf",
            "--fresh",
            "--cli-target",
            "/tmp/bin/lf",
        ])
        .expect("parse local promotion");
        assert!(matches!(
            cli.command,
            Some(Commands::Install {
                cmd: Some(InstallCommand::Promote {
                    from_build: Some(_),
                    fresh: true,
                    ..
                })
            })
        ));
        assert!(Cli::try_parse_from([
            "lf",
            "install",
            "promote",
            "--fresh",
            "--cli-target",
            "/tmp/bin/lf",
        ])
        .is_err());
    }

    #[test]
    fn pm_init_accepts_all_flag() {
        let cli = Cli::try_parse_from(["lf", "wave", "connect", "--all"]).expect("parse");
        let Some(Commands::Wave {
            cmd:
                WaveCommand::Connect {
                    wave,
                    wave_flag,
                    all,
                    team_key,
                    team_name,
                },
        }) = cli.command
        else {
            panic!("expected pm init command");
        };

        assert_eq!(wave, None);
        assert_eq!(wave_flag, None);
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
            "--wave",
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
    /// Poll a configured channel and post each Run's final answer
    Serve { wave: String },
}
