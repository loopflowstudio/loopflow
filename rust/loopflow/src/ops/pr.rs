use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::engine::agent::{launch_agent, AgentCapabilities, AgentConfig, ProcessConfig};
use crate::engine::builtins::get_builtin_ops_prompt;
use crate::engine::config::load_config_or_default;
use crate::engine::git::{current_branch, fetch, get_default_branch, rev_parse, sync_main};
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
    pub updated: bool,
}

#[derive(Debug, Clone)]
pub struct PrInfo {
    pub url: String,
    pub number: u64,
    pub state: String,
    pub branch: String,
    pub merge_commit: Option<String>,
    /// The PR's current head commit (`headRefOid`), when GitHub reports one.
    pub head_sha: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrCopy {
    pub title: String,
    pub body: String,
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

    let main_repo = resolve_main_repo(repo);
    let default_branch = get_default_branch(&main_repo)?;
    let stack = crate::ops::task::task_stack(repo)?;
    let base_branch = match stack.as_ref().and_then(|stack| stack.parent_branch.clone()) {
        Some(parent) => parent,
        None if stack.is_some() => default_branch.clone(),
        None => pr_target(repo, &main_repo, &default_branch)?,
    };

    // Commit the feature branch before syncing so PR context reflects pushed state.
    let commit_options = CommitOptions {
        add: true,
        push: true,
        message: Some("lf pr open: prepare branch".to_string()),
        agent: options.agent.clone(),
        ..CommitOptions::for_task("commit")
    };
    commit_workflow(repo, &commit_options, progress)?;

    if base_branch == default_branch {
        let _ = sync_main(&main_repo, &default_branch);
    } else {
        fetch(&main_repo, "origin", &base_branch)?;
    }

    let base_ref = format!("origin/{base_branch}");
    let stack_needs_rebase = stack.as_ref().is_some_and(|stack| {
        stack.parent_branch.is_none()
            || rev_parse(repo, &base_ref).is_ok_and(|tip| tip != stack.fork_base)
    });
    if stack_needs_rebase || (stack.is_none() && commits_behind(repo, &base_branch)? > 0) {
        progress.status("Branch behind base, rebasing...");
        crate::ops::rebase::rebase_with_recovery(
            repo,
            &crate::ops::rebase::RebaseOptions {
                onto: base_ref.clone(),
                push: true,
                fork_base: stack.as_ref().map(|stack| stack.fork_base.clone()),
            },
            progress,
        )?;
        if let Some(stack) = &stack {
            let new_base = rev_parse(repo, &base_ref)?;
            crate::ops::task::record_stack_rebase(stack, &new_base, stack.parent_branch.is_none())?;
        }
    }

    let copy = resolve_pr_copy(repo, options, progress)?;
    let title = copy.title.trim();
    let body = copy.body.trim();
    let branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    crate::ops::task::request_task_pr_publication(repo, crate::task::AfterMerge::Review, None)?;

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
                updated: true,
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
                head_sha: None,
            }),
        };
        (
            PrResult {
                url,
                created: true,
                updated: false,
            },
            info,
        )
    };
    crate::ops::task::attach_task_github_pr(repo, pr.as_ref())?;
    Ok(result)
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

    let mut generated = generate_pr_copy(repo, progress, options.agent.as_deref())?;
    if let Some(body_override) = options.body.as_deref() {
        generated.body = body_override.to_string();
    }
    Ok(generated)
}

pub fn generate_pr_copy(
    repo: &Path,
    progress: &impl Progress,
    agent_override: Option<&str>,
) -> OpsResult<PrCopy> {
    let template = get_builtin_ops_prompt("pr_message")
        .ok_or_else(|| OpsError::Message("builtin pr_message prompt not found".to_string()))?;
    let main_repo = resolve_main_repo(repo);
    let default_branch = get_default_branch(&main_repo)?;
    let base_branch = match crate::ops::task::task_stack(repo)? {
        Some(stack) => stack.parent_branch.unwrap_or(default_branch),
        None => pr_target(repo, &main_repo, &default_branch)?,
    };
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
    let agent = agent_override
        .map(str::to_string)
        .or_else(|| config.agent.clone())
        .unwrap_or_else(|| "claude:opus".to_string());
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
            head_sha: pr.head_ref_oid,
        }));
    }

    Ok(None)
}

pub(crate) fn current_or_merged_pr_for_branch(
    repo: &Path,
    branch: &str,
) -> OpsResult<Option<PrInfo>> {
    if !gh_available() {
        return Ok(None);
    }
    let output = Command::new("gh")
        .arg("pr")
        .arg("list")
        .arg("--head")
        .arg(branch)
        .arg("--state")
        .arg("all")
        .arg("--json")
        .arg("url,state,isDraft,number,mergeCommit,headRefOid")
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("gh pr list --head {branch} --state all"),
            stderr: stderr_from_output(&output),
        });
    }
    let list: Vec<GhPr> = serde_json::from_slice(&output.stdout).map_err(|error| {
        OpsError::Message(format!("failed to parse gh pr list output: {error}"))
    })?;
    Ok(select_reconcile_pr(list).map(|pr| PrInfo {
        url: pr.url,
        number: pr.number,
        state: if pr.is_draft {
            "draft".to_string()
        } else {
            pr.state.to_ascii_lowercase()
        },
        branch: branch.to_string(),
        merge_commit: pr.merge_commit.map(|commit| commit.oid),
        head_sha: pr.head_ref_oid,
    }))
}

/// A branch can carry several PRs — a merged one plus a stray open/draft or
/// closed sibling left behind by a land. The merged PR is the branch's truth;
/// reconcile must observe it. Rank **merged > open/draft > closed**, newest as
/// the tiebreak within a rank, so an out-of-band merge is never hidden by a
/// noisier sibling.
fn select_reconcile_pr(prs: Vec<GhPr>) -> Option<GhPr> {
    prs.into_iter()
        .max_by_key(|pr| (reconcile_rank(pr), pr.number))
}

fn reconcile_rank(pr: &GhPr) -> u8 {
    match pr.state.to_ascii_uppercase().as_str() {
        "MERGED" => 3,
        "CLOSED" => 1,
        _ => 2, // open, including drafts
    }
}

/// Read the required-check state for `branch`'s open PR from GitHub.
///
/// Uses `gh pr checks --required --json` — the authoritative view of the
/// branch-protection checks that gate the merge. `gh pr checks` exits non-zero
/// while checks are pending or failing, so the exit status is ignored: what
/// matters is whether stdout parses. Returns `None` when gh is unavailable, the
/// PR has no required checks configured, or the output cannot be read — CI state
/// is simply unknown, never a hard error on the reconcile path.
pub(crate) fn required_check_state(repo: &Path, branch: &str) -> Option<RequiredChecks> {
    if !gh_available() {
        return None;
    }
    let output = Command::new("gh")
        .arg("pr")
        .arg("checks")
        .arg(branch)
        .arg("--required")
        .arg("--json")
        .arg("name,bucket,link")
        .current_dir(repo)
        .output()
        .ok()?;
    // Empty stdout with a failure exit means "no required checks" (or an error we
    // treat as unknown), not "all green" — distinguish by whether JSON parses.
    let checks: Vec<GhCheck> = serde_json::from_slice(&output.stdout).ok()?;
    if checks.is_empty() {
        return None;
    }
    Some(RequiredChecks::from_checks(checks))
}

/// The classified required-check reading for one head: overall state plus the
/// checks that are not passing, named for the `ci-fix` skill.
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

fn resolve_main_repo(repo: &Path) -> PathBuf {
    main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf())
}

fn commits_behind(repo: &Path, base_branch: &str) -> OpsResult<i32> {
    let output = Command::new("git")
        .arg("rev-list")
        .arg("--count")
        .arg(format!("HEAD..origin/{}", base_branch))
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Ok(0);
    }
    let count = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<i32>()
        .unwrap_or(0);
    Ok(count)
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
        parse_generated_pr_copy, pr_number_from_url, select_reconcile_pr, GhCheck, GhPr, PrCopy,
        RequiredChecks,
    };

    fn gh_pr(number: u64, state: &str, is_draft: bool) -> GhPr {
        GhPr {
            url: format!("https://example.com/pr/{number}"),
            state: state.to_string(),
            is_draft,
            number,
            merge_commit: None,
            head_ref_oid: None,
        }
    }

    fn check(name: &str, bucket: &str) -> GhCheck {
        GhCheck {
            name: name.to_string(),
            bucket: bucket.to_string(),
            link: Some(format!("https://ci/{name}")),
        }
    }

    #[test]
    fn reconcile_prefers_the_merged_pr_over_a_newer_sibling() {
        // A land left a newer empty draft (#909) beside the merged PR (#905).
        let picked =
            select_reconcile_pr(vec![gh_pr(909, "OPEN", true), gh_pr(905, "MERGED", false)])
                .expect("a PR is selected");
        assert_eq!(picked.number, 905);
        assert_eq!(picked.state, "MERGED");
    }

    #[test]
    fn reconcile_prefers_open_over_closed_and_newest_within_a_rank() {
        assert_eq!(
            select_reconcile_pr(vec![gh_pr(20, "CLOSED", false), gh_pr(10, "OPEN", false)])
                .unwrap()
                .number,
            10
        );
        assert_eq!(
            select_reconcile_pr(vec![gh_pr(10, "OPEN", false), gh_pr(20, "OPEN", true)])
                .unwrap()
                .number,
            20
        );
        assert!(select_reconcile_pr(vec![]).is_none());
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
    fn created_pr_url_carries_the_attachment_number() {
        assert_eq!(
            pr_number_from_url("https://github.com/loopflowstudio/loopflow/pull/872"),
            Some(872)
        );
        assert_eq!(pr_number_from_url("https://example.com/not-a-pr"), None);
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
