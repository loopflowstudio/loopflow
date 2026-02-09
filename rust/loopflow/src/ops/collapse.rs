use std::collections::HashSet;
use std::path::Path;
use std::process::{Command, Output};

use serde::Deserialize;

use crate::engine::git::{current_branch, delete_local_branch, get_default_branch, is_clean};
use crate::engine::naming::sanitize_for_branch;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::util::command_exists;

#[derive(Debug, Clone, Default)]
pub struct CollapseOptions {
    pub wave_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CollapseResult {
    pub new_pr_url: Option<String>,
    pub closed_prs: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct AbsorbOptions {
    pub target_pr_number: u64,
}

#[derive(Debug, Clone)]
pub struct AbsorbResult {
    pub target_branch: String,
    pub commits_absorbed: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct GhOpenPr {
    number: u64,
    #[serde(rename = "headRefName")]
    head_ref_name: String,
}

pub fn collapse_prs(
    repo: &Path,
    options: &CollapseOptions,
    progress: &impl Progress,
) -> OpsResult<CollapseResult> {
    ensure_gh_available()?;

    if !is_clean(repo)? {
        return Err(OpsError::Message(
            "uncommitted changes; commit or stash before collapsing PRs".to_string(),
        ));
    }

    let wave_name = options
        .wave_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| OpsError::Message("wave name is required for collapse".to_string()))?;

    let base_branch = get_default_branch(repo)?;
    run_git(repo, ["fetch", "origin", &base_branch])?;

    let wave_branch_token = sanitize_for_branch(wave_name);
    let mut open_prs = list_open_prs(repo)?
        .into_iter()
        .filter(|pr| pr.head_ref_name.contains(&wave_branch_token))
        .collect::<Vec<_>>();

    open_prs.sort_by_key(|pr| pr.number);

    if open_prs.len() < 2 {
        return Err(OpsError::Message(
            "need at least 2 open PRs to collapse".to_string(),
        ));
    }

    let mut commits = Vec::new();
    let mut seen = HashSet::new();
    for pr in &open_prs {
        let branch_commits = commit_range(repo, &base_branch, &pr.head_ref_name)?;
        for sha in branch_commits {
            if seen.insert(sha.clone()) {
                commits.push(sha);
            }
        }
    }

    if commits.is_empty() {
        return Err(OpsError::Message(
            "no commits found across open PR branches".to_string(),
        ));
    }

    let branch_base = format!("{}-collapsed", sanitize_for_branch(wave_name));
    let collapsed_branch = unique_branch_name(repo, &branch_base)?;
    let original_branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;

    progress.status(&format!(
        "Creating {collapsed_branch} from origin/{base_branch}..."
    ));
    run_git(
        repo,
        [
            "checkout",
            "-b",
            &collapsed_branch,
            &format!("origin/{base_branch}"),
        ],
    )?;

    if let Err(err) = cherry_pick_commits(repo, &commits) {
        let _ = run_git(repo, ["checkout", &original_branch]);
        let _ = delete_local_branch(repo, &collapsed_branch);
        return Err(err);
    }

    progress.status("Pushing collapsed branch...");
    run_git(repo, ["push", "-u", "origin", &collapsed_branch])?;

    let title = format!("Collapse PR stack for {wave_name}");
    let body = collapse_pr_body(&open_prs);

    progress.status("Creating collapsed PR...");
    let pr_output = run_gh(
        repo,
        [
            "pr",
            "create",
            "--title",
            &title,
            "--body",
            &body,
            "--base",
            &base_branch,
            "--head",
            &collapsed_branch,
        ],
    )?;
    let pr_url = output_stdout(&pr_output);
    let new_pr_url = if pr_url.is_empty() {
        None
    } else {
        Some(pr_url)
    };

    progress.status("Closing previous PRs...");
    let mut closed_prs = Vec::new();
    for pr in &open_prs {
        close_pr(repo, pr.number)?;
        closed_prs.push(pr.number);
    }

    Ok(CollapseResult {
        new_pr_url,
        closed_prs,
    })
}

pub fn absorb_into_pr(
    repo: &Path,
    options: &AbsorbOptions,
    progress: &impl Progress,
) -> OpsResult<AbsorbResult> {
    ensure_gh_available()?;

    if !is_clean(repo)? {
        return Err(OpsError::Message(
            "uncommitted changes; commit or stash before absorbing".to_string(),
        ));
    }

    let original_branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    let target_branch = pr_head_branch(repo, options.target_pr_number)?;

    progress.status(&format!("Fetching target branch origin/{target_branch}..."));
    run_git(repo, ["fetch", "origin", &target_branch])?;

    let commits = commit_range_from_ref(repo, &format!("origin/{target_branch}"), "HEAD")?;

    checkout_target_branch(repo, &target_branch)?;

    if let Err(err) = cherry_pick_commits(repo, &commits) {
        let _ = run_git(repo, ["checkout", &original_branch]);
        return Err(err);
    }

    if !commits.is_empty() {
        progress.status("Pushing absorbed commits...");
        run_git(repo, ["push", "origin", &target_branch])?;
    }

    Ok(AbsorbResult {
        target_branch,
        commits_absorbed: commits.len(),
    })
}

fn ensure_gh_available() -> OpsResult<()> {
    if !command_exists("gh") {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }
    Ok(())
}

fn list_open_prs(repo: &Path) -> OpsResult<Vec<GhOpenPr>> {
    let output = run_gh(
        repo,
        [
            "pr",
            "list",
            "--author",
            "@me",
            "--state",
            "open",
            "--json",
            "number,headRefName,url",
        ],
    )?;
    let stdout = output_stdout(&output);
    serde_json::from_str::<Vec<GhOpenPr>>(&stdout)
        .map_err(|err| OpsError::Parse(format!("failed to parse gh pr list: {err}")))
}

fn commit_range(repo: &Path, base_branch: &str, branch: &str) -> OpsResult<Vec<String>> {
    commit_range_from_ref(
        repo,
        &format!("origin/{base_branch}"),
        &format!("origin/{branch}"),
    )
}

fn commit_range_from_ref(repo: &Path, base_ref: &str, head_ref: &str) -> OpsResult<Vec<String>> {
    let output = run_git(
        repo,
        [
            "log",
            "--reverse",
            "--format=%H",
            &format!("{base_ref}..{head_ref}"),
        ],
    )?;
    Ok(output_stdout(&output)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect())
}

fn cherry_pick_commits(repo: &Path, commits: &[String]) -> OpsResult<()> {
    for sha in commits {
        let output = Command::new("git")
            .args(["cherry-pick", sha])
            .current_dir(repo)
            .output()?;
        if !output.status.success() {
            let _ = Command::new("git")
                .args(["cherry-pick", "--abort"])
                .current_dir(repo)
                .output();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(OpsError::Message(format!(
                "cherry-pick conflict while applying {sha}: {stderr}"
            )));
        }
    }
    Ok(())
}

fn unique_branch_name(repo: &Path, base: &str) -> OpsResult<String> {
    let mut candidate = base.to_string();
    let mut suffix = 2;

    while branch_exists(repo, &candidate)? {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }

    Ok(candidate)
}

fn branch_exists(repo: &Path, branch: &str) -> OpsResult<bool> {
    let local_ref = format!("refs/heads/{branch}");
    let remote_ref = format!("refs/remotes/origin/{branch}");

    Ok(ref_exists(repo, &local_ref)? || ref_exists(repo, &remote_ref)?)
}

fn ref_exists(repo: &Path, reference: &str) -> OpsResult<bool> {
    let status = Command::new("git")
        .args(["show-ref", "--verify", "--quiet", reference])
        .current_dir(repo)
        .status()?;
    Ok(status.success())
}

fn close_pr(repo: &Path, number: u64) -> OpsResult<()> {
    let output = Command::new("gh")
        .args(["pr", "close", &number.to_string(), "--delete-branch"])
        .current_dir(repo)
        .output()?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let lowered = stderr.to_lowercase();
    if lowered.contains("already closed") || lowered.contains("pull request is closed") {
        return Ok(());
    }

    Err(OpsError::CommandFailed {
        command: format!("gh pr close {} --delete-branch", number),
        stderr,
    })
}

fn collapse_pr_body(prs: &[GhOpenPr]) -> String {
    let mut body = String::from("This PR collapses the following open PR stack:\n");
    for pr in prs {
        body.push_str(&format!("- #{} ({})\n", pr.number, pr.head_ref_name));
    }
    body
}

fn pr_head_branch(repo: &Path, pr_number: u64) -> OpsResult<String> {
    let output = run_gh(
        repo,
        [
            "pr",
            "view",
            &pr_number.to_string(),
            "--json",
            "headRefName",
            "-q",
            ".headRefName",
        ],
    )?;

    let branch = output_stdout(&output);
    if branch.is_empty() {
        return Err(OpsError::Message(format!(
            "no head branch found for PR #{}",
            pr_number
        )));
    }

    Ok(branch)
}

fn checkout_target_branch(repo: &Path, target_branch: &str) -> OpsResult<()> {
    let checkout_existing = Command::new("git")
        .args(["checkout", target_branch])
        .current_dir(repo)
        .output()?;
    if checkout_existing.status.success() {
        return Ok(());
    }

    run_git(
        repo,
        [
            "checkout",
            "-b",
            target_branch,
            &format!("origin/{target_branch}"),
        ],
    )?;
    Ok(())
}

fn run_git<const N: usize>(repo: &Path, args: [&str; N]) -> OpsResult<Output> {
    let output = Command::new("git").args(args).current_dir(repo).output()?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(OpsError::CommandFailed {
            command: format!("git {}", args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

fn run_gh<const N: usize>(repo: &Path, args: [&str; N]) -> OpsResult<Output> {
    let output = Command::new("gh").args(args).current_dir(repo).output()?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(OpsError::CommandFailed {
            command: format!("gh {}", args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

fn output_stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
