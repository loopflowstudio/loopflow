use clap::{Args, Parser, Subcommand};

pub mod commands;
pub mod discovery;
pub mod output;
pub mod session;

#[derive(Parser, Debug)]
#[command(name = "lf")]
#[command(about = "Run steps and flows with coding agents")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// List available steps and flows
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

    /// Run this invocation in a separate worktree targeting the current branch
    #[arg(long, conflicts_with_all = ["stack", "fork"])]
    pub dispatch: bool,

    /// Run this invocation in a worktree stacked on a parent run
    #[arg(long, value_name = "RUN_ID", conflicts_with_all = ["dispatch", "fork"])]
    pub stack: Option<String>,

    /// Run this invocation in a separate worktree forked from the review base
    #[arg(long, conflicts_with_all = ["dispatch", "stack"])]
    pub fork: bool,
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
    /// Git operations
    Op {
        #[command(subcommand)]
        op: OpsCommand,
    },
    /// Local launchd jobs that run lf commands on a schedule
    Cron {
        #[command(subcommand)]
        cmd: CronCommand,
    },
    /// Start a wave: a long-lived listener (journal, doors, live events over
    /// a loopback HTTP port; discovery via `wave/<name>/.wave-endpoint`)
    /// that spawns and supervises the wave's flowloop as a resident child
    /// process.
    #[command(name = "wave")]
    Wave {
        /// Wave name (matches wave/<name>/)
        name: String,
        /// Take over even if lfd reports another live wave-agent session
        #[arg(long)]
        force: bool,
        /// Serve dormant: listener only, no resident (health reads flowloop: null)
        #[arg(long, conflicts_with = "flowloop_only")]
        no_flowloop: bool,
        /// Run only the resident flowloop against an existing listener
        #[arg(long, conflicts_with = "no_flowloop")]
        flowloop_only: bool,
    },
    /// Run a task as a bounded flowloop: loop task-pass until the PR merges
    Task {
        /// What to do — free text; the flow's skills clarify it into a design
        /// doc and drive one small PR to merged
        seed: String,
        /// Loop a different flow (any flow is loopable)
        #[arg(long = "flow", default_value = "task-pass")]
        flow: String,
        /// Wave name (default: inferred from the current worktree/branch)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        /// Maximum passes before escalation
        #[arg(long = "max-passes", default_value_t = 8)]
        max_passes: u32,
        /// Per-pass timeout in seconds
        #[arg(long = "pass-timeout-secs", default_value_t = 1800)]
        pass_timeout_secs: u64,
        /// Overall timeout in seconds
        #[arg(long = "wall-clock-secs", default_value_t = 7200)]
        wall_clock_secs: u64,
        /// Poll interval for the loop file's recheck predicate
        #[arg(long = "poll-secs", default_value_t = 60)]
        poll_secs: u64,
        /// Maximum agent turns per pass
        #[arg(long = "max-turns")]
        max_turns: Option<u32>,
    },
    /// Show token usage by repo and provider (from a running lfd)
    Usage,
    /// List every wave in the registry (running and stopped), marking which
    /// have a live server. Local-only query over the shared ledger.
    Ls {
        /// Emit the wave snapshot as JSON (Loopflow's dashboard snapshot)
        #[arg(long)]
        json: bool,
    },
    /// Show one wave's runs, attention, and (when live) flowloop state, from the
    /// registry. Defaults to the ambient wave (`LFD_WAVE_ID`).
    Status {
        /// Wave name (default: the ambient wave)
        wave: Option<String>,
        /// Emit the status snapshot as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show recent loopflow runs from the local ledger (all repos, local-only)
    Runs {
        /// Emit the run history as JSON
        #[arg(long)]
        json: bool,
    },
    /// Reconstruct one run from the local ledger: steps, durations, tokens, prompt logs
    Trace {
        /// Run id from `lf runs` (a unique prefix is enough)
        run_id: String,
    },
    /// Post a message into a wave's thread (worker reports, child-wave
    /// escalations, proactive FYIs). Reads stdin when TEXT is omitted.
    Chat {
        /// Message text (reads stdin when omitted — heredoc-friendly)
        #[arg(trailing_var_arg = true)]
        text: Vec<String>,
        /// Attribution label for machine speech (e.g. --from ci). Overrides
        /// the ambient label; absent = the ambient sender (env, else "cli").
        #[arg(long)]
        from: Option<String>,
        #[command(flatten)]
        target: WaveTargetArgs,
    },
    /// Follow a wave's live event stream (turns, flowloop state, memory) until
    /// killed. Defaults to the invoking context's wave; exits 0 with a note
    /// when no wave resolves.
    Sub {
        /// Wave name (default: the ambient wave — env, else worktree)
        wave: Option<String>,
        /// Emit raw frames as NDJSON instead of human lines
        #[arg(long)]
        json: bool,
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
    /// to resolve them locally. Example: `lf ssh mini-heart -- lf op pr`.
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
    /// External: step/flow name (when no subcommand matches)
    #[command(external_subcommand)]
    External(Vec<String>),
}

/// Wave targeting shared by `lf chat` and `lf memory`: default is the
/// invoking context's wave (`LFD_WAVE_ID` env, else the worktree name).
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
        #[command(flatten)]
        target: WaveTargetArgs,
    },
}

#[derive(Subcommand, Debug)]
pub enum OpsCommand {
    /// Copy context to clipboard
    Cp {
        /// Patterns to exclude
        #[arg(short = 'e', long = "exclude")]
        exclude: Vec<String>,
        /// Files or directories to include
        paths: Vec<String>,
    },
    /// Check loopflow dependencies
    Doctor {
        /// Print the generated Brewfile (from the declared dependency list) and exit
        #[arg(long, hide = true)]
        brewfile: bool,
    },
    /// Rebase current branch onto target (default: main)
    Rebase {
        /// Print the planned rebase strategy without mutating git
        #[arg(long)]
        plan: bool,
        /// Branch to rebase onto
        onto: Option<String>,
    },
    /// Push current branch to remote
    Push {
        #[arg(long)]
        force: bool,
    },
    /// Land a PR hands-off: rebase, clear scratch, arm auto-merge, and rotate
    /// the worktree. For a human-gated merge instead, use `lf op submit`.
    Land {
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        local: bool,
        #[arg(short = 'c', long = "create-pr")]
        create_pr: bool,
        #[arg(short = 'w', long = "worktree")]
        worktree: Option<String>,
        #[arg(short = 'm', long = "message")]
        message: Option<String>,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "body")]
        body: Option<String>,
    },
    /// Prepare a PR to land: rebase, clear scratch, mark ready, and assign it
    /// to you. Nothing merges until you click merge on GitHub — the one
    /// required gate. Does not arm auto-merge or rotate the worktree.
    Submit {
        #[arg(long)]
        strict: bool,
        #[arg(short = 'c', long = "create-pr")]
        create_pr: bool,
        #[arg(short = 'w', long = "worktree")]
        worktree: Option<String>,
        #[arg(short = 'm', long = "message")]
        message: Option<String>,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "body")]
        body: Option<String>,
    },
    /// Create or update a PR
    Pr {
        #[arg(short = 'm', long = "model", short_alias = 'M')]
        model: Option<String>,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "body")]
        body: Option<String>,
    },
    /// Update local main to match origin
    Sync,
    /// Compile loopflow steps into your home vendor Skills directories
    #[command(name = "sync-skills")]
    SyncSkills {
        /// Confirm writes under ~/ without prompting
        #[arg(short = 'y', long = "yes")]
        yes: bool,
        /// Keep stale loopflow-generated skills
        #[arg(long = "no-prune")]
        no_prune: bool,
    },
    /// Rotate a recurring wave onto a fresh branch (pushed with upstream)
    Advance {
        /// Wave name (default: inferred from the worktree)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
    },
    /// Create next iteration branch
    Next {
        #[arg(short = 'c', long = "create-pr")]
        create_pr: bool,
        #[arg(long = "no-rebase")]
        no_rebase: bool,
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
    /// Abandon branch: close PR, remove worktree, delete branch
    Abandon {
        /// Branch to abandon (default: current)
        branch: Option<String>,
        #[arg(short = 'f', long)]
        force: bool,
    },
    /// Remote branch operations
    Branches {
        #[command(subcommand)]
        cmd: BranchesCommand,
    },
    /// Worktree operations
    Wt {
        #[command(subcommand)]
        cmd: WtCommand,
    },
    /// Shell integration
    Shell {
        #[command(subcommand)]
        cmd: ShellCommand,
    },
    /// Release operations (run, check, notes, bump, tag, status)
    Release {
        #[command(subcommand)]
        cmd: ReleaseCommand,
    },
    /// Roadmap in Linear (show, update, init, status)
    Pm {
        #[command(subcommand)]
        cmd: PmCommand,
    },
    /// Provider authentication for local lf steps and ops
    Auth {
        #[command(subcommand)]
        cmd: AuthCommand,
    },
    /// Merge-queue maintenance for stacked wave runs
    Queue {
        #[command(subcommand)]
        cmd: QueueCommand,
    },
    /// Kill every lf-* tmux session and clear stale wave endpoints (fresh start)
    #[command(name = "reset-waves")]
    ResetWaves {
        /// Skip the confirmation prompt
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum CronCommand {
    /// Install or replace a scheduled lf invocation
    Add {
        /// Wave name passed to `lf <flow> --wave <wave>`
        #[arg(short = 'w', long = "wave")]
        wave: String,
        /// Flow or step name to run
        #[arg(long = "flow")]
        flow: String,
        /// Schedule expression. v0 supports `daily`.
        #[arg(long = "schedule", default_value = "daily")]
        schedule: String,
    },
    /// List installed loopflow cron jobs
    List,
    /// Uninstall a scheduled lf invocation
    Remove {
        /// Wave name passed to `lf <flow> --wave <wave>`
        #[arg(short = 'w', long = "wave")]
        wave: String,
        /// Flow or step name to remove
        #[arg(long = "flow")]
        flow: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum QueueCommand {
    /// Run one reconcile pass: stack-status inference, draft/ready flips,
    /// lazy head rebase, queue-block attention writes
    Reconcile {
        /// Only this wave (default: every wave with queue state)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum BranchesCommand {
    /// Preview remote branches by filter
    List {
        #[command(flatten)]
        filters: BranchFilterArgs,
    },
    /// Delete remote branches by filter
    Prune {
        #[command(flatten)]
        filters: BranchFilterArgs,
        /// Show what would be pruned without deleting anything
        #[arg(long = "dry-run")]
        dry_run: bool,
        /// Skip confirmation
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },
}

#[derive(Args, Debug, Clone)]
pub struct BranchFilterArgs {
    /// Branches authored by user (`@me` for current git user)
    #[arg(long = "user")]
    pub user: Option<String>,
    /// Branches whose name includes this wave segment
    #[arg(long = "wave")]
    pub wave: Option<String>,
    /// No commits in the last duration (for example 30d, 2w)
    #[arg(long = "stale")]
    pub stale: Option<String>,
    /// First unique commit before YYYY-MM-DD
    #[arg(long = "created-before")]
    pub created_before: Option<String>,
    /// Only branches already merged into main
    #[arg(long = "merged")]
    pub merged: bool,
    /// Include branches with open PRs
    #[arg(long = "include-open-prs")]
    pub include_open_prs: bool,
}

#[derive(Subcommand, Debug)]
pub enum PmCommand {
    /// Connect (or create) the wave's Linear project; write linear_project to GOAL.md
    Init {
        /// Wave name (auto-detected if omitted)
        wave: Option<String>,
        /// Wave name (flag form; same as positional wave)
        #[arg(short = 'w', long = "wave", conflicts_with_all = ["wave", "all"])]
        wave_flag: Option<String>,
        /// Initialize all waves under wave/
        #[arg(long, conflicts_with_all = ["wave", "wave_flag"])]
        all: bool,
    },
    /// Print the wave's live Linear roadmap
    Show {
        /// Wave name (auto-detected if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
    },
    /// Create, edit, or close a roadmap task in Linear
    Update {
        /// Wave name (auto-detected if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        /// Existing task id to edit or close; omit to create a new task
        #[arg(long = "id")]
        id: Option<String>,
        /// Task title
        #[arg(long = "title")]
        title: String,
        /// Task notes/description
        #[arg(long = "notes")]
        notes: Option<String>,
        /// Set to `done` to close the task
        #[arg(long = "status")]
        status: Option<String>,
        /// PR URL to attach as a comment (the loop's write-back link)
        #[arg(long = "pr")]
        pr: Option<String>,
    },
    /// Show roadmap status for linked waves
    Status {
        /// Wave name (all PM-enabled waves if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
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
    },
    /// External: provider name (so `lf op auth linear` works)
    #[command(external_subcommand)]
    External(Vec<String>),
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
    /// Create a new worktree
    Create {
        /// Worktree name (creates ../NAME)
        name: String,
        /// Stack as a child under a parent branch (defaults to the current branch)
        #[arg(short = 'c', long = "child", value_name = "PARENT", num_args = 0..=1, default_missing_value = "__current__")]
        child: Option<String>,
        /// Root an independent sibling branch from the default branch (the default)
        #[arg(short = 's', long = "sibling", conflicts_with = "child")]
        sibling: bool,
        /// Print the placement plan without creating a worktree
        #[arg(long)]
        plan: bool,
    },
    /// Switch to a worktree by wave name, leaf name, or full branch
    Switch {
        /// Worktree name or full branch name to switch to
        name: String,
    },
    /// Switch to the parent worktree in the stack (toward main)
    Up,
    /// Switch to a child worktree in the stack (away from main)
    Down {
        /// Which child to descend into, when there is more than one
        name: Option<String>,
    },
    /// List worktrees
    List {
        #[arg(long)]
        format: Option<String>,
        #[arg(long)]
        full: bool,
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

#[derive(Subcommand, Debug)]
pub enum ShellCommand {
    /// Print shell integration code
    Init {
        /// Shell to generate for (bash, zsh, fish)
        shell: Option<String>,
    },
    /// Install shell integration to config file
    Install {
        /// Shell to install for (bash, zsh, fish)
        shell: Option<String>,
    },
    /// Run a shell directive
    Directive {
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pm_init_accepts_positional_wave() {
        let cli = Cli::try_parse_from(["lf", "op", "pm", "init", "pm"]).expect("parse");
        let Some(Commands::Op {
            op:
                OpsCommand::Pm {
                    cmd:
                        PmCommand::Init {
                            wave,
                            wave_flag,
                            all,
                        },
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
    fn pm_init_accepts_all_flag() {
        let cli = Cli::try_parse_from(["lf", "op", "pm", "init", "--all"]).expect("parse");
        let Some(Commands::Op {
            op:
                OpsCommand::Pm {
                    cmd:
                        PmCommand::Init {
                            wave,
                            wave_flag,
                            all,
                        },
                },
        }) = cli.command
        else {
            panic!("expected pm init command");
        };

        assert_eq!(wave, None);
        assert_eq!(wave_flag, None);
        assert!(all);
    }

    #[test]
    fn pm_show_accepts_wave_flag() {
        let cli =
            Cli::try_parse_from(["lf", "op", "pm", "show", "--wave", "goals"]).expect("parse");
        let Some(Commands::Op {
            op: OpsCommand::Pm {
                cmd: PmCommand::Show { wave },
            },
        }) = cli.command
        else {
            panic!("expected pm show command");
        };
        assert_eq!(wave.as_deref(), Some("goals"));
    }

    #[test]
    fn pm_update_parses_create_and_close() {
        let cli = Cli::try_parse_from([
            "lf", "op", "pm", "update", "--title", "Ship it", "--notes", "details",
        ])
        .expect("parse");
        let Some(Commands::Op {
            op:
                OpsCommand::Pm {
                    cmd:
                        PmCommand::Update {
                            wave,
                            id,
                            title,
                            notes,
                            status,
                            pr,
                        },
                },
        }) = cli.command
        else {
            panic!("expected pm update command");
        };
        assert_eq!(wave, None);
        assert_eq!(id, None);
        assert_eq!(title, "Ship it");
        assert_eq!(notes.as_deref(), Some("details"));
        assert_eq!(status, None);
        assert_eq!(pr, None);

        let cli = Cli::try_parse_from([
            "lf",
            "op",
            "pm",
            "update",
            "--id",
            "123",
            "--title",
            "Ship it",
            "--status",
            "done",
            "--pr",
            "https://github.com/acme/repo/pull/7",
        ])
        .expect("parse");
        let Some(Commands::Op {
            op:
                OpsCommand::Pm {
                    cmd: PmCommand::Update { id, status, pr, .. },
                },
        }) = cli.command
        else {
            panic!("expected pm update command");
        };
        assert_eq!(id.as_deref(), Some("123"));
        assert_eq!(status.as_deref(), Some("done"));
        assert_eq!(pr.as_deref(), Some("https://github.com/acme/repo/pull/7"));
    }

    #[test]
    fn chat_parses_text_and_targeting() {
        let cli = Cli::try_parse_from(["lf", "chat", "shipped", "the", "parser"]).expect("parse");
        let Some(Commands::Chat { text, from, target }) = cli.command else {
            panic!("expected chat command");
        };
        assert_eq!(text, vec!["shipped", "the", "parser"]);
        assert_eq!(from, None);
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

        // Machine speech declares itself: --from rides ahead of the text
        // (the webhook gatekeeper's planned argv).
        let cli =
            Cli::try_parse_from(["lf", "chat", "--wave", "goals", "--from", "ci", "CI failed"])
                .expect("parse");
        let Some(Commands::Chat { text, from, .. }) = cli.command else {
            panic!("expected chat command");
        };
        assert_eq!(text, vec!["CI failed"]);
        assert_eq!(from.as_deref(), Some("ci"));

        // --wave and --parent are mutually exclusive.
        assert!(Cli::try_parse_from(["lf", "chat", "--wave", "goals", "--parent", "x"]).is_err());
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

        let cli =
            Cli::try_parse_from(["lf", "memory", "add", "one fact", "--parent"]).expect("parse");
        let Some(Commands::Memory {
            cmd: Some(MemoryCommand::Add { fact, target }),
            ..
        }) = cli.command
        else {
            panic!("expected memory add");
        };
        assert_eq!(fact, "one fact");
        assert!(target.parent);
    }

    #[test]
    fn op_advance_parses_optional_wave() {
        let cli = Cli::try_parse_from(["lf", "op", "advance"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Op {
                op: OpsCommand::Advance { wave: None }
            })
        ));

        let cli = Cli::try_parse_from(["lf", "op", "advance", "-w", "goals"]).expect("parse");
        let Some(Commands::Op {
            op: OpsCommand::Advance { wave },
        }) = cli.command
        else {
            panic!("expected op advance command");
        };
        assert_eq!(wave.as_deref(), Some("goals"));
    }

    #[test]
    fn branches_list_accepts_filters() {
        let cli = Cli::try_parse_from([
            "lf", "op", "branches", "list", "--user", "@me", "--stale", "60d",
        ])
        .expect("parse");
        let Some(Commands::Op {
            op:
                OpsCommand::Branches {
                    cmd:
                        BranchesCommand::List {
                            filters:
                                BranchFilterArgs {
                                    user,
                                    stale,
                                    merged,
                                    ..
                                },
                        },
                },
        }) = cli.command
        else {
            panic!("expected branches list command");
        };

        assert_eq!(user.as_deref(), Some("@me"));
        assert_eq!(stale.as_deref(), Some("60d"));
        assert!(!merged);
    }

    #[test]
    fn branches_prune_accepts_yes_and_dry_run() {
        let cli = Cli::try_parse_from([
            "lf",
            "op",
            "branches",
            "prune",
            "--wave",
            "redesign",
            "--dry-run",
            "-y",
        ])
        .expect("parse");
        let Some(Commands::Op {
            op:
                OpsCommand::Branches {
                    cmd:
                        BranchesCommand::Prune {
                            filters: BranchFilterArgs { wave, .. },
                            dry_run,
                            yes,
                        },
                },
        }) = cli.command
        else {
            panic!("expected branches prune command");
        };

        assert_eq!(wave.as_deref(), Some("redesign"));
        assert!(dry_run);
        assert!(yes);
    }

    #[test]
    fn op_pr_accepts_model_override() {
        let cli = Cli::try_parse_from(["lf", "op", "pr", "-m", "codex"]).expect("parse");
        let Some(Commands::Op {
            op: OpsCommand::Pr { model, title, body },
        }) = cli.command
        else {
            panic!("expected pr command");
        };

        assert_eq!(model.as_deref(), Some("codex"));
        assert_eq!(title, None);
        assert_eq!(body, None);
    }

    #[test]
    fn top_level_model_reaches_op_command() {
        let cli = Cli::try_parse_from(["lf", "-m", "codex", "op", "pr"]).expect("parse");
        let Some(Commands::Op {
            op: OpsCommand::Pr { model, title, body },
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
