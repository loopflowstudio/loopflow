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
    /// Git operations
    Op {
        #[command(subcommand)]
        op: OpsCommand,
    },
    /// Start a wave's reactive server: a long-lived process that autonomously
    /// runs progress subagents and serves the live conversation + chat over a
    /// loopback HTTP port (discovery via `wave/<name>/.wave-endpoint`).
    #[command(name = "wave", alias = "loop")]
    Wave {
        /// Wave name (matches wave/<name>/)
        name: String,
        /// Take over even if lfd reports another live wave-agent session
        #[arg(long)]
        force: bool,
    },
    /// Show token usage by repo and provider (from a running lfd)
    Usage,
    /// Show recent loopflow runs from the local ledger (all repos, local-only)
    Runs,
    /// Reconstruct one run from the local ledger: steps, durations, tokens, prompt logs
    Trace {
        /// Run id from `lf runs` (a unique prefix is enough)
        run_id: String,
    },
    /// Queue operations against the shared run registry (daemonless dispatch)
    Q {
        #[command(subcommand)]
        cmd: QCommand,
    },
    /// Post a message into a wave's thread (worker reports, child-wave
    /// escalations, proactive FYIs). Reads stdin when TEXT is omitted.
    Chat {
        /// Message text (reads stdin when omitted — heredoc-friendly)
        #[arg(trailing_var_arg = true)]
        text: Vec<String>,
        #[command(flatten)]
        target: WaveTargetArgs,
    },
    /// Read or curate a wave's MEMORY.md (server-owned; bare `lf memory` = show)
    Memory {
        #[command(subcommand)]
        cmd: Option<MemoryCommand>,
        #[command(flatten)]
        target: WaveTargetArgs,
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
    /// Replace MEMORY.md from stdin (written by the live server, journaled)
    Update {
        /// One-line summary journaled with the update (default: first line)
        #[arg(long)]
        summary: Option<String>,
        #[command(flatten)]
        target: WaveTargetArgs,
    },
    /// Append one curated fact as a bullet
    Add {
        /// The fact to append
        fact: String,
        #[command(flatten)]
        target: WaveTargetArgs,
    },
}

#[derive(Subcommand, Debug)]
pub enum QCommand {
    /// Worker dispatch
    Worker {
        #[command(subcommand)]
        cmd: WorkerCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum WorkerCommand {
    /// Dispatch a worker: record its run + session in the registry and
    /// launch `lf <flow>: <task>` in a detached tmux session
    Run {
        /// Wave name
        wave: String,
        /// Flow the worker runs
        #[arg(long)]
        flow: String,
        /// Task text handed to the flow
        #[arg(long)]
        task: String,
        /// Run in the wave's shared worktree instead of a fresh one
        #[arg(long, conflicts_with = "stack")]
        pool: bool,
        /// Fork the worker's branch from this run's branch (dependent series)
        #[arg(long, value_name = "RUN_ID")]
        stack: Option<String>,
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
    Doctor,
    /// Rebase current branch onto target (default: main)
    Rebase {
        /// Branch to rebase onto
        onto: Option<String>,
    },
    /// Push current branch to remote
    Push {
        #[arg(long)]
        force: bool,
    },
    /// Submit PR to merge queue
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
    /// Sync loopflow steps into vendor Skills directories
    #[command(name = "sync-skills")]
    SyncSkills {
        /// Also write global vendor skill directories under ~/
        #[arg(long = "global")]
        global: bool,
        /// Confirm global writes without prompting
        #[arg(short = 'y', long = "yes")]
        yes: bool,
        /// Keep stale loopflow-generated skills
        #[arg(long = "no-prune")]
        no_prune: bool,
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
    /// Roadmap in Asana (show, update, init, status)
    Pm {
        #[command(subcommand)]
        cmd: PmCommand,
    },
    /// Provider authentication for local lf steps and ops
    Auth {
        #[command(subcommand)]
        cmd: AuthCommand,
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
    /// Connect (or create) the wave's Asana project; write asana_project to GOAL.md
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
    /// Print the wave's live Asana roadmap
    Show {
        /// Wave name (auto-detected if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
    },
    /// Create, edit, or close a roadmap task in Asana
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
    /// External: provider name (so `lf op auth asana` works)
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
        #[arg(short = 'b', long = "base")]
        base: Option<String>,
        #[arg(short = 's', long = "stack")]
        stack: bool,
    },
    /// Switch to a worktree
    Switch {
        /// Worktree name or full branch name to switch to
        name: String,
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
        let Some(Commands::Chat { text, target }) = cli.command else {
            panic!("expected chat command");
        };
        assert_eq!(text, vec!["shipped", "the", "parser"]);
        assert_eq!(target.wave, None);
        assert!(!target.parent);

        // No text: stdin is the body. Flags come before the trailing text.
        let cli = Cli::try_parse_from(["lf", "chat", "--parent"]).expect("parse");
        let Some(Commands::Chat { text, target }) = cli.command else {
            panic!("expected chat command");
        };
        assert!(text.is_empty());
        assert!(target.parent);

        let cli = Cli::try_parse_from(["lf", "chat", "--wave", "goals", "hi"]).expect("parse");
        let Some(Commands::Chat { text, target }) = cli.command else {
            panic!("expected chat command");
        };
        assert_eq!(text, vec!["hi"]);
        assert_eq!(target.wave.as_deref(), Some("goals"));

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
