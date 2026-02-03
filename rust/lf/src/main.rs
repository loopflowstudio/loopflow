use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::debug;
use tracing_subscriber::EnvFilter;

mod commands;
mod discovery;
mod output;

#[derive(Parser, Debug)]
#[command(name = "lf")]
#[command(about = "Run steps and flows with coding agents")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// List available steps and flows
    #[arg(short, long)]
    list: bool,

    /// Direction(s) to apply (repeatable or comma-separated)
    #[arg(
        short = 'd',
        long = "direction",
        value_delimiter = ',',
        short_alias = 'D'
    )]
    direction: Vec<String>,

    /// Area scope (paths to include in context)
    #[arg(short = 'a', long = "area", short_alias = 'A')]
    area: Vec<PathBuf>,

    /// Include clipboard content in prompt
    #[arg(short = 'c', long = "clipboard", short_alias = 'C')]
    clipboard: bool,

    /// Model to use (backend or backend:variant)
    #[arg(short = 'm', long = "model", short_alias = 'M')]
    model: Option<String>,

    /// Skip permission prompts
    #[arg(long)]
    yolo: bool,

    /// Run interactively
    #[arg(short = 'i', long = "interactive", short_alias = 'I')]
    interactive: bool,

    /// Run in batch/headless mode
    #[arg(short = 'b', long = "batch", short_alias = 'B')]
    batch: bool,

    /// Copy prompt to clipboard and open web client
    #[arg(long)]
    web: bool,

    /// Enable Chrome integration (Claude)
    #[arg(long)]
    chrome: bool,

    /// Disable Chrome integration (Claude)
    #[arg(long = "no-chrome", overrides_with = "chrome")]
    no_chrome: bool,

    /// Wave name for roadmap scoping
    #[arg(short = 'w', long = "wave", short_alias = 'W')]
    wave: Option<String>,
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
enum Commands {
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
    Rebase {
        onto: Option<String>,
    },
    Push {
        #[arg(long)]
        force: bool,
    },
    Land {
        #[arg(long)]
        strategy: Option<String>,
    },
    Pr {
        title: Option<String>,
        #[arg(long)]
        draft: bool,
    },
    Sync,
    Next,
    Commit {
        #[arg(short = 'm', long = "message", short_alias = 'M')]
        message: Option<String>,
    },
    Abandon {
        #[arg(long)]
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

/// Flags that take a value (next arg is the value).
const VALUE_FLAGS: &[&str] = &[
    "-d",
    "--direction",
    "-a",
    "--area",
    "-m",
    "--model",
    "-w",
    "--wave",
];

/// Flags that are boolean (no value).
const BOOL_FLAGS: &[&str] = &[
    "-l",
    "--list",
    "-c",
    "--clipboard",
    "--yolo",
    "-i",
    "--interactive",
    "-b",
    "--batch",
    "--web",
    "--chrome",
    "--no-chrome",
    "-h",
    "--help",
    "-V",
    "--version",
];

/// Known subcommands that should not be treated as step names.
const KNOWN_COMMANDS: &[&str] = &["run", ":", "ops", "help"];

/// Reorder args so flags come before the step/flow name.
/// This allows `lf debug -c` to work like `lf -c debug`.
fn reorder_args(args: Vec<String>) -> Vec<String> {
    if args.len() <= 1 {
        return args;
    }

    let program = args[0].clone();
    let rest = &args[1..];

    // If first arg is a known command, don't reorder
    if let Some(first) = rest.first() {
        if KNOWN_COMMANDS.contains(&first.as_str()) {
            return args;
        }
    }

    // Find where the step name is and collect flags that come after it
    let mut flags_before: Vec<String> = Vec::new();
    let mut step_and_args: Vec<String> = Vec::new();
    let mut flags_after: Vec<String> = Vec::new();

    let mut i = 0;
    let mut found_step = false;

    while i < rest.len() {
        let arg = &rest[i];

        if !found_step {
            if arg.starts_with('-') {
                // It's a flag before the step
                flags_before.push(arg.clone());
                if VALUE_FLAGS.contains(&arg.as_str()) && i + 1 < rest.len() {
                    i += 1;
                    flags_before.push(rest[i].clone());
                }
            } else {
                // Found the step name
                found_step = true;
                step_and_args.push(arg.clone());
            }
        } else {
            // After the step name
            if arg.starts_with('-') {
                // Check if it's a known lf flag
                if BOOL_FLAGS.contains(&arg.as_str()) || VALUE_FLAGS.contains(&arg.as_str()) {
                    flags_after.push(arg.clone());
                    if VALUE_FLAGS.contains(&arg.as_str()) && i + 1 < rest.len() {
                        i += 1;
                        flags_after.push(rest[i].clone());
                    }
                } else {
                    // Unknown flag - treat as step arg
                    step_and_args.push(arg.clone());
                }
            } else {
                // Non-flag after step - it's a step arg
                step_and_args.push(arg.clone());
            }
        }
        i += 1;
    }

    // Reconstruct: program + flags_before + flags_after + step_and_args
    let mut result = vec![program];
    result.extend(flags_before);
    result.extend(flags_after);
    result.extend(step_and_args);
    result
}

fn main() -> anyhow::Result<()> {
    // Initialize tracing with RUST_LOG env filter
    // Usage: RUST_LOG=lf=debug lf debug
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("lf=warn".parse().expect("valid directive")),
        )
        .with_writer(std::io::stderr)
        .without_time()
        .init();

    // Reorder args so flags can appear after the step name
    let args: Vec<String> = std::env::args().collect();
    let args = reorder_args(args);

    let cli = Cli::parse_from(args);
    debug!(?cli, "parsed CLI arguments");

    if cli.list {
        return commands::list::show_all();
    }

    match &cli.command {
        Some(Commands::Run { name, args }) => commands::step::run(name, args, &cli),
        Some(Commands::Inline { prompt }) => commands::inline::run(prompt, &cli),
        Some(Commands::Ops { op }) => commands::ops::run(op),
        Some(Commands::External(args)) => {
            let (name, step_args) = commands::step::split_step_args(args)?;
            commands::step::run(&name, &step_args, &cli)
        }
        None => commands::step::run_interactive(&cli),
    }
}

#[cfg(test)]
mod tests {
    use super::reorder_args;

    #[test]
    fn reorder_args_flag_after_step() {
        let args = vec!["lf".to_string(), "debug".to_string(), "-c".to_string()];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-c", "debug"]);
    }

    #[test]
    fn reorder_args_flag_before_step() {
        let args = vec!["lf".to_string(), "-c".to_string(), "debug".to_string()];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-c", "debug"]);
    }

    #[test]
    fn reorder_args_value_flag_after_step() {
        let args = vec![
            "lf".to_string(),
            "debug".to_string(),
            "-m".to_string(),
            "codex".to_string(),
        ];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-m", "codex", "debug"]);
    }

    #[test]
    fn reorder_args_mixed_flags() {
        let args = vec![
            "lf".to_string(),
            "-i".to_string(),
            "implement".to_string(),
            "-c".to_string(),
            "-m".to_string(),
            "claude".to_string(),
        ];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-i", "-c", "-m", "claude", "implement"]);
    }

    #[test]
    fn reorder_args_step_with_args() {
        let args = vec![
            "lf".to_string(),
            "implement:".to_string(),
            "add".to_string(),
            "logout".to_string(),
            "-c".to_string(),
        ];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-c", "implement:", "add", "logout"]);
    }

    #[test]
    fn reorder_args_known_command_unchanged() {
        let args = vec![
            "lf".to_string(),
            "ops".to_string(),
            "commit".to_string(),
            "-m".to_string(),
            "msg".to_string(),
        ];
        let result = reorder_args(args);
        // Known commands should not be reordered
        assert_eq!(result, vec!["lf", "ops", "commit", "-m", "msg"]);
    }

    #[test]
    fn reorder_args_no_step() {
        let args = vec!["lf".to_string(), "-l".to_string()];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-l"]);
    }
}
