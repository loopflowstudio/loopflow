use crate::commands::util::find_repo_root;
use crate::OpsCommand;
use anyhow::{anyhow, Result};
use loopflow_engine::git::{
    commit, current_branch, delete_local_branch, get_default_branch, land, pr_create_draft, push,
    push_with_upstream, rebase, sync_main, LandStrategy,
};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn run(op: &OpsCommand) -> Result<()> {
    match op {
        OpsCommand::Rebase { onto } => rebase_current(onto.as_deref()),
        OpsCommand::Push { force } => push_current(*force),
        OpsCommand::Land { strategy } => land_current(strategy.as_deref()),
        OpsCommand::Pr { title, draft } => open_pr(title.as_deref(), *draft),
        OpsCommand::Sync => sync_current(),
        OpsCommand::Next => next_branch(),
        OpsCommand::Commit { message } => commit_current(message.as_deref()),
        OpsCommand::Abandon { force } => abandon_current(*force),
    }
}

fn rebase_current(onto: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let base = get_default_branch(&repo_root)?;
    let onto_ref = onto
        .map(|value| value.to_string())
        .unwrap_or_else(|| format!("origin/{base}"));
    let result = rebase(&repo_root, &onto_ref, None)?;
    if !result.success {
        return Err(anyhow!("rebase failed"));
    }
    Ok(())
}

fn push_current(force: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    push(&repo_root, force).map_err(Into::into)
}

fn land_current(strategy: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_branch = get_default_branch(&repo_root)?;
    let land_strategy = match strategy {
        Some("local") | Some("merge") => LandStrategy::LocalMerge,
        Some("squash") | Some("squash_merge") => LandStrategy::SquashMerge,
        _ => LandStrategy::SquashMerge,
    };
    let _ = land(&repo_root, land_strategy, &main_branch)?;
    Ok(())
}

fn open_pr(title: Option<&str>, draft: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    if draft {
        let url = pr_create_draft(&repo_root)?;
        println!("{}", url);
        return Ok(());
    }

    let mut cmd = Command::new("gh");
    cmd.arg("pr").arg("create").arg("--fill");
    if let Some(title) = title {
        cmd.arg("--title").arg(title);
    }

    let status = cmd.current_dir(&repo_root).status()?;
    if !status.success() {
        return Err(anyhow!("gh pr create failed"));
    }
    Ok(())
}

fn sync_current() -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_branch = get_default_branch(&repo_root)?;
    let ok = sync_main(&repo_root, &main_branch)?;
    if !ok {
        return Err(anyhow!("working tree dirty; sync aborted"));
    }
    Ok(())
}

fn next_branch() -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_branch = get_default_branch(&repo_root)?;
    let _ = land(&repo_root, LandStrategy::SquashMerge, &main_branch)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let next_name = format!("next-{}", timestamp);

    let status = Command::new("git")
        .arg("checkout")
        .arg(&main_branch)
        .current_dir(&repo_root)
        .status()?;
    if !status.success() {
        return Err(anyhow!("failed to checkout {}", main_branch));
    }

    let status = Command::new("git")
        .arg("checkout")
        .arg("-b")
        .arg(&next_name)
        .current_dir(&repo_root)
        .status()?;
    if !status.success() {
        return Err(anyhow!("failed to create branch {}", next_name));
    }

    push_with_upstream(&repo_root, "origin", &next_name)?;
    Ok(())
}

fn commit_current(message: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let message = message.ok_or_else(|| anyhow!("commit message required"))?;
    commit(&repo_root, message).map_err(Into::into)
}

fn abandon_current(force: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_branch = get_default_branch(&repo_root)?;
    let branch = current_branch(&repo_root)?;

    if branch.as_deref() == Some(&main_branch) {
        println!("Already on {}", main_branch);
        return Ok(());
    }

    let status = Command::new("git")
        .arg("checkout")
        .arg(&main_branch)
        .current_dir(&repo_root)
        .status()?;
    if !status.success() {
        return Err(anyhow!("failed to checkout {}", main_branch));
    }

    if let Some(branch) = branch {
        if force {
            delete_local_branch(&repo_root, &branch)?;
        } else {
            let status = Command::new("git")
                .arg("branch")
                .arg("-d")
                .arg(&branch)
                .current_dir(&repo_root)
                .status()?;
            if !status.success() {
                return Err(anyhow!("failed to delete branch {}", branch));
            }
        }
    }

    Ok(())
}
