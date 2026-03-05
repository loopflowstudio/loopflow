use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, Instant};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::engine::command::run_command;
use crate::engine::config::{load_config_or_default, Config, ReleaseTargetConfig};
use crate::engine::git::{delete_local_branch, get_default_branch, worktree_remove};
use crate::engine::worktrees::{create_with_schema, main_repo_root};
use crate::ops::commit::{commit_workflow, CommitOptions};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::land::{land, LandOptions};
use crate::ops::progress::Progress;
use crate::ops::util::command_exists;

/// A merged PR with enough context for release notes.
#[derive(Debug, Clone, Serialize)]
pub struct MergedPr {
    pub number: u32,
    pub title: String,
    pub body: Option<String>,
    pub files: Vec<String>,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
}

#[derive(Debug, Deserialize)]
struct GhMergedPr {
    number: u32,
    title: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    files: Vec<GhPrFile>,
    #[serde(default)]
    additions: u64,
    #[serde(default)]
    deletions: u64,
    #[serde(default, rename = "changedFiles")]
    changed_files: u64,
}

#[derive(Debug, Deserialize)]
struct GhPrFile {
    #[serde(default, alias = "path")]
    filename: String,
}

#[derive(Debug, Deserialize)]
struct GhApiPrFile {
    filename: String,
}

#[derive(Debug, Deserialize, Clone)]
struct GhRunListEntry {
    #[serde(default, rename = "headBranch")]
    head_branch: Option<String>,
    #[serde(default, rename = "displayTitle")]
    display_title: Option<String>,
    status: String,
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhPrSummary {
    number: u64,
}

#[derive(Debug, Deserialize)]
struct GhPrMergeCommit {
    oid: String,
}

#[derive(Debug, Deserialize)]
struct GhPrView {
    state: String,
    #[serde(default, rename = "mergeCommit")]
    merge_commit: Option<GhPrMergeCommit>,
    #[serde(default)]
    url: Option<String>,
}

impl From<GhMergedPr> for MergedPr {
    fn from(value: GhMergedPr) -> Self {
        Self {
            number: value.number,
            title: value.title,
            body: value.body,
            files: value
                .files
                .into_iter()
                .map(|file| file.filename)
                .filter(|file| !file.is_empty())
                .collect(),
            additions: value.additions,
            deletions: value.deletions,
            changed_files: value.changed_files,
        }
    }
}

#[derive(Debug, Clone)]
struct ReleaseTarget {
    name: String,
    area: Vec<String>,
    tag_prefix: String,
    manifests: Vec<PathBuf>,
    workflow: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReleaseStatusResult {
    pub target: String,
    pub latest_tag: Option<String>,
    pub workflow_status: Option<String>,
    pub workflow_conclusion: Option<String>,
    pub workflow_url: Option<String>,
    pub release_exists: bool,
}

#[derive(Debug, Clone)]
pub struct ReleaseRunResult {
    pub target: String,
    pub version: String,
    pub tag: String,
    pub workflow_url: Option<String>,
    pub release_exists: bool,
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

    let target = default_release_target(repo);
    generate_release_with_target(repo, version_input, &target, progress)
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

pub fn release_status(repo: &Path, target_name: Option<&str>) -> OpsResult<ReleaseStatusResult> {
    if !command_exists("gh") {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    let (main_repo, target) = resolve_repo_and_target(repo, target_name)?;

    let latest_tag = latest_tag_optional(&main_repo, &target)?;
    let (workflow, release_exists) = match latest_tag.as_deref() {
        Some(tag) => (
            find_workflow_run(&main_repo, tag, &target)?,
            github_release_exists(&main_repo, tag)?,
        ),
        None => (None, false),
    };

    Ok(ReleaseStatusResult {
        target: target.name,
        latest_tag,
        workflow_status: workflow.as_ref().map(|run| run.status.clone()),
        workflow_conclusion: workflow.as_ref().and_then(|run| run.conclusion.clone()),
        workflow_url: workflow.and_then(|run| run.url),
        release_exists,
    })
}

/// Check if any PRs have merged since the last tag.
///
/// Returns the list of merged PRs if changes exist, empty vec if not.
pub fn release_check(repo: &Path, target_name: Option<&str>) -> OpsResult<Vec<MergedPr>> {
    if !command_exists("gh") {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    let (main_repo, target) = resolve_repo_and_target(repo, target_name)?;

    let prev_tag = match latest_tag_optional(&main_repo, &target)? {
        Some(tag) => tag,
        None => return Ok(Vec::new()),
    };

    merged_prs_since(&main_repo, &prev_tag, &target)
}

/// Generate release notes for the given version.
///
/// Writes RELEASE_NOTES.md. Returns the generated notes content.
pub fn release_notes(
    repo: &Path,
    version: &str,
    prev_tag: Option<&str>,
    target_name: Option<&str>,
    progress: &impl Progress,
) -> OpsResult<String> {
    if !command_exists("gh") {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    let (main_repo, target) = resolve_repo_and_target(repo, target_name)?;

    let resolved_prev_tag = match prev_tag {
        Some(tag) => tag.to_string(),
        None => latest_tag(&main_repo, &target)?,
    };

    let version = normalize_version(version);

    progress.status("Collecting merged PRs...");
    let prs = merged_prs_since(&main_repo, &resolved_prev_tag, &target)?;

    progress.status("Generating release notes...");
    let notes = generate_release_notes(&prs, &version, &resolved_prev_tag, &target)?;

    progress.status("Writing RELEASE_NOTES.md...");
    write_release_notes(&main_repo, &notes, &version)?;

    Ok(notes)
}

/// Bump version in all manifest files for the target.
pub fn release_bump(
    repo: &Path,
    version: &str,
    target_name: Option<&str>,
    progress: &impl Progress,
) -> OpsResult<()> {
    let (main_repo, target) = resolve_repo_and_target(repo, target_name)?;

    let version = normalize_version(version);
    bump_manifest_versions(&main_repo, &target, &version, progress)
}

/// Create a git tag and push it to the remote.
///
/// Returns the full tag string (e.g. `v0.9.6`).
pub fn release_tag(repo: &Path, version: &str, target_name: Option<&str>) -> OpsResult<String> {
    let (main_repo, target) = resolve_repo_and_target(repo, target_name)?;

    let version = normalize_version(version);
    tag_and_push(&main_repo, &version, &target)
}

/// Run the full release workflow in one shot.
///
/// Flow:
/// 1) check merged PRs since previous tag
/// 2) bump manifests + generate notes in a temporary worktree
/// 3) commit, open PR, and enqueue auto-merge
/// 4) wait for merge queue completion
/// 5) tag merged commit and push
/// 6) wait for release workflow/release publication
pub fn release_run(
    repo: &Path,
    version_input: &str,
    target_name: Option<&str>,
    progress: &impl Progress,
) -> OpsResult<ReleaseRunResult> {
    if !command_exists("gh") {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    let (main_repo, target) = resolve_repo_and_target(repo, target_name)?;
    let prev_tag = latest_tag(&main_repo, &target)?;
    let merged_prs = merged_prs_since(&main_repo, &prev_tag, &target)?;
    if merged_prs.is_empty() {
        return Err(OpsError::Message(
            "no PRs merged since last tag; nothing to release".to_string(),
        ));
    }

    let version = resolve_version(&prev_tag, version_input, &target)?;
    let main_branch = get_default_branch(&main_repo)?;
    let wt_name = if target.name == "default" {
        format!("release.default.v{version}")
    } else {
        format!("release.{}.v{version}", target.name)
    };

    progress.status(&format!("Creating release worktree {wt_name}..."));
    let wt = create_with_schema(&main_repo, &wt_name, Some(&main_branch), None)?;
    let wt_path = wt.path;
    let wt_branch = wt.branch;

    let prepared = prepare_release_in_worktree(
        &wt_path,
        &version,
        &prev_tag,
        &merged_prs,
        &target,
        progress,
    );
    cleanup_release_worktree(&main_repo, &wt_path, &wt_branch, progress);
    let prepared = prepared?;

    progress.status("Waiting for release PR to merge...");
    let merged_commit = wait_for_pr_merge(&main_repo, prepared.pr_number, progress)?;

    progress.status(&format!("Tagging {}...", target_tag(&target, &version)));
    let tag = tag_and_push_ref(&main_repo, &version, &target, Some(&merged_commit))?;

    progress.status(&format!("Waiting for release pipeline for {tag}..."));
    let (workflow_url, release_exists) =
        wait_for_release_publication(&main_repo, &tag, &target, progress)?;

    Ok(ReleaseRunResult {
        target: target.name,
        version,
        tag,
        workflow_url,
        release_exists,
    })
}

#[derive(Debug)]
struct PreparedRelease {
    pr_number: u64,
}

fn prepare_release_in_worktree(
    wt_path: &Path,
    version: &str,
    prev_tag: &str,
    merged_prs: &[MergedPr],
    target: &ReleaseTarget,
    progress: &impl Progress,
) -> OpsResult<PreparedRelease> {
    progress.status(&format!(
        "Bumping manifests for {}...",
        target_tag(target, version)
    ));
    bump_manifest_versions(wt_path, target, version, progress)?;

    progress.status(&format!(
        "Generating release notes for {}...",
        target_tag(target, version)
    ));
    run_release_notes_step(wt_path, version, prev_tag, merged_prs, target)?;

    progress.status("Committing release changes...");
    let _ = commit_workflow(
        wt_path,
        &CommitOptions {
            add: true,
            push: true,
            create_draft_pr: true,
            message: Some(release_commit_message(target, version)),
            ..CommitOptions::for_task("release")
        },
        progress,
    )?;

    let pr = current_pr_summary(wt_path)?;

    progress.status("Enqueuing release PR for merge...");
    land(
        wt_path,
        &LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
        },
        progress,
    )?;

    Ok(PreparedRelease {
        pr_number: pr.number,
    })
}

#[derive(Debug, Serialize)]
struct ReleaseNotesContext {
    version: String,
    prev_tag: String,
    target: String,
    tag_prefix: String,
    area_scope: Vec<String>,
    merged_prs: Vec<MergedPr>,
    previous_release_notes: Option<String>,
}

fn run_release_notes_step(
    repo: &Path,
    version: &str,
    prev_tag: &str,
    merged_prs: &[MergedPr],
    target: &ReleaseTarget,
) -> OpsResult<()> {
    let previous_notes = fs::read_to_string(repo.join("RELEASE_NOTES.md")).ok();
    let context = ReleaseNotesContext {
        version: version.to_string(),
        prev_tag: prev_tag.to_string(),
        target: target.name.clone(),
        tag_prefix: target.tag_prefix.clone(),
        area_scope: target.area.clone(),
        merged_prs: merged_prs.to_vec(),
        previous_release_notes: previous_notes,
    };

    let mut context_file = tempfile::NamedTempFile::new_in(repo)?;
    serde_json::to_writer_pretty(&mut context_file, &context)
        .map_err(|err| OpsError::Parse(format!("failed to encode release-notes context: {err}")))?;
    let context_path = context_file.path().to_string_lossy().to_string();

    let mut cmd = Command::new("lf");
    cmd.arg("--batch")
        .arg("release-notes")
        .current_dir(repo)
        .env("LF_RELEASE_NOTES_CONTEXT", &context_path);
    run_command(&mut cmd).map_err(|err| OpsError::CommandFailed {
        command: err.command_line(),
        stderr: err.stderr,
    })?;

    if !repo.join("RELEASE_NOTES.md").exists() {
        return Err(OpsError::Message(
            "release-notes step did not write RELEASE_NOTES.md".to_string(),
        ));
    }

    Ok(())
}

fn cleanup_release_worktree(
    main_repo: &Path,
    wt_path: &Path,
    branch: &str,
    progress: &impl Progress,
) {
    if let Err(err) = worktree_remove(main_repo, wt_path) {
        progress.error(&format!(
            "Warning: could not remove release worktree {}: {err}",
            wt_path.display()
        ));
    }
    let _ = delete_local_branch(main_repo, branch);
}

fn release_commit_message(target: &ReleaseTarget, version: &str) -> String {
    if target.name == "default" {
        format!("release: v{version}")
    } else {
        format!("release: {} v{version}", target.name)
    }
}

fn current_pr_summary(repo: &Path) -> OpsResult<GhPrSummary> {
    let output = run_stdout(repo, "gh", &["pr", "view", "--json", "number"])?;
    serde_json::from_str(&output)
        .map_err(|err| OpsError::Parse(format!("failed to parse PR summary: {err}")))
}

fn wait_for_pr_merge(repo: &Path, pr_number: u64, progress: &impl Progress) -> OpsResult<String> {
    let started = Instant::now();
    let timeout = Duration::from_secs(60 * 60);
    let poll = Duration::from_secs(10);
    let pr_number_arg = pr_number.to_string();
    let mut attempt: u64 = 0;

    loop {
        let output = run_stdout(
            repo,
            "gh",
            &[
                "pr",
                "view",
                &pr_number_arg,
                "--json",
                "state,mergeCommit,url",
            ],
        )?;
        let view: GhPrView = serde_json::from_str(&output)
            .map_err(|err| OpsError::Parse(format!("failed to parse PR state: {err}")))?;

        match view.state.as_str() {
            "MERGED" => {
                let commit = view.merge_commit.ok_or_else(|| {
                    OpsError::Message(format!(
                        "PR #{pr_number} is merged but merge commit is unavailable"
                    ))
                })?;
                return Ok(commit.oid);
            }
            "CLOSED" => {
                let url = view.url.unwrap_or_else(|| format!("PR #{pr_number}"));
                return Err(OpsError::Message(format!(
                    "{url} was closed without merging"
                )));
            }
            _ => {}
        }

        if started.elapsed() >= timeout {
            return Err(OpsError::Message(format!(
                "timed out waiting for PR #{pr_number} to merge"
            )));
        }

        if attempt.is_multiple_of(6) {
            progress.status(&format!("PR #{pr_number} still in merge queue..."));
        }
        attempt += 1;
        thread::sleep(poll);
    }
}

fn wait_for_release_publication(
    repo: &Path,
    tag: &str,
    target: &ReleaseTarget,
    progress: &impl Progress,
) -> OpsResult<(Option<String>, bool)> {
    let started = Instant::now();
    let timeout = Duration::from_secs(60 * 60);
    let poll = Duration::from_secs(10);
    let no_workflow_grace = Duration::from_secs(90);
    let mut attempt: u64 = 0;
    let mut saw_workflow = false;

    loop {
        let workflow = find_workflow_run(repo, tag, target)?;
        let release_exists = github_release_exists(repo, tag)?;

        if let Some(run) = workflow {
            saw_workflow = true;
            if run.status == "completed" {
                let conclusion = run
                    .conclusion
                    .unwrap_or_else(|| "unknown".to_string())
                    .to_lowercase();
                if conclusion == "success" {
                    return Ok((run.url, release_exists));
                }

                let url = run
                    .url
                    .unwrap_or_else(|| "(workflow URL unavailable)".to_string());
                return Err(OpsError::Message(format!(
                    "release workflow failed for {tag}: {conclusion} ({url})"
                )));
            }
        } else if release_exists {
            return Ok((None, true));
        } else if !saw_workflow
            && target.workflow.is_none()
            && started.elapsed() >= no_workflow_grace
        {
            progress.status(&format!(
                "No release workflow detected for {tag}; tag push complete"
            ));
            return Ok((None, false));
        }

        if started.elapsed() >= timeout {
            return Err(OpsError::Message(format!(
                "timed out waiting for release workflow for {tag}"
            )));
        }

        if attempt.is_multiple_of(6) {
            progress.status(&format!("Release workflow for {tag} still running..."));
        }
        attempt += 1;
        thread::sleep(poll);
    }
}

fn generate_release_with_target(
    repo: &Path,
    version_input: &str,
    target: &ReleaseTarget,
    progress: &impl Progress,
) -> OpsResult<String> {
    progress.status("Finding latest tag...");
    let prev_tag = latest_tag(repo, target)?;

    let version = resolve_version(&prev_tag, version_input, target)?;

    progress.status("Collecting merged PRs...");
    let prs = merged_prs_since(repo, &prev_tag, target)?;

    progress.status("Generating release notes...");
    let notes = generate_release_notes(&prs, &version, &prev_tag, target)?;

    progress.status("Writing RELEASE_NOTES.md...");
    write_release_notes(repo, &notes, &version)?;

    Ok(version)
}

fn resolve_target(
    config: &Config,
    target_name: Option<&str>,
    repo: &Path,
) -> OpsResult<ReleaseTarget> {
    if config.release.targets.is_empty() {
        return Ok(default_release_target(repo));
    }

    let selected_name = if let Some(name) = target_name {
        name.to_string()
    } else if config.release.targets.len() == 1 {
        config
            .release
            .targets
            .keys()
            .next()
            .cloned()
            .ok_or_else(|| OpsError::Message("missing release target".to_string()))?
    } else {
        return Err(OpsError::Message(
            "multiple release targets; use --target <name>".to_string(),
        ));
    };

    let target_config = config
        .release
        .targets
        .get(&selected_name)
        .ok_or_else(|| OpsError::Message(format!("unknown release target: {selected_name}")))?;

    Ok(build_release_target(&selected_name, target_config, repo))
}

/// Resolve the main repo root and release target from a repo path and optional target name.
///
/// Common preamble for all decomposed release functions.
fn resolve_repo_and_target(
    repo: &Path,
    target_name: Option<&str>,
) -> OpsResult<(PathBuf, ReleaseTarget)> {
    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let config = load_config_or_default(Some(&main_repo));
    let target = resolve_target(&config, target_name, &main_repo)?;
    Ok((main_repo, target))
}

fn build_release_target(name: &str, config: &ReleaseTargetConfig, repo: &Path) -> ReleaseTarget {
    let manifests = if config.manifests.is_empty() {
        detect_manifests(repo, &config.area)
    } else {
        config
            .manifests
            .iter()
            .map(|manifest| PathBuf::from(manifest.trim_start_matches("./")))
            .collect()
    };

    ReleaseTarget {
        name: name.to_string(),
        area: config.area.clone(),
        tag_prefix: config.tag_prefix.clone(),
        manifests,
        workflow: config.workflow.clone(),
    }
}

fn default_release_target(repo: &Path) -> ReleaseTarget {
    ReleaseTarget {
        name: "default".to_string(),
        area: Vec::new(),
        tag_prefix: String::new(),
        manifests: detect_manifests(repo, &[]),
        workflow: None,
    }
}

fn detect_manifests(repo: &Path, area: &[String]) -> Vec<PathBuf> {
    let mut manifests = Vec::new();
    let mut seen = HashSet::new();

    let mut roots = area_search_roots(area);
    if roots.is_empty() {
        roots.push(PathBuf::new());
    }

    for rel_root in roots {
        let root = if rel_root.as_os_str().is_empty() {
            repo.to_path_buf()
        } else {
            repo.join(&rel_root)
        };

        if !root.exists() {
            continue;
        }

        if root.is_file() {
            maybe_add_manifest(repo, &root, &mut manifests, &mut seen);
            continue;
        }

        maybe_add_manifest(repo, &root.join("Cargo.toml"), &mut manifests, &mut seen);
        maybe_add_manifest(repo, &root.join("package.json"), &mut manifests, &mut seen);

        let pyproject = root.join("pyproject.toml");
        if pyproject.exists() && should_auto_bump_pyproject(&pyproject) {
            maybe_add_manifest(repo, &pyproject, &mut manifests, &mut seen);
        }
    }

    manifests
}

fn area_search_roots(area: &[String]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    for pattern in area {
        let normalized = pattern
            .trim()
            .trim_start_matches("./")
            .trim_end_matches('/');
        if normalized.is_empty() {
            continue;
        }

        let root = prefix_before_glob(normalized);
        if root.as_os_str().is_empty() {
            continue;
        }

        if seen.insert(root.clone()) {
            roots.push(root);
        }
    }

    roots
}

fn prefix_before_glob(pattern: &str) -> PathBuf {
    let mut root = PathBuf::new();
    for component in Path::new(pattern).components() {
        let part = component.as_os_str().to_string_lossy();
        if contains_glob_chars(&part) {
            break;
        }
        root.push(component.as_os_str());
    }
    root
}

fn maybe_add_manifest(
    repo: &Path,
    candidate: &Path,
    manifests: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
) {
    if !candidate.exists() || !is_known_manifest(candidate) {
        return;
    }

    let rel = candidate
        .strip_prefix(repo)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| candidate.to_path_buf());

    if seen.insert(rel.clone()) {
        manifests.push(rel);
    }
}

fn is_known_manifest(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("Cargo.toml") | Some("package.json") | Some("pyproject.toml")
    )
}

fn should_auto_bump_pyproject(path: &Path) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };

    if content.contains("version =") {
        return true;
    }

    let has_dynamic_version = content.contains("dynamic") && content.contains("\"version\"");
    if has_dynamic_version && content.contains("hatch-vcs") {
        return false;
    }

    has_dynamic_version
}

fn resolve_version(prev_tag: &str, input: &str, target: &ReleaseTarget) -> OpsResult<String> {
    match input.trim() {
        "patch" | "minor" | "major" => {
            let prev_version = version_from_tag(prev_tag, target)?;
            bump_version(&prev_version, input.trim())
        }
        other => Ok(normalize_version(other)),
    }
}

fn normalize_version(version: &str) -> String {
    version.trim().trim_start_matches('v').to_string()
}

fn version_from_tag(tag: &str, target: &ReleaseTarget) -> OpsResult<String> {
    let unscoped = if target.tag_prefix.is_empty() {
        tag
    } else {
        tag.strip_prefix(&target.tag_prefix).ok_or_else(|| {
            OpsError::Parse(format!(
                "tag '{tag}' does not match target prefix '{}'",
                target.tag_prefix
            ))
        })?
    };

    Ok(normalize_version(unscoped))
}

fn target_tag(target: &ReleaseTarget, version: &str) -> String {
    format!("{}v{version}", target.tag_prefix)
}

fn display_tag_prefix(target: &ReleaseTarget) -> &str {
    if target.tag_prefix.is_empty() {
        "(none)"
    } else {
        target.tag_prefix.as_str()
    }
}

fn display_area_scope(target: &ReleaseTarget) -> String {
    if target.area.is_empty() {
        "(entire repository)".to_string()
    } else {
        target.area.join(", ")
    }
}

fn latest_tag(repo: &Path, target: &ReleaseTarget) -> OpsResult<String> {
    latest_tag_optional(repo, target)?.ok_or_else(|| {
        OpsError::Message(format!(
            "no previous tag found matching {}; create a tag before running release",
            tag_glob(target)
        ))
    })
}

fn latest_tag_optional(repo: &Path, target: &ReleaseTarget) -> OpsResult<Option<String>> {
    let pattern = tag_glob(target);
    let tags = run_stdout(
        repo,
        "git",
        &["tag", "-l", &pattern, "--sort=-version:refname"],
    )?;
    let latest = tags
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string);
    Ok(latest)
}

fn tag_glob(target: &ReleaseTarget) -> String {
    format!("{}v*", target.tag_prefix)
}

fn merged_prs_since(repo: &Path, tag: &str, target: &ReleaseTarget) -> OpsResult<Vec<MergedPr>> {
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

    let mut prs = if target.area.is_empty() {
        list_merged_prs(repo, &base_branch, &search, false)?
    } else {
        match list_merged_prs(repo, &base_branch, &search, true) {
            Ok(prs) => prs,
            Err(err) if should_fallback_for_pr_files(&err) => {
                let mut fallback_prs = list_merged_prs(repo, &base_branch, &search, false)?;
                let repo_slug = github_repo_slug(repo)?;
                for pr in &mut fallback_prs {
                    pr.files = fetch_pr_files(repo, &repo_slug, pr.number)?;
                }
                fallback_prs
            }
            Err(err) => return Err(err),
        }
    };

    prs.reverse();

    if !target.area.is_empty() {
        prs.retain(|pr| {
            pr.files
                .iter()
                .any(|path| area_matches(path, target.area.as_slice()))
        });
    }

    Ok(prs)
}

fn list_merged_prs(
    repo: &Path,
    base_branch: &str,
    search: &str,
    include_files: bool,
) -> OpsResult<Vec<MergedPr>> {
    let fields = if include_files {
        "number,title,body,files,additions,deletions,changedFiles"
    } else {
        "number,title,body,additions,deletions,changedFiles"
    };

    let output = run_stdout(
        repo,
        "gh",
        &[
            "pr",
            "list",
            "--state",
            "merged",
            "--base",
            base_branch,
            "--search",
            search,
            "--json",
            fields,
            "--limit",
            "200",
        ],
    )?;

    let prs: Vec<GhMergedPr> = serde_json::from_str(&output)
        .map_err(|err| OpsError::Parse(format!("failed to parse merged PR list: {err}")))?;

    Ok(prs.into_iter().map(Into::into).collect())
}

fn should_fallback_for_pr_files(err: &OpsError) -> bool {
    match err {
        OpsError::CommandFailed { stderr, .. } => {
            stderr.contains("Unknown JSON field") && stderr.contains("files")
        }
        OpsError::Parse(message) => message.contains("files"),
        _ => false,
    }
}

fn github_repo_slug(repo: &Path) -> OpsResult<String> {
    run_stdout(
        repo,
        "gh",
        &[
            "repo",
            "view",
            "--json",
            "nameWithOwner",
            "--jq",
            ".nameWithOwner",
        ],
    )
}

fn fetch_pr_files(repo: &Path, repo_slug: &str, pr_number: u32) -> OpsResult<Vec<String>> {
    let endpoint = format!("repos/{repo_slug}/pulls/{pr_number}/files?per_page=100");
    let output = run_stdout(repo, "gh", &["api", &endpoint])?;
    let files: Vec<GhApiPrFile> = serde_json::from_str(&output).map_err(|err| {
        OpsError::Parse(format!("failed to parse PR files for #{pr_number}: {err}"))
    })?;

    Ok(files.into_iter().map(|file| file.filename).collect())
}

fn area_matches(path: &str, patterns: &[String]) -> bool {
    let normalized_path = path.trim_start_matches("./");
    patterns
        .iter()
        .any(|pattern| path_matches_pattern(normalized_path, pattern))
}

fn path_matches_pattern(path: &str, pattern: &str) -> bool {
    let normalized = pattern
        .trim()
        .trim_start_matches("./")
        .trim_end_matches('/');
    if normalized.is_empty() {
        return false;
    }

    if !contains_glob_chars(normalized) {
        return path == normalized || path.starts_with(&format!("{normalized}/"));
    }

    let regex = match Regex::new(&glob_to_regex(normalized)) {
        Ok(regex) => regex,
        Err(_) => return false,
    };

    regex.is_match(path)
}

fn contains_glob_chars(value: &str) -> bool {
    value.contains('*') || value.contains('?') || value.contains('[')
}

fn glob_to_regex(pattern: &str) -> String {
    let mut regex = String::from("^");
    let mut chars = pattern.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '*' => {
                if chars.peek() == Some(&'*') {
                    let _ = chars.next();
                    regex.push_str(".*");
                } else {
                    regex.push_str("[^/]*");
                }
            }
            '?' => regex.push_str("[^/]"),
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' | '\\' => {
                regex.push('\\');
                regex.push(ch);
            }
            _ => regex.push(ch),
        }
    }

    regex.push('$');
    regex
}

fn generate_release_notes(
    prs: &[MergedPr],
    version: &str,
    prev_tag: &str,
    target: &ReleaseTarget,
) -> OpsResult<String> {
    let mut lines = vec![
        format!("## Changes since {prev_tag}"),
        String::new(),
        format!("- Target: {}", target.name),
        format!("- Tag prefix: {}", display_tag_prefix(target)),
        format!("- Area scope: {}", display_area_scope(target)),
        String::new(),
        "## Merged PRs".to_string(),
        String::new(),
    ];

    if prs.is_empty() {
        lines.push("- No merged PRs found in this window.".to_string());
    } else {
        for pr in prs {
            lines.push(format!(
                "- #{} {} (+{} -{}, {} files)",
                pr.number, pr.title, pr.additions, pr.deletions, pr.changed_files
            ));
            if let Some(body) = pr
                .body
                .as_deref()
                .map(str::trim)
                .filter(|body| !body.is_empty())
            {
                let single_line = body.replace('\n', " ");
                lines.push(format!("  - {}", single_line));
            }
        }
    }

    lines.push(String::new());
    lines.push(format!("_Generated mechanically for v{version}._"));
    Ok(lines.join("\n"))
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

    fs::write(path, content)?;
    Ok(())
}

fn bump_manifest_versions(
    repo: &Path,
    target: &ReleaseTarget,
    version: &str,
    progress: &impl Progress,
) -> OpsResult<()> {
    if target.manifests.is_empty() {
        progress.status("No manifest version files detected; skipping version bump.");
        return Ok(());
    }

    let mut bumped_cargo = false;
    let mut bumped_pyproject = false;

    for manifest in &target.manifests {
        let manifest_path = repo.join(manifest);
        if !manifest_path.exists() {
            return Err(OpsError::Message(format!(
                "release manifest not found: {}",
                manifest_path.display()
            )));
        }

        let original = fs::read_to_string(&manifest_path)?;
        let name = manifest_path.file_name().and_then(|name| name.to_str());
        let updated = match name {
            Some("Cargo.toml") => bump_cargo_toml(&original, version)?,
            Some("package.json") => bump_package_json(&original, version)?,
            Some("pyproject.toml") => bump_pyproject_toml(&original, version)?,
            _ => continue,
        };

        if updated != original {
            fs::write(&manifest_path, updated)?;
            progress.status(&format!("Bumped version in {}", manifest_path.display()));
            match name {
                Some("Cargo.toml") => bumped_cargo = true,
                Some("pyproject.toml") => bumped_pyproject = true,
                _ => {}
            }
        }
    }

    // Update lock files so they stay in sync with manifest versions.
    if bumped_cargo && repo.join("Cargo.lock").exists() {
        let _ = Command::new("cargo")
            .args(["update", "--workspace"])
            .current_dir(repo)
            .output();
    }
    if bumped_pyproject && repo.join("uv.lock").exists() {
        let _ = Command::new("uv").arg("lock").current_dir(repo).output();
    }

    Ok(())
}

fn bump_cargo_toml(content: &str, version: &str) -> OpsResult<String> {
    if let Some(updated) = replace_toml_version_in_section(content, "[workspace.package]", version)?
    {
        return Ok(updated);
    }
    if let Some(updated) = replace_toml_version_in_section(content, "[package]", version)? {
        return Ok(updated);
    }

    Err(OpsError::Parse(
        "Cargo.toml missing version in [workspace.package] or [package]".to_string(),
    ))
}

fn bump_package_json(content: &str, version: &str) -> OpsResult<String> {
    let version_re = Regex::new(r#"\"version\"\s*:\s*\"[^\"]*\""#)
        .map_err(|err| OpsError::Parse(format!("invalid package.json regex: {err}")))?;
    if !version_re.is_match(content) {
        return Err(OpsError::Parse(
            "package.json missing version field".to_string(),
        ));
    }

    Ok(version_re
        .replacen(content, 1, format!("\"version\": \"{version}\"").as_str())
        .to_string())
}

fn bump_pyproject_toml(content: &str, version: &str) -> OpsResult<String> {
    if let Some(updated) = replace_toml_version_in_section(content, "[project]", version)? {
        return Ok(updated);
    }

    set_pyproject_dynamic_version(content, version)
}

fn replace_toml_version_in_section(
    content: &str,
    section_name: &str,
    version: &str,
) -> OpsResult<Option<String>> {
    let version_re = Regex::new(r#"^(\s*version\s*=\s*)[\"'][^\"']*[\"'](.*)$"#)
        .map_err(|err| OpsError::Parse(format!("invalid version regex: {err}")))?;

    let mut lines = Vec::new();
    let mut in_section = false;
    let mut replaced = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed == section_name;
            lines.push(line.to_string());
            continue;
        }

        if in_section && !replaced {
            if let Some(captures) = version_re.captures(line) {
                let prefix = captures.get(1).map(|m| m.as_str()).unwrap_or("version = ");
                let suffix = captures.get(2).map(|m| m.as_str()).unwrap_or("");
                lines.push(format!("{prefix}\"{version}\"{suffix}"));
                replaced = true;
                continue;
            }
        }

        lines.push(line.to_string());
    }

    if replaced {
        Ok(Some(join_lines_like_original(content, lines)))
    } else {
        Ok(None)
    }
}

fn set_pyproject_dynamic_version(content: &str, version: &str) -> OpsResult<String> {
    let dynamic_re = Regex::new(r#"^(\s*)dynamic\s*=\s*\[(?P<items>[^\]]*)\]\s*$"#)
        .map_err(|err| OpsError::Parse(format!("invalid dynamic regex: {err}")))?;

    let mut lines = Vec::new();
    let mut in_project = false;
    let mut saw_project = false;
    let mut inserted = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_project && !inserted {
                lines.push(format!("version = \"{version}\""));
                inserted = true;
            }

            in_project = trimmed == "[project]";
            if in_project {
                saw_project = true;
            }
            lines.push(line.to_string());
            continue;
        }

        if in_project {
            if let Some(captures) = dynamic_re.captures(line) {
                let indent = captures.get(1).map(|m| m.as_str()).unwrap_or("");
                let raw_items = captures.name("items").map(|m| m.as_str()).unwrap_or("");
                let mut items = parse_toml_string_array(raw_items);
                let before = items.len();
                items.retain(|item| item != "version");
                if items.len() != before {
                    if !inserted {
                        lines.push(format!("{indent}version = \"{version}\""));
                        inserted = true;
                    }
                    if !items.is_empty() {
                        lines.push(format!(
                            "{indent}dynamic = [{}]",
                            items
                                .iter()
                                .map(|item| format!("\"{item}\""))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                    continue;
                }
            }

            if trimmed.starts_with("version") {
                inserted = true;
            }
        }

        lines.push(line.to_string());
    }

    if in_project && !inserted {
        lines.push(format!("version = \"{version}\""));
        inserted = true;
    }

    if !saw_project {
        return Err(OpsError::Parse(
            "pyproject.toml missing [project] section".to_string(),
        ));
    }

    if inserted {
        return Ok(join_lines_like_original(content, lines));
    }

    Err(OpsError::Parse(
        "pyproject.toml missing version or dynamic version field".to_string(),
    ))
}

fn parse_toml_string_array(raw_items: &str) -> Vec<String> {
    raw_items
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.trim_matches('"').trim_matches('\'').to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn join_lines_like_original(original: &str, lines: Vec<String>) -> String {
    let mut joined = lines.join("\n");
    if original.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

fn tag_and_push(repo: &Path, version: &str, target: &ReleaseTarget) -> OpsResult<String> {
    tag_and_push_ref(repo, version, target, None)
}

fn tag_and_push_ref(
    repo: &Path,
    version: &str,
    target: &ReleaseTarget,
    target_ref: Option<&str>,
) -> OpsResult<String> {
    let tag = target_tag(target, version);
    let ref_name = target_ref.unwrap_or("HEAD");
    let target_sha = run_stdout(repo, "git", &["rev-parse", ref_name])?;
    let target_sha = target_sha.trim().to_string();

    if let Some(remote_sha) = remote_tag_sha(repo, &tag)? {
        if remote_sha == target_sha {
            eprintln!("Tag {tag} already exists on origin at target commit, skipping");
            return Ok(tag);
        }
        return Err(OpsError::Message(format!(
            "tag {tag} already exists on origin at {remote_sha}, expected {target_sha}"
        )));
    }

    match local_tag_sha(repo, &tag)? {
        Some(local_sha) if local_sha == target_sha => {}
        Some(local_sha) => {
            return Err(OpsError::Message(format!(
                "local tag {tag} exists at {local_sha}, expected {target_sha}"
            )));
        }
        None => {
            run_stdout(repo, "git", &["tag", &tag, ref_name])?;
        }
    }

    let push_output = run_output(repo, "git", &["push", "origin", &tag])?;
    if push_output.status.success() {
        return Ok(tag);
    }

    let stderr = String::from_utf8_lossy(&push_output.stderr).to_string();
    if stderr.contains("already exists")
        && remote_tag_sha(repo, &tag)?
            .as_deref()
            .is_some_and(|remote_sha| remote_sha == target_sha)
    {
        eprintln!("Tag {tag} was created concurrently, continuing");
        return Ok(tag);
    }

    Err(OpsError::CommandFailed {
        command: format!("git push origin {tag}"),
        stderr,
    })
}

fn local_tag_sha(repo: &Path, tag: &str) -> OpsResult<Option<String>> {
    let output = run_output(repo, "git", &["rev-list", "-n", "1", tag])?;
    if !output.status.success() {
        return Ok(None);
    }

    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sha.is_empty() {
        Ok(None)
    } else {
        Ok(Some(sha))
    }
}

fn remote_tag_sha(repo: &Path, tag: &str) -> OpsResult<Option<String>> {
    let pattern = format!("refs/tags/{tag}");
    let output = run_output(repo, "git", &["ls-remote", "--tags", "origin", &pattern])?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("git ls-remote --tags origin {pattern}"),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sha = stdout
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().next())
        .map(str::to_string);

    Ok(sha)
}

fn find_workflow_run(
    repo: &Path,
    tag: &str,
    target: &ReleaseTarget,
) -> OpsResult<Option<GhRunListEntry>> {
    let mut args = vec![
        "run".to_string(),
        "list".to_string(),
        "--json".to_string(),
        "headBranch,displayTitle,status,conclusion,url".to_string(),
        "--limit".to_string(),
        "50".to_string(),
    ];
    if let Some(workflow) = target.workflow.as_deref() {
        args.push("--workflow".to_string());
        args.push(workflow.to_string());
    }

    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = run_stdout(repo, "gh", arg_refs.as_slice())?;
    let runs: Vec<GhRunListEntry> = serde_json::from_str(&output)
        .map_err(|err| OpsError::Parse(format!("failed to parse workflow run list: {err}")))?;

    let matching = runs.into_iter().find(|run| {
        run.head_branch.as_deref() == Some(tag)
            || run
                .display_title
                .as_deref()
                .is_some_and(|title| title.contains(tag))
    });

    Ok(matching)
}

fn github_release_exists(repo: &Path, tag: &str) -> OpsResult<bool> {
    let output = run_output(repo, "gh", &["release", "view", tag, "--json", "tagName"])?;
    Ok(output.status.success())
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

fn run_output(repo: &Path, command: &str, args: &[&str]) -> OpsResult<Output> {
    Command::new(command)
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(OpsError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ======================================================================
    // bump_cargo_toml
    // ======================================================================

    #[test]
    fn cargo_toml_workspace_package() {
        let input = r#"[workspace]
members = ["crates/*"]

[workspace.package]
version = "0.9.2"
edition = "2021"
"#;
        let result = bump_cargo_toml(input, "0.9.3").unwrap();
        assert!(result.contains(r#"version = "0.9.3""#));
        assert!(!result.contains("0.9.2"));
    }

    #[test]
    fn cargo_toml_package_section() {
        let input = r#"[package]
name = "mylib"
version = "1.0.0"
edition = "2021"
"#;
        let result = bump_cargo_toml(input, "1.1.0").unwrap();
        assert!(result.contains(r#"version = "1.1.0""#));
        assert!(!result.contains("1.0.0"));
    }

    #[test]
    fn cargo_toml_prefers_workspace_over_package() {
        let input = r#"[workspace.package]
version = "2.0.0"
edition = "2021"

[package]
name = "sub"
version.workspace = true
"#;
        let result = bump_cargo_toml(input, "2.1.0").unwrap();
        assert!(result.contains(r#"version = "2.1.0""#));
        // workspace version bumped, package version.workspace = true untouched
        assert!(result.contains("version.workspace = true"));
    }

    #[test]
    fn cargo_toml_no_version_errors() {
        let input = "[package]\nname = \"noversion\"\n";
        assert!(bump_cargo_toml(input, "1.0.0").is_err());
    }

    #[test]
    fn cargo_toml_preserves_trailing_newline() {
        let input = "[package]\nversion = \"0.1.0\"\n";
        let result = bump_cargo_toml(input, "0.2.0").unwrap();
        assert!(result.ends_with('\n'));
    }

    #[test]
    fn cargo_toml_single_quotes() {
        let input = "[package]\nversion = '0.1.0'\n";
        let result = bump_cargo_toml(input, "0.2.0").unwrap();
        assert!(result.contains(r#"version = "0.2.0""#));
    }

    // ======================================================================
    // bump_package_json
    // ======================================================================

    #[test]
    fn package_json_basic() {
        let input = r#"{
  "name": "mypackage",
  "version": "1.2.3",
  "description": "test"
}"#;
        let result = bump_package_json(input, "1.3.0").unwrap();
        assert!(result.contains(r#""version": "1.3.0""#));
        assert!(!result.contains("1.2.3"));
    }

    #[test]
    fn package_json_only_first_version() {
        let input = r#"{
  "version": "1.0.0",
  "dependencies": {
    "foo": { "version": "2.0.0" }
  }
}"#;
        let result = bump_package_json(input, "1.1.0").unwrap();
        assert!(result.contains(r#""version": "1.1.0""#));
        // second "version" untouched
        assert!(result.contains(r#""version": "2.0.0""#));
    }

    #[test]
    fn package_json_no_version_errors() {
        let input = r#"{ "name": "noversion" }"#;
        assert!(bump_package_json(input, "1.0.0").is_err());
    }

    // ======================================================================
    // bump_pyproject_toml
    // ======================================================================

    #[test]
    fn pyproject_static_version() {
        let input = r#"[project]
name = "mypkg"
version = "0.5.0"
"#;
        let result = bump_pyproject_toml(input, "0.6.0").unwrap();
        assert!(result.contains(r#"version = "0.6.0""#));
        assert!(!result.contains("0.5.0"));
    }

    #[test]
    fn pyproject_dynamic_version() {
        let input = r#"[project]
name = "mypkg"
dynamic = ["version"]
"#;
        let result = bump_pyproject_toml(input, "1.0.0").unwrap();
        assert!(result.contains(r#"version = "1.0.0""#));
        assert!(!result.contains("dynamic"));
    }

    #[test]
    fn pyproject_dynamic_version_with_other_dynamic_fields() {
        let input = r#"[project]
name = "mypkg"
dynamic = ["version", "readme"]
"#;
        let result = bump_pyproject_toml(input, "1.0.0").unwrap();
        assert!(result.contains(r#"version = "1.0.0""#));
        assert!(result.contains(r#"dynamic = ["readme"]"#));
    }

    #[test]
    fn pyproject_no_project_section_errors() {
        let input = "[tool.something]\nfoo = 1\n";
        assert!(bump_pyproject_toml(input, "1.0.0").is_err());
    }

    #[test]
    fn pyproject_preserves_trailing_newline() {
        let input = "[project]\nversion = \"0.1.0\"\n";
        let result = bump_pyproject_toml(input, "0.2.0").unwrap();
        assert!(result.ends_with('\n'));
    }

    // ======================================================================
    // replace_toml_version_in_section
    // ======================================================================

    #[test]
    fn toml_version_replace_preserves_surrounding() {
        let input = r#"[package]
name = "test"
version = "0.1.0"
edition = "2021"
"#;
        let result = replace_toml_version_in_section(input, "[package]", "0.2.0")
            .unwrap()
            .unwrap();
        assert!(result.contains("name = \"test\""));
        assert!(result.contains("edition = \"2021\""));
        assert!(result.contains(r#"version = "0.2.0""#));
    }

    #[test]
    fn toml_version_wrong_section_returns_none() {
        let input = "[other]\nversion = \"0.1.0\"\n";
        let result = replace_toml_version_in_section(input, "[package]", "0.2.0").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn toml_version_only_replaces_in_target_section() {
        let input = r#"[package]
version = "1.0.0"

[dependencies]
version = "2.0.0"
"#;
        let result = replace_toml_version_in_section(input, "[package]", "1.1.0")
            .unwrap()
            .unwrap();
        assert!(result.contains(r#"version = "1.1.0""#));
        // dependency section untouched
        assert!(result.contains("[dependencies]\nversion = \"2.0.0\""));
    }

    // ======================================================================
    // area_matches / glob helpers
    // ======================================================================

    #[test]
    fn area_matches_directory_prefix() {
        let patterns = vec!["src/api/".to_string()];
        assert!(area_matches("src/api/handler.rs", &patterns));
        assert!(!area_matches("src/web/handler.rs", &patterns));
    }

    #[test]
    fn area_matches_exact_file() {
        let patterns = vec!["README.md".to_string()];
        assert!(area_matches("README.md", &patterns));
        assert!(!area_matches("docs/README.md", &patterns));
    }

    #[test]
    fn area_matches_glob_star() {
        let patterns = vec!["src/*.rs".to_string()];
        assert!(area_matches("src/main.rs", &patterns));
        assert!(!area_matches("src/sub/main.rs", &patterns));
    }

    #[test]
    fn area_matches_glob_doublestar() {
        let patterns = vec!["src/**/*.rs".to_string()];
        assert!(area_matches("src/sub/main.rs", &patterns));
        assert!(area_matches("src/a/b/c/main.rs", &patterns));
        // **/ requires at least one directory level between src/ and the filename
        assert!(!area_matches("src/main.rs", &patterns));
    }

    #[test]
    fn area_matches_normalizes_dot_slash() {
        let patterns = vec!["./src/".to_string()];
        assert!(area_matches("src/main.rs", &patterns));
        assert!(area_matches("./src/main.rs", &patterns));
    }

    // ======================================================================
    // version helpers
    // ======================================================================

    #[test]
    fn version_from_tag_default_target() {
        let target = ReleaseTarget {
            name: "default".to_string(),
            area: vec![],
            tag_prefix: String::new(),
            manifests: vec![],
            workflow: None,
        };
        assert_eq!(version_from_tag("v1.2.3", &target).unwrap(), "1.2.3");
    }

    #[test]
    fn version_from_tag_scoped_target() {
        let target = ReleaseTarget {
            name: "cli".to_string(),
            area: vec![],
            tag_prefix: "cli/".to_string(),
            manifests: vec![],
            workflow: None,
        };
        assert_eq!(version_from_tag("cli/v1.2.3", &target).unwrap(), "1.2.3");
    }

    #[test]
    fn version_from_tag_prefix_mismatch_errors() {
        let target = ReleaseTarget {
            name: "cli".to_string(),
            area: vec![],
            tag_prefix: "cli/".to_string(),
            manifests: vec![],
            workflow: None,
        };
        assert!(version_from_tag("v1.2.3", &target).is_err());
    }

    #[test]
    fn target_tag_default() {
        let target = ReleaseTarget {
            name: "default".to_string(),
            area: vec![],
            tag_prefix: String::new(),
            manifests: vec![],
            workflow: None,
        };
        assert_eq!(target_tag(&target, "1.0.0"), "v1.0.0");
    }

    #[test]
    fn target_tag_scoped() {
        let target = ReleaseTarget {
            name: "cli".to_string(),
            area: vec![],
            tag_prefix: "cli/".to_string(),
            manifests: vec![],
            workflow: None,
        };
        assert_eq!(target_tag(&target, "2.0.0"), "cli/v2.0.0");
    }
}
