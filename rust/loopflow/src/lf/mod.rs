use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

pub mod commands;
pub mod discovery;
pub mod output;

#[derive(Parser, Debug, Default)]
#[command(name = "lf")]
#[command(about = "Open Loopflow or run its CLI")]
#[command(version)]
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

    /// Skip permission prompts
    #[arg(long)]
    pub yolo: bool,

    /// Run interactively
    #[arg(short = 'i', long = "interactive", short_alias = 'I')]
    pub interactive: bool,

    /// Run in batch/headless mode
    #[arg(short = 'b', long = "batch", short_alias = 'B')]
    pub batch: bool,

    /// Hand off to an interactive vendor session in the terminal (overrides session.launch)
    #[arg(long, conflicts_with = "ide")]
    pub tui: bool,

    /// Hand off to an interactive vendor session in the vendor app (overrides session.launch)
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
    /// Personal browser and provider account routing profiles
    Profile {
        #[command(subcommand)]
        cmd: ProfileCommand,
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
    /// A Wave's execution Home: resolve, probe, and start it on its Home
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
        /// Take over even if another live wave session is registered
        #[arg(long)]
        force: bool,
    },
    /// Stop a served wave gracefully
    Stop {
        /// Wave name
        name: String,
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
    /// Linear-backed Task Session lifecycle
    Task {
        #[command(subcommand)]
        cmd: TaskCommand,
    },
    /// Durable interactive work handed from an agent to a human
    Handoff {
        #[command(subcommand)]
        cmd: HandoffCommand,
    },
    /// Internal: run one durable Task Session process generation
    #[command(name = "__task", hide = true)]
    TaskRunner {
        session_id: String,
        #[arg(long)]
        generation: u32,
    },
    /// Internal: run one durable Project Session process generation
    #[command(name = "__project", hide = true)]
    ProjectRunner {
        session_id: String,
        #[arg(long)]
        generation: u32,
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
    /// Show token usage and cost by repo and provider (from the local ledger)
    Usage {
        /// Emit per-boundary spend (skill, provider:model, repo) as JSON
        #[arg(long)]
        json: bool,
        /// Window for --json, in days
        #[arg(long, default_value_t = 30)]
        days: u32,
    },
    /// Graph output-token throughput for the last hour and show running lf processes
    Top,
    /// Inspect supplied agent context and its contributing assets
    Context {
        /// Window in days
        #[arg(long, default_value_t = 30)]
        days: u32,
        /// Filter by wave
        #[arg(long)]
        wave: Option<String>,
        /// Filter by absolute main-repo path
        #[arg(long)]
        repo: Option<String>,
        /// Emit the stable context dataset as JSON
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
        /// Emit the run history as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show recent lf process executions (all repos, local-only)
    Execs {
        /// Emit the process history as JSON
        #[arg(long)]
        json: bool,
    },
    /// Reconstruct the process tree containing one exec, with its skill launches
    Trace {
        /// Exec id from `lf execs` (a unique prefix is enough)
        exec_id: String,
        /// Emit the process tree as JSON
        #[arg(long)]
        json: bool,
        /// Render the normalized recorded conversation
        #[arg(long, conflicts_with = "json")]
        events: bool,
        /// Stream stored event objects as JSONL
        #[arg(long, requires = "events")]
        jsonl: bool,
        /// Select one launch by id prefix
        #[arg(long, requires = "events")]
        launch: Option<String>,
    },
    /// Converse with a served mind's thread (humans); --follow replays it and
    /// --steer reaches the live body. Agents use `lf radio pub` for
    /// agent-to-agent comms, not this.
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
        #[command(flatten)]
        target: WaveTargetArgs,
    },
    /// Publish to or subscribe to the ephemeral agent bus.
    #[command(subcommand_required = true, arg_required_else_help = true)]
    Radio {
        #[command(subcommand)]
        command: RadioCommand,
    },
    // Reserve the retired spelling so the external-subcommand fallback cannot
    // reinterpret the retired top-level spelling as a skill. This variant can
    // never parse.
    #[command(
        name = "sub",
        hide = true,
        about = "Removed; use `lf radio sub`",
        arg_required_else_help = true
    )]
    RetiredSub {
        #[arg(required = true, value_parser = reject_retired_sub)]
        removed: String,
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
    /// Read or curate a wave's MEMORY.md (server-owned; bare `lf memory` = show)
    Memory {
        #[command(subcommand)]
        cmd: Option<MemoryCommand>,
        #[command(flatten)]
        target: WaveTargetArgs,
    },
    /// Run a command on a remote host carrying your local credentials.
    ///
    /// Resolves the local credential bundle (GitHub, Claude, PM) and forwards it
    /// over the ssh channel per-invocation; nothing persists on the remote. The
    /// Doppler token is never forwarded — name specific secrets with `--secret`
    /// to resolve them locally. Example: `lf ssh mini-heart -- lf pr open`.
    Ssh {
        /// Remote host (ssh alias or user@host)
        host: String,
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
        /// Command to run on the remote (after `--`)
        #[arg(last = true)]
        cmd: Vec<String>,
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
pub enum RadioCommand {
    /// Broadcast on the agent bus. Reads stdin when TEXT is omitted.
    Pub {
        /// Message text (reads stdin when omitted — heredoc-friendly)
        #[arg(trailing_var_arg = true)]
        text: Vec<String>,
        /// Broadcast on another channel (a hand's `goals.<run>`) instead of
        /// your own.
        #[arg(short = 'c', long = "channel", conflicts_with = "parent")]
        channel: Option<String>,
        /// Broadcast to the parent wave's channel (escalation up the tree).
        #[arg(long)]
        parent: bool,
        /// Byline for machine speech (e.g. --from ci). Testimony, not proof:
        /// the row records it beside the channel it arrived on.
        #[arg(long)]
        from: Option<String>,
    },
    /// Hear broadcasts on a channel and its descendants while listening.
    Sub {
        /// Channel prefix (default: the ambient channel — env, else worktree)
        channel: Option<String>,
        /// Emit heard frames as NDJSON instead of human lines
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum HandoffCommand {
    /// Open or return the parent's one unresolved interactive handoff
    Open {
        /// Parent reference: wave:<id>, project:<id>, or task:<id>
        #[arg(long)]
        parent: String,
        /// Canonical execution Home, e.g. jack@local or ssh://jack@host
        #[arg(long)]
        home: String,
        /// Absolute worktree/current directory on that Home
        #[arg(long)]
        cwd: PathBuf,
        /// Provider that owns the resumed history
        #[arg(long)]
        provider: String,
        /// Provider transcript/session id, when one exists
        #[arg(long = "provider-session")]
        provider_session: Option<String>,
        /// Existing parent body generation being handed off
        #[arg(long)]
        generation: u32,
        /// Why human interaction is required
        #[arg(long)]
        reason: String,
        /// Required environment entry as KEY=VALUE (repeatable)
        #[arg(long = "env")]
        environment: Vec<String>,
        /// Emit the durable Session as JSON
        #[arg(long)]
        json: bool,
        /// Structured attach argv after `--`
        #[arg(last = true, required = true)]
        attach_argv: Vec<String>,
    },
    /// Show one durable handoff Session
    Status {
        session_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Record first attach and return its descriptor; never streams terminal bytes
    Attach {
        session_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Record successful completion
    Complete {
        session_id: String,
        #[arg(long)]
        summary: String,
        #[arg(long)]
        json: bool,
    },
    /// Hand unfinished work back to the parent agent
    Back {
        session_id: String,
        #[arg(long)]
        summary: String,
        #[arg(long)]
        json: bool,
    },
    /// Record terminal interactive-body failure
    Fail {
        session_id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
}

fn reject_retired_sub(_: &str) -> Result<String, String> {
    Err("the top-level subscription command was removed; use `lf radio sub`".to_string())
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

/// Wave targeting shared by `lf chat` and `lf memory`: default is the
/// invoking context's wave (`LF_WAVE_ID` env, else the worktree name).
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
pub enum MemoryCommand {
    /// Print the wave's MEMORY.md
    Show {
        #[command(flatten)]
        target: WaveTargetArgs,
    },
    /// Print memory facts added since the last update
    Log {
        /// Emit facts with their evidence receipts as JSON
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        target: WaveTargetArgs,
    },
    /// Replace MEMORY.md from stdin (written by the live server, journaled)
    Update {
        /// One-line summary journaled with the update (default: first line)
        #[arg(long)]
        summary: Option<String>,
        #[command(flatten)]
        target: WaveTargetArgs,
    },
    /// Publish one fact to the replayable memory stream
    Add {
        /// The fact to publish
        fact: String,
        /// Evidence receipt binding the fact to its raw record, written as
        /// `kind:reference` (e.g. `chat_turn:turn-3`, `run:<run_id>`,
        /// `pr:owner/repo#N`). Repeatable for many-to-one evidence.
        #[arg(long = "receipt")]
        receipts: Vec<String>,
        #[command(flatten)]
        target: WaveTargetArgs,
    },
}

#[derive(Subcommand, Debug)]
pub enum ProjectCommand {
    /// Create a Linear Project first, then start its durable Project Session
    Start {
        title: String,
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        #[arg(long)]
        directive: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Start or resume the current Session for an existing Linear Project
    Run {
        /// Linear Project UUID or unique slug
        project_id: String,
        #[arg(long)]
        directive: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show durable Project Session state and reconcile process liveness
    Status {
        /// Linear Project UUID, unique slug, or historical Project Session id
        project_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Queue an audited instruction for exactly the next provider turn
    FollowUp {
        project_id: String,
        message: String,
        #[arg(long)]
        json: bool,
    },
    /// Redirect Project work now, relaunching the Session when needed
    Steer {
        project_id: String,
        message: String,
        #[arg(long)]
        json: bool,
    },
    /// Interrupt the active Project turn and optionally replace its next instruction
    Interrupt {
        project_id: String,
        #[arg(long = "message")]
        message: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Read or wait for one durable Project command receipt
    Receipt {
        command_id: String,
        #[arg(long, value_enum)]
        until: Option<crate::ops::ChildReceiptUntil>,
        #[arg(long, default_value = "30s")]
        timeout: String,
        #[arg(long)]
        json: bool,
    },
    /// Confirm that this Project incorporated its current direction
    Acknowledge {
        project_id: String,
        #[arg(long)]
        directive: u32,
        #[arg(long)]
        summary: String,
        #[arg(long)]
        json: bool,
    },
    /// Resolve a durable Project decision request
    Decide {
        project_id: String,
        decision_id: String,
        choice: String,
        #[arg(long)]
        message: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Ask the owning Wave to choose while preserving this Project Session
    RequestDecision {
        project_id: String,
        prompt: String,
        #[arg(long = "option", required = true)]
        options: Vec<String>,
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value = "30m")]
        timeout: String,
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
    /// Resume the same Project Session, optionally handing its next body to another agent
    Resume {
        project_id: String,
        message: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long, requires = "model")]
        reason: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Attach to the writable Project Session control terminal
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
    /// Ensure its Project Session, then start or return the existing Linear task
    Run {
        issue: String,
        #[arg(long)]
        name: Option<String>,
        /// Use this Task flow instead of the Project/default flow
        #[arg(long, value_name = "FLOW")]
        flow: Option<String>,
        /// Fork this Task's worktree from another Task's active PR
        #[arg(long = "stack-on", value_name = "PARENT_TASK")]
        stack_on: Option<String>,
        #[arg(long)]
        directive: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Create a Linear task, ensure its Project Session, then start its Task Session
    Start {
        title: String,
        #[arg(short = 'p', long = "project")]
        project_id: String,
        #[arg(long)]
        name: Option<String>,
        /// Use this Task flow instead of the Project/default flow
        #[arg(long, value_name = "FLOW")]
        flow: Option<String>,
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
    /// Queue an audited instruction for exactly the next provider turn
    FollowUp {
        issue: String,
        message: String,
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
    /// Interrupt the active provider turn and optionally replace its next instruction
    Interrupt {
        issue: String,
        #[arg(long = "message")]
        message: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Read or wait for one durable command receipt
    Receipt {
        command_id: String,
        #[arg(long, value_enum)]
        until: Option<crate::ops::ChildReceiptUntil>,
        #[arg(long, default_value = "30s")]
        timeout: String,
        #[arg(long)]
        json: bool,
    },
    /// Confirm that this Task incorporated its current direction
    Acknowledge {
        issue: String,
        #[arg(long)]
        directive: u32,
        #[arg(long)]
        summary: String,
        #[arg(long)]
        json: bool,
    },
    /// Resolve a durable Task decision request
    Decide {
        issue: String,
        decision_id: String,
        choice: String,
        #[arg(long)]
        message: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Ask the Task's Project Session to choose while preserving this Task Session
    RequestDecision {
        issue: String,
        prompt: String,
        #[arg(long = "option", required = true)]
        options: Vec<String>,
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value = "30m")]
        timeout: String,
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
    /// Resume the same Task Session, optionally handing its next body to another agent
    Resume {
        issue: String,
        message: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long, requires = "model")]
        reason: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Attach read-write to the Task Session control terminal
    Attach { issue: String },
    /// Explicitly end a Task Session without merging
    Abandon {
        issue: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum PrCommand {
    /// Show current branch's PR state
    Status,
    /// After an out-of-band merge, rotate this Task to its next serial PR,
    /// carrying preserved follow-up edits forward onto the new branch.
    Next {
        /// Name the next serial branch (defaults to the settled PR's next slug,
        /// then the sequence number).
        slug: Option<String>,
    },
    /// Create or update a PR
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
        /// Wave name passed to `lf <flow> --wave <wave>`
        #[arg(short = 'w', long = "wave")]
        wave: String,
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
    /// Connect a wave to its Linear Initiative and team (Task prefix)
    Init {
        /// Wave name (auto-detected if omitted)
        wave: Option<String>,
        /// Wave name (flag form; same as positional wave)
        #[arg(short = 'w', long = "wave", conflicts_with_all = ["wave", "all"])]
        wave_flag: Option<String>,
        /// Initialize all waves under wave/
        #[arg(long, conflicts_with_all = ["wave", "wave_flag"])]
        all: bool,
        /// Team key = Task prefix (e.g. PRD). Defaults from the wave name.
        #[arg(long = "team-key")]
        team_key: Option<String>,
        /// Team display name. Defaults to the title-cased wave name.
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
    /// Move a wave's existing settled issues into its own Linear team
    Reteam {
        /// Wave name (auto-detected if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
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
    },
    /// Replace a Linear Project's definition and KRs
    Update {
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        #[arg(short = 'p', long = "project")]
        project: String,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "definition")]
        definition: String,
        /// Key result; repeat for each KR. Prefix with `[x] ` when it holds.
        #[arg(long = "kr", required = true)]
        krs: Vec<String>,
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

/// `lf home` — the shared Home control path a conductor surface drives.
#[derive(Debug, Subcommand)]
pub enum HomeCommand {
    /// Probe a Wave's Home for liveness and the one contextual action.
    ///
    /// Prints the Home address, its state (unreachable/stopped/running/unknown)
    /// with the evidence, the attach endpoint when running, and the action to
    /// offer. `--json` emits the `HomeRuntimeDto` a UI consumes.
    Probe {
        /// Wave name; defaults to the ambient wave.
        wave: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Idempotently start a Wave on its configured Home and return the attach
    /// identity. Safe to repeat: an already-running Home is returned as-is rather
    /// than launched twice. Targets the Home, not the machine running this
    /// command.
    Start {
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
        /// Disconnect one managed OAuth account
        #[arg(long)]
        account: Option<String>,
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
        /// Create or reconnect an isolated OAuth account profile
        #[arg(long)]
        account: Option<String>,
        /// Chrome profile directory, name, or signed-in email for browser OAuth
        #[arg(long, requires = "account", conflicts_with = "profile")]
        chrome_profile: Option<String>,
        /// Use this Loopflow profile's host-local Chrome binding
        #[arg(long, requires = "account")]
        profile: Option<String>,
    },
    /// Adopt an existing Claude login into a managed account
    Import {
        provider: String,
        /// Create or register this isolated OAuth account profile
        #[arg(long)]
        account: String,
        /// Chrome profile directory, name, or signed-in email
        #[arg(long, conflicts_with = "profile")]
        chrome_profile: Option<String>,
        /// Use this Loopflow profile's host-local Chrome binding
        #[arg(long)]
        profile: Option<String>,
    },
    /// List managed Claude and Codex OAuth accounts
    Accounts {
        /// Provider name (optional)
        provider: Option<String>,
    },
    /// Record provider-specific account identity, routing, and billing state
    Set {
        provider: String,
        account: String,
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
    Reset { provider: String, account: String },
    /// External: provider name (so `lf auth linear` works)
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    /// Create a personal routing profile
    Create {
        profile: String,
        /// Chrome profile directory, name, or signed-in email on this host
        #[arg(long)]
        chrome_profile: Option<String>,
    },
    /// List personal routing profiles and their provider accounts
    List,
    /// Bind provider accounts to profiles
    Account {
        #[command(subcommand)]
        cmd: ProfileAccountCommand,
    },
    /// Configure this repository's profile order
    Route {
        #[command(subcommand)]
        cmd: ProfileRouteCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProfileAccountCommand {
    /// Bind a provider account by account id or login email
    Set {
        profile: String,
        provider: String,
        account: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProfileRouteCommand {
    /// Atomically replace the default and ordered backup profiles
    Set {
        #[arg(long)]
        default: String,
        #[arg(long = "backup")]
        backups: Vec<String>,
        /// Repository owner/name; defaults to the current repository
        #[arg(long)]
        repo: Option<String>,
    },
    /// Show the default and ordered backup profiles
    Show {
        /// Repository owner/name; defaults to the current repository
        #[arg(long)]
        repo: Option<String>,
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
    /// Remove worktrees whose branches have been merged
    Prune {
        /// Show what would be pruned without removing anything
        #[arg(long)]
        dry_run: bool,
        /// Also prune fresh worktrees (no commits beyond main)
        #[arg(long)]
        include_fresh: bool,
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
        for flow in ["connect", "disconnect"] {
            let subcommand = auth.find_subcommand(flow).expect("auth flow exists");
            assert!(subcommand
                .get_arguments()
                .any(|argument| argument.get_long() == Some("account")));
        }
    }

    #[test]
    fn profile_route_accepts_ordered_backups() {
        let cli = Cli::try_parse_from([
            "lf",
            "profile",
            "route",
            "set",
            "--default",
            "primary@example.com",
            "--backup",
            "engineering@example.com",
            "--backup",
            "personal@example.com",
        ])
        .expect("parse profile route");

        assert!(matches!(
            cli.command,
            Some(Commands::Profile {
                cmd: ProfileCommand::Route {
                    cmd: ProfileRouteCommand::Set {
                        default,
                        backups,
                        repo: None,
                    }
                }
            }) if default == "primary@example.com"
                && backups == vec!["engineering@example.com", "personal@example.com"]
        ));
    }

    #[test]
    fn profile_create_accepts_a_host_local_chrome_profile() {
        let cli = Cli::try_parse_from([
            "lf",
            "profile",
            "create",
            "engineering@example.com",
            "--chrome-profile",
            "engineering@example.com",
        ])
        .expect("parse profile Chrome binding");

        assert!(matches!(
            cli.command,
            Some(Commands::Profile {
                cmd: ProfileCommand::Create {
                    profile,
                    chrome_profile: Some(chrome_profile),
                }
            }) if profile == "engineering@example.com"
                && chrome_profile == "engineering@example.com"
        ));
    }

    #[test]
    fn profile_account_set_accepts_a_login_email() {
        let cli = Cli::try_parse_from([
            "lf",
            "profile",
            "account",
            "set",
            "primary@example.com",
            "claude",
            "primary@example.com",
        ])
        .expect("parse profile account mapping");

        assert!(matches!(
            cli.command,
            Some(Commands::Profile {
                cmd: ProfileCommand::Account {
                    cmd: ProfileAccountCommand::Set {
                        profile,
                        provider,
                        account,
                    }
                }
            }) if profile == "primary@example.com"
                && provider == "claude"
                && account == "primary@example.com"
        ));
    }

    #[test]
    fn auth_connect_accepts_managed_account_slug() {
        let cli = Cli::try_parse_from(["lf", "auth", "connect", "claude", "--account", "primary"])
            .expect("parse managed account login");

        assert!(matches!(
            cli.command,
            Some(Commands::Auth {
                cmd: AuthCommand::Connect {
                    provider,
                    account,
                    chrome_profile: None,
                    profile: None,
                }
            }) if provider == "claude" && account.as_deref() == Some("primary")
        ));
    }

    #[test]
    fn auth_connect_accepts_matching_chrome_profile() {
        let cli = Cli::try_parse_from([
            "lf",
            "auth",
            "connect",
            "claude",
            "--account",
            "loopflow",
            "--chrome-profile",
            "operator@example.com",
        ])
        .expect("parse Chrome profile pairing");

        assert!(matches!(
            cli.command,
            Some(Commands::Auth {
                cmd: AuthCommand::Connect {
                    provider,
                    account,
                    chrome_profile: Some(chrome_profile),
                    profile: None,
                }
            }) if provider == "claude"
                && account.as_deref() == Some("loopflow")
                && chrome_profile == "operator@example.com"
        ));
    }

    #[test]
    fn auth_connect_accepts_a_loopflow_profile() {
        let cli = Cli::try_parse_from([
            "lf",
            "auth",
            "connect",
            "claude",
            "--account",
            "primary",
            "--profile",
            "personal@example.com",
        ])
        .expect("parse Loopflow profile binding");

        assert!(matches!(
            cli.command,
            Some(Commands::Auth {
                cmd: AuthCommand::Connect {
                    provider,
                    account,
                    chrome_profile: None,
                    profile: Some(profile),
                }
            }) if provider == "claude"
                && account.as_deref() == Some("primary")
                && profile == "personal@example.com"
        ));
    }

    #[test]
    fn auth_connect_accepts_a_codex_profile() {
        let cli = Cli::try_parse_from([
            "lf",
            "auth",
            "connect",
            "codex",
            "--account",
            "engineering",
            "--profile",
            "engineering@example.com",
        ])
        .expect("parse Codex profile binding");

        assert!(matches!(
            cli.command,
            Some(Commands::Auth {
                cmd: AuthCommand::Connect {
                    provider,
                    account,
                    chrome_profile: None,
                    profile: Some(profile),
                }
            }) if provider == "codex"
                && account.as_deref() == Some("engineering")
                && profile == "engineering@example.com"
        ));
    }

    #[test]
    fn auth_import_accepts_managed_account_and_chrome_profile() {
        let cli = Cli::try_parse_from([
            "lf",
            "auth",
            "import",
            "claude",
            "--account",
            "loopflow",
            "--chrome-profile",
            "jack@example.com",
        ])
        .expect("parse existing login import");

        assert!(matches!(
            cli.command,
            Some(Commands::Auth {
                cmd: AuthCommand::Import {
                    provider,
                    account,
                    chrome_profile: Some(chrome_profile),
                    profile: None,
                }
            }) if provider == "claude"
                && account == "loopflow"
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
            "loopflow",
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

        assert!(matches!(
            cli.command,
            Some(Commands::Auth {
                cmd: AuthCommand::Set {
                    provider,
                    account,
                    login_email: Some(login_email),
                    routing: Some(routing),
                    plan: Some(plan),
                    paid_through: Some(paid_through),
                    clear_plan: false,
                    clear_paid_through: false,
                }
            }) if provider == "codex"
                && account == "loopflow"
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
    fn task_run_accepts_an_explicit_flow() {
        let cli = Cli::try_parse_from(["lf", "task", "run", "INF-123", "--flow", "iterate"])
            .expect("parse task review policy");
        let Some(Commands::Task {
            cmd: TaskCommand::Run { flow, .. },
        }) = cli.command
        else {
            panic!("expected task run command");
        };
        assert_eq!(flow.as_deref(), Some("iterate"));
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

        let land =
            Cli::try_parse_from(["lf", "pr", "land", "-c", "-p"]).expect("parse completing land");
        assert!(matches!(
            land.command,
            Some(Commands::Pr {
                cmd: Some(PrCommand::Land {
                    complete: true,
                    create_pr: true,
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
        assert!(matches!(cli.command, Some(Commands::Top)));
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
    fn task_interrupt_requires_explicit_message_flag() {
        let cli = Cli::try_parse_from([
            "lf",
            "task",
            "interrupt",
            "INF-123",
            "--message",
            "take the smaller approach",
        ])
        .expect("parse task interrupt");
        let Some(Commands::Task {
            cmd:
                TaskCommand::Interrupt {
                    issue,
                    message,
                    json,
                },
        }) = cli.command
        else {
            panic!("expected task interrupt command");
        };
        assert_eq!(issue, "INF-123");
        assert_eq!(message.as_deref(), Some("take the smaller approach"));
        assert!(!json);
    }

    #[test]
    fn task_receipt_and_decision_commands_parse_the_durable_ids() {
        let receipt = Cli::try_parse_from([
            "lf",
            "task",
            "receipt",
            "cc_00000000000000000000000000000000",
            "--until",
            "incorporated",
            "--timeout",
            "30s",
            "--json",
        ])
        .expect("parse task receipt");
        assert!(matches!(
            receipt.command,
            Some(Commands::Task {
                cmd: TaskCommand::Receipt {
                    until: Some(crate::ops::ChildReceiptUntil::Incorporated),
                    timeout,
                    json: true,
                    ..
                }
            }) if timeout == "30s"
        ));

        let acknowledge = Cli::try_parse_from([
            "lf",
            "task",
            "acknowledge",
            "INF-123",
            "--directive",
            "2",
            "--summary",
            "parser work is now first",
        ])
        .expect("parse task acknowledgement");
        assert!(matches!(
            acknowledge.command,
            Some(Commands::Task {
                cmd: TaskCommand::Acknowledge {
                    issue,
                    directive: 2,
                    ..
                }
            }) if issue == "INF-123"
        ));

        let decide = Cli::try_parse_from([
            "lf",
            "task",
            "decide",
            "INF-123",
            "cd_00000000000000000000000000000000",
            "revise",
            "--message",
            "cover the race",
            "--json",
        ])
        .expect("parse task decide");
        assert!(matches!(
            decide.command,
            Some(Commands::Task {
                cmd: TaskCommand::Decide {
                    issue,
                    choice,
                    json: true,
                    ..
                }
            }) if issue == "INF-123" && choice == "revise"
        ));
    }

    #[test]
    fn task_steering_verbs_are_distinct_and_support_json_receipts() {
        let follow_up = Cli::try_parse_from([
            "lf",
            "task",
            "follow-up",
            "INF-123",
            "audit retry callers",
            "--json",
        ])
        .expect("parse task follow-up");
        let Some(Commands::Task {
            cmd:
                TaskCommand::FollowUp {
                    issue,
                    message,
                    json,
                },
        }) = follow_up.command
        else {
            panic!("expected task follow-up command");
        };
        assert_eq!(issue, "INF-123");
        assert_eq!(message, "audit retry callers");
        assert!(json);

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
    fn project_session_controls_parse_durable_ids_and_waits() {
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

        let receipt = Cli::try_parse_from([
            "lf",
            "project",
            "receipt",
            "cc_00000000000000000000000000000000",
            "--until",
            "applied",
            "--timeout",
            "30s",
        ])
        .expect("parse project receipt");
        assert!(matches!(
            receipt.command,
            Some(Commands::Project {
                cmd: ProjectCommand::Receipt {
                    command_id,
                    until: Some(crate::ops::ChildReceiptUntil::Applied),
                    timeout,
                    ..
                },
            }) if command_id.starts_with("cc_") && timeout == "30s"
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
    fn cli_parses_interactive_handoff_contract() {
        let open = Cli::try_parse_from([
            "lf",
            "handoff",
            "open",
            "--parent",
            "task:ts_00000000000000000000000000000000",
            "--home",
            "jack@local",
            "--cwd",
            "/src/loopflow.task",
            "--provider",
            "codex",
            "--provider-session",
            "thread-1",
            "--generation",
            "3",
            "--reason",
            "OAuth login required",
            "--env",
            "LF_HOME=/tmp/lf",
            "--json",
            "--",
            "tmux",
            "attach-session",
            "-t",
            "lf-task-interactive",
        ])
        .expect("parse handoff open");
        assert!(matches!(
            open.command,
            Some(Commands::Handoff {
                cmd: HandoffCommand::Open {
                    generation: 3,
                    json: true,
                    attach_argv,
                    ..
                }
            }) if attach_argv == ["tmux", "attach-session", "-t", "lf-task-interactive"]
        ));

        let back = Cli::try_parse_from([
            "lf",
            "handoff",
            "back",
            "ih_00000000000000000000000000000000",
            "--summary",
            "finish the review fixes headlessly",
            "--json",
        ])
        .expect("parse handoff back");
        assert!(matches!(
            back.command,
            Some(Commands::Handoff {
                cmd: HandoffCommand::Back {
                    summary,
                    json: true,
                    ..
                }
            }) if summary == "finish the review fixes headlessly"
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
        let cli = Cli::try_parse_from(["lf", "pm", "reteam", "--wave", "product"]).expect("parse");
        let Some(Commands::Pm {
            cmd: PmCommand::Reteam { wave, apply },
        }) = cli.command
        else {
            panic!("expected pm reteam command");
        };
        assert_eq!(wave.as_deref(), Some("product"));
        assert!(!apply);

        let cli = Cli::try_parse_from(["lf", "pm", "reteam", "--apply"]).expect("parse apply");
        let Some(Commands::Pm {
            cmd: PmCommand::Reteam { apply, .. },
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

        // Machine speech does not ride this verb: bylines belong to the bus
        // (`lf radio pub --from`), and chat refuses the flag at parse.
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
            "--wave",
            "goals",
        ])
        .expect("parse durable history");
        let Some(Commands::Chat {
            history,
            json,
            limit,
            target,
            ..
        }) = cli.command
        else {
            panic!("expected chat command");
        };
        assert!(history);
        assert!(json);
        assert_eq!(limit, Some(20));
        assert_eq!(target.wave.as_deref(), Some("goals"));

        assert!(Cli::try_parse_from(["lf", "chat", "--json", "--wave", "goals"]).is_err());
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
    fn radio_pub_parses_channel_parent_and_byline() {
        // `-c`/`--channel` addresses a hand's channel; text trails.
        let cli = Cli::try_parse_from(["lf", "radio", "pub", "-c", "goals.148e", "landed PR"])
            .expect("parse");
        let Some(Commands::Radio {
            command:
                RadioCommand::Pub {
                    text,
                    channel,
                    parent,
                    from,
                },
        }) = cli.command
        else {
            panic!("expected radio pub command");
        };
        assert_eq!(text, vec!["landed PR"]);
        assert_eq!(channel.as_deref(), Some("goals.148e"));
        assert!(!parent);
        assert_eq!(from, None);

        // Escalation up the tree.
        let cli = Cli::try_parse_from(["lf", "radio", "pub", "--parent", "blocked"])
            .expect("parse parent");
        let Some(Commands::Radio {
            command: RadioCommand::Pub { parent, .. },
        }) = cli.command
        else {
            panic!("expected radio pub command");
        };
        assert!(parent);

        // A channel and the parent are mutually exclusive — a report goes to
        // one place.
        assert!(
            Cli::try_parse_from(["lf", "radio", "pub", "-c", "goals.148e", "--parent", "x"])
                .is_err()
        );
    }

    #[test]
    fn radio_sub_parses_channel_and_json() {
        let cli = Cli::try_parse_from(["lf", "radio", "sub", "goals", "--json"]).expect("parse");
        let Some(Commands::Radio {
            command: RadioCommand::Sub { channel, json },
        }) = cli.command
        else {
            panic!("expected radio sub command");
        };
        assert_eq!(channel.as_deref(), Some("goals"));
        assert!(json);
    }

    #[test]
    fn radio_requires_an_explicit_operation_and_rejects_old_forms() {
        let error = Cli::try_parse_from(["lf", "radio"]).expect_err("bare radio shows help");
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
        assert!(Cli::try_parse_from(["lf", "radio", "worker done"]).is_err());
        assert!(Cli::try_parse_from(["lf", "sub"]).is_err());
        assert!(Cli::try_parse_from(["lf", "sub", "goals"]).is_err());
    }

    /// Steer is a thread op, and the thread is not the bus. The shared
    /// dispatch once let `--steer` leak across the verb split; separate
    /// transports make it unspellable.
    #[test]
    fn radio_has_no_steer_flag() {
        assert!(Cli::try_parse_from(["lf", "radio", "pub", "--steer", "gate it"]).is_err());
    }

    #[test]
    fn memory_parses_bare_show_update_and_add() {
        let cli = Cli::try_parse_from(["lf", "memory", "--wave", "goals"]).expect("parse");
        let Some(Commands::Memory { cmd, target }) = cli.command else {
            panic!("expected memory command");
        };
        assert!(cmd.is_none(), "bare memory is show");
        assert_eq!(target.wave.as_deref(), Some("goals"));

        let cli =
            Cli::try_parse_from(["lf", "memory", "update", "--summary", "learned"]).expect("parse");
        let Some(Commands::Memory {
            cmd: Some(MemoryCommand::Update { summary, .. }),
            ..
        }) = cli.command
        else {
            panic!("expected memory update");
        };
        assert_eq!(summary.as_deref(), Some("learned"));

        let cli = Cli::try_parse_from([
            "lf",
            "memory",
            "add",
            "one fact",
            "--receipt",
            "chat_turn:turn-3",
            "--receipt",
            "run:run-9",
            "--parent",
        ])
        .expect("parse");
        let Some(Commands::Memory {
            cmd:
                Some(MemoryCommand::Add {
                    fact,
                    receipts,
                    target,
                }),
            ..
        }) = cli.command
        else {
            panic!("expected memory add");
        };
        assert_eq!(fact, "one fact");
        assert_eq!(receipts, vec!["chat_turn:turn-3", "run:run-9"]);
        assert!(target.parent);
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
