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
pub struct CombineOptions {
    pub wave_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CombineResult {
    pub new_pr_url: Option<String>,
    pub closed_prs: Vec<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct GhOpenPr {
    number: u64,
    #[serde(rename = "headRefName")]
    head_ref_name: String,
}

pub fn combine_prs(
    repo: &Path,
    options: &CombineOptions,
    progress: &impl Progress,
) -> OpsResult<CombineResult> {
    ensure_gh_available()?;

    if !is_clean(repo)? {
        return Err(OpsError::Message(
            "uncommitted changes; commit or stash before combining PRs".to_string(),
        ));
    }

    let wave_name = options
        .wave_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| OpsError::Message("wave name is required for combine".to_string()))?;

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
            "need at least 2 open PRs to combine".to_string(),
        ));
    }

    let mut commits = Vec::new();
    let mut seen = HashSet::new();
    for pr in &open_prs {
        let branch_commits = commit_range_from_ref(
            repo,
            &format!("origin/{base_branch}"),
            &format!("origin/{}", pr.head_ref_name),
        )?;
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

    let branch_base = format!("{}-combined", sanitize_for_branch(wave_name));
    let combined_branch = unique_branch_name(repo, &branch_base)?;
    let original_branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;

    progress.status(&format!(
        "Creating {combined_branch} from origin/{base_branch}..."
    ));
    run_git(
        repo,
        [
            "checkout",
            "-b",
            &combined_branch,
            &format!("origin/{base_branch}"),
        ],
    )?;

    if let Err(err) = cherry_pick_commits(repo, &commits) {
        let _ = run_git(repo, ["checkout", &original_branch]);
        let _ = delete_local_branch(repo, &combined_branch);
        return Err(err);
    }

    progress.status("Pushing combined branch...");
    run_git(repo, ["push", "-u", "origin", &combined_branch])?;

    let title = format!("Combine PRs for {wave_name}");
    let body = combine_pr_body(&open_prs);

    progress.status("Creating combined PR...");
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
            &combined_branch,
        ],
    )?;
    let pr_url = output_stdout(&pr_output);
    let new_pr_url = if pr_url.is_empty() {
        None
    } else {
        Some(pr_url)
    };

    // Update the combined PR with an LLM-generated title and description.
    update_combined_pr_message(repo, progress);

    progress.status("Closing previous PRs...");
    let mut closed_prs = Vec::new();
    for pr in &open_prs {
        close_pr(repo, pr.number)?;
        closed_prs.push(pr.number);
    }

    Ok(CombineResult {
        new_pr_url,
        closed_prs,
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
            "number,headRefName",
        ],
    )?;
    let stdout = output_stdout(&output);
    serde_json::from_str::<Vec<GhOpenPr>>(&stdout)
        .map_err(|err| OpsError::Parse(format!("failed to parse gh pr list: {err}")))
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
        if let Err(err) = run_git(repo, ["cherry-pick", sha]) {
            let _ = run_git(repo, ["cherry-pick", "--abort"]);
            match err {
                OpsError::CommandFailed { stderr, .. } => {
                    return Err(OpsError::Message(format!(
                        "cherry-pick conflict while applying {sha}: {stderr}"
                    )));
                }
                other => return Err(other),
            }
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
    let number = number.to_string();
    match run_gh(repo, ["pr", "close", &number, "--delete-branch"]) {
        Ok(_) => Ok(()),
        Err(OpsError::CommandFailed { command, stderr }) => {
            let lowered = stderr.to_lowercase();
            if lowered.contains("already closed") || lowered.contains("pull request is closed") {
                Ok(())
            } else {
                Err(OpsError::CommandFailed { command, stderr })
            }
        }
        Err(err) => Err(err),
    }
}

fn combine_pr_body(prs: &[GhOpenPr]) -> String {
    let mut body = String::from("This PR combines the following open PRs:\n");
    for pr in prs {
        body.push_str(&format!("- #{} ({})\n", pr.number, pr.head_ref_name));
    }
    body
}

fn run_git<const N: usize>(repo: &Path, args: [&str; N]) -> OpsResult<Output> {
    run_command(repo, "git", args)
}

fn run_gh<const N: usize>(repo: &Path, args: [&str; N]) -> OpsResult<Output> {
    run_command(repo, "gh", args)
}

fn run_command<const N: usize>(repo: &Path, program: &str, args: [&str; N]) -> OpsResult<Output> {
    let output = Command::new(program)
        .args(args)
        .current_dir(repo)
        .output()?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(OpsError::CommandFailed {
            command: format!("{program} {}", args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

fn update_combined_pr_message(repo: &Path, progress: &impl Progress) {
    progress.status("Generating PR title...");
    match crate::ops::generate_pr_message(repo, None) {
        Ok(message) => {
            if let Err(err) = run_gh(
                repo,
                [
                    "pr",
                    "edit",
                    "--title",
                    &message.title,
                    "--body",
                    &message.body,
                ],
            ) {
                progress.status(&format!("Warning: failed to update PR title: {err}"));
            }
        }
        Err(err) => {
            progress.status(&format!("Warning: failed to generate PR message: {err}"));
        }
    }
}

fn output_stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
