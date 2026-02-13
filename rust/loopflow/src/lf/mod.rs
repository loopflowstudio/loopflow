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

    /// Area scope (paths to include in context)
    #[arg(short = 'a', long = "area", short_alias = 'A')]
    pub area: Vec<PathBuf>,

    /// Include clipboard content in prompt
    #[arg(short = 'c', long = "clipboard", short_alias = 'C')]
    pub clipboard: bool,

    /// Model to use (backend or backend:variant)
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

    /// Wave name for roadmap scoping
    #[arg(short = 'w', long = "wave", short_alias = 'W')]
    pub wave: Option<String>,
}

impl Cli {
    /// Get chrome setting: Some(true) if --chrome, Some(false) if --no-chrome, None if neither.
    pub fn chrome_setting(&self) -> Option<bool> {
        if self.chrome {
            Some(true)
        } else if self.no_chrome {
            Some(false)
        } else {
            None
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run a step or flow
    Run {
        name: String,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
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
        /// Include lfdocs (scratch/, root .md, roadmap/)
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
    Rebase {
        onto: Option<String>,
    },
    Push {
        #[arg(long)]
        force: bool,
    },
    Land {
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        local: bool,
        #[arg(short = 'c', long = "create-pr")]
        create_pr: bool,
        #[arg(short = 'w', long = "worktree")]
        worktree: Option<String>,
        #[arg(long = "no-lint")]
        no_lint: bool,
    },
    Pr {
        #[arg(short = 'r', long = "refresh")]
        refresh: bool,
        #[arg(long = "no-lint")]
        no_lint: bool,
    },
    Sync,
    Next {
        #[arg(short = 'c', long = "create-pr")]
        create_pr: bool,
        #[arg(long = "no-rebase")]
        no_rebase: bool,
    },
    Commit {
        #[arg(short = 'm', long = "message", short_alias = 'M')]
        message: Option<String>,
        #[arg(short = 'p', long = "push", short_alias = 'P')]
        push: bool,
        #[arg(long = "no-add")]
        no_add: bool,
        #[arg(long = "no-lint")]
        no_lint: bool,
    },
    Abandon {
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
}

#[derive(Subcommand, Debug)]
pub enum WtCommand {
    Create {
        name: String,
        #[arg(short = 'b', long = "base")]
        base: Option<String>,
        #[arg(short = 's', long = "stack")]
        stack: bool,
    },
    Switch {
        name: String,
    },
    List {
        #[arg(long)]
        format: Option<String>,
        #[arg(long)]
        full: bool,
        #[arg(long)]
        sync: bool,
    },
    Prune {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        debug: bool,
    },
    #[command(alias = "rm")]
    Remove {
        name: String,
        #[arg(short = 'f', long = "force")]
        force: bool,
    },
    Ci {
        #[arg(short = 'w', long = "watch")]
        watch: bool,
        #[arg(short = 'l', long = "logs")]
        logs: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ShellCommand {
    Init {
        shell: Option<String>,
    },
    Install {
        shell: Option<String>,
    },
    Directive {
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
}
