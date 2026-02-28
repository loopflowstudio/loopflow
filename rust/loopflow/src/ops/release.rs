use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

use regex::Regex;
use serde::Deserialize;

use crate::engine::agent::{
    launch_agent, AgentCapabilities, AgentConfig, LaunchResult, ProcessConfig,
};
use crate::engine::builtins::get_builtin_ops_prompt;
use crate::engine::command::run_command;
use crate::engine::config::{load_config_or_default, Config, ReleaseTargetConfig};
use crate::engine::git::{delete_local_branch, get_default_branch, sync_main, worktree_remove};
use crate::engine::worktrees::{create_with_schema, main_repo_root};
use crate::ops::commit::{commit_workflow, CommitOptions};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::land::{land, LandOptions};
use crate::ops::progress::Progress;
use crate::ops::util::command_exists;

/// A merged PR with enough context for release notes.
#[derive(Debug, Clone)]
struct MergedPr {
    number: u32,
    title: String,
    body: Option<String>,
    files: Vec<String>,
    additions: u64,
    deletions: u64,
    changed_files: u64,
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
    #[serde(rename = "databaseId")]
    id: u64,
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
struct GhRunView {
    status: String,
    #[serde(default)]
    conclusion: Option<String>,
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
pub struct PublishOptions {
    pub version_input: String,
    pub dry_run: bool,
    pub target: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PublishResult {
    pub version: Option<String>,
    pub target: String,
    pub bootstrapped: bool,
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

/// Full release workflow: sync main, create worktree, bump versions, generate
/// release notes, land PR, create tag, and monitor workflow outcomes.
pub fn publish_release(
    repo: &Path,
    options: &PublishOptions,
    progress: &impl Progress,
) -> OpsResult<PublishResult> {
    if !command_exists("gh") {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let config = load_config_or_default(Some(&main_repo));
    let target = resolve_target(&config, options.target.as_deref(), &main_repo)?;

    if needs_bootstrap(&main_repo, &target)? {
        if options.dry_run {
            progress.status(&format!(
                "Would start release bootstrap for target '{}'",
                target.name
            ));
            return Ok(PublishResult {
                version: None,
                target: target.name,
                bootstrapped: true,
            });
        }

        progress.status("No tags/workflow detected. Starting release bootstrap...");
        bootstrap_release(&main_repo, &target)?;
        progress.status("Bootstrap complete. Re-run `lf ops release`.");

        return Ok(PublishResult {
            version: None,
            target: target.name,
            bootstrapped: true,
        });
    }

    let prev_tag = latest_tag(&main_repo, &target)?;
    let version = resolve_version(&prev_tag, &options.version_input, &target)?;

    // Check for a release that's prepared (notes + manifests) but not yet tagged,
    // or already tagged but with a failed workflow. Resume instead of starting fresh.
    if let Some(result) =
        try_resume_release(&main_repo, &version, &target, options.dry_run, progress)?
    {
        return Ok(result);
    }

    let wt_name = if target.name == "default" {
        format!("release-v{version}")
    } else {
        format!("release-{}-v{version}", target.name)
    };

    if options.dry_run {
        progress.status(&format!("Would publish {}", target_tag(&target, &version)));
        progress.status(&format!("  Target: {}", target.name));
        progress.status(&format!("  Create worktree: {wt_name}"));
        progress.status(&format!(
            "  Bump versions in {} manifest(s)",
            target.manifests.len()
        ));
        progress.status("  Generate release notes");
        progress.status("  Commit, PR, and land");
        progress.status(&format!("  Tag {} and push", target_tag(&target, &version)));
        progress.status("  Monitor release workflow + GitHub Release");
        return Ok(PublishResult {
            version: Some(version),
            target: target.name,
            bootstrapped: false,
        });
    }

    progress.status("Syncing main...");
    let main_branch = get_default_branch(&main_repo)?;
    sync_main(&main_repo, &main_branch)?;

    progress.status(&format!("Creating worktree {wt_name}..."));
    let wt = create_with_schema(&main_repo, &wt_name, Some(&main_branch), None)?;
    let wt_path = wt.path;
    let wt_branch = wt.branch;

    let publish_result = publish_in_worktree(&wt_path, &version, &target, progress);

    cleanup_worktree(&main_repo, &wt_path, &wt_branch, progress);

    publish_result?;

    progress.status("Syncing main after merge...");
    sync_main(&main_repo, &main_branch)?;

    progress.status(&format!("Tagging {}...", target_tag(&target, &version)));
    let tag = tag_and_push(&main_repo, &version, &target)?;

    monitor_release_workflow(&main_repo, &tag, &target, progress)?;

    progress.status(&format!("{} published.", target_tag(&target, &version)));
    Ok(PublishResult {
        version: Some(version),
        target: target.name,
        bootstrapped: false,
    })
}

/// Check if a release for `version` is already prepared or tagged, and resume it.
///
/// Returns `Some(result)` if the release was resumed (or already complete).
/// Returns `None` to fall through to the fresh workflow.
fn try_resume_release(
    repo: &Path,
    version: &str,
    target: &ReleaseTarget,
    dry_run: bool,
    progress: &impl Progress,
) -> OpsResult<Option<PublishResult>> {
    let notes_version = read_release_notes_version(repo);
    if notes_version.as_deref() != Some(version) {
        return Ok(None);
    }

    if !manifests_at_version(repo, target, version) {
        return Ok(None);
    }

    let tag = target_tag(target, version);
    let remote_tag = tag_exists_remote(repo, &tag)?;

    if remote_tag {
        // Tag exists on remote — check workflow/release status.
        let release_exists = github_release_exists(repo, &tag)?;
        let workflow = find_workflow_run(repo, &tag, target)?;

        let workflow_ok = workflow
            .as_ref()
            .is_some_and(|r| r.conclusion.as_deref() == Some("success"));

        if release_exists && workflow_ok {
            progress.status(&format!("{tag} already released."));
            return Ok(Some(PublishResult {
                version: Some(version.to_string()),
                target: target.name.clone(),
                bootstrapped: false,
            }));
        }

        if dry_run {
            progress.status(&format!(
                "Would resume {tag} (tag exists, monitoring workflow)"
            ));
            return Ok(Some(PublishResult {
                version: Some(version.to_string()),
                target: target.name.clone(),
                bootstrapped: false,
            }));
        }

        // Tag pushed but workflow incomplete or failed — monitor/diagnose.
        progress.status(&format!(
            "Resuming {tag} (tag exists, checking workflow)..."
        ));
        monitor_release_workflow(repo, &tag, target, progress)?;

        progress.status(&format!("{tag} published."));
        return Ok(Some(PublishResult {
            version: Some(version.to_string()),
            target: target.name.clone(),
            bootstrapped: false,
        }));
    }

    // Notes + manifests ready but no remote tag — sync, tag, and push.
    if dry_run {
        progress.status(&format!(
            "Would resume {tag} (notes and manifests ready, tagging)"
        ));
        return Ok(Some(PublishResult {
            version: Some(version.to_string()),
            target: target.name.clone(),
            bootstrapped: false,
        }));
    }

    progress.status(&format!(
        "Resuming {tag} (notes and manifests ready, tagging)..."
    ));
    let main_branch = get_default_branch(repo)?;
    sync_main(repo, &main_branch)?;

    let pushed_tag = tag_and_push(repo, version, target)?;
    monitor_release_workflow(repo, &pushed_tag, target, progress)?;

    progress.status(&format!("{tag} published."));
    Ok(Some(PublishResult {
        version: Some(version.to_string()),
        target: target.name.clone(),
        bootstrapped: false,
    }))
}

pub fn release_status(repo: &Path, target_name: Option<&str>) -> OpsResult<ReleaseStatusResult> {
    if !command_exists("gh") {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let config = load_config_or_default(Some(&main_repo));
    let target = resolve_target(&config, target_name, &main_repo)?;

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
    let notes = generate_release_notes(repo, &prs, &version, &prev_tag, target)?;

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

/// Read the version from the first line of RELEASE_NOTES.md (`# v{X.Y.Z}`).
fn read_release_notes_version(repo: &Path) -> Option<String> {
    let path = repo.join("RELEASE_NOTES.md");
    let content = fs::read_to_string(path).ok()?;
    let first_line = content.lines().next()?.trim();
    let version = first_line.strip_prefix("# v")?;
    if version.split('.').count() == 3 {
        Some(version.to_string())
    } else {
        None
    }
}

/// Check whether all manifests in the target already have the given version.
fn manifests_at_version(repo: &Path, target: &ReleaseTarget, version: &str) -> bool {
    if target.manifests.is_empty() {
        return false;
    }

    target.manifests.iter().all(|manifest| {
        let path = repo.join(manifest);
        let Ok(content) = fs::read_to_string(&path) else {
            return false;
        };
        match manifest.file_name().and_then(|n| n.to_str()) {
            Some("Cargo.toml") => toml_has_version(&content, version),
            Some("package.json") => json_has_version(&content, version),
            Some("pyproject.toml") => toml_has_version(&content, version),
            _ => false,
        }
    })
}

fn toml_has_version(content: &str, version: &str) -> bool {
    let target = format!("version = \"{version}\"");
    content.lines().any(|line| line.trim() == target)
}

fn json_has_version(content: &str, version: &str) -> bool {
    let target = format!("\"version\": \"{version}\"");
    content
        .lines()
        .any(|line| line.replace(' ', "").contains(&target.replace(' ', "")))
}

/// Check if a tag exists on the remote.
fn tag_exists_remote(repo: &Path, tag: &str) -> OpsResult<bool> {
    let output = run_stdout(repo, "git", &["ls-remote", "--tags", "origin", tag])?;
    Ok(!output.trim().is_empty())
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
    repo: &Path,
    prs: &[MergedPr],
    version: &str,
    prev_tag: &str,
    target: &ReleaseTarget,
) -> OpsResult<String> {
    let template = get_builtin_ops_prompt("release_notes")
        .ok_or_else(|| OpsError::Message("builtin release_notes prompt not found".to_string()))?
        .replace("{version}", version)
        .replace("{target_name}", &target.name);

    let pr_summary = if prs.is_empty() {
        "- No merged PRs found in this window.".to_string()
    } else {
        prs.iter()
            .map(format_pr_for_prompt)
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    let area_scope = display_area_scope(target);

    let previous_notes = fs::read_to_string(repo.join("RELEASE_NOTES.md")).unwrap_or_default();
    let previous_notes_section = if previous_notes.trim().is_empty() {
        String::new()
    } else {
        format!("\n\n## Previous release notes\n\n{previous_notes}")
    };

    let prompt = format!(
        "{template}\n\n## Release context\n\n- Target version: v{version}\n- Previous tag: {prev_tag}\n- Release target: {}\n- Tag prefix: {}\n- Area scope: {}\n\n## Merged PRs\n\n{pr_summary}{previous_notes_section}\n",
        target.name,
        display_tag_prefix(target),
        area_scope
    );

    let result = launch_ops_agent(repo, prompt, true)?;
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
    let header = format!(
        "- #{} {} (+{} -{}, {} files)",
        pr.number, pr.title, pr.additions, pr.deletions, pr.changed_files
    );
    match pr.body.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
        Some(body) => format!("{header}\n  Body: {body}"),
        None => header,
    }
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

fn publish_in_worktree(
    wt_path: &Path,
    version: &str,
    target: &ReleaseTarget,
    progress: &impl Progress,
) -> OpsResult<()> {
    progress.status(&format!(
        "Bumping manifest versions for {}...",
        target_tag(target, version)
    ));
    bump_manifest_versions(wt_path, target, version, progress)?;

    progress.status(&format!(
        "Generating release notes for {}...",
        target_tag(target, version)
    ));
    generate_release_with_target(wt_path, version, target, progress)?;

    progress.status("Committing release changes...");
    commit_workflow(
        wt_path,
        &CommitOptions {
            add: true,
            message: Some(release_commit_message(target, version)),
            ..CommitOptions::for_task("release")
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

fn release_commit_message(target: &ReleaseTarget, version: &str) -> String {
    if target.name == "default" {
        format!("release: v{version}")
    } else {
        format!("release: {} v{version}", target.name)
    }
}

fn cleanup_worktree(main_repo: &Path, wt_path: &Path, branch: &str, progress: &impl Progress) {
    if let Err(err) = worktree_remove(main_repo, wt_path) {
        progress.error(&format!("Warning: could not remove worktree: {err}"));
    }
    let _ = delete_local_branch(main_repo, branch);
}

fn tag_and_push(repo: &Path, version: &str, target: &ReleaseTarget) -> OpsResult<String> {
    let tag = target_tag(target, version);
    run_stdout(repo, "git", &["tag", &tag])?;
    run_stdout(repo, "git", &["push", "origin", &tag])?;
    Ok(tag)
}

fn monitor_release_workflow(
    repo: &Path,
    tag: &str,
    target: &ReleaseTarget,
    progress: &impl Progress,
) -> OpsResult<()> {
    monitor_release_workflow_inner(repo, tag, target, progress, 0)
}

fn monitor_release_workflow_inner(
    repo: &Path,
    tag: &str,
    target: &ReleaseTarget,
    progress: &impl Progress,
    attempt: u8,
) -> OpsResult<()> {
    if attempt > 2 {
        return Err(OpsError::Message(
            "release diagnosis retry limit reached".to_string(),
        ));
    }

    progress.status("Waiting for release workflow run...");
    let run = wait_for_workflow_run(repo, tag, target)?;
    let Some(run) = run else {
        return Err(OpsError::Message(format!(
            "no workflow run found for tag {tag}"
        )));
    };

    if let Some(url) = run.url.as_deref() {
        progress.status(&format!("Workflow: {url}"));
    }

    loop {
        let view = view_workflow_run(repo, run.id)?;
        let conclusion = view
            .conclusion
            .clone()
            .unwrap_or_else(|| "(pending)".to_string());
        progress.status(&format!(
            "Workflow run {} status: {} / {}",
            run.id, view.status, conclusion
        ));

        if view.status == "completed" {
            let release_exists = github_release_exists(repo, tag)?;

            if view.conclusion.as_deref() == Some("success") {
                if release_exists {
                    progress.status("GitHub Release is published.");
                } else {
                    progress.error("Workflow succeeded but GitHub Release not found yet.");
                }
                return Ok(());
            }

            // Workflow failed, but the release job may have succeeded
            // (e.g. publish-crates failed after GitHub Release was created).
            // Don't offer retag if the release already exists.
            let conclusion = view.conclusion.unwrap_or_else(|| "unknown".to_string());
            if release_exists {
                progress.error(&format!(
                    "Release workflow failed ({conclusion}), but GitHub Release is published. \
                     Check failed jobs: gh run view {}",
                    run.id
                ));
                return Err(OpsError::Message(
                    "release workflow partially failed; GitHub Release exists".to_string(),
                ));
            }

            let logs = workflow_logs(repo, run.id)
                .unwrap_or_else(|err| format!("failed to fetch workflow logs: {err}"));
            progress.error(&format!("Release workflow failed: {conclusion}"));
            if !logs.trim().is_empty() {
                progress.error(&logs);
            }

            if !progress.confirm("Diagnose with agent?") {
                return Err(OpsError::Message(
                    "release workflow failed; diagnosis skipped".to_string(),
                ));
            }

            diagnose_release_failure(repo, tag, target, &logs)?;
            retag(repo, tag)?;
            return monitor_release_workflow_inner(repo, tag, target, progress, attempt + 1);
        }

        thread::sleep(Duration::from_secs(15));
    }
}

fn wait_for_workflow_run(
    repo: &Path,
    tag: &str,
    target: &ReleaseTarget,
) -> OpsResult<Option<GhRunListEntry>> {
    thread::sleep(Duration::from_secs(5));
    for _ in 0..20 {
        if let Some(run) = find_workflow_run(repo, tag, target)? {
            return Ok(Some(run));
        }
        thread::sleep(Duration::from_secs(5));
    }
    Ok(None)
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
        "databaseId,headBranch,displayTitle,status,conclusion,url".to_string(),
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

fn view_workflow_run(repo: &Path, run_id: u64) -> OpsResult<GhRunView> {
    let output = run_stdout(
        repo,
        "gh",
        &[
            "run",
            "view",
            &run_id.to_string(),
            "--json",
            "status,conclusion,url",
        ],
    )?;

    serde_json::from_str(&output)
        .map_err(|err| OpsError::Parse(format!("failed to parse workflow run view: {err}")))
}

fn workflow_logs(repo: &Path, run_id: u64) -> OpsResult<String> {
    run_stdout(repo, "gh", &["run", "view", &run_id.to_string(), "--log"])
}

fn github_release_exists(repo: &Path, tag: &str) -> OpsResult<bool> {
    let output = run_output(repo, "gh", &["release", "view", tag, "--json", "tagName"])?;
    Ok(output.status.success())
}

fn diagnose_release_failure(
    repo: &Path,
    tag: &str,
    target: &ReleaseTarget,
    logs: &str,
) -> OpsResult<()> {
    let template = get_builtin_ops_prompt("release_diagnose").ok_or_else(|| {
        OpsError::Message("builtin release_diagnose prompt not found".to_string())
    })?;

    let clipped_logs = clip_text(logs, 20_000);
    let area_scope = display_area_scope(target);
    let prompt = format!(
        "{template}\n\n## Failure context\n\n- Tag: {tag}\n- Target: {}\n- Tag prefix: {}\n- Area scope: {}\n\n## Workflow logs\n\n{}\n",
        target.name,
        display_tag_prefix(target),
        area_scope,
        clipped_logs
    );

    let result = launch_ops_agent(repo, prompt, false)?;
    if result.exit_code != 0 {
        return Err(OpsError::AgentFailed(
            "diagnosis agent session failed".to_string(),
        ));
    }

    Ok(())
}

fn retag(repo: &Path, tag: &str) -> OpsResult<()> {
    let _ = run_stdout(repo, "git", &["tag", "-d", tag]);
    let delete_ref = format!(":refs/tags/{tag}");
    let _ = run_stdout(repo, "git", &["push", "origin", &delete_ref]);

    run_stdout(repo, "git", &["tag", tag])?;
    run_stdout(repo, "git", &["push", "origin", tag])?;
    Ok(())
}

fn clip_text(text: &str, max_chars: usize) -> String {
    let mut clipped = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        clipped.push_str("\n\n... (truncated)");
    }
    clipped
}

fn needs_bootstrap(repo: &Path, target: &ReleaseTarget) -> OpsResult<bool> {
    let has_tag = latest_tag_optional(repo, target)?.is_some();
    let has_workflow = has_release_workflow(repo, target)?;
    Ok(!has_tag && !has_workflow)
}

fn has_release_workflow(repo: &Path, target: &ReleaseTarget) -> OpsResult<bool> {
    if let Some(workflow) = target.workflow.as_deref() {
        let configured = repo.join(workflow);
        if configured.exists() {
            return Ok(true);
        }
    }

    let workflows_dir = repo.join(".github").join("workflows");
    if !workflows_dir.exists() {
        return Ok(false);
    }

    let tag_pattern = format!("{}v*", target.tag_prefix);
    let entries = fs::read_dir(&workflows_dir)?;
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let extension = path.extension().and_then(|ext| ext.to_str());
        if !matches!(extension, Some("yml") | Some("yaml")) {
            continue;
        }

        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        if content.contains("tags:") && content.contains(&tag_pattern) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn bootstrap_release(repo: &Path, target: &ReleaseTarget) -> OpsResult<()> {
    let template = get_builtin_ops_prompt("release_init")
        .ok_or_else(|| OpsError::Message("builtin release_init prompt not found".to_string()))?;

    let manifests = if target.manifests.is_empty() {
        "(none detected)".to_string()
    } else {
        target
            .manifests
            .iter()
            .map(|path| format!("- {}", path.display()))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let area_scope = display_area_scope(target);
    let prompt = format!(
        "{template}\n\n## Release target\n\n- Name: {}\n- Tag prefix: {}\n- Area scope: {}\n\n## Manifest files\n\n{}\n",
        target.name,
        display_tag_prefix(target),
        area_scope,
        manifests
    );

    let result = launch_ops_agent(repo, prompt, false)?;
    if result.exit_code != 0 {
        return Err(OpsError::AgentFailed(
            "release bootstrap session failed".to_string(),
        ));
    }

    Ok(())
}

fn launch_ops_agent(repo: &Path, prompt: String, auto: bool) -> OpsResult<LaunchResult> {
    let config = load_config_or_default(Some(repo));
    let launch = AgentConfig {
        task_prompt: prompt,
        agent: config.agent.clone(),
        skip_permissions: true,
        cwd: Some(repo.to_path_buf()),
        ..Default::default()
    };
    let process = ProcessConfig {
        auto,
        stream: false,
        ..Default::default()
    };
    let capabilities = AgentCapabilities {
        chrome: config.chrome,
    };
    launch_agent(&launch, &process, &capabilities)
        .map_err(|err| OpsError::AgentFailed(err.to_string()))
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

    // ======================================================================
    // read_release_notes_version
    // ======================================================================

    #[test]
    fn release_notes_version_valid() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("RELEASE_NOTES.md"), "# v0.9.5\n\nNotes.").unwrap();
        assert_eq!(
            read_release_notes_version(dir.path()),
            Some("0.9.5".to_string())
        );
    }

    #[test]
    fn release_notes_version_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_release_notes_version(dir.path()), None);
    }

    #[test]
    fn release_notes_version_no_header() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("RELEASE_NOTES.md"), "Just some text.").unwrap();
        assert_eq!(read_release_notes_version(dir.path()), None);
    }

    #[test]
    fn release_notes_version_invalid_format() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("RELEASE_NOTES.md"),
            "# v1.0\n\nOnly two parts.",
        )
        .unwrap();
        assert_eq!(read_release_notes_version(dir.path()), None);
    }

    // ======================================================================
    // manifests_at_version
    // ======================================================================

    #[test]
    fn manifests_match_cargo_toml() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace.package]\nversion = \"1.2.3\"\n",
        )
        .unwrap();
        let target = ReleaseTarget {
            name: "default".to_string(),
            area: vec![],
            tag_prefix: String::new(),
            manifests: vec![PathBuf::from("Cargo.toml")],
            workflow: None,
        };
        assert!(manifests_at_version(dir.path(), &target, "1.2.3"));
        assert!(!manifests_at_version(dir.path(), &target, "1.2.4"));
    }

    #[test]
    fn manifests_match_pyproject_toml() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nversion = \"0.5.0\"\n",
        )
        .unwrap();
        let target = ReleaseTarget {
            name: "default".to_string(),
            area: vec![],
            tag_prefix: String::new(),
            manifests: vec![PathBuf::from("pyproject.toml")],
            workflow: None,
        };
        assert!(manifests_at_version(dir.path(), &target, "0.5.0"));
        assert!(!manifests_at_version(dir.path(), &target, "0.6.0"));
    }

    #[test]
    fn manifests_empty_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        let target = ReleaseTarget {
            name: "default".to_string(),
            area: vec![],
            tag_prefix: String::new(),
            manifests: vec![],
            workflow: None,
        };
        assert!(!manifests_at_version(dir.path(), &target, "1.0.0"));
    }

    #[test]
    fn manifests_partial_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nversion = \"0.9.0\"\n",
        )
        .unwrap();
        let target = ReleaseTarget {
            name: "default".to_string(),
            area: vec![],
            tag_prefix: String::new(),
            manifests: vec![PathBuf::from("Cargo.toml"), PathBuf::from("pyproject.toml")],
            workflow: None,
        };
        // Both must match for true
        assert!(!manifests_at_version(dir.path(), &target, "1.0.0"));
    }

    // ======================================================================
    // toml_has_version / json_has_version
    // ======================================================================

    #[test]
    fn toml_version_check() {
        assert!(toml_has_version("version = \"1.0.0\"\n", "1.0.0"));
        assert!(!toml_has_version("version = \"1.0.0\"\n", "2.0.0"));
        assert!(toml_has_version(
            "[package]\nversion = \"1.0.0\"\nedition = \"2021\"\n",
            "1.0.0"
        ));
    }

    #[test]
    fn json_version_check() {
        assert!(json_has_version("{\n  \"version\": \"1.0.0\"\n}", "1.0.0"));
        assert!(!json_has_version("{\n  \"version\": \"1.0.0\"\n}", "2.0.0"));
    }
}
