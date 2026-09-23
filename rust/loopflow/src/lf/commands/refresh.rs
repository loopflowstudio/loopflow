//! Public refresh orchestration; promotion remains the installer transaction.

use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use crate::engine::worktrees::main_repo_root;
use crate::lf::commands::ops::{refresh_required_packages, CliProgress};
use crate::lf::commands::util::find_repo_root;
use crate::ops::checkout::refresh_main;

fn run_command(repo: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new(args[0])
        .args(&args[1..])
        .current_dir(repo)
        .status()
        .with_context(|| format!("starting {}", args.join(" ")))?;
    if !status.success() {
        return Err(anyhow!(
            "{} failed ({status}); fix the error above and rerun `lf install`",
            args.join(" ")
        ));
    }
    Ok(())
}

pub fn run() -> Result<()> {
    let caller = find_repo_root()?;
    refresh_main(&caller, &CliProgress)?;
    let repo = main_repo_root(&caller)?;
    refresh_required_packages()?;
    // uv sync is convergent: an unchanged lock/environment installs nothing.
    run_command(&repo, &["uv", "sync", "--locked"])?;
    run_command(
        &repo,
        &[
            "uv",
            "run",
            "--locked",
            "python",
            "scripts/install.py",
            "refresh",
        ],
    )
}

pub fn schedule() -> Result<()> {
    let repo = main_repo_root(&find_repo_root()?)?;
    run_command(
        &repo,
        &[
            "uv",
            "run",
            "--locked",
            "python",
            "scripts/install.py",
            "schedule",
        ],
    )
}
