use clap::{Parser, Subcommand};
use std::path::PathBuf;

use loopflow_engine::git::{create_branch, push, rebase, LandStrategy};
use loopflow_engine::GitError;

#[derive(Parser)]
#[command(name = "lf-engine", about = "Loopflow engine CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Rebase {
        #[arg(long)]
        worktree: PathBuf,
        #[arg(long)]
        onto: String,
        #[arg(long)]
        base_commit: Option<String>,
    },
    Push {
        #[arg(long)]
        worktree: PathBuf,
        #[arg(long)]
        force_with_lease: bool,
    },
    Branch {
        #[arg(long)]
        worktree: PathBuf,
        #[arg(long)]
        name: String,
    },
    Land {
        #[arg(long)]
        worktree: PathBuf,
        #[arg(long)]
        strategy: LandStrategy,
        #[arg(long, default_value = "main")]
        main_branch: String,
    },
}

fn print_json<T: serde::Serialize>(value: &T) {
    let payload = serde_json::to_string(value).expect("serialize result");
    println!("{}", payload);
}

fn print_error(err: &GitError) -> ! {
    let payload = serde_json::to_string(err).expect("serialize error");
    eprintln!("{}", payload);
    std::process::exit(1);
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Rebase {
            worktree,
            onto,
            base_commit,
        } => match rebase(&worktree, &onto, base_commit.as_deref()) {
            Ok(result) => print_json(&result),
            Err(err) => print_error(&err),
        },
        Commands::Push {
            worktree,
            force_with_lease,
        } => match push(&worktree, force_with_lease) {
            Ok(()) => print_json(&serde_json::Value::Null),
            Err(err) => print_error(&err),
        },
        Commands::Branch { worktree, name } => match create_branch(&worktree, &name) {
            Ok(result) => print_json(&result),
            Err(err) => print_error(&err),
        },
        Commands::Land {
            worktree,
            strategy,
            main_branch,
        } => match loopflow_engine::git::land(&worktree, strategy, &main_branch) {
            Ok(result) => print_json(&result),
            Err(err) => print_error(&err),
        },
    }
}
