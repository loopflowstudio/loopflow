use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::engine::agent::{launch_agent, AgentCapabilities, AgentConfig, ProcessConfig};
use crate::engine::config::load_config_or_default;
use crate::engine::git::{current_branch, get_default_branch, rev_parse};
use crate::engine::load_skill;
use crate::engine::worktrees::{list_worktrees, main_repo_root};

use crate::ops::commit::{commit_workflow, CommitOptions};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::util::{command_exists, stderr_from_output};

#[derive(Debug, Clone)]
pub struct PrOptions {
    pub title: Option<String>,
    pub body: Option<String>,
    pub agent: Option<String>,
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
    /// GitHub's authoritative merge instant from the single-PR REST response.
    pub merged_at: Option<String>,
    /// The PR's current head commit (`headRefOid`), when GitHub reports one.
    pub head_sha: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrCopy {
    pub title: String,
    pub body: String,
}

const GITHUB_PR_TITLE_MAX_CHARS: usize = 256;
const FAILED_CHECK_LOG_MAX_CHARS: usize = 24_000;
const FAILED_CHECK_LOG_TIMEOUT: Duration = Duration::from_secs(30);
const TASK_PR_CONTEXT_START: &str = "<!-- loopflow:task-pr-context:start -->";
const TASK_PR_CONTEXT_END: &str = "<!-- loopflow:task-pr-context:end -->";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskPrCopyLifecycle {
    Published,
    Continues { next_slug: Option<String> },
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
    if !gh_available() {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }
    let task_context = crate::ops::task::task_pr_context(repo)?;

    let main_repo = resolve_main_repo(repo);
    let default_branch = get_default_branch(&main_repo)?;
    let stack = crate::ops::task::task_stack(repo)?;
    let base_branch = match stack.as_ref().and_then(|stack| stack.parent_branch.clone()) {
        Some(parent) => parent,
        None if stack.is_some() => default_branch.clone(),
        None => pr_target(repo, &main_repo, &default_branch)?,
    };

    // Prove the Task PR range without healing integration metadata before the
    // first remote side effect. No-op for non-Task worktrees.
    crate::ops::task::verify_task_pr_range_without_healing(repo)?;

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
    commit_workflow(repo, &commit_options, progress)?;
    crate::ops::task::require_task_pr_range_nonempty_without_healing(repo)?;
    require_non_task_pr_range_nonempty(repo, stack.is_some(), &base_branch)?;
    let branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    let published_head = rev_parse(repo, "HEAD")?;
    crate::ops::commit::push_with_upstream_if_needed(repo)?;

    let copy = normalize_task_pr_copy(
        resolve_pr_copy(repo, options, cached_copy, progress)?,
        task_context.as_ref(),
        &TaskPrCopyLifecycle::Published,
    )?;
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
    let locked_branch = current_branch(repo)?;
    let locked_head = rev_parse(repo, "HEAD")?;
    if locked_branch.as_deref() != Some(branch.as_str()) || locked_head != published_head {
        return Err(OpsError::Message(format!(
            "branch changed before PR publication; expected {branch} at {published_head}"
        )));
    }
    crate::ops::commit::verify_remote_branch_head(repo, &branch, &published_head)?;
    let title = copy.title.trim();
    let body = copy.body.trim();
    crate::ops::task::request_task_pr_publication(repo, title, body)?;

    let (result, pr) = if let Some(pr) = find_open_pr(repo)? {
        progress.status("Updating PR...");
        update_pr(repo, pr.number, title, body, &base_branch)?;
        if pr.is_draft {
            mark_pr_ready(repo, pr.number)?;
        }
        let info = pr_info(&branch, pr);
        (
            PrResult {
                url: info.url.clone(),
                created: false,
            },
            Some(info),
        )
    } else {
        progress.status("Creating PR...");
        let url = create_pr(repo, title, body, &base_branch)?;
        let visible = find_open_pr(repo)?;
        if let Some(pr) = &visible {
            if pr.is_draft {
                mark_pr_ready(repo, pr.number)?;
            }
        }
        let info = match visible {
            Some(pr) => Some(pr_info(&branch, pr)),
            None => pr_number_from_url(&url).map(|number| PrInfo {
                number,
                url: url.clone(),
                state: "open".to_string(),
                branch: branch.clone(),
                merge_commit: None,
                merged_at: None,
                head_sha: None,
            }),
        };
        (PrResult { url, created: true }, info)
    };
    crate::ops::task::attach_task_github_pr(repo, pr.as_ref())?;
    Ok(result)
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

    let title = context.pr_title();
    if title.chars().count() > GITHUB_PR_TITLE_MAX_CHARS {
        return Err(OpsError::Message(format!(
            "Task identifier and name exceed GitHub's {GITHUB_PR_TITLE_MAX_CHARS}-character PR title limit"
        )));
    }

    let task_link = context.task_link();
    let task_cycle = context.cycle().as_str();
    let pr_lifecycle = match lifecycle {
        TaskPrCopyLifecycle::Published => format!(
            "PR {} is published for review; no Task settlement is requested.",
            context.sequence
        ),
        TaskPrCopyLifecycle::Continues {
            next_slug: Some(next_slug),
        } => format!(
            "Merging PR {} leaves the Task open and names {} as the next serial PR.",
            context.sequence,
            _markdown_code(next_slug)
        ),
        TaskPrCopyLifecycle::Continues { next_slug: None } => format!(
            "Merging PR {} leaves the Task open for another serial PR.",
            context.sequence
        ),
        TaskPrCopyLifecycle::Completes => {
            format!("Merging PR {} completes the Task.", context.sequence)
        }
    };
    let managed = format!(
        "{TASK_PR_CONTEXT_START}\n> [!NOTE]\n> **Task:** {task_link}\n> **Task cycle:** {task_cycle}\n> **PR lifecycle:** {pr_lifecycle}\n{TASK_PR_CONTEXT_END}"
    );
    let reviewer_context = _strip_managed_task_context(&copy.body);
    let body = if reviewer_context.is_empty() {
        managed
    } else {
        format!("{managed}\n\n{reviewer_context}")
    };
    Ok(PrCopy { title, body })
}

fn _markdown_code(value: &str) -> String {
    format!("`{}`", value.replace('`', "\\`"))
}

fn _strip_managed_task_context(body: &str) -> String {
    let without_block = match (
        body.find(TASK_PR_CONTEXT_START),
        body.find(TASK_PR_CONTEXT_END),
    ) {
        (Some(start), Some(end)) if start < end => {
            let after = end + TASK_PR_CONTEXT_END.len();
            format!("{}\n{}", &body[..start], &body[after..])
        }
        _ => body.to_string(),
    };
    without_block
        .lines()
        .filter(|line| !line.trim_start().starts_with("Linear Task:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn pr_info(branch: &str, pr: GhPr) -> PrInfo {
    PrInfo {
        url: pr.url,
        number: pr.number,
        state: if pr.is_draft {
            "draft".to_string()
        } else {
            pr.state.to_ascii_lowercase()
        },
        branch: branch.to_string(),
        merge_commit: pr.merge_commit.map(|commit| commit.oid),
        merged_at: None,
        head_sha: pr.head_ref_oid,
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
            "the canonical checkout on main is the Wave/Project control plane and cannot open a PR; create a Linear task and run it with `lf task run <issue-id>`"
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
    let scratch = repo.join("scratch");
    if !scratch.exists() {
        return Ok(cached);
    }

    let mut removed = false;
    for entry in std::fs::read_dir(&scratch)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let gate_owned = matches!(
            name.as_ref(),
            ".pr-copy-ref" | "pr-title.txt" | "pr-body.md"
        ) || name.ends_with("-review.md");
        if gate_owned {
            std::fs::remove_file(path)?;
            removed = true;
        }
    }
    if removed {
        progress.status("Removing task-gate artifacts before publication");
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
    let capabilities = AgentCapabilities {
        chrome: config.chrome,
    };

    let result = launch_agent(&launch, &process, &capabilities)
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

    if let Some(pr) = find_open_pr(repo)? {
        let state = if pr.is_draft { "draft" } else { "open" }.to_string();
        return Ok(Some(PrInfo {
            url: pr.url,
            number: pr.number,
            state,
            branch,
            merge_commit: pr.merge_commit.map(|commit| commit.oid),
            merged_at: None,
            head_sha: pr.head_ref_oid,
        }));
    }

    Ok(None)
}

pub(crate) fn auto_merge_enabled(repo: &Path, number: u64) -> OpsResult<bool> {
    let query = "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){pullRequest(number:$number){autoMergeRequest{enabledAt} mergeQueueEntry{id}}}}";
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
            "--jq",
            ".data.repository.pullRequest | (.autoMergeRequest != null) or (.mergeQueueEntry != null)",
        ])
        .current_dir(repo)
        .output()?;
    if !observation.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("gh api graphql [pull request #{number} merge request]"),
            stderr: stderr_from_output(&observation),
        });
    }
    match String::from_utf8_lossy(&observation.stdout).trim() {
        "false" => Ok(false),
        "true" => Ok(true),
        value => Err(OpsError::Message(format!(
            "could not determine whether pull request #{number} has auto-merge enabled: {value:?}"
        ))),
    }
}

/// Revoke GitHub auto-merge for one PR before a stored request can be cleared.
/// The read makes replay idempotent after a prior disable succeeded.
pub(crate) fn disable_auto_merge(repo: &Path, number: u32) -> OpsResult<()> {
    if !auto_merge_enabled(repo, u64::from(number))? {
        return Ok(());
    }
    let output = Command::new("gh")
        .args(["pr", "merge", &number.to_string(), "--disable-auto"])
        .current_dir(repo)
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(OpsError::CommandFailed {
        command: format!("gh pr merge {number} --disable-auto"),
        stderr: stderr_from_output(&output),
    })
}

pub(crate) fn enable_auto_merge(
    repo: &Path,
    number: u64,
    title: Option<&str>,
    body: Option<&str>,
    head_sha: &str,
) -> OpsResult<()> {
    if auto_merge_enabled(repo, number)? {
        let number = u32::try_from(number).map_err(|_| {
            OpsError::Message(format!("pull request #{number} exceeds supported range"))
        })?;
        // A pre-existing remote arm carries no durable Loopflow head binding.
        // Replace it so every accepted Auto request crosses our exact-head
        // command boundary, even when GitHub already reports auto-merge.
        disable_auto_merge(repo, number)?;
    }

    let number_arg = number.to_string();
    let mut command = Command::new("gh");
    command
        .arg("pr")
        .arg("merge")
        .arg(&number_arg)
        .arg("--squash")
        .arg("--auto")
        .arg("--match-head-commit")
        .arg(head_sha);
    if let Some(title) = title {
        command.arg("--subject").arg(title);
    }
    if let Some(body) = body.filter(|body| !body.trim().is_empty()) {
        command.arg("--body").arg(body);
    }
    let output = command.current_dir(repo).output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(OpsError::CommandFailed {
        command: format!("gh pr merge {number} --squash --auto --match-head-commit {head_sha}"),
        stderr: stderr_from_output(&output),
    })
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
    /// A quota, network, or GitHub failure. `reason` is human-facing; the caller
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

/// One `TaskPr.publication.github` PR read via a single bounded REST call — no
/// enumeration. Task reconcile always holds a persisted PR number, so it never
/// needs `gh pr list`; an unpublished working PR has no number and is not read
/// remotely at all. A transport/quota/network failure returns `Degraded` rather
/// than erroring, so local Task control survives a GitHub outage.
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
    let Some((owner, name)) = crate::engine::worktrees::github_repo_nwo(repo) else {
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

/// A 404 from `gh api` means the PR ref no longer exists — distinct from a
/// quota/network failure, and not something to retry or treat as degraded.
fn is_missing_pr(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("http 404")
}

/// Turn a failed `gh api` read into a concise, human-facing degraded reason. The
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
        }
    }
}

/// Read the required-check state for `branch`'s open PR from GitHub.
///
/// Read the merge-gate state for `branch`'s head. `gh pr checks` exits non-zero
/// while checks are pending or failing, so valid JSON outranks the exit status.
/// A missing required-check set is distinct from an unreadable one: callers may
/// wait on the former, while watched landing must surface or back off the latter.
///
/// Branch protection frequently requires only an aggregate roll-up check (e.g.
/// `tests-result`) whose own job link points at the aggregation step, not the
/// leaf job that actually failed. So the gate state (failing/pending/passing) is
/// read from `--required` — the authoritative merge gate — while the failing
/// checks handed to a ci-fix turn are the actionable *leaves* read from the full
/// check set. Seeding a ci-fix turn with the aggregate gives the skill nothing
/// to act on; seeding the leaves points it at the broken job.
pub(crate) fn merge_gate_state(repo: &Path, branch: &str) -> OpsResult<Option<MergeGateReading>> {
    if !gh_available() {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }
    let required = read_check_set(repo, branch, true)?;
    if required.is_empty() {
        return Ok(None);
    }
    let full = read_check_set(repo, branch, false)?;
    Ok(Some(MergeGateReading::from_checks(required, full)))
}

fn read_check_set(repo: &Path, branch: &str, required: bool) -> OpsResult<Vec<GhCheck>> {
    let mut command = Command::new("gh");
    command.arg("pr").arg("checks").arg(branch);
    if required {
        command.arg("--required");
    }
    let output = command
        .arg("--json")
        .arg("name,bucket,link")
        .current_dir(repo)
        .output()?;
    parse_check_set_output(
        branch,
        required,
        output.status.success(),
        &output.stdout,
        &stderr_from_output(&output),
    )
}

fn parse_check_set_output(
    branch: &str,
    required: bool,
    succeeded: bool,
    stdout: &[u8],
    stderr: &str,
) -> OpsResult<Vec<GhCheck>> {
    if let Ok(checks) = serde_json::from_slice(stdout) {
        return Ok(checks);
    }
    if required && stderr.to_ascii_lowercase().contains("no required checks") {
        return Ok(Vec::new());
    }
    if !succeeded {
        let required_flag = if required { " --required" } else { "" };
        return Err(OpsError::CommandFailed {
            command: format!("gh pr checks {branch}{required_flag}"),
            stderr: stderr.to_string(),
        });
    }
    Err(OpsError::Message(format!(
        "could not parse GitHub check state for branch {branch}"
    )))
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
    fn from_checks(required: Vec<GhCheck>, full: Vec<GhCheck>) -> Self {
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

/// One immutable GitHub Actions run/job named by a check's details URL.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ActionsRunRef {
    run_id: String,
    job_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostedLogMode {
    Inspect,
    Repair,
}

impl HostedLogMode {
    fn flag(self) -> &'static str {
        match self {
            Self::Inspect => "--log",
            Self::Repair => "--log-failed",
        }
    }
}

impl ActionsRunRef {
    fn parse(value: &str) -> Option<Self> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }
        if trimmed.chars().all(|character| character.is_ascii_digit()) {
            return Some(Self {
                run_id: trimmed.to_string(),
                job_id: None,
            });
        }
        let after = trimmed.split_once("/actions/runs/")?.1;
        let mut parts = after.split('/');
        let run_id = parts.next()?;
        if !is_numeric(run_id) {
            return None;
        }
        let job_id = match parts.next() {
            Some("job" | "jobs") => parts.next().filter(|value| is_numeric(value)),
            _ => None,
        };
        Some(Self {
            run_id: run_id.to_string(),
            job_id: job_id.map(str::to_string),
        })
    }
}

fn is_numeric(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|character| character.is_ascii_digit())
}

/// Fetch the failed hosted step for one immutable GitHub Actions check URL.
pub(crate) fn failed_check_log(repo: &Path, name: &str, url: &str) -> OpsResult<String> {
    _failed_check_log(repo, name, url, HostedLogMode::Inspect)
}

pub(super) fn bounded_failed_check_log(repo: &Path, name: &str, url: &str) -> OpsResult<String> {
    _failed_check_log(repo, name, url, HostedLogMode::Repair)
}

fn _failed_check_log(repo: &Path, name: &str, url: &str, mode: HostedLogMode) -> OpsResult<String> {
    let run_ref = ActionsRunRef::parse(url).ok_or_else(|| match mode {
        HostedLogMode::Inspect if url.trim().is_empty() => {
            OpsError::Message("No details URL for this check; open the PR checks tab.".to_string())
        }
        HostedLogMode::Inspect => OpsError::Message(format!(
            "Logs not available via gh for this check. Open: {url}"
        )),
        HostedLogMode::Repair => {
            let source = if url.trim().is_empty() {
                "the check has no details URL".to_string()
            } else {
                format!("{url} is not a GitHub Actions run or job URL")
            };
            OpsError::Message(format!(
                "exact hosted failure evidence is unavailable for {name}: {source}"
            ))
        }
    })?;

    let mut command = Command::new("gh");
    command
        .arg("run")
        .arg("view")
        .arg(&run_ref.run_id)
        .arg(mode.flag());
    if let Some(job_id) = &run_ref.job_id {
        command.arg("--job").arg(job_id);
    }
    command.current_dir(repo);
    let output = command_output_with_timeout(&mut command, FAILED_CHECK_LOG_TIMEOUT).map_err(
        |error| match mode {
            HostedLogMode::Inspect => OpsError::Message(format!(
                "Couldn't fetch logs (run {}): {error}\nThe run may be missing, expired (>90 days), or private. Open: {url}",
                run_ref.run_id
            )),
            HostedLogMode::Repair => {
            OpsError::Message(format!(
                "exact hosted failure evidence is unavailable for {name}: {error}. Open {url}"
            ))
            }
        },
    )?;
    if !output.status.success() {
        let stderr = stderr_from_output(&output);
        return Err(match mode {
            HostedLogMode::Inspect => OpsError::Message(format!(
                "Couldn't fetch logs (run {}): {stderr}\nThe run may be missing, expired (>90 days), or private. Open: {url}",
                run_ref.run_id
            )),
            HostedLogMode::Repair => {
                let job = run_ref
                    .job_id
                    .as_deref()
                    .map(|job_id| format!(" --job {job_id}"))
                    .unwrap_or_default();
                OpsError::CommandFailed {
                    command: format!(
                        "gh run view {} {}{job}",
                        run_ref.run_id,
                        mode.flag()
                    ),
                    stderr: format!(
                        "exact hosted failure evidence is unavailable for {name}: {stderr}. Open {url}"
                    ),
                }
            }
        });
    }
    let log = String::from_utf8_lossy(&output.stdout);
    if mode == HostedLogMode::Repair && log.trim().is_empty() {
        return Err(OpsError::Message(format!(
            "exact hosted failure evidence is empty for {name}; open {url}"
        )));
    }
    match mode {
        HostedLogMode::Inspect => {
            let mut rendered = format!("### {name}\n\n{log}");
            if !rendered.ends_with('\n') {
                rendered.push('\n');
            }
            rendered.push('\n');
            Ok(rendered)
        }
        HostedLogMode::Repair => {
            let log = bounded_log_tail(&log, FAILED_CHECK_LOG_MAX_CHARS);
            Ok(format!("### {name}\nSource: {url}\n\n{log}\n"))
        }
    }
}

fn command_output_with_timeout(command: &mut Command, timeout: Duration) -> OpsResult<Output> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| OpsError::Message("command stdout was not captured".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| OpsError::Message("command stderr was not captured".to_string()))?;
    let stdout_reader = thread::spawn(move || {
        let mut output = Vec::new();
        let mut stdout = stdout;
        stdout.read_to_end(&mut output).map(|_| output)
    });
    let stderr_reader = thread::spawn(move || {
        let mut output = Vec::new();
        let mut stderr = stderr;
        stderr.read_to_end(&mut output).map(|_| output)
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(OpsError::Message(format!(
                "command exceeded its {} second deadline",
                timeout.as_secs()
            )));
        }
        thread::sleep(Duration::from_millis(25));
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| OpsError::Message("command stdout reader panicked".to_string()))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| OpsError::Message("command stderr reader panicked".to_string()))??;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn bounded_log_tail(text: &str, max_chars: usize) -> String {
    let characters = text.chars().count();
    if characters <= max_chars {
        return text.to_string();
    }
    let omitted = characters - max_chars;
    let start = text
        .char_indices()
        .nth(omitted)
        .map(|(index, _)| index)
        .unwrap_or(text.len());
    format!(
        "[{omitted} earlier hosted log characters omitted]\n{}",
        &text[start..]
    )
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

#[derive(Debug, Deserialize)]
struct GhCheck {
    #[serde(default)]
    name: String,
    #[serde(default)]
    bucket: String,
    #[serde(default)]
    link: Option<String>,
}

fn find_open_pr(repo: &Path) -> OpsResult<Option<GhPr>> {
    let branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    let output = Command::new("gh")
        .arg("pr")
        .arg("list")
        .arg("--head")
        .arg(&branch)
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

pub(crate) fn retarget_open_pr(repo: &Path, base: &str) -> OpsResult<()> {
    let Some(pr) = find_open_pr(repo)? else {
        return Ok(());
    };
    let output = Command::new("gh")
        .arg("pr")
        .arg("edit")
        .arg(pr.number.to_string())
        .arg("--base")
        .arg(base)
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "gh pr edit --base".to_string(),
            stderr: stderr_from_output(&output),
        });
    }
    Ok(())
}

fn mark_pr_ready(repo: &Path, number: u64) -> OpsResult<()> {
    let output = Command::new("gh")
        .arg("pr")
        .arg("ready")
        .arg(number.to_string())
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "gh pr ready".to_string(),
            stderr: stderr_from_output(&output),
        });
    }
    Ok(())
}

fn create_pr(repo: &Path, title: &str, body: &str, base: &str) -> OpsResult<String> {
    let mut cmd = Command::new("gh");
    cmd.arg("pr")
        .arg("create")
        .arg("--title")
        .arg(title)
        .arg("--body")
        .arg(body)
        .arg("--base")
        .arg(base);
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
) -> OpsResult<PrInfo> {
    let url = create_pr(repo, title, body, base)?;
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

#[derive(Debug, Deserialize)]
struct GeneratedPrCopy {
    title: String,
    body: String,
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
    let parsed: GeneratedPrCopy = serde_json::from_str(raw.trim()).ok()?;
    let title = parsed.title.trim().to_string();
    if title.is_empty() || is_placeholder_pr_copy(&title, &parsed.body) {
        return None;
    }
    Some(PrCopy {
        title,
        body: parsed.body,
    })
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
    use super::{
        bounded_log_tail, classify_pr_read_failure, command_output_with_timeout, is_missing_pr,
        normalize_task_pr_copy, parse_check_set_output, parse_generated_pr_copy,
        pr_number_from_url, ActionsRunRef, GhCheck, GhRestHead, GhRestPr, MergeGateReading, PrCopy,
        RequiredChecks, TaskPrCopyLifecycle,
    };
    use crate::ops::task::TaskPrContext;
    use crate::task::TaskLifecyclePlan;
    use std::process::Command;
    use std::time::Duration;

    fn check(name: &str, bucket: &str) -> GhCheck {
        GhCheck {
            name: name.to_string(),
            bucket: bucket.to_string(),
            link: Some(format!("https://ci/{name}")),
        }
    }

    fn rest_pr(state: &str, merged: bool, draft: bool) -> GhRestPr {
        GhRestPr {
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
    fn actions_run_reference_keeps_the_exact_job() {
        let reference = ActionsRunRef::parse(
            "https://github.com/loopflowstudio/loopflow/actions/runs/978123456/job/111222333",
        )
        .expect("job URL parses");
        assert_eq!(reference.run_id, "978123456");
        assert_eq!(reference.job_id.as_deref(), Some("111222333"));

        let plural = ActionsRunRef::parse(
            "https://github.com/loopflowstudio/loopflow/actions/runs/978123456/jobs/444555666",
        )
        .expect("plural job URL parses");
        assert_eq!(plural.job_id.as_deref(), Some("444555666"));

        let run = ActionsRunRef::parse(
            "https://github.com/loopflowstudio/loopflow/actions/runs/978123456",
        )
        .expect("run URL parses");
        assert_eq!(run.job_id, None);
        assert_eq!(ActionsRunRef::parse(" 978123456 "), Some(run));
    }

    #[test]
    fn actions_run_reference_rejects_non_actions_evidence() {
        assert_eq!(ActionsRunRef::parse("https://example.com/build/123"), None);
        assert_eq!(
            ActionsRunRef::parse("https://github.com/o/r/actions/runs/abc"),
            None
        );
        assert_eq!(ActionsRunRef::parse(""), None);
    }

    #[test]
    fn hosted_log_bound_keeps_the_failure_tail() {
        assert_eq!(
            bounded_log_tail("setup\nFAIL exact\n", 100),
            "setup\nFAIL exact\n"
        );
        assert_eq!(
            bounded_log_tail("discard this prefix\nFAIL exact\n", 11),
            "[20 earlier hosted log characters omitted]\nFAIL exact\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn hosted_log_command_has_a_finite_deadline() {
        let mut command = Command::new("sh");
        command.args(["-c", "while :; do :; done"]);

        let error = command_output_with_timeout(&mut command, Duration::from_millis(25))
            .expect_err("non-terminating evidence command times out");

        assert!(error.to_string().contains("deadline"));
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
    fn unreadable_required_checks_remain_an_error() {
        let error = parse_check_set_output(
            "jack/task",
            true,
            false,
            b"",
            "HTTP 403: authentication required",
        )
        .unwrap_err();
        assert!(error.to_string().contains("authentication required"));

        assert!(parse_check_set_output(
            "jack/task",
            true,
            false,
            b"",
            "no required checks reported on branch",
        )
        .unwrap()
        .is_empty());
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

    fn task_pr_context(first_flow: &str) -> TaskPrContext {
        let mut lifecycle = TaskLifecyclePlan::defaults();
        lifecycle.first.flow = first_flow.to_string();
        TaskPrContext {
            title: "Make Task PR copy explain intent and lifecycle".to_string(),
            identifier: "LOO-249".to_string(),
            url: "https://linear.app/loopflow/issue/LOO-249/task-pr-copy".to_string(),
            lifecycle,
            sequence: 1,
        }
    }

    #[test]
    fn fix_task_pr_copy_uses_one_canonical_title_and_durable_context() {
        let copy = normalize_task_pr_copy(
            PrCopy {
                title: "pr copy: explain the review contract".to_string(),
                body: "## Evaluate\n\n`cargo test -p loopflow task_pr_copy`".to_string(),
            },
            Some(&task_pr_context("incident")),
            &TaskPrCopyLifecycle::Completes,
        )
        .expect("normalize Task PR copy");

        assert_eq!(
            copy.title,
            "LOO-249: Make Task PR copy explain intent and lifecycle"
        );
        assert_eq!(
            copy.body,
            "<!-- loopflow:task-pr-context:start -->\n\
> [!NOTE]\n\
> **Task:** [LOO-249 — Make Task PR copy explain intent and lifecycle](https://linear.app/loopflow/issue/LOO-249/task-pr-copy)\n\
> **Task cycle:** fix\n\
> **PR lifecycle:** Merging PR 1 completes the Task.\n\
<!-- loopflow:task-pr-context:end -->\n\n\
## Evaluate\n\n`cargo test -p loopflow task_pr_copy`"
        );
    }

    #[test]
    fn feature_task_pr_copy_refreshes_lifecycle_without_duplicating_context() {
        let published = normalize_task_pr_copy(
            PrCopy {
                title: "ignored".to_string(),
                body: "Linear Task: [OLD-1](https://example.com/old)\n\nReviewer proof."
                    .to_string(),
            },
            Some(&task_pr_context("task-design")),
            &TaskPrCopyLifecycle::Published,
        )
        .expect("publish Task PR copy");
        let continued = normalize_task_pr_copy(
            published,
            Some(&task_pr_context("task-design")),
            &TaskPrCopyLifecycle::Continues {
                next_slug: Some("follow-up-proof".to_string()),
            },
        )
        .expect("refresh Task PR copy");

        assert_eq!(
            continued
                .body
                .matches("loopflow:task-pr-context:start")
                .count(),
            1
        );
        assert!(continued.body.contains(
            "Merging PR 1 leaves the Task open and names `follow-up-proof` as the next serial PR."
        ));
        assert!(continued.body.contains("> **Task cycle:** feature"));
        assert!(continued.body.ends_with("Reviewer proof."));
        assert!(!continued.body.contains("Linear Task: [OLD-1]"));
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
lf pm init
```"#;
        assert_eq!(
            parse_generated_pr_copy(raw),
            Some(PrCopy {
                title: "pm: add linear provider".to_string(),
                body: "## Usage\n\n```bash\nlf pm init\n```".to_string(),
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
