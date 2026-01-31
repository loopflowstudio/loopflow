use clap::{Parser, Subcommand};
use std::path::PathBuf;

use loopflow_engine::git::{
    commit, create_branch, delete_local_branch, delete_remote_branch, get_default_branch, is_clean,
    pr_create_draft, pr_exists, pr_merge_squash_auto, push, push_with_upstream, rebase, stage_all,
    sync_main, worktree_remove, LandStrategy,
};
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
    PushWithUpstream {
        #[arg(long)]
        worktree: PathBuf,
        #[arg(long)]
        remote: String,
        #[arg(long)]
        branch: String,
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
    DefaultBranch {
        #[arg(long)]
        repo: PathBuf,
    },
    IsClean {
        #[arg(long)]
        repo: PathBuf,
    },
    StageAll {
        #[arg(long)]
        repo: PathBuf,
    },
    Commit {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        message: String,
    },
    DeleteRemoteBranch {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        remote: String,
        #[arg(long)]
        branch: String,
    },
    DeleteLocalBranch {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        branch: String,
    },
    PrExists {
        #[arg(long)]
        repo: PathBuf,
    },
    PrCreateDraft {
        #[arg(long)]
        repo: PathBuf,
    },
    PrMergeSquashAuto {
        #[arg(long)]
        repo: PathBuf,
    },
    SyncMain {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long, default_value = "main")]
        main_branch: String,
    },
    WorktreeRemove {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        path: PathBuf,
    },
    Version,
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
        Commands::PushWithUpstream {
            worktree,
            remote,
            branch,
        } => match push_with_upstream(&worktree, &remote, &branch) {
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
        Commands::DefaultBranch { repo } => match get_default_branch(&repo) {
            Ok(result) => print_json(&result),
            Err(err) => print_error(&err),
        },
        Commands::IsClean { repo } => match is_clean(&repo) {
            Ok(result) => print_json(&result),
            Err(err) => print_error(&err),
        },
        Commands::StageAll { repo } => match stage_all(&repo) {
            Ok(()) => print_json(&serde_json::Value::Null),
            Err(err) => print_error(&err),
        },
        Commands::Commit { repo, message } => match commit(&repo, &message) {
            Ok(()) => print_json(&serde_json::Value::Null),
            Err(err) => print_error(&err),
        },
        Commands::DeleteRemoteBranch {
            repo,
            remote,
            branch,
        } => match delete_remote_branch(&repo, &remote, &branch) {
            Ok(()) => print_json(&serde_json::Value::Null),
            Err(err) => print_error(&err),
        },
        Commands::DeleteLocalBranch { repo, branch } => match delete_local_branch(&repo, &branch) {
            Ok(()) => print_json(&serde_json::Value::Null),
            Err(err) => print_error(&err),
        },
        Commands::PrExists { repo } => match pr_exists(&repo) {
            Ok(result) => print_json(&result),
            Err(err) => print_error(&err),
        },
        Commands::PrCreateDraft { repo } => match pr_create_draft(&repo) {
            Ok(result) => print_json(&result),
            Err(err) => print_error(&err),
        },
        Commands::PrMergeSquashAuto { repo } => match pr_merge_squash_auto(&repo) {
            Ok(()) => print_json(&serde_json::Value::Null),
            Err(err) => print_error(&err),
        },
        Commands::SyncMain {
            repo,
            main_branch,
        } => match sync_main(&repo, &main_branch) {
            Ok(result) => print_json(&result),
            Err(err) => print_error(&err),
        },
        Commands::WorktreeRemove { repo, path } => match worktree_remove(&repo, &path) {
            Ok(()) => print_json(&serde_json::Value::Null),
            Err(err) => print_error(&err),
        },
        Commands::Version => print_json(&env!("CARGO_PKG_VERSION")),
    }
}
