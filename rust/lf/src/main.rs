use clap::Parser;
use tracing::debug;
use tracing_subscriber::EnvFilter;

use lf::{Cli, Commands};

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
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("lf=info,loopflow_engine=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .without_time()
        .init();

    // Reorder args so flags can appear after the step name
    let args: Vec<String> = std::env::args().collect();
    let args = reorder_args(args);

    let cli = Cli::parse_from(args);
    debug!(?cli, "parsed CLI arguments");

    if cli.list {
        return lf::commands::list::show_all();
    }

    match &cli.command {
        Some(Commands::Run { name, args }) => lf::commands::run::run(Some(name), args, None, &cli),
        Some(Commands::Inline { prompt }) => {
            let text = prompt.join(" ");
            lf::commands::run::run(None, &[], Some(&text), &cli)
        }
        Some(Commands::Ops { op }) => lf::commands::ops::run(op),
        Some(Commands::External(args)) => {
            let (name, step_args) = lf::commands::run::split_step_args(args)?;
            lf::commands::run::run(Some(&name), &step_args, None, &cli)
        }
        None => lf::commands::run::run(None, &[], None, &cli),
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
    fn reorder_args_value_flag_before_step() {
        // lf -m codex implement -> should stay the same (already correct order)
        let args = vec![
            "lf".to_string(),
            "-m".to_string(),
            "codex".to_string(),
            "implement".to_string(),
        ];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-m", "codex", "implement"]);
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
