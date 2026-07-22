use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, Instant};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::engine::command::{run_command, CommandError};
use crate::engine::config::{
    load_config_or_default, Config, ReleaseCompletion, ReleaseTargetConfig,
};
use crate::engine::git::{
    acquire_worktree_lease, delete_local_branch, fetch, get_default_branch, sync_main,
    worktree_remove, worktree_remove_owned, WorktreeLease,
};
use crate::engine::naming::{git_user, sanitize_for_branch};
use crate::engine::worktrees::{create_named_worktree, main_repo_root, worktree_path};
use crate::ops::commit::{commit_workflow, CommitOptions};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::land::{finish_land_after_rebase, LandOptions};
use crate::ops::pr::PrCopy;
use crate::ops::progress::Progress;
use crate::ops::util::command_exists;

const RELEASE_QUEUE_PR_LIMIT: usize = 200;
const RELEASE_CONTEXT_MAX_BYTES: usize = 128 * 1024;
const RELEASE_CONTEXT_MAX_COMMITS: usize = 256;
const RELEASE_CONTEXT_MAX_PRS: usize = RELEASE_QUEUE_PR_LIMIT;
const RELEASE_CONTEXT_MAX_FILES_PER_CHANGE: usize = 100;
const RELEASE_CONTEXT_MAX_AREA_SCOPES: usize = 32;
const RELEASE_CONTEXT_MAX_NAME_BYTES: usize = 256;
const RELEASE_CONTEXT_MAX_TITLE_BYTES: usize = 300;
const RELEASE_CONTEXT_MAX_PATH_BYTES: usize = 512;
const RELEASE_CONTEXT_MAX_PR_BODY_BYTES: usize = 4 * 1024;
const RELEASE_CONTEXT_MAX_DECISIONS_BYTES: usize = 16 * 1024;
const RELEASE_CONTEXT_MAX_PREVIOUS_NOTES_BYTES: usize = 16 * 1024;
const RELEASE_NOTES_MAX_BYTES: usize = 60 * 1024;
const FALLBACK_NOTES_MAX_COMMITS: usize = 50;
const FALLBACK_NOTES_MAX_PRS: usize = 50;
const RELEASE_NOTES_STATUS_PREFIX: &str = "<!-- loopflow:release-notes=";
const RELEASE_WORKTREE_CONTEXT_ENV: [&str; 5] = [
    crate::durable::RUN_CONTEXT_ENV,
    "LF_RUN_ID",
    crate::durable::RUN_LEASE_ENV,
    crate::durable::AGENT_INVOCATION_ENV,
    crate::engine::wave_context::WAVE_ID_ENV,
];

struct ReleaseWorktreeContext {
    previous: Vec<(&'static str, Option<OsString>)>,
}

impl ReleaseWorktreeContext {
    fn enter() -> Self {
        let previous = RELEASE_WORKTREE_CONTEXT_ENV
            .iter()
            .map(|name| {
                let value = std::env::var_os(name);
                std::env::remove_var(name);
                (*name, value)
            })
            .collect();
        Self { previous }
    }
}

impl Drop for ReleaseWorktreeContext {
    fn drop(&mut self) {
        for (name, value) in &self.previous {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

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
    pub merge_commit: Option<String>,
}

/// A commit in the exact git range being released.
#[derive(Debug, Clone, Serialize)]
pub struct ReleaseCommit {
    pub sha: String,
    pub title: String,
    pub files: Vec<String>,
}

/// Git changes are release truth; merged PRs enrich their narrative.
#[derive(Debug, Clone, Serialize)]
pub struct ReleaseChangeSet {
    pub previous_tag: Option<String>,
    pub commits: Vec<ReleaseCommit>,
    pub merged_prs: Vec<MergedPr>,
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
    #[serde(default, rename = "mergeCommit")]
    merge_commit: Option<GhPrMergeCommit>,
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
    database_id: u64,
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
struct GhPrMergeCommit {
    oid: String,
}

#[derive(Debug, Deserialize)]
struct GhPrView {
    state: String,
    #[serde(rename = "mergeStateStatus")]
    merge_state_status: String,
    #[serde(default, rename = "mergeCommit")]
    merge_commit: Option<GhPrMergeCommit>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhReleasePr {
    number: u64,
    state: String,
    #[serde(default, rename = "mergeCommit")]
    merge_commit: Option<GhPrMergeCommit>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default, rename = "headRefOid")]
    head_ref_oid: Option<String>,
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
            merge_commit: value.merge_commit.map(|commit| commit.oid),
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
    verify: Vec<String>,
    prepare: Vec<String>,
    completion: ReleaseCompletion,
    publisher: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ReleaseStatusResult {
    pub target: String,
    pub latest_tag: Option<String>,
    pub notes_status: Option<ReleaseNotesStatus>,
    pub workflow_status: Option<String>,
    pub workflow_conclusion: Option<String>,
    pub workflow_url: Option<String>,
    pub release_exists: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReleaseNotesDegradation {
    MissingCli,
    Cooldown,
    Quota,
    Authentication,
    RateLimit,
    ProviderUnavailable,
}

impl ReleaseNotesDegradation {
    fn slug(self) -> &'static str {
        match self {
            Self::MissingCli => "missing-cli",
            Self::Cooldown => "cooldown",
            Self::Quota => "quota",
            Self::Authentication => "authentication",
            Self::RateLimit => "rate-limit",
            Self::ProviderUnavailable => "provider-unavailable",
        }
    }

    fn from_slug(value: &str) -> Option<Self> {
        match value {
            "missing-cli" => Some(Self::MissingCli),
            "cooldown" => Some(Self::Cooldown),
            "quota" => Some(Self::Quota),
            "authentication" => Some(Self::Authentication),
            "rate-limit" => Some(Self::RateLimit),
            "provider-unavailable" => Some(Self::ProviderUnavailable),
            _ => None,
        }
    }
}

impl std::fmt::Display for ReleaseNotesDegradation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.slug())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReleaseNotesStatus {
    Narrative,
    Degraded(ReleaseNotesDegradation),
    Missing,
    Legacy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseReceipt {
    pub target: String,
    pub version: String,
    pub tag: String,
    pub commit: String,
    pub workflow_run_id: u64,
    pub workflow_url: Option<String>,
    pub release_exists: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReleaseRunOutcome {
    NoChanges {
        target: String,
        latest_tag: Option<String>,
    },
    Released(ReleaseReceipt),
    Resumed(ReleaseReceipt),
}

#[derive(Debug)]
struct ReleaseWorkflowResult {
    database_id: u64,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhReleaseView {
    #[serde(rename = "isDraft")]
    is_draft: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GitHubReleaseState {
    Missing,
    Draft,
    Published,
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
    let (notes_status, workflow, release_exists) = match latest_tag.as_deref() {
        Some(tag) => (
            Some(release_notes_status(&main_repo, tag, &target)?),
            find_workflow_run(&main_repo, tag, &target)?,
            github_release_exists(&main_repo, tag)?,
        ),
        None => (None, None, false),
    };

    Ok(ReleaseStatusResult {
        target: target.name,
        latest_tag,
        notes_status,
        workflow_status: workflow.as_ref().map(|run| run.status.clone()),
        workflow_conclusion: workflow.as_ref().and_then(|run| run.conclusion.clone()),
        workflow_url: workflow.and_then(|run| run.url),
        release_exists,
    })
}

/// Check the exact git range since the last target tag.
pub fn release_check(repo: &Path, target_name: Option<&str>) -> OpsResult<ReleaseChangeSet> {
    if !command_exists("gh") {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    let (main_repo, target) = resolve_repo_and_target(repo, target_name)?;
    collect_release_changes(&main_repo, &target)
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

    progress.status("Collecting release changes...");
    let commits = release_commits_since(&main_repo, Some(&resolved_prev_tag), &target)?;
    let commit_shas = commits
        .iter()
        .map(|commit| commit.sha.as_str())
        .collect::<HashSet<_>>();
    let prs = merged_prs_since(&main_repo, Some(&resolved_prev_tag), &target, &commit_shas)?;

    progress.status("Generating narrative release notes...");
    run_release_notes_stage(
        &main_repo,
        &version,
        Some(&resolved_prev_tag),
        &commits,
        &prs,
        &target,
        progress,
    )?;

    let notes = fs::read_to_string(main_repo.join("RELEASE_NOTES.md"))?;
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

/// Stage assets on a draft GitHub Release or publish that draft as latest.
pub fn release_publish(
    repo: &Path,
    tag: &str,
    notes: Option<&Path>,
    assets: &[PathBuf],
    finalize: bool,
) -> OpsResult<()> {
    if !command_exists("gh") {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }
    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());

    if finalize {
        if github_release_state(&main_repo, tag)? == GitHubReleaseState::Missing {
            return Err(OpsError::Message(format!(
                "cannot publish {tag}: draft GitHub Release does not exist"
            )));
        }
        run_stdout(
            &main_repo,
            "gh",
            &["release", "edit", tag, "--draft=false", "--latest"],
        )?;
        return Ok(());
    }

    let notes = notes.ok_or_else(|| {
        OpsError::Message("release publish requires --notes before --finalize".to_string())
    })?;
    if !notes.is_file() {
        return Err(OpsError::Message(format!(
            "release notes not found: {}",
            notes.display()
        )));
    }
    for asset in assets {
        if !asset.is_file() {
            return Err(OpsError::Message(format!(
                "release asset not found: {}",
                asset.display()
            )));
        }
    }

    let notes_arg = notes.to_string_lossy().to_string();
    match github_release_state(&main_repo, tag)? {
        GitHubReleaseState::Missing => {
            let mut args = vec![
                "release".to_string(),
                "create".to_string(),
                tag.to_string(),
                "--draft".to_string(),
                "--verify-tag".to_string(),
                "--title".to_string(),
                tag.to_string(),
                "--notes-file".to_string(),
                notes_arg,
            ];
            args.extend(
                assets
                    .iter()
                    .map(|asset| asset.to_string_lossy().to_string()),
            );
            let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
            run_stdout(&main_repo, "gh", &refs)?;
        }
        GitHubReleaseState::Draft | GitHubReleaseState::Published => {
            run_stdout(
                &main_repo,
                "gh",
                &["release", "edit", tag, "--notes-file", &notes_arg],
            )?;
            if !assets.is_empty() {
                let mut args = vec![
                    "release".to_string(),
                    "upload".to_string(),
                    tag.to_string(),
                    "--clobber".to_string(),
                ];
                args.extend(
                    assets
                        .iter()
                        .map(|asset| asset.to_string_lossy().to_string()),
                );
                let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
                run_stdout(&main_repo, "gh", &refs)?;
            }
        }
    }
    Ok(())
}

/// Run the full release workflow in one shot.
///
/// Flow:
/// 1) check target-scoped commits since the previous tag
/// 2) bump manifests, run repo preparation, and generate notes in a worktree
/// 3) commit, open PR, and enqueue auto-merge
/// 4) wait for merge queue completion
/// 5) tag merged commit and push
/// 6) wait for the credential-free release build
/// 7) run the repo publisher, when configured
pub fn release_run(
    repo: &Path,
    version_input: &str,
    target_name: Option<&str>,
    progress: &impl Progress,
) -> OpsResult<ReleaseRunOutcome> {
    if !command_exists("gh") {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    let (main_repo, target) = resolve_repo_and_target(repo, target_name)?;

    let default_branch = get_default_branch(&main_repo)?;
    if !sync_main(&main_repo, &default_branch)? {
        return Err(OpsError::Message(format!(
            "could not synchronize {default_branch} with origin before release selection"
        )));
    }

    run_publisher_check(&main_repo, &target, progress)?;

    let latest_tag = latest_tag_optional(&main_repo, &target)?;
    let mut failed_latest_build = None;

    if let Some(tag) = latest_tag.as_deref() {
        if !target.publisher.is_empty()
            && github_release_state(&main_repo, tag)? != GitHubReleaseState::Published
        {
            let run = find_workflow_run(&main_repo, tag, &target)?.ok_or_else(|| {
                OpsError::Message(format!(
                    "latest release {tag} is incomplete and has no hosted build to resume"
                ))
            })?;
            let conclusion = run
                .conclusion
                .as_deref()
                .unwrap_or("unknown")
                .to_lowercase();
            if run.status == "completed" && conclusion != "success" {
                failed_latest_build = Some((conclusion, run.url));
            } else {
                progress.status(&format!("Resuming incomplete release {tag}..."));
                return resume_existing_release(&main_repo, tag, &target, progress);
            }
        } else if target.publisher.is_empty()
            && matches!(version_input.trim(), "patch" | "minor" | "major")
            && !release_completion_satisfied(&main_repo, tag, &target)?
        {
            progress.status(&format!("Resuming release completion for {tag}..."));
            return resume_existing_release(&main_repo, tag, &target, progress);
        }
    }

    if !matches!(version_input.trim(), "patch" | "minor" | "major") {
        let version = resolve_version(None, version_input, &target)?;
        let tag = target_tag(&target, &version);
        if remote_tag_sha(&main_repo, &tag)?.is_some() {
            progress.status(&format!("Resuming release completion for {tag}..."));
            return resume_existing_release(&main_repo, &tag, &target, progress);
        }
    }

    let changes = collect_release_changes(&main_repo, &target)?;
    if changes.commits.is_empty() {
        if let Some((conclusion, url)) = failed_latest_build {
            let url = url.unwrap_or_else(|| "workflow URL unavailable".to_string());
            let tag = latest_tag.as_deref().unwrap_or("latest tag");
            return Err(OpsError::Message(format!(
                "latest release build failed for {tag}: {conclusion} ({url}); no merged fix is available"
            )));
        }
        return Ok(ReleaseRunOutcome::NoChanges {
            target: target.name,
            latest_tag: changes.previous_tag,
        });
    }

    if let Some((conclusion, _)) = failed_latest_build.as_ref() {
        let tag = latest_tag.as_deref().unwrap_or("latest tag");
        progress.status(&format!(
            "Advancing past failed {tag} build ({conclusion}) with merged fixes..."
        ));
    }

    let version = resolve_version(changes.previous_tag.as_deref(), version_input, &target)?;

    if !target.verify.is_empty() {
        progress.status("Running repository release verification...");
        run_release_hooks(
            &main_repo,
            &target.verify,
            &target,
            Some(&version),
            changes.previous_tag.as_deref(),
            "verification",
        )?;
    }

    let new_tag = target_tag(&target, &version);
    if remote_tag_sha(&main_repo, &new_tag)?.is_some() {
        progress.status(&format!("Resuming release completion for {new_tag}..."));
        return resume_existing_release(&main_repo, &new_tag, &target, progress);
    }

    let wt_name = release_worktree_name(&target, &version);
    let branch = release_branch_name(&main_repo, &wt_name)?;
    let existing_pr = find_release_pr(&main_repo, &branch)?;
    let merged_commit = if let Some(pr) = existing_pr {
        match pr.state.as_str() {
            "MERGED" => pr.merge_commit.map(|commit| commit.oid).ok_or_else(|| {
                OpsError::Message(format!(
                    "release PR #{} is merged but its merge commit is unavailable",
                    pr.number
                ))
            })?,
            "OPEN" => {
                let head_sha = pr.head_ref_oid.as_deref().ok_or_else(|| {
                    OpsError::Message(format!(
                        "open release PR #{} has no observable head commit",
                        pr.number
                    ))
                })?;
                progress.status(&format!("Resuming release PR #{}...", pr.number));
                finish_release_pr(
                    &main_repo,
                    &wt_name,
                    &branch,
                    PreparedRelease {
                        pr_number: pr.number,
                        head_sha: head_sha.to_string(),
                    },
                    &target,
                    &version,
                    progress,
                )?
            }
            _ => {
                let url = pr.url.unwrap_or_else(|| format!("PR #{}", pr.number));
                return Err(OpsError::Message(format!(
                    "{url} was closed without merging; remove or rename the release branch before retrying"
                )));
            }
        }
    } else {
        let main_branch = get_default_branch(&main_repo)?;
        progress.status(&format!("Creating release worktree {wt_name}..."));
        let wt = create_named_worktree(&main_repo, &wt_name, Some(&main_branch), true)?;
        let wt_path = wt.path;
        let wt_branch = wt.branch;

        let prepared = prepare_release_in_worktree(
            &wt_path,
            &version,
            changes.previous_tag.as_deref(),
            &changes.commits,
            &changes.merged_prs,
            &target,
            progress,
        );
        cleanup_release_worktree(&main_repo, &wt_path, &wt_branch, None, progress);
        let prepared = prepared?;

        progress.status("Waiting for release PR to merge...");
        finish_release_pr(
            &main_repo, &wt_name, &branch, prepared, &target, &version, progress,
        )?
    };

    progress.status(&format!("Tagging {}...", target_tag(&target, &version)));
    let tag = tag_and_push_ref(&main_repo, &version, &target, Some(&merged_commit))?;

    progress.status(&format!("Waiting for release pipeline for {tag}..."));
    let workflow = wait_for_release_workflow(&main_repo, &tag, &target, progress)?;
    if !target.publisher.is_empty() {
        run_publisher(&main_repo, &tag, &target, &workflow, progress)?;
    }
    let release_exists = github_release_exists(&main_repo, &tag)?;

    Ok(ReleaseRunOutcome::Released(ReleaseReceipt {
        target: target.name,
        version,
        tag,
        commit: merged_commit,
        workflow_run_id: workflow.database_id,
        workflow_url: workflow.url,
        release_exists,
    }))
}

fn resume_existing_release(
    repo: &Path,
    tag: &str,
    target: &ReleaseTarget,
    progress: &impl Progress,
) -> OpsResult<ReleaseRunOutcome> {
    let workflow = wait_for_release_workflow(repo, tag, target, progress)?;
    if !target.publisher.is_empty() {
        run_publisher(repo, tag, target, &workflow, progress)?;
    }
    let commit = local_tag_sha(repo, tag)?
        .ok_or_else(|| OpsError::Message(format!("release tag {tag} is not present locally")))?;

    Ok(ReleaseRunOutcome::Resumed(ReleaseReceipt {
        target: target.name.clone(),
        version: version_from_tag(tag, target)?,
        tag: tag.to_string(),
        commit,
        workflow_run_id: workflow.database_id,
        workflow_url: workflow.url,
        release_exists: github_release_exists(repo, tag)?,
    }))
}

fn release_worktree_name(target: &ReleaseTarget, version: &str) -> String {
    let target_name = target.name.replace('.', "-");
    let version = version.replace('.', "-");
    format!("release-{target_name}-v{version}")
}

fn publish_worktree_name(target: &ReleaseTarget, tag: &str) -> String {
    let target_name = target.name.replace('.', "-");
    let tag = tag.replace(['/', '.'], "-");
    format!("publish-{target_name}-{tag}")
}

fn run_publisher_check(
    repo: &Path,
    target: &ReleaseTarget,
    progress: &impl Progress,
) -> OpsResult<()> {
    let publisher = expand_publisher_command(repo, &target.publisher);
    let Some((program, args)) = publisher.split_first() else {
        return Ok(());
    };
    progress.status("Checking release host...");
    let mut cmd = Command::new(program);
    cmd.args(args).arg("check").current_dir(repo);
    run_command(&mut cmd).map_err(|err| OpsError::CommandFailed {
        command: err.command_line(),
        stderr: err.stderr,
    })?;
    Ok(())
}

fn run_publisher(
    repo: &Path,
    tag: &str,
    target: &ReleaseTarget,
    workflow: &ReleaseWorkflowResult,
    progress: &impl Progress,
) -> OpsResult<()> {
    let publisher = expand_publisher_command(repo, &target.publisher);
    let Some((program, args)) = publisher.split_first() else {
        return Ok(());
    };
    if workflow.database_id == 0 {
        return Err(OpsError::Message(format!(
            "release workflow for {tag} did not expose a run id"
        )));
    }

    let wt_name = publish_worktree_name(target, tag);
    let wt_path = worktree_path(repo, &wt_name);
    let lease = acquire_worktree_lease(repo, &wt_path, &format!("release publisher for {tag}"))?;

    let artifact_dir = tempfile::tempdir()?;
    let run_id = workflow.database_id.to_string();
    progress.status(&format!("Downloading release artifacts for {tag}..."));
    run_stdout(
        repo,
        "gh",
        &[
            "run",
            "download",
            &run_id,
            "--dir",
            artifact_dir.path().to_string_lossy().as_ref(),
        ],
    )?;

    progress.status(&format!(
        "Materializing tagged publisher worktree {wt_name}..."
    ));
    let wt = create_named_worktree(repo, &wt_name, Some(tag), false)?;
    let publish_result = {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .arg("publish")
            .arg("--tag")
            .arg(tag)
            .arg("--artifacts")
            .arg(artifact_dir.path())
            .env("LF_RELEASE_MAIN_REPO", repo)
            .env("LF_RELEASE_SOURCE_REPO", &wt.path)
            .env(
                "LF_RELEASE_WORKFLOW_RUN_ID",
                workflow.database_id.to_string(),
            )
            .current_dir(&wt.path);
        run_command(&mut cmd).map_err(|err| OpsError::CommandFailed {
            command: err.command_line(),
            stderr: err.stderr,
        })
    };
    cleanup_release_worktree(repo, &wt.path, &wt.branch, Some(&lease), progress);
    publish_result?;

    if github_release_state(repo, tag)? != GitHubReleaseState::Published {
        return Err(OpsError::Message(format!(
            "publisher completed without publishing GitHub Release {tag}"
        )));
    }
    Ok(())
}

fn expand_publisher_command(repo: &Path, publisher: &[String]) -> Vec<String> {
    let repo = repo.to_string_lossy();
    publisher
        .iter()
        .map(|arg| arg.replace("{repo}", &repo))
        .collect()
}

#[derive(Debug)]
struct PreparedRelease {
    pr_number: u64,
    head_sha: String,
}

fn prepare_release_in_worktree(
    wt_path: &Path,
    version: &str,
    prev_tag: Option<&str>,
    commits: &[ReleaseCommit],
    merged_prs: &[MergedPr],
    target: &ReleaseTarget,
    progress: &impl Progress,
) -> OpsResult<PreparedRelease> {
    // Release preparation owns its branch independently of whichever Work
    // launched the controller. Provider/account authority remains available.
    let _context = ReleaseWorktreeContext::enter();
    progress.status(&format!(
        "Bumping manifests for {}...",
        target_tag(target, version)
    ));
    bump_manifest_versions(wt_path, target, version, progress)?;

    if !target.prepare.is_empty() {
        progress.status("Running repository release preparation...");
        run_release_hooks(
            wt_path,
            &target.prepare,
            target,
            Some(version),
            prev_tag,
            "preparation",
        )?;
    }

    progress.status(&format!(
        "Generating release notes for {}...",
        target_tag(target, version)
    ));
    run_release_notes_stage(
        wt_path, version, prev_tag, commits, merged_prs, target, progress,
    )?;

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

    progress.status("Enqueuing release PR for merge...");
    let pr_copy = release_pr_copy(wt_path, target, version)?;
    let options = LandOptions {
        strict: true,
        local: false,
        create_pr: false,
        complete: false,
        next_slug: None,
        worktree: None,
        commit_message: None,
        pr_title: Some(pr_copy.title),
        pr_body: Some(pr_copy.body),
        agent: None,
    };
    let pr = finish_land_after_rebase(wt_path, &options, progress)?.ok_or_else(|| {
        OpsError::Message("release land completed without a pull request".to_string())
    })?;
    let head_sha = pr.head_sha.ok_or_else(|| {
        OpsError::Message(format!(
            "release pull request #{} has no observable head commit",
            pr.number
        ))
    })?;

    Ok(PreparedRelease {
        pr_number: pr.number,
        head_sha,
    })
}

fn finish_release_pr(
    main_repo: &Path,
    worktree_name: &str,
    release_branch: &str,
    mut prepared: PreparedRelease,
    target: &ReleaseTarget,
    version: &str,
    progress: &impl Progress,
) -> OpsResult<String> {
    loop {
        match wait_for_pr_merge(main_repo, prepared.pr_number, &prepared.head_sha, progress)? {
            ReleasePrWait::Merged(commit) => return Ok(commit),
            ReleasePrWait::NeedsIntegration(state) => {
                progress.status(&format!(
                    "Release PR #{} is {state}; rebuilding on current main...",
                    prepared.pr_number
                ));
                fetch_release_branch(main_repo, release_branch, &prepared.head_sha)?;
                let wt = create_named_worktree(main_repo, worktree_name, None, true)?;
                let refreshed = rebuild_release_pr(
                    main_repo,
                    &wt.path,
                    &prepared.head_sha,
                    target,
                    version,
                    progress,
                );
                cleanup_release_worktree(main_repo, &wt.path, &wt.branch, None, progress);
                prepared = refreshed?;
            }
        }
    }
}

fn fetch_release_branch(repo: &Path, branch: &str, expected_head: &str) -> OpsResult<()> {
    let remote_ref = format!("refs/remotes/origin/{branch}");
    let refspec = format!("+refs/heads/{branch}:{remote_ref}");
    fetch(repo, "origin", &refspec)?;
    let remote_head = crate::engine::git::rev_parse(repo, &remote_ref)?;
    if remote_head != expected_head {
        return Err(OpsError::Message(format!(
            "release PR head changed while recovery was fetching it: expected {expected_head}, found {remote_head}"
        )));
    }
    Ok(())
}

fn rebuild_release_pr(
    main_repo: &Path,
    worktree: &Path,
    expected_head: &str,
    target: &ReleaseTarget,
    version: &str,
    progress: &impl Progress,
) -> OpsResult<PreparedRelease> {
    let current_head = crate::engine::git::rev_parse(worktree, "HEAD")?;
    if current_head != expected_head {
        return Err(OpsError::Message(format!(
            "release PR head changed while recovery was materializing it: expected {expected_head}, found {current_head}"
        )));
    }

    let main_branch = get_default_branch(main_repo)?;
    let main_ref = format!("origin/{main_branch}");
    run_stdout(worktree, "git", &["reset", "--hard", &main_ref])?;
    let changes = collect_release_changes(main_repo, target)?;
    prepare_release_in_worktree(
        worktree,
        version,
        changes.previous_tag.as_deref(),
        &changes.commits,
        &changes.merged_prs,
        target,
        progress,
    )
}

#[derive(Debug, Serialize)]
struct ReleaseNotesContext {
    version: String,
    prev_tag: Option<String>,
    target: String,
    tag_prefix: String,
    area_scope: Vec<String>,
    commits: Vec<ReleaseCommit>,
    merged_prs: Vec<MergedPr>,
    decisions: Option<String>,
    previous_release_notes: Option<String>,
    source_limits: ReleaseNotesSourceLimits,
    omissions: ReleaseNotesOmissions,
}

#[derive(Debug, Serialize)]
struct ReleaseNotesSourceLimits {
    context_bytes: usize,
    commits: usize,
    merged_prs: usize,
    files_per_change: usize,
    area_scopes: usize,
    name_bytes: usize,
    title_bytes: usize,
    path_bytes: usize,
    pr_body_bytes: usize,
    decisions_bytes: usize,
    previous_release_notes_bytes: usize,
    release_notes_bytes: usize,
}

#[derive(Debug, Serialize)]
struct ReleaseNotesOmissions {
    commits: usize,
    merged_prs: usize,
    commit_files: usize,
    pr_files: usize,
    area_scopes: usize,
    text_bytes: usize,
    decisions_bytes: usize,
    previous_release_notes_bytes: usize,
}

fn run_release_notes_stage(
    repo: &Path,
    version: &str,
    prev_tag: Option<&str>,
    commits: &[ReleaseCommit],
    merged_prs: &[MergedPr],
    target: &ReleaseTarget,
    progress: &impl Progress,
) -> OpsResult<()> {
    let notes_path = repo.join("RELEASE_NOTES.md");
    let (previous_notes, previous_notes_omitted) =
        read_bounded_text(&notes_path, RELEASE_CONTEXT_MAX_PREVIOUS_NOTES_BYTES)?;
    let previous_notes_file = move_release_notes_aside(repo, &notes_path)?;

    let result = (|| -> OpsResult<()> {
        promote_unreleased_dir(repo, version)?;
        let decisions_path = repo
            .join("release")
            .join(format!("v{version}"))
            .join("DECISIONS.md");
        let (decisions, decisions_omitted) =
            read_bounded_text(&decisions_path, RELEASE_CONTEXT_MAX_DECISIONS_BYTES)?;
        let (context, context_json) = build_release_notes_context(
            version,
            prev_tag,
            commits,
            merged_prs,
            target,
            decisions,
            decisions_omitted,
            previous_notes,
            previous_notes_omitted,
        )?;

        let mut context_file = tempfile::NamedTempFile::new_in(repo)?;
        context_file.write_all(&context_json)?;
        let context_path = context_file.path().to_string_lossy().to_string();

        let mut cmd = Command::new("lf");
        cmd.arg("--batch")
            .arg("release-notes")
            .current_dir(repo)
            .env("LF_RELEASE_NOTES_CONTEXT", &context_path);
        let degradation = match run_command(&mut cmd) {
            Ok(_) => None,
            Err(err) => match classify_release_notes_degradation(&err) {
                Some(degradation) => {
                    let notes = generate_release_notes(&context)?;
                    write_release_notes(repo, &notes, version)?;
                    Some(degradation)
                }
                None => {
                    return Err(OpsError::Message(format!(
                        "release gate blocked: release-notes agent failed outside the supported provider-degradation policy: {err}"
                    )));
                }
            },
        };

        finalize_release_notes(repo, version, degradation)?;
        archive_release_notes(repo, version)?;
        match degradation {
            None => progress.status("Release notes: narrative; release gate safe."),
            Some(reason) => progress.warning(&format!(
                "Release notes: degraded ({reason}); deterministic fallback keeps the release gate safe."
            )),
        }
        Ok(())
    })();

    if result.is_err() {
        restore_release_notes(&notes_path, previous_notes_file.as_ref())?;
    }
    result
}

fn move_release_notes_aside(
    repo: &Path,
    notes_path: &Path,
) -> OpsResult<Option<tempfile::TempPath>> {
    if !notes_path.exists() {
        return Ok(None);
    }
    let backup = tempfile::NamedTempFile::new_in(repo)?.into_temp_path();
    fs::remove_file(&backup)?;
    fs::rename(notes_path, &backup)?;
    Ok(Some(backup))
}

fn restore_release_notes(
    notes_path: &Path,
    previous_notes: Option<&tempfile::TempPath>,
) -> OpsResult<()> {
    match fs::remove_file(notes_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    if let Some(previous_notes) = previous_notes {
        fs::rename(previous_notes, notes_path)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_release_notes_context(
    version: &str,
    prev_tag: Option<&str>,
    commits: &[ReleaseCommit],
    merged_prs: &[MergedPr],
    target: &ReleaseTarget,
    decisions: Option<String>,
    decisions_omitted: usize,
    previous_release_notes: Option<String>,
    previous_release_notes_omitted: usize,
) -> OpsResult<(ReleaseNotesContext, Vec<u8>)> {
    let mut omissions = ReleaseNotesOmissions {
        commits: commits.len().saturating_sub(RELEASE_CONTEXT_MAX_COMMITS),
        merged_prs: merged_prs.len().saturating_sub(RELEASE_CONTEXT_MAX_PRS),
        commit_files: 0,
        pr_files: 0,
        area_scopes: target
            .area
            .len()
            .saturating_sub(RELEASE_CONTEXT_MAX_AREA_SCOPES),
        text_bytes: 0,
        decisions_bytes: decisions_omitted,
        previous_release_notes_bytes: previous_release_notes_omitted,
    };
    let mut context = ReleaseNotesContext {
        version: bound_text(
            version,
            RELEASE_CONTEXT_MAX_NAME_BYTES,
            &mut omissions.text_bytes,
        ),
        prev_tag: prev_tag.map(|tag| {
            bound_text(
                tag,
                RELEASE_CONTEXT_MAX_NAME_BYTES,
                &mut omissions.text_bytes,
            )
        }),
        target: bound_text(
            &target.name,
            RELEASE_CONTEXT_MAX_NAME_BYTES,
            &mut omissions.text_bytes,
        ),
        tag_prefix: bound_text(
            &target.tag_prefix,
            RELEASE_CONTEXT_MAX_NAME_BYTES,
            &mut omissions.text_bytes,
        ),
        area_scope: target
            .area
            .iter()
            .take(RELEASE_CONTEXT_MAX_AREA_SCOPES)
            .map(|scope| {
                bound_text(
                    scope,
                    RELEASE_CONTEXT_MAX_PATH_BYTES,
                    &mut omissions.text_bytes,
                )
            })
            .collect(),
        commits: commits
            .iter()
            .take(RELEASE_CONTEXT_MAX_COMMITS)
            .map(|commit| bound_release_commit(commit, &mut omissions))
            .collect(),
        merged_prs: merged_prs
            .iter()
            .take(RELEASE_CONTEXT_MAX_PRS)
            .map(|pr| bound_merged_pr(pr, &mut omissions))
            .collect(),
        decisions,
        previous_release_notes,
        source_limits: ReleaseNotesSourceLimits {
            context_bytes: RELEASE_CONTEXT_MAX_BYTES,
            commits: RELEASE_CONTEXT_MAX_COMMITS,
            merged_prs: RELEASE_CONTEXT_MAX_PRS,
            files_per_change: RELEASE_CONTEXT_MAX_FILES_PER_CHANGE,
            area_scopes: RELEASE_CONTEXT_MAX_AREA_SCOPES,
            name_bytes: RELEASE_CONTEXT_MAX_NAME_BYTES,
            title_bytes: RELEASE_CONTEXT_MAX_TITLE_BYTES,
            path_bytes: RELEASE_CONTEXT_MAX_PATH_BYTES,
            pr_body_bytes: RELEASE_CONTEXT_MAX_PR_BODY_BYTES,
            decisions_bytes: RELEASE_CONTEXT_MAX_DECISIONS_BYTES,
            previous_release_notes_bytes: RELEASE_CONTEXT_MAX_PREVIOUS_NOTES_BYTES,
            release_notes_bytes: RELEASE_NOTES_MAX_BYTES,
        },
        omissions,
    };

    loop {
        let json = serde_json::to_vec(&context).map_err(|err| {
            OpsError::Parse(format!("failed to encode release-notes context: {err}"))
        })?;
        if json.len() <= RELEASE_CONTEXT_MAX_BYTES {
            return Ok((context, json));
        }
        if context.merged_prs.len() >= context.commits.len() && !context.merged_prs.is_empty() {
            context.merged_prs.pop();
            context.omissions.merged_prs += 1;
        } else if !context.commits.is_empty() {
            context.commits.pop();
            context.omissions.commits += 1;
        } else if !context.merged_prs.is_empty() {
            context.merged_prs.pop();
            context.omissions.merged_prs += 1;
        } else {
            return Err(OpsError::Message(format!(
                "release gate blocked: bounded release-notes metadata exceeds {RELEASE_CONTEXT_MAX_BYTES} bytes"
            )));
        }
    }
}

fn bound_release_commit(
    commit: &ReleaseCommit,
    omissions: &mut ReleaseNotesOmissions,
) -> ReleaseCommit {
    omissions.commit_files += commit
        .files
        .len()
        .saturating_sub(RELEASE_CONTEXT_MAX_FILES_PER_CHANGE);
    ReleaseCommit {
        sha: bound_text(
            &commit.sha,
            RELEASE_CONTEXT_MAX_NAME_BYTES,
            &mut omissions.text_bytes,
        ),
        title: bound_text(
            &commit.title,
            RELEASE_CONTEXT_MAX_TITLE_BYTES,
            &mut omissions.text_bytes,
        ),
        files: commit
            .files
            .iter()
            .take(RELEASE_CONTEXT_MAX_FILES_PER_CHANGE)
            .map(|path| {
                bound_text(
                    path,
                    RELEASE_CONTEXT_MAX_PATH_BYTES,
                    &mut omissions.text_bytes,
                )
            })
            .collect(),
    }
}

fn bound_merged_pr(pr: &MergedPr, omissions: &mut ReleaseNotesOmissions) -> MergedPr {
    omissions.pr_files += pr
        .files
        .len()
        .saturating_sub(RELEASE_CONTEXT_MAX_FILES_PER_CHANGE);
    MergedPr {
        number: pr.number,
        title: bound_text(
            &pr.title,
            RELEASE_CONTEXT_MAX_TITLE_BYTES,
            &mut omissions.text_bytes,
        ),
        body: pr.body.as_deref().map(|body| {
            bound_text(
                body,
                RELEASE_CONTEXT_MAX_PR_BODY_BYTES,
                &mut omissions.text_bytes,
            )
        }),
        files: pr
            .files
            .iter()
            .take(RELEASE_CONTEXT_MAX_FILES_PER_CHANGE)
            .map(|path| {
                bound_text(
                    path,
                    RELEASE_CONTEXT_MAX_PATH_BYTES,
                    &mut omissions.text_bytes,
                )
            })
            .collect(),
        additions: pr.additions,
        deletions: pr.deletions,
        changed_files: pr.changed_files,
        merge_commit: pr.merge_commit.as_deref().map(|commit| {
            bound_text(
                commit,
                RELEASE_CONTEXT_MAX_NAME_BYTES,
                &mut omissions.text_bytes,
            )
        }),
    }
}

fn bound_text(value: &str, max_bytes: usize, omitted: &mut usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    *omitted += value.len() - end;
    value[..end].to_string()
}

fn read_bounded_text(path: &Path, max_bytes: usize) -> OpsResult<(Option<String>, usize)> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((None, 0)),
        Err(error) => return Err(error.into()),
    };
    let total_bytes = file.metadata()?.len().min(usize::MAX as u64) as usize;
    let mut bytes = Vec::with_capacity(total_bytes.min(max_bytes));
    file.take(max_bytes as u64).read_to_end(&mut bytes)?;
    let mut omitted = total_bytes.saturating_sub(bytes.len());
    let decoded = String::from_utf8_lossy(&bytes);
    let value = bound_text(&decoded, max_bytes, &mut omitted);
    Ok((Some(value), omitted))
}

fn classify_release_notes_degradation(err: &CommandError) -> Option<ReleaseNotesDegradation> {
    let stderr = err.stderr.to_lowercase();
    let message = err.message.to_lowercase();
    let combined = format!("{stderr}\n{message}");
    let mentions_agent_cli = combined.contains("claude")
        || combined.contains("codex")
        || combined.contains("opencode")
        || combined.contains("agent");
    let missing_binary = combined.contains("cli not found")
        || combined.contains("binary not found")
        || combined.contains("not found on path")
        || combined.contains("no such file or directory")
        || combined.contains("failed to spawn");
    if mentions_agent_cli && missing_binary {
        return Some(ReleaseNotesDegradation::MissingCli);
    }
    if combined.contains("account subscription limit")
        || combined.contains("usage limit reached")
        || combined.contains("usage limit has been reached")
        || combined.contains("you've hit your usage limit")
        || combined.contains("subscription quota")
        || combined.contains("quota exceeded")
        || combined.contains("insufficient_quota")
    {
        return Some(ReleaseNotesDegradation::Quota);
    }
    if combined.contains("account credential invalidated")
        || combined.contains("no authenticated")
        || combined.contains("not authenticated")
        || combined.contains("not logged in")
        || combined.contains("please sign in")
        || combined.contains("log in again")
        || combined.contains("login required")
        || combined.contains("unauthorized")
        || combined.contains("status 401")
    {
        return Some(ReleaseNotesDegradation::Authentication);
    }
    if combined.contains("no eligible managed")
        || combined.contains("cooling down")
        || combined.contains("cooldown")
    {
        return Some(ReleaseNotesDegradation::Cooldown);
    }
    if combined.contains("provider rate limit")
        || combined.contains("too many requests")
        || combined.contains("status 429")
        || combined.contains("status code 429")
    {
        return Some(ReleaseNotesDegradation::RateLimit);
    }
    if combined.contains("provider capacity")
        || combined.contains("provider unavailable")
        || combined.contains("provider transport")
        || combined.contains("temporarily unavailable")
        || combined.contains("service unavailable")
        || combined.contains("server is busy")
        || combined.contains("server overloaded")
        || combined.contains("try again later")
        || combined.contains("internal server error")
        || combined.contains("status 502")
        || combined.contains("status 503")
        || combined.contains("status 504")
        || combined.contains("connection reset")
        || combined.contains("connection closed")
        || combined.contains("connection refused")
        || combined.contains("network error")
        || combined.contains("request timed out")
        || combined.contains("request timeout")
    {
        return Some(ReleaseNotesDegradation::ProviderUnavailable);
    }
    None
}

fn finalize_release_notes(
    repo: &Path,
    version: &str,
    degradation: Option<ReleaseNotesDegradation>,
) -> OpsResult<()> {
    let path = repo.join("RELEASE_NOTES.md");
    let file = fs::File::open(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            OpsError::Message(
                "release gate blocked: release-notes skill did not write fresh RELEASE_NOTES.md"
                    .to_string(),
            )
        } else {
            error.into()
        }
    })?;
    let reported_bytes = file.metadata()?.len();
    let mut raw =
        String::with_capacity(reported_bytes.min((RELEASE_NOTES_MAX_BYTES + 1) as u64) as usize);
    file.take((RELEASE_NOTES_MAX_BYTES + 1) as u64)
        .read_to_string(&mut raw)?;
    if reported_bytes > RELEASE_NOTES_MAX_BYTES as u64 || raw.len() > RELEASE_NOTES_MAX_BYTES {
        return Err(OpsError::Message(format!(
            "release gate blocked: release notes are at least {} bytes; maximum queue metadata is {RELEASE_NOTES_MAX_BYTES} bytes",
            reported_bytes.max(raw.len() as u64)
        )));
    }
    let expected_header = format!("# v{version}");
    let mut lines = raw.trim().lines();
    let actual_header = lines.next().map(str::trim);
    if actual_header != Some(expected_header.as_str()) {
        return Err(OpsError::Message(format!(
            "release gate blocked: release notes must start with '{expected_header}'"
        )));
    }

    let body = lines
        .filter(|line| !line.trim().starts_with(RELEASE_NOTES_STATUS_PREFIX))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if body.is_empty() {
        return Err(OpsError::Message(
            "release gate blocked: release notes contain no release content".to_string(),
        ));
    }
    let marker = match degradation {
        None => "<!-- loopflow:release-notes=narrative;gate=safe -->".to_string(),
        Some(reason) => format!(
            "<!-- loopflow:release-notes=degraded;reason={};gate=safe -->",
            reason.slug()
        ),
    };
    let content = format!("{expected_header}\n\n{marker}\n\n{body}");
    if content.len() > RELEASE_NOTES_MAX_BYTES {
        return Err(OpsError::Message(format!(
            "release gate blocked: release notes are {} bytes; maximum queue metadata is {RELEASE_NOTES_MAX_BYTES} bytes",
            content.len()
        )));
    }
    write_atomic(&path, content.as_bytes())
}

fn write_atomic(path: &Path, content: &[u8]) -> OpsResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| OpsError::Message(format!("path has no parent: {}", path.display())))?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(content)?;
    file.persist(path)
        .map_err(|error| OpsError::Io(error.error))?;
    Ok(())
}

/// Rename `release/unreleased/` to `release/v<version>/` at tag time.
///
/// Idempotent: if `release/unreleased/` doesn't exist, does nothing.
/// If `release/v<version>/` already exists (e.g. from a retried release
/// or a backfilled NOTES.md), merges unreleased contents into it instead
/// of failing.
fn promote_unreleased_dir(repo: &Path, version: &str) -> OpsResult<()> {
    let unreleased = repo.join("release").join("unreleased");
    if !unreleased.exists() {
        return Ok(());
    }

    let target = repo.join("release").join(format!("v{version}"));
    if !target.exists() {
        fs::rename(&unreleased, &target)?;
        return Ok(());
    }

    // Target already exists — move each entry from unreleased into target.
    for entry in fs::read_dir(&unreleased)? {
        let entry = entry?;
        let dest = target.join(entry.file_name());
        if dest.exists() {
            return Err(OpsError::Message(format!(
                "cannot promote {}: {} already exists",
                entry.path().display(),
                dest.display()
            )));
        }
        fs::rename(entry.path(), dest)?;
    }
    fs::remove_dir(&unreleased)?;
    Ok(())
}

/// Copy `RELEASE_NOTES.md` into `release/v<version>/NOTES.md` for archival.
fn archive_release_notes(repo: &Path, version: &str) -> OpsResult<()> {
    let src = repo.join("RELEASE_NOTES.md");
    let dir = repo.join("release").join(format!("v{version}"));
    let notes = fs::read(&src)?;
    write_atomic(&dir.join("NOTES.md"), &notes)
}

fn release_notes_status(
    repo: &Path,
    tag: &str,
    target: &ReleaseTarget,
) -> OpsResult<ReleaseNotesStatus> {
    let version = version_from_tag(tag, target)?;
    let archive = format!("release/v{version}/NOTES.md");
    let reference = format!("{tag}:{archive}");
    let output = run_output(repo, "git", &["show", &reference])?;
    if !output.status.success() {
        return Ok(ReleaseNotesStatus::Missing);
    }
    let notes = String::from_utf8_lossy(&output.stdout);
    Ok(parse_release_notes_status(&notes))
}

fn parse_release_notes_status(notes: &str) -> ReleaseNotesStatus {
    let marker = notes
        .lines()
        .find_map(|line| line.trim().strip_prefix(RELEASE_NOTES_STATUS_PREFIX));
    let Some(marker) = marker else {
        return ReleaseNotesStatus::Legacy;
    };
    if marker == "narrative;gate=safe -->" {
        return ReleaseNotesStatus::Narrative;
    }
    let reason = marker
        .strip_prefix("degraded;reason=")
        .and_then(|value| value.strip_suffix(";gate=safe -->"))
        .and_then(ReleaseNotesDegradation::from_slug);
    if let Some(reason) = reason {
        return ReleaseNotesStatus::Degraded(reason);
    }
    ReleaseNotesStatus::Legacy
}

fn cleanup_release_worktree(
    main_repo: &Path,
    wt_path: &Path,
    branch: &str,
    lease: Option<&WorktreeLease>,
    progress: &impl Progress,
) {
    let result = match lease {
        Some(lease) => worktree_remove_owned(main_repo, wt_path, lease),
        None => worktree_remove(main_repo, wt_path),
    };
    if let Err(err) = result {
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

fn release_pr_copy(repo: &Path, target: &ReleaseTarget, version: &str) -> OpsResult<PrCopy> {
    Ok(PrCopy {
        title: release_commit_message(target, version),
        body: fs::read_to_string(repo.join("RELEASE_NOTES.md"))?,
    })
}

fn release_branch_name(repo: &Path, worktree_name: &str) -> OpsResult<String> {
    let author = git_user(repo)?;
    Ok(format!("{author}/{}", sanitize_for_branch(worktree_name)))
}

fn find_release_pr(repo: &Path, branch: &str) -> OpsResult<Option<GhReleasePr>> {
    let output = run_stdout(
        repo,
        "gh",
        &[
            "pr",
            "list",
            "--head",
            branch,
            "--state",
            "all",
            "--json",
            "number,state,mergeCommit,url,headRefOid",
            "--limit",
            "1",
        ],
    )?;
    let mut prs: Vec<GhReleasePr> = serde_json::from_str(&output)
        .map_err(|err| OpsError::Parse(format!("failed to parse release PR: {err}")))?;
    Ok(prs.pop())
}

enum ReleasePrWait {
    Merged(String),
    NeedsIntegration(String),
}

fn wait_for_pr_merge(
    repo: &Path,
    pr_number: u64,
    head_sha: &str,
    progress: &impl Progress,
) -> OpsResult<ReleasePrWait> {
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
                "state,mergeStateStatus,mergeCommit,url",
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
                return Ok(ReleasePrWait::Merged(commit.oid));
            }
            "CLOSED" => {
                let url = view.url.unwrap_or_else(|| format!("PR #{pr_number}"));
                return Err(OpsError::Message(format!(
                    "{url} was closed without merging"
                )));
            }
            _ => {}
        }

        if matches!(view.merge_state_status.as_str(), "BEHIND" | "DIRTY") {
            return Ok(ReleasePrWait::NeedsIntegration(
                view.merge_state_status.to_ascii_lowercase(),
            ));
        }

        if started.elapsed() >= timeout {
            return Err(OpsError::Message(format!(
                "timed out waiting for PR #{pr_number} to merge"
            )));
        }

        if !crate::ops::pr::auto_merge_enabled(repo, pr_number)? {
            progress.status(&format!(
                "Re-arming release PR #{pr_number} for exact-head auto-merge..."
            ));
            crate::ops::pr::enable_auto_merge(repo, pr_number, None, None, head_sha)?;
        }

        if attempt.is_multiple_of(6) {
            progress.status(&format!(
                "PR #{pr_number} is open ({}) and awaiting GitHub auto-merge...",
                view.merge_state_status.to_ascii_lowercase()
            ));
        }
        attempt += 1;
        thread::sleep(poll);
    }
}

fn wait_for_release_workflow(
    repo: &Path,
    tag: &str,
    target: &ReleaseTarget,
    progress: &impl Progress,
) -> OpsResult<ReleaseWorkflowResult> {
    if target.publisher.is_empty() && target.completion == ReleaseCompletion::Tag {
        return Ok(ReleaseWorkflowResult {
            database_id: 0,
            url: None,
        });
    }

    let started = Instant::now();
    let timeout = Duration::from_secs(60 * 60);
    let poll = Duration::from_secs(10);
    let no_workflow_grace = Duration::from_secs(90);
    let mut attempt: u64 = 0;
    let mut saw_workflow = false;

    loop {
        let workflow = find_workflow_run(repo, tag, target)?;
        let release_exists = github_release_exists(repo, tag)?;

        if target.publisher.is_empty()
            && target.completion == ReleaseCompletion::GithubRelease
            && release_exists
        {
            return Ok(ReleaseWorkflowResult {
                database_id: workflow.as_ref().map_or(0, |run| run.database_id),
                url: workflow.and_then(|run| run.url),
            });
        }

        if let Some(run) = workflow {
            saw_workflow = true;
            if run.status == "completed" {
                let conclusion = run
                    .conclusion
                    .unwrap_or_else(|| "unknown".to_string())
                    .to_lowercase();
                if conclusion == "success" {
                    if !target.publisher.is_empty()
                        || target.completion == ReleaseCompletion::Workflow
                    {
                        return Ok(ReleaseWorkflowResult {
                            database_id: run.database_id,
                            url: run.url,
                        });
                    }
                } else {
                    let url = run
                        .url
                        .unwrap_or_else(|| "(workflow URL unavailable)".to_string());
                    return Err(OpsError::Message(format!(
                        "release workflow failed for {tag}: {conclusion} ({url})"
                    )));
                }
            }
        } else if !saw_workflow
            && target.workflow.is_none()
            && started.elapsed() >= no_workflow_grace
        {
            progress.status(&format!(
                "No release workflow detected for {tag}; tag push complete"
            ));
            return Ok(ReleaseWorkflowResult {
                database_id: 0,
                url: None,
            });
        }

        if started.elapsed() >= timeout {
            return Err(OpsError::Message(format!(
                "timed out waiting for {:?} completion for {tag}",
                target.completion
            )));
        }

        if attempt.is_multiple_of(6) {
            progress.status(&format!("Release workflow for {tag} still running..."));
        }
        attempt += 1;
        thread::sleep(poll);
    }
}

fn release_completion_satisfied(repo: &Path, tag: &str, target: &ReleaseTarget) -> OpsResult<bool> {
    if target.completion == ReleaseCompletion::Tag {
        return Ok(true);
    }
    if target.completion == ReleaseCompletion::GithubRelease && github_release_exists(repo, tag)? {
        return Ok(true);
    }

    let Some(run) = find_workflow_run(repo, tag, target)? else {
        return Ok(false);
    };
    if run.status != "completed" {
        return Ok(false);
    }
    let conclusion = run
        .conclusion
        .as_deref()
        .unwrap_or("unknown")
        .to_lowercase();
    if conclusion != "success" {
        let url = run
            .url
            .unwrap_or_else(|| "(workflow URL unavailable)".to_string());
        return Err(OpsError::Message(format!(
            "release workflow failed for {tag}: {conclusion} ({url})"
        )));
    }

    Ok(target.completion == ReleaseCompletion::Workflow)
}

fn generate_release_with_target(
    repo: &Path,
    version_input: &str,
    target: &ReleaseTarget,
    progress: &impl Progress,
) -> OpsResult<String> {
    progress.status("Finding latest tag...");
    let prev_tag = latest_tag_optional(repo, target)?;

    let version = resolve_version(prev_tag.as_deref(), version_input, target)?;

    progress.status("Collecting release changes...");
    let commits = release_commits_since(repo, prev_tag.as_deref(), target)?;
    let commit_shas = commits
        .iter()
        .map(|commit| commit.sha.as_str())
        .collect::<HashSet<_>>();
    let prs = merged_prs_since(repo, prev_tag.as_deref(), target, &commit_shas)?;

    progress.status("Generating release notes...");
    let (context, _) = build_release_notes_context(
        &version,
        prev_tag.as_deref(),
        &commits,
        &prs,
        target,
        None,
        0,
        None,
        0,
    )?;
    let notes = generate_release_notes(&context)?;

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

    let completion = config.completion.unwrap_or_else(|| {
        if config.workflow.is_some() {
            ReleaseCompletion::Workflow
        } else {
            ReleaseCompletion::Tag
        }
    });

    ReleaseTarget {
        name: name.to_string(),
        area: config.area.clone(),
        tag_prefix: config.tag_prefix.clone(),
        manifests,
        workflow: config.workflow.clone(),
        verify: config.verify.clone(),
        prepare: config.prepare.clone(),
        completion,
        publisher: config.publisher.clone(),
    }
}

fn default_release_target(repo: &Path) -> ReleaseTarget {
    ReleaseTarget {
        name: "default".to_string(),
        area: Vec::new(),
        tag_prefix: String::new(),
        manifests: detect_manifests(repo, &[]),
        workflow: None,
        verify: Vec::new(),
        prepare: Vec::new(),
        completion: ReleaseCompletion::Tag,
        publisher: Vec::new(),
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

fn resolve_version(
    prev_tag: Option<&str>,
    input: &str,
    target: &ReleaseTarget,
) -> OpsResult<String> {
    match input.trim() {
        "patch" | "minor" | "major" => {
            let prev_tag = prev_tag.ok_or_else(|| {
                OpsError::Message(
                    "no previous tag; pass an explicit X.Y.Z version for the first release"
                        .to_string(),
                )
            })?;
            let prev_version = version_from_tag(prev_tag, target)?;
            bump_version(&prev_version, input.trim())
        }
        other => {
            let version = normalize_version(other);
            let _ = bump_version(&version, "patch")?;
            Ok(version)
        }
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

fn latest_tag(repo: &Path, target: &ReleaseTarget) -> OpsResult<String> {
    latest_tag_optional(repo, target)?.ok_or_else(|| {
        OpsError::Message(format!(
            "no previous tag found matching {}; create a tag before running release",
            tag_glob(target)
        ))
    })
}

fn latest_tag_optional(repo: &Path, target: &ReleaseTarget) -> OpsResult<Option<String>> {
    // Release tags on origin are authoritative. Long-lived release hosts may
    // retain an old object for a tag that was repaired remotely; force-update
    // this target's remote version tags while preserving local-only tags from
    // an interrupted push.
    let pattern = tag_glob(target);
    let refspec = format!("+refs/tags/{pattern}:refs/tags/{pattern}");
    let fetch = run_output(repo, "git", &["fetch", "origin", "--quiet", &refspec])?;
    if !fetch.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("git fetch origin --quiet {refspec}"),
            stderr: String::from_utf8_lossy(&fetch.stderr).to_string(),
        });
    }

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

fn collect_release_changes(repo: &Path, target: &ReleaseTarget) -> OpsResult<ReleaseChangeSet> {
    let previous_tag = latest_tag_optional(repo, target)?;
    let commits = release_commits_since(repo, previous_tag.as_deref(), target)?;
    let commit_shas = commits
        .iter()
        .map(|commit| commit.sha.as_str())
        .collect::<HashSet<_>>();
    let merged_prs = merged_prs_since(repo, previous_tag.as_deref(), target, &commit_shas)?;

    Ok(ReleaseChangeSet {
        previous_tag,
        commits,
        merged_prs,
    })
}

fn release_commits_since(
    repo: &Path,
    previous_tag: Option<&str>,
    target: &ReleaseTarget,
) -> OpsResult<Vec<ReleaseCommit>> {
    let range = previous_tag
        .map(|tag| format!("{tag}..HEAD"))
        .unwrap_or_else(|| "HEAD".to_string());
    let log = run_stdout(
        repo,
        "git",
        &[
            "log",
            "--first-parent",
            "--reverse",
            "--format=%H%x1f%s",
            &range,
        ],
    )?;
    let mut commits = Vec::new();

    for line in log.lines().filter(|line| !line.trim().is_empty()) {
        let Some((sha, title)) = line.split_once('\u{1f}') else {
            return Err(OpsError::Parse(format!(
                "failed to parse release commit: {line}"
            )));
        };
        let files = run_stdout(
            repo,
            "git",
            &[
                "diff-tree",
                "--root",
                "-m",
                "--first-parent",
                "--no-commit-id",
                "--name-only",
                "-r",
                sha,
            ],
        )?
        .lines()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

        if target.area.is_empty()
            || files
                .iter()
                .any(|path| area_matches(path, target.area.as_slice()))
        {
            commits.push(ReleaseCommit {
                sha: sha.to_string(),
                title: title.to_string(),
                files,
            });
        }
    }

    Ok(commits)
}

fn merged_prs_since(
    repo: &Path,
    previous_tag: Option<&str>,
    target: &ReleaseTarget,
    commit_shas: &HashSet<&str>,
) -> OpsResult<Vec<MergedPr>> {
    let search = match previous_tag {
        Some(tag) => {
            let tagged_at = run_stdout(repo, "git", &["log", "-1", "--format=%aI", tag])?;
            if tagged_at.is_empty() {
                return Err(OpsError::Message(format!(
                    "could not determine merge date for tag {tag}"
                )));
            }
            let date = tagged_at.split('T').next().unwrap_or(&tagged_at);
            Some(format!("merged:>={date}"))
        }
        None => None,
    };

    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let base_branch = get_default_branch(&main_repo)?;

    let mut prs = if target.area.is_empty() {
        list_merged_prs(repo, &base_branch, search.as_deref(), false)?
    } else {
        match list_merged_prs(repo, &base_branch, search.as_deref(), true) {
            Ok(prs) => prs,
            Err(err) if should_fallback_for_pr_files(&err) => {
                let mut fallback_prs =
                    list_merged_prs(repo, &base_branch, search.as_deref(), false)?;
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
    prs.retain(|pr| {
        pr.merge_commit
            .as_deref()
            .is_some_and(|sha| commit_shas.contains(sha))
    });

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
    search: Option<&str>,
    include_files: bool,
) -> OpsResult<Vec<MergedPr>> {
    let fields = if include_files {
        "number,title,body,files,additions,deletions,changedFiles,mergeCommit"
    } else {
        "number,title,body,additions,deletions,changedFiles,mergeCommit"
    };

    let mut args = vec!["pr", "list", "--state", "merged", "--base", base_branch];
    if let Some(search) = search {
        args.extend(["--search", search]);
    }
    let limit = RELEASE_QUEUE_PR_LIMIT.to_string();
    args.extend(["--json", fields, "--limit", &limit]);
    let output = run_stdout(repo, "gh", &args)?;

    let prs: Vec<GhMergedPr> = serde_json::from_str(&output)
        .map_err(|err| OpsError::Parse(format!("failed to parse merged PR list: {err}")))?;

    Ok(prs
        .into_iter()
        .take(RELEASE_QUEUE_PR_LIMIT)
        .map(Into::into)
        .collect())
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

fn generate_release_notes(context: &ReleaseNotesContext) -> OpsResult<String> {
    let previous = context.prev_tag.as_deref().unwrap_or("repository start");
    let tag_prefix = if context.tag_prefix.is_empty() {
        "v".to_string()
    } else {
        format!("{}v", context.tag_prefix)
    };
    let area_scope = if context.area_scope.is_empty() {
        "all files".to_string()
    } else {
        context.area_scope.join(", ")
    };
    let mut lines = vec![
        format!("## Changes since {previous}"),
        String::new(),
        format!("- Target: {}", context.target),
        format!("- Tag prefix: {tag_prefix}"),
        format!("- Area scope: {area_scope}"),
        String::new(),
    ];

    lines.push("## Commits".to_string());
    lines.push(String::new());

    for commit in context.commits.iter().take(FALLBACK_NOTES_MAX_COMMITS) {
        let short_sha = commit.sha.get(..7).unwrap_or(&commit.sha);
        lines.push(format!("- `{short_sha}` {}", commit.title));
    }
    let omitted_commits = context.omissions.commits
        + context
            .commits
            .len()
            .saturating_sub(FALLBACK_NOTES_MAX_COMMITS);
    if omitted_commits > 0 {
        lines.push(format!(
            "- … {omitted_commits} more commit(s) in the release range."
        ));
    }

    lines.push(String::new());
    lines.push("## Merged PRs".to_string());
    lines.push(String::new());

    if context.merged_prs.is_empty() {
        lines.push("- No merged PRs found in this window.".to_string());
    } else {
        for pr in context.merged_prs.iter().take(FALLBACK_NOTES_MAX_PRS) {
            lines.push(format!(
                "- #{} {} (+{} -{}, {} files)",
                pr.number, pr.title, pr.additions, pr.deletions, pr.changed_files
            ));
        }
        let omitted_prs = context.omissions.merged_prs
            + context
                .merged_prs
                .len()
                .saturating_sub(FALLBACK_NOTES_MAX_PRS);
        if omitted_prs > 0 {
            lines.push(format!(
                "- … {omitted_prs} more merged PR(s) in the release range."
            ));
        }
    }

    lines.push(String::new());
    if context.omissions.text_bytes > 0
        || context.omissions.commit_files > 0
        || context.omissions.pr_files > 0
        || context.omissions.decisions_bytes > 0
        || context.omissions.previous_release_notes_bytes > 0
        || context.omissions.area_scopes > 0
    {
        lines.push(
            "_Some narrative source was omitted to keep release context bounded._".to_string(),
        );
        lines.push(String::new());
    }
    lines.push(format!(
        "_Generated mechanically for v{}._",
        context.version
    ));
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
        run_stdout(repo, "cargo", &["update", "--workspace"])?;
    }
    if bumped_pyproject && repo.join("uv.lock").exists() {
        run_stdout(repo, "uv", &["lock"])?;
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
    ensure_commit_local(repo, &target_sha)?;

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

/// Ensure `sha` exists in `repo`'s local object database, fetching from
/// origin if it doesn't.
///
/// `git rev-parse <40-char-hex>` echoes the input without verifying the
/// object is actually present. When the SHA originates from `gh pr view`
/// (a merge commit landed on origin via the merge queue), the local repo
/// may not have fetched it yet — and `git tag <name> <sha>` then fails
/// with `trying to write ref ... with nonexistent object`.
fn ensure_commit_local(repo: &Path, sha: &str) -> OpsResult<()> {
    if commit_exists_locally(repo, sha) {
        return Ok(());
    }
    let _ = run_output(repo, "git", &["fetch", "origin"]);
    if commit_exists_locally(repo, sha) {
        return Ok(());
    }
    Err(OpsError::Message(format!(
        "commit {sha} is not present locally even after fetching origin"
    )))
}

fn commit_exists_locally(repo: &Path, sha: &str) -> bool {
    run_output(repo, "git", &["cat-file", "-e", sha])
        .map(|output| output.status.success())
        .unwrap_or(false)
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

    let matching = runs
        .iter()
        .find(|run| run.head_branch.as_deref() == Some(tag))
        .cloned()
        .or_else(|| {
            runs.into_iter().find(|run| {
                run.display_title
                    .as_deref()
                    .is_some_and(|title| title.contains(tag))
            })
        });

    Ok(matching)
}

fn github_release_exists(repo: &Path, tag: &str) -> OpsResult<bool> {
    Ok(github_release_state(repo, tag)? == GitHubReleaseState::Published)
}

fn github_release_state(repo: &Path, tag: &str) -> OpsResult<GitHubReleaseState> {
    let output = run_output(repo, "gh", &["release", "view", tag, "--json", "isDraft"])?;
    if !output.status.success() {
        return Ok(GitHubReleaseState::Missing);
    }
    let view: GhReleaseView = serde_json::from_slice(&output.stdout)
        .map_err(|err| OpsError::Parse(format!("failed to parse GitHub Release state: {err}")))?;
    Ok(if view.is_draft {
        GitHubReleaseState::Draft
    } else {
        GitHubReleaseState::Published
    })
}

fn run_release_hooks(
    repo: &Path,
    hooks: &[String],
    target: &ReleaseTarget,
    version: Option<&str>,
    previous_tag: Option<&str>,
    phase: &str,
) -> OpsResult<()> {
    for hook in hooks {
        let command = hook
            .replace("{target}", &target.name)
            .replace("{version}", version.unwrap_or(""))
            .replace("{previous_tag}", previous_tag.unwrap_or(""));
        let mut cmd = Command::new("sh");
        cmd.args(["-c", &command]).current_dir(repo);
        run_command(&mut cmd).map_err(|err| OpsError::CommandFailed {
            command: format!("release {phase}: {command}"),
            stderr: err.stderr,
        })?;
    }
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

    #[test]
    fn release_worktree_names_are_flat() {
        let target = ReleaseTarget {
            name: "default".to_string(),
            area: Vec::new(),
            tag_prefix: "v".to_string(),
            manifests: Vec::new(),
            workflow: None,
            verify: Vec::new(),
            prepare: Vec::new(),
            completion: ReleaseCompletion::Tag,
            publisher: Vec::new(),
        };

        assert_eq!(
            release_worktree_name(&target, "0.11.0"),
            "release-default-v0-11-0"
        );

        let named = ReleaseTarget {
            name: "rust.cli".to_string(),
            ..target
        };
        assert_eq!(
            release_worktree_name(&named, "1.2.3-beta.1"),
            "release-rust-cli-v1-2-3-beta-1"
        );
    }

    // ======================================================================
    // Repo-owned preparation (the release cut is the publication authority)
    // ======================================================================

    fn python3_available() -> bool {
        Command::new("python3")
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    /// The release worktree — not a manual script invocation — turns drafts into
    /// canonical migrations. This drives the production release sequence through
    /// the target's `prepare` hook, so it is sabotage-sensitive: deleting hook
    /// execution makes the drafts never freeze and the asserts below fail.
    ///
    /// The bare temp worktree has no manifests (so the version bump is a no-op).
    /// A deliberate release-note archive collision then stops the workflow before
    /// it launches an agent. Canonicalization runs first, so the tree is already
    /// frozen by the time `prepare_release_in_worktree` returns that expected error.
    #[test]
    fn the_release_run_canonicalizes_drafts_into_the_committed_tree() {
        let script =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/canonicalize_migrations.py");
        if !script.is_file() || !python3_available() {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let migrations = root.join("rust/loopflow/src/store/migrations");
        let drafts = migrations.join("drafts");
        fs::create_dir_all(&drafts).unwrap();
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::copy(&script, root.join("scripts/canonicalize_migrations.py")).unwrap();
        fs::write(
            migrations.join("0.11.001_initial.sql"),
            "CREATE TABLE waves (id TEXT);\n",
        )
        .unwrap();
        let registry_rs = root.join("rust/loopflow/src/store/migrations.rs");
        fs::write(
            &registry_rs,
            "const MIGRATIONS: &[Migration] = &[\n    Migration {\n        \
             id: MigrationId {\n            major: 0,\n            minor: 11,\n            \
             patch: None,\n            ordinal: 1,\n        },\n        name: \"initial\",\n        \
             sql: include_str!(\"migrations/0.11.001_initial.sql\"),\n    },\n];\n",
        )
        .unwrap();
        fs::write(
            drafts.join("add_wave_colour__deadbeefdeadbeefdeadbeefdeadbeef.sql"),
            "-- name: add_wave_colour\n-- id: deadbeefdeadbeefdeadbeefdeadbeef\n\
             -- depends_on: \n\
             ALTER TABLE waves ADD COLUMN colour TEXT;\n",
        )
        .unwrap();
        let unreleased = root.join("release/unreleased");
        let release_archive = root.join("release/v0.11.4");
        fs::create_dir_all(&unreleased).unwrap();
        fs::create_dir_all(&release_archive).unwrap();
        fs::write(unreleased.join("collision.md"), "unreleased\n").unwrap();
        fs::write(release_archive.join("collision.md"), "archived\n").unwrap();

        // A target with no manifests: the version bump is a no-op, the repo hook
        // runs, then the deliberate archive collision stops the notes stage.
        let target = ReleaseTarget {
            name: "default".to_string(),
            area: Vec::new(),
            tag_prefix: "v".to_string(),
            manifests: Vec::new(),
            workflow: None,
            verify: Vec::new(),
            prepare: vec![
                "python3 scripts/canonicalize_migrations.py {version} --release-cut".to_string(),
            ],
            completion: ReleaseCompletion::Tag,
            publisher: Vec::new(),
        };
        let result = prepare_release_in_worktree(
            root,
            "0.11.4",
            Some("v0.11.3"),
            &[],
            &[],
            &target,
            &crate::ops::progress::NullProgress,
        );
        assert!(
            result.is_err(),
            "release-note collision should stop release"
        );

        // The draft is now a canonical, registered migration.
        let canonical = migrations.join("0.11.4.001_release.sql");
        assert!(canonical.is_file(), "canonical migration not written");
        assert_eq!(
            fs::read_to_string(&canonical).unwrap(),
            "-- draft: add_wave_colour\nALTER TABLE waves ADD COLUMN colour TEXT;\n"
        );
        let registry = fs::read_to_string(&registry_rs).unwrap();
        assert!(registry.contains("patch: Some(4)"), "patch not registered");
        assert!(registry.contains("name: \"release\""), "not registered");
        // The draft is consumed.
        let remaining: Vec<_> = fs::read_dir(&drafts)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".sql"))
            .collect();
        assert!(remaining.is_empty(), "draft not consumed");
    }

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
            verify: vec![],
            prepare: vec![],
            completion: ReleaseCompletion::Tag,
            publisher: Vec::new(),
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
            verify: vec![],
            prepare: vec![],
            completion: ReleaseCompletion::Tag,
            publisher: Vec::new(),
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
            verify: vec![],
            prepare: vec![],
            completion: ReleaseCompletion::Tag,
            publisher: Vec::new(),
        };
        assert!(version_from_tag("v1.2.3", &target).is_err());
    }

    #[test]
    fn first_release_requires_an_explicit_semver() {
        let target = default_release_target(Path::new("."));

        let error = resolve_version(None, "patch", &target)
            .unwrap_err()
            .to_string();
        assert!(error.contains("explicit X.Y.Z"), "{error}");
        assert_eq!(resolve_version(None, "1.0.0", &target).unwrap(), "1.0.0");
    }

    #[test]
    fn target_tag_default() {
        let target = ReleaseTarget {
            name: "default".to_string(),
            area: vec![],
            tag_prefix: String::new(),
            manifests: vec![],
            workflow: None,
            verify: vec![],
            prepare: vec![],
            completion: ReleaseCompletion::Tag,
            publisher: Vec::new(),
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
            verify: vec![],
            prepare: vec![],
            completion: ReleaseCompletion::Tag,
            publisher: Vec::new(),
        };
        assert_eq!(target_tag(&target, "2.0.0"), "cli/v2.0.0");
    }

    #[test]
    fn release_pr_copy_uses_generated_release_notes() {
        let tmp = tempfile::tempdir().unwrap();
        let target = default_release_target(tmp.path());
        write_release_notes(tmp.path(), "## Highlights\n\n- Releases work.", "1.2.3").unwrap();

        let copy = release_pr_copy(tmp.path(), &target, "1.2.3").unwrap();

        assert_eq!(copy.title, "release: v1.2.3");
        assert_eq!(copy.body, "# v1.2.3\n\n## Highlights\n\n- Releases work.");
    }

    // ======================================================================
    // release/unreleased → release/v<version> promotion + NOTES archival
    // ======================================================================

    #[test]
    fn promote_unreleased_is_noop_without_dir() {
        let tmp = tempfile::tempdir().unwrap();
        promote_unreleased_dir(tmp.path(), "0.9.10").unwrap();
        assert!(!tmp.path().join("release").join("v0.9.10").exists());
    }

    #[test]
    fn promote_unreleased_renames_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let unreleased = tmp.path().join("release").join("unreleased");
        fs::create_dir_all(&unreleased).unwrap();
        fs::write(unreleased.join("CHANGES.md"), "ledger").unwrap();

        promote_unreleased_dir(tmp.path(), "0.9.10").unwrap();

        assert!(!unreleased.exists());
        let promoted = tmp.path().join("release").join("v0.9.10");
        assert_eq!(
            fs::read_to_string(promoted.join("CHANGES.md")).unwrap(),
            "ledger",
        );
    }

    #[test]
    fn promote_unreleased_merges_into_existing_version_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let unreleased = tmp.path().join("release").join("unreleased");
        fs::create_dir_all(&unreleased).unwrap();
        fs::write(unreleased.join("CHANGES.md"), "ledger").unwrap();

        let version_dir = tmp.path().join("release").join("v0.9.10");
        fs::create_dir_all(&version_dir).unwrap();
        fs::write(version_dir.join("NOTES.md"), "backfilled notes").unwrap();

        promote_unreleased_dir(tmp.path(), "0.9.10").unwrap();

        assert!(!unreleased.exists());
        assert_eq!(
            fs::read_to_string(version_dir.join("CHANGES.md")).unwrap(),
            "ledger",
        );
        assert_eq!(
            fs::read_to_string(version_dir.join("NOTES.md")).unwrap(),
            "backfilled notes",
        );
    }

    #[test]
    fn promote_unreleased_errors_on_file_collision() {
        let tmp = tempfile::tempdir().unwrap();
        let unreleased = tmp.path().join("release").join("unreleased");
        fs::create_dir_all(&unreleased).unwrap();
        fs::write(unreleased.join("CHANGES.md"), "new").unwrap();

        let version_dir = tmp.path().join("release").join("v0.9.10");
        fs::create_dir_all(&version_dir).unwrap();
        fs::write(version_dir.join("CHANGES.md"), "old").unwrap();

        assert!(promote_unreleased_dir(tmp.path(), "0.9.10").is_err());
    }

    #[test]
    fn archive_release_notes_copies_root_to_version_dir() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("RELEASE_NOTES.md"), "# v0.9.10\n\nnotes").unwrap();

        archive_release_notes(tmp.path(), "0.9.10").unwrap();

        assert_eq!(
            fs::read_to_string(tmp.path().join("release").join("v0.9.10").join("NOTES.md"))
                .unwrap(),
            "# v0.9.10\n\nnotes",
        );
    }

    // ======================================================================
    // ensure_commit_local
    // ======================================================================

    #[test]
    fn ensure_commit_local_fetches_object_pushed_by_another_clone() {
        use loopflow_test_support::TestRepo;
        use std::process::Command;

        let repo = TestRepo::new();

        // A second clone simulates a contributor who pushed a commit the
        // local repo hasn't fetched yet — exactly the merge-queue race
        // that wedged tag creation in v0.9.10.
        let other = tempfile::tempdir().unwrap();
        run_or_panic(Command::new("git").args([
            "clone",
            repo.bare_path().to_str().unwrap(),
            other.path().to_str().unwrap(),
        ]));
        run_or_panic(Command::new("git").current_dir(other.path()).args([
            "config",
            "user.email",
            "remote@example.com",
        ]));
        run_or_panic(Command::new("git").current_dir(other.path()).args([
            "config",
            "user.name",
            "Remote",
        ]));
        std::fs::write(other.path().join("remote.txt"), "remote-only").unwrap();
        run_or_panic(
            Command::new("git")
                .current_dir(other.path())
                .args(["add", "."]),
        );
        run_or_panic(Command::new("git").current_dir(other.path()).args([
            "commit",
            "-m",
            "remote commit",
        ]));
        let new_sha = String::from_utf8(
            Command::new("git")
                .current_dir(other.path())
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        run_or_panic(Command::new("git").current_dir(other.path()).args(["push"]));

        assert!(
            !commit_exists_locally(repo.path(), &new_sha),
            "precondition: object should not exist locally before fetch",
        );

        ensure_commit_local(repo.path(), &new_sha).expect("fetch + verify should succeed");

        assert!(commit_exists_locally(repo.path(), &new_sha));
    }

    fn run_or_panic(cmd: &mut std::process::Command) {
        let output = cmd.output().expect("run git");
        if !output.status.success() {
            panic!(
                "git {:?} failed: {}",
                cmd,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    fn release_hooks_are_repo_owned_and_expand_release_context() {
        let repo = tempfile::tempdir().unwrap();
        let target = default_release_target(repo.path());
        run_release_hooks(
            repo.path(),
            &["printf '%s %s %s' '{target}' '{version}' '{previous_tag}' > hook.txt".to_string()],
            &target,
            Some("1.2.3"),
            Some("v1.2.2"),
            "preparation",
        )
        .unwrap();

        assert_eq!(
            fs::read_to_string(repo.path().join("hook.txt")).unwrap(),
            "default 1.2.3 v1.2.2"
        );
    }

    #[test]
    fn failing_release_hook_stops_the_release() {
        let repo = tempfile::tempdir().unwrap();
        let target = default_release_target(repo.path());
        let error = run_release_hooks(
            repo.path(),
            &["echo 'repository rejected release' >&2; exit 1".to_string()],
            &target,
            None,
            None,
            "verification",
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("repository rejected release"), "{error}");
    }
}
