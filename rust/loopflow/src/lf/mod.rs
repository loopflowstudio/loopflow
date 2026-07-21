use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

pub mod commands;
pub mod discovery;
pub mod output;

#[derive(Parser, Debug, Default)]
#[command(name = "lf")]
#[command(about = "Open Loopflow or run its CLI")]
#[command(version = crate::build_info::BUILD_VERSION)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// List available skills and flows
    #[arg(short, long)]
    pub list: bool,

    /// Direction(s) to apply (repeatable or comma-separated)
    #[arg(
        short = 'd',
        long = "direction",
        value_delimiter = ',',
        short_alias = 'D'
    )]
    pub direction: Vec<String>,

    /// Exclude config default directions
    #[arg(long = "no-direction")]
    pub no_direction: bool,

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

    /// Wave name for wave/ scoping
    #[arg(short = 'w', long = "wave", short_alias = 'W')]
    pub wave: Option<String>,

    /// Exclude loopflow operating guidance
    #[arg(long = "no-loopflow")]
    pub no_loopflow: bool,
}

impl Cli {
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
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run an inline prompt
    #[command(name = ":")]
    Inline {
        #[arg(trailing_var_arg = true)]
        prompt: Vec<String>,
    },
    /// Open or focus Loopflow.app
    Desktop,
    /// Ask the current Turn's parent and block until its durable Answer arrives
    Ask {
        /// Question text, or `wait [<ask-id>]` to resume an existing exchange
        #[arg(trailing_var_arg = true, value_name = "QUESTION|wait [ASK_ID]")]
        args: Vec<String>,
    },
    /// Authorize global lf promotion against the shared migration frontier
    Install {
        #[command(subcommand)]
        cmd: InstallCommand,
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
    /// Chrome access venues used during provider login ceremonies
    Profile {
        #[command(subcommand)]
        cmd: ProfileCommand,
    },
    /// Route providers through ordered managed accounts
    Route {
        #[command(subcommand)]
        cmd: RouteCommand,
    },
    /// Release operations (run, check, notes, bump, tag, status)
    Release {
        #[command(subcommand)]
        cmd: ReleaseCommand,
    },
    /// Linear Initiatives, Projects, and tasks for waves
    Pm {
        #[command(subcommand)]
        cmd: PmCommand,
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
    /// Local launchd jobs that run lf commands on a schedule
    Cron {
        #[command(subcommand)]
        cmd: CronCommand,
    },
    /// Run a Wave's listener, thread, and residency. Steerable.
    ///
    /// By convention this is the wave loop's entrypoint — a served mind is one
    /// you can chat with while it runs.
    Wave {
        /// Wave name
        name: String,
        /// Take over even if another live Wave is registered
        #[arg(long)]
        force: bool,
    },
    /// Start one or more Waves on this machine.
    Start {
        /// Wave names. With none, starts eligible Waves in the current repo.
        waves: Vec<String>,
        /// Internal identity bindings carried by an explicit Home SSH hop.
        #[arg(long = "wave-id", value_name = "NAME=ID", hide = true)]
        wave_ids: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Stop a served wave gracefully
    Stop {
        /// Wave name
        name: String,
    },
    /// Pause new turns while keeping the Wave listener available.
    Pause {
        /// Wave name
        name: String,
        /// Emit the resulting turn intent as JSON
        #[arg(long)]
        json: bool,
    },
    /// Resume new turns for a paused Wave.
    Resume {
        /// Wave name
        name: String,
        /// Emit the resulting turn intent as JSON
        #[arg(long)]
        json: bool,
    },
    /// Internal: the resident body a listener spawns for its own wave. Never
    /// booted by hand — `lf wave` owns the listener half.
    #[command(name = "__resident", hide = true)]
    Resident {
        /// Wave name
        name: String,
    },
    /// Internal resident primitive: execute one expanded top-level flow step.
    #[command(name = "__flow-step", hide = true)]
    FlowStep {
        flow: String,
        index: usize,
        seed: String,
    },
    /// Project lifecycle operations
    Project {
        #[command(subcommand)]
        cmd: ProjectCommand,
    },
    /// Linear-backed Task lifecycle
    Task {
        #[command(subcommand)]
        cmd: TaskCommand,
    },
    /// Inspect, attach, and hand back provider or opaque AgentInvocations
    Invocation {
        #[command(subcommand)]
        cmd: InvocationCommand,
    },
    /// Inspect and control stable Wave, Project, or Task Work
    Work {
        #[command(subcommand)]
        cmd: WorkCommand,
    },
    /// Internal: run a Project or Task body holding the ambient Run lease
    #[command(name = "__work", hide = true)]
    WorkRunner {
        #[arg(value_parser = ["project", "task"])]
        kind: String,
        work_id: String,
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
    /// Show subscription state per account and token spend by repo/provider
    Usage {
        /// Emit one additive row per provider-measured Turn as JSON
        #[arg(long)]
        json: bool,
        /// Spend window, in days
        #[arg(long, default_value_t = 30)]
        days: u32,
        /// Poll every account now, even the freshly observed ones
        #[arg(long, short = 'r')]
        refresh: bool,
        /// Skip polling entirely; show only stored observations
        #[arg(long)]
        cached: bool,
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
        /// Rank siblings by cumulative completed tokens or five-minute rate
        #[arg(long, value_enum, default_value_t)]
        sort: commands::top::ActivitySort,
    },
    /// Refresh live Loopflow call trees on a terminal; print once when redirected
    Top {
        /// Emit one versioned activity snapshot as JSON
        #[arg(long)]
        json: bool,
        /// Rank siblings by cumulative completed tokens or five-minute rate
        #[arg(long, value_enum, default_value = "rate-5m")]
        sort: commands::top::ActivitySort,
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
    /// Inspect supplied agent context and its contributing assets
    Context {
        /// Window in days
        #[arg(long, default_value_t = 30)]
        days: u32,
        /// Inclusive window start as a Unix timestamp (overrides --days)
        #[arg(long)]
        started_after: Option<i64>,
        /// Exclusive window end as a Unix timestamp
        #[arg(long)]
        started_before: Option<i64>,
        /// Filter by wave
        #[arg(long)]
        wave: Vec<String>,
        /// Filter by attributed Linear Project slug
        #[arg(long)]
        project: Vec<String>,
        /// Filter by attributed Linear Task identifier
        #[arg(long)]
        task: Vec<String>,
        /// Filter by absolute main-repo path
        #[arg(long)]
        repo: Vec<String>,
        /// Filter by flow
        #[arg(long)]
        flow: Vec<String>,
        /// Filter by skill
        #[arg(long)]
        skill: Vec<String>,
        /// Filter by provider
        #[arg(long)]
        provider: Vec<String>,
        /// Filter by model
        #[arg(long)]
        model: Vec<String>,
        /// Filter by invocation surface
        #[arg(long)]
        surface: Vec<String>,
        /// Filter by invocation outcome
        #[arg(long)]
        outcome: Vec<String>,
        /// Filter by capture state
        #[arg(long)]
        capture_state: Vec<String>,
        /// Include only invocations with observed steering turns
        #[arg(long)]
        steered_only: bool,
        /// Include only invocations containing a current file instruction revision
        #[arg(long)]
        current_revision_only: bool,
        /// Emit the Context Lab snapshot as JSON
        #[arg(long)]
        json: bool,
    },
    /// Audit the local run ledger: continuity, vocabulary, attribution, identity, lineage, coverage
    Doctor {
        /// Emit the audit as JSON
        #[arg(long)]
        json: bool,
    },
    /// List every wave in the registry (running and stopped), marking which
    /// have a live server. Local-only query over the shared ledger.
    Ls {
        /// Emit the wave snapshot as JSON (Loopflow's dashboard snapshot)
        #[arg(long)]
        json: bool,
    },
    /// Show one wave's Project/Task hierarchy, runs, attention, and live loop
    /// state from the registry. Defaults to the ambient wave (`LF_WAVE_ID`).
    Status {
        /// Wave name (default: the ambient wave)
        wave: Option<String>,
        /// Emit the status snapshot as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show the machine-wide roadmap: every open Task across every Wave, joined
    /// to live evidence and bucketed into Now / Needs attention / Available /
    /// Later. Global by default; `--wave` scopes it. Local-only, deterministic.
    Roadmap {
        /// Scope to one Wave (default: every Wave on this machine)
        #[arg(long)]
        wave: Option<String>,
        /// Emit the roadmap snapshot as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show recent agent-backed skill runs with context and token evidence
    Runs {
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
        #[command(subcommand)]
        cmd: Option<RunsCommand>,
    },
    /// Show recent lf process executions (all repos, local-only)
    Execs {
        /// Emit the process history as JSON
        #[arg(long)]
        json: bool,
    },
    /// Reconstruct one process tree from an exec or trace address
    Trace {
        /// Exec id from `lf execs` or trace id from Context Lab
        exec_id: String,
        /// Emit the process tree as JSON
        #[arg(long)]
        json: bool,
        /// Include exact prompt and normalized conversation bodies for one address
        #[arg(long, requires = "json", conflicts_with = "events")]
        content: bool,
        /// Render the normalized recorded conversation
        #[arg(long, conflicts_with = "json")]
        events: bool,
        /// Stream stored event objects as JSONL
        #[arg(long, requires = "events")]
        jsonl: bool,
        /// Select one AgentInvocation by id prefix
        #[arg(long)]
        invocation: Option<String>,
        /// Select one turn by id prefix (with --content)
        #[arg(long, requires = "content")]
        turn: Option<String>,
    },
    /// Converse with a served mind's thread; --follow replays it and --steer
    /// reaches the live body.
    Chat {
        /// Message text (reads stdin when omitted unless --follow or --history)
        #[arg(trailing_var_arg = true)]
        text: Vec<String>,
        /// Replay and follow the thread while typed lines post into it.
        #[arg(long, conflicts_with_all = ["text", "history", "json", "limit"])]
        follow: bool,
        /// Inject into a live steer-capable turn; otherwise queue.
        #[arg(long, conflicts_with_all = ["parent", "history", "json", "limit"])]
        steer: bool,
        /// Read the latest durable turns without requiring a live listener.
        #[arg(long, conflicts_with = "text")]
        history: bool,
        /// Emit the durable history snapshot as JSON.
        #[arg(long, requires = "history")]
        json: bool,
        /// Maximum durable turns to return (default: 12).
        #[arg(long, requires = "history")]
        limit: Option<usize>,
        /// Select one immutable conversation epoch.
        #[arg(long, requires = "history")]
        epoch: Option<String>,
        #[command(flatten)]
        target: WaveTargetArgs,
    },
    // Same reservation for the retired `lf op` namespace, which held every
    // operation before the runtime collapsed to waves, projects, and tasks.
    // Without it, `lf op land` reports a missing skill named `op` instead of
    // naming the command that replaced it.
    #[command(
        name = "op",
        hide = true,
        about = "Removed; the operations are top-level (`lf pr`, `lf rebase`, `lf wt`, `lf pm`)",
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
    /// Run a flow by name — the explicit form for names that collide with a
    /// built-in command
    Flow {
        /// Flow name
        name: String,
        /// Message for the flow
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Run a skill (skill) by name — the explicit form
    Skill {
        /// Skill name
        name: String,
        /// Message for the skill
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// External: skill/flow name (when no subcommand matches)
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand, Debug)]
pub enum RunsCommand {
    /// Tombstone terminal captures whose conversation artifacts are gone, and
    /// finalize orphaned `capturing` invocations. Dry-run by default; `--apply`
    /// writes. A red `lf doctor` capture check means un-acknowledged loss —
    /// this is the explicit acknowledgment that turns historical loss green
    /// while leaving fresh loss red.
    Reconcile {
        /// Apply the tombstone/finalize transitions (default: dry-run report)
        #[arg(long)]
        apply: bool,
        /// Reconcile recent missing captures too (default: age-guard <48h as
        /// candidates to investigate, not tombstone)
        #[arg(long)]
        all: bool,
        /// Emit the reconciliation report as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum InvocationCommand {
    /// List AgentInvocations supervised by Runs
    List {
        /// Include only invocations whose supervising Run may still be live
        #[arg(long)]
        active: bool,
        #[arg(long)]
        json: bool,
    },
    /// Show one AgentInvocation and its generic attach route
    Status {
        invocation_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Return the generic attach descriptor without changing Run state
    Attach {
        invocation_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Record explicit terminal evidence for an opaque invocation boundary
    Handback {
        invocation_id: String,
        #[arg(long, value_parser = ["succeeded", "failed", "interrupted", "unknown"])]
        outcome: String,
        #[arg(long)]
        json: bool,
    },
    /// Exec the AgentInvocation's generic attach route
    Present { invocation_id: String },
}

#[derive(Subcommand, Debug)]
pub enum WorkCommand {
    /// Show current Epoch, Basis, Run, and Wait projection
    Status {
        #[arg(value_parser = ["wave", "project", "task"])]
        kind: String,
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Record Wave Work's Home. Refuses while the Work has a live Run.
    Place {
        #[arg(value_parser = ["wave"])]
        kind: String,
        id: String,
        home_id: crate::durable::HomeId,
        #[arg(long)]
        json: bool,
    },
    /// Append authored direction through User or active parent Run authority
    Steer {
        #[arg(value_parser = ["wave", "project", "task"])]
        kind: String,
        id: String,
        message: String,
        #[arg(long)]
        json: bool,
    },
    /// List pending Asks routed to the User or this parent Work
    Asks {
        #[arg(value_parser = ["wave", "project", "task"])]
        kind: Option<String>,
        id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Answer one exact Ask; the first authorized Answer wins
    Answer {
        ask_id: crate::durable::AskId,
        text: String,
        #[arg(long)]
        json: bool,
    },
    /// Interrupt the current Turn or opaque Invocation boundary
    Interrupt {
        #[arg(value_parser = ["wave", "project", "task"])]
        kind: String,
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Abandon the current Epoch from an authenticated User surface
    Abandon {
        #[arg(value_parser = ["wave", "project", "task"])]
        kind: String,
        id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
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
        "auth" | "commit" | "cron" | "doctor" | "pm" | "rebase" | "release" | "sync-skills"
        | "wt" => format!("use `lf {sub}`"),
        _ => "the operations are top-level now — see `lf --help`".to_string(),
    };
    Err(format!("`lf op {sub}` was removed; {hint}"))
}

/// Wave targeting for `lf chat`: default is the invoking context's wave
/// (`LF_WAVE_ID` env, else the worktree name).
#[derive(Args, Debug, Clone, Default)]
pub struct WaveTargetArgs {
    /// Target wave by name
    #[arg(short = 'w', long = "wave", conflicts_with = "parent")]
    pub wave: Option<String>,
    /// Target the invoking wave's parent (escalation up the wave tree)
    #[arg(long)]
    pub parent: bool,
}

#[derive(Subcommand, Debug)]
pub enum ProjectCommand {
    /// Create a Linear Project first, then start its durable Project
    Start {
        title: String,
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        #[arg(long)]
        directive: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Start or resume the current Work for an existing Linear Project
    Run {
        /// Linear Project UUID or unique slug
        project_id: String,
        #[arg(long)]
        directive: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show durable Project state and reconcile process liveness
    Status {
        /// Linear Project UUID, unique slug, or historical Project id
        project_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Redirect Project Work now, relaunching its provider when needed
    Steer {
        project_id: String,
        message: String,
        #[arg(long)]
        json: bool,
    },
    /// Interrupt the active Project turn
    Interrupt {
        project_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Wait without polling an LM
    Wait {
        project_id: String,
        #[arg(long, default_value = "terminal", value_parser = ["waiting", "terminal"])]
        until: String,
        #[arg(long)]
        timeout: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Resume the same Project, optionally handing its next body to another agent
    Resume {
        project_id: String,
        #[arg(long)]
        model: Option<String>,
        #[arg(long, requires = "model")]
        reason: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Attach to the writable Project control terminal
    Attach { project_id: String },
    /// End Project pursuit without deleting its durable history
    Abandon {
        project_id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    /// Promote a project into a resident child wave through the authored flow
    Promote {
        /// Linear Project slug under the parent wave
        slug: String,
        /// Parent wave (default: ambient wave)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum TaskCommand {
    /// Ensure its Project, then start or return the existing Linear task
    Run {
        issue: String,
        #[arg(long)]
        name: Option<String>,
        /// Override the Project's first flow for a new Task
        #[arg(long, value_name = "FLOW")]
        first: Option<String>,
        /// Override the Project's loop flow for a new Task
        #[arg(long = "loop", value_name = "FLOW")]
        loop_: Option<String>,
        /// Override the Project's finally flow for a new Task
        #[arg(long, value_name = "FLOW")]
        finally: Option<String>,
        /// Fork this Task's worktree from another Task's active PR
        #[arg(long = "stack-on", value_name = "PARENT_TASK")]
        stack_on: Option<String>,
        #[arg(long)]
        directive: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Create a Linear task, ensure its Project, then start its Task
    Start {
        /// Required Linear Project id or slug
        project_id: String,
        /// Task title; omitted when stdin supplies the report and first line
        title: Option<String>,
        #[arg(long)]
        name: Option<String>,
        /// Override the Project's first flow
        #[arg(long, value_name = "FLOW")]
        first: Option<String>,
        /// Override the Project's loop flow
        #[arg(long = "loop", value_name = "FLOW")]
        loop_: Option<String>,
        /// Override the Project's finally flow
        #[arg(long, value_name = "FLOW")]
        finally: Option<String>,
        /// Fork this Task's worktree from another Task's active PR
        #[arg(long = "stack-on", value_name = "PARENT_TASK")]
        stack_on: Option<String>,
        #[arg(long)]
        directive: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show durable state and reconcile process liveness
    Status {
        issue: String,
        #[arg(long)]
        json: bool,
    },
    /// List files changed from this Task's recorded base commit
    Changes {
        issue: String,
        #[arg(long)]
        json: bool,
    },
    /// Show this Task's patch, optionally limited to one changed file
    Diff {
        issue: String,
        path: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Read one file from this Task's worktree
    File {
        issue: String,
        path: String,
        #[arg(long)]
        json: bool,
    },
    /// Complete a Task without requiring another pull request
    Complete {
        issue: String,
        #[arg(long)]
        summary: String,
        #[arg(long)]
        json: bool,
    },
    /// Redirect the active provider turn, interrupting when live steer is unavailable
    Steer {
        issue: String,
        message: String,
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
    /// Resume the same Task, optionally handing its next body to another agent
    Resume {
        issue: String,
        #[arg(long)]
        model: Option<String>,
        #[arg(long, requires = "model")]
        reason: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Recover an abandoned Task as a linked successor on the same worktree
    Recover {
        issue: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Explicitly end a Task without merging
    Abandon {
        issue: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum InstallCommand {
    /// Preview whether this build may replace the global lf (read-only).
    /// Reads the shared store's migration frontier and live-body count against
    /// this binary's own registry; mutates nothing and exits non-zero on a
    /// refusal so a caller can gate on it.
    Preflight {
        /// Emit the structured PromotionPreview as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Promote this build to the global CLI: content-address it into ~/.lf/bin
    /// and atomically repoint the target symlink, under the exclusive promotion
    /// lock. Refuses — leaving every target unchanged — on incompatible or
    /// live-body evidence.
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
    /// Show current branch's PR state
    Status,
    /// After an out-of-band merge, rotate this Task to its next serial PR,
    /// carrying committed and uncommitted follow-up onto the new branch.
    Next {
        /// Name the next serial branch (defaults to the settled PR's next slug,
        /// then the sequence number).
        slug: Option<String>,
    },
    /// Publish a PR headlessly: push, create or refresh, print state + URL.
    /// Opens no review surface.
    Publish {
        #[arg(short = 'm', long = "model", short_alias = 'M')]
        model: Option<String>,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "body")]
        body: Option<String>,
    },
    /// Publish a PR, then open it for review (the GitHub page in the browser).
    /// The explicit human review action.
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
    /// Land a PR hands-off: rebase, clear scratch, and arm auto-merge
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
        /// Schedule expression. v0 supports `daily`.
        #[arg(long = "schedule", default_value = "daily")]
        schedule: String,
    },
    /// List installed loopflow cron jobs
    List,
    /// Reconcile installed launchd jobs to match a wave's declared `crons:`
    Sync {
        /// Wave whose GOAL.md `crons:` drive the installed jobs
        #[arg(short = 'w', long = "wave")]
        wave: String,
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
pub enum PmCommand {
    /// Connect a Wave to its Initiative and the repository's Team (Task prefix)
    Init {
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
    /// Read the wave's local Project and task snapshot
    Show {
        /// Wave name (auto-detected if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        /// Linear Project slug
        #[arg(short = 'p', long = "project")]
        project: Option<String>,
        /// Emit the task snapshot as JSON
        #[arg(long)]
        json: bool,
        /// Force a refresh from Linear before reading
        #[arg(long = "sync", conflicts_with = "no_sync")]
        sync: bool,
        /// Read the local cache only; never contact Linear
        #[arg(long = "no-sync")]
        no_sync: bool,
    },
    /// Show local PM status for linked waves
    Status {
        /// Wave name (all PM-enabled waves if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
    },
    /// Compare Linear with local wave bindings
    Doctor,
    /// Move every linked wave onto the repository's one Linear team
    Reteam {
        /// Execute the moves; without it, `reteam` only prints the plan (dry run)
        #[arg(long)]
        apply: bool,
    },
    /// Refresh the local PM snapshot from Linear
    Sync {
        /// Wave name (all linked waves if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        /// Compare without writing the SQLite snapshot
        #[arg(long = "plan")]
        plan: bool,
    },
    /// Rename the Linear Initiative backing a wave
    Rename {
        /// Wave name (auto-detected if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        /// Linear Initiative title
        #[arg(long = "title")]
        title: String,
    },
    /// Linear task operations
    Task {
        #[command(subcommand)]
        cmd: PmTaskCommand,
    },
    /// Linear Project operations
    Project {
        #[command(subcommand)]
        cmd: PmProjectCommand,
    },
    /// Linear webhook receiver: stream human edits into Tasks
    Webhook {
        #[command(subcommand)]
        cmd: PmWebhookCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum PmWebhookCommand {
    /// Run the receiver that turns Linear edits into Task direction. Reads the
    /// signing secret from LF_LINEAR_WEBHOOK_SECRET (source it from Doppler).
    Serve {
        /// Address to bind (a reverse proxy gives Linear the public HTTPS URL)
        #[arg(long, default_value = "127.0.0.1:8899")]
        addr: String,
    },
    /// Register the Issue/Comment webhook with Linear (one-time). Reads the
    /// signing secret from LF_LINEAR_WEBHOOK_SECRET.
    Register {
        /// Public HTTPS URL Linear will POST deliveries to
        #[arg(long)]
        url: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum PmProjectCommand {
    /// Create a Linear Project in the wave's Initiative
    Create {
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        #[arg(long = "title")]
        title: String,
        #[arg(long = "definition")]
        definition: String,
        /// Key result; repeat for each KR. Prefix with `[x] ` when it holds.
        #[arg(long = "kr", required = true)]
        krs: Vec<String>,
        /// Flow run once when each Task starts
        #[arg(long)]
        first: Option<String>,
        /// Flow repeated while each Task makes progress
        #[arg(long = "loop")]
        loop_: Option<String>,
        /// Flow run to gate, learn from, and land each Task
        #[arg(long)]
        finally: Option<String>,
    },
    /// Update a Linear Project's content or Task flows
    Update {
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        #[arg(short = 'p', long = "project")]
        project: String,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "definition")]
        definition: Option<String>,
        /// Replace KRs; repeat for each KR. Prefix with `[x] ` when it holds.
        #[arg(long = "kr")]
        krs: Vec<String>,
        /// Flow run once when each Task starts
        #[arg(long)]
        first: Option<String>,
        /// Flow repeated while each Task makes progress
        #[arg(long = "loop")]
        loop_: Option<String>,
        /// Flow run to gate, learn from, and land each Task
        #[arg(long)]
        finally: Option<String>,
    },
    /// Archive a Linear Project and refresh the wave snapshot
    Archive {
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        #[arg(short = 'p', long = "project")]
        project: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum PmTaskCommand {
    /// Create a Linear task
    Create {
        /// Wave name (auto-detected if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        /// Linear Project slug
        #[arg(short = 'p', long = "project")]
        project: String,
        /// Task title
        #[arg(long = "title")]
        title: String,
        /// Task notes/description
        #[arg(long = "notes")]
        notes: Option<String>,
    },
    /// Update a Linear task
    Update {
        /// Existing task id to edit
        #[arg(long = "id")]
        id: String,
        /// Wave name (auto-detected if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        /// Linear Project slug
        #[arg(short = 'p', long = "project")]
        project: Option<String>,
        /// Task title
        #[arg(long = "title")]
        title: Option<String>,
        /// Task notes/description
        #[arg(long = "notes")]
        notes: Option<String>,
    },
    /// Close a Linear task and optionally link the shipped PR
    Done {
        /// Existing task id to close
        #[arg(long = "id")]
        id: String,
        /// Wave name (auto-detected if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        /// PR URL to attach as a comment
        #[arg(long = "pr")]
        pr: Option<String>,
    },
    /// Move a task into a wave's Linear Project
    Move {
        /// Existing task id to move
        #[arg(long = "id")]
        id: String,
        /// Destination wave
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        /// Destination Linear Project slug
        #[arg(short = 'p', long = "project")]
        project: String,
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
    /// Probe a Wave's Home for liveness and the one contextual action.
    ///
    /// Prints the Home route, its state (unreachable/stopped/running/unknown)
    /// with the evidence, the attach endpoint when running, and the action to
    /// offer. `--json` emits the `HomeRuntimeDto` a UI consumes.
    Probe {
        /// Wave name; defaults to the ambient wave.
        wave: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Show authentication status for local credentials
    Status {
        /// Provider name (optional)
        provider: Option<String>,
    },
    /// Disconnect a provider from local lf credentials
    Disconnect {
        /// Provider name
        provider: String,
        /// Disconnect one managed OAuth login
        #[arg(long)]
        email: Option<String>,
    },
    /// Store an API key from the provider's environment variable
    Configure {
        /// Provider name
        provider: String,
    },
    /// Start a provider auth flow explicitly
    Connect {
        /// Provider name
        provider: String,
        /// Login email or an unambiguous prefix
        email: Option<String>,
        /// Bootstrap through this Chrome directory, name, or signed-in email
        #[arg(long, requires = "email")]
        chrome_profile: Option<String>,
    },
    /// Adopt an existing Claude login
    Import {
        provider: String,
        /// Verified login email
        #[arg(long)]
        email: String,
        /// Chrome profile directory, name, or signed-in email
        #[arg(long)]
        chrome_profile: Option<String>,
    },
    /// Manage the ordered Chrome venues that can log in an account
    Access {
        #[command(subcommand)]
        cmd: AuthAccessCommand,
    },
    /// List managed Claude and Codex OAuth accounts
    Accounts {
        /// Provider name (optional)
        provider: Option<String>,
        /// Ask each provider now instead of reporting only cached state
        #[arg(long)]
        verify: bool,
    },
    /// Record provider-specific account identity, routing, and billing state
    Set {
        provider: String,
        /// Login email or an unambiguous prefix
        email: String,
        #[arg(long)]
        login_email: Option<String>,
        /// automatic, explicit-only, or disabled
        #[arg(long)]
        routing: Option<String>,
        #[arg(long, conflicts_with = "clear_plan")]
        plan: Option<String>,
        #[arg(long)]
        clear_plan: bool,
        /// Last paid day, as YYYY-MM-DD
        #[arg(long, conflicts_with = "clear_paid_through")]
        paid_through: Option<String>,
        #[arg(long)]
        clear_paid_through: bool,
    },
    /// Clear observed utilization and cooldown for an account
    Reset {
        provider: String,
        /// Login email or an unambiguous prefix
        email: String,
    },
    /// External: provider name (so `lf auth linear` works)
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    /// Record a Chrome access venue
    Create {
        /// Chrome profile directory, name, or signed-in email on this host
        #[arg(long)]
        chrome_profile: String,
        /// Stable venue name; defaults to the Chrome directory
        #[arg(long = "as")]
        name: Option<String>,
        /// Expected signed-in email; defaults to Chrome's current login
        #[arg(long)]
        expects: Option<String>,
    },
    /// List Chrome venues and the accounts that reference them
    List,
}

#[derive(Debug, Subcommand)]
pub enum AuthAccessCommand {
    /// Atomically replace an account's ordered access venues
    Set {
        provider: String,
        /// Login email or an unambiguous prefix
        email: String,
        #[arg(long = "profile", required = true)]
        profiles: Vec<String>,
    },
    /// Append one access venue
    Add {
        provider: String,
        /// Login email or an unambiguous prefix
        email: String,
        #[arg(long = "profile")]
        profile: String,
    },
    /// Remove one access venue
    Rm {
        provider: String,
        /// Login email or an unambiguous prefix
        email: String,
        #[arg(long = "profile")]
        profile: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum RouteCommand {
    /// Atomically replace one provider's route for a repository
    Set {
        provider: String,
        #[arg(required = true)]
        accounts: Vec<String>,
        /// Repository owner/name; defaults to the current repository
        #[arg(long)]
        repo: Option<String>,
    },
    /// Configure the store-wide fallback route
    Default {
        #[command(subcommand)]
        cmd: DefaultRouteCommand,
    },
    /// Show repo routes or the defaults they fall back to
    Show {
        /// Repository owner/name; defaults to the current repository
        #[arg(long)]
        repo: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum DefaultRouteCommand {
    /// Atomically replace one provider's store-wide route
    Set {
        provider: String,
        #[arg(required = true)]
        accounts: Vec<String>,
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
    /// Show CI status for current branch
    Ci {
        #[arg(short = 'w', long = "watch")]
        watch: bool,
        #[arg(short = 'l', long = "logs")]
        logs: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

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
    fn auth_help_exposes_managed_account_flows() {
        let command = Cli::command();
        let auth = command
            .find_subcommand("auth")
            .expect("auth subcommand exists");

        for flow in [
            "connect",
            "import",
            "accounts",
            "set",
            "reset",
            "disconnect",
        ] {
            assert!(
                auth.find_subcommand(flow).is_some(),
                "auth help is missing {flow}"
            );
        }
        let connect = auth
            .find_subcommand("connect")
            .expect("connect flow exists");
        assert!(connect
            .get_arguments()
            .any(|argument| argument.get_id() == "email"));
        assert!(connect
            .get_arguments()
            .any(|argument| argument.get_long() == Some("chrome-profile")));
        assert!(!connect
            .get_arguments()
            .any(|argument| argument.get_long() == Some("profile")));
    }

    #[test]
    fn route_accepts_a_provider_specific_account_order() {
        let cli = Cli::try_parse_from(["lf", "route", "set", "claude", "loopflow", "primary"])
            .expect("parse account route");

        assert!(matches!(
            cli.command,
            Some(Commands::Route {
                cmd: RouteCommand::Set {
                    provider,
                    accounts,
                    repo: None,
                }
            }) if provider == "claude" && accounts == vec!["loopflow", "primary"]
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
    fn profile_create_accepts_a_host_local_chrome_profile() {
        let cli = Cli::try_parse_from([
            "lf",
            "profile",
            "create",
            "--chrome-profile",
            "Profile 8",
            "--as",
            "engineering",
            "--expects",
            "engineering@example.com",
        ])
        .expect("parse profile Chrome binding");

        assert!(matches!(
            cli.command,
            Some(Commands::Profile {
                cmd: ProfileCommand::Create {
                    chrome_profile,
                    name: Some(name),
                    expects: Some(expects),
                }
            }) if chrome_profile == "Profile 8"
                && name == "engineering"
                && expects == "engineering@example.com"
        ));
    }

    #[test]
    fn auth_access_set_accepts_ordered_profiles() {
        let cli = Cli::try_parse_from([
            "lf",
            "auth",
            "access",
            "set",
            "claude",
            "operator@",
            "--profile",
            "personal",
            "--profile",
            "engineering",
        ])
        .expect("parse account access order");

        assert!(cli.account.is_empty());
        assert!(matches!(
            cli.command,
            Some(Commands::Auth {
                cmd: AuthCommand::Access {
                    cmd: AuthAccessCommand::Set {
                        provider,
                        email,
                        profiles,
                    }
                }
            }) if provider == "claude"
                && email == "operator@"
                && profiles == vec!["personal", "engineering"]
        ));
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
                }
            }) if provider == "claude"
                && email == "operator@"
                && chrome_profile == "Profile 9"
        ));
    }

    #[test]
    fn auth_import_accepts_managed_account_and_chrome_profile() {
        let cli = Cli::try_parse_from([
            "lf",
            "auth",
            "import",
            "claude",
            "--email",
            "jack@example.com",
            "--chrome-profile",
            "jack@example.com",
        ])
        .expect("parse existing login import");

        assert!(cli.account.is_empty());
        assert!(matches!(
            cli.command,
            Some(Commands::Auth {
                cmd: AuthCommand::Import {
                    provider,
                    email,
                    chrome_profile: Some(chrome_profile),
                }
            }) if provider == "claude"
                && email == "jack@example.com"
                && chrome_profile == "jack@example.com"
        ));
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
                    email,
                    login_email: Some(login_email),
                    routing: Some(routing),
                    plan: Some(plan),
                    paid_through: Some(paid_through),
                    clear_plan: false,
                    clear_paid_through: false,
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
        let cli = Cli::try_parse_from(["lf", "pm", "init", "pm"]).expect("parse");
        let Some(Commands::Pm {
            cmd:
                PmCommand::Init {
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
    fn task_run_accepts_lifecycle_flow_overrides() {
        let cli = Cli::try_parse_from([
            "lf",
            "task",
            "run",
            "INF-123",
            "--first",
            "incident",
            "--loop",
            "ship-5whys",
            "--finally",
            "ship",
        ])
        .expect("parse task lifecycle overrides");
        let Some(Commands::Task {
            cmd:
                TaskCommand::Run {
                    first,
                    loop_,
                    finally,
                    ..
                },
        }) = cli.command
        else {
            panic!("expected task run command");
        };
        assert_eq!(first.as_deref(), Some("incident"));
        assert_eq!(loop_.as_deref(), Some("ship-5whys"));
        assert_eq!(finally.as_deref(), Some("ship"));
    }

    #[test]
    fn task_run_rejects_retired_reviewer_flag() {
        assert!(
            Cli::try_parse_from(["lf", "task", "run", "INF-123", "--reviewer", "parent"]).is_err()
        );
    }

    #[test]
    fn task_start_requires_project_and_allows_piped_title_omission() {
        let cli = Cli::try_parse_from(["lf", "task", "start", "incident-management"])
            .expect("parse Task start with required Project");
        let Some(Commands::Task {
            cmd: TaskCommand::Start {
                project_id, title, ..
            },
        }) = cli.command
        else {
            panic!("expected task start command");
        };
        assert_eq!(project_id, "incident-management");
        assert_eq!(title, None);
        assert!(Cli::try_parse_from(["lf", "task", "start"]).is_err());
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
    fn context_accepts_repeatable_invocation_set_filters() {
        let cli = Cli::try_parse_from([
            "lf",
            "context",
            "--started-after",
            "100",
            "--started-before",
            "200",
            "--repo",
            "/src/a",
            "--repo",
            "/src/b",
            "--project",
            "context",
            "--task",
            "W2-71",
            "--outcome",
            "failed",
            "--capture-state",
            "partial",
            "--steered-only",
            "--current-revision-only",
            "--json",
        ])
        .expect("parse context query");
        let Some(Commands::Context {
            started_after,
            started_before,
            repo,
            project,
            task,
            outcome,
            capture_state,
            steered_only,
            current_revision_only,
            json,
            ..
        }) = cli.command
        else {
            panic!("expected context command");
        };

        assert_eq!(started_after, Some(100));
        assert_eq!(started_before, Some(200));
        assert_eq!(repo, ["/src/a", "/src/b"]);
        assert_eq!(project, ["context"]);
        assert_eq!(task, ["W2-71"]);
        assert_eq!(outcome, ["failed"]);
        assert_eq!(capture_state, ["partial"]);
        assert!(steered_only);
        assert!(current_revision_only);
        assert!(json);
    }

    #[test]
    fn trace_content_requires_json_and_accepts_an_exact_address() {
        let cli = Cli::try_parse_from([
            "lf",
            "trace",
            "run-1",
            "--json",
            "--content",
            "--invocation",
            "invocation-1",
            "--turn",
            "turn-1",
        ])
        .expect("parse trace content");
        assert!(matches!(
            cli.command,
            Some(Commands::Trace {
                content: true,
                invocation: Some(invocation),
                turn: Some(turn),
                ..
            }) if invocation == "invocation-1" && turn == "turn-1"
        ));
        assert!(Cli::try_parse_from(["lf", "trace", "run-1", "--content"]).is_err());
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
    fn top_is_a_first_class_machine_dashboard() {
        let cli = Cli::try_parse_from(["lf", "top"]).expect("parse top");
        assert!(matches!(
            cli.command,
            Some(Commands::Top {
                json: false,
                sort: commands::top::ActivitySort::Rate5m,
            })
        ));

        let cli =
            Cli::try_parse_from(["lf", "ps", "--json", "--sort", "tokens"]).expect("parse ps");
        assert!(matches!(
            cli.command,
            Some(Commands::Ps {
                json: true,
                sort: commands::top::ActivitySort::Tokens,
            })
        ));

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
    fn task_workspace_commands_address_the_task_then_optional_file() {
        let changes = Cli::try_parse_from(["lf", "task", "changes", "INF-123", "--json"])
            .expect("parse task changes");
        assert!(matches!(
            changes.command,
            Some(Commands::Task {
                cmd: TaskCommand::Changes { issue, json: true }
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
                }
            }) if issue == "INF-123" && path == "src/parser.rs"
        ));

        let file =
            Cli::try_parse_from(["lf", "task", "file", "INF-123", "src/parser.rs", "--json"])
                .expect("parse task file");
        assert!(matches!(
            file.command,
            Some(Commands::Task {
                cmd: TaskCommand::File { issue, path, json: true }
            }) if issue == "INF-123" && path == "src/parser.rs"
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
    fn task_steer_is_the_only_authored_direction_command() {
        let steer = Cli::try_parse_from([
            "lf",
            "task",
            "steer",
            "INF-123",
            "take the smaller approach",
        ])
        .expect("parse task steer");
        let Some(Commands::Task {
            cmd:
                TaskCommand::Steer {
                    issue,
                    message,
                    json,
                },
        }) = steer.command
        else {
            panic!("expected task steer command");
        };
        assert_eq!(issue, "INF-123");
        assert_eq!(message, "take the smaller approach");
        assert!(!json);

        for removed in ["follow-up", "acknowledge", "decide", "request-decision"] {
            assert!(
                Cli::try_parse_from(["lf", "task", removed, "INF-123"]).is_err(),
                "{removed} must not remain as a compatibility command"
            );
        }
    }

    #[test]
    fn project_start_accepts_title_wave_and_json() {
        let cli = Cli::try_parse_from([
            "lf",
            "project",
            "start",
            "Release stability",
            "--wave",
            "infrastructure",
            "--json",
        ])
        .expect("parse project start");
        let Some(Commands::Project {
            cmd: ProjectCommand::Start {
                title, wave, json, ..
            },
        }) = cli.command
        else {
            panic!("expected project start command");
        };
        assert_eq!(title, "Release stability");
        assert_eq!(wave.as_deref(), Some("infrastructure"));
        assert!(json);
    }

    #[test]
    fn project_steer_parses() {
        let steer = Cli::try_parse_from([
            "lf",
            "project",
            "steer",
            "project-uuid",
            "prioritize the CLI path",
            "--json",
        ])
        .expect("parse project steer");
        assert!(matches!(
            steer.command,
            Some(Commands::Project {
                cmd: ProjectCommand::Steer {
                    project_id,
                    message,
                    json: true,
                },
            }) if project_id == "project-uuid" && message == "prioritize the CLI path"
        ));
    }

    #[test]
    fn task_and_project_resume_accept_audited_model_handoffs() {
        let task = Cli::try_parse_from([
            "lf",
            "task",
            "resume",
            "W2-135",
            "--model",
            "codex",
            "--reason",
            "Claude quota exhausted",
            "--json",
        ])
        .expect("parse Task body handoff");
        assert!(matches!(
            task.command,
            Some(Commands::Task {
                cmd: TaskCommand::Resume {
                    issue,
                    model: Some(model),
                    reason: Some(reason),
                    json: true,
                    ..
                }
            }) if issue == "W2-135" && model == "codex" && reason == "Claude quota exhausted"
        ));

        let project = Cli::try_parse_from([
            "lf",
            "project",
            "resume",
            "loopflow-api",
            "--model",
            "claude:opus",
        ])
        .expect("parse Project body handoff");
        assert!(matches!(
            project.command,
            Some(Commands::Project {
                cmd: ProjectCommand::Resume {
                    project_id,
                    model: Some(model),
                    reason: None,
                    ..
                }
            }) if project_id == "loopflow-api" && model == "claude:opus"
        ));

        assert!(Cli::try_parse_from([
            "lf",
            "task",
            "resume",
            "W2-135",
            "--reason",
            "quota exhausted",
        ])
        .is_err());
    }

    #[test]
    fn task_recover_accepts_an_optional_audited_reason() {
        let cli = Cli::try_parse_from([
            "lf",
            "task",
            "recover",
            "PRD-9",
            "--reason",
            "the abandoned work is still valid",
            "--json",
        ])
        .expect("parse Task recovery");
        assert!(matches!(
            cli.command,
            Some(Commands::Task {
                cmd: TaskCommand::Recover {
                    issue,
                    reason: Some(reason),
                    json: true,
                }
            }) if issue == "PRD-9" && reason == "the abandoned work is still valid"
        ));
    }
    #[test]
    fn cli_parses_generic_invocation_contract() {
        let cli = Cli::try_parse_from([
            "lf",
            "invocation",
            "handback",
            "invocation_1",
            "--outcome",
            "unknown",
            "--json",
        ])
        .expect("parse Invocation handback");
        assert!(matches!(
            cli.command,
            Some(Commands::Invocation {
                cmd: InvocationCommand::Handback {
                    invocation_id,
                    outcome,
                    json: true,
                }
            }) if invocation_id == "invocation_1" && outcome == "unknown"
        ));
    }

    #[test]
    fn cli_parses_stable_work_controls() {
        let cli = Cli::try_parse_from([
            "lf",
            "work",
            "steer",
            "task",
            "task_1",
            "inspect the failure",
            "--json",
        ])
        .expect("parse Work steer");
        assert!(matches!(
            cli.command,
            Some(Commands::Work {
                cmd: WorkCommand::Steer {
                    kind,
                    id,
                    message,
                    json: true,
                }
            }) if kind == "task" && id == "task_1" && message == "inspect the failure"
        ));

        let asks = Cli::try_parse_from(["lf", "work", "asks", "project", "project_1"])
            .expect("parse Work asks");
        assert!(matches!(
            asks.command,
            Some(Commands::Work {
                cmd: WorkCommand::Asks { kind: Some(kind), id: Some(id), json: false }
            }) if kind == "project" && id == "project_1"
        ));
        let answer = Cli::try_parse_from([
            "lf",
            "work",
            "answer",
            "ask_00000000000000000000000000000001",
            "keep the durable exchange",
            "--json",
        ])
        .expect("parse Work answer");
        assert!(matches!(
            answer.command,
            Some(Commands::Work {
                cmd: WorkCommand::Answer { text, json: true, .. }
            }) if text == "keep the durable exchange"
        ));
        assert!(Cli::try_parse_from(["lf", "work", "continue", "task", "task_1"]).is_err());
        assert!(Cli::try_parse_from(["lf", "work", "escalate", "task", "task_1"]).is_err());

        let place = Cli::try_parse_from([
            "lf",
            "work",
            "place",
            "wave",
            "wave_00000000000000000000000000000001",
            "home_00000000000000000000000000000001",
            "--json",
        ])
        .expect("parse Work placement");
        assert!(matches!(
            place.command,
            Some(Commands::Work {
                cmd: WorkCommand::Place { kind, id, json: true, .. }
            }) if kind == "wave" && id == "wave_00000000000000000000000000000001"
        ));
        assert!(Cli::try_parse_from([
            "lf",
            "work",
            "place",
            "task",
            "task_00000000000000000000000000000001",
            "home_00000000000000000000000000000001",
        ])
        .is_err());
    }

    #[test]
    fn cli_parses_ask_and_wait_as_shell_arguments() {
        let ask =
            Cli::try_parse_from(["lf", "ask", "Which proof matters?"]).expect("parse Ask question");
        assert!(matches!(
            ask.command,
            Some(Commands::Ask { args }) if args == ["Which proof matters?"]
        ));
        let wait =
            Cli::try_parse_from(["lf", "ask", "wait", "ask_00000000000000000000000000000001"])
                .expect("parse Ask wait");
        assert!(matches!(
            wait.command,
            Some(Commands::Ask { args })
                if args == ["wait", "ask_00000000000000000000000000000001"]
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
    fn pm_init_accepts_all_flag() {
        let cli = Cli::try_parse_from(["lf", "pm", "init", "--all"]).expect("parse");
        let Some(Commands::Pm {
            cmd:
                PmCommand::Init {
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
            "pm",
            "init",
            "--wave",
            "product",
            "--team-key",
            "PRD",
            "--team-name",
            "Product",
        ])
        .expect("parse");
        let Some(Commands::Pm {
            cmd:
                PmCommand::Init {
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
        let cli = Cli::try_parse_from(["lf", "pm", "reteam"]).expect("parse");
        let Some(Commands::Pm {
            cmd: PmCommand::Reteam { apply },
        }) = cli.command
        else {
            panic!("expected pm reteam command");
        };
        assert!(!apply);

        let cli = Cli::try_parse_from(["lf", "pm", "reteam", "--apply"]).expect("parse apply");
        let Some(Commands::Pm {
            cmd: PmCommand::Reteam { apply },
        }) = cli.command
        else {
            panic!("expected pm reteam command");
        };
        assert!(apply);
    }

    #[test]
    fn pm_project_archive_accepts_wave_and_project() {
        let cli = Cli::try_parse_from([
            "lf",
            "pm",
            "project",
            "archive",
            "--wave",
            "product",
            "--project",
            "wave-chat",
        ])
        .expect("parse");
        let Some(Commands::Pm {
            cmd:
                PmCommand::Project {
                    cmd: PmProjectCommand::Archive { wave, project },
                },
        }) = cli.command
        else {
            panic!("expected pm project archive command");
        };

        assert_eq!(wave.as_deref(), Some("product"));
        assert_eq!(project, "wave-chat");
    }

    #[test]
    fn pm_show_parses_refresh_modes() {
        let cli = Cli::try_parse_from(["lf", "pm", "show", "--wave", "goals"]).expect("parse");
        let Some(Commands::Pm {
            cmd:
                PmCommand::Show {
                    wave,
                    project,
                    json,
                    sync,
                    no_sync,
                },
        }) = cli.command
        else {
            panic!("expected pm show command");
        };
        assert_eq!(wave.as_deref(), Some("goals"));
        assert_eq!(project, None);
        assert!(!json);
        assert!(!sync);
        assert!(!no_sync);

        let cli = Cli::try_parse_from(["lf", "pm", "show", "--sync"]).expect("force sync");
        let Some(Commands::Pm {
            cmd: PmCommand::Show { sync, no_sync, .. },
        }) = cli.command
        else {
            panic!("expected pm show command");
        };
        assert!(sync);
        assert!(!no_sync);

        let cli = Cli::try_parse_from(["lf", "pm", "show", "--no-sync"]).expect("cache-only read");
        let Some(Commands::Pm {
            cmd: PmCommand::Show { sync, no_sync, .. },
        }) = cli.command
        else {
            panic!("expected pm show command");
        };
        assert!(!sync);
        assert!(no_sync);

        assert!(Cli::try_parse_from(["lf", "pm", "show", "--sync", "--no-sync"]).is_err());
    }

    #[test]
    fn chat_parses_text_and_targeting() {
        let cli = Cli::try_parse_from(["lf", "chat", "shipped", "the", "parser"]).expect("parse");
        let Some(Commands::Chat {
            text,
            follow,
            steer,
            target,
            ..
        }) = cli.command
        else {
            panic!("expected chat command");
        };
        assert_eq!(text, vec!["shipped", "the", "parser"]);
        assert!(!follow);
        assert!(!steer);
        assert_eq!(target.wave, None);
        assert!(!target.parent);

        // No text: stdin is the body. Flags come before the trailing text.
        let cli = Cli::try_parse_from(["lf", "chat", "--parent"]).expect("parse");
        let Some(Commands::Chat { text, target, .. }) = cli.command else {
            panic!("expected chat command");
        };
        assert!(text.is_empty());
        assert!(target.parent);

        let cli = Cli::try_parse_from(["lf", "chat", "--wave", "goals", "hi"]).expect("parse");
        let Some(Commands::Chat { text, target, .. }) = cli.command else {
            panic!("expected chat command");
        };
        assert_eq!(text, vec!["hi"]);
        assert_eq!(target.wave.as_deref(), Some("goals"));

        // Chat messages have no machine-authored byline.
        assert!(Cli::try_parse_from([
            "lf",
            "chat",
            "--wave",
            "goals",
            "--from",
            "ci",
            "CI failed"
        ])
        .is_err());

        let cli =
            Cli::try_parse_from(["lf", "chat", "--steer", "change course"]).expect("parse steer");
        let Some(Commands::Chat { text, steer, .. }) = cli.command else {
            panic!("expected chat command");
        };
        assert_eq!(text, vec!["change course"]);
        assert!(steer);

        assert!(
            Cli::try_parse_from(["lf", "chat", "--steer", "--from", "ci", "change course"])
                .is_err()
        );
        assert!(Cli::try_parse_from(["lf", "chat", "--steer", "--parent", "x"]).is_err());

        // --wave and --parent are mutually exclusive.
        assert!(Cli::try_parse_from(["lf", "chat", "--wave", "goals", "--parent", "x"]).is_err());

        let cli = Cli::try_parse_from(["lf", "chat", "--follow", "--wave", "goals"])
            .expect("parse follow");
        let Some(Commands::Chat {
            text,
            follow,
            steer,
            target,
            ..
        }) = cli.command
        else {
            panic!("expected chat command");
        };
        assert!(text.is_empty());
        assert!(follow);
        assert!(!steer);
        assert_eq!(target.wave.as_deref(), Some("goals"));

        assert!(Cli::try_parse_from(["lf", "chat", "--follow", "hello"]).is_err());

        let cli = Cli::try_parse_from([
            "lf",
            "chat",
            "--history",
            "--json",
            "--limit",
            "20",
            "--epoch",
            "chat-epoch-2",
            "--wave",
            "goals",
        ])
        .expect("parse durable history");
        let Some(Commands::Chat {
            history,
            json,
            limit,
            epoch,
            target,
            ..
        }) = cli.command
        else {
            panic!("expected chat command");
        };
        assert!(history);
        assert!(json);
        assert_eq!(limit, Some(20));
        assert_eq!(epoch.as_deref(), Some("chat-epoch-2"));
        assert_eq!(target.wave.as_deref(), Some("goals"));

        assert!(Cli::try_parse_from(["lf", "chat", "--json", "--wave", "goals"]).is_err());
        assert!(Cli::try_parse_from(["lf", "chat", "--epoch", "chat-epoch-2"]).is_err());
        assert!(Cli::try_parse_from([
            "lf",
            "chat",
            "--history",
            "--json",
            "--follow",
            "--wave",
            "goals"
        ])
        .is_err());
        assert!(Cli::command().find_subcommand("wavechat").is_none());
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
