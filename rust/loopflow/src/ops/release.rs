use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, Instant};

use crate::ops::release_lock::ReleaseLock;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::command::{run_command, CommandError};
use crate::config::{load_config_or_default, Config, ReleaseCompletion, ReleaseTargetConfig};
use crate::git::worktrees::{
    branch_exists, create_named_worktree, list_porcelain, main_repo_root, worktree_path,
    CreateWorktreeResult,
};
use crate::git::{
    acquire_worktree_lease, acquire_worktree_lease_wait, current_branch,
    delete_local_branch_inheriting, fetch_inheriting, get_default_branch, is_clean, ref_exists,
    rev_parse, worktree_remove_owned, WorktreeLease,
};
use crate::naming::{git_user, sanitize_for_branch};
use crate::ops::commit::{commit_workflow, CommitOptions};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::land::{finish_arm_after_sync, LandOptions};
use crate::ops::pr::{current_pr, merge_gate_state, PrCopy};
use crate::ops::progress::Progress;
use crate::ops::util::command_exists;
use crate::prompt::write_prompt_log;

const RELEASE_QUEUE_PR_LIMIT: usize = 200;
const RELEASE_QUERY_PR_LIMIT: usize = 1000;
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
const RELEASE_CONTEXT_MAX_BREAKING_BYTES: usize = 4 * 1024;
const RELEASE_CONTEXT_MAX_PREVIOUS_NOTES_BYTES: usize = 16 * 1024;
const RELEASE_NOTES_MAX_BYTES: usize = 60 * 1024;
const FALLBACK_NOTES_MAX_COMMITS: usize = 50;
const FALLBACK_NOTES_MAX_PRS: usize = 50;
const RELEASE_NOTES_STATUS_PREFIX: &str = "<!-- loopflow:release-notes=";
const RELEASE_WORKTREE_CONTEXT_ENV: [&str; 3] = [
    crate::lf::WORK_DECLARATION_ENV,
    crate::session_record::CAPTURE_KEY_ENV,
    crate::work::wave::context::WAVE_ID_ENV,
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

#[derive(Debug, Deserialize, Serialize, Clone)]
struct GhRunListEntry {
    #[serde(rename = "databaseId")]
    database_id: u64,
    #[serde(default, rename = "headBranch")]
    head_branch: Option<String>,
    #[serde(default, rename = "headSha")]
    head_sha: Option<String>,
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
struct GhReleasePr {
    number: u64,
    state: String,
    #[serde(default, rename = "mergeCommit")]
    merge_commit: Option<GhPrMergeCommit>,
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

#[derive(Debug, Clone, Serialize)]
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
        origin_commit: String,
    },
    Released(ReleaseReceipt),
    Resumed(ReleaseReceipt),
}

#[derive(Debug)]
struct ReleaseWorkflowResult {
    database_id: u64,
    url: Option<String>,
}

#[derive(Debug, Clone)]
struct ReleaseCandidate {
    tag: String,
    commit: String,
    branch: String,
}

impl ReleaseCandidate {
    fn new(target: &ReleaseTarget, tag: &str, commit: &str) -> Self {
        let target_name = sanitize_ref_segment(&target.name);
        let tag_name = sanitize_ref_segment(tag);
        Self {
            tag: tag.to_string(),
            commit: commit.to_string(),
            branch: format!("release-candidate/{target_name}/{tag_name}/{commit}"),
        }
    }
}

/// The released tag a candidate branch was cut for: its name parses as one of
/// [`ReleaseCandidate`]'s branches, `head` is the commit it names, and the tag
/// exists. Anything else is not release evidence.
pub(crate) fn released_candidate_tag<'a>(
    branch: &str,
    head: &str,
    tags: &'a HashSet<String>,
) -> Option<&'a str> {
    let rest = branch.strip_prefix("release-candidate/")?;
    let (rest, commit) = rest.rsplit_once('/')?;
    let (target, tag_name) = rest.rsplit_once('/')?;
    if target.is_empty() || target.contains('/') || commit != head {
        return None;
    }
    tags.iter()
        .find(|tag| sanitize_ref_segment(tag) == tag_name)
        .map(String::as_str)
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
    let (major, minor, patch) = parse_version(current)
        .ok_or_else(|| OpsError::Parse(format!("invalid version format: {current}")))?;
    match bump {
        "major" => Ok(format!("{}.0.0", major + 1)),
        "minor" => Ok(format!("{}.{}.0", major, minor + 1)),
        "patch" => Ok(format!("{}.{}.{}", major, minor, patch + 1)),
        _ => Err(OpsError::Parse(format!("unknown bump type: {bump}"))),
    }
}

/// `X.Y.Z`, with an optional leading `v`.
fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version
        .trim()
        .trim_start_matches('v')
        .split('.')
        .map(|part| part.parse().ok());
    let triple = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(triple)
}

pub fn release_status(repo: &Path, target_name: Option<&str>) -> OpsResult<ReleaseStatusResult> {
    if !command_exists("gh") {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    let (main_repo, target) = resolve_repo_and_target(repo, target_name)?;

    let latest_tag = latest_tag_optional(&main_repo, &target)?;
    let (notes_status, workflow, release_exists) = match latest_tag.as_deref() {
        Some(tag) => {
            let candidate = release_candidate_for_tag(&main_repo, tag, &target)?;
            (
                Some(release_notes_status(&main_repo, tag, &target)?),
                find_workflow_run(&main_repo, &candidate, &target)?,
                github_release_exists(&main_repo, tag)?,
            )
        }
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

/// Generate release notes for the given version from origin's default branch,
/// the base `lf release run` releases.
///
/// Writes RELEASE_NOTES.md into the checkout containing `repo`. Returns the
/// generated notes content.
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
    let checkout = crate::repo::discover_repo_root(repo)
        .map_err(|error| OpsError::Message(error.to_string()))?
        .unwrap_or_else(|| repo.to_path_buf());

    let resolved_prev_tag = match prev_tag {
        Some(tag) => Some(tag.to_string()),
        None => notes_base(&main_repo, version, &target)?,
    };

    let version = normalize_version(version);

    progress.status("Collecting release changes...");
    let default_branch = get_default_branch(&main_repo)?;
    fetch_inheriting(
        &main_repo,
        "origin",
        &format!("+refs/heads/{default_branch}:refs/remotes/origin/{default_branch}"),
        &|_| {},
    )?;
    let changes = collect_changes_between(
        &main_repo,
        resolved_prev_tag.as_deref(),
        &format!("origin/{default_branch}"),
        &target,
    )?;

    progress.status("Generating narrative release notes...");
    run_release_notes_stage(
        &checkout,
        &version,
        resolved_prev_tag.as_deref(),
        &changes.commits,
        &changes.merged_prs,
        &target,
        progress,
        &|_| {},
    )?;

    let notes = fs::read_to_string(checkout.join("RELEASE_NOTES.md"))?;
    Ok(notes)
}

/// Generate notes in a temporary directory without modifying release files.
pub fn preview_release_notes(
    repo: &Path,
    version: &str,
    prev_tag: Option<&str>,
    target_name: Option<&str>,
    progress: &impl Progress,
) -> OpsResult<String> {
    let (repo, target) = resolve_repo_and_target(repo, target_name)?;
    let version = normalize_version(version);
    let base = match prev_tag {
        Some(tag) => Some(tag.to_string()),
        None => notes_base(&repo, &version, &target)?,
    };
    let tagged_revision = local_tag_sha(&repo, &target_tag(&target, &version))?;
    let revision = match &tagged_revision {
        Some(revision) => revision.clone(),
        None => rev_parse(&repo, "HEAD")?,
    };
    let changes = collect_changes_between(&repo, base.as_deref(), &revision, &target)?;
    let range_start = base.as_deref().unwrap_or("repository start");
    progress.status(&format!(
        "Previewing v{version}: {range_start}..{revision} ({} commits, {} merged PRs)",
        changes.commits.len(),
        changes.merged_prs.len()
    ));
    let ((previous, previous_omitted), (decisions, decisions_omitted), (breaking, _)) =
        if tagged_revision.is_some() {
            (
                read_bounded_revision_text(
                    &repo,
                    &format!("{revision}^:RELEASE_NOTES.md"),
                    RELEASE_CONTEXT_MAX_PREVIOUS_NOTES_BYTES,
                )?,
                read_bounded_revision_text(
                    &repo,
                    &format!("{revision}:release/v{version}/DECISIONS.md"),
                    RELEASE_CONTEXT_MAX_DECISIONS_BYTES,
                )?,
                read_bounded_revision_text(
                    &repo,
                    &format!("{revision}:release/v{version}/BREAKING.md"),
                    RELEASE_CONTEXT_MAX_BREAKING_BYTES,
                )?,
            )
        } else {
            let release_file = |name: &str| {
                let unreleased = repo.join("release/unreleased").join(name);
                if unreleased.exists() {
                    unreleased
                } else {
                    repo.join(format!("release/v{version}")).join(name)
                }
            };
            (
                read_bounded_text(
                    &repo.join("RELEASE_NOTES.md"),
                    RELEASE_CONTEXT_MAX_PREVIOUS_NOTES_BYTES,
                )?,
                read_bounded_text(
                    &release_file("DECISIONS.md"),
                    RELEASE_CONTEXT_MAX_DECISIONS_BYTES,
                )?,
                read_bounded_text(
                    &release_file("BREAKING.md"),
                    RELEASE_CONTEXT_MAX_BREAKING_BYTES,
                )?,
            )
        };
    let (context, json) = build_release_notes_context(
        &version,
        base.as_deref(),
        &changes.commits,
        &changes.merged_prs,
        &target,
        decisions,
        decisions_omitted,
        breaking_lines(breaking.as_deref()),
        previous,
        previous_omitted,
    )?;
    let output = tempfile::tempdir()?;
    generate_notes_file(&repo, output.path(), &context, &json, progress, &|_| {})?;
    Ok(fs::read_to_string(output.path().join("RELEASE_NOTES.md"))?)
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
    bump_manifest_versions(&main_repo, &target, &version, progress, &|_| {})
}

/// Create a git tag and push it to the remote.
///
/// Returns the full tag string (e.g. `v0.9.6`).
pub fn release_tag(repo: &Path, version: &str, target_name: Option<&str>) -> OpsResult<String> {
    let (main_repo, target) = resolve_repo_and_target(repo, target_name)?;

    let lock = ReleaseLock::acquire(&main_repo, &target.name)?;
    let version = normalize_version(version);
    tag_and_push_ref(&main_repo, &version, &target, None, &lock)
}

/// Stage assets on a draft GitHub Release or publish that draft without marking it Latest.
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
    let config = load_config_or_default(Some(&main_repo));
    let matching: Vec<_> = config
        .release
        .targets
        .iter()
        .map(|(name, cfg)| build_release_target(name, cfg, &main_repo))
        .filter(|target| version_from_tag(tag, target).is_ok())
        .collect();
    let target = match matching.as_slice() {
        [target] => target.clone(),
        [] if config.release.targets.is_empty() => default_release_target(&main_repo),
        _ => {
            return Err(OpsError::Message(format!(
                "release tag {tag} does not identify exactly one configured target"
            )))
        }
    };
    let lock = ReleaseLock::acquire(&main_repo, &target.name)?;

    if finalize {
        if github_release_state(&main_repo, tag)? == GitHubReleaseState::Missing {
            return Err(OpsError::Message(format!(
                "cannot publish {tag}: draft GitHub Release does not exist"
            )));
        }
        run_locked_stdout(
            &lock,
            &main_repo,
            "gh",
            // GitHub's Latest marker is the `stable` pointer; only the release
            // run moves it, once this release has soaked.
            &["release", "edit", tag, "--draft=false", "--latest=false"],
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
            run_locked_stdout(&lock, &main_repo, "gh", &refs)?;
        }
        GitHubReleaseState::Draft | GitHubReleaseState::Published => {
            run_locked_stdout(
                &lock,
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
                run_locked_stdout(&lock, &main_repo, "gh", &refs)?;
            }
        }
    }
    Ok(())
}

/// Run the full release workflow in one shot.
///
/// Graph:
/// 1) check target-scoped commits since the previous tag
/// 2) bump manifests, run repo preparation, and generate notes in a worktree
/// 3) commit, open PR, and enqueue auto-merge
/// 4) wait for merge queue completion
/// 5) build the merged commit under a provisional candidate ref
/// 6) prepare the publisher's signed artifact set, when configured
/// 7) tag the proven commit and publish those exact artifacts
/// 8) move the `stable` pointer forward, for targets completing on a GitHub Release
pub fn release_run(
    repo: &Path,
    version_input: &str,
    target_name: Option<&str>,
    progress: &impl Progress,
) -> OpsResult<ReleaseRunOutcome> {
    let (main_repo, target) = resolve_repo_and_target(repo, target_name)?;
    let lock = &ReleaseLock::acquire(&main_repo, &target.name)?;
    if !command_exists("gh") {
        return Err(OpsError::Message("gh CLI not found".into()));
    }
    let outcome = release_version(&main_repo, &target, version_input, progress, lock)?;
    if target.completion == ReleaseCompletion::GithubRelease {
        advance_stable(&main_repo, &target, progress, lock)?;
    }
    Ok(outcome)
}

fn release_version(
    main_repo: &Path,
    target: &ReleaseTarget,
    version_input: &str,
    progress: &impl Progress,
    lock: &ReleaseLock,
) -> OpsResult<ReleaseRunOutcome> {
    let default_branch = get_default_branch(main_repo)?;
    fetch_inheriting(
        main_repo,
        "origin",
        &format!("+refs/heads/{default_branch}:refs/remotes/origin/{default_branch}"),
        &|command| lock.inherit(command),
    )?;
    let source_commit = rev_parse(main_repo, &format!("origin/{default_branch}"))?;

    run_publisher_check(main_repo, target, progress)?;

    let latest_tag = latest_tag_optional(main_repo, target)?;
    let version = resolve_version(latest_tag.as_deref(), version_input, target)?;
    verify_source(
        main_repo,
        &source_commit,
        target,
        &version,
        latest_tag.as_deref(),
        progress,
        lock,
    )?;

    if let Some(tag) = latest_tag.as_deref() {
        if !target.publisher.is_empty()
            && github_release_state(main_repo, tag)? != GitHubReleaseState::Published
        {
            let candidate = release_candidate_for_tag(main_repo, tag, target)?;
            let run = find_workflow_run(main_repo, &candidate, target)?.ok_or_else(|| {
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
                return Err(OpsError::Message(format!(
                    "unfinished release {tag} build failed ({conclusion}); retry its build before starting another version"
                )));
            } else {
                progress.status(&format!("Resuming incomplete release {tag}..."));
                return resume_existing_release(main_repo, tag, target, progress, lock);
            }
        } else if target.publisher.is_empty()
            && matches!(version_input.trim(), "patch" | "minor" | "major")
            && !release_completion_satisfied(main_repo, tag, target)?
        {
            progress.status(&format!("Resuming release completion for {tag}..."));
            return resume_existing_release(main_repo, tag, target, progress, lock);
        }
    }

    if !matches!(version_input.trim(), "patch" | "minor" | "major") {
        let version = resolve_version(None, version_input, target)?;
        let tag = target_tag(target, &version);
        if remote_tag_sha(main_repo, &tag)?.is_some() {
            progress.status(&format!("Resuming release completion for {tag}..."));
            return resume_existing_release(main_repo, &tag, target, progress, lock);
        }
    }

    let changes = collect_version_changes(main_repo, target, &version, &source_commit)?;
    if changes.commits.is_empty() {
        return Ok(ReleaseRunOutcome::NoChanges {
            target: target.name.clone(),
            latest_tag,
            origin_commit: source_commit,
        });
    }

    let new_tag = target_tag(target, &version);
    if remote_tag_sha(main_repo, &new_tag)?.is_some() {
        progress.status(&format!("Resuming release completion for {new_tag}..."));
        return resume_existing_release(main_repo, &new_tag, target, progress, lock);
    }

    let latest_candidate = find_latest_candidate_workflow(main_repo, &new_tag, target)?;
    let mut resumed_candidate = None;
    let mut retry_after = None;
    if let Some(run) = latest_candidate {
        let commit = run
            .head_sha
            .clone()
            .expect("candidate workflow was selected with a head SHA");
        let conclusion = run
            .conclusion
            .as_deref()
            .unwrap_or("unknown")
            .to_lowercase();
        let build_failed = run.status == "completed" && conclusion != "success";
        let preparation_missing = run.status == "completed"
            && conclusion == "success"
            && !target.publisher.is_empty()
            && !publisher_artifact_dir(main_repo, &new_tag, &commit, run.database_id)
                .join("candidate.json")
                .is_file();
        if !build_failed && !preparation_missing {
            resumed_candidate = Some(commit);
        } else {
            let main_ref = format!("origin/{default_branch}");
            let current_main = run_stdout(main_repo, "git", &["rev-parse", &main_ref])?;
            if current_main == commit {
                if preparation_missing {
                    resumed_candidate = Some(commit);
                } else {
                    let url = run
                        .url
                        .unwrap_or_else(|| "workflow URL unavailable".to_string());
                    return Err(OpsError::Message(format!(
                    "release candidate for {new_tag} failed: {conclusion} ({url}); no merged fix is available"
                )));
                }
            } else {
                let reason = if build_failed {
                    format!("failed build ({conclusion})")
                } else {
                    "incomplete publisher preparation".to_string()
                };
                progress.status(&format!(
                    "Retrying {new_tag} after {reason} with merged fixes..."
                ));
                retry_after = Some(commit);
            }
        }
    }

    let mut wt_name = release_worktree_name(target, &version);
    if let Some(commit) = retry_after.as_deref() {
        let revision = commit.get(..9).unwrap_or(commit);
        wt_name.push_str("-retry-");
        wt_name.push_str(revision);
    }
    let branch = release_branch_name(main_repo, &wt_name)?;
    let existing_pr = find_release_pr(main_repo, &branch)?;
    let merged_commit = if let Some(commit) = resumed_candidate {
        commit
    } else if let Some(pr) = existing_pr {
        match pr.state.as_str() {
            "MERGED" => {
                let release_commit = pr.merge_commit.map(|commit| commit.oid).ok_or_else(|| {
                    OpsError::Message(format!(
                        "release PR #{} is merged but its merge commit is unavailable",
                        pr.number
                    ))
                })?;
                // No candidate was built yet, so the version is unspent: corrections
                // merged after the release PR ship under the same version.
                let main_ref = format!("origin/{default_branch}");
                let current_main = run_stdout(main_repo, "git", &["rev-parse", &main_ref])?;
                if current_main != release_commit
                    && is_ancestor(main_repo, &release_commit, &current_main)
                {
                    progress.status(&format!(
                        "Release PR #{} merged at {release_commit}; building {main_ref} at {current_main} with the fixes merged since...",
                        pr.number
                    ));
                    current_main
                } else {
                    release_commit
                }
            }
            "OPEN" => {
                let head_sha = pr.head_ref_oid.as_deref().ok_or_else(|| {
                    OpsError::Message(format!(
                        "open release PR #{} has no observable head commit",
                        pr.number
                    ))
                })?;
                progress.status(&format!("Resuming release PR #{}...", pr.number));
                arm_resumed_release(main_repo, pr.number, head_sha, progress, lock)?;
                finish_release_pr(
                    main_repo,
                    &wt_name,
                    PreparedRelease {
                        pr_number: pr.number,
                        head_sha: head_sha.to_string(),
                    },
                    target,
                    &version,
                    progress,
                    lock,
                )?
            }
            _ => {
                let url = format!("PR #{}", pr.number);
                return Err(OpsError::Message(format!(
                "{url} was closed without merging; remove or rename the release branch before retrying"
            )));
            }
        }
    } else {
        progress.status(&format!("Creating release worktree {wt_name}..."));
        let lease = acquire_worktree_lease(
            main_repo,
            &worktree_path(main_repo, &wt_name),
            "release PR preparation",
        )?;
        let wt = create_named_worktree(main_repo, &wt_name, Some(&source_commit), &|command| {
            lock.inherit(command);
            lease.inherit(command);
        })?;
        let wt_path = wt.path;
        let wt_branch = wt.branch;

        let prepared = prepare_release_in_worktree(
            &wt_path, &version, &changes, target, progress, lock, &lease,
        );
        cleanup_release_worktree(main_repo, &wt_path, &wt_branch, lease, progress, lock);
        let prepared = prepared?;

        progress.status("Waiting for release PR to merge...");
        finish_release_pr(
            main_repo, &wt_name, prepared, target, &version, progress, lock,
        )?
    };

    let tag = target_tag(target, &version);
    let candidate = ReleaseCandidate::new(target, &tag, &merged_commit);
    finish_candidate(main_repo, &candidate, target, progress, lock)
}

fn resume_existing_release(
    repo: &Path,
    tag: &str,
    target: &ReleaseTarget,
    progress: &impl Progress,
    lock: &ReleaseLock,
) -> OpsResult<ReleaseRunOutcome> {
    let candidate = release_candidate_for_tag(repo, tag, target)?;
    finish_candidate(repo, &candidate, target, progress, lock)
}

fn checked_candidate_tag(repo: &Path, candidate: &ReleaseCandidate) -> OpsResult<Option<String>> {
    let tagged = remote_tag_sha(repo, &candidate.tag)?;
    if tagged.as_ref().is_some_and(|sha| sha != &candidate.commit) {
        return Err(OpsError::Message(format!(
            "release {} moved away from saved candidate {}",
            candidate.tag, candidate.commit
        )));
    }
    Ok(tagged)
}

fn finish_candidate(
    repo: &Path,
    candidate: &ReleaseCandidate,
    target: &ReleaseTarget,
    progress: &impl Progress,
    lock: &ReleaseLock,
) -> OpsResult<ReleaseRunOutcome> {
    let tagged = checked_candidate_tag(repo, candidate)?;
    // Origin preflight cannot prove a resumed or newly merged candidate.
    verify_source(
        repo,
        &candidate.commit,
        target,
        &version_from_tag(&candidate.tag, target)?,
        None,
        progress,
        lock,
    )?;
    let workflow = if tagged.is_some() {
        wait_for_release_workflow(repo, candidate, target, progress, false)?
    } else {
        wait_for_candidate_workflow(repo, candidate, target, progress, lock)?
    };
    let published = github_release_exists(repo, &candidate.tag)?;
    if !published {
        let artifacts = if target.publisher.is_empty() {
            None
        } else {
            Some(prepare_publisher(
                repo, candidate, target, &workflow, progress, lock,
            )?)
        };
        if tagged.is_none() {
            let version = version_from_tag(&candidate.tag, target)?;
            tag_and_push_ref(repo, &version, target, Some(&candidate.commit), lock)?;
        }
        if let Some(artifacts) = artifacts.as_deref() {
            run_publisher(
                repo,
                &candidate.tag,
                target,
                &workflow,
                artifacts,
                progress,
                lock,
            )?;
            delete_prepared_artifacts(artifacts, progress);
        } else if target.completion == ReleaseCompletion::GithubRelease {
            wait_for_release_workflow(repo, candidate, target, progress, false)?;
        }
    }
    let receipt = ReleaseReceipt {
        target: target.name.clone(),
        version: version_from_tag(&candidate.tag, target)?,
        tag: candidate.tag.clone(),
        commit: candidate.commit.clone(),
        workflow_run_id: workflow.database_id,
        workflow_url: workflow.url,
        release_exists: github_release_exists(repo, &candidate.tag)?,
    };
    delete_candidate_ref(repo, candidate, lock);
    Ok(if tagged.is_some() {
        ReleaseRunOutcome::Resumed(receipt)
    } else {
        ReleaseRunOutcome::Released(receipt)
    })
}

fn verify_source(
    repo: &Path,
    source: &str,
    target: &ReleaseTarget,
    version: &str,
    previous: Option<&str>,
    progress: &impl Progress,
    lock: &ReleaseLock,
) -> OpsResult<()> {
    if !target.verify.is_empty() {
        let name = format!("verify-{}-{source}", sanitize_ref_segment(&target.name));
        let lease = acquire_worktree_lease(
            repo,
            &worktree_path(repo, &name),
            "release source verification",
        )?;
        let wt = materialize_exact_source_worktree(repo, &name, source, &lease, lock)?;
        let result = run_release_hooks(
            &wt.path,
            &target.verify,
            target,
            Some(version),
            previous,
            "verification",
            lock,
            &lease,
        );
        cleanup_release_worktree(repo, &wt.path, &wt.branch, lease, progress, lock);
        result?;
    }
    Ok(())
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

fn materialize_exact_source_worktree(
    repo: &Path,
    worktree_name: &str,
    revision: &str,
    lease: &WorktreeLease,
    lock: &ReleaseLock,
) -> OpsResult<CreateWorktreeResult> {
    let inherit = |command: &mut Command| {
        lock.inherit(command);
        lease.inherit(command);
    };
    let path = worktree_path(repo, worktree_name);
    let branch = release_branch_name(repo, worktree_name)?;
    let expected_commit = rev_parse(repo, &format!("{revision}^{{commit}}"))?;
    let registrations = list_porcelain(repo)?;
    let registered_at_path = registrations
        .iter()
        .find(|(registered_path, _)| registered_path == &path)
        .map(|(_, registered_branch)| registered_branch.clone());
    let branch_registration = registrations
        .iter()
        .find(|(_, registered_branch)| registered_branch.as_deref() == Some(branch.as_str()))
        .map(|(registered_path, _)| registered_path.clone());

    if let Some(registered_path) = branch_registration.as_deref() {
        if registered_path != path {
            return Err(publisher_worktree_refused(
                &path,
                &branch,
                &expected_commit,
                &format!(
                    "generated branch is registered at {}",
                    registered_path.display()
                ),
            ));
        }
    }

    if let Some(observed_branch) = registered_at_path.as_ref() {
        if observed_branch.as_deref() != Some(branch.as_str()) {
            let observed = observed_branch.as_deref().unwrap_or("detached HEAD");
            return Err(publisher_worktree_refused(
                &path,
                &branch,
                &expected_commit,
                &format!("path is registered on {observed}"),
            ));
        }
    }

    let local_head = if branch_exists(repo, &branch)? {
        Some(rev_parse(repo, &format!("refs/heads/{branch}^{{commit}}"))?)
    } else {
        None
    };
    if let Some(observed) = local_head.as_deref() {
        if observed != expected_commit {
            return Err(publisher_worktree_refused(
                &path,
                &branch,
                &expected_commit,
                &format!("local branch resolves to {observed}"),
            ));
        }
    }

    let remote_ref = format!("refs/remotes/origin/{branch}");
    if ref_exists(repo, &remote_ref)? {
        let remote_head = rev_parse(repo, &format!("{remote_ref}^{{commit}}"))?;
        if remote_head != expected_commit {
            return Err(publisher_worktree_refused(
                &path,
                &branch,
                &expected_commit,
                &format!("origin branch resolves to {remote_head}"),
            ));
        }
    }

    if registered_at_path.is_some() && path.exists() {
        let observed_branch = current_branch(&path)?;
        if observed_branch.as_deref() != Some(branch.as_str()) {
            let observed = observed_branch.as_deref().unwrap_or("detached HEAD");
            return Err(publisher_worktree_refused(
                &path,
                &branch,
                &expected_commit,
                &format!("worktree reports branch {observed}"),
            ));
        }
        let observed_head = rev_parse(&path, "HEAD^{commit}")?;
        if observed_head != expected_commit {
            return Err(publisher_worktree_refused(
                &path,
                &branch,
                &expected_commit,
                &format!("worktree HEAD resolves to {observed_head}"),
            ));
        }
        if !is_clean(&path)? {
            let status = run_stdout(&path, "git", &["status", "--short"])?;
            return Err(publisher_worktree_refused(
                &path,
                &branch,
                &expected_commit,
                &format!("worktree is dirty: {status}"),
            ));
        }
        return Ok(CreateWorktreeResult {
            path,
            branch,
            base_branch: None,
            base_commit: Some(expected_commit),
        });
    }

    if registered_at_path.is_none() && path.exists() && !publisher_path_is_empty(&path)? {
        return Err(publisher_worktree_refused(
            &path,
            &branch,
            &expected_commit,
            "unregistered generated path is not an empty directory",
        ));
    }

    if registered_at_path.is_some() {
        worktree_remove_owned(repo, &path, true, lease, &inherit)?;
    }
    if local_head.is_some() {
        delete_local_branch_inheriting(repo, &branch, &inherit)?;
    }
    if path.exists() {
        fs::remove_dir(&path)?;
    }

    let worktree = create_named_worktree(repo, worktree_name, Some(revision), &inherit)?;
    let observed_head = rev_parse(&worktree.path, "HEAD^{commit}")?;
    if observed_head != expected_commit {
        return Err(OpsError::Message(format!(
            "publisher worktree {} materialized branch {branch} at {observed_head}, expected {expected_commit}; checkout and branch retained for inspection before retry",
            path.display()
        )));
    }
    Ok(worktree)
}

fn publisher_path_is_empty(path: &Path) -> OpsResult<bool> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_dir() {
        return Ok(false);
    }
    Ok(fs::read_dir(path)?.next().is_none())
}

fn publisher_worktree_refused(
    path: &Path,
    branch: &str,
    expected_commit: &str,
    observed: &str,
) -> OpsError {
    OpsError::Message(format!(
        "publisher worktree recovery refused for {}: expected branch {branch} at {expected_commit}; {observed}; no state was changed",
        path.display()
    ))
}

fn prepare_worktree_name(target: &ReleaseTarget, candidate: &ReleaseCandidate) -> String {
    let mut name =
        publish_worktree_name(target, &candidate.tag).replacen("publish-", "prepare-", 1);
    let revision = candidate.commit.get(..9).unwrap_or(&candidate.commit);
    name.push('-');
    name.push_str(revision);
    name
}

fn sanitize_ref_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect()
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

fn prepare_publisher(
    repo: &Path,
    candidate: &ReleaseCandidate,
    target: &ReleaseTarget,
    workflow: &ReleaseWorkflowResult,
    progress: &impl Progress,
    lock: &ReleaseLock,
) -> OpsResult<PathBuf> {
    let publisher = expand_publisher_command(repo, &target.publisher);
    let Some((program, args)) = publisher.split_first() else {
        return Err(OpsError::Message(
            "release candidate preparation requires a publisher".to_string(),
        ));
    };
    if workflow.database_id == 0 {
        return Err(OpsError::Message(format!(
            "release workflow for {} did not expose a run id",
            candidate.tag
        )));
    }

    let artifact_dir = publisher_artifact_dir(
        repo,
        &candidate.tag,
        &candidate.commit,
        workflow.database_id,
    );

    let run_id = workflow.database_id.to_string();
    progress.status(&format!(
        "Downloading candidate artifacts for {}...",
        candidate.tag
    ));
    let hosted_artifacts = super::read_retry::retry_read("candidate artifact download", || {
        let directory = tempfile::tempdir()?;
        super::read_retry::bounded_output(
            Command::new("gh")
                .current_dir(repo)
                .args(["run", "download", &run_id, "--dir"])
                .arg(directory.path()),
            // This downloads the complete multi-platform set. A healthy transfer
            // at 200 KiB/s already needs over six minutes for today's artifacts.
            std::time::Duration::from_secs(900),
        )?;
        Ok(directory)
    })?;

    let wt_name = prepare_worktree_name(target, candidate);
    let wt_path = worktree_path(repo, &wt_name);
    let lease = acquire_worktree_lease(
        repo,
        &wt_path,
        &format!("release candidate preparation for {}", candidate.tag),
    )?;
    progress.status(&format!("Preparing signed candidate {}...", candidate.tag));
    let wt = materialize_exact_source_worktree(repo, &wt_name, &candidate.commit, &lease, lock)?;
    let prepare_result = {
        let mut cmd = lock.command(program);
        lease.inherit(&mut cmd);
        cmd.args(args)
            .arg("prepare")
            .arg("--tag")
            .arg(&candidate.tag)
            .arg("--artifacts")
            .arg(hosted_artifacts.path())
            .arg("--output")
            .arg(&artifact_dir)
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
    cleanup_release_worktree(repo, &wt.path, &wt.branch, lease, progress, lock);
    prepare_result?;

    if !artifact_dir.join("candidate.json").is_file() {
        return Err(OpsError::Message(format!(
            "publisher prepared {} without writing candidate.json",
            candidate.tag
        )));
    }
    Ok(artifact_dir)
}

fn publisher_artifact_dir(repo: &Path, tag: &str, commit: &str, run_id: u64) -> PathBuf {
    repo.join(".lf/releases")
        .join(sanitize_ref_segment(tag))
        .join(format!("{commit}-{run_id}"))
}

fn delete_prepared_artifacts(path: &Path, progress: &impl Progress) {
    if let Err(error) = fs::remove_dir_all(path) {
        progress.warning(&format!(
            "Release completed, but prepared artifacts at {} could not be removed: {error}",
            path.display()
        ));
    }
}

fn run_publisher(
    repo: &Path,
    tag: &str,
    target: &ReleaseTarget,
    workflow: &ReleaseWorkflowResult,
    artifact_dir: &Path,
    progress: &impl Progress,
    lock: &ReleaseLock,
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

    progress.status(&format!(
        "Materializing tagged publisher worktree {wt_name}..."
    ));
    let wt = materialize_exact_source_worktree(repo, &wt_name, tag, &lease, lock)?;
    let publish_result = {
        let mut cmd = lock.command(program);
        lease.inherit(&mut cmd);
        cmd.args(args)
            .arg("publish")
            .arg("--tag")
            .arg(tag)
            .arg("--artifacts")
            .arg(artifact_dir)
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
    cleanup_release_worktree(repo, &wt.path, &wt.branch, lease, progress, lock);
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
    changes: &ReleaseChangeSet,
    target: &ReleaseTarget,
    progress: &impl Progress,
    lock: &ReleaseLock,
    lease: &WorktreeLease,
) -> OpsResult<PreparedRelease> {
    // Release preparation owns its branch independently of whichever Work
    // launched the controller. Provider/account authority remains available.
    let _context = ReleaseWorktreeContext::enter();
    progress.status(&format!(
        "Bumping manifests for {}...",
        target_tag(target, version)
    ));
    bump_manifest_versions(wt_path, target, version, progress, &|command| {
        lock.inherit(command);
        lease.inherit(command);
    })?;

    if !target.prepare.is_empty() {
        progress.status("Running repository release preparation...");
        run_release_hooks(
            wt_path,
            &target.prepare,
            target,
            Some(version),
            changes.previous_tag.as_deref(),
            "preparation",
            lock,
            lease,
        )?;
    }

    progress.status(&format!(
        "Generating release notes for {}...",
        target_tag(target, version)
    ));
    run_release_notes_stage(
        wt_path,
        version,
        changes.previous_tag.as_deref(),
        &changes.commits,
        &changes.merged_prs,
        target,
        progress,
        &|command| {
            lock.inherit(command);
            lease.inherit(command);
        },
    )?;

    progress.status("Committing release changes...");
    let _ = commit_workflow(
        wt_path,
        &CommitOptions {
            add: true,
            push: true,
            create_draft_pr: false,
            message: Some(release_commit_message(target, version)),
            ..CommitOptions::for_task("release")
        },
        progress,
        &|command| {
            lock.inherit(command);
            lease.inherit(command);
        },
    )?;

    progress.status("Enqueuing release PR for merge...");
    let pr_copy = release_pr_copy(wt_path, target, version)?;
    let options = release_land_options(Some(pr_copy));
    let pr = finish_arm_after_sync(wt_path, &options, progress, &|command| {
        lock.inherit(command);
        lease.inherit(command);
    })?
    .ok_or_else(|| {
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

/// The repository identity and PR number Loopflow records landings under.
fn landing_pr(repo: &Path, pr_number: u64) -> OpsResult<(crate::repository::RepoId, u32)> {
    let repo_id = crate::repository::RepoId::discover(repo)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let number = u32::try_from(pr_number).map_err(|_| {
        OpsError::Message(format!("pull request #{pr_number} exceeds supported range"))
    })?;
    Ok((repo_id, number))
}

/// Running the release again is how a person resumes after their own dequeue
/// or disable, and arms a PR nothing has explained. Repair, eviction and merge
/// removals stay with the shared merge decision.
fn arm_resumed_release(
    repo: &Path,
    pr_number: u64,
    head_sha: &str,
    progress: &impl Progress,
    lock: &ReleaseLock,
) -> OpsResult<()> {
    if crate::ops::pr::observe_merge_request(repo, pr_number)?.is_some() {
        return Ok(());
    }
    let (repo_id, number) = landing_pr(repo, pr_number)?;
    if !matches!(
        crate::ops::pr_landing::read_merge_request_removal(repo, repo_id.as_str(), number)?,
        crate::ops::pr::MergeRequestRemoval::Manual(_)
            | crate::ops::pr::MergeRequestRemoval::Unknown
    ) {
        return Ok(());
    }
    progress.status(&format!(
        "Re-arming release PR #{pr_number} for exact-head auto-merge..."
    ));
    crate::ops::pr::enable_auto_merge(repo, pr_number, None, head_sha, &|command| {
        lock.inherit(command)
    })
}

fn release_land_options(copy: Option<PrCopy>) -> LandOptions {
    LandOptions {
        strict: true,
        local: false,
        wait_and_fix: false,
        worktree: None,
        commit_message: None,
        pr_title: copy.as_ref().map(|copy| copy.title.clone()),
        pr_body: copy.map(|copy| copy.body),
        agent: None,
    }
}

fn finish_release_pr(
    main_repo: &Path,
    worktree_name: &str,
    mut prepared: PreparedRelease,
    target: &ReleaseTarget,
    version: &str,
    progress: &impl Progress,
    lock: &ReleaseLock,
) -> OpsResult<String> {
    let release_branch = release_branch_name(main_repo, worktree_name)?;
    loop {
        match wait_for_pr_merge(main_repo, prepared.pr_number, &prepared.head_sha, progress)? {
            ReleasePrWait::Merged(commit) => {
                if let Some((path, _)) = list_porcelain(main_repo)?
                    .into_iter()
                    .find(|(_, branch)| branch.as_deref() == Some(&release_branch))
                {
                    match acquire_worktree_lease_wait(
                        main_repo,
                        &path,
                        "settled release cleanup",
                        Duration::from_secs(5),
                    ) {
                        Ok(lease) => {
                            let pr =
                                crate::ops::pr::observe_pr_merge(&path, prepared.pr_number)?.pr;
                            crate::ops::pr_landing::reconcile_armed_pr(
                                &path,
                                &release_land_options(None),
                                &pr,
                                lock,
                                &lease,
                            )?;
                            cleanup_release_worktree(
                                main_repo,
                                &path,
                                &release_branch,
                                lease,
                                progress,
                                lock,
                            );
                        }
                        Err(error) => progress.warning(&format!(
                            "Retaining release checkout {}: {error}",
                            path.display()
                        )),
                    }
                }
                return Ok(commit);
            }
            ReleasePrWait::NeedsIntegration => {
                progress.status(&format!(
                    "Release PR #{} needs current main; rebuilding on it...",
                    prepared.pr_number
                ));
                fetch_release_branch(main_repo, &release_branch, &prepared.head_sha, lock)?;
                let default_branch = get_default_branch(main_repo)?;
                let main_ref = format!("origin/{default_branch}");
                run_locked_stdout(
                    lock,
                    main_repo,
                    "git",
                    &[
                        "fetch",
                        "origin",
                        &format!(
                            "+refs/heads/{default_branch}:refs/remotes/origin/{default_branch}"
                        ),
                    ],
                )?;
                let lease = acquire_worktree_lease(
                    main_repo,
                    &worktree_path(main_repo, worktree_name),
                    "release PR rebuild",
                )?;
                let wt = materialize_exact_source_worktree(
                    main_repo,
                    worktree_name,
                    &prepared.head_sha,
                    &lease,
                    lock,
                )?;
                let refreshed = (|| {
                    let mut reset = lock.command("git");
                    reset
                        .current_dir(&wt.path)
                        .args(["reset", "--hard", &main_ref]);
                    lease.inherit(&mut reset);
                    command_stdout(&mut reset)?;
                    // Regenerate from main while retaining the published release
                    // ancestry, so preparation can push without rewriting history.
                    let mut merge = lock.command("git");
                    merge.current_dir(&wt.path).args([
                        "merge",
                        "--no-commit",
                        "--no-ff",
                        "--strategy=ours",
                        &prepared.head_sha,
                    ]);
                    lease.inherit(&mut merge);
                    command_stdout(&mut merge)?;
                    let changes = collect_version_changes(main_repo, target, version, &main_ref)?;
                    prepare_release_in_worktree(
                        &wt.path, version, &changes, target, progress, lock, &lease,
                    )
                })();
                cleanup_release_worktree(main_repo, &wt.path, &wt.branch, lease, progress, lock);
                prepared = match refreshed {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        let observed =
                            crate::ops::pr::observe_pr_merge(main_repo, prepared.pr_number)?;
                        if observed.pr.state == "merged" {
                            continue;
                        }
                        return Err(error);
                    }
                };
            }
            ReleasePrWait::NeedsRepair(head_sha) => {
                progress.status(&format!(
                    "Release PR #{} needs a CI repair; reconciling it...",
                    prepared.pr_number
                ));
                prepared.head_sha = head_sha;
                fetch_release_branch(main_repo, &release_branch, &prepared.head_sha, lock)?;
                let lease = acquire_worktree_lease_wait(
                    main_repo,
                    &worktree_path(main_repo, worktree_name),
                    "release CI repair",
                    Duration::from_secs(5),
                )?;
                let wt = materialize_exact_source_worktree(
                    main_repo,
                    worktree_name,
                    &prepared.head_sha,
                    &lease,
                    lock,
                )?;
                let pr = current_pr(&wt.path)?.ok_or_else(|| {
                    OpsError::Message(format!("release PR #{} is unavailable", prepared.pr_number))
                })?;
                let options = release_land_options(None);
                let _context = ReleaseWorktreeContext::enter();
                match crate::ops::pr_landing::reconcile_armed_pr(
                    &wt.path, &options, &pr, lock, &lease,
                ) {
                    Ok(landing) => {
                        wait_for_release_repair(&landing)?;
                        if let Some(commit) = landing.merge_commit {
                            cleanup_release_worktree(
                                main_repo, &wt.path, &wt.branch, lease, progress, lock,
                            );
                            return Ok(commit);
                        }
                        if let Some(head) = current_pr(&wt.path)?.and_then(|pr| pr.head_sha) {
                            prepared.head_sha = head;
                        }
                    }
                    Err(error) => {
                        // Main can advance during repair. Release preparation owns
                        // rebuilding version metadata; the finite reconciler owns CI repair.
                        let view =
                            crate::ops::pr::observe_pr_merge(main_repo, prepared.pr_number)?.pr;
                        // Repair can finish after reconciliation read failing checks.
                        // Re-enter merge settlement when fresh evidence has overtaken them.
                        if view.state == "merged" {
                            continue;
                        }
                        if matches!(view.merge_state.as_deref(), Some("behind" | "dirty")) {
                            if let Some(pr) = current_pr(&wt.path)? {
                                if let Some(head) = pr.head_sha {
                                    prepared.head_sha = head;
                                }
                            }
                            continue;
                        }
                        // Retain the repair checkout on a real block or provider
                        // failure so re-entry can continue its authored work.
                        return Err(error);
                    }
                }
            }
        }
    }
}

fn wait_for_release_repair(landing: &crate::pr_landing::PrLanding) -> OpsResult<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let store = runtime.block_on(crate::ops::pr_landing::landing_store())?;
    let deadline = Instant::now() + Duration::from_secs(60 * 60);
    while crate::ops::pr_landing::repair_running(&store, &landing.repo, landing.pr_number)? {
        if Instant::now() >= deadline {
            return Err(OpsError::ReleaseDeferred {
                reason: format!(
                    "release PR #{} still has an active CI repair",
                    landing.pr_number
                ),
                continuation: format!(
                    "observe landing {} and retry after its repair Process exits",
                    landing.id
                ),
            });
        }
        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn fetch_release_branch(
    repo: &Path,
    branch: &str,
    expected_head: &str,
    lock: &ReleaseLock,
) -> OpsResult<()> {
    let remote_ref = format!("refs/remotes/origin/{branch}");
    let refspec = format!("+refs/heads/{branch}:{remote_ref}");
    fetch_inheriting(repo, "origin", &refspec, &|command| lock.inherit(command))?;
    let remote_head = crate::git::rev_parse(repo, &remote_ref)?;
    if remote_head != expected_head {
        return Err(OpsError::Message(format!(
            "release PR head changed while recovery was fetching it: expected {expected_head}, found {remote_head}"
        )));
    }
    Ok(())
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
    /// Release-cut lines that every reader must see, such as
    /// `lf < 0.15.0 can no longer open the store`; finalized notes carry them verbatim.
    breaking: Vec<String>,
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

#[allow(clippy::too_many_arguments)]
fn run_release_notes_stage(
    repo: &Path,
    version: &str,
    prev_tag: Option<&str>,
    commits: &[ReleaseCommit],
    merged_prs: &[MergedPr],
    target: &ReleaseTarget,
    progress: &impl Progress,
    inherit_notes: &impl Fn(&mut Command),
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
        let (breaking, _) = read_bounded_text(
            &breaking_path(repo, version),
            RELEASE_CONTEXT_MAX_BREAKING_BYTES,
        )?;
        let (context, context_json) = build_release_notes_context(
            version,
            prev_tag,
            commits,
            merged_prs,
            target,
            decisions,
            decisions_omitted,
            breaking_lines(breaking.as_deref()),
            previous_notes,
            previous_notes_omitted,
        )?;

        generate_notes_file(repo, repo, &context, &context_json, progress, inherit_notes)?;
        archive_release_notes(repo, version)?;
        Ok(())
    })();

    if result.is_err() {
        restore_release_notes(&notes_path, previous_notes_file.as_ref())?;
    }
    result
}

fn generate_notes_file(
    repo: &Path,
    output: &Path,
    context: &ReleaseNotesContext,
    context_json: &[u8],
    progress: &impl Progress,
    inherit_notes: &impl Fn(&mut Command),
) -> OpsResult<()> {
    progress.status(&format!(
        "Notes input: {} commits, {} PRs; omitted {} commits, {} PRs, {} file paths, {} text bytes",
        context.commits.len(),
        context.merged_prs.len(),
        context.omissions.commits,
        context.omissions.merged_prs,
        context.omissions.commit_files + context.omissions.pr_files,
        context.omissions.text_bytes
    ));
    let context_path = write_prompt_log(
        output,
        std::str::from_utf8(context_json).expect("serialized JSON is UTF-8"),
        &format!("release-notes-context-{}", uuid::Uuid::new_v4()),
        None,
    )?;

    let mut cmd = Command::new("lf");
    cmd.args(["--batch"])
        .arg("release-notes")
        .arg(format!(
            "Write notes only to {} (LF_RELEASE_NOTES_OUTPUT). Do not modify any other file, \
             publish a release, or run git mutations. Use only LF_RELEASE_NOTES_CONTEXT as evidence. \
             For cycle notes, reconcile later changes with earlier ones and describe the final behavior. \
             Features in this range may already have shipped in patches.",
            output.join("RELEASE_NOTES.md").display()
        ))
        .current_dir(repo)
        .env("LF_RELEASE_NOTES_CONTEXT", &context_path)
        .env("LF_RELEASE_NOTES_OUTPUT", output.join("RELEASE_NOTES.md"));
    inherit_notes(&mut cmd);
    let degradation = match run_command(&mut cmd) {
        Ok(_) => None,
        Err(err) => match classify_release_notes_degradation(&err) {
            Some(degradation) => {
                let notes = generate_release_notes(context)?;
                write_release_notes(output, &notes, &context.version)?;
                Some(degradation)
            }
            None => {
                return Err(OpsError::Message(format!(
                    "release gate blocked: release-notes agent failed outside the supported provider-degradation policy: {err}"
                )));
            }
        },
    };

    finalize_release_notes(output, &context.version, &context.breaking, degradation)?;
    match degradation {
        None => progress.status("Release notes: narrative; release gate safe."),
        Some(reason) => progress.warning(&format!(
            "Release notes: degraded ({reason}); deterministic fallback keeps the release gate safe."
        )),
    }
    Ok(())
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
    breaking: Vec<String>,
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
        breaking,
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

    let mut files_per_change = RELEASE_CONTEXT_MAX_FILES_PER_CHANGE;
    let mut body_bytes = RELEASE_CONTEXT_MAX_PR_BODY_BYTES;
    loop {
        let json = serde_json::to_vec(&context).map_err(|err| {
            OpsError::Parse(format!("failed to encode release-notes context: {err}"))
        })?;
        if json.len() <= RELEASE_CONTEXT_MAX_BYTES {
            return Ok((context, json));
        }
        // A cycle overview needs its latest changes as much as its earliest.
        // Reduce per-change detail before dropping entire commits or PRs.
        if files_per_change > 1 {
            files_per_change = (files_per_change / 2).max(1);
            for commit in &mut context.commits {
                context.omissions.commit_files +=
                    commit.files.len().saturating_sub(files_per_change);
                commit.files.truncate(files_per_change);
            }
            for pr in &mut context.merged_prs {
                context.omissions.pr_files += pr.files.len().saturating_sub(files_per_change);
                pr.files.truncate(files_per_change);
            }
            continue;
        }
        if body_bytes > 128 {
            body_bytes /= 2;
            for pr in &mut context.merged_prs {
                if let Some(body) = &mut pr.body {
                    *body = bound_text(body, body_bytes, &mut context.omissions.text_bytes);
                }
            }
            continue;
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

fn read_bounded_revision_text(
    repo: &Path,
    reference: &str,
    max_bytes: usize,
) -> OpsResult<(Option<String>, usize)> {
    if !run_output(repo, "git", &["cat-file", "-e", reference])?
        .status
        .success()
    {
        return Ok((None, 0));
    }
    let value = run_stdout(repo, "git", &["show", reference])?;
    let mut omitted = 0;
    Ok((Some(bound_text(&value, max_bytes, &mut omitted)), omitted))
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
    breaking: &[String],
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
    let missing: Vec<String> = breaking
        .iter()
        .filter(|line| !body.contains(line.as_str()))
        .map(|line| format!("**{line}**\n\n"))
        .collect();
    let content = format!(
        "{expected_header}\n\n{marker}\n\n{}{body}",
        missing.concat()
    );
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
            let incoming = fs::read_to_string(entry.path())?;
            let existing = fs::read_to_string(&dest)?;
            if incoming == existing {
                fs::remove_file(entry.path())?;
                continue;
            }
            // A repair cut under the same version adds its own compatibility lines.
            if entry.file_name() == "BREAKING.md" {
                let mut merged = existing.clone();
                for line in incoming.lines() {
                    if !existing.lines().any(|known| known == line) {
                        if !merged.is_empty() && !merged.ends_with('\n') {
                            merged.push('\n');
                        }
                        merged.push_str(line);
                        merged.push('\n');
                    }
                }
                write_atomic(&dest, merged.as_bytes())?;
                fs::remove_file(entry.path())?;
                continue;
            }
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
    lease: WorktreeLease,
    progress: &impl Progress,
    lock: &ReleaseLock,
) {
    // A repair's completion receipt can precede process exit. Reacquire
    // independently and allow its inherited descriptors a bounded time to close.
    drop(lease);
    let removal = (|| {
        let lease = acquire_worktree_lease_wait(
            main_repo,
            wt_path,
            "release worktree cleanup",
            Duration::from_secs(5),
        )?;
        let inherit = |command: &mut Command| {
            lock.inherit(command);
            lease.inherit(command);
        };
        worktree_remove_owned(main_repo, wt_path, true, &lease, &inherit)?;
        let _ = delete_local_branch_inheriting(main_repo, branch, &inherit);
        Ok::<_, crate::error::GitError>(())
    })();
    if let Err(err) = removal {
        progress.error(&format!(
            "Warning: could not remove release worktree {}: {err}",
            wt_path.display()
        ));
    }
}

fn release_commit_message(target: &ReleaseTarget, version: &str) -> String {
    if target.name == "default" {
        format!("release: v{version}")
    } else {
        format!("release: {} v{version}", target.name)
    }
}

/// A release cut records lines every reader of the release must see here,
/// one per line; `release/unreleased/` becomes this directory at the notes stage.
fn breaking_path(repo: &Path, version: &str) -> PathBuf {
    repo.join("release")
        .join(format!("v{version}"))
        .join("BREAKING.md")
}

fn breaking_lines(text: Option<&str>) -> Vec<String> {
    text.unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn release_pr_copy(repo: &Path, target: &ReleaseTarget, version: &str) -> OpsResult<PrCopy> {
    let (breaking, _) = read_bounded_text(
        &breaking_path(repo, version),
        RELEASE_CONTEXT_MAX_BREAKING_BYTES,
    )?;
    let mut title = release_commit_message(target, version);
    // The title names the break; the notes name what raised it.
    if let Some(line) = breaking_lines(breaking.as_deref()).last() {
        let line = line.split(" (raised by:").next().unwrap_or(line);
        title = format!("{title}: {line}");
    }
    Ok(PrCopy {
        title,
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
            "number,state,mergeCommit,headRefOid",
        ],
    )?;
    let prs: Vec<GhReleasePr> = serde_json::from_str(&output)
        .map_err(|err| OpsError::Parse(format!("failed to parse release PR: {err}")))?;
    Ok(current_release_pr(prs))
}

/// A release PR closed without merging is a withdrawn attempt. GitHub keeps it
/// under the branch name forever, so it must not stand in for this version's
/// release: the open or merged PR does, and with neither the release starts fresh.
fn current_release_pr(prs: Vec<GhReleasePr>) -> Option<GhReleasePr> {
    prs.into_iter().find(|pr| pr.state != "CLOSED")
}

#[derive(Debug, PartialEq, Eq)]
enum ReleasePrWait {
    Merged(String),
    NeedsIntegration,
    /// The shared merge decision found a failure on this head for ci-fix.
    NeedsRepair(String),
}

/// Release follows the landing's merge decision; `None` keeps polling.
fn release_wait(
    pr: &crate::ops::pr::PrInfo,
    observation: crate::ops::pr_landing::LandingObservation,
) -> OpsResult<Option<ReleasePrWait>> {
    use crate::ops::pr_landing::LandingObservation;
    match observation {
        LandingObservation::Merged { merge_commit, .. } => {
            Ok(Some(ReleasePrWait::Merged(merge_commit)))
        }
        LandingObservation::Closed { .. } => Err(OpsError::Message(format!(
            "{} was closed without merging",
            pr.url
        ))),
        LandingObservation::Withdrawn { removal, .. } => Err(OpsError::ManualMergeRemoval {
            pr_number: pr.number,
            actor: removal.actor,
            at: removal.at,
            queued: removal.queued,
        }),
        LandingObservation::Failing { head_sha, .. } => {
            Ok(Some(ReleasePrWait::NeedsRepair(head_sha)))
        }
        LandingObservation::Pending { .. } | LandingObservation::Passing { .. } => Ok(None),
    }
}

fn wait_for_pr_merge(
    repo: &Path,
    pr_number: u64,
    head_sha: &str,
    progress: &impl Progress,
) -> OpsResult<ReleasePrWait> {
    let (repo_id, number) = landing_pr(repo, pr_number)?;
    let started = Instant::now();
    let timeout = Duration::from_secs(60 * 60);
    let poll = Duration::from_secs(10);
    let mut attempt: u64 = 0;

    loop {
        let observation = crate::ops::pr::observe_pr_merge(repo, pr_number)?;
        let view = &observation.pr;
        let removal =
            || crate::ops::pr_landing::read_merge_request_removal(repo, repo_id.as_str(), number);

        if view.state == "open" && crate::ops::pr::pr_requires_resync(&observation, removal)? {
            return Ok(ReleasePrWait::NeedsIntegration);
        }

        let decided =
            crate::ops::pr_landing::decide_merge(&observation, head_sha, removal, |head| {
                merge_gate_state(repo, pr_number, head)
            })?;
        if let Some(wait) = release_wait(view, decided)? {
            return Ok(wait);
        }

        if started.elapsed() >= timeout {
            return Err(OpsError::ReleaseDeferred {
                reason: format!("release PR #{pr_number} is still awaiting merge"),
                continuation: format!("rerun `lf release run` to resume PR #{pr_number}"),
            });
        }

        if attempt.is_multiple_of(6) {
            progress.status(&format!(
                "PR #{pr_number} is open ({}) and awaiting GitHub auto-merge...",
                view.merge_state.as_deref().unwrap_or("unknown")
            ));
        }
        attempt += 1;
        thread::sleep(poll);
    }
}

fn wait_for_candidate_workflow(
    repo: &Path,
    candidate: &ReleaseCandidate,
    target: &ReleaseTarget,
    progress: &impl Progress,
    lock: &ReleaseLock,
) -> OpsResult<ReleaseWorkflowResult> {
    let Some(workflow_name) = target.workflow.as_deref() else {
        return Ok(ReleaseWorkflowResult {
            database_id: 0,
            url: None,
        });
    };

    ensure_candidate_ref(repo, candidate, lock)?;
    if find_workflow_run(repo, candidate, target)?.is_none() {
        run_locked_stdout(
            lock,
            repo,
            "gh",
            &[
                "workflow",
                "run",
                workflow_name,
                "--ref",
                &candidate.branch,
                "-f",
                &format!("tag={}", candidate.tag),
            ],
        )?;
    }

    wait_for_release_workflow(repo, candidate, target, progress, true)
}

fn wait_for_release_workflow(
    repo: &Path,
    candidate: &ReleaseCandidate,
    target: &ReleaseTarget,
    progress: &impl Progress,
    accept_build_success: bool,
) -> OpsResult<ReleaseWorkflowResult> {
    if target.publisher.is_empty()
        && target.workflow.is_none()
        && target.completion == ReleaseCompletion::Tag
    {
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
        let workflow = find_workflow_run(repo, candidate, target)?;
        let release_exists = github_release_exists(repo, &candidate.tag)?;

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
                    if accept_build_success
                        || !target.publisher.is_empty()
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
                        "release workflow failed for {}: {conclusion} ({url})",
                        candidate.tag
                    )));
                }
            }
        } else if !saw_workflow
            && target.workflow.is_none()
            && started.elapsed() >= no_workflow_grace
        {
            progress.status(&format!(
                "No release workflow detected for {}; tag push complete",
                candidate.tag
            ));
            return Ok(ReleaseWorkflowResult {
                database_id: 0,
                url: None,
            });
        }

        if started.elapsed() >= timeout {
            return Err(OpsError::ReleaseDeferred {
                reason: format!(
                    "waiting for {:?} completion for {}",
                    target.completion, candidate.tag
                ),
                continuation: format!(
                    "rerun `lf release run` to resume exact candidate {} at {}",
                    candidate.tag, candidate.commit
                ),
            });
        }

        if attempt.is_multiple_of(6) {
            progress.status(&format!(
                "Release workflow for {} still running...",
                candidate.tag
            ));
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

    let candidate = release_candidate_for_tag(repo, tag, target)?;
    let Some(run) = find_workflow_run(repo, &candidate, target)? else {
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
    let latest_tag = latest_tag_optional(repo, target)?;

    let version = resolve_version(latest_tag.as_deref(), version_input, target)?;
    let prev_tag = notes_base(repo, &version, target)?;

    progress.status("Collecting release changes...");
    let changes = collect_changes_between(repo, prev_tag.as_deref(), "HEAD", target)?;

    progress.status("Generating release notes...");
    let (context, _) = build_release_notes_context(
        &version,
        prev_tag.as_deref(),
        &changes.commits,
        &changes.merged_prs,
        target,
        None,
        0,
        Vec::new(),
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

/// Minor notes cover the whole cycle since the preceding `.0`; other versions
/// cover changes since the latest tag.
fn notes_base(repo: &Path, version: &str, target: &ReleaseTarget) -> OpsResult<Option<String>> {
    let (major, minor, patch) = parse_version(version)
        .ok_or_else(|| OpsError::Parse(format!("invalid version format: {version}")))?;
    let latest = latest_tag_optional(repo, target)?;
    if patch != 0 || minor == 0 || latest.is_none() {
        return Ok(latest);
    }
    let tag = target_tag(target, &format!("{major}.{}.0", minor - 1));
    if !ref_exists(repo, &format!("refs/tags/{tag}"))? {
        return Err(OpsError::Message(format!(
            "minor release notes require baseline tag {tag}; supply --prev-tag for a notes-only override"
        )));
    }
    Ok(Some(tag))
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
    let base = format!("origin/{}", get_default_branch(repo)?);
    let previous_tag = latest_tag_optional(repo, target)?;
    collect_changes_between(repo, previous_tag.as_deref(), &base, target)
}

fn collect_version_changes(
    repo: &Path,
    target: &ReleaseTarget,
    version: &str,
    source: &str,
) -> OpsResult<ReleaseChangeSet> {
    let base = notes_base(repo, version, target)?;
    collect_changes_between(repo, base.as_deref(), source, target)
}

fn collect_changes_between(
    repo: &Path,
    previous_tag: Option<&str>,
    revision: &str,
    target: &ReleaseTarget,
) -> OpsResult<ReleaseChangeSet> {
    let commits = release_commits_between(repo, previous_tag, revision, target)?;
    let commit_shas = commits
        .iter()
        .map(|commit| commit.sha.as_str())
        .collect::<HashSet<_>>();
    let merged_prs = merged_prs_between(repo, previous_tag, revision, target, &commit_shas)?;

    Ok(ReleaseChangeSet {
        previous_tag: previous_tag.map(str::to_string),
        commits,
        merged_prs,
    })
}

fn release_commits_between(
    repo: &Path,
    previous_tag: Option<&str>,
    revision: &str,
    target: &ReleaseTarget,
) -> OpsResult<Vec<ReleaseCommit>> {
    let range = previous_tag
        .map(|tag| format!("{tag}..{revision}"))
        .unwrap_or_else(|| revision.to_string());
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

fn merged_prs_between(
    repo: &Path,
    previous_tag: Option<&str>,
    revision: &str,
    target: &ReleaseTarget,
    commit_shas: &HashSet<&str>,
) -> OpsResult<Vec<MergedPr>> {
    let ending_at = run_stdout(repo, "git", &["log", "-1", "--format=%cI", revision])?;
    // GitHub records mergedAt after the merge commit's timestamp. Include that
    // whole day, then use commit membership to select the exact range.
    let ending_date = ending_at.split('T').next().unwrap_or(&ending_at);
    let upper_bound = format!("merged:<={ending_date}");
    let search = match previous_tag {
        Some(tag) => {
            let tagged_at = run_stdout(repo, "git", &["log", "-1", "--format=%aI", tag])?;
            if tagged_at.is_empty() {
                return Err(OpsError::Message(format!(
                    "could not determine merge date for tag {tag}"
                )));
            }
            let date = tagged_at.split('T').next().unwrap_or(&tagged_at);
            Some(format!("merged:>={date} {upper_bound}"))
        }
        None => Some(upper_bound),
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
    let limit = RELEASE_QUERY_PR_LIMIT.to_string();
    args.extend(["--json", fields, "--limit", &limit]);
    let output = run_stdout(repo, "gh", &args)?;

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
    inherit_tools: &impl Fn(&mut Command),
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
    for (bumped, lockfile, program, args) in [
        (
            bumped_cargo,
            "Cargo.lock",
            "cargo",
            &["update", "--workspace"][..],
        ),
        (bumped_pyproject, "uv.lock", "uv", &["lock"][..]),
    ] {
        if bumped && repo.join(lockfile).exists() {
            let mut command = Command::new(program);
            command.args(args).current_dir(repo);
            inherit_tools(&mut command);
            command_stdout(&mut command)?;
        }
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

fn tag_and_push_ref(
    repo: &Path,
    version: &str,
    target: &ReleaseTarget,
    target_ref: Option<&str>,
    lock: &ReleaseLock,
) -> OpsResult<String> {
    let tag = target_tag(target, version);
    let ref_name = target_ref.unwrap_or("HEAD");
    let target_sha = run_stdout(repo, "git", &["rev-parse", ref_name])?;
    let target_sha = target_sha.trim().to_string();
    ensure_commit_local(repo, &target_sha, lock)?;

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
            run_locked_stdout(lock, repo, "git", &["tag", &tag, ref_name])?;
        }
    }

    let push_output = lock
        .command("git")
        .args(["push", "origin", &tag])
        .current_dir(repo)
        .output()?;
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

fn release_candidate_for_tag(
    repo: &Path,
    tag: &str,
    target: &ReleaseTarget,
) -> OpsResult<ReleaseCandidate> {
    let commit = local_tag_sha(repo, tag)?
        .ok_or_else(|| OpsError::Message(format!("release tag {tag} is not present locally")))?;
    Ok(ReleaseCandidate::new(target, tag, &commit))
}

fn ensure_candidate_ref(
    repo: &Path,
    candidate: &ReleaseCandidate,
    lock: &ReleaseLock,
) -> OpsResult<()> {
    ensure_commit_local(repo, &candidate.commit, lock)?;
    if let Some(remote_sha) = remote_branch_sha(repo, &candidate.branch)? {
        if remote_sha == candidate.commit {
            return Ok(());
        }
        return Err(OpsError::Message(format!(
            "release candidate {} exists at {remote_sha}, expected {}",
            candidate.branch, candidate.commit
        )));
    }

    let refspec = format!("{}:refs/heads/{}", candidate.commit, candidate.branch);
    run_locked_stdout(lock, repo, "git", &["push", "origin", &refspec])?;
    Ok(())
}

fn delete_candidate_ref(repo: &Path, candidate: &ReleaseCandidate, lock: &ReleaseLock) {
    let Ok(Some(remote_sha)) = remote_branch_sha(repo, &candidate.branch) else {
        return;
    };
    if remote_sha != candidate.commit {
        eprintln!(
            "Candidate ref {} moved to {remote_sha}; leaving it untouched",
            candidate.branch
        );
        return;
    }
    let output = lock
        .command("git")
        .args(["push", "origin", "--delete", &candidate.branch])
        .current_dir(repo)
        .output();
    if !matches!(output, Ok(output) if output.status.success()) {
        eprintln!(
            "Release completed, but candidate ref {} could not be removed",
            candidate.branch
        );
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
fn ensure_commit_local(repo: &Path, sha: &str, lock: &ReleaseLock) -> OpsResult<()> {
    if commit_exists_locally(repo, sha) {
        return Ok(());
    }
    let _ = lock
        .command("git")
        .args(["fetch", "origin"])
        .current_dir(repo)
        .output();
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

fn remote_branch_sha(repo: &Path, branch: &str) -> OpsResult<Option<String>> {
    let ref_name = format!("refs/heads/{branch}");
    let output = run_output(repo, "git", &["ls-remote", "--heads", "origin", &ref_name])?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("git ls-remote --heads origin {ref_name}"),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().next())
        .map(str::to_string))
}

fn find_workflow_run(
    repo: &Path,
    candidate: &ReleaseCandidate,
    target: &ReleaseTarget,
) -> OpsResult<Option<GhRunListEntry>> {
    let runs = list_workflow_runs(repo, target)?;

    let matching = runs
        .iter()
        .find(|run| {
            run.head_branch.as_deref() == Some(&candidate.branch)
                && run.head_sha.as_deref() == Some(&candidate.commit)
        })
        .cloned()
        .or_else(|| {
            runs.iter()
                .find(|run| {
                    run.head_branch.as_deref() == Some(&candidate.tag)
                        && run
                            .head_sha
                            .as_deref()
                            .is_none_or(|sha| sha == candidate.commit)
                })
                .cloned()
        })
        .or_else(|| {
            runs.into_iter().find(|run| {
                run.display_title
                    .as_deref()
                    .is_some_and(|title| title.contains(&candidate.tag))
                    && run
                        .head_sha
                        .as_deref()
                        .is_none_or(|sha| sha == candidate.commit)
            })
        });

    Ok(matching)
}

fn find_latest_candidate_workflow(
    repo: &Path,
    tag: &str,
    target: &ReleaseTarget,
) -> OpsResult<Option<GhRunListEntry>> {
    if target.workflow.is_none() {
        return Ok(None);
    }
    let prefix = format!(
        "release-candidate/{}/{}/",
        sanitize_ref_segment(&target.name),
        sanitize_ref_segment(tag)
    );
    Ok(list_workflow_runs(repo, target)?.into_iter().find(|run| {
        run.head_branch
            .as_deref()
            .is_some_and(|branch| branch.starts_with(&prefix))
            && run.head_sha.is_some()
    }))
}

fn list_workflow_runs(repo: &Path, target: &ReleaseTarget) -> OpsResult<Vec<GhRunListEntry>> {
    let mut args = vec![
        "run".to_string(),
        "list".to_string(),
        "--json".to_string(),
        "databaseId,headBranch,headSha,displayTitle,status,conclusion,url".to_string(),
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
    Ok(runs)
}

fn github_release_exists(repo: &Path, tag: &str) -> OpsResult<bool> {
    Ok(github_release_state(repo, tag)? == GitHubReleaseState::Published)
}

fn github_release_state(repo: &Path, tag: &str) -> OpsResult<GitHubReleaseState> {
    let output = run_output(repo, "gh", &["release", "view", tag, "--json", "isDraft"])?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr == "release not found" {
            return Ok(GitHubReleaseState::Missing);
        }
        return Err(OpsError::Message(format!(
            "cannot establish GitHub Release state for {tag}: {stderr}"
        )));
    }
    let view: GhReleaseView = serde_json::from_slice(&output.stdout)
        .map_err(|err| OpsError::Parse(format!("failed to parse GitHub Release state: {err}")))?;
    Ok(if view.is_draft {
        GitHubReleaseState::Draft
    } else {
        GitHubReleaseState::Published
    })
}

// -- Streams ------------------------------------------------------------------
//
// `preview` is the newest published release. `stable` is GitHub's Latest
// marker, which every install path reads (`releases/latest`): the release run
// moves it to the newest release that has soaked for `STABLE_SOAK`, and never
// back. `dev` is green main and has no pointer here.

const STABLE_SOAK: chrono::TimeDelta = chrono::TimeDelta::days(7);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhReleaseListEntry {
    tag_name: String,
    is_latest: bool,
    is_draft: bool,
    is_prerelease: bool,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug)]
struct PublishedRelease {
    tag: String,
    version: (u64, u64, u64),
    published_at: chrono::DateTime<chrono::Utc>,
    is_latest: bool,
}

fn published_releases(repo: &Path, target: &ReleaseTarget) -> OpsResult<Vec<PublishedRelease>> {
    let json = run_stdout(
        repo,
        "gh",
        &[
            "release",
            "list",
            "--limit",
            "100",
            "--json",
            "tagName,isLatest,isDraft,isPrerelease,publishedAt",
        ],
    )?;
    let entries: Vec<GhReleaseListEntry> = serde_json::from_str(&json)
        .map_err(|err| OpsError::Parse(format!("failed to parse GitHub Releases: {err}")))?;
    Ok(entries
        .into_iter()
        .filter(|entry| !entry.is_draft && !entry.is_prerelease)
        .filter_map(|entry| {
            let version = parse_version(&version_from_tag(&entry.tag_name, target).ok()?)?;
            Some(PublishedRelease {
                tag: entry.tag_name,
                version,
                published_at: entry.published_at?,
                is_latest: entry.is_latest,
            })
        })
        .collect())
}

/// A regression Task as the stable rule reads it: releases from `version` on
/// carry the bug, until the first release published after its fix landed.
#[derive(Debug)]
struct Regression {
    task: String,
    version: (u64, u64, u64),
    fixed_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Regression {
    fn covers(&self, release: &PublishedRelease) -> bool {
        release.version >= self.version
            && self
                .fixed_at
                .is_none_or(|fixed_at| release.published_at < fixed_at)
    }
}

/// The repository's regression Tasks. A done Task whose completion time is
/// unreadable keeps covering, so a stalled `stable` is the failure, not a bug.
fn regressions(repo: &Path) -> OpsResult<Vec<Regression>> {
    let runtime = tokio::runtime::Runtime::new()?;
    let store = runtime.block_on(crate::ops::planning_sync::planning_store())?;
    let tasks = store
        .sqlite
        .regression_tasks(&repo.to_string_lossy())
        .map_err(|error| OpsError::Message(format!("read regression Tasks: {error}")))?;
    Ok(tasks
        .into_iter()
        .filter(|task| task.state.as_deref() != Some("canceled"))
        .filter_map(|task| {
            let fixed_at = if task.state.as_deref() == Some("completed") {
                task.completed_at
                    .as_deref()
                    .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
                    .map(|at| at.with_timezone(&chrono::Utc))
            } else {
                None
            };
            Some(Regression {
                version: parse_version(task.regressed_in.as_deref()?)?,
                task: task.identifier,
                fixed_at,
            })
        })
        .collect())
}

/// The release `stable` should point at now, when that is ahead of where it
/// points: the newest soaked release no regression covers.
fn stable_advance<'a>(
    releases: &'a [PublishedRelease],
    regressions: &[Regression],
    now: chrono::DateTime<chrono::Utc>,
) -> Option<&'a PublishedRelease> {
    let current = releases
        .iter()
        .find(|release| release.is_latest)
        .map(|release| release.version);
    releases
        .iter()
        .filter(|release| release.published_at + STABLE_SOAK <= now)
        .filter(|release| {
            !regressions
                .iter()
                .any(|regression| regression.covers(release))
        })
        .max_by_key(|release| release.version)
        .filter(|candidate| current.is_none_or(|current| candidate.version > current))
}

/// Move `stable` forward. A configured publisher first points its own
/// stable-only artifacts (the Mac app download) at the release, so the marker
/// moves last and a failed promotion retries on the next run.
fn advance_stable(
    repo: &Path,
    target: &ReleaseTarget,
    progress: &impl Progress,
    lock: &ReleaseLock,
) -> OpsResult<()> {
    let releases = published_releases(repo, target)?;
    let regressions = regressions(repo)?;
    for regression in &regressions {
        let skipped: Vec<&str> = releases
            .iter()
            .filter(|release| regression.covers(release))
            .map(|release| release.tag.as_str())
            .collect();
        if !skipped.is_empty() {
            progress.status(&format!(
                "{} keeps stable off {}",
                regression.task,
                skipped.join(", ")
            ));
        }
    }
    let Some(next) = stable_advance(&releases, &regressions, chrono::Utc::now()) else {
        return Ok(());
    };
    progress.status(&format!(
        "Moving stable to {} (published {})...",
        next.tag,
        next.published_at.format("%Y-%m-%d")
    ));
    let publisher = expand_publisher_command(repo, &target.publisher);
    if let Some((program, args)) = publisher.split_first() {
        let mut cmd = lock.command(program);
        cmd.args(args)
            .args(["promote", "--tag", &next.tag])
            .env("LF_RELEASE_MAIN_REPO", repo)
            .current_dir(repo);
        run_command(&mut cmd).map_err(|err| OpsError::CommandFailed {
            command: err.command_line(),
            stderr: err.stderr,
        })?;
    }
    run_locked_stdout(
        lock,
        repo,
        "gh",
        &["release", "edit", &next.tag, "--latest"],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)] // Hook expansion and the two independent ownership scopes.
fn run_release_hooks(
    repo: &Path,
    hooks: &[String],
    target: &ReleaseTarget,
    version: Option<&str>,
    previous_tag: Option<&str>,
    phase: &str,
    lock: &ReleaseLock,
    lease: &WorktreeLease,
) -> OpsResult<()> {
    for hook in hooks {
        let command = hook
            .replace("{target}", &target.name)
            .replace("{version}", version.unwrap_or(""))
            .replace("{previous_tag}", previous_tag.unwrap_or(""));
        let mut cmd = lock.command("sh");
        lease.inherit(&mut cmd);
        cmd.args(["-c", &command]).current_dir(repo);
        run_command(&mut cmd).map_err(|err| OpsError::CommandFailed {
            command: format!("release {phase}: {command}"),
            stderr: err.stderr,
        })?;
    }
    Ok(())
}

fn is_ancestor(repo: &Path, ancestor: &str, descendant: &str) -> bool {
    Command::new("git")
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .current_dir(repo)
        .status()
        .is_ok_and(|status| status.success())
}

fn run_stdout(repo: &Path, command: &str, args: &[&str]) -> OpsResult<String> {
    let mut cmd = Command::new(command);
    cmd.args(args).current_dir(repo);
    command_stdout(&mut cmd)
}

fn run_locked_stdout(
    lock: &ReleaseLock,
    repo: &Path,
    command: &str,
    args: &[&str],
) -> OpsResult<String> {
    command_stdout(lock.command(command).args(args).current_dir(repo))
}

fn command_stdout(cmd: &mut Command) -> OpsResult<String> {
    let output = run_command(cmd).map_err(|err| OpsError::CommandFailed {
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

    fn published(tag: &str, days_ago: i64, is_latest: bool) -> PublishedRelease {
        PublishedRelease {
            tag: tag.to_string(),
            version: parse_version(tag).unwrap(),
            published_at: chrono::Utc::now() - chrono::TimeDelta::days(days_ago),
            is_latest,
        }
    }

    #[test]
    fn stable_moves_to_the_newest_soaked_release_and_never_back() {
        let now = chrono::Utc::now();
        let releases = [
            published("v0.16.1", 1, false),
            published("v0.16.0", 8, false),
            published("v0.15.9", 10, false),
            published("v0.15.2", 30, true),
        ];
        assert_eq!(stable_advance(&releases, &[], now).unwrap().tag, "v0.16.0");

        let at_stable = [
            published("v0.16.1", 1, false),
            published("v0.16.0", 8, true),
        ];
        assert!(stable_advance(&at_stable, &[], now).is_none());

        // A pointer moved ahead by hand stays put until a newer release soaks.
        let ahead = [
            published("v0.16.1", 1, true),
            published("v0.16.0", 8, false),
        ];
        assert!(stable_advance(&ahead, &[], now).is_none());
        assert!(stable_advance(&[published("v0.16.0", 6, false)], &[], now).is_none());
    }

    fn regression(version: &str, fixed_days_ago: Option<i64>) -> Regression {
        Regression {
            task: "LOO-1".to_string(),
            version: parse_version(version).unwrap(),
            fixed_at: fixed_days_ago.map(|days| chrono::Utc::now() - chrono::TimeDelta::days(days)),
        }
    }

    #[test]
    fn an_open_regression_holds_stable_below_the_release_it_regressed_in() {
        let now = chrono::Utc::now();
        let releases = [
            published("v0.16.1", 9, false),
            published("v0.16.0", 12, false),
            published("v0.15.9", 20, false),
            published("v0.15.2", 30, true),
        ];
        let open = [regression("v0.16.0", None)];
        assert_eq!(
            stable_advance(&releases, &open, now).unwrap().tag,
            "v0.15.9"
        );

        let at_regressed = [regression("v0.15.9", None)];
        assert!(stable_advance(&releases, &at_regressed, now).is_none());
    }

    #[test]
    fn a_fixed_regression_releases_stable_at_the_fix_once_it_soaks() {
        let now = chrono::Utc::now();
        // Fixed 10 days ago: v0.16.1 (published 9 days ago) ships the fix.
        let releases = [
            published("v0.16.1", 9, false),
            published("v0.16.0", 12, false),
            published("v0.15.2", 30, true),
        ];
        let fixed = [regression("v0.16.0", Some(10))];
        assert_eq!(
            stable_advance(&releases, &fixed, now).unwrap().tag,
            "v0.16.1"
        );

        // The fix shipped in v0.16.1, still soaking; v0.16.0 still carries the bug.
        let soaking = [
            published("v0.16.1", 3, false),
            published("v0.16.0", 12, false),
            published("v0.15.2", 30, true),
        ];
        assert!(stable_advance(&soaking, &[regression("v0.16.0", Some(4))], now).is_none());
    }

    #[test]
    fn merge_request_removals_decide_landing_and_release_alike() {
        use crate::ops::pr::{
            ManualRemoval, MergeGateReading, MergeRequestRemoval, PrInfo, PrMergeObservation,
            RepairWindow,
        };
        use crate::ops::pr_landing::{decide_merge, LandingObservation};
        use crate::work::task::CiCheck;
        use serde_json::json;

        #[derive(Debug, PartialEq)]
        enum Release {
            Stops,
            Polls,
            Repairs,
        }
        let at = |time: &str| time.parse::<chrono::DateTime<chrono::Utc>>().unwrap();
        let window = |start: &str, end: Option<&str>| RepairWindow {
            start: at(start),
            end: end.map(at),
        };
        // PR #1654: Jack disabled auto-merge, later dequeued the queued release,
        // and the queue finally removed it as merged.
        let disabled = json!({"__typename": "AutoMergeDisabledEvent",
            "createdAt": "2026-10-22T18:56:59Z", "reason": "Manually disabled by user",
            "reasonCode": "manually_disabled", "actor": {"login": "jacklionheart"}});
        let dequeued = json!({"__typename": "RemovedFromMergeQueueEvent",
            "createdAt": "2026-10-22T20:13:13Z", "reason": "manual",
            "actor": {"login": "jacklionheart"}});
        let merged = json!({"__typename": "RemovedFromMergeQueueEvent",
            "createdAt": "2026-10-22T20:23:15Z", "reason": "merged",
            "actor": {"login": "github-merge-queue"}});
        // PR #1676: the merge-group run failed on a runner fault and the queue
        // removed the PR; a second entry later hit a conflict and ran nothing.
        let armed =
            json!({"__typename": "AutoMergeEnabledEvent", "createdAt": "2026-10-25T05:40:31Z"});
        let added =
            json!({"__typename": "AddedToMergeQueueEvent", "createdAt": "2026-10-25T05:50:32Z"});
        let evicted = json!({"__typename": "RemovedFromMergeQueueEvent",
            "createdAt": "2026-10-25T06:02:33Z", "reason": "failed_checks",
            "actor": {"login": "github-merge-queue"}});
        let readded =
            json!({"__typename": "AddedToMergeQueueEvent", "createdAt": "2026-10-25T06:10:00Z"});
        let conflicted = json!({"__typename": "RemovedFromMergeQueueEvent",
            "createdAt": "2026-10-25T06:15:00Z", "reason": "merge_conflict"});
        let run = "https://github.com/loopflowstudio/loopflow/actions/runs/40166998506";

        let failing = |name: &str, url: Option<&str>| LandingObservation::Failing {
            head_sha: "head".into(),
            failing_checks: vec![CiCheck {
                name: name.into(),
                url: url.map(str::to_string),
            }],
        };
        let pending = LandingObservation::Pending {
            head_sha: "head".into(),
        };
        let withdrawn = |time: &str, queued| LandingObservation::Withdrawn {
            head_sha: "head".into(),
            removal: ManualRemoval {
                at: at(time),
                actor: Some("jacklionheart".into()),
                queued,
            },
        };
        let cases = [
            (
                "manual dequeue",
                vec![disabled.clone(), dequeued.clone()],
                vec![],
                false,
                withdrawn("2026-10-22T20:13:13Z", true),
                Release::Stops,
            ),
            (
                "manual disable",
                vec![disabled.clone()],
                vec![],
                false,
                withdrawn("2026-10-22T18:56:59Z", false),
                Release::Stops,
            ),
            (
                "repair disarm inside its window",
                vec![disabled.clone()],
                vec![window("2026-10-22T18:50:00Z", Some("2026-10-22T19:00:00Z"))],
                false,
                failing("merge-request (repair-disarmed)", None),
                Release::Repairs,
            ),
            (
                "disarm while a repair runs and checks are pending",
                vec![disabled.clone()],
                vec![window("2026-10-22T18:50:00Z", None)],
                true,
                pending.clone(),
                Release::Polls,
            ),
            (
                "disarm outside every repair window",
                vec![disabled.clone()],
                vec![window("2026-10-22T18:00:00Z", Some("2026-10-22T18:30:00Z"))],
                false,
                withdrawn("2026-10-22T18:56:59Z", false),
                Release::Stops,
            ),
            (
                "merged by the queue",
                vec![disabled.clone(), dequeued.clone(), merged.clone()],
                vec![],
                false,
                pending.clone(),
                Release::Polls,
            ),
            (
                "failed_checks eviction",
                vec![armed.clone(), added.clone(), evicted.clone()],
                vec![],
                false,
                failing("merge-queue (failed_checks)", Some(run)),
                Release::Repairs,
            ),
            (
                "merge_conflict eviction after a failed_checks one",
                vec![
                    armed.clone(),
                    added.clone(),
                    evicted.clone(),
                    readded,
                    conflicted,
                ],
                vec![],
                false,
                failing("merge-queue (merge_conflict)", None),
                Release::Repairs,
            ),
            (
                "latest event is an arm",
                vec![disabled.clone(), armed.clone()],
                vec![],
                false,
                failing("merge-request (unexplained)", None),
                Release::Repairs,
            ),
            (
                "no event",
                vec![],
                vec![],
                false,
                failing("merge-request (unexplained)", None),
                Release::Repairs,
            ),
        ];
        let pr = PrInfo {
            url: "https://github.com/loopflowstudio/loopflow/pull/1676".into(),
            number: 1676,
            state: "open".into(),
            branch: "release".into(),
            merge_commit: None,
            merged_at: None,
            head_sha: Some("head".into()),
            merge_state: Some("clean".into()),
        };
        for (name, nodes, repairs, checks_pending, landing, release) in cases {
            let timeline = serde_json::to_vec(&json!({"data": {"repository": {
                "pullRequest": {"timelineItems": {"nodes": nodes}}}}}))
            .unwrap();
            // Only the eviction's own queue entry produced a run.
            let runs = |entry: chrono::DateTime<chrono::Utc>, removed| {
                assert!(entry < removed, "{name}");
                Ok((entry == at("2026-10-25T05:50:32Z")).then(|| run.to_string()))
            };
            let removal: MergeRequestRemoval =
                crate::ops::pr::removal_from_timeline(&timeline, &repairs, runs).unwrap();
            let observed = decide_merge(
                &PrMergeObservation::fixture(pr.clone(), None, true),
                "head",
                || Ok(removal),
                |_| {
                    Ok(Some(MergeGateReading {
                        failing: false,
                        pending: checks_pending,
                        failing_leaves: Vec::new(),
                    }))
                },
            )
            .unwrap();
            assert_eq!(observed, landing, "{name}");
            let waited = match release_wait(&pr, observed) {
                Err(OpsError::ManualMergeRemoval { .. }) => Release::Stops,
                Ok(None) => Release::Polls,
                Ok(Some(ReleasePrWait::NeedsRepair(head))) if head == "head" => Release::Repairs,
                other => panic!("{name}: {other:?}"),
            };
            assert_eq!(waited, release, "{name}");
        }
    }

    #[test]
    fn withdrawn_release_pr_does_not_block_the_version() {
        let pr = |number, state: &str| GhReleasePr {
            number,
            state: state.to_string(),
            merge_commit: None,
            head_ref_oid: None,
        };

        let only_withdrawn = current_release_pr(vec![pr(1303, "CLOSED")]);
        assert!(only_withdrawn.is_none());

        let retried = current_release_pr(vec![pr(1305, "OPEN"), pr(1303, "CLOSED")]);
        assert_eq!(retried.map(|pr| pr.number), Some(1305));

        let released = current_release_pr(vec![pr(1304, "CLOSED"), pr(1300, "MERGED")]);
        assert_eq!(released.map(|pr| pr.number), Some(1300));
    }

    #[test]
    fn cycle_context_keeps_early_and_late_changes_before_detail() {
        let repo = tempfile::tempdir().unwrap();
        let target = default_release_target(repo.path());
        let files: Vec<String> = (0..100).map(|i| format!("src/component-{i}.rs")).collect();
        let commits: Vec<ReleaseCommit> = (0..174)
            .map(|i| ReleaseCommit {
                sha: format!("{i:040x}"),
                title: format!("Improvement {i}"),
                files: files.clone(),
            })
            .collect();
        let prs: Vec<MergedPr> = (0..174)
            .map(|i| MergedPr {
                number: i,
                title: format!("Improvement {i}"),
                body: Some("Useful description. ".repeat(200)),
                files: files.clone(),
                additions: 1,
                deletions: 0,
                changed_files: 100,
                merge_commit: Some(format!("{i:040x}")),
            })
            .collect();
        let (context, json) = build_release_notes_context(
            "0.13.0",
            Some("v0.12.0"),
            &commits,
            &prs,
            &target,
            None,
            0,
            Vec::new(),
            None,
            0,
        )
        .unwrap();
        assert!(json.len() <= RELEASE_CONTEXT_MAX_BYTES);
        assert_eq!(context.commits.len(), 174);
        assert_eq!(context.merged_prs.first().unwrap().number, 0);
        assert_eq!(context.merged_prs.last().unwrap().number, 173);
        assert_eq!(context.omissions.commits, 0);
        assert_eq!(context.omissions.merged_prs, 0);
        assert!(context.omissions.text_bytes > 0);
        assert!(context.omissions.commit_files > 0);
    }

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
    // Repo-owned preparation
    // ======================================================================

    /// The release worktree runs the target's `prepare` hook before notes, so
    /// what it writes is part of the release commit. Deleting hook execution
    /// makes the asserts below fail.
    ///
    /// The bare temp worktree has no manifests (so the version bump is a no-op).
    /// A deliberate release-note archive collision then stops the workflow before
    /// it launches an agent; the hook has already run by then.
    #[test]
    fn the_release_run_runs_the_prepare_hook_in_the_release_worktree() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let unreleased = root.join("release/unreleased");
        let release_archive = root.join("release/v0.11.4");
        fs::create_dir_all(&unreleased).unwrap();
        fs::create_dir_all(&release_archive).unwrap();
        fs::write(unreleased.join("collision.md"), "unreleased\n").unwrap();
        fs::write(release_archive.join("collision.md"), "archived\n").unwrap();

        let target = ReleaseTarget {
            name: "default".to_string(),
            area: Vec::new(),
            tag_prefix: "v".to_string(),
            manifests: Vec::new(),
            workflow: None,
            verify: Vec::new(),
            prepare: vec!["printf {version} > prepared.txt".to_string()],
            completion: ReleaseCompletion::Tag,
            publisher: Vec::new(),
        };
        run_or_panic(Command::new("git").args(["init"]).current_dir(root));
        let lock = ReleaseLock::acquire(root, "default").unwrap();
        let lease = acquire_worktree_lease(root, root, "test preparation").unwrap();
        let changes = ReleaseChangeSet {
            previous_tag: Some("v0.11.3".into()),
            commits: Vec::new(),
            merged_prs: Vec::new(),
        };
        let result = prepare_release_in_worktree(
            root,
            "0.11.4",
            &changes,
            &target,
            &crate::ops::progress::NullProgress,
            &lock,
            &lease,
        );
        assert!(
            result.is_err(),
            "release-note collision should stop release"
        );
        assert_eq!(
            fs::read_to_string(root.join("prepared.txt")).unwrap(),
            "0.11.4"
        );
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

    #[test]
    fn a_raised_reader_floor_reaches_the_pr_title_and_the_notes() {
        let tmp = tempfile::tempdir().unwrap();
        let target = default_release_target(tmp.path());
        let line = "lf < 1.2.3 can no longer open the store (raised by: drop_waves)";
        fs::create_dir_all(tmp.path().join("release/unreleased")).unwrap();
        fs::write(
            tmp.path().join("release/unreleased/BREAKING.md"),
            format!("{line}\n"),
        )
        .unwrap();
        promote_unreleased_dir(tmp.path(), "1.2.3").unwrap();
        fs::write(
            tmp.path().join("RELEASE_NOTES.md"),
            "# v1.2.3\n\n## Highlights\n\n- Releases work.",
        )
        .unwrap();
        let breaking = breaking_lines(Some(
            &fs::read_to_string(breaking_path(tmp.path(), "1.2.3")).unwrap(),
        ));

        finalize_release_notes(tmp.path(), "1.2.3", &breaking, None).unwrap();
        let copy = release_pr_copy(tmp.path(), &target, "1.2.3").unwrap();

        assert_eq!(
            copy.title,
            "release: v1.2.3: lf < 1.2.3 can no longer open the store"
        );
        assert!(copy.body.contains(&format!("**{line}**\n\n## Highlights")));
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
    fn promote_unreleased_appends_a_repair_cuts_breaking_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let unreleased = tmp.path().join("release").join("unreleased");
        fs::create_dir_all(&unreleased).unwrap();
        fs::write(
            unreleased.join("BREAKING.md"),
            "floor raised by b\nfloor raised by a\n",
        )
        .unwrap();

        let version_dir = tmp.path().join("release").join("v0.9.10");
        fs::create_dir_all(&version_dir).unwrap();
        fs::write(version_dir.join("BREAKING.md"), "floor raised by a\n").unwrap();

        promote_unreleased_dir(tmp.path(), "0.9.10").unwrap();

        assert!(!unreleased.exists());
        assert_eq!(
            fs::read_to_string(version_dir.join("BREAKING.md")).unwrap(),
            "floor raised by a\nfloor raised by b\n",
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

        ensure_commit_local(
            repo.path(),
            &new_sha,
            &ReleaseLock::acquire(repo.path(), "default").unwrap(),
        )
        .expect("fetch + verify should succeed");

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
        run_or_panic(Command::new("git").args(["init"]).current_dir(repo.path()));
        let lock = ReleaseLock::acquire(repo.path(), "default").unwrap();
        let lease = acquire_worktree_lease(repo.path(), repo.path(), "test hook").unwrap();
        run_release_hooks(
            repo.path(),
            &["printf '%s %s %s' '{target}' '{version}' '{previous_tag}' > hook.txt".to_string()],
            &target,
            Some("1.2.3"),
            Some("v1.2.2"),
            "preparation",
            &lock,
            &lease,
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
        run_or_panic(Command::new("git").args(["init"]).current_dir(repo.path()));
        let lock = ReleaseLock::acquire(repo.path(), "default").unwrap();
        let lease = acquire_worktree_lease(repo.path(), repo.path(), "test hook").unwrap();
        let error = run_release_hooks(
            repo.path(),
            &["echo 'repository rejected release' >&2; exit 1".to_string()],
            &target,
            None,
            None,
            "verification",
            &lock,
            &lease,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("repository rejected release"), "{error}");
    }
}
