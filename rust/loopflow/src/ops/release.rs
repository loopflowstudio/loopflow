use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::engine::agent::{launch_agent, AgentCapabilities, LaunchConfig, ProcessConfig};
use crate::engine::builtins::get_builtin_ops_prompt;
use crate::engine::command::run_command;
use crate::engine::config::load_config_or_default;
use crate::engine::git::{delete_local_branch, get_default_branch, sync_main, worktree_remove};
use crate::engine::worktrees::{create_with_schema, main_repo_root};
use crate::ops::commit::{commit_workflow, CommitOptions};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::land::{land, LandOptions};
use crate::ops::progress::Progress;
use crate::ops::util::command_exists;

/// A merged PR with enough context for release notes.
#[derive(Debug, Deserialize)]
struct MergedPr {
    number: u32,
    title: String,
    body: Option<String>,
}

/// Generate release notes and write RELEASE_NOTES.md.
///
/// `version_input` is either a bump keyword (`patch`, `minor`, `major`)
/// or an explicit version like `0.9.1` / `v0.9.1`.
///
/// Returns the resolved version string (without `v` prefix).
pub fn generate_release(
    repo: &Path,
    version_input: &str,
    progress: &impl Progress,
) -> OpsResult<String> {
    if !command_exists("gh") {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    progress.status("Finding latest tag...");
    let prev_tag = latest_tag(repo)?;

    let version = resolve_version(&prev_tag, version_input)?;

    progress.status("Collecting merged PRs...");
    let prs = merged_prs_since(repo, &prev_tag)?;

    progress.status("Generating release notes...");
    let notes = generate_release_notes(repo, &prs, &version, &prev_tag)?;

    progress.status("Writing RELEASE_NOTES.md...");
    write_release_notes(repo, &notes, &version)?;

    Ok(version)
}

/// Parse `X.Y.Z` and bump the specified component.
pub fn bump_version(current: &str, bump: &str) -> OpsResult<String> {
    let clean = current.trim().trim_start_matches('v');
    let parts: Vec<&str> = clean.split('.').collect();
    if parts.len() != 3 {
        return Err(OpsError::Parse(format!(
            "invalid version format: {current}"
        )));
    }
    let major: u32 = parts[0]
        .parse()
        .map_err(|_| OpsError::Parse(format!("invalid major version: {}", parts[0])))?;
    let minor: u32 = parts[1]
        .parse()
        .map_err(|_| OpsError::Parse(format!("invalid minor version: {}", parts[1])))?;
    let patch: u32 = parts[2]
        .parse()
        .map_err(|_| OpsError::Parse(format!("invalid patch version: {}", parts[2])))?;

    match bump {
        "major" => Ok(format!("{}.0.0", major + 1)),
        "minor" => Ok(format!("{}.{}.0", major, minor + 1)),
        "patch" => Ok(format!("{}.{}.{}", major, minor, patch + 1)),
        _ => Err(OpsError::Parse(format!("unknown bump type: {bump}"))),
    }
}

/// Resolve version input: bump keywords go through `bump_version`,
/// anything else is treated as an explicit version.
fn resolve_version(prev_tag: &str, input: &str) -> OpsResult<String> {
    match input.trim() {
        "patch" | "minor" | "major" => bump_version(prev_tag, input.trim()),
        other => Ok(normalize_version(other)),
    }
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
    let launch = LaunchConfig {
        task_prompt: prompt,
        model: Some(config.agent_model.clone()),
        skip_permissions: true,
        cwd: Some(repo.to_path_buf()),
        ..Default::default()
    };
    let process = ProcessConfig {
        auto: true,
        stream: false,
        ..Default::default()
    };
    let capabilities = AgentCapabilities {
        chrome: config.chrome,
    };

    let result = launch_agent(&launch, &process, &capabilities)
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

fn run_stdout(repo: &Path, command: &str, args: &[&str]) -> OpsResult<String> {
    let mut cmd = Command::new(command);
    cmd.args(args).current_dir(repo);
    let output = run_command(&mut cmd).map_err(|err| OpsError::CommandFailed {
        command: err.command_line(),
        stderr: err.stderr,
    })?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// =========================================================================
// lf ops release — full publish workflow
// =========================================================================

#[derive(Debug, Clone)]
pub struct PublishOptions {
    pub version_input: String,
    pub dry_run: bool,
}

#[derive(Debug)]
pub struct PublishResult {
    pub version: String,
}

/// Full release workflow: sync main, create worktree, generate release notes,
/// commit, land, tag, push tag.
///
/// Runs from any branch — always works off a fresh worktree from origin/main.
pub fn publish_release(
    repo: &Path,
    options: &PublishOptions,
    progress: &impl Progress,
) -> OpsResult<PublishResult> {
    if !command_exists("gh") {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());

    // Resolve version before creating worktree so dry-run can report it.
    let prev_tag = latest_tag(&main_repo)?;
    let version = resolve_version(&prev_tag, &options.version_input)?;
    let wt_name = format!("release-v{version}");

    if options.dry_run {
        progress.status(&format!("Would publish v{version}"));
        progress.status(&format!("  Create worktree: {wt_name}"));
        progress.status("  Generate release notes");
        progress.status("  Commit, PR, and land");
        progress.status(&format!("  Tag v{version} and push"));
        return Ok(PublishResult { version });
    }

    // 1. Sync main
    progress.status("Syncing main...");
    let main_branch = get_default_branch(&main_repo)?;
    sync_main(&main_repo, &main_branch)?;

    // 2. Create worktree off main
    progress.status(&format!("Creating worktree {wt_name}..."));
    let wt = create_with_schema(&main_repo, &wt_name, Some(&main_branch), None)?;
    let wt_path = wt.path;
    let wt_branch = wt.branch;

    let publish_result = publish_in_worktree(&wt_path, &version, progress);

    // Clean up worktree regardless of success/failure
    cleanup_worktree(&main_repo, &wt_path, &wt_branch, progress);

    publish_result?;

    // 5. Sync main to get the merged commit, then tag
    progress.status("Syncing main after merge...");
    sync_main(&main_repo, &main_branch)?;

    progress.status(&format!("Tagging v{version}..."));
    tag_and_push(&main_repo, &version)?;

    progress.status(&format!("v{version} published. CI will build and release."));
    Ok(PublishResult { version })
}

/// Run the release workflow inside the worktree. Separated so cleanup
/// can happen regardless of success or failure.
fn publish_in_worktree(wt_path: &Path, version: &str, progress: &impl Progress) -> OpsResult<()> {
    // 3. Generate release notes
    progress.status(&format!("Generating release notes for v{version}..."));
    generate_release(wt_path, version, progress)?;

    // 4. Commit and land
    progress.status("Committing release notes...");
    commit_workflow(
        wt_path,
        &CommitOptions {
            add: true,
            lint: false,
            push: false,
            create_draft_pr: false,
            task: "release".to_string(),
            flow_parents: Vec::new(),
            message: Some(format!("release: v{version}")),
        },
        progress,
    )?;

    progress.status("Creating PR and landing...");
    land(
        wt_path,
        &LandOptions {
            strict: false,
            local: false,
            create_pr: true,
            worktree: None,
            lint: false,
        },
        progress,
    )?;

    Ok(())
}

fn cleanup_worktree(main_repo: &Path, wt_path: &Path, branch: &str, progress: &impl Progress) {
    if let Err(e) = worktree_remove(main_repo, wt_path) {
        progress.error(&format!("Warning: could not remove worktree: {e}"));
    }
    let _ = delete_local_branch(main_repo, branch);
}

fn tag_and_push(repo: &Path, version: &str) -> OpsResult<()> {
    let tag = format!("v{version}");
    run_stdout(repo, "git", &["tag", &tag])?;
    run_stdout(repo, "git", &["push", "origin", &tag])?;
    Ok(())
}
