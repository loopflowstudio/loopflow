use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::agent::{run_agent, AgentConfig, ProcessConfig};
use crate::config::{load_config_or_default, load_repo_config, MergeMethod};
use crate::git::worktrees::{list_worktrees, main_repo_root};
use crate::git::{current_branch, get_default_branch, rev_parse};
use crate::graph::load_skill;

use crate::ops::commit::{commit_workflow, CommitOptions};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::util::{command_exists, stderr_from_output};

#[derive(Debug, Clone)]
pub struct PrOptions {
    pub title: Option<String>,
    pub body: Option<String>,
    pub agent: Option<String>,
    /// Create drafts and preserve existing readiness instead of marking ready.
    pub draft: bool,
}

#[derive(Debug, Clone)]
pub struct PrResult {
    pub url: String,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrInfo {
    pub url: String,
    pub number: u64,
    pub state: String,
    pub branch: String,
    pub merge_commit: Option<String>,
    /// GitHub's authoritative merge instant from the PR response.
    pub merged_at: Option<String>,
    /// The PR's current head commit (`headRefOid`), when GitHub reports one.
    pub head_sha: Option<String>,
    /// GitHub's mergeability classification, when observed.
    pub merge_state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PrCopy {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Deserialize)]
struct PublishedPrCopy {
    #[serde(rename = "headRefOid")]
    head_sha: String,
    #[serde(flatten)]
    copy: PrCopy,
}

const GITHUB_PR_TITLE_MAX_CHARS: usize = 256;
const TASK_PR_CONTEXT_START: &str = "<!-- loopflow:task-pr-context:start -->";
const TASK_PR_CONTEXT_END: &str = "<!-- loopflow:task-pr-context:end -->";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskPrCopyLifecycle {
    Draft,
    Published,
    Completes,
}

#[derive(Debug, Deserialize)]
struct GhPr {
    url: String,
    state: String,
    #[serde(default, rename = "isDraft")]
    is_draft: bool,
    number: u64,
    #[serde(default, rename = "mergeCommit")]
    merge_commit: Option<GhCommit>,
    #[serde(default, rename = "headRefOid")]
    head_ref_oid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhCommit {
    oid: String,
}

pub fn create_or_update_pr(
    repo: &Path,
    options: &PrOptions,
    progress: &impl Progress,
) -> OpsResult<PrResult> {
    reject_control_plane_pr(repo)?;
    crate::ops::task::refresh_task_pr_for_publication(repo)?;
    crate::ops::sync::restart_landed_branch(repo)?;
    if !gh_available() {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }
    crate::ops::task::task_pr_context(repo)?;

    let main_repo = resolve_main_repo(repo);
    let default_branch = get_default_branch(&main_repo)?;
    let publication_base = || -> OpsResult<(bool, String)> {
        let stack = crate::ops::task::task_stack(repo)?;
        let base = match stack.as_ref().and_then(|stack| stack.parent_branch.clone()) {
            Some(parent) => parent,
            None if stack.is_some() => default_branch.clone(),
            None => pr_target(repo, &main_repo, &default_branch)?,
        };
        Ok((stack.is_some(), base))
    };
    let (stacked, base_branch) = publication_base()?;

    // Record the ancestry already present in Git before the first remote effect.
    // This recognizes completed merges without integrating or rewriting history.
    crate::ops::task::refresh_task_pr_base(repo)?;

    // Gate output is an in-worktree handoff, never published content. Consume
    // valid cached copy before deleting the gate-owned files so the commit and
    // push below can only expose the reviewed implementation tree.
    let cached_copy = consume_gate_artifacts(repo, progress)?;

    // Publication owns no integration. Commit locally, prove the resulting
    // range, then push exactly the branch the user has now. A PR may honestly
    // remain behind its base until an explicit integration boundary.
    let commit_options = CommitOptions {
        add: true,
        push: false,
        message: Some("lf pr open: prepare branch".to_string()),
        agent: options.agent.clone(),
        ..CommitOptions::for_task("commit")
    };
    if !crate::git::worktrees::is_persistent_worktree(repo)? {
        commit_workflow(repo, &commit_options, progress, &|_| {})?;
    }
    crate::ops::task::require_task_pr_range_nonempty(repo)?;
    require_non_task_pr_range_nonempty(repo, stacked, &base_branch)?;
    let branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    let published_head = rev_parse(repo, "HEAD")?;
    crate::ops::commit::push_with_upstream_if_needed(repo, &|_| {})?;

    let copy = resolve_pr_copy(repo, options, cached_copy, progress)?;
    let current_branch_state = current_branch(repo)?;
    let current_head = rev_parse(repo, "HEAD")?;
    if current_branch_state.as_deref() != Some(branch.as_str()) || current_head != published_head {
        return Err(OpsError::Message(format!(
            "PR copy generation changed the published branch/HEAD; expected {branch} at {published_head}"
        )));
    }
    crate::ops::commit::verify_remote_branch_head(repo, &branch, &published_head)?;
    // Keep publication and its durable GitHub projection atomic with respect
    // to later Loopflow pushes and shipping requests in this worktree.
    let _mutation = crate::ops::task::lock_task_pr_mutation(repo)?;
    // Preparation may have selected a parent while copy generation was running.
    let (stacked, base_branch) = publication_base()?;
    require_non_task_pr_range_nonempty(repo, stacked, &base_branch)?;
    let locked_branch = current_branch(repo)?;
    let locked_head = rev_parse(repo, "HEAD")?;
    if locked_branch.as_deref() != Some(branch.as_str()) || locked_head != published_head {
        return Err(OpsError::Message(format!(
            "branch changed before PR publication; expected {branch} at {published_head}"
        )));
    }
    crate::ops::commit::verify_remote_branch_head(repo, &branch, &published_head)?;
    // A same-head refresh preserves an armed request. Read it under the mutation
    // lock, after pushing has revoked any request for a superseded head.
    let task_context = crate::ops::task::task_pr_context(repo)?;
    let existing_pr = find_open_pr(repo)?;
    let draft = options.draft && existing_pr.as_ref().is_none_or(|pr| pr.is_draft);
    let lifecycle = match task_context
        .as_ref()
        .and_then(|context| context.merge_request.as_ref())
        .filter(|request| request.head_sha == published_head)
    {
        Some(_) => TaskPrCopyLifecycle::Completes,
        None if draft => TaskPrCopyLifecycle::Draft,
        None => TaskPrCopyLifecycle::Published,
    };
    let copy = normalize_task_pr_copy(copy, task_context.as_ref(), &lifecycle)?;
    let title = copy.title.trim();
    let body = copy.body.trim();
    crate::ops::task::request_task_pr_publication(repo, title, body)?;
    if let Some(mut pr) = existing_pr {
        let info = pr_info(&branch, &pr);
        crate::ops::task::attach_task_github_pr(repo, Some(&info), &|_| {})?;
        if !draft {
            mark_pr_ready(repo, &mut pr)?;
            crate::ops::task::attach_task_github_pr(repo, Some(&pr_info(&branch, &pr)), &|_| {})?;
        }
        progress.status("Updating PR...");
        update_pr(repo, info.number, title, body, &base_branch)?;
        crate::ops::task::record_task_pr_presentation(repo, title, body)?;
        Ok(PrResult {
            url: info.url,
            created: false,
        })
    } else {
        progress.status("Creating PR...");
        let url = create_pr(repo, title, body, &base_branch, draft, &|_| {})?;
        crate::ops::task::record_task_pr_presentation(repo, title, body)?;
        let acknowledged = pr_number_from_url(&url).map(|number| PrInfo {
            number,
            url: url.clone(),
            state: if draft { "draft" } else { "open" }.to_string(),
            branch: branch.clone(),
            merge_commit: None,
            merged_at: None,
            head_sha: None,
            merge_state: None,
        });
        // Creation identity survives a later read, readiness or linking failure.
        if let Some(info) = acknowledged.as_ref() {
            crate::ops::task::attach_task_github_pr(repo, Some(info), &|_| {})?;
        }
        if let Some(mut pr) = find_open_pr(repo)? {
            crate::ops::task::attach_task_github_pr(repo, Some(&pr_info(&branch, &pr)), &|_| {})?;
            if !draft {
                mark_pr_ready(repo, &mut pr)?;
                crate::ops::task::attach_task_github_pr(
                    repo,
                    Some(&pr_info(&branch, &pr)),
                    &|_| {},
                )?;
            }
        } else if acknowledged.is_none() {
            crate::ops::task::attach_task_github_pr(repo, None, &|_| {})?;
        }
        Ok(PrResult { url, created: true })
    }
}

pub(crate) fn normalize_task_pr_copy(
    copy: PrCopy,
    context: Option<&crate::ops::task::TaskPrContext>,
    lifecycle: &TaskPrCopyLifecycle,
) -> OpsResult<PrCopy> {
    let Some(context) = context else {
        return Ok(copy);
    };
    let task_title = context.title.trim();
    let task_identifier = context.identifier.trim();
    if task_title.is_empty()
        || task_identifier.is_empty()
        || task_title.chars().any(char::is_control)
        || task_identifier.chars().any(char::is_control)
    {
        return Err(OpsError::Message(
            "Task PR context requires a non-empty, single-line Task identifier and name"
                .to_string(),
        ));
    }

    let title = copy.title;
    if title.chars().count() > GITHUB_PR_TITLE_MAX_CHARS {
        return Err(OpsError::Message(format!(
            "PR title exceeds GitHub's {GITHUB_PR_TITLE_MAX_CHARS}-character PR title limit"
        )));
    }

    let task_link = context.task_link();
    let pr_lifecycle: String = match lifecycle {
        TaskPrCopyLifecycle::Draft => "The pull request is a draft; no Task settlement is requested.".into(),
        TaskPrCopyLifecycle::Published => "The pull request is published for review; no Task settlement is requested.".into(),
        TaskPrCopyLifecycle::Completes => "Merge delivers the code. Record independent later work as Tasks and comments; complete the delivered outcome.".into(),
    };
    let managed = format!(
        "{TASK_PR_CONTEXT_START}\n> [!NOTE]\n> **Task:** {task_link}\n> **PR lifecycle:** {pr_lifecycle}\n{TASK_PR_CONTEXT_END}"
    );
    let reviewer_context = _strip_managed_task_context(&copy.body);
    let body = if reviewer_context.is_empty() {
        managed
    } else {
        // The authored opening paragraph is the summary; keep it ahead of metadata.
        let summary_end = reviewer_context
            .split_inclusive('\n')
            .take_while(|line| !line.trim().is_empty())
            .map(str::len)
            .sum();
        let (summary, rest) = reviewer_context.split_at(summary_end);
        let separator_len: usize = rest
            .split_inclusive('\n')
            .take_while(|line| line.trim().is_empty())
            .map(str::len)
            .sum();
        let summary = summary.trim_end_matches('\n');
        let details = &rest[separator_len..];
        if details.is_empty() {
            format!("{summary}\n\n{managed}")
        } else {
            format!("{summary}\n\n{managed}\n\n{details}")
        }
    };
    Ok(PrCopy { title, body })
}

fn _markdown_code(value: &str) -> String {
    format!("`{}`", value.replace('`', "\\`"))
}

fn _strip_managed_task_context(body: &str) -> String {
    let mut without_block = body.to_string();
    while let Some(start) = without_block.find(TASK_PR_CONTEXT_START) {
        let Some(end) = without_block[start..].find(TASK_PR_CONTEXT_END) else {
            break;
        };
        let after = start + end + TASK_PR_CONTEXT_END.len();
        without_block = format!(
            "{}\n\n{}",
            without_block[..start].trim_end(),
            without_block[after..].trim_start_matches(['\r', '\n'])
        );
    }
    without_block
        .lines()
        .filter(|line| !line.trim_start().starts_with("Linear Task:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn pr_info(branch: &str, pr: &GhPr) -> PrInfo {
    PrInfo {
        url: pr.url.clone(),
        number: pr.number,
        state: if pr.is_draft {
            "draft".to_string()
        } else {
            pr.state.to_ascii_lowercase()
        },
        branch: branch.to_string(),
        merge_commit: pr.merge_commit.as_ref().map(|commit| commit.oid.clone()),
        merged_at: None,
        head_sha: pr.head_ref_oid.clone(),
        merge_state: None,
    }
}

fn pr_number_from_url(url: &str) -> Option<u64> {
    url.trim_end_matches('/').rsplit('/').next()?.parse().ok()
}

pub(crate) fn reject_control_plane_pr(repo: &Path) -> OpsResult<()> {
    let main_repo = main_repo_root(repo)?;
    let checkout = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let main_repo = main_repo.canonicalize().unwrap_or(main_repo);
    let default_branch = get_default_branch(repo)?;
    let branch = current_branch(repo)?;
    if checkout == main_repo && branch.as_deref() == Some(default_branch.as_str()) {
        return Err(OpsError::Message(
            "the default branch cannot open a PR; use `lf wt create <name>` or publish from the persistent conversation checkout"
                .to_string(),
        ));
    }
    Ok(())
}

fn resolve_pr_copy(
    repo: &Path,
    options: &PrOptions,
    cached: Option<PrCopy>,
    progress: &impl Progress,
) -> OpsResult<PrCopy> {
    if let Some(title) = options
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(PrCopy {
            title: title.to_string(),
            body: options.body.clone().unwrap_or_default(),
        });
    }

    let mut copy = match cached {
        Some(copy) => {
            progress.status("Using cached PR copy from task gate");
            copy
        }
        None => generate_pr_copy(repo, progress, options.agent.as_deref())?,
    };
    if let Some(body_override) = options.body.as_deref() {
        copy.body = body_override.to_string();
    }
    Ok(copy)
}

fn consume_gate_artifacts(repo: &Path, progress: &impl Progress) -> OpsResult<Option<PrCopy>> {
    let cached = read_cached_pr_copy(repo, progress)?;
    if crate::git::worktrees::is_persistent_worktree(repo)? {
        return Ok(cached);
    }
    let scratch = repo.join("scratch");
    let mut removed = false;
    for name in [".pr-copy-ref", "pr-title.txt", "pr-body.md"] {
        let path = scratch.join(name);
        if path.is_file() {
            std::fs::remove_file(path)?;
            removed = true;
        }
    }
    if removed {
        progress.status("Removing gate artifacts before publication");
    }
    Ok(cached)
}

pub(crate) fn read_cached_pr_copy(
    repo: &Path,
    progress: &impl Progress,
) -> OpsResult<Option<PrCopy>> {
    let title_path = repo.join("scratch/pr-title.txt");
    let body_path = repo.join("scratch/pr-body.md");
    let ref_path = repo.join("scratch/.pr-copy-ref");

    if !title_path.exists() || !body_path.exists() {
        return Ok(None);
    }

    let title = std::fs::read_to_string(&title_path)?.trim().to_string();
    if title.is_empty() {
        return Ok(None);
    }

    let copied_for = match std::fs::read_to_string(&ref_path) {
        Ok(value) => value.trim().to_string(),
        Err(_) => {
            progress.status("Ignoring cached PR copy: scratch/.pr-copy-ref is missing");
            return Ok(None);
        }
    };
    if !is_recent_ancestor(repo, &copied_for, 1)? {
        progress.status("Ignoring cached PR copy: branch changed since gate output");
        return Ok(None);
    }

    let body = std::fs::read_to_string(body_path)?;
    Ok(Some(PrCopy { title, body }))
}

/// Check if HEAD is no more than `max_ahead` commits ahead of `commit`.
/// This tolerates one bookkeeping commit after gate output while still
/// forcing regeneration if substantive commits were added later.
fn is_recent_ancestor(repo: &Path, commit: &str, max_ahead: u32) -> OpsResult<bool> {
    let output = Command::new("git")
        .args(["rev-list", "--count", &format!("{commit}..HEAD")])
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Ok(false);
    }
    let ahead = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .unwrap_or(u32::MAX);
    Ok(ahead <= max_ahead)
}

/// Read before pushing: a push can advance the PR head without updating its copy.
pub(crate) fn published_pr_copy(repo: &Path, head: &str) -> OpsResult<Option<PrCopy>> {
    let branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    let output = Command::new("gh")
        .args([
            "pr",
            "list",
            "--head",
            &branch,
            "--state",
            "open",
            "--json",
            "headRefOid,title,body",
        ])
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("gh pr list --head {branch}"),
            stderr: stderr_from_output(&output),
        });
    }
    let copies: Vec<PublishedPrCopy> = serde_json::from_slice(&output.stdout)
        .map_err(|error| OpsError::Message(format!("failed to read published PR copy: {error}")))?;
    Ok(copies
        .into_iter()
        .find(|published| published.head_sha == head)
        .map(|published| published.copy))
}

pub fn generate_pr_copy(
    repo: &Path,
    progress: &impl Progress,
    agent_override: Option<&str>,
) -> OpsResult<PrCopy> {
    let template = load_skill("pr-message", repo)
        .map_err(|err| OpsError::Message(format!("pr-message skill not found: {err}")))?
        .content
        .ok_or_else(|| OpsError::Message("pr-message skill has no content".to_string()))?;
    let main_repo = resolve_main_repo(repo);
    let default_branch = get_default_branch(&main_repo)?;
    let stack = crate::ops::task::task_stack(repo)?;
    let base_branch = match stack.as_ref() {
        Some(stack) => stack.parent_branch.clone().unwrap_or(default_branch),
        None => pr_target(repo, &main_repo, &default_branch)?,
    };
    require_non_task_pr_range_nonempty(repo, stack.is_some(), &base_branch)?;
    let log = git_stdout(
        repo,
        &["log", &format!("origin/{base_branch}..HEAD"), "--oneline"],
    )?;
    let stat = git_stdout(
        repo,
        &["diff", &format!("origin/{base_branch}...HEAD"), "--stat"],
    )?;
    let diff = git_stdout(repo, &["diff", &format!("origin/{base_branch}...HEAD")])?;
    let diff = truncate_chars(&diff, 20_000);

    let prompt = format!(
        "{template}\n\n## Base branch\n{base_branch}\n\n## Commits\n```\n{log}\n```\n\n## Diff stat\n```\n{stat}\n```\n\n## Unified diff\n```diff\n{diff}\n```\n\nReturn exactly one JSON object with this schema:\n{{\"title\":\"...\",\"body\":\"...\"}}\nNo markdown fences. No explanation."
    );

    let config = load_config_or_default(Some(repo));
    let agent = agent_override.unwrap_or_else(|| config.agent()).to_string();
    progress.status("Generating PR title/body...");

    let launch = AgentConfig {
        chrome: config.chrome,
        task_prompt: prompt,
        agent: Some(agent),
        cwd: Some(repo.to_path_buf()),
        skip_permissions: config.yolo,
        ..Default::default()
    };
    let process = ProcessConfig {
        auto: true,
        stream: false,
        ..Default::default()
    };

    let result = run_agent(&launch, &process)
        .map_err(|err| OpsError::Message(format!("failed to generate PR copy: {err}")))?;
    if result.exit_code != 0 {
        return Err(OpsError::Message(format!(
            "PR copy generation failed (exit {}): {}",
            result.exit_code,
            result.stderr.trim()
        )));
    }

    let combined = format!("{}\n{}", result.stdout, result.stderr);
    parse_generated_pr_copy(&result.stdout)
        .or_else(|| parse_generated_pr_copy(&result.stderr))
        .or_else(|| parse_generated_pr_copy(&combined))
        .ok_or_else(|| {
            OpsError::Message(format!(
                "failed to parse generated PR copy from agent output\n{}",
                format_pr_copy_parse_preview(&combined)
            ))
        })
}

fn require_non_task_pr_range_nonempty(
    repo: &Path,
    task_stack_present: bool,
    base_branch: &str,
) -> OpsResult<()> {
    if task_stack_present {
        return Ok(());
    }
    let range = format!("origin/{base_branch}...HEAD");
    let output = Command::new("git")
        .args(["diff", "--quiet", &range, "--"])
        .current_dir(repo)
        .output()?;
    match output.status.code() {
        Some(0) => Err(OpsError::Message(format!(
            "branch has no changes from {base_branch}; it may already be landed. Refused before PR copy generation or GitHub mutation"
        ))),
        Some(1) => Ok(()),
        _ => Err(OpsError::CommandFailed {
            command: format!("git diff --quiet {range} --"),
            stderr: stderr_from_output(&output),
        }),
    }
}

pub fn gh_available() -> bool {
    command_exists("gh")
}

pub fn pr_exists_for_current_branch(repo: &Path) -> OpsResult<bool> {
    Ok(find_open_pr(repo)?.is_some())
}

pub fn current_pr(repo: &Path) -> OpsResult<Option<PrInfo>> {
    if !gh_available() {
        return Ok(None);
    }
    let branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    Ok(find_open_branch_pr(repo, &branch)?.map(|pr| pr_info(&branch, &pr)))
}

pub(crate) fn branch_pr(repo: &Path, branch: &str) -> OpsResult<Option<PrInfo>> {
    if !gh_available() {
        return Ok(None);
    }
    Ok(find_open_branch_pr(repo, branch)?.map(|pr| pr_info(branch, &pr)))
}

pub(crate) fn auto_merge_enabled(repo: &Path, number: u64) -> OpsResult<bool> {
    Ok(observe_merge_request(repo, number)?.is_some())
}

#[derive(Debug)]
pub(crate) enum MergeRequest {
    Auto,
    AwaitingQueue,
    Queued(String),
}

/// Whether an open PR can merge only after main is merged into it: GitHub
/// reports a conflict, requires an up-to-date head with no queue to provide
/// one, or the queue evicted the PR for a conflict. Queue membership tests an
/// integrated commit and takes precedence. Anything else, including a failed
/// check, is repaired on top of the branch against the same base. `removal`
/// runs only for an unarmed PR on a queue.
pub(crate) fn pr_requires_resync(
    observation: &PrMergeObservation,
    removal: impl FnOnce() -> OpsResult<MergeRequestRemoval>,
) -> OpsResult<bool> {
    if matches!(observation.request, Some(MergeRequest::Queued(_))) {
        return Ok(false);
    }
    let state = observation.pr.merge_state.as_deref().unwrap_or_default();
    if state.eq_ignore_ascii_case("dirty")
        || (state.eq_ignore_ascii_case("behind") && !observation.merge_queue_enabled)
    {
        return Ok(true);
    }
    if observation.request.is_some() || !observation.merge_queue_enabled {
        return Ok(false);
    }
    Ok(matches!(
        removal()?,
        MergeRequestRemoval::Evicted { reason, .. } if reason.eq_ignore_ascii_case("merge_conflict")
    ))
}

#[derive(Debug)]
pub(crate) struct PrMergeObservation {
    pub pr: PrInfo,
    pub request: Option<MergeRequest>,
    merge_queue_enabled: bool,
    methods: MergeMethods,
}

#[cfg(test)]
impl PrMergeObservation {
    pub(crate) fn fixture(
        pr: PrInfo,
        request: Option<MergeRequest>,
        merge_queue_enabled: bool,
    ) -> Self {
        Self {
            pr,
            request,
            merge_queue_enabled,
            methods: MergeMethods {
                merge_commit_allowed: true,
                squash_merge_allowed: true,
                rebase_merge_allowed: true,
            },
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MergeMethods {
    merge_commit_allowed: bool,
    squash_merge_allowed: bool,
    rebase_merge_allowed: bool,
}

impl MergeMethods {
    fn allows(&self, method: MergeMethod) -> bool {
        match method {
            MergeMethod::Merge => self.merge_commit_allowed,
            MergeMethod::Squash => self.squash_merge_allowed,
            MergeMethod::Rebase => self.rebase_merge_allowed,
        }
    }

    fn select(&self, configured: Option<MergeMethod>) -> OpsResult<MergeMethod> {
        if let Some(method) = configured {
            return if self.allows(method) {
                Ok(method)
            } else {
                Err(OpsError::Message(format!("repository merge_method {method:?} is disabled on GitHub; update the repository policy or use manual submission")))
            };
        }
        [MergeMethod::Squash, MergeMethod::Merge, MergeMethod::Rebase]
            .into_iter()
            .find(|method| self.allows(*method))
            .ok_or_else(|| OpsError::Message("GitHub reports no allowed merge method; use manual submission to inspect repository policy".into()))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPrMerge {
    id: String,
    number: u64,
    url: String,
    state: String,
    is_draft: bool,
    head_ref_name: String,
    head_ref_oid: String,
    merged_at: Option<String>,
    merge_commit: Option<GhCommit>,
    merge_state_status: String,
    is_merge_queue_enabled: bool,
    // GitHub can report queue membership before it exposes the entry.
    is_in_merge_queue: bool,
    // Value requires the fields to exist while allowing GitHub's null values.
    auto_merge_request: serde_json::Value,
    merge_queue_entry: serde_json::Value,
}

pub(crate) fn observe_pr_merge(repo: &Path, number: u64) -> OpsResult<PrMergeObservation> {
    let query = "query LoopflowPrMerge($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){mergeCommitAllowed squashMergeAllowed rebaseMergeAllowed pullRequest(number:$number){id number url state isDraft headRefName headRefOid mergedAt mergeCommit{oid} mergeStateStatus isMergeQueueEnabled isInMergeQueue autoMergeRequest{enabledAt} mergeQueueEntry{id}}}}";
    let observation = Command::new("gh")
        .args([
            "api",
            "graphql",
            "-F",
            "owner={owner}",
            "-F",
            "name={repo}",
            "-F",
            &format!("number={number}"),
            "-f",
            &format!("query={query}"),
        ])
        .current_dir(repo)
        .output()?;
    if !observation.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("gh api graphql [pull request #{number} merge state]"),
            stderr: stderr_from_output(&observation),
        });
    }
    parse_pr_merge(&observation.stdout)
}

fn parse_pr_merge(output: &[u8]) -> OpsResult<PrMergeObservation> {
    let response: serde_json::Value = serde_json::from_slice(output)
        .map_err(|error| OpsError::Parse(format!("failed to parse PR merge state: {error}")))?;
    if response
        .get("errors")
        .is_some_and(|errors| !errors.is_null() && !errors.as_array().is_some_and(Vec::is_empty))
    {
        return Err(OpsError::Message(
            "GitHub returned errors while reading PR merge state".into(),
        ));
    }
    let pr: GhPrMerge =
        serde_json::from_value(response["data"]["repository"]["pullRequest"].clone())
            .map_err(|error| OpsError::Parse(format!("failed to parse PR merge state: {error}")))?;
    for (name, value) in [
        ("autoMergeRequest", &pr.auto_merge_request),
        ("mergeQueueEntry", &pr.merge_queue_entry),
    ] {
        if !value.is_null() && !value.is_object() {
            return Err(OpsError::Parse(format!("invalid PR {name}")));
        }
    }
    let state = match pr.state.as_str() {
        "OPEN" if pr.is_draft => "draft",
        "OPEN" => "open",
        "CLOSED" => "closed",
        "MERGED" => "merged",
        state => return Err(OpsError::Parse(format!("unknown PR state: {state}"))),
    };
    let request = if pr.is_in_merge_queue || !pr.merge_queue_entry.is_null() {
        Some(MergeRequest::Queued(pr.id))
    } else if pr.auto_merge_request.is_null() {
        None
    } else if pr.is_merge_queue_enabled {
        Some(MergeRequest::AwaitingQueue)
    } else {
        Some(MergeRequest::Auto)
    };
    Ok(PrMergeObservation {
        merge_queue_enabled: pr.is_merge_queue_enabled,
        methods: serde_json::from_value(response["data"]["repository"].clone()).map_err(
            |error| OpsError::Parse(format!("failed to read repository merge methods: {error}")),
        )?,
        pr: PrInfo {
            url: pr.url,
            number: pr.number,
            state: state.to_string(),
            branch: pr.head_ref_name,
            merge_commit: pr.merge_commit.map(|commit| commit.oid),
            merged_at: pr.merged_at,
            head_sha: Some(pr.head_ref_oid),
            merge_state: Some(pr.merge_state_status.to_ascii_lowercase()),
        },
        request,
    })
}

/// A person dequeued a PR or disabled its auto-merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManualRemoval {
    pub at: chrono::DateTime<chrono::Utc>,
    pub actor: Option<String>,
    /// Removed from the merge queue rather than taken off auto-merge.
    pub queued: bool,
}

/// Why an open PR has no merge request, read from its latest merge-request event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MergeRequestRemoval {
    Manual(ManualRemoval),
    /// A Loopflow CI repair disarmed it while repairing this PR.
    ByLoopflow,
    /// GitHub's queue dropped it; `run_url` is the merge-group run for this entry.
    Evicted {
        reason: String,
        run_url: Option<String>,
    },
    /// The queue removed it because it merged; the PR read has not caught up.
    Merged,
    /// No timeline event explains the missing request.
    Unknown,
}

/// The lifetime of one CI repair on a PR; `end` is absent while it still runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RepairWindow {
    pub start: chrono::DateTime<chrono::Utc>,
    pub end: Option<chrono::DateTime<chrono::Utc>>,
}

impl RepairWindow {
    fn contains(&self, at: chrono::DateTime<chrono::Utc>) -> bool {
        self.start <= at && self.end.is_none_or(|end| at <= end)
    }
}

/// GitHub runs every removal with the same token, so actor and reason cannot
/// tell Loopflow from a person; only Loopflow's own repair windows can.
pub(crate) fn merge_request_removal(
    repo: &Path,
    number: u64,
    repairs: &[RepairWindow],
) -> OpsResult<MergeRequestRemoval> {
    removal_from_events(
        merge_request_events(repo, number)?,
        repairs,
        |added, removed| merge_group_run(repo, number, added, removed),
    )
}

fn removal_from_events(
    mut events: Vec<MergeRequestEvent>,
    repairs: &[RepairWindow],
    run: impl FnOnce(
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
    ) -> OpsResult<Option<String>>,
) -> OpsResult<MergeRequestRemoval> {
    events.sort_by_key(|event| event.created_at);
    let Some(latest) = events.last() else {
        return Ok(MergeRequestRemoval::Unknown);
    };
    let manual = match latest.typename.as_str() {
        "RemovedFromMergeQueueEvent" => match latest.reason.as_deref() {
            Some("manual") => true,
            Some("merged") => return Ok(MergeRequestRemoval::Merged),
            reason => {
                let reason = reason.unwrap_or("unknown").to_string();
                // The run tested this queue entry: from its addition to its removal.
                let added = events
                    .iter()
                    .rev()
                    .find(|event| event.typename == "AddedToMergeQueueEvent")
                    .map(|event| event.created_at);
                let run_url = match added {
                    Some(added) => run(added, latest.created_at)?,
                    None => None,
                };
                return Ok(MergeRequestRemoval::Evicted { reason, run_url });
            }
        },
        "AutoMergeDisabledEvent" => latest.reason_code.as_deref() == Some("manually_disabled"),
        _ => false,
    };
    if !manual {
        return Ok(MergeRequestRemoval::Unknown);
    }
    if repairs
        .iter()
        .any(|window| window.contains(latest.created_at))
    {
        return Ok(MergeRequestRemoval::ByLoopflow);
    }
    Ok(MergeRequestRemoval::Manual(ManualRemoval {
        at: latest.created_at,
        actor: latest.actor.as_ref().map(|actor| actor.login.clone()),
        queued: latest.typename == "RemovedFromMergeQueueEvent",
    }))
}

/// The non-successful merge-group run GitHub created for `number` between its
/// queue entry and its removal.
fn merge_group_run(
    repo: &Path,
    number: u64,
    added: chrono::DateTime<chrono::Utc>,
    removed: chrono::DateTime<chrono::Utc>,
) -> OpsResult<Option<String>> {
    #[derive(Deserialize)]
    struct Runs {
        workflow_runs: Vec<Run>,
    }
    #[derive(Deserialize)]
    struct Run {
        head_branch: Option<String>,
        conclusion: Option<String>,
        html_url: String,
    }
    let time =
        |at: chrono::DateTime<chrono::Utc>| at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let output = Command::new("gh")
        .args([
            "api",
            &format!(
                "repos/{{owner}}/{{repo}}/actions/runs?event=merge_group&per_page=100&created={}..{}",
                time(added),
                time(removed)
            ),
        ])
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("gh api [merge-group runs for pull request #{number}]"),
            stderr: stderr_from_output(&output),
        });
    }
    let runs: Runs = serde_json::from_slice(&output.stdout)
        .map_err(|error| OpsError::Parse(format!("failed to parse merge-group runs: {error}")))?;
    // GitHub lists newest first; queue branches are gh-readonly-queue/<base>/pr-<n>-<sha>.
    let marker = format!("/pr-{number}-");
    Ok(runs
        .workflow_runs
        .into_iter()
        .find(|run| {
            run.head_branch
                .as_deref()
                .is_some_and(|branch| branch.contains(&marker))
                && run.conclusion.as_deref().is_some_and(|c| c != "success")
        })
        .map(|run| run.html_url))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MergeRequestEvent {
    #[serde(rename = "__typename")]
    typename: String,
    created_at: chrono::DateTime<chrono::Utc>,
    reason: Option<String>,
    reason_code: Option<String>,
    actor: Option<MergeRequestActor>,
}

#[derive(Deserialize)]
struct MergeRequestActor {
    login: String,
}

/// Read a recorded `LoopflowPrMergeRequestEvents` response as the reader would.
#[cfg(test)]
pub(crate) fn removal_from_timeline(
    timeline: &[u8],
    repairs: &[RepairWindow],
    run: impl FnOnce(
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
    ) -> OpsResult<Option<String>>,
) -> OpsResult<MergeRequestRemoval> {
    removal_from_events(parse_merge_request_events(timeline)?, repairs, run)
}

fn merge_request_events(repo: &Path, number: u64) -> OpsResult<Vec<MergeRequestEvent>> {
    let query = "query LoopflowPrMergeRequestEvents($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){pullRequest(number:$number){timelineItems(last:20,itemTypes:[REMOVED_FROM_MERGE_QUEUE_EVENT,AUTO_MERGE_DISABLED_EVENT,AUTO_MERGE_ENABLED_EVENT,ADDED_TO_MERGE_QUEUE_EVENT]){nodes{__typename ... on RemovedFromMergeQueueEvent{createdAt reason actor{login}} ... on AutoMergeDisabledEvent{createdAt reason reasonCode actor{login}} ... on AutoMergeEnabledEvent{createdAt} ... on AddedToMergeQueueEvent{createdAt}}}}}}";
    let output = Command::new("gh")
        .args([
            "api",
            "graphql",
            "-F",
            "owner={owner}",
            "-F",
            "name={repo}",
            "-F",
            &format!("number={number}"),
            "-f",
            &format!("query={query}"),
        ])
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("gh api graphql [pull request #{number} merge removals]"),
            stderr: stderr_from_output(&output),
        });
    }
    parse_merge_request_events(&output.stdout)
}

fn parse_merge_request_events(output: &[u8]) -> OpsResult<Vec<MergeRequestEvent>> {
    let response: serde_json::Value = serde_json::from_slice(output)
        .map_err(|error| OpsError::Parse(format!("failed to parse PR timeline: {error}")))?;
    serde_json::from_value(
        response["data"]["repository"]["pullRequest"]["timelineItems"]["nodes"].clone(),
    )
    .map_err(|error| OpsError::Parse(format!("failed to parse PR timeline: {error}")))
}

pub(crate) fn observe_merge_request(repo: &Path, number: u64) -> OpsResult<Option<MergeRequest>> {
    Ok(observe_pr_merge(repo, number)?.request)
}

/// Revoke GitHub auto-merge or queue membership before a stored request is cleared.
/// The read makes replay idempotent after a prior disable succeeded.
pub(crate) fn disable_auto_merge(
    repo: &Path,
    number: u32,
    inherit: &impl Fn(&mut Command),
) -> OpsResult<()> {
    let Some(request) = observe_merge_request(repo, u64::from(number))? else {
        return Ok(());
    };
    let mut command = Command::new("gh");
    inherit(&mut command);
    let description = match request {
        MergeRequest::Auto | MergeRequest::AwaitingQueue => {
            command.args(["pr", "merge", &number.to_string(), "--disable-auto"]);
            format!("gh pr merge {number} --disable-auto")
        }
        MergeRequest::Queued(id) => {
            // gh pr merge returns success without acting on an already queued PR.
            command.args([
                "api",
                "graphql",
                "-f",
                "query=mutation($id:ID!){dequeuePullRequest(input:{id:$id}){clientMutationId}}",
                "-f",
                &format!("id={id}"),
            ]);
            format!("gh api graphql [dequeue pull request #{number}]")
        }
    };
    let output = command.current_dir(repo).output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(OpsError::CommandFailed {
        command: description,
        stderr: stderr_from_output(&output),
    })
}

/// Inherit the caller's capabilities in both replacement and arming children.
pub(crate) fn enable_auto_merge(
    repo: &Path,
    number: u64,
    copy: Option<&PrCopy>,
    head_sha: &str,
    inherit: &impl Fn(&mut Command),
) -> OpsResult<()> {
    let observation = observe_pr_merge(repo, number)?;
    // A queue entry tests the head it was queued with, and any push dequeues it.
    // Replacing it would dequeue a green PR only to ask GitHub to queue it again.
    if matches!(observation.request, Some(MergeRequest::Queued(_)))
        && observation.pr.head_sha.as_deref() == Some(head_sha)
    {
        return Ok(());
    }
    let configured = load_repo_config(repo)
        .map_err(|error| OpsError::Message(error.to_string()))?
        .and_then(|config| config.merge_method);
    // A merge queue owns its strategy. Validate policy before replacing an arm.
    let method = if observation.merge_queue_enabled {
        None
    } else {
        Some(observation.methods.select(configured)?)
    };
    if observation.request.is_some() {
        let number = u32::try_from(number).map_err(|_| {
            OpsError::Message(format!("pull request #{number} exceeds supported range"))
        })?;
        // A pre-existing remote arm carries no durable Loopflow head binding.
        // Replace it so every accepted Auto request crosses our exact-head
        // command boundary, even when GitHub already reports auto-merge.
        disable_auto_merge(repo, number, inherit)?;
    }

    let mut command = Command::new("gh");
    inherit(&mut command);
    command.arg("pr").arg("merge").arg(number.to_string());
    if let Some(method) = method {
        command.arg(match method {
            MergeMethod::Merge => "--merge",
            MergeMethod::Squash => "--squash",
            MergeMethod::Rebase => "--rebase",
        });
    }
    command
        .arg("--auto")
        .arg("--match-head-commit")
        .arg(head_sha);
    if let Some(copy) =
        copy.filter(|_| matches!(method, Some(MergeMethod::Merge | MergeMethod::Squash)))
    {
        command.arg("--subject").arg(&copy.title);
        if !copy.body.trim().is_empty() {
            command.arg("--body").arg(&copy.body);
        }
    }
    command.current_dir(repo);
    let description =
        format!("gh pr merge {number} --auto --match-head-commit {head_sha} (method: {method:?})");
    // gh exits 0 without arming when its own read believes the PR is already
    // queued, which a disable just before can leave stale. Confirm GitHub holds
    // a request, ask once more, and otherwise report GitHub's answer.
    let mut answers = Vec::new();
    for _ in 0..2 {
        let output = command.output()?;
        if !output.status.success() {
            return Err(OpsError::CommandFailed {
                command: description,
                stderr: stderr_from_output(&output),
            });
        }
        for stream in [&output.stdout, &output.stderr] {
            let text = String::from_utf8_lossy(stream).trim().to_string();
            if !text.is_empty() {
                answers.push(text);
            }
        }
        if holds_merge_request(repo, number)? {
            return Ok(());
        }
    }
    let answered = if answers.is_empty() {
        String::new()
    } else {
        format!("; gh answered: {}", answers.join("\n"))
    };
    Err(OpsError::Message(format!(
        "GitHub accepted `{description}` but #{number} has no auto-merge request or queue entry{answered}"
    )))
}

/// Whether GitHub reports the PR armed, queued or merged, allowing a few
/// seconds for a fresh request to become visible.
fn holds_merge_request(repo: &Path, number: u64) -> OpsResult<bool> {
    const READS: u32 = 3;
    for read in 1..=READS {
        let observation = observe_pr_merge(repo, number)?;
        if observation.request.is_some() || observation.pr.state == "merged" {
            return Ok(true);
        }
        if read < READS {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    Ok(false)
}

/// The outcome of a bounded, single-PR remote observation. GitHub is a
/// reconciliation input, never the Task's store of record: a transport, quota,
/// or network failure must leave the cached Task/PR row standing rather than
/// erroring the control command that triggered the read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PrObservation {
    /// The remote confirmed the PR's current state.
    Fresh(PrInfo),
    /// The PR number 404s — its ref was deleted remotely. The caller keeps its
    /// cached settled/working state; the merge (if any) is already persisted.
    NotFound,
    /// A quota, network, or GitHub failure. `reason` is user-facing; the caller
    /// preserves its cached state and surfaces the reason as degraded freshness.
    Degraded { reason: String },
}

/// Whether a PR read may be served from a cache, or must reflect GitHub now.
///
/// `Cached` lets `gh api --cache 60s` coalesce reads across a burst of `lf`
/// processes. `Fresh` drops that flag so the read reflects GitHub's live state —
/// the ci-fix settlement path needs the authoritative head the repair body just
/// pushed, which a warm cache would hide behind the pre-turn head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrReadFreshness {
    Cached,
    Fresh,
}

/// Read a known PR through one REST call, preserving remote-read uncertainty.
pub(crate) fn observe_pr_by_number(
    repo: &Path,
    number: u32,
    branch: &str,
    freshness: PrReadFreshness,
) -> PrObservation {
    if !gh_available() {
        return PrObservation::Degraded {
            reason: "gh CLI not found".to_string(),
        };
    }
    let Some((owner, name)) = crate::git::worktrees::github_repo_nwo(repo) else {
        return PrObservation::Degraded {
            reason: "could not resolve GitHub owner/repo from the origin remote".to_string(),
        };
    };
    let endpoint = format!("repos/{owner}/{name}/pulls/{number}");
    // `Fresh` drops `--cache` so the read hits GitHub live; `Cached` coalesces a
    // control-command burst into one read for 60s.
    let mut args = vec!["api"];
    if matches!(freshness, PrReadFreshness::Cached) {
        args.extend(["--cache", "60s"]);
    }
    args.extend([
        "-H",
        "Accept: application/vnd.github+json",
        endpoint.as_str(),
    ]);
    let output = match Command::new("gh").current_dir(repo).args(&args).output() {
        Ok(output) => output,
        Err(error) => {
            return PrObservation::Degraded {
                reason: format!("failed to invoke gh while reading PR #{number}: {error}"),
            }
        }
    };
    if !output.status.success() {
        let stderr = stderr_from_output(&output);
        if is_missing_pr(&stderr) {
            return PrObservation::NotFound;
        }
        return PrObservation::Degraded {
            reason: classify_pr_read_failure(number, &stderr),
        };
    }
    match serde_json::from_slice::<GhRestPr>(&output.stdout) {
        Ok(pr) => PrObservation::Fresh(pr.into_info(branch)),
        Err(error) => PrObservation::Degraded {
            reason: format!("failed to parse gh api response for PR #{number}: {error}"),
        },
    }
}

pub(crate) fn observe_pr_by_branch(repo: &Path, branch: &str) -> PrObservation {
    let output = match Command::new("gh")
        .current_dir(repo)
        .args([
            "pr", "list", "--head", branch, "--state", "all", "--limit", "2", "--json", "number",
        ])
        .output()
    {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            return PrObservation::Degraded {
                reason: format!(
                    "could not discover PR for {branch}: {}",
                    stderr_from_output(&output)
                ),
            };
        }
        Err(error) => {
            return PrObservation::Degraded {
                reason: format!("could not discover PR for {branch}: {error}"),
            };
        }
    };
    #[derive(Deserialize)]
    struct Number {
        number: u32,
    }
    let candidates = match serde_json::from_slice::<Vec<Number>>(&output.stdout) {
        Ok(candidates) => candidates,
        Err(error) => {
            return PrObservation::Degraded {
                reason: format!("could not parse PR discovery for {branch}: {error}"),
            };
        }
    };
    match candidates.as_slice() {
        [] => PrObservation::NotFound,
        [candidate] => observe_pr_by_number(repo, candidate.number, branch, PrReadFreshness::Fresh),
        _ => PrObservation::Degraded {
            reason: format!(
                "multiple pull requests use branch {branch}; delivery remains unresolved"
            ),
        },
    }
}

/// A 404 from `gh api` means the PR ref no longer exists — distinct from a
/// quota/network failure, and not something to retry or treat as degraded.
fn is_missing_pr(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("http 404")
}

/// Turn a failed `gh api` read into a concise, user-facing degraded reason. The
/// quota case is called out by name because exhausted API budgets are a
/// recurring dogfood failure.
fn classify_pr_read_failure(number: u32, stderr: &str) -> String {
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("rate limit") || lower.contains("rate-limit") {
        format!("GitHub API rate limit exhausted while reading PR #{number}")
    } else if lower.contains("could not resolve host")
        || lower.contains("network is unreachable")
        || lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("connection refused")
    {
        format!("network failure while reading PR #{number}")
    } else {
        format!(
            "GitHub read for PR #{number} failed: {}",
            stderr.lines().next().unwrap_or("").trim()
        )
    }
}

/// GitHub's REST shape for a single pull request. Not a wire DTO — this
/// deserializes an external API response, mirroring the tolerance of `GhPr`.
#[derive(Debug, Deserialize)]
struct GhRestPr {
    mergeable_state: Option<String>,
    #[serde(default)]
    merged: bool,
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default, rename = "merge_commit_sha")]
    merge_commit_sha: Option<String>,
    #[serde(default)]
    merged_at: Option<String>,
    number: u64,
    #[serde(rename = "html_url")]
    html_url: String,
    head: GhRestHead,
}

#[derive(Debug, Deserialize)]
struct GhRestHead {
    #[serde(default)]
    sha: Option<String>,
}

impl GhRestPr {
    fn into_info(self, branch: &str) -> PrInfo {
        // REST reports only open|closed; a merged PR is closed + merged:true.
        let state = if self.merged {
            "merged".to_string()
        } else if self.state.eq_ignore_ascii_case("closed") {
            "closed".to_string()
        } else if self.draft {
            "draft".to_string()
        } else {
            "open".to_string()
        };
        PrInfo {
            url: self.html_url,
            number: self.number,
            state,
            branch: branch.to_string(),
            merge_commit: if self.merged {
                self.merge_commit_sha
            } else {
                None
            },
            merged_at: if self.merged { self.merged_at } else { None },
            head_sha: self.head.sha,
            merge_state: self.mergeable_state,
        }
    }
}

/// Read one complete check set for the observed head. Required checks decide
/// the gate; the same snapshot's leaf failures supply repair details.
pub(crate) fn merge_gate_state(
    repo: &Path,
    number: u64,
    head_sha: &str,
) -> OpsResult<Option<MergeGateReading>> {
    let mut cursor = None;
    let mut cursors = std::collections::HashSet::new();
    let mut checks = Vec::new();
    loop {
        let page = read_check_page(repo, number, cursor.as_deref())?;
        if page.head != head_sha || page.commit.as_deref() != Some(head_sha) {
            return Ok(None);
        }
        let Some(contexts) = page.contexts else {
            return Ok(None);
        };
        checks.extend(contexts.nodes);
        if !contexts.page_info.has_next_page {
            let producers = match &page.base {
                Some(base) => RequiredProducers::read(repo, &required_contexts_for(repo, base)),
                None => RequiredProducers::default(),
            };
            return Ok(project_checks(checks, &producers));
        }
        let next = contexts.page_info.end_cursor.ok_or_else(|| {
            OpsError::Message(format!(
                "GitHub check page for PR #{number} has no next cursor"
            ))
        })?;
        if !cursors.insert(next.clone()) {
            return Err(OpsError::Message(format!(
                "GitHub check pagination repeated a cursor for PR #{number}"
            )));
        }
        cursor = Some(next);
    }
}

const CHECKS_QUERY: &str = r#"query LoopflowPrChecks($owner:String!,$name:String!,$number:Int!,$endCursor:String) {
  repository(owner:$owner,name:$name) {
    pullRequest(number:$number) {
      headRefOid baseRefName
      commits(last:1) { nodes { commit {
        oid
        statusCheckRollup { contexts(first:100,after:$endCursor) {
          pageInfo { hasNextPage endCursor }
          nodes {
            __typename
            ... on CheckRun {
              name status conclusion detailsUrl startedAt
              isRequired(pullRequestNumber:$number)
              checkSuite { workflowRun { event workflow { name } } }
            }
            ... on StatusContext {
              context state targetUrl
              isRequired(pullRequestNumber:$number)
            }
          }
        } }
      } } }
    }
  }
}"#;

fn read_check_page(repo: &Path, number: u64, cursor: Option<&str>) -> OpsResult<GhCheckPage> {
    let mut command = Command::new("gh");
    command.args([
        "api", "graphql", "-F", "owner={owner}", "-F", "name={repo}",
        "-F", &format!("number={number}"), "-f", &format!("query={CHECKS_QUERY}"),
        "--jq", ".data.repository.pullRequest | {head: .headRefOid, base: .baseRefName, commit: .commits.nodes[0].commit.oid, contexts: .commits.nodes[0].commit.statusCheckRollup.contexts}",
    ]);
    if let Some(cursor) = cursor {
        command.args(["-f", &format!("endCursor={cursor}")]);
    }
    let output = super::read_retry::retry_read("GitHub check page", || {
        let output = command.current_dir(repo).output()?;
        if !output.status.success() {
            return Err(OpsError::CommandFailed {
                command: "GitHub check page".into(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(output)
    })?;
    serde_json::from_slice(&output.stdout).map_err(|error| {
        OpsError::Parse(format!(
            "could not parse GitHub checks for PR #{number}: {error}"
        ))
    })
}

#[derive(Debug, Deserialize)]
struct GhCheckPage {
    head: String,
    base: Option<String>,
    commit: Option<String>,
    contexts: Option<GhCheckContexts>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhCheckContexts {
    nodes: Vec<GhCheckContext>,
    page_info: GhCheckPageInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhCheckPageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__typename", rename_all_fields = "camelCase")]
enum GhCheckContext {
    CheckRun {
        name: String,
        status: String,
        conclusion: Option<String>,
        details_url: Option<String>,
        is_required: bool,
        #[serde(with = "time::serde::rfc3339::option")]
        started_at: Option<time::OffsetDateTime>,
        check_suite: GhCheckSuite,
    },
    StatusContext {
        context: String,
        state: String,
        target_url: Option<String>,
        is_required: bool,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhCheckSuite {
    workflow_run: Option<GhCheckWorkflowRun>,
}

#[derive(Debug, Deserialize)]
struct GhCheckWorkflowRun {
    event: String,
    workflow: Option<GhCheckWorkflow>,
}

#[derive(Debug, Deserialize)]
struct GhCheckWorkflow {
    name: String,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum GhCheckIdentity {
    Run(String, Option<String>, Option<String>),
    Status(String),
}

fn project_checks(
    mut contexts: Vec<GhCheckContext>,
    producers: &RequiredProducers,
) -> Option<MergeGateReading> {
    contexts.sort_by_key(|context| {
        std::cmp::Reverse(match context {
            GhCheckContext::CheckRun { started_at, .. } => *started_at,
            GhCheckContext::StatusContext { .. } => None,
        })
    });
    let mut seen = std::collections::HashSet::new();
    let mut required = Vec::new();
    let mut full = Vec::new();
    let mut fails_required = false;
    for context in contexts {
        let (identity, name, state, link, is_required) = match context {
            GhCheckContext::CheckRun {
                name,
                status,
                conclusion,
                details_url,
                is_required,
                check_suite,
                ..
            } => {
                let (workflow, event) = match check_suite.workflow_run {
                    Some(run) => (run.workflow.map(|workflow| workflow.name), Some(run.event)),
                    None => (None, None),
                };
                let state = if status == "COMPLETED" {
                    conclusion.unwrap_or_default()
                } else {
                    status
                };
                // A failed job a required aggregate needs fails that aggregate
                // before GitHub even creates its check run.
                fails_required |= !is_required
                    && GhCheck::new(name.clone(), &state, None).failed()
                    && producers.fails_required(workflow.as_deref(), &name);
                let identity = GhCheckIdentity::Run(name.clone(), workflow, event);
                (identity, name, state, details_url, is_required)
            }
            GhCheckContext::StatusContext {
                context,
                state,
                target_url,
                is_required,
            } => (
                GhCheckIdentity::Status(context.clone()),
                context,
                state,
                target_url,
                is_required,
            ),
        };
        if !seen.insert(identity) {
            continue;
        }
        let check = GhCheck::new(name, &state, link);
        if is_required {
            required.push(check.clone());
        }
        full.push(check);
    }
    if required.is_empty() && !fails_required {
        return None;
    }
    let mut reading = MergeGateReading::from_checks(required, full);
    reading.failing |= fails_required;
    Some(reading)
}

// Required checks and the jobs that produce them

const REQUIRED_TTL: Duration = Duration::from_secs(3600);

/// Required check names by (checkout, base branch), with when they were read.
type RequiredCache = HashMap<(PathBuf, String), (Instant, BTreeSet<String>)>;

static REQUIRED: LazyLock<Mutex<RequiredCache>> = LazyLock::new(Default::default);

/// Required check names from a `rules/branches/{branch}` or classic
/// `protection/required_status_checks` response.
pub(crate) fn required_contexts(body: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return names;
    };
    let mut collect = |checks: &serde_json::Value| {
        for check in checks.as_array().into_iter().flatten() {
            if let Some(name) = check.as_str().or_else(|| check["context"].as_str()) {
                names.insert(name.to_string());
            }
        }
    };
    for rule in value.as_array().into_iter().flatten() {
        if rule["type"] == "required_status_checks" {
            collect(&rule["parameters"]["required_status_checks"]);
        }
    }
    collect(&value["contexts"]);
    collect(&value["checks"]);
    names
}

/// The base branch's required check names, reread hourly. An unreadable rule
/// set contributes nothing: the landing then waits for the required check itself.
fn required_contexts_for(repo: &Path, base: &str) -> BTreeSet<String> {
    let key = (repo.to_path_buf(), base.to_string());
    if let Some((read_at, names)) = REQUIRED.lock().expect("required cache poisoned").get(&key) {
        if read_at.elapsed() < REQUIRED_TTL {
            return names.clone();
        }
    }
    let mut names = BTreeSet::new();
    for path in [
        format!("repos/{{owner}}/{{repo}}/rules/branches/{base}"),
        format!("repos/{{owner}}/{{repo}}/branches/{base}/protection/required_status_checks"),
    ] {
        if let Ok(output) = Command::new("gh")
            .args(["api", &path])
            .current_dir(repo)
            .output()
        {
            if output.status.success() {
                names.extend(required_contexts(&String::from_utf8_lossy(&output.stdout)));
            }
        }
    }
    REQUIRED
        .lock()
        .expect("required cache poisoned")
        .insert(key, (Instant::now(), names.clone()));
    names
}

/// The workflow jobs whose failure fails a required check: each required
/// check's producing job and everything it transitively `needs`, read from the
/// checkout's `.github/workflows`. A required check with no producer here is
/// judged only when its own check run reports.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RequiredProducers {
    /// (workflow name, job name) pairs.
    jobs: HashSet<(String, String)>,
    /// The required checks some job here produces.
    produced: BTreeSet<String>,
}

#[derive(Debug, Deserialize)]
struct WorkflowFile {
    name: Option<String>,
    #[serde(default)]
    jobs: HashMap<String, WorkflowJob>,
}

#[derive(Debug, Deserialize)]
struct WorkflowJob {
    name: Option<String>,
    #[serde(default)]
    needs: WorkflowNeeds,
}

#[derive(Debug, Default, Deserialize)]
#[serde(untagged)]
enum WorkflowNeeds {
    #[default]
    None,
    One(String),
    Many(Vec<String>),
}

impl RequiredProducers {
    pub(crate) fn read(checkout: &Path, required: &BTreeSet<String>) -> Self {
        let directory = checkout.join(".github").join("workflows");
        let Ok(entries) = std::fs::read_dir(&directory) else {
            return Self::default();
        };
        let mut files: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "yml" || extension == "yaml")
            })
            .filter_map(|path| {
                let source = std::fs::read_to_string(&path).ok()?;
                let relative = path
                    .strip_prefix(checkout)
                    .ok()?
                    .to_string_lossy()
                    .into_owned();
                Some((relative, source))
            })
            .collect();
        files.sort();
        Self::parse(&files, required)
    }

    fn parse(files: &[(String, String)], required: &BTreeSet<String>) -> Self {
        let mut jobs = HashSet::new();
        let mut produced = BTreeSet::new();
        for (path, source) in files {
            let Ok(workflow) = serde_yaml_ng::from_str::<WorkflowFile>(source) else {
                continue;
            };
            // GitHub names an unnamed workflow by its path.
            let workflow_name = workflow.name.clone().unwrap_or_else(|| path.clone());
            let display = |key: &str| -> String {
                workflow.jobs[key]
                    .name
                    .clone()
                    .filter(|name| !name.contains("${{"))
                    .unwrap_or_else(|| key.to_string())
            };
            for key in workflow.jobs.keys() {
                if !required.contains(&display(key)) {
                    continue;
                }
                produced.insert(display(key));
                let mut pending = vec![key.clone()];
                let mut seen = HashSet::new();
                while let Some(key) = pending.pop() {
                    if !seen.insert(key.clone()) {
                        continue;
                    }
                    let Some(job) = workflow.jobs.get(&key) else {
                        continue;
                    };
                    match &job.needs {
                        WorkflowNeeds::None => {}
                        WorkflowNeeds::One(need) => pending.push(need.clone()),
                        WorkflowNeeds::Many(needs) => pending.extend(needs.iter().cloned()),
                    }
                    jobs.insert((workflow_name.clone(), display(&key)));
                }
            }
        }
        Self { jobs, produced }
    }

    /// Whether this check run, by its workflow and job, is one a required check
    /// is made of. Matrix runs (`rust-test (library, 3, 5)`) match their job.
    pub(crate) fn fails_required(&self, workflow: Option<&str>, check: &str) -> bool {
        let Some(workflow) = workflow else {
            return false;
        };
        let job = check.split_once(" (").map_or(check, |(job, _)| job);
        self.jobs.contains(&(workflow.to_string(), job.to_string()))
    }
}

/// The merge-gate reading for one head: whether the required checks block the
/// merge, plus the actionable *leaf* checks to seed a ci-fix turn with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeGateReading {
    pub failing: bool,
    pub pending: bool,
    pub failing_leaves: Vec<GhFailingCheck>,
}

impl MergeGateReading {
    pub(crate) fn from_checks(required: Vec<GhCheck>, full: Vec<GhCheck>) -> Self {
        let gate = RequiredChecks::from_checks(required);
        let required_names: std::collections::HashSet<&str> = gate
            .failing_checks
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        let full_failing: Vec<GhFailingCheck> = full
            .into_iter()
            .filter(|c| matches!(c.bucket.as_str(), "fail" | "cancel"))
            .map(|c| GhFailingCheck {
                name: c.name,
                url: c.link.filter(|link| !link.is_empty()),
            })
            .collect();
        // Drop the required aggregates when at least one non-required leaf also
        // failed — the aggregate's link is the roll-up, not the broken job. When
        // the only failures *are* the required checks, they are genuine leaves
        // (a repo that requires a leaf directly); keep them so the seed is never
        // empty on a real gate failure. When the full read gave nothing, fall
        // back to the required failing checks.
        let leaves: Vec<GhFailingCheck> = full_failing
            .iter()
            .filter(|c| !required_names.contains(c.name.as_str()))
            .cloned()
            .collect();
        let failing_leaves = if !leaves.is_empty() {
            leaves
        } else if !full_failing.is_empty() {
            full_failing
        } else {
            gate.failing_checks.clone()
        };
        Self {
            failing: gate.failing,
            pending: gate.pending,
            failing_leaves,
        }
    }
}

/// The classified required-check reading for one head: overall gate state plus
/// the required checks that are not passing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredChecks {
    pub failing: bool,
    pub pending: bool,
    pub failing_checks: Vec<GhFailingCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhFailingCheck {
    pub name: String,
    pub url: Option<String>,
}

impl RequiredChecks {
    fn from_checks(checks: Vec<GhCheck>) -> Self {
        let mut failing = false;
        let mut pending = false;
        let mut failing_checks = Vec::new();
        for check in checks {
            match check.bucket.as_str() {
                // `cancel` blocks the merge like a failure and needs a re-run or
                // fix, so it counts as failing rather than green.
                "fail" | "cancel" => {
                    failing = true;
                    failing_checks.push(GhFailingCheck {
                        name: check.name,
                        url: check.link.filter(|link| !link.is_empty()),
                    });
                }
                "pending" => pending = true,
                _ => {}
            }
        }
        Self {
            failing,
            pending,
            failing_checks,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GhCheck {
    name: String,
    bucket: String,
    link: Option<String>,
}

impl GhCheck {
    /// Classify one check from GitHub's upper-case state or conclusion.
    pub(crate) fn new(name: String, state: &str, link: Option<String>) -> Self {
        let bucket = match state {
            "SUCCESS" => "pass",
            "SKIPPED" | "NEUTRAL" => "skipping",
            "ERROR" | "FAILURE" | "TIMED_OUT" | "ACTION_REQUIRED" => "fail",
            "CANCELLED" => "cancel",
            _ => "pending",
        };
        Self {
            name,
            bucket: bucket.to_string(),
            link,
        }
    }

    pub(crate) fn failed(&self) -> bool {
        matches!(self.bucket.as_str(), "fail" | "cancel")
    }
}

fn find_open_pr(repo: &Path) -> OpsResult<Option<GhPr>> {
    let branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    find_open_branch_pr(repo, &branch)
}

pub(crate) fn latest_merged_branch_head(repo: &Path, branch: &str) -> OpsResult<Option<String>> {
    if !gh_available() || crate::git::worktrees::github_repo_nwo(repo).is_none() {
        return Ok(None);
    }
    let output = Command::new("gh")
        .args([
            "pr",
            "list",
            "--head",
            branch,
            "--state",
            "merged",
            "--limit",
            "1",
            "--json",
            "state,headRefOid",
        ])
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("read merged PR for {branch}"),
            stderr: stderr_from_output(&output),
        });
    }
    #[derive(Deserialize)]
    struct MergedHead {
        state: String,
        #[serde(rename = "headRefOid")]
        head: Option<String>,
    }
    let prs: Vec<MergedHead> = serde_json::from_slice(&output.stdout)
        .map_err(|error| OpsError::Parse(format!("read merged PR head: {error}")))?;
    Ok(prs
        .into_iter()
        .find(|pr| pr.state.eq_ignore_ascii_case("merged"))
        .and_then(|pr| pr.head))
}

fn find_open_branch_pr(repo: &Path, branch: &str) -> OpsResult<Option<GhPr>> {
    let output = Command::new("gh")
        .arg("pr")
        .arg("list")
        .arg("--head")
        .arg(branch)
        .arg("--json")
        .arg("url,state,isDraft,number,mergeCommit,headRefOid")
        .current_dir(repo)
        .output()?;

    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("gh pr list --head {branch}"),
            stderr: stderr_from_output(&output),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let list: Vec<GhPr> = serde_json::from_str(&stdout)
        .map_err(|e| OpsError::Message(format!("failed to parse gh pr list output: {e}")))?;
    let open = list
        .into_iter()
        .find(|pr| pr.state.to_uppercase() == "OPEN");
    Ok(open)
}

fn update_pr(repo: &Path, number: u64, title: &str, body: &str, base: &str) -> OpsResult<()> {
    let output = Command::new("gh")
        .arg("pr")
        .arg("edit")
        .arg(number.to_string())
        .arg("--title")
        .arg(title)
        .arg("--body")
        .arg(body)
        .arg("--base")
        .arg(base)
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "gh pr edit".to_string(),
            stderr: stderr_from_output(&output),
        });
    }
    Ok(())
}

pub(crate) fn retarget_open_pr(
    repo: &Path,
    base: &str,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<()> {
    let Some(pr) = find_open_pr(repo)? else {
        return Ok(());
    };
    let mut cmd = Command::new("gh");
    cmd.arg("pr")
        .arg("edit")
        .arg(pr.number.to_string())
        .arg("--base")
        .arg(base)
        .current_dir(repo);
    inherit_pr(&mut cmd);
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "gh pr edit --base".to_string(),
            stderr: stderr_from_output(&output),
        });
    }
    Ok(())
}

fn mark_pr_ready(repo: &Path, pr: &mut GhPr) -> OpsResult<()> {
    if !pr.is_draft {
        return Ok(());
    }
    let output = Command::new("gh")
        .arg("pr")
        .arg("ready")
        .arg(pr.number.to_string())
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "gh pr ready".to_string(),
            stderr: stderr_from_output(&output),
        });
    }
    pr.is_draft = false;
    Ok(())
}

fn create_pr(
    repo: &Path,
    title: &str,
    body: &str,
    base: &str,
    draft: bool,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<String> {
    let mut cmd = Command::new("gh");
    cmd.arg("pr")
        .arg("create")
        .arg("--title")
        .arg(title)
        .arg("--body")
        .arg(body)
        .arg("--base")
        .arg(base);
    inherit_pr(&mut cmd);
    if draft {
        cmd.arg("--draft");
    }
    let output = cmd.current_dir(repo).output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "gh pr create".to_string(),
            stderr: stderr_from_output(&output),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Create the review surface for a branch already committed, integrated, and
/// pushed by submit/land. This deliberately performs no Git mutation.
pub(crate) fn create_pr_from_pushed_branch(
    repo: &Path,
    title: &str,
    body: &str,
    base: &str,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<PrInfo> {
    let url = create_pr(repo, title, body, base, false, inherit_pr)?;
    let number = pr_number_from_url(&url).ok_or_else(|| {
        OpsError::Message(format!("could not read PR number from created URL {url}"))
    })?;
    let branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    Ok(PrInfo {
        url,
        number,
        state: "open".to_string(),
        branch,
        merge_commit: None,
        merged_at: None,
        head_sha: Some(rev_parse(repo, "HEAD")?),
        merge_state: None,
    })
}

fn resolve_main_repo(repo: &Path) -> PathBuf {
    main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf())
}

fn pr_target(repo: &Path, main_repo: &Path, default_branch: &str) -> OpsResult<String> {
    let current_branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;

    if let Ok(worktrees) = list_worktrees(main_repo) {
        if let Some(state) = worktrees
            .into_iter()
            .find(|wt| wt.branch.as_deref() == Some(&current_branch))
        {
            if let Some(base_branch) = state.base_branch {
                if base_branch != current_branch {
                    return resolve_pr_target(repo, &base_branch);
                }
            }
        }
    }

    Ok(default_branch.to_string())
}

fn resolve_pr_target(repo: &Path, base_branch: &str) -> OpsResult<String> {
    if base_branch == "main" {
        return Ok("main".to_string());
    }

    let output = Command::new("gh")
        .arg("pr")
        .arg("view")
        .arg(base_branch)
        .arg("--json")
        .arg("state")
        .arg("-q")
        .arg(".state")
        .current_dir(repo)
        .output()?;
    if output.status.success() {
        let state = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_uppercase();
        if state == "MERGED" {
            return Ok("main".to_string());
        }
    }
    Ok(base_branch.to_string())
}

fn parse_generated_pr_copy(raw: &str) -> Option<PrCopy> {
    parse_json_copy(raw)
        .or_else(|| extract_fenced_json(raw).and_then(parse_json_copy))
        .or_else(|| {
            let mut candidates = extract_json_candidates(raw).collect::<Vec<_>>();
            candidates.reverse();
            candidates.into_iter().find_map(|candidate| {
                parse_json_copy(candidate).or_else(|| parse_loose_json_copy(candidate))
            })
        })
        .or_else(|| extract_last_object_like_candidate(raw).and_then(parse_loose_json_copy))
        .or_else(|| parse_labeled_copy(raw))
}

fn parse_json_copy(raw: &str) -> Option<PrCopy> {
    let mut copy: PrCopy = serde_json::from_str(raw.trim()).ok()?;
    copy.title = copy.title.trim().to_string();
    if copy.title.is_empty() || is_placeholder_pr_copy(&copy.title, &copy.body) {
        return None;
    }
    Some(copy)
}

fn parse_labeled_copy(raw: &str) -> Option<PrCopy> {
    #[derive(Clone, Copy)]
    enum Section {
        Title,
        Body,
    }

    let mut section = None;
    let mut title = None;
    let mut body_lines = Vec::new();

    for line in raw.lines() {
        if let Some((label, remainder)) = parse_labeled_line(line) {
            match label {
                "title" => {
                    if !remainder.is_empty() {
                        title = Some(remainder.to_string());
                        section = None;
                    } else {
                        section = Some(Section::Title);
                    }
                }
                "body" => {
                    body_lines.clear();
                    if !remainder.is_empty() {
                        body_lines.push(remainder.to_string());
                    }
                    section = Some(Section::Body);
                }
                _ => {}
            }
            continue;
        }

        match section {
            Some(Section::Title) if !line.trim().is_empty() => {
                title = Some(line.trim().to_string());
                section = None;
            }
            Some(Section::Title) => {}
            Some(Section::Body) => body_lines.push(line.to_string()),
            None => {}
        }
    }

    let title = title?.trim().to_string();
    if title.is_empty() {
        return None;
    }

    Some(PrCopy {
        title,
        body: body_lines.join("\n").trim_matches('\n').to_string(),
    })
}

fn parse_labeled_line(line: &str) -> Option<(&'static str, &str)> {
    for label in ["title", "body"] {
        if let Some(remainder) = match_label(line, label) {
            return Some((label, remainder));
        }
    }
    None
}

fn match_label<'a>(line: &'a str, label: &str) -> Option<&'a str> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let bare = trimmed
        .trim_start_matches(['#', '-', '*', ' '])
        .trim_end_matches(['*', ':', ' ']);
    if bare.eq_ignore_ascii_case(label) {
        return Some("");
    }

    let colon_index = trimmed.find(':')?;
    let (prefix, remainder) = trimmed.split_at(colon_index);
    let prefix = prefix.trim().trim_matches('*');
    if !prefix.eq_ignore_ascii_case(label) {
        return None;
    }
    Some(remainder[1..].trim())
}

fn extract_fenced_json(raw: &str) -> Option<&str> {
    let start = raw.find("```json")?;
    let rest = &raw[start + "```json".len()..];
    let end = rest.find("```")?;
    Some(rest[..end].trim())
}

fn extract_json_candidates(raw: &str) -> impl Iterator<Item = &str> {
    let mut candidates = Vec::new();
    let mut start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape = false;

    for (idx, ch) in raw.char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }

            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(idx);
                }
                depth += 1;
            }
            '}' => {
                if depth == 0 {
                    continue;
                }
                depth -= 1;
                if depth == 0 {
                    if let Some(object_start) = start {
                        candidates.push(&raw[object_start..=idx]);
                    }
                    start = None;
                }
            }
            _ => {}
        }
    }

    candidates.into_iter()
}

fn extract_last_object_like_candidate(raw: &str) -> Option<&str> {
    let title_idx = raw.rfind("\"title\"");
    let body_idx = raw.rfind("\"body\"");
    let key_idx = title_idx.into_iter().chain(body_idx).max()?;
    let start = raw[..key_idx].rfind('{')?;
    let end = raw[key_idx..].rfind('}')? + key_idx;
    Some(&raw[start..=end])
}

fn parse_loose_json_copy(raw: &str) -> Option<PrCopy> {
    let raw = raw.trim();
    if !raw.starts_with('{') || !raw.ends_with('}') {
        return None;
    }

    let title = extract_loose_field(raw, "title", false)?.trim().to_string();
    let body = extract_loose_field(raw, "body", true)?;
    if title.is_empty() || is_placeholder_pr_copy(&title, &body) {
        return None;
    }
    Some(PrCopy { title, body })
}

fn is_placeholder_pr_copy(title: &str, body: &str) -> bool {
    title.trim() == "..." && body.trim() == "..."
}

fn format_pr_copy_parse_preview(raw: &str) -> String {
    const MAX_CHARS: usize = 400;
    let preview = raw.trim();
    if preview.is_empty() {
        return "Agent output was empty.".to_string();
    }

    let truncated = if preview.chars().count() > MAX_CHARS {
        let end = preview
            .char_indices()
            .nth(MAX_CHARS)
            .map(|(idx, _)| idx)
            .unwrap_or(preview.len());
        format!("{}…", &preview[..end])
    } else {
        preview.to_string()
    };
    format!("Output preview:\n{truncated}")
}

fn extract_loose_field(raw: &str, key: &str, allow_object_end: bool) -> Option<String> {
    let needle = format!("\"{key}\"");
    let key_start = raw.find(&needle)?;
    let after_key = &raw[key_start + needle.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();
    let opening_quote = after_colon.find('"')?;
    let value = &after_colon[opening_quote + 1..];

    let end = if allow_object_end {
        let object_end = value.rfind('}')?;
        value[..object_end].rfind('"')?
    } else {
        find_loose_field_end(value)?
    };

    decode_loose_json_string(&value[..end])
}

fn find_loose_field_end(raw: &str) -> Option<usize> {
    for (idx, ch) in raw.char_indices() {
        if ch != '"' {
            continue;
        }

        let next = raw[idx + ch.len_utf8()..]
            .chars()
            .find(|candidate| !candidate.is_whitespace());
        if next.is_none_or(|candidate| candidate == ',' || candidate == '}') {
            return Some(idx);
        }
    }
    None
}

fn decode_loose_json_string(raw: &str) -> Option<String> {
    let mut decoded = String::new();
    let mut chars = raw.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }

        let escaped = chars.next()?;
        decoded.push(match escaped {
            '"' => '"',
            '\\' => '\\',
            '/' => '/',
            'b' => '\u{0008}',
            'f' => '\u{000C}',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            other => other,
        });
    }

    Some(decoded)
}

fn git_stdout(repo: &Path, args: &[&str]) -> OpsResult<String> {
    let output = Command::new("git").args(args).current_dir(repo).output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("git {}", args.join(" ")),
            stderr: stderr_from_output(&output),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut iter = text.char_indices();
    if iter.nth(max_chars).is_none() {
        return text.to_string();
    }
    let end = text
        .char_indices()
        .nth(max_chars)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len());
    format!("{}\n\n[diff truncated]", &text[..end])
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn merge_policy_uses_allowed_methods_and_rejects_disabled_preferences() {
        use crate::config::MergeMethod;
        let methods = super::MergeMethods {
            merge_commit_allowed: true,
            squash_merge_allowed: false,
            rebase_merge_allowed: true,
        };
        assert_eq!(methods.select(None).unwrap(), MergeMethod::Merge);
        assert_eq!(
            methods.select(Some(MergeMethod::Rebase)).unwrap(),
            MergeMethod::Rebase
        );
        assert!(methods.select(Some(MergeMethod::Squash)).is_err());
        let none = super::MergeMethods {
            merge_commit_allowed: false,
            squash_merge_allowed: false,
            rebase_merge_allowed: false,
        };
        assert!(none.select(None).is_err());
    }

    #[test]
    fn release_merge_observation_respects_queue_membership() {
        for (entry, auto, queue_enabled, behind, dirty) in [
            (
                serde_json::json!({"id":"queue"}),
                serde_json::Value::Null,
                true,
                false,
                false,
            ),
            (
                serde_json::Value::Null,
                serde_json::json!({"enabledAt":"now"}),
                true,
                false,
                true,
            ),
            (
                serde_json::Value::Null,
                serde_json::json!({"enabledAt":"now"}),
                false,
                true,
                true,
            ),
            (
                serde_json::Value::Null,
                serde_json::Value::Null,
                false,
                true,
                true,
            ),
        ] {
            let value = serde_json::json!({"data":{"repository":{"mergeCommitAllowed":true,"squashMergeAllowed":true,"rebaseMergeAllowed":true,"pullRequest":{
                "id":"pr", "number":1, "url":"https://example.test/pr/1", "state":"OPEN", "isDraft":false,
                "headRefName":"release", "headRefOid":"exact-head", "mergedAt":null, "mergeCommit":null,
                "mergeStateStatus":"BEHIND", "isMergeQueueEnabled":queue_enabled, "isInMergeQueue":false,
                "autoMergeRequest":auto, "mergeQueueEntry":entry
            }}}});
            let mut observation =
                super::parse_pr_merge(&serde_json::to_vec(&value).unwrap()).unwrap();
            let unknown = || Ok(super::MergeRequestRemoval::Unknown);
            assert_eq!(
                super::pr_requires_resync(&observation, unknown).unwrap(),
                behind
            );
            observation.pr.merge_state = Some("dirty".into());
            assert_eq!(
                super::pr_requires_resync(&observation, unknown).unwrap(),
                dirty
            );
        }
    }

    use super::{
        classify_pr_read_failure, disable_auto_merge, enable_auto_merge, is_missing_pr,
        merge_gate_state, normalize_task_pr_copy, observe_merge_request, parse_generated_pr_copy,
        parse_pr_merge, pr_number_from_url, pr_requires_resync, project_checks, required_contexts,
        GhCheck, GhRestHead, GhRestPr, MergeGateReading, MergeRequest, MergeRequestRemoval, PrCopy,
        PrInfo, PrMergeObservation, RequiredChecks, RequiredProducers, TaskPrCopyLifecycle,
    };
    use crate::ops::task::TaskPrContext;
    use serde_json::{json, Value};

    struct CheckFixture {
        directory: tempfile::TempDir,
        path: Option<OsString>,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl CheckFixture {
        fn new(first: Value, second: Value) -> Self {
            let lock = crate::journal::test_env_lock();
            let directory = tempfile::tempdir().unwrap();
            std::fs::write(directory.path().join("first.json"), first.to_string()).unwrap();
            std::fs::write(directory.path().join("second.json"), second.to_string()).unwrap();
            let script = directory.path().join("gh");
            std::fs::write(
                &script,
                r#"#!/bin/sh
fixture="$(dirname "$0")"
if [ -f "$fixture/transient" ]; then
  rm "$fixture/transient"
  echo "HTTP 502: Bad Gateway" >&2
  exit 1
fi
case "$*" in
  *endCursor=*)
    cat "$fixture/second.json"
    if [ -f "$fixture/fail" ]; then echo 'GitHub unavailable' >&2; exit 1; fi ;;
  *) cat "$fixture/first.json" ;;
esac
"#,
            )
            .unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            let path = std::env::var_os("PATH");
            let paths = std::iter::once(directory.path().to_path_buf())
                .chain(std::env::split_paths(path.as_deref().unwrap_or_default()));
            std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
            Self {
                directory,
                path,
                _lock: lock,
            }
        }

        fn read(&self) -> crate::ops::error::OpsResult<Option<MergeGateReading>> {
            merge_gate_state(self.directory.path(), 1325, "head-1")
        }
    }

    impl Drop for CheckFixture {
        fn drop(&mut self) {
            match &self.path {
                Some(path) => std::env::set_var("PATH", path),
                None => std::env::remove_var("PATH"),
            }
        }
    }

    #[test]
    fn check_page_recovers_502_without_accepting_a_changed_head() {
        for head in ["head-1", "head-2"] {
            let fixture = CheckFixture::new(
                page(
                    head,
                    vec![run("test", "SUCCESS", true, "2026-09-29T00:00:00Z")],
                    None,
                ),
                page(head, vec![], None),
            );
            std::fs::write(fixture.directory.path().join("transient"), "").unwrap();
            assert_eq!(fixture.read().unwrap().is_some(), head == "head-1");
        }
    }

    fn page(head: &str, nodes: Vec<Value>, next: Option<&str>) -> Value {
        json!({"head":head,"commit":head,"contexts":{
            "nodes":nodes,"pageInfo":{"hasNextPage":next.is_some(),"endCursor":next}
        }})
    }

    fn run(name: &str, conclusion: &str, required: bool, started: &str) -> Value {
        json!({"__typename":"CheckRun", "name":name,"status":"COMPLETED",
            "conclusion":conclusion,"isRequired":required,"startedAt":started,
            "detailsUrl":format!("https://ci/{name}"),
            "checkSuite":{"workflowRun":{"event":"pull_request","workflow":{"name":"CI"}}}
        })
    }

    #[test]
    fn one_rule_decides_when_a_pr_needs_main_merged() {
        let evicted = |reason: &str| MergeRequestRemoval::Evicted {
            reason: reason.to_string(),
            run_url: None,
        };
        // (request, queue, removal, behind, dirty, clean)
        for (request, queue, removal, behind, dirty, clean) in [
            (None, false, MergeRequestRemoval::Unknown, true, true, false),
            (
                Some(MergeRequest::Auto),
                false,
                MergeRequestRemoval::Unknown,
                true,
                true,
                false,
            ),
            // A repair disarm on a queue: the queue brings the base up to date.
            (
                None,
                true,
                MergeRequestRemoval::ByLoopflow,
                false,
                true,
                false,
            ),
            (
                Some(MergeRequest::AwaitingQueue),
                true,
                evicted("merge_conflict"),
                false,
                true,
                false,
            ),
            (None, true, evicted("merge_conflict"), true, true, true),
            (None, true, evicted("failed_checks"), false, true, false),
            (
                Some(MergeRequest::Queued("pr-id".to_string())),
                true,
                MergeRequestRemoval::Unknown,
                false,
                false,
                false,
            ),
        ] {
            let take = || Ok(removal.clone());
            let mut observation = PrMergeObservation::fixture(
                PrInfo {
                    url: "https://example.com/pr/1".into(),
                    number: 1,
                    state: "open".into(),
                    branch: "feature".into(),
                    merge_commit: None,
                    merged_at: None,
                    head_sha: Some("head".into()),
                    merge_state: None,
                },
                request,
                queue,
            );
            for (state, expected) in [("BEHIND", behind), ("dirty", dirty), ("clean", clean)] {
                observation.pr.merge_state = Some(state.into());
                assert_eq!(
                    pr_requires_resync(&observation, take).unwrap(),
                    expected,
                    "{state} {:?} queue={queue}",
                    observation.request
                );
            }
        }
    }

    fn merge_response(state: &str, request: Value, queue: Value) -> Value {
        json!({"data":{"repository":{"mergeCommitAllowed":true,"squashMergeAllowed":true,"rebaseMergeAllowed":true,"pullRequest":{
            "id":"PR_fixture", "number":1329, "url":"https://example.com/pr/1329",
            "state":state, "isDraft":false, "headRefName":"feature", "headRefOid":"head-1",
            "mergedAt":"2026-09-29T05:02:15Z", "mergeCommit":{"oid":"merge-1"},
            "mergeStateStatus":"BEHIND", "isMergeQueueEnabled":true, "isInMergeQueue":!queue.is_null(),
            "autoMergeRequest":request, "mergeQueueEntry":queue
        }}}})
    }

    #[test]
    fn merged_observation_retains_evidence_after_auto_merge_is_removed() {
        let response = merge_response("MERGED", Value::Null, Value::Null);
        let result = parse_pr_merge(&serde_json::to_vec(&response).unwrap()).unwrap();
        assert_eq!(result.pr.state, "merged");
        assert_eq!(result.pr.merge_commit.as_deref(), Some("merge-1"));
        assert_eq!(result.pr.merged_at.as_deref(), Some("2026-09-29T05:02:15Z"));
        assert!(result.request.is_none());
    }

    #[test]
    fn partial_merge_observations_never_authorize_landing_decisions() {
        let complete = merge_response("OPEN", Value::Null, Value::Null);
        let mut partial = complete.clone();
        partial["errors"] = json!([{"message":"Rate limit exceeded"}]);
        let mut missing_request = complete.clone();
        missing_request["data"]["repository"]["pullRequest"]
            .as_object_mut()
            .unwrap()
            .remove("autoMergeRequest");
        for response in [
            json!(null),
            json!({"data":{"repository":{"pullRequest":null}}}),
            partial,
            missing_request,
            merge_response("UNKNOWN", Value::Null, Value::Null),
            merge_response("OPEN", json!(false), Value::Null),
            merge_response("OPEN", Value::Null, json!([])),
        ] {
            assert!(
                parse_pr_merge(&serde_json::to_vec(&response).unwrap()).is_err(),
                "accepted {response}"
            );
        }
    }

    #[test]
    fn queued_observation_keeps_the_pull_request_id_for_cancellation() {
        let response = merge_response("OPEN", Value::Null, json!({"id":"QUEUE_entry"}));
        let result = parse_pr_merge(&serde_json::to_vec(&response).unwrap()).unwrap();
        assert!(matches!(result.request, Some(MergeRequest::Queued(id)) if id == "PR_fixture"));
    }

    #[test]
    fn waiting_for_queue_is_an_active_request_that_can_be_revoked() {
        let fixture = CheckFixture::new(
            merge_response(
                "OPEN",
                json!({"enabledAt":"2026-09-29T00:00:00Z"}),
                Value::Null,
            ),
            merge_response("OPEN", Value::Null, Value::Null),
        );
        std::fs::write(fixture.directory.path().join("armed"), "").unwrap();
        std::fs::write(
            fixture.directory.path().join("gh"),
            r#"#!/bin/sh
fixture="$(dirname "$0")"
if [ -f "$fixture/transient" ]; then
  rm "$fixture/transient"
  echo "HTTP 502: Bad Gateway" >&2
  exit 1
fi
case "$*" in
  *--disable-auto*) rm "$fixture/armed" ;;
  *) if [ -f "$fixture/armed" ]; then cat "$fixture/first.json"; else cat "$fixture/second.json"; fi ;;
esac
"#,
        )
        .unwrap();
        let repo = fixture.directory.path();
        assert!(matches!(
            observe_merge_request(repo, 1329).unwrap(),
            Some(MergeRequest::AwaitingQueue)
        ));
        disable_auto_merge(repo, 1329, &|_| {}).unwrap();
        assert!(observe_merge_request(repo, 1329).unwrap().is_none());
    }

    /// A fake gh whose GraphQL read reports `first.json` while `$fixture/<state>`
    /// exists and `second.json` otherwise; `on_merge` handles `gh pr merge`.
    fn arm_fixture(first: Value, second: Value, state: &str, on_merge: &str) -> CheckFixture {
        let fixture = CheckFixture::new(first, second);
        std::fs::write(
            fixture.directory.path().join("gh"),
            format!(
                r#"#!/bin/sh
fixture="$(dirname "$0")"
case "$*" in
  *dequeuePullRequest*) rm -f "$fixture/{state}" ;;
  "pr merge"*) {on_merge} ;;
  *) if [ -f "$fixture/{state}" ]; then cat "$fixture/first.json"; else cat "$fixture/second.json"; fi ;;
esac
"#
            ),
        )
        .unwrap();
        fixture
    }

    #[test]
    fn merge_queue_membership_without_an_entry_is_a_queued_request() {
        let mut response = merge_response("OPEN", Value::Null, Value::Null);
        response["data"]["repository"]["pullRequest"]["isInMergeQueue"] = json!(true);
        let result = parse_pr_merge(&serde_json::to_vec(&response).unwrap()).unwrap();
        assert!(matches!(result.request, Some(MergeRequest::Queued(id)) if id == "PR_fixture"));
    }

    #[test]
    fn arming_keeps_a_queue_entry_for_the_same_head() {
        let mut queued = merge_response("OPEN", Value::Null, Value::Null);
        queued["data"]["repository"]["pullRequest"]["isInMergeQueue"] = json!(true);
        let fixture = arm_fixture(
            queued,
            merge_response("OPEN", Value::Null, Value::Null),
            "queued",
            "rm -f \"$fixture/queued\"",
        );
        let repo = fixture.directory.path();
        std::fs::write(repo.join("queued"), "").unwrap();
        enable_auto_merge(repo, 1329, None, "head-1", &|_| {}).unwrap();
        assert!(matches!(
            observe_merge_request(repo, 1329).unwrap(),
            Some(MergeRequest::Queued(_))
        ));
    }

    #[test]
    fn arming_asks_again_when_github_ignores_the_first_request() {
        let fixture = arm_fixture(
            merge_response("OPEN", json!({"enabledAt":"now"}), Value::Null),
            merge_response("OPEN", Value::Null, Value::Null),
            "armed",
            "if [ -f \"$fixture/asked\" ]; then touch \"$fixture/armed\"; else touch \"$fixture/asked\"; fi",
        );
        let repo = fixture.directory.path();
        enable_auto_merge(repo, 1329, None, "head-1", &|_| {}).unwrap();
        assert!(observe_merge_request(repo, 1329).unwrap().is_some());
    }

    #[test]
    fn arming_reports_githubs_answer_when_nothing_is_armed() {
        let fixture = arm_fixture(
            merge_response("OPEN", json!({"enabledAt":"now"}), Value::Null),
            merge_response("OPEN", Value::Null, Value::Null),
            "armed",
            "echo '! Pull request #1329 is already queued to merge' >&2",
        );
        let error = enable_auto_merge(fixture.directory.path(), 1329, None, "head-1", &|_| {})
            .unwrap_err()
            .to_string();
        assert!(error.contains("no auto-merge request"), "{error}");
        assert!(error.contains("already queued to merge"), "{error}");
    }

    #[test]
    fn check_read_includes_required_failures_on_later_pages() {
        let server = CheckFixture::new(
            page(
                "head-1",
                vec![run("lint", "SUCCESS", true, "2026-09-29T00:00:00Z")],
                Some("next"),
            ),
            page(
                "head-1",
                vec![run("test", "FAILURE", true, "2026-09-29T00:00:00Z")],
                None,
            ),
        );
        let reading = server.read().unwrap().unwrap();
        assert!(reading.failing);
        assert_eq!(reading.failing_leaves[0].name, "test");
    }

    #[test]
    fn changed_head_and_failed_pages_cannot_reuse_partial_green_checks() {
        let green = page(
            "head-1",
            vec![run("test", "SUCCESS", true, "2026-09-29T00:00:00Z")],
            Some("next"),
        );
        let mut changed_commit = page("head-1", vec![], None);
        changed_commit["commit"] = json!("head-2");
        for second in [page("head-2", vec![], None), changed_commit] {
            let server = CheckFixture::new(green.clone(), second);
            assert!(server.read().unwrap().is_none());
        }
        let server = CheckFixture::new(green, page("head-1", vec![], None));
        std::fs::write(server.directory.path().join("fail"), "").unwrap();
        assert!(server
            .read()
            .unwrap_err()
            .to_string()
            .contains("GitHub check page: read failed after 1 attempt"));
    }

    #[test]
    fn missing_checks_and_unreadable_or_incomplete_pages_remain_distinct() {
        for first in [
            page("head-1", vec![], None),
            json!({"head":"head-1","commit":"head-1","contexts":null}),
        ] {
            let server = CheckFixture::new(first, Value::Null);
            assert!(server.read().unwrap().is_none());
        }
        for first in [
            json!("invalid response"),
            json!({"head":"head-1","commit":"head-1","contexts":{"nodes":[],"pageInfo":{"hasNextPage":true,"endCursor":null}}}),
        ] {
            let server = CheckFixture::new(first, Value::Null);
            assert!(server.read().is_err());
        }
        let repeated = page("head-1", vec![], Some("same-cursor"));
        let server = CheckFixture::new(repeated.clone(), repeated);
        assert!(server
            .read()
            .unwrap_err()
            .to_string()
            .contains("repeated a cursor"));
    }

    #[test]
    fn check_reruns_keep_the_latest_result_without_merging_workflows_or_events() {
        let old = run("test", "FAILURE", true, "2026-09-28T00:00:00Z");
        let new = run("test", "SUCCESS", true, "2026-09-29T00:00:00Z");
        let mut other_workflow = old.clone();
        other_workflow["isRequired"] = json!(false);
        other_workflow["checkSuite"]["workflowRun"]["workflow"]["name"] = json!("Other");
        other_workflow["detailsUrl"] = json!("https://ci/other");
        let mut other_event = other_workflow.clone();
        other_event["checkSuite"]["workflowRun"]["workflow"]["name"] = json!("CI");
        other_event["checkSuite"]["workflowRun"]["event"] = json!("push");
        other_event["detailsUrl"] = json!("https://ci/push");
        let contexts =
            serde_json::from_value(json!([old, new, other_workflow, other_event])).unwrap();
        let reading = project_checks(contexts, &RequiredProducers::default()).unwrap();
        assert!(!reading.failing);
        let urls: Vec<_> = reading
            .failing_leaves
            .iter()
            .map(|c| c.url.as_deref())
            .collect();
        assert_eq!(
            urls,
            vec![Some("https://ci/other"), Some("https://ci/push")]
        );
    }

    #[test]
    fn context_states_preserve_gate_and_optional_failure_meanings() {
        for (state, failing, pending) in [
            ("SUCCESS", false, false),
            ("SKIPPED", false, false),
            ("NEUTRAL", false, false),
            ("CANCELLED", true, false),
            ("ACTION_REQUIRED", true, false),
            ("TIMED_OUT", true, false),
            ("FAILURE", true, false),
            ("FUTURE_STATE", false, true),
        ] {
            let required = run("required", state, true, "2026-09-29T00:00:00Z");
            let optional = run("optional", "FAILURE", false, "2026-09-29T00:00:00Z");
            let reading = project_checks(
                serde_json::from_value(json!([required, optional])).unwrap(),
                &RequiredProducers::default(),
            )
            .unwrap();
            assert_eq!(
                (reading.failing, reading.pending),
                (failing, pending),
                "{state}"
            );
            assert_eq!(reading.failing_leaves[0].name, "optional");
        }
        let mut pending = run("required", "FAILURE", true, "2026-09-29T00:00:00Z");
        pending["status"] = json!("IN_PROGRESS");
        let reading = project_checks(
            serde_json::from_value(json!([pending])).unwrap(),
            &RequiredProducers::default(),
        )
        .unwrap();
        assert!(!reading.failing);
        assert!(reading.pending);
    }

    #[test]
    fn completed_required_failure_is_actionable_while_other_checks_run() {
        let failed = run("lint", "FAILURE", true, "2026-10-17T00:00:00Z");
        let mut pending = run("integration", "SUCCESS", true, "2026-10-17T00:00:00Z");
        pending["status"] = json!("IN_PROGRESS");
        pending["conclusion"] = json!(null);
        let reading = project_checks(
            serde_json::from_value(json!([failed, pending])).unwrap(),
            &RequiredProducers::default(),
        )
        .unwrap();
        assert!(reading.failing);
        assert!(reading.pending);
        assert_eq!(reading.failing_leaves.len(), 1);
        assert_eq!(reading.failing_leaves[0].name, "lint");
    }

    const WORKFLOW: &str = r#"
name: CI
on: [pull_request]
jobs:
  scratch-clear:
    runs-on: ubuntu-latest
  rust-build:
    needs: scratch-clear
    runs-on: ubuntu-latest
  rust-test:
    needs: [scratch-clear, rust-build]
    strategy:
      matrix:
        shard: [0, 1]
    runs-on: ubuntu-latest
  docs:
    runs-on: ubuntu-latest
  tests-result:
    if: ${{ always() }}
    needs: [rust-test]
    runs-on: ubuntu-latest
"#;

    fn producers() -> RequiredProducers {
        RequiredProducers::parse(
            &[(".github/workflows/ci.yml".into(), WORKFLOW.into())],
            &["tests-result".to_string()].into(),
        )
    }

    #[test]
    fn required_checks_come_from_rulesets_or_classic_protection() {
        let rules = r#"[{"type":"deletion"},{"type":"required_status_checks","parameters":{"required_status_checks":[{"context":"tests-result","integration_id":15368}]}}]"#;
        assert_eq!(
            required_contexts(rules),
            ["tests-result".to_string()].into()
        );
        let classic = r#"{"contexts":["tests-result"],"checks":[{"context":"lint","app_id":1}]}"#;
        assert_eq!(
            required_contexts(classic),
            ["lint".to_string(), "tests-result".to_string()].into()
        );
        assert!(required_contexts(r#"{"message":"Branch not protected"}"#).is_empty());
    }

    #[test]
    fn required_producers_follow_needs_and_match_matrix_runs() {
        let producers = producers();
        for job in [
            "tests-result",
            "rust-test (0)",
            "rust-build",
            "scratch-clear",
        ] {
            assert!(producers.fails_required(Some("CI"), job), "{job}");
        }
        assert!(!producers.fails_required(Some("CI"), "docs"));
        assert!(!producers.fails_required(Some("Other"), "rust-build"));
        assert!(!producers.fails_required(None, "rust-build"));
    }

    #[test]
    fn a_needed_job_fails_the_gate_before_its_aggregate_reports() {
        // GitHub creates no `tests-result` run until everything it needs finishes.
        let contexts = json!([
            run("rust-test (1)", "FAILURE", false, "2026-10-21T00:00:00Z"),
            run("docs", "FAILURE", false, "2026-10-21T00:00:00Z"),
        ]);
        assert!(
            project_checks(
                serde_json::from_value(contexts.clone()).unwrap(),
                &RequiredProducers::default()
            )
            .is_none(),
            "without producers the gate is unreported"
        );
        let reading =
            project_checks(serde_json::from_value(contexts).unwrap(), &producers()).unwrap();
        assert!(reading.failing);
        let leaves: Vec<_> = reading
            .failing_leaves
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(leaves, ["rust-test (1)", "docs"]);
        // An optional failure alone still leaves the gate unreported.
        let optional = json!([run("docs", "FAILURE", false, "2026-10-21T00:00:00Z")]);
        assert!(project_checks(serde_json::from_value(optional).unwrap(), &producers()).is_none());
    }

    #[test]
    fn legacy_status_contexts_are_distinct_from_same_named_check_runs() {
        let contexts = json!([
            run("test", "SUCCESS", false, "2026-09-29T00:00:00Z"),
            {"__typename":"StatusContext","context":"test","state":"ERROR",
             "targetUrl":"https://legacy/test","isRequired":true},
            {"__typename":"StatusContext","context":"test","state":"SUCCESS",
             "targetUrl":"https://legacy/older","isRequired":true}
        ]);
        let reading = project_checks(
            serde_json::from_value(contexts).unwrap(),
            &RequiredProducers::default(),
        )
        .unwrap();
        assert!(reading.failing);
        assert_eq!(
            reading.failing_leaves[0].url.as_deref(),
            Some("https://legacy/test")
        );
    }

    fn check(name: &str, bucket: &str) -> GhCheck {
        GhCheck {
            name: name.to_string(),
            bucket: bucket.to_string(),
            link: Some(format!("https://ci/{name}")),
        }
    }

    fn rest_pr(state: &str, merged: bool, draft: bool) -> GhRestPr {
        GhRestPr {
            mergeable_state: None,
            merged,
            state: state.to_string(),
            draft,
            merge_commit_sha: merged.then(|| "deadbeef".to_string()),
            merged_at: merged.then(|| "2026-07-21T19:00:00Z".to_string()),
            number: 905,
            html_url: "https://github.com/loopflowstudio/loopflow/pull/905".to_string(),
            head: GhRestHead {
                sha: Some("headsha".to_string()),
            },
        }
    }

    #[test]
    fn rest_merged_pr_maps_to_merged_state_with_commit_and_head() {
        // REST reports a merged PR as closed+merged:true; reconcile needs "merged"
        // and the head sha (the merged branch tip, carried forward on rotation).
        let info = rest_pr("closed", true, false).into_info("jack/task-1");
        assert_eq!(info.state, "merged");
        assert_eq!(info.merge_commit.as_deref(), Some("deadbeef"));
        assert_eq!(info.merged_at.as_deref(), Some("2026-07-21T19:00:00Z"));
        assert_eq!(info.head_sha.as_deref(), Some("headsha"));
        assert_eq!(info.branch, "jack/task-1");
    }

    #[test]
    fn rest_open_and_draft_and_closed_states_map_through() {
        assert_eq!(rest_pr("open", false, false).into_info("b").state, "open");
        assert_eq!(rest_pr("open", false, true).into_info("b").state, "draft");
        assert_eq!(
            rest_pr("closed", false, false).into_info("b").state,
            "closed"
        );
        // A non-merged PR never carries a merge commit.
        assert!(rest_pr("closed", false, false)
            .into_info("b")
            .merge_commit
            .is_none());
    }

    #[test]
    fn rate_limit_stderr_classifies_as_a_named_quota_degradation() {
        let reason = classify_pr_read_failure(
            905,
            "gh: API rate limit already exceeded for user ID 37011 (HTTP 403)",
        );
        assert!(reason.contains("rate limit"), "reason was: {reason}");
        assert!(reason.contains("#905"));
    }

    #[test]
    fn missing_pr_stderr_is_detected_but_a_5xx_is_not() {
        assert!(is_missing_pr("gh: Not Found (HTTP 404)"));
        assert!(!is_missing_pr("gh: Internal Server Error (HTTP 500)"));
    }

    #[test]
    fn required_checks_let_failure_dominate_pending() {
        let checks = RequiredChecks::from_checks(vec![
            check("build", "fail"),
            check("test", "pending"),
            check("lint", "pass"),
        ]);
        assert!(checks.failing);
        assert!(checks.pending);
        assert_eq!(
            checks
                .failing_checks
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            vec!["build"]
        );
    }

    #[test]
    fn required_checks_treat_cancel_as_failing() {
        let checks = RequiredChecks::from_checks(vec![check("deploy", "cancel")]);
        assert!(checks.failing);
        assert_eq!(checks.failing_checks.len(), 1);
    }

    #[test]
    fn required_checks_pending_only_when_nothing_failed() {
        let checks =
            RequiredChecks::from_checks(vec![check("build", "pending"), check("lint", "pass")]);
        assert!(!checks.failing);
        assert!(checks.pending);
        assert!(checks.failing_checks.is_empty());
    }

    #[test]
    fn required_checks_pass_when_all_green() {
        let checks =
            RequiredChecks::from_checks(vec![check("build", "pass"), check("lint", "skipping")]);
        assert!(!checks.failing);
        assert!(!checks.pending);
    }

    #[test]
    fn merge_gate_seeds_actionable_leaves_not_the_required_aggregate() {
        // Branch protection requires only the `tests-result` roll-up; the real
        // failure is the `rust-test` leaf. The gate is failing, and the ci-fix
        // seed names the leaf with the leaf's own job link — never the aggregate.
        let required = vec![check("tests-result", "fail")];
        let full = vec![
            check("tests-result", "fail"),
            check("rust-test", "fail"),
            check("python-test", "pass"),
        ];
        let reading = MergeGateReading::from_checks(required, full);
        assert!(reading.failing);
        let names: Vec<&str> = reading
            .failing_leaves
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(names, vec!["rust-test"]);
        assert!(
            !names.contains(&"tests-result"),
            "the aggregate never seeds a ci-fix turn"
        );
        assert_eq!(
            reading.failing_leaves[0].url.as_deref(),
            Some("https://ci/rust-test"),
            "the seed carries the leaf's own job link, not the roll-up's"
        );
    }

    #[test]
    fn merge_gate_keeps_a_required_leaf_when_it_is_the_only_failure() {
        // A repo that requires the leaf directly (no aggregate): the required
        // check *is* the actionable leaf, so it stays in the seed.
        let required = vec![check("rust-test", "fail")];
        let full = vec![check("rust-test", "fail"), check("lint", "pass")];
        let reading = MergeGateReading::from_checks(required, full);
        assert!(reading.failing);
        let names: Vec<&str> = reading
            .failing_leaves
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(names, vec!["rust-test"]);
    }

    #[test]
    fn merge_gate_falls_back_to_required_when_the_full_read_is_empty() {
        // gh gave no full check set (only the `--required` read succeeded): the
        // seed degrades to the required failing checks rather than emptying out.
        let required = vec![check("tests-result", "fail")];
        let reading = MergeGateReading::from_checks(required, vec![]);
        assert!(reading.failing);
        assert_eq!(
            reading
                .failing_leaves
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            vec!["tests-result"]
        );
    }

    #[test]
    fn created_pr_url_carries_the_attachment_number() {
        assert_eq!(
            pr_number_from_url("https://github.com/loopflowstudio/loopflow/pull/872"),
            Some(872)
        );
        assert_eq!(pr_number_from_url("https://example.com/not-a-pr"), None);
    }

    fn task_pr_context() -> TaskPrContext {
        TaskPrContext {
            title: "Make Task PR copy explain intent and lifecycle".to_string(),
            identifier: "LOO-249".to_string(),
            url: Some("https://linear.app/loopflow/issue/LOO-249/task-pr-copy".to_string()),
            merge_request: None,
        }
    }

    #[test]
    fn task_pr_copy_preserves_title_and_summary_before_durable_context() {
        let copy = normalize_task_pr_copy(
            PrCopy {
                title: "Understand what merging this PR will do".to_string(),
                body: "Reviewers can see what work remains.\nThe summary can wrap.\n\n## Evaluate\n\nRecorded proof.".to_string(),
            },
            Some(&task_pr_context()),
            &TaskPrCopyLifecycle::Completes,
        )
        .expect("normalize Task PR copy");

        assert_eq!(copy.title, "Understand what merging this PR will do");
        assert_eq!(
            copy.body,
            "Reviewers can see what work remains.\nThe summary can wrap.\n\n\
<!-- loopflow:task-pr-context:start -->\n\
> [!NOTE]\n\
> **Task:** [Make Task PR copy explain intent and lifecycle · LOO-249](https://linear.app/loopflow/issue/LOO-249/task-pr-copy)\n\
> **PR lifecycle:** Merge delivers the code. Record independent later work as Tasks and comments; complete the delivered outcome.\n\
<!-- loopflow:task-pr-context:end -->\n\n\
## Evaluate\n\nRecorded proof."
        );
    }

    #[test]
    fn task_pr_copy_recognizes_whitespace_only_paragraph_separators() {
        let render = |separator: &str| {
            normalize_task_pr_copy(
                PrCopy {
                    title: "Keep review context readable".to_string(),
                    body: format!(
                        "Summary.{separator}    indented example\n\n## Evaluate\n\nProof."
                    ),
                },
                Some(&task_pr_context()),
                &TaskPrCopyLifecycle::Published,
            )
            .unwrap()
        };
        let expected = render("\n\n");
        for separator in ["\n \t\n", "\r\n \r\n\r\n"] {
            let copy = render(separator);
            assert_eq!(copy, expected);
            assert_eq!(
                normalize_task_pr_copy(
                    copy.clone(),
                    Some(&task_pr_context()),
                    &TaskPrCopyLifecycle::Published
                )
                .unwrap(),
                copy
            );
        }
    }

    #[test]
    fn task_pr_copy_refreshes_lifecycle_and_scope_without_accumulating_context() {
        let mut context = task_pr_context();
        let mut copy = PrCopy {
            title: "Understand what merging this PR will do".to_string(),
            body: "Linear Task: [OLD-1](https://example.com/old)\n\nReviewers can see what work remains.\n\n\n    lf wave status example\n\n## Evaluate\n\nRecorded proof.".to_string(),
        };
        for (lifecycle, expected) in [
            (
                TaskPrCopyLifecycle::Draft,
                "is a draft; no Task settlement is requested.",
            ),
            (
                TaskPrCopyLifecycle::Published,
                "no Task settlement is requested.",
            ),
            (
                TaskPrCopyLifecycle::Completes,
                "complete the delivered outcome.",
            ),
        ] {
            copy = normalize_task_pr_copy(copy, Some(&context), &lifecycle).unwrap();
            assert!(copy
                .body
                .starts_with("Reviewers can see what work remains.\n\n<!--"));
            assert!(copy
                .body
                .ends_with("    lf wave status example\n\n## Evaluate\n\nRecorded proof."));
            assert_eq!(
                copy.body.matches("loopflow:task-pr-context:start").count(),
                1
            );
            assert!(copy.body.contains(expected));
            assert!(!copy.body.contains("Linear Task: [OLD-1]"));
            assert_eq!(
                normalize_task_pr_copy(copy.clone(), Some(&context), &lifecycle).unwrap(),
                copy
            );
        }

        context.title = "Find the next useful review action".to_string();
        copy.title = "Find the remaining proof after a partial delivery".to_string();
        copy.body = copy.body.replace(
            "Reviewers can see what work remains.",
            "Reviewers can find the proof still needed.",
        );
        let revised =
            normalize_task_pr_copy(copy, Some(&context), &TaskPrCopyLifecycle::Published).unwrap();
        assert_eq!(
            revised.title,
            "Find the remaining proof after a partial delivery"
        );
        assert!(revised
            .body
            .starts_with("Reviewers can find the proof still needed.\n\n<!--"));
        assert!(revised
            .body
            .contains("Find the next useful review action · LOO-249"));
        assert!(revised.body.contains(
            "The pull request is published for review; no Task settlement is requested."
        ));
        assert!(!revised.body.contains("completes the Task"));
        assert!(!revised.body.contains("Make Task PR copy explain intent"));
    }

    #[test]
    fn task_pr_copy_moves_existing_top_block_and_handles_summary_only() {
        let context = task_pr_context();
        let empty = normalize_task_pr_copy(
            PrCopy {
                title: "A specific change".to_string(),
                body: String::new(),
            },
            Some(&context),
            &TaskPrCopyLifecycle::Published,
        )
        .unwrap();
        let legacy = PrCopy {
            title: empty.title,
            body: format!("{}\n\n{}\n\nSummary only.", empty.body, empty.body),
        };
        let moved = normalize_task_pr_copy(legacy, Some(&context), &TaskPrCopyLifecycle::Completes)
            .unwrap();
        assert!(moved.body.starts_with("Summary only.\n\n<!--"));
        assert_eq!(
            moved.body.matches("loopflow:task-pr-context:start").count(),
            1
        );
        assert_eq!(
            normalize_task_pr_copy(
                moved.clone(),
                Some(&context),
                &TaskPrCopyLifecycle::Completes
            )
            .unwrap(),
            moved
        );
    }

    #[test]
    fn non_task_pr_copy_is_unchanged() {
        let copy = PrCopy {
            title: "auth: keep refresh ownership explicit".to_string(),
            body: "## Evaluate\n\nRun the auth smoke test.".to_string(),
        };

        assert_eq!(
            normalize_task_pr_copy(copy.clone(), None, &TaskPrCopyLifecycle::Published)
                .expect("normalize ordinary PR"),
            copy
        );
    }

    #[test]
    fn parse_generated_pr_copy_accepts_plain_json() {
        let raw = r###"{"title":"docs: tighten wave docs","body":"## Try it!"}"###;
        assert_eq!(
            parse_generated_pr_copy(raw),
            Some(PrCopy {
                title: "docs: tighten wave docs".to_string(),
                body: "## Try it!".to_string(),
            })
        );
    }

    #[test]
    fn parse_generated_pr_copy_ignores_non_json_braces_around_reply() {
        let raw = r###"warning: telemetry payload {ignored=true}
{"title":"docs: tighten wave docs","body":"## Try it!\n- run tests"}
info: done {ok=true}"###;
        assert_eq!(
            parse_generated_pr_copy(raw),
            Some(PrCopy {
                title: "docs: tighten wave docs".to_string(),
                body: "## Try it!\n- run tests".to_string(),
            })
        );
    }

    #[test]
    fn parse_generated_pr_copy_handles_braces_inside_body_strings() {
        let raw = r###"preface
{"title":"docs: tighten wave docs","body":"Use {native|container} and keep JSON like {\"a\":1}."}
trailer"###;
        assert_eq!(
            parse_generated_pr_copy(raw),
            Some(PrCopy {
                title: "docs: tighten wave docs".to_string(),
                body: "Use {native|container} and keep JSON like {\"a\":1}.".to_string(),
            })
        );
    }

    #[test]
    fn parse_generated_pr_copy_accepts_title_and_body_labels() {
        let raw = r#"Title: pm: add linear provider
Body:
## Usage

```bash
lf wave connect
```"#;
        assert_eq!(
            parse_generated_pr_copy(raw),
            Some(PrCopy {
                title: "pm: add linear provider".to_string(),
                body: "## Usage\n\n```bash\nlf wave connect\n```".to_string(),
            })
        );
    }

    #[test]
    fn parse_generated_pr_copy_prefers_final_object_over_prompt_schema() {
        let raw = r###"Return exactly one JSON object with this schema:
{"title":"...","body":"..."}
No markdown fences.

{"title":"wave: ship algedonic signals with repair backoff","body":"## Usage\n\nRun the demo."}"###;
        assert_eq!(
            parse_generated_pr_copy(raw),
            Some(PrCopy {
                title: "wave: ship algedonic signals with repair backoff".to_string(),
                body: "## Usage\n\nRun the demo.".to_string(),
            })
        );
    }

    #[test]
    fn parse_generated_pr_copy_handles_literal_newlines_in_body() {
        let raw = r###"codex
{"title":"wave: ship algedonic signals with repair backoff","body":"## Usage

```bash
cargo test repair_chain
```

## Summary

Repairs now back off before escalating."}"###;
        assert_eq!(
            parse_generated_pr_copy(raw),
            Some(PrCopy {
                title: "wave: ship algedonic signals with repair backoff".to_string(),
                body: "## Usage\n\n```bash\ncargo test repair_chain\n```\n\n## Summary\n\nRepairs now back off before escalating.".to_string(),
            })
        );
    }

    #[test]
    fn parse_generated_pr_copy_accepts_markdown_section_labels() {
        let raw = r#"## Title
pm: add linear provider

## Body
## Usage

- bootstrap a Linear-backed wave"#;
        assert_eq!(
            parse_generated_pr_copy(raw),
            Some(PrCopy {
                title: "pm: add linear provider".to_string(),
                body: "## Usage\n\n- bootstrap a Linear-backed wave".to_string(),
            })
        );
    }

    #[test]
    fn parse_generated_pr_copy_handles_unescaped_quotes_inside_body() {
        let raw = r###"{"title":"ops: harden pr copy parsing","body":"## Summary

Use "lf pr open" after gating to open or update the PR."}"###;
        assert_eq!(
            parse_generated_pr_copy(raw),
            Some(PrCopy {
                title: "ops: harden pr copy parsing".to_string(),
                body: "## Summary\n\nUse \"lf pr open\" after gating to open or update the PR."
                    .to_string(),
            })
        );
    }
}
