use std::path::Path;
use std::process::{Command, Output};

use serde::Deserialize;

use crate::engine::agent::{launch_agent, LaunchConfig};
use crate::engine::builtins::get_builtin_ops_prompt;
use crate::engine::command::run_command;
use crate::engine::config::load_config_or_default;
use crate::engine::git::get_default_branch;
use crate::engine::worktrees::main_repo_root;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::util::command_exists;

/// A merged PR with enough context for release notes.
#[derive(Debug, Deserialize)]
struct MergedPr {
    number: u32,
    title: String,
    body: Option<String>,
}

pub fn release(repo: &Path, version: &str, progress: &impl Progress) -> OpsResult<String> {
    if !command_exists("gh") {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    let version = normalize_version(version);

    progress.status("Finding latest tag...");
    let prev_tag = latest_tag(repo)?;

    progress.status("Collecting merged PRs...");
    let prs = merged_prs_since(repo, &prev_tag)?;

    progress.status("Generating release notes...");
    let notes = generate_release_notes(repo, &prs, &version, &prev_tag)?;

    progress.status("Writing RELEASE_NOTES.md...");
    write_release_notes(repo, &notes, &version)?;

    progress.status("Creating release PR...");
    create_release_pr(repo, &version)
}

fn normalize_version(version: &str) -> String {
    version.trim().trim_start_matches('v').to_string()
}

fn latest_tag(repo: &Path) -> OpsResult<String> {
    let tag = run_stdout(repo, "git", &["describe", "--tags", "--abbrev=0"])?;
    if tag.is_empty() {
        return Err(OpsError::Message(
            "no previous tag found; create a tag before running release".to_string(),
        ));
    }
    Ok(tag)
}

fn merged_prs_since(repo: &Path, tag: &str) -> OpsResult<Vec<MergedPr>> {
    let tagged_at = run_stdout(repo, "git", &["log", "-1", "--format=%aI", tag])?;
    if tagged_at.is_empty() {
        return Err(OpsError::Message(format!(
            "could not determine merge date for tag {tag}"
        )));
    }

    let date = tagged_at
        .split('T')
        .next()
        .map(str::to_string)
        .unwrap_or(tagged_at);

    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let base_branch = get_default_branch(&main_repo)?;
    let search = format!("merged:>={date}");

    let output = run_stdout(
        repo,
        "gh",
        &[
            "pr",
            "list",
            "--state",
            "merged",
            "--base",
            &base_branch,
            "--search",
            &search,
            "--json",
            "number,title,body",
            "--limit",
            "200",
        ],
    )?;

    let mut prs: Vec<MergedPr> = serde_json::from_str(&output)
        .map_err(|err| OpsError::Parse(format!("failed to parse merged PR list: {err}")))?;
    prs.reverse();
    Ok(prs)
}

fn generate_release_notes(
    repo: &Path,
    prs: &[MergedPr],
    version: &str,
    prev_tag: &str,
) -> OpsResult<String> {
    let template = get_builtin_ops_prompt("release_notes")
        .ok_or_else(|| OpsError::Message("builtin release_notes prompt not found".to_string()))?
        .replace("{version}", version);

    let pr_summary = if prs.is_empty() {
        "- No merged PRs found in this window.".to_string()
    } else {
        prs.iter()
            .map(format_pr_for_prompt)
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    let prompt = format!(
        "{template}\n\n## Release context\n\n- Target version: v{version}\n- Previous tag: {prev_tag}\n\n## Merged PRs\n\n{pr_summary}\n"
    );

    let config = load_config_or_default(Some(repo));
    let launch_config = LaunchConfig {
        auto: true,
        stream: false,
        skip_permissions: true,
        chrome: config.chrome,
        cwd: Some(repo.to_path_buf()),
        ..Default::default()
    };

    let result = launch_agent(&config.agent_model, &prompt, &launch_config)
        .map_err(|err| OpsError::AgentFailed(err.to_string()))?;
    if result.exit_code != 0 {
        return Err(OpsError::AgentFailed(result.stderr));
    }

    let notes = result.stdout.trim().to_string();
    if notes.is_empty() {
        return Err(OpsError::Parse("empty release notes output".to_string()));
    }
    Ok(notes)
}

fn format_pr_for_prompt(pr: &MergedPr) -> String {
    let body = pr.body.as_deref().unwrap_or("").trim();
    if body.is_empty() {
        return format!("- #{} {}", pr.number, pr.title);
    }

    let max_chars = 400;
    let mut clipped = body.chars().take(max_chars).collect::<String>();
    if body.chars().count() > max_chars {
        clipped.push('…');
    }

    format!("- #{} {}\n  - Body: {}", pr.number, pr.title, clipped)
}

fn write_release_notes(repo: &Path, notes: &str, version: &str) -> OpsResult<()> {
    let path = repo.join("RELEASE_NOTES.md");
    let header = format!("# v{version}");
    let trimmed = notes.trim();
    let first_line = trimmed.lines().next().map(str::trim);

    let content = if first_line == Some(header.as_str()) {
        trimmed.to_string()
    } else {
        format!("{header}\n\n{trimmed}")
    };

    std::fs::write(path, content)?;
    Ok(())
}

fn create_release_pr(repo: &Path, version: &str) -> OpsResult<String> {
    let branch = format!("release/v{version}");

    run_checked(repo, "git", &["checkout", "-B", &branch])?;
    run_checked(repo, "git", &["add", "RELEASE_NOTES.md"])?;

    if !has_staged_changes(repo)? {
        return Err(OpsError::Message(
            "RELEASE_NOTES.md has no changes to commit".to_string(),
        ));
    }

    let commit_message = format!("release: v{version}");
    run_checked(repo, "git", &["commit", "-m", &commit_message])?;
    run_checked(repo, "git", &["push", "-u", "origin", &branch])?;

    let url = run_stdout(
        repo,
        "gh",
        &["pr", "create", "--fill", "--label", "release"],
    )?;

    run_checked(repo, "gh", &["pr", "merge", "--auto", "--squash"])?;

    Ok(url)
}

fn has_staged_changes(repo: &Path) -> OpsResult<bool> {
    let status = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(repo)
        .status()?;
    Ok(!status.success())
}

fn run_checked(repo: &Path, command: &str, args: &[&str]) -> OpsResult<Output> {
    let mut cmd = Command::new(command);
    cmd.args(args).current_dir(repo);
    run_command(&mut cmd).map_err(|err| OpsError::CommandFailed {
        command: err.command_line(),
        stderr: err.stderr,
    })
}

fn run_stdout(repo: &Path, command: &str, args: &[&str]) -> OpsResult<String> {
    let output = run_checked(repo, command, args)?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
