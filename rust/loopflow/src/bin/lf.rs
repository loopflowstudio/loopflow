use clap::Parser;
use tracing::debug;
use tracing_subscriber::EnvFilter;

use loopflow::journal::{self, LfEventFields, LfEventType, LfNode};
use loopflow::lf::{Cli, Commands};

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
    "--no-direction",
    "--yolo",
    "-i",
    "--interactive",
    "-b",
    "--batch",
    "--web",
    "--chrome",
    "--no-chrome",
    "--lfdocs",
    "--no-lfdocs",
    "--diff-files",
    "--no-diff-files",
    "--diff",
    "--no-diff",
    "-h",
    "--help",
    "-V",
    "--version",
];

/// Known subcommands that should not be treated as step names.
const KNOWN_COMMANDS: &[&str] = &[":", "op", "help"];

fn is_value_flag(arg: &str) -> bool {
    VALUE_FLAGS.contains(&arg)
}

fn is_known_flag(arg: &str) -> bool {
    BOOL_FLAGS.contains(&arg) || is_value_flag(arg)
}

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
                if is_value_flag(arg) && i + 1 < rest.len() {
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
                if is_known_flag(arg) {
                    flags_after.push(arg.clone());
                    if is_value_flag(arg) && i + 1 < rest.len() {
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

fn join_args(args: &[String]) -> Option<String> {
    if args.is_empty() {
        None
    } else {
        Some(args.join(" "))
    }
}

fn with_runtime<T>(
    repo_root: &std::path::Path,
    command: &[String],
    run: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    journal::emit(
        repo_root,
        LfNode::Run,
        LfEventType::Started,
        LfEventFields {
            wave_name: loopflow::engine::worktrees::main_repo_root(repo_root)
                .ok()
                .and_then(|main_repo| {
                    loopflow::engine::worktrees::wave_name_from_worktree_and_main(
                        repo_root, &main_repo,
                    )
                }),
            worktree: Some(repo_root.display().to_string()),
            command: Some(command.to_vec()),
            ..LfEventFields::default()
        },
    );
    let result = run();
    match &result {
        Ok(_) => journal::emit(
            repo_root,
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        ),
        Err(err) => journal::emit(
            repo_root,
            LfNode::Run,
            LfEventType::Errored,
            LfEventFields {
                error: Some(err.to_string()),
                ..LfEventFields::default()
            },
        ),
    }
    result
}

fn in_repo_runtime<T>(
    command: &[String],
    run: impl FnOnce(&std::path::Path) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let repo_root = loopflow::lf::commands::util::find_repo_root()?;
    with_runtime(&repo_root, command, || run(&repo_root))
}

fn run_target(
    name: &str,
    message: Option<&str>,
    cli: &Cli,
    command: &[String],
) -> anyhow::Result<()> {
    let repo_root = loopflow::lf::commands::util::find_repo_root()?;
    match loopflow::lf::discovery::discover_target(&repo_root, name)? {
        loopflow::lf::discovery::Target::Step(_) => with_runtime(&repo_root, command, || {
            loopflow::lf::commands::run::run(Some(name), message, cli)
        }),
        loopflow::lf::discovery::Target::Flow(flow) => with_runtime(&repo_root, command, || {
            loopflow::lf::commands::flow::run(&flow, message, cli, &repo_root)
        }),
    }
}

fn main() -> anyhow::Result<()> {
    // Ensure Ctrl+C terminates lf and the child agent. Without this,
    // child.wait() retries on EINTR and hangs while the agent catches
    // SIGINT and keeps running. SIGTERM the agent first so it doesn't
    // survive as an orphan.
    ctrlc::set_handler(|| {
        loopflow::engine::agent::kill_child_if_running();
        std::process::exit(130);
    })
    .expect("failed to set Ctrl+C handler");

    // Initialize tracing with RUST_LOG env filter
    // Usage: RUST_LOG=lf=debug lf debug
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("lf=info,loopflow=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .without_time()
        .init();

    // Reorder args so flags can appear after the step name
    let args: Vec<String> = std::env::args().collect();
    let args = reorder_args(args);

    let cli = Cli::parse_from(args.clone());
    debug!(?cli, "parsed CLI arguments");

    if cli.list {
        return in_repo_runtime(&args, |_| loopflow::lf::commands::list::show_all());
    }

    match &cli.command {
        Some(Commands::Inline { prompt }) => {
            let text = prompt.join(" ");
            in_repo_runtime(&args, |_| {
                loopflow::lf::commands::run::run(None, Some(&text), &cli)
            })
        }
        Some(Commands::Op { op }) => {
            in_repo_runtime(&args, |_| loopflow::lf::commands::ops::run(op))
        }
        Some(Commands::External(external_args)) => {
            let (name, step_args) = loopflow::lf::commands::run::split_step_args(external_args)?;
            let message = join_args(&step_args);
            run_target(&name, message.as_deref(), &cli, &args)
        }
        None => in_repo_runtime(&args, |_| {
            loopflow::lf::commands::run::run(None, None, &cli)
        }),
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
    fn reorder_args_no_direction_flag_after_step() {
        let args = vec![
            "lf".to_string(),
            "implement".to_string(),
            "--no-direction".to_string(),
        ];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "--no-direction", "implement"]);
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
            "op".to_string(),
            "commit".to_string(),
            "-m".to_string(),
            "msg".to_string(),
        ];
        let result = reorder_args(args);
        // Known commands should not be reordered
        assert_eq!(result, vec!["lf", "op", "commit", "-m", "msg"]);
    }

    #[test]
    fn reorder_args_no_step() {
        let args = vec!["lf".to_string(), "-l".to_string()];
        let result = reorder_args(args);
        assert_eq!(result, vec!["lf", "-l"]);
    }
}
