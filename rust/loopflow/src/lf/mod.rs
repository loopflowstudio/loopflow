use clap::{Parser, Subcommand};
use std::path::PathBuf;

pub mod commands;
pub mod discovery;
pub mod output;

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

    /// Area scope (paths to include in context)
    #[arg(short = 'a', long = "area", short_alias = 'A')]
    pub area: Vec<PathBuf>,

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

    /// Copy prompt to clipboard and open web client
    #[arg(long)]
    pub web: bool,

    /// Enable Chrome integration (Claude)
    #[arg(long)]
    pub chrome: bool,

    /// Disable Chrome integration (Claude)
    #[arg(long = "no-chrome", overrides_with = "chrome")]
    pub no_chrome: bool,

    /// Include lfdocs (wave/, scratch/, root .md files)
    #[arg(long = "lfdocs")]
    pub lfdocs: bool,

    /// Exclude lfdocs (wave/, scratch/, root .md files)
    #[arg(long = "no-lfdocs", overrides_with = "lfdocs")]
    pub no_lfdocs: bool,

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

    /// Get lfdocs setting: Some(true) if --lfdocs, Some(false) if --no-lfdocs, None if neither.
    pub fn lfdocs_setting(&self) -> Option<bool> {
        Self::toggle_setting(self.lfdocs, self.no_lfdocs)
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
    Ops {
        #[command(subcommand)]
        op: OpsCommand,
    },
    /// External: step/flow name (when no subcommand matches)
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand, Debug)]
pub enum OpsCommand {
    /// Copy context to clipboard
    Cp {
        /// Patterns to exclude
        #[arg(short = 'e', long = "exclude")]
        exclude: Vec<String>,
        /// Include lfdocs (scratch/, root .md, wave/)
        #[arg(long)]
        lfdocs: bool,
        /// Exclude lfdocs
        #[arg(long = "no-lfdocs")]
        no_lfdocs: bool,
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
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "body")]
        body: Option<String>,
    },
    /// Update local main to match origin
    Sync,
    /// Create next iteration branch
    Next {
        #[arg(short = 'c', long = "create-pr")]
        create_pr: bool,
        #[arg(long = "no-rebase")]
        no_rebase: bool,
    },
    /// Commit changes (explicit message required)
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
    /// Pick next wave item and move to scratch/
    Ingest {
        /// Wave name (auto-detected from worktree or branch if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
    },
    /// PM tool integration (import, sync)
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
pub enum PmCommand {
    /// Bootstrap PM provider roles for a wave
    Init {
        /// Wave name (auto-detected if omitted)
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
    },
    /// Import projects from PM tool as waves
    Import {
        /// Team ID in the PM provider
        #[arg(short = 't', long = "team")]
        team_id: String,
    },
    /// Three-way sync between local wave files and PM tool
    Sync {
        /// Wave name (auto-detected if omitted)
        wave: Option<String>,
    },
    /// Show PM provider status for linked waves
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
    /// External: provider name (so `lf ops auth asana` works)
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
